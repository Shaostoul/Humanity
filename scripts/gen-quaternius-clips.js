#!/usr/bin/env node
// THE QUATERNIUS CLIPS WE USE, TRIMMED (character animation rung 1, 2026-11-09;
// docs/design/character-animation.md sections 2.3 and 13). Quaternius's Universal Animation Library
// [Standard] is CC0 by the License.txt inside the download (finding
// docs/reference/findings/2026-11-08-character-assets.md, its 2026-11-09 addendum). Its file holds 43
// clips and a mannequin mesh; the game plays a dozen of them on the MakeHuman body, retargeted by
// role at load (crates/humanity-animation/src/retarget.rs). This writes just those clips, on the
// library's 65-joint rig (Unreal mannequin names, role map data/characters/role_maps/quaternius_ual.ron),
// with no mesh:
//
//   assets/models/characters/quaternius_ual/quaternius_ual.gltf + .bin
//   assets/models/characters/quaternius_ual/License.txt   Quaternius's own, copied byte for byte
//   assets/models/characters/quaternius_ual/SOURCE.md     the download, its hashes, each clip's speed
//
// From the in-place file (UAL1_Standard.glb, root motion off: the capsule is the truth, plan 3.3).
// Each kept channel's keys are copied exactly; a channel that never leaves the joint's rest (the
// bones' lengths, the scales) is left out, which the reader treats as the rest anyway.
//
// EACH GAIT'S SPEED is measured here, from the feet: the clip is played through the rig (forward
// kinematics), and while a foot is down (its ball within 2 cm of the lowest it gets) it slides back
// under the in-place body at the speed the body would travel; the median of that, both feet, is the
// clip's ground speed. data/characters/clips.ron's speed_mps takes these numbers, so the stride
// warping plays each clip at the rate that keeps its feet still (the root-motion file's own travel is
// recorded beside them: an artist's choice, not the feet's).
//
//   node scripts/gen-quaternius-clips.js            write the files (needs the download)
//   node scripts/gen-quaternius-clips.js --check    exit 1 if the committed files differ
//   --src DIR    the download's folder (default HUMANITY_QUATERNIUS_SRC, else
//                E:\HumanityOS\quaternius-src\ual-standard-2026-11-09)
//
// --check without the download compares the committed files with the hashes SOURCE.md records, and
// says so. The download is untrusted input: its files are checked against the SHA-256 hashes below
// before they are read, read as data, and nothing in it is run.

'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const { GltfBuilder, differing } = require('./lib/gltf-builder.js');

const ROOT = path.join(__dirname, '..');
const OUT = path.join(ROOT, 'assets', 'models', 'characters', 'quaternius_ual');
const NAME = 'quaternius_ual';
const DEFAULT_SRC = process.env.HUMANITY_QUATERNIUS_SRC || 'E:\\HumanityOS\\quaternius-src\\ual-standard-2026-11-09';
const PACK = 'Universal Animation Library[Standard]';
const SOURCE = {
  page: 'https://quaternius.itch.io/universal-animation-library',
  zip: `${PACK}.zip`,
  zipSha256: 'cc73fc4e495b82958207316596317a3f40b9fa38065bde1027937452da537724',
  zipBytes: 15904933,
  version: 'v3.0 (16/6/2026), the free Standard tier',
  downloaded: '2026-11-09',
  approved: 'the operator, 2026-11-09 ("Yes, let\'s see what it has.")',
};
// The extracted files read, with their SHA-256 (each also matches the zip's own CRC-32 for it,
// checked 2026-11-09).
const INPUTS = {
  glb: [`x/${PACK}/Unreal-Godot/UAL1_Standard.glb`, '69591853d817488edaa8fd9bf8fc1d821eaeaf789f8627b3cd23b41c4ed67997'],
  rootMotion: [`x/${PACK}/Unreal-Godot/UAL1_Standard_RM.glb`, null],
  license: [`x/${PACK}/License.txt`, '6d01f55c6e4c49a2c9963e147e561945ae2c83958c8ca667d90a6bffdbfac061'],
};
// The clips the game uses (data/characters/clips.ron names them by these names).
const CLIPS = [
  'Idle_Loop', 'Walk_Loop', 'Jog_Fwd_Loop', 'Sprint_Loop',
  'Crouch_Idle_Loop', 'Crouch_Fwd_Loop',
  'Jump_Start', 'Jump_Loop', 'Jump_Land',
  'Sitting_Enter', 'Sitting_Idle_Loop', 'Sitting_Exit',
];
// The gaits whose ground speed is measured, and the feet they are measured on.
const GAITS = ['Walk_Loop', 'Jog_Fwd_Loop', 'Sprint_Loop', 'Crouch_Fwd_Loop'];
const FEET = ['ball_l', 'ball_r'];

