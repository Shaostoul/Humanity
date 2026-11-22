// The website's opening of a held copy (2026-11-20, docs/design/hide-and-keep.md rung 5):
// web/shared/held-seal.js, the twin of crates/humanity-protocol/src/held_seal.rs (format v4), and
// the one-file export both apps write.
//
// Run: node --test scripts/tests/held-web.test.js   (in `just rig-tests`)
//
// What it proves:
//   1. The parts the relay's code sealed (scripts/tests/fixtures/held-v4.json, written by the Rust
//      test `held_fixture_rust_and_the_website_agree`) open here to exactly the bytes they hold,
//      and with no other key.
//   2. The website's own sealing opens for both people, and its RE-WRAP of the Rust fixture's
//      headers for the successor alone heads the same bodies; both go into held-v4-web.json, which
//      the Rust test opens (JS writes, Rust reads).
//   3. A part of several pieces opens whole, and one cut at a piece's end, with a piece swapped or
//      a bit flipped, does not.
//   4. The export is a .zip any system opens: every entry stored with its CRC-32, the list and the
//      words and each file in it, and never the restore key.
//   5. The owner fixture (scripts/tests/fixtures/held-owner-v2.json, written by the Rust test
//      `held_owner_fixture_rust_and_the_website_agree`): the held part routes' caller lines (each
//      with its domain line), a recipient's check of its own slot of each new header, the header
//      hashes and the acceptance digest with its round, the same on both sides; and the check
//      catches a garbage slot, the wrong count, the wrong place, a slot wrapping another key, and a
//      body sent where a header belongs.
// HOS_WEB_DIR points the test at another copy of web/ (used to see it red).
//
// Seen red 2026-11-20, each against a copy of held-seal.js with one change: the last-piece flag
// written `${last}` ("true") instead of 1: test 1, "this part does not open with this key"; the
// last piece taken to be the one shorter than a full piece (`len < PIECE + TAG`): test 3, at the
// part of exactly two full pieces; `delete about.restore_key` left out: test 4, "the list carries
// neither the restore key"; openPart reading only the first recipient's slot: test 1. For v4: the
// commitment written into the associated data as raw bytes, not hex: test 1 ("part 0 opens to what
// Rust sealed"); the commitment comparison taken out of checkHeader: "a slot wrapping another key
// under the real commitment is caught". Both v4 breaks re-run 2026-11-21 by mutation in the real
// file (scratchpad ownfix4-mutate.js): red as stated (the raw commitment in a piece's associated
// data failed tests 1 to 3).

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

const REPO = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(REPO, 'web');
const FX = JSON.parse(fs.readFileSync(path.join(REPO, 'scripts', 'tests', 'fixtures', 'held-v4.json'), 'utf8'));
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

