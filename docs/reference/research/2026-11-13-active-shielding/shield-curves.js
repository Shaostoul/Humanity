// Draws shield-curves.svg from shield-curves.json (the cited points), two charts stacked.
// Usage (from the repo root):
//   node docs/reference/research/2026-11-13-active-shielding/shield-curves.js
// Reads the JSON beside this script and writes the SVG beside it. No dependencies.
//
// What is drawn, so a reader can trust it:
// - a marker for every point in the JSON (a value printed in the named source, or the ratio of two
//   values printed in the same source, as each series says);
// - a dashed line joining a series' markers: interpolation between read values, never beyond the
//   last one;
// - a dotted line for each "estimate" (labelled as an estimate in the legend);
// - the colours change with the viewer's light or dark setting (prefers-color-scheme).
const fs = require('fs');
const path = require('path');

const dir = __dirname;
const data = JSON.parse(fs.readFileSync(path.join(dir, 'shield-curves.json'), 'utf8'));

const W = 960;              // whole picture width
const ML = 78, MR = 24;     // plot margins, left and right
const PH = 330;             // plot height
const PW = W - ML - MR;     // plot width

// One style per series id: CSS class, marker shape.
const STYLE = {
  water: { cls: 's-water', marker: 'circle' },
  poly: { cls: 's-poly', marker: 'square' },
  regolith: { cls: 's-rock', marker: 'triangle' },
  leo600: { cls: 's-earth', marker: 'diamond' },
  leo500: { cls: 's-earth2', marker: 'circle-small' },
  earth: { cls: 's-earth', marker: 'diamond' },
  sr2s: { cls: 's-sr2s', marker: 'square' },
  hesse: { cls: 's-hesse', marker: 'diamond' },
};

const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const f = (n) => (Math.round(n * 10) / 10).toString();

function marker(shape, x, y, cls) {
  switch (shape) {
    case 'square': return `<rect class="${cls} mk" x="${f(x - 4)}" y="${f(y - 4)}" width="8" height="8"/>`;
    case 'triangle': return `<path class="${cls} mk" d="M${f(x)} ${f(y - 5)}L${f(x + 5)} ${f(y + 4)}L${f(x - 5)} ${f(y + 4)}Z"/>`;
    case 'diamond': return `<path class="${cls} mk" d="M${f(x)} ${f(y - 5.5)}L${f(x + 5.5)} ${f(y)}L${f(x)} ${f(y + 5.5)}L${f(x - 5.5)} ${f(y)}Z"/>`;
    case 'circle-small': return `<circle class="${cls} mk" cx="${f(x)}" cy="${f(y)}" r="2.8"/>`;
    default: return `<circle class="${cls} mk" cx="${f(x)}" cy="${f(y)}" r="4.2"/>`;
  }
}

