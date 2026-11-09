#!/usr/bin/env node
// The procedural test figure and its clip library (character animation rung 1, 2026-11-08;
// docs/design/character-animation.md). Two skinned glTF files, both our own work, made here so that
// skinning, retargeting and the state machine are proven before any outside asset is downloaded (the
// real body and clips need the operator's yes, file by file):
//
//   assets/models/characters/hos_test_figure/  the BODY: a jointed figure in an A-pose (arms 40
//       degrees down), bones named the way Blender's rigs name them (pelvis, thigh.L, forearm.L) with
//       Blender's convention of each bone's +Y along the bone, forearm twist joints, and six
//       materials the body's data file tints (skin, hair, shirt, trousers, shoes, hands). It also
//       carries `ref_walk`: the walk authored directly on THIS rig, which the engine's retarget test
//       compares the clip library's walk against after retargeting.
//   assets/models/characters/hos_test_clips/   the CLIP LIBRARY: a different rig playing the same
//       roles, the way a Quaternius or Mixamo library differs from a MakeHuman body: a T-pose,
//       Mixamo-style names (mixamorig:LeftForeArm), 5% taller, every bone's local axes rolled its own
//       way, and an "Armature" node scaled 0.01 over centimetre joints (Mixamo's export). Its clips:
//       idle, walk, run, turn_left, turn_right.
//
// THE MOTION is written once, as each role's turn in a canonical frame (a T-pose facing +Z, +Y up,
// +X the figure's left, every rest rotation the identity) chained down the limbs like forward
// kinematics, plus the hips' travel. Each rig's keys are that motion written in its own frames: a
// joint's animated model rotation is `D * C * W_rest`, where D is the role's canonical turn and C
// turns the rig's rest bone onto the canonical T-pose bone (the identity for the T-pose library; the
// A-pose correction for the body's limbs). That is exactly the retargeter's formula
// (crates/humanity-animation/src/retarget.rs), so the retargeted library walk must land on `ref_walk`.
//
//   node scripts/gen-test-figure.js          write both files
//   node scripts/gen-test-figure.js --check  exit 1 if the committed files differ from what it makes
//
// Deterministic: no randomness, fixed float formatting.

'use strict';
const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const OUT = {
  body: path.join(ROOT, 'assets/models/characters/hos_test_figure'),
  clips: path.join(ROOT, 'assets/models/characters/hos_test_clips'),
};

// ── Small vector and quaternion maths ([x, y, z], [x, y, z, w]) ─────────────
const add = (a, b) => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const mul = (a, s) => [a[0] * s, a[1] * s, a[2] * s];
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const len = (a) => Math.sqrt(dot(a, a));
const norm = (a) => { const l = len(a); return l > 1e-12 ? mul(a, 1 / l) : [0, 0, 0]; };
const deg = (d) => (d * Math.PI) / 180;

const QI = [0, 0, 0, 1];
function qAxis(axis, angle) {
  const a = norm(axis);
  const s = Math.sin(angle / 2);
  return [a[0] * s, a[1] * s, a[2] * s, Math.cos(angle / 2)];
}
const qx = (a) => qAxis([1, 0, 0], a);
const qy = (a) => qAxis([0, 1, 0], a);
const qz = (a) => qAxis([0, 0, 1], a);
function qmul(a, b) {
  return [
    a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
    a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
    a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
    a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
  ];
}
const qinv = (q) => [-q[0], -q[1], -q[2], q[3]];
function qnorm(q) {
  const l = Math.hypot(q[0], q[1], q[2], q[3]);
  return [q[0] / l, q[1] / l, q[2] / l, q[3] / l];
}
function qrot(q, v) {
  const p = qmul(qmul(q, [v[0], v[1], v[2], 0]), qinv(q));
  return [p[0], p[1], p[2]];
}
// The shortest turn taking unit vector a onto unit vector b (glam's Quat::from_rotation_arc).
function qarc(a, b) {
  const d = dot(a, b);
  if (d > 0.999999) return QI.slice();
  if (d < -0.999999) {
    let axis = cross([1, 0, 0], a);
    if (len(axis) < 1e-6) axis = cross([0, 1, 0], a);
    return qAxis(axis, Math.PI);
  }
  const c = cross(a, b);
  return qnorm([c[0], c[1], c[2], 1 + d]);
}
// Positive w, so identical rotations write identical keys.
const qcanon = (q) => (q[3] < 0 ? q.map((x) => -x) : q);

// Column-major 4x4 matrices.
function mTRS(t, r, s) {
  const [x, y, z, w] = r;
  const m = [
    1 - 2 * (y * y + z * z), 2 * (x * y + z * w), 2 * (x * z - y * w), 0,
    2 * (x * y - z * w), 1 - 2 * (x * x + z * z), 2 * (y * z + x * w), 0,
    2 * (x * z + y * w), 2 * (y * z - x * w), 1 - 2 * (x * x + y * y), 0,
    t[0], t[1], t[2], 1,
  ];
  for (let c = 0; c < 3; c++) for (let r2 = 0; r2 < 3; r2++) m[c * 4 + r2] *= s[c];
  return m;
}
function mMul(a, b) {
  const o = new Array(16).fill(0);
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) {
    let v = 0;
    for (let k = 0; k < 4; k++) v += a[k * 4 + r] * b[c * 4 + k];
    o[c * 4 + r] = v;
  }
  return o;
}
// Inverse of T * R * S (uniform s): S^-1 R^-1 T^-1.
function mInvTRS(t, r, s) {
  const ri = qinv(r);
  const ti = mul(qrot(ri, t), -1 / s);
  return mTRS(ti, ri, [1 / s, 1 / s, 1 / s]);
}

