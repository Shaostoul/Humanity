#!/usr/bin/env node
// The starter survival garden, worked out from the numbers (docs/design/survival-garden.md).
//
// Question: the SMALLEST garden, with the FEWEST crops, that feeds one person everything plants can
// give (energy, protein with every essential amino acid, the essential fats, every vitamin and
// mineral USDA measures in crops), and what it cannot give.
//
// Method: a linear program. Each candidate crop gets an area (m2 of lit canopy). One square metre of
// a crop delivers so many grams of food a day (its cited yield, spacing and days in the bed, less
// USDA's refuse, the seed kept back and storage losses), and each gram carries USDA's nutrients
// (less what cooking takes, by USDA's retention factors). Minimise the total area so that every
// nutrient reaches its target. Then search every small set of crops to find the fewest that can do
// it, round each crop up to whole 1 m2 plots the game can plant, and report.
//
// Reads (nothing is typed into this file):
//   data/garden/survival_garden.json     the settings, per-crop extras, the gaps and how the ship covers them
//   data/food/crop_micronutrients.json   USDA nutrients per 100 g, refuse, retention factors
//   data/food/nutrient_requirements.json the daily targets (DRI, FAO amino acid pattern, person.ron tiers)
//   data/plants.csv, data/garden/yields.ron, data/garden/harvest_windows.ron, data/items.csv
//                                        the game's own yields, spacing, days and picking windows
//
// Usage:
//   node scripts/survival-garden.js                   the report (markdown) for both tiers
//   node scripts/survival-garden.js --tier bare       one tier
//   node scripts/survival-garden.js --residents 3     size for three people (every target times three)
//   node scripts/survival-garden.js --json            the result as JSON
//   node scripts/survival-garden.js --write           rewrite the generated block in the design doc
//   node scripts/survival-garden.js --check           exit 1 if the design doc's generated block is stale
//
// Plain node, no packages. The simplex below is the textbook two-phase method with Bland's rule,
// which cannot cycle; rows are scaled to their target so a microgram row and a kilocalorie row
// carry the same weight.

"use strict";

const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const DOC = path.join(ROOT, "docs", "design", "survival-garden.md");
const BEGIN = "<!-- survival-garden.js:begin (generated: node scripts/survival-garden.js --write) -->";
const END = "<!-- survival-garden.js:end -->";

// ---------------------------------------------------------------------------------------------
// Reading the game's files
// ---------------------------------------------------------------------------------------------

function readJson(rel) {
  return JSON.parse(fs.readFileSync(path.join(ROOT, rel), "utf8"));
}

/** A CSV with '#' comment lines and a header row; no quoted fields (true of both files read). */
function readCsv(rel) {
  const lines = fs.readFileSync(path.join(ROOT, rel), "utf8").split(/\r?\n/).filter((l) => l && !l.startsWith("#"));
  const head = lines.shift().split(",");
  return lines.map((l) => {
    const f = l.split(",");
    return Object.fromEntries(head.map((h, i) => [h, f[i]]));
  });
}

/** data/garden/yields.ron: (plant: "x", basis: B, per_plant_kg: (lo, hi)) rows. */
function readYields() {
  const txt = fs.readFileSync(path.join(ROOT, "data/garden/yields.ron"), "utf8");
  const out = {};
  const re = /\(plant:\s*"([a-z_]+)",\s*basis:\s*(\w+),\s*per_plant_kg:\s*\(([\d.]+),\s*([\d.]+)\)\)/g;
  let m;
  while ((m = re.exec(txt))) out[m[1]] = { basis: m[2], lo: Number(m[3]), hi: Number(m[4]) };
  return out;
}

/** data/garden/harvest_windows.ron: Once, or Picked(window_days, every_days). */
function readWindows() {
  const txt = fs.readFileSync(path.join(ROOT, "data/garden/harvest_windows.ron"), "utf8");
  const out = {};
  const re = /\(plant:\s*"([a-z_]+)",\s*harvest:\s*(Once|Picked\(window_days:\s*([\d.]+),\s*every_days:\s*([\d.]+)\))\)/g;
  let m;
  while ((m = re.exec(txt))) out[m[1]] = m[2] === "Once" ? { window: 0 } : { window: Number(m[3]), every: Number(m[4]) };
  return out;
}

// ---------------------------------------------------------------------------------------------
// The linear program
// ---------------------------------------------------------------------------------------------

/**
 * Minimise c.x subject to rows { a: number[], op: ">=" | "<=", b } and x >= 0.
 * Returns { ok, x, value } or { ok: false, why }.
 */
function simplex(c, rows) {
  const n = c.length;
  const EPS = 1e-9;
  // Normalise every row to b >= 0 (flipping the inequality with the sign).
  const R = rows.map((r) => (r.b < 0 ? { a: r.a.map((v) => -v), op: r.op === ">=" ? "<=" : ">=", b: -r.b } : r));
  const m = R.length;
  // Columns: n originals, then one slack/surplus per row, then one artificial per ">=" row.
  const nArt = R.filter((r) => r.op === ">=").length;
  const W = n + m + nArt;
  const T = []; // m rows of W + 1 (last column = rhs)
  const basis = [];
  let art = n + m;
  for (let i = 0; i < m; i++) {
    const row = new Array(W + 1).fill(0);
    for (let j = 0; j < n; j++) row[j] = R[i].a[j];
    row[W] = R[i].b;
    if (R[i].op === "<=") {
      row[n + i] = 1;
      basis.push(n + i);
    } else {
      row[n + i] = -1;
      row[art] = 1;
      basis.push(art);
      art++;
    }
    T.push(row);
  }
  function pivot(pr, pc) {
    const p = T[pr][pc];
    for (let j = 0; j <= W; j++) T[pr][j] /= p;
    for (let i = 0; i < m; i++) {
      if (i === pr) continue;
      const f = T[i][pc];
      if (Math.abs(f) < 1e-15) continue;
      for (let j = 0; j <= W; j++) T[i][j] -= f * T[pr][j];
    }
    basis[pr] = pc;
  }
  /** Run simplex on objective `obj` (length W, minimise) over allowed columns. */
  function run(obj, allowed) {
    for (let iter = 0; iter < 5000; iter++) {
      // Reduced costs: obj_j - sum(obj_basis * T_ij).
      let enter = -1;
      for (let j = 0; j < W; j++) {
        if (!allowed(j) || basis.includes(j)) continue;
        let rc = obj[j];
        for (let i = 0; i < m; i++) rc -= obj[basis[i]] * T[i][j];
        if (rc < -EPS) { enter = j; break; } // Bland: the lowest index that improves
      }
      if (enter < 0) return true;
      let leave = -1, best = Infinity;
      for (let i = 0; i < m; i++) {
        if (T[i][enter] > EPS) {
          const ratio = T[i][W] / T[i][enter];
          if (ratio < best - EPS || (Math.abs(ratio - best) <= EPS && basis[i] < basis[leave])) { best = ratio; leave = i; }
        }
      }
      if (leave < 0) return false; // unbounded
      pivot(leave, enter);
    }
    return false;
  }
  // Phase 1: minimise the sum of artificials.
  const obj1 = new Array(W).fill(0);
  for (let j = n + m; j < W; j++) obj1[j] = 1;
  run(obj1, () => true);
  let infeas = 0;
  for (let i = 0; i < m; i++) if (basis[i] >= n + m) infeas += T[i][W];
  if (infeas > 1e-7) return { ok: false, why: "infeasible" };
  // Drive any artificial left in the basis (at zero) out, where a non-artificial column can replace it.
  for (let i = 0; i < m; i++) {
    if (basis[i] < n + m) continue;
    for (let j = 0; j < n + m; j++) if (Math.abs(T[i][j]) > EPS && !basis.includes(j)) { pivot(i, j); break; }
  }
  // Phase 2: the real objective, artificials barred.
  const obj2 = new Array(W).fill(0);
  for (let j = 0; j < n; j++) obj2[j] = c[j];
  if (!run(obj2, (j) => j < n + m)) return { ok: false, why: "unbounded" };
  const x = new Array(n).fill(0);
  for (let i = 0; i < m; i++) if (basis[i] < n) x[basis[i]] = T[i][W];
  return { ok: true, x, value: x.reduce((s, v, j) => s + v * c[j], 0) };
}

