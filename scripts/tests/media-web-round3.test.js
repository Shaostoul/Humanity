// The Media page on the website, round 3 (2026-10-11): docs/design/media-showcase.md "Round 3: fixes
// from the round 2 review", its "Both apps" and "Web only" items, against the relay shapes R3-6 to
// R3-9 (`resets_in_s` and `left_bytes` on a 429, `uploads_in_progress` on /api/media/mine,
// POST /api/media/ticket answering {url, expires_in_s}, X-Upload-Token on the resumable routes).
//
// Run: node --test scripts/tests/media-web-round3.test.js   (in `just rig-tests` with the others)
//
// Against the stand-ins in scripts/tests/media-web-harness.js (this file's own moved there in
// round 4, the stand-in relay as `resumableRelay`); the server here is a stand-in for
// src/relay/resumable.rs that keeps several uploads at once and logs every request it gets, so a
// test can say exactly which requests were made after a press. Every wait the upload makes goes
// through a stand-in sleep that records how long it was asked for and ends only when the test
// wakes it, so a test sees each wait and nothing spins. HOS_WEB_DIR points the test at another copy
// of web/ (used to see each test red).
//
// What it proves, by the review's findings:
//  1. Any 5xx, and a 429 without the allowance's JSON (nginx's own refusal), is waited out with a
//     growing wait and tried again, at the start and on a piece, never the end of an upload; the
//     same server error ten times running stops with its words and Retry, which carries on. The
//     409 busy is asked again after a growing wait, never once a second.
//  2. Opening another form while an upload runs leaves it running: uploads are a list, each with
//     its own bar, Pause and Cancel, each published by its own form with what was typed in it.
//     Cancel while the server is storing the file leaves nothing: since round 4 the server deletes
//     the stored file with the cancel and the page removes nothing by hand (2b, changed in round 4).
//  3. The record of an upload to resume is dated by its last progress (the relay expires a session
//     a day after its last piece) and forgotten when it fails; Close on a failed upload cancels it
//     on the server; the server's `uploads_in_progress` are listed with Cancel.
//  4. A file that can no longer be read ends the upload with words saying so, instead of looking
//     like a dropped connection for ever.
//  5. Members-only pictures are fetched with an AbortController: stopped when no longer shown, so
//     their slot frees; kept (not revoked, not fetched again) across a list loaded afresh. A
//     members-only video gets a new ticket where it stopped once its old one runs out.
//  6. /media/<n> (it was /media?id=<n> until round 5) keeps the label cover (click to load):
//     nothing is fetched before the click.
//  8. Resume matches the file's fingerprint (SHA-256 of its first and last megabyte), not only its
//     name, size and date.
//  B. Both apps: Pause stops every request in every phase (the piece on its way, a server error's
//     wait, the busy wait, an allowance's wait); an allowance waits the server's `resets_in_s` (at
//     least 5 seconds) and a piece is cut to `left_bytes`; the resumable routes carry the token in
//     the X-Upload-Token header and never in an address.
//
// Seen red 2026-10-11: each with one break made in a copy of web/ under HOS_WEB_DIR, then green
// again on the real web/: see the note above each test.

const test = require("node:test");
const assert = require("node:assert");

const H = require("./media-web-harness.js");
const {
  media, upload, ME, MIB, byId, text, respond, path0, at, fakeStorage, settle, item,
  listAnswer, mineAnswer, listRoutes, openPage, SHA256, fingerprintOf,
} = H;

// The resumable upload's stand-ins (aFile, sleeper, drive, the stand-in relay and one uploader on
// it) moved to media-web-harness.js in round 4, so round 4's tests run against the same ones. The
// stand-in relay answers a cancel as round 4's does now (see the harness).
const { T0, aFile, sleeper, drive, NGINX_503, NGINX_429 } = H;
const relay3 = H.resumableRelay;
const uploader = H.uploaderOn;

// The page's upload list.
const uploadsBox = (doc) => byId(doc, "media-uploads");
const entries = (doc) => uploadsBox(doc).querySelectorAll("[data-upload]");
const entryPart = (entry, part) => entry.querySelector("[data-upload-" + part + "]");
const entryButton = (entry, action) => entry.querySelector('[data-upload-action="' + action + '"]');
const notice = (doc) => text(byId(doc, "media-notice").querySelector(".media-notice-text"));

async function pickFile(doc, file) {
  const input = byId(doc, "media-file-input");
  input.files = [file];
  input.dispatch("change");
  await settle();
}

// ── 1. Server errors are waited out ──────────────────────────────────────

