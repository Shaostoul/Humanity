// The website's settings sync (notes, to-dos, the garden, settings) leaves the browser only sealed
// with a key from the person's recovery phrase, and only a sealed copy from the server is applied
// (BUG-283, 2026-11-07). Run: node --test scripts/tests/sync-sealed-web.test.js
//
// The sync code is read out of web/chat/app.js (the text between its "User Data Sync" heading and
// "Watch localStorage changes") and run with Node's own WebCrypto against stubs for the socket,
// the storage and the notice, so the test checks the page's real code. SYNC_APP=<path> points it
// at another copy (how the red run below was made).
//
// Seen red 2026-11-07 against the code before the fix (git show HEAD:web/chat/app.js): with no key
// the blob went out as plain text ("nothing is sent without a key" failed: one sync_save carrying
// the note), a failed seal sent it plain too, and an unsealed copy from the server was applied.
'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const FILE = process.env.SYNC_APP || path.join(__dirname, '..', '..', 'web', 'chat', 'app.js');

/** The page's sync code in a sandbox: `seed` is what identitySeed() gives (null: no key). */
function load({ seed, failSeal = false } = {}) {
  const src = fs.readFileSync(FILE, 'utf8');
  const start = src.indexOf('// ── User Data Sync ──');
  const end = src.indexOf('// Watch localStorage changes');
  assert.ok(start >= 0 && end > start, 'the sync code is where the test expects it');
  const store = new Map([['humanity_notes', JSON.stringify([{ text: 'the plan for Tuesday' }])]]);
  const sent = [];
  const notices = [];
  const pending = [];
  const subtle = failSeal
    ? Object.assign(Object.create(globalThis.crypto.subtle), {
        importKey: (...a) => globalThis.crypto.subtle.importKey(...a),
        digest: (...a) => globalThis.crypto.subtle.digest(...a),
        encrypt: () => Promise.reject(new Error('sealing broke')),
      })
    : globalThis.crypto.subtle;
  const i18n = { t: (k) => k };
  const ctx = vm.createContext({
    crypto: { subtle, getRandomValues: (a) => globalThis.crypto.getRandomValues(a) },
    TextEncoder, TextDecoder, btoa, atob, Uint8Array, String, JSON, Date, parseInt, console: { warn() {}, log() {} },
    identitySeed: async () => seed,
    pkcs8FromSeed: (s) => new Uint8Array([48, 46, ...s]),
    myKey: 'my-public-key',
    WebSocket: { OPEN: 1 },
    ws: { readyState: 1, send: (m) => sent.push(JSON.parse(m)) },
    localStorage: { getItem: (k) => (store.has(k) ? store.get(k) : null), setItem: (k, v) => store.set(k, String(v)) },
    setTimeout: (fn) => { pending.push(Promise.resolve().then(fn)); return pending.length; },
    clearTimeout: () => {},
    addNotice: (text) => notices.push(text),
    window: { i18n },
    i18n,
  });
  vm.runInContext(src.slice(start, end) + '\n;globalThis.__s = { encryptSyncData, decryptSyncData, scheduleSyncSave, handleSyncData };', ctx);
  const settle = async () => { while (pending.length) await pending.shift(); };
  return { s: ctx.__s, sent, notices, settle, store };
}

test('nothing is sent without a key, and the page says why, once', async () => {
  const { s, sent, notices, settle } = load({ seed: null });
  await s.handleSyncData('null'); // the server has nothing: push this device's copy
  await settle();
  s.scheduleSyncSave();
  await settle();
  assert.strictEqual(sent.length, 0, `nothing is sent without a key: ${JSON.stringify(sent)}`);
  assert.deepStrictEqual(notices, ['chat.sync_not_sealed'], 'the page says why, once a page load');
});

test('a failed seal sends nothing', async () => {
  const { s, sent, settle } = load({ seed: new Uint8Array(32).fill(5), failSeal: true });
  await s.handleSyncData('null');
  await settle();
  assert.strictEqual(sent.length, 0, `a failed seal sends nothing: ${JSON.stringify(sent)}`);
  assert.strictEqual(await s.encryptSyncData('{"humanity_notes":"x"}'), null);
});

test('with a key the copy goes out sealed, and it opens again', async () => {
  const seed = new Uint8Array(32).fill(5);
  const { s, sent, settle } = load({ seed });
  await s.handleSyncData('null');
  await settle();
  assert.strictEqual(sent.length, 1, 'one save');
  assert.strictEqual(sent[0].type, 'sync_save');
  const blob = JSON.parse(sent[0].data);
  assert.strictEqual(blob.v, 1);
  assert.ok(blob.iv && blob.encrypted, 'sealed');
  assert.ok(!sent[0].data.includes('Tuesday'), 'the words never travel in the clear');
  const opened = await s.decryptSyncData(sent[0].data);
  assert.ok(opened.includes('the plan for Tuesday'), 'and it opens with the same key');
});

test('an unsealed copy from the server is never applied', async () => {
  const { s } = load({ seed: new Uint8Array(32).fill(5) });
  const plain = JSON.stringify({ humanity_settings: '{"theme":"evil"}' });
  assert.strictEqual(await s.decryptSyncData(plain), null, 'plain JSON is not applied');
  assert.strictEqual(await s.decryptSyncData('not json at all'), null, 'nor anything that is not a sealed copy');
  const badSeal = JSON.stringify({ v: 1, key: 'my-public-key', iv: 'AAAAAAAAAAAAAAAA', encrypted: 'AAAAAAAAAAAAAAAAAAAAAA==' });
  assert.strictEqual(await s.decryptSyncData(badSeal), null, 'nor a seal that does not open');
});
