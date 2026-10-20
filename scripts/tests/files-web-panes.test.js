// The Media page on the website in three panes (2026-10-19), mirroring the desktop's of v0.1521.0:
// src/gui/pages/files/panes.rs, list.rs, viewer.rs, item_view.rs (draw_details) and edit.rs
// (draw_in_place), src/gui/widgets/panes.rs, sound_view.rs and waveform.rs; docs/design/
// files-showcase.md "The three panes". The operator, 2026-10-19: "a left side card that is the list
// of all the media and right side is like viewing the in-depth details of one media item. Maybe into
// thirds? ... When clicking edit, instead of popping up another window, just change the details panel
// on the right to accept edits. Once done clicking save undoes the GUI changes for edit mode. ...
// when we switch to edit mode, add the RGB boxed areas so edit mode stands out."
//
// Run: node --test scripts/tests/files-web-panes.test.js
//
// Against the stand-ins in scripts/tests/files-web-harness.js (a document that holds only what the
// page built, where reading or setting innerHTML throws; a network that records every request;
// signing that returns "sig:" plus the words signed), plus this file's own: a window whose size the
// test sets and whose events it fires, a history that records what the page asks of it, and a
// decoder that reads a WAV file the way the browser's decodeAudioData answers (Node has no Web Audio).
// HOS_WEB_DIR points the test at another copy of web/ (used to see each test red).
//
// What it proves:
//  1. Which way the panes go follows the page's shape, as the desktop's arrange_for: side by side on
//     a wide window, one above another on a tall or narrow one (a phone), decided again on a resize;
//     each pane never below its narrowest (280 px across, 170 px stacked); the list's parts are in
//     the left pane as rows, each one button with no buttons of its own.
//  2. Nothing is chosen at first (no view counted); a row's press shows that item in the middle pane
//     (fitted) and its details on the right, marks the row, and counts one view; another row moves
//     all three; the same row again counts nothing more; Close puts the two panes' lines back. From
//     the Gallery, a tile's press shows the item in the panes, and Close brings the Gallery back.
//  3. Edit turns the right pane into the form in place, edged in the moving colours: Cancel with
//     nothing changed goes straight back; with changes it asks (Keep editing keeps what was typed,
//     Discard the changes sends nothing); another row, Close, Back and the browser's own leaving ask
//     too; Escape keeps editing; Save sends the edit and the pane is the details again.
//  4. A sound's waveform is its own: the peaks of the fixture tests/fixtures/media/two-tones-440-
//     then-2000.wav (one second at 0.9 of full scale, then one at 0.25), as the desktop's test reads
//     it; on the page nothing of the sound is fetched before Play here, then its own address once
//     (never the counting route), read once.
//  5. A press on the waveform goes where it is: its left edge the start, its right the end, a quarter
//     of the way along a quarter of the time; on the page, the press moves the <audio>'s time.
//  6. The split survives a reload: a drag on a handle moves one boundary (never past a pane's
//     narrowest), is kept in this browser when it ends, and the next page opens with it; a
//     double-click puts the two panes back; side by side and stacked are kept apart; storage that
//     refuses changes nothing else.
//
// Seen red 2026-10-19: each with one break made in a copy of web/ under HOS_WEB_DIR, then green
// again on the real web/: see the note above each test.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const H = require("./files-web-harness.js");
const { ROOT, WEB, ME, ANN, byId, text, respond, at, path0, fakeStorage, settle, item, listAnswer, listRoutes, openPage } = H;
const MP = require(path.join(WEB, "pages", "files-panes.js"));
const MS = require(path.join(WEB, "pages", "files-sound.js"));
const MI = require(path.join(WEB, "pages", "files-item.js"));
const FRISK = require(path.join(WEB, "shared", "file-risk.js"));
const RISK_LIST = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "file_risks.json"), "utf8"));
const WAV = fs.readFileSync(path.join(ROOT, "tests", "fixtures", "media", "two-tones-440-then-2000.wav"));

/** A window the test sizes and whose events it fires (resize, beforeunload). */
function fakeWindow(width, height) {
  const listeners = {};
  return {
    innerWidth: width, innerHeight: height, listeners,
    addEventListener(t, f) { (listeners[t] = listeners[t] || []).push(f); },
    removeEventListener(t, f) { listeners[t] = (listeners[t] || []).filter((x) => x !== f); },
    /** Fire `type`; the event, to read what the page did with it. */
    fire(type) {
      const ev = { type, defaultPrevented: false, returnValue: undefined, preventDefault() { this.defaultPrevented = true; } };
      for (const f of (listeners[type] || []).slice()) f(ev);
      return ev;
    },
  };
}
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

