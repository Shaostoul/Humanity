// After an erase, only the person's Enter signs up again (BUG-135, the operator's option 2,
// 2026-10-04). The web half; the relay half is tested in src/relay/features.rs and the native
// half in src/gui/connections.rs and src/net/ws_client.rs.
//
// Run:  node --test scripts/tests/erase-sign-up-again.test.js
//
// What it guards: the relay now remembers, for a limited time, that a key erased its account,
// and signs nothing up for it again unless the identify says `sign_up_again`. So that field
// must come from the person pressing Enter under the erase note, on that one socket, and never
// from a reload's auto-connect or an automatic reconnect, or the erased account would come back
// by itself exactly as before.
//
// The page scripts run as they do in the browser, in one shared global scope (node:vm), with
// the DOM replaced by a stub: elements remember what is written to them, anything else is a
// value that accepts everything (the pattern of voice-rejoin.test.js).

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const WEB = path.join(__dirname, "..", "..", "web");

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

// An element that keeps what is written to it (value, textContent, style), so the page's
// own reads see the test's writes.
function element() {
  const target = { value: "", textContent: "", style: {}, dataset: {}, disabled: false };
  return new Proxy(target, {
    get: (t, p) => (p in t ? t[p] : anything()),
    set: (t, p, v) => { t[p] = v; return true; },
  });
}

