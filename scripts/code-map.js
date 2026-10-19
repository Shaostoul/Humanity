#!/usr/bin/env node
// code-map: one line per source file, so an AI (or a person) can see what exists without opening
// hundreds of files (operator, 2026-10-18: "a series of hyper efficient memory files for AI ... to
// easily know everything that already exists without having to scan the source files").
//
// Writes docs/ai/code-map.md: every .rs file under src/ and every .js file under web/ (not the
// generated app/web copy), each as
//   `path` (lines): what its header comment says it is. Main items: a, b, c
// where the header is the file's first `//!` line (Rust) or first comment line (JS), and the main
// items are its top-level `pub` / `pub(crate)` fns, structs, enums, traits and consts (Rust) or its
// top-level functions (JS), at most ITEMS_MAX of them. Grouped by folder, folders in path order.
//
// GENERATED, never edited by hand: `just code-map` writes it, and `just brief` refreshes it at the
// start of every session, so it is never more than a session old. It is a map, not a reference:
// open the file it names before relying on a detail. A file whose header is missing shows "(no
// header comment)", which is itself worth fixing.
//
// Usage: node scripts/code-map.js [--check]   (--check: exit 1 if the file on disk is stale)

'use strict';
const fs = require('fs');
const path = require('path');

const REPO = path.resolve(__dirname, '..');
const OUT = path.join(REPO, 'docs', 'ai', 'code-map.md');
const ITEMS_MAX = 8;
const ABOUT_MAX = 200;
const SKIP_DIRS = new Set(['target', 'node_modules', 'vendor', '.git', '.claude', 'app']);

function walk(dir, ext, out) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (SKIP_DIRS.has(e.name)) continue;
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, ext, out);
    else if (e.name.endsWith(ext) && !e.name.endsWith('.min.js')) out.push(p);
  }
  return out;
}

/** The text up to its first full stop followed by a space (or all of it). */
function firstSentence(t) {
  const m = t.match(/^(.*?[.!?])(\s|$)/);
  return (m ? m[1] : t).trim();
}

/** The first sentence of a Rust file's header: its `//!` lines (or, for a child test file, its
 *  leading `//` lines), joined, up to the first full stop. */
