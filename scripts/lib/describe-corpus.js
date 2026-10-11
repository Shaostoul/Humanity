// Files with hidden information in them, built byte by byte, for holding the desktop's Rust
// `describe` (src/relay/core/describe_metadata.rs) to the web's `HosStrip.describe`
// (web/shared/strip-metadata.js): the attach step's "This photo says where it was taken and which
// camera took it." must be the same sentence in both apps (docs/design/media-showcase.md, "Files
// on the sender's device and in messages, exact", part 1).
//
// Two sets:
//   fixed()      named files covering every place describe looks: Exif (both byte orders, GPS,
//                camera, serial, lens, times, people, comments), XMP (attributes and elements,
//                GPS in both notations, place names), IPTC, PNG text keywords (and ImageMagick's
//                raw Exif profile), WebP chunks, MP4/MOV/3GP boxes (Apple's keys, the copyright
//                sign items, Android's keys, 3GPP's loci in UTF-8 and UTF-16), HEIF, and broken
//                and cut-off files. Written to tests/fixtures/describe/ by
//                scripts/make-describe-fixtures.js, with the web's answers.
//   random(n, seed)  generated files: the same builders with random values (text with accents,
//                emoji, control characters and a byte-order mark; random numbers, byte orders and
//                sizes), random cuts and random flipped bytes.
//
// Plain CommonJS, no dependencies.

'use strict';

// ── bytes ───────────────────────────────────────────────────────────────────────────────────────

