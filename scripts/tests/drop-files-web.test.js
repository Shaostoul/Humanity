// Files dragged from the computer's file manager onto the website (2026-10-17): the web half of the
// drag and drop the desktop app has (src/gui/drop_files.rs), in its words. The chat's half is held
// by scripts/tests/private-files-web.test.js (its "drop:" tests), which has the chat's harness.
//
// Run: node --test scripts/tests/drop-files-web.test.js   (in `just rig-tests`)
//
// What runs: web/shared/drop-overlay.js (the overlay and the drag events), the Media page
// (web/pages/files-app.js with files-edit.js and files-item.js, on the stand-ins of
// scripts/tests/files-web-harness.js: a document holding only what the page built, where reading or
// setting innerHTML throws, and a network that records every request), and the Shared Files
// upload (web/pages/shared-files-upload.js) with the REAL cleaning (/shared/upload-clean.js) and
// real Files. The window is a stand-in that keeps its listeners and its timers (none runs by
// itself) and fires drag events as a browser does: "Files" among the drag's types, the files
// themselves only at the drop (a browser hides them until then). HOS_WEB_DIR points the test at
// another copy of web/ (used to see each test red).
//
// What it proves:
//  1. The overlay: one large line, nothing else; it stays while the drag crosses the page's own
//     elements (dragenter and dragleave counted), goes when the drag leaves the window, when no
//     dragover has come for a while (a drag that left without a last dragleave), and at the drop;
//     the drop hands over the files. Refused: "Files cannot be dropped here." and the reason,
//     dropEffect "none", the drop does nothing (but is still the page's, so the browser does not
//     open the file in the tab). A dragged link or text is left alone.
//  2. The Media page, signed in: "Drop to upload to Media", and the drop opens Add to Media with the
//     dropped files exactly as choosing them does (the first is the item, the rest added once it
//     is up). An item open but not being edited closes, so the form is seen.
//  3. The Media item's Edit open: "Drop to add to Dusk", and the drop adds the file to the item as
//     its Add a file does (the resumable upload, then files/add; never published).
//  4. The Media page where a drop would do nothing: signed out, a role that cannot upload, the
//     Admin tab, an admin's Edit of someone else's item; each says why in the page's own words.
//  5. Shared Files, signed in: "Upload a file" is offered; "Drop to upload to Shared Files"; the
//     drop checks every size first (those over the limit named in one line), uploads the rest with
//     this session's token and share=1, exactly as the button's picker does, and the list is
//     loaded again.
//  6. Shared Files where a drop would do nothing: no identity here, a role that cannot upload;
//     the button is not offered, and the page and the overlay give the same reason.
//  7. Shared Files cleans a file before it goes: a phone photo shows the step (what is hidden, the
//     switch); Upload sends the cleaned photo, Cancel sends nothing.
//  8. Every page that takes files loads the overlay's script and stylesheet before its own script,
//     and the stylesheet uses the theme's variables only, the dialogs' dimming among them.
//
// Red first: each test names the break it was seen failing on.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const H = require("./files-web-harness.js");
const { ROOT, WEB, ME, ANN, MIB, byId, text, fakeDocument, respond, at, path0, fakeFetch, fakeSession, fakeStorage, settle, listAnswer, listRoutes, openPage, resumableRelay, aFile } = H;
const D = require(path.join(WEB, "shared", "drop-overlay.js"));
const SFU = require(path.join(WEB, "pages", "shared-files-upload.js"));
const UC = require(path.join(WEB, "shared", "upload-clean.js"));
const B = require("./file-bytes.js");

const FX = JSON.parse(fs.readFileSync(path.join(ROOT, "scripts", "tests", "fixtures", "files-round5-v1.json"), "utf8"));
const CATS = FX.list_by_downloads.categories;
const L = FX.licences;
const FILES_WORDS = H.media.FILES_WORDS;

// ── The window, the drags and the overlay ────────────────────────────────

