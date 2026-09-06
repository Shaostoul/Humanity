#!/usr/bin/env node
// Tiny static preview server that mirrors the production nginx routing so web pages
// can be previewed locally exactly as they are served live:
//   /            -> web/pages/index.html  (prod flattens web/pages/* into the web
//                                          root, so its /index.html IS this file)
//   /library     -> web/pages/library.html  (mirrors nginx `try_files $uri.html`,
//                                            the catch-all every extensionless
//                                            page link on the site relies on)
//   /pages/*     -> web/pages/*      (and /shared, /chat, /activities likewise)
//   /data/*      -> data/*           (the same dir the deploy rsyncs to /var/www/.../data)
//   /docs/*      -> docs/*
//   /assets/*    -> assets/*
// It does NOT proxy /api or /ws (no relay), so pages that need the live relay show
// their empty/offline state, which is fine for layout checks. Static-content pages
// (like the roadmap, which reads /data/roadmap.json) render fully.
//
//   node scripts/preview-server.js [port]   (default 8099)

const http = require('http');
const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const PORT = parseInt(process.argv[2] || process.env.PORT || '8099', 10);

const TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.webp': 'image/webp',
  '.ico': 'image/x-icon',
  '.woff2': 'font/woff2',
  '.wasm': 'application/wasm',
};

// Map a URL path to a file on disk, mirroring nginx aliases.
function resolve(urlPath) {
  let p = decodeURIComponent(urlPath.split('?')[0]);
  // Prod's web root holds web/pages/* flattened, so its index.html is this one.
  if (p === '/' || p === '') return path.join(ROOT, 'web', 'pages', 'index.html');
  // Aliased top-level dirs served from the repo root (not from web/).
  for (const alias of ['/data/', '/docs/', '/assets/']) {
    if (p.startsWith(alias)) return path.join(ROOT, p.slice(1));
  }
  // Everything else is served from web/.
  return path.join(ROOT, 'web', p.slice(1));
}

// Extensionless fallback mirroring nginx `try_files $uri $uri.html`: on prod the
// flattened pages make /library resolve to library.html, so locally an
// extensionless miss retries web/pages/<name>.html before 404ing.
function fallback(urlPath) {
  const p = decodeURIComponent(urlPath.split('?')[0]);
  if (path.extname(p) || p.includes('..')) return null;
  return path.join(ROOT, 'web', 'pages', p.slice(1) + '.html');
}

// Optional same-origin proxy to a locally running relay, so a page that TALKS
// to the relay can actually be exercised here instead of only being looked at.
//
//   node scripts/preview-server.js 8099 --api http://127.0.0.1:8787
//   HUMANITY_PREVIEW_API=http://127.0.0.1:8787 node scripts/preview-server.js
//
// Without this, /api and /ws 404 and every relay-backed page can only be
// verified as far as its empty state, which is how a broken admin call ships:
// the page looks right and nobody ever watched a request leave it. Proxying
// through the SAME origin also sidesteps the relay's browser-origin allowlist,
// which does not (and should not) list this dev port.
const API_TARGET = (() => {
  const flag = process.argv.indexOf('--api');
  const raw = (flag !== -1 && process.argv[flag + 1]) || process.env.HUMANITY_PREVIEW_API || '';
  if (!raw) return null;
  try {
    return new URL(raw);
  } catch {
    console.warn(`preview server: ignoring unparseable --api value "${raw}"`);
    return null;
  }
})();

function proxyToRelay(req, res) {
  const opts = {
    hostname: API_TARGET.hostname,
    port: API_TARGET.port || (API_TARGET.protocol === 'https:' ? 443 : 80),
    path: req.url,
    method: req.method,
    // Present the relay's own origin as the Host so it sees a request that
    // looks local to it, not one from this dev port.
    headers: { ...req.headers, host: API_TARGET.host },
  };
  const client = API_TARGET.protocol === 'https:' ? require('https') : http;
  const upstream = client.request(opts, (up) => {
    res.writeHead(up.statusCode || 502, up.headers);
    up.pipe(res);
  });
  upstream.on('error', (e) => {
    res.writeHead(502, { 'Content-Type': 'text/plain' });
    res.end(`502: relay at ${API_TARGET.origin} did not answer (${e.message})`);
  });
  req.pipe(upstream);
}

http.createServer((req, res) => {
  if (API_TARGET && (req.url === '/health' || req.url.startsWith('/api/'))) {
    proxyToRelay(req, res);
    return;
  }
  let file = resolve(req.url);
  fs.stat(file, (err, st) => {
    if (!err && st.isDirectory()) file = path.join(file, 'index.html');
    else if (err) {
      const retry = fallback(req.url);
      if (retry && fs.existsSync(retry)) file = retry;
    }
    fs.readFile(file, (e, buf) => {
      if (e) {
        res.writeHead(404, { 'Content-Type': 'text/plain' });
        res.end('404: ' + req.url);
        return;
      }
      const ext = path.extname(file).toLowerCase();
      res.writeHead(200, { 'Content-Type': TYPES[ext] || 'application/octet-stream' });
      res.end(buf);
    });
  });
}).listen(PORT, () => {
  console.log(`preview server on http://localhost:${PORT}  (root: ${ROOT})`);
  if (API_TARGET) console.log(`  /api and /health proxied to ${API_TARGET.origin}`);
  else console.log('  /api not proxied (pass --api http://127.0.0.1:PORT to reach a local relay)');
});
