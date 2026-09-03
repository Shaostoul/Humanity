// Approach-distance sweep for the constructed-path SDF stride: shipped (lag)
// vs lag-corrected. Distance from the ray start to the surface stands in for
// 1/cos(theta) on a layered deck.
'use strict';
const fs = require('fs');
let src = fs.readFileSync(__dirname + '/march_twin2.js', 'utf8');
src = src.replace('function march(d, tauCol, h, j, sdf, cap) {\n  const L0 = 1500, m1 = 5000;',
  'function march(d, tauCol, h, j, sdf, cap, L0v) {\n  const L0 = L0v || 1500, m1 = 5000;');
src = src.replace(/^const HS[\s\S]*$/m, '');
src += '\nmodule.exports = { march, profile, tauTable, stats };\n';
fs.writeFileSync(__dirname + '/march_lib.js', src);
const { march, profile, tauTable, stats } = require('./march_lib.js');

function sweep(label, sdf) {
  console.log('\n' + label);
  const o = { W: 90, pw: 1, hashA: 46, hashLam: 6.6, turbAmp: 0.42, turbLam: 25 };
  const oT = { T: 400, W: 90, pw: 1, hashA: 46, hashLam: 6.6, turbAmp: 0.42, turbLam: 25 };
  for (const L0 of [400, 700, 1000, 1500, 2500, 4000, 6000]) {
    const R = [], F = [], B = [], IT = [];
    let truthR = 0;
    for (let s = 0; s < 6; s++) {
      const d = profile(o, s + 1); const tc = tauTable(d, -200, 5000, 0.25);
      truthR += march(d, tc, 0.5, 0.5, null, 1e6, L0).rad / 6;
      const dT = profile(oT, s + 1); const tcT = tauTable(dT, -200, 5000, 0.25);
      for (let k = 0; k < 96; k++) {
        const j = (k + 0.5) / 96;
        const r = march(d, tc, 188, j, sdf, undefined, L0); R.push(r.rad); F.push(Math.max(0, r.first)); IT.push(r.iters);
        const rt = march(dT, tcT, 188, j, sdf, undefined, L0); B.push(rt.body);
      }
    }
    const sr = stats(R), sf = stats(F), sb = stats(B);
    const missed = B.filter(b => b < 0.05).length / B.length;
    console.log(`  approach ${String(L0).padStart(4)} m: rad ${sr.mean.toFixed(3)} (truth ${truthR.toFixed(3)}) sd ${sr.sd.toFixed(3)} | first-hit depth mean ${sf.mean.toFixed(0)} m sd ${sf.sd.toFixed(0)} m | 400 m cloud: body ${sb.mean.toFixed(2)} sd ${sb.sd.toFixed(2)} missed ${(missed * 100).toFixed(0)}% | iters ${stats(IT).mean.toFixed(0)}`);
  }
}
sweep('=== C-sweep: SHIPPED constructed stride (lag), margin 311, refine 45; vs distance from ray start to surface ===', { margin: 311, refine: 45 });
sweep('=== D-sweep: lag-corrected stride ===', { margin: 311, refine: 45, fixLag: true });
sweep('=== C-sweep with the wide-edge margin 521 (rind still 90 m in the profile) ===', { margin: 521, refine: 45 });