function rustHeader(lines) {
  const doc = [];
  let marker = null;
  for (const l of lines.slice(0, 60)) {
    const inner = l.match(/^\s*\/\/!\s?(.*)$/);
    const plain = l.match(/^\s*\/\/(?![!/])\s?(.*)$/);
    const m = inner ? ['//!', inner[1]] : plain && marker !== '//!' ? ['//', plain[1]] : null;
    if (m) {
      if (marker && m[0] !== marker) break;
      marker = m[0];
      if (!m[1].trim()) {
        if (doc.length) break;
        continue;
      }
      if (!/^(SPDX|#)/.test(m[1].trim())) doc.push(m[1].trim());
      continue;
    }
    if (doc.length || (l.trim() && !l.trim().startsWith('#!'))) break;
  }
  return firstSentence(doc.join(' '));
}

/** Its top-level public items: `pub` or `pub(crate)` at column 0 (not inside an impl), types first,
 *  then functions, then constants. */
function rustItems(lines) {
  const out = [];
  for (const l of lines) {
    const m = l.match(/^pub(?:\(crate\))?\s+(?:async\s+|unsafe\s+|const\s+(?=fn))*(fn|struct|enum|trait|const|static|type|mod)\s+([A-Za-z_][A-Za-z0-9_]*)/);
    if (m && m[1] !== 'mod') out.push({ kind: m[1], name: m[2] });
  }
  const rank = (k) => (['struct', 'enum', 'trait', 'type'].includes(k) ? 0 : k === 'fn' ? 1 : 2);
  return out.sort((a, b) => rank(a.kind) - rank(b.kind)).map((i) => i.name);
}

/** A JS file's first comment line. */
function jsHeader(lines) {
  for (const l of lines.slice(0, 30)) {
    const m = l.match(/^\s*(?:\/\/+|\/\*\*?|\*)\s?(.*?)\s*(?:\*\/)?$/);
    if (m && m[1].trim() && !/^(eslint|@ts-|global |'use strict'|SPDX)/.test(m[1].trim())) return m[1].trim();
    if (l.trim() && !/^\s*(\/\/|\/\*|\*|'use strict'|"use strict")/.test(l)) break;
  }
  return '';
}

/** Its top-level functions (declared at column 0, or as `const x = (...) =>` at column 0/2). */
function jsItems(lines) {
  const out = [];
  for (const l of lines) {
    const m = l.match(/^(?:export\s+)?(?:async\s+)?function\s+([A-Za-z_$][\w$]*)/) || l.match(/^  (?:async\s+)?function\s+([A-Za-z_$][\w$]*)/);
    if (m) out.push(m[1]);
  }
  return out;
}

function clip(s, n) {
  return s.length > n ? s.slice(0, n - 3).trimEnd() + '...' : s;
}

function entry(file, kind) {
  const text = fs.readFileSync(file, 'utf8');
  const lines = text.split(/\r?\n/);
  const rel = path.relative(REPO, file).split(path.sep).join('/');
  const header = kind === 'rs' ? rustHeader(lines) : jsHeader(lines);
  const items = kind === 'rs' ? rustItems(lines) : jsItems(lines);
  const shown = items.slice(0, ITEMS_MAX).join(', ') + (items.length > ITEMS_MAX ? ` (+${items.length - ITEMS_MAX} more)` : '');
  // Source comments use em dashes freely; the map is a document people read, so they become commas.
  const about = header ? clip(header.replace(/\s*—\s*/g, ', '), ABOUT_MAX) : '(no header comment)';
  const name = path.posix.basename(rel);
  const test = /(_tests?\.rs|\.test\.js)$/.test(name) || name === 'tests.rs';
  return { test, name, dir: path.posix.dirname(rel), line: `- \`${name}\` (${lines.length}): ${about}${shown ? ` Main items: ${shown}.` : ''}`, lines: lines.length };
}

function build() {
  const rs = walk(path.join(REPO, 'src'), '.rs', []).map((f) => entry(f, 'rs'));
  const js = walk(path.join(REPO, 'web'), '.js', []).map((f) => entry(f, 'js'));
  const all = [...rs, ...js].sort((a, b) => (a.dir === b.dir ? a.line.localeCompare(b.line) : a.dir.localeCompare(b.dir)));
  const total = (list) => list.reduce((n, e) => n + e.lines, 0);
  const out = [
    '# Code map (generated)',
    '',
    '> **Generated by `scripts/code-map.js`; do not edit by hand.** `just code-map` writes it and `just brief`',
    '> refreshes it every session. One line per source file: what its header comment says it is, and its',
    '> main public items. It is a MAP: open the file before relying on a detail. Search it with',
    '> `grep -i <word> docs/ai/code-map.md` before searching the source. Features (what a person can do)',
    '> are in `docs/FEATURES.md`; pages in `docs/PAGES.md`.',
    '',
    `${rs.length} Rust files (${total(rs).toLocaleString('en-US')} lines) under \`src/\`; ${js.length} JavaScript files (${total(js).toLocaleString('en-US')} lines) under \`web/\`.`,
    '',
  ];
  // The folders first, each in one line (its mod.rs's, or its own file's, header), small enough to
  // read whole at the start of a session; then every file.
  const dirs = [...new Set(all.map((e) => e.dir))];
  out.push('## Folders', '');
  for (const dir of dirs) {
    const here = all.filter((e) => e.dir === dir);
    const own = here.find((e) => e.name === 'mod.rs') || all.find((e) => `${e.dir}/${e.name}` === `${dir}.rs`);
    const about = own ? own.line.replace(/^- `[^`]+` \(\d+\): /, '').replace(/ Main items: .*$/, '') : '';
    out.push(`- \`${dir}/\` (${here.length} files)${about && about !== '(no header comment)' ? ': ' + clip(about, 140) : ''}`);
  }
  out.push('', '## Files');
  // One heading a folder; its test files on one line at the end (they are named for what they test).
  for (const dir of dirs) {
    const here = all.filter((e) => e.dir === dir);
    out.push('', `## ${dir}/`, '');
    for (const e of here.filter((e) => !e.test)) out.push(e.line);
    const tests = here.filter((e) => e.test);
    if (tests.length) out.push('- tests: ' + tests.map((e) => '`' + e.name + '`').join(', '));
  }
  return out.join('\n') + '\n';
}

const text = build();
if (process.argv.includes('--check')) {
  const now = fs.existsSync(OUT) ? fs.readFileSync(OUT, 'utf8').replace(/\r\n/g, '\n') : '';
  if (now !== text) {
    console.error('code-map: docs/ai/code-map.md is stale; run `just code-map`');
    process.exit(1);
  }
  console.log('code-map: up to date');
} else {
  fs.mkdirSync(path.dirname(OUT), { recursive: true });
  fs.writeFileSync(OUT, text);
  console.log(`code-map: wrote ${path.relative(REPO, OUT)} (${Math.round(text.length / 1024)} KB, ${text.split('\n').length} lines)`);
}
