// The website's top menu is drawn from data/gui/navigation.json (docs/design/navigation.md, rung 1,
// 2026-11-03): the parity pair's website half. The desktop half is src/gui/navigation_tests.rs
// (`the_desktop_row_is_drawn_from_the_file`), which holds the desktop's row to the same file.
//
// Run: node --test scripts/tests/navigation-web.test.js
//
// What it proves:
//  1. The menu written into web/shared/shell.js is what scripts/gen-navigation.js writes from the
//     file and data/i18n/en.json today (a file edited without running it fails here).
//  2. The row web/shared/shell.js draws, run whole as a page loads it (wip-shell-harness.js), is
//     the file's today_row.web: its groups' classes in order, each tab's address, words and icon,
//     and the tab a page lights (web_active's older keys), and the words a tab still shows where
//     they differ from its entry's (web_label: the own-life category is "Life" on the desktop since
//     rung 2, and the website's tab, which opens its Home page, still says Home until rung 5). Read
//     from the file here, not through the generator, so a generator that wrote the wrong thing fails
//     too.
//  3. A file the website cannot draw is refused, naming the entry.
//
// HOS_WEB_DIR points the shell at another copy of web/ (how test 2 was seen red).

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const GEN = require(path.join(ROOT, 'scripts', 'gen-navigation.js'));
const { loadShell, EN, WEB } = require('./wip-shell-harness.js');

const NAV_TEXT = fs.readFileSync(GEN.NAV_PATH, 'utf8');
const NAV = JSON.parse(NAV_TEXT);

/** en.json's words for a dotted key. */
function words(key) {
  const w = GEN.word(EN, key);
  assert.ok(w !== undefined, key + ' is a key in data/i18n/en.json');
  return w;
}

/** web/shared/icons.js's drawing of an icon, as the shell asks for it. */
const ICONS = (() => {
  const ctx = { localStorage: { getItem: () => null, setItem() {} }, document: { documentElement: { style: { setProperty() {} } } } };
  ctx.window = ctx;
  vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(path.join(WEB, 'shared', 'icons.js'), 'utf8'), ctx);
  return ctx;
})();

/** Every entry of the file by id. */
function byId(nav) {
  const m = new Map();
  for (const c of nav.categories) {
    m.set(c.id, c);
    for (const i of c.items || []) m.set(i.id, i);
  }
  for (const o of nav.outside || []) m.set(o.id, o);
  return m;
}

/** The row in the shell's markup: [{ cls, tabs: [{ href, active, tip, icon, label }] }]. */
function drawnRow(html) {
  const pieces = html.split('<div class="nav-divider"></div>');
  pieces.shift(); // the front door, which the shell draws itself
  pieces[pieces.length - 1] = pieces[pieces.length - 1].split('<div class="spacer"></div>')[0];
  return pieces.map((piece) => {
    const g = /^<span class="([^"]+)">/.exec(piece);
    assert.ok(g, 'a group starts with its class: ' + piece.slice(0, 80));
    const tabs = [];
    const re = /<a href="([^"]*)" class="tab( active)?" data-tip="([^"]*)"><span class="tab-icon"[^>]*>([\s\S]*?)<\/span><span class="tab-label">([^<]*)<\/span><\/a>/g;
    let m;
    while ((m = re.exec(piece))) tabs.push({ href: m[1], active: !!m[2], tip: m[3], icon: m[4], label: m[5] });
    return { cls: g[1], tabs };
  });
}

// Seen red 2026-11-03 with the file's Onboarding and Tasks swapped and the block not written again:
// "web/shared/shell.js's menu is out of date: run node scripts/gen-navigation.js".
test('the menu written into web/shared/shell.js is current with data/gui/navigation.json', () => {
  const have = fs.readFileSync(GEN.SHELL_PATH, 'utf8');
  assert.strictEqual(have, GEN.wanted(), 'web/shared/shell.js\'s menu is out of date: run node scripts/gen-navigation.js');
});