/** A window that keeps its listeners and its timers (none runs by itself). */
function fakeWindow() {
  const listeners = {};
  const timers = [];
  return {
    listeners, timers,
    addEventListener(t, f) { (listeners[t] = listeners[t] || []).push(f); },
    removeEventListener(t, f) { listeners[t] = (listeners[t] || []).filter((x) => x !== f); },
    setTimeout(fn, ms) { timers.push({ fn, ms, live: true }); return timers.length; },
    clearTimeout(id) { const t = timers[id - 1]; if (t) t.live = false; },
    /** Run every timer still waiting (as if the time had passed). */
    runTimers() { for (const t of timers.slice()) if (t.live) { t.live = false; t.fn(); } },
    /** Fire a drag event as a browser does; the event, to read what the page did with it. */
    fire(type, dt) {
      const ev = { type, dataTransfer: dt, relatedTarget: null, defaultPrevented: false, preventDefault() { this.defaultPrevented = true; } };
      for (const f of (listeners[type] || []).slice()) f(ev);
      return ev;
    },
  };
}
/** A drag from the computer's file manager: "Files" among its types; its files only at the drop. */
const filesDrag = (files) => ({ types: ["Files"], files: files || [], dropEffect: "none" });
/** A dragged link or piece of text. */
const textDrag = () => ({ types: ["text/plain", "text/uri-list"], files: [], dropEffect: "none" });
/** The overlay as a person sees it ({line, reason, refused}), or null when it is not shown. */
function overlayOf(doc) {
  const el = doc.body.querySelector(".hos-drop-overlay");
  if (!el || el.hidden) return null;
  assert.equal(el.children.length, 2, "the overlay holds its line and its reason, nothing else");
  const [line, reason] = el.children;
  return { line: text(line), reason: reason.hidden ? "" : text(reason), refused: el.className.split(/\s+/).includes("hos-drop-refused") };
}
/** Drag files in over the page, then drop `files`; the drop's event. */
async function dropOn(win, files) {
  win.fire("dragenter", filesDrag());
  win.fire("dragover", filesDrag());
  const ev = win.fire("drop", filesDrag(files));
  await settle();
  return ev;
}
async function until(done, turns) {
  for (let i = 0; i < (turns || 40) && !done(); i++) await settle();
}

// ── 1. The overlay ───────────────────────────────────────────────────────

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR: with onLeave hiding on every
// dragleave (no count): "crossing a child element keeps it"; with the stale timer never armed:
// "a drag that left without a last dragleave: gone once no dragover comes"; with onDrop handing
// over the files before asking again: "a refused drop hands nothing over" (2 handed over against 1).
// Seen red again 2026-10-17 after the desktop's words changed, with WORDS.uploadToFiles put back to
// 'Drop to add to Media': "'Drop to add to Media' == 'Drop to upload to Media'".
test("1. the overlay: one line while files are over the page, gone when they leave or drop; refused says why; text is ignored", async () => {
  const doc = fakeDocument();
  const win = fakeWindow();
  let decision = { take: true, line: "Drop to upload to Files" };
  const got = [];
  const o = D.attach({ window: win, document: doc, decide: () => decision, onDrop: (files) => got.push(files) });
  assert.equal(overlayOf(doc), null, "nothing before a drag");

  const enter = win.fire("dragenter", filesDrag());
  assert.ok(enter.defaultPrevented, "a file drag is the page's");
  assert.deepEqual(overlayOf(doc), { line: "Drop to upload to Files", reason: "", refused: false }, "the line, and nothing else");
  const over = win.fire("dragover", filesDrag());
  assert.ok(over.defaultPrevented && over.dataTransfer.dropEffect === "copy", "dragover: the page's, copy");
  win.fire("dragenter", filesDrag()); // a child element
  win.fire("dragleave", filesDrag()); // the element before it
  assert.ok(overlayOf(doc), "crossing a child element keeps it");
  win.fire("dragleave", filesDrag()); // out of the window
  assert.equal(overlayOf(doc), null, "leaving the window takes it away");

  win.fire("dragenter", filesDrag());
  win.fire("dragover", filesDrag());
  win.runTimers();
  assert.equal(overlayOf(doc), null, "a drag that left without a last dragleave: gone once no dragover comes");
  win.fire("dragenter", filesDrag());
  assert.ok(overlayOf(doc), "and it comes back with the next drag, its count started afresh");
  win.fire("dragleave", filesDrag());
  assert.equal(overlayOf(doc), null, "one leave for that one enter");

  const a = new File(["a"], "a.txt");
  const b = new File(["b"], "b.txt");
  win.fire("dragenter", filesDrag());
  const drop = win.fire("drop", filesDrag([a, b]));
  assert.ok(drop.defaultPrevented, "the drop is the page's, never the browser's (which would open the file in the tab)");
  assert.equal(overlayOf(doc), null, "the drop takes it away");
  assert.deepEqual(got, [[a, b]], "and hands over the files");

  decision = { take: false, reason: "Signing in to this server..." };
  win.fire("dragenter", filesDrag());
  const no = win.fire("dragover", filesDrag());
  assert.deepEqual(overlayOf(doc), { line: D.WORDS.cannot, reason: "Signing in to this server...", refused: true }, "refused: the line and the reason");
  assert.equal(D.WORDS.cannot, "Files cannot be dropped here.");
  assert.ok(no.defaultPrevented && no.dataTransfer.dropEffect === "none", "the pointer says no; the browser still does not open the file");
  const refusedDrop = win.fire("drop", filesDrag([a]));
  assert.ok(refusedDrop.defaultPrevented, "a refused drop is still the page's");
  assert.equal(got.length, 1, "a refused drop hands nothing over");
  assert.equal(overlayOf(doc), null);

  decision = { take: true, line: "Drop to upload to Files" };
  const t1 = win.fire("dragenter", textDrag());
  const t2 = win.fire("dragover", textDrag());
  const t3 = win.fire("drop", textDrag());
  assert.equal(overlayOf(doc), null, "a dragged link or text: no overlay");
  assert.ok(!t1.defaultPrevented && !t2.defaultPrevented && !t3.defaultPrevented, "left to the browser (text into a box)");
  assert.equal(got.length, 1, "and nothing handed over");

  o.detach();
  win.fire("dragenter", filesDrag());
  assert.equal(overlayOf(doc), null, "detached: gone, and no longer listening");
  assert.equal(D.sendIn("#general"), "Drop to send in #general");
  assert.equal(D.addTo("Dusk"), "Drop to add to Dusk");
  assert.equal(D.WORDS.uploadToFiles, "Drop to upload to Files");
  assert.equal(D.uploadAndPostIn("#general"), "Drop to upload to Files and post in #general");
  assert.equal(D.WORDS.sharedFiles, "Drop to upload to Shared Files");
});

