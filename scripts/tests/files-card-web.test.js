// A Media item posted in chat, shown as a card, on the website (2026-10-17): the web half of what the
// desktop app does (src/gui/pages/files/chat_card.rs and its chat_card_tests.rs), in its words. The
// operator: "can we make it when someone clicks on the file in chat it brings up the full popup
// window for that item instead of just having truncated version. I uploaded a set of files but,
// they're all part of the same thing. I'd rather people know exactly what they're downloading
// instead of guessing ... a card in the chat for the media thing(s). Like a simplified version of
// the full popup card. Card could start as a collapsed line ... switchable between expanded and
// collapsed."
//
// Run: node --test scripts/tests/files-card-web.test.js   (in `just rig-tests`)
//
// What runs: the card (web/chat/chat-files-card.js) on the stand-in document, network and signing of
// scripts/tests/files-web-harness.js (a document holding only what was built in it, where reading
// or setting innerHTML throws, so nothing the server sent can be read as HTML; a network that
// records every request; a "signature" that is the words signed); the item window's two halves
// (web/shared/upload-window.js) and the Media page in its item-window form (web/pages/files-app.js
// at /files/<id>?window=item); the protected setup's own rule (web/shared/protected.js
// protectedHidesPictures) and its line from the shipped data/gui/safety_presets.json. The chat page
// itself (app.js drawing the card under a message, the address out of its words) is in
// scripts/tests/private-files-web.test.js, its "media card:" test, which has the chat's harness.
// HOS_WEB_DIR points the test at another copy of web/ (used to see each test red).
//
// What it proves:
//  1. A message's links: this site's /files/<number> (full or bare) is an item, once each, the
//     punctuation after it left off; another website's /files/ page, a query, a word after /files/
//     are not; the card's addresses come out of the words, the space they leave tidied.
//  2. The collapsed line, exactly: the triangle ("Show more"), the item's own file's type badge, its
//     title, "2 files, 2 MB", Open with its tooltip; nothing more. The item is asked for once per
//     page, as the Media page opens an item by its number (signed when there is an identity here).
//  3. Opened out: "by Ann in Fire flow arts", its whole description, every file with its badge,
//     "name, size" and "name, cover picture, size"; closed again it is one line; open or closed is
//     remembered per message and item.
//  4. Open, the title and the picture bring up the item's FULL window: the upload window's frame on
//     /files/7?window=item, which is the Media page's own item page with a Download for each file,
//     whose Close closes the window; one window at a time.
//  5. The words when the server does not have it for this person, and when the request fails.
//  6. The picture follows the Media page's rules: a friend's or my own at once; a non-friend's waits
//     for Show the picture; a labelled item says its labels; the protected setup leaves it out with
//     its line (the uploader's or the message sender's, whoever is not a friend); one too large: no
//     picture at all; a members-only cover fetched with the upload token in a header, never in its
//     address.
// Since the review of 2026-10-17:
//  1b. Only an exact address of this site comes out of the words: nothing inside a word (a crafted
//     "htt/files/5ps://..." never becomes a link), never another website's page; a line with nothing
//     taken out stays exactly as it was. (The DM half, through the real addDmMessage, is
//     warnings-web.test.js 11.)
//  7. Open on an item whose window was kept for its upload brings that window back and tells the page
//     in it to open its item again, with its Edit and the upload; never a second frame on the item.
//  8. The framed windows show the page's own dialogs (Remove's press-and-hold confirmation, the
//     tooltip, the drop overlay), and Escape in the confirmation closes only it.
//  9. An item with no name is called by its kind's word (the kinds file's); the by-line trims its name.
// 10. A file's name cannot hide its real ending: no right-to-left override in the card, on the item's
//     page (its rows, its Download's name) or in its Edit (its rows and rename box).
// The list staying at its bottom as a card goes in and fills in is private-files-web.test.js's
// "media card: the list stays at its bottom" test; the thread panel's cards are its "media card: the
// thread panel" test.
// Since 2026-10-18:
// 11. A members-only item opened from a card with a passphrase-wrapped identity: the item window's page,
//     with no key of its own, is signed for by the chat that framed it (/shared/upload-window.js,
//     files-app.js signerFor), its list and Pick going out with the chat's signature; nobody but that
//     frame, from this site, while it is there, is answered; it is refused once it has left its page,
//     and only a Media purpose is signed; with no identity in the chat either, a plain refusal.
// 12. A card no longer in the document is let go when the next cards go in, never in the turn it is made.
// 13. Size words: the Media page's, the chat card's and the 3D view's, held to the cases the desktop's
//     api::size_words is held to (scripts/tests/fixtures/size-words-cases.json).
// 14. That item window's page, with no sign-in of its own, is lent the chat's upload token for this
//     server (/shared/upload-window.js 'token'): its members-only cover, each Download's ticket, View
//     in 3D and a video's Play here go out with it in the X-Upload-Token header, never in an address or
//     storage;
//     a token the server no longer knows (the chat signed in again) is asked for again.
// 14b. Only that frame, from this site, on its page, while it is there, is given it; a chat holding
//     none refuses (and is asked again after a wait); a page signed in itself never asks.
//
// Red first: each test names the break it was seen failing on.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const H = require("./files-web-harness.js");
const { ROOT, WEB, ME, ANN, MIB, ORIGIN, byId, text, respond, at, fakeFetch, fakeSigner, fakeURLs, fakeDocument, settle, listAnswer, listRoutes, openPage, aFile, resumableRelay } = H;
const MC = require(path.join(WEB, "chat", "chat-files-card.js"));
const UW = require(path.join(WEB, "shared", "upload-window.js"));
const P = require(path.join(WEB, "shared", "protected.js"));
const MI = require(path.join(WEB, "pages", "files-item.js"));
const RISK = require(path.join(WEB, "shared", "file-risk.js"));
const FILE_RISKS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "file_risks.json"), "utf8"));
const HIDDEN_LINE = P.protectedPresetFrom(JSON.parse(fs.readFileSync(path.join(ROOT, "data", "gui", "safety_presets.json"), "utf8"))).picture_hidden_line;
const W = MC.WORDS;
const BEN = "c3".repeat(32);

// The desktop's own fixture (src/gui/pages/files/item_tests.rs rich_item), as the relay sends it:
// Ann's "Dusk" in Fire flow arts, its own dusk.glb and an added textures.zip.
function dusk(over) {
  return H.item(Object.assign({
    id: 7, url: "/uploads/7_dusk.glb", filename: "7_dusk.glb", original_name: "dusk.glb", title: "Dusk",
    description: "First line of it\nThe rest, on its own line", kind: "model", size_bytes: 2 * MIB, total_bytes: 2 * MIB + 4000,
    uploader_key: ANN, uploader_name: "Ann", category: { id: 2, name: "Fire flow arts" }, file_count: 2,
    files: [
      { file: 0, filename: "7_dusk.glb", original_name: "dusk.glb", kind: "model", size_bytes: 2 * MIB, url: "/uploads/7_dusk.glb" },
      { file: 1, filename: "8_textures.zip", original_name: "textures.zip", kind: "file", size_bytes: 4000, url: "/uploads/8_textures.zip" },
    ],
    cover: null,
  }, over || {}));
}
const COVER = { file: 2, filename: "9_cover.png", kind: "picture", size_bytes: 120 * 1024, url: "/uploads/9_cover.png" };

/**
 * The cards of one page, on a stand-in document. `o`: item (what the server answers for item 7, or
 * null for nothing), answer (the whole answer), routes, myKey, token, friends (keys whose pictures
 * show without a click), protectedOn (the setup with its pictures rule on), openItem.
 */
function cardsPage(o) {
  const opts = o || {};
  const doc = fakeDocument();
  const answer = opts.answer || listAnswer(opts.item === null ? [] : [opts.item || dusk()]);
  const net = fakeFetch((opts.routes || []).concat([
    [at("/api/files/list", "POST"), () => respond(200, answer)],
    [at("/api/files", "GET"), () => respond(200, answer)],
  ]));
  const signer = fakeSigner(ME);
  const urls = fakeURLs();
  const opened = [];
  const friends = opts.friends || [];
  const state = opts.protectedOn ? { on: true, pictures: "never" } : null;
  const me = opts.myKey === undefined ? ME : opts.myKey;
  const env = {
    document: doc, fetch: net.fetch, origin: ORIGIN, myKey: () => me, sign: signer.sign,
    token: () => (opts.token === undefined ? "tok-1" : opts.token), URL: urls,
    // As app.js fileBadgeFill draws one (HosFileRisk.badgeHtml): here the name it was asked for.
    badge: (slot, name) => { slot.dataset.badge = name; },
    hidesPicturesFrom: (key) => P.protectedHidesPictures(state, { isMe: !!key && key === me, isFriend: friends.includes(key) }),
    protectedOn: () => !!state,
    hiddenLine: () => HIDDEN_LINE,
    isFriend: (key) => friends.includes(key),
    openItem: opts.openItem || ((id) => opened.push(id)),
  };
  const cards = MC.createFilesCards(env);
  /** The cards for `ids` in a message from `from` at `ts`, put on the page. */
  const post = async (ids, from, ts) => {
    const box = cards.attach(ids, { from: from || ANN, timestamp: ts == null ? 1000 : ts });
    doc.body.appendChild(box);
    await settle();
    return box;
  };
  return { doc, net, signer, urls, opened, cards, env, post };
}
const cls = (sel) => "." + sel;
const lineOf = (card) => card.querySelector(cls("files-card-line"));
const moreOf = (card) => card.querySelector(cls("files-card-more"));
const toggle = async (card) => { card.querySelector(cls("files-card-toggle")).click(); await settle(); };

