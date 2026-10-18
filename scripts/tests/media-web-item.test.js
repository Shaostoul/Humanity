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
//  9. View in 3D (2026-10-16, docs/design/model-viewer.md) is beside the Download of a .glb or .stl
//     file only, fetches nothing until pressed, then fetches the file's own address (never the
//     counting route), and its view takes the cover's place until Close the 3D view.
// 10. The protected setup leaves out another person's items the way the desktop does (tiles.rs
//     show_of, Show::Hidden): under its pictures rule, or for a content label. A hidden item shows
//     the setup's line on its tile and its page, offers no View in 3D, and a 3D view open when the
//     setup comes on closes. Your own items are never hidden.
// 11. (2026-10-17) Under each file row whose kind deserves a word (an archive, a Blender file), that
//     word from /shared/file-risk.js, by the file's name: the list asked for once, no file read.
// 12. (2026-10-17) Each file row starts with its type's badge (a grey "File" until the list is in,
//     then exactly what HosFileRisk.badgeHtml draws), and says "name, size".
// 13. (2026-10-17) Edit is in three tinted parts, "About it", "Who can see it" and "Files", each
//     holding its own fields, with Save and Cancel under them; its file rows show their types.
// 14. (2026-10-17) Every file row in Edit has Rename, the item's own file and the cover included;
//     its box holds the name without its ending, the ending after it, one row at a time.
// 15. (2026-10-17) Save name (or Enter) sends files/rename with the ending put back and both stored
//     names, signed over the BLAKE3 of the name as sent (held to another BLAKE3); then "Renamed.".
// 16. (2026-10-17) Use as cover is the uploader's, on a picture that is neither the item's own file
//     nor the cover; it sends files/cover and the page says "That picture is the cover now."
// 17. (2026-10-17) A refused rename or cover shows the relay's words in Edit's problem line, and a
//     refused rename keeps its box.
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
  // Since 2026-10-17 each row's badge says what kind of file it is (test 12), so the row itself says
  // only "name, size" ("name, cover picture, size" for the cover), as the desktop's does.
  const rows = byId(doc, "media-item-files").querySelectorAll(".media-item-file-name").map(text);
  // The cover by the name it was uploaded with (the relay sends a cover's original_name since
  // 2026-10-17, and the fixture carries it).
  assert.deepEqual(rows, ["dusk.png, 1 KB", "textures.zip, 4 KB", "cover.jpg, cover picture, 5 KB"]);
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
  assert.ok(rows.includes("clip.webm, 20 KB"), "the answer's item is the page's: " + rows.join(" | "));
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

// ── 9. View in 3D ────────────────────────────────────────────────────────

/** A stand-in for /shared/model-view.js's open (Node has no WebGL): it records what it was asked and fetches the bytes as the real viewer does. */
function modelViewStandIn() {
  const opened = [];
  return {
    opened,
    open(container, o) {
      const rec = { container, o, closed: false, bytes: null, error: null };
      rec.done = o.fetchBytes(undefined).then((b) => { rec.bytes = b; }, (e) => { rec.error = e; });
      opened.push(rec);
      return { close() { rec.closed = true; } };
    },
  };
}
const MIB = 1024 * 1024;
/** An item whose files are a .blend, a .glb and an .stl (its cover the fixture's picture). */
function modelItem(over, url) {
  const at = url || ((n, name) => "/uploads/" + name);
  return fxItem(Object.assign({
    kind: "model", url: at(0, "1760_ship.blend"), filename: "1760_ship.blend", original_name: "ship.blend", size_bytes: 5000, file_count: 3,
    files: [
      { file: 0, filename: "1760_ship.blend", original_name: "ship.blend", kind: "model", size_bytes: 5000, url: at(0, "1760_ship.blend") },
      { file: 1, filename: "1761_ship.glb", original_name: "ship.glb", kind: "model", size_bytes: 2224, url: at(1, "1761_ship.glb") },
      { file: 3, filename: "1763_part.stl", original_name: "Part.STL", kind: "model", size_bytes: 3 * MIB, url: at(3, "1763_part.stl") },
    ],
  }, over || {}));
}
const isModelFetch = (c) => /\.(glb|stl)$/i.test(path0(c.url)) || /^\/api\/media\/file\//.test(path0(c.url));

