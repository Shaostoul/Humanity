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
//   node scripts/obj-to-plant-gltf.js <file.obj> [<file.obj> ...]
//   node scripts/obj-to-plant-gltf.js --outdir assets/models/plants <file.obj> ...
//
// Output per input: assets/models/plants/<slug>/<slug>.gltf + .bin +
// <slug>_palette.png  (slug = lowercased file stem, e.g. Carrot_1 -> carrot_1).
//
// Palette notes:
// - Each material gets a 4x4-texel block in a square grid; UVs sit at block
//   centers and the glTF sampler is NEAREST, so colors never bleed.
// - Near-white low-saturation colors are darkened just below the engine's
//   white-key cutout threshold (assets/mod.rs white_key_alpha_if_cutout keys
//   near-white texels transparent for photo cutouts; a white palette block
//   would vanish). Clamp is invisible on these low-poly models.

const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

// ── CLI ──────────────────────────────────────────────────────────────
const args = process.argv.slice(2);
let outRoot = 'assets/models/plants';
const files = [];
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--outdir') { outRoot = args[++i]; continue; }
  files.push(args[i]);
}
if (files.length === 0) {
  console.error('usage: node scripts/obj-to-plant-gltf.js [--outdir DIR] <file.obj> ...');
  process.exit(1);
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

// ── MTL parser: name -> [r,g,b] 0..255 ───────────────────────────────
function parseMtl(mtlPath) {
  const colors = {};
  if (!fs.existsSync(mtlPath)) return colors;
  let cur = null;
  for (const line of fs.readFileSync(mtlPath, 'utf8').split(/\r?\n/)) {
    const t = line.trim();
    if (t.startsWith('newmtl ')) cur = t.slice(7).trim();
    else if (cur && t.startsWith('Kd ')) {
      let [r, g, b] = t.slice(3).trim().split(/\s+/).map(Number).map(v => Math.round((v ?? 0) * 255));
      // White-key dodge: keep near-white low-saturation colors below the
      // engine cutout threshold (min channel >= 210 && spread < 28 keys out).
      const mx = Math.max(r, g, b), mn = Math.min(r, g, b);
      if (mn >= 210 && mx - mn < 28) { const s = 205 / mn; r = Math.round(r * s); g = Math.round(g * s); b = Math.round(b * s); }
      colors[cur] = [r, g, b];
    }
  }
  return colors;
}

// ── OBJ -> single-primitive glTF ─────────────────────────────────────
function convert(objPath) {
  const stem = path.basename(objPath).replace(/\.obj$/i, '');
  const slug = stem.toLowerCase();
  const mtlColors = parseMtl(objPath.replace(/\.obj$/i, '.mtl'));
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
    asset: { version: '2.0', generator: 'obj-to-plant-gltf.js (HumanityOS, Quaternius CC0 source)' },
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

for (const f of files) convert(f);
