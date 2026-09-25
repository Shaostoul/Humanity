// Usage (from the repo root): node docs/reference/research/2026-09-24-fire-performance/tables.js docs/reference/research/2026-09-24-fire-performance/alloys-final.json > docs/reference/research/2026-09-24-fire-performance/tables.md
// alloys.json: [{name, basis, density_g_cc, modulus_gpa, yield_mpa, thermal_conductivity_w_mk,
//   baseline?:true (one row, 6061-T6), sweep?:true (rows to show in the wall-thickness sweep),
//   yield_bands?:[{min_in, max_in, yield_mpa}] (a minimum that depends on wall; walls outside every band use yield_mpa),
//   footnote?:string (printed under the tables, with a [n] marker after the row name),
//   alts?:[{label, yield_mpa}] (alternative yields; their bending strength at 0.040 and 0.065 in is computed into the footnote)}]
// Computes, for a 60 in staff tube at 0.75 in OD and several walls:
//   weight (g), bending stiffness EI relative to a 3/4 x 0.065 in 6061-T6 tube,
//   bending strength (yield moment, sigma_y * I / c) relative to the same baseline,
//   a rough dent-resistance index (sigma_y * t^2, ring-crush scaling) relative to baseline,
//   heat carried along the tube (k * A) relative to baseline.
const fs = require('fs');
const alloys = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));
const IN = 0.0254, L = 60 * IN, OD = 0.75 * IN;
const WALLS_IN = [0.028, 0.035, 0.040, 0.049, 0.058, 0.065, 0.083];
function tube(t_in) {
  const t = t_in * IN, ID = OD - 2 * t;
  const A = Math.PI / 4 * (OD * OD - ID * ID);          // m^2
  const I = Math.PI / 64 * (OD ** 4 - ID ** 4);          // m^4
  return { t, A, I, c: OD / 2 };
}
// Yield minimum for this wall: a matching band if the alloy has wall bands, else the single value.
function yieldAt(a, t_in) {
  for (const b of a.yield_bands || []) if (t_in >= b.min_in - 1e-9 && t_in <= b.max_in + 1e-9) return b.yield_mpa;
  return a.yield_mpa;
}
function calc(a, t_in, yOverride) {
  const g = tube(t_in);
  const y = yOverride ?? yieldAt(a, t_in);
  return {
    mass_g: a.density_g_cc * 1000 * g.A * L * 1000,      // density kg/m3 * m3 -> kg -> g
    EI: a.modulus_gpa * 1e9 * g.I,
    My: y * 1e6 * g.I / g.c,
    dent: y * g.t * g.t,
    heat: (a.thermal_conductivity_w_mk || NaN) * g.A,
  };
}
const base = alloys.find(a => a.baseline);
if (!base) throw new Error('need exactly one entry with baseline:true (6061-T6 drawn tube)');
const B = calc(base, 0.065);
const r = (x, y) => (x / y).toFixed(2);
// Footnote numbering follows row order; the marker goes after the name in every table.
const fn = new Map();
for (const a of alloys) if (a.footnote || (a.alts && a.alts.length)) fn.set(a, fn.size + 1);
const label = a => fn.has(a) ? `${a.name} [${fn.get(a)}]` : a.name;
let out = '';
out += `Baseline for all ratios: 6061-T6 aluminium, 60 in x 0.75 in OD x 0.065 in wall (${B.mass_g.toFixed(0)} g).\n\n`;
out += `## Same tube (60 in x 0.75 in OD x 0.040 in wall), every alloy\n\n`;
out += `| Alloy | Strength basis | Weight g (lb) | Stiffness | Bending strength | Dent index | Heat carried |\n|---|---|---|---|---|---|---|\n`;
for (const a of alloys) {
  const c = calc(a, 0.040);
  out += `| ${label(a)} | ${a.basis || "?"} | ${c.mass_g.toFixed(0)} (${(c.mass_g / 453.592).toFixed(2)}) | ${r(c.EI, B.EI)} | ${r(c.My, B.My)} | ${r(c.dent, B.dent)} | ${isNaN(c.heat) ? 'n/a' : r(c.heat, B.heat)} |\n`;
}
out += `\n## Same tube (60 in x 0.75 in OD x 0.065 in wall), every alloy\n\n`;
out += `At this wall the dent index equals the bending strength for every alloy (both reduce to yield strength / 241 MPa), so it is not repeated here.\n\n`;
out += `| Alloy | Strength basis | Weight g (lb) | Stiffness | Bending strength |\n|---|---|---|---|---|\n`;
for (const a of alloys) {
  const c = calc(a, 0.065);
  out += `| ${label(a)} | ${a.basis || "?"} | ${c.mass_g.toFixed(0)} (${(c.mass_g / 453.592).toFixed(2)}) | ${r(c.EI, B.EI)} | ${r(c.My, B.My)} |\n`;
}
const pick = alloys.filter(a => a.sweep);
out += `\n## Wall thickness sweep (60 in x 0.75 in OD)\n\n`;
for (const a of pick) {
  out += `### ${label(a)} (${a.basis || "?"})\n\n| Wall in (mm) | ID in | Weight g (lb) | Stiffness | Bending strength | Dent index |\n|---|---|---|---|---|---|\n`;
  for (const w of WALLS_IN) {
    const c = calc(a, w);
    out += `| ${w.toFixed(3)} (${(w * 25.4).toFixed(2)}) | ${(0.75 - 2 * w).toFixed(3)} | ${c.mass_g.toFixed(0)} (${(c.mass_g / 453.592).toFixed(2)}) | ${r(c.EI, B.EI)} | ${r(c.My, B.My)} | ${r(c.dent, B.dent)} |\n`;
  }
  out += '\n';
}
out += `Notes: stiffness = how much it flexes (higher = stiffer). Bending strength = load before it takes a permanent bend (yield). Dent index = yield strength x wall squared, a rough ring-crush scaling for denting on a drop, useful only for comparing, not a prediction. Heat carried = thermal conductivity x metal cross-section (how much heat the tube walls conduct toward the hands).\n\n`;
out += `Strength basis: "tube min" = a published specification minimum for tube in that alloy and condition. "sheet min", "strip min", "bar min" and "bar/plate min" = the specification minimum for another product form, used because no tube minimum was found. "typical" = a typical value, not a guaranteed minimum, so it reads high next to the minimum-based rows.\n`;
if (fn.size) {
  out += `\nRow notes:\n\n`;
  for (const [a, n] of fn) {
    let s = `[${n}] ${a.name}: ${a.footnote || ''}`.trimEnd();
    for (const alt of a.alts || []) {
      const c40 = calc(a, 0.040, alt.yield_mpa), c65 = calc(a, 0.065, alt.yield_mpa);
      s += ` With ${alt.label}: bending strength ${r(c40.My, B.My)} at 0.040 in and ${r(c65.My, B.My)} at 0.065 in (dent index ${r(c40.dent, B.dent)} at 0.040 in).`;
    }
    out += `${s}\n\n`;
  }
}
process.stdout.write(out.replace(/\n+$/, '\n'));
