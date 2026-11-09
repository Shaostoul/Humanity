// The SimReady pack step (scripts/simready/pack.js; docs/design/simready-props.md, section 11):
// what it writes is a pure function of the bake, and what it writes is valid.
//
// From a tiny synthetic bake (two triangles, 4x4 textures, one hull) it builds an asset twice and
// compares every byte; parses the GLB (header, chunk lengths, every accessor inside the binary
// chunk, POSITION min/max present, mesh 0 the drawn model with the material, only node 0 in the
// scene); decodes the PNGs it embedded (signature, IHDR, the deflated rows inflate to the right
// size); and reads the pack archive's ustar headers (checksums, names, 512-byte blocks).
//
// RED FIRST (2026-11-09): with the GLB's asset.extras given a `built: Date.now()` field, the
// double build failed "the two builds differ: appliance_fixture.glb"; with the PNG writer's row
// filter byte dropped, the decode failed "rows inflate to 16 bytes, expected 17"; with
// applyBakedRepaints doing nothing, the moulded-letter test failed "a letter pixel takes the
// handle's colour in the base colour map".
//
//   node --test scripts/tests/simready-pack.test.js

'use strict';
const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const zlib = require('zlib');
const crypto = require('crypto');

const pack = require('../simready/pack.js');
const REPO = path.resolve(__dirname, '..', '..');

function f32(values) {
  const b = Buffer.alloc(values.length * 4);
  values.forEach((v, i) => b.writeFloatLE(v, i * 4));
  return b;
}

/** A synthetic asset: SRC with a licence (the real CC BY text, when the repo has a copy), BUILD
 *  with extract.json, bake.json and the raw files. Returns the context, or null with no licence. */
function fixture() {
  const lic = (() => {
    const root = path.join(REPO, 'assets', 'models', 'simready');
    if (!fs.existsSync(root)) return null;
    for (const d of fs.readdirSync(root)) {
      const p = path.join(root, d, 'LICENSE.txt');
      if (fs.existsSync(p)) return fs.readFileSync(p);
    }
    return null;
  })();
  if (!lic) return null;
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'hos-simready-pack-'));
  const folder = 'appliance_fixture';
  const src = path.join(tmp, 'src');
  const build = path.join(tmp, 'build');
  fs.mkdirSync(path.join(src, folder, '.metadata'), { recursive: true });
  fs.writeFileSync(path.join(src, folder, '.metadata', 'LICENSE.txt'), lic);
  const bd = path.join(build, folder, 'bake');
  fs.mkdirSync(bd, { recursive: true });
  // Two triangles in Blender's Z-up metres: a 1 x 1 m quad standing 0.5 m tall along Z.
  const corner = (x, y, z, u, v) => [x, y, z, 0, -1, 0, u, v];
  const quad = [corner(0, 0, 0, 0, 0), corner(1, 0, 0, 1, 0), corner(1, 0, 0.5, 1, 1), corner(0, 0, 0, 0, 0), corner(1, 0, 0.5, 1, 1), corner(0, 0, 0.5, 0, 1)].flat();
  fs.writeFileSync(path.join(bd, 'lod1.f32'), f32(quad));
  fs.writeFileSync(path.join(bd, 'lod2.f32'), f32(quad));
  const tex = v => f32(Array.from({ length: 4 * 4 * 4 }, (_, i) => (i % 4 === 3 ? 1 : v)));
  for (const [k, v] of [['albedo', 0.5], ['normal', 0.5], ['roughness', 0.7], ['metallic', 0], ['occlusion', 1]]) fs.writeFileSync(path.join(bd, `${k}.f32`), tex(v));
  const t = k => ({ file: `${k}.f32`, width: 4, height: 4 });
  fs.writeFileSync(path.join(bd, 'bake.json'), JSON.stringify({
    source_triangles: 2,
    meshes: { lod1: { file: 'lod1.f32', corners: 6, triangles: 2 }, lod2: { file: 'lod2.f32', corners: 6, triangles: 2 } },
    textures: { albedo: t('albedo'), normal: t('normal'), roughness: t('roughness'), metallic: t('metallic'), occlusion: t('occlusion') },
    hulls: [{ collider: '/Root/body/mesh', approximation: 'convexHull', body: '/Root/body', vertices: [[0, 0, 0], [1, 0, 0], [0, 0.5, 0], [0, 0, 1]], triangles: [[0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2]] }],
    blender: 'test',
    config: { size_class: 'small' },
  }));
  fs.writeFileSync(path.join(build, folder, 'extract.json'), JSON.stringify({
    folder, package_id: 'simready.hf.nvidia.simready-assets.appliance_fixture.2026.11.09', license: 'CC-BY-4.0',
    bounds_min_m: [0, 0, 0], bounds_max_m: [1, 0.5, 0],
    physics: { bodies: [{ path: '/Root/body', kinematic: false, mass: { mass_kg: 2.5, center_of_mass_m: [0.5, 0.25, 0] } }], colliders: [{ path: '/Root/body/mesh', body: '/Root/body', approximation: 'convexHull', mass: null, physics_material: null }], materials: {}, joints: [] },
    usd_version: 'test',
  }));
  return { tmp, asset: { folder, room: 'kitchen', row: 'new', brand_check: { verdict: 'clear' } }, ctx: { SRC: src, BUILD: build } };
}

