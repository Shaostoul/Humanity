// The website's light theme reaches every page (2026-10-19). Found by a builder on the web
// Tools page: choosing the light theme recoloured only the top menu and every page stayed
// dark. It was site-wide, and it had three causes, each pinned here:
//
//  A. web/shared/settings.js wrote the DARK palette as inline variables on <html> on every
//     page load. An inline variable outranks every stylesheet rule, so theme.css's light
//     block never applied; only the nav, whose light rules used literal colours, changed.
//  B. The light block redefined 15 of the 45 colour variables theme.css defines, so every
//     surface painted with the other 30 (panels, sidebars, strong text, the chat section
//     tints) stayed dark. And the light choice lived in two places (shell.js read a
//     `hos_theme` key no control wrote; the Settings page wrote `humanity_settings.theme`,
//     which nothing applied).
//  C. Pages painted surfaces with literal dark colours (#181818 cards, #1a1a1a inputs,
//     #0a0a0a wells) where a theme variable exists.
//
// Run: node --test scripts/tests/web-theme-light.test.js
//
// What it proves:
//  1. theme.css is what scripts/gen-theme-css.js writes from data/gui/theme.ron, and its
//     light block redefines every colour variable the :root block defines (none forgotten).
//  2. The generator refuses a palette that misses a variable, or names one that is gone.
//  3. shell.js's theme block applies the ONE stored choice: light, dark, and "system"
//     following the device; high contrast (a dark palette of its own) wins while it is on;
//     the toggle writes that same choice.
//  4. settings.js never writes the base palette inline for dark or light; it paints only the
//     dark presets (midnight, OLED) and clears them when the choice changes; the default
//     accent is left to theme.css in the light theme, a chosen accent applies in both.
//  5. No page stylesheet paints a background with a literal dark colour (CSS files, <style>
//     blocks and style="" in HTML, CSS strings in JS). A surface that is meant to be dark in
//     both themes (a video player, the map of space, the always-dark tooltip) says so on the
//     same line: `theme-exempt: <reason>`.
//  6. Every colour variable a page defines with a dark value has a light value too, in a
//     :root[data-theme="light"] rule of the same file (or theme.css's light block covers it).
//
// HOS_WEB_DIR points the web checks at another copy of web/ (used to see each one red).
//
// Red first, 2026-10-19: 12 of the 16 tests failed run against the web/ of HEAD before the
// fix (01bdef11c) with HOS_WEB_DIR, and two checks against deliberate breaks:
//   1. HEAD's theme.css: "out of date", and no light block inside the generated region
//      (the old one was hand-written after it and redefined 15 of the 45 colour variables).
//   2. with the coverage check in buildLight turned off, "Missing expected exception".
//   3. "the stored light choice turns the light theme on", "system", high contrast and the
//      toggle failed: the old shell read hos_theme, so humanity_settings.theme = "light" did
//      nothing. With `&& !highContrast()` dropped from the new shell, the high contrast
//      test failed on its own.
//   4. "light: no base palette inline" and "dark: ..." failed: --bg was inline on <html>.
//   5. 104 literal dark backgrounds in 21 files (settings.html 18, settings-app.js 12,
//      chat/voice.css 10, shell.js 9, tasks.html 7, calendar.html 7, admin.html 6, ...).
//   6. theme.css's own --bg-tertiary, --bg-panel, --bg-sidebar, --dm-bg, --server-bg, ...,
//      --bg-panel on admin, maps, market, ops and trade, and tasks.html's --col-backlog,
//      --col-done, --border-bright, ... had no light value.
// The four that passed on HEAD: the generator's own check (new with the fix), "dark, a dark
// preset, nothing stored..." (dark was always the default), "a chosen accent applies in the
// light theme too", and the background check's self-test (it runs on its own sample file).

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const GEN = require(path.join(ROOT, 'scripts', 'gen-theme-css.js'));

const read = (p) => fs.readFileSync(p, 'utf8').replace(/\r\n/g, '\n');
const THEME_CSS = read(path.join(WEB, 'shared', 'theme.css'));

// ── Colour helpers ──────────────────────────────────────────────────────

