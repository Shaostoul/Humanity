// pack.js - the pure node step of the SimReady pipeline (docs/design/simready-props.md, section 4,
// step 4): from the bake's raw files to what ships.
//
// For each asset it writes, into assets/models/simready/<folder lowercased>/ (the home set) or the
// Medical Bay pack's staging folder (E:\HumanityOS\simready-packs\simready-medical\):
//   <folder>.glb   ONE binary glTF: mesh 0 = the baked model (what the engine's machine path draws,
//                  its first mesh's first primitive), mesh 1 = its quarter-triangle distance
//                  version (same material, textures stored once), then the collider hulls as
//                  meshes no scene node draws, flagged in `extras`. One material: base colour
//                  (sRGB, RGBA so the engine's white key never touches it), normal, and ORM
//                  (R occlusion, G roughness, B metallic, the glTF packing).
//   LICENSE.txt    NVIDIA's own CC BY 4.0 legal code, byte for byte, checked by SHA-256.
//   SOURCE.md      the credit the finding asks for, with every change made.
//   physics.ron    the neutral UsdPhysics values, in the model's own frame.
// and the set's manifest.json and assets/models/LICENSE-nvidia-simready.md.
//
// Deterministic by construction: fixed key order, no timestamps, vertices deduplicated in first-seen
// order, PNG rows filtered by a fixed rule and deflated by node's zlib at level 9 (the node and zlib
// versions are recorded in the manifest, since another zlib may deflate differently).

'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const zlib = require('zlib');
const crypto = require('crypto');

/** SHA-256 of Creative Commons' CC BY 4.0 legal code, the text every asset's LICENSE.txt carries
 *  (docs/reference/findings/2026-11-08-nvidia-simready-assets.md, F2 and F3). */
const CC_BY_40_SHA256 = '9ba9550ad48438d0836ddab3da480b3b69ffa0aac7b7878b5a0039e7ab429411';
const GENERATOR = 'simready pack.js (HumanityOS)';
const DATASET = 'https://huggingface.co/datasets/nvidia/simready-assets';
const LICENCE_URL = 'https://creativecommons.org/licenses/by/4.0/';
const PACK_ID = 'simready-medical';
const PACK_VERSION = 1;
/** Folder names never used: named after other companies' trademarked products (the finding, F3). */
const DENY = /wd40|rubikscube/i;

// ---------------------------------------------------------------------------------------------
// PNG
// ---------------------------------------------------------------------------------------------

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

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
}

/** Encode 8-bit pixels (`channels` 3 or 4, row 0 at the top) as PNG. Each row takes the filter
 *  (of the five) with the smallest sum of absolute values, the usual heuristic, so the bytes are
 *  a pure function of the pixels. */
function encodePng(pixels, width, height, channels) {
  const stride = width * channels;
  const out = Buffer.alloc((stride + 1) * height);
  const prev = Buffer.alloc(stride);
  const cand = [0, 1, 2, 3, 4].map(() => Buffer.alloc(stride));
  for (let y = 0; y < height; y++) {
    const row = pixels.subarray(y * stride, (y + 1) * stride);
    const up = y > 0 ? pixels.subarray((y - 1) * stride, y * stride) : prev;
    let best = 0;
    let bestSum = Infinity;
    for (let f = 0; f < 5; f++) {
      const c = cand[f];
      let sum = 0;
      for (let i = 0; i < stride; i++) {
        const a = i >= channels ? row[i - channels] : 0;
        const b = up[i];
        const cc = i >= channels ? up[i - channels] : 0;
        let pred;
        if (f === 0) pred = 0;
        else if (f === 1) pred = a;
        else if (f === 2) pred = b;
        else if (f === 3) pred = (a + b) >> 1;
        else {
          const p = a + b - cc;
          const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - cc);
          pred = pa <= pb && pa <= pc ? a : pb <= pc ? b : cc;
        }
        const v = (row[i] - pred) & 0xff;
        c[i] = v;
        sum += v < 128 ? v : 256 - v;
      }
      if (sum < bestSum) { bestSum = sum; best = f; }
    }
    out[y * (stride + 1)] = best;
    cand[best].copy(out, y * (stride + 1) + 1);
  }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = channels === 4 ? 6 : 2;
  ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', zlib.deflateSync(out, { level: 9, memLevel: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

const srgb = v => {
  const c = Math.min(1, Math.max(0, v));
  return c <= 0.0031308 ? c * 12.92 : 1.055 * Math.pow(c, 1 / 2.4) - 0.055;
};
const byte = v => Math.round(Math.min(1, Math.max(0, v)) * 255);

/** A baked float32 RGBA image (Blender's row 0 is the BOTTOM) as 8-bit rows, top first. */
function readImage(file, w, h) {
  const buf = fs.readFileSync(file);
  if (buf.length !== w * h * 16) throw new Error(`${file}: ${buf.length} bytes, expected ${w * h * 16}`);
  const f = new Float32Array(buf.buffer, buf.byteOffset, w * h * 4);
  return { f, w, h, at: (x, y, c) => f[((h - 1 - y) * w + x) * 4 + c] };
}

