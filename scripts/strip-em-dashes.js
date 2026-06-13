#!/usr/bin/env node
// Remove em dashes from docs (operator directive 2026-06-13: em dashes read as
// AI-written and drive readers away). Replaces the common spaced form " — "
// with ", " outside fenced code blocks, leaves hyphens and en dashes alone, and
// reports any remaining bare em dashes for manual review. Skips the frozen
// Project Universe archive and the auto-generated Jekyll canon mirror.
//
// Usage:
//   node scripts/strip-em-dashes.js            # apply
//   node scripts/strip-em-dashes.js --dry-run  # report only

const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const DRY = process.argv.includes('--dry-run');
const EM = '—';

const SKIP_DIRS = [
  path.join('docs', 'history', 'project-universe-site'), // frozen archive
  path.join('docs', 'website', '_includes', 'canon'),    // auto-generated mirror
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

// Replace em dashes in a single prose line (NOT inside a code fence).
// Order matters: handle the spaced form first, then edge forms.
// In a heading line the em dash is usually a title/subtitle separator, so a
// colon reads better there than a comma.
function fixLine(line, isHeading) {
  let s = line;
  if (isHeading) {
    // First spaced em dash -> ": "; any remaining -> ", ".
    s = s.replace(new RegExp('\\s+' + EM + '\\s+'), ': ');
  }
  // " — "  -> ", "   (the dominant parenthetical/appositive break)
  s = s.replace(new RegExp('\\s+' + EM + '\\s+', 'g'), ', ');
  // "word— word" or "word —word" leftovers -> ", "
  s = s.replace(new RegExp('\\s*' + EM + '\\s*', 'g'), ', ');
  // tidy ONLY the artifacts an em-dash->comma swap can create. Do NOT touch
  // commas generally (e.g. never reformat "1,000" or list commas).
  s = s.replace(/,\s*,/g, ',');  // ", ," -> ","   (two breaks collided)
  s = s.replace(/ +,/g, ',');    // " ,"  -> ","   (space we left before a comma)
  s = s.replace(/\(, +/g, '(');  // "(, x" -> "(x"  (em dash right after a paren)
  s = s.replace(/ +,\)/g, ')');  // "x ,)" -> "x)"
  return s;
}

let totalFiles = 0;
let changedFiles = 0;
let totalReplaced = 0;
const remaining = [];

for (const file of walk(path.join(ROOT, 'docs'), [])) {
  totalFiles++;
  const orig = fs.readFileSync(file, 'utf8');
  if (!orig.includes(EM)) continue;

  const lines = orig.split('\n');
  let inFence = false;
  let fileReplaced = 0;
  const out = lines.map((line) => {
    const t = line.trimStart();
    if (t.startsWith('```') || t.startsWith('~~~')) {
      inFence = !inFence;
      return line; // never touch the fence line itself
    }
    if (inFence) return line; // preserve code block contents verbatim
    if (!line.includes(EM)) return line;
    const before = (line.match(new RegExp(EM, 'g')) || []).length;
    const isHeading = /^#{1,6}\s/.test(line);
    const fixed = fixLine(line, isHeading);
    fileReplaced += before;
    return fixed;
  });
  const result = out.join('\n');

  // Count any em dashes still present (inside code fences we intentionally kept).
  const left = (result.match(new RegExp(EM, 'g')) || []).length;
  if (left > 0) remaining.push(`${path.relative(ROOT, file)}: ${left} (in code blocks, left as-is)`);

  if (result !== orig) {
    changedFiles++;
    totalReplaced += fileReplaced;
    if (!DRY) fs.writeFileSync(file, result);
  }
}

console.log(`${DRY ? '[dry-run] ' : ''}docs scanned: ${totalFiles}`);
console.log(`files changed: ${changedFiles}`);
console.log(`em dashes replaced (prose): ${totalReplaced}`);
if (remaining.length) {
  console.log(`\nem dashes left inside code blocks (review manually if any are prose):`);
  remaining.forEach((r) => console.log('  ' + r));
}
