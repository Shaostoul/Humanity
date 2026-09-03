// Targeted runs: (1) backtrack one-step-lag loss law vs h on a dense cliff,
// (2) camera INSIDE a cloud: the i==0 rewind samples behind the eye.
const SIGMA = 0.045, FLOOR = 23.2, GATE = 0.02, TAU_MAX = 0.75;
function march(rho, L, j, h, useTrap, backtrack, mfp) {
  let t_cur = 0, dens_prev = 0, tauHat = 0, tauBehind = 0, minT = 0, iters = 0;
  for (let i = 0; i < 4000; i++) {
    if (t_cur >= L) break; iters++;
    let dt = Math.min(h, L - t_cur);
    if (mfp && dens_prev > GATE) dt = Math.min(dt, Math.max(TAU_MAX / (SIGMA * dens_prev), FLOOR));
    const tm = t_cur + dt * j;
    if (i === 0) t_cur += dt * Math.max(j, 1e-3); else t_cur += dt;
    const dens = rho(tm);
    if (backtrack && dens > GATE && dens_prev <= GATE && SIGMA * dens * dt > TAU_MAX) { t_cur -= dt; dens_prev = dens; minT = Math.min(minT, t_cur); continue; }
    const dl = dens_prev; dens_prev = dens;
    if (dens <= 0.001) continue;
    const od = SIGMA * (useTrap ? 0.5 * (dens + dl) : dens) * dt;
    tauHat += od; if (tm < 0) tauBehind += od;
  }
  return { tauHat, tauBehind, minT, iters };
}
function avg(rhoOf, L, h, NJ = 200, NP = 200, ...rest) {
  let s = 0, sb = 0, smin = 0, si = 0, n = 0;
  for (let b = 0; b < NP; b++) { const rho = rhoOf((b + 0.5) / NP * h);
    for (let a = 0; a < NJ; a++) { const m = march(rho, L, (a + 0.5) / NJ, h, ...rest); s += m.tauHat; sb += m.tauBehind; smin += m.minT; si += m.iters; n++; } }
  return { tau: s / n, behind: sb / n, minT: smin / n, iters: si / n };
}
console.log('(1) dense cliff D=1, 2000 m thick, shipped law: lost path vs h (prediction h/6)');
for (const h of [23.2, 45, 94, 188, 375, 522, 928]) {
  const r = avg(e => x => (x >= 800 + e && x < 2800 + e) ? 1 : 0, 6000, h, 200, 200, true, true, true);
  console.log(`h=${String(h).padEnd(6)} E[tau]=${r.tau.toFixed(3)} ref 90.000  lost path=${((90 - r.tau) / SIGMA).toFixed(1)} m  h/6=${(h / 6).toFixed(1)}  iters=${r.iters.toFixed(1)}`);
}
console.log('\n(1b) same, POINT estimator (no trapezoid) with backtrack+mfp: is the loss the lag or the trapezoid?');
for (const h of [188, 522, 928]) {
  const r = avg(e => x => (x >= 800 + e && x < 2800 + e) ? 1 : 0, 6000, h, 200, 200, false, true, true);
  console.log(`h=${String(h).padEnd(6)} E[tau]=${r.tau.toFixed(3)}  lost path=${((90 - r.tau) / SIGMA).toFixed(1)} m`);
}
console.log('\n(1c) same, trapezoid, NO backtrack (mfp on): the trapezoid-only entry bias');
for (const h of [188, 522, 928]) {
  const r = avg(e => x => (x >= 800 + e && x < 2800 + e) ? 1 : 0, 6000, h, 200, 200, true, false, true);
  console.log(`h=${String(h).padEnd(6)} E[tau]=${r.tau.toFixed(3)}  lost path=${((90 - r.tau) / SIGMA).toFixed(1)} m`);
}
console.log('\n(2) camera INSIDE uniform cloud (rho=D everywhere incl. behind the eye), ray 3000 m: optical depth accumulated from BEHIND the camera');
for (const D of [1, 0.3, 0.1]) for (const h of [188, 375, 522]) {
  const r = avg(e => x => D, 3000, h, 200, 4, true, true, true);
  console.log(`D=${D} h=${String(h).padEnd(4)} E[tau]=${r.tau.toFixed(2)} ref ${(SIGMA * D * 3000).toFixed(2)}  from behind eye: tau=${r.behind.toFixed(2)} = ${(r.behind / (SIGMA * D)).toFixed(0)} m of path  mean rewind=${(-r.minT).toFixed(0)} m (h/2=${(h / 2).toFixed(0)})`);
}
