// The crate split's ratchet (scripts/check-crate-layers.js): over a budget or a new wrong-way edge
// fails; under a budget asks for the budget to come down. Run: node --test scripts/tests/
const test = require('node:test');
const assert = require('node:assert');
const { judge } = require('../check-crate-layers.js');

test('an edge over its budget, or a new edge, fails', () => {
  const r = judge({ 'a -> b': 5, 'c -> d': 1 }, { 'a -> b': 4 });
  assert.deepStrictEqual(r.over, [{ edge: 'a -> b', n: 5, budget: 4 }]);
  assert.deepStrictEqual(r.fresh, [{ edge: 'c -> d', n: 1 }]);
});

test('an edge under its budget, or gone, asks for the budget to come down', () => {
  const r = judge({ 'a -> b': 3 }, { 'a -> b': 4, 'e -> f': 2 });
  assert.strictEqual(r.over.length + r.fresh.length, 0);
  assert.deepStrictEqual(r.under, [{ edge: 'a -> b', n: 3, budget: 4 }, { edge: 'e -> f', n: 0, budget: 2 }]);
});

test('at budget, nothing to say', () => {
  const r = judge({ 'a -> b': 4 }, { 'a -> b': 4 });
  assert.deepStrictEqual(r, { over: [], fresh: [], under: [] });
});

// The reading the ratchet counts with (scripts/crate-graph.js, 2026-10-31): test code is every item
// gated to tests alone, wherever it sits, and a path in a comment or a string is not a reference.
const { blankLiterals, cfgIsTestOnly, nonTestCode, declaredFiles } = require('../crate-graph.js');

test('comments and literals are blanked in place: same length, same lines, no paths left', () => {
  const src = 'let a = "crate::x"; // crate::y\r\n/* crate::w */ use crate::z; let c = \'"\'; let r = r#"a "crate::v" b"#;\n';
  const out = blankLiterals(src);
  assert.strictEqual(out.length, src.length);
  assert.strictEqual(out.split('\n').length, src.split('\n').length);
  for (const gone of ['crate::x', 'crate::y', 'crate::w', 'crate::v']) assert.ok(!out.includes(gone), gone);
  assert.ok(out.includes('use crate::z;'));
  // A lifetime is not a char literal: the code after it is read.
  assert.ok(blankLiterals("fn f<'a>(x: &'a str) { crate::k(); }").includes('crate::k'));
});

test('only a test-only cfg gates test code', () => {
  assert.ok(cfgIsTestOnly('test'));
  assert.ok(cfgIsTestOnly('all(test,feature="native")'));
  assert.ok(cfgIsTestOnly('all(feature="native",test)'));
  assert.ok(!cfgIsTestOnly('any(test,feature="native")'));
  assert.ok(!cfgIsTestOnly('not(test)'));
  assert.ok(!cfgIsTestOnly('feature="native"'));
});

test('a test item in the middle of a file goes, and the code after it stays', () => {
  const src = [
    'use crate::a;',
    '#[cfg(test)]',
    'mod sowing_tests;',
    '#[cfg(test)]',
    '#[path = "falls_tests.rs"]',
    'mod tests;',
    '#[cfg(all(test, feature = "native"))]',
    'mod native_tests { use crate::engine::x; fn f() { let c = \'{\'; } }',
    'fn after() { crate::b(); }',
    '#[cfg(test)]',
    'mod t {',
    '    use crate::c;',
    '}',
    '#[cfg(not(test))]',
    'fn live() { crate::d(); }',
  ].join('\n');
  const { code, testMods } = nonTestCode(src);
  assert.strictEqual(code.length, src.length);
  for (const kept of ['crate::a', 'crate::b', 'crate::d', 'fn after', 'fn live']) assert.ok(code.includes(kept), kept);
  for (const gone of ['crate::engine', 'crate::c', 'sowing_tests']) assert.ok(!code.includes(gone), gone);
  assert.deepStrictEqual(testMods, [{ name: 'sowing_tests', path: null }, { name: 'tests', path: 'falls_tests.rs' }]);
});

