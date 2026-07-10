#!/usr/bin/env node
/**
 * build-stars-bin.js
 *
 * Converts data/stars.csv (HYG star catalog, ~34 MB, ~120k rows) into the
 * compact binary catalog data/stars.bin that the native renderer parses at
 * startup (src/renderer/stars.rs::StarCatalog). The CSV stays in the repo as
 * the human-readable source of truth; the .bin is the shipped runtime format
 * (the release bundle tars the whole data/ dir, so committing the .bin is all
 * it takes to distribute it). Re-run this script whenever stars.csv changes.
 *
 * Usage:
 *   node scripts/build-stars-bin.js        (or: just build-stars)
 *
 * ── Format "HOSSTAR1" (all integers/floats little-endian) ────────────────
 *
 *   Header (16 bytes):
 *     0..8   magic  b"HOSSTAR1"
 *     8..12  u32    star_count   (main records)
 *     12..16 u32    named_count  (sidecar entries)
 *
 *   Main records: star_count x 15 bytes each:
 *     0..12  f32 x3 unit direction on the celestial sphere (equatorial
 *            J2000, same frame as the HYG x,y,z columns). Stored as a unit
 *            vector rather than ra/dec because that is EXACTLY what the
 *            renderer consumes (StarVertex.direction) -- storing ra/dec
 *            would force a trig conversion per star at load and introduce a
 *            second coordinate convention to get wrong. Computed as
 *            f64-normalize(x,y,z) then cast to f32, which is bit-identical
 *            to what the old CSV loader produced (IEEE-754 f64 arithmetic
 *            and f64->f32 rounding are deterministic across JS and Rust).
 *     12..14 u16    apparent magnitude, quantized: q = round((mag+2.0)*1024)
 *            clamped to [0,65535]. Covers mag -2.0 .. +62.0 (Sirius, the
 *            brightest star, is -1.44; the Sun sits at the origin and is
 *            dropped by the position filter). Max quantization error is
 *            0.5/1024 = 0.00049 mag, i.e. a brightness error under 0.05%
 *            (brightness = 10^((6.5-mag)/2.5)) -- far below anything a
 *            display can show.
 *     14..15 u8     B-V color index, quantized over [-0.4, 2.0] (exactly the
 *            domain ci_to_rgb() clamps to): q = round((clamp(ci)+0.4)/2.4*255).
 *            Max error 2.4/255/2 = 0.0047 in ci; ci_to_rgb's channel slopes
 *            are <= 1.0 per unit ci, so the color error is < 0.5% of one
 *            RGB channel -- invisible.
 *
 *     15 bytes/star. ALL stars with |position| >= 0.001 pc are included
 *     (only Sol at the origin is dropped); the naked-eye brightness cutoff
 *     stays IN CODE (StarCatalog::skybox_vertices) so rendering policy can
 *     change without regenerating this file.
 *
 *   Named-star sidecar: named_count variable-length entries, emitted in CSV
 *   row order (order matters: the loader replays the same first-wins /
 *   last-wins map semantics the CSV parser had). Only the ~5k stars with a
 *   proper name or a Bayer designation appear here -- the constellation
 *   resolver needs name->direction, and putting names in the main record
 *   would bloat all 120k rows for the sake of a few thousand:
 *     0      u8     kind: 0 = proper-name key ("vega"), 1 = bayer key
 *                   ("alp lyr" = bayer + " " + con, both lowercased)
 *     1      u8     key length in bytes
 *     2..    key    UTF-8, pre-ASCII-lowercased (matches the resolver's
 *                   to_ascii_lowercase lookups)
 *     ..+12  f32 x3 unit direction (same value as the main record)
 *
 * The Rust loader is src/renderer/stars.rs::StarCatalog::from_bin. Keep the
 * two in sync -- the round-trip unit tests there lock the format.
 */

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const CSV_PATH = path.join(ROOT, 'data', 'stars.csv');
const BIN_PATH = path.join(ROOT, 'data', 'stars.bin');

