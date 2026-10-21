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
  // Native emphasis ink (egui .strong() labels). The web's <strong> keeps its
  // bold face and inherits its parent's colour, which never had the native
  // near-invisible defect; the variable is here for components that mirror a
  // native strong label exactly.
  lines.push(`  --text-strong: ${c('text_strong')};`);
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
  lines.push('  /* Maps page symbols (Maps increment 2): space, orbit lines by kind of body, names,');
  lines.push('     the dark edge behind a name, the belts\' dust (rock, ice), reference rings and L points */');
  lines.push(`  --map-space: ${c('map_space')}; /* theme-exempt: space is dark in both themes, the map draws over it */`);
  lines.push(`  --map-orbit-planet: ${c('map_orbit_planet')};`);
  lines.push(`  --map-orbit-moon: ${c('map_orbit_moon')};`);
  lines.push(`  --map-orbit-minor: ${c('map_orbit_minor')};`);
  lines.push(`  --map-label: ${c('map_label')};`);
  lines.push(`  --map-label-minor: ${c('map_label_minor')};`);
  lines.push(`  --map-label-halo: ${c('map_label_halo')};`);
  lines.push(`  --map-belt: ${c('map_belt')};`);
  lines.push(`  --map-belt-outer: ${c('map_belt_outer')};`);
  lines.push(`  --map-reference: ${c('map_reference')};`);
  lines.push(`  --map-orbit-width: ${n(t.map_orbit_width, 1.2)}px;`);
  lines.push(`  --map-label-size: ${n(t.map_label_size, 12.5)}px;`);
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
  lines.push('');
  lines.push('  /* Chat message-row avatar + pill (shared with native widgets/row.rs) */');
  lines.push(`  --avatar-size: ${n(t.avatar_size, 32)}px;`);
  lines.push(`  --avatar-gap: ${n(t.avatar_gap, 8)}px;`);
  lines.push(`  --pill-radius: ${n(t.pill_radius, 9)}px;`);
  lines.push('');
  lines.push('  /* Nav group colors — native nav_legacy_* tokens (RED = platform entry,');
  lines.push('     GREEN = your stuff, BLUE = system). The -rgb triples exist for rgba() use. */');
  lines.push(`  --nav-red: ${c('nav_legacy_red')};`);
  lines.push(`  --nav-green: ${c('nav_legacy_green')};`);
  lines.push(`  --nav-blue: ${c('nav_legacy_blue')};`);
  lines.push(`  --nav-sim: ${c('nav_sim')};`);
  lines.push(`  --nav-settings: ${c('nav_settings')};`);
  lines.push(`  --nav-red-rgb: ${rgbTriple(t.nav_legacy_red)};`);
  lines.push(`  --nav-green-rgb: ${rgbTriple(t.nav_legacy_green)};`);
  lines.push(`  --nav-blue-rgb: ${rgbTriple(t.nav_legacy_blue)};`);
  lines.push(`  --nav-sim-rgb: ${rgbTriple(t.nav_sim)};`);
  lines.push(`  --nav-settings-rgb: ${rgbTriple(t.nav_settings)};`);
  lines.push('');
  lines.push('  /* Chat section tints — shared with native draw_dm/groups/servers_section */');
  lines.push(`  --dm-bg: ${c('dm_bg')};`);
  lines.push(`  --dm-row-bg: ${c('dm_row_bg')};`);
  lines.push(`  --dm-row-hover: ${c('dm_row_hover')};`);
  lines.push(`  --group-bg: ${c('group_bg')};`);
  lines.push(`  --group-row-bg: ${c('group_row_bg')};`);
  lines.push(`  --group-row-hover: ${c('group_row_hover')};`);
  lines.push(`  --server-bg: ${c('server_bg')};`);
  lines.push(`  --server-row-bg: ${c('server_row_bg')};`);
  lines.push(`  --server-row-hover: ${c('server_row_hover')};`);
  lines.push('');
  lines.push('  /* Section colours past red, green and blue (the Apps page, 2026-10-19): cyan, magenta,');
  lines.push('     yellow, each a dark fill and a bright edge: the native section_* tokens. */');
  lines.push(`  --section-cyan-bg: ${c('section_cyan_bg')};`);
  lines.push(`  --section-cyan: ${c('section_cyan')};`);
  lines.push(`  --section-magenta-bg: ${c('section_magenta_bg')};`);
  lines.push(`  --section-magenta: ${c('section_magenta')};`);
  lines.push(`  --section-yellow-bg: ${c('section_yellow_bg')};`);
  lines.push(`  --section-yellow: ${c('section_yellow')};`);
  lines.push('}');
  lines.push('');
  lines.push(...buildLight(lines));
  lines.push(END);

  return lines.join('\n');
}

