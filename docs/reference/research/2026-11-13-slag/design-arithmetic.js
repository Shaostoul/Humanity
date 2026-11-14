// Slag design arithmetic (2026-11-13) for docs/design/slag.md. Run: node design-arithmetic.js
// Inputs are named with their source ids (F = docs/reference/findings/2026-11-13-slag.md,
// A/B/C = the helper notes in this folder). Ship inputs come from the master plan and
// `node scripts/space-program.js` (Drum 1's 3.53 km2 hull and 7,781 people on 2026-11-13).
const hull = 3.53e6;          // m2, Drum 1 hull (space-program.js, 2026-11-13)
const floor = 2 * Math.PI * 250 * 2000; // m2, Drum 1 floor (250 m radius, 2,000 m long)
const water = 10 * hull / 1e3; // kt -> use t
const shieldT = 10 * hull;     // t
const steelT = 1.37 * hull;    // t (17.5 cm steel, active-shielding research)
const soilT = 2.6 * floor;     // t (soil and buildings, sizes research 6.2, judgement)
const base = shieldT + steelT + soilT;
const people = 7781;
console.log('floor km2', (floor / 1e6).toFixed(2), 'shield Mt', (shieldT / 1e6).toFixed(1), 'steel Mt', (steelT / 1e6).toFixed(2), 'soil+bldg Mt', (soilT / 1e6).toFixed(2), 'base Mt', (base / 1e6).toFixed(1));
// vote table
const wSlope = 0.61, rSlope = 0.65; // dose factor per t/m2 at depth (water 6->7: 16.1/26.5; regolith 9->11)
for (const d of [0, 1, 2, 3, 5, 10]) {
  const add = d * hull, m = (base + add) / base;
  const opt = 5 * Math.pow(rSlope, d), pes = 5 * Math.pow(Math.pow(wSlope, 1 / 2.1), d);
  console.log(`d=${d} add ${(add / 1e6).toFixed(1)} Mt  mass x${m.toFixed(3)} (+${((m - 1) * 100).toFixed(0)}%) turn x${Math.sqrt(m).toFixed(3)} (+${((Math.sqrt(m) - 1) * 100).toFixed(0)}%)  dose ${opt.toFixed(2)}-${pes.toFixed(2)} mSv  t/person ${(add / people).toFixed(0)}  shell load at 0.05g ${(d * 1000 * 9.81 * 0.05 / 1000).toFixed(2)} kN/m2`);
}
// propellant per 1 m/s for the base ship
for (const [name, ve] of [['steam 192 s', 192 * 9.80665], ['mass driver 8 km/s', 8000], ['ion 4190 s', 4190 * 9.80665]]) {
  const dm = base * (1 - Math.exp(-1 / ve));
  console.log(name, 'per 1 m/s:', dm.toFixed(0), 't');
}
const mdE = 42.7e6; // J/kg electrical (O'Neill 1977, 560 N at 2.9 MW at 8 km/s)
const dm8 = base * (1 - Math.exp(-1 / 8000));
console.log('mass driver energy per m/s GWh', (dm8 * 1000 * mdE / 3.6e12).toFixed(0), 'days at 84 MW', (dm8 * 1000 * mdE / 84e6 / 86400).toFixed(0));
const thrown10MW = 10e6 * 3.15576e7 / mdE / 1000; // t/yr
console.log('10 MW mass driver throws t/yr', thrown10MW.toFixed(0), 'dv m/s/yr', (8000 * Math.log(base / (base - thrown10MW))).toFixed(2));
// micrometeoroids (Grun via SPENVIS, C1): >=1 mg 6.0e-4 /m2/yr; >=1 g 7.0e-8
console.log('hits/yr >=1 mg', (6.0e-4 * hull).toFixed(0), '>=1 g', (7.0e-8 * hull).toFixed(2));
// slag from Drum 1's own steel, smelted from stony ore (A14): 2.26-3.46 t/t fully reduced, 4.9-24 metal grains only
console.log('slag from Drum 1 steel Mt: full reduction H..LL', (steelT * 2.26 / 1e6).toFixed(1), (steelT * 3.46 / 1e6).toFixed(1), ' grains-only H', (steelT * 4.9 / 1e6).toFixed(1), ' as t/m2 over hull', (steelT * 2.26 / hull).toFixed(1), (steelT * 4.9 / hull).toFixed(1));
console.log('all-slag shield 13.4 t/m2 Mt', (13.4 * hull / 1e6).toFixed(1), ' per person t', (13.4 * hull / people).toFixed(0));
// water from carbonaceous rock: residue per t of water (A2: CI 16.6 wt% water-eq, C 4.13; CM 10.3, 2.32; Ross 5-20%)
for (const [n, w, c] of [['CI', 16.6, 4.13], ['CM', 10.3, 2.32], ['20%', 20, 3], ['5%', 5, 2]]) console.log(n, 'rock left per t water', ((100 - w - c) / w).toFixed(1));
console.log('Drum 1 shield water from CI/CM rock leaves Mt', (shieldT * (100 - 16.6 - 4.13) / 16.6 / 1e6).toFixed(0), (shieldT * (100 - 10.3 - 2.32) / 10.3 / 1e6).toFixed(0));
// balance 1-2% of spinning mass
const spin = steelT + soilT;
console.log('balance mass t', (0.01 * spin).toFixed(0), (0.02 * spin).toFixed(0));
// soil one-time (C25): 0.216-0.358 t/m2 at 0.3 m, 0.6 at 0.5 m; top-up 0.1-4 kg/m2/yr
console.log('soil whole floor Mt', (0.216 * floor / 1e6).toFixed(2), (0.358 * floor / 1e6).toFixed(2), 'top-up t/yr', (0.1e-3 * floor).toFixed(0), (4e-3 * floor).toFixed(0));
// P filter: excreta P 0.44-0.61 kg/person/yr (closed-loop-test 6.6), 2.15 g P/kg slag (C27)
console.log('P filter slag t/yr Drum 1', (0.44 / 2.15e-3 * people / 1000).toFixed(0), (0.61 / 2.15e-3 * people / 1000).toFixed(0), 'kg/person', (0.44 / 2.15e-3).toFixed(0), (0.61 / 2.15e-3).toFixed(0));
// MRE: 50-125 kWh/kg O2 (A19/A20), 0.15-0.375 kg O2 per kg feed (A21 review)
const lo = 50 * 0.15 * 1000 / 1000, hi = 125 * 0.375 * 1000 / 1000; // MWh per t feed
console.log('MRE MWh per t slag', lo.toFixed(1), hi.toFixed(1), ' 10 MW plant t/yr', (87660 / hi).toFixed(0), (87660 / lo).toFixed(0));
// heat store: 0.13 MWh/t (C18)
console.log('heat store t per 100 MWh', (100 / 0.13).toFixed(0));
// asteroid slag indices (A17, H): SiO2 55.6 MgO 35.3 Al2O3 3.2 CaO 2.6
const s = { SiO2: 55.6, MgO: 35.3, Al2O3: 3.2, CaO: 2.6 };
console.log('basicity (CaO+MgO)/SiO2', ((s.CaO + s.MgO) / s.SiO2).toFixed(2), ' acidity (SiO2+Al2O3)/(CaO+MgO)', ((s.SiO2 + s.Al2O3) / (s.CaO + s.MgO)).toFixed(2));
// player scale: smelt_iron 2 ore -> 1 ingot + 1 slag (1 kg); copper 2 ore -> 6 slag; hold 10 units
console.log('slag per full hold kg: iron', 5, 'copper', 30, ' trips per t/m2 over Drum 1 (iron..copper) millions', (hull * 1000 / 5 / 1e6).toFixed(0), (hull * 1000 / 30 / 1e6).toFixed(0));
// 1 km drum outer area
const a1k = 2 * Math.PI * 1000 * 1300 + 2 * Math.PI * 1000 * 1000;
console.log('1 km drum shell km2', (a1k / 1e6).toFixed(1), ' fleet 10bn (500,000 drums) t per t/m2', (a1k * 500000).toExponential(2));
