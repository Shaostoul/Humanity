// bake-run.js - the node half of the bake step: pick each asset's size class from its measured
// bounds, write the bake's settings, run bake_lods.py in Blender, and check what came back
// (docs/design/simready-props.md, sections 4 and 9).

'use strict';
const fs = require('fs');
const os = require('os');
const path = require('path');
const { applyRepaints } = require('./repaint');

/** Size classes by the model's largest dimension (design section 9). */
const CLASSES = [
  { name: 'small', under_m: 0.4, lod1_triangles: 3000, albedo_px: 512, pbr_px: 512 },
  { name: 'medium', under_m: 1.2, lod1_triangles: 10000, albedo_px: 1024, pbr_px: 512 },
  { name: 'large', under_m: Infinity, lod1_triangles: 25000, albedo_px: 1024, pbr_px: 1024 },
];

function sizeClass(extract) {
  const dims = extract.bounds_max_m.map((v, i) => v - extract.bounds_min_m[i]);
  const largest = Math.max(...dims);
  return { ...CLASSES.find(c => largest < c.under_m), largest_m: largest };
}

function bakeConfig(extract) {
  const cls = sizeClass(extract);
  return {
    size_class: cls.name,
    largest_m: cls.largest_m,
    lod1_triangles: cls.lod1_triangles,
    lod2_ratio: 0.25,
    albedo_px: cls.albedo_px,
    pbr_px: cls.pbr_px,
    meters_per_unit: extract.meters_per_unit,
    samples: 4,
    margin_px: 8,
    island_margin: 0.004,
    cage_fraction: 0.01,
    weld_m: 0.00005,
    hull_grid_m: Math.max(0.002, cls.largest_m / 64),
    threads: Math.max(1, Math.min(os.cpus().length - 1, 11)),
  };
}

async function bake(assets, { SRC, BUILD, runBlender }) {
  for (const a of assets) {
    const dir = path.join(BUILD, a.folder);
    const xpath = path.join(dir, 'extract.json');
    if (!fs.existsSync(xpath)) throw new Error(`${a.folder}: no extract.json; run the extract step first`);
    const extract = JSON.parse(fs.readFileSync(xpath, 'utf8'));
    fs.rmSync(path.join(dir, 'bake'), { recursive: true, force: true });
    const cfg = bakeConfig(extract);
    // The brand check's repaints: painted copies the bake uses in place of the originals.
    cfg.texture_overrides = applyRepaints(a, path.join(SRC, a.folder), path.join(dir, 'repaint'));
    const cfgPath = path.join(dir, 'bake-config.json');
    fs.writeFileSync(cfgPath, JSON.stringify(cfg, null, 1) + '\n');
    const t0 = Date.now();
    await runBlender(path.join(__dirname, 'bake_lods.py'), [path.join(SRC, a.folder), dir, cfgPath]);
    const b = JSON.parse(fs.readFileSync(path.join(dir, 'bake', 'bake.json'), 'utf8'));
    console.log(`${a.folder}: ${cfg.size_class}, ${b.source_triangles} -> ${b.meshes.lod1.triangles} / ${b.meshes.lod2.triangles} triangles, ${cfg.albedo_px}/${cfg.pbr_px} px, ${b.hulls.length} hulls, ${((Date.now() - t0) / 1000).toFixed(0)} s`);
  }
}

module.exports = { bake, bakeConfig, sizeClass, CLASSES };
