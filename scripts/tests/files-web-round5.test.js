// The Media page on the website, round 5's list (2026-10-12): docs/design/media-round-5-plan.md,
// lane 1G ("web list", section 5, Wave 1), built to sections 3.3 (the list's wire: sort, cursor,
// the featured shelf per category), 3.6 (an item's own address, /files/<id>) and 3.10 (the
// website), with decisions 7 and 13 of section 9 (the title names the host; the next page loads on
// scroll AND by a More button) and the operator's answers of section 10 (featured items may show
// twice; a shelf per category). The web half of the desktop's lane 1C.
//
// Run: node --test scripts/tests/files-web-round5.test.js
//
// Against the stand-ins in scripts/tests/files-web-harness.js (a document that holds only what the
// page built, where reading or setting innerHTML throws; a network that records every request;
// signing that returns "sig:" plus the words signed), plus two of this file's own: a history that
// records each pushState, replaceState and back, and an IntersectionObserver whose reports the
// test makes. The list's answers and the feature purposes come from the shared fixture the relay
// makes, scripts/tests/fixtures/files-round5-v1.json. HOS_WEB_DIR points the test at another copy
// of web/ (used to see each test red). Nothing here depends on /shared/server-strip.js (lane 1F's).
//
// What it proves:
//  1. The h1 and the tab's title name the host: "Media on united-humanity.us".
//  2. The Media and Admin tabs are drawn for an admin only; Admin holds the admins' section, and
//     /files#admin opens it.
//  3. A Sort change asks for the list afresh with `sort` (no cursor), and the next page sends back
//     that answer's `next_cursor` with the same sort.
//  4. The sentinel after the grid coming into view asks for the next page once, however many
//     times it reports while that page is on its way; it is watched with a 600 px margin.
//  5. With a category picked, the shelf is that category's (`featured=1&category=c`), headed
//     "Featured in <its name>"; with none, the main shelf.
//  6. The picked category's description shows under the filters, as text.
//  7. /files/12 opens item 12 (the list route asked for `id=12`), without pushing anything.
//  8. Opening a tile pushes /files/<id> onto the history and names the item in the title; Close
//     goes Back; Back and Forward (`route`) show what the address names.
//  9. Report links a Media item as /files/<n>, an address the page opens.
// 10. Feature is three ways for admins: not featured, the main shelf, or its category's shelf.
//
// Seen red 2026-10-12: each with one break made in a copy of web/ (pages, shared, chat) under
// HOS_WEB_DIR, then green again on the real web/: see the note above each test.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const H = require("./files-web-harness.js");
const {
  ROOT, media, report, ANN, byId, text, respond, path0, at, queryOf, fakeSession, settle, item, listAnswer,
  listRoutes, openPage,
} = H;

const FX = JSON.parse(fs.readFileSync(path.join(ROOT, "scripts", "tests", "fixtures", "files-round5-v1.json"), "utf8"));
// The relay's own categories, one with a description (General has none).
const CATS = FX.list_by_downloads.categories;

const tileOf = (doc, id) => byId(doc, "files-grid").querySelectorAll(".files-tile").find((t) => t.dataset.id === String(id));
const notice = (doc) => text(byId(doc, "files-notice").querySelector(".files-notice-text"));
const choose = (doc, id, value) => { const s = byId(doc, id); s.value = value; s.dispatch("change"); };
/** The list's own requests (not the featured shelf's). */
const lists = (net) => net.calls.filter((c) => c.url === "/api/files/list" && c.body.featured !== 1);
/** The shelf's requests, signed or not. */
const shelves = (net) => net.calls.filter((c) => (c.url === "/api/files/list" && c.body.featured === 1) ||
  (path0(c.url) === "/api/files" && queryOf(c.url).get("featured") === "1"));

/** A history that records what the page asks of it. */
function fakeHistory() {
  const calls = [];
  return {
    calls,
    pushState: (_s, _t, url) => calls.push(["push", url]),
    replaceState: (_s, _t, url) => calls.push(["replace", url]),
    back: () => calls.push(["back"]),
  };
}