/** Seal with the website's own `sealPart` to one key (the multi-piece tests below). */
async function sealPart(m, H, publicKey, context, plain) {
  return Buffer.from(await H.sealPart(m, [publicKey], context, plain));
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

test('the parts the relay sealed open here, for the owner and the successor, and only with their keys', async () => {
  const { m, H } = await load();
  const me = kyberOf(m, Buffer.from(FX.seed_hex, 'hex'));
  const successor = kyberOf(m, Buffer.from(FX.second_seed_hex, 'hex'));
  const other = kyberOf(m, new Uint8Array(32).fill(8));
  assert.ok(FX.parts.length >= 3, 'the fixture holds a manifest, a file and an empty part');
  for (const p of FX.parts) {
    const sealed = Buffer.from(p.sealed_b64, 'base64');
    const ctx = H.partContext(FX.package, p.part);
    assert.strictEqual(hex(await H.openPart(m, me.secretKey, ctx, sealed)), p.plain_hex, 'part ' + p.part + ' opens to what Rust sealed');
    assert.strictEqual(hex(await H.openPart(m, successor.secretKey, ctx, sealed)), p.plain_hex, 'part ' + p.part + ' opens for the successor');
    await assert.rejects(H.openPart(m, other.secretKey, ctx, sealed), 'part ' + p.part + ' opens for nobody else');
    await assert.rejects(H.openPart(m, me.secretKey, H.partContext(FX.package, p.part + 1), sealed), 'nor as another part');
  }
});

// The website seals a part to two people, and RE-WRAPS the Rust fixture's headers for the
// successor alone (a change of recipients in a browser); the Rust test opens
// scripts/tests/fixtures/held-v4-web.json (HELD_FIXTURE_WRITE=1 writes a fresh one).
test('the website seals a part to two people and re-wraps a header, and Rust opens both', async () => {
  const { m, H } = await load();
  const owner = kyberOf(m, Buffer.from(FX.seed_hex, 'hex'));
  const successor = kyberOf(m, Buffer.from(FX.second_seed_hex, 'hex'));
  const other = kyberOf(m, new Uint8Array(32).fill(8));
  const ctx = H.partContext('pkgweb', 1);
  const plain = new Uint8Array(H.PIECE + 3).map((_, i) => i % 199);
  const sealed = await H.sealPart(m, [owner.publicKey, successor.publicKey], ctx, plain);
  assert.strictEqual(sealed.length, H.sealedLen(plain.length, 2), 'its size is sealedLen');
  assert.deepStrictEqual(Buffer.from(await H.openPart(m, owner.secretKey, ctx, sealed)), Buffer.from(plain), 'the owner opens it');
  assert.deepStrictEqual(Buffer.from(await H.openPart(m, successor.secretKey, ctx, sealed)), Buffer.from(plain), 'the successor opens it');
  await assert.rejects(H.openPart(m, other.secretKey, ctx, sealed), 'nobody else does');
  const webFx = path.join(REPO, 'scripts', 'tests', 'fixtures', 'held-v4-web.json');
  // The re-wrap: each Rust-sealed part's header, opened with the owner's key and wrapped for the
  // successor alone, heads the very same body for the successor and no longer for the owner.
  const rewrapped = [];
  for (const p of FX.parts) {
    const sealed = new Uint8Array(Buffer.from(p.sealed_b64, 'base64'));
    const ctx2 = H.partContext(FX.package, p.part);
    const header = await H.rewrapHeader(m, owner.secretKey, ctx2, sealed.subarray(0, H.headerLen(2)), [successor.publicKey]);
    assert.strictEqual(header.length, H.headerLen(1), 'a header alone');
    const rebuilt = Buffer.concat([header, sealed.subarray(H.headerLen(2))]);
    assert.strictEqual(hex(await H.openPart(m, successor.secretKey, ctx2, rebuilt)), p.plain_hex, 'the re-wrapped part ' + p.part + ' opens for the successor');
    await assert.rejects(H.openPart(m, owner.secretKey, ctx2, rebuilt), 'and no longer for the owner');
    rewrapped.push({ part: p.part, header_b64: Buffer.from(header).toString('base64') });
  }
  if (process.env.HELD_FIXTURE_WRITE) {
    const pkg = 'fedcba9876543210fedcba9876543210';
    const wanted = [[0, enc.encode('{"v":1,"package":"' + pkg + '","words":"sealed by the website"}')], [1, enc.encode('a file the website sealed')], [2, new Uint8Array(0)]];
    const parts = [];
    for (const [n, p] of wanted) {
      const s2 = await H.sealPart(m, [owner.publicKey, successor.publicKey], H.partContext(pkg, n), p);
      parts.push({ part: n, sealed_b64: Buffer.from(s2).toString('base64'), plain_hex: hex(p) });
    }
    const about = 'Held parts (format v4) sealed by web/shared/held-seal.js sealPart to the two seeds of held-v4.json, and headers of held-v4.json re-wrapped by rewrapHeader for the second seed alone (scripts/tests/held-web.test.js); opened by the Rust test held_fixture_rust_and_the_website_agree.';
    fs.writeFileSync(webFx, JSON.stringify({ about, package: pkg, parts, rewrapped }, null, 2) + '\n');
  }
  const web = JSON.parse(fs.readFileSync(webFx, 'utf8'));
  for (const p of web.parts) {
    const s2 = Buffer.from(p.sealed_b64, 'base64');
    assert.strictEqual(hex(await H.openPart(m, owner.secretKey, H.partContext(web.package, p.part), s2)), p.plain_hex, 'the checked-in web part ' + p.part + ' opens');
  }
  assert.strictEqual(web.rewrapped.length, FX.parts.length, 'every re-wrapped header is checked in');
  for (const r of web.rewrapped) {
    await H.checkHeader(m, successor.secretKey, H.partContext(FX.package, r.part), Buffer.from(r.header_b64, 'base64'), 1, 0);
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
  await assert.rejects(H.openPart(m, me.secretKey, ctx, sealed.subarray(0, H.headerLen(1) + piece)), 'cut at a piece\'s end');
  const body = sealed.subarray(H.headerLen(1));
  const swapped = Buffer.concat([sealed.subarray(0, H.headerLen(1)), body.subarray(piece, 2 * piece), body.subarray(0, piece), body.subarray(2 * piece)]);
  await assert.rejects(H.openPart(m, me.secretKey, ctx, swapped), 'pieces swapped');
  const flipped = Buffer.from(sealed);
  flipped[H.headerLen(1) + 10] ^= 1;
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

// ── The owner fixture, both ways (format v4; the second review) ──
//
// scripts/tests/fixtures/held-owner-v2.json is written by the Rust test
// `held_owner_fixture_rust_and_the_website_agree` (core held_seal): what a held part route's caller
// signs (each line starting with the domain line), and a recipient's acceptance of a change of two
// packages: every new header (re-wrapped for the owner and the successor), its hash, the round and
// the digest. Here: `callerPurpose` gives every line; each header checks out with `checkHeader` in
// the owner's slot and in the successor's, to the same hashes; `acceptanceDigest` is the same
// digest. Then what a recipient's device must catch.
// Seen red 2026-11-20, each break alone in a copy of held-seal.js (HOS_WEB_DIR): the digest's
// packages left unsorted: "the digest is the one the relay works out"; the round left out of the
// digest's text: the same; callerPurpose without its domain line: "the caller signs what the relay
// checks: part"; checkHeader opening any slot: "the owner's key in the successor's place is caught".
// All four re-run 2026-11-21 by mutation in the real file: red as stated (the last in the recipient's
// check test below).
const OWNER_FX = JSON.parse(fs.readFileSync(path.join(REPO, 'scripts', 'tests', 'fixtures', 'held-owner-v2.json'), 'utf8'));

test('the owner fixture: the caller lines, and the header check and digest Rust works out', async () => {
  const { m, H } = await load();
  for (const c of OWNER_FX.caller) assert.strictEqual(H.callerPurpose(c.route, c.grant, c.id, c.part, c.offset), c.purpose, 'the caller signs what the relay checks: ' + c.route);
  assert.ok(OWNER_FX.caller.every((c) => c.purpose.startsWith('hum/held-caller/v1\nheld_')), 'each with its domain line');
  assert.strictEqual(H.CALLER_KEY, 'x-hos-held-key');
  assert.strictEqual(H.CALLER_TIME, 'x-hos-held-time');
  assert.strictEqual(H.CALLER_SIG, 'x-hos-held-sig');
  const owner = kyberOf(m, new Uint8Array(32).fill(7));
  const successor = kyberOf(m, new Uint8Array(32).fill(6));
  const items = [];
  for (const p of OWNER_FX.packages) {
    const headers = p.headers_b64.map((h) => new Uint8Array(Buffer.from(h, 'base64')));
    const mine = await H.checkRewrappedPackage(m, owner.secretKey, p.package, headers.length, 2, 0, async (i) => headers[i]);
    const theirs = await H.checkRewrappedPackage(m, successor.secretKey, p.package, headers.length, 2, 1, async (i) => headers[i]);
    assert.deepStrictEqual(theirs, mine, 'both see the same headers');
    assert.deepStrictEqual(mine, OWNER_FX.hashes.find((h) => h.package === p.package).hashes, 'each header hash is the one Rust works out');
    items.push([p.package, mine]);
  }
  assert.strictEqual(H.acceptanceDigest(m, OWNER_FX.round, items), OWNER_FX.digest, 'the digest is the one the relay works out');
  assert.strictEqual(H.acceptanceDigest(m, OWNER_FX.round, items.slice().reverse()), OWNER_FX.digest, 'in any order');
  assert.notStrictEqual(H.acceptanceDigest(m, 'another round', items), OWNER_FX.digest, 'another round, another digest');
});

test("a recipient's check catches a garbage slot, the wrong count or place, another key and a body", async () => {
  const { m, H } = await load();
  const owner = kyberOf(m, new Uint8Array(32).fill(7));
  const successor = kyberOf(m, new Uint8Array(32).fill(6));
  const third = kyberOf(m, new Uint8Array(32).fill(5));
  const ctx = H.partContext('pkgchk', 1);
  const held = await H.sealPart(m, [owner.publicKey], ctx, enc.encode('the words'));
  const header = await H.rewrapHeader(m, owner.secretKey, ctx, held.subarray(0, H.headerLen(1)), [owner.publicKey, successor.publicKey]);
  await assert.doesNotReject(H.checkHeader(m, successor.secretKey, ctx, header, 2, 1), 'an honest re-wrap checks out');
  const garbled = header.slice();
  garbled[H.headerLen(1) + 40] ^= 1;
  await assert.rejects(H.checkHeader(m, successor.secretKey, ctx, garbled, 2, 1), /does not open for your key/, 'a garbage slot is caught');
  await assert.doesNotReject(H.checkHeader(m, owner.secretKey, ctx, garbled, 2, 0), "while the owner's own slot still checks");
  await assert.rejects(H.checkHeader(m, successor.secretKey, ctx, header, 1, 1), /names 1/, 'the wrong count is caught');
  await assert.rejects(H.checkHeader(m, owner.secretKey, ctx, header, 2, 1), /does not open for your key/, "the owner's key in the successor's place is caught");
  await assert.rejects(H.checkHeader(m, third.secretKey, ctx, header, 2, 1), /does not open for your key/, 'a third key is caught');
  const whole = Buffer.concat([header, held.subarray(H.headerLen(1))]);
  await assert.rejects(H.checkHeader(m, successor.secretKey, ctx, whole, 2, 1), /header alone/, 'a body where a header belongs is caught');
  // A slot wrapping another key under the real commitment: built by hand as wrapFor would, with a
  // fresh key's wrapping but the body's commitment in the header and the associated data.
  const commitment = H.parseHeader(held).commitment;
  const other = globalThis.crypto.getRandomValues(new Uint8Array(32));
  const { cipherText, sharedSecret } = m.ml_kem768.encapsulate(successor.publicKey);
  const wrapKey = m.blake3.create({ context: enc.encode(H.WRAP_DOMAIN), dkLen: 32 }).update(sharedSecret).digest();
  const k = await globalThis.crypto.subtle.importKey('raw', wrapKey, { name: 'AES-GCM' }, false, ['encrypt']);
  const wrapped = new Uint8Array(await globalThis.crypto.subtle.encrypt({ name: 'AES-GCM', iv: new Uint8Array(12), additionalData: enc.encode(H.wrapAad(ctx, 1, 0, commitment)) }, k, other));
  const forged = Buffer.concat([enc.encode(H.MAGIC), Uint8Array.of(1), commitment, cipherText, wrapped]);
  await assert.rejects(H.checkHeader(m, successor.secretKey, ctx, forged, 1, 0), /not the key its body is sealed under/, 'a slot wrapping another key under the real commitment is caught');
  await assert.rejects(H.openSlot(m, successor.secretKey, ctx, forged), /not the key its body is sealed under/, 'and the opening refuses it');
});
