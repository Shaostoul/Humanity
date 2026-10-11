// Add to Media cleans every file on this device before it goes up (2026-10-11,
// docs/design/media-showcase.md, "Files on the sender's device and in messages, exact", parts 1
// and 2), on the website: web/pages/media-app.js with web/shared/upload-clean.js, and the
// resumable upload's keep_metadata (web/pages/media-upload.js).
//
// Run: node --test scripts/tests/media-web-files.test.js
//
// The page runs on the shared stand-ins (media-web-harness.js), with the REAL cleaning module and
// real files built byte by byte (file-bytes.js): a phone photo with where it was taken and the
// camera's serial number in it, a video cut before its movie, a HEIC photo, a 70 MB phone video.
// What went up is read back from the stand-in relay's pieces.
//
// What it proves:
//  1. A phone photo: the form says what it holds, the switch is on, and what goes up is the
//     cleaned photo, the strip's own bytes, with no keep_metadata.
//  2. With the switch off, the photo goes exactly as picked, the start says keep_metadata: 1, and
//     the choice is remembered for the next file.
//  3. A file that could not be checked (a video cut before its movie, a HEIC photo) is never added
//     to Media: the words, and Upload off; kept by choice it goes as picked with keep_metadata.
//  4. With the protected setup on: no switch (a line instead), a photo is always cleaned, and a file
//     that could not be checked cannot be added at all.
//  5. A video over 64 MB is read by its box headers (nothing near its size is read before the
//     upload starts), and what goes up is byte for byte what the strip makes of the whole file.
//
// HOS_WEB_DIR points the test at another copy of web/. Red first: see the end of this file.

const test = require("node:test");
const assert = require("node:assert");
const path = require("node:path");

const H = require("./media-web-harness.js");
const { WEB, MIB, byId, text, respond, at, fakeStorage, settle, item, listAnswer, listRoutes, openPage } = H;
const UC = require(path.join(WEB, "shared", "upload-clean.js"));
const S = require(path.join(WEB, "shared", "strip-metadata.js"));
const B = require("./file-bytes.js");

const PHOTO = B.phoneJpeg();
const fileOf = (bytes, name, type) => new File([bytes], name, { type, lastModified: 1700000000000 });
const same = (a, b) => Buffer.from(a).equals(Buffer.from(b));

/** The page with the real cleaning, a stand-in relay that keeps every piece, and publishing that answers. */
async function page(opts) {
  const relay = H.resumableRelay();
  const storage = (opts && opts.storage) || fakeStorage();
  const p = await openPage({
    clean: UC, storage,
    routes: listRoutes(listAnswer([]), [
      ...relay.routes,
      [at("/api/media", "POST"), () => respond(200, { ok: true, item: item() })],
    ]),
  });
  return Object.assign(p, { relay, storage });
}

async function pickFile(doc, file) {
  const input = byId(doc, "media-file-input");
  input.files = [file];
  input.dispatch("change");
  await settle();
}

/** What went up: every piece the stand-in relay took, in order, joined. */
function sent(relay) {
  const parts = relay.pieces.slice().sort((a, b) => a.offset - b.offset).map((p) => Buffer.from(p.c.init.body));
  return new Uint8Array(Buffer.concat(parts));
}
const startBody = (relay) => relay.log.find((l) => l.kind === "start").c.body;

async function upload(doc) {
  byId(doc, "media-form-go").click();
  for (let i = 0; i < 20; i++) await settle();
}

function turnSwitch(doc, on) {
  const box = byId(doc, "media-form-remove-details");
  box.checked = on;
  box.dispatch("change");
}

// ── 1. A phone photo ─────────────────────────────────────────────────────

test("1. a phone photo: the form says what it holds, the switch is on, and the cleaned photo goes up with no keep_metadata", async () => {
  const { doc, relay } = await page();
  await pickFile(doc, fileOf(PHOTO, "IMG_0001.jpg", "image/jpeg"));
  assert.equal(text(byId(doc, "media-form-hidden")), "This photo says where it was taken and which camera took it.", "what it holds");
  assert.equal(byId(doc, "media-form-remove-details").checked, true, "the switch on");
  assert.ok(text(byId(doc, "media-form-details")).includes(UC.WORDS.switchLabel), "with its label");
  await upload(doc);
  const want = S.stripFile(PHOTO, "IMG_0001.jpg").bytes;
  assert.ok(same(sent(relay), want), "the cleaned photo went up, the strip's own bytes");
  assert.ok(!B.contains(sent(relay), "SN-0042-SECRET"), "its camera's serial number gone");
  const body = startBody(relay);
  assert.equal(body.size, want.length, "the start gives the cleaned size");
  assert.equal(body.filename, "IMG_0001.jpg");
  assert.ok(!("keep_metadata" in body), "no keep_metadata");
});

// ── 2. The switch off ────────────────────────────────────────────────────

test("2. with the switch off the photo goes as picked, the start says keep_metadata: 1, and the choice is remembered", async () => {
  const { doc, relay, storage } = await page();
  await pickFile(doc, fileOf(PHOTO, "IMG_0001.jpg", "image/jpeg"));
  turnSwitch(doc, false);
  assert.equal(text(byId(doc, "media-form-details-off")), UC.WORDS.switchOffLine, "it says everything goes");
  await upload(doc);
  assert.ok(same(sent(relay), PHOTO), "exactly as picked");
  assert.equal(startBody(relay).keep_metadata, 1, "keep_metadata: 1");
  assert.equal(storage.m.get(UC.REMOVE_DETAILS_KEY), "0", "remembered on this device");
  await pickFile(doc, fileOf(PHOTO, "IMG_0002.jpg", "image/jpeg"));
  assert.equal(byId(doc, "media-form-remove-details").checked, false, "the next form starts as last chosen");
});

