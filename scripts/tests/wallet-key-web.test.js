// The website's wallet key is derived from the identity seed under its own label, byte for byte
// the desktop app's (2026-11-19, the PQC review's item 2).
// Run: node --test scripts/tests/wallet-key-web.test.js
//
// Until 2026-11-19 the 32 bytes the recovery phrase writes down WERE the wallet's Ed25519 key,
// so exporting the wallet key to any wallet tool handed over the root of the chat identity and of
// every past private message (docs/reference/research/2026-11-16-pqc-review/). Now the wallet key
// is BLAKE3 derive_key("hum/ed25519-wallet/v1", seed). This loads web/shared/wallet.js as the
// browser does, gives it the vendored noble BLAKE3 the site ships and Node's WebCrypto Ed25519
// (a stack independent of the desktop app's blake3 and ed25519-dalek crates), and pins the same
// three values as the Rust test src/net/identity.rs wallet_key_known_answer_for_the_frozen_phrase
// for the frozen seed [7;32] ("alpha deal scrub ... bright truly"): the same phrase shows the same
// wallet address on both apps.
//
// Seen red 2026-11-19 against a copy of wallet.js whose walletSecretFromSeed returned the seed
// itself (the old relationship, WALLET_LIB=<that copy>): both tests failed, "secret: expected
// d883ea7c...6e73, got 0707...07" and "the wallet secret is the recovery phrase's own 32 bytes".
// (The old wallet address for this seed was GmaDrppBC7P5ARKV8g3djiwP89vz1jLK23V2GBjuAEGB.)
'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');
const { pathToFileURL } = require('url');

const ROOT = path.join(__dirname, '..', '..');
const LIB = process.env.WALLET_LIB || path.join(ROOT, 'web', 'shared', 'wallet.js');
const BUNDLE = path.join(ROOT, 'web', 'shared', 'vendor', 'noble-pq.bundle.js');

// The same constants as the Rust test (src/net/identity.rs): editing the derivation must break
// both or neither.
const KAT_SEED = new Uint8Array(32).fill(7);
const KAT_SECRET = 'd883ea7c52a0db010de32a89fef61181bda081710736b236c1184ff224496e73';
const KAT_PUBKEY = 'a697fa9ba1a4794c49a86dd87a3798b61775c9e436848e3331df855b84f56e5e';
const KAT_ADDRESS = 'CDK69dGcrdKp2LFhmeKGnCg3mFv6ZhpPLHvqqgQB5vGd';

const hex = (b) => Buffer.from(b).toString('hex');

function loadWallet() {
  const store = {};
  const window = {};
  const ctx = {
    window, console, crypto: globalThis.crypto, TextEncoder, atob, btoa, BigInt, Uint8Array,
    fetch: () => Promise.reject(new Error('no network in tests')),
    localStorage: { getItem: (k) => (k in store ? store[k] : null), setItem: (k, v) => { store[k] = String(v); } },
  };
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(LIB, 'utf8'), ctx, { filename: LIB });
  assert.ok(window.HosWallet, 'wallet.js exposes window.HosWallet');
  return window.HosWallet;
}

async function blake3() {
  const m = await import(pathToFileURL(BUNDLE).href);
  assert.ok(m.blake3, 'the vendored bundle exports blake3');
  return m.blake3;
}

/** The Ed25519 public key WebCrypto makes for a 32-byte private key (the old wallet's maker). */
async function ed25519Public(secret) {
  const prefix = [0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20];
  const p = new Uint8Array(48);
  p.set(prefix, 0);
  p.set(secret, 16);
  const k = await crypto.subtle.importKey('pkcs8', p, 'Ed25519', true, ['sign']);
  return hex(Buffer.from((await crypto.subtle.exportKey('jwk', k)).x, 'base64url'));
}

