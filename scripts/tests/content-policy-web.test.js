// The content policy is one press away on the website (2026-11-20; docs/legal/content-policy.md,
// the Digital Public Goods Standard's indicator 9b, docs/reference/findings/2026-11-18-digital-public-goods.md).
// The native twin is src/gui/pages/safety_content_policy_tests.rs.
//
// Run: node --test scripts/tests/content-policy-web.test.js
//
// What it proves, each from what the page DRAWS, not from what the code was told:
//  1. The Library holds the policy under the slug every link names (`content-policy`, the slug
//     web/pages/library-app.js `slugOf` and the desktop's library_slug_of_file derive), and its
//     shipped copy is the source document's (scripts/build-library.js was run).
//  2. The site's footer, drawn by web/shared/shell.js run whole in node:vm (wip-shell-harness.js),
//     links /library#content-policy with its words and its hint by key, on every page.
//  3. The chat's Safety panel (web/chat/chat-privacy.js `safetyPanelHtml`, run in node:vm) has the
//     section: its heading and words from data/i18n/en.json and the link with its hint.
//  4. The rules page links the policy where it says how to report.
//
// RED FIRST (2026-11-20), run before the links existed (the words were already in en.json):
//   "the footer links the content policy": "no link to /library#content-policy in the footer";
//   "the chat's Safety panel has the content policy section": "the panel has no
//   content-policy section"; "the rules page links the policy": "web/pages/rules.html does not
//   link /library#content-policy". Test 1 was seen red with SLUG spelled content_policy (the
//   underscore a file name has, not the slug): "the Library has no document with the slug
//   content_policy".

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const H = require('./wip-shell-harness.js');
const ROOT = H.ROOT;
const WEB = H.WEB;
const SLUG = 'content-policy';
const HREF = '/library#' + SLUG;

/** The slug a Library file gets, exactly as web/pages/library-app.js `slugOf` derives it. */
const slugOf = (file) => String(file).replace(/\.md$/i, '').replace(/_/g, '-').toLowerCase();

/** The `<a ...>` element in `html` whose href is `href`, with its attributes and its text, or null. */
function linkTo(html, href) {
  const re = /<a\b([^>]*)>([\s\S]*?)<\/a>/g;
  let m;
  while ((m = re.exec(String(html)))) {
    const attrs = {};
    for (const a of m[1].matchAll(/([\w-]+)="([^"]*)"/g)) attrs[a[1]] = a[2];
    if (attrs.href === href) return { attrs, text: m[2].replace(/<[^>]+>/g, '').trim() };
  }
  return null;
}

const unescape = (s) => String(s).replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&');

test('the Library ships the content policy under the slug the links name', () => {
  const index = JSON.parse(fs.readFileSync(path.join(ROOT, 'data', 'library', 'index.json'), 'utf8'));
  const docs = index.categories.flatMap((c) => c.docs || []);
  const doc = docs.find((d) => slugOf(d.file) === SLUG);
  assert.ok(doc, 'the Library has no document with the slug ' + SLUG);
  const shipped = fs.readFileSync(path.join(ROOT, 'data', 'library', doc.file), 'utf8');
  const source = fs.readFileSync(path.join(ROOT, 'docs', 'legal', 'content-policy.md'), 'utf8');
  const firstLine = (s) => s.split(/\r?\n/)[0];
  assert.strictEqual(firstLine(shipped), firstLine(source), 'the shipped copy is the policy (run node scripts/build-library.js)');
});

test('the footer links the content policy, words and hint by key, on every page', () => {
  for (const page of ['/pages/notes.html', '/pages/rules.html', '/chat']) {
    const shell = H.loadShell({ path: page });
    shell.readDone();
    const footer = shell.appended.find((e) => e.className === 'site-footer');
    assert.ok(footer, 'the shell drew its footer on ' + page);
    const a = linkTo(footer.innerHTML, HREF);
    assert.ok(a, 'no link to ' + HREF + ' in the footer on ' + page);
    assert.strictEqual(a.attrs['data-i18n'], 'footer.content_policy');
    assert.strictEqual(a.attrs['data-i18n-tip'], 'footer.content_policy_hint');
    assert.strictEqual(a.text, H.t('footer.content_policy'), 'the link reads its words from en.json');
    assert.strictEqual(unescape(a.attrs['data-tip-desc']), H.t('footer.content_policy_hint'), 'the hint is en.json\'s');
  }
});

/** chat-privacy.js run in node:vm with what its Safety panel needs and nothing else. */
function loadSafetyPanel() {
  const ctx = {
    console: { log() {}, warn() {}, error() {}, info() {}, debug() {} },
    document: { getElementById: () => null, querySelector: () => null, querySelectorAll: () => [] },
    setTimeout: () => 0,
    clearTimeout() {},
    handleMessage() {},
    i18n: { t: H.t, has: () => true },
  };
  ctx.window = ctx;
  vm.createContext(ctx);
  for (const rel of ['shared/reach.js', 'chat/chat-privacy.js']) {
    vm.runInContext(fs.readFileSync(path.join(WEB, rel), 'utf8'), ctx, { filename: rel });
  }
  return vm.runInContext('safetyPanelHtml', ctx);
}

test('the chat\'s Safety panel has the content policy section, words and hint by key', () => {
  const html = loadSafetyPanel()({ known: true, rows: [], chosen: [], ticksInUse: '', requests: [], groupReports: null, blocked: [] });
  assert.ok(html.includes('data-content-policy'), 'the panel has no content-policy section');
  assert.ok(html.includes(H.t('content_policy.heading')), 'the heading is en.json\'s');
  assert.ok(unescape(html).includes(H.t('content_policy.body')), 'the words are en.json\'s');
  const a = linkTo(html, HREF);
  assert.ok(a, 'the section links ' + HREF);
  assert.strictEqual(a.attrs['data-i18n-tip'], 'content_policy.open_hint');
  assert.strictEqual(a.text, H.t('content_policy.open'));
  assert.strictEqual(unescape(a.attrs['data-tip-desc']), H.t('content_policy.open_hint'));
});

test('the rules page links the policy where it says how to report', () => {
  const html = fs.readFileSync(path.join(WEB, 'pages', 'rules.html'), 'utf8');
  const a = linkTo(html, HREF);
  assert.ok(a, 'web/pages/rules.html does not link ' + HREF);
  assert.strictEqual(a.attrs['data-i18n'], 'content_policy.open');
  assert.strictEqual(a.attrs['data-i18n-tip'], 'content_policy.open_hint');
  assert.ok(!html.includes('/report &lt;name&gt; &lt;what happened&gt;'), 'the old /report command is no longer how the page says to report');
});