/** The baked maps as 8-bit pixels, rows top first: { px, w, h, ch }. */
function albedoPixels(img, scale) {
  const w = img.w / scale, h = img.h / scale;
  const px = Buffer.alloc(w * h * 4);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    for (let c = 0; c < 3; c++) {
      let s = 0;
      for (let j = 0; j < scale; j++) for (let i = 0; i < scale; i++) s += img.at(x * scale + i, y * scale + j, c);
      px[(y * w + x) * 4 + c] = byte(srgb(s / (scale * scale)));
    }
    px[(y * w + x) * 4 + 3] = 255;
  }
  return { px, w, h, ch: 4 };
}

function normalPixels(img) {
  const px = Buffer.alloc(img.w * img.h * 3);
  for (let y = 0; y < img.h; y++) for (let x = 0; x < img.w; x++) {
    let nx = img.at(x, y, 0) * 2 - 1, ny = img.at(x, y, 1) * 2 - 1, nz = img.at(x, y, 2) * 2 - 1;
    const len = Math.hypot(nx, ny, nz);
    if (len < 1e-6) { nx = 0; ny = 0; nz = 1; } else { nx /= len; ny /= len; nz /= len; }
    const o = (y * img.w + x) * 3;
    px[o] = byte(nx * 0.5 + 0.5); px[o + 1] = byte(ny * 0.5 + 0.5); px[o + 2] = byte(nz * 0.5 + 0.5);
  }
  return { px, w: img.w, h: img.h, ch: 3 };
}

function ormPixels(occ, rough, metal) {
  const px = Buffer.alloc(occ.w * occ.h * 3);
  for (let y = 0; y < occ.h; y++) for (let x = 0; x < occ.w; x++) {
    const o = (y * occ.w + x) * 3;
    px[o] = byte(occ.at(x, y, 0)); px[o + 1] = byte(rough.at(x, y, 0)); px[o + 2] = byte(metal.at(x, y, 0));
  }
  return { px, w: occ.w, h: occ.h, ch: 3 };
}

// The brand check's repaints of the BAKE itself (`"stage": "baked"` in brand_check.repaints). A
// maker's name moulded into a model's GEOMETRY is in no source texture, only in what the bake
// projects from the detailed mesh: the step ladder's ASSURED INDUSTRIES letters in orange rubber
// on its treads, the screwdriver's VORTEN, the utility knife's DRAVOX (found 2026-11-09 in the
// homestead captures and a sheet of every baked base colour, after the texture-based check had
// passed them). So it is painted out of the baked maps:
//   { "stage": "baked", "rects": [[x0, y0, x1, y1], ...], "key": [r, g, b], "tol": n, "sample": [x, y] }
// in the base colour map's own pixels (row 0 at the top; no rects means the whole map). A pixel
// inside the rects whose base colour is within `tol` of `key` (the largest channel difference)
// takes the sample pixel's value, in the base colour, normal and ORM maps alike (their coordinates
// scaled from the base colour's), so the letters leave no relief behind for a later rung's normal
// mapping either. Pure arithmetic on the bake, so the output stays byte-stable.
function bakedRepaints(a) {
  return ((a.brand_check && a.brand_check.repaints) || []).filter(r => r.stage === 'baked');
}

function applyBakedRepaints(maps, specs) {
  const al = maps.albedo;
  for (const s of specs) {
    const mask = new Uint8Array(al.w * al.h);
    for (const [x0, y0, x1, y1] of s.rects || [[0, 0, al.w, al.h]]) {
      for (let y = Math.max(0, y0); y < Math.min(al.h, y1); y++) for (let x = Math.max(0, x0); x < Math.min(al.w, x1); x++) {
        const o = (y * al.w + x) * al.ch;
        const d = Math.max(Math.abs(al.px[o] - s.key[0]), Math.abs(al.px[o + 1] - s.key[1]), Math.abs(al.px[o + 2] - s.key[2]));
        if (d <= s.tol) mask[y * al.w + x] = 1;
      }
    }
    for (const m of Object.values(maps)) {
      const sx = Math.min(m.w - 1, Math.floor((s.sample[0] * m.w) / al.w));
      const sy = Math.min(m.h - 1, Math.floor((s.sample[1] * m.h) / al.h));
      const sv = Buffer.from(m.px.subarray((sy * m.w + sx) * m.ch, (sy * m.w + sx + 1) * m.ch));
      for (let y = 0; y < m.h; y++) for (let x = 0; x < m.w; x++) {
        if (!mask[Math.floor((y * al.h) / m.h) * al.w + Math.floor((x * al.w) / m.w)]) continue;
        sv.copy(m.px, (y * m.w + x) * m.ch);
      }
    }
  }
}

// ---------------------------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------------------------

/** Blender Z-up to glTF Y-up: (x, y, z) -> (x, z, -y). */
const zup = (x, y, z) => [x, z, -y];

function readMesh(file) {
  const buf = fs.readFileSync(file);
  return new Float32Array(buf.buffer, buf.byteOffset, buf.length / 4);
}

