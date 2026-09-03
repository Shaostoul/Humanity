// 1-D twin of the shipped cloud march estimator (40-clouds.wgsl ~3039-3345),
// used to measure (a) per-pixel variance vs the frozen depth-jitter phase and
// (b) mean bias vs coarse step, for density edges of different widths and for
// sub-step field content (warp hash, interior turbulence).
//
// Units: metres. sigma = 45/km (cumulus). The ray starts 1500 m before the
// nominal surface x=0 in clear air.
'use strict';
const SIGMA = 0.045;      // 1/m  (reg.ext_km = 45 for cumulus)
const MU = 0.6;           // sun-path obliquity factor for the lighting twin
const TAU_MAX = 0.75;     // CLOUD_STEP_TAU_MAX
const GATE = 0.02;        // CLOUD_STEP_INTERIOR_GATE
const MFP_FLOOR = 23.2;   // slab_h * 0.002 (11.6 km slab)
const ITER_CAP = 224;

function mulberry32(a) {
  return function () {
    a |= 0; a = (a + 0x6D2B79F5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
// Band-limited stationary noise in [-1,1] around wavelength lam.
function makeNoise(lam, seed) {
  const rnd = mulberry32(seed);
  const comps = [];
  for (let k = 0; k < 5; k++) {
    comps.push({ w: 2 * Math.PI / (lam * (0.7 + 0.6 * rnd())), ph: 2 * Math.PI * rnd(), a: 0.5 + 0.5 * rnd() });
  }
  const norm = comps.reduce((s, c) => s + c.a, 0);
  return x => comps.reduce((s, c) => s + c.a * Math.sin(c.w * x + c.ph), 0) / norm;
}

// Density profile along the ray.
//  W: ramp width (m) from the (hashed) surface to full density; pw: ramp power.
//  hashA/hashLam: surface displacement hash (constructed warp shell).
//  turbAmp/turbLam: interior multiplicative turbulence.
function profile(o, seed) {
  const nH = o.hashA ? makeNoise(o.hashLam, seed * 7 + 1) : null;
  const nT = o.turbAmp ? makeNoise(o.turbLam, seed * 7 + 2) : null;
  const dmax = o.Dmax || 1;
  return function (x) {
    let xe = x;
    if (nH) xe = x + o.hashA * nH(x);
    if (xe <= 0) return 0;
    let v = Math.pow(Math.min(xe / o.W, 1), o.pw || 1);
    if (nT) v *= 1 + o.turbAmp * nT(x);
    return Math.max(0, Math.min(1, v)) * dmax;
  };
}

// Column optical depth table for the lighting twin (exact integral of d).
function tauTable(d, x0, x1, dx) {
  const n = Math.ceil((x1 - x0) / dx);
  const tab = new Float64Array(n + 1);
  let acc = 0;
  for (let i = 1; i <= n; i++) {
    const xa = x0 + (i - 1) * dx, xb = x0 + i * dx;
    acc += 0.5 * (d(xa) + d(xb)) * dx * SIGMA;
    tab[i] = acc;
  }
  return function (x) {
    const f = (x - x0) / dx;
    const i = Math.floor(f);
    if (i < 0) return 0;
    if (i >= n) return tab[n];
    return tab[i] + (tab[i + 1] - tab[i]) * (f - i);
  };
}

// The shipped march. h = coarse step (step_near / cone law value), j = frozen
// per-pixel jitter in (0,1). sdf: null (noise path) or {margin} (constructed:
// sphere-trace outside the margin, 45 m inside it - 40-clouds.wgsl ~3087-3101).
function march(d, tauCol, h, j, sdf, opts) {
  const L0 = 1500, m1 = 5000;
  let t = -L0, dens_prev = 0, sdf_prev = 1e9, trans = 1;
  let acc = 0, accw = 0, accd = 0, first = -1, iters = 0;
  const trapezoid = !(opts && opts.noTrapezoid);
  for (let i = 0; i < ITER_CAP; i++) {
    if (t >= m1) break;
    iters = i + 1;
    let dt = h;
    if (dens_prev > GATE) dt = Math.min(dt, Math.max(TAU_MAX / (SIGMA * dens_prev), MFP_FLOOR));
    if (sdf && sdf_prev < 1e8) {
      const safe = sdf_prev - sdf.margin;
      if (safe > 0) dt = Math.max(dt, safe); else dt = Math.min(dt, sdf.refine || 45);
    }
    dt = Math.min(dt, m1 - t);
    if (i === ITER_CAP - 1) dt = m1 - t;
    const tm = t + dt * j;
    if (i === 0) t += dt * Math.max(j, 1e-3); else t += dt;
    const dens = d(tm);
    const sdfv = sdf ? -tm : 1e9; // nominal signed distance to the surface at x=0
    if (dens > GATE && dens_prev <= GATE && SIGMA * dens * dt > TAU_MAX) {
      t -= dt; dens_prev = dens; sdf_prev = sdfv; continue; // coarse-entry backtrack
    }
    const dens_last = dens_prev;
    dens_prev = dens; sdf_prev = sdfv;
    if (dens <= 0.001) continue;
    const di = trapezoid ? 0.5 * (dens + dens_last) : dens;
    const a = 1 - Math.exp(-SIGMA * di * dt);
    if (first < 0) first = tm;
    const L = Math.exp(-MU * tauCol(tm));
    acc += L * trans * a; accw += trans * a; accd += tm * trans * a;
    trans *= 1 - a;
    if (trans <= 0.005) break;
  }
  return { body: 1 - trans, rad: accw > 0 ? acc / accw : 0, first, meant: accw > 0 ? accd / accw : 0, iters };
}

function stats(arr) {
  const n = arr.length;
  const m = arr.reduce((s, v) => s + v, 0) / n;
  const v = arr.reduce((s, x) => s + (x - m) * (x - m), 0) / n;
  return { mean: m, sd: Math.sqrt(v) };
}

function run(label, o, hs, sdf, seeds, opts) {
  const NJ = 96;
  const out = {};
  // truth: 0.5 m fixed step, centred sample, same trapezoid, no refinement games
  let truthB = 0, truthR = 0, truthM = 0;
  const profs = [];
  for (let s = 0; s < seeds; s++) {
    const d = profile(o, s + 1);
    const tc = tauTable(d, -200, 5000, 0.25);
    profs.push({ d, tc });
    const tr = march(d, tc, 0.5, 0.5, null, opts);
    truthB += tr.body / seeds; truthR += tr.rad / seeds; truthM += tr.meant / seeds;
  }
  for (const h of hs) {
    const B = [], R = [], F = [], M = [], I = [];
    for (const p of profs) {
      for (let k = 0; k < NJ; k++) {
        const j = (k + 0.5) / NJ;
        const r = march(p.d, p.tc, h, j, sdf, opts);
        B.push(r.body); R.push(r.rad); F.push(r.first); M.push(r.meant); I.push(r.iters);
      }
    }
    const sb = stats(B), sr = stats(R), sf = stats(F), sm = stats(M), si = stats(I);
    out[h] = { body: sb, rad: sr, first: sf, meant: sm, iters: si.mean };
  }
  const line = h => {
    const r = out[h];
    return `  h=${String(h).padStart(3)}: body mean ${r.body.mean.toFixed(3)} (bias ${(r.body.mean - truthB >= 0 ? '+' : '')}${(r.body.mean - truthB).toFixed(3)}) sd ${r.body.sd.toFixed(3)} | rad mean ${r.rad.mean.toFixed(3)} (bias ${(r.rad.mean - truthR >= 0 ? '+' : '')}${(r.rad.mean - truthR).toFixed(3)}) sd ${r.rad.sd.toFixed(3)} | first sd ${r.first.sd.toFixed(0)} m | meant bias ${(r.meant.mean - truthM).toFixed(0)} m sd ${r.meant.sd.toFixed(0)} m | iters ${r.iters.toFixed(0)}`;
  };
  console.log(`\n${label}   [truth: body ${truthB.toFixed(3)} rad ${truthR.toFixed(3)} meant ${truthM.toFixed(0)} m]`);
  for (const h of hs) console.log(line(h));
  return out;
}

const HS = [45, 188, 375, 522];
console.log('=== NOISE PATH: plain ramps, no sub-step content (dens = (x/W)^1.7, D=1) ===');
for (const W of [10, 93, 300, 520, 1000, 2000]) run(`ramp W=${W} m pw=1.7`, { W, pw: 1.7 }, HS, null, 1);

console.log('\n=== NOISE PATH: skirt-only ramp to D=0.31 over W, then to 1 over 1800 m (the measured noise-path profile) ===');
{
  // piecewise: 0..W -> 0.31 (pow 1.7 of carve), W..1800 -> 1
  const o = { W: 520, pw: 1.7 };
  const custom = { W: 1800, pw: 1.7 };
  run('measured-shape ramp (1.8 km to full, 0.31 at 520 m)', custom, HS, null, 1);
}

console.log('\n=== CONSTRUCTED PATH: 90 m rind, warp hash shell (+-46 m at 6.6 m cells), SDF refine 45 m inside 311 m margin ===');
const SDF = { margin: 311, refine: 45 };
run('rind 90 m, no hash, no turb', { W: 90, pw: 1 }, HS, SDF, 1);
run('rind 90 m + hash +-46 m @ 6.6 m', { W: 90, pw: 1, hashA: 46, hashLam: 6.6 }, HS, SDF, 6);
run('rind 90 m + hash +-46 m @ 6.6 m + turb +-42% @ 25 m', { W: 90, pw: 1, hashA: 46, hashLam: 6.6, turbAmp: 0.42, turbLam: 25 }, HS, SDF, 6);
run('rind 300 m (wide-edge bit) + hash + turb', { W: 300, pw: 1, hashA: 46, hashLam: 6.6, turbAmp: 0.42, turbLam: 25 }, HS, SDF, 6);
run('rind 300 m, hash band-limited to 60 m cells (honest warp mip), turb @ 25 m', { W: 300, pw: 1, hashA: 46, hashLam: 60, turbAmp: 0.42, turbLam: 25 }, HS, SDF, 6);
run('rind 300 m, hash @ 60 m, turb band-limited to 120 m', { W: 300, pw: 1, hashA: 46, hashLam: 60, turbAmp: 0.42, turbLam: 120 }, HS, SDF, 6);
run('rind 90 m, hash @ 60 m, turb @ 120 m (band-limit only, narrow rind)', { W: 90, pw: 1, hashA: 46, hashLam: 60, turbAmp: 0.42, turbLam: 120 }, HS, SDF, 6);
run('rind 300 m, NO hash, NO turb (pure wide ramp)', { W: 300, pw: 1 }, HS, SDF, 1);
run('rind 300 m, SDF refine 15 m (step law tightened instead)', { W: 90, pw: 1, hashA: 46, hashLam: 6.6, turbAmp: 0.42, turbLam: 25 }, HS, { margin: 311, refine: 15 }, 6);

console.log('\n=== INTERIOR ONLY: full-density body with turbulence, no edge coin flip possible (x starts inside) ===');
{
  // Camera inside cloud: profile constant 1 with turb; the estimator variance here is pure interior aliasing.
  const o = { W: 1, pw: 1, turbAmp: 0.42, turbLam: 25 };
  const o2 = { W: 1, pw: 1, turbAmp: 0.42, turbLam: 120 };
  const o3 = { W: 1, pw: 1, turbAmp: 0.42, turbLam: 400 };
  for (const [lab, oo] of [['turb 25 m', o], ['turb 120 m', o2], ['turb 400 m', o3]]) {
    const R = [], B = [];
    let truthR = 0;
    const seeds = 6, NJ = 96;
    for (let s = 0; s < seeds; s++) {
      const d0 = profile(oo, s + 1);
      // shift so the ray starts already inside: x -> x + 1600
      const d = x => d0(x + 1600);
      const tc = tauTable(d, -200, 5000, 0.25);
      truthR += march(d, tc, 0.5, 0.5, null).rad / seeds;
      for (let k = 0; k < NJ; k++) { const r = march(d, tc, 188, (k + 0.5) / NJ, null); R.push(r.rad); B.push(r.body); }
    }
    const sr = stats(R);
    console.log(`  interior ${lab}: rad mean ${sr.mean.toFixed(3)} (truth ${truthR.toFixed(3)}) sd ${sr.sd.toFixed(3)}   body sd ${stats(B).sd.toFixed(4)}`);
  }
}