// ── 1. Links ──────────────────────────────────────────────────────────────

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR, each break in web/chat/chat-files-card.js:
// with onThisSite answering true for every address: "another website's /files/ page stays a link" (actual
// [ 42, 7, 9 ]); with the punctuation after an address kept: the same assertion (actual [ 7, 42 ]); with
// withoutLinks leaving the space it makes: "the desktop's own words for the same message" (actual 'Look: ,
// and  again  https://other.example/files/9 ...'); with sizeWords rounding kilobytes down: '1 file, 3 KB' ==
// '1 file, 4 KB'; with countLine's floor of one file taken out: "at least one file" (actual '0 files, 1 byte').
// Added 2026-10-17 after the review, and seen red the same way: with the scheme matched in lower case
// only (`/^https?:\/\//` without `i`): "Https:// is this site's too"; with byLine not trimming the name:
// 'by   Ann   in Fire flow arts' == 'by Ann in Fire flow arts' (the Media page's by-line trims it too).
test("1. this site's /files/<number> is an item, once each, its punctuation left off; another website's /files/ page is not; the addresses come out of the words", () => {
  const text1 = "Look: " + ORIGIN + "/files/42, and /files/7 again /files/42 https://other.example/files/9 /files/x /files/";
  assert.deepEqual(MC.itemLinks(text1, ORIGIN), [42, 7], "another website's /files/ page stays a link");
  assert.equal(MC.withoutLinks(text1, [42, 7], ORIGIN), "Look: , and again https://other.example/files/9 /files/x /files/", "the desktop's own words for the same message");
  assert.equal(MC.withoutLinks("https://h/files/7", [], ORIGIN), "https://h/files/7", "nothing taken out when there is no card");
  assert.deepEqual(MC.itemLinks("see " + ORIGIN + "/files/7.", ORIGIN), [7], "a full stop after it is not part of it");
  assert.deepEqual(MC.itemLinks("(" + ORIGIN + "/files/7)", ORIGIN), [], "an address inside brackets starts with the bracket: a plain link, as on the desktop");
  assert.deepEqual(MC.itemLinks(ORIGIN + "/files/7?window=item " + ORIGIN + "/files/7/x", ORIGIN), [], "a query or more path after the number: not an item's address");
  assert.deepEqual(MC.itemLinks("http://united-humanity.us/files/7", ORIGIN), [], "the same name on another scheme is another website");
  // A capitalised scheme is still this site (2026-10-17, review; the desktop reads it the same).
  assert.deepEqual(MC.itemLinks("Https://united-humanity.us/files/5 HTTPS://United-Humanity.us/files/6", ORIGIN), [5, 6], "Https:// is this site's too");
  assert.equal(MC.withoutLinks("see Https://united-humanity.us/files/5 now", [5], ORIGIN), "see now", "and comes out of the words");
  // The words of the collapsed line's count (the desktop's count_line).
  assert.equal(MC.countLine(dusk()), "2 files, 2 MB");
  assert.equal(MC.countLine(dusk({ file_count: 1, total_bytes: 0, size_bytes: 4000 })), "1 file, 4 KB");
  assert.equal(MC.countLine(dusk({ file_count: 0, total_bytes: 0, size_bytes: 1 })), "1 file, 1 byte", "at least one file");
  // Word for word the Media page's sizes and by-line, so the card cannot drift from it.
  for (const b of [0, 1, 900, 4000, 2 * MIB, 2 * MIB + 4000, 150 * MIB, 13 * 1024 * MIB]) assert.equal(MC.sizeWords(b), H.media.sizeWords(b));
  for (const it of [dusk(), dusk({ category: null }), dusk({ uploader_name: "" }), dusk({ uploader_name: "  Ann  " }), dusk({ uploader_name: "   " })]) assert.equal(MC.byLine(it), MI.byLine(it));
});

// ONLY AN EXACT ADDRESS IS TAKEN OUT (2026-10-17, review). A stranger's crafted DM "Look /files/5
// htt/files/5ps://evil.example/login" had every "/files/5" in it removed by plain replacement, which
// glued the rest into a live https://evil.example/login link that the stranger-link hold never saw.
// The chat page's half (the same message through the real addDmMessage) is warnings-web.test.js 11.
// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR, each break in web/chat/chat-files-card.js:
// withoutLinks back to plain replacement of every address found, the whole line's space tidied (the
// review's version): "never a link made out of the words"; with the this-site check left out of
// withoutLinks: "another website's page with the same number stays"; with the punctuation after a
// taken address dropped: "its punctuation stays, as a word of its own"; with a line that had nothing
// taken out rebuilt anyway, its words one space apart: "a line with nothing taken out is kept exactly".
test("1b. only an exact address of this site comes out: nothing inside a word, never another website's; lines with nothing taken out stay exactly as they were", () => {
  const out = (text, ids) => MC.withoutLinks(text, ids, ORIGIN);
  // The review's crafted message.
  assert.equal(out("Look /files/5 htt/files/5ps://evil.example/login", [5]), "Look htt/files/5ps://evil.example/login", "never a link made out of the words");
  // A word is taken out only when it IS the address: another number is another item.
  assert.equal(out("see /files/5 and /files/55", [5]), "see and /files/55", "/files/55 is not /files/5");
  assert.equal(out("see /files/5 and /files/55", [5, 55]), "see and", "both, when both have cards");
  assert.equal(out("own /files/5 mirror https://other.site/files/5", [5]), "own mirror https://other.site/files/5", "another website's page with the same number stays");
  assert.equal(out("Look: " + ORIGIN + "/files/42, and /files/7.", [42, 7]), "Look: , and .", "its punctuation stays, as a word of its own");
  // A line with nothing taken out is kept exactly: a code block's indentation, two spaces, a tab.
  const code = "Here:\n    let x = 1;\n        y(x);\n  two  spaces\tand a tab\n/files/5";
  assert.equal(out(code, [5]), "Here:\n    let x = 1;\n        y(x);\n  two  spaces\tand a tab", "a line with nothing taken out is kept exactly");
  // A line with something taken out: its own leading space, then the words left one space apart.
  assert.equal(out("    see  /files/5   now", [5]), "    see now", "its leading space kept, the words left one space apart");
  // A line left with nothing goes; the blank lines between are kept.
  assert.equal(out("Dusk\n" + ORIGIN + "/files/7\n\nmore  words", [7]), "Dusk\n\nmore  words", "the line it empties goes; the rest as it was");
  assert.equal(out(ORIGIN + "/files/7", [7]), "", "an address alone: nothing left");
});

// ── 2. The collapsed line ─────────────────────────────────────────────────

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR, each break in web/chat/chat-files-card.js:
// with the line's badge drawn for its title: "the item's own file's type badge" (actual 'Dusk'); with Open
// drawn without its tooltip: "Open's tooltip" (actual undefined); with the signed request skipped: "the
// members' list, signed" (actual '/api/files?id=7&limit=1'); with attach asking for an item again: "asked for
// once for the page" (actual 2); with a card starting opened out: "the triangle opens it out" (actual
// 'Show less').
test("2. the card starts as one line: the triangle, its own file's badge, its title, how many files and how much, Open; the item asked for once, as the Files page asks", async () => {
  const p = cardsPage();
  const box = await p.post([7]);
  const card = box.children[0];
  const [tri, badge, title, count, open] = lineOf(card).children;
  assert.equal(lineOf(card).children.length, 5, "five parts and nothing more");
  assert.equal(tri.title, "Show more", "the triangle opens it out");
  assert.equal(tri.getAttribute("aria-expanded"), "false");
  assert.equal(badge.dataset.badge, "dusk.glb", "the item's own file's type badge");
  assert.equal(RISK.fileType(badge.dataset.badge, null, FILE_RISKS).label, "3D model", "test setup: a 3D model's badge");
  assert.equal(text(title), "Dusk", "its title");
  assert.equal(title.title, "See everything about it, with a Download for each file.");
  assert.equal(text(count), "2 files, 2 MB", "how many files and how much in all");
  assert.equal(text(open), "Open");
  assert.equal(open.title, "See everything about it, with a Download for each file.", "Open's tooltip");
  assert.equal(moreOf(card), null, "collapsed: no picture, files or description");
  assert.ok(!text(card).includes("textures.zip") && !text(card).includes("First line"), "collapsed: one line");
  // Asked for as the Media page opens an item by its number: signed over media_list, one item.
  assert.equal(p.net.calls.length, 1, "one request");
  const call = p.net.calls[0];
  assert.equal(call.url, "/api/files/list", "the members' list, signed");
  assert.deepEqual({ id: call.body.id, limit: call.body.limit, key: call.body.key, sig: call.body.sig }, { id: 7, limit: 1, key: ME, sig: "sig:media_list" });
  assert.deepEqual(Object.keys(call.body).sort(), Object.keys(Object.assign(H.media.listParams({ id: 7 }, null, 1), { key: 1, timestamp: 1, sig: 1 })).sort(), "the Files page's own fields");
  // Another message with the same item: no second request.
  await p.post([7], BEN, 2000);
  assert.equal(p.net.calls.length, 1, "asked for once for the page");
  // Without an identity here: the public list.
  const anon = cardsPage({ myKey: "" });
  await anon.post([7]);
  assert.deepEqual(anon.net.calls.map((c) => c.url), ["/api/files?id=7&limit=1"], "unsigned, the public list, as the Files page asks");
});

