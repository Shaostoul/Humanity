// Test files built byte by byte (2026-10-11): phone photos and videos with where they were taken
// in them, a HEIC photo, and the cuts of a video. Not a test itself (its name does not end in
// .test.js). The builders are scripts/tests/strip-metadata.test.js's own, which are the relay's
// Rust tests' (src/relay/core/strip_metadata.rs, strip_video_metadata.rs), copied line for line,
// so the files the upload tests send are the files the strip is held to. Used by
// upload-clean-web.test.js, media-web-files.test.js and private-files-web.test.js.

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
const u64be = (n) => { const big = BigInt(n); return [...u32be(Number(big >> 32n)), ...u32be(Number(big & 0xffffffffn))]; };
const zeros = (n) => new Uint8Array(n);
/** Filler bytes that are never ASCII letters, so no box name appears in them by accident. */
function filler(n, seed) { return Uint8Array.from({ length: n }, (_, i) => 0x80 | ((i * 7 + seed * 13) % 128)); }
function find(hay, needle, from = 0) {
  const n = bytes(needle);
  outer: for (let i = from; i + n.length <= hay.length; i++) {
    for (let k = 0; k < n.length; k++) if (hay[i + k] !== n[k]) continue outer;
    return i;
  }
  return -1;
}
const contains = (hay, needle) => find(hay, needle) >= 0;

// ── Videos ───────────────────────────────────────────────────────────────

function bx(name, payload) { const p = bytes(payload); return cat(u32be(p.length + 8), name, p); }
function bx64(name, payload) { const p = bytes(payload); return cat(u32be(1), name, u64be(p.length + 16), p); }
function bx0(name, payload) { return cat(u32be(0), name, payload); }

const PLACE = "+37.3349-122.0090+030.000/";
const ftypQt = () => bx("ftyp", "qt  \x00\x00\x02\x00qt  ");
const ftypMp4 = () => bx("ftyp", "isom\x00\x00\x02\x00isomiso2avc1mp41");
const mvhd = () => bx("mvhd", filler(100, 1));
/** The frames, with a decoy "udta" box inside to prove mdat is never looked into. */
const frames = (n = 150) => cat(filler(n, 2), bx("udta", "DECOY-inside-the-frames"), filler(n, 3));

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
const XMP_UUID = [0xBE, 0x7A, 0xCF, 0xCB, 0x97, 0xA9, 0x42, 0xE8, 0x9C, 0x71, 0x99, 0x94, 0x91, 0xE3, 0xAF, 0xAC];
const xmpUuid = () => bx("uuid", cat(XMP_UUID,
  '<x:xmpmeta><rdf:Description exif:GPSLatitude="37,20.094N" exif:GPSLongitude="122,0.54W"/></x:xmpmeta>'));
const SPHERICAL_UUID = [0xff, 0xcc, 0x82, 0x63, 0xf8, 0x55, 0x4a, 0x93, 0x88, 0x14, 0x58, 0x7a, 0x02, 0x52, 0x1f, 0xdd];
const sphericalUuid = () => bx("uuid", cat(SPHERICAL_UUID, "<GSpherical:Spherical>true</GSpherical:Spherical>"));

/**
 * A whole movie with real chunk offsets pointing into mdat ("moovFirst" or "mdatFirst").
 * ex.frames: how many filler bytes on each side of the decoy in mdat (a bigger file).
 */
function movie(layout, ftyp, ex = {}) {
  const top = ex.top || [], moovExtra = ex.moov || [], trakExtra = ex.trak || [];
  const moovWith = (offsets) => {
    const body = cat(mvhd(), trak(offsets, trakExtra), ...moovExtra);
    return ex.moovSize0 ? bx0("moov", body) : bx("moov", body);
  };
  const fr = frames(ex.frames || 150);
  const mdat = ex.mdat64 ? bx64("mdat", fr) : ex.mdatSize0 ? bx0("mdat", fr) : bx("mdat", fr);
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

/** A phone video: where it was filmed and which camera, Apple's way. */
const phoneVideo = (layout = "moovFirst", ex = {}) => movie(layout, ftypQt(), Object.assign({ moov: [appleMeta()] }, ex));

/** A HEIC still photo: ftyp naming heic, a top-level meta holding the picture, and no movie. */
const heic = () => cat(bx("ftyp", "heic\0\0\0\0mif1heic"), bx("meta", cat(zeros(4), bx("hdlr", filler(25, 15)))), bx("mdat", frames()));

// ── Pictures ─────────────────────────────────────────────────────────────

/** One JPEG segment: FF, marker, length (counting itself), payload. */
function seg(marker, payload) { const p = bytes(payload); return cat([0xff, marker], u16be(p.length + 2), p); }

const JFIF = () => seg(0xe0, cat("JFIF\0", [1, 1, 0, 0, 1, 0, 1, 0, 0]));
const DQT = () => seg(0xdb, cat([0], filler(64, 20)));
const SOF0 = () => seg(0xc0, [8, 0, 8, 0, 8, 1, 1, 0x11, 0]);
const DHT = () => seg(0xc4, cat([0], [0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0], [0]));
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
/** A phone photo: where it was taken, the camera and its serial number, in Exif. */
const phoneJpeg = () => jpeg([exifApp1(phoneExif())]);
/** A JPEG whose segments stop making sense part way (the strip cannot walk it). */
const brokenJpeg = () => cat([0xff, 0xd8], JFIF(), exifApp1(phoneExif()).subarray(0, 40));

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

module.exports = {
  bytes, cat, u16be, u32be, u64be, zeros, filler, find, contains,
  bx, bx64, bx0, PLACE, ftypQt, ftypMp4, frames, trak, appleMeta, androidUdta, lociUdta, trackMeta, xmpUuid,
  sphericalUuid, movie, everything, phoneVideo, heic,
  seg, jpeg, tiff, phoneExif, exifApp1, phoneJpeg, brokenJpeg, crc32, chunk, png,
};
