#!/usr/bin/env node
// Sweep em-dashes (U+2014) from user-facing DATA content (glossary, quests,
// item / plant / trade descriptions, onboarding, constellations, etc.).
//
// Per-format rules (em-dashes only ever live INSIDE quoted strings / field
// text / comments in these formats, never in structural syntax):
//   .csv             -> "—" becomes " - " (spaced hyphen). NEVER a comma: a
//                       comma is a CSV column delimiter and would split the
//                       field, corrupting the row. Only spaces/tabs around the
//                       dash are consumed (never a newline, which would merge
//                       two rows).
//   .json/.ron/.toml -> spaced prose dash becomes ", "; any remaining dash
//                       becomes "-". Commas are safe inside the quoted values
//                       where these dashes live.
//
// EXCLUDES (not user-facing app copy, or must not be rewritten):
//   data/coordination/**   internal AI session journal / registry / state
//   *.md                   developer docs
//   announcements_archive.json, legacy_channel_content.json
//                          historical message records (rewriting edits the past)

const fs = require('fs');
const path = require('path');

const EM = '—';
const EXCLUDE_BASENAMES = new Set([
  'announcements_archive.json',
  'legacy_channel_content.json',
]);
const EXTS = ['.json', '.ron', '.toml', '.csv'];

function walk(dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (e.name === 'coordination') continue; // skip internal coordination tree
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, out);
    else if (EXTS.includes(path.extname(e.name)) && !EXCLUDE_BASENAMES.has(e.name)) out.push(p);
  }
  return out;
}

const CSV_RE = new RegExp('[ \\t]*' + EM + '[ \\t]*', 'g');
const PROSE_RE = new RegExp(' +' + EM + ' +', 'g');
const ANY_RE = new RegExp(EM, 'g');

let total = 0;
const touched = [];
for (const f of walk('data')) {
  const c = fs.readFileSync(f, 'utf8');
  if (!c.includes(EM)) continue;
  const n = c.split(EM).length - 1;
  const out = f.endsWith('.csv')
    ? c.replace(CSV_RE, ' - ')
    : c.replace(PROSE_RE, ', ').replace(ANY_RE, '-');
  if (out !== c) {
    fs.writeFileSync(f, out);
    total += n;
    touched.push(`${path.relative('.', f)} (${n})`);
  }
}
console.log(`em-dashes rewritten in data content: ${total} across ${touched.length} files`);
touched.forEach((t) => console.log('  ' + t));

// Validate every JSON file in the tree still parses (catch any breakage now).
let checked = 0;
let bad = 0;
for (const f of walk('data')) {
  if (!f.endsWith('.json')) continue;
  checked++;
  try {
    JSON.parse(fs.readFileSync(f, 'utf8'));
  } catch (e) {
    bad++;
    console.error(`  x JSON parse FAILED: ${path.relative('.', f)} : ${e.message}`);
  }
}
console.log(`JSON validation: ${checked - bad}/${checked} parse OK`);
if (bad > 0) process.exit(1);
