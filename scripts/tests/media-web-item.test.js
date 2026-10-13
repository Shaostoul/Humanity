// The Media item's own page and its Edit form on the website (2026-10-12): docs/design/
// media-round-5-plan.md, lane 2A ("web item", section 5, Wave 2), built to sections 3.2 (one
// item's wire), 3.4 (the writes), 3.5 (downloads and views) and 3.10 (the website), with the
// orchestrator's decision 1 in section 9 and the operator's answers in section 10. The web half of
// the desktop's lane 1D: these tests mirror src/gui/pages/media/item_tests.rs one for one.
//
// Run: node --test scripts/tests/media-web-item.test.js
//
// Against the stand-ins in scripts/tests/media-web-harness.js (a document that holds only what the
// page built, where reading or setting innerHTML throws; a network that records every request;
// signing that returns "sig:" plus the words signed, so a request shows exactly which purpose its
// signature covers). The items, the licences and the edit's exact wire come from the shared
// fixture the relay makes, scripts/tests/fixtures/media-round5-v1.json, which this file only reads.
// HOS_WEB_DIR points the test at another copy of web/ (used to see each test red).
//
// What it proves:
//  1. A tile opens the item's page with its whole description, its files (each with Download), its
//     licence and its counts, and counts a view.
//  2. A card shows the description's first line (cut at a word, at most 120 characters) and "2 files".
//  3. Picking a Creative Commons licence in Edit shows its warning above Save; on a members-only
//     item the second fact too; the game tick only with a licence that allows it.
//  4. An admin sees another person's licence and game tick but cannot change them ("Only Ann can
//     change these."), and the edit sends them as they were; a moderator gets no Edit.
//  5. Add a file uploads (the resumable upload), then attaches it to the item (files/add, signed
//     over media_file_add:<id>:<upload>:file), and never publishes it.
//  6. Copy link copies https://<host>/media/<id>.
//  7. A Download goes through the counting route, /api/media/download/<id>?file=<n>; a members-only
//     one through a ticket made for that route and that file.
//  8. The edit is signed over the BLAKE3 of its text exactly as sent: the fixture's purpose, and
//     the fixture's body byte for byte.
//
// Seen red 2026-10-12: each with one break made in a copy of web/ under HOS_WEB_DIR, then green
// again on the real web/: see the note above each test.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const H = require("./media-web-harness.js");
const { ROOT, WEB, ME, ANN, ORIGIN, byId, text, everyElement, respond, at, path0, fakeSession, settle, listAnswer, listRoutes, openPage, resumableRelay, aFile } = H;
const MI = require(path.join(WEB, "pages", "media-item.js"));
const ED = require(path.join(WEB, "pages", "media-edit.js"));
const B3 = require(path.join(WEB, "shared", "blake3.js"));

const FX = JSON.parse(fs.readFileSync(path.join(ROOT, "scripts", "tests", "fixtures", "media-round5-v1.json"), "utf8"));
const CATS = FX.list_by_downloads.categories;
const L = FX.licences;

/**
 * The fixture's item, unlabelled (a label keeps its cover over the page until clicked) and Ann's
 * (the fixture's stand-in key is this page's own, ME), changed by `over`.
 */
const fxItem = (over) => Object.assign({}, FX.item_members_view, { labels: [], uploader_key: ANN }, over || {});
/** The list's answer with the server's categories and licences. */
const answerOf = (items) => listAnswer(items, { categories: CATS, licences: L });
const tileOf = (doc, id) => byId(doc, "media-grid").querySelectorAll(".media-tile").find((t) => t.dataset.id === String(id));
const notice = (doc) => text(byId(doc, "media-notice").querySelector(".media-notice-text"));
const choose = (el, value) => { el.value = value; el.dispatch("change"); };
const tick = (el, on) => { el.checked = on; el.dispatch("change"); };
/** Settle until `done()` holds (the resumable upload takes several turns). */
async function until(done, turns) {
  for (let i = 0; i < (turns || 40) && !done(); i++) await settle();
}