// ── The website's light theme ────────────────────────────────────────────
//
// WEB-ONLY. The desktop app has no light theme (data/gui/theme.ron holds one
// dark palette; docs/design/ui-system.md lists the light theme under
// "Web-only"), so this palette has no theme.ron source to come from. It lives
// HERE, in the generator, so that web/shared/theme.css is still never edited
// by hand. If the desktop app gains a light theme, these values move into
// theme.ron and this table goes.
//
// EVERY colour variable the :root block above defines must have an entry, or
// the generator stops (see buildLight). That rule is the fix for half of the
// 2026-10-19 report "the light theme recoloured only the top menu": the old
// hand-written light block redefined 15 of the 45 colour variables, so every
// surface painted with the other 30 (panels, sidebars, strong text, the chat
// section tints) stayed dark. 'same' says on purpose that a colour reads the
// same on light and dark (the nav category hues), so a new variable can never
// be missed silently.
//
// Contrast (WCAG AA, 4.5:1 for body text), measured 2026-10-19 against the
// three grounds text sits on, --bg-secondary (#e8e8e8, the darkest), --bg
// (#f4f4f4) and --bg-card (#ffffff): --text 14.2:1 at worst, --text-secondary
// 8.8:1, --text-muted 6.0:1, --accent 4.53:1, --success 5.2:1, --warning
// 4.52:1, --danger 4.8:1, --info 5.0:1. White on --accent is 5.55:1 and on
// every badge at least 4.59:1. The accent is a darker orange than the dark theme's on purpose:
// #ed8c24 on a light page is 2.4:1, too faint for link text, and no orange can
// carry BOTH dark text on it and itself as text on a light page, so in light
// mode the ink on the accent turns white (--text-on-accent, 5.5:1).
const LIGHT = {
  // Backgrounds: page #f4f4f4, cards and panels white, inputs and wells grey.
  '--bg': '#f4f4f4',
  '--bg-secondary': '#e8e8e8',
  '--bg-tertiary': '#e0e0e0',
  '--bg-card': '#ffffff',
  '--bg-card-hover': '#f0f0f0',
  '--bg-input': '#ffffff',
  '--bg-hover': '#e0e0e0',
  '--bg-modal': 'rgba(0, 0, 0, 0.45)',
  '--bg-panel': '#ffffff',
  '--bg-sidebar': '#ebebeb',
  '--bg-sidebar-dark': '#e2e2e2',
  // Text
  '--text': '#1a1a1a',
  '--text-secondary': '#3d3d45',
  '--text-muted': '#55555e',
  '--text-on-accent': '#ffffff',
  '--text-strong': '#000000',
  // Accent
  '--accent': '#a84f00',
  '--accent-hover': '#8c4200',
  '--accent-pressed': '#7a3900',
  '--accent-dim': 'rgba(168, 79, 0, 0.12)',
  // Semantic
  '--success': '#1a6e2e',
  '--warning': '#8a6100',
  '--danger': '#c0281c',
  '--info': '#1f5fbf',
  '--border': '#c8c8d0',
  '--border-focus': '#a84f00',
  // Badges: the same hues, darkened so white text on them and the colour as
  // text on a light page both read.
  '--badge-admin': '#b85c0f',
  '--badge-mod': '#1b7f46',
  '--badge-verified': '#1f6fae',
  '--badge-donor': '#7d3f98',
  '--badge-live': '#c0322a',
  // Nav category hues: identical in both themes (the pills tint and border
  // with them at low alpha, which reads on either ground).
  '--nav-red': 'same',
  '--nav-green': 'same',
  '--nav-blue': 'same',
  '--nav-sim': 'same',
  '--nav-settings': 'same',
  // Chat section tints (DMs red, groups green, servers blue): pale washes of
  // the same hues.
  '--dm-bg': '#f6e6e6',
  '--dm-row-bg': '#f0dada',
  '--dm-row-hover': '#e8cccc',
  '--group-bg': '#e4f2e4',
  '--group-row-bg': '#d8ecd8',
  '--group-row-hover': '#cae4ca',
  '--server-bg': '#e6e6f6',
  '--server-row-bg': '#dadaf0',
  '--server-row-hover': '#ccccea',
  // The Apps page's cyan, magenta and yellow sections (2026-10-20): pale washes for the grounds,
  // and each hue darkened so its band name reads on a light page (4.5:1 or better on white).
  '--section-cyan-bg': '#e2f3f5',
  '--section-cyan': '#0b7480',
  '--section-magenta-bg': '#f5e3f3',
  '--section-magenta': '#9c2c90',
  '--section-yellow-bg': '#f6f0d6',
  '--section-yellow': '#7f6200',
  // The Maps page's space is black in both themes (it is space; the line that
  // defines --map-space says so with theme-exempt), and its symbols sit on it,
  // so all of them keep their dark-theme colours.
  '--map-space': 'same',
  '--map-orbit-planet': 'same',
  '--map-orbit-moon': 'same',
  '--map-orbit-minor': 'same',
  '--map-label': 'same',
  '--map-label-minor': 'same',
  '--map-label-halo': 'same',
  '--map-belt': 'same',
  '--map-belt-outer': 'same',
  '--map-reference': 'same',
};

