#!/usr/bin/env node
// obj-to-plant-gltf.js — convert a flat-colored OBJ+MTL model (the Quaternius
// CC0 packs ship Blend/FBX/OBJ, no glTF) into the engine's loader-ready shape:
// ONE mesh, ONE primitive, POSITION/NORMAL/TEXCOORD_0 + u32 indices, plus a
// tiny PALETTE texture that carries the MTL's per-material diffuse colors —
// each face's UVs point at its material's texel block, so the untextured
// low-poly model rides the exact same type-19 textured pipeline as the
// photoscanned plants (see src/assets/mod.rs parse_gltf_mesh_textured and
// scripts/repack-plant-gltf.js for the shape this mirrors).
//
// Usage:
//   node scripts/obj-to-plant-gltf.js --kd linear|srgb [--outdir DIR] <file.obj> ...
//   node scripts/obj-to-plant-gltf.js --kd linear|srgb --restamp <model.gltf> ...
//
// Output per input: assets/models/plants/<slug>/<slug>.gltf + .bin +
// <slug>_palette.png  (slug = lowercased file stem, e.g. Carrot_1 -> carrot_1).
//
// --kd says what the pack's MTL colours ARE, and there is no default because
// guessing wrong is invisible until the model is in the game (2026-10-03):
//   linear  the Kd numbers are linear light, as a Blender export writes them.
//           The Quaternius crop pack (assets/models/plants/) is this.
//   srgb    the Kd numbers are already display (sRGB) values.
//           The Kenney furniture kit (assets/models/furniture/) is this.
// The palette PNG is decoded by the GPU as sRGB (the engine uploads base
// colour textures as Rgba8UnormSrgb, and glTF requires a base colour
// texture's RGB to be sRGB-encoded), so a linear Kd has to be ENCODED to sRGB
// on its way into the PNG. Until 2026-10-03 every pack went in byte for byte,
// which left the crops nearly black: lettuce leaves (Kd 0.118, 0.133, 0.075)
// reached the shader at 1.5% luminance, beetroot at 0.3%, and the greenhouse
// towers grew black sprouts. Read as linear, the same numbers give beet red,
// straw-gold ripe wheat, crimson berries and the teal flower the plant notes
// in data/plants_visual.ron describe.
//
// --restamp fixes a model this converter made BEFORE --kd existed, from its
// own palette (the source OBJs are not kept in the repo): every texel byte b
// was round(Kd * 255), so Kd is b / 255 and the texel is rewritten the way
// --kd would have written it. The file is stamped `asset.extras.kd_space`,
// and a stamped file is refused, so a palette can never be encoded twice.
//
// Palette notes:
// - Each material gets a 4x4-texel block in a square grid; UVs sit at block
//   centers and the glTF sampler is NEAREST, so colors never bleed.
// - Near-white low-saturation colors are darkened just below the engine's
//   white-key cutout threshold (assets/mod.rs white_key_alpha_if_cutout keys
//   near-white texels transparent for photo cutouts; a white palette block
//   would vanish). Clamp is invisible on these low-poly models. The dodge is
//   applied to the bytes the PNG actually holds, after the sRGB encode,
//   because those are the bytes the engine's white key looks at.

const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

// What `asset.generator` says on every model this script writes. The pack
// a model came from is credited in assets/models/LICENSE-cc0-model-packs.md;
// this line used to name Quaternius on the Kenney furniture too.
const GENERATOR = 'obj-to-plant-gltf.js (HumanityOS)';
const KD_SPACES = ['linear', 'srgb'];

// ── Colour: MTL Kd (0..1) to the palette's sRGB bytes ────────────────
/** The sRGB transfer function: linear light 0..1 to an sRGB value 0..1. */
function srgbEncode(x) {
  const v = Math.min(1, Math.max(0, x));
  return v <= 0.0031308 ? v * 12.92 : 1.055 * Math.pow(v, 1 / 2.4) - 0.055;
}

