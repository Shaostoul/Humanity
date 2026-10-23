// The website's chat message rows mirror the desktop app's (Track W step 3, 2026-10-22,
// docs/design/web-native-parity.md). The desktop app is the parent; what it draws, in
// src/gui/pages/chat.rs and src/gui/widgets/row.rs:
//   - one timestamp pill per message holding the time, the Þ, and the reactions INSIDE it, each
//     its emoji and count, sorted, yours in the accent (paint_timestamp_pill);
//   - the Þ opens the expanded pill (`pill_expand`): ↩ Reply, Pin, Edit, a Þ, the quick reactions
//     and ∞ for all of them, at the pill's right edge, and the full time "YYYY-MM-DD HH:MM:SS UTC"
//     to the left of the Þ (`pill_ts_expand`); a private row (a P2P group, the scratch pad,
//     is_private_channel) offers Reply alone;
//   - a right-click on a message opens its menu: Copy text, ↩ Quote / reply, Pin message, Edit,
//     Delete, then Report and Block on someone else's message;
//   - sender groups alternate their ground, bg_primary then row_stripe;
//   - a quote reads as one line, with no empty line after it.
// Until this change the web drew the reactions as separate bordered pills beside the time, opened
// an emoji strip above the row's right edge (cut in half on the first message by the list's top
// edge), had a hover action bar native has not, no stripes, and an empty line after a quote.
//
// Run: node --test scripts/tests/chat-rows-web.test.js   (in `just rig-tests`)
//
// What runs: the view modules (web/chat/view/timestampPill.js, pillPopup.js, messageMenu.js) on a
// small stand-in document that holds only what they built in it (no innerHTML: a test that finds
// a button proves the module made it); the chat itself (app.js addChatMessage and formatBody,
// chat-messages.js renderReactions) in a node:vm context with an absorbing document, as
// block-web.test.js runs it; and the stylesheets and theme as files. HOS_WEB_DIR points it at
// another copy of web/ (used to see each test red).
//
// Red first, 2026-10-22: run against the web/ of fb790eb8b (HOS_WEB_DIR) all 17 failed: no
// reactionBadgesHTML, no pillPopup.js or messageMenu.js, the reactions container outside the pill,
// the Þ a <span>, no --row-stripe, the hover ground and the hover bar in messages.css, and
// formatBody's "</div>\n" after a quote. And against deliberate breaks of the new code: the
// private-row test in pillPopupItems taken out failed "a private row's bar offers Reply alone";
// Edit offered without the row's .edit-btn failed both menu-contents tests; formatBody's join
// without the replace failed "the next line follows the quote at once".

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const read = (rel) => fs.readFileSync(path.join(WEB, rel), "utf8").replace(/\r\n/g, "\n");
const has = (rel) => fs.existsSync(path.join(WEB, rel));

const ME = "a1".repeat(32);
const ANN = "b2".repeat(32);
const BEN = "c3".repeat(32);
const T0 = Date.UTC(2026, 9, 22, 20, 55, 3); // 2026-10-22 20:55:03 UTC

// ── A stand-in document that holds only what was built in it ────────────

