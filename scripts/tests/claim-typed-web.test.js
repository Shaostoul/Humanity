// A `/claim` typed in the website's chat goes to the server alone (2026-11-20, the owner review's
// finding 6), run under Node:
//
//   node --test scripts/tests/claim-typed-web.test.js      (in `just rig-tests`)
//
// The claim code from the server's own log makes its bearer the server's owner, so a line typed
// with it must never reach a person: with a direct message or a group open, `/claim <code>` used to
// be routed like any message (chat-ui.js intercepted only the bare `/claim`; sendComposedContent
// and chat-groups-p2p.js's composer sent the rest to the people on screen). Now any line whose first
// word is /claim, in any letter case, goes to the server alone as
// {"type":"owner_claim","code","typed":true} before any routing (app.js takeTypedClaim, the twin of
// the desktop's net/owner.rs typed_claim). What it proves:
//  1. A direct message open: the code goes out only in owner_claim; nothing is routed to the DM.
//  2. A P2P group open: the same; nothing is posted to the group.
//  3. The group's own composer, with chat-ui.js not loaded: the same.
//  4. sendComposedContent (the router the attachment paths use) given a /claim line: the same.
//  5. The public room: owner_claim, not a chat line; and `/claimed` is not a claim.
//
// The page scripts run as in the browser, in one shared global scope (node:vm), over the stand-in
// page of chat-links-web.test.js (files-web-harness.js's elements where they are built). The two
// modules chat-groups-p2p.js imports are stand-ins whose group message carries its plaintext, so
// a line sent to a group is seen in what is posted. HOS_WEB_DIR points the test at another copy of
// web/ (used to see it red).
//
// Seen red 2026-11-20 against the website before the fix (the test written first): all five failed;
// the DM "nothing was routed to the people on screen" (the line '/claim K7Q2-...' went to the DM's
// router), each group "nothing posted carries the code" (the group message's plaintext was
// '/CLAIM   K7Q2-...'), the router "handled", the room "the claim went to the server". Then each
// check taken out alone in a copy of web/chat (HOS_WEB_DIR), and only its own test failed:
// chat-ui.js's composer: test 1; sendComposedContent: test 4; the group composer: test 3 ("nothing
// posted"); app.js's sendMessage: test 3's room ("the claim went to the server"); only a lower-case
// /claim taken: tests 2 and 3; any line starting /claim taken: test 5 ("/claimed" is a message).

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const H = require("./files-web-harness.js");
const ROOT = H.ROOT;
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");

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

function keeper(extra) {
  const kept = Object.assign({
    innerHTML: "", dataset: {}, style: {}, listeners: {},
    addEventListener(type, f) { (kept.listeners[type] = kept.listeners[type] || []).push(f); },
  }, extra || {});
  return new Proxy(kept, { get: (o, p) => (p in o ? o[p] : anything()), set: (o, p, v) => { o[p] = v; return true; } });
}

function fakeStorage() {
  const m = new Map();
  return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k), clear: () => m.clear() };
}

/** The two modules chat-groups-p2p.js imports: a group message's submission carries its plaintext. */
const STAND_INS = {
  "/shared/pq-object.js": { buildGroupMsgV1: async ({ plaintext }) => ({ objectId: "built", submission: { object_type: "group_msg_v1", plaintext } }) },
  "/shared/vendor/noble-pq.bundle.js": { blake3: { create: () => ({ update() { return this; }, digest: () => new Uint8Array(32) }) } },
};

const settle = async () => { for (let i = 0; i < 20; i++) await new Promise((r) => setImmediate(r)); };

