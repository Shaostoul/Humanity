// The web app removes hidden location and camera information from a file before it leaves the
// device (web/shared/strip-metadata.js, 2026-10-11; docs/design/files-showcase.md, the
// operator's answers of 2026-10-11, answer 4), with the same rules and the same results as the
// relay's own strips, which are the specification:
//   src/relay/core/strip_metadata.rs        JPEG, PNG, WebP (GIF passed through)
//   src/relay/core/strip_video_metadata.rs  MP4, MOV, 3GP
//
// Run: node --test scripts/tests/strip-metadata.test.js
//
// Pure node, nothing booted or connected. Every file is built byte by byte here, mirroring the
// Rust tests' own builders (the video ones line for line, so the same movies come out: the
// "everything" movie with mdat first is 1492 bytes here as it is there). The last test is the
// strongest: where rustc is installed it compiles a tiny program around the relay's two real
// source files, strips several thousand inputs with it, and requires the JavaScript to give back
// the same bytes and the same report for every one (set HOS_STRIP_SKIP_RUST=1 to skip it).
//
// Each test was seen failing once against a deliberately broken copy of strip-metadata.js
// (HOS_WEB_DIR pointed at the broken copy), then passing on the real one; its comment says what
// the red run showed.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const REPO = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(REPO, "web");
const S = require(path.join(WEB, "shared", "strip-metadata.js"));

// ════════════════════════════════════════════════════════════════════════════
// Building bytes
// ════════════════════════════════════════════════════════════════════════════

/** Bytes from a string (one byte per character, so "\xA9xyz" is the Rust b"\xA9xyz"), an array or bytes. */
function bytes(x) {
  if (x instanceof Uint8Array) return x;
  if (typeof x === "string") return Uint8Array.from(x, (c) => c.charCodeAt(0) & 0xff);
  return Uint8Array.from(x);
}
function cat(...parts) {
  const bs = parts.map(bytes);
  const out = new Uint8Array(bs.reduce((n, b) => n + b.length, 0));
  let at = 0;
  for (const b of bs) { out.set(b, at); at += b.length; }
  return out;
}
const u16be = (n) => [(n >> 8) & 0xff, n & 0xff];
const u32be = (n) => [(n >>> 24) & 0xff, (n >>> 16) & 0xff, (n >>> 8) & 0xff, n & 0xff];
const u32le = (n) => [n & 0xff, (n >>> 8) & 0xff, (n >>> 16) & 0xff, (n >>> 24) & 0xff];
const u64be = (n) => { const big = BigInt(n); return [...u32be(Number(big >> 32n)), ...u32be(Number(big & 0xffffffffn))]; };
const zeros = (n) => new Uint8Array(n);

function contains(hay, needle) { return find(hay, needle) >= 0; }
function find(hay, needle, from = 0) {
  const n = bytes(needle);
  outer: for (let i = from; i + n.length <= hay.length; i++) {
    for (let k = 0; k < n.length; k++) if (hay[i + k] !== n[k]) continue outer;
    return i;
  }
  return -1;
}
function latin(b, s, e) { return String.fromCharCode(...b.subarray(s, e)); }
function same(a, b) { return a.length === b.length && a.every((x, i) => x === b[i]); }

// ════════════════════════════════════════════════════════════════════════════
// Video boxes: the Rust test's builders, line for line
// ════════════════════════════════════════════════════════════════════════════

function bx(name, payload) { const p = bytes(payload); return cat(u32be(p.length + 8), name, p); }
function bx64(name, payload) { const p = bytes(payload); return cat(u32be(1), name, u64be(p.length + 16), p); }
function bx0(name, payload) { return cat(u32be(0), name, payload); }
/** Filler bytes that are never ASCII letters, so no box name appears in them by accident. */
function filler(n, seed) { return Uint8Array.from({ length: n }, (_, i) => 0x80 | ((i * 7 + seed * 13) % 128)); }

const PLACE = "+37.3349-122.0090+030.000/";
const ftypQt = () => bx("ftyp", "qt  \x00\x00\x02\x00qt  ");
const ftypMp4 = () => bx("ftyp", "isom\x00\x00\x02\x00isomiso2avc1mp41");
const mvhd = () => bx("mvhd", filler(100, 1));
/** The frames, with a decoy "udta" box inside to prove mdat is never looked into. */
const frames = () => cat(filler(150, 2), bx("udta", "DECOY-inside-the-frames"), filler(150, 3));

function trak(offsets, extra) {
  const stco = bx("stco", cat([0, 0, 0, 0], u32be(2), u32be(offsets[0]), u32be(offsets[1])));
  const stbl = bx("stbl", cat(bx("stsd", filler(40, 4)), bx("stts", filler(16, 5)), bx("stsz", filler(20, 6)), stco));
  const minf = bx("minf", cat(bx("vmhd", filler(12, 7)), stbl));
  const mdia = bx("mdia", cat(bx("mdhd", filler(24, 8)), bx("hdlr", filler(25, 9)), minf));
  const edts = bx("edts", bx("elst", filler(20, 10)));
  const tkhd = bx("tkhd", filler(84, 11));
  return bx("trak", cat(tkhd, edts, mdia, ...extra));
}

/** Apple's QuickTime metadata: hdlr, keys, then an ilst of values, the location first. */
function appleMeta() {
  const hdlr = bx("hdlr", cat(zeros(8), "mdta", zeros(13)));
  const key = (k) => cat(u32be(k.length + 8), "mdta", k);
  const keys = bx("keys", cat(zeros(4), u32be(3), key("com.apple.quicktime.location.ISO6709"),
    key("com.apple.quicktime.make"), key("com.apple.quicktime.model")));
  const item = (i, text) => {
    const data = bx("data", cat(u32be(1), zeros(4), text));
    return cat(u32be(data.length + 8), u32be(i), data);
  };
  const ilst = bx("ilst", cat(item(1, PLACE), item(2, "Apple"), item(3, "iPhone 15 Pro")));
  return bx("meta", cat(hdlr, keys, ilst));
}

/** Android's "\xA9xyz" inside udta: text length, language, text. */
const androidUdta = () => bx("udta", bx("\xA9xyz", cat(u16be(PLACE.length), [0x15, 0xc7], PLACE)));

/** 3GPP's loci inside udta: a named place with its coordinates (16.16 fixed point). */
function lociUdta() {
  const fixed = (deg) => u32be(Math.trunc(deg * 65536) >>> 0);
  return bx("udta", bx("loci", cat(zeros(4), [0x15, 0xc7], "Grandma's house\0", [0],
    fixed(-122.009), fixed(37.3349), fixed(30.0), "Earth\0", "\0")));
}

const trackMeta = () => bx("meta", cat(zeros(4), bx("ilst", "TRACK-META-camera-serial-123")));
const xmpUuid = () => bx("uuid", cat(S.XMP_UUID,
  '<x:xmpmeta><rdf:Description exif:GPSLatitude="37,20.094N" exif:GPSLongitude="122,0.54W"/></x:xmpmeta>'));
const SPHERICAL_UUID = [0xff, 0xcc, 0x82, 0x63, 0xf8, 0x55, 0x4a, 0x93, 0x88, 0x14, 0x58, 0x7a, 0x02, 0x52, 0x1f, 0xdd];
const sphericalUuid = () => bx("uuid", cat(SPHERICAL_UUID, "<GSpherical:Spherical>true</GSpherical:Spherical>"));

/** A whole movie with real chunk offsets pointing into mdat ("moovFirst" or "mdatFirst"). */
function movie(layout, ftyp, ex = {}) {
  const top = ex.top || [], moovExtra = ex.moov || [], trakExtra = ex.trak || [];
  const moovWith = (offsets) => {
    const body = cat(mvhd(), trak(offsets, trakExtra), ...moovExtra);
    return ex.moovSize0 ? bx0("moov", body) : bx("moov", body);
  };
  const mdat = ex.mdat64 ? bx64("mdat", frames()) : ex.mdatSize0 ? bx0("mdat", frames()) : bx("mdat", frames());
  const mdatHeader = ex.mdat64 ? 16 : 8;
  const head = cat(ftyp, ...top);
  if (layout === "moovFirst") {
    const first = head.length + moovWith([0, 0]).length + mdatHeader;
    return cat(head, moovWith([first, first + 150]), mdat);
  }
  const wide = bx("wide", []);
  const first = head.length + wide.length + mdatHeader;
  return cat(head, wide, mdat, moovWith([first, first + 150]));
}

/** Every kind of metadata the video strip neutralises, in one movie. */
const everything = () => ({ top: [xmpUuid(), sphericalUuid()], moov: [appleMeta(), androidUdta()], trak: [lociUdta(), trackMeta()] });

// ---------- checking a video result (the Rust test's checks) ----------

/** The byte range of the first box with this name (names that appear once in the file only). */
function findBox(buf, name) {
  const at = find(buf, name, 4);
  assert.ok(at >= 4, `test setup: box ${name} present`);
  const start = at - 4;
  const size32 = (buf[start] * 0x1000000) + ((buf[start + 1] << 16) | (buf[start + 2] << 8) | buf[start + 3]);
  let size = size32;
  if (size32 === 0) size = buf.length - start;
  if (size32 === 1) size = Number(new DataView(buf.buffer, buf.byteOffset).getBigUint64(at + 4));
  return [start, start + size];
}