/** An IntersectionObserver whose reports the test makes (`report(true)`: the sentinel came into view). */
function fakeObservers() {
  const made = [];
  class FakeIO {
    constructor(cb, opts) { this.cb = cb; this.opts = opts; this.watching = new Set(); this.observed = 0; made.push(this); }
    observe(el) { this.watching.add(el); this.observed++; }
    unobserve(el) { this.watching.delete(el); }
    disconnect() { this.watching.clear(); }
    report(isIntersecting) { this.cb(Array.from(this.watching).map((target) => ({ target, isIntersecting })), this); }
  }
  return { FakeIO, made };
}

// ── 1. The heading names the server ──────────────────────────────────────

// Seen red 2026-10-12 with build()'s h1 back to the word "Media": "the h1 names the host /
// actual: 'Media' / expected: 'Media on united-humanity.us'".
test("1. the h1 and the tab's title name the host", async () => {
  const { doc } = await openPage({ routes: listRoutes(listAnswer([item({ id: 1 })])) });
  const h1 = byId(doc, "files-title");
  assert.equal(h1 && h1.tagName, "H1");
  assert.equal(text(h1), "Files on united-humanity.us", "the h1 names the host");
  assert.equal(doc.title, "Files on united-humanity.us", "and so does the tab's title");
});

// ── 2. The Admin place, for admins only ──────────────────────────────────

// Seen red 2026-10-12 with renderTabs drawing the tabs for anyone (`const admin = true`): "a
// member gets no tabs / actual: [FakeElement] / expected: null". The tabs are places since
// 2026-10-19 (files-places.js): everyone has Shared on <server> and Shared files; the server's
// data folder (since 2026-10-20, BUG-206) and Admin are an admin's alone.
test("2. the Admin place is an admin's alone; Admin holds the section, /files#admin opens it", async () => {
  const mounted = [];
  const placeNames = (doc) => byId(doc, "files-places").children.filter((c) => c.tagName === "BUTTON").map((t) => t.textContent);
  const member = await openPage({ hash: "#admin", mountAdmin: (c) => mounted.push(c), routes: listRoutes(listAnswer([])) });
  assert.deepEqual(placeNames(member.doc), ["Shared on united-humanity.us", "Shared files"], "a member's places: no data folder, no Admin");
  assert.equal(byId(member.doc, "files-place-admin"), null, "and no Admin place, even at /files#admin");
  assert.equal(byId(member.doc, "files-main").hidden, false, "the list shows");
  assert.equal(byId(member.doc, "files-admin"), null, "and no admins' section");
  assert.equal(mounted.length, 0);

  const history = fakeHistory();
  const op = await openPage({ session: fakeSession({ role: "admin" }), history, mountAdmin: (c) => mounted.push(c), routes: listRoutes(listAnswer([])) });
  assert.deepEqual(placeNames(op.doc), ["Shared on united-humanity.us", "Shared files", "Admin", "The server's data folder"], "an admin's places, in the desktop's order");
  assert.equal(byId(op.doc, "files-place-shared").getAttribute("aria-current"), "page", "what is shared first");
  assert.equal(byId(op.doc, "files-main").hidden, false);
  assert.equal(byId(op.doc, "files-admin-wrap").hidden, true, "Admin waits behind its place");
  assert.equal(mounted.length, 1, "the section is mounted once");
  assert.equal(mounted[0].id, "files-admin-body");
  byId(op.doc, "files-place-admin").click();
  assert.equal(byId(op.doc, "files-main").hidden, true, "Admin hides the list");
  assert.equal(byId(op.doc, "files-admin-wrap").hidden, false, "and shows the section");
  assert.equal(byId(op.doc, "files-place-admin").getAttribute("aria-current"), "page");
  assert.deepEqual(history.calls.at(-1), ["replace", "/files#admin"], "the address says so, in place");
  byId(op.doc, "files-place-shared").click();
  assert.equal(byId(op.doc, "files-main").hidden, false);
  assert.deepEqual(history.calls.at(-1), ["replace", "/files"]);
  assert.equal(mounted.length, 1, "switching places mounts nothing again");

  // /files#admin opens the Admin place for an admin (Server Settings and /admin link there).
  const direct = await openPage({ hash: "#admin", session: fakeSession({ role: "owner" }), mountAdmin: () => {}, routes: listRoutes(listAnswer([])) });
  assert.equal(byId(direct.doc, "files-admin-wrap").hidden, false, "/files#admin opens Admin");
  assert.equal(byId(direct.doc, "files-main").hidden, true);
});

