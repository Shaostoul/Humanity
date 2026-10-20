// A page the website stops shipping is removed from the live site, and the deploy publishes
// only the game data the repo tracks (2026-10-20). Both found by a review of that night's work.
//
//  1. scripts/sync-web-root.sh copies web/pages/ and web/shared/ onto the web root WITHOUT
//     deleting (the web root also holds releases/ and uploads/ that no copy owns), so a page
//     removed from the repo keeps being served until a line removes it by name. The Agents page
//     was deleted from the repo on 2026-08-16 and was still live, linked from the mobile menu,
//     two months later; the Tools, Media and Shared Files pages retired on 2026-10-20 would have
//     joined it.
//  2. Its data step copied EVERY *.json under the server's data/ folder to the public site. On a
//     server that folder is also the running relay's, so a JSON file the relay writes there (a
//     members-only upload, an upload waiting to be published) would have been published.
//
// Run: node --test scripts/tests/web-root-retired.test.js
//
// What it proves:
//  1. Every .html, .js or .css file ever deleted from (or renamed away from) web/pages/ or
//     web/shared/, and not back in the repo, is removed from the web root by a line of the
//     script, at the path the deploy put it (pages/*.html at the root, page scripts under
//     pages/, shared files under shared/). Read from git history, so nothing has to remember.
//  2. The check catches what it is for: with a removal taken out of the script it names exactly
//     the file that would stay live.
//  3. The data step publishes git's list of tracked JSON, not a wildcard over data/, and refuses
//     a folder that is not a git checkout.
//
// HOS_SYNC_SCRIPT points the checks at another copy of the script (used to see them red).
//
// Red first, 2026-10-20, run against the script as it was before the fix (523de7fe2):
//   HOS_SYNC_SCRIPT=<that copy> node --test scripts/tests/web-root-retired.test.js
//   1. "these retired files would stay live": agents.html, ai-usage.html, audit.html,
//      dashboard.html, data.html, landing.html, media.html, pages/media-add.js, ... ,
//      pages/tools-app.js, projects.html, shared-files.html, shared/media.css, tools.html
//   3. "the data step publishes a wildcard over data/": the old rsync --include='*.json' line.

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const REPO = path.resolve(__dirname, '..', '..');
const SCRIPT = process.env.HOS_SYNC_SCRIPT || path.join(REPO, 'scripts', 'sync-web-root.sh');

function git(args) {
  return execFileSync('git', ['-C', REPO, ...args], { encoding: 'utf8', maxBuffer: 64 << 20 });
}

// A shallow checkout (CI's default) has no history to read, so check 1 would pass on nothing.
function shallow() {
  return git(['rev-parse', '--is-shallow-repository']).trim() === 'true';
}

// Every top-level page, page script or shared file that git history shows leaving web/pages/ or
// web/shared/ and that is not in the repo now.
function retiredFiles() {
  const out = git(['log', '--diff-filter=DR', '--name-status', '--format=', '--', 'web/pages/', 'web/shared/']);
  const gone = new Set();
  for (const line of out.split('\n')) {
    const parts = line.trim().split('\t');
    if (parts.length < 2) continue;
    const old = parts[1]; // "D\tpath" or "R100\told\tnew": the old path either way
    if (!/^web\/(pages|shared)\/[^/]+\.(html|js|css)$/.test(old)) continue;
    if (fs.existsSync(path.join(REPO, old))) continue;
    gone.add(old);
  }
  return [...gone].sort();
}

// Where the deploy put a file: pages' HTML flattened to the root, their scripts under pages/,
// shared files under shared/ (scripts/sync-web-root.sh).
function servedAt(repoPath) {
  const base = path.basename(repoPath);
  if (repoPath.startsWith('web/pages/')) return base.endsWith('.html') ? base : 'pages/' + base;
  return 'shared/' + base;
}

// The web-root paths the script removes: the `for old in a b c; do rm -f "$DST/${old}.html"`
// loops, and every "$DST/x" or "$DST"/x on an `rm -f` line (a * matches within one folder).
function removals(text) {
  const pats = [];
  const loopRe = /for old in ([^;]+); do\s*\n\s*rm -f "\$DST\/\$\{old\}(\.[a-z]+)"/g;
  let m;
  while ((m = loopRe.exec(text))) {
    for (const name of m[1].trim().split(/\s+/)) if (name !== '\\') pats.push(name + m[2]);
  }
  for (const line of text.split('\n')) {
    if (!/^\s*rm -f /.test(line)) continue;
    const tokRe = /"\$DST(?:\/([^"]+)"|"\/(\S+))/g;
    let t;
    while ((t = tokRe.exec(line))) {
      const rel = t[1] || t[2];
      if (rel && !rel.includes('${')) pats.push(rel);
    }
  }
  return pats;
}

function covered(rel, pats) {
  return pats.some((p) => {
    const re = '^' + p.replace(/[.+?^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '[^/]*') + '$';
    return new RegExp(re).test(rel);
  });
}

function stillLive(text, files) {
  const pats = removals(text);
  return files.map(servedAt).filter((rel) => !covered(rel, pats)).sort();
}

// True when the data step copies a wildcard over the server's data/ folder.
function publishesWildcard(text) {
  return /rsync[^\n]*(\\\n[^\n]*)*--include='\*\.json'[^\n]*(\\\n[^\n]*)*"\$SRC\/data\/"/.test(text);
}

const scriptText = () => fs.readFileSync(SCRIPT, 'utf8');

test('every page and page file ever removed from web/ is removed from the live site by name', (t) => {
  if (shallow()) return t.skip('shallow checkout: no history to read');
  const files = retiredFiles();
  assert.ok(files.length >= 20, `expected the known retirements in history, found ${files.length}`);
  assert.deepStrictEqual(stillLive(scriptText(), files), [], 'these retired files would stay live');
});

test('the check names the file a missing removal would leave live (a check that cannot fail proves nothing)', () => {
  const text = scriptText();
  const sample = ['web/pages/agents.html', 'web/pages/tools-app.js', 'web/pages/media-panes.js', 'web/shared/media.css'];
  assert.deepStrictEqual(stillLive(text, sample), []);
  const noAgents = text.replace(/(for old in [^;]*)\bagents\b/, '$1');
  assert.deepStrictEqual(stillLive(noAgents, sample), ['agents.html']);
  const noToolsJs = text.replace(/"\$DST\/pages\/tools-app\.js" ?/, '');
  assert.deepStrictEqual(stillLive(noToolsJs, sample), ['pages/tools-app.js']);
  const noMediaGlob = text.replace(/^rm -f "\$DST"\/pages\/media-\*\.js\n/m, '');
  assert.deepStrictEqual(stillLive(noMediaGlob, sample), ['pages/media-panes.js']);
});

test('the deploy publishes only the data JSON the repo tracks', () => {
  const text = scriptText();
  assert.ok(!publishesWildcard(text), 'the data step publishes a wildcard over data/');
  assert.match(text, /ls-files -z -- 'data\/\*\.json' \| rsync -a --from0 --files-from=- "\$SRC\/" "\$DST\/"/);
  assert.match(text, /not a git checkout[^\n]*refusing/);
  // The old line, which this check exists to catch.
  const old = `rsync -a --prune-empty-dirs \\\n  --include='*/' --include='*.json' --exclude='*' \\\n  "$SRC/data/" "$DST/data/"`;
  assert.ok(publishesWildcard(old), 'the wildcard check no longer recognises the old line');
});