// ── Reading a GLB ─────────────────────────────────────────────────────────
function readGlb(buf) {
  if (buf.readUInt32LE(0) !== 0x46546c67) throw new Error('not a GLB');
  const jsonLen = buf.readUInt32LE(12);
  const json = JSON.parse(buf.slice(20, 20 + jsonLen).toString('utf8'));
  const binLen = buf.readUInt32LE(20 + jsonLen);
  const bin = buf.slice(28 + jsonLen, 28 + jsonLen + binLen);
  const WIDTH = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 };
  const accessor = (i) => {
    const a = json.accessors[i];
    if (a.componentType !== 5126 || a.sparse) throw new Error(`accessor ${i}: only dense floats are expected here`);
    const v = json.bufferViews[a.bufferView];
    const w = WIDTH[a.type];
    const stride = v.byteStride || w * 4;
    const out = new Float32Array(a.count * w);
    const at = (v.byteOffset || 0) + (a.byteOffset || 0);
    for (let k = 0; k < a.count; k++) for (let c = 0; c < w; c++) out[k * w + c] = bin.readFloatLE(at + k * stride + c * 4);
    return out;
  };
  return { json, accessor };
}

function sha256(buf) {
  return crypto.createHash('sha256').update(buf).digest('hex');
}

// ── Small maths for the speed measurement ─────────────────────────────────
function qrot(q, v) {
  const [x, y, z, w] = q;
  const ix = w * v[0] + y * v[2] - z * v[1];
  const iy = w * v[1] + z * v[0] - x * v[2];
  const iz = w * v[2] + x * v[1] - y * v[0];
  const iw = -x * v[0] - y * v[1] - z * v[2];
  return [ix * w + iw * -x + iy * -z - iz * -y, iy * w + iw * -y + iz * -x - ix * -z, iz * w + iw * -z + ix * -y - iy * -x];
}
function qmul(a, b) {
  return [
    a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
    a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
    a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
    a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
  ];
}

/** World positions of every node at key `k` of a clip (keys are shared: 30 a second). */
function poseAt(json, rest, tracks, k) {
  const local = rest.map((r, i) => {
    const t = tracks.get(i) || {};
    const pick = (arr, w, fallback) => (arr ? Array.from(arr.slice(Math.min(k, arr.length / w - 1) * w, Math.min(k, arr.length / w - 1) * w + w)) : fallback);
    return { t: pick(t.translation, 3, r.t), r: pick(t.rotation, 4, r.r), s: pick(t.scale, 3, r.s) };
  });
  const world = new Array(rest.length);
  const place = (i, parent) => {
    const l = local[i];
    if (!parent) world[i] = { p: l.t, q: l.r, s: l.s };
    else {
      const sc = [l.t[0] * parent.s[0], l.t[1] * parent.s[1], l.t[2] * parent.s[2]];
      const p = qrot(parent.q, sc);
      world[i] = { p: [parent.p[0] + p[0], parent.p[1] + p[1], parent.p[2] + p[2]], q: qmul(parent.q, l.r), s: [parent.s[0] * l.s[0], parent.s[1] * l.s[1], parent.s[2] * l.s[2]] };
    }
    for (const c of json.nodes[i].children || []) place(c, world[i]);
  };
  const roots = json.scenes[json.scene || 0].nodes;
  for (const r of roots) place(r, null);
  return world;
}