// Seen red 2026-10-16 twice: with modelBytes() fetching through the counting route
// (`env.fetch('/api/media/download/1?file=' + f.file, init)`), the viewer's fetch went there:
// "Error: no route in the test for /api/media/download/1?file=1"; with the button offered on every
// file (`modelName` answering any name), "the .glb and .stl rows only (not the .blend, not the cover)".
test("9. View in 3D is beside a .glb or .stl file's Download, and fetches its own address only when pressed", async () => {
  const MV = require(path.join(WEB, "shared", "model-view.js"));
  const glb = fs.readFileSync(path.join(ROOT, "scripts", "tests", "fixtures", "model-cube.glb"));
  const viewer = modelViewStandIn();
  const { doc, page, net } = await openPage({ modelView: viewer, routes: listRoutes(answerOf([modelItem()]), [
    [at("/uploads/1761_ship.glb"), () => respond(200, "", new Uint8Array(glb))],
  ]) });
  page.openViewer(page.state.items[0]);
  await settle();
  const files = byId(doc, "media-item-files");
  const buttons = files.querySelectorAll("[data-view3d]");
  assert.deepEqual(buttons.map((b) => b.dataset.view3d), ["1", "3"], "the .glb and .stl rows only (not the .blend, not the cover)");
  assert.deepEqual(buttons.map(text), ["View in 3D", "View in 3D"]);
  assert.equal(buttons[0].title, "Look at it here and turn it with the mouse. It is fetched from the server first (2 KB).", "the tooltip names its size");
  assert.equal(buttons[1].title, MV.fill(MV.WORDS.viewTip, { size: "3 MB" }));
  assert.ok(files.querySelector('[data-download="1"]'), "Download stays beside it");
  assert.equal(viewer.opened.length, 0, "nothing opened");
  assert.ok(!net.calls.some(isModelFetch), "nothing fetched before it is pressed: " + net.calls.map((c) => c.url).join(", "));
  assert.ok(byId(doc, "media-viewer-open").querySelector(".media-viewer-img"), "the cover is shown at the top");

  byId(doc, "media-view3d-1").click();
  await settle();
  assert.equal(viewer.opened.length, 1, "pressed: the viewer opens");
  const rec = viewer.opened[0];
  await rec.done;
  assert.equal(rec.error, null, String(rec.error));
  assert.deepEqual([rec.o.name, rec.o.fileName, rec.o.size], ["ship.glb", "ship.glb", 2224]);
  const asked = net.calls.map((c) => (c.init.method || "GET") + " " + c.url);
  assert.ok(net.calls.some((c) => c.url === "/uploads/1761_ship.glb"), "the file's own address: " + asked.join(", "));
  assert.ok(!net.calls.some((c) => c.url.startsWith("/api/media/download")), "never the counting route: " + asked.join(", "));
  assert.equal(rec.bytes.byteLength, glb.length, "its bytes, handed to the viewer");
  const box = byId(doc, "media-model");
  assert.ok(box && byId(doc, "media-viewer-open").children[0].children.includes(box), "the view takes the cover's place at the top");
  assert.equal(byId(doc, "media-viewer-open").querySelector(".media-viewer-img"), null, "the cover is gone meanwhile");
  assert.equal(text(byId(doc, "media-model-close")), "Close the 3D view");
  assert.equal(box.querySelector(".media-viewer-name").textContent, "ship.glb", "with the file's name");
  // The rest of the page drawn again (a reload of the item) keeps the same view.
  page.itemView.renderInfo();
  assert.equal(byId(doc, "media-model"), box, "the view stays while the page around it is drawn again");

  byId(doc, "media-model-close").click();
  assert.equal(rec.closed, true, "Close the 3D view closes the viewer");
  assert.equal(byId(doc, "media-model"), null);
  assert.ok(byId(doc, "media-viewer-open").querySelector(".media-viewer-img"), "and the cover is back");

  // A members-only item: the members' file route with the token in its header, exactly as a
  // picture is fetched; no ticket, no counting route.
  const members = modelItem({ visibility: "members", cover: null }, (n) => "/api/media/file/1?file=" + n);
  const v2 = modelViewStandIn();
  const m = await openPage({ modelView: v2, routes: listRoutes(answerOf([members]), [
    [at("/api/media/file/1"), () => respond(200, "", new Uint8Array(glb))],
  ]) });
  m.page.openViewer(m.page.state.items[0]);
  await settle();
  assert.ok(!m.net.calls.some(isModelFetch), "nothing fetched before it is pressed");
  byId(m.doc, "media-view3d-3").click();
  await v2.opened[0].done;
  const got = m.net.calls.filter(isModelFetch);
  assert.deepEqual(got.map((c) => [c.url, c.headers["X-Upload-Token"]]), [["/api/media/file/1?file=3", "tok-1"]], "the members' file route, the token in its header");
  assert.ok(!m.net.calls.some((c) => c.url === "/api/media/ticket" || c.url.startsWith("/api/media/download")), "no ticket, no counting route");
  assert.equal(v2.opened[0].o.fileName, "Part.STL", "read as STL");
  // Closing the item closes its view.
  m.page.closeViewer();
  assert.equal(v2.opened[0].closed, true, "closing the item closes the 3D view");
});

// ── 10. The protected setup ──────────────────────────────────────────────

const PROTECTED_KEY = "humanity_protected_setup";
const SAFETY = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "gui", "safety_presets.json"), "utf8"));
const PICTURE_LINE = SAFETY.presets.find((p) => p.id === "protected").picture_hidden_line;
const presetRoute = [at("/data/gui/safety_presets.json"), () => respond(200, SAFETY)];
/** This device's storage with the setup on and its pictures rule `pictures` ("never" or "click"). */
function setupOn(pictures) {
  const storage = H.fakeStorage();
  storage.setItem(PROTECTED_KEY, JSON.stringify({ on: true, pictures }));
  return storage;
}