function fakeDocument() {
  const els = new Map();
  return new Proxy({}, {
    get(_t, prop) {
      if (prop === "getElementById") {
        return (id) => {
          if (!els.has(id)) els.set(id, element());
          return els.get(id);
        };
      }
      return anything();
    },
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

// A socket that records what it was sent; the page assigns its onopen.
function fakeSocket() {
  return { readyState: 0, sent: [], send(s) { this.sent.push(JSON.parse(s)); }, close() { this.readyState = 3; } };
}

function loadChat(storage) {
  const timers = [];
  const ctx = {
    __timers: timers,
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: fakeDocument(),
    navigator: anything(),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", reload() {} },
    localStorage: storage || fakeStorage(),
    sessionStorage: fakeStorage(),
    fetch: () => Promise.reject(new Error("no network in tests")),
    // Timers are kept, not run, so a test can run the one it means (the load-time
    // auto-connect, scheduleReconnect's redial) exactly when it chooses.
    setTimeout: (fn) => { timers.push(fn); return timers.length; },
    clearTimeout: () => {},
    setInterval: () => 0,
    clearInterval: () => {},
    WebSocket: Object.assign(function () { return fakeSocket(); }, { OPEN: 1, CONNECTING: 0, CLOSED: 3 }),
    addEventListener: () => {},
    removeEventListener: () => {},
    matchMedia: () => anything(),
    requestAnimationFrame: () => 0,
    Notification: anything(),
    crypto: anything(),
    alert: () => {},
    TextEncoder,
    TextDecoder,
    URLSearchParams,
    URL,
    // What crypto.js and pq.js give the page before app.js runs: an identity, ready.
    getOrCreateIdentity: async () => ({ publicKeyHex: "ab12cd34", canSign: true }),
    attachPqIdentity: async () => true,
    myKyberPublicBase64: "",
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  const run = (rel) => vm.runInContext(fs.readFileSync(path.join(WEB, rel), "utf8"), ctx, { filename: rel });
  run("shared/events.js");
  run("chat/app.js");
  for (const name of ["updateUserList", "updateStats", "renderServerList", "addSystemMessage", "hosIcon", "shortKey", "esc"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  run("chat/chat-privacy.js");
  return ctx;
}

const ERASED_FLAG = "humanity_account_erased";

// Press the login screen's Enter (or run connect() the way a reload or a restore does when
// `how` is undefined), open the socket, and return the identify it sent.
async function connectAndIdentify(ctx, how, name = "Ada") {
  ctx.document.getElementById("name-input").value = name;
  await vm.runInContext("connect", ctx)(how);
  const sock = vm.runInContext("ws", ctx);
  sock.onopen();
  sock.readyState = 1;
  const identify = sock.sent.find((m) => m.type === "identify");
  assert.ok(identify, "the socket sent an identify");
  return identify;
}

// Seen red 2026-10-04 with the `sign_up_again` lines taken out of openSocket's onopen:
// "the Enter under the erase note did not say sign_up_again".
test("Enter under the erase note says sign_up_again on that socket only", async () => {
  const storage = fakeStorage();
  storage.setItem(ERASED_FLAG, "erased");
  const ctx = loadChat(storage);
  const identify = await connectAndIdentify(ctx, "enter");
  assert.strictEqual(identify.sign_up_again, true, "the Enter under the erase note did not say sign_up_again");
  assert.strictEqual(storage.getItem(ERASED_FLAG), null, "the choice is made: the note's flag is cleared");

  // The network drops; the page reconnects by itself. That identify must not say it.
  const first = vm.runInContext("ws", ctx);
  first.close();
  vm.runInContext("ws = null; openSocket();", ctx);
  const again = vm.runInContext("ws", ctx);
  assert.notStrictEqual(again, first, "a new socket");
  again.onopen();
  const reconnect = again.sent.find((m) => m.type === "identify");
  assert.ok(reconnect, "the reconnect identified");
  assert.ok(!("sign_up_again" in reconnect), "an automatic reconnect said sign_up_again");
});

// Seen red 2026-10-04 with signUpAgainChoice ignoring `how` (any connect under the note):
// "a connect that was not the person's Enter said sign_up_again".
test("a connect that is not the person's Enter never says it, note or not", async () => {
  const storage = fakeStorage();
  storage.setItem(ERASED_FLAG, "erased");
  const ctx = loadChat(storage);
  const identify = await connectAndIdentify(ctx, undefined);
  assert.ok(!("sign_up_again" in identify), "a connect that was not the person's Enter said sign_up_again");

  // And Enter with no erase note says nothing either.
  const fresh = loadChat(fakeStorage());
  const plain = await connectAndIdentify(fresh, "enter");
  assert.ok(!("sign_up_again" in plain), "Enter with no erase here said sign_up_again");
});

// The relay's answer to a key it remembers erasing (`earlier`: true) puts the note back, and
// the next Enter is the person's choice to come back.
//
// Seen red 2026-10-04 with the `sign_up_again` lines taken out of openSocket's onopen:
// "Expected values to be strictly equal" (undefined, not true).
test("an earlier erase told by the server brings the note back, and Enter then signs up again", async () => {
  const storage = fakeStorage();
  const ctx = loadChat(storage);
  await connectAndIdentify(ctx, "enter");
  await vm.runInContext("handleMessage", ctx)({ type: "account_erased", to: "ab12cd34", partial: false, earlier: true });
  assert.strictEqual(storage.getItem(ERASED_FLAG), "erased", "the note's flag is set");
  assert.strictEqual(storage.getItem("humanity_name"), null, "the saved name is forgotten, so a reload does not connect");
  const identify = await connectAndIdentify(ctx, "enter");
  assert.strictEqual(identify.sign_up_again, true);
});

// The sentence before the erase: the real number of days, the same words as native
// (src/relay/storage/erased_accounts.rs `erase_memory_sentence`, whose test pins this text).
// It names the backups (review finding 2: "after that nothing of it is left" was not true of
// them), and says nothing at all when the server's number is unknown (finding 6: a promise
// about a server that may not keep it is worse than none).
//
// Seen red 2026-10-04 on c0c04fc39 (the old sentence): "Expected values to be strictly
// equal" ('... after that nothing of it is left.' against the new words).
test("the erase sentence names the server's days in the native words, and the backups", () => {
  const ctx = loadChat();
  const say = vm.runInContext("eraseMemorySentence", ctx);
  assert.strictEqual(
    say(30),
    "After the erase this server remembers for up to 30 days that this account was erased, as a one-way fingerprint that is not your name or your data, so your other devices do not sign you up again by themselves; then the entry is deleted here, and a copy of it in this server's backups lasts until that backup is deleted."
  );
  assert.match(say(1), /for up to 1 day that/);
  assert.strictEqual(say(null), "", "a sentence was promised with no number from the server");
  assert.strictEqual(say(0), "");
});

// Review finding 17: drive the page's own automatic paths, not openSocket() by hand. The
// redial scheduleReconnect sets after a drop, and the auto-connect a reload makes when a name
// is saved, never say sign_up_again, even right after the person's Enter said it, or with the
// erase note's flag still set.
//
// Seen red 2026-10-04 with scheduleReconnect's redial changed to openSocket({ signUpAgain:
// true }): "the redial after a drop said sign_up_again".
test("the automatic redial and the reload's auto-connect never say sign_up_again", async () => {
  const storage = fakeStorage();
  storage.setItem(ERASED_FLAG, "erased");
  const ctx = loadChat(storage);
  const identify = await connectAndIdentify(ctx, "enter");
  assert.strictEqual(identify.sign_up_again, true);

  // The network drops: the page's own onclose schedules the redial; run it.
  const first = vm.runInContext("ws", ctx);
  first.readyState = 3;
  const before = ctx.__timers.length;
  first.onclose();
  assert.ok(ctx.__timers.length > before, "a drop scheduled no redial");
  ctx.__timers[ctx.__timers.length - 1]();
  const redial = vm.runInContext("ws", ctx);
  assert.notStrictEqual(redial, first, "the redial opened a new socket");
  redial.onopen();
  const again = redial.sent.find((m) => m.type === "identify");
  assert.ok(again, "the redial identified");
  assert.ok(!("sign_up_again" in again), "the redial after a drop said sign_up_again");

  // A reload with a saved name connects by itself (app.js, at load): never the choice.
  const reloaded = fakeStorage();
  reloaded.setItem("humanity_name", "Ada");
  reloaded.setItem(ERASED_FLAG, "erased");
  const page = loadChat(reloaded);
  assert.ok(page.__timers.length > 0, "a saved name scheduled no auto-connect");
  await page.__timers[0]();
  const sock = vm.runInContext("ws", page);
  assert.ok(sock, "the auto-connect opened a socket");
  sock.onopen();
  const auto = sock.sent.find((m) => m.type === "identify");
  assert.ok(auto, "the auto-connect identified");
  assert.ok(!("sign_up_again" in auto), "a reload's auto-connect said sign_up_again");
});

// Review finding 17: the number of days is read from the server's real answer. The field
// name is the one the relay serializes (src/relay/api.rs ServerInfoResponse), so a rename on
// either side fails here instead of silently falling back.
//
// Seen red 2026-10-04 with eraseMemoryDays reading `info.erase_days`: "Expected values to be
// strictly equal" (null, not 12).
test("the erase dialog reads the days from /api/server-info's real field", async () => {
  const api = fs.readFileSync(path.join(__dirname, "..", "..", "src", "relay", "api.rs"), "utf8");
  const struct = api.slice(api.indexOf("pub struct ServerInfoResponse"));
  assert.match(struct.slice(0, struct.indexOf("\n}")), /pub erased_accounts_ttl_days: i64,/, "the relay no longer sends erased_accounts_ttl_days");
  const ctx = loadChat();
  let asked = null;
  ctx.fetch = async (url) => { asked = url; return { ok: true, json: async () => ({ name: "x", erased_accounts_ttl_days: 12 }) }; };
  const days = await vm.runInContext("eraseMemoryDays", ctx)();
  assert.strictEqual(asked, "/api/server-info");
  assert.strictEqual(days, 12);
  ctx.fetch = async () => ({ ok: true, json: async () => ({ name: "an older relay" }) });
  assert.strictEqual(await vm.runInContext("eraseMemoryDays", ctx)(), null, "a relay without the field gave a number");
});

// The Tasks board signs in on its own socket. Review finding 10: it did not know
// `account_erased`, so it kept retrying (5 s doubling to a minute) and told the person to
// "check the connection". It is a refusal: the page says why and waits for the next click.
//
// Seen red 2026-10-04 on c0c04fc39: "the Tasks page did not take account_erased as a
// refusal".
test("the Tasks page takes account_erased as a refusal and stops retrying", async () => {
  const timers = [];
  const sockets = [];
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: fakeDocument(),
    navigator: anything(),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/tasks", search: "" },
    localStorage: fakeStorage(),
    fetch: () => Promise.reject(new Error("no network in tests")),
    setTimeout: (fn) => { timers.push(fn); return timers.length; },
    clearTimeout: () => {},
    WebSocket: Object.assign(function () {
      const handlers = {};
      const s = {
        readyState: 1, sent: [],
        addEventListener(t, fn) { (handlers[t] = handlers[t] || []).push(fn); },
        send(m) { this.sent.push(JSON.parse(m)); },
        close() { this.readyState = 3; (handlers.close || []).forEach((f) => f()); },
        fire(t, ev) { (handlers[t] || []).forEach((f) => f(ev)); },
      };
      sockets.push(s);
      return s;
    }, { OPEN: 1, CONNECTING: 0, CLOSED: 3 }),
    TextEncoder,
    btoa: (s) => Buffer.from(s, "binary").toString("base64"),
  };
  ctx.window = ctx;
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(WEB, "pages", "tasks-app.js"), "utf8"), ctx, { filename: "tasks-app.js" });
  vm.runInContext("loadTaskIdentity = async () => ({ dilithiumPublicHex: 'ab12cd34' })", ctx);
  assert.strictEqual(await vm.runInContext("ensureTaskWs", ctx)(), true);
  const sock = sockets[sockets.length - 1];
  sock.fire("open");
  assert.ok(sock.sent.find((m) => m.type === "identify"), "the page identified");
  const before = timers.length;
  sock.fire("message", { data: JSON.stringify({ type: "account_erased", to: "ab12cd34", partial: false, earlier: true }) });
  assert.match(vm.runInContext("taskWsRefusal", ctx), /erased/, "the Tasks page did not take account_erased as a refusal");
  assert.strictEqual(sock.readyState, 3, "the refused socket was left open");
  assert.strictEqual(timers.length, before, "the page scheduled another sign-in after the refusal");
  assert.match(vm.runInContext("taskSignInHelp()", ctx), /erased/, "the person is not told why");
});