/** A gait's ground speed from its feet (see the header), metres a second. */
function gaitSpeed(json, accessor, rest, anim) {
  const tracks = tracksOf(json, accessor, anim);
  const times = accessor(anim.samplers[anim.channels[0].sampler].input);
  const n = times.length;
  const dt = (times[n - 1] - times[0]) / (n - 1);
  const feet = FEET.map((f) => json.nodes.findIndex((nd) => nd.name === f));
  const poses = [];
  for (let k = 0; k < n; k++) poses.push(poseAt(json, rest, tracks, k));
  const speeds = [];
  for (const f of feet) {
    let low = Infinity;
    for (const p of poses) low = Math.min(low, p[f].p[1]);
    for (let k = 0; k + 1 < n; k++) {
      const a = poses[k][f].p, b = poses[k + 1][f].p;
      if (a[1] < low + 0.02 && b[1] < low + 0.02) speeds.push(-(b[2] - a[2]) / dt);
    }
  }
  speeds.sort((x, y) => x - y);
  return speeds.length ? speeds[Math.floor(speeds.length / 2)] : 0;
}

function tracksOf(json, accessor, anim) {
  const tracks = new Map();
  for (const ch of anim.channels) {
    const s = anim.samplers[ch.sampler];
    if (s.interpolation && s.interpolation !== 'LINEAR') throw new Error(`${anim.name}: ${s.interpolation} keys are not expected`);
    if (!tracks.has(ch.target.node)) tracks.set(ch.target.node, {});
    tracks.get(ch.target.node)[ch.target.path] = accessor(s.output);
  }
  return tracks;
}

// ── The build ─────────────────────────────────────────────────────────────
function readInput(src, key) {
  const [rel, want] = INPUTS[key];
  const buf = fs.readFileSync(path.join(src, ...rel.split('/')));
  if (want && sha256(buf) !== want) throw new Error(`${rel}: SHA-256 ${sha256(buf)}, expected ${want}; refusing a changed download`);
  return buf;
}

function build(src) {
  const { json, accessor } = readGlb(readInput(src, 'glb'));
  const license = readInput(src, 'license');
  const skin = json.skins[0];
  const jointNodes = skin.joints;
  const rest = json.nodes.map((n) => ({ t: n.translation || [0, 0, 0], r: n.rotation || [0, 0, 0, 1], s: n.scale || [1, 1, 1] }));
  const ibm = accessor(skin.inverseBindMatrices);

  // The root-motion file's own travel per clip, for the record.
  const rm = readGlb(fs.readFileSync(path.join(src, ...INPUTS.rootMotion[0].split('/'))));
  const rmTravel = {};
  for (const a of rm.json.animations) {
    for (const ch of a.channels) {
      if (rm.json.nodes[ch.target.node].name !== 'root' || ch.target.path !== 'translation') continue;
      const s = a.samplers[ch.sampler];
      const v = rm.accessor(s.output);
      const t = rm.accessor(s.input);
      const dz = v[v.length - 1] - v[2];
      rmTravel[a.name] = { metres: dz, seconds: t[t.length - 1] - t[0] };
    }
  }

  // The out file's nodes: the skin's joints, in the skin's order, under their parents.
  const g = new GltfBuilder();
  const nodeOf = new Map(jointNodes.map((n, i) => [n, i]));
  const nodes = jointNodes.map((n) => {
    const src = json.nodes[n];
    const out = { name: src.name };
    if (src.translation) out.translation = src.translation;
    if (src.rotation) out.rotation = src.rotation;
    if (src.scale) out.scale = src.scale;
    const kids = (src.children || []).filter((c) => nodeOf.has(c)).map((c) => nodeOf.get(c));
    if (kids.length) out.children = kids;
    return out;
  });
  const parented = new Set(nodes.flatMap((n) => n.children || []));
  const roots = nodes.map((_, i) => i).filter((i) => !parented.has(i));
  if (roots.length !== 1) throw new Error(`the rig has ${roots.length} roots`);

  const report = [];
  const animations = CLIPS.map((name) => {
    const anim = json.animations.find((a) => a.name === name);
    if (!anim) throw new Error(`the library has no clip ${name}`);
    const samplers = [];
    const channels = [];
    let inputAcc = null;
    let kept = 0, dropped = 0;
    for (const ch of anim.channels) {
      if (!nodeOf.has(ch.target.node)) continue;
      const s = anim.samplers[ch.sampler];
      const values = accessor(s.output);
      const r = rest[ch.target.node];
      const restValue = ch.target.path === 'rotation' ? r.r : ch.target.path === 'scale' ? r.s : r.t;
      const w = restValue.length;
      let moves = false;
      for (let k = 0; k < values.length && !moves; k++) {
        const d = values[k] - restValue[k % w];
        // A rotation and its negation are the same turn.
        const dn = ch.target.path === 'rotation' ? values[k] + restValue[k % w] : d;
        if (Math.abs(d) > 1e-5 && (ch.target.path !== 'rotation' || Math.abs(dn) > 1e-5)) moves = true;
      }
      if (!moves) { dropped++; continue; }
      if (inputAcc === null) inputAcc = g.accessor(accessor(s.input), 'SCALAR', { minMax: true });
      samplers.push({ input: inputAcc, output: g.accessor(values, w === 4 ? 'VEC4' : 'VEC3'), interpolation: 'LINEAR' });
      channels.push({ sampler: samplers.length - 1, target: { node: nodeOf.get(ch.target.node), path: ch.target.path } });
      kept++;
    }
    const times = accessor(anim.samplers[anim.channels[0].sampler].input);
    const entry = { name, seconds: times[times.length - 1] - times[0], kept, dropped };
    if (GAITS.includes(name)) entry.speed = gaitSpeed(json, accessor, rest, anim);
    if (rmTravel[name]) entry.rm = rmTravel[name];
    report.push(entry);
    return { name, samplers, channels };
  });
  // The library's mannequin is not written, so no mesh node: a skin needs none (glTF 2.0
  // 3.7.3: a skin is referenced by nodes that have a mesh; one referenced by none is allowed).
  const ibmAcc = g.accessor(ibm, 'MAT4');
  const gltf = {
    asset: {
      version: '2.0',
      generator: 'HumanityOS scripts/gen-quaternius-clips.js',
      copyright: 'Animations by Quaternius, CC0 1.0 Universal (Universal Animation Library [Standard]); trimmed by HumanityOS',
    },
    scene: 0,
    scenes: [{ name: NAME, nodes: roots }],
    nodes,
    skins: [{ name: `${NAME}_skin`, joints: nodes.map((_, i) => i), inverseBindMatrices: ibmAcc, skeleton: roots[0] }],
    animations,
  };
  return { made: g.finish(gltf, `${NAME}.bin`), license, report };
}

