// Every website page people can reach has a title, a one-sentence description and a share preview
// (2026-11-07, the operator's website checklist: "Let's get that done."; docs/design/privacy-notice.md
// section 6). Search engines show the description; Facebook, X, Discord and Slack build a link's
// preview card from the og: and twitter: tags, reading the page's HTML WITHOUT running its scripts.
//
// Run: node --test scripts/tests/page-meta-web.test.js
//
// What it proves:
//  1. Every HTML file under web/ is in data/site/page_meta.json, as a page or in `left_out` with a
//     reason, so a new page cannot slip through without a description.
//  2. Each listed page, read as a crawler reads it (the raw HTML), has a non-empty <title>, one
//     description, and og:type, og:title, og:description, og:url, og:image and twitter:card; og:url
//     is the row's address; the picture exists in web/.
//  3. Each description comes from en.json by key, is one plain sentence (ends with a full stop, 60
//     to 200 characters) and no two pages share one (each says what ITS page is for).
//  4. The block in each page is exactly what scripts/gen-page-meta.js writes (so editing the data
//     or en.json without running it fails here).
//  5. The checker itself: a page with no description or no og:image fails it.
//
// SEEN RED FIRST (2026-11-07, before the generator ran; the 404 page new, the privacy page not yet
// written): test 2 failed with 31 of the 46 pages "no description" (e.g. "web/pages/accord.html: no
// description"), 43 "no og:image", "web/pages/privacy.html: no such file", and even the front door
// failing on "og:url is https://united-humanity.us, the row says https://united-humanity.us/" (its
// canonical link ends in a slash); test 4 failed on "web/pages/privacy.html: no such file".

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');
const GEN = require(path.join(ROOT, 'scripts', 'gen-page-meta.js'));
const data = JSON.parse(fs.readFileSync(GEN.DATA, 'utf8'));
const en = JSON.parse(fs.readFileSync(GEN.EN, 'utf8'));

function htmlFiles(dir) {
  let out = [];
  for (const f of fs.readdirSync(dir)) {
    const p = path.join(dir, f);
    if (fs.statSync(p).isDirectory()) out = out.concat(htmlFiles(p));
    else if (f.endsWith('.html')) out.push(path.relative(ROOT, p).split(path.sep).join('/'));
  }
  return out;
}

/** What a crawler finds in a page's <head>: the title and every meta name/property -> content. */
function crawl(html) {
  const head = html.slice(0, Math.max(0, html.search(/<\/head>/i)));
  const t = /<title[^>]*>([\s\S]*?)<\/title>/i.exec(head);
  const metas = {};
  for (const m of head.matchAll(/<meta\s+([^>]*)>/gi)) {
    const a = m[1];
    const name = /(?:name|property)\s*=\s*"([^"]+)"/i.exec(a);
    const content = /content\s*=\s*"([^"]*)"/i.exec(a);
    if (!name) continue;
    (metas[name[1].toLowerCase()] = metas[name[1].toLowerCase()] || []).push(content ? content[1] : '');
  }
  return { title: t ? t[1].trim() : '', metas };
}

const REQUIRED = ['description', 'og:type', 'og:title', 'og:description', 'og:url', 'og:image', 'twitter:card'];

/** The problems a crawler would meet on one page. */
function problemsOf(file, html, row) {
  const p = [];
  const c = crawl(html);
  if (!c.title) p.push(file + ': no <title>');
  for (const k of REQUIRED) {
    const v = c.metas[k];
    if (!v || !v.length || !v[0].trim()) p.push(file + ': no ' + k);
    else if (v.length > 1) p.push(file + ': ' + v.length + ' ' + k + ' tags');
  }
  if (row && c.metas['og:url'] && c.metas['og:url'][0] !== (row.origin || data.origin) + row.path) {
    p.push(file + ': og:url is ' + c.metas['og:url'][0] + ', the row says ' + (row.origin || data.origin) + row.path);
  }
  return p;
}

