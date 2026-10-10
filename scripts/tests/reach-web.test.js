// "Who can reach me" in the web chat client (step B, 2026-10-09,
// docs/design/blocking-and-safe-mode.md 10c, contact requests as amended in review the same day),
// mirroring native Settings > Safety.
//
// Run: node --test scripts/tests/reach-web.test.js   (in `just rig-tests`)
//
// The page scripts run as they do in the browser, in one shared global scope (node:vm), with
// the DOM replaced by a stub, as in friend-pass-web.test.js: the real friend-pass.js, reach.js,
// block.js, crypto.js, chat-dm-store.js (over a stand-in IndexedDB), app.js, chat-dms.js, chat-social.js
// and chat-privacy.js. Only the Dilithium and Kyber primitives are stand-ins: "signing" returns
// the signed words and "checking" compares them; "sealing" base64s the plaintext and names the
// key it was sealed to, so a test can read exactly what would travel inside the ciphertext.
// The real primitives are covered by scripts/pq-kat.mjs and the relay's tests.
//
// What it proves:
//  0. The words: the padding buckets are the same in the relay's Rust (src/net/dm_pq.rs) and the
//     web sealer (crypto.js); a contact request's text is the marker and {name, pass}; the five
//     audiences decide as 10c says.
//  1. `reach_settings` from the relay is what the Safety page shows; before it arrives the rows
//     show the safe defaults and cannot be changed.
//  2. Changing a row sends `reach_set` with only that kind, and the row shows the relay's answer.
//  3. A `reach_refused` for a message shows the spec's sentence and a Send request button, once a
//     minute per person; the button sends a contact request.
//  4. A contact request goes out as an ordinary signed, sealed, padded DM flagged
//     `contact_request`, carrying my name and my pass for them (the default `may`, this server),
//     with a self-copy; from then on they hold my pass and I follow them.
//  5. Receiving: a request whose pass checks is listed under the name the member list has for the
//     signed sender (never the claimed one) and its text is kept nowhere; one whose pass does not
//     check is dropped; Accept follows back and sends my pass, each put presenting the
//     requester's pass as `friend_cert`; Ignore sends nothing; my own request echoed from another
//     device records the pass I gave. A DM from someone my settings refuse is listed by name with
//     its text dropped; "nobody" lists neither; under "groups" a DM the server let through is
//     kept whatever this page's own group list says (10m R8: membership is the server's call).
//     And window.peerData is the member list app.js keeps.
//  6. "People I choose" (10c-ii, 2026-10-10, which replaced "People who may call me"): a friend
//     with no choice gets the default `may`; unticking Message drops exactly `message`, `invite`
//     and `voice_message`; ticking Call adds only `call`; all three unticked still gives a valid
//     pass, `invite` alone; each tick re-issues the pass and withdraws the old serial after the
//     new pass went; Unfollow and Block clear the choice, so a friendship begun again starts from
//     the defaults; the "In use now" line for every mix of row settings; the page lists each
//     friend once with three ticks, wired, held still while their pass is minted.
//  7. 10l (2026-10-10): a re-issued pass (a tick) and a contact request's pass carry a ref and
//     count as given only after the relay's `dm_put_ok`, which is also when the passes they
//     replace are withdrawn, the self-copy goes and (for a request) I follow them; refused or
//     unanswered for 30 seconds, nothing is recorded or withdrawn, the ticks stay as chosen
//     (also on the Safety page), a request says it was not delivered, and the next sweep sends
//     the same `may`; a reply on a pass that went unanswered is not dropped as a stranger's, and
//     Block withdraws that pass. Tests that go on to use a pass answer the relay's way first
//     (answerPuts), since it counts as given only then.
//  8. 10m (2026-10-10, passes across my own devices): an untick sends its self-copy at once,
//     before the withdrawals, and dm_put_ok sends it no second time; an added tick holds it; my
//     own echo of a refused one is not adopted (also after a reload). A pass my other device
//     withdrew is not replaced with the defaults: the friend is marked, kept across a reload,
//     listed "(updating their pass)" with free ticks, and given nothing by the sweep or their
//     follow until the echo of that device's pass arrives or I tick, follow or accept here. A
//     pass refused for "reach" is not sent again by itself, so its offer shows once. See the
//     block at the end of this file.
//
// Red first, 2026-10-09 (each mutation made in a copy of web/, run, seen failing; tests 0, 4 and 5
// seen red again after the amendment):
//  0: crypto.js DM_PAD_BUCKETS starting at 512 failed "the buckets match and a request is the
//     marker and a pass"; so did reach.js's marker spelled "[[hum:contact-request]]".
//  1: onReachSettings without `reachKnown = reachSettingsFrom(settings)` failed "reach_settings
//     drives what the Safety page shows" (the rows stayed on the defaults, disabled).
//  2: chooseReachAudience sending the whole current set instead of the one kind failed
//     "a changed row sends reach_set with only that kind".
//  3: the `reach_refused` branch taken out of chat-privacy.js's handleMessage failed
//     "a refusal offers a contact request" (no sentence, no button).
//  4: pqBuildContactRequest without `built.recipientPut.contact_request = true` failed "a contact
//     request goes out as a signed DM" (not flagged).
//  5: ingestContactRequest without the pass check listed a request whose pass named someone else;
//     acceptContactRequest without storeCertFrom sent its reply with no friend_cert; the
//     reachScreenDm call taken out of app.js's dm_new kept a refused DM's text; and
//     reachSharesGroupWith answering false for a loaded group list refused a shared group: each
//     failed "receiving a request; a DM my settings refuse". Without `window.peerData = peerData`
//     in app.js it failed too.
//  6: sendPendingWithdrawals() taken out of chat-social.js reissuePassTo failed "the call list
//     re-issues the pass" (no cert_revoke for the old serial). That test became the 10c-ii ones.
//
// Red first, 2026-10-10 (10c-ii; one break at a time in a copy of web/ named by HOS_WEB_DIR):
//  reach.js: Message giving `message` alone failed "the ticks and the pass they give", "a tick
//     re-issues" and the page test; all three off giving an empty `may` failed the first two;
//     Call ticked by default failed the first; "In use now" without grouping rows on one
//     audience failed "the lines above the list" and the page test; the old call-row sentence
//     failed the same two.
//  chat-social.js: setFriendTick adding or dropping only the kind's own word (the old call
//     tick's way) failed "a tick re-issues" ('call,invite,trade,voice_message' for 'call,trade');
//     Unfollow without withdrawPassesTo failed "Unfollow and Block clear the choice".
//  chat-privacy.js: blockLocally without withdrawPassesTo failed "Unfollow and Block clear the
//     choice"; the tick boxes not wired, and the model reading every friend as the defaults,
//     each failed the page test.
//
// Red first, 2026-10-10 (the batch review; the three tests at the end, each run against web/ as
// at a3ca8f167 through HOS_WEB_DIR and seen failing there):
//  reach.js: the web's own help sentences, which differed from src/net/reach.rs in 11 places:
//     "message under nobody: the desktop app's words".
//  chat-privacy.js: a request refusal leaving earlier offers as they were: "the earlier offer's
//     button is turned off".
//  chat-social.js: an echoed pass only added beside the old ones: "the ticks are the echoed
//     pass's, not the union with the old one". With adoptEchoedPass not skipping a serial this
//     device already withdrew: "a pass already taken back here is not standing again".
//
// Red first, 2026-10-10 (10l parity with the desktop app; one break at a time through HOS_WEB_DIR):
//  chat-social.js: reissuePassTo not withdrawing the passes an untick takes away: "the pass
//     allowing calls is withdrawn at once, before any answer".
//  chat-privacy.js: the list built from standing passes only: "Ann stays on the list, Call
//     unticked, held still".
//  chat-dm-store.js: no cap on the passes never answered (friend-pass-web.test.js): "the fifth
//     withdraws the oldest".

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

// Wait until the page is idle, not for a fixed number of turns (2026-10-09): the page's real
// asynchronous work is WebCrypto (the store hashes and encrypts every record), and a fixed 12
// turns was outrun on a loaded machine, failing about 2 runs in 12. Counting the WebCrypto calls
// in flight is block-web.test.js's way; settle() now waits for 12 idle turns in a row.
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

// The relay's answer (10l, 2026-10-10) to every put that carried a ref and has not been answered
// yet: `dm_put_ok`, as a relay that stored them sends, or `dm_put_refused` with `reason`. A pass
// counts as given only after `dm_put_ok`, so tests that go on to use a pass answer first.
const answeredRefs = new Set();
async function answerPuts(t, type = "dm_put_ok", reason = "rate") {
  for (const m of [...t.sock.sent]) {
    if (m.type !== "dm_put" || typeof m.ref !== "string" || answeredRefs.has(m.ref)) continue;
    answeredRefs.add(m.ref);
    await t.handle(type === "dm_put_ok" ? { type, ref: m.ref } : { type, ref: m.ref, reason });
  }
  await settle();
}

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
    crypto: countedCrypto,
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
  run("shared/block.js"); // Block's words, for the 10c-ii test that blocks a friend
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
// A pass `issuer` gave `grantee` on `server` (the stand-in signature is the signed words).
function passFrom(issuer, grantee = ME, server = SERVER, serial = "0123456789abcdef0123456789abcdef", may = "invite,message,trade,voice_message") {
  return fp.friendPassJson(serial, may, b64(fp.friendPassPreimage(server, issuer, grantee, serial, may)));
}
// A contact request from `from` claiming `name`, carrying `pass`.
function requestEnvelope(from, name, pass, ts) {
  return dmEnvelope(from, reach.contactRequestText(name, pass), ts);
}

