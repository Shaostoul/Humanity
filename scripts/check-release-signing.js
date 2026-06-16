#!/usr/bin/env node
/**
 * check-release-signing.js -- surface the silent "desktop auto-update is dead" failure.
 *
 * The v0.421+ desktop updater REFUSES to install any release that lacks a
 * `release-manifest.json.sig.json` asset (see src/updater.rs MANIFEST_SIG_NAME +
 * "Refusing to install"). Signing is operator-only (`just sign-release vX.Y.Z`, needs the
 * passphrase). When signing is skipped, the updater silently offers NOTHING -- exactly the
 * gap that went unnoticed for ~48 releases (v0.421.0 -> v0.469.0). This check makes it loud.
 *
 * Checks the most recent releases for the required signature asset and reports which are
 * unsigned, calling out the LATEST (the one the updater would offer). Exits non-zero if the
 * latest non-prerelease release is unsigned, so `just status` shows it in red.
 *
 * Usage: node scripts/check-release-signing.js [limit]   (default limit 12)
 */
'use strict';
const { execSync } = require('child_process');

const REPO = 'Shaostoul/Humanity';
const SIG_ASSET = 'release-manifest.json.sig.json';
const limit = parseInt(process.argv[2] || '12', 10);

function gh(args) {
  return execSync(`gh ${args}`, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
}

let releases;
try {
  releases = JSON.parse(gh(`release list --repo ${REPO} --limit ${limit} --json tagName,isLatest,isPrerelease`));
} catch (e) {
  console.error('Release signing check: could not list releases (is gh authed?):', e.message);
  process.exit(0); // non-fatal: do not red the whole status run on a gh hiccup
}

const rows = [];
for (const r of releases) {
  let names = [];
  try {
    const assets = JSON.parse(gh(`release view ${r.tagName} --repo ${REPO} --json assets`)).assets || [];
    names = assets.map(a => a.name);
  } catch (e) {
    names = [];
  }
  rows.push({ tag: r.tagName, latest: r.isLatest, prerelease: r.isPrerelease, signed: names.includes(SIG_ASSET) });
}

console.log('── Release signing (desktop auto-update gate) ──');
for (const row of rows) {
  const mark = row.signed ? 'OK  signed' : 'XX  UNSIGNED';
  const flags = [row.latest ? 'LATEST' : '', row.prerelease ? 'prerelease' : ''].filter(Boolean).join(' ');
  console.log(`  ${mark}  ${row.tag}${flags ? '  (' + flags + ')' : ''}`);
}

const latest = rows.find(r => r.latest && !r.prerelease) || rows.find(r => !r.prerelease);
const unsignedCount = rows.filter(r => !r.signed && !r.prerelease).length;

if (latest && !latest.signed) {
  console.log('');
  console.log(`  >> LATEST release ${latest.tag} is UNSIGNED. The desktop updater offers nothing.`);
  console.log(`  >> Fix (operator only): export HUMANITY_SIGNING_PASSPHRASE=... && just sign-release ${latest.tag}`);
  process.exitCode = 1;
} else if (latest && latest.signed) {
  console.log('');
  console.log(`  Latest (${latest.tag}) is signed. Desktop auto-update is live.`);
  if (unsignedCount > 0) {
    console.log(`  (${unsignedCount} older release(s) unsigned -- harmless, the updater only offers the latest signed one.)`);
  }
}