// ---------------------------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------------------------

function load() {
  const garden = readJson("data/garden/survival_garden.json");
  const food = readJson("data/food/crop_micronutrients.json");
  const req = readJson("data/food/nutrient_requirements.json");
  const plants = Object.fromEntries(readCsv("data/plants.csv").map((p) => [p.id, p]));
  const items = Object.fromEntries(readCsv("data/items.csv").map((i) => [i.id, i]));
  return { garden, food, req, plants, items, yields: readYields(), windows: readWindows() };
}

/** Nutrients per 100 g of the food as eaten (USDA raw values times the cooking retention). */
function eatenPer100g(D, id) {
  const fc = D.food.crops[id];
  const prep = (D.garden.crops[id] || {}).prepared_as || "raw";
  const keep = prep === "raw" ? {} : (D.food.retention[prep] || { percent: {} }).percent;
  const out = {};
  for (const [k, v] of Object.entries(fc.per_100g)) out[k] = v === null ? null : v * (keep[k] === undefined ? 100 : keep[k]) / 100;
  return out;
}

/**
 * One square metre of a crop, one day, in a tier: grams harvested (gross, the harvest item as
 * weighed) and grams eaten, with where each factor came from. null when the tier has no yield for it.
 */
function productivity(D, id, tierYields) {
  const g = D.garden.crops[id] || {};
  const fc = D.food.crops[id];
  if (!fc) return null;
  const plotM2 = D.garden.layout.plot_m2;
  const refuse = (fc.refuse_pct || 0) / 100;
  const keepShare = 1 - refuse;
  const seed = g.seed_fraction || 0;
  const store = g.storage_loss || 0;
  if (tierYields === "cea" && g.cea && !g.cea_excluded) {
    const share = g.cea.edible_share !== undefined ? g.cea.edible_share : fc.edible_share;
    const gross = g.cea.edible_fresh_g_m2_d;
    return {
      source: "cea", grossG: gross, eatenG: gross * share * keepShare * (1 - seed) * (1 - store), days: g.cea.days,
      waterL: g.cea.transpiration_kg_m2_d, o2G: g.cea.o2_g_m2_d, inedibleG: g.cea.inedible_fresh_g_m2_d,
      plantsPerPlot: null, cycle: g.cea.days, firstHarvest: g.cea.days, dli: g.cea.dli,
    };
  }
  if (tierYields === "cea" && g.field_excluded) return null;
  // Field: the game's own numbers.
  const p = D.plants[id];
  let perPlantKg, areaPerPlant, src;
  if (g.proposed_row) {
    perPlantKg = g.proposed_row.per_plant_kg;
    areaPerPlant = g.proposed_row.area_per_plant_m2;
    src = "proposed";
  } else {
    const y = D.yields[id];
    if (!p || !y || !p.area_per_plant_m2) return null;
    perPlantKg = (y.lo + y.hi) / 2;
    areaPerPlant = Number(p.area_per_plant_m2);
    src = "field";
  }
  const growth = g.growth_days_override || Number(p.growth_days);
  const w = D.windows[id] || { window: 0 };
  const cycle = growth + w.window;
  const plantsPerPlot = Math.max(1, Math.floor(plotM2 / areaPerPlant + 1e-9));
  const grossKgPerM2 = (plantsPerPlot * perPlantKg) / plotM2;
  const gross = (grossKgPerM2 * 1000) / cycle;
  const waterPerPlant = Number(p.water_liters_per_day) || 0;
  return {
    source: src, floorWeight: g.floor_weight || 1, lpExcluded: !!g.lp_excluded, grossG: gross, eatenG: gross * fc.edible_share * keepShare * (1 - seed) * (1 - store),
    waterL: (plantsPerPlot * waterPerPlant) / plotM2, o2G: null, inedibleG: null,
    plantsPerPlot, cycle, firstHarvest: growth, window: w.window, perPlantKg, areaPerPlant,
    removal: { n: Number(p.removal_n_g_per_kg) || null, p2o5: Number(p.removal_p2o5_g_per_kg) || null, k2o: Number(p.removal_k2o_g_per_kg) || null },
  };
}

/**
 * The mushroom room's growing medium (2026-11-07). A crop's dry residue a square metre a day:
 * its edible dry matter x (1 - harvest index) / harvest index, with NASA's measured harvest index
 * (BVAD 2022, each crop's cea.harvest_index_pct) and USDA's water content; 0 for a crop without a
 * measured index (so the residue is an underestimate). A mushroom's dry substrate need a square
 * metre of shelf a day: its harvest over its biological efficiency (fresh mushrooms per dry
 * substrate).
 */
function residueDryG(D, id, prod) {
  const g = D.garden.crops[id] || {};
  const fc = D.food.crops[id];
  const hi = g.cea && g.cea.harvest_index_pct ? g.cea.harvest_index_pct / 100 : null;
  if (!hi || !fc || prod.floorWeight < 1) return 0;
  const dry = 1 - (fc.per_100g.water_g || 0) / 100;
  const edibleDry = prod.grossG * fc.edible_share * dry;
  return (edibleDry * (1 - hi)) / hi;
}

function substrateNeedG(D, id, prod) {
  const be = (D.garden.crops[id] || {}).biological_efficiency;
  return be ? prod.grossG / be : 0;
}

/** Grams of button mushroom a day the vitamin D shelf grows for `residents`, and its substrate. */
function vitaminDShelf(D, residents) {
  const uv = D.garden.treatments.uv_flash;
  const freshG = ((15 * residents) / uv.vitamin_d_ug_per_100g) * 100;
  const be = (D.garden.crops[uv.crop] || {}).biological_efficiency || 1;
  return { crop: uv.crop, freshG, substrateG: freshG / be };
}