// ── 3. Opened out ─────────────────────────────────────────────────────────

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR, each break in web/chat/chat-files-card.js:
// with the description cut to its first line: "its whole description" (actual 'First line of it'); with the
// cover left out of the file list: "every file, the cover last" (actual its two files only); with keyOf
// naming the item alone: "another message's card: its own state" (it came open); with a file row drawn
// without its badge: a TypeError reading the badge's dataset (there was none).
test("3. opened out: who put it there, its description and every file with its badge; closed again one line; remembered per message and item", async () => {
  const p = cardsPage({ item: dusk({ cover: COVER }), friends: [ANN] });
  const box = await p.post([7], ANN, 1000);
  const card = box.children[0];
  await toggle(card);
  const tri = lineOf(card).children[0];
  assert.equal(tri.title, "Show less");
  assert.equal(tri.getAttribute("aria-expanded"), "true");
  const more = moreOf(card);
  assert.ok(more, "opened out");
  assert.equal(text(more.querySelector(cls("files-card-by"))), "by Ann in Fire flow arts", "the Files page's by-line");
  assert.equal(text(more.querySelector(cls("files-card-desc"))), "First line of it\nThe rest, on its own line", "its whole description");
  const rows = more.querySelectorAll(cls("files-card-file"));
  assert.deepEqual(rows.map((r) => text(r.querySelector(cls("files-card-file-name")))), ["dusk.glb, 2 MB", "textures.zip, 4 KB", "9_cover.png, cover picture, 120 KB"], "every file, the cover last");
  assert.deepEqual(rows.map((r) => r.querySelector(cls("files-card-badge")).dataset.badge), ["dusk.glb", "textures.zip", "9_cover.png"], "each with its own type's badge");
  assert.equal(RISK.fileType("textures.zip", null, FILE_RISKS).label, "Archive", "test setup: an archive's badge");
  // The same item in another message starts as one line; this one stays open when drawn again.
  const other = (await p.post([7], BEN, 2000)).children[0];
  assert.equal(moreOf(other), null, "another message's card: its own state");
  const again = (await p.post([7], ANN, 1000)).children[0];
  assert.ok(moreOf(again), "the same message drawn again: still open");
  await toggle(card);
  assert.equal(moreOf(card), null, "closed again: one line");
});

// ── 4. Open: the item's full window ───────────────────────────────────────

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR:
// in shared/upload-window.js, itemPage without ?window=item: "the Media page's own item page, in its
// item-window form" (actual '/files/7'); in pages/files-app.js, env.itemWindow ignored: "nothing of the
// list" (the whole Media page was drawn in the frame); in pages/files-item.js, close() without its
// closed() call: "its Close closes the window" (actual 0); in shared/upload-window.js, showPage keeping
// a frame on another page: '/files/7?window=item' == '/files/9?window=item'; in chat/chat-files-card.js, the
// title's click doing nothing: "the title opens it too" (actual null).
test("4. Open, the title and the picture bring up the item's full window: the Files page's own item page in a frame, with a Download for each file, whose Close closes it", async () => {
  // The chat's half: the window over the chat (a stand-in window that hears messages).
  const heard = {};
  const win = { location: { origin: ORIGIN }, addEventListener: (t, f) => { (heard[t] = heard[t] || []).push(f); } };
  let opener = null; // made once, on the page's own document, as the chat makes it
  let host = null;
  const p = cardsPage({
    item: dusk({ cover: COVER }), friends: [ANN],
    openItem: (id, item) => {
      if (!opener) opener = MC.itemWindowOpener(UW, { window: win, document: p.doc });
      host = opener(id, item);
    },
  });
  const card = (await p.post([7])).children[0];
  const [, , title, , open] = lineOf(card).children;
  open.click();
  const dialog = () => p.doc.body.querySelector(cls("hos-upload-window"));
  assert.ok(dialog() && dialog().hidden === false, "a window over the chat");
  const frame = dialog().querySelector("iframe");
  assert.equal(frame.src, "/files/7?window=item", "the Files page's own item page, in its item-window form");
  assert.equal(frame.src, UW.itemPage(7));
  assert.equal(dialog().getAttribute("aria-label"), "Dusk", "named for the item");
  dialog().querySelector(cls("hos-upload-window-close")).click();
  assert.equal(dialog(), null, "its Close takes it away");
  title.click();
  assert.ok(dialog() && dialog().querySelector("iframe").src === "/files/7?window=item", "the title opens it too");
  host.close();
  await toggle(card);
  moreOf(card).querySelector(cls("files-card-picture")).click();
  assert.ok(dialog() && dialog().querySelector("iframe").src === "/files/7?window=item", "and the picture");
  // One window at a time: another item's Open sends it there.
  host.showPage(UW.itemPage(9), "Other");
  assert.equal(p.doc.body.querySelectorAll(cls("hos-upload-window")).length, 1, "one window");
  assert.equal(dialog().querySelector("iframe").src, "/files/9?window=item");

  // The window's half: the Media page at /files/7?window=item with the item window's connector.
  const sent = [];
  const conn = { send: (event, data) => sent.push(Object.assign({ event }, data || {})) };
  const m = await openPage({ path: "/files/7", itemWindow: conn, routes: listRoutes(listAnswer([dusk({ cover: COVER }), H.item({ id: 3 })])) });
  assert.ok(byId(m.doc, "files-viewer-open"), "the item's own page");
  assert.equal(text(m.doc.body.querySelector(cls("files-viewer-title"))), "Dusk");
  assert.equal(byId(m.doc, "files-grid"), null, "nothing of the list");
  assert.equal(byId(m.doc, "files-title"), null, "nor the page's heading");
  assert.equal(byId(m.doc, "files-item-files").querySelectorAll("[data-download]").length, 3, "a Download for each file, the cover's too");
  assert.ok(m.net.calls.some((c) => c.url === "/api/files/list" && c.body && c.body.id === 7), "the item asked for by its number");
  assert.deepEqual(sent.map((s) => s.event + (s.event === "busy" ? ":" + s.busy : "")), ["busy:false"], "only that nothing is going in it");
  byId(m.doc, "files-viewer-close").click();
  assert.equal(sent.filter((s) => s.event === "close").length, 1, "its Close closes the window");
});

// ── 5. Not there, or not loaded ───────────────────────────────────────────

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR, each break in web/chat/chat-files-card.js:
// with a missing item drawn as still loading: 'Loading the Media item...' == 'That item is not on the Media
// page, or you may not see it.'; with a refusal said by its status alone: 'This Media item could not be
// loaded: 503' == 'This Media item could not be loaded: The server is busy. Try again in a moment.'.
test("5. the words when the server does not have it for this person, and when the request fails", async () => {
  const gone = cardsPage({ item: null });
  const g = await gone.post([7]);
  assert.equal(text(g), "That item is not on the Files page, or you may not see it.");
  assert.equal(text(g), W.notThere);
  const busy = cardsPage({ routes: [[at("/api/files/list", "POST"), () => respond(503, "The server is busy. Try again in a moment.")]] });
  assert.equal(text(await busy.post([7])), "This item from the Files page could not be loaded: The server is busy. Try again in a moment.");
  const proxy = cardsPage({ routes: [[at("/api/files/list", "POST"), () => respond(502, "<html>Bad gateway</html>")]] });
  assert.equal(text(await proxy.post([7])), "This item from the Files page could not be loaded: The server answered 502.");
  // While it is on its way.
  const slow = cardsPage({ routes: [[at("/api/files/list", "POST"), () => new Promise(() => {})]] });
  assert.equal(text(await slow.post([7])), "Loading the item from the Files page...");
});

// ── 6. The picture ────────────────────────────────────────────────────────

/** The picture part of an open card for item `it` in a message from `from`: {img, note, show}. */
async function pictureIn(o, it, from) {
  const p = cardsPage(Object.assign({ item: it }, o));
  const card = (await p.post([7], from || ANN, 1000)).children[0];
  await toggle(card);
  const more = moreOf(card);
  const img = more.querySelector("img");
  const ask = more.querySelector(cls("files-card-ask"));
  const note = more.querySelector(cls("files-card-note"));
  return { p, card, more, img, ask, note, show: ask ? ask.querySelector(cls("files-card-show")) : null };
}

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR, each break in web/chat/chat-files-card.js:
// with the click-first rule for a non-friend taken out: "not fetched before the click"; with only the
// uploader asked about under the protected setup (the desktop's rule): "a non-friend's message never shows
// a card's picture"; with the 16 MB limit taken out: "too large to show here: no picture"; with the upload
// token put in the address: "fetched from the members' route" (it asked /api/files/file/7?file=2&token=...);
// with a labelled item not asked about first: a TypeError (no Show the picture row); with the protected
// setup's label rule taken out: 'Labelled: Spoiler.' == 'Hidden: the protected setup leaves out items with a
// content label.'; with ownPath's same-site check taken out: "a cover named on another website: not loaded,
// not even its path here".
test("6. the picture follows the Files page's rules: a friend's at once, a non-friend's on a click, labels, the protected setup, too large, members only", async () => {
  // A friend's cover: at once, from this server's own address; a click opens the item.
  const f = await pictureIn({ friends: [ANN] }, dusk({ cover: COVER }));
  assert.ok(f.img, "a friend's picture shows");
  assert.equal(f.img.src, "/uploads/9_cover.png", "its cover, from this server");
  f.more.querySelector(cls("files-card-picture")).click();
  assert.deepEqual(f.p.opened, [7], "a click on it opens the item");
  // My own item: at once.
  assert.ok((await pictureIn({}, dusk({ cover: COVER, uploader_key: ME }), ME)).img, "my own picture shows");
  // The item's own picture when it has no cover.
  const pic = await pictureIn({ friends: [ANN] }, dusk({ kind: "picture", url: "/uploads/7_dusk.png", cover: null }));
  assert.equal(pic.img.src, "/uploads/7_dusk.png", "else its own picture");
  // A model with no cover: no picture part.
  const none = await pictureIn({ friends: [ANN] }, dusk());
  assert.equal(none.img, null);
  assert.equal(none.more.children[0].className, "files-card-by", "nothing before who put it there");
  // Someone who is not a friend: a click first, in the desktop's words.
  const nf = await pictureIn({}, dusk({ cover: COVER }));
  assert.equal(nf.img, null, "not fetched before the click");
  assert.equal(text(nf.ask.querySelector(cls("files-card-note"))), "Pictures from Ann show only when you choose.");
  assert.equal(text(nf.show), "Show the picture");
  nf.show.click();
  await settle();
  assert.ok(moreOf(nf.card).querySelector("img"), "shown once asked");
  // A labelled item: its labels, by the server's names.
  const lab = await pictureIn({ friends: [ANN] }, dusk({ cover: COVER, labels: ["spoiler", "flashing"] }));
  assert.equal(text(lab.ask.querySelector(cls("files-card-note"))), "Labelled: Spoiler, Flashing lights.");
  assert.equal(lab.img, null);
  // The protected setup: the uploader not a friend, or the message's sender not a friend.
  const prot = await pictureIn({ protectedOn: true }, dusk({ cover: COVER }));
  assert.equal(prot.img, null);
  assert.equal(text(prot.note), HIDDEN_LINE, "the preset's own line");
  const sender = await pictureIn({ protectedOn: true, friends: [ANN] }, dusk({ cover: COVER }), BEN);
  assert.equal(sender.img, null, "a non-friend's message never shows a card's picture");
  assert.equal(text(sender.note), HIDDEN_LINE);
  assert.ok((await pictureIn({ protectedOn: true, friends: [ANN] }, dusk({ cover: COVER }), ANN)).img, "a friend's message with a friend's item shows it");
  const labProt = await pictureIn({ protectedOn: true, friends: [ANN] }, dusk({ cover: COVER, labels: ["spoiler"] }));
  assert.equal(text(labProt.note), "Hidden: the protected setup leaves out items with a content label.");
  // Too large to show here: nothing at all.
  const big = await pictureIn({ friends: [ANN] }, dusk({ cover: Object.assign({}, COVER, { size_bytes: 17 * MIB }) }));
  assert.equal(big.img, null, "too large to show here: no picture");
  assert.equal(big.note, null, "no line either");
  // A members-only cover: fetched with the session's upload token in a header, shown through an
  // object URL; never with the token in an address.
  const memRoutes = [[at("/api/files/file/7"), () => respond(200, null, new Uint8Array([0x89, 0x50, 0x4e, 0x47]))]];
  const mem = await pictureIn({ friends: [ANN], routes: memRoutes }, dusk({ visibility: "members", url: "/api/files/file/7", cover: Object.assign({}, COVER, { url: "/api/files/file/7?file=2" }) }));
  await settle();
  const got = mem.p.net.calls.find((c) => c.url === "/api/files/file/7?file=2");
  assert.ok(got, "fetched from the members' route");
  assert.equal(got.headers["X-Upload-Token"], "tok-1", "with the token in a header");
  const shown = moreOf(mem.card).querySelector("img");
  assert.ok(shown && shown.src.startsWith("blob:test/"), "shown through an object URL");
  assert.ok(!mem.p.net.calls.some((c) => c.url.includes("tok-1")), "the token is in no address");
  const noTok = await pictureIn({ friends: [ANN], token: "" }, dusk({ visibility: "members", cover: Object.assign({}, COVER, { url: "/api/files/file/7?file=2" }) }));
  assert.equal(text(noTok.note), "This members-only file shows once this page is signed in to the server.");
  // Another website's address is never loaded.
  const away = await pictureIn({ friends: [ANN] }, dusk({ cover: Object.assign({}, COVER, { url: "https://other.example/uploads/9_cover.png" }) }));
  assert.equal(away.img, null, "a cover named on another website: not loaded, not even its path here");
});

