#!/usr/bin/env node
// The mothership's SPACE PROGRAM, totalled for a population (2026-11-06).
//
// The operator asked how the ship's total size is measured. Bottom up, the way architects size a
// building and NASA's 1975 summer study sized a settlement: every kind of space gets an area per
// person (or per unit), the population multiplies it, circulation and structure are added, and the
// total is the size. The list lives in data/ship/space_program.toml (each row names its source);
// this script totals it and checks it against Sector 1, the drum the operator sized (250 m radius,
// 2 km long) with its spine segment and the bow.
//
//   node scripts/space-program.js                 the file's default population
//   node scripts/space-program.js 10000           totals for 10,000 people
//   node scripts/space-program.js 10000 --mix homestead=0.05,cabin=0.25,apartment=0.70
//   node scripts/space-program.js 1000 --rows     every row's area as well
//   node scripts/space-program.js --block         the apartment block template's numbers
//   node scripts/space-program.js --sources       where the numbers come from
//   node scripts/space-program.js 1000 --json     the totals as JSON
//
// Exit code 0 when the population fits Sector 1 (and the block template checks out), 1 when it
// does not, 2 on a bad file or argument. Design: docs/design/mothership-master-plan.md 3.12.
// Tests: scripts/tests/space-program.test.js.
//
// The data is TOML. Node has no TOML reader built in and the repo carries no npm dependency for
// one, so `parseToml` below reads the subset the data uses (tables, arrays of tables, strings,
// numbers, booleans, nested and multi-line arrays, comments). The engine's own `toml` crate reads
// the same files when the game starts using them.

'use strict';

const fs = require('fs');
const path = require('path');

const REPO = path.resolve(__dirname, '..');
const PROGRAM = path.join(REPO, 'data', 'ship', 'space_program.toml');

// ---------------------------------------------------------------------------------------------
// A small TOML reader (the subset above). Throws with the line number on anything else.

