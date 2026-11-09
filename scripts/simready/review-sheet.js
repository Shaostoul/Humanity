#!/usr/bin/env node
// review-sheet.js - the pictures the brand and emblem check looks at (docs/design/simready-props.md,
// section 8), before anything is converted.
//
// For each asset of the starter set: its look image, and every colour texture its materials
// reference (base colour and decal maps; normal and ORM maps carry no print), scaled to at most
// 1024 px with ffmpeg into <build>/_review/<folder>/, plus a full-size copy of every texture made for
// that one object (not one of NVIDIA's shared tiling materials) and every decal, where small
// printed text would be lost at 1024 px. Each sheet is then looked at, by the agent and offered to
// the operator, and the verdict recorded in starter-set.json as `brand_check`:
//   { "date": "YYYY-MM-DD", "by": "...", "verdict": "clear" | "repaint" | "drop", "notes": "..." }
// pack.js refuses an asset with no verdict, and skips one marked "drop".
//
//   node scripts/simready/review-sheet.js [--asset <folder>]...
//
// ffmpeg is the instrument pipeline's dependency already (docs/ai/release.md); it runs below normal
// priority. The textures are untrusted data: ffmpeg only decodes them, into a separate folder.

'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const { spawnSync } = require('child_process');

const LIST = path.join(__dirname, 'starter-set.json');
const SRC = process.env.HUMANITY_SIMREADY_SRC || 'E:\\HumanityOS\\simready-src';
const BUILD = process.env.HUMANITY_SIMREADY_BUILD || 'E:\\HumanityOS\\simready-build';

const isColour = p => /(_alb|albedo|_a\.png$|decal|_color|_basecolor|_diff)/i.test(path.basename(p)) && !/normal|_nor|_orm|rough/i.test(path.basename(p));
const isShared = p => /_tile_|^T_(Plastic|Metal|Rubber|Steel|Aluminium|Leather|Polyurethane|Wood)_/i.test(path.basename(p));

function ffmpeg(args) {
  const r = spawnSync('ffmpeg', ['-hide_banner', '-loglevel', 'error', '-y', ...args], { windowsHide: true, encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`ffmpeg ${args.join(' ')}: ${r.stderr}`);
}

function main() {
  const names = [];
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i++) if (argv[i] === '--asset') names.push(argv[++i]);
  const list = JSON.parse(fs.readFileSync(LIST, 'utf8'));
  try { os.setPriority(os.constants.priority.PRIORITY_BELOW_NORMAL); } catch (_) { /* best effort */ }
  for (const a of list.assets) {
    if (names.length && !names.includes(a.folder)) continue;
    const src = path.join(SRC, a.folder);
    const out = path.join(BUILD, '_review', a.folder);
    fs.rmSync(out, { recursive: true, force: true });
    fs.mkdirSync(out, { recursive: true });
    const plan = JSON.parse(fs.readFileSync(path.join(src, 'plan.json'), 'utf8'));
    const look = a.files.find(f => f.kind === 'look');
    const shots = [];
    if (look) shots.push(look.path);
    shots.push(...plan.textures.filter(isColour));
    for (const rel of shots) {
      const inPath = path.join(src, ...rel.split('/'));
      const base = path.basename(rel).replace(/\.[^.]+$/, '');
      ffmpeg(['-i', inPath, '-vf', "scale='min(1024,iw)':-2", '-q:v', '3', path.join(out, base + '.jpg')]);
      if (rel !== (look && look.path) && (!isShared(rel) || /decal/i.test(rel))) {
        ffmpeg(['-i', inPath, '-q:v', '2', path.join(out, base + '.full.jpg')]);
      }
    }
    console.log(`${a.folder}: ${shots.length} pictures -> ${out}`);
  }
}

main();
