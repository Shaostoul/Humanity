#!/usr/bin/env node
'use strict';
/**
 * serve-instrument-packs.js (2026-11-06, docs/ai/release.md "Instrument packs"): serve the built
 * instrument packs from this computer, so Settings > Audio > Instruments can install them before
 * they are uploaded. Binds 127.0.0.1 only (no firewall prompt, nothing reachable from outside).
 *
 *   node scripts/serve-instrument-packs.js [folder] [port]
 *   then run the app with HUMANITY_INSTRUMENT_PACKS_URL=http://127.0.0.1:<port>/
 *
 * Defaults: E:\HumanityOS\instrument-packs (or HUMANITY_INSTRUMENT_PACKS_OUT), port 8000. Serves
 * plain files by name from that one folder, nothing under it and nothing above it.
 */

const fs = require('fs');
const http = require('http');
const path = require('path');

const dir = path.resolve(process.argv[2] || process.env.HUMANITY_INSTRUMENT_PACKS_OUT || 'E:\\HumanityOS\\instrument-packs');
const port = Number(process.argv[3] || 8000);

http.createServer((req, res) => {
  const name = decodeURIComponent((req.url || '/').split('?')[0].replace(/^\/+/, ''));
  const file = path.join(dir, name);
  if (!name || name.includes('/') || name.includes('\\') || name.includes('..') || !fs.existsSync(file) || !fs.statSync(file).isFile()) {
    res.writeHead(404).end('not found\n');
    return;
  }
  res.writeHead(200, { 'Content-Length': fs.statSync(file).size, 'Content-Type': 'application/x-tar' });
  fs.createReadStream(file).pipe(res);
  console.log(`${new Date().toISOString()} ${name}`);
}).listen(port, '127.0.0.1', () => {
  console.log(`serving ${dir} on http://127.0.0.1:${port}/ (set HUMANITY_INSTRUMENT_PACKS_URL to that address)`);
});