test("the buckets match and a request is the marker and a pass", async () => {
  const rust = fs.readFileSync(path.join(ROOT, "src", "net", "dm_pq.rs"), "utf8");
  const m = rust.match(/DM_PAD_BUCKETS:\s*\[usize;\s*\d+\]\s*=\s*\[([^\]]+)\]/);
  assert.ok(m, "src/net/dm_pq.rs still declares DM_PAD_BUCKETS");
  const rustBuckets = m[1].split(",").map((s) => Number(s.trim()));
  const { ctx } = await loadChat();
  const webBuckets = Array.from(vm.runInContext("DM_PAD_BUCKETS", ctx));
  assert.deepEqual(webBuckets, rustBuckets, "web and native pad to the same buckets");

  // The request's words (10c as amended): the marker, then {name, pass}.
  assert.equal(reach.CONTACT_REQUEST_MARKER, "[[hum:contact-request:v1]]");
  const pass = passFrom(ME, ANN);
  for (const name of ["A", "Me_1", "x".repeat(24), "a-b_C9"]) {
    const text = reach.contactRequestText(name, pass);
    assert.ok(text.startsWith("[[hum:contact-request:v1]]{"), "the marker, then the JSON");
    assert.deepEqual(JSON.parse(text.slice(reach.CONTACT_REQUEST_MARKER.length)), { name, pass }, "a name and a pass, nothing else");
    assert.ok(reach.isContactRequestText(text));
    assert.deepEqual(reach.contactRequestParse(text), { name, pass });
  }
  for (const bad of ["", "two words", "x".repeat(25), "café", "a.b", "http://x", null]) {
    assert.equal(reach.contactRequestText(bad, pass), null, `"${bad}" is not a registered-name word`);
  }
  assert.equal(reach.contactRequestText("Ann", ""), null, "no pass, no request");
  assert.equal(reach.contactRequestParse("hello"), null, "plain text is not a request");
  assert.equal(reach.contactRequestParse("[[hum:contact-request:v1]]{\"name\":\"Ann\"}"), null, "a request without a pass is not one");
  assert.equal(reach.contactRequestParse("[[hum:contact-request:v1]]not json"), null);
  assert.ok(reach.isContactRequestText("[[hum:contact-request:v1]]not json"), "but it is still kept out of the conversation");

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
  assert.ok(page.includes(reach.REACH_TICKS_UNUSED), "the People I choose list says no row uses it");

  // The list comes from the passes I gave: Ann's includes call, Ben's does not.
  store.recordPassSent(ANN, "aa".repeat(16), "call,invite,message,trade,voice_message");
  store.recordPassSent(BEN, "bb".repeat(16), "invite,message,trade,voice_message");
  m = model();
  assert.deepEqual(m.chosen.map((c) => [c.name, c.ticks.call]), [["Ann", true], ["Ben", false]]);
  assert.ok(/data-tick-key="[0-9a-f]+" data-tick-kind="call"[^>]* checked/.test(html(m)), "a ticked Call is drawn ticked");
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
  const t = await loadChat();
  const { sock, appended, handle } = t;
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
  // "Request sent" once the server took it (10l); refused, the button can send again.
  assert.ok(button.disabled && button.textContent !== "Request sent", "not called sent before the server says so");
  await answerPuts(t, "dm_put_refused", "rate");
  assert.ok(!button.disabled && button.textContent === "Send request", "refused: it can be sent again");
  await button.onclick();
  await answerPuts(t);
  assert.equal(button.textContent, "Request sent");
  assert.ok(button.disabled);

  await handle({ type: "reach_refused", kind: "trade", to: ANN });
  assert.ok(appended.some((el) => textOf(el).includes(reach.REACH_REFUSED_TRADE)), "a refused trade says so");
});

// A refused contact request (only "Nobody" refuses one; the relay marks it `request: true`,
// 2026-10-10) says they are not taking requests, once, and no Send request button is offered to
// them again, where the offer used to return each minute. Seen red with the `request` branch
// taken out of onReachRefused: "says they are not taking requests".
test("a refused contact request says they are not taking requests, and no offer comes back", async () => {
  const { appended, handle } = await loadChat();
  await handle({ type: "reach_refused", kind: "message", to: CY, request: true });
  const said = appended.filter((el) => textOf(el).includes(reach.REACH_NOT_TAKING_REQUESTS));
  assert.equal(said.length, 1, "says they are not taking requests");
  assert.ok(textOf(said[0]).includes("Not delivered to Cy."), "and who it did not reach");
  await handle({ type: "reach_refused", kind: "message", to: CY, request: true });
  assert.equal(appended.filter((el) => textOf(el).includes(reach.REACH_NOT_TAKING_REQUESTS)).length, 1, "once");
  await handle({ type: "reach_refused", kind: "message", to: CY });
  assert.ok(!appended.some((el) => textOf(el).includes(reach.REACH_REFUSED_MESSAGE)), "and a later refused message offers no request");
});

test("a contact request goes out as a signed DM, flagged, carrying my name and my pass for them", async () => {
  const t = await loadChat();
  const { sock, store, fn } = t;
  assert.equal(await fn("sendContactRequest")(ANN), true);
  // The self-copy follows once the server took the request (10l).
  assert.equal(sock.sent.filter((m) => m.type === "dm_put" && m.to === ME).length, 0, "no self-copy before the server's answer");
  await answerPuts(t);
  const puts = sock.sent.filter((m) => m.type === "dm_put");
  const toAnn = puts.filter((m) => m.to === ANN);
  const toMe = puts.filter((m) => m.to === ME);
  assert.equal(toAnn.length, 1, "one deposit for them");
  assert.equal(toMe.length, 1, "and the self-copy, which tells my other devices");
  const put = toAnn[0];
  assert.equal(put.contact_request, true, "flagged");
  assert.ok(!("friend_cert" in put), "I hold no pass from them to present");
  assert.ok(!("contact_request" in toMe[0]), "the self-copy is not flagged");

  const { sealedTo, plain } = opened(put);
  assert.equal(sealedTo, kyberOf(ANN), "sealed to their DM key");
  const webBuckets = Array.from(vm.runInContext("DM_PAD_BUCKETS", (await loadChat()).ctx));
  const inner = JSON.parse(plain);
  // The ordinary padding (crypto.js pqBuildDmPuts): a pad field fills it to just under a bucket.
  assert.equal(typeof inner.pad, "string", "padded like any DM");
  assert.ok(webBuckets.some((size) => plain.length <= size && plain.length > size - 12), "to just under a bucket");
  assert.deepEqual([inner.v, inner.from, inner.to], [2, ME, ANN], "an ordinary v2 inner payload");
  assert.equal(unb64(inner.sig), `hum/dm/v2\n${ME}\n${ANN}\n${inner.ts}\n${inner.text}`, "signed by me, as every DM is");
  const req = reach.contactRequestParse(inner.text);
  assert.ok(req, "its text is a contact request");
  assert.equal(req.name, "Me_1", "my registered name");
  const pass = fp.friendPassParse(req.pass);
  assert.ok(pass, "and a v2 pass");
  assert.equal(pass.may, "invite,message,trade,voice_message", "with the default may: no calls");
  assert.equal(unb64(pass.sig), fp.friendPassPreimage(SERVER, ME, ANN, pass.serial, pass.may), "given by me to them on this server");

  assert.deepEqual(store.certsSent[ANN], [{ serial: pass.serial, may: pass.may }], "they now hold my pass, so their reply gets in");
  assert.ok(store.following.has(ANN), "and I follow them, so their Accept completes the friendship");
});

