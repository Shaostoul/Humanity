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

// Hand tools each recipe needs (data/crafting/tools.ron, 2026-09-26). The
// file's shape is fixed and simple, so it is read with two patterns; the
// resolution mirrors ToolRules::tools_for in src/systems/crafting/tools.rs:
// a per-recipe list overrides the rules, the first rule matching the station
// (and the category, when it names one) applies, a recipe made by hand
// matches no rule, and a recipe never needs a tool it makes.
const TOOLS_RON = fs.readFileSync(path.join('data', 'crafting', 'tools.ron'), 'utf8')
  .split(/\r?\n/).map((l) => l.replace(/\/\/.*$/, '')).join('\n');
const listOf = (s) => (s.match(/"([^"]+)"/g) || []).map((q) => q.slice(1, -1));
const toolRules = [...TOOLS_RON.matchAll(/\(station:\s*"([^"]*)",\s*category:\s*"([^"]*)",\s*tools:\s*\[([^\]]*)\]\)/g)]
  .map((m) => ({ station: m[1], category: m[2], tools: listOf(m[3]) }));
const recipesPart = TOOLS_RON.slice(TOOLS_RON.indexOf('recipes:'));
const toolOverrides = Object.fromEntries(
  [...recipesPart.matchAll(/"([^"]+)":\s*\[([^\]]*)\]/g)].map((m) => [m[1], listOf(m[2])]),
);
if (!toolRules.length) throw new Error('tools.ron: no rules parsed');
function toolsFor(id, station, category, outputs) {
  let list = toolOverrides[id];
  if (!list) {
    const rule = station && toolRules.find((r) => r.station === station && (!r.category || r.category === category));
    list = rule ? rule.tools : [];
  }
  return list.filter((t) => !outputs.some((o) => o.id === t)).map((t) => ({ id: t, label: pretty(t) }));
}

const recipes = lines.map((line) => {
  const f = splitRow(line);
  const outputs = parseItems(f[col.outputs]);
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
    tools: toolsFor(f[col.id], (f[col.station_required] || '').trim(), f[col.category] || '', outputs),
  };
}).filter((r) => r.id);

const categories = [...new Set(recipes.map((r) => r.category))].sort();

fs.writeFileSync(OUT, JSON.stringify({ count: recipes.length, categories, recipes }, null, 2) + '\n');
console.log(`Wrote ${OUT}: ${recipes.length} recipes, ${categories.length} categories`);
