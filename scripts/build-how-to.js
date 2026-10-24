#!/usr/bin/env node
// The How-to data's build and checker (docs/design/how-to.md, increment 1, 2026-10-23).
//
//   node scripts/build-how-to.js           check every how-to and write data/how_to/index.json
//   node scripts/build-how-to.js --check   check, and fail if index.json is not what it would write
//                                          (in `just lints` and `just validate-data`)
//
// What it does, every run:
//   1. Reads the shape from schemas/how_to.toml (its [fields."..."] tables): the schema file IS the
//      shape check, so the two cannot disagree.
//   2. Checks data/how_to/categories.json and planned.json (ids unique, categories real).
//   3. Checks every data/how_to/<top category>/<id>.json with web/shared/how-to-logic.js `check`
//      (the same gates and codes as src/how_to/check.rs) against the real data: items, recipes,
//      skills, Curriculum topics, Library documents and tags, the sources registry and its roles,
//      the Laws page's places, the app's pages and Settings sections, the files in this repository.
//   4. Runs every case of tests/fixtures/how_to/broken.json: each must be refused with its code,
//      each `passes` edit must leave no refusal, every code must have a case, and this file's codes
//      must be src/how_to/check.rs's. That is how each gate is seen red on every run.
//   5. Holds its canonical CBOR to web/shared/canonical-cbor.js, the encoder the website's signed
//      objects use (the Rust side is held by src/how_to/tests.rs comparing index.json's hashes).
//   6. Writes data/how_to/index.json (the list the website reads first; each how-to's hash, its
//      review state and the words search reads), and prints the gaps: written and planned by branch.
//
// Exit 1 on any refusal, any fixture that did not go red, a code on one side only, or (with --check)
// a stale index. Warnings are printed and do not fail.

'use strict';

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const L = require(path.join(ROOT, 'web/shared/how-to-logic.js'));
const blake3 = require(path.join(ROOT, 'web/shared/blake3.js'));
const CHECK = process.argv.includes('--check');
// --show-red prints each fixture's refusal, the evidence that its gate fired.
const SHOW_RED = process.argv.includes('--show-red');
const DIR = path.join(ROOT, 'data/how_to');

const failures = [];
function fail(msg) { failures.push(msg); }
function read(rel) { return fs.readFileSync(path.join(ROOT, rel), 'utf8'); }
function json(rel) { return JSON.parse(read(rel)); }
function set(list) { const o = Object.create(null); list.forEach((x) => { o[x] = true; }); return o; }

function csvIds(rel) {
  return set(read(rel).split(/\r?\n/).map((l) => l.trim()).filter((l) => l && l[0] !== '#' && !l.startsWith('id,')).map((l) => l.split(',')[0].trim()).filter(Boolean));
}

/** Page ids from src/gui/dispatch.rs `page_id`, and Settings' sections from settings.rs SECTIONS. */
function appPages() {
  const dispatch = read('src/gui/dispatch.rs');
  const body = dispatch.slice(dispatch.indexOf('pub fn page_id'), dispatch.indexOf('pub fn page_from_id'));
  const pages = Object.create(null);
  for (const m of body.matchAll(/GuiPage::\w+\s*=>\s*"([a-z_]+)"/g)) pages[m[1]] = [];
  const settings = read('src/gui/pages/settings.rs');
  const block = settings.slice(settings.indexOf('pub const SECTIONS'), settings.indexOf('];', settings.indexOf('pub const SECTIONS')));
  pages.settings = [...block.matchAll(/SettingsCategory::(\w+)/g)].map((m) => m[1].toLowerCase());
  if (Object.keys(pages).length < 20 || pages.settings.length < 10) fail('could not read the app\'s pages from src/gui/dispatch.rs and src/gui/pages/settings.rs');
  return pages;
}

function libraryReading() {
  const out = [];
  (function walk(v) {
    if (Array.isArray(v)) v.forEach(walk);
    else if (v && typeof v === 'object') {
      if (typeof v.file === 'string') out.push(v.file.replace(/\.md$/i, '').replace(/_/g, '-').toLowerCase());
      Object.values(v).forEach(walk);
    }
  })(json('data/library/index.json'));
  return set(out);
}

function tops(categories) {
  const out = Object.create(null);
  (function add(nodes, top) { nodes.forEach((n) => { out[n.id] = top || n.id; add(n.children || [], top || n.id); }); })(categories);
  return out;
}

// ── The shape and the tree ──
const toml = read('schemas/how_to.toml');
const schema = L.parseSchema(toml, 'fields');
const categoryShape = L.parseSchema(toml, 'category_fields');
const plannedShape = L.parseSchema(toml, 'planned_fields');
if (Object.keys(schema).length < 100) fail('schemas/how_to.toml: only ' + Object.keys(schema).length + ' fields read; the parser lost its place');

