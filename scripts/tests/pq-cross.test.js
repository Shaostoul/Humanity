// Post-quantum cross-implementation tests, the website's side (2026-11-19, the post-quantum
// review's plan item 1b, docs/reference/research/2026-11-16-pqc-review/).
//
// Run: node --test scripts/tests/pq-cross.test.js   (discovered by `just rig-tests`)
//
// The known-answer tests (scripts/pq-kat.mjs) only prove the website and the Rust code derive the
// same keys from one seed. These check what the Rust code MADE with its keys, using the website's
// library (the vendored noble bundle) and the website's own chat code (web/chat/pq.js and
// web/chat/crypto.js in a node:vm page, scripts/lib/chat-pq-page.js):
//   - ML-KEM-768: a ciphertext the Rust code encapsulated decapsulates here to its shared secret,
//     and the same fixed randomness encapsulates to the same bytes here;
//   - ML-DSA-65: the Rust code's signature verifies through pq.js pqVerifyMessage, and noble's
//     deterministic signature is the Rust signature byte for byte;
//   - a complete sealed-sender v2 envelope the Rust code sealed (src/net/dm_pq.rs) opens with
//     crypto.js pqOpenDmEnvelope, and its inner Dilithium3 signature proves its sender.
// The Rust tests check the other way round (crates/humanity-protocol/src/pq_cross_tests.rs,
// src/net/dm_pq_cross_tests.rs). The fixtures, and how each half is written:
// scripts/pq-cross-fixtures.mjs. HOS_WEB_DIR points the page at another copy of web/.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const { REPO, loadNoble, keysOf, chatPage } = require("../lib/chat-pq-page.js");

// HOS_PQ_CROSS_DIR: a folder holding changed copies of the two fixtures (how each test was seen red;
// the Rust tests read the same variable).
const DIR = process.env.HOS_PQ_CROSS_DIR || path.join(REPO, "scripts", "tests", "fixtures");
const read = (name) => JSON.parse(fs.readFileSync(path.join(DIR, name), "utf8"));
const PQ = read("pq-cross-v1.json");
const DM = read("dm-envelope-v2.json");

const hex = (b) => Buffer.from(b).toString("hex");
const unb64 = (s) => new Uint8Array(Buffer.from(s, "base64"));
const unhex = (s) => new Uint8Array(Buffer.from(s, "hex"));
const enc = (s) => new TextEncoder().encode(s);

/** A copy of a base64 value with one byte flipped. */
function flipped(b64, at) {
  const b = Buffer.from(b64, "base64");
  b[at] ^= 0x01;
  return b.toString("base64");
}

// Seen red 2026-11-19 with one byte of rust.kem.ct_b64 flipped: "noble decapsulates the Rust
// ciphertext to the Rust shared secret" failed (ML-KEM's implicit rejection gives another secret,
// not an error, so the comparison is the check).
test("ML-KEM-768: the Rust code encapsulated to our key, and noble decapsulates to its shared secret", async () => {
  const m = await loadNoble();
  const k = PQ.rust.kem;
  const mine = await keysOf(k.recipient_master_byte);
  assert.strictEqual(Buffer.from(mine.kyb.publicKey).toString("base64"), k.ek_b64, "the Rust half's key is the one noble derives");
  const ct = unb64(k.ct_b64);
  assert.strictEqual(ct.length, 1088, "an ML-KEM-768 ciphertext");
  assert.strictEqual(hex(m.ml_kem768.decapsulate(ct, mine.kyb.secretKey)), k.ss_hex, "noble decapsulates the Rust ciphertext to the Rust shared secret");
});

// Seen red 2026-11-19 with one hex digit of rust.kem.deterministic.ct_blake3 changed: "the same
// ciphertext" failed.
test("ML-KEM-768: the same randomness encapsulates to the same bytes as the Rust code", async () => {
  const m = await loadNoble();
  const k = PQ.rust.kem;
  const { cipherText, sharedSecret } = m.ml_kem768.encapsulate(unb64(k.ek_b64), unhex(k.deterministic.m_hex));
  assert.strictEqual(hex(m.blake3(cipherText, { dkLen: 32 })), k.deterministic.ct_blake3, "the same ciphertext");
  assert.strictEqual(hex(sharedSecret), k.deterministic.ss_hex, "the same shared secret");
});

// Seen red 2026-11-19 with one byte of rust.dsa.sig_b64 flipped: "the Rust signature verifies
// through the website's pqVerifyMessage" failed.
test("ML-DSA-65: the Rust signature verifies in the website's code, and noble's deterministic signature is the same bytes", async () => {
  const m = await loadNoble();
  const d = PQ.rust.dsa;
  const page = await chatPage(d.signer_master_byte);
  const verify = page.fn("pqVerifyMessage");
  const { dil } = await keysOf(d.signer_master_byte);
  const sig = unb64(d.sig_b64);
  assert.strictEqual(sig.length, 3309, "an ML-DSA-65 signature");
  assert.strictEqual(await verify(dil.publicKey, enc(d.message), sig), true, "the Rust signature verifies through the website's pqVerifyMessage");
  assert.strictEqual(await verify(dil.publicKey, enc(d.message + "!"), sig), false, "another message");
  assert.strictEqual(await verify(dil.publicKey, enc(d.message), unb64(flipped(d.sig_b64, 100))), false, "a flipped byte");
  assert.strictEqual(
    Buffer.from(m.ml_dsa65.sign(enc(d.message), dil.secretKey, { extraEntropy: false })).toString("base64"),
    d.sig_b64,
    "noble's deterministic signature (FIPS 204, rnd all zeros, empty context) is the Rust one byte for byte",
  );
});