/** Corners (8 floats each, Blender space) to an indexed mesh in glTF space, moved by `place`. */
function indexMesh(corners, place) {
  const f32 = new Float32Array(8);
  const key = new Uint8Array(f32.buffer);
  const map = new Map();
  const verts = [];
  const indices = [];
  for (let i = 0; i < corners.length; i += 8) {
    const p = place(zup(corners[i], corners[i + 1], corners[i + 2]));
    const n = place.turn(zup(corners[i + 3], corners[i + 4], corners[i + 5]));
    f32[0] = p[0]; f32[1] = p[1]; f32[2] = p[2];
    f32[3] = n[0]; f32[4] = n[1]; f32[5] = n[2];
    f32[6] = corners[i + 6]; f32[7] = 1 - corners[i + 7];
    const k = Buffer.from(key).toString('latin1');
    let idx = map.get(k);
    if (idx === undefined) {
      idx = verts.length;
      map.set(k, idx);
      verts.push(Float32Array.from(f32));
    }
    indices.push(idx);
  }
  return { verts, indices };
}

function boundsOf(corners) {
  const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < corners.length; i += 8) {
    const p = zup(corners[i], corners[i + 1], corners[i + 2]);
    for (let k = 0; k < 3; k++) { lo[k] = Math.min(lo[k], p[k]); hi[k] = Math.max(hi[k], p[k]); }
  }
  return { lo, hi };
}

// ---------------------------------------------------------------------------------------------
// GLB
// ---------------------------------------------------------------------------------------------

class Bin {
  constructor() { this.parts = []; this.length = 0; this.views = []; this.accessors = []; }
  view(buf, target) {
    const pad = (4 - (this.length % 4)) % 4;
    if (pad) { this.parts.push(Buffer.alloc(pad)); this.length += pad; }
    const v = { buffer: 0, byteOffset: this.length, byteLength: buf.length };
    if (target) v.target = target;
    this.parts.push(buf);
    this.length += buf.length;
    this.views.push(v);
    return this.views.length - 1;
  }
  accessor(a) { this.accessors.push(a); return this.accessors.length - 1; }
}

function addPrimitive(bin, mesh, material) {
  const n = mesh.verts.length;
  const pos = Buffer.alloc(n * 12), nor = Buffer.alloc(n * 12), uv = Buffer.alloc(n * 8);
  const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  mesh.verts.forEach((v, i) => {
    for (let k = 0; k < 3; k++) {
      pos.writeFloatLE(v[k], i * 12 + k * 4);
      nor.writeFloatLE(v[3 + k], i * 12 + k * 4);
      lo[k] = Math.min(lo[k], v[k]); hi[k] = Math.max(hi[k], v[k]);
    }
    uv.writeFloatLE(v[6], i * 8); uv.writeFloatLE(v[7], i * 8 + 4);
  });
  const wide = n > 65535;
  const idx = Buffer.alloc(mesh.indices.length * (wide ? 4 : 2));
  mesh.indices.forEach((x, i) => (wide ? idx.writeUInt32LE(x, i * 4) : idx.writeUInt16LE(x, i * 2)));
  const round = a => a.map(x => Math.fround(x));
  const attributes = {
    POSITION: bin.accessor({ bufferView: bin.view(pos, 34962), componentType: 5126, count: n, type: 'VEC3', min: round(lo), max: round(hi) }),
    NORMAL: bin.accessor({ bufferView: bin.view(nor, 34962), componentType: 5126, count: n, type: 'VEC3' }),
    TEXCOORD_0: bin.accessor({ bufferView: bin.view(uv, 34962), componentType: 5126, count: n, type: 'VEC2' }),
  };
  const indices = bin.accessor({ bufferView: bin.view(idx, 34963), componentType: wide ? 5125 : 5123, count: mesh.indices.length, type: 'SCALAR' });
  const prim = { attributes, indices, mode: 4 };
  if (material !== undefined) prim.material = material;
  return prim;
}

function addHull(bin, hull) {
  const pos = Buffer.alloc(hull.vertices.length * 12);
  const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  hull.vertices.forEach((v, i) => v.forEach((x, k) => {
    pos.writeFloatLE(x, i * 12 + k * 4);
    lo[k] = Math.min(lo[k], Math.fround(x)); hi[k] = Math.max(hi[k], Math.fround(x));
  }));
  const tris = hull.triangles.flat();
  const idx = Buffer.alloc(tris.length * 2);
  tris.forEach((x, i) => idx.writeUInt16LE(x, i * 2));
  return {
    attributes: { POSITION: bin.accessor({ bufferView: bin.view(pos, 34962), componentType: 5126, count: hull.vertices.length, type: 'VEC3', min: lo, max: hi }) },
    indices: bin.accessor({ bufferView: bin.view(idx, 34963), componentType: 5123, count: tris.length, type: 'SCALAR' }),
    mode: 4,
  };
}