/** Same size, every reported box renamed free with only zeros inside, nothing else changed. */
function assertOnlyReportedBoxesChanged(before, after, r) {
  assert.strictEqual(after.length, before.length, "the size must never change");
  const inside = new Uint8Array(before.length);
  let zeroed = 0;
  for (const b of r.removed) {
    const s = b.offset, e = b.offset + b.size;
    assert.ok(e <= after.length, `${b.path} reported past the end`);
    assert.strictEqual(latin(after, s + 4, s + 8), "free", `${b.path} must be renamed free`);
    const body = before[s] === 0 && before[s + 1] === 0 && before[s + 2] === 0 && before[s + 3] === 1 ? s + 16 : s + 8;
    assert.ok(same(after.subarray(s, s + 4), before.subarray(s, s + 4)), `${b.path}: size field kept`);
    assert.ok(same(after.subarray(s + 8, body), before.subarray(s + 8, body)), `${b.path}: 64-bit size kept`);
    assert.ok(after.subarray(body, e).every((x) => x === 0), `${b.path} must hold only zeros`);
    zeroed += e - body;
    inside.fill(1, s, e);
  }
  assert.strictEqual(r.bytesZeroed, zeroed, "bytesZeroed must add up");
  for (let i = 0; i < before.length; i++) {
    if (before[i] !== after[i]) assert.ok(inside[i], `byte ${i} changed outside every reported box`);
  }
}

/** The boxes playback needs are byte-identical, and the chunk offsets still point at the same frames. */
function assertPlaybackBytesIdentical(before, after) {
  assert.strictEqual(after.length, before.length, "the size must never change");
  for (const name of ["ftyp", "mvhd", "tkhd", "edts", "mdia", "stco", "mdat"]) {
    const [s, e] = findBox(before, name);
    assert.ok(same(before.subarray(s, e), after.subarray(s, e)), `'${name}' must be byte-identical`);
  }
  const [stco] = findBox(before, "stco");
  const [ms, me] = findBox(before, "mdat");
  for (let i = 0; i < 2; i++) {
    const at = stco + 16 + 4 * i;
    const off = before[at] * 0x1000000 + ((before[at + 1] << 16) | (before[at + 2] << 8) | before[at + 3]);
    assert.ok(off >= ms && off < me, `test setup: chunk offset ${off} must point into mdat`);
    assert.ok(same(before.subarray(off, off + 16), after.subarray(off, off + 16)), `the frames at ${off} must not move`);
  }
}

const paths = (r) => r.removed.map((b) => b.path);

/** An ftyp box, then `n` empty `free` boxes (8 bytes each), then `tail`. */
function ftypThenFree(n, tail = []) {
  const head = ftypMp4();
  const t = bytes(tail);
  const out = new Uint8Array(head.length + 8 * n + t.length);
  out.set(head, 0);
  if (n > 0) {
    out.set([0, 0, 0, 8, 0x66, 0x72, 0x65, 0x65], head.length);
    for (let have = 1; have < n; have *= 2) out.copyWithin(head.length + 8 * have, head.length, head.length + 8 * Math.min(have, n - have));
  }
  out.set(t, head.length + 8 * n);
  return out;
}
const complete = (r) => r.kind === "mp4" && r.foundMoov && r.stoppedEarly.length === 0 && !r.unfinished;

// ════════════════════════════════════════════════════════════════════════════
// Pictures
// ════════════════════════════════════════════════════════════════════════════

/** One JPEG segment: FF, marker, length (counting itself), payload. */
function seg(marker, payload) { const p = bytes(payload); return cat([0xff, marker], u16be(p.length + 2), p); }

const JFIF = () => seg(0xe0, cat("JFIF\0", [1, 1, 0, 0, 1, 0, 1, 0, 0]));
const DQT = () => seg(0xdb, cat([0], filler(64, 20)));
const SOF0 = () => seg(0xc0, [8, 0, 8, 0, 8, 1, 1, 0x11, 0]);
const DHT = () => seg(0xc4, cat([0], [0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], [0]));
const ICC = () => seg(0xe2, cat("ICC_PROFILE\0", [1, 1], filler(40, 21)));
const ADOBE = () => seg(0xee, cat("Adobe", [0, 100, 0, 0, 0, 0, 1]));
/** Start of scan, then entropy data holding a decoy APP1 that must never be looked at. */
const SCAN = () => cat(seg(0xda, [1, 1, 0, 0, 63, 0]), filler(60, 22),
  [0xff, 0xe1, 0x00, 0x14], "Exif\0\0DECOY-AFTER-SOS", filler(30, 23), [0xff, 0xd9]);

/** A JPEG: SOI, JFIF, `head`, DQT, SOF0, DHT, `tail`, then the scan and EOI. */
function jpeg(head = [], tail = []) { return cat([0xff, 0xd8], JFIF(), ...head, DQT(), SOF0(), DHT(), ...tail, SCAN()); }

/** A big-endian TIFF (an Exif block) built from IFD0, Exif IFD and GPS IFD entries [tag, type, count, valueBytes]. */
function tiff({ ifd0 = [], exif = [], gps = [] }) {
  const ifdBytes = (entries, at) => {
    const dataStart = at + 2 + 12 * entries.length + 4;
    const head = [...u16be(entries.length)];
    let data = [];
    for (const [tag, type, count, value] of entries) {
      const v = [...bytes(value)];
      head.push(...u16be(tag), ...u16be(type), ...u32be(count));
      if (v.length <= 4) head.push(...v, ...new Array(4 - v.length).fill(0));
      else {
        head.push(...u32be(dataStart + data.length));
        data.push(...v);
        if (data.length & 1) data.push(0);
      }
    }
    head.push(0, 0, 0, 0);
    return [...head, ...data];
  };
  // Layout: header, GPS IFD, Exif IFD, IFD0 (so IFD0's pointers are known when it is built).
  const gpsAt = 8;
  const gpsB = gps.length ? ifdBytes(gps, gpsAt) : [];
  const exifAt = gpsAt + gpsB.length;
  const exifB = exif.length ? ifdBytes(exif, exifAt) : [];
  const ifd0At = exifAt + exifB.length;
  const pointers = [];
  if (exif.length) pointers.push([0x8769, 4, 1, u32be(exifAt)]);
  if (gps.length) pointers.push([0x8825, 4, 1, u32be(gpsAt)]);
  const ifd0B = ifdBytes([...ifd0, ...pointers], ifd0At);
  return cat("MM", [0, 42], u32be(ifd0At), gpsB, exifB, ifd0B);
}
const rational3 = (a, b, c) => cat(u32be(a[0]), u32be(a[1]), u32be(b[0]), u32be(b[1]), u32be(c[0]), u32be(c[1]));
/** An Exif block saying an Apple iPhone 15 Pro took it at 37 20' 5.64" N, 122 0' 3.24" W, with a serial number. */
function phoneExif() {
  return tiff({
    ifd0: [[0x010f, 2, 6, "Apple\0"], [0x0110, 2, 14, "iPhone 15 Pro\0"]],
    exif: [[0x9003, 2, 20, "2026:10:11 09:30:00\0"], [0xa431, 2, 15, "SN-0042-SECRET\0"]],
    gps: [[1, 2, 2, "N\0"], [2, 5, 3, rational3([37, 1], [20, 1], [564, 100])],
      [3, 2, 2, "W\0"], [4, 5, 3, rational3([122, 1], [0, 1], [324, 100])]],
  });
}
const exifApp1 = (t) => seg(0xe1, cat("Exif\0\0", t));
const xmpApp1 = (xml) => seg(0xe1, cat("http://ns.adobe.com/xap/1.0/\0", xml));
function iptcApp13() {
  const ds = (n, text) => cat([0x1c, 2, n], u16be(text.length), text);
  const iptc = cat(ds(90, "Springfield"), ds(120, "Our garden at home"));
  return seg(0xed, cat("Photoshop 3.0\0", "8BIM", [0x04, 0x04], [0, 0], u32be(iptc.length), iptc));
}

/** PNG chunks, with real CRCs. */
function crc32(b) {
  let c = 0xffffffff;
  for (const x of b) { c ^= x; for (let k = 0; k < 8; k++) c = c & 1 ? (c >>> 1) ^ 0xedb88320 : c >>> 1; }
  return (~c) >>> 0;
}
function chunk(name, payload) {
  const p = bytes(payload);
  const body = cat(name, p);
  return cat(u32be(p.length), body, u32be(crc32(body)));
}
const PNG_SIG = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const IHDR = () => chunk("IHDR", [0, 0, 0, 4, 0, 0, 0, 4, 8, 6, 0, 0, 0]);
const IDAT = () => chunk("IDAT", filler(30, 24));
const IEND = () => chunk("IEND", []);
function png(...middle) { return cat(PNG_SIG, IHDR(), ...middle, IDAT(), IEND()); }

/** A WebP container around the chunks given (each [name, payload], padded to even). */
function webpChunk(name, payload, pad = true) {
  const p = bytes(payload);
  return cat(name, u32le(p.length), p, pad && p.length & 1 ? [0] : []);
}
function webp(...chunks) { const body = cat(...chunks); return cat("RIFF", u32le(body.length + 4), "WEBP", body); }

// ════════════════════════════════════════════════════════════════════════════
// The video tests (strip_video_metadata.rs, mirrored)
// ════════════════════════════════════════════════════════════════════════════

