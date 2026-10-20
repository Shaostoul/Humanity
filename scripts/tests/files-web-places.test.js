// The Files page's places on the website (2026-10-19): web/pages/files-places.js, with the two
// places that are not the page's own list, Shared files (web/pages/files-shared.js, the Shared Files
// page until then) and the HumanityOS folder (web/pages/files-folder.js, the data browser that was at
// /files). The desktop's twin is src/gui/pages/files/places.rs. The operator, 2026-10-19: "What's the
// real difference between these two? Why would we not want to merge them?"
//
// And (5, 2026-10-20) the page's "How this page works" (web/shared/how-it-works.js, the desktop's
// src/gui/pages/files/how.rs and widgets/how_it_works.rs): one small toggle beside the heading,
// closed by default, folding the page's own line and each place's; outside it, where the person
// acts, Shared files' line (they are public) and the HumanityOS folder's caution stay.
//
// Runs under Node's own test runner against the stand-in document of files-web-harness.js:
//   node --test scripts/tests/files-web-places.test.js
"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const H = require("./files-web-harness.js");
const { WEB, ROOT, ME, ANN, MIB, media, byId, text, respond, at, settle, listAnswer, listRoutes, openPage, aFile, fakeSession, fakeStorage } = H;
const FPL = require(path.join(WEB, "pages", "files-places.js"));
const FSH = require(path.join(WEB, "pages", "files-shared.js"));
const FFO = require(path.join(WEB, "pages", "files-folder.js"));
const HOW = require(path.join(WEB, "shared", "how-it-works.js"));

/** A history that keeps what the page asked of it. */
function fakeHistory() {
  const calls = [];
  return { calls, pushState: (_s, _t, url) => calls.push(["push", url]), replaceState: (_s, _t, url) => calls.push(["replace", url]), back: () => calls.push(["back"]) };
}
/** A window that keeps its drag listeners (none of its timers runs by itself). */
function fakeWindow() {
  const listeners = {};
  return {
    listeners,
    addEventListener(t, f) { (listeners[t] = listeners[t] || []).push(f); },
    removeEventListener(t, f) { listeners[t] = (listeners[t] || []).filter((x) => x !== f); },
    setTimeout: () => 0, clearTimeout() {},
    fire(type, dt) {
      const ev = { type, dataTransfer: dt, relatedTarget: null, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; } };
      for (const f of (listeners[type] || []).slice()) f(ev);
      return ev;
    },
  };
}
const filesDrag = (files) => ({ types: ["Files"], files: files || [], dropEffect: "none" });
/** The overlay's line, or null when it is not shown. */
function overlayLine(doc) {
  const el = doc.body.querySelector(".hos-drop-overlay");
  return !el || el.hidden ? null : text(el.children[0]);
}

const SHARED = [
  { name: "bracket.glb", url: "/uploads/1760_bracket.glb", size_bytes: 812000, uploaded_at: 1760000000000, uploader_name: "You", uploader_key: ME },
  { name: "staff-head.stl", url: "/uploads/1761_staff-head.stl", size_bytes: 2 * MIB, uploaded_at: 1760000000000, uploader_name: "Ann", uploader_key: ANN },
];
/** The page's routes and the server's shared files. */
const routes = () => listRoutes(listAnswer([]), [[at("/api/uploads"), () => respond(200, { files: SHARED })]]);

// ── 1. The places, their names and addresses ─────────────────────────────

// Seen red 2026-10-19 with shownPlace letting a member keep #admin: "Admin is an admin's alone".
test("1. the places: two for everyone, the data folder and Admin for an admin, each at its own address", () => {
  assert.deepEqual(FPL.listed(false), ["shared", "shared-files"]);
  assert.deepEqual(FPL.listed(true), ["shared", "shared-files", "folder", "admin"]);
  for (const place of FPL.listed(true)) assert.equal(FPL.placeOf(FPL.addressOf(place).replace("/files", "")), place, "the address names the place: " + place);
  assert.equal(FPL.addressOf("shared"), "/files", "what is shared is the page itself");
  assert.equal(FPL.placeOf("#nothing"), "shared", "an unknown hash shows what is shared");
  assert.equal(FPL.shownPlace("admin", false), "shared", "Admin is an admin's alone");
  assert.equal(FPL.shownPlace("folder", false), "shared", "and so is the server's data folder (BUG-206)");
  assert.equal(FPL.shownPlace("folder", true), "folder");
  assert.equal(FPL.labelOf("shared", "united-humanity.us"), "Shared on united-humanity.us", "the desktop's words");
  assert.equal(FPL.labelOf("shared-files"), "Shared files");
  assert.equal(FPL.labelOf("folder"), "HumanityOS folder");
});

