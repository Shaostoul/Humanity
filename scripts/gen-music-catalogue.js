#!/usr/bin/env node
// Writes data/music/catalogue.json, the website's copy of what the Music page's Explore cards and
// Written music list read (docs/design/music-page.md section 19): each genre (its words keys, its
// plain sentence, its tempo, the instruments it plays), each mood (where it plays, its tempo, whether
// it is heard as danger) and each written piece's catalogue details, all from the RON files the app
// reads (data/music/genres, data/music/moods, data/music/intensity.ron, data/music/scores/index.ron).
// The website serves tracked data/*.json only, and cannot read RON, hence this copy. Never edit the
// JSON by hand: change the RON and run `node scripts/gen-music-catalogue.js`;
// scripts/tests/music-web.test.js refuses a stale copy.
//
// Usage: node scripts/gen-music-catalogue.js [--check]   (--check: exit 1 if the file is stale)
'use strict';

const fs = require('fs');
const path = require('path');
const { parseRon } = require('./lib/ron-read');

const ROOT = path.resolve(__dirname, '..');
const MUSIC = path.join(ROOT, 'data', 'music');
const OUT = path.join(MUSIC, 'catalogue.json');
// The library's own defaults (crates/humanity-music/src/library.rs): listed first, the rest by id.
const DEFAULT_GENRE = 'mothership_calm';
const DEFAULT_MOOD = 'mothership_calm';

const readRon = (p) => parseRon(fs.readFileSync(p, 'utf8'));
const dirRon = (d) => fs.readdirSync(path.join(MUSIC, d)).filter((f) => f.endsWith('.ron')).sort().map((f) => readRon(path.join(MUSIC, d, f)));
const byDefault = (def) => (a, b) => (a.id !== def) - (b.id !== def) || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0);

/** The words keys of the instruments a genre plays, as the app's Explore card names them
 * (src/gui/pages/music/explore.rs `instrument_words`): a part's key where one is named, else the
 * instrument's own words key; its leads, its palette and its added parts, else the mood's roles. */
function instrumentKeys(g, moods, instruments) {
  const keyOf = (x) => (x.part ? `instrument_packs.part.${x.part}` : (instruments[x.instrument] || `music.instrument.${x.instrument}`));
  const out = [];
  for (const x of [...(g.leads || []), ...(g.palette || []), ...(g.roles_add || [])]) {
    const k = keyOf(x);
    if (!out.includes(k)) out.push(k);
  }
  if (!out.length) {
    const m = moods.find((x) => x.id === DEFAULT_MOOD) || moods[0];
    for (const r of (m && m.roles) || []) {
      const k = keyOf(r);
      if (!out.includes(k)) out.push(k);
    }
  }
  return out.slice(0, 6);
}

function build() {
  const instruments = {};
  for (const i of dirRon('instruments')) instruments[i.id] = i.words || '';
  const moods = dirRon('moods').sort(byDefault(DEFAULT_MOOD));
  const genres = dirRon('genres').sort(byDefault(DEFAULT_GENRE));
  const bands = (readRon(path.join(MUSIC, 'intensity.ron')).bands || []).map((b) => b.mood).filter(Boolean);
  const scores = (readRon(path.join(MUSIC, 'scores', 'index.ron')).pieces || []);
  const pick = (o, keys) => Object.fromEntries(keys.map((k) => [k, o[k] ?? '']));
  return {
    _generated: 'Written by scripts/gen-music-catalogue.js from data/music (genres, moods, intensity.ron, scores/index.ron). Do not edit: change the RON and run the script.',
    genres: genres.map((g) => ({
      ...pick(g, ['id', 'words', 'name', 'plain_words', 'plain', 'about_words', 'about']),
      tempo_bpm: g.tempo_bpm || null,
      instruments: instrumentKeys(g, moods, instruments),
    })),
    moods: moods.map((m) => ({
      ...pick(m, ['id', 'words', 'where_words', 'where_plain']),
      tempo_bpm: m.tempo_bpm || null,
      danger: bands.includes(m.id),
    })),
    scores: scores.map((s) => ({
      ...pick(s, ['id', 'words', 'name', 'composer_words', 'composer', 'era_words', 'era', 'composer_years', 'key_root', 'key_mode', 'edition', 'source', 'source_url', 'typesetter', 'licence', 'credit_words', 'tempo_words', 'tempo_finding']),
      year: s.year || 0,
      length_s: s.length_s || 0,
      parts: (s.staves || []).map((st) => (st.part ? `instrument_packs.part.${st.part}` : (instruments[st.instrument] || `music.instrument.${st.instrument}`))).filter((k, i, a) => a.indexOf(k) === i),
    })),
  };
}

const text = () => JSON.stringify(build(), null, 2) + '\n';

if (require.main === module) {
  const want = text();
  if (process.argv.includes('--check')) {
    const have = fs.existsSync(OUT) ? fs.readFileSync(OUT, 'utf8').replace(/\r\n/g, '\n') : '';
    if (have !== want) {
      console.error('data/music/catalogue.json is stale: run node scripts/gen-music-catalogue.js');
      process.exit(1);
    }
    console.log('data/music/catalogue.json is current');
  } else {
    fs.writeFileSync(OUT, want);
    console.log(`wrote ${path.relative(ROOT, OUT)}`);
  }
}

module.exports = { build, text, OUT };
