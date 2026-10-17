// The Media page on the website (2026-10-11, docs/design/media-showcase.md "Increment 2, exact").
//
// Run: node --test scripts/tests/media-web.test.js   (in `just rig-tests`)
//
// The page's own files run as they do in the browser, against the stand-ins in
// scripts/tests/media-web-harness.js (shared with media-web-round2.test.js, which holds round 2:
// the relay's follow-ups and the review's fixes): a document that holds ONLY what the page built
// in it (innerHTML throws), a network that records every request and answers like
// src/relay/media.rs, and signing that returns "sig:" plus the words it signed. The session's
// sign-in check is real ML-DSA-65 from the vendored noble bundle, verified the way the relay
// verifies it. HOS_WEB_DIR points the test at another copy of web/ (used to see each test red).
//
// What it proves (the design's web Proof list):
//  1. Empty: the empty words, no tile, no featured strip, no More.
//  2. Items: tiles with titles, uploader names and pick counts; the featured strip (asked for
//     with featured=1); More pages with the answer's `cursor`; the category and person filters come from the
//     server's answers and the kinds from data/media/kinds.json.
//  3. A labelled tile shows its label instead of its picture, and no picture is even requested
//     until it is clicked.
//  4. The members-only line for someone not signed in, the unsigned public list then; a signed
//     list (media_list) for someone signed in, no line, and a members-only file fetched from
//     /api/media/file/{id} with the session's upload token in a header, shown through an object
//     URL.
//  5. Pictures load only from this server's own addresses.
//  6. Add to Media: the share shown from the relay's own count ("You have 412 MB of your 512 MB
//     left"), an upload over the share stopped before it starts (no upload request at all), one
//     within it sent as a resumable upload with the session token, then published.
//  7. Put on Media (a Shared Files row): public only, the visibility choice not offered and the
//     line saying why; no upload. On a members-only server Add to Media offers no choice either.
//  8. Each signed purpose names its target: publish, pick, unpick, remove (with the words naming
//     what happens to the file), feature; and in the admins' section the category list's hash,
//     a suggestion's decision, a waiting upload's removal by name.
//  9. The admins' section: the visibility switch with the relay's members sentence under it, the
//     four settings saved in one server_settings_update, the meter ("12.4 GB of 50 GB used") with
//     its bar, the prune lists and the suggestions waiting with the suggester named.
// 10. The session signs in with a real Dilithium3 signature over the relay's exact words, sends
//     nothing else first, takes the upload token, role and limit from its own peer_list entry,
//     asks for the settings, keeps the role list, and with no identity opens no socket.
// 13. (2026-10-17) Add to Media is in three tinted parts, "Files" (blue) first, then "About it"
//     (red) and "Who can see it" (green), the buttons under them; each chosen file shows its type's
//     badge (a grey "File" until the list of kinds is in, asked for once), then its name and size.
// 14. (2026-10-17) Shared Files: each row's badge just before the file's name, drawn again once the
//     list of kinds is in; a name is never markup.
// 15. (2026-10-17) The badge's and the tinted parts' styles use theme tokens only (the levels and the
//     three sidebar colours), and every page that shows them loads them and /shared/file-risk.js.
//
// Seen red, 2026-10-11 (each with the code it guards broken in a copy of web/ under HOS_WEB_DIR,
// then green again on the real web/): see the note above each test.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const H = require("./media-web-harness.js");
const {
  WEB, media, admin, relaySession, ORIGIN, ME, ANN, MIB, byId, text, respond, path0, at, fakeSession,
  settle, item, listAnswer, mineAnswer, listRoutes, openPage, BLAKE, openAdmin,
} = H;

// ── 1. Empty ─────────────────────────────────────────────────────────────

// Seen red with renderGrid's empty branch drawing nothing: "the empty words / actual: null".
test("1. an empty Media page says so and draws no tile, strip or More", async () => {
  const { doc } = await openPage({ routes: listRoutes(listAnswer([])) });
  const grid = byId(doc, "media-grid");
  assert.equal(text(grid.querySelector(".media-empty")), media.MEDIA_WORDS.empty, "the empty words");
  assert.equal(grid.querySelectorAll(".media-tile").length, 0, "no tile");
  assert.equal(byId(doc, "media-featured-row"), null, "no featured strip");
  assert.equal(byId(doc, "media-more"), null, "no More");
});

// ── 2. Items ─────────────────────────────────────────────────────────────

// Seen red with takeAnswer's `next_before` read as `nextBefore`: "More is there when the server
// says there is more / actual: null". Round 5 (2026-10-12) changed it: the answer's `next_cursor`
// (opaque, "<value>.<id>") replaced `next_before`, More sends it back as `cursor`, and an item's
// `featured` is "main", "category" or "none". Round 2 (2026-10-11): seen red with the kind filter built
// from the items' kinds again instead of the kinds file: "the kinds come from
// data/media/kinds.json".
test("2. items show as tiles with names and picks, the featured strip leads, More pages on", async () => {
  const first = listAnswer([
    item({ id: 9, title: "Contact staff", featured: "main", picks: 3 }),
    item({ id: 8, title: "", url: "/uploads/8_clip.mp4", kind: "video", uploader_key: ME, uploader_name: "Me", picks: 1 }),
  ], { next_cursor: "8.8" });
  const second = listAnswer([item({ id: 7, title: "Older", kind: "model", url: "/uploads/7_part.stl", uploader_key: "c3".repeat(32), uploader_name: "Cy" })]);
  let page = 0;
  const { doc, net } = await openPage({ routes: listRoutes(first, [
    [(c) => c.url === "/api/media/list" && c.body && c.body.featured !== 1, () => respond(200, page++ === 0 ? first : second)],
  ]) });
  const tiles = byId(doc, "media-grid").querySelectorAll(".media-tile");
  assert.equal(tiles.length, 2);
  assert.equal(text(tiles[0].querySelector(".media-tile-title")), "Contact staff");
  assert.equal(text(tiles[0].querySelector(".media-tile-meta")), "by Ann · 3 picks");
  assert.equal(tiles[0].querySelector("img").src, "/uploads/1_a.png", "a picture tile shows the picture from /uploads/");
  assert.equal(text(tiles[1].querySelector(".media-tile-title")), "8_clip.mp4", "an untitled item is called by its file name");
  assert.equal(text(tiles[1].querySelector(".media-tile-meta")), "by Me · 1 pick");
  assert.equal(text(tiles[1].querySelector(".media-mark-word")), "Video", "a kind's name for one, from the kinds file");
  assert.ok(tiles[1].querySelector(".media-mark-play"), "and its mark");
  const strip = byId(doc, "media-featured-row");
  assert.ok(strip, "the featured strip is there");
  assert.equal(strip.querySelectorAll(".media-tile").length, 1);
  assert.equal(text(strip.querySelector(".media-tile-title")), "Contact staff");
  // The filters: the server's categories and people, the kinds from the kinds file.
  const options = (id) => byId(doc, id).children.map((o) => o.textContent);
  assert.deepEqual(options("media-filter-category"), ["All categories", "General", "Fire flow arts"]);
  assert.deepEqual(options("media-filter-kind"), ["All kinds", "Pictures", "Videos", "Models", "Files"], "the kinds come from data/media/kinds.json");
  assert.deepEqual(options("media-filter-person"), ["Everyone", "Ann", "Me"]);
  const more = byId(doc, "media-more");
  assert.ok(more, "More is there when the server says there is more");
  more.click();
  await settle();
  const lists = net.calls.filter((c) => c.url === "/api/media/list" && c.body.featured !== 1);
  assert.equal(lists.at(-1).body.cursor, "8.8", "More sends back the answer's cursor");
  assert.equal(lists.at(-1).body.before, undefined, "and never `before`, which the relay no longer reads");
  assert.equal(byId(doc, "media-grid").querySelectorAll(".media-tile").length, 3, "the next page is added below");
  assert.equal(byId(doc, "media-more"), null, "no More after the last page");
  // A filter asks the server again with it.
  const kind = byId(doc, "media-filter-kind");
  kind.value = "video";
  kind.dispatch("change");
  await settle();
  assert.equal(net.calls.filter((c) => c.url === "/api/media/list").at(-1).body.kind, "video");
});