/** The targets, as rows over nutrient keys: { key, label, unit, min, max, kind }. */
function targets(D, tierName, residents) {
  const tier = D.garden.tiers[tierName];
  const t = D.req.tiers[tier.person_tier];
  const rows = [];
  rows.push({ key: "energy_kcal", label: "Energy", unit: "kcal", min: t.energy_kcal, max: t.energy_kcal * tier.kcal_max_factor, held: true });
  rows.push({ key: "protein_g", label: "Protein", unit: "g", min: t.protein_g, held: true });
  const aa = D.req.amino_acids;
  for (const [k, mgPerG] of Object.entries(aa.mg_per_g_protein)) {
    rows.push({ key: k, label: k.replace(/_g$/, "").replace("_aa", " amino acids").replace(/_/g, " "), unit: "g", min: (mgPerG * t.protein_g) / 1000, held: true, amino: true });
  }
  for (const n of D.req.nutrients) {
    const target = Math.max(n.male, n.female) * (n.factor || 1);
    const held = n.from_plants === "yes"; // "medium" is reported, not held (nutrient_requirements.json _about)
    // A held UL is planned a little under its value (layout.ul_margin) so whole-plot rounding stays under it.
    const margin = D.garden.layout.ul_margin || 1;
    rows.push({ key: n.key, label: n.label, unit: n.unit, min: target, max: held && n.ul && !n.ul_note ? n.ul * margin : null, ul: n.ul || null, held, from: n.from_plants, kind: n.kind });
  }
  for (const r of rows) {
    r.min *= residents;
    if (r.max) r.max *= residents;
    if (r.ul) r.ul *= residents;
  }
  return rows;
}

/** Amount of a target's nutrient in one square metre-day of a crop. */
function coef(D, id, prod, key) {
  const per = eatenPer100g(D, id);
  const g = prod.eatenG / 100;
  const sums = D.req.amino_acids.sums;
  if (sums[key]) return sums[key].reduce((s, k) => s + (per[k] || 0), 0) * g;
  return (per[key] || 0) * g;
}

/** Every crop's amount of every target key (and fat) in one square metre-day, computed once. */
function coefTable(D, prods, T) {
  const C = {};
  for (const id of Object.keys(prods)) {
    C[id] = {};
    for (const t of T) C[id][t.key] = coef(D, id, prods[id], t.key);
    C[id].fat_g = coef(D, id, prods[id], "fat_g");
    C[id].carb_g = coef(D, id, prods[id], "carb_g");
    C[id].eaten_g = prods[id].eatenG;
    C[id].floor_weight = prods[id].floorWeight || 1;
    C[id].residue_g = residueDryG(D, id, prods[id]);
    C[id].substrate_g = substrateNeedG(D, id, prods[id]);
  }
  return C;
}

/** Solve the LP over a set of crops. Returns { ok, area: {id: m2}, total } */
function solve(D, tierName, ids, residents, C, T, opts = {}) {
  const tier = D.garden.tiers[tierName];
  const cap = opts.noMassCap ? null : tier.max_eaten_g_per_day;

  const A = (key) => ids.map((id) => C[id][key]);
  const rows = [];
  for (const t of T) {
    if (!t.held) continue;
    const a = A(t.key);
    rows.push({ a: a.map((v) => v / t.min), op: ">=", b: 1 });
    if (t.max) rows.push({ a: a.map((v) => v / t.max), op: "<=", b: 1 });
  }
  if (cap) rows.push({ a: ids.map((id) => C[id].eaten_g / (cap * residents)), op: "<=", b: 1 });
  const perCrop = opts.noMassCap ? null : tier.max_eaten_g_per_crop;
  if (perCrop) ids.forEach((id, i) => rows.push({ a: ids.map((jd, j) => (j === i ? C[jd].eaten_g / (perCrop * residents) : 0)), op: "<=", b: 1 }));
  if (opts.maxShelf) rows.push({ a: ids.map((id) => (C[id].floor_weight < 1 ? 1 / opts.maxShelf : 0)), op: "<=", b: 1 });
  // THE GROWING MEDIUM (2026-11-07): the mushrooms eat no more substrate than the garden's own
  // residue can become, after the vitamin D shelf has taken its share:
  //   sum(mushroom need x) - usable x sum(residue x) <= -(the vitamin D shelf's need).
  if (opts.substrate) {
    const u = opts.substrate.usable;
    const scale = 100;
    rows.push({ a: ids.map((id) => (C[id].substrate_g - u * C[id].residue_g) / scale), op: "<=", b: -opts.substrate.reservedG / scale });
  }
  const kcal = A("energy_kcal");
  const fat = A("fat_g");
  if (tier.fat_pct_energy) {
    const [lo, hi] = tier.fat_pct_energy;
    const E = T[0].min;
    rows.push({ a: ids.map((_, j) => (9 * fat[j] - (lo / 100) * kcal[j]) / E), op: ">=", b: 0 });
    rows.push({ a: ids.map((_, j) => (9 * fat[j] - (hi / 100) * kcal[j]) / E), op: "<=", b: 0 });
  }
  if (tier.max_kcal_share_per_crop) {
    const s = tier.max_kcal_share_per_crop;
    const E = T[0].min;
    ids.forEach((_, i) => rows.push({ a: ids.map((__, j) => ((i === j ? kcal[j] : 0) - s * kcal[j]) / E), op: "<=", b: 0 }));
  }
  for (const [id, m2] of Object.entries(tier.variety_min_m2 || {})) {
    const i = ids.indexOf(id);
    if (i >= 0) rows.push({ a: ids.map((_, j) => (j === i ? 1 / (m2 * residents) : 0)), op: ">=", b: 1 });
  }
  const r = simplex(ids.map((id) => C[id].floor_weight), rows);
  if (!r.ok) return { ok: false };
  return { ok: true, area: Object.fromEntries(ids.map((id, j) => [id, r.x[j]]).filter(([, v]) => v > 1e-6)), total: r.value };
  // total is floor area: lit canopy plus mushroom shelf over its stack.
}

function* combos(arr, k, start = 0, pre = []) {
  if (pre.length === k) { yield pre; return; }
  for (let i = start; i <= arr.length - (k - pre.length); i++) yield* combos(arr, k, i + 1, [...pre, arr[i]]);
}

/** Supply of every target from areas (m2 per crop). */
function supply(C, area, T) {
  const out = { fat_g: 0, carb_g: 0 };
  for (const t of T) out[t.key] = 0;
  for (const [id, m2] of Object.entries(area)) for (const k of Object.keys(out)) out[k] += C[id][k] * m2;
  return out;
}