/** A 16-bit PCM WAV file read as the browser's decodeAudioData answers it: one array per channel, -1 to 1. */
function readWav(bytes) {
  const b = Buffer.from(bytes);
  let fmt = null;
  let data = null;
  for (let off = 12; off + 8 <= b.length;) {
    const id = b.toString("ascii", off, off + 4);
    const len = b.readUInt32LE(off + 4);
    if (id === "fmt ") fmt = { channels: b.readUInt16LE(off + 10), rate: b.readUInt32LE(off + 12), bits: b.readUInt16LE(off + 22) };
    if (id === "data") data = b.subarray(off + 8, off + 8 + len);
    off += 8 + len + (len % 2);
  }
  assert.ok(fmt && data && fmt.bits === 16, "test setup: a 16-bit PCM WAV");
  const n = Math.floor(data.length / 2 / fmt.channels);
  const channels = [];
  for (let c = 0; c < fmt.channels; c++) {
    const ch = new Float32Array(n);
    for (let i = 0; i < n; i++) ch[i] = data.readInt16LE((i * fmt.channels + c) * 2) / 32768;
    channels.push(ch);
  }
  return { channels, sampleRate: fmt.rate, duration: n / fmt.rate };
}

const rows = (doc) => byId(doc, "files-grid").querySelectorAll(".files-row");
const rowOf = (doc, id) => rows(doc).find((r) => r.dataset.id === String(id));
const tileOf = (doc, id) => byId(doc, "files-grid").querySelectorAll(".files-tile").find((t) => t.dataset.id === String(id));
const views = (net) => net.calls.filter((c) => c.url === "/api/files/viewed").map((c) => c.body.id);
const within = (el, child) => { for (let n = child; n; n = n.parentNode) if (n === el) return true; return false; };
const typeInto = (el, v) => { el.value = v; el.dispatch("input"); };

const DUSK = () => item({ id: 1, title: "Dusk over the homestead", url: "/uploads/1_dusk.png", filename: "1_dusk.png", uploader_key: ME, uploader_name: "Me", description: "From the ridge." });
const SHIP = () => item({
  id: 2, title: "Fibonacci homestead", kind: "model", url: "/uploads/2_ship.glb", filename: "2_ship.glb", original_name: "ship.glb", size_bytes: 2224,
  files: [{ file: 0, filename: "2_ship.glb", original_name: "ship.glb", kind: "model", size_bytes: 2224, url: "/uploads/2_ship.glb" }],
});
const DRUMS = () => item({
  id: 5, title: "Fire circle drums", kind: "file", url: "/uploads/5_drums.wav", filename: "5_drums.wav", original_name: "drums.wav", size_bytes: WAV.length,
  files: [{ file: 0, filename: "5_drums.wav", original_name: "drums.wav", kind: "file", size_bytes: WAV.length, url: "/uploads/5_drums.wav" }],
});

// ── 1. Which way the panes go ────────────────────────────────────────────