// ── The two rigs ─────────────────────────────────────────────────────────────
// Each joint: [name, role, parent name, model-space rest position (metres)]. The body is an
// A-pose; the library a T-pose 5% taller. Positions are the figure's, before any armature scale.
const ARM_DROP = deg(40);
function bodyJoints() {
  const J = [];
  const add2 = (name, role, parent, pos) => J.push({ name, role, parent, pos });
  add2('pelvis', 'hips', null, [0, 0.98, 0]);
  add2('spine_01', 'spine', 'pelvis', [0, 1.08, -0.005]);
  add2('spine_02', 'chest', 'spine_01', [0, 1.22, -0.01]);
  add2('spine_03', 'upperChest', 'spine_02', [0, 1.36, -0.01]);
  add2('neck', 'neck', 'spine_03', [0, 1.52, 0.0]);
  add2('head', 'head', 'neck', [0, 1.6, 0.01]);
  for (const [sfx, side, sx] of [['L', 'left', 1], ['R', 'right', -1]]) {
    const dir = [sx * Math.cos(ARM_DROP), -Math.sin(ARM_DROP), 0];
    const sh = [sx * 0.18, 1.47, -0.01];
    const el = add(sh, mul(dir, 0.28));
    const wr = add(el, mul(dir, 0.25));
    add2(`clavicle.${sfx}`, `${side}Shoulder`, 'spine_03', [sx * 0.03, 1.47, -0.0]);
    add2(`upper_arm.${sfx}`, `${side}UpperArm`, `clavicle.${sfx}`, sh);
    add2(`forearm.${sfx}`, `${side}LowerArm`, `upper_arm.${sfx}`, el);
    add2(`forearm_twist.${sfx}`, `${side}LowerArmTwist`, `forearm.${sfx}`, add(el, mul(dir, 0.125)));
    add2(`hand.${sfx}`, `${side}Hand`, `forearm.${sfx}`, wr);
    add2(`thigh.${sfx}`, `${side}UpperLeg`, 'pelvis', [sx * 0.095, 0.93, 0]);
    add2(`shin.${sfx}`, `${side}LowerLeg`, `thigh.${sfx}`, [sx * 0.095, 0.52, 0.01]);
    add2(`foot.${sfx}`, `${side}Foot`, `shin.${sfx}`, [sx * 0.095, 0.09, -0.01]);
    add2(`toe.${sfx}`, `${side}Toes`, `foot.${sfx}`, [sx * 0.095, 0.02, 0.13]);
  }
  return J;
}
const CLIP_SCALE = 1.05;
function clipJoints() {
  const J = [];
  const add2 = (name, role, parent, pos) => J.push({ name, role, parent, pos: mul(pos, CLIP_SCALE) });
  add2('mixamorig:Hips', 'hips', null, [0, 0.98, 0]);
  add2('mixamorig:Spine', 'spine', 'mixamorig:Hips', [0, 1.09, 0]);
  add2('mixamorig:Spine1', 'chest', 'mixamorig:Spine', [0, 1.23, 0]);
  add2('mixamorig:Spine2', 'upperChest', 'mixamorig:Spine1', [0, 1.37, 0]);
  add2('mixamorig:Neck', 'neck', 'mixamorig:Spine2', [0, 1.52, 0]);
  add2('mixamorig:Head', 'head', 'mixamorig:Neck', [0, 1.6, 0.01]);
  for (const [nm, side, sx] of [['Left', 'left', 1], ['Right', 'right', -1]]) {
    add2(`mixamorig:${nm}Shoulder`, `${side}Shoulder`, 'mixamorig:Spine2', [sx * 0.03, 1.47, 0]);
    add2(`mixamorig:${nm}Arm`, `${side}UpperArm`, `mixamorig:${nm}Shoulder`, [sx * 0.18, 1.47, 0]);
    add2(`mixamorig:${nm}ForeArm`, `${side}LowerArm`, `mixamorig:${nm}Arm`, [sx * 0.46, 1.47, 0]);
    add2(`mixamorig:${nm}Hand`, `${side}Hand`, `mixamorig:${nm}ForeArm`, [sx * 0.71, 1.47, 0]);
    add2(`mixamorig:${nm}UpLeg`, `${side}UpperLeg`, 'mixamorig:Hips', [sx * 0.095, 0.93, 0]);
    add2(`mixamorig:${nm}Leg`, `${side}LowerLeg`, `mixamorig:${nm}UpLeg`, [sx * 0.095, 0.52, 0]);
    add2(`mixamorig:${nm}Foot`, `${side}Foot`, `mixamorig:${nm}Leg`, [sx * 0.095, 0.09, 0]);
    add2(`mixamorig:${nm}ToeBase`, `${side}Toes`, `mixamorig:${nm}Foot`, [sx * 0.095, 0.02, 0.13]);
  }
  return J;
}

// The canonical T-pose: every role's rest position in the canonical frame (the library's, unscaled),
// used for the bone directions C aligns to.
const AIM = {
  leftShoulder: 'leftUpperArm', leftUpperArm: 'leftLowerArm', leftLowerArm: 'leftHand',
  rightShoulder: 'rightUpperArm', rightUpperArm: 'rightLowerArm', rightLowerArm: 'rightHand',
  leftUpperLeg: 'leftLowerLeg', leftLowerLeg: 'leftFoot', leftFoot: 'leftToes',
  rightUpperLeg: 'rightLowerLeg', rightLowerLeg: 'rightFoot', rightFoot: 'rightToes',
};

