// The Media page on the website, round 2 (2026-10-11): docs/design/media-showcase.md "Increment 2
// follow-ups, exact" and its As built note (the relay's routes, fields and refusal words), "The
// operator's answers (2026-10-11)" and "Resumable uploads, exact", and the fixes the review of the
// web Media page asked for.
//
// Run: node --test scripts/tests/media-web-round2.test.js   (in `just rig-tests`)
//
// Against the stand-ins in scripts/tests/media-web-harness.js (shared with media-web.test.js).
// web/shared/pq-relay-auth.js and web/chat/chat-reports.js run in their own node:vm global scope,
// as they do in the browser. HOS_WEB_DIR points the test at another copy of web/ (used to see each
// test red).
//
// What it proves:
//  A. No two signed requests from one page share a timestamp (pq-relay-auth.js), so Accept on a
//     suggestion, which reads the list twice at once, is never refused as a replay.
//  B. Every write on an item names its file (`filename`, the relay's W5); a 409 "not the Media
//     item you were looking at" loads the list again and says so; an item whose file the answer
//     does not name is never acted on.
//  C. The person's own share comes from POST /api/media/mine (uploads in progress counted, the
//     role's largest Media file), refreshed after an upload, a publish and a removal; the uploads
//     waiting to be published are listed with Publish and Remove (by file name).
//  D. Members-only files: fetched with the token in the X-Upload-Token header and shown through
//     object URLs; no token in any address on the page; revoked once no longer shown; Download
//     saves from the fetched file under the uploader's own name.
//  E. The viewer is not drawn again when the session changes or the list reloads, so a playing
//     video keeps playing.
//  F. The featured strip shows every featured item (featured=1), not only those on the page.
//  G. The admins' Save ends on any refusal, on a dropped connection or after no answer, and only
//     an admin's save that carries what was sent counts as the answer.
//  H. Add to Media only for a role that may upload.
//  I. Keep on the server and Stop keeping (admins and moderators), the kept badge, Keep in the
//     prune lists, kept_bytes on its own line.
//  J. My picks from POST /api/media/picks (signed, `of`), Pick and Unpick from `picked_by_me`.
//  K. Remove's words from each item's `keeps_file`.
//  L. Report on a tile: context `media`, evidence [{kind: "media", id}], signed by the reporter;
//     the help outside this server for the danger reasons; the relay's answer or refusal.
//  M. The admins' Reports view in the chat shows a Media item's title, kind, labels and
//     visibility with an Open that loads it by number (/media?id=<n>), which the page opens.
//  N. The resumable upload: pieces, progress, Pause and Resume, a dropped connection, the
//     allowance (429) with when it resets, no room at the finish (507) with Retry sending the
//     empty piece, an answer lost on the way (`done`), carrying on after a reload, storage that
//     refuses.
//  O. A long category name does not widen the filters past a phone's width.
//
// Seen red, 2026-10-11: each with one break made in a fresh copy of web/ (pages, shared, chat)
// under HOS_WEB_DIR, pq-relay-auth.js and chat-reports.js included, then green again on the real
// web/: see the note above each test.
//
// Round 3 (2026-10-11, media-web-round3.test.js has the rest) changed what some of these hold the
// page to, and each changed test says how: D and E (members-only Download and video go through a
// ticket), N3 (Pause stops the piece on its way, the record carries a fingerprint), N4 (an
// allowance waits the server's `resets_in_s` and carries on by itself), N8 (the form moves into
// the uploads list), and every piece is its bytes with the token in the X-Upload-Token header.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const H = require("./media-web-harness.js");
const {
  ROOT, WEB, media, upload, admin, report, ME, ANN, MIB, byId, text, everyElement, respond, fakeFetch, path0, at,
  queryOf, fakeSession, fakeStorage, settle, item, listAnswer, mineAnswer, listRoutes, openPage, openAdmin, usageAnswer,
  BLAKE, SHA256, fingerprintOf, bodyLen,
} = H;

const REASONS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "report_reasons.json"), "utf8"));
const OUTSIDE = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "outside_help.json"), "utf8"));
const LABELS_FILE = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "media", "labels.json"), "utf8"));
const ID = "a".repeat(32);

const tileOf = (doc, id) => byId(doc, "media-grid").querySelectorAll(".media-tile").find((t) => t.dataset.id === String(id));
const notice = (doc) => text(byId(doc, "media-notice").querySelector(".media-notice-text"));

function aFile(name, size, type, lastModified) {
  return {
    name, size, type: type || "video/mp4", lastModified: lastModified || 1700000000000,
    slice: (a, b) => new Blob([new Uint8Array(Math.max(0, Math.min(size, b) - a))]),
  };
}

async function pickFile(doc, file) {
  const input = byId(doc, "media-file-input");
  input.files = [file];
  input.dispatch("change");
  await settle();
}

// ── A. Signed requests never share a timestamp ───────────────────────────

/** pq-relay-auth.js in its own global scope, with this clock and a stand-in identity. */
function loadPqRelayAuth(clock) {
  const ctx = {
    console: { warn() {}, log() {} },
    localStorage: { getItem: (k) => (k === "humanity_key_backup" ? JSON.stringify({ privateKeyPkcs8: Buffer.alloc(48, 5).toString("base64") }) : null) },
    atob: (s) => Buffer.from(s, "base64").toString("binary"),
    TextEncoder, TextDecoder, Uint8Array,
    Date: { now: clock },
    crypto: {},
  };
  ctx.window = ctx;
  ctx.pqDeriveIdentity = async () => ({ dilithiumPublicHex: ME, dilithiumSecret: new Uint8Array(1) });
  ctx.pqSignMessage = async (_secret, bytes) => new Uint8Array(Buffer.from("sig:" + new TextDecoder().decode(bytes)));
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(WEB, "shared", "pq-relay-auth.js"), "utf8"), ctx, { filename: "pq-relay-auth.js" });
  return ctx;
}

// Seen red 2026-10-11 with `const ts = Date.now()` back in getPqSignedAuth: "two signatures in
// the same millisecond never share a timestamp".
test("A. no two signed requests from one page share a timestamp", async () => {
  const auth = loadPqRelayAuth(() => 1760000000000);
  const [a, b] = await Promise.all([auth.getPqSignedAuth("media_list"), auth.getPqSignedAuth("media_list")]);
  assert.notEqual(a.timestamp, b.timestamp, "two signatures in the same millisecond never share a timestamp");
  const c = await auth.getPqSignedAuth("media_usage");
  assert.ok(c.timestamp > Math.max(a.timestamp, b.timestamp), "and never go back");
  assert.equal(Buffer.from(a.sig, "hex").toString(), "sig:media_list\n" + a.timestamp, "each signed over its own timestamp");
  // The clock moving on is used as it is.
  let now = 1760000000000;
  const later = loadPqRelayAuth(() => now);
  const x = await later.getPqSignedAuth("media_list");
  now += 5000;
  const y = await later.getPqSignedAuth("media_list");
  assert.equal(y.timestamp - x.timestamp, 5000);
});

