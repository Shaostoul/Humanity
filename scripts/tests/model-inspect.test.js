// The model inspector's twin on the website and the command line (web/shared/model-inspect.js and
// scripts/inspect-model.js, 2026-11-14; docs/design/model-inspector.md). The desktop's original is
// src/gui/model_view/inspect.rs; both read the planted models made by
// scripts/gen-inspect-fixtures.js and must find the same faults with the same codes.
//
// Run: node --test scripts/tests/model-inspect.test.js
//
// What it proves:
//  1. The planted model's faults are all found and its good parts pass: the muzzle pointing into
//     the model, the empty with no name, the empty no convention names, the door too small, the
//     tilted mount, the physics hull 1 m off the model; the level seat is not a finding; three
//     planted catalog rows are judged off, turned and fitting.
//  2. A model exported in centimetres is called out, and a second part the game would drop is said.
//  3. A .gltf reads its own outside files (never above its folder), and a missing picture is named.
//  4. The words are en.json's, filled (no key and no {{placeholder}} left).
//  5. Every catalog box fits its fitted model (turned by its model_yaw), or is a known exception
//     with its reason (data/models/catalog_fit_exceptions.json); a row whose model is in a download
//     pack not installed here is named with its pack. Failing since the furniture fixes
//     (2026-11-15), as the desktop's every_catalog_box_fits_its_fitted_model fails.
//
// HOS_WEB_DIR points the test at another copy of web/ (used to see each test red).
// Seen red 2026-11-14, each with one break made in a copy of web/shared/model-inspect.js under
// HOS_WEB_DIR, then green again on the real web/: see the note above each test.
'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const MI = require(path.join(WEB, 'shared', 'model-inspect.js'));
const CLI = require(path.join(ROOT, 'scripts', 'inspect-model.js'));
const FIX = path.join(__dirname, 'fixtures', 'inspect');
const CONV = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'models', 'inspect_conventions.json'), 'utf8'));
const WORDS = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'i18n', 'en.json'), 'utf8')).model_inspect.finding;

function run(rel, catalog) {
  const file = path.join(FIX, rel);
  const phys = path.join(path.dirname(file), 'physics.ron');
  return MI.inspect(path.basename(file), fs.readFileSync(file), {
    conventions: CONV,
    physics: fs.existsSync(phys) ? fs.readFileSync(phys, 'utf8') : undefined,
    catalog: catalog || [],
    readOutside: CLI.outsideReader(path.dirname(file)),
  });
}
const codes = (i) => i.findings.map((f) => f.code);
const named = (i, code) => i.findings.filter((f) => f.code === code).map((f) => f.vars.name);
const row = (id, size) => ({ file: 'test.ron', id, label: id, shape: 'box', size });

// Seen red with the outward test flipped (`dot(...) > 0`): muzzle_01 no longer found; and with the
// collision union left at the named mesh alone: no collision_off.
test('the planted model: every fault found, the good parts pass', () => {
  const i = run('planted/planted.glb', [row('planted_wide', [2.0, 0.5, 1.0]), row('planted_turned', [0.8, 0.5, 1.0]), row('planted_fits', [1.0, 0.5, 0.8])]);
  assert.strictEqual(i.nodes.length, 9);
  assert.deepStrictEqual(i.nodes.map((n) => n.kind), ['group', 'mesh', 'collision', 'empty', 'empty', 'empty', 'empty', 'empty', 'empty']);
  assert.deepStrictEqual(i.nodes.filter((n) => n.kind === 'empty').map((n) => n.convention), ['muzzle', 'seat', null, null, 'door', 'mount']);
  assert.deepStrictEqual(i.min.map((x) => +x.toFixed(5)), [-0.5, 0, -0.4]);
  assert.deepStrictEqual(i.max.map((x) => +x.toFixed(5)), [0.5, 0.5, 0.4]);
  assert.deepStrictEqual(named(i, 'empty_wrong_way'), ['muzzle_01']);
  assert.deepStrictEqual(named(i, 'empty_unknown'), ['Empty.003']);
  assert.deepStrictEqual(named(i, 'empty_size'), ['door_a']);
  assert.deepStrictEqual(named(i, 'empty_not_level'), ['mount_top']);
  assert.ok(codes(i).includes('empty_unnamed'));
  assert.ok(!i.findings.some((f) => Object.values(f.vars).includes('seat_main')), 'the level seat is fine');
  assert.strictEqual(i.colliders.length, 2);
  assert.strictEqual(i.findings.find((f) => f.code === 'collision_off').vars.gap, '1.00');
  const verdict = (id) => i.catalog.find((f) => f.row.id === id).verdict;
  assert.strictEqual(verdict('planted_wide'), 'off');
  assert.strictEqual(verdict('planted_turned'), 'turned');
  assert.strictEqual(verdict('planted_fits'), 'fits');
  assert.ok(codes(i).includes('pivot_base_centred') && !codes(i).includes('engine_differs') && !codes(i).includes('scale_huge'));
  assert.strictEqual(i.findings[0].level, 'problem', 'problems first');
});

