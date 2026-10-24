// Key cards in the website's chat (2026-10-22, docs/design/key-cards.md): web/chat/chat-key-cards.js
// with crypto.js, chat-dm-store.js and app.js, the browser twin of src/engine/key_cards.rs.
//
// Run: node --test scripts/tests/key-cards-chat-web.test.js   (in `just rig-tests`)
//
// The real chat scripts run in a node:vm page with real cards from the shared fixture
// (scripts/tests/fixtures/key-card-v1.json, made by the relay's builder) and the real Dilithium3
// check of the vendored noble bundle as window.pqVerifyMessage. The page's server is a recorded
// socket the test answers by hand. What is proved:
//   - a server-swapped key is refused: nothing is sealed to it, and the page says why;
//   - a good card is taken, whichever server sent it, and the pin is the identity's, so it still
//     seals after the page loads its store for another server;
//   - a newer card with a different key raises the change notice with Accept, and nothing is
//     sealed to that person until it is accepted;
//   - my own card is signed with my identity and rides the identify.
// HOS_WEB_DIR points the test at another copy of web/ (used to see each test red).

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const REPO = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(REPO, "web");
const FX = JSON.parse(fs.readFileSync(path.join(REPO, "scripts", "tests", "fixtures", "key-card-v1.json"), "utf8"));
const SIGNER = FX.card.key; // the person written to: the fixture's card is theirs
const ME_SEED = 41;

const enc = (s) => new TextEncoder().encode(s);

let NOBLE = null;
async function noble() {
  if (NOBLE) return NOBLE;
  const src = fs.readFileSync(path.join(REPO, "web", "shared", "vendor", "noble-pq.bundle.js"));
  NOBLE = await import("data:text/javascript;base64," + src.toString("base64"));
  return NOBLE;
}
async function keysOf(byte) {
  const m = await noble();
  const seed = new Uint8Array(32).fill(byte);
  const dil = m.ml_dsa65.keygen(m.blake3.create({ context: enc("hum/dilithium3/v1"), dkLen: 32 }).update(seed).digest());
  const kyb = m.ml_kem768.keygen(m.blake3.create({ context: enc("hum/kyber768/v1"), dkLen: 64 }).update(seed).digest());
  return { seed, dil, kyb, key: Buffer.from(dil.publicKey).toString("hex"), kyber: Buffer.from(kyb.publicKey).toString("base64") };
}

// ── A page, with just enough browser around it (the reach-web.test.js fakes) ──

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
function fakeElement(tag) {
  const own = { tag, children: [], style: {}, dataset: {}, className: "", textContent: "", disabled: false };
  own.appendChild = (c) => { own.children.push(c); return c; };
  return new Proxy(own, {
    get(t, prop) {
      if (prop === "then") return undefined;
      if (prop in t) return t[prop];
      return anything();
    },
    set(t, prop, v) { t[prop] = v; return true; },
  });
}
const fakeDocument = new Proxy({}, {
  get(_t, prop) { return prop === "createElement" ? fakeElement : anything(); },
  set: () => true,
});
function fakeStorage() {
  const m = new Map();
  return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k), clear: () => m.clear() };
}
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
  return { readyState: 1, sent: [], send(s) { this.sent.push(JSON.parse(s)); }, close() {} };
}
async function settle(n = 40) {
  for (let i = 0; i < n; i++) await new Promise((r) => setImmediate(r));
}
async function until(cond, what) {
  for (let i = 0; i < 2000; i++) {
    if (cond()) return;
    await new Promise((r) => setImmediate(r));
  }
  assert.fail(`never happened: ${what}`);
}

