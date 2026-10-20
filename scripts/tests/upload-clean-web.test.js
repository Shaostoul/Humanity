// Cleaning a file on the sender's own device before it leaves, on the website (2026-10-11,
// docs/design/files-showcase.md, "Files on the sender's device and in messages, exact", parts 1
// and 2): web/shared/upload-clean.js, around web/shared/strip-metadata.js.
//
// Run: node --test scripts/tests/upload-clean-web.test.js
//
// What it proves:
//  1. The words on screen are the spec's (the switch, the describe sentences, the question, the
//     recipient's line), and the protected setup is read under protected.js's own key and rule.
//  2. The switch is on by default, remembered as the last choice, and storage that refuses
//     changes nothing.
//  3. A phone photo and a phone video are cleaned to exactly the bytes the strip makes of them
//     (the strip itself is held to the relay's Rust by strip-metadata.test.js), keep their name,
//     type and date, and the attach step's sentence is the describe's.
//  4. A video bigger than the whole-file limit is read by its box headers only, and comes out
//     byte for byte as the whole-file strip makes it, for every layout the strip's tests use and
//     for every cut of such a video (the could-not-be-checked answer the same each time).
//  5. A file that could not be checked (a picture the strip cannot walk, a video cut before its
//     movie, a HEIC photo, a file whose bytes are not what its name says) is said so, and sent as
//     it is only on a yes: never to a public shelf (unless its details are kept by choice), never
//     with the protected setup on, and in a private conversation only after "Send it anyway?",
//     with `unchecked` set for its marker.
//  6. A file with nothing hidden in it needs no attach step.
//
// HOS_WEB_DIR points the test at another copy of web/ (used to see each test red). Red first: see
// the end of this file.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const C = require(path.join(WEB, "shared", "upload-clean.js"));
const S = require(path.join(WEB, "shared", "strip-metadata.js"));
const P = require(path.join(WEB, "shared", "protected.js"));
const B = require("./file-bytes.js");

const SPEC = fs.readFileSync(path.join(ROOT, "docs", "design", "files-showcase.md"), "utf8");
// The section this builds, with its line wrapping undone.
const SECTION = SPEC.slice(SPEC.indexOf("## Files on the sender's device and in messages, exact")).replace(/\s+/g, " ");

const MIB = 1024 * 1024;
const same = (a, b) => Buffer.from(a).equals(Buffer.from(b));
const fileOf = (bytes, name, type) => new File([bytes], name, { type: type || "", lastModified: 1760000000000 });
const bytesOf = async (f) => new Uint8Array(await f.arrayBuffer());

function storage(init) {
  const m = new Map(Object.entries(init || {}));
  return { m, getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k) };
}
const refusing = { getItem() { throw new Error("denied"); }, setItem() { throw new Error("denied"); } };

// ── 1. The words ─────────────────────────────────────────────────────────

test("the words are the spec's, and the protected setup is read with protected.js's key and rule", () => {
  assert.ok(SECTION.includes(`"${C.WORDS.switchLabel}"`), "the switch's label");
  assert.ok(SECTION.includes(`"${C.WORDS.askPrivate}"`), "the question in a DM or a group");
  assert.ok(SECTION.includes(`"${C.WORDS.recipientLine}"`), "the recipient's line");
  assert.ok(SECTION.includes('"This photo says where it was taken and which camera took it."'), "the spec's photo sentence");
  assert.ok(SECTION.includes('"This video says where it was filmed."'), "and its video sentence");
  assert.equal(C.PROTECTED_STORAGE_KEY, P.PROTECTED_STORAGE_KEY, "the protected setup's own storage key");
  // The same answer as protected.js's protectedStateRead for every kind of kept value.
  const raws = [null, "", "{}", '{"on":false}', '{"on":true}', "not json", '{"on":false,"pin":1}', "[]", "0"];
  for (const raw of raws) {
    const st = storage(raw == null ? {} : { [P.PROTECTED_STORAGE_KEY]: raw });
    assert.equal(C.protectedSetupOn(st), !!P.protectedStateRead(st), `kept ${JSON.stringify(raw)}`);
  }
  assert.equal(C.protectedSetupOn(refusing), false, "storage that cannot be read counts as nothing kept");
});

