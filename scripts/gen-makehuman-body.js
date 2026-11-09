#!/usr/bin/env node
// THE MAKEHUMAN BODY, CONVERTED (character animation rung 1, 2026-11-09;
// docs/design/character-animation.md sections 2.3 and 13). Our own converter: it reads MakeHuman's
// CC0 DATA (the base mesh, the default skeleton and its weights, the young-adult macro targets) and
// writes two skinned glTF bodies, a female and a male, each with the shape targets the look can
// drive as glTF morph targets:
//
//   assets/models/characters/makehuman/makehuman_female.gltf + .bin
//   assets/models/characters/makehuman/makehuman_male.gltf + .bin
//   assets/models/characters/makehuman/SOURCE.md   the licence (CC0), the commit, the inputs' hashes
//
// MakeHuman's CODE is AGPL and none of it is used or copied: target blending, joint placement and
// the weights are implemented here from the file formats themselves (a target is lines of "vertex
// dx dy dz" in decimetres; a joint is the mean of a list of vertices; a bone runs from its head
// joint to its tail joint; the weights file lists, per bone, vertex and weight).
//
// HOW A BODY IS MADE
// - Shape: base.obj plus MakeHuman's ancestry targets for the gender, a third each (MakeHuman's own
//   default mix), at average muscle and weight (whose universal targets are empty by design), with
//   the feet on MakeHuman's ground helper. The female stands 1.59 m, the male 1.73 m.
// - Skeleton: the bones the role map data/characters/role_maps/makehuman_default.ron names (56 of
//   the 163), each bone's head joint as its position, every rest rotation the identity (our
//   retargeter works in model space, so bone axes carry no meaning). Every other bone's weights go
//   to its nearest kept ancestor, except the four smaller toes, which go to the kept middle toe so
//   all five bend together. The bones dropped are in-chain segments and twist segments that rung 1
//   leaves at rest under their parent anyway (so dropping them changes nothing that moves), the
//   metacarpals, the breasts and the face (rung 9's). Why 56: section 8.0 of the plan measures it.
// - Weights: the four largest per vertex after the merge, renormalised, written as unorm16 summing
//   to exactly 65535.
// - Regions (the materials the body's data file tints): from the dominant bone of each face, the
//   face and neck skin, a long-sleeved shirt, trousers, shoes (feet) and hands; the scalp and the
//   brows picked out by position on the base mesh (the same faces on both bodies), the scalp also
//   given a thin shell of hair 5 mm out so a head has hair's volume; generated eyeballs (sclera,
//   iris, pupil) where MakeHuman's eye helpers sit, each turning with its eye bone. MakeHuman's
//   helper geometry (tights, skirt, hair and genital helpers, joint cubes, teeth) is not drawn.
// - Shape targets (glTF morph targets, POSITION and NORMAL deltas; sparse where the target is):
//     height_min, height_max                   MakeHuman's height targets (average build)
//     proportions_ideal, proportions_uncommon  its body proportions targets (average build)
//     muscle_min, muscle_max, weight_min, weight_max            the build's edges
//     muscle_min_weight_min ... muscle_max_weight_max           the build's corners
//     african, asian, caucasian               each ancestry's difference from the default third-each
//   The build targets are the raw universal targets (each a shape at that corner of the muscle and
//   weight grid), so a later build slider weights them bilinearly the way the grid reads. Each
//   target's movement of every joint is written beside it, as an accessor named
//   `joint_deltas:<target>` (one VEC3 per skin joint, metres), so a reshaped body's skeleton follows
//   its mesh (the engine's `Skeleton::reshaped`). MakeHuman's ground helper does not move under any
//   of these targets (checked here), so the feet stay on the floor at every shape.
//
//   node scripts/gen-makehuman-body.js                 write the files (needs the sources)
//   node scripts/gen-makehuman-body.js --check         exit 1 if the committed files differ
//   node scripts/gen-makehuman-body.js --preview DIR   also render front, side and back views of
//                                                      both bodies to PNG (a look without a boot)
//   --src DIR    the MakeHuman source folder (default HUMANITY_MAKEHUMAN_SRC, else
//                E:\HumanityOS\makehuman-src\a8bc2d54, where the 2026-11-09 fetch put them)
//
// --check WITHOUT THE SOURCES (CI, a checkout without the E: drive) compares the committed files with
// the SHA-256 hashes SOURCE.md records, and says that is all it could do; with the sources it builds
// everything again and compares byte for byte. Every input is checked against the git blob hash
// below before it is read, so a changed or corrupted source refuses to convert. The sources are
// untrusted input: they are read as text and numbers, never run.
//
// Deterministic: no randomness, no dates from the clock, fixed float formatting.

'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const zlib = require('zlib');
const { GltfBuilder, differing } = require('./lib/gltf-builder.js');

