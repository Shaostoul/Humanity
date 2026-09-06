#!/usr/bin/env node
/**
 * import-offerings.mjs — the Market bulk importer (v0.1140).
 *
 * Publishes a provider_v1 entry and a batch of offering_v1 objects onto a
 * relay's signed-object substrate (POST /api/v2/objects), signed with the
 * merchant's Dilithium3 identity derived from their 32-byte master seed --
 * the SAME derivation the chat client uses, so the merchant identity IS
 * their chat identity. Schemas: schemas/provider.toml, schemas/offering.toml.
 * The relay validates every payload at its storage chokepoint; anything this
 * script builds wrong is rejected with a plain-language reason.
 *
 * Usage:
 *   node scripts/import-offerings.mjs \
 *     --server https://united-humanity.us \
 *     --seed-hex <64-hex 32-byte master seed>   (or --seed-file path) \
 *     --provider samples/provider-sample.json \
 *     --offerings samples/offerings-sample.json \
 *     [--dry-run]
 *
 * Re-import semantics (schemas/offering.toml [identity]): the same
 * offering_key updates the offering (new revision, newer updated_at);
 * re-running the import "touches" every row, which is how a provider says
 * "still true". Rows are NEVER duplicated by re-import.
 *
 * The offerings file is a JSON array of offering payload objects (field
 * names exactly as in schemas/offering.toml). CSV is deliberately not
 * parsed here yet: real catalogs disagree on columns, so convert to JSON
 * with your own mapping first; samples/offerings-sample.json shows the
 * shape for both a good and a service.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { join } from 'node:path';

const REPO = join(fileURLToPath(import.meta.url), '..', '..');

// ── args ────────────────────────────────────────────────────────────────────
const args = {};
for (let i = 2; i < process.argv.length; i++) {
  const a = process.argv[i];
  if (a === '--dry-run') args.dryRun = true;
  else if (a.startsWith('--')) args[a.slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase())] = process.argv[++i];
}
function die(msg) {
  console.error(`import-offerings: ${msg}`);
  process.exit(1);
}
if (!args.server) die('--server is required (e.g. https://united-humanity.us)');
if (!args.provider) die('--provider is required (a provider_v1 payload JSON file)');
if (!args.offerings) die('--offerings is required (a JSON array of offering payloads)');
let seedHex = args.seedHex;
if (!seedHex && args.seedFile) seedHex = readFileSync(args.seedFile, 'utf8').trim();
if (!seedHex || !/^[0-9a-f]{64}$/i.test(seedHex)) {
  die('--seed-hex (or --seed-file) must supply the 64-hex 32-byte master seed');
}
const SERVER = args.server.replace(/\/+$/, '');

// ── sample-data guard (2026-09-06) ──────────────────────────────────────────
// The bundled samples/*.json describe "Rivertown Bikes, 12 Main St", an
// invented storefront written to show the file shape. Someone ran this
// importer against the live server with them, so united-humanity.us has
// spent months telling visitors it has "2 offerings from 1 provider" that do
// not exist. Signed objects cannot be un-published by anyone but their
// author, so the cheap moment to catch this is here, before the POST.
// Local servers stay unguarded: that is what the samples are for.
{
  const usesSamples = [args.provider, args.offerings].some(
    (p) => p && /(^|[\\/])(scripts[\\/])?samples[\\/]/.test(p)
  );
  const isLocal = /^https?:\/\/(localhost|127\.0\.0\.1|\[::1\]|0\.0\.0\.0)(:|\/|$)/.test(SERVER);
  if (usesSamples && !isLocal && !args.allowSampleData && !args.dryRun) {
    die(
      `refusing to publish the bundled sample catalog to ${SERVER}.\n` +
        '  These files are invented demo data ("Rivertown Bikes"). Publishing them\n' +
        '  puts fake storefronts in a real directory, and a signed object can only\n' +
        '  be withdrawn by whoever signed it.\n' +
        '  Point --provider/--offerings at your real catalog, use --dry-run to\n' +
        '  rehearse, or pass --allow-sample-data if you genuinely mean it.'
    );
  }
}

// ── crypto + canonical encoding (the same modules the clients ship) ─────────
const noble = await import(pathToFileURL(join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js')).href);
const cbor = await import(pathToFileURL(join(REPO, 'web', 'shared', 'canonical-cbor.js')).href);
const pqObject = await import(pathToFileURL(join(REPO, 'web', 'shared', 'pq-object.js')).href);

const master = Uint8Array.from(seedHex.match(/.{2}/g).map((h) => parseInt(h, 16)));
// Identical to web/chat/pq.js + pq_crypto.rs: blake3 keyed-context derive.
const dilSeed = noble.blake3
  .create({ context: new TextEncoder().encode('hum/dilithium3/v1'), dkLen: 32 })
  .update(master)
  .digest();
const kp = noble.ml_dsa65.keygen(dilSeed);
const sign = async (msg) => noble.ml_dsa65.sign(msg, kp.secretKey);
const blake3 = (b) => noble.blake3(b, { dkLen: 32 });
console.log(`identity: ${Buffer.from(blake3(kp.publicKey)).toString('hex').slice(0, 16)}... (Dilithium3)`);

// ── JSON -> canonical CBOR payload ──────────────────────────────────────────
// Floats: CBOR major 7 / 64-bit. The payload is opaque bytes to the envelope
// (the signature covers the bytes as-is), so float canonicalization is a
// non-issue for identity; the relay's validator accepts float or integer.
function cborFloat64(n) {
  const buf = new ArrayBuffer(9);
  const dv = new DataView(buf);
  dv.setUint8(0, 0xfb);
  dv.setFloat64(1, n);
  return new Uint8Array(buf);
}
const CBOR_TRUE = new Uint8Array([0xf5]);
const CBOR_FALSE = new Uint8Array([0xf4]);

function encodeValue(v) {
  if (typeof v === 'string') return cbor.cborText(v);
  if (typeof v === 'boolean') return v ? CBOR_TRUE : CBOR_FALSE;
  if (typeof v === 'number') {
    return Number.isInteger(v) && v >= 0 ? cbor.cborUint(v) : cborFloat64(v);
  }
  if (Array.isArray(v)) return cbor.cborArray(v.map(encodeValue));
  if (v && typeof v === 'object') return encodeMap(v);
  throw new Error(`cannot encode value: ${JSON.stringify(v)}`);
}
function encodeMap(obj) {
  const pairs = [];
  for (const [k, v] of Object.entries(obj)) {
    if (v === null || v === undefined) continue; // absent, not null
    pairs.push([cbor.cborText(k), encodeValue(v)]);
  }
  return cbor.cborMap(pairs);
}

// ── submit ──────────────────────────────────────────────────────────────────
async function post(submission) {
  const res = await fetch(`${SERVER}/api/v2/objects`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(submission),
  });
  const body = await res.json().catch(() => ({}));
  return { ok: res.ok, status: res.status, body };
}

async function publish(objectType, payloadObj, references) {
  const payload = encodeMap(payloadObj);
  const { objectId, submission } = await pqObject.buildSignedObject({
    objectType,
    payload,
    authorPublicKey: kp.publicKey,
    sign,
    blake3,
    createdAt: Date.now(),
    references,
  });
  if (args.dryRun) {
    console.log(`[dry-run] ${objectType} ${objectId} (${payload.length} bytes)`);
    return { objectId, ok: true };
  }
  const res = await post(submission);
  if (!res.ok) {
    console.error(`  REJECTED ${objectType}: HTTP ${res.status} ${JSON.stringify(res.body)}`);
    return { objectId, ok: false };
  }
  return { objectId, ok: true };
}

// ── run ─────────────────────────────────────────────────────────────────────
const providerPayload = JSON.parse(readFileSync(args.provider, 'utf8'));
if (!providerPayload.updated_at) providerPayload.updated_at = Date.now();
const prov = await publish('provider_v1', providerPayload, []);
if (!prov.ok) die('provider publish failed; offerings not attempted');
console.log(`provider: ${providerPayload.display_name} -> ${prov.objectId}`);

const rows = JSON.parse(readFileSync(args.offerings, 'utf8'));
if (!Array.isArray(rows)) die('--offerings must be a JSON array');
let ok = 0;
let failed = 0;
for (const row of rows) {
  const payload = { ...row };
  payload.provider_ref = prov.objectId;
  if (!payload.updated_at) payload.updated_at = Date.now();
  const r = await publish('offering_v1', payload, [prov.objectId]);
  if (r.ok) {
    ok++;
    console.log(`  offering ${payload.offering_key}: ${r.objectId.slice(0, 16)}...`);
  } else {
    failed++;
    console.error(`  offering ${payload.offering_key || '(no key)'} FAILED`);
  }
}
console.log(`done: ${ok} published, ${failed} rejected${args.dryRun ? ' (dry run, nothing sent)' : ''}`);
// exitCode (not process.exit): lets node drain its handles cleanly on
// Windows instead of tripping a libuv teardown assert.
process.exitCode = failed > 0 ? 1 : 0;
