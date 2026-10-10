// An admin erases another person's data, in the web chat client (2026-10-10,
// docs/design/blocking-and-safe-mode.md 10i), mirroring the desktop app.
//
// Run: node --test scripts/tests/admin-erase-web.test.js   (in `just rig-tests`)
//
// The page scripts run as they do in the browser, in one shared global scope (node:vm), with the
// DOM replaced by a stub that keeps what is written to it, as in block-web.test.js and
// report-web.test.js (whose harness this copies): the real friend-pass.js, reach.js, block.js,
// admin-erase.js, crypto.js, chat-dm-store.js (over a stand-in IndexedDB), app.js,
// chat-messages.js, chat-dms.js, chat-social.js, chat-groups-p2p.js, chat-ui.js,
// chat-voice-rooms.js, chat-voice-calls.js, chat-voice-modal.js, chat-privacy.js and chat-p2p.js, in
// index.html's order. The stub's elements keep the listeners a script adds, so a test types into
// the confirm's real input and presses its real buttons. Only the Dilithium and Kyber primitives
// are stand-ins (nothing here signs or seals). HOS_WEB_DIR points the test at another copy of web/
// (used to see each test red).
//
// What it proves (10i's client items, the web half):
//  0. The words and the frames are 10i's: the confirm sentence, the erased person's words and the
//     two frames' fields are read out of the spec itself. The pure rules: who is offered the
//     action, the exact-name check (trimmed, letter case counts), the frame builder refusing a
//     name that does not match.
//  1. "Erase their data" shows in the member menu and the voice modal only for an admin or the
//     owner, never for a moderator or a plain member, never on an admin's or the owner's row, and
//     never on my own; and the confirm cannot be opened by a moderator by calling it directly.
//  2. The confirm says 10i's sentence for that person, and its Erase button is enabled only while
//     the typed name, trimmed, is exactly theirs; a wrong name sends nothing.
//  3. Erase sends exactly {"type":"admin_erase","target":<key>,"confirm_name":<name>} and nothing
//     else, and the confirm closes.
//  4. admin_erase_done shows the receipt to the admin, in the chat and in a dialog, and says so
//     when part of the erase did not finish.
//  5. account_erased with by_admin: true shows the by-admin words on the login screen (and after a
//     reload), and Enter there still signs up again; without it, or with by_admin: false, the old
//     self-erase words; with partial, the unfinished note.
//
// Red first, 2026-10-10: each mutation made in a fresh copy of web/, this test run against it with
// HOS_WEB_DIR, and seen failing with the assertion named (each passing again on the real web/):
//  0: admin-erase.js adminEraseConfirmText saying "files" for "uploads": "the confirm sentence is
//     10i's, word for word".
//  1: admin-erase.js adminEraseOffered without its targetRole line: "never on an admin's row (Ben)".
//  2: admin-erase.js adminEraseNameMatches comparing in lower case: "a name in the wrong letter
//     case keeps Erase disabled".
//  3: admin-erase.js adminEraseFrame adding `name: name` to the frame: "exactly 10i's frame".
//  4: app.js without its `case 'admin_erase_done'`: "the receipt is shown in the chat".
//  5: admin-erase.js erasedKindOf ignoring by_admin: "the by-admin words are on the login screen".
// And the page's own wiring, the same way: chat-ui.js amAdmin without the owner: "an owner is
// offered it on a member's row (menu)"; app.js's account_erased not asking erasedKindOf: "the
// by-admin words are on the login screen"; chat-voice-modal.js offering it without the rule: "never
// on an admin's row (voice modal)"; chat-ui.js's input listener not touching the button: "the name
// with spaces around it enables Erase (trimmed)".

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");
const ROOT = path.join(__dirname, "..", "..");
const ae = require(path.join(WEB, "shared", "admin-erase.js"));

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