// ── 3. A labelled tile ───────────────────────────────────────────────────

// Seen red with thumb() skipping the veil (labels ignored): "a labelled tile requests no
// picture before it is clicked / 1 !== 0".
test("3. a labelled tile shows its label instead of its picture until clicked", async () => {
  const { doc } = await openPage({ routes: listRoutes(listAnswer([item({ id: 4, labels: ["flashing", "graphic"], title: "Fire show" })])) });
  const tile = byId(doc, "media-grid").querySelector(".media-tile");
  assert.equal(tile.querySelectorAll("img").length, 0, "a labelled tile requests no picture before it is clicked");
  const veil = tile.querySelector(".media-veil");
  assert.ok(veil, "the label stands in the picture's place");
  assert.equal(text(veil.querySelector(".media-veil-labels")), "Flashing lights, Graphic", "the label names come from the server's list");
  assert.deepEqual(tile.querySelectorAll(".media-badge-label").map((b) => b.textContent), ["Flashing lights", "Graphic"], "label badges");
  veil.click();
  const shown = byId(doc, "media-grid").querySelector(".media-tile");
  assert.equal(shown.querySelectorAll("img").length, 1, "after the click the picture shows");
  assert.equal(shown.querySelector("img").src, "/uploads/1_a.png");
  assert.equal(shown.querySelector(".media-veil"), null);
});

// ── 4. Members ───────────────────────────────────────────────────────────

// Seen red with renderMembers drawing nothing: "someone not signed in is told / actual: null";
// and with fetchList never signing: "a signed-in person's list is signed / actual: undefined".
// Round 2: seen red with the members' file asked for as /api/media/file/7?token=tok-1 again (no
// header): "the members' file route, with nothing after it".
test("4. the members-only line, the public list unsigned, the members' list signed, files by token", async () => {
  const publicAnswer = listAnswer([item({ id: 3 })], { members_view: false, categories: [], labels: [] });
  const out = await openPage({ myKey: "", session: null, routes: listRoutes(publicAnswer) });
  assert.equal(text(byId(out.doc, "media-members-line")), "Sign in to this server to see its members' media.", "someone not signed in is told");
  const lists = out.net.calls.filter((c) => path0(c.url) === "/api/media");
  assert.equal(lists.length, 2, "the list and the featured strip");
  for (const l of lists) assert.equal(l.init.method, undefined, "the public list is a plain GET");
  assert.equal(out.signer.signed.length, 0, "nothing is signed without an identity");
  assert.equal(byId(out.doc, "media-add-btn"), null, "no Add to Media");
  assert.equal(byId(out.doc, "media-filter-picks"), null, "no My picks");
  assert.equal(out.doc.body.querySelectorAll("[data-action]").length, 0, "no Pick, Remove, Feature or Report");
  assert.equal(text(byId(out.doc, "media-status").querySelector(".media-status-text")).startsWith("Not signed in on this page."), true);

  const membersAnswer = listAnswer([item({ id: 7, url: "/api/media/file/7", visibility: "members" }), item({ id: 6 })]);
  const inside = await openPage({ routes: listRoutes(membersAnswer, [
    [at("/api/media/file/7"), () => respond(200, "", new Uint8Array([9, 9]))],
  ]) });
  const list = inside.net.calls.find((c) => c.url === "/api/media/list");
  assert.ok(list, "a signed-in person's list is signed (POST /api/media/list)");
  assert.equal(list.body.sig, "sig:media_list", "the list read is signed as media_list");
  assert.equal(list.body.key, ME);
  assert.equal(byId(inside.doc, "media-members-line"), null, "no line for a member");
  const file = inside.net.calls.find((c) => path0(c.url) === "/api/media/file/7");
  assert.equal(file.url, "/api/media/file/7", "the members' file route, with nothing after it");
  assert.equal(file.init.headers["X-Upload-Token"], "tok-1", "the token goes in a header");
  const img = byId(inside.doc, "media-grid").querySelector(".media-tile").querySelector("img");
  assert.equal(img.src, "blob:test/1", "a members-only file is shown through the page's own address");
  assert.ok(byId(inside.doc, "media-add-btn"), "Add to Media for a signed-in member");

  // Without the session's token yet, the members-only file is not requested at all.
  const early = await openPage({ session: fakeSession({ status: "connecting", uploadToken: "" }), routes: listRoutes(membersAnswer) });
  const first = byId(early.doc, "media-grid").querySelector(".media-tile");
  assert.equal(first.querySelectorAll("img").length, 0);
  assert.equal(text(first.querySelector(".media-thumb-missing")), media.MEDIA_WORDS.waitingToken);
  assert.equal(early.net.calls.filter((c) => c.url.startsWith("/api/media/file/")).length, 0, "nothing asked for without a token");
});

// ── 5. Only this server's addresses ──────────────────────────────────────

// Seen red with ownFilePath accepting any address: "a picture from another website is not
// loaded / 1 !== 0".
test("5. a picture loads only from this server's own addresses", async () => {
  const answer = listAnswer([
    item({ id: 3, url: "https://elsewhere.example/x.png" }),
    item({ id: 2, url: "//elsewhere.example/uploads/x.png" }),
    item({ id: 1, url: "/uploads/../api/x.png" }),
  ]);
  const { doc } = await openPage({ routes: listRoutes(answer) });
  const tiles = byId(doc, "media-grid").querySelectorAll(".media-tile");
  assert.equal(tiles.length, 3);
  for (const t of tiles) {
    assert.equal(t.querySelectorAll("img").length, 0, "a picture from another website is not loaded");
    assert.equal(text(t.querySelector(".media-thumb-missing")), media.MEDIA_WORDS.notThisServer);
  }
  assert.equal(media.ownFilePath("/uploads/5_a.png", ORIGIN), "/uploads/5_a.png");
  assert.equal(media.ownFilePath(ORIGIN + "/uploads/5_a.png", ORIGIN), "/uploads/5_a.png");
  assert.equal(media.ownFilePath("/api/media/file/12", ORIGIN), "/api/media/file/12");
  assert.equal(media.ownFilePath("/api/media/file/12/../../x", ORIGIN), null);
  assert.equal(media.ownFilePath("javascript:alert(1)", ORIGIN), null);
});

// ── 6. Add to Media ──────────────────────────────────────────────────────

/** The relay's resumable routes, answering a whole file in one piece. */
function resumableRoutes(name) {
  return [
    [at("/api/upload/resumable", "POST"), (c) => respond(201, { id: "a".repeat(32), offset: 0, size: c.body.size, chunk_max: 8 * MIB })],
    [(c) => path0(c.url) === "/api/upload/resumable/" + "a".repeat(32) && c.init.method === "PATCH",
      () => respond(200, { url: "/uploads/" + name, filename: name, size: 10 * MIB, type: "image/png" })],
  ];
}

