// The website's opening of a held copy (2026-11-20, docs/design/hide-and-keep.md rung 5):
// web/shared/held-seal.js, the twin of crates/humanity-protocol/src/held_seal.rs's opening half,
// and the one-file export both apps write.
//
// Run: node --test scripts/tests/held-web.test.js   (in `just rig-tests`)
//
// What it proves:
//   1. The parts the relay's code sealed (scripts/tests/fixtures/held-v1.json, written by the Rust
//      test `held_fixture_rust_and_the_website_agree`) open here to exactly the bytes they hold,
//      and with no other key.
//   2. A part of several pieces opens whole, and one cut at a piece's end, with a piece swapped or
//      a bit flipped, does not (sealed here by a test-only twin of `seal_stream`).
//   3. The export is a .zip any system opens: every entry stored with its CRC-32, the list and the
//      words and each file in it, and never the restore key.
// HOS_WEB_DIR points the test at another copy of web/ (used to see it red).
//
// Seen red 2026-11-20, each against a copy of held-seal.js with one change: the last-piece flag
// written `${last}` ("true") instead of 1: test 1, "this part does not open with this key"; the
// last piece taken to be the one shorter than a full piece (`len < PIECE + TAG`): test 2, at the
// part of exactly two full pieces (added for it: the first three-piece case passed that change);
// `delete about.restore_key` left out: test 3, "the list carries neither the restore key".

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

const REPO = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(REPO, 'web');
const FX = JSON.parse(fs.readFileSync(path.join(REPO, 'scripts', 'tests', 'fixtures', 'held-v1.json'), 'utf8'));
const enc = new TextEncoder();

let m = null;
let H = null;
async function load() {
  if (!m) {
    m = await import(pathToFileURL(path.join(REPO, 'web', 'shared', 'vendor', 'noble-pq.bundle.js')).href);
    H = await import(pathToFileURL(path.join(WEB, 'shared', 'held-seal.js')).href);
  }
  return { m, H };
}

function kyberOf(m, seed) {
  return m.ml_kem768.keygen(m.blake3.create({ context: enc.encode('hum/kyber768/v1'), dkLen: 64 }).update(seed).digest());
}
const hex = (u8) => Buffer.from(u8).toString('hex');

/** A test-only twin of the Rust `seal_stream`, so the opening of many pieces can be tried here. */
async function sealPart(m, H, publicKey, context, plain) {
  const { cipherText, sharedSecret } = m.ml_kem768.encapsulate(publicKey);
  const keyBytes = m.blake3.create({ context: enc.encode(H.DOMAIN), dkLen: 32 }).update(sharedSecret).digest();
  const key = await crypto.subtle.importKey('raw', keyBytes, { name: 'AES-GCM' }, false, ['encrypt']);
  const out = [enc.encode(H.MAGIC), cipherText];
  const pieces = Math.max(1, Math.ceil(plain.length / H.PIECE));
  for (let i = 0; i < pieces; i++) {
    const nonce = new Uint8Array(12);
    new DataView(nonce.buffer).setBigUint64(4, BigInt(i));
    const aad = enc.encode(H.pieceAad(context, i, i === pieces - 1));
    out.push(new Uint8Array(await crypto.subtle.encrypt({ name: 'AES-GCM', iv: nonce, additionalData: aad }, key, plain.subarray(i * H.PIECE, (i + 1) * H.PIECE))));
  }
  return Buffer.concat(out.map((b) => Buffer.from(b)));
}

/** Read a stored .zip back: its entries' names and bytes, each CRC checked. */
function unzip(H, bytes) {
  const b = Buffer.from(bytes);
  const endAt = b.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
  assert.ok(endAt >= 0, 'an end record');
  const count = b.readUInt16LE(endAt + 10);
  let at = b.readUInt32LE(endAt + 16);
  const out = new Map();
  for (let i = 0; i < count; i++) {
    assert.strictEqual(b.readUInt32LE(at), 0x02014b50, 'a central directory entry');
    const crc = b.readUInt32LE(at + 16);
    const size = b.readUInt32LE(at + 24);
    const nameLen = b.readUInt16LE(at + 28);
    const local = b.readUInt32LE(at + 42);
    const name = b.subarray(at + 46, at + 46 + nameLen).toString('utf8');
    assert.strictEqual(b.readUInt32LE(local), 0x04034b50, 'its local header');
    assert.strictEqual(b.readUInt16LE(local + 8), 0, 'stored');
    const dataAt = local + 30 + b.readUInt16LE(local + 26) + b.readUInt16LE(local + 28);
    const data = b.subarray(dataAt, dataAt + size);
    assert.strictEqual(H.crc32(data), crc, name + ' has its CRC-32');
    out.set(name, data);
    at += 46 + nameLen + b.readUInt16LE(at + 30) + b.readUInt16LE(at + 32);
  }
  return out;
}