// An element that keeps what is written to it (text, children, handlers, style, attributes) and
// absorbs the rest. Its listeners are kept, so a test can type into an input and press a button a
// script built; `remove()` is recorded. Until its HTML is written, its innerHTML is its text
// escaped, as a browser's is (app.js esc() relies on that).
const escapeText = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
function fakeElement(tag) {
  const own = { tag, children: [], style: {}, dataset: {}, attrs: {}, className: "", textContent: "", value: "", disabled: false, listeners: {}, removed: false };
  own.appendChild = (c) => { own.children.push(c); return c; };
  own.addEventListener = (type, fn) => { (own.listeners[type] = own.listeners[type] || []).push(fn); };
  own.setAttribute = (k, v) => { own.attrs[k] = String(v); };
  own.remove = () => { own.removed = true; };
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

function fakeDocument(state) {
  const byId = new Map();
  const kept = (k) => { if (!byId.has(k)) byId.set(k, fakeElement(k)); return byId.get(k); };
  return new Proxy({}, {
    get(_t, prop) {
      if (prop === "createElement") return (tag) => { const e = fakeElement(tag); state.created.push(e); return e; };
      if (prop === "getElementById") return (id) => kept(id);
      if (prop === "querySelector") return () => anything();
      if (prop === "querySelectorAll") return () => [];
      if (prop === "hidden") return false;
      return anything();
    },
    set: () => true,
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

// Just enough IndexedDB for chat-dm-store.js: requests answer on the next turn.
function fakeIndexedDB() {
  const stores = { msgs: new Map(), meta: new Map() };
  const req = (result) => {
    const r = { result: undefined, onsuccess: null, onerror: null };
    setImmediate(() => { r.result = result; if (r.onsuccess) r.onsuccess(); });
    return r;
  };
  const objectStore = (name) => ({
    get: (k) => req(stores[name].get(k)),
    put: (v) => { stores[name].set(v.k ?? v.scope, v); return req(undefined); },
    delete: (k) => { stores[name].delete(k); return req(undefined); },
    index: () => ({ getAll: (scope) => req([...stores[name].values()].filter((v) => v.scope === scope)) }),
  });
  const db = { objectStoreNames: { contains: () => true }, transaction: (name) => ({ objectStore: () => objectStore(name) }) };
  return { open: () => req(db) };
}

function fakeSocket() {
  return { readyState: 1, sent: [], raw: [], closed: false, send(s) { this.raw.push(s); this.sent.push(JSON.parse(s)); }, close() { this.closed = true; } };
}

function FakePeerConnection() {
  this.setRemoteDescription = async () => {};
  this.setLocalDescription = async () => {};
  this.createAnswer = async () => ({ type: "answer", sdp: "v=0 test-answer" });
  this.createOffer = async () => ({ type: "offer", sdp: "v=0 test-offer" });
  this.createDataChannel = () => ({ readyState: "connecting", send() {} });
  this.addTrack = () => {};
  this.addIceCandidate = async () => {};
  this.close = () => {};
}

const ME = "a1".repeat(32);
const ANN = "b2".repeat(32);  // a member
const BEN = "c3".repeat(32);  // an admin
const CY = "d4".repeat(32);   // the owner
const DEE = "e5".repeat(32);  // a moderator
const SERVER = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
const MY_KYBER = "my-kyber";
const kyberOf = (k) => (k === ME ? MY_KYBER : "kyber-" + k.slice(0, 4));
const ERASED_FLAG = "humanity_account_erased";

let cryptoInFlight = 0;
const countedSubtle = new Proxy(globalThis.crypto.subtle, {
  get(t, prop) {
    const v = t[prop];
    if (typeof v !== "function") return v;
    return (...args) => {
      cryptoInFlight++;
      return Promise.resolve(v.apply(t, args)).finally(() => { cryptoInFlight--; });
    };
  },
});
const countedCrypto = {
  subtle: countedSubtle,
  getRandomValues: (a) => globalThis.crypto.getRandomValues(a),
  randomUUID: () => globalThis.crypto.randomUUID(),
};

async function settle() {
  let idle = 0;
  for (let i = 0; i < 5000 && idle < 12; i++) {
    await new Promise((r) => setImmediate(r));
    idle = cryptoInFlight === 0 ? idle + 1 : 0;
  }
}

const b64 = (s) => Buffer.from(s, "utf8").toString("base64");
const unb64 = (s) => Buffer.from(s, "base64").toString("utf8");

// The chat page signed in as Me_1 with `myRole`, beside Ann (a member), Ben (an admin), Cy (the
// owner) and Dee (a moderator).
async function loadChat(opts = {}) {
  const state = { appended: [], created: [] };
  const serviceWorker = { register: () => Promise.resolve({ scope: "/" }), ready: new Promise(() => {}), controller: null, addEventListener() {} };
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: fakeDocument(state),
    navigator: new Proxy({}, { get: (_t, p) => (p === "serviceWorker" ? serviceWorker : anything()), has: () => true }),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", reload() {} },
    localStorage: opts.storage || fakeStorage(),
    sessionStorage: fakeStorage(),
    indexedDB: fakeIndexedDB(),
    fetch: (url) => (String(url).startsWith("/api/federation/servers")
      ? Promise.resolve({ ok: true, json: async () => [] })
      : Promise.reject(new Error("no network in tests"))),
    setTimeout: () => 0,
    clearTimeout: () => {},
    setInterval: () => 0,
    clearInterval: () => {},
    WebSocket: Object.assign(function () { return fakeSocket(); }, { OPEN: 1, CONNECTING: 0, CLOSED: 3 }),
    RTCPeerConnection: FakePeerConnection,
    RTCSessionDescription: function (d) { return d; },
    RTCIceCandidate: function (c) { return c; },
    Audio: function () { return anything(); },
    addEventListener: () => {},
    removeEventListener: () => {},
    matchMedia: () => anything(),
    requestAnimationFrame: () => 0,
    Notification: anything(),
    innerWidth: 1280,
    innerHeight: 800,
    crypto: countedCrypto,
    btoa: globalThis.btoa,
    atob: globalThis.atob,
    TextEncoder,
    TextDecoder,
    URLSearchParams,
    URL,
    prompt: () => { throw new Error("a browser prompt was used"); },
  };
  ctx.window = ctx;
  ctx.self = ctx;
  vm.createContext(ctx);
  const run = (rel) => vm.runInContext(fs.readFileSync(path.join(WEB, rel), "utf8"), ctx, { filename: rel });
  // In index.html's order.
  run("shared/events.js");
  run("shared/friend-pass.js");
  run("shared/reach.js");
  run("shared/block.js");
  run("shared/admin-erase.js");
  run("chat/crypto.js");
  run("chat/chat-dm-store.js");
  run("chat/view/timestampPill.js");
  run("chat/view/messageRow.js");
  run("chat/app.js");
  run("chat/chat-messages.js");
  run("chat/chat-dms.js");
  run("chat/chat-social.js");
  run("chat/chat-groups-p2p.js");
  run("chat/chat-ui.js");
  run("chat/chat-voice-rooms.js");
  run("chat/chat-voice-calls.js");
  run("chat/chat-voice-modal.js");
  run("chat/chat-privacy.js");
  run("chat/chat-p2p.js");
  for (const name of ["hosIcon", "holdToConfirm", "holdConfirm", "updateStats"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  ctx.appendMessage = (el) => { state.appended.push(el); };
  ctx.notifyNewMessage = () => {};
  ctx.playNotificationChime = () => {};
  ctx.sendSWNotification = () => {};
  const key = await globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  ctx.getDmStoreKey = async () => key;
  ctx.pqSignMessage = async (_secret, bytes) => new Uint8Array(bytes);
  ctx.pqVerifyMessage = async (_pk, bytes, sig) => Buffer.from(bytes).equals(Buffer.from(sig));
  ctx.pqDmSeal = async (pub, plaintext) => ({ ek_ct_b64: b64(pub), nonce_b64: "AAAA", ct_b64: b64(plaintext) });
  ctx.pqDmOpen = async (_secret, ek, _nonce, ct) => (unb64(ek) === MY_KYBER ? unb64(ct) : null);
  const sock = fakeSocket();
  vm.runInContext(`(s, me) => {
    ws = s; myKey = me; myName = 'Me_1'; activeChannel = 'general';
    myDilithiumPublicHex = me; myDilithiumSecret = new Uint8Array(4);
    myKyberPublicBase64 = '${MY_KYBER}'; myKyberSecret = new Uint8Array(4);
  }`, ctx)(sock, ME);
  const handle = async (msg) => { await vm.runInContext("handleMessage", ctx)(msg); await settle(); };
  await handle({ type: "identify_challenge", nonce: "ab", server_did: SERVER });
  const store = vm.runInContext("hosDmStore", ctx);
  assert.ok(await store.init(ME, "localhost"), "the store loads");
  store.setPassServer(SERVER);
  const users = [[ME, "Me_1", opts.myRole || ""], [ANN, "Ann", ""], [BEN, "Ben", "admin"], [CY, "Cy", "owner"], [DEE, "Dee", "mod"]]
    .map(([k, name, role]) => ({ public_key: k, name, role, kyber_public: kyberOf(k), online: true }));
  await handle({ type: "full_user_list", users });
  sock.sent.length = 0;
  sock.raw.length = 0;
  state.appended.length = 0;
  state.created.length = 0;
  const fn = (name) => vm.runInContext(name, ctx);
  const el = (id) => ctx.document.getElementById(id);
  return { ctx, sock, state, appended: state.appended, created: state.created, handle, fn, el };
}

function textOf(e) {
  if (!e) return "";
  const own = [e.textContent, typeof e.innerHTML === "string" ? e.innerHTML : ""].filter((s) => typeof s === "string").join(" ");
  return [own, ...(e.children || []).map(textOf)].join(" ");
}
const shown = (appended) => appended.map(textOf).join("\n");
const ev = { preventDefault() {}, stopPropagation() {}, clientX: 10, clientY: 10 };

// The member menu's HTML for this person, as I see it.
function menuFor(page, name, key) {
  page.fn("showUserContextMenu")(ev, name, key);
  return page.el("user-context-menu").innerHTML;
}

// The voice modal's buttons named "Erase their data", for this person.
function voiceEraseButtons(page, name, key) {
  const from = page.created.length;
  page.fn("openVoiceUserModal")(name, key);
  const made = page.created.slice(from);
  page.fn("closeVoiceUserModal")();
  return made.filter((e) => e.tag === "button" && e.textContent === ae.ADMIN_ERASE_LABEL);
}

// The confirm's parts, from what the page created for it.
function confirmParts(page, from = 0) {
  const made = page.created.slice(from);
  const pick = (f) => made.filter(f).pop();
  return {
    overlay: pick((e) => e.id === "admin-erase-overlay"),
    sentence: pick((e) => e.className === "admin-erase-what"),
    input: pick((e) => e.tag === "input" && e.id === "admin-erase-name"),
    send: pick((e) => e.tag === "button" && String(e.className).includes("admin-erase-send")),
    error: pick((e) => e.className === "admin-erase-error"),
    receipt: pick((e) => e.className === "admin-erase-receipt"),
    titles: made.filter((e) => e.tag === "h2").map((e) => e.textContent),
  };
}
const type = (input, text) => { input.value = text; for (const f of input.listeners.input || []) f({}); };
const press = (button) => { for (const f of button.listeners.click || []) f({}); };

// 10i of the spec, whitespace folded, for the words and frames it fixes.
function spec10i() {
  const spec = fs.readFileSync(path.join(ROOT, "docs", "design", "blocking-and-safe-mode.md"), "utf8");
  const start = spec.indexOf("## 10i.");
  assert.ok(start > 0, "the spec has a 10i");
  const end = spec.indexOf("\n## ", start + 4);
  return spec.slice(start, end > 0 ? end : undefined).replace(/\s+/g, " ");
}

test("the words and the frames are 10i's, and the pure rules hold", () => {
  const s = spec10i();
  const confirm = s.match(/"(This deletes everything[^"]*)"/);
  assert.ok(confirm, "10i has the confirm sentence");
  assert.equal(ae.adminEraseConfirmText("Ann"), confirm[1].replace("<name>", "Ann"), "the confirm sentence is 10i's, word for word");
  const words = s.match(/"(A server admin erased[^"]*)"/);
  assert.ok(words, "10i has the erased person's words");
  assert.equal(ae.ERASED_BY_ADMIN_WORDS, words[1], "the erased person's words are 10i's");
  assert.ok(ae.ERASED_BY_ADMIN_NOTE.startsWith(ae.ERASED_BY_ADMIN_WORDS), "the login note starts with them");

  // The frame: 10i's fields, in its order, nothing else.
  const frame = s.match(/`(\{"type":"admin_erase",[^`]*\})`/);
  assert.ok(frame, "10i has the admin_erase frame");
  const want = JSON.parse(frame[1]);
  const built = ae.adminEraseFrame(ANN, "Ann", "Ann");
  assert.deepEqual(Object.keys(built), Object.keys(want), "the frame's fields are 10i's");
  assert.equal(built.type, want.type);
  assert.equal(JSON.stringify(built), `{"type":"admin_erase","target":"${ANN}","confirm_name":"Ann"}`);
  const done = s.match(/`(\{"type":"admin_erase_done",[^`]*\})`/);
  assert.ok(done, "10i has the admin_erase_done frame");
  assert.deepEqual([...done[1].matchAll(/"(\w+)":/g)].map((m) => m[1]), ["type", "name", "receipt", "partial"], "the receipt frame read here is 10i's");
  assert.ok(/`by_admin: true`/.test(s), "10i names by_admin");

  // Who is offered it.
  const offered = (myRole, targetRole, targetKey = ANN) => ae.adminEraseOffered({ myRole, myKey: ME, targetKey, targetRole });
  for (const r of ["admin", "owner", "Admin", " owner "]) assert.equal(offered(r, ""), true, `an ${r} is offered it on a member's row`);
  for (const r of ["mod", "moderator", "", "user", "verified", "donor", "shipwright", null, undefined]) {
    assert.equal(offered(r, ""), false, `a ${r} is not offered it`);
  }
  for (const r of ["admin", "owner", "Admin"]) assert.equal(offered("admin", r), false, `never on an ${r}'s row`);
  assert.equal(offered("admin", "mod"), true, "a moderator's row is offered it");
  assert.equal(offered("owner", "", ME), false, "never on my own row");
  assert.equal(offered("owner", "", ME.toUpperCase()), false, "never on my own row, letter case aside");
  assert.equal(offered("admin", "", "bot_heron"), false, "never on a bot's row");
  assert.equal(offered("admin", "", ""), false, "never without a key");

  // The name check: trimmed, exact, letter case counts.
  assert.equal(ae.adminEraseNameMatches("Ann", "Ann"), true);
  assert.equal(ae.adminEraseNameMatches("  Ann \t", "Ann"), true, "trimmed");
  for (const bad of ["ann", "ANN", "An", "Ann1", "A nn", "", " "]) assert.equal(ae.adminEraseNameMatches(bad, "Ann"), false, JSON.stringify(bad));
  assert.equal(ae.adminEraseNameMatches("", ""), false, "no name, no match");
  assert.equal(ae.adminEraseNameMatches(null, "Ann"), false);
  assert.equal(ae.adminEraseFrame(ANN, "ann", "Ann"), null, "no frame for a name that does not match");
  assert.equal(ae.adminEraseFrame("", "Ann", "Ann"), null, "no frame without a key");
  assert.deepEqual(ae.adminEraseFrame(ANN, " Ann ", "Ann"), { type: "admin_erase", target: ANN, confirm_name: "Ann" }, "the typed name, trimmed");

  // Which note an account_erased leaves.
  assert.equal(ae.erasedKindOf({ type: "account_erased", partial: false, by_admin: true }), "admin");
  assert.equal(ae.erasedKindOf({ type: "account_erased", partial: false, by_admin: false }), "erased");
  assert.equal(ae.erasedKindOf({ type: "account_erased", partial: false }), "erased", "by_admin absent: the self-erase");
  assert.equal(ae.erasedKindOf({ type: "account_erased", partial: true, by_admin: true }), "unfinished", "an unfinished erase says so first");
});

test("Erase their data shows only for an admin or the owner, never on an admin's row or my own", async () => {
  for (const myRole of ["admin", "owner"]) {
    const page = await loadChat({ myRole });
    assert.ok(menuFor(page, "Ann", ANN).includes(ae.ADMIN_ERASE_LABEL), `an ${myRole} is offered it on a member's row (menu)`);
    assert.ok(menuFor(page, "Dee", DEE).includes(ae.ADMIN_ERASE_LABEL), `an ${myRole} is offered it on a moderator's row (menu)`);
    assert.ok(!menuFor(page, "Ben", BEN).includes(ae.ADMIN_ERASE_LABEL), "never on an admin's row (Ben)");
    assert.ok(!menuFor(page, "Cy", CY).includes(ae.ADMIN_ERASE_LABEL), "never on the owner's row (Cy)");
    assert.ok(!menuFor(page, "Me_1", ME).includes(ae.ADMIN_ERASE_LABEL), "never on my own row (menu)");
    assert.ok(menuFor(page, "Ann", ANN).includes("Unban"), `the ${myRole} sees the admin actions`);
    assert.equal(voiceEraseButtons(page, "Ann", ANN).length, 1, `an ${myRole} is offered it in the voice modal`);
    assert.equal(voiceEraseButtons(page, "Ben", BEN).length, 0, "never on an admin's row (voice modal)");
    assert.equal(voiceEraseButtons(page, "Cy", CY).length, 0, "never on the owner's row (voice modal)");
    assert.equal(voiceEraseButtons(page, "Me_1", ME).length, 0, "never on my own row (voice modal)");
  }
  for (const myRole of ["mod", ""]) {
    const page = await loadChat({ myRole });
    const menu = menuFor(page, "Ann", ANN);
    if (myRole === "mod") assert.ok(menu.includes("Kick"), "the moderator sees the mod actions");
    assert.ok(!menu.includes(ae.ADMIN_ERASE_LABEL), `a ${myRole || "member"} is not offered it (menu)`);
    assert.equal(voiceEraseButtons(page, "Ann", ANN).length, 0, `a ${myRole || "member"} is not offered it (voice modal)`);
    // Nor can the confirm be opened by calling it.
    const from = page.created.length;
    assert.equal(page.fn("openAdminEraseDialog")({ target: ANN, name: "Ann" }), null, "the confirm does not open");
    assert.equal(confirmParts(page, from).overlay, undefined, "nothing is drawn");
    assert.equal(page.sock.sent.length, 0, "nothing is sent");
  }
});

test("the confirm says 10i's sentence, and Erase needs the exact name", async () => {
  const page = await loadChat({ myRole: "admin" });
  menuFor(page, "Ann", ANN);
  page.fn("adminEraseFromCtx")();
  const c = confirmParts(page);
  assert.ok(c.overlay && c.input && c.send && c.sentence, "the confirm is drawn with its field and its Erase button");
  assert.ok(c.titles.includes(ae.ADMIN_ERASE_LABEL), "titled Erase their data");
  assert.equal(c.sentence.textContent, ae.adminEraseConfirmText("Ann"), "the sentence names Ann");
  assert.equal(c.send.textContent, "Erase");
  assert.equal(c.send.disabled, true, "Erase is disabled until the name is typed");
  for (const wrong of ["", "A", "An", "Ann1", "Ben"]) {
    type(c.input, wrong);
    assert.equal(c.send.disabled, true, `"${wrong}" keeps Erase disabled`);
  }
  type(c.input, "ann");
  assert.equal(c.send.disabled, true, "a name in the wrong letter case keeps Erase disabled");
  type(c.input, "ANN");
  assert.equal(c.send.disabled, true, "a name in the wrong letter case keeps Erase disabled");
  type(c.input, "  Ann ");
  assert.equal(c.send.disabled, false, "the name with spaces around it enables Erase (trimmed)");
  type(c.input, "Ann");
  assert.equal(c.send.disabled, false, "the exact name enables Erase");
  type(c.input, "Annie");
  assert.equal(c.send.disabled, true, "and a change away from it disables it again");

  // Even pressed some other way, a wrong name sends nothing and says why.
  type(c.input, "ann");
  assert.equal(page.fn("submitAdminEraseDialog")(), false);
  assert.equal(page.sock.sent.length, 0, "a wrong name sends nothing");
  assert.ok(String(c.error.textContent).includes("does not match"), "and the confirm says the name does not match");
  assert.equal(c.overlay.removed, false, "the confirm stays open");

  // Cancel closes it and sends nothing.
  page.fn("closeAdminEraseDialog")();
  assert.equal(c.overlay.removed, true, "Cancel closes the confirm");
  assert.equal(page.sock.sent.length, 0);
});

test("Erase sends exactly 10i's frame and nothing else", async () => {
  const page = await loadChat({ myRole: "owner" });
  // From the voice modal's button this time.
  const [btn] = voiceEraseButtons(page, "Dee", DEE);
  assert.ok(btn, "the voice modal offers it on a moderator's row");
  const from = page.created.length;
  press(btn);
  const c = confirmParts(page, from);
  assert.ok(c.input && c.send, "the button opens the confirm");
  assert.equal(c.sentence.textContent, ae.adminEraseConfirmText("Dee"));
  type(c.input, " Dee ");
  press(c.send);
  assert.equal(page.sock.raw.length, 1, "one frame");
  assert.equal(page.sock.raw[0], `{"type":"admin_erase","target":"${DEE}","confirm_name":"Dee"}`, "exactly 10i's frame");
  assert.deepEqual(page.sock.sent[0], { type: "admin_erase", target: DEE, confirm_name: "Dee" }, "exactly 10i's frame");
  assert.equal(c.overlay.removed, true, "the confirm closes");
  assert.ok(shown(page.appended).includes("Asked the server to erase Dee's data"), "and the chat says it was asked");
  assert.equal(page.fn("submitAdminEraseDialog")(), false, "a second press sends nothing more");
  assert.equal(page.sock.raw.length, 1);
});

test("admin_erase_done shows the receipt to the admin", async () => {
  const page = await loadChat({ myRole: "admin" });
  await page.handle({ type: "admin_erase_done", name: "Ann", receipt: [["messages", 5], ["profile", 1], ["uploads", 0]], partial: false });
  const text = "Erased Ann's data from this server (messages: 5, profile: 1). Anything on their own devices is untouched.";
  assert.equal(ae.adminEraseReceiptText({ name: "Ann", receipt: [["messages", 5], ["profile", 1], ["uploads", 0]], partial: false }), text);
  assert.ok(shown(page.appended).includes(text), "the receipt is shown in the chat");
  const c = confirmParts(page);
  assert.ok(c.receipt, "and in a dialog");
  assert.equal(c.receipt.textContent, text);
  assert.ok(c.titles.includes("Data erased"));

  // Nothing stored: said so. Part of it failed: said so, and to erase again.
  assert.equal(ae.adminEraseReceiptText({ name: "Ann", receipt: [], partial: false }), "Erased Ann's data from this server (nothing was stored). Anything on their own devices is untouched.");
  page.appended.length = 0;
  const from = page.created.length;
  await page.handle({ type: "admin_erase_done", name: "Ann", receipt: [["messages", 2], ["profile_FAILED", 1]], partial: true });
  const unfinished = "The erase of Ann's data did not finish: part of it failed (messages: 2, profile_FAILED: 1). Use Erase their data again to finish it.";
  assert.ok(shown(page.appended).includes(unfinished), "an unfinished erase says so in the chat");
  const d = confirmParts(page, from);
  assert.equal(d.receipt.textContent, unfinished);
  assert.ok(d.titles.includes("Erase not finished"));
  assert.equal(page.sock.sent.length, 0, "a receipt sends nothing");
});

test("account_erased by an admin shows the by-admin words; without by_admin, the old ones", async () => {
  const oldNote = "Your account on this server was erased, so pressing Enter signs you up again as a new account on this server.";
  const unfinishedNote = "The erase of your account on this server did not finish, so press Enter and use Erase account again.";

  // By an admin.
  const storage = fakeStorage();
  const page = await loadChat({ storage });
  assert.equal(page.fn("ERASED_ENTER_NOTE"), oldNote, "the old self-erase note is unchanged");
  await page.handle({ type: "account_erased", to: ME, partial: false, earlier: false, by_admin: true });
  const note = String(page.el("login-note").textContent);
  assert.ok(note.includes(ae.ERASED_BY_ADMIN_WORDS), "the by-admin words are on the login screen");
  assert.equal(note, ae.ERASED_BY_ADMIN_NOTE);
  assert.ok(!note.includes("Your account on this server was erased"), "instead of the self-erase words");
  assert.equal(storage.getItem(ERASED_FLAG), "admin", "kept for a reload");
  assert.equal(storage.getItem("humanity_name"), null, "the saved name is forgotten, as for the self-erase");
  assert.equal(page.sock.closed, true, "and the page leaves the server");
  assert.equal(page.fn("signUpAgainChoice")("enter", "admin"), true, "Enter there signs up again");
  assert.equal(page.fn("signUpAgainChoice")(undefined, "admin"), false, "an automatic connect never does");

  // A reload shows the same words.
  const again = await loadChat({ storage });
  assert.equal(again.el("login-note").textContent, ae.ERASED_BY_ADMIN_NOTE, "after a reload too");

  // The self-erase: by_admin absent, or false.
  for (const extra of [{}, { by_admin: false }]) {
    const s = fakeStorage();
    const p = await loadChat({ storage: s });
    await p.handle({ type: "account_erased", to: ME, partial: false, earlier: false, ...extra });
    const n = String(p.el("login-note").textContent);
    assert.equal(n, oldNote, `the self-erase words (${JSON.stringify(extra)})`);
    assert.ok(!n.includes(ae.ERASED_BY_ADMIN_WORDS), "and not the by-admin words");
    assert.equal(s.getItem(ERASED_FLAG), "erased");
  }

  // Unfinished: the unfinished note, by an admin or not.
  for (const by_admin of [false, true]) {
    const s = fakeStorage();
    const p = await loadChat({ storage: s });
    await p.handle({ type: "account_erased", to: ME, partial: true, earlier: false, by_admin });
    assert.equal(p.el("login-note").textContent, unfinishedNote, `partial (by_admin ${by_admin})`);
  }
});
