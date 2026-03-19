#!/usr/bin/env node
/**
 * bundle-web.js — Build a local web bundle for desktop offline mode.
 *
 * Copies all web-servable files into desktop/src-tauri/web/ and generates
 * a manifest.json with SHA-256 hashes so the desktop app can compare
 * local vs remote for background sync.
 */

const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const ROOT = path.resolve(__dirname, '..');
const OUT = path.join(ROOT, 'desktop', 'src-tauri', 'web');

// Source directories to bundle: [src relative to ROOT, dest relative to OUT, extensions]
const SOURCES = [
  { src: 'shared',                          dest: 'shared',           exts: ['.js', '.css', '.json'] },
  { src: 'shared/icons',                    dest: 'shared/icons',     exts: ['.png', '.svg', '.ico'] },
  { src: 'pages',                           dest: 'pages',            exts: ['.html', '.js', '.css'] },
  { src: 'game',                            dest: 'game',             exts: ['.html'] },
  { src: 'game/js',                         dest: 'game/js',          exts: ['.js'] },
  { src: 'crates/humanity-relay/client',    dest: 'client',           exts: ['.html', '.js', '.css', '.ico', '.png', '.svg'] },
  { src: 'assets/ui/icons',                 dest: 'assets/ui/icons',  exts: ['.png', '.svg'] },
];

function sha256(buffer) {
  return 'sha256:' + crypto.createHash('sha256').update(buffer).digest('hex');
}

function rmdir(dir) {
  if (fs.existsSync(dir)) {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

function copyFiles(srcDir, destDir, exts) {
  const copied = [];
  if (!fs.existsSync(srcDir)) return copied;

  const entries = fs.readdirSync(srcDir, { withFileTypes: true });
  for (const entry of entries) {
    if (!entry.isFile()) continue;
    const ext = path.extname(entry.name).toLowerCase();
    if (!exts.includes(ext)) continue;

    const srcPath = path.join(srcDir, entry.name);
    const destPath = path.join(destDir, entry.name);
    ensureDir(destDir);
    fs.copyFileSync(srcPath, destPath);

    const content = fs.readFileSync(destPath);
    const stat = fs.statSync(destPath);
    const modified = Math.floor(stat.mtimeMs / 1000);

    copied.push({
      path: '/' + path.relative(OUT, destPath).replace(/\\/g, '/'),
      hash: sha256(content),
      size: stat.size,
      modified,
    });
  }
  return copied;
}

// ── Main ────────────────────────────────────────────────────────────────────

console.log('Bundling web files for desktop...\n');

// Clean output directory.
rmdir(OUT);
ensureDir(OUT);

let allFiles = [];

for (const { src, dest, exts } of SOURCES) {
  const srcDir = path.join(ROOT, src);
  const destDir = path.join(OUT, dest);
  const copied = copyFiles(srcDir, destDir, exts);
  allFiles.push(...copied);
  if (copied.length > 0) {
    console.log(`  ${dest}/  ${copied.length} files`);
  }
}

// Sort by path for deterministic output.
allFiles.sort((a, b) => a.path.localeCompare(b.path));

const totalSize = allFiles.reduce((sum, f) => sum + f.size, 0);

// Read version from tauri.conf.json.
let version = '0.0.0';
try {
  const conf = JSON.parse(fs.readFileSync(path.join(ROOT, 'desktop', 'src-tauri', 'tauri.conf.json'), 'utf8'));
  version = conf.version || version;
} catch { /* use fallback */ }

// Create index.html redirect.
const indexHtml = `<!DOCTYPE html>
<html>
<head><meta http-equiv="refresh" content="0;url=/client/index.html"><title>Redirecting...</title></head>
<body><p>Redirecting to <a href="/client/index.html">chat</a>...</p></body>
</html>
`;
fs.writeFileSync(path.join(OUT, 'index.html'), indexHtml);

// Add index.html to manifest.
const indexContent = fs.readFileSync(path.join(OUT, 'index.html'));
const indexStat = fs.statSync(path.join(OUT, 'index.html'));
allFiles.push({
  path: '/index.html',
  hash: sha256(indexContent),
  size: indexStat.size,
  modified: Math.floor(indexStat.mtimeMs / 1000),
});

// Write manifest.
const manifest = {
  version,
  files: allFiles,
  total_size: totalSize + indexStat.size,
  file_count: allFiles.length,
};
fs.writeFileSync(path.join(OUT, 'manifest.json'), JSON.stringify(manifest, null, 2));

// Stats.
const sizeMB = (manifest.total_size / 1024 / 1024).toFixed(2);
console.log(`\nBundle complete:`);
console.log(`  Files:  ${manifest.file_count}`);
console.log(`  Size:   ${sizeMB} MB`);
console.log(`  Output: ${OUT}`);