const ROOT = path.join(__dirname, '..');
const OUT = path.join(ROOT, 'assets', 'models', 'characters', 'makehuman');
const ROLE_MAP = path.join(ROOT, 'data', 'characters', 'role_maps', 'makehuman_default.ron');
const DEFAULT_SRC = process.env.HUMANITY_MAKEHUMAN_SRC || 'E:\\HumanityOS\\makehuman-src\\a8bc2d54';
const SOURCE = {
  repo: 'https://github.com/makehumancommunity/makehuman',
  commit: 'a8bc2d54ff0ac92e78ff71431b1023eda42bf482',
  commitDate: '2024-06-26',
  fetched: '2026-11-09',
  approved: 'the operator, 2026-11-09 ("Yes. We need at least a male and a female.")',
};
// Every file read, under the source folder (makehuman/data/ in the repository), with its git blob
// SHA-1 as GitHub reported it on 2026-11-09 (the fetch's MANIFEST.json, checked against the bytes).
const INPUTS = [
  ['3dobjs/base.obj', 'd26635e9326e3cca30778fd7b9c00062b03cce09'],
  ['rigs/default.mhskel', 'b02cbecae00143856410d7561adf006d83bf9b3e'],
  ['rigs/default_weights.mhw', '66185806afacf87749be8e5cf6dbf66496b2a6b0'],
  ['targets/macrodetails/african-female-young.target', 'a7d2b130667cc81ecd1ca0598da9be27bec1864a'],
  ['targets/macrodetails/african-male-young.target', 'dd5743e48700267d76596f575bf17b4b5cc3b3e0'],
  ['targets/macrodetails/asian-female-young.target', 'f862c96add11c31ab1ca75c69e1c67ae5e0e3d5f'],
  ['targets/macrodetails/asian-male-young.target', '8cd3ef3e9ddb06ee2d166f75f9ac10871251f938'],
  ['targets/macrodetails/caucasian-female-young.target', '9d1f0cbeedc9a6a51abe33f1ebb5fa7c5a7edbf1'],
  ['targets/macrodetails/caucasian-male-young.target', 'c3b82f92c5ced85599199cd184b0faf3b3fc6881'],
  ['targets/macrodetails/universal-female-young-averagemuscle-maxweight.target', 'e0b442f75e36ad0980cd7ccda41b6b8d67b9f200'],
  ['targets/macrodetails/universal-female-young-averagemuscle-minweight.target', '7b4dfc290b1228f48f00e7b121ffe8932a7bbb75'],
  ['targets/macrodetails/universal-female-young-maxmuscle-averageweight.target', '91ca14d335389e4e4a964132fc6580ada688cb64'],
  ['targets/macrodetails/universal-female-young-maxmuscle-maxweight.target', 'c3f7a80bfa534dc8723deff98d045c9f99679ccd'],
  ['targets/macrodetails/universal-female-young-maxmuscle-minweight.target', '7eb470715d485b9f21733dd7142f0cd0369638ac'],
  ['targets/macrodetails/universal-female-young-minmuscle-averageweight.target', '8d3c861b41dda1167fac41eae3dd6458cc28e8c8'],
  ['targets/macrodetails/universal-female-young-minmuscle-maxweight.target', 'a4317e0dd09303df9034d1fe1906d49d98cdcfa8'],
  ['targets/macrodetails/universal-female-young-minmuscle-minweight.target', 'da95e85f2ab24a32c2d2ae037254325b2f10fec3'],
  ['targets/macrodetails/universal-male-young-averagemuscle-maxweight.target', '15d495149eec197fed9ab8dda745cdfd4878ab2b'],
  ['targets/macrodetails/universal-male-young-averagemuscle-minweight.target', '0a53c15a2d6cf23120c786d95b36d7a618c61acf'],
  ['targets/macrodetails/universal-male-young-maxmuscle-averageweight.target', 'b7c039c7f463fe1f1721b9ab269c7248d9c4f2f7'],
  ['targets/macrodetails/universal-male-young-maxmuscle-maxweight.target', 'a0f12e4b0c0f84f2a6b90badfaf252a125e9e3d1'],
  ['targets/macrodetails/universal-male-young-maxmuscle-minweight.target', '6b21e86b3800e7bd736b8d9aa98cd98040acd334'],
  ['targets/macrodetails/universal-male-young-minmuscle-averageweight.target', '2b585bb27c88a0ef63972f0b2d480d61850c5889'],
  ['targets/macrodetails/universal-male-young-minmuscle-maxweight.target', '42d9f8ca69fe7bdc40429efc7c515e08e90b7a20'],
  ['targets/macrodetails/universal-male-young-minmuscle-minweight.target', 'e63021e74918cff0f60b65e2d134ce6f256a3cb2'],
  ['targets/macrodetails/height/female-young-averagemuscle-averageweight-maxheight.target', '1db1440ae3d7ad4259d64991047093f450fb38b0'],
  ['targets/macrodetails/height/female-young-averagemuscle-averageweight-minheight.target', '864d9eea661dab7e37177957f452d622f20a5c0a'],
  ['targets/macrodetails/height/male-young-averagemuscle-averageweight-maxheight.target', '1db1440ae3d7ad4259d64991047093f450fb38b0'],
  ['targets/macrodetails/height/male-young-averagemuscle-averageweight-minheight.target', '7208f5dcf183c0cdba3eaa1e2da8cf21e7726b84'],
  ['targets/macrodetails/proportions/female-young-averagemuscle-averageweight-idealproportions.target', '145e69247246510dd0a0e5a81a63bed2f6e5090a'],
  ['targets/macrodetails/proportions/female-young-averagemuscle-averageweight-uncommonproportions.target', '28d0cd73e8927c828844bafcb6b5c570c27e8c90'],
  ['targets/macrodetails/proportions/male-young-averagemuscle-averageweight-idealproportions.target', 'c0a95b13bd0d2a78b025da7502f7b5653f917904'],
  ['targets/macrodetails/proportions/male-young-averagemuscle-averageweight-uncommonproportions.target', '4adadda4ad9c78086f309a47e2ece30670097a8c'],
];

const GENDERS = ['female', 'male'];
const ANCESTRIES = ['african', 'asian', 'caucasian'];
const DM = 0.1; // MakeHuman works in decimetres.
const BODY_VERTS = 13380; // base.obj's "body" group is vertices 0 to 13379.
// The trousers' waistband, decimetres over the hip joints (MakeHuman's root bone) on the base mesh:
// a little under the navel.
const WAIST_OVER_HIP_DM = 1.0;
// Steps of the shirt's smoothing (pairs of Taubin's shrink and grow).
const SHIRT_SMOOTH_STEPS = 24;
// The four smaller toes of each foot bend with the kept middle toe (see the header).
const MERGE_INTO = {};
for (const s of ['L', 'R']) for (const t of [1, 2, 4, 5]) MERGE_INTO[`toe${t}-1.${s}`] = `toe3-1.${s}`;

// The shape targets each body carries: its name in the glTF, and how it is made from target files
// (paths under targets/macrodetails/, with {g} the gender). `minus` subtracts a third of each
// listed file (the ancestry targets are differences from the default mix).
const TARGETS = [
  { name: 'height_min', files: ['height/{g}-young-averagemuscle-averageweight-minheight'] },
  { name: 'height_max', files: ['height/{g}-young-averagemuscle-averageweight-maxheight'] },
  { name: 'proportions_ideal', files: ['proportions/{g}-young-averagemuscle-averageweight-idealproportions'] },
  { name: 'proportions_uncommon', files: ['proportions/{g}-young-averagemuscle-averageweight-uncommonproportions'] },
  { name: 'muscle_min', files: ['universal-{g}-young-minmuscle-averageweight'] },
  { name: 'muscle_max', files: ['universal-{g}-young-maxmuscle-averageweight'] },
  { name: 'weight_min', files: ['universal-{g}-young-averagemuscle-minweight'] },
  { name: 'weight_max', files: ['universal-{g}-young-averagemuscle-maxweight'] },
  { name: 'muscle_min_weight_min', files: ['universal-{g}-young-minmuscle-minweight'] },
  { name: 'muscle_min_weight_max', files: ['universal-{g}-young-minmuscle-maxweight'] },
  { name: 'muscle_max_weight_min', files: ['universal-{g}-young-maxmuscle-minweight'] },
  { name: 'muscle_max_weight_max', files: ['universal-{g}-young-maxmuscle-maxweight'] },
  ...ANCESTRIES.map((a) => ({ name: a, files: [`${a}-{g}-young`], minus: ANCESTRIES.map((b) => `${b}-{g}-young`) })),
];

// Regions: each kept bone's region (the region of a face is its vertices' dominant bones' region,
// by majority). The scalp, brows and eyes are placed by position below.
function regionOfBone(b) {
  if (/^(wrist|finger)/.test(b)) return 'hands';
  if (/^(foot|toe)/.test(b)) return 'shoes';
  if (/^(root|upperleg|lowerleg)/.test(b)) return 'trousers';
  if (/^(spine|clavicle|upperarm|lowerarm)/.test(b)) return 'shirt';
  return 'skin'; // neck, head, jaw, eyes
}
// Ties between regions at a face go to the first listed.
const REGION_ORDER = ['hands', 'shoes', 'shirt', 'trousers', 'skin'];
// The materials, in the order the primitives are written.
const MATERIALS = ['skin', 'hair', 'brows', 'shirt', 'trousers', 'shoes', 'hands', 'sclera', 'iris', 'pupil'];