// Seen red 2026-10-16 twice: with hiddenBy() answering null always (the rule not applied), "the
// preset's line where the cover was / actual: null"; with closeIfHidden() closing nothing, "the open
// view closes / actual: false".
test("10. the protected setup leaves out another person's model: no View in 3D, and an open view closes", async () => {
  assert.equal(MI.hiddenBy(null, modelItem(), ME, "x"), null, "the setup off: nothing is hidden");
  const anns = modelItem();
  const mine = modelItem({ id: 2, uploader_key: ME, uploader_name: "Me" });
  const viewer = modelViewStandIn();
  const { doc, page, net } = await openPage({ storage: setupOn("never"), modelView: viewer, routes: listRoutes(answerOf([anns, mine]), [presetRoute]) });
  await settle();
  const tile = tileOf(doc, 1);
  assert.equal(text(tile.querySelector("[data-hidden]")), PICTURE_LINE, "the preset's line where the cover was");
  assert.equal(tile.querySelector("img"), null, "no cover on Ann's tile");
  assert.equal(tileOf(doc, 2).querySelector("[data-hidden]"), null, "your own tile shows as usual");

  page.openViewer(page.state.items[0]);
  await settle();
  assert.equal(text(byId(doc, "media-viewer-hidden")), PICTURE_LINE, "the item's page says so where the cover was");
  assert.equal(byId(doc, "media-viewer-open").querySelector(".media-viewer-img"), null, "no cover on the page either");
  assert.equal(byId(doc, "media-item-files").querySelectorAll("[data-view3d]").length, 0, "no View in 3D: a model's textures are pictures too");
  assert.ok(byId(doc, "media-item-files").querySelector('[data-download="1"]'), "Download stays (as on the desktop)");
  page.openViewer(page.state.items[1]);
  await settle();
  assert.equal(byId(doc, "media-viewer-hidden"), null, "your own model is never hidden");
  assert.deepEqual(byId(doc, "media-item-files").querySelectorAll("[data-view3d]").map((b) => b.dataset.view3d), ["1", "3"]);
  assert.equal(viewer.opened.length, 0);
  assert.ok(!net.calls.some(isModelFetch), "nothing of the model fetched");

  // A 3D view open when the setup is turned on (in Chat, in another tab): it closes, and says hidden.
  const storage = H.fakeStorage();
  const v2 = modelViewStandIn();
  const m = await openPage({ storage, modelView: v2, routes: listRoutes(answerOf([modelItem()]), [presetRoute,
    [at("/uploads/1761_ship.glb"), () => respond(200, "", new Uint8Array([1]))]]) });
  m.page.openViewer(m.page.state.items[0]);
  await settle();
  byId(m.doc, "media-view3d-1").click();
  await settle();
  assert.ok(byId(m.doc, "media-model"), "open while the setup is off");
  storage.setItem(PROTECTED_KEY, JSON.stringify({ on: true, pictures: "never" }));
  m.page.itemView.protectionChanged();
  await settle();
  assert.equal(v2.opened[0].closed, true, "the open view closes");
  assert.equal(byId(m.doc, "media-model"), null, "and is gone from the page");
  assert.equal(text(byId(m.doc, "media-viewer-hidden")), PICTURE_LINE, "which says hidden where it was");
  assert.equal(byId(m.doc, "media-item-files").querySelectorAll("[data-view3d]").length, 0, "with no View in 3D");

  // The label rule: with pictures shown after a click, someone else's labelled item is still left
  // out (the desktop's LABEL_HIDDEN_LINE); your own labelled item is not.
  const l = await openPage({ storage: setupOn("click"), modelView: modelViewStandIn(), routes: listRoutes(answerOf([
    modelItem({ labels: ["spoiler"] }), modelItem({ id: 2, uploader_key: ME, labels: ["spoiler"] }), modelItem({ id: 3 }),
  ]), [presetRoute]) });
  assert.equal(text(tileOf(l.doc, 1).querySelector("[data-hidden]")), MI.LABEL_HIDDEN_LINE);
  assert.equal(MI.LABEL_HIDDEN_LINE, "Hidden: the protected setup leaves out items with a content label.");
  assert.equal(tileOf(l.doc, 2).querySelector("[data-hidden]"), null, "your own labelled item is not hidden");
  assert.equal(tileOf(l.doc, 3).querySelector("[data-hidden]"), null, "an unlabelled one shows when pictures come after a click");
});

// ── 11. What kind of file it is ──────────────────────────────────────────

// 2026-10-17 (/shared/file-risk.js, the Discord suggestion): under each file row whose kind deserves
// a word (an archive, a Blender file, a program, a script), that word, by the file's name: the page
// never reads a file before its Download. The list (data/safety/file_risks.json, the desktop app's
// too) is asked for once, the first time a file row is drawn before it is in, and the rows are drawn
// again when it comes. Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR with riskOf never
// asking for the list: "the list is asked for once, when the rows are first drawn"; with the line
// left out of renderInfo: "the archive's word, under its row".
test("11. each file row says what kind of file it is when that deserves a word, by its name, once the list is in", async () => {
  const FR = require(path.join(WEB, "shared", "file-risk.js"));
  const RISKS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "file_risks.json"), "utf8"));
  const riskOf = (name) => FR.assess(name, null, RISKS);
  FR.use(null); // not in yet: the page asks for it
  let asked = 0;
  const riskRoute = [at("/data/safety/file_risks.json"), () => { asked++; return respond(200, RISKS); }];
  const blend = { file: 3, filename: "1763_z.blend", kind: "file", original_name: "ship.blend", size_bytes: 5000, url: "/uploads/1763_z.blend" };
  const item = fxItem({ files: FX.item_members_view.files.concat([blend]) });
  const { doc, page, net } = await openPage({ routes: listRoutes(answerOf([item]), [riskRoute]) });
  page.openViewer(page.state.items[0]);
  await settle();
  assert.equal(asked, 1, "the list is asked for once, when the rows are first drawn");
  // The page's ids: file 0's line is `media-file-caution`, file n's `media-file-caution-<n>`.
  const caution = (n) => byId(doc, n ? "media-file-caution-" + n : "media-file-caution");
  assert.equal(text(caution(1)), riskOf("textures.zip").words, "the archive's word, under its row");
  // The words themselves are the data file's (the archive kind with its ending filled in), read from
  // the file, never typed out: they are rewritten there from time to time.
  // Seen red 2026-10-17: typed out as "This is an archive (.zip).", it failed with "in the list's
  // words" once the data file's words were rewritten that day; read from the file, it passes.
  assert.equal(text(caution(1)), RISKS.kinds.find((k) => k.id === "archive").words.replace("{ext}", ".zip"), "in the list's words");
  assert.equal(caution(1).className, "file-caution", "the warning colour's class");
  assert.equal(text(caution(3)), riskOf("ship.blend").words, "a Blender file's word");
  assert.equal(caution(0), null, "a picture: no line");
  assert.equal(caution(2), null, "the cover: no line");
  // Each line comes right after its own row.
  const kids = byId(doc, "media-item-files").children;
  const rowAt = kids.findIndex((k) => k.dataset && k.dataset.file === "1");
  assert.equal(kids[rowAt + 1], caution(1), "right under textures.zip's row");
  // Nothing of any file was read to say it, and the page drawn again asks no more.
  assert.ok(!net.calls.some((c) => c.url.startsWith("/uploads/1761_y") || c.url.startsWith("/uploads/1763_z")), "no file read to say it");
  page.itemView.renderInfo();
  await settle();
  assert.equal(asked, 1, "drawn again: not asked again");
  assert.equal(text(caution(1)), riskOf("textures.zip").words);
});

