// The website's Web Push requests, signed over what the server keeps (2026-11-06, BUG-255).
//
// Run: node --test scripts/tests/push-subscribe-web.test.js   (in `just rig-tests`)
// HOS_WEB_DIR points it at another copy of web/ (how each test was seen red).
//
// THE FAULTS IT GUARDS: the chat's subscribe signed only `push_subscribe\n{timestamp}`, so a
// captured request could be sent again with another endpoint (one the server would then POST
// to), and Settings' unsubscribe sent `{ endpoint }` with no signature at all. The relay now
// refuses both shapes (src/relay/push_routes.rs), so the website must send the new ones or
// notifications silently stop. And the chat subscribed ONCE EVER (a `hos_push_subscribed` mark),
// while the server forgets a subscription on a 404 or 410 from the push service: a browser whose
// subscription the server forgot never subscribed again (found by a critic, 2026-11-06).
//
// What it proves, with the real chat-ui.js requestNotifications / subscribeToPush, crypto.js
// pqSignChatMessage, settings-app.js togglePushNotifications / unsubscribeFromPush and
// shared/pq-relay-auth.js run in node:vm over a stand-in browser (service worker, push manager,
// fetch, localStorage):
//  1. Subscribe sends { key, endpoint, p256dh, auth, timestamp, sig }, and the bytes handed to the
//     Dilithium signer are exactly scripts/tests/fixtures/push-signing-v1.json's subscribe
//     preimage, the words the relay's test holds `subscribe_purpose` to.
//  2. A refusal is told to the person in plain words, by key: each reason the relay can answer
//     (outbound.rs `Refusal::reason`, and push_routes.rs' keys and clock) has its words in
//     en.json, and an unknown one gets the general words.
//  3. Turning push off in Settings sends { key, endpoint, timestamp, sig }, the bytes signed being
//     the fixture's unsubscribe preimage; with no identity readable on the page nothing is sent;
//     and the chat, loaded afterwards, subscribes nothing while push is off.
//  4. Every load sends the browser's subscription again (once per load, not on each reconnect),
//     so one the server forgot is taken back on the next load.
//  5. A subscription the browser made under an older key of this server is dropped and made anew.
//
// Red first, 2026-11-06, against a copy of web/ holding the files as they were before BUG-255
// (git show HEAD:web/... into a scratch folder, HOS_WEB_DIR=<it>):
//  1: "the bytes signed are the fixture's subscribe preimage" (actual 'push_subscribe\n1800000000000').
//  2: "a refusal is told to the person" (nothing was said: the old code ignored a refused answer).
//  3: "the unsubscribe is signed" (the old body was { endpoint } alone).
//  4: "a forgotten subscription is re-sent on the next load" (0 sent: the mark from the first load
//     stopped it).
//  5: "a new subscription is made under the server's current key" (nothing sent: the old code gave
//     up when subscribe failed).

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const read = (p) => fs.readFileSync(path.join(WEB, p), 'utf8').replace(/\r\n/g, '\n');
const FIXTURE = JSON.parse(fs.readFileSync(path.join(ROOT, 'scripts', 'tests', 'fixtures', 'push-signing-v1.json'), 'utf8'));

function flatten(obj, prefix, out) {
  for (const [k, v] of Object.entries(obj)) {
    const key = prefix ? prefix + '.' + k : k;
    if (v && typeof v === 'object') flatten(v, key, out);
    else out[key] = v;
  }
  return out;
}
const EN = flatten(JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'i18n', 'en.json'), 'utf8')), '', {});

/** `start` in `src` through the first `endMark` after it. */
function slice(src, start, endMark) {
  const a = src.indexOf(start);
  assert.ok(a >= 0, 'no ' + start);
  const b = src.indexOf(endMark, a);
  assert.ok(b > a, 'no end for ' + start);
  return src.slice(a, b + endMark.length);
}

/** A browser's localStorage: one per browser, shared by every page load in it. */
function storage(initial) {
  const m = new Map(Object.entries(initial || {}));
  return { getItem: (k) => (m.has(k) ? m.get(k) : null), setItem: (k, v) => m.set(k, String(v)), removeItem: (k) => m.delete(k) };
}
const BACKUP = { humanity_key_backup: JSON.stringify({ privateKeyPkcs8: Buffer.alloc(48, 7).toString('base64') }) };

/** Lets every pending promise chain in the page run to its end. */
async function settle() {
  for (let i = 0; i < 30; i++) await new Promise((r) => setImmediate(r));
}

/** A stand-in browser page: a push subscription for the fixture's endpoint and keys, a fetch that
 *  records what is sent, and a Dilithium signer that records the bytes it is handed. `opts.store`
 *  is the browser's localStorage (a fresh one by default), `opts.answer` the server's answer to a
 *  subscribe, `opts.oldKey` a subscription held under another key (subscribe fails until it goes). */