// Seen red: with the `meta` arm for moov and trak removed from walkLevel, the paths came back []
// where ['moov/meta'] was expected.
test("Apple's location in moov/meta is gone", () => {
  const before = movie("mdatFirst", ftypQt(), { moov: [appleMeta()] });
  assert.ok(contains(before, "location.ISO6709") && contains(before, PLACE), "test setup");
  const r = S.stripFile(before, "IMG_0001.MOV");
  assert.deepStrictEqual(paths(r), ["moov/meta"]);
  assert.ok(!contains(r.bytes, "location.ISO6709"), "the location key must be gone");
  assert.ok(!contains(r.bytes, PLACE), "the coordinates must be gone");
  assert.ok(!contains(r.bytes, "iPhone 15 Pro"), "the model must be gone");
  assert.ok(complete(r) && r.changed && r.claimed === "mp4");
  assertOnlyReportedBoxesChanged(before, r.bytes, r);
  assertPlaybackBytesIdentical(before, r.bytes);
});

// Seen red: with the `udta` arm for moov removed, the paths came back [] where ['moov/udta'] was
// expected.
test("Android's \xA9xyz in moov/udta is gone", () => {
  const before = movie("moovFirst", ftypMp4(), { moov: [androidUdta()] });
  assert.ok(contains(before, "\xA9xyz") && contains(before, PLACE), "test setup");
  const r = S.stripFile(before, "video/mp4");
  assert.deepStrictEqual(paths(r), ["moov/udta"]);
  assert.ok(!contains(r.bytes, "\xA9xyz") && !contains(r.bytes, PLACE));
  assert.ok(complete(r));
  assertOnlyReportedBoxesChanged(before, r.bytes, r);
  assertPlaybackBytesIdentical(before, r.bytes);
});

// Seen red: with the walk no longer stepping into `trak`, the paths came back [] where
// ['moov/trak/udta'] was expected.
test("a track's udta location (3GPP loci) is gone", () => {
  const before = movie("moovFirst", ftypMp4(), { trak: [lociUdta()] });
  assert.ok(contains(before, "Grandma's house"), "test setup");
  const r = S.stripFile(before, "clip.3gp");
  assert.deepStrictEqual(paths(r), ["moov/trak/udta"]);
  assert.ok(!contains(r.bytes, "Grandma's house") && !contains(r.bytes, "loci"));
  assertOnlyReportedBoxesChanged(before, r.bytes, r);
  assertPlaybackBytesIdentical(before, r.bytes);
});

// Seen red: with isXmpUuid answering yes for every uuid box, the paths listed 'uuid(xmp)' twice:
// the 360-degree video box was zeroed too.
test("an XMP uuid is zeroed and another uuid is left alone", () => {
  const before = movie("mdatFirst", ftypMp4(), { top: [sphericalUuid(), xmpUuid()] });
  const r = S.stripFile(before, "video/mp4");
  assert.deepStrictEqual(paths(r), ["uuid(xmp)"]);
  assert.ok(!contains(r.bytes, "GPSLatitude"), "the XMP GPS must be gone");
  assert.ok(!contains(r.bytes, S.XMP_UUID), "the XMP id itself is zeroed with the rest");
  const sph = find(before, SPHERICAL_UUID) - 8;
  const len = sphericalUuid().length;
  assert.ok(same(before.subarray(sph, sph + len), r.bytes.subarray(sph, sph + len)), "the other uuid must be byte-identical");
  assertOnlyReportedBoxesChanged(before, r.bytes, r);
  assertPlaybackBytesIdentical(before, r.bytes);
});

for (const [layout, ftyp, name] of [["moovFirst", ftypMp4, "moov before mdat"], ["mdatFirst", ftypQt, "mdat before moov"]]) {
  // Seen red: with the zeros filled 16 bytes past each box's end, both failed "the 360 uuid
  // stays": the XMP box's overrun wiped the start of the 360-degree uuid box right after it (the
  // Rust test's own red; in JavaScript a fill past the end of the array is clipped, so the file
  // cannot grow the way the Rust's did).
  test(`${name} keeps every playback byte`, () => {
    const before = movie(layout, ftyp(), everything());
    if (layout === "mdatFirst") assert.strictEqual(before.length, 1492, "test setup: the same movie as the Rust test's");
    const r = S.stripFile(before, "video/mp4");
    assert.deepStrictEqual(paths(r), ["uuid(xmp)", "moov/trak/udta", "moov/trak/meta", "moov/meta", "moov/udta"]);
    for (const gone of ["location.ISO6709", PLACE, "Grandma's house", "GPSLatitude", "TRACK-META"]) {
      assert.ok(!contains(r.bytes, gone), `${gone} must be gone`);
    }
    assert.ok(contains(r.bytes, SPHERICAL_UUID), "the 360 uuid stays");
    assert.ok(contains(r.bytes, "DECOY-inside-the-frames"), "mdat is never read or written");
    assert.ok(complete(r));
    assertOnlyReportedBoxesChanged(before, r.bytes, r);
    assertPlaybackBytesIdentical(before, r.bytes);
  });
}

// Seen red: with a 64-bit box's end computed 8 bytes too far, the walk landed inside moov's header
// and the paths came back [] where ['moov/udta'] was expected.
test("a 64-bit mdat is stepped over", () => {
  const before = movie("mdatFirst", ftypQt(), { moov: [androidUdta()], mdat64: true });
  const [ms] = findBox(before, "mdat");
  assert.deepStrictEqual([...before.subarray(ms, ms + 4)], [0, 0, 0, 1], "test setup: 64-bit size");
  const r = S.stripFile(before, "video/quicktime");
  assert.deepStrictEqual(paths(r), ["moov/udta"]);
  assert.ok(!contains(r.bytes, PLACE));
  assert.ok(contains(r.bytes, "DECOY-inside-the-frames"), "the decoy inside mdat is never touched");
  assert.ok(complete(r));
  assertOnlyReportedBoxesChanged(before, r.bytes, r);
  assertPlaybackBytesIdentical(before, r.bytes);
});

// Seen red: with size 0 treated as a broken size, the mdat running to the end of the file was
// reported as broken ("in the top level of the file, the box 'mdat' at byte 563 claims a size of
// 0, smaller than its own header": byte 563, as in the Rust test's red note).
test("a size-0 last box runs to the end of the file", () => {
  let before = movie("moovFirst", ftypMp4(), { moov: [androidUdta()], mdatSize0: true });
  let r = S.stripFile(before, "video/mp4");
  assert.deepStrictEqual(paths(r), ["moov/udta"]);
  assert.ok(complete(r), JSON.stringify(r.stoppedEarly));
  assertOnlyReportedBoxesChanged(before, r.bytes, r);
  assertPlaybackBytesIdentical(before, r.bytes);

  // moov last with size 0: its children are walked up to the end.
  before = movie("mdatFirst", ftypQt(), { moov: [androidUdta()], moovSize0: true });
  r = S.stripFile(before, "video/mp4");
  assert.deepStrictEqual(paths(r), ["moov/udta"]);
  assert.ok(complete(r), JSON.stringify(r.stoppedEarly));
  assert.ok(!contains(r.bytes, PLACE));
  assertOnlyReportedBoxesChanged(before, r.bytes, r);
  assertPlaybackBytesIdentical(before, r.bytes);

  // An XMP uuid last with size 0: zeroed right up to the end.
  before = cat(movie("moovFirst", ftypMp4()), bx0("uuid", xmpUuid().subarray(8)));
  r = S.stripFile(before, "video/mp4");
  assert.deepStrictEqual(paths(r), ["uuid(xmp)"]);
  assert.strictEqual(r.removed[0].offset + r.removed[0].size, before.length);
  assert.ok(!contains(r.bytes, "GPSLatitude"));
  assertOnlyReportedBoxesChanged(before, r.bytes, r);
});

// Seen red: with both boxHeader's "claims more than is left" check and walkMp4's second lock
// removed (the same two the Rust test's red removed), a cut-off file was walked as if it were
// whole and "uuid(xmp) reported past the end" failed.
test("truncated files never throw and never write outside the file", () => {
  for (const layout of ["moovFirst", "mdatFirst"]) {
    const full = movie(layout, ftypMp4(), everything());
    for (let cut = 0; cut <= full.length; cut++) {
      const before = full.subarray(0, cut);
      const r = S.stripFile(before, "video/mp4");
      assertOnlyReportedBoxesChanged(before, r.bytes, r);
    }
    assert.ok(complete(S.stripFile(full, "video/mp4")), "the whole file is clean and complete");
  }
  // A cut inside moov (mdat first): moov claims more than is left, so it is not walked, nothing
  // in it is written, and the report says so.
  const full = movie("mdatFirst", ftypMp4(), everything());
  const [moovStart] = findBox(full, "moov");
  const cut = full.subarray(0, moovStart + 40);
  const r = S.stripFile(cut, "video/mp4");
  assert.ok(same(r.bytes.subarray(moovStart), cut.subarray(moovStart)), "nothing in the cut-off moov may be written");
  assertOnlyReportedBoxesChanged(cut, r.bytes, r);
  assert.ok(!r.foundMoov && !complete(r));
  assert.deepStrictEqual(paths(r), ["uuid(xmp)"], "only the XMP before the cut is reached");
  assert.strictEqual(r.stoppedEarly.length, 1, JSON.stringify(r.stoppedEarly));
  assert.ok(r.unfinished, "the sender is asked before it goes");
  assert.strictEqual(r.untouchedReason, S.SAY.noMovie);
});