async function pickFile(doc, file) {
  const input = byId(doc, "media-file-input");
  input.files = [file];
  input.dispatch("change");
  await settle();
}

function aFile(name, size, type) {
  return { name, size, type: type || "image/png", lastModified: 1700000000000, slice: (a, b) => new Blob([new Uint8Array(Math.max(0, Math.min(size, b) - a))]) };
}

// Seen red with openAddForm leaving out shareStop: "an upload over the share is stopped /
// actual: null"; with the form not waiting for the share (`checking` always false): "no upload
// while the share is being worked out / actual: false"; and with shareLeftWords showing the whole
// share as left: "'You have 512 MB of your 512 MB left' == 'You have 412 MB of your 512 MB left'".
// Round 2: the share now comes from POST /api/media/mine (seen red in media-web-round2.test.js).
test("6. Add to Media shows the share, stops an upload over it before it starts, uploads one within it", async () => {
  let open;
  const gate = new Promise((r) => { open = r; });
  const { doc, net, signer } = await openPage({ routes: listRoutes(listAnswer([]), [
    // The person's own use is held back until `gate` opens (the first answer, at the page's start, is not).
    [at("/api/media/mine", "POST"), (() => { let n = 0; return async () => { if (n++ > 0) await gate; return respond(200, mineAnswer()); }; })()],
    ...resumableRoutes("1760_new.png"),
    [at("/api/media", "POST"), (c) => respond(200, { ok: true, item: item({ id: 200, url: "/uploads/" + c.body.filename }) })],
  ]) });
  // Over the share: 100 MB used of 512 MB, a 413 MB file. While the share is being worked out,
  // nothing can be uploaded.
  await pickFile(doc, aFile("big.mp4", 413 * MIB, "video/mp4"));
  assert.equal(byId(doc, "media-form-go").disabled, true, "no upload while the share is being worked out");
  byId(doc, "media-form-go").disabled = false; // a determined click on a button that should not go
  byId(doc, "media-form-go").click();
  await settle();
  assert.equal(net.calls.filter((c) => path0(c.url).startsWith("/api/upload")).length, 0, "no upload while the share is being worked out");
  open();
  await settle();
  assert.equal(text(byId(doc, "media-share-line")), "You have 412 MB of your 512 MB left");
  const stop = byId(doc, "media-form-stop");
  assert.ok(stop, "an upload over the share is stopped");
  assert.match(text(stop), /^"big\.mp4" is 413 MB, more than the 412 MB you have left of your 512 MB share, so it was not uploaded\./);
  assert.equal(byId(doc, "media-form-go"), null, "nothing to press that would upload it");
  assert.equal(net.calls.filter((c) => path0(c.url).startsWith("/api/upload")).length, 0, "no upload request at all");
  byId(doc, "media-form-cancel").click();
  assert.equal(byId(doc, "media-form-stop"), null);

  // Within the share.
  await pickFile(doc, aFile("staff.png", 10 * MIB));
  assert.equal(byId(doc, "media-form-stop"), null);
  byId(doc, "media-form-title").value = "Spinning at dusk";
  byId(doc, "media-form-title").dispatch("input");
  const cat = byId(doc, "media-form-category");
  cat.value = "2";
  cat.dispatch("change");
  const flashing = byId(doc, "media-form-labels").querySelector('[data-label="flashing"]');
  flashing.checked = true;
  flashing.dispatch("change");
  byId(doc, "media-form-vis-members").checked = true;
  byId(doc, "media-form-vis-members").dispatch("change");
  byId(doc, "media-form-go").click();
  await settle();
  const start = net.calls.find((c) => path0(c.url) === "/api/upload/resumable");
  // Round 3 (R3-9): the token in the X-Upload-Token header, never in the address.
  assert.equal(start.url, "/api/upload/resumable", "no token in the address");
  assert.equal(start.headers["X-Upload-Token"], "tok-1", "started with the session token");
  assert.deepEqual(start.body, { filename: "staff.png", size: 10 * MIB, content_type: "image/png", media: 1 }, "as a Media upload, no hash");
  const pub = net.calls.find((c) => c.url === "/api/media" && c.init.method === "POST");
  assert.equal(pub.body.filename, "1760_new.png");
  // Round 5: the purpose carries the licence and the game tick (the defaults here).
  assert.equal(pub.body.sig, "sig:media_publish:1760_new.png:all_rights_reserved:0", "the publish is signed over its file name, licence and game tick");
  assert.deepEqual([pub.body.licence, pub.body.project_use], ["all_rights_reserved", false]);
  assert.equal(pub.body.title, "Spinning at dusk");
  assert.equal(pub.body.category_id, 2);
  assert.deepEqual(pub.body.labels, ["flashing"]);
  assert.equal(pub.body.visibility, "members");
  assert.equal(byId(doc, "media-form-go"), null, "the form closes");
  assert.equal(text(byId(doc, "media-notice").querySelector(".media-notice-text")), media.MEDIA_WORDS.published);
  assert.ok(signer.signed.includes("media_publish:1760_new.png:all_rights_reserved:0"));
});

// ── 7. Put on Media ──────────────────────────────────────────────────────

// Seen red with renderForm drawing Put on Media like Add to Media (its public-only branch
// skipped, so the Public / Members only choice was offered and the reason left out): "null ==
// 'This file is already public where it was shared.'".
// Several files at once (2026-10-16): one item holds them all. The form names the others, all of
// them are checked against the share together, and once the item is published each of the others
// is uploaded and added to it (POST /api/media/files/add). The desktop app does the same
// (round4_tests.rs files_chosen_with_the_first_are_added_to_the_item_once_it_is_published).
// Seen red with attachMore not called: "the other file is added to the new item / 0 !== 1"; with
// togetherStop left out: "two that fit alone but not together are stopped / actual: null".
test("6b. Add to Media takes several files: one item, the others added to it once it is up", async () => {
  const { doc, net } = await openPage({ routes: listRoutes(listAnswer([]), [
    [at("/api/media/mine", "POST"), () => respond(200, mineAnswer())],
    ...resumableRoutes("1760_new.png"),
    [at("/api/media", "POST"), (c) => respond(200, { ok: true, item: item({ id: 200, url: "/uploads/" + c.body.filename }) })],
    [at("/api/media/files/add", "POST"), () => respond(200, { ok: true, item: item({ id: 200, url: "/uploads/1760_new.png" }) })],
  ]) });
  const input = byId(doc, "media-file-input");
  assert.equal(input.multiple, true, "the picker takes several");

  // Two that fit alone (412 MB left) but not together.
  input.files = [aFile("left.mp4", 300 * MIB, "video/mp4"), aFile("right.mp4", 300 * MIB, "video/mp4")];
  input.dispatch("change");
  await settle();
  const stop = byId(doc, "media-form-stop");
  assert.ok(stop, "two that fit alone but not together are stopped");
  assert.match(text(stop), /^These 2 files come to 600 MB, more than the 412 MB left of your share/);
  byId(doc, "media-form-cancel").click();

  input.files = [aFile("model.stl", 10 * MIB, "model/stl"), aFile("photo.png", MIB)];
  input.dispatch("change");
  await settle();
  // Since 2026-10-17 each further file is its own row, its type's badge first (test 13). Seen red
  // with the further files back in one line of text: "the form says the others go on the item too".
  const more = byId(doc, "media-form-more");
  assert.equal(text(more.children[0]), media.MORE_LEAD, "the form says the others go on the item too");
  assert.deepEqual(more.querySelectorAll(".media-form-file-name").map(text), ["photo.png (1 MB)"], "the form names the other file");
  byId(doc, "media-form-go").click();
  for (let i = 0; i < 5; i++) await settle();
  const adds = net.calls.filter((c) => c.url === "/api/media/files/add");
  assert.equal(adds.length, 1, "the other file is added to the new item");
  assert.deepEqual([adds[0].body.id, adds[0].body.filename, adds[0].body.as], [200, "1760_new.png", "file"], "to item 200, as a file");
  assert.equal(net.calls.filter((c) => c.url === "/api/media" && c.init.method === "POST").length, 1, "one item is published, not two");
});

