#!/usr/bin/env node
/**
 * climate-fit.js
 *
 * Derives Earth's Layer 1 climate coefficients in data/environment/climate.ron
 * from a public reanalysis, so every number in that row can be reproduced
 * rather than taken on trust. Design: docs/design/environment-fields.md,
 * "Layer 1 as built". CPU model: src/systems/env_layer1.rs.
 *
 * SOURCE: NCEP/NCAR Reanalysis 1 (Kalnay et al. 1996, "The NCEP/NCAR 40-Year
 * Reanalysis Project", Bull. Amer. Meteor. Soc. 77:437-471), long-term monthly
 * means for 1991-2020, T62 Gaussian grid (94 x 192), read through NOAA PSL's
 * public OPeNDAP server as ASCII. Acknowledgement PSL asks for: "NCEP-NCAR
 * Reanalysis 1 data provided by the NOAA PSL, Boulder, Colorado, USA, from
 * their website at https://psl.noaa.gov". US government data, public domain.
 *
 *   air.2m  2 m air temperature        -> the temperature fit
 *   uwnd.10m, vwnd.10m 10 m wind        -> the prevailing wind series
 *   land.sfc  land mask (1 = land)      -> land versus sea
 *
 * WHAT IT FITS
 *
 * 1. Sea-level annual-mean temperature: F0 + F2 * P2(x), x = sin(latitude),
 *    P2(x) = (3x^2 - 1)/2. The form is North, Cahalan and Coakley 1981,
 *    "Energy balance climate models", Rev. Geophys. Space Phys. 19:91-121,
 *    equation (51) with its Table 1 (after North and Coakley 1979), whose own
 *    fit is F0 = 14.9 C, F2 = -28.0 C. Fitted here to SEA cells only, because
 *    the ocean surface IS sea level; land cells carry their terrain height
 *    (Tibet, Antarctica) and would bias a sea-level fit cold.
 *
 * 2. The seasonal cycle: x * (A cos 2 pi tau + B sin 2 pi tau), tau = years
 *    since the northern winter solstice (21 December). Same form as North et
 *    al.'s P1 term (their A11 = -13.2, B11 = -8.1 for the symmetrized northern
 *    hemisphere), but fitted SEPARATELY per hemisphere and per surface (land,
 *    sea), because the southern hemisphere is mostly ocean and its cycle is
 *    about a quarter of the northern one; one symmetric fit cannot hold both.
 *
 * 3. Prevailing wind: annual mean plus first harmonic of the zonal-mean 10 m
 *    wind, as a 16-term sine series in colatitude theta,
 *      u(theta) = sum_{n=1..16} sin(n theta) (m_n + c_n cos 2 pi tau + s_n sin 2 pi tau),
 *    which vanishes at both poles by construction (a wind vector has no single
 *    east component at a pole) and needs no lookup table on the GPU.
 *
 * Usage:
 *   node scripts/climate-fit.js            fetch (cached in the OS temp dir) and print
 *   node scripts/climate-fit.js --cache D  use/keep the downloads in directory D
 *
 * Output is the Earth row's numbers and a check table against North et al.
 */
const fs = require('fs');
const os = require('os');
const path = require('path');
const https = require('https');

const BASE = 'https://psl.noaa.gov/thredds/dodsC/Datasets';
const SOURCES = {
  air: `${BASE}/ncep.reanalysis.derived/surface_gauss/air.2m.mon.ltm.nc.ascii?air[0:11][0:93][0:191],lat[0:93]`,
  uwnd: `${BASE}/ncep.reanalysis.derived/surface_gauss/uwnd.10m.mon.ltm.nc.ascii?uwnd[0:11][0:93][0:191],lat[0:93]`,
  vwnd: `${BASE}/ncep.reanalysis.derived/surface_gauss/vwnd.10m.mon.ltm.nc.ascii?vwnd[0:11][0:93][0:191],lat[0:93]`,
  land: `${BASE}/ncep.reanalysis/surface_gauss/land.sfc.gauss.nc.ascii?land[0:0][0:93][0:191]`,
};
const NY = 94, NX = 192, NT = 12;
/** Wind series length. 16 terms: area-weighted rms error 0.35 m/s (u) and
 * 0.18 m/s (v) over all months; 12 terms gives 0.48 / 0.24. */