// Seen red 2026-10-11 with the 5xx branch in loop() put back to `return fail(b.words)` (the round 2
// behaviour, review 1): "'failed' == 'done'" for "a 503 on a piece is waited out, never the end".
test("1a. a 503 (nginx's) on a piece and at the start is waited out with a growing wait, then the upload carries on", async () => {
  let piece503 = 3;
  let start503 = 1;
  const relay = relay3({
    start: async () => (start503-- > 0 ? respond(503, NGINX_503) : null),
    piece: async (_c, _s, off) => (off === 8 * MIB && piece503-- > 0 ? respond(503, NGINX_503) : null),
  });
  const up = uploader(relay, aFile("clip.mp4", 20 * MIB));
  up.u.start();
  await settle();
  assert.equal(up.u.state.phase, "server_wait", "the start's 503 is a wait, not a failure");
  assert.equal(up.u.state.error, "The server answered 503.", "in plain words, never nginx's page");
  await drive(up.sl, () => up.u.state.phase === "done");
  assert.equal(up.u.state.phase, "done", "a 503 on a piece is waited out, never the end");
  assert.ok(!up.phases.includes("failed"), "never failed on the way");
  assert.deepEqual(up.sl.waits, [2000, 2000, 4000, 8000], "each wait longer than the last (the start's, then three on the piece)");
  assert.deepEqual(relay.pieces.filter((p) => p.offset === 8 * MIB).length, 4, "the piece is sent again after each wait");
});

// Seen red 2026-10-11 with the 429 branch requiring nothing but the status (`res.status === 429`
// taken as the allowance, without its JSON): "'allowance' == 'server_wait'".
test("1b. a 429 without the allowance's JSON (a proxy's rate limit) is waited out too, not taken for the allowance", async () => {
  let refuse = 2;
  const relay = relay3({ piece: async (_c, _s, off) => (off === 8 * MIB && refuse-- > 0 ? respond(429, NGINX_429) : null) });
  const up = uploader(relay, aFile("clip.mp4", 20 * MIB));
  up.u.start();
  await settle();
  assert.equal(up.u.state.phase, "server_wait");
  assert.equal(up.u.state.error, "The server answered 429.");
  await drive(up.sl, () => up.u.state.phase === "done");
  assert.equal(up.u.state.phase, "done");
  assert.deepEqual(up.sl.waits, [2000, 4000]);
});

// Seen red 2026-10-11 with SAME_ERROR_TRIES raised to 1000 (no stop after ten): "ten times
// running / 1000 !== 10".
test("1c. the same server error ten times running stops with its words and Retry; Retry carries on", async () => {
  let broken = true;
  const relay = relay3({ piece: async (_c, _s, off) => (broken && off === 8 * MIB ? respond(502, "The relay is restarting.") : null) });
  const { doc, net } = await openPage({ sleep: undefined, routes: listRoutes(listAnswer([]), [
    ...relay.routes,
    [at("/api/media", "POST"), () => respond(200, { ok: true, item: item() })],
  ]) });
  // The page's own sleep (the harness's resolves at once); ten 502s in a row stop it.
  await pickFile(doc, aFile("clip.mp4", 20 * MIB));
  byId(doc, "media-form-go").click();
  await settle();
  const entry = entries(doc)[0];
  assert.equal(relay.pieces.filter((p) => p.offset === 8 * MIB).length, 10, "ten times running");
  assert.ok(text(entryPart(entry, "words")).startsWith("The relay is restarting."), "ten times running stops with the server's words");
  assert.ok(text(entryPart(entry, "words")).endsWith(media.MEDIA_WORDS.uploadServerErrorTail.trim()));
  assert.ok(entryButton(entry, "retry"), "and Retry");
  assert.ok(entryButton(entry, "cancel"), "and Cancel");
  broken = false;
  entryButton(entry, "retry").click();
  await settle();
  assert.equal(notice(doc), media.MEDIA_WORDS.published, "Retry carries on to the end and it is published");
  assert.ok(net.calls.find((c) => c.url === "/api/media" && c.init.method === "POST"));
});

// Seen red 2026-10-11 with the busy branch waiting a fixed `const wait = 1000` (round 2's poll):
// "the busy answer is asked again after a growing wait".
test("1d. a 409 busy is asked again after a growing wait, never once a second", async () => {
  let busy = 3;
  const relay = relay3({
    piece: async (_c, _s, off) => (off === 8 * MIB && busy-- > 0
      ? respond(409, { error: "An earlier piece of this upload is still being written. Ask again in a moment.", busy: true })
      : null),
  });
  const up = uploader(relay, aFile("clip.mp4", 20 * MIB));
  up.u.start();
  await drive(up.sl, () => up.u.state.phase === "done");
  assert.equal(up.u.state.phase, "done");
  assert.deepEqual(up.sl.waits, [2000, 4000, 8000], "the busy answer is asked again after a growing wait");
  const gets = relay.log.filter((e) => e.kind === "GET").length;
  assert.equal(gets, 3, "asking where the server got after each wait");
});

// ── 2. Uploads are a list; each publishes itself ─────────────────────────

