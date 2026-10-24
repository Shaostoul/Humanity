// WCAG 2.1 AA contrast, checked from the theme TOKENS both apps paint with (the ELI5 mandate's
// accessibility audit, docs/design/wcag-audit-2026-10-23.md). The desktop app reads
// data/gui/theme.ron; the website's dark theme is generated from the same file and its light
// theme is scripts/gen-theme-css.js's LIGHT palette, so one table covers both apps.
//
// Run: node --test scripts/tests/contrast.test.js
//
// The rule (WCAG 2.1, 1.4.3 and 1.4.11): text 4.5:1 against what it sits on (3:1 for large text,
// which these tokens are never limited to); the parts of a control a person needs to see it by
// (a checkbox's outline, a slider's rail, a focus ring) 3:1. PAIRS below says which token sits on
// which, from the code that paints them; a failing pair is fixed in the TOKEN, never with a literal.
//
// Red first, 2026-10-23, against the tokens and widgets before the audit's fixes:
//   dark  success button ink (white on success)       2.41  (needs 4.5)
//   dark  danger button ink (white on danger)          4.08
//   dark  slider rail (slider_track on bg_card)        1.65  (needs 3)
//   dark  badges' white letter                         2.81 on badge_admin, 2.90 on badge_mod,
//                                                      3.29 on badge_verified, 3.82 on badge_live
// (the white-ink pairs failed as the third test: the code painted a fixed white on every fill)
// The fix: the ink on a filled control is chosen per fill (Theme::ink_on in src/gui/theme.rs), and
// slider_track was raised in theme.ron. (The website's danger buttons are outlined, not filled, so
// their words are the danger text pair above.)

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');
const GEN = require(path.join(ROOT, 'scripts', 'gen-theme-css.js'));

function lum([r, g, b]) {
  const f = (c) => { c /= 255; return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4); };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}
