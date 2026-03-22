#!/usr/bin/env node
/**
 * generate-asset-manifest.js
 * Scans the repo for all image assets and outputs a JSON manifest.
 * Usage: node scripts/generate-asset-manifest.js > asset-manifest.json
 *
 * Categories:
 *   icons-png  — assets/icons/*.png
 *   icons-svg  — assets/icons/*.svg
 *   concepts   — assets/concepts/*
 *   app-icons  — ui/shared/icons/*, app/icons/*, client favicons
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
      dir: 'assets/icons',
      assets: scanDir('assets/icons', f => f.endsWith('.png')),
    },
    'icons-svg': {
      label: 'SVG Icons',
      dir: 'assets/icons',
      assets: scanDir('assets/icons', f => f.endsWith('.svg')),
    },
    'concepts': {
      label: 'Concept Art',
      dir: 'assets/concepts',
      assets: scanDir('assets/concepts'),
    },
    'app-icons': {
      label: 'App Icons',
      dirs: ['web/shared/icons', 'app/icons', 'web/chat'],
      assets: [
        ...scanDir('web/shared/icons'),
        ...scanDir('app/icons'),
        ...scanDir('web/chat', f => /^favicon\./i.test(f)),
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