// ── 7. An item window kept for its upload, brought back ───────────────────

/** A window that keeps its listeners and what is posted to it (post-in-chat-web.test.js's). */
function fakeWindow(origin, parent) {
  const listeners = {};
  const w = {
    location: { origin },
    posted: [],
    addEventListener(t, f) { (listeners[t] = listeners[t] || []).push(f); },
    removeEventListener(t, f) { listeners[t] = (listeners[t] || []).filter((x) => x !== f); },
    postMessage(data, to) { w.posted.push({ data, to }); },
    fire(type, ev) { for (const f of (listeners[type] || []).slice()) f(ev); },
    setTimeout() { return 0; },
    clearTimeout() {},
  };
  w.parent = parent || w;
  return w;
}
const said = (win, from, event, data) => win.fire("message", { source: from, origin: ORIGIN, data: Object.assign({ kind: UW.KIND, event }, data || {}) });

// REOPENING AN ITEM WHOSE FRAME IS HIDDEN WHILE BUSY (2026-10-17, review): open an item from a card,
// Edit, add a file (the frame says busy), close the item with its own Close (the frame says close; the
// host hides the busy frame), then Open the same item while the upload runs: the window came back
// empty, because the frame was only unhidden and the page in it had closed its item.
// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR, each break:
// in chat/chat-files-card.js, itemWindowOpener back to one remembered window (the review's version):
// "the same window" (a second window, its own frame on /files/7, was made: the kept one stayed hidden);
// in shared/upload-window.js, showPage not telling the frame 'open': "the page in it told to open its
// item again" (actual []);
// in shared/upload-window.js, connectFrame not hearing 'open': "its host's" (actual 0);
// in pages/files-app.js, the onOpen registration taken out: "told to open it again: the item is back";
// in pages/files-app.js, reopen not opening the Edit: "with its Edit, and the upload on its way in it"
// (actual 0).
test("7. Open on an item whose window was kept for its upload brings that window back, the item and its upload in it; never a second frame on it", async () => {
  // The chat's half: the opener and its windows, over a stand-in window.
  const doc = fakeDocument();
  const hostWin = fakeWindow(ORIGIN);
  const open = MC.itemWindowOpener(UW, { window: hostWin, document: doc });
  const windows = () => doc.body.querySelectorAll(cls("hos-upload-window"));
  const a = open(7, dusk(), "Dusk");
  const frameA = a.frame();
  const insideA = fakeWindow(ORIGIN, hostWin);
  frameA.contentWindow = insideA;
  said(hostWin, insideA, "busy", { busy: true }); // an Edit's file is uploading
  said(hostWin, insideA, "close"); // the item's own Close
  assert.ok(a.element() && a.element().hidden === true, "hidden, kept while its upload goes");
  // Another item meanwhile: a window of its own.
  const b = open(9, dusk({ id: 9, title: "Other" }), "Other");
  assert.notEqual(b, a, "another item, another window");
  assert.equal(windows().filter((w) => !w.hidden).length, 1, "one shown");
  // Open on the first item again: its own window back, the other closed.
  assert.equal(insideA.posted.length, 0, "test setup: nothing told to the frame yet");
  const again = open(7, dusk(), "Dusk");
  assert.equal(again, a, "the same window");
  assert.equal(a.frame(), frameA, "the same frame, its upload still going in it");
  assert.equal(windows().filter((w) => w.querySelector("iframe").src === UW.itemPage(7)).length, 1, "one frame on the item");
  assert.equal(a.element().hidden, false, "shown");
  assert.equal(windows().filter((w) => !w.hidden).length, 1, "one shown: the other closed");
  assert.deepEqual(insideA.posted, [{ data: { kind: UW.KIND, event: "open" }, to: ORIGIN }], "the page in it told to open its item again, on this site only");

  // The window's half: 'open' heard from its host only, from this site.
  const frameWin = fakeWindow(ORIGIN, hostWin);
  let opens = 0;
  UW.connectFrame({ window: frameWin }).onOpen(() => { opens++; });
  said(frameWin, fakeWindow(ORIGIN), "open");
  frameWin.fire("message", { source: hostWin, origin: "https://elsewhere.example", data: { kind: UW.KIND, event: "open" } });
  assert.equal(opens, 0, "not another window's, nor another site's");
  said(frameWin, hostWin, "open");
  assert.equal(opens, 1, "its host's");

  // The page's half: the Media page at /files/7?window=item. Its Edit adds a file (held on its way),
  // its Close closes the item and the window; told 'open', the item is back, its Edit and the upload in it.
  const relay = resumableRelay({ start: () => new Promise(() => {}) });
  const pageWin = fakeWindow(ORIGIN, hostWin);
  const mine = dusk({ uploader_key: ME, uploader_name: "Me" });
  const m = await openPage({ path: "/files/7", itemWindow: UW.connectFrame({ window: pageWin }), routes: listRoutes(listAnswer([mine]), relay.routes) });
  byId(m.doc, "files-item-edit-open").click();
  const input = byId(m.doc, "files-edit-files").querySelector('[data-attach-input="file"]');
  input.files = [aFile("clip.webm", 20000, { type: "video/webm" })];
  input.dispatch("change");
  await settle();
  const told = () => hostWin.posted.map((p) => p.data.event + (p.data.event === "busy" ? ":" + p.data.busy : ""));
  assert.ok(told().includes("busy:true"), "test setup: the upload keeps the window: " + told());
  assert.equal(byId(m.doc, "files-edit-files").querySelectorAll("[data-attach-row]").length, 1, "test setup: the upload shows in the Edit");
  byId(m.doc, "files-viewer-close").click();
  assert.equal(byId(m.doc, "files-viewer-open"), null, "test setup: its Close closes the item");
  assert.ok(told().includes("close"), "test setup: and the window");
  said(pageWin, hostWin, "open");
  await settle();
  assert.ok(byId(m.doc, "files-viewer-open"), "told to open it again: the item is back");
  assert.equal(text(m.doc.body.querySelector(cls("files-viewer-title"))), "Dusk");
  const edit = byId(m.doc, "files-edit-files");
  assert.equal(edit ? edit.querySelectorAll("[data-attach-row]").length : 0, 1, "with its Edit, and the upload on its way in it");
  // Told again while it shows: nothing changes (no second Edit, nothing asked again).
  const asked = m.net.calls.length;
  said(pageWin, hostWin, "open");
  await settle();
  assert.equal(byId(m.doc, "files-edit-files").querySelectorAll("[data-attach-row]").length, 1, "told again: the same page");
  assert.equal(m.net.calls.length, asked, "nothing asked again");
});

// ── 8. The framed windows show the page's own dialogs ─────────────────────

/** /shared/hold-confirm.js run on `doc` (it draws on document.body and listens on document). */
function holdConfirmOn(doc) {
  if (!doc.head) doc.head = doc.createElement("head");
  const ctx = { document: doc, console };
  ctx.window = ctx;
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(WEB, "shared", "hold-confirm.js"), "utf8"), ctx, { filename: "hold-confirm.js" });
  return ctx.holdConfirm;
}