// Seen red 2026-10-11 with startUpload put back as round 2 had it (the form left where it was as
// st.form, and onDone publishing only `if (st.form === f)`, so opening another form orphaned the
// upload): "the upload is in the list / 0 !== 1".
test("2a. another form opened while an upload runs leaves it running; each upload has its own bar and publishes with its own form", async () => {
  const hold = {};
  const relay = relay3({ piece: async (_c, sess, off) => {
    if (off === 8 * MIB && sess.stored === "1760_first.mp4") await new Promise((r) => { hold.first = r; });
    if (off === 0 && sess.stored === "1760_second.mp4") await new Promise((r) => { hold.second = r; });
    return null;
  } });
  const published = [];
  const { doc, page } = await openPage({ routes: listRoutes(listAnswer([]), [
    ...relay.routes,
    [at("/api/media", "POST"), (c) => { published.push(c.body); return respond(200, { ok: true, item: item() }); }],
  ]) });
  await pickFile(doc, aFile("first.mp4", 20 * MIB));
  byId(doc, "media-form-title").value = "The first one";
  byId(doc, "media-form-title").dispatch("input");
  byId(doc, "media-form-go").click();
  await settle();
  assert.equal(entries(doc).length, 1, "the upload is in the list");
  assert.equal(byId(doc, "media-form").childNodes.length, 0, "and the form area is free again");
  // Another file picked while the first is on its way: a new form, the first untouched.
  await pickFile(doc, aFile("second.mp4", 4 * MIB, { fill: 2 }));
  assert.equal(text(byId(doc, "media-form").querySelector("h2")), "Add to Files");
  byId(doc, "media-form-title").value = "The second one";
  byId(doc, "media-form-title").dispatch("input");
  byId(doc, "media-form-go").click();
  await settle();
  assert.equal(entries(doc).length, 2, "two uploads, each in the list");
  const [first, second] = entries(doc);
  assert.ok(entryPart(first, "bar") && entryPart(second, "bar"), "each with its own bar");
  assert.ok(entryButton(first, "pause") && entryButton(first, "cancel"), "the first still has its Pause and Cancel");
  hold.second();
  await settle();
  assert.deepEqual(published.map((p) => p.title), ["The second one"], "the second is published by its own form");
  // Put on Media opened meanwhile changes nothing either.
  page.openPutForm({ filename: "1700_part.stl", name: "bushing.stl" });
  assert.equal(text(byId(doc, "media-form").querySelector("h2")), "Put on the Files page");
  assert.equal(entries(doc).length, 1, "the first upload is still in the list");
  hold.first();
  await settle();
  assert.deepEqual(published.map((p) => [p.title, p.filename]), [["The second one", "1760_second.mp4"], ["The first one", "1760_first.mp4"]], "the first upload is published, with its own title");
  assert.equal(entries(doc).length, 0, "and both leave the list once published");
});

// Seen red 2026-10-11 with onStored left out of the uploader options (removeStored never called):
// "the stored file is removed too".
// Changed in round 4 (W4-3, the cancel answer): the server deletes the stored file with the cancel
// and says so (`removed_stored`), so the page removes nothing by hand (no POST /api/media/remove)
// and asks no GET first. Seen red 2026-10-11 against round 3's web/ (git show 3077155ed:web/):
// "nothing is removed by hand" (round 3 still sent its own /api/media/remove).
test("2b. Cancel while the server is storing the file deletes the upload, and the server deletes the stored file with it", async () => {
  let finishing;
  const sl = sleeper();
  const relay = relay3({ finish: async () => { await new Promise((r) => { finishing = r; }); return null; } });
  const { doc, net } = await openPage({ sleep: sl.sleep, routes: listRoutes(listAnswer([]), [
    ...relay.routes,
    [at("/api/media/remove", "POST"), (c) => respond(200, { ok: true, removed_upload: c.body.filename, file_removed: true, file_kept: false })],
  ]) });
  await pickFile(doc, aFile("clip.mp4", 10 * MIB));
  byId(doc, "media-form-go").click();
  await settle();
  assert.equal(typeof finishing, "function", "the last piece is with the server, which is storing it");
  entryButton(entries(doc)[0], "cancel").click();
  await settle();
  // The cancel is answered busy while the store runs, and asked again after a wait.
  const busyDeletes = relay.log.filter((e) => e.kind === "DELETE").length;
  assert.equal(busyDeletes, 1, "the cancel is asked once while the server is busy");
  assert.equal(sl.waits.at(-1), 2000, "then waits");
  // The store finishes meanwhile; the next cancel finds the file stored and deletes it too.
  finishing();
  await drive(sl, () => entries(doc).length === 0, 10);
  const del = relay.log.filter((e) => e.kind === "DELETE");
  assert.equal(del.length, 2, "the upload is deleted on the server");
  assert.ok(relay.sessions.get(del[0].id).storedRemoved, "and the stored file with it, by the server");
  assert.equal(net.calls.filter((c) => c.url === "/api/media/remove").length, 0, "nothing is removed by hand");
  assert.equal(relay.log.filter((e) => e.kind === "GET").length, 0, "nor asked about first");
  assert.equal(net.calls.filter((c) => c.url === "/api/media" && c.init.method === "POST").length, 0, "and it is not published");
  assert.equal(notice(doc), media.MEDIA_WORDS.cancelledStored);
  assert.equal(entries(doc).length, 0);
});

// ── 3. Abandoned sessions ─────────────────────────────────────────────────

