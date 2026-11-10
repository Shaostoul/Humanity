#!/usr/bin/env node
// THE LOOK, CONVERTED (character animation, "the look", 2026-11-09; docs/design/
// character-animation.md section 16). Our own converter, like scripts/gen-makehuman-body.js: it reads
// MakeHuman's CC0 DATA and writes what the game needs to shape every person's own face and body
// feature by feature and to dress them in real clothes, shoes, hats and hair:
//
//   assets/models/characters/makehuman/makehuman_details.bin   every face and body detail target
//       the sliders drive, as sparse vertex deltas over the base mesh (the layout at `writeDetails`)
//   data/characters/details.ron      the detail sliders, grouped, each naming the targets it drives
//                                    and the bodies it is for (generated: never edit by hand)
//   assets/models/characters/makehuman/garments/<id>/<id>.bin   each garment, hair, brows or lashes
//       fitted to the base mesh: every vertex's three base vertices, weights and offset (MakeHuman's
//       .mhclo binding), its skin weights, its drawn vertices and triangles, and the base vertices
//       it hides (delete_verts) (the layout at `writeGarment`), beside its textures copied as they
//       are (the sources, kept whole) and its thumbnail
//   assets/models/characters/makehuman/garments/<id>/<id>.dds   the texture the game draws: colour
//       beside normal map, every level, BC7-encoded here at conversion time by our own encoder
//       (scripts/lib/garment-texture.js and scripts/lib/bc7.js, 2026-11-10; shipped data, not a
//       cache: CLAUDE.md's no-caches rule forbids encoding on load and keeping it)
//   data/characters/garments.ron     each of them: kind, files, material (generated)
//   assets/models/characters/makehuman/LOOK-SOURCE.md   the licence, the inputs and their hashes,
//       the outputs' hashes
//
// MakeHuman's CODE is AGPL and none of it is used or copied: a target is lines of "vertex dx dy dz"
// in decimetres; a .mhclo vertex line is "v1 v2 v3 w1 w2 w3 d1 d2 d3" (the garment vertex is the
// weighted sum of three base vertices plus an offset, each offset axis scaled by how far apart the
// two reference vertices of `x_scale`, `y_scale` and `z_scale` are against the distance written
// there) or a single "v" (the garment vertex sits on that base vertex); `delete_verts` lists the base
// vertices under the garment, with "a - b" for a run; an .mhmat is "key value" lines; an .mhw lists,
// per bone, garment vertex and weight. All of it was read from the files themselves.
//
// The detail sliders come from MakeHuman's own slider list (`modifiers/modeling_modifiers.json`, CC0,
// the same commit as the bodies): each modifier with a low and a high end is one slider, a left and a
// right one (`l-...`, `r-...`) are one slider moving both sides together (the asymmetry targets move
// one side on purpose), and its group is MakeHuman's. The measurement, asymmetry and body-shape
// targets, which that list does not name, are paired from their file names.
//
//   node scripts/gen-makehuman-look.js                 write everything (needs the sources)
//   node scripts/gen-makehuman-look.js --check         exit 1 if a committed file differs (without
//                                                      the sources: against LOOK-SOURCE.md's hashes)
//   node scripts/gen-makehuman-look.js --words FILE    also write the sliders' and garments' words
//                                                      as an i18n fragment (node scripts/i18n-add.js FILE)
//
// Every input is checked before it is read: the detail targets and the slider list against their
// git blob SHA-1s in the fetches' MANIFEST.json, the asset pack's files against the CRC-32 the zip's
// own central directory records, and the zip against the SHA-256 the character assets finding
// records. The sources are untrusted input: read as text and numbers, never run. Deterministic.

'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const os = require('os');
const body = require('./gen-makehuman-body.js');
const garmentTexture = require('./lib/garment-texture.js');

// Where the outputs are written and checked: the repo, or (HUMANITY_LOOK_ROOT, for the converter's
// own test) a copy of its outputs elsewhere, so a hand-edited file can be checked without touching
// the repo's.
const ROOT = process.env.HUMANITY_LOOK_ROOT || path.join(__dirname, '..');
const OUT = path.join(ROOT, 'assets', 'models', 'characters', 'makehuman');
const GARMENT_DIR = path.join(OUT, 'garments');
const DETAILS_RON = path.join(ROOT, 'data', 'characters', 'details.ron');
const GARMENTS_RON = path.join(ROOT, 'data', 'characters', 'garments.ron');
const NOTE = path.join(OUT, 'LOOK-SOURCE.md');
const MH = 'E:\\HumanityOS\\makehuman-src';
const DETAIL_SRC = process.env.HUMANITY_MAKEHUMAN_DETAILS || path.join(MH, 'detail-targets-a8bc2d54');
const PACK_SRC = process.env.HUMANITY_MAKEHUMAN_PACK || path.join(MH, 'system-assets-2026-11-09');
const PACK_ZIP = 'makehuman_system_assets_cc0.zip';
// docs/reference/findings/2026-11-08-character-assets.md, the 2026-11-09 (later) addendum.
const PACK_SHA256 = 'b542127a8e25547c7c29c19f2d1d2adb9a664c80396ecd694095dbc8028a0107';
const MODIFIERS = { rel: 'modifiers/modeling_modifiers.json', sha: '6cff6af98d4e7fa11b007b220d50dc1a627fc654' };
const DM = 0.1; // MakeHuman works in decimetres.
// A detail delta is stored to a tenth of a millimetre: MakeHuman writes three decimals of a decimetre.
const DELTA_UNIT_M = 1e-4;

// ── Small helpers ─────────────────────────────────────────────────────────
const sha256 = (b) => crypto.createHash('sha256').update(b).digest('hex');
const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const len = (a) => Math.hypot(a[0], a[1], a[2]);
function fail(msg) {
  throw new Error(msg);
}

/** The text of a source file, as lines without comments or blanks. */
function lines(text) {
  return text.split(/\r?\n/).map((l) => l.trim()).filter((l) => l && l[0] !== '#');
}

// ── CRC-32 and the zip's central directory (the pack's integrity) ─────────
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

/** Each entry's CRC-32 and size from a zip's central directory, by its path. */
function zipDirectory(zip) {
  let eocd = -1;
  for (let i = zip.length - 22; i >= Math.max(0, zip.length - 65557); i--) {
    if (zip.readUInt32LE(i) === 0x06054b50) { eocd = i; break; }
  }
  if (eocd < 0) fail('the asset pack zip has no end of central directory');
  const count = zip.readUInt16LE(eocd + 10);
  let p = zip.readUInt32LE(eocd + 16);
  const out = new Map();
  for (let k = 0; k < count; k++) {
    if (zip.readUInt32LE(p) !== 0x02014b50) fail('the asset pack zip central directory is malformed');
    const crc = zip.readUInt32LE(p + 16);
    const size = zip.readUInt32LE(p + 24);
    const nameLen = zip.readUInt16LE(p + 28);
    const extra = zip.readUInt16LE(p + 30);
    const comment = zip.readUInt16LE(p + 32);
    const name = zip.toString('utf8', p + 46, p + 46 + nameLen);
    out.set(name, { crc, size });
    p += 46 + nameLen + extra + comment;
  }
  return out;
}

