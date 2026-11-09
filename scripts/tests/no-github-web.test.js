// No page of the website asks GitHub's API (BUG-281, 2026-11-07). Every page's update check
// (web/shared/shell.js), the download page and the Dev workbench's Claude tab asked
// api.github.com, so GitHub saw every visitor's address and browser. They read this server's own
// files now: /releases/manifest.json (written by scripts/vps/regen-releases-manifest when a release
// lands) and /dev-info/ (written by every deploy, scripts/sync-web-root.sh). GitHub sees a visitor
// only when they press a download button, which goes to the release file's own address there.
// Run: node --test scripts/tests/no-github-web.test.js
//
// Seen red 2026-11-07 against the tree before the fix: "no page asks api.github.com" named
// web/shared/shell.js, web/pages/download.html and web/pages/dev-claude.js, the site's
// Content-Security-Policy still allowed https://api.github.com in all three server blocks, and the
// deploy wrote no /dev-info/ files.
'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');
const { spawnSync } = require('child_process');

const ROOT = path.join(__dirname, '..', '..');
const WEB = path.join(ROOT, 'web');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

function walk(dir, out) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else if (/\.(js|html|mjs)$/.test(e.name)) out.push(p);
  }
  return out;
}

test('no page of the website asks api.github.com', () => {
  const askers = walk(WEB, []).filter((f) => fs.readFileSync(f, 'utf8').includes('api.github.com'));
  assert.deepStrictEqual(askers.map((f) => path.relative(ROOT, f).replace(/\\/g, '/')), [], 'no page asks api.github.com');
});

test("the site's Content-Security-Policy no longer lets a page reach api.github.com", () => {
  const conf = read('scripts/nginx/humanity.conf');
  const csps = conf.split('\n').filter((l) => l.includes('Content-Security-Policy'));
  assert.ok(csps.length >= 3, 'the policy lines are found');
  for (const l of csps) assert.ok(!l.includes('api.github.com'), 'connect-src without api.github.com');
});

test("every page's update check reads this server's release list", () => {
  const shell = read('web/shared/shell.js');
  assert.ok(shell.includes("fetch('/releases/manifest.json')"), "shell.js asks /releases/manifest.json");
});

test("the download page takes the platform's file from this server's list and links its own address on GitHub", async () => {
  const html = read('web/pages/download.html');
  const start = html.indexOf('function newestRelease()');
  const end = html.indexOf('detectPlatform();', start);
  assert.ok(start > 0 && end > start, 'the release helpers are where the test expects them');
  const manifest = {
    releases: [
      { tag: 'v0.1549.0', assets: [
        { name: 'HumanityOS-windows-x64.exe', size: 90e6 },
        { name: 'HumanityOS-windows-x64.zip', size: 398e6 },
        { name: 'HumanityOS-linux-x64.zip', size: 397e6 },
      ] },
      { tag: 'v0.1548.0', assets: [{ name: 'HumanityOS-windows-x64.zip', size: 1 }] },
    ],
  };
  const asked = [];
  const ctx = vm.createContext({
    fetch: (u) => { asked.push(u); return Promise.resolve({ json: () => Promise.resolve(manifest) }); },
    Math, encodeURIComponent,
  });
  vm.runInContext(html.slice(start, end) + '\n;globalThis.__d = { newestRelease, releaseFileUrl, fetchDirectDownload };', ctx);
  const btn = { href: '' };
  const sub = { textContent: '' };
  ctx.__d.fetchDirectDownload('windows', 'x64', btn, sub);
  for (let i = 0; i < 5; i++) await new Promise((r) => setImmediate(r));
  assert.deepStrictEqual(asked, ['/releases/manifest.json'], 'only this server is asked');
  assert.strictEqual(btn.href, 'https://github.com/Shaostoul/Humanity/releases/download/v0.1549.0/HumanityOS-windows-x64.zip', 'the full game, newest release');
  assert.match(sub.textContent, /398 MB/, 'with its size');
  assert.match(sub.textContent, /v0\.1549\.0/, 'and its version');
});

// Git for Windows' bash, not the WSL launcher a bare "bash" can resolve to.
function bashPath() {
  if (process.platform !== 'win32') return 'bash';
  for (const p of ['C:/Program Files/Git/bin/bash.exe', 'C:/Program Files/Git/usr/bin/bash.exe']) if (fs.existsSync(p)) return p;
  return 'bash';
}
const px = (p) => (process.platform === 'win32' ? p.replace(/\\/g, '/').replace(/^([A-Za-z]):/, (_, d) => `/${d.toLowerCase()}`) : p);

test("every deploy writes the Claude tab's /dev-info/ files, valid JSON whatever the commit's subject says", () => {
  const sh = read('scripts/sync-web-root.sh').replace(/\r\n/g, '\n');
  const start = sh.indexOf('mkdir -p "$DST/dev-info"');
  const end = sh.indexOf('echo "web root synced', start);
  assert.ok(start > 0 && end > start, 'the deploy writes /dev-info/');
  const tmp = fs.mkdtempSync(path.join(require('os').tmpdir(), 'hos-devinfo-'));
  const src = path.join(tmp, 'src');
  const dst = path.join(tmp, 'dst');
  fs.mkdirSync(path.join(src, 'docs'), { recursive: true });
  fs.writeFileSync(path.join(src, 'docs', 'PRIORITIES.md'), '# P\n\n## TIER 0\n\n### The top item\n');
  const subject = 'Quote " and back\\slash\tand a tab';
  // A stand-in for git: the subject for %s, a time for %cI.
  const script = `set -euo pipefail
SRC='${px(src)}'; DST='${px(dst)}'
src_git() { case "$*" in *%s*) printf '%s\\n' '${subject.replace(/'/g, "'\\''")}';; *%cI*) echo 2026-11-07T12:00:00-08:00;; esac; }
${sh.slice(start, end)}`;
  // From a file: a Windows command line would mangle the script's backslashes on the way to bash.
  const file = path.join(tmp, 'run.sh');
  fs.writeFileSync(file, script);
  const r = spawnSync(bashPath(), [px(file)], { encoding: 'utf8' });
  assert.strictEqual(r.status, 0, r.stderr);
  const main = JSON.parse(fs.readFileSync(path.join(dst, 'dev-info', 'main.json'), 'utf8'));
  assert.strictEqual(main.at, '2026-11-07T12:00:00-08:00');
  assert.strictEqual(main.subject, 'Quote " and back\\slashand a tab', 'quotes and backslashes kept, the tab dropped');
  assert.ok(fs.readFileSync(path.join(dst, 'dev-info', 'priorities.md'), 'utf8').includes('### The top item'));
  fs.rmSync(tmp, { recursive: true, force: true });
});