// Seen red 2026-10-11 with touchUpload's call taken out of loop(): "dated by its last piece".
test("3a. the record of an upload to resume is dated by its last progress, and forgotten when it fails", async () => {
  let t = T0;
  let refuse = false;
  const relay = relay3({ piece: async (_c, _s, off) => { t += 3600 * 1000; return refuse && off === 16 * MIB ? respond(403, "Your role on this server cannot upload files.") : null; } });
  const storage = fakeStorage();
  const file = aFile("clip.mp4", 24 * MIB);
  const up = uploader(relay, file, { storage, now: () => t });
  refuse = true;
  up.u.start();
  await settle();
  // Started at T0, two pieces taken an hour apart; the third refused.
  assert.equal(up.u.state.phase, "failed");
  assert.equal(storage.getItem(upload.RESUME_KEY), "[]", "forgotten when it fails");
  // The same again, its third piece still on its way: two pieces taken an hour apart.
  t = T0;
  const relay2 = relay3({ piece: async (_c, _s, off) => { t += 3600 * 1000; if (off === 16 * MIB) await new Promise(() => {}); return null; } });
  const up2 = uploader(relay2, file, { storage, now: () => t });
  up2.u.start();
  await settle();
  const rec = JSON.parse(storage.getItem(upload.RESUME_KEY))[0];
  assert.equal(rec.at, T0 + 2 * 3600 * 1000, "dated by its last piece");
  // 23 hours after the last piece (well over a day after the start): still found.
  const fp = fingerprintOf(new Uint8Array(24 * MIB));
  storage.setItem(upload.RESUME_KEY, JSON.stringify([Object.assign({}, rec, { at: T0 + 2 * 3600 * 1000 })]));
  assert.ok(upload.findUpload(storage, ME, file, fp, T0 + 25 * 3600 * 1000), "a day after the start, but not after the last piece: still carried on");
  assert.equal(upload.findUpload(storage, ME, file, fp, T0 + 27 * 3600 * 1000), null, "a day after the last piece: the relay has removed it");
});

// Seen red 2026-10-11 with Close calling only dropUpload (no cancel, round 2's closeForm on a
// failed upload): "Close on a failed upload cancels it on the server".
test("3b. Close on a failed upload cancels it on the server", async () => {
  const relay = relay3({ piece: async (_c, _s, off) => (off === 8 * MIB ? respond(413, "This piece is larger than the server takes.") : null) });
  const { doc } = await openPage({ routes: listRoutes(listAnswer([]), relay.routes) });
  await pickFile(doc, aFile("clip.mp4", 20 * MIB));
  byId(doc, "media-form-go").click();
  await settle();
  const entry = entries(doc)[0];
  assert.equal(text(entryPart(entry, "words")), "This piece is larger than the server takes.");
  assert.equal(entryButton(entry, "cancel"), null, "a failed upload has Close only");
  entryButton(entry, "close").click();
  await settle();
  assert.equal(relay.log.filter((e) => e.kind === "DELETE").length, 1, "Close on a failed upload cancels it on the server");
  assert.equal(entries(doc).length, 0);
});

// Seen red 2026-10-11 with renderWaiting drawing only the waiting list (`if (false)` for the
// unfinished one): "the unfinished uploads the server holds are listed"; and with its filter
// leaving out `!tracked.has(p.id)`: "the page's own upload is shown once, in its list".
test("3c. the server's uploads in progress are listed with Cancel; one this page carries is not listed twice", async () => {
  let mine = mineAnswer({ uploads_in_progress: [
    { id: "f".repeat(32), filename: "Old clip.mp4", size: 900 * MIB, offset: 312 * MIB, touched_at: T0 - 3600000 },
  ] });
  const storage = fakeStorage();
  storage.setItem(upload.RESUME_KEY, JSON.stringify([{ owner: ME, id: "f".repeat(32), name: "Old clip.mp4", size: 900 * MIB, lastModified: 1, fp: "00", at: T0 }]));
  const deletes = [];
  const { doc, net, confirms } = await openPage({ storage, routes: listRoutes(listAnswer([]), [
    [at("/api/media/mine", "POST"), () => respond(200, mine)],
    [(c) => path0(c.url) === "/api/upload/resumable/" + "f".repeat(32) && c.init.method === "DELETE", (c) => { deletes.push(c); mine = mineAnswer(); return respond(200, { ok: true }); }],
  ]) });
  const list = byId(doc, "media-unfinished-list");
  assert.ok(list, "the unfinished uploads the server holds are listed");
  assert.equal(text(list.querySelector("h2")), media.MEDIA_WORDS.unfinishedTitle);
  const row = list.querySelector("[data-unfinished]");
  assert.equal(text(row.querySelector(".media-admin-row-text")), "Old clip.mp4 · 312 MB of 900 MB sent");
  row.querySelector("[data-unfinished-cancel]").click();
  await settle();
  assert.equal(confirms.at(-1), '"Old clip.mp4"\n\n' + media.MEDIA_WORDS.unfinishedCancel);
  assert.equal(deletes.length, 1, "Cancel deletes it on the server");
  assert.equal(deletes[0].headers["X-Upload-Token"], "tok-1", "with the token in the header");
  assert.ok(!/token/i.test(deletes[0].url), "never in the address");
  assert.equal(storage.getItem(upload.RESUME_KEY), "[]", "and this browser forgets it");
  assert.equal(notice(doc), media.MEDIA_WORDS.unfinishedCancelled);
  assert.equal(byId(doc, "media-unfinished-list"), null, "the list follows the server's answer");
  assert.ok(net.calls.filter((c) => c.url === "/api/media/mine").length >= 2, "asked again after the cancel");

  // An upload this page is carrying is not listed among the unfinished ones as well.
  let release;
  const relay = relay3({ piece: async (_c, _s, off) => { if (off === 8 * MIB) await new Promise((r) => { release = r; }); return null; } });
  const page2 = await openPage({ routes: listRoutes(listAnswer([]), [
    ...relay.routes,
    [at("/api/media/mine", "POST"), () => respond(200, mineAnswer({ uploads_in_progress: [{ id: "a".repeat(32), filename: "clip.mp4", size: 20 * MIB, offset: 8 * MIB, touched_at: T0 }] }))],
  ]) });
  await pickFile(page2.doc, aFile("clip.mp4", 20 * MIB));
  byId(page2.doc, "media-form-go").click();
  await settle();
  await page2.page.loadMine();
  assert.equal(byId(page2.doc, "media-unfinished-list"), null, "the page's own upload is shown once, in its list");
  release();
});