/** A string as UTF-8 bytes. */
function utf8(s) { return new Uint8Array(Buffer.from(s, 'utf8')); }
/** A string as one byte per character ("\xA9xyz" is four bytes). */
function latin(s) { return Uint8Array.from(s, (c) => c.charCodeAt(0) & 0xff); }
function bytes(x) {
  if (x instanceof Uint8Array) return x;
  if (typeof x === 'string') return latin(x);
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
const u16le = (n) => [n & 0xff, (n >> 8) & 0xff];
const u32be = (n) => [(n >>> 24) & 0xff, (n >>> 16) & 0xff, (n >>> 8) & 0xff, n & 0xff];
const u32le = (n) => [n & 0xff, (n >>> 8) & 0xff, (n >>> 16) & 0xff, (n >>> 24) & 0xff];
const zeros = (n) => new Uint8Array(n);
/** Filler that is never an ASCII letter, so no box or tag name appears in it by accident. */
function filler(n, seed) { return Uint8Array.from({ length: n }, (_, i) => 0x80 | ((i * 7 + seed * 13) % 128)); }

// ── Exif (TIFF) ─────────────────────────────────────────────────────────────────────────────────

const ascii = (tag, s) => ({ tag, type: 2, count: utf8(s).length + 1, data: cat(utf8(s), [0]) });
const rationals = (tag, parts, little) => ({
  tag, type: 5, count: parts.length,
  data: cat(...parts.map(([n, d]) => cat(little ? u32le(n) : u32be(n), little ? u32le(d) : u32be(d)))),
});
const short = (tag, v, little) => ({ tag, type: 3, count: 1, data: little ? u16le(v) : u16be(v) });
const undef = (tag, s) => ({ tag, type: 7, count: bytes(s).length, data: bytes(s) });

/** A TIFF block: IFD0 (with pointers to an Exif IFD and a GPS IFD when given), then the values. */
function tiff(little, ifd0, exif, gps) {
  const W16 = little ? u16le : u16be;
  const W32 = little ? u32le : u32be;
  const first = ifd0.slice();
  if (exif) first.push({ tag: 0x8769, type: 4, count: 1, ptr: 'exif' });
  if (gps) first.push({ tag: 0x8825, type: 4, count: 1, ptr: 'gps' });
  const ifds = [['ifd0', first]];
  if (exif) ifds.push(['exif', exif]);
  if (gps) ifds.push(['gps', gps]);
  let pos = 8;
  const where = {};
  for (const [name, es] of ifds) { where[name] = pos; pos += 2 + 12 * es.length + 4; }
  let dataPos = pos;
  const parts = [little ? 'II' : 'MM', W16(42), W32(8)];
  const data = [];
  for (const [, es] of ifds) {
    parts.push(W16(es.length));
    for (const e of es) {
      parts.push(W16(e.tag), W16(e.type), W32(e.count));
      if (e.ptr) parts.push(W32(where[e.ptr]));
      else if (e.data.length <= 4) parts.push(cat(e.data, zeros(4 - e.data.length)));
      else {
        parts.push(W32(e.offset !== undefined ? e.offset : dataPos));
        data.push(e.data);
        dataPos += e.data.length;
      }
    }
    parts.push(W32(0));
  }
  return cat(...parts, ...data);
}

/** A phone's Exif: camera, times, and where it was taken. */
function phoneExif(o = {}) {
  const little = o.little !== undefined ? o.little : true;
  const lat = o.lat || [[37, 1], [20, 1], [5640, 1000]];
  const lon = o.lon || [[122, 1], [0, 1], [3240, 100]];
  return tiff(little,
    [ascii(0x010f, o.make !== undefined ? o.make : 'Apple'), ascii(0x0110, o.model !== undefined ? o.model : 'iPhone 15 Pro'),
      ascii(0x0131, '17.0'), ascii(0x0132, '2026:10:11 09:30:00')],
    [ascii(0x9003, '2026:10:11 09:30:00'), ascii(0xa431, 'SN12345'), ascii(0xa434, 'Back camera 6.86mm f/1.78')],
    o.noGps ? null : [ascii(1, o.latRef || 'N'), rationals(2, lat, little), ascii(3, o.lonRef || 'W'), rationals(4, lon, little), short(5, 0, little)]);
}

// ── JPEG ────────────────────────────────────────────────────────────────────────────────────────

function seg(marker, payload) { const p = bytes(payload); return cat([0xff, marker], u16be(p.length + 2), p); }
const JFIF = () => seg(0xe0, cat('JFIF\0', [1, 1, 0, 0, 1, 0, 1, 0, 0]));
const ICC = () => seg(0xe2, cat('ICC_PROFILE\0', [1, 1], filler(40, 20)));
const exifApp1 = (t) => seg(0xe1, cat('Exif\0\0', t));
const xmpApp1 = (x) => seg(0xe1, cat('http://ns.adobe.com/xap/1.0/\0', utf8(x)));
const xmpExtApp1 = (x) => seg(0xe1, cat('http://ns.adobe.com/xmp/extension/\0', filler(40, 3), utf8(x)));
function iptc(records) { return cat(...records.map(([ds, s]) => cat([0x1c, 0x02, ds], u16be(utf8(s).length), utf8(s)))); }
const iptcApp13 = (records) => seg(0xed, cat('Photoshop 3.0\0', '8BIM', [0x04, 0x04, 0, 0], u32be(iptc(records).length), iptc(records)));
/** A JPEG made of these segments, then a scan the walk never reads. */
function jpeg(segments, tail) {
  return cat([0xff, 0xd8], JFIF(), ...(segments || []), seg(0xdb, filler(65, 21)), [0xff, 0xda], filler(60, 22), [0xff, 0xd9], tail || []);
}

// ── XMP ─────────────────────────────────────────────────────────────────────────────────────────

const XMP_FULL = '<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF><rdf:Description exif:GPSLatitude="37,20.094N" ' +
  'exif:GPSLongitude="122,0.54W" photoshop:City="Cupertino" xmp:CreatorTool="Photos 9.0" xmp:CreateDate="2026-10-11">' +
  '<tiff:Make>Apple</tiff:Make><tiff:Model >iPhone 15 Pro</tiff:Model><dc:creator>Ann</dc:creator>' +
  '<aux:SerialNumber>42</aux:SerialNumber><aux:Lens>6.86mm</aux:Lens></rdf:Description></rdf:RDF></x:xmpmeta>';

// ── PNG ─────────────────────────────────────────────────────────────────────────────────────────

const PNG_SIG = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
function chunk(name, payload) { const p = bytes(payload); return cat(u32be(p.length), name, p, [0x12, 0x34, 0x56, 0x78]); }
const IHDR = () => chunk('IHDR', cat(u32be(4), u32be(4), [8, 6, 0, 0, 0]));
function png(...chunks) { return cat(PNG_SIG, IHDR(), ...chunks, chunk('IDAT', filler(30, 23)), chunk('IEND', [])); }
const itxt = (kw, text, compressed) => chunk('iTXt', cat(kw, [0, compressed ? 1 : 0, 0], 'en', [0], 'translated', [0], utf8(text)));
const textChunk = (kw, text) => chunk('tEXt', cat(kw, [0], utf8(text)));
/** ImageMagick's raw profile: "exif", the length, then the Exif as hex. */
function rawProfile(t) {
  const hex = Buffer.from(t).toString('hex').replace(/(.{72})/g, '$1\n');
  return `\nexif\n    ${t.length}\n${hex}\n`;
}

// ── WebP ────────────────────────────────────────────────────────────────────────────────────────

function webpChunk(name, payload, pad = true) {
  const p = bytes(payload);
  return cat(name, u32le(p.length), p, pad && p.length % 2 ? [0] : []);
}
function webp(...chunks) { const body = cat(...chunks); return cat('RIFF', u32le(body.length + 4), 'WEBP', body); }

// ── MP4, MOV, 3GP, HEIF ─────────────────────────────────────────────────────────────────────────

function bx(name, payload) { const p = bytes(payload); return cat(u32be(p.length + 8), name, p); }
function bx64(name, payload) { const p = bytes(payload); return cat(u32be(1), name, u32be(0), u32be(p.length + 16), p); }
const ftypQt = () => bx('ftyp', 'qt  \x00\x00\x02\x00qt  ');
const ftypMp4 = () => bx('ftyp', 'isom\x00\x00\x02\x00isomiso2avc1mp41');
const ftyp3gp = () => bx('ftyp', '3gp4\x00\x00\x02\x003gp4isom');
const ftypM4a = () => bx('ftyp', 'M4A \x00\x00\x02\x00M4A isom');
const ftypHeic = () => bx('ftyp', 'heic\x00\x00\x00\x00mif1heic');
const mvhd = () => bx('mvhd', filler(100, 1));
const mdat = () => bx('mdat', cat(filler(80, 2), bx('udta', 'DECOY-inside-the-frames'), filler(40, 3)));
const XMP_UUID = [0xbe, 0x7a, 0xcf, 0xcb, 0x97, 0xa9, 0x42, 0xe8, 0x9c, 0x71, 0x99, 0x94, 0x91, 0xe3, 0xaf, 0xac];
const xmpUuid = (x) => bx('uuid', cat(XMP_UUID, utf8(x)));

/** Apple's QuickTime metadata: hdlr, keys, then an ilst of values. */
function appleMeta(pairs, iso) {
  const hdlr = bx('hdlr', cat(zeros(8), 'mdta', zeros(13)));
  const key = (k) => cat(u32be(utf8(k).length + 8), 'mdta', utf8(k));
  const keys = bx('keys', cat(zeros(4), u32be(pairs.length), ...pairs.map(([k]) => key(k))));
  const item = (i, text) => { const data = bx('data', cat(u32be(1), zeros(4), utf8(text))); return cat(u32be(data.length + 8), u32be(i), data); };
  const ilst = bx('ilst', cat(...pairs.map(([, v], i) => item(i + 1, v))));
  return bx('meta', cat(iso ? zeros(4) : [], hdlr, keys, ilst));
}
const APPLE = [['com.apple.quicktime.location.ISO6709', '+37.3349-122.0090+030.000/'], ['com.apple.quicktime.make', 'Apple'],
  ['com.apple.quicktime.model', 'iPhone 15 Pro'], ['com.apple.quicktime.software', '17.0'],
  ['com.apple.quicktime.creationdate', '2026-10-11T09:30:00-0700'], ['com.apple.quicktime.location.name', 'Infinite Loop'],
  ['com.apple.quicktime.camera.lens_model', 'iPhone 15 Pro back camera'], ['com.apple.quicktime.full-frame-rate-playback-intent', '1']];
const ANDROID = [['com.android.version', '14'], ['com.android.manufacturer', 'Google'], ['com.android.model', 'Pixel 8']];

/** A copyright-sign item as QuickTime writes it in udta: text length, language, text. */
const qtItem = (name, text) => bx(name, cat(u16be(utf8(text).length), [0x15, 0xc7], utf8(text)));
/** The same in an ilst: a data box. */
const ilstItem = (name, text) => bx(name, bx('data', cat(u32be(1), zeros(4), utf8(text))));

function loci(name, utf16, lat, lon) {
  const fixed = (deg) => u32be(Math.trunc(deg * 65536) >>> 0);
  const nm = utf16 ? cat([0xfe, 0xff], ...Array.from(name, (c) => u16be(c.charCodeAt(0))), [0, 0]) : cat(utf8(name), [0]);
  return bx('loci', cat(zeros(4), [0x15, 0xc7], nm, [0], fixed(lon), fixed(lat), fixed(30), 'Earth\0', '\0'));
}

function trak(...extra) {
  const tkhd = bx('tkhd', filler(84, 11));
  const mdia = bx('mdia', cat(bx('mdhd', filler(24, 8)), bx('hdlr', filler(25, 9)), bx('minf', filler(30, 10))));
  return bx('trak', cat(tkhd, mdia, ...extra));
}

/** A movie: ftyp, the movie box with these in it, the frames; `top` boxes after the frames. */
function movie(ftyp, moovParts, top, mdatFirst) {
  const moov = bx('moov', cat(mvhd(), ...moovParts));
  return mdatFirst ? cat(ftyp, mdat(), moov, ...(top || [])) : cat(ftyp, moov, mdat(), ...(top || []));
}

// ── the fixed set ───────────────────────────────────────────────────────────────────────────────

/** [file name, the name or type passed to describe, bytes]. */
function fixed() {
  const out = [];
  const add = (file, nameOrType, b) => out.push([file, nameOrType, b]);
  // JPEG
  add('jpeg-phone.jpg', 'IMG_0001.JPG', jpeg([exifApp1(phoneExif()), ICC()]));
  add('jpeg-phone-bigendian.jpg', 'image/jpeg', jpeg([exifApp1(phoneExif({ little: false, latRef: 'S', lonRef: 'E', make: 'Canon', model: 'Canon EOS R5' }))]));
  add('jpeg-camera-only.jpg', 'photo.jpeg', jpeg([exifApp1(phoneExif({ noGps: true, make: 'NIKON CORPORATION', model: 'NIKON Z 6' }))]));
  add('jpeg-gps-zero-denominator.jpg', 'a.jpg', jpeg([exifApp1(phoneExif({ lat: [[37, 0], [1, 1], [1, 1]] }))]));
  add('jpeg-xmp.jpg', 'x.jpg', jpeg([xmpApp1(XMP_FULL)]));
  add('jpeg-xmp-decimal-gps.jpg', 'x.jpg', jpeg([xmpApp1('<rdf:Description exif:GPSLatitude = "-33.8568" exif:GPSLongitude="151.2153"/>')]));
  add('jpeg-xmp-drone.jpg', 'x.jpg', jpeg([xmpApp1('<rdf:Description drone-dji:GpsLatitude="22.5431" drone-dji:GpsLongitude="113.9437" tiff:Make="DJI"/>')]));
  add('jpeg-xmp-extension.jpg', 'x.jpg', jpeg([xmpExtApp1('<Iptc4xmpCore:Location>Old Town</Iptc4xmpCore:Location>')]));
  add('jpeg-iptc.jpg', 'x.jpg', jpeg([iptcApp13([[90, 'Lisbon'], [101, 'Portugal'], [80, 'Ann'], [5, 'Holiday'], [55, '20261011'], [25, 'beach']])]));
  add('jpeg-comment.jpg', 'x.jpg', jpeg([seg(0xfe, 'made with love')]));
  add('jpeg-blank-comment.jpg', 'x.jpg', jpeg([seg(0xfe, '   ')]));
  add('jpeg-unknown-app1.jpg', 'x.jpg', jpeg([seg(0xe1, 'SOMETHING ELSE')]));
  add('jpeg-after-end.jpg', 'x.jpg', cat([0xff, 0xd8], JFIF(), [0xff, 0xd9], 'TRAILING'));
  add('jpeg-clean.jpg', 'x.jpg', jpeg([ICC()]));
  add('jpeg-lost-sync.jpg', 'x.jpg', cat([0xff, 0xd8], exifApp1(phoneExif()), [0x00, 0x11], filler(20, 4)));
  add('jpeg-cut.jpg', 'x.jpg', jpeg([exifApp1(phoneExif())]).subarray(0, 60));
  add('jpeg-unicode-camera.jpg', 'x.jpg', jpeg([exifApp1(phoneExif({ make: '﻿École\u0001\u0002 caméra', model: 'Modèle 📷 X' }))]));
  // PNG
  add('png-exif.png', 'image/png', png(chunk('eXIf', phoneExif())));
  add('png-exif-prefixed.png', 'p.png', png(chunk('eXIf', cat('Exif\0\0', phoneExif({ little: false })))));
  add('png-text.png', 'p.png', png(textChunk('Author', 'Ann'), textChunk('Title', 'Garden'), textChunk('Creation Time', 'now'),
    textChunk('Software', 'GIMP'), textChunk('Source', 'Pixel 8'), textChunk('Weird', 'x'), chunk('tIME', [7, 234, 10, 11, 9, 30, 0])));
  add('png-itxt-xmp.png', 'p.png', png(itxt('XML:com.adobe.xmp', XMP_FULL)));
  add('png-itxt-compressed.png', 'p.png', png(itxt('XML:com.adobe.xmp', 'zzz', true)));
  add('png-ztxt.png', 'p.png', png(chunk('zTXt', cat('Raw profile type exif', [0, 0], filler(30, 5)))));
  add('png-raw-profile.png', 'p.png', png(textChunk('Raw profile type exif', rawProfile(phoneExif()))));
  add('png-after-iend.png', 'p.png', cat(png(), 'TRAIL'));
  add('png-cut.png', 'p.png', png(textChunk('Author', 'Ann')).subarray(0, 40));
  add('png-clean.png', 'p.png', png());
  // WebP
  add('webp-exif.webp', 'image/webp', webp(webpChunk('VP8X', [0x08, 0, 0, 0, 3, 0, 0, 3, 0, 0]), webpChunk('VP8L', [0x2f, 0, 0, 0]),
    webpChunk('EXIF', cat('Exif\0\0', phoneExif()))));
  add('webp-xmp.webp', 'w.webp', webp(webpChunk('VP8L', [0x2f, 0, 0, 0]), webpChunk('XMP ', utf8(XMP_FULL))));
  add('webp-trailing.webp', 'w.webp', cat(webp(webpChunk('VP8L', [0x2f, 0, 0, 0])), [1, 2, 3]));
  add('webp-broken.webp', 'w.webp', cat('RIFF', u32le(40), 'WEBP', webpChunk('EXIF', 'GPS'), 'VP8L', u32le(500), filler(8, 6)));
  // GIF and others
  add('gif.gif', 'g.gif', cat('GIF89a', filler(20, 7)));
  add('webm.webm', 'clip.webm', cat([0x1a, 0x45, 0xdf, 0xa3], filler(30, 8)));
  add('other.txt', 'notes.txt', utf8('just some words'));
  add('named-jpeg-not-one.jpg', 'IMG_0002.JPG', utf8('not a picture at all'));
  // Videos
  add('mov-apple.mov', 'IMG_0003.MOV', movie(ftypQt(), [appleMeta(APPLE)], null, true));
  add('mp4-apple-iso-meta.mp4', 'video/mp4', movie(ftypMp4(), [appleMeta(APPLE, true)]));
  add('mp4-android.mp4', 'VID_1.mp4', movie(ftypMp4(), [bx('udta', cat(qtItem('\xA9xyz', '+37.3349-122.0090/'), qtItem('\xA9mak', 'Google'), qtItem('\xA9mod', 'Pixel 8'))),
    appleMeta(ANDROID)]));
  add('mp4-ilst-items.mp4', 'v.mp4', movie(ftypMp4(), [bx('udta', bx('meta', cat(zeros(4), bx('hdlr', filler(25, 9)),
    bx('ilst', cat(ilstItem('\xA9nam', 'Holiday'), ilstItem('\xA9ART', 'Ann'), ilstItem('\xA9day', '2026'), ilstItem('\xA9too', 'Lavf'),
      ilstItem('\xA9xyz', '+48.8584+002.2945/'), ilstItem('zzzz', 'odd'))))))]));
  add('3gp-loci.3gp', 'video/3gpp', movie(ftyp3gp(), [trak(bx('udta', loci("Grandma's house", false, 37.3349, -122.009)))]));
  add('3gp-loci-utf16.3gp', 'c.3gp', movie(ftyp3gp(), [bx('udta', loci('Café Zoë', true, -33.8568, 151.2153))]));
  add('mp4-xmp-uuid.mp4', 'v.mp4', movie(ftypMp4(), [], [xmpUuid(XMP_FULL)]));
  add('mp4-udta-xmp.mp4', 'v.mp4', movie(ftypMp4(), [bx('udta', bx('XMP_', utf8('<x tiff:Model="GoPro HERO12"/>')))]));
  add('mp4-trak-meta.mp4', 'v.mp4', movie(ftypMp4(), [trak(bx('meta', cat(zeros(4), bx('ilst', 'TRACK-META'))))]));
  add('mp4-camera-items.mp4', 'v.mp4', movie(ftypMp4(), [bx('udta', cat(bx('CAME', 'serial'), bx('LENS', 'wide'), bx('FIRM', 'v1'), bx('free', zeros(8))))]));
  add('m4a-recording.m4a', 'audio/mp4', movie(ftypM4a(), [bx('udta', bx('meta', cat(zeros(4), bx('hdlr', filler(25, 9)), bx('ilst', ilstItem('\xA9mak', 'Zoom')))))]));
  add('mp4-64bit.mp4', 'v.mp4', cat(ftypMp4(), bx64('moov', cat(mvhd(), bx64('udta', qtItem('\xA9xyz', '-12.5+045.25/')))), mdat()));
  add('mp4-top-meta.mp4', 'v.mp4', cat(movie(ftypMp4(), []), bx('meta', cat(zeros(4), bx('ilst', ilstItem('\xA9xyz', '+10+020/'))))));
  add('mp4-broken-box.mp4', 'v.mp4', cat(ftypMp4(), bx('moov', cat(mvhd(), qtItem('\xA9xyz', 'x'), cat(u32be(5000), 'trak', filler(40, 12)), appleMeta(APPLE))), mdat()));
  add('mp4-no-moov.mp4', 'v.mp4', cat(ftypMp4(), mdat()));
  add('mp4-clean.mp4', 'v.mp4', movie(ftypMp4(), [trak()]));
  add('heic-xmp.heic', 'IMG_0004.HEIC', cat(ftypHeic(), bx('meta', cat(zeros(4), bx('hdlr', filler(25, 15)))), xmpUuid('<x exif:GPSLatitude="1,2N" exif:GPSLongitude="3,4E"/>'), mdat()));
  add('heic-plain.heic', 'image/heic', cat(ftypHeic(), bx('meta', cat(zeros(4), bx('hdlr', filler(25, 15)))), mdat()));
  const full = movie(ftypMp4(), [appleMeta(APPLE), bx('udta', loci('Here', false, 1, 2))]);
  for (const cut of [20, 60, 140, 200, 300]) add(`mp4-cut-${cut}.mp4`, 'v.mp4', full.subarray(0, cut));
  return out;
}

// ── the random set ──────────────────────────────────────────────────────────────────────────────

/** A small seeded generator (mulberry32), so a failing input can be found again by its number. */
function rng(seed) {
  let a = seed >>> 0;
  const next = () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
  const int = (n) => Math.floor(next() * n);
  const pick = (xs) => xs[int(xs.length)];
  return { next, int, pick, chance: (p) => next() < p };
}

const PIECES = ['Apple', 'Canon', 'iPhone 15 Pro', 'Pixel 8', 'É', 'ç', 'ü', '東京', 'Zoë', '📷', ' ', '　', '﻿', ' ', '  ',
  '\u0001', '\u0007', '\t', '\n', '\u007F', '"', '<', '>', '=', ',', '.', '-', '+', 'N', 'S', 'E', 'W', 's', 'w', '0', '1', '37', '122',
  '20.094', '5.6', 'Infinity', 'e5', ' ', 'x'];

/** Random text from pieces that matter to the parsers (spaces of every kind, signs, letters). */
function randomText(g, max) {
  let s = '';
  const n = 1 + g.int(max || 8);
  for (let i = 0; i < n; i++) s += g.pick(PIECES);
  return s;
}

/** A random GPS value in one of the notations XMP and ISO 6709 use. */
function randomCoordText(g) {
  const d = () => String(g.int(200));
  const f = () => (g.chance(0.5) ? '.' + String(g.int(100000)) : '');
  const pad = (n, w) => String(n).padStart(w, '0');
  const sign = () => g.pick(['+', '-']);
  // A real position in each of ISO 6709's three forms (degrees; degrees and minutes; degrees,
  // minutes and seconds), with an altitude and the closing slash sometimes, as phones write them.
  const iso = (form) => {
    const one = (deg, width) => {
      const dd = pad(g.int(deg), width);
      if (form === 0) return dd + f();
      if (form === 1) return dd + pad(g.int(60), 2) + f();
      return dd + pad(g.int(60), 2) + pad(g.int(60), 2) + f();
    };
    return `${sign()}${one(90, 2)}${sign()}${one(180, 3)}${g.chance(0.5) ? sign() + String(g.int(900)) + f() : ''}${g.chance(0.7) ? '/' : ''}`;
  };
  if (g.chance(0.5)) {
    switch (g.int(5)) {
      case 0: case 1: case 2: return iso(g.int(3));
      case 3: return `${g.int(90)},${g.int(60)}${f()}${g.pick(['N', 'S', 'n', 's'])}`;
      default: return `${g.int(180)},${g.int(60)},${g.int(60)}${f()}${g.pick(['', ' ', ' '])}${g.pick(['E', 'W', 'e', 'w'])}`;
    }
  }
  switch (g.int(6)) {
    case 0: return `${d()},${d()}${f()}${g.pick(['N', 'S', 'E', 'W', 'n', 's'])}`;
    case 1: return `${d()},${d()},${d()}${f()} ${g.pick(['N', 'S', 'E', 'W'])}`;
    case 2: return `${g.pick(['-', ''])}${d()}${f()}`;
    case 3: return `${g.pick(['+', '-'])}${String(g.int(100000))}${f()}${g.pick(['+', '-'])}${String(g.int(10000000))}${f()}/`;
    case 4: return randomText(g, 4);
    default: return `  ${d()}${f()}e${g.int(3)}`;
  }
}

function randomXmp(g) {
  const parts = [];
  const n = 1 + g.int(6);
  for (let i = 0; i < n; i++) {
    const tag = g.pick(['exif:GPSLatitude', 'exif:GPSLongitude', 'drone-dji:GpsLatitude', 'drone-dji:GpsLongitude', 'tiff:Make', 'tiff:Model',
      'photoshop:City', 'Iptc4xmpCore:Location', 'Iptc4xmpCore:LocationX', 'aux:Lens', 'aux:LensInfo', 'aux:SerialNumber', 'xmp:CreateDate',
      'dc:creator', 'dc:title', 'GPSLATITUDE', 'gpslongitude_x', 'photoshop:Country', 'photoshop:CountryCode']);
    const v = g.chance(0.5) ? randomCoordText(g) : randomText(g);
    const ws = g.pick(['', ' ', '  ', ' ', '\t']);
    switch (g.int(4)) {
      case 0: parts.push(`${tag}${ws}=${ws}"${v.replace(/"/g, '')}"`); break;
      case 1: parts.push(`<${tag}>${v.replace(/</g, '')}</${tag}>`); break;
      case 2: parts.push(`<${tag}${g.pick([' a="1"', '　', ' ', 'x'])}>${v.replace(/</g, '')}</${tag}>`); break;
      default: parts.push(`${tag} ${v}`);
    }
  }
  return parts.join(g.pick([' ', '', '\n', '><']));
}

function randomRational(g) { return [g.int(5) === 0 ? 0 : g.int(100000), g.int(6) === 0 ? 0 : 1 + g.int(1000)]; }

function randomExif(g) {
  const little = g.chance(0.5);
  const ifd0 = [];
  if (g.chance(0.7)) ifd0.push(ascii(0x010f, randomText(g)));
  if (g.chance(0.7)) ifd0.push(ascii(0x0110, randomText(g)));
  if (g.chance(0.2)) ifd0.push(undef(0x010f, 'ab'));
  for (const t of [0x0131, 0x0132, 0x013b, 0x8298, 0x9c9d, 0x010e, 0x9c9b]) if (g.chance(0.15)) ifd0.push(ascii(t, randomText(g, 3)));
  const exif = g.chance(0.6) ? [] : null;
  if (exif) for (const t of [0x9003, 0xa430, 0xa431, 0xa432, 0xa435, 0x927c, 0xa420]) if (g.chance(0.2)) exif.push(ascii(t, randomText(g, 3)));
  if (exif && g.chance(0.3)) exif.push(undef(0x9286, g.chance(0.5) ? 'ASCII\0\0\0hello' : 'ASCII\0\0\0   '));
  const gps = g.chance(0.6) ? [] : null;
  if (gps) {
    if (g.chance(0.8)) gps.push(ascii(1, g.pick(['N', 'S', 's', 'X', ''])));
    if (g.chance(0.8)) gps.push(rationals(2, [randomRational(g), randomRational(g), randomRational(g)].slice(0, 1 + g.int(3)), little));
    if (g.chance(0.8)) gps.push(ascii(3, g.pick(['E', 'W', 'w', ''])));
    if (g.chance(0.8)) gps.push(rationals(4, [randomRational(g), randomRational(g), randomRational(g)], little));
    if (g.chance(0.3)) gps.push(short(6, 7, little));
  }
  let t = tiff(little, ifd0, exif, gps);
  if (g.chance(0.15)) { t = t.slice(); t[4 + g.int(4)] = g.int(256); } // a broken IFD offset
  return t;
}

function randomPicture(g) {
  switch (g.int(3)) {
    case 0: {
      const segs = [];
      const n = 1 + g.int(4);
      for (let i = 0; i < n; i++) {
        switch (g.int(6)) {
          case 0: segs.push(exifApp1(randomExif(g))); break;
          case 1: segs.push(xmpApp1(randomXmp(g))); break;
          case 2: segs.push(iptcApp13(Array.from({ length: 1 + g.int(4) }, () => [g.pick([90, 92, 101, 80, 5, 55, 65, 25, 7]), randomText(g, 4)]))); break;
          case 3: segs.push(seg(0xfe, randomText(g, 3))); break;
          case 4: segs.push(ICC()); break;
          default: segs.push(seg(0xe1, randomText(g, 3)));
        }
      }
      return jpeg(segs, g.chance(0.1) ? 'tail' : []);
    }
    case 1: {
      const chunks = [];
      const n = 1 + g.int(4);
      for (let i = 0; i < n; i++) {
        switch (g.int(6)) {
          case 0: chunks.push(chunk('eXIf', g.chance(0.5) ? randomExif(g) : cat('Exif\0\0', randomExif(g)))); break;
          case 1: chunks.push(textChunk(g.pick(['Author', 'title', 'Source', 'Software', 'Creation Time', 'Comment', 'Other', 'Raw profile type exif']), randomText(g, 4))); break;
          case 2: chunks.push(itxt(g.pick(['XML:com.adobe.xmp', 'Description', 'Raw profile type APP1']), randomXmp(g), g.chance(0.2))); break;
          case 3: chunks.push(textChunk('Raw profile type exif', rawProfile(randomExif(g)))); break;
          case 4: chunks.push(chunk('tIME', [7, 234, 1, 2, 3, 4, 5])); break;
          default: chunks.push(chunk('zTXt', cat('Author', [0, 0], filler(10, 9))));
        }
      }
      return png(...chunks);
    }
    default: {
      const chunks = [webpChunk('VP8L', [0x2f, 0, 0, 0])];
      if (g.chance(0.6)) chunks.push(webpChunk('EXIF', g.chance(0.5) ? randomExif(g) : cat('Exif\0\0', randomExif(g))));
      if (g.chance(0.5)) chunks.push(webpChunk('XMP ', utf8(randomXmp(g))));
      return webp(...chunks);
    }
  }
}

function randomVideo(g) {
  const ftyp = g.pick([ftypMp4, ftypQt, ftyp3gp, ftypM4a, ftypHeic])();
  const moov = [];
  const top = [];
  const n = g.int(4);
  for (let i = 0; i < n; i++) {
    switch (g.int(7)) {
      case 0: {
        const pairs = [];
        for (let k = 0; k < 1 + g.int(4); k++) {
          const [key, v] = g.pick(APPLE.concat(ANDROID));
          pairs.push([key, g.chance(0.5) ? v : (key.endsWith('ISO6709') ? randomCoordText(g) : randomText(g))]);
        }
        moov.push(appleMeta(pairs, g.chance(0.5)));
        break;
      }
      case 1: moov.push(bx('udta', cat(...Array.from({ length: 1 + g.int(3) }, () =>
        qtItem(g.pick(['\xA9xyz', '\xA9mak', '\xA9mod', '\xA9day', '\xA9ART', 'kywd', 'CAME', 'zzzz']), g.chance(0.5) ? randomCoordText(g) : randomText(g)))))); break;
      case 2: moov.push(bx('udta', loci(randomText(g, 3).replace(/[^\u0000-￿]/gu, '') || 'x', g.chance(0.3), (g.next() - 0.5) * 200, (g.next() - 0.5) * 400))); break;
      case 3: top.push(xmpUuid(randomXmp(g))); break;
      case 4: moov.push(trak(bx('udta', bx('XMP_', utf8(randomXmp(g)))))); break;
      case 5: moov.push(bx('udta', bx('meta', cat(zeros(4), bx('ilst', cat(...Array.from({ length: 1 + g.int(3) }, () =>
        ilstItem(g.pick(['\xA9xyz', '\xA9nam', '\xA9too', 'desc', 'qqqq']), g.chance(0.5) ? randomCoordText(g) : randomText(g))))))))); break;
      default: moov.push(bx('free', filler(10, 3)));
    }
  }
  let b = g.chance(0.1) ? cat(ftyp, mdat(), ...top) : movie(ftyp, moov, top, g.chance(0.5));
  if (g.chance(0.15)) b = b.subarray(0, g.int(b.length + 1));
  return b;
}

/** `n` random files: pictures, videos, and some with a few bytes flipped or cut short. */
function random(n, seed) {
  const g = rng(seed);
  const out = [];
  for (let i = 0; i < n; i++) {
    let b = g.chance(0.5) ? randomPicture(g) : randomVideo(g);
    if (g.chance(0.1)) {
      b = b.slice();
      for (let k = 0; k < 1 + g.int(4); k++) b[g.int(b.length)] = g.int(256);
    }
    if (g.chance(0.05)) b = b.subarray(0, g.int(b.length + 1));
    out.push([`random-${i}`, g.pick(['', 'x.jpg', 'clip.MOV', 'video/mp4; codecs=x', ' IMAGE/PNG ', 'a.b.heic', 'noext']), b]);
  }
  return out;
}

module.exports = { fixed, random, rng, utf8, cat };
