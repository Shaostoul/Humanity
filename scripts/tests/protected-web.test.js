// The protected setup in the web chat client (step G, 2026-10-10,
// docs/design/blocking-and-safe-mode.md 10h), mirroring the desktop app.
//
// Run: node --test scripts/tests/protected-web.test.js   (in `just rig-tests`)
//
// The page scripts run as they do in the browser, in one shared global scope (node:vm), with the
// DOM replaced by a stub that keeps what is written to it (as in warnings-web.test.js): the real
// bip39-english.js, friend-pass.js, reach.js, block.js, report.js, warnings.js, protected.js,
// crypto.js, chat-dm-store.js (over a stand-in IndexedDB), app.js, chat-messages.js, chat-dms.js,
// chat-social.js, chat-groups-p2p.js, chat-ui.js, chat-voice-rooms.js, chat-voice-calls.js,
// chat-profile.js, chat-privacy.js, chat-reports.js, chat-warnings.js, chat-protected.js,
// chat-onboarding.js and chat-p2p.js, in index.html's order. The settings page's backup and
// recovery phrase section (web/pages/settings-app.js) runs on its own stub page (test 19). Stand-ins: the Dilithium and Kyber primitives ("signing"
// returns the signed words, "sealing" base64s the plaintext and names the key it was sealed to, so
// a test reads exactly what would travel), BLAKE3 (SHA-256 here; no hash is checked), and the two
// modules chat-groups-p2p.js imports. The PIN verifier is NOT a stand-in: it is real PBKDF2-SHA-256
// at 600,000 rounds through WebCrypto, so each PIN check costs a quarter of a second. The preset is
// always the shipped data/gui/safety_presets.json (one test serves an altered copy to prove the
// words come from the file). HOS_WEB_DIR points the test at another copy of web/ (used to see each
// test red).
//
// What it proves (10h's Proof list, the web items, and the rest of 10h's web surface):
//  1. The preset is read from the real data file; its reach values are the frame turning it on
//     sends; every word on screen comes from the file (an altered file shows its own words, and no
//     sentence of the file is typed into the code).
//  2. The PIN verifier: PBKDF2-SHA-256, 600,000 rounds, a random 16-byte salt (recomputed here
//     with Node's own PBKDF2); the right PIN opens, a wrong one does not; the PIN itself is
//     nowhere in what storage holds.
//  3. Three wrong PINs in a row wait 60 seconds (the right PIN is refused meanwhile, unchecked),
//     kept across a reload; after the wait the right PIN opens.
//  4. Every locked action is refused without the PIN (and when the prompt is cancelled, and with a
//     wrong PIN) and allowed with it: a "Who can reach me" row, a "People I choose" tick, Follow,
//     Follow back, accepting a contact request, sending one, a friend code, joining a group by
//     ticket, joining a voice room, turning warnings off, turning the pictures rule down, showing
//     public rooms, changing the PIN, turning the setup off.
//  5. Every never-locked action runs without it: Block, Report, Unfollow, leaving a group,
//     leaving a voice room, muting, ignoring a request, and turning warnings, the pictures rule
//     or the room filter back up.
//  6. The review step lists friends (everyone holding a pass from me), groups and voice rooms,
//     each with Remove: a friend is unfollowed and their pass withdrawn, a group and a room left.
//     Nothing is kept before the last step.
//  7. Turning it on sends exactly one `reach_set` with the preset's values and nothing else (the
//     frames recorded), no flag, no fetch; not connected, it does not turn on.
//  8. Public rooms: only read-only rooms are listed, with the preset's line; a hidden room's posts
//     are dropped before anything draws or notifies them; it cannot be opened; showing them needs
//     the PIN; the federation list goes with them.
//  9. Pictures and files from someone who is not a friend are not shown, one line in place of
//     each (posts, group messages, DM attachments, link previews); a friend's and my own still
//     click to load; the rule turned down shows them again.
// 10. Warnings show for friends too while it is on.
// 11. Contact requests show "Accept (needs the PIN)" and Ignore.
// 12. The always-visible line, at the top of Settings > Safety and above the DM list; the
//     routes line in Settings > Safety.
// 13. Every string the feature shows is held to the preset's avoid_words (and has no em dash).
// 14. Forgot the PIN: the right recovery phrase lets a new PIN be chosen, a wrong one does not.
// 15. The state stays on this device: not in the self notes Block sends, the account export, the
//     encrypted local store, or the settings backup (which cannot import it either).
// 16. A pass goes only to a friend the PIN holder let be one: a follow made before the setup and
//     followed back after it gives no pass; Unfollow and Block mean the PIN again next time.
// 17. index.html loads protected.js before chat-privacy.js and chat-protected.js after the scripts
//     it draws into; every gate sits before the send it guards; the never-locked paths have none.
// 18. Showing the recovery phrase needs the PIN while it is on (`show_phrase`, mirroring the desktop
//     app): the Seed button and /recovery, Safety's own Show the recovery phrase, every copy of the
//     identity it comes from (the encrypted backup file, the identity file, the device-link code),
//     and the onboarding guide, which shows no words while it is on. All free while off.
// 19. The settings page, which does not load the chat scripts, reads the setup from the same
//     storage key the same way (fail closed) and refuses View Recovery Phrase and the backup file
//     while it is on, with one line saying where the PIN opens them (the preset's own words when
//     the file has them).
// 20. Forgot the PIN takes only the phrase of the identity the setup was turned on under: another
//     identity on the same device is refused even with its own correct phrase, and a missing or
//     damaged stored identity falls back to the identity in use (the desktop app's rule).
// 21. The PIN opens one action, the one it was asked for: an action that returns before using it,
//     or throws, leaves nothing open for the next one, and it never opens another kind.
// 22. Starting a group and making (or copying) an invite ticket need the PIN (in test 4's list);
//     the typed /friend-code and /redeem, in the composer and in a thread, ask as the buttons do;
//     a redeem made with the PIN follows the person the relay names with no second PIN.
// 23. My own Unfollow, echoed from another of my devices, takes them off the approved list.
// 24. The review lists mutual follows still owed a pass, and keeping them approves them.
// 25. While it is on, warnings stay on whatever the identity's own switch says; only the PIN turns
//     them off, and that choice is kept on this device.
//
// Red first, 2026-10-10: each mutation made in a copy of web/, this test run against it with
// HOS_WEB_DIR, and seen failing with the assertion named (the others passing unless said):
//  1: chat-protected.js fetching '/data/gui/safety_preset.json' (a typo): "the page asked for
//     /data/gui/safety_presets.json" (and 13 more tests, which all need the words to turn it on).
//  2: protected.js PROTECTED_PIN_ITERATIONS 100000: "600,000 rounds, the vaults' strength"; the
//     setup keeping the typed PIN beside its verifier: "the PIN is not in what storage holds".
//  3: protectedAfterWrong never starting the wait: "the third: 60 seconds".
//  4: joinVoiceRoom's gate taken out: "joining a voice room: the PIN prompt opens" (and test 17);
//     setFollowLocal's: "Follow: the PIN prompt opens" (and tests 16, 17); chooseReachAudience's:
//     "a Who can reach me row: the PIN prompt opens" (and tests 2, 3, 14, 17); protectedTurnOff's:
//     "turning the setup off: the PIN prompt opens".
//  5: leaveVoiceRoom asking for the PIN: "leaving a voice room runs" (and test 17).
//  6: the review's group Remove not leaving the group: "Remove for a group leaves it".
//  7: apply also sending a privacy_update: "exactly one frame: an ordinary reach_set with the
//     preset's values".
//  8: protectedChannelShown listing every room: "on: read-only rooms only"; the frame screen
//     taken out of chat-protected.js: "not drawn".
//  9: formatBody without its protectedBodyHtml step: "on: a non-friend's pictures and files are gone".
// 10: drawMessageWarnings checking a friend against the friends' entries only: "on: a friend's
//     message gets the strangers' entries too".
// 11: contactRequestsHtml always saying Accept: "on: Accept (needs the PIN)".
// 12: renderDmList without the status line: "the line is above the DM list" (and test 1's "the
//     always-visible line is the file's").
// 13: the Turn off label reading "Turn off the kid safe setup": "no word from the finding's list,
//     anywhere it shows".
// 14: protectedPhraseMatches answering true: "another identity's phrase does not".
// 15: settings-app.js importData without its IMPORT_NEVER_KEYS check: "except the setup, which a
//     file can never turn off or replace".
// 16: sendFriendCertTo without its protectedPassAllowed line: "no pass goes to Dan: following back
//     after the setup is not a friend the PIN holder let be one" (and test 17).
// 17: protected.js loaded after chat-privacy.js in index.html: "and before chat-privacy.js".
// Added 2026-10-10 (the recovery phrase and the setup's identity), the same way:
// 18: openSeedPhraseModal's gate taken out: "on: the PIN prompt opens"; protectedActionLocked
//     locking show_phrase while off: "free while off" (and test 4's own "is free while off");
//     Safety's button not wired: "Safety's button asks for the PIN"; openEncryptedBackupModal's
//     gate taken out: "on: the encrypted backup file asks for the PIN"; the downloadIdentityBackup
//     wrap's gate: "on: the identity file asks for the PIN"; the openLinkDeviceModal wrap's gate:
//     "on: the device-link code asks for the PIN"; the onboarding guide never locked: "on: the
//     guide's phrase step is locked".
// 19: settingsOpenSeed without its refusal: "on: the phrase is not even made"; settingsOpenBackup
//     without it: "on: no backup file either"; settingsProtectedOn reading what it cannot parse as
//     off: "the same answer as protected.js for \"null\"".
// 20: protectedForgotSubmit without the identity check: "another identity: even its own correct
//     phrase is refused"; protectedSetupApply not handing over the identity: "turning it on keeps
//     the identity in use" (and test 14's "the right phrase lets a new PIN be chosen"); a
//     missing stored identity refusing every phrase (fail closed, which would lock a damaged setup
//     for good; replaced 2026-10-10 by the desktop app's rule): "a missing identity: the phrase of
//     the identity in use opens it".
// Added 2026-10-10 (the batch review), each new test run against web/ as at a3ca8f167 (before the
// fix) and seen failing there with the assertion named:
// 4 (two new rows): "starting a group: the PIN prompt opens" (createP2pGroup had no gate).
// 21: "a second action asks for the PIN again, though the first returned without using it" (the
//     PIN held a 10-second grant that an action returning early left open).
// 22: "P.protectedTypedCommand is not a function" (the typed commands went to the relay as chat).
// 23: "an Unfollow echoed from another device takes them off".
// 24: "both friends are listed, the one owed a pass too".
// 25: "the identity's own switch does not turn them off while the setup is on".
// 17 (new rows): "chat/chat-privacy.js: async function chooseFriendTick asks
//     protectedNeedsPin('reach_tick') before setFriendTick(" and the rows for createP2pGroup,
//     createP2pInvite, redeemFriendCode and the typed commands.
// And one break at a time in a copy of the fixed web/: the typed routing taken out of chat-ui.js:
// "/friend-code: the PIN prompt opens"; createP2pInvite's gate taken out: "making or copying a
// group's invite ticket: the PIN prompt opens"; the redeem's approval taken out: "no second PIN
// for the friend the code named"; sendThreadReply's typed check taken out: "typed in a thread: the
// PIN prompt opens"; protectedAskThen not clearing its grant when the run returns: "a second
// action asks for the PIN again, though the first returned without using it"; setMessageWarningsOn
// not keeping warnings_off: "with it, they are off".

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const nodeCrypto = require("node:crypto");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");
const ROOT = path.join(__dirname, "..", "..");
const P = require(path.join(WEB, "shared", "protected.js"));
const reach = require(path.join(WEB, "shared", "reach.js"));
const fp = require(path.join(WEB, "shared", "friend-pass.js"));
const W = require(path.join(WEB, "shared", "warnings.js"));
const report = require(path.join(WEB, "shared", "report.js"));
const SHIPPED_PRESETS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "gui", "safety_presets.json"), "utf8"));
const SHIPPED_WARNINGS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "warnings.json"), "utf8"));
const SHIPPED_REASONS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "report_reasons.json"), "utf8"));
const PRESET = P.protectedPresetFrom(SHIPPED_PRESETS);
const clone = (x) => JSON.parse(JSON.stringify(x));

// ── The stub page (as warnings-web.test.js) ──────────────────────────────

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

const escapeText = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const escHtml = (s) => String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
const NOTHING_YET = new Set([".edit-area", ".edited-marker", ".block-indicator"]);

