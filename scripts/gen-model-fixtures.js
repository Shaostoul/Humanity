#!/usr/bin/env node
// Write the small 3D test files both model viewers' tests read (docs/design/model-viewer.md):
//
//   scripts/tests/fixtures/model-cube.glb    a cube, 24 vertices and 12 triangles, normals and
//     texture coordinates, one material (a base colour factor and a 2x2 PNG base colour texture,
//     metallic 0, roughness 0.5), on a child node scaled by 2 under a root node moved +3 on x.
//     So the cube's corners are at x = 1..5, y = -2..2, z = -2..2 once its nodes are applied.
//   scripts/tests/fixtures/model-wedge.stl   a binary STL: 2 triangles.
//   scripts/tests/fixtures/model-wedge-ascii.stl  the same 2 triangles as ASCII STL.
//
// Run: node scripts/gen-model-fixtures.js   (only when these files need to change)
const fs = require("fs");
const path = require("path");
const zlib = require("zlib");

const out = path.resolve(__dirname, "tests", "fixtures");
fs.mkdirSync(out, { recursive: true });

// ── A 2x2 RGBA PNG: red, green / blue, white ──
function crc32(buf) {
  let c, crc = 0xffffffff;
  for (let n = 0; n < buf.length; n++) {
    c = (crc ^ buf[n]) & 0xff;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    crc = (crc >>> 8) ^ c;
  }
  return (crc ^ 0xffffffff) >>> 0;
}
function pngChunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
}
function png2x2() {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(2, 0);
  ihdr.writeUInt32BE(2, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // RGBA
  const rows = Buffer.from([
    0, 255, 0, 0, 255, 0, 255, 0, 255,
    0, 0, 0, 255, 255, 255, 255, 255, 255,
  ]);
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    pngChunk("IHDR", ihdr),
    pngChunk("IDAT", zlib.deflateSync(rows)),
    pngChunk("IEND", Buffer.alloc(0)),
  ]);
}

// ── The cube: 6 faces, 4 vertices each (own normals and uvs) ──
function cube() {
  const faces = [
    { n: [1, 0, 0], u: [0, 0, -1], v: [0, 1, 0] },
    { n: [-1, 0, 0], u: [0, 0, 1], v: [0, 1, 0] },
    { n: [0, 1, 0], u: [1, 0, 0], v: [0, 0, -1] },
    { n: [0, -1, 0], u: [1, 0, 0], v: [0, 0, 1] },
    { n: [0, 0, 1], u: [1, 0, 0], v: [0, 1, 0] },
    { n: [0, 0, -1], u: [-1, 0, 0], v: [0, 1, 0] },
  ];
  const pos = [], nor = [], uv = [], idx = [];
  faces.forEach((f, i) => {
    for (const [a, b] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) {
      pos.push(f.n[0] + a * f.u[0] + b * f.v[0], f.n[1] + a * f.u[1] + b * f.v[1], f.n[2] + a * f.u[2] + b * f.v[2]);
      nor.push(...f.n);
      uv.push((a + 1) / 2, (1 - b) / 2);
    }
    const o = i * 4;
    idx.push(o, o + 1, o + 2, o, o + 2, o + 3);
  });
  return { pos, nor, uv, idx };
}

function pad4(buf, fill) {
  const extra = (4 - (buf.length % 4)) % 4;
  return extra ? Buffer.concat([buf, Buffer.alloc(extra, fill)]) : buf;
}

