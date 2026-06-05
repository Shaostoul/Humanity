#!/usr/bin/env node
// Sweep em-dashes (U+2014) from the web app's user-facing copy. Unlike Rust,
// HTML text isn't quoted, so this is a global replace per file (em-dash is never
// HTML/JS/CSS syntax, so it can only ever live in text, strings, or comments —
// all safe to rewrite). Rules: spaced prose dash -> ", "; any remaining dash
// (ranges, compounds, dividers) -> "-".
//
// EXCLUDES crypto/encoding/data files where a stray rewrite has no upside and a
// theoretical downside (their dashes are only ever in comments anyway).

const fs = require('fs');
const path = require('path');

const EXCLUDE = new Set([
  'canonical-cbor.js',
  'bip39-english.js',
  'pq-identity.js',
  'pq-object.js',
  'pq-relay-auth.js',
  'qrcode.js',
]);
const EXTS = ['.html', '.js', '.css'];
const EM = '—';

function walk(dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else if (EXTS.some((x) => e.name.endsWith(x)) && !EXCLUDE.has(e.name)) out.push(p);
  }
  return out;
}

let total = 0;
const touched = [];
for (const f of walk('web')) {
  const c = fs.readFileSync(f, 'utf8');
  if (!c.includes(EM)) continue;
  const n = c.split(EM).length - 1;
  const out = c
    .replace(new RegExp(' +' + EM + ' +', 'g'), ', ') // spaced prose clause
    .replace(new RegExp(EM, 'g'), '-'); // ranges / compounds / dividers
  if (out !== c) {
    fs.writeFileSync(f, out);
    total += n;
    touched.push(`${path.relative('.', f)} (${n})`);
  }
}
console.log(`em-dashes rewritten in web copy: ${total} across ${touched.length} files`);
touched.forEach((t) => console.log('  ' + t));