function ratio(a, b) {
  const la = lum(a);
  const lb = lum(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}
function fromHex(h) {
  const m = /^#([0-9a-f]{6})$/i.exec(h.trim());
  if (!m) return null;
  const n = parseInt(m[1], 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}
const WHITE = [255, 255, 255];
const BLACK = [0, 0, 0];
/** The ink a filled control uses: whichever of white and black reads better on the fill
 *  (Theme::ink_on in src/gui/theme.rs, the same rule). */
function inkOn(fill) {
  return ratio(WHITE, fill) >= ratio(BLACK, fill) ? WHITE : BLACK;
}

/** The dark palette as the desktop app paints it: theme.ron's tokens, by name. */
function darkPalette() {
  const t = GEN.parseTheme(fs.readFileSync(GEN.RON_PATH, 'utf8'));
  const out = {};
  for (const [k, v] of Object.entries(t)) if (v && v.r !== undefined) out[k] = [v.r * 255, v.g * 255, v.b * 255].map(Math.round);
  return out;
}

/** The website's light palette, mapped back to token names (--text -> text_primary). */
function lightPalette(dark) {
  const css = {};
  for (const [k, v] of Object.entries(GEN.LIGHT)) css[k] = v;
  const name = {
    bg_primary: '--bg', bg_secondary: '--bg-secondary', bg_tertiary: '--bg-tertiary', bg_card: '--bg-card',
    row_stripe: '--row-stripe', bg_panel: '--bg-panel', bg_sidebar: '--bg-sidebar', bg_sidebar_dark: '--bg-sidebar-dark',
    text_primary: '--text', text_secondary: '--text-secondary', text_muted: '--text-muted',
    text_on_accent: '--text-on-accent', text_strong: '--text-strong', accent: '--accent', success: '--success',
    warning: '--warning', danger: '--danger', info: '--info', border: '--border', border_focus: '--border-focus',
    badge_admin: '--badge-admin', badge_mod: '--badge-mod', badge_verified: '--badge-verified', badge_donor: '--badge-donor',
    badge_live: '--badge-live', slider_track: '--slider-track',
  };
  const out = {};
  for (const [tok, v] of Object.entries(name)) {
    const c = css[v] && css[v] !== 'same' ? fromHex(css[v]) : dark[tok];
    if (c) out[tok] = c;
  }
  return out;
}

const BACKGROUNDS = ['bg_primary', 'bg_secondary', 'bg_tertiary', 'bg_card', 'row_stripe'];
/** [what, foreground, background(s), minimum]. A foreground of 'ink' means the ink chosen for that fill. */
const PAIRS = [
  ['body text', 'text_primary', BACKGROUNDS, 4.5],
  ['secondary text', 'text_secondary', BACKGROUNDS, 4.5],
  ['muted text (hints, notes)', 'text_muted', BACKGROUNDS, 4.5],
  ['accent text and links', 'accent', ['bg_primary', 'bg_card', 'bg_secondary'], 4.5],
  ['success text', 'success', ['bg_primary', 'bg_card'], 4.5],
  ['warning text', 'warning', ['bg_primary', 'bg_card'], 4.5],
  ['danger text', 'danger', ['bg_primary', 'bg_card'], 4.5],
  ['info text', 'info', ['bg_primary', 'bg_card'], 4.5],
  ['primary button label', 'text_on_accent', ['accent'], 4.5],
  ['secondary button label', 'text_primary', ['bg_tertiary'], 4.5],
  ['danger button label', 'ink', ['danger'], 4.5],
  ['success button label', 'ink', ['success'], 4.5],
  ['role badge letter', 'ink', ['badge_admin', 'badge_mod', 'badge_verified', 'badge_donor', 'badge_live'], 4.5],
  ['status badge word', 'ink', ['success', 'warning', 'danger', 'info', 'accent'], 4.5],
  ['a control\'s outline (checkbox, text box)', 'text_muted', ['bg_card', 'bg_primary', 'bg_tertiary'], 3],
  ['a slider\'s rail', 'slider_track', ['bg_card', 'bg_primary', 'bg_secondary'], 3],
  ['the focus ring', 'border_focus', ['bg_card', 'bg_primary', 'bg_tertiary'], 3],
];
const DARK_ONLY = [
  ['map names', 'map_label', ['map_space'], 4.5],
  ['map minor names', 'map_label_minor', ['map_space'], 4.5],
];

function check(palette, pairs) {
  const fails = [];
  const rows = [];
  for (const [what, fg, bgs, min] of pairs) {
    for (const bg of bgs) {
      const b = palette[bg];
      if (!b) continue;
      const f = fg === 'ink' ? inkOn(b) : palette[fg];
      if (!f) continue;
      const r = ratio(f, b);
      rows.push([what, fg, bg, r]);
      if (r + 1e-9 < min) fails.push(`${what}: ${fg} on ${bg} is ${r.toFixed(2)}:1, needs ${min}:1`);
    }
  }
  return { fails, rows };
}

test('dark theme (desktop app and website): every text and control pair meets WCAG 2.1 AA', () => {
  const dark = darkPalette();
  const { fails, rows } = check(dark, PAIRS.concat(DARK_ONLY));
  if (process.env.CONTRAST_REPORT) for (const [w, f, b, r] of rows) console.log(r.toFixed(2).padStart(6) + '  ' + f + ' on ' + b + '  (' + w + ')');
  assert.deepStrictEqual(fails, [], 'dark theme pairs below AA:\n' + fails.join('\n'));
});

test('light theme (website): every text and control pair meets WCAG 2.1 AA', () => {
  const light = lightPalette(darkPalette());
  const { fails, rows } = check(light, PAIRS);
  if (process.env.CONTRAST_REPORT) for (const [w, f, b, r] of rows) console.log(r.toFixed(2).padStart(6) + '  ' + f + ' on ' + b + '  (' + w + ')');
  assert.deepStrictEqual(fails, [], 'light theme pairs below AA:\n' + fails.join('\n'));
});

// The 'ink' pairs above hold only if the code really paints a filled control's words with the
// ink rule, so read the code: no fixed white (or black) on a danger, success, badge or role-badge
// fill on the desktop app.
test('filled controls paint their words with the ink rule, not a fixed white', () => {
  const src = (rel) => fs.readFileSync(path.join(ROOT, rel), 'utf8');
  const button = src('src/gui/widgets/button.rs');
  const widgets = src('src/gui/widgets/mod.rs');
  const bad = [];
  // Every `ButtonVariant::Danger => (` / `Success => (` arm: its first element is the ink.
  for (const m of button.matchAll(/ButtonVariant::(Danger|Success)\s*=>\s*\(\s*([^,]+),/g)) {
    if (!/ink_on\(/.test(m[2])) bad.push('button.rs ' + m[1] + ' arm paints its words with ' + m[2].trim());
  }
  for (const fn of ['role_badge', 'badge', 'badge_sm']) {
    const at = widgets.indexOf('pub fn ' + fn + '(');
    assert.ok(at >= 0, 'widgets::' + fn + ' exists');
    const body = widgets.slice(at, widgets.indexOf('\n}\n', at));
    if (/Color32::WHITE|Color32::BLACK/.test(body)) bad.push('widgets::' + fn + ' paints its words a fixed colour');
  }
  assert.deepStrictEqual(bad, [], bad.join('\n'));
});

test('the ink rule picks the readable one, and the check can fail', () => {
  assert.deepStrictEqual(inkOn([255, 220, 0]), BLACK, 'dark ink on yellow');
  assert.deepStrictEqual(inkOn([20, 40, 120]), WHITE, 'white on navy');
  const { fails } = check({ a: [120, 120, 120], b: [100, 100, 100] }, [['test', 'a', ['b'], 4.5]]);
  assert.strictEqual(fails.length, 1, 'a grey on a grey fails');
});