// World rest rotations. The body: Blender's convention, each bone's +Y along the bone (toward its
// first child), +Z turned to face forward where it can. The library: an arbitrary roll per joint.
function restRotationsBody(J) {
  return J.map((j) => {
    const child = J.find((c) => c.parent === j.name && !c.name.includes('twist'));
    // A bone with no child carries on its parent's way (the hand along the forearm, the toe along
    // the foot); the head points up.
    const parent = J.find((c) => c.name === j.parent);
    const onward = j.role === 'head' || !parent ? [0, 0.2, 0] : sub(j.pos, parent.pos);
    const target = child ? child.pos : add(j.pos, onward);
    const along = norm(sub(target, j.pos));
    const q1 = qarc([0, 1, 0], along);
    return qcanon(qnorm(q1));
  });
}
function restRotationsClip(J) {
  return J.map((_, i) => qcanon(qAxis([1, 2 + i, 0.5], 0.4 + 0.37 * i)));
}

// ── The canonical motion ────────────────────────────────────────────────────
// Each role's LOCAL turn in the canonical frame (identity rests), chained down the hierarchy to give
// its model-space turn D. Angles in degrees; `p` is the phase 0..1 through one cycle.
const TAU = Math.PI * 2;
const ROLE_PARENT = {
  spine: 'hips', chest: 'spine', upperChest: 'chest', neck: 'upperChest', head: 'neck',
  leftShoulder: 'upperChest', leftUpperArm: 'leftShoulder', leftLowerArm: 'leftUpperArm', leftHand: 'leftLowerArm',
  rightShoulder: 'upperChest', rightUpperArm: 'rightShoulder', rightLowerArm: 'rightUpperArm', rightHand: 'rightLowerArm',
  leftUpperLeg: 'hips', leftLowerLeg: 'leftUpperLeg', leftFoot: 'leftLowerLeg', leftToes: 'leftFoot',
  rightUpperLeg: 'hips', rightLowerLeg: 'rightUpperLeg', rightFoot: 'rightLowerLeg', rightToes: 'rightFoot',
};
const ROLES = ['hips', ...Object.keys(ROLE_PARENT)];

// Arms hang from the T-pose: the upper arm turned down about Z, then swung about X; the elbow bends
// the forearm forward about the upper arm's local Y.
function armLocals(L, side, hang, swing, elbow, wrist) {
  const sx = side === 'left' ? 1 : -1;
  L[`${side}UpperArm`] = qmul(qx(deg(-swing)), qz(deg(-sx * hang)));
  L[`${side}LowerArm`] = qy(deg(-sx * elbow));
  L[`${side}Hand`] = qz(deg(-sx * wrist));
}
// A leg: the thigh swings forward about -X, the knee bends the shin back about +X, the ankle flexes,
// and the toes bend up (the ball of the foot rolling off the floor).
function legLocals(L, side, swing, knee, ankle, toes = 0) {
  L[`${side}UpperLeg`] = qx(deg(-swing));
  L[`${side}LowerLeg`] = qx(deg(knee));
  L[`${side}Foot`] = qx(deg(-ankle));
  L[`${side}Toes`] = qx(deg(-toes));
}

const gaits = {
  walk: { period: 1.05, swingF: 25, swingB: 22, knee: 58, stanceKnee: 8, arm: 18, elbow: 18, elbowSwing: 10, bob: 0.025, sway: 0.018, pelvisYaw: 6, lean: 3 },
  run: { period: 0.68, swingF: 42, swingB: 26, knee: 105, stanceKnee: 22, arm: 34, elbow: 78, elbowSwing: 14, bob: 0.045, sway: 0.012, pelvisYaw: 9, lean: 11 },
};
function gaitLocals(g, p) {
  const L = {};
  const s = Math.sin(TAU * p);
  const c = Math.cos(TAU * p);
  // The left leg forward at p = 0.25, back at 0.75; the right half a cycle behind.
  const thigh = (ph) => { const v = Math.sin(TAU * ph); return v > 0 ? v * g.swingF : v * g.swingB; };
  // Knee flexion peaks just before mid-swing and is already rising at toe-off (three quarters), as
  // a real gait's does, so the trailing foot rolls off its toes instead of dipping them under the
  // floor; a small flex through stance.
  const knee = (ph) => g.stanceKnee * Math.max(0, Math.sin(TAU * (ph - 0.25))) * 0.6 + g.knee * Math.pow(Math.max(0, Math.cos(TAU * (ph - 0.95))), 1.4) + 4;
  // The ankle: toe up at heel strike (a quarter), flat at mid-stance, a little toe down at toe-off;
  // the toes bend up through push-off, so the foot rolls over its ball as the heel lifts.
  const ankle = (ph) => { const v = Math.sin(TAU * ph); return v > 0 ? 12 * v : 5 * v; };
  const toes = (ph) => { const q = (((ph % 1) + 1) % 1 - 0.52) / 0.33; return q > 0 && q < 1 ? 40 * Math.sin(Math.PI * q) : 0; };
  legLocals(L, 'left', thigh(p), knee(p), ankle(p), toes(p));
  legLocals(L, 'right', thigh(p + 0.5), knee(p + 0.5), ankle(p + 0.5), toes(p + 0.5));
  // Arms swing against the legs.
  armLocals(L, 'left', 76, -g.arm * s, g.elbow + g.elbowSwing * Math.max(0, -s), 4);
  armLocals(L, 'right', 76, g.arm * s, g.elbow + g.elbowSwing * Math.max(0, s), 4);
  // The pelvis turns with the forward leg, the chest against it; the head stays on the path.
  L.hips = qmul(qy(deg(g.pelvisYaw * s)), qz(deg(2.5 * c)));
  L.spine = qmul(qx(deg(g.lean)), qy(deg(-g.pelvisYaw * 0.6 * s)));
  L.chest = qy(deg(-g.pelvisYaw * 0.5 * s));
  L.upperChest = qx(deg(g.lean * 0.3));
  L.neck = qy(deg(g.pelvisYaw * 0.3 * s));
  L.head = qx(deg(-g.lean * 1.1));
  // The hips ride highest at mid-stance, over a straight leg (p = 0 and a half), and lowest at the
  // double support between (a quarter and three quarters), when both legs slant: so the feet stay
  // near the floor without IK, which rung 2 adds.
  const travel = [g.sway * s, g.bob * Math.cos(TAU * 2 * p) - g.bob, 0];
  return { L, travel };
}
function idleLocals(p) {
  const L = {};
  const b = Math.sin(TAU * p);
  legLocals(L, 'left', 1.5, 4 + 1.5 * b, 1.5);
  legLocals(L, 'right', -1.0, 5 - 1.5 * b, 0.5);
  armLocals(L, 'left', 78, 2 + 1.0 * b, 12, 6);
  armLocals(L, 'right', 78, 1 - 1.0 * b, 14, 6);
  L.hips = qz(deg(1.2 * b));
  L.spine = qx(deg(1.0 + 0.6 * Math.sin(TAU * 2 * p)));
  L.chest = qx(deg(1.5 * Math.sin(TAU * 2 * p)));
  L.upperChest = QI.slice();
  L.neck = qy(deg(3 * Math.sin(TAU * p + 1)));
  L.head = qx(deg(-1.5 + 1.2 * Math.sin(TAU * 2 * p + 0.5)));
  return { L, travel: [0.012 * b, 0, 0] };
}
// Turning on the spot: short steps, the leading foot lifting first, the pelvis leading the turn.
function turnLocals(p, dir) {
  const L = {};
  const lead = dir > 0 ? 'left' : 'right';
  const trail = dir > 0 ? 'right' : 'left';
  const lift = (ph) => Math.max(0, Math.sin(TAU * ph));
  legLocals(L, lead, 14 * lift(p), 6 + 34 * lift(p), 6 * lift(p));
  legLocals(L, trail, 14 * lift(p + 0.5), 6 + 34 * lift(p + 0.5), 6 * lift(p + 0.5));
  armLocals(L, 'left', 76, 6 * Math.sin(TAU * p), 16, 5);
  armLocals(L, 'right', 76, -6 * Math.sin(TAU * p), 16, 5);
  L.hips = qy(deg(dir * (6 + 3 * Math.sin(TAU * p))));
  L.spine = qy(deg(dir * 3));
  L.chest = qy(deg(dir * 2));
  L.upperChest = QI.slice();
  L.neck = qy(deg(dir * 4));
  L.head = qy(deg(dir * 4));
  return { L, travel: [0.01 * dir * Math.sin(TAU * p), -0.004 * Math.abs(Math.sin(TAU * p * 2)), 0] };
}

