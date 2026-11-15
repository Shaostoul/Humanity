#!/usr/bin/env node
// The model inspector on the command line (docs/design/model-inspector.md, rung 1): the same report
// the Files page's Inspect mode shows, as JSON, for agents and scripts. It reads the file with
// web/shared/model-inspect.js (the website's twin of src/gui/model_view/inspect.rs), against
// data/models/inspect_conventions.json, the physics.ron beside the file, and every catalog row in
// data/machines/*.ron that draws it; each finding's words are en.json's
// model_inspect.finding.<code>.
//
//   node scripts/inspect-model.js <model file>   one model's report
//   node scripts/inspect-model.js --catalog      every catalog row with a model: its box against
//                                                its fitted model (turned by the row's model_yaw),
//                                                the known exceptions with their reasons
//                                                (data/models/catalog_fit_exceptions.json), and
//                                                each row whose model is in a download pack not
//                                                installed here, with how a person gets it
//
// It never changes a file. Exit 0 with the report; 1 when the file cannot be read at all, or (with
// --catalog) when a row's box does not fit its model and is not a known exception, the same rule
// as the desktop's every_catalog_box_fits_its_fitted_model.
'use strict';

const fs = require('fs');
const path = require('path');
const MI = require('../web/shared/model-inspect.js');
const { readMachineHome } = require('./lib/home-furniture.js');

const ROOT = path.resolve(__dirname, '..');
const DATA = path.join(ROOT, 'data');

function conventions() {
  try {
    return { conventions: JSON.parse(fs.readFileSync(path.join(DATA, 'models', 'inspect_conventions.json'), 'utf8')) };
  } catch (e) {
    return { conventions: {}, conventionsError: e.message };
  }
}

const WORDS = (() => {
  try {
    return JSON.parse(fs.readFileSync(path.join(DATA, 'i18n', 'en.json'), 'utf8')).model_inspect.finding;
  } catch {
    return {};
  }
})();

/** A .gltf's outside file: a plain relative path in its own folder or below, never above it. */
function outsideReader(dir) {
  return (uri) => {
    let rel;
    try { rel = decodeURIComponent(uri || ''); } catch { return null; }
    if (!rel || rel.includes(':') || rel.startsWith('/') || rel.startsWith('\\')) return null;
    if (rel.split(/[\\/]/).some((p) => p === '..')) return null;
    const full = path.resolve(dir, rel);
    if (!full.startsWith(path.resolve(dir) + path.sep)) return null;
    try { return new Uint8Array(fs.readFileSync(full)); } catch { return null; }
  };
}

/** Every catalog row in data/machines/*.ron that names a model: [{file, id, label, shape, size, model, model_yaw}]. */
function catalogRows() {
  const out = [];
  const dir = path.join(DATA, 'machines');
  for (const f of fs.readdirSync(dir).filter((n) => n.endsWith('.ron')).sort()) {
    const home = readMachineHome(fs.readFileSync(path.join(dir, f), 'utf8'));
    for (const [id, def] of Object.entries(home.catalog)) {
      if (!def.model) continue;
      // A row repeated in several files with the same size and model is one row.
      const yaw = def.model_yaw || 0;
      if (out.some((r) => r.id === id && r.model === def.model && String(r.size) === String(def.size) && r.model_yaw === yaw)) continue;
      out.push({ file: f, id, label: def.label, shape: def.shape, size: def.size, model: def.model, model_yaw: yaw });
    }
  }
  return out;
}

/** The model file a catalog row names, on this computer: the data folder first, then the app's. */
function modelPath(model) {
  return [path.join(DATA, model), path.join(ROOT, model)].find((p) => fs.existsSync(p)) || null;
}

function inspectFile(file, catalog) {
  const bytes = fs.readFileSync(file);
  const physicsFile = path.join(path.dirname(file), 'physics.ron');
  const physics = fs.existsSync(physicsFile) ? fs.readFileSync(physicsFile, 'utf8') : undefined;
  return MI.inspect(path.basename(file), bytes, Object.assign(conventions(), { physics, catalog, readOutside: outsideReader(path.dirname(file)) }));
}

/** The report as JSON: the inspection with each finding's words, and lines counted not listed. */
function report(file, insp) {
  const v = (a) => a.map((x) => Number(x.toFixed(4)));
  return {
    file: path.relative(ROOT, file).replace(/\\/g, '/'),
    size_m: v(insp.max.map((x, k) => x - insp.min[k])),
    bounds: { min: v(insp.min), max: v(insp.max) },
    parts: insp.parts,
    materials: insp.materials,
    nodes: insp.nodes.map((n) => ({
      name: n.name,
      depth: n.depth,
      kind: n.kind,
      convention: n.convention,
      position: v([n.world[12], n.world[13], n.world[14]]),
      axes: { x: v(n.world.slice(0, 3)), y: v(n.world.slice(4, 7)), z: v(n.world.slice(8, 11)) },
    })),
    textures: insp.textures,
    clips: insp.clips,
    skins: insp.skins,
    colliders: insp.colliders.map((w) => ({ label: w.label, edges: w.edges.length, min: v(w.min), max: v(w.max) })),
    catalog: insp.catalog.map((f) => ({ file: f.row.file, id: f.row.id, size: f.row.size, model_yaw: f.row.model_yaw || 0, scale: Number(f.scale.toFixed(4)), fitted_m: v(f.fitted), box_m: v(f.box), verdict: f.verdict })),
    findings: insp.findings.map((f) => ({ level: f.level, code: f.code, words: MI.wordsOf(f, WORDS), vars: f.vars })),
  };
}