const N_TERMS = 16;
/** Month mid-points, day of year, and the northern winter solstice. */
const MONTH_MID_DOY = [15.5, 45, 74.5, 105, 135.5, 166, 196.5, 227.5, 258, 288.5, 319, 349.5];
const SOLSTICE_DOY = 355;
const TAU = MONTH_MID_DOY.map(d => ((((d - SOLSTICE_DOY) / 365.25) % 1) + 1) % 1);

function fetchText(url) {
  return new Promise((resolve, reject) => {
    https.get(url, res => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        resolve(fetchText(res.headers.location));
        return;
      }
      if (res.statusCode !== 200) {
        reject(new Error(`${url}: HTTP ${res.statusCode}`));
        return;
      }
      let body = '';
      res.setEncoding('utf8');
      res.on('data', c => (body += c));
      res.on('end', () => resolve(body));
    }).on('error', reject);
  });
}

async function load(name, cacheDir) {
  const file = path.join(cacheDir, `${name}.txt`);
  if (!fs.existsSync(file)) {
    process.stderr.write(`fetching ${name} ...\n`);
    fs.writeFileSync(file, await fetchText(SOURCES[name]));
  }
  const lines = fs.readFileSync(file, 'utf8').split(/\r?\n/);
  const grid = {};
  let lat = null;
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(/^\[(\d+)\]\[(\d+)\], (.*)$/);
    if (m) grid[`${m[1]},${m[2]}`] = m[3].split(',').map(Number);
    if (lines[i].trim() === 'lat[94]' && lat === null) lat = lines[i + 1].split(',').map(Number);
  }
  return { grid, lat };
}

/** Weighted linear least squares: rows of A, targets y, weights w. */
function lsq(A, y, w) {
  const n = A[0].length;
  const N = Array.from({ length: n }, () => new Float64Array(n));
  const r = new Float64Array(n);
  for (let i = 0; i < A.length; i++) {
    for (let a = 0; a < n; a++) {
      r[a] += w[i] * A[i][a] * y[i];
      for (let b = 0; b < n; b++) N[a][b] += w[i] * A[i][a] * A[i][b];
    }
  }
  for (let i = 0; i < n; i++) {
    for (let k = i + 1; k < n; k++) {
      const f = N[k][i] / N[i][i];
      for (let j = i; j < n; j++) N[k][j] -= f * N[i][j];
      r[k] -= f * r[i];
    }
  }
  const s = new Float64Array(n);
  for (let i = n - 1; i >= 0; i--) {
    let v = r[i];
    for (let j = i + 1; j < n; j++) v -= N[i][j] * s[j];
    s[i] = v / N[i][i];
  }
  return Array.from(s);
}

/** Mean of the cells of row j whose land flag equals `want` (null = all). */
function rowMean(grid, t, j, mask, want) {
  const row = grid[`${t},${j}`];
  let s = 0, n = 0;
  for (let x = 0; x < NX; x++) {
    if (want !== null && (mask[x] > 0.5 ? 1 : 0) !== want) continue;
    s += row[x];
    n++;
  }
  return n ? { mean: s / n, n } : null;
}