class El {
  constructor(doc, tag) {
    this.ownerDocument = doc;
    this.tagName = String(tag).toUpperCase();
    this.childNodes = [];
    this.parentNode = null;
    this.attrs = new Map();
    this.dataset = {};
    this.style = {};
    this.listeners = {};
    this.className = "";
    this.id = "";
    this.hidden = false;
    this.title = "";
    this._text = "";
    this.rect = { left: 0, top: 0, width: 0, height: 0 };
    const self = this;
    this.classList = {
      add: (c) => { if (!self.classList.contains(c)) self.className = (self.className + " " + c).trim(); },
      remove: (c) => { self.className = self.className.split(/\s+/).filter((x) => x && x !== c).join(" "); },
      contains: (c) => self.className.split(/\s+/).includes(c),
      toggle: (c, on) => { (on === undefined ? !self.classList.contains(c) : on) ? self.classList.add(c) : self.classList.remove(c); },
    };
  }
  get children() { return this.childNodes; }
  get firstChild() { return this.childNodes[0] || null; }
  get isConnected() { for (let n = this; n; n = n.parentNode) if (n === this.ownerDocument.body) return true; return false; }
  get textContent() { return this._text + this.childNodes.map((c) => c.textContent).join(""); }
  set textContent(v) { this.childNodes = []; this._text = String(v); }
  get innerHTML() { throw new Error("the module read innerHTML"); }
  set innerHTML(v) { throw new Error("the module set innerHTML: " + String(v).slice(0, 60)); }
  appendChild(c) { if (c.parentNode) c.parentNode.removeChild(c); c.parentNode = this; this.childNodes.push(c); return c; }
  removeChild(c) { const i = this.childNodes.indexOf(c); if (i >= 0) this.childNodes.splice(i, 1); c.parentNode = null; return c; }
  replaceChildren(...cs) { for (const c of this.childNodes.slice()) this.removeChild(c); this._text = ""; for (const c of cs) this.appendChild(c); }
  remove() { if (this.parentNode) this.parentNode.removeChild(this); }
  contains(n) { for (; n; n = n.parentNode) if (n === this) return true; return false; }
  setAttribute(k, v) { this.attrs.set(k, String(v)); if (k === "id") this.id = String(v); }
  getAttribute(k) { return this.attrs.has(k) ? this.attrs.get(k) : null; }
  removeAttribute(k) { this.attrs.delete(k); }
  addEventListener(t, f) { (this.listeners[t] = this.listeners[t] || []).push(f); }
  removeEventListener(t, f) { this.listeners[t] = (this.listeners[t] || []).filter((x) => x !== f); }
  dispatch(type, extra) {
    const ev = Object.assign({ type, target: this, preventDefault() { ev.defaultPrevented = true; }, stopPropagation() {} }, extra || {});
    for (let n = this; n; n = n.parentNode) for (const f of (n.listeners[type] || []).slice()) f(ev);
    return ev;
  }
  click() { this.dispatch("click", { detail: 1 }); }
  focus() { this.ownerDocument.activeElement = this; }
  getBoundingClientRect() {
    const r = this.rect;
    return { left: r.left, top: r.top, width: r.width, height: r.height, right: r.left + r.width, bottom: r.top + r.height };
  }
  matches(sel) {
    return sel.split(",").map((s) => s.trim()).some((s) => {
      let m;
      if ((m = /^\.([\w-]+)$/.exec(s))) return this.classList.contains(m[1]);
      if ((m = /^#([\w-]+)$/.exec(s))) return this.id === m[1];
      if (/^[a-z]+$/i.test(s)) return this.tagName === s.toUpperCase();
      throw new Error("the stand-in does not know the selector " + s);
    });
  }
  closest(sel) { for (let n = this; n && n.matches; n = n.parentNode) if (n.matches(sel)) return n; return null; }
  querySelectorAll(sel) {
    const out = [];
    const walk = (n) => { for (const c of n.childNodes) { if (c.matches && c.matches(sel)) out.push(c); if (c.childNodes) walk(c); } };
    walk(this);
    return out;
  }
  querySelector(sel) { return this.querySelectorAll(sel)[0] || null; }
}

function standIn(view = { w: 1280, h: 900 }) {
  const docListeners = {};
  const winListeners = {};
  const doc = {
    activeElement: null,
    createElement: (tag) => new El(doc, tag),
    addEventListener: (t, f) => { (docListeners[t] = docListeners[t] || []).push(f); },
    removeEventListener: (t, f) => { docListeners[t] = (docListeners[t] || []).filter((x) => x !== f); },
    getElementById: (id) => doc.body.querySelector("#" + id),
    listeners: docListeners,
  };
  doc.body = new El(doc, "body");
  doc.activeElement = doc.body;
  const timers = [];
  const win = {
    document: doc,
    innerWidth: view.w,
    innerHeight: view.h,
    addEventListener: (t, f) => { (winListeners[t] = winListeners[t] || []).push(f); },
    removeEventListener: (t, f) => { winListeners[t] = (winListeners[t] || []).filter((x) => x !== f); },
    setTimeout: (f, ms) => { timers.push({ f, ms }); return timers.length; },
    clearTimeout: (id) => { if (timers[id - 1]) timers[id - 1].f = null; },
    navigator: { clipboard: { copied: [], writeText(t) { this.copied.push(t); return Promise.resolve(); } } },
    getSelection: () => "",
    Array, Object, String, Math, Number, Date, isFinite,
  };
  win.window = win;
  vm.createContext(win);
  const run = (rel) => vm.runInContext(read(rel), win, { filename: rel });
  // Run what is waiting on a timer (the close after the pointer left, the focus check).
  const flush = () => { for (const t of timers.splice(0)) if (t.f) t.f(); };
  const docEvent = (type, extra) => {
    const ev = Object.assign({ type, preventDefault() { ev.defaultPrevented = true; }, stopPropagation() {} }, extra || {});
    for (const f of (docListeners[type] || []).slice()) f(ev);
    return ev;
  };
  return { win, doc, run, flush, docEvent };
}

// A drawn row as app.js leaves it: a pill with its Þ, and the touch bar's buttons the row has.
function rowIn(doc, buttons = []) {
  const row = new El(doc, "div");
  row.className = "message";
  const pill = row.appendChild(new El(doc, "span"));
  pill.className = "ts-pill";
  pill.rect = { left: 300, top: 200, width: 90, height: 19 };
  const thorn = pill.appendChild(new El(doc, "button"));
  thorn.className = "ts-thorn";
  thorn.setAttribute("aria-expanded", "false");
  thorn.rect = { left: 345, top: 200, width: 10, height: 19 };
  const bar = row.appendChild(new El(doc, "div"));
  bar.className = "msg-actions";
  for (const [cls, title] of buttons) {
    const b = bar.appendChild(new El(doc, "button"));
    b.className = cls;
    b.title = title || "";
  }
  doc.body.appendChild(row);
  return { row, pill, thorn };
}

const labels = (els) => els.map((b) => b.getAttribute("aria-label") || b.textContent);

// ── 1. The pill ─────────────────────────────────────────────────────────

test("the pill holds the time, the Þ as a button, then the reactions, in that order", () => {
  const ctx = { window: {} };
  vm.createContext(ctx);
  vm.runInContext(read("chat/view/timestampPill.js"), ctx);
  const html = ctx.window.timestampPillHTML({
    time: "20:55", iso: "2026-10-22T20:55:03.000Z", thorn: true,
    reactionsHtml: '<span class="reactions" data-from="k" data-ts="1"></span>',
  });
  const at = (s) => html.indexOf(s);
  assert.ok(html.startsWith('<span class="ts-pill">') && html.endsWith("</span>"), "one pill");
  assert.ok(at('datetime="2026-10-22T20:55:03.000Z">20:55') > 0, "the time, its moment for machines");
  assert.ok(at('<button type="button" class="ts-thorn" aria-haspopup="true" aria-expanded="false"') > at("20:55"), "then the Þ, a button a keyboard reaches");
  assert.ok(html.includes("data-no-tip>Þ</button>"), "its hover opens the bar at once: no hint over it");
  assert.ok(at('class="reactions"') > at("Þ</button>"), "then the reactions");
  assert.equal(html.lastIndexOf("</span></span>"), html.length - "</span></span>".length, "inside the pill");
  assert.ok(!ctx.window.timestampPillHTML({ time: "1" }).includes("ts-thorn"), "no Þ unless asked (a DM row)");
});

test("each reaction reads as the desktop pill paints it: emoji then count, sorted, mine marked, blocked people not counted", () => {
  const ctx = { window: {} };
  vm.createContext(ctx);
  vm.runInContext(read("chat/view/timestampPill.js"), ctx);
  assert.equal(typeof ctx.window.reactionBadgesHTML, "function", "reactionBadgesHTML exists");
  const html = ctx.window.reactionBadgesHTML(
    { "🔥": new Set([ANN]), "❤": new Set([ME, BEN]), "👍": [BEN] },
    { myKey: ME, from: ANN, ts: 5, esc: (s) => String(s), hidden: (k) => k === BEN },
  );
  assert.ok(html.indexOf("❤") < html.indexOf("🔥"), "sorted by emoji, as native sorts them");
  assert.ok(!html.includes("👍"), "a reaction nobody shown is left out");
  assert.ok(html.includes('class="reaction-badge mine"') && html.includes('aria-pressed="true"'), "mine marked");
  assert.ok(html.includes('❤<span class="count">1</span>'), "emoji then count, the blocked reactor not counted");
  assert.ok(html.includes('<button type="button" class="reaction-badge"'), "a button: a keyboard reaches it");
  assert.ok(html.includes(`data-target-from="${ANN}" data-target-ts="5" data-emoji="🔥"`), "it names what it toggles");
});

// ── 2. The expanded pill ────────────────────────────────────────────────

function popupPage(view) {
  const p = standIn(view);
  assert.ok(has("chat/view/pillPopup.js"), "web/chat/view/pillPopup.js exists");
  p.run("chat/view/pillPopup.js");
  return p;
}

function wire(p, row, over = {}) {
  const calls = [];
  p.win.wirePillPopup(row, Object.assign({
    ts: T0, privateRow: false, canPin: false, canEdit: false,
    top: () => ["❤", "👍", "🔥"], all: () => ["❤", "👍", "🔥", "🎉"],
    onReply: () => calls.push(["reply"]), onPin: () => calls.push(["pin"]), onEdit: () => calls.push(["edit"]),
    onReact: (e) => calls.push(["react", e]),
  }, over));
  return calls;
}

const bar = (p) => p.doc.body.querySelector(".pill-popup");
const sizeBar = (p, w = 300, h = 30) => { const b = bar(p); if (b) b.rect = { left: 0, top: 0, width: w, height: h }; };

test("hovering the Þ opens the bar at the pill's right edge: Reply, Pin, Edit, Þ, the quick reactions, ∞; the full time left of the Þ", () => {
  const p = popupPage();
  const { row, thorn } = rowIn(p.doc);
  wire(p, row, { canPin: true, canEdit: true });
  thorn.dispatch("pointerenter", { pointerType: "mouse" });
  const b = bar(p);
  assert.ok(b, "the bar opens on hover");
  assert.equal(b.getAttribute("role"), "toolbar");
  assert.deepEqual(labels(b.firstChild.querySelectorAll("button")), ["Reply", "Pin message", "Edit message", "React with ❤", "React with 👍", "React with 🔥", "All reactions"]);
  assert.ok(b.firstChild.querySelector(".pill-popup-thorn"), "a Þ between the functions and the reactions");
  // The site's hover hint (shell.js) names the functions, as native's on_hover_text does, and
  // stays off the reactions (native shows none there), where it would cover the bar.
  const [reply] = b.firstChild.querySelectorAll("button");
  assert.ok(reply.getAttribute("data-tip-desc"), "Reply has its hint");
  assert.equal(b.querySelector(".pill-popup-react").getAttribute("data-no-tip"), "", "a reaction has none");
  assert.equal(thorn.getAttribute("aria-expanded"), "true");
  const ts = p.doc.body.querySelector(".pill-ts-expand");
  assert.equal(ts && ts.textContent, "2026-10-22 20:55:03 UTC", "the full time, UTC, native's format");
  // Placed: re-place with the bar's real size (the stand-in measures nothing until told).
  sizeBar(p);
  ts.rect = { left: 0, top: 0, width: 160, height: 22 };
  p.win.repositionPillPopup();
  assert.equal(b.style.left, "390px", "at the pill's right edge (300 + 90)");
  assert.equal(b.style.top, "195px", "centred on the pill (209.5 - 15, rounded)");
  assert.equal(ts.style.left, "181px", "the time's right edge 4 px left of the Þ (345 - 4 - 160)");
});

test("the bar stays while the pointer is on the pill or the bar, and closes once it has left both", () => {
  const p = popupPage();
  const { row, pill, thorn } = rowIn(p.doc);
  wire(p, row);
  thorn.dispatch("pointerenter", { pointerType: "mouse" });
  pill.dispatch("pointerleave", { pointerType: "mouse" });
  bar(p).dispatch("pointerenter", { pointerType: "mouse" });
  p.flush();
  assert.ok(bar(p), "crossing from the pill to the bar keeps it");
  bar(p).dispatch("pointerleave", { pointerType: "mouse" });
  p.flush();
  assert.equal(bar(p), null, "off both: closed");
  assert.equal(thorn.getAttribute("aria-expanded"), "false");
  assert.equal(p.doc.body.querySelector(".pill-ts-expand"), null, "the full time goes with it");
  // A finger's tap sends pointerleave too: it never closes what the tap opened.
  thorn.dispatch("click", { detail: 1 });
  pill.dispatch("pointerleave", { pointerType: "touch" });
  p.flush();
  assert.ok(bar(p), "a tap opens it, and the finger lifting does not close it");
});

test("from the keyboard: Enter on the Þ opens it with the focus on Reply, arrows move, Escape closes and gives the focus back", () => {
  const p = popupPage();
  const { row, thorn } = rowIn(p.doc);
  wire(p, row, { canEdit: true });
  thorn.focus();
  thorn.dispatch("click", { detail: 0 }); // what Enter or Space on a button sends
  const b = bar(p);
  assert.ok(b, "open");
  assert.equal(p.doc.activeElement.getAttribute("aria-label"), "Reply", "the focus on its first button");
  b.dispatch("keydown", { key: "ArrowRight" });
  assert.equal(p.doc.activeElement.getAttribute("aria-label"), "Edit message", "ArrowRight");
  b.dispatch("keydown", { key: "End" });
  assert.equal(p.doc.activeElement.getAttribute("aria-label"), "All reactions", "End");
  p.docEvent("keydown", { key: "Escape" });
  assert.equal(bar(p), null, "Escape closes it");
  assert.equal(p.doc.activeElement, thorn, "and the focus is back on the Þ");
});

test("its buttons do what they say; ∞ shows every reaction; a press elsewhere closes it", () => {
  const p = popupPage();
  const { row, thorn } = rowIn(p.doc);
  const calls = wire(p, row, { canPin: true, canEdit: true });
  thorn.click();
  bar(p).querySelector(".pill-popup-react").click();
  assert.deepEqual(calls, [["react", "❤"]], "a quick reaction");
  assert.ok(bar(p), "and the bar stays for another");
  const more = bar(p).querySelector(".pill-popup-more");
  const all = bar(p).querySelector(".pill-popup-all");
  assert.equal(all.hidden, true, "∞'s panel starts closed");
  more.click();
  assert.equal(all.hidden, false, "∞ opens every reaction");
  assert.equal(more.getAttribute("aria-expanded"), "true");
  assert.deepEqual(labels(all.querySelectorAll("button")), ["React with ❤", "React with 👍", "React with 🔥", "React with 🎉"]);
  all.querySelectorAll("button")[3].click();
  assert.deepEqual(calls[1], ["react", "🎉"]);
  assert.equal(all.hidden, true, "a choice closes the panel");
  bar(p).querySelector(".pill-popup-pin").click();
  assert.deepEqual(calls[2], ["pin"]);
  assert.equal(bar(p), null, "a function closes the bar");
  thorn.click();
  p.docEvent("pointerdown", { target: p.doc.body });
  assert.equal(bar(p), null, "a press anywhere else closes it");
  thorn.click();
  p.docEvent("pointerdown", { target: bar(p).firstChild });
  assert.ok(bar(p), "a press on the bar does not");
  // One at a time.
  const other = rowIn(p.doc);
  wire(p, other.row);
  other.thorn.click();
  assert.equal(p.doc.body.querySelectorAll(".pill-popup").length, 1, "opening another closes the first");
  assert.equal(thorn.getAttribute("aria-expanded"), "false");
});

test("a private row's bar offers Reply alone (BUG-178), and Pin and Edit only where the row may", () => {
  const p = popupPage();
  const { row, thorn } = rowIn(p.doc);
  wire(p, row, { privateRow: true, canPin: true, canEdit: true });
  thorn.click();
  assert.deepEqual(labels(bar(p).querySelectorAll("button")), ["Reply"], "a P2P group or the scratch pad: Reply alone");
  p.win.closePillPopup(false);
  const plain = rowIn(p.doc);
  wire(p, plain.row);
  plain.thorn.click();
  assert.deepEqual(labels(bar(p).firstChild.querySelectorAll("button")).slice(0, 2), ["Reply", "React with ❤"], "someone else's message, not staff: no Pin, no Edit");
});

test("where there is no room right of the pill (a phone), the bar opens under it, inside the window", () => {
  const p = popupPage({ w: 375, h: 700 });
  const { row, pill, thorn } = rowIn(p.doc);
  pill.rect = { left: 120, top: 300, width: 80, height: 19 };
  wire(p, row);
  thorn.click();
  sizeBar(p, 340, 30);
  p.win.repositionPillPopup();
  assert.equal(bar(p).style.left, "27px", "kept 8 px inside the right edge (375 - 8 - 340)");
  assert.equal(bar(p).style.top, "321px", "under the pill (319 + 2)");
  const place = p.win.pillPopupPlace({ left: 10, right: 90, top: 680, bottom: 699 }, { w: 300, h: 40 }, { w: 375, h: 700 });
  assert.equal(place.top, 638, "near the bottom: above the pill instead (680 - 2 - 40)");
  // The full time: left of the Þ where it fits; where it would cover the Þ, just above it.
  // Seen 2026-10-22 at 375 px: pinned to the left edge, it hid the Þ.
  const roomy = p.win.pillTimestampPlace({ left: 400, right: 410, top: 200, bottom: 219 }, { w: 160, h: 22 }, { w: 1280, h: 900 });
  assert.deepEqual([roomy.left, roomy.above], [236, false], "left of the Þ (400 - 4 - 160)");
  const tight = p.win.pillTimestampPlace({ left: 140, right: 160, top: 214, bottom: 233 }, { w: 160, h: 22 }, { w: 375, h: 700 });
  assert.deepEqual([tight.left, tight.top, tight.above], [8, 188, true], "above it, inside the window (214 - 4 - 22)");
});

// ── 3. The message menu ─────────────────────────────────────────────────

function menuPage() {
  const p = standIn();
  assert.ok(has("chat/view/messageMenu.js"), "web/chat/view/messageMenu.js exists");
  p.run("chat/view/messageMenu.js");
  const menu = p.doc.body.appendChild(new El(p.doc, "div"));
  menu.id = "user-context-menu";
  menu.rect = { left: 0, top: 0, width: 160, height: 200 };
  return { ...p, menu };
}

const entries = (menu) => menu.querySelectorAll(".ctx-item").map((d) => d.textContent);
// The arrow asks for its text form (U+FE0E), never a colour emoji tile.
const QUOTE = String.fromCharCode(0x21a9, 0xfe0e) + " Quote / reply";

test("a right-click on someone else's message offers native's menu, each entry pressing the row's own button", () => {
  const p = menuPage();
  const { row } = rowIn(p.doc, [["react-btn"], ["reply-btn"], ["pin-btn"], ["mypin-btn"], ["delete-btn", "Delete (moderator)"]]);
  const pressed = [];
  for (const b of row.querySelectorAll("button")) b.addEventListener("click", () => pressed.push(b.className));
  const person = { reported: 0, blocked: 0, report() { this.reported++; }, block() { this.blocked++; } };
  p.win.wireMessageMenu(row, { text: "The garden towers", person });
  const ev = row.dispatch("contextmenu", { clientX: 400, clientY: 300 });
  assert.ok(ev.defaultPrevented, "the browser's own menu is not shown");
  assert.ok(p.menu.classList.contains("open"), "the menu opens");
  assert.deepEqual(entries(p.menu), ["Copy text", QUOTE, "Pin message", "Pin for me", "Delete (moderator)", "Report", "Block"]);
  assert.ok(p.menu.querySelector(".ctx-sep"), "Report and Block apart, after a line");
  p.menu.querySelectorAll(".ctx-item")[1].click();
  assert.deepEqual(pressed, ["reply-btn"], "Quote / reply presses the row's Reply");
  row.dispatch("contextmenu", { clientX: 400, clientY: 300 });
  p.menu.querySelectorAll(".ctx-item")[0].click();
  assert.deepEqual(p.win.navigator.clipboard.copied, ["The garden towers"], "Copy text copies the words");
  row.dispatch("contextmenu", { clientX: 400, clientY: 300 });
  p.menu.querySelectorAll(".ctx-item")[5].click();
  assert.equal(person.reported, 1, "Report");
  assert.equal(p.menu.classList.contains("open"), false, "an entry closes the menu");
});

test("the menu offers only what the row does: my own message (Edit, Delete, no Report); a private row (Reply alone)", () => {
  const p = menuPage();
  const mine = rowIn(p.doc, [["react-btn"], ["reply-btn"], ["edit-btn"], ["mypin-btn"], ["delete-btn", "Delete"]]);
  p.win.wireMessageMenu(mine.row, { text: "mine", person: null });
  mine.row.dispatch("contextmenu", { clientX: 1, clientY: 1 });
  assert.deepEqual(entries(p.menu), ["Copy text", QUOTE, "Pin for me", "Edit", "Delete"]);
  const group = rowIn(p.doc, [["reply-btn"]]);
  p.win.wireMessageMenu(group.row, { text: "", person: { report() {}, block() {} } });
  group.row.dispatch("contextmenu", { clientX: 1, clientY: 1 });
  assert.deepEqual(entries(p.menu), [QUOTE, "Report", "Block"], "a group row: no Pin, Edit or Delete, and a file's marker is never copied");
  // The site's hover hints (shell.js) move a button's title to data-native-title as soon as it is
  // drawn: a moderator's Delete still says so. Seen red 2026-10-22 in the browser: "Delete".
  const modRow = rowIn(p.doc, [["reply-btn"], ["delete-btn"]]);
  modRow.row.querySelector(".delete-btn").setAttribute("data-native-title", "Delete (moderator)");
  p.win.wireMessageMenu(modRow.row, { text: "x", person: { report() {}, block() {} } });
  modRow.row.dispatch("contextmenu", { clientX: 1, clientY: 1 });
  assert.ok(entries(p.menu).includes("Delete (moderator)"), "a moderator's Delete, its title moved");
});

test("from the keyboard the menu opens at the focused control with the focus on its first entry; a link keeps the browser's menu", () => {
  const p = menuPage();
  const { row, thorn } = rowIn(p.doc, [["reply-btn"]]);
  p.win.wireMessageMenu(row, { text: "words", person: null });
  thorn.focus();
  row.dispatch("contextmenu", { target: thorn, clientX: 0, clientY: 0 });
  assert.equal(p.menu.style.left, "345px", "at the Þ");
  assert.equal(p.menu.style.top, "219px", "under it");
  assert.equal(p.doc.activeElement.textContent, "Copy text", "the focus on the first entry");
  p.menu.firstChild.dispatch("keydown", { key: "ArrowDown" });
  assert.equal(p.doc.activeElement.textContent, QUOTE);
  p.menu.firstChild.dispatch("keydown", { key: "Escape" });
  assert.equal(p.menu.classList.contains("open"), false, "Escape closes it");
  assert.equal(p.doc.activeElement, thorn, "and the focus goes back");
  // Opened with the mouse the focus stays on the page: Escape still closes it, and the arrows
  // step into it. Seen red 2026-10-22 in the browser: Escape left a right-clicked menu open.
  p.doc.activeElement = p.doc.body;
  row.dispatch("contextmenu", { clientX: 400, clientY: 300 });
  p.docEvent("keydown", { key: "ArrowDown" });
  assert.equal(p.doc.activeElement.textContent, "Copy text", "ArrowDown steps into a mouse-opened menu");
  p.doc.activeElement = p.doc.body;
  p.docEvent("keydown", { key: "Escape" });
  assert.equal(p.menu.classList.contains("open"), false, "Escape closes a mouse-opened menu");
  const link = row.appendChild(new El(p.doc, "a"));
  const ev = link.dispatch("contextmenu", { clientX: 5, clientY: 5 });
  assert.ok(!ev.defaultPrevented, "a right-click on a link is the browser's");
});

// ── 4. The rows the chat draws ──────────────────────────────────────────

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

// An element that keeps what is written to it and absorbs the rest. Until its HTML is written its
// innerHTML is its text escaped, as a browser's is (app.js esc() relies on that).
const escapeText = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
function absorbing(tag) {
  const own = { tag, children: [], style: {}, dataset: {}, className: "", textContent: "" };
  return new Proxy(own, {
    get(t, prop) {
      if (prop === "then") return undefined;
      if (prop === "innerHTML") return "innerHTML" in t ? t.innerHTML : escapeText(t.textContent);
      if (prop in t) return t[prop];
      return anything();
    },
    set(t, prop, v) { t[prop] = v; return true; },
  });
}

function loadChat() {
  const drawn = [];
  const kept = new Map();
  const keep = (k) => { if (!kept.has(k)) kept.set(k, absorbing(k)); return kept.get(k); };
  const doc = new Proxy({}, {
    get(_t, prop) {
      if (prop === "createElement") return (tag) => absorbing(tag);
      if (prop === "getElementById") return (id) => keep("#" + id);
      if (prop === "querySelector") return (sel) => (String(sel).startsWith(".reactions[") ? keep(sel) : anything());
      if (prop === "querySelectorAll") return () => [];
      return anything();
    },
    set: () => true,
  });
  const storage = () => { const m = new Map(); return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k) }; };
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: doc,
    navigator: anything(),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", reload() {} },
    localStorage: storage(),
    sessionStorage: storage(),
    indexedDB: anything(),
    fetch: () => Promise.reject(new Error("no network in tests")),
    setTimeout: () => 0, clearTimeout() {}, setInterval: () => 0, clearInterval() {},
    WebSocket: Object.assign(function () { return anything(); }, { OPEN: 1 }),
    addEventListener() {}, removeEventListener() {}, matchMedia: () => anything(), requestAnimationFrame: () => 0,
    innerWidth: 1280, innerHeight: 800, crypto: globalThis.crypto, TextEncoder, TextDecoder, URL, URLSearchParams,
    btoa: globalThis.btoa, atob: globalThis.atob, Notification: anything(), Audio: function () { return anything(); },
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  const run = (rel) => vm.runInContext(read(rel), ctx, { filename: rel });
  run("shared/events.js");
  run("chat/view/timestampPill.js");
  run("chat/view/messageRow.js");
  run("chat/app.js");
  run("chat/chat-messages.js");
  run("chat/chat-dms.js");
  for (const name of ["hosIcon", "roleBadge", "updateStats", "isMobile"]) if (typeof ctx[name] !== "function") ctx[name] = () => "";
  // Record what each row hooks: its expanded pill and its menu (the modules have their own tests).
  const popups = [];
  const menus = [];
  const opened = [];
  ctx.wirePillPopup = (row, o) => popups.push({ row, o });
  ctx.wireMessageMenu = (row, o) => menus.push({ row, o });
  ctx.openPillPopup = (row, how) => opened.push({ row, how });
  ctx.appendMessage = (el) => drawn.push(el);
  const sent = [];
  vm.runInContext("(s, me) => { ws = s; myKey = me; myName = 'Me_1'; activeChannel = 'general'; }", ctx)(
    { readyState: 1, send: (s) => sent.push(JSON.parse(s)) }, ME);
  const fn = (name) => vm.runInContext(name, ctx);
  const add = (author, key, body, ts, opts) => fn("addChatMessage")(author, body, ts, key, true, true, null, null, false, null, opts);
  return { ctx, fn, add, drawn, popups, menus, opened, sent, keep };
}