// Seen red with `scale.tallest_m` read as 500: no scale_huge for the 180 m chair.
test('a model in centimetres and a part the game drops are found', () => {
  const i = run('huge/huge.glb');
  const huge = i.findings.find((f) => f.code === 'scale_huge');
  assert.ok(huge, codes(i).join(', '));
  assert.strictEqual(huge.vars.cm, '1.80');
  const differs = i.findings.find((f) => f.code === 'engine_differs');
  assert.ok(differs && differs.level === 'check');
  assert.strictEqual(i.parts, 2);
});

// Seen red with `imageSize` answering [1, 1] for any bytes, null included: no texture_missing.
test('a .gltf reads its own files and names a missing picture', () => {
  const i = run('missing/missing.gltf');
  const miss = i.findings.find((f) => f.code === 'texture_missing');
  assert.ok(miss, codes(i).join(', '));
  assert.strictEqual(miss.vars.file, 'missing_palette.png');
  const read = CLI.outsideReader(path.join(FIX, 'missing'));
  assert.ok(read('missing.bin'));
  assert.strictEqual(read('../planted/physics.ron'), null, 'never above its folder');
  assert.strictEqual(read('..%2Fplanted%2Fphysics.ron'), null, 'not even spelled with escapes');
  assert.strictEqual(read(path.join(FIX, 'planted', 'physics.ron')), null, 'never an absolute path');
  assert.strictEqual(read('https://example.com/x.bin'), null, 'never an address');
  // Without a reader (the website) the same file says its buffer is not beside it.
  const bare = MI.inspect('missing.gltf', fs.readFileSync(path.join(FIX, 'missing', 'missing.gltf')), { conventions: CONV });
  assert.deepStrictEqual(codes(bare), ['unreadable']);
});

test('every finding has its words from en.json, filled', () => {
  for (const rel of ['planted/planted.glb', 'huge/huge.glb', 'missing/missing.gltf']) {
    for (const f of run(rel, [row('r', [2, 0.5, 1])]).findings) {
      const w = MI.wordsOf(f, WORDS);
      assert.ok(!w.startsWith('model_inspect.') && !w.includes('{{'), `${f.code}: ${w}`);
    }
  }
});

// THE CATALOG CHECK (report mode 2026-11-14, failing since the furniture fixes of 2026-11-15). The
// same rule as `node scripts/inspect-model.js --catalog` (which exits 1 on an unexcused row) and the
// desktop's `every_catalog_box_fits_its_fitted_model`. Seen red 2026-11-15 with the wardrobe's box
// back at 1.20 m wide: "cabin.ron wardrobe: box 1.2 x 0.59 m, fitted 0.941 x 0.588 m (off)" (and
// `--catalog` exited 1).
test('every catalog box fits its fitted model, or is a known exception', () => {
  const r = CLI.catalogReport();
  for (const m of r.missing) console.log(`  ${m.words}`);
  for (const m of r.exceptions) console.log(`  known: ${m.file} ${m.id}: ${m.reason}`);
  assert.ok(r.checked >= 10, `only ${r.checked} catalog models were checked`);
  assert.ok(r.missing.every((m) => m.pack), `a missing model no download pack holds: ${r.missing.filter((m) => !m.pack).map((m) => m.model).join(', ')}`);
  assert.ok(r.exceptions.every((m) => m.reason && m.reason.trim()), 'every known exception says why');
  assert.deepStrictEqual(r.mismatches.map((m) => `${m.file} ${m.id}: box ${m.box_m[0]} x ${m.box_m[1]} m, fitted ${m.fitted_m[0]} x ${m.fitted_m[2]} m (${m.verdict})`), [], `catalog box(es) do not fit their fitted models`);
  for (const e of CLI.fitExceptions()) assert.ok(r.exceptions.some((m) => m.file === e.file && m.id === e.id), `the exception ${e.file} ${e.id} names no row whose box is off: take it out of data/models/catalog_fit_exceptions.json`);
});

