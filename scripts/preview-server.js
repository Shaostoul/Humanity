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
// It does NOT proxy /api or /ws unless a relay is named (--api, below), so pages that
// need the live relay show their empty/offline state, which is fine for layout checks. Static-content pages
// (like the roadmap, which reads /data/roadmap.json) render fully. With a relay named, it also
// sends what nginx sends the relay: /uploads/* (the files people put up) and one Media item's
// own page, /files/<number> (2026-10-12, docs/design/media-round-5-plan.md 3.6), while /files
// itself stays the static page.
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

// What the relay answers rather than a file from disk, as nginx routes it
// (scripts/nginx/humanity.conf): /health, everything under /api/ and /uploads/, and one Media
// item's page, /files/<number> (a regular-expression location there; /files alone is a page).
function goesToRelay(url) {
  const p = url.split('?')[0];
  return p === '/health' || p.startsWith('/api/') || p.startsWith('/uploads/') || /^\/files\/\d+$/.test(p);
}

function proxyToRelay(req, res) {
  // X-Real-IP is set from the connection, as nginx's `proxy_set_header X-Real-IP $remote_addr`
  // does, and whatever the browser sent in it is dropped: the relay's Media counts tell people
  // apart by it (src/relay/files_counts.rs).
  const headers = { ...req.headers, host: API_TARGET.host };
  delete headers['x-real-ip'];
  if (req.socket.remoteAddress) headers['x-real-ip'] = req.socket.remoteAddress;
  const opts = {
    hostname: API_TARGET.hostname,
    port: API_TARGET.port || (API_TARGET.protocol === 'https:' ? 443 : 80),
    path: req.url,
    method: req.method,
    // Present the relay's own origin as the Host so it sees a request that
    // looks local to it, not one from this dev port.
    headers,
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

const server = http.createServer((req, res) => {
  if (API_TARGET && goesToRelay(req.url)) {
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
});

// The chat's WebSocket (/ws) too, when a relay is named (2026-10-12): the upgrade request is
// passed through as raw bytes with the relay's own Host and Origin (its browser-origin allowlist
// does not list this dev port), then the two sockets are joined. Two browser tabs on two origins
// (http://127.0.0.1:PORT and http://localhost:PORT keep separate storage, so two identities) can
// then talk through one local relay, the way a real two-person test needs.
server.on('upgrade', (req, socket, head) => {
  if (!API_TARGET || !req.url.startsWith('/ws')) { socket.destroy(); return; }
  const up = require('net').connect(Number(API_TARGET.port || 80), API_TARGET.hostname, () => {
    const headers = { ...req.headers, host: API_TARGET.host, origin: API_TARGET.origin };
    const CRLF = String.fromCharCode(13, 10);
    const lines = Object.entries(headers).map(([k, v]) => `${k}: ${v}`).join(CRLF);
    up.write(`${req.method} ${req.url} HTTP/1.1${CRLF}${lines}${CRLF}${CRLF}`);
    if (head && head.length) up.write(head);
    up.pipe(socket);
    socket.pipe(up);
  });
  up.on('error', () => socket.destroy());
  socket.on('error', () => up.destroy());
});

// Loopback only, like every dev listener here: on Windows a program listening on every address
// raises a firewall prompt once per program path (CLAUDE.md, "No Windows Firewall prompts").
server.listen(PORT, '127.0.0.1', () => {
  console.log(`preview server on http://127.0.0.1:${PORT}  (root: ${ROOT})`);
  if (API_TARGET) console.log(`  /api, /uploads, /files/<number>, /health and /ws proxied to ${API_TARGET.origin}`);
  else console.log('  /api not proxied (pass --api http://127.0.0.1:PORT to reach a local relay)');
});
