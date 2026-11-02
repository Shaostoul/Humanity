// The unfinished-software warning's mute on the website (2026-11-02, docs/design/app-bottom-strip.md,
// section 3.5): web/shared/shell.js's Mute, the mirror of the desktop app's
// src/gui/bottom_strip/mute.rs. The operator, 2026-11-02: "mute for this session, one day, one
// week"; and of the title screen, "It should stay on the title. I think it helps keep it honest."
//
// Run: node --test scripts/tests/wip-mute-web.test.js   (listed in `just rig-tests`)
// HOS_WEB_DIR points it at another copy of web/ (how each test was seen red).
//
// What it proves:
//  1. The rule is the desktop app's: shell.js's hosWipMuted, run on the same cases the Rust test
//     reads (scripts/tests/fixtures/wip-mute-cases.json).
//  2. Mute says what en.json says, opens a menu (hidden until then), and the choices are the data
//     file the desktop reads (data/gui/wip_mute.json), each word a key en.json has.
//  3. A day's mute kept in localStorage hides the strip and gives it no room (--hos-wip-h 0px); it
//     shows again at its time, with a new version, and a timer is set for its end.
//  4. This session's mute lives in sessionStorage: it hides the strip in its tab and is gone in a
//     new one (the next launch); a session-shaped record in localStorage is not taken.
//  5. The front page and the warning's own Library page show the strip whatever the mute; another
//     Library document does not, and moving between them (no hashchange: the Library's own event)
//     shows and hides it.
//  6. Mute and its choices take clicks: pressing Mute opens the menu with the three choices, pressing
//     For one day keeps a day's mute and hides the strip; muted, the menu offers Show it again.
//  7. A blocked storage means the strip shows, and choosing a mute throws nothing.
//
// Red first, 2026-11-02, each break made in a copy of web/ and this test run on it with
// HOS_WEB_DIR (each passing again on the real web/):
//  1, 3: hosWipMuted ignoring the version: "this session, but a new version started", "a new
//     version shows it again"; with `now <= until`: "one day, at its time", "back at the day's end".
//  3, 4, 5, 6: syncHeight measuring the hidden strip as before: "muted, the strip takes no room".
//  5: alwaysHere answering false: "the front page (/) shows it while muted"; library-app.js
//     without its announcement: "the Library announces a document opened without a hashchange".
//  4: a session-shaped record taken from localStorage: "a session mute found in localStorage is
//     not taken".
//  6: the menu's click handler not choosing: For one day kept nothing.
//  7: the mute's storage reads unguarded; and before that, on the first run of this test, the
//     shell's own nav-mode read (unguarded, at load) threw on a blocked storage and stopped the
//     shell before the strip was built, so those reads are guarded now too.

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const H = require('./wip-shell-harness.js');

const shell = fs.readFileSync(path.join(H.WEB, 'shared', 'shell.js'), 'utf8').replace(/\r\n/g, '\n');
const cases = JSON.parse(fs.readFileSync(path.join(__dirname, 'fixtures', 'wip-mute-cases.json'), 'utf8'));
const choicesFile = JSON.parse(fs.readFileSync(path.join(H.ROOT, 'data', 'gui', 'wip_mute.json'), 'utf8'));
const V = '0.1547.0';
const NOW = 1800000000;
const KEY = 'hos_wip_mute';

function functionSource(name) {
  const start = shell.indexOf('function ' + name + '(');
  assert.ok(start >= 0, 'shell.js has no function ' + name);
  const end = shell.indexOf('\n  }\n', start);
  assert.ok(end > start, 'could not find the end of ' + name);
  return shell.slice(start, end + 4);
}

function stripMarkup() {
  const at = shell.indexOf('(function wipStrip()');
  const from = shell.indexOf('holder.innerHTML =', at);
  const to = shell.indexOf(';\n', from);
  return vm.runInNewContext(shell.slice(from + 'holder.innerHTML ='.length, to), {});
}

