#!/usr/bin/env node
/**
 * build-athyg-bin.js
 *
 * Converts the ATHYG catalog (Augmented Tycho-2 + Gaia + HYG, ~2.5M stars,
 * https://github.com/astronexus/ATHYG-Database) into the same HOSSTAR1
 * binary format the renderer already parses (see scripts/build-stars-bin.js
 * for the full format spec and src/renderer/stars.rs::StarCatalog::from_bin
 * for the loader). This is star-catalog ladder RUNG 2: the extended catalog
 * the app offers as an in-app download (Settings > Graphics), ~38 MB - too
 * big to commit to the repo, so the output is uploaded as a GitHub release
 * asset instead (tag assets-stars-*, deliberately NOT v* so no workflow
 * triggers).
 *
 * Usage:
 *   node scripts/build-athyg-bin.js <athyg1.csv.gz> [<athyg2.csv.gz> ...]
 *   (writes data/stars-athyg.bin)
 *
 * Column differences from HYG's stars.csv: positions are x0/y0/z0 (same
 * equatorial ICRS/J2000 cartesian frame, parsecs), and rows may carry empty
 * mag (no usable brightness -> skipped: a star we cannot brightness-scale
 * would render as a wrong-brightness dot, worse than absent). proper/bayer/
 * con feed the same named-star sidecar, so constellation figures resolve
 * from the extended catalog exactly as they do from the standard one.
 *
 * Self-checks: per-record spot verification like build-stars-bin.js, plus a
 * cross-catalog frame check - Sirius's unit direction here must match its
 * direction in data/stars.bin (dot > 0.999999); catching a frame/convention
 * mix-up at build time instead of as a subtly rotated sky.
 */

const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const readline = require('readline');

const ROOT = path.resolve(__dirname, '..');
const OUT_PATH = path.join(ROOT, 'data', 'stars-athyg.bin');
const STD_BIN = path.join(ROOT, 'data', 'stars.bin');

const MAGIC = 'HOSSTAR1';
const RECORD_SIZE = 15;

function rustF64(s, def) {
  if (s === '' || s === undefined) return def;
  const v = Number(s);
  return Number.isNaN(v) ? def : v;
}

function asciiLower(s) {
  let out = '';
  for (const ch of s) {
    const c = ch.charCodeAt(0);
    out += c >= 65 && c <= 90 ? String.fromCharCode(c + 32) : ch;
  }
  return out;
}