function writeGlb({ name, lod1, lod2, hulls, pngs, extras }) {
  const bin = new Bin();
  const images = ['albedo', 'normal', 'orm'].map(k => ({ bufferView: bin.view(pngs[k]), mimeType: 'image/png', name: `${name}_${k}` }));
  const meshes = [
    { name: `${name}_lod1`, primitives: [addPrimitive(bin, lod1, 0)] },
    { name: `${name}_lod2`, primitives: [addPrimitive(bin, lod2, 0)] },
  ];
  const nodes = [
    { name: `${name}_lod1`, mesh: 0 },
    { name: `${name}_lod2`, mesh: 1, extras: { lod: 2, note: 'distance version, same material; not in the scene' } },
  ];
  hulls.forEach((h, i) => {
    meshes.push({ name: `${name}_collider_${i}`, primitives: [addHull(bin, h)] });
    nodes.push({ name: `${name}_collider_${i}`, mesh: meshes.length - 1, extras: { collider: h.approximation, source_prim: h.collider, body: h.body } });
  });
  const json = {
    asset: {
      version: '2.0',
      generator: GENERATOR,
      copyright: 'Based on NVIDIA SimReady Assets, by NVIDIA, licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/); converted and modified by HumanityOS. See SOURCE.md.',
      extras,
    },
    scene: 0,
    scenes: [{ name, nodes: [0] }],
    nodes,
    meshes,
    materials: [{
      name: `${name}_baked`,
      pbrMetallicRoughness: { baseColorTexture: { index: 0 }, metallicFactor: 1, roughnessFactor: 1, metallicRoughnessTexture: { index: 2 } },
      normalTexture: { index: 1 },
      occlusionTexture: { index: 2 },
    }],
    textures: [0, 1, 2].map(i => ({ sampler: 0, source: i })),
    samplers: [{ magFilter: 9729, minFilter: 9987, wrapS: 33071, wrapT: 33071 }],
    images,
    accessors: bin.accessors,
    bufferViews: bin.views,
    buffers: [{ byteLength: 0 }],
  };
  const binBuf = Buffer.concat(bin.parts);
  const binPad = Buffer.concat([binBuf, Buffer.alloc((4 - (binBuf.length % 4)) % 4)]);
  json.buffers[0].byteLength = binPad.length;
  let jsonBuf = Buffer.from(JSON.stringify(json), 'utf8');
  jsonBuf = Buffer.concat([jsonBuf, Buffer.alloc((4 - (jsonBuf.length % 4)) % 4, 0x20)]);
  const header = Buffer.alloc(12);
  header.writeUInt32LE(0x46546c67, 0);
  header.writeUInt32LE(2, 4);
  header.writeUInt32LE(12 + 8 + jsonBuf.length + 8 + binPad.length, 8);
  const jh = Buffer.alloc(8); jh.writeUInt32LE(jsonBuf.length, 0); jh.writeUInt32LE(0x4e4f534a, 4);
  const bh = Buffer.alloc(8); bh.writeUInt32LE(binPad.length, 0); bh.writeUInt32LE(0x004e4942, 4);
  return Buffer.concat([header, jh, jsonBuf, bh, binPad]);
}

// ---------------------------------------------------------------------------------------------
// physics.ron, SOURCE.md
// ---------------------------------------------------------------------------------------------

const f6 = x => (x === null || x === undefined || !Number.isFinite(x) ? 'None' : `Some(${Number(x.toFixed(6))})`);
const v3 = v => (v ? `Some((${v.map(x => Number(x.toFixed(6))).join(', ')}))` : 'None');
const str = s => JSON.stringify(s == null ? '' : String(s));
const last = p => (p ? p.split('/').pop() : '');

/** The model's physics, from extract.json's neutral UsdPhysics values, in the model's own frame. */
function physicsRon(a, x, place, hullIndex) {
  const P = x.physics;
  const bodies = P.bodies.map(b => {
    const cols = P.colliders.filter(c => c.body === b.path);
    let mass = b.mass && b.mass.mass_kg;
    if (mass == null) {
      const ms = cols.map(c => c.mass && c.mass.mass_kg).filter(m => m != null);
      mass = ms.length ? ms.reduce((s, m) => s + m, 0) : null;
    }
    return { b, mass };
  });
  const known = bodies.map(x => x.mass).filter(m => m != null);
  const total = known.length ? known.reduce((s, m) => s + m, 0) : null;
  // The whole model's centre of mass: the mass-weighted mean of every centre given (a body's own,
  // else its colliders'), moved into the model's frame.
  const parts = [];
  for (const b of P.bodies) {
    if (b.mass && b.mass.mass_kg != null && b.mass.center_of_mass_m) parts.push(b.mass);
    else for (const c of P.colliders.filter(c => c.body === b.path)) if (c.mass && c.mass.mass_kg != null && c.mass.center_of_mass_m) parts.push(c.mass);
  }
  const msum = parts.reduce((s, p) => s + p.mass_kg, 0);
  const com = msum > 0 ? { center_of_mass_m: [0, 1, 2].map(k => parts.reduce((s, p) => s + p.mass_kg * p.center_of_mass_m[k], 0) / msum) } : null;
  const lines = [];
  lines.push(`// ${a.folder}: its physics, generated by scripts/simready/pack.js from the neutral UsdPhysics layer of`);
  lines.push('// the NVIDIA SimReady asset (CC BY 4.0, see SOURCE.md and LICENSE.txt beside this file). Metres,');
  lines.push("// kilograms, Y up, in the model's own frame: its footprint centred on the origin, its base on y = 0.");
  lines.push('// Read by src/assets/model_physics.rs. Do not edit by hand: `just simready` writes it.');
  lines.push('(');
  lines.push(`    source: ${str(a.folder)},`);
  lines.push(`    package_id: ${str(x.package_id)},`);
  lines.push(`    mass_kg: ${f6(total)},`);
  lines.push(`    center_of_mass_m: ${v3(com && com.center_of_mass_m ? place(com.center_of_mass_m) : null)},`);
  lines.push('    bodies: [');
  for (const { b, mass } of bodies) {
    const cols = P.colliders.filter(c => c.body === b.path);
    const m = b.mass || (cols.length === 1 && cols[0].mass) || {};
    lines.push(`        (name: ${str(last(b.path))}, mass_kg: ${f6(mass)}, kinematic: ${b.kinematic}, principal_inertia_kg_m2: ${v3(m.diagonal_inertia_kg_m2 || null)}),`);
  }
  lines.push('    ],');
  lines.push('    colliders: [');
  for (const c of P.colliders) {
    const pm = (c.physics_material && P.materials[c.physics_material]) || {};
    const hi = hullIndex.get(c.path);
    lines.push(`        (body: ${str(last(c.body))}, part: ${str(last(c.path))}, approximation: ${str(c.approximation)}, hull_mesh: ${hi === undefined ? 'None' : `Some(${hi})`}, friction: ${f6(pm.dynamic_friction)}, static_friction: ${f6(pm.static_friction)}, restitution: ${f6(pm.restitution)}),`);
  }
  lines.push('    ],');
  lines.push('    joints: [');
  for (const j of P.joints) {
    const lim = j.lower_deg !== undefined ? `lower: ${f6(j.lower_deg)}, upper: ${f6(j.upper_deg)}, units: "deg"` : j.lower_m !== undefined ? `lower: ${f6(j.lower_m)}, upper: ${f6(j.upper_m)}, units: "m"` : 'lower: None, upper: None, units: ""';
    lines.push(`        (kind: ${str(j.type)}, body0: ${str(last(j.body0))}, body1: ${str(last(j.body1))}, axis: ${str(j.axis || '')}, ${lim}),`);
  }
  lines.push('    ],');
  lines.push(')');
  return { text: lines.join('\n') + '\n', mass: total };
}

