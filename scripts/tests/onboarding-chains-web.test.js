// The learn-by-doing chains on the website's /onboarding page (web/pages/onboarding.html) say what
// the desktop's suggested cards say (2026-10-30, the seams review): each chain's one-line
// `purpose` from data/onboarding/quests.json (version 5), and on a step with a written how-to,
// Open the how-to, beside Go there. The desktop shows the chains only as the Tasks page's
// suggested cards (src/gui/pages/tasks.rs), which read both fields; this page had neither.
//
// Run: node --test scripts/tests/onboarding-chains-web.test.js   (in `just rig-tests`)
// HOS_WEB_DIR points it at another copy of web/.
//
// The page's own chain script runs in node:vm over a stand-in page, fed the real quests.json.
//
// Red first, 2026-10-30, against onboarding.html as it was before (HOS_WEB_DIR on a copy of it):
// "every card carries its chain's purpose: first-steps" and "first-steps:backup links its
// how-to".

const test = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const ROOT = path.join(__dirname, '..', '..');
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, 'web');
const quests = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'onboarding', 'quests.json'), 'utf8'));
const en = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'i18n', 'en.json'), 'utf8'));

/** The page's chain script: the <script> holding "Quest chain renderer". */
function chainScript() {
  const html = fs.readFileSync(path.join(WEB, 'pages', 'onboarding.html'), 'utf8').replace(/\r\n/g, '\n');
  const at = html.indexOf('Quest chain renderer');
  assert.ok(at >= 0, 'onboarding.html has its chain renderer');
  const from = html.lastIndexOf('<script>', at) + '<script>'.length;
  return html.slice(from, html.indexOf('</script>', at));
}

const escHtml = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;').replace(/'/g, '&#39;');

async function page() {
  const listeners = {};
  const el = (id) => ({
    id, className: '', innerHTML: '', dataset: {},
    addEventListener: (t, f) => { listeners[(id || 'el') + ':' + t] = f; },
    querySelector: () => null, remove() {}, focus() {},
  });
  const root = el('quest-root');
  const appended = [];
  const ctx = {
    console, JSON, Promise, String, Object, Array,
    CSS: { escape: (s) => s },
    localStorage: { getItem: () => null, setItem() {}, removeItem() {} },
    i18n: { t: (k) => k, apply() {} },
    fetch: () => Promise.resolve({ ok: true, json: () => Promise.resolve(quests) }),
    document: {
      getElementById: (id) => (id === 'quest-root' ? root : null),
      querySelector: () => null,
      createElement: () => el(''),
      addEventListener() {},
      body: { appendChild: (c) => { appended.push(c); return c; } },
    },
  };
  ctx.window = ctx;
  vm.createContext(ctx);
  vm.runInContext(chainScript(), ctx, { filename: 'onboarding.html#chains' });
  for (let i = 0; i < 5; i++) await Promise.resolve();
  const open = (id) => {
    listeners['quest-root:click']({ target: { closest: (sel) => (sel === '.chain-card' ? { dataset: { chain: id } } : null) } });
    return appended[appended.length - 1].innerHTML;
  };
  return { root, open };
}

test('every card carries its chain\'s purpose', async () => {
  const { root } = await page();
  assert.ok(quests.chains.length >= 6, 'the shipped chains');
  for (const c of quests.chains) {
    assert.ok(c.purpose, c.id + ' has a purpose in quests.json');
    assert.ok(root.innerHTML.includes(escHtml(c.purpose)), 'every card carries its chain\'s purpose: ' + c.id);
  }
});

test('a step with a written how-to offers Open the how-to; one without does not', async () => {
  const { open } = await page();
  let linked = 0;
  for (const c of quests.chains) {
    const win = open(c.id);
    assert.ok(win.includes(escHtml(c.purpose)), 'the window says the purpose too: ' + c.id);
    for (const s of c.steps) {
      const link = 'href="/how-to#' + encodeURIComponent(s.how_to || '') + '"';
      if (s.how_to) {
        assert.ok(win.includes(link), c.id + ':' + s.id + ' links its how-to');
        linked++;
      }
    }
    const howTos = (win.match(/href="\/how-to#/g) || []).length;
    assert.strictEqual(howTos, c.steps.filter((s) => s.how_to).length, c.id + ': one how-to link per step that has one');
  }
  assert.ok(linked >= 2, 'the shipped chains link their written how-tos');
  // The words are the Tasks page's, by key, with the hint (the same two in both places).
  const src = chainScript();
  assert.match(src, /i18n\.t\('tasks\.suggested_how_to'\)/);
  assert.match(src, /data-i18n-tip="tasks\.suggested_how_to_hint"/);
  assert.ok(en.tasks.suggested_how_to && en.tasks.suggested_how_to_hint, 'en.json has both');
});