/** Keep a near-white low-saturation colour just under the engine's white
 *  key (min channel >= 210 and spread < 28 keys out), so it is not cut
 *  away as photo background. Bytes in, bytes out. */
function dodgeWhiteKey([r, g, b]) {
  const mx = Math.max(r, g, b), mn = Math.min(r, g, b);
  if (mn >= 210 && mx - mn < 28) {
    const s = 205 / mn;
    return [Math.round(r * s), Math.round(g * s), Math.round(b * s)];
  }
  return [r, g, b];
}

/** One MTL Kd triple (0..1 each) as the three bytes its palette block
 *  holds, for a pack whose Kd is in `space` ('linear' or 'srgb'). */
function kdToBytes(kd, space) {
  if (!KD_SPACES.includes(space)) throw new Error(`kd space must be one of ${KD_SPACES.join(', ')}, got ${space}`);
  const enc = space === 'linear' ? srgbEncode : (v) => Math.min(1, Math.max(0, v));
  return dodgeWhiteKey(kd.map((v) => Math.round(enc(v ?? 0) * 255)));
}

// ── Minimal PNG writer (RGBA8) ───────────────────────────────────────
function crc32(buf) {
  let c, table = crc32.table;
  if (!table) {
    table = crc32.table = new Int32Array(256);
    for (let n = 0; n < 256; n++) {
      c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      table[n] = c;
    }
  }
  c = -1;
  for (let i = 0; i < buf.length; i++) c = (c >>> 8) ^ table[(c ^ buf[i]) & 0xff];
  return (c ^ -1) >>> 0;
}
function pngChunk(type, data) {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(body));
  return Buffer.concat([len, body, crc]);
}
function writePng(w, h, rgba) {
  const sig = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]);
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; ihdr[9] = 6; // 8-bit RGBA
  const raw = Buffer.alloc((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) {
    raw[y * (w * 4 + 1)] = 0; // filter: none
    rgba.copy(raw, y * (w * 4 + 1) + 1, y * w * 4, (y + 1) * w * 4);
  }
  const idat = zlib.deflateSync(raw);
  return Buffer.concat([sig, pngChunk('IHDR', ihdr), pngChunk('IDAT', idat), pngChunk('IEND', Buffer.alloc(0))]);
}

// ── Minimal PNG reader (8-bit RGB/RGBA, non-interlaced) ──────────────
// Only for --restamp, which reads palettes this script wrote. Undoes all five
// row filters so a palette re-saved by another tool still reads.
function readPng(buf) {
  let o = 8, w = 0, h = 0, colorType = 0, depth = 0, interlace = 0;
  const idat = [];
  while (o < buf.length) {
    const len = buf.readUInt32BE(o), type = buf.toString('ascii', o + 4, o + 8), d = buf.subarray(o + 8, o + 8 + len);
    if (type === 'IHDR') { w = d.readUInt32BE(0); h = d.readUInt32BE(4); depth = d[8]; colorType = d[9]; interlace = d[12]; }
    if (type === 'IDAT') idat.push(d);
    o += 12 + len;
  }
  if (depth !== 8 || (colorType !== 6 && colorType !== 2) || interlace !== 0) {
    throw new Error(`palette PNG must be 8-bit RGB or RGBA, non-interlaced (depth ${depth}, colour type ${colorType})`);
  }
  const bpp = colorType === 6 ? 4 : 3, stride = w * bpp;
  const raw = zlib.inflateSync(Buffer.concat(idat));
  const px = Buffer.alloc(h * stride);
  for (let y = 0; y < h; y++) {
    const filter = raw[y * (stride + 1)];
    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? px[y * stride + x - bpp] : 0;
      const b = y > 0 ? px[(y - 1) * stride + x] : 0;
      const c = x >= bpp && y > 0 ? px[(y - 1) * stride + x - bpp] : 0;
      let v = raw[y * (stride + 1) + 1 + x];
      if (filter === 1) v += a;
      else if (filter === 2) v += b;
      else if (filter === 3) v += (a + b) >> 1;
      else if (filter === 4) {
        const p = a + b - c, pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      px[y * stride + x] = v & 255;
    }
  }
  // Always hand back RGBA.
  if (bpp === 4) return { w, h, rgba: px };
  const rgba = Buffer.alloc(w * h * 4, 255);
  for (let i = 0; i < w * h; i++) px.copy(rgba, i * 4, i * 3, i * 3 + 3);
  return { w, h, rgba };
}

