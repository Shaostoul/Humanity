#!/usr/bin/env node
// snapshot-diff.js (2026-07-02): after regenerating UI snapshots (`just
// snapshots`), report which pages' PNGs actually CHANGED versus the committed
// versions -- the mechanical "did my change alter any page I didn't intend to
// touch?" check that previously required eyeballing 27 images by hand.
//
// Usage:  node scripts/snapshot-diff.js            (report only)
//         node scripts/snapshot-diff.js --strict   (exit 1 if anything changed)
//
// How: `git status` finds byte-changed PNGs (the offscreen renderer is
// deterministic on one machine, so unchanged pages are byte-identical). For
// each changed file, if ffmpeg is on PATH we also compute PSNR against the
// committed version: "inf" means byte noise only (e.g. encoder metadata),
// bigger visual changes score lower (typically < 40 dB). Without ffmpeg the
// changed-file list alone is still the answer.

const { execSync, spawnSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');

const repo = path.resolve(__dirname, '..');
const snapDir = 'tests/snapshots';

function sh(cmd) {
  return execSync(cmd, { cwd: repo, encoding: 'utf8' });
}

const strict = process.argv.includes('--strict');
const status = sh(`git status --porcelain -- ${snapDir}`)
  .split('\n')
  .map((l) => l.trim())
  .filter(Boolean);

const changed = status
  .filter((l) => l.startsWith('M ') || l.startsWith(' M'))
  .map((l) => l.slice(2).trim());
const added = status
  .filter((l) => l.startsWith('??'))
  .map((l) => l.slice(2).trim());

if (changed.length === 0 && added.length === 0) {
  console.log('snapshot-diff: no snapshot changes vs the committed versions.');
  process.exit(0);
}

let haveFfmpeg = false;
try {
  execSync('ffmpeg -version', { stdio: 'ignore' });
  haveFfmpeg = true;
} catch (_) {
  /* optional */
}

function psnr(file) {
  // Extract the committed version to a temp file and compare.
  const tmp = path.join(os.tmpdir(), 'snapdiff-head.png');
  try {
    const head = execSync(`git show HEAD:${file.replace(/\\/g, '/')}`, {
      cwd: repo,
      maxBuffer: 64 * 1024 * 1024,
    });
    fs.writeFileSync(tmp, head);
  } catch (_) {
    return 'new-in-HEAD?';
  }
  const out = spawnSync(
    'ffmpeg',
    ['-hide_banner', '-i', path.join(repo, file), '-i', tmp, '-filter_complex', 'psnr', '-f', 'null', '-'],
    { encoding: 'utf8' }
  );
  const m = (out.stderr || '').match(/average:([0-9.]+|inf)/);
  return m ? `${m[1]} dB` : 'psnr-unavailable';
}

if (changed.length) {
  console.log(`snapshot-diff: ${changed.length} page(s) CHANGED vs committed:`);
  for (const f of changed) {
    const score = haveFfmpeg ? `  (PSNR avg ${psnr(f)})` : '';
    console.log(`  CHANGED  ${f}${score}`);
  }
}
if (added.length) {
  console.log(`snapshot-diff: ${added.length} NEW snapshot(s):`);
  for (const f of added) console.log(`  NEW      ${f}`);
}
console.log(
  haveFfmpeg
    ? 'Note: "inf dB" = pixel-identical (encoder byte noise only); < ~45 dB = visible change, open the PNG.'
    : 'Tip: install ffmpeg to get PSNR scores (distinguishes byte noise from visible changes).'
);
process.exit(strict && changed.length ? 1 : 0);
