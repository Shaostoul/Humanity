// "Who can reach me" in the web chat client (step B, 2026-10-09,
// docs/design/blocking-and-safe-mode.md 10c), mirroring native Settings > Safety.
//
// Run: node --test scripts/tests/reach-web.test.js   (in `just rig-tests`)
//
// The page scripts run as they do in the browser, in one shared global scope (node:vm), with
// the DOM replaced by a stub, as in friend-pass-web.test.js: the real friend-pass.js, reach.js,
// crypto.js, chat-dm-store.js (over a stand-in IndexedDB), app.js, chat-dms.js, chat-social.js
// and chat-privacy.js. Only the Dilithium and Kyber primitives are stand-ins: "signing" returns
// the signed words and "checking" compares them; "sealing" base64s the plaintext and names the
// key it was sealed to, so a test can read exactly what would travel inside the ciphertext.
// The real primitives are covered by scripts/pq-kat.mjs and the relay's tests.
//
// What it proves:
//  0. The words and the contact request's size: the first padding bucket is the same in the
//     relay's Rust (src/net/dm_pq.rs), the web sealer (crypto.js) and reach.js, and a request's
//     plaintext is exactly that long whatever the name; the five audiences decide as 10c says.
//  1. `reach_settings` from the relay is what the Safety page shows; before it arrives the rows
//     show the safe defaults and cannot be changed.
//  2. Changing a row sends `reach_set` with only that kind, and the row shows the relay's answer.
//  3. A `reach_refused` for a message shows the spec's sentence and a Send request button, once a
//     minute per person; the button sends a contact request.
//  4. A contact request goes out flagged, sealed at the smallest bucket, carrying only my name.
//  5. A DM from someone my settings refuse is shown as a request by name, its text stored and
//     shown nowhere; a contact request is listed by name; "nobody" lists neither; "groups" lets
//     in someone who shares a P2P group and no one else; Accept follows back and gives my pass;
//     Ignore sends nothing.
//  6. "People who may call me": adding a friend re-issues their pass with `call`, removing
//     re-issues without it, and each time the old serial is withdrawn after the new pass went.
//
// Red first, 2026-10-09 (each mutation made, run, seen failing, put back):
//  0: crypto.js DM_PAD_BUCKETS starting at 512 failed "the first bucket is the same everywhere".
//  1: onReachSettings without `reachKnown = reachSettingsFrom(settings)` failed "reach_settings
//     drives what the Safety page shows" (the rows stayed on the defaults, disabled).
//  2: chooseReachAudience sending the whole current set instead of the one kind failed
//     "a changed row sends reach_set with only that kind".
//  3: the `reach_refused` branch taken out of chat-privacy.js's handleMessage failed
//     "a refusal offers a contact request" (no sentence, no button).
//  4: CONTACT_REQUEST_BYTES set to 1024 in reach.js failed "a contact request goes out at the
//     smallest bucket" (and test 0).
//  5: the reachScreenDm call taken out of app.js's dm_new case failed "a DM my settings refuse
//     becomes a request with no text" (the text was stored in the conversation); and
//     reachSharesGroupWith answering false for a loaded group list failed it too ("a shared group
//     lets it through").
//  6: sendPendingWithdrawals() taken out of chat-social.js reissuePassTo failed "the call list
//     re-issues the pass" (no cert_revoke for the old serial).

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");
const ROOT = path.join(__dirname, "..", "..");
const reach = require(path.join(WEB, "shared", "reach.js"));
const fp = require(path.join(WEB, "shared", "friend-pass.js"));

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

// An element that keeps what is written to it (text, children, handlers) and absorbs the rest.
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
  return { readyState: 1, sent: [], send(s) { this.sent.push(JSON.parse(s)); }, close() {} };
}

const ME = "a1".repeat(32);
const ANN = "b2".repeat(32);
const BEN = "c3".repeat(32);
const CY = "d4".repeat(32);
const SERVER = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
const CTL_FOLLOW = "[[hum:follow]]";
const CTL_FRIEND_CERT = "[[hum:friend-cert]]";
const MY_KYBER = "my-kyber";
const kyberOf = (k) => "kyber-" + k.slice(0, 4);

async function settle() {
  for (let i = 0; i < 12; i++) await new Promise((r) => setImmediate(r));
}