const categories = json('data/how_to/categories.json').categories || [];
const planned = json('data/how_to/planned.json').planned || [];
const catIds = Object.create(null);
(function walk(nodes) {
  nodes.forEach((n) => {
    // A node checked against [category_fields]: children are checked as nodes in their turn.
    const flat = Object.assign({}, n); delete flat.children;
    const s = Object.assign({}, categoryShape); delete s.children;
    L.shapeErrors(flat, s).forEach((e) => fail('data/how_to/categories.json `' + n.id + '`: ' + e));
    if (catIds[n.id]) fail('data/how_to/categories.json: the id `' + n.id + '` is used twice');
    catIds[n.id] = true;
    walk(n.children || []);
  });
})(categories);
const planIds = Object.create(null);
planned.forEach((p) => {
  L.shapeErrors(p, plannedShape).forEach((e) => fail('data/how_to/planned.json `' + p.id + '`: ' + e));
  if (planIds[p.id]) fail('data/how_to/planned.json: `' + p.id + '` is listed twice');
  planIds[p.id] = true;
  if (!catIds[p.category]) fail('data/how_to/planned.json `' + p.id + '`: category `' + p.category + '` is not in the tree');
});

// ── The context ──
const registry = json('data/sources/registry.json').sources || [];
const cx = {
  schema,
  blake3,
  categories: tops(categories),
  written: Object.create(null),
  planned: planIds,
  items: csvIds('data/items.csv'),
  recipes: csvIds('data/recipes.csv'),
  skills: csvIds('data/skills/skills.csv'),
  topics: set((json('data/curriculum/syllabus.json').topics || []).map((t) => t.id)),
  reading: libraryReading(),
  tags: set((json('data/library/tags.json').groups || []).flatMap((g) => g.tags.map((t) => t.id))),
  sources: Object.fromEntries(registry.map((s) => [s.id, s.kind === 'role'])),
  places: set((json('data/laws/laws.json').jurisdictions || []).map((j) => j.id)),
  pages: appPages(),
  actions: Object.create(null),
  repoHas: (p) => fs.existsSync(path.join(ROOT, p)),
};