test('1. the website mutes for exactly the cases the desktop app does', () => {
  const ctx = {};
  vm.runInNewContext(functionSource('hosWipMuted') + '\nthis.hosWipMuted = hosWipMuted;', ctx);
  assert.ok(cases.cases.length >= 10, 'the shared cases are nearly empty');
  for (const c of cases.cases) {
    assert.strictEqual(ctx.hosWipMuted(c.mute, cases.version, c.now), c.muted, c.why);
  }
});

test('2. Mute says what en.json says, its menu starts closed, and its choices are the shared data file', () => {
  const html = stripMarkup();
  const m = /<button type="button" class="hos-wip-mute"([^>]*)>([^<]*)<\/button>/.exec(html);
  assert.ok(m, 'the strip has its Mute button');
  assert.strictEqual(m[2], H.EN.wip.mute, "Mute's word is en.json's");
  assert.match(m[1], /data-i18n="wip\.mute"/);
  assert.match(m[1], /data-i18n-tip="wip\.mute_hint"/);
  assert.strictEqual(/data-tip-desc="([^"]*)"/.exec(m[1])[1], H.EN.wip.mute_hint, "Mute's hint is en.json's");
  assert.match(m[1], /aria-haspopup="menu"/);
  assert.match(m[1], /aria-expanded="false"/);
  assert.match(html, /class="hos-wip-menu" role="menu" hidden>/, 'the menu is closed until Mute is pressed');
  // The link's hint is en.json's too, now that the page it opens also says why the bar is here.
  assert.strictEqual(/class="hos-wip-more"[^>]*data-tip-desc="([^"]*)"/.exec(html)[1], H.EN.wip.more_hint);
  // The page that shows it whatever the mute is the one its link opens, and the desktop's.
  const slug = /var WIP_DETAILS_SLUG = '([^']+)';/.exec(shell);
  assert.ok(slug, 'shell.js names the warning\'s own page');
  assert.ok(html.includes('href="/library#' + slug[1] + '"'), 'the link opens that page');
  const rust = fs.readFileSync(path.join(H.ROOT, 'src', 'gui', 'bottom_strip.rs'), 'utf8');
  assert.ok(rust.includes('pub const DETAILS_SLUG: &str = "' + slug[1] + '";'), 'the desktop app\'s page is the same');
  // The choices: the desktop's file, every word a key.
  assert.match(shell, /fetch\('\/data\/gui\/wip_mute\.json'/, 'the shell reads the shared choices');
  assert.deepStrictEqual(choicesFile.mute_for.map((c) => c.id), ['session', 'day', 'week']);
  for (const c of choicesFile.mute_for) {
    for (const k of [c.label, c.hint]) assert.notStrictEqual(H.t(k), k, k + ' is a key en.json has');
  }
  for (const k of ['wip.muted', 'wip.muted_hint', 'wip.show_again', 'wip.show_again_hint', 'wip.muted_until', 'wip.muted_session', 'wip.settings_label', 'wip.settings_showing', 'wip.settings_web_hint', 'wip.mute_unavailable']) {
    assert.notStrictEqual(H.t(k), k, k + ' is a key en.json has');
  }
});

test('3. a day\'s mute hides the strip and takes its room, and ends at its time or with a new version', () => {
  const day = { until: NOW + 86400, version: V };
  const muted = H.loadShell({ nowSecs: NOW + 60, local: { [KEY]: day } });
  assert.ok(muted.strip, 'the strip is built (muted is not gone: Settings and the front page need it)');
  assert.strictEqual(muted.strip.hidden, true, 'a day\'s mute hides the strip');
  assert.strictEqual(muted.rootStyle['--hos-wip-h'], '0px', 'muted, the strip takes no room');
  const timer = muted.timers.find((x) => x.ms > 86000 * 1000);
  assert.ok(timer, 'a timer brings the strip back at the mute\'s end: ' + JSON.stringify(muted.timers.map((x) => x.ms)));
  // At its time, the next page shows it; and so does the timer, on the page left open.
  const over = H.loadShell({ nowSecs: NOW + 86400, local: { [KEY]: day } });
  assert.strictEqual(over.strip.hidden, false, 'back at the day\'s end');
  assert.strictEqual(over.rootStyle['--hos-wip-h'], '27px');
  muted.clock.now = (NOW + 86400) * 1000;
  timer.f();
  assert.strictEqual(muted.strip.hidden, false, 'the timer brought it back on the open page');
  assert.strictEqual(muted.rootStyle['--hos-wip-h'], '27px', 'and its room with it');
  const newer = H.loadShell({ nowSecs: NOW + 60, version: '0.1548.0', local: { [KEY]: day } });
  assert.strictEqual(newer.strip.hidden, false, 'a new version shows it again');
});