// ── 4. A file that can no longer be read ─────────────────────────────────

// Seen red 2026-10-11 with the slice's read failure taken for a dropped connection (readSlice's
// catch going to dropWait): "'reconnecting' == 'failed'" for "ends with words saying so".
test("4. a file that can no longer be read ends the upload with words saying so, instead of retrying for ever", async () => {
  const file = aFile("clip.mp4", 20 * MIB);
  const relay = relay3({ piece: async (_c, _s, off) => { if (off === 0) file.gone = true; return null; } });
  const up = uploader(relay, file);
  up.u.start();
  await drive(up.sl, () => up.u.state.phase === "failed", 12);
  assert.equal(up.u.state.phase, "failed", "ends with words saying so");
  assert.ok(up.u.state.unreadable);
  assert.equal(up.u.state.error, '"clip.mp4" can no longer be read on this device (it was moved, changed, or the drive it is on was unplugged), so the upload stopped.');
  assert.ok(!up.phases.includes("reconnecting"), "never taken for a dropped connection");
  assert.equal(relay.pieces.length, 1, "nothing sent after the read failed");
  // On the page: the words, and Close (which ends it on the server).
  const file2 = aFile("clip.mp4", 20 * MIB, { fill: 7 });
  const relay2 = relay3({ piece: async (_c, _s, off) => { if (off === 0) file2.gone = true; return null; } });
  const { doc } = await openPage({ routes: listRoutes(listAnswer([]), relay2.routes) });
  await pickFile(doc, file2);
  byId(doc, "media-form-go").click();
  await settle();
  const entry = entries(doc)[0];
  assert.match(text(entryPart(entry, "words")), /can no longer be read on this device/);
  entryButton(entry, "close").click();
  await settle();
  assert.equal(relay2.log.filter((e) => e.kind === "DELETE").length, 1);
});

// ── 5. Members-only pictures and videos ──────────────────────────────────

// Seen red 2026-10-11 with dropBlob not aborting (the `e.ctl.abort()` line taken out): "the fetch
// of a picture no longer shown is stopped"; and with sweepBlobs ignoring st.reloadKeep: "a list
// loaded again keeps the picture (not revoked)".
test("5a. a members-only picture's fetch is stopped once it is no longer shown, and kept across a list loaded again", async () => {
  const pics = [1, 2, 3, 4, 5].map((id) => item({ id, url: "/api/media/file/" + id, visibility: "members", title: "P" + id, filename: id + "_p.png" }));
  let showing = pics.slice(0, 4);
  let holdList = null;
  const hung = new Set([1, 2, 3, 4]);
  const { doc, net, urls, page } = await openPage({ routes: listRoutes(null, [
    [(c) => c.url === "/api/media/list" && c.body.featured !== 1, async () => { if (holdList) await holdList; return respond(200, listAnswer(showing)); }],
    [(c) => c.url === "/api/media/list" && c.body.featured === 1, () => respond(200, listAnswer([]))],
    [(c) => path0(c.url).startsWith("/api/media/file/"), async (c) => {
      const id = Number(path0(c.url).split("/").pop());
      if (hung.has(id)) await new Promise(() => {}); // a slow file that never ends by itself
      return respond(200, "", new Uint8Array([id]));
    }],
  ]) });
  const fileCalls = () => net.calls.filter((c) => path0(c.url).startsWith("/api/media/file/"));
  assert.equal(fileCalls().length, 4, "four at once");
  // The filter changes: the four leave the page, a fifth comes. Their fetches are stopped and the
  // fifth gets a slot.
  showing = [pics[4]];
  await page.load(true);
  await settle();
  assert.ok(fileCalls().slice(0, 4).every((c) => c.aborted), "the fetch of a picture no longer shown is stopped");
  assert.ok(fileCalls().some((c) => path0(c.url) === "/api/media/file/5"), "and the fifth gets its slot");
  const five = byId(doc, "media-grid").querySelector(".media-tile").querySelector("img");
  assert.ok(five && five.src.startsWith("blob:"), "the fifth shows");
  // The list loads again, its answer slower than the featured strip's (which sweeps meanwhile).
  let open;
  holdList = new Promise((r) => { open = r; });
  const before = fileCalls().length;
  const reload = page.load(true);
  await page.loadFeatured();
  await settle();
  open();
  await reload;
  await settle();
  assert.deepEqual(urls.revoked, [], "a list loaded again keeps the picture (not revoked)");
  assert.equal(fileCalls().length, before, "and does not fetch it again");
});