// REMOVE IN THE FRAMED ITEM WINDOW (2026-10-17, review): files.html's rule that leaves the site's menu
// and footer out of a framed window hid every other element on the body too, among them the
// press-and-hold confirmation /shared/hold-confirm.js draws there, so Remove item (and removing a file
// in Edit) opened an invisible confirmation whose promise never settled; and the tooltip shell.js
// draws in place of every title (it takes the titles away) was hidden as well. And Escape in that
// confirmation also closed the item under it, and the window with it.
// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR, each break: in pages/files.html, the
// rule without :not(.hold-confirm-overlay) (the review's version): "the confirmation Remove asks with
// shows in the frame"; without :not(#hos-rich-tooltip): "and the tooltip"; in pages/files-app.js, the
// Escape handler without its holdConfirmOpen check: "the item stays open under it".
test("8. the framed windows show the page's own dialogs: Remove's press-and-hold confirmation and the tooltip; Escape in the confirmation closes only it", async () => {
  const html = fs.readFileSync(path.join(WEB, "pages", "files.html"), "utf8");
  const rule = /\[data-files-window\] body > ((?::not\([^)]*\))+) \{ display: none !important; \}/.exec(html);
  assert.ok(rule, "the framed page leaves out what is not its own");
  const kept = rule[1].match(/:not\(([^)]*)\)/g).map((s) => s.slice(5, -1));
  // Does the rule (body > :not(a):not(b)...) match `el`, a child of the body: is it hidden in a frame?
  const hidden = (el) => !kept.some((sel) => (sel[0] === "#" ? el.id === sel.slice(1) : sel[0] === "." ? String(el.className || "").split(/\s+/).includes(sel.slice(1)) : false));
  assert.ok(hidden({ id: "", className: "hos-nav" }) && hidden({ id: "hos-footer", className: "" }), "test setup: the site's menu and footer stay out");
  assert.ok(!hidden({ id: "files-root", className: "" }), "test setup: the page shows");

  // Remove's confirmation, as /shared/hold-confirm.js really draws it, over the item window. Escape
  // in it is its own Cancel and closes nothing under it.
  const sent = [];
  const conn = { send: (event, data) => sent.push(Object.assign({ event }, data || {})) };
  const m = await openPage({ path: "/files/7", itemWindow: conn, keys: true, routes: listRoutes(listAnswer([dusk()])) });
  assert.ok(byId(m.doc, "files-viewer-open"), "test setup: the item is open");
  const answer = holdConfirmOn(m.doc)("Remove this item?", { seconds: 2, confirmLabel: "Hold to remove" });
  const overlay = m.doc.body.children.find((c) => c.parentNode === m.doc.body && c.className === "hold-confirm-overlay");
  assert.ok(overlay, "test setup: drawn on the body");
  assert.ok(!hidden(overlay), "the confirmation Remove asks with shows in the frame");
  // The tooltip /shared/shell.js draws in place of every title.
  const tip = /tip\.id = '([\w-]+)'/.exec(fs.readFileSync(path.join(WEB, "shared", "shell.js"), "utf8"));
  assert.ok(tip, "test setup: shell.js names its tooltip");
  assert.ok(!hidden({ id: tip[1], className: "" }), "and the tooltip");
  // The drop overlay, as /shared/drop-overlay.js names it.
  const drop = /className = '(hos-drop-overlay)'/.exec(fs.readFileSync(path.join(WEB, "shared", "drop-overlay.js"), "utf8"));
  assert.ok(drop && !hidden({ id: "", className: drop[1] }), "and the drop overlay");
  m.doc.key("Escape");
  assert.equal(await answer, false, "Escape is its Cancel");
  assert.equal(m.doc.body.querySelector(cls("hold-confirm-overlay")), null, "and takes it away");
  assert.ok(byId(m.doc, "files-viewer-open"), "the item stays open under it");
  assert.equal(sent.filter((s) => s.event === "close").length, 0, "and so does its window");
  m.doc.key("Escape");
  assert.equal(byId(m.doc, "files-viewer-open"), null, "with no confirmation open, Escape closes the item");
  assert.equal(sent.filter((s) => s.event === "close").length, 1, "and its window");
});

// ── 9. A title from the kind's word; the by-line's name trimmed ───────────

// SMALL PARITY FIXES (2026-10-17, review): a card for an item with no title and no file name fell
// back to the raw kind ("model"), where the desktop's display_title says the kinds file's word for
// one of it ("3D model"); and its by-line used the uploader's name untrimmed.
// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR, each break in web/chat/chat-files-card.js:
// with displayTitle falling back to the raw kind: "the kinds file's word for one of it, as the desktop
// says it"; with fetchItem never asking for the kinds file: the same; with byLine not trimming: "the
// uploader's name trimmed" (and test 1's comparison with the Media page's by-line).
test("9. an item with no name is called by its kind's word, as the desktop calls it; the by-line's name is trimmed", async () => {
  const KINDS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "files", "kinds.json"), "utf8"));
  const nameless = (id) => dusk({ id, title: "", original_name: "", filename: "", url: "/api/files/file/" + id, visibility: "members", files: [], file_count: 1, uploader_name: "   Ann   " });
  const titles = [];
  const p = cardsPage({
    answer: listAnswer([nameless(7)]), friends: [ANN],
    routes: [[at("/data/files/kinds.json"), () => respond(200, KINDS)]],
    openItem: (id, item, title) => titles.push([id, title]),
  });
  const card = (await p.post([7])).children[0];
  const title = lineOf(card).children[2];
  assert.equal(text(title), "3D model", "the kinds file's word for one of it, as the desktop says it");
  assert.equal(MC.displayTitle(nameless(7), MC.parseKinds(KINDS)), "3D model");
  title.click();
  assert.deepEqual(titles, [[7, "3D model"]], "its window is named so too");
  await toggle(card);
  assert.equal(text(moreOf(card).querySelector(cls("files-card-by"))), "by Ann in Fire flow arts", "the uploader's name trimmed");
  assert.equal(p.net.calls.filter((c) => c.url === MC.KINDS_URL).length, 1, "the kinds file asked for once");
  // An item with a title never asks for it (test 2: one request); one the kinds file does not list
  // reads as the server named it, as the desktop's one_name.
  assert.equal(MC.displayTitle(dusk({ title: "", original_name: "", url: "/api/files/file/7", kind: "sculpture" }), MC.parseKinds(KINDS)), "sculpture");
});

// ── 10. A file's name cannot hide its real ending ─────────────────────────

// A NAME CANNOT HIDE ITS ENDING (2026-10-17, the coordinator): a right-to-left override (U+202E) makes
// a browser show "photo<RLO>gnp.exe" as "photoexe.png". Every file name these pages draw goes through
// /shared/file-risk.js plainName (the desktop's file_risk::plain_name): the card's file rows and its
// badge, the item page's rows, its Download's name and its 3D view's, and the Edit's rows and rename box.
// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR, each break: in chat/chat-files-card.js,
// shownName returning fileName as it is: "the card's file row: no override in it"; in pages/files-item.js,
// shownName returning fileName as it is: "the item page's file row" (it read "photo<RLO>gnp.exe, 4 KB");
// in pages/files-edit.js, the rows named by view.fileName: "the Edit's file row"; the rename box started
// from view.fileName: "the rename box starts with the plain name, its ending after it".
test("10. a file's name cannot hide its real ending: a right-to-left override is not drawn in the card, on the item's page or in its Edit", async () => {
  const RLO = "‮";
  const sneaky = "photo" + RLO + "gnp.exe"; // a browser shows it as "photoexe.png"
  const shown = "photognp.exe";
  const it = dusk({
    uploader_key: ME, uploader_name: "Me",
    files: [
      { file: 0, filename: "7_dusk.glb", original_name: "dusk.glb", kind: "model", size_bytes: 2 * MIB, url: "/uploads/7_dusk.glb" },
      { file: 1, filename: "8_photo.exe", original_name: sneaky, kind: "file", size_bytes: 4000, url: "/uploads/8_photo.exe" },
    ],
  });
  assert.equal(RISK.plainName(sneaky), shown, "test setup: file-risk.js's plain name");
  // The card: its file row and the row's badge.
  const p = cardsPage({ item: it, friends: [ANN] });
  const card = (await p.post([7])).children[0];
  await toggle(card);
  const row = moreOf(card).querySelectorAll(cls("files-card-file"))[1];
  assert.equal(text(row.querySelector(cls("files-card-file-name"))), shown + ", 4 KB", "the card's file row: no override in it");
  assert.equal(row.querySelector(cls("files-card-badge")).dataset.badge, shown, "its badge judges the name shown");
  // The collapsed line's badge, for an item whose own file's name hides its ending.
  const q = cardsPage({ item: dusk({ original_name: sneaky }) });
  assert.equal(lineOf((await q.post([7])).children[0]).children[1].dataset.badge, shown, "the line's badge too");
  // The item's own page: its row, its badge, and the name Download saves it under.
  const m = await openPage({ path: "/files/7", routes: listRoutes(listAnswer([it])) });
  const names = byId(m.doc, "files-item-files").querySelectorAll(".files-item-file-name").map(text);
  assert.ok(names.includes(shown + ", 4 KB"), "the item page's file row: " + names.join(" | "));
  assert.ok(byId(m.doc, "files-item-files").querySelectorAll("[data-file-badge]").some((b) => b.dataset.fileBadge === shown), "and its badge");
  byId(m.doc, "files-item-files").querySelector('[data-download="1"]').click();
  await settle();
  assert.deepEqual(m.saved.map((s) => s.name), [shown], "Download saves it by that name");
  // Its Edit: the row, and the rename box's starting words.
  byId(m.doc, "files-item-edit-open").click();
  const editRows = byId(m.doc, "files-edit-files").querySelectorAll("[data-edit-file]");
  assert.ok(editRows.some((r) => text(r.querySelector(".files-item-file-name")) === shown + ", 4 KB"), "the Edit's file row: " + editRows.map(text).join(" | "));
  byId(m.doc, "files-edit-files").querySelector('[data-rename-file="1"]').click();
  const box = byId(m.doc, "files-edit-files").querySelector('[data-rename-input="1"]');
  assert.equal(box.value, "photognp", "the rename box starts with the plain name, its ending after it");
  assert.ok(!text(m.doc.body).includes(RLO), "nowhere on the page");
});