test("6c. several files are checked together: words and limits", () => {
  const files = [{ name: "a", size: 300 * MIB }, { name: "b", size: 300 * MIB }];
  assert.equal(media.togetherStop(files, 100 * MIB, 512 * MIB, NaN).startsWith("These 2 files come to 600 MB, more than the 412 MB left of your share"), true);
  assert.equal(media.togetherStop(files, 0, 1024 * MIB, 500 * MIB).startsWith("These 2 files come to 600 MB, more than the 500 MB left in this server's whole Media space"), true);
  assert.equal(media.togetherStop(files, 0, 1024 * MIB, NaN), null, "they fit");
  assert.equal(media.togetherStop([files[0]], 500 * MIB, 512 * MIB, NaN), null, "one file is checked as one always was");
  assert.equal(media.moreWords([{ name: "x.png", size: 2 * MIB }]), "Also on this item, each added once the item is up: x.png (2 MB)");
});

test("7. Put on Media from a Shared Files row offers public only and says why; a members-only server offers no choice", async () => {
  const { doc, net } = await openPage({
    put: { filename: "1700_part.stl", name: "bushing.stl" },
    routes: listRoutes(listAnswer([]), [[at("/api/media", "POST"), () => respond(200, { ok: true, item: item() })]]),
  });
  assert.equal(text(byId(doc, "media-form").querySelector("h2")), "Put on Media");
  assert.equal(text(byId(doc, "media-form-public-only")), "This file is already public where it was shared.");
  assert.equal(byId(doc, "media-form-visibility"), null, "the visibility choice is not offered");
  assert.equal(byId(doc, "media-form-vis-members"), null);
  byId(doc, "media-form-go").click();
  await settle();
  assert.equal(net.calls.filter((c) => path0(c.url).startsWith("/api/upload")).length, 0, "no upload: the file is on the server already");
  const pub = net.calls.find((c) => c.url === "/api/media" && c.init.method === "POST");
  assert.equal(pub.body.visibility, "public");
  assert.equal(pub.body.filename, "1700_part.stl");
  assert.equal(pub.body.sig, "sig:media_publish:1700_part.stl:all_rights_reserved:0", "round 5: with the licence and the game tick");

  // A members-only server: Add to Media puts it on as members only, with no choice offered.
  const settings = Object.assign({}, fakeSession().state.settings, { media_visibility: "members" });
  const m = await openPage({ session: fakeSession({ settings }), routes: listRoutes(listAnswer([]), [
    ...resumableRoutes("9_m.png"),
    [at("/api/media", "POST"), () => respond(200, { ok: true, item: item() })],
  ]) });
  await pickFile(m.doc, aFile("m.png", MIB));
  assert.equal(byId(m.doc, "media-form-visibility"), null);
  assert.equal(text(byId(m.doc, "media-form-members-only")), media.MEDIA_WORDS.serverMembersOnly);
  byId(m.doc, "media-form-go").click();
  await settle();
  assert.equal(m.net.calls.find((c) => c.url === "/api/media" && c.init.method === "POST").body.visibility, "members");
});

// ── 8. Each signed purpose names its target ──────────────────────────────

// Seen red with purposes.pick dropping the id ("media_pick"): "pick is signed over its item /
// 'sig:media_pick' !== 'sig:media_pick:5'"; and with removeWords always saying the file stays:
// "a file uploaded for Media goes with it / ...Remove from Media. The file stays where it came
// from." Round 2: seen red with act() leaving out `filename`: "every write names the item's file
// too".
test("8. pick, unpick, remove and feature are each signed over their own item, naming its file", async () => {
  const answer = listAnswer([item({ id: 5, picks: 0 }), item({ id: 6, uploader_key: ME, uploader_name: "Me", url: "/uploads/6_mine.png", keeps_file: false })]);
  const routes = listRoutes(answer, [
    [at("/api/media/pick", "POST"), () => respond(200, { ok: true, id: 5, picked: true, picks: 1 })],
    [at("/api/media/unpick", "POST"), () => respond(200, { ok: true, id: 5, picked: false, picks: 0 })],
    [at("/api/media/remove", "POST"), () => respond(200, { ok: true, removed: 6, file_removed: true, file_kept: false })],
    [at("/api/media/feature", "POST"), () => respond(200, { ok: true, id: 5, featured: "main" })],
  ]);
  const { doc, net, confirms } = await openPage({ session: fakeSession({ role: "admin" }), routes });
  const tileOf = (id) => byId(doc, "media-grid").querySelectorAll(".media-tile").find((t) => t.dataset.id === String(id));
  assert.equal(tileOf(6).querySelector('[data-action="pick"]'), null, "no Pick on your own item");
  tileOf(5).querySelector('[data-action="pick"]').click();
  await settle();
  const last = () => net.calls.filter((c) => !c.url.startsWith("/api/media/list") && c.url !== "/api/media/mine").at(-1);
  assert.equal(last().url, "/api/media/pick");
  assert.equal(last().body.sig, "sig:media_pick:5", "pick is signed over its item");
  assert.equal(last().body.id, 5);
  assert.equal(last().body.filename, "1_a.png", "every write names the item's file too");
  assert.equal(text(tileOf(5).querySelector(".media-tile-meta")), "by Ann · 1 pick");
  tileOf(5).querySelector('[data-action="unpick"]').click();
  await settle();
  assert.equal(last().url, "/api/media/unpick");
  assert.equal(last().body.sig, "sig:media_pick:5", "unpick is signed over its item");
  assert.equal(last().body.filename, "1_a.png");
  // Media round 5, lane 1G (2026-10-12): Feature is a three-way choice now (a select on the tile:
  // Not featured, Featured, Featured in its category), so this picks "main" in it where it used to
  // press a Feature button. What it proves is unchanged.
  const where = tileOf(5).querySelector('[data-action="feature"]');
  assert.equal(where.value, "none", "an item on no shelf reads Not featured");
  where.value = "main";
  where.dispatch("change");
  await settle();
  const feature = net.calls.find((c) => c.url === "/api/media/feature");
  // Round 5: Feature puts it on the main shelf, signed over the item and where.
  assert.equal(feature.body.sig, "sig:media_feature:5:main");
  assert.equal(feature.body.featured, "main");
  assert.equal(feature.body.filename, "1_a.png");
  assert.ok(net.calls.filter((c) => c.url === "/api/media/list" && c.body.featured === 1).length >= 2, "the strip is asked for again");
  tileOf(6).querySelector('[data-action="remove"]').click();
  await settle();
  assert.equal(confirms.at(-1), '"Contact staff"\n\n' + media.MEDIA_WORDS.removeDeletes, "a file uploaded for Media goes with it");
  const rm = net.calls.find((c) => c.url === "/api/media/remove");
  assert.equal(rm.body.sig, "sig:media_remove:6");
  assert.equal(rm.body.id, 6);
  assert.equal(rm.body.filename, "6_mine.png");
  assert.equal(tileOf(6), undefined, "the tile is gone");
  assert.equal(text(byId(doc, "media-notice").querySelector(".media-notice-text")), media.MEDIA_WORDS.removedDeleted);
  assert.equal(media.removedWords({ file_kept: true }), "Removed from Media. The file stays where it came from.");
});