// A ROW TURNED INSIDE ITS BOX (model_yaw, 2026-11-15): the double bed's model lies along its file's
// +Z; its row (a box long along x) fits only with the quarter turn, and the turn is glam's sense,
// so its headboard (the tall part, at the file's far +Z) lands at the box's -X end. Seen red with the
// turn left out of the fit (`turnedBounds(engine.positions, 0)`): "catalog_turned".
test("a model turned by its row's model_yaw fits its box, its head where the turn puts it", () => {
  const bed = [path.join(ROOT, 'assets', 'models', 'furniture', 'beddouble', 'beddouble.gltf')];
  const turned = Object.assign(row('bed', [2.04, 0.68, 1.73]), { model_yaw: 270 });
  const insp = CLI.inspectFile(bed[0], [turned, row('bed_unturned', [2.04, 0.68, 1.73])]);
  assert.deepStrictEqual(insp.catalog.map((f) => f.verdict), ['fits', 'turned']);
  const [mn, mx] = MI.turnedBounds([[0, 0, 0], [0, 1, 1.125]], 270);
  assert.deepStrictEqual([mn[0], mx[0], mn[2], mx[2]], [-1.125, 0, 0, 0], "the file's +Z end turns to -X");
});

// THE WEBSITE'S INSPECT MODE (web/shared/model-view.js with model-inspect.js loaded): once the
// model is read, Inspect sits under the view; pressed, its panel shows the report in en.json's words
// (the planted muzzle named), the node tree with each empty selectable, and Hide the inspector takes
// it away. Without model-inspect.js there is no Inspect at all.
// Seen red 2026-11-14 with the inspection left out of model-view.js's start() (`if (MI)` made
// `if (false)`): "no Inspect button shown" (the bar stays hidden).
test('the website viewer: Inspect shows the report and the node tree', async () => {
  const MV = require(path.join(WEB, 'shared', 'model-view.js'));
  const { fakeDocument } = require('./files-web-harness.js');
  const en = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'i18n', 'en.json'), 'utf8'));
  const before = globalThis.i18n;
  globalThis.i18n = { t: (key) => key.split('.').reduce((o, k) => (o && typeof o === 'object' ? o[k] : undefined), en) || key };
  try {
    const doc = fakeDocument();
    const box = doc.createElement('div');
    doc.body.appendChild(box);
    const renderer = () => ({ prepare: async () => {}, draw: () => true, free: () => {}, destroy: () => {} });
    const bytes = fs.readFileSync(path.join(FIX, 'planted', 'planted.glb'));
    const view = MV.open(box, { name: 'planted.glb', size: bytes.length, renderer, inspector: MI, conventions: CONV, fetchBytes: async () => new Uint8Array(bytes) });
    await view.done;
    const button = box.querySelector('#model-view-inspect');
    const bar = box.querySelector('.model-view-inspect-bar');
    assert.ok(button && bar.style.display === 'flex', 'no Inspect button shown');
    assert.strictEqual(button.textContent, 'Inspect');
    assert.ok(view.inspection(), 'the model was inspected');
    button.click();
    const panel = box.querySelector('.model-view-inspect');
    assert.strictEqual(panel.style.display, 'block');
    const shown = panel.textContent;
    assert.ok(shown.includes('The muzzle empty muzzle_01 points into the model'), shown.slice(0, 300));
    assert.ok(shown.includes('muzzle_01 (empty: muzzle)') && shown.includes('(no name) (empty: no convention)'), 'the node tree');
    assert.strictEqual(button.textContent, 'Hide the inspector');
    const nodes = box.querySelectorAll('.model-view-node');
    assert.strictEqual(nodes.length, 9);
    nodes[3].click();
    assert.ok(box.querySelectorAll('.model-view-node')[3].className.includes('selected'), 'an empty is selected');
    box.querySelector('#model-view-inspect').click();
    assert.strictEqual(panel.style.display, 'none');
    view.close();
    // Without the inspector, no Inspect.
    const plain = doc.createElement('div');
    doc.body.appendChild(plain);
    const v2 = MV.open(plain, { name: 'planted.glb', size: bytes.length, renderer, fetchBytes: async () => new Uint8Array(bytes) });
    await v2.done;
    assert.notStrictEqual(plain.querySelector('.model-view-inspect-bar').style.display, 'flex', 'no Inspect without the inspector');
    v2.close();
  } finally {
    globalThis.i18n = before;
  }
});
