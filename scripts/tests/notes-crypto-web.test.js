// The website's Notes seals a locked note with PBKDF2-SHA-256 at 600,000 rounds, the figure the
// app, the vault and docs/reference/cryptography.md promise (2026-10-24; it was 300,000). An older
// version-1 note (300,000) still opens. Run: node --test scripts/tests/notes-crypto-web.test.js
//
// The crypto helpers are read out of web/pages/notes-app.js (the text between its "Crypto helpers"
// and "Data helpers" headings) and run with Node's own WebCrypto, so the test checks the page's
// real code. NOTES_APP=<path> points it at another copy (how the red run below was made).
//
// Seen red 2026-10-24 against the 300,000-round helpers (git show HEAD~1:web/pages/notes-app.js):
// "a new note is sealed as version 2" (1 !== 2), and the note did not open with 600,000 rounds.
'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const FILE = process.env.NOTES_APP || path.join(__dirname, '..', '..', 'web', 'pages', 'notes-app.js');

function helpers() {
  const src = fs.readFileSync(FILE, 'utf8');
  const start = src.indexOf('// ── Crypto helpers');
  const end = src.indexOf('// ── Data helpers');
  assert.ok(start >= 0 && end > start, 'the crypto helpers are where the test expects them');
  const ctx = vm.createContext({ crypto: globalThis.crypto, TextEncoder, TextDecoder, btoa, atob, Uint8Array, String, Error });
  vm.runInContext(src.slice(start, end) + '\n;globalThis.__h = { aesEncrypt, aesDecrypt };', ctx);
  return ctx.__h;
}

const b64 = (bytes) => Buffer.from(bytes).toString('base64');
const unb64 = (s) => new Uint8Array(Buffer.from(s, 'base64'));

async function keyFor(passphrase, salt, rounds, use) {
  const mat = await crypto.subtle.importKey('raw', new TextEncoder().encode(passphrase), 'PBKDF2', false, ['deriveKey']);
  return crypto.subtle.deriveKey({ name: 'PBKDF2', salt, iterations: rounds, hash: 'SHA-256' }, mat, { name: 'AES-GCM', length: 256 }, false, [use]);
}

test('a new note is sealed as version 2 and opens only with 600,000 rounds', async () => {
  const { aesEncrypt } = helpers();
  const bundle = await aesEncrypt('the plan for Tuesday', 'correct horse');
  assert.strictEqual(bundle.v, 2, 'a new note is sealed as version 2');
  const key = await keyFor('correct horse', unb64(bundle.salt), 600000, 'decrypt');
  const plain = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: unb64(bundle.iv) }, key, unb64(bundle.ct));
  assert.strictEqual(new TextDecoder().decode(plain), 'the plan for Tuesday', 'the note opened with 600,000 rounds');
});

test('a version-1 note (300,000 rounds) still opens, and an unknown version is refused in words', async () => {
  const { aesDecrypt } = helpers();
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await keyFor('pass', salt, 300000, 'encrypt');
  const ct = await crypto.subtle.encrypt({ name: 'AES-GCM', iv }, key, new TextEncoder().encode('old note'));
  const v1 = { v: 1, salt: b64(salt), iv: b64(iv), ct: b64(new Uint8Array(ct)) };
  assert.strictEqual(await aesDecrypt(v1, 'pass'), 'old note', 'a version-1 note opens');
  await assert.rejects(aesDecrypt({ ...v1, v: 9 }, 'pass'), /format this page does not know/, 'an unknown version is refused in words');
});