// ── 12. Each file's type, always (2026-10-17) ────────────────────────────

// The operator: "highlight the file type so people ALWAYS know what file type they're looking at"
// (for "people inexperienced with technology and possibly illiterate people"). Each file row starts
// with a badge (media-edit.js fileBadge): HosFileRisk.badgeHtml's shape built with createElement,
// a drawing, the type's word and the file's ending, coloured by its level. Before the list of kinds
// is in it is a plain grey "File"; the rows are drawn again when it comes.
const FRISK = require(path.join(WEB, "shared", "file-risk.js"));
const RISK_LIST = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "file_risks.json"), "utf8"));
const SVG_NS = "http://www.w3.org/2000/svg";

/** What HosFileRisk.badgeHtml draws for `name` (with the shipped list), read back into its parts. */
function badgeHtmlParts(name) {
  const html = FRISK.badgeHtml(name, null, RISK_LIST);
  const attr = (k) => (new RegExp(" " + k + '="([^"]*)"').exec(html) || [])[1];
  return {
    className: attr("class"), name: attr("data-file-badge"), type: attr("data-file-type"), title: attr("title"),
    label: (/class="file-badge-label">([^<]*)</.exec(html) || [])[1], ending: (/class="file-badge-ending">([^<]*)</.exec(html) || [])[1] || "",
    shapes: (html.match(/<(rect|line|circle|polygon|polyline) /g) || []).length,
  };
}
/** The same parts read off a badge the page built, its drawing made in the SVG namespace. */
function badgeParts(el) {
  const svg = el.children.find((c) => c.tagName === "SVG");
  assert.ok(svg && svg.namespaceURI === SVG_NS, "the badge's drawing is SVG, made in its namespace");
  return {
    className: el.className, name: el.dataset.fileBadge, type: el.dataset.fileType, title: el.title,
    label: text(el.querySelector(".file-badge-label")), ending: text(el.querySelector(".file-badge-ending")) || "",
    shapes: svg.children.length,
  };
}
/**
 * A list route that answers only once `release()` is called (the list late, as on a slow line). Its
 * hold ends with the test whatever happens (`t.after`): file-risk.js is one module for the whole
 * file, and a load left hanging by a failed test would be joined by the next test's page.
 */
function lateList(t) {
  let release = () => {};
  const gate = new Promise((r) => { release = r; });
  t.after(async () => { release(); for (let i = 0; i < 4; i++) await settle(); });
  return { release, route: [at("/data/safety/file_risks.json"), async () => { await gate; return respond(200, RISK_LIST); }] };
}

// Seen red 2026-10-17, each in a copy of web/ through HOS_WEB_DIR: with media-item.js renderInfo's
// row drawn without its badge: "each row starts with its badge: media-item-file-name"; with
// media-edit.js fileBadge handed an empty list (every badge the grey File): "dusk.png: the badge
// file-risk.js draws" (className 'file-badge level-unknown' against 'file-badge level-view').
test("12. each file row starts with its type's badge: a grey File until the list is in, then the badge file-risk.js draws", async (t) => {
  FRISK.use(null); // not in yet: the page asks for it
  const late = lateList(t);
  const { doc, page } = await openPage({ routes: listRoutes(answerOf([fxItem()]), [late.route]) });
  page.openViewer(page.state.items[0]);
  await settle();
  const rows = () => byId(doc, "media-item-files").querySelectorAll(".media-item-file-row");
  assert.equal(rows().length, 3, "test setup: three rows, the cover's included");
  for (const row of rows()) {
    assert.ok(row.children[0].className.startsWith("file-badge"), "each row starts with its badge: " + row.children[0].className);
    assert.equal(row.children[0].className, "file-badge level-unknown", "before the list is in: grey");
    assert.equal(text(row.children[0].querySelector(".file-badge-label")), "File", "and the word File");
    assert.equal(row.children[1].className, "media-item-file-name", "then the name");
  }
  assert.equal(text(rows()[1].children[0].querySelector(".file-badge-ending")), ".zip", "with the file's own ending");
  late.release();
  for (let i = 0; i < 4; i++) await settle();
  const names = ["dusk.png", "textures.zip", "cover.jpg"];
  rows().forEach((row, i) => {
    assert.deepEqual(badgeParts(row.children[0]), badgeHtmlParts(names[i]), names[i] + ": the badge file-risk.js draws");
    assert.equal(row.children[1].className, "media-item-file-name", names[i] + ": just before its name");
  });
  assert.equal(rows()[0].children[0].className, "file-badge level-view", "a picture: fine to look at");
  assert.equal(text(rows()[0].children[0]), "Picture.png");
  assert.equal(rows()[1].children[0].className, "file-badge level-care", "an archive: take care");
  assert.equal(text(rows()[1].children[0]), "Archive.zip");
});

// ── 13. Edit in three tinted parts (2026-10-17) ─────────────────────────

/** The tinted parts directly in `el`. */
const partsOf = (el) => el.children.filter((c) => c.className.split(/\s+/).includes("tint-section"));
const partTitles = (el) => partsOf(el).map((p) => [p.className, text(p.children[0])]);
/** Is `child` inside `el`? */
const within = (el, child) => { for (let n = child; n; n = n.parentNode) if (n === el) return true; return false; };

