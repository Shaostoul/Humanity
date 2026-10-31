// The website's VIEW TOGGLE (web/shared/view-toggle.js, 2026-10-30), the twin of the app's
// src/gui/widgets/view_toggle.rs: one list, Table or Cards. Pure node with a stand-in document
// and storage, no browser: runs in `just rig-tests`.
//
// What it holds: the toggle draws Table then Cards, the fallback chosen until a choice is kept;
// a click on the other view tells the page, presses that button and keeps the choice on this
// device, so the next visit draws it; a click on the view already shown does nothing; a kept
// value that is no view (an old or edited one) falls back.
//
// Seen red 2026-10-30 with the click not keeping the choice (`store(id, v)` left out):
// "AssertionError [ERR_ASSERTION]: the next visit draws the kept view".

const test = require("node:test");
const assert = require("node:assert");

// The stand-ins: a document whose elements keep what the toggle sets, and a storage.
function element(tag) {
  const el = {
    tagName: tag.toUpperCase(),
    children: [],
    attrs: {},
    dataset: {},
    listeners: {},
    className: "",
    textContent: "",
    classList: { added: [], add(c) { this.added.push(c); } },
    setAttribute(k, v) { this.attrs[k] = String(v); },
    getAttribute(k) { return this.attrs[k]; },
    appendChild(c) { this.children.push(c); return c; },
    addEventListener(type, fn) { this.listeners[type] = fn; },
    click() { if (this.listeners.click) this.listeners.click(); },
  };
  return el;
}
const doc = { createElement: element };
const store = {};
global.window = {
  localStorage: { getItem: (k) => (k in store ? store[k] : null), setItem: (k, v) => { store[k] = String(v); } },
};
const Toggle = require("../../web/shared/view-toggle.js");
delete global.window;

function host() {
  const el = element("div");
  el.ownerDocument = doc;
  return el;
}
const pressed = (el) => el.children.filter((b) => b.attrs["aria-pressed"] === "true").map((b) => b.dataset.view);

test("Table then Cards, the fallback until a choice is kept", () => {
  const el = host();
  const view = Toggle.render(el, "t1", "cards", () => {});
  assert.strictEqual(view, "cards");
  assert.deepStrictEqual(el.children.map((b) => b.dataset.view), ["table", "cards"]);
  assert.deepStrictEqual(pressed(el), ["cards"]);
  assert.strictEqual(el.attrs.role, "group");
  // Each button says what it does: its hint is a key's words (here the key, with no i18n loaded).
  assert.ok(el.children.every((b) => b.dataset.tipDesc && b.dataset.tipDesc.startsWith("common.view_")));
});

test("a click on the other view tells the page, presses it and keeps it", () => {
  const el = host();
  const picked = [];
  Toggle.render(el, "t2", "cards", (v) => picked.push(v));
  el.children[0].click();
  assert.deepStrictEqual(picked, ["table"]);
  assert.deepStrictEqual(pressed(el), ["table"]);
  el.children[0].click();
  assert.deepStrictEqual(picked, ["table"], "the view shown already: nothing");
  const again = host();
  assert.strictEqual(Toggle.render(again, "t2", "cards", () => {}), "table", "the next visit draws the kept view");
  assert.strictEqual(Toggle.render(host(), "t3", "cards", () => {}), "cards", "another list keeps its own");
});

test("a kept value that is no view falls back", () => {
  store.hos_view_t4 = "grid";
  assert.strictEqual(Toggle.render(host(), "t4", "table", () => {}), "table");
  assert.strictEqual(Toggle.normalize("cards", "table"), "cards");
  assert.strictEqual(Toggle.normalize(null, "cards"), "cards");
});
