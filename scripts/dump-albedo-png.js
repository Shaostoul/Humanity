#!/usr/bin/env node
// Dump a HOSALB1 planet albedo .bin (data/planets/*_albedo.bin) back to a
// viewable PNG -- the eyeball end of the bake pipeline. Use it to verify a
// bake visually (registration, pole fills, seams) before committing:
//
//   node scripts/dump-albedo-png.js data/planets/pluto_albedo.bin out.png
//
// Zero dependencies: a minimal PNG encoder (filter-0 rows + zlib deflate +
// hand-rolled CRC32) is ~40 lines. Output is 8-bit RGB non-interlaced --
// the same subset scripts/build-planet-albedo.js can decode, so a dump can
// even be round-tripped through the baker as a smoke test.

'use strict';

const fs = require('fs');
const zlib = require('zlib');

function fail(msg) {
  console.error('ERROR: ' + msg);
  process.exit(1);
}

const [binPath, outPath] = process.argv.slice(2);
if (!binPath || !outPath) {
  fail('usage: node scripts/dump-albedo-png.js <albedo.bin> <out.png>');
}

const buf = fs.readFileSync(binPath);
if (buf.length < 15 || buf.toString('ascii', 0, 7) !== 'HOSALB1') {
  fail('not a HOSALB1 albedo file');
}
const width = buf.readUInt32LE(7);
const height = buf.readUInt32LE(11);
const expected = 15 + width * height * 3;
if (buf.length !== expected) {
  fail(`payload is ${buf.length} bytes, expected ${expected} for ${width}x${height}`);
}
const pixels = buf.subarray(15);

// CRC32 (PNG polynomial), table-driven.
const CRC_TABLE = new Uint32Array(256);
for (let n = 0; n < 256; n++) {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  CRC_TABLE[n] = c >>> 0;
}
function crc32(...bufs) {
  let c = 0xffffffff;
  for (const b of bufs) {
    for (let i = 0; i < b.length; i++) c = CRC_TABLE[(c ^ b[i]) & 0xff] ^ (c >>> 8);
  }
  return (c ^ 0xffffffff) >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const typeBuf = Buffer.from(type, 'ascii');
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(typeBuf, data));
  return Buffer.concat([len, typeBuf, data, crc]);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(width, 0);
ihdr.writeUInt32BE(height, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 2; // color type RGB
// [10..12] compression/filter/interlace = 0

// Raw scanlines: filter byte 0 + RGB row.
const stride = width * 3;
const raw = Buffer.alloc((stride + 1) * height);
for (let y = 0; y < height; y++) {
  raw[y * (stride + 1)] = 0;
  pixels.copy(raw, y * (stride + 1) + 1, y * stride, (y + 1) * stride);
}

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk('IHDR', ihdr),
  chunk('IDAT', zlib.deflateSync(raw, { level: 6 })),
  chunk('IEND', Buffer.alloc(0)),
]);
fs.writeFileSync(outPath, png);
console.log(`Wrote ${outPath} (${width}x${height}, ${(png.length / 1e6).toFixed(2)} MB)`);