function browser(opts = {}) {
  const sent = [];
  const signed = [];
  const said = [];
  const sub = {
    endpoint: FIXTURE.endpoint,
    toJSON: () => ({ endpoint: FIXTURE.endpoint, keys: { p256dh: FIXTURE.p256dh, auth: FIXTURE.auth } }),
    unsubscribed: false,
    unsubscribe() { this.unsubscribed = true; return Promise.resolve(true); },
  };
  let held = opts.oldKey ? { unsubscribe() { held = null; return Promise.resolve(true); } } : sub;
  const reg = {
    pushManager: {
      subscribe: () => (held && held !== sub ? Promise.reject(new Error('InvalidStateError: another key')) : Promise.resolve((held = sub))),
      getSubscription: () => Promise.resolve(held),
    },
  };
  const ctx = {
    console: { log() {}, warn() {} },
    atob, btoa, TextEncoder, TextDecoder,
    navigator: { serviceWorker: { ready: Promise.resolve(reg) } },
    PushManager: function () {},
    Notification: { permission: 'granted' },
    localStorage: opts.store || storage(BACKUP),
    document: { getElementById: () => null },
    i18n: { t: (k) => (k in EN ? EN[k] : k) },
    addSystemMessage: (text) => said.push(text),
    updatePushSubToggles: () => {},
    myIdentity: { canSign: true, publicKeyHex: 'd1l1th1um' },
    myDilithiumSecret: new Uint8Array(4),
    pqSignMessage: (secret, bytes) => { signed.push(new TextDecoder().decode(bytes)); return Promise.resolve(new Uint8Array([0xab, 0xcd])); },
    pqDeriveIdentity: () => Promise.resolve({ dilithiumPublicHex: 'd1l1th1um', dilithiumSecret: new Uint8Array(4) }),
    fetch: (url, o) => {
      if (url === '/api/vapid-public-key') return Promise.resolve({ json: () => Promise.resolve({ key: 'BAAA' }) });
      sent.push({ url, body: JSON.parse(o.body) });
      return Promise.resolve(opts.answer || { ok: true, json: () => Promise.resolve({ status: 'ok' }) });
    },
  };
  ctx.window = ctx;
  ctx.globalThis = ctx;
  vm.createContext(ctx);
  vm.runInContext('Date.now = function () { return ' + FIXTURE.timestamp + '; };', ctx);
  return { ctx, sent, signed, said, sub, held: () => held };
}

/** The chat's push pieces, as one page load: its refusal words and once-a-load mark (when the
 *  file has them), requestNotifications, subscribeToPush, and the signer it calls, crypto.js
 *  pqSignChatMessage. */
function chatPage(opts) {
  const b = browser(opts);
  const ui = read('chat/chat-ui.js');
  const optional = (mark, end, fallback) => (ui.includes(mark) ? slice(ui, mark, end) : fallback);
  vm.runInContext(optional('var PUSH_REFUSED_WORDS = {', '\n};\n', 'var PUSH_REFUSED_WORDS = {};'), b.ctx, { filename: 'chat-ui.js#words' });
  vm.runInContext(optional('var pushSentThisLoad = false;', '\n', ''), b.ctx, { filename: 'chat-ui.js#pushSentThisLoad' });
  vm.runInContext(slice(ui, 'function requestNotifications() {', '\n}\n'), b.ctx, { filename: 'chat-ui.js#requestNotifications' });
  vm.runInContext(slice(ui, 'function subscribeToPush() {', '\n}\n'), b.ctx, { filename: 'chat-ui.js#subscribeToPush' });
  vm.runInContext(slice(read('chat/crypto.js'), 'async function pqSignChatMessage(', '\n}\n'), b.ctx, { filename: 'crypto.js#pqSignChatMessage' });
  return b;
}

/** Settings' push switch, with the shared signer the page loads (pq-relay-auth.js). */
function settingsPage(opts) {
  const b = browser(opts);
  const app = read('pages/settings-app.js');
  if (app.includes('async function unsubscribeFromPush(')) {
    vm.runInContext(slice(app, 'async function unsubscribeFromPush(', '\n}\n'), b.ctx, { filename: 'settings-app.js#unsubscribeFromPush' });
  }
  vm.runInContext(slice(app, 'async function togglePushNotifications(', '\n}\n'), b.ctx, { filename: 'settings-app.js#togglePushNotifications' });
  vm.runInContext(read('shared/pq-relay-auth.js'), b.ctx, { filename: 'pq-relay-auth.js' });
  return b;
}