function analyse(D, tierName, residents, opts = {}) {
  const variant = (D.garden.variants || {})[tierName];
  const baseName = variant ? variant.base : tierName;
  let tier = D.garden.tiers[baseName];
  if (variant) tier = { ...tier, label: variant.label, search_max_crops: 0, choose: 'all' };
  if (opts.onlyIds) tier = { ...tier, search_max_crops: 0, choose: 'all' };
  // The mushroom room (adopted in the bare tier 2026-11-07): its species join the candidates, its
  // racks cap the shelf, and the growing medium caps what they eat (solve's substrate row).
  const room = tier.mushroom_room || (variant && variant.extra_candidates ? { candidates: variant.extra_candidates, max_shelf_m2: variant.max_shelf_m2 } : null);
  const candidates = [...D.garden.candidates, ...((room && room.candidates) || [])];
  const T = targets(D, baseName, residents);
  const prods = {};
  for (const id of candidates) {
    const p = productivity(D, id, tier.yields);
    if (p && p.eatenG > 0) prods[id] = p;
  }
  // In the comfort tier a crop without a hydroponic figure still grows at its field yield.
  if (tier.yields === "cea") {
    for (const id of candidates) {
      if (prods[id]) continue;
      const g = D.garden.crops[id] || {};
      if (g.field_excluded) continue;
      const p = productivity(D, id, "field");
      if (p && p.eatenG > 0) prods[id] = p;
    }
  }
  // A crop whose field daily rate the game overstates (lp_excluded) may be a variety plot, never a reason.
  for (const id of Object.keys(prods)) if (prods[id].lpExcluded && !(tier.variety_min_m2 || {})[id]) delete prods[id];
  if (opts.onlyIds) for (const id of Object.keys(prods)) if (!opts.onlyIds.includes(id)) delete prods[id];
  const ids = Object.keys(prods);
  const C = coefTable(D, prods, T);
  const so = {
    maxShelf: room && room.max_shelf_m2 ? room.max_shelf_m2 * residents : null,
    substrate: room && D.garden.substrate ? { usable: D.garden.substrate.residue_usable_share, reservedG: vitaminDShelf(D, residents).substrateG } : null,
  };
  let full = solve(D, baseName, ids, residents, C, T, so);
  // A garden whose residue cannot even carry the vitamin D shelf grows no mushrooms for food, and
  // the shelf falls back to the supplement (the operator's "both", 2026-11-07); the program says so.
  if (!full.ok && so.substrate && so.substrate.reservedG > 0) {
    so.substrate = { ...so.substrate, reservedG: 0, vitaminDFallback: true };
    full = solve(D, baseName, ids, residents, C, T, so);
  }
  const uncapped = solve(D, baseName, ids, residents, C, T, { ...so, noMassCap: true });
  // A calorie plan ignores every other need, the vitamin D shelf's substrate among them.
  const soEnergy = { ...so, substrate: so.substrate ? { ...so.substrate, reservedG: 0 } : null };
  const energyOnly = solve(D, baseName, ids, residents, C, T.map((t) => (t.key === 'energy_kcal' || t.key === 'protein_g' ? t : { ...t, held: false })), soEnergy);
  if (!full.ok) return { tierName, T, prods, C, infeasible: true };

  // THE FEWEST CROPS. Three steps, each cheap:
  //  1. A necessary test that throws most sets away without an LP: with eaten energy capped at
  //     kcal_max, a set can supply at most cap x (its best nutrient-per-kcal) of any nutrient.
  //  2. Every set of k crops for k = 1, 2, ... while the number of sets stays under EXHAUSTIVE_MAX,
  //     so the smallest feasible k is proven, not guessed.
  //  3. Past that, the sets are drawn from a short pool: the crops of the full LP and of every best
  //     set found so far (said in the report).
  const EXHAUSTIVE_MAX = 60000;
  const heldMins = T.filter((t) => t.held && t.key !== "energy_kcal");
  const capKcal = T[0].max || Infinity;
  const canCover = (set) => heldMins.every((t) => {
    let best = 0;
    for (const id of set) {
      const e = C[id].energy_kcal;
      const v = C[id][t.key];
      if (v <= 0) continue;
      best = Math.max(best, e > 0 ? v / e : Infinity);
    }
    return best * capKcal >= t.min * (1 - 1e-9);
  });
  const nChooseK = (n, k) => { let r = 1; for (let i = 1; i <= k; i++) r = (r * (n - k + i)) / i; return Math.round(r); };
  const byK = [];
  const varietyIds = Object.keys(tier.variety_min_m2 || {}).filter((id) => prods[id]);
  const fullPool = ids.filter((id) => !varietyIds.includes(id));
  const shortPool = new Set(Object.keys(full.area).filter((id) => !varietyIds.includes(id)));
  const kMax = tier.search_max_crops || 0;
  let firstFeasibleK = null;
  for (let k = 1; k <= kMax; k++) {
    const exhaustive = nChooseK(fullPool.length, k) <= EXHAUSTIVE_MAX;
    const pool = exhaustive ? fullPool : [...shortPool];
    if (k > pool.length) break;
    let best = null, feasible = 0, tried = 0;
    const near = [];
    for (const set of combos(pool, k)) {
      const all = [...varietyIds, ...set];
      if (!canCover(all)) continue;
      tried++;
      const r = solve(D, baseName, all, residents, C, T, so);
      if (!r.ok) continue;
      feasible++;
      near.push({ total: r.total, set: all, area: r.area });
      if (!best || r.total < best.total - 1e-9) best = { ...r, set: all };
    }
    near.sort((x, y) => x.total - y.total);
    byK.push({ k, tried, feasible, best, exhaustive, poolSize: pool.length, near: near.slice(0, 8) });
    if (best) for (const id of best.set) if (!varietyIds.includes(id)) shortPool.add(id);
    if (best && firstFeasibleK === null) firstFeasibleK = k;
    if (firstFeasibleK !== null && k >= firstFeasibleK + 2) break;
  }
  // The chosen garden: the fewest crops (the minimalist answer) for the bare tier, the full LP for comfort.
  let chosen = full;
  if (tier.choose === "fewest") {
    const few = byK.find((b) => b.best);
    chosen = few ? few.best : full;
  } else if (tier.choose === "knee") {
    const knee = byK.find((b) => b.best && b.best.total <= full.total * (1 + (tier.area_tolerance || 0)) + 1e-9);
    chosen = knee ? knee.best : full;
  }
  // WHOLE PLOTS. The game plants 1 m2 plots, so the LP's areas become whole plots: every crop is
  // rounded UP (every minimum then holds, because every crop adds to every nutrient), then plots are
  // taken back one at a time while every minimum still holds. Each step takes the plot that most
  // reduces what is over a limit (a UL or the energy ceiling first, then the estimated caps on food
  // eaten and on shelf), and among equals the one that saves the most floor. It stops when no plot
  // can go without breaking a minimum. Minimums are never traded away; a cap may stay a little over.
  const plotM2 = D.garden.layout.plot_m2;
  // A mushroom's unit is one rack shelf (0.72 m2), not a 1 m2 plot: the rack is built in shelves.
  const unitOf = (id) => (C[id].floor_weight < 1 ? D.garden.layout.mushroom_rack_shelf_m2 : plotM2);
  const plots = {};
  const vminOf = (id) => Math.ceil(((tier.variety_min_m2 || {})[id] || 0) * residents / plotM2 - 1e-9);
  for (const [id, m2] of Object.entries(chosen.area)) plots[id] = Math.max(1, vminOf(id), Math.ceil(m2 / unitOf(id) - 1e-6));
  const toArea = () => Object.fromEntries(Object.entries(plots).map(([id, n]) => [id, n * unitOf(id)]));
  // The growing medium as a hard limit in the rounding, like a UL: what the mushrooms need over
  // what the residue gives, as a fraction over.
  const substrateOver = (area) => {
    if (!so.substrate) return 0;
    const need = Object.entries(area).reduce((s, [id, m2]) => s + C[id].substrate_g * m2, 0) + so.substrate.reservedG;
    const give = so.substrate.usable * Object.entries(area).reduce((s, [id, m2]) => s + C[id].residue_g * m2, 0);
    return need > give ? (give > 0 ? need / give - 1 : need) : 0;
  };
  const minsHold = (area) => { const sp = supply(C, area, T); return T.every((t) => !t.held || sp[t.key] >= t.min * (1 - 1e-9)); };
  // [hard excess (ULs, the energy ceiling), soft excess (estimated caps)], each as a sum of fractions over.
  const excess = (area) => {
    const sp = supply(C, area, T);
    let hard = substrateOver(area), soft = 0;
    for (const t of T) if (t.held && t.max && sp[t.key] > t.max) hard += sp[t.key] / t.max - 1;
    const eaten = Object.entries(area).map(([id, m2]) => C[id].eaten_g * m2);
    const capG = tier.max_eaten_g_per_day ? tier.max_eaten_g_per_day * residents : null;
    const sumE = eaten.reduce((x, y) => x + y, 0);
    if (capG && sumE > capG) soft += sumE / capG - 1;
    const capC = tier.max_eaten_g_per_crop ? tier.max_eaten_g_per_crop * residents : null;
    if (capC) for (const g of eaten) if (g > capC) soft += g / capC - 1;
    if (so.maxShelf) {
      const sh = Object.entries(area).filter(([id]) => C[id].floor_weight < 1).reduce((x, [, m2]) => x + m2, 0);
      if (sh > so.maxShelf) soft += sh / so.maxShelf - 1;
    }
    return [hard, soft];
  };
  for (let guard = 0; guard < 2000; guard++) {
    const [h0, s0] = excess(toArea());
    let best = null;
    for (const id of Object.keys(plots)) {
      if (plots[id] - 1 < Math.max(1, vminOf(id))) continue;
      plots[id]--;
      if (minsHold(toArea())) {
        const [h, sft] = excess(toArea());
        const key = [h0 - h, s0 - sft, C[id].floor_weight || 1];
        if (!best || key[0] > best.key[0] + 1e-12 || (Math.abs(key[0] - best.key[0]) <= 1e-12 && (key[1] > best.key[1] + 1e-12 || (Math.abs(key[1] - best.key[1]) <= 1e-12 && key[2] > best.key[2])))) best = { id, key };
      }
      plots[id]++;
    }
    if (!best) break;
    plots[best.id]--;
  }
  const plotArea = toArea();
  const costs = [];
  for (const t of T) {
    if (!t.held) continue;
    const lpS = supply(C, chosen.area, T)[t.key];
    if (lpS > t.min * 1.005) continue;
    const T2 = T.map((x) => (x.key === t.key ? { ...x, min: x.min * 0.9 } : x));
    const r = solve(D, baseName, Object.keys(chosen.area).length && tier.choose === 'knee' ? chosen.set || Object.keys(chosen.area) : ids, residents, C, T2, so);
    if (r.ok && chosen.total - r.total >= 0.05) costs.push({ label: t.label, saved: chosen.total - r.total });
  }
  return { tierName, tier, T, prods, full, uncapped, energyOnly, byK, costs, firstFeasibleK, chosen, plots, plotArea, C, lpSupply: supply(C, chosen.area, T), plotSupply: supply(C, plotArea, T), residents, substrate: substrateBalance(D, C, plotArea, residents), mushroomRoom: !!room, vitaminDFallback: !!(so.substrate && so.substrate.vitaminDFallback) };
}

