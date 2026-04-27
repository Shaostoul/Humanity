#!/usr/bin/env node
/**
 * gen-theme-presets-js.js
 *
 * Reads data/themes/presets.json (canonical) and rewrites the AUTO-GENERATED
 * block of `ACCENT_PRESETS`, `FONT_SIZES`, and `THEMES` inside
 * web/shared/settings.js so the web UI stays in sync.
 *
 * Usage:
 *   node scripts/gen-theme-presets-js.js
 *
 * Idempotent — running twice produces no diff. See docs/design/infinite-of-x.md.
 */

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const SRC = path.join(ROOT, 'data', 'themes', 'presets.json');
const DST = path.join(ROOT, 'web', 'shared', 'settings.js');
const BEGIN = '// AUTO-GENERATED FROM data/themes/presets.json — do not edit by hand.';
const END   = '// END AUTO-GENERATED.';

function indent(text, prefix) {
  return text.split('\n').map(line => line.length ? prefix + line : line).join('\n');
}

function fmtAccent(p) {
  return `    { name: ${JSON.stringify(p.name)}, color: ${JSON.stringify(p.color)} }`;
}
function fmtFont(f) {
  return `    { label: ${JSON.stringify(f.label)}, value: ${JSON.stringify(f.value)}, size: ${JSON.stringify(f.size)} }`;
}
function fmtThemeKey(name, vars) {
  const lines = Object.entries(vars).map(([k, v]) => `      ${JSON.stringify(k)}: ${JSON.stringify(v)}`);
  return `    ${name}: {\n${lines.join(',\n')}\n    }`;
}

const data = JSON.parse(fs.readFileSync(SRC, 'utf8'));

const accentLines = data.accent_presets.map(fmtAccent).join(',\n');
const fontLines   = data.font_sizes.map(fmtFont).join(',\n');
const themeLines  = Object.entries(data.themes).map(([k, v]) => fmtThemeKey(k, v)).join(',\n');

const generated =
`${BEGIN}
  const ACCENT_PRESETS = [
${accentLines}
  ];

  const FONT_SIZES = [
${fontLines}
  ];

  const THEMES = {
${themeLines}
  };
  ${END}`;

const original = fs.readFileSync(DST, 'utf8');
const startIdx = original.indexOf(BEGIN);
const endIdx   = original.indexOf(END);

let updated;
if (startIdx === -1 || endIdx === -1) {
  // First run: locate the legacy hand-written block (starts with `const ACCENT_PRESETS = [`).
  const legacyRe = /\s*const ACCENT_PRESETS = \[[\s\S]*?const THEMES = \{[\s\S]*?\};\n/;
  const m = original.match(legacyRe);
  if (!m) {
    console.error(`Could not find ACCENT_PRESETS/FONT_SIZES/THEMES block to replace in ${DST}.`);
    console.error('Either add the AUTO-GENERATED markers manually, or restore the legacy hand-written block.');
    process.exit(1);
  }
  // Preserve the leading newline + 2-space indent before our marker.
  updated = original.replace(legacyRe, `\n  ${generated}\n`);
} else {
  // Replace existing AUTO-GENERATED block.
  const before = original.slice(0, startIdx);
  const after  = original.slice(endIdx + END.length);
  updated = before + generated + after;
}

if (updated !== original) {
  fs.writeFileSync(DST, updated, 'utf8');
  console.log(`Wrote ${DST}`);
} else {
  console.log(`No change: ${DST}`);
}
