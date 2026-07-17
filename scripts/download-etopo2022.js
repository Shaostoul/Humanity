#!/usr/bin/env node
/**
 * download-etopo2022.js
 *
 * Fetch the NOAA ETOPO 2022 15-arc-second global relief model (surface/ice
 * elevation, GeoTIFF) - distributed as 288 tiles of 15x15 degrees, each a
 * plain (non-Big) TIFF of 3600x3600 float32 metres. Public domain (US
 * government work). Total ~7 GB.
 *
 * Destination: ext_data/etopo2022_15s/ (gitignored - source data, never
 * committed; we ship only derived artifacts like the base heightmap grid,
 * region tile packs, and the ocean mask).
 *
 * Resumable: a tile whose on-disk size matches the server's Content-Length
 * is skipped, so re-running after an interruption only fetches what's
 * missing. Sequential on purpose (be polite to NOAA).
 *
 * Usage: node scripts/download-etopo2022.js
 */
const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const BASE =
  'https://www.ngdc.noaa.gov/mgg/global/relief/ETOPO2022/data/15s/15s_surface_elev_gtif';
const OUT_DIR = path.join(__dirname, '..', 'ext_data', 'etopo2022_15s');
fs.mkdirSync(OUT_DIR, { recursive: true });

// Tile corners: the name encodes the tile's NORTH-WEST corner.
// Rows: N90 down to S60 in 15-degree steps (12 rows; the equator row is N00).
// Cols: W180 east to E165 in 15-degree steps (24 cols).
const rows = [];
for (let lat = 90; lat > -90; lat -= 15) {
  rows.push(lat > 0 ? `N${String(lat).padStart(2, '0')}` : lat === 0 ? 'N00' : `S${String(-lat).padStart(2, '0')}`);
}
const cols = [];
for (let lon = -180; lon < 180; lon += 15) {
  cols.push(lon < 0 ? `W${String(-lon).padStart(3, '0')}` : `E${String(lon).padStart(3, '0')}`);
}

function headLength(url) {
  try {
    const out = execFileSync('curl', ['-sI', url], { encoding: 'utf8' });
    const m = out.match(/content-length:\s*(\d+)/i);
    return m ? parseInt(m[1], 10) : null;
  } catch (e) {
    return null;
  }
}

let done = 0, skipped = 0, failed = 0, bytes = 0;
const total = rows.length * cols.length;
for (const r of rows) {
  for (const c of cols) {
    const name = `ETOPO_2022_v1_15s_${r}${c}_surface.tif`;
    const url = `${BASE}/${name}`;
    const dest = path.join(OUT_DIR, name);
    const want = headLength(url);
    if (want == null) {
      console.error(`MISSING on server: ${name}`);
      failed++;
      continue;
    }
    if (fs.existsSync(dest) && fs.statSync(dest).size === want) {
      skipped++;
      continue;
    }
    try {
      execFileSync('curl', ['-sf', '-o', dest, url], { stdio: 'inherit' });
      const got = fs.statSync(dest).size;
      if (got !== want) throw new Error(`size mismatch ${got} != ${want}`);
      bytes += got;
      done++;
      if (done % 10 === 0) {
        console.log(`progress: ${done + skipped}/${total} tiles (${(bytes / 1073741824).toFixed(2)} GB fetched this run)`);
      }
    } catch (e) {
      console.error(`FAILED ${name}: ${e.message}`);
      failed++;
    }
  }
}
console.log(
  `ETOPO 2022 15s: ${done} downloaded, ${skipped} already present, ${failed} failed, ${(bytes / 1073741824).toFixed(2)} GB this run -> ${OUT_DIR}`
);
if (failed > 0) process.exit(1);