test("receiving a request; a DM my settings refuse; Accept and Ignore", async () => {
  const { ctx, sock, store, appended, notified, handle, fn } = await loadChat();
  assert.equal(ctx.peerData && ctx.peerData[BEN] && ctx.peerData[BEN].display_name, "Ben", "window.peerData is the member list app.js keeps");
  await handle({ type: "reach_settings", settings: { message: "friends", call: "chosen", trade: "friends" } });
  store.recordPassSent(ANN, "aa".repeat(16), "invite,message,trade,voice_message"); // Ann is a friend
  const everything = () => [...appended.map(textOf), ...notified.map((a) => a.join(" ")), JSON.stringify(store.contactRequests), fn("contactRequestsSidebarHtml")(), fn("safetyPanelHtml")(fn("safetyModel")())].join("\n");

  // A refused DM: listed by name, its text nowhere.
  const SECRET = "meet me at the old mill, here is my number";
  await handle({ type: "dm_new", id: 1, content: dmEnvelope(CY, SECRET) });
  await settle();
  assert.deepEqual(store.conversation(CY), [], "the text is not kept");
  assert.deepEqual(store.contactRequestList().map((r) => [r.id, r.key, r.name, r.hasPass]), [[CY, CY, "Cy", false]], "Cy is listed by name");
  assert.ok(!everything().includes("old mill"), "and the text is shown nowhere");
  assert.ok(fn("contactRequestsSidebarHtml")().includes(`data-req-accept="${CY}"`), "the DMs tab lists it with Accept");

  await handle({ type: "dm_new", id: 2, content: dmEnvelope(ANN, "hi from Ann") });
  await settle();
  assert.equal(store.conversation(ANN).length, 1, "a friend's DM is kept as before");

  // Contact requests: the pass must be Ben's, for me, on this server; the name shown is the member list's.
  const bensPass = passFrom(BEN);
  await handle({ type: "dm_new", id: 3, content: requestEnvelope(BEN, "Admin_Team", passFrom(BEN, ANN)) });
  await handle({ type: "dm_new", id: 4, content: requestEnvelope(BEN, "Admin_Team", passFrom(BEN, ME, "did:hum:elsewhere")) });
  await handle({ type: "dm_new", id: 5, content: requestEnvelope(BEN, "Admin_Team", passFrom(ANN)) });
  await settle();
  assert.deepEqual(store.contactRequestList().map((r) => r.key), [CY], "a request whose pass does not check is dropped");
  await handle({ type: "dm_new", id: 6, content: requestEnvelope(BEN, "Admin_Team", bensPass) });
  await settle();
  const ben = store.contactRequestList().find((r) => r.key === BEN);
  assert.ok(ben && ben.hasPass, "a request whose pass checks is listed");
  assert.equal(ben.name, "Ben", "under the member list's name for the signed sender, not the one claimed");
  assert.ok(!everything().includes("Admin_Team"), "the claimed name is shown nowhere");
  assert.deepEqual(store.conversation(BEN), [], "and the request is not a message");
  const STRANGER = "e5".repeat(32);
  await handle({ type: "dm_new", id: 7, content: requestEnvelope(STRANGER, "Admin_Team", passFrom(STRANGER)) });
  await settle();
  assert.equal(store.contactRequestList().find((r) => r.key === STRANGER).name, vm.runInContext("shortKey", ctx)(STRANGER), "someone the member list does not know shows as their short key");
  fn("ignoreContactRequest")(STRANGER);

  // Ignore: gone, nothing sent.
  sock.sent.length = 0;
  fn("ignoreContactRequest")(CY);
  assert.deepEqual(store.contactRequestList().map((r) => r.key), [BEN]);
  assert.equal(sock.sent.length, 0, "Ignore tells no one");

  // Accept: follow back and send my pass, each put presenting Ben's pass to his relay.
  assert.equal(await fn("acceptContactRequest")(BEN), true);
  await settle();
  const toBen = sock.sent.filter((m) => m.type === "dm_put" && m.to === BEN);
  const texts = toBen.map((m) => JSON.parse(opened(m).plain).text);
  assert.ok(texts.includes(CTL_FOLLOW), "Accept follows them back");
  assert.ok(texts.includes(CTL_FRIEND_CERT), "and gives them my pass");
  for (const m of toBen) assert.equal(m.friend_cert, bensPass, "each reply carries the pass Ben's request gave me");
  assert.equal(store.certFor(BEN), bensPass);
  await answerPuts({ sock, handle }); // my pass counts as given once the server took it (10l)
  assert.ok(store.certSentTo(BEN));
  assert.deepEqual(store.contactRequestList(), []);

  // My own request, echoed from another of my devices: the pass I gave is recorded.
  const mine = passFrom(ME, CY, SERVER, "fedcba9876543210fedcba9876543210");
  const echo = JSON.stringify({ v: 2, ek_ct_b64: b64(MY_KYBER), nonce_b64: "AAAA", ct_b64: b64(JSON.stringify({
    v: 2, from: ME, to: CY, ts: 5, text: reach.contactRequestText("Me_1", mine),
    sig: b64(`hum/dm/v2\n${ME}\n${CY}\n5\n${reach.contactRequestText("Me_1", mine)}`),
  })) });
  await handle({ type: "dm_new", id: 8, content: echo });
  await settle();
  assert.deepEqual(store.certsSent[CY], [{ serial: "fedcba9876543210fedcba9876543210", may: "invite,message,trade,voice_message" }]);
  assert.ok(store.following.has(CY));
  assert.deepEqual(store.contactRequestList(), [], "and it is not a request to me");

  // "Nobody": not even a request.
  const DAN = "f6".repeat(32);
  await handle({ type: "reach_settings", settings: { message: "nobody", call: "chosen", trade: "friends" } });
  await handle({ type: "dm_new", id: 9, content: requestEnvelope(DAN, "Dan", passFrom(DAN)) });
  await handle({ type: "dm_new", id: 10, content: dmEnvelope(DAN, "again") });
  await settle();
  assert.deepEqual(store.contactRequestList(), [], "nobody lists no request");
  assert.deepEqual(store.conversation(DAN), [], "and keeps no text");

  // "Groups": group membership is the server's call (10m R8, 2026-10-10). Someone it let through
  // shares a group as far as this page is concerned, even when this page's own list of groups,
  // loaded on connect, does not show it yet (they joined since). This used to check that list and
  // turn the DM into a request with its text dropped.
  await handle({ type: "reach_settings", settings: { message: "groups", call: "chosen", trade: "friends" } });
  ctx._p2pGroups = [{ group_id: "g1", members: [ME, ANN] }];
  await handle({ type: "dm_new", id: 11, content: dmEnvelope(DAN, "I joined your group this morning") });
  await settle();
  assert.equal(store.conversation(DAN).length, 1, "a DM the server let through under Groups is kept, whatever this page's group list says");
  assert.deepEqual(store.contactRequestList(), [], "and is not made a request");

  // "Anyone": a stranger's DM is a message.
  const EVE = "a7".repeat(32);
  await handle({ type: "reach_settings", settings: { message: "anyone", call: "chosen", trade: "friends" } });
  await handle({ type: "dm_new", id: 13, content: dmEnvelope(EVE, "hello from Eve") });
  await settle();
  assert.equal(store.conversation(EVE).length, 1, "anyone lets it through");
});

// ── 10c-ii: a tick per friend for Messages, Calls and Trades ─────────────

// Every mix of ticks, as {message, call, trade}.
const TICK_MIXES = [];
for (const message of [false, true]) for (const call of [false, true]) for (const trade of [false, true]) TICK_MIXES.push({ message, call, trade });
const minus = (a, b) => a.filter((w) => !b.includes(w)).sort();

test("10c-ii: the ticks and the pass they give", () => {
  // A friend with no saved choice: Message and Trade, not Call, the same as a new pass (step A).
  assert.deepEqual(reach.REACH_TICK_DEFAULTS, { message: true, call: false, trade: true });
  for (const none of [null, undefined, ""]) {
    assert.deepEqual(reach.reachTicksFromMay(none), { message: true, call: false, trade: true }, "no pass: the defaults");
  }
  assert.deepEqual(reach.reachMayFromTicks(reach.REACH_TICK_DEFAULTS), fp.FRIEND_PASS_DEFAULT_MAY, "the defaults give the default may");
  assert.deepEqual(reach.reachMayFromTicks({}), fp.FRIEND_PASS_DEFAULT_MAY, "and so does no choice at all");
  assert.deepEqual(reach.reachMayFromTicks(null), fp.FRIEND_PASS_DEFAULT_MAY);
  assert.deepEqual(reach.reachMayFromTicks(reach.reachTicksFromMay(null)), fp.FRIEND_PASS_DEFAULT_MAY);

  for (const t of TICK_MIXES) {
    const words = reach.reachMayFromTicks(t);
    assert.equal(fp.friendPassMay(words), words.join(","), `${JSON.stringify(t)}: a valid pass, in canonical order`);
    assert.deepEqual(reach.reachTicksFromMay(words.join(",")), t, `${JSON.stringify(t)}: the pass reads back as the same ticks`);
  }

  // Unticking Message drops message, invite and voice_message and nothing else.
  for (const t of TICK_MIXES.filter((x) => x.message && (x.call || x.trade))) {
    const before = reach.reachMayFromTicks(t);
    const after = reach.reachMayFromTicks({ ...t, message: false });
    assert.deepEqual(minus(before, after), ["invite", "message", "voice_message"], `${JSON.stringify(t)}: unticking Message drops exactly those`);
    assert.deepEqual(minus(after, before), [], "and adds nothing");
  }
  // Ticking Call adds call and nothing else.
  for (const t of TICK_MIXES.filter((x) => !x.call && (x.message || x.trade))) {
    const before = reach.reachMayFromTicks(t);
    const after = reach.reachMayFromTicks({ ...t, call: true });
    assert.deepEqual(minus(after, before), ["call"], `${JSON.stringify(t)}: ticking Call adds only call`);
    assert.deepEqual(minus(before, after), [], "and takes nothing away");
  }
  assert.deepEqual(reach.reachMayFromTicks({ message: true, call: true, trade: true }), ["call", "invite", "message", "trade", "voice_message"]);

  // All three unticked: still a valid pass, `invite` alone (the format refuses an empty may).
  const none = reach.reachMayFromTicks({ message: false, call: false, trade: false });
  assert.deepEqual(none, ["invite"], "invite alone");
  assert.equal(fp.friendPassMay(none), "invite", "which the pass format accepts");
  assert.equal(fp.friendPassMay([]), null, "(an empty may it refuses)");
  assert.deepEqual(reach.reachTicksFromMay("invite"), { message: false, call: false, trade: false }, "and it reads back as nothing ticked");
  // The edges of the filler: it is there only when nothing else is.
  assert.deepEqual(reach.reachMayFromTicks({ message: false, call: true, trade: false }), ["call"]);
  assert.deepEqual(reach.reachMayFromTicks({ message: false, call: false, trade: true }), ["trade"]);
});

