#!/usr/bin/env node
// Generate a minimal valid GLB (binary glTF 2.0) with a DISTINCTIVE shape —
// a low-poly "crate with a pyramid lid" — so the model-pipeline wiring
// (v0.734, MachineDef.model) can be verified visually: a machine rendering
// this is unmistakably NOT one of the engine's primitive boxes/cylinders.
//
// Pure Node, no deps: builds the JSON chunk + BIN chunk (positions + indices,
// u16) with correct 4-byte alignment. ~1 m sized, meters, Y-up — exactly the
// authoring rules in docs/game/model-pipeline.md.
//
// Run: node scripts/gen-test-glb.js   -> writes data/models/test_crate.glb

const fs = require('fs');
const path = require('path');

// ── Geometry: a 1x0.7x1 m box (crate) + a pyramid lid to 1.2 m apex ──
const P = [
  // box bottom (y=0) and top (y=0.7), 8 corners
  [-0.5, 0.0, -0.5], [0.5, 0.0, -0.5], [0.5, 0.0, 0.5], [-0.5, 0.0, 0.5],
  [-0.5, 0.7, -0.5], [0.5, 0.7, -0.5], [0.5, 0.7, 0.5], [-0.5, 0.7, 0.5],
  // pyramid apex
  [0.0, 1.2, 0.0],
];
const TRI = [
  // bottom (facing down)
  [0, 2, 1], [0, 3, 2],
  // sides
  [0, 1, 5], [0, 5, 4],
  [1, 2, 6], [1, 6, 5],
  [2, 3, 7], [2, 7, 6],
  [3, 0, 4], [3, 4, 7],
  // pyramid lid (4 triangles to the apex)
  [4, 5, 8], [5, 6, 8], [6, 7, 8], [7, 4, 8],
];

const positions = Buffer.alloc(P.length * 12);
P.forEach(([x, y, z], i) => {
  positions.writeFloatLE(x, i * 12);
  positions.writeFloatLE(y, i * 12 + 4);
  positions.writeFloatLE(z, i * 12 + 8);
});
const indices = Buffer.alloc(TRI.length * 3 * 2);
TRI.flat().forEach((v, i) => indices.writeUInt16LE(v, i * 2));

// BIN chunk: positions then indices, each 4-byte aligned.
const pad4 = (b) => Buffer.concat([b, Buffer.alloc((4 - (b.length % 4)) % 4)]);
const posPadded = pad4(positions);
const idxPadded = pad4(indices);
const bin = Buffer.concat([posPadded, idxPadded]);

const mins = [Math.min(...P.map(p => p[0])), Math.min(...P.map(p => p[1])), Math.min(...P.map(p => p[2]))];
const maxs = [Math.max(...P.map(p => p[0])), Math.max(...P.map(p => p[1])), Math.max(...P.map(p => p[2]))];

const gltf = {
  asset: { version: '2.0', generator: 'HumanityOS gen-test-glb' },
  scene: 0,
  scenes: [{ nodes: [0] }],
  nodes: [{ mesh: 0, name: 'test_crate' }],
  meshes: [{ primitives: [{ attributes: { POSITION: 0 }, indices: 1 }], name: 'test_crate' }],
  accessors: [
    { bufferView: 0, componentType: 5126, count: P.length, type: 'VEC3', min: mins, max: maxs },
    { bufferView: 1, componentType: 5123, count: TRI.length * 3, type: 'SCALAR' },
  ],
  bufferViews: [
    { buffer: 0, byteOffset: 0, byteLength: positions.length },
    { buffer: 0, byteOffset: posPadded.length, byteLength: indices.length },
  ],
  buffers: [{ byteLength: bin.length }],
};

// JSON chunk padded with spaces to 4 bytes.
let json = Buffer.from(JSON.stringify(gltf), 'utf8');
if (json.length % 4) json = Buffer.concat([json, Buffer.from(' '.repeat(4 - (json.length % 4)))]);

const header = Buffer.alloc(12);
header.write('glTF', 0);
header.writeUInt32LE(2, 4);
header.writeUInt32LE(12 + 8 + json.length + 8 + bin.length, 8);
const jsonHdr = Buffer.alloc(8);
jsonHdr.writeUInt32LE(json.length, 0);
jsonHdr.writeUInt32LE(0x4e4f534a, 4); // 'JSON'
const binHdr = Buffer.alloc(8);
binHdr.writeUInt32LE(bin.length, 0);
binHdr.writeUInt32LE(0x004e4942, 4); // 'BIN\0'

const out = Buffer.concat([header, jsonHdr, json, binHdr, bin]);
const dest = path.join('data', 'models', 'test_crate.glb');
fs.mkdirSync(path.dirname(dest), { recursive: true });
fs.writeFileSync(dest, out);
console.log(`wrote ${dest} (${out.length} bytes, ${P.length} verts, ${TRI.length} tris)`);