// ── MTL parser: name -> [r,g,b] 0..255 ───────────────────────────────
function parseMtl(mtlPath, kdSpace) {
  const colors = {};
  if (!fs.existsSync(mtlPath)) return colors;
  let cur = null;
  for (const line of fs.readFileSync(mtlPath, 'utf8').split(/\r?\n/)) {
    const t = line.trim();
    if (t.startsWith('newmtl ')) cur = t.slice(7).trim();
    else if (cur && t.startsWith('Kd ')) {
      colors[cur] = kdToBytes(t.slice(3).trim().split(/\s+/).map(Number), kdSpace);
    }
  }
  return colors;
}

/** --restamp: rewrite the palette of a model converted before --kd existed
 *  as if it had been converted with `kdSpace`, and stamp it. Returns a line
 *  saying what changed. Throws on a model already stamped. */
function restamp(gltfPath, kdSpace) {
  if (!KD_SPACES.includes(kdSpace)) throw new Error(`kd space must be one of ${KD_SPACES.join(', ')}, got ${kdSpace}`);
  const gltf = JSON.parse(fs.readFileSync(gltfPath, 'utf8'));
  if (!/^obj-to-plant-gltf\.js/.test(gltf.asset?.generator || '')) throw new Error(`${gltfPath}: not made by this converter`);
  const stamped = gltf.asset?.extras?.kd_space;
  if (stamped) throw new Error(`${gltfPath}: already stamped kd_space ${stamped}; restamping would encode it twice`);
  const pngPath = path.join(path.dirname(gltfPath), gltf.images[0].uri);
  const { w, h, rgba } = readPng(fs.readFileSync(pngPath));
  const seen = new Map();
  let changed = false;
  for (let i = 0; i < w * h; i++) {
    const o = i * 4;
    // Pure white is the converter's unused-block fill (Buffer.alloc(.., 255)),
    // never a material colour: a white Kd was dodged to 205 on the way in.
    if (rgba[o] === 255 && rgba[o + 1] === 255 && rgba[o + 2] === 255) continue;
    const before = [rgba[o], rgba[o + 1], rgba[o + 2]];
    const after = kdToBytes(before.map((b) => b / 255), kdSpace);
    changed ||= after.some((b, k) => b !== before[k]);
    rgba[o] = after[0]; rgba[o + 1] = after[1]; rgba[o + 2] = after[2];
    seen.set(before.join(','), after.join(','));
  }
  // An sRGB pack's palette comes back byte for byte: leave its file alone.
  if (changed) fs.writeFileSync(pngPath, writePng(w, h, rgba));
  gltf.asset.generator = GENERATOR;
  gltf.asset.extras = { ...(gltf.asset.extras || {}), kd_space: kdSpace };
  fs.writeFileSync(gltfPath, JSON.stringify(gltf));
  return `${path.basename(gltfPath)}: kd ${kdSpace}, ${[...seen].map(([a, b]) => `(${a}) -> (${b})`).join('  ')}`;
}