test("10c-ii: the lines above the list, and each row's help sentence", () => {
  assert.equal(reach.REACH_TICKS_NOTE, "These ticks count for a row set to People I choose.");
  const use = reach.reachTicksInUse;
  const S = (message, call, trade) => ({ message, call, trade });
  // None set to People I choose.
  assert.equal(use(S("friends", "friends", "friends")), "Not in use now: no row is set to People I choose.");
  assert.equal(use(S("nobody", "anyone", "groups")), "Not in use now: no row is set to People I choose.");
  // One: the spec's own example, which is also the safe defaults.
  assert.equal(use(S("friends", "chosen", "friends")), "In use now: Calls. Messages and Trades are set to Friends, so every friend gets through for those.");
  assert.equal(use(null), use(S("friends", "chosen", "friends")), "the defaults read as the spec's example");
  assert.equal(use(S("anyone", "nobody", "chosen")),
    "In use now: Trades. Messages are set to Anyone, so anyone gets through for those. Calls are set to Nobody, so no one gets through for those.");
  assert.equal(use(S("chosen", "friends", "nobody")),
    "In use now: Messages. Calls are set to Friends, so every friend gets through for those. Trades are set to Nobody, so no one gets through for those.");
  // Two.
  assert.equal(use(S("chosen", "chosen", "groups")),
    "In use now: Messages and Calls. Trades are set to Friends and people in my groups, so every friend and everyone in your groups gets through for those.");
  assert.equal(use(S("chosen", "anyone", "chosen")), "In use now: Messages and Trades. Calls are set to Anyone, so anyone gets through for those.");
  // All three.
  assert.equal(use(S("chosen", "chosen", "chosen")), "In use now: Messages, Calls and Trades.");

  // Every one of the 125 mixes: each row named once, a chosen one under "In use now", any
  // other beside its audience's name.
  for (const m of reach.REACH_AUDIENCES) for (const c of reach.REACH_AUDIENCES) for (const t of reach.REACH_AUDIENCES) {
    const s = S(m, c, t);
    const line = use(s);
    const chosen = reach.REACH_KINDS.filter((k) => s[k] === "chosen");
    if (!chosen.length) { assert.equal(line, reach.REACH_TICKS_UNUSED); continue; }
    const head = line.slice(0, line.indexOf("."));
    for (const k of reach.REACH_KINDS) {
      const label = reach.REACH_KIND_LABELS[k];
      assert.equal(line.split(label).length - 1, 1, `${JSON.stringify(s)}: ${label} named once in "${line}"`);
      assert.equal(head.includes(label), s[k] === "chosen", `${JSON.stringify(s)}: ${label} is in use exactly when chosen`);
    }
    for (const a of new Set(reach.REACH_KINDS.map((k) => s[k]).filter((x) => x !== "chosen"))) {
      assert.ok(line.includes(`set to ${reach.REACH_AUDIENCE_LABELS[a]}, so ${reach.REACH_THROUGH[a]} for those.`), `${JSON.stringify(s)}: says what ${a} lets through`);
    }
  }

  // The rows' help under People I choose names the new list, for all three kinds.
  assert.equal(reach.reachExplain("call", "chosen"), 'Only the friends you tick for Call on your "People I choose" list can call you.');
  assert.equal(reach.reachExplain("trade", "chosen"), 'Only the friends you tick for Trade on your "People I choose" list can send you trade requests.');
  assert.equal(reach.reachExplain("message", "chosen"),
    'Only the friends you tick for Message on your "People I choose" list can message you. Anyone else can send a contact request that shows you only their name.');
  for (const k of reach.REACH_KINDS) for (const a of reach.REACH_AUDIENCES) {
    assert.ok(!reach.reachExplain(k, a).includes("People who may call me"), "the old list's name is gone");
  }
});

test("10c-ii: a tick re-issues the pass with the new may, then withdraws the old serial", async () => {
  const { sock, store, handle, fn } = await loadChat();
  const OLD = "00112233445566778899aabbccddeeff";
  store.recordPassSent(ANN, OLD, "invite,message,trade,voice_message");
  assert.deepEqual(fn("friendTicks")(ANN), { message: true, call: false, trade: true }, "a new friend: the defaults");
  const sentPass = () => {
    const put = sock.sent.find((m) => m.type === "dm_put" && m.to === ANN);
    assert.ok(put, "a new pass goes to Ann");
    const inner = JSON.parse(opened(put).plain);
    assert.equal(inner.text, CTL_FRIEND_CERT);
    return { pass: fp.friendPassParse(inner.cert), at: sock.sent.indexOf(put) };
  };
  // Each tick's pass is answered as a relay that stored it answers (10l), so the old one is withdrawn.
  const tick = async (kind, on) => {
    sock.sent.length = 0;
    const sent = await fn("setFriendTick")(ANN, kind, on);
    await answerPuts({ sock, handle });
    return sent;
  };

  // Call ticked: everything she had, plus call; the old serial withdrawn after the new pass went.
  assert.equal(await tick("call", true), true);
  const first = sentPass();
  assert.equal(first.pass.may, "call,invite,message,trade,voice_message");
  assert.notEqual(first.pass.serial, OLD, "a new serial");
  const revoked = sock.sent.findIndex((m) => m.type === "cert_revoke" && m.serial === OLD);
  assert.ok(revoked > first.at, "the old serial is withdrawn, after the new pass went");
  assert.deepEqual(store.certsSent[ANN], [{ serial: first.pass.serial, may: first.pass.may }], "the new pass is the record");
  assert.deepEqual(fn("friendTicks")(ANN), { message: true, call: true, trade: true });

  // Message unticked: message, invite and voice_message go, nothing else.
  assert.equal(await tick("message", false), true);
  const second = sentPass();
  assert.equal(second.pass.may, "call,trade");
  assert.deepEqual(minus(first.pass.may.split(","), second.pass.may.split(",")), ["invite", "message", "voice_message"]);
  assert.ok(sock.sent.some((m) => m.type === "cert_revoke" && m.serial === first.pass.serial));

  // Down to nothing ticked: a valid pass that carries invite alone.
  assert.equal(await tick("trade", false), true);
  assert.equal(sentPass().pass.may, "call");
  assert.equal(await tick("call", false), true);
  const empty = sentPass();
  assert.ok(empty.pass, "still a pass the format accepts");
  assert.equal(empty.pass.may, "invite");
  assert.deepEqual(fn("friendTicks")(ANN), { message: false, call: false, trade: false });
  assert.ok(store.certSentTo(ANN), "unticking does not end the friendship");

  // A tick that already stands sends nothing; nothing for someone with no pass, or an unknown kind.
  assert.equal(await tick("call", false), true);
  assert.deepEqual(sock.sent, [], "nothing to change, nothing sent");
  assert.equal(await fn("setFriendTick")(CY, "call", true), false, "only for someone I gave a pass");
  assert.equal(await fn("setFriendTick")(ANN, "voice", true), false, "only the three kinds");

  // Another device of mine still listing a withdrawn serial drops it when the relay confirms.
  store.recordPassSent(ANN, first.pass.serial, first.pass.may);
  await handle({ type: "cert_revoked", to: ME, serial: first.pass.serial });
  assert.deepEqual(store.certsSent[ANN].map((p) => p.serial), [empty.pass.serial]);
});

test("10c-ii: Unfollow and Block clear the choice; a friendship begun again starts from the defaults", async () => {
  const { ctx, sock, store, fn, handle } = await loadChat();
  const answer = () => answerPuts({ sock, handle }); // the relay took the pass (10l)
  const befriend = vm.runInContext(`(k) => {
    myFollowers.add(k); myFollowing.add(k);
    hosDmStore.setFollower(k, true); hosDmStore.setFollowing(k, true);
  }`, ctx);
  const passTo = (peer) => {
    const put = [...sock.sent].reverse().find((m) => m.type === "dm_put" && m.to === peer && JSON.parse(opened(m).plain).text === CTL_FRIEND_CERT);
    return put ? fp.friendPassParse(JSON.parse(opened(put).plain).cert) : null;
  };
  const chosenKeys = () => fn("safetyModel")().chosen.map((c) => c.key);
  const custom = async () => {
    await answer();
    assert.equal(await fn("setFriendTick")(ANN, "call", true), true);
    await answer();
    assert.equal(await fn("setFriendTick")(ANN, "trade", false), true);
    await answer();
    assert.deepEqual(fn("friendTicks")(ANN), { message: true, call: true, trade: false }, "a choice of my own");
  };

  // Unfollow.
  befriend(ANN);
  store.recordPassSent(ANN, "aa".repeat(16), "invite,message,trade,voice_message");
  await custom();
  const chosenSerial = store.certsSent[ANN][0].serial;
  sock.sent.length = 0;
  await fn("setFollowLocal")(ANN, false);
  assert.ok(sock.sent.some((m) => m.type === "cert_revoke" && m.serial === chosenSerial), "Unfollow withdraws the pass that held the choice");
  assert.equal(store.passMayTo(ANN), null, "no pass to her stands");
  assert.ok(!chosenKeys().includes(ANN), "she leaves the People I choose list");
  assert.deepEqual(fn("friendTicks")(ANN), reach.REACH_TICK_DEFAULTS, "and her choice is cleared");
  // Following again (she still follows me): the new pass carries the defaults, not the old choice.
  sock.sent.length = 0;
  await fn("setFollowLocal")(ANN, true);
  await settle();
  assert.equal(passTo(ANN).may, "invite,message,trade,voice_message", "a friendship begun again starts from the defaults");
  assert.deepEqual(fn("friendTicks")(ANN), { message: true, call: false, trade: true });

  // Block.
  await custom();
  const blockedSerial = store.certsSent[ANN][0].serial;
  sock.sent.length = 0;
  assert.equal(await fn("blockKey")(ANN), true);
  await settle();
  assert.ok(sock.sent.some((m) => m.type === "cert_revoke" && m.serial === blockedSerial), "Block withdraws the pass that held the choice");
  assert.equal(store.passMayTo(ANN), null);
  assert.ok(!chosenKeys().includes(ANN), "she leaves the list");
  assert.deepEqual(fn("friendTicks")(ANN), reach.REACH_TICK_DEFAULTS, "and her choice is cleared");
  assert.equal(await fn("unblockKey")(ANN), true);
  await settle();
  assert.equal(store.certSentTo(ANN), false, "Unblock gives no pass by itself");
  sock.sent.length = 0;
  await fn("setFollowLocal")(ANN, true);
  await settle();
  assert.equal(passTo(ANN).may, "invite,message,trade,voice_message", "following again after Unblock starts from the defaults");
});

