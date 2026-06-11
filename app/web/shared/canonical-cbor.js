// canonical-cbor.js — byte-exact canonical CBOR encoder (no dependencies).
//
// Must reproduce, byte-for-byte, the Rust encoder in
// src/relay/core/encoding.rs (rules from design/conformance/canonical_cbor_rules.md):
//   - definite-length only
//   - shortest-form integer / length headers
//   - map keys sorted: SHORTER encoded key first, then bytewise-lexicographic
//     (RFC 7049 §3.9 "Canonical CBOR" length-first ordering — NOT the newer
//      bytewise-only deterministic ordering; do not swap libraries that use it)
//   - no floats, no tags, UTF-8 text only
//
// Locked to the Rust impl by the group-object KAT (scripts/group-object-kat.mjs
// ↔ src/relay/core/object.rs::group_object_canonical_kat). If you change the
// encoding, both MUST change or the KAT fails (a web-built signed object would
// then be unverifiable by the relay — DO NOT SHIP).
//
// This is the foundation for ALL web-built signed objects (groups, votes,
// vouches, recovery shares …), not just P2P groups.

const _enc = new TextEncoder();

function _concat(chunks) {
  let len = 0;
  for (const c of chunks) len += c.length;
  const out = new Uint8Array(len);
  let off = 0;
  for (const c of chunks) { out.set(c, off); off += c.length; }
  return out;
}

// Write a CBOR head: major type (0..7) in the top 3 bits + a shortest-form
// argument. Returns the head bytes (1, 2, 3, 5, or 9 bytes).
function _head(major, n) {
  const mt = major << 5;
  if (n < 24) return new Uint8Array([mt | n]);
  if (n < 0x100) return new Uint8Array([mt | 24, n]);
  if (n < 0x10000) return new Uint8Array([mt | 25, (n >> 8) & 0xff, n & 0xff]);
  if (n < 0x100000000) {
    return new Uint8Array([mt | 26, (n >>> 24) & 0xff, (n >>> 16) & 0xff, (n >>> 8) & 0xff, n & 0xff]);
  }
  // 64-bit: use BigInt to avoid precision loss above 2^53.
  const big = BigInt(n);
  const b = new Uint8Array(8);
  for (let i = 7; i >= 0; i--) { b[i] = Number((big >> BigInt((7 - i) * 8)) & 0xffn); }
  return _concat([new Uint8Array([mt | 27]), b]);
}

/** Unsigned integer (major type 0). */
export function cborUint(n) {
  if (typeof n === 'bigint') {
    if (n < 0n) throw new Error('canonical-cbor: negative integers not supported');
    return _headBig(0, n);
  }
  if (!Number.isInteger(n) || n < 0) throw new Error('canonical-cbor: uint must be a non-negative integer');
  return _head(0, n);
}

function _headBig(major, n) {
  const mt = major << 5;
  if (n < 24n) return new Uint8Array([mt | Number(n)]);
  if (n < 0x100n) return new Uint8Array([mt | 24, Number(n)]);
  if (n < 0x10000n) return new Uint8Array([mt | 25, Number((n >> 8n) & 0xffn), Number(n & 0xffn)]);
  if (n < 0x100000000n) {
    return new Uint8Array([mt | 26, Number((n >> 24n) & 0xffn), Number((n >> 16n) & 0xffn), Number((n >> 8n) & 0xffn), Number(n & 0xffn)]);
  }
  const b = new Uint8Array(8);
  for (let i = 7; i >= 0; i--) b[i] = Number((n >> BigInt((7 - i) * 8)) & 0xffn);
  return _concat([new Uint8Array([mt | 27]), b]);
}

/** Text string (major type 3), UTF-8. */
export function cborText(s) {
  const bytes = _enc.encode(s);
  return _concat([_head(3, bytes.length), bytes]);
}

/** Byte string (major type 2). Accepts Uint8Array. */
export function cborBytes(u8) {
  const bytes = u8 instanceof Uint8Array ? u8 : new Uint8Array(u8);
  return _concat([_head(2, bytes.length), bytes]);
}

/** Array (major type 4) of already-encoded item byte-strings. */
export function cborArray(encodedItems) {
  return _concat([_head(4, encodedItems.length), ..._concat0(encodedItems)]);
}

// concat helper that returns an array of the single concatenated chunk (so the
// spread in cborArray/cborMap stays simple).
function _concat0(chunks) { return [_concat(chunks)]; }

/**
 * Map (major type 5) from an array of [encodedKey, encodedValue] pairs.
 * Sorts pairs by canonical key ordering before writing.
 */
export function cborMap(pairs) {
  const sorted = pairs.slice().sort((a, b) => {
    const ka = a[0], kb = b[0];
    if (ka.length !== kb.length) return ka.length - kb.length; // shorter first
    for (let i = 0; i < ka.length; i++) {
      if (ka[i] !== kb[i]) return ka[i] - kb[i];               // then bytewise
    }
    return 0;
  });
  const body = [];
  for (const [k, v] of sorted) { body.push(k, v); }
  return _concat([_head(5, sorted.length), ..._concat0(body)]);
}