// ── 2. The Media page: Add to Media ─────────────────────────────────────

/** What the Add to Media form shows: its chosen file's row and the further files' rows. */
const formRows = (doc) => [text(byId(doc, "files-form").querySelector(".files-form-file")), byId(doc, "files-form-more").querySelectorAll(".files-form-more-file").map(text)];

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR: with files-app.js dropped() opening
// the form with the first file only (`openAddForm(files[0])`): "the rest, to be added once it is up"
// (actual [] against [dawn.mp4]); with dropDecision's line for the page made Shared Files' one
// (DROP.WORDS.sharedFiles): "over the page: Drop to add to Media" (the words then; the desktop's now are
// "Drop to upload to Media", 2026-10-17). Seen red again that day with the line put back to
// 'Drop to add to Media': "over the page: Drop to upload to Media" (actual { line: 'Drop to add to
// Media', ... }).
test("2. the Files page: Drop to upload to Files opens Add to Files with the dropped files, as choosing them does", async () => {
  const win = fakeWindow();
  const { doc, page } = await openPage({ window: win, routes: listRoutes(listAnswer([])) });
  assert.ok(byId(doc, "files-add-btn"), "test setup: signed in, Add to Files offered");
  win.fire("dragenter", filesDrag());
  assert.deepEqual(overlayOf(doc), { line: "Drop to upload to Files", reason: "", refused: false }, "over the page: Drop to upload to Files");
  const dusk = aFile("dusk.mp4", 3 * MIB);
  const dawn = aFile("dawn.mp4", 2 * MIB);
  const drop = win.fire("drop", filesDrag([dusk, dawn]));
  await settle();
  assert.ok(drop.defaultPrevented);
  assert.equal(overlayOf(doc), null, "gone at the drop");
  const f = page.state.form;
  assert.ok(f && f.mode === "add", "Add to Files opened");
  assert.equal(f.file, dusk, "the first dropped file is the item");
  assert.deepEqual(f.more, [dawn], "the rest, to be added once it is up");

  // Chosen with the button instead: the same form.
  const picked = await openPage({ routes: listRoutes(listAnswer([])) });
  const input = byId(picked.doc, "files-file-input");
  input.files = [dusk, dawn];
  input.dispatch("change");
  await settle();
  assert.equal(picked.page.state.form.file, dusk);
  assert.deepEqual(picked.page.state.form.more, [dawn]);
  assert.deepEqual(formRows(doc), formRows(picked.doc), "drawn the same: " + JSON.stringify(formRows(doc)));

  // An item open (not being edited) covers the page: a drop closes it, so the form is seen.
  const item = H.item();
  const w2 = fakeWindow();
  const viewing = await openPage({ window: w2, routes: listRoutes(listAnswer([item])) });
  viewing.page.openViewer(viewing.page.state.items[0]);
  assert.ok(byId(viewing.doc, "files-viewer-open"), "test setup: an item is open");
  w2.fire("dragenter", filesDrag());
  assert.equal(overlayOf(viewing.doc).line, "Drop to upload to Files", "an item open without its Edit: still Add to Files");
  await dropOn(w2, [dusk]);
  assert.equal(byId(viewing.doc, "files-viewer-open"), null, "the item closed");
  assert.equal(viewing.page.state.form && viewing.page.state.form.file, dusk, "and Add to Files opened with the file");
});

