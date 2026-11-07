#!/usr/bin/env node
// Write each website page's description and share preview from data/site/page_meta.json and
// data/i18n/en.json (2026-11-07, docs/design/privacy-notice.md section 6).
//
//   node scripts/gen-page-meta.js           rewrite the marked block in every listed page
//   node scripts/gen-page-meta.js --check   exit 1 when a page's block is not what the data says
//
// WHY A GENERATOR: social sites (Facebook, X, Discord, Slack...) and search engines read a page's
// HTML without running its scripts, so a share preview added by shell.js would never be seen. The
// tags have to be in every page's own <head>, and 47 hand-copied blocks drift (the chat's said
// "Ed25519 identity" a year after the identity became Dilithium3). One block, written from data:
//
//   <!-- page-meta: ... -->
//   <meta name="description" data-i18n-content="KEY" content="...">
//   <meta property="og:type" content="website">
//   <meta property="og:site_name" content="HumanityOS">
//   <meta property="og:title" ...>          the page's own <title>, by its key when it has one
//   <meta property="og:description" data-i18n-content="KEY" content="...">
//   <meta property="og:url" content="ORIGIN + PATH">
//   <meta property="og:image" ...>  (+ width, height, alt by key)
//   <meta name="twitter:card" content="summary_large_image">
//   <!-- /page-meta -->
//
// The words come from en.json by key (the ELI5 mandate's one word source); data-i18n-content keeps
// them translatable and lets scripts/i18n-web.js --html-check hold them equal to en.json. Writing
// a page removes any description, og: or twitter: tag elsewhere in its <head>, so a page has one
// set. The page keeps its own line endings (the working copies are CRLF).
'use strict';

const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const DATA = path.join(ROOT, 'data', 'site', 'page_meta.json');
const EN = path.join(ROOT, 'data', 'i18n', 'en.json');

const OPEN = '<!-- page-meta: written by scripts/gen-page-meta.js from data/site/page_meta.json and data/i18n/en.json; edit those, not these lines -->';
const CLOSE = '<!-- /page-meta -->';

const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

function unescapeHtml(s) {
  return String(s)
    .replace(/&quot;/g, '"').replace(/&#39;|&apos;/g, "'").replace(/&lt;/g, '<').replace(/&gt;/g, '>')
    .replace(/&nbsp;/g, ' ').replace(/&amp;/g, '&');
}

/** A dotted key's words in en.json, or undefined. */
function word(en, key) {
  let v = en;
  for (const part of String(key).split('.')) {
    if (v === null || typeof v !== 'object' || !(part in v)) return undefined;
    v = v[part];
  }
  return typeof v === 'string' ? v : undefined;
}

/** The page's <title>: its key (data-i18n) if it has one, and its text. */
function titleOf(html) {
  const m = /<title([^>]*)>([\s\S]*?)<\/title>/i.exec(html);
  if (!m) return null;
  const key = /data-i18n\s*=\s*"([^"]+)"/.exec(m[1]) || /data-i18n\s*=\s*([^\s>]+)/.exec(m[1]);
  return { key: key ? key[1] : null, text: unescapeHtml(m[2].trim()) };
}

/** The lines of the block for one page (no indentation, no line endings). */
function blockLines(data, en, row, html) {
  const problems = [];
  const t = titleOf(html);
  if (!t || !t.text) problems.push(row.file + ': no <title>');
  const desc = word(en, row.description);
  if (desc === undefined) problems.push(row.file + ': description key ' + row.description + ' is not in en.json');
  const img = Object.assign({}, data.image, row.image || {});
  const alt = word(en, img.alt);
  if (alt === undefined) problems.push(row.file + ': image alt key ' + img.alt + ' is not in en.json');
  if (problems.length) return { problems };
  const origin = row.origin || data.origin;
  const titleWords = t.key && word(en, t.key) !== undefined ? word(en, t.key) : t.text;
  const titleAttr = t.key && word(en, t.key) !== undefined ? ' data-i18n-content="' + esc(t.key) + '"' : '';
  const imageUrl = /^https?:\/\//.test(img.path) ? img.path : origin + img.path;
  return {
    problems,
    lines: [
      OPEN,
      '<meta name="description" data-i18n-content="' + esc(row.description) + '" content="' + esc(desc) + '">',
      '<meta property="og:type" content="website">',
      '<meta property="og:site_name" content="' + esc(data.site_name) + '">',
      '<meta property="og:title"' + titleAttr + ' content="' + esc(titleWords) + '">',
      '<meta property="og:description" data-i18n-content="' + esc(row.description) + '" content="' + esc(desc) + '">',
      '<meta property="og:url" content="' + esc(origin + row.path) + '">',
      '<meta property="og:image" content="' + esc(imageUrl) + '">',
      '<meta property="og:image:width" content="' + esc(img.width) + '">',
      '<meta property="og:image:height" content="' + esc(img.height) + '">',
      '<meta property="og:image:alt" data-i18n-content="' + esc(img.alt) + '" content="' + esc(alt) + '">',
      '<meta name="twitter:card" content="summary_large_image">',
      CLOSE,
    ],
  };
}