// Seen red: with one bad size stopping the whole walk (a flag that ended every level, not just
// its own), the paths held only ['moov/udta']: the XMP box after moov was no longer neutralised.
test("a box running past its parent stops only that level", () => {
  // Inside moov: a good udta, then a trak claiming 5000 bytes, then an Apple meta out of reach.
  const badTrak = cat(u32be(5000), "trak", filler(40, 12));
  let before = cat(ftypMp4(), bx("moov", cat(mvhd(), androidUdta(), badTrak, appleMeta())), xmpUuid(), bx("mdat", frames()));
  let r = S.stripFile(before, "video/mp4");
  assert.deepStrictEqual(paths(r), ["moov/udta", "uuid(xmp)"], "before the bad box, and the next level up, still done");
  assert.strictEqual(r.stoppedEarly.length, 1);
  assert.ok(r.stoppedEarly[0].includes("in moov") && r.stoppedEarly[0].includes("'trak'"), r.stoppedEarly[0]);
  assert.ok(!complete(r) && r.unfinished);
  assert.strictEqual(r.untouchedReason, S.SAY.videoUnreadable);
  // The honest limit: after a broken box nobody can know where the next one starts.
  assert.ok(contains(r.bytes, "location.ISO6709"));
  assertOnlyReportedBoxesChanged(before, r.bytes, r);
  const [ms, me] = findBox(before, "mdat");
  assert.ok(same(before.subarray(ms, me), r.bytes.subarray(ms, me)));

  // A size under 8 inside a track stops that track only.
  const tiny = cat(u32be(4), "udta", filler(20, 13));
  const trk = bx("trak", cat(bx("tkhd", filler(84, 14)), tiny));
  before = cat(ftypMp4(), bx("moov", cat(mvhd(), trk, androidUdta())), bx("mdat", frames()));
  r = S.stripFile(before, "video/mp4");
  assert.deepStrictEqual(paths(r), ["moov/udta"], "the moov level carries on after the broken track");
  assert.deepStrictEqual(r.stoppedEarly, [
    "in a moov/trak, the box 'udta' at byte 248 claims a size of 4, smaller than its own header"]);
  assertOnlyReportedBoxesChanged(before, r.bytes, r);
});

// Seen red: with sniffKind no longer recognising the EBML magic, the kind came back 'other' where
// 'webm' was expected.
test("WebM and plain text are left untouched", () => {
  const webm = cat([0x1a, 0x45, 0xdf, 0xa3, 0x9f, 0x42, 0x86, 0x81, 0x01], "\x42\x82\x84webm",
    bx("moov", androidUdta()), xmpUuid());
  let r = S.stripFile(webm, "clip.webm");
  assert.strictEqual(r.kind, "webm");
  assert.strictEqual(r.bytes, webm, "the very array given back, untouched");
  assert.ok(!r.changed && !r.unfinished);
  assert.strictEqual(r.untouchedReason, S.SAY.webm);
  r = S.stripFile(bytes("just some text, not a video"), "notes.txt");
  assert.strictEqual(r.kind, "other");
  assert.ok(!r.changed && !r.unfinished && r.untouchedReason === S.SAY.other);
});

// Seen red: with the "only in a file with a movie" condition removed, the HEIC picture's own meta
// box was zeroed (paths ['meta'] instead of []).
test("a HEIC still picture keeps its top-level meta, and says it cannot be cleaned yet", () => {
  const meta = bx("meta", cat(zeros(4), bx("hdlr", filler(25, 15)), bx("pitm", [0, 0, 0, 0, 0, 1])));
  let before = cat(bx("ftyp", "heic\0\0\0\0mif1heic"), meta, bx("mdat", frames()));
  let r = S.stripFile(before, "IMG_0002.HEIC");
  assert.strictEqual(r.kind, "heif");
  assert.deepStrictEqual(paths(r), []);
  assert.ok(!r.foundMoov && !r.changed && r.bytes === before);
  assert.ok(r.unfinished);
  assert.strictEqual(r.untouchedReason, S.SAY.heif);

  // In a movie, the same top-level meta is neutralised.
  before = cat(movie("moovFirst", ftypMp4()), bx("meta", "\0\0\0\0TOP-META"));
  r = S.stripFile(before, "video/mp4");
  assert.deepStrictEqual(paths(r), ["meta"]);
  assert.ok(!contains(r.bytes, "TOP-META"));
});

// Seen red: with the size check dropped from isMp4Family, an ftyp claiming 4 bytes counted as an
// MP4 ("size smaller than a header" failed).
test("isMp4Family reads the first box", () => {
  assert.ok(S.isMp4Family(ftypMp4()));
  assert.ok(S.isMp4Family(ftypQt().subarray(0, 8)), "eight bytes are enough");
  assert.ok(S.isMp4Family(bx64("ftyp", "isom\0\0\0\0")), "a 64-bit size is allowed");
  assert.ok(!S.isMp4Family(bytes([0x1a, 0x45, 0xdf, 0xa3, 0x9f, 0x42, 0x86, 0x81])), "WebM");
  assert.ok(!S.isMp4Family(bytes("\xFF\xD8\xFF\xE0\x00\x10JFIF")), "JPEG");
  assert.ok(!S.isMp4Family(ftypMp4().subarray(0, 7)), "too short to tell");
  assert.ok(!S.isMp4Family(bytes("\0\0\0\x04ftypisom")), "size smaller than a header");
  assert.ok(!S.isMp4Family(bytes("\0\0\0\0ftypisom")), "size 0: the whole file is ftyp");
  assert.ok(!S.isMp4Family(bx("moov", mvhd())), "no ftyp first");
});

// Seen red: with `free` boxes planned like `udta` ones, the second run reported the four boxes
// the first run had made inside moov and its track (moov/trak/free twice, moov/free twice), the
// Rust test's own red.
test("running twice changes nothing more", () => {
  for (const layout of ["moovFirst", "mdatFirst"]) {
    const once = S.stripFile(movie(layout, ftypMp4(), everything()), "video/mp4");
    assert.strictEqual(once.removed.length, 5);
    const twice = S.stripFile(once.bytes, "video/mp4");
    assert.ok(!twice.changed && twice.bytesZeroed === 0, JSON.stringify(paths(twice)));
    assert.ok(complete(twice));
    assert.strictEqual(twice.bytes, once.bytes);
  }
});

// Seen red: with neutralise copying by `b.slice()` (which, for a Node Buffer, shares the caller's
// memory), "the caller's buffer is never written" failed for the Buffer.
test("the caller's buffer is never written, whatever it is passed as", () => {
  const original = movie("moovFirst", ftypMp4(), everything());
  for (const make of [(b) => Buffer.from(b), (b) => cat(b), (b) => cat(b).buffer]) {
    const given = make(original);
    const r = S.stripFile(given, "video/mp4");
    const view = given instanceof ArrayBuffer ? new Uint8Array(given) : given;
    assert.ok(same(view, original), "the caller's buffer is never written");
    assert.ok(r.changed && r.bytes !== given && r.bytes.length === original.length);
  }
  const pic = jpeg([exifApp1(phoneExif())]);
  const given = Buffer.from(pic);
  const r = S.stripFile(given, "image/jpeg");
  assert.ok(r.changed && same(given, pic), "a picture's input is never written either");
});

// Seen red: with the cap lowered to 1000 in walkLevel's check, the walk over 5000 empty boxes
// stopped before the XMP box after them: paths [] where ['uuid(xmp)'] was expected.
test("a file of more than a million boxes stops at the cap the Rust uses", () => {
  const many = ftypThenFree(S.MAX_BOXES + 10, xmpUuid());
  const r = S.stripFile(many, "video/mp4");
  assert.deepStrictEqual(r.stoppedEarly, ["stopped after reading 1000000 box headers, more than any real video has"]);
  assert.ok(!r.changed && r.unfinished);
  // Fewer than the cap: the walk reaches an XMP box after them.
  assert.deepStrictEqual(paths(S.stripFile(ftypThenFree(5000, xmpUuid()), "video/mp4")), ["uuid(xmp)"]);
});

// ════════════════════════════════════════════════════════════════════════════
// The picture tests (strip_metadata.rs, mirrored and widened)
// ════════════════════════════════════════════════════════════════════════════

// Seen red: with APP1 (0xE1) left out of the dropped markers, "Exif must be gone" failed.
test("JPEG: Exif, XMP, IPTC and comments are removed, every other segment kept in order", () => {
  // The Rust test's own check: a fake Exif APP1 spliced right after SOI goes.
  const fake = seg(0xe1, "Exif\0\0FAKE-GPS-COORDINATES-12.34,56.78");
  const plain = jpeg();
  const tagged = cat(plain.subarray(0, 2), fake, plain.subarray(2));
  let r = S.stripFile(tagged, "image/jpeg");
  assert.ok(!contains(r.bytes, "GPS-COORDINATES"), "Exif must be gone");
  assert.ok(r.bytes.length < tagged.length);
  assert.ok(same(r.bytes, plain), "the rest is the original file, byte for byte");

  // Everything at once: the result is exactly the file built without those segments.
  const before = jpeg([exifApp1(phoneExif()), ICC(), xmpApp1('<x:xmpmeta exif:GPSLatitude="1,2N"/>'), ADOBE()],
    [iptcApp13(), seg(0xfe, "Shot at 12 Elm Street")]);
  r = S.stripFile(before, "photo.jpg");
  assert.deepStrictEqual(paths(r), ["APP1 Exif", "APP1 XMP", "APP13", "COM"]);
  assert.ok(same(r.bytes, jpeg([ICC(), ADOBE()])), "only the four segments are gone, the colour ones stay");
  assert.ok(contains(r.bytes, "DECOY-AFTER-SOS"), "nothing after the start of scan is ever walked");
  assert.ok(r.changed && !r.unfinished && r.untouchedReason === null);
  for (const gone of ["iPhone", "SN-0042", "Springfield", "Elm Street", "GPSLatitude"]) assert.ok(!contains(r.bytes, gone), gone);
});