/**
 * The growing medium of a garden as planted: the residue its crops give, what its mushrooms and
 * the vitamin D shelf take, and what preparing it costs (2026-11-07, data/garden/survival_garden.json
 * `substrate`). Grams and litres a day; the spent substrate's phosphorus a year.
 */
function substrateBalance(D, C, area, residents) {
  const S = D.garden.substrate;
  if (!S) return null;
  const residue = Object.entries(area).map(([id, m2]) => [id, C[id].residue_g * m2]).filter(([, g]) => g > 0).sort((x, y) => y[1] - x[1]);
  const residueG = residue.reduce((s, [, g]) => s + g, 0);
  const usableG = residueG * S.residue_usable_share;
  const mushrooms = Object.entries(area).filter(([id]) => C[id].floor_weight < 1).map(([id, m2]) => [id, C[id].substrate_g * m2]);
  const vit = vitaminDShelf(D, residents);
  const needG = mushrooms.reduce((s, [, g]) => s + g, 0) + vit.substrateG;
  const mushroomFreshG = Object.entries(area).filter(([id]) => C[id].floor_weight < 1).reduce((s, [id, m2]) => s + C[id].substrate_g * m2 * (D.garden.crops[id].biological_efficiency || 0), 0) + vit.freshG;
  const spentDryG = Math.max(0, needG - mushroomFreshG * (1 - S.mushroom_water_share));
  return {
    residue,
    residueG,
    usableG,
    mushrooms,
    vitaminD: vit,
    needG,
    coverage: needG > 0 ? usableG / needG : Infinity,
    pasteuriseKwh: (needG / 1000) * S.pasteurise_kj_per_kg_dry / 3600,
    waterL: (needG / 1000) * S.moisture / (1 - S.moisture),
    spentDryG,
    spentPgYr: S.spent.p_pct.map((p) => (spentDryG * 365 * p) / 100),
  };
}

// ---------------------------------------------------------------------------------------------
// The report
// ---------------------------------------------------------------------------------------------

const f0 = (v) => (v === null || v === undefined ? "-" : Math.round(v).toLocaleString("en-US"));
const f1 = (v) => (v === null || v === undefined ? "-" : (Math.round(v * 10) / 10).toLocaleString("en-US", { minimumFractionDigits: 1, maximumFractionDigits: 1 }));
const f2 = (v) => (v === null || v === undefined ? "-" : (Math.round(v * 100) / 100).toFixed(2));
const sig = (v) => (Math.abs(v) >= 100 ? f0(v) : Math.abs(v) >= 10 ? f1(v) : f2(v));
const M2_FT2 = 10.7639;
const KG_LB = 2.20462;
const L_GAL = 0.264172;
const dual = (m2) => `${f1(m2)} m2 (${f0(m2 * M2_FT2)} sq ft)`;