/** Read the asset pack's files through one checker: each must match the zip's CRC-32 and size. */
function packReader(src) {
  const zip = fs.readFileSync(path.join(src, PACK_ZIP));
  const got = sha256(zip);
  if (got !== PACK_SHA256) fail(`${PACK_ZIP}: SHA-256 ${got}, the finding records ${PACK_SHA256}; refusing a changed pack`);
  const dir = zipDirectory(zip);
  const read = new Map();
  return {
    read(rel) {
      if (read.has(rel)) return read.get(rel);
      const entry = dir.get(rel);
      if (!entry) fail(`${rel} is not in ${PACK_ZIP}`);
      const buf = fs.readFileSync(path.join(src, 'x', ...rel.split('/')));
      if (buf.length !== entry.size || crc32(buf) !== entry.crc) fail(`${rel} differs from the copy in ${PACK_ZIP}; refusing it`);
      read.set(rel, buf);
      return buf;
    },
    has: (rel) => dir.has(rel),
    list: (prefix) => [...dir.keys()].filter((k) => k.startsWith(prefix) && !k.endsWith('/')).sort(),
    used: () => [...read.entries()].map(([rel, buf]) => [rel, sha256(buf)]).sort((a, b) => (a[0] < b[0] ? -1 : 1)),
  };
}

// ── PNG: the mean colour of a diffuse texture (decoded whole: garment-texture.js) ──
const decodePng = garmentTexture.decodePng;

/** The mean linear colour of a texture's covered texels (alpha over a half), sampled on a grid. */
function meanColour(png) {
  const { w, h, rgba } = decodePng(png);
  const size = { w, h };
  const lin = (c) => { const s = c / 255; return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4; };
  const sum = [0, 0, 0];
  let n = 0;
  const step = Math.max(1, Math.floor(Math.min(w, h) / 256));
  for (let y = 0; y < h; y += step) for (let x = 0; x < w; x += step) {
    const o = (y * w + x) * 4;
    if (rgba[o + 3] < 128) continue;
    sum[0] += lin(rgba[o]); sum[1] += lin(rgba[o + 1]); sum[2] += lin(rgba[o + 2]);
    n++;
  }
  return { w: size.w, h: size.h, mean: n ? sum.map((s) => Math.round((s / n) * 1000) / 1000) : [0.5, 0.5, 0.5] };
}