function sourceMd(a, x, b, info) {
  const changes = [
    'converted from OpenUSD to binary glTF',
    `every part joined into one mesh and simplified from ${b.source_triangles} to ${b.meshes.lod1.triangles} triangles, with a ${b.meshes.lod2.triangles}-triangle distance version`,
    `its materials baked into one texture set (base colour ${info.albedo_px} px, normal and ORM ${info.pbr_px} px) on a new UV layout`,
    'moved so its footprint is centred on the origin and its base sits on the floor' + (info.turn ? `, and turned ${info.turn} degrees about the vertical` : ''),
    'convex collider hulls computed from its collision parts, and its physics values carried into physics.ron',
  ];
  if (a.brand_check && a.brand_check.verdict === 'repaint') changes.push(`repainted: ${a.brand_check.notes}`);
  for (const t of x.textures_outside || []) changes.push(`its materials name \`${t}\`, outside its package's Textures folder and not in the download, so that surface is a plain grey`);
  return [
    `# ${a.folder}`,
    '',
    `Based on \`${a.folder}\` from NVIDIA SimReady Assets (${DATASET}), package`,
    `\`${x.package_id}\`, by NVIDIA, licensed under CC BY 4.0 (${LICENCE_URL}).`,
    '',
    'Converted and modified by HumanityOS with `just simready` (scripts/simready/):',
    '',
    ...changes.map((c, i) => `- ${c}${i === changes.length - 1 ? '.' : ';'}`),
    '',
    `The licence is in LICENSE.txt beside this file, unchanged. Provided as is, without warranties (see`,
    'LICENSE.txt, section 5). NVIDIA does not endorse HumanityOS. Credits: CREDITS.md, LICENSES.md,',
    'assets/models/LICENSE-nvidia-simready.md and Settings > Credits.',
    '',
  ].join('\n');
}

// ---------------------------------------------------------------------------------------------
// Packing one asset, and the set
// ---------------------------------------------------------------------------------------------

function sha(buf) { return crypto.createHash('sha256').update(buf).digest('hex'); }