// Seen red 2026-10-11 with the video's error listener left out of renderViewer: "a new ticket
// for the same video"; and with Download's state back in the viewer's key and its press drawing
// the viewer again (round 2's): "and the playing video is not drawn again".
test("5b. a members-only video plays through a ticket, and gets a new one where it stopped when the old one runs out", async () => {
  let t = T0;
  let n = 0;
  const clip = item({ id: 4, url: "/api/media/file/4", visibility: "members", kind: "video", title: "Fire show", filename: "4_fire.mp4" });
  const { doc, net, saved } = await openPage({ now: () => t, routes: listRoutes(listAnswer([clip]), [
    // Round 5 (lane 2A): a Download's ticket is for the counting route, /api/media/download/<id>.
    [at("/api/media/ticket", "POST"), (c) => respond(200, { url: (c.body.download ? "/api/media/download/4?file=0&" : "/api/media/file/4?") + "ticket=t" + (++n), expires_in_s: 600 })],
  ]) });
  byId(doc, "media-grid").querySelector(".media-thumb").click();
  await settle();
  const video = byId(doc, "media-viewer-open").querySelector("video");
  assert.equal(video.src, "/api/media/file/4?ticket=t1");
  assert.equal(net.calls.filter((c) => path0(c.url) === "/api/media/file/4").length, 0, "the page never fetches the video itself");
  // Twelve minutes in, the browser's next range request is refused: the video errors.
  t += 12 * 60 * 1000;
  video.currentTime = 431.5;
  video.paused = false;
  let played = 0;
  video.play = () => { played++; return Promise.resolve(); };
  video.dispatch("error");
  await settle();
  assert.equal(video.src, "/api/media/file/4?ticket=t2", "a new ticket for the same video");
  assert.equal(video.currentTime, 431.5, "where it stopped");
  assert.equal(played, 1, "and it plays on");
  assert.equal(byId(doc, "media-viewer-open").querySelector("video"), video, "the same video element");
  // A ticket that fails within its first minute is the file's own problem: no loop of tickets.
  video.dispatch("error");
  await settle();
  assert.equal(n, 2, "no new ticket for a fresh one");
  // Download beside it: a ticket of its own, saved by the browser, and the video is not drawn again.
  byId(doc, "media-download").click();
  await settle();
  assert.deepEqual(saved, [{ url: "/api/media/download/4?file=0&ticket=t3", name: "4_fire.mp4" }], "Download saves through a ticket of its own");
  assert.equal(byId(doc, "media-viewer-open").querySelector("video"), video, "and the playing video is not drawn again");
  assert.equal(video.src, "/api/media/file/4?ticket=t2", "nor its ticket changed");
});

// ── 6. /media/<n> keeps the label cover ──────────────────────────────────

// Seen red 2026-10-11 with renderViewer's `covered` branch taken out (round 2's viewer): "the
// label cover, not the picture". Media round 5, lane 1G (2026-10-12): an item's address is
// /media/<n> now and `/media?id=<n>` is gone, so the page is opened at the path /media/42 (the
// harness's `path`, where it took `openId`); what the test proves is unchanged.
test("6. /media/<n> shows a labelled item's label cover and fetches nothing until it is clicked", async () => {
  const it = item({ id: 42, title: "Fire show", kind: "picture", url: "/api/media/file/42", visibility: "members", labels: ["flashing"], filename: "42_f.png" });
  const { doc, net } = await openPage({ path: "/media/42", routes: listRoutes(listAnswer([]), [
    [(c) => c.url === "/api/media/list" && c.body.id === 42, () => respond(200, listAnswer([it]))],
    [at("/api/media/file/42"), () => respond(200, "", new Uint8Array([42]))],
  ]) });
  const viewer = byId(doc, "media-viewer-open");
  const veil = byId(doc, "media-viewer-veil");
  assert.ok(veil, "the label cover, not the picture");
  assert.equal(text(veil.querySelector(".media-veil-labels")), "Flashing lights");
  assert.equal(text(veil.querySelector(".media-veil-show")), media.MEDIA_WORDS.clickToShow);
  assert.equal(viewer.querySelector("img"), null, "no picture drawn");
  assert.equal(byId(doc, "media-download"), null, "nor a Download");
  assert.equal(net.calls.filter((c) => path0(c.url) === "/api/media/file/42").length, 0, "nothing fetched before the click");
  veil.click();
  await settle();
  assert.equal(net.calls.filter((c) => path0(c.url) === "/api/media/file/42").length, 1, "fetched on the click");
  assert.ok(byId(doc, "media-viewer-open").querySelector("img"), "and shown");
});