// ── 3. Sort ──────────────────────────────────────────────────────────────

// Seen red 2026-10-12 with the Sort select's change asking for the next page instead of afresh
// (`st.filters.sort = sort.value; load(false)`): "a new sort asks afresh, with no cursor / actual:
// '9.9' / expected: undefined".
test("3. a Sort change asks afresh with `sort`, and the next page sends back its `next_cursor`", async () => {
  const newest = listAnswer([item({ id: 9, title: "Newest" })], { next_cursor: "9.9", categories: CATS });
  const { doc, net } = await openPage({ routes: listRoutes(newest, [
    [(c) => c.url === "/api/files/list" && c.body.featured !== 1 && c.body.sort === "downloads",
      (c) => respond(200, c.body.cursor ? listAnswer([], { categories: CATS }) : FX.list_by_downloads)],
  ]) });
  assert.deepEqual(byId(doc, "files-filter-sort").children.map((o) => o.textContent), ["Newest", "Most picked", "Most downloaded"]);
  assert.equal(byId(doc, "files-filter-sort").value, "new", "newest first to begin with");
  assert.equal(lists(net)[0].body.sort, undefined, "newest first is the relay's default, not sent");
  choose(doc, "files-filter-sort", "downloads");
  await settle();
  const sorted = lists(net).at(-1);
  assert.equal(sorted.body.sort, "downloads", "the list is asked for with the sort");
  assert.equal(sorted.body.cursor, undefined, "a new sort asks afresh, with no cursor");
  assert.equal(sorted.body.sig, "sig:media_list");
  assert.deepEqual(byId(doc, "files-grid").querySelectorAll(".files-tile").map((t) => t.dataset.id), ["1"], "the sorted list replaces the old one");
  assert.equal(byId(doc, "files-filter-sort").value, "downloads", "the select keeps the choice");
  byId(doc, "files-more").click();
  await settle();
  const next = lists(net).at(-1);
  assert.deepEqual([next.body.sort, next.body.cursor], ["downloads", FX.list_by_downloads.next_cursor], "the next page: the same sort, the answer's cursor");
});

// ── 4. Loading on scroll ─────────────────────────────────────────────────

// Seen red 2026-10-12 with more() asking without its in-flight guard (`st.loading ||` taken out):
// "the next page is asked for once / actual: 3 / expected: 1".
test("4. the sentinel coming into view asks for the next page once; More stays", async () => {
  const io = fakeObservers();
  let release;
  const held = new Promise((r) => { release = r; });
  const first = listAnswer([item({ id: 9, title: "Nine" }), item({ id: 8, title: "Eight" })], { next_cursor: "8.8" });
  const { doc, net } = await openPage({ IntersectionObserver: io.FakeIO, routes: listRoutes(first, [
    [(c) => c.url === "/api/files/list" && c.body.cursor === "8.8", async () => { await held; return respond(200, listAnswer([item({ id: 7, title: "Seven" })])); }],
  ]) });
  assert.equal(io.made.length, 1, "one observer");
  const watch = io.made[0];
  assert.equal(watch.opts.rootMargin, "600px", "the next page is asked for 600 px before the end shows");
  assert.deepEqual(Array.from(watch.watching).map((e) => e.id), ["files-sentinel"], "it watches the sentinel");
  const sentinel = byId(doc, "files-sentinel");
  const main = byId(doc, "files-main");
  assert.ok(main.children.indexOf(sentinel) > main.children.indexOf(byId(doc, "files-grid")), "after the grid");
  assert.ok(watch.observed >= 2, "watched afresh once the first page came, so a short page loads on");
  assert.ok(byId(doc, "files-more"), "More stays, for keyboards");

  watch.report(false);
  await settle();
  assert.equal(lists(net).filter((c) => c.body.cursor === "8.8").length, 0, "out of view: nothing asked");
  watch.report(true);
  watch.report(true);
  watch.report(true);
  await settle();
  assert.equal(lists(net).filter((c) => c.body.cursor === "8.8").length, 1, "the next page is asked for once");
  byId(doc, "files-more").click();
  await settle();
  assert.equal(lists(net).filter((c) => c.body.cursor === "8.8").length, 1, "More while it is on its way asks nothing more");
  release();
  await settle();
  assert.deepEqual(byId(doc, "files-grid").querySelectorAll(".files-tile").map((t) => t.dataset.id), ["9", "8", "7"], "the next page is added below");
  const asked = lists(net).length;
  watch.report(true);
  await settle();
  assert.equal(lists(net).length, asked, "nothing is asked after the last page");
  assert.equal(byId(doc, "files-more"), null);
});