// The operator asked for "the RGB aesthetic for the different sections of the interface like how
// the DMs, Groups, and servers in chat are but darker RGB in the pop up window", and noticed it
// missing from Edit. As the desktop's Edit window (media/edit.rs): "About it" (red), "Who can see
// it" (green), "Files" (blue), each holding exactly its fields; Save and Cancel under them, outside.
// Its file rows show each file's type too, drawn again once the list of kinds is in, and say
// "name, size" ("name, cover picture, size" for the cover).
// Seen red 2026-10-17, each in a copy of web/ through HOS_WEB_DIR: with media-edit.js draw() putting
// the fields straight into the form, no tintSection: "the three parts, in order" (actual []); with
// the editor's rows drawn without their badge: "the cover's row: its badge first"; with media-item.js
// riskOf not drawing an open Edit form's files again once the list is in: "the editor's archive,
// once the list is in" ('file-badge level-unknown' against 'file-badge level-care').
test("13. Edit is in three tinted parts (About it, Who can see it, Files) with Save and Cancel under them; its files show their types", async (t) => {
  FRISK.use(null);
  const late = lateList(t);
  const mine = fxItem({ uploader_key: ME, uploader_name: "Me" });
  const { doc, page } = await openPage({ routes: listRoutes(answerOf([mine]), [late.route]) });
  page.openViewer(page.state.items[0]);
  byId(doc, "media-item-edit-open").click();
  const form = byId(doc, "media-edit");
  assert.deepEqual(partTitles(form), [
    ["tint-section tint-red", "About it"], ["tint-section tint-green", "Who can see it"], ["tint-section tint-blue", "Files"],
  ], "the three parts, in order");
  for (const p of partsOf(form)) assert.equal(p.children[0].className, "tint-section-title", "each with its title on top");
  const [about, who, files] = partsOf(form);
  for (const id of ["media-edit-title", "media-edit-category", "media-edit-category-desc", "media-edit-description", "media-edit-labels"]) {
    assert.ok(within(about, byId(doc, id)), "About it holds #" + id);
  }
  for (const id of ["media-edit-visibility", "media-edit-licence"]) assert.ok(within(who, byId(doc, id)), "Who can see it holds #" + id);
  assert.ok(within(who, form.querySelector("[data-hosting]")), "Who can see it holds the hosting paragraph");
  for (const id of ["media-edit-files", "media-edit-add-file", "media-edit-cover"]) assert.ok(within(files, byId(doc, id)), "Files holds #" + id);
  for (const id of ["media-edit-save", "media-edit-cancel", "media-edit-problem"]) {
    const el = byId(doc, id);
    assert.ok(el && partsOf(form).every((p) => !within(p, el)), "#" + id + " is outside the parts");
  }
  const kids = form.children;
  assert.ok(kids.indexOf(files) < kids.findIndex((c) => within(c, byId(doc, "media-edit-save"))), "Save comes under them");
  assert.equal(form.querySelectorAll("h4").length, 0, "the files part's own heading went: its part says Files");

  // Its rows: the item itself, then the added file and the cover, each with its badge first.
  const list = byId(doc, "media-edit-files");
  const firstRow = list.children[0];
  assert.equal(firstRow.children[0].className, "file-badge level-unknown", "the item itself: its badge first, grey before the list is in");
  assert.equal(text(firstRow.children[1]), ED.firstFileWords("dusk.png"));
  const editRows = list.querySelectorAll("[data-edit-file]");
  assert.deepEqual(editRows.map((r) => text(r.querySelector(".media-item-file-name"))), ["textures.zip, 4 KB", "cover.jpg, cover picture, 5 KB"], "name, size; the cover says so");
  assert.ok(editRows[1].children[0].className.startsWith("file-badge"), "the cover's row: its badge first");
  late.release();
  for (let i = 0; i < 4; i++) await settle();
  const after = byId(doc, "media-edit-files");
  assert.equal(after.querySelectorAll("[data-edit-file]")[0].children[0].className, "file-badge level-care", "the editor's archive, once the list is in");
  assert.deepEqual(badgeParts(after.children[0].children[0]), badgeHtmlParts("dusk.png"), "the item itself: the badge file-risk.js draws");
  assert.deepEqual(badgeParts(after.querySelectorAll("[data-edit-file]")[1].children[0]), badgeHtmlParts("cover.jpg"), "the cover: the badge file-risk.js draws");
});

// ── 14 to 17. Rename and Use as cover in Edit (2026-10-17) ──────────────
//
// The operator: "Can we allow renaming of files in the edit window? Also, I uploaded a png with the
// blend and STL files but, I can't seem to assign it as the preview photo." The desktop's Edit window
// is the original (src/gui/pages/media/edit.rs draw_files; its words RENAME, RENAME_TIP, SAVE_NAME,
// USE_AS_COVER, USE_AS_COVER_TIP, and item_view.rs FILE_RENAMED, COVER_CHOSEN), the relay's routes
// src/relay/media_edit.rs files_rename and files_cover.

/**
 * The operator's kind of item: a Blender file, with its STL, a PNG and a tarball added (the STL's
 * own name in capitals), and the fixture's cover.
 */
