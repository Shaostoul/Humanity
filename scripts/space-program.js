#!/usr/bin/env node
// The mothership's SPACE PROGRAM, totalled for a population (2026-11-06).
//
// The operator asked how the ship's total size is measured. Bottom up, the way architects size a
// building and NASA's 1975 summer study sized a settlement: every kind of space gets an area per
// person (or per unit), the population multiplies it, circulation and structure are added, and the
// total is the size. The list lives in data/ship/space_program.toml (each row names its source);
// this script totals it and checks it against Drum 1, the drum the operator sized (250 m radius,
// 2 km long) with its spine segment and the bow.
//
// 2026-11-07, his answers: a SECTOR is a community of 500 ("Let's start with a sector for 500
// people."), so `--sector` sizes one, lays it out on Drum 1's floor and checks its walks and its
// noise; and "Let's aim for 1 billion and 10 billion.", so `--scale` works out the drums, the
// ships and the fleet that hold them, against hoop stress, spin stability and the spine's own
// loads (docs/reference/findings/2026-11-07-drum-limits.md).
//
//   node scripts/space-program.js                 one sector (the file's default population)
//   node scripts/space-program.js 10000           totals for 10,000 people
//   node scripts/space-program.js 10000 --mix homestead=0.05,cabin=0.25,apartment=0.70
//   node scripts/space-program.js 10000 --farm-levels 2   the ship's farms stacked two high
//   node scripts/space-program.js 1000 --rows     every row's area as well
//   node scripts/space-program.js --sector        the 500-person sector: its drum, walks, noise
//   node scripts/space-program.js --scale         1 and 10 billion: drums, ships, the fleet
//   node scripts/space-program.js --block         the apartment block template's numbers
//   node scripts/space-program.js --sources       where the numbers come from
//   node scripts/space-program.js 1000 --json     the totals as JSON
//
// Exit code 0 when the population fits Drum 1 (and the block template checks out), 1 when it
// does not, 2 on a bad file or argument. Design: docs/design/mothership-master-plan.md 3.12, 3.15
// and 3.16. Tests: scripts/tests/space-program.test.js.
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
const PERS = ['person', 'unit', 'neighbourhood', 'sector', 'drum', 'ship'];