// The readers the crate cards added (2026-10-31, scripts/crate-cards.js).
const { declaredGates, innerGates, pathDecls, testOnlyCode } = require('../crate-graph.js');

test('a module\'s gates are the cfg attributes on its mod line, never a cfg named in a comment above it', () => {
  const lib = [
    '/// Engine-wide on purpose, NOT `#[cfg(native)]`, so every view reads one source.',
    'pub mod cosmos;',
    '/// The play journal.',
    '#[cfg(feature = "native")]',
    'pub mod play_journal;',
    '#[cfg(test)]',
    'pub(crate) mod test_clock;',
    '',
    '#[cfg(feature = "relay")]',
    'pub mod relay;',
    'pub mod hot_reload;',
    '#[cfg(test)] #[path = "x_tests.rs"] mod same_line;',
  ].join('\r\n');
  const g = declaredGates(lib);
  assert.deepStrictEqual(g.get('cosmos'), []);
  assert.deepStrictEqual(g.get('play_journal'), ['feature = "native"']);
  assert.deepStrictEqual(g.get('test_clock'), ['test']);
  assert.deepStrictEqual(g.get('relay'), ['feature = "relay"']);
  assert.deepStrictEqual(g.get('hot_reload'), []);
  assert.deepStrictEqual(g.get('same_line'), ['test']);
  assert.deepStrictEqual(innerGates('//! Signed releases.\n#![cfg(feature = "native")]\nuse x;\n'), ['feature = "native"']);
});

test('a #[path] declaration is read from code, its path from the source, and a commented one is not', () => {
  const raw = '#[cfg(test)]\n#[path = "household_tests.rs"]\nmod tests;\n// #[path = "old.rs"] mod gone;\n#[path="a/b.rs"] pub(crate) mod b;\n';
  const { all } = nonTestCode(raw);
  assert.deepStrictEqual(pathDecls(raw, all), [{ name: 'tests', path: 'household_tests.rs' }, { name: 'b', path: 'a/b.rs' }]);
});

test('the test reading keeps only the test-gated items, in place', () => {
  const raw = 'use crate::a;\n#[cfg(test)]\nmod t { use crate::engine::x; }\nfn f() { crate::b(); }\n';
  const { code, all } = nonTestCode(raw);
  const tests = testOnlyCode(all, code);
  assert.strictEqual(tests.length, raw.length);
  assert.ok(tests.includes('crate::engine::x'));
  for (const gone of ['crate::a', 'crate::b']) assert.ok(!tests.includes(gone), gone);
});

test('a declared test module is found where rustc finds it', () => {
  assert.deepStrictEqual(declaredFiles('systems/farming/mod.rs', { name: 'heat_tests', path: null }),
    ['systems/farming/heat_tests.rs', 'systems/farming/heat_tests/mod.rs']);
  assert.deepStrictEqual(declaredFiles('gui/pages/maps.rs', { name: 'tests', path: null }),
    ['gui/pages/maps/tests.rs', 'gui/pages/maps/tests/mod.rs']);
  assert.deepStrictEqual(declaredFiles('systems/day_energy.rs', { name: 'tests', path: 'day_energy_tests.rs' }),
    ['systems/day_energy_tests.rs']);
  assert.deepStrictEqual(declaredFiles('lib.rs', { name: 'test_clock', path: null }), ['test_clock.rs', 'test_clock/mod.rs']);
});

// Test code is held file by file (2026-10-31, the crate split's backlog before rung 1).
const { flatByFile, flatBudgets, nestByFile } = require('../check-crate-layers.js');