function parseGlb(buf) {
  assert.strictEqual(buf.readUInt32LE(0), 0x46546c67, 'glTF magic');
  assert.strictEqual(buf.readUInt32LE(4), 2, 'version 2');
  assert.strictEqual(buf.readUInt32LE(8), buf.length, 'the header length is the file length');
  const jl = buf.readUInt32LE(12);
  assert.strictEqual(buf.readUInt32LE(16), 0x4e4f534a, 'JSON chunk');
  const json = JSON.parse(buf.subarray(20, 20 + jl).toString('utf8'));
  const bl = buf.readUInt32LE(20 + jl);
  assert.strictEqual(buf.readUInt32LE(24 + jl), 0x004e4942, 'BIN chunk');
  assert.strictEqual(28 + jl + bl, buf.length, 'the chunks fill the file');
  return { json, bin: buf.subarray(28 + jl, 28 + jl + bl) };
}

function inflatePng(png) {
  assert.deepStrictEqual([...png.subarray(0, 8)], [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a], 'PNG signature');
  const w = png.readUInt32BE(16), h = png.readUInt32BE(20), type = png[25];
  const channels = type === 6 ? 4 : 3;
  let off = 8;
  const idat = [];
  while (off < png.length) {
    const len = png.readUInt32BE(off);
    const kind = png.subarray(off + 4, off + 8).toString('ascii');
    if (kind === 'IDAT') idat.push(png.subarray(off + 8, off + 8 + len));
    off += 12 + len;
  }
  const raw = zlib.inflateSync(Buffer.concat(idat));
  assert.strictEqual(raw.length, h * (w * channels + 1), `rows inflate to ${raw.length / h} bytes, expected ${w * channels + 1}`);
  return { w, h, channels };
}

test('an asset builds byte for byte the same twice, as a valid GLB with valid PNGs', t => {
  const fx = fixture();
  if (!fx) { t.skip('no shipped LICENSE.txt to copy (run `just simready` first)'); return; }
  try {
    const a = pack.buildAsset(fx.asset, fx.ctx);
    const b = pack.buildAsset(fx.asset, fx.ctx);
    for (const name of Object.keys(a.files)) assert.ok(a.files[name].equals(b.files[name]), `the two builds differ: ${name}`);
    const { json, bin } = parseGlb(a.files['appliance_fixture.glb']);
    assert.deepStrictEqual(json.scenes[0].nodes, [0], 'only the drawn model is in the scene');
    assert.strictEqual(json.meshes[0].primitives[0].material, 0, 'mesh 0 is the model, with the material');
    assert.ok(json.meshes.length >= 3, 'the distance version and the hull follow it');
    for (const acc of json.accessors) {
      const v = json.bufferViews[acc.bufferView];
      assert.ok(v.byteOffset + v.byteLength <= bin.length, 'every accessor lies inside the binary chunk');
    }
    const pos = json.accessors[json.meshes[0].primitives[0].attributes.POSITION];
    assert.ok(pos.min && pos.max, 'POSITION has its bounds');
    assert.ok(Math.abs(pos.min[1]) < 1e-6, 'the base sits on y = 0');
    for (const img of json.images) {
      const v = json.bufferViews[img.bufferView];
      inflatePng(bin.subarray(v.byteOffset, v.byteOffset + v.byteLength));
    }
    const phys = a.files['physics.ron'].toString('utf8');
    assert.match(phys, /mass_kg: Some\(2\.5\)/, 'the mass is carried into physics.ron');
    assert.match(a.files['SOURCE.md'].toString('utf8'), /CC BY 4\.0 \(https:\/\/creativecommons\.org\/licenses\/by\/4\.0\/\)/);
  } finally {
    fs.rmSync(fx.tmp, { recursive: true, force: true });
  }
});