/** Emit the light block, refusing to run if it does not cover every colour
 *  variable the :root block defines (or names one that no longer exists). */
function buildLight(rootLines, light = LIGHT) {
  const colourVars = [];
  for (const line of rootLines) {
    // A trailing /* comment */ is allowed (the theme-exempt note on --map-space).
    const m = /^\s*(--[\w-]+):\s*(#[0-9a-fA-F]{3,8}|rgba?\([^)]*\));\s*(\/\*.*\*\/)?\s*$/.exec(line);
    if (m) colourVars.push(m[1]);
  }
  const missing = colourVars.filter((v) => !(v in light));
  const stale = Object.keys(light).filter((v) => !colourVars.includes(v));
  if (missing.length || stale.length) {
    throw new Error(
      'gen-theme-css: the LIGHT palette must cover every colour variable exactly.\n' +
      (missing.length ? `  no light value for: ${missing.join(', ')}\n` : '') +
      (stale.length ? `  light value for a variable :root no longer defines: ${stale.join(', ')}\n` : '') +
      "  Add a light colour (or 'same') to LIGHT in scripts/gen-theme-css.js."
    );
  }
  const out = [];
  out.push('/* The website\'s light theme (web-only: the desktop app has none). shell.js sets');
  out.push('   data-theme="light" on <html> from the Settings choice. :root[data-theme] and');
  out.push('   not a bare [data-theme], so it outranks a page <style> that sets its own');
  out.push('   :root variables. Values: LIGHT in scripts/gen-theme-css.js. */');
  out.push(':root[data-theme="light"] {');
  out.push('  color-scheme: light;');
  for (const v of colourVars) {
    if (light[v] === 'same') continue;
    out.push(`  ${v}: ${light[v]};`);
  }
  out.push('}');
  return out;
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

  // Normalize CRLF before marker matching (v0.692 fix): the BEGIN marker embeds
  // \n newlines, so once git checked the file out with \r\n the indexOf missed,
  // the :root fallback matched INSIDE the previous generated block, and every
  // regen nested another banner pair (the file had accumulated four).
  css = css.replace(/\r\n/g, '\n');
  const beginIdx = css.indexOf(BEGIN);
  // Collapse first-BEGIN .. LAST-END so pre-fix accumulated duplicates heal on
  // the next regen instead of persisting.
  const endIdx = css.lastIndexOf(END);

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

// Run when called as a script; scripts/tests/web-theme-light.test.js requires it
// to check theme.css against it and to see the light-coverage rule refuse.
if (require.main === module) main();
module.exports = { parseTheme, buildCss, buildLight, LIGHT, BEGIN, END, RON_PATH, CSS_PATH };
