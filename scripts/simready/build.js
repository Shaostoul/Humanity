#!/usr/bin/env node
// build.js - the SimReady conversion pipeline, one command (docs/design/simready-props.md, section 4).
//
//   just simready                       plan, extract, bake and pack every asset of the starter set
//   just simready --asset <folder>      the same for one asset (repeatable)
//   node scripts/simready/build.js <step> [--asset <folder>]...
//     plan     list the textures each asset's materials reference (<src>/<folder>/plan.json),
//              which fetch.js pass 2 reads
//     extract  units, bounds, materials and the neutral physics, from pxr (extract.json)
//     bake     simplify, unwrap and bake each asset in Blender (bake/ in the build folder)
//     pack     write the shipped GLB, LICENSE.txt, SOURCE.md and physics.ron (node, pure)
//     all      extract, bake and pack
//     preview  write named assets into the build folder's _preview/, no verdict needed (a look first)
//     check    rebuild into a scratch folder and compare every byte with what is committed
//
// Folders: the downloaded originals in HUMANITY_SIMREADY_SRC (default E:\HumanityOS\simready-src),
// intermediates in HUMANITY_SIMREADY_BUILD (default E:\HumanityOS\simready-build), wiped per asset
// at the start of each run (no incremental reuse: the no-caches rule), outputs in
// assets/models/simready/ (the home set) and E:\HumanityOS\simready-packs\ (the Medical Bay pack).
//
// Blender runs below normal priority on all but one core (the operator's share of his PC).

'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawn } = require('child_process');
const { findBlender, blenderVersion } = require('./find-blender');

const REPO = path.resolve(__dirname, '..', '..');
const LIST = path.join(__dirname, 'starter-set.json');
const SRC = process.env.HUMANITY_SIMREADY_SRC || 'E:\\HumanityOS\\simready-src';
const BUILD = process.env.HUMANITY_SIMREADY_BUILD || 'E:\\HumanityOS\\simready-build';

function parseArgs(argv) {
  const a = { step: argv[0], assets: [], rest: [] };
  for (let i = 1; i < argv.length; i++) {
    if (argv[i] === '--asset') a.assets.push(argv[++i]);
    else a.rest.push(argv[i]);
  }
  return a;
}

function loadList() {
  return JSON.parse(fs.readFileSync(LIST, 'utf8'));
}

function chosen(list, names) {
  const all = list.assets;
  if (!names.length) return all;
  const out = names.map(n => all.find(a => a.folder === n) || (() => { throw new Error(`${n} is not in the starter set`); })());
  return out;
}

/** Run Blender headless on a script, below normal priority, on all but one core. */
function runBlender(script, scriptArgs) {
  const exe = findBlender();
  const threads = Math.max(1, Math.min(os.cpus().length - 1, 11));
  const argv = ['--background', '--factory-startup', '--python-exit-code', '1', '--threads', String(threads), '--python', script, '--', ...scriptArgs];
  return new Promise((resolve, reject) => {
    const child = spawn(exe, argv, { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
    try { os.setPriority(child.pid, os.constants.priority.PRIORITY_BELOW_NORMAL); } catch (_) { /* best effort */ }
    let out = '';
    let err = '';
    child.stdout.on('data', d => { out += d; });
    child.stderr.on('data', d => { err += d; });
    child.on('close', code => {
      if (code === 0) resolve({ out, err });
      else reject(new Error(`Blender ${path.basename(script)} ${scriptArgs[0]} failed (${code}):\n${(err || out).split('\n').filter(l => !/^Warning: in _/.test(l)).slice(-25).join('\n')}`));
    });
  });
}

/** Expand a referenced `<UDIM>` texture into the tiles the pinned list holds. */
function expandUdim(asset, refs) {
  const out = [];
  for (const r of refs) {
    if (!r.includes('<UDIM>')) { out.push(r); continue; }
    const re = new RegExp('^' + r.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace('<UDIM>', '\\d{4}') + '$');
    const tiles = asset.files.filter(f => re.test(f.path)).map(f => f.path);
    if (!tiles.length) throw new Error(`${asset.folder}: ${r} names UDIM tiles the pinned list does not hold`);
    out.push(...tiles);
  }
  return [...new Set(out)].sort();
}

async function plan(assets) {
  for (const a of assets) {
    const dir = path.join(SRC, a.folder);
    const tmp = path.join(dir, 'plan.raw.json');
    await runBlender(path.join(__dirname, 'extract_usd.py'), [dir, tmp, '--plan']);
    const raw = JSON.parse(fs.readFileSync(tmp, 'utf8'));
    fs.unlinkSync(tmp);
    const textures = expandUdim(a, raw.textures);
    for (const t of textures) {
      if (!a.files.some(f => f.path === t)) throw new Error(`${a.folder}: its materials reference ${t}, which the pinned list does not hold`);
    }
    fs.writeFileSync(path.join(dir, 'plan.json'), JSON.stringify({ textures }, null, 1) + '\n');
    for (const o of raw.outside || []) {
      console.log(`${a.folder}: its materials also name ${o}, outside Textures/ and not in the download: the bake paints that surface plain grey`);
    }
    const listed = a.files.filter(f => f.kind === 'texture').length;
    console.log(`${a.folder}: ${textures.length} of ${listed} pinned textures referenced`);
  }
}

async function extract(assets) {
  for (const a of assets) {
    const out = path.join(BUILD, a.folder);
    fs.rmSync(out, { recursive: true, force: true });
    fs.mkdirSync(out, { recursive: true });
    await runBlender(path.join(__dirname, 'extract_usd.py'), [path.join(SRC, a.folder), path.join(out, 'extract.json')]);
    const x = JSON.parse(fs.readFileSync(path.join(out, 'extract.json'), 'utf8'));
    const size = x.bounds_max_m.map((v, i) => (v - x.bounds_min_m[i]).toFixed(3));
    console.log(`${a.folder}: ${x.render_meshes.length} meshes, ${x.triangles} triangles, ${size.join(' x ')} m, ${x.physics.bodies.length} bodies, ${x.physics.colliders.length} colliders`);
  }
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const list = loadList();
  const assets = chosen(list, args.assets);
  const steps = {
    plan: () => plan(assets),
    extract: () => extract(assets),
    bake: () => require('./bake-run').bake(assets, { SRC, BUILD, runBlender }),
    pack: () => require('./pack').packAll(assets, { SRC, BUILD, REPO, list, blender: blenderVersion(findBlender()) }),
    preview: () => require('./pack').preview(assets, { SRC, BUILD, REPO, list, blender: blenderVersion(findBlender()) }),
    all: async () => {
      await extract(assets);
      await require('./bake-run').bake(assets, { SRC, BUILD, runBlender });
      // The manifests list every asset, so the pack step always writes the whole set (from the
      // build folder's current bakes) even when only some assets were rebuilt.
      await require('./pack').packAll(list.assets, { SRC, BUILD, REPO, list, blender: blenderVersion(findBlender()) });
    },
    check: () => require('./pack').check(assets, { SRC, BUILD, REPO, list, blender: blenderVersion(findBlender()) }),
  };
  if (!steps[args.step]) {
    console.error('usage: node scripts/simready/build.js plan|extract|bake|pack|preview|all|check [--asset <folder>]...');
    process.exit(1);
  }
  await steps[args.step]();
}

main().catch(e => { console.error(e.message); process.exit(1); });
