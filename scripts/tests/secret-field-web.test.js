// The Show / Hide eye beside every password box on the website (web/shared/secret-field.js,
// 2026-11-22; the desktop app's twin is src/gui/widgets/secret_field.rs). The operator, on his
// phone: "I also have no way of previewing the passphrase in the encrypt your private key window.
// It's just the dots with encrypt or skip." Run: node scripts/tests/secret-field-web.test.js
//
// Seen red 2026-11-22 before secret-field.js existed: "web/shared/secret-field.js: ENOENT"; and
// with hideAll left out of the window's blur, "switching windows hides it again" found the box
// still type "text".

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const repo = path.resolve(__dirname, "..", "..");

/** A small page: elements with children, attributes, clicks, and a window that can lose focus. */
function fakePage() {
  const listeners = { window: {}, document: {} };
  const on = (bag) => (type, fn) => { (bag[type] = bag[type] || []).push(fn); };
  const win = { addEventListener: on(listeners.window) };
  const doc = {
    visibilityState: "visible",
    readyState: "complete",
    defaultView: win,
    addEventListener: on(listeners.document),
    createElement(tag) { return element(tag); },
    createElementNS(_ns, tag) { return element(tag); },
  };
  function element(tag) {
    const el = {
      tagName: tag.toUpperCase(), children: [], parentNode: null, type: "", title: "", className: "",
      style: { cssText: "" }, dataset: {}, attrs: {}, clicks: [], ownerDocument: doc,
      setAttribute(k, v) { el.attrs[k] = String(v); },
      getAttribute(k) { return k in el.attrs ? el.attrs[k] : null; },
      appendChild(c) { c.parentNode = el; el.children.push(c); return c; },
      insertBefore(c, ref) { c.parentNode = el; el.children.splice(el.children.indexOf(ref), 0, c); return c; },
      get nextSibling() { const p = el.parentNode; if (!p) return null; return p.children[p.children.indexOf(el) + 1] || null; },
      replaceChildren(...cs) { el.children = []; cs.forEach((c) => el.appendChild(c)); },
      addEventListener(type, fn) { if (type === "click") el.clicks.push(fn); },
      click() { el.clicks.forEach((fn) => fn()); },
      focus() {},
      all() { return el.children.flatMap((c) => [c, ...c.all()]); },
      querySelectorAll(sel) {
        assert.strictEqual(sel, 'input[type="password"]');
        return el.all().filter((c) => c.tagName === "INPUT" && c.type === "password");
      },
    };
    return el;
  }
  doc.body = element("body");
  doc.querySelectorAll = (sel) => doc.body.querySelectorAll(sel);
  const fire = (bag, type) => (bag[type] || []).forEach((fn) => fn());
  return { doc, win, blur: () => fire(listeners.window, "blur"), hideTab: () => { doc.visibilityState = "hidden"; fire(listeners.document, "visibilitychange"); } };
}

/** secret-field.js loaded into a context with `page`'s document; the module's API. */
function load(page) {
  const src = fs.readFileSync(path.join(repo, "web/shared/secret-field.js"), "utf8");
  const ctx = { document: page.doc, window: page.win, i18n: { t: (k) => ({ "secret_field.show": "Show", "secret_field.hide": "Hide" })[k] || k } };
  ctx.globalThis = ctx;
  vm.createContext(ctx);
  vm.runInContext(src.replace("typeof window !== 'undefined' ? window : globalThis", "globalThis"), ctx);
  return ctx.HosSecretField;
}

/** A password box inside a dialog in the page. */
function passwordBox(page) {
  const dialog = page.doc.createElement("div");
  const input = page.doc.createElement("input");
  input.type = "password";
  dialog.appendChild(input);
  page.doc.body.appendChild(dialog);
  return input;
}

test("every password box gets a Show button; pressing it shows the text, again hides it", () => {
  const page = fakePage();
  const input = passwordBox(page);
  load(page);
  const button = input.parentNode.children.find((c) => c.tagName === "BUTTON");
  assert.ok(button, "no Show button beside the password box");
  assert.strictEqual(input.type, "password", "the box does not start hidden");
  assert.strictEqual(button.attrs["aria-label"], "Show");
  assert.strictEqual(button.attrs["aria-pressed"], "false");
  assert.ok(button.children.some((c) => c.tagName === "SVG"), "the button holds no drawn eye");
  button.click();
  assert.strictEqual(input.type, "text", "pressing Show does not show the text");
  assert.strictEqual(button.attrs["aria-label"], "Hide");
  button.click();
  assert.strictEqual(input.type, "password", "pressing Hide does not hide it");
});

test("switching windows or tabs hides it again", () => {
  const page = fakePage();
  const input = passwordBox(page);
  const api = load(page);
  const button = input.parentNode.children.find((c) => c.tagName === "BUTTON");
  button.click();
  assert.strictEqual(input.type, "text");
  page.blur();
  assert.strictEqual(input.type, "password", "switching windows hides it again");
  button.click();
  page.hideTab();
  assert.strictEqual(input.type, "password", "hiding the tab hides it again");
  assert.ok(api);
});

test("a box with its own button is left alone, and a box is given one button only", () => {
  const page = fakePage();
  const own = passwordBox(page);
  own.setAttribute("data-secret-field", "own");
  const api = load(page);
  assert.ok(!own.parentNode.children.some((c) => c.tagName === "BUTTON"), "a second button beside a box with its own");
  const later = passwordBox(page);
  api.scan(page.doc);
  api.scan(page.doc);
  assert.strictEqual(later.parentNode.children.filter((c) => c.tagName === "BUTTON").length, 1);
});

test("shell.js loads it on every page, and its words are in the catalog", () => {
  const shell = fs.readFileSync(path.join(repo, "web/shared/shell.js"), "utf8");
  assert.ok(shell.includes("loadSharedModule('/shared/secret-field.js')"), "shell.js does not load secret-field.js");
  const en = JSON.parse(fs.readFileSync(path.join(repo, "data/i18n/en.json"), "utf8"));
  for (const k of ["show", "hide", "show_hint", "hide_hint"]) {
    assert.ok(en.secret_field && en.secret_field[k], `secret_field.${k} is not in data/i18n/en.json`);
  }
});
