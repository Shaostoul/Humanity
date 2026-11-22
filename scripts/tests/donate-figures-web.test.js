// The website's Donate page shows the server's published monthly figures (2026-11-22,
// docs/design/live-funding.md; the native twin is src/gui/pages/donate_figures.rs): each channel
// with its amount and date, each cost, and the sentence "Donations cover $20 of $241 a month.",
// and nothing at all when a server publishes none.
//
// Run: node --test scripts/tests/donate-figures-web.test.js
//
// Seen red 2026-11-22: with okLink() accepting every link, "a bad row was read" (the rogue-server
// test); with render() not hiding the card when there are no figures, "the card is shown with no
// figures".

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');
const F = require(path.join(ROOT, 'web', 'pages', 'donate-figures.js'));
const W = require(path.join(ROOT, 'scripts', 'i18n-web.js'));
const EN = W.flatten(JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'i18n', 'en.json'), 'utf8')));

// The words, as i18n.t gives them: the key's English with {{name}} filled in.
function t(key, vars) {
  assert.ok(EN[key] !== undefined, 'no word for key ' + key);
  return EN[key].replace(/\{\{(\w+)\}\}/g, (_, n) => String(vars[n]));
}

// A tiny DOM: elements with children, text, attributes and style, enough to read back what is drawn.
function fakeDoc() {
  function el(tag) {
    return {
      tag, children: [], attrs: {}, style: {}, className: '', textContent: '',
      get firstChild() { return this.children[0] || null; },
      appendChild(c) { this.children.push(c); return c; },
      removeChild(c) { this.children.splice(this.children.indexOf(c), 1); return c; },
      setAttribute(k, v) { this.attrs[k] = v; },
    };
  }
  return { createElement: el };
}
function allText(e) { return [e.textContent].concat(e.children.map(allText)).filter(Boolean).join('\n'); }
function allLinks(e) { return (e.attrs.href ? [e.attrs.href] : []).concat(...e.children.map(allLinks)); }

const served = {
  enabled: true,
  channels: [
    { name: 'Patreon', link: 'https://www.patreon.com/x', monthly_usd: 20, as_of: '2026-11-22' },
    { name: 'CashApp', link: '', monthly_usd: 0, as_of: '2026-11-22' },
  ],
  costs: [
    { name: 'Servers', monthly_usd: 41, as_of: '2026-11-20' },
    { name: 'Tools', monthly_usd: 200, as_of: '2026-11-20' },
  ],
};

test('the page shows the channels, the costs and the covered sentence', () => {
  const doc = fakeDoc();
  const section = doc.createElement('div');
  section.style.display = 'none';
  assert.strictEqual(F.render(section, served, t, doc), true);
  assert.strictEqual(section.style.display, '');
  const text = allText(section);
  for (const want of [
    'This month', 'Donations cover $20 of $241 a month.', 'Patreon', '$20 a month', 'as of 2026-11-22',
    'Servers', '$41 a month', 'CashApp',
  ]) assert.ok(text.includes(want), 'the page does not show: ' + want);
  assert.deepStrictEqual(allLinks(section), ['https://www.patreon.com/x'], 'only a channel with a link gets one');
});

test('donations that meet the costs say so, and a server with no costs says what comes in', () => {
  const paid = JSON.parse(JSON.stringify(served));
  paid.channels[0].monthly_usd = 300;
  assert.strictEqual(F.sentence(F.read(paid), t), 'Donations cover all $241 a month.');
  assert.strictEqual(F.sentence(F.read({ channels: served.channels, costs: [] }), t), 'Donations bring in $20 a month.');
  assert.strictEqual(F.money(12.5), '12.50');
});

test('nothing is shown when none are published', () => {
  const doc = fakeDoc();
  const section = doc.createElement('div');
  for (const funding of [null, undefined, {}, { enabled: true, goal_usd: 100 }, { channels: [], costs: [] }]) {
    section.style.display = '';
    section.appendChild(doc.createElement('p'));
    assert.strictEqual(F.render(section, funding, t, doc), false);
    assert.strictEqual(section.style.display, 'none', 'the card is shown with no figures');
    assert.strictEqual(section.children.length, 0, 'old rows are cleared');
  }
});

test('a rogue server cannot draw a link that is not https, a bad amount or a thousand rows', () => {
  const f = F.read({
    channels: [
      { name: 'Evil', link: 'http://evil.example', monthly_usd: 5 },
      { name: 'Script', link: 'javascript:alert(1)', monthly_usd: 5 },
      { name: 'Negative', link: '', monthly_usd: -1 },
      { name: 'Huge', link: '', monthly_usd: 2000000 },
      { name: '', link: '', monthly_usd: 1 },
      { name: 'Fine', link: 'https://ok.example', monthly_usd: 5 },
    ].concat(Array.from({ length: 100 }, (_, i) => ({ name: 'row' + i, monthly_usd: 1 }))),
  });
  assert.ok(f.channels.every((c) => ['Fine'].includes(c.name) || /^row/.test(c.name)), 'a bad row was read');
  assert.strictEqual(f.channels.length, 32, 'at most 32 rows are read');
});