test('the parts the relay sealed open here, and only with their key', async () => {
  const { m, H } = await load();
  const me = kyberOf(m, Buffer.from(FX.seed_hex, 'hex'));
  const other = kyberOf(m, new Uint8Array(32).fill(8));
  assert.ok(FX.parts.length >= 3, 'the fixture holds a manifest, a file and an empty part');
  for (const p of FX.parts) {
    const sealed = Buffer.from(p.sealed_b64, 'base64');
    const ctx = H.partContext(FX.package, p.part);
    assert.strictEqual(hex(await H.openPart(m, me.secretKey, ctx, sealed)), p.plain_hex, 'part ' + p.part + ' opens to what Rust sealed');
    await assert.rejects(H.openPart(m, other.secretKey, ctx, sealed), 'part ' + p.part + ' opens for nobody else');
    await assert.rejects(H.openPart(m, me.secretKey, H.partContext(FX.package, p.part + 1), sealed), 'nor as another part');
  }
});

test('a part of many pieces opens whole, and a cut, swapped or altered one does not', async () => {
  const { m, H } = await load();
  const me = kyberOf(m, new Uint8Array(32).fill(9));
  const ctx = H.partContext('pkg', 1);
  const plain = new Uint8Array(2 * H.PIECE + 5).map((_, i) => i % 251);
  const sealed = await sealPart(m, H, me.publicKey, ctx, plain);
  assert.deepStrictEqual(Buffer.from(await H.openPart(m, me.secretKey, ctx, sealed)), Buffer.from(plain), 'three pieces open whole');
  const piece = H.PIECE + H.TAG;
  await assert.rejects(H.openPart(m, me.secretKey, ctx, sealed.subarray(0, H.HEADER + piece)), 'cut at a piece\'s end');
  const body = sealed.subarray(H.HEADER);
  const swapped = Buffer.concat([sealed.subarray(0, H.HEADER), body.subarray(piece, 2 * piece), body.subarray(0, piece), body.subarray(2 * piece)]);
  await assert.rejects(H.openPart(m, me.secretKey, ctx, swapped), 'pieces swapped');
  const flipped = Buffer.from(sealed);
  flipped[H.HEADER + 10] ^= 1;
  await assert.rejects(H.openPart(m, me.secretKey, ctx, flipped), 'a bit flipped');
  const empty = await sealPart(m, H, me.publicKey, ctx, new Uint8Array(0));
  assert.strictEqual((await H.openPart(m, me.secretKey, ctx, empty)).length, 0, 'an empty part opens empty');
  // Exactly two full pieces: the last one is full-size and still the last (Rust looks ahead).
  const even = new Uint8Array(2 * H.PIECE).map((_, i) => i % 13);
  const evenSealed = await sealPart(m, H, me.publicKey, ctx, even);
  assert.deepStrictEqual(Buffer.from(await H.openPart(m, me.secretKey, ctx, evenSealed)), Buffer.from(even), 'a part of exactly two full pieces opens whole');
});

test('the export is one stored .zip with the list, the words and each file, never the restore key', async () => {
  const { H } = await load();
  const manifest = {
    v: 1, package: 'p', report_id: 4, author: 'aa', stamp: 42, channel: 'global', words: 'the words, exactly', held_by: 'bb', held_at: 1,
    restore_key: 'ff'.repeat(32),
    files: [
      { part: 1, name: '1_x_pic.png', folder: 'public', uploader: 'aa', shelf: 0, original_name: 'pic.png', bytes: 3, hash: '', poster_part: null, poster_bytes: 0, poster_hash: '' },
      { part: 2, name: '2_x_vid.mp4', folder: 'members', uploader: 'aa', shelf: 2, original_name: '../evil/vid.mp4', bytes: 3, hash: '', poster_part: 3, poster_bytes: 1, poster_hash: '' },
    ],
  };
  const files = new Map([[1, enc.encode('png')], [2, enc.encode('mp4')], [3, enc.encode('j')]]);
  const zip = unzip(H, H.zipStored(H.exportEntries(manifest, files, 99)));
  assert.deepStrictEqual([...zip.keys()], ['held-item.json', 'post.txt', 'files/1-pic.png', 'files/2-vid.mp4', 'files/3-poster-vid.mp4.jpg'], 'the layout, with no folder escape');
  assert.strictEqual(zip.get('post.txt').toString('utf8'), 'the words, exactly');
  assert.strictEqual(zip.get('files/2-vid.mp4').toString('utf8'), 'mp4');
  const about = JSON.parse(zip.get('held-item.json').toString('utf8'));
  assert.strictEqual(about.report_id, 4);
  assert.strictEqual(about.exported_at, 99);
  assert.ok(!('restore_key' in about) && !('words' in about), 'the list carries neither the restore key nor the words twice');
  assert.ok(!Buffer.from(H.zipStored(H.exportEntries(manifest, files, 99))).includes('ff'.repeat(32)), 'the restore key is nowhere in the file');
});