// ── Small vector maths ────────────────────────────────────────────────────
const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const add = (a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const mul = (a, s) => [a[0] * s, a[1] * s, a[2] * s];
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const len = (a) => Math.sqrt(dot(a, a));
const norm = (a) => { const l = len(a); return l > 1e-12 ? mul(a, 1 / l) : [0, 0, 0]; };

// ── Reading the sources ───────────────────────────────────────────────────
function gitBlobSha1(buf) {
  return crypto.createHash('sha1').update(`blob ${buf.length}\0`).update(buf).digest('hex');
}

function readSources(src) {
  const text = {};
  for (const [rel, sha] of INPUTS) {
    const p = path.join(src, ...rel.split('/'));
    const buf = fs.readFileSync(p);
    const got = gitBlobSha1(buf);
    if (got !== sha) throw new Error(`${p}: git blob ${got}, expected ${sha} (commit ${SOURCE.commit}); refusing a changed source`);
    text[rel] = buf.toString('utf8');
  }
  return text;
}

function parseObj(text) {
  const v = [];
  const vt = [];
  const faces = []; // { group, v: [4], vt: [4] }
  let group = '';
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line[0] === '#') continue;
    const p = line.split(/\s+/);
    if (p[0] === 'v') v.push([+p[1], +p[2], +p[3]]);
    else if (p[0] === 'vt') vt.push([+p[1], +p[2]]);
    else if (p[0] === 'g') group = p[1];
    else if (p[0] === 'f') {
      const corners = p.slice(1).map((c) => c.split('/').map((x) => +x - 1));
      faces.push({ group, v: corners.map((c) => c[0]), vt: corners.map((c) => c[1]) });
    }
  }
  return { v, vt, faces };
}

function parseTarget(text) {
  const out = new Map();
  for (const raw of text.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line || line[0] === '#') continue;
    const p = line.split(/\s+/);
    out.set(+p[0], [+p[1], +p[2], +p[3]]);
  }
  return out;
}

/** The bone names the role map names, in its order (the bones the body keeps). */
function keptBones() {
  const text = fs.readFileSync(ROLE_MAP, 'utf8');
  const kept = [];
  for (const m of text.matchAll(/^\s*"([^"]+)":\s*"([^"]+)",?\s*$/gm)) kept.push(m[1]);
  if (kept.length < 20) throw new Error(`${ROLE_MAP}: only ${kept.length} bones read`);
  return kept;
}

// ── The build ─────────────────────────────────────────────────────────────
function build(src) {
  const text = readSources(src);
  const obj = parseObj(text['3dobjs/base.obj']);
  const skel = JSON.parse(text['rigs/default.mhskel']);
  const weightsIn = JSON.parse(text['rigs/default_weights.mhw']).weights;
  const target = (rel) => parseTarget(text[`targets/macrodetails/${rel}.target`]);

  // Groups of vertices.
  const groupVerts = {};
  for (const f of obj.faces) {
    if (!groupVerts[f.group]) groupVerts[f.group] = new Set();
    for (const i of f.v) groupVerts[f.group].add(i);
  }
  const groundVerts = [...groupVerts['joint-ground']].sort((a, b) => a - b);
  const groundY = groundVerts.reduce((s, i) => s + obj.v[i][1], 0) / groundVerts.length;

  // The skeleton, reduced.
  const keepList = keptBones();
  const KEPT = new Set(keepList);
  for (const b of keepList) if (!skel.bones[b]) throw new Error(`role map bone ${b} is not in default.mhskel`);
  const boneOrder = Object.keys(skel.bones); // the file's order
  const keptOf = (b) => {
    if (KEPT.has(b)) return b;
    if (MERGE_INTO[b]) return MERGE_INTO[b];
    const p = skel.bones[b].parent;
    if (!p) throw new Error(`bone ${b} has no kept ancestor`);
    return keptOf(p);
  };
  const keptParent = (b) => (skel.bones[b].parent ? keptOf(skel.bones[b].parent) : null);
  // Joints parents first, depth first, children in the file's order: the skin order.
  const joints = [];
  const visit = (b) => {
    joints.push(b);
    for (const c of boneOrder) if (KEPT.has(c) && c !== b && keptParent(c) === b) visit(c);
  };
  const roots = keepList.filter((b) => keptParent(b) === null);
  if (roots.length !== 1) throw new Error(`the kept bones have ${roots.length} roots: ${roots}`);
  visit(roots[0]);
  if (joints.length !== keepList.length) throw new Error(`placed ${joints.length} of ${keepList.length} kept bones`);
  const jointIndex = new Map(joints.map((b, i) => [b, i]));

  // Weights per vertex, merged into the kept bones, the four largest, renormalised.
  const perVertex = new Map();
  for (const b of boneOrder) {
    const list = weightsIn[b];
    if (!list) continue;
    const j = jointIndex.get(keptOf(b));
    for (const [vi, w] of list) {
      if (!perVertex.has(vi)) perVertex.set(vi, new Map());
      const m = perVertex.get(vi);
      m.set(j, (m.get(j) || 0) + w);
    }
  }
  const skinOf = (vi) => {
    const m = perVertex.get(vi);
    if (!m) throw new Error(`vertex ${vi} has no weight`);
    const top = [...m.entries()].sort((a, b) => b[1] - a[1] || a[0] - b[0]).slice(0, 4);
    return unorm16(top);
  };
  const dominant = (vi) => {
    const m = perVertex.get(vi);
    let best = -1, bw = -1;
    for (const [j, w] of m) if (w > bw || (w === bw && j < best)) { best = j; bw = w; }
    return joints[best];
  };

  // Faces of the body, triangulated along the shorter diagonal of the base mesh (the same
  // triangles for both bodies).
  const bodyFaces = obj.faces.filter((f) => f.group === 'body');
  const base = obj.v;
  const quadTris = (f) => {
    const [a, b, c, d] = f.v;
    if (f.v.length === 3) return [[0, 1, 2]];
    return len(sub(base[a], base[c])) <= len(sub(base[b], base[d])) ? [[0, 1, 2], [0, 2, 3]] : [[0, 1, 3], [1, 2, 3]];
  };

  // Regions per face: the dominant bones' regions by majority, then three cuts by place on the base
  // mesh (decimetres), which give straight hems where the bones' borders zig-zag: the waist (the
  // torso under the waistband wears the trousers), the collar (round the neck, lower at the front)
  // and the scalp (vertex by vertex, a face is hair when all its corners are).
  const eyeCentreBase = (side) => mean(base, [...groupVerts[`helper-${side}-eye`]]);
  const jointBase = (b) => mean(base, skel.joints[skel.bones[b].head]);
  const faceRegion = bodyFaces.map((f) => {
    const count = {};
    for (const vi of f.v) {
      const r = regionOfBone(dominant(vi));
      count[r] = (count[r] || 0) + 1;
    }
    let best = null;
    for (const r of REGION_ORDER) if (count[r] && (best === null || count[r] > count[best])) best = r;
    return best;
  });
  const hipY = jointBase('root')[1];
  const neck = jointBase('neck01');
  const hairAt = scalpPicker(base, mul(add(eyeCentreBase('l'), eyeCentreBase('r')), 0.5));
  const clamp01 = (x) => Math.min(1, Math.max(0, x));
  bodyFaces.forEach((f, i) => {
    const doms = f.v.map(dominant);
    const c = mean(base, f.v);
    if (faceRegion[i] === 'shirt' && !doms.some((b) => /^(clavicle|upperarm|lowerarm)/.test(b)) && c[1] < hipY + WAIST_OVER_HIP_DM) faceRegion[i] = 'trousers';
    if ((faceRegion[i] === 'shirt' || faceRegion[i] === 'skin') && doms.every((b) => /^(spine01|neck|clavicle)/.test(b))) {
      const t = clamp01((c[2] - (neck[2] - 0.6)) / 1.2);
      faceRegion[i] = c[1] > neck[1] + 0.15 - 0.45 * t ? 'skin' : 'shirt';
    }
    if (faceRegion[i] === 'skin' && doms.every((b) => /^(head|neck)/.test(b)) && f.v.every((vi) => hairAt(base[vi]))) faceRegion[i] = 'hair';
  });

  // The targets, read once per gender.
  const bodies = {};
  for (const g of GENDERS) {
    const tg = (rel) => target(rel.replace('{g}', g));
    // The body's own shape: the base plus a third of each ancestry.
    const shape = base.map((p) => p.slice());
    for (const a of ANCESTRIES) for (const [i, d] of tg(`${a}-{g}-young`)) for (let k = 0; k < 3; k++) shape[i][k] += d[k] / 3;
    // Each target's delta per vertex (decimetres), dense.
    const deltas = TARGETS.map((t) => {
      const d = base.map(() => [0, 0, 0]);
      for (const file of t.files) for (const [i, v] of tg(file)) for (let k = 0; k < 3; k++) d[i][k] += v[k];
      for (const file of t.minus || []) for (const [i, v] of tg(file)) for (let k = 0; k < 3; k++) d[i][k] -= v[k] / 3;
      // The ground helper's centre (its corners may breathe: african-female-young moves them
      // +-0.01 dm symmetrically, which leaves the centre where it was).
      if (len(mean(d, groundVerts)) > 1e-6) throw new Error(`target ${t.name} moves the ground helper`);
      return d;
    });
    bodies[g] = makeBody(g, { obj, skel, shape, deltas, groundY, groupVerts, joints, jointIndex, keptParent, skinOf, bodyFaces, faceRegion, quadTris });
  }
  return bodies;
}