/** The chat page's scripts (chat-ui.js unless `withoutUi`), signed in, the socket open. */
function loadChat(opts = {}) {
  const doc = H.fakeDocument();
  const input = keeper({ value: "", disabled: false, focus() {} });
  const thread = keeper({ value: "", disabled: false, focus() {} });
  const els = { messages: keeper(), "msg-input": input, "thread-input": thread, "send-btn": keeper({ disabled: false }), "channel-header": keeper(), "server-list": keeper() };
  const document = new Proxy({}, {
    get(_t, p) {
      if (p === "getElementById") return (id) => els[id] || doc.getElementById(id) || anything();
      if (p === "createElement") return (tag) => doc.createElement(tag);
      if (p === "createTextNode") return (s) => doc.createTextNode(s);
      if (p === "body") return doc.body;
      if (p === "readyState") return "complete";
      if (p === "addEventListener" || p === "removeEventListener") return () => {};
      return anything();
    },
  });
  const posted = [];
  const serviceWorker = { register: () => Promise.resolve({ scope: "/" }), ready: new Promise(() => {}), controller: null, addEventListener() {} };
  const navigator = new Proxy({}, { get: (_t, p) => (p === "serviceWorker" ? serviceWorker : anything()), has: () => true });
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document, navigator, location: { hash: "", pathname: "/chat", search: "", host: "united-humanity.us", protocol: "https:", origin: "https://united-humanity.us", reload() {} },
    history: { state: null, replaceState() {}, pushState() {} },
    localStorage: fakeStorage(), sessionStorage: fakeStorage(),
    fetch: async (url, o) => { posted.push(String(url) + " " + String((o && o.body) || "")); return { ok: true, status: 200, json: async () => ({}), text: async () => "" }; },
    setTimeout: () => 0, clearTimeout: () => {}, setInterval: () => 0, clearInterval: () => {},
    WebSocket: Object.assign(function () { return anything(); }, { OPEN: 1, CONNECTING: 0, CLOSED: 3 }),
    addEventListener: () => {}, removeEventListener: () => {}, matchMedia: () => anything(),
    requestAnimationFrame: () => 0, Notification: anything(), innerWidth: 1280, innerHeight: 800,
    crypto: globalThis.crypto, btoa: globalThis.btoa, atob: globalThis.atob, prompt: () => null,
    TextEncoder, TextDecoder, URLSearchParams, URL, CustomEvent: function () {},
    __testImport: async (spec) => STAND_INS[spec],
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  const run = (rel, change) => {
    const src = fs.readFileSync(path.join(WEB, rel), "utf8");
    vm.runInContext(change ? change(src) : src, ctx, { filename: rel });
  };
  const files = ["shared/safe-markup.js", "shared/i18n-en.js", "shared/i18n.js", "shared/events.js", "shared/roles.js", "shared/friend-pass.js", "shared/reach.js", "shared/block.js",
    "chat/crypto.js", "chat/chat-dm-store.js", "chat/view/timestampPill.js", "chat/view/messageRow.js",
    "shared/chat-access.js", "shared/chat-links.js", "chat/app.js", "chat/chat-messages.js", "chat/chat-dms.js", "chat/chat-social.js",
    "chat/chat-groups-p2p.js", "shared/notify-rules.js", "chat/chat-notify.js", "chat/chat-ui.js"];
  for (const rel of files) {
    if (opts.withoutUi && rel === "chat/chat-ui.js") continue;
    if (!fs.existsSync(path.join(WEB, rel))) continue;
    run(rel, rel === "chat/chat-groups-p2p.js" ? (s) => s.replace(/\bimport\(/g, "__testImport(") : null);
  }
  ctx.hosIcon = () => "";
  ctx.generateIdenticon = () => ""; // the stand-in page has no canvas
  ctx.addChatMessage = () => {};     // a sent line's echo on screen is not what this test reads
  ctx.esc =(str) => String(str).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
  for (const f of ["loadHistory", "loadPinsForChannel", "loadReactionsForChannel", "renderScratchPadNotes", "renderDmConversationFromStore"]) ctx[f] = () => Promise.resolve();
  const notices = [];
  ctx.addNotice = (text) => notices.push(String(text));
  ctx.addSystemMessage = (text) => notices.push(String(text));
  // The app's own lines (app.js appLine, the claim help among them) arrive as drawn elements.
  ctx.appendMessage = (el) => notices.push(String(el && el.textContent));
  const sent = [];
  const set = (js) => vm.runInContext(js, ctx);
  set("myKey = 'me'; myName = 'Me'; myIdentity = { canSign: true }; myDilithiumSecret = new Uint8Array(4); peerData = { me: { public_key: 'me', role: '' }, bea: { public_key: 'bea', display_name: 'Bea', role: '' } };");
  vm.runInContext("(s) => { ws = s; }", ctx)({ readyState: 1, send: (s) => sent.push(JSON.parse(s)) });
  ctx.pqSignMessage = async () => new Uint8Array(8);
  // What reaches the router the DM and group sends go through, counted (it still runs).
  const routed = [];
  if (typeof ctx.sendComposedContent === "function") {
    const real = ctx.sendComposedContent;
    ctx.sendComposedContent = async (c) => { routed.push(String(c)); return real(c); };
  }
  const type = async (text) => { input.value = text; await vm.runInContext("sendMessage()", ctx); await settle(); };
  return { ctx, set, input, thread, sent, posted, routed, notices, type };
}

const CODE = "K7Q2-M9XV-P4TR";

/** Nothing but one owner_claim carried the code, to the server: no frame, post or routed line. */
function onlyToTheServer(page, code) {
  // The leaks first, so a red run names where the code went.
  assert.deepStrictEqual(page.routed.filter((p) => p.includes(code)), [], "nothing was routed to the people on screen");
  assert.deepStrictEqual(page.posted.filter((p) => p.includes(code)), [], "nothing posted carries the code");
  const leaks = page.sent.filter((f) => f.type !== "owner_claim" && JSON.stringify(f).includes(code));
  assert.deepStrictEqual(leaks, [], "no other frame carries the code");
  const claims = page.sent.filter((f) => f.type === "owner_claim");
  assert.deepStrictEqual(claims, [{ type: "owner_claim", code, typed: true }], "the claim went to the server, typed: " + JSON.stringify(page.sent));
}

test("1. a direct message open: /claim <code> goes to the server alone", async () => {
  const page = loadChat();
  page.set("activeDmPartner = 'bea';");
  await page.type("/claim " + CODE);
  onlyToTheServer(page, CODE);
  assert.strictEqual(page.input.value, "", "the composer is emptied");
});

test("2. a P2P group open: /CLAIM <code>, in any case, goes to the server alone", async () => {
  const page = loadChat();
  page.ctx.activeP2pGroup = { id: "g1", name: "Hikers", epoch: 1, epochKey: "K" };
  await page.type("  /CLAIM   " + CODE + "  ");
  onlyToTheServer(page, CODE);
});

test("3. the group's own composer, and app.js's, chat-ui.js not loaded: still the server alone", async () => {
  const page = loadChat({ withoutUi: true });
  page.ctx.activeP2pGroup = { id: "g1", name: "Hikers", epoch: 1, epochKey: "K" };
  await page.type("/Claim " + CODE);
  onlyToTheServer(page, CODE);
  // No group open: the line reaches app.js's own sendMessage, which keeps it off the room too.
  const room = loadChat({ withoutUi: true });
  await room.type("/claim " + CODE);
  onlyToTheServer(room, CODE);
});

test("4. sendComposedContent given a /claim line (the attachment paths' router): the server alone", async () => {
  const page = loadChat();
  page.set("activeDmPartner = 'bea';");
  const ok = await vm.runInContext("sendComposedContent", page.ctx)("/claim " + CODE);
  await settle();
  assert.strictEqual(ok, true, "handled");
  const claims = page.sent.filter((f) => f.type === "owner_claim");
  assert.deepStrictEqual(claims, [{ type: "owner_claim", code: CODE, typed: true }]);
  assert.deepStrictEqual(page.sent.filter((f) => f.type !== "owner_claim"), [], "nothing else sent");
  page.ctx.activeP2pGroup = { id: "g1", name: "Hikers", epoch: 1, epochKey: "K" };
  page.set("activeDmPartner = null;");
  await vm.runInContext("sendComposedContent", page.ctx)("/claim " + CODE);
  await settle();
  assert.deepStrictEqual(page.posted.filter((p) => p.includes(CODE)), [], "nothing posted to the group");
});

test("5. the public room: owner_claim, not a chat line; and a line only starting /claim reaches nobody", async () => {
  const page = loadChat();
  await page.type("/claim " + CODE);
  onlyToTheServer(page, CODE);
  page.set("activeDmPartner = 'bea';");
  await page.type("/claimed my prize");
  // The second review's W5: a line starting with /claim is never a message, even when it is not a
  // well-formed claim; it shows the claim help instead.
  assert.ok(!page.routed.includes("/claimed my prize"), "never routed to a person: " + JSON.stringify(page.routed));
  assert.ok(page.notices.some((n) => n.includes("/claim")), "the claim help is shown: " + JSON.stringify(page.notices));
  assert.strictEqual(page.sent.filter((f) => f.type === "owner_claim").length, 1, "no second claim");
});

// THE NEAR MISSES (the owner branch's second review, W5; the critic's `/claimCODE`, `/claim:CODE`
// and a zero-width space): each, typed with a direct message open, is a claim for the server alone,
// and none reaches the person. Seen red 2026-11-20 with the branch's typedClaimCode (the first word
// must equal /claim): "nothing was routed to the people on screen" (the lines went to Bea), and
// test 5 "never routed to a person: ["/claimed my prize"]".
test("7. /claimCODE, /claim:CODE and a zero-width space in front: the server alone", async () => {
  for (const line of ["/claim" + CODE, "/claim:" + CODE, "\u200B/claim " + CODE, "\uFEFF/CLAIM: " + CODE]) {
    const page = loadChat();
    page.set("activeDmPartner = 'bea';");
    await page.type(line);
    onlyToTheServer(page, CODE);
  }
});

// THE EDIT PATH (W5): saving an edit whose new text is a /claim line sends no edit to the room; the
// claim goes to the server alone. Seen red 2026-11-20 against the branch's chat-messages.js (no
// takeTypedClaim in Save): "Save asks takeTypedClaim before the edit frame is built".
test("8. an edit that is a /claim line is never sent to the room", async () => {
  const page = loadChat();
  const said = await vm.runInContext("(async (c) => { const t = takeTypedClaim(c); return t; })", page.ctx)("/claim " + CODE);
  assert.strictEqual(said, true, "the edit's line is taken as a claim");
  onlyToTheServer(page, CODE);
  const src = fs.readFileSync(path.join(WEB, "chat", "chat-messages.js"), "utf8");
  const save = src.slice(src.indexOf("saveBtn.onclick"), src.indexOf("type: 'edit'"));
  assert.ok(/takeTypedClaim\(newContent\)\) return;/.test(save), "Save asks takeTypedClaim before the edit frame is built");
});