// ── 2. The switch ────────────────────────────────────────────────────────

test("the switch is on by default, remembered as the last choice, and refused storage changes nothing", () => {
  const st = storage();
  assert.equal(C.removeDetailsChosen(st), true, "on by default");
  assert.equal(C.rememberRemoveDetails(st, false), true);
  assert.equal(st.m.get(C.REMOVE_DETAILS_KEY), "0", "kept as the last choice");
  assert.equal(C.removeDetailsChosen(st), false, "and read back");
  C.rememberRemoveDetails(st, true);
  assert.equal(C.removeDetailsChosen(st), true);
  assert.equal(C.removeDetailsChosen(refusing), true, "storage that refuses: on");
  assert.equal(C.rememberRemoveDetails(refusing, false), false, "and nothing thrown");
});

// ── 3. A phone photo and a phone video ───────────────────────────────────

test("a phone photo and a phone video are cleaned to the strip's own bytes, keeping their name, type and date", async () => {
  const photo = B.phoneJpeg();
  const pf = fileOf(photo, "IMG_0001.JPG", "image/jpeg");
  const c = await C.cleanFile(pf);
  const want = S.stripFile(photo, pf);
  assert.ok(want.changed, "test setup: the strip changes it");
  assert.equal(c.changed, true);
  assert.ok(same(await bytesOf(c.cleaned), want.bytes), "the cleaned photo is the strip's bytes");
  assert.ok(!B.contains(await bytesOf(c.cleaned), "SN-0042-SECRET"), "its camera's serial number gone");
  assert.deepEqual([c.cleaned.name, c.cleaned.type, c.cleaned.lastModified], [pf.name, pf.type, pf.lastModified], "same name, type and date");
  assert.equal(c.summary, "This photo says where it was taken and which camera took it.");
  assert.ok(c.details.some((d) => d.key === "location"), "the details name the location");
  assert.equal(c.unchecked, false);
  assert.ok(same(await bytesOf(c.original), photo), "the original untouched");

  const video = B.movie("moovFirst", B.ftypMp4(), { moov: [B.androidUdta()] });
  const vf = fileOf(video, "VID_0002.mp4", "video/mp4");
  const v = await C.cleanFile(vf);
  assert.equal(v.summary, "This video says where it was filmed.", "the spec's video sentence");
  assert.ok(same(await bytesOf(v.cleaned), S.stripFile(video, vf).bytes), "the strip's bytes");
  assert.equal((await bytesOf(v.cleaned)).length, video.length, "a video keeps its length");
  assert.ok(!B.contains(await bytesOf(v.cleaned), B.PLACE), "where it was filmed is gone");
});

// ── 4. A big video, by its box headers ───────────────────────────────────

const LAYOUTS = [
  ["moov first", () => B.movie("moovFirst", B.ftypMp4(), B.everything())],
  ["mdat first", () => B.movie("mdatFirst", B.ftypQt(), B.everything())],
  ["a 64-bit mdat", () => B.movie("mdatFirst", B.ftypMp4(), Object.assign(B.everything(), { mdat64: true }))],
  ["an mdat to the end", () => B.movie("moovFirst", B.ftypMp4(), Object.assign(B.everything(), { mdatSize0: true }))],
  ["a moov to the end", () => B.movie("mdatFirst", B.ftypMp4(), Object.assign(B.everything(), { moovSize0: true }))],
  ["a phone video", () => B.phoneVideo("mdatFirst")],
  ["nothing in it", () => B.movie("moovFirst", B.ftypMp4())],
];