// Locals chained into model-space turns D, by role.
function turnsFrom(L) {
  const D = {};
  for (const r of ROLES) {
    const local = L[r] || QI;
    D[r] = ROLE_PARENT[r] ? qmul(D[ROLE_PARENT[r]], local) : local;
  }
  return D;
}

const CLIPS = [
  { name: 'idle', period: 4.0, keys: 48, at: (p) => idleLocals(p) },
  { name: 'walk', period: gaits.walk.period, keys: 24, at: (p) => gaitLocals(gaits.walk, p) },
  { name: 'run', period: gaits.run.period, keys: 20, at: (p) => gaitLocals(gaits.run, p) },
  { name: 'turn_left', period: 0.9, keys: 24, at: (p) => turnLocals(p, 1) },
  { name: 'turn_right', period: 0.9, keys: 24, at: (p) => turnLocals(p, -1) },
];

// ── A rig's keys for one clip ───────────────────────────────────────────────
// For each joint with a role: model rotation D * C * W_rest, written local to its parent's animated
// model rotation (the armature's for a root). The hips also travel (scaled to the rig's hips height,
// the canonical motion being the library's).
function rigKeys(rig, clip) {
  const times = [];
  const rot = rig.joints.map(() => []);
  const trans = [];
  const canonHips = clipJoints()[0].pos[1];
  const travelScale = rig.joints[0].pos[1] / canonHips;
  for (let k = 0; k <= clip.keys; k++) {
    const p = k / clip.keys;
    times.push(p * clip.period);
    const { L, travel } = clip.at(p % 1);
    const D = turnsFrom(L);
    const world = [];
    rig.joints.forEach((j, i) => {
      const d = D[j.role] || null;
      const parentWorld = j.parent ? world[rig.index[j.parent]] : rig.outerRot;
      let w;
      if (d) w = qnorm(qmul(qmul(d, rig.align[i]), rig.restWorld[i]));
      else w = qnorm(qmul(parentWorld, rig.restLocal[i].r));
      world.push(w);
      rot[i].push(qcanon(qnorm(qmul(qinv(parentWorld), w))));
    });
    // Hips travel: model-space position, written in the armature's units.
    const hipsWorld = add(rig.joints[0].pos, mul(travel, travelScale));
    trans.push(mul(qrot(qinv(rig.outerRot), hipsWorld), 1 / rig.outerScale));
  }
  return { times, rot, trans };
}