// Seen red 2026-10-11 with the same break (Date.now() in getPqSignedAuth): "the two media_list
// reads carry different timestamps (and every other signed request too)".
test("A2. Accept on a suggestion reads the list twice at once, and the two reads never share a timestamp", async () => {
  // The page and its admins' section, signing through pq-relay-auth.js with a clock that does not
  // move: Accept reloads the categories (media_list) and the page's list (media_list) at once.
  const frozen = loadPqRelayAuth(() => 1760000000000);
  const sign = (p) => frozen.getPqSignedAuth(p);
  const out = await openPage({
    session: fakeSession({ role: "admin" }), sign, routes: listRoutes(listAnswer([]), [
      [at("/api/media/suggestions", "POST"), () => respond(200, { all: true, suggestions: [{ id: 3, suggester_key: ANN, suggester_name: "Ann", name: "Builds", note: "", created_at: 1, state: "open" }] })],
      [at("/api/media/suggestions/decide", "POST"), () => respond(200, { ok: true })],
      [at("/api/media/usage", "POST"), () => respond(200, usageAnswer())],
    ]),
    mountAdmin: (c, ctx) => admin.mountMediaAdmin(c, {
      document: ctx.doc, fetch: ctx.net.fetch, sign, blake3: BLAKE, session: fakeSession({ role: "admin" }),
      confirm: async () => true, onChange: () => { ctx.page.load(true); ctx.page.loadFeatured(); },
    }),
  });
  await settle();
  const before = out.net.calls.length;
  byId(out.doc, "media-admin-suggestions").querySelector('[data-decide="accept"]').click();
  await settle();
  const reads = out.net.calls.slice(before).filter((c) => c.url === "/api/media/list");
  assert.ok(reads.length >= 2, "Accept reads the list more than once: " + reads.length);
  const all = out.net.calls.filter((c) => c.body && c.body.sig && c.body.timestamp).map((c) => c.body.timestamp);
  assert.equal(new Set(all).size, all.length, "the two media_list reads carry different timestamps (and every other signed request too)");
});

// ── B. Every write names the item's file ─────────────────────────────────

// Seen red 2026-10-11 with act() reloading nothing on a 409 (only saying the words): "the list
// is asked for again (and the strip)"; with itemFileName reading only `url`: "an item the server
// names by `filename` is acted on".
test("B. a write names the item's file; a page out of date is loaded again; an unnamed file is never acted on", async () => {
  let stale = true;
  const answer = listAnswer([
    item({ id: 5 }),
    item({ id: 7, url: "/api/media/file/7", visibility: "members", filename: "7_m.png", uploader_key: ME }),
    item({ id: 8, url: "/api/media/file/8", visibility: "members", uploader_key: ME, title: "No name here" }),
  ]);
  const { doc, net } = await openPage({ session: fakeSession({ role: "admin" }), routes: listRoutes(answer, [
    [at("/api/media/file/7"), () => respond(200, "")],
    [at("/api/media/file/8"), () => respond(200, "")],
    [at("/api/media/pick", "POST"), () => (stale
      ? respond(409, "That is not the Media item you were looking at: the page is out of date. Load it again.")
      : respond(200, { ok: true, id: 5, picked: true, picks: 1 }))],
    [at("/api/media/feature", "POST"), (c) => respond(200, { ok: true, id: c.body.id, featured: true })],
  ]) });
  const listsBefore = net.calls.filter((c) => c.url === "/api/media/list").length;
  tileOf(doc, 5).querySelector('[data-action="pick"]').click();
  await settle();
  assert.equal(net.calls.find((c) => c.url === "/api/media/pick").body.filename, "1_a.png");
  assert.equal(notice(doc), media.MEDIA_WORDS.reloaded, "and says the page was out of date");
  assert.ok(net.calls.filter((c) => c.url === "/api/media/list").length >= listsBefore + 2, "the list is asked for again (and the strip)");
  // An item the server names by `filename` (a members-only one, whose address is its number).
  tileOf(doc, 7).querySelector('[data-action="feature"]').click();
  await settle();
  const f = net.calls.filter((c) => c.url === "/api/media/feature").at(-1);
  assert.equal(f && f.body.filename, "7_m.png", "an item the server names by `filename` is acted on");
  // An item whose file the answer does not name: nothing is sent, and the page says why.
  const sentBefore = net.calls.length;
  tileOf(doc, 8).querySelector('[data-action="feature"]').click();
  await settle();
  assert.equal(net.calls.length, sentBefore, "nothing is sent for an item whose file is not named");
  assert.equal(notice(doc), media.MEDIA_WORDS.noFileName);
  assert.equal(media.itemFileName(item({ url: "/uploads/a/../b" })), "", "never a path");
  assert.equal(media.itemFileName(item({ filename: "/uploads/9_x.png" })), "9_x.png");
});

// ── C. The person's own share and their uploads waiting ──────────────────

