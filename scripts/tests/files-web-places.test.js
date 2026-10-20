// The Files page's places on the website (2026-10-19): web/pages/files-places.js, with the two
// places that are not the page's own list, Shared files (web/pages/files-shared.js, the Shared Files
// page until then) and the HumanityOS folder (web/pages/files-folder.js, the data browser that was at
// /files). The desktop's twin is src/gui/pages/files/places.rs. The operator, 2026-10-19: "What's the
// real difference between these two? Why would we not want to merge them?"
//
// Runs under Node's own test runner against the stand-in document of files-web-harness.js:
//   node --test scripts/tests/files-web-places.test.js
"use strict";

const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");

const H = require("./files-web-harness.js");
const { WEB, ME, ANN, MIB, byId, text, respond, at, settle, listAnswer, listRoutes, openPage, aFile, fakeSession } = H;
const FPL = require(path.join(WEB, "pages", "files-places.js"));

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
test("1. the places: three for everyone and Admin for an admin, each at its own address", () => {
  assert.deepEqual(FPL.listed(false), ["shared", "shared-files", "folder"]);
  assert.deepEqual(FPL.listed(true), ["shared", "shared-files", "folder", "admin"]);
  for (const place of FPL.listed(true)) assert.equal(FPL.placeOf(FPL.addressOf(place).replace("/files", "")), place, "the address names the place: " + place);
  assert.equal(FPL.addressOf("shared"), "/files", "what is shared is the page itself");
  assert.equal(FPL.placeOf("#nothing"), "shared", "an unknown hash shows what is shared");
  assert.equal(FPL.shownPlace("admin", false), "shared", "Admin is an admin's alone");
  assert.equal(FPL.shownPlace("folder", false), "folder");
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
  const folder = await openPage({ view: "panes", hash: "#folder", routes: routes() });
  assert.equal(byId(folder.doc, "files-place-folder").getAttribute("aria-current"), "page", "/files#folder opens the HumanityOS folder");
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
