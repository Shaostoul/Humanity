#!/usr/bin/env node
/**
 * gen-navigation.js: the website's top menu, from data/gui/navigation.json (docs/design/navigation.md,
 * rung 1, 2026-11-03).
 *
 * data/gui/navigation.json is the one menu for both apps: the desktop app reads it directly
 * (src/gui/navigation.rs), the website through this script, which writes the part the website
 * draws into web/shared/shell.js, between the BEGIN and END GENERATED NAVIGATION lines, with each
 * entry's English words from data/i18n/en.json. Why a written copy and not a fetch: shell.js builds
 * the menu the moment a page loads, before any fetch could answer, and the desktop app's offline
 * copy of the website (scripts/bundle-web.js) carries web/ and not data/; i18n-en.js is the same
 * arrangement for the words (scripts/gen-i18n.js).
 *
 * In rung 1 the website draws `today_row.web` (what its single row shows until rung 5 draws the two
 * rows): each group's CSS class and its tabs, each tab's address, icon (a name in
 * web/shared/icons.js; `humanity` is the brand H), words and the key the page lights it by
 * (`web_active`, else its id).
 *
 * It refuses a file the website cannot draw, naming the entry (the desktop's own rules, pages
 * included, are src/gui/navigation.rs's): an id twice, a word key en.json lacks, an icon icons.js
 * does not have, an address that does not start with /, a row naming no entry or one with no
 * address. Never edit the block by hand: edit the data file (or a word in en.json) and run this.
 * scripts/tests/navigation-web.test.js fails while the block is out of date.
 *
 * Usage:
 *   node scripts/gen-navigation.js           # rewrite the block in web/shared/shell.js
 *   node scripts/gen-navigation.js --check   # exit 1 when it is out of date
 */
'use strict';

const fs = require('fs');
const path = require('path');
const vm = require('vm');

const ROOT = path.resolve(__dirname, '..');
const NAV_PATH = path.join(ROOT, 'data', 'gui', 'navigation.json');
const EN_PATH = path.join(ROOT, 'data', 'i18n', 'en.json');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const SHELL_PATH = path.join(WEB, 'shared', 'shell.js');
const ICONS_PATH = path.join(WEB, 'shared', 'icons.js');

const BEGIN = '  // BEGIN GENERATED NAVIGATION';
const END = '  // END GENERATED NAVIGATION';

/** en.json's words for a dotted key, or undefined. */
function word(en, key) {
  let v = en;
  for (const part of String(key).split('.')) v = v && typeof v === 'object' ? v[part] : undefined;
  return typeof v === 'string' ? v : undefined;
}

/** The icon names web/shared/icons.js draws. */
function iconNames(iconsText) {
  const win = {};
  const ctx = {
    window: win,
    localStorage: { getItem: () => null, setItem() {} },
    document: { documentElement: { style: { setProperty() {} } } },
  };
  vm.createContext(ctx);
  vm.runInContext(iconsText, ctx, { filename: 'icons.js' });
  return new Set(win.hosIconNames ? win.hosIconNames() : []);
}

/** Every entry of the file: each category, its items, then what is outside the rows. */
function places(nav) {
  const out = [];
  for (const c of nav.categories || []) {
    out.push(c);
    for (const i of c.items || []) out.push(i);
  }
  for (const o of nav.outside || []) out.push(o);
  return out;
}

/**
 * The website's row from the file's text, checked: [{ cls, tabs: [{ id, href, icon, label, key, active }] }].
 * Throws an Error naming the entry when the website cannot draw it.
 */
function webRow(navText, enText, iconsText) {
  const nav = JSON.parse(navText);
  const en = JSON.parse(enText);
  const icons = iconNames(iconsText);
  const byId = new Map();
  const addresses = new Set();
  for (const p of places(nav)) {
    const who = 'the entry `' + p.id + '`';
    if (typeof p.id !== 'string' || !/^[a-z0-9_]+$/.test(p.id)) throw new Error(who + ': an id is lower-case letters, digits and _');
    if (byId.has(p.id)) throw new Error(who + ' is in the file twice');
    byId.set(p.id, p);
    for (const what of ['label', 'hint']) {
      if (p[what] !== undefined && p[what] !== null && word(en, p[what]) === undefined) {
        throw new Error(who + ': its ' + what + ' `' + p[what] + '` is not a key in data/i18n/en.json');
      }
    }
    if (p.icon !== undefined && p.icon !== null && p.icon !== 'humanity' && !icons.has(p.icon)) {
      throw new Error(who + ': its icon `' + p.icon + '` is not in web/shared/icons.js');
    }
    if (p.web !== undefined && p.web !== null) {
      if (typeof p.web !== 'string' || !p.web.startsWith('/')) throw new Error(who + ': the website address `' + p.web + '` does not start with /');
      if (addresses.has(p.web)) throw new Error(who + ': the website address `' + p.web + '` is another entry\'s already');
      addresses.add(p.web);
    }
  }
  const today = (nav.today_row && nav.today_row.web) || [];
  const activeOf = (nav.today_row && nav.today_row.web_active) || {};
  const shown = new Set();
  const row = today.map((g) => {
    if (typeof g.class !== 'string' || !g.class.trim()) throw new Error('today\'s website row has a group with no class');
    return {
      cls: g.class,
      tabs: (g.items || []).map((id) => {
        const p = byId.get(id);
        if (!p) throw new Error('today\'s website row names `' + id + '`, which is no entry');
        if (!p.web || !p.label || !p.icon) throw new Error('today\'s website row names `' + id + '`, which has no website address, no words or no icon');
        if (shown.has(id)) throw new Error('today\'s website row shows `' + id + '` twice');
        shown.add(id);
        return { id, href: p.web, icon: p.icon, label: word(en, p.label), key: p.label, active: activeOf[id] || id };
      }),
    };
  });
  for (const id of Object.keys(activeOf)) {
    if (!shown.has(id)) throw new Error('today\'s website row: web_active names `' + id + '`, which is not in that row');
  }
  return row;
}

