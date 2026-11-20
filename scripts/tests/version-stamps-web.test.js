// THE APP VERSION WRITTEN INTO THE WEBSITE IS THE ONE IN Cargo.toml, AND THE BUMP STAMPS EVERY COPY
// (2026-11-20). Pure node, no game: runs in `just rig-tests`.
//
// Run:  node --test scripts/tests/version-stamps-web.test.js
//
// The defect this guards: the website's script policy work (docs/PRIORITIES.md item 6b) moved the
// download page's and the Ops page's inline scripts into web/pages/download-app.js and
// web/pages/ops-app.js, but scripts/bump-version.js went on stamping the version into download.html
// and ops.html only. The first release after the merge would have left the download button pointing
// at the previous release and the Ops page naming the previous version, with nothing failing.
//
// SEEN RED (2026-11-20), on the branch merged with main's v0.1565.0 before the fix: check 1 listed
// "web/pages/download-app.js:72 says 0.1564.0" (and :73, :162) and "web/pages/ops-app.js:7 says
// 0.1564.0"; check 2 listed "web/pages/download-app.js" and "web/pages/ops-app.js" as files
// holding a version stamp that scripts/bump-version.js never names. After the fix, the bump run on
// a scratch copy of the repo (0.1565.0 -> 0.1565.1) left all 12 stamps saying 0.1565.1.

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');
const WEB = path.join(ROOT, 'web');

/** The one version every workspace member carries ([workspace.package] in the root Cargo.toml). */
function cargoVersion() {
  const toml = fs.readFileSync(path.join(ROOT, 'Cargo.toml'), 'utf8');
  const m = /\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"/m.exec(toml);
  assert.ok(m, 'Cargo.toml has a [workspace.package] version');
  return m[1];
}

// The shapes the bump writes (scripts/bump-version.js): a release link, the download page's
// fallback version, the Ops page's version line, the shell's update check, the Settings footer.
const STAMPS = [
  /releases\/tag\/v(\d+\.\d+\.\d+)/g,
  /GitHub Releases · v(\d+\.\d+\.\d+)/g,
  /__HOS_APP_VERSION\s*\|\|\s*'(\d+\.\d+\.\d+)'/g,
  /\('debug-version'\)\.textContent\s*=\s*'v(\d+\.\d+\.\d+)'/g,
  /CURRENT_VERSION\s*=\s*'(\d+\.\d+\.\d+)'/g,
  /HumanityOS, v(\d+\.\d+\.\d+)/g,
];

function walk(dir, out = []) {
  for (const f of fs.readdirSync(dir)) {
    const p = path.join(dir, f);
    const st = fs.statSync(p);
    if (st.isDirectory()) { if (f !== 'vendor' && f !== 'node_modules') walk(p, out); }
    else if (/\.(html|js)$/.test(f)) out.push(p);
  }
  return out;
}

/** Every stamp under web/: { file (repo-relative, forward slashes), line, version }. */
function stamps() {
  const out = [];
  for (const p of walk(WEB)) {
    const src = fs.readFileSync(p, 'utf8');
    const rel = path.relative(ROOT, p).split(path.sep).join('/');
    for (const re of STAMPS) {
      re.lastIndex = 0;
      let m;
      while ((m = re.exec(src))) out.push({ file: rel, line: src.slice(0, m.index).split('\n').length, version: m[1] });
    }
  }
  return out;
}

test('every version stamp on the website says Cargo.toml\'s version', () => {
  const want = cargoVersion();
  const all = stamps();
  assert.ok(all.length >= 5, 'the stamps were found (' + all.length + '): the shapes above still match the pages');
  const bad = all.filter((s) => s.version !== want).map((s) => `${s.file}:${s.line} says ${s.version}`);
  assert.deepStrictEqual(bad, [], `\nVERSION STAMPS that do not say ${want}:\n` + bad.join('\n'));
});

test('scripts/bump-version.js stamps every file that holds a version stamp', () => {
  const bump = fs.readFileSync(path.join(ROOT, 'scripts', 'bump-version.js'), 'utf8');
  const files = [...new Set(stamps().map((s) => s.file))].sort();
  const missed = files.filter((f) => !bump.includes(`'${f}'`));
  assert.deepStrictEqual(missed, [], '\nFILES holding a version stamp that scripts/bump-version.js never names:\n' + missed.join('\n'));
});