const MAGIC = 'HOSSTAR1';
const RECORD_SIZE = 15;

/**
 * Mimic Rust's `field.parse::<f64>().unwrap_or(def)`: empty or non-numeric
 * strings fall back to the default instead of NaN leaking into the math.
 * (Number('') is 0 in JS, and parseFloat('1.2abc') is 1.2 -- both would
 * silently diverge from the Rust CSV fallback parser, so guard explicitly.)
 */
function rustF64(s, def) {
  if (s === '') return def;
  const v = Number(s);
  return Number.isNaN(v) ? def : v;
}

/** ASCII-only lowercase, matching Rust's to_ascii_lowercase (JS toLowerCase
 *  also folds non-ASCII, which would desync keys against the Rust lookups). */
function asciiLower(s) {
  let out = '';
  for (const ch of s) {
    const c = ch.charCodeAt(0);
    out += c >= 65 && c <= 90 ? String.fromCharCode(c + 32) : ch;
  }
  return out;
}

function main() {
  const t0 = Date.now();
  const csv = fs.readFileSync(CSV_PATH, 'utf8');
  const lines = csv.split(/\r?\n/);
  if (lines.length && lines[lines.length - 1] === '') lines.pop();

  // Header columns, resolved by name (same trim + quote-strip the Rust CSV
  // parser uses; the HYG file naively comma-splits fine -- no embedded commas).
  const strip = (s) => s.trim().replace(/^"|"$/g, '');
  const cols = lines[0].split(',').map(strip);
  const idx = (n) => cols.indexOf(n);
  const xi = idx('x'), yi = idx('y'), zi = idx('z');
  const mi = idx('mag'), cii = idx('ci');
  const pi = idx('proper'), bi = idx('bayer'), ci = idx('con');
  for (const [name, i] of [['x', xi], ['y', yi], ['z', zi], ['mag', mi], ['ci', cii], ['proper', pi], ['bayer', bi], ['con', ci]]) {
    if (i < 0) { console.error(`stars.csv: missing column '${name}'`); process.exit(1); }
  }
  const maxIdx = Math.max(xi, yi, zi, mi, cii, pi, bi, ci);

  const records = Buffer.alloc((lines.length - 1) * RECORD_SIZE);
  let starCount = 0;
  const namedParts = []; // Buffers, in row order (order is load-bearing, see header)
  let namedCount = 0;

  for (let li = 1; li < lines.length; li++) {
    const f = lines[li].split(',').map(strip);
    if (f.length <= maxIdx) continue; // malformed short row -- skip whole row

    const x = rustF64(f[xi], 0.0);
    const y = rustF64(f[yi], 0.0);
    const z = rustF64(f[zi], 0.0);
    const len = Math.sqrt(x * x + y * y + z * z);
    if (len < 0.001) continue; // Sol sits at the origin; no direction exists

    // f64 normalize, then the Buffer write rounds f64 -> f32 exactly like
    // Rust's `as f32` (round-to-nearest-even), so directions are bit-exact.
    const off = starCount * RECORD_SIZE;
    records.writeFloatLE(x / len, off);
    records.writeFloatLE(y / len, off + 4);
    records.writeFloatLE(z / len, off + 8);

    const mag = rustF64(f[mi], 20.0); // same default the CSV loader used
    const magQ = Math.min(65535, Math.max(0, Math.round((mag + 2.0) * 1024)));
    records.writeUInt16LE(magQ, off + 12);

    const civ = rustF64(f[cii], 0.0);
    const ciClamped = Math.min(2.0, Math.max(-0.4, civ));
    records.writeUInt8(Math.round(((ciClamped + 0.4) / 2.4) * 255), off + 14);
    starCount++;

    // Sidecar: name keys for the constellation resolver.
    const emitNamed = (kind, key) => {
      const kb = Buffer.from(key, 'utf8');
      if (kb.length === 0 || kb.length > 255) return;
      const e = Buffer.alloc(2 + kb.length + 12);
      e.writeUInt8(kind, 0);
      e.writeUInt8(kb.length, 1);
      kb.copy(e, 2);
      e.writeFloatLE(x / len, 2 + kb.length);
      e.writeFloatLE(y / len, 2 + kb.length + 4);
      e.writeFloatLE(z / len, 2 + kb.length + 8);
      namedParts.push(e);
      namedCount++;
    };
    const proper = f[pi];
    if (proper !== '') emitNamed(0, asciiLower(proper));
    const bayer = f[bi], con = f[ci];
    if (bayer !== '' && con !== '') emitNamed(1, `${asciiLower(bayer)} ${asciiLower(con)}`);
  }

  const header = Buffer.alloc(16);
  header.write(MAGIC, 0, 'ascii');
  header.writeUInt32LE(starCount, 8);
  header.writeUInt32LE(namedCount, 12);
  const bin = Buffer.concat([header, records.subarray(0, starCount * RECORD_SIZE), ...namedParts]);
  fs.writeFileSync(BIN_PATH, bin);

  // ── Self-verify: decode the file we just wrote and cross-check a spread of
  // stars against a fresh CSV recomputation, within quantization tolerance.
  // Catches encode/offset bugs at build time instead of as a black sky.
  verify(bin, lines, { xi, yi, zi, mi, cii, maxIdx, strip });

  const kb = (bin.length / 1024).toFixed(0);
  console.log(
    `stars.bin: ${starCount} stars + ${namedCount} named keys -> ${kb} KB ` +
    `(${(bin.length / starCount).toFixed(1)} B/star incl. sidecar) in ${Date.now() - t0} ms`
  );
}

