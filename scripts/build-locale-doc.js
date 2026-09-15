#!/usr/bin/env node
// Generates a readable Library document for each locale from its data files.
//
// WHY THIS EXISTS
// data/locales/<id>/ holds everything true about a real place: the climate
// normals, the soil series, the watershed, the tides, the hazards, the shape of
// the ground. All of it correct, all of it JSON, and none of it readable by the
// person the simulation is for. A locale nobody can read is a promise, not a
// feature.
//
// So the same data that drives the simulation also becomes a guide you can open
// in the Library, which means the two can never disagree: there is one set of
// numbers and two renderings of it.
//
// The output is GENERATED. Edit the locale data, then re-run this; never edit
// the markdown, because the next run overwrites it.
//
//   node scripts/build-locale-doc.js
//
// Then `node scripts/build-library.js` to ship it.

const fs = require('fs');
const path = require('path');

const LOCALES = 'data/locales';
const OUT_DIR = path.join('docs', 'user', 'locale');

function readJson(dir, name) {
  const p = path.join(dir, name);
  if (!fs.existsSync(p)) return null;
  try {
    return JSON.parse(fs.readFileSync(p, 'utf8'));
  } catch (err) {
    console.error('locale-doc: could not parse ' + p + ': ' + err.message);
    process.exit(1);
  }
}

/** Wrap prose at 72 columns, the width the rest of docs/user/ uses. */
function wrap(text, width = 72) {
  const out = [];
  for (const para of String(text).split('\n')) {
    let line = '';
    for (const word of para.split(/\s+/).filter(Boolean)) {
      if (line && (line + ' ' + word).length > width) { out.push(line); line = word; }
      else line = line ? line + ' ' + word : word;
    }
    out.push(line);
  }
  return out.join('\n');
}