// ── 8. The fingerprint ───────────────────────────────────────────────────

// Seen red 2026-10-11 with findUpload's `r.fp === fp` comparison taken out: "a different file with
// the same name, size and date starts afresh".
test("8. resuming matches the file's fingerprint (its first and last megabyte), not only its name, size and date", async () => {
  const remembered = (fill) => JSON.stringify([{ owner: ME, id: "a".repeat(32), name: "clip.mp4", size: 20 * MIB, lastModified: 1700000000000, fp: fingerprintOf(new Uint8Array(20 * MIB).fill(fill)), at: T0 - 3600000 }]);
  // A copy edited in place: the same name, size and date, other bytes.
  const storage = fakeStorage();
  storage.setItem(upload.RESUME_KEY, remembered(0));
  const relay = relay3();
  relay.next = 1; // the server's new session is "b...": the remembered "a..." is not it
  relay.sessions.set("a".repeat(32), { id: "a".repeat(32), offset: 8 * MIB, size: 20 * MIB, stored: "1760_old.mp4", type: "video/mp4", done: null, deleted: false });
  const other = uploader(relay, aFile("clip.mp4", 20 * MIB, { fill: 9 }), { storage });
  other.u.start();
  await drive(other.sl, () => other.u.state.phase === "done");
  assert.equal(relay.log.filter((e) => e.kind === "start").length, 1, "a different file with the same name, size and date starts afresh");
  assert.equal(relay.pieces[0].offset, 0);
  // The same bytes: carried on from where the server got.
  const storage2 = fakeStorage();
  storage2.setItem(upload.RESUME_KEY, remembered(9));
  const relay2 = relay3();
  relay2.sessions.set("a".repeat(32), { id: "a".repeat(32), offset: 8 * MIB, size: 20 * MIB, stored: "1760_old.mp4", type: "video/mp4", done: null, deleted: false });
  const same = uploader(relay2, aFile("clip.mp4", 20 * MIB, { fill: 9 }), { storage: storage2 });
  same.u.start();
  await drive(same.sl, () => same.u.state.phase === "done");
  assert.equal(relay2.log.filter((e) => e.kind === "start").length, 0, "the same file carries on");
  assert.equal(relay2.pieces[0].offset, 8 * MIB);
  // The fingerprint itself: SHA-256 of the first and the last megabyte.
  const bytes = new Uint8Array(3 * MIB);
  bytes[0] = 1;
  bytes[3 * MIB - 1] = 2;
  const f = { name: "x", size: bytes.length, slice: (a, b) => new Blob([bytes.slice(a, b)]) };
  assert.equal(await upload.fingerprint(f, SHA256), fingerprintOf(bytes));
  const before = await upload.fingerprint(f, SHA256);
  bytes[MIB + 5] = 3; // the middle is not in it
  assert.equal(await upload.fingerprint(f, SHA256), before, "the middle is not in it");
  bytes[3 * MIB - 1] = 4;
  assert.notEqual(await upload.fingerprint(f, SHA256), before, "the last megabyte is");
});

// ── B. Both apps ──────────────────────────────────────────────────────────

// Seen red 2026-10-11 with call() leaving the AbortController's signal off the request (the piece
// on its way never stopped): "'pausing' == 'paused'"; and with the loop's two `if (stopping())`
// checks before a piece taken out (round 2 asked where the server got after its busy wait
// whatever Pause said): "no request after Pause, in the busy wait / 4 !== 3".
test("B1. Pause stops every request in every phase: the piece on its way, a server error's wait, the busy wait, an allowance's wait", async () => {
  // The piece on its way.
  const relayA = relay3({ piece: async (_c, _s, off) => { if (off === 8 * MIB) await new Promise(() => {}); return null; } });
  const a = uploader(relayA, aFile("clip.mp4", 20 * MIB));
  a.u.start();
  await settle();
  a.u.pause();
  await drive(a.sl, null, 5);
  assert.equal(a.u.state.phase, "paused");
  assert.ok(relayA.pieces.at(-1).c.aborted, "the piece on its way is stopped");
  // Each of the waits: no request at all after Pause, however long it is left.
  const cases = {
    "a server error's wait": (off) => (off === 8 * MIB ? respond(503, NGINX_503) : null),
    "the busy wait": (off) => (off === 8 * MIB ? respond(409, { error: "Still writing.", busy: true }) : null),
    "an allowance's wait": (off) => (off === 8 * MIB ? respond(429, { error: "The hour's uploads are used.", allowance: "hour", resets_in_s: 600, left_bytes: 0 }) : null),
  };
  for (const [name, answer] of Object.entries(cases)) {
    const relay = relay3({ piece: async (_c, _s, off) => answer(off) });
    const up = uploader(relay, aFile("clip.mp4", 20 * MIB));
    up.u.start();
    await settle();
    assert.equal(up.sl.wakes.length, 1, name + ": waiting");
    const seen = relay.log.length;
    up.u.pause();
    await drive(up.sl, null, 5);
    assert.equal(up.u.state.phase, "paused", name + ": paused");
    assert.equal(relay.log.length, seen, "no request after Pause, in " + name);
  }
});