// ── OBJ -> single-primitive glTF ─────────────────────────────────────
function convert(objPath, kdSpace, outRoot) {
  const stem = path.basename(objPath).replace(/\.obj$/i, '');
  const slug = stem.toLowerCase();
  const mtlColors = parseMtl(objPath.replace(/\.obj$/i, '.mtl'), kdSpace);
  const matNames = Object.keys(mtlColors);
  if (matNames.length === 0) matNames.push('__default');

  // Palette grid: each material owns a 4x4 block in a square-ish grid.
  const cols = Math.ceil(Math.sqrt(matNames.length));
  const rows = Math.ceil(matNames.length / cols);
  const pw = cols * 4, ph = rows * 4;
  const px = Buffer.alloc(pw * ph * 4, 255);
  const matUv = {};
  matNames.forEach((name, i) => {
    const [r, g, b] = mtlColors[name] || [120, 140, 90];
    const bx = (i % cols) * 4, by = Math.floor(i / cols) * 4;
    for (let y = by; y < by + 4; y++) for (let x = bx; x < bx + 4; x++) {
      const o = (y * pw + x) * 4;
      px[o] = r; px[o + 1] = g; px[o + 2] = b; px[o + 3] = 255;
    }
    matUv[name] = [(bx + 2) / pw, (by + 2) / ph];
  });

  // Parse the OBJ.
  const vs = [], vns = [];
  const verts = [];  // welded [x,y,z, nx,ny,nz, u,v]
  const indices = [];
  const weld = new Map();
  let curMat = matNames[0];
  const src = fs.readFileSync(objPath, 'utf8');
  for (const line of src.split(/\r?\n/)) {
    const t = line.trim();
    if (t.startsWith('v ')) vs.push(t.slice(2).trim().split(/\s+/).map(Number));
    else if (t.startsWith('vn ')) vns.push(t.slice(3).trim().split(/\s+/).map(Number));
    else if (t.startsWith('usemtl ')) { const n = t.slice(7).trim(); curMat = matUv[n] ? n : matNames[0]; }
    else if (t.startsWith('f ')) {
      const refs = t.slice(2).trim().split(/\s+/).map(r => {
        const p = r.split('/');
        return [parseInt(p[0], 10) - 1, p[2] ? parseInt(p[2], 10) - 1 : -1];
      });
      // Triangulate the polygon as a fan; compute a face normal fallback.
      let fn = null;
      for (let i = 1; i + 1 < refs.length; i++) {
        const tri = [refs[0], refs[i], refs[i + 1]];
        if (tri.some(([vi, ni]) => ni < 0) && !fn) {
          const [a, b, c] = tri.map(([vi]) => vs[vi]);
          const u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
          const w = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
          fn = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
          const l = Math.hypot(...fn) || 1;
          fn = fn.map(v => v / l);
        }
        for (const [vi, ni] of tri) {
          const n = ni >= 0 ? vns[ni] : fn;
          const [u0, v0] = matUv[curMat];
          const key = vi + '/' + (ni >= 0 ? ni : 'f' + fn.map(v => v.toFixed(3)).join(',')) + '/' + curMat;
          let idx = weld.get(key);
          if (idx === undefined) {
            idx = verts.length;
            verts.push([...vs[vi], ...n, u0, v0]);
            weld.set(key, idx);
          }
          indices.push(idx);
        }
      }
    }
  }
  if (verts.length === 0) { console.error(`${stem}: no geometry, skipped`); return; }

  // Build the .bin: positions, normals, uvs, then u32 indices.
  const vcount = verts.length;
  const fpos = new Float32Array(vcount * 3), fnorm = new Float32Array(vcount * 3), fuv = new Float32Array(vcount * 2);
  const mins = [Infinity, Infinity, Infinity], maxs = [-Infinity, -Infinity, -Infinity];
  verts.forEach((v, i) => {
    for (let k = 0; k < 3; k++) {
      fpos[i * 3 + k] = v[k];
      if (v[k] < mins[k]) mins[k] = v[k];
      if (v[k] > maxs[k]) maxs[k] = v[k];
      fnorm[i * 3 + k] = v[3 + k];
    }
    fuv[i * 2] = v[6]; fuv[i * 2 + 1] = v[7];
  });
  const idxArr = new Uint32Array(indices);
  const posB = Buffer.from(fpos.buffer), normB = Buffer.from(fnorm.buffer), uvB = Buffer.from(fuv.buffer), idxB = Buffer.from(idxArr.buffer);
  const bin = Buffer.concat([posB, normB, uvB, idxB]);

  const outDir = path.join(outRoot, slug);
  fs.mkdirSync(outDir, { recursive: true });
  fs.writeFileSync(path.join(outDir, `${slug}.bin`), bin);
  fs.writeFileSync(path.join(outDir, `${slug}_palette.png`), writePng(pw, ph, px));

  const gltf = {
    asset: { version: '2.0', generator: GENERATOR, extras: { kd_space: kdSpace } },
    scene: 0,
    scenes: [{ nodes: [0] }],
    nodes: [{ mesh: 0, name: slug }],
    meshes: [{ name: slug, primitives: [{ attributes: { POSITION: 0, NORMAL: 1, TEXCOORD_0: 2 }, indices: 3, material: 0 }] }],
    materials: [{ name: 'palette', pbrMetallicRoughness: { baseColorTexture: { index: 0 }, metallicFactor: 0.0, roughnessFactor: 0.9 } }],
    textures: [{ sampler: 0, source: 0 }],
    samplers: [{ magFilter: 9728, minFilter: 9728, wrapS: 33071, wrapT: 33071 }],
    images: [{ uri: `${slug}_palette.png` }],
    buffers: [{ uri: `${slug}.bin`, byteLength: bin.length }],
    bufferViews: [
      { buffer: 0, byteOffset: 0, byteLength: posB.length },
      { buffer: 0, byteOffset: posB.length, byteLength: normB.length },
      { buffer: 0, byteOffset: posB.length + normB.length, byteLength: uvB.length },
      { buffer: 0, byteOffset: posB.length + normB.length + uvB.length, byteLength: idxB.length },
    ],
    accessors: [
      { bufferView: 0, componentType: 5126, count: vcount, type: 'VEC3', min: mins, max: maxs },
      { bufferView: 1, componentType: 5126, count: vcount, type: 'VEC3' },
      { bufferView: 2, componentType: 5126, count: vcount, type: 'VEC2' },
      { bufferView: 3, componentType: 5125, count: idxArr.length, type: 'SCALAR' },
    ],
  };
  fs.writeFileSync(path.join(outDir, `${slug}.gltf`), JSON.stringify(gltf));
  console.log(`${stem} -> ${outDir}/${slug}.gltf  (${vcount} verts, ${idxArr.length / 3} tris, ${matNames.length} colors)`);
}

