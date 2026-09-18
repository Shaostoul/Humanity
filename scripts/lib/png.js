// A small PNG codec for the dev rigs, so a script can COMPARE two screen
// snapshots pixel for pixel (and count a snapshot's colours) without a
// dependency. Node ships zlib; the rest of PNG is a CRC, a few chunks and
// five row filters, which is what this file is.
//
// decode(buffer) -> { width, height, rgba: Uint8Array }  8-bit RGB / RGBA /
//   gray / gray+alpha, non-interlaced (what the `image` crate writes for the
//   engine's screen snapshots). Anything else throws with the reason.
// encode({ width, height, rgba }) -> Buffer  8-bit RGBA, filter 0 per row.
//
// The helpers below them are the rig's actual questions: how many pixels
// differ between two images, and how many distinct colours one image has.

const zlib = require("zlib");

const SIGNATURE = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);

// CRC-32 over chunk type + data, as the PNG spec requires.
const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();
function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function paeth(a, b, c) {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  if (pa <= pb && pa <= pc) return a;
  return pb <= pc ? b : c;
}

function decode(buf) {
  if (!buf.subarray(0, 8).equals(SIGNATURE)) throw new Error("not a PNG (bad signature)");
  let pos = 8;
  let width = 0;
  let height = 0;
  let bitDepth = 0;
  let colorType = 0;
  let interlace = 0;
  const idat = [];
  while (pos < buf.length) {
    const len = buf.readUInt32BE(pos);
    const type = buf.toString("latin1", pos + 4, pos + 8);
    const data = buf.subarray(pos + 8, pos + 8 + len);
    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      bitDepth = data[8];
      colorType = data[9];
      interlace = data[12];
    } else if (type === "IDAT") {
      idat.push(data);
    } else if (type === "IEND") {
      break;
    }
    pos += 12 + len;
  }
  if (bitDepth !== 8) throw new Error(`unsupported bit depth ${bitDepth} (only 8)`);
  if (interlace !== 0) throw new Error("interlaced PNGs are not supported");
  const channels = { 0: 1, 2: 3, 4: 2, 6: 4 }[colorType];
  if (!channels) throw new Error(`unsupported colour type ${colorType} (gray, RGB, gray+alpha, RGBA only)`);
  const raw = zlib.inflateSync(Buffer.concat(idat));
  const stride = width * channels;
  const rgba = new Uint8Array(width * height * 4);
  let prev = new Uint8Array(stride);
  let off = 0;
  for (let y = 0; y < height; y++) {
    const filter = raw[off++];
    const row = new Uint8Array(raw.subarray(off, off + stride));
    off += stride;
    for (let i = 0; i < stride; i++) {
      const a = i >= channels ? row[i - channels] : 0;
      const b = prev[i];
      const c = i >= channels ? prev[i - channels] : 0;
      let v = row[i];
      switch (filter) {
        case 0: break;
        case 1: v += a; break;
        case 2: v += b; break;
        case 3: v += (a + b) >> 1; break;
        case 4: v += paeth(a, b, c); break;
        default: throw new Error(`bad row filter ${filter} at row ${y}`);
      }
      row[i] = v & 0xff;
    }
    for (let x = 0; x < width; x++) {
      const s = x * channels;
      const d = (y * width + x) * 4;
      if (channels === 1) {
        rgba[d] = rgba[d + 1] = rgba[d + 2] = row[s];
        rgba[d + 3] = 255;
      } else if (channels === 2) {
        rgba[d] = rgba[d + 1] = rgba[d + 2] = row[s];
        rgba[d + 3] = row[s + 1];
      } else if (channels === 3) {
        rgba[d] = row[s];
        rgba[d + 1] = row[s + 1];
        rgba[d + 2] = row[s + 2];
        rgba[d + 3] = 255;
      } else {
        rgba[d] = row[s];
        rgba[d + 1] = row[s + 1];
        rgba[d + 2] = row[s + 2];
        rgba[d + 3] = row[s + 3];
      }
    }
    prev = row;
  }
  return { width, height, rgba };
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const typeBuf = Buffer.from(type, "latin1");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])), 0);
  return Buffer.concat([len, typeBuf, data, crc]);
}

function encode({ width, height, rgba }) {
  if (rgba.length !== width * height * 4) throw new Error("rgba length does not match width x height x 4");
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  ihdr[10] = 0; // compression
  ihdr[11] = 0; // filter method
  ihdr[12] = 0; // no interlace
  const stride = width * 4;
  const raw = Buffer.alloc((stride + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (stride + 1)] = 0; // filter type 0 (none)
    Buffer.from(rgba.buffer, rgba.byteOffset + y * stride, stride).copy(raw, y * (stride + 1) + 1);
  }
  return Buffer.concat([SIGNATURE, chunk("IHDR", ihdr), chunk("IDAT", zlib.deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}

// How many pixels differ between two decoded images (any channel, any
// amount). Images of different sizes differ by every pixel of the larger.
function diffPixels(a, b) {
  if (a.width !== b.width || a.height !== b.height) {
    return { differing: Math.max(a.width * a.height, b.width * b.height), total: Math.max(a.width * a.height, b.width * b.height), sizeMismatch: true };
  }
  const total = a.width * a.height;
  let differing = 0;
  for (let i = 0; i < total; i++) {
    const o = i * 4;
    if (a.rgba[o] !== b.rgba[o] || a.rgba[o + 1] !== b.rgba[o + 1] || a.rgba[o + 2] !== b.rgba[o + 2] || a.rgba[o + 3] !== b.rgba[o + 3]) differing++;
  }
  return { differing, total, sizeMismatch: false };
}

// Distinct colours and the share of the single most common one. A blank
// screen is one colour at 100%; a page is many colours with the background
// well under 100%.
function colourStats(img) {
  const counts = new Map();
  const total = img.width * img.height;
  for (let i = 0; i < total; i++) {
    const o = i * 4;
    const key = (img.rgba[o] << 24) | (img.rgba[o + 1] << 16) | (img.rgba[o + 2] << 8) | img.rgba[o + 3];
    counts.set(key, (counts.get(key) || 0) + 1);
  }
  let top = 0;
  for (const n of counts.values()) if (n > top) top = n;
  return { distinct: counts.size, dominantShare: total ? top / total : 1, total };
}

module.exports = { decode, encode, diffPixels, colourStats, crc32 };