// Seen red with remove() not waiting for the confirmation: "a cancelled Remove sends nothing /
// 3 !== 2" (the removal went out).
test("8b. a cancelled Remove sends nothing", async () => {
  const { doc, net } = await openPage({ confirm: false, session: fakeSession({ role: "admin" }), routes: listRoutes(listAnswer([item({ id: 5 })])) });
  const before = net.calls.length;
  byId(doc, "media-grid").querySelector('[data-action="remove"]').click();
  await settle();
  assert.equal(net.calls.length, before, "a cancelled Remove sends nothing");
});

// ── 9. The admins' section ───────────────────────────────────────────────

// Seen red with meterWords reading the budget as the used bytes: "the meter / '50 GB of 50 GB
// used'"; and with the members note left out of renderSettings: "the relay's sentence under the
// switch / actual: null". (That second break first failed as a TypeError: the section passed a
// null to append(), which a browser writes as the word "null". Fixed in media-admin.js, and the
// stand-in's append() now does what a browser does.)
test("9. the admins' section: settings with the members sentence, the meter, the prune lists, suggestions", async () => {
  const { doc, session } = await openAdmin();
  // Visibility, with the relay's own sentence under it.
  assert.equal(byId(doc, "media-admin-vis-public").checked, true);
  assert.equal(text(byId(doc, "media-admin-members-note")), "Members are everyone signed in to this server. To keep it to people you let in, turn on lockdown.", "the relay's sentence under the switch");
  assert.equal(byId(doc, "media-admin-budget").value, "2048");
  assert.equal(byId(doc, "media-admin-share").value, "512");
  assert.equal(byId(doc, "media-admin-previews").checked, true);
  byId(doc, "media-admin-vis-members").checked = true;
  byId(doc, "media-admin-vis-members").dispatch("change");
  byId(doc, "media-admin-budget").value = "51200";
  byId(doc, "media-admin-budget").dispatch("input");
  byId(doc, "media-admin-previews").checked = false;
  byId(doc, "media-admin-previews").dispatch("change");
  byId(doc, "media-admin-save").click();
  assert.deepEqual(session.sent.at(-1), {
    type: "server_settings_update", media_visibility: "members", media_budget_mb: 51200, media_per_person_mb: 512, media_public_previews_only: false,
  }, "the four settings in one server_settings_update");
  session.state.settings = Object.assign({}, session.state.settings, { media_visibility: "members", media_budget_mb: 51200, media_public_previews_only: false });
  session.emit({ type: "frame", msg: { type: "server_settings_state", settings: session.state.settings } });
  assert.equal(text(byId(doc, "media-admin-settings-msg")), "Saved.");
  // The meter.
  assert.equal(text(byId(doc, "media-admin-meter")), "12.4 GB of 50 GB used", "the meter");
  assert.equal(byId(doc, "media-admin-meter-bar").children[0].style.width, "24.8%");
  // The prune lists.
  assert.equal(text(byId(doc, "media-admin-largest").querySelector(".media-admin-row-text")), "Big video · 3 GB · by Ann");
  assert.equal(text(byId(doc, "media-admin-oldest").querySelector(".media-admin-row-text")), "First · 2 MB · by Ann");
  assert.equal(text(byId(doc, "media-admin-least-viewed").querySelector(".media-hint")), "Nothing here.");
  assert.match(text(byId(doc, "media-admin-waiting").querySelector(".media-admin-row-text")), /^1760_wait\.mp4 · 40 MB · by Ann · Deleted /);
  // The suggestions waiting, the suggester named; a decided one is not listed.
  const rows = byId(doc, "media-admin-suggestions").querySelectorAll("[data-suggestion]");
  assert.equal(rows.length, 1);
  assert.match(text(rows[0]), /Builds.*Suggested by Ann.*Ship builds/);
  // The categories, General without Remove.
  const catRows = byId(doc, "media-admin-cat-rows").querySelectorAll("[data-cat-row]");
  assert.equal(catRows.length, 2);
  assert.equal(catRows[0].querySelector("[data-cat-remove]"), null, "General stays");
  assert.ok(catRows[1].querySelector("[data-cat-remove]"));
});

// Seen red with categoriesBody re-serialising the list with its keys sorted, which the relay
// would hash as a different list: '[{"description":"","id":1,"name":"General"},...' ==
// '[{"id":1,"name":"General","description":""},...'.
test("9b. the admins' writes are each signed over their own target", async () => {
  const { doc, net, confirms } = await openAdmin();
  // Rename, add, reorder, then save: signed over the hash of the list text exactly as sent.
  const name1 = byId(doc, "media-admin-cat-rows").querySelector('[data-cat-name="1"]');
  name1.value = "Fire flow";
  name1.dispatch("input");
  byId(doc, "media-admin-cat-new-name").value = "Builds";
  byId(doc, "media-admin-cat-add").click();
  byId(doc, "media-admin-cat-rows").querySelector('[data-cat-up="2"]').click();
  byId(doc, "media-admin-cat-save").click();
  await settle();
  const save = net.calls.find((c) => c.url === "/api/media/categories");
  const raw = save.init.body;
  const listText = raw.slice(raw.indexOf(',"categories":') + ',"categories":'.length, -1);
  assert.equal(listText, '[{"id":1,"name":"General","description":""},{"name":"Builds","description":""},{"id":2,"name":"Fire flow","description":""}]');
  const hash = Buffer.from(await BLAKE(new TextEncoder().encode(listText))).toString("hex");
  assert.equal(save.body.sig, "sig:media_categories:" + hash, "the signature covers the list exactly as sent");
  assert.equal(text(byId(doc, "media-admin-cat-msg")), "Saved.");
  // Accept a suggestion.
  byId(doc, "media-admin-suggestions").querySelector('[data-decide="accept"]').click();
  await settle();
  const decide = net.calls.find((c) => c.url === "/api/media/suggestions/decide");
  assert.equal(decide.body.sig, "sig:media_suggestions:3:accept");
  assert.equal(decide.body.id, 3);
  assert.equal(decide.body.accept, true);
  // Remove an upload waiting to be published: by its file name.
  byId(doc, "media-admin-waiting").querySelector(".media-btn-danger").click();
  await settle();
  assert.equal(confirms.at(-1), '"1760_wait.mp4"\n\nRemove this upload and delete the file.');
  const rm = net.calls.find((c) => c.url === "/api/media/remove");
  assert.equal(rm.body.sig, "sig:media_remove:1760_wait.mp4");
  assert.equal(rm.body.filename, "1760_wait.mp4");
  assert.equal(rm.body.id, undefined);
  // The reads.
  assert.equal(net.calls.find((c) => c.url === "/api/media/usage").body.sig, "sig:media_usage");
  assert.equal(net.calls.find((c) => c.url === "/api/media/suggestions").body.sig, "sig:media_suggestions");
});