// THE THREAD REPLY (the owner branch's third review, W3): a reply typed in a thread's own box that is
// a /claim line goes to the server alone, never into the room as a reply quoting the thread.
// Seen red 2026-11-21 against the branch's chat-messages.js (sendThreadReply without
// takeTypedClaim): "no other frame carries the code" got the chat frame with the reply.
test("9. a thread reply that is a /claim line goes to the server alone", async () => {
  const page = loadChat();
  page.set("currentThread = { from: 'bea', author: 'Bea', timestamp: 5, body: 'hello' };");
  page.thread.value = "/claim " + CODE;
  // A reply that went out would also draw itself in the thread (by markup the stand-in page refuses):
  // kept, so the leak is what a red run names.
  let failed = null;
  await vm.runInContext("sendThreadReply()", page.ctx).catch((e) => { failed = e; });
  await settle();
  onlyToTheServer(page, CODE);
  assert.strictEqual(failed, null, "the reply path did not run");
  assert.strictEqual(page.thread.value, "", "the box is emptied");
});

test("6. /claim alone, in a direct message, opens Claim ownership and sends nothing", async () => {
  const page = loadChat();
  let opened = 0;
  page.ctx.openOwnerClaim = () => { opened += 1; };
  page.set("activeDmPartner = 'bea';");
  await page.type("/claim");
  assert.strictEqual(opened, 1, "Claim ownership opened");
  assert.deepStrictEqual(page.sent, [], "nothing sent");
  assert.deepStrictEqual(page.routed, [], "nothing routed");
});