test("a drawn row: the reactions inside the pill, a Þ button, sender groups striped in turn", () => {
  const c = loadChat();
  c.add("Ann", ANN, "first", T0);
  c.add("Ann", ANN, "second", T0 + 1000);
  c.add("Ben", BEN, "third", T0 + 2000);
  c.add("Ann", ANN, "fourth", T0 + 3000);
  const html = c.drawn[0].innerHTML;
  const pill = html.slice(html.indexOf('<span class="ts-pill">'));
  assert.ok(pill.indexOf(`<span class="reactions" data-from="${ANN}" data-ts="${T0}"></span></span>`) > 0, "the reactions container closes the pill");
  assert.ok(html.includes('<button type="button" class="ts-thorn"'), "the Þ is a button");
  assert.ok(html.includes('datetime="2026-10-22T20:55:03.000Z"'), "the time carries its moment");
  const stripe = (el) => (/msg-stripe-([ab])/.exec(el.className) || [])[1];
  assert.deepEqual(c.drawn.map(stripe), ["b", "b", "a", "b"], "one ground per sender group, alternating");
});

test("each row hooks its expanded pill and its menu with what it may offer", () => {
  const c = loadChat();
  c.add("Ann", ANN, "hello", T0);
  c.add("Me_1", ME, "mine", T0 + 1000);
  c.add("Ann", ANN, "in the group", T0 + 2000, { privateRow: true });
  assert.equal(c.popups.length, 3, "every row's Þ is hooked");
  const [ann, mine, group] = c.popups.map((p) => p.o);
  assert.deepEqual([ann.privateRow, ann.canPin, ann.canEdit], [false, false, false], "someone else's, not staff");
  assert.deepEqual([mine.canEdit], [true], "mine: Edit");
  assert.equal(group.privateRow, true, "a group row: private");
  assert.equal(String(ann.ts), String(T0));
  // A reaction from the bar goes out as the relay takes it.
  ann.onReact("🔥");
  assert.deepEqual(c.sent.map((m) => [m.type, m.target_from, m.target_timestamp, m.emoji]), [["reaction", ANN, T0, "🔥"]]);
  // Staff get Pin.
  vm.runInContext(`peerData[myKey] = { role: 'mod' }`, c.ctx);
  c.add("Ann", ANN, "pin me", T0 + 3000);
  assert.equal(c.popups[3].o.canPin, true, "a moderator: Pin");
  // The menu: words to copy and the person on someone else's message; never on mine.
  const [annMenu, mineMenu, groupMenu] = c.menus.map((m) => m.o);
  assert.equal(annMenu.text, "hello");
  assert.ok(annMenu.person && typeof annMenu.person.report === "function" && typeof annMenu.person.block === "function", "Report and Block on Ann's");
  assert.equal(mineMenu.person, null, "none on mine");
  assert.ok(groupMenu.person, "a group row may still be reported, as its sender's menu can");
});