// ── 1. The item's page ───────────────────────────────────────────────────

// Seen red 2026-10-12 with the description left out of media-item.js renderInfo (its
// `media-item-description` line taken out): "the whole description, its second line too / actual:
// null / expected: 'First line\nThe rest'".
test("1. a tile opens the item with its description, files, licence and counts, and counts a view", async () => {
  const { doc, net } = await openPage({ routes: listRoutes(answerOf([fxItem()])) });
  tileOf(doc, 1).querySelector(".media-thumb").click();
  await settle();
  assert.ok(byId(doc, "media-viewer-open"), "the click opens the item's page");
  const viewed = net.calls.find((c) => c.url === "/api/media/viewed");
  assert.deepEqual(viewed && [viewed.body, viewed.headers["X-Upload-Token"]], [{ id: 1 }, undefined], "and counts a view (a public item's without the token)");
  assert.equal(text(byId(doc, "media-item-description")), "First line\nThe rest", "the whole description, its second line too");
  assert.equal(text(byId(doc, "media-item-by")), "by Ann in Fire flow arts");
  assert.equal(text(byId(doc, "media-item-counts")), "1 pick, 5 views, 3 downloads");
  const licence = byId(doc, "media-item-licence");
  assert.equal(text(licence), "Licence: CC BY 4.0");
  assert.equal(licence.querySelector("a").href, L.offered[1].url, "linked to its deed");
  assert.equal(text(byId(doc, "media-item-licence-words")), L.offered[1].line, "with the server's own line for it");
  assert.equal(text(byId(doc, "media-item-game")), MI.WORDS.offeredForGame);
  const rows = byId(doc, "media-item-files").querySelectorAll(".media-item-file-name").map(text);
  assert.deepEqual(rows.map((r) => r.split(", ").slice(0, 2).join(", ")), ["dusk.png, picture", "textures.zip, file", "1762_c.jpg, cover picture"]);
  assert.equal(byId(doc, "media-item-files").querySelectorAll("[data-download]").length, 3, "each file has its own Download");
  assert.ok(byId(doc, "media-copy-link"), "Copy link for anyone");
  assert.equal(byId(doc, "media-item-edit-open"), null, "no Edit on someone else's item");
  const actions = byId(doc, "media-viewer-open").querySelector(".media-tile-actions");
  assert.deepEqual(actions.children.map((b) => b.dataset.action), ["unpick", "report"], "a member's own actions on it");
});

// ── 2. The card ──────────────────────────────────────────────────────────

// Seen red 2026-10-12 with tile() putting the whole description on the card (`text:
// item.description`): "the second line stays on the page / actual: 'First line\nThe rest'".
test("2. the card shows the first line and the file count", async () => {
  const long = "A slow contact staff spin at dusk on the beach, with the tide coming in and the last of the light on the water behind the dancer";
  assert.ok(long.length > MI.CARD_LINE_MAX);
  const cut = MI.firstLine(long + "\nThe second line stays on the page");
  assert.ok(cut.length <= MI.CARD_LINE_MAX && cut.endsWith("..."), cut);
  const kept = cut.slice(0, -3);
  assert.ok(long.startsWith(kept) && long[kept.length] === " ", "cut at a word: " + cut);
  assert.equal(MI.firstLine("  Short one\nmore"), "Short one");

  const { doc } = await openPage({ routes: listRoutes(answerOf([fxItem()])) });
  const card = tileOf(doc, 1);
  assert.equal(text(card.querySelector(".media-tile-line")), "First line", "the second line stays on the page");
  assert.equal(text(card.querySelector(".media-tile-meta")), "by Ann · 1 pick, 3 downloads · 2 files");
  assert.equal(card.querySelector("img").src, "/uploads/1762_c.jpg", "the cover first");
});

// ── 3. The Creative Commons warning ──────────────────────────────────────

