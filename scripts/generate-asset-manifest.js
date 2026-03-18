#!/usr/bin/env node
/**
 * generate-asset-manifest.js
 * Scans the repo for all image assets and outputs a JSON manifest.
 * Usage: node scripts/generate-asset-manifest.js > asset-manifest.json
 *
 * Categories:
 *   icons-png  — assets/ui/icons/*.png
 *   icons-svg  — assets/ui/icons/*.svg
 *   concepts   — assets/concepts/*
 *   app-icons  — shared/icons/*, desktop/src-tauri/icons/*, client favicons
 */

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');

function scanDir(dir, filter) {
  const full = path.join(ROOT, dir);
  if (!fs.existsSync(full)) return [];
  return fs.readdirSync(full)
    .filter(f => {
      const stat = fs.statSync(path.join(full, f));
      return stat.isFile() && (!filter || filter(f));
    })
    .sort()
    .map(f => {
      const stat = fs.statSync(path.join(full, f));
      return {
        name: f,
        path: '/' + dir.replace(/\\/g, '/') + '/' + f,
        size: stat.size,
      };
    });
}

const manifest = {
  generated: new Date().toISOString(),
  categories: {
    'icons-png': {
      label: 'PNG Icons',
      dir: 'assets/ui/icons',
      assets: scanDir('assets/ui/icons', f => f.endsWith('.png')),
    },
    'icons-svg': {
      label: 'SVG Icons',
      dir: 'assets/ui/icons',
      assets: scanDir('assets/ui/icons', f => f.endsWith('.svg')),
    },
    'concepts': {
      label: 'Concept Art',
      dir: 'assets/concepts',
      assets: scanDir('assets/concepts'),
    },
    'app-icons': {
      label: 'App Icons',
      dirs: ['shared/icons', 'desktop/src-tauri/icons', 'crates/humanity-relay/client'],
      assets: [
        ...scanDir('shared/icons'),
        ...scanDir('desktop/src-tauri/icons'),
        ...scanDir('crates/humanity-relay/client', f => /^favicon\./i.test(f)),
      ],
    },
  },
};

let total = 0;
for (const cat of Object.values(manifest.categories)) {
  total += cat.assets.length;
}
manifest.totalAssets = total;

process.stdout.write(JSON.stringify(manifest, null, 2) + '\n');
