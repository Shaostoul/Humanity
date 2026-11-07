// The site's own 404 page, and nginx not naming its version (2026-11-07, BUG-277,
// docs/design/privacy-notice.md section 5). Before this, a missing address on united-humanity.us
// answered with nginx's default page: "404 Not Found" and "nginx/1.22.1", no way on, and the
// server's exact version for anyone scanning for a known flaw.
//
// Run: node --test scripts/tests/not-found-web.test.js
//
// What it proves:
//  1. scripts/nginx/humanity.conf sends a missing address to /404.html (`error_page 404 /404.html;`),
//     keeps that page internal, and has `server_tokens off;` in EVERY server block.
//  2. The config is well formed as far as can be checked without nginx (none on the dev PC): braces
//     balance, every directive ends in `;` or opens a block. The real check is `nginx -t` on the
//     server; BUG-277 in docs/BUGS.md has the operator's commands.
//  3. web/pages/404.html exists (scripts/sync-web-root.sh copies web/pages/*.html to the web root),
//     and every address in it is root-absolute: nginx serves it AT the missing address (/a/b/c), so
//     "shared/theme.css" would point at /a/b/shared/theme.css and the page would draw unstyled.
//  4. The checks themselves fail on a config without the lines, and on a relative link.
//
// SEEN RED FIRST (2026-11-07, against the config and pages before this change): test 1 failed
// "no error_page 404 /404.html" and "server block 1 of 4 has no server_tokens off" (all four);
// test 3 failed "web/pages/404.html is missing".

const { test } = require('node:test');
const assert = require('node:assert');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.join(__dirname, '..', '..');
const CONF = path.join(ROOT, 'scripts', 'nginx', 'humanity.conf');
const PAGE = path.join(ROOT, 'web', 'pages', '404.html');

/** The config with comments and quoted strings blanked, so braces and semicolons can be counted.
 *  Line by line, quotes first within a line: a comment's apostrophe ("the relay's") must not open
 *  a "string" that runs on to the next comment and swallows the directives between. */
function strip(conf) {
  return conf.replace(/\r\n/g, '\n').split('\n').map((line) => {
    let out = '';
    let q = null;
    for (const c of line) {
      if (q) { if (c === q) { q = null; out += c; } continue; }
      if (c === '#') break;
      if (c === '"' || c === "'") { q = c; out += c; continue; }
      out += c;
    }
    return out;
  }).join('\n');
}

/** The body of each top-level `server { ... }` block. */
function serverBlocks(conf) {
  const s = strip(conf);
  const out = [];
  const re = /(^|\n)\s*server\s*\{/g;
  let m;
  while ((m = re.exec(s)) !== null) {
    let depth = 1;
    let i = m.index + m[0].length;
    const start = i;
    for (; i < s.length && depth > 0; i++) {
      if (s[i] === '{') depth++;
      else if (s[i] === '}') depth--;
    }
    out.push(s.slice(start, i - 1));
    re.lastIndex = i;
  }
  return out;
}

function confProblems(conf) {
  const p = [];
  const s = strip(conf);
  if (!/(^|\n)\s*error_page\s+404\s+\/404\.html\s*;/.test(s)) p.push('no error_page 404 /404.html');
  if (!/location\s*=\s*\/404\.html\s*\{\s*internal\s*;\s*\}/.test(s)) p.push('no internal location = /404.html');
  const blocks = serverBlocks(conf);
  if (!blocks.length) p.push('no server blocks');
  blocks.forEach((b, i) => {
    // Only the block's own level: a nested location's line would not cover the block.
    let depth = 0;
    let own = '';
    for (const c of b) {
      if (c === '{') depth++;
      else if (c === '}') depth--;
      else if (depth === 0) own += c;
    }
    if (!/(^|[;\s])server_tokens\s+off\s*;/.test(own)) p.push('server block ' + (i + 1) + ' of ' + blocks.length + ' has no server_tokens off');
  });
  return p;
}

function syntaxProblems(conf) {
  const p = [];
  const s = strip(conf);
  let depth = 0;
  s.split('\n').forEach((line, n) => {
    for (const c of line) {
      if (c === '{') depth++;
      else if (c === '}') depth--;
      if (depth < 0) p.push('line ' + (n + 1) + ': a } with no {');
    }
    const t = line.trim();
    if (t && !/[;{}]$/.test(t)) p.push('line ' + (n + 1) + ': a directive that does not end in ; or open a block: ' + t);
  });
  if (depth !== 0) p.push('braces do not balance (' + depth + ' left open)');
  return p;
}

function pageProblems(html) {
  const p = [];
  for (const m of html.matchAll(/\s(?:href|src)\s*=\s*"([^"]*)"/g)) {
    const u = m[1];
    if (!u.startsWith('/') && !/^https?:\/\//.test(u) && !u.startsWith('#') && !u.startsWith('mailto:')) p.push('a relative address: ' + u);
  }
  return p;
}

test('nginx sends a missing address to our page and never names its version', () => {
  assert.deepStrictEqual(confProblems(fs.readFileSync(CONF, 'utf8')), []);
});

test('the nginx config is well formed, as far as can be checked without nginx', () => {
  assert.deepStrictEqual(syntaxProblems(fs.readFileSync(CONF, 'utf8')), []);
});

test('the 404 page exists and every address in it is root-absolute', () => {
  assert.ok(fs.existsSync(PAGE), 'web/pages/404.html is missing');
  assert.deepStrictEqual(pageProblems(fs.readFileSync(PAGE, 'utf8')), []);
});

test('the checks fail on a config without the lines and on a relative link', () => {
  const bare = 'server {\n    server_name a;\n    location / { try_files $uri =404; }\n}\nserver {\n    listen 80;\n    server_tokens off;\n    return 301 https://a;\n}\n';
  const p = confProblems(bare);
  assert.ok(p.includes('no error_page 404 /404.html'), p.join('; '));
  assert.ok(p.includes('server block 1 of 2 has no server_tokens off'), p.join('; '));
  assert.ok(!p.includes('server block 2 of 2 has no server_tokens off'), p.join('; '));
  // A server_tokens line inside a location does not count for the block.
  assert.ok(confProblems('server {\n location / { server_tokens off; }\n}\n').includes('server block 1 of 1 has no server_tokens off'));
  assert.ok(syntaxProblems('server {\n    listen 80\n}\n').some((x) => /does not end/.test(x)));
  assert.ok(syntaxProblems('server {\n    listen 80;\n').some((x) => /balance/.test(x)));
  assert.deepStrictEqual(pageProblems('<link href="shared/theme.css">'), ['a relative address: shared/theme.css']);
});