// ── 5. The shelf per category ────────────────────────────────────────────

// Seen red 2026-10-12 with loadFeatured asking for the main shelf whatever was picked
// (`listParams({ featured: 1 }, ...)` again): "the shelf is asked for with the category /
// actual: undefined / expected: 2".
test("5. with a category picked, the shelf is that category's: featured=1&category=c", async () => {
  const answer = listAnswer([item({ id: 9, title: "On the main shelf", featured: "main" }), item({ id: 5, title: "Staff spin", featured: "category" })], { categories: CATS });
  const { doc, net } = await openPage({ routes: listRoutes(answer, [
    [(c) => c.url === "/api/files/list" && c.body.featured === 1 && c.body.category === 2,
      () => respond(200, listAnswer([item({ id: 5, title: "Staff spin", featured: "category" })], { categories: CATS }))],
  ]) });
  const shelf = () => byId(doc, "files-featured-row").querySelectorAll(".files-tile").map((t) => t.dataset.id);
  assert.equal(text(byId(doc, "files-featured-title")), "Featured", "the main shelf to begin with");
  assert.deepEqual(shelf(), ["9"]);
  assert.equal(shelves(net)[0].body.category, undefined, "the main shelf names no category");
  choose(doc, "files-filter-category", "2");
  await settle();
  const asked = shelves(net).at(-1);
  assert.equal(asked.body.featured, 1);
  assert.equal(asked.body.category, 2, "the shelf is asked for with the category");
  assert.equal(text(byId(doc, "files-featured-title")), "Featured in Fire flow arts", "headed with its name");
  assert.deepEqual(shelf(), ["5"], "that category's shelf");
  assert.equal(lists(net).at(-1).body.category, 2, "and the list is filtered too");
  choose(doc, "files-filter-category", "");
  await settle();
  assert.equal(shelves(net).at(-1).body.category, undefined, "All categories: the main shelf again");
  assert.equal(text(byId(doc, "files-featured-title")), "Featured");

  // Without an identity: the public list, GET /api/files?featured=1&category=2.
  const out = await openPage({ myKey: "", session: null, routes: listRoutes(listAnswer([], { members_view: false, categories: CATS })) });
  choose(out.doc, "files-filter-category", "2");
  await settle();
  const q = queryOf(shelves(out.net).at(-1).url);
  assert.deepEqual([q.get("featured"), q.get("category")], ["1", "2"]);
});

// ── 6. The category's description ────────────────────────────────────────