// ── 2. Shared files, a place of the page ─────────────────────────────────

// Seen red 2026-10-19 with the Put button drawn on every row: "a stranger's file: no Put".
test("2. Shared files: the server's files in the left pane, the one chosen in the middle and on the right; Put on the Files page on your own opens the form", async () => {
  const history = fakeHistory();
  const { doc, page, net } = await openPage({ view: "panes", history, routes: routes() });
  assert.equal(byId(doc, "files-place-area").hidden, true, "test setup: what is shared shows first");
  byId(doc, "files-place-shared-files").click();
  await settle();
  assert.deepEqual(history.calls.at(-1), ["replace", "/files#shared-files"], "the address names the place");
  assert.equal(byId(doc, "files-main").hidden, true, "the list of what is shared goes");
  assert.equal(byId(doc, "files-place-area").hidden, false, "and the place's panes show");
  assert.ok(net.calls.some((c) => c.url === "/api/uploads?limit=500"), "the server's shared files asked for");
  const left = byId(doc, "files-place-left");
  assert.ok(left.querySelector("#files-places"), "the places at the top of its left pane");
  const rows = byId(doc, "sf-list").children;
  assert.deepEqual(rows.map((r) => text(r.querySelector(".sf-name-text"))), ["bracket.glb", "staff-head.stl"], "a row per file");
  assert.equal(text(byId(doc, "files-place-middle")), "Choose a file on the left to see it here.", "nothing chosen yet");

  rows[1].click();
  assert.ok(text(byId(doc, "files-place-middle")).includes("staff-head.stl"), "the file chosen, in the middle");
  assert.equal(byId(doc, "files-place-middle").querySelector(".sf-dl").href, "/uploads/1761_staff-head.stl", "with its Download");
  assert.equal(byId(doc, "sf-put"), null, "a stranger's file: no Put");

  byId(doc, "sf-list").children[0].click();
  const put = byId(doc, "sf-put");
  assert.equal(text(put), "Put on the Files page", "your own file: Put on the Files page");
  put.click();
  await settle();
  assert.equal(byId(doc, "files-main").hidden, false, "back to what is shared");
  assert.equal(byId(doc, "files-place-area").hidden, true);
  assert.deepEqual(history.calls.at(-1), ["replace", "/files"]);
  assert.ok(page.state.form && page.state.form.mode === "put" && page.state.form.filename === "1760_bracket.glb", "the form for it, without the file picker");
});

// ── 3. /files#shared-files and /files#folder open their place ─────────────

test("3. the page opened at a place's address shows that place", async () => {
  const shared = await openPage({ view: "panes", hash: "#shared-files", routes: routes() });
  assert.equal(byId(shared.doc, "files-place-area").hidden, false, "/files#shared-files opens Shared files");
  assert.equal(byId(shared.doc, "files-place-shared-files").getAttribute("aria-current"), "page");
  const folder = await openPage({ view: "panes", hash: "#folder", session: fakeSession({ role: "admin" }), routes: routes() });
  assert.equal(byId(folder.doc, "files-place-folder").getAttribute("aria-current"), "page", "/files#folder opens the HumanityOS folder for an admin");
  assert.equal(byId(folder.doc, "files-main").hidden, true);
});

// ── 4. Files dropped on Shared files go to Shared Files ───────────────────

// Seen red 2026-10-19 with the page's own decision kept for every place: "over Shared files: Drop to
// upload to Shared Files" (actual 'Drop to upload to Files').
test("4. files dragged over Shared files go to Shared Files, not Add to Files; over what is shared, Add to Files", async () => {
  const win = fakeWindow();
  const { doc, page } = await openPage({ view: "panes", window: win, session: fakeSession(), routes: routes() });
  win.fire("dragenter", filesDrag());
  assert.equal(overlayLine(doc), "Drop to upload to Files", "over what is shared: Add to Files");
  win.fire("dragleave", filesDrag());
  win.fire("drop", filesDrag([]));
  byId(doc, "files-place-shared-files").click();
  await settle();
  win.fire("dragenter", filesDrag());
  assert.equal(overlayLine(doc), "Drop to upload to Shared Files", "over Shared files: Drop to upload to Shared Files");
  win.fire("drop", filesDrag([aFile("part.stl", 1024)]));
  await settle();
  assert.equal(page.state.form, null, "and Add to Files did not open");
});

// ── 5. How this page works ───────────────────────────────────────────────