function mean(points, ids) {
  const s = [0, 0, 0];
  for (const i of ids) { s[0] += points[i][0]; s[1] += points[i][1]; s[2] += points[i][2]; }
  return mul(s, 1 / ids.length);
}

/**
 * Is a vertex of the base mesh (decimetres) on the scalp? Above a plane that runs from the
 * forehead's hairline down to the nape, and not on the ears (the hair comes down round them).
 * Measured on the base mesh 2026-11-09: the eyes' centre at y 7.28, z 1.25; the crown at 8.49; the
 * back of the skull at z -0.39; the ears out past |x| 0.64 between y 6.75 and 7.62, z -0.15 to 0.95.
 */
function scalpPicker(base, eye) {
  let top = -Infinity;
  for (let i = 0; i < BODY_VERTS; i++) top = Math.max(top, base[i][1]);
  // The hairline's height over the eyes, by the angle round the head seen from above (0 straight
  // ahead, 180 straight behind): high on the forehead, down to the top of the ear at the sides, down
  // to the nape behind. Level at the back, where the mesh's rows run level, so the edge is clean.
  const centreZ = (eye[2] + -0.39) / 2;
  const LINE = [[0, 0.62 * (top - eye[1])], [40, 0.5 * (top - eye[1])], [72, 0.08], [108, 0.08], [150, -0.55], [180, -0.6]];
  const hairline = (deg) => {
    for (let k = 1; k < LINE.length; k++) {
      if (deg <= LINE[k][0]) {
        const t = (deg - LINE[k - 1][0]) / (LINE[k][0] - LINE[k - 1][0]);
        return LINE[k - 1][1] + (LINE[k][1] - LINE[k - 1][1]) * t;
      }
    }
    return LINE[LINE.length - 1][1];
  };
  return (p) => {
    const deg = (Math.atan2(Math.abs(p[0]), p[2] - centreZ) * 180) / Math.PI;
    const ear = Math.abs(p[0]) > 0.64 && p[1] > eye[1] - 0.55 && p[1] < eye[1] + 0.34 && p[2] > eye[2] - 1.4 && p[2] < eye[2] - 0.3;
    return p[1] > eye[1] + hairline(deg) && !ear;
  };
}

/**
 * A brow over one eye of a shaped body (metres): a tapered, arched strip laid on the skin 0.8 mm out,
 * as generated geometry so its edge is smooth whatever the mesh's faces are. `hit` finds the skin in
 * front of a point. Returns its vertices (position, normal, the surface's morph deltas there) and
 * triangles.
 */
function browStrip(eye, side, hit) {
  const s = side === 'l' ? 1 : -1;
  const n = 14;
  const verts = [];
  for (let k = 0; k <= n; k++) {
    const u = k / n; // medial (0) to lateral (1)
    const x = eye[0] + s * (-0.018 + 0.045 * u);
    const mid = eye[1] + 0.0155 + 0.0055 * (1 - ((u - 0.6) / 0.6) ** 2);
    const half = 0.0045 * (1 - u) + 0.0013 * u;
    for (const y of [mid + half, mid - half]) {
      const h = hit([x, y, eye[2] + 0.2]);
      if (!h) throw new Error(`no skin in front of the ${side} brow at u ${u}`);
      verts.push({ p: add(h.p, mul(h.n, 0.0008)), n: h.n, deltas: h.deltas });
    }
  }
  const tris = [];
  for (let k = 0; k < n; k++) {
    const [t0, b0, t1, b1] = [2 * k, 2 * k + 1, 2 * k + 2, 2 * k + 3];
    // Wound to face out of the skin (+Z for a face looking forward).
    const fwd = cross(sub(verts[t1].p, verts[b0].p), sub(verts[t0].p, verts[b0].p))[2] > 0;
    if (fwd) tris.push([b0, t1, t0], [b0, b1, t1]);
    else tris.push([b0, t0, t1], [b0, t1, b1]);
  }
  return { verts, tris };
}

/**
 * The first skin a ray from `from` straight back (-Z) meets: the point, the smooth normal there and
 * each target's delta there (all by the triangle's barycentric weights). Null if it misses.
 */
