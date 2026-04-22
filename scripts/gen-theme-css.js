#!/usr/bin/env node
/**
 * gen-theme-css.js
 *
 * Reads data/gui/theme.ron (the native canonical theme) and rewrites the
 * CSS variables block inside web/shared/theme.css so the web frontend
 * stays visually aligned with the native desktop app.
 *
 * Usage:
 *   node scripts/gen-theme-css.js
 *
 * See docs/design/ui-system.md for the token-to-variable mapping.
 */

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const RON_PATH = path.join(ROOT, 'data', 'gui', 'theme.ron');
const CSS_PATH = path.join(ROOT, 'web', 'shared', 'theme.css');
const BEGIN = '/* =========================================================================\n * AUTO-GENERATED from data/gui/theme.ron — do not edit by hand.\n * Regenerate with: node scripts/gen-theme-css.js\n * ========================================================================= */';
const END = '/* ======================== END AUTO-GENERATED ======================== */';

/**
 * Parse the subset of RON used by theme.ron. Handles:
 *   field: (f, f, f, f)   // RGBA tuple
 *   field: 12.0           // float
 *   field: 42             // integer
 * Ignores comments (// to end of line).
 */
function parseTheme(src) {
  // Strip line comments
  const clean = src.replace(/\/\/.*$/gm, '');
  const tokens = {};

  // Match RGBA tuples
  const tupleRe = /(\w+)\s*:\s*\(\s*([\d.eE+-]+)\s*,\s*([\d.eE+-]+)\s*,\s*([\d.eE+-]+)\s*,\s*([\d.eE+-]+)\s*\)/g;
  let m;
  while ((m = tupleRe.exec(clean)) !== null) {
    tokens[m[1]] = {
      kind: 'rgba',
      r: parseFloat(m[2]),
      g: parseFloat(m[3]),
      b: parseFloat(m[4]),
      a: parseFloat(m[5]),
    };
  }

  // Strip tuples, then match scalars
  const noTuples = clean.replace(/\(\s*[\d.eE+\-\s,]+\s*\)/g, '()');
  const numRe = /(\w+)\s*:\s*([\d.eE+-]+)\s*[,)]/g;
  while ((m = numRe.exec(noTuples)) !== null) {
    if (tokens[m[1]]) continue;
    const v = parseFloat(m[2]);
    tokens[m[1]] = { kind: 'num', value: v };
  }

  return tokens;
}

function hex(n) {
  return Math.max(0, Math.min(255, Math.round(n))).toString(16).padStart(2, '0');
}

function toCss(tok) {
  if (!tok || tok.kind !== 'rgba') return null;
  const r = tok.r * 255;
  const g = tok.g * 255;
  const b = tok.b * 255;
  if (tok.a >= 0.999) {
    return '#' + hex(r) + hex(g) + hex(b);
  }
  return `rgba(${Math.round(r)}, ${Math.round(g)}, ${Math.round(b)}, ${tok.a.toFixed(3).replace(/\.?0+$/, '')})`;
}

function rgbTriple(tok) {
  return `${Math.round(tok.r * 255)}, ${Math.round(tok.g * 255)}, ${Math.round(tok.b * 255)}`;
}

function n(tok, fallback) {
  if (!tok || tok.kind !== 'num') return fallback;
  return tok.value;
}

function pxToRem(px) {
  const rem = px / 16;
  return `${Number.isInteger(rem * 10000) ? rem.toFixed(4).replace(/\.?0+$/, '') : rem.toFixed(4)}rem`;
}