function luminance(r, g, b) {
  const f = (c) => { c /= 255; return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4); };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

/** The literal colours in a CSS value, ignoring any var(...) (its fallback never paints
 *  while the variable is defined). */
function literalColours(value) {
  let v = '';
  for (let i = 0; i < value.length;) {
    if (value.startsWith('var(', i)) {
      let depth = 0, j = i;
      for (; j < value.length; j++) {
        if (value[j] === '(') depth++;
        else if (value[j] === ')' && --depth === 0) break;
      }
      i = j + 1;
      v += ' ';
    } else v += value[i++];
  }
  const out = [];
  for (const m of v.matchAll(/#([0-9a-fA-F]{8}|[0-9a-fA-F]{6}|[0-9a-fA-F]{3,4})\b/g)) {
    let h = m[1];
    if (h.length <= 4) h = h.split('').map((c) => c + c).join('');
    out.push({ text: m[0], r: parseInt(h.slice(0, 2), 16), g: parseInt(h.slice(2, 4), 16), b: parseInt(h.slice(4, 6), 16), a: h.length === 8 ? parseInt(h.slice(6, 8), 16) / 255 : 1 });
  }
  for (const m of v.matchAll(/rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([\d.]+%?)\s*)?\)/g)) {
    const a = m[4] === undefined ? 1 : m[4].endsWith('%') ? parseFloat(m[4]) / 100 : parseFloat(m[4]);
    out.push({ text: m[0], r: +m[1], g: +m[2], b: +m[3], a });
  }
  if (/\bblack\b/.test(v)) out.push({ text: 'black', r: 0, g: 0, b: 0, a: 1 });
  return out;
}

/** A dark surface colour: near-black (relative luminance under 0.04, about #383838) and
 *  mostly opaque. See-through pure black is a scrim behind a dialog, which darkens the
 *  page the same way in either theme, so it is allowed. */
function isDarkSurface(c) {
  if (c.a < 0.8) return false;
  if (c.r === 0 && c.g === 0 && c.b === 0 && c.a < 1) return false;
  return luminance(c.r, c.g, c.b) < 0.04;
}

// ── The files the web serves ────────────────────────────────────────────

function webFiles(dir = WEB, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) {
      // vendor: third-party code; fonts, icons, bg: no stylesheets in them.
      if (!['vendor', 'fonts', 'icons', 'bg'].includes(e.name)) webFiles(p, out);
    } else if (/\.(css|html|js)$/.test(e.name) && !/\.min\.js$/.test(e.name) && e.name !== 'bip39-english.js') {
      out.push(p);
    }
  }
  return out;
}
const FILES = webFiles();
const rel = (p) => path.relative(WEB, p).replace(/\\/g, '/');

// An exemption must give a reason: words after the colon, not just the end of a comment.
const EXEMPT = /theme-exempt:\s*[A-Za-z]/;

// ── 1 and 2: theme.css and its generator ────────────────────────────────

test('theme.css is what gen-theme-css.js writes from theme.ron', () => {
  const tokens = GEN.parseTheme(fs.readFileSync(GEN.RON_PATH, 'utf8'));
  const generated = GEN.buildCss(tokens);
  assert.ok(THEME_CSS.includes(generated),
    'web/shared/theme.css is out of date: run node scripts/gen-theme-css.js');
});