// Seen red: with "tIME" left out of the dropped chunk names, the paths lacked 'tIME'.
test("PNG: text, Exif and time chunks are removed, the rest kept, bytes after IEND dropped", () => {
  const before = png(chunk("tEXt", "Author\0secret location: my house"), chunk("eXIf", phoneExif()),
    chunk("iTXt", "XML:com.adobe.xmp\0\0\0\0\0<x:xmpmeta tiff:Model=\"Pixel 9\"/>"),
    chunk("tIME", [7, 234, 10, 11, 9, 30, 0]), chunk("zTXt", "Comment\0\0compressed"), chunk("gAMA", [0, 0, 0xb1, 0x8f]));
  const r = S.stripFile(cat(before, "TRAILING"), "image/png");
  assert.deepStrictEqual(paths(r), ["tEXt", "eXIf", "iTXt", "tIME", "zTXt", "bytes after IEND"]);
  assert.ok(same(r.bytes, png(chunk("gAMA", [0, 0, 0xb1, 0x8f]))));
  assert.ok(!contains(r.bytes, "secret location"), "tEXt must be gone");
});

// Seen red: with the VP8X flag clearing skipped, "EXIF flag must be cleared" failed (the flag
// bits still held 12: EXIF and XMP both set).
test("WebP: EXIF and XMP chunks are removed, their flags cleared, the RIFF size rewritten", () => {
  // The Rust test's hand-built container, plus an odd-length XMP chunk and an ICC chunk.
  const before = webp(
    webpChunk("VP8X", [0b0011_1100, 0, 0, 0, 3, 0, 0, 3, 0, 0]), // ICC, alpha, EXIF and XMP flags, 4x4
    webpChunk("ICCP", filler(20, 25)),
    webpChunk("VP8L", [0x2f, 0, 0, 0]),
    webpChunk("EXIF", "GPS 12.34,56.78 here"),
    webpChunk("XMP ", "<x:xmpmeta odd/>"));
  const r = S.stripFile(before, "image/webp");
  assert.deepStrictEqual(paths(r), ["EXIF", "XMP"]);
  assert.ok(!contains(r.bytes, "GPS 12.34") && !contains(r.bytes, "xmpmeta"), "EXIF and XMP chunks must be gone");
  assert.ok(contains(r.bytes, "VP8L") && contains(r.bytes, "ICCP"), "image and colour chunks survive");
  const vp8x = find(r.bytes, "VP8X");
  assert.strictEqual(r.bytes[vp8x + 8] & 0b1100, 0, "EXIF flag must be cleared");
  assert.strictEqual(r.bytes[vp8x + 8], 0b0011_0000, "the ICC and alpha flags are kept");
  const riff = r.bytes[4] | (r.bytes[5] << 8) | (r.bytes[6] << 16) | (r.bytes[7] << 24);
  assert.strictEqual(riff + 8, r.bytes.length, "the RIFF size matches the rewritten file");
});

// Seen red: with stripFile always returning its rebuilt copy, "the very array given back" failed
// for the clean JPEG (an equal copy, not the same array).
test("clean pictures come back as the very array given, unchanged", () => {
  for (const [file, type] of [[jpeg([ICC()]), "image/jpeg"], [png(), "image/png"],
    [webp(webpChunk("VP8X", [0x10, 0, 0, 0, 3, 0, 0, 3, 0, 0]), webpChunk("VP8L", [0x2f, 0, 0, 0])), "image/webp"]]) {
    const r = S.stripFile(file, type);
    assert.strictEqual(r.bytes, file, `${type}: the very array given back`);
    assert.ok(!r.changed && r.removed.length === 0 && !r.unfinished && r.untouchedReason === null, type);
  }
});

// Seen red: with rebuildPicture ignoring the walk's trouble, the broken JPEG came back as the 20
// bytes walked before the trouble instead of its original 304 ("the original bytes, not a
// cut-short rewrite").
test("a broken picture is given back unchanged, and the report says so (fail open, as the Rust)", () => {
  // The Rust test's garbage.
  for (const [junk, type] of [[[0xff, 0xd8, 0x00, 0x01, 0x02], "image/jpeg"], ["not a png", "image/png"], ["RIFFxxxx", "image/webp"]]) {
    const b = bytes(junk);
    const r = S.stripFile(b, type);
    assert.strictEqual(r.bytes, b, `${type}: unchanged`);
    assert.ok(!r.changed && r.unfinished && r.untouchedReason, type);
  }
  // A JPEG with Exif, then a segment whose length runs past the end of the file.
  const broken = cat([0xff, 0xd8], JFIF(), exifApp1(phoneExif()), [0xff, 0xdb, 0x40, 0x00], filler(20, 26));
  let r = S.stripFile(broken, "image/jpeg");
  assert.strictEqual(r.bytes, broken, "the original bytes, not a cut-short rewrite");
  assert.ok(!r.changed && r.unfinished && r.untouchedReason === S.SAY.pictureUnreadable);
  // A PNG whose chunk claims more than is left.
  const brokenPng = cat(PNG_SIG, IHDR(), chunk("eXIf", phoneExif()), u32be(9999), "IDAT", filler(10, 27));
  r = S.stripFile(brokenPng, "image/png");
  assert.ok(r.bytes === brokenPng && r.unfinished);
  // A WebP whose last chunk claims more than is left (and is not the unpadded-last-chunk case).
  const brokenWebp = cat("RIFF", u32le(40), "WEBP", webpChunk("EXIF", "GPS"), "VP8L", u32le(500), filler(8, 28));
  r = S.stripFile(brokenWebp, "image/webp");
  assert.ok(r.bytes === brokenWebp && r.unfinished);
  // A file named as a JPEG whose bytes are not one.
  r = S.stripFile(bytes("plain text"), "holiday.jpg");
  assert.ok(r.unfinished && r.claimed === "jpeg" && r.kind === "other" && r.untouchedReason.includes("JPEG"));
});

// Seen red: with GIF sent through the JPEG walker, the GIF came back "unfinished" with the
// unreadable-picture sentence.
test("GIF is passed through untouched, as the relay does", () => {
  const gif = cat("GIF89a", [4, 0, 4, 0, 0, 0, 0], [0x21, 0xfe, 5], "hello", [0, 0x3b]);
  const r = S.stripFile(gif, "anim.gif");
  assert.strictEqual(r.bytes, gif);
  assert.ok(r.kind === "gif" && !r.changed && !r.unfinished && r.untouchedReason === S.SAY.gif);
});

// Seen red: with stripFile dispatching on the claimed type instead of the bytes, a JPEG named
// .png went to the PNG walker, which refused it, and the kind check failed.
test("the bytes decide the kind; the name is reported beside it", () => {
  const pic = jpeg([exifApp1(phoneExif())]);
  const r = S.stripFile(pic, { name: "looks-like.png", type: "" });
  assert.ok(r.kind === "jpeg" && r.claimed === "png" && r.changed && !contains(r.bytes, "iPhone"));
  assert.strictEqual(S.claimedKind("image/jpeg; charset=binary"), "jpeg");
  assert.strictEqual(S.claimedKind("IMG_0001.MOV"), "mp4");
  assert.strictEqual(S.claimedKind({ type: "image/heic", name: "x.jpg" }), "heif");
  assert.strictEqual(S.claimedKind("notes.txt"), null);
});

// Seen red: with the browser branch writing `root.HosStripX`, "HosStrip lands on window" failed.
test("loaded as a classic browser script, HosStrip lands on window and takes bytes from another realm", () => {
  const vm = require("node:vm");
  // A bare page: no `module`, its own Uint8Array, and no TextDecoder (the fallback decoder runs).
  const window = {};
  window.window = window;
  vm.createContext(window);
  vm.runInContext(fs.readFileSync(path.join(WEB, "shared", "strip-metadata.js"), "utf8"), window);
  assert.strictEqual(typeof window.HosStrip, "object", "HosStrip lands on window");
  const pic = jpeg([exifApp1(phoneExif())]);
  const r = window.HosStrip.stripFile(pic, "photo.jpg");
  assert.ok(r.changed && same(r.bytes, jpeg()), "a Uint8Array from this realm is read and cleaned");
  const d = window.HosStrip.describe(movie("mdatFirst", ftypQt(), { moov: [appleMeta()] }).buffer, "clip.mov");
  assert.strictEqual(d.summary, "This video says where it was filmed and which camera filmed it.");
});

// ════════════════════════════════════════════════════════════════════════════
// describe
// ════════════════════════════════════════════════════════════════════════════