function skinInFront(from, tris, P, N, D) {
  let best = null;
  for (const [a, b, c] of tris) {
    // Moller-Trumbore with the direction (0, 0, -1).
    const e1 = sub(P[b], P[a]);
    const e2 = sub(P[c], P[a]);
    const dir = [0, 0, -1];
    const h = cross(dir, e2);
    const det = dot(e1, h);
    if (Math.abs(det) < 1e-12) continue;
    const s = sub(from, P[a]);
    const u = dot(s, h) / det;
    if (u < 0 || u > 1) continue;
    const q = cross(s, e1);
    const v = dot(dir, q) / det;
    if (v < 0 || u + v > 1) continue;
    const t = dot(e2, q) / det;
    if (t <= 0 || (best && t >= best.t)) continue;
    best = { t, a, b, c, u, v };
  }
  if (!best) return null;
  const { a, b, c, u, v } = best;
  const w = 1 - u - v;
  const blend = (arr) => add(add(mul(arr[a], w), mul(arr[b], u)), mul(arr[c], v));
  return { p: blend(P), n: norm(blend(N)), deltas: D.map((d) => blend(d)) };
}

/** Four (joint, weight) pairs as unorm16 summing to exactly 65535 (the reader's own rule). */
function unorm16(top) {
  const pairs = top.concat([[0, 0], [0, 0], [0, 0]]).slice(0, 4);
  const sum = pairs.reduce((s, p) => s + p[1], 0);
  const w = pairs.map((p) => Math.round((p[1] / sum) * 65535));
  const total = w.reduce((s, x) => s + x, 0);
  let largest = 0;
  for (let k = 1; k < 4; k++) if (pairs[k][1] > pairs[largest][1]) largest = k;
  w[largest] += 65535 - total;
  return { joints: pairs.map((p) => p[0]), weights: w };
}

/** Smooth vertex normals of a triangle list over `points` (area-weighted), for the listed vertices. */
function vertexNormals(points, tris, count) {
  const n = new Array(count).fill(null).map(() => [0, 0, 0]);
  for (const [a, b, c] of tris) {
    const fn = cross(sub(points[b], points[a]), sub(points[c], points[a]));
    for (const i of [a, b, c]) { n[i][0] += fn[0]; n[i][1] += fn[1]; n[i][2] += fn[2]; }
  }
  return n.map(norm);
}

/** A UV sphere looking along +Z: rings of polar angle (degrees from +Z) with their region. */
const EYE_RINGS = [[0, 'pupil'], [8, 'pupil'], [14, 'iris'], [25, 'iris'], [32, 'sclera'], [50, 'sclera'], [75, 'sclera'], [105, 'sclera'], [140, 'sclera'], [180, 'sclera']];
const EYE_SEGMENTS = 20;