// Seen red 2026-11-03 with the hand-written shell.js of the commit before (HOS_WEB_DIR at a copy
// of it, where it passed with the shipped file) and the file's Onboarding and Tasks swapped: "the
// website's row is the file's (groups and addresses), on /pages/notes.html", the sixth group
// "+ '/onboarding', '/tasks', - '/onboarding'"; with the row drawn from the file (and the block
// written again) the same swap passes. Rung 2 (2026-11-03): seen red with the own-life category's
// words changed to "Life" and this test not yet reading web_label: "own_life's words are en.json's
// Life", 'Home' !== 'Life' (the website's tab, which the generator kept on Home).
test('the row shell.js draws is the file\'s today_row.web, words, icons and lit tab included', () => {
  const entries = byId(NAV);
  const activeOf = NAV.today_row.web_active || {};
  const labelOf = NAV.today_row.web_label || {};
  // web_icon: the website tab's own icon where it differs (Play is Download there, 2026-11-04).
  const iconOf = NAV.today_row.web_icon || {};
  const want = NAV.today_row.web.map((g) => ({
    cls: g.class,
    tabs: g.items.map((id) => {
      const e = entries.get(id);
      return { id, href: e.web, label: words(labelOf[id] || e.label), icon: iconOf[id] || e.icon, active: activeOf[id] || id };
    }),
  }));
  // A page for each tab, lighting it the way its page does (its script tag's data-active, the key
  // web_active gives), and one page with no tab: the right tab, and only it, is lit.
  const pages = [
    ['/pages/notes.html', 'notes', null],
    ...want.flatMap((g) => g.tabs.map((t) => [t.href, t.active, t.href])),
  ];
  for (const [page, key, litHref] of pages) {
    const { navHtml } = loadShell({ path: page, active: key, icons: true });
    assert.ok(navHtml, 'the shell wrote its menu on ' + page);
    const drawn = drawnRow(navHtml);
    assert.deepStrictEqual(
      drawn.map((g) => [g.cls, g.tabs.map((t) => t.href)]),
      want.map((g) => [g.cls, g.tabs.map((t) => t.href)]),
      'the website\'s row is the file\'s (groups and addresses), on ' + page,
    );
    want.forEach((g, gi) => g.tabs.forEach((t, ti) => {
      const d = drawn[gi].tabs[ti];
      assert.strictEqual(d.label, t.label, t.id + '\'s words are en.json\'s ' + t.label);
      assert.strictEqual(d.tip, t.label, t.id + '\'s hover words');
      const icon = t.icon === 'humanity' ? 'H' : ICONS.hosIcon(t.icon, 15);
      assert.strictEqual(d.icon, icon, t.id + '\'s icon is icons.js\'s ' + t.icon);
    }));
    const lit = drawn.flatMap((g) => g.tabs).filter((t) => t.active).map((t) => t.href);
    assert.deepStrictEqual(lit, litHref ? [litHref] : [], 'the lit tab on ' + page + ' (data-active ' + key + ')');
  }
});

// Seen red 2026-11-03 with webRow's rules switched off (it returned before checking): all ten
// "was read" ("an id twice: was read", "a word key en.json lacks: was read", ...).
test('a file the website cannot draw is refused, naming the entry', () => {
  const en = fs.readFileSync(GEN.EN_PATH, 'utf8');
  const icons = fs.readFileSync(GEN.ICONS_PATH, 'utf8');
  const entry = (v, id) => byId(v).get(id);
  const cases = [
    ['an id twice', (v) => { entry(v, 'tasks').id = 'maps'; }, 'the entry `maps` is in the file twice'],
    ['a word key en.json lacks', (v) => { entry(v, 'tasks').label = 'nav.taks'; }, 'the entry `tasks`: its label `nav.taks` is not a key in data/i18n/en.json'],
    ['an icon icons.js lacks', (v) => { entry(v, 'tasks').icon = 'tasklistt'; }, 'the entry `tasks`: its icon `tasklistt` is not in web/shared/icons.js'],
    ['an address without its slash', (v) => { entry(v, 'tasks').web = 'tasks'; }, 'the website address `tasks` does not start with /'],
    ['an address twice', (v) => { entry(v, 'tasks').web = '/maps'; }, 'the website address `/maps` is another entry\'s already'],
    ['a row naming no entry', (v) => { v.today_row.web[0].items[0] = 'plya'; }, 'today\'s website row names `plya`, which is no entry'],
    ['a row naming a page with no address', (v) => { v.today_row.web[0].items[0] = 'studio'; }, 'today\'s website row names `studio`, which has no website address'],
    ['a tab twice', (v) => { v.today_row.web[1].items[0] = 'play'; }, 'today\'s website row shows `play` twice'],
    ['a group with no class', (v) => { v.today_row.web[0].class = ' '; }, 'today\'s website row has a group with no class'],
    ['web_active for a tab not in the row', (v) => { v.today_row.web_active.studio = 'studio'; }, 'web_active names `studio`'],
    ['web_label for a tab not in the row', (v) => { v.today_row.web_label.studio = 'nav.studio'; }, 'web_label names `studio`'],
    ['web_label with a key en.json lacks', (v) => { v.today_row.web_label.own_life = 'nav.hoem'; }, 'the key `nav.hoem`, which is not in data/i18n/en.json'],
  ];
  const letThrough = [];
  for (const [what, breakIt, says] of cases) {
    const v = JSON.parse(NAV_TEXT);
    breakIt(v);
    try {
      GEN.webRow(JSON.stringify(v), en, icons);
      letThrough.push(what + ': was read');
    } catch (e) {
      if (!String(e.message).includes(says)) letThrough.push(what + ': refused, but saying ' + JSON.stringify(e.message));
    }
  }
  assert.throws(() => GEN.webRow('{ "categories": [', en, icons), 'a file cut short is refused');
  assert.deepStrictEqual(letThrough, [], 'every breakage refused, naming it');
});