// ── Building a rig: local rests, inverse binds, alignment ───────────────────
function buildRig(joints, restWorld, outerScale, canon) {
  const index = {};
  joints.forEach((j, i) => { index[j.name] = i; });
  const outerRot = QI.slice();
  const restLocal = joints.map((j, i) => {
    if (!j.parent) {
      return { t: mul(j.pos, 1 / outerScale), r: restWorld[i], s: [1, 1, 1] };
    }
    const p = index[j.parent];
    const pr = restWorld[p];
    const t = mul(qrot(qinv(pr), sub(j.pos, joints[p].pos)), 1 / outerScale);
    return { t, r: qcanon(qnorm(qmul(qinv(pr), restWorld[i]))), s: [1, 1, 1] };
  });
  // Model matrix = armature scale * chain; its inverse is the inverse bind.
  const ibm = joints.map((j, i) => mInvTRS(j.pos, restWorld[i], outerScale));
  // C: the turn from this rig's rest bone onto the canonical T-pose bone, for aligned roles.
  const align = joints.map((j) => {
    const aim = AIM[j.role];
    if (!aim) return QI.slice();
    const ci = canon.findIndex((c) => c.role === j.role);
    const ca = canon.findIndex((c) => c.role === aim);
    const a = joints.findIndex((c) => c.role === aim);
    if (ci < 0 || ca < 0 || a < 0) return QI.slice();
    return qarc(norm(sub(joints[a].pos, j.pos)), norm(sub(canon[ca].pos, canon[ci].pos)));
  });
  return { joints, index, restWorld, restLocal, ibm, align, outerScale, outerRot };
}

// ── The meshes ──────────────────────────────────────────────────────────────
// Vertices carry four joint indices and weights; each primitive is one material.
function newPrim(material) {
  return { material, pos: [], nrm: [], uv: [], jnt: [], wgt: [], idx: [] };
}
function pushVert(pr, p, n, uv, joints) {
  // joints: [[index, weight], ...] up to four, normalised.
  const js = joints.filter((x) => x[1] > 1e-4).sort((a, b) => b[1] - a[1]).slice(0, 4);
  const sum = js.reduce((s, x) => s + x[1], 0);
  const j4 = [0, 0, 0, 0];
  const w4 = [0, 0, 0, 0];
  js.forEach((x, i) => { j4[i] = x[0]; w4[i] = x[1] / sum; });
  pr.pos.push(p); pr.nrm.push(norm(n)); pr.uv.push(uv); pr.jnt.push(j4); pr.wgt.push(w4);
  return pr.pos.length - 1;
}
// A tapered tube from a to b: rings of `sides` vertices, radii (across, deep) at each end, weights
// by a function of the fraction along it. Capped at either end on request. Wound so the outside
// faces out (counter-clockwise seen from outside).
function tube(pr, a, b, ra, rb, sides, rings, weightAt, opts = {}) {
  const axis = norm(sub(b, a));
  const ref = Math.abs(axis[2]) < 0.9 ? [0, 0, 1] : [1, 0, 0];
  // u: across the figure's front where possible; w completes the frame.
  const w = norm(cross(axis, ref));
  const u = norm(cross(w, axis));
  const base = pr.pos.length;
  for (let r = 0; r <= rings; r++) {
    const f = r / rings;
    const c = add(a, mul(sub(b, a), f));
    const rx = ra[0] + (rb[0] - ra[0]) * f;
    const rz = ra[1] + (rb[1] - ra[1]) * f;
    for (let s = 0; s < sides; s++) {
      const th = (s / sides) * TAU;
      const dirv = add(mul(u, Math.cos(th) * rz), mul(w, Math.sin(th) * rx));
      const n = add(mul(u, Math.cos(th) / Math.max(rz, 1e-4)), mul(w, Math.sin(th) / Math.max(rx, 1e-4)));
      pushVert(pr, add(c, dirv), n, [s / sides, f], weightAt(f));
    }
  }
  for (let r = 0; r < rings; r++) {
    for (let s = 0; s < sides; s++) {
      const i0 = base + r * sides + s;
      const i1 = base + r * sides + ((s + 1) % sides);
      const i2 = i0 + sides;
      const i3 = i1 + sides;
      pr.idx.push(i0, i1, i2, i1, i3, i2);
    }
  }
  const cap = (ring, centre, normal, flip) => {
    const ci = pushVert(pr, centre, normal, [0.5, ring ? 1 : 0], weightAt(ring ? 1 : 0));
    for (let s = 0; s < sides; s++) {
      const i0 = base + ring * rings * sides + s;
      const i1 = base + ring * rings * sides + ((s + 1) % sides);
      if (flip) pr.idx.push(ci, i1, i0); else pr.idx.push(ci, i0, i1);
    }
  };
  if (opts.capA) cap(0, a, mul(axis, -1), true);
  if (opts.capB) cap(1, b, axis, false);
}
// An ellipsoid (or the upper part of one, for the hair cap) on one joint.
function ellipsoid(pr, centre, radii, joint, opts = {}) {
  const stacks = opts.stacks || 12;
  const slices = opts.slices || 16;
  const top = opts.from || 0; // 0 = full; the cap starts lower
  const base = pr.pos.length;
  const stackFrom = Math.round(top * stacks);
  for (let i = 0; i <= stacks; i++) {
    const phi = (i / stacks) * Math.PI; // 0 at the top
    for (let j = 0; j <= slices; j++) {
      const th = (j / slices) * TAU;
      const n = [Math.sin(phi) * Math.sin(th), Math.cos(phi), Math.sin(phi) * Math.cos(th)];
      const p = add(centre, [n[0] * radii[0], n[1] * radii[1], n[2] * radii[2]]);
      pushVert(pr, p, [n[0] / radii[0], n[1] / radii[1], n[2] / radii[2]], [j / slices, i / stacks], [[joint, 1]]);
    }
  }
  const row = slices + 1;
  const stackTo = opts.to ? Math.round(opts.to * stacks) : stacks;
  for (let i = stackFrom === 0 ? 0 : 0; i < stackTo; i++) {
    for (let j = 0; j < slices; j++) {
      const a = base + i * row + j;
      const b = a + row;
      // The hair leaves the face open: skip the front-lower band.
      if (opts.skip && opts.skip(i / stacks, j / slices)) continue;
      pr.idx.push(a, b, a + 1, a + 1, b, b + 1);
    }
  }
}
// A box between two corners in a frame (origin, x, y, z axes), on given joint weights.
function box(pr, origin, ax, ay, az, size, weights) {
  const [sx, sy, sz] = size;
  const faces = [
    [ax, ay, az, sx, sy, sz], [mul(ax, -1), ay, mul(az, -1), sx, sy, sz],
    [az, ay, mul(ax, -1), sz, sy, sx], [mul(az, -1), ay, ax, sz, sy, sx],
    [ax, mul(az, -1), ay, sx, sz, sy], [ax, az, mul(ay, -1), sx, sz, sy],
  ];
  for (const [r, u, n, w, h, d] of faces) {
    const c = add(origin, mul(n, d / 2));
    const base = pr.pos.length;
    for (const [a, b] of [[-1, -1], [1, -1], [1, 1], [-1, 1]]) {
      const p = add(c, add(mul(r, (a * w) / 2), mul(u, (b * h) / 2)));
      pushVert(pr, p, n, [(a + 1) / 2, (b + 1) / 2], weights(p));
    }
    pr.idx.push(base, base + 1, base + 2, base, base + 2, base + 3);
  }
}