// ── The detail targets ────────────────────────────────────────────────────
/** The detail fetch's manifest: target path (under targets/) to git blob SHA-1. */
function detailManifest(src) {
  const m = JSON.parse(fs.readFileSync(path.join(src, 'MANIFEST.json'), 'utf8'));
  const out = new Map();
  for (const e of m.manifest) {
    const rel = e.path.replace(/^makehuman\/data\/targets\//, '');
    if (rel.endsWith('.target')) out.set(rel, e.git_blob_sha1);
  }
  return { files: out, sha: sha256(fs.readFileSync(path.join(src, 'MANIFEST.json'))) };
}

/** A target file, checked, as sparse deltas in metres (zero lines left out). */
function readTarget(src, manifest, rel, used) {
  const want = manifest.files.get(rel);
  if (!want) fail(`${rel} is not in the detail fetch's MANIFEST.json`);
  const buf = fs.readFileSync(path.join(src, 'targets', ...rel.split('/')));
  const got = body.gitBlobSha1(buf);
  if (got !== want) fail(`${rel}: git blob ${got}, the manifest records ${want}; refusing a changed source`);
  used.set(rel, want);
  const out = [];
  for (const l of lines(buf.toString('utf8'))) {
    const p = l.split(/\s+/);
    const d = [+p[1], +p[2], +p[3]];
    if (d.every((x) => x === 0)) continue;
    out.push([+p[0], d.map((x) => x * DM)]);
  }
  return out;
}

// MakeHuman's group names to the character page's groups (the arms-and-legs group splits by part).
function groupOf(mhGroup, base) {
  if (mhGroup === 'armslegs') {
    if (/^(hand)/.test(base)) return 'hands';
    if (/^(foot)/.test(base)) return 'feet';
    if (/^(upperarm|lowerarm)/.test(base)) return 'arms';
    return 'legs';
  }
  return { eyebrows: 'brows', ears: 'ears', cheek: 'cheeks', hip: 'hips', breast: 'chest', pelvis: 'hips', buttocks: 'hips' }[mhGroup] || mhGroup;
}

// The groups in the order the character page shows them (the face first, top down, then the body).
const GROUP_ORDER = ['head', 'forehead', 'brows', 'eyes', 'nose', 'cheeks', 'mouth', 'chin', 'ears', 'neck', 'torso', 'chest', 'stomach', 'hips', 'arms', 'hands', 'legs', 'feet', 'measure', 'bodyshape', 'asymmetry'];

/**
 * A slider id from a target's base, with its high end when MakeHuman has more than one slider on
 * that base ("nose-trans" + "up" is "nose_trans_up"; "nose-hump" is "nose_hump").
 */
function sliderId(base, high) {
  let id = base.replace(/-/g, '_');
  if (high) id += `_${high}`;
  id = id.replace(/^measure_/, 'm_').replace(/_scale_/g, '_').replace(/^bodyshapes_elvs_(fem|man)_/, 'shape_');
  if (id.length > 24) id = id.replace(/upperarm/, 'uarm').replace(/lowerarm/, 'larm').replace(/upperleg/, 'uleg').replace(/lowerleg/, 'lleg');
  if (id.length > 24 || !/^[a-z0-9_-]+$/.test(id)) fail(`slider id ${id} (from ${base}) is not one a look can carry`);
  return id;
}

/**
 * The sliders: MakeHuman's modifiers (a low and a high end, or one target) and the measure, asym
 * and body-shape targets paired by name. Each slider: id, group, low and high target names, the
 * bodies it is for (empty: both), and the end words' ids.
 */
function makeSliders(src, mods, manifest, used) {
  const names = new Set([...manifest.files.keys()].map((rel) => path.basename(rel, '.target')));
  const relOf = new Map([...manifest.files.keys()].map((rel) => [path.basename(rel, '.target'), rel]));
  const sliders = [];
  const seen = new Set();
  // An id may be made once per body (the female and male body shapes share their names).
  const push = (s) => {
    const k = `${s.id}|${s.bodies.join(',')}`;
    if (seen.has(k) || seen.has(`${s.id}|`) || (!s.bodies.length && [...seen].some((x) => x.startsWith(`${s.id}|`)))) fail(`slider ${s.id} is made twice`);
    for (const t of [...s.low, ...s.high]) if (!names.has(t)) fail(`slider ${s.id} names target ${t}, which the fetch does not have`);
    seen.add(k);
    sliders.push(s);
  };
  // Bases MakeHuman puts more than one slider on (the three ways a part moves).
  const counted = new Map();
  for (const g of mods) for (const m of g.modifiers) if (m.target) counted.set(m.target, (counted.get(m.target) || 0) + 1);
  const repeated = new Set([...counted.entries()].filter(([, n]) => n > 1).map(([t]) => t));
  for (const g of mods) {
    if (/^macrodetails|^genitals$/.test(g.group)) continue;
    for (const m of g.modifiers) {
      if (m.macrovar) {
        // The breast's two macro sliders, from MakeHuman's average-build targets of the female body.
        const f = 'female-young-averagemuscle-averageweight';
        if (m.macrovar === 'BreastSize') push({ id: 'breast_size', group: 'chest', low: [`${f}-mincup-averagefirmness`], high: [`${f}-maxcup-averagefirmness`], bodies: ['female'], ends: ['decr', 'incr'] });
        else if (m.macrovar === 'BreastFirmness') push({ id: 'breast_firmness', group: 'chest', low: [`${f}-averagecup-minfirmness`], high: [`${f}-averagecup-maxfirmness`], bodies: ['female'], ends: ['decr', 'incr'] });
        continue;
      }
      if (m.target === 'bulge') continue; // with the genitals, not fetched
      const sided = /^[lr]-/.test(m.target);
      const base = sided ? m.target.slice(2) : m.target;
      if (sided && m.target.startsWith('l-')) continue; // the r- row makes the slider for both sides
      const sides = sided ? ['l-', 'r-'] : [''];
      if (m.min && m.max) {
        push({ id: sliderId(base, repeated.has(m.target) ? m.max : null), group: groupOf(g.group, base), low: sides.map((s) => `${s}${base}-${m.min}`), high: sides.map((s) => `${s}${base}-${m.max}`), bodies: [], ends: [m.min, m.max] });
      } else {
        push({ id: sliderId(base, null), group: groupOf(g.group, base), low: [], high: sides.map((s) => `${s}${base}`), bodies: [], ends: ['none', 'full'] });
      }
    }
  }
  // Measurements: measure-<part>-decr and -incr.
  for (const n of [...names].filter((x) => x.startsWith('measure-') && x.endsWith('-incr')).sort()) {
    const base = n.slice(0, -'-incr'.length);
    push({ id: sliderId(base, null), group: 'measure', low: [`${base}-decr`], high: [n], bodies: [], ends: ['decr', 'incr'] });
  }
  // Asymmetry: asym-<part>-<n>-l and -r, one slider from the left side to the right.
  for (const n of [...names].filter((x) => /^asymm?-.*-l$/.test(x)).sort()) {
    const base = n.slice(0, -2);
    push({ id: sliderId(base.replace(/^asymm-/, 'asym-'), null), group: 'asymmetry', low: [n], high: [`${base}-r`], bodies: [], ends: ['left', 'right'] });
  }
  // Body shapes: one target each, for the body they were made for.
  for (const n of [...names].filter((x) => x.startsWith('bodyshapes-')).sort()) {
    const fem = n.includes('-fem-');
    push({ id: sliderId(n, null), group: 'bodyshape', low: [], high: [n], bodies: [fem ? 'female' : 'male'], ends: ['none', 'full'] });
  }
  // Read every target a slider names, checked.
  const targets = new Map();
  for (const s of sliders) for (const t of [...s.low, ...s.high]) if (!targets.has(t)) targets.set(t, readTarget(src, manifest, relOf.get(t), used));
  sliders.sort((a, b) => GROUP_ORDER.indexOf(a.group) - GROUP_ORDER.indexOf(b.group));
  for (const s of sliders) if (!GROUP_ORDER.includes(s.group)) fail(`slider ${s.id} has group ${s.group}, which the page does not order`);
  return { sliders, targets };
}

/**
 * makehuman_details.bin:
 *   "HOSMHD01"; u32 base vertices Nb (the mesh the targets are for); u32 targets K;
 *   K times: u32 name length, the name's bytes, u32 n, then n x (u32 base vertex, i16 dx, dy, dz in
 *   tenths of a millimetre, i16 0): the target's movement (sparse).
 */
function writeDetails(targets, nb) {
  const b = new body.Bin();
  b.tag('HOSMHD01');
  b.u32(nb);
  const names = [...targets.keys()].sort();
  b.u32(names.length);
  for (const name of names) {
    b.u32(name.length);
    b.tag(name);
    const list = targets.get(name);
    b.u32(list.length);
    const rec = Buffer.alloc(list.length * 12);
    list.forEach(([vi, d], k) => {
      if (vi >= nb) fail(`target ${name} moves vertex ${vi}, past the base mesh's ${nb}`);
      rec.writeUInt32LE(vi, k * 12);
      for (let a = 0; a < 3; a++) {
        const q = Math.round(d[a] / DELTA_UNIT_M);
        if (Math.abs(q * DELTA_UNIT_M - d[a]) > 1e-7 || Math.abs(q) > 32767) fail(`target ${name}: a delta of ${d[a]} m is not a whole tenth of a millimetre`);
        rec.writeInt16LE(q, k * 12 + 4 + a * 2);
      }
    });
    b.parts.push(rec);
  }
  return b.bytes();
}

// ── Words (an i18n fragment for scripts/i18n-add.js) ──────────────────────
// Plain words for MakeHuman's names (the ELI5 mandate: no anatomy jargon on the page).
const PART = {
  head: 'Head', forehead: 'Forehead', eyebrows: 'Brows', neck: 'Neck', eye: 'Eye', nose: 'Nose', mouth: 'Mouth', ear: 'Ear', chin: 'Chin',
  cheek: 'Cheek', torso: 'Torso', hip: 'Hips', stomach: 'Belly', buttocks: 'Bottom', pelvis: 'Pelvis', breast: 'Breasts', nipple: 'Nipples',
  foot: 'Foot', hand: 'Hand', lowerarm: 'Forearm', upperarm: 'Upper arm', lowerleg: 'Lower leg', upperleg: 'Thigh', leg: 'Leg',
  lowerlegs: 'Lower legs', upperlegs: 'Thighs', measure: '', asym: '', asymm: '',
};
const WORD = {
  trans: 'position', scale: 'size', vert: 'height', horiz: 'width', depth: 'depth', fat: 'fat', muscle: 'muscle', age: 'age', angle: 'tilt',
  back: 'back', double: 'double chin', bag: 'bags', height: 'height', eyefold: 'eyelid fold', epicanthus: 'inner corner fold', push1: 'upper lid',
  push2: 'lower lid', corner1: 'inner corner', corner2: 'outer corner', height1: 'upper lid height', height2: 'opening', height3: 'lower lid height',
  nostrils: 'nostrils', width: 'width', point: 'tip', base: 'base', width1: 'bridge width', width2: 'middle width', width3: 'lower width',
  compression: 'pinch', curve: 'curve', greek: 'straight profile', hump: 'bump', volume: 'fullness', septumangle: 'base angle', flaring: 'flare',
  lowerlip: 'lower lip', upperlip: 'upper lip', cupidsbow: 'upper lip curve', dimples: 'dimples', laugh: 'smile', lines: 'lines', ext: 'corners',
  angles: 'corners', middle: 'middle', philtrum: 'groove above the lip', flap: 'flap', lobe: 'lobe', rot: 'turn', shape: 'shape', pointed: 'pointed',
  wing: 'wing', jaw: 'jaw', drop: 'drop', cleft: 'dimple', prominent: 'jut', bones: 'bones', prognathism: 'underbite', inner: 'inner fullness',
  vshape: 'V shape', dorsi: 'back muscles', pectoral: 'chest muscles', waist: 'waist', navel: 'navel', tone: 'firmness', pregnant: 'pregnancy',
  dist: 'spacing', size: 'size', firmness: 'firmness', fingers: 'fingers', diameter: 'thickness', distance: 'spacing', length: 'length',
  valgus: 'knock knees', circ: 'around', frontchest: 'chest front', nubian: 'brow ridge', temple: 'temples', oval: 'oval', round: 'round',
  rectangular: 'rectangular', square: 'square', triangular: 'triangular', invertedtriangular: 'inverted triangle', diamond: 'diamond',
  ankle: 'ankle', bust: 'bust', calf: 'calf', hips: 'hips', knee: 'knee', lowerarm: 'forearm', neck: 'neck', waist: 'waist', shoulder: 'shoulder',
  thigh: 'thigh', underbust: 'under the bust', upperarm: 'upper arm', wrist: 'wrist', brown: 'brow', top: 'top of the head', trunk: 'trunk',
  cheek: 'cheek', ear: 'ear', eye: 'eye', mouth: 'mouth', nose: 'nose', temple: 'temple', breast: 'breast', concave: 'concave', convex: 'convex', down: 'down', up: 'up', in: 'in', out: 'out',
};
const END = {
  decr: 'less', incr: 'more', down: 'down', up: 'up', in: 'in', out: 'out', backward: 'back', forward: 'forward', concave: 'hollow', convex: 'rounded',
  compress: 'pinched', uncompress: 'open', left: 'left', right: 'right', none: 'none', full: 'full', pointed: 'pointed', triangle: 'triangular',
  square: 'square', round: 'round',
};
const GROUP_WORDS = {
  head: ['Head', 'The shape and size of the head as a whole.'],
  forehead: ['Forehead', 'The forehead, the brow ridge and the temples.'],
  brows: ['Brows', 'Where the brows sit and how they slope.'],
  eyes: ['Eyes', 'The eyes and eyelids, both sides together.'],
  nose: ['Nose', 'The nose: its size, bridge, tip and nostrils.'],
  cheeks: ['Cheeks', 'The cheeks and cheekbones, both sides together.'],
  mouth: ['Mouth', 'The lips and the mouth around them.'],
  chin: ['Chin and jaw', 'The chin and the jaw.'],
  ears: ['Ears', 'The ears, both sides together.'],
  neck: ['Neck', 'The neck.'],
  torso: ['Torso', 'The chest and back as a whole, and their muscles.'],
  chest: ['Chest', 'The breasts and chest.'],
  stomach: ['Belly', 'The belly and the navel.'],
  hips: ['Hips', 'The hips, the waist and the bottom.'],
  arms: ['Arms', 'The upper arms and forearms, both sides together.'],
  hands: ['Hands', 'The hands and fingers, both sides together.'],
  legs: ['Legs', 'The thighs and lower legs, both sides together.'],
  feet: ['Feet', 'The feet, both sides together.'],
  measure: ['Measurements', 'Each one changes the body around one place people measure (bust, waist, hips, arms, legs), as a tailor would.'],
  bodyshape: ['Body shapes', "Whole-body shapes a MakeHuman artist made and named by where a body carries its weight (the names are the artist's). Each adds its shape on top of the rest."],
  asymmetry: ['One side', 'Real faces and bodies are not quite the same on both sides. Each slider makes one side differ, to the left or to the right.'],
};

// Names the rule below would word badly, by slider id.
const NAMED = {
  neck_double: 'Double chin', leg_valgus: 'Knock knees', chin_jaw_drop: 'Jaw drop', chin_bones: 'Jaw width at the back',
  chin_prognathism: 'Underbite', chin_prominent: 'Chin jut', cheek_bones: 'Cheekbones', cheek_inner: 'Inner cheeks', cheek_volume: 'Cheek fullness',
  torso_muscle_dorsi: 'Back muscles', torso_muscle_pectoral: 'Chest muscles', mouth_philtrum_volume: 'Groove above the lip',
  breast_volume_vert: 'Breast fullness, high or low', hip_waist: 'Waist height', lowerlegs_height: 'Lower leg length', upperlegs_height: 'Thigh length',
  eye_bag_incr: 'Eye bags', eye_bag_out: 'Eye bags, in or out', eye_eyefold_angle: 'Eyelid fold tilt', eye_eyefold_convex: 'Eyelid fold shape',
  eye_eyefold_up: 'Eyelid fold height', eye_push1: 'Upper eyelid, in or out', eye_push2: 'Lower eyelid, in or out', eye_epicanthus: 'Inner eye fold',
  eye_height1: 'Upper eyelid height', eye_height2: 'Eye opening', eye_height3: 'Lower eyelid height', eye_corner1: 'Inner eye corner',
  eye_corner2: 'Outer eye corner', eye_scale: 'Eye size', head_age: 'Head age', head_fat: 'Head fullness', forehead_nubian: 'Brow ridge',
  forehead_temple: 'Temples', ear_rot: 'Ear turn', ear_wing: 'Ears stand out', ear_flap: 'Ear rim', stomach_navel_out: 'Navel, in or out',
  stomach_navel_up: 'Navel height', stomach_pregnant: 'Pregnancy', stomach_tone: 'Belly firmness', pelvis_tone: 'Pelvis firmness',
  buttocks_volume: 'Bottom fullness', nose_greek: 'Straight nose profile', nose_septumangle: 'Nose base angle', nose_compression: 'Nose pinch',
  mouth_angles: 'Mouth corners', mouth_cupidsbow: 'Upper lip curve', mouth_cupidsbow_width: 'Upper lip curve width', mouth_laugh_lines: 'Smile lines',
  mouth_lowerlip_ext: 'Lower lip corners', mouth_upperlip_ext: 'Upper lip corners', mouth_dimples: 'Dimples', breast_trans: 'Breast height',
  breast_dist: 'Breast spacing', breast_point: 'Breast point', nipple_size: 'Nipple size', nipple_point: 'Nipple point',
  hand_fingers_distance: 'Finger spacing', hand_fingers_diameter: 'Finger thickness', hand_fingers_length: 'Finger length',
  upperarm_shoulder_muscle: 'Shoulder muscle', m_frontchest_dist: 'Chest width at the front', m_lowerleg_height: 'Lower leg length',
  m_upperleg_height: 'Thigh length', m_neck_height: 'Neck length', m_napetowaist_dist: 'Back length, neck to waist',
  m_waisttohip_dist: 'Waist to hip length', m_shoulder_dist: 'Shoulder width', m_lowerarm_length: 'Forearm length', m_upperarm_length: 'Upper arm length',
  m_underbust_circ: 'Under the bust, around', eye_bag_height: 'Eye bag height', ear_scale: 'Ear size', ear_vert: 'Ear height',
  ear_lobe: 'Ear lobes', ear_depth: 'Ear depth', ear_shape_triangle: 'Ear shape, pointed or triangular', ear_shape_round: 'Ear shape, round or square',
  hand_scale: 'Hand size', foot_scale: 'Foot size', hand_trans: 'Hands, in or out',
};
// The trans sliders' axis in words, by the high end MakeHuman gives it.
const AXIS = { out: 'side to side', up: 'up or down', forward: 'forward or back' };
// Parts that come in pairs, named in the plural (one slider moves both sides).
const PLURAL = { Eye: 'Eyes', Ear: 'Ears', Cheek: 'Cheeks', Foot: 'Feet', Hand: 'Hands', Forearm: 'Forearms', 'Upper arm': 'Upper arms', Thigh: 'Thighs', 'Lower leg': 'Lower legs', Leg: 'Legs' };
const LIMB = /^(lowerarm|upperarm|lowerleg|upperleg|foot|hand)$/;

/** A slider's name in words, from its target's base. */
function sliderWords(s) {
  if (NAMED[s.id]) return NAMED[s.id];
  const t = (s.high[0] || s.low[0]).replace(/^[lr]-/, '');
  const parts = t.replace(/-(decr|incr|down|up|in|out|backward|forward|concave|convex|compress|uncompress|l|r)$/, '').split('-');
  const cap = (w) => w.charAt(0).toUpperCase() + w.slice(1);
  if (parts[0] === 'measure') {
    const what = parts.slice(1).filter((p) => p !== 'circ' && p !== 'dist' && p !== 'length');
    return `${cap(what.map((p) => WORD[p] || p).join(' '))}, around`;
  }
  if (/^asymm?$/.test(parts[0])) return `${cap(WORD[parts[1]] || parts[1])} ${parts[2]}`;
  if (parts[0] === 'bodyshapes') return parts.slice(3).join(' ').replace(/^./, (c) => c.toUpperCase());
  if (s.id === 'breast_size') return 'Breast size';
  if (s.id === 'breast_firmness') return 'Breast firmness';
  let head = PART[parts[0]] !== undefined ? PART[parts[0]] : parts[0];
  if (s.low.length + s.high.length > 2 || s.high.length > 1) head = PLURAL[head] || head;
  if (parts[1] === 'trans') return `${head}, ${AXIS[s.ends[1]] || 'position'}`;
  const rest = parts.slice(1).filter((p, i, a) => !(p === 'scale' && a.length > i + 1)).map((p) => (p === 'vert' && LIMB.test(parts[0]) ? 'length' : WORD[p] || p));
  return cap([head, ...rest].filter(Boolean).join(' ').trim());
}

function wordsFragment(sliders, garments) {
  const c = {};
  for (const [g, [name, hint]] of Object.entries(GROUP_WORDS)) {
    c[`detail_group_${g}`] = name;
    c[`detail_group_${g}_hint`] = hint;
  }
  for (const s of sliders) {
    c[`detail_${s.id}`] = sliderWords(s);
    for (const e of s.ends) if (!END[e]) fail(`slider ${s.id} ends at ${e}, which has no words`);
  }
  for (const [k, v] of Object.entries(END)) c[`detail_end_${k}`] = v;
  for (const g of garments) c[`garment_${g.id}`] = g.words;
  return { characters: c };
}

// ── The garments ──────────────────────────────────────────────────────────
/** Which assets of the pack are converted, and what kind each is. */
function garmentList(pack) {
  const out = [];
  for (const [folder, kindOf] of [['clothes', (id) => (id.startsWith('shoes') ? 'shoes' : id.startsWith('fedora') ? 'hat' : 'clothes')], ['hair', () => 'hair'], ['eyebrows', () => 'brows'], ['eyelashes', () => 'lashes']]) {
    const ids = [...new Set(pack.list(`${folder}/`).map((p) => p.split('/')[1]))].sort();
    for (const id of ids) {
      const mhclo = pack.list(`${folder}/${id}/`).find((p) => p.endsWith('.mhclo'));
      if (mhclo) out.push({ id, kind: kindOf(id), folder: `${folder}/${id}`, mhclo });
    }
  }
  return out;
}

/** An .mhclo: its header keys, its vertex bindings and the base vertices it hides. */
function parseMhclo(text) {
  const head = {};
  const verts = [];
  const hidden = [];
  let mode = '';
  for (const l of lines(text)) {
    const p = l.split(/\s+/);
    if (p[0] === 'verts') { mode = 'verts'; continue; }
    if (p[0] === 'delete_verts') { mode = 'delete'; continue; }
    if (/^[a-z_]/.test(p[0])) { (head[p[0]] = head[p[0]] || []).push(p.slice(1)); if (mode === 'delete') mode = ''; continue; }
    if (mode === 'verts') {
      if (p.length === 1) verts.push({ v: [+p[0], 0, 0], w: [1, 0, 0], d: [0, 0, 0] });
      else if (p.length === 9) verts.push({ v: [+p[0], +p[1], +p[2]], w: [+p[3], +p[4], +p[5]], d: [+p[6], +p[7], +p[8]] });
      else fail(`an .mhclo vertex line of ${p.length} numbers`);
    } else if (mode === 'delete') {
      for (let k = 0; k < p.length; k++) {
        if (p[k + 1] === '-') { for (let i = +p[k]; i <= +p[k + 2]; i++) hidden.push(i); k += 2; } else hidden.push(+p[k]);
      }
    }
  }
  return { head, verts, hidden: [...new Set(hidden)].sort((a, b) => a - b) };
}

/** An .obj: positions (decimetres), texture coordinates and faces (position and uv indices). */
function parseObj(text) {
  const v = [];
  const vt = [];
  const faces = [];
  for (const l of lines(text)) {
    const p = l.split(/\s+/);
    if (p[0] === 'v') v.push([+p[1], +p[2], +p[3]]);
    else if (p[0] === 'vt') vt.push([+p[1], +p[2]]);
    else if (p[0] === 'f') faces.push(p.slice(1).map((c) => { const q = c.split('/'); return [+q[0] - 1, q[1] ? +q[1] - 1 : -1]; }));
  }
  return { v, vt, faces };
}

/** An .mhmat as key to value. */
function parseMhmat(text) {
  const out = {};
  for (const l of lines(text)) {
    const i = l.search(/\s/);
    if (i > 0) out[l.slice(0, i)] = l.slice(i).trim();
  }
  return out;
}

/** Four (joint, weight) pairs, the largest, as unorm16 summing to exactly 65535. */
function topFour(m) {
  const top = [...m.entries()].filter(([, w]) => w > 0).sort((a, b) => b[1] - a[1] || a[0] - b[0]).slice(0, 4);
  if (!top.length) fail('a garment vertex with no weight');
  return body.unorm16(top);
}

function convertGarment(g, pack, shared) {
  const text = (rel) => pack.read(rel).toString('utf8');
  const clo = parseMhclo(text(g.mhclo));
  const objName = clo.head.obj_file[0][0];
  const obj = parseObj(text(`${g.folder}/${objName}`));
  if (obj.v.length !== clo.verts.length) fail(`${g.id}: ${obj.v.length} obj vertices, ${clo.verts.length} bindings`);
  const matRel = path.posix.normalize(`${g.folder}/${clo.head.material[0][0]}`);
  const mat = parseMhmat(text(matRel));
  const matDir = path.posix.dirname(matRel);
  const nb = shared.raw.length;
  const toM = (p) => [p[0] * DM, (p[1] - shared.groundY) * DM, p[2] * DM];
  for (const b of clo.verts) for (let k = 0; k < 3; k++) if (b.w[k] !== 0 && (b.v[k] < 0 || b.v[k] >= nb)) fail(`${g.id}: a binding names base vertex ${b.v[k]}`);
  const scale = ['x_scale', 'y_scale', 'z_scale'].map((k) => {
    const e = clo.head[k] && clo.head[k][0];
    if (!e) fail(`${g.id}: no ${k}`);
    return [+e[0], +e[1], +e[2] * DM];
  });
  // Skin weights: the garment's own .mhw where it has one (the shoes), else the bound base
  // vertices' weights blended by the binding's weights.
  let skin;
  const mhwRel = clo.head.vertexboneweights_file && `${g.folder}/${clo.head.vertexboneweights_file[0][0]}`;
  if (mhwRel) {
    const mhw = JSON.parse(text(mhwRel)).weights;
    const per = clo.verts.map(() => new Map());
    for (const [bone, list] of Object.entries(mhw)) {
      if (!list.length) continue;
      const j = shared.jointIndex.get(shared.keptOf(bone));
      for (const [vi, w] of list) if (vi < per.length) per[vi].set(j, (per[vi].get(j) || 0) + w);
    }
    skin = per.map((m, i) => {
      if (m.size) return topFour(m);
      return blendSkin(clo.verts[i], shared, g.id);
    });
  } else skin = clo.verts.map((b) => blendSkin(b, shared, g.id));
  // Drawn vertices: each distinct (position, uv) corner, in face order; quads on the shorter diagonal.
  const ref = obj.v.map(toM);
  const key = new Map();
  const drawn = [];
  const idx = [];
  const corner = ([vi, ti]) => {
    const k = `${vi}|${ti}`;
    if (!key.has(k)) {
      key.set(k, drawn.length);
      const t = ti >= 0 && obj.vt[ti] ? obj.vt[ti] : [0, 0];
      drawn.push([vi, t[0], 1 - t[1]]);
    }
    return key.get(k);
  };
  for (const f of obj.faces) {
    const c = f.map(corner);
    if (f.length === 3) idx.push(c[0], c[1], c[2]);
    else if (f.length === 4) {
      const [a, b, cc, d] = f.map((x) => x[0]);
      if (len(sub(ref[a], ref[cc])) <= len(sub(ref[b], ref[d]))) idx.push(c[0], c[1], c[2], c[0], c[2], c[3]);
      else idx.push(c[0], c[1], c[3], c[1], c[2], c[3]);
    } else for (let k = 1; k + 1 < f.length; k++) idx.push(c[0], c[k], c[k + 1]);
  }
  // Textures, copied as they are.
  const tex = (k) => (mat[k] ? path.posix.normalize(`${matDir}/${mat[k]}`) : null);
  const diffuse = tex('diffuseTexture');
  if (!diffuse) fail(`${g.id}: no diffuse texture`);
  const files = [diffuse, tex('normalmapTexture'), tex('aomapTexture')].filter(Boolean);
  const thumb = `${g.folder}/${path.posix.basename(g.mhclo, '.mhclo')}.thumb`;
  const colour = meanColour(pack.read(diffuse));
  const alpha = /true/i.test(mat.transparent || '');
  const twoSided = /false/i.test(mat.backfaceCull || '');
  const shininess = Math.min(1, Math.max(0, +(mat.shininess || 0.2)));
  return {
    id: g.id,
    kind: g.kind,
    name: (clo.head.name && clo.head.name[0][0]) || g.id,
    tags: (clo.head.tag || []).map((t) => t.join(' ')).filter((t) => !/MakeHuman/.test(t)).map((t) => t.toLowerCase()),
    clo,
    scale,
    skin,
    ref,
    drawn,
    idx,
    files,
    thumb: pack.has(thumb) ? thumb : null,
    diffuse,
    normal: tex('normalmapTexture'),
    ao: tex('aomapTexture'),
    colour,
    alpha,
    twoSided,
    // A heuristic from MakeHuman's Phong shininess (0 matte to 1 glossy) to a roughness.
    roughness: Math.round((0.95 - 0.55 * shininess) * 100) / 100,
    zDepth: clo.head.z_depth ? +clo.head.z_depth[0][0] : 50,
  };
}

/** A bound vertex's skin: its three base vertices' merged weights, blended by the binding. */
function blendSkin(b, shared, id) {
  const m = new Map();
  for (let k = 0; k < 3; k++) {
    if (b.w[k] === 0) continue;
    // A garment bound to a helper (the tights, the skirt, the hair helper) takes the weights of the
    // body vertex nearest that helper vertex, so cloth and the skin under it bend alike: the
    // helpers' own weights are coarser, and the female suits fitted to the tights parted from the
    // skin at the waist in a crouch (the poke-through test, 2026-11-09).
    const vw = shared.perVertex.get(shared.skinOf(b.v[k]));
    if (!vw) fail(`${id}: base vertex ${b.v[k]} has no weights`);
    for (const [j, w] of vw) m.set(j, (m.get(j) || 0) + w * b.w[k]);
  }
  return topFour(m);
}

/**
 * A garment's .bin:
 *   "HOSMHG01"; u32 bound vertices Nv; u32 drawn vertices Nd; u32 indices Ni; u32 hidden Nh;
 *   3 x (u32 a, u32 b, f32 distance m): x_scale, y_scale, z_scale (an offset axis is scaled by
 *     |base[a] - base[b]| along it over the distance);
 *   Nv x (3 x u32 base vertex, 3 x f32 weight, 3 x f32 offset m): the binding;
 *   Nv x 3 x f32: the .obj's own vertex positions (metres, the raw base mesh's frame): where
 *     MakeHuman fitted it on the raw base mesh;
 *   Nv x (4 x u8 joint, 4 x u16 weight summing to 65535): the skin;
 *   Nd x (u32 bound vertex, f32 u, f32 v); Ni x u32 (triangles over the drawn vertices);
 *   Nh x u32: the base vertices it hides (MakeHuman's delete_verts).
 */
function writeGarment(g) {
  const b = new body.Bin();
  b.tag('HOSMHG01');
  b.u32(g.clo.verts.length);
  b.u32(g.drawn.length);
  b.u32(g.idx.length);
  b.u32(g.clo.hidden.length);
  for (const [a, c, d] of g.scale) { b.u32(a); b.u32(c); b.f32s([d]); }
  const bind = Buffer.alloc(g.clo.verts.length * 36);
  g.clo.verts.forEach((v, i) => {
    for (let k = 0; k < 3; k++) {
      bind.writeUInt32LE(v.w[k] === 0 ? 0 : v.v[k], i * 36 + k * 4);
      bind.writeFloatLE(v.w[k], i * 36 + 12 + k * 4);
      bind.writeFloatLE(v.d[k] * DM, i * 36 + 24 + k * 4);
    }
  });
  b.parts.push(bind);
  b.f32s(g.ref.flat());
  const sk = Buffer.alloc(g.skin.length * 12);
  g.skin.forEach((s, i) => {
    for (let k = 0; k < 4; k++) { sk[i * 12 + k] = s.joints[k]; sk.writeUInt16LE(s.weights[k], i * 12 + 4 + k * 2); }
  });
  b.parts.push(sk);
  const dr = Buffer.alloc(g.drawn.length * 12);
  g.drawn.forEach(([vi, u, v], i) => { dr.writeUInt32LE(vi, i * 12); dr.writeFloatLE(u, i * 12 + 4); dr.writeFloatLE(v, i * 12 + 8); });
  b.parts.push(dr);
  const ix = Buffer.alloc(g.idx.length * 4);
  g.idx.forEach((x, i) => ix.writeUInt32LE(x, i * 4));
  b.parts.push(ix);
  const hd = Buffer.alloc(g.clo.hidden.length * 4);
  g.clo.hidden.forEach((x, i) => hd.writeUInt32LE(x, i * 4));
  b.parts.push(hd);
  return b.bytes();
}

/** The cover regions a garment's delete_verts reach: every region with a vertex it hides (the hiding itself is exactly those vertices). */
function suggestedCovers(g, shared) {
  const count = new Map();
  const hit = new Map();
  shared.regions.forEach((r) => count.set(r, (count.get(r) || 0) + 1));
  for (const vi of g.clo.hidden) { const r = shared.regions[vi]; if (r !== undefined && r !== 255) hit.set(r, (hit.get(r) || 0) + 1); }
  return [...hit.entries()].filter(([r, n]) => n > 0 && count.get(r) > 0).map(([r]) => body.COVER_REGIONS[r]).sort((a, b) => body.COVER_REGIONS.indexOf(a) - body.COVER_REGIONS.indexOf(b));
}

// ── The data files ────────────────────────────────────────────────────────
const ron = (s) => JSON.stringify(s);
const GENERATED = '// GENERATED by scripts/gen-makehuman-look.js from MakeHuman\'s CC0 data: never edit by hand, change the\n// converter and run it again (docs/design/character-animation.md section 16).\n';

function detailsRon(sliders) {
  const out = [GENERATED, '(', '    file: "assets/models/characters/makehuman/makehuman_details.bin",', `    groups: ${ron(GROUP_ORDER)},`, '    sliders: ['];
  for (const s of sliders) {
    out.push(`        (id: ${ron(s.id)}, group: ${ron(s.group)}, low: ${ron(s.low)}, high: ${ron(s.high)}, ends: (${ron(s.ends[0])}, ${ron(s.ends[1])})${s.bodies.length ? `, bodies: ${ron(s.bodies)}` : ''}),`);
  }
  out.push('    ],', ')', '');
  return out.join('\n');
}

const fx = (x) => (Number.isInteger(x) ? `${x}.0` : String(x));
function garmentsRon(garments) {
  const out = [GENERATED, '(', '    garments: ['];
  const dir = (g) => `assets/models/characters/makehuman/garments/${g.id}`;
  for (const g of garments) {
    out.push('        (');
    out.push(`            id: ${ron(g.id)}, kind: ${g.kind[0].toUpperCase() + g.kind.slice(1)}, file: ${ron(`${dir(g)}/${g.id}.bin`)},`);
    out.push(`            texture: ${ron(`${dir(g)}/${g.id}.dds`)}, texture_px: ${g.tex.size}, normal_map: ${g.tex.hasNormal}, luma: ${fx(g.tex.meanLuma)},`);
    out.push(`            colour: (${g.colour.mean.map(fx).join(', ')}), roughness: ${fx(g.roughness)}, alpha: ${g.alpha}, two_sided: ${g.twoSided},`);
    out.push(`            covers: ${ron(g.covers)}, tags: ${ron(g.tags)},`);
    out.push('        ),');
  }
  out.push('    ],', ')', '');
  return out.join('\n');
}

// Each garment's words on the page (the wardrobe and the hair, brows and lashes choosers).
function garmentWords(g) {
  const n = g.id.match(/(\d+)$/);
  const num = n ? ` ${+n[1]}` : '';
  const base = g.id.replace(/\d+$/, '').replace(/_+$/, '');
  const named = {
    female_casualsuit: 'Casual outfit', male_casualsuit: 'Casual outfit', female_elegantsuit: 'Dress-up outfit', male_elegantsuit: 'Suit',
    female_sportsuit: 'Sports outfit', male_worksuit: 'Work overalls', shoes: 'Shoes', fedora: 'Fedora', fedora_cocked: 'Tilted fedora',
    afro: 'Afro', bob: 'Bob', braid: 'Braid', long: 'Long', ponytail: 'Ponytail', short: 'Short', eyebrow: 'Brows', eyelashes: 'Lashes',
  }[base];
  if (!named) fail(`garment ${g.id} has no words`);
  return g.id === 'fedora_cocked' ? named : `${named}${num}`;
}

// ── Main ─────────────────────────────────────────────────────────────────
function argValue(flag) {
  const i = process.argv.indexOf(flag);
  return i >= 0 ? process.argv[i + 1] : null;
}

function noteText(files, inputs, sliders, garments, totals) {
  const L = [
    '# MakeHuman look: where it comes from',
    '',
    'Made by `scripts/gen-makehuman-look.js` (HumanityOS, character animation, the look, 2026-11-09) from',
    'MakeHuman\'s data files: the face and body detail targets, the slider list, and the system asset',
    'pack\'s clothes, shoes, hats, hair, brows and lashes. Do not edit these files by hand: change the',
    'converter and run it again. The bodies themselves: `SOURCE.md`.',
    '',
    '## Licence',
    '',
    'Dedicated to the public domain under **CC0 1.0 Universal**',
    '(https://creativecommons.org/publicdomain/zero/1.0/). MakeHuman\'s `LICENSE.md`, section C: "These',
    'assets have been released under CC0 1.0 Universal." Every clothes, hair and material file of the',
    'asset pack opens with: "This asset was explicitly released as CC0 in september 2020.", and the',
    'pack\'s own manifest (`packs/makehuman_system_assets.json`) gives `"license": "CC0"` for all 94',
    'assets. MakeHuman\'s source code is AGPL and none of it is used: the converter reads the files\'',
    'formats itself. Finding: `docs/reference/findings/2026-11-08-character-assets.md` (its 2026-11-09',
    'addenda: the asset pack, and the detail targets).',
    '',
    'CC0 1.0 Universal, the operative words of its section 2: "To the greatest extent permitted by, but',
    'not in contravention of, applicable law, Affirmer hereby overtly, fully, permanently, irrevocably and',
    'unconditionally waives, abandons, and surrenders all of Affirmer\'s Copyright and Related Rights and',
    'associated claims and causes of action, whether now known or unknown (including existing as well as',
    'future claims and causes of action), in the Work". The full text:',
    'https://creativecommons.org/publicdomain/zero/1.0/legalcode',
    '',
    '## Sources',
    '',
    `- Detail targets: https://github.com/makehumancommunity/makehuman, commit \`a8bc2d54ff0ac92e78ff71431b1023eda42bf482\` (2024-06-26), fetched 2026-11-09, approved by the operator ("Yes."); \`MANIFEST.json\` SHA-256 \`${inputs.detailManifest}\`, each target checked against its git blob SHA-1 there (${inputs.targets} targets read).`,
    `- The slider list: \`${MODIFIERS.rel}\`, git blob \`${MODIFIERS.sha}\` (the same commit).`,
    `- Asset pack: \`${PACK_ZIP}\` from https://files.makehumancommunity.org/asset_packs/makehuman_system_assets/, SHA-256 \`${PACK_SHA256}\`, fetched 2026-11-09, approved by the operator ("Let's do it."); each file read checked against the CRC-32 the zip records. The files read, with their SHA-256:`,
    '',
    ...inputs.pack.map(([rel, h]) => `  - \`${rel}\` \`${h}\``),
    '',
    '## What was made',
    '',
    `- \`makehuman_details.bin\`: ${inputs.targets} detail targets driven by ${sliders.length} sliders (\`data/characters/details.ron\`).`,
    `- ${garments.length} garments (\`data/characters/garments.ron\`): ${['clothes', 'shoes', 'hat', 'hair', 'brows', 'lashes'].map((k) => `${garments.filter((g) => g.kind === k).length} ${k}`).join(', ')}; ${totals.files} files, ${(totals.bytes / 1048576).toFixed(1)} MB in all.`,
    `- Each garment's texture as the game draws it (\`<id>.dds\`, ${(totals.dds / 1048576).toFixed(1)} MB for the ${garments.length}): the colour (times its ambient occlusion map) beside the normal map, every level down to 4 texels a half, each half at most ${garmentTexture.TEXTURE_PX} texels, BC7 (sRGB) by our own encoder (\`scripts/lib/bc7.js\`; the scan: \`docs/reference/research/2026-11-10-bc7-encoders/\`). The source PNGs beside them stay whole. Level 0 against the packed source, peak signal-to-noise ratio (dB; higher is closer, about 40 and over is hard to tell apart; a card's colour where its alpha is at least a half, where it shows):`,
    '',
    ...garments.map((g) => `  - \`${g.id}\`: colour ${g.tex.psnr.colour}, normal map ${g.tex.hasNormal ? g.tex.psnr.normal : '(flat)'}${g.tex.psnr.alpha === null ? '' : `, alpha ${g.tex.psnr.alpha}`}`),
    '',
    '## Output hashes (SHA-256)',
    '',
    '`node scripts/gen-makehuman-look.js --check` compares the files with these when the sources are not',
    'on the machine.',
    '',
    ...files.map(([p, bytes]) => `- \`${path.relative(ROOT, p).replace(/\\/g, '/')}\` \`${sha256(bytes)}\``),
    '',
  ];
  return L.join('\n');
}

async function main() {
  const check = process.argv.includes('--check');
  const wordsOut = argValue('--words');
  const haveSources = fs.existsSync(path.join(DETAIL_SRC, 'MANIFEST.json')) && fs.existsSync(path.join(PACK_SRC, PACK_ZIP)) && fs.existsSync(path.join(body.DEFAULT_SRC, '3dobjs', 'base.obj'));
  if (!haveSources) {
    if (!check) fail(`the MakeHuman sources are not on this machine (${DETAIL_SRC}, ${PACK_SRC}, ${body.DEFAULT_SRC})`);
    const text = fs.existsSync(NOTE) ? fs.readFileSync(NOTE, 'utf8').replace(/\r\n/g, '\n') : '';
    const want = [...text.matchAll(/^- `([^`]+)` `([0-9a-f]{64})`$/gm)];
    if (!want.length) fail(`${path.relative(ROOT, NOTE)} records no output hashes`);
    let bad = 0;
    for (const [, rel, hash] of want) {
      const p = path.join(ROOT, rel);
      let bytes = fs.existsSync(p) ? fs.readFileSync(p) : null;
      if (bytes && /\.(ron|md)$/.test(rel)) bytes = Buffer.from(bytes.toString().replace(/\r\n/g, '\n'));
      const got = bytes ? sha256(bytes) : 'missing';
      if (got !== hash) { console.error(`${rel}: ${got}, LOOK-SOURCE.md records ${hash}`); bad++; }
    }
    if (bad) process.exit(1);
    console.log(`the MakeHuman sources are not on this machine: checked the ${want.length} committed files against LOOK-SOURCE.md's hashes only`);
    return;
  }
  const { shared } = body.build(body.DEFAULT_SRC);
  // The slider list, checked.
  const modBuf = fs.readFileSync(path.join(body.DEFAULT_SRC, ...MODIFIERS.rel.split('/')));
  if (body.gitBlobSha1(modBuf) !== MODIFIERS.sha) fail(`${MODIFIERS.rel} differs from git blob ${MODIFIERS.sha}`);
  const mods = JSON.parse(modBuf.toString('utf8'));
  const manifest = detailManifest(DETAIL_SRC);
  const used = new Map();
  const { sliders, targets } = makeSliders(DETAIL_SRC, mods, manifest, used);
  const files = [];
  files.push([path.join(OUT, 'makehuman_details.bin'), writeDetails(targets, shared.raw.length)]);
  files.push([DETAILS_RON, Buffer.from(detailsRon(sliders))]);
  // The garments.
  const pack = packReader(PACK_SRC);
  // Each helper vertex's nearest body vertex on the raw base mesh (whose weights it takes, blendSkin).
  const BODY = 13380;
  const nearest = new Map();
  for (let h = BODY; h < shared.raw.length; h++) {
    let best = 0, bd = Infinity;
    const p = shared.raw[h];
    for (let v = 0; v < BODY; v++) {
      const q = shared.raw[v];
      const d = (p[0] - q[0]) ** 2 + (p[1] - q[1]) ** 2 + (p[2] - q[2]) ** 2;
      if (d < bd) { bd = d; best = v; }
    }
    nearest.set(h, best);
  }
  shared.skinOf = (vi) => (vi >= BODY ? nearest.get(vi) : vi);
  const garments = garmentList(pack).map((g) => convertGarment(g, pack, shared));
  // Each garment's texture, packed and BC7-encoded, on worker threads (each garment on one, in the
  // garments' order whatever order they finish in, so the output is the same every run). Four
  // threads at most: the operator keeps a core of his own (CLAUDE.md, his share of his PC).
  const threads = Math.max(1, Math.min(+(process.env.HUMANITY_LOOK_THREADS || 4), os.cpus().length - 2));
  const t0 = Date.now();
  const packed = await garmentTexture.packAll(garments.map((g) => ({ id: g.id, diffuse: pack.read(g.diffuse), normal: g.normal ? pack.read(g.normal) : null, ao: g.ao ? pack.read(g.ao) : null, cards: g.alpha })), threads);
  console.log(`packed and BC7-encoded ${garments.length} garment textures on ${threads} threads in ${((Date.now() - t0) / 1000).toFixed(1)} s`);
  let assetBytes = 0;
  let assetFiles = 0;
  let ddsBytes = 0;
  garments.forEach((g, k) => {
    g.tex = packed[k];
    g.covers = suggestedCovers(g, shared);
    g.words = garmentWords(g);
    const dir = path.join(GARMENT_DIR, g.id);
    const add = (p, bytes) => { files.push([p, bytes]); assetBytes += bytes.length; assetFiles++; };
    add(path.join(dir, `${g.id}.bin`), writeGarment(g));
    for (const rel of g.files) add(path.join(dir, path.posix.basename(rel)), pack.read(rel));
    if (g.thumb) add(path.join(dir, `${g.id}.thumb.png`), pack.read(g.thumb));
    add(path.join(dir, `${g.id}.dds`), g.tex.dds);
    ddsBytes += g.tex.dds.length;
  });
  files.push([GARMENTS_RON, Buffer.from(garmentsRon(garments))]);
  const detailsBytes = files[0][1].length;
  const inputs = { detailManifest: manifest.sha, targets: targets.size, pack: pack.used() };
  files.push([NOTE, Buffer.from(noteText(files, inputs, sliders, garments, { files: assetFiles, bytes: assetBytes + detailsBytes, dds: ddsBytes }))]);
  for (const [p, bytes] of files) if (bytes.length >= 50 * 1024 * 1024) fail(`${p} is ${bytes.length} bytes, over the 50 MB a file in git may be`);
  if (wordsOut) fs.writeFileSync(wordsOut, JSON.stringify(wordsFragment(sliders, garments), null, 1));
  if (check) {
    const bad = [];
    for (const [p, bytes] of files) {
      let have = fs.existsSync(p) ? fs.readFileSync(p) : null;
      if (have && /\.(ron|md)$/.test(p)) have = Buffer.from(have.toString().replace(/\r\n/g, '\n'));
      if (!have || !have.equals(bytes)) bad.push(path.relative(ROOT, p));
    }
    for (const p of bad) console.error(`${p} differs from what scripts/gen-makehuman-look.js makes`);
    if (bad.length) process.exit(1);
    console.log(`the MakeHuman look matches the converter (${files.length} files)`);
    return;
  }
  for (const [p, bytes] of files) {
    fs.mkdirSync(path.dirname(p), { recursive: true });
    fs.writeFileSync(p, bytes);
  }
  const total = files.reduce((s, [, b]) => s + b.length, 0);
  console.log(`wrote ${files.length} files, ${(total / 1048576).toFixed(1)} MB: ${sliders.length} sliders over ${targets.size} targets, ${garments.length} garments`);
  for (const g of garments) console.log(`  ${g.kind.padEnd(7)} ${g.id.padEnd(22)} ${String(g.clo.verts.length).padStart(5)} verts ${String(g.idx.length / 3).padStart(6)} tris  hides ${String(g.clo.hidden.length).padStart(5)}  covers ${g.covers.join(';') || '-'}`);
}

if (require.main === module) {
  main().catch((e) => {
    console.error(e.message);
    process.exit(1);
  });
}
module.exports = { parseMhclo, parseObj, decodePng, sliderId };