// ── CLI ──────────────────────────────────────────────────────────────
function main(argv) {
  let outRoot = 'assets/models/plants';
  let kdSpace = null;
  let restampMode = false;
  const files = [];
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--outdir') { outRoot = argv[++i]; continue; }
    if (argv[i] === '--kd') { kdSpace = argv[++i]; continue; }
    if (argv[i] === '--restamp') { restampMode = true; continue; }
    files.push(argv[i]);
  }
  if (files.length === 0 || !KD_SPACES.includes(kdSpace)) {
    console.error('usage: node scripts/obj-to-plant-gltf.js --kd linear|srgb [--outdir DIR] <file.obj> ...');
    console.error('       node scripts/obj-to-plant-gltf.js --kd linear|srgb --restamp <model.gltf> ...');
    console.error('--kd is required: linear for a Blender export (the Quaternius crops), srgb for');
    console.error('display values (the Kenney furniture). See the header of this script.');
    process.exit(1);
  }
  let failed = 0;
  for (const f of files) {
    if (!restampMode) { convert(f, kdSpace, outRoot); continue; }
    try {
      console.log(restamp(f, kdSpace));
    } catch (e) {
      console.error(e.message);
      failed++;
    }
  }
  if (failed) process.exit(2);
}

module.exports = { srgbEncode, dodgeWhiteKey, kdToBytes, readPng, writePng, restamp, GENERATOR };

if (require.main === module) main(process.argv.slice(2));