function buildOne(id) {
  const dir = path.join(LOCALES, id);
  const locale = readJson(dir, 'locale.json');
  if (!locale) return null;
  const climate = readJson(dir, 'climate.json');
  const soil = readJson(dir, 'soil.json');
  const water = readJson(dir, 'water.json');
  const tides = readJson(dir, 'tides.json');
  const hazards = readJson(dir, 'hazards.json');
  const terrain = readJson(dir, 'terrain.json');
  const phenology = readJson(dir, 'phenology.json');

  const L = [];
  const p = t => L.push(wrap(t), '');
  const h = (n, t) => L.push('#'.repeat(n) + ' ' + t, '');

  h(1, 'Where You Are: ' + locale.name + ', ' + locale.region);

  p('This is the real place the simulation is built on. Every number below ' +
    'comes from the same data the world itself is built from, so what you read ' +
    'here and what you walk around in cannot drift apart.');

  if (locale.setting) p('**The setting.** ' + locale.setting + '.');

  if (terrain && terrain.elevation_range) {
    const e = terrain.elevation_range;
    p('**The shape of it.** Sea level to about ' + e.max_m + ' metres, averaging ' +
      e.mean_m + '. ' + (e.note || ''));
  }

  // ── The year ──
  if (climate) {
    h(2, 'The year here');
    p('Climate zone ' + climate.koppen + ', USDA hardiness zone ' +
      climate.hardiness_zone + '. About ' + climate.annual_precip_mm +
      ' mm of rain a year and roughly ' + climate.growing_season_days +
      ' frost-free days.');

    if (Array.isArray(climate.monthly) && climate.monthly.length) {
      L.push('| Month | Average high | Average low | Rain | Daylight |');
      L.push('|---|---|---|---|---|');
      for (const m of climate.monthly) {
        L.push('| ' + m.month + ' | ' + m.mean_high_c + ' C | ' + m.mean_low_c +
          ' C | ' + m.precip_mm + ' mm | ' + m.daylight_hours + ' h |');
      }
      L.push('');
    }

    if (climate.frost_last_spring && climate.frost_first_autumn) {
      h(3, 'Frost, and the mistake that costs a crop');
      const s = climate.frost_last_spring, a = climate.frost_first_autumn;
      p('Last spring frost: ' + s.average + ' on average, as early as ' +
        s.early_10pct + ' and as late as ' + s.late_90pct + '. First autumn ' +
        'frost: ' + a.average + ' on average, as early as ' + a.early_10pct +
        ' and as late as ' + a.late_90pct + '.');
      if (climate.how_to_read_the_frost_dates) p(climate.how_to_read_the_frost_dates);
    }

    if (climate.extremes) {
      const x = climate.extremes;
      p('**Extremes on record.** ' + x.record_high_c + ' C at the highest, ' +
        x.record_low_c + ' C at the lowest. A building here should be designed ' +
        'for about ' + x.design_winter_c + ' C in winter.');
    }
  }

  // ── What happens when ──
  // The climate table says what the WEATHER does. This says what the PLACE
  // does, which is the half a person actually plans around: when the salmon
  // come up the creek, when the berries are ready, when the frost arrives.
  if (phenology && Array.isArray(phenology.months) && phenology.months.length) {
    h(2, 'What happens when');
    p('The climate table above is the weather. This is the year as events: what ' +
      'comes into leaf, what fruits, what runs up the creek, what arrives and ' +
      'what leaves. Each entry says how confident it is, because a date from a ' +
      'published record and a regional generalisation are not the same claim.');
    if (phenology._how_to_read_confidence) p(phenology._how_to_read_confidence);

    const byId = new Map((phenology.events || []).map(e => [e.id, e]));
    for (const m of phenology.months) {
      L.push('### ' + m.month, '');
      if (m.notes) p(m.notes);
      const events = (m.events || []).map(id => byId.get(id)).filter(Boolean);
      for (const e of events) {
        const conf = e.confidence && e.confidence !== 'measured' && e.confidence !== 'published_range'
          ? ' (' + String(e.confidence).replace(/_/g, ' ') + ')'
          : '';
        L.push('- **' + e.name + '**' + conf + '. ' +
          // The window often already ends in a full stop; do not add a second.
          (e.window ? String(e.window).replace(/[.\s]+$/, '') + '. ' : '') +
          (e.why_it_matters || ''));
      }
      if (events.length) L.push('');
    }
    if (phenology._what_is_deliberately_absent) {
      h(3, 'What this calendar deliberately leaves out');
      p(phenology._what_is_deliberately_absent);
    }
  }

  // ── The ground ──
  if (soil && Array.isArray(soil.series) && soil.series.length) {
    h(2, 'The ground');
    const top = soil.series.slice(0, 6);
    p('The soil survey maps this area into named series. These six cover most ' +
      'of it, and the percentages are of mapped LAND: the inlet is water and is ' +
      'not soil-mapped at all.');
    L.push('| Series | Share | Texture | Drainage | pH |');
    L.push('|---|---|---|---|---|');
    for (const s of top) {
      L.push('| ' + s.name + ' | ' + s.pct + '% | ' + (s.texture || '') + ' | ' +
        (s.drainage || '') + ' | ' + (s.ph_range || '') + ' |');
    }
    L.push('');
    if (soil.summary) p(String(soil.summary).split('\n\n')[0]);
  }

  // ── Water ──
  if (water) {
    h(2, 'Water');
    if (water.watershed && water.watershed.wria) {
      const w = water.watershed.wria;
      p('**The watershed.** ' + (w.name ? w.name + ' (WRIA ' + w.number + ')' : 'WRIA ' + w.number) +
        '. ' + (w.note || ''));
    }
    if (water.watershed && water.watershed.drainage_path_to_ocean) {
      p('**Where it goes.** ' + (water.watershed.drainage_path_to_ocean.description || ''));
    }
    if (Array.isArray(water.surface_waters) && water.surface_waters.length) {
      p('**Surface water nearby.** ' +
        water.surface_waters.slice(0, 10).map(s => s.name).join(', ') + '.');
    }
    if (water.groundwater && water.groundwater.viability) {
      p('**Groundwater.** ' + water.groundwater.viability);
    }
    if (water.groundwater && water.groundwater.legal_limits_on_new_wells) {
      const lw = water.groundwater.legal_limits_on_new_wells;
      const txt = (lw.wria_15_override && (lw.wria_15_override.summary || lw.wria_15_override.rule)) ||
        (lw.statewide_permit_exemption && lw.statewide_permit_exemption.summary);
      if (txt) p('**What you may legally take.** ' + txt);
    }
  }

  // ── Tides ──
  if (tides && tides.regime) {
    h(2, 'The tide');
    p('**Pattern.** ' + (tides.regime.plain_meaning || tides.regime.classification || ''));
    if (tides.derived_estimate_for_dyes_inlet && tides.derived_estimate_for_dyes_inlet.warning) {
      p('**A caution about the numbers.** ' + tides.derived_estimate_for_dyes_inlet.warning);
    }
    if (tides.tidal_currents && tides.tidal_currents.plain_meaning) {
      p('**Currents.** ' + tides.tidal_currents.plain_meaning);
    }
  }

  // ── Hazards ──
  if (hazards && Array.isArray(hazards.hazards) && hazards.hazards.length) {
    h(2, 'What can actually hurt you here');
    p('Not a generic list. Somewhere on a subduction zone has a different ' +
      'honest preparedness answer from somewhere on a floodplain, and preparing ' +
      'for the wrong one is worse than not preparing at all. The column that ' +
      'matters most is warning time, because it decides whether preparation or ' +
      'reaction is the useful skill.');
    for (const z of hazards.hazards) {
      L.push('### ' + z.hazard, '');
      const sev = String(z.severity || '').replace(/[.s]+$/, '');
      p(sev + (sev ? '. ' : '') + (z.local_effect || ''));
      // Trim a trailing stop off each part before joining, or a value that
      // already ends in one produces ".." in the rendered page.
      const tidy = v => String(v).replace(/[.\s]+$/, '');
      const bits = [];
      if (z.likelihood) bits.push('**Likelihood:** ' + tidy(z.likelihood));
      if (z.return_period) bits.push('**Return period:** ' + tidy(z.return_period));
      if (z.warning_time) bits.push('**Warning time:** ' + tidy(z.warning_time));
      if (bits.length) p(bits.join('. ') + '.');
    }
  }

  // ── Orientation ──
  if (terrain && terrain.named_features_for_orientation) {
    h(2, 'Finding your way');
    const f = terrain.named_features_for_orientation;
    for (const key of Object.keys(f)) {
      if (key === 'note' || key === 'source_url') continue;
      const v = f[key];
      const names = Array.isArray(v) ? v
        : (v && typeof v === 'object') ? Object.values(v).flat() : [v];
      const flat = names.filter(x => typeof x === 'string');
      if (flat.length) p('**' + key.replace(/_/g, ' ') + ':** ' + flat.join(', ') + '.');
    }
  }

  // ── Authorities ──
  if (locale.authorities) {
    h(2, 'Who to ask');
    p('Some questions have no general answer, only a local one: what may be ' +
      'taken, what may be planted, whether the shellfish are safe today, what ' +
      'water you may legally draw. These are the bodies that answer them here.');
    for (const [role, a] of Object.entries(locale.authorities)) {
      L.push('- **' + a.name + '**' + (a.url ? ' (' + a.url + ')' : '') +
        (a.note ? '. ' + a.note : '') + '  ');
    }
    L.push('');
  }

  h(2, 'Sources');
  p('Every figure above comes from a public-domain source, almost all of them ' +
    'US federal: NOAA for the climate normals and the tides, USDA NRCS for the ' +
    'soil survey, USGS for elevation and hydrography, FEMA and USGS for the ' +
    'hazards. The licence line inside each data file records which, so the claim ' +
    'can be checked rather than believed.');
  p('The data itself lives in `' + path.posix.join(LOCALES, id) + '/`. This ' +
    'document is GENERATED from it by `scripts/build-locale-doc.js`. Editing ' +
    'this page does nothing: edit the data and run the script.');

  return L.join('\n').replace(/\n{3,}/g, '\n\n').trimEnd() + '\n';
}

if (!fs.existsSync(LOCALES)) {
  console.log('No ' + LOCALES + ' directory; nothing to build.');
  process.exit(0);
}
fs.mkdirSync(OUT_DIR, { recursive: true });

let built = 0;
for (const id of fs.readdirSync(LOCALES)) {
  const md = buildOne(id);
  if (!md) continue;
  const out = path.join(OUT_DIR, id + '.md');
  fs.writeFileSync(out, md);
  built++;
  console.log('locale-doc: wrote ' + out + ' (' + md.split('\n').length + ' lines)');
}
// A generated document with an em dash in it would trip the project-wide rule
// the moment somebody pasted a data value containing one, so check here rather
// than finding out in review.
for (const id of fs.readdirSync(OUT_DIR)) {
  const text = fs.readFileSync(path.join(OUT_DIR, id), 'utf8');
  const bad = text.match(/[—–]/g);
  if (bad) {
    console.error('locale-doc: ' + id + ' contains ' + bad.length +
      ' em or en dash(es), which the house rule forbids. They came from the ' +
      'locale data; fix them there.');
    process.exit(1);
  }
}
console.log('locale-doc: ' + built + ' document(s) generated, no forbidden dashes');