test("10c-ii: Settings > Safety > People I choose, each friend once with three ticks", async () => {
  const { ctx, sock, store, handle, fn } = await loadChat();
  // A Safety card that keeps its HTML and answers for the tick boxes drawn in it, so the
  // page's own handlers can be pressed.
  const card = { innerHTML: "", boxes: [] };
  card.querySelector = () => null;
  card.querySelectorAll = (sel) => {
    if (sel !== "input[data-tick-key]") return [];
    card.boxes = [...card.innerHTML.matchAll(/<input type="checkbox" data-tick-key="([^"]+)" data-tick-kind="([^"]+)"[^>]*>/g)]
      .map((m) => ({ dataset: { tickKey: m[1], tickKind: m[2] }, checked: / checked\b/.test(m[0]), disabled: / disabled\b/.test(m[0]), onchange: null }));
    return card.boxes;
  };
  const overlay = { classList: { contains: () => true, add() {}, remove() {} } };
  ctx.document = new Proxy({}, {
    get(_t, prop) {
      if (prop === "getElementById") return (id) => (id === "safety-overlay" ? overlay : id === "safety-card" ? card : null);
      return anything();
    },
  });
  const box = (key, kind) => card.boxes.find((b) => b.dataset.tickKey === key && b.dataset.tickKind === kind);

  await handle({ type: "reach_settings", settings: { message: "friends", call: "chosen", trade: "friends" } });
  store.recordPassSent(ANN, "aa".repeat(16), "call,invite,message,trade,voice_message");
  store.recordPassSent(BEN, "bb".repeat(16), "invite,message,trade,voice_message");
  store.recordPassSent(CY, "cc".repeat(16), "invite");
  const m = fn("safetyModel")();
  assert.deepEqual(m.chosen.map((c) => [c.name, c.ticks]), [
    ["Ann", { message: true, call: true, trade: true }],
    ["Ben", { message: true, call: false, trade: true }],
    ["Cy", { message: false, call: false, trade: false }],
  ], "each friend I gave a pass, by name, with the ticks the pass carries");
  assert.equal(m.ticksInUse, "In use now: Calls. Messages and Trades are set to Friends, so every friend gets through for those.");

  fn("renderSafetyPanel")();
  const page = card.innerHTML;
  assert.ok(page.includes(">People I choose</h3>"), "the list is called People I choose");
  assert.ok(!page.includes("People who may call me"), "the old name is gone");
  const note = page.indexOf(reach.REACH_TICKS_NOTE);
  const inUse = page.indexOf(m.ticksInUse);
  const firstFriend = page.indexOf("data-chosen-key=");
  assert.ok(note >= 0 && inUse > note && firstFriend > inUse, "the line about the ticks, then which rows use them, then the friends");
  for (const key of [ANN, BEN, CY]) {
    assert.equal(page.split(`data-chosen-key="${key}"`).length - 1, 1, "each friend once");
    assert.deepEqual(card.boxes.filter((b) => b.dataset.tickKey === key).map((b) => b.dataset.tickKind), ["message", "call", "trade"], "with three ticks");
  }
  assert.ok(page.includes(`aria-label="Ben: Message"`) && page.includes("<span>Call</span>") && page.includes("<span>Trade</span>"), "labelled Message, Call and Trade");
  assert.deepEqual([box(BEN, "message").checked, box(BEN, "call").checked, box(BEN, "trade").checked], [true, false, true], "drawn as the pass has them");
  assert.deepEqual([box(CY, "message").checked, box(CY, "call").checked, box(CY, "trade").checked], [false, false, false]);

  // Ticking Call for Ben, through the page's own box.
  const benCall = box(BEN, "call");
  assert.equal(typeof benCall.onchange, "function", "each tick is wired");
  sock.sent.length = 0;
  benCall.checked = true;
  benCall.onchange();
  assert.ok(card.innerHTML.includes("(updating their pass)"), "while his pass is minted the page says so");
  assert.ok(box(BEN, "message").disabled && box(BEN, "trade").disabled && !box(ANN, "call").disabled, "and holds his ticks still, only his");
  await settle();
  const put = sock.sent.find((x) => x.type === "dm_put" && x.to === BEN);
  assert.ok(put, "a new pass goes to Ben");
  assert.equal(fp.friendPassParse(JSON.parse(opened(put).plain).cert).may, "call,invite,message,trade,voice_message");
  // Until the server answers, his pass is still being updated (10l).
  assert.ok(card.innerHTML.includes("(updating their pass)"), "still updating until the server answers");
  await answerPuts({ sock, handle });
  assert.ok(sock.sent.some((x) => x.type === "cert_revoke" && x.serial === "bb".repeat(16)), "and the old one is withdrawn");
  assert.ok(box(BEN, "call").checked && !box(BEN, "call").disabled, "drawn again, ticked");
  assert.ok(!card.innerHTML.includes("(updating their pass)"));

  // The line follows the rows.
  await handle({ type: "reach_settings", settings: { message: "chosen", call: "chosen", trade: "nobody" } });
  assert.ok(card.innerHTML.includes("In use now: Messages and Calls. Trades are set to Nobody, so no one gets through for those."));
  await handle({ type: "reach_settings", settings: { message: "friends", call: "anyone", trade: "friends" } });
  assert.ok(card.innerHTML.includes(reach.REACH_TICKS_UNUSED));

  // No friends yet.
  for (const k of [ANN, BEN, CY]) store.withdrawPassesTo(k);
  fn("renderSafetyPanel")();
  assert.ok(card.innerHTML.includes("Friends appear here once you have some.") && !card.innerHTML.includes("data-chosen-key"));
});

// ── The batch review's fixes (2026-10-10) ────────────────────────────────