// Seen red 2026-10-12 with media-edit.js licenceChoice never drawing `cc_warning`: "the warning
// shows once CC BY 4.0 is picked / actual: null".
test("3. a Creative Commons licence shows its warning before Save", async () => {
  const mine = fxItem({ uploader_key: ME, uploader_name: "Me", licence: "all_rights_reserved", project_use: false });
  const { doc, page } = await openPage({ routes: listRoutes(answerOf([mine])) });
  page.openViewer(page.state.items[0]);
  byId(doc, "media-item-edit-open").click();
  const edit = byId(doc, "media-edit");
  assert.ok(edit, "Edit opens its form");
  assert.equal(edit.querySelector("[data-cc-warning]"), null, "All rights reserved: no warning");
  assert.equal(text(edit.querySelector(".media-hosting-heading")), "Your uploads stay yours.", "the hosting paragraph, in the server's words");
  choose(edit.querySelector("[data-licence]"), "cc_by_4");
  const warning = edit.querySelector("[data-cc-warning]");
  assert.equal(text(warning), L.cc_warning, "the warning shows once CC BY 4.0 is picked");
  const order = everyElement(doc);
  assert.ok(order.indexOf(warning) < order.indexOf(byId(doc, "media-edit-save")), "it shows before Save");
  assert.equal(edit.querySelector("[data-cc-members-warning]"), null, "a public item: not the members' fact");
  assert.ok(edit.querySelector("[data-game-offer]"), "CC BY may be offered for the game");
  tick(byId(doc, "media-edit-vis-members"), true);
  assert.equal(text(edit.querySelector("[data-cc-members-warning]")), L.cc_members_warning, "members only: every member who receives it may republish it");
  choose(edit.querySelector("[data-licence]"), "cc_by_nc_4");
  assert.equal(edit.querySelector("[data-game-offer]"), null, "NonCommercial: not for the game");
});

// ── 4. An admin and someone else's licence ───────────────────────────────

// Seen red 2026-10-12 with createEditForm passing `ownerOnly: null` for everyone: "an admin is told
// who can change them / actual: null".
test("4. an admin sees the licence but cannot change it; a moderator gets no Edit", async () => {
  const anns = fxItem({ uploader_key: ANN });
  const mod = await openPage({ session: fakeSession({ role: "mod" }), routes: listRoutes(answerOf([anns])) });
  mod.page.openViewer(mod.page.state.items[0]);
  assert.equal(byId(mod.doc, "media-item-edit-open"), null, "a moderator edits nothing");
  assert.equal(tileOf(mod.doc, 1).querySelector('[data-action="edit"]'), null, "nor from the card");

  const { doc, page, net } = await openPage({ session: fakeSession({ role: "admin" }), routes: listRoutes(answerOf([anns]), [
    [at("/api/media/edit", "POST"), () => respond(200, { ok: true, item: anns })],
  ]) });
  assert.ok(tileOf(doc, 1).querySelector('[data-action="edit"]'), "an admin may edit the words, from the card too");
  page.openViewer(page.state.items[0]);
  byId(doc, "media-item-edit-open").click();
  const edit = byId(doc, "media-edit");
  assert.equal(text(edit.querySelector("[data-owner-only]")), "Only Ann can change these.", "an admin is told who can change them");
  assert.equal(edit.querySelector("[data-licence]"), null, "no list to pick from");
  assert.equal(text(edit.querySelector("[data-licence-name]")), "CC BY 4.0", "the licence, shown");
  const game = edit.querySelector("[data-game-offer]");
  assert.equal(game.disabled, true, "the tick does not move");
  tick(game, false);
  byId(doc, "media-edit-title").value = "Dusk again";
  byId(doc, "media-edit-title").dispatch("input");
  byId(doc, "media-edit-save").click();
  await settle();
  const sent = net.calls.find((c) => c.url === "/api/media/edit");
  assert.ok(sent, "the edit goes");
  assert.deepEqual([sent.body.edit.title, sent.body.edit.licence, sent.body.edit.project_use], ["Dusk again", "cc_by_4", true], "the words changed, the licence and tick as they were");
});