// Seen red 2026-11-19 with one hex digit of noble.kem.deterministic.ss_hex changed: "noble's
// deterministic encapsulation, reproduced" failed.
test("the noble half is still what noble makes", async () => {
  const m = await loadNoble();
  const k = PQ.noble.kem;
  const mine = await keysOf(PQ.rust.kem.recipient_master_byte);
  assert.strictEqual(k.ek_blake3, hex(m.blake3(unb64(PQ.rust.kem.ek_b64), { dkLen: 32 })), "noble encapsulated to the Rust half's key");
  assert.strictEqual(hex(m.ml_kem768.decapsulate(unb64(k.ct_b64), mine.kyb.secretKey)), k.ss_hex, "its randomized ciphertext opens to its secret");
  const det = m.ml_kem768.encapsulate(unb64(PQ.rust.kem.ek_b64), unhex(k.deterministic.m_hex));
  assert.deepStrictEqual(
    { ct_blake3: hex(m.blake3(det.cipherText, { dkLen: 32 })), ss_hex: hex(det.sharedSecret) },
    { ct_blake3: k.deterministic.ct_blake3, ss_hex: k.deterministic.ss_hex },
    "noble's deterministic encapsulation, reproduced",
  );
  const d = PQ.noble.dsa;
  const { dil } = await keysOf(d.signer_master_byte);
  assert.strictEqual(Buffer.from(m.ml_dsa65.sign(enc(d.message), dil.secretKey, { extraEntropy: false })).toString("base64"), d.sig_deterministic_b64, "noble's deterministic signature, reproduced");
  assert.ok(m.ml_dsa65.verify(unb64(d.sig_hedged_b64), enc(d.message), dil.publicKey), "its hedged signature verifies");
});

// Seen red 2026-11-19 with one byte of the Rust envelope's ct_b64 flipped: "the Rust envelope opens
// in the website's code" failed (pqOpenDmEnvelope returned null: AES-GCM's tag).
test("a sealed-sender v2 envelope the Rust code sealed opens in the website's code, and its sender is proven", async () => {
  const r = DM.rust;
  const recipient = await chatPage(r.recipient_master_byte);
  assert.strictEqual(recipient.me.kyber, r.recipient_kyber_b64, "the page's own DM key is the one the Rust code sealed to");
  const inner = await recipient.fn("pqOpenDmEnvelope")(r.envelope);
  assert.ok(inner, "the Rust envelope opens in the website's code");
  assert.strictEqual(inner.from, (await keysOf(r.sender_master_byte)).key, "from the sender, its signature checked by pqOpenDmEnvelope");
  assert.strictEqual(inner.to, recipient.me.key, "to me");
  assert.strictEqual(inner.ts, r.ts, "the time the Rust code signed");
  assert.strictEqual(inner.text, r.text, "the text the Rust code signed, its UTF-8 intact");
  assert.strictEqual(inner.text, DM.web.text, "the same text as the website's half");
  // Tampered or sent to someone else: nothing opens.
  const env = JSON.parse(r.envelope);
  assert.strictEqual(await recipient.fn("pqOpenDmEnvelope")(JSON.stringify({ ...env, ct_b64: flipped(env.ct_b64, 40) })), null, "a flipped byte in the body");
  assert.strictEqual(await recipient.fn("pqOpenDmEnvelope")(JSON.stringify({ ...env, ek_ct_b64: flipped(env.ek_ct_b64, 40) })), null, "a flipped byte in the encapsulation");
  const sender = await chatPage(r.sender_master_byte);
  assert.strictEqual(await sender.fn("pqOpenDmEnvelope")(r.envelope), null, "the sender's own key does not open the recipient's copy");
});

// Seen red 2026-11-19 with one character of web.text changed: "the text the website signed" failed.
test("the website's envelope is still what the website's code opens", async () => {
  const w = DM.web;
  assert.strictEqual(w.sealed_to, DM.rust.recipient_kyber_b64, "sealed to the Rust half's recipient key");
  assert.deepStrictEqual(Object.keys(JSON.parse(w.envelope)).sort(), ["ct_b64", "ek_ct_b64", "nonce_b64", "v"], "nobody's key, no sender");
  const recipient = await chatPage(w.recipient_master_byte);
  const inner = await recipient.fn("pqOpenDmEnvelope")(w.envelope);
  assert.ok(inner, "the website's envelope opens");
  assert.strictEqual(inner.from, (await keysOf(w.sender_master_byte)).key);
  assert.strictEqual(inner.ts, w.ts);
  assert.strictEqual(inner.text, w.text, "the text the website signed");
});