/**
 * Encode a Humanity Network Object to canonical CBOR bytes — mirrors
 * src/relay/core/object.rs::Object::to_canonical_bytes.
 *
 * fields:
 *   protocol_version       (number, required)
 *   object_type            (string, required)
 *   space_id               (string | null)
 *   channel_id             (string | null)
 *   author_public_key      (Uint8Array, required)
 *   created_at             (number | null)
 *   references             (string[] of object_id hex; [] if none)
 *   payload_schema_version (number, required)
 *   payload_encoding       (string, required)
 *   payload                (Uint8Array, required)
 *   signature              (Uint8Array, required — zero-filled for signable bytes)
 */
export function encodeObjectCanonical(f) {
  const pairs = [];
  pairs.push([cborText('author_public_key'), cborBytes(f.author_public_key)]);
  if (f.channel_id != null) pairs.push([cborText('channel_id'), cborText(f.channel_id)]);
  if (f.created_at != null) pairs.push([cborText('created_at'), cborUint(f.created_at)]);
  pairs.push([cborText('object_type'), cborText(f.object_type)]);
  pairs.push([cborText('payload'), cborBytes(f.payload)]);
  pairs.push([cborText('payload_encoding'), cborText(f.payload_encoding)]);
  pairs.push([cborText('payload_schema_version'), cborUint(f.payload_schema_version)]);
  pairs.push([cborText('protocol_version'), cborUint(f.protocol_version)]);
  pairs.push([cborText('references'), cborArray((f.references || []).map(cborText))]);
  pairs.push([cborText('signature'), cborBytes(f.signature)]);
  if (f.space_id != null) pairs.push([cborText('space_id'), cborText(f.space_id)]);
  return cborMap(pairs);
}

/* ── Decoder ───────────────────────────────────────────────────────────────
 * Parse canonical-CBOR bytes (as produced by the encoder above, or by Rust's
 * ciborium) back into a JS value:
 *   - unsigned integers       → number (or BigInt for values > 2^53-1)
 *   - byte strings (major 2)  → Uint8Array
 *   - text strings (major 3)  → string
 *   - arrays (major 4)        → Array
 *   - maps (major 5)          → plain object (text-string keys only — which is
 *                                what every Object payload + the group schemas
 *                                use; reject other key types loudly)
 * Indefinite-length, tags, negative ints, and floats are NOT supported (this
 * matches the canonical encoder's "no floats/no tags/definite-length" rules).
 * Used to read group_epoch_key_v1 / group_msg_v1 payloads off the relay's
 * SignedObjectResponse JSON.
 */

const _dec = new TextDecoder();

function _readHead(bytes, off) {
  const initial = bytes[off];
  const major = initial >> 5;
  const minor = initial & 0x1f;
  let arg, hLen;
  if (minor < 24) { arg = minor; hLen = 1; }
  else if (minor === 24) { arg = bytes[off + 1]; hLen = 2; }
  else if (minor === 25) { arg = (bytes[off + 1] << 8) | bytes[off + 2]; hLen = 3; }
  else if (minor === 26) {
    // u32 — use multiplication for the top byte so >2^31 doesn't go negative.
    arg = bytes[off + 1] * 0x1000000
        + ((bytes[off + 2] << 16) | (bytes[off + 3] << 8) | bytes[off + 4]);
    hLen = 5;
  } else if (minor === 27) {
    // u64 — use BigInt; collapse to Number if it fits losslessly.
    let big = 0n;
    for (let i = 0; i < 8; i++) big = (big << 8n) | BigInt(bytes[off + 1 + i]);
    arg = big <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(big) : big;
    hLen = 9;
  } else {
    throw new Error('canonical-cbor: indefinite-length not supported');
  }
  return [major, arg, hLen];
}

function _decodeAt(bytes, off) {
  const [major, arg, hLen] = _readHead(bytes, off);
  let pos = off + hLen;
  switch (major) {
    case 0: return [arg, pos];                              // unsigned int
    case 2: {                                               // byte string
      const len = Number(arg);
      const val = bytes.subarray(pos, pos + len);
      return [new Uint8Array(val), pos + len];
    }
    case 3: {                                               // text string
      const len = Number(arg);
      const val = _dec.decode(bytes.subarray(pos, pos + len));
      return [val, pos + len];
    }
    case 4: {                                               // array
      const out = [];
      const len = Number(arg);
      for (let i = 0; i < len; i++) {
        const [item, np] = _decodeAt(bytes, pos);
        out.push(item); pos = np;
      }
      return [out, pos];
    }
    case 5: {                                               // map (text keys)
      const obj = {};
      const len = Number(arg);
      for (let i = 0; i < len; i++) {
        const [k, p1] = _decodeAt(bytes, pos);
        const [v, p2] = _decodeAt(bytes, p1);
        if (typeof k !== 'string') {
          throw new Error('canonical-cbor: non-text map key not supported');
        }
        obj[k] = v; pos = p2;
      }
      return [obj, pos];
    }
    default:
      throw new Error(`canonical-cbor: unsupported major type ${major}`);
  }
}

/** Decode canonical CBOR bytes into a JS value. */
export function decodeCanonicalCbor(bytes) {
  const u8 = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  const [val] = _decodeAt(u8, 0);
  return val;
}

export const _internal = { _head, _headBig, _concat };