// Seen red 2026-10-11 with openAddForm leaving the uploads in progress out of what is used:
// "the share is the relay's own count"; and with renderWaiting drawing nothing: "the uploads
// waiting are listed".
test("C. the share is the relay's own count, uploads waiting are listed with Remove, and it is asked again after each change", async () => {
  let mine = mineAnswer({ used_bytes: 112 * MIB, in_progress_bytes: 88 * MIB, waiting: [{ filename: "1760_w.mp4", original_name: "Beach.mp4", size_bytes: 12 * MIB, uploaded_at: 1 }] });
  const { doc, net, confirms } = await openPage({ routes: listRoutes(listAnswer([]), [
    [at("/api/media/mine", "POST"), () => respond(200, mine)],
    [at("/api/media/remove", "POST"), () => { mine = mineAnswer({ used_bytes: 112 * MIB, in_progress_bytes: 88 * MIB }); return respond(200, { ok: true, removed_upload: "1760_w.mp4", file_removed: true, file_kept: false }); }],
  ]) });
  const mineCalls = () => net.calls.filter((c) => c.url === "/api/media/mine");
  assert.equal(mineCalls()[0].body.sig, "sig:media_mine", "signed as media_mine");
  const list = byId(doc, "media-waiting-list");
  assert.ok(list, "the uploads waiting are listed");
  const rows = list.querySelectorAll("[data-waiting]");
  assert.equal(rows.length, 1, "the uploads waiting are listed");
  assert.equal(text(rows[0].querySelector(".media-admin-row-text")), "Beach.mp4 · 12 MB");
  // Remove a waiting upload: by its file name, then the share is asked for again.
  const n0 = mineCalls().length;
  byId(doc, "media-waiting-list").querySelector("[data-waiting-remove]").click();
  await settle();
  assert.equal(confirms.at(-1), '"Beach.mp4"\n\n' + media.MEDIA_WORDS.removeWaiting);
  const rm = net.calls.find((c) => c.url === "/api/media/remove");
  assert.equal(rm.body.filename, "1760_w.mp4", "removed by its file name");
  assert.equal(rm.body.id, undefined);
  assert.equal(rm.body.sig, "sig:media_remove:1760_w.mp4");
  assert.ok(mineCalls().length > n0, "the share is asked for again after a removal");
  assert.equal(byId(doc, "media-waiting-list"), null, "and the list follows it");
  assert.equal(notice(doc), media.MEDIA_WORDS.removedWaiting);
  // The share: 112 MB used and 88 MB still arriving, of 512 MB.
  await pickFile(doc, aFile("clip.mp4", 400 * MIB));
  assert.equal(text(byId(doc, "media-share-line")), "You have 312 MB of your 512 MB left", "the share is the relay's own count");
  assert.match(text(byId(doc, "media-form-stop")), /more than the 312 MB you have left/);
  byId(doc, "media-form-cancel").click();
  // The role's largest Media file.
  mine = mineAnswer({ file_limit_bytes: 50 * MIB });
  await pickFile(doc, aFile("clip.mp4", 60 * MIB));
  assert.match(text(byId(doc, "media-form-stop")), /over your role's 50 MB limit for one Media file/);
  byId(doc, "media-form-cancel").click();
  // What is left of the whole Media space.
  mine = mineAnswer({ budget_left_bytes: 30 * MIB });
  await pickFile(doc, aFile("clip.mp4", 40 * MIB));
  assert.match(text(byId(doc, "media-form-stop")), /more than the 30 MB left in this server's Media space/);
});

// Seen red 2026-10-11 with openWaitingForm leaving the upload's stored name out of the form:
// "published under its stored name".
test("C2. Publish on an upload that waits opens the form and publishes that file, then asks for the share again", async () => {
  const mine = mineAnswer({ waiting: [{ filename: "1760_w.mp4", original_name: "Beach.mp4", size_bytes: 12 * MIB, uploaded_at: 1 }] });
  const { doc, net } = await openPage({ routes: listRoutes(listAnswer([]), [
    [at("/api/media/mine", "POST"), () => respond(200, mine)],
    [at("/api/media", "POST"), () => respond(200, { ok: true, item: item() })],
  ]) });
  const n0 = net.calls.filter((c) => c.url === "/api/media/mine").length;
  byId(doc, "media-waiting-list").querySelector("[data-waiting-publish]").click();
  assert.equal(text(byId(doc, "media-form").querySelector("h2")), "Publish an upload");
  assert.ok(byId(doc, "media-form-vis-members"), "an upload made for Media may be members only");
  byId(doc, "media-form-go").click();
  await settle();
  const pub = net.calls.find((c) => c.url === "/api/media" && c.init.method === "POST");
  assert.equal(pub.body.filename, "1760_w.mp4", "published under its stored name");
  assert.equal(pub.body.sig, "sig:media_publish:1760_w.mp4");
  assert.ok(net.calls.filter((c) => c.url === "/api/media/mine").length > n0, "the share is asked for again after a publish");
});

// ── D. Members-only files by header, through object URLs ─────────────────

// Seen red 2026-10-11 with sweepBlobs taking nothing away: "the object URL of a file no longer
// shown is revoked"; with downloadControl linking the members' file route instead of the fetched
// file: "the viewer's Download is the fetched file".
// Round 3 (R3-8): a members-only file's Download (not a picture already here) goes through a
// ticket the browser saves itself, never through the page's memory. Seen red 2026-10-11 with
// Download's press fetching the file into the page again (blobFor): "Download asks for a ticket".
test("D. members-only files come by header through object URLs, no token anywhere, revoked when gone, saved under their own name", async () => {
  let showSeven = true;
  const seven = item({ id: 7, url: "/api/media/file/7", visibility: "members", title: "Members' photo", original_name: "IMG_0001.jpg" });
  const nine = item({ id: 9, url: "/api/media/file/9", visibility: "members", kind: "file", title: "Plans", original_name: "Workshop plans.pdf", uploader_key: ANN, filename: "9_plans.pdf" });
  const { doc, net, urls, saved, page } = await openPage({ routes: listRoutes(null, [
    [(c) => c.url === "/api/media/list" && c.body.featured !== 1 && !c.body.id, () => respond(200, listAnswer(showSeven ? [seven, nine] : [nine]))],
    [(c) => c.url === "/api/media/list" && c.body.featured === 1, () => respond(200, listAnswer([]))],
    [at("/api/media/file/7"), () => respond(200, "", new Uint8Array([1]))],
    [at("/api/media/file/9"), () => respond(200, "", new Uint8Array([2]))],
    [at("/api/media/ticket", "POST"), (c) => respond(200, { url: "/api/media/file/" + c.body.id + "?ticket=t" + c.body.id, expires_in_s: 600 })],
  ]) });
  const img = tileOf(doc, 7).querySelector("img");
  assert.equal(img.src, "blob:test/1");
  // No token in any address on the page: every src and href.
  for (const el of everyElement(doc)) {
    for (const v of [el.src, el.href, el.getAttribute("src"), el.getAttribute("href")]) {
      if (typeof v === "string") assert.ok(!/token/i.test(v), "no token in an address: " + v);
    }
  }
  for (const c of net.calls.filter((x) => x.url.startsWith("/api/media/file/"))) {
    assert.ok(!/token/i.test(c.url), "the file route is asked for with no token in its address: " + c.url);
    assert.equal(c.headers["X-Upload-Token"], "tok-1");
  }
  // The picture in the viewer: its Download is the fetched file, under the uploader's own name.
  tileOf(doc, 7).querySelector(".media-thumb").click();
  const dl = byId(doc, "media-download");
  assert.equal(dl.href, "blob:test/1", "the viewer's Download is the fetched file");
  assert.equal(dl.getAttribute("download"), "IMG_0001.jpg", "under the name it had on the uploader's device");
  byId(doc, "media-viewer-close").click();
  // A file (not shown on its tile): Download asks for a ticket and saves through it under its own name.
  assert.equal(net.calls.filter((c) => path0(c.url) === "/api/media/file/9").length, 0, "a file is not fetched for its tile");
  tileOf(doc, 9).querySelector(".media-thumb").click();
  byId(doc, "media-download").click();
  await settle();
  const ticket = net.calls.find((c) => c.url === "/api/media/ticket");
  assert.ok(ticket, "Download asks for a ticket");
  assert.deepEqual([ticket.body, ticket.headers["X-Upload-Token"]], [{ id: 9, filename: "9_plans.pdf" }, "tok-1"], "a ticket for that one file, the token in the header");
  assert.deepEqual(saved, [{ url: "/api/media/file/9?ticket=t9", name: "Workshop plans.pdf" }], "Download saves through the ticket");
  assert.equal(net.calls.filter((c) => path0(c.url) === "/api/media/file/9").length, 0, "the page itself never fetches it");
  byId(doc, "media-viewer-close").click();
  // Item 7 leaves the page (a filter, a reload): its object URL is revoked.
  showSeven = false;
  await page.load(true);
  await settle();
  assert.deepEqual(urls.revoked, ["blob:test/1"], "the object URL of a file no longer shown is revoked");
  page.dispose();
  assert.deepEqual(urls.live(), [], "and every one when the page goes");
});

// ── E. The viewer stays put through the session's changes ────────────────

// Seen red 2026-10-11 with renderAll calling renderViewer() every time again: "the same video
// element, still playing / the viewer was drawn again".
// Round 3 (R3-8): a members-only video plays through a ticket, so the browser streams it. Seen red
// 2026-10-11 with the viewer fetching the video into the page again (blobFor): "the video shows".
test("E. a playing video is not drawn again when the session changes or the list reloads", async () => {
  const clip = item({ id: 4, url: "/api/media/file/4", visibility: "members", kind: "video", title: "Fire show", filename: "4_fire.mp4" });
  const pub = item({ id: 3, url: "/uploads/3_v.mp4", kind: "video", title: "Public clip" });
  const { doc, session, page } = await openPage({ routes: listRoutes(listAnswer([clip, pub]), [
    [at("/api/media/file/4"), () => respond(200, "", new Uint8Array([4]))],
    [at("/api/media/ticket", "POST"), () => respond(200, { url: "/api/media/file/4?ticket=t4", expires_in_s: 600 })],
  ]) });
  tileOf(doc, 4).querySelector(".media-thumb").click();
  await settle();
  const video = byId(doc, "media-viewer-open").querySelector("video");
  assert.ok(video, "the video shows");
  assert.equal(video.src, "/api/media/file/4?ticket=t4");
  // The connection drops and comes back with a new token; the list reloads; a notice.
  session.state.status = "closed";
  session.state.uploadToken = "";
  session.emit({ type: "state" });
  session.state.status = "bound";
  session.state.uploadToken = "tok-2";
  session.emit({ type: "state" });
  await page.load(true);
  await settle();
  assert.equal(byId(doc, "media-viewer-open").querySelector("video"), video, "the same video element, still playing / the viewer was drawn again");
  // A public video too.
  byId(doc, "media-viewer-close").click();
  tileOf(doc, 3).querySelector(".media-thumb").click();
  const v2 = byId(doc, "media-viewer-open").querySelector("video");
  assert.equal(v2.src, "/uploads/3_v.mp4");
  session.emit({ type: "state" });
  await page.load(true);
  await settle();
  assert.equal(byId(doc, "media-viewer-open").querySelector("video"), v2);
});

// ── F. The featured strip: every featured item ────────────────────────────

// Seen red 2026-10-11 with renderFeatured filtering the loaded items again: "a featured item not
// on the loaded page is in the strip".
test("F. the featured strip shows every featured item, asked for with featured=1", async () => {
  const far = item({ id: 99, title: "From long ago", featured: true, url: "/uploads/99_old.png" });
  const { doc, net } = await openPage({ routes: listRoutes(listAnswer([item({ id: 5 })]), [
    [(c) => c.url === "/api/media/list" && c.body.featured === 1, () => respond(200, listAnswer([far]))],
  ]) });
  const strip = byId(doc, "media-featured-row");
  assert.ok(strip, "a featured item not on the loaded page is in the strip");
  assert.equal(text(strip.querySelector(".media-tile-title")), "From long ago");
  const ask = net.calls.find((c) => c.url === "/api/media/list" && c.body.featured === 1);
  assert.equal(ask.body.sig, "sig:media_list", "signed like the list");
  // Without an identity, the public form: GET /api/media?featured=1.
  const out = await openPage({ myKey: "", session: null, routes: listRoutes(listAnswer([], { members_view: false }), [
    [(c) => path0(c.url) === "/api/media" && queryOf(c.url).get("featured") === "1", () => respond(200, listAnswer([far], { members_view: false }))],
  ]) });
  assert.ok(byId(out.doc, "media-featured-row"));
});

// ── G. The admins' Save always ends ───────────────────────────────────────

// Seen red 2026-10-11 with each half of the round 1 onSession put back alone: ending a save only on
// words with "server settings" in them: "a refusal ends the save"; taking any settings answer as
// the save's: "the answer to the page's own request is not the save's answer".
test("G. the admins' Save ends on any refusal, on a dropped connection or with no answer, and only its own answer is Saved", async () => {
  const { doc, session, timers } = await openAdmin();
  const save = () => { byId(doc, "media-admin-share").value = "640"; byId(doc, "media-admin-share").dispatch("input"); byId(doc, "media-admin-save").click(); };
  const button = () => text(byId(doc, "media-admin-save"));
  const msg = () => text(byId(doc, "media-admin-settings-msg"));
  // A refusal in words the old check missed.
  save();
  assert.equal(button(), "Saving...");
  session.emit({ type: "frame", msg: { type: "system", message: "Update failed: database is locked" } });
  assert.equal(button(), "Save", "a refusal ends the save");
  assert.equal(msg(), "Update failed: database is locked");
  // The answer to this page's own settings request (target set) is not the save's answer, nor is
  // another admin's save of something else; a notice about another admin's save is not a refusal.
  save();
  session.emit({ type: "frame", msg: { type: "server_settings_state", target: ME, settings: session.state.settings } });
  assert.equal(button(), "Saving...", "the answer to the page's own request is not the save's answer");
  session.emit({ type: "frame", msg: { type: "server_settings_state", settings: Object.assign({}, session.state.settings, { media_per_person_mb: 99 }) } });
  session.emit({ type: "frame", msg: { type: "system", message: "Server settings updated by admin." } });
  assert.equal(button(), "Saving...", "another admin's save is not this one's answer");
  session.emit({ type: "frame", msg: { type: "server_settings_state", settings: Object.assign({}, session.state.settings, { media_per_person_mb: 640 }) } });
  assert.equal(button(), "Save");
  assert.equal(msg(), "Saved.");
  // No answer at all: the wait ends it.
  save();
  timers.at(-1).fn();
  assert.equal(button(), "Save", "no answer ends the save");
  assert.equal(msg(), admin.ADMIN_WORDS.noAnswer);
  // A dropped connection ends it.
  save();
  session.state.status = "closed";
  session.emit({ type: "state" });
  assert.equal(button(), "Save");
  assert.equal(msg(), admin.ADMIN_WORDS.lostConnection);
});

// ── H. Add to Media only for a role that may upload ───────────────────────

// Seen red 2026-10-11 with renderActions offering Add to Media to any signed-in member (round 1):
// "a role that cannot upload is not offered Add to Media".
test("H. Add to Media is offered only to a role that may upload", async () => {
  const cannot = await openPage({ session: fakeSession({ role: "unverified" }), routes: listRoutes(listAnswer([])) });
  assert.equal(byId(cannot.doc, "media-add-btn"), null, "a role that cannot upload is not offered Add to Media");
  assert.equal(text(byId(cannot.doc, "media-cannot-upload")), media.MEDIA_WORDS.cannotUpload, "and is told why");
  const none = await openPage({ session: fakeSession({ role: "" }), routes: listRoutes(listAnswer([])) });
  assert.equal(byId(none.doc, "media-add-btn"), null, "no role at all: no Add to Media");
  const unknown = await openPage({ session: fakeSession({ roles: null }), routes: listRoutes(listAnswer([])) });
  assert.equal(byId(unknown.doc, "media-add-btn"), null, "the role list not here yet: not offered yet");
  assert.equal(byId(unknown.doc, "media-cannot-upload"), null, "and nothing said yet");
  // The role list arrives and allows it.
  unknown.session.state.roles = H.ROLES;
  unknown.session.emit({ type: "state" });
  assert.ok(byId(unknown.doc, "media-add-btn"), "offered once the role list says the role may upload");
});

// ── I. Keep on the server ─────────────────────────────────────────────────

// Seen red 2026-10-11 with purposes.keep leaving out on/off ("media_keep:5"): "signed over the
// item and on"; and with the kept line left out of renderUsage: "kept_bytes on its own line".
test("I. admins and moderators keep an item on the server; the badge, Stop keeping, the prune lists and the meter's own line", async () => {
  const routes = listRoutes(listAnswer([item({ id: 5 }), item({ id: 6, kept: true, url: "/uploads/6_k.png" })]), [
    [at("/api/media/keep", "POST"), (c) => respond(200, { ok: true, id: c.body.id, kept: c.body.kept })],
  ]);
  const member = await openPage({ routes });
  assert.equal(member.doc.body.querySelectorAll('[data-action="keep"]').length, 0, "not for a member");
  assert.ok(tileOf(member.doc, 6).querySelector("[data-kept]"), "everyone sees the kept badge");
  assert.equal(text(tileOf(member.doc, 6).querySelector("[data-kept]")), media.MEDIA_WORDS.keptBadge);
  const mod = await openPage({ session: fakeSession({ role: "mod" }), routes });
  tileOf(mod.doc, 5).querySelector('[data-action="keep"]').click();
  await settle();
  const keep = mod.net.calls.find((c) => c.url === "/api/media/keep");
  assert.deepEqual([keep.body.id, keep.body.filename, keep.body.kept, keep.body.sig], [5, "1_a.png", true, "sig:media_keep:5:on"], "signed over the item and on");
  assert.ok(tileOf(mod.doc, 5).querySelector("[data-kept]"), "the badge shows at once");
  assert.equal(notice(mod.doc), media.MEDIA_WORDS.keptNow);
  tileOf(mod.doc, 6).querySelector('[data-action="unkeep"]').click();
  await settle();
  const stop = mod.net.calls.filter((c) => c.url === "/api/media/keep").at(-1);
  assert.deepEqual([stop.body.id, stop.body.kept, stop.body.sig], [6, false, "sig:media_keep:6:off"]);
  assert.equal(tileOf(mod.doc, 6).querySelector("[data-kept]"), null);
  // The admins' section: Keep in the prune lists, and what the server keeps on its own line.
  const a = await openAdmin({ usage: usageAnswer({ kept_bytes: 3 * 1024 * MIB }) });
  assert.equal(text(byId(a.doc, "media-admin-kept")), media.keptMeterWords(3 * 1024 * MIB), "kept_bytes on its own line");
  assert.equal(media.keptMeterWords(3 * 1024 * MIB), "Kept by the server: 3 GB of that, counted in the budget and in nobody's share");
  byId(a.doc, "media-admin-largest").querySelector("[data-prune-keep]").click();
  await settle();
  const pk = a.net.calls.find((c) => c.url === "/api/media/keep");
  assert.deepEqual([pk.body.id, pk.body.filename, pk.body.kept, pk.body.sig], [11, "11_big.mp4", true, "sig:media_keep:11:on"]);
  // A prune list's Remove names the file too.
  byId(a.doc, "media-admin-oldest").querySelector("[data-prune-remove]").click();
  await settle();
  const rm = a.net.calls.find((c) => c.url === "/api/media/remove");
  assert.deepEqual([rm.body.id, rm.body.filename, rm.body.sig], [12, "12_first.png", "sig:media_remove:12"]);
});

// ── J. My picks ───────────────────────────────────────────────────────────

// Seen red 2026-10-11 with load() reading GET /api/media/picks/<key> again: "My picks is the signed
// POST"; and with tileActions ignoring `picked_by_me`: "Unpick on an item the server says I
// picked".
test("J. My picks is the signed POST /api/media/picks, and each item's picked_by_me drives Pick and Unpick", async () => {
  const { doc, net } = await openPage({ routes: listRoutes(listAnswer([item({ id: 5, picked_by_me: true, picks: 2 }), item({ id: 6, picked_by_me: false })]), [
    [at("/api/media/picks", "POST"), () => respond(200, listAnswer([item({ id: 5, picked_by_me: true, picks: 2, visibility: "members", url: "/api/media/file/5", filename: "5_m.png" })]))],
    [at("/api/media/file/5"), () => respond(200, "")],
  ]) });
  assert.equal(tileOf(doc, 5).querySelector("[data-action]").dataset.action, "unpick", "Unpick on an item the server says I picked");
  assert.equal(tileOf(doc, 6).querySelector("[data-action]").dataset.action, "pick");
  const picks = byId(doc, "media-filter-picks");
  picks.checked = true;
  picks.dispatch("change");
  await settle();
  const ask = net.calls.find((c) => c.url === "/api/media/picks");
  assert.ok(ask, "My picks is the signed POST");
  assert.equal(ask.body.of, ME, "whose picks");
  assert.equal(ask.body.sig, "sig:media_list", "signed like the list");
  assert.equal(net.calls.filter((c) => c.url.startsWith("/api/media/picks/")).length, 0, "never the unsigned GET");
  assert.equal(byId(doc, "media-grid").querySelectorAll(".media-tile").length, 1, "members-only picks included");
});

// ── K. Remove's words from keeps_file ─────────────────────────────────────

// Seen red 2026-10-11 with removeWords reading `visibility` again (round 1): "an item from chat
// keeps its file".
test("K. Remove's words come from each item's keeps_file", () => {
  assert.equal(media.removeWords(item({ keeps_file: true, visibility: "public" })), media.MEDIA_WORDS.removeKeeps, "an item from chat keeps its file");
  assert.equal(media.removeWords(item({ keeps_file: false, visibility: "public" })), media.MEDIA_WORDS.removeDeletes);
  assert.equal(media.removeWords(item({ keeps_file: undefined })), media.MEDIA_WORDS.removeEither, "unknown: words that cover both");
  assert.equal(media.MEDIA_WORDS.removeKeeps, "Remove from Media. The file stays where it came from.");
  assert.equal(media.MEDIA_WORDS.removeDeletes, "Remove from Media and delete the file.");
});

// ── L. Report on a tile ───────────────────────────────────────────────────

/** Routes for the report's data files. */
const reportRoutes = [
  [(c) => path0(c.url) === report.REPORT_REASONS_URL, () => respond(200, REASONS)],
  [(c) => path0(c.url) === report.OUTSIDE_HELP_URL, () => respond(200, OUTSIDE)],
];

// Seen red 2026-10-11 with report.js REPORT_CONTEXTS without 'media': "the report is built:
// Where this report was made is not known."; and with sendReport leaving the item out of the
// evidence: "evidence is the item by its number".
test("L. Report on a Media tile sends report_v2 with context media and the item as evidence, signed by the reporter", async () => {
  const { doc, session } = await openPage({ routes: listRoutes(listAnswer([item({ id: 5, title: "Contact staff" }), item({ id: 6, uploader_key: ME })]), reportRoutes) });
  assert.equal(tileOf(doc, 6).querySelector('[data-action="report"]'), null, "no Report on your own item");
  tileOf(doc, 5).querySelector('[data-action="report"]').click();
  await settle();
  assert.equal(text(byId(doc, "media-report-open").querySelector("h2")), 'Report "Contact staff"');
  assert.equal(text(byId(doc, "media-report-lead")), "This goes to this server's admins and moderators. Ann is not told who reported them.");
  const reasons = byId(doc, "media-report-reasons").querySelectorAll("[data-report-reason]");
  assert.deepEqual(reasons.map((r) => r.value), report.reportReasonsFrom(REASONS).map((r) => r.id), "the reasons from the data file");
  assert.equal(byId(doc, "media-report-send").disabled, true, "nothing to send before a reason");
  // A danger reason: the help outside this server under its help.
  const danger = reasons.find((r) => r.value === "child_danger");
  danger.checked = true;
  danger.dispatch("change");
  assert.ok(text(byId(doc, "media-report-reason-help")).includes("contact your local emergency number"));
  const outside = byId(doc, "media-report-outside-help");
  assert.ok(outside, "the help outside this server for a danger reason");
  assert.equal(byId(doc, "media-report-country").value, "GB", "the language's country first");
  const link = outside.querySelector("a");
  assert.equal(link.rel, "noopener noreferrer");
  assert.equal(link.target, "_blank");
  // Spam, a note, Send.
  const spam = byId(doc, "media-report-reasons").querySelectorAll("[data-report-reason]").find((r) => r.value === "spam");
  spam.checked = true;
  spam.dispatch("change");
  assert.equal(byId(doc, "media-report-outside-help"), null, "not for other reasons");
  byId(doc, "media-report-note").value = "Posted ten times";
  byId(doc, "media-report-note").dispatch("input");
  byId(doc, "media-report-send").click();
  await settle();
  const frame = session.sent.at(-1);
  assert.ok(frame && frame.type === "report_v2", "the report is built: " + text(byId(doc, "media-report-error")));
  assert.deepEqual(Object.keys(frame), ["type", "target", "context", "reason", "note", "evidence", "ts", "sig"]);
  assert.equal(frame.type, "report_v2");
  assert.equal(frame.target, ANN);
  assert.equal(frame.context, "media", "the report is built");
  assert.equal(frame.reason, "spam");
  assert.equal(frame.note, "Posted ten times");
  assert.deepEqual(frame.evidence, [{ kind: "media", id: 5 }], "evidence is the item by its number");
  const hash = Buffer.from(await BLAKE(new TextEncoder().encode(JSON.stringify(frame.evidence)))).toString("hex");
  assert.equal(Buffer.from(frame.sig, "base64").toString(), "sig:" + report.reportPreimage(ME, ANN, "spam", hash, frame.ts), "signed by the reporter over the relay's words");
  assert.equal(text(byId(doc, "media-report-send")), "Sending...");
  // The relay's answer.
  session.emit({ type: "frame", msg: { type: "report_received", id: 12 } });
  assert.equal(byId(doc, "media-report-open"), null, "the dialog closes");
  assert.equal(notice(doc), report.REPORT_RECEIVED_LINE);
  // A refusal shows in the dialog.
  tileOf(doc, 5).querySelector('[data-action="report"]').click();
  await settle();
  const r2 = byId(doc, "media-report-reasons").querySelectorAll("[data-report-reason]").find((r) => r.value === "spam");
  r2.checked = true;
  r2.dispatch("change");
  byId(doc, "media-report-send").click();
  await settle();
  session.emit({ type: "frame", msg: { type: "system", message: "Server settings updated by admin." } });
  assert.equal(text(byId(doc, "media-report-send")), "Sending...", "another notice is not the answer");
  session.emit({ type: "frame", msg: { type: "system", message: "You reported them already today." } });
  assert.equal(text(byId(doc, "media-report-error")), "You reported them already today.");
  assert.equal(text(byId(doc, "media-report-send")), "Send report");
  assert.deepEqual(report.mediaEvidenceItem("7"), { kind: "media", id: 7 });
  assert.equal(report.mediaEvidenceItem(0), null);
  assert.equal(report.reportEvidenceProblem([{ kind: "media", id: 5 }]), null);
  assert.equal(report.reportEvidenceProblem([{ kind: "media", id: "5" }]), "The Media item is not named.");
});

// ── M. The admins' Reports view and /media?id=<n> ─────────────────────────

/** chat-reports.js in its own global scope, over report.js, with fetch serving the Media data files. */
function loadChatReports() {
  const ctx = {
    console: { log() {}, warn() {}, error() {} },
    fetch: async (u) => {
      const p = String(u);
      if (p === "/data/media/kinds.json") return { ok: true, json: async () => H.KINDS };
      if (p === "/data/media/labels.json") return { ok: true, json: async () => LABELS_FILE };
      return { ok: false, json: async () => null };
    },
    handleMessage: () => {},
    document: undefined,
  };
  ctx.window = ctx;
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(WEB, "shared", "report.js"), "utf8"), ctx, { filename: "report.js" });
  vm.runInContext(fs.readFileSync(path.join(WEB, "chat", "chat-reports.js"), "utf8"), ctx, { filename: "chat-reports.js" });
  return ctx;
}