function makeBody(gender, ctx) {
  const { obj, shape, deltas, groundY, groupVerts, joints, jointIndex, keptParent, skinOf, bodyFaces, faceRegion, quadTris, skel } = ctx;
  const toM = (p) => [p[0] * DM, (p[1] - groundY) * DM, p[2] * DM];
  const P = shape.map(toM);
  const D = deltas.map((d) => d.map((v) => mul(v, DM)));

  // Joint positions (metres) and their movement under each target.
  const jointPos = joints.map((b) => mean(P, skel.joints[skel.bones[b].head]));
  const jointDeltas = D.map((d) => joints.map((b) => mean(d, skel.joints[skel.bones[b].head])));

  // The shirt reads as cloth over the body, not paint on it: the inside of the shirt region (every
  // face round a vertex the shirt's, so the hems stay put and nothing cracks) smoothed by Taubin's
  // two-step filter, which takes out small detail without shrinking the torso.
  const near = new Array(BODY_VERTS).fill(null).map(() => new Set());
  const allShirt = new Array(BODY_VERTS).fill(true);
  bodyFaces.forEach((f, fi) => {
    for (let k = 0; k < f.v.length; k++) {
      near[f.v[k]].add(f.v[(k + 1) % f.v.length]);
      near[f.v[(k + 1) % f.v.length]].add(f.v[k]);
      if (faceRegion[fi] !== 'shirt') allShirt[f.v[k]] = false;
    }
  });
  for (let step = 0; step < SHIRT_SMOOTH_STEPS; step++) {
    const factor = step % 2 === 0 ? 0.5 : -0.53;
    const next = P.slice(0, BODY_VERTS).map((p, i) => {
      if (!allShirt[i] || near[i].size === 0) return p;
      let avg = [0, 0, 0];
      for (const j of [...near[i]].sort((a, b) => a - b)) avg = add(avg, P[j]);
      avg = mul(avg, 1 / near[i].size);
      return add(p, mul(sub(avg, p), factor));
    });
    for (let i = 0; i < BODY_VERTS; i++) P[i] = next[i];
  }

  // Normals of the body's surface, at its own shape and at each target (for the NORMAL deltas).
  const allTris = [];
  for (const f of bodyFaces) for (const t of quadTris(f)) allTris.push(t.map((k) => f.v[k]));
  const normalsAt = (pts) => vertexNormals(pts, allTris, BODY_VERTS);
  const N = normalsAt(P);
  const ND = D.map((d) => {
    const moved = P.map((p, i) => (i < BODY_VERTS ? add(p, d[i]) : p));
    const nm = normalsAt(moved);
    return N.map((n, i) => sub(nm[i], n));
  });

  // The hair's shell thins to nothing over its last three rings of vertices, so its edge lies on the
  // scalp (which is hair-coloured too) instead of standing out as a lip along the hairline's steps.
  const hairRing = new Array(BODY_VERTS).fill(Infinity);
  const inHair = new Array(BODY_VERTS).fill(false);
  const outHair = new Array(BODY_VERTS).fill(false);
  bodyFaces.forEach((f, fi) => { for (const vi of f.v) (faceRegion[fi] === 'hair' ? inHair : outHair)[vi] = true; });
  let frontier = [];
  for (let i = 0; i < BODY_VERTS; i++) if (inHair[i] && outHair[i]) { hairRing[i] = 0; frontier.push(i); }
  for (let ring = 1; frontier.length; ring++) {
    const next = [];
    for (const i of frontier) for (const j of [...near[i]].sort((a, b) => a - b)) if (inHair[j] && hairRing[j] === Infinity) { hairRing[j] = ring; next.push(j); }
    frontier = next;
  }
  const shellTaper = (vi) => Math.min(1, hairRing[vi] / 3);

  // The primitives: vertices made per (region, base vertex, uv), in face order.
  const prims = Object.fromEntries(MATERIALS.map((m) => [m, newPrim(m)]));
  const keyOf = new Map();
  const vertexFor = (region, vi, ti, opts = {}) => {
    const key = `${region}|${vi}|${ti}|${opts.shell ? 1 : 0}`;
    if (keyOf.has(key)) return keyOf.get(key);
    const pr = prims[region];
    const idx = pr.pos.length;
    let p = P[vi];
    if (opts.shell) p = add(p, mul(N[vi], opts.shell * shellTaper(vi)));
    pr.pos.push(p);
    pr.nrm.push(N[vi]);
    pr.uv.push(ti != null && obj.vt[ti] ? [obj.vt[ti][0], 1 - obj.vt[ti][1]] : [0, 0]);
    pr.skin.push(skinOf(vi));
    pr.morph.push(D.map((d, k) => ({ p: d[vi], n: ND[k][vi] })));
    keyOf.set(key, idx);
    return idx;
  };
  bodyFaces.forEach((f, fi) => {
    const region = faceRegion[fi];
    for (const t of quadTris(f)) {
      const ids = t.map((k) => vertexFor(region, f.v[k], f.vt[k]));
      prims[region].idx.push(...ids);
      if (region === 'hair') {
        // The hair's shell over the scalp, 6 mm out where it is not thinning to its edge.
        const sh = t.map((k) => vertexFor('hair', f.v[k], f.vt[k], { shell: 0.006 }));
        prims.hair.idx.push(...sh);
      }
    }
  });

  // The brows: generated strips on the skin over each eye, on the head bone, following the skin
  // under every shape target.
  const headTris = allTris.filter((t) => t.every((i) => P[i][1] > jointPos[jointIndex.get('neck03')][1]));
  const hit = (from) => skinInFront(from, headTris, P, N, D);
  const headBone = jointIndex.get('head');
  for (const side of ['l', 'r']) {
    const ids = [...groupVerts[`helper-${side}-eye`]];
    const strip = browStrip(mean(P, ids), side, hit);
    const pr = prims.brows;
    const base0 = pr.pos.length;
    for (const v of strip.verts) {
      pr.pos.push(v.p);
      pr.nrm.push(v.n);
      pr.uv.push([0, 0]);
      pr.skin.push({ joints: [headBone, 0, 0, 0], weights: [65535, 0, 0, 0] });
      pr.morph.push(v.deltas.map((d) => ({ p: d, n: [0, 0, 0] })));
    }
    for (const t of strip.tris) pr.idx.push(...t.map((k) => base0 + k));
  }

  // The eyes: generated spheres where MakeHuman's eye helpers sit, each on its eye bone.
  for (const side of ['l', 'r']) {
    const ids = [...groupVerts[`helper-${side}-eye`]].sort((a, b) => a - b);
    const centre = mean(P, ids);
    let r = 0;
    for (const i of ids) r += len(sub(P[i], centre));
    r /= ids.length;
    const bone = jointIndex.get(`eye.${side.toUpperCase()}`);
    const move = D.map((d) => mean(d, ids));
    const ringIdx = [];
    EYE_RINGS.forEach(([deg, region], ri) => {
      const th = (deg * Math.PI) / 180;
      const row = {};
      // Each ring is written once per region it borders, so a region's faces share its vertices.
      for (const reg of new Set([region, (EYE_RINGS[ri + 1] || EYE_RINGS[ri])[1]])) {
        const segs = deg === 0 || deg === 180 ? 1 : EYE_SEGMENTS;
        row[reg] = [];
        for (let s = 0; s < segs; s++) {
          const ph = (s / EYE_SEGMENTS) * 2 * Math.PI;
          const dir = [Math.sin(th) * Math.cos(ph), Math.sin(th) * Math.sin(ph), Math.cos(th)];
          const pr = prims[reg];
          row[reg].push(pr.pos.length);
          pr.pos.push(add(centre, mul(dir, r)));
          pr.nrm.push(dir);
          pr.uv.push([s / EYE_SEGMENTS, deg / 180]);
          pr.skin.push({ joints: [bone, 0, 0, 0], weights: [65535, 0, 0, 0] });
          pr.morph.push(move.map((m) => ({ p: m, n: [0, 0, 0] })));
        }
      }
      ringIdx.push(row);
    });
    for (let ri = 0; ri + 1 < EYE_RINGS.length; ri++) {
      const reg = EYE_RINGS[ri + 1][1] === EYE_RINGS[ri][1] ? EYE_RINGS[ri][1] : EYE_RINGS[ri + 1][1];
      const a = ringIdx[ri][reg];
      const b = ringIdx[ri + 1][reg];
      const pr = prims[reg];
      for (let s = 0; s < EYE_SEGMENTS; s++) {
        const s1 = (s + 1) % EYE_SEGMENTS;
        const a0 = a[a.length === 1 ? 0 : s], a1 = a[a.length === 1 ? 0 : s1];
        const b0 = b[b.length === 1 ? 0 : s], b1 = b[b.length === 1 ? 0 : s1];
        // Outward winding (counter-clockwise seen from outside).
        if (a.length !== 1) pr.idx.push(a0, b0, a1);
        if (b.length !== 1) pr.idx.push(a1, b0, b1);
      }
    }
  }

  // The rig: identity rest rotations, each joint at its bone's head.
  const parentIdx = joints.map((b) => (keptParent(b) ? jointIndex.get(keptParent(b)) : null));
  const rig = joints.map((b, i) => ({
    name: b,
    parent: parentIdx[i],
    t: parentIdx[i] == null ? jointPos[i] : sub(jointPos[i], jointPos[parentIdx[i]]),
    ibm: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, -jointPos[i][0], -jointPos[i][1], -jointPos[i][2], 1],
  }));
  let top = -Infinity;
  for (let i = 0; i < BODY_VERTS; i++) top = Math.max(top, P[i][1]);
  return { gender, prims: MATERIALS.map((m) => prims[m]).filter((p) => p.idx.length), rig, jointDeltas, height: top, triangles: allTris.length };
}

function newPrim(material) {
  return { material, pos: [], nrm: [], uv: [], skin: [], morph: [], idx: [] };
}