// Seen red: with the GPS IFD pointer (0x8825) not followed, the details lacked 'location'.
test("describe: a phone photo says where and with which camera, in plain words, no numbers", () => {
  const pic = jpeg([exifApp1(phoneExif())], [iptcApp13(), seg(0xfe, "Shot at 12 Elm Street")]);
  const d = S.describe(pic, "photo.jpg");
  assert.ok(d.hasLocation && d.hasCamera && d.complete);
  assert.strictEqual(d.summary, "This photo says where it was taken and which camera took it.");
  assert.deepStrictEqual(d.details.map((x) => x.key), ["location", "place-name", "camera", "serial", "time", "comment"]);
  assert.strictEqual(d.details.find((x) => x.key === "camera").words, "Which camera or phone took it (Apple iPhone 15 Pro)");
  assert.strictEqual(d.details[0].words, "Where it was taken");
  assert.ok(!("coordinates" in d) && !JSON.stringify(d).includes("37.3") && !JSON.stringify(d).includes("Springfield"),
    "no coordinates or place unless asked");
  const asked = S.describe(pic, "photo.jpg", { coordinates: true });
  assert.ok(Math.abs(asked.coordinates.latitude - 37.3349) < 1e-4, JSON.stringify(asked.coordinates));
  assert.ok(Math.abs(asked.coordinates.longitude + 122.0009) < 1e-4, JSON.stringify(asked.coordinates));
  assert.strictEqual(asked.place, "Springfield");
  assert.ok(same(pic, jpeg([exifApp1(phoneExif())], [iptcApp13(), seg(0xfe, "Shot at 12 Elm Street")])), "read only");
});

// Seen red: with Apple's keys list ignored (every ilst item read as an unknown name), the Apple
// video described only "Other hidden details" and no location.
test("describe: videos from an iPhone, an Android phone and a 3GPP camera", () => {
  let d = S.describe(movie("mdatFirst", ftypQt(), { moov: [appleMeta()] }), "IMG_0003.MOV", { coordinates: true });
  assert.strictEqual(d.summary, "This video says where it was filmed and which camera filmed it.");
  assert.deepStrictEqual(d.details.map((x) => x.words), ["Where it was filmed", "Which camera or phone filmed it (Apple iPhone 15 Pro)"]);
  assert.deepStrictEqual(d.coordinates, { latitude: 37.3349, longitude: -122.009 });

  d = S.describe(movie("moovFirst", ftypMp4(), { moov: [androidUdta()] }), "video/mp4", { coordinates: true });
  assert.ok(d.hasLocation && !d.hasCamera);
  assert.strictEqual(d.summary, "This video says where it was filmed.");
  assert.deepStrictEqual(d.coordinates, { latitude: 37.3349, longitude: -122.009 });

  d = S.describe(movie("moovFirst", ftypMp4(), { trak: [lociUdta()] }), "video/3gpp", { coordinates: true });
  assert.deepStrictEqual(d.details.map((x) => x.key), ["location", "place-name"]);
  assert.strictEqual(d.place, "Grandma's house");
  assert.ok(Math.abs(d.coordinates.latitude - 37.3349) < 1e-4 && Math.abs(d.coordinates.longitude + 122.009) < 1e-4);

  d = S.describe(movie("moovFirst", ftypMp4(), { top: [xmpUuid()] }), "video/mp4", { coordinates: true });
  assert.ok(d.hasLocation && Math.abs(d.coordinates.latitude - (37 + 20.094 / 60)) < 1e-9 && d.coordinates.longitude < -122);
});

// Seen red: with the eXIf chunk sent to the XMP scanner instead of the Exif reader, the PNG
// described only 'other' (no location, camera, serial or time).
test("describe: PNG and WebP", () => {
  let d = S.describe(png(chunk("eXIf", phoneExif()), chunk("tEXt", "Author\0Ann"), chunk("tIME", [7, 234, 10, 11, 9, 30, 0])), "image/png");
  assert.deepStrictEqual(d.details.map((x) => x.key), ["location", "camera", "serial", "time", "changed-time", "person"]);
  // The Rust test's WebP: its EXIF chunk is not a real Exif block, so all it can say is "other".
  d = S.describe(webp(webpChunk("VP8X", [8, 0, 0, 0, 3, 0, 0, 3, 0, 0]), webpChunk("VP8L", [0x2f, 0, 0, 0]),
    webpChunk("EXIF", "GPS 12.34,56.78 here")), "image/webp");
  assert.ok(!d.hasLocation && !d.hasCamera);
  assert.strictEqual(d.summary, "This photo carries other hidden details.");
  d = S.describe(webp(webpChunk("VP8L", [0x2f, 0, 0, 0]), webpChunk("EXIF", cat("Exif\0\0", phoneExif()))), "image/webp");
  assert.ok(d.hasLocation && d.hasCamera);
});

// Seen red: with describe also reading the kept JFIF segment, the cleaned JPEG still described
// "Other hidden details".
test("describe finds nothing in what stripFile gives back", () => {
  const files = [
    jpeg([exifApp1(phoneExif()), ICC()], [iptcApp13(), seg(0xfe, "note")]),
    png(chunk("eXIf", phoneExif()), chunk("tEXt", "Author\0Ann")),
    webp(webpChunk("VP8X", [0x0c, 0, 0, 0, 3, 0, 0, 3, 0, 0]), webpChunk("VP8L", [0x2f, 0, 0, 0]), webpChunk("EXIF", phoneExif())),
    movie("moovFirst", ftypMp4(), everything()), movie("mdatFirst", ftypQt(), everything()),
  ];
  for (const f of files) {
    assert.ok(S.describe(f).details.length > 0, "test setup: something to find");
    const after = S.stripFile(f).bytes;
    const d = S.describe(after);
    assert.deepStrictEqual(d.details, [], `${S.sniffKind(f)}: ${JSON.stringify(d.details)}`);
    assert.strictEqual(d.summary, null);
  }
  assert.strictEqual(S.describe(jpeg()).summary, null, "a clean photo says nothing");
});

// ════════════════════════════════════════════════════════════════════════════
// Random inputs (fixed seed)
// ════════════════════════════════════════════════════════════════════════════

/** mulberry32: a small fixed-seed generator, so the "random" files are the same on every run. */
function rng(seed) {
  let a = seed >>> 0;
  const r = {
    next() {
      a = (a + 0x6d2b79f5) >>> 0;
      let t = a;
      t = Math.imul(t ^ (t >>> 15), t | 1);
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
      return (t ^ (t >>> 14)) >>> 0;
    },
    below(n) { return n ? r.next() % n : 0; },
    bytes(n) { return Uint8Array.from({ length: n }, () => r.next() & 0xff); },
    pick(list) { return list[r.below(list.length)]; },
  };
  return r;
}

const BOX_NAMES = ["moov", "trak", "udta", "meta", "uuid", "mdat", "free", "mvhd", "tkhd", "ftyp", "mdia", "\xA9xyz"];

/** The Rust test's random_boxes: nested up to three deep, sizes usually right, sometimes 0, 64-bit or wrong. */
function randomBoxes(g, depth) {
  const out = [];
  for (let k = g.below(5); k > 0; k--) {
    const name = g.below(8) === 0 ? g.bytes(4) : bytes(g.pick(BOX_NAMES));
    const nm = latin(name, 0, 4);
    let payload;
    if (depth < 3 && ["moov", "trak", "udta", "meta"].includes(nm) && g.below(4) !== 0) payload = randomBoxes(g, depth + 1);
    else if (nm === "uuid" && g.below(2) === 0) payload = cat(S.XMP_UUID, g.bytes(g.below(20)));
    else payload = g.bytes(g.below(40));
    const how = g.below(10);
    if (how === 0) out.push(bx0(name, payload));
    else if (how === 1) {
      const large = g.below(2) === 0 ? BigInt(payload.length + 16) : ((BigInt(g.next()) << 32n) | BigInt(g.next())) >> BigInt(g.below(64));
      out.push(cat(u32be(1), name, u64be(large), payload));
    } else if (how === 2) {
      const wrong = g.below(2) === 0 ? g.below(80) : g.next();
      out.push(cat(u32be(wrong), name, payload));
    } else out.push(bx(name, payload));
  }
  return cat(...out);
}

/** The Rust test's random_file: noise, a random box tree, or a real movie damaged and maybe cut. */
function randomVideo(g, i) {
  const which = g.below(4);
  if (which === 0) { const noise = g.bytes(g.below(200)); return g.below(2) === 0 ? cat(ftypMp4(), noise) : noise; }
  if (which === 1) return cat(ftypMp4(), randomBoxes(g, 0));
  const f = movie(i % 2 === 0 ? "moovFirst" : "mdatFirst", ftypMp4(), everything());
  for (let k = g.below(4); k > 0; k--) f[g.below(f.length)] = g.next() & 0xff;
  return g.below(3) === 0 ? f.subarray(0, g.below(f.length + 1)) : f;
}

function damage(g, f) {
  const out = cat(f);
  if (g.below(3) === 0) for (let k = 1 + g.below(3); k > 0; k--) out[g.below(out.length || 1)] = g.next() & 0xff;
  return g.below(4) === 0 ? out.subarray(0, g.below(out.length + 1)) : out;
}

