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
 * Checks recent releases for the signature asset and reports which version a desktop
 * client is ACTUALLY offered. That is the newest SIGNED release, not the newest release:
 * src/updater.rs filters to eligible releases and takes the first, so an unsigned newest
 * release leaves auto-update working but behind. Exits non-zero ONLY when NO signed
 * release exists, which is the case where auto-update genuinely offers nothing.
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
// What a desktop client ACTUALLY installs. src/updater.rs builds an `eligible`
// predicate (non-prerelease, non-draft, carries the manifest when signing keys
// are provisioned) and then does `.find(|r| eligible(r))` over the release list,
// newest first. So it offers the newest SIGNED release, not the newest release.
const offered = rows.find(r => r.signed && !r.prerelease) || null;

console.log('');
if (!offered) {
  // The genuinely broken case: nothing at all is installable.
  console.log('  >> NO SIGNED RELEASE EXISTS. Desktop auto-update offers nothing at all,');
  console.log('  >> and it fails silently: an ineligible release is invisible, not an error.');
  console.log(`  >> Fix (operator only): export HUMANITY_SIGNING_PASSPHRASE=... && just sign-release ${latest ? latest.tag : '<tag>'}`);
  process.exitCode = 1;
} else if (latest && !latest.signed) {
  // Auto-update is ALIVE, just behind. This used to be reported as "the
  // updater offers nothing", which is wrong and sent at least one session
  // chasing a broken update path that was working.
  console.log(`  Desktop auto-update is LIVE: users are offered ${offered.tag}.`);
  console.log(`  The newest release (${latest.tag}) is UNSIGNED, so nobody is offered it yet.`);
  console.log(`  Sign it when convenient: just sign-release ${latest.tag}`);
  if (unsignedCount > 1) {
    console.log(`  (${unsignedCount} releases newer than ${offered.tag} are unsigned.)`);
  }
  // Deliberately exit 0: being one or more releases behind is a backlog, not a
  // failure, and a red exit here trains people to ignore this check.
} else {
  console.log(`  Latest (${latest.tag}) is signed. Desktop auto-update is live and current.`);
  if (unsignedCount > 0) {
    console.log(`  (${unsignedCount} older release(s) unsigned; harmless, the updater takes the newest signed one.)`);
  }
}
