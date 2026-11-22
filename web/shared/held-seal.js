// held-seal.js: the website's twin of crates/humanity-protocol/src/held_seal.rs (2026-11-20,
// docs/design/hide-and-keep.md): the opening (rung 5), the sealing, the RE-WRAP a change of
// recipients needs (the owner's handover, a change of successor), a recipient's check of a new
// header, and the one-file export both apps write.
//
// A post an admin hid and kept is sealed on the relay to the server owner's key card, a part per
// file (part 0 the post's words and the list of its files). Only the owner's own device opens a
// part: the owner's Export opens every part straight into one .zip file the owner saves, and the
// owner's Restore opens each file and uploads it again. Nothing here ever draws a part on screen.
//
// Every byte layout is the Rust side's (format v4; the "Held copies" row of
// docs/reference/cryptography.md):
//   a part = "HUMHELD4" (8 bytes), how many recipients n (1 byte), the KEY COMMITMENT (32 bytes:
//   BLAKE3 derive_key("hum/held-commit/v4", part key)), then per recipient the ML-KEM-768
//   ciphertext (1088) and the part key wrapped for them (32 + 16-byte tag: AES-256-GCM under BLAKE3
//   derive_key("hum/held-wrap/v4", shared secret), a zero nonce, associated data
//   "hum/held-wrap/v4\n<package>\n<part>\n<n>\n<slot>\n<commitment hex>"); then pieces of at most
//   1 MiB of plaintext plus a 16-byte AES-256-GCM tag under BLAKE3 derive_key("hum/held/v4", part
//   key); piece i's nonce is four zero bytes then i (big-endian u64); its associated data is
//   "hum/held/v4\n<package>\n<part>\n<commitment hex>\n<i>\n<1 if last else 0>". The pieces bind the
//   commitment, not the slots, so a change of recipients RE-WRAPS the header and never touches a
//   body; each slot binds the count and its place, so a slot stripped, added or moved makes every
//   other slot refuse; an unwrapped key must match the commitment.
// scripts/tests/fixtures/held-v4.json (Rust sealed it) and held-v4-web.json (sealed and re-wrapped
// here) hold the two sides to each other: the Rust test `held_fixture_rust_and_the_website_agree`
// opens both, and scripts/tests/held-web.test.js opens both here. Change both or neither.
//
// The post-quantum and hash primitives are the vendored noble bundle (/shared/vendor/
// noble-pq.bundle.js), handed in as `m`, so this module runs the same in the browser and in Node;
// AES-256-GCM is WebCrypto (globalThis.crypto in both). No third-party import.

export const DOMAIN = 'hum/held/v4';
export const WRAP_DOMAIN = 'hum/held-wrap/v4';
export const COMMIT_DOMAIN = 'hum/held-commit/v4';
export const MAGIC = 'HUMHELD4';
export const PIECE = 1 << 20;
export const TAG = 16;
export const KYBER_CIPHERTEXT_LEN = 1088;
export const MAX_RECIPIENTS = 4;
export const COMMITMENT = 32;
/** One recipient's bytes in a part's header: the ML-KEM ciphertext and the wrapped part key. */
export const SLOT = KYBER_CIPHERTEXT_LEN + 32 + TAG;

/** The bytes before the first piece of a part sealed to `recipients` people: the header. */
export function headerLen(recipients) {
  return MAGIC.length + 1 + COMMITMENT + recipients * SLOT;
}

/** The sealed size of a part holding `plain` bytes sealed to `recipients` people. */
export function sealedLen(plain, recipients) {
  return headerLen(recipients) + plain + TAG * Math.max(1, Math.ceil(plain / PIECE));
}
export const MANIFEST_VERSION = 1;

const enc = new TextEncoder();
const dec = new TextDecoder();

function hexOf(bytes) {
  let hex = '';
  for (let i = 0; i < bytes.length; i++) hex += bytes[i].toString(16).padStart(2, '0');
  return hex;
}