// Seen red 2026-10-19 in a copy of web/ through HOS_WEB_DIR: with arrangeFor deciding by the fit
// alone (its `w >= h` taken out): "a tall window stacks them" (actual 'across', expected 'down'); with
// files-app.js's resize listener left unwired (`win.addEventListener('resize', resized)` taken out):
// "a resize to a phone stacks them" (actual 'across', expected 'down').
test("1. the panes go side by side on a wide page and one above another on a tall or narrow one, decided again on a resize; the list is rows", async () => {
  // The desktop's own cases (widgets/panes.rs the_way_the_panes_go_follows_the_areas_shape), at the web's gap.
  const g = MP.GAP;
  assert.equal(MP.arrangeFor(2560, 1300, 3, 280, g), "across", "his monitor");
  assert.equal(MP.arrangeFor(1280, 900, 3, 280, g), "across", "a laptop");
  assert.equal(MP.arrangeFor(900, 1400, 3, 280, g), "down", "a tall window stacks them");
  assert.equal(MP.arrangeFor(800, 600, 3, 280, g), "down", "too narrow for three side by side");
  assert.equal(MP.arrangeFor(375, 812, 3, 280, g), "down", "a phone");
  assert.equal(MP.arrangeFor(1000, 1000, 3, 280, g), "across", "square, and they fit");

  const win = fakeWindow(1600, 900);
  const { doc } = await openPage({ view: "panes", window: win, routes: listRoutes(listAnswer([DUSK(), SHIP()])) });
  const panes = byId(doc, "files-panes");
  assert.ok(panes && panes.hidden === false, "the panes, with nothing kept in this browser");
  assert.equal(panes.dataset.arrange, "across", "side by side on a wide page");
  const across = panes.style.gridTemplateColumns;
  assert.equal((across.match(/minmax\(280px, /g) || []).length, 3, "three panes, none narrower than 280 px: " + across);
  assert.ok(across.includes("28fr") && across.includes("42fr") && across.includes("30fr"), "the desktop's first split, 28 / 42 / 30: " + across);
  // The list's parts are in the left pane, as rows; each row is one button with nothing to press inside it.
  const list = byId(doc, "files-list-pane");
  for (const id of ["files-filters", "files-grid", "files-sentinel", "files-foot"]) assert.ok(within(list, byId(doc, id)), "#" + id + " is in the left pane");
  assert.equal(rows(doc).length, 2, "a row for each item");
  assert.equal(byId(doc, "files-grid").querySelectorAll(".files-tile").length, 0, "no tiles");
  for (const r of rows(doc)) {
    assert.equal(r.tagName, "BUTTON", "a row is one button");
    assert.equal(r.querySelectorAll("button").length + r.querySelectorAll("select").length, 0, "with no buttons of its own (they are on the right)");
  }
  assert.equal(text(rowOf(doc, 2).querySelector(".files-row-by")), "by Ann in Fire flow arts");
  assert.equal(text(rowOf(doc, 2).querySelector(".files-row-facts")), "3D model, 0 picks, 0 downloads, 2 KB", "its kind, counts and size, as the desktop's row");

  win.innerWidth = 375;
  win.innerHeight = 812;
  win.fire("resize");
  assert.equal(panes.dataset.arrange, "down", "a resize to a phone stacks them");
  assert.equal((panes.style.gridTemplateRows.match(/minmax\(170px, /g) || []).length, 3, "each at least 170 px tall: " + panes.style.gridTemplateRows);
  assert.equal(panes.style.gridTemplateColumns, "minmax(0, 1fr)", "as wide as the page");
  for (const hd of panes.querySelectorAll("[data-handle]")) assert.equal(hd.getAttribute("aria-orientation"), "horizontal", "its handles lie across");
  win.innerWidth = 1600;
  win.innerHeight = 900;
  win.fire("resize");
  assert.equal(panes.dataset.arrange, "across", "and back");
});

// ── 2. The row pressed is the item shown ─────────────────────────────────

// Seen red 2026-10-19 in a copy of web/ through HOS_WEB_DIR: with files-item.js open() not telling
// the page the item changed (its `c.selected()` call taken out): "the pressed row is marked" (actual
// 'files-row', expected 'files-row files-row-chosen'); with files-app.js viewNow() ignoring the item
// shown (`st.layout.gallery` alone): "a tile's press shows the item in the panes" (actual true: the
// panes stayed hidden).
test("2. a row's press shows that item in the middle and its details on the right, marks the row and counts one view; Close puts the panes' lines back", async () => {
  const { doc, net } = await openPage({ view: "panes", routes: listRoutes(listAnswer([DUSK(), SHIP()])) });
  assert.ok(byId(doc, "files-choose") && byId(doc, "files-details-empty"), "nothing chosen: the two panes say what they will hold");
  assert.equal(text(byId(doc, "files-choose")), MP.WORDS.choose);
  assert.equal(text(byId(doc, "files-details-empty")), MP.WORDS.details);
  assert.equal(byId(doc, "files-viewer-open"), null);
  assert.deepEqual(views(net), [], "no item chosen for the person, no view counted");

  rowOf(doc, 1).click();
  await settle();
  const face = byId(doc, "files-face");
  const details = byId(doc, "files-details");
  assert.ok(within(byId(doc, "files-panes"), face) && within(byId(doc, "files-panes"), details), "both in the page's panes");
  const img = face.querySelector("img");
  assert.deepEqual([img && img.src, img && img.className], ["/uploads/1_dusk.png", "files-viewer-img files-face-fit"], "the picture, fitted to the middle pane");
  assert.equal(byId(doc, "files-view-full").href, "/uploads/1_dusk.png", "with View full size");
  assert.equal(text(details.querySelector("h2")), "Dusk over the homestead", "its details on the right");
  assert.equal(text(byId(doc, "files-item-description")), "From the ridge.");
  assert.ok(byId(doc, "files-item-files"), "with its files");
  assert.equal(rowOf(doc, 1).className, "files-row files-row-chosen", "the pressed row is marked");
  assert.equal(rowOf(doc, 1).getAttribute("aria-current"), "true");
  assert.deepEqual(views(net), [1], "one view counted");
  assert.equal(byId(doc, "files-choose"), null, "the lines are gone");

  rowOf(doc, 2).click();
  await settle();
  assert.equal(text(byId(doc, "files-details").querySelector("h2")), "Fibonacci homestead", "another row moves the details");
  assert.equal(text(byId(doc, "files-face").querySelector(".files-face-kind")), "3D model", "and the middle: a model as what it is");
  assert.ok(byId(doc, "files-face-view3d") && byId(doc, "files-face-download"), "with View in 3D and Download");
  assert.ok(!net.calls.some((c) => path0(c.url) === "/uploads/2_ship.glb"), "nothing of the model fetched before View in 3D");
  assert.deepEqual([rowOf(doc, 1).className, rowOf(doc, 2).className], ["files-row", "files-row files-row-chosen"], "the mark follows");
  assert.deepEqual(views(net), [1, 2]);
  rowOf(doc, 2).click();
  await settle();
  assert.deepEqual(views(net), [1, 2], "the same row again counts nothing more");

  byId(doc, "files-viewer-close").click();
  assert.equal(byId(doc, "files-viewer-open"), null, "Close: nothing shown");
  assert.ok(byId(doc, "files-choose") && byId(doc, "files-details-empty"), "the panes' lines are back");
  assert.equal(rows(doc).filter((r) => r.className.includes("chosen")).length, 0, "no row marked");

  // From the Gallery (the desktop's other view): a tile's press shows the item in the panes; Close
  // brings the Gallery back.
  const g = await openPage({ routes: listRoutes(listAnswer([DUSK(), SHIP()])) });
  assert.equal(byId(g.doc, "files-panes").hidden, true, "test setup: the Gallery");
  assert.ok(tileOf(g.doc, 2), "its tiles");
  assert.equal(byId(g.doc, "files-view-gallery").getAttribute("aria-pressed"), "true", "the switch says so");
  tileOf(g.doc, 2).querySelector(".files-thumb").click();
  await settle();
  assert.equal(byId(g.doc, "files-panes").hidden, false, "a tile's press shows the item in the panes");
  assert.equal(rowOf(g.doc, 2).className, "files-row files-row-chosen", "its row marked in the list");
  byId(g.doc, "files-viewer-close").click();
  assert.equal(byId(g.doc, "files-panes").hidden, true, "Close: the Gallery again");
  assert.ok(tileOf(g.doc, 1) && tileOf(g.doc, 2), "with its tiles");
  byId(g.doc, "files-view-panes").click();
  assert.equal(byId(g.doc, "files-panes").hidden, false, "Panes: the panes");
  assert.equal(JSON.parse(g.storage.m.get(MP.LAYOUT_KEY)).gallery, false, "and kept in this browser");
});

// ── 3. Edit in place ─────────────────────────────────────────────────────

// Seen red 2026-10-19 in a copy of web/ through HOS_WEB_DIR: with files-item.js cancelEdit closing
// the form whatever it held (`unsaved()` taken out of it): "with changes, Cancel asks first" (actual
// null); with open() not asking (its unsaved() check made `false`): "another row asks first" (actual
// null: the typed title was dropped); with files-app.js leaving() answering nothing: "the browser
// asks before the page is left" (actual false); with editMode() leaving the pane's class alone: "the
// pane is edged in the moving colours" (actual 'files-pane files-details-pane').
test("3. Edit turns the right pane into the form; Cancel goes straight back or asks; leaving asks; Save returns to the details", async () => {
  const win = fakeWindow(1600, 900);
  const history = fakeHistory();
  const { doc, net, page } = await openPage({ view: "panes", window: win, history, keys: true, routes: listRoutes(listAnswer([DUSK(), SHIP()]), [
    [at("/api/files/edit", "POST"), (c) => respond(200, { ok: true, item: Object.assign(DUSK(), { title: c.body.edit.title }) })],
  ]) });
  rowOf(doc, 1).click();
  await settle();
  const edit = () => byId(doc, "files-item-edit-open");
  const asked = () => byId(doc, "files-discard");
  edit().click();
  const pane = byId(doc, "files-details");
  assert.ok(byId(doc, "files-edit") && within(pane, byId(doc, "files-edit")), "the form in place of the details, in the right pane");
  assert.equal(pane.className, "files-pane files-details-pane files-pane-editing", "the pane is edged in the moving colours");
  assert.equal(byId(doc, "files-item-info").hidden, true, "the details step aside");
  assert.equal(text(byId(doc, "files-edit").querySelector("h3")), 'Editing "Dusk over the homestead"', "headed as the desktop's");
  assert.deepEqual(byId(doc, "files-edit").children.filter((c) => c.className.startsWith("tint-section")).map((c) => c.dataset.tint), ["red", "green", "blue"], "its three tinted parts");
  const foot = byId(doc, "files-edit").querySelector(".files-edit-foot");
  assert.ok(foot && within(foot, byId(doc, "files-edit-save")) && within(foot, byId(doc, "files-edit-cancel")), "Save and Cancel in the foot that stays in view");
  assert.equal(byId(doc, "files-face").querySelector("img").src, "/uploads/1_dusk.png", "the picture stays in the middle");

  // Cancel with nothing changed: straight back.
  byId(doc, "files-edit-cancel").click();
  assert.equal(asked(), null, "nothing changed: nothing asked");
  assert.equal(byId(doc, "files-edit"), null, "the details again");
  assert.equal(byId(doc, "files-item-info").hidden, false);
  assert.equal(byId(doc, "files-details").className, "files-pane files-details-pane", "the edge goes with it");

  // With changes, Cancel asks; Keep editing keeps what was typed; Discard sends nothing.
  edit().click();
  typeInto(byId(doc, "files-edit-title"), "Dusk, again");
  byId(doc, "files-edit-cancel").click();
  assert.ok(asked(), "with changes, Cancel asks first");
  assert.equal(text(byId(doc, "files-discard-words")), MI.discardWords("Dusk over the homestead"));
  assert.equal(text(byId(doc, "files-discard-words")), 'Your changes to "Dusk over the homestead" are not saved.', "in the desktop's words");
  byId(doc, "files-discard-keep").click();
  assert.equal(asked(), null);
  assert.equal(byId(doc, "files-edit-title").value, "Dusk, again", "Keep editing: what was typed is there");
  doc.key("Escape");
  assert.ok(byId(doc, "files-edit"), "Escape with nothing asked: it would close the item, so it asks");
  assert.ok(asked(), "asked");
  doc.key("Escape");
  assert.equal(asked(), null, "Escape in the question keeps editing");
  assert.ok(byId(doc, "files-viewer-open") && byId(doc, "files-edit"), "the item and its form stay");

  // The browser's own leaving asks while there are changes.
  assert.equal(win.fire("beforeunload").defaultPrevented, true, "the browser asks before the page is left");

  // Another row asks first; kept, nothing moved; discarded, it moves (and only then is its view counted).
  rowOf(doc, 2).click();
  assert.ok(asked(), "another row asks first");
  byId(doc, "files-discard-keep").click();
  assert.equal(text(byId(doc, "files-details").querySelector("h2")), "Dusk over the homestead", "kept: the same item");
  assert.deepEqual(views(net), [1], "and no view counted for the other");
  // Back while editing: asked; kept, the item's address is put back.
  const before = history.calls.length;
  page.route("/files", "");
  assert.ok(asked(), "Back asks first");
  byId(doc, "files-discard-keep").click();
  assert.deepEqual(history.calls.slice(before), [["push", "/files/1"]], "kept: its address is put back");
  assert.ok(byId(doc, "files-viewer-open"), "and it stays open");
  rowOf(doc, 2).click();
  byId(doc, "files-discard-yes").click();
  await settle();
  assert.equal(text(byId(doc, "files-details").querySelector("h2")), "Fibonacci homestead", "discarded: the other item");
  assert.deepEqual(views(net), [1, 2]);
  assert.ok(!net.calls.some((c) => c.url === "/api/files/edit"), "nothing was sent");
  assert.equal(win.fire("beforeunload").defaultPrevented, false, "nothing unsaved: the browser is not asked to");

  // Save: the edit goes, and the pane is the details again.
  rowOf(doc, 1).click();
  await settle();
  edit().click();
  typeInto(byId(doc, "files-edit-title"), "Dusk, saved");
  byId(doc, "files-edit-save").click();
  await settle();
  const sent = net.calls.filter((c) => c.url === "/api/files/edit");
  assert.equal(sent.length, 1, "Save sends the edit");
  assert.equal(sent[0].body.edit.title, "Dusk, saved");
  assert.equal(byId(doc, "files-edit"), null, "Save returns the pane to the details");
  assert.equal(byId(doc, "files-details").className, "files-pane files-details-pane", "without the edge");
  assert.equal(text(byId(doc, "files-details").querySelector("h2")), "Dusk, saved", "showing the item as saved");
  assert.equal(text(byId(doc, "files-item-status")), MI.WORDS.saved);

  // Close with changes asks; discarded, it closes.
  edit().click();
  typeInto(byId(doc, "files-edit-description"), "Changed my mind");
  byId(doc, "files-viewer-close").click();
  assert.ok(asked() && byId(doc, "files-viewer-open"), "Close asks first");
  byId(doc, "files-discard-yes").click();
  assert.equal(byId(doc, "files-viewer-open"), null, "discarded: closed");
});

// ── 4. A sound's waveform is its own ─────────────────────────────────────

// Seen red 2026-10-19 in a copy of web/ through HOS_WEB_DIR: with files-sound.js peaksOf keeping each
// stretch's last sample alone (`lo = s; hi = s;` in place of the lowest and highest): "the first
// second peaks near 0.9: 0.0847 to 0.8960" (the desktop's own red was the same break); with the view
// started at once (files-item.js soundFor calling `view.start()`): "nothing of the sound fetched
// before Play here" (actual 1, expected 0).
test("4. a sound's waveform is the peaks of the sound itself; nothing of it is fetched before Play here, then its own address once", async () => {
  const wav = readWav(WAV);
  assert.equal(wav.sampleRate, 8000, "test setup: the desktop's fixture, 8000 samples a second");
  assert.ok(Math.abs(wav.duration - 2) < 0.01, "two seconds");
  const peaks = MS.peaksOf(wav.channels, MS.BUCKETS);
  assert.equal(peaks.length, 1024, "1024 stretches, as the desktop keeps");
  const n = peaks.length;
  const loud = (p) => p.reduce((m, [lo, hi]) => Math.max(m, Math.abs(lo), Math.abs(hi)), 0);
  const quiet = (p) => p.reduce((m, [lo, hi]) => Math.min(m, Math.max(Math.abs(lo), Math.abs(hi))), 1);
  const first = peaks.slice(0, n / 2 - 2);
  const second = peaks.slice(n / 2 + 2);
  assert.ok(Math.abs(loud(first) - 0.9) < 0.02 && quiet(first) > 0.8, "the first second peaks near 0.9: " + quiet(first).toFixed(4) + " to " + loud(first).toFixed(4));
  assert.ok(Math.abs(loud(second) - 0.25) < 0.02 && quiet(second) > 0.2, "the second near 0.25: " + quiet(second).toFixed(4) + " to " + loud(second).toFixed(4));
  assert.deepEqual(MS.peaksOf([new Float32Array([0.5, NaN, 2, -3])], 2), [[0, 0.5], [-1, 1]], "silence for what is not a number; full scale at most");
  assert.equal(MS.bandEdges(8000).length, MS.BANDS + 1, "24 bands");

  FRISK.use(RISK_LIST); // the file-type list in: drums.wav is a sound
  const decoded = [];
  const { doc, net, page } = await openPage({
    view: "panes", decodeAudio: async (bytes) => { decoded.push(bytes.byteLength); return readWav(bytes); },
    routes: listRoutes(listAnswer([DRUMS()]), [[at("/uploads/5_drums.wav"), () => respond(200, "", new Uint8Array(WAV))]]),
  });
  rowOf(doc, 5).click();
  await settle();
  const own = () => net.calls.filter((c) => path0(c.url) === "/uploads/5_drums.wav");
  assert.equal(own().length, 0, "nothing of the sound fetched before Play here");
  assert.ok(byId(doc, "files-sound-play"), "Play here in the middle pane");
  assert.equal(text(byId(doc, "files-sound-line")), "drums.wav, 31 KB", "what it is and how long a fetch it is");
  byId(doc, "files-sound-play").click();
  for (let i = 0; i < 4; i++) await settle();
  assert.equal(own().length, 1, "Play here: its own address, once");
  assert.ok(!net.calls.some((c) => c.url.startsWith("/api/files/download")), "never the counting route: listening here is not a download");
  assert.deepEqual(decoded, [WAV.length], "read once, the whole file");
  const view = page.itemView.page.sound.view;
  assert.equal(view.state.phase, "ready", view.state.problem);
  assert.deepEqual(view.state.peaks, peaks, "the waveform drawn is the sound's own peaks");
  assert.ok(byId(doc, "files-sound-wave") && byId(doc, "files-sound-audio"), "its waveform over the browser's own player");
  assert.ok(/^blob:test\//.test(byId(doc, "files-sound-audio").src), "played from the bytes already fetched");
  // The page drawn again (the item answered afresh, the list reloaded) fetches and reads nothing more.
  page.itemView.renderInfo();
  page.itemView.takeItem(DRUMS());
  await settle();
  assert.equal(own().length, 1, "fetched once");
  assert.deepEqual(decoded, [WAV.length], "read once");
  assert.equal(byId(doc, "files-sound"), view.el, "the same view, playing on");
  assert.deepEqual(views(net), [5], "one view, from the row's press");
});

// ── 5. A press on the waveform goes where it is ──────────────────────────

// Seen red 2026-10-19 in a copy of web/ through HOS_WEB_DIR: with seekFraction leaving the waveform's
// left edge out (`x / width`): "its left edge is the start" (actual 0.5, expected 0); with
// files-sound.js seekTo measuring from the canvas's edge instead of the waveform's (its 6 px pad taken
// out): "a quarter of the way along is a quarter of the time" (actual 0.5145631067961165, expected 0.5).
test("5. a press on the waveform maps to where along it the press was, and moves the sound there", async () => {
  assert.equal(MS.seekFraction(200, 400, 200), 0, "its left edge is the start");
  assert.equal(MS.seekFraction(200, 400, 600), 1, "its right edge is the end");
  assert.equal(MS.seekFraction(200, 400, 300), 0.25, "a quarter of the way along");
  assert.equal(MS.seekFraction(200, 400, 10), 0, "left of it, the start");
  assert.equal(MS.seekFraction(200, 400, 900), 1, "right of it, the end");
  assert.equal(MS.seekFraction(200, 0, 300), 0, "a waveform with no width: the start");

  FRISK.use(RISK_LIST);
  const { doc } = await openPage({
    view: "panes", decodeAudio: async (bytes) => readWav(bytes),
    routes: listRoutes(listAnswer([DRUMS()]), [[at("/uploads/5_drums.wav"), () => respond(200, "", new Uint8Array(WAV))]]),
  });
  rowOf(doc, 5).click();
  await settle();
  byId(doc, "files-sound-play").click();
  for (let i = 0; i < 4; i++) await settle();
  const canvas = byId(doc, "files-sound-wave");
  const audio = byId(doc, "files-sound-audio");
  // The canvas 412 px wide from x = 100; the waveform inside it from 106 to 506 (its 6 px edge).
  canvas.getBoundingClientRect = () => ({ left: 100, top: 0, width: 412, height: 200 });
  audio.duration = 2;
  canvas.dispatch("pointerdown", { clientX: 206, pointerId: 1 });
  assert.equal(audio.currentTime, 0.5, "a quarter of the way along is a quarter of the time");
  canvas.dispatch("pointermove", { clientX: 506, pointerId: 1 });
  assert.equal(audio.currentTime, 2, "a drag along it goes with it, to the end");
  canvas.dispatch("pointerup", { pointerId: 1 });
  canvas.dispatch("pointermove", { clientX: 106, pointerId: 1 });
  assert.equal(audio.currentTime, 2, "let go, a move does nothing");
});

// ── 6. The split survives a reload ───────────────────────────────────────

// Seen red 2026-10-19 in a copy of web/ through HOS_WEB_DIR: with files-panes.js drag unclamped
// (`a = out[i] * total + delta`): "never below the narrowest: 0.9,-0.2,0.3" (the middle pane below
// nothing, the desktop's own red); with files-app.js setShares never saving (its `if (settled)
// MP.saveLayout(...)` taken out): "kept in this browser once the drag ends: null"; with readLayout
// answering the defaults whatever was kept: "the next page opens with it" ('minmax(280px, 28fr) ...'
// where '34.33fr' was kept).
test("6. a drag on a handle moves the split, is kept when it ends, and the next page opens with it; a double-click puts it back", async () => {
  // The desktop's own drag cases (widgets/panes.rs a_drag_moves_one_boundary_within_the_narrowest).
  const s = MP.drag([0.3, 0.4, 0.3], 0, 100, 1000, 200);
  assert.ok(Math.abs(s[0] - 0.4) < 1e-9 && Math.abs(s[1] - 0.3) < 1e-9 && Math.abs(s[2] - 0.3) < 1e-9, String(s));
  const far = MP.drag(s, 0, 500, 1000, 200);
  assert.ok(Math.abs(far[1] * 1000 - 200) < 1e-6, "never below the narrowest: " + far);
  assert.deepEqual(MP.checked([NaN, 1, 1], [0.3, 0.4, 0.3]), [0.3, 0.4, 0.3], "a broken split is the default");
  assert.ok(Math.abs(MP.checked([2, 2, 4], [0.3, 0.4, 0.3])[2] - 0.5) < 1e-9, "a usable one is made to sum to one");

  const storage = fakeStorage();
  const win = fakeWindow(1600, 900); // the panes' own length: 1600 less two 10 px handles, 1580
  const first = await openPage({ view: "panes", window: win, storage, routes: listRoutes(listAnswer([DUSK()])) });
  const handle = byId(first.doc, "files-panes").querySelector('[data-handle="0"]');
  assert.ok(handle && handle.getAttribute("role") === "separator", "a handle between the list and the item");
  handle.dispatch("pointerdown", { clientX: 500, pointerId: 1, button: 0 });
  handle.dispatch("pointermove", { clientX: 600, pointerId: 1 });
  const moved = byId(first.doc, "files-panes").style.gridTemplateColumns;
  assert.ok(moved.includes("34.33fr"), "the list grows by the 100 px dragged: " + moved);
  assert.equal(storage.m.has(MP.LAYOUT_KEY), false, "not kept while it is held");
  handle.dispatch("pointerup", { pointerId: 1 });
  const kept = JSON.parse(storage.m.get(MP.LAYOUT_KEY) || "null");
  assert.ok(kept && Math.abs(kept.across[0] - (0.28 + 100 / 1580)) < 1e-9, "kept in this browser once the drag ends: " + JSON.stringify(kept));
  assert.ok(Math.abs(kept.across[1] - (0.42 - 100 / 1580)) < 1e-9 && kept.across[2] === 0.3, "the item's pane gave it; the details kept theirs");
  assert.deepEqual(kept.down, MP.DOWN.slice(), "stacked is kept apart, untouched");
  handle.dispatch("pointerdown", { clientX: 600, pointerId: 2, button: 0 });
  handle.dispatch("pointermove", { clientX: 2600, pointerId: 2 });
  handle.dispatch("pointerup", { pointerId: 2 });
  const squeezed = JSON.parse(storage.m.get(MP.LAYOUT_KEY)).across;
  assert.ok(squeezed[1] * 1580 >= MP.MIN_PANE - 1e-6, "never past the item pane's narrowest: " + squeezed[1] * 1580);

  // Back to the 100 px drag, then the next page.
  handle.dispatch("dblclick");
  const reset = JSON.parse(storage.m.get(MP.LAYOUT_KEY)).across;
  assert.ok(Math.abs(reset[0] / (reset[0] + reset[1]) - 0.28 / 0.70) < 1e-9, "a double-click puts the two back in the desktop's proportion: " + reset);
  handle.dispatch("pointerdown", { clientX: 500, pointerId: 3, button: 0 });
  handle.dispatch("pointermove", { clientX: 600, pointerId: 3 });
  handle.dispatch("pointerup", { pointerId: 3 });
  const before = byId(first.doc, "files-panes").style.gridTemplateColumns;
  const next = await openPage({ view: "panes", window: fakeWindow(1600, 900), storage, routes: listRoutes(listAnswer([DUSK()])) });
  assert.equal(byId(next.doc, "files-panes").style.gridTemplateColumns, before, "the next page opens with it");
  assert.equal(byId(next.doc, "files-panes").hidden, false, "in the panes");
  // Stacked keeps its own split: a phone opens with the desktop's stacked one.
  const phone = await openPage({ view: "panes", window: fakeWindow(375, 812), storage, routes: listRoutes(listAnswer([DUSK()])) });
  assert.equal(byId(phone.doc, "files-panes").style.gridTemplateRows, MP.template(MP.DOWN.slice(), MP.MIN_PANE_DOWN), "stacked: its own split");

  // Storage that refuses (a private window): the defaults, a drag lasts the visit, nothing thrown.
  const refused = await openPage({ view: "panes", window: fakeWindow(1600, 900), storage: fakeStorage(true), routes: listRoutes(listAnswer([DUSK()])) });
  const rh = byId(refused.doc, "files-panes").querySelector('[data-handle="0"]');
  assert.ok(byId(refused.doc, "files-panes").style.gridTemplateColumns.includes("28fr"), "the default split");
  rh.dispatch("pointerdown", { clientX: 500, pointerId: 1, button: 0 });
  rh.dispatch("pointermove", { clientX: 600, pointerId: 1 });
  rh.dispatch("pointerup", { pointerId: 1 });
  assert.ok(byId(refused.doc, "files-panes").style.gridTemplateColumns.includes("34.33fr"), "the drag holds for this visit");
});

// ── 7. The desktop's setting each mirror names is a real one ─────────────

// These scripts say which desktop setting each of their browser keys mirrors ("the desktop's
// config.json `files_layout`"). The review of 2026-10-20 found files-panes.js still naming
// `media_layout`, the field's name before the page was the Files page. Seen red the same day
// against it as it was: "files-panes.js names config.json `media_layout`, a field the desktop's
// settings (src/config.rs AppConfig) have".
test("7. every config.json setting the Files page's scripts name is a field of the desktop's settings", () => {
  const config = fs.readFileSync(path.join(ROOT, "src", "config.rs"), "utf8");
  const fields = new Set([...config.matchAll(/^\s*pub (\w+):/gm)].map((m) => m[1]));
  const files = fs.readdirSync(path.join(WEB, "pages")).filter((f) => /^files-.*\.js$/.test(f)).map((f) => path.join("pages", f));
  files.push(path.join("shared", "how-it-works.js"));
  let named = 0;
  for (const f of files) {
    const text = fs.readFileSync(path.join(WEB, f), "utf8");
    for (const m of text.matchAll(/config\.json `(\w+)`/g)) {
      named++;
      assert.ok(fields.has(m[1]), `${path.basename(f)} names config.json \`${m[1]}\`, a field the desktop's settings (src/config.rs AppConfig) have`);
    }
  }
  assert.ok(named >= 2, "the check found the names it is for: " + named);
});
