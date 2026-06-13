#!/usr/bin/env node
// Markdown link checker for the docs/ tree (plus root README.md / CLAUDE.md). Used to
// verify the audience-first reorg did not break any internal links. It resolves every
// relative markdown link and bare repo-root path reference against the file's location
// and reports targets that do not exist on disk.
//
//   node scripts/check-doc-links.js            # report broken links, exit 1 if any
//   node scripts/check-doc-links.js --quiet    # only the summary line
//
// Skips: external URLs (http/https/mailto), pure anchors (#...), and the frozen
// archives (history/project-universe-site, website/_includes/canon).

const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const QUIET = process.argv.includes('--quiet');

const SKIP_DIRS = [
  path.join('docs', 'history', 'project-universe-site'),
  // The Jekyll website uses permalink routes + baseurl, not on-disk relative paths,
  // so its links cannot be validated as files. Its own build catches its breaks.
  path.join('docs', 'website'),
  'node_modules',
  '.git',
];

function walk(dir, acc) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    const rel = path.relative(ROOT, full);
    if (SKIP_DIRS.some((s) => rel === s || rel.startsWith(s + path.sep))) continue;
    if (entry.isDirectory()) walk(full, acc);
    else if (entry.name.endsWith('.md')) acc.push(full);
  }
  return acc;
}

// Strip a trailing #anchor and surrounding angle brackets / quotes from a link target.
function cleanTarget(t) {
  let s = t.trim();
  if (s.startsWith('<') && s.endsWith('>')) s = s.slice(1, -1);
  // drop title:  (path "Title")
  s = s.replace(/\s+["'(].*$/, '');
  const hash = s.indexOf('#');
  if (hash >= 0) s = s.slice(0, hash);
  return s.trim();
}

function isExternal(t) {
  return /^(https?:|mailto:|tel:|ftp:|data:|#)/i.test(t) || t === '';
}

// Resolve a link target found in `fromFile` to an absolute path, or null if it is the
// kind we do not check (external, anchor-only).
function resolveTarget(fromFile, target) {
  const t = cleanTarget(target);
  if (isExternal(t)) return null;
  // Repo-root-relative bare references like "docs/design/x.md" or "data/foo.json"
  // (common in CLAUDE.md and code comments). Treat a leading known-top-dir as
  // repo-root-relative.
  if (/^(docs|data|src|web|scripts|assets|schemas)\//.test(t)) {
    return path.join(ROOT, t);
  }
  if (t.startsWith('/')) return path.join(ROOT, t.slice(1));
  return path.resolve(path.dirname(fromFile), t);
}

const LINK_RE = /\[[^\]]*\]\(([^)]+)\)/g;

const files = walk(path.join(ROOT, 'docs'), []);
// Also check the two root entry docs that point into docs/.
for (const extra of ['README.md', 'CLAUDE.md']) {
  const p = path.join(ROOT, extra);
  if (fs.existsSync(p)) files.push(p);
}

let broken = [];
let checked = 0;
let inFence = false;

for (const file of files) {
  const text = fs.readFileSync(file, 'utf8');
  const lines = text.split('\n');
  inFence = false;
  for (const line of lines) {
    const t = line.trim();
    if (t.startsWith('```') || t.startsWith('~~~')) { inFence = !inFence; continue; }
    if (inFence) continue;
    let m;
    LINK_RE.lastIndex = 0;
    while ((m = LINK_RE.exec(line))) {
      const target = m[1];
      const resolved = resolveTarget(file, target);
      if (resolved === null) continue;
      checked++;
      // Only flag links to .md / .json / known doc assets that should exist on disk.
      // Allow directory links (resolve to a dir).
      let ok = false;
      try {
        const st = fs.statSync(resolved);
        ok = st.isFile() || st.isDirectory();
      } catch (e) { ok = false; }
      if (!ok) {
        broken.push({
          file: path.relative(ROOT, file),
          target: cleanTarget(target),
        });
      }
    }
  }
}

if (!QUIET && broken.length) {
  console.log('Broken internal links:');
  for (const b of broken) console.log(`  ${b.file}  ->  ${b.target}`);
  console.log('');
}
console.log(`Checked ${checked} internal links across ${files.length} files. Broken: ${broken.length}`);
process.exit(broken.length ? 1 : 0);
