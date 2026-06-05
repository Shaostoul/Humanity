#!/usr/bin/env node
// Replace em-dashes (U+2014) that sit INSIDE string literals with ", " across a
// source tree. Operator rule: no em-dashes in user-facing copy (they read as
// machine-written). We target string literals (single AND multi-line, including
// Rust `\`-continuations) and leave code comments alone where possible.
//
// SAFETY: U+2014 is never code syntax in Rust or JS, so even if the greedy
// string regex over-matches a span, the only characters ever rewritten are
// em-dashes. Nothing executable changes. `cargo check` / the build confirm it.
//
// Usage: node scripts/strip-emdash-strings.js <dir> <comma-separated-exts>
//   node scripts/strip-emdash-strings.js src/gui .rs

const fs = require('fs');
const path = require('path');

const EM = '—'; // em-dash, escaped so file encoding can't corrupt the match
const AROUND = new RegExp(' *' + EM + ' *', 'g'); // collapse spaces around it

function walk(dir, exts, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, exts, out);
    else if (exts.some((x) => e.name.endsWith(x))) out.push(p);
  }
  return out;
}

// Match string literals: a double quote, any run of (escaped char | non-quote
// non-backslash, including newlines), a closing quote. `s` flag lets it span
// lines for multi-line literals.
const STRING_RE = /"(?:\\.|[^"\\])*"/gs;

function strip(content) {
  let n = 0;
  const out = content.replace(STRING_RE, (str) => {
    if (!str.includes(EM)) return str;
    n += str.split(EM).length - 1;
    let s = str.replace(AROUND, ', ');
    // A literal that was ONLY an em-dash placeholder became `", "` — make it a
    // plain hyphen instead of a stray comma.
    if (s === '", "') s = '"-"';
    return s;
  });
  return { out, n };
}

const dir = process.argv[2] || 'src/gui';
const exts = (process.argv[3] || '.rs').split(',');
let total = 0;
const touched = [];
for (const f of walk(dir, exts)) {
  const c = fs.readFileSync(f, 'utf8');
  const { out, n } = strip(c);
  if (n > 0 && out !== c) {
    fs.writeFileSync(f, out);
    total += n;
    touched.push(`${path.relative('.', f)} (${n})`);
  }
}
console.log(`em-dashes rewritten inside string literals: ${total} across ${touched.length} files`);
touched.forEach((t) => console.log('  ' + t));
