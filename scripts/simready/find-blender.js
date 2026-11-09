// find-blender.js - where Blender is, and which version (docs/design/simready-props.md, section 4).
//
// Blender is the pipeline's USD reader (its bundled `pxr`) and baker. It is a build tool only:
// nothing of it ships. Looked for, in order: HUMANITY_BLENDER; the Steam library folders named in
// Steam's libraryfolders.vdf (the operator's copy is the Steam one, on D:, while the .blend file
// association still names an old E: path); the usual install folders. Steam updates Blender on its
// own, so the build records the version it ran with and `--check` says so when it changed.

'use strict';
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

function steamLibraries() {
  const out = [];
  for (const steam of ['C:\\Program Files (x86)\\Steam', 'C:\\Program Files\\Steam']) {
    const vdf = path.join(steam, 'steamapps', 'libraryfolders.vdf');
    if (!fs.existsSync(vdf)) continue;
    const text = fs.readFileSync(vdf, 'utf8');
    for (const m of text.matchAll(/"path"\s+"([^"]+)"/g)) out.push(m[1].replace(/\\\\/g, '\\'));
  }
  for (const d of ['C', 'D', 'E', 'F']) out.push(`${d}:\\Steam`, `${d}:\\SteamLibrary`);
  return out;
}

/** The Blender executable, or throws with what was tried. */
function findBlender() {
  const tried = [];
  const env = process.env.HUMANITY_BLENDER;
  if (env) {
    if (fs.existsSync(env)) return env;
    throw new Error(`HUMANITY_BLENDER names ${env}, which does not exist`);
  }
  const candidates = steamLibraries().map(l => path.join(l, 'steamapps', 'common', 'Blender', 'blender.exe'));
  for (const base of ['C:\\Program Files\\Blender Foundation']) {
    if (!fs.existsSync(base)) continue;
    for (const d of fs.readdirSync(base).sort().reverse()) candidates.push(path.join(base, d, 'blender.exe'));
  }
  for (const c of candidates) {
    tried.push(c);
    if (fs.existsSync(c)) return c;
  }
  throw new Error(`Blender not found (set HUMANITY_BLENDER). Tried:\n  ${tried.join('\n  ')}`);
}

/** "Blender 5.2.3 LTS" -> "5.2.3". */
function blenderVersion(exe) {
  const out = execFileSync(exe, ['--version'], { encoding: 'utf8', windowsHide: true });
  const m = out.match(/Blender\s+(\d+\.\d+\.\d+)/);
  if (!m) throw new Error(`could not read Blender's version from: ${out.slice(0, 200)}`);
  return m[1];
}

module.exports = { findBlender, blenderVersion };
