#!/usr/bin/env node
'use strict';
/**
 * serve-ai-files.js (2026-11-09, embeddable AI rung 2; docs/design/embeddable-ai.md 6.2): serve the
 * staged `assets-ai-1` files from this computer, so Settings > AI can download, resume and install them
 * before they are uploaded. Binds 127.0.0.1 only (no firewall prompt, nothing reachable from outside),
 * and answers `Range: bytes=N-` with the rest of the file (206), as GitHub's servers do, so resuming can
 * be tried.
 *
 *   node scripts/serve-ai-files.js <staged folder> [port]
 *   node scripts/ai-catalog.js --print --base-url http://127.0.0.1:<port>/ > <scratch>/dev-catalog.ron
 *   then run the app with HUMANITY_AI_CATALOG=<scratch>/dev-catalog.ron (a full path)
 *
 * Serves plain files by name from that one folder, nothing under it and nothing above it.
 */

const fs = require('fs');
const http = require('http');
const path = require('path');

const dir = path.resolve(process.argv[2] || '.');
const port = Number(process.argv[3] || 8001);

http
  .createServer((req, res) => {
    const name = decodeURIComponent((req.url || '/').split('?')[0].replace(/^\/+/, ''));
    const file = path.join(dir, name);
    if (!name || name.includes('/') || name.includes('\\') || name.includes('..') || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
      res.writeHead(404).end('not found\n');
      return;
    }
    const size = fs.statSync(file).size;
    const m = /^bytes=(\d+)-$/.exec(req.headers.range || '');
    const from = m ? Number(m[1]) : 0;
    if (m && from < size) {
      res.writeHead(206, { 'Content-Length': size - from, 'Content-Range': `bytes ${from}-${size - 1}/${size}`, 'Content-Type': 'application/octet-stream' });
      fs.createReadStream(file, { start: from }).pipe(res);
    } else {
      res.writeHead(200, { 'Content-Length': size, 'Content-Type': 'application/octet-stream' });
      fs.createReadStream(file).pipe(res);
    }
    console.log(`${new Date().toISOString()} ${name}${m ? ` from ${from}` : ''}`);
  })
  .listen(port, '127.0.0.1', () => {
    console.log(`serving ${dir} on http://127.0.0.1:${port}/`);
  });
