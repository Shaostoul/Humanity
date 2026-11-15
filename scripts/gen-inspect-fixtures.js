#!/usr/bin/env node
// Write the planted test models both model inspectors' tests read (docs/design/model-inspector.md;
// src/gui/model_view/inspect_tests.rs and scripts/tests/model-inspect.test.js). Each is built to
// carry known faults, so a test that finds them proves the inspector looks:
//
//   scripts/tests/fixtures/inspect/planted/planted.glb, with physics.ron beside it:
//     a 1.0 x 0.5 x 0.8 m box ("body", mesh 0) on its base, centred; a collision mesh named by
//     convention ("UCX_body", mesh 1) that matches it; a physics hull (mesh 2, in no scene) standing
//     1 m off the model along +x, so the collision shapes do not cover the model; and empties:
//       muzzle_01  its front (+Z) turned to point INTO the model (wrong way)
//       seat_main  level, fine
//       (no name)  an empty with no name
//       Empty.003  Blender's default name, which no convention marks
//       door_a     scaled to 0.05 m, where a door's size must be 0.4 to 4 m
//       mount_top  turned a quarter about x, so it is tilted where a mount must be level
//   scripts/tests/fixtures/inspect/huge/huge.glb: a chair exported in centimetres as metres, 180 x
//     90 x 60 m, its cushion a second mesh (so the game, which draws only the first mesh's first
//     part, would leave it out).
//   scripts/tests/fixtures/inspect/missing/missing.gltf (+ missing.bin): a box whose base colour
//     texture names missing_palette.png, which is not there.
//
// Run: node scripts/gen-inspect-fixtures.js   (only when these files need to change)
'use strict';

const fs = require('fs');
const path = require('path');

const OUT = path.resolve(__dirname, 'tests', 'fixtures', 'inspect');

/** A box's 8 corners and 12 triangles. */
function box(min, max) {
  const p = [];
  for (let i = 0; i < 8; i++) p.push(i & 1 ? max[0] : min[0], i & 2 ? max[1] : min[1], i & 4 ? max[2] : min[2]);
  const q = [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]];
  const idx = [];
  for (const [a, b, c, d] of q) idx.push(a, b, c, a, c, d);
  return { positions: new Float32Array(p), indices: new Uint16Array(idx) };
}

/** A glTF document from meshes (each one box) and nodes: { json, bin }. */
function document(meshes, nodes, extra) {
  const chunks = [];
  let length = 0;
  const views = [];
  const accessors = [];
  const push = (typed) => {
    const pad = (4 - (length % 4)) % 4;
    if (pad) { chunks.push(Buffer.alloc(pad)); length += pad; }
    chunks.push(Buffer.from(typed.buffer, typed.byteOffset, typed.byteLength));
    views.push({ buffer: 0, byteOffset: length, byteLength: typed.byteLength });
    length += typed.byteLength;
    return views.length - 1;
  };
  const gltfMeshes = meshes.map((m) => {
    const b = box(m.min, m.max);
    const pv = push(b.positions);
    accessors.push({ bufferView: pv, componentType: 5126, count: 8, type: 'VEC3', min: m.min, max: m.max });
    const pa = accessors.length - 1;
    const attributes = { POSITION: pa };
    if (m.uvs) {
      const uv = new Float32Array(16);
      const uvv = push(uv);
      accessors.push({ bufferView: uvv, componentType: 5126, count: 8, type: 'VEC2' });
      attributes.TEXCOORD_0 = accessors.length - 1;
    }
    const iv = push(b.indices);
    accessors.push({ bufferView: iv, componentType: 5123, count: 36, type: 'SCALAR' });
    const prim = { attributes, indices: accessors.length - 1 };
    if (m.material !== undefined) prim.material = m.material;
    return { name: m.name, primitives: [prim] };
  });
  const pad = (4 - (length % 4)) % 4;
  if (pad) { chunks.push(Buffer.alloc(pad)); length += pad; }
  const json = Object.assign({
    asset: { version: '2.0', generator: 'gen-inspect-fixtures.js (HumanityOS)' },
    scene: 0,
    scenes: [{ nodes: [0] }],
    nodes,
    meshes: gltfMeshes,
    accessors,
    bufferViews: views,
    buffers: [{ byteLength: length }],
  }, extra || {});
  return { json, bin: Buffer.concat(chunks) };
}