function partsItem(over) {
  return fxItem(Object.assign({
    kind: "model", url: "/uploads/1760_cap.blend", filename: "1760_cap.blend", original_name: "cap.blend", size_bytes: 5000, file_count: 4,
    files: [
      { file: 0, filename: "1760_cap.blend", original_name: "cap.blend", kind: "model", size_bytes: 5000, url: "/uploads/1760_cap.blend" },
      { file: 1, filename: "1761_cap.stl", original_name: "Cap.STL", kind: "model", size_bytes: 3000, url: "/uploads/1761_cap.stl" },
      { file: 3, filename: "1763_photo.png", original_name: "photo.png", kind: "picture", size_bytes: 2000, url: "/uploads/1763_photo.png" },
      { file: 4, filename: "1764_src.tar.gz", original_name: "src.tar.gz", kind: "file", size_bytes: 1000, url: "/uploads/1764_src.tar.gz" },
    ],
  }, over || {}));
}
const mineParts = () => partsItem({ uploader_key: ME, uploader_name: "Me" });
/** The page with `it` open and its Edit form open; `routes` answer first. */
async function openEditOf(it, o) {
  const opts = o || {};
  const p = await openPage(Object.assign({ routes: listRoutes(answerOf([it]), opts.routes || []) }, opts.session ? { session: opts.session } : {}));
  p.page.openViewer(p.page.state.items[0]);
  byId(p.doc, "media-item-edit-open").click();
  assert.ok(byId(p.doc, "media-edit"), "test setup: Edit is open");
  return p;
}
const filesBox = (doc) => byId(doc, "media-edit-files");
const renameButtons = (doc) => filesBox(doc).querySelectorAll("[data-rename-file]");
const renameOf = (doc, n) => renameButtons(doc).find((b) => b.dataset.renameFile === String(n));
const boxes = (doc) => filesBox(doc).querySelectorAll("[data-rename-input]");
const renamingRow = (doc, n) => filesBox(doc).querySelectorAll("[data-renaming]").find((r) => r.dataset.renaming === String(n));
const coverButtons = (doc) => filesBox(doc).querySelectorAll("[data-use-as-cover]");
const typeInto = (el, v) => { el.value = v; el.dispatch("input"); };
const statusLine = (doc) => text(byId(doc, "media-item-status"));
/** BLAKE3 by another implementation than the page's: the vendored noble bundle (held to the Rust crate by scripts/pq-kat.mjs), from the real web/. */
let NOBLE = null;
async function nobleBlake3Hex(bytes) {
  if (!NOBLE) {
    const src = fs.readFileSync(path.join(ROOT, "web", "shared", "vendor", "noble-pq.bundle.js"));
    NOBLE = await import("data:text/javascript;base64," + Buffer.from(src).toString("base64"));
  }
  return Buffer.from(NOBLE.blake3.create({ dkLen: 32 }).update(bytes).digest()).toString("hex");
}

// Every row of the Files part has Rename, the item's own file and the cover included, in the
// desktop's words; pressed, the row becomes its badge, a box holding the name without its ending
// (the stored file's ending, so "Cap.STL" over "1761_cap.stl" is "Cap"; a tarball's is "tar.gz"), the
// ending right after it, fixed, then Save name and Cancel; one row at a time; an upload moving on
// (which draws the part again) keeps what was typed; an admin's Edit renames too (the relay lets the
// uploader or an admin).
// Seen red 2026-10-17, each check by one break in a copy of web/pages/media-edit.js through
// HOS_WEB_DIR: Rename left off the item's own row: "every row: the item's own file, each added file
// and the cover" (['1','3','4','2']); RENAME_TIP's words changed: "the desktop's words"; the
// renaming row unmarked: "Rename turns its row into the box"; the badge left out of it: "its badge
// stays first"; stemOf answering the whole name: "the box holds the name without its ending"
// ('cap.blend'); the ending's span left out: "the ending after it, fixed" (null); the ending put
// before the box: "right after the box" (1 against 3); SAVE_NAME's word changed: "then Save name
// and Cancel"; every row a box once one renames (`if (f.renaming)`): "the other rows keep their
// Rename" (0 against 4); a second Rename keeping the first box (`f.renaming = f.renaming || ...`)
// and stemOf matching the ending's case: "one row at a time" ([['0','cap']], then [['1','Cap.STL']]);
// the box's input not kept: "an upload moving on keeps what was typed" (['Cap']); storedEnding
// taking only the last part: "a tarball's ending is tar.gz" (['src.tar'], '.gz'); the cover never
// made a box: "the cover too" ([], null); Cancel leaving the box: "Cancel puts the row back" ([1, 4]);
// Rename only for the uploader: "an admin's Edit ... has Rename on every row too" (0).
test("14. every file row in Edit has Rename; its box holds the name without its ending, the ending after it, one row at a time", async () => {
  const { doc, page } = await openEditOf(mineParts());
  assert.deepEqual(renameButtons(doc).map((b) => b.dataset.renameFile), ["0", "1", "3", "4", "2"], "every row: the item's own file, each added file and the cover");
  assert.deepEqual([...new Set(renameButtons(doc).map((b) => text(b) + " / " + b.title))], ["Rename / Change the name people see and download it as. Its ending stays."], "the desktop's words");

  renameOf(doc, 0).click();
  const row0 = renamingRow(doc, 0);
  assert.ok(row0, "Rename turns its row into the box");
  assert.ok(row0.children[0].className.startsWith("file-badge"), "its badge stays first");
  const box0 = row0.querySelector("[data-rename-input]");
  assert.deepEqual([box0.tagName, box0.type, box0.value], ["INPUT", "text", "cap"], "the box holds the name without its ending");
  const ending0 = row0.querySelector("[data-rename-ending]");
  assert.equal(text(ending0), ".blend", "the ending after it, fixed");
  assert.equal(row0.children.indexOf(ending0), row0.children.indexOf(box0) + 1, "right after the box");
  assert.deepEqual(row0.querySelectorAll("button").map(text), ["Save name", "Cancel"], "then Save name and Cancel");
  assert.equal(renameButtons(doc).length, 4, "the other rows keep their Rename");

  renameOf(doc, 1).click();
  assert.deepEqual(boxes(doc).map((b) => [b.dataset.renameInput, b.value]), [["1", "Cap"]], "one row at a time: the STL's now, its ending matched whatever its case");
  typeInto(boxes(doc)[0], "Cap, printed");
  page.itemView.uploadsChanged();
  assert.deepEqual(boxes(doc).map((b) => b.value), ["Cap, printed"], "an upload moving on keeps what was typed");

  renameOf(doc, 4).click();
  assert.deepEqual([boxes(doc).map((b) => b.value), text(renamingRow(doc, 4).querySelector("[data-rename-ending]"))], [["src"], ".tar.gz"], "a tarball's ending is tar.gz");
  renameOf(doc, 2).click();
  assert.deepEqual([boxes(doc).map((b) => b.value), (renamingRow(doc, 2) ? text(renamingRow(doc, 2).querySelector("[data-rename-ending]")) : null)], [["cover"], ".jpg"], "the cover too");
  renamingRow(doc, 2).querySelector("[data-cancel-rename]").click();
  assert.deepEqual([boxes(doc).length, renameButtons(doc).length], [0, 5], "Cancel puts the row back");

  const adm = await openEditOf(partsItem(), { session: fakeSession({ role: "admin" }) });
  assert.equal(renameButtons(adm.doc).length, 5, "an admin's Edit of someone else's item has Rename on every row too");
});