// Seen red 2026-10-11 with reportsViewHtml drawing a Media item's (empty) text like a message's:
// "the item's title"; and with report.js reportItemView dropping the item's number: "Open loads
// it by its number".
test("M. the admins' Reports view shows a reported Media item with Open, and /media?id=<n> opens it", async () => {
  const ctx = loadChatReports();
  const set = vm.runInContext("(items) => { reportsView.open = false; reportsView.tab = 'open'; reportsView.lists.open = items.map(reportItemView); }", ctx);
  const view = () => vm.runInContext("reportsViewHtml(reportsViewModel())", ctx);
  set([{ id: 1, target: ANN, context: "media", reason: "spam", note: "", created_at: 1760000000000, state: "open",
    evidence: [{ kind: "media", id: 42, uploader: ANN, title: "Fire show", media_kind: "video", labels: ["flashing"], visibility: "members", checked: true }] }]);
  view();
  await settle();
  const html = view();
  assert.ok(html.includes("Fire show"), "the item's title");
  assert.ok(html.includes("Video · Labels: Flashing lights · Members only"), "its kind, labels and who can see it, by their names: " + html.slice(html.indexOf("report-media-facts"), html.indexOf("report-media-facts") + 160));
  assert.ok(html.includes('href="/media?id=42"'), "Open loads it by its number");
  assert.ok(/data-report-media-open="42"[^>]*target="_blank" rel="noopener noreferrer"/.test(html), "in a new tab");
  assert.ok(html.includes("Checked: on the Media page, put there by"), "the checked badge");
  assert.ok(html.includes("The Media page"), "where it happened");
  // An item the relay could not find: its number, not proven.
  set([{ id: 2, target: ANN, context: "media", reason: "spam", evidence: [{ kind: "media", id: 43, checked: false }] }]);
  const h2 = view();
  assert.ok(h2.includes("Media item 43") && h2.includes(report.REPORT_NOT_PROVEN));

  // The Media page opened at /media?id=42: the list route asked for that item, and the viewer shows it.
  const it = item({ id: 42, title: "Fire show", kind: "video", url: "/uploads/42_f.mp4" });
  const { doc, net } = await openPage({ openId: 42, routes: listRoutes(listAnswer([]), [
    [(c) => c.url === "/api/media/list" && c.body.id === 42, () => respond(200, listAnswer([it]))],
  ]) });
  const ask = net.calls.find((c) => c.url === "/api/media/list" && c.body.id === 42);
  assert.ok(ask, "the list route is asked for id=42");
  assert.equal(ask.body.limit, 1);
  assert.equal(text(byId(doc, "media-viewer-open").querySelector("h2")), "Fire show");
  const gone = await openPage({ openId: 77, routes: listRoutes(listAnswer([])) });
  assert.equal(notice(gone.doc), media.MEDIA_WORDS.notFound);
});