/** The generated block, its BEGIN and END lines included. */
function block(row) {
  const q = (s) => JSON.stringify(String(s));
  const lines = [
    BEGIN + ' (scripts/gen-navigation.js, from data/gui/navigation.json and data/i18n/en.json: do not edit)',
    "  var NAV_SOURCE = 'data/gui/navigation.json'; // the file this is written from (tests/page_parity_lint.rs looks for this line)",
    '  var NAV_ROW = [',
  ];
  row.forEach((g, gi) => {
    lines.push('    { cls: ' + q(g.cls) + ', tabs: [');
    g.tabs.forEach((t, ti) => {
      lines.push(
        '      { id: ' + q(t.id) + ', href: ' + q(t.href) + ', icon: ' + q(t.icon) + ', label: ' + q(t.label) +
          ', key: ' + q(t.key) + ', active: ' + q(t.active) + ' }' + (ti < g.tabs.length - 1 ? ',' : '') +
          ' // i18n-exempt: en.json\'s words for its key',
      );
    });
    lines.push('    ] }' + (gi < row.length - 1 ? ',' : ''));
  });
  lines.push('  ];', END);
  return lines.join('\n');
}

/** shell.js's text with its block rewritten from the file. Throws when the markers are not there once each. */
function rewrite(shellText, row) {
  const crlf = shellText.includes('\r\n');
  const text = shellText.replace(/\r\n/g, '\n');
  const a = text.indexOf(BEGIN);
  const b = text.indexOf(END);
  if (a < 0 || b < a || text.indexOf(BEGIN, a + 1) >= 0 || text.indexOf(END, b + 1) >= 0) {
    throw new Error('web/shared/shell.js must hold the BEGIN and END GENERATED NAVIGATION lines once each');
  }
  const out = text.slice(0, a) + block(row) + text.slice(b + END.length);
  return crlf ? out.replace(/\n/g, '\r\n') : out;
}

/**
 * The row shell.js's text holds (its NAV_ROW, as written): [{ cls, tabs: [{ id, href, icon, label, key, active }] }].
 * For tests that ask what the website's menu shows (the Apps, Files and chat tests).
 */
function shellRow(shellText) {
  const m = /var NAV_ROW = (\[[\s\S]*?\r?\n {2}\]);/.exec(shellText);
  if (!m) throw new Error('web/shared/shell.js holds no NAV_ROW');
  // Through JSON, so the arrays and objects are this realm's (assert.deepStrictEqual compares prototypes).
  return JSON.parse(JSON.stringify(vm.runInNewContext('(' + m[1] + ')')));
}

function read(p) {
  return fs.readFileSync(p, 'utf8');
}

/** shell.js as it should be, from the files on disk. */
function wanted() {
  return rewrite(read(SHELL_PATH), webRow(read(NAV_PATH), read(EN_PATH), read(ICONS_PATH)));
}

function main() {
  let want;
  try {
    want = wanted();
  } catch (e) {
    console.error('gen-navigation: data/gui/navigation.json: ' + e.message);
    process.exit(1);
  }
  const have = read(SHELL_PATH);
  if (process.argv.includes('--check')) {
    if (have !== want) {
      console.error('web/shared/shell.js\'s menu is out of date with data/gui/navigation.json: run node scripts/gen-navigation.js');
      process.exit(1);
    }
    console.log('web/shared/shell.js\'s menu is current');
    return;
  }
  if (have === want) {
    console.log('web/shared/shell.js\'s menu is current');
    return;
  }
  fs.writeFileSync(SHELL_PATH, want);
  console.log('wrote the menu into ' + path.relative(ROOT, SHELL_PATH));
}

module.exports = { webRow, block, rewrite, wanted, word, shellRow, NAV_PATH, EN_PATH, SHELL_PATH, ICONS_PATH, BEGIN, END };

if (require.main === module) main();