test("a video over the whole-file limit is read by its box headers and comes out as the whole-file strip makes it", async () => {
  for (const [name, make] of LAYOUTS) {
    const video = make();
    const f = fileOf(video, "clip.mp4", "video/mp4");
    const whole = await C.cleanFile(f);
    const sparse = await C.cleanFile(f, { wholeMax: 0 });
    const want = S.stripFile(video, f);
    assert.ok(same(await bytesOf(whole.cleaned), want.bytes), `${name}: whole, the strip's bytes`);
    assert.ok(same(await bytesOf(sparse.cleaned), want.bytes), `${name}: by its headers, the same bytes`);
    assert.equal(sparse.changed, want.changed, `${name}: changed alike`);
    assert.equal(sparse.unchecked, whole.unchecked, `${name}: checked alike`);
    assert.equal(sparse.summary, whole.summary, `${name}: the same sentence`);
  }
  // What it read: the movie index and the boxes that can hold a location, never the frames.
  const big = B.movie("mdatFirst", B.ftypMp4(), Object.assign(B.everything(), { frames: 200000 }));
  const sin = await C.standIn(fileOf(big, "big.mp4"));
  assert.ok(sin.bytes.length < big.length / 50, "the stand-in is a small part of the file");
  assert.ok(!B.contains(sin.bytes, "DECOY-inside-the-frames"), "and holds none of the frames");
});

test("every cut of a video comes out the same by its headers as whole, and is said to be unchecked alike", async () => {
  // The movie index last (a cut inside it means no movie found) and first (a cut in the frames
  // after it is a box whose size makes no sense, after the index was read).
  for (const layout of ["mdatFirst", "moovFirst"]) {
    const video = B.movie(layout, B.ftypMp4(), B.everything());
    let unchecked = 0;
    for (let cut = 1; cut < video.length; cut++) {
      const part = video.subarray(0, cut);
      const f = fileOf(part, "cut.mp4", "video/mp4");
      const whole = await C.cleanFile(f);
      const sparse = await C.cleanFile(f, { wholeMax: 0 });
      if (!same(await bytesOf(whole.cleaned), await bytesOf(sparse.cleaned))) assert.fail(`${layout}, cut at ${cut}: different bytes`);
      if (whole.unchecked !== sparse.unchecked) assert.fail(`${layout}, cut at ${cut}: checked differently`);
      if (whole.unchecked) unchecked++;
    }
    assert.ok(unchecked > video.length / 2, `${layout}: most cuts could not be checked`);
  }
});

// ── 5. A file that could not be checked ──────────────────────────────────

test("a file that could not be checked: said so, never to a public shelf, never under the protected setup, asked in private", async () => {
  const cases = [
    ["a picture the strip cannot walk", fileOf(B.brokenJpeg(), "photo.jpg", "image/jpeg"), false],
    ["a video cut before its movie", fileOf(B.movie("mdatFirst", B.ftypMp4(), B.everything()).subarray(0, 300), "clip.mp4", "video/mp4"), false],
    ["a HEIC photo", fileOf(B.heic(), "IMG_0002.HEIC", "image/heic"), true],
    ["a file whose bytes are not what its name says", fileOf(Buffer.from("plain words, not a picture"), "photo.jpg", "image/jpeg"), false],
  ];
  for (const [name, f, heif] of cases) {
    const c = await C.cleanFile(f);
    assert.equal(c.unchecked, true, `${name}: could not be checked`);
    assert.equal(c.heif, heif, `${name}: HEIC or not`);
    assert.ok(typeof c.reason === "string" && c.reason.length > 20, `${name}: says why`);
    assert.ok(same(await bytesOf(c.cleaned), await bytesOf(f)) || !heif, `${name}: a HEIC is sent as it is`);

    const pub = C.decide(c, { where: "public", remove: true });
    if (heif) {
      // A HEIC to a public shelf goes to the server as picked: the relay cleans HEIF stills itself
      // since 2026-10-11 and refuses one it cannot read through (seen red with the HEIC rule taken
      // out of decide: "a HEIC goes to the server, which cleans it" got "refuse").
      assert.deepEqual([pub.action, pub.keepMetadata, pub.file], ["send", false, f], `${name}: a HEIC goes to the server, which cleans it`);
      assert.equal(C.stepNeeded(c, { where: "public", remove: true }), false, "with no step in the way");
    } else {
      assert.equal(pub.action, "refuse", `${name}: never to a public shelf`);
      assert.equal(pub.canKeep, true, "unless its details are kept by choice");
      assert.ok(pub.words.includes(C.WORDS.publicKeepHint), "and the words say how");
    }
    const kept = C.decide(c, { where: "public", remove: false });
    assert.deepEqual([kept.action, kept.keepMetadata, kept.file], ["send", true, f], `${name}: kept by choice, it goes as picked with keep_metadata`);

    const priv = C.decide(c, { where: "private", remove: true });
    assert.equal(priv.action, "ask", `${name}: in private, asked`);
    assert.equal(priv.words, heif ? C.WORDS.askPrivateHeif : C.WORDS.askPrivate);
    assert.equal(priv.unchecked, true, "its marker will say so");

    for (const where of ["public", "private"]) {
      const prot = C.decide(c, { where, remove: false, protectedOn: true });
      if (heif && where === "public") {
        assert.equal(prot.action, "send", `${name}: a public HEIC goes to the server, which cleans it, even under the protected setup`);
        assert.equal(prot.keepMetadata, false, "never keep_metadata under the protected setup");
        continue;
      }
      assert.equal(prot.action, "refuse", `${name}: the protected setup sends only files it could check (${where})`);
      assert.equal(prot.canKeep, false, "with no way round it");
      assert.ok(C.stepNeeded(c, { where, remove: false, protectedOn: true }), "and the step says so");
    }
  }
});