// ── 5. Add a file ────────────────────────────────────────────────────────

// Seen red 2026-10-12 with startUpload's onDone publishing every upload (`publish(f)`, the attach
// left out): "added, never published / actual: undefined" (a POST /api/media, and no files/add).
test("5. adding a file uploads, then attaches it; it is never published", async () => {
  const mine = fxItem({ uploader_key: ME, uploader_name: "Me" });
  const relay = resumableRelay();
  const added = Object.assign({}, mine, { file_count: 3, files: mine.files.concat([{ file: 5, filename: "1760_clip.webm", original_name: "clip.webm", kind: "video", size_bytes: 20000, url: "/uploads/1760_clip.webm" }]) });
  const { doc, page, net } = await openPage({ routes: listRoutes(answerOf([mine]), relay.routes.concat([
    [at("/api/media/files/add", "POST"), () => respond(200, { ok: true, item: added })],
  ])) });
  page.openViewer(page.state.items[0]);
  byId(doc, "media-item-edit-open").click();
  const input = byId(doc, "media-edit-files").querySelector('[data-attach-input="file"]');
  input.files = [aFile("clip.webm", 20000, { type: "video/webm" })];
  input.dispatch("change");
  await until(() => net.calls.some((c) => c.url === "/api/media/files/add"));
  const asked = net.calls.map((c) => (c.init.method || "GET") + " " + path0(c.url));
  const add = net.calls.find((c) => c.url === "/api/media/files/add");
  assert.ok(add, "added, never published: " + asked.join(", "));
  assert.ok(relay.log.some((l) => l.kind === "start" && l.c.body.media === 1), "the same resumable upload as Add to Media's, on the Media shelf");
  assert.deepEqual([add.body.id, add.body.filename, add.body.add, add.body.as, add.body.sig], [1, "1760_x.png", "1760_clip.webm", "file", "sig:" + "media_file_add:1:1760_clip.webm:file"]);
  assert.ok(!net.calls.some((c) => c.url === "/api/media" && (c.init.method || "GET") === "POST"), "never published: " + asked.join(", "));
  await settle();
  assert.equal(notice(doc), 'Added to "Dusk".');
  const rows = byId(doc, "media-item-files").querySelectorAll(".media-item-file-name").map(text);
  assert.ok(rows.some((r) => r.startsWith("clip.webm, video")), "the answer's item is the page's: " + rows.join(" | "));
});

// ── 6. Copy link ─────────────────────────────────────────────────────────

// Seen red 2026-10-12 with itemLink making the old address (`/media?id=`): "the item's own page /
// actual: 'https://united-humanity.us/media?id=12'".
test("6. Copy link is https://<host>/media/<id>", async () => {
  assert.equal(MI.itemLink("https://united-humanity.us", 12), "https://united-humanity.us/media/12", "the item's own page");
  const copied = [];
  const had = Object.getOwnPropertyDescriptor(globalThis.navigator, "clipboard");
  Object.defineProperty(globalThis.navigator, "clipboard", { value: { writeText: async (t) => copied.push(t) }, configurable: true });
  try {
    const { doc, page } = await openPage({ routes: listRoutes(answerOf([fxItem()])) });
    page.openViewer(page.state.items[0]);
    byId(doc, "media-copy-link").click();
    assert.deepEqual(copied, [ORIGIN + "/media/1"], "copied");
    assert.equal(text(byId(doc, "media-copied")), "Copied: " + ORIGIN + "/media/1", "and shown");
  } finally {
    if (had) Object.defineProperty(globalThis.navigator, "clipboard", had); else delete globalThis.navigator.clipboard;
  }
});

// ── 7. Downloads are counted ─────────────────────────────────────────────