/** A .glb from a document. */
function glb({ json, bin }) {
  let text = Buffer.from(JSON.stringify(json), 'utf8');
  const tpad = (4 - (text.length % 4)) % 4;
  text = Buffer.concat([text, Buffer.alloc(tpad, 0x20)]);
  const head = Buffer.alloc(12);
  head.write('glTF', 0, 'ascii');
  head.writeUInt32LE(2, 4);
  head.writeUInt32LE(12 + 8 + text.length + 8 + bin.length, 8);
  const chunk = (type, data) => {
    const h = Buffer.alloc(8);
    h.writeUInt32LE(data.length, 0);
    h.write(type, 4, 'ascii');
    return Buffer.concat([h, data]);
  };
  return Buffer.concat([head, chunk('JSON', text), chunk('BIN\0', bin)]);
}

function write(rel, bytes) {
  const file = path.join(OUT, rel);
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, bytes);
  console.log(`wrote ${path.relative(path.resolve(__dirname, '..'), file)} (${bytes.length} bytes)`);
}

// ── planted ──
const S = Math.SQRT1_2;
const planted = document(
  [
    { name: 'body', min: [-0.5, 0, -0.4], max: [0.5, 0.5, 0.4] },
    { name: 'UCX_body', min: [-0.51, 0, -0.41], max: [0.51, 0.51, 0.41] },
    { name: 'hull_offset', min: [0.5, 0, -0.4], max: [1.5, 0.5, 0.4] },
  ],
  [
    { name: 'planted', children: [1, 2, 3, 4, 5, 6, 7, 8] },
    { name: 'body', mesh: 0 },
    { name: 'UCX_body', mesh: 1 },
    { name: 'muzzle_01', translation: [0, 0.25, 0.3], rotation: [0, 1, 0, 0] },
    { name: 'seat_main', translation: [0, 0.5, 0] },
    { translation: [0.2, 0.5, 0] },
    { name: 'Empty.003', translation: [-0.2, 0.5, 0] },
    { name: 'door_a', translation: [0, 0.25, -0.4], scale: [0.05, 0.05, 0.05] },
    { name: 'mount_top', translation: [0.3, 0.5, 0], rotation: [S, 0, 0, S] },
  ],
);
planted.json.buffers[0].byteLength = planted.bin.length;
write('planted/planted.glb', glb(planted));
write('planted/physics.ron', Buffer.from(`// The planted test model's physics (scripts/gen-inspect-fixtures.js): one collider whose hull,
// mesh 2, stands 1 m off the visible model along +x. Read by src/assets/model_physics.rs.
(
    source: "planted",
    package_id: "test.planted",
    mass_kg: Some(10.0),
    center_of_mass_m: Some((0.0, 0.25, 0.0)),
    bodies: [
        (name: "body", mass_kg: Some(10.0), kinematic: false, principal_inertia_kg_m2: None),
    ],
    colliders: [
        (body: "body", part: "hull_offset", approximation: "convexHull", hull_mesh: Some(2), friction: Some(0.5), static_friction: Some(0.5), restitution: Some(0.1)),
    ],
    joints: [],
)
`, 'utf8'));

// ── huge ──
const huge = document(
  [
    { name: 'chair', min: [-90, 0, -30], max: [90, 90, 30] },
    { name: 'cushion', min: [-80, 40, -25], max: [80, 50, 25] },
  ],
  [
    { name: 'chair_root', children: [1, 2] },
    { name: 'chair', mesh: 0 },
    { name: 'cushion', mesh: 1 },
  ],
);
huge.json.buffers[0].byteLength = huge.bin.length;
write('huge/huge.glb', glb(huge));

// ── missing ──
const missing = document(
  [{ name: 'crate', min: [-0.25, 0, -0.25], max: [0.25, 0.5, 0.25], material: 0, uvs: true }],
  [{ name: 'crate', mesh: 0 }],
  {
    materials: [{ name: 'painted', pbrMetallicRoughness: { baseColorTexture: { index: 0 }, metallicFactor: 0, roughnessFactor: 0.8 } }],
    textures: [{ source: 0 }],
    images: [{ uri: 'missing_palette.png' }],
  },
);
missing.json.scenes = [{ nodes: [0] }];
missing.json.buffers[0] = { uri: 'missing.bin', byteLength: missing.bin.length };
write('missing/missing.gltf', Buffer.from(JSON.stringify(missing.json, null, 1) + '\n', 'utf8'));
write('missing/missing.bin', missing.bin);
