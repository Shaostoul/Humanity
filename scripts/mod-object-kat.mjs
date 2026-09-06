#!/usr/bin/env node
/**
 * mod-object-kat.mjs — cross-language known-answer test for the SIGNED
 * MODERATION objects: `mod_action_v1` and `space_policy_v1`.
 *
 * Why this exists. The moderator's CLIENT signs a moderation action, never the
 * relay: a relay cannot hold a moderator's secret key, and a log signed by the
 * party whose behaviour it exists to constrain proves only that the relay
 * agrees with itself. That is the decision that unblocked rung 1 of
 * docs/design/signed_moderation_logs.md after four months.
 *
 * The consequence is that a browser and the Rust relay must encode these
 * objects IDENTICALLY, byte for byte. If they drift, an action signed in a
 * browser is rejected as an invalid signature, and the audit log the whole
 * design exists to provide silently contains only the actions taken from the
 * other client. That failure is quiet, which is exactly why it needs a KAT.
 *
 * Asserts, against the Rust goldens in
 * src/relay/core/moderation.rs::cross_language_kat (duplicated here on purpose,
 * so editing the encoding must break BOTH or neither):
 *   1. payload bytes are byte-identical for both object types,
 *   2. the SIGNABLE bytes are byte-identical (length + BLAKE3),
 *   3. the frozen deterministic RUST signatures VERIFY under noble over the
 *      JS-computed signable bytes. A Dilithium verify binds the exact message,
 *      so a pass is cryptographic proof of byte equality, not a hash comparison
 *      we could have fooled ourselves with,
 *   4. splicing the Rust signature into the JS object reproduces the Rust
 *      object_id (full-object byte equality),
 *   5. the validation rules match: the same inputs are refused on both sides.
 *
 * Run: `node scripts/mod-object-kat.mjs` (or `just mod-kat`).
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { join } from 'node:path';

const REPO = join(fileURLToPath(import.meta.url), '..', '..');
const CBOR = join(REPO, 'web', 'shared', 'canonical-cbor.js');
const OBJ = join(REPO, 'web', 'shared', 'pq-object.js');
const BUNDLE = join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js');

// GOLDEN — must equal src/relay/core/moderation.rs::cross_language_kat.
const KAT_CREATED_AT = 1_751_328_000_000;
const KAT_SPACE = 'united-humanity.us';
const KAT_TARGET = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';
const KAT_REASON = 'repeated harassment in #general after a warning';
const KAT_RULE = 'respect-every-person';

const GOLD = {
  mod_action: {
    payloadHex:
      'a764726f6c65606472756c6574726573706563742d65766572792d706572736f6e66616374696f6e646d75746566726561736f6e782f7265706561746564206861726173736d656e7420696e202367656e6572616c2061667465722061207761726e696e67667461726765747840303132333435363738396162636465663031323334353637383961626364656630313233343536373839616263646566303132333435363738396162636465666a657870697265735f61741b00000197c34888006b7461726765745f6b696e64686964656e74697479',
    signableLen: 5684,
    signableBlake3: 'f4c5e89ab9c8edca0eb1f7e3a0c6cd6dfefe76c64b69484982e760d9da2b6c20',
    objectId: '8fc28ee60cb7b2bf9201bfacbe26a8b1a414a7c0cd7da60b45e65eefdea03dcf',
    sigFixture: join(REPO, 'src', 'relay', 'core', 'pq_kat_mod_action_sig.hex'),
  },
  space_policy: {
    payloadHex:
      'a6656f776e65727840303132333435363738396162636465663031323334353637383961626364656630313233343536373839616263646566303132333435363738396162636465666761707065616c7378364f70656e20616e206973737565206174206769746875622e636f6d2f5368616f73746f756c2f48756d616e6974792f6973737565732e6972756c65735f75726c782068747470733a2f2f756e697465642d68756d616e6974792e75732f72756c65736a6d6f64657261746f7273827840313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131317840323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232326a72756c65735f6861736878403333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333370756e7369676e65645f616c6c6f77656482646869646566756e68696465',
    signableLen: 5904,
    signableBlake3: 'd07496f0b69220adeab8640616d735d247ec2f8410c67df86a6484dd9de73b03',
    objectId: 'e56f6806d31ab08523cae62a5831535cb0faae634f122c9f38ea3d8aa4b00749',
    sigFixture: join(REPO, 'src', 'relay', 'core', 'pq_kat_space_policy_sig.hex'),
  },
};

const hex = (b) => Buffer.from(b).toString('hex');
const fromHex = (h) => Uint8Array.from(Buffer.from(h.trim(), 'hex'));

const cbor = await import(pathToFileURL(CBOR).href);
const obj = await import(pathToFileURL(OBJ).href);
const noble = await import(pathToFileURL(BUNDLE).href);

// The canonical KAT master seed, same as the Dilithium and vote KATs.
const master = new Uint8Array(32).fill(7);
const dilSeed = noble.blake3
  .create({ context: new TextEncoder().encode('hum/dilithium3/v1'), dkLen: 32 })
  .update(master)
  .digest();
const kp = noble.ml_dsa65.keygen(dilSeed);
const blake3 = (d) => noble.blake3.create({ dkLen: 32 }).update(d).digest();
const sign = async (msg) => noble.ml_dsa65.sign(msg, kp.secretKey);

let failures = 0;
function check(name, pass, detail) {
  if (!pass) failures++;
  console.log(`${pass ? 'PASS' : 'FAIL'}  ${name}${detail ? '  ->  ' + detail : ''}`);
}

async function verifyType(label, built, gold) {
  // 1. payload bytes
  check(`${label}: payload bytes match Rust`, hex(built.payloadBytes) === gold.payloadHex,
    hex(built.payloadBytes) === gold.payloadHex ? `${built.payloadBytes.length} bytes` : 'DRIFTED');

  // 2. signable bytes
  check(`${label}: signable length matches`, built.signable.length === gold.signableLen,
    `${built.signable.length} vs ${gold.signableLen}`);
  const b3 = hex(blake3(built.signable));
  check(`${label}: signable BLAKE3 matches`, b3 === gold.signableBlake3, b3);

  // 3. the FROZEN RUST SIGNATURE verifies over the JS signable bytes.
  //    This is the load-bearing assertion: a Dilithium verify binds the exact
  //    message, so it cannot pass unless the bytes are identical.
  const rustSig = fromHex(readFileSync(gold.sigFixture, 'utf8'));
  const ok = noble.ml_dsa65.verify(rustSig, built.signable, kp.publicKey);
  check(`${label}: the Rust signature verifies over JS-computed bytes`, ok === true, String(ok));

  // 4. splicing the Rust signature reproduces the Rust object_id
  const spliced = cbor.encodeObjectCanonical({ ...built.base, signature: rustSig });
  const id = hex(blake3(spliced));
  check(`${label}: object_id matches Rust`, id === gold.objectId, id);
}

// ── mod_action_v1 ──────────────────────────────────────────────────────────
{
  const payloadBytes = cbor.encodeCanonical
    ? cbor.encodeCanonical(obj.modActionV1Payload({
        action: 'mute', target: KAT_TARGET, targetKind: 'identity',
        reason: KAT_REASON, rule: KAT_RULE, expiresAt: KAT_CREATED_AT, role: '',
      }))
    : obj.modActionV1Payload({
        action: 'mute', target: KAT_TARGET, targetKind: 'identity',
        reason: KAT_REASON, rule: KAT_RULE, expiresAt: KAT_CREATED_AT, role: '',
      });
  const base = {
    protocol_version: 1,
    object_type: 'mod_action_v1',
    space_id: KAT_SPACE,
    channel_id: null,
    author_public_key: kp.publicKey,
    created_at: KAT_CREATED_AT,
    references: [],
    payload_schema_version: 1,
    payload_encoding: 'cbor_canonical_v1',
    payload: payloadBytes,
    signature: new Uint8Array(3309),
  };
  const signable = cbor.encodeObjectCanonical(base);
  await verifyType('mod_action_v1', { payloadBytes, base, signable }, GOLD.mod_action);
}

// ── space_policy_v1 ────────────────────────────────────────────────────────
{
  const payloadBytes = obj.spacePolicyV1Payload({
    owner: KAT_TARGET,
    moderators: ['11'.repeat(32), '22'.repeat(32)],
    rulesUrl: 'https://united-humanity.us/rules',
    rulesHash: '33'.repeat(32),
    appeals: 'Open an issue at github.com/Shaostoul/Humanity/issues.',
    unsignedAllowed: ['hide', 'unhide'],
  });
  const base = {
    protocol_version: 1,
    object_type: 'space_policy_v1',
    space_id: KAT_SPACE,
    channel_id: null,
    author_public_key: kp.publicKey,
    created_at: KAT_CREATED_AT,
    references: [],
    payload_schema_version: 1,
    payload_encoding: 'cbor_canonical_v1',
    payload: payloadBytes,
    signature: new Uint8Array(3309),
  };
  const signable = cbor.encodeObjectCanonical(base);
  await verifyType('space_policy_v1', { payloadBytes, base, signable }, GOLD.space_policy);
}

// ── 5. the validation rules agree across languages ─────────────────────────
// Each of these is refused by moderation.rs with the matching error; if the JS
// side accepted one, a browser could publish an object the relay must reject,
// which reads to a moderator as "the button does nothing".
const refusals = [
  ['empty reason', () => obj.modActionV1Payload({ action: 'ban', target: 'x', targetKind: 'identity', reason: '   ' })],
  ['unknown action', () => obj.modActionV1Payload({ action: 'vaporize', target: 'x', targetKind: 'identity', reason: 'r' })],
  ['unknown target_kind', () => obj.modActionV1Payload({ action: 'ban', target: 'x', targetKind: 'vibes', reason: 'r' })],
  ['empty target', () => obj.modActionV1Payload({ action: 'ban', target: '', targetKind: 'identity', reason: 'r' })],
  ['grant_role without a role', () => obj.modActionV1Payload({ action: 'grant_role', target: 'x', targetKind: 'identity', reason: 'r' })],
  ['role on a non-role action', () => obj.modActionV1Payload({ action: 'ban', target: 'x', targetKind: 'identity', reason: 'r', role: 'mod' })],
  ['policy with no owner', () => obj.spacePolicyV1Payload({ owner: '' })],
  ['policy allowing an unknown action', () => obj.spacePolicyV1Payload({ owner: 'o', unsignedAllowed: ['vaporize'] })],
];
for (const [name, fn] of refusals) {
  let threw = false;
  try { fn(); } catch (_e) { threw = true; }
  check(`refused: ${name}`, threw);
}

// A well-formed action must still build, or the checks above prove nothing.
let built = false;
try {
  obj.modActionV1Payload({ action: 'ban', target: 'x', targetKind: 'identity', reason: 'spam', rule: '', expiresAt: 0, role: '' });
  built = true;
} catch (e) { console.log('  (build error: ' + e.message + ')'); }
check('a valid action still builds', built);

console.log(failures === 0
  ? '\nmoderation object KAT: all checks passed'
  : `\nmoderation object KAT: ${failures} FAILED — do not ship, browser-signed moderation would be unverifiable`);
process.exit(failures === 0 ? 0 : 1);