// ── 3. The Media item's Edit ─────────────────────────────────────────────

const fxItem = (over) => Object.assign({}, FX.item_members_view, { labels: [], uploader_key: ANN }, over || {});
const answerOf = (items) => listAnswer(items, { categories: CATS, licences: L });

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR: with dropDecision ignoring an open
// Edit (Add to Media's line and path): "with Edit open: Drop to add to Dusk" (actual "Drop to add to
// Media"); with dropped() handing the files to Add to Media while Edit is open: "added to the item,
// never published" (no files/add).
test("3. with an item's Edit open: Drop to add to Dusk, and the drop adds the file as Add a file does", async () => {
  const mine = fxItem({ uploader_key: ME, uploader_name: "Me" });
  const relay = resumableRelay();
  const added = Object.assign({}, mine, { file_count: 3, files: mine.files.concat([{ file: 5, filename: "1760_clip.webm", original_name: "clip.webm", kind: "video", size_bytes: 20000, url: "/uploads/1760_clip.webm" }]) });
  const win = fakeWindow();
  const { doc, page, net } = await openPage({ window: win, routes: listRoutes(answerOf([mine]), relay.routes.concat([
    [at("/api/files/item-files/add", "POST"), () => respond(200, { ok: true, item: added })],
  ])) });
  page.openViewer(page.state.items[0]);
  byId(doc, "files-item-edit-open").click();
  assert.ok(byId(doc, "files-edit-add-file"), "test setup: Edit is open, with Add a file");
  win.fire("dragenter", filesDrag());
  assert.deepEqual(overlayOf(doc), { line: "Drop to add to Dusk", reason: "", refused: false }, "with Edit open: Drop to add to Dusk");
  await dropOn(win, [aFile("clip.webm", 20000, { type: "video/webm" })]);
  await until(() => net.calls.some((c) => c.url === "/api/files/item-files/add"));
  const asked = net.calls.map((c) => (c.init.method || "GET") + " " + path0(c.url));
  const add = net.calls.find((c) => c.url === "/api/files/item-files/add");
  assert.ok(add, "added to the item, never published: " + asked.join(", "));
  assert.ok(relay.log.some((l) => l.kind === "start" && l.c.body.media === 1 && l.c.body.filename === "clip.webm"), "the same resumable upload as Add a file's");
  assert.deepEqual([add.body.id, add.body.add, add.body.as], [1, "1760_clip.webm", "file"], "as one of its files");
  assert.ok(!net.calls.some((c) => c.url === "/api/files" && (c.init.method || "GET") === "POST"), "never published");
  assert.equal(page.state.form, null, "Add to Files was not opened");
});