// ── N. The resumable upload ───────────────────────────────────────────────

/**
 * A stand-in for src/relay/resumable.rs: one session at a time. `hooks.piece(call, offset)` may
 * answer a piece itself (or throw, a dropped connection) before the server takes it.
 */
function resumableServer(size, hooks) {
  const s = { id: ID, offset: 0, size, chunk: 8 * MIB, done: null, starts: 0, deleted: 0, pieces: [], hooks: hooks || {} };
  const done = () => ({ url: "/uploads/1760_clip.mp4", filename: "1760_clip.mp4", size, type: "video/mp4" });
  s.routes = [
    [at("/api/upload/resumable", "POST"), async (c) => {
      if (s.hooks.start) { const r = await s.hooks.start(c); if (r) return r; }
      s.starts++;
      return respond(201, { id: ID, offset: 0, size: c.body.size, chunk_max: s.chunk });
    }],
    [(c) => path0(c.url) === "/api/upload/resumable/" + ID && (c.init.method || "GET") === "GET", async () => {
      if (s.hooks.status) { const r = await s.hooks.status(); if (r) return r; }
      return respond(200, Object.assign({ offset: s.offset, size: s.size, chunk_max: s.chunk }, s.done ? { done: s.done } : {}));
    }],
    [(c) => path0(c.url) === "/api/upload/resumable/" + ID && c.init.method === "PATCH", async (c) => {
      const off = Number(queryOf(c.url).get("offset"));
      // Round 3: a piece is its bytes (read before it is sent), and the token is in the header (R3-9).
      s.pieces.push({ offset: off, len: bodyLen(c.init.body), token: c.headers["X-Upload-Token"], inAddress: /token/i.test(c.url) });
      if (s.hooks.piece) { const r = await s.hooks.piece(c, off); if (r) return r; }
      if (s.done) return respond(200, s.done);
      if (off !== s.offset) return respond(409, { error: "This piece starts at byte " + off + ", but the server has " + s.offset + " bytes of the file. Send from there.", offset: s.offset, size: s.size });
      s.offset += bodyLen(c.init.body);
      if (s.offset >= s.size) {
        if (s.hooks.finish) { const r = await s.hooks.finish(); if (r) return r; }
        s.done = done();
        return respond(200, s.done);
      }
      return respond(200, { offset: s.offset, size: s.size });
    }],
    [(c) => path0(c.url) === "/api/upload/resumable/" + ID && c.init.method === "DELETE", () => { s.deleted++; return respond(200, { ok: true }); }],
  ];
  return s;
}