// Seen red 2026-10-11 with allowanceWait ignoring resets_in_s (round 2's resets_at against this
// device's clock): "the server's own seconds"; with its 5-second floor taken out: "at least 5
// seconds"; and with the cut to left_bytes taken out (`pieceCap = left` removed): "a piece cut to
// what is left of the allowance".
test("B2. an allowance waits the server's resets_in_s (at least 5 seconds), and a piece is cut to left_bytes", async () => {
  let refused = 0;
  const relay = relay3({
    piece: async (_c, _s, off, len) => {
      // 3 MiB left of the day: an 8 MiB piece is refused with what is left; a 3 MiB one taken;
      // then nothing is left for 2 seconds (by the server's clock).
      if (off === 8 * MIB && len > 3 * MIB && refused === 0) { refused++; return respond(429, { error: "Over the day's allowance.", allowance: "day", resets_at: 1, resets_in_s: 7200, left_bytes: 3 * MIB, offset: 8 * MIB }); }
      if (off === 11 * MIB && refused === 1) { refused++; return respond(429, { error: "Over the day's allowance.", allowance: "day", resets_at: 1, resets_in_s: 2, left_bytes: 0, offset: 11 * MIB }); }
      return null;
    },
  });
  const up = uploader(relay, aFile("clip.mp4", 20 * MIB), { now: T0 });
  up.u.start();
  await settle();
  assert.deepEqual(relay.pieces.map((p) => [p.offset, p.len]).slice(0, 3), [[0, 8 * MIB], [8 * MIB, 8 * MIB], [8 * MIB, 3 * MIB]], "a piece cut to what is left of the allowance, sent at once");
  assert.equal(up.u.state.phase, "allowance");
  assert.deepEqual(up.sl.waits, [5000], "at least 5 seconds, though the server said 2");
  assert.equal(up.u.state.resetsAt, T0 + 5000);
  up.sl.wakeAll();
  await drive(up.sl, () => up.u.state.phase === "done");
  assert.equal(up.u.state.phase, "done", "then it carries on by itself");
  assert.equal(relay.pieces.at(-1).offset + relay.pieces.at(-1).len, 20 * MIB);
  // The server's own seconds when they are longer.
  const relay2 = relay3({ piece: async (_c, _s, off) => (off === 8 * MIB && !relay2.waited ? respond(429, { error: "The hour's uploads are used.", allowance: "hour", resets_in_s: 1500, left_bytes: 0 }) : null) });
  const up2 = uploader(relay2, aFile("clip.mp4", 20 * MIB));
  up2.u.start();
  await settle();
  assert.deepEqual(up2.sl.waits, [1500 * 1000], "the server's own seconds");
  relay2.waited = true;
  // At the start too.
  const relay3b = relay3({ start: async () => (relay3b.waited ? null : respond(429, { error: "The hour's uploads are used.", allowance: "hour", resets_in_s: 60 })) });
  const up3 = uploader(relay3b, aFile("clip.mp4", 4 * MIB));
  up3.u.start();
  await settle();
  assert.equal(up3.u.state.phase, "allowance");
  assert.deepEqual(up3.sl.waits, [60000]);
  relay3b.waited = true;
  await drive(up3.sl, () => up3.u.state.phase === "done");
  assert.equal(up3.u.state.phase, "done");
});

// Seen red 2026-10-11 with at() putting `?token=` back in the address (round 2): "no token in any
// address"; and with headers() leaving X-Upload-Token out: "every request carries the token in
// its header".
test("B3. every resumable request carries the token in X-Upload-Token, and none in its address", async () => {
  let dropOnce = true;
  const relay = relay3({ piece: async (_c, _s, off) => { if (off === 8 * MIB && dropOnce) { dropOnce = false; throw new TypeError("Failed to fetch"); } return null; } });
  const up = uploader(relay, aFile("clip.mp4", 20 * MIB));
  up.u.start();
  await drive(up.sl, () => up.u.state.phase === "done");
  const kinds = new Set(relay.log.map((e) => e.kind));
  assert.deepEqual([...kinds].sort(), ["GET", "PATCH", "start"], "the start, the pieces and asking where it got");
  for (const e of relay.log) {
    assert.ok(!/token/i.test(e.c.url), "no token in any address: " + e.c.url);
    assert.equal(e.c.headers["X-Upload-Token"], "tok-1", "every request carries the token in its header: " + e.kind);
  }
  // And Cancel's DELETE.
  let hang;
  const relay2 = relay3({ piece: async (_c, _s, off) => { if (off === 8 * MIB) await new Promise((r) => { hang = r; }); return null; } });
  const up2 = uploader(relay2, aFile("clip.mp4", 20 * MIB));
  up2.u.start();
  await settle();
  up2.u.cancel();
  await drive(up2.sl, () => up2.u.state.phase === "cancelled");
  const del = relay2.log.find((e) => e.kind === "DELETE");
  assert.ok(del && del.c.headers["X-Upload-Token"] === "tok-1" && !/token/i.test(del.c.url), "the cancel too");
  assert.equal(up2.u.state.phase, "cancelled");
  hang();
});
