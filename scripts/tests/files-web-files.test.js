// Add to Media cleans every file on this device before it goes up (2026-10-11,
// docs/design/files-showcase.md, "Files on the sender's device and in messages, exact", parts 1
// and 2), on the website: web/pages/files-app.js with web/shared/upload-clean.js, and the
// resumable upload's keep_metadata (web/pages/files-upload.js).
//
// Run: node --test scripts/tests/files-web-files.test.js
//
// The page runs on the shared stand-ins (files-web-harness.js), with the REAL cleaning module and
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
//  6. When the server refuses the file at the finish because it could not clean it (422, BUG-275),
//     the chat's question: Don't send stores nothing; Send anyway uploads it again exactly as picked,
//     signed. The protected setup, and any other refusal, keep the server's words.
//
// HOS_WEB_DIR points the test at another copy of web/. Red first: see the end of this file.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const H = require("./files-web-harness.js");
const { WEB, MIB, byId, text, respond, at, fakeStorage, settle, item, listAnswer, listRoutes, openPage } = H;
const UC = require(path.join(WEB, "shared", "upload-clean.js"));
const S = require(path.join(WEB, "shared", "strip-metadata.js"));
const B = require("./file-bytes.js");

const PHOTO = B.phoneJpeg();
const fileOf = (bytes, name, type) => new File([bytes], name, { type, lastModified: 1700000000000 });
const same = (a, b) => Buffer.from(a).equals(Buffer.from(b));

/** The page with the real cleaning, a stand-in relay that keeps every piece, and publishing that answers. */
async function page(opts) {
  const relay = H.resumableRelay(opts && opts.hooks);
  const storage = (opts && opts.storage) || fakeStorage();
  const p = await openPage({
    clean: UC, storage,
    routes: listRoutes(listAnswer([]), [
      ...relay.routes,
      [at("/api/files", "POST"), () => respond(200, { ok: true, item: item() })],
    ]),
  });
  return Object.assign(p, { relay, storage });
}

async function pickFile(doc, file) {
  const input = byId(doc, "files-file-input");
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
  byId(doc, "files-form-go").click();
  for (let i = 0; i < 20; i++) await settle();
}

function turnSwitch(doc, on) {
  const box = byId(doc, "files-form-remove-details");
  box.checked = on;
  box.dispatch("change");
}

// ── 1. A phone photo ─────────────────────────────────────────────────────

test("1. a phone photo: the form says what it holds, the switch is on, and the cleaned photo goes up with no keep_metadata", async () => {
  const { doc, relay } = await page();
  await pickFile(doc, fileOf(PHOTO, "IMG_0001.jpg", "image/jpeg"));
  assert.equal(text(byId(doc, "files-form-hidden")), "This photo says where it was taken and which camera took it.", "what it holds");
  assert.equal(byId(doc, "files-form-remove-details").checked, true, "the switch on");
  assert.ok(text(byId(doc, "files-form-details")).includes(UC.WORDS.switchLabel), "with its label");
  // Since 2026-10-17 the form is in tinted parts (files-web.test.js 13): what the file holds and the
  // switch are in its "Files" part, with the file. Seen red with detailsBox put after the parts:
  // "what it holds is in the Files part" (actual null).
  const partOf = (el) => { for (let n = el; n; n = n.parentNode) if (String(n.className).split(/\s+/).includes("tint-section")) return n; return null; };
  const part = partOf(byId(doc, "files-form-details"));
  assert.equal(part && part.className + ": " + text(part.children[0]), "tint-section tint-blue: Files", "what it holds is in the Files part");
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
  assert.equal(text(byId(doc, "files-form-details-off")), UC.WORDS.switchOffLine, "it says everything goes");
  await upload(doc);
  assert.ok(same(sent(relay), PHOTO), "exactly as picked");
  assert.equal(startBody(relay).keep_metadata, 1, "keep_metadata: 1");
  // With the person's signed choice over the file's BLAKE3 (BUG-260), which the relay requires.
  const keep = startBody(relay).keep;
  assert.ok(keep && keep.sig && startBody(relay).blake3, "with the signed choice and the hash");
  assert.ok(Buffer.from(keep.sig, "base64").toString().startsWith("hum/upload-keep/v1\n"), "signed under the relay's domain");
  assert.equal(storage.m.get(UC.REMOVE_DETAILS_KEY), "0", "remembered on this device");
  await pickFile(doc, fileOf(PHOTO, "IMG_0002.jpg", "image/jpeg"));
  assert.equal(byId(doc, "files-form-remove-details").checked, false, "the next form starts as last chosen");
});

