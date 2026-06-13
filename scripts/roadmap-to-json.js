#!/usr/bin/env node
// Generate data/roadmap.json from docs/ROADMAP.md so the public roadmap page renders
// the SAME content the maintainers edit by hand. ROADMAP.md is the single source of
// truth (it is both the public roadmap and the build to-do list); this script is the
// bridge to the website. Run it after editing ROADMAP.md:
//
//   node scripts/roadmap-to-json.js
//
// The web page (web/pages/roadmap-app.js) fetches /data/roadmap.json and renders the
// "Right now" queue, the themed sections with progress, and the recently-shipped list.

const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const SRC = path.join(ROOT, 'docs', 'ROADMAP.md');
const OUT = path.join(ROOT, 'data', 'roadmap.json');

// H2 sections that are explanatory prose, not roadmap data, skip them.
const SKIP_SECTIONS = new Set([
  'how to read this',
  'how this roadmap stays honest',
]);

const VALID_STATUS = new Set(['done', 'building', 'next', 'planned', 'future']);

// Detect a list-item START. Returns { status, head } (head = first-line remainder),
// or null if the line is not a list item. Continuation lines are joined separately so
// multi-line wrapped items keep their full text (and any trailing version token).
function startItem(line) {
  const m = line.match(/^\s*(?:[-*]|\d+\.)\s+(.*)$/);
  if (!m) return null;
  let head = m[1].trim();

  let status = null;
  const sm = head.match(/^`?\[([a-zA-Z][a-zA-Z-]*)\]`?\s*(.*)$/);
  if (sm) {
    status = sm[1].toLowerCase();
    head = sm[2].trim();
  }
  return { status, head };
}

// Given an item's fully-joined raw text, pull out a version and clean the markup.
function finalizeItem(it) {
  let rest = it.raw.trim();
  let version = null;

  // Leading `v0.422` token (recently-shipped form).
  const vm = rest.match(/^`(v[0-9][^`]*)`\s*(.*)$/);
  if (vm) {
    version = vm[1];
    rest = vm[2].trim();
  }
  // Or a trailing "(v0.342)" / "(v0.382 to v0.394)" version anywhere in the text.
  if (!version) {
    const tv = rest.match(/\((v0\.[0-9][^)]*)\)/);
    if (tv) version = tv[1];
  }
  // Drop a trailing "(v0.xxx ...)." parenthetical from the display text, the version
  // is shown as its own badge, so the words read cleanly without it.
  rest = rest.replace(/\s*\(v0\.[0-9][^)]*\)\s*\.?\s*$/, '.').replace(/\.\.$/, '.');

  // Strip markdown emphasis + stray inline-code backticks for a clean plain render
  // (the web escapes HTML anyway). Keep the words, drop the markup.
  const text = rest.replace(/\*\*/g, '').replace(/`/g, '').trim();
  return { status: it.status, version, text };
}

function isStructural(line) {
  const t = line.trim();
  return t === '' || t.startsWith('|') || t.startsWith('>') || t.startsWith('#') ||
    t.startsWith('---') || t.startsWith('```') || t.startsWith('~~~');
}

const md = fs.readFileSync(SRC, 'utf8');
const lines = md.split('\n');

const sections = [];
let cur = null;
let last = null;   // the in-progress item (for joining continuation lines)
let inFence = false;

for (const line of lines) {
  const t = line.trim();

  if (t.startsWith('```') || t.startsWith('~~~')) {
    inFence = !inFence;
    continue;
  }
  if (inFence) continue;

  const h2 = line.match(/^##\s+(.+?)\s*$/);
  if (h2) {
    const title = h2[1].trim();
    const key = title.toLowerCase();
    cur = {
      title,
      kind: key === 'right now' ? 'now'
        : key === 'recently shipped' ? 'recent'
        : SKIP_SECTIONS.has(key) ? 'skip'
        : 'theme',
      description: '',
      items: [],
    };
    sections.push(cur);
    last = null;
    continue;
  }

  if (!cur || cur.kind === 'skip') { last = null; continue; }

  const started = startItem(line);
  if (started) {
    last = { status: started.status, raw: started.head };
    cur.items.push(last);
    continue;
  }

  // Blank line ends the current item (markdown list-item boundary).
  if (t === '') { last = null; continue; }

  // Indented, non-structural continuation of the current item, join it.
  if (last && /^\s+\S/.test(line) && !isStructural(line)) {
    last.raw += ' ' + t;
    continue;
  }

  // First prose paragraph after a theme heading becomes its description.
  if (cur.kind === 'theme' && !cur.description && !isStructural(line)) {
    cur.description = t;
  }
}

// Finalize: join is done, now extract versions + clean markup.
for (const s of sections) {
  s.items = s.items.map((it) => {
    const f = finalizeItem(it);
    if (!f.status && s.kind === 'recent') f.status = 'done';
    return f;
  });
}

// Assemble the output shape.
const now = (sections.find((s) => s.kind === 'now') || { items: [] }).items;
const recent = (sections.find((s) => s.kind === 'recent') || { items: [] }).items;
const themes = sections
  .filter((s) => s.kind === 'theme')
  .map((s) => {
    const total = s.items.length;
    const done = s.items.filter((i) => i.status === 'done').length;
    return { title: s.title, description: s.description, items: s.items, done, total };
  });

// Sanity: warn on any unknown status tags so a typo cannot silently mis-render.
const unknown = [];
for (const s of sections) {
  for (const it of s.items) {
    if (it.status && !VALID_STATUS.has(it.status)) unknown.push(`${s.title}: [${it.status}] ${it.text}`);
  }
}
if (unknown.length) {
  console.warn('WARNING: unknown status tags (fix these in ROADMAP.md):');
  unknown.forEach((u) => console.warn('  ' + u));
}

const totalItems = themes.reduce((a, t) => a + t.total, 0);
const doneItems = themes.reduce((a, t) => a + t.done, 0);

const out = {
  source: 'docs/ROADMAP.md',
  note: 'Generated by scripts/roadmap-to-json.js. Do not edit by hand; edit ROADMAP.md and regenerate.',
  now,
  themes,
  recent,
  summary: { done: doneItems, total: totalItems },
};

fs.writeFileSync(OUT, JSON.stringify(out, null, 2) + '\n');
console.log(`Wrote ${path.relative(ROOT, OUT)}`);
console.log(`  ${now.length} active items, ${themes.length} themes, ${recent.length} recent releases`);
console.log(`  ${doneItems}/${totalItems} themed items done`);