test('a test budget belongs to its file: another file on the same edge cannot spend it', () => {
  const budgets = flatBudgets({ 'src/systems/farming/heat_tests.rs': { 'humanity-systems -> humanity-engine': 6 } });
  const counts = flatByFile(new Map([
    ['src/systems/farming/heat_tests.rs', new Map([['humanity-systems -> humanity-engine', 5]])],
    ['src/systems/wounds_tests.rs', new Map([['humanity-systems -> humanity-engine', 1]])],
  ]));
  const r = judge(counts, budgets);
  assert.deepStrictEqual(r.fresh, [{ edge: 'src/systems/wounds_tests.rs: humanity-systems -> humanity-engine', n: 1 }]);
  assert.deepStrictEqual(r.under, [{ edge: 'src/systems/farming/heat_tests.rs: humanity-systems -> humanity-engine', n: 5, budget: 6 }]);
  // And the file-by-file shape crate-layers.json keeps comes back from the flat one.
  assert.deepStrictEqual(nestByFile(counts), {
    'src/systems/farming/heat_tests.rs': { 'humanity-systems -> humanity-engine': 5 },
    'src/systems/wounds_tests.rs': { 'humanity-systems -> humanity-engine': 1 },
  });
  assert.deepStrictEqual(flatBudgets(nestByFile(counts)), counts);
});

// main.rs reaches the library by its name (2026-10-31): its uses were missing from every count.
const { cargoTargets, pathsIn, analyse } = require('../crate-graph.js');

test('Cargo.toml names the library and the binaries\' roots', () => {
  const toml = '[package]\r\nname = "humanity-engine"\r\nkeywords = ["a", "b"]\r\n\r\n[[bin]]\r\nname = "HumanityOS"\r\npath = "src/main.rs"\r\n\r\n[dependencies]\r\nname = "not-this"\r\n';
  assert.deepStrictEqual(cargoTargets(toml), { lib: 'humanity_engine', bins: ['main.rs'] });
  assert.deepStrictEqual(cargoTargets('[package]\nname = "a-b"\n[lib]\nname = "own"\n'), { lib: 'own', bins: ['main.rs'] });
});

test('a binary\'s library paths are read as the library\'s crate paths, its own crate:: is not', () => {
  const src = 'fn main() { humanity_engine::release_update::sign(); use humanity_engine::{relay::run, run as go}; crate::own::x(); my_humanity_engine::y::z(); }';
  const paths = pathsIn(src, { root: 'humanity_engine' }).map((p) => p.join('::'));
  assert.deepStrictEqual(paths, ['release_update::sign', 'relay::run', 'run']);
  // In the library the reading is the one it always was.
  assert.deepStrictEqual(pathsIn('use crate::{a::b, c}; crate::d::e(); super::f();', { parentOf: () => ['p'] }).map((p) => p.join('::')),
    ['d::e', 'a::b', 'c', 'p::f']);
});

