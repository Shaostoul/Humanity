// 1-D model of the cloud march estimator in 40-clouds.wgsl (lines ~3039-3260),
// used to test: does the EXPECTED transmittance depend on the coarse step h
// for a cliff edge under a uniform per-ray jitter, and does that dependence
// vanish when the edge is a ramp wider than h?
//
// Mirrors: shared per-ray jitter (tm = t_cur + dt*j, first step advances by
// dt*j), interior MFP refinement (dens_prev > 0.02 -> dt = min(dt,
// max(0.75/(sigma*dens_prev), 23.2 m))), the coarse-entry backtrack, the
// trapezoid with skip-if-clear (v0.1258), front-to-back Beer accumulation.

const SIGMA = 0.045;          // 45/km cumulus, metres^-1
const FLOOR = 11600 * 0.002;  // slab_h * 0.002 = 23.2 m
const GATE = 0.02;
const TAU_MAX = 0.75;

function smoothstep(a, b, x) { const t = Math.min(1, Math.max(0, (x - a) / (b - a))); return t * t * (3 - 2 * t); }

// ---- profiles: return density in [0,1] at x (metres from camera) ----
function cliff(D, e1, thick) { return x => (x >= e1 && x < e1 + thick) ? D : 0; }
function ramp(D, e1, w, thick) {
  return x => {
    if (x < e1 || x >= e1 + thick) return 0;
    const u = x - e1;
    if (u < w) return D * u / w;
    if (u > thick - w) return D * (thick - u) / w;
    return D;
  };
}
function sramp(D, e1, w, thick) { // smoothstep-shaped ramp
  return x => {
    if (x < e1 || x >= e1 + thick) return 0;
    const u = x - e1;
    if (u < w) return D * smoothstep(0, 1, u / w);
    if (u > thick - w) return D * smoothstep(0, 1, (thick - u) / w);
    return D;
  };
}
// Noise-path skirt as measured: carve slope 3.3e-4/m, erosion floor 0.41,
// dens_n = 1.69*(carve-0.41)/carve, dens = dens_n^1.7 * smoothstep(0,.12,carve)
function noiseSkirt(e1, thick) {
  const kc = 3.3e-4;
  const f = u => {
    const carve = Math.min(1, Math.max(0, kc * u));
    if (carve <= 0.41) return 0;
    const dn = Math.min(1, 1.69 * (carve - 0.41) / carve);
    return Math.pow(dn, 1.7) * smoothstep(0, 0.12, carve);
  };
  return x => {
    if (x < e1 || x >= e1 + thick) return 0;
    const u = x - e1;
    return f(Math.min(u, thick - u));
  };
}

// ---- reference (fine quadrature) ----
function reference(rho, L, shadeMode) {
  const n = 50000, dx = L / n;
  let tau = 0, Lacc = 0;
  for (let i = 0; i < n; i++) {
    const x = (i + 0.5) * dx;
    const d = rho(x);
    const a = SIGMA * d * dx;
    const S = shade(shadeMode, tau);
    Lacc += Math.exp(-tau) * a * S;
    tau += a;
  }
  return { T: Math.exp(-tau), L: Lacc, tau };
}
// shade: 'flat' = 1 (then L = 1-T); 'rind' = sun behind camera, lit by the
// depth already marched (bright rind, dark core)
function shade(mode, tauDepth) { return mode === 'flat' ? 1 : Math.exp(-tauDepth); }

// ---- the march ----
// opts: {est:'point'|'trap', backtrack:bool, mfp:bool, h:coarse step}
function march(rho, L, j, opts, shadeMode) {
  let t_cur = 0, dens_prev = 0, trans = 1, Lacc = 0, tauHat = 0, tauDepth = 0;
  const h = opts.h;
  for (let i = 0; i < 4000; i++) {
    if (t_cur >= L) break;
    let dt = Math.min(h, L - t_cur);
    if (opts.mfp && dens_prev > GATE) dt = Math.min(dt, Math.max(TAU_MAX / (SIGMA * dens_prev), FLOOR));
    const tm = t_cur + dt * j;
    if (i === 0) t_cur += dt * Math.max(j, 1e-3); else t_cur += dt;
    const dens = tm < 0 ? 0 : rho(tm);
    if (opts.backtrack && dens > GATE && dens_prev <= GATE && SIGMA * dens * dt > TAU_MAX) {
      t_cur -= dt; dens_prev = dens; continue;
    }
    const dens_last = dens_prev; dens_prev = dens;
    if (dens <= 0.001) continue;
    const dens_i = opts.est === 'trap' ? 0.5 * (dens + dens_last) : dens;
    const od = SIGMA * dens_i * dt;
    const a = 1 - Math.exp(-od);
    tauHat += od;
    // shade at the sample from the estimator's own marched depth (what the
    // shader's sun march would see up to the truth of its own taps; here the
    // rind is a function of position to keep the comparison about the eye)
    const S = shade(shadeMode, SIGMA * cumDepth(rho, tm));
    Lacc += trans * a * S;
    trans *= (1 - a);
    tauDepth += od;
  }
  return { T: trans, L: Lacc, tauHat };
}
// cumulative depth for the shade term (cached per profile via closure below)
let _cum = null;
function cumDepth(rho, x) { return _cum(x); }
function buildCum(rho, L) {
  const n = 20000, dx = L / n; const arr = new Float64Array(n + 1); let s = 0;
  for (let i = 0; i < n; i++) { s += rho((i + 0.5) * dx) * dx; arr[i + 1] = s; }
  _cum = x => { const u = Math.min(Math.max(x / dx, 0), n); const i = Math.floor(u); return i >= n ? arr[n] : arr[i] + (arr[i + 1] - arr[i]) * (u - i); };
}