// ── Writing ──────────────────────────────────────────────────────────────
function writeBody(body) {
  const name = `makehuman_${body.gender}`;
  const g = new GltfBuilder();
  const nodes = [];
  const jointNodes = body.rig.map((j) => {
    nodes.push({ name: j.name, translation: j.t.map(Math.fround) });
    return nodes.length - 1;
  });
  body.rig.forEach((j, i) => {
    if (j.parent == null) return;
    const pn = nodes[jointNodes[j.parent]];
    (pn.children = pn.children || []).push(jointNodes[i]);
  });
  const targetNames = TARGETS.map((t) => t.name);
  const primitives = body.prims.map((p, pi) => {
    const n = p.pos.length;
    const flat = (arr, w) => { const out = new Float32Array(n * w); arr.forEach((v, i) => { for (let k = 0; k < w; k++) out[i * w + k] = v[k]; }); return out; };
    const joints = new Uint8Array(n * 4);
    const weights = new Uint16Array(n * 4);
    p.skin.forEach((s, i) => { for (let k = 0; k < 4; k++) { joints[i * 4 + k] = s.joints[k]; weights[i * 4 + k] = s.weights[k]; } });
    const attributes = {
      POSITION: g.accessor(flat(p.pos, 3), 'VEC3', { target: 34962, minMax: true }),
      NORMAL: g.accessor(flat(p.nrm, 3), 'VEC3', { target: 34962 }),
      TEXCOORD_0: g.accessor(flat(p.uv, 2), 'VEC2', { target: 34962 }),
      JOINTS_0: g.accessor(joints, 'VEC4', { target: 34962 }),
      WEIGHTS_0: g.accessor(weights, 'VEC4', { target: 34962, normalized: true }),
    };
    const indices = n < 65536 ? Uint16Array.from(p.idx) : Uint32Array.from(p.idx);
    const targets = targetNames.map((tn, k) => ({
      POSITION: morphAccessor(g, p.morph.map((m) => m[k].p), `morph:${tn}:${p.material}`, true),
      NORMAL: morphAccessor(g, p.morph.map((m) => m[k].n), null, false),
    }));
    return { attributes, indices: g.accessor(indices, 'SCALAR', { target: 34963 }), material: pi, targets };
  });
  // Each target's movement of every joint (see the header).
  targetNames.forEach((tn, k) => {
    const d = new Float32Array(body.rig.length * 3);
    body.jointDeltas[k].forEach((v, i) => { d[i * 3] = v[0]; d[i * 3 + 1] = v[1]; d[i * 3 + 2] = v[2]; });
    g.accessor(d, 'VEC3', { name: `joint_deltas:${tn}` });
  });
  const ibm = new Float32Array(body.rig.length * 16);
  body.rig.forEach((j, i) => ibm.set(j.ibm, i * 16));
  const ibmAcc = g.accessor(ibm, 'MAT4');
  const meshNode = nodes.length;
  nodes.push({ name: `${name}_mesh`, mesh: 0, skin: 0 });
  const gltf = {
    asset: {
      version: '2.0',
      generator: 'HumanityOS scripts/gen-makehuman-body.js',
      copyright: `MakeHuman assets, CC0 1.0 Universal (${SOURCE.repo}, commit ${SOURCE.commit.slice(0, 8)}); converted by HumanityOS`,
    },
    scene: 0,
    scenes: [{ name, nodes: [jointNodes[0], meshNode] }],
    nodes,
    meshes: [{ name, primitives, weights: targetNames.map(() => 0), extras: { targetNames } }],
    materials: body.prims.map((p) => ({ name: p.material, pbrMetallicRoughness: { baseColorFactor: [0.7, 0.7, 0.7, 1], metallicFactor: 0, roughnessFactor: 0.7 } })),
    skins: [{ name: `${name}_skin`, joints: jointNodes, inverseBindMatrices: ibmAcc, skeleton: jointNodes[0] }],
  };
  return g.finish(gltf, `${name}.bin`);
}

/** A morph target's deltas: sparse when fewer than 60% of the vertices move, else dense. */
function morphAccessor(g, deltas, name, minMax) {
  const n = deltas.length;
  const moved = [];
  deltas.forEach((d, i) => { if (Math.abs(d[0]) > 1e-7 || Math.abs(d[1]) > 1e-7 || Math.abs(d[2]) > 1e-7) moved.push(i); });
  const opts = { minMax };
  if (name) opts.name = name;
  if (moved.length < 0.6 * n) {
    const indices = n < 65536 ? Uint16Array.from(moved) : Uint32Array.from(moved);
    const values = new Float32Array(moved.length * 3);
    moved.forEach((i, k) => { values[k * 3] = deltas[i][0]; values[k * 3 + 1] = deltas[i][1]; values[k * 3 + 2] = deltas[i][2]; });
    if (moved.length === 0) {
      // glTF requires a sparse count of at least one: list the first vertex with a zero delta.
      return g.sparse(n, 'VEC3', n < 65536 ? Uint16Array.from([0]) : Uint32Array.from([0]), new Float32Array(3), opts);
    }
    return g.sparse(n, 'VEC3', indices, values, opts);
  }
  const flat = new Float32Array(n * 3);
  deltas.forEach((d, i) => { flat[i * 3] = d[0]; flat[i * 3 + 1] = d[1]; flat[i * 3 + 2] = d[2]; });
  return g.accessor(flat, 'VEC3', opts);
}

function sourceNote(files, bodies) {
  const sha = (b) => crypto.createHash('sha256').update(b).digest('hex');
  const lines = [
    '# MakeHuman bodies: where they come from',
    '',
    'Made by `scripts/gen-makehuman-body.js` (HumanityOS, character animation rung 1, 2026-11-09) from',
    'MakeHuman\'s data files. Do not edit these files by hand: change the converter and run it again.',
    '',
    '## Licence',
    '',
    'The MakeHuman assets these are made from are dedicated to the public domain under **CC0 1.0',
    'Universal** (https://creativecommons.org/publicdomain/zero/1.0/). MakeHuman\'s `LICENSE.md`,',
    'section C: "These assets have been released under CC0 1.0 Universal."; `default.mhskel` carries',
    '`"license": "CC0"`. MakeHuman\'s source code is AGPL and none of it is used here: the converter',
    'reads the data files\' formats itself. Finding:',
    '`docs/reference/findings/2026-11-08-character-assets.md` (with its 2026-11-09 addendum).',
    '',
    '## Source',
    '',
    `- Repository: ${SOURCE.repo}`,
    `- Commit: \`${SOURCE.commit}\` (${SOURCE.commitDate})`,
    `- Fetched: ${SOURCE.fetched}, approved by ${SOURCE.approved}`,
    '- Files read (under `makehuman/data/`), each checked against its git blob SHA-1 before reading:',
    '',
    ...INPUTS.map(([rel, s]) => `  - \`${rel}\` \`${s}\``),
    '',
    '## What was made',
    '',
    ...bodies.map((b) => `- \`makehuman_${b.gender}\`: ${b.prims.reduce((s, p) => s + p.pos.length, 0)} vertices, ${b.prims.reduce((s, p) => s + p.idx.length / 3, 0)} triangles, ${b.rig.length} joints, ${TARGETS.length} shape targets, ${b.height.toFixed(3)} m tall`),
    '',
    '## Output hashes (SHA-256)',
    '',
    '`node scripts/gen-makehuman-body.js --check` compares the files with these when the sources are',
    'not on the machine.',
    '',
    ...files.map(([p, bytes]) => `- \`${path.basename(p)}\` \`${sha(bytes)}\``),
    '',
  ];
  return lines.join('\n');
}

// ── A look without a boot: front, side and back views as PNG ─────────────
const PREVIEW_COLOURS = {
  skin: [205, 160, 130], hair: [70, 45, 30], brows: [60, 40, 28], shirt: [40, 150, 175], trousers: [55, 58, 70],
  shoes: [35, 30, 28], hands: [215, 170, 140], sclera: [240, 240, 236], iris: [70, 110, 140], pupil: [15, 15, 15],
};