test('the light block redefines every colour variable the :root block defines', () => {
  const begin = THEME_CSS.indexOf(GEN.BEGIN);
  const end = THEME_CSS.indexOf(GEN.END);
  assert.ok(begin >= 0 && end > begin, 'theme.css has its generated region');
  const region = THEME_CSS.slice(begin, end);
  const root = /:root \{([\s\S]*?)\n\}/.exec(region)[1];
  const lightM = /:root\[data-theme="light"\] \{([\s\S]*?)\n\}/.exec(region);
  assert.ok(lightM, 'the generated region holds a :root[data-theme="light"] block');
  const light = lightM[1];
  const colourVars = [...root.matchAll(/(--[\w-]+):\s*(#[0-9a-fA-F]{3,8}|rgba?\([^)]*\));/g)].map((m) => m[1]);
  assert.ok(colourVars.length >= 40, 'found the colour variables (' + colourVars.length + ')');
  const missing = colourVars.filter((v) => !new RegExp('\\n\\s*' + v + ':').test('\n' + light) && GEN.LIGHT[v] !== 'same');
  assert.deepStrictEqual(missing, [], 'colour variables with no light value: ' + missing.join(', '));
  // And the light values are what the generator's palette says.
  for (const v of colourVars) {
    if (GEN.LIGHT[v] === 'same') continue;
    assert.ok(light.includes(`${v}: ${GEN.LIGHT[v]};`), v + ' is ' + GEN.LIGHT[v] + ' in the light block');
  }
});

test('the generator refuses a light palette that misses a variable or names a gone one', () => {
  const lines = ['  --bg: #000000;', '  --bg-panel: #000000;', '  --nav-red: #e74c3c;', '  --space-xs: 0.125rem;'];
  const ok = { '--bg': '#f4f4f4', '--bg-panel': '#ffffff', '--nav-red': 'same' };
  const out = GEN.buildLight(lines, ok).join('\n');
  assert.match(out, /--bg: #f4f4f4;/);
  assert.doesNotMatch(out, /--nav-red/, "'same' writes nothing: the dark value stands");
  assert.doesNotMatch(out, /--space-xs/, 'only colours are themed');
  const missing = { ...ok }; delete missing['--bg-panel'];
  assert.throws(() => GEN.buildLight(lines, missing), /no light value for: --bg-panel/);
  assert.throws(() => GEN.buildLight(lines, { ...ok, '--gone': '#fff' }), /no longer defines: --gone/);
  // The real palette covers the real :root block (buildCss would throw otherwise).
  assert.doesNotThrow(() => GEN.buildCss(GEN.parseTheme(fs.readFileSync(GEN.RON_PATH, 'utf8'))));
});

// ── Stand-ins for running the page scripts ─────────────────────────────

function fakeRoot() {
  const attrs = new Map();
  const props = new Map();
  return {
    attrs, props,
    setAttribute: (k, v) => attrs.set(k, String(v)),
    removeAttribute: (k) => attrs.delete(k),
    getAttribute: (k) => (attrs.has(k) ? attrs.get(k) : null),
    hasAttribute: (k) => attrs.has(k),
    style: {
      setProperty: (k, v) => props.set(k, String(v)),
      removeProperty: (k) => props.delete(k),
      getPropertyValue: (k) => props.get(k) || '',
    },
  };
}

function fakeStorage(init = {}) {
  const m = new Map(Object.entries(init));
  return {
    getItem: (k) => (m.has(k) ? m.get(k) : null),
    setItem: (k, v) => m.set(k, String(v)),
    removeItem: (k) => m.delete(k),
    map: m,
  };
}

/** A window running the given scripts: the document holds only the root element. */
function runPage(scripts, { stored = {}, systemLight = false } = {}) {
  const root = fakeRoot();
  const listeners = [];
  const mqListeners = [];
  const ctx = {
    console,
    localStorage: fakeStorage(stored),
    setInterval: () => 0,
    clearInterval: () => {},
    setTimeout: () => 0,
    matchMedia: (q) => ({
      matches: /prefers-color-scheme: light/.test(q) ? systemLight : false,
      addEventListener: (_t, fn) => mqListeners.push(fn),
    }),
    addEventListener: (t, fn) => listeners.push([t, fn]),
    document: {
      documentElement: root,
      readyState: 'complete',
      querySelector: () => null,
      getElementById: () => null,
      addEventListener: () => {},
    },
  };
  ctx.window = ctx;
  vm.createContext(ctx);
  for (const s of scripts) vm.runInContext(s, ctx);
  return { ctx, root, listeners, mqListeners };
}

/** shell.js's theme block on its own (the rest of shell.js builds the whole nav). */
function shellThemeBlock() {
  const shell = read(path.join(WEB, 'shared', 'shell.js'));
  const start = shell.search(/\n {2}\/\/ ── Theme[^\n]*\n/);
  assert.ok(start >= 0, 'shell.js has its theme block');
  const end = shell.indexOf('\n  })();', start);
  assert.ok(end > start, 'the theme block closes');
  return shell.slice(start, end + '\n  })();'.length);
}

const settingsBlob = (o) => ({ humanity_settings: JSON.stringify(o) });

// ── 3: shell.js applies the one stored choice ──────────────────────────

test('the stored light choice turns the light theme on', () => {
  const { root } = runPage([shellThemeBlock()], { stored: settingsBlob({ theme: 'light' }) });
  assert.strictEqual(root.getAttribute('data-theme'), 'light');
});

test('dark, a dark preset, nothing stored, or garbage stored: no light theme', () => {
  for (const stored of [settingsBlob({ theme: 'dark' }), settingsBlob({ theme: 'midnight' }), {}, { humanity_settings: '{nope' }]) {
    const { root } = runPage([shellThemeBlock()], { stored });
    assert.strictEqual(root.getAttribute('data-theme'), null, JSON.stringify(stored));
  }
});

test('"system" follows the device, and a change on the device is followed', () => {
  const light = runPage([shellThemeBlock()], { stored: settingsBlob({ theme: 'system' }), systemLight: true });
  assert.strictEqual(light.root.getAttribute('data-theme'), 'light');
  const dark = runPage([shellThemeBlock()], { stored: settingsBlob({ theme: 'system' }), systemLight: false });
  assert.strictEqual(dark.root.getAttribute('data-theme'), null);
  assert.ok(dark.mqListeners.length >= 1, 'listens for the device changing');
});

test('high contrast keeps the page dark while it is on (its white ink on light cards would vanish)', () => {
  const on = runPage([shellThemeBlock()], { stored: { ...settingsBlob({ theme: 'light' }), a11y_high_contrast: 'true' } });
  assert.strictEqual(on.root.getAttribute('data-theme'), null);
  const off = runPage([shellThemeBlock()], { stored: { ...settingsBlob({ theme: 'light' }), a11y_high_contrast: 'false' } });
  assert.strictEqual(off.root.getAttribute('data-theme'), 'light');
});

test('the toggle writes the same stored choice the Settings page reads', () => {
  const { ctx, root } = runPage([shellThemeBlock()], { stored: settingsBlob({ theme: 'dark', accent: '#4488ff' }) });
  ctx.hosToggleTheme();
  assert.strictEqual(root.getAttribute('data-theme'), 'light');
  const saved = JSON.parse(ctx.localStorage.getItem('humanity_settings'));
  assert.strictEqual(saved.theme, 'light');
  assert.strictEqual(saved.accent, '#4488ff', 'the rest of the settings are kept');
  ctx.hosToggleTheme();
  assert.strictEqual(root.getAttribute('data-theme'), null);
});

// ── 4: settings.js never paints the base palette inline ────────────────

const DEFAULTS_JS = read(path.join(WEB, 'shared', 'defaults.js'));
const SETTINGS_JS = read(path.join(WEB, 'shared', 'settings.js'));
const PALETTE = ['--bg', '--bg-secondary', '--bg-card', '--bg-card-hover', '--bg-input', '--bg-hover', '--text', '--text-muted', '--border'];

test('light: no base palette inline, and the default accent is left to theme.css', () => {
  const { root } = runPage([shellThemeBlock(), DEFAULTS_JS, SETTINGS_JS], { stored: settingsBlob({ theme: 'light' }) });
  assert.strictEqual(root.getAttribute('data-theme'), 'light');
  for (const k of PALETTE) assert.strictEqual(root.style.getPropertyValue(k), '', k + ' is not inline');
  for (const k of ['--accent', '--accent-hover', '--accent-dim']) assert.strictEqual(root.style.getPropertyValue(k), '', k + ' is not inline');
});

test('dark: no base palette inline (theme.css holds it), the accent as before', () => {
  const { root } = runPage([shellThemeBlock(), DEFAULTS_JS, SETTINGS_JS], { stored: settingsBlob({ theme: 'dark' }) });
  assert.strictEqual(root.getAttribute('data-theme'), null);
  for (const k of PALETTE) assert.strictEqual(root.style.getPropertyValue(k), '', k + ' is not inline');
  assert.strictEqual(root.style.getPropertyValue('--accent'), '#ed8c24');
});

test('a dark preset is painted inline, and choosing light clears all of it', () => {
  const page = runPage([shellThemeBlock(), DEFAULTS_JS, SETTINGS_JS], { stored: settingsBlob({ theme: 'midnight' }) });
  assert.strictEqual(page.root.style.getPropertyValue('--bg'), '#080812');
  assert.strictEqual(page.root.getAttribute('data-theme'), null);
  page.ctx.localStorage.setItem('humanity_settings', JSON.stringify({ theme: 'light' }));
  page.ctx.hosApplySettings();
  assert.strictEqual(page.root.getAttribute('data-theme'), 'light');
  for (const k of PALETTE) assert.strictEqual(page.root.style.getPropertyValue(k), '', k + ' cleared');
});

test('a chosen accent applies in the light theme too', () => {
  const { root } = runPage([shellThemeBlock(), DEFAULTS_JS, SETTINGS_JS], { stored: settingsBlob({ theme: 'light', accent: '#4488ff' }) });
  assert.strictEqual(root.style.getPropertyValue('--accent'), '#4488ff');
});

test('a page whose shell has not loaded: settings.js still sets no palette inline', () => {
  const { root } = runPage([DEFAULTS_JS, SETTINGS_JS], { stored: settingsBlob({ theme: 'light' }) });
  for (const k of PALETTE) assert.strictEqual(root.style.getPropertyValue(k), '', k + ' is not inline');
});

// ── 5: no literal dark backgrounds in page stylesheets ─────────────────

const BACKGROUND = /background(?:-color)?\s*:\s*([^;'"`}\n]+)|\.style\.background(?:Color)?\s*=\s*['"]([^'"]+)['"]/g;

function darkBackgrounds(file) {
  const hits = [];
  read(file).split('\n').forEach((line, i) => {
    if (EXEMPT.test(line)) return;
    for (const m of line.matchAll(BACKGROUND)) {
      for (const c of literalColours(m[1] || m[2])) {
        if (isDarkSurface(c)) hits.push(`${rel(file)}:${i + 1}: ${c.text}  ${line.trim().slice(0, 110)}`);
      }
    }
  });
  return hits;
}

test('the background check catches what it is for (a check that cannot fail proves nothing)', () => {
  const tmp = path.join(require('node:os').tmpdir(), 'hos-theme-lint-' + process.pid + '.css');
  fs.writeFileSync(tmp, [
    '.a { background: #181818; }',                      // caught
    '.b { background: linear-gradient(#0e0e0e, #fff); }', // caught (the dark stop)
    "el.style.cssText = 'background:#111;color:#eee';",  // caught (a CSS string in JS)
    "el.style.background = 'rgba(8,8,10,0.97)';",        // caught
    '.c { background: var(--bg-card, #161616); }',       // a fallback: fine
    '.d { background: rgba(0,0,0,0.85); }',              // a scrim: fine
    '.e { background: #e8e8e8; }',                       // light: fine
    '.f { background: #000; } /* theme-exempt: a video player */', // said so: fine
    '.g { background: #000; } /* theme-exempt: */',      // no reason given: caught
  ].join('\n'));
  try {
    const hits = darkBackgrounds(tmp).map((h) => h.split(': ')[1].split(' ')[0]);
    assert.deepStrictEqual(hits, ['#181818', '#0e0e0e', '#111', 'rgba(8,8,10,0.97)', '#000']);
  } finally { fs.unlinkSync(tmp); }
});

test('no page stylesheet paints a background with a literal dark colour', () => {
  const hits = FILES.flatMap(darkBackgrounds);
  assert.deepStrictEqual(hits, [],
    'Literal dark backgrounds stay dark under the light theme. Use a theme variable ' +
    '(var(--bg-card), var(--bg-secondary), var(--bg-input), ...) or a page variable with a ' +
    'light value; or, for a surface dark in BOTH themes, say why on the line with ' +
    '`theme-exempt: <reason>`.\n' + hits.join('\n'));
});

// ── 6: every dark page variable has a light value ──────────────────────

/** Names a file redefines inside a rule whose selector names the light theme. */
function lightNames(text) {
  const names = new Set();
  for (const m of text.matchAll(/\[data-theme=["']?light["']?\][^{}]*\{([^{}]*)\}/g)) {
    for (const d of m[1].matchAll(/(--[\w-]+)\s*:/g)) names.add(d[1]);
  }
  return names;
}
const THEME_LIGHT = lightNames(THEME_CSS);

test('every colour variable a page defines with a dark value has a light value', () => {
  const hits = [];
  for (const file of FILES) {
    const text = read(file);
    const light = lightNames(text);
    text.split('\n').forEach((line, i) => {
      if (EXEMPT.test(line)) return;
      for (const m of line.matchAll(/(--[\w-]+)\s*:\s*([^;'"`}\n]+)/g)) {
        if (!literalColours(m[2]).some(isDarkSurface)) continue;
        if (light.has(m[1]) || THEME_LIGHT.has(m[1])) continue;
        hits.push(`${rel(file)}:${i + 1}: ${m[1]}: ${m[2].trim()}`);
      }
    });
  }
  assert.deepStrictEqual(hits, [],
    'A variable with a dark colour and no light value keeps the page dark under the light ' +
    'theme. Give it one in a :root[data-theme="light"] rule in the same file.\n' + hits.join('\n'));
});

// ── 7: text coloured with a literal is readable in the light theme ─────
//
// Found 2026-10-20 by the review of that night's merges, on the Files page's server data folder
// (web/shared/files-folder.css): its JSON viewer's keys and strings (#7ec8e3, #a8d89a) read at
// about 1.7:1 and 1.5:1 on the light page, and Save's black text on the light accent at about
// 3.8:1. Check 5 cannot see either: a pale ink on a light page is the text side of a dark
// surface. So: in every page stylesheet (CSS files and the <style> elements of HTML pages; a
// colour a script writes is not a stylesheet and is not read here), each `color:` that names a
// literal colour, or a variable the same file defines, must reach 4.5:1 (WCAG AA for body text)
// in the light theme against its rule's own background (a theme variable's light value, or a
// literal) or, when the rule sets none, the light page background. A rule only for a dark or
// high-contrast theme is not read. Same escape as 5: `theme-exempt: <reason>` on the line, for
// text that sits on a surface dark in both themes.
//
// OLDER_TEXT holds the lines that were already there when this check came, as a count per file:
// a file may not gain one, and a file that loses one must lower its count (so the list only
// shrinks, as tests/theme_token_lint.rs's LEGACY_OFFENDERS does). Many are white text on a
// surface the rule does not paint (a badge drawn by another rule, a video's dark overlay), which
// a person must look at; none is new. Black text on the accent, the review's Save button, was on
// seven more pages: those 13 lines moved to var(--text-on-accent) the same day (its dark value is
// #0d0d0d, so the dark theme looks the same).
//
// Red first, 2026-10-20, against files-folder.css as the review found it: "no page stylesheet
// colours text with a literal that is unreadable in the light theme" listed its six lines
// (shared/files-folder.css:15: #000 on var(--accent), 3.78:1; :46 #7ec8e3 on the page, 1.70:1;
// :47 #a8d89a, 1.47:1; :48 #d4a0ff, 1.86:1; :49 #f0a500, 1.89:1; :50 #888, 3.22:1).

/** The light theme's own colour variables: theme.css's generated light block. */
const LIGHT_VARS = (() => {
  const region = THEME_CSS.slice(THEME_CSS.indexOf(GEN.BEGIN), THEME_CSS.indexOf(GEN.END));
  const block = /:root\[data-theme="light"\] \{([\s\S]*?)\n\}/.exec(region)[1];
  const out = {};
  for (const m of block.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) out[m[1]] = m[2].trim();
  return out;
})();
const LIGHT_PAGE = literalColours(LIGHT_VARS['--bg'])[0];

/** WCAG contrast ratio of two colours (alpha ignored: callers pass solid ones). */
function contrast(a, b) {
  const x = luminance(a.r, a.g, a.b), y = luminance(b.r, b.g, b.b);
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
}

/** A file's stylesheet text with line numbers kept: a CSS file whole, an HTML page's <style>
 *  elements only (everything else blanked to spaces, its newlines kept). */
function stylesheetText(file) {
  const text = read(file);
  if (!file.endsWith('.html')) return text;
  const blank = (s) => s.replace(/[^\n]/g, ' ');
  let out = '', at = 0;
  for (const m of text.matchAll(/(<style\b[^>]*>)([\s\S]*?)<\/style>/gi)) {
    const body = m.index + m[1].length;
    out += blank(text.slice(at, body)) + m[2];
    at = body + m[2].length;
  }
  return out + blank(text.slice(at));
}

/** The lines where `css` colours text below 4.5:1 in the light theme, as `<line>: <what>`. */
function lowContrastText(css) {
  const lines = css.split('\n');
  // Where each line starts, so a block's line is found without counting from the top each time.
  const starts = [0];
  for (let i = css.indexOf('\n'); i >= 0; i = css.indexOf('\n', i + 1)) starts.push(i + 1);
  const lineAt = (offset) => {
    let lo = 0, hi = starts.length - 1;
    while (lo < hi) { const mid = (lo + hi + 1) >> 1; if (starts[mid] <= offset) lo = mid; else hi = mid - 1; }
    return lo + 1;
  };
  // The innermost rule blocks, `selector { body }`, in one pass (a regular expression over a page
  // of blanked HTML backtracks for minutes): an @media's own braces hold no declarations.
  const blocks = [];
  let boundary = 0, open = -1, selector = 0;
  for (let i = 0; i < css.length; i++) {
    if (css[i] === '{') { open = i; selector = boundary; boundary = i + 1; }
    else if (css[i] === '}') {
      if (open >= 0) blocks.push({ selector: css.slice(selector, open), body: css.slice(open + 1, i), at: open + 1 });
      open = -1; boundary = i + 1;
    }
  }
  // The variables this file defines: their light value (a light-theme rule's), else their value.
  const plain = {}, light = {};
  for (const b of blocks) {
    const isLight = /\[data-theme=["']?light["']?\]/.test(b.selector);
    for (const d of b.body.matchAll(/(--[\w-]+)\s*:\s*([^;]+)/g)) {
      if (isLight) light[d[1]] = d[2];
      else if (!(d[1] in plain)) plain[d[1]] = d[2];
    }
  }
  const pageVar = (name) => light[name] || plain[name];
  const solid = (value) => literalColours(value).find((c) => c.a >= 0.8) || null;
  const hits = [];
  for (const b of blocks) {
    // A rule only for a dark or high-contrast theme is not what the light theme draws.
    if (/data-theme=["']?(?!light\b)\w|high-contrast/i.test(b.selector)) continue;
    let backdrop = LIGHT_PAGE, under = 'the page';
    const bg = /(?<![\w-])background(?:-color)?\s*:\s*([^;]+)/.exec(b.body);
    if (bg) {
      const v = /var\((--[\w-]+)/.exec(bg[1]);
      const value = v ? LIGHT_VARS[v[1]] || pageVar(v[1]) : bg[1];
      const c = value ? solid(value) : null;
      if (c) { backdrop = c; under = bg[1].trim(); }
    }
    for (const d of b.body.matchAll(/(?<![\w-])color\s*:\s*([^;]+)/g)) {
      const line = lineAt(b.at + d.index);
      if (EXEMPT.test(lines[line - 1] || '')) continue;
      let inks = literalColours(d[1]).filter((c) => c.a >= 0.5);
      if (!inks.length) {
        const v = /^\s*var\((--[\w-]+)/.exec(d[1]);
        const value = v && !(v[1] in LIGHT_VARS) ? pageVar(v[1]) : null;
        inks = value ? literalColours(value).filter((c) => c.a >= 0.5) : [];
      }
      for (const ink of inks) {
        const r = contrast(ink, backdrop);
        if (r < 4.5) hits.push(`${line}: ${d[1].trim()} on ${under}, ${r.toFixed(2)}:1`);
      }
    }
  }
  return hits;
}

/** The lines each page stylesheet has, by file (only files that have some). */
function lowContrastByFile() {
  const out = {};
  for (const file of FILES.filter((f) => /\.(css|html)$/.test(f))) {
    const hits = lowContrastText(stylesheetText(file));
    if (hits.length) out[rel(file)] = hits;
  }
  return out;
}

/** The lines that were there when check 7 came (2026-10-20), as a count per file: only shrinks. */
const OLDER_TEXT = {
  'chat/messages.css': 2, 'chat/modals.css': 1, 'chat/sidebar.css': 4, 'chat/voice.css': 8,
  'pages/admin.html': 3, 'pages/bugs.html': 3, 'pages/dev.html': 4, 'pages/governance.html': 4,
  'pages/guilds.html': 1, 'pages/home.html': 1, 'pages/inventory.html': 1, 'pages/library.html': 7,
  'pages/ops.html': 1, 'pages/roadmap.html': 3, 'pages/watch.html': 2, 'pages/widgets.html': 2,
};

test('the text check catches what it is for (a check that cannot fail proves nothing)', () => {
  const css = [
    '.a { color: #7ec8e3; }',                                    // caught: pale ink on the light page
    '.b { background: var(--accent); color: #000; }',            // caught: black on the light accent
    '.c { --ink: #a8d89a; }',
    '.d { color: var(--ink); }',                                 // caught: a page variable with no light value
    ':root[data-theme="light"] .e { --ok: #2e6b1f; }',
    '.e { --ok: #a8d89a; color: var(--ok); }',                   // fine: its light value reads
    '.f { color: var(--text-muted, #888); }',                    // a theme variable (its fallback never paints): fine
    '.g { color: #1a1a1a; }',                                    // dark ink: fine
    '.h { background: var(--accent); color: var(--text-on-accent); }', // fine
    '.i { color: #fff; } /* theme-exempt: on the video, dark in both themes */', // said so: fine
    '.j { color: #fff; } /* theme-exempt: */',                   // no reason given: caught
    ':root[data-theme="dark"] .k { color: #eee; }',              // a dark-only rule: not read
    '.l { background-color: #222; color: #eee; }',               // its own dark surface: fine
  ].join('\n');
  const lines = lowContrastText(css).map((h) => +h.split(':')[0]);
  assert.deepStrictEqual(lines, [1, 2, 4, 11]);
  // An HTML page: its <style> only, its lines where they are in the page.
  const html = '<p style="color:#fff">x</p>\n<style>\n.a { color: #fff; }\n</style>\n<script>const o = { color: "#fff" };</script>';
  const tmp = path.join(require('node:os').tmpdir(), 'hos-text-lint-' + process.pid + '.html');
  fs.writeFileSync(tmp, html);
  try { assert.deepStrictEqual(lowContrastText(stylesheetText(tmp)).map((h) => +h.split(':')[0]), [3]); } finally { fs.unlinkSync(tmp); }
});

test('no page stylesheet colours text with a literal that is unreadable in the light theme', () => {
  const byFile = lowContrastByFile();
  const problems = [];
  for (const [file, hits] of Object.entries(byFile)) {
    if (hits.length > (OLDER_TEXT[file] || 0)) problems.push(...hits.map((h) => `${file}:${h}`));
  }
  for (const [file, count] of Object.entries(OLDER_TEXT)) {
    const now = (byFile[file] || []).length;
    if (now < count) problems.push(`${file}: ${now} of its ${count} older lines left: lower its count in OLDER_TEXT`);
  }
  assert.deepStrictEqual(problems, [],
    'Text in a literal colour that is under 4.5:1 in the light theme (or a page variable whose ' +
    'light value is). Use a theme variable (var(--text), var(--text-muted), var(--text-on-accent), ' +
    'var(--info), var(--success), ...) or a page variable with a light value in a ' +
    ':root[data-theme="light"] rule; or, for text on a surface dark in BOTH themes, say why on ' +
    'the line with `theme-exempt: <reason>`.\n' + problems.join('\n'));
});