function buildCss(tokens) {
  const t = tokens;
  const c = (k) => toCss(t[k]) || '#000';

  const spacing_xs = n(t.spacing_xs, 2);
  const spacing_sm = n(t.spacing_sm, 4);
  const spacing_md = n(t.spacing_md, 8);
  const spacing_lg = n(t.spacing_lg, 12);
  const spacing_xl = n(t.spacing_xl, 16);

  const font_small = n(t.font_size_small, 11);
  const font_body = n(t.font_size_body, 13);
  const font_heading = n(t.font_size_heading, 16);
  const font_title = n(t.font_size_title, 22);

  const lines = [];
  lines.push(BEGIN);
  lines.push(':root {');
  lines.push('  /* Backgrounds */');
  lines.push(`  --bg: ${c('bg_primary')};`);
  lines.push(`  --bg-secondary: ${c('bg_secondary')};`);
  lines.push(`  --bg-tertiary: ${c('bg_tertiary')};`);
  lines.push(`  --bg-card: ${c('bg_card')};`);
  lines.push(`  --bg-card-hover: ${c('bg_tertiary')};`);
  lines.push(`  --bg-input: ${c('bg_tertiary')};`);
  lines.push(`  --bg-hover: ${c('bg_tertiary')};`);
  lines.push(`  --bg-modal: ${c('bg_modal')};`);
  lines.push(`  --bg-panel: ${c('bg_panel')};`);
  lines.push(`  --bg-sidebar: ${c('bg_sidebar')};`);
  lines.push(`  --bg-sidebar-dark: ${c('bg_sidebar_dark')};`);
  lines.push('');
  lines.push('  /* Text */');
  lines.push(`  --text: ${c('text_primary')};`);
  lines.push(`  --text-secondary: ${c('text_secondary')};`);
  lines.push(`  --text-muted: ${c('text_muted')};`);
  lines.push(`  --text-on-accent: ${c('text_on_accent')};`);
  lines.push('');
  lines.push('  /* Accent */');
  lines.push(`  --accent: ${c('accent')};`);
  lines.push(`  --accent-hover: ${c('accent_hover')};`);
  lines.push(`  --accent-pressed: ${c('accent_pressed')};`);
  lines.push(`  --accent-dim: rgba(${rgbTriple(t.accent)}, 0.15);`);
  lines.push('');
  lines.push('  /* Semantic */');
  lines.push(`  --success: ${c('success')};`);
  lines.push(`  --warning: ${c('warning')};`);
  lines.push(`  --danger: ${c('danger')};`);
  lines.push(`  --info: ${c('info')};`);
  lines.push(`  --border: ${c('border')};`);
  lines.push(`  --border-focus: ${c('border_focus')};`);
  lines.push('');
  lines.push('  /* Badges */');
  lines.push(`  --badge-admin: ${c('badge_admin')};`);
  lines.push(`  --badge-mod: ${c('badge_mod')};`);
  lines.push(`  --badge-verified: ${c('badge_verified')};`);
  lines.push(`  --badge-donor: ${c('badge_donor')};`);
  lines.push(`  --badge-live: ${c('badge_live')};`);
  lines.push('');
  lines.push('  /* Radii */');
  lines.push(`  --radius-sm: ${n(t.border_radius_widget, 3)}px;`);
  lines.push(`  --radius: ${n(t.border_radius, 4)}px;`);
  lines.push(`  --radius-lg: ${n(t.border_radius_lg, 8)}px;`);
  lines.push(`  --badge-radius: ${n(t.badge_radius, 3)}px;`);
  lines.push('');
  lines.push('  /* Spacing (derived from native tokens via x/16 rem conversion) */');
  lines.push(`  --space-xs: ${pxToRem(spacing_xs)};`);
  lines.push(`  --space-sm: ${pxToRem(spacing_sm)};`);
  lines.push(`  --space-md: ${pxToRem(spacing_md)};`);
  lines.push(`  --space-lg: ${pxToRem(spacing_lg)};`);
  lines.push(`  --space-xl: ${pxToRem(spacing_xl)};`);
  lines.push(`  --space-2xl: ${pxToRem(spacing_xl * 1.5)};`);
  lines.push(`  --space-3xl: ${pxToRem(spacing_xl * 2)};`);
  lines.push('');
  lines.push('  /* Typography */');
  lines.push(`  --text-xs: ${pxToRem(Math.max(9, font_small - 1))};`);
  lines.push(`  --text-sm: ${pxToRem(font_small)};`);
  lines.push(`  --text-base: ${pxToRem(font_body)};`);
  lines.push(`  --text-lg: ${pxToRem(font_heading - 1)};`);
  lines.push(`  --text-xl: ${pxToRem(font_heading)};`);
  lines.push(`  --text-2xl: ${pxToRem(font_title - 4)};`);
  lines.push(`  --text-3xl: ${pxToRem(font_title)};`);
  lines.push(`  --font-size-base: ${pxToRem(font_body)};`);
  lines.push(`  --line-height: 1.6;`);
  lines.push('');
  lines.push('  /* Icons */');
  lines.push(`  --icon-size: ${n(t.icon_size, 14)}px;`);
  lines.push(`  --icon-small: ${n(t.icon_small, 12)}px;`);
  lines.push(`  --icon-weight: 3;`);
  lines.push('');
  lines.push('  /* Widget sizing */');
  lines.push(`  --button-height: ${n(t.button_height, 24)}px;`);
  lines.push(`  --input-height: ${n(t.input_height, 24)}px;`);
  lines.push(`  --sidebar-width: ${n(t.sidebar_width, 240)}px;`);
  lines.push(`  --modal-width: ${n(t.modal_width, 440)}px;`);
  lines.push(`  --row-height: ${n(t.row_height, 18)}px;`);
  lines.push(`  --header-height: ${n(t.header_height, 24)}px;`);
  lines.push(`  --content-width: none;`);
  lines.push('}');
  lines.push(END);

  return lines.join('\n');
}

function main() {
  if (!fs.existsSync(RON_PATH)) {
    console.error(`theme.ron not found at ${RON_PATH}`);
    process.exit(1);
  }
  const ron = fs.readFileSync(RON_PATH, 'utf8');
  const tokens = parseTheme(ron);
  const count = Object.keys(tokens).length;
  console.log(`Parsed ${count} tokens from data/gui/theme.ron`);

  const generated = buildCss(tokens);

  let css = '';
  if (fs.existsSync(CSS_PATH)) {
    css = fs.readFileSync(CSS_PATH, 'utf8');
  }

  const beginIdx = css.indexOf(BEGIN);
  const endIdx = css.indexOf(END);

  if (beginIdx !== -1 && endIdx !== -1) {
    // Replace the existing generated block.
    css = css.slice(0, beginIdx) + generated + css.slice(endIdx + END.length);
  } else {
    // First run: locate the old `:root { ... }` block (top-level, not nested)
    // and replace it with the generated block.
    const rootRe = /:root\s*\{[\s\S]*?\}/;
    if (rootRe.test(css)) {
      css = css.replace(rootRe, generated);
    } else {
      // No :root block found — prepend.
      css = generated + '\n\n' + css;
    }
  }

  fs.writeFileSync(CSS_PATH, css);
  console.log(`Wrote ${CSS_PATH}`);
  console.log('Tokens mapped:');
  ['bg_primary', 'bg_card', 'accent', 'text_primary', 'border'].forEach((k) => {
    if (tokens[k]) console.log(`  ${k} -> ${toCss(tokens[k])}`);
  });
}

main();