function bodyMesh(rig) {
  const J = rig.joints;
  const I = (n) => rig.index[n];
  const P = (n) => J[I(n)].pos;
  const prims = {};
  for (const m of ['skin', 'hair', 'shirt', 'trousers', 'shoes', 'hands']) prims[m] = newPrim(m);
  const blend = (from, a, b) => (f) => {
    // From joint `from`; shared with the parent `a` near the start and the child `b` near the end.
    const out = [];
    let wf = 1;
    if (a != null && f < 0.25) { const k = 0.5 * (1 - f / 0.25); out.push([a, k]); wf -= k; }
    if (b != null && f > 0.75) { const k = 0.5 * ((f - 0.75) / 0.25); out.push([b, k]); wf -= k; }
    out.push([from, wf]);
    return out;
  };
  // The torso: rings up the spine, each weighted between the spine joints around its height.
  const spine = ['pelvis', 'spine_01', 'spine_02', 'spine_03', 'neck'].map(I);
  const heights = spine.map((j) => J[j].pos[1]);
  const torsoWeights = (y) => {
    for (let k = 0; k < heights.length - 1; k++) {
      if (y <= heights[k + 1] || k === heights.length - 2) {
        const f = Math.min(1, Math.max(0, (y - heights[k]) / (heights[k + 1] - heights[k])));
        return [[spine[k], 1 - f], [spine[k + 1], f]];
      }
    }
    return [[spine[0], 1]];
  };
  // Trousers: the pelvis block, hips to just above the belt.
  const pelvisRings = [[0.84, 0.15, 0.105], [0.9, 0.165, 0.11], [0.98, 0.165, 0.11], [1.05, 0.155, 0.105]];
  const shirtRings = [[1.05, 0.155, 0.105], [1.16, 0.15, 0.1], [1.28, 0.165, 0.11], [1.4, 0.19, 0.115], [1.47, 0.2, 0.11], [1.52, 0.1, 0.07]];
  const ringsTube = (pr, rings, weightsAt) => {
    const sides = 16;
    const base = pr.pos.length;
    rings.forEach(([y, rx, rz]) => {
      for (let s = 0; s < sides; s++) {
        const th = (s / sides) * TAU;
        const p = [Math.sin(th) * rx, y, Math.cos(th) * rz - 0.005];
        pushVert(pr, p, [Math.sin(th) / rx, 0, Math.cos(th) / rz], [s / sides, y], weightsAt(y));
      }
    });
    for (let r = 0; r < rings.length - 1; r++) {
      for (let s = 0; s < sides; s++) {
        const i0 = base + r * sides + s;
        const i1 = base + r * sides + ((s + 1) % sides);
        pr.idx.push(i0, i1, i0 + sides, i1, i1 + sides, i0 + sides);
      }
    }
    return { base, sides, count: rings.length };
  };
  const pel = ringsTube(prims.trousers, pelvisRings, (y) => (y < 0.98 ? [[I('pelvis'), 1]] : torsoWeights(y)));
  // Close the crotch with a fan.
  {
    const pr = prims.trousers;
    const c = pushVert(pr, [0, 0.83, 0], [0, -1, 0], [0.5, 0], [[I('pelvis'), 1]]);
    for (let s = 0; s < pel.sides; s++) pr.idx.push(c, pel.base + ((s + 1) % pel.sides), pel.base + s);
  }
  ringsTube(prims.shirt, shirtRings, torsoWeights);
  // The legs: thigh and shin, trousers; the knee shared.
  for (const sfx of ['L', 'R']) {
    tube(prims.trousers, add(P(`thigh.${sfx}`), [0, 0.04, 0]), P(`shin.${sfx}`), [0.085, 0.09], [0.058, 0.062], 12, 6, blend(I(`thigh.${sfx}`), I('pelvis'), I(`shin.${sfx}`)));
    tube(prims.trousers, P(`shin.${sfx}`), add(P(`foot.${sfx}`), [0, 0.02, 0]), [0.058, 0.062], [0.042, 0.045], 12, 6, blend(I(`shin.${sfx}`), I(`thigh.${sfx}`), I(`foot.${sfx}`)), { capB: true });
    // The shoe: a box from the heel to the toe, on the foot and the toe, its sole flat on the floor
    // (y = 0) at rest: the box is 9 cm tall, centred 4.5 cm up.
    const heelAt = P(`foot.${sfx}`);
    const toeAt = P(`toe.${sfx}`);
    const heel = [heelAt[0], 0.045, heelAt[2] - 0.05];
    const toeEnd = [toeAt[0], 0.045, toeAt[2] + 0.06];
    const fwd = norm(sub(toeEnd, heel));
    const side = norm(cross([0, 1, 0], fwd));
    const up = cross(fwd, side);
    const mid = add(heel, mul(sub(toeEnd, heel), 0.5));
    box(prims.shoes, add(mid, [0, 0.0, 0]), side, up, fwd, [0.1, 0.09, len(sub(toeEnd, heel))], (p) => {
      const f = dot(sub(p, heel), fwd) / len(sub(toeEnd, heel));
      return f > 0.72 ? [[I(`toe.${sfx}`), 1]] : [[I(`foot.${sfx}`), 1]];
    });
  }
  // The arms: the upper arm in the shirt's sleeve, the forearm bare (its distal half on the twist
  // joint), the hand.
  for (const sfx of ['L', 'R']) {
    const sh = P(`upper_arm.${sfx}`);
    const el = P(`forearm.${sfx}`);
    const wr = P(`hand.${sfx}`);
    // The shoulder: a short sleeve piece from the clavicle.
    tube(prims.shirt, add(P(`clavicle.${sfx}`), [0, 0.0, 0]), sh, [0.06, 0.07], [0.06, 0.065], 12, 2, blend(I(`clavicle.${sfx}`), I('spine_03'), I(`upper_arm.${sfx}`)));
    tube(prims.shirt, sh, el, [0.06, 0.065], [0.048, 0.05], 12, 6, blend(I(`upper_arm.${sfx}`), I(`clavicle.${sfx}`), I(`forearm.${sfx}`)));
    const twist = I(`forearm_twist.${sfx}`);
    tube(prims.skin, el, wr, [0.046, 0.048], [0.034, 0.03], 12, 6, (f) => {
      const t = Math.min(1, Math.max(0, (f - 0.25) / 0.65));
      const wt = 0.85 * t * t * (3 - 2 * t);
      const out = [[I(`forearm.${sfx}`), 1 - wt], [twist, wt]];
      if (f < 0.2) out.push([I(`upper_arm.${sfx}`), 0.5 * (1 - f / 0.2)]);
      if (f > 0.85) out.push([I(`hand.${sfx}`), 0.4 * ((f - 0.85) / 0.15)]);
      return out;
    });
    // The hand: a flat box along the arm's direction, palm down at rest.
    const dir = norm(sub(wr, el));
    const sideAxis = norm(cross(dir, [0, 0, 1]));
    const up = norm(cross(sideAxis, dir));
    box(prims.hands, add(wr, mul(dir, 0.09)), dir, up, sideAxis, [0.18, 0.035, 0.085], () => [[I(`hand.${sfx}`), 1]]);
  }
  // The neck and head (skin), a nose that shows which way the figure faces, and a cap of hair.
  tube(prims.skin, P('neck'), add(P('head'), [0, 0.02, 0]), [0.05, 0.055], [0.05, 0.055], 12, 3, blend(I('neck'), I('spine_03'), I('head')));
  const hc = add(P('head'), [0, 0.09, 0.01]);
  ellipsoid(prims.skin, hc, [0.092, 0.118, 0.105], I('head'));
  box(prims.skin, add(hc, [0, -0.01, 0.1]), [1, 0, 0], [0, 1, 0], [0, 0, 1], [0.025, 0.04, 0.03], () => [[I('head'), 1]]);
  ellipsoid(prims.hair, add(hc, [0, 0.012, -0.008]), [0.1, 0.122, 0.112], I('head'), {
    to: 0.62,
    skip: (v, u) => v > 0.3 && (u < 0.16 || u > 0.84),
  });
  return Object.values(prims);
}