// Seen red with renderAdmin mounting for any signed-in person: "a member who is not an admin
// gets no admins' section / actual: [object]".
test("9c. the Media page shows the admins' section to admins only", async () => {
  const mounted = [];
  const member = await openPage({ mountAdmin: (c) => mounted.push(c), routes: listRoutes(listAnswer([])) });
  assert.equal(byId(member.doc, "media-admin"), null, "a member who is not an admin gets no admins' section");
  assert.equal(mounted.length, 0);
  const op = await openPage({ session: fakeSession({ role: "owner" }), mountAdmin: (c) => mounted.push(c), routes: listRoutes(listAnswer([])) });
  assert.ok(byId(op.doc, "media-admin"));
  assert.equal(mounted.length, 1);
  assert.equal(mounted[0].id, "media-admin-body");
});

// ── 10. The session ──────────────────────────────────────────────────────

let NOBLE = null;
async function noble() {
  if (NOBLE) return NOBLE;
  const src = fs.readFileSync(path.join(WEB, "shared", "vendor", "noble-pq.bundle.js"));
  NOBLE = await import("data:text/javascript;base64," + Buffer.from(src).toString("base64"));
  return NOBLE;
}

function fakeWebSocketClass(sockets) {
  class FakeWS {
    constructor(url) { this.url = url; this.readyState = 0; this.sent = []; this.listeners = {}; sockets.push(this); }
    addEventListener(t, f) { (this.listeners[t] = this.listeners[t] || []).push(f); }
    fire(t, ev) { for (const f of this.listeners[t] || []) f(ev || {}); }
    send(s) { if (this.readyState !== 1) throw new Error("send on a socket that is not open"); this.sent.push(JSON.parse(s)); }
    close() { if (this.readyState === 3) return; this.readyState = 3; this.fire("close"); }
    open() { this.readyState = 1; this.fire("open"); }
    receive(o) { this.fire("message", { data: JSON.stringify(o) }); }
  }
  return FakeWS;
}

// Seen red with identifyPreimage leaving out the nonce: "the relay accepts the sign-in / actual:
// false"; and with takePeerList reading `token`: "the upload token from the person's own entry
// / '' !== 'tok-9'". Round 2: seen red with the role_list branch taken out of onFrame: "the role
// list is kept".
test("10. the session signs in with a real Dilithium3 signature and takes its own token, role and settings", async () => {
  const n = await noble();
  const seed = new Uint8Array(32).fill(7);
  const kp = n.ml_dsa65.keygen(n.blake3.create({ context: new TextEncoder().encode("hum/dilithium3/v1"), dkLen: 32 }).update(seed).digest());
  const identity = { dilithiumPublicHex: Buffer.from(kp.publicKey).toString("hex"), dilithiumSecret: kp.secretKey };
  const sockets = [];
  const storage = new Map([["humanity_name", "Shaostoul"]]);
  const s = relaySession.createRelaySession({
    getIdentity: async () => identity,
    signBytes: async (secret, bytes) => n.ml_dsa65.sign(bytes, secret),
    WebSocket: fakeWebSocketClass(sockets),
    location: { protocol: "https:", host: "united-humanity.us" },
    storage: { getItem: (k) => storage.get(k) || null },
    setTimeout: () => 0, clearTimeout: () => {},
  });
  const events = [];
  s.subscribe((ev) => events.push(ev));
  assert.equal(await s.start(), true);
  const ws = sockets[0];
  assert.equal(ws.url, "wss://united-humanity.us/ws");
  ws.open();
  assert.deepEqual(ws.sent, [{ type: "identify", public_key: identity.dilithiumPublicHex, display_name: "Shaostoul" }]);
  assert.equal(s.send({ type: "anything" }), false, "nothing else is sent before the relay accepts the sign-in");
  ws.receive({ type: "identify_challenge", nonce: "n0nce" });
  await settle();
  const resp = ws.sent[1];
  assert.equal(resp.type, "identify_response");
  const words = new TextEncoder().encode("hum/identify/v1\nn0nce\n" + identity.dilithiumPublicHex);
  assert.equal(n.ml_dsa65.verify(Buffer.from(resp.sig_b64, "base64"), words, kp.publicKey), true, "the relay accepts the sign-in");
  assert.equal(s.state.uploadToken, "");
  ws.receive({ type: "peer_list", peers: [
    { public_key: "f0".repeat(32), role: "admin", upload_token: null },
    { public_key: identity.dilithiumPublicHex, role: "admin", upload_token: "tok-9", upload_limit_mb: 100 },
  ] });
  assert.equal(s.state.status, "bound");
  assert.equal(s.state.uploadToken, "tok-9", "the upload token from the person's own entry");
  assert.equal(s.state.role, "admin");
  assert.equal(s.state.uploadLimitBytes, 100 * MIB);
  assert.deepEqual(ws.sent[2], { type: "server_settings_request" }, "it asks for the server's settings");
  ws.receive({ type: "server_settings_state", settings: { media_visibility: "members", media_per_person_mb: 64 } });
  assert.equal(s.state.settings.media_per_person_mb, 64);
  // The role list comes with the settings: what the person's role allows (can it upload).
  assert.equal(s.roleDef(), null, "nothing known before the role list");
  ws.receive({ type: "role_list", roles: [{ id: "verified", can_upload: true }, { id: "admin", can_upload: false }] });
  assert.ok(Array.isArray(s.state.roles), "the role list is kept");
  assert.deepEqual(s.roleDef(), { id: "admin", can_upload: false }, "the person's own role");
  // A later peer list (someone joining) without a token keeps the token.
  ws.receive({ type: "peer_list", peers: [{ public_key: identity.dilithiumPublicHex, role: "admin" }] });
  assert.equal(s.state.uploadToken, "tok-9");
  // A dropped socket's token is gone.
  ws.close();
  assert.equal(s.state.status, "closed");
  assert.equal(s.state.uploadToken, "");
  assert.ok(events.some((e) => e.type === "frame" && e.msg.type === "server_settings_state"));

  // No identity here: no socket at all.
  const none = [];
  const empty = relaySession.createRelaySession({ getIdentity: async () => null, WebSocket: fakeWebSocketClass(none), location: { protocol: "https:", host: "x" } });
  assert.equal(await empty.start(), false);
  assert.equal(none.length, 0, "no identity, no socket");
  assert.equal(empty.state.status, "no_identity");
});

// ── The words ────────────────────────────────────────────────────────────

// Seen red with sizeWords keeping the ".0": "the design's own example / '12.4 GB of 50.0 GB used'".
test("11. sizes, the share and the meter read as the design writes them", () => {
  assert.equal(media.meterWords(Math.round(12.4 * 1024 * MIB), 50 * 1024 * MIB), "12.4 GB of 50 GB used", "the design's own example");
  assert.equal(media.shareLeftWords(100 * MIB, 512 * MIB), "You have 412 MB of your 512 MB left");
  assert.equal(media.shareLeftWords(600 * MIB, 512 * MIB), "You have 0 bytes of your 512 MB left");
  assert.equal(media.sizeWords(1.5 * MIB), "1.5 MB");
  assert.equal(media.sizeWords(900 * 1024), "900 KB");
  assert.equal(media.shareStop({ name: "a", size: 10 }, 0, 512 * MIB), null);
  assert.equal(media.limitStop({ name: "a", size: 2 * MIB }, MIB).startsWith('"a" is 2 MB, over your role\'s 1 MB limit for one Media file'), true);
  assert.equal(media.limitStop({ name: "a", size: 2 * MIB }, 0), null, "0 is no limit: up to the share");
  // No em dash in anything the page or the section says.
  for (const w of Object.values(media.MEDIA_WORDS).concat(Object.values(admin.ADMIN_WORDS))) assert.ok(!w.includes("—"), w);
});

