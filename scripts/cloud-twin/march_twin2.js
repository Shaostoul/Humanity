// 1-D twin of the shipped cloud march estimator, v2: truth run uncapped,
// optional lag-corrected SDF stride, thin-chord (silhouette) profiles.
'use strict';
const SIGMA = 0.045, MU = 0.6, TAU_MAX = 0.75, GATE = 0.02, MFP_FLOOR = 23.2, ITER_CAP = 224;

function mulberry32(a) { return function () { a |= 0; a = (a + 0x6D2B79F5) | 0; let t = Math.imul(a ^ (a >>> 15), 1 | a); t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t; return ((t ^ (t >>> 14)) >>> 0) / 4294967296; }; }
function makeNoise(lam, seed) { const rnd = mulberry32(seed); const comps = []; for (let k = 0; k < 5; k++) comps.push({ w: 2 * Math.PI / (lam * (0.7 + 0.6 * rnd())), ph: 2 * Math.PI * rnd(), a: 0.5 + 0.5 * rnd() }); const norm = comps.reduce((s, c) => s + c.a, 0); return x => comps.reduce((s, c) => s + c.a * Math.sin(c.w * x + c.ph), 0) / norm; }

// o.T: total chord thickness (mirror the ramp at T/2) for silhouette grazes.
function profile(o, seed) {
  const nH = o.hashA ? makeNoise(o.hashLam, seed * 7 + 1) : null;
  const nT = o.turbAmp ? makeNoise(o.turbLam, seed * 7 + 2) : null;
  const dmax = o.Dmax || 1;
  return function (x) {
    let xe = x;
    if (nH) xe = x + o.hashA * nH(x);
    if (o.T) xe = o.T / 2 - Math.abs(xe - o.T / 2); // distance in from the nearer face
    if (xe <= 0) return 0;
    let v = Math.pow(Math.min(xe / o.W, 1), o.pw || 1);
    if (nT) v *= 1 + o.turbAmp * nT(x);
    return Math.max(0, Math.min(1, v)) * dmax;
  };
}
function tauTable(d, x0, x1, dx) { const n = Math.ceil((x1 - x0) / dx); const tab = new Float64Array(n + 1); let acc = 0; for (let i = 1; i <= n; i++) { const xa = x0 + (i - 1) * dx, xb = x0 + i * dx; acc += 0.5 * (d(xa) + d(xb)) * dx * SIGMA; tab[i] = acc; } return x => { const f = (x - x0) / dx; const i = Math.floor(f); if (i < 0) return 0; if (i >= n) return tab[n]; return tab[i] + (tab[i + 1] - tab[i]) * (f - i); }; }

// sdf: null | {margin, refine, fixLag}
function march(d, tauCol, h, j, sdf, cap) {
  const L0 = 1500, m1 = 5000;
  let t = -L0, dens_prev = 0, sdf_prev = 1e9, tm_prev = -L0, trans = 1;
  let acc = 0, accw = 0, accd = 0, first = -1, iters = 0;
  const CAP = cap || ITER_CAP;
  for (let i = 0; i < CAP; i++) {
    if (t >= m1) break;
    iters = i + 1;
    let dt = h;
    if (dens_prev > GATE) dt = Math.min(dt, Math.max(TAU_MAX / (SIGMA * dens_prev), MFP_FLOOR));
    if (sdf && sdf_prev < 1e8) {
      let safe = sdf_prev - sdf.margin;
      if (sdf.fixLag) safe -= (t - tm_prev); // stride measured from the sample point, not the step end
      if (safe > 0) dt = Math.max(dt, safe); else dt = Math.min(dt, sdf.refine || 45);
    }
    dt = Math.min(dt, m1 - t);
    if (i === CAP - 1) dt = m1 - t;
    const tm = t + dt * j;
    if (i === 0) t += dt * Math.max(j, 1e-3); else t += dt;
    const dens = d(tm);
    const sdfv = sdf ? -tm : 1e9;
    if (dens > GATE && dens_prev <= GATE && SIGMA * dens * dt > TAU_MAX) { t -= dt; dens_prev = dens; sdf_prev = sdfv; tm_prev = tm; continue; }
    const dens_last = dens_prev;
    dens_prev = dens; sdf_prev = sdfv; tm_prev = tm;
    if (dens <= 0.001) continue;
    const di = 0.5 * (dens + dens_last);
    const a = 1 - Math.exp(-SIGMA * di * dt);
    if (first < 0) first = tm;
    const L = Math.exp(-MU * tauCol(tm));
    acc += L * trans * a; accw += trans * a; accd += tm * trans * a;
    trans *= 1 - a;
    if (trans <= 0.005) break;
  }
  return { body: 1 - trans, rad: accw > 0 ? acc / accw : 0, first, meant: accw > 0 ? accd / accw : 0, iters };
}
function stats(arr) { const n = arr.length; const m = arr.reduce((s, v) => s + v, 0) / n; const v = arr.reduce((s, x) => s + (x - m) * (x - m), 0) / n; return { mean: m, sd: Math.sqrt(v) }; }
const f3 = v => (v >= 0 ? '+' : '') + v.toFixed(3);

