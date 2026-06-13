#!/usr/bin/env node
// Auto-repair broken RELATIVE markdown links in docs/ by resolving each broken target
// to its unique basename match within the docs tree. This fixes the residue from the
// audience-first reorg: a file that linked to a sibling via ../01-VISION.md or
// ./00-START-HERE.md still points at the old location after the sibling moved; if that
// basename now exists at exactly one place under docs/, the link is rewritten to the
// correct relative path. Links whose target basename is missing (genuinely phantom) or
// ambiguous (>1 match) are left untouched and reported.
//
//   node scripts/fix-doc-links.js --dry-run
//   node scripts/fix-doc-links.js

const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const DOCS = path.join(ROOT, 'docs');
const DRY = process.argv.includes('--dry-run');

const SKIP_DIRS = [
  path.join('docs', 'history', 'project-universe-site'),
  path.join('docs', 'website'),
  'node_modules', '.git',
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

const files = walk(DOCS, []);

// basename -> [absolute paths]
const byBase = new Map();
for (const f of files) {
  const b = path.basename(f);
  if (!byBase.has(b)) byBase.set(b, []);
  byBase.get(b).push(f);
}

function isExternal(t) {
  return /^(https?:|mailto:|tel:|ftp:|data:|#)/i.test(t) || t === '';
}

function splitAnchor(t) {
  const h = t.indexOf('#');
  return h >= 0 ? [t.slice(0, h), t.slice(h)] : [t, ''];
}

const LINK_RE = /(\[[^\]]*\]\()([^)]+)(\))/g;

let fixed = 0;
let leftBroken = [];
let filesChanged = 0;

for (const file of files) {
  const orig = fs.readFileSync(file, 'utf8');
  const lines = orig.split('\n');
  let inFence = false;
  let changed = false;

  const out = lines.map((line) => {
    const tl = line.trim();
    if (tl.startsWith('```') || tl.startsWith('~~~')) { inFence = !inFence; return line; }
    if (inFence) return line;

    return line.replace(LINK_RE, (m, pre, target, post) => {
      const [pathPart, anchor] = splitAnchor(target.trim());
      if (isExternal(target.trim()) || pathPart === '') return m;
      // Only touch relative links (skip repo-root-style docs/.. data/.. and absolute /).
      if (/^(docs|data|src|web|scripts|assets|schemas)\//.test(pathPart) || pathPart.startsWith('/')) return m;

      // Does it already resolve?
      const abs = path.resolve(path.dirname(file), pathPart);
      let resolves = false;
      try { resolves = fs.existsSync(abs); } catch (e) { resolves = false; }
      if (resolves) return m;

      // Broken, try a unique basename match within docs/.
      const base = path.basename(pathPart);
      const matches = byBase.get(base);
      if (matches && matches.length === 1) {
        let relNew = path.relative(path.dirname(file), matches[0]).split(path.sep).join('/');
        if (!relNew.startsWith('.')) relNew = './' + relNew;
        fixed++;
        changed = true;
        return pre + relNew + anchor + post;
      }
      leftBroken.push(`${path.relative(ROOT, file)} -> ${pathPart}` +
        (matches ? ` (ambiguous: ${matches.length})` : ' (no match)'));
      return m;
    });
  });

  if (changed) {
    filesChanged++;
    if (!DRY) fs.writeFileSync(file, out.join('\n'));
  }
}

console.log(`${DRY ? '[dry-run] ' : ''}auto-fixed ${fixed} relative links across ${filesChanged} files`);
console.log(`left unfixed (phantom or ambiguous): ${leftBroken.length}`);