/** Build every output file of one asset in memory: { files: {name: Buffer}, info }. */
function buildAsset(a, { SRC, BUILD }) {
  if (DENY.test(a.folder)) throw new Error(`${a.folder}: on the deny list (a trademarked product name)`);
  const dir = path.join(BUILD, a.folder);
  const x = JSON.parse(fs.readFileSync(path.join(dir, 'extract.json'), 'utf8'));
  const b = JSON.parse(fs.readFileSync(path.join(dir, 'bake', 'bake.json'), 'utf8'));
  const bd = path.join(dir, 'bake');
  const lod1c = readMesh(path.join(bd, b.meshes.lod1.file));
  const lod2c = readMesh(path.join(bd, b.meshes.lod2.file));
  const bb = boundsOf(lod1c);
  // The bake must be the extracted model: its bounds within 2% (or 5 mm, for the smallest props, where
  // simplification alone moves a surface a few millimetres) of extract.json's. A Blender unit
  // conversion that differed would show here, not in a screenshot.
  for (let k = 0; k < 3; k++) {
    const want = x.bounds_max_m[k] - x.bounds_min_m[k];
    const got = bb.hi[k] - bb.lo[k];
    if (Math.abs(got - want) > Math.max(0.02 * want, 0.005)) throw new Error(`${a.folder}: baked size ${got.toFixed(3)} m on axis ${k} is not the extracted ${want.toFixed(3)} m`);
  }
  const t = [-(bb.lo[0] + bb.hi[0]) / 2, -bb.lo[1], -(bb.lo[2] + bb.hi[2]) / 2];
  const turn = a.turn_deg || 0;
  const c = Math.cos((turn * Math.PI) / 180), s = Math.sin((turn * Math.PI) / 180);
  const rot = v => [v[0] * c + v[2] * s, v[1], -v[0] * s + v[2] * c];
  const place = p => rot([p[0] + t[0], p[1] + t[1], p[2] + t[2]]);
  place.turn = rot;
  const lod1 = indexMesh(lod1c, place);
  const lod2 = indexMesh(lod2c, place);
  const hulls = b.hulls.map(h => ({ ...h, vertices: h.vertices.map(v => place(v)) }));
  const hullIndex = new Map(hulls.map((h, i) => [h.collider, 2 + i]));

  const tex = k => readImage(path.join(bd, b.textures[k].file), b.textures[k].width, b.textures[k].height);
  const maps = {
    albedo: albedoPixels(tex('albedo'), 1),
    normal: normalPixels(tex('normal')),
    orm: ormPixels(tex('occlusion'), tex('roughness'), tex('metallic')),
  };
  applyBakedRepaints(maps, bakedRepaints(a));
  const pngs = Object.fromEntries(Object.entries(maps).map(([k, m]) => [k, encodePng(m.px, m.w, m.h, m.ch)]));
  const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  lod1.verts.forEach(v => { for (let k = 0; k < 3; k++) { lo[k] = Math.min(lo[k], v[k]); hi[k] = Math.max(hi[k], v[k]); } });
  const dims = hi.map((v, k) => Number((v - lo[k]).toFixed(4)));
  const info = {
    albedo_px: b.textures.albedo.width,
    pbr_px: b.textures.normal.width,
    turn,
    size_class: b.config.size_class,
    dims_m: dims,
    lod1_triangles: b.meshes.lod1.triangles,
    lod2_triangles: b.meshes.lod2.triangles,
    source_triangles: b.source_triangles,
    hulls: hulls.length,
  };
  const name = a.folder.toLowerCase();
  const glb = writeGlb({
    name, lod1, lod2, hulls, pngs,
    extras: { source: a.folder, package_id: x.package_id, licence: 'CC-BY-4.0', dataset: DATASET, size_m: dims },
  });
  const phys = physicsRon(a, x, place, hullIndex);
  info.mass_kg = phys.mass == null ? null : Number(phys.mass.toFixed(4));
  const licence = fs.readFileSync(path.join(SRC, a.folder, '.metadata', 'LICENSE.txt'));
  if (sha(licence) !== CC_BY_40_SHA256) throw new Error(`${a.folder}: LICENSE.txt is not the CC BY 4.0 legal code`);
  const files = {
    [`${name}.glb`]: glb,
    'LICENSE.txt': licence,
    'SOURCE.md': Buffer.from(sourceMd(a, x, b, info), 'utf8'),
    'physics.ron': Buffer.from(phys.text, 'utf8'),
  };
  return { name, files, info, package_id: x.package_id, blender: b.blender, usd: x.usd_version };
}

function outRoots(REPO) {
  return {
    home: path.join(REPO, 'assets', 'models', 'simready'),
    pack: process.env.HUMANITY_SIMREADY_PACKS ? path.join(process.env.HUMANITY_SIMREADY_PACKS, PACK_ID) : path.join('E:\\HumanityOS\\simready-packs', PACK_ID),
  };
}

const isMedical = a => a.room === 'medical';

function verdictOk(a, allowUnchecked) {
  const v = a.brand_check && a.brand_check.verdict;
  if (v === 'clear' || v === 'repaint') return true;
  if (v === 'drop') return false;
  if (allowUnchecked) return true;
  throw new Error(`${a.folder}: no brand check verdict yet (docs/design/simready-props.md, section 8); refusing to pack it`);
}

/** Build the whole set in memory: the per-asset files plus the two manifests and the licence note. */
function buildSet(assets, ctx, allowUnchecked) {
  const out = { home: {}, pack: {} };
  const rows = [];
  for (const a of assets) {
    if (!verdictOk(a, allowUnchecked)) { console.log(`${a.folder}: dropped by its brand check`); continue; }
    const r = buildAsset(a, ctx);
    const where = isMedical(a) ? 'pack' : 'home';
    for (const [f, buf] of Object.entries(r.files)) out[where][`${r.name}/${f}`] = buf;
    rows.push({ a, r, where });
  }
  const versions = {
    blender: ctx.blender,
    usd: rows[0] ? rows[0].r.usd : null,
    node: process.versions.node,
    zlib: process.versions.zlib,
  };
  for (const where of ['home', 'pack']) {
    const mine = rows.filter(x => x.where === where);
    if (!mine.length) continue;
    const manifest = {
      comment: where === 'home'
        ? 'NVIDIA SimReady models converted by `just simready` (docs/design/simready-props.md). CC BY 4.0, not CC0: see LICENSE-nvidia-simready.md one folder up. Every file is listed with its SHA-256; `node scripts/simready/build.js check` rebuilds and compares.'
        : 'The Medical Bay pack of NVIDIA SimReady models (docs/design/simready-props.md, section 7). CC BY 4.0.',
      generator: GENERATOR,
      versions,
      assets: mine.map(({ a, r }) => ({
        folder: a.folder,
        dir: r.name,
        package_id: r.package_id,
        room: a.room,
        row: a.row,
        brand_check: a.brand_check ? a.brand_check.verdict : 'unchecked',
        ...r.info,
        files: Object.fromEntries(Object.entries(r.files).map(([f, buf]) => [f, { bytes: buf.length, sha256: sha(buf) }])),
      })),
    };
    out[where]['manifest.json'] = Buffer.from(JSON.stringify(manifest, null, 1) + '\n', 'utf8');
    if (where === 'pack') {
      out.pack['pack.ron'] = Buffer.from(`(id: "${PACK_ID}", version: ${PACK_VERSION}, instruments: [])\n`, 'utf8');
    }
  }
  const credit = licenceNote(rows);
  return { out, rows, credit, versions };
}