function uploaderFor(server, file, o) {
  const opts = o || {};
  const net = fakeFetch(server.routes);
  const phases = [];
  const result = { done: null };
  const storage = opts.storage || fakeStorage();
  const u = upload.createUploader({
    fetch: net.fetch, token: opts.token || (() => "tok-1"), file, owner: ME, storage, digest: SHA256,
    sleep: opts.sleep || (() => Promise.resolve()), now: () => 1760000000000,
    onChange: (s) => phases.push(s.phase), onDone: (a) => { result.done = a; },
  });
  return { u, net, phases, result, storage };
}

// Seen red 2026-10-11 with the loop never taking the offset from a piece's answer: "three pieces,
// each where the last ended"; and with forgetUpload taken out of finish: "forgotten once done".
test("N1. a file goes up in pieces of the server's size, each where the last ended, remembered until done", async () => {
  const server = resumableServer(20 * MIB);
  const file = aFile("clip.mp4", 20 * MIB);
  const up = uploaderFor(server, file);
  up.u.start();
  await settle();
  assert.deepEqual(server.pieces.map((p) => [p.offset, p.len]), [[0, 8 * MIB], [8 * MIB, 8 * MIB], [16 * MIB, 4 * MIB]], "three pieces, each where the last ended");
  assert.ok(server.pieces.every((p) => p.token === "tok-1" && !p.inAddress), "the token in the header, never in the address");
  assert.deepEqual(up.result.done, { url: "/uploads/1760_clip.mp4", filename: "1760_clip.mp4", size: 20 * MIB, type: "video/mp4" });
  assert.equal(up.u.state.phase, "done");
  assert.ok(up.phases.includes("sending"));
  assert.equal(up.storage.getItem(upload.RESUME_KEY), "[]", "forgotten once done");
});