test('every HTML file under web/ is listed, as a page or left out with a reason', () => {
  const listed = new Set(data.pages.map((r) => r.file));
  const left = new Map((data.left_out || []).map((r) => [r.file, r.why]));
  const missing = htmlFiles(path.join(ROOT, 'web')).filter((f) => !listed.has(f) && !left.has(f));
  assert.deepStrictEqual(missing, [], 'add each to data/site/page_meta.json (pages, or left_out with why):\n' + missing.join('\n'));
  for (const [f, why] of left) assert.ok(why && why.trim().length > 10, f + ': left_out needs a reason');
  const paths = data.pages.map((r) => (r.origin || data.origin) + r.path);
  assert.strictEqual(new Set(paths).size, paths.length, 'two rows share an address');
});

test('every listed page has a title, a description and the share tags, as a crawler reads it', () => {
  const problems = [];
  for (const row of data.pages) {
    const f = path.join(ROOT, row.file);
    if (!fs.existsSync(f)) { problems.push(row.file + ': no such file'); continue; }
    problems.push(...problemsOf(row.file, fs.readFileSync(f, 'utf8'), row));
  }
  const img = path.join(ROOT, 'web', ...data.image.path.split('/').filter(Boolean));
  if (!fs.existsSync(img)) problems.push('the share picture ' + data.image.path + ' is not in web/');
  assert.deepStrictEqual(problems, [], '\n' + problems.join('\n') + '\nRun: node scripts/gen-page-meta.js');
});

test('each description is one plain sentence, by key, and no two pages share one', () => {
  const problems = [];
  const seen = new Map();
  for (const row of data.pages) {
    const d = GEN.word(en, row.description);
    if (d === undefined) { problems.push(row.file + ': ' + row.description + ' is not in data/i18n/en.json'); continue; }
    if (!/[.!?]$/.test(d)) problems.push(row.file + ': the description does not end a sentence: ' + d);
    if (d.length < 60 || d.length > 200) problems.push(row.file + ': the description is ' + d.length + ' characters (60 to 200): ' + d);
    if (/\[\[/.test(d)) problems.push(row.file + ': a glossary mark in a description shows as brackets in a search result');
    if (seen.has(d)) problems.push(row.file + ' and ' + seen.get(d) + ' share a description');
    seen.set(d, row.file);
  }
  assert.deepStrictEqual(problems, [], '\n' + problems.join('\n'));
});

test('each page carries exactly the block scripts/gen-page-meta.js writes', () => {
  const results = GEN.plan();
  const problems = results.flatMap((r) => r.problems);
  assert.deepStrictEqual(problems, [], '\n' + problems.join('\n'));
  const stale = results.filter((r) => r.have !== r.want).map((r) => r.row.file + ': not what the generator writes');
  assert.deepStrictEqual(stale, [], '\n' + stale.join('\n') + '\nRun: node scripts/gen-page-meta.js');
});

test('the checker catches a page with no description or no picture', () => {
  const row = { file: 'x.html', path: '/x' };
  const good =
    '<html><head><title>X</title><meta name="description" content="d">' +
    '<meta property="og:type" content="website"><meta property="og:title" content="X">' +
    '<meta property="og:description" content="d"><meta property="og:url" content="' + data.origin + '/x">' +
    '<meta property="og:image" content="i"><meta name="twitter:card" content="summary_large_image"></head></html>';
  assert.deepStrictEqual(problemsOf('x.html', good, row), []);
  assert.ok(problemsOf('x.html', good.replace(/<meta name="description"[^>]*>/, ''), row).some((p) => /no description/.test(p)));
  assert.ok(problemsOf('x.html', good.replace(/<meta property="og:image"[^>]*>/, ''), row).some((p) => /no og:image/.test(p)));
  assert.ok(problemsOf('x.html', good.replace('/x"', '/y"'), row).some((p) => /og:url/.test(p)));
  // And the generator puts a block in a bare page, keeping CRLF.
  const bare = '<html>\r\n<head>\r\n  <title>X</title>\r\n</head>\r\n</html>\r\n';
  const r = GEN.render(data, en, { file: 'x.html', path: '/x', description: 'page_meta.web' }, bare);
  assert.deepStrictEqual(r.problems, []);
  assert.ok(!/[^\r]\n/.test(r.text), 'line endings kept as CRLF');
  assert.deepStrictEqual(problemsOf('x.html', r.text, row), []);
});
