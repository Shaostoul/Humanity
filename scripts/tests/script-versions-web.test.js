// A cache-busting version means one set of bytes for ever (2026-11-21, D8 of the signed-requests
// round 2).
//
// Run: node --test scripts/tests/script-versions-web.test.js   (in `just rig-tests`)
//
// THE FAULT IT GUARDS: a page loads `/chat/pq.js?v=N`, and a browser holding `?v=N` keeps those
// bytes. Round 1 of the signed requests changed pq.js and pq-relay-auth.js and gave them ?v=2 and
// ?v=3, numbers main had already used for the older files, so a cached browser ran the old signer
// on several pages; other pages still loaded stale numbers of scripts that had moved on.
//
// THE CHOICE (a recorded table, not git history): scripts/tests/fixtures/script-versions.json
// records, for each script and version, the SHA-256 of its bytes (line endings read as LF). A table
// works in a shallow CI checkout and in a copy of web/ with no history, and it remembers a version
// even after every page has moved past it, so an old number can never be taken up again for new
// bytes. scripts/record-script-versions.js adds new versions and refuses to rewrite a recorded one.
//
// What it proves: every versioned script any page loads is recorded at that version with exactly
// its current bytes (so changed bytes need a new number), and every page loads a script at one
// version (so no page keeps a stale number).
//
// RED EVIDENCE (2026-11-21): with web/chat/pq.js changed by one comment and its ?v= left as it was,
// "every versioned script is recorded with exactly its bytes" failed: "chat/pq.js changed but
// ?v=20261121 already meant other bytes"; with pages/admin.html back at blake3.js?v=1, "every page
// loads a script at one version" failed naming shared/blake3.js.

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const ROOT = path.join(__dirname, '..', '..');
const tool = require(path.join(ROOT, 'scripts', 'record-script-versions.js'));

test('every versioned script is recorded with exactly its bytes', () => {
  const wrong = tool.problems(tool.readTable()).filter((p) => !p.includes('more than one version'));
  assert.deepStrictEqual(wrong, [], wrong.join('\n'));
});

test('every page loads a script at one version', () => {
  const wrong = tool.problems(tool.readTable()).filter((p) => p.includes('more than one version'));
  assert.deepStrictEqual(wrong, [], wrong.join('\n'));
});

test('the pages load versioned scripts at all, and the table holds them', () => {
  const refs = tool.references();
  assert.ok(refs.length > 100, 'the pages load versioned scripts: ' + refs.length);
  assert.ok(refs.some((r) => r.script === 'chat/pq.js'), 'pq.js is among them');
});