// Seen red 2026-10-12 with renderFilters leaving the description line empty (its two lines taken
// out): "the picked category's description / actual: '' / expected: 'Staff, poi, fans and the
// rest'".
test("6. the picked category's description shows under the filters, as text", async () => {
  const odd = { id: 3, name: "Odd", description: '<img src=x onerror="alert(1)">', position: 2 };
  const { doc } = await openPage({ routes: listRoutes(listAnswer([], { categories: CATS.concat([odd]) })) });
  const desc = () => byId(doc, "files-category-desc");
  assert.equal(desc().hidden, true, "nothing under the filters with no category picked");
  choose(doc, "files-filter-category", "2");
  await settle();
  assert.equal(text(desc()), "Staff, poi, fans and the rest", "the picked category's description");
  assert.equal(desc().hidden, false);
  const main = byId(doc, "files-main");
  assert.equal(main.children.indexOf(desc()), main.children.indexOf(byId(doc, "files-filters")) + 1, "right under the filters");
  choose(doc, "files-filter-category", "3");
  await settle();
  assert.equal(text(desc()), odd.description, "the server's words as text, never as markup");
  assert.equal(desc().querySelectorAll("img").length, 0);
  choose(doc, "files-filter-category", "1");
  await settle();
  assert.equal(desc().hidden, true, "General has none: nothing shows");
});

// ── 7. /files/12 ─────────────────────────────────────────────────────────

// Seen red 2026-10-12 with start() no longer reading the address (its `itemIdOf(env.path)` lines
// taken out): "the list route is asked for item 12 / actual: undefined / expected: true".
test("7. /files/12 opens item 12, and pushes nothing", async () => {
  const history = fakeHistory();
  const twelve = Object.assign({}, FX.item_members_view, { id: 12, labels: [] });
  const { doc, net } = await openPage({ path: "/files/12", history, routes: listRoutes(listAnswer([]), [
    [(c) => c.url === "/api/files/list" && c.body.id === 12, () => respond(200, listAnswer([twelve]))],
  ]) });
  const ask = net.calls.find((c) => c.url === "/api/files/list" && c.body.id === 12);
  assert.ok(ask, "the list route is asked for item 12");
  assert.equal(ask.body.limit, 1);
  assert.equal(text(byId(doc, "files-viewer-open").querySelector("h2")), "Dusk", "and its viewer opens");
  assert.equal(doc.title, "Dusk, Files on united-humanity.us", "the title names it, as the relay's page does");
  assert.deepEqual(history.calls, [], "the address names it already: nothing pushed");
  byId(doc, "files-viewer-close").click();
  assert.deepEqual(history.calls, [["replace", "/files"]], "closing it leaves /files in its place (there is no step to go Back to)");
  assert.equal(doc.title, "Files on united-humanity.us");
  // Only that shape is an item's address; `?id=` is gone.
  assert.deepEqual(["/files/12", "/files/12/", "/files/x", "/files", "/files/", "/pages/media/3"].map(media.itemIdOf), [12, null, null, null, null, null]);
  const old = await openPage({ path: "/files", routes: listRoutes(listAnswer([])) });
  assert.equal(old.net.calls.filter((c) => c.url === "/api/files/list" && c.body.id).length, 0, "/files opens no item");
});

// ── 8. The history ───────────────────────────────────────────────────────