// Save name sends POST /api/media/files/rename with the item's number and own stored name, the
// file's number and ITS stored name, and the box's text trimmed with "." and the ending put back;
// it is signed over media_file_rename:<id>:<file>:<BLAKE3 hex of that name's UTF-8 bytes>, checked
// here against another BLAKE3 than the page's (a name with letters outside ASCII, so a hash over
// anything but its UTF-8 bytes fails). Answered, the item is the answer's and the page says
// "Renamed."; Enter in the box saves too, here on the item's own file.
// Seen red 2026-10-17, each check by one break in a copy of web/pages/media-edit.js through
// HOS_WEB_DIR: a press sending twice (the busy check gone, the click calling saveName twice): "one
// request for one press" (2); file_name read off the wrong field: "the item's number and own stored
// name, the file's number and its stored name" (undefined); nameWithEnding leaving the ending off:
// "the name with its ending put back, trimmed" ('Dragon egg cap, été'); renamePurpose hashing each
// character's code as one byte, not UTF-8: "signed over the BLAKE3 of the name exactly as sent"
// (8391958b... against 42f528da...); the status saying the remove's words: "the page says Renamed.";
// the box left open on success: "the box closes" (1); the answer's item not taken: "the answer's item
// is the page's" ('Cap.STL, 3 KB'); the keydown listener gone: "Enter in the box saves too" (1
// against 2); storedOf giving the item's own file no stored name: "the item's own file, by its own
// stored name..." (file_name '', name 'Dragon egg').
test("15. Save name sends the name with its ending put back, signed over the BLAKE3 of the name as sent; then Renamed.", async () => {
  const mine = mineParts();
  const renamed = (file, to) => Object.assign({}, mine, file === 0 ? { original_name: to } : {}, {
    files: mine.files.map((x) => (x.file === file ? Object.assign({}, x, { original_name: to }) : x)),
  });
  const { doc, net } = await openEditOf(mine, { routes: [
    [at("/api/media/files/rename", "POST"), (c) => respond(200, { ok: true, item: renamed(c.body.file, c.body.name) })],
  ] });
  renameOf(doc, 1).click();
  typeInto(boxes(doc)[0], "  Dragon egg cap, été  ");
  renamingRow(doc, 1).querySelector("[data-save-name]").click();
  await settle();
  const sent = () => net.calls.filter((c) => c.url === "/api/media/files/rename");
  assert.equal(sent().length, 1, "one request for one press");
  const b = sent()[0].body;
  assert.deepEqual([b.id, b.filename, b.file, b.file_name], [1, "1760_cap.blend", 1, "1761_cap.stl"], "the item's number and own stored name, the file's number and its stored name");
  assert.equal(b.name, "Dragon egg cap, été.stl", "the name with its ending put back, trimmed");
  assert.equal(b.sig, "sig:media_file_rename:1:1:" + await nobleBlake3Hex(new TextEncoder().encode(b.name)), "signed over the BLAKE3 of the name exactly as sent");
  assert.equal(statusLine(doc), "Renamed.", "the page says Renamed.");
  assert.equal(boxes(doc).length, 0, "the box closes");
  const named = filesBox(doc).querySelectorAll("[data-edit-file]").find((r) => r.dataset.editFile === "1");
  assert.equal(text(named.querySelector(".media-item-file-name")), "Dragon egg cap, été.stl, 3 KB", "the answer's item is the page's");

  // Enter in the box saves too: the item's own file this time.
  renameOf(doc, 0).click();
  typeInto(boxes(doc)[0], "Dragon egg");
  boxes(doc)[0].dispatch("keydown", { key: "Enter" });
  await settle();
  assert.equal(sent().length, 2, "Enter in the box saves too");
  const b0 = sent()[1].body;
  assert.deepEqual(
    [b0.file, b0.file_name, b0.name, b0.sig, text(filesBox(doc).children[0].children[1])],
    [0, "1760_cap.blend", "Dragon egg.blend", "sig:media_file_rename:1:0:" + await nobleBlake3Hex(new TextEncoder().encode("Dragon egg.blend")), ED.firstFileWords("Dragon egg.blend")],
    "the item's own file, by its own stored name, its ending put back; its row says its new name",
  );
});