function licenceNote(rows) {
  const line = ({ a, r, where }) => `| \`${r.name}/\` | \`${a.folder}\` | \`${r.package_id}\` | ${where === 'home' ? 'in the app' : 'Medical Bay pack'} |`;
  return [
    '# NVIDIA SimReady Assets (CC BY 4.0)',
    '',
    'The folders under `assets/models/simready/`, and those of the downloadable Medical Bay pack',
    '(installed from Settings into `<data>/models/packs/simready-medical/`), are based on',
    `**NVIDIA SimReady Assets** (${DATASET}), **by NVIDIA**, licensed under`,
    `**CC BY 4.0** (${LICENCE_URL}). They are NOT public domain, unlike the CC0 packs`,
    'credited in `LICENSE-cc0-model-packs.md` beside this file, and they stay CC BY 4.0 inside this',
    'CC0 project.',
    '',
    'HumanityOS converted each one from OpenUSD to glTF and modified it: joined and simplified the',
    'mesh, baked its materials into one texture set, moved its origin, computed collider hulls and',
    'carried its physics values into `physics.ron`. Each folder holds the asset\'s own `LICENSE.txt`',
    '(unchanged) and a `SOURCE.md` listing its changes. Provided as is, without warranties (CC BY 4.0,',
    'section 5). NVIDIA does not endorse HumanityOS. The research behind this:',
    '`docs/reference/findings/2026-11-08-nvidia-simready-assets.md`.',
    '',
    'Generated by `just simready` (scripts/simready/pack.js); do not edit by hand.',
    '',
    '| Folder | NVIDIA asset | Package | Ships |',
    '|---|---|---|---|',
    ...rows.map(line),
    '',
  ].join('\n');
}

// ---------------------------------------------------------------------------------------------
// The pack archive (plain ustar, the instrument packs' reader: src/instrument_packs/tar.rs)
// ---------------------------------------------------------------------------------------------

function tarOf(files) {
  const blocks = [];
  for (const name of Object.keys(files).sort()) {
    const data = files[name];
    if (Buffer.byteLength(name) > 100) throw new Error(`tar: name too long: ${name}`);
    const h = Buffer.alloc(512);
    h.write(name, 0, 'utf8');
    h.write('0000644\0', 100);
    h.write('0000000\0', 108);
    h.write('0000000\0', 116);
    h.write(data.length.toString(8).padStart(11, '0') + '\0', 124);
    h.write('00000000000\0', 136);
    h.write('        ', 148);
    h.write('0', 156);
    h.write('ustar\0', 257);
    h.write('00', 263);
    let sum = 0;
    for (let i = 0; i < 512; i++) sum += h[i];
    h.write(sum.toString(8).padStart(6, '0') + '\0 ', 148);
    blocks.push(h, data, Buffer.alloc((512 - (data.length % 512)) % 512));
  }
  blocks.push(Buffer.alloc(1024));
  return Buffer.concat(blocks);
}

/** data/models/packs.ron: what ships in the app and the downloadable packs. */
function packsRon({ homeModels, homeBytes, tarName, tarBytes, tarSha, packModels }) {
  return [
    '// The 3D model packs (docs/design/simready-props.md, section 7; src/model_packs/catalog.rs):',
    '// what ships in the app, and each pack to download from Settings > Graphics > Furniture models.',
    '// Written by `just simready` (scripts/simready/pack.js) in the same run as the archive, so the',
    '// size and SHA-256 below are the uploaded file\'s. Uploading the archive to the release is the',
    "// operator's step (docs/ai/release.md). Do not edit by hand.",
    '(',
    `    release: "assets-simready-1",`,
    `    base_url: "https://github.com/Shaostoul/Humanity/releases/download/assets-simready-1/",`,
    `    core: (words: "model_packs.core", models: ${homeModels}, bytes: ${homeBytes}, licence: "model_packs.licence.cc_by"),`,
    '    packs: [',
    '        (',
    `            id: "${PACK_ID}",`,
    `            version: ${PACK_VERSION},`,
    '            words: "model_packs.pack.simready_medical",',
    `            file: "${tarName}",`,
    `            bytes: ${tarBytes},`,
    `            sha256: "${tarSha}",`,
    '            licence: "model_packs.licence.cc_by",',
    `            models: [${packModels.map(m => JSON.stringify(m)).join(', ')}],`,
    '        ),',
    '    ],',
    ')',
    '',
  ].join('\n');
}