test('the pack archive is plain ustar the installer reads', () => {
  const tar = pack.tarOf({ 'pack.ron': Buffer.from('(id: "x", version: 1, instruments: [])\n'), 'a/b.glb': Buffer.alloc(700, 1) });
  assert.strictEqual(tar.length % 512, 0, 'whole 512-byte blocks');
  const header = tar.subarray(0, 512);
  const stored = parseInt(header.subarray(148, 154).toString('ascii'), 8);
  const blank = Buffer.from(header);
  blank.fill(0x20, 148, 156);
  const sum = [...blank].reduce((s, b) => s + b, 0);
  assert.strictEqual(stored, sum, 'the header checksum');
  assert.strictEqual(header.subarray(0, 8).toString('ascii'), 'a/b.glb\0', 'names in sorted order');
  assert.strictEqual(parseInt(header.subarray(124, 135).toString('ascii'), 8), 700, 'the size field');
  assert.strictEqual(crypto.createHash('sha256').update(tar).digest('hex'), crypto.createHash('sha256').update(pack.tarOf({ 'a/b.glb': Buffer.alloc(700, 1), 'pack.ron': Buffer.from('(id: "x", version: 1, instruments: [])\n') })).digest('hex'), 'the archive does not depend on the order files were given');
});

test('a name moulded into the geometry is painted out of every baked map, and nothing else is', () => {
  // An 8x8 base colour (orange handle, a 2x2 dark "letter" at 2..4) and 4x4 normal and ORM maps:
  // the letter covers one texel of each smaller map.
  const map = (w, ch, fill) => ({ px: Buffer.alloc(w * w * ch, 0).fill(Buffer.from(fill)), w, h: w, ch });
  const maps = { albedo: map(8, 4, [124, 78, 11, 255]), normal: map(4, 3, [128, 128, 255]), orm: map(4, 3, [255, 140, 0]) };
  const set = (m, x, y, v) => Buffer.from(v).copy(m.px, (y * m.w + x) * m.ch);
  const get = (m, x, y) => [...m.px.subarray((y * m.w + x) * m.ch, (y * m.w + x + 1) * m.ch)];
  for (const [x, y] of [[2, 2], [3, 2], [2, 3], [3, 3]]) set(maps.albedo, x, y, [42, 42, 42, 255]);
  set(maps.normal, 1, 1, [90, 160, 230]);
  set(maps.orm, 1, 1, [80, 200, 0]);
  // A dark pixel outside the rects (a grip) must survive.
  set(maps.albedo, 7, 7, [42, 42, 42, 255]);
  pack.applyBakedRepaints(maps, [{ stage: 'baked', rects: [[0, 0, 6, 6]], key: [42, 42, 42], tol: 40, sample: [6, 1] }]);
  assert.deepStrictEqual(get(maps.albedo, 2, 2), [124, 78, 11, 255], 'a letter pixel takes the handle\'s colour in the base colour map');
  assert.deepStrictEqual(get(maps.normal, 1, 1), [128, 128, 255], 'and its relief leaves the normal map');
  assert.deepStrictEqual(get(maps.orm, 1, 1), [255, 140, 0], 'and the ORM map');
  assert.deepStrictEqual(get(maps.albedo, 7, 7), [42, 42, 42, 255], 'a matching pixel outside the rects is kept');
  assert.deepStrictEqual(get(maps.albedo, 0, 0), [124, 78, 11, 255], 'the handle is unchanged');
});