// ── 3. Could not be checked ──────────────────────────────────────────────

test("3. a video cut before its movie and a HEIC photo are never added unchecked; kept by choice they go as picked", async () => {
  const cut = B.movie("mdatFirst", B.ftypMp4(), B.everything()).subarray(0, 300);
  for (const [name, file, first] of [
    ["a cut video", fileOf(cut, "clip.mp4", "video/mp4"), null],
    ["a HEIC photo", fileOf(B.heic(), "IMG_0003.HEIC", "image/heic"), S.SAY.heif],
  ]) {
    const { doc, relay } = await page();
    await pickFile(doc, file);
    const words = text(byId(doc, "media-form-unchecked"));
    assert.ok(words.includes(UC.WORDS.publicKeepHint), `${name}: the words say why and how: ${words}`);
    if (first) assert.ok(words.startsWith(first), `${name}: the relay's own words`);
    assert.equal(byId(doc, "media-form-go").disabled, true, `${name}: Upload off`);
    byId(doc, "media-form-go").disabled = false; // a determined click
    await upload(doc);
    assert.equal(relay.log.length, 0, `${name}: nothing started`);
    turnSwitch(doc, true);
    turnSwitch(doc, false);
    assert.equal(text(byId(doc, "media-form-unchecked")), "", `${name}: kept by choice, nothing in the way`);
    assert.equal(byId(doc, "media-form-go").disabled, false);
    await upload(doc);
    assert.equal(startBody(relay).keep_metadata, 1, `${name}: keep_metadata: 1`);
    assert.ok(same(sent(relay), new Uint8Array(await file.arrayBuffer())), `${name}: as picked`);
  }
});

// ── 4. The protected setup ───────────────────────────────────────────────

test("4. with the protected setup on: no switch, a photo always cleaned, an unchecked file not added at all", async () => {
  const storage = fakeStorage();
  storage.setItem(UC.PROTECTED_STORAGE_KEY, JSON.stringify({ v: 1, on: true }));
  storage.setItem(UC.REMOVE_DETAILS_KEY, "0");
  let p = await page({ storage });
  await pickFile(p.doc, fileOf(PHOTO, "IMG_0001.jpg", "image/jpeg"));
  assert.equal(byId(p.doc, "media-form-remove-details"), null, "no switch");
  assert.equal(text(byId(p.doc, "media-form-protected")), UC.WORDS.protectedLine, "the protected setup's line");
  await upload(p.doc);
  assert.ok(same(sent(p.relay), S.stripFile(PHOTO, "x.jpg").bytes), "cleaned, whatever the switch last said");
  assert.ok(!("keep_metadata" in startBody(p.relay)), "never keep_metadata");

  p = await page({ storage });
  await pickFile(p.doc, fileOf(B.heic(), "IMG_0003.HEIC", "image/heic"));
  assert.equal(byId(p.doc, "media-form-go").disabled, true, "an unchecked file: Upload off");
  assert.ok(text(byId(p.doc, "media-form-unchecked")).includes(UC.WORDS.protectedRefused), "saying why");
  assert.equal(byId(p.doc, "media-form-remove-details"), null, "and no way round it");
});

// ── 5. A big video ───────────────────────────────────────────────────────

test("5. a 70 MB phone video is read by its box headers, and what goes up is the whole-file strip's bytes", async () => {
  const video = B.phoneVideo("mdatFirst", { frames: 35 * MIB });
  assert.ok(video.length > UC.WHOLE_MAX, "test setup: over the whole-file limit");
  const file = fileOf(video, "VID_0004.MOV", "video/quicktime");
  // Every read of the picked file's bytes (a slice is only a view; reading it is what costs).
  const reads = [];
  const slice = file.slice.bind(file);
  file.slice = (a, b) => {
    const part = slice(a, b);
    const read = part.arrayBuffer.bind(part);
    part.arrayBuffer = () => { reads.push(part.size); return read(); };
    return part;
  };
  const { doc, relay } = await page();
  await pickFile(doc, file);
  assert.equal(text(byId(doc, "media-form-hidden")), "This video says where it was filmed and which camera filmed it.");
  assert.ok(Math.max(...reads) < 8 * MIB, "nothing near its size read before the upload: largest read " + Math.max(...reads));
  await upload(doc);
  const want = S.stripFile(video, "VID_0004.MOV");
  assert.ok(want.changed, "test setup: the strip changes it");
  const up = sent(relay);
  assert.equal(up.length, video.length, "the same length");
  assert.ok(same(up, want.bytes), "byte for byte the whole-file strip's");
  assert.ok(!B.contains(up, B.PLACE), "where it was filmed is gone");
});

// ── Red first ────────────────────────────────────────────────────────────
// 2026-10-11: each break made in a copy of web/ and this file run against it with HOS_WEB_DIR,
// seen failing with the assertion named; every other test passed.
//  M1 media-app.js, startUpload sending `f.file` (as picked) instead of `sending.file`: test 1
//     "the cleaned photo went up, the strip's own bytes".
//  M2 media-upload.js, the start's body without keep_metadata: test 2 "keep_metadata: 1".
//  M3 media-app.js, sendingOf deciding as a private conversation ('private'): test 3 "a cut video:
//     the words say why and how" (it asked instead of refusing).
//  M4 upload-clean.js, the whole-file branch taken at any size (`if (true)`): test 5 "nothing near
//     its size read before the upload: largest read 73401162".
//  M5 media-app.js, f.protectedOn never set: test 4 "no switch".