/** What a part is bound to: its package and its number in it. */
export function partContext(pkg, part) {
  return `${pkg}\n${part}`;
}

/** The associated data of piece `index` of the part `context` whose key commitment is `commitment` (bytes). */
export function pieceAad(context, commitment, index, last) {
  return `${DOMAIN}\n${context}\n${hexOf(commitment)}\n${index}\n${last ? 1 : 0}`;
}

/** The associated data of recipient `slot`'s wrapped part key in the part `context` sealed to `recipients` people under `commitment`. */
export function wrapAad(context, recipients, slot, commitment) {
  return `${WRAP_DOMAIN}\n${context}\n${recipients}\n${slot}\n${hexOf(commitment)}`;
}

function nonceOf(index) {
  const n = new Uint8Array(12);
  let v = BigInt(index);
  for (let i = 11; i >= 4; i--) { n[i] = Number(v & 0xffn); v >>= 8n; }
  return n;
}

function deriveKey(m, domain, material) {
  return m.blake3.create({ context: enc.encode(domain), dkLen: 32 }).update(material).digest();
}

/** The commitment to a part key: BLAKE3 derive_key("hum/held-commit/v4", part key). */
export function keyCommitment(m, partKey) {
  return deriveKey(m, COMMIT_DOMAIN, partKey);
}

function concat(parts, total) {
  const out = new Uint8Array(total);
  let o = 0;
  for (const p of parts) { out.set(p, o); o += p.length; }
  return out;
}

function sameBytes(a, b) {
  if (a.length !== b.length) return false;
  let d = 0;
  for (let i = 0; i < a.length; i++) d |= a[i] ^ b[i];
  return d === 0;
}

/** Read the header at the start of `bytes`: { recipients, commitment, header } (header = its exact
 *  bytes). Throws when it does not start as a sealed part does or is cut short. */
export function parseHeader(bytes) {
  const b = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  const n = b[MAGIC.length];
  if (b.length < MAGIC.length + 1 || dec.decode(b.subarray(0, MAGIC.length)) !== MAGIC || !n || n > MAX_RECIPIENTS || b.length < headerLen(n)) {
    throw new Error('this is not a sealed held part');
  }
  const at = MAGIC.length + 1;
  return { recipients: n, commitment: b.slice(at, at + COMMITMENT), header: b.subarray(0, headerLen(n)) };
}

/** BLAKE3 (hex) of a header's bytes: what an acceptance commits to, per part. */
export function headerHash(m, header) {
  return hexOf(m.blake3(header));
}

/** The key slot `slot` of the parsed header `h` wraps, unwrapped with `kyberSecret` BEFORE the
 *  commitment is compared; null when the slot is not this key's (or moved, or the count changed). */
async function unwrapSlot(m, kyberSecret, context, h, slot) {
  const at = MAGIC.length + 1 + COMMITMENT + slot * SLOT;
  try {
    // ML-KEM answers every ciphertext (implicit rejection): a slot that is not this key's shows
    // itself only when its wrapped part key does not open.
    const ss = m.ml_kem768.decapsulate(h.header.subarray(at, at + KYBER_CIPHERTEXT_LEN), kyberSecret);
    const wrap = await globalThis.crypto.subtle.importKey('raw', deriveKey(m, WRAP_DOMAIN, ss), { name: 'AES-GCM' }, false, ['decrypt']);
    const key = new Uint8Array(await globalThis.crypto.subtle.decrypt(
      { name: 'AES-GCM', iv: new Uint8Array(12), additionalData: enc.encode(wrapAad(context, h.recipients, slot, h.commitment)), tagLength: 128 },
      wrap,
      h.header.subarray(at + KYBER_CIPHERTEXT_LEN, at + SLOT),
    ));
    return key.length === 32 ? key : null;
  } catch (_) {
    return null;
  }
}

const UNCOMMITTED = 'the key in this part\'s header is not the key its body is sealed under';