function preview(bodies, dir) {
  fs.mkdirSync(dir, { recursive: true });
  for (const b of bodies) {
    for (const [view, rot] of [['front', (p) => [p[0], p[1], p[2]]], ['side', (p) => [p[2], p[1], -p[0]]], ['back', (p) => [-p[0], p[1], -p[2]]], ['face', (p) => [p[0], p[1], p[2]]], ['headside', (p) => [p[2], p[1], -p[0]]], ['headback', (p) => [-p[0], p[1], -p[2]]]]) {
      const face = view === 'face' || view.startsWith('head');
      const W = face ? 400 : 360, H = face ? 400 : 760;
      const img = Buffer.alloc(W * H * 3, 235);
      const zb = new Float32Array(W * H).fill(-Infinity);
      const scale = face ? 1700 : 380;
      const cy = face ? b.height - 0.13 : b.height / 2;
      const light = norm([0.4, 0.6, 0.7]);
      for (const p of b.prims) {
        const col = PREVIEW_COLOURS[p.material] || [255, 0, 255];
        const pts = p.pos.map(rot).map((q) => [W / 2 + q[0] * scale, H / 2 - (q[1] - cy) * scale, q[2]]);
        for (let t = 0; t < p.idx.length; t += 3) {
          const [a, c, d] = [p.idx[t], p.idx[t + 1], p.idx[t + 2]];
          const n = norm(cross(sub(p.pos[c].map((x) => x), p.pos[a]), sub(p.pos[d], p.pos[a])));
          const nr = rot(n);
          if (nr[2] <= 0) continue; // back-facing
          // Smooth shading from the vertex normals, as the game lights a body.
          const sh = [a, c, d].map((k) => 0.35 + 0.65 * Math.max(0, dot(rot(p.nrm[k]), light)));
          tri(img, zb, W, H, pts[a], pts[c], pts[d], col, sh);
        }
      }
      fs.writeFileSync(path.join(dir, `makehuman_${b.gender}_${view}.png`), png(W, H, img));
    }
  }
}

function tri(img, zb, W, H, a, b, c, col, sh) {
  const minX = Math.max(0, Math.floor(Math.min(a[0], b[0], c[0]))), maxX = Math.min(W - 1, Math.ceil(Math.max(a[0], b[0], c[0])));
  const minY = Math.max(0, Math.floor(Math.min(a[1], b[1], c[1]))), maxY = Math.min(H - 1, Math.ceil(Math.max(a[1], b[1], c[1])));
  const area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
  if (Math.abs(area) < 1e-9) return;
  for (let y = minY; y <= maxY; y++) for (let x = minX; x <= maxX; x++) {
    const px = x + 0.5, py = y + 0.5;
    const w0 = ((b[0] - px) * (c[1] - py) - (b[1] - py) * (c[0] - px)) / area;
    const w1 = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) / area;
    const w2 = 1 - w0 - w1;
    if (w0 < 0 || w1 < 0 || w2 < 0) continue;
    const z = w0 * a[2] + w1 * b[2] + w2 * c[2];
    const k = y * W + x;
    if (z <= zb[k]) continue;
    zb[k] = z;
    const shade = w0 * sh[0] + w1 * sh[1] + w2 * sh[2];
    img[k * 3] = Math.min(255, col[0] * shade); img[k * 3 + 1] = Math.min(255, col[1] * shade); img[k * 3 + 2] = Math.min(255, col[2] * shade);
  }
}

function png(W, H, rgb) {
  const crcTable = [];
  for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; crcTable.push(c >>> 0); }
  const crc = (buf) => { let c = 0xffffffff; for (const x of buf) c = crcTable[(c ^ x) & 0xff] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; };
  const chunk = (type, data) => {
    const t = Buffer.from(type);
    const l = Buffer.alloc(4); l.writeUInt32BE(data.length);
    const c = Buffer.alloc(4); c.writeUInt32BE(crc(Buffer.concat([t, data])));
    return Buffer.concat([l, t, data, c]);
  };
  const raw = Buffer.alloc((W * 3 + 1) * H);
  for (let y = 0; y < H; y++) { raw[y * (W * 3 + 1)] = 0; rgb.copy(raw, y * (W * 3 + 1) + 1, y * W * 3, (y + 1) * W * 3); }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(W, 0); ihdr.writeUInt32BE(H, 4); ihdr[8] = 8; ihdr[9] = 2;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}

// ── Main ─────────────────────────────────────────────────────────────────
function argValue(flag) {
  const i = process.argv.indexOf(flag);
  return i >= 0 ? process.argv[i + 1] : null;
}

function main() {
  const check = process.argv.includes('--check');
  const src = argValue('--src') || DEFAULT_SRC;
  const previewDir = argValue('--preview');
  const note = path.join(OUT, 'SOURCE.md');
  if (!fs.existsSync(path.join(src, '3dobjs', 'base.obj'))) {
    if (!check) {
      console.error(`the MakeHuman sources are not at ${src} (--src DIR or HUMANITY_MAKEHUMAN_SRC)`);
      process.exit(1);
    }
    // Without the sources: the committed files against the hashes SOURCE.md records.
    // (A CRLF checkout of SOURCE.md reads the same.)
    const text = fs.existsSync(note) ? fs.readFileSync(note, 'utf8').replace(/\r\n/g, '\n') : '';
    const want = [...text.matchAll(/^- `([^`]+)` `([0-9a-f]{64})`$/gm)];
    if (want.length === 0) { console.error(`${path.relative(ROOT, note)} records no output hashes`); process.exit(1); }
    let bad = 0;
    for (const [, file, hash] of want) {
      const p = path.join(OUT, file);
      let bytes = fs.existsSync(p) ? fs.readFileSync(p) : null;
      if (bytes && file.endsWith('.gltf')) bytes = Buffer.from(bytes.toString().replace(/\r\n/g, '\n'));
      const got = bytes ? crypto.createHash('sha256').update(bytes).digest('hex') : 'missing';
      if (got !== hash) { console.error(`${file}: ${got}, SOURCE.md records ${hash}`); bad++; }
    }
    if (bad) process.exit(1);
    console.log(`the MakeHuman sources are not on this machine (${src}): checked the ${want.length} committed files against SOURCE.md's hashes only`);
    return;
  }
  const bodies = Object.values(build(src));
  const files = [];
  for (const b of bodies) {
    const made = writeBody(b);
    files.push([path.join(OUT, `makehuman_${b.gender}.gltf`), Buffer.from(made.json)]);
    files.push([path.join(OUT, `makehuman_${b.gender}.bin`), made.bin]);
  }
  files.push([note, Buffer.from(sourceNote(files, bodies))]);
  if (previewDir) preview(bodies, previewDir);
  if (check) {
    const bad = differing(files, fs, path, ROOT);
    for (const p of bad) console.error(`${p} differs from what scripts/gen-makehuman-body.js makes`);
    if (bad.length) process.exit(1);
    console.log('the MakeHuman bodies match the converter');
    return;
  }
  fs.mkdirSync(OUT, { recursive: true });
  for (const [p, bytes] of files) {
    fs.writeFileSync(p, bytes);
    console.log(`wrote ${path.relative(ROOT, p)} (${bytes.length} bytes)`);
  }
  for (const b of bodies) console.log(`${b.gender}: ${b.height.toFixed(3)} m, ${b.rig.length} joints, ${b.prims.reduce((s, p) => s + p.pos.length, 0)} vertices, ${b.triangles} body triangles`);
}

if (require.main === module) main();
module.exports = { build, TARGETS, MATERIALS, regionOfBone, unorm16 };
