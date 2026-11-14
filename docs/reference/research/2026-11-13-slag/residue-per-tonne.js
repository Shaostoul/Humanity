// Derived figures for sl-A-notes.md. Inputs: Lodders 2021 Table 1 (ppm by mass),
// Vernazza et al. 2016 Table 1 (metal vol%), Ross 2001 Table 3 (densities), NASA 2026 MRE poster.
const L = {
  CI: {H:18600,C:41300,Na:5100,Mg:95170,Al:8370,Si:107740,P:978,S:53600,K:539,Ca:8840,Ti:450,Cr:2610,Mn:1896,Fe:185620,Co:508,Ni:10950,Pt:0.931,Ir:0.474},
  CM: {H:11500,C:23200,Na:4000,Mg:119000,Al:11400,Si:132000,P:980,S:30000,K:410,Ca:12000,Ti:610,Cr:3090,Mn:1700,Fe:211000,Co:560,Ni:12500,Pt:1.1,Ir:0.59},
  H:  {H:800,C:2350,Na:6100,Mg:139400,Al:11100,Si:170400,P:1190,S:20000,K:760,Ca:12400,Ti:630,Cr:3500,Mn:2340,Fe:271500,Co:805,Ni:17100,Pt:1.58,Ir:0.76},
  L:  {H:1260,C:2500,Na:6900,Mg:148600,Al:11700,Si:184600,P:1030,S:22000,K:880,Ca:13000,Ti:690,Cr:3660,Mn:2580,Fe:218450,Co:600,Ni:12700,Pt:1.08,Ir:0.52},
  LL: {H:1700,C:3920,Na:6900,Mg:153000,Al:11800,Si:189000,P:980,S:21000,K:860,Ca:13200,Ti:680,Cr:3680,Mn:2600,Fe:198000,Co:480,Ni:10200,Pt:0.88,Ir:0.36},
};
const f = {Si:60.084/28.086, Mg:40.304/24.305, Al:101.961/(2*26.982), Ca:56.077/40.078, Na:61.979/(2*22.990), K:94.196/(2*39.098), Cr:151.99/(2*51.996), Mn:70.937/54.938, Ti:79.866/47.867, P:141.94/(2*30.974)};
const name = {Si:'SiO2',Mg:'MgO',Al:'Al2O3',Ca:'CaO',Na:'Na2O',K:'K2O',Cr:'Cr2O3',Mn:'MnO',Ti:'TiO2',P:'P2O5'};
for (const [g,c] of Object.entries(L)) {
  const ox = {}; let sum = 0;
  for (const e of Object.keys(f)) { ox[name[e]] = c[e]/1e4*f[e]; sum += ox[name[e]]; }
  const metal = (c.Fe + c.Ni + c.Co)/1e4;
  const fes = c.S/1e4 * (87.91/32.06);
  const h2o = c.H/1e4 * (18.015/2.016);
  const slagComp = Object.fromEntries(Object.entries(ox).map(([k,v])=>[k,(100*v/sum).toFixed(1)]));
  console.log(g, 'non-Fe oxide sum wt%', sum.toFixed(1), '| all Fe+Ni+Co as metal wt%', metal.toFixed(1),
    '| slag t per t metal (all Fe reduced)', (sum/metal).toFixed(2), '| FeS if all S', fes.toFixed(1), '| H2O-equiv', h2o.toFixed(1), '| C wt%', (c.C/1e4).toFixed(2));
  console.log('   slag normalised wt%:', JSON.stringify(slagComp));
  console.log('   bulk Pt ppm', c.Pt, 'Ir', c.Ir, '| C mol/100 g', (c.C/1e4/12.011).toFixed(3), 'Fe mol/100 g', (c.Fe/1e4/55.845).toFixed(3));
}
const rhoM = (7.0+7.8)/2, rhoS = (3.5+3.8)/2;
for (const [g,v,pt] of [['H',8.4,1.58],['L',4.1,1.08],['LL',2,0.88]]) {
  const w = v*rhoM/rhoS;
  console.log(g, 'metal vol%', v, '-> wt%', w.toFixed(1), '-> residue t per t metal', ((100-w)/w).toFixed(1), '| Pt in metal if all Pt there (ppm)', (pt*100/w).toFixed(1));
}
console.log('Ross S-type 20% metal -> residue t/t', (80/20).toFixed(1), '; Ross M-type 98.5% free metal -> residue kg/t', (1.5/98.5*1000).toFixed(0), '; 99% ->', (1/99*1000).toFixed(0));
console.log('CI free metal 0.1% (Ross) -> residue t/t', (99.9/0.1).toFixed(0));
console.log('NASA 2026 MRE: kWh per kg O2 =', (1000/(0.3*31.998)).toFixed(0));
console.log('NASA 2026 MRE: 45 V * 210 A / 0.07 kg/h =', (45*210/1000/0.07).toFixed(0), 'kWh/kg');
console.log('Schreiner thesis: 14 kW / 1000 kg/yr =', (14*8766/1000).toFixed(0), 'kWh/kg; 56.5 kW / 10000 =', (56.5*8766/10000).toFixed(1));
const Ah = 1000/31.998*4*96485/3600;
console.log('Faraday charge per kg O2 (Ah):', Ah.toFixed(0), '; kWh per kg per volt at 100% CE:', (Ah/1000).toFixed(2));
for (const g of [1.45,10.77,13.42,15.79]) console.log('carbothermal', g, 'g/kWh ->', (1000/g).toFixed(0), 'kWh/kg');
console.log('Guerrero-Gonzalez & Zabel: 311.34 kW over (25+23.9) t/yr =', (311.34*8766/48900).toFixed(0), 'kWh per kg product; per kg O2 =', (311.34*8766/23900).toFixed(0));