// ── 11. A members-only item opened from a card, with a passphrase-wrapped identity ──

/**
 * The chat (`host`) and the item window's page in the frame over it (`frame`, on /files/7?window=item),
 * hearing each other: a message posted to one arrives a turn later, as a browser's does, from the
 * other, with this site's origin, and only when it was posted to that origin.
 */
function linkedWindows() {
  const host = fakeWindow(ORIGIN);
  const frame = fakeWindow(ORIGIN, host);
  Object.assign(frame.location, { pathname: "/files/7", search: "?window=item" });
  const wire = (to, from) => {
    to.postMessage = (data, target) => {
      to.posted.push({ data, to: target });
      if (target === to.location.origin) setImmediate(() => to.fire("message", { source: from, origin: ORIGIN, data }));
    };
  };
  wire(host, frame);
  wire(frame, host);
  return { host, frame };
}

// THE ITEM WINDOW HAD NO KEY TO SIGN WITH (2026-10-18). A card's own request is signed with the chat's
// unlocked identity, so a members-only item shows on it; its Open framed /files/7?window=item, whose page
// takes its key from the copy this browser keeps (humanity_key_backup, /shared/pq-relay-auth.js). A
// passphrase-wrapped identity keeps no such copy, so the framed page listed unsigned and said the item was
// not there. Now the chat signs for the page in the frame it made (/shared/upload-window.js): only that
// frame, from this site, on the page it was given, while it is there; only the Media page's purposes.
// Seen red 2026-10-18 in a copy of web/ through HOS_WEB_DIR, each break: in shared/upload-window.js, the
// host's 'sign' answer taken out of onMessage: "the item's own page, asked for signed by the chat" (the
// page fell back to listing unsigned after its ask went unanswered: actual null); in pages/files-app.js,
// signerFor ignoring the item window: "signed as the chat's identity" (actual 'none'); in
// chat/chat-files-card.js, the opener not handing `sign` to its windows: the same (the identity ask was
// refused); in shared/upload-window.js, both of the host's checks that the asker is its frame taken out
// (onMessage's source check and answerAsk's frameWindow() !== from; either alone still holds): "a window
// that is not the frame gets nothing" (actual 1: it was answered); onMessage without its origin check:
// "another site gets nothing"; answerAsk without its onOwnPage checks: "a frame that left its page is
// refused" (it was signed, 'sig:media_list'); without signable(): "a chat message's words are never
// signed for it" (it was signed, 'sig:hello everyone').
test("11. a members-only item opened from a card, the identity passphrase-wrapped: the chat signs for the page in its window; nobody else is answered", async () => {
  const members = dusk({ visibility: "members", url: "/api/files/file/7" });
  // The chat's half: its opener, its own signer (the identity unlocked in the chat).
  const wins = linkedWindows();
  const doc = fakeDocument();
  const chat = fakeSigner(ME);
  let chatKey = ME;
  const open = MC.itemWindowOpener(UW, { window: wins.host, document: doc, sign: chat.sign, myKey: () => chatKey });
  const host = open(7, members, "Dusk");
  host.frame().contentWindow = wins.frame;

  // The page's half: no key of its own, so it asks the chat who it signs as (an ask unanswered for a
  // fifth of a second here goes on without: the chat's answer takes a few turns).
  const conn = UW.connectFrame({ window: wins.frame, setTimeout: (fn) => setTimeout(fn, 200), clearTimeout: (t) => clearTimeout(t) });
  const who = await H.media.signerFor({ identity: null, ownSign: async () => null, itemWindow: conn });
  assert.equal(who.via, "chat", "signed as the chat's identity");
  assert.equal(who.myKey, ME, "its key");
  // The relay answers a members-only item to a signed list only, as media.rs does.
  const onlySigned = [
    [at("/api/files/list", "POST"), (c) => respond(200, listAnswer(c.body && c.body.sig ? [members] : []))],
    [at("/api/files/pick", "POST"), () => respond(200, { ok: true, picks: 1 })],
  ];
  const m = await openPage({ path: "/files/7", itemWindow: conn, myKey: who.myKey, sign: who.sign, session: null, routes: listRoutes(listAnswer([]), onlySigned) });
  await settle();
  assert.ok(byId(m.doc, "files-viewer-open"), "the item's own page, asked for signed by the chat");
  assert.equal(text(m.doc.body.querySelector(cls("files-viewer-title"))), "Dusk");
  const list = m.net.calls.find((c) => c.url === "/api/files/list" && c.body && c.body.id === 7);
  assert.deepEqual([list.body.key, list.body.sig], [ME, "sig:media_list"], "the list request carries the chat's signature");
  assert.ok(!m.net.calls.some((c) => c.url.startsWith("/api/files?")), "never listed unsigned");
  // Pick, through the same signing.
  byId(m.doc, "files-viewer-open").querySelector('[data-action="pick"]').click();
  await settle();
  const picked = m.net.calls.find((c) => c.url === "/api/files/pick");
  assert.ok(picked && picked.body.sig === "sig:media_pick:7" && picked.body.key === ME, "Pick signed by the chat too");
  assert.ok(chat.signed.includes("media_pick:7"), "with the chat's own signer");

  // Only the frame the chat made, from this site, on its page, while it is there.
  const ask = async (from, origin, data) => {
    wins.host.fire("message", { source: from, origin, data: Object.assign({ kind: UW.KIND }, data) });
    await settle();
  };
  const answersTo = (w) => w.posted.filter((p) => p.data && (p.data.event === "signed" || p.data.event === "identity"));
  const stranger = fakeWindow(ORIGIN);
  Object.assign(stranger.location, { pathname: "/files/7", search: "?window=item" });
  const before = chat.signed.length;
  await ask(stranger, ORIGIN, { event: "sign", ask: 901, purpose: "media_list" });
  assert.equal(answersTo(stranger).length, 0, "a window that is not the frame gets nothing");
  await ask(wins.frame, "https://elsewhere.example", { event: "sign", ask: 902, purpose: "media_list" });
  assert.ok(!answersTo(wins.frame).some((p) => p.data.ask === 902), "another site gets nothing");
  assert.equal(chat.signed.length, before, "and nothing was signed for either");
  // A purpose that is not the Media page's (the chat signs its messages over the same words): refused.
  await ask(wins.frame, ORIGIN, { event: "sign", ask: 903, purpose: "hello everyone" });
  assert.deepEqual(answersTo(wins.frame).find((p) => p.data.ask === 903).data, { kind: UW.KIND, event: "signed", ask: 903, refused: true }, "a chat message's words are never signed for it");
  assert.equal(chat.signed.length, before, "nothing signed");
  // The frame gone to another page of this site: refused (the page in it is not the one the chat
  // framed), nothing signed for it.
  wins.frame.location.pathname = "/chat";
  await ask(wins.frame, ORIGIN, { event: "sign", ask: 904, purpose: "media_list" });
  assert.deepEqual((answersTo(wins.frame).find((p) => p.data.ask === 904) || {}).data, { kind: UW.KIND, event: "signed", ask: 904, refused: true }, "a frame that left its page is refused");
  assert.equal(chat.signed.length, before, "and nothing is signed for it");
  wins.frame.location.pathname = "/files/7";
  // No unlocked identity in the chat either: a plain refusal, and the page lists unsigned.
  chatKey = "";
  await ask(wins.frame, ORIGIN, { event: "identity", ask: 905 });
  assert.deepEqual(answersTo(wins.frame).find((p) => p.data.ask === 905).data, { kind: UW.KIND, event: "identity", ask: 905, refused: true }, "refused when the chat has no identity either");
  const none = await H.media.signerFor({ identity: null, ownSign: async () => null, itemWindow: conn });
  assert.deepEqual([none.via, none.myKey], ["none", ""], "so the page goes on as one with no identity here");
  chatKey = ME;
  // The window closed with nothing going in it: its frame is gone, and so is the signing.
  host.close();
  assert.equal(host.frame(), null, "test setup: the frame is gone");
  await ask(wins.frame, ORIGIN, { event: "sign", ask: 906, purpose: "media_list" });
  assert.ok(!answersTo(wins.frame).some((p) => p.data.ask === 906), "a frame that is gone gets nothing");
});

// ── 12. A card no longer in the document is let go ────────────────────────

// CARDS KEPT FOREVER (2026-10-18, the orchestrator): `mounted` was pruned only inside render(), which
// runs for an item when it or its picture comes in; a card for an item already in the cache is never
// drawn again, so every card ever put in stayed in memory with its message's element. Now the cards no
// longer in the document are let go at the end of the turn new ones are put in.
// Seen red 2026-10-18 in a copy of web/ through HOS_WEB_DIR, each break in chat/chat-files-card.js: with
// attach's pruneSoon() call taken out: "the card removed from the document is let go" (actual 2); with the
// prune run at once inside attach, not at the end of the turn: "test setup: one card for the item"
// (actual 0: a card was let go before its caller put it in the document).
test("12. a card removed from the document is let go when the next cards go in; the ones still shown stay", async () => {
  const p = cardsPage({ friends: [ANN] });
  const count = (id) => (p.cards.mounted.get(id) || new Set()).size;
  const first = await p.post([7], ANN, 1000);
  assert.equal(count(7), 1, "test setup: one card for the item");
  assert.equal(first.children[0].isConnected, true, "test setup: it is in the document");
  first.remove(); // its message left the page (another conversation opened)
  // The item is in the cache, so nothing draws its cards again (render never runs for it).
  const second = await p.post([7], BEN, 2000);
  assert.equal(count(7), 1, "the card removed from the document is let go");
  assert.equal([...p.cards.mounted.get(7)][0].el, second.children[0], "the one still shown stays");
  // Every card of an item gone: the item itself is let go.
  second.remove();
  await p.post([8], BEN, 3000);
  assert.equal(p.cards.mounted.has(7), false, "no card of it left: nothing kept for it");
  // A card is never let go in the turn it is made, before its caller has put it in the document.
  const made = p.cards.attach([7], { from: ANN, timestamp: 4000 });
  assert.equal(made.children[0].isConnected, false, "test setup: made, not in the document yet");
  p.doc.body.appendChild(made);
  await settle();
  assert.equal(count(7), 1, "made and put in within the turn: kept");
});