function verify(bin, lines, ctx) {
  const { xi, yi, zi, mi, cii, maxIdx, strip } = ctx;
  if (bin.subarray(0, 8).toString('ascii') !== MAGIC) throw new Error('verify: bad magic');
  const starCount = bin.readUInt32LE(8);

  // Walk the CSV again independently, checking every 997th star (co-prime
  // stride -> spread across the whole catalog) plus the first and last.
  let sIdx = 0;
  let checked = 0;
  for (let li = 1; li < lines.length; li++) {
    const f = lines[li].split(',').map(strip);
    if (f.length <= maxIdx) continue;
    const x = rustF64(f[xi], 0.0), y = rustF64(f[yi], 0.0), z = rustF64(f[zi], 0.0);
    const len = Math.sqrt(x * x + y * y + z * z);
    if (len < 0.001) continue;
    if (sIdx % 997 === 0 || sIdx === starCount - 1) {
      const off = 16 + sIdx * RECORD_SIZE;
      const dx = bin.readFloatLE(off), dy = bin.readFloatLE(off + 4), dz = bin.readFloatLE(off + 8);
      if (dx !== Math.fround(x / len) || dy !== Math.fround(y / len) || dz !== Math.fround(z / len)) {
        throw new Error(`verify: direction mismatch at star ${sIdx} (csv line ${li + 1})`);
      }
      const mag = rustF64(f[mi], 20.0);
      const magBack = bin.readUInt16LE(off + 12) / 1024 - 2.0;
      if (Math.abs(magBack - mag) > 0.0005 && mag >= -2.0 && mag <= 61.9) {
        throw new Error(`verify: mag mismatch at star ${sIdx}: ${magBack} vs ${mag}`);
      }
      const civ = Math.min(2.0, Math.max(-0.4, rustF64(f[cii], 0.0)));
      const ciBack = (bin.readUInt8(off + 14) / 255) * 2.4 - 0.4;
      if (Math.abs(ciBack - civ) > 0.0048) {
        throw new Error(`verify: ci mismatch at star ${sIdx}: ${ciBack} vs ${civ}`);
      }
      checked++;
    }
    sIdx++;
  }
  if (sIdx !== starCount) throw new Error(`verify: star count mismatch: ${sIdx} vs ${starCount}`);
  console.log(`verify: ${checked} sampled stars match the CSV within quantization tolerance`);
}

main();