// The page is wired in: the nav beside Watch, its scripts in order, the Shared Files row link.
// Seen red with the /media link taken out of shell.js: "the nav links the page / false".
test("12. the page is in the nav beside Watch and loads its scripts in order", () => {
  const shell = fs.readFileSync(path.join(WEB, "shared", "shell.js"), "utf8");
  assert.ok(/mobileLink\('\/watch',\s*'Watch'\)\s*\+[\s\S]{0,300}?mobileLink\('\/media',\s*'Media'\)/.test(shell), "the nav links the page");
  assert.ok(/p\.startsWith\('\/media'\)/.test(shell), "the page lights its tab");
  const html = fs.readFileSync(path.join(WEB, "pages", "media.html"), "utf8");
  const order = ["/shared/hold-confirm.js", "/chat/pq.js", "/shared/pq-relay-auth.js", "/shared/relay-session.js", "/shared/report.js", "/pages/media-upload.js", "/pages/media-app.js", "/pages/media-admin.js"];
  const at2 = order.map((s) => html.indexOf('src="' + s));
  assert.ok(at2.every((i) => i > 0), "every script is loaded: " + at2);
  assert.deepEqual(at2.slice().sort((a, b) => a - b), at2, "in order");
  assert.ok(html.includes('id="media-root"'));
  // The Admin page no longer mounts the section (round 5, lane 1F): its Media panel is one line
  // linking /media#admin and it loads none of these scripts, which server-strip-web.test.js checks.
  const shared = fs.readFileSync(path.join(WEB, "pages", "shared-files.html"), "utf8");
  assert.ok(shared.includes("'/media?put=' + encodeURIComponent("), "a Shared Files row links to Put on Media");
  assert.ok(/f\.uploader_key !== myKey/.test(shared), "on the person's own rows only");
});

// ── 13. Add to Media in three tinted parts, each file's type shown (2026-10-17) ──

const vm = require("node:vm");
const FRISK = require(path.join(WEB, "shared", "file-risk.js"));
const RISK_LIST = JSON.parse(fs.readFileSync(path.join(H.ROOT, "data", "safety", "file_risks.json"), "utf8"));
const LICENCES = JSON.parse(fs.readFileSync(path.join(H.ROOT, "scripts", "tests", "fixtures", "media-round5-v1.json"), "utf8")).licences;
/** The tinted parts directly in `el`. */
const partsOf = (el) => el.children.filter((c) => String(c.className).split(/\s+/).includes("tint-section"));
/** Is `child` inside `el`? */
const within = (el, child) => { for (let n = child; n; n = n.parentNode) if (n === el) return true; return false; };
/** A file-kinds route that answers only once released; its hold ends with the test (`t.after`). */
function lateList(t) {
  let release = () => {};
  const gate = new Promise((r) => { release = r; });
  const late = { asked: 0, release };
  late.route = [at("/data/safety/file_risks.json"), async () => { late.asked++; await gate; return respond(200, RISK_LIST); }];
  t.after(async () => { release(); for (let i = 0; i < 4; i++) await settle(); });
  return late;
}

// The operator asked for "the RGB aesthetic for the different sections of the interface like how
// the DMs, Groups, and servers in chat are but darker RGB in the pop up window"; and every file's
// type, always ("protect people inexperienced with technology and possibly illiterate people"). As
// the desktop's Add to Media (media/add.rs): "Files" (blue) first, holding each chosen file with its
// type's badge and size and the share left; then "About it" (red) and "Who can see it" (green); the
// buttons under them, outside. Each badge is a grey "File" until the list of kinds is in, asked for
// once, then the file's type.
// Seen red 2026-10-17, each in a copy of web/ through HOS_WEB_DIR: with fileLine drawing no badge:
// "the chosen file: its badge first"; with About it and Who can see it put before Files: "the three
// parts, Files first" (red, green, blue against blue, red, green); with askFileTypes never asking:
// "the list is asked for once" (0 against 1).
test("13. Add to Media is in three tinted parts, its files first; each chosen file shows its type", async (t) => {
  FRISK.use(null); // not in yet: the form asks for it
  const late = lateList(t);
  const { doc } = await openPage({ routes: listRoutes(listAnswer([], { licences: LICENCES }), [
    [at("/api/media/mine", "POST"), () => respond(200, mineAnswer())],
    late.route,
  ]) });
  const input = byId(doc, "media-file-input");
  input.files = [aFile("model.stl", 10 * MIB, "model/stl"), aFile("setup.exe", MIB, "application/octet-stream")];
  input.dispatch("change");
  await settle();
  const panel = byId(doc, "media-form").children[0];
  assert.deepEqual(partsOf(panel).map((p) => [p.className, text(p.children[0])]), [
    ["tint-section tint-blue", "Files"], ["tint-section tint-red", "About it"], ["tint-section tint-green", "Who can see it"],
  ], "the three parts, Files first");
  const [files, about, who] = partsOf(panel);
  for (const p of partsOf(panel)) assert.equal(p.children[0].className, "tint-section-title", "each with its title on top");
  const line = files.querySelector(".media-form-file");
  assert.ok(line && line.children[0] && line.children[0].className.startsWith("file-badge"), "the chosen file: its badge first");
  assert.equal(line.children[0].className, "file-badge level-unknown", "a grey File before the list is in");
  assert.equal(text(line.children[1]), "model.stl (10 MB)", "then its name and size");
  const moreRows = byId(doc, "media-form-more").querySelectorAll(".media-form-more-file");
  assert.equal(moreRows.length, 1, "the further file, on its own row");
  assert.ok(within(files, moreRows[0]), "in the Files part");
  assert.equal(text(moreRows[0].children[1]), "setup.exe (1 MB)");
  assert.ok(within(files, byId(doc, "media-share-line")), "the share left is in the Files part");
  for (const id of ["media-form-title", "media-form-category", "media-form-description", "media-form-labels"]) assert.ok(within(about, byId(doc, id)), "About it holds #" + id);
  for (const id of ["media-form-visibility", "media-form-licence"]) assert.ok(within(who, byId(doc, id)), "Who can see it holds #" + id);
  assert.ok(within(who, panel.querySelector("[data-hosting]")), "Who can see it holds the hosting paragraph");
  for (const id of ["media-form-go", "media-form-cancel"]) {
    assert.ok(byId(doc, id) && partsOf(panel).every((p) => !within(p, byId(doc, id))), "#" + id + " is outside the parts");
  }
  assert.ok(panel.children.indexOf(who) < panel.children.indexOf(byId(doc, "media-form-actions")), "the buttons under them");
  assert.equal(late.asked, 1, "the list is asked for once");

  late.release();
  for (let i = 0; i < 4; i++) await settle();
  const first = files.querySelector(".media-form-file").children[0];
  assert.equal(first.className, "file-badge level-view", "a 3D model, once the list is in: fine to look at");
  assert.equal(text(first), "3D model.stl");
  const program = byId(doc, "media-form-more").querySelector(".media-form-more-file").children[0];
  assert.equal(program.className, "file-badge level-danger", "a program: red");
  assert.equal(text(program), "Program.exe");
  assert.equal(program.title, "Program");
  assert.equal(late.asked, 1, "and not asked again");
});