/** Unwrap the part key from whichever slot of `header` is this key's and check it against the
 *  header's commitment (the Rust `open_slot`). Throws for anything not sealed to this key and for a
 *  slot wrapping any other key than the one the body is sealed under. */
export async function openSlot(m, kyberSecret, context, header) {
  const h = parseHeader(header);
  let key = null;
  for (let slot = 0; slot < h.recipients && !key; slot++) key = await unwrapSlot(m, kyberSecret, context, h, slot);
  if (!key) throw new Error('this part is not sealed to this key');
  if (!sameBytes(keyCommitment(m, key), h.commitment)) throw new Error(UNCOMMITTED);
  return key;
}

/** The header of the part `context` whose part key is `partKey` (with commitment `commitment`),
 *  wrapped for the Kyber768 public keys `recipients` in slot order. */
async function wrapFor(m, partKey, commitment, recipients, context) {
  if (!recipients.length || recipients.length > MAX_RECIPIENTS) throw new Error('a part is sealed to 1 to 4 people');
  const out = [enc.encode(MAGIC), Uint8Array.of(recipients.length), commitment];
  for (let slot = 0; slot < recipients.length; slot++) {
    const { cipherText, sharedSecret } = m.ml_kem768.encapsulate(recipients[slot]);
    const wrap = await globalThis.crypto.subtle.importKey('raw', deriveKey(m, WRAP_DOMAIN, sharedSecret), { name: 'AES-GCM' }, false, ['encrypt']);
    out.push(cipherText, new Uint8Array(await globalThis.crypto.subtle.encrypt(
      { name: 'AES-GCM', iv: new Uint8Array(12), additionalData: enc.encode(wrapAad(context, recipients.length, slot, commitment)), tagLength: 128 },
      wrap,
      partKey,
    )));
  }
  return concat(out, out.reduce((n, p) => n + p.length, 0));
}

/**
 * The owner's device, in a change of recipients (the Rust `rewrap_header`): open this key's slot of
 * `header` (part `context`) and wrap the same part key, under the same commitment, for the Kyber768
 * public keys `recipients` in slot order. The new header; the body it heads is unchanged.
 */
export async function rewrapHeader(m, kyberSecret, context, header, recipients) {
  const h = parseHeader(header);
  const key = await openSlot(m, kyberSecret, context, header);
  try { return await wrapFor(m, key, h.commitment, recipients, context); }
  finally { key.fill(0); }
}

/**
 * A recipient's device, in a change of recipients (the Rust `check_header`): check the new `header`
 * of part `context` (exactly a header), sealed to `count` people with this key in slot `slot`: it
 * names that many people, this key's slot opens, and the key it wraps is the one the header commits
 * to. No body is read. Resolves to the header's hash; throws with why, in the Rust side's words.
 */
export async function checkHeader(m, kyberSecret, context, header, count, slot) {
  const h = parseHeader(header);
  const bytes = header instanceof Uint8Array ? header : new Uint8Array(header);
  if (bytes.length !== headerLen(h.recipients)) throw new Error('this is not a part\'s header alone');
  if (h.recipients !== count) throw new Error(`it is sealed to ${h.recipients} people, but the change names ${count}`);
  const key = await unwrapSlot(m, kyberSecret, context, h, slot);
  if (!key) throw new Error('your place in it does not open for your key');
  const ok = sameBytes(keyCommitment(m, key), h.commitment);
  key.fill(0);
  if (!ok) throw new Error(UNCOMMITTED);
  return headerHash(m, bytes);
}

/**
 * Open the part `context` sealed to the Kyber768 secret key `kyberSecret`: its plaintext bytes.
 * Throws for anything not sealed to that key, sealed as another part, altered, reordered or cut
 * short, exactly where the Rust `open_stream` refuses.
 */