// Seen red 2026-10-11 with the network error path failing the upload instead of waiting and asking
// (fail(...) in the catch): "it carries on after the drop".
test("N2. a dropped connection: it waits, asks the server how far it got, and carries on from there", async () => {
  let drops = 1;
  const server = resumableServer(20 * MIB, {
    // The second piece arrives, then the connection drops before its answer.
    piece: async (c, off) => {
      if (off === 8 * MIB && drops-- > 0) { server.offset += bodyLen(c.init.body); throw new TypeError("Failed to fetch"); }
      return null;
    },
  });
  const waits = [];
  const up = uploaderFor(server, aFile("clip.mp4", 20 * MIB), { sleep: (ms) => { waits.push(ms); return Promise.resolve(); } });
  up.u.start();
  await settle();
  assert.equal(up.u.state.phase, "done", "it carries on after the drop");
  assert.ok(up.phases.includes("reconnecting"), "and says the connection dropped");
  assert.deepEqual(waits, [2000], "it waits first");
  assert.deepEqual(server.pieces.map((p) => p.offset), [0, 8 * MIB, 16 * MIB], "nothing sent twice: it asked where the server got");
});

// Seen red 2026-10-11 with pause() never setting pauseWanted: "'done' == 'paused'" (it went on to
// the end); and with rememberUpload taken out of begin(): "remembered with its session id while
// it goes". Round 3: Pause stops the piece on its way at once (round 2 waited for it), the record
// carries the file's fingerprint, and Resume first asks where the server got, since the stopped
// piece may have arrived after all (here it did). Seen red 2026-10-11 with the request's signal
// left off (the piece on its way never stopped): "paused at once, the piece on its way stopped /
// 'pausing' !== 'paused'".
test("N3. Pause stops the piece on its way, Resume asks where the server got and carries on", async () => {
  let release;
  const held = new Promise((r) => { release = r; });
  const server = resumableServer(20 * MIB, { piece: async (_c, off) => { if (off === 0) await held; return null; } });
  const up = uploaderFor(server, aFile("clip.mp4", 20 * MIB));
  up.u.start();
  await settle();
  const kept = JSON.parse(up.storage.getItem(upload.RESUME_KEY));
  assert.deepEqual(kept, [{ owner: ME, id: ID, name: "clip.mp4", size: 20 * MIB, lastModified: 1700000000000, fp: fingerprintOf(new Uint8Array(20 * MIB)), at: 1760000000000 }], "remembered with its session id while it goes");
  up.u.pause();
  await settle();
  assert.equal(up.u.state.phase, "paused", "paused at once, the piece on its way stopped");
  release();
  await settle();
  assert.equal(up.u.state.phase, "paused");
  assert.equal(server.pieces.length, 1, "no piece after Pause");
  const gets = () => up.net.calls.filter((c) => c.init.method === "GET").length;
  const before = gets();
  up.u.resume();
  await settle();
  assert.equal(up.u.state.phase, "done");
  assert.ok(gets() > before, "Resume asked the server where it got");
  assert.deepEqual(server.pieces.map((p) => p.offset), [0, 8 * MIB, 16 * MIB]);
});

// The uploads list's controls (round 3: each upload in the list names its parts by data attributes).
const upPart = (doc, part) => byId(doc, "media-uploads").querySelector("[data-upload-" + part + "]");
const upButton = (doc, action) => byId(doc, "media-uploads").querySelector('[data-upload-action="' + action + '"]');

// Seen red 2026-10-11 with the 429 branch treated as a plain failure: "and when it resets, here"
// (the words were the relay's, the time and Resume missing). Round 3 (R3-6): the wait is the
// server's own `resets_in_s` and the upload carries on by itself after it; Pause and Cancel stay.
// Seen red 2026-10-11 with the wait worked out from `resets_at` against this device's clock again:
// "and when it resets, here".
test("N4. the role's allowance (429): the relay's words, when it resets, and it carries on by itself", async () => {
  let refuse = true;
  const words = "Your role may upload 100 MB a day on this server, and you have uploaded 96 MB today, and this file would pass that. The allowance resets at midnight UTC, in 5 hours 12 minutes.";
  const server = resumableServer(20 * MIB, {
    piece: async (_c, off) => (refuse && off === 8 * MIB ? respond(429, { error: words, allowance: "day", resets_at: 1, resets_in_s: 18720, left_bytes: 0, offset: 8 * MIB, size: 20 * MIB }) : null),
  });
  const waits = [];
  const wakes = [];
  const { doc } = await openPage({
    sleep: (ms) => { waits.push(ms); return new Promise((r) => wakes.push(r)); },
    routes: listRoutes(listAnswer([]), [...server.routes, [at("/api/media", "POST"), () => respond(200, { ok: true, item: item() })]]),
  });
  await pickFile(doc, aFile("clip.mp4", 20 * MIB));
  byId(doc, "media-form-go").click();
  await settle();
  const said = text(upPart(doc, "words"));
  const resetsAt = 1760000000000 + 18720 * 1000;
  assert.ok(said.startsWith(words), "the allowance, in the relay's words");
  assert.ok(said.includes(media.resetsWords(resetsAt)) && media.resetsWords(resetsAt).startsWith("That is "), "and when it resets, here");
  assert.ok(said.endsWith(media.MEDIA_WORDS.uploadAllowanceThen), "and that it carries on by itself");
  assert.deepEqual(waits, [18720 * 1000], "it waits the server's own seconds");
  assert.equal(upPart(doc, "bar").getAttribute("aria-valuenow"), "40", "the bar where it stopped");
  assert.ok(upButton(doc, "pause") && upButton(doc, "cancel"), "Pause and Cancel while it waits");
  refuse = false;
  wakes.shift()();
  await settle();
  assert.equal(text(byId(doc, "media-notice").querySelector(".media-notice-text")), media.MEDIA_WORDS.published, "uploaded and published once allowed");
});

