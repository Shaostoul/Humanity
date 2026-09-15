#!/usr/bin/env node
// Curriculum completeness report, and the gate that keeps the syllabus honest.
//
// WHY THIS EXISTS
// "How complete is the Library?" had no answer before 2026-09-15 because there
// was no denominator. data/curriculum/syllabus.json is that denominator; this
// script is how you read it.
//
// It also VALIDATES, because a syllabus that references a skill or a document
// that does not exist is worse than no syllabus: it reports coverage that is
// not there. Every `skills` entry must exist in data/skills/skills.csv, every
// `reading` slug must exist in the shipped Library, and every `prerequisites`
// id must be a real topic. Unknown references exit non-zero.
//
// Usage:
//   node scripts/curriculum-status.js           # summary
//   node scripts/curriculum-status.js --gaps    # every absent topic, by subject
//   node scripts/curriculum-status.js --hazard  # lethal/serious topics and their state

const fs = require('fs');

const SYLLABUS = 'data/curriculum/syllabus.json';
const SKILLS = 'data/skills/skills.csv';
const LIBRARY = 'data/library/index.json';

function read(file, what) {
  if (!fs.existsSync(file)) {
    console.error('missing ' + what + ': ' + file);
    process.exit(1);
  }
  return fs.readFileSync(file, 'utf8');
}

const syl = JSON.parse(read(SYLLABUS, 'syllabus'));
const topics = syl.topics || [];
const subjects = syl.subjects || [];

// ── Real skill ids, from the game's own skill tree ──
const skillIds = new Set(
  read(SKILLS, 'skills')
    .split(/\r?\n/)
    .slice(1)
    .map(l => l.split(',')[0].trim())
    .filter(Boolean)
);

// ── Real Library slugs, derived the same way both clients derive them ──
const librarySlugs = new Set();
if (fs.existsSync(LIBRARY)) {
  const idx = JSON.parse(fs.readFileSync(LIBRARY, 'utf8'));
  for (const c of idx.categories || []) {
    for (const d of c.docs || []) {
      librarySlugs.add(d.file.replace(/\.md$/i, '').replace(/_/g, '-').toLowerCase());
    }
  }
}

const topicIds = new Set(topics.map(t => t.id));
const subjectIds = new Set(subjects.map(s => s.id));

// ── Validation ──
const errors = [];
const seen = new Set();
for (const t of topics) {
  if (seen.has(t.id)) errors.push('duplicate topic id: ' + t.id);
  seen.add(t.id);
  if (!subjectIds.has(t.subject)) errors.push(t.id + ': unknown subject "' + t.subject + '"');
  for (const s of t.skills || []) {
    if (!skillIds.has(s)) errors.push(t.id + ': unknown skill "' + s + '" (not in ' + SKILLS + ')');
  }
  for (const r of t.reading || []) {
    if (librarySlugs.size && !librarySlugs.has(r)) {
      errors.push(t.id + ': reading "' + r + '" is not a shipped Library document');
    }
  }
  for (const p of t.prerequisites || []) {
    if (!topicIds.has(p)) errors.push(t.id + ': unknown prerequisite "' + p + '"');
  }
  // A topic claiming to be written must name where it is written.
  if (t.status !== 'absent' && !(t.reading || []).length) {
    errors.push(t.id + ': status "' + t.status + '" but no reading listed');
  }
  // The rule that matters most: a topic that can kill someone must not ship
  // half-written. Nothing enforces this except this line.
  if (t.hazard === 'lethal' && t.status === 'stub') {
    errors.push(t.id + ': hazard "lethal" must not sit at status "stub"');
  }
}

// ── Report ──
const byStatus = {};
for (const t of topics) byStatus[t.status] = (byStatus[t.status] || 0) + 1;
const done = (byStatus.verified || 0);
const pct = topics.length ? (100 * done / topics.length) : 0;