// ── 13. Size words, the desktop's and the website's, from one list ────────

// HOW BIG A FILE IS, IN THE SAME WORDS ON BOTH (2026-10-18): the desktop's api::size_words and the
// website's sizeWords are held to the same cases (scripts/tests/fixtures/size-words-cases.json; the
// desktop reads it in its own test). The website keeps two copies, the Media page's (files-app.js) and the
// chat card's (chat-files-card.js): one shared helper would need a new script on the chat page (its load
// order changed) or a home in a file about something else. The 3D view keeps a third
// (/shared/model-view.js). Each is held to the list.
// Seen red 2026-10-18 in a copy of web/ through HOS_WEB_DIR: in chat/chat-files-card.js with oneDecimal
// rounding a half down (Math.floor(x * 10 + 0.5 - 1e-9) / 10): "the chat card's: 1310720" ('1.2 MB' ==
// '1.3 MB'); in pages/files-app.js with kilobytes rounded down (Math.floor): "the Media page's: 1536"
// ('1 KB' == '2 KB').
test("13. size words: the Files page's, the chat card's and the 3D view's all say the shared list's words", () => {
  const fixture = JSON.parse(fs.readFileSync(path.join(__dirname, "fixtures", "size-words-cases.json"), "utf8"));
  assert.ok(Array.isArray(fixture.cases) && fixture.cases.length >= 10, "test setup: the shared cases");
  const MV = require(path.join(WEB, "shared", "model-view.js"));
  for (const [bytes, words] of fixture.cases) {
    assert.equal(H.media.sizeWords(bytes), words, "the Files page's: " + bytes);
    assert.equal(MC.sizeWords(bytes), words, "the chat card's: " + bytes);
    assert.equal(MV.sizeWords(bytes), words, "the 3D view's: " + bytes);
  }
});

// ── 14. The chat lends the item window its upload token ───────────────────

/**
 * Ann's members-only "Dusk" (a 3D model with an added file and a cover picture), every file of it on
 * the members' route, as the relay lists it to a member.
 */
function membersDusk() {
  return dusk({
    visibility: "members", url: "/api/files/file/7",
    files: [
      { file: 0, filename: "7_dusk.glb", original_name: "dusk.glb", kind: "model", size_bytes: 2 * MIB, url: "/api/files/file/7" },
      { file: 1, filename: "8_textures.zip", original_name: "textures.zip", kind: "file", size_bytes: 4000, url: "/api/files/file/7?file=1" },
    ],
    cover: { file: 2, filename: "9_cover.png", kind: "picture", size_bytes: 120 * 1024, url: "/api/files/file/7?file=2" },
  });
}
const NOT_KNOWN = { error: "That upload token is not a session signed in to this server." };
/**
 * The relay's members' routes for `known` (the upload tokens of the sockets signed in now): a file
 * and a ticket only with one of them in the X-Upload-Token header, else the relay's own 403 (media.rs
 * file_core, media_ticket.rs ticket). The signed list answers the members-only items `items`.
 */
function membersRoutes(known, items) {
  let n = 0;
  const has = (c) => known.has(c.headers["X-Upload-Token"]);
  return [
    [at("/api/files/list", "POST"), (c) => respond(200, listAnswer(c.body && c.body.sig ? items : []))],
    [(c) => c.url.startsWith("/api/files/file/"), (c) => (has(c) ? respond(200, {}, new Uint8Array([9, 9, 9])) : respond(403, NOT_KNOWN))],
    [at("/api/files/ticket", "POST"), (c) => {
      if (!has(c)) return respond(403, NOT_KNOWN);
      n++;
      const where = c.body.download ? "/api/files/download/" + c.body.id + "?file=" + c.body.file : "/api/files/file/" + c.body.id + "?file=" + (c.body.file || 0);
      return respond(200, { url: where + "&ticket=t" + n, expires_in_s: 600 });
    }],
    [at("/api/files/viewed", "POST"), () => respond(200, { ok: true })],
  ];
}
/**
 * The chat's half (the opener a card's Open uses, with the chat's signer and its upload token
 * `chatToken()`) and the item window's page for item `id` in the frame over it, with no key of its own
 * and no sign-in of its own (a passphrase-wrapped identity: getPqIdentity found no copy, so the page
 * made no relay session; `session` gives it one). `known`: the tokens the stand-in relay knows.
 */
async function lentPage(o) {
  const wins = linkedWindows();
  Object.assign(wins.frame.location, { pathname: "/files/" + o.id });
  const chat = fakeSigner(ME);
  const open = MC.itemWindowOpener(UW, { window: wins.host, document: fakeDocument(), sign: chat.sign, myKey: () => ME, token: o.chatToken });
  const host = open(o.id, o.items[0], "Dusk");
  host.frame().contentWindow = wins.frame;
  const conn = UW.connectFrame({ window: wins.frame, setTimeout: (fn) => setTimeout(fn, 200), clearTimeout: (t) => clearTimeout(t) });
  const who = await H.media.signerFor({ identity: null, ownSign: async () => null, itemWindow: conn });
  assert.equal(who.via, "chat", "test setup: the page signs through the chat");
  const m = await openPage({
    path: "/files/" + o.id, itemWindow: conn, myKey: who.myKey, sign: who.sign, session: o.session === undefined ? null : o.session,
    routes: listRoutes(listAnswer([]), membersRoutes(o.known, o.items)), modelView: o.modelView || null,
  });
  await settle();
  /** What the page asked the chat for its token, and what the chat answered it. */
  const asks = () => wins.host.posted.filter((x) => x.data && x.data.event === "token");
  const answers = () => wins.frame.posted.filter((x) => x.data && x.data.event === "token");
  return { wins, host, conn, m, asks, answers };
}
const headerOf = (c) => c.headers["X-Upload-Token"];
const callsTo = (m, start) => m.net.calls.filter((c) => c.url.startsWith(start));