function glb() {
  const c = cube();
  const f32 = (a) => Buffer.from(new Float32Array(a).buffer);
  const parts = [f32(c.pos), f32(c.nor), f32(c.uv), Buffer.from(new Uint16Array(c.idx).buffer), png2x2()];
  const views = [];
  let bin = Buffer.alloc(0);
  for (const p of parts) {
    bin = pad4(bin, 0);
    views.push({ buffer: 0, byteOffset: bin.length, byteLength: p.length });
    bin = Buffer.concat([bin, p]);
  }
  bin = pad4(bin, 0);
  views[0].target = 34962; views[1].target = 34962; views[2].target = 34962; views[3].target = 34963;
  const json = {
    asset: { version: "2.0", generator: "HumanityOS gen-model-fixtures.js" },
    scene: 0,
    scenes: [{ nodes: [0] }],
    nodes: [
      { name: "root", translation: [3, 0, 0], children: [1] },
      { name: "cube", scale: [2, 2, 2], mesh: 0 },
    ],
    meshes: [{ name: "cube", primitives: [{ attributes: { POSITION: 0, NORMAL: 1, TEXCOORD_0: 2 }, indices: 3, material: 0, mode: 4 }] }],
    materials: [{
      name: "checker",
      pbrMetallicRoughness: { baseColorFactor: [1, 1, 1, 1], baseColorTexture: { index: 0 }, metallicFactor: 0, roughnessFactor: 0.5 },
    }],
    textures: [{ source: 0, sampler: 0 }],
    samplers: [{ magFilter: 9728, minFilter: 9728 }],
    images: [{ bufferView: 4, mimeType: "image/png" }],
    accessors: [
      { bufferView: 0, componentType: 5126, count: 24, type: "VEC3", min: [-1, -1, -1], max: [1, 1, 1] },
      { bufferView: 1, componentType: 5126, count: 24, type: "VEC3" },
      { bufferView: 2, componentType: 5126, count: 24, type: "VEC2" },
      { bufferView: 3, componentType: 5123, count: 36, type: "SCALAR" },
    ],
    bufferViews: views,
    buffers: [{ byteLength: bin.length }],
  };
  const jsonBuf = pad4(Buffer.from(JSON.stringify(json), "utf8"), 0x20);
  const header = Buffer.alloc(12);
  header.write("glTF", 0, "ascii");
  header.writeUInt32LE(2, 4);
  const chunk = (type, data) => {
    const h = Buffer.alloc(8);
    h.writeUInt32LE(data.length, 0);
    h.write(type, 4, "ascii");
    return Buffer.concat([h, data]);
  };
  const body = Buffer.concat([chunk("JSON", jsonBuf), chunk("BIN\0", bin)]);
  header.writeUInt32LE(12 + body.length, 8);
  return Buffer.concat([header, body]);
}

// ── A wedge: two triangles sharing an edge, in STL's z-up ──
const WEDGE = [
  [[0, 0, 0], [10, 0, 0], [0, 10, 0]],
  [[0, 0, 0], [0, 10, 0], [0, 0, 5]],
];
function stlBinary() {
  const buf = Buffer.alloc(84 + WEDGE.length * 50);
  buf.write("HumanityOS model viewer test wedge", 0, "ascii");
  buf.writeUInt32LE(WEDGE.length, 80);
  WEDGE.forEach((t, i) => {
    const o = 84 + i * 50;
    // The stored normal is deliberately wrong (0,0,0): readers work normals out from the corners.
    t.forEach((p, k) => p.forEach((v, j) => buf.writeFloatLE(v, o + 12 + k * 12 + j * 4)));
  });
  return buf;
}
function stlAscii() {
  const lines = ["solid wedge"];
  for (const t of WEDGE) {
    lines.push("  facet normal 0 0 0", "    outer loop");
    for (const p of t) lines.push(`      vertex ${p.join(" ")}`);
    lines.push("    endloop", "  endfacet");
  }
  lines.push("endsolid wedge", "");
  return Buffer.from(lines.join("\n"), "ascii");
}

fs.writeFileSync(path.join(out, "model-cube.glb"), glb());
fs.writeFileSync(path.join(out, "model-wedge.stl"), stlBinary());
fs.writeFileSync(path.join(out, "model-wedge-ascii.stl"), stlAscii());
console.log("wrote model-cube.glb, model-wedge.stl, model-wedge-ascii.stl in scripts/tests/fixtures");