// average over jitters j and edge phases phi (edge position e1 = base + phi*h)
function expect(mkProfile, L, opts, shadeMode, NJ = 64, NP = 64) {
  let sT = 0, sT2 = 0, sL = 0, sL2 = 0, sTau = 0, n = 0, refT = 0, refL = 0, refTau = 0;
  for (let b = 0; b < NP; b++) {
    const phi = (b + 0.5) / NP;
    const rho = mkProfile(phi * opts.h);
    buildCum(rho, L);
    const r = reference(rho, L, shadeMode);
    refT += r.T; refL += r.L; refTau += r.tau;
    for (let a = 0; a < NJ; a++) {
      const j = (a + 0.5) / NJ;
      const m = march(rho, L, j, opts, shadeMode);
      sT += m.T; sT2 += m.T * m.T; sL += m.L; sL2 += m.L * m.L; sTau += m.tauHat; n++;
    }
  }
  const mT = sT / n, mL = sL / n;
  return { T: mT, sdT: Math.sqrt(Math.max(0, sT2 / n - mT * mT)), L: mL, sdL: Math.sqrt(Math.max(0, sL2 / n - mL * mL)), tau: sTau / n,
           refT: refT / NP, refL: refL / NP, refTau: refTau / NP };
}

const f3 = x => (x >= 0 ? ' ' : '') + x.toFixed(4);
function row(label, r) {
  return `${label.padEnd(34)} E[tau]=${f3(r.tau)} (ref ${f3(r.refTau)})  E[T]=${f3(r.T)} (ref ${f3(r.refT)}, bias ${f3(r.T - r.refT)}) sdT=${f3(r.sdT)}  E[L]=${f3(r.L)} (ref ${f3(r.refL)}, bias ${f3(r.L - r.refL)}) sdL=${f3(r.sdL)}`;
}

const HS = [23.2, 45, 94, 188, 375, 522, 928];
const L = 6000; // ray length, m
const base = 800; // edge base position

function sweep(title, mk, estOpts, shadeMode) {
  console.log(`\n=== ${title}  [est=${estOpts.est} backtrack=${estOpts.backtrack} mfp=${estOpts.mfp} shade=${shadeMode}] ===`);
  for (const h of HS) {
    const r = expect(off => mk(base + off), L, { ...estOpts, h }, shadeMode);
    console.log(row(`h=${h}`, r));
  }
}

const which = process.argv[2] || 'all';
const FULL = { est: 'trap', backtrack: true, mfp: true };
const POINT = { est: 'point', backtrack: false, mfp: false };
const TRAPONLY = { est: 'trap', backtrack: false, mfp: false };

if (which === 'all' || which === 'ideal') {
  // Part 1: the ideal jittered Riemann estimator (no refinement) on a cliff.
  sweep('CLIFF D=1 thick=2000, IDEAL point estimator', e => cliff(1, e, 2000), POINT, 'flat');
  sweep('CLIFF D=0.1 thick=2000, IDEAL point estimator', e => cliff(0.1, e, 2000), POINT, 'flat');
  sweep('CLIFF D=0.03 thick=2000, IDEAL point estimator', e => cliff(0.03, e, 2000), POINT, 'flat');
  sweep('CLIFF D=0.1 thick=2000, trapezoid+skip, no refine', e => cliff(0.1, e, 2000), TRAPONLY, 'flat');
  sweep('RAMP w=520 D=0.1 thick=2000, IDEAL point', e => ramp(0.1, e, 520, 2000), POINT, 'flat');
  sweep('RAMP w=520 D=0.1 thick=2000, trapezoid+skip', e => ramp(0.1, e, 520, 2000), TRAPONLY, 'flat');
}
if (which === 'all' || which === 'shader') {
  // Part 2: the shipped estimator.
  for (const D of [1, 0.3, 0.1, 0.05, 0.03]) {
    sweep(`CLIFF D=${D} thick=2000, SHIPPED law`, e => cliff(D, e, 2000), FULL, 'rind');
  }
  for (const w of [23, 90, 250, 520, 1000, 1800]) {
    sweep(`RAMP w=${w} D=1 thick=3000, SHIPPED law`, e => ramp(1, e, w, 3000), FULL, 'rind');
  }
  sweep('NOISE-PATH SKIRT (1.8 km toe) thick=4000, SHIPPED', e => noiseSkirt(e, 4000), FULL, 'rind');
  sweep('SMOOTHSTEP RAMP w=520 D=1, SHIPPED', e => sramp(1, e, 520, 3000), FULL, 'rind');
  sweep('CONSTRUCTED 9 m ramp D=1 thick=1500, SHIPPED', e => ramp(1, e, 9, 1500), FULL, 'rind');
  sweep('CONSTRUCTED 90 m ramp D=1 thick=1500, SHIPPED', e => ramp(1, e, 90, 1500), FULL, 'rind');
}