/** A meta line the block owns: a description, any og: property, any twitter: name. */
const OWNED = /^[ \t]*<meta\s+(?:name\s*=\s*"?(?:description|twitter:[\w:-]+)"?|property\s*=\s*"?og:[\w:-]+"?)[^>]*>[ \t]*$/i;

/** The page's text with its block written (or rewritten). */
function render(data, en, row, html) {
  const crlf = /\r\n/.test(html);
  const src = html.replace(/\r\n/g, '\n');
  const headEnd = src.search(/<\/head>/i);
  if (headEnd < 0) return { problems: [row.file + ': no </head>'] };
  let head = src.slice(0, headEnd);
  const rest = src.slice(headEnd);
  // Out with the old block and any loose description/og/twitter lines.
  const lines = head.split('\n');
  const kept = [];
  let inBlock = false;
  for (const line of lines) {
    if (line.includes(OPEN.slice(0, 20))) { inBlock = true; continue; }
    if (inBlock) { if (line.includes(CLOSE)) inBlock = false; continue; }
    if (OWNED.test(line)) continue;
    kept.push(line);
  }
  // Two blank lines left where a removed group stood become one.
  const tidy = [];
  for (const line of kept) {
    if (line.trim() === '' && tidy.length && tidy[tidy.length - 1].trim() === '') continue;
    tidy.push(line);
  }
  const b = blockLines(data, en, row, src);
  if (b.problems.length) return { problems: b.problems };
  const at = tidy.findIndex((l) => /<\/title>/i.test(l));
  if (at < 0) return { problems: [row.file + ': the </title> is not on a line of its own in <head>'] };
  const indent = (/^([ \t]*)/.exec(tidy[at]) || ['', ''])[1];
  tidy.splice(at + 1, 0, ...b.lines.map((l) => indent + l));
  let out = tidy.join('\n') + rest;
  if (crlf) out = out.replace(/\n/g, '\r\n');
  return { problems: [], text: out };
}

function load() {
  return {
    data: JSON.parse(fs.readFileSync(DATA, 'utf8')),
    en: JSON.parse(fs.readFileSync(EN, 'utf8')),
  };
}

/** Every listed page: { row, have, want, problems }. */
function plan() {
  const { data, en } = load();
  return data.pages.map((row) => {
    const file = path.join(ROOT, row.file);
    if (!fs.existsSync(file)) return { row, problems: [row.file + ': no such file'] };
    const have = fs.readFileSync(file, 'utf8');
    const r = render(data, en, row, have);
    return { row, have, want: r.text, problems: r.problems };
  });
}

if (require.main === module) {
  const check = process.argv.includes('--check');
  const results = plan();
  const problems = results.flatMap((r) => r.problems);
  if (problems.length) {
    console.error(problems.join('\n'));
    process.exit(1);
  }
  const stale = results.filter((r) => r.have !== r.want);
  if (check) {
    if (stale.length) {
      console.error('page meta out of date (run node scripts/gen-page-meta.js):\n  ' + stale.map((r) => r.row.file).join('\n  '));
      process.exit(1);
    }
    console.log('page meta current on ' + results.length + ' pages');
  } else {
    for (const r of stale) fs.writeFileSync(path.join(ROOT, r.row.file), r.want);
    console.log('wrote page meta on ' + stale.length + ' of ' + results.length + ' pages');
  }
}

module.exports = { plan, render, blockLines, titleOf, word, DATA, EN, OPEN, CLOSE };