// The library's mesh: one thin stick per bone, so the file is a valid skinned model.
function clipMesh(rig) {
  const pr = newPrim('mannequin');
  rig.joints.forEach((j, i) => {
    if (!j.parent) return;
    const p = rig.index[j.parent];
    tube(pr, rig.joints[p].pos, j.pos, [0.012, 0.012], [0.01, 0.01], 4, 1, () => [[p, 1]]);
  });
  return [pr];
}

// ── glTF writing ─────────────────────────────────────────────────────────────
const f32 = (x) => Math.fround(x);
function writeGltf(dir, name, rig, prims, clips) {
  const bin = [];
  let byteLen = 0;
  const bufferViews = [];
  const accessors = [];
  const pushBuf = (arrayBuffer, target) => {
    const pad = (4 - (byteLen % 4)) % 4;
    if (pad) { bin.push(Buffer.alloc(pad)); byteLen += pad; }
    const view = { buffer: 0, byteOffset: byteLen, byteLength: arrayBuffer.byteLength };
    if (target) view.target = target;
    bufferViews.push(view);
    bin.push(Buffer.from(arrayBuffer));
    byteLen += arrayBuffer.byteLength;
    return bufferViews.length - 1;
  };
  const acc = (values, comps, type, componentType, target, withMinMax) => {
    let arr;
    if (componentType === 5126) arr = new Float32Array(values.flat());
    else if (componentType === 5123) arr = new Uint16Array(values.flat());
    else arr = new Uint32Array(values.flat());
    const view = pushBuf(arr.buffer, target);
    const a = { bufferView: view, componentType, count: values.length, type };
    if (withMinMax) {
      const mn = new Array(comps).fill(Infinity);
      const mx = new Array(comps).fill(-Infinity);
      for (const v of values) {
        const vv = Array.isArray(v) ? v : [v];
        vv.forEach((x, i) => { mn[i] = Math.min(mn[i], f32(x)); mx[i] = Math.max(mx[i], f32(x)); });
      }
      a.min = mn; a.max = mx;
    }
    accessors.push(a);
    return accessors.length - 1;
  };
  const nodes = [];
  const jointNodes = [];
  // An "Armature" node over the joints when the rig is scaled (the library), else the joints are roots.
  let armature = null;
  if (rig.outerScale !== 1) {
    armature = nodes.length;
    nodes.push({ name: 'Armature', scale: [rig.outerScale, rig.outerScale, rig.outerScale], children: [] });
  }
  rig.joints.forEach((j, i) => {
    const l = rig.restLocal[i];
    jointNodes.push(nodes.length);
    nodes.push({ name: j.name, translation: l.t.map(f32), rotation: l.r.map(f32) });
  });
  rig.joints.forEach((j, i) => {
    const parentNode = j.parent ? jointNodes[rig.index[j.parent]] : armature;
    if (parentNode == null) return;
    (nodes[parentNode].children = nodes[parentNode].children || []).push(jointNodes[i]);
  });
  const materials = prims.map((p) => ({ name: p.material, pbrMetallicRoughness: { baseColorFactor: [0.7, 0.7, 0.7, 1], metallicFactor: 0, roughnessFactor: 0.7 } }));
  const primitives = prims.map((p, mi) => ({
    attributes: {
      POSITION: acc(p.pos, 3, 'VEC3', 5126, 34962, true),
      NORMAL: acc(p.nrm, 3, 'VEC3', 5126, 34962, false),
      TEXCOORD_0: acc(p.uv, 2, 'VEC2', 5126, 34962, false),
      JOINTS_0: acc(p.jnt, 4, 'VEC4', 5123, 34962, false),
      WEIGHTS_0: acc(p.wgt, 4, 'VEC4', 5126, 34962, false),
    },
    indices: acc(p.idx.map((x) => x), 1, 'SCALAR', 5125, 34963, false),
    material: mi,
  }));
  const ibm = acc(rig.ibm.map((m) => m), 16, 'MAT4', 5126, null, false);
  const meshNode = nodes.length;
  nodes.push({ name: `${name}_mesh`, mesh: 0, skin: 0 });
  const roots = [armature != null ? armature : jointNodes[0], meshNode];
  const animations = clips.map((clip) => {
    const k = rigKeys(rig, clip);
    const input = acc(k.times, 1, 'SCALAR', 5126, null, true);
    const samplers = [];
    const channels = [];
    rig.joints.forEach((j, i) => {
      if (!ROLES.includes(j.role)) return; // twist joints: driven at run time
      samplers.push({ input, output: acc(k.rot[i], 4, 'VEC4', 5126, null, false), interpolation: 'LINEAR' });
      channels.push({ sampler: samplers.length - 1, target: { node: jointNodes[i], path: 'rotation' } });
    });
    samplers.push({ input, output: acc(k.trans, 3, 'VEC3', 5126, null, false), interpolation: 'LINEAR' });
    channels.push({ sampler: samplers.length - 1, target: { node: jointNodes[0], path: 'translation' } });
    return { name: clip.name, samplers, channels };
  });
  const gltf = {
    asset: { version: '2.0', generator: 'HumanityOS scripts/gen-test-figure.js', copyright: 'HumanityOS contributors; our own procedural work' },
    scene: 0,
    scenes: [{ name, nodes: roots }],
    nodes,
    meshes: [{ name, primitives }],
    materials,
    skins: [{ name: `${name}_skin`, joints: jointNodes, inverseBindMatrices: ibm, skeleton: armature != null ? armature : jointNodes[0] }],
    animations,
    accessors,
    bufferViews,
    buffers: [{ uri: `${name}.bin`, byteLength: byteLen }],
  };
  const json = JSON.stringify(gltf, (key, v) => (typeof v === 'number' && !Number.isInteger(v) ? Number(v.toPrecision(9)) : v), 1) + '\n';
  return { json, bin: Buffer.concat(bin) };
}