// "Use as cover" is the uploader's, on a picture already on the item, never on the item's own file
// (a picture or not) or on the cover there now, in the desktop's words. Pressed, it sends POST
// /api/media/files/cover signed over media_file_cover:<id>:<file>; answered, the picture is the
// cover, the old cover one of the files (with Use as cover now), and the page says "That picture is
// the cover now." An admin's Edit of someone else's item has none.
// Seen red 2026-10-17, each check by one break in a copy of web/pages/media-edit.js through
// HOS_WEB_DIR: the picture check gone: "only the PNG" (['1','3','4']); USE_AS_COVER_TIP's words
// changed: "the desktop's words"; the purpose naming the stored name, not the number: "one request,
// signed over media_file_cover:1:3" ('sig:media_file_cover:1:1763_photo.png'); the status saying
// "Renamed.": "the page says so"; the answer's item not taken: "the answer's item: the picture the
// cover..." (the old rows, ['3']); Use as cover on the item's own picture: "not on the item's own
// picture, the zip or the cover" (['0']); the uploader check gone: "an admin's Edit has none" (['3']).
test("16. Use as cover is on a picture that is not the cover, for the uploader; it makes it the cover", async () => {
  const mine = mineParts();
  // The relay's answer: the PNG the cover, keeping its own name (src/relay/media.rs item_json writes
  // the cover as it writes its files), the old cover one of the files.
  const swapped = Object.assign({}, mine, {
    cover: { file: 3, filename: "1763_photo.png", original_name: "photo.png", kind: "picture", size_bytes: 2000, url: "/uploads/1763_photo.png" },
    files: mine.files.filter((x) => x.file !== 3).concat([{ file: 2, filename: "1762_c.jpg", original_name: "", kind: "picture", size_bytes: 4642, url: "/uploads/1762_c.jpg" }]),
  });
  const { doc, net } = await openEditOf(mine, { routes: [[at("/api/media/files/cover", "POST"), () => respond(200, { ok: true, item: swapped })]] });
  assert.deepEqual(coverButtons(doc).map((b) => b.dataset.useAsCover), ["3"], "only the PNG: not the Blender file, the STL, the tarball or the cover");
  const use = coverButtons(doc)[0];
  assert.deepEqual([text(use), use.title], ["Use as cover", "Show this picture on the item's card and page. The cover there now becomes one of its files."], "the desktop's words");
  use.click();
  await settle();
  const sent = net.calls.filter((c) => c.url === "/api/media/files/cover").map((c) => [c.body.id, c.body.filename, c.body.file, c.body.file_name, c.body.sig]);
  assert.deepEqual(sent, [[1, "1760_cap.blend", 3, "1763_photo.png", "sig:media_file_cover:1:3"]], "one request, signed over media_file_cover:1:3");
  assert.equal(statusLine(doc), "That picture is the cover now.", "the page says so");
  assert.deepEqual(
    [filesBox(doc).querySelectorAll(".media-item-file-name").map(text), coverButtons(doc).map((b) => b.dataset.useAsCover)],
    [["Cap.STL, 3 KB", "src.tar.gz, 1000 bytes", "1762_c.jpg, 5 KB", "photo.png, cover picture, 2 KB"], ["2"]],
    "the answer's item: the picture the cover, the old cover one of its files, and may be the cover again",
  );

  // A picture as the item's own file: never on it (it is the item itself), nor on the cover.
  const pic = await openEditOf(fxItem({ uploader_key: ME, uploader_name: "Me" }));
  assert.deepEqual(coverButtons(pic.doc).map((b) => b.dataset.useAsCover), [], "not on the item's own picture, the zip or the cover");
  const adm = await openEditOf(partsItem(), { session: fakeSession({ role: "admin" }) });
  assert.deepEqual(coverButtons(adm.doc).map((b) => b.dataset.useAsCover), [], "an admin's Edit has none (only the uploader chooses the cover)");
});

// Refused, a rename or a Use as cover shows the relay's own words in the Edit window's problem line
// (#media-edit-problem, as Save's refusals do), nothing is said to be done, and a refused rename
// keeps its box open with what was typed, to be put right; either may be pressed again.
// Seen red 2026-10-17, each check by one break in a copy of web/pages/media-edit.js through
// HOS_WEB_DIR: saveName's refusal said on the status line: "the relay's words in the problem line"
// (''); the box closed on a refusal: "the box stays with what was typed" ([]); the busy flag kept
// after a refused rename: "and may be saved again" (disabled); "Renamed." said before the answer:
// "nothing is said to be renamed"; useAsCover's refusal said on the status line: "a refused cover:
// its words in the problem line" (the rename's words still there); the cover said before the answer:
// "nothing is said to be the cover"; the busy flag kept after a refused cover: "Use as cover may be
// pressed again" ([true]). The setup check (".png" sent) went red under the ending and input breaks.
test("17. a refused rename or cover shows the relay's words in Edit's problem line, and a refused rename keeps its box", async () => {
  const { doc, net } = await openEditOf(mineParts(), { routes: [
    [at("/api/media/files/rename", "POST"), () => respond(400, "A file needs a name before its ending.")],
    [at("/api/media/files/cover", "POST"), () => respond(409, "That is not the Media item you were looking at: the page is out of date. Load it again.")],
  ] });
  renameOf(doc, 3).click();
  typeInto(boxes(doc)[0], "   ");
  renamingRow(doc, 3).querySelector("[data-save-name]").click();
  await settle();
  const sent = net.calls.filter((c) => c.url === "/api/media/files/rename");
  assert.deepEqual(sent.map((c) => c.body.name), [".png"], "test setup: the empty name went, with its ending");
  assert.equal(text(byId(doc, "media-edit-problem")), "A file needs a name before its ending.", "the relay's words in the problem line");
  assert.deepEqual(boxes(doc).map((b) => b.value), ["   "], "the box stays with what was typed");
  assert.equal(renamingRow(doc, 3).querySelector("[data-save-name]").disabled, false, "and may be saved again");
  assert.notEqual(statusLine(doc), "Renamed.", "nothing is said to be renamed");

  renamingRow(doc, 3).querySelector("[data-cancel-rename]").click();
  coverButtons(doc)[0].click();
  await settle();
  assert.equal(text(byId(doc, "media-edit-problem")), "That is not the Media item you were looking at: the page is out of date. Load it again.", "a refused cover: its words in the problem line");
  assert.notEqual(statusLine(doc), "That picture is the cover now.", "nothing is said to be the cover");
  assert.deepEqual(coverButtons(doc).map((b) => b.disabled), [false], "Use as cover may be pressed again");
});
