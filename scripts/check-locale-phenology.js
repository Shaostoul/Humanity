#!/usr/bin/env node
// Validates every locale's phenology.json, and above all keeps its confidence
// labels honest.
//
// WHY THIS EXISTS
// phenology.json grades every event: `measured`, `published_range`,
// `local_consensus` or `estimated`. That grading is the file's best idea,
// because a date computed from thirty years of observations and a regional
// generalisation are not the same claim and a calendar that blurs them is worse
// than one with fewer entries.
//
// A grade only means something if it can be checked. On the day after the file
// shipped, a writer reading it found NINE events claiming `published_range`,
// which the file's own key defines as "a named source states this range", while
// carrying an empty sources array. Five of them were the lethal-plant timing
// events, including the camas bloom window that the species data calls the only
// safe window for telling camas from death camas.
//
// So: `published_range` and `measured` must carry a real source. A
// `local_consensus` event need not, because "nobody publishes this date" is
// precisely what that grade says, but it must still point somewhere, which in
// practice means the species record its timing derives from.
//
// The same writer noticed something the per-event labels hide, and it is worth
// a reader knowing in one sentence: not one of the biological events on the
// Silverdale calendar is `measured`. This script reports that ratio every run
// rather than leaving it to be rediscovered by adding up 68 labels.
//
//   node scripts/check-locale-phenology.js
//   node scripts/check-locale-phenology.js --quiet

const fs = require('fs');
const path = require('path');

const LOCALES = 'data/locales';
const QUIET = process.argv.includes('--quiet');

const CONFIDENCE = ['measured', 'published_range', 'local_consensus', 'estimated'];
const NEEDS_SOURCE = ['measured', 'published_range'];
const BIOLOGICAL = ['bloom', 'leaf_out', 'fruit', 'seed', 'run', 'migration', 'breeding'];
const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December'];

if (!fs.existsSync(LOCALES)) {
  console.log('No ' + LOCALES + ' directory; nothing to check.');
  process.exit(0);
}

const problems = [];
let locales = 0, events = 0;

for (const id of fs.readdirSync(LOCALES)) {
  const file = path.join(LOCALES, id, 'phenology.json');
  if (!fs.existsSync(file)) continue;          // optional per schemas/locale.toml
  locales++;

  let doc;
  try {
    doc = JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch (err) {
    problems.push(id + '/phenology.json: not valid JSON (' + err.message + ')');
    continue;
  }

  const speciesFile = path.join(LOCALES, id, 'species.json');
  const speciesIds = new Set(
    fs.existsSync(speciesFile)
      ? (JSON.parse(fs.readFileSync(speciesFile, 'utf8')).species || []).map(s => s.id)
      : []
  );

  const byId = new Map((doc.events || []).map(e => [e.id, e]));
  for (const e of doc.events || []) {
    events++;
    const where = id + ' "' + (e.name || e.id || '?') + '"';
    if (!CONFIDENCE.includes(e.confidence)) {
      problems.push(where + ': confidence ' + JSON.stringify(e.confidence) +
        ' is not one of ' + CONFIDENCE.join('/'));
    }
    const srcs = e.sources || [];
    if (!srcs.length) {
      problems.push(where + ': no sources at all. Even a local_consensus event ' +
        'must point at the record its timing comes from.');
    } else if (NEEDS_SOURCE.includes(e.confidence) && !srcs.some(s => /^https?:/.test(String(s)))) {
      problems.push(
        where + ': confidence "' + e.confidence + '" with no http source. That ' +
        'grade asserts somebody published this; either cite them, or grade it ' +
        '"local_consensus", which is what "we worked it out" is called.'
      );
    }
    if (e.species && speciesIds.size && !speciesIds.has(e.species)) {
      problems.push(where + ': species "' + e.species + '" is not in species.json');
    }
    if (!e.why_it_matters) {
      problems.push(where + ': no `why_it_matters`. An event a reader cannot act ' +
        'on is a fact, not a calendar entry.');
    }
  }

  const months = doc.months || [];
  if (months.length !== 12) {
    problems.push(id + ': ' + months.length + ' months, expected 12');
  }
  for (const m of months) {
    if (!MONTHS.includes(m.month)) problems.push(id + ': unknown month ' + JSON.stringify(m.month));
    for (const ref of m.events || []) {
      if (!byId.has(ref)) problems.push(id + ' ' + m.month + ': names unknown event "' + ref + '"');
    }
  }

  // The ratio worth stating out loud rather than leaving to be rediscovered.
  if (!QUIET) {
    const bio = (doc.events || []).filter(e => BIOLOGICAL.includes(e.kind));
    const bioMeasured = bio.filter(e => e.confidence === 'measured').length;
    const counts = (doc.events || []).reduce((a, e) => {
      a[e.confidence] = (a[e.confidence] || 0) + 1; return a;
    }, {});
    console.log(id + ': ' + (doc.events || []).length + ' events ' + JSON.stringify(counts));
    console.log('  biological events: ' + bio.length + ', of which measured: ' + bioMeasured +
      (bio.length && !bioMeasured
        ? '  <- none. Every measured event here is weather, water or tide.'
        : ''));
  }
}

if (!QUIET && problems.length) {
  console.log('');
  console.log('Problems:');
  for (const p of problems) console.log('  ' + p);
  console.log('');
}
console.log('Checked ' + events + ' phenology events across ' + locales +
  ' locale(s). Problems: ' + problems.length);
process.exit(problems.length ? 1 : 0);