// ── 14. Shared Files: each file's type before its name (2026-10-17) ─────

/**
 * Run the Shared Files page's own script (web/pages/shared-files.html, its one inline <script>)
 * against a stand-in page: its list element (whose innerHTML is what it draws), its search box, the
 * server's list of uploads (`files`) and HosFileRisk (`risk`).
 */
function sharedFilesPage(files, risk) {
  const html = fs.readFileSync(path.join(WEB, "pages", "shared-files.html"), "utf8");
  const src = /<script>([\s\S]*?)<\/script>/.exec(html)[1];
  const els = { "sf-list": { innerHTML: "", querySelectorAll: () => [] }, "sf-q": { value: "", addEventListener() {} } };
  const asked = [];
  const ctx = {
    document: { getElementById: (id) => els[id] || null, createElement: () => ({}) },
    fetch: async (u) => { asked.push(String(u)); return { ok: true, json: async () => ({ files }) }; },
    HosFileRisk: risk, console, Promise,
  };
  ctx.window = ctx;
  vm.runInNewContext(src, ctx);
  return { list: els["sf-list"], asked };
}

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR: with the row drawn without typeBadge
// (its old text box of the ending): "each row: its badge, then its name"; with the rows not drawn
// again once the list is in: "bracket.stl, once the list is in: a 3D model's badge".
test("14. Shared Files: each row shows its file's type just before its name, drawn again once the list is in", async () => {
  FRISK.use(null);
  let loaded = false;
  let release = () => {};
  const gate = new Promise((r) => { release = r; });
  const risk = {
    ready: () => loaded,
    load: () => gate.then(() => { loaded = true; return RISK_LIST; }),
    badgeHtml: (name, head) => FRISK.badgeHtml(name, head, loaded ? RISK_LIST : undefined),
  };
  const files = [
    { name: "bracket.stl", url: "/uploads/1_bracket.stl", size_bytes: 2048, uploaded_at: 1, uploader_name: "Ann" },
    { name: '<b>"odd"</b>.zip', url: "/uploads/2_odd.zip", size_bytes: 10, uploaded_at: 1, uploader_name: "Ben" },
  ];
  const page = sharedFilesPage(files, risk);
  await settle();
  assert.deepEqual(page.asked, ["/api/uploads?limit=500"], "test setup: the page asked for the list of uploads");
  const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
  const rowName = (name) => '<div class="sf-name">' + FRISK.badgeHtml(name, null, loaded ? RISK_LIST : undefined) + '<span class="sf-name-text">' + esc(name) + "</span></div>";
  for (const f of files) assert.ok(page.list.innerHTML.includes(rowName(f.name)), "each row: its badge, then its name: " + f.name);
  assert.ok(page.list.innerHTML.includes('class="file-badge level-unknown"'), "a grey File before the list is in");
  assert.ok(!page.list.innerHTML.includes("sf-icon"), "the old box of the file's ending is gone: the badge says it");
  assert.ok(!page.list.innerHTML.includes("<b>"), "a name is never markup");
  release();
  await settle();
  assert.ok(page.list.innerHTML.includes(rowName("bracket.stl")) && rowName("bracket.stl").includes('class="file-badge level-view" data-file-badge="bracket.stl" data-file-type="model"'),
    "bracket.stl, once the list is in: a 3D model's badge");
  assert.ok(page.list.innerHTML.includes(rowName(files[1].name)) && rowName(files[1].name).includes("level-care"), "an archive: take care");
});

// ── 15. The badge's and the parts' styles, and every page that shows them (2026-10-17) ──

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR: with file-badge.css's danger level on a
// colour theme.css does not have (var(--dangerous)): "file-badge.css: every colour and size is a
// theme token: --dangerous"; with the stylesheet link taken out of shared-files.html: "pages/shared-
// files.html links /shared/file-badge.css".
test("15. the badge and the tinted parts use theme tokens only, and every page that shows them loads them", () => {
  const theme = fs.readFileSync(path.join(WEB, "shared", "theme.css"), "utf8");
  const defined = new Set([...theme.matchAll(/(--[\w-]+)\s*:/g)].map((m) => m[1]));
  const badgeCss = fs.readFileSync(path.join(WEB, "shared", "file-badge.css"), "utf8");
  const tintCss = fs.readFileSync(path.join(WEB, "shared", "tint-section.css"), "utf8");
  for (const [name, css] of [["file-badge.css", badgeCss], ["tint-section.css", tintCss]]) {
    const own = new Set([...css.matchAll(/(--[\w-]+)\s*:/g)].map((m) => m[1]));
    for (const m of css.matchAll(/var\((--[\w-]+)/g)) {
      assert.ok(defined.has(m[1]) || own.has(m[1]), name + ": every colour and size is a theme token: " + m[1]);
    }
  }
  const rule = (css, sel) => { const m = new RegExp(sel.replace(/[.*+?^${}()|[\]\\]/g, "\\$&") + "\\s*\\{([^}]*)\\}").exec(css); return m ? m[1].replace(/\s+/g, " ").trim() : null; };
  assert.equal(rule(badgeCss, ".file-badge.level-view"), "--badge: var(--success);");
  assert.equal(rule(badgeCss, ".file-badge.level-care"), "--badge: var(--warning);");
  assert.equal(rule(badgeCss, ".file-badge.level-danger"), "--badge: var(--danger);");
  assert.equal(rule(badgeCss, ".file-badge.level-unknown"), "--badge: var(--text-muted);");
  assert.ok(/border-width:\s*2px/.test(rule(badgeCss, ".file-badge.disguised")), "a disguised file's badge: a thicker edge");
  assert.equal(rule(tintCss, ".tint-section.tint-red"), "--tint: var(--dm-bg);", "red is the DMs' colour");
  assert.equal(rule(tintCss, ".tint-section.tint-green"), "--tint: var(--group-bg);", "green the groups'");
  assert.equal(rule(tintCss, ".tint-section.tint-blue"), "--tint: var(--server-bg);", "blue the servers'");
  const section = rule(tintCss, ".tint-section");
  assert.ok(section.includes("background: color-mix(in srgb, var(--tint) 70%, transparent);") && section.includes("border: 1px solid var(--tint);"), "darker, edged in the colour itself: " + section);
  const page = (rel) => fs.readFileSync(path.join(WEB, rel), "utf8");
  for (const rel of ["chat/index.html", "pages/media.html", "pages/shared-files.html"]) {
    const html = page(rel);
    assert.ok(/<link rel="stylesheet" href="\/shared\/file-badge\.css\?v=\d+">/.test(html), rel + " links /shared/file-badge.css");
    assert.ok(/<script src="\/shared\/file-risk\.js\?v=\d+"><\/script>/.test(html), rel + " loads /shared/file-risk.js");
  }
  for (const rel of ["chat/index.html", "pages/media.html"]) {
    assert.ok(/<link rel="stylesheet" href="\/shared\/tint-section\.css\?v=\d+">/.test(page(rel)), rel + " links /shared/tint-section.css");
  }
  const media = page("pages/media.html");
  assert.ok(media.indexOf("/shared/file-risk.js") < media.indexOf("/pages/media-edit.js"), "file-risk.js before media-edit.js, which draws the badges");
  const shared = page("pages/shared-files.html");
  assert.ok(shared.indexOf("/shared/file-risk.js") < shared.indexOf("<script>"), "and before the Shared Files page's own script");
});
