#!/usr/bin/env node
/**
 * i18n-add.js: put words into data/i18n/en.json (the one source of the app's
 * and the website's English, src/gui/i18n.rs and web/shared/i18n.js), then
 * regenerate web/shared/i18n-en.js.
 *
 * Words go in as a JSON fragment shaped like en.json itself:
 *   { "calculator": { "title": "Calculator", "clear_hint": "Empties the display." } }
 * New keys are added at the end of their object; an existing key with the SAME
 * words is left alone; an existing key with DIFFERENT words is refused (two
 * pages would be fighting over one key) unless --replace says the change is
 * meant. Several people and agents add words at once in this repo, so the
 * read-merge-write holds a lock (a directory beside en.json) while it works.
 *
 * Usage:
 *   node scripts/i18n-add.js fragment.json [more.json ...] [--replace]
 *   node scripts/i18n-add.js --json '{"calculator":{"title":"Calculator"}}'
 */
'use strict';

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const EN_PATH = path.join(ROOT, 'data', 'i18n', 'en.json');
const LOCK = EN_PATH + '.lock';

function sleep(ms) {
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
}

function withLock(fn) {
  const start = Date.now();
  for (;;) {
    try {
      fs.mkdirSync(LOCK);
      break;
    } catch (e) {
      if (e.code !== 'EEXIST') throw e;
      // A lock older than 30 s is a crashed writer's: take it over.
      try {
        if (Date.now() - fs.statSync(LOCK).mtimeMs > 30000) { fs.rmdirSync(LOCK); continue; }
      } catch { /* gone already */ }
      if (Date.now() - start > 20000) throw new Error('en.json is locked by another writer (' + LOCK + ')');
      sleep(50);
    }
  }
  try { return fn(); } finally { try { fs.rmdirSync(LOCK); } catch { /* fine */ } }
}

/** Merge `frag` into `base` in place. Returns { added, same, conflicts }. */
function merge(base, frag, replace, prefix = '', out = { added: [], same: [], conflicts: [] }) {
  for (const [k, v] of Object.entries(frag)) {
    const key = prefix ? prefix + '.' + k : k;
    if (v && typeof v === 'object' && !Array.isArray(v)) {
      if (base[k] === undefined) base[k] = {};
      if (typeof base[k] !== 'object') { out.conflicts.push(key + ' (a word here, an object in the fragment)'); continue; }
      merge(base[k], v, replace, key, out);
    } else if (typeof v === 'string') {
      if (base[k] === undefined) { base[k] = v; out.added.push(key); }
      else if (base[k] === v) out.same.push(key);
      else if (typeof base[k] === 'object') out.conflicts.push(key + ' (an object in en.json, a word in the fragment)');
      else if (replace) { base[k] = v; out.added.push(key + ' (replaced)'); }
      else out.conflicts.push(key + ': en.json has ' + JSON.stringify(base[k]) + ', the fragment ' + JSON.stringify(v));
    } else {
      out.conflicts.push(key + ' (only words and objects belong in en.json)');
    }
  }
  return out;
}

function main() {
  const args = process.argv.slice(2);
  const replace = args.includes('--replace');
  const frags = [];
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--replace') continue;
    if (args[i] === '--json') { frags.push(JSON.parse(args[++i])); continue; }
    frags.push(JSON.parse(fs.readFileSync(args[i], 'utf8')));
  }
  if (!frags.length) {
    console.log('usage: node scripts/i18n-add.js fragment.json [...] [--replace] | --json \'{...}\'');
    process.exit(2);
  }
  const result = withLock(() => {
    const en = JSON.parse(fs.readFileSync(EN_PATH, 'utf8'));
    const all = { added: [], same: [], conflicts: [] };
    for (const f of frags) merge(en, f, replace, '', all);
    if (all.conflicts.length) return all;
    fs.writeFileSync(EN_PATH, JSON.stringify(en, null, 2) + '\n');
    return all;
  });
  if (result.conflicts.length) {
    console.error('NOT WRITTEN, these keys already hold other words:\n  ' + result.conflicts.join('\n  ') +
      '\nUse a new key, or --replace when the change is meant (then check every page using it).');
    process.exit(1);
  }
  const gen = require('./gen-i18n.js');
  fs.writeFileSync(gen.OUT_PATH, gen.build(fs.readFileSync(EN_PATH, 'utf8')));
  console.log('en.json: ' + result.added.length + ' added, ' + result.same.length + ' already there; web/shared/i18n-en.js regenerated');
}

if (require.main === module) main();
module.exports = { merge };
