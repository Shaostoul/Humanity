'use strict';
/**
 * The pipeline's own .7z reader (scripts/lib/sevenzip.js) on archives written here byte by byte, so
 * it runs anywhere without 7-Zip.
 *
 * Why (2026-11-13): FreePats' Ukulele (20260811) is the first sound bank whose folders are a CHAIN of
 * two coders, Deflate and 7-Zip's Delta filter, and the reader only knew one coder per folder. The
 * archives below are a Deflate-and-Delta chain in both orders a writer may list the coders (7-Zip
 * lists the last stage first; the ukulele's writer listed Deflate first), and the file's CRC32 must
 * come out right.
 *
 * Seen RED first (2026-11-13) against the reader before the chain support (the HEAD version copied
 * over it, this file run, the change put back): "7z: a folder of 2 coders is not supported", the
 * same error the real ukulele archive gave.
 */

const test = require('node:test');
const assert = require('node:assert');
const zlib = require('zlib');
const { readSevenZip } = require('../lib/sevenzip');

/** A 7z "number" (values under 16384 are all these archives need). */
function num(n) {
  if (n < 0x80) return Buffer.from([n]);
  if (n < 0x4000) return Buffer.from([0x80 | (n >> 8), n & 0xff]);
  throw new Error('fixture number too large');
}

function deltaEncode(data, distance) {
  const out = Buffer.from(data);
  for (let i = data.length - 1; i >= distance; i--) out[i] = (data[i] - data[i - distance]) & 0xff;
  return out;
}

/**
 * One file in one folder of two coders: Delta (distance 3, property byte 2) then Deflate.
 * `deltaFirst`: 7-Zip's order (coder 0 is the Delta filter whose output is the folder's, fed by the
 * Deflate coder 1 that reads the packed stream); otherwise the ukulele's order (coder 0 is Deflate).
 */
function chainArchive(name, data, deltaFirst) {
  const packed = zlib.deflateRawSync(deltaEncode(data, 3));
  const delta = Buffer.concat([Buffer.from([0x20 | 1, 0x03]), num(1), Buffer.from([0x02])]);
  const deflate = Buffer.from([3, 0x04, 0x01, 0x08]);
  const coders = deltaFirst ? [delta, deflate] : [deflate, delta];
  // Bind pair (in, out): the Delta coder's input is the Deflate coder's output.
  const bind = deltaFirst ? [num(0), num(1)] : [num(1), num(0)];
  const folder = Buffer.concat([num(2), ...coders, ...bind]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32LE(zlib.crc32(data));
  const nameBytes = Buffer.concat([Buffer.from(name, 'utf16le'), Buffer.alloc(2)]);
  const header = Buffer.concat([
    Buffer.from([0x01, 0x04]),
    Buffer.from([0x06]), num(0), num(1), Buffer.from([0x09]), num(packed.length), Buffer.from([0x00]),
    Buffer.from([0x07, 0x0b]), num(1), Buffer.from([0x00]), folder,
    Buffer.from([0x0c]), num(data.length), num(data.length),
    Buffer.from([0x0a, 0x01]), crc, Buffer.from([0x00]),
    Buffer.from([0x00]),
    Buffer.from([0x05]), num(1), Buffer.from([0x11]), num(1 + nameBytes.length), Buffer.from([0x00]), nameBytes, Buffer.from([0x00]),
    Buffer.from([0x00]),
  ]);
  const start = Buffer.alloc(32);
  Buffer.from([0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c, 0x00, 0x04]).copy(start, 0);
  start.writeBigUInt64LE(BigInt(packed.length), 12);
  start.writeBigUInt64LE(BigInt(header.length), 20);
  start.writeUInt32LE(zlib.crc32(header), 28);
  start.writeUInt32LE(zlib.crc32(start.subarray(12, 32)), 8);
  return Buffer.concat([start, packed, header]);
}

// Something with structure the Delta filter changes: a 16-bit stereo-ish ramp.
const DATA = Buffer.from(Array.from({ length: 3000 }, (_, i) => (i * 7 + (i % 3) * 50) & 0xff));

for (const deltaFirst of [true, false]) {
  test(`a Deflate and Delta chain, ${deltaFirst ? "7-Zip's order" : "the ukulele's order"}`, () => {
    const files = readSevenZip(chainArchive('samples/C4.flac', DATA, deltaFirst));
    assert.strictEqual(files.length, 1);
    assert.strictEqual(files[0].name, 'samples/C4.flac');
    assert.ok(files[0].data.equals(DATA), 'the file comes back byte for byte (its CRC32 checked too)');
  });
}

test('a damaged chained archive fails its checksum', () => {
  const a = chainArchive('x.bin', DATA, true);
  // Flip a byte of the stored CRC32 in the header (the header checksum is rewritten to match, so
  // only the file's own checksum can catch it).
  const nextOffset = Number(a.readBigUInt64LE(12));
  const header = a.subarray(32 + nextOffset);
  const at = header.indexOf(Buffer.from([0x0a, 0x01])) + 2;
  header[at] ^= 0xff;
  a.writeUInt32LE(zlib.crc32(header), 28);
  a.writeUInt32LE(zlib.crc32(a.subarray(12, 32)), 8);
  assert.throws(() => readSevenZip(a), /checksum/);
});