// ── 4. The Media page where a drop would do nothing ─────────────────────

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR: with dropDecision taking the files
// whenever the person has a key (`if (myKey)`): "a role that cannot upload: its words"; with
// files-edit.js giving an admin's Edit of someone else's item addFiles: "only its uploader adds
// files: the desktop's words" (it said "Drop to add to Dusk").
test("4. the Files page says why a drop would do nothing, in its own words, and nothing happens", async () => {
  const refusedOn = async (opts, setup) => {
    const win = fakeWindow();
    const p = await openPage(Object.assign({ window: win }, opts));
    if (setup) setup(p);
    win.fire("dragenter", filesDrag());
    const seen = overlayOf(p.doc);
    await dropOn(win, [aFile("dusk.mp4", MIB)]);
    return { seen, p };
  };
  const signedOut = await refusedOn({ myKey: "", session: null, routes: listRoutes(listAnswer([])) });
  assert.deepEqual(signedOut.seen, { line: "Files cannot be dropped here.", reason: FILES_WORDS.noIdentity, refused: true }, "signed out: why");
  assert.equal(signedOut.p.page.state.form, null, "and the drop does nothing");

  const cannot = await refusedOn({ session: fakeSession({ role: "unverified" }), routes: listRoutes(listAnswer([])) });
  assert.deepEqual(cannot.seen, { line: "Files cannot be dropped here.", reason: FILES_WORDS.cannotUpload, refused: true }, "a role that cannot upload: its words");
  assert.equal(text(byId(cannot.p.doc, "files-cannot-upload")), FILES_WORDS.cannotUpload, "the same words the page shows where Add to Files would be");
  assert.equal(cannot.p.page.state.form, null);

  const adminTab = await refusedOn({ session: fakeSession({ role: "admin" }), hash: "#admin", routes: listRoutes(listAnswer([])) });
  assert.deepEqual(adminTab.seen, { line: "Files cannot be dropped here.", reason: "Add to Files is with what is shared on this server, the first place.", refused: true }, "the Admin place: Add to Files is not on it");
  assert.equal(adminTab.p.page.state.form, null);

  const anns = fxItem({ uploader_key: ANN });
  const admin = await refusedOn({ session: fakeSession({ role: "admin" }), routes: listRoutes(answerOf([anns])) }, (p) => {
    p.page.openViewer(p.page.state.items[0]);
    byId(p.doc, "files-item-edit-open").click();
  });
  assert.deepEqual(admin.seen, { line: "Files cannot be dropped here.", reason: "Only Ann can add files to this item.", refused: true }, "only its uploader adds files: the desktop's words");
  assert.equal(text(byId(admin.p.doc, "files-edit-files-owner-only")), "Only Ann can add files to this item.", "said in the Edit form's Files part too");
  assert.equal(byId(admin.p.doc, "files-edit-add-file"), null, "where no Add a file is offered");
  assert.ok(!admin.p.net.calls.some((c) => c.url.startsWith("/api/upload")), "nothing uploaded");
  assert.equal(admin.p.page.state.form, null);
});

// ── 5 to 7. Shared Files ─────────────────────────────────────────────────