test('4. this session\'s mute is the tab\'s: gone in a new one, and never taken from lasting storage', () => {
  const tab = H.loadShell({ nowSecs: NOW, session: { [KEY]: { version: V } } });
  assert.strictEqual(tab.strip.hidden, true, 'muted for this session in its tab');
  assert.strictEqual(tab.rootStyle['--hos-wip-h'], '0px');
  const next = H.loadShell({ nowSecs: NOW });
  assert.strictEqual(next.strip.hidden, false, 'a new tab (the next launch) shows it');
  const lasting = H.loadShell({ nowSecs: NOW, local: { [KEY]: { version: V } } });
  assert.strictEqual(lasting.strip.hidden, false, 'a session mute found in localStorage is not taken');
});

test('5. the front page and the warning\'s own page show it whatever the mute; another document does not', () => {
  const week = { [KEY]: { until: NOW + 604800, version: V } };
  for (const p of ['/', '/index.html', '/pages/index.html']) {
    const front = H.loadShell({ nowSecs: NOW + 60, path: p, local: week });
    assert.strictEqual(front.strip.hidden, false, 'the front page (' + p + ') shows it while muted');
    assert.strictEqual(front.rootStyle['--hos-wip-h'], '27px');
  }
  const own = H.loadShell({ nowSecs: NOW + 60, path: '/library', hash: '#unfinished-software', local: week });
  assert.strictEqual(own.strip.hidden, false, 'the warning\'s own page shows it while muted');
  const section = H.loadShell({ nowSecs: NOW + 60, path: '/library', hash: '#unfinished-software/muting-it', local: week });
  assert.strictEqual(section.strip.hidden, false, 'and a section of it');
  // The Library moves between documents with pushState (no hashchange) and says so.
  own.ctx.location.hash = '#roadmap';
  own.fire('hos-library-doc');
  assert.strictEqual(own.strip.hidden, true, 'another Library document is muted like any page');
  assert.strictEqual(own.rootStyle['--hos-wip-h'], '0px');
  own.ctx.location.hash = '#unfinished-software';
  own.fire('popstate');
  assert.strictEqual(own.strip.hidden, false, 'Back to the warning\'s page shows it again');
  const lib = fs.readFileSync(path.join(H.WEB, 'pages', 'library-app.js'), 'utf8');
  assert.match(lib, /new Event\('hos-library-doc'\)/, 'the Library announces a document opened without a hashchange');
});

