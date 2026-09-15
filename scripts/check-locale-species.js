#!/usr/bin/env node
// Validates every locale's species.json against the shape and the safety rules
// in schemas/locale.toml.
//
// WHY THIS EXISTS
// The Silverdale species data was researched by one set of agents and then put
// to an adversarial verifier per species. Forty-eight of sixty plant records
// were refuted, and the single most common finding, by a wide margin, was an
// EMPTY LOOKALIKE FIELD on something a person might eat.
//
// That is not a tidiness problem. Foraging is the one subject in the curriculum
// that kills people who trust it, and it kills them through confusion, not
// through ignorance: nobody eats water hemlock on purpose, they eat it thinking
// it is hemlock waterparsnip, which is edible and whose common name contains the
// word hemlock. A record that says "edible: yes" with no lookalikes is worse
// than no record, because it reads as a clearance.
//
// So the rule this enforces above all others: anything edible, and anything
// toxic, must name what it is confused with and give a field character a person
// can actually check. A species with genuinely no dangerous double says so
// explicitly, with one entry named "none in this region".
//
//   node scripts/check-locale-species.js
//   node scripts/check-locale-species.js --quiet

const fs = require('fs');
const path = require('path');

const LOCALES = 'data/locales';
const QUIET = process.argv.includes('--quiet');

const KEYS = ['id', 'common_name', 'scientific_name', 'taxon_rank', 'kind', 'form', 'status',
  'protected', 'edible', 'edible_detail', 'toxic', 'lookalikes', 'season', 'habitat',
  'uses', 'sources'];
const KIND = ['plant', 'fungus', 'mammal', 'bird', 'fish', 'shellfish', 'invertebrate', 'insect', 'reptile', 'amphibian'];
const STATUS = ['native', 'introduced', 'invasive', 'cultivated'];
const EDIBLE = ['no', 'yes', 'conditional'];
const DANGER = ['none', 'mild', 'serious', 'lethal'];

if (!fs.existsSync(LOCALES)) {
  console.log('No ' + LOCALES + ' directory; nothing to check.');
  process.exit(0);
}

let checkedLocales = 0;
let checkedSpecies = 0;
const problems = [];

for (const locale of fs.readdirSync(LOCALES)) {
  const file = path.join(LOCALES, locale, 'species.json');
  if (!fs.existsSync(file)) continue;   // optional per schemas/locale.toml
  checkedLocales++;

  let doc;
  try {
    doc = JSON.parse(fs.readFileSync(file, 'utf8'));
  } catch (err) {
    problems.push(locale + '/species.json: not valid JSON (' + err.message + ')');
    continue;
  }
  const species = doc.species || [];
  if (!Array.isArray(species) || !species.length) {
    problems.push(locale + '/species.json: no species array');
    continue;
  }

  const ids = new Set();
  for (const r of species) {
    checkedSpecies++;
    const where = locale + ' "' + (r.common_name || r.id || '?') + '"';

    for (const k of KEYS) if (!(k in r)) problems.push(where + ': missing field `' + k + '`');
    if (!KIND.includes(r.kind)) problems.push(where + ': kind ' + JSON.stringify(r.kind) + ' is not one of ' + KIND.join('/'));
    if (!STATUS.includes(r.status)) problems.push(where + ': status ' + JSON.stringify(r.status));
    if (!EDIBLE.includes(r.edible)) problems.push(where + ': edible ' + JSON.stringify(r.edible));
    if (r.id) {
      if (ids.has(r.id)) problems.push(where + ': duplicate id "' + r.id + '"');
      ids.add(r.id);
    }
    if (!Array.isArray(r.sources) || !r.sources.length) {
      problems.push(where + ': no sources. An unsourced species claim is a guess.');
    }
    // "conditional" means the condition is stated. Without it the word is just
    // a hedge, and a reader takes a hedge as a yes.
    if (r.edible === 'conditional' && !r.edible_detail) {
      problems.push(where + ': edible "conditional" with no condition stated in `edible_detail`');
    }

    if (!Array.isArray(r.lookalikes)) {
      problems.push(where + ': `lookalikes` is not an array');
      continue;
    }
    if (!r.lookalikes.length && (r.edible !== 'no' || r.toxic)) {
      problems.push(
        where + ': EMPTY `lookalikes` on a species that is edible or toxic. ' +
        'If it genuinely has no dangerous double here, say so with one entry ' +
        'named "none in this region".'
      );
    }
    for (const l of r.lookalikes) {
      if (!DANGER.includes(l.danger)) {
        problems.push(where + ': lookalike "' + (l.name || '?') + '" danger ' + JSON.stringify(l.danger));
      }
      if (!l.how_to_tell_them_apart) {
        problems.push(where + ': lookalike "' + (l.name || '?') + '" gives no way to tell them apart');
      }
    }
  }

  // A lookalike id must point at a record in this locale. Null is fine and
  // means "named, not yet in the dataset", which is a gap rather than a lie.
  for (const r of species) {
    for (const l of r.lookalikes || []) {
      if (l.id && !ids.has(l.id)) {
        problems.push(
          locale + ' "' + r.common_name + '": lookalike id "' + l.id +
          '" points at no species in this locale. Use null if it is not here yet.'
        );
      }
    }
  }
}

if (!QUIET && problems.length) {
  console.log('Problems in locale species data:');
  for (const p of problems) console.log('  ' + p);
  console.log('');
}
console.log(
  'Checked ' + checkedSpecies + ' species across ' + checkedLocales +
  ' locale(s). Problems: ' + problems.length
);
process.exit(problems.length ? 1 : 0);
