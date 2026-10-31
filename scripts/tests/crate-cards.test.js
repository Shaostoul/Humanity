// The crate cards' lint and generator (scripts/crate-cards.js, docs/crates/, 2026-10-31): every
// crate in the plan has a card, no card names a crate the plan does not, each card has the six
// headings in order, words under each, ONE generated block under "At a glance", and requests in the
// mailbox's one-line form; and the check never fails because the generated numbers moved.
// Run: node --test scripts/tests/crate-cards.test.js (also in `just rig-tests`).
const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const C = require('../crate-cards.js');

const PLAN = { crates: [{ name: 'humanity-core', uses: [] }, { name: 'humanity-gui', uses: ['humanity-core'] }] };
const NAMES = PLAN.crates.map((c) => c.name);

/** A sound card: the skeleton with every "(to write)" part written, and a filled block. */
function card(name, { requests = [], block = ['- **Size:** 10 lines in 1 file.'] } = {}) {
  let t = C.skeleton(name).replace(/\(to write\) /g, '');
  t = t.replace(`${C.START}\n${C.END}`, [C.START, ...block, C.END].join('\n'));
  if (requests.length) t = t.replace('None yet.', requests.join('\n'));
  return t;
}

test('a sound card passes, and so does the same card with CRLF line endings', () => {
  assert.deepStrictEqual(C.checkCard('humanity-core', card('humanity-core'), NAMES), []);
  assert.deepStrictEqual(C.checkCard('humanity-core', card('humanity-core').replace(/\n/g, '\r\n'), NAMES), []);
});

test('a missing heading fails and names it', () => {
  const t = card('humanity-core').replace('## Open work\n', '');
  const p = C.checkCard('humanity-core', t, NAMES);
  assert.ok(p.some((x) => x.includes('missing heading') && x.includes('## Open work')), p.join('\n'));
});

test('headings out of order, an extra one, or a wrong title fail', () => {
  const swapped = card('humanity-core').replace('## Rules', '## TMP').replace('## How to test it', '## Rules').replace('## TMP', '## How to test it');
  assert.ok(C.checkCard('humanity-core', swapped, NAMES).some((x) => x.includes('out of order')));
  const extra = card('humanity-core').replace('## Open work', '## Notes\n\nSomething.\n\n## Open work');
  assert.ok(C.checkCard('humanity-core', extra, NAMES).some((x) => x.includes('"## Notes"')));
  assert.ok(C.checkCard('humanity-gui', card('humanity-core'), NAMES).some((x) => x.includes('# humanity-gui')));
});

test('no generated block, two of them, or one in the wrong section fail', () => {
  const none = card('humanity-core').replace(C.START + '\n', '').replace(C.END + '\n', '');
  assert.ok(C.checkCard('humanity-core', none, NAMES).some((x) => x.includes('exactly one generated block')));
  const two = card('humanity-core').replace('## Rules\n', `## Rules\n\n${C.START}\n${C.END}\n`);
  assert.ok(C.checkCard('humanity-core', two, NAMES).some((x) => x.includes('exactly one generated block')));
  const moved = card('humanity-core')
    .replace(`${C.START}\n- **Size:** 10 lines in 1 file.\n${C.END}\n`, 'Hand-written.\n')
    .replace('## Rules\n', `## Rules\n\n${C.START}\n${C.END}\n`);
  assert.ok(C.checkCard('humanity-core', moved, NAMES).some((x) => x.includes('belongs under')));
});

