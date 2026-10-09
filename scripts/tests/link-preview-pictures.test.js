// Link-preview pictures load by themselves only from this site (2026-10-09).
//
// Run:  node --test scripts/tests/link-preview-pictures.test.js
//
// Why it matters: when a message carries a link, the relay sends a preview
// (title, description and the address of a picture). web/chat/app.js used to
// put that picture straight into the page, so the reader's browser fetched it
// from the other website, which shows that site the reader's network address
// and when they read the message. Someone could post a link to a site they run
// to collect the address of everyone who scrolled past
// (docs/design/blocking-and-safe-mode.md, section 7.1 item 5). Now only a
// picture this site serves loads by itself; any other waits behind a "Load
// picture" button that names the site, as pictures in messages already wait
// for a click.
//
// The page scripts run as they do in the browser, in one shared global scope
// (node:vm). The DOM is a stub, except that created elements remember their
// HTML and a message lookup finds one fake message, so the real link_previews
// handler can be fed a frame and the card it builds read back.
// HOS_WEB_DIR points the test at another copy of web/ (used to see it red
// against the code before the fix).

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");
const SITE = "https://united-humanity.example";

function anything() {
  const fn = function () {};
  return new Proxy(fn, {
    get(_t, prop) {
      if (prop === Symbol.toPrimitive) return () => "";
      if (prop === Symbol.iterator) return function* () {};
      if (prop === "then") return undefined;
      if (prop === "length") return 0;
      return anything();
    },
    set: () => true,
    has: () => true,
    apply: () => anything(),
    construct: () => anything(),
  });
}

function fakeStorage() {
  const m = new Map();
  return {
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => m.set(k, String(v)),
    removeItem: (k) => m.delete(k),
    clear: () => m.clear(),
  };
}

const htmlEscape = (t) => String(t).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

// An element that keeps what is written to it; anything else is the stub.
function fakeElement(tag) {
  const real = {
    tagName: String(tag).toUpperCase(),
    className: "",
    style: {},
    dataset: {},
    _html: "",
    replacedWith: null,
    classList: {
      set: new Set(),
      add(c) { this.set.add(c); },
      remove(c) { this.set.delete(c); },
      toggle(c) { if (this.set.has(c)) this.set.delete(c); else this.set.add(c); },
      contains(c) { return this.set.has(c); },
    },
    replaceWith(n) { this.replacedWith = n; },
  };
  Object.defineProperty(real, "textContent", { set(t) { this._html = htmlEscape(t); }, get() { return this._html; } });
  Object.defineProperty(real, "innerHTML", { set(h) { this._html = String(h); }, get() { return this._html; } });
  return new Proxy(real, {
    get: (t, p) => (p in t ? t[p] : anything()),
    set: (t, p, v) => { t[p] = v; return true; },
  });
}

