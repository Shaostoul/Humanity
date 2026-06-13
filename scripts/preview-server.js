#!/usr/bin/env node
// Tiny static preview server that mirrors the production nginx routing so web pages
// can be previewed locally exactly as they are served live:
//   /            -> web/index.html
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
  if (p === '/' || p === '') return path.join(ROOT, 'web', 'index.html');
  // Aliased top-level dirs served from the repo root (not from web/).
  for (const alias of ['/data/', '/docs/', '/assets/']) {
    if (p.startsWith(alias)) return path.join(ROOT, p.slice(1));
  }
  // Everything else is served from web/.
  return path.join(ROOT, 'web', p.slice(1));
}

http.createServer((req, res) => {
  let file = resolve(req.url);
  fs.stat(file, (err, st) => {
    if (!err && st.isDirectory()) file = path.join(file, 'index.html');
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
});
