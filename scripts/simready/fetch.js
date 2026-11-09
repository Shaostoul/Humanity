#!/usr/bin/env node
// fetch.js - download the NVIDIA SimReady starter set, exactly as pinned in
// scripts/simready/starter-set.json, and nothing else (docs/design/simready-props.md, section 3).
//
// The operator approved this download on 2026-11-08: the 712 files the list pins, into
// E:\HumanityOS\simready-src (never into git), stopping if any file's size or hash differs.
// So this script:
//   * fetches ONLY paths the pinned list names, one request at a time, with an honest user agent;
//   * checks every file's size and hash before keeping it: SHA-256 for Git LFS files (the hash
//     Hugging Face's tree API gave), the git blob SHA-1 for the few small files kept outside LFS
//     (the licence and packaging JSON); a mismatch deletes the partial file and STOPS the run,
//     because it means NVIDIA changed the dataset and the list needs a fresh yes;
//   * copies an entry marked `same_as` from the file already fetched (NVIDIA shares its tiling
//     textures between assets under the same bytes) and checks the copy's hash too;
//   * keeps a file already on disk only after re-checking its size and hash (this is the source
//     download, not a cache: a file that fails the check is fetched again).
//
// Two passes (the list is a ceiling, design section 3):
//   node scripts/simready/fetch.js --pass 1          metadata, USD layers, look images
//   node scripts/simready/fetch.js --pass 2          the textures plan.json says the materials use
//   node scripts/simready/fetch.js --pass 2 --all    every pinned texture
// Options: --asset <folder> (repeatable), --dest <dir> (default HUMANITY_SIMREADY_SRC or
// E:\HumanityOS\simready-src), --dry (list what would be fetched, fetch nothing).
//
// Downloaded files are untrusted data: each asset lands in its own folder under the destination,
// every path is checked to stay inside it, and nothing here executes or parses them.

'use strict';
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const LIST = path.join(__dirname, 'starter-set.json');
const UA = 'HumanityOS-asset-fetch/1.0 (fetching CC BY 4.0 assets the project credits; https://united-humanity.us)';

function parseArgs(argv) {
  const a = { pass: 0, assets: [], dest: process.env.HUMANITY_SIMREADY_SRC || 'E:\\HumanityOS\\simready-src', all: false, dry: false };
  for (let i = 0; i < argv.length; i++) {
    const k = argv[i];
    if (k === '--pass') a.pass = Number(argv[++i]);
    else if (k === '--asset') a.assets.push(argv[++i]);
    else if (k === '--dest') a.dest = argv[++i];
    else if (k === '--all') a.all = true;
    else if (k === '--dry') a.dry = true;
    else throw new Error(`unknown argument ${k}`);
  }
  if (a.pass !== 1 && a.pass !== 2) throw new Error('say --pass 1 or --pass 2');
  return a;
}

/** The hash of a file's bytes, in the form its list entry pins. */
function hashOf(entry, bytes) {
  if (entry.sha256) return { want: entry.sha256, got: crypto.createHash('sha256').update(bytes).digest('hex') };
  const h = crypto.createHash('sha1');
  h.update(`blob ${bytes.length}\0`);
  h.update(bytes);
  return { want: entry.git_sha1, got: h.digest('hex') };
}

/** Whether `bytes` are exactly the pinned file. */
function matches(entry, bytes) {
  if (bytes.length !== entry.size) return false;
  const { want, got } = hashOf(entry, bytes);
  return want === got;
}

/** The local path of one entry, refusing anything that would leave the asset's folder. */
function localPath(dest, folder, rel) {
  if (!/^[A-Za-z0-9_]+$/.test(folder)) throw new Error(`refusing folder name ${folder}`);
  if (rel.includes('..') || path.isAbsolute(rel) || /[:\\]/.test(rel)) throw new Error(`refusing path ${folder}/${rel}`);
  const root = path.resolve(dest, folder);
  const p = path.resolve(root, ...rel.split('/'));
  if (!p.startsWith(root + path.sep)) throw new Error(`refusing path ${folder}/${rel}`);
  return p;
}