/** The known exceptions (data/models/catalog_fit_exceptions.json): [{file, id, reason}]. */
function fitExceptions() {
  try {
    return JSON.parse(fs.readFileSync(path.join(DATA, 'models', 'catalog_fit_exceptions.json'), 'utf8')).exceptions || [];
  } catch {
    return [];
  }
}

/** The download packs (data/models/packs.ron, src/model_packs): [{id, words, file, bytes}]. */
function modelPacks() {
  let text;
  try { text = fs.readFileSync(path.join(DATA, 'models', 'packs.ron'), 'utf8'); } catch { return []; }
  const list = text.slice(Math.max(0, text.indexOf('packs:')));
  const out = [];
  const re = /\(\s*id:\s*"([^"]+)"[\s\S]*?words:\s*"([^"]+)"[\s\S]*?file:\s*"([^"]+)"[\s\S]*?bytes:\s*(\d+)/g;
  let m;
  while ((m = re.exec(list))) out.push({ id: m[1], words: m[2], file: m[3], bytes: Number(m[4]) });
  return out;
}

/** en.json's words at a dotted key, or the key itself. */
function wordsAt(key) {
  try {
    return key.split('.').reduce((o, k) => o[k], JSON.parse(fs.readFileSync(path.join(DATA, 'i18n', 'en.json'), 'utf8')));
  } catch {
    return key;
  }
}

function catalogReport() {
  const rows = catalogRows();
  const known = fitExceptions();
  const packs = modelPacks();
  const mismatches = [];
  const exceptions = [];
  const missing = [];
  let checked = 0;
  for (const row of rows) {
    const file = modelPath(row.model);
    if (!file) {
      // A row whose model is in a download pack not installed here: named with its pack and how a
      // person gets it (Settings > Graphics > Furniture models), not judged.
      const pack = packs.find((p) => row.model.startsWith(`models/packs/${p.id}/`));
      const vars = pack ? { row: row.id, file: row.file, pack: wordsAt(pack.words), mb: (pack.bytes / 1e6).toFixed(1) } : { row: row.id, file: row.file, model: row.model };
      const key = pack ? 'catalog_in_pack' : 'catalog_model_absent';
      missing.push(Object.assign({ file: row.file, id: row.id, model: row.model }, pack ? { pack: pack.id, download: pack.file, bytes: pack.bytes } : {}, { words: MI.wordsOf({ code: key, vars }, WORDS) }));
      continue;
    }
    let insp;
    try { insp = inspectFile(file, [row]); } catch { continue; }
    const f = insp.catalog[0];
    if (!f) continue;
    checked++;
    if (f.verdict !== 'fits') {
      const finding = insp.findings.find((x) => x.code.startsWith('catalog_') && x.code !== 'catalog_fits');
      const entry = { file: row.file, id: row.id, model: row.model, model_yaw: row.model_yaw || 0, box_m: f.box.map((x) => Number(x.toFixed(3))), height_m: row.size[1], fitted_m: f.fitted.map((x) => Number(x.toFixed(3))), verdict: f.verdict, across: f.across, words: finding ? MI.wordsOf(finding, WORDS) : '' };
      const ex = known.find((e) => e.file === row.file && e.id === row.id);
      if (ex) exceptions.push(Object.assign(entry, { reason: ex.reason }));
      else mismatches.push(entry);
    }
  }
  return { checked, tolerance: (conventions().conventions.tolerance || {}), mismatches, exceptions, missing };
}

/** The catalog rows that draw the file at `file`. */
function rowsFor(file) {
  const real = path.resolve(file);
  const keys = [path.relative(ROOT, real), path.relative(DATA, real)].map((k) => k.replace(/\\/g, '/'));
  return catalogRows().filter((r) => keys.includes(r.model));
}

if (require.main === module) {
  const args = process.argv.slice(2);
  if (args[0] === '--catalog') {
    const r = catalogReport();
    process.stdout.write(JSON.stringify(r, null, 2) + '\n');
    if (r.mismatches.length) process.exitCode = 1;
  } else if (args[0] && !args[0].startsWith('--')) {
    const file = path.resolve(args[0]);
    let insp;
    try {
      insp = inspectFile(file, rowsFor(file));
    } catch (e) {
      console.error(`inspect-model: ${args[0]} could not be read: ${e.message}`);
      process.exit(1);
    }
    process.stdout.write(JSON.stringify(report(file, insp), null, 2) + '\n');
  } else {
    console.error('usage: node scripts/inspect-model.js <model file> | --catalog');
    process.exit(2);
  }
}

module.exports = { inspectFile, report, catalogReport, catalogRows, rowsFor, outsideReader, fitExceptions, modelPacks };
