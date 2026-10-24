// Characters past a field's limit, in red on dark red, on the website (2026-10-24): the shared
// mirror (web/shared/over-limit.js and .css), the website's twin of the desktop's
// src/gui/widgets/over_limit.rs, used by the chat's message box and the profile page's bio. The bio
// is a soft limit like the desktop's: no maxlength cutting a paste, and a save past 280 characters
// is refused with the reason instead of cut.
// Run: node --test scripts/tests/over-limit-web.test.js
//
// Seen red 2026-10-24 against the web/ before this (HOS_WEB_DIR): there was no shared/over-limit.js,
// the bio had maxlength="280", and the save cut it with substring(0, 280).
'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, '..', '..', 'web');
const read = (rel) => fs.readFileSync(path.join(WEB, rel), 'utf8');

// Just enough page for the mirror: elements with children, classes, styles and listeners.
function el(tag) {
  const e = {
    tagName: tag, children: [], style: {}, attrs: {}, listeners: {}, textContent: '', value: '', className: '',
    offsetLeft: 0, offsetTop: 0, offsetWidth: 300, offsetHeight: 80, scrollTop: 0,
    classList: {
      s: new Set(),
      add(c) { this.s.add(c); }, remove(c) { this.s.delete(c); }, contains(c) { return this.s.has(c); },
    },
    setAttribute(k, v) { e.attrs[k] = v; },
    addEventListener(t, f) { e.listeners[t] = f; },
    appendChild(c) { c.parentElement = e; e.children.push(c); return c; },
    insertBefore(c, ref) { c.parentElement = e; e.children.splice(Math.max(0, e.children.indexOf(ref)), 0, c); return c; },
    querySelector(sel) {
      const cls = sel.replace(':scope > .', '').replace(/^\./, '');
      return e.children.find((c) => c.className === cls) || null;
    },
  };
  return e;
}

function mirrorPage() {
  const box = el('div');
  const input = box.appendChild(el('textarea'));
  const document = {
    createElement: (t) => el(t),
    createTextNode: (t) => ({ nodeText: t }),
  };
  const win = { document, getComputedStyle: () => ({ backgroundColor: 'rgb(1, 2, 3)', font: '16px sans-serif' }), Array, String };
  win.window = win;
  vm.createContext(win);
  vm.runInContext(read('shared/over-limit.js'), win);
  assert.strictEqual(typeof win.hosOverLimit, 'function', 'shared/over-limit.js defines hosOverLimit');
  // The mirror appends text nodes and a <mark>: read them back in order.
  const shown = (m) => m.children.map((c) => (c.nodeText !== undefined ? c.nodeText : '[' + c.textContent + ']')).join('');
  return { win, input, box, shown };
}

test('the characters past the limit are marked, counted as the server counts them', () => {
  const p = mirrorPage();
  p.input.value = 'abc\u{1F600}de';
  assert.strictEqual(p.win.hosOverLimit(p.input, 4), true, 'six characters against four');
  const m = p.box.querySelector(':scope > .over-limit-mirror');
  assert.ok(m, 'a mirror behind the box');
  assert.strictEqual(m.style.display, 'block');
  assert.strictEqual(p.shown(m), 'abc\u{1F600}[de]\n', 'the emoji is one character, within the limit; "de" is marked');
  assert.strictEqual(m.children.find((c) => c.className === 'over-limit-chars').tagName, 'mark');
  assert.ok(p.input.classList.contains('over-limit-typing'), "the box's own words go transparent");
  assert.strictEqual(m.style.backgroundColor, 'rgb(1, 2, 3)', "the mirror takes the box's background");
  assert.strictEqual(m.attrs['aria-hidden'], 'true', 'screen readers read the box, not the mirror');
  p.input.value = 'abcd';
  assert.strictEqual(p.win.hosOverLimit(p.input, 4), false, 'at the limit is within it');
  assert.strictEqual(m.style.display, 'none');
  assert.ok(!p.input.classList.contains('over-limit-typing'));
});

test('the styles are shared, in the theme colours, and the chat uses the shared mirror', () => {
  const css = read('shared/over-limit.css');
  assert.match(css, /\.over-limit-chars \{ color: var\(--danger\); background: var\(--danger-bg\);/);
  assert.match(css, /\.over-limit-typing \{ color: transparent !important;/);
  assert.doesNotMatch(read('chat/inputs.css'), /\.over-limit-mirror \{/, 'no second copy in the chat');
  const page = read('chat/index.html');
  const shared = page.indexOf('/shared/over-limit.js');
  assert.ok(shared > 0 && shared < page.indexOf('/chat/chat-ui.js'), 'loaded before chat-ui.js');
  assert.ok(page.includes('/shared/over-limit.css'));
  assert.match(read('chat/chat-ui.js'), /return hosOverLimit\(input, lineLimitApplies\(\) \? getMaxMsgLength\(\) : Infinity\);/);
});

test('the profile bio is a soft limit: marked past 280, and a save past it is refused with why', () => {
  const html = read('pages/profile.html');
  const bio = html.match(/<textarea id="f-bio"[^>]*>/);
  assert.ok(bio, 'the bio box');
  assert.doesNotMatch(bio[0], /maxlength/, 'no maxlength cutting a paste');
  for (const src of ['/shared/over-limit.js', '/shared/i18n.js', '/shared/i18n-en.js']) assert.ok(html.includes('<script src="' + src), src + ' is loaded');
  assert.ok(html.includes('/shared/over-limit.css'));
  assert.match(html, /window\.hosOverLimit\(bio, BIO_MAX\)/, 'the bio box shows its excess');
  assert.match(html, /var len = Array\.from\(bio\.value\)\.length;/, 'the count is in characters');
  assert.match(html, /var bioChars = Array\.from\(val\('bio'\)\)\.length;\s*\n\s*if \(bioChars > BIO_MAX\) \{[\s\S]{0,200}i18n\.t\('profile\.bio_over_limit'[\s\S]{0,200}return;/, 'a save past it is refused with why');
  assert.doesNotMatch(html, /val\('bio'\)\.substring\(0, 280\)/, 'never cut');
  const words = JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'data', 'i18n', 'en.json'), 'utf8'));
  assert.match(words.profile.bio_over_limit, /\{\{n\}\}[\s\S]*\{\{limit\}\}/, 'the words name both numbers');
});
