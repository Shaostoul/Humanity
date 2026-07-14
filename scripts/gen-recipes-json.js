#!/usr/bin/env node
// Generate data/recipes.json from data/recipes.csv so the WEB crafting page can
// render a read-only recipe browser (sync-web only deploys *.json under data/,
// not the CSV). recipes.csv stays the single source of truth (native reads it);
// this is a generated artifact -- re-run after editing recipes.csv:
//   node scripts/gen-recipes-json.js
'use strict';
const fs = require('fs');
const path = require('path');

const CSV = path.join('data', 'recipes.csv');
const OUT = path.join('data', 'recipes.json');

const raw = fs.readFileSync(CSV, 'utf8');
const lines = raw.split(/\r?\n/).filter((l) => l.trim() && !l.trim().startsWith('#'));
const header = lines.shift().split(',').map((h) => h.trim());
const col = Object.fromEntries(header.map((h, i) => [h, i]));

// Prettify an item id like "iron_ore_0" -> "Iron Ore" for display.
function pretty(id) {
  return id
    .replace(/_\d+$/, '')
    .split('_')
    .filter(Boolean)
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ');
}

// Parse "iron_ore_0:2|coal_0:1" -> [{id,label,qty}, ...].
function parseItems(s) {
  if (!s) return [];
  return s.split('|').map((part) => {
    const [id, qty] = part.split(':');
    return { id, label: pretty(id), qty: Number(qty) || 1 };
  });
}

// Split a CSV row keeping everything after the 9th comma as the description
// (the last column may contain commas).
function splitRow(line) {
  const parts = line.split(',');
  if (parts.length <= header.length) return parts;
  const head = parts.slice(0, header.length - 1);
  head.push(parts.slice(header.length - 1).join(','));
  return head;
}

const recipes = lines.map((line) => {
  const f = splitRow(line);
  return {
    id: f[col.id],
    name: f[col.name],
    category: f[col.category] || 'misc',
    inputs: parseItems(f[col.inputs]),
    outputs: parseItems(f[col.outputs]),
    craft_time_sec: Number(f[col.craft_time_sec]) || 0,
    station: f[col.station_required] ? pretty(f[col.station_required]) : '',
    skill: f[col.skill_required] ? pretty(f[col.skill_required]) : '',
    skill_level: Number(f[col.skill_level]) || 1,
    description: (f[col.description] || '').trim(),
  };
}).filter((r) => r.id);

const categories = [...new Set(recipes.map((r) => r.category))].sort();

fs.writeFileSync(OUT, JSON.stringify({ count: recipes.length, categories, recipes }, null, 2) + '\n');
console.log(`Wrote ${OUT}: ${recipes.length} recipes, ${categories.length} categories`);