function run(label, o, hs, sdf, seeds) {
  const NJ = 96, out = {};
  let truthB = 0, truthR = 0, truthM = 0;
  const profs = [];
  for (let s = 0; s < seeds; s++) {
    const d = profile(o, s + 1); const tc = tauTable(d, -200, 5000, 0.25); profs.push({ d, tc });
    const tr = march(d, tc, 0.5, 0.5, null, 1e6);
    truthB += tr.body / seeds; truthR += tr.rad / seeds; truthM += tr.meant / seeds;
  }
  console.log(`\n${label}   [truth: body ${truthB.toFixed(3)} rad ${truthR.toFixed(3)} meant ${truthM.toFixed(0)} m]`);
  for (const h of hs) {
    const B = [], R = [], F = [], M = [], I = [];
    for (const p of profs) for (let k = 0; k < NJ; k++) { const r = march(p.d, p.tc, h, (k + 0.5) / NJ, sdf); B.push(r.body); R.push(r.rad); F.push(r.first); M.push(r.meant); I.push(r.iters); }
    const sb = stats(B), sr = stats(R), sf = stats(F), sm = stats(M);
    out[h] = { sb, sr };
    console.log(`  h=${String(h).padStart(3)}: body ${sb.mean.toFixed(3)} bias ${f3(sb.mean - truthB)} sd ${sb.sd.toFixed(3)} | rad ${sr.mean.toFixed(3)} bias ${f3(sr.mean - truthR)} sd ${sr.sd.toFixed(3)} | first sd ${sf.sd.toFixed(0)} m | meant bias ${(sm.mean - truthM).toFixed(0)} m sd ${sm.sd.toFixed(0)} m | iters ${stats(I).mean.toFixed(0)}`);
  }
  return out;
}

const HS = [45, 188, 375, 522];
console.log('=== A. NOISE PATH, opaque body behind the edge: ramp dens=(x/W)^1.7 ===');
for (const W of [93, 300, 520, 1000, 2000]) run(`A ramp W=${W}`, { W, pw: 1.7 }, HS, null, 1);

console.log('\n=== B. NOISE PATH, THIN CHORD (silhouette graze): bump of thickness T with edge ramp W (pw 1.7) ===');
for (const [T, W] of [[150, 93], [400, 93], [400, 300], [1000, 300], [1000, 520], [2000, 520], [2000, 1000]]) run(`B chord T=${T} W=${W}`, { T, W, pw: 1.7 }, HS, null, 1);

console.log('\n=== C. CONSTRUCTED PATH as shipped (stride applied from step end; 311 m margin; 45 m refine) ===');
const SDF = { margin: 311, refine: 45 };
run('C rind 90 m, plain', { W: 90, pw: 1 }, HS, SDF, 1);
run('C rind 90 m + hash 46 m @ 6.6 m + turb 42% @ 25 m', { W: 90, pw: 1, hashA: 46, hashLam: 6.6, turbAmp: 0.42, turbLam: 25 }, HS, SDF, 6);
run('C rind 300 m + hash + turb (wide-edge bit; margin 521)', { W: 300, pw: 1, hashA: 46, hashLam: 6.6, turbAmp: 0.42, turbLam: 25 }, HS, { margin: 521, refine: 45 }, 6);
run('C rind 90 m thin chord T=400 + hash + turb', { T: 400, W: 90, pw: 1, hashA: 46, hashLam: 6.6, turbAmp: 0.42, turbLam: 25 }, HS, SDF, 6);

console.log('\n=== D. CONSTRUCTED PATH with the stride LAG CORRECTED (stride measured from the sample point) ===');
const SDFX = { margin: 311, refine: 45, fixLag: true };
run('D rind 90 m, plain', { W: 90, pw: 1 }, HS, SDFX, 1);
run('D rind 90 m + hash 46 m @ 6.6 m', { W: 90, pw: 1, hashA: 46, hashLam: 6.6 }, HS, SDFX, 6);
run('D rind 90 m + hash + turb 42% @ 25 m', { W: 90, pw: 1, hashA: 46, hashLam: 6.6, turbAmp: 0.42, turbLam: 25 }, HS, SDFX, 6);
run('D rind 300 m + hash + turb', { W: 300, pw: 1, hashA: 46, hashLam: 6.6, turbAmp: 0.42, turbLam: 25 }, HS, { margin: 521, refine: 45, fixLag: true }, 6);
run('D rind 300 m + hash @ 60 m (band-limited warp) + turb @ 25 m', { W: 300, pw: 1, hashA: 46, hashLam: 60, turbAmp: 0.42, turbLam: 25 }, HS, { margin: 521, refine: 45, fixLag: true }, 6);
run('D rind 300 m + hash @ 60 m + turb @ 120 m', { W: 300, pw: 1, hashA: 46, hashLam: 60, turbAmp: 0.42, turbLam: 120 }, HS, { margin: 521, refine: 45, fixLag: true }, 6);
run('D rind 90 m + hash @ 60 m + turb @ 120 m (band-limit only)', { W: 90, pw: 1, hashA: 46, hashLam: 60, turbAmp: 0.42, turbLam: 120 }, HS, SDFX, 6);
run('D rind 90 m thin chord T=400 + hash + turb', { T: 400, W: 90, pw: 1, hashA: 46, hashLam: 6.6, turbAmp: 0.42, turbLam: 25 }, HS, SDFX, 6);
run('D rind 300 m thin chord T=400 + hash@60 + turb@120', { T: 400, W: 300, pw: 1, hashA: 46, hashLam: 60, turbAmp: 0.42, turbLam: 120 }, HS, { margin: 521, refine: 45, fixLag: true }, 6);
run('D rind 300 m + hash + turb, refine 15 m', { W: 300, pw: 1, hashA: 46, hashLam: 6.6, turbAmp: 0.42, turbLam: 25 }, HS, { margin: 521, refine: 15, fixLag: true }, 6);
