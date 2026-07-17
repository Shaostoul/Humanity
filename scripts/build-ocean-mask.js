#!/usr/bin/env node
/**
 * build-ocean-mask.js
 *
 * Derive data/planets/earth_ocean_mask.bin from the shipped heightmap:
 * which cells are OCEAN (connected salt water) vs land - including land
 * that lies BELOW sea level (Death Valley, the Dead Sea shore, the Caspian
 * depression), which a naive "elevation <= 0" test would flood.
 *
 * Method: flood fill (BFS) across cells with elevation <= 0 m, seeded in
 * the mid-Pacific. Cells the fill reaches are ocean; below-sea-level cells
 * it cannot reach stay dry. Longitude wraps; latitude clamps.
 *
 * NOTE the Caspian Sea: it IS water in reality, but it is endorheic (not
 * connected to the world ocean), so this mask correctly classifies it as
 * NOT ocean. Inland water (lakes, seas without an ocean connection) is a
 * separate future mask/class - see docs/design/ocean.md.
 *
 * Output format "HOSOCM1" (little-endian):
 *   bytes 0..7   magic b"HOSOCM1"
 *   bytes 7..11  u32 width   (must match the heightmap grid)
 *   bytes 11..15 u32 height
 *   bytes 15..   ceil(width*height / 8) bytes, bit-packed row-major
 *                (row 0 = northernmost; bit set = ocean). Bit i of byte
 *                floor(i/8) is (cellIndex & 7) with LSB-first packing.
 *
 * Usage: node scripts/build-ocean-mask.js [heightmap.bin] [out.bin]
 * Rust loader + tests: src/terrain/ocean_mask.rs (keep formats in sync).
 */
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const HM_PATH = process.argv[2] || path.join(ROOT, 'data', 'planets', 'earth_heightmap.bin');
const OUT_PATH = process.argv[3] || path.join(ROOT, 'data', 'planets', 'earth_ocean_mask.bin');

// ── Read the HOSHGT1 heightmap ──
const buf = fs.readFileSync(HM_PATH);
if (buf.toString('latin1', 0, 7) !== 'HOSHGT1') throw new Error('bad heightmap magic');
const W = buf.readUInt32LE(7);
const H = buf.readUInt32LE(11);
const MIN_M = buf.readFloatLE(15);
const MAX_M = buf.readFloatLE(19);
const range = MAX_M - MIN_M;
console.log(`heightmap: ${W}x${H}, ${MIN_M}..${MAX_M} m`);

const metersAt = (x, y) => {
  const q = buf.readUInt16LE(23 + 2 * (y * W + x));
  return MIN_M + (q / 65535) * range;
};

// ── Flood fill ocean from a mid-Pacific seed ──
// Cell indices for a lat/lon (cell-centered grid, row 0 north).
const cellOf = (lat, lon) => ({
  x: Math.min(W - 1, Math.max(0, Math.floor(((lon + 180) / 360) * W))),
  y: Math.min(H - 1, Math.max(0, Math.floor(((90 - lat) / 180) * H))),
});

const ocean = new Uint8Array(W * H); // 1 = ocean (unpacked while filling)
const seed = cellOf(0, -160); // mid-Pacific, guaranteed open ocean
if (metersAt(seed.x, seed.y) > 0) throw new Error('seed cell is above sea level?!');

// BFS with an explicit queue (W*H up to ~6.5M cells today; scales fine).
const qx = new Int32Array(W * H);
const qy = new Int32Array(W * H);
let head = 0, tail = 0;
const push = (x, y) => {
  const i = y * W + x;
  if (!ocean[i] && metersAt(x, y) <= 0) {
    ocean[i] = 1;
    qx[tail] = x; qy[tail] = y; tail++;
  }
};
push(seed.x, seed.y);
while (head < tail) {
  const x = qx[head], y = qy[head]; head++;
  push((x + 1) % W, y);            // east (wraps)
  push((x + W - 1) % W, y);        // west (wraps)
  if (y + 1 < H) push(x, y + 1);   // south
  if (y > 0) push(x, y - 1);       // north
}

// ── Stats + sanity ──
let oceanCells = 0, belowDry = 0;
for (let y = 0; y < H; y++) {
  for (let x = 0; x < W; x++) {
    const i = y * W + x;
    if (ocean[i]) oceanCells++;
    else if (metersAt(x, y) <= 0) belowDry++;
  }
}
const pct = ((oceanCells / (W * H)) * 100).toFixed(1);
console.log(`ocean cells: ${oceanCells} (${pct}% of grid; Earth is ~71% by area - equirectangular over-weights the poles so a bit off is expected)`);
console.log(`below-sea-level DRY cells (Death Valley class + endorheic seas): ${belowDry}`);
const dv = cellOf(36.23, -116.82); // Death Valley
const atl = cellOf(30, -40); // mid-Atlantic
console.log(`Death Valley is ocean? ${!!ocean[dv.y * W + dv.x]} (must be false)`);
console.log(`Mid-Atlantic is ocean? ${!!ocean[atl.y * W + atl.x]} (must be true)`);
if (ocean[dv.y * W + dv.x] || !ocean[atl.y * W + atl.x]) throw new Error('sanity check failed');

// ── Bit-pack + write ──
const packed = Buffer.alloc(Math.ceil((W * H) / 8));
for (let i = 0; i < W * H; i++) {
  if (ocean[i]) packed[i >> 3] |= 1 << (i & 7);
}
const header = Buffer.alloc(15);
header.write('HOSOCM1', 0, 'latin1');
header.writeUInt32LE(W, 7);
header.writeUInt32LE(H, 11);
fs.writeFileSync(OUT_PATH, Buffer.concat([header, packed]));
console.log(`wrote ${OUT_PATH} (${((15 + packed.length) / 1048576).toFixed(2)} MB)`);