export async function openPart(m, kyberSecret, context, sealed) {
  const bytes = sealed instanceof Uint8Array ? sealed : new Uint8Array(sealed);
  const h = parseHeader(bytes);
  const partKey = await openSlot(m, kyberSecret, context, bytes);
  const key = await globalThis.crypto.subtle.importKey('raw', deriveKey(m, DOMAIN, partKey), { name: 'AES-GCM' }, false, ['decrypt']);
  partKey.fill(0);
  const body = bytes.subarray(headerLen(h.recipients));
  const out = [];
  let total = 0;
  let index = 0;
  let off = 0;
  for (;;) {
    const len = Math.min(PIECE + TAG, body.length - off);
    if (len < TAG) throw new Error('this part is cut short');
    const last = off + len === body.length;
    let plain;
    try {
      plain = new Uint8Array(await globalThis.crypto.subtle.decrypt(
        { name: 'AES-GCM', iv: nonceOf(index), additionalData: enc.encode(pieceAad(context, h.commitment, index, last)), tagLength: 128 },
        key,
        body.subarray(off, off + len),
      ));
    } catch (_) {
      throw new Error('this part does not open with this key (not sealed to it, sealed as another part, altered or cut short)');
    }
    out.push(plain);
    total += plain.length;
    if (last) break;
    off += len;
    index += 1;
  }
  return concat(out, total);
}

/**
 * Seal `plain` as the part `context` to the Kyber768 public keys `recipients` (1 to 4), exactly as
 * the Rust `seal_stream` does: a fresh random part key, committed to and wrapped for each, the body
 * in 1 MiB pieces. (A change of recipients never re-seals: it re-wraps, `rewrapHeader`.)
 */
export async function sealPart(m, recipients, context, plain) {
  const bytes = plain instanceof Uint8Array ? plain : new Uint8Array(plain);
  const partKey = globalThis.crypto.getRandomValues(new Uint8Array(32));
  const commitment = keyCommitment(m, partKey);
  const out = [await wrapFor(m, partKey, commitment, recipients, context)];
  const key = await globalThis.crypto.subtle.importKey('raw', deriveKey(m, DOMAIN, partKey), { name: 'AES-GCM' }, false, ['encrypt']);
  partKey.fill(0);
  const pieces = Math.max(1, Math.ceil(bytes.length / PIECE));
  for (let i = 0; i < pieces; i++) {
    out.push(new Uint8Array(await globalThis.crypto.subtle.encrypt(
      { name: 'AES-GCM', iv: nonceOf(i), additionalData: enc.encode(pieceAad(context, commitment, i, i === pieces - 1)), tagLength: 128 },
      key,
      bytes.subarray(i * PIECE, (i + 1) * PIECE),
    )));
  }
  return concat(out, out.reduce((n, p) => n + p.length, 0));
}

/** Open part 0 of `pkg`, the post's words and its list of files (the Rust `Manifest`). */
/** Whether the opened `manifest` is the held item the owner CHOSE (core held_seal `manifest_is`):
 *  its package and its report (the third review's W7), and its post's author and time (the fourth
 *  review's finding 8: one report can hold several posts, each its own item), as the list showed
 *  them in `item` ({ package, report_id, author, stamp }). */
export function manifestIs(manifest, item) {
  return !!manifest && !!item && String(manifest.package) === String(item.package) && Number(manifest.report_id) === Number(item.report_id)
    && String(manifest.author) === String(item.author) && Number(manifest.stamp) === Number(item.stamp);
}

export async function openManifest(m, kyberSecret, pkg, sealed) {
  const manifest = JSON.parse(dec.decode(await openPart(m, kyberSecret, partContext(pkg, 0), sealed)));
  if (!manifest || manifest.v !== MANIFEST_VERSION || manifest.package !== pkg) throw new Error('this list of files is not this package\'s');
  return manifest;
}

