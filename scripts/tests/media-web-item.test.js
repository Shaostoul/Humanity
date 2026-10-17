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
  assert.ok(text(caution(1)).startsWith("This is an archive (.zip)."), "in the list's words");
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