test('an empty section, or a skeleton placeholder left in, fails', () => {
  const empty = card('humanity-core').replace(/## Rules\n\n[^\n]+\n/, '## Rules\n\n');
  assert.ok(C.checkCard('humanity-core', empty, NAMES).some((x) => x.includes('"## Rules" is empty')));
  assert.ok(C.checkCard('humanity-core', C.skeleton('humanity-core'), NAMES).some((x) => x.includes('(to write)')));
});

test('the mailbox: a request in the form passes and is counted; any other shape, or a sender not in the plan, fails', () => {
  const good = '- 2026-11-02, from humanity-gui (the Garden page session): make `DataStore::keys` public. Why: the Garden page lists what is loaded.';
  const t = card('humanity-core', { requests: [good] });
  assert.deepStrictEqual(C.checkCard('humanity-core', t, NAMES), []);
  assert.strictEqual(C.requestsOf(t).length, 1);
  assert.strictEqual(C.requestsOf(card('humanity-core')).length, 0);
  const loose = card('humanity-core', { requests: ['- please make DataStore::keys public'] });
  assert.ok(C.checkCard('humanity-core', loose, NAMES).some((x) => x.includes("mailbox's form")));
  const stranger = card('humanity-core', { requests: [good.replace('humanity-gui', 'humanity-sim')] });
  assert.ok(C.checkCard('humanity-core', stranger, NAMES).some((x) => x.includes('not another crate')));
  const self = card('humanity-core', { requests: [good.replace('humanity-gui', 'humanity-core')] });
  assert.ok(C.checkCard('humanity-core', self, NAMES).some((x) => x.includes('not another crate')));
});

test('the check never compares the numbers: a block whose every figure moved still passes', () => {
  const before = card('humanity-core', { block: ['- **Size:** 14,579 lines in 50 files.', '| `ecs::World` | 100 | engine 100 |'] });
  const after = before.replace('14,579', '15,002').replace('50 files', '53 files').replace('| 100 | engine 100 |', '| 131 | engine 120, gui 11 |');
  assert.deepStrictEqual(C.checkCard('humanity-core', before, NAMES), []);
  assert.deepStrictEqual(C.checkCard('humanity-core', after, NAMES), []);
});

test('a folder of cards: a missing card, a stray card and a missing index each fail; the full set passes', (t) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'hos-crate-cards-'));
  t.after(() => fs.rmSync(dir, { recursive: true, force: true }));
  const write = (f, text) => fs.writeFileSync(path.join(dir, f), text);
  const index = `# Cards\n\n${C.START}\n| [humanity-core](humanity-core.md) |\n| [humanity-gui](humanity-gui.md) |\n${C.END}\n`;
  write('humanity-core.md', card('humanity-core'));
  let p = C.checkAll(dir, PLAN);
  assert.ok(p.some((x) => x.includes('humanity-gui.md is missing')), p.join('\n'));
  assert.ok(p.some((x) => x.includes('README.md, the index, is missing')), p.join('\n'));
  write('humanity-gui.md', card('humanity-gui'));
  write('README.md', index);
  assert.deepStrictEqual(C.checkAll(dir, PLAN), []);
  write('humanity-sim.md', card('humanity-sim'));
  p = C.checkAll(dir, PLAN);
  assert.ok(p.some((x) => x.includes('humanity-sim.md is a card for a crate the plan does not name')), p.join('\n'));
  fs.rmSync(path.join(dir, 'humanity-sim.md'));
  write('README.md', index.replace('| [humanity-gui](humanity-gui.md) |\n', ''));
  assert.ok(C.checkAll(dir, PLAN).some((x) => x.includes('does not link humanity-gui.md')));
});

test('the generator swaps only the block, in the file\'s own line endings, and leaves a file without one alone', () => {
  const text = 'Hand-written above.\r\n\r\n' + [C.START, 'old line', C.END].join('\r\n') + '\r\n\r\nHand-written below.\r\n';
  const out = C.replaceBlock(text, ['new 1', 'new 2']);
  assert.strictEqual(out, 'Hand-written above.\r\n\r\n' + [C.START, 'new 1', 'new 2', C.END].join('\r\n') + '\r\n\r\nHand-written below.\r\n');
  assert.strictEqual(C.replaceBlock('no markers here\n', ['x']), null);
});

test('an interface item is the path up to its first capitalised name', () => {
  assert.strictEqual(C.itemOf('ecs::components::Transform::default'), 'ecs::components::Transform');
  assert.strictEqual(C.itemOf('data_parse::parse_ron'), 'data_parse::parse_ron');
  assert.strictEqual(C.itemOf('cosmos::M_PER_AU'), 'cosmos::M_PER_AU');
});

test('a test\'s name is its module path, its inline modules and its function, as libtest prints it', () => {
  const { blankLiterals } = require('../crate-graph.js');
  const file = (rel, mod, crate, src) => ({ rel, mod, crate, all: blankLiterals(src) });
  const names = C.testNames({ files: [
    file('relay/resumable_tests.rs', 'relay/resumable/tests', 'humanity-relay', [
      'mod round2 {',
      '    #[test]',
      '    fn a() { let s = "{"; }',
      '    #[tokio::test(flavor = "multi_thread")]',
      '    async fn b() {}',
      '}',
      '#[test]',
      '#[ignore]',
      'fn c() {}',
    ].join('\n')),
    file('gui/ui_snapshots.rs', 'gui/ui_snapshots', 'humanity-gui', [
      'macro_rules! page_snapshot { ($name:ident, $page:expr) => { #[test] #[ignore] fn $name() {} }; }',
      'page_snapshot!(snapshot_calendar, "calendar");',
      'format!("{}", 1);',
    ].join('\n')),
    file('lib.rs', 'lib', 'humanity-engine', '#[cfg(test)]\nmod tests {\n    #[test]\n    fn root() {}\n}\n'),
    file('main.rs', 'main', 'humanity-engine', '#[test]\nfn not_a_lib_test() {}\n'),
  ] });
  assert.deepStrictEqual([...names].sort(), [
    ['gui::ui_snapshots::snapshot_calendar', 'humanity-gui'],
    ['relay::resumable::tests::c', 'humanity-relay'],
    ['relay::resumable::tests::round2::a', 'humanity-relay'],
    ['relay::resumable::tests::round2::b', 'humanity-relay'],
    ['tests::root', 'humanity-engine'],
  ]);
});

test('the cards in the tree pass the check (what `just lints` runs)', () => {
  const plan = JSON.parse(fs.readFileSync(path.join(__dirname, '..', 'crate-layers.json'), 'utf8'));
  assert.deepStrictEqual(C.checkAll(path.join(__dirname, '..', '..', 'docs', 'crates'), plan), []);
});