function chart(c, top) {
  const out = [];
  const pt = top + 46;                // plot top
  const X = (x) => ML + (x - c.x_min) / (c.x_max - c.x_min) * PW;
  let Y;
  if (c.y_log) {
    const a = Math.log10(c.y_min), b = Math.log10(c.y_max);
    Y = (v) => pt + (b - Math.log10(v)) / (b - a) * PH;
  } else {
    Y = (v) => pt + (c.y_max - v) / (c.y_max - c.y_min) * PH;
  }
  out.push(`<text class="title" x="${ML}" y="${top + 22}">${esc(c.title)}</text>`);
  out.push(`<rect class="plot" x="${ML}" y="${pt}" width="${PW}" height="${PH}"/>`);
  // Grid and ticks.
  const yt = c.y_log ? [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000] : [];
  if (!c.y_log) for (let v = c.y_min; v <= c.y_max + 1e-9; v += c.y_step) yt.push(v);
  for (const v of yt) {
    if (v < c.y_min || v > c.y_max) continue;
    out.push(`<line class="grid" x1="${ML}" x2="${ML + PW}" y1="${f(Y(v))}" y2="${f(Y(v))}"/>`);
    out.push(`<text class="tick" x="${ML - 8}" y="${f(Y(v) + 4)}" text-anchor="end">${v}</text>`);
  }
  for (let x = c.x_min; x <= c.x_max + 1e-9; x += c.x_step) {
    out.push(`<line class="grid" x1="${f(X(x))}" x2="${f(X(x))}" y1="${pt}" y2="${pt + PH}"/>`);
    out.push(`<text class="tick" x="${f(X(x))}" y="${pt + PH + 18}" text-anchor="middle">${x}</text>`);
  }
  out.push(`<text class="axis" x="${ML + PW / 2}" y="${pt + PH + 40}" text-anchor="middle">${esc(c.x_label)}</text>`);
  out.push(`<text class="axis" transform="translate(${ML - 54} ${pt + PH / 2}) rotate(-90)" text-anchor="middle">${esc(c.y_label)}</text>`);
  // Reference lines.
  for (const h of c.hlines) {
    const cls = h.y === 5 ? 'ref-target' : 'ref-limit';
    out.push(`<line class="${cls}" x1="${ML}" x2="${ML + PW}" y1="${f(Y(h.y))}" y2="${f(Y(h.y))}"/>`);
    out.push(`<text class="reflabel ${cls}-t" x="${ML + PW - 6}" y="${f(Y(h.y) - 5)}" text-anchor="end">${esc(h.label)}</text>`);
  }
  for (const v of c.vlines) {
    out.push(`<line class="ref-decided" x1="${f(X(v.x))}" x2="${f(X(v.x))}" y1="${pt}" y2="${pt + PH}"/>`);
    out.push(`<text class="reflabel ref-decided-t" x="${f(X(v.x) + 6)}" y="${pt + 16}">${esc(v.label)}</text>`);
  }
  // Estimates (dotted), drawn under the read series.
  for (const e of c.estimates) {
    const pts = [];
    for (let x = e.from[0]; x <= e.to_x + 1e-9; x += 0.25) pts.push([x, e.from[1] * Math.pow(e.factor_per_t, x - e.from[0])]);
    out.push(`<polyline class="est" points="${pts.map(([x, y]) => `${f(X(x))},${f(Y(y))}`).join(' ')}"/>`);
  }
  // Series: dashed interpolation, then markers.
  for (const s of c.series) {
    const st = STYLE[s.id];
    const pts = s.points.map((p) => [X(p[0]), Y(p[1])]);
    if (pts.length > 1) out.push(`<polyline class="${st.cls} interp" points="${pts.map(([x, y]) => `${f(x)},${f(y)}`).join(' ')}"/>`);
    for (const [x, y] of pts) out.push(marker(st.marker, x, y, st.cls));
  }
  // Legend: one row per series (label, then source in small type), then the estimate rows and the key.
  let ly = pt + PH + 66;
  for (const s of c.series) {
    const st = STYLE[s.id];
    out.push(`<line class="${st.cls} interp" x1="${ML}" x2="${ML + 26}" y1="${ly - 4}" y2="${ly - 4}"/>`);
    out.push(marker(st.marker, ML + 13, ly - 4, st.cls));
    out.push(`<text class="leg" x="${ML + 36}" y="${ly}">${esc(s.label)}</text>`);
    out.push(`<text class="src" x="${ML + 36}" y="${ly + 15}">${esc(s.source)}</text>`);
    ly += 36;
  }
  for (const e of c.estimates) {
    out.push(`<line class="est" x1="${ML}" x2="${ML + 26}" y1="${ly - 4}" y2="${ly - 4}"/>`);
    out.push(`<text class="leg" x="${ML + 36}" y="${ly}">${esc(e.label)}</text>`);
    ly += 24;
  }
  out.push(`<text class="src" x="${ML}" y="${ly}">Markers: values read from the source (chart 2: the ratio of two values printed in the same source). Dashed lines join them (interpolation only).</text>`);
  ly += 16;
  return { svg: out.join('\n'), bottom: ly };
}

