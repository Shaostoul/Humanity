#!/usr/bin/env node
// gen-harvest-items.js (v0.749, closure ladder rung 6)
//
// Closes the harvest-item coverage gap: 132 plants, ~19 produce items, and
// harvest_item_for() only tried vegetable_/fruit_/grain_ prefixes - so herbs,
// legumes, mushrooms, fiber crops, and the apothecary garden harvested to a
// warning log. This script:
//   1. Adds a harvest_item COLUMN to data/plants.csv (explicit per row):
//      the existing prefix-resolved item where one exists, else a newly
//      generated id.
//   2. Appends the missing produce rows to data/items.csv (category food,
//      subcategory by plant type; herbs dry + light, legumes/mushrooms solid).
// Idempotent: reruns resolve everything and generate nothing.

const fs = require('fs');

const plantsPath = 'data/plants.csv';
const itemsPath = 'data/items.csv';

const itemIds = new Set(
  fs.readFileSync(itemsPath, 'utf8').split(/\r?\n/)
    .filter(l => l && !l.startsWith('#') && !l.startsWith('id,'))
    .map(l => l.split(',')[0])
);

const lines = fs.readFileSync(plantsPath, 'utf8').split(/\r?\n/);
const out = [];
const newItems = [];
let headerDone = false;

// Prefix per plant type for GENERATED ids (existing prefix search still wins).
const typePrefix = {
  vegetable: 'vegetable', fruit: 'fruit', grain: 'grain',
  legume: 'legume', herb: 'herb', fiber: 'fiber',
};

for (const line of lines) {
  if (line.startsWith('#') || line.trim() === '') { out.push(line); continue; }
  if (!headerDone && line.startsWith('id,')) {
    headerDone = true;
    if (line.includes('harvest_item')) { console.log('harvest_item column already present'); }
    out.push(line.includes('harvest_item') ? line : line + ',harvest_item');
    continue;
  }
  const cols = line.split(',');
  const id = cols[0];
  const name = cols[1];
  const type = cols[3];
  // Already has the column? (idempotent rerun)
  const headerCols = out.find(l => l.startsWith('id,')).split(',').length;
  if (cols.length >= headerCols) { out.push(line); continue; }

  // Resolve like harvest_item_for: existing prefix items win.
  let harvest = '';
  for (const p of ['vegetable', 'fruit', 'grain']) {
    const cand = `${p}_${id}_0`;
    if (itemIds.has(cand)) { harvest = cand; break; }
  }
  if (!harvest) {
    const prefix = typePrefix[type] || 'produce';
    harvest = `${prefix}_${id}_0`;
    if (!itemIds.has(harvest)) {
      // Weight/volume/class by type: herbs are light + dry; the rest solid.
      const herbish = type === 'herb';
      const fiberish = type === 'fiber';
      const weight = herbish ? 0.05 : 0.3;
      const vol = herbish ? 0.15 : 0.45;
      const cls = herbish ? 'dry_goods' : 'solid';
      const cat = fiberish ? 'material' : 'food';
      const desc = fiberish
        ? `Harvested ${name.toLowerCase()} fiber bundle`
        : `Fresh harvested ${name.toLowerCase()}`;
      newItems.push(`${harvest},${name},${cat},${type},plant_fiber,${weight},20,0,${desc},${cls},${vol}`);
      itemIds.add(harvest);
    }
  }
  out.push(line + ',' + harvest);
}

fs.writeFileSync(plantsPath, out.join('\n'));
if (newItems.length) {
  fs.appendFileSync(itemsPath, newItems.join('\n') + '\n');
}
console.log(`plants.csv: harvest_item column written; items.csv: +${newItems.length} produce rows`);