function build() {
  const canon = clipJoints().map((j) => ({ ...j, pos: mul(j.pos, 1 / CLIP_SCALE) }));
  const bj = bodyJoints();
  const body = buildRig(bj, restRotationsBody(bj), 1, canon);
  const cj = clipJoints();
  const lib = buildRig(cj, restRotationsClip(cj), 0.01, canon);
  const walk = CLIPS.find((c) => c.name === 'walk');
  return {
    body: writeGltf(OUT.body, 'hos_test_figure', body, bodyMesh(body), [{ ...walk, name: 'ref_walk' }]),
    clips: writeGltf(OUT.clips, 'hos_test_clips', lib, clipMesh(lib), CLIPS),
  };
}

function main() {
  const check = process.argv.includes('--check');
  const made = build();
  let differs = false;
  for (const [key, name] of [['body', 'hos_test_figure'], ['clips', 'hos_test_clips']]) {
    const dir = OUT[key];
    const files = [[`${name}.gltf`, Buffer.from(made[key].json)], [`${name}.bin`, made[key].bin]];
    for (const [file, bytes] of files) {
      const p = path.join(dir, file);
      if (check) {
        const have = fs.existsSync(p) ? fs.readFileSync(p) : null;
        // The .gltf is text: compare without line endings (git may check it out with CRLF).
        const same = file.endsWith('.gltf')
          ? have && have.toString().replace(/\r\n/g, '\n') === bytes.toString()
          : have && have.equals(bytes);
        if (!same) { console.error(`${path.relative(ROOT, p)} differs from what scripts/gen-test-figure.js makes`); differs = true; }
      } else {
        fs.mkdirSync(dir, { recursive: true });
        fs.writeFileSync(p, bytes);
        console.log(`wrote ${path.relative(ROOT, p)} (${bytes.length} bytes)`);
      }
    }
  }
  if (check) {
    if (differs) process.exit(1);
    console.log('the test figure and its clips match the generator');
  }
}

if (require.main === module) main();
module.exports = { build, qarc, qmul, qrot, turnsFrom, CLIPS };
