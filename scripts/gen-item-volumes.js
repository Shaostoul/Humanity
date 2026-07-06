#!/usr/bin/env node
// Generate the `volume_l` column for data/items.csv (material-storage Stage A,
// v0.726): effective storage volume per unit, derived from the item's mass and
// its base material's density with a per-category PACKING fraction (how much
// of an item's storage envelope is actually material — clothing is mostly air
// and folds, an ingot is nearly solid).
//
//   volume_l = weight_kg / (density_kg_m3 * packing) * 1000
//
// Idempotent: skips rows that already have a volume_l column value; safe to
// re-run after adding items (only fills the missing ones). Aborts rather than
// guessing if a data row's column count is unexpected.
//
// Run: node scripts/gen-item-volumes.js
//
// After ADDING a density to materials.csv for a material that previously fell
// back, recompute just that material's rows (hand-tuned rows for OTHER
// materials stay untouched):
//   RECOMPUTE_MATS=carbon,stone node scripts/gen-item-volumes.js

const fs = require('fs');

const ITEMS = 'data/items.csv';
const MATERIALS = 'data/materials.csv';

// Packing fraction by items.csv `category` (fraction of the storage envelope
// that is solid material). Tuned for sane liters: t-shirt ~2.6 L, steel ingot
// ~1.5 L, plank bundle ~3 L. Anything unlisted uses DEFAULT_PACKING.
const PACKING = {
  clothing: 0.05,   // folded fabric is mostly air
  tool: 0.35,       // handles, heads, dead space around shapes
  tools: 0.35,
  weapon: 0.35,
  material: 0.6,    // ingots, sheets, dense stock
  resource: 0.5,    // ores, raw lumps
  food: 0.45,       // produce, packaged goods
  seed: 0.55,
  medical: 0.3,     // kits with padding
  electronics: 0.3, // enclosures with boards inside
  furniture: 0.12,  // bulky frames, mostly enclosed air
  container: 0.15,  // empty vessels store as their shell
  vehicle: 0.25,    // kit crates / assembled hulks (rarely inventoried)
  book: 0.8,
  misc: 0.4,
};
const DEFAULT_PACKING = 0.4;
const FALLBACK_DENSITY = 1000.0; // unknown material: water-like

function parseCsv(path) {
  return fs.readFileSync(path, 'utf8').split('\n');
}

// Build material id -> density map (header-mapped like the game loader).
const matLines = parseCsv(MATERIALS).filter(l => l.trim() && !l.trim().startsWith('#'));
const matHeader = matLines[0].split(',');
const dIdx = matHeader.indexOf('density_kg_m3');
const idIdx = matHeader.indexOf('id');
const density = new Map();
for (const line of matLines.slice(1)) {
  const cols = line.split(',');
  const d = parseFloat(cols[dIdx]);
  if (cols[idIdx] && Number.isFinite(d) && d > 0) density.set(cols[idIdx].trim(), d);
}
console.log(`materials: ${density.size} densities loaded`);

const lines = fs.readFileSync(ITEMS, 'utf8').split('\n');
const out = [];
let headerCols = null;
let volIdx = -1;
let wIdx = -1, mIdx = -1, cIdx = -1;
let filled = 0, kept = 0;
const missingMats = new Set();

for (const line of lines) {
  const t = line.trim();
  if (!t || t.startsWith('#')) { out.push(line); continue; }
  if (!headerCols) {
    headerCols = t.split(',');
    wIdx = headerCols.indexOf('weight_kg');
    mIdx = headerCols.indexOf('base_material');
    cIdx = headerCols.indexOf('category');
    volIdx = headerCols.indexOf('volume_l');
    if (wIdx < 0 || mIdx < 0 || cIdx < 0) { console.error('header missing expected columns'); process.exit(1); }
    if (volIdx < 0) { out.push(t + ',volume_l'); volIdx = headerCols.length; }
    else out.push(line);
    continue;
  }
  const cols = t.split(',');
  if (cols.length !== headerCols.length && cols.length !== volIdx + 1) {
    console.error(`ABORT: row has ${cols.length} cols (expected ${headerCols.length}): ${t.slice(0, 80)}`);
    process.exit(1);
  }
  const recomputeMats = new Set((process.env.RECOMPUTE_MATS || '').split(',').map(s => s.trim()).filter(Boolean));
  const rowMat = (cols[mIdx] || '').trim();
  const forceRecompute = recomputeMats.has(rowMat);
  if (cols.length === volIdx + 1 && cols[volIdx] !== '' && !forceRecompute) { out.push(line); kept++; continue; }
  const w = parseFloat(cols[wIdx]) || 0;
  const mat = (cols[mIdx] || '').trim();
  const cat = (cols[cIdx] || '').trim();
  const d = density.get(mat) || FALLBACK_DENSITY;
  if (!density.has(mat) && mat) missingMats.add(mat);
  const packing = PACKING[cat] !== undefined ? PACKING[cat] : DEFAULT_PACKING;
  let vol = (w / (d * packing)) * 1000.0;
  if (!Number.isFinite(vol) || vol <= 0) vol = 0.1;
  // Round to sensible precision: 2 decimals under 10 L, 1 above.
  const volStr = vol < 10 ? vol.toFixed(2) : vol.toFixed(1);
  out.push(cols.slice(0, volIdx).join(',') + ',' + volStr);
  filled++;
}

fs.writeFileSync(ITEMS, out.join('\n'));
console.log(`volume_l: filled ${filled}, kept ${kept} existing`);
if (missingMats.size) console.log('materials with NO density (fallback 1000):', [...missingMats].sort().join(', '));
