// The recovery phrase and the device-link code, covered until shown, on the website
// (web/shared/phrase-cover.js, 2026-10-16; the desktop app's twin is
// src/gui/widgets/phrase_cover.rs). Run: node scripts/tests/phrase-cover-web.test.js
//
// Seen red 2026-10-16: with the window's blur not covering the words, "switching to another
// window covers them" found all 24 still in the page; with the intro's step 1 writing its own word
// grid again, "the intro's step 1 holds none of the words" found them in its HTML.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const repo = path.resolve(__dirname, "..", "..");
const PHRASE = "alpha deal scrub asthma idea logic bright thought alpha deal scrub asthma idea logic bright thought alpha deal scrub asthma idea logic bright truly";

/** A small page: elements with children, text, clicks, and a window that can lose focus. */
function fakePage() {
  const listeners = { window: {}, document: {} };
  const on = (bag) => (type, fn) => { (bag[type] = bag[type] || []).push(fn); };
  const off = (bag) => (type, fn) => { bag[type] = (bag[type] || []).filter((f) => f !== fn); };
  const win = { addEventListener: on(listeners.window), removeEventListener: off(listeners.window) };
  const doc = {
    visibilityState: "visible",
    defaultView: win,
    addEventListener: on(listeners.document),
    removeEventListener: off(listeners.document),
    createElement(tag) { return element(tag); },
  };
  function element(tag) {
    const el = {
      tagName: tag.toUpperCase(), children: [], parent: null, ownText: "", className: "", type: "", style: { cssText: "" },
      ownerDocument: doc, clicks: [],
      get isConnected() { let e = el; while (e.parent) e = e.parent; return e === doc.body; },
      set textContent(t) { el.ownText = String(t); el.children = []; },
      get textContent() { return el.ownText + el.children.map((c) => c.textContent).join(" "); },
      appendChild(c) { c.parent = el; el.children.push(c); return c; },
      append(...cs) { cs.forEach((c) => el.appendChild(c)); },
      replaceChildren() { el.children.forEach((c) => { c.parent = null; }); el.children = []; },
      addEventListener(type, fn) { if (type === "click") el.clicks.push(fn); },
      click() { el.clicks.forEach((fn) => fn()); },
      all() { return el.children.flatMap((c) => [c, ...c.all()]); },
    };
    return el;
  }
  doc.body = element("body");
  const fire = (bag, type) => (listeners[bag][type] || []).slice().forEach((fn) => fn());
  return { doc, win, listeners, fire };
}

const cover = require(path.join(repo, "web/shared/phrase-cover.js"));
const button = (host, text) => host.all().find((e) => e.tagName === "BUTTON" && e.textContent === text);
const wordsShown = (host) => PHRASE.split(" ").filter((w) => host.all().some((e) => e.className === "phrase-cover-word" && e.textContent === w)).length;

test("the words are covered until shown, and cover themselves again", () => {
  const page = fakePage();
  const host = page.doc.body.appendChild(page.doc.createElement("div"));
  cover.words(host, PHRASE);
  assert.equal(wordsShown(host), 0, "nothing of the phrase is in the page at first");
  assert.ok(!PHRASE.split(" ").some((w) => host.textContent.split(/\s+/).includes(w)), "not one word, anywhere in it");
  assert.equal(host.all().filter((e) => e.textContent === cover.WORDS.coveredWord).length, 24, "24 covered places");
  assert.ok(host.textContent.includes(cover.WORDS.warning), "the line saying when it is safe");

  button(host, cover.WORDS.show).click();
  assert.equal(wordsShown(host), 24, "Show the words shows all 24");
  page.fire("window", "blur");
  assert.equal(wordsShown(host), 0, "switching to another window covers them");

  button(host, cover.WORDS.show).click();
  page.doc.visibilityState = "hidden";
  page.fire("document", "visibilitychange");
  assert.equal(wordsShown(host), 0, "a hidden tab covers them");
  page.doc.visibilityState = "visible";

  button(host, cover.WORDS.show).click();
  button(host, cover.WORDS.hide).click();
  assert.equal(wordsShown(host), 0, "Hide the words covers them");

  host.parent.children = [];
  host.parent = null;
  page.fire("window", "blur");
  assert.equal((page.listeners.window.blur || []).length, 0, "a cover no longer in the page stops listening");
});

test("the device-link code is covered until shown, and drawn into the page", () => {
  const page = fakePage();
  const host = page.doc.body.appendChild(page.doc.createElement("div"));
  let drawnInPage = null;
  cover.code(host, (body) => {
    drawnInPage = body.isConnected;
    body.appendChild(page.doc.createElement("canvas"));
  });
  assert.equal(drawnInPage, null, "the code is not drawn while covered");
  assert.ok(host.textContent.includes(cover.WORDS.codeHidden) && host.textContent.includes(cover.WORDS.codeWarning));
  button(host, cover.WORDS.showCode).click();
  assert.equal(drawnInPage, true, "Show the code draws it, already in the page (its QR is found by id)");
  page.fire("window", "blur");
  assert.ok(!host.all().some((e) => e.tagName === "CANVAS"), "switching windows takes it off the page");
});

test("the intro's step 1 holds none of the words until Show the words", () => {
  const ctx = { window: {}, console };
  ctx.globalThis = ctx;
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(repo, "web/chat/chat-onboarding.js"), "utf8"), ctx);
  const html = ctx.step1(PHRASE, false);
  assert.ok(html.includes('id="ob-phrase-cover"'), "a place for the covered words");
  assert.ok(!PHRASE.split(" ").some((w) => new RegExp("\\b" + w + "\\b").test(html)), "the intro's step 1 holds none of the words");

  // Wired: the words go into that place, covered.
  const page = fakePage();
  const host = page.doc.body.appendChild(page.doc.createElement("div"));
  ctx.window.HosPhraseCover = cover;
  const overlay = { querySelector: (sel) => (sel === "#ob-phrase-cover" ? host : null) };
  ctx.wireStep1(overlay, PHRASE);
  assert.equal(host.all().filter((e) => e.textContent === cover.WORDS.coveredWord).length, 24, "covered in place");
  button(host, cover.WORDS.show).click();
  assert.equal(wordsShown(host), 24, "and shown on Show the words");
});