async function loadChat() {
  const m = await noble();
  const me = await keysOf(ME_SEED);
  const appended = [];
  const sealedTo = [];
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: fakeDocument,
    navigator: anything(),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", reload() {} },
    localStorage: fakeStorage(),
    sessionStorage: fakeStorage(),
    indexedDB: fakeIndexedDB(),
    fetch: () => Promise.reject(new Error("no network in tests")),
    setTimeout,
    clearTimeout,
    setInterval: () => 0,
    clearInterval: () => {},
    WebSocket: Object.assign(function () { return fakeSocket(); }, { OPEN: 1, CONNECTING: 0, CLOSED: 3 }),
    Audio: function () { return anything(); },
    addEventListener: () => {},
    removeEventListener: () => {},
    matchMedia: () => anything(),
    requestAnimationFrame: () => 0,
    Notification: anything(),
    crypto: globalThis.crypto,
    btoa: globalThis.btoa,
    atob: globalThis.atob,
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
  run("shared/friend-pass.js");
  run("shared/reach.js");
  run("shared/block.js");
  run("shared/key-card.js");
  run("chat/crypto.js");
  run("chat/chat-dm-store.js");
  run("chat/chat-key-cards.js");
  run("chat/app.js");
  run("chat/chat-dms.js");
  for (const name of ["updateStats", "renderServerList", "renderGroupList", "hosIcon", "playNotificationChime", "renderPresenceSidebarForActiveContext", "roleBadge", "isBlocked", "generateIdenticon", "switchSidebarTab", "renderChannelList", "isMobile", "sweepFriendPasses"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  ctx.appendMessage = (el) => { appended.push(el); };
  // The real post-quantum primitives where they matter: the check of every card is the vendored
  // noble Dilithium3 (pq.js's own call), my signatures are made with my real secret; sealing is
  // recorded (which key it went to is the question here).
  ctx.pqVerifyMessage = async (pub, msg, sig) => { try { return !!m.ml_dsa65.verify(sig, msg, pub); } catch (e) { return false; } };
  ctx.pqSignMessage = async (secret, bytes) => m.ml_dsa65.sign(bytes, secret);
  ctx.pqDmSeal = async (pub, plaintext) => { sealedTo.push(pub); return { ek_ct_b64: "AA==", nonce_b64: "AA==", ct_b64: Buffer.from(plaintext).toString("base64") }; };
  ctx.pqDeriveIdentity = async () => ({ dilithiumPublicHex: me.key, dilithiumSecret: me.dil.secretKey });
  ctx.pqDeriveKyber = async () => ({ kyberPublicBytes: me.kyb.publicKey, kyberSecret: me.kyb.secretKey });
  const key = await globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  ctx.getDmStoreKey = async () => key;
  vm.runInContext(`(s, me) => { ws = s; myKey = me; myName = 'Me_1'; myIdentity = { seed32: new Uint8Array(32).fill(${ME_SEED}) }; }`, ctx)(fakeSocket(), me.key);
  assert.ok(await vm.runInContext("attachPqIdentity", ctx)(), "my identity attaches");
  const store = vm.runInContext("hosDmStore", ctx);
  assert.ok(await store.init(me.key, "localhost"), "the store loads");
  const sock = vm.runInContext("ws", ctx);
  const handle = (msg) => vm.runInContext("handleMessage", ctx)(msg);
  const fn = (name) => vm.runInContext(name, ctx);
  return { ctx, me, store, sock, handle, fn, appended, sealedTo, m };
}

/** Start sealing a DM to `who`, answer the page's key_card_request with `cards`, and return what was built. */
async function sealWithAnswer(page, who, cards) {
  page.sock.sent.length = 0;
  const building = page.fn("pqBuildDmPuts")("hello", who, Date.now());
  await until(() => page.sock.sent.some((f) => f.type === "key_card_request"), "the page asks the server for the card");
  const ask = page.sock.sent.find((f) => f.type === "key_card_request");
  assert.deepEqual(ask.keys, [who], "it asks for exactly that person's card");
  await page.handle({ type: "key_cards", to: page.me.key, cards, missing: cards.length ? [] : [who] });
  return building;
}

// Seen red 2026-10-22 with chat-key-cards.js ingest taking a card on its shape alone, without
// HosKeyCard.cardFor (the old trust in what the server says): "no DM was built" failed.
test("a server-swapped key is refused: nothing is sealed to it, and the page says why", async () => {
  const page = await loadChat();
  const built = await sealWithAnswer(page, SIGNER, [FX.swapped]);
  assert.equal(built, null, "no DM was built");
  assert.ok(!page.sealedTo.includes(FX.swapped.kyber), "nothing was sealed to the swapped key");
  assert.equal(page.fn("getPeerEcdhPublic")(SIGNER), null, "no key for them");
  const kc = page.fn("hosKeyCards");
  assert.equal(kc.refusal(SIGNER), "no_card");
  assert.match(kc.refusalText(SIGNER), /^Nothing was sent: .* has no signed key card on this server/);
  // The old way in, a member list carrying a bare key, gives nothing either.
  await page.handle({ type: "full_user_list", users: [{ public_key: SIGNER, name: "Ann", role: "", kyber_public: FX.swapped.kyber, online: true }] });
  await settle();
  assert.equal(page.fn("getPeerEcdhPublic")(SIGNER), null, "a list's bare key gave them a key");
});

// Seen red 2026-10-22 with hosDmStore keeping the pins under the server's scope (`this.scope`)
// instead of the identity's: "the pin is the identity's" failed after the store loaded for
// another server.
test("a valid card is accepted from any server, and the pin is the identity's", async () => {
  const page = await loadChat();
  const built = await sealWithAnswer(page, SIGNER, [FX.card]);
  assert.ok(built && built.recipientPut, "the DM was built");
  assert.equal(page.sealedTo[0], FX.card.kyber, "sealed to the key in the card");
  assert.ok(await page.store.init(page.me.key, "other.example"), "the store loads for another server");
  assert.equal(page.fn("getPeerEcdhPublic")(SIGNER), FX.card.kyber, "the pin is the identity's, so it seals there too");
  // A card that arrives from that server, the same card, changes nothing.
  assert.equal(await page.fn("hosKeyCards").ingest(FX.card), "same");
});

// Seen red 2026-10-22 with HosKeyCard.sealingKey ignoring `changes`: "nothing is sealed to them
// while the change waits" failed.
test("a changed key raises the change notice, and nothing is sealed until it is accepted", async () => {
  const page = await loadChat();
  await sealWithAnswer(page, SIGNER, [FX.card]);
  const signer = await keysOf(FX.signer_master_byte);
  const K = page.fn("HosKeyCard");
  const newer = await K.signCard(SIGNER, FX.other_card.kyber, FX.card.at + 10, (bytes) => page.m.ml_dsa65.sign(bytes, signer.dil.secretKey));
  vm.runInContext("(k) => { activeDmPartner = k; }", page.ctx)(SIGNER);
  page.appended.length = 0;
  await page.handle({ type: "key_cards", to: page.me.key, cards: [newer], missing: [] });
  await settle();
  const kc = page.fn("hosKeyCards");
  assert.equal(page.fn("getPeerEcdhPublic")(SIGNER), null, "nothing is sealed to them while the change waits");
  assert.equal(kc.refusal(SIGNER), "changed");
  const notice = page.appended.find((el) => el.className === "message system key-changed");
  assert.ok(notice, "the change notice is shown in their conversation");
  assert.equal(notice.children[0].textContent, FX.refusals.notice.replace(/Ann/g, kc._name(SIGNER)));
  const sealsBefore = page.sealedTo.length;
  assert.equal(await page.fn("pqBuildDmPuts")("hi", SIGNER, Date.now()), null, "a DM is refused");
  assert.equal(page.sealedTo.length, sealsBefore, "and nothing was sealed");
  const accept = notice.children[1];
  assert.equal(accept.textContent, "Accept the new key");
  accept.onclick();
  assert.equal(page.fn("getPeerEcdhPublic")(SIGNER), FX.other_card.kyber, "the accepted key is sealed to");
  const built = await page.fn("pqBuildDmPuts")("hi", SIGNER, Date.now());
  assert.ok(built && page.sealedTo.includes(FX.other_card.kyber));
});

test("my own card is signed with my identity and rides the identify", async () => {
  const page = await loadChat();
  const card = page.fn("myKeyCard");
  const K = page.fn("HosKeyCard");
  assert.ok(await K.cardFor(card, page.me.key, async (p, m, s) => !!page.m.ml_dsa65.verify(s, m, p)), "my card checks out as mine");
  assert.equal(card.kyber, page.me.kyber, "with my Kyber768 key");
  vm.runInContext("ws = null", page.ctx);
  page.fn("openSocket")({});
  const sock = page.fn("ws");
  sock.onopen();
  const identify = sock.sent.find((f) => f.type === "identify");
  assert.ok(identify, "an identify went out");
  assert.deepEqual(identify.key_card, card, "carrying my key card");
  assert.equal(identify.kyber_public, undefined, "and no bare key");
});