function randomJpeg(g) {
  const parts = [[0xff, 0xd8]];
  for (let k = g.below(7); k > 0; k--) {
    const kind = g.below(10);
    if (kind === 0) parts.push([0xff, g.below(2) ? 0xd0 + g.below(8) : 0x01]);
    else if (kind === 1) parts.push([g.next() & 0x7f]); // lost sync
    else {
      const marker = g.pick([0xe0, 0xe1, 0xe1, 0xe2, 0xed, 0xee, 0xfe, 0xdb, 0xc0, 0xc4, 0xdd, g.next() & 0xff]);
      let payload = g.below(3) === 0 ? cat("Exif\0\0", phoneExif()) : g.bytes(g.below(60));
      if (marker === 0xe1 && g.below(3) === 0) payload = cat("http://ns.adobe.com/xap/1.0/\0", g.bytes(g.below(30)));
      const len = g.below(5) === 0 ? g.next() & 0xffff : payload.length + 2;
      parts.push(cat([0xff, marker], u16be(len), payload));
    }
  }
  const end = g.below(5);
  if (end === 0) parts.push(cat(seg(0xda, [1, 1, 0, 0, 63, 0]), g.bytes(g.below(80)), [0xff, 0xd9]));
  else if (end === 1) parts.push(cat([0xff, 0xd9], g.bytes(g.below(10))));
  else if (end === 2) parts.push([0xff, 0xd9]);
  else if (end === 3) parts.push(SCAN());
  return damage(g, cat(...parts));
}

function randomPng(g) {
  const parts = [PNG_SIG];
  for (let k = g.below(7); k > 0; k--) {
    const name = g.below(8) === 0 ? g.bytes(4) : bytes(g.pick(["IHDR", "tEXt", "zTXt", "iTXt", "eXIf", "tIME", "IDAT", "PLTE", "gAMA", "acTL", "iCCP"]));
    const payload = g.below(3) === 0 ? phoneExif() : g.bytes(g.below(50));
    const len = g.below(6) === 0 ? g.next() : payload.length;
    parts.push(cat(u32be(len), name, payload, g.bytes(4)));
  }
  if (g.below(4) !== 0) parts.push(IEND());
  if (g.below(4) === 0) parts.push(g.bytes(g.below(12)));
  return damage(g, cat(...parts));
}

function randomWebp(g) {
  const chunks = [];
  for (let k = g.below(7); k > 0; k--) {
    const name = g.below(8) === 0 ? g.bytes(4) : bytes(g.pick(["VP8X", "VP8L", "VP8 ", "ALPH", "ANIM", "ANMF", "ICCP", "EXIF", "XMP "]));
    const payload = latin(name, 0, 4) === "VP8X" && g.below(2) ? cat([g.next() & 0xff], g.bytes(9)) : g.bytes(g.below(41));
    const len = g.below(7) === 0 ? g.next() : payload.length;
    const pad = payload.length & 1 && g.below(5) !== 0 ? [0] : [];
    chunks.push(cat(name, u32le(len), payload, pad));
  }
  if (g.below(5) === 0) chunks.push(g.bytes(1 + g.below(9)));
  const body = cat(...chunks);
  const riff = g.below(4) === 0 ? g.next() : body.length + 4;
  return damage(g, cat("RIFF", u32le(riff), "WEBP", body));
}

/** A well-formed picture with a random mix of metadata in it (every length right, nothing damaged). */
function wellFormedPicture(g) {
  const some = (list) => list.filter(() => g.below(2) === 0).map((make) => make());
  const which = g.below(3);
  if (which === 0) {
    return jpeg(some([() => exifApp1(phoneExif()), ICC, () => xmpApp1(g.bytes(g.below(30))), ADOBE]),
      some([iptcApp13, () => seg(0xfe, g.bytes(g.below(20))), () => seg(0xe1, g.bytes(g.below(20)))]));
  }
  if (which === 1) {
    return cat(png(...some([() => chunk("tEXt", "Author\0Ann"), () => chunk("eXIf", phoneExif()), () => chunk("gAMA", [0, 0, 0xb1, 0x8f]),
      () => chunk("tIME", g.bytes(7)), () => chunk("iTXt", g.bytes(g.below(20))), () => chunk("zTXt", g.bytes(g.below(20)))])),
    g.below(4) === 0 ? g.bytes(g.below(6)) : []);
  }
  return webp(...some([() => webpChunk("VP8X", cat([g.next() & 0xff], g.bytes(9))), () => webpChunk("ICCP", g.bytes(g.below(21))),
    () => webpChunk("EXIF", g.bytes(g.below(21))), () => webpChunk("VP8L", g.bytes(1 + g.below(20))), () => webpChunk("XMP ", g.bytes(g.below(21)))]));
}

function randomPicture(g) {
  const which = g.below(7);
  if (which === 0) return randomJpeg(g);
  if (which === 1) return randomPng(g);
  if (which === 2) return randomWebp(g);
  if (which === 3) return cat("GIF89a", g.bytes(g.below(40)));
  if (which === 4) return g.bytes(g.below(60));
  if (which === 5) return wellFormedPicture(g);
  return g.below(2) ? randomJpeg(g) : randomWebp(g);
}

/** The input with the removed ranges cut out (what a JPEG or PNG strip must equal). */
function withoutRanges(b, removed) {
  const cut = new Uint8Array(b.length);
  for (const r of removed) cut.fill(1, r.offset, r.offset + r.size);
  return b.filter((_, i) => !cut[i]);
}

// Seen red: with the fill in neutralise running one byte past each box's end, "byte 1157 changed
// outside every reported box" (the box after one lost its first byte).
test("random videos never throw, never change size, and a second run changes nothing", () => {
  const g = rng(0x5eedcafe);
  let walked = 0;
  for (let i = 0; i < 4000; i++) {
    const before = randomVideo(g, i);
    const r = S.stripFile(before, "video/mp4");
    assertOnlyReportedBoxesChanged(before, r.bytes, r);
    if (r.kind === "mp4" || r.kind === "heif") walked++;
    const again = S.stripFile(r.bytes, "video/mp4");
    assert.ok(!again.changed, `input ${i}: a second run changed ${JSON.stringify(paths(again))}`);
    S.describe(before, "video/mp4", { coordinates: true });
    assert.deepStrictEqual(S.describe(r.bytes).details, [], `input ${i}: describe finds nothing once cleaned`);
  }
  assert.ok(walked > 2000, `most inputs should reach the box walk, got ${walked}`);
});

// Seen red: with walkPng's "claims more than is left" check removed, input 583 came back longer
// than it went in ("a picture never grows").
test("random pictures never throw, never grow, and only lose what the report names", () => {
  const g = rng(0xf00d0001);
  const seen = {};
  for (let i = 0; i < 4000; i++) {
    const before = randomPicture(g);
    const r = S.stripFile(before, "picture");
    seen[r.kind] = (seen[r.kind] || 0) + 1;
    assert.ok(r.bytes.length <= before.length, `input ${i}: a picture never grows`);
    if (!r.changed) assert.strictEqual(r.bytes, before, `input ${i}: unchanged means the very array`);
    if (r.unfinished) assert.ok(!r.changed, `input ${i}: a broken picture fails open`);
    if (r.changed && (r.kind === "jpeg" || r.kind === "png")) {
      assert.ok(same(r.bytes, withoutRanges(before, r.removed)), `input ${i}: exactly the reported ranges are gone`);
    }
    if (r.changed && r.kind === "webp") {
      const riff = (r.bytes[4] | (r.bytes[5] << 8) | (r.bytes[6] << 16) | (r.bytes[7] << 24)) >>> 0;
      assert.strictEqual(riff + 8, r.bytes.length, `input ${i}: the RIFF size is rewritten`);
    }
    assert.ok(same(S.stripFile(r.bytes, "picture").bytes, r.bytes), `input ${i}: a second run changes nothing`);
    S.describe(before, "picture", { coordinates: true });
    if (!r.unfinished) assert.deepStrictEqual(S.describe(r.bytes).details, [], `input ${i}: describe finds nothing once cleaned`);
  }
  for (const k of ["jpeg", "png", "webp", "gif", "other"]) assert.ok(seen[k] > 200, `${k}: ${JSON.stringify(seen)}`);
});

// ════════════════════════════════════════════════════════════════════════════
// Agreement with the Rust itself
// ════════════════════════════════════════════════════════════════════════════

/** A tiny program around the relay's two real source files: strips every input both ways. */
function harnessSource() {
  const src = (f) => JSON.stringify(path.join(REPO, "src", "relay", "core", f).replace(/\\/g, "/"));
  return `
#[path = ${src("strip_metadata.rs")}]
mod strip_metadata;
#[path = ${src("strip_video_metadata.rs")}]
mod strip_video_metadata;

use strip_video_metadata::VideoKind;

fn esc(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\\\\""),
            '\\\\' => o.push_str("\\\\\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn put(out: &mut Vec<u8>, b: &[u8]) {
    out.extend_from_slice(&(b.len() as u32).to_le_bytes());
    out.extend_from_slice(b);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let input = std::fs::read(&args[1]).expect("read the inputs");
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < input.len() {
        let code = input[i];
        let len = u32::from_le_bytes([input[i + 1], input[i + 2], input[i + 3], input[i + 4]]) as usize;
        let data = &input[i + 5..i + 5 + len];
        i += 5 + len;
        let ct = match code { 1 => "image/jpeg", 2 => "image/png", 3 => "image/webp", 4 => "image/gif", _ => "application/octet-stream" };
        let pic = strip_metadata::strip_image_metadata(ct, data);
        let mut vid = data.to_vec();
        let r = strip_video_metadata::strip_video_metadata_in_buffer(&mut vid).expect("in memory never fails");
        let kind = match r.kind { VideoKind::Mp4Family => "mp4", VideoKind::Matroska => "matroska", VideoKind::Other => "other" };
        let boxes: Vec<String> = r.neutralised.iter()
            .map(|b| format!("{{\\"path\\":\\"{}\\",\\"offset\\":{},\\"size\\":{}}}", esc(&b.path), b.offset, b.size)).collect();
        let stops: Vec<String> = r.stopped_early.iter().map(|s| format!("\\"{}\\"", esc(s))).collect();
        let rep = format!("{{\\"kind\\":\\"{}\\",\\"found_moov\\":{},\\"bytes_zeroed\\":{},\\"neutralised\\":[{}],\\"stopped_early\\":[{}]}}",
            kind, r.found_moov, r.bytes_zeroed, boxes.join(","), stops.join(","));
        put(&mut out, &pic);
        put(&mut out, &vid);
        put(&mut out, rep.as_bytes());
    }
    std::fs::write(&args[2], out).expect("write the results");
}
`;
}