test("the reactions fill the pill, and the old emoji strip is gone", () => {
  const c = loadChat();
  c.add("Ann", ANN, "hello", T0);
  c.fn("applyReaction")(ANN, T0, "❤", BEN, "Ben");
  c.fn("applyReaction")(ANN, T0, "❤", ME, "Me_1");
  const box = c.keep(`.reactions[data-from="${ANN}"][data-ts="${T0}"]`);
  assert.ok(box.innerHTML.includes('class="reaction-badge mine"') && box.innerHTML.includes('❤<span class="count">2</span>'), "❤2, mine");
  assert.ok(!c.drawn[0].innerHTML.includes('class="reaction-picker"'), "no separate picker");
  assert.equal(typeof c.ctx.showReactionPicker, "undefined", "the old emoji strip is gone");
});

// The relay sends every reaction back to everyone, its sender too (src/relay/relay.rs, the
// Reaction arm's broadcast), and a reaction is a toggle. Seen red 2026-10-22 in the browser and
// here: sendReaction applied it at once and the server's copy took it straight back off.
test("my reaction shows once the server's copy comes back, and stays", () => {
  const c = loadChat();
  c.add("Ann", ANN, "hello", T0);
  c.fn("sendReaction")(ANN, T0, "🔥");
  assert.deepEqual(c.sent.map((m) => m.type), ["reaction"], "sent");
  // What the relay broadcasts back (app.js handleMessage's `reaction` case calls this).
  c.fn("applyReaction")(ANN, T0, "🔥", ME, "Me_1");
  const box = c.keep(`.reactions[data-from="${ANN}"][data-ts="${T0}"]`);
  assert.ok(box.innerHTML.includes('🔥<span class="count">1</span>') && box.innerHTML.includes("mine"), "🔥1, mine");
});

