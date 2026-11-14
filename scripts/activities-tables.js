#!/usr/bin/env node
// Regenerate section 3's catalog tables in docs/design/activities.md from
// data/gameplay/activities.json (2026-11-14). The data file is the source of truth; the
// tables are its readable copy, between the two CATALOG TABLES markers. Run it after
// changing an activity:
//
//   node scripts/activities-tables.js
//
// tests/activities_catalog_lint.rs (in `just lints`) fails when an id is missing from the
// document or its table row shows a different status than the data, so a stale table is
// caught rather than trusted.

const fs = require('fs');
const path = require('path');

const root = path.join(__dirname, '..');
const docPath = path.join(root, 'docs/design/activities.md');
const cat = JSON.parse(fs.readFileSync(path.join(root, 'data/gameplay/activities.json'), 'utf8'));
const BEGIN = '<!-- CATALOG TABLES: generated from data/gameplay/activities.json by scripts/activities-tables.js; regenerate rather than edit -->';
const END = '<!-- END CATALOG TABLES -->';

const short = (p) => p.replace('crates/humanity-systems/src/', 'systems/').replace('crates/humanity-ship/src/', 'ship/');
const cell = (s) => String(s).replace(/\|/g, '\\|');

const out = [];
for (const f of cat.families) {
  out.push(`### ${f.name}`, '', '| Activity | Status | Skill | Evidence | Note |', '|---|---|---|---|---|');
  for (const a of cat.activities.filter((x) => x.family === f.id)) {
    const ev = [];
    if (a.features_heading) ev.push(`FEATURES: "${a.features_heading}"`);
    for (const p of a.evidence.slice(0, 2)) ev.push('`' + short(p) + '`');
    if (a.design_doc) ev.push('design: `' + a.design_doc.replace('docs/design/', '').replace('docs/game/', 'game/') + '`');
    const st = a.status + (a.operator_to_decide ? ', operator to decide' : '');
    out.push(`| \`${a.id}\` ${cell(a.name)} | ${st} | ${a.skill || 'none'} | ${cell(ev.join('; ') || 'none')} | ${cell(a.note)} |`);
  }
  out.push('');
}

const raw = fs.readFileSync(docPath, 'utf8');
const eol = raw.includes('\r\n') ? '\r\n' : '\n';
const doc = raw.replace(/\r\n/g, '\n');
const b = doc.indexOf(BEGIN);
if (b < 0) {
  console.error('activities-tables: the BEGIN marker is missing from docs/design/activities.md');
  process.exit(1);
}
const e = doc.indexOf(END);
const before = doc.slice(0, b + BEGIN.length);
const after = e >= 0 ? doc.slice(e) : END + doc.slice(b + BEGIN.length);
const next = (before + '\n\n' + out.join('\n') + '\n' + after).replace(/\n/g, eol);
fs.writeFileSync(docPath, next);
console.log(`activities-tables: ${cat.activities.length} activities in ${cat.families.length} families written`);