test('the recorder refuses to give a recorded version other bytes', () => {
  // A copy of one page and one script in a scratch folder, with a table of its own.
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'hos-script-versions-'));
  try {
    fs.mkdirSync(path.join(dir, 'web', 'chat'), { recursive: true });
    fs.writeFileSync(path.join(dir, 'web', 'index.html'), '<script src="/chat/x.js?v=1"></script>');
    fs.writeFileSync(path.join(dir, 'web', 'chat', 'x.js'), 'one();\n');
    const env = { ...process.env, HOS_WEB_DIR: path.join(dir, 'web'), HOS_SCRIPT_VERSIONS: path.join(dir, 'table.json') };
    const run = (args) => {
      try { execFileSync(process.execPath, [path.join(ROOT, 'scripts', 'record-script-versions.js'), ...args], { env, stdio: 'pipe' }); return 0; }
      catch (e) { return e.status; }
    };
    assert.strictEqual(run([]), 0, 'a new version is recorded');
    assert.strictEqual(run(['--check']), 0, 'and then checks out');
    fs.writeFileSync(path.join(dir, 'web', 'chat', 'x.js'), 'two();\n');
    assert.strictEqual(run(['--check']), 1, 'changed bytes under the same version are caught');
    assert.strictEqual(run([]), 1, 'and the recorder refuses to rewrite the version');
    const table = JSON.parse(fs.readFileSync(path.join(dir, 'table.json'), 'utf8'));
    assert.strictEqual(Object.keys(table['chat/x.js']).length, 1, 'the old version keeps its old bytes');
    fs.writeFileSync(path.join(dir, 'web', 'index.html'), '<script src="/chat/x.js?v=2"></script>');
    assert.strictEqual(run([]), 0, 'a new number for the new bytes is recorded');
    assert.strictEqual(run(['--check']), 0);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

// C9 (2026-11-21, signed requests round 3): versions only rise. A number below or equal to the
// highest recorded for a script is refused (a browser may hold it with other bytes), even one the
// table never saw with these bytes, and a version that is not a whole number is refused. Red
// evidence (2026-11-21, `newVersionProblem` made to answer '' always): "a lower number is refused"
// failed, left 0.
test('the recorder refuses a reused or lower version number', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'hos-script-versions-'));
  try {
    fs.mkdirSync(path.join(dir, 'web', 'chat'), { recursive: true });
    const page = (v) => fs.writeFileSync(path.join(dir, 'web', 'index.html'), '<script src="/chat/x.js?v=' + v + '"></script>');
    fs.writeFileSync(path.join(dir, 'web', 'chat', 'x.js'), 'one();\n');
    const table = path.join(dir, 'table.json');
    // History once used 7 for this script (the backfill's mark: no bytes recorded).
    fs.writeFileSync(table, JSON.stringify({ 'chat/x.js': { 7: 'git:000000000000' } }));
    const env = { ...process.env, HOS_WEB_DIR: path.join(dir, 'web'), HOS_SCRIPT_VERSIONS: table };
    const run = (args) => {
      try { execFileSync(process.execPath, [path.join(ROOT, 'scripts', 'record-script-versions.js'), ...args], { env, stdio: 'pipe' }); return 0; }
      catch (e) { return e.status; }
    };
    page(5);
    assert.strictEqual(run(['--check']), 1, 'a lower number is caught');
    assert.strictEqual(run([]), 1, 'a lower number is refused');
    page(7);
    assert.strictEqual(run([]), 1, 'a number history used is refused');
    page('8a');
    assert.strictEqual(run([]), 1, 'a version that is not a whole number is refused');
    page(8);
    assert.strictEqual(run([]), 0, 'a higher number is recorded');
    assert.strictEqual(run(['--check']), 0);
    assert.deepStrictEqual(Object.keys(JSON.parse(fs.readFileSync(table, 'utf8'))['chat/x.js']), ['7', '8']);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

// THE RELEASE KEEPS THE TABLE TRUE (2026-11-21, signed requests round 5). Every release runs
// scripts/bump-version.js, which writes the new version into web/shared/shell.js and
// web/pages/settings-app.js (and any other script it stamps). It changed their bytes and left their
// ?v= alone, so the table failed after every release and a browser holding the old ?v= kept the
// old copy. The orchestrator's call: the bump itself gives each script it stamps a fresh ?v= on
// every page that loads it at a version, and records it, so no release step can forget it.
//
// What it proves: the bump, run on a copy of the repository (the scripts, the table, Cargo.toml,
// the crates' Cargo.toml files and every page and script of web/), leaves the table check passing,
// twice in a row (two releases in one day), and its list of changed files names the pages and the
// table it rewrote, so the release commit carries them.
//
// RED EVIDENCE (2026-11-21, the bump before this fix, on a copy of this branch at 0.1568.0): the
// check after the bump failed with "shared/shell.js changed but ?v=20261122 already meant other
// bytes (chat/index.html)" and "pages/settings-app.js changed but ?v=20261122 already meant other
// bytes (pages/settings.html)".
test('a release bump leaves every versioned script recorded with exactly its bytes', () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'hos-bump-versions-'));
  try {
    const copy = (rel) => {
      fs.mkdirSync(path.dirname(path.join(dir, rel)), { recursive: true });
      fs.copyFileSync(path.join(ROOT, rel), path.join(dir, rel));
    };
    for (const rel of ['scripts/bump-version.js', 'scripts/record-script-versions.js',
      'scripts/tests/fixtures/script-versions.json', 'Cargo.toml', 'Cargo.lock']) copy(rel);
    for (const crate of fs.readdirSync(path.join(ROOT, 'crates'))) {
      if (fs.existsSync(path.join(ROOT, 'crates', crate, 'Cargo.toml'))) copy('crates/' + crate + '/Cargo.toml');
    }
    (function walk(rel) {
      for (const e of fs.readdirSync(path.join(ROOT, rel), { withFileTypes: true })) {
        const sub = rel + '/' + e.name;
        if (e.isDirectory()) { if (!['app', 'node_modules'].includes(e.name)) walk(sub); }
        else if (/\.(html|js)$/.test(e.name)) copy(sub);
      }
    })('web');
    // The copy's own paths: no override from the environment may point it back at this checkout.
    const env = { ...process.env };
    delete env.HOS_WEB_DIR;
    delete env.HOS_SCRIPT_VERSIONS;
    const node = (args) => {
      try { return { code: 0, out: execFileSync(process.execPath, args, { cwd: dir, env, stdio: 'pipe', encoding: 'utf8' }) }; }
      catch (e) { return { code: e.status, out: String(e.stdout || '') + String(e.stderr || '') }; }
    };
    const check = () => node([path.join(dir, 'scripts', 'record-script-versions.js'), '--check']);
    assert.strictEqual(check().code, 0, 'test setup: the copy checks out before the bump:\n' + check().out);
    const versionOf = (page, script) => {
      const m = new RegExp('src="/' + script.replace(/[.*+?^${}()|[\]\\/]/g, '\\$&') + '\\?v=(\\d+)"').exec(fs.readFileSync(path.join(dir, 'web', page), 'utf8'));
      assert.ok(m, 'test setup: ' + page + ' loads ' + script + ' at a version');
      return Number(m[1]);
    };
    const before = versionOf('chat/index.html', 'shared/shell.js');
    const list = path.join(dir, 'bump-files.txt');
    for (const round of [1, 2]) {
      const bump = node([path.join(dir, 'scripts', 'bump-version.js'), 'patch', '--changed-list', list]);
      assert.strictEqual(bump.code, 0, 'bump ' + round + ' ran:\n' + bump.out);
      const after = check();
      assert.strictEqual(after.code, 0, 'after bump ' + round + ' the table still checks out:\n' + after.out);
      const changed = fs.readFileSync(list, 'utf8').split(/\r?\n/).filter(Boolean);
      for (const want of ['Cargo.toml', 'web/shared/shell.js', 'web/chat/index.html', 'web/pages/settings.html',
        'scripts/tests/fixtures/script-versions.json']) {
        assert.ok(changed.includes(want), 'bump ' + round + '\'s changed-file list names ' + want + ': ' + changed.join(' '));
      }
    }
    assert.ok(versionOf('chat/index.html', 'shared/shell.js') >= before + 2, 'each bump gave shell.js a higher ?v=');
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});