// ── 3. Could not be checked ──────────────────────────────────────────────

// THE CHAT'S QUESTION, NOT A REFUSAL (2026-11-07, BUG-264; the operator: "Let's do that."). A video
// cut before its movie could not be checked: the form asks the chat's question by key, "Don't send"
// first, holding the focus, then "Send anyway", each with its hint, in place of Upload and Cancel.
// "Don't send" closes the form with nothing started; "Send anyway" uploads it exactly as picked
// with keep_metadata and the signed choice. The switch turned off still sends it the same way.
// Seen red 2026-11-07 against the files-add.js and upload-clean.js from before: "a cut video: the
// chat's question" failed (the old refusal, "...turn off \"Remove location and camera details\".").
test("3. a video that could not be checked is asked about: Don't send first, Send anyway sends it as picked, signed; a HEIC goes to the server", async () => {
  const cut = B.movie("mdatFirst", B.ftypMp4(), B.everything()).subarray(0, 300);
  const focused = { el: null };
  const realFocus = H.FakeElement.prototype.focus;
  H.FakeElement.prototype.focus = function () { focused.el = this; };
  try {
    const name = "a cut video";
    const file = fileOf(cut, "clip.mp4", "video/mp4");
    let { doc, relay } = await page();
    await pickFile(doc, file);
    assert.equal(text(byId(doc, "files-form-unchecked")), "upload_keep.question_video", `${name}: the chat's question`);
    const no = byId(doc, "files-form-dont-send");
    const yes = byId(doc, "files-form-send-anyway");
    assert.ok(no && yes && !byId(doc, "files-form-go"), `${name}: the two answers in place of Upload`);
    assert.equal(no.parentNode.children.indexOf(no), 0, `${name}: Don't send first`);
    assert.equal(focused.el, no, `${name}: Don't send holds the focus`);
    assert.deepEqual([text(no), no.getAttribute("data-tip-desc"), text(yes), yes.getAttribute("data-tip-desc")],
      ["upload_keep.dont_send", "upload_keep.dont_send_hint", "upload_keep.send_anyway", "upload_keep.send_anyway_hint"], `${name}: the words and hints by key`);
    no.click();
    for (let i = 0; i < 5; i++) await settle();
    assert.equal(relay.log.length, 0, `${name}: Don't send starts nothing`);
    assert.equal(byId(doc, "files-form-send-anyway"), null, `${name}: and closes the form`);
    assert.ok(text(doc.body).includes("upload_keep.not_sent"), `${name}: and the page says so`);

    ({ doc, relay } = await page());
    await pickFile(doc, file);
    byId(doc, "files-form-send-anyway").click();
    for (let i = 0; i < 20; i++) await settle();
    assert.equal(startBody(relay).keep_metadata, 1, `${name}: Send anyway says keep_metadata: 1`);
    assert.ok(startBody(relay).keep && startBody(relay).blake3, `${name}: with the signed choice (BUG-260)`);
    assert.ok(same(sent(relay), new Uint8Array(await file.arrayBuffer())), `${name}: exactly as picked`);

    ({ doc, relay } = await page());
    await pickFile(doc, file);
    turnSwitch(doc, false);
    assert.equal(text(byId(doc, "files-form-unchecked")), "", `${name}: kept by choice, nothing to ask`);
    assert.equal(byId(doc, "files-form-go").disabled, false);
    await upload(doc);
    assert.equal(startBody(relay).keep_metadata, 1, `${name}: the switch off says keep_metadata: 1 too`);
    assert.ok(startBody(relay).keep && startBody(relay).blake3, `${name}: signed the same way`);
    assert.ok(same(sent(relay), new Uint8Array(await file.arrayBuffer())), `${name}: as picked`);
  } finally {
    H.FakeElement.prototype.focus = realFocus;
  }
  // A HEIC photo goes up as picked, without keep_metadata, so the relay cleans it (2026-10-11).
  const heic = fileOf(B.heic(), "IMG_0003.HEIC", "image/heic");
  const { doc, relay } = await page();
  await pickFile(doc, heic);
  assert.equal(text(byId(doc, "files-form-unchecked")), "", "a HEIC: nothing in the way");
  assert.equal(byId(doc, "files-form-go").disabled, false, "a HEIC: Upload on");
  await upload(doc);
  assert.ok(!("keep_metadata" in startBody(relay)), "a HEIC: no keep_metadata, so the server cleans it");
  assert.ok(same(sent(relay), new Uint8Array(await heic.arrayBuffer())), "a HEIC: as picked");
});

