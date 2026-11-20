// held-seal.js: the website's twin of crates/humanity-protocol/src/held_seal.rs, the OPENING half
// only (2026-11-20, docs/design/hide-and-keep.md rung 5), plus the one-file export both apps write.
//
// A post an admin hid and kept is sealed on the relay to the server owner's key card, a part per
// file (part 0 the post's words and the list of its files). Only the owner's own device opens a
// part: the owner's Export opens every part straight into one .zip file the owner saves, and the
// owner's Restore opens each file and uploads it again. Nothing here ever draws a part on screen.
//
// Every byte layout is the Rust side's (the "Held copies" row of docs/reference/cryptography.md):
//   a part = "HUMHELD1" (8 bytes), the ML-KEM-768 ciphertext (1088), then pieces of at most
//   1 MiB of plaintext plus a 16-byte AES-256-GCM tag; the key is BLAKE3 derive_key("hum/held/v1",
//   shared secret); piece i's nonce is four zero bytes then i (big-endian u64); its associated data
//   is "hum/held/v1\n<package>\n<part>\n<i>\n<1 if last else 0>".
// scripts/tests/fixtures/held-v1.json holds the two sides to each other: Rust sealed it, the
// Rust test `held_fixture_rust_and_the_website_agree` opens it, and scripts/tests/held-web.test.js
// opens it here. Change both or neither.
//
// The post-quantum and hash primitives are the vendored noble bundle (/shared/vendor/
// noble-pq.bundle.js), handed in as `m`, so this module runs the same in the browser and in Node;
// AES-256-GCM is WebCrypto (globalThis.crypto in both). No third-party import.

export const DOMAIN = 'hum/held/v1';
export const MAGIC = 'HUMHELD1';
export const PIECE = 1 << 20;
export const TAG = 16;
export const KYBER_CIPHERTEXT_LEN = 1088;
export const HEADER = MAGIC.length + KYBER_CIPHERTEXT_LEN;
export const MANIFEST_VERSION = 1;

const enc = new TextEncoder();
const dec = new TextDecoder();

/** What a part is bound to: its package and its number in it. */
export function partContext(pkg, part) {
  return `${pkg}\n${part}`;
}

/** The associated data of piece `index` of the part `context`. */
export function pieceAad(context, index, last) {
  return `${DOMAIN}\n${context}\n${index}\n${last ? 1 : 0}`;
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

function concat(parts, total) {
  const out = new Uint8Array(total);
  let o = 0;
  for (const p of parts) { out.set(p, o); o += p.length; }
  return out;
}

/**
 * Open the part `context` sealed to the Kyber768 secret key `kyberSecret`: its plaintext bytes.
 * Throws for anything not sealed to that key, sealed as another part, altered, reordered or cut
 * short, exactly where the Rust `open_stream` refuses.
 */
export async function openPart(m, kyberSecret, context, sealed) {
  const bytes = sealed instanceof Uint8Array ? sealed : new Uint8Array(sealed);
  if (bytes.length < HEADER || dec.decode(bytes.subarray(0, MAGIC.length)) !== MAGIC) {
    throw new Error('this is not a sealed held part');
  }
  let ss;
  try {
    ss = m.ml_kem768.decapsulate(bytes.subarray(MAGIC.length, HEADER), kyberSecret);
  } catch (_) {
    throw new Error('this part is not sealed to this key');
  }
  const key = await globalThis.crypto.subtle.importKey('raw', deriveKey(m, DOMAIN, ss), { name: 'AES-GCM' }, false, ['decrypt']);
  const body = bytes.subarray(HEADER);
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
        { name: 'AES-GCM', iv: nonceOf(index), additionalData: enc.encode(pieceAad(context, index, last)), tagLength: 128 },
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

/** Open part 0 of `pkg`, the post's words and its list of files (the Rust `Manifest`). */
export async function openManifest(m, kyberSecret, pkg, sealed) {
  const manifest = JSON.parse(dec.decode(await openPart(m, kyberSecret, partContext(pkg, 0), sealed)));
  if (!manifest || manifest.v !== MANIFEST_VERSION || manifest.package !== pkg) throw new Error('this list of files is not this package\'s');
  return manifest;
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
