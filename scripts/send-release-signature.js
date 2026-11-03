#!/usr/bin/env node
// Tell the website's server that a release is signed (2026-11-02, docs/design/front-door.md 3.1).
//
// The front door and the Devlog mark a signed release "Signed". The server learns it by checking
// the signatures ITSELF: this sends a release's release-manifest.json and its
// release-manifest.json.sig.json, as text, to POST /api/release-signature, and the server keeps them
// only when both signatures (Ed25519 and Dilithium3) verify against the project's release public
// keys compiled into it (crates/humanity-protocol/src/release_signature.rs, the same check the
// desktop updater makes before it installs). No secret is sent or needed: only the project's keys
// make a pair that passes, so this is safe to run from anywhere, any number of times.
//
// Usage:
//   node scripts/send-release-signature.js <manifest> <signature>   one release's two files
//   node scripts/send-release-signature.js --backfill               every release on GitHub that
//                                                                   carries both files (needs gh)
// Options: --server <url> (default $HUMANITY_SERVER_URL, else https://united-humanity.us).
//
// `just sign-release` runs the first form as its last step; `just send-release-signatures` runs the
// backfill (once, for the releases signed before the route existed) or retries one by version:
// `just send-release-signatures v0.1547.0`.
'use strict';

const fs = require('fs');
const os = require('os');
const path = require('path');
const { execFileSync } = require('child_process');

const REPO = 'Shaostoul/Humanity';
const MANIFEST = 'release-manifest.json';
const SIG = 'release-manifest.json.sig.json';

function usage(code) {
  console.error('usage: node scripts/send-release-signature.js <manifest> <signature> [--server <url>]');
  console.error('       node scripts/send-release-signature.js --backfill [--server <url>]');
  console.error('       node scripts/send-release-signature.js --release vX.Y.Z [--server <url>]');
  process.exit(code);
}

/** Send one pair; resolves to the server's answer. Never throws for a refusal: it is reported. */
async function send(server, manifest, signature) {
  const res = await fetch(server.replace(/\/+$/, '') + '/api/release-signature', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ manifest, signature }),
  });
  let body = null;
  try { body = await res.json(); } catch (e) { body = { error: 'the server answered ' + res.status + ' with no JSON' }; }
  return { status: res.status, body };
}

/** The tags of the releases on GitHub that carry both signed files, newest first. */
function signedTags() {
  const out = execFileSync('gh', [
    'api', '--paginate', `repos/${REPO}/releases?per_page=100`,
    '--jq', `.[] | select(any(.assets[]; .name == "${SIG}")) | select(any(.assets[]; .name == "${MANIFEST}")) | .tag_name`,
  ], { encoding: 'utf8' });
  return out.split(/\r?\n/).map((s) => s.trim()).filter(Boolean);
}

/** Download one release's pair into a fresh folder; returns the two files' text. */
function download(tag) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'hos-relsig-'));
  try {
    execFileSync('gh', ['release', 'download', tag, '--repo', REPO, '-D', dir, '--pattern', MANIFEST, '--pattern', SIG], { stdio: 'ignore' });
    return { manifest: fs.readFileSync(path.join(dir, MANIFEST), 'utf8'), signature: fs.readFileSync(path.join(dir, SIG), 'utf8') };
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

function report(label, r) {
  if (r.status === 200 && r.body && r.body.ok) {
    console.log(`  ok    ${label}: v${r.body.version} is marked signed on the server (${r.body.kept})`);
    return true;
  }
  console.error(`  FAIL  ${label}: ${r.status} ${(r.body && r.body.error) || JSON.stringify(r.body)}`);
  return false;
}

async function main() {
  const args = process.argv.slice(2);
  let server = process.env.HUMANITY_SERVER_URL || 'https://united-humanity.us';
  const si = args.indexOf('--server');
  if (si >= 0) { server = args[si + 1]; if (!server) usage(2); args.splice(si, 2); }

  let ok = true;
  if (args[0] === '--backfill' || args[0] === '--release') {
    const tags = args[0] === '--backfill' ? signedTags() : [args[1]];
    if (!tags.length || !tags[0]) usage(2);
    console.log(`Sending ${tags.length} signed release(s) to ${server}:`);
    for (const tag of tags) {
      try {
        const pair = download(tag);
        ok = report(tag, await send(server, pair.manifest, pair.signature)) && ok;
      } catch (e) {
        console.error(`  FAIL  ${tag}: ${e.message}`);
        ok = false;
      }
    }
  } else {
    if (args.length !== 2) usage(2);
    const [m, s] = args;
    const r = await send(server, fs.readFileSync(m, 'utf8'), fs.readFileSync(s, 'utf8')).catch((e) => ({ status: 0, body: { error: e.message } }));
    ok = report(path.basename(m), r);
  }
  if (!ok) {
    console.error('Not every release was marked. The signatures on GitHub are unaffected; retry with');
    console.error('  just send-release-signatures vX.Y.Z');
    process.exit(1);
  }
}

main();