const readText = (p) => fs.readFileSync(p, "utf8").replace(/\r\n/g, "\n");
/** A `pub const NAME: &str = "...";` string out of one of the desktop's files. */
function rustConst(name, ...file) {
  const src = readText(path.join(ROOT, "src", "gui", ...file));
  const m = src.match(new RegExp("pub const " + name + ': &str = "((?:[^"\\\\]|\\\\.)*)";'));
  assert.ok(m, file.join("/") + " has " + name);
  return m[1].replace(/\\"/g, '"').replace(/\\\\/g, "\\");
}
/** The words a person sees: every text not inside a hidden element. */
function visibleText(n) {
  if (n.nodeType === 3) return n.textContent;
  if (n.hidden) return "";
  return n.childNodes.map(visibleText).join("\n");
}
/** The fold's parts as [name, colour, [its lines]]. */
function foldParts(fold) {
  const out = [];
  for (const c of fold.children) {
    if (c.tagName === "H2") out.push([text(c), c.getAttribute("data-tint"), []]);
    else if (c.tagName === "P" && out.length) out[out.length - 1][2].push(text(c));
  }
  return out;
}
const PAGE_LINE = rustConst("PAGE_LINE", "pages", "files", "how.rs");

// Seen red 2026-10-20, each break made in a copy of web/ (HOS_WEB_DIR) with the other tests of
// this file passing: with the page's line drawn under the heading again (where the short "chose to
// share" line was), "closed, the page's line is nowhere on the page" failed (actual false); with
// the Shared files place's left-pane line taken out (files-shared.js), "Shared files' line stays
// where its files are: they are public" failed; with the folder place's caution left blank
// (files-folder.js), "the folder's caution stays beside the file" failed; with the fold's
// HumanityOS folder part left out, "open, each place's line under its name" failed (and 5b's
// "open from the start, its parts drawn": actual 2, expected 3); with readOpen answering open
// unless "0" is kept (how-it-works.js), "closed by default" failed (actual 'true'). Since
// 2026-10-20 the folder and its part in the fold are an admin's (BUG-206): 6 holds them.
test("5a. How this page works: a toggle beside the heading, closed by default; Shared files' line and the folder's caution stay where they are acted on; opened, the page's line and each place's", async () => {
  assert.equal(HOW.LABEL, rustConst("LABEL", "widgets", "how_it_works.rs"), "the desktop's LABEL");
  assert.equal(HOW.TIP, rustConst("TIP", "widgets", "how_it_works.rs"), "the desktop's TIP");
  assert.equal(media.FILES_WORDS.pageLine, PAGE_LINE, "the desktop's PAGE_LINE, word for word");
  const storage = fakeStorage();
  const { doc } = await openPage({ view: "panes", storage, routes: routes() });
  const toggle = byId(doc, "files-how-toggle");
  const fold = byId(doc, "files-how");
  assert.ok(toggle && fold, "the toggle and its fold are on the page");
  assert.equal(toggle.parentNode, byId(doc, "files-title").parentNode, "beside the heading");
  assert.equal(toggle.tagName, "BUTTON", "a real button (Tab reaches it, Enter and Space press it)");
  assert.equal(toggle.getAttribute("type"), "button");
  assert.equal(text(toggle), HOW.LABEL);
  assert.equal(toggle.getAttribute("title"), HOW.TIP);
  assert.equal(toggle.getAttribute("aria-controls"), "files-how");

  // Closed.
  assert.equal(toggle.getAttribute("aria-expanded"), "false", "closed by default");
  assert.equal(fold.hidden, true);
  assert.equal(fold.children.length, 0, "closed, the fold holds nothing");
  assert.ok(!visibleText(doc.body).includes(PAGE_LINE), "closed, the page's line is nowhere on the page");
  assert.ok(!visibleText(doc.body).includes("chose to share"), "and the short line that was under the heading is gone");
  byId(doc, "files-place-shared-files").click();
  await settle();
  assert.ok(visibleText(byId(doc, "files-place-left")).includes(FSH.WORDS.how), "Shared files' line stays where its files are: they are public");
  assert.ok(FSH.WORDS.how.startsWith("Files here are public: anyone can download them."));
  assert.ok(!byId(doc, "files-how-toggle").hidden && visibleText(doc.body).includes(HOW.LABEL), "the toggle on every place");
  // The HumanityOS folder's caution stays on its right pane, beside the file (the place draws its
  // parts with markup, so it is made here against a document that allows it).
  const made = [];
  const node = (tag) => { const n = { tagName: tag.toUpperCase(), className: "", id: "", textContent: "", kids: [], set innerHTML(v) { this.html = v; }, append(...k) { this.kids.push(...k); } }; made.push(n); return n; };
  const before = globalThis.document;
  globalThis.document = { createElement: node };
  let place;
  try { place = FFO.createFolderPlace(); } finally { globalThis.document = before; }
  const caution = place.right.kids.find((k) => k.textContent === FFO.CAUTION);
  assert.ok(caution && caution.className === "files-hint", "the folder's caution stays beside the file");
  assert.match(FFO.CAUTION, /Changing one changes how the app works/);

  // Opened.
  toggle.click();
  assert.equal(toggle.getAttribute("aria-expanded"), "true", "aria-expanded follows");
  assert.equal(fold.hidden, false);
  assert.deepEqual(foldParts(fold), [
    ["Shared on united-humanity.us", "red", [PAGE_LINE]],
    ["Shared files", "red", [FSH.WORDS.how]],
  ], "open, each place's line under its name (a member's: the data folder is an admin's, 6)");
  assert.equal(storage.m.get(media.HOW_KEY), "1", "kept, under hos_files_how_open");
  assert.equal(media.HOW_KEY, "hos_files_how_open");
  toggle.click();
  assert.equal(toggle.getAttribute("aria-expanded"), "false", "pressed again, it closes");
  assert.equal(fold.hidden, true);
  assert.equal(storage.m.get(media.HOW_KEY), "0");
});

// Seen red 2026-10-20 in a copy of web/ with files-app.js giving the fold no storage (`storage:
// null`): "it survives a reload" failed (actual 'false', expected 'true'; and 5a's "kept, under
// hos_files_how_open").
test("5b. How this page works survives a reload; storage refused, the page still draws and the fold opens; a kept value that is not 1 is closed", async () => {
  const storage = fakeStorage();
  const first = await openPage({ view: "panes", storage, routes: routes() });
  byId(first.doc, "files-how-toggle").click();
  const again = await openPage({ view: "panes", storage, routes: routes() });
  assert.equal(byId(again.doc, "files-how-toggle").getAttribute("aria-expanded"), "true", "it survives a reload");
  assert.equal(byId(again.doc, "files-how").hidden, false);
  assert.equal(foldParts(byId(again.doc, "files-how")).length, 2, "open from the start, its parts drawn");
  byId(again.doc, "files-how-toggle").click();
  const closed = await openPage({ view: "panes", storage, routes: routes() });
  assert.equal(byId(closed.doc, "files-how-toggle").getAttribute("aria-expanded"), "false", "closed, and that is kept too");

  const refused = await openPage({ view: "panes", storage: fakeStorage(true), routes: routes() });
  const t = byId(refused.doc, "files-how-toggle");
  assert.equal(t.getAttribute("aria-expanded"), "false", "storage refused: closed");
  t.click();
  assert.equal(t.getAttribute("aria-expanded"), "true", "and it still opens");
  assert.ok(byId(refused.doc, "files-title"), "the page drew");

  for (const kept of ["true", "open", "0"]) {
    const s = fakeStorage();
    s.setItem(media.HOW_KEY, kept);
    const odd = await openPage({ view: "panes", storage: s, routes: routes() });
    assert.equal(byId(odd.doc, "files-how-toggle").getAttribute("aria-expanded"), "false", JSON.stringify(kept) + " kept: closed");
  }

  // files.html links the styles and loads the script before files-app.js.
  const html = readText(path.join(WEB, "pages", "files.html"));
  assert.match(html, /<link rel="stylesheet" href="\/shared\/how-it-works\.css\?v=\d+">/, "files.html links how-it-works.css");
  const s = html.indexOf('src="/shared/how-it-works.js'), a = html.indexOf('src="/pages/files-app.js');
  assert.ok(s > 0 && s < a, "how-it-works.js loads before files-app.js");
});

// ── 6. The server's data folder is an admin's alone (BUG-206) ─────────────

/**
 * A document of plain nodes for files-folder.js, which draws with markup: each element asked for
 * by id is made the first time and kept, with what was written into it (innerHTML reads back the
 * escaped textContent until markup is written, as a browser's does).
 */
function folderDocument() {
  const node = () => ({
    textContent: "", style: {}, dataset: {}, className: "", children: [],
    get innerHTML() { return this.html != null ? this.html : String(this.textContent).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;"); },
    set innerHTML(v) { this.html = v; },
    classList: { add() {}, remove() {}, toggle() {} },
    appendChild(c) { this.children.push(c); return c; },
    append(...k) { this.children.push(...k); },
    insertAdjacentElement() {}, addEventListener() {}, querySelector: () => null, remove() {},
  });
  const ids = new Map();
  return {
    getElementById(id) { if (!ids.has(id)) ids.set(id, node()); return ids.get(id); },
    createElement: () => node(),
    querySelectorAll: () => [],
  };
}
/**
 * files-folder.js run with `signer` as this browser's getPqSignedAuth and `answer(url, body)` as
 * the server, `then(doc)` after its tree is read: what it asked of the server, and its tree.
 */
async function withFolder(signer, answer, then) {
  const calls = [];
  const names = ["document", "window", "fetch", "CSS", "confirm"];
  const before = names.map((n) => [n, Object.getOwnPropertyDescriptor(globalThis, n)]);
  const doc = folderDocument();
  Object.assign(globalThis, {
    document: doc, window: { getPqSignedAuth: signer }, CSS: { escape: (s) => s }, confirm: () => true,
    fetch: async (url, init) => { calls.push({ url, init }); return { json: async () => answer(url, init && init.body ? JSON.parse(init.body) : null) }; },
  });
  try {
    await FFO.refreshTree();
    if (then) await then(doc);
  } finally {
    for (const [n, d] of before) { if (d) Object.defineProperty(globalThis, n, d); else delete globalThis[n]; }
  }
  return { calls, tree: doc.getElementById("tree-list").innerHTML };
}

// Found 2026-10-20 by the review of that night's merges: GET /api/data-files and /read served any
// text file under the server's data/ to anyone, a fresh server's claim code among them, and this
// place was listed for everyone. Seen red the same day: with a member's places listing the folder
// again (files-places.js `listed`), "a member has no data folder place" failed (actual
// [FakeElement], expected null); with the listing back on the unsigned GET (fetchDir), "no
// identity: nothing is asked of the server" failed (actual 1, expected 0).
test("6. the server's data folder is an admin's alone: a member has no such place, even at /files#folder, nor its line; it lists and reads by a signed POST", async () => {
  const member = await openPage({ view: "panes", hash: "#folder", routes: routes() });
  assert.equal(byId(member.doc, "files-place-folder"), null, "a member has no data folder place");
  assert.equal(byId(member.doc, "files-main").hidden, false, "at /files#folder a member sees what is shared");
  assert.equal(byId(member.doc, "files-place-area").hidden, true);
  const storage = fakeStorage();
  storage.setItem(media.HOW_KEY, "1");
  const admin = await openPage({ view: "panes", storage, session: fakeSession({ role: "admin" }), routes: routes() });
  assert.ok(byId(admin.doc, "files-place-folder"), "an admin has it");
  assert.deepEqual(foldParts(byId(admin.doc, "files-how")).map((p) => p[0]), ["Shared on united-humanity.us", "Shared files", FPL.WORDS.folder], "and its line in the fold");

  // No identity in this browser: the server is not asked, and the tree says why.
  const none = await withFolder(async () => null, () => ({}));
  assert.equal(none.calls.length, 0, "no identity: nothing is asked of the server");
  assert.ok(none.tree.includes("admins can open or change its data folder"), "the tree says the folder is for admins: " + none.tree);

  // The server refusing: its own words in the tree.
  const signer = async (purpose) => ({ key: "kkkk", timestamp: 1760000000123, sig: "sig:" + purpose });
  const refused = await withFolder(signer, () => ({ error: "Only this server's admins can open or change its data folder." }));
  assert.ok(refused.tree.includes("open or change its data folder"), "the server's refusal in the tree");

  // An admin's identity: the list and the read are POSTs, signed over the path, with no path or
  // signature in the address.
  const entries = [{ name: "notes.txt", path: "data/notes.txt", is_directory: false, extension: "txt", size: 5, modified: 0 }];
  const asked = await withFolder(signer, (url, body) => (url === "/api/data-files" ? { path: body.path, entries } : { path: body.path, content: "notes", size: 5, modified: 0 }), async (doc) => {
    const row = doc.getElementById("tree-list").children[0];
    assert.ok(row && typeof row.onclick === "function", "the file is in the tree");
    row.onclick();
    await settle();
  });
  assert.deepEqual(asked.calls.map((c) => [c.url, c.init && c.init.method]), [["/api/data-files", "POST"], ["/api/data-files/read", "POST"]], "a POST to list, then one to read, the addresses bare");
  assert.deepEqual(JSON.parse(asked.calls[0].init.body), { path: "data", key: "kkkk", timestamp: 1760000000123, sig: "sig:data_files_list\ndata" }, "the listing signed over its folder (src/relay/api.rs list_purpose)");
  assert.deepEqual(JSON.parse(asked.calls[1].init.body), { path: "data/notes.txt", key: "kkkk", timestamp: 1760000000123, sig: "sig:data_files_read\ndata/notes.txt" }, "the read signed over its file (read_purpose)");
});