// ── 4. The protected setup ───────────────────────────────────────────────

test("4. with the protected setup on: no switch, a photo always cleaned, an unchecked file not added at all", async () => {
  const storage = fakeStorage();
  storage.setItem(UC.PROTECTED_STORAGE_KEY, JSON.stringify({ v: 1, on: true }));
  storage.setItem(UC.REMOVE_DETAILS_KEY, "0");
  let p = await page({ storage });
  await pickFile(p.doc, fileOf(PHOTO, "IMG_0001.jpg", "image/jpeg"));
  assert.equal(byId(p.doc, "files-form-remove-details"), null, "no switch");
  assert.equal(text(byId(p.doc, "files-form-protected")), UC.WORDS.protectedLine, "the protected setup's line");
  await upload(p.doc);
  assert.ok(same(sent(p.relay), S.stripFile(PHOTO, "x.jpg").bytes), "cleaned, whatever the switch last said");
  assert.ok(!("keep_metadata" in startBody(p.relay)), "never keep_metadata");

  const cut = B.movie("mdatFirst", B.ftypMp4(), B.everything()).subarray(0, 300);
  p = await page({ storage });
  await pickFile(p.doc, fileOf(cut, "clip.mp4", "video/mp4"));
  assert.equal(byId(p.doc, "files-form-go").disabled, true, "an unchecked file: Upload off");
  assert.ok(text(byId(p.doc, "files-form-unchecked")).includes(UC.WORDS.protectedRefused), "saying why");
  assert.equal(byId(p.doc, "files-form-remove-details"), null, "and no way round it");

  // A HEIC goes to the server, which cleans it, under the protected setup too.
  p = await page({ storage });
  await pickFile(p.doc, fileOf(B.heic(), "IMG_0003.HEIC", "image/heic"));
  await upload(p.doc);
  assert.ok(!("keep_metadata" in startBody(p.relay)), "a HEIC under the protected setup: never keep_metadata");
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
  assert.equal(text(byId(doc, "files-form-hidden")), "This video says where it was filmed and which camera filmed it.");
  assert.ok(Math.max(...reads) < 8 * MIB, "nothing near its size read before the upload: largest read " + Math.max(...reads));
  await upload(doc);
  const want = S.stripFile(video, "VID_0004.MOV");
  assert.ok(want.changed, "test setup: the strip changes it");
  const up = sent(relay);
  assert.equal(up.length, video.length, "the same length");
  assert.ok(same(up, want.bytes), "byte for byte the whole-file strip's");
  assert.ok(!B.contains(up, B.PLACE), "where it was filmed is gone");
});

// ── 6. The server's 422 at the finish ────────────────────────────────────

// The relay's own words for a HEIC or AVIF photo it could not read through (upload_rules.rs).
const RULES = fs.readFileSync(path.join(H.ROOT, "src", "relay", "upload_rules.rs"), "utf8");
const HEIF_REFUSED = RULES.match(/pub\(crate\) const HEIF_REFUSED: &str = "([^"]+)";/)[1];

/** The stand-in relay's hooks for a server that refuses the first finish as the relay does when it
 *  cannot clean the file (422, upload_rules.rs CANNOT_CLEAN), dropping that upload: nothing stored. */