// Seen red 2026-10-12 with download() saving a public file from its own address (`f.url`): "the
// counted route / actual: [ { url: '/uploads/1761_y.zip', ... } ]".
test("7. a download goes through the counting route", async () => {
  const { doc, page, saved, net } = await openPage({ routes: listRoutes(answerOf([fxItem()])) });
  page.openViewer(page.state.items[0]);
  byId(doc, "media-item-files").querySelector('[data-download="1"]').click();
  await settle();
  assert.deepEqual(saved, [{ url: "/api/media/download/1?file=1", name: "textures.zip" }], "the counted route");
  assert.ok(!net.calls.some((c) => c.url.startsWith("/uploads/")), "never the file's own address");

  const members = Object.assign({}, FX.item_members_only, { labels: [] });
  const m = await openPage({ routes: listRoutes(answerOf([members]), [
    [at("/api/media/file/1"), () => respond(200, "", new Uint8Array([1]))],
    [at("/api/media/ticket", "POST"), (c) => respond(200, { url: "/api/media/download/1?file=" + c.body.file + "&ticket=t1", expires_in_s: 600 })],
  ]) });
  m.page.openViewer(m.page.state.items[0]);
  byId(m.doc, "media-item-files").querySelector('[data-download="1"]').click();
  await settle();
  const ticket = m.net.calls.find((c) => c.url === "/api/media/ticket");
  assert.deepEqual([ticket.body, ticket.headers["X-Upload-Token"]], [{ id: 1, filename: "1760_x.png", file: 1, download: true }, "tok-1"], "a members-only file: a ticket for the counted route, for that file");
  assert.deepEqual(m.saved, [{ url: "/api/media/download/1?file=1&ticket=t1", name: "textures.zip" }]);
});

// ── 8. The edit's signature ──────────────────────────────────────────────

// Seen red 2026-10-12 with editBody writing the edit again (`JSON.stringify(Object.assign({key,
// timestamp, sig, id, filename}, {edit: JSON.parse(text)}))` with the edit's keys in the form's
// order, title first): "the edit is sent exactly as it was hashed".
test("8. the edit is signed over the exact text sent", async () => {
  assert.equal(ED.editPurpose(FX.edit.id, FX.edit.text), FX.edit.purpose, "the fixture's purpose, from its text");
  const mine = fxItem({ uploader_key: ME });
  const { doc, page, net } = await openPage({ routes: listRoutes(answerOf([mine]), [
    [at("/api/media/edit", "POST"), () => respond(200, { ok: true, item: mine })],
  ]) });
  page.openViewer(page.state.items[0]);
  byId(doc, "media-item-edit-open").click();
  // The form shows the fixture's own edit: Dusk, its two lines, Fire flow arts, flashing, CC BY, offered.
  byId(doc, "media-edit-labels").querySelector('[data-label="flashing"]').checked = true;
  byId(doc, "media-edit-labels").querySelector('[data-label="flashing"]').dispatch("change");
  byId(doc, "media-edit-save").click();
  await settle();
  const sent = net.calls.find((c) => c.url === "/api/media/edit");
  const raw = sent.init.body;
  assert.equal(typeof raw, "string");
  const editText = raw.slice(raw.indexOf('"edit":') + '"edit":'.length, -1);
  const purpose = sent.body.sig.slice("sig:".length);
  assert.equal("media_edit:1:" + B3.toHex(B3.hash(new TextEncoder().encode(editText))), purpose, "the edit is sent exactly as it was hashed");
  assert.equal(editText, FX.edit.text, "the fixture's edit text");
  assert.equal(purpose, FX.edit.purpose, "signed over the fixture's purpose");
  const want = FX.edit.body.replace('"timestamp":' + FX.edit.timestamp, '"timestamp":' + sent.body.timestamp).replace('"sig":"SIG"', '"sig":"sig:' + FX.edit.purpose + '"');
  assert.equal(raw, want, "the fixture's body, byte for byte");
  await settle();
  assert.equal(byId(doc, "media-edit"), null, "Save closes the form");
  assert.equal(text(byId(doc, "media-item-status")), MI.WORDS.saved);
});