test('the wallet key for the frozen test phrase matches the desktop app', async () => {
  const W = loadWallet();
  const b3 = await blake3();
  assert.strictEqual(W.WALLET_KEY_DOMAIN, 'hum/ed25519-wallet/v1');
  const secret = await W.walletSecretFromSeed(KAT_SEED, b3);
  assert.strictEqual(hex(secret), KAT_SECRET, `secret: expected ${KAT_SECRET}, got ${hex(secret)}`);
  const wallet = await W.walletKeyFromSeed(KAT_SEED, b3);
  assert.strictEqual(wallet.publicKeyHex, KAT_PUBKEY);
  assert.strictEqual(wallet.address, KAT_ADDRESS);
  assert.strictEqual(W.publicKeyToSolanaAddress(wallet.publicKeyHex), KAT_ADDRESS, 'the address is the public key in base58');
  // The key really signs as that public key (what a Solana transfer needs).
  const msg = new TextEncoder().encode('a transfer message');
  const sig = await crypto.subtle.sign('Ed25519', wallet.privateKey, msg);
  assert.ok(await crypto.subtle.verify('Ed25519', wallet.publicKey, sig, msg), 'signs as its public key');
  // The 64-byte export a wallet tool takes is [the derived secret | its public key].
  const kp = await W.exportSolanaKeypair(KAT_SEED, b3);
  assert.strictEqual(hex(kp), KAT_SECRET + KAT_PUBKEY);
});

test('the wallet secret is not the recovery phrase entropy', async () => {
  const W = loadWallet();
  const b3 = await blake3();
  const counting = Uint8Array.from({ length: 32 }, (_, i) => i);
  for (const seed of [KAT_SEED, new Uint8Array(32), new Uint8Array(32).fill(0xff), counting]) {
    const secret = await W.walletSecretFromSeed(seed, b3);
    assert.notStrictEqual(hex(secret), hex(seed), "the wallet secret is the recovery phrase's own 32 bytes");
    const dil = b3.create({ context: new TextEncoder().encode('hum/dilithium3/v1'), dkLen: 32 }).update(seed).digest();
    assert.notStrictEqual(hex(secret), hex(dil), "the wallet secret is the chat identity's seed");
    const wallet = await W.walletKeyFromSeed(seed, b3);
    assert.notStrictEqual(wallet.publicKeyHex, await ed25519Public(seed), 'the wallet is still the Ed25519 key of the seed itself');
  }
  await assert.rejects(W.walletSecretFromSeed(new Uint8Array(16), b3), /32 bytes/, 'a wrong-sized seed makes no wallet');
});

// No page makes a wallet address out of an identity key or a stored public key: the only source of
// an address is the derived wallet key. Before 2026-11-19 the Wallet page read localStorage
// humanity_pubkey, the Settings page and every profile card passed myIdentity.publicKeyHex or the
// person's identity key (Dilithium3 since the PQ cutover, never an address) to
// publicKeyToSolanaAddress, and both pages that show the address must load the seed reader and the
// wallet library before their own script.
// Seen red 2026-11-19 against the pages at HEAD (WALLET_TREE=<git archive HEAD web>): "web/chat/chat-profile.js, web/pages/settings-app.js,
// web/pages/wallet-app.js" called publicKeyToSolanaAddress.
test('no page makes the wallet address from an identity key', () => {
  const TREE = process.env.WALLET_TREE || ROOT; // another checkout's tree, for the red run
  const pages = path.join(TREE, 'web');
  const callers = [];
  (function walk(dir) {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) { if (e.name !== 'vendor') walk(p); continue; }
      const rel = path.relative(TREE, p).split(path.sep).join('/');
      if (!e.name.endsWith('.js') || rel === 'web/shared/wallet.js') continue;
      const src = fs.readFileSync(p, 'utf8');
      if (/publicKeyToSolanaAddress\s*\(/.test(src) || /humanity_pubkey/.test(src) && /wallet/i.test(e.name)) {
        callers.push(rel);
      }
    }
  })(pages);
  assert.deepStrictEqual(callers.sort(), [], `these make an address from a stored or identity key: ${callers.join(', ')}`);
  for (const [html, app] of [['web/pages/wallet.html', '/pages/wallet-app.js'], ['web/pages/settings.html', '/pages/settings-app.js']]) {
    const src = fs.readFileSync(path.join(TREE, html), 'utf8');
    const at = (needle) => src.indexOf(needle);
    assert.ok(at('/shared/wallet.js') >= 0 && at('/shared/wallet.js') < at(app), `${html} loads wallet.js before ${app}`);
    assert.ok(at('/shared/pq-relay-auth.js') >= 0 && at('/shared/pq-relay-auth.js') < at(app), `${html} loads the seed reader before ${app}`);
  }
});