function refusingFirstFinish(status, words) {
  let refused = 0;
  return { finish: (sess) => { if (refused++) return null; sess.deleted = true; return respond(status, words); } };
}
const startsOf = (relay) => relay.log.filter((l) => l.kind === "start").map((l) => l.c.body);
/** What went up in one upload (its session's pieces, in order, joined). */
const piecesOf = (relay, id) => new Uint8Array(Buffer.concat(relay.pieces.filter((p) => p.id === id).sort((a, b) => a.offset - b.offset).map((p) => Buffer.from(p.c.init.body))));
const publishes = (net) => net.calls.filter((c) => H.path0(c.url) === "/api/files" && (c.init.method || "GET") === "POST");
/** The question's window (upload-clean.js askSendAnyway) and its two buttons, in their order. */
function question(doc) {
  const overlay = byId(doc, "keep-ask-overlay");
  return overlay ? { overlay, buttons: overlay.querySelectorAll("button") } : null;
}

// THE SERVER COULD NOT CLEAN IT AT THE FINISH (2026-11-07, BUG-275; BUG-264's "Not in this fix"): a
// HEIC photo goes up as picked for the server to clean (test 3), and a server that cannot read it
// through refuses it at the finish with 422 (the relay's CANNOT_CLEAN, here HEIF_REFUSED) and drops
// the upload, nothing stored. Add to Files asks the chat's question in its own window (upload-clean.js
// askSendAnyway, as the chat does at its 422): "Don't send" first and holding the focus. "Don't send"
// ends it: the row goes, nothing starts again, nothing is published, the page says so. "Send anyway"
// starts the upload again, the photo exactly as picked with keep_metadata and the signed choice, and
// publishes it. A phone photo cleaned on this device that the server refuses the same way goes again
// AS PICKED, not as cleaned. Under the protected setup the server's words stay (the setup sends no file
// whose details could not be removed), and so does any other refusal's.
// Seen red 2026-11-07 against the files-upload.js and files-add.js from before (HOS_WEB_DIR at a copy of
// web/ from HEAD): "a HEIC refused at the finish: the question opens" failed (actual null: the row
// said the server's words "HEIC and AVIF photos cannot be cleaned of where they were taken yet...").
test("6. the server's 422 at the finish asks: Don't send stores nothing, Send anyway uploads it again as picked, signed; protected and other refusals keep the words", async () => {
  const heic = fileOf(B.heic(), "IMG_0003.HEIC", "image/heic");
  const heicBytes = new Uint8Array(await heic.arrayBuffer());
  const focused = { el: null };
  const realFocus = H.FakeElement.prototype.focus;
  H.FakeElement.prototype.focus = function () { focused.el = this; realFocus.call(this); };
  try {
    // Don't send.
    let p = await page({ hooks: refusingFirstFinish(422, HEIF_REFUSED) });
    await pickFile(p.doc, heic);
    await upload(p.doc);
    let q = question(p.doc);
    assert.ok(q, "a HEIC refused at the finish: the question opens");
    assert.ok(text(q.overlay).includes("upload_keep.question_picture"), "the chat's question, by key");
    assert.deepEqual(q.buttons.map((b) => b.dataset.keepAsk), ["dont-send", "send-anyway"], "Don't send first");
    assert.equal(focused.el, q.buttons[0], "holding the focus");
    assert.ok(q.buttons.every((b) => b.getAttribute("data-tip-desc")), "each with its hint");
    q.buttons[0].onclick();
    for (let i = 0; i < 20; i++) await settle();
    assert.equal(question(p.doc), null, "Don't send: the question goes");
    assert.equal(startsOf(p.relay).length, 1, "nothing starts again");
    assert.equal(publishes(p.net).length, 0, "nothing is published");
    assert.equal(p.page.state.uploads.length, 0, "the row goes");
    assert.ok(p.relay.sessions.get("a".repeat(32)).deleted && !p.relay.sessions.get("a".repeat(32)).done, "the server stored nothing");
    assert.ok(text(p.doc.body).includes("upload_keep.not_sent"), "and the page says so");

    // Send anyway.
    p = await page({ hooks: refusingFirstFinish(422, HEIF_REFUSED) });
    await pickFile(p.doc, heic);
    await upload(p.doc);
    question(p.doc).buttons[1].onclick();
    for (let i = 0; i < 30; i++) await settle();
    const starts = startsOf(p.relay);
    assert.equal(starts.length, 2, "Send anyway: the upload starts again");
    assert.ok(!("keep_metadata" in starts[0]), "test setup: the first went for the server to clean");
    assert.equal(starts[1].keep_metadata, 1, "keep_metadata: 1");
    assert.ok(starts[1].keep && starts[1].keep.sig && starts[1].blake3, "with the signed choice (BUG-260)");
    assert.ok(Buffer.from(starts[1].keep.sig, "base64").toString().startsWith("hum/upload-keep/v1\n" + H.ME + "\n" + starts[1].blake3 + "\n"), "signed by me, for this very file");
    assert.ok(same(piecesOf(p.relay, "b".repeat(32)), heicBytes), "the photo exactly as picked");
    assert.equal(publishes(p.net).length, 1, "and it is published");
    assert.equal(question(p.doc), null, "the question is gone");
  } finally {
    H.FakeElement.prototype.focus = realFocus;
  }

  // A photo cleaned on this device, refused at the finish all the same: Send anyway sends it AS PICKED.
  let p = await page({ hooks: refusingFirstFinish(422, "This picture could not be read all the way through...") });
  await pickFile(p.doc, fileOf(PHOTO, "IMG_0001.jpg", "image/jpeg"));
  await upload(p.doc);
  assert.ok(same(piecesOf(p.relay, "a".repeat(32)), S.stripFile(PHOTO, "IMG_0001.jpg").bytes), "test setup: the cleaned photo went first");
  question(p.doc).buttons[1].onclick();
  for (let i = 0; i < 30; i++) await settle();
  assert.equal(startsOf(p.relay)[1].keep_metadata, 1, "a cleaned photo: Send anyway keeps its details");
  assert.ok(same(piecesOf(p.relay, "b".repeat(32)), PHOTO), "a cleaned photo: Send anyway sends it exactly as picked");

  // Under the protected setup: the server's words, no question.
  const storage = fakeStorage();
  storage.setItem(UC.PROTECTED_STORAGE_KEY, JSON.stringify({ v: 1, on: true }));
  p = await page({ storage, hooks: refusingFirstFinish(422, HEIF_REFUSED) });
  await pickFile(p.doc, heic);
  await upload(p.doc);
  assert.equal(question(p.doc), null, "the protected setup: no question");
  assert.ok(text(p.doc.body).includes(HEIF_REFUSED), "the protected setup: the server's words");
  assert.equal(startsOf(p.relay).length, 1, "the protected setup: nothing more is sent");

  // Any other refusal at the finish: its own words, no question.
  p = await page({ hooks: refusingFirstFinish(400, "This server no longer takes files of this kind.") });
  await pickFile(p.doc, heic);
  await upload(p.doc);
  assert.equal(question(p.doc), null, "another refusal: no question");
  assert.ok(text(p.doc.body).includes("This server no longer takes files of this kind."), "another refusal: its words");
});

// ── Red first ────────────────────────────────────────────────────────────
// 2026-10-11: each break made in a copy of web/ and this file run against it with HOS_WEB_DIR,
// seen failing with the assertion named; every other test passed.
//  M1 files-app.js, startUpload sending `f.file` (as picked) instead of `sending.file`: test 1
//     "the cleaned photo went up, the strip's own bytes".
//  M2 files-upload.js, the start's body without keep_metadata: test 2 "keep_metadata: 1".
//  M3 files-app.js, sendingOf deciding as a private conversation ('private'): test 3 "a cut video:
//     the words say why and how" (it asked instead of refusing).
//  M4 upload-clean.js, the whole-file branch taken at any size (`if (true)`): test 5 "nothing near
//     its size read before the upload: largest read 73401162".
//  M5 files-app.js, f.protectedOn never set: test 4 "no switch".