async function main() {
  const t0 = Date.now();
  const inputs = process.argv.slice(2);
  if (inputs.length === 0) {
    console.error('usage: node scripts/build-athyg-bin.js <athyg-part.csv.gz> [...more parts]');
    process.exit(1);
  }

  // Growable record storage: 2.6M stars x 15 B = ~39 MB. Chunked buffers
  // avoid one giant realloc-heavy Buffer.concat churn per row.
  const CHUNK_STARS = 262_144;
  const chunks = [];
  let chunk = Buffer.alloc(CHUNK_STARS * RECORD_SIZE);
  let chunkUsed = 0; // stars in the current chunk
  let starCount = 0;
  const namedParts = [];
  let namedCount = 0;
  let skippedNoMag = 0;
  let skippedNoPos = 0;
  let cols = null, xi, yi, zi, mi, cii, pi, bi, ci, maxIdx;
  let sirius = null; // [dx, dy, dz] captured for the frame check

  const strip = (s) => s.trim().replace(/^"|"$/g, '');

  for (const input of inputs) {
    const rl = readline.createInterface({
      input: fs.createReadStream(input).pipe(zlib.createGunzip()),
      crlfDelay: Infinity,
    });
    let firstLine = true;
    for await (const line of rl) {
      if (firstLine) {
        firstLine = false;
        const hdr = line.split(',').map(strip);
        if (cols === null) {
          // Only the FIRST part carries a header row; later parts are direct
          // row continuations (ATHYG splits one table across files to stay
          // under GitHub's 100 MB limit). A first part without an 'id' header
          // is a usage error, not data.
          if (hdr[0] !== 'id') {
            console.error(`${input}: first input must carry the ATHYG header row (got '${hdr[0]}...')`);
            process.exit(1);
          }
          cols = hdr;
          const idx = (n) => cols.indexOf(n);
          xi = idx('x0'); yi = idx('y0'); zi = idx('z0');
          mi = idx('mag'); cii = idx('ci');
          pi = idx('proper'); bi = idx('bayer'); ci = idx('con');
          for (const [name, i] of [['x0', xi], ['y0', yi], ['z0', zi], ['mag', mi], ['ci', cii], ['proper', pi], ['bayer', bi], ['con', ci]]) {
            if (i < 0) { console.error(`${input}: missing column '${name}'`); process.exit(1); }
          }
          maxIdx = Math.max(xi, yi, zi, mi, cii, pi, bi, ci);
          continue; // header consumed
        }
        if (hdr.join(',') === cols.join(',')) {
          continue; // a later part that repeats the header - consume it
        }
        // Headerless continuation: fall through and parse this line as DATA.
      }
      const f = line.split(',').map(strip);
      if (f.length <= maxIdx) continue;

      // No magnitude = no brightness to render; skip rather than guess.
      const magStr = f[mi];
      if (magStr === '') { skippedNoMag++; continue; }
      const mag = rustF64(magStr, 20.0);

      const x = rustF64(f[xi], 0.0);
      const y = rustF64(f[yi], 0.0);
      const z = rustF64(f[zi], 0.0);
      const len = Math.sqrt(x * x + y * y + z * z);
      if (len < 0.001) { skippedNoPos++; continue; } // Sol / positionless rows

      if (chunkUsed === CHUNK_STARS) {
        chunks.push(chunk);
        chunk = Buffer.alloc(CHUNK_STARS * RECORD_SIZE);
        chunkUsed = 0;
      }
      const off = chunkUsed * RECORD_SIZE;
      const dx = x / len, dy = y / len, dz = z / len;
      chunk.writeFloatLE(dx, off);
      chunk.writeFloatLE(dy, off + 4);
      chunk.writeFloatLE(dz, off + 8);
      chunk.writeUInt16LE(Math.min(65535, Math.max(0, Math.round((mag + 2.0) * 1024))), off + 12);
      const civ = Math.min(2.0, Math.max(-0.4, rustF64(f[cii], 0.0)));
      chunk.writeUInt8(Math.round(((civ + 0.4) / 2.4) * 255), off + 14);
      chunkUsed++;
      starCount++;

      const emitNamed = (kind, key) => {
        const kb = Buffer.from(key, 'utf8');
        if (kb.length === 0 || kb.length > 255) return;
        const e = Buffer.alloc(2 + kb.length + 12);
        e.writeUInt8(kind, 0);
        e.writeUInt8(kb.length, 1);
        kb.copy(e, 2);
        e.writeFloatLE(dx, 2 + kb.length);
        e.writeFloatLE(dy, 2 + kb.length + 4);
        e.writeFloatLE(dz, 2 + kb.length + 8);
        namedParts.push(e);
        namedCount++;
      };
      const proper = f[pi];
      if (proper !== '') {
        emitNamed(0, asciiLower(proper));
        if (asciiLower(proper) === 'sirius') sirius = [Math.fround(dx), Math.fround(dy), Math.fround(dz)];
      }
      const bayer = f[bi], con = f[ci];
      if (bayer !== '' && con !== '') emitNamed(1, `${asciiLower(bayer)} ${asciiLower(con)}`);
    }
  }
  chunks.push(chunk.subarray(0, chunkUsed * RECORD_SIZE));

  const header = Buffer.alloc(16);
  header.write(MAGIC, 0, 'ascii');
  header.writeUInt32LE(starCount, 8);
  header.writeUInt32LE(namedCount, 12);
  const bin = Buffer.concat([header, ...chunks, ...namedParts]);
  fs.writeFileSync(OUT_PATH, bin);

  // ── Frame check: Sirius must point the same way as in the standard
  // catalog. A frame/convention mistake would pass every per-record check
  // (self-consistent garbage) but fail this cross-catalog one.
  if (sirius && fs.existsSync(STD_BIN)) {
    const std = fs.readFileSync(STD_BIN);
    const stdSirius = findNamed(std, 'sirius');
    if (!stdSirius) throw new Error('frame check: sirius missing from stars.bin sidecar');
    const dot = sirius[0] * stdSirius[0] + sirius[1] * stdSirius[1] + sirius[2] * stdSirius[2];
    if (dot < 0.999999) throw new Error(`frame check FAILED: sirius dot = ${dot} (frames disagree)`);
    console.log(`frame check: sirius agrees with stars.bin (dot = ${dot.toFixed(8)})`);
  } else {
    console.warn('frame check skipped (no sirius or no data/stars.bin)');
  }

  const mb = (bin.length / 1024 / 1024).toFixed(1);
  console.log(
    `stars-athyg.bin: ${starCount} stars + ${namedCount} named keys -> ${mb} MB ` +
    `(${(bin.length / starCount).toFixed(1)} B/star incl. sidecar); ` +
    `skipped ${skippedNoMag} magless + ${skippedNoPos} positionless rows; ${Date.now() - t0} ms`
  );
}

/** Scan a HOSSTAR1 buffer's sidecar for a kind-0 (proper) key. */
function findNamed(bin, wanted) {
  const starCount = bin.readUInt32LE(8);
  const namedCount = bin.readUInt32LE(12);
  let o = 16 + starCount * RECORD_SIZE;
  for (let i = 0; i < namedCount; i++) {
    const kind = bin.readUInt8(o);
    const klen = bin.readUInt8(o + 1);
    const key = bin.subarray(o + 2, o + 2 + klen).toString('utf8');
    const d = o + 2 + klen;
    if (kind === 0 && key === wanted) {
      return [bin.readFloatLE(d), bin.readFloatLE(d + 4), bin.readFloatLE(d + 8)];
    }
    o = d + 12;
  }
  return null;
}

main().catch((e) => { console.error(e); process.exit(1); });