// THE ITEM WINDOW HAD NO UPLOAD TOKEN (2026-10-18). Since v0.1513.0 the chat signs for the item window's
// page when the identity is passphrase-wrapped, so the item shows; but its files (the cover, the video's
// Play here, each Download) are fetched with the session's upload token, which a page gets only from its
// own sign-in to the relay, and that page has none. The chat now lends it its own token for this server
// (/shared/upload-window.js {event: 'token', ask} -> {event: 'token', ask, token} or refused), under the
// signing's guards; the page keeps it in memory (files-app.js fileToken) and asks again when the server
// no longer knows it.
// Seen red 2026-10-18 in a copy of web/ through HOS_WEB_DIR, each break: in pages/files-app.js, start()
// not asking and fileToken() not borrowing: "the page asked the chat for its token" (actual 0); in
// shared/upload-window.js, the host's onMessage not passing 'token' on to answerAsk: "the chat answered
// with its own" (actual undefined: no answer); in chat/chat-files-card.js, itemWindowOpener not handing
// `token` to createHost: the same (actual {..., refused: true}); in shared/upload-window.js, the token's
// answer posted for any site ('*'): "posted to that frame, for this site only" (actual '*'); in
// pages/files-app.js, pumpBlobs back on the page's own token(): "the members-only cover, fetched with the
// chat's token and shown" (actual { asked: [], shown: null }); the item view handed the page's own token
// in place of fileToken: "the Download's ticket asked for with the chat's token" (actual { tickets: [],
// saved: [], asks: 1 }); in pages/files-item.js, ticketWith not asking again: "the chat signed in again
// ..." (actual { tickets: ['tok-chat-1', 'tok-chat-1'], saved: [one], asks: 2 }); modelBytes not asking
// again: "View in 3D: ..." (actual { fetched: ['tok-chat-2', 'tok-chat-2'], errors: [null, 'That upload
// token is not a session signed in to this server.'], bytes: [3, 0] }); the ticket's address
// carrying `?token=`: "never in any address"; the lent token put in storage: "never in this browser's
// storage"; in pages/files-app.js, fetchBlob's tokenRefused clause taken out: "the cover the server no
// longer knew the token for ..." (actual { asked: ['tok-old'], shown: null }); in pages/files-item.js,
// ticketFor asking with another token: "Play here: ..." (actual { tickets: ['xtok-chat-1'], src: null }).
test("14.the item window over the chat, with no sign-in of its own, is lent the chat's upload token for a members-only item's cover, Play here and Downloads", async () => {
  const items = [membersDusk()];
  const known = new Set(["tok-chat-1"]);
  let chatTok = "tok-chat-1";
  // A stand-in for /shared/model-view.js's open (Node has no WebGL), fetching the bytes as it does.
  const views = [];
  const modelView = {
    open(container, o) {
      const rec = { o, error: null, bytes: null };
      rec.done = o.fetchBytes(undefined).then((b) => { rec.bytes = b; }, (e) => { rec.error = e; });
      views.push(rec);
      return { close() {} };
    },
  };
  const p = await lentPage({ id: 7, items, known, chatToken: () => chatTok, modelView });
  const { m } = p;
  assert.ok(byId(m.doc, "files-viewer-open"), "test setup: the item's own page");
  assert.equal(p.asks().length, 1, "the page asked the chat for its token");
  const answer = p.answers()[0];
  assert.deepEqual(answer && answer.data, { kind: UW.KIND, event: "token", ask: answer && answer.data.ask, token: "tok-chat-1" }, "the chat answered with its own");
  assert.equal(answer.to, ORIGIN, "posted to that frame, for this site only");
  // The cover: fetched once from the members' route with the chat's token in its header, and shown
  // through an address of the page's own (an object URL), never the route itself.
  const coverOf = (page) => {
    const img = byId(page.doc, "files-viewer-open").querySelector(".files-viewer-img");
    return { asked: callsTo(page, "/api/files/file/7?file=2").map(headerOf), shown: img ? img.src.replace(/\d+$/, "") : null };
  };
  assert.deepEqual(coverOf(m), { asked: ["tok-chat-1"], shown: "blob:test/" }, "the members-only cover, fetched with the chat's token and shown");
  // A Download: a ticket asked for with the same token, the file saved from the ticket's address.
  const downloads = () => ({ tickets: callsTo(m, "/api/files/ticket").map(headerOf), saved: m.saved.map((s) => s.url), asks: p.asks().length });
  byId(m.doc, "files-item-files").querySelector('[data-download="1"]').click();
  await settle();
  assert.deepEqual(downloads(), { tickets: ["tok-chat-1"], saved: ["/api/files/download/7?file=1&ticket=t1"], asks: 1 }, "the Download's ticket asked for with the chat's token, the file saved from its address");
  // The chat signs in again: its old token ends with its socket, and the relay no longer knows it.
  known.clear();
  known.add("tok-chat-2");
  chatTok = "tok-chat-2";
  byId(m.doc, "files-item-files").querySelector('[data-download="1"]').click();
  await settle();
  assert.deepEqual(downloads(), {
    tickets: ["tok-chat-1", "tok-chat-1", "tok-chat-2"],
    saved: ["/api/files/download/7?file=1&ticket=t1", "/api/files/download/7?file=1&ticket=t2"],
    asks: 2,
  }, "the chat signed in again: the Download refused, the chat asked once more, the ticket asked again with its new token and the file saved");
  // View in 3D of its own dusk.glb: the members' file route with the lent token in its header; the chat
  // signing in again meanwhile is the same: asked once more, fetched again with the new one.
  const modelFetches = () => m.net.calls.filter((c) => c.url === "/api/files/file/7").map(headerOf);
  byId(m.doc, "files-view3d").click();
  await settle();
  await views[0].done;
  byId(m.doc, "files-model-close").click();
  known.clear();
  known.add("tok-chat-3");
  chatTok = "tok-chat-3";
  byId(m.doc, "files-view3d").click();
  await settle();
  await views[1].done;
  await settle();
  assert.deepEqual({ fetched: modelFetches(), errors: views.map((v) => (v.error ? v.error.message : null)), bytes: views.map((v) => (v.bytes ? v.bytes.byteLength : 0)) },
    { fetched: ["tok-chat-2", "tok-chat-2", "tok-chat-3"], errors: [null, null], bytes: [3, 3] }, "View in 3D: the model fetched with the lent token, and again with the chat's new one");
  // The token is never in an address, and the page never keeps it anywhere but in memory.
  assert.ok(!m.net.calls.some((c) => /tok-chat/.test(c.url)), "never in any address");
  assert.ok(![...m.storage.m.values()].some((v) => /tok-chat/.test(v)), "never in this browser's storage");

  // A page whose first borrowed token the server already does not know (the chat had signed in again
  // just before): the cover's refusal lets it go, and the chat's new one fetches it.
  let first = true;
  const q = await lentPage({ id: 7, items, known: new Set(["tok-new"]), chatToken: () => { const t = first ? "tok-old" : "tok-new"; first = false; return t; } });
  assert.deepEqual(coverOf(q.m), { asked: ["tok-old", "tok-new"], shown: "blob:test/" }, "the cover the server no longer knew the token for, fetched again with the chat's new one, and shown");

  // A members-only video: its Play here streams from a ticket asked for with the chat's token.
  const clip = H.item({ id: 8, kind: "video", visibility: "members", url: "/api/files/file/8", filename: "8_clip.mp4", original_name: "clip.mp4", title: "Clip", uploader_key: ANN });
  const v = await lentPage({ id: 8, items: [clip], known: new Set(["tok-chat-1"]), chatToken: () => "tok-chat-1" });
  const video = byId(v.m.doc, "files-viewer-open").querySelector(".files-viewer-video");
  assert.deepEqual({ tickets: callsTo(v.m, "/api/files/ticket").map(headerOf), src: video ? video.src : null }, { tickets: ["tok-chat-1"], src: "/api/files/file/8?file=0&ticket=t1" }, "Play here: a ticket asked for with the chat's token, the video streamed from its address");
});

// The guards, and the asking again. Seen red 2026-10-18 in a copy of web/ through HOS_WEB_DIR, each
// break: in shared/upload-window.js, both of the host's checks that the asker is its frame taken out
// (onMessage's source check and answerAsk's frameWindow() !== from; either alone still holds): "a window
// that is not the frame gets nothing" (actual 1); onMessage without its origin check: "another site gets
// nothing"; answerAsk without its onOwnPage checks: "a frame that left its page is refused" (actual
// token: 'tok-chat-2'); answerAsk's tokenShaped check taken out: "refused when the chat holds no token"
// (actual token: ''); both `!frame` checks turned round so a host whose frame is gone still answers: "a
// frame that is gone gets nothing"; in pages/files-app.js, pictureSource's no-token line and pumpBlobs'
// `if (!t) return` taken out: "no members-only file asked for ..." (actual { asked: 1, cover: false });
// borrowToken's refusal leaving no timer: "one ask again, after the wait" (actual 0); the timer not
// asking: "asked again after the wait" (actual 1); no renderAll when the token comes: "then the cover,
// with the chat's token" (actual []); borrows() without its `!session`: "a page signed in itself never
// asks the chat" (actual 1); fileToken() skipping the page's own token: "its own token fetches the
// cover" (actual []).
test("14b.the chat's token goes to its own frame alone, on its page, from this site; a chat with no token, and a page signed in itself, give none", async () => {
  const items = [membersDusk()];
  let chatTok = "tok-chat-2";
  const p = await lentPage({ id: 7, items, known: new Set(["tok-chat-2"]), chatToken: () => chatTok });
  const ask = async (from, origin, n) => {
    p.wins.host.fire("message", { source: from, origin, data: { kind: UW.KIND, event: "token", ask: n } });
    await settle();
  };
  const tokensTo = (w) => w.posted.filter((x) => x.data && x.data.event === "token");
  // A window of this site that is not the frame the chat made (even one on the same address): nothing.
  const stranger = fakeWindow(ORIGIN);
  Object.assign(stranger.location, { pathname: "/files/7", search: "?window=item" });
  await ask(stranger, ORIGIN, 801);
  assert.equal(tokensTo(stranger).length, 0, "a window that is not the frame gets nothing");
  // The frame, but the message from another site: nothing.
  await ask(p.wins.frame, "https://elsewhere.example", 802);
  assert.ok(!tokensTo(p.wins.frame).some((x) => x.data.ask === 802), "another site gets nothing");
  // The frame gone to another page of this site: refused, no token in the answer.
  p.wins.frame.location.pathname = "/chat";
  await ask(p.wins.frame, ORIGIN, 803);
  assert.deepEqual((tokensTo(p.wins.frame).find((x) => x.data.ask === 803) || {}).data, { kind: UW.KIND, event: "token", ask: 803, refused: true }, "a frame that left its page is refused");
  p.wins.frame.location.pathname = "/files/7";
  // The chat holding no token (not signed in, its connection down): refused.
  chatTok = "";
  await ask(p.wins.frame, ORIGIN, 804);
  assert.deepEqual((tokensTo(p.wins.frame).find((x) => x.data.ask === 804) || {}).data, { kind: UW.KIND, event: "token", ask: 804, refused: true }, "refused when the chat holds no token");
  // The window closed: its frame is gone, and so is the lending.
  chatTok = "tok-chat-2";
  p.host.close();
  assert.equal(p.host.frame(), null, "test setup: the frame is gone");
  await ask(p.wins.frame, ORIGIN, 805);
  assert.ok(!tokensTo(p.wins.frame).some((x) => x.data.ask === 805), "a frame that is gone gets nothing");

  // A page opened while the chat holds no token: it waits for one and asks for no file; asked again
  // after the wait (a message to the chat, nothing over the network), the chat's token fetches the cover.
  let tok = "";
  const q = await lentPage({ id: 7, items, known: new Set(["tok-chat-3"]), chatToken: () => tok });
  assert.equal(q.asks().length, 1, "test setup: the page asked");
  assert.equal(q.answers()[0].data.refused, true, "test setup: refused");
  assert.deepEqual({
    asked: callsTo(q.m, "/api/files/file/").length + callsTo(q.m, "/api/files/ticket").length,
    cover: !!byId(q.m.doc, "files-viewer-open").querySelector(".files-viewer-img"),
  }, { asked: 0, cover: false }, "no members-only file asked for, and no cover drawn, without a token");
  tok = "tok-chat-3";
  const retry = q.m.timers.filter((t) => t.ms === 5000);
  assert.equal(retry.length, 1, "one ask again, after the wait");
  retry[0].fn();
  await settle();
  assert.equal(q.asks().length, 2, "asked again after the wait");
  assert.deepEqual(callsTo(q.m, "/api/files/file/7?file=2").map(headerOf), ["tok-chat-3"], "then the cover, with the chat's token");

  // A page with a sign-in of its own (this browser keeps a copy of the key): its own token, and the
  // chat never asked for one.
  const own = await lentPage({ id: 7, items, known: new Set(["tok-1"]), chatToken: () => "tok-chat-9", session: H.fakeSession() });
  assert.equal(own.asks().length, 0, "a page signed in itself never asks the chat");
  assert.deepEqual(callsTo(own.m, "/api/files/file/7?file=2").map(headerOf), ["tok-1"], "its own token fetches the cover");
});