/** The Shared Files upload in a stand-in page, signed in as `over` says (session null: no identity). */
function sharedFiles(opts) {
  const o = opts || {};
  const doc = fakeDocument();
  const root = doc.createElement("div");
  root.id = "sf-upload";
  doc.body.appendChild(root);
  const win = fakeWindow();
  const net = fakeFetch([[at("/api/upload", "POST"), (c) => respond(200, { url: "/uploads/1760_" + c.init.body.get("file").name })]]);
  let reloaded = 0;
  const session = o.session === undefined ? fakeSession(o.over) : o.session;
  const up = SFU.createSharedFilesUpload({
    document: doc, root, window: win, session, fetch: net.fetch, FormData,
    clean: o.clean === undefined ? UC : o.clean, storage: o.storage || fakeStorage(),
    onUploaded: () => { reloaded++; },
  });
  up.start();
  return { doc, win, net, up, reloaded: () => reloaded };
}
/** Every upload made: its address (the token stood in) and the file it carried, name and bytes. */
async function uploadsOf(net) {
  const out = [];
  for (const c of net.calls.filter((x) => path0(x.url) === "/api/upload")) {
    const f = c.init.body.get("file");
    out.push({ url: c.url, name: f.name, bytes: new Uint8Array(await f.arrayBuffer()) });
  }
  return out;
}
const STL = new Uint8Array(Buffer.from("solid bracket\nfacet normal 0 0 1\nendsolid bracket\n"));
const stl = (name, bytes) => new File([bytes || STL], name, { type: "model/stl" });

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR: with takeFiles' line naming the files
// over the limit taken out: "the one over the limit is named" (actual ''); with the upload's address
// missing share=1: "... this session's token, and share=1" (actual "/api/upload?token=tok-1").
test("5. Shared Files: Drop to upload to Shared Files; sizes checked first, the rest uploaded as the button's picker does", async () => {
  const s = sharedFiles({ over: { uploadLimitBytes: MIB } });
  assert.equal(byId(s.doc, "sf-upload-btn").hidden, false, "Upload a file is offered");
  assert.equal(text(byId(s.doc, "sf-upload-btn")), "Upload a file", "in the desktop's words");
  s.win.fire("dragenter", filesDrag());
  assert.deepEqual(overlayOf(s.doc), { line: "Drop to upload to Shared Files", reason: "", refused: false });
  const big = stl("big.stl", new Uint8Array(2 * MIB));
  await dropOn(s.win, [stl("bracket.stl"), big]);
  await until(() => s.reloaded() > 0);
  assert.equal(overlayOf(s.doc), null);
  assert.equal(text(byId(s.doc, "sf-upload-notice")), "Not uploaded, over the 1 MB limit for you on this server: big.stl.", "the one over the limit is named");
  const sent = await uploadsOf(s.net);
  assert.deepEqual(sent.map((u) => [u.url, u.name]), [["/api/upload?token=tok-1&share=1", "bracket.stl"]], "the one over the limit is named, and never uploaded; this session's token, and share=1");
  assert.ok(Buffer.from(sent[0].bytes).equals(Buffer.from(STL)), "its own bytes (nothing hidden in it)");
  assert.equal(text(byId(s.doc, "sf-upload-status")), "Uploaded bracket.stl. It is in the list below.");
  assert.equal(s.reloaded(), 1, "and the list is loaded again");

  // The same two chosen with the button: the same line and the same upload.
  const p = sharedFiles({ over: { uploadLimitBytes: MIB } });
  const input = byId(p.doc, "sf-upload-input");
  assert.equal(input.multiple, true, "several at once");
  byId(p.doc, "sf-upload-btn").click();
  input.files = [stl("bracket.stl"), big];
  input.dispatch("change");
  await until(() => p.reloaded() > 0);
  assert.equal(text(byId(p.doc, "sf-upload-notice")), text(byId(s.doc, "sf-upload-notice")));
  assert.deepEqual((await uploadsOf(p.net)).map((u) => [u.url, u.name]), sent.map((u) => [u.url, u.name]), "the picker and the drop: the same path");
});

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR: with dropDecision answering take
// whenever the page is signed in (no role check): "a role that cannot upload: why not"; with render
// never hiding the button: "not offered without an identity here".
test("6. Shared Files where a drop would do nothing: the page and the overlay give the same reason, and nothing goes", async () => {
  const none = sharedFiles({ session: null });
  assert.equal(byId(none.doc, "sf-upload-btn").hidden, true, "not offered without an identity here");
  assert.equal(text(byId(none.doc, "sf-upload-why")), SFU.WORDS.noIdentity, "and the page says why");
  none.win.fire("dragenter", filesDrag());
  assert.deepEqual(overlayOf(none.doc), { line: "Files cannot be dropped here.", reason: SFU.WORDS.noIdentity, refused: true }, "the overlay: the same reason");
  await dropOn(none.win, [stl("bracket.stl")]);
  assert.equal(none.net.calls.length, 0, "and nothing goes");

  const cannot = sharedFiles({ over: { role: "unverified" } });
  assert.equal(byId(cannot.doc, "sf-upload-btn").hidden, true);
  assert.equal(text(byId(cannot.doc, "sf-upload-why")), SFU.WORDS.cannotUpload);
  cannot.win.fire("dragenter", filesDrag());
  assert.deepEqual(overlayOf(cannot.doc), { line: "Files cannot be dropped here.", reason: SFU.WORDS.cannotUpload, refused: true }, "a role that cannot upload: why not");
  await dropOn(cannot.win, [stl("bracket.stl")]);
  assert.equal(cannot.net.calls.length, 0, "nothing goes");

  const signingIn = sharedFiles({ over: { status: "connecting", uploadToken: "" } });
  signingIn.win.fire("dragenter", filesDrag());
  assert.deepEqual(overlayOf(signingIn.doc), { line: "Files cannot be dropped here.", reason: "Signing in to this server...", refused: true }, "still signing in");
});

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR: with one() sending the file as dropped
// (`send(file.name, file, false)`, no cleaning): "the cleaned photo goes, not the one dropped".
test("7. Shared Files cleans a file before it goes: a phone photo shows the step; Upload sends it cleaned, Cancel nothing", async () => {
  const photo = () => new File([B.phoneJpeg()], "IMG_0001.jpg", { type: "image/jpeg" });
  const cleaned = await UC.cleanFile(photo());
  assert.ok(cleaned.summary && cleaned.changed, "test setup: the photo holds where it was taken");
  const cleanBytes = new Uint8Array(await cleaned.cleaned.arrayBuffer());

  const s = sharedFiles();
  await dropOn(s.win, [photo()]);
  await until(() => byId(s.doc, "sf-upload-step-card"));
  const card = byId(s.doc, "sf-upload-step-card");
  assert.ok(card, "the step shows");
  assert.equal(text(card.querySelector(".sf-step-summary")), cleaned.summary, "what is hidden in it");
  assert.equal(byId(s.doc, "sf-upload-remove").checked, true, "the switch, on");
  assert.equal(s.net.calls.length, 0, "nothing goes before Upload");
  byId(s.doc, "sf-upload-go").click();
  await until(() => s.reloaded() > 0);
  const sent = await uploadsOf(s.net);
  assert.equal(sent.length, 1);
  assert.equal(sent[0].url, "/api/upload?token=tok-1&share=1", "no keep_metadata");
  assert.ok(Buffer.from(sent[0].bytes).equals(Buffer.from(cleanBytes)), "the cleaned photo goes, not the one dropped");
  assert.equal(byId(s.doc, "sf-upload-step-card"), null, "and the step is gone");

  const c = sharedFiles();
  await dropOn(c.win, [photo()]);
  await until(() => byId(c.doc, "sf-upload-step-card"));
  byId(c.doc, "sf-upload-cancel").click();
  for (let i = 0; i < 4; i++) await settle();
  assert.equal(c.net.calls.length, 0, "Cancel: nothing goes");
  assert.equal(byId(c.doc, "sf-upload-step-card"), null);
});