function loadChat() {
  // The message the previews belong to: its body records the cards put after it.
  const inserted = [];
  const body = { after: (card) => inserted.push(card) };
  const message = { querySelector: (sel) => (sel === ".body" ? body : null) };
  const hooks = { findMessage: false };
  const document = new Proxy({}, {
    get(_t, p) {
      if (p === "createElement") return fakeElement;
      if (p === "querySelector") return (sel) => (hooks.findMessage && sel.startsWith(".message[") ? message : anything());
      return anything();
    },
  });
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document,
    navigator: anything(),
    location: { hash: "", host: "united-humanity.example", origin: SITE, protocol: "https:", pathname: "/chat", search: "", reload() {} },
    localStorage: fakeStorage(),
    sessionStorage: fakeStorage(),
    fetch: () => Promise.reject(new Error("no network in tests")),
    setTimeout: () => 0,
    clearTimeout: () => {},
    setInterval: () => 0,
    clearInterval: () => {},
    WebSocket: Object.assign(function () { return anything(); }, { OPEN: 1, CONNECTING: 0, CLOSED: 3 }),
    addEventListener: () => {},
    removeEventListener: () => {},
    matchMedia: () => anything(),
    requestAnimationFrame: () => 0,
    Notification: anything(),
    crypto: anything(),
    TextEncoder,
    TextDecoder,
    URLSearchParams,
    URL,
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  const run = (rel) => vm.runInContext(fs.readFileSync(path.join(WEB, rel), "utf8"), ctx, { filename: rel });
  run("shared/events.js");
  run("chat/app.js");
  for (const name of ["updateUserList", "updateStats", "renderServerList", "addSystemMessage", "shortKey"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  ctx.hosIcon = () => ""; // shared/icons.js is not loaded here
  hooks.findMessage = true;
  return { ctx, inserted };
}

// Feed the relay's link_previews frame through the page's real message handler,
// and return the HTML of each card it put under the message.
async function previewCards(ctx, inserted, previews) {
  await vm.runInContext("handleMessage", ctx)({
    type: "link_previews", channel: "general", from: "a1".repeat(32), timestamp: 1700000000000, previews,
  });
  return inserted.map((card) => card.innerHTML);
}

// Seen red 2026-10-09 against the code before the fix (HOS_WEB_DIR pointed at
// HEAD's web/): the card held an <img class="lp-thumb"> pointing at
// tracker.example, which the browser fetches as soon as the card is shown, and
// there was no button. Also red with only the `auto:` line of
// linkPreviewPicture changed to `auto: true` (this test and the last two).
// The protocol line and the resolve-against-the-link line were each disabled
// once too: "only ordinary web addresses" and "a short picture address" went
// red, each on its own.
test("a picture on another website does not load by itself; a button names the site", async () => {
  const { ctx, inserted } = loadChat();
  const [html] = await previewCards(ctx, inserted, [
    { url: "https://news.example/story", title: "Story", description: "About it", site_name: "News", image: "https://tracker.example/p.png?id=7" },
  ]);
  assert.ok(html.includes("Story"), "the preview itself is shown");
  assert.ok(!html.includes("<img"), "no picture element, so nothing is fetched from the other site");
  assert.ok(html.includes("Load picture"), "a Load picture button is offered");
  assert.ok(html.includes("tracker.example"), "the button names the site the picture is on");
  assert.ok(html.includes('data-lp-picture="https://tracker.example/p.png?id=7"'), "the button carries the picture's address");
});

test("pressing Load picture puts the picture in its place", async () => {
  const { ctx, inserted } = loadChat();
  await previewCards(ctx, inserted, [
    { url: "https://news.example/story", title: "Story", image: "https://tracker.example/p.png" },
  ]);
  const card = inserted[0];
  const button = fakeElement("button");
  button.dataset.lpPicture = "https://tracker.example/p.png";
  card.onclick({ target: { closest: (sel) => (sel === "[data-lp-picture]" ? button : null) } });
  assert.ok(button.replacedWith, "the button was replaced");
  assert.strictEqual(button.replacedWith.tagName, "IMG");
  assert.strictEqual(button.replacedWith.src, "https://tracker.example/p.png");
  assert.strictEqual(card.classList.contains("collapsed"), false, "pressing the button does not fold the card away");
});

test("a picture this site serves itself still shows straight away", async () => {
  const { ctx, inserted } = loadChat();
  const [html] = await previewCards(ctx, inserted, [
    { url: SITE + "/library", title: "Library", image: SITE + "/uploads/cover.png" },
  ]);
  assert.ok(html.includes(`<img class="lp-thumb" src="${SITE}/uploads/cover.png"`), "shown as a picture");
  assert.ok(!html.includes("Load picture"), "no button needed");
});

test("a short picture address belongs to the linked page's site, not to this one", () => {
  const { ctx } = loadChat();
  const pick = vm.runInContext("linkPreviewPicture", ctx);
  // "/img/p.png" on news.example's page means news.example's picture.
  const pic = pick({ url: "https://news.example/story", image: "/img/p.png" }, SITE);
  assert.strictEqual(pic.url, "https://news.example/img/p.png");
  assert.strictEqual(pic.auto, false);
  assert.strictEqual(pic.site, "news.example");
  // "//host/x" keeps its own host.
  assert.strictEqual(pick({ url: "https://news.example/", image: "//cdn.example/x.png" }, SITE).site, "cdn.example");
});

test("only ordinary web addresses are offered at all", () => {
  const { ctx } = loadChat();
  const pick = vm.runInContext("linkPreviewPicture", ctx);
  for (const image of ["javascript:alert(1)", "data:image/png;base64,AAAA", "file:///etc/passwd", "", "   "]) {
    assert.strictEqual(pick({ url: "https://news.example/", image }, SITE), null, `no picture for ${JSON.stringify(image)}`);
  }
  assert.strictEqual(pick({ url: "https://news.example/" }, SITE), null, "no image field, no picture");
  assert.strictEqual(pick({ image: "https://cdn.example/x.png" }, SITE).auto, false, "a preview with no link still works");
  assert.strictEqual(pick({ url: "https://news.example/", image: "https://cdn.example/x.png" }, "").auto, false, "with no known page origin nothing loads by itself");
});