test('the counting reads a binary through the library, and test code file by file (a tree on disk)', () => {
  const fs = require('fs');
  const os = require('os');
  const path = require('path');
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'crate-graph-'));
  try {
    const src = path.join(dir, 'src');
    fs.mkdirSync(src);
    fs.writeFileSync(path.join(src, 'lib.rs'), 'pub mod low;\npub mod top;\n');
    fs.writeFileSync(path.join(src, 'low.rs'), 'pub fn x() {}\n#[cfg(test)]\nmod tests { #[test] fn t() { crate::top::go(); } }\n');
    fs.writeFileSync(path.join(src, 'top.rs'), 'pub fn go() { crate::low::x(); }\n');
    fs.writeFileSync(path.join(src, 'main.rs'), 'fn main() { humanity_engine::top::go(); humanity_engine::low::x(); }\n');
    const plan = { crates: [
      { name: 'low', paths: ['low', 'main.rs'], uses: [] },
      { name: 'top', paths: ['top', 'lib.rs'], uses: ['low'] },
    ] };
    const a = analyse({ tests: true, src, plan, cargo: '[package]\nname = "humanity-engine"\n[[bin]]\npath = "src/main.rs"\n' });
    // main.rs is filed under `low` here, so its use of `top` points the wrong way, and is counted.
    assert.deepStrictEqual([...a.edges.get('low -> top')], [['main: top::go', 1]]);
    assert.deepStrictEqual([...a.crossing.get('top -> low')], [['low::x', 1]]);
    // The test in low.rs reaching up is held by its file.
    assert.deepStrictEqual([...a.testByFile.get('src/low.rs')], [['low -> top', 1]]);
    assert.deepStrictEqual(a.testList.map((r) => `${r.path}:${r.line} ${r.item}`), ['src/low.rs:3 top::go']);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

// The crate root's own items (2026-10-31): `crate::data_dir()` named no module, so it went uncounted
// from ~57 files below the engine; a root item is the crate's that owns lib.rs.
const { rootItems } = require('../crate-graph.js');

test('a crate root\'s items are its fns, statics, consts, types and re-exports, never its modules', () => {
  const lib = [
    'pub mod low;',
    'pub(crate) static DATA_DIR: std::sync::OnceLock<u8> = std::sync::OnceLock::new();',
    'pub(crate) use crate::low::data_dir;',
    'pub use crate::low::{a, b as bee, c::*};',
    'const MAX: f32 = 3.0;',
    'pub async fn run() {}',
    'macro_rules! say { () => {} }',
    'fn outer() {',
    '    fn inner() {}',
    '}',
  ].join('\n');
  assert.deepStrictEqual([...rootItems(lib)].sort(), ['DATA_DIR', 'MAX', 'a', 'bee', 'data_dir', 'outer', 'run', 'say']);
});

test('a path to a crate-root item, lower or upper case, is a reference into the crate owning lib.rs', () => {
  const fs = require('fs');
  const os = require('os');
  const path = require('path');
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'crate-graph-root-'));
  try {
    const src = path.join(dir, 'src');
    fs.mkdirSync(src);
    fs.writeFileSync(path.join(src, 'lib.rs'), 'pub mod low;\npub mod top;\npub(crate) static DATA_DIR: u8 = 0;\npub(crate) fn data_dir() -> u8 { DATA_DIR }\npub(crate) use crate::low::old as kept;\n');
    fs.writeFileSync(path.join(src, 'low.rs'), 'pub fn old() {}\npub fn x() -> u8 { crate::data_dir() + crate::DATA_DIR }\npub fn y() { crate::kept(); }\n');
    fs.writeFileSync(path.join(src, 'top.rs'), 'pub fn go() -> u8 { crate::data_dir() }\n');
    const plan = { crates: [
      { name: 'low', paths: ['low'], uses: [] },
      { name: 'top', paths: ['top', 'lib.rs'], uses: ['low'] },
    ] };
    const a = analyse({ src, plan, cargo: '[package]\nname = "x"\n' });
    assert.deepStrictEqual([...a.edges.get('low -> top')].sort(), [['low: DATA_DIR', 1], ['low: data_dir', 1], ['low: kept', 1]]);
    assert.deepStrictEqual([...a.codeByFile.get('src/low.rs')], [['low -> top', 3]]);
    assert.ok(!a.codeByFile.has('src/top.rs'), 'the crate that owns lib.rs reads its own root freely');
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

// A cut crate's test items are not built into a dependent's tests (2026-10-31, rung 1's first need).
const { fixtureCrossings } = require('../crate-graph.js');

test('a test reaching another crate\'s test-only module is listed; its own crate\'s is not', () => {
  const fs = require('fs');
  const os = require('os');
  const path = require('path');
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'crate-graph-fx-'));
  try {
    const src = path.join(dir, 'src');
    fs.mkdirSync(src);
    fs.writeFileSync(path.join(src, 'lib.rs'), 'pub mod low;\npub mod top;\n#[cfg(test)]\npub(crate) mod test_temp;\n');
    fs.writeFileSync(path.join(src, 'test_temp.rs'), 'pub(crate) fn dir() {}\n');
    fs.writeFileSync(path.join(src, 'low.rs'), 'pub fn x() {}\n#[cfg(test)]\npub(crate) mod tests {\n    pub(crate) fn helper() {}\n    #[test] fn own() { helper(); crate::test_temp::dir(); }\n}\n');
    fs.writeFileSync(path.join(src, 'top.rs'), 'pub fn go() {}\n#[cfg(test)]\nmod tests {\n    #[test] fn t() { crate::low::tests::helper(); crate::test_temp::dir(); crate::low::x(); }\n}\n');
    const plan = { crates: [
      { name: 'low', paths: ['low', 'test_temp'], uses: [] },
      { name: 'top', paths: ['top', 'lib.rs'], uses: ['low'] },
    ] };
    const rows = fixtureCrossings(analyse({ tests: true, src, plan, cargo: '[package]\nname = "x"\n' }));
    assert.deepStrictEqual(rows.map((r) => `${r.from} -> ${r.owner}: ${r.module} (${r.n})`).sort(), ['top -> low: low::tests (1)', 'top -> low: test_temp (1)']);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

// The cut crates (2026-11-01, rung 1): a crate the plan gives a `dir` is read from its own folder,
// named by its library's name from anywhere, and the plan must agree with the workspace's manifests.
const { manifestDeps, boundaryProblems } = require('../check-crate-layers.js');

test('a cut crate is read in its own folder and by its name; a wrong-way use between cut crates is counted', () => {
  const fs = require('fs');
  const os = require('os');
  const path = require('path');
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'crate-graph-cut-'));
  try {
    const write = (rel, text) => { fs.mkdirSync(path.dirname(path.join(dir, rel)), { recursive: true }); fs.writeFileSync(path.join(dir, rel), text); };
    write('crates/x-low/src/lib.rs', 'pub mod a;\n');
    write('crates/x-low/src/a.rs', 'pub fn f() { crate::a::g(); x_mid::b::h(); }\npub fn g() {}\n#[cfg(test)]\nmod tests { #[test] fn t() { super::g(); } }\n');
    write('crates/x-mid/src/lib.rs', 'pub mod b;\n');
    write('crates/x-mid/src/b.rs', 'pub fn h() { x_low::a::g(); crate::b::i(); }\npub fn i() {}\n');
    write('src/lib.rs', 'pub mod top;\n');
    write('src/top.rs', 'pub fn go() { x_low::a::f(); x_mid::b::h(); }\n');
    const plan = { crates: [
      { name: 'x-low', dir: 'crates/x-low', uses: [] },
      { name: 'x-mid', dir: 'crates/x-mid', uses: ['x-low'] },
      { name: 'x-top', paths: ['top', 'lib.rs'], uses: ['x-mid'] },
    ] };
    const a = analyse({ tests: true, repo: dir, plan, cargo: '[package]\nname = "x-top"\n' });
    // Each file is its folder's crate, named by its path from the repo root.
    assert.deepStrictEqual(a.files.map((f) => `${f.path} ${f.crate} ${f.mod}`).sort(), [
      'crates/x-low/src/a.rs x-low a', 'crates/x-low/src/lib.rs x-low lib',
      'crates/x-mid/src/b.rs x-mid b', 'crates/x-mid/src/lib.rs x-mid lib',
      'src/lib.rs x-top lib', 'src/top.rs x-top top',
    ]);
    // The low crate naming the middle one points the wrong way, and is counted by its file.
    assert.deepStrictEqual([...a.edges.get('x-low -> x-mid')], [['a: b::h', 1]]);
    assert.deepStrictEqual([...a.codeByFile.get('crates/x-low/src/a.rs')], [['x-low -> x-mid', 1]]);
    // Allowed uses by name are crossings, not edges; a crate's own crate:: is no crossing at all.
    assert.deepStrictEqual([...a.crossing.get('x-top -> x-low')], [['a::f', 1]]);
    assert.deepStrictEqual([...a.crossing.get('x-mid -> x-low')], [['a::g', 1]]);
    assert.ok(!a.edges.has('x-mid -> x-low') && !a.edges.has('x-top -> x-low'));
    assert.ok(!a.testByFile.size, 'the test in a.rs names only its own crate');
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('the plan must agree with the workspace: members, dirs and the dependencies between them', () => {
  const root = '[workspace]\nmembers = [".", "crates/x-low",\n  "crates/x-mid"]\n\n[package]\nname = "x-top"\n\n[dependencies]\nx-low = { path = "crates/x-low" }\nrav1d = { path = "vendor/rav1d" }\n\n[dev-dependencies]\nx-mid = { path = "crates/x-mid", features = ["test-support"] }\n';
  assert.deepStrictEqual(manifestDeps(root), {
    members: ['.', 'crates/x-low', 'crates/x-mid'],
    deps: [
      { table: 'dependencies', name: 'x-low', path: 'crates/x-low' },
      { table: 'dependencies', name: 'rav1d', path: 'vendor/rav1d' },
      { table: 'dev-dependencies', name: 'x-mid', path: 'crates/x-mid' },
    ],
  });
  const plan = { crates: [
    { name: 'x-low', dir: 'crates/x-low', uses: [] },
    { name: 'x-mid', dir: 'crates/x-mid', uses: ['x-low'] },
    { name: 'x-top', paths: ['top', 'lib.rs'], uses: ['x-mid'] },
  ] };
  const tomls = {
    'crates/x-low': '[package]\nname = "x-low"\n',
    'crates/x-mid': '[package]\nname = "x-mid"\n[dependencies]\nx-low = { path = "../x-low", default-features = false }\n',
  };
  const read = (d) => (d in tomls ? tomls[d] : null);
  assert.deepStrictEqual(boundaryProblems(plan, root, read, 'x-top'), []);
  // Red: the low crate given a dependency on the middle one, which the plan forbids.
  const bad = { ...tomls, 'crates/x-low': '[package]\nname = "x-low"\n[dependencies]\nx-mid = { path = "../x-mid" }\n' };
  assert.deepStrictEqual(boundaryProblems(plan, root, (d) => (d in bad ? bad[d] : null), 'x-top'), [
    "x-low's Cargo.toml depends on x-mid ([dependencies]), which the plan does not let x-low use",
  ]);
  // Red: the same in a dependency's own table ([dependencies.x] form), which an inline-only reader missed.
  const tableForm = { ...tomls, 'crates/x-low': '[package]\nname = "x-low"\n\n[dev-dependencies.x-mid]\npath = "../x-mid"\nfeatures = ["test-support"]\n' };
  assert.deepStrictEqual(boundaryProblems(plan, root, (d) => (d in tableForm ? tableForm[d] : null), 'x-top'), [
    "x-low's Cargo.toml depends on x-mid ([dev-dependencies]), which the plan does not let x-low use",
  ]);
  // Red: a member the plan does not know, a cut crate that is not a member, a wrong package name.
  const two = root.replace('"crates/x-low",\n  "crates/x-mid"', '"crates/x-mid", "crates/x-other"');
  assert.deepStrictEqual(boundaryProblems(plan, two, read, 'x-top'), [
    "x-low's dir, crates/x-low, is not a member of the root Cargo.toml's [workspace]",
    'the workspace member crates/x-other is not a crate the plan cut (give its plan entry "dir": "crates/x-other")',
  ]);
  const renamed = { ...tomls, 'crates/x-low': '[package]\nname = "x-lower"\n' };
  assert.deepStrictEqual(boundaryProblems(plan, root, (d) => (d in renamed ? renamed[d] : null), 'x-top'), ['crates/x-low/Cargo.toml names its package "x-lower", the plan "x-low"']);
  // Red: a cut crate that still lists paths under src/.
  const stale = { crates: plan.crates.map((c) => (c.name === 'x-low' ? { ...c, paths: ['low'] } : c)) };
  assert.deepStrictEqual(boundaryProblems(stale, root, read, 'x-top'), ['x-low is cut (dir crates/x-low) but still lists paths under src/: its files are in its dir now']);
});
