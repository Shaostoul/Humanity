#!/usr/bin/env node
/**
 * gen-web-galaxy-bg.js
 *
 * Regenerates the website's space-background images (web/shared/bg/*.jpg)
 * from the Milky Way glow bake. The bake is REAL integrated starlight
 * (scripts/build-galaxy-glow.js accumulates the star catalog into an
 * equirectangular all-sky texture), so the website background is the same
 * sky the game renders. Run this whenever the bake is rebuilt.
 *
 * Source preference: data/galaxy_glow_ultra.png (16384x8192, from the 25M-star
 * catalog; gitignored, download via Settings > Graphics) with fallback to the
 * committed data/galaxy_glow.png. The output JPEGs (~200 KB) ARE committed.
 *
 * Crop math: the galactic core (Sagittarius bulge; RA 266.42 deg, Dec -29.0 deg)
 * lands at u = 0.2401, v = 0.661 of the equirect (see the mapping contract in
 * assets/shaders/galaxy_glow.wgsl). At 16384x8192 that is pixel (3934, 5414).
 * The wide sitewide crop frames the band arcing diagonally with plenty of dark
 * space for text; the hero crop is tighter and brighter for the landing page.
 * At the 2048x1024 fallback resolution every number below is scaled by /8.
 *
 * Requires ffmpeg on PATH.
 */
const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const ULTRA = path.join(ROOT, 'data', 'galaxy_glow_ultra.png');
const BASE = path.join(ROOT, 'data', 'galaxy_glow.png');
const OUT_DIR = path.join(ROOT, 'web', 'shared', 'bg');

const src = fs.existsSync(ULTRA) ? ULTRA : BASE;
const scale = src === ULTRA ? 1 : 2048 / 16384;
if (src === BASE) {
  console.warn('galaxy_glow_ultra.png not found; using the low-res committed bake.');
  console.warn('Download the ultra bake (Settings > Graphics) for full quality.');
}

fs.mkdirSync(OUT_DIR, { recursive: true });
const r = (n) => Math.round(n * scale);

// Wide sitewide background: band arcs diagonally, mostly dark space.
execSync(
  `ffmpeg -y -loglevel error -i "${src}" -vf "crop=${r(6400)}:${r(3600)}:${r(734)}:${r(3414)},scale=2560:1440" -q:v 4 "${path.join(OUT_DIR, 'galaxy-core.jpg')}"`,
  { stdio: 'inherit' }
);
// Tighter, brighter hero crop for the landing page.
execSync(
  `ffmpeg -y -loglevel error -i "${src}" -vf "crop=${r(4200)}:${r(2360)}:${r(1834)}:${r(4050)},scale=1920:1080" -q:v 4 "${path.join(OUT_DIR, 'galaxy-core-hero.jpg')}"`,
  { stdio: 'inherit' }
);
console.log('Wrote web/shared/bg/galaxy-core.jpg + galaxy-core-hero.jpg from ' + path.basename(src));