test('1. subscribe sends the signed shape, signed over the fixture\'s words', async () => {
  const page = chatPage();
  page.ctx.subscribeToPush();
  await settle();
  assert.strictEqual(page.sent.length, 1, 'one subscribe request');
  assert.strictEqual(page.sent[0].url, '/api/push/subscribe');
  assert.deepStrictEqual(page.signed, [FIXTURE.subscribe_preimage], 'the bytes signed are the fixture\'s subscribe preimage');
  assert.deepStrictEqual(page.sent[0].body, {
    key: 'd1l1th1um', endpoint: FIXTURE.endpoint, p256dh: FIXTURE.p256dh, auth: FIXTURE.auth, timestamp: FIXTURE.timestamp, sig: 'abcd',
  });
  assert.deepStrictEqual(page.said, [], 'nothing to tell the person');
});

test('2. a refusal is told to the person in plain words, by key', async () => {
  const refused = (reason) => ({ ok: false, status: 400, json: () => Promise.resolve({ reason, error: 'relay words' }) });
  for (const [reason, key] of [['private', 'notifications.push_refused_private'], ['not_https', 'notifications.push_refused_not_https'],
    ['replay', 'notifications.push_refused_other'], ['constructor', 'notifications.push_refused_other']]) {
    const page = chatPage({ answer: refused(reason) });
    page.ctx.subscribeToPush();
    await settle();
    assert.deepStrictEqual(page.said, [EN[key]], 'a refusal is told to the person (' + reason + ')');
  }

  // Every reason the relay can answer for a subscribe has its words.
  const guard = fs.readFileSync(path.join(ROOT, 'src', 'relay', 'outbound.rs'), 'utf8').replace(/\r\n/g, '\n');
  const reasonFn = slice(guard, 'pub fn reason(self)', '\n    }\n');
  const reasons = [...reasonFn.matchAll(/=> "([a-z_]+)"/g)].map((m) => m[1]).concat(['keys', 'clock']);
  assert.ok(reasons.length >= 9, 'read the relay\'s reasons: ' + reasons);
  const map = chatPage().ctx.PUSH_REFUSED_WORDS;
  for (const r of reasons) {
    assert.ok(map[r], 'the website has words for the relay\'s reason "' + r + '"');
    assert.ok(EN[map[r]], map[r] + ' is in en.json');
  }
  assert.ok(EN['notifications.push_refused_other'], 'and general words for any other refusal');
});

test('3. turning push off in Settings sends a signed unsubscribe, and the chat then subscribes nothing', async () => {
  const store = storage(BACKUP);
  const page = settingsPage({ store });
  await page.ctx.togglePushNotifications(false);
  await settle();
  assert.ok(page.sub.unsubscribed, 'the browser forgets the subscription');
  assert.strictEqual(page.sent.length, 1, 'the server is told');
  assert.strictEqual(page.sent[0].url, '/api/push/unsubscribe');
  assert.ok(page.sent[0].body.sig, 'the unsubscribe is signed');
  assert.deepStrictEqual(page.sent[0].body, { key: 'd1l1th1um', endpoint: FIXTURE.endpoint, timestamp: FIXTURE.timestamp, sig: 'abcd' });
  assert.deepStrictEqual(page.signed, [FIXTURE.unsubscribe_preimage], 'the bytes signed are the fixture\'s unsubscribe preimage');

  // The chat, loaded afterwards in the same browser, leaves push off.
  const chat = chatPage({ store });
  chat.ctx.requestNotifications();
  await settle();
  assert.deepStrictEqual(chat.sent, [], 'push turned off in Settings stays off');

  // With no identity readable on this page, nothing is sent (the server forgets it on the push
  // service's 410 instead), and the browser still forgets it.
  const bare = settingsPage({ store: storage({}) });
  await bare.ctx.togglePushNotifications(false);
  await settle();
  assert.ok(bare.sub.unsubscribed);
  assert.deepStrictEqual(bare.sent, [], 'no unsigned unsubscribe is sent');
});

test('4. every load sends the subscription again, so one the server forgot comes back', async () => {
  const store = storage(BACKUP);
  const first = chatPage({ store });
  first.ctx.requestNotifications();
  await settle();
  assert.strictEqual(first.sent.length, 1, 'the first load subscribes');
  first.ctx.requestNotifications(); // a reconnect within the same load
  await settle();
  assert.strictEqual(first.sent.length, 1, 'not again on a reconnect within the load');

  // The server forgets it (its push service answered 410): nothing the browser can see. Next load:
  const next = chatPage({ store });
  next.ctx.requestNotifications();
  await settle();
  assert.strictEqual(next.sent.length, 1, 'a forgotten subscription is re-sent on the next load');
  assert.strictEqual(next.sent[0].url, '/api/push/subscribe');
  assert.deepStrictEqual(next.signed, [FIXTURE.subscribe_preimage]);
});

test('5. a subscription held under an older key of this server is dropped and made anew', async () => {
  const page = chatPage({ oldKey: true });
  page.ctx.requestNotifications();
  await settle();
  assert.strictEqual(page.sent.length, 1, 'a new subscription is made under the server\'s current key');
  assert.strictEqual(page.held(), page.sub, 'the browser holds the new one');
});