function validateProgram(prog) {
  const errs = [];
  const sources = new Set((prog.source || []).map((s) => s.id));
  const ids = new Set();
  const shareSum = (prog.home || []).reduce((a, h) => a + (h.share || 0), 0);
  if (Math.abs(shareSum - 1) > 1e-6) errs.push(`the home shares sum to ${shareSum.toFixed(4)}, not 1`);
  if (!(prog.sector && prog.sector.residents > 0)) errs.push('[sector] residents must be above 0');
  if (prog.farms && !(prog.farms.levels >= 1)) errs.push('[farms] levels must be 1 or more');
  for (const h of prog.home || []) {
    if (!sources.has(h.source)) errs.push(`home ${h.id}: unknown source ${h.source}`);
    if (!(h.share >= 0)) errs.push(`home ${h.id}: share must be 0 or more`);
    if (!h.template && !(h.people_per_home > 0)) errs.push(`home ${h.id}: people_per_home must be above 0`);
    if (!h.template && !(h.plot_m2 > 0) && !(h.unit_m2 > 0)) errs.push(`home ${h.id}: needs plot_m2, unit_m2 or a template`);
    if (h.block && h.block.problems.length) errs.push(...h.block.problems.map((p) => `home ${h.id} (${h.template}): ${p}`));
    // A home that grows its own food must have room for its household's gardens.
    if (h.garden_m2_per_person && h.greenhouse_m2) {
      const need = h.garden_m2_per_person * Math.ceil(h.people_per_home);
      if (need > h.greenhouse_m2) errs.push(`home ${h.id}: ${Math.ceil(h.people_per_home)} residents' gardens need ${need.toFixed(1)} m2 but its greenhouse is ${h.greenhouse_m2} m2`);
    }
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
    if (s.per === 'drum' && !(s.units_per_drum >= 0)) errs.push(`space ${s.id}: units_per_drum missing`);
    if (s.levels !== undefined && !(s.levels >= 1)) errs.push(`space ${s.id}: levels must be 1 or more`);
    if (s.who !== undefined && !['all', 'fed_by_ship'].includes(s.who)) errs.push(`space ${s.id}: who must be all or fed_by_ship`);
  }
  for (const m of prog.material || []) {
    if (!sources.has(m.source)) errs.push(`material ${m.id}: unknown source ${m.source}`);
    if (!(m.strength_mpa > 0 && m.density_kg_m3 > 0)) errs.push(`material ${m.id}: strength_mpa and density_kg_m3 must be above 0`);
  }
  if (prog.scale) {
    const mats = new Set((prog.material || []).map((m) => m.id));
    for (const k of ['spine_material', 'hull_material']) if (!mats.has(prog.scale[k])) errs.push(`[scale] ${k} ${prog.scale[k]} is not a [[material]]`);
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

// Everything the population needs, by zone, with each row's own figures. `opts.farmLevels`
// overrides [farms] levels for the rows marked `farm = true`.
function totalFor(prog, population, mix, opts = {}) {
  const d = prog.defaults;
  const homesOut = [];
  const rows = [];
  const zone = Object.fromEntries(ZONES.map((z) => [z, 0]));
  const gross = Object.fromEntries(ZONES.map((z) => [z, 0]));
  let fedByShip = 0;
  const neighbourhoods = Math.max(1, Math.ceil(population / d.neighbourhood_people));
  const sectors = Math.max(1, Math.ceil(population / ((prog.sector && prog.sector.residents) || Infinity)));
  const farmLevels = opts.farmLevels || (prog.farms && prog.farms.levels) || 1;
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
        units = sectors;
        area = units * s.unit_m2;
        break;
      case 'drum':
        units = s.units_per_drum;
        area = units * s.unit_m2;
        break;
      case 'ship':
        units = 1;
        area = s.unit_m2;
        break;
    }
    area = Math.max(area, s.min_m2 || 0, (s.min_m2_per_neighbourhood || 0) * neighbourhoods);
    const built = indoor(area, s.net);
    // On the drum a stacked space covers less floor; elsewhere every deck is floor. The farms
    // stack as [farms] says (the operator's one level, two or more allowed).
    const levels = s.farm ? farmLevels : s.levels || 1;
    const floor = s.zone === 'drum' || s.zone === 'end_cap' ? built / levels : built;
    zone[s.zone] += floor;
    gross[s.zone] += built;
    rows.push({ id: s.id, name: s.name, zone: s.zone, units, built_m2: built, floor_m2: floor, source: s.source });
  }
  return { population, neighbourhoods, sectors, fedByShip, homes: homesOut, rows, zone, gross };
}