const b64 = (s) => Buffer.from(s, "utf8").toString("base64");
const unb64 = (s) => Buffer.from(s, "base64").toString("utf8");

async function loadChat() {
  const appended = [];
  const notified = [];
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: fakeDocument,
    navigator: anything(),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", reload() {} },
    localStorage: fakeStorage(),
    sessionStorage: fakeStorage(),
    indexedDB: fakeIndexedDB(),
    fetch: () => Promise.reject(new Error("no network in tests")),
    setTimeout: () => 0,
    clearTimeout: () => {},
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
  run("chat/crypto.js");
  run("chat/chat-dm-store.js");
  run("chat/app.js");
  run("chat/chat-dms.js");
  for (const name of ["updateStats", "renderServerList", "renderGroupList", "hosIcon", "playNotificationChime", "renderPresenceSidebarForActiveContext", "roleBadge", "isBlocked", "generateIdenticon", "switchSidebarTab", "renderChannelList", "isMobile"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  ctx.notifyNewMessage = (...args) => { notified.push(args); };
  run("chat/chat-social.js");
  run("chat/chat-privacy.js");
  // Record what is drawn into the message area (system lines and the refusal offer).
  ctx.appendMessage = (el) => { appended.push(el); };
  // The stand-in primitives (see the top of this file).
  const key = await globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  ctx.getDmStoreKey = async () => key;
  ctx.pqSignMessage = async (_secret, bytes) => new Uint8Array(bytes);
  ctx.pqVerifyMessage = async (_pk, bytes, sig) => Buffer.from(bytes).equals(Buffer.from(sig));
  ctx.pqDmSeal = async (pub, plaintext) => ({ ek_ct_b64: b64(pub), nonce_b64: "AAAA", ct_b64: b64(plaintext) });
  ctx.pqDmOpen = async (_secret, ek, _nonce, ct) => (unb64(ek) === MY_KYBER ? unb64(ct) : null);
  vm.runInContext(`(s, me) => {
    ws = s; myKey = me; myName = 'Me_1';
    myDilithiumPublicHex = me; myDilithiumSecret = new Uint8Array(4);
    myKyberPublicBase64 = '${MY_KYBER}'; myKyberSecret = new Uint8Array(4);
  }`, ctx)(fakeSocket(), ME);
  const sock = vm.runInContext("ws", ctx);
  const handle = (msg) => vm.runInContext("handleMessage", ctx)(msg);
  // The relay's challenge names the server; the member list carries names and DM keys.
  await handle({ type: "identify_challenge", nonce: "ab", server_did: SERVER });
  const store = vm.runInContext("hosDmStore", ctx);
  assert.ok(await store.init(ME, "localhost"), "the store loads");
  const users = [[ANN, "Ann"], [BEN, "Ben"], [CY, "Cy"]].map(([k, name]) => ({ public_key: k, name, role: "", kyber_public: kyberOf(k) }));
  await handle({ type: "full_user_list", users });
  await settle();
  sock.sent.length = 0;
  appended.length = 0;
  const fn = (name) => vm.runInContext(name, ctx);
  return { ctx, sock, store, appended, notified, handle, fn };
}

// Everything readable in a recorded element and its children.
function textOf(el) {
  if (!el) return "";
  const own = [el.textContent, typeof el.innerHTML === "string" ? el.innerHTML : ""].filter((s) => typeof s === "string").join(" ");
  return [own, ...(el.children || []).map(textOf)].join(" ");
}
function findButton(el, label) {
  if (!el) return null;
  if (el.tag === "button" && el.textContent === label) return el;
  for (const c of el.children || []) { const b = findButton(c, label); if (b) return b; }
  return null;
}

// What a sealed dm_put carries (the stand-in seal is base64 of the plaintext).
function opened(put) {
  const env = JSON.parse(put.content);
  return { sealedTo: unb64(env.ek_ct_b64), plain: unb64(env.ct_b64) };
}

// A DM as a sender's client seals it to me (signed with the stand-in signer).
function dmEnvelope(from, text, ts = 1760000000000) {
  const sig = b64(`hum/dm/v2\n${from}\n${ME}\n${ts}\n${text}`);
  const inner = JSON.stringify({ v: 2, from, to: ME, ts, text, sig });
  return JSON.stringify({ v: 2, ek_ct_b64: b64(MY_KYBER), nonce_b64: "AAAA", ct_b64: b64(inner) });
}
function requestEnvelope(name) {
  return JSON.stringify({ v: 2, ek_ct_b64: b64(MY_KYBER), nonce_b64: "AAAA", ct_b64: b64(reach.contactRequestJson(name)) });
}

test("the first bucket is the same everywhere, and a request's plaintext is exactly that long", async () => {
  const rust = fs.readFileSync(path.join(ROOT, "src", "net", "dm_pq.rs"), "utf8");
  const m = rust.match(/DM_PAD_BUCKETS:\s*\[usize;\s*\d+\]\s*=\s*\[([^\]]+)\]/);
  assert.ok(m, "src/net/dm_pq.rs still declares DM_PAD_BUCKETS");
  const rustBuckets = m[1].split(",").map((s) => Number(s.trim()));
  const { ctx } = await loadChat();
  const webBuckets = Array.from(vm.runInContext("DM_PAD_BUCKETS", ctx));
  assert.deepEqual(webBuckets, rustBuckets, "web and native pad to the same buckets");
  assert.equal(reach.CONTACT_REQUEST_BYTES, rustBuckets[0], "a contact request is the smallest bucket");

  for (const name of ["A", "Me_1", "x".repeat(24), "a-b_C9"]) {
    const json = reach.contactRequestJson(name);
    assert.equal(Buffer.byteLength(json, "utf8"), 256, `exactly 256 bytes for "${name}"`);
    assert.deepEqual(Object.keys(JSON.parse(json)).sort(), ["contact_request", "name", "pad", "v"], "a name and nothing else");
    assert.deepEqual(reach.contactRequestParse(json), { name });
  }
  for (const bad of ["", "two words", "x".repeat(25), "café", "a.b", "http://x", null]) {
    assert.equal(reach.contactRequestJson(bad), null, `"${bad}" is not a registered-name word`);
  }
  assert.equal(reach.contactRequestParse(JSON.stringify({ v: 2, contact_request: true, name: "Ann", pad: " ".repeat(300) })), null, "longer than the bucket is not a request");
  assert.equal(reach.contactRequestParse(JSON.stringify({ v: 2, name: "Ann" })), null, "no flag, no request");
  assert.equal(reach.contactRequestParse(JSON.stringify({ v: 2, contact_request: true, name: "see my site" })), null, "a sentence is not a name");

  // The five audiences (10c).
  const who = (passMay, sharesGroup) => ({ passMay, sharesGroup });
  const table = [
    ["nobody", who("call,message", true), false],
    ["chosen", who(null, true), false],
    ["chosen", who("invite,message,trade,voice_message", false), false], // a pass without call
    ["chosen", who("call,invite,message", false), true],
    ["friends", who(null, true), false],
    ["friends", who("invite,message", false), true],
    ["groups", who(null, false), false],
    ["groups", who(null, true), true],
    ["groups", who("message", false), true],
    ["anyone", who(null, false), true],
    ["everyone", who("call", true), false], // an unknown word reaches no one
  ];
  for (const [audience, w, want] of table) {
    assert.equal(reach.reachAllows(audience, "call", w), want, `${audience} ${JSON.stringify(w)}`);
  }
  assert.deepEqual(reach.reachSettingsFrom(null), { message: "friends", call: "chosen", trade: "friends" }, "the safe defaults");
  assert.deepEqual(reach.reachSettingsFrom({ message: "anyone", call: "sometimes" }), { message: "anyone", call: "chosen", trade: "friends" });
  assert.equal(reach.reachSetFrame({ message: "anyone", bogus: "friends" }), null, "an unknown kind refuses the whole set");
  assert.equal(reach.reachSetFrame({ message: "everyone" }), null, "an unknown audience refuses the whole set");
  assert.equal(reach.reachSetFrame({}), null);
});

test("reach_settings drives what the Safety page shows", async () => {
  const { store, handle, fn } = await loadChat();
  const model = fn("safetyModel");
  const html = fn("safetyPanelHtml");

  let m = model();
  assert.equal(m.known, false);
  assert.deepEqual(m.rows.map((r) => [r.kind, r.label, r.audience, r.disabled]), [
    ["message", "Messages", "friends", true], ["call", "Calls", "chosen", true], ["trade", "Trades", "friends", true],
  ], "before the relay speaks: the safe defaults, not changeable");
  assert.ok(html(m).includes("Waiting for this server"), "and the page says why");

  await handle({ type: "reach_settings", settings: { message: "anyone", call: "friends", trade: "nobody" } });
  m = model();
  assert.equal(m.known, true);
  assert.deepEqual(m.rows.map((r) => [r.kind, r.audience, r.disabled]), [
    ["message", "anyone", false], ["call", "friends", false], ["trade", "nobody", false],
  ], "the relay's word is what the rows show");
  for (const r of m.rows) {
    assert.equal(r.explain, reach.reachExplain(r.kind, r.audience), "each row explains its choice");
    assert.deepEqual(r.options.map((o) => o.label), ["Nobody", "People I choose", "Friends", "Friends and people in my groups", "Anyone"]);
  }
  const page = html(m);
  assert.ok(!page.includes("Waiting for this server"));
  const rowHtml = (kind) => page.slice(page.indexOf(`data-kind="${kind}"`), page.indexOf("</select>", page.indexOf(`data-kind="${kind}"`)));
  assert.ok(rowHtml("message").includes('<option value="anyone" selected>'), "Messages shows Anyone");
  assert.ok(rowHtml("call").includes('<option value="friends" selected>'), "Calls shows Friends");
  assert.ok(rowHtml("trade").includes('<option value="nobody" selected>'), "Trades shows Nobody");
  assert.ok(!rowHtml("message").includes(" disabled"), "and they can be changed");
  assert.ok(page.includes('so this list is used only when Calls is set to "People I choose"'), "the call list says when it applies");

  // The call list comes from the passes I gave: Ann's includes call, Ben's does not.
  store.recordPassSent(ANN, "aa".repeat(16), "call,invite,message,trade,voice_message");
  store.recordPassSent(BEN, "bb".repeat(16), "invite,message,trade,voice_message");
  m = model();
  assert.deepEqual(m.callers.map((c) => c.name), ["Ann"]);
  assert.deepEqual(m.others.map((c) => c.name), ["Ben"]);
  assert.ok(html(m).includes(`data-call-remove="${ANN}"`) && html(m).includes(`<option value="${BEN}">Ben</option>`));
});

test("a changed row sends reach_set with only that kind, and shows the answer", async () => {
  const { sock, handle, fn } = await loadChat();
  const choose = fn("chooseReachAudience");
  assert.equal(choose("call", "friends"), false, "nothing is sent before the relay has said what the settings are");
  assert.equal(sock.sent.length, 0);

  await handle({ type: "reach_settings", settings: { message: "friends", call: "chosen", trade: "friends" } });
  assert.equal(choose("call", "friends"), true);
  assert.deepEqual(sock.sent, [{ type: "reach_set", settings: { call: "friends" } }], "the exact 10c frame, one kind");
  let row = fn("safetyModel")().rows.find((r) => r.kind === "call");
  assert.deepEqual([row.audience, row.saving], ["friends", true], "the row shows the choice while it is saved");

  await handle({ type: "reach_settings", settings: { message: "friends", call: "friends", trade: "friends" } });
  row = fn("safetyModel")().rows.find((r) => r.kind === "call");
  assert.deepEqual([row.audience, row.saving], ["friends", false], "then the relay's answer");

  assert.equal(choose("call", "everyone"), false, "an unknown audience is not sent");
  assert.equal(choose("voice", "anyone"), false, "nor an unknown kind");
  assert.equal(sock.sent.length, 1);
});

test("a refusal offers a contact request, and the button sends one", async () => {
  const { sock, appended, handle } = await loadChat();
  await handle({ type: "reach_refused", kind: "message", to: CY });
  const offer = appended.find((el) => textOf(el).includes(reach.REACH_REFUSED_MESSAGE));
  assert.ok(offer, "the spec's sentence is shown");
  assert.ok(textOf(offer).includes("Not delivered to Cy."), "it says who it did not reach");
  const button = findButton(offer, "Send request");
  assert.ok(button, "with a Send request button");

  await handle({ type: "reach_refused", kind: "message", to: CY });
  assert.equal(appended.filter((el) => textOf(el).includes(reach.REACH_REFUSED_MESSAGE)).length, 1, "once a minute per person");

  await button.onclick();
  const put = sock.sent.find((m) => m.type === "dm_put" && m.contact_request === true);
  assert.ok(put && put.to === CY, "pressing it sends a contact request to them");
  assert.equal(button.textContent, "Request sent");

  await handle({ type: "reach_refused", kind: "trade", to: ANN });
  assert.ok(appended.some((el) => textOf(el).includes(reach.REACH_REFUSED_TRADE)), "a refused trade says so");
});

test("a contact request goes out at the smallest bucket, flagged, carrying only my name", async () => {
  const { sock, store, fn } = await loadChat();
  assert.equal(await fn("sendContactRequest")(ANN), true);
  const puts = sock.sent.filter((m) => m.type === "dm_put");
  assert.equal(puts.length, 1, "one deposit: no self-copy, nothing in it is history");
  const put = puts[0];
  assert.deepEqual(Object.keys(put).sort(), ["contact_request", "content", "to", "type"], "no pass, no other field");
  assert.equal(put.contact_request, true);
  assert.equal(put.to, ANN);
  const { sealedTo, plain } = opened(put);
  assert.equal(sealedTo, kyberOf(ANN), "sealed to their DM key");
  assert.equal(Buffer.byteLength(plain, "utf8"), 256, "the plaintext is the smallest bucket exactly");
  const inner = JSON.parse(plain);
  assert.deepEqual(Object.keys(inner).sort(), ["contact_request", "name", "pad", "v"], "my name and nothing else: no key, no text, no signature");
  assert.equal(inner.name, "Me_1");
  assert.equal(inner.pad.trim(), "");
  assert.ok(store.following.has(ANN), "I follow them from here, so their Accept completes the friendship");

  // The recipient's client reads it back as a request, never as a message.
  const back = await vm.runInContext("pqOpenDmEnvelope", (await loadChat()).ctx)(put.content.replace(b64(kyberOf(ANN)), b64(MY_KYBER)));
  assert.deepEqual(back, { contact_request: true, name: "Me_1" });
});

test("a DM my settings refuse becomes a request with no text; Accept and Ignore", async () => {
  const { ctx, sock, store, appended, notified, handle, fn } = await loadChat();
  await handle({ type: "reach_settings", settings: { message: "friends", call: "chosen", trade: "friends" } });
  store.recordPassSent(ANN, "aa".repeat(16), "invite,message,trade,voice_message"); // Ann is a friend

  const SECRET = "meet me at the old mill, here is my number";
  await handle({ type: "dm_new", id: 1, content: dmEnvelope(CY, SECRET) });
  await settle();
  assert.deepEqual(store.conversation(CY), [], "the text is not kept");
  const listed = store.contactRequestList();
  assert.deepEqual(listed.map((r) => [r.id, r.key, r.name]), [[CY, CY, "Cy"]], "Cy is listed by name");
  const shown = [...appended.map(textOf), ...notified.map((a) => a.join(" ")), JSON.stringify(store.contactRequests), fn("contactRequestsSidebarHtml")()].join("\n");
  assert.ok(!shown.includes("old mill"), "and the text is shown nowhere");
  assert.ok(fn("contactRequestsSidebarHtml")().includes(`data-req-accept="${CY}"`), "the DMs tab lists it with Accept");

  await handle({ type: "dm_new", id: 2, content: dmEnvelope(ANN, "hi from Ann") });
  await settle();
  assert.equal(store.conversation(ANN).length, 1, "a friend's DM is kept as before");

  // A contact request (name only) is listed under the member it names.
  await handle({ type: "dm_new", id: 3, content: requestEnvelope("Ben") });
  await settle();
  assert.deepEqual(store.contactRequestList().map((r) => r.key).sort(), [BEN, CY].sort());

  // Ignore: gone, nothing sent.
  sock.sent.length = 0;
  fn("ignoreContactRequest")(CY);
  assert.deepEqual(store.contactRequestList().map((r) => r.key), [BEN]);
  assert.equal(sock.sent.length, 0, "Ignore tells no one");

  // Accept: follow back, which (their request counting as their follow) makes us friends and gives my pass.
  assert.equal(await fn("acceptContactRequest")(BEN), true);
  await settle();
  const toBen = sock.sent.filter((m) => m.type === "dm_put" && m.to === BEN).map((m) => JSON.parse(opened(m).plain).text);
  assert.ok(toBen.includes(CTL_FOLLOW), "Accept follows them back");
  assert.ok(toBen.includes(CTL_FRIEND_CERT), "and gives them my pass");
  assert.ok(store.certSentTo(BEN));
  assert.deepEqual(store.contactRequestList(), []);

  // "Nobody": not even a request.
  await handle({ type: "reach_settings", settings: { message: "nobody", call: "chosen", trade: "friends" } });
  await handle({ type: "dm_new", id: 4, content: requestEnvelope("Cy") });
  await handle({ type: "dm_new", id: 5, content: dmEnvelope(CY, "again") });
  await settle();
  assert.deepEqual(store.contactRequestList(), [], "nobody lists no request");
  assert.deepEqual(store.conversation(CY), [], "and keeps no text");

  // "Groups": someone who shares a P2P group with me gets through; someone who does not is a request.
  await handle({ type: "reach_settings", settings: { message: "groups", call: "chosen", trade: "friends" } });
  ctx._p2pGroups = [{ group_id: "g1", members: [ME, ANN] }];
  await handle({ type: "dm_new", id: 6, content: dmEnvelope(CY, "not in my groups") });
  await settle();
  assert.deepEqual(store.conversation(CY), [], "no shared group: no text kept");
  assert.deepEqual(store.contactRequestList().map((r) => r.key), [CY], "but a request");
  ctx._p2pGroups = [{ group_id: "g2", members: [ME, CY] }];
  await handle({ type: "dm_new", id: 7, content: dmEnvelope(CY, "we share a group") });
  await settle();
  assert.equal(store.conversation(CY).length, 1, "a shared group lets it through");

  // "Anyone": a stranger's DM is a message.
  const DAN = "e5".repeat(32);
  await handle({ type: "reach_settings", settings: { message: "anyone", call: "chosen", trade: "friends" } });
  await handle({ type: "dm_new", id: 8, content: dmEnvelope(DAN, "hello from Dan") });
  await settle();
  assert.equal(store.conversation(DAN).length, 1, "anyone lets it through");
});

test("the call list re-issues the pass, then withdraws the old serial", async () => {
  const { sock, store, handle, fn } = await loadChat();
  const OLD = "00112233445566778899aabbccddeeff";
  store.recordPassSent(ANN, OLD, "invite,message,trade,voice_message");
  assert.equal(fn("friendMayCall")(ANN), false);

  assert.equal(await fn("setFriendMayCall")(ANN, true), true);
  const passOut = sock.sent.findIndex((m) => m.type === "dm_put" && m.to === ANN);
  const revokeOut = sock.sent.findIndex((m) => m.type === "cert_revoke" && m.serial === OLD);
  assert.ok(passOut >= 0, "a new pass goes to Ann");
  const sent = JSON.parse(opened(sock.sent[passOut]).plain);
  assert.equal(sent.text, CTL_FRIEND_CERT);
  const pass = fp.friendPassParse(sent.cert);
  assert.equal(pass.may, "call,invite,message,trade,voice_message", "everything she had, plus call");
  assert.notEqual(pass.serial, OLD, "a new serial");
  assert.ok(revokeOut > passOut, "the old serial is withdrawn, after the new pass went");
  assert.deepEqual(store.certsSent[ANN], [{ serial: pass.serial, may: pass.may }]);
  assert.ok(fn("friendMayCall")(ANN));

  // Off again: re-issued without call, the call pass withdrawn.
  sock.sent.length = 0;
  assert.equal(await fn("setFriendMayCall")(ANN, false), true);
  const again = fp.friendPassParse(JSON.parse(opened(sock.sent.find((m) => m.type === "dm_put" && m.to === ANN)).plain).cert);
  assert.equal(again.may, "invite,message,trade,voice_message");
  assert.ok(sock.sent.some((m) => m.type === "cert_revoke" && m.serial === pass.serial));
  assert.equal(fn("friendMayCall")(ANN), false);

  // Another device of mine still listing a withdrawn serial drops it when the relay confirms.
  store.recordPassSent(ANN, pass.serial, pass.may);
  await handle({ type: "cert_revoked", to: ME, serial: pass.serial });
  assert.deepEqual(store.certsSent[ANN].map((p) => p.serial), [again.serial]);
  assert.equal(await fn("setFriendMayCall")(CY, true), false, "only for someone I gave a pass");
});