const sleep = ms => new Promise(r => setTimeout(r, ms));

async function download(url) {
  for (let attempt = 0; attempt < 5; attempt++) {
    let res;
    try {
      res = await fetch(url, { headers: { 'User-Agent': UA }, redirect: 'follow' });
    } catch (e) {
      await sleep(5000 * (attempt + 1));
      continue;
    }
    if (res.status === 429 || res.status >= 500) { await sleep(15000 * (attempt + 1)); continue; }
    if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
    return Buffer.from(await res.arrayBuffer());
  }
  throw new Error(`${url}: gave up after five tries`);
}

function wanted(entry, pass, planned, all) {
  if (pass === 1) return entry.kind !== 'texture';
  if (entry.kind !== 'texture') return false;
  return all || (planned && planned.has(entry.path));
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const list = JSON.parse(fs.readFileSync(LIST, 'utf8'));
  const assets = list.assets.filter(a => args.assets.length === 0 || args.assets.includes(a.folder));
  if (args.assets.length && assets.length !== args.assets.length) throw new Error('an --asset is not in the pinned list');
  // Where each content hash lives once fetched, for the `same_as` copies.
  const byHash = new Map();
  let fetched = 0, copied = 0, kept = 0, bytes = 0;
  for (const a of assets) {
    let planned = null;
    if (args.pass === 2 && !args.all) {
      const planPath = path.join(args.dest, a.folder, 'plan.json');
      if (!fs.existsSync(planPath)) throw new Error(`${a.folder}: no plan.json; run the plan step first (or pass --all)`);
      const plan = JSON.parse(fs.readFileSync(planPath, 'utf8'));
      planned = new Set(plan.textures);
      for (const t of plan.textures) {
        if (!a.files.some(f => f.path === t)) throw new Error(`${a.folder}: the plan names ${t}, which the pinned list does not; stopping`);
      }
    }
    for (const e of a.files) {
      const key = e.sha256 || e.git_sha1;
      const p = localPath(args.dest, a.folder, e.path);
      if (!wanted(e, args.pass, planned, args.all)) {
        if (fs.existsSync(p)) byHash.set(key, p);
        continue;
      }
      if (fs.existsSync(p) && matches(e, fs.readFileSync(p))) { byHash.set(key, p); kept++; continue; }
      if (args.dry) { console.log(`would fetch ${a.folder}/${e.path} ${e.size}`); continue; }
      fs.mkdirSync(path.dirname(p), { recursive: true });
      let buf;
      if (e.same_as && byHash.has(key)) {
        buf = fs.readFileSync(byHash.get(key));
        copied++;
      } else {
        const src = e.same_as || `${a.folder}/${e.path}`;
        buf = await download(list.url_base + src.split('/').map(encodeURIComponent).join('/'));
        fetched++;
        bytes += buf.length;
        await sleep(150);
      }
      if (!matches(e, buf)) {
        const { want, got } = hashOf(e, buf);
        console.error(`STOP: ${a.folder}/${e.path}: pinned ${e.size} bytes ${want}, got ${buf.length} bytes ${got}.`);
        console.error('The dataset changed since the list was pinned (2026-11-08). Nothing more is fetched; the list needs a fresh yes.');
        process.exit(2);
      }
      fs.writeFileSync(p + '.part', buf);
      fs.renameSync(p + '.part', p);
      byHash.set(key, p);
    }
    console.log(`${a.folder}: done`);
  }
  console.log(`pass ${args.pass}: fetched ${fetched} files (${(bytes / 1048576).toFixed(1)} MiB), copied ${copied}, already present and checked ${kept}`);
}

main().catch(e => { console.error(e.message); process.exit(1); });
