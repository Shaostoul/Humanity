// Pin present_dither:"0" into the showcase of the high-frequency gates'
// vantages in tests/visual/vantages.json, by targeted text edits (the file
// mixes one-line and multi-line objects, so a JSON rewrite would churn it).
const fs = require('fs');
const p = 'C:/Humanity/.claude/worktrees/agent-a5e6f9189e45ed9b9/tests/visual/vantages.json';
let t = fs.readFileSync(p, 'utf8');
const crlf = t.includes('\r\n');
t = t.replace(/\r\n/g, '\n');
const ids = [
  // glint-autocorr.mjs
  'ocean-grazing-calm', 'ocean-storm-glitter', 'ocean-glint-150km', 'shore-limb-0630',
  // blackline-census.mjs
  'blackline-clouds-off',
  // speckle-census.js, cloud-speckle-census.js, cloud-grain-metric.js
  'approach-2000km-high', 'approach-2000km-ultra',
  'orbit-2000-high-mask', 'orbit-2000-high-nolight', 'orbit-2000-high-nodither', 'orbit-2000-high-notemporal',
  'orbit-2000-high-sun', 'orbit-2000-high-amb', 'orbit-2000-low-tier', 'silverdale-flight-2km',
  'orbit-speckle-873-r1', 'orbit-speckle-873-r1-mask', 'orbit-speckle-873-r1-off',
  'orbit-speckle-873-r4', 'orbit-speckle-873-r4-mask', 'orbit-speckle-873-r4-off',
  'prof-polar-873-r1', 'prof-polar-873-r4', 'prof-stepinv-1000-r1-eco0-mask', 'prof-stepinv-1000-r1-eco1-mask',
  'orbit-tier-873-high-r1-mask', 'orbit-tier-873-high-r4-mask', 'cloudres-full', 'cloudres-half', 'cloudres-nodither',
  'deck-55-equator',
  // terminator-grain.js
  'orbit-terminator-3000km', 'orbit-terminator-3000km-b', 'orbit-terminator-3000km-noms', 'cloudlum-term-high',
];
const parsed = JSON.parse(t);
for (const id of ids) {
  const v = parsed.vantages.find((x) => x.id === id);
  if (!v) throw new Error('no vantage ' + id);
  if (!v.showcase) throw new Error('no showcase block in ' + id);
  if (v.showcase.present_dither !== undefined) { console.log('already', id); continue; }
  const at = t.indexOf(`"id": "${id}"`);
  if (at < 0 || t.indexOf(`"id": "${id}"`, at + 1) >= 0) throw new Error('id text not unique: ' + id);
  const next = t.indexOf('"id": "', at + 5);
  const sc = t.indexOf('"showcase": {', at);
  if (sc < 0 || (next >= 0 && sc > next)) throw new Error('showcase not found for ' + id);
  const brace = sc + '"showcase": {'.length;
  if (t[brace] === '\n') {
    const lineEnd = t.indexOf('\n', brace + 1);
    const nextLine = t.slice(brace + 1, lineEnd);
    const indent = nextLine.match(/^\s*/)[0];
    t = t.slice(0, brace + 1) + indent + '"present_dither": "0",\n' + t.slice(brace + 1);
  } else if (t[brace] === '}') {
    t = t.slice(0, brace) + '"present_dither":"0"' + t.slice(brace);
  } else {
    t = t.slice(0, brace) + '"present_dither":"0",' + t.slice(brace);
  }
}
// Top-level note next to the other underscore notes.
const anchor = '  "_wind_note": ';
if (t.indexOf(anchor) < 0) throw new Error('no _wind_note');
const note = '  "_dither_note": "HDR scene target increment 4 (2026-09-27, docs/design/hdr-scene-target.md) put ONE triangular dither of about one code in the present pass, before the 8-bit write. Metrics that read pixel-to-pixel differences would read that grain instead of the scene, so the vantages of the high-frequency gates (glint-autocorr.mjs, blackline-census.mjs, speckle-census.js, cloud-speckle-census.js, cloud-grain-metric.js, terminator-grain.js) pin {\\"present_dither\\":\\"0\\"}; the rigs reset it to on for every cell that does not pin it. Mean-based metrics (aurora-gate.js, night-side-mean.js, measure-sky.mjs, cloud-brightness.js) are unmoved by it and do not pin it. aurora-comb.js is left dithered on purpose: its baselines were taken with the aurora\'s own dither of the same size, so an undithered aurora would move its baseline instead.",\n';
t = t.replace(anchor, note + anchor);
JSON.parse(t); // still valid
const after = JSON.parse(t);
let n = 0;
for (const id of ids) if (after.vantages.find((x) => x.id === id).showcase.present_dither === '0') n++;
if (crlf) t = t.replace(/\n/g, '\r\n');
fs.writeFileSync(p, t);
console.log('pinned', n, 'of', ids.length);