function cropName(D, id) {
  return D.plants[id] ? D.plants[id].name : id;
}

function report(D, a) {
  const L = [];
  const { T, prods, plots, plotArea, plotSupply, tier } = a;
  const res = a.residents;
  const per = res > 1 ? ` for ${res} residents` : "";
  const start = D.garden.start_tier === a.tierName ? " (THE START GARDEN: every new homesteader begins with it, the operator's choice of 2026-11-07)" : "";
  L.push(`### Tier: ${tier.label}${per}${start}`);
  L.push("");
  if (a.infeasible) { L.push("No garden of the candidate crops meets every target under this tier's rules."); return L.join("\n"); }
  // Lit canopy (green crops) and mushroom shelf (stacked, in the dark) are counted apart.
  const isShelf = (id) => (prods[id].floorWeight || 1) < 1;
  const lit = Object.entries(plotArea).filter(([id]) => !isShelf(id)).reduce((s, [, v]) => s + v, 0);
  const shelf = Object.entries(plotArea).filter(([id]) => isShelf(id)).reduce((s, [, v]) => s + v, 0);
  const nPlots = Object.entries(plots).filter(([id]) => !isShelf(id)).reduce((s, [, v]) => s + v, 0);
  const sunline = D.garden.sunline;
  const kwhPerM2 = (sunline.dli_mol_m2_d * 1e6) / sunline.led_umol_per_j / 3.6e6;
  const fullShelf = Object.keys(a.full.area).filter((id) => isShelf(id));
  L.push(`- Smallest area, any crops (the linear program over all ${Object.keys(prods).length} candidates): **${dual(a.full.total)}** of ${fullShelf.length ? "floor (canopy plus a fifth of the mushroom shelf)" : "canopy"}, ${Object.keys(a.full.area).length} crops${fullShelf.length ? ` (${fullShelf.map((id) => cropName(D, id).toLowerCase() + " " + f1(a.full.area[id]) + " m2 of shelf").join(", ")}, as much as the garden's residue can grow)` : ""}.`);
  if (a.byK.length) {
    const rows = a.byK.map((b) => `${b.k}: ${b.best ? dual(b.best.total) + " (" + b.best.set.map((id) => cropName(D, id)).join(", ") + ")" : "none"}`);
    L.push(`- Smallest area with exactly k crops${Object.keys(tier.variety_min_m2 || {}).length ? " beyond the variety plots" : ""}, k = ${rows.join("; ")}.`);
  }
  if (a.energyOnly && a.energyOnly.ok) L.push(`- Energy and protein alone (the way a calorie plan sizes a farm, every other nutrient ignored): ${dual(a.energyOnly.total)} (${Object.entries(a.energyOnly.area).map(([id, m2]) => cropName(D, id) + ' ' + f1(m2)).join(', ')}). The rest of the area is what the vitamins, minerals and fats cost.`);
  if (tier.max_eaten_g_per_day && a.uncapped && a.uncapped.ok) {
    const ue = Object.entries(a.uncapped.area).reduce((sum, [id, m2]) => sum + a.C[id].eaten_g * m2, 0);
    L.push(`- Without the caps on food eaten (${f1(tier.max_eaten_g_per_day / 1000)} kg, ${f1((tier.max_eaten_g_per_day / 1000) * KG_LB)} lb, a day${tier.max_eaten_g_per_crop ? `, and ${f1(tier.max_eaten_g_per_crop / 1000)} kg of any one crop` : ''}), the smallest garden would be ${dual(a.uncapped.total)} (${Object.entries(a.uncapped.area).map(([id, m2]) => cropName(D, id) + ' ' + f1(m2)).join(', ')}), feeding ${f1(ue / 1000)} kg (${f1((ue / 1000) * KG_LB)} lb) of food a day.`);
  }
  const kneeK = a.byK.find((b) => b.best && b.best.set === (a.chosen.set || null));
  if (kneeK && kneeK.near.length > 1) {
    L.push(`- The closest ${kneeK.k}-crop gardens (near-ties: a few percent is inside the uncertainty of the yields and the estimates): ${kneeK.near.slice(0, 6).map((n) => n.set.map((id) => cropName(D, id)).join(' + ') + ' ' + f1(n.total)).join('; ')} m2.`);
  }
  if (a.costs && a.costs.length) {
    L.push(`- What each binding target costs (canopy saved if that one target were 10 percent lower): ${a.costs.map((c) => c.label + ' ' + f1(c.saved) + ' m2').join(', ')}.`);
  }
  const rack = D.garden.layout.mushroom_rack_shelf_m2 * D.garden.layout.mushroom_rack_shelves;
  L.push(`- The garden, rounded up to whole 1 m2 plots: **${dual(lit)} of lit canopy**, ${nPlots} plots in ${Math.ceil(nPlots / 2)} beds of 2 m2; with paths (x${D.garden.layout.path_factor}) **${dual(lit * D.garden.layout.path_factor)}** of floor.${shelf > 0 ? ` Plus ${dual(shelf)} of mushroom shelf, ${Math.ceil(shelf / rack - 1e-9)} rack${Math.ceil(shelf / rack - 1e-9) > 1 ? "s" : ""} of ${f1(rack)} m2.` : ""}`);
  L.push(`- Light, if the sunline is lamps: ${f1(lit * kwhPerM2)} kWh a day (${f2(kwhPerM2)} kWh per m2 at ${sunline.dli_mol_m2_d} mol and ${sunline.led_umol_per_j} umol/J, photons at the canopy only).`);
  L.push("");
  L.push("| Crop | Plots (m2) | Plants a plot | Days in bed | Sow a plot every | Harvest a day | Eaten a day | kcal a day | Protein | Water to air a day | Yield source |");
  L.push("|---|---|---|---|---|---|---|---|---|---|---|");
  let totG = 0, totEat = 0, totW = 0;
  for (const [id, n] of Object.entries(plots).sort((x, y) => y[1] - x[1])) {
    const p = prods[id];
    const m2 = plotArea[id];
    const harvest = p.grossG * m2;
    const eaten = p.eatenG * m2;
    const kc = coef(D, id, p, "energy_kcal") * m2;
    const pr = coef(D, id, p, "protein_g") * m2;
    const w = p.waterL * m2;
    totG += harvest; totEat += eaten; totW += w;
    const every = p.cycle / n;
    L.push(`| ${cropName(D, id)} | ${isShelf(id) ? `${n} shelves (${f2(m2)} m2)` : n} | ${p.plantsPerPlot || "-"} | ${f0(p.cycle)}${p.window ? ` (${f0(p.firstHarvest)} + ${f0(p.window)} picking)` : ""} | ${f1(every)} days | ${f0(harvest)} g (${f2((harvest / 1000) * KG_LB)} lb) | ${f0(eaten)} g | ${f0(kc)} | ${f1(pr)} g | ${f1(w)} L (${f2(w * L_GAL)} gal) | ${p.source === "cea" ? "BVAD 2022 (hydroponic)" : p.source === "proposed" ? "NCSU CEFS (proposed row)" : "plants.csv / yields.ron"} |`);
  }
  const nShelves = Object.entries(plots).filter(([id]) => isShelf(id)).reduce((s, [, v]) => s + v, 0);
  L.push(`| **Total** | ${nPlots}${nShelves ? ` + ${nShelves} shelves` : ""} | | | | ${f0(totG)} g (${f2((totG / 1000) * KG_LB)} lb) | ${f0(totEat)} g | ${f0(plotSupply.energy_kcal)} | ${f1(plotSupply.protein_g)} g | ${f1(totW)} L (${f2(totW * L_GAL)} gal) | |`);
  L.push("");
  // The nutrient table.
  L.push("| Nutrient | Daily target | Supplied | Percent | Gap or margin | Notes |");
  L.push("|---|---|---|---|---|---|");
  const lim = [];
  for (const t of T) {
    const s = plotSupply[t.key];
    const pct = t.min ? (100 * s) / t.min : null;
    let note = "";
    if (!t.held) note = t.from === "no" ? "plants cannot: see the gaps" : t.from === "unmeasured" ? "USDA does not measure it in these foods" : t.from === "medium" ? "not held: the bed's medium decides it (USDA's value is US farm soil); see the gaps" : "";
    if (t.amino) note = pct < 100 / 0.85 ? "would not survive 85% digestibility" : "survives 85% digestibility";
    if (t.ul && s > t.ul) note += (note ? "; " : "") + `above the UL (${sig(t.ul)})`;
    const lpPct = t.min ? (100 * a.lpSupply[t.key]) / t.min : null;
    if (t.held && lpPct !== null && lpPct < 100.5) lim.push(t.label);
    if (t.max && a.lpSupply[t.key] > t.max * 0.995) lim.push(t.label + ' (its ceiling)');
    const gap = !t.held ? (s >= t.min ? "met at USDA values" : t.from === "unmeasured" ? "unknown" : `${sig(t.min - s)} ${t.unit} short`) : pct >= 100 ? `+${f0(pct - 100)}%` : `${sig(t.min - s)} ${t.unit} short`;
    L.push(`| ${t.label} | ${sig(t.min)}${t.max && t.key === "energy_kcal" ? " to " + sig(t.max) : ""} ${t.unit} | ${t.held || s > 0 ? sig(s) : "-"} ${t.unit} | ${pct === null ? "-" : f0(pct) + "%"} | ${gap} | ${note} |`);
  }
  L.push("");
  if (tier.max_eaten_g_per_day && Object.entries(a.chosen.area).reduce((sum, [id, m2]) => sum + a.C[id].eaten_g * m2, 0) > tier.max_eaten_g_per_day * a.residents * 0.995) lim.push("the cap on food eaten");
  if (tier.max_eaten_g_per_crop) for (const [id, m2] of Object.entries(a.chosen.area)) if (a.C[id].eaten_g * m2 > tier.max_eaten_g_per_crop * a.residents * 0.995) lim.push(`the cap on ${cropName(D, id).toLowerCase()} eaten`);
  if (lim.length) L.push(`Binding in the linear program (these set the garden's size; rounding up to whole plots adds the margins above): ${lim.join(", ")}.`);
  const kc = plotSupply.energy_kcal;
  L.push(`Share of energy (at 9, 4 and 4 kcal a gram): fat ${f1((100 * 9 * plotSupply.fat_g) / kc)}%, protein ${f1((100 * 4 * plotSupply.protein_g) / kc)}%, carbohydrate ${f1((100 * 4 * plotSupply.carb_g) / kc)}%; the DRI ranges are 20 to 35, 10 to 35 and 45 to 65.`);
  L.push("");
  return { text: L.join("\n"), lit, totEat, totW };
}

function loops(D, a) {
  // Nutrients out in the harvest and water, per year; oxygen; the vitamin D shelf.
  const L = [];
  const { prods, plotArea } = a;
  let n = 0, p2o5 = 0, k2o = 0, unsourced = [];
  for (const [id, m2] of Object.entries(plotArea)) {
    const p = prods[id];
    const kgYr = (p.grossG * m2 * 365) / 1000;
    const rm = p.removal || (D.plants[id] ? { n: Number(D.plants[id].removal_n_g_per_kg) || null, p2o5: Number(D.plants[id].removal_p2o5_g_per_kg) || null, k2o: Number(D.plants[id].removal_k2o_g_per_kg) || null } : {});
    if (!rm.n) unsourced.push(id);
    n += (kgYr * (rm.n || 0)) / 1000;
    p2o5 += (kgYr * (rm.p2o5 || 0)) / 1000;
    k2o += (kgYr * (rm.k2o || 0)) / 1000;
  }
  const P = p2o5 * 0.4364, K = k2o * 0.8301;
  const dietP = (a.plotSupply.phosphorus_mg * 365) / 1e6;
  const dietK = (a.plotSupply.potassium_mg * 365) / 1e6;
  const dietN = (a.plotSupply.protein_g / 6.25 * 365) / 1000;
  L.push(`- Carried out in the harvest a year (plants.csv removal columns): N ${f1(n)} kg (${f1(n * KG_LB)} lb), P ${f2(P)} kg (P2O5 ${f2(p2o5)}), K ${f1(K)} kg (K2O ${f1(k2o)})${unsourced.length ? `; no removal figure for ${unsourced.join(", ")}` : ""}.`);
  L.push(`- Eaten a year (USDA, the same garden): N ${f1(dietN)} kg (protein / 6.25), P ${f2(dietP)} kg, K ${f1(dietK)} kg. What is eaten leaves the body again: data/closed_loop/person.ron puts one person's excreta at 4.65 kg N and 0.44 to 0.61 kg P a year.`);
  // Oxygen from the food: the body burns what it eats.
  const mj = (a.plotSupply.energy_kcal * 4.184) / 1000;
  L.push(`- Oxygen: eating ${f0(a.plotSupply.energy_kcal)} kcal a day burns ${f2(mj * 0.07004)} kg of oxygen (person.ron, 0.0700 kg per MJ). The crops made at least that much building the food, and more for their stalks and leaves, which give it back to the air only when they are composted.`);
  const ceaO2 = Object.entries(plotArea).reduce((s, [id, m2]) => s + (prods[id].o2G ? prods[id].o2G * m2 : 0), 0);
  if (ceaO2 > 0) L.push(`- Oxygen made by the crops with a BVAD figure (Table 4-91, whole plant): ${f2(ceaO2 / 1000)} kg a day.`);
  // The vitamin D shelf.
  const uv = D.garden.treatments.uv_flash;
  const y = D.yields[uv.crop], pl = D.plants[uv.crop], w = D.windows[uv.crop] || { window: 0 };
  if (y && pl) {
    const shelf = D.garden.layout.mushroom_rack_shelf_m2;
    const perShelf = Math.floor(shelf / Number(pl.area_per_plant_m2) + 1e-9);
    const cycle = Number(pl.growth_days) + w.window;
    const gDay = (perShelf * ((y.lo + y.hi) / 2) * 1000) / cycle * (1 - (D.food.crops[uv.crop].refuse_pct || 0) / 100);
    const need = (15 * a.residents / uv.vitamin_d_ug_per_100g) * 100;
    L.push(`- Vitamin D from UV-flashed ${cropName(D, uv.crop).toLowerCase()}s: ${f0(need)} g a day at ${uv.vitamin_d_ug_per_100g} ug/100 g; one ${f2(shelf)} m2 shelf of the mushroom rack holds ${perShelf} square feet of bed and gives ${f0(gDay)} g a day (${f0(cycle)} days a crop), so **${Math.ceil(need / gDay)} shelf** (${f0((100 * need) / gDay)} percent of one).`);
  }
  // The growing medium (2026-11-07).
  const s = a.substrate;
  if (s) {
    const S = D.garden.substrate;
    const top = s.residue.slice(0, 3).map(([id, g]) => `${cropName(D, id).toLowerCase()} ${f0(g)} g`).join(", ");
    const eats = [...s.mushrooms.map(([id, g]) => `${cropName(D, id).toLowerCase()} ${f0(g)} g`), `the vitamin D shelf ${f0(s.vitaminD.substrateG)} g`].join(", ");
    L.push(`- The mushroom room's growing medium: the crops leave ${f0(s.residueG)} g of dry residue a day (${top}; only crops with a measured harvest index count), ${f0(s.usableG)} g usable at ${f0(100 * S.residue_usable_share)} percent; the mushrooms need ${f0(s.needG)} g of dry substrate a day (${eats}), so the residue covers **${s.coverage === Infinity ? "all of it" : f0(100 * s.coverage) + " percent"}**${a.vitaminDFallback ? " (too little for the vitamin D shelf: it falls back to the supplement, and no mushrooms are grown for food)" : a.mushroomRoom ? " (the linear program holds it at 100 percent or more)" : s.coverage < 1 ? " (too little: the vitamin D shelf falls back to the supplement)" : ""}.`);
    L.push(`- Preparing it: wetting to ${f0(100 * S.moisture)} percent moisture takes ${f2(s.waterL)} L of water a day (it returns to the air and the compost); pasteurising it with hot air takes ${f2(s.pasteuriseKwh)} kWh a day at ${f0(S.pasteurise_kj_per_kg_dry)} kJ a dry kg. The spent substrate, ${f0(s.spentDryG)} g dry a day, goes to the compost and back to the beds, carrying ${f0(s.spentPgYr[0])} to ${f0(s.spentPgYr[1])} g of phosphorus a year.`);
  }
  return L.join("\n");
}

function scaling(D, tierName, base) {
  const L = ["| Residents | Lit canopy | Per resident | Plots | Vitamin D shelves |", "|---|---|---|---|---|"];
  const set = base.chosen.set || Object.keys(base.chosen.area);
  for (const n of [1, 2, 4, 8]) {
    const a = n === 1 ? base : analyse(D, tierName, n, { onlyIds: set });
    if (a.infeasible) continue;
    const lit = Object.values(a.plotArea).reduce((s, v) => s + v, 0);
    const uv = D.garden.treatments.uv_flash, pl = D.plants[uv.crop], y = D.yields[uv.crop], w = D.windows[uv.crop] || { window: 0 };
    const perShelf = Math.floor(D.garden.layout.mushroom_rack_shelf_m2 / Number(pl.area_per_plant_m2) + 1e-9);
    const gDay = (perShelf * ((y.lo + y.hi) / 2) * 1000) / (Number(pl.growth_days) + w.window);
    L.push(`| ${n} | ${dual(lit)} | ${dual(lit / n)} | ${Object.values(a.plots).reduce((s, v) => s + v, 0)} | ${Math.ceil(((15 * n) / uv.vitamin_d_ug_per_100g) * 100 / gDay)} |`);
  }
  return L.join("\n");
}

function main() {
  const args = process.argv.slice(2);
  const D = load();
  const tierArg = args.includes("--tier") ? args[args.indexOf("--tier") + 1] : null;
  const residents = args.includes("--residents") ? Number(args[args.indexOf("--residents") + 1]) : 1;
  const tiers = tierArg ? [tierArg] : Object.keys(D.garden.tiers);
  if (!tierArg) tiers.push(...Object.keys(D.garden.variants || {}));
  const results = tiers.map((t) => analyse(D, t, residents));
  if (args.includes("--json")) {
    console.log(JSON.stringify(results.map((r) => ({ tier: r.tierName, plots: r.plots, lpArea: r.chosen && r.chosen.area, fullLp: r.full, byK: r.byK, supply: r.plotSupply })), null, 1));
    return;
  }
  const parts = [];
  for (const a of results) {
    const r = report(D, a);
    parts.push(r.text);
    parts.push("Loops:");
    parts.push("");
    parts.push(loops(D, a));
    parts.push("");
  }
  if (!tierArg && residents === 1) {
    parts.push("### Scaling, bare tier");
    parts.push("");
    parts.push(scaling(D, "bare", results.find((r) => r.tierName === "bare")));
    parts.push("");
  }
  const block = parts.join("\n").replace(/\n{3,}/g, "\n\n").trim();
  if (args.includes("--write") || args.includes("--check")) {
    const raw = fs.readFileSync(DOC, "utf8");
    const eol = raw.includes("\r\n") ? "\r\n" : "\n";
    const doc = raw.replace(/\r\n/g, "\n");
    const i = doc.indexOf(BEGIN), j = doc.indexOf(END);
    if (i < 0 || j < i) { console.error(`survival-garden: the markers are missing from ${DOC}`); process.exit(2); }
    const next = doc.slice(0, i + BEGIN.length) + "\n\n" + block + "\n\n" + doc.slice(j);
    if (args.includes("--check")) {
      if (next !== doc) { console.error("survival-garden: docs/design/survival-garden.md's generated block is stale; run node scripts/survival-garden.js --write"); process.exit(1); }
      console.log("survival-garden: the design doc matches the data");
      return;
    }
    fs.writeFileSync(DOC, next.replace(/\n/g, eol));
    console.log(`survival-garden: wrote the generated block into ${path.relative(ROOT, DOC)}`);
    return;
  }
  console.log(block);
}

if (require.main === module) main();
module.exports = { simplex, load, analyse, productivity, targets, residueDryG, substrateNeedG, vitaminDShelf, substrateBalance };