const parts = [];
let top = 0;
for (const c of data.charts) {
  const r = chart(c, top);
  parts.push(r.svg);
  top = r.bottom + 34;
}
const notes = [
  'No liquid hydrogen or aluminium series: no table for them at these depths was found in a source read this pass (README, section 2).',
  'Deep-space values: OLTARIS effective dose (female body model), 17 June 1977 to 17 June 1978,',
  'a low-sunspot year the paper calls conservative (2008 to 2009 was lower still).',
];
for (const n of notes) { parts.push(`<text class="src" x="${ML}" y="${top}">${esc(n)}</text>`); top += 16; }
const H = Math.ceil(top + 14);

const css = `
svg { --bg:#ffffff; --fg:#1d232b; --muted:#56616e; --grid:#dfe4ea; --plot:#fbfcfd;
  --water:#1f6fb8; --poly:#2b8a4b; --rock:#a3561f; --earth:#7a3fa6; --earth2:#b58ad0;
  --sr2s:#c0392b; --hesse:#d07a00; --est:#6f8496; --target:#c0392b; --limit:#7d8894; --decided:#1d232b; }
@media (prefers-color-scheme: dark) {
  svg { --bg:#14181d; --fg:#e6eaef; --muted:#a3adb8; --grid:#2c333b; --plot:#181d23;
    --water:#6cb4ff; --poly:#6fd890; --rock:#e7a06a; --earth:#c79af0; --earth2:#e0c4f6;
    --sr2s:#ff8a7a; --hesse:#ffbe5c; --est:#9fb2c3; --target:#ff8a7a; --limit:#8f9aa6; --decided:#e6eaef; }
}
.bg { fill: var(--bg); }
.plot { fill: var(--plot); stroke: var(--muted); stroke-width: 1; }
.grid { stroke: var(--grid); stroke-width: 1; }
text { font-family: system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; fill: var(--fg); }
.title { font-size: 17px; font-weight: 600; }
.axis { font-size: 13px; fill: var(--muted); }
.tick { font-size: 12px; fill: var(--muted); }
.leg { font-size: 13px; }
.src { font-size: 11.5px; fill: var(--muted); }
.reflabel { font-size: 12px; font-weight: 600; }
.interp { fill: none; stroke-width: 1.8; stroke-dasharray: 6 4; }
.est { fill: none; stroke: var(--est); stroke-width: 2; stroke-dasharray: 1.5 4; stroke-linecap: round; }
.mk { stroke: var(--bg); stroke-width: 1; }
.ref-target { stroke: var(--target); stroke-width: 1.6; }
.ref-target-t { fill: var(--target); }
.ref-limit { stroke: var(--limit); stroke-width: 1.2; stroke-dasharray: 8 4; }
.ref-limit-t { fill: var(--limit); }
.ref-decided { stroke: var(--decided); stroke-width: 1.4; stroke-dasharray: 3 3; }
.ref-decided-t { fill: var(--decided); }
.s-water { stroke: var(--water); } .s-water.mk { fill: var(--water); }
.s-poly { stroke: var(--poly); } .s-poly.mk { fill: var(--poly); }
.s-rock { stroke: var(--rock); } .s-rock.mk { fill: var(--rock); }
.s-earth { stroke: var(--earth); } .s-earth.mk { fill: var(--earth); }
.s-earth2 { stroke: var(--earth2); } .s-earth2.mk { fill: var(--earth2); }
.s-sr2s { stroke: var(--sr2s); } .s-sr2s.mk { fill: var(--sr2s); }
.s-hesse { stroke: var(--hesse); } .s-hesse.mk { fill: var(--hesse); }
.mk { stroke: var(--bg); }
`;

const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" role="img" aria-labelledby="t d">
<title id="t">Radiation shield curves: dose against shielding mass, and what a magnetic field leaves</title>
<desc id="d">Generated by shield-curves.js from shield-curves.json; every marker is a cited value. Water, polyethylene and regolith in deep space and polyethylene under Earth's field (Globus and Strout 2017), and field-on over field-off ratios (Vuolo et al. 2016; Hesse-Withbroe and Arquilla 2025).</desc>
<style>${css}</style>
<rect class="bg" x="0" y="0" width="${W}" height="${H}"/>
${parts.join('\n')}
</svg>
`;
fs.writeFileSync(path.join(dir, 'shield-curves.svg'), svg);
console.log(`wrote shield-curves.svg (${W} x ${H})`);
