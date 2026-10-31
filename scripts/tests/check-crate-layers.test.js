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
    ['systems/farming/heat_tests.rs', new Map([['humanity-systems -> humanity-engine', 5]])],
    ['systems/wounds_tests.rs', new Map([['humanity-systems -> humanity-engine', 1]])],
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
    assert.deepStrictEqual([...a.testByFile.get('low.rs')], [['low -> top', 1]]);
    assert.deepStrictEqual(a.testList.map((r) => `${r.rel}:${r.line} ${r.item}`), ['low.rs:3 top::go']);
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
    assert.deepStrictEqual([...a.codeByFile.get('low.rs')], [['low -> top', 3]]);
    assert.ok(!a.codeByFile.has('top.rs'), 'the crate that owns lib.rs reads its own root freely');
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