test("a quote reads as one line: no empty line after it, nor after a list", () => {
  const c = loadChat();
  const html = c.fn("formatBody")("> from the design notes\nWe should try two\n- more light\n- easier harvest\nend", ANN);
  assert.ok(html.includes('<div class="quote-block">from the design notes</div>We should try two'), "the next line follows the quote at once");
  assert.ok(html.includes("</ul>end"), "and the line after a list");
  assert.ok(c.fn("formatBody")("one\ntwo", ANN).includes("one\ntwo"), "plain line breaks stay");
});

// ── 5. The look: theme and stylesheets ──────────────────────────────────

test("the stripe is a theme token in both themes, from theme.ron's row_stripe", () => {
  const GEN = require(path.join(ROOT, "scripts", "gen-theme-css.js"));
  const ron = GEN.parseTheme(fs.readFileSync(GEN.RON_PATH, "utf8"));
  assert.ok(ron.row_stripe, "theme.ron has row_stripe (native's)");
  const theme = read("shared/theme.css");
  const dark = /:root \{[\s\S]*?--row-stripe: (#[0-9a-f]{6});/.exec(theme);
  assert.ok(dark, "theme.css defines --row-stripe");
  assert.equal(dark[1], "#" + [ron.row_stripe.r, ron.row_stripe.g, ron.row_stripe.b].map((v) => Math.round(v * 255).toString(16).padStart(2, "0")).join(""), "the same colour native paints");
  assert.match(theme, /:root\[data-theme="light"\] \{[\s\S]*?--row-stripe: #[0-9a-f]{6};/, "and a light one");
});

test("the stylesheets: stripes, the reactions inside the pill, no hover ground or hover bar, a solid ground under the words", () => {
  const css = read("chat/messages.css");
  const rule = (sel) => {
    const at = css.indexOf(sel + " {");
    return at < 0 ? "" : css.slice(at, css.indexOf("}", at));
  };
  assert.match(rule(".message.msg-stripe-a"), /background: var\(--row-stripe\)/, "the second sender group's ground");
  assert.ok(rule(".ts-pill .reactions"), "reactions styled inside the pill");
  assert.equal(rule(".message:hover"), "", "no hover ground on a row (native has none)");
  assert.equal(rule(".message:hover .msg-actions"), "", "no hover bar (the Þ and the menu carry the actions)");
  assert.match(rule(".message.mobile-selected .msg-actions"), /display: flex/, "a tap still shows the touch bar");
  assert.match(rule(".pill-popup"), /position: fixed/, "the expanded pill hangs off the page, never cut by the list");
  assert.match(rule(".message .author"), /color: var\(--text-strong\)/, "names in the brightest ink, as native");
  const layout = read("chat/layout.css");
  const area = layout.slice(layout.indexOf("#chat-area {"), layout.indexOf("}", layout.indexOf("#chat-area {")));
  assert.match(area, /background: var\(--bg\)/, "the chat area has a solid ground (no galaxy behind the words)");
  const page = read("chat/index.html");
  const order = ["view/timestampPill.js", "view/messageRow.js", "view/pillPopup.js", "view/messageMenu.js", "/chat/app.js"].map((s) => page.indexOf(s));
  assert.ok(order.every((v, i) => v > 0 && (i === 0 || v > order[i - 1])), "the view modules load before app.js, in order");
});

// ── 6. The channel header and the composer (Track W step 4) ─────────────
// Native (src/gui/pages/chat.rs draw_center_panel and its input bar): "# general" at the heading
// size, then "|  description" small and muted; a DM's header is Back, "DM: Ada" in the DM ink,
// Report, Block; the composer is one rounded box with the text, then Search, Pins (with the
// count), ?, Attach and Send, its hint naming where the message goes ("Message #general").
// Red first, 2026-10-22, against the web/ of 2f872f2be (step 3): each test below failed.

test("the channel header reads as native's: the name, then the description after a bar, not a dash", () => {
  const c = loadChat();
  vm.runInContext("channelList = [{ id: 'general', name: 'general', description: 'General discussion' }]; activeChannel = 'general';", c.ctx);
  c.fn("updateChannelHeader")();
  const html = c.keep("#channel-header").innerHTML;
  assert.ok(html.includes('<span class="ch-name"># general</span><span class="ch-desc">General discussion</span>'), "no '- ' before the description (the bar is drawn by .ch-desc::before)");
  const css = read("chat/sidebar.css");
  assert.match(css, /#channel-header \.ch-desc::before \{ content: '\|'/, "the bar");
  assert.match(css.slice(css.indexOf("#channel-header .ch-name {")), /font-size: var\(--text-xl\)/, "the name at native's heading size (font_size_heading = --text-xl)");
});

test("a DM's header: Back, then DM: and the name in the DM ink, then Report and Block", () => {
  const c = loadChat();
  vm.runInContext("(k) => { activeDmPartner = k; activeDmPartnerName = 'Ada'; }", c.ctx)(ANN);
  c.fn("renderDmHeader")();
  const html = c.keep("#channel-header").innerHTML;
  assert.ok(html.startsWith('<button type="button" class="ch-back" onclick="closeDmView()"'), "Back first, a real button");
  assert.ok(html.includes('<span class="ch-name ch-dm">DM: Ada</span>'), "DM: Ada in the DM ink");
  assert.ok(html.includes(">Report</button>") && html.includes(">Block</button>"), "Report and Block stay");
  const theme = read("shared/theme.css");
  assert.match(theme, /--dm-accent: #[0-9a-f]{6};[\s\S]*:root\[data-theme="light"\][\s\S]*--dm-accent: #[0-9a-f]{6};/, "the DM ink is a theme token in both themes (theme.ron dm_accent)");
  // A DM has no pins: the composer's count from the channel left behind goes. Seen red
  // 2026-10-22 in the browser: "📌 1" from #general in the DM with Ada.
  c.keep("#composer-pin-count").textContent = "1";
  c.fn("openDmConversation")(ANN, "Ada");
  assert.equal(c.keep("#composer-pin-count").textContent, "", "no pin count in a DM");
  assert.equal(c.keep("#msg-input").placeholder, "Message Ada", "and native's hint: Message Ada");
});

test("the composer is native's one box: text, Search, Pins, ?, Attach, Send; its hint names the channel", () => {
  const page = read("chat/index.html");
  const box = page.slice(page.indexOf('<div id="composer-box">'), page.indexOf("</div>", page.indexOf('<div id="composer-box">')));
  assert.ok(box.length > 0, "a composer box");
  const ids = ["cmd-palette-btn", "msg-input", "composer-search-btn", "composer-pins-btn", "help-btn", "attach-btn", "send-btn"].map((id) => box.indexOf(`id="${id}"`));
  assert.ok(ids.every((v, i) => v > 0 && (i === 0 || v > ids[i - 1])), "in native's order, the web's + first: " + ids.join(","));
  assert.ok(box.includes('<span class="attach-label">Attach</span>'), "Attach says so, as native's button does");
  const c = loadChat();
  vm.runInContext("channelList = [{ id: 'general', name: 'general' }]; activeChannel = 'general';", c.ctx);
  c.fn("updateInputForChannel")();
  assert.equal(c.keep("#msg-input").placeholder, "Message #general", "native's hint");
  vm.runInContext("currentPins = [{ from_name: 'Ada', content: 'x', original_timestamp: 1, pinned_by: 'Me' }];", c.ctx);
  c.fn("renderPinBar")();
  assert.equal(c.keep("#composer-pin-count").textContent, "1", "the pin button carries the count, as native's");
  vm.runInContext("currentPins = [];", c.ctx);
  c.fn("renderPinBar")();
  assert.equal(c.keep("#composer-pin-count").textContent, "", "and none when nothing is pinned");
  assert.equal(c.keep("#pins-empty").style.display, "block", "the list then says so");
  const css = read("chat/inputs.css");
  assert.match(css, /#composer-box #composer-search-btn,\s*#composer-box #composer-pins-btn,/, "on a phone Search and Pins stay where they were, so the text keeps its room");
});

// ── 7. The top menu (Track W step 5) ────────────────────────────────────
// Native's menu (src/gui/pages/escape_menu.rs draw_nav_bar_one_tier) is one wrapping row of
// category-coloured groups with a "·" between groups (separator_dot), Chat a group of its own.
// The web's had the same groups but a 1 px line between them with no room around it, and Chat
// shared its group with Watch and Files. Red first, 2026-10-22, against step 4's web/.
test("the site menu: a dot between groups, as native's separator_dot, and Chat a group of its own", () => {
  const shell = read("shared/shell.js");
  const at = shell.indexOf(".hub-nav .nav-divider {");
  const rule = shell.slice(at, shell.indexOf("}", at));
  assert.match(rule, /border-radius: 50%/, "a dot, not a line");
  const chat = shell.indexOf("navTab('/chat'");
  const watch = shell.indexOf("navTab('/watch'");
  assert.ok(chat > 0 && watch > chat, "Chat, then Watch");
  const between = shell.slice(chat, watch);
  assert.ok(between.includes("</span>") && between.includes('<div class="nav-divider"></div>'), "a dot between Chat and Watch, each in its own group");
});