// ── Who asks for a held part (2026-11-20, the owner review's finding 1; the domain line, R6) ──
//
// A grant (a signed action's object id) sits in the held items log and a part is sealed to the
// owner and the successor, so a grant alone never hands out a part: every request to a held part
// route carries its caller's key, the time, and a Dilithium3 signature over `purpose\ntime`, and
// the relay serves the part only to the key the grant belongs to. The purpose starts with its own
// domain line.
// NOT YET CLOSED, said plainly (the owner branch's third review, S6, 2026-11-21): the domain line alone
// does not keep a signed chat line out today, because a chat message's content may hold a
// newline, so a chat line whose content is this very purpose is signed over the same
// `purpose\ntime` and broadcast to everyone, who could replay it within the five minutes. What
// closes it is the relay refusing every chat line that begins with a `hum/` domain line, which
// arrives with the signed-requests branch (src/relay/signed_request.rs, round 2); once both
// branches are merged, the held caller moves onto that request scheme. NOTE to whoever merges the
// second of the two: make that move then (it is not done here).
// The twin of core held_seal's
// CALLER_KEY, CALLER_TIME, CALLER_SIG, CALLER_DOMAIN and caller_purpose; web/chat/chat-held.js
// `callerHeaders` signs. scripts/tests/fixtures/held-owner-v2.json holds both sides to the same
// purposes.

/** The header carrying the caller's Dilithium3 public key (hex). */
export const CALLER_KEY = 'x-hos-held-key';
/** The header carrying the time the caller signed (Unix ms). */
export const CALLER_TIME = 'x-hos-held-time';
/** The header carrying the caller's signature (hex) over `purpose\ntime`. */
export const CALLER_SIG = 'x-hos-held-sig';
/** The first line of every held caller's purpose. */
export const CALLER_DOMAIN = 'hum/held-caller/v1';

/** What a held part route's caller signs: the domain line, then the route (`part` GET, `restore`
 *  PUT, `finish` POST with part and offset 0, `header` GET and `rewrap` PUT for the owner in a
 *  change, `rewrapped` GET for a recipient checking it), the grant, the item, the part and the
 *  offset, so one signature is good for exactly one request. */
export function callerPurpose(route, grant, id, part, offset) {
  return `${CALLER_DOMAIN}\nheld_${route}:${grant}:${id}:${part}:${offset}`;
}

// ── A change of recipients: the owner re-wraps, each recipient checks its own slot ──
//
// The twins of core held_seal's rewrap_package, check_rewrapped_package and acceptance_digest;
// scripts/tests/fixtures/held-owner-v2.json holds both sides to the same header hashes and digest.

/** Re-wrap every part of `pkg` (`parts` parts) for `recipients` from the owner's key: `fetchHeader(part)`
 *  reads a part's current header, `uploadHeader(part, header)` sends the new one. No body. */
export async function rewrapPackage(m, kyberSecret, recipients, pkg, parts, fetchHeader, uploadHeader) {
  for (let part = 0; part < parts; part++) {
    let header;
    try { header = await rewrapHeader(m, kyberSecret, partContext(pkg, part), await fetchHeader(part), recipients); }
    catch (e) { throw new Error(`part ${part}: ${e.message}`); }
    await uploadHeader(part, header);
  }
}

/** On a recipient's device: check every new header of `pkg` (`parts` parts, each read by
 *  `fetchHeader(part)`), sealed to `count` people with this key in slot `slot`. Resolves to each
 *  header's hash in part order; throws with why, naming the part. */
export async function checkRewrappedPackage(m, kyberSecret, pkg, parts, count, slot, fetchHeader) {
  const hashes = [];
  for (let part = 0; part < parts; part++) {
    const header = await fetchHeader(part);
    try { hashes.push(await checkHeader(m, kyberSecret, partContext(pkg, part), header, count, slot)); }
    catch (e) { throw new Error(`part ${part}: ${e.message}`); }
  }
  return hashes;
}

/** The first line of the text an acceptance's digest is taken over. */
export const ACCEPT_DOMAIN = 'hum/held-accept/v4';

/** What a recipient's acceptance of a change commits to: BLAKE3 (hex) of the domain line, the
 *  change's round, then for each package in order of its id a line with the package id and a line
 *  `<part> <header hash>` per part. `items` is [[package id, [hash of header 0, ...]], ...]. */