// ── 8. The pages load it ─────────────────────────────────────────────────

// Seen red 2026-10-17 in a copy of web/ through HOS_WEB_DIR: with files.html's drop-overlay.js
// line taken out: "pages/files.html loads /shared/drop-overlay.js before its page script"; with a
// hand-written colour in drop-overlay.css (`color: #fff`): "theme variables only".
test("8. every page that takes files loads the overlay before its own script; the stylesheet uses the theme only", () => {
  const page = (rel) => fs.readFileSync(path.join(WEB, rel), "utf8");
  for (const [rel, own] of [["chat/index.html", "/chat/chat-messages.js"], ["pages/files.html", "/pages/files-app.js"]]) {
    const html = page(rel);
    assert.ok(/<link rel="stylesheet" href="\/shared\/drop-overlay\.css\?v=\d+">/.test(html), rel + " links /shared/drop-overlay.css");
    const at1 = html.search(/<script src="\/shared\/drop-overlay\.js\?v=\d+"><\/script>/);
    const at2 = html.indexOf('src="' + own);
    assert.ok(at1 > 0 && at2 > at1, rel + " loads /shared/drop-overlay.js before its page script");
  }
  // Shared Files is the Files page's "Shared files" place since 2026-10-19 (its own page until
  // then): the Files page loads its upload's scripts in order, before the place and the page.
  const shared = page("pages/files.html");
  const order = ["/shared/relay-session.js", "/shared/strip-metadata.js", "/shared/upload-clean.js", "/shared/drop-overlay.js", "/pages/shared-files-upload.js", "/pages/files-shared.js", "/pages/files-app.js"].map((s) => shared.indexOf('src="' + s));
  assert.ok(order.every((i) => i > 0), "the Files page loads Shared files' upload scripts: " + order);
  assert.deepEqual(order.slice().sort((a, b) => a - b), order, "in order");
  assert.ok(!fs.existsSync(path.join(WEB, "pages", "shared-files.html")), "and the Shared Files page that was is gone");

  const css = page("shared/drop-overlay.css");
  const values = css.replace(/\/\*[\s\S]*?\*\//g, "");
  assert.ok(!/#[0-9a-f]{3,8}\b|rgba?\(|hsla?\(/i.test(values), "theme variables only");
  assert.ok(/\.hos-drop-overlay\s*\{[^}]*background:\s*var\(--bg-modal\)/.test(values), "dimmed as the dialogs are (var(--bg-modal))");
  assert.ok(/pointer-events:\s*none/.test(values), "it never takes the pointer from the page");
});