console.log('CURRICULUM: ' + topics.length + ' topics across ' + subjects.length + ' subjects');
console.log('');
console.log('  verified (teaching-grade) : ' + String(byStatus.verified || 0).padStart(3) + '   ' + pct.toFixed(1) + '%');
console.log('  sourced                   : ' + String(byStatus.sourced || 0).padStart(3));
console.log('  stub                      : ' + String(byStatus.stub || 0).padStart(3));
console.log('  absent                    : ' + String(byStatus.absent || 0).padStart(3));
console.log('');

// The four layers. A topic is a complete EXPERIENCE only when all four exist.
const layer = { reading: 0, skills: 0, data: 0, sources: 0, all_four: 0 };
for (const t of topics) {
  const r = (t.reading || []).length > 0;
  const s = (t.skills || []).length > 0;
  const d = (t.data || []).length > 0;
  const c = (t.sources || []).length > 0;
  if (r) layer.reading++;
  if (s) layer.skills++;
  if (d) layer.data++;
  if (c) layer.sources++;
  if (r && s && d && c) layer.all_four++;
}
const pc = n => (100 * n / topics.length).toFixed(0) + '%';
console.log('  FOUR LAYERS (a topic is a complete experience only with all four)');
console.log('    has a document      : ' + String(layer.reading).padStart(3) + '  ' + pc(layer.reading));
console.log('    levels a skill      : ' + String(layer.skills).padStart(3) + '  ' + pc(layer.skills));
console.log('    has simulation data : ' + String(layer.data).padStart(3) + '  ' + pc(layer.data));
console.log('    cites a source      : ' + String(layer.sources).padStart(3) + '  ' + pc(layer.sources));
console.log('    ALL FOUR            : ' + String(layer.all_four).padStart(3) + '  ' + pc(layer.all_four));
console.log('');

const locale = topics.filter(t => t.locale_dependent).length;
console.log('  locale-dependent topics   : ' + locale + '  (' + pc(locale) + ' need real data about a real place)');
const lethal = topics.filter(t => t.hazard === 'lethal');
const lethalDone = lethal.filter(t => t.status === 'verified').length;
console.log('  can kill if taught wrong  : ' + lethal.length + ', of which ' + lethalDone + ' verified');
console.log('');

console.log('  BY SUBJECT');
for (const s of subjects.slice().sort((a, b) => a.order - b.order)) {
  const ts = topics.filter(t => t.subject === s.id);
  const v = ts.filter(t => t.status === 'verified').length;
  const bar = '#'.repeat(Math.round(10 * v / Math.max(ts.length, 1))).padEnd(10, '.');
  console.log('    ' + bar + '  ' + String(v) + '/' + String(ts.length).padEnd(3) + ' ' + s.title);
}

if (process.argv.includes('--gaps')) {
  console.log('');
  console.log('  ABSENT TOPICS');
  for (const s of subjects.slice().sort((a, b) => a.order - b.order)) {
    const ts = topics.filter(t => t.subject === s.id && t.status === 'absent');
    if (!ts.length) continue;
    console.log('    ' + s.title);
    for (const t of ts) {
      const h = t.hazard && t.hazard !== 'none' ? '  [' + t.hazard + ']' : '';
      console.log('      - ' + t.id.padEnd(28) + t.title + h);
    }
  }
}

if (process.argv.includes('--hazard')) {
  console.log('');
  console.log('  HAZARDOUS TOPICS (write these carefully or not at all)');
  for (const t of topics.filter(t => t.hazard === 'lethal' || t.hazard === 'serious')
    .sort((a, b) => (a.hazard === 'lethal' ? 0 : 1) - (b.hazard === 'lethal' ? 0 : 1))) {
    console.log('    [' + (t.hazard || '').padEnd(7) + '] ' + t.status.padEnd(8) + ' ' + t.id);
  }
}

if (errors.length) {
  console.error('');
  console.error(errors.length + ' SYLLABUS ERROR(S):');
  for (const e of errors) console.error('  - ' + e);
  console.error('');
  console.error('A syllabus that points at a skill or document that does not exist reports');
  console.error('coverage it does not have. Fix the reference or the data file.');
  process.exit(1);
}
console.log('');
console.log('syllabus validates: every skill, document and prerequisite reference resolves');