// Seen red 2026-10-12 with openViewer leaving the address alone (its `go('/files/' + item.id, ...)`
// taken out): "opening a tile pushes its address / actual: [] / expected: [['push', '/files/9']]".
test("8. opening a tile pushes /files/<id>; Close goes Back; Back and Forward show what the address names", async () => {
  const history = fakeHistory();
  const nine = item({ id: 9, title: "Contact staff" });
  const far = item({ id: 3, title: "From long ago", url: "/uploads/3_old.png" });
  const { doc, net, page } = await openPage({ history, routes: listRoutes(listAnswer([nine]), [
    [(c) => c.url === "/api/files/list" && c.body.id === 3, () => respond(200, listAnswer([far]))],
  ]) });
  const byNumber = (id) => net.calls.filter((c) => c.url === "/api/files/list" && c.body.id === id).length;
  tileOf(doc, 9).querySelector(".files-thumb").click();
  assert.deepEqual(history.calls, [["push", "/files/9"]], "opening a tile pushes its address");
  assert.equal(doc.title, "Contact staff, Files on united-humanity.us");
  byId(doc, "files-viewer-close").click();
  assert.deepEqual(history.calls.at(-1), ["back"], "Close goes Back to /files");
  assert.equal(byId(doc, "files-viewer-open"), null);
  // The browser's Back after that lands on /files: nothing more is asked of the history.
  await page.route("/files", "");
  assert.equal(history.calls.length, 2);
  // Forward to /files/9: the item opens from its address, as the list holds it (nothing asked),
  // and nothing is pushed.
  await page.route("/files/9", "");
  await settle();
  assert.equal(text(byId(doc, "files-viewer-open").querySelector("h2")), "Contact staff");
  assert.equal(byNumber(9), 0, "an item the list holds is not asked for again");
  assert.equal(history.calls.length, 2, "Forward pushes nothing");
  // Back again, to /files: the viewer closes, and the address is left as the browser set it.
  await page.route("/files", "");
  assert.equal(byId(doc, "files-viewer-open"), null);
  assert.equal(history.calls.length, 2);
  assert.equal(doc.title, "Files on united-humanity.us");
  // An address naming an item the list does not hold: asked for by its number.
  await page.route("/files/3", "");
  await settle();
  assert.equal(byNumber(3), 1, "the item is asked for by its number");
  assert.equal(text(byId(doc, "files-viewer-open").querySelector("h2")), "From long ago");
  assert.equal(history.calls.length, 2);
});

// ── 9. Report links /files/<n> ───────────────────────────────────────────

// Seen red 2026-10-12 with report.js's reportFilesHref back to "/files?id=": "Report links the
// item's own page / actual: '/files?id=12'".
test("9. Report links a Media item as /files/<n>, which the page opens", () => {
  assert.equal(report.reportFilesHref(12), "/files/12", "Report links the item's own page");
  assert.equal(media.itemIdOf(report.reportFilesHref(12)), 12, "an address the Files page opens");
});

// ── 10. Feature, three ways ──────────────────────────────────────────────

// Seen red 2026-10-12 with the select's change featuring on the main shelf whatever was picked
// (`feature(item, 'main')`): "featured in its category, signed so / actual: [ 'main',
// 'sig:media_feature:1:main', '1760_x.png' ]".
test("10. an admin features an item on the main shelf, on its category's only, or on neither", async () => {
  const one = Object.assign({}, FX.item_members_view, { featured: "none", labels: [], uploader_key: ANN });
  const { doc, net } = await openPage({ session: fakeSession({ role: "admin" }), routes: listRoutes(listAnswer([one], { categories: CATS }), [
    [at("/api/files/feature", "POST"), (c) => respond(200, { ok: true, id: c.body.id, featured: c.body.featured })],
  ]) });
  const where = () => tileOf(doc, 1).querySelector('[data-action="feature"]');
  assert.equal(where().tagName, "SELECT");
  assert.deepEqual(where().children.map((o) => [o.value, o.textContent]), [["none", "Not featured"], ["main", "Featured"], ["category", "Featured in its category"]]);
  assert.equal(where().value, "none");
  where().value = "category";
  where().dispatch("change");
  await settle();
  const sent = net.calls.filter((c) => c.url === "/api/files/feature").at(-1);
  assert.deepEqual([sent.body.featured, sent.body.sig, sent.body.filename], ["category", "sig:" + FX.purposes.feature_category, "1760_x.png"], "featured in its category, signed so");
  assert.equal(notice(doc), "Featured in its category: it leads that category's shelf now.");
  assert.equal(where().value, "category", "the tile reads where it is now");
  where().value = "none";
  where().dispatch("change");
  await settle();
  assert.equal(net.calls.filter((c) => c.url === "/api/files/feature").at(-1).body.sig, "sig:" + FX.purposes.feature_none);
  assert.equal(notice(doc), "Taken off the featured shelves.");
  // A member gets no Feature at all.
  const member = await openPage({ routes: listRoutes(listAnswer([one])) });
  assert.equal(tileOf(member.doc, 1).querySelector('[data-action="feature"]'), null);
});