function sourceNote(files, report) {
  const lines = [
    '# Quaternius Universal Animation Library clips: where they come from',
    '',
    'Made by `scripts/gen-quaternius-clips.js` (HumanityOS, character animation rung 1, 2026-11-09):',
    'the clips the game plays, trimmed out of Quaternius\'s file with their keys unchanged. Do not edit',
    'these files by hand: change the converter and run it again.',
    '',
    '## Licence',
    '',
    '**CC0 1.0 Universal** (https://creativecommons.org/publicdomain/zero/1.0/), by the `License.txt`',
    'inside the download, copied beside this file unchanged. Quaternius\'s licence page showed a',
    'different licence (QAL v1.0, "Last updated: 8/28/2026") when it was read on 2026-11-08; the',
    'instrument that travels with these exact bytes is CC0. Finding:',
    '`docs/reference/findings/2026-11-08-character-assets.md`, its 2026-11-09 addendum.',
    '',
    '## Source',
    '',
    `- Page: ${SOURCE.page} (${SOURCE.version})`,
    `- File: \`${SOURCE.zip}\`, ${SOURCE.zipBytes} bytes, SHA-256 \`${SOURCE.zipSha256}\``,
    `- Downloaded: ${SOURCE.downloaded}, approved by ${SOURCE.approved}`,
    `- Read from it: \`${INPUTS.glb[0].replace(/^x\//, '')}\` (SHA-256 \`${INPUTS.glb[1]}\`, root motion off) and, for the`,
    '  travel column below only, `UAL1_Standard_RM.glb`.',
    '',
    '## The clips, and each gait\'s ground speed',
    '',
    'Speed: measured from the feet (the converter\'s header says how), the number',
    '`data/characters/clips.ron` uses. Travel: the root-motion file\'s own root movement over the clip.',
    '',
    '| Clip | Seconds | Channels kept (left at rest) | Speed from the feet, m/s | Root-motion travel, m/s |',
    '|---|---:|---:|---:|---:|',
    ...report.map((r) => `| ${r.name} | ${r.seconds.toFixed(3)} | ${r.kept} (${r.dropped}) | ${r.speed != null ? r.speed.toFixed(3) : ''} | ${r.rm && r.rm.metres ? (r.rm.metres / r.rm.seconds).toFixed(3) : ''} |`),
    '',
    '## Output hashes (SHA-256)',
    '',
    '`node scripts/gen-quaternius-clips.js --check` compares the files with these when the download is',
    'not on the machine.',
    '',
    ...files.map(([p, bytes]) => `- \`${path.basename(p)}\` \`${sha256(bytes)}\``),
    '',
  ];
  return lines.join('\n');
}