// Seen red 2026-10-11 with the 507 branch keeping the old offset (so Retry sent the last piece
// again before the empty one): "Retry sends only the empty piece at the end, nothing again".
test("N5. no room at the finish (507): the relay's words, and Retry sends the empty piece that finishes it", async () => {
  let full = true;
  const words = "There is no room left in this server's Media space for it. The file you sent is kept here for a day, so once there is room it can be finished without sending it again.";
  const server = resumableServer(20 * MIB, {
    finish: async () => (full ? respond(507, words) : null),
    piece: async (c, off) => {
      // The retry: an empty piece at the end tries the finish again.
      if (off === server.size && bodyLen(c.init.body) === 0 && !full) { server.done = { url: "/uploads/1760_clip.mp4", filename: "1760_clip.mp4", size: server.size, type: "video/mp4" }; return respond(200, server.done); }
      return null;
    },
  });
  const { doc, net } = await openPage({ routes: listRoutes(listAnswer([]), [...server.routes, [at("/api/media", "POST"), () => respond(200, { ok: true, item: item() })]]) });
  await pickFile(doc, aFile("clip.mp4", 20 * MIB));
  byId(doc, "media-form-go").click();
  await settle();
  assert.equal(text(upPart(doc, "words")), words);
  full = false;
  const sentBefore = server.pieces.length;
  upButton(doc, "retry").click();
  await settle();
  assert.deepEqual(server.pieces.slice(sentBefore).map((x) => [x.offset, x.len]), [[20 * MIB, 0]], "Retry sends only the empty piece at the end, nothing again");
  assert.ok(net.calls.find((c) => c.url === "/api/media" && c.init.method === "POST"), "then publishes");
});

// Seen red 2026-10-11 with status() ignoring `done` (the page then sent the empty piece to find
// out): "nothing sent again".
test("N6. the last piece's answer lost on the way: the server's `done` is taken and nothing is sent again", async () => {
  let lose = true;
  const server = resumableServer(10 * MIB, {
    piece: async (c, off) => {
      if (off === 8 * MIB && lose) {
        lose = false;
        server.offset = server.size;
        server.done = { url: "/uploads/1760_clip.mp4", filename: "1760_clip.mp4", size: server.size, type: "video/mp4" };
        throw new TypeError("Failed to fetch");
      }
      return null;
    },
  });
  const up = uploaderFor(server, aFile("clip.mp4", 10 * MIB));
  up.u.start();
  await settle();
  assert.equal(up.u.state.phase, "done", "an answer lost on the way: the server's done is taken");
  assert.equal(up.result.done.filename, "1760_clip.mp4");
  assert.equal(server.pieces.length, 2, "nothing sent again");
});

// Seen red 2026-10-11 with findUpload comparing only the owner and the name: "a different file of
// the same name starts afresh"; and with openAddForm counting the upload's own bytes in progress
// on top of themselves: "its own bytes in progress are not counted twice".
test("N7. after a reload, picking the same file carries on; another file of that name starts afresh; storage that refuses changes nothing else", async () => {
  // Round 3: a record carries the file's fingerprint (its first and last megabyte).
  const remembered = JSON.stringify([{ owner: ME, id: ID, name: "clip.mp4", size: 20 * MIB, lastModified: 1700000000000, fp: fingerprintOf(new Uint8Array(20 * MIB)), at: 1760000000000 - 3600000 }]);
  // Another file of the same name (another size, changed since): a new upload, the old one left be.
  const elsewhere = fakeStorage();
  elsewhere.setItem(upload.RESUME_KEY, remembered);
  const other = resumableServer(4 * MIB);
  const up = uploaderFor(other, aFile("clip.mp4", 4 * MIB, "video/mp4", 1700000099999), { storage: elsewhere });
  up.u.start();
  await settle();
  assert.equal(other.starts, 1, "a different file of the same name starts afresh");
  assert.equal(other.pieces[0].offset, 0);
  // Someone else's upload of the same file on this browser is not carried on either.
  const theirs = fakeStorage();
  theirs.setItem(upload.RESUME_KEY, remembered.replace(ME, ANN));
  const mineToo = resumableServer(20 * MIB);
  const fresh = uploaderFor(mineToo, aFile("clip.mp4", 20 * MIB), { storage: theirs });
  fresh.u.start();
  await settle();
  assert.equal(mineToo.starts, 1, "only the person's own upload is carried on");
  // The same file after a reload: carried on from where the server got.
  const storage = fakeStorage();
  storage.setItem(upload.RESUME_KEY, remembered);
  const server = resumableServer(20 * MIB);
  server.offset = 8 * MIB; // what arrived before the reload
  const { doc } = await openPage({ storage, routes: listRoutes(listAnswer([]), [
    ...server.routes,
    // 480 MB used, and this very upload (20 MB) counted as in progress: 32 MB left for it, which
    // it fits; counted twice, 12 MB would be left and it would be stopped.
    [at("/api/media/mine", "POST"), () => respond(200, mineAnswer({ used_bytes: 480 * MIB, in_progress_bytes: 20 * MIB }))],
    [at("/api/media", "POST"), () => respond(200, { ok: true, item: item() })],
  ]) });
  await pickFile(doc, aFile("clip.mp4", 20 * MIB));
  assert.equal(byId(doc, "media-form-stop"), null, "its own bytes in progress are not counted twice");
  byId(doc, "media-form-go").click();
  await settle();
  assert.equal(server.starts, 0, "the same file carries on");
  assert.equal(server.pieces[0].offset, 8 * MIB, "from where the server got");
  assert.equal(text(byId(doc, "media-notice").querySelector(".media-notice-text")), media.MEDIA_WORDS.published);
  // A browser that refuses storage: the upload still goes, only the resume is lost.
  const third = resumableServer(4 * MIB);
  const refused = uploaderFor(third, aFile("x.mp4", 4 * MIB), { storage: fakeStorage(true) });
  refused.u.start();
  await settle();
  assert.equal(refused.u.state.phase, "done");
});

// Seen red 2026-10-11 with closeForm leaving the upload running (no cancel): "Cancel deletes the
// upload on the server". Round 3: the form moves into the uploads list when its upload starts
// (its ids take the upload's number), and its parts are found there. Seen red 2026-10-11 with the
// form left where it was (round 2's startUpload): "the upload's bar is in the list".
test("N8. the page shows the progress with Pause and Resume, and Cancel deletes the upload on the server", async () => {
  let release;
  const held = new Promise((r) => { release = r; });
  const server = resumableServer(20 * MIB, { piece: async (_c, off) => { if (off === 8 * MIB) await held; return null; } });
  const { doc } = await openPage({ routes: listRoutes(listAnswer([]), server.routes) });
  await pickFile(doc, aFile("clip.mp4", 20 * MIB));
  byId(doc, "media-form-title").value = "Typed while it goes";
  byId(doc, "media-form-title").dispatch("input");
  byId(doc, "media-form-go").click();
  await settle();
  assert.ok(upPart(doc, "bar"), "the upload's bar is in the list");
  assert.equal(upPart(doc, "bar").getAttribute("aria-valuenow"), "40");
  assert.equal(text(upPart(doc, "words")), "8 MB of 20 MB sent.");
  const title = byId(doc, "media-form-title-1");
  assert.ok(title, "the form went into the uploads list with what was typed");
  assert.equal(title.value, "Typed while it goes");
  upButton(doc, "pause").click();
  release();
  await settle();
  assert.equal(byId(doc, "media-form-title-1"), title, "the fields are not drawn again while it goes");
  assert.ok(text(upPart(doc, "words")).endsWith(media.MEDIA_WORDS.uploadPaused));
  assert.ok(upButton(doc, "resume"));
  upButton(doc, "cancel").click();
  await settle();
  assert.equal(server.deleted, 1, "Cancel deletes the upload on the server");
  assert.equal(byId(doc, "media-uploads").querySelector("[data-upload]"), null, "and it leaves the list");
  assert.equal(byId(doc, "media-form-go"), null);
});

// ── O. A phone's width ────────────────────────────────────────────────────

// Seen red 2026-10-11 against the round 1 media.css (no max-width on .media-select): "a select is
// held to the width it has".
test("O. a long category name cannot widen the filters past a phone's width", () => {
  const css = fs.readFileSync(path.join(WEB, "shared", "media.css"), "utf8");
  const rules = [...css.matchAll(/([^{}]*\.media-select[^{}]*)\{([^}]*)\}/g)].map((m) => m[2]).join(";");
  assert.ok(/max-width:\s*100%/.test(rules), "a select is held to the width it has");
});