/** Every fixed file the tests above build, for the agreement check. */
function fixedCorpus() {
  const out = [];
  for (const layout of ["moovFirst", "mdatFirst"]) {
    for (const ftyp of [ftypMp4(), ftypQt()]) {
      out.push(movie(layout, ftyp, everything()));
      for (const ex of [{ moov: [appleMeta()] }, { moov: [androidUdta()] }, { trak: [lociUdta(), trackMeta()] },
        { top: [sphericalUuid(), xmpUuid()] }, { moov: [androidUdta()], mdat64: true },
        { moov: [androidUdta()], mdatSize0: true }, { moov: [androidUdta()], moovSize0: true }]) {
        out.push(movie(layout, ftyp, ex));
      }
    }
    const full = movie(layout, ftypMp4(), everything());
    for (let cut = 0; cut <= full.length; cut += 7) out.push(full.subarray(0, cut));
  }
  out.push(cat(movie("moovFirst", ftypMp4()), bx0("uuid", xmpUuid().subarray(8))));
  out.push(cat(ftypMp4(), bx("moov", cat(mvhd(), androidUdta(), cat(u32be(5000), "trak", filler(40, 12)), appleMeta())), xmpUuid(), bx("mdat", frames())));
  out.push(cat(ftypMp4(), bx("moov", cat(mvhd(), bx("trak", cat(bx("tkhd", filler(84, 14)), cat(u32be(4), "udta", filler(20, 13)))), androidUdta())), bx("mdat", frames())));
  out.push(cat(bx("ftyp", "heic\0\0\0\0mif1heic"), bx("meta", cat(zeros(4), bx("hdlr", filler(25, 15)))), bx("mdat", frames())));
  out.push(cat(movie("moovFirst", ftypMp4()), bx("meta", "\0\0\0\0TOP-META")));
  out.push(cat([0x1a, 0x45, 0xdf, 0xa3, 0x9f, 0x42, 0x86, 0x81, 0x01], bx("moov", androidUdta())));
  out.push(cat(ftypMp4(), u32be(1), "mdat", u64be(8), filler(20, 1)));
  out.push(cat(ftypMp4(), u32be(1), "mdat", u32be(0xffffffff), u32be(0xfffffff0)));
  out.push(cat(ftypMp4(), u32be(1), "mdat", [0, 0, 1]));
  // The box cap: more than a million empty boxes after the ftyp, then an XMP box.
  out.push(ftypThenFree(S.MAX_BOXES + 3, xmpUuid()), ftypThenFree(S.MAX_BOXES - 2, xmpUuid()));
  // Pictures.
  const fake = seg(0xe1, "Exif\0\0FAKE-GPS-COORDINATES-12.34,56.78");
  out.push(cat([0xff, 0xd8], fake, jpeg().subarray(2)));
  out.push(jpeg([exifApp1(phoneExif()), ICC(), xmpApp1("<x/>"), ADOBE()], [iptcApp13(), seg(0xfe, "c")]));
  out.push(jpeg([ICC()]), jpeg(), cat([0xff, 0xd8], JFIF(), [0xff, 0xd9]), cat([0xff, 0xd8], JFIF(), [0xff, 0xd9], "after"));
  out.push(cat([0xff, 0xd8], JFIF(), exifApp1(phoneExif()), [0xff, 0xdb, 0x40, 0x00], filler(20, 26)));
  out.push(cat(png(chunk("tEXt", "Author\0x"), chunk("eXIf", phoneExif()), chunk("tIME", [1, 2, 3, 4, 5, 6, 7])), "TRAIL"));
  out.push(png(), cat(PNG_SIG, IHDR(), u32be(9999), "IDAT", filler(10, 27)));
  out.push(webp(webpChunk("VP8X", [0x3c, 0, 0, 0, 3, 0, 0, 3, 0, 0]), webpChunk("VP8L", [0x2f, 0, 0, 0]),
    webpChunk("EXIF", "GPS 12.34,56.78 here"), webpChunk("XMP ", "odd")));
  out.push(cat("RIFF", u32le(99), "WEBP", webpChunk("VP8L", [1, 2, 3], false)));
  out.push(cat("RIFF", u32le(40), "WEBP", webpChunk("EXIF", "GPS"), "VP8L", u32le(500), filler(8, 28)));
  out.push(cat("GIF89a", "x"), bytes([0xff, 0xd8, 0, 1, 2]), bytes("not a png"), bytes("RIFFxxxx"));
  return out;
}

// Seen red: with walkWebp's tolerance for an unpadded last chunk removed, "input 483 (webp): the
// bytes differ from the Rust's" (the Rust rebuilt it, the JavaScript gave it back unchanged).
test("agrees byte for byte with the relay's Rust strips", (t) => {
  if (process.env.HOS_STRIP_SKIP_RUST === "1") return t.skip("HOS_STRIP_SKIP_RUST=1");
  const rustc = spawnSync("rustc", ["--version"], { encoding: "utf8" });
  if (rustc.status !== 0) return t.skip("rustc is not installed here, so the Rust cannot be run");

  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "hum_strip_agree_"));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  fs.writeFileSync(path.join(dir, "main.rs"), harnessSource());
  const exe = path.join(dir, process.platform === "win32" ? "harness.exe" : "harness");
  const built = spawnSync("rustc", ["--edition", "2021", "-A", "warnings", "-C", "opt-level=1", "-C", "debuginfo=0",
    "-o", exe, path.join(dir, "main.rs")], { encoding: "utf8", cwd: dir });
  assert.strictEqual(built.status, 0, `the harness around the relay's strips must compile:\n${built.stderr}`);

  const corpus = fixedCorpus();
  const gv = rng(0x0dd5eed1);
  for (let i = 0; i < 4000; i++) corpus.push(randomVideo(gv, i));
  const gp = rng(0x0dd5eed2);
  for (let i = 0; i < 5000; i++) corpus.push(randomPicture(gp));

  const CODE = { jpeg: 1, png: 2, webp: 3, gif: 4 };
  const js = corpus.map((b) => S.stripFile(b, "x"));
  const packed = cat(...corpus.flatMap((b, i) => [[CODE[js[i].kind] || 0, ...u32le(b.length)], b]));
  fs.writeFileSync(path.join(dir, "in.bin"), packed);
  const ran = spawnSync(exe, [path.join(dir, "in.bin"), path.join(dir, "out.bin")], { encoding: "utf8", timeout: 300000 });
  assert.strictEqual(ran.status, 0, `the harness must run: ${ran.stderr}`);

  const out = new Uint8Array(fs.readFileSync(path.join(dir, "out.bin")));
  let at = 0;
  const take = () => { const n = (out[at] | (out[at + 1] << 8) | (out[at + 2] << 16) | (out[at + 3] << 24)) >>> 0; const b = out.subarray(at + 4, at + 4 + n); at += 4 + n; return b; };
  const KIND = { mp4: ["mp4", "heif"], matroska: ["webm"], other: ["jpeg", "png", "webp", "gif", "other"] };
  let pictures = 0, picturesChanged = 0, videos = 0, changedByBoth = 0, stopped = 0;
  for (let i = 0; i < corpus.length; i++) {
    const pic = take(), vid = take();
    const rep = JSON.parse(new TextDecoder().decode(take()));
    const r = js[i];
    const label = `input ${i} (${r.kind})`;
    if (CODE[r.kind]) {
      pictures++;
      assert.ok(same(r.bytes, pic), `${label}: the bytes differ from the Rust's`);
      assert.strictEqual(r.changed, !same(pic, corpus[i]), `${label}: changed`);
      if (r.changed) picturesChanged++;
    } else {
      videos++;
      assert.ok(KIND[rep.kind].includes(r.kind), `${label}: the Rust says ${rep.kind}`);
      assert.ok(same(r.bytes, vid), `${label}: the bytes differ from the Rust's`);
      if (rep.kind === "mp4") {
        assert.strictEqual(r.foundMoov, rep.found_moov, `${label}: foundMoov`);
        assert.strictEqual(r.bytesZeroed, rep.bytes_zeroed, `${label}: bytesZeroed`);
        assert.deepStrictEqual(r.removed, rep.neutralised, `${label}: the boxes`);
        assert.deepStrictEqual(r.stoppedEarly, rep.stopped_early, `${label}: the sentences`);
        if (r.changed) changedByBoth++;
        if (r.stoppedEarly.length) stopped++;
      }
    }
  }
  t.diagnostic(`${corpus.length} inputs: ${pictures} pictures (${picturesChanged} changed), ${videos} others (${changedByBoth} videos changed, ${stopped} stopped early), all identical to the Rust`);
  assert.ok(picturesChanged > 1000 && changedByBoth > 1000 && stopped > 200, `a real spread of inputs: ${picturesChanged} ${changedByBoth} ${stopped}`);
});