function argValue(flag) {
  const i = process.argv.indexOf(flag);
  return i >= 0 ? process.argv[i + 1] : null;
}

function main() {
  const check = process.argv.includes('--check');
  const src = argValue('--src') || DEFAULT_SRC;
  const note = path.join(OUT, 'SOURCE.md');
  if (!fs.existsSync(path.join(src, ...INPUTS.glb[0].split('/')))) {
    if (!check) {
      console.error(`the Quaternius download is not at ${src} (--src DIR or HUMANITY_QUATERNIUS_SRC)`);
      process.exit(1);
    }
    const text = fs.existsSync(note) ? fs.readFileSync(note, 'utf8').replace(/\r\n/g, '\n') : '';
    const want = [...text.matchAll(/^- `([^`]+)` `([0-9a-f]{64})`$/gm)];
    if (want.length === 0) { console.error(`${path.relative(ROOT, note)} records no output hashes`); process.exit(1); }
    let bad = 0;
    for (const [, file, hash] of want) {
      const p = path.join(OUT, file);
      let bytes = fs.existsSync(p) ? fs.readFileSync(p) : null;
      if (bytes && file.endsWith('.gltf')) bytes = Buffer.from(bytes.toString().replace(/\r\n/g, '\n'));
      const got = bytes ? sha256(bytes) : 'missing';
      if (got !== hash) { console.error(`${file}: ${got}, SOURCE.md records ${hash}`); bad++; }
    }
    if (bad) process.exit(1);
    console.log(`the Quaternius download is not on this machine (${src}): checked the ${want.length} committed files against SOURCE.md's hashes only`);
    return;
  }
  const { made, license, report } = build(src);
  const files = [
    [path.join(OUT, `${NAME}.gltf`), Buffer.from(made.json)],
    [path.join(OUT, `${NAME}.bin`), made.bin],
    [path.join(OUT, 'License.txt'), license],
  ];
  files.push([note, Buffer.from(sourceNote(files, report))]);
  if (check) {
    // License.txt is Quaternius's own bytes (CRLF): compared byte for byte like the .bin.
    const bad = differing(files.filter(([p]) => !p.endsWith('License.txt')), fs, path, ROOT);
    const lic = files.find(([p]) => p.endsWith('License.txt'));
    if (!fs.existsSync(lic[0]) || !fs.readFileSync(lic[0]).equals(lic[1])) bad.push(path.relative(ROOT, lic[0]));
    for (const p of bad) console.error(`${p} differs from what scripts/gen-quaternius-clips.js makes`);
    if (bad.length) process.exit(1);
    console.log('the Quaternius clips match the converter');
    return;
  }
  fs.mkdirSync(OUT, { recursive: true });
  for (const [p, bytes] of files) {
    fs.writeFileSync(p, bytes);
    console.log(`wrote ${path.relative(ROOT, p)} (${bytes.length} bytes)`);
  }
  for (const r of report) console.log(`${r.name}: ${r.seconds.toFixed(3)} s, ${r.kept} channels${r.speed != null ? `, ${r.speed.toFixed(3)} m/s from the feet` : ''}${r.rm && r.rm.metres ? `, root motion ${(r.rm.metres / r.rm.seconds).toFixed(3)} m/s` : ''}`);
}

if (require.main === module) main();
module.exports = { build, CLIPS };