// What Drum 1 (and the bow) offer.
function drumRoom(prog) {
  const s = prog.drum;
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

// The largest population Drum 1 holds at this mix (the floors grow in steps, so a search).
function maxPopulation(prog, mix, opts = {}) {
  const room = drumRoom(prog);
  let lo = 0;
  let hi = 1;
  while (fits(totalFor(prog, hi, mix, opts), room).all) {
    lo = hi;
    hi *= 2;
    if (hi > 1e9) return Infinity;
  }
  while (hi - lo > 1) {
    const mid = Math.floor((lo + hi) / 2);
    if (fits(totalFor(prog, mid, mix, opts), room).all) lo = mid;
    else hi = mid;
  }
  return lo;
}

// How many drums of Drum 1's size a population needs: each brings its floor and one spine
// segment; the bow is the ship's, counted once.
function drumsNeeded(total, room) {
  return Math.max(1, Math.ceil(total.zone.drum / room.drum), Math.ceil(total.zone.spine / room.spine));
}

// ---------------------------------------------------------------------------------------------
// The 500-person sector (2026-11-07): its floor, how it lies on a drum, its walks and its noise.

const G = 9.80665;
const rpm = (radius) => (60 / (2 * Math.PI)) * Math.sqrt(G / radius);

// A sector on a drum of this radius is a patch `around` metres of the circumference by `along`
// metres of the drum's length, cut as a whole ring, a half ring or a quarter ring. Its three
// neighbourhoods lie side by side along the patch's longer side, each node at its middle; the
// sector's civic hall at the middle of the patch. Walks follow streets (around plus along).
function sectorLayouts(floorM2, radius, sector) {
  const circ = 2 * Math.PI * radius;
  const n = Math.max(1, Math.round(sector.residents / ((sector.layers || []).find((l) => l[0] === 'neighbourhood') || [0, sector.residents])[1]));
  return [1, 2, 4].map((cut) => {
    const around = circ / cut;
    const along = floorM2 / around;
    const long = Math.max(around, along);
    const short = Math.min(around, along);
    const nodeWalk = short / 2 + long / (2 * n);
    const hallWalk = short / 2 + long / 2;
    return {
      cut,
      name: cut === 1 ? 'a whole ring' : cut === 2 ? 'a half ring' : 'a quarter ring',
      around,
      along,
      neighbourhoods: n,
      nodeWalk,
      hallWalk,
      nodeWalkS: nodeWalk / sector.walk_m_per_s,
      hallWalkS: hallWalk / sector.walk_m_per_s,
      walksOk: nodeWalk / sector.walk_m_per_s <= sector.node_walk_max_s && hallWalk / sector.walk_m_per_s <= sector.daily_walk_max_s,
    };
  });
}

// How far a loud room must stand from a bedroom facade: 3.13's free-field estimate,
// Lp = Lw - 20 log10(r) - hard_floor - wall_loss, solved for the night limit.
function noiseDistance(lw, wallLoss, noise) {
  return Math.pow(10, (lw - noise.hard_floor_db - wallLoss - noise.night_facade_db) / 20);
}

function sectorStudy(prog, mix, opts = {}) {
  const sec = prog.sector;
  const lone = totalFor(prog, sec.residents, mix, opts);
  const full = maxPopulation(prog, mix, opts);
  const fullT = totalFor(prog, full, mix, opts);
  const sharedFloor = (fullT.zone.drum / full) * sec.residents;
  const radius = prog.drum.drum_radius_m;
  const room = drumRoom(prog);
  const layouts = sectorLayouts(lone.zone.drum, radius, sec);
  const ok = layouts.filter((l) => l.walksOk);
  const best = (ok.length ? ok : layouts).slice().sort((a, b) => a.hallWalk - b.hallWalk)[0];
  const stableLength = ((prog.scale && prog.scale.stability_l_over_r) || 1.3) * radius;
  const loud = (prog.space || []).filter((s) => s.sound_power_dba !== undefined).map((s) => ({
    id: s.id,
    lw: s.sound_power_dba,
    wallLoss: s.wall_loss_db || 0,
    enclosed: noiseDistance(s.sound_power_dba, s.wall_loss_db || 0, prog.noise),
    open: noiseDistance(s.sound_power_dba, 0, prog.noise),
  }));
  return {
    residents: sec.residents,
    layers: sec.layers,
    lone,
    loneFloor: lone.zone.drum,
    sharedFloor,
    drumFull: full,
    sectorsInDrum1: Math.floor(room.drum / sharedFloor),
    radius,
    rpm: rpm(radius),
    layouts,
    best,
    halfRingLength: lone.zone.drum / (Math.PI * radius),
    stableLength,
    drum1LOverR: prog.drum.drum_length_m / radius,
    loud,
  };
}

// ---------------------------------------------------------------------------------------------
// One billion and ten billion (2026-11-07): what holds them. Real limits only (the finding
// 2026-11-07-drum-limits.md); the jump that moves the ships is a game fiction and is not used.

// A spinning drum's rim at 1 g carries its own mass (hoop stress rho g R, Jensen 2024) and what
// presses on it, the air and the contents: the wall it needs is t = q R / (sigma - rho g R), and
// no wall at all once rho g R reaches the working strength.
function hoop(material, radius, sc) {
  const sigma = (material.strength_mpa * 1e6) / sc.safety_factor;
  const self = material.density_kg_m3 * G * radius;
  const q = sc.air_kpa * 1000 + sc.contents_t_per_m2 * 1000 * G;
  const t = sigma > self ? (q * radius) / (sigma - self) : Infinity;
  return {
    material: material.id,
    radius,
    sigmaMpa: sigma / 1e6,
    selfMpa: self / 1e6,
    maxRadius: sigma / (material.density_kg_m3 * G),
    thickness: t,
    kgPerM2: t * material.density_kg_m3,
  };
}

function scaleStudy(prog, mix, opts = {}) {
  const sc = prog.scale;
  const sec = prog.sector;
  const mats = Object.fromEntries((prog.material || []).map((m) => [m.id, m]));
  const hullMat = mats[sc.hull_material];
  const spineMat = mats[sc.spine_material];
  // The floor a person takes once a drum is full (the minimums shared out): Drum 1's figure.
  const full = maxPopulation(prog, mix, opts);
  const fullT = totalFor(prog, full, mix, opts);
  const floorPerPerson = fullT.zone.drum / full;
  const sectorFloor = floorPerPerson * sec.residents;
  const spineAllow = (spineMat.strength_mpa * 1e6) / sc.safety_factor;
  // A spine pulled (or pushed) at acceleration a carries everything ahead of (or behind) the
  // drive: its section grows along it and its mass is rho a L^2 m' / (2 sigma) for payload m' a
  // metre, a share rho a L / (2 sigma) of the payload. Holding that share sets the longest ship.
  const maxShipLength = (2 * sc.spine_mass_share * spineAllow) / (spineMat.density_kg_m3 * sc.manoeuvre_max_g * G);
  const materials = (prog.material || []).map((m) => (prog.drum_class || []).map((c) => hoop(m, c.radius_m, sc)));
  const classes = (prog.drum_class || []).map((c) => {
    const radius = c.radius_m;
    const length = sc.stability_l_over_r * radius;
    const floor = 2 * Math.PI * radius * length;
    const sectors = Math.floor(floor / sectorFloor);
    const people = sectors * sec.residents;
    const h = hoop(hullMat, radius, sc);
    const pitch = length + sc.drum_gap_m;
    // Per drum: the rim, two end walls (domes of radius R holding the air, membrane stress
    // p R / 2t), the contents, and the non-rotating shield tube over one pitch of the row.
    const endT = (sc.air_kpa * 1000 * radius) / (2 * ((hullMat.strength_mpa * 1e6) / sc.safety_factor));
    const rimKg = floor * h.kgPerM2;
    const endKg = 2 * (2 * Math.PI * radius * radius) * endT * hullMat.density_kg_m3;
    const contentsKg = floor * sc.contents_t_per_m2 * 1000;
    const shieldKg = 2 * Math.PI * radius * pitch * sc.shield_t_per_m2 * 1000;
    const drumKg = rimKg + endKg + contentsKg + shieldKg;
    const drumsPerShip = Math.max(1, Math.floor(maxShipLength / pitch));
    const shipLength = drumsPerShip * pitch;
    const shipPeople = drumsPerShip * people;
    const shipKg = drumsPerShip * drumKg * (1 + sc.spine_mass_share);
    // A spine PUSHED from the stern is a column: at the stern it carries the whole ship's
    // F = M a in a section A = F / sigma; as a thin tube of the class's spine radius r its I is
    // A r^2 / 2, and Euler's P = pi^2 E I / L^2 gives the longest unbraced run.
    let buckleLength = null;
    if (spineMat.modulus_gpa) {
      const force = (shipKg / (1 + sc.spine_mass_share)) * sc.manoeuvre_max_g * G;
      const area = force / spineAllow;
      const inertia = (area * c.spine_radius_m * c.spine_radius_m) / 2;
      buckleLength = Math.sqrt((Math.PI * Math.PI * spineMat.modulus_gpa * 1e9 * inertia) / force);
    }
    const fleets = sc.populations.map((pop) => {
      const sectorsNeeded = Math.ceil(pop / sec.residents);
      const drums = Math.ceil(sectorsNeeded / sectors);
      const ships = Math.ceil(drums / drumsPerShip);
      const oneShipLength = drums * pitch;
      const fleetKg = drums * drumKg * (1 + sc.spine_mass_share);
      const rockDiameter = Math.cbrt((6 * fleetKg) / (Math.PI * sc.rock_density_kg_m3));
      return {
        population: pop,
        sectors: sectorsNeeded,
        drums,
        ships,
        oneShipLength,
        // The share of its payload one spine would weigh if every drum were on it.
        oneShipSpineShare: (spineMat.density_kg_m3 * sc.manoeuvre_max_g * G * oneShipLength) / (2 * spineAllow),
        oneShipLightS: oneShipLength / 299792458,
        fleetKg,
        beltShare: fleetKg / sc.main_belt_kg,
        rockDiameter,
        perPersonT: fleetKg / pop / 1000,
      };
    });
    return {
      id: c.id,
      name: c.name,
      radius,
      rpm: rpm(radius),
      length,
      floor,
      sectors,
      people,
      hoop: h,
      pitch,
      drumKg,
      parts: { rimKg, endKg, contentsKg, shieldKg },
      drumsPerShip,
      shipLength,
      shipPeople,
      shipKg,
      spineRadius: c.spine_radius_m,
      buckleLength,
      fleets,
    };
  });
  return { floorPerPerson, sectorFloor, maxShipLength, spineAllowMpa: spineAllow / 1e6, materials, classes, drum1: { radius: prog.drum.drum_radius_m, length: prog.drum.drum_length_m, stableLength: sc.stability_l_over_r * prog.drum.drum_radius_m } };
}

// ---------------------------------------------------------------------------------------------
// Printing.

const fmt = (x) => Math.round(x).toLocaleString('en-US');
const km2 = (m2) => (m2 / 1e6).toFixed(m2 < 1e6 ? 3 : 2);

function report(prog, population, mix, showRows, opts = {}) {
  const t = totalFor(prog, population, mix, opts);
  const room = drumRoom(prog);
  const ok = fits(t, room);
  const out = [];
  const levels = opts.farmLevels || prog.farms.levels;
  out.push(`Space program for ${fmt(population)} people (${t.sectors} sector${t.sectors === 1 ? '' : 's'} of ${fmt(prog.sector.residents)}, ${t.neighbourhoods} neighbourhood${t.neighbourhoods === 1 ? '' : 's'}; the ship's farms feed ${fmt(t.fedByShip)} on ${levels} level${levels === 1 ? '' : 's'})`);
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
  out.push('By zone                    needed         Drum 1 offers');
  out.push(`  drum floor        ${km2(t.zone.drum).padStart(10)} km2   ${km2(room.drum).padStart(8)} km2   ${ok.drum ? 'fits' : 'DOES NOT FIT'} (${((100 * t.zone.drum) / room.drum).toFixed(0)}%)`);
  out.push(`  spine decks       ${km2(t.zone.spine).padStart(10)} km2   ${km2(room.spine).padStart(8)} km2   ${ok.spine ? 'fits' : 'DOES NOT FIT'} (${((100 * t.zone.spine) / room.spine).toFixed(0)}%, ${room.spineDecks} decks)`);
  out.push(`  bow decks         ${km2(t.zone.bow).padStart(10)} km2   ${km2(room.bow).padStart(8)} km2   ${ok.bow ? 'fits' : 'DOES NOT FIT'} (${((100 * t.zone.bow) / room.bow).toFixed(0)}%, ${room.bowDecks} decks)`);
  out.push(`  end caps          ${km2(t.zone.end_cap).padStart(10)} km2   (lift shafts and stairs)`);
  out.push(`  radiator fin      ${km2(t.zone.exterior).padStart(10)} km2   outside the hull (Drum 1's own hull is ${km2(room.hull_m2)} km2)`);
  const perPerson = (t.zone.drum + t.zone.spine + t.zone.bow) / population;
  out.push('');
  out.push(`Floor per person, all zones: ${fmt(perPerson)} m2 (drum ${fmt(t.zone.drum / population)}, spine ${fmt(t.zone.spine / population)}, bow ${fmt(t.zone.bow / population)})`);
  out.push(`Drum 1 ${ok.all ? 'HOLDS' : 'DOES NOT HOLD'} ${fmt(population)} people at this mix.`);
  out.push(`Drum 1 holds at most ${fmt(maxPopulation(prog, mix, opts))} people at this mix.`);
  if (!ok.all) out.push(`Drums of Drum 1's size needed: ${fmt(drumsNeeded(t, room))}.`);
  out.push(`Drum 1's shield (${prog.drum.shield_t_per_m2} t/m2 over ${km2(room.hull_m2)} km2 of hull): ${fmt(room.shield_t / 1e6)} million tonnes.`);
  return { text: out.join('\n'), ok: ok.all, total: t, room };
}

const mins = (s) => `${(s / 60).toFixed(1)} min`;

function sectorReport(prog, s) {
  const out = [];
  out.push(`The sector: ${fmt(s.residents)} residents (the operator, 2026-11-07), laid out on a drum of ${fmt(s.radius)} m radius (${s.rpm.toFixed(2)} rpm)`);
  out.push(`  layers inside it: ${s.layers.map(([n, p]) => `${n} ${fmt(p)}`).join(', ')}`);
  out.push('');
  out.push('Homes');
  for (const h of s.lone.homes) out.push(`  ${h.name.padEnd(10)} ${fmt(h.people).padStart(5)} people  ${fmt(h.homes).padStart(4)} ${h.unit}`);
  out.push('');
  out.push(`Drum floor: ${km2(s.loneFloor)} km2 for a sector on its own (${fmt(s.loneFloor / s.residents)} m2 a resident, the minimums for a hospital, a hangar bay and the like included);`);
  out.push(`  ${km2(s.sharedFloor)} km2 once Drum 1 is full and those minimums are shared (${fmt(s.sharedFloor / s.residents)} m2 a resident).`);
  out.push(`  spine decks ${km2(s.lone.zone.spine)} km2, bow decks ${km2(s.lone.zone.bow)} km2, radiator fin ${km2(s.lone.zone.exterior)} km2.`);
  out.push(`Drum 1 (${fmt(prog.drum.drum_length_m)} m long) holds ${fmt(s.drumFull)} people at this mix: ${fmt(s.sectorsInDrum1)} sectors.`);
  out.push('');
  out.push('How the sector lies on the drum (the three neighbourhoods side by side along its longer side):');
  for (const l of s.layouts) {
    out.push(`  ${l.name.padEnd(15)} ${fmt(l.around).padStart(6)} m around x ${fmt(l.along).padStart(5)} m along: node ${fmt(l.nodeWalk)} m (${mins(l.nodeWalkS)}), civic hall ${fmt(l.hallWalk)} m (${mins(l.hallWalkS)}) ${l.walksOk ? 'meets both walking rules' : 'BREAKS a walking rule'}`);
  }
  out.push(`  shortest walk to the civic hall that meets both rules: ${s.best.name}`);
  out.push('');
  out.push(`Spin stability: a free cylinder is stable only up to ${s.stableLength.toFixed(0)} m long at this radius (1.3 radii); Drum 1 is ${s.drum1LOverR.toFixed(1)} radii long.`);
  out.push(`  Two sectors as half rings fill the circumference of a drum ${fmt(s.halfRingLength)} m long (${(s.halfRingLength / s.radius).toFixed(2)} radii, ${s.halfRingLength <= s.stableLength ? 'inside' : 'PAST'} the limit) before its end walls.`);
  for (const n of s.loud) {
    out.push(`Night noise: ${n.id} (${n.lw} dB(A) behind ${n.wallLoss} dB walls) keeps a bedroom facade under ${prog.noise.night_facade_db} dB from ${n.enclosed.toFixed(1)} m; standing in the open it would need ${fmt(n.open)} m.`);
  }
  return out.join('\n');
}

const sci = (x) => x.toExponential(2).replace(/e\+?/, ' x 10^');

function scaleReport(prog, s) {
  const out = [];
  const sc = prog.scale;
  out.push(`One billion and ten billion (the operator, 2026-11-07). A sector is ${fmt(prog.sector.residents)} people; a full drum gives each ${fmt(s.floorPerPerson)} m2 of floor, ${km2(s.sectorFloor)} km2 a sector.`);
  out.push('');
  out.push(`Hoop stress at 1 g (working strength = strength / ${sc.safety_factor}; air ${sc.air_kpa} kPa and ${sc.contents_t_per_m2} t/m2 of contents on the rim):`);
  for (const row of s.materials) {
    const m = (prog.material || []).find((x) => x.id === row[0].material);
    out.push(`  ${m.name}: no drum wider than ${(row[0].maxRadius / 1000).toFixed(1)} km even empty; ${row.map((h) => `${fmt(h.radius)} m radius: wall ${h.thickness < 1 ? (h.thickness * 100).toFixed(1) + ' cm' : h.thickness.toFixed(2) + ' m'}, ${fmt(h.kgPerM2)} kg/m2`).join('; ')}`);
  }
  out.push('');
  out.push(`Drum 1 (${fmt(s.drum1.radius)} m x ${fmt(s.drum1.length)} m) is ${(s.drum1.length / s.drum1.radius).toFixed(1)} radii long; a free cylinder spins stably only up to ${sc.stability_l_over_r} radii (${fmt(s.drum1.stableLength)} m).`);
  out.push(`The spine (${sc.spine_material}, working ${fmt(s.spineAllowMpa)} MPa) at ${sc.manoeuvre_max_g} g, weighing at most ${sc.spine_mass_share * 100}% of what it carries: a ship at most ${fmt(s.maxShipLength / 1000)} km long.`);
  for (const c of s.classes) {
    out.push('');
    out.push(`${c.name}: ${fmt(c.length)} m long (the stability limit), ${km2(c.floor)} km2 of floor, ${fmt(c.sectors)} sectors, ${fmt(c.people)} people a drum`);
    out.push(`  a drum weighs ${sci(c.drumKg / 1000)} t: rim ${sci(c.parts.rimKg / 1000)}, end walls ${sci(c.parts.endKg / 1000)}, contents ${sci(c.parts.contentsKg / 1000)}, shield ${sci(c.parts.shieldKg / 1000)}`);
    out.push(`  a ship: ${fmt(c.drumsPerShip)} drums, ${fmt(c.shipLength / 1000)} km, ${fmt(c.shipPeople)} people, ${sci(c.shipKg / 1000)} t`);
    for (const f of c.fleets) {
      out.push(`  ${sci(f.population)} people: ${fmt(f.sectors)} sectors in ${fmt(f.drums)} drums. One ship would be ${fmt(f.oneShipLength / 1000)} km long (${f.oneShipLightS.toFixed(2)} light-seconds) and its spine, by an estimate that leaves out the spine's own weight, would weigh ${f.oneShipSpineShare.toFixed(0)} times what it carries: a FLEET of ${fmt(f.ships)} ships. ${sci(f.fleetKg / 1000)} t in all (${fmt(f.perPersonT)} t a person), ${sci(f.beltShare)} of the main belt's mass, one rock ${(f.rockDiameter / 1000).toFixed(1)} km across.`);
    }
    if (c.buckleLength) out.push(`  pushed from the stern, that spine (a tube of ${fmt(c.spineRadius)} m radius) buckles past ${fmt(c.buckleLength)} m between braces (Euler), so the drive pulls from the bow or every drum carries its own thrusters`);
  }
  return out.join('\n');
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
  const fi = args.indexOf('--farm-levels');
  const opts = {};
  try {
    if (mi >= 0) mix = parseMix(args[mi + 1] || '', prog);
    if (fi >= 0) {
      opts.farmLevels = Number(args[fi + 1]);
      if (!(Number.isInteger(opts.farmLevels) && opts.farmLevels >= 1)) throw new Error('--farm-levels takes a whole number of 1 or more');
    }
  } catch (e) {
    console.error(e.message);
    return 2;
  }
  if (args.includes('--sector')) {
    const s = sectorStudy(prog, mix, opts);
    if (args.includes('--json')) console.log(JSON.stringify({ ...s, lone: undefined, zone: s.lone.zone }, null, 2));
    else console.log(sectorReport(prog, s));
    return s.best.walksOk ? 0 : 1;
  }
  if (args.includes('--scale')) {
    const s = scaleStudy(prog, mix, opts);
    if (args.includes('--json')) console.log(JSON.stringify(s, null, 2));
    else console.log(scaleReport(prog, s));
    return 0;
  }
  const popArg = args.find((a, k) => /^\d[\d_,]*$/.test(a) && args[k - 1] !== '--mix' && args[k - 1] !== '--farm-levels');
  const population = popArg ? Number(popArg.replace(/[_,]/g, '')) : prog.defaults.population;
  if (!(population > 0)) {
    console.error('the population must be a whole number above 0');
    return 2;
  }
  const r = report(prog, population, mix, args.includes('--rows'), opts);
  if (args.includes('--json')) {
    console.log(JSON.stringify({ population, fits: r.ok, zone: r.total.zone, room: r.room, homes: r.total.homes }, null, 2));
  } else {
    console.log(r.text);
  }
  return r.ok ? 0 : 1;
}

module.exports = {
  parseToml,
  loadProgram,
  validateProgram,
  summarizeBlock,
  totalFor,
  drumRoom,
  fits,
  maxPopulation,
  drumsNeeded,
  parseMix,
  sectorLayouts,
  noiseDistance,
  sectorStudy,
  hoop,
  scaleStudy,
};

if (require.main === module) process.exitCode = main(process.argv);