async function main() {
  const ci = process.argv.indexOf('--cache');
  const cacheDir = ci > 0 ? process.argv[ci + 1] : path.join(os.tmpdir(), 'humanity-climate-fit');
  fs.mkdirSync(cacheDir, { recursive: true });
  const air = await load('air', cacheDir);
  const uwnd = await load('uwnd', cacheDir);
  const vwnd = await load('vwnd', cacheDir);
  const land = await load('land', cacheDir);
  const lat = air.lat;
  const sinL = j => Math.sin((lat[j] * Math.PI) / 180);
  const cosL = j => Math.cos((lat[j] * Math.PI) / 180);

  // 1. Sea-only annual mean, F0 + F2 P2(x).
  {
    const A = [], y = [], w = [];
    for (let j = 0; j < NY; j++) {
      const mask = land.grid[`0,${j}`];
      let m = 0, n = 0;
      for (let t = 0; t < NT; t++) {
        const r = rowMean(air.grid, t, j, mask, 0);
        if (!r) break;
        m += r.mean / NT;
        n = r.n;
      }
      if (!n) continue;
      const x = sinL(j);
      A.push([1, 1.5 * x * x - 0.5]);
      y.push(m - 273.15);
      w.push(cosL(j) * n);
    }
    const [F0, F2] = lsq(A, y, w);
    console.log(`sea_level_mean_c: ${F0.toFixed(2)},   // North et al. 1981 Table 1: 14.9`);
    console.log(`p2_c: ${F2.toFixed(2)},              // North et al. 1981 Table 1: -28.0`);
  }

  // 2. Seasonal first harmonic, per hemisphere and surface, proportional to x.
  for (const [hemi, sign] of [['north', 1], ['south', -1]]) {
    for (const [surf, want] of [['sea', 0], ['land', 1]]) {
      const A = [], y = [], w = [];
      for (let j = 0; j < NY; j++) {
        if (Math.sign(lat[j]) !== sign) continue;
        const mask = land.grid[`0,${j}`];
        const first = rowMean(air.grid, 0, j, mask, want);
        if (!first || first.n < 4) continue;
        const x = sinL(j);
        for (let t = 0; t < NT; t++) {
          const r = rowMean(air.grid, t, j, mask, want);
          // Columns: a per-row annual mean (absorbed, not reported), then
          // x cos and x sin, the two shared seasonal coefficients.
          const row = new Array(NY + 2).fill(0);
          row[j] = 1;
          row[NY] = x * Math.cos(2 * Math.PI * TAU[t]);
          row[NY + 1] = x * Math.sin(2 * Math.PI * TAU[t]);
          A.push(row);
          y.push(r.mean);
          w.push(cosL(j) * first.n);
        }
      }
      // Drop the unused per-row columns so the system is not singular.
      const used = [...new Set(A.map(r => r.findIndex((v, k) => k < NY && v === 1)))];
      const A2 = A.map(r => [...used.map(k => r[k]), r[NY], r[NY + 1]]);
      const s = lsq(A2, y, w);
      const a = s[s.length - 2], b = s[s.length - 1];
      console.log(`season_${hemi}_${surf}: (${a.toFixed(2)}, ${b.toFixed(2)}),`);
    }
  }
  console.log('// North et al. 1981 Table 1 (symmetrized north, land and sea together): (-13.2, -8.1)');

  // 3. Prevailing wind sine series.
  for (const [name, D] of [['u', uwnd], ['v', vwnd]]) {
    const A = [], y = [], w = [];
    for (let j = 0; j < NY; j++) {
      const theta = ((90 - lat[j]) * Math.PI) / 180;
      for (let t = 0; t < NT; t++) {
        const r = rowMean(D.grid, t, j, null, null);
        const ct = Math.cos(2 * Math.PI * TAU[t]), st = Math.sin(2 * Math.PI * TAU[t]);
        const row = [];
        for (let n = 1; n <= N_TERMS; n++) {
          const bn = Math.sin(n * theta);
          row.push(bn, bn * ct, bn * st);
        }
        A.push(row);
        y.push(r.mean);
        w.push(cosL(j));
      }
    }
    const c = lsq(A, y, w);
    let se = 0, sw = 0;
    for (let i = 0; i < A.length; i++) {
      const e = A[i].reduce((s, v, k) => s + v * c[k], 0) - y[i];
      se += w[i] * e * e;
      sw += w[i];
    }
    const pick = k => c.filter((_, i) => i % 3 === k).map(v => v.toFixed(3)).join(', ');
    console.log(`// ${name}: area-weighted rms error ${Math.sqrt(se / sw).toFixed(2)} m/s over all months`);
    console.log(`wind_${name}_mean: [${pick(0)}],`);
    console.log(`wind_${name}_cos: [${pick(1)}],`);
    console.log(`wind_${name}_sin: [${pick(2)}],`);
  }
}

main().catch(e => {
  console.error(e.message);
  process.exit(1);
});