// Each row's help sentence: the desktop app's words are the source of truth. They are read out of
// src/net/reach.rs (`Audience::meaning`) here, so a change on either side that the other does not
// follow fails at once.
test("each row's help sentence is the desktop app's, read out of src/net/reach.rs", () => {
  const rust = fs.readFileSync(path.join(ROOT, "src", "net", "reach.rs"), "utf8");
  const start = rust.indexOf("pub fn meaning(self, kind: ReachKind)");
  assert.ok(start > 0, "reach.rs has Audience::meaning");
  const body = rust.slice(start, rust.indexOf("pub struct ReachSettings", start));
  const arms = [...body.matchAll(/\(ReachKind::(\w+), Audience::(\w+)\)\s*=>\s*\{?\s*"((?:[^"\\]|\\.)*)"/g)];
  assert.equal(arms.length, 15, "one sentence for each kind and audience");
  const unescape = (s) => {
    assert.ok(!/\\[^"\\]/.test(s), "only \\\" and \\\\ escapes in these strings");
    return s.replace(/\\(["\\])/g, "$1");
  };
  const seen = new Set();
  for (const [, kind, audience, said] of arms) {
    const k = kind.toLowerCase();
    const a = audience.toLowerCase();
    assert.ok(reach.REACH_KINDS.includes(k) && reach.REACH_AUDIENCES.includes(a), `${k}/${a} is a kind and an audience the web knows`);
    seen.add(k + "/" + a);
    assert.equal(reach.reachExplain(k, a), unescape(said), `${k} under ${a}: the desktop app's words`);
  }
  assert.equal(seen.size, 15, "every pair once");
  assert.equal(reach.reachExplain("message", "everyone"), "", "nothing for an audience it does not know");
});

test("a refused contact request takes the Send request button off every earlier offer for that person", async () => {
  const { sock, appended, handle } = await loadChat();
  await handle({ type: "reach_refused", kind: "message", to: CY });
  const offer = appended.find((el) => textOf(el).includes(reach.REACH_REFUSED_MESSAGE));
  const button = findButton(offer, "Send request");
  assert.ok(button && !button.disabled, "an offer with a live Send request button");
  await handle({ type: "reach_refused", kind: "message", to: CY, request: true });
  assert.ok(button.disabled, "the earlier offer's button is turned off");
  assert.ok(textOf(offer).includes(reach.REACH_NOT_TAKING_REQUESTS), "and the offer says they are not taking requests");
  sock.sent.length = 0;
  if (typeof button.onclick === "function") await button.onclick();
  assert.deepEqual(sock.sent, [], "pressing it anyway sends nothing");
});

// A DM self-copy from another of my devices (from me, to `to`), sealed to my DM key.
function selfEnvelope(to, text, cert, ts = 1760000005000) {
  const sig = b64(`hum/dm/v2\n${ME}\n${to}\n${ts}\n${text}`);
  const inner = JSON.stringify({ v: 2, from: ME, to, ts, text, sig, cert });
  return JSON.stringify({ v: 2, ek_ct_b64: b64(MY_KYBER), nonce_b64: "AAAA", ct_b64: b64(inner) });
}

test("a pass echoed from another of my devices replaces this one's record of the friend: a tick taken away there stays away", async () => {
  const { sock, store, handle, fn } = await loadChat();
  const OLD = "00112233445566778899aabbccddeeff";
  const NEW = "ffeeddccbbaa99887766554433221100";
  store.recordPassSent(ANN, OLD, "invite,message,trade,voice_message");
  // My other device gave Ann Call and took Trade away: its new pass, echoed to me.
  const may = "call,invite,message,voice_message";
  const cert = fp.friendPassJson(NEW, may, b64(fp.friendPassPreimage(SERVER, ME, ANN, NEW, may)));
  sock.sent.length = 0;
  await handle({ type: "dm_new", id: 81, content: selfEnvelope(ANN, CTL_FRIEND_CERT, cert) });
  await settle();
  assert.deepEqual(fn("friendTicks")(ANN), { message: true, call: true, trade: false }, "the ticks are the echoed pass's, not the union with the old one");
  assert.deepEqual(store.certsSent[ANN].map((p) => p.serial), [NEW], "the echoed pass is the record");
  assert.ok(sock.sent.some((m) => m.type === "cert_revoke" && m.serial === OLD), "the pass it replaced is withdrawn here too");
  // My own earlier pass, echoing back late after the newer one replaced it: it does not come back.
  const oldCert = fp.friendPassJson(OLD, "invite,message,trade,voice_message", b64(fp.friendPassPreimage(SERVER, ME, ANN, OLD, "invite,message,trade,voice_message")));
  sock.sent.length = 0;
  await handle({ type: "dm_new", id: 82, content: selfEnvelope(ANN, CTL_FRIEND_CERT, oldCert, 1760000004000) });
  await settle();
  assert.deepEqual(store.certsSent[ANN].map((p) => p.serial), [NEW], "a pass already taken back here is not standing again");
  assert.ok(!sock.sent.some((m) => m.type === "cert_revoke" && m.serial === NEW), "and the newer one is not withdrawn for it");
});

// ── 10l: a pass counts as given only once the server took it (2026-10-10) ──
// docs/design/blocking-and-safe-mode.md 10l. A re-issued pass (a tick) and a contact request's
// pass each carry a `ref`; only the relay's `dm_put_ok` for it records the pass, withdraws the
// ones it replaces and lets the self-copy go. A `dm_put_refused`, or 30 seconds with no answer,
// records nothing, withdraws nothing and keeps the ticks as chosen; the next sweep sends the same
// `may` again. Red first: each test below was run against web/ as at b46441843 (before 10l)
// through HOS_WEB_DIR and seen failing there with the assertion named in the list at the end of
// this file.

const memberUsers = () => [[ANN, "Ann"], [BEN, "Ben"], [CY, "Cy"]].map(([k, name]) => ({ public_key: k, name, role: "", kyber_public: kyberOf(k) }));
const passOf = (put) => fp.friendPassParse(JSON.parse(opened(put).plain).cert);
const selfCopiesOf = (sock) => sock.sent.filter((m) => m.type === "dm_put" && m.to === ME);
const DEFAULT_MAY = "invite,message,trade,voice_message";
const WITH_CALL = "call,invite,message,trade,voice_message";

test("10l: a re-issued pass is recorded, and the old one withdrawn, only after dm_put_ok", async () => {
  const { sock, store, handle, fn } = await loadChat();
  const OLD = "00112233445566778899aabbccddeeff";
  store.recordPassSent(ANN, OLD, DEFAULT_MAY);
  assert.equal(await fn("setFriendTick")(ANN, "call", true), true, "Call ticked: the new pass goes");
  await settle();
  const put = sock.sent.find((m) => m.type === "dm_put" && m.to === ANN);
  assert.ok(put, "a new pass goes to Ann");
  const pass = passOf(put);
  assert.equal(pass.may, WITH_CALL);
  assert.deepEqual(store.certsSent[ANN], [{ serial: OLD, may: DEFAULT_MAY }], "not given until the server says so: the old pass is still the record");
  assert.ok(!sock.sent.some((m) => m.type === "cert_revoke"), "and the old one is not withdrawn yet");
  assert.equal(selfCopiesOf(sock).length, 0, "nor are my other devices told");
  assert.ok(typeof put.ref === "string" && /^[A-Za-z0-9_-]{1,64}$/.test(put.ref), "the put carries a ref");
  assert.deepEqual(fn("friendTicks")(ANN), { message: true, call: true, trade: true }, "the ticks show my choice meanwhile");
  assert.equal(fn("friendPassUpdating")(ANN), true, "and the page holds them still");

  await handle({ type: "dm_put_ok", ref: put.ref });
  await settle();
  assert.deepEqual(store.certsSent[ANN], [{ serial: pass.serial, may: WITH_CALL }], "given once the server took it");
  const at = sock.sent.indexOf(put);
  const revoked = sock.sent.findIndex((m) => m.type === "cert_revoke" && m.serial === OLD);
  assert.ok(revoked > at, "the old pass is withdrawn then");
  const self = selfCopiesOf(sock);
  assert.equal(self.length, 1, "and my other devices are told");
  assert.equal(opened(self[0]).sealedTo, MY_KYBER);
  assert.equal(JSON.parse(opened(self[0]).plain).cert, JSON.parse(opened(put).plain).cert, "of the pass the server took");
  assert.equal(fn("friendPassUpdating")(ANN), false);
  assert.equal(store.passIntent[ANN], undefined, "the choice is the record now");
});

test("10l: refused or unanswered, nothing is recorded or withdrawn, the ticks stay, and the next sweep sends the same may", async () => {
  const { ctx, sock, store, handle, fn } = await loadChat();
  const answerWaits = [];
  ctx.setTimeout = (cb, ms) => { if (ms === 30000) answerWaits.push(cb); return 0; };
  const OLD = "00112233445566778899aabbccddeeff";
  store.recordPassSent(ANN, OLD, DEFAULT_MAY);
  const putsToAnn = () => sock.sent.filter((m) => m.type === "dm_put" && m.to === ANN);
  const nothingChanged = (why) => {
    assert.deepEqual(store.certsSent[ANN], [{ serial: OLD, may: DEFAULT_MAY }], `${why}: nothing is recorded`);
    assert.ok(!sock.sent.some((m) => m.type === "cert_revoke"), `${why}: nothing is withdrawn`);
    assert.equal(selfCopiesOf(sock).length, 0, `${why}: my other devices are told nothing`);
    assert.deepEqual(fn("friendTicks")(ANN), { message: true, call: true, trade: true }, `${why}: the ticks stay as I chose`);
    const row = fn("safetyModel")().chosen.find((c) => c.key === ANN);
    assert.ok(row && row.ticks.call === true && row.updating === false, `${why}: the Safety page draws Call ticked, not held`);
  };

  // Refused (a new account's slower refill, say).
  assert.equal(await fn("setFriendTick")(ANN, "call", true), true);
  await settle();
  const [first] = putsToAnn();
  await handle({ type: "dm_put_refused", ref: first.ref, reason: "rate" });
  await settle();
  nothingChanged("refused");

  // The next sweep (on the member list) sends the same may again.
  await handle({ type: "full_user_list", users: memberUsers() });
  await settle();
  const second = putsToAnn()[1];
  assert.ok(second, "the next sweep sends Ann's pass again");
  assert.equal(passOf(second).may, WITH_CALL, "with the same may");
  assert.notEqual(passOf(second).serial, passOf(first).serial, "under a new serial");

  // Thirty seconds without an answer.
  assert.equal(answerWaits.length, 2);
  answerWaits[1]();
  await settle();
  nothingChanged("unanswered");
  await handle({ type: "dm_put_ok", ref: second.ref });
  await settle();
  nothingChanged("answered after the wait");

  // The next sweep again; this time the server takes it.
  await handle({ type: "full_user_list", users: memberUsers() });
  await settle();
  const third = putsToAnn()[2];
  assert.ok(third && passOf(third).may === WITH_CALL, "sent again with the same may");
  await handle({ type: "dm_put_ok", ref: third.ref });
  await settle();
  assert.deepEqual(store.certsSent[ANN], [{ serial: passOf(third).serial, may: WITH_CALL }], "given once the server took it");
  const revoked = sock.sent.filter((m) => m.type === "cert_revoke").map((m) => m.serial).sort();
  assert.deepEqual(revoked, [OLD, passOf(second).serial].sort(), "the old pass and the unanswered one are withdrawn; the refused one never stood");
  await handle({ type: "full_user_list", users: memberUsers() });
  await settle();
  assert.equal(putsToAnn().length, 3, "nothing more is owed");
});

test("10l: a contact request's pass, and the follow, count only once the server took it", async () => {
  const t = await loadChat();
  const { ctx, sock, store, appended, handle, fn } = t;
  const answerWaits = [];
  ctx.setTimeout = (cb, ms) => { if (ms === 30000) answerWaits.push(cb); return 0; };
  const said = () => appended.map(textOf).join("\n");
  const requestTo = (k) => sock.sent.filter((m) => m.type === "dm_put" && m.to === k && m.contact_request === true);
  const requestPass = (k) => fp.friendPassParse(reach.contactRequestParse(JSON.parse(opened(requestTo(k)[0]).plain).text).pass);

  // Taken.
  assert.equal(await fn("sendContactRequest")(ANN), true);
  const toAnn = requestTo(ANN)[0];
  assert.deepEqual(store.certsSent[ANN] || [], [], "not given until the server says so");
  assert.ok(!store.following.has(ANN), "nor do I follow them yet");
  assert.equal(selfCopiesOf(sock).length, 0, "and my other devices are not told");
  assert.ok(typeof toAnn.ref === "string" && /^[A-Za-z0-9_-]{1,64}$/.test(toAnn.ref), "the request carries a ref");
  assert.ok(!said().includes("Contact request sent."), "nor is it called sent");
  await handle({ type: "dm_put_ok", ref: toAnn.ref });
  await settle();
  assert.deepEqual(store.certsSent[ANN], [{ serial: requestPass(ANN).serial, may: DEFAULT_MAY }], "the pass it carried is given");
  assert.ok(store.following.has(ANN), "I follow them");
  assert.equal(selfCopiesOf(sock).length, 1, "my other devices are told");
  assert.ok(said().includes("Contact request sent."), "and it is called sent");

  // Refused (the burst limit).
  assert.equal(await fn("sendContactRequest")(BEN), true);
  await handle({ type: "dm_put_refused", ref: requestTo(BEN)[0].ref, reason: "rate" });
  await settle();
  assert.deepEqual(store.certsSent[BEN] || [], [], "refused: nothing is given");
  assert.ok(!store.following.has(BEN), "nor followed");
  assert.equal(selfCopiesOf(sock).length, 1, "nor told to my other devices");
  assert.ok(said().includes("Your contact request to Ben was not delivered."), "and it says so");
  assert.ok(!sock.sent.some((m) => m.type === "cert_revoke"), "nothing is withdrawn");

  // Unanswered.
  assert.equal(await fn("sendContactRequest")(CY), true);
  answerWaits[answerWaits.length - 1]();
  await settle();
  assert.deepEqual(store.certsSent[CY] || [], [], "unanswered: nothing is given");
  assert.ok(!store.following.has(CY), "nor followed");
  assert.ok(said().includes("This server did not say whether your contact request to Cy arrived"), "and it says so");
  await handle({ type: "dm_put_ok", ref: requestTo(CY)[0].ref });
  await settle();
  assert.deepEqual(store.certsSent[CY] || [], [], "a late answer records nothing");
  assert.equal(selfCopiesOf(sock).length, 1);
  // But the server may have stored it, and then Cy holds my pass: a message from Cy that the relay
  // let through on it is not dropped as a stranger's (Messages: Friends).
  await handle({ type: "reach_settings", settings: { message: "friends", call: "chosen", trade: "friends" } });
  await handle({ type: "dm_new", id: 91, content: dmEnvelope(CY, "yes, let us talk") });
  await settle();
  assert.equal(store.conversation(CY).length, 1, "Cy's reply on the pass I may have given is kept");
  // And Block withdraws that pass, though it was never counted as given.
  const cySerial = requestPass(CY).serial;
  sock.sent.length = 0;
  assert.equal(await fn("blockKey")(CY), true);
  await settle();
  assert.ok(sock.sent.some((m) => m.type === "cert_revoke" && m.serial === cySerial), "Block withdraws the pass that may be standing");
});

// Red first, 2026-10-10 (10l). The three 10l tests were run against web/ as at b46441843 (before
// 10l) through HOS_WEB_DIR and seen failing there: "not given until the server says so: the old
// pass is still the record", "refused: nothing is recorded" and "not given until the server says
// so" (each pass was recorded the moment it was sent); so did the three earlier tests given a
// 10l assertion ("not called sent before the server says so", "no self-copy before the server's
// answer", "still updating until the server answers"). Then one break at a time in a copy of the
// fixed web/:
//  chat-social.js: the sweep without its re-issue loop: "the next sweep sends Ann's pass again";
//    friendTicks reading the record instead of the choice: "the ticks show my choice meanwhile"
//    and "refused: the ticks stay as I chose"; a refusal recorded as given: "refused: nothing is
//    recorded" and "refused: nothing is given".
//  chat-dm-store.js: passTaken not withdrawing the passes still waiting: "the old pass and the
//    unanswered one are withdrawn; the refused one never stood".
//  app.js: dm_put_ok not routed: every test that answers failed (the 10l re-issue test at "given
//    once the server took it").
//  chat-privacy.js: reachAllowsFrom reading only the passes on record: "Cy's reply on the pass I
//    may have given is kept".

test("10l: a tick taken away withdraws the passes allowing it at once; the friend stays on the list", async () => {
  const { sock, store, handle, fn } = await loadChat();
  store.setFollowing(ANN, true);
  store.setFollower(ANN, true);
  const OLD = "00112233445566778899aabbccddeeff";
  store.recordPassSent(ANN, OLD, WITH_CALL);
  const putsToAnn = () => sock.sent.filter((m) => m.type === "dm_put" && m.to === ANN);
  const row = () => fn("safetyModel")().chosen.find((c) => c.key === ANN);

  assert.equal(await fn("setFriendTick")(ANN, "call", false), true, "Call unticked: the new pass goes");
  await settle();
  const [put] = putsToAnn();
  assert.equal(passOf(put).may, DEFAULT_MAY, "without calls");
  assert.ok(sock.sent.some((m) => m.type === "cert_revoke" && m.serial === OLD), "the pass allowing calls is withdrawn at once, before any answer");
  assert.equal(store.certSentTo(ANN), false, "so none stands meanwhile");
  assert.ok(row() && row().ticks.call === false && row().updating === true, "Ann stays on the list, Call unticked, held still");

  await handle({ type: "dm_put_refused", ref: put.ref, reason: "rate" });
  await settle();
  assert.ok(row() && row().ticks.call === false && row().updating === false, "refused: still on the list, Call still unticked");
  assert.equal(await fn("setFriendTick")(ANN, "call", false), true, "and a tick there still answers");

  // The next sweep sends the same may again, and this time the server takes it.
  await handle({ type: "full_user_list", users: memberUsers() });
  await settle();
  const again = putsToAnn()[1];
  assert.ok(again && passOf(again).may === DEFAULT_MAY, "the next sweep sends a pass without calls");
  await handle({ type: "dm_put_ok", ref: again.ref });
  await settle();
  assert.deepEqual(store.certsSent[ANN], [{ serial: passOf(again).serial, may: DEFAULT_MAY }], "given once the server took it");
});

// ── 10m: passes across my own devices (2026-10-10) ──────────────────────
// docs/design/blocking-and-safe-mode.md 10m. R2: a re-issue that takes something away sends its
// self-copy at once, before the withdrawals, and the server's answer sends it no second time; my
// own echo of one the server refused is not adopted here. R3: a pass withdrawn by my other device
// leaves the friend marked "changed on my other device", and this tab gives them no pass by
// itself until that device's choice arrives or the person decides here. R7: a pass refused for
// "reach" is not sent again by itself this session. R8 is the "Groups" lines of the receiving
// test above. Seen red: see the list at the end of this block.

/** A mutual follow, as the page and its store know one. */
async function befriendBoth(t, key) {
  t.store.setFollowing(key, true);
  t.store.setFollower(key, true);
  vm.runInContext("(k) => { myFollowing.add(k); myFollowers.add(k); }", t.ctx)(key);
}
/** A control DM from `from` to me, as their client seals it, carrying `cert`. */
function controlEnvelope(from, text, cert, ts = 1760000006000) {
  const sig = b64(`hum/dm/v2\n${from}\n${ME}\n${ts}\n${text}`);
  const inner = JSON.stringify({ v: 2, from, to: ME, ts, text, sig, cert });
  return JSON.stringify({ v: 2, ek_ct_b64: b64(MY_KYBER), nonce_b64: "AAAA", ct_b64: b64(inner) });
}
const NO_TICKS = { message: false, call: false, trade: false };

test("10m R2: an untick sends its self-copy at once, before the withdrawals; dm_put_ok sends it no second time", async () => {
  const { sock, store, handle, fn } = await loadChat();
  const OLD = "00112233445566778899aabbccddeeff";
  store.recordPassSent(ANN, OLD, WITH_CALL);
  const putsToAnn = () => sock.sent.filter((m) => m.type === "dm_put" && m.to === ANN);

  assert.equal(await fn("setFriendTick")(ANN, "call", false), true, "Call unticked: the new pass goes");
  await settle();
  const [put] = putsToAnn();
  const self = selfCopiesOf(sock);
  assert.equal(self.length, 1, "its self-copy goes at once, with the pass");
  assert.equal(JSON.parse(opened(self[0]).plain).cert, JSON.parse(opened(put).plain).cert, "a copy of the new pass");
  assert.equal(opened(self[0]).sealedTo, MY_KYBER, "sealed to me");
  const revoke = sock.sent.find((m) => m.type === "cert_revoke" && m.serial === OLD);
  assert.ok(revoke, "the pass allowing calls is withdrawn at once");
  assert.ok(sock.sent.indexOf(put) < sock.sent.indexOf(self[0]) && sock.sent.indexOf(self[0]) < sock.sent.indexOf(revoke),
    "the new pass, then its self-copy, then the withdrawal: my other devices hear the new choice first");
  assert.equal(store.certSentTo(ANN), false, "the record changes only on dm_put_ok");

  await handle({ type: "dm_put_ok", ref: put.ref });
  await settle();
  assert.equal(selfCopiesOf(sock).length, 1, "dm_put_ok does not send the self-copy a second time");
  assert.deepEqual(store.certsSent[ANN], [{ serial: passOf(put).serial, may: DEFAULT_MAY }], "and records the pass then");

  // An added tick keeps its self-copy held until the server took the pass (10l).
  sock.sent.length = 0;
  assert.equal(await fn("setFriendTick")(ANN, "call", true), true);
  await settle();
  const [added] = putsToAnn();
  assert.equal(passOf(added).may, WITH_CALL);
  assert.equal(selfCopiesOf(sock).length, 0, "an added tick holds its self-copy");
  await handle({ type: "dm_put_ok", ref: added.ref });
  await settle();
  assert.equal(selfCopiesOf(sock).length, 1, "until the server took the pass");
});

test("10m R2: my own echo of an untick the server refused is not adopted here, even after a reload; the next sweep sends it again", async () => {
  const t = await loadChat();
  const { sock, store, handle, fn } = t;
  await befriendBoth(t, ANN);
  const OLD = "00112233445566778899aabbccddeeff";
  store.recordPassSent(ANN, OLD, WITH_CALL);
  const putsToAnn = () => sock.sent.filter((m) => m.type === "dm_put" && m.to === ANN);

  assert.equal(await fn("setFriendTick")(ANN, "call", false), true);
  await settle();
  const [put] = putsToAnn();
  const cert = JSON.parse(opened(put).plain).cert;
  await handle({ type: "dm_put_refused", ref: put.ref, reason: "rate" });
  await settle();
  // The self-copy of that pass (it may have gone with it) comes back to this tab from my mailbox.
  await handle({ type: "dm_new", id: 95, content: selfEnvelope(ANN, CTL_FRIEND_CERT, cert, 1760000007000) });
  await settle();
  assert.equal(store.certSentTo(ANN), false, "my own echo of a pass the server refused is not counted as given");
  assert.equal(store.passIntent[ANN], DEFAULT_MAY, "my choice is still owed to her");

  // Across a reload the refusal is still known.
  await settle();
  assert.ok(await store.init(ME, "localhost"), "the store loads again");
  await handle({ type: "dm_new", id: 96, content: selfEnvelope(ANN, CTL_FRIEND_CERT, cert, 1760000008000) });
  await settle();
  assert.equal(store.certSentTo(ANN), false, "nor after a reload");

  await handle({ type: "full_user_list", users: memberUsers() });
  await settle();
  const again = putsToAnn()[1];
  assert.ok(again && passOf(again).may === DEFAULT_MAY, "the next sweep sends a pass without calls again");
});

test("10m R3: a pass my other device withdrew is not replaced with the defaults", async () => {
  const t = await loadChat();
  const { sock, store, handle, fn } = t;
  await befriendBoth(t, BEN);
  const OLD = "00112233445566778899aabbccddeeff";
  store.recordPassSent(BEN, OLD, DEFAULT_MAY);
  const putsToBen = () => sock.sent.filter((m) => m.type === "dm_put" && m.to === BEN && JSON.parse(opened(m).plain).text === CTL_FRIEND_CERT);
  const row = () => fn("safetyModel")().chosen.find((c) => c.key === BEN);

  // The review's case: the desktop app unticks Trade for Ben, withdrawing this pass at once; its
  // new pass is refused, so all this tab hears is the relay confirming the withdrawal.
  await handle({ type: "cert_revoked", to: ME, serial: OLD });
  await settle();
  assert.equal(store.certSentTo(BEN), false, "the withdrawn pass leaves the record");
  await handle({ type: "full_user_list", users: memberUsers() });
  await settle();
  assert.deepEqual(putsToBen(), [], "this tab sends Ben no pass: not one with trade, not one at all");
  assert.ok(store.passChangedOnOtherDevice(BEN), "Ben is marked: changed on my other device");
  // His follow arriving again sends none either.
  await handle({ type: "dm_new", id: 70, content: dmEnvelope(BEN, CTL_FOLLOW, 1760000001000) });
  await settle();
  assert.deepEqual(putsToBen(), [], "nor does his follow");
  const r = row();
  assert.ok(r, "Ben stays on People I choose");
  assert.equal(r.updating, true, "drawn as updating his pass");
  assert.equal(r.held, false, "with his ticks free to change");
  assert.deepEqual(r.ticks, NO_TICKS, "and none drawn as given");
  assert.ok(fn("safetyPanelHtml")(fn("safetyModel")()).includes("(updating their pass)"));

  // Kept across a reload.
  await settle();
  assert.ok(await store.init(ME, "localhost"));
  assert.ok(store.passChangedOnOtherDevice(BEN), "the mark is kept across a reload");
  await handle({ type: "full_user_list", users: memberUsers() });
  await settle();
  assert.deepEqual(putsToBen(), [], "and still no pass goes");

  // The desktop's pass, echoed: adopted, the mark clears, his ticks are the desktop's choice.
  const NEW = "ffeeddccbbaa99887766554433221100";
  const may = "invite,message,voice_message";
  const cert = fp.friendPassJson(NEW, may, b64(fp.friendPassPreimage(SERVER, ME, BEN, NEW, may)));
  await handle({ type: "dm_new", id: 71, content: selfEnvelope(BEN, CTL_FRIEND_CERT, cert) });
  await settle();
  assert.equal(store.passChangedOnOtherDevice(BEN), false, "the echo clears the mark");
  assert.deepEqual(fn("friendTicks")(BEN), { message: true, call: false, trade: false }, "Trade stays away");
  assert.equal(row().updating, false);
  await handle({ type: "full_user_list", users: memberUsers() });
  await settle();
  assert.deepEqual(putsToBen(), [], "and nothing more is owed");
});

test("10m R3: the mark clears when the person ticks, follows or accepts on this device; my own withdrawal marks no one", async () => {
  const t = await loadChat();
  const { sock, store, handle, fn } = t;
  for (const k of [ANN, BEN, CY]) await befriendBoth(t, k);
  const passTo = (peer) => {
    const put = [...sock.sent].reverse().find((m) => m.type === "dm_put" && m.to === peer && JSON.parse(opened(m).plain).text === CTL_FRIEND_CERT);
    return put ? passOf(put) : null;
  };
  const withdrawnElsewhere = async (peer, serial) => {
    store.recordPassSent(peer, serial, DEFAULT_MAY);
    await handle({ type: "cert_revoked", to: ME, serial });
    await settle();
  };

  // A tick: the pass carries exactly what is ticked, and the mark goes.
  await withdrawnElsewhere(CY, "c1".repeat(16));
  assert.equal(await fn("setFriendTick")(CY, "message", true), true, "a tick for a marked friend sends a pass");
  await settle();
  assert.equal(passTo(CY) && passTo(CY).may, "invite,message,voice_message", "Message only: not the defaults' Trade");
  assert.equal(store.passChangedOnOtherDevice(CY), false, "and clears the mark");

  // A follow made here.
  await withdrawnElsewhere(ANN, "a1".repeat(16));
  await fn("setFollowLocal")(ANN, true);
  await settle();
  assert.ok(passTo(ANN), "following again here sends a pass");
  assert.equal(store.passChangedOnOtherDevice(ANN), false);

  // Accepting a request from them here.
  await withdrawnElsewhere(BEN, "b1".repeat(16));
  await handle({ type: "dm_new", id: 72, content: requestEnvelope(BEN, "Ben", passFrom(BEN), 1760000002000) });
  await settle();
  const [req] = store.contactRequestList();
  assert.ok(req && req.key === BEN, "Ben's request is listed");
  assert.equal(await fn("acceptContactRequest")(req.id), true);
  await settle();
  assert.ok(passTo(BEN), "accepting sends a pass");
  assert.equal(store.passChangedOnOtherDevice(BEN), false);

  // A withdrawal this device made itself (Unfollow) marks no one when the relay confirms it.
  await answerPuts(t);
  const mine = store.certsSent[ANN][0].serial;
  await fn("setFollowLocal")(ANN, false);
  await handle({ type: "cert_revoked", to: ME, serial: mine });
  await settle();
  assert.equal(store.passChangedOnOtherDevice(ANN), false, "my own Unfollow marks no one");
  assert.ok(!fn("safetyModel")().chosen.some((c) => c.key === ANN), "and she leaves the list");
});

test("10m R7: a pass refused for reach is not sent again by itself; the offer shows once", async () => {
  const t = await loadChat();
  const { ctx, sock, store, appended, handle, fn } = t;
  await befriendBoth(t, ANN);
  await befriendBoth(t, BEN);
  // The offer comes at most once a minute per person: let the minutes pass between member lists.
  let shift = 0;
  const RealDate = Date;
  ctx.Date = class extends RealDate { static now() { return RealDate.now() + shift; } };
  const putsTo = (peer) => sock.sent.filter((m) => m.type === "dm_put" && m.to === peer && JSON.parse(opened(m).plain).text === CTL_FRIEND_CERT);
  const offersFor = (name) => appended.filter((el) => textOf(el).includes(reach.REACH_REFUSED_MESSAGE) && textOf(el).includes(`Not delivered to ${name}.`)).length;
  // Ann's and Ben's settings say Friends and neither has given me a pass: the relay refuses each
  // pass by their setting, saying `reach_refused` and then `dm_put_refused` with reason "reach".
  const answered = new Set();
  const relayRefusesByReach = async () => {
    for (const m of [...sock.sent]) {
      if (m.type !== "dm_put" || typeof m.ref !== "string" || answered.has(m.ref)) continue;
      answered.add(m.ref);
      await handle({ type: "reach_refused", kind: "message", to: m.to });
      await handle({ type: "dm_put_refused", ref: m.ref, reason: "reach" });
    }
    await settle();
  };
  const memberList = async () => {
    shift += 61000;
    await handle({ type: "full_user_list", users: memberUsers() });
    await settle();
    await relayRefusesByReach();
  };

  await memberList();
  assert.equal(putsTo(ANN).length, 1, "the first member list sends Ann a pass");
  assert.equal(offersFor("Ann"), 1, "refused: the offer shows");
  await memberList();
  await memberList();
  assert.equal(putsTo(ANN).length, 1, "it is not sent again by itself");
  assert.equal(offersFor("Ann"), 1, "and the offer shows once, not on every member list");

  // Her pass reaches me: mine goes on the next member list, presenting hers.
  const hers = passFrom(ANN);
  await handle({ type: "dm_new", id: 61, content: controlEnvelope(ANN, CTL_FRIEND_CERT, hers) });
  await settle();
  assert.equal(store.certFor(ANN), hers, "her pass is kept");
  shift += 61000;
  await handle({ type: "full_user_list", users: memberUsers() });
  await settle();
  assert.equal(putsTo(ANN).length, 2, "then mine goes again");
  assert.equal(putsTo(ANN)[1].friend_cert, hers, "presenting hers");

  // Following again here lets it go too.
  assert.equal(putsTo(BEN).length, 1);
  await fn("setFollowLocal")(BEN, true);
  await settle();
  assert.equal(putsTo(BEN).length, 2, "following again here sends Ben's pass again");
});

// Red first, 2026-10-10 (10m). Each test above was run against web/ as at bf8c4c582 (before 10m)
// through HOS_WEB_DIR and seen failing there:
//  R2: "its self-copy goes at once, with the pass" (it was held until dm_put_ok); "my own echo of
//    a pass the server refused is not counted as given" (adoptEchoedPass took it, and the sweep
//    stopped sending her the pass).
//  R3: "this tab sends Ben no pass: not one with trade, not one at all" (the sweep gave him the
//    defaults, Trade included); "a tick for a marked friend sends a pass" (he had left the list).
//  R7: "it is not sent again by itself" (every member list sent it, and the offer came back).
//  R8: "a DM the server let through under Groups is kept, whatever this page's group list says"
//    (in the receiving test above; the page's stale group list turned it into a request).

// 10m R3, the desktop app's rule (src/net/dm_store.rs withdrawal_confirmed): a pass this tab still
// has on its way to Ben was minted without my other device's choice, so when that device's
// withdrawal leaves him no standing pass, it goes too and Ben is marked. Seen red 2026-10-10
// against the merged web half before this (a pass on its way kept Ben unmarked): "Ben is marked
// though a pass of mine was on its way".
test("10m R3: a pass of mine still on its way goes too when my other device withdrew his", async () => {
  const t = await loadChat();
  const { sock, store, handle } = t;
  await befriendBoth(t, BEN);
  const OLD = "00112233445566778899aabbccddeeff";
  const ONWAY = "0f0e0d0c0b0a09080706050403020100";
  store.recordPassSent(BEN, OLD, DEFAULT_MAY);
  store.passSending(BEN, ONWAY, WITH_CALL);
  sock.sent.length = 0;
  await handle({ type: "cert_revoked", to: ME, serial: OLD });
  await settle();
  assert.ok(store.passChangedOnOtherDevice(BEN), "Ben is marked though a pass of mine was on its way");
  assert.equal((store.passesUnsure[BEN] || []).length, 0, "and that pass is no longer waiting");
  assert.ok(sock.sent.some((m) => m.type === "cert_revoke" && m.serial === ONWAY), "it is withdrawn at once");
  await handle({ type: "full_user_list", users: memberUsers() });
  await settle();
  assert.ok(!sock.sent.some((m) => m.type === "dm_put" && m.to === BEN), "and no pass goes to him by itself");
});