export function acceptanceDigest(m, round, items) {
  const sorted = items.slice().sort((a, b) => (a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0));
  let text = `${ACCEPT_DOMAIN}\n${round}\n`;
  for (const [pkg, hashes] of sorted) {
    text += pkg + '\n';
    hashes.forEach((h, i) => { text += `${i} ${h}\n`; });
  }
  return hexOf(m.blake3(enc.encode(text)));
}

// ── The one-file export (a .zip with every entry stored, not compressed) ──
//
// Both apps write the same layout (src/net/held_export.rs is the desktop's): `held-item.json`,
// the list of files and who and when, never the restore key; `post.txt`, the post's words; and
// `files/<part>-<name>`, each file as it was. Stored entries need no compression library, and
// every system opens them.

const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();

/** CRC-32 (the zip format's, polynomial 0xEDB88320). */
export function crc32(bytes) {
  let c = 0xffffffff;
  for (let i = 0; i < bytes.length; i++) c = CRC_TABLE[(c ^ bytes[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

/** A name safe inside the zip: no folders, no characters a system refuses, never empty. */
export function safeEntryName(name) {
  const last = String(name || '').split(/[\\/]/).pop() || '';
  const cleaned = last.replace(/[\u0000-\u001f<>:"|?*]/g, '_').replace(/^\.+/, '_').slice(0, 120);
  return cleaned || 'file';
}

/** The entries of the export of `manifest` (opened) with `files` (part number -> bytes). */
export function exportEntries(manifest, files, exportedAt) {
  const about = { ...manifest, exported_at: exportedAt };
  delete about.restore_key;
  delete about.words;
  const entries = [
    { name: 'held-item.json', data: enc.encode(JSON.stringify(about, null, 2)) },
    { name: 'post.txt', data: enc.encode(String(manifest.words || '')) },
  ];
  for (const f of manifest.files || []) {
    const name = safeEntryName(f.original_name || f.name);
    if (files.has(f.part)) entries.push({ name: `files/${f.part}-${name}`, data: files.get(f.part) });
    if (f.poster_part != null && files.has(f.poster_part)) entries.push({ name: `files/${f.poster_part}-poster-${name}.jpg`, data: files.get(f.poster_part) });
  }
  return entries;
}

/** A .zip of `entries` ({name, data: Uint8Array}), every entry stored. */
export function zipStored(entries) {
  const parts = [];
  const central = [];
  let offset = 0;
  const u16 = (v) => [v & 0xff, (v >>> 8) & 0xff];
  const u32 = (v) => [v & 0xff, (v >>> 8) & 0xff, (v >>> 16) & 0xff, (v >>> 24) & 0xff];
  for (const e of entries) {
    const name = enc.encode(e.name);
    const crc = crc32(e.data);
    const size = e.data.length;
    if (size > 0xfffffffe) throw new Error('a file larger than 4 GB cannot go into this export');
    // Version 20, flag bit 11 (the name is UTF-8), stored, no time.
    const common = [...u16(20), ...u16(0x0800), ...u16(0), ...u16(0), ...u16(0x21), ...u32(crc), ...u32(size), ...u32(size), ...u16(name.length), ...u16(0)];
    const local = new Uint8Array([...u32(0x04034b50), ...common]);
    parts.push(local, name, e.data);
    central.push(new Uint8Array([...u32(0x02014b50), ...u16(20), ...common, ...u16(0), ...u16(0), ...u16(0), ...u32(0), ...u32(offset)]), name);
    offset += local.length + name.length + size;
  }
  const centralSize = central.reduce((n, p) => n + p.length, 0);
  const end = new Uint8Array([...u32(0x06054b50), ...u16(0), ...u16(0), ...u16(entries.length), ...u16(entries.length), ...u32(centralSize), ...u32(offset), ...u16(0)]);
  const all = [...parts, ...central, end];
  return concat(all, all.reduce((n, p) => n + p.length, 0));
}