function parseToml(text) {
  const root = {};
  let table = root;
  let i = 0;
  let line = 1;
  const n = text.length;

  const fail = (msg) => {
    throw new Error(`TOML line ${line}: ${msg}`);
  };
  const peek = () => text[i];
  const skipSpace = (newlines) => {
    for (;;) {
      const c = text[i];
      if (c === ' ' || c === '\t' || c === '\r') {
        i++;
      } else if (c === '\n' && newlines) {
        i++;
        line++;
      } else if (c === '#') {
        while (i < n && text[i] !== '\n') i++;
      } else {
        return;
      }
    }
  };
  const readKey = () => {
    skipSpace(false);
    if (peek() === '"') return readString();
    const start = i;
    while (i < n && /[A-Za-z0-9_-]/.test(text[i])) i++;
    if (i === start) fail(`expected a key, found ${JSON.stringify(text[i])}`);
    return text.slice(start, i);
  };
  const readString = () => {
    i++; // opening quote
    let out = '';
    while (i < n && text[i] !== '"') {
      if (text[i] === '\n') fail('a string runs past the end of its line');
      if (text[i] === '\\') {
        const e = text[i + 1];
        const map = { n: '\n', t: '\t', '"': '"', '\\': '\\', r: '\r' };
        if (!(e in map)) fail(`unsupported escape \\${e}`);
        out += map[e];
        i += 2;
      } else {
        out += text[i++];
      }
    }
    if (text[i] !== '"') fail('unterminated string');
    i++;
    return out;
  };
  const readValue = () => {
    skipSpace(false);
    const c = peek();
    if (c === '"') return readString();
    if (c === '[') {
      i++;
      const arr = [];
      for (;;) {
        skipSpace(true);
        if (peek() === ']') {
          i++;
          return arr;
        }
        arr.push(readValue());
        skipSpace(true);
        if (peek() === ',') {
          i++;
        } else if (peek() === ']') {
          i++;
          return arr;
        } else {
          fail(`expected , or ] in an array, found ${JSON.stringify(peek())}`);
        }
      }
    }
    const start = i;
    while (i < n && /[^\s,\]#]/.test(text[i])) i++;
    const word = text.slice(start, i);
    if (word === 'true') return true;
    if (word === 'false') return false;
    if (/^[+-]?(\d[\d_]*)(\.\d[\d_]*)?([eE][+-]?\d+)?$/.test(word)) return Number(word.replace(/_/g, ''));
    fail(`unsupported value ${JSON.stringify(word)}`);
  };

  while (i < n) {
    skipSpace(true);
    if (i >= n) break;
    if (peek() === '[') {
      const isArray = text[i + 1] === '[';
      i += isArray ? 2 : 1;
      const name = readKey();
      skipSpace(false);
      if (text.slice(i, i + (isArray ? 2 : 1)) !== (isArray ? ']]' : ']')) fail('unclosed table header');
      i += isArray ? 2 : 1;
      if (isArray) {
        if (!Array.isArray(root[name])) root[name] = [];
        table = {};
        root[name].push(table);
      } else {
        if (root[name] !== undefined) fail(`table [${name}] defined twice`);
        table = {};
        root[name] = table;
      }
    } else {
      const key = readKey();
      skipSpace(false);
      if (peek() !== '=') fail(`expected = after ${key}`);
      i++;
      if (key in table) fail(`key ${key} set twice`);
      table[key] = readValue();
    }
    skipSpace(false);
    if (i < n && peek() !== '\n') fail(`unexpected ${JSON.stringify(peek())} after a value`);
  }
  return root;
}

// ---------------------------------------------------------------------------------------------
// Loading and checking the program.

function loadProgram(file = PROGRAM) {
  const prog = parseToml(fs.readFileSync(file, 'utf8'));
  for (const h of prog.home || []) {
    if (h.template) {
      const t = path.join(path.dirname(file), h.template);
      h.block = summarizeBlock(parseToml(fs.readFileSync(t, 'utf8')));
    }
  }
  const errors = validateProgram(prog);
  if (errors.length) throw new Error(`${path.relative(REPO, file)}:\n  ${errors.join('\n  ')}`);
  return prog;
}

const ZONES = ['drum', 'spine', 'bow', 'stern', 'end_cap', 'exterior'];
const PERS = ['person', 'unit', 'neighbourhood', 'sector', 'ship'];

function validateProgram(prog) {
  const errs = [];
  const sources = new Set((prog.source || []).map((s) => s.id));
  const ids = new Set();
  const shareSum = (prog.home || []).reduce((a, h) => a + (h.share || 0), 0);
  if (Math.abs(shareSum - 1) > 1e-6) errs.push(`the home shares sum to ${shareSum.toFixed(4)}, not 1`);
  for (const h of prog.home || []) {
    if (!sources.has(h.source)) errs.push(`home ${h.id}: unknown source ${h.source}`);
    if (!(h.share >= 0)) errs.push(`home ${h.id}: share must be 0 or more`);
    if (!h.template && !(h.people_per_home > 0)) errs.push(`home ${h.id}: people_per_home must be above 0`);
    if (!h.template && !(h.plot_m2 > 0) && !(h.unit_m2 > 0)) errs.push(`home ${h.id}: needs plot_m2, unit_m2 or a template`);
    if (h.block && h.block.problems.length) errs.push(...h.block.problems.map((p) => `home ${h.id} (${h.template}): ${p}`));
  }
  for (const s of prog.space || []) {
    if (ids.has(s.id)) errs.push(`space ${s.id} appears twice`);
    ids.add(s.id);
    if (!ZONES.includes(s.zone)) errs.push(`space ${s.id}: zone ${s.zone} is not one of ${ZONES.join(', ')}`);
    if (!PERS.includes(s.per)) errs.push(`space ${s.id}: per ${s.per} is not one of ${PERS.join(', ')}`);
    if (!sources.has(s.source)) errs.push(`space ${s.id}: unknown source ${s.source}`);
    if (s.per === 'person' && !(s.m2_per_person >= 0)) errs.push(`space ${s.id}: m2_per_person missing`);
    if (s.per !== 'person' && !(s.unit_m2 >= 0)) errs.push(`space ${s.id}: unit_m2 missing`);
    if (s.per === 'unit' && !(s.people_per_unit > 0)) errs.push(`space ${s.id}: people_per_unit missing`);
    if (s.levels !== undefined && !(s.levels >= 1)) errs.push(`space ${s.id}: levels must be 1 or more`);
    if (s.who !== undefined && !['all', 'fed_by_ship'].includes(s.who)) errs.push(`space ${s.id}: who must be all or fed_by_ship`);
  }
  return errs;
}

// The apartment block template: its homes, residents and the floor it covers, with the checks a
// design drawn from it must pass.
function summarizeBlock(t) {
  const problems = [];
  const f = t.footprint || {};
  const st = t.storeys || {};
  const ring = f.outer_w_m * f.outer_d_m - f.atrium_w_m * f.atrium_d_m;
  const galleryOuterW = f.atrium_w_m + 2 * f.gallery_w_m;
  const galleryOuterD = f.atrium_d_m + 2 * f.gallery_w_m;
  const gallery = galleryOuterW * galleryOuterD - f.atrium_w_m * f.atrium_d_m;
  const cores = (t.core || []).reduce((a, c) => a + c.area_m2, 0);
  const residentialStoreys = st.count - st.first_residential + 1;
  const units = t.unit || [];
  const unitsNetPerStorey = units.reduce((a, u) => a + u.net_m2 * u.per_storey, 0);
  const homesPerStorey = units.reduce((a, u) => a + u.per_storey, 0);
  const peoplePerStorey = units.reduce((a, u) => a + u.people * u.per_storey, 0);
  const groundNet = (t.ground_room || []).reduce((a, r) => a + r.net_m2, 0);
  const availablePerStorey = ring - gallery - cores;
  const groundAvailable = ring - cores;
  const plotW = f.outer_w_m + 2 * f.plot_margin_m;
  const plotD = f.outer_d_m + 2 * f.plot_margin_m;
  const homes = homesPerStorey * residentialStoreys;
  const people = peoplePerStorey * residentialStoreys;
  const plot = plotW * plotD;

  if (unitsNetPerStorey > availablePerStorey) {
    problems.push(`a residential storey's homes need ${unitsNetPerStorey} m2 but the ring leaves ${availablePerStorey} m2 after the gallery and the cores`);
  }
  if (groundNet > groundAvailable) problems.push(`the ground floor rooms need ${groundNet} m2 but the ring holds ${groundAvailable} m2`);
  if ((t.rules || {}).lift_to_every_storey && !(t.core || []).some((c) => c.lift)) problems.push('lift_to_every_storey needs a core with a lift');
  if ((t.core || []).filter((c) => c.stair).length < ((t.rules || {}).stairs || 1)) problems.push('fewer stairs than rules.stairs asks for');
  const ringDepth = (f.outer_w_m - galleryOuterW) / 2;
  for (const u of units) {
    if (!(u.people >= 1 && u.people <= 2 * u.bedrooms)) {
      problems.push(`unit ${u.id}: ${u.people} people in ${u.bedrooms} bedroom(s) is crowded by the bedroom standard (at most two a bedroom)`);
    }
  }
  return {
    id: t.id,
    name: t.name,
    storeys: st.count,
    residentialStoreys,
    homes,
    people,
    plot_m2: plot,
    footprint_m2: f.outer_w_m * f.outer_d_m,
    ring_m2: ring,
    gallery_m2: gallery,
    cores_m2: cores,
    unitsNetPerStorey,
    availablePerStorey,
    groundNet,
    groundAvailable,
    ringDepth,
    top_floor_g: st.height_m ? (250 - (st.count - 1) * st.height_m) / 250 : null,
    m2_per_person: plot / people,
    problems,
  };
}

// ---------------------------------------------------------------------------------------------
// The totals.

function parseMix(arg, prog) {
  const mix = {};
  for (const part of arg.split(',')) {
    const [k, v] = part.split('=');
    if (!(prog.home || []).some((h) => h.id === k)) throw new Error(`--mix: no home kind ${k}`);
    mix[k] = Number(v);
    if (!(mix[k] >= 0)) throw new Error(`--mix: ${part} is not a share`);
  }
  const sum = Object.values(mix).reduce((a, b) => a + b, 0);
  if (Math.abs(sum - 1) > 1e-6) throw new Error(`--mix shares sum to ${sum}, not 1`);
  return mix;
}

// Everything the population needs, by zone, with each row's own figures.
function totalFor(prog, population, mix) {
  const d = prog.defaults;
  const homesOut = [];
  const rows = [];
  const zone = Object.fromEntries(ZONES.map((z) => [z, 0]));
  const gross = Object.fromEntries(ZONES.map((z) => [z, 0]));
  let fedByShip = 0;
  const neighbourhoods = Math.max(1, Math.ceil(population / d.neighbourhood_people));
  const indoor = (m2, net) => (net ? m2 * d.indoor_circulation * (1 + d.structure_allowance) : m2);

  for (const h of prog.home) {
    const share = mix && h.id in mix ? mix[h.id] : h.share;
    const people = population * share;
    if (!h.feeds_itself) fedByShip += people;
    let homes = 0;
    let floor = 0;
    let built = 0;
    if (people > 0) {
      if (h.block) {
        homes = Math.ceil(people / h.block.people);
        floor = homes * h.block.plot_m2;
        built = homes * h.block.footprint_m2 * h.block.storeys;
      } else if (h.plot_m2) {
        homes = Math.ceil(people / h.people_per_home);
        floor = homes * h.plot_m2 * (h.plot_gross || 1);
        built = homes * h.plot_m2;
      } else {
        homes = Math.ceil(people / h.people_per_home);
        built = indoor(homes * (h.unit_m2 + (h.shared_m2 || 0) * h.people_per_home), h.net);
        floor = built / (h.storeys || 1);
      }
    }
    zone.drum += floor;
    gross.drum += built;
    homesOut.push({ id: h.id, name: h.name, share, people, homes, unit: h.block ? 'blocks' : 'homes', floor_m2: floor, built_m2: built });
  }

  for (const s of prog.space) {
    const basis = s.who === 'fed_by_ship' ? fedByShip : population;
    let area = 0;
    let units = null;
    switch (s.per) {
      case 'person':
        area = s.m2_per_person * basis;
        break;
      case 'unit':
        units = basis > 0 ? Math.max(s.min_units || 0, Math.ceil(basis / s.people_per_unit)) + (s.spare_units || 0) : 0;
        area = units * s.unit_m2;
        break;
      case 'neighbourhood':
        units = neighbourhoods;
        area = units * s.unit_m2;
        break;
      case 'sector':
        units = s.units_per_sector;
        area = units * s.unit_m2;
        break;
      case 'ship':
        units = 1;
        area = s.unit_m2;
        break;
    }
    area = Math.max(area, s.min_m2 || 0, (s.min_m2_per_neighbourhood || 0) * neighbourhoods);
    const built = indoor(area, s.net);
    // On the drum a stacked space covers less floor; elsewhere every deck is floor.
    const floor = s.zone === 'drum' || s.zone === 'end_cap' ? built / (s.levels || 1) : built;
    zone[s.zone] += floor;
    gross[s.zone] += built;
    rows.push({ id: s.id, name: s.name, zone: s.zone, units, built_m2: built, floor_m2: floor, source: s.source });
  }
  return { population, neighbourhoods, fedByShip, homes: homesOut, rows, zone, gross };
}

// What Sector 1 (and the bow) offer.
function sectorRoom(prog) {
  const s = prog.sector;
  const drum = 2 * Math.PI * s.drum_radius_m * s.drum_length_m * s.drum_usable;
  const deck = Math.PI * s.spine_radius_m * s.spine_radius_m;
  let decks = 0;
  for (const [from, to, spacing] of s.spine_segments) decks += Math.floor((to - from) / spacing);
  const spine = decks * deck * s.spine_usable;
  const b = prog.bow;
  const bowDecks = Math.floor(b.length_m / b.deck_spacing_m);
  const bow = bowDecks * Math.PI * b.radius_m * b.radius_m * b.usable;
  const hull = 2 * Math.PI * s.drum_radius_m * s.drum_length_m + 2 * Math.PI * s.drum_radius_m * s.drum_radius_m;
  return { drum, spine, spineDecks: decks, bow, bowDecks, hull_m2: hull, shield_t: hull * s.shield_t_per_m2 };
}

function fits(total, room) {
  return {
    drum: total.zone.drum <= room.drum,
    spine: total.zone.spine <= room.spine,
    bow: total.zone.bow <= room.bow,
    all: total.zone.drum <= room.drum && total.zone.spine <= room.spine && total.zone.bow <= room.bow,
  };
}

// The largest population Sector 1 holds at this mix (the floors grow in steps, so a search).
function maxPopulation(prog, mix) {
  const room = sectorRoom(prog);
  let lo = 0;
  let hi = 1;
  while (fits(totalFor(prog, hi, mix), room).all) {
    lo = hi;
    hi *= 2;
    if (hi > 1e9) return Infinity;
  }
  while (hi - lo > 1) {
    const mid = Math.floor((lo + hi) / 2);
    if (fits(totalFor(prog, mid, mix), room).all) lo = mid;
    else hi = mid;
  }
  return lo;
}

// How many sectors of Sector 1's size a population needs: each brings one drum and one spine
// segment; the bow is the ship's, counted once.
function sectorsNeeded(total, room) {
  return Math.max(1, Math.ceil(total.zone.drum / room.drum), Math.ceil(total.zone.spine / room.spine));
}

// ---------------------------------------------------------------------------------------------
// Printing.

const fmt = (x) => Math.round(x).toLocaleString('en-US');
const km2 = (m2) => (m2 / 1e6).toFixed(m2 < 1e6 ? 3 : 2);

function report(prog, population, mix, showRows) {
  const t = totalFor(prog, population, mix);
  const room = sectorRoom(prog);
  const ok = fits(t, room);
  const out = [];
  out.push(`Space program for ${fmt(population)} people (${t.neighbourhoods} neighbourhood${t.neighbourhoods === 1 ? '' : 's'}; the ship's farms feed ${fmt(t.fedByShip)})`);
  out.push('');
  out.push('Homes');
  for (const h of t.homes) {
    out.push(`  ${h.name.padEnd(10)} ${(h.share * 100).toFixed(0).padStart(3)}%  ${fmt(h.people).padStart(9)} people  ${fmt(h.homes).padStart(7)} ${h.unit.padEnd(6)} ${fmt(h.floor_m2).padStart(11)} m2 of drum floor`);
  }
  if (showRows) {
    out.push('');
    out.push('Spaces (built area, and the floor it covers)');
    for (const r of t.rows) {
      const u = r.units === null ? '' : `${r.units} x `;
      out.push(`  ${r.zone.padEnd(8)} ${r.id.padEnd(18)} ${u.padStart(7)}${fmt(r.built_m2).padStart(11)} m2 built ${fmt(r.floor_m2).padStart(11)} m2 floor  [${r.source}]`);
    }
  }
  out.push('');
  out.push('By zone                    needed         Sector 1 offers');
  out.push(`  drum floor        ${km2(t.zone.drum).padStart(10)} km2   ${km2(room.drum).padStart(8)} km2   ${ok.drum ? 'fits' : 'DOES NOT FIT'} (${((100 * t.zone.drum) / room.drum).toFixed(0)}%)`);
  out.push(`  spine decks       ${km2(t.zone.spine).padStart(10)} km2   ${km2(room.spine).padStart(8)} km2   ${ok.spine ? 'fits' : 'DOES NOT FIT'} (${((100 * t.zone.spine) / room.spine).toFixed(0)}%, ${room.spineDecks} decks)`);
  out.push(`  bow decks         ${km2(t.zone.bow).padStart(10)} km2   ${km2(room.bow).padStart(8)} km2   ${ok.bow ? 'fits' : 'DOES NOT FIT'} (${((100 * t.zone.bow) / room.bow).toFixed(0)}%, ${room.bowDecks} decks)`);
  out.push(`  end caps          ${km2(t.zone.end_cap).padStart(10)} km2   (lift shafts and stairs)`);
  out.push(`  radiator fin      ${km2(t.zone.exterior).padStart(10)} km2   outside the hull (Sector 1's own hull is ${km2(room.hull_m2)} km2)`);
  const perPerson = (t.zone.drum + t.zone.spine + t.zone.bow) / population;
  out.push('');
  out.push(`Floor per person, all zones: ${fmt(perPerson)} m2 (drum ${fmt(t.zone.drum / population)}, spine ${fmt(t.zone.spine / population)}, bow ${fmt(t.zone.bow / population)})`);
  out.push(`Sector 1 ${ok.all ? 'HOLDS' : 'DOES NOT HOLD'} ${fmt(population)} people at this mix.`);
  out.push(`Sector 1 holds at most ${fmt(maxPopulation(prog, mix))} people at this mix.`);
  if (!ok.all) out.push(`Sectors of Sector 1's size needed: ${fmt(sectorsNeeded(t, room))}.`);
  out.push(`Sector 1's shield (${prog.sector.shield_t_per_m2} t/m2 over ${km2(room.hull_m2)} km2 of hull): ${fmt(room.shield_t / 1e6)} million tonnes.`);
  return { text: out.join('\n'), ok: ok.all, total: t, room };
}

function blockReport(b) {
  return [
    `${b.name} (${b.id})`,
    `  ${b.storeys} storeys, ${b.residentialStoreys} of them homes: ${b.homes} homes for up to ${b.people} people`,
    `  footprint ${fmt(b.footprint_m2)} m2, its plot ${fmt(b.plot_m2)} m2: ${b.m2_per_person.toFixed(1)} m2 of drum floor per resident`,
    `  a residential storey: ring ${fmt(b.ring_m2)} m2 - gallery ${fmt(b.gallery_m2)} - cores ${fmt(b.cores_m2)} = ${fmt(b.availablePerStorey)} m2 for homes; the homes take ${fmt(b.unitsNetPerStorey)} m2 (${fmt(b.availablePerStorey - b.unitsNetPerStorey)} m2 for walls and services)`,
    `  ground floor rooms ${fmt(b.groundNet)} of ${fmt(b.groundAvailable)} m2; homes ${b.ringDepth.toFixed(1)} m deep`,
    b.top_floor_g ? `  the top storey's floor feels ${b.top_floor_g.toFixed(3)} g on a 250 m drum` : '',
    b.problems.length ? `  PROBLEMS:\n    ${b.problems.join('\n    ')}` : '  checks: all pass',
  ].filter(Boolean).join('\n');
}

function main(argv) {
  let prog;
  try {
    prog = loadProgram();
  } catch (e) {
    console.error(e.message);
    return 2;
  }
  const args = argv.slice(2);
  if (args.includes('--sources')) {
    for (const s of prog.source) console.log(`${s.id.padEnd(16)} ${s.what}\n${''.padEnd(16)} ${s.where}`);
    return 0;
  }
  const blockHome = prog.home.find((h) => h.block);
  if (args.includes('--block')) {
    if (!blockHome) {
      console.error('no home kind names a block template');
      return 2;
    }
    console.log(blockReport(blockHome.block));
    return blockHome.block.problems.length ? 1 : 0;
  }
  let mix = null;
  const mi = args.indexOf('--mix');
  try {
    if (mi >= 0) mix = parseMix(args[mi + 1] || '', prog);
  } catch (e) {
    console.error(e.message);
    return 2;
  }
  const popArg = args.find((a, k) => /^\d[\d_,]*$/.test(a) && args[k - 1] !== '--mix');
  const population = popArg ? Number(popArg.replace(/[_,]/g, '')) : prog.defaults.population;
  if (!(population > 0)) {
    console.error('the population must be a whole number above 0');
    return 2;
  }
  const r = report(prog, population, mix, args.includes('--rows'));
  if (args.includes('--json')) {
    console.log(JSON.stringify({ population, fits: r.ok, zone: r.total.zone, room: r.room, homes: r.total.homes }, null, 2));
  } else {
    console.log(r.text);
  }
  return r.ok ? 0 : 1;
}

module.exports = { parseToml, loadProgram, validateProgram, summarizeBlock, totalFor, sectorRoom, fits, maxPopulation, sectorsNeeded, parseMix };

if (require.main === module) process.exitCode = main(process.argv);