// ── The how-tos ──
const files = [];
for (const top of fs.readdirSync(DIR, { withFileTypes: true }).filter((d) => d.isDirectory()).map((d) => d.name).sort()) {
  for (const f of fs.readdirSync(path.join(DIR, top)).sort()) {
    if (f.endsWith('.json') && f.split('.').length === 2) files.push(top + '/' + f);
  }
}
const texts = files.map((f) => [f, fs.readFileSync(path.join(DIR, f), 'utf8')]);
for (const [f, t] of texts) {
  try { const id = JSON.parse(t).id; if (typeof id === 'string' && !cx.written[id]) cx.written[id] = f; } catch (_) { /* the checker says so */ }
}
const entries = [];
const warnings = [];
for (const [f, t] of texts) {
  const { findings, howTo } = L.check(f, t, cx);
  L.refusals(findings).forEach((x) => fail('data/how_to/' + f + ': ' + x.code + ': ' + x.message));
  findings.filter((x) => L.SEVERITY[x.code] === 'warn').forEach((x) => warnings.push('data/how_to/' + f + ': ' + x.code + ': ' + x.message));
  if (!howTo || L.refusals(findings).length) continue;
  const words = Array.from(new Set(L.searchText(howTo).split(/[^a-z0-9']+/).filter((w) => w.length > 1))).join(' ');
  entries.push({
    id: howTo.id,
    file: f,
    title: howTo.title,
    summary: howTo.summary,
    category: howTo.category,
    top: cx.categories[howTo.category],
    audience: howTo.audience || 'everyone',
    realities: howTo.realities,
    apps: howTo.apps || [],
    time: howTo.time,
    difficulty: howTo.difficulty,
    hazard: howTo.hazard,
    review: { status: howTo.review.status, checked: howTo.review.checked || null, person: howTo.review.person || null },
    written_by: howTo.written_by,
    makes: howTo.makes || null,
    steps: howTo.steps.length,
    hash: L.hash(howTo, blake3),
    search: words,
  });
}
entries.sort((a, b) => a.title.toLowerCase().localeCompare(b.title.toLowerCase()));

// ── Every gate, seen red ──
const fixture = json('tests/fixtures/how_to/broken.json');
const base = json('tests/fixtures/how_to/base.json');
function apply(doc, e) {
  const parts = e.path.replace(/^\//, '').split('/').map((p) => p.replace(/~1/g, '/').replace(/~0/g, '~'));
  const last = parts.pop();
  let at = doc;
  for (const p of parts) {
    if (Array.isArray(at)) at = at[+p];
    else { if (!(p in at)) at[p] = {}; at = at[p]; }
  }
  let value;
  if (e.fill !== undefined) value = 'x'.repeat(e.fill);
  else if (e.hash) value = L.hash(doc, blake3);
  else value = e.set;
  if (e['delete']) { if (Array.isArray(at)) at.splice(+last, 1); else delete at[last]; }
  else if (Array.isArray(at) && last === '-') at.push(value);
  else if (Array.isArray(at)) at[+last] = value;
  else at[last] = value;
}
function caseText(c) {
  if (c.raw !== undefined) return c.raw;
  const doc = JSON.parse(JSON.stringify(base));
  (c.edits || []).forEach((e) => apply(doc, e));
  return JSON.stringify(doc, null, 2);
}
const red = new Set();
for (const c of fixture.cases) {
  const { findings } = L.check(c.file || fixture.base_file, caseText(c), cx);
  const hit = findings.find((x) => x.code === c.code);
  if (SHOW_RED && hit) console.log('red ' + c.code + ' (' + c.why + '): ' + hit.message);
  if (hit) red.add(c.code);
  else fail('broken.json: the gate ' + c.code + ' did not fire (' + c.why + '); got ' + JSON.stringify(findings.map((x) => x.code)));
}
const baseCheck = L.check(fixture.base_file, JSON.stringify(base, null, 2), cx);
L.refusals(baseCheck.findings).forEach((x) => fail('tests/fixtures/how_to/base.json is refused: ' + x.code + ': ' + x.message));
for (const p of fixture.passes) {
  L.refusals(L.check(p.file || fixture.base_file, caseText(p), cx).findings).forEach((x) => fail('broken.json pass "' + p.why + '" is refused: ' + x.code + ': ' + x.message));
}
const codes = L.CODES.map((c) => c[0]);
codes.filter((c) => !red.has(c)).forEach((c) => fail('broken.json has no case that shows the gate ' + c + ' red'));
const rustSrc = read('src/how_to/check.rs');
const idFn = rustSrc.slice(rustSrc.indexOf('pub fn id(self)'), rustSrc.indexOf('pub fn severity(self)'));
const rustCodes = [...idFn.matchAll(/Code::\w+\s*=>\s*"([a-z_]+)"/g)].map((m) => m[1]);
const onlyRust = rustCodes.filter((c) => !codes.includes(c));
const onlyJs = codes.filter((c) => !rustCodes.includes(c));
if (rustCodes.length < 30 || onlyRust.length || onlyJs.length) fail('the two checkers\' codes differ: only in src/how_to/check.rs ' + JSON.stringify(onlyRust) + ', only in web/shared/how-to-logic.js ' + JSON.stringify(onlyJs));

// ── The encoding, held to the website's signed-object encoder ──
async function encoderAgrees() {
  // Imported from its text as a data: URL: the file is an ES module without a package.json saying so,
  // which Node would warn about on every run.
  const C = await import('data:text/javascript,' + encodeURIComponent(read('web/shared/canonical-cbor.js')));
  function enc(v) {
    if (v === null) return new Uint8Array([0xf6]);
    if (v === true) return new Uint8Array([0xf5]);
    if (v === false) return new Uint8Array([0xf4]);
    if (typeof v === 'number') return C.cborUint(v);
    if (typeof v === 'string') return C.cborText(v);
    if (Array.isArray(v)) return C.cborArray(v.map(enc));
    return C.cborMap(Object.keys(v).map((k) => [C.cborText(k), enc(v[k])]));
  }
  const docs = [base].concat(texts.map(([, t]) => JSON.parse(t)));
  for (const d of docs) {
    const copy = Object.assign({}, d); delete copy.review;
    const a = Buffer.from(enc(copy)), b = Buffer.from(L.canonicalBytes(d));
    if (!a.equals(b)) fail('the canonical CBOR of ' + d.id + ' differs from web/shared/canonical-cbor.js');
  }
}

(async () => {
  await encoderAgrees();

  // ── The names of what the how-tos name, so the website shows words, not ids (the desktop reads
  //    them from the same files: src/how_to/library.rs read_names) ──
  const bodies = texts.map(([, t]) => { try { return JSON.parse(t); } catch (_) { return null; } }).filter(Boolean);
  const used = { items: new Set(), skills: new Set(), topics: new Set(), reading: new Set(), places: new Set(), roles: new Set() };
  bodies.forEach((b) => {
    (b.tools || []).concat(b.materials || []).forEach((n) => { if (n.item) used.items.add(n.item); });
    ((b.help && b.help.acts_on) || []).forEach((i) => used.items.add(i));
    (b.skills || []).forEach((s) => used.skills.add(s));
    (b.topics || []).forEach((t) => used.topics.add(t));
    (b.reading || []).forEach((r) => used.reading.add(r));
    (b.steps || []).forEach((s) => { if (s['if'] && s['if'].place) used.places.add(s['if'].place); });
    (b.stop_and_call || []).forEach((c) => { if (typeof c.who === 'string' && c.who.startsWith('role:')) used.roles.add(c.who.slice(5)); });
  });
  function csvNames(rel) {
    const out = {};
    read(rel).split(/\r?\n/).filter((l) => l && l[0] !== '#' && !l.startsWith('id,')).forEach((l) => { const c = l.split(','); if (c[0] && c[1]) out[c[0].trim()] = c[1].trim(); });
    return out;
  }
  function pick(all, ids) { const out = {}; [...ids].sort().forEach((id) => { if (all[id] !== undefined) out[id] = all[id]; }); return out; }
  const readingTitles = {};
  (function walk(v) {
    if (Array.isArray(v)) v.forEach(walk);
    else if (v && typeof v === 'object') {
      if (typeof v.file === 'string' && typeof v.title === 'string') readingTitles[v.file.replace(/\.md$/i, '').replace(/_/g, '-').toLowerCase()] = v.title;
      Object.values(v).forEach(walk);
    }
  })(json('data/library/index.json'));
  const names = {
    items: pick(csvNames('data/items.csv'), used.items),
    skills: pick(csvNames('data/skills/skills.csv'), used.skills),
    topics: pick(Object.fromEntries((json('data/curriculum/syllabus.json').topics || []).map((t) => [t.id, t.title])), used.topics),
    reading: pick(readingTitles, used.reading),
    places: pick(Object.fromEntries((json('data/laws/laws.json').jurisdictions || []).map((j) => [j.id, j.name])), used.places),
    roles: pick(Object.fromEntries(registry.filter((s) => s.kind === 'role').map((s) => [s.id, s.name])), used.roles),
  };
  // The place roles resolve in until the person's place is a saved setting (increment 7): the first
  // locale, as the desktop takes it.
  const localeDirs = fs.readdirSync(path.join(ROOT, 'data/locales')).filter((d) => fs.existsSync(path.join(ROOT, 'data/locales', d, 'locale.json'))).sort();
  let locale = null;
  if (localeDirs.length) {
    const lj = json('data/locales/' + localeDirs[0] + '/locale.json');
    const auth = {};
    [...used.roles].sort().forEach((r) => { const a = (lj.authorities || {})[r]; if (a) auth[r] = { name: a.name || '', url: a.url || '', note: a.note || '' }; });
    locale = { name: lj.region ? lj.name + ', ' + lj.region : lj.name, authorities: auth };
  }

  // ── The index ──
  const index = {
    _comment: 'GENERATED by scripts/build-how-to.js from data/how_to/: do not edit. The list the website reads first (web/pages/how-to-app.js), then each how-to\'s file on open. `hash` is BLAKE3 of the how-to\'s canonical CBOR without its review (src/how_to/canonical.rs); `search` is the words search reads, the problems\' signs among them; `names` and `locale` are the words for what the how-tos name, from the files the desktop reads them from. Planned how-tos are in planned.json.',
    how_tos: entries,
    names,
    locale,
    counts: { written: entries.length, planned: planned.length },
  };
  const want = JSON.stringify(index, null, 2) + '\n';
  const indexPath = path.join(DIR, 'index.json');
  const have = fs.existsSync(indexPath) ? fs.readFileSync(indexPath, 'utf8').replace(/\r\n/g, '\n') : '';
  if (CHECK) {
    if (have !== want) fail('data/how_to/index.json is not what the how-tos make: run node scripts/build-how-to.js');
  } else if (have !== want) {
    fs.writeFileSync(indexPath, want);
    console.log('wrote data/how_to/index.json');
  }

  // ── The gaps ──
  const perTop = {};
  categories.forEach((c) => { perTop[c.id] = [0, 0]; });
  entries.forEach((e) => { perTop[e.top][0]++; });
  planned.forEach((p) => { const t = cx.categories[p.category]; if (perTop[t]) perTop[t][1]++; });
  const gaps = categories.map((c) => c.title + ' ' + perTop[c.id][0] + '/' + (perTop[c.id][0] + perTop[c.id][1])).join(', ');
  warnings.forEach((w) => console.log('warning: ' + w));
  if (failures.length) {
    failures.forEach((f) => console.error('FAIL: ' + f));
    console.error('>> how-to: ' + failures.length + ' failure(s)');
    process.exit(1);
  }
  console.log('>> how-to ok: ' + entries.length + ' written, ' + planned.length + ' planned (' + gaps + '); ' + fixture.cases.length + ' fixtures red as expected, ' + red.size + ' of ' + codes.length + ' gates, ' + fixture.passes.length + ' allowed edits pass');
})().catch((e) => { console.error(e && e.stack ? e.stack : String(e)); process.exit(1); });