function fakeDom(state) {
  const all = [];
  function el(tag) {
    const classes = new Set();
    const own = {
      tag, children: [], parts: new Map(), style: {}, dataset: {}, attrs: {},
      className: "", textContent: "", value: "", disabled: false, checked: false, removed: false,
    };
    own.classList = {
      add: (c) => classes.add(c), remove: (c) => classes.delete(c), contains: (c) => classes.has(c),
      toggle: (c) => (classes.has(c) ? (classes.delete(c), false) : (classes.add(c), true)),
    };
    own.appendChild = (c) => { own.children.push(c); return c; };
    own.setAttribute = (k, v) => { own.attrs[k] = String(v); };
    own.getAttribute = (k) => (k in own.attrs ? own.attrs[k] : null);
    own.querySelector = (sel) => {
      if (NOTHING_YET.has(sel)) return null;
      if (!own.parts.has(sel)) own.parts.set(sel, el(sel));
      return own.parts.get(sel);
    };
    own.querySelectorAll = () => [];
    own.addEventListener = () => {};
    own.remove = () => { own.removed = true; };
    own.focus = () => {};
    own.after = () => {};
    const p = new Proxy(own, {
      get(t, prop) {
        if (prop === "then") return undefined;
        if (prop === "innerHTML") return "innerHTML" in t ? t.innerHTML : escapeText(t.textContent);
        if (prop in t) return t[prop];
        return anything();
      },
      set(t, prop, v) { t[prop] = v; return true; },
    });
    all.push(p);
    return p;
  }
  const byId = new Map();
  const kept = (id) => { if (!byId.has(id)) byId.set(id, el("#" + id)); return byId.get(id); };
  return new Proxy({}, {
    get(_t, prop) {
      if (prop === "createElement") return el;
      if (prop === "getElementById") return kept;
      if (prop === "body") return kept("__body");
      if (prop === "querySelector") return () => anything();
      if (prop === "querySelectorAll") {
        return (sel) => {
          if (sel === ".msg-warning") return all.filter((e) => String(e.className).split(/\s+/).includes("msg-warning"));
          if (sel === ".message[data-from]") return state.appended.filter((e) => e && e.dataset && e.dataset.from);
          return [];
        };
      }
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
    _map: m,
  };
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
  return { open: () => req(db), stores };
}

function fakeSocket() {
  return { readyState: 1, sent: [], send(s) { this.sent.push(JSON.parse(s)); }, close() {} };
}

function FakePeerConnection() {
  this.setRemoteDescription = async () => {};
  this.setLocalDescription = async () => {};
  this.createAnswer = async () => ({ type: "answer", sdp: "v=0" });
  this.createOffer = async () => ({ type: "offer", sdp: "v=0" });
  this.createDataChannel = () => ({ readyState: "connecting", send() {} });
  this.addTrack = () => {};
  this.addIceCandidate = async () => {};
  this.close = () => {};
}

const ME = "a1".repeat(32);
const ANN = "b2".repeat(32);
const BEN = "c3".repeat(32);
const CY = "d4".repeat(32);
const DAN = "f6".repeat(32);
const SERVER = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
const MY_KYBER = "my-kyber";
const kyberOf = (k) => "kyber-" + k.slice(0, 4);
const GROUP = "g1";
const MAY = "invite,message,trade,voice_message";
const S1 = "11".repeat(16);
const S2 = "22".repeat(16);
const PIN = "90817263";
const OTHER_PIN = "55512340";
const SEED = Uint8Array.from({ length: 32 }, (_, i) => (i * 7 + 3) & 0xff);
const OTHER_SEED = Uint8Array.from({ length: 32 }, (_, i) => (i * 11 + 5) & 0xff);
const CHANNELS = [
  { id: "general", name: "general", read_only: false },
  { id: "announcements", name: "announcements", read_only: true },
  { id: "dev", name: "dev", read_only: false, voice_enabled: true },
];
const CTL_FOLLOW = "[[hum:follow]]";
const CTL_UNFOLLOW = "[[hum:unfollow]]";
const CTL_FRIEND_CERT = "[[hum:friend-cert]]";

// The page's real asynchronous work is WebCrypto (the store, and the PIN's 600,000 rounds) and the
// stand-in IndexedDB. Counting the WebCrypto calls in flight lets settle() wait until the page is
// idle; a PBKDF2 run outlasts any fixed number of turns, so the bound is time, not turns.
let cryptoInFlight = 0;
let pbkdf2Runs = 0;
const countedSubtle = new Proxy(globalThis.crypto.subtle, {
  get(t, prop) {
    const v = t[prop];
    if (typeof v !== "function") return v;
    return (...args) => {
      cryptoInFlight++;
      if (prop === "deriveBits" && args[0] && args[0].name === "PBKDF2") pbkdf2Runs++;
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
  const until = Date.now() + 20000;
  let idle = 0;
  while (idle < 12 && Date.now() < until) {
    await new Promise((r) => setImmediate(r));
    idle = cryptoInFlight === 0 ? idle + 1 : 0;
  }
}

const b64 = (s) => Buffer.from(s, "utf8").toString("base64");
const unb64 = (s) => Buffer.from(s, "base64").toString("utf8");
const bytesToString = (u) => { let s = ""; for (let i = 0; i < u.length; i++) s += String.fromCharCode(u[i]); return Buffer.from(s, "latin1").toString("utf8"); };
const ok = (json) => Promise.resolve({ ok: true, status: 200, json: async () => json, text: async () => JSON.stringify(json) });

// The two modules chat-groups-p2p.js imports: a join and a leave build a submission that names
// what they are, so a test can read what was posted.
function fakeGroupModules() {
  const obj = {
    parseGroupMsgPayload: (payload) => {
      const j = payload && typeof payload === "object" && !ArrayBuffer.isView(payload) && "epoch" in payload ? payload : JSON.parse(bytesToString(payload));
      return { epoch: j.epoch, nonce: "n", ct: j.text };
    },
    aesGcmDecrypt: async (_key, _nonce, ct) => ct,
    openGroupEpochKey: async () => ({ epoch: 1, epochKey: "K" }),
    verifyObjectSubmission: async (sub) => ({ ok: true, objectId: sub.id, authorPubHex: sub.author, payload: sub.payload, createdAt: sub.created_at }),
    buildGroupMsgV1: async ({ plaintext }) => ({ objectId: "built", submission: { object_type: "group_msg_v1", plaintext } }),
    groupSharesHistory: () => false,
    decodeInviteTicket: (t) => {
      if (t !== "TICKET-OK") throw new Error("bad ticket");
      return { groupId: "g2", inviteId: "i2", secret: "s", groupName: "Hikers" };
    },
    buildGroupJoinV1: async (a) => ({ submission: { object_type: "group_join_v1", group: a.groupId } }),
    buildGroupMemberV1: async (a) => ({ submission: { object_type: "group_member_v1", action: a.action, group: a.groupId } }),
    // Starting a group and minting its invite ticket (both locked while the setup is on).
    buildGroupV1: async (a) => ({ objectId: "g-new", submission: { object_type: "group_v1", name: a.name } }),
    buildGroupEpochKeyV1: async (a) => ({ objectId: "e-new", submission: { object_type: "group_epoch_key_v1", group: a.groupId, epoch: a.epoch } }),
    randomEpochKey: () => "K",
    randomInviteSecret: () => "secret",
    buildGroupInviteV1: async (a) => ({ objectId: "i-new", submission: { object_type: "group_invite_v1", group: a.groupId } }),
    encodeInviteTicket: (t) => "TICKET-" + t.groupId,
  };
  const noble = {
    blake3: {
      create: () => {
        let d = new Uint8Array(0);
        return { update(x) { d = x; return this; }, digest() { const o = new Uint8Array(32); o.set(d.slice(0, 32)); return o; } };
      },
    },
  };
  return { "/shared/pq-object.js": obj, "/shared/vendor/noble-pq.bundle.js": noble };
}

async function loadChat(opts = {}) {
  const state = { appended: [] };
  const fetches = [];
  const posted = [];
  const exportBodies = [];
  const notified = [];
  const serviceWorker = { register: () => Promise.resolve({ scope: "/" }), ready: new Promise(() => {}), controller: null, addEventListener() {} };
  const storage = opts.localStorage || fakeStorage();
  const presets = opts.presets || SHIPPED_PRESETS;
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: fakeDom(state),
    navigator: new Proxy({}, { get: (_t, p) => (p === "serviceWorker" ? serviceWorker : anything()), has: () => true }),
    location: { hash: "", host: "localhost", protocol: "https:", pathname: "/chat", search: "", origin: "https://localhost", reload() {} },
    localStorage: storage,
    sessionStorage: opts.sessionStorage || fakeStorage(),
    indexedDB: fakeIndexedDB(),
    fetch: (url, init) => {
      const u = String(url);
      const method = (init && init.method) || "GET";
      fetches.push({ url: u, method, body: init && init.body });
      if (u.startsWith("/api/federation/servers")) return ok(opts.federated || []);
      if (u === P.PROTECTED_PRESETS_URL) return ok(clone(presets));
      if (u === W.WARNINGS_URL) return ok(clone(SHIPPED_WARNINGS));
      if (u.startsWith(report.REPORT_REASONS_URL)) return ok(clone(SHIPPED_REASONS));
      if (u === "/api/v2/objects" && method === "POST") { posted.push(JSON.parse(init.body)); return ok({ ok: true }); }
      if (u === "/api/account/export") {
        exportBodies.push(init && init.body);
        return Promise.resolve({ ok: false, status: 503, text: async () => "not in tests", json: async () => ({}) });
      }
      return Promise.reject(new Error("no network in tests"));
    },
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
  };
  ctx.window = ctx;
  ctx.self = ctx;
  const modules = fakeGroupModules();
  ctx.__testImport = async (spec) => {
    if (!modules[spec]) throw new Error("no module " + spec);
    return modules[spec];
  };
  vm.createContext(ctx);
  const run = (rel, rewrite) => {
    let src = fs.readFileSync(path.join(WEB, rel), "utf8");
    if (rewrite) src = rewrite(src);
    vm.runInContext(src, ctx, { filename: rel });
  };
  // In index.html's order.
  run("shared/events.js");
  run("chat/bip39-english.js");
  run("shared/friend-pass.js");
  run("shared/reach.js");
  run("shared/block.js");
  run("shared/report.js");
  run("shared/warnings.js");
  run("shared/protected.js");
  run("chat/crypto.js");
  run("chat/chat-dm-store.js");
  run("chat/view/timestampPill.js");
  run("chat/view/messageRow.js");
  run("chat/app.js");
  run("chat/chat-messages.js");
  run("chat/chat-dms.js");
  run("chat/chat-social.js");
  run("chat/chat-groups-p2p.js", (s) => s.replace(/\bimport\(/g, "__testImport("));
  run("chat/chat-ui.js");
  run("chat/chat-voice-rooms.js");
  run("chat/chat-voice-calls.js");
  run("chat/chat-profile.js");
  run("chat/chat-privacy.js");
  run("chat/chat-reports.js");
  run("chat/chat-warnings.js");
  run("chat/chat-protected.js");
  run("chat/chat-onboarding.js");
  run("chat/chat-p2p.js");
  for (const name of ["hosIcon", "holdToConfirm", "holdConfirm", "updateStats"]) {
    if (typeof ctx[name] !== "function") ctx[name] = () => "";
  }
  ctx.holdConfirm = async () => true;
  ctx.appendMessage = (el) => { state.appended.push(el); };
  ctx.notifyNewMessage = (...a) => { notified.push(a); };
  ctx.playNotificationChime = () => {};
  ctx.sendSWNotification = () => {};
  ctx.addNotice = () => {};
  const key = await globalThis.crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  ctx.getDmStoreKey = async () => key;
  ctx.pqSignMessage = async (_secret, bytes) => new Uint8Array(bytes);
  ctx.pqVerifyMessage = async (_pk, bytes, sig) => Buffer.from(bytes).equals(Buffer.from(sig));
  ctx.pqDmSeal = async (pub, plaintext) => ({ ek_ct_b64: b64(pub), nonce_b64: "AAAA", ct_b64: b64(plaintext) });
  ctx.pqDmOpen = async (_secret, ek, _nonce, ct) => (unb64(ek) === MY_KYBER ? unb64(ct) : null);
  ctx.pqBlake3 = async (bytes) => new Uint8Array(nodeCrypto.createHash("sha256").update(Buffer.from(bytes)).digest());
  const sock = fakeSocket();
  // The identity in use (opts.me / opts.seed: another identity on the same device).
  const me = opts.me || ME;
  vm.runInContext(`(s, me, seed) => {
    ws = s; myKey = me; myName = 'Me_1'; activeChannel = 'general'; identityConfirmed = true;
    myDilithiumPublicHex = me; myDilithiumSecret = new Uint8Array(4);
    myKyberPublicBase64 = '${MY_KYBER}'; myKyberSecret = new Uint8Array(4);
    myIdentity = { publicKeyHex: me, seed32: seed, canSign: true };
    dmFetchSent = true;
  }`, ctx)(sock, me, new Uint8Array(opts.seed || SEED));
  const handle = async (msg) => { await vm.runInContext("handleMessage", ctx)(msg); await settle(); };
  await handle({ type: "identify_challenge", nonce: "ab", server_did: SERVER });
  const store = vm.runInContext("hosDmStore", ctx);
  assert.ok(await store.init(me, "localhost"), "the store loads");
  store.setPassServer(SERVER);
  const users = [[me, "Me_1"], [ANN, "Ann"], [BEN, "Ben"], [CY, "Cy"], [DAN, "Dan"]].map(([k, name]) => ({ public_key: k, name, role: "", kyber_public: kyberOf(k), online: true }));
  await handle({ type: "full_user_list", users });
  // The relay's word on "Who can reach me": the safe defaults.
  await handle({ type: "reach_settings", settings: { message: "friends", call: "chosen", trade: "friends" } });
  vm.runInContext("updateChannelList", ctx)(clone(opts.channels || CHANNELS));
  await settle();
  sock.sent.length = 0;
  state.appended.length = 0;
  const fn = (name) => vm.runInContext(name, ctx);
  const el = (id) => ctx.document.getElementById(id);
  const set = (code, ...args) => vm.runInContext(code, ctx)(...args);
  const chat = {
    ctx, sock, store, state, storage, appended: state.appended, fetches, posted, exportBodies, notified, handle, fn, el, set,
    pinOpen: () => vm.runInContext("protectedPin", ctx) !== null,
    pinMode: () => { const p = vm.runInContext("protectedPin", ctx); return p ? p.mode : null; },
    setupStep: () => { const s = vm.runInContext("protectedSetup", ctx); return s ? s.step : null; },
    saved: () => P.protectedStateParse(storage.getItem(P.PROTECTED_STORAGE_KEY)),
  };
  return chat;
}

// ── Helpers ──────────────────────────────────────────────────────────────

/** A friend: a mutual follow holding a pass from me (serial `serial`). */
function befriend(chat, key, serial) {
  chat.store.setFollowing(key, true);
  chat.store.setFollower(key, true);
  chat.store.recordPassSent(key, serial, MAY);
  chat.set("(k) => { myFollowing.add(k); myFollowers.add(k); }", key);
}

/** Turn the setup on through its four steps, with `pin`. */
async function turnOn(chat, pin = PIN) {
  assert.equal(chat.fn("openProtectedSetup")(), true, "the setup's steps open");
  assert.equal(chat.fn("protectedSetupContinue")(), true, "step 1 (read) to step 2");
  assert.equal(await chat.fn("protectedSetupChoosePin")(pin, pin), true, "step 2: the PIN twice");
  assert.equal(chat.setupStep(), "review", "step 3: the review");
  assert.equal(chat.fn("protectedSetupApply")(), true, "step 4: apply");
  await settle();
  assert.ok(chat.saved(), "the setup is on");
  chat.sock.sent.length = 0;
  chat.posted.length = 0;
}

/** Type `pin` into the prompt and press its Continue, through the prompt's own wiring. */
async function answerPin(chat, pin) {
  const card = chat.el("protected-pin-card");
  card.querySelector("[data-protected-pin]").value = pin;
  await card.querySelector("[data-protected-pin-submit]").onclick();
  await settle();
}

/** Press the prompt's Cancel. */
async function cancelPin(chat) {
  chat.el("protected-pin-card").querySelector("[data-protected-pin-cancel]").onclick();
  await settle();
}

/** What a sealed dm_put carries (the stand-in seal is base64 of the plaintext). */
function opened(put) {
  const env = JSON.parse(put.content);
  return { sealedTo: unb64(env.ek_ct_b64), inner: JSON.parse(unb64(env.ct_b64)) };
}
const putsTo = (chat, key) => chat.sock.sent.filter((m) => m.type === "dm_put" && opened(m).sealedTo === kyberOf(key));

function envelope(from, to, text, ts) {
  const sig = b64(`hum/dm/v2\n${from}\n${to}\n${ts}\n${text}`);
  const inner = JSON.stringify({ v: 2, from, to, ts, text, sig });
  return JSON.stringify({ v: 2, ek_ct_b64: b64(MY_KYBER), nonce_b64: "AAAA", ct_b64: b64(inner) });
}

function passFrom(issuer, serial = "0123456789abcdef0123456789abcdef", may = MAY) {
  return fp.friendPassJson(serial, may, b64(fp.friendPassPreimage(SERVER, issuer, ME, serial, may)));
}

// The phrase, the BIP39 way: 256 bits of seed and 8 of SHA-256 checksum, 24 words of 11 bits.
function phraseOf(seed, list) {
  const cs = nodeCrypto.createHash("sha256").update(seed).digest()[0];
  const bits = [];
  for (const b of seed) for (let i = 7; i >= 0; i--) bits.push((b >> i) & 1);
  for (let i = 7; i >= 0; i--) bits.push((cs >> i) & 1);
  const out = [];
  for (let i = 0; i < 24; i++) {
    let x = 0;
    for (let j = 0; j < 11; j++) x = (x << 1) | bits[i * 11 + j];
    out.push(list[x]);
  }
  return out;
}

function partsOf(e) { return e && e.parts ? [...e.parts.values()] : []; }
function kidsOf(e) { return e && Array.isArray(e.children) ? e.children : []; }
function textOf(e) {
  if (!e) return "";
  const own = [e.textContent, typeof e.innerHTML === "string" ? e.innerHTML : ""].filter((s) => typeof s === "string").join(" ");
  return [own, ...kidsOf(e).map(textOf), ...partsOf(e).map(textOf)].join(" ");
}

function safetyHtml(chat) {
  chat.fn("openSafetyPanel")();
  return chat.el("safety-card").innerHTML;
}

// Every value anywhere inside `v` (objects, arrays), as strings.
function deepValues(v, out = []) {
  if (v && typeof v === "object") for (const x of Object.values(v)) deepValues(x, out);
  else out.push(String(v));
  return out;
}

// ── 1. The preset ────────────────────────────────────────────────────────

test("the preset is read from the real data file, and every word on screen comes from it", async () => {
  assert.ok(PRESET, "data/gui/safety_presets.json holds a valid `protected` preset");
  assert.deepEqual(PRESET.reach, { message: "friends", call: "chosen", trade: "friends" }, "10h's reach values");
  assert.equal(PRESET.warnings_on_friends, true);
  assert.equal(PRESET.pictures_from_non_friends, "never");
  assert.equal(PRESET.public_rooms, "read_only_only");
  assert.deepEqual([PRESET.pin_digits_min, PRESET.pin_digits_max, PRESET.pin_wrong_tries_before_wait, PRESET.pin_wait_seconds], [4, 12, 3, 60]);
  assert.equal(PRESET.sentences.length, 7, "the reading step's seven sentences");
  assert.deepEqual(P.protectedReachFrame(PRESET), reach.reachSetFrame({ message: "friends", call: "chosen", trade: "friends" }), "the frame is reach.js's own reach_set");
  // A file missing a word, or with a word the relay does not know, is not used at all.
  const bad = clone(SHIPPED_PRESETS);
  delete bad.presets[0].status_line;
  assert.equal(P.protectedPresetFrom(bad), null, "a preset missing a line is refused");
  const bad2 = clone(SHIPPED_PRESETS);
  bad2.presets[0].reach.message = "everyone";
  assert.equal(P.protectedPresetFrom(bad2), null, "a reach word the relay does not know is refused");
  assert.equal(P.protectedPresetFrom(null), null);

  // The page reads the file.
  const chat = await loadChat();
  assert.ok(chat.fetches.some((f) => f.url === P.PROTECTED_PRESETS_URL), "the page asked for /data/gui/safety_presets.json");
  const html = safetyHtml(chat);
  assert.ok(html.includes(escHtml(PRESET.name)), "Settings > Safety has the Protected setup section");
  assert.ok(html.includes(escHtml(PRESET.button)), "with the button");
  assert.ok(html.indexOf(escHtml(PRESET.summary)) > html.indexOf(escHtml(PRESET.button)), "and under it the finding's sentence 1");

  // An altered file shows its own words: nothing on screen is a copy kept in the code.
  const altered = clone(SHIPPED_PRESETS);
  const a = altered.presets[0];
  a.button = "Altered button words";
  a.summary = "Altered summary words.";
  a.status_line = "Altered status line.";
  a.sentences = ["Altered sentence one.", "Altered sentence two."];
  const other = await loadChat({ presets: altered });
  const oh = safetyHtml(other);
  assert.ok(oh.includes("Altered button words") && oh.includes("Altered summary words."), "the section shows the file's words");
  assert.ok(!oh.includes(escHtml(PRESET.summary)), "and not the shipped ones");
  assert.equal(other.fn("openProtectedSetup")(), true);
  const readStep = other.el("protected-setup-card").innerHTML;
  assert.ok(readStep.includes("Altered sentence one.") && readStep.includes("Altered sentence two."), "the reading step shows the file's sentences");
  other.fn("closeProtectedSetup")();
  await turnOn(other);
  other.fn("renderDmList")();
  assert.ok(other.el("dm-list").innerHTML.includes("Altered status line."), "the always-visible line is the file's");

  // No sentence or line of the file is typed into the code.
  const code = ["shared/protected.js", "chat/chat-protected.js", "chat/chat-privacy.js", "chat/chat-dms.js", "chat/chat-ui.js", "chat/app.js"]
    .map((rel) => fs.readFileSync(path.join(WEB, rel), "utf8")).join("\n");
  for (const s of P.protectedPresetStrings(PRESET)) {
    assert.ok(!code.includes(s), `the preset's words are not copied into the code: ${JSON.stringify(s.slice(0, 40))}`);
  }
});

// ── 2. The PIN verifier ──────────────────────────────────────────────────

test("the PIN verifier: PBKDF2-SHA-256 at 600,000 rounds with a 16-byte salt; right opens, wrong does not; the PIN is never kept", async () => {
  const v = await P.protectedMakeVerifier(PIN);
  assert.equal(v.kdf, "PBKDF2-SHA-256");
  assert.equal(v.iterations, 600000, "600,000 rounds, the vaults' strength");
  const salt = Buffer.from(v.salt, "base64");
  assert.equal(salt.length, 16, "a 16-byte salt");
  // Recomputed with Node's own PBKDF2: the verifier is exactly what 10h names.
  const want = nodeCrypto.pbkdf2Sync(PIN, salt, 600000, 32, "sha256").toString("base64");
  assert.equal(v.hash, want, "PBKDF2-SHA-256 of the PIN with that salt");
  assert.equal(await P.protectedPinMatches(PIN, v), true, "the right PIN opens");
  assert.equal(await P.protectedPinMatches(OTHER_PIN, v), false, "a wrong PIN does not");
  assert.equal(await P.protectedPinMatches("", v), false, "nor nothing");
  const v2 = await P.protectedMakeVerifier(PIN);
  assert.notEqual(v2.salt, v.salt, "a new random salt each time");
  assert.notEqual(v2.hash, v.hash);
  assert.equal(await P.protectedPinMatches(PIN, Object.assign({}, v, { iterations: 1000 })), false, "a verifier weaker than 600,000 rounds is not one");
  for (const [pin, okay] of [["1234", true], ["123456789012", true], ["123", false], ["1234567890123", false], ["12a4", false], [" 1234", false], ["١٢٣٤", false]]) {
    assert.equal(P.protectedPinOk(pin, P.protectedRulesFrom(PRESET)), okay, `PIN ${JSON.stringify(pin)}`);
  }

  // What the page keeps: a verifier, and the PIN nowhere.
  const chat = await loadChat();
  await turnOn(chat, PIN);
  const raw = chat.storage.getItem(P.PROTECTED_STORAGE_KEY);
  assert.ok(!raw.includes(PIN), "the PIN is not in what storage holds");
  const kept = JSON.parse(raw);
  assert.ok(!deepValues(kept).includes(PIN), "in no field");
  assert.equal(kept.pin.iterations, 600000);
  assert.equal(Buffer.from(kept.pin.salt, "base64").length, 16);
  assert.equal(kept.pin.hash, nodeCrypto.pbkdf2Sync(PIN, Buffer.from(kept.pin.salt, "base64"), 600000, 32, "sha256").toString("base64"), "only its verifier");
  for (const [k, val] of chat.storage._map) assert.ok(!String(val).includes(PIN), `nor anywhere else in storage (${k})`);
  // The right PIN opens a locked action; a wrong one does not.
  assert.equal(chat.fn("chooseReachAudience")("trade", "nobody"), false);
  await answerPin(chat, OTHER_PIN);
  assert.equal(chat.sock.sent.length, 0, "a wrong PIN sends nothing");
  assert.ok(chat.el("protected-pin-card").innerHTML.includes(escHtml(P.PROTECTED_LABELS.pin_wrong)), "and says so");
  await answerPin(chat, PIN);
  assert.deepEqual(chat.sock.sent, [{ type: "reach_set", settings: { trade: "nobody" } }], "the right PIN opens");
});

// ── 3. The wait ──────────────────────────────────────────────────────────

test("three wrong PINs in a row wait 60 seconds, kept across a reload", async () => {
  // Pure: the count, the wait, and the count starting again after it.
  let s = P.protectedStateNew(PRESET, null, []);
  s = P.protectedAfterWrong(s, 1000);
  s = P.protectedAfterWrong(s, 2000);
  assert.deepEqual([s.wrong, P.protectedWaitLeftMs(s, 2000)], [2, 0], "two wrong: no wait yet");
  s = P.protectedAfterWrong(s, 3000);
  assert.equal(P.protectedWaitLeftMs(s, 3000), 60000, "the third: 60 seconds");
  assert.equal(P.protectedWaitLeftMs(s, 63000), 0, "and then none");
  assert.equal(s.wrong, 0, "the count starts again after the wait");

  const storage = fakeStorage();
  const chat = await loadChat({ localStorage: storage });
  await turnOn(chat, PIN);
  chat.fn("chooseReachAudience")("trade", "nobody");
  for (let i = 0; i < 3; i++) await answerPin(chat, OTHER_PIN);
  const prompt = () => chat.el("protected-pin-card").innerHTML;
  assert.ok(prompt().includes("60 seconds"), "after three wrong PINs the prompt says to wait 60 seconds");
  const runs = pbkdf2Runs;
  await answerPin(chat, PIN);
  assert.equal(chat.sock.sent.length, 0, "the right PIN is refused during the wait");
  assert.equal(pbkdf2Runs, runs, "without even being checked");
  assert.ok(chat.pinOpen(), "the prompt stays");
  await cancelPin(chat);

  // A reload does not skip the wait.
  const again = await loadChat({ localStorage: storage });
  again.fn("chooseReachAudience")("trade", "nobody");
  await answerPin(again, PIN);
  assert.equal(again.sock.sent.length, 0, "after a reload the wait still holds");
  // Sixty seconds later the right PIN opens.
  const later = Date.now() + 61000;
  again.set("(t) => { protectedNow = () => t; }", later);
  await answerPin(again, PIN);
  assert.deepEqual(again.sock.sent, [{ type: "reach_set", settings: { trade: "nobody" } }], "after the wait the right PIN opens");
  assert.equal(again.saved().wrong, 0, "and the count starts again");
});

// ── 4. Locked actions ────────────────────────────────────────────────────

const LOCKED = [
  {
    name: "a Who can reach me row",
    run: (c) => c.fn("chooseReachAudience")("message", "anyone"),
    done: (c) => c.sock.sent.some((m) => m.type === "reach_set" && m.settings.message === "anyone"),
  },
  {
    name: "a People I choose tick",
    before: (c) => befriend(c, ANN, S1),
    run: (c) => c.fn("chooseFriendTick")(ANN, "call", true),
    // The new pass goes out, waiting for the server's answer: the old one is
    // withdrawn only once the server took it (10l, reach-web.test.js and
    // friend-pass-web.test.js prove that part).
    done: (c) => putsTo(c, ANN).some((m) => opened(m).inner.text === CTL_FRIEND_CERT && typeof m.ref === "string"),
  },
  {
    name: "Follow",
    run: (c) => c.fn("setFollowLocal")(BEN, true),
    done: (c) => putsTo(c, BEN).some((m) => opened(m).inner.text === CTL_FOLLOW),
  },
  {
    name: "Follow back",
    before: (c) => { c.store.setFollower(CY, true); c.set("(k) => { myFollowers.add(k); }", CY); },
    run: (c) => c.fn("setFollowLocal")(CY, true),
    done: (c) => putsTo(c, CY).some((m) => opened(m).inner.text === CTL_FOLLOW) && putsTo(c, CY).some((m) => opened(m).inner.text === CTL_FRIEND_CERT),
  },
  {
    name: "accepting a contact request",
    after: (c) => { c.store.addContactRequest({ key: BEN, name: "Ben", pass: passFrom(BEN), ts: 5 }); },
    run: (c) => c.fn("acceptContactRequest")(BEN),
    done: (c) => !c.store.contactRequests[BEN] && putsTo(c, BEN).some((m) => opened(m).inner.text === CTL_FOLLOW),
  },
  {
    name: "sending a contact request",
    run: (c) => c.fn("sendContactRequest")(CY),
    done: (c) => c.sock.sent.some((m) => m.type === "dm_put" && m.contact_request === true),
  },
  {
    name: "a friend code",
    run: (c) => c.fn("sendFriendCodeRequest")(),
    done: (c) => c.sock.sent.some((m) => m.type === "friend_code_request"),
  },
  {
    name: "joining a group by ticket",
    run: (c) => c.fn("joinP2pGroupByTicket")("TICKET-OK"),
    done: (c) => c.posted.some((p) => p.object_type === "group_join_v1"),
  },
  {
    name: "starting a group",
    run: (c) => { c.fn("createP2pGroup")("Climbers", false); },
    done: (c) => c.posted.some((p) => p.object_type === "group_v1" && p.name === "Climbers"),
  },
  {
    name: "making or copying a group's invite ticket",
    run: (c) => { c.fn("createP2pInvite")("g1", "Hikers"); },
    done: (c) => c.posted.some((p) => p.object_type === "group_invite_v1" && p.group === "g1"),
  },
  {
    name: "joining a voice room",
    run: (c) => { c.fn("joinVoiceRoom")("dev"); },
    done: (c) => c.sock.sent.some((m) => m.type === "voice_room" && m.action === "join" && m.room_id === "dev"),
  },
  {
    name: "turning warnings off",
    run: (c) => c.fn("setMessageWarningsOn")(false),
    done: (c) => c.store.warningsOn === false,
  },
  {
    name: "turning the pictures rule down",
    run: (c) => c.fn("protectedSetPictures")("click"),
    done: (c) => c.saved().pictures === "click",
  },
  {
    name: "showing public rooms",
    run: (c) => c.fn("protectedSetPublicRooms")("all"),
    done: (c) => c.saved().public_rooms === "all",
  },
  {
    name: "turning the setup off",
    run: (c) => c.fn("protectedTurnOff")(),
    done: (c) => c.storage.getItem(P.PROTECTED_STORAGE_KEY) === null && c.sock.sent.length === 0,
  },
];

test("every locked action is refused without the PIN and allowed with it", async () => {
  // The rule itself: with the setup on, every locked action needs the PIN; off, none does.
  const on = P.protectedStateNew(PRESET, null, []);
  for (const a of P.PROTECTED_LOCKED_ACTIONS) {
    assert.equal(P.protectedActionLocked(a, on), true, `${a} needs the PIN while on`);
    assert.equal(P.protectedActionLocked(a, null), false, `${a} is free while off`);
  }
  assert.equal(P.protectedActionLocked("something_new", on), true, "an action the rules do not name is locked (fail closed)");

  for (const L of LOCKED) {
    const chat = await loadChat();
    if (L.before) L.before(chat);
    await turnOn(chat, PIN);
    if (L.after) L.after(chat);
    await settle();
    chat.sock.sent.length = 0;
    chat.posted.length = 0;
    const warningsBefore = chat.store.warningsOn;

    // Refused: the prompt opens and nothing happens.
    L.run(chat);
    await settle();
    assert.ok(chat.pinOpen(), `${L.name}: the PIN prompt opens`);
    assert.equal(L.done(chat), false, `${L.name}: refused without the PIN`);
    // Cancelled: still nothing.
    await cancelPin(chat);
    assert.ok(!chat.pinOpen(), `${L.name}: Cancel closes the prompt`);
    assert.equal(L.done(chat), false, `${L.name}: refused when the prompt is cancelled`);
    // A wrong PIN: still nothing.
    L.run(chat);
    await settle();
    await answerPin(chat, OTHER_PIN);
    assert.equal(L.done(chat), false, `${L.name}: refused with a wrong PIN`);
    assert.ok(chat.pinOpen(), `${L.name}: the prompt stays after a wrong PIN`);
    assert.equal(chat.store.warningsOn, warningsBefore);
    // The right PIN: it happens.
    await answerPin(chat, PIN);
    assert.equal(L.done(chat), true, `${L.name}: allowed with the PIN`);
    assert.ok(!chat.pinOpen(), `${L.name}: the prompt closes`);
  }

  // Changing the PIN: the current one first, then the new one twice.
  const chat = await loadChat();
  await turnOn(chat, PIN);
  const pending = chat.fn("protectedChangePin")();
  await settle();
  assert.equal(chat.pinMode(), "pin", "changing the PIN asks for the current one");
  await answerPin(chat, OTHER_PIN);
  assert.equal(chat.pinMode(), "pin", "a wrong current PIN changes nothing");
  await answerPin(chat, PIN);
  assert.equal(chat.pinMode(), "newpin", "the right one lets a new one be chosen");
  const card = chat.el("protected-pin-card");
  card.querySelector("[data-protected-pin-new]").value = OTHER_PIN;
  card.querySelector("[data-protected-pin-new-again]").value = OTHER_PIN;
  await card.querySelector("[data-protected-pin-new-submit]").onclick();
  await settle();
  assert.equal(await pending, true);
  assert.ok(!chat.pinOpen());
  const v = chat.saved().pin;
  assert.equal(await P.protectedPinMatches(OTHER_PIN, v), true, "the new PIN is the one kept");
  assert.equal(await P.protectedPinMatches(PIN, v), false, "and the old one no longer opens");
});

// ── 5. Never locked ──────────────────────────────────────────────────────

test("Block, Report, Unfollow, leaving, muting and every way down never need the PIN", async () => {
  const on = P.protectedStateNew(PRESET, null, []);
  for (const a of P.PROTECTED_NEVER_LOCKED) assert.equal(P.protectedActionLocked(a, on), false, `${a} never needs the PIN`);
  for (const a of ["block", "report", "unfollow", "leave_group", "leave_voice_room", "mute"]) {
    assert.ok(P.PROTECTED_NEVER_LOCKED.includes(a), `${a} is on the never-locked list (10h)`);
  }

  const chat = await loadChat();
  befriend(chat, ANN, S1);
  befriend(chat, BEN, S2);
  await turnOn(chat, PIN);
  let asked = 0;
  const realAsk = chat.fn("protectedAskPin");
  chat.ctx.protectedAskPin = (a) => { asked++; return realAsk(a); };
  const noPrompt = (what) => { assert.equal(asked, 0, `${what}: no PIN asked`); assert.ok(!chat.pinOpen(), `${what}: no prompt`); };

  // Block.
  assert.equal(await chat.fn("blockKey")(ANN), true, "Block runs");
  await settle();
  assert.ok(chat.store.isBlocked(ANN));
  assert.ok(chat.sock.sent.some((m) => m.type === "cert_revoke" && m.serial === S1), "and withdraws the pass");
  noPrompt("Block");
  assert.ok(!chat.saved().approved.includes(ANN), "a blocked person is no longer a friend the PIN holder let be one");

  // Report.
  await chat.fn("openReportDialog")({ target: CY, context: "profile" });
  await settle();
  assert.ok(chat.fn("reportDialogChoose")(SHIPPED_REASONS.reasons ? SHIPPED_REASONS.reasons[0].id : "spam"), "a reason from the file");
  chat.sock.sent.length = 0;
  assert.equal(await chat.fn("submitReportDialog")(), true, "Report runs");
  assert.ok(chat.sock.sent.some((m) => m.type === "report_v2"), "the report is sent");
  noPrompt("Report");

  // Unfollow.
  chat.sock.sent.length = 0;
  await chat.fn("setFollowLocal")(BEN, false);
  await settle();
  assert.ok(putsTo(chat, BEN).some((m) => opened(m).inner.text === CTL_UNFOLLOW), "Unfollow tells them");
  assert.ok(chat.sock.sent.some((m) => m.type === "cert_revoke" && m.serial === S2), "and withdraws the pass");
  noPrompt("Unfollow");

  // Leaving a group.
  chat.posted.length = 0;
  await chat.fn("leaveP2pGroup")(GROUP);
  await settle();
  assert.ok(chat.posted.some((p) => p.object_type === "group_member_v1" && p.action === "remove"), "leaving a group runs");
  noPrompt("leaving a group");

  // Leaving a voice room.
  chat.set("() => { window._currentRoomId = 'announcements'; }");
  chat.sock.sent.length = 0;
  chat.fn("leaveVoiceRoom")();
  assert.ok(chat.sock.sent.some((m) => m.type === "voice_room" && m.action === "leave"), "leaving a voice room runs");
  noPrompt("leaving a voice room");

  // Muting.
  chat.set("() => { localStream = { getAudioTracks: () => [{ enabled: true }] }; isMuted = false; }");
  chat.fn("toggleMute")();
  assert.equal(chat.fn("isMuted"), true, "muting runs");
  noPrompt("muting");

  // Ignoring a request, and every way back up.
  chat.store.addContactRequest({ key: DAN, name: "Dan", pass: null, ts: 9 });
  chat.fn("ignoreContactRequest")(DAN);
  assert.ok(!chat.store.contactRequests[DAN], "Ignore runs");
  chat.store.setWarningsOn(false);
  assert.equal(chat.fn("setMessageWarningsOn")(true), true, "warnings back on");
  chat.fn("protectedSave")(Object.assign(chat.saved(), { pictures: "click", public_rooms: "all" }));
  assert.equal(chat.fn("protectedSetPictures")("never"), true, "the pictures rule back up");
  assert.equal(chat.fn("protectedSetPublicRooms")("read_only_only"), true, "public rooms hidden again");
  noPrompt("ignoring a request and turning things back up");
});

// ── 6. The review step ───────────────────────────────────────────────────

test("the review step lists who can already reach this device, with Remove for a friend, a group and a room", async () => {
  const chat = await loadChat();
  befriend(chat, ANN, S1);
  chat.set("() => { window._p2pGroups = [{ group_id: 'g1', name: 'Hikers', members: [] }]; window._currentRoomId = 'dev'; window._voiceChannels = [{ id: 'dev', name: 'dev', participants: [] }]; }");
  assert.equal(chat.fn("openProtectedSetup")(), true);
  const card = () => chat.el("protected-setup-card").innerHTML;
  // 1. Read.
  let pos = -1;
  for (const s of PRESET.sentences) {
    const at = card().indexOf(escHtml(s));
    assert.ok(at > pos, `the reading step shows the preset's sentences in order: ${JSON.stringify(s.slice(0, 40))}`);
    pos = at;
  }
  chat.fn("protectedSetupContinue")();
  // 2. The PIN: twice, 4 to 12 digits, with the recovery-phrase sentence said first.
  assert.ok(card().includes(escHtml(PRESET.forgot_pin_explain)), "the PIN step says who can change the PIN, before it is chosen");
  assert.equal(await chat.fn("protectedSetupChoosePin")(PIN, OTHER_PIN), false, "two different PINs are refused");
  assert.equal(await chat.fn("protectedSetupChoosePin")("123", "123"), false, "three digits are refused");
  assert.ok(card().includes(escHtml(P.protectedLabel(PRESET, "pin_rule", { min: 4, max: 12 }))), "with the rule");
  assert.equal(chat.storage.getItem(P.PROTECTED_STORAGE_KEY), null, "nothing is kept before the last step");
  assert.equal(await chat.fn("protectedSetupChoosePin")(PIN, PIN), true);
  // 3. The review.
  assert.equal(chat.setupStep(), "review");
  let html = card();
  assert.ok(html.includes(escHtml(PRESET.review_intro)), "the review's words");
  for (const [kind, id, name] of [["friend", ANN, "Ann"], ["group", "g1", "Hikers"], ["room", "dev", "dev"]]) {
    assert.ok(html.includes(`data-protected-remove="${kind}" data-protected-remove-id="${id}"`), `${name} is listed with Remove`);
    assert.ok(html.includes(name));
  }
  assert.ok(html.includes(escHtml(PRESET.review_keep)), "and Keep the rest");
  const m = chat.fn("protectedReviewModel")();
  assert.deepEqual([m.friends.map((f) => f.key), m.groups.map((g) => g.id), m.rooms.map((r) => r.id)], [[ANN], ["g1"], ["dev"]]);

  chat.sock.sent.length = 0;
  assert.equal(await chat.fn("protectedReviewRemove")("friend", ANN), true);
  await settle();
  assert.ok(putsTo(chat, ANN).some((p) => opened(p).inner.text === CTL_UNFOLLOW), "Remove for a friend unfollows them");
  assert.ok(chat.sock.sent.some((f) => f.type === "cert_revoke" && f.serial === S1), "which withdraws their pass");
  assert.equal(chat.store.certSentTo(ANN), false);
  assert.equal(await chat.fn("protectedReviewRemove")("group", "g1"), true);
  await settle();
  assert.ok(chat.posted.some((p) => p.object_type === "group_member_v1" && p.action === "remove" && p.group === "g1"), "Remove for a group leaves it");
  assert.equal(await chat.fn("protectedReviewRemove")("room", "dev"), true);
  assert.ok(chat.sock.sent.some((f) => f.type === "voice_room" && f.action === "leave"), "Remove for a voice room leaves it");
  const after = chat.fn("protectedReviewModel")();
  assert.deepEqual([after.friends.length, after.groups.length, after.rooms.length], [0, 0, 0], "and each is gone from the lists");
  assert.equal(card().split(escHtml(P.PROTECTED_LABELS.none)).length - 1, 3, "three empty lists say so");
  assert.equal(chat.storage.getItem(P.PROTECTED_STORAGE_KEY), null, "still nothing kept");
  // 4. Apply.
  assert.equal(chat.fn("protectedSetupApply")(), true);
  assert.deepEqual(chat.saved().approved, [], "no friend was kept, so none is let through without the PIN");

  // Cancelling at any step leaves it off.
  const other = await loadChat();
  other.fn("openProtectedSetup")();
  other.fn("protectedSetupContinue")();
  await other.fn("protectedSetupChoosePin")(PIN, PIN);
  other.el("protected-setup-card").querySelector("[data-protected-setup-cancel]").onclick();
  assert.equal(other.setupStep(), null);
  assert.equal(other.storage.getItem(P.PROTECTED_STORAGE_KEY), null, "cancelled in the review: still off");
});

// ── 7. One frame ─────────────────────────────────────────────────────────

test("turning it on sends exactly one reach_set with the preset's values, and nothing else", async () => {
  const chat = await loadChat();
  befriend(chat, ANN, S1);
  chat.sock.sent.length = 0;
  chat.posted.length = 0;
  const fetchedBefore = chat.fetches.length;
  assert.equal(chat.fn("openProtectedSetup")(), true);
  chat.fn("protectedSetupContinue")();
  await chat.fn("protectedSetupChoosePin")(PIN, PIN);
  // "Keep the rest", through the step's own button.
  chat.el("protected-setup-card").querySelector("[data-protected-setup-apply]").onclick();
  await settle();
  assert.ok(chat.saved(), "it is on");
  assert.deepEqual(chat.sock.sent, [{ type: "reach_set", settings: { message: "friends", call: "chosen", trade: "friends" } }],
    "exactly one frame: an ordinary reach_set with the preset's values");
  assert.deepEqual(chat.sock.sent[0], P.protectedReachFrame(PRESET));
  const wire = JSON.stringify(chat.sock.sent);
  assert.ok(!/protect/i.test(wire), "no flag, no protected field");
  const kept = chat.saved();
  assert.ok(!wire.includes(kept.pin.salt) && !wire.includes(kept.pin.hash), "nothing of the PIN's verifier");
  assert.equal(chat.posted.length, 0, "nothing posted");
  assert.ok(chat.fetches.slice(fetchedBefore).every((f) => f.method === "GET" && !/protect/i.test(f.url + (f.body || ""))), "and no request carries anything about it");
  assert.deepEqual(kept.approved, [ANN], "the friend kept in the review is let through");
  assert.equal(chat.store.warningsOn, true, "warnings are on");

  // Not connected: it does not turn on, and says so.
  const off = await loadChat();
  off.fn("openProtectedSetup")();
  off.fn("protectedSetupContinue")();
  await off.fn("protectedSetupChoosePin")(PIN, PIN);
  off.sock.readyState = 3;
  assert.equal(off.fn("protectedSetupApply")(), false);
  assert.equal(off.storage.getItem(P.PROTECTED_STORAGE_KEY), null, "not connected: not turned on");
  assert.ok(off.el("protected-setup-card").innerHTML.includes(escHtml(P.PROTECTED_LABELS.not_connected)));
  assert.equal(off.sock.sent.length, 0);
});

// ── 8. Public rooms ──────────────────────────────────────────────────────

test("public rooms: only read-only rooms are listed, the rest hidden with one line; showing them needs the PIN", async () => {
  const on = P.protectedStateNew(PRESET, null, []);
  assert.deepEqual(P.protectedChannelsShown(CHANNELS, null), { shown: CHANNELS, hidden: 0 }, "off: every room");
  assert.deepEqual(P.protectedChannelsShown(CHANNELS, on).shown.map((c) => c.id), ["announcements"], "on: read-only rooms only");
  assert.equal(P.protectedChannelsShown(CHANNELS, on).hidden, 2);
  assert.equal(P.protectedChannelsShown(CHANNELS, Object.assign({}, on, { public_rooms: "all" })).hidden, 0, "shown with the PIN: every room");

  const chat = await loadChat({ federated: [{ name: "Other", url: "https://other.example", trust_tier: 1, status: "online", server_id: "o1" }] });
  const list = () => { chat.fn("renderServerList")(); return chat.el("server-list").innerHTML; };
  const line = escHtml(PRESET.public_rooms_hidden_line);
  let html = list();
  for (const id of ["general", "announcements", "dev"]) assert.ok(html.includes(`data-channel-id="${id}"`), `off: #${id} is listed`);
  assert.ok(!html.includes(line));
  assert.ok(html.includes("Other"), "off: the federation list too");

  await turnOn(chat);
  html = list();
  assert.ok(html.includes('data-channel-id="announcements"'), "on: the read-only room is listed");
  assert.ok(!html.includes('data-channel-id="general"') && !html.includes('data-channel-id="dev"'), "the public rooms are not");
  assert.ok(html.includes(line), "and one line says they are hidden by the setup");
  assert.ok(!html.includes("Other"), "the federation list goes with them");
  chat.fn("renderChannelList")();
  assert.ok(!chat.el("channel-list").innerHTML.includes("general"), "the older list too");
  assert.equal(chat.fn("activeChannel"), "announcements", "the open public room was left for a listed one");
  chat.fn("switchChannel")("general");
  assert.equal(chat.fn("activeChannel"), "announcements", "a hidden room cannot be opened");

  // A post in a hidden room never reaches anything that draws or notifies it.
  chat.set("() => { activeChannel = 'general'; }");
  chat.appended.length = 0;
  chat.notified.length = 0;
  await chat.handle({ type: "chat", from: BEN, from_name: "Ben", content: "hello all", timestamp: 7001, channel: "general" });
  await chat.handle({ type: "chat", from: BEN, from_name: "Ben", content: "over here", timestamp: 7002, channel: "dev" });
  assert.equal(chat.appended.length, 0, "not drawn");
  assert.equal(chat.notified.length, 0, "not notified");
  assert.ok(!chat.ctx.unreadChannelCounts.general && !chat.ctx.unreadChannelCounts.dev, "not even an unread mark");
  // A hidden room's history is not loaded.
  const before = chat.fetches.length;
  await chat.fn("loadHistory")();
  assert.ok(!chat.fetches.slice(before).some((f) => f.url.startsWith("/api/messages")), "nor its history");
  chat.set("() => { activeChannel = 'announcements'; }");
  await chat.handle({ type: "chat", from: BEN, from_name: "Ben", content: "news", timestamp: 7003, channel: "announcements" });
  assert.equal(chat.appended.length, 1, "a read-only room's post is drawn");

  // The setup screen says what showing them means; showing them needs the PIN.
  const sh = safetyHtml(chat);
  assert.ok(sh.includes(escHtml(PRESET.public_rooms_explain)), "Settings > Safety says what showing them means");
  chat.fn("protectedSetPublicRooms")("all");
  await settle();
  assert.ok(chat.pinOpen(), "showing them asks for the PIN");
  await answerPin(chat, PIN);
  html = list();
  for (const id of ["general", "announcements", "dev"]) assert.ok(html.includes(`data-channel-id="${id}"`), `shown with the PIN: #${id}`);
  assert.ok(!html.includes(line));

  // With no read-only room at all, the scratch pad (nothing from anyone) is where it goes.
  const none = await loadChat({ channels: [CHANNELS[0], CHANNELS[2]] });
  await turnOn(none);
  assert.equal(none.fn("activeChannel"), none.fn("SCRATCH_PAD_ID"), "no listed room: the scratch pad");
});

// ── 9. Pictures ──────────────────────────────────────────────────────────

test("pictures and files from someone who is not a friend are not shown; a friend's and mine still click to load", async () => {
  const chat = await loadChat();
  befriend(chat, ANN, S1);
  const body = "look https://example.com/a.png and https://example.com/b.mp3 and https://example.com/c.mp4 and https://example.com/d.pdf";
  const line = escHtml(PRESET.picture_hidden_line);
  const post = (from, ts) => {
    chat.appended.length = 0;
    chat.fn("addChatMessage")(from === ME ? "Me_1" : "x", body, ts, from, true, false, null, null, false, null);
    return chat.appended[0].innerHTML;
  };
  let h = post(BEN, 100);
  assert.ok(h.includes("click to load") && h.includes("<audio") && h.includes("file-card"), "off: everyone's pictures click to load");
  await turnOn(chat);
  h = post(BEN, 101);
  assert.ok(!h.includes("img-placeholder") && !h.includes("<audio") && !h.includes("<video") && !h.includes("file-card"), "on: a non-friend's pictures and files are gone");
  assert.equal(h.split(line).length - 1, 4, "one line in place of each");
  assert.ok(h.includes("look"), "the words stay");
  h = post(ANN, 102);
  assert.ok(h.includes("click to load") && !h.includes(line), "a friend's still click to load");
  h = post(ME, 103);
  assert.ok(h.includes("click to load"), "and mine");

  // A file in a direct message from someone who is not a friend is not shown or opened.
  const marker = chat.fn("pqBuildFileMarker")({ url: "/uploads/x.enc", name: "x.png", mime: "image/png", size: 10, key: "k", nonce: "n" });
  chat.appended.length = 0;
  chat.fn("addDmMessage")("Ben", marker, 104, BEN, ME, true);
  h = chat.appended[0].innerHTML;
  assert.ok(h.includes(line) && !h.includes("enc-attach"), "a DM file from a non-friend: the line, no card");
  chat.appended.length = 0;
  chat.fn("addDmMessage")("Ann", marker, 105, ANN, ME, true);
  assert.ok(chat.appended[0].innerHTML.includes("enc-attach"), "from a friend: the card");
  // A link preview keeps its words and loses its picture.
  const pv = { url: "https://example.com/p", title: "T", image: "https://example.com/i.png" };
  assert.equal(chat.fn("protectedPreviewFor")(pv, BEN).image, undefined, "a non-friend's preview has no picture");
  assert.equal(chat.fn("protectedPreviewFor")(pv, BEN).title, "T");
  assert.equal(chat.fn("protectedPreviewFor")(pv, ANN).image, pv.image, "a friend's keeps it");

  // The rule turned down (with the PIN) shows them again.
  chat.fn("protectedSetPictures")("click");
  await settle();
  await answerPin(chat, PIN);
  h = post(BEN, 106);
  assert.ok(h.includes("click to load"), "turned down: click to load again");
});

// ── 10. Warnings for friends ─────────────────────────────────────────────

test("warnings show for friends too while it is on", async () => {
  const chat = await loadChat();
  befriend(chat, ANN, S1);
  const text = "Act now, last chance";
  const entry = SHIPPED_WARNINGS.warnings.find((w) => w.id === "urgency_secrecy");
  assert.deepEqual(entry.applies_to, ["strangers"], "an entry the file shows for strangers only");
  const receive = async (ts) => {
    chat.set("(k) => { activeDmPartner = k; activeDmPartnerName = 'Ann'; }", ANN);
    chat.appended.length = 0;
    await chat.handle({ type: "dm_new", id: ts, content: envelope(ANN, ME, text, ts) });
    const row = chat.appended.find((e) => e && e.dataset && e.dataset.from === ANN);
    assert.ok(row, "the friend's DM is drawn");
    return textOf(row);
  };
  assert.ok(!(await receive(201)).includes(entry.title), "off: a friend's message gets the friends' entries only");
  await turnOn(chat);
  assert.ok((await receive(202)).includes(entry.title), "on: a friend's message gets the strangers' entries too");
  assert.deepEqual(P.protectedWarningAudiences(true, P.protectedStateNew(PRESET, null, [])), ["friends", "strangers"]);
  assert.deepEqual(P.protectedWarningAudiences(true, null), ["friends"]);
  assert.deepEqual(P.protectedWarningAudiences(false, null), ["strangers"]);
});

// ── 11 and 12. Requests and the always-visible line ──────────────────────

test("contact requests show Accept (needs the PIN) and Ignore; the always-visible line is in Safety and above the DMs", async () => {
  const chat = await loadChat();
  chat.store.addContactRequest({ key: BEN, name: "Ben", pass: null, ts: 3 });
  chat.fn("renderDmList")();
  let dm = chat.el("dm-list").innerHTML;
  assert.ok(/data-req-accept="[0-9a-f]+"[^>]*>Accept</.test(dm), "off: Accept");
  assert.ok(!dm.includes(escHtml(PRESET.status_line)), "off: no line");
  let sh = safetyHtml(chat);
  assert.ok(!sh.includes(escHtml(PRESET.status_line)) && !sh.includes(escHtml(PRESET.routes_line)));

  await turnOn(chat);
  chat.fn("renderDmList")();
  dm = chat.el("dm-list").innerHTML;
  assert.ok(dm.includes(`>${escHtml(PRESET.accept_needs_pin)}<`), "on: Accept (needs the PIN)");
  assert.ok(/data-req-ignore="[0-9a-f]+"[^>]*>Ignore</.test(dm), "and Ignore");
  assert.ok(dm.indexOf(escHtml(PRESET.status_line)) >= 0, "the line is above the DM list");
  assert.ok(dm.indexOf(escHtml(PRESET.status_line)) < dm.indexOf("data-req-accept"), "at its top");
  sh = safetyHtml(chat);
  const top = sh.indexOf(escHtml(PRESET.status_line));
  assert.ok(top >= 0 && top < sh.indexOf("Who can reach me"), "and at the top of Settings > Safety");
  assert.ok(sh.includes(escHtml(PRESET.routes_line)), "the routes line is in Settings > Safety");
});

// ── 13. Words ────────────────────────────────────────────────────────────

test("every string the setup shows is held to the preset's avoid_words", async () => {
  const avoid = PRESET.avoid_words;
  assert.ok(avoid.includes("kid safe") && avoid.includes("verified") && avoid.includes("monitor"), "the finding's list, from the file");
  assert.equal(P.protectedAvoidHits(["This setup is Kid Safe"], avoid).length, 1, "the test can fail (letter case aside)");

  const shown = [...P.protectedPresetStrings(PRESET), ...Object.values(P.PROTECTED_LABELS)];
  // Everything drawn, in every state: the section off and on, each step, each prompt mode with its errors.
  const chat = await loadChat();
  befriend(chat, ANN, S1);
  chat.set("() => { window._p2pGroups = [{ group_id: 'g1', name: 'Hikers', members: [] }]; }");
  shown.push(safetyHtml(chat));
  chat.fn("openProtectedSetup")();
  shown.push(chat.el("protected-setup-card").innerHTML);
  chat.fn("protectedSetupContinue")();
  await chat.fn("protectedSetupChoosePin")("1", "2");
  shown.push(chat.el("protected-setup-card").innerHTML);
  await chat.fn("protectedSetupChoosePin")(PIN, PIN);
  shown.push(chat.el("protected-setup-card").innerHTML);
  chat.fn("protectedSetupApply")();
  await settle();
  shown.push(safetyHtml(chat));
  chat.store.addContactRequest({ key: BEN, name: "Ben", pass: null, ts: 3 });
  chat.fn("renderDmList")();
  shown.push(chat.el("dm-list").innerHTML);
  chat.fn("renderServerList")();
  shown.push(chat.el("server-list").innerHTML);
  chat.fn("chooseReachAudience")("trade", "nobody");
  await settle();
  shown.push(chat.el("protected-pin-card").innerHTML);
  for (let i = 0; i < 3; i++) { await answerPin(chat, OTHER_PIN); shown.push(chat.el("protected-pin-card").innerHTML); }
  chat.fn("protectedPinForgot")();
  shown.push(chat.el("protected-pin-card").innerHTML);
  await chat.fn("protectedForgotSubmit")("not the phrase");
  shown.push(chat.el("protected-pin-card").innerHTML);
  await chat.fn("protectedForgotSubmit")(phraseOf(SEED, chat.ctx.BIP39_ENGLISH).join(" "));
  await chat.fn("protectedNewPinSubmit")("1", "2");
  shown.push(chat.el("protected-pin-card").innerHTML);
  shown.push(chat.fn("protectedPictureHiddenHtml")());

  const hits = P.protectedAvoidHits(shown, avoid);
  assert.deepEqual(hits.map((x) => `${x.word} in ${JSON.stringify(x.text.slice(0, 60))}`), [], "no word from the finding's list, anywhere it shows");
  for (const s of shown) assert.ok(!s.includes("\u2014"), "and no em dash");
  for (const rel of ["shared/protected.js", "chat/chat-protected.js"]) {
    assert.ok(!fs.readFileSync(path.join(WEB, rel), "utf8").includes("\u2014"), `${rel} has no em dash`);
  }
});

// ── 14. Forgot the PIN ───────────────────────────────────────────────────

test("forgot the PIN: the right recovery phrase lets a new PIN be chosen, a wrong one does not", async () => {
  const chat = await loadChat();
  await turnOn(chat, PIN);
  const list = chat.ctx.BIP39_ENGLISH;
  const words = phraseOf(SEED, list);
  assert.equal(chat.fn("chooseReachAudience")("trade", "nobody"), false);
  await settle();
  chat.el("protected-pin-card").querySelector("[data-protected-pin-forgot]").onclick();
  assert.equal(chat.pinMode(), "forgot", "Forgot the PIN? asks for the recovery phrase");
  assert.ok(chat.el("protected-pin-card").innerHTML.includes(escHtml(PRESET.forgot_pin_explain)), "saying who can change it, first");
  const typePhrase = async (text) => {
    const card = chat.el("protected-pin-card");
    card.querySelector("[data-protected-phrase]").value = text;
    await card.querySelector("[data-protected-phrase-submit]").onclick();
    await settle();
  };
  await typePhrase(phraseOf(OTHER_SEED, list).join(" "));
  assert.equal(chat.pinMode(), "forgot", "another identity's phrase does not");
  assert.ok(chat.el("protected-pin-card").innerHTML.includes(escHtml(P.PROTECTED_LABELS.phrase_wrong)));
  await typePhrase([words[1], words[0], ...words.slice(2)].join(" "));
  assert.equal(chat.pinMode(), "forgot", "the right words out of order do not");
  await typePhrase(words.slice(0, 23).join(" "));
  assert.equal(chat.pinMode(), "forgot", "nor 23 of them");
  // The right phrase, as a backup screen shows it (numbered, capitals): a new PIN may be chosen.
  await typePhrase(words.map((w, i) => `${i + 1}. ${w[0].toUpperCase()}${w.slice(1)}`).join("\n"));
  assert.equal(chat.pinMode(), "newpin", "the right phrase lets a new PIN be chosen");
  assert.ok(chat.saved(), "the setup stays on");
  const card = chat.el("protected-pin-card");
  card.querySelector("[data-protected-pin-new]").value = OTHER_PIN;
  card.querySelector("[data-protected-pin-new-again]").value = OTHER_PIN;
  await card.querySelector("[data-protected-pin-new-submit]").onclick();
  await settle();
  assert.equal(chat.pinMode(), "pin", "then it asks for the PIN again");
  await answerPin(chat, PIN);
  assert.equal(chat.sock.sent.length, 0, "the old PIN no longer opens");
  await answerPin(chat, OTHER_PIN);
  assert.deepEqual(chat.sock.sent, [{ type: "reach_set", settings: { trade: "nobody" } }], "the new one does");
  assert.ok(chat.saved(), "and the setup is still on");
  assert.equal(P.protectedPhraseMatches(words.join(" "), null), false, "with no phrase to compare (a locked identity), nothing matches");
});

// ── 15. Per device ───────────────────────────────────────────────────────

test("the setup's state stays on this device: not in the self notes, the account export, the local store or the settings backup", async () => {
  const chat = await loadChat();
  befriend(chat, ANN, S1);
  await turnOn(chat);
  const kept = chat.saved();
  const secrets = [kept.pin.salt, kept.pin.hash, P.PROTECTED_STORAGE_KEY];

  // The self notes Block sends.
  await chat.fn("blockKey")(ANN);
  await settle();
  const notes = chat.sock.sent.filter((m) => m.type === "dm_put" && opened(m).sealedTo === MY_KYBER);
  assert.equal(notes.length, 1, "Block sends one note to my own devices");
  assert.equal(opened(notes[0]).inner.text, "[[hum:block:v1]]" + ANN, "which is the block note and nothing else");
  const wire = JSON.stringify(chat.sock.sent);
  for (const s of secrets) assert.ok(!wire.includes(s), "no frame carries the setup");
  assert.ok(!/protect/i.test(wire));

  // The account export (what is sent to the server to ask for it).
  await chat.fn("exportMyAccountData")();
  await settle();
  assert.equal(chat.exportBodies.length, 1);
  assert.deepEqual(Object.keys(JSON.parse(chat.exportBodies[0])).sort(), ["key", "sig", "timestamp"], "the export request carries only who asks");

  // The encrypted local store (the one whose block list my other devices learn).
  const metaRow = [...chat.ctx.indexedDB.stores.meta.values()][0];
  const meta = await chat.store._decrypt(metaRow.box);
  const metaText = JSON.stringify(meta);
  for (const s of secrets) assert.ok(!metaText.includes(s), "the local store does not hold it");

  // The settings page's backup: its key list does not name it, and an import never writes it.
  const settings = fs.readFileSync(path.join(WEB, "pages", "settings-app.js"), "utf8");
  const exportList = settings.slice(settings.indexOf("function exportData()"), settings.indexOf("const out = {};", settings.indexOf("function exportData()")));
  assert.ok(exportList.includes("hos_notes_v1"), "the backup's key list is there");
  assert.ok(!exportList.includes(P.PROTECTED_STORAGE_KEY), "and does not name the setup");
  const vault = settings.slice(settings.indexOf("async function vault_syncToCloud()"), settings.indexOf("window.vault_syncToCloud"));
  assert.deepEqual([...vault.matchAll(/localStorage\.getItem\(([^)]*)\)/g)].map((x) => x[1]), ["VAULT_LS_KEY"], "the vault sync reads the vault only");
  const start = settings.indexOf("const IMPORT_NEVER_KEYS");
  assert.ok(start > 0, "settings-app.js names the keys an import never writes");
  const code = settings.slice(start, settings.indexOf("// ── Keyboard Shortcuts", start));
  const ls = fakeStorage();
  const ictx = vm.createContext({
    localStorage: ls, JSON, holdConfirm: async () => true, alert() {}, loadPrefs() {}, applyPrefs() {},
    FileReader: class { readAsText(f) { this.onload({ target: { result: f.text } }); } },
  });
  vm.runInContext(code + "\n;this.importData = importData;", ictx);
  ictx.importData({ target: { files: [{ text: JSON.stringify({ [P.PROTECTED_STORAGE_KEY]: { on: false }, hos_notes_v1: [1] }) }] } });
  await settle();
  assert.equal(ls.getItem("hos_notes_v1"), "[1]", "an import writes what it holds");
  assert.equal(ls.getItem(P.PROTECTED_STORAGE_KEY), null, "except the setup, which a file can never turn off or replace");
});

// ── 16. Passes only to friends the PIN holder let be one ─────────────────

test("a follow made before the setup and followed back after it gives no pass without the PIN", async () => {
  const chat = await loadChat();
  befriend(chat, ANN, S1);
  // I followed Dan before the setup; Dan had not followed back.
  chat.store.setFollowing(DAN, true);
  chat.set("(k) => { myFollowing.add(k); }", DAN);
  await turnOn(chat);
  assert.deepEqual(chat.saved().approved, [ANN], "only the friend kept in the review");
  // Dan follows back.
  await chat.handle({ type: "dm_new", id: 301, content: envelope(DAN, ME, CTL_FOLLOW, 9301) });
  await settle();
  assert.equal(putsTo(chat, DAN).length, 0, "no pass goes to Dan: following back after the setup is not a friend the PIN holder let be one");
  assert.equal(chat.store.certSentTo(DAN), false);
  // Dan's contact request can still be accepted, with the PIN.
  chat.store.addContactRequest({ key: DAN, name: "Dan", pass: passFrom(DAN), ts: 9302 });
  chat.fn("acceptContactRequest")(DAN);
  await settle();
  assert.ok(chat.pinOpen(), "accepting asks for the PIN");
  await answerPin(chat, PIN);
  assert.ok(putsTo(chat, DAN).some((m) => opened(m).inner.text === CTL_FRIEND_CERT), "with it, Dan gets a pass");
  assert.ok(chat.saved().approved.includes(DAN));
  // Unfollow and Block mean the PIN again next time.
  await chat.fn("setFollowLocal")(ANN, false);
  await settle();
  assert.ok(!chat.saved().approved.includes(ANN), "after Unfollow");
  chat.fn("setFollowLocal")(ANN, true);
  await settle();
  assert.ok(chat.pinOpen(), "following again asks for the PIN");
  await cancelPin(chat);
});

// ── 17. Where it is loaded and where the gates sit ───────────────────────

test("index.html loads protected.js before chat-privacy.js, and every gate sits before the send it guards", () => {
  const src = (rel) => fs.readFileSync(path.join(WEB, rel), "utf8");
  const html = src("chat/index.html");
  const at = (needle) => { const i = html.indexOf(needle); assert.ok(i > 0, `index.html loads ${needle}`); return i; };
  assert.ok(at('src="/shared/reach.js?v=') < at('src="/shared/protected.js?v='), "protected.js after reach.js");
  assert.ok(at('src="/shared/protected.js?v=') < at('src="/chat/chat-privacy.js?v='), "and before chat-privacy.js");
  assert.ok(at('src="/shared/protected.js?v=') < at('src="/chat/app.js?v='), "and before every chat script that asks it");
  assert.ok(at('src="/chat/chat-privacy.js?v=') < at('src="/chat/chat-protected.js?v='), "chat-protected.js after the Safety page");
  assert.ok(at('src="/chat/chat-warnings.js?v=') < at('src="/chat/chat-protected.js?v='), "and after the recovery phrase it checks");
  assert.ok(at('src="/chat/chat-reports.js?v=') < at('src="/chat/chat-protected.js?v='), "and after every other script that wraps the frame handler");

  const gates = [
    ["chat/chat-privacy.js", "function chooseReachAudience(", "protectedTake('reach_row')", "ws.send("],
    ["chat/chat-privacy.js", "async function acceptContactRequest(", "protectedBefriendAllowed(", "setFollowLocal("],
    ["chat/chat-privacy.js", "async function sendContactRequest(", "protectedBefriendAllowed(", "ws.send("],
    ["chat/chat-privacy.js", "async function chooseFriendTick(", "protectedNeedsPin('reach_tick')", "setFriendTick("],
    ["chat/chat-social.js", "async function setFollowLocal(", "protectedBefriendAllowed(", "sendDmControl("],
    ["chat/chat-social.js", "function sendFriendCodeRequest(", "protectedTake('friend_code')", "ws.send("],
    ["chat/chat-social.js", "function redeemFriendCode(", "protectedTake('friend_code')", "ws.send("],
    ["chat/chat-groups-p2p.js", "async function createP2pGroup(", "protectedTake('start_group')", "postObject("],
    ["chat/chat-groups-p2p.js", "async function createP2pInvite(", "protectedTake('group_invite')", "postObject("],
    ["chat/chat-ui.js", "sendMessage = async function() {", "protectedTypedCommand(val)", "await _origSendMessage2();"],
    ["chat/chat-messages.js", "async function sendThreadReply(", "protectedTypedCommand(content)", "ws.send("],
    ["chat/chat-social.js", "async function setFriendTick(", "protectedTake('reach_tick')", "reissuePassTo("],
    ["chat/chat-social.js", "async function sendFriendCertTo(", "protectedPassAllowed(", "pqBuildFriendCert("],
    ["chat/chat-groups-p2p.js", "async function joinP2pGroupByTicket(", "protectedTake('join_group')", "postObject("],
    ["chat/chat-voice-rooms.js", "async function joinVoiceRoom(", "protectedTake('join_voice_room')", "ws.send("],
    ["chat/chat-warnings.js", "function setMessageWarningsOn(", "protectedTake('warnings_off')", "store.setWarningsOn("],
  ];
  for (const [rel, start, gate, send] of gates) {
    const s = src(rel);
    const a = s.indexOf(start);
    assert.ok(a >= 0, `${rel}: ${start} is there`);
    const g = s.indexOf(gate, a);
    const x = s.indexOf(send, a);
    assert.ok(g > a && x > g, `${rel}: ${start.replace(/\(.*$/, "")} asks ${gate} before ${send}`);
  }
  // The never-locked paths have no gate.
  const reports = src("chat/chat-reports.js");
  assert.ok(!/protected(Take|Unlock|AskThen|BefriendAllowed)/.test(reports), "Report never asks for the PIN");
  for (const [rel, start, end] of [
    ["chat/chat-privacy.js", "async function blockKey(", "async function unblockKey("],
    ["chat/chat-voice-rooms.js", "function leaveVoiceRoom(", "async function deleteVoiceChannel("],
    ["chat/chat-groups-p2p.js", "async function leaveP2pGroup(", "async function disbandP2pGroup("],
  ]) {
    const s = src(rel);
    const body = s.slice(s.indexOf(start), s.indexOf(end));
    assert.ok(body.length > 0 && !/protected(Take|Unlock|AskThen|BefriendAllowed)/.test(body), `${rel}: ${start.replace(/\(.*$/, "")} never asks for the PIN`);
  }
});

// ── 18. The recovery phrase needs the PIN while it is on ─────────────────

const OTHER_KEY = "e5".repeat(32);
const bodyKids = (chat, id) => kidsOf(chat.el("__body")).filter((e) => e.id === id);
/** How many times the 24 words of `seed` are on screen in the chat's recovery phrase box. */
function phraseShown(chat, seed) {
  const words = phraseOf(seed, chat.ctx.BIP39_ENGLISH);
  return bodyKids(chat, "seed-phrase-overlay").filter((e) => words.every((w) => String(e.innerHTML).includes(">" + w + "</span>"))).length;
}

/** Reopen the onboarding guide (Help, Getting Started) and record what its steps were built with. */
async function reopenGuide(chat) {
  const built = [];
  const buildStep = chat.fn("buildStep");
  const generateMnemonic = chat.fn("generateMnemonic");
  let generated = 0;
  chat.ctx.buildStep = (step, mnemonic, locked) => { built.push({ step, mnemonic, locked }); return buildStep(step, mnemonic, locked); };
  chat.ctx.generateMnemonic = async () => { generated++; return generateMnemonic(); };
  chat.fn("reopenOnboardingWizard")();
  await settle();
  chat.ctx.buildStep = buildStep;
  chat.ctx.generateMnemonic = generateMnemonic;
  assert.ok(built.length > 0, "the guide opened");
  return { first: built[0], generated, step1: buildStep(1, built[0].mnemonic, built[0].locked) };
}

test("showing the recovery phrase from the chat needs the PIN while it is on, and is free while off", async () => {
  assert.ok(P.PROTECTED_LOCKED_ACTIONS.includes("show_phrase"), "show_phrase is a locked action");
  assert.ok(!P.PROTECTED_NEVER_LOCKED.includes("show_phrase"));
  assert.equal(P.protectedActionLocked("show_phrase", null), false, "free while off");

  // Off: the Seed button (and /recovery) shows the 24 words at once, no prompt.
  const off = await loadChat();
  await off.fn("confirmRevealSeedPhrase")();
  await settle();
  assert.ok(!off.pinOpen(), "off: no PIN prompt");
  assert.equal(phraseShown(off, SEED), 1, "off: the phrase is shown");

  // On: refused without the PIN, when the prompt is cancelled and with a wrong PIN; shown with the right one.
  const chat = await loadChat();
  await turnOn(chat);
  await chat.fn("confirmRevealSeedPhrase")();
  await settle();
  assert.ok(chat.pinOpen(), "on: the PIN prompt opens");
  assert.equal(phraseShown(chat, SEED), 0, "on: refused without the PIN");
  await cancelPin(chat);
  assert.equal(phraseShown(chat, SEED), 0, "on: refused when the prompt is cancelled");
  await chat.fn("confirmRevealSeedPhrase")();
  await settle();
  await answerPin(chat, OTHER_PIN);
  assert.equal(phraseShown(chat, SEED), 0, "on: refused with a wrong PIN");
  await answerPin(chat, PIN);
  assert.ok(!chat.pinOpen(), "the prompt closes");
  assert.equal(phraseShown(chat, SEED), 1, "on: allowed with the PIN");
  // The permission is used up: the next time asks again.
  await chat.fn("confirmRevealSeedPhrase")();
  await settle();
  assert.ok(chat.pinOpen(), "the next time asks again");
  assert.equal(phraseShown(chat, SEED), 1, "and shows nothing more until it is answered");
  await cancelPin(chat);

  // Safety's own button, where the settings page and the onboarding guide send the PIN holder.
  const html = safetyHtml(chat);
  assert.ok(html.includes(escHtml(P.PROTECTED_LABELS.show_phrase)), "on: Safety has Show the recovery phrase");
  assert.ok(html.includes(escHtml(PRESET.forgot_pin_explain)), "with the preset's line on who can change the PIN");
  assert.ok(chat.el("safety-overlay").classList.contains("open"));
  chat.el("safety-card").querySelector("[data-protected-show-phrase]").onclick();
  await settle();
  assert.ok(!chat.el("safety-overlay").classList.contains("open"), "Safety closes, so the phrase's box is not under it");
  assert.ok(chat.pinOpen(), "Safety's button asks for the PIN");
  assert.equal(phraseShown(chat, SEED), 1);
  await answerPin(chat, PIN);
  assert.equal(phraseShown(chat, SEED), 2, "and shows the phrase after it");
  assert.ok(!safetyHtml(off).includes(escHtml(P.PROTECTED_LABELS.show_phrase)), "off: the section has no such button");

  // Every copy of the identity the phrase comes from asks too: the encrypted backup file (also
  // /backup), the plain identity file (/export) and the device-link code.
  for (const c of [off, chat]) {
    c.exports = 0;
    c.ctx.exportIdentityJSON = async () => { c.exports++; return null; };
    c.ctx.alert = () => {};
  }
  const copies = [
    { name: "the encrypted backup file", run: (c) => c.fn("openEncryptedBackupModal")(), count: (c) => bodyKids(c, "encrypted-backup-overlay").length },
    { name: "the identity file", run: (c) => c.fn("downloadIdentityBackup")("Me_1"), count: (c) => c.exports },
    { name: "the device-link code", run: (c) => c.fn("openLinkDeviceModal")(), count: (c) => c.exports },
  ];
  for (const k of copies) {
    const offBefore = k.count(off);
    k.run(off);
    await settle();
    assert.ok(!off.pinOpen(), `off: ${k.name} asks no PIN`);
    assert.equal(k.count(off), offBefore + 1, `off: ${k.name} is made`);

    const before = k.count(chat);
    k.run(chat);
    await settle();
    assert.ok(chat.pinOpen(), `on: ${k.name} asks for the PIN`);
    assert.equal(k.count(chat), before, `on: ${k.name} is refused without it`);
    await cancelPin(chat);
    assert.equal(k.count(chat), before, `on: ${k.name} is refused when the prompt is cancelled`);
    k.run(chat);
    await settle();
    await answerPin(chat, PIN);
    assert.equal(k.count(chat), before + 1, `on: ${k.name} is made with the PIN`);
  }

  // The onboarding guide, which Help reopens at any time: no words while it is on.
  const guideOff = await reopenGuide(off);
  assert.equal(guideOff.first.locked, false, "off: the guide's phrase step is not locked");
  assert.equal(guideOff.first.mnemonic, phraseOf(SEED, off.ctx.BIP39_ENGLISH).join(" "), "off: it holds the phrase");
  assert.ok(guideOff.step1.includes(">" + phraseOf(SEED, off.ctx.BIP39_ENGLISH)[5] + "</span>"), "off: and step 1 shows it");
  const guideOn = await reopenGuide(chat);
  assert.equal(guideOn.first.locked, true, "on: the guide's phrase step is locked");
  assert.equal(guideOn.first.mnemonic, null, "on: it holds no phrase");
  assert.equal(guideOn.generated, 0, "on: the phrase is not even made");
  for (const w of phraseOf(SEED, chat.ctx.BIP39_ENGLISH)) assert.ok(!guideOn.step1.includes(">" + w + "</span>"), "on: step 1 shows no word of it");
  assert.ok(guideOn.step1.includes(escHtml(P.PROTECTED_LABELS.phrase_needs_pin)), "on: step 1 says where the PIN opens it");
  assert.ok(!chat.pinOpen(), "the guide asks for no PIN on its own");
});

// ── 19. The settings page ────────────────────────────────────────────────

const PAGE_WORDS = Array.from({ length: 24 }, (_, i) => "pageword" + i);

/** The settings page's backup and recovery phrase section, run in a stub page over `storage`. */
function loadSettingsPage(storage, presets) {
  const settings = fs.readFileSync(path.join(WEB, "pages", "settings-app.js"), "utf8");
  const start = settings.indexOf("// ── Settings-specific backup/recovery-phrase modals ──");
  const end = settings.indexOf("function settingsShowPhraseUnavailable()");
  assert.ok(start > 0 && end > start, "settings-app.js has its backup and recovery phrase section");
  const calls = { mnemonic: 0, backup: 0, fetches: [] };
  const ctx = {
    console: { log() {}, warn() {}, error() {} },
    document: fakeDom({ appended: [] }),
    localStorage: storage,
    navigator: { clipboard: { writeText: async () => {} } },
    setTimeout: () => 0,
    settingsIdentityReady: Promise.resolve(),
    generateMnemonic: async () => { calls.mnemonic++; return PAGE_WORDS.join(" "); },
    exportEncryptedIdentityBackup: async () => { calls.backup++; },
    checkBackupStatus: () => {},
    fetch: async (url) => { calls.fetches.push(String(url)); return { ok: true, json: async () => clone(presets || SHIPPED_PRESETS) }; },
  };
  vm.createContext(ctx);
  vm.runInContext(settings.slice(start, end), ctx, { filename: "pages/settings-app.js" });
  const body = () => kidsOf(ctx.document.getElementById("__body"));
  return {
    ctx, calls, body,
    phraseShown: () => body().some((e) => String(e.innerHTML).includes(">" + PAGE_WORDS[7] + "</span>")),
    backupAsked: () => body().some((e) => String(e.innerHTML).includes("set-bkp-pass")),
    lines: () => body().map((e) => e.textContent).filter(Boolean),
  };
}

test("the settings page refuses the recovery phrase while the setup is on, saying where the PIN opens it", async () => {
  // Its names are protected.js's own.
  const page = loadSettingsPage(fakeStorage());
  assert.equal(page.ctx.SETTINGS_PROTECTED_KEY, P.PROTECTED_STORAGE_KEY, "the same storage key");
  assert.equal(page.ctx.SETTINGS_PHRASE_NEEDS_PIN, P.PROTECTED_LABELS.phrase_needs_pin, "the same line");
  assert.equal(page.ctx.SETTINGS_PRESETS_URL, P.PROTECTED_PRESETS_URL, "the same preset file");
  // It reads the setup as protected.js does: off only when nothing is kept or it says it is off.
  const raws = [null, "", '{"on":false}', "{}", "null", "[]", "garbage{", '"x"', JSON.stringify(P.protectedStateNew(PRESET, null, [], ME))];
  for (const raw of raws) {
    const s = fakeStorage();
    if (raw !== null) s.setItem(P.PROTECTED_STORAGE_KEY, raw);
    assert.equal(loadSettingsPage(s).ctx.settingsProtectedOn(), !!P.protectedStateParse(raw), `the same answer as protected.js for ${JSON.stringify(raw)}`);
  }

  // Off: View Recovery Phrase shows the words, and Backup asks for its passphrase.
  await page.ctx.settingsOpenSeed();
  assert.equal(page.calls.mnemonic, 1, "off: the phrase is made");
  assert.ok(page.phraseShown(), "off: and shown");
  await page.ctx.settingsOpenBackup();
  assert.ok(page.backupAsked(), "off: the backup asks for its passphrase");
  assert.deepEqual(page.lines(), [], "off: no line");

  // On, as the chat turns it on: refused, with one line saying where the PIN opens it.
  const storage = fakeStorage();
  storage.setItem(P.PROTECTED_STORAGE_KEY, JSON.stringify(P.protectedStateNew(PRESET, await P.protectedMakeVerifier(PIN), [], ME)));
  const on = loadSettingsPage(storage);
  await on.ctx.settingsOpenSeed();
  assert.equal(on.calls.mnemonic, 0, "on: the phrase is not even made");
  assert.ok(!on.phraseShown(), "on: nothing shows it");
  assert.deepEqual(on.lines(), [P.PROTECTED_LABELS.phrase_needs_pin], "on: one line says where the PIN opens it");
  await on.ctx.settingsOpenBackup();
  assert.ok(!on.backupAsked(), "on: no backup file either");
  assert.equal(on.calls.backup, 0);
  assert.equal(on.lines().length, 2, "on: the same line again");
  assert.deepEqual(P.protectedAvoidHits(on.lines(), PRESET.avoid_words), [], "the line holds no word from the finding's list");
  assert.ok(!on.lines().some((s) => s.includes("\u2014")), "and no em dash");

  // A setup this page cannot read counts as on (fail closed).
  const damaged = fakeStorage();
  damaged.setItem(P.PROTECTED_STORAGE_KEY, "garbage{");
  const dp = loadSettingsPage(damaged);
  await dp.ctx.settingsOpenSeed();
  assert.equal(dp.calls.mnemonic, 0, "damaged: refused too");

  // The words come from the preset file when it has them.
  const altered = clone(SHIPPED_PRESETS);
  altered.presets[0].labels.phrase_needs_pin = "Altered words for the PIN line.";
  const ap = loadSettingsPage(storage, altered);
  await ap.ctx.settingsOpenSeed();
  assert.ok(ap.calls.fetches.includes(P.PROTECTED_PRESETS_URL), "the page asked for the preset file");
  assert.deepEqual(ap.lines(), ["Altered words for the PIN line."], "the file's own label wins");
});

// ── 20. Forgot the PIN: only the setup's own identity ────────────────────

test("forgot the PIN takes only the phrase of the identity the setup was turned on under", async () => {
  // The rule.
  const st = P.protectedStateNew(PRESET, null, [], ME);
  assert.equal(st.identity, ME, "the state keeps the identity it was turned on under");
  assert.equal(P.protectedIdentityMatches(st, ME.toUpperCase()), true, "letter case aside");
  assert.equal(P.protectedIdentityMatches(st, OTHER_KEY), false, "another identity does not match");
  assert.equal(P.protectedIdentityMatches(null, ME), false, "nothing matches while it is off");
  for (const bad of [undefined, null, "", 42, "not a key!", { k: ME }]) {
    const raw = clone(st);
    if (bad === undefined) delete raw.identity; else raw.identity = bad;
    const parsed = P.protectedStateParse(JSON.stringify(raw));
    assert.ok(parsed, "a damaged identity leaves the setup on");
    assert.equal(P.protectedIdentityMatches(parsed, ME), true, `a ${JSON.stringify(bad)} identity falls back to the identity in use`);
    assert.equal(P.protectedIdentityMatches(parsed, ""), false, "but never to no identity");
  }

  const forgot = async (c, typed) => {
    if (!c.pinOpen()) { c.fn("chooseReachAudience")("trade", "nobody"); await settle(); }
    if (c.pinMode() === "pin") c.el("protected-pin-card").querySelector("[data-protected-pin-forgot]").onclick();
    assert.equal(c.pinMode(), "forgot");
    const card = c.el("protected-pin-card");
    card.querySelector("[data-protected-phrase]").value = typed;
    await card.querySelector("[data-protected-phrase-submit]").onclick();
    await settle();
    return c.pinMode();
  };

  // Turned on in the chat, under ME: its own phrase works.
  const chat = await loadChat();
  await turnOn(chat);
  assert.equal(chat.saved().identity, ME, "turning it on keeps the identity in use");
  const list = chat.ctx.BIP39_ENGLISH;
  const mine = phraseOf(SEED, list).join(" ");
  const theirs = phraseOf(OTHER_SEED, list).join(" ");
  assert.equal(await forgot(chat, mine), "newpin", "the setup's own identity: its phrase lets a new PIN be chosen");
  await cancelPin(chat);

  // A new identity made on this device: the setup is still on (this device's storage keeps it).
  const other = await loadChat({ localStorage: chat.storage, me: OTHER_KEY, seed: OTHER_SEED });
  assert.ok(other.saved(), "the setup is still on under the new identity");
  assert.equal((await other.fn("recoveryPhraseWords")()).join(" "), theirs, "the phrase typed below is the new identity's own, correct one");
  assert.equal(await forgot(other, theirs), "forgot", "another identity: even its own correct phrase is refused");
  assert.ok(other.el("protected-pin-card").innerHTML.includes(escHtml(PRESET.labels.phrase_wrong)), "with the preset's phrase_wrong line");
  assert.equal(await forgot(other, mine), "forgot", "and the setup identity's phrase, typed under another identity, too");
  assert.equal(await P.protectedPinMatches(PIN, other.saved().pin), true, "the PIN is unchanged");
  await cancelPin(other);

  // A missing or damaged stored identity (only damage leaves one) falls back to the identity in
  // use, so the right phrase still opens a damaged setup rather than it locking for good.
  for (const [what, bad] of [["missing", undefined], ["damaged", "not a key!"]]) {
    const raw = JSON.parse(chat.storage.getItem(P.PROTECTED_STORAGE_KEY));
    if (bad === undefined) delete raw.identity; else raw.identity = bad;
    const s = fakeStorage();
    s.setItem(P.PROTECTED_STORAGE_KEY, JSON.stringify(raw));
    const c = await loadChat({ localStorage: s });
    assert.equal(await forgot(c, mine), "newpin", `a ${what} identity: the phrase of the identity in use opens it`);
    await cancelPin(c);
  }

  // With no identity in use, the setup does not turn on (Forgot could never open it).
  const none = await loadChat();
  none.set("() => { myKey = ''; }");
  assert.equal(none.fn("openProtectedSetup")(), true);
  none.fn("protectedSetupContinue")();
  await none.fn("protectedSetupChoosePin")(PIN, PIN);
  assert.equal(none.fn("protectedSetupApply")(), false, "no identity: not applied");
  assert.equal(none.saved(), null, "and nothing kept");
});

// ── 21. The batch review's fixes (2026-10-10) ────────────────────────────
// Each seen failing against the code before the fix (web/ as at a3ca8f167, through HOS_WEB_DIR):
// the assertion it tripped is in the red list at the top of this file (21 to 26).

test("the PIN opens one action, the one it was asked for, and nothing after that action returns", async () => {
  const chat = await loadChat();
  befriend(chat, ANN, S1);
  befriend(chat, BEN, S2);
  await turnOn(chat, PIN);
  // A tick already as asked: the PIN is given, and the change, having nothing to do, returns
  // before it uses the PIN.
  const first = chat.fn("chooseFriendTick")(ANN, "message", true);
  await settle();
  assert.ok(chat.pinOpen(), "the tick asks for the PIN");
  await answerPin(chat, PIN);
  assert.equal(await first, true);
  chat.sock.sent.length = 0;
  // The next tick, for someone else, asks again.
  chat.fn("chooseFriendTick")(BEN, "call", true);
  await settle();
  assert.ok(chat.pinOpen(), "a second action asks for the PIN again, though the first returned without using it");
  assert.deepEqual(chat.sock.sent, [], "and nothing is re-issued without it");
  await cancelPin(chat);

  // The gate itself: a run that returns early, one that throws, and one of another kind.
  const askThen = chat.fn("protectedAskThen");
  const take = chat.fn("protectedTake");
  let p = askThen("join_group", () => "returned early");
  await settle();
  await answerPin(chat, PIN);
  assert.equal(await p, "returned early");
  assert.equal(take("join_group"), false, "a grant its run did not take is gone once the run returns");
  p = askThen("join_group", () => { throw new Error("boom"); });
  const rejected = assert.rejects(p); // handled at once: the run throws once the PIN is given
  await settle();
  await answerPin(chat, PIN);
  await rejected;
  assert.equal(take("join_group"), false, "and once it throws");
  p = askThen("join_group", async () => [take("befriend"), take("join_group"), take("join_group")]);
  await settle();
  await answerPin(chat, PIN);
  assert.deepEqual(await p, [false, true, false], "it opens its own action, once, and no other kind");
});

test("starting a group, an invite ticket and the typed friend-code commands need the PIN while it is on", async () => {
  // The rule, and the words that say so.
  const on = P.protectedStateNew(PRESET, null, []);
  for (const a of ["start_group", "group_invite", "friend_code"]) assert.equal(P.protectedActionLocked(a, on), true, `${a} needs the PIN`);
  assert.deepEqual(P.protectedTypedCommand("/friend-code"), { action: "friend_code", command: "friend-code", code: "" });
  assert.deepEqual(P.protectedTypedCommand("  /REDEEM   ab12  "), { action: "friend_code", command: "redeem", code: "ab12" });
  assert.equal(P.protectedTypedCommand("redeem ab12"), null);
  assert.ok(/starting a group/.test(PRESET.status_line) && /inviting someone to a group/.test(PRESET.routes_line), "the status and routes lines say so");

  const chat = await loadChat();
  await turnOn(chat, PIN);
  const input = chat.el("msg-input");
  // /friend-code, typed.
  input.value = "/friend-code";
  await chat.fn("sendMessage")();
  await settle();
  assert.ok(chat.pinOpen(), "/friend-code: the PIN prompt opens");
  assert.deepEqual(chat.sock.sent, [], "/friend-code: nothing reaches the server without the PIN");
  await answerPin(chat, PIN);
  assert.deepEqual(chat.sock.sent, [{ type: "friend_code_request" }], "with it, the code is asked for, once");
  // /redeem <code>, typed.
  chat.sock.sent.length = 0;
  input.value = "/REDEEM  ABCD1234";
  await chat.fn("sendMessage")();
  await settle();
  assert.ok(chat.pinOpen(), "/redeem: the PIN prompt opens");
  assert.deepEqual(chat.sock.sent, [], "/redeem: nothing reaches the server without the PIN");
  await answerPin(chat, PIN);
  assert.deepEqual(chat.sock.sent, [{ type: "friend_code_redeem", code: "ABCD1234" }], "with it, the code is redeemed");
  // The answer names whose code it was: following them asks for no second PIN.
  chat.sock.sent.length = 0;
  await chat.handle({ type: "friend_code_result", success: true, name: "Ben", message: "", owner_key: BEN });
  assert.ok(!chat.pinOpen(), "no second PIN for the friend the code named");
  assert.ok(putsTo(chat, BEN).some((m) => opened(m).inner.text === CTL_FOLLOW), "following them goes out");
  assert.ok(chat.saved().approved.includes(BEN), "and they are a friend the PIN holder let be one");
  // An answer nobody asked for makes no friend without the PIN.
  await chat.handle({ type: "friend_code_result", success: true, name: "Cy", message: "", owner_key: CY });
  assert.ok(chat.pinOpen(), "an answer nobody asked for asks for the PIN before following anyone");
  await cancelPin(chat);
  assert.deepEqual(putsTo(chat, CY), []);
  // The thread reply box would send them to the server as commands too.
  chat.sock.sent.length = 0;
  chat.set("(k) => { currentThread = { from: k, author: 'Ann', body: 'hi', timestamp: 5 }; }", ANN);
  chat.el("thread-input").value = "/friend-code";
  await chat.fn("sendThreadReply")();
  await settle();
  assert.ok(chat.pinOpen(), "typed in a thread: the PIN prompt opens");
  assert.deepEqual(chat.sock.sent, [], "and nothing is sent");
  await cancelPin(chat);
});

test("my own Unfollow, from another of my devices, takes them off the approved list here too", async () => {
  const chat = await loadChat();
  befriend(chat, ANN, S1);
  await turnOn(chat, PIN);
  assert.ok(chat.saved().approved.includes(ANN), "kept at the review: approved");
  await chat.handle({ type: "dm_new", id: 71, content: envelope(ME, ANN, CTL_UNFOLLOW, 1760000009000) });
  assert.ok(!chat.saved().approved.includes(ANN), "an Unfollow echoed from another device takes them off");
  assert.equal(chat.store.certSentTo(ANN), false, "and withdraws the pass");
  chat.fn("setFollowLocal")(ANN, true);
  await settle();
  assert.ok(chat.pinOpen(), "following them again needs the PIN");
  await cancelPin(chat);
});

test("the review lists mutual follows still owed a pass, and keeping them approves them", async () => {
  const chat = await loadChat();
  befriend(chat, ANN, S1);
  // Cy: a mutual follow whose pass has not gone out yet (the sweep would give it).
  chat.store.setFollowing(CY, true);
  chat.store.setFollower(CY, true);
  chat.set("(k) => { myFollowing.add(k); myFollowers.add(k); }", CY);
  assert.equal(chat.store.certSentTo(CY), false);
  chat.fn("openProtectedSetup")();
  chat.fn("protectedSetupContinue")();
  await chat.fn("protectedSetupChoosePin")(PIN, PIN);
  assert.deepEqual(chat.fn("protectedReviewModel")().friends.map((f) => f.key).sort(), [ANN, CY].sort(), "both friends are listed, the one owed a pass too");
  assert.ok(chat.el("protected-setup-card").innerHTML.includes(`data-protected-remove="friend" data-protected-remove-id="${CY}"`), "with Remove");
  assert.equal(chat.fn("protectedSetupApply")(), true);
  await settle();
  assert.deepEqual(chat.saved().approved.slice().sort(), [ANN, CY].sort(), "the friends kept are the ones approved");
  chat.sock.sent.length = 0;
  await chat.fn("sweepFriendPasses")();
  await settle();
  assert.ok(putsTo(chat, CY).some((m) => opened(m).inner.text === CTL_FRIEND_CERT), "so the pass sweep gives Cy the pass owed");
});

test("while it is on, warnings stay on whatever the identity's own switch says; only the PIN turns them off", async () => {
  const chat = await loadChat();
  await turnOn(chat, PIN);
  assert.equal(chat.fn("messageWarningsOn")(), true);
  // A restored identity whose warnings were off: its own switch, in its local store, says off.
  chat.store.setWarningsOn(false);
  assert.equal(chat.fn("messageWarningsOn")(), true, "the identity's own switch does not turn them off while the setup is on");
  assert.ok(/data-warnings-switch[^>]*checked/.test(safetyHtml(chat)), "and Safety shows them on");
  // The PIN holder can turn them off, and on again.
  chat.fn("setMessageWarningsOn")(false);
  await settle();
  assert.ok(chat.pinOpen(), "turning them off asks for the PIN");
  await answerPin(chat, PIN);
  assert.equal(chat.fn("messageWarningsOn")(), false, "with it, they are off");
  chat.store.setWarningsOn(true);
  assert.equal(chat.fn("messageWarningsOn")(), false, "and another identity's switch does not turn them back on either");
  assert.equal(chat.fn("setMessageWarningsOn")(true), true, "turning them on needs no PIN");
  assert.equal(chat.fn("messageWarningsOn")(), true);
  // With the setup off, the identity's own switch decides again.
  chat.fn("protectedTurnOff")();
  await settle();
  await answerPin(chat, PIN);
  assert.equal(chat.saved(), null, "off");
  chat.store.setWarningsOn(false);
  assert.equal(chat.fn("messageWarningsOn")(), false, "off: the identity's own switch");
});