// ---------------------------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------------------------

function writeTree(root, files, { clean }) {
  if (clean) fs.rmSync(root, { recursive: true, force: true });
  for (const [rel, buf] of Object.entries(files)) {
    const p = path.join(root, ...rel.split('/'));
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, buf);
  }
}

async function packAll(assets, ctx) {
  const allow = process.argv.includes('--allow-unchecked');
  const full = assets.length === ctx.list.assets.length;
  if (!full) throw new Error('pack writes the whole set (its manifests list every asset): run it without --asset, or use check');
  const { out, rows, credit } = buildSet(assets, ctx, allow);
  const roots = outRoots(ctx.REPO);
  writeTree(roots.home, out.home, { clean: true });
  writeTree(roots.pack, out.pack, { clean: true });
  fs.writeFileSync(path.join(ctx.REPO, 'assets', 'models', 'LICENSE-nvidia-simready.md'), credit);
  const tar = tarOf(out.pack);
  const tarName = `humanity-models-${PACK_ID}-v${PACK_VERSION}.tar`;
  const tarPath = path.join(path.dirname(roots.pack), tarName);
  fs.writeFileSync(tarPath, tar);
  fs.writeFileSync(tarPath + '.sha256', `${sha(tar)}  ${tarName}\n`);
  const homeBytes = Object.values(out.home).reduce((s, b) => s + b.length, 0);
  // The catalog Settings reads (src/model_packs/catalog.rs), from this same run, so the size and
  // SHA-256 it lists are the archive's that is uploaded (the instrument packs' rule).
  const packModels = rows.filter(r => r.where === 'pack').map(r => r.r.name);
  fs.writeFileSync(path.join(ctx.REPO, 'data', 'models', 'packs.ron'), packsRon({
    homeModels: rows.filter(r => r.where === 'home').length,
    homeBytes,
    tarName,
    tarBytes: tar.length,
    tarSha: sha(tar),
    packModels,
  }));
  console.log(`home set: ${rows.filter(r => r.where === 'home').length} models, ${(homeBytes / 1048576).toFixed(1)} MiB in assets/models/simready/`);
  console.log(`Medical Bay pack: ${rows.filter(r => r.where === 'pack').length} models, ${tar.length} bytes, sha256 ${sha(tar)} -> ${tarPath}`);
  return { tarBytes: tar.length, tarSha: sha(tar), tarName };
}

/** Rebuild from the build folder into memory and compare with what is on disk. */
async function check(assets, ctx) {
  const roots = outRoots(ctx.REPO);
  const manifestPath = path.join(roots.home, 'manifest.json');
  const committed = fs.existsSync(manifestPath) ? JSON.parse(fs.readFileSync(manifestPath, 'utf8')) : null;
  if (committed && committed.versions.blender !== ctx.blender) {
    console.log(`Blender is ${ctx.blender} now; the committed models were built with ${committed.versions.blender}. Steam updates Blender by itself, and another version may bake different bytes: rebuild with \`just simready\` and review, rather than read the differences below as a fault.`);
  }
  if (committed && committed.versions.zlib !== process.versions.zlib) {
    console.log(`node's zlib is ${process.versions.zlib}; the committed PNGs were deflated by ${committed.versions.zlib}, which may give different bytes for the same pixels.`);
  }
  const { out, credit } = buildSet(assets, ctx, false);
  let bad = 0;
  const cmp = (p, buf) => {
    if (!fs.existsSync(p)) { console.log(`missing: ${p}`); bad++; return; }
    if (!fs.readFileSync(p).equals(buf)) { console.log(`differs: ${p}`); bad++; }
  };
  for (const [rel, buf] of Object.entries(out.home)) cmp(path.join(roots.home, ...rel.split('/')), buf);
  for (const [rel, buf] of Object.entries(out.pack)) cmp(path.join(roots.pack, ...rel.split('/')), buf);
  if (assets.length === ctx.list.assets.length) cmp(path.join(ctx.REPO, 'assets', 'models', 'LICENSE-nvidia-simready.md'), Buffer.from(credit));
  if (bad) { console.log(`check: ${bad} file(s) differ`); process.exit(1); }
  console.log(`check: every file rebuilt byte for byte (${Object.keys(out.home).length + Object.keys(out.pack).length} files)`);
}

/** Write named assets' files into a scratch folder, verdicts not required: a look before packing. */
async function preview(assets, ctx) {
  const root = path.join(ctx.BUILD, '_preview');
  for (const a of assets) {
    const r = buildAsset(a, ctx);
    writeTree(path.join(root, r.name), r.files, { clean: true });
    console.log(`${a.folder}: ${r.info.dims_m.join(' x ')} m, ${r.info.lod1_triangles} triangles, glb ${r.files[`${r.name}.glb`].length} bytes -> ${path.join(root, r.name)}`);
  }
}

module.exports = { packAll, check, preview, buildAsset, encodePng, writeGlb, indexMesh, tarOf, physicsRon, CC_BY_40_SHA256, DENY, buildSet, applyBakedRepaints };