test('6. Mute and its choices take clicks: the menu opens, For one day mutes, Show it again unmutes', async () => {
  const page = H.loadShell({ nowSecs: NOW });
  assert.strictEqual(page.strip.hidden, false);
  assert.strictEqual(page.menu.hidden, true, 'the menu starts closed');
  assert.ok((page.mute.listeners.click || []).length >= 1, 'Mute listens for a click');
  page.mute.listeners.click.forEach((f) => f({ type: 'click' }));
  await page.settle();
  assert.strictEqual(page.menu.hidden, false, 'pressing Mute opened its menu');
  assert.strictEqual(page.mute.attrs['aria-expanded'], 'true');
  const items = H.menuItems(page.menu);
  assert.deepStrictEqual(items.map((i) => i.id), ['session', 'day', 'week'], 'the menu offers the shared choices');
  assert.deepStrictEqual(items.map((i) => i.label), [H.EN.wip.mute_session, H.EN.wip.mute_day, H.EN.wip.mute_week], 'in en.json\'s words');
  for (const i of items) assert.ok(i.attrs['data-i18n-tip'] && i.attrs['data-tip-desc'], i.id + ' has its hint');
  H.clickItem(page.menu, items.find((i) => i.id === 'day'));
  assert.deepStrictEqual(JSON.parse(page.local.get(KEY)), { until: NOW + 86400, version: V }, 'For one day kept a day\'s mute');
  assert.strictEqual(page.strip.hidden, true, 'and hid the strip at once');
  assert.strictEqual(page.rootStyle['--hos-wip-h'], '0px', 'with its room');
  assert.strictEqual(page.menu.hidden, true, 'the menu closed');
  assert.ok(page.events.includes('hos-wip-mute'), 'Settings is told');

  // On the front page it still shows: Mute reads Muted, and its menu offers Show it again.
  const front = H.loadShell({ nowSecs: NOW + 60, path: '/', local: { [KEY]: { until: NOW + 86400, version: V } } });
  assert.strictEqual(front.mute.textContent, H.EN.wip.muted, 'muted, the front page\'s Mute reads Muted');
  front.mute.listeners.click.forEach((f) => f({ type: 'click' }));
  await front.settle();
  const again = H.menuItems(front.menu).find((i) => i.again);
  assert.ok(again, 'muted, the menu offers Show it again');
  assert.strictEqual(again.label, H.EN.wip.show_again);
  assert.match(String(front.menu.innerHTML), /Muted until \d{4}-\d\d-\d\d \d\d:\d\d/, 'and says until when');
  H.clickItem(front.menu, again);
  assert.strictEqual(front.local.has(KEY), false, 'Show it again took the mute away');
  assert.strictEqual(front.mute.textContent, H.EN.wip.mute, 'and Mute reads Mute again');

  // This session, chosen in the menu, lands in sessionStorage and takes a day's out.
  const tab = H.loadShell({ nowSecs: NOW, local: { [KEY]: { until: NOW + 86400, version: V } } });
  tab.ctx.hosWipMute.showAgain();
  tab.mute.listeners.click.forEach((f) => f({ type: 'click' }));
  await tab.settle();
  H.clickItem(tab.menu, H.menuItems(tab.menu).find((i) => i.id === 'session'));
  assert.deepStrictEqual(JSON.parse(tab.session.get(KEY)), { version: V }, 'this session is kept for the tab');
  assert.strictEqual(tab.local.has(KEY), false, 'and nothing lasting');
  assert.strictEqual(tab.strip.hidden, true);
});

test('7. a blocked storage means the strip shows, and choosing throws nothing', async () => {
  const page = H.loadShell({ nowSecs: NOW, blocked: true });
  assert.strictEqual(page.strip.hidden, false, 'nothing can be read, so the strip shows');
  assert.strictEqual(page.rootStyle['--hos-wip-h'], '27px');
  page.mute.listeners.click.forEach((f) => f({ type: 'click' }));
  await page.settle();
  assert.doesNotThrow(() => H.clickItem(page.menu, H.menuItems(page.menu).find((i) => i.id === 'week')));
  assert.strictEqual(page.strip.hidden, false, 'nothing was kept, so the strip still shows');
});

test('8. Settings > Notifications > Muted has the row, drawn through the shell\'s own mute', () => {
  const html = fs.readFileSync(path.join(H.WEB, 'pages', 'settings.html'), 'utf8');
  assert.match(html, /<div id="wip-mute-row"><\/div>/, 'the Muted group has the warning\'s row');
  const app = fs.readFileSync(path.join(H.WEB, 'pages', 'settings-app.js'), 'utf8');
  assert.match(app, /function renderWipMuteRow\(\)/);
  assert.match(app, /window\.hosWipMute/, 'it reads and changes the mute through the shell');
  assert.match(app, /addEventListener\('hos-wip-mute', renderWipMuteRow\)/, 'and follows the bar\'s own Mute');
  assert.match(app, /api\.showAgain\(\)/, 'muted, it offers Show it again');
  assert.match(app, /api\.choose\(pick\)/, 'unmuted, it offers the choices');
});