test("the protected setup always removes the details, whatever the switch says", async () => {
  const f = fileOf(B.phoneJpeg(), "IMG.jpg", "image/jpeg");
  const c = await C.cleanFile(f);
  const d = C.decide(c, { where: "public", remove: false, protectedOn: true });
  assert.equal(d.action, "send");
  assert.equal(d.keepMetadata, false, "never keep_metadata");
  assert.equal(d.file, c.cleaned, "the cleaned file");
  assert.equal(C.stepNeeded(c, { where: "public", remove: false, protectedOn: true }), false, "and there is nothing to decide");
});

// ── 6. Nothing hidden ────────────────────────────────────────────────────

test("a file with nothing hidden needs no attach step; one with hidden details or a recipient's line does", async () => {
  const pdf = await C.cleanFile(fileOf(Buffer.from("%PDF-1.7 minutes"), "notes.pdf", "application/pdf"));
  assert.deepEqual([pdf.unchecked, pdf.summary, pdf.changed], [false, null, false], "a document: nothing to say");
  assert.equal(C.stepNeeded(pdf, { where: "public", remove: true }), false);
  const png = await C.cleanFile(fileOf(B.png(), "shot.png", "image/png"));
  assert.equal(C.stepNeeded(png, { where: "private", remove: true }), false, "a clean screenshot goes at once");
  assert.equal(C.stepNeeded(png, { where: "private", remove: true }, "Ann accepts files only from friends."), true, "unless there is a line about the recipient");
  const photo = await C.cleanFile(fileOf(B.phoneJpeg(), "IMG.jpg", "image/jpeg"));
  assert.equal(C.stepNeeded(photo, { where: "public", remove: true }), true, "a phone photo: the step, with the switch");
  assert.equal(C.decide(photo, { where: "public", remove: true }).keepMetadata, false);
});

// ── Red first ────────────────────────────────────────────────────────────
// 2026-10-11: each break made in a copy of web/ and this file run against it with HOS_WEB_DIR,
// seen failing with the assertion named; every other test passed.
//  R1 upload-clean.js, standIn copying `mdat` like the boxes it looks into (the COPIED set with
//     'mdat' added): test 4 "the stand-in is a small part of the file".
//  R2 upload-clean.js, standIn's bad header no longer pushed (the stand-in ends before it): test 4
//     (cuts) "moovFirst, cut at 1165: checked differently" (the walk read through where the whole
//     file's stopped; with the movie index last no cut could show it, so the cuts run both ways).
//  R3 upload-clean.js, decide() sending a could-not-be-checked file to a public shelf: test 5
//     "a picture the strip cannot walk: never to a public shelf".
//  R4 upload-clean.js, the HEIC rule taken out (`out.kind === 'heif' || claimed === 'heif'`
//     false): test 5 "a HEIC photo: HEIC or not".
//  R5 upload-clean.js, protectedSetupOn reading 'humanity_protected' (another key): test 1 "kept
//     \"{}\"".
//  R6 upload-clean.js, removeDetailsChosen reading '1' as keep: test 2 "on by default" passed and
//     "and read back" failed.
