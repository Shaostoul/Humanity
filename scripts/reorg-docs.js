#!/usr/bin/env node
// One-shot docs reorg: move loose root docs into audience/topic folders and rewrite
// every repo-root-style reference (docs/OLD -> docs/NEW) across CLAUDE.md, docs/,
// web/, scripts/, and src/ comments. Filenames are kept identical (folder-only moves)
// so the rewrite is a simple, auditable path-prefix change. Relative markdown links
// inside moved files are NOT rewritten here, scripts/check-doc-links.js finds those and
// they are fixed in a follow-up pass.
//
//   node scripts/reorg-docs.js --dry-run   # show the plan, touch nothing
//   node scripts/reorg-docs.js             # execute (git mv + reference rewrite)

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const ROOT = path.join(__dirname, '..');
const DRY = process.argv.includes('--dry-run');

// folder -> [filenames moved from docs/ root into docs/<folder>/]
const MOVES = {
  'history': [
    'roadmap-2026-05-01.md', 'forward-roadmap-2026-04-30.md', 'history.md',
    'our_homesteading_future.md', 'knowledge-gardening.md',
    'md_information_architecture_plan.md', 'earth_fleet_twin.md',
  ],
  'admin': [
    'SELF-HOSTING.md', 'forgejo-setup.md', 'torrent-infrastructure.md',
    'distribution-mirrors.md', 'release-signing.md',
  ],
  'design': [
    'DESIGN.md', 'action_log.md', 'feature_web.md', 'header_navigation_architecture.md',
    'humanity_full_replacement_blueprint.md', 'market_integration_architecture.md',
  ],
  'ai': [
    '05-AI-ONBOARDING.md', 'ai-onboarding.md', 'AGENTS.md', 'BOOTSTRAP.md',
    'OPERATING_CONTRACT.md',
  ],
  'user': [
    'ONBOARDING.md',
  ],
  'contributor': [
    '00-START-HERE.md', '01-VISION.md', '02-ARCHITECTURE.md', '03-MODULE-MAP.md',
    '04-CONTRIBUTING.md', '06-SOURCE-OF-TRUTH-MAP.md', '07-MODULE-SPEC-TEMPLATE.md',
    '08-V1-MODULE-BACKBONE.md', '09-LIFEFORM-PARITY-FRAMEWORK.md',
    'ENGINE_REFERENCE.md', 'development_loop.md', 'validate_data.md',
  ],
};

// Build the old->new map (paths relative to repo root, forward slashes).
const map = [];   // [{ old: 'docs/ai/AGENTS.md', neu: 'docs/ai/AGENTS.md' }]
for (const folder of Object.keys(MOVES)) {
  for (const f of MOVES[folder]) {
    map.push({ old: `docs/${f}`, neu: `docs/${folder}/${f}`, folder, file: f });
  }
}

// 1) git mv each file (after ensuring the destination folder exists).
for (const m of map) {
  const src = path.join(ROOT, m.old);
  if (!fs.existsSync(src)) {
    console.warn(`SKIP (missing): ${m.old}`);
    continue;
  }
  const destDir = path.join(ROOT, 'docs', m.folder);
  if (!fs.existsSync(destDir)) {
    if (DRY) console.log(`mkdir docs/${m.folder}`);
    else fs.mkdirSync(destDir, { recursive: true });
  }
  if (DRY) {
    console.log(`git mv ${m.old} -> ${m.neu}`);
  } else {
    execSync(`git mv "${m.old}" "${m.neu}"`, { cwd: ROOT });
  }
}

// 2) Rewrite repo-root-style references docs/OLD -> docs/NEW everywhere.
// Replace longest paths first so no prefix collision (none here, but safe).
const ordered = [...map].sort((a, b) => b.old.length - a.old.length);

const SCAN_DIRS = ['docs', 'web', 'scripts', 'src'];
const SCAN_EXT = new Set(['.md', '.js', '.mjs', '.rs', '.json', '.html', '.css', '.toml']);
const SKIP = [
  path.join('docs', 'history', 'project-universe-site'),
  path.join('docs', 'website', '_includes', 'canon'),
  'node_modules', '.git', 'target',
];

function walk(dir, acc) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    const rel = path.relative(ROOT, full);
    if (SKIP.some((s) => rel === s || rel.startsWith(s + path.sep))) continue;
    if (entry.isDirectory()) walk(full, acc);
    else if (SCAN_EXT.has(path.extname(entry.name))) acc.push(full);
  }
  return acc;
}

const filesToScan = [path.join(ROOT, 'CLAUDE.md')];
for (const d of SCAN_DIRS) walk(path.join(ROOT, d), filesToScan);

let filesChanged = 0;
let totalRefs = 0;
for (const file of filesToScan) {
  let text = fs.readFileSync(file, 'utf8');
  let n = 0;
  for (const m of ordered) {
    // Match the old path when NOT already followed by more path chars (so docs/ai/AGENTS.md
    // does not match inside docs/AGENTS.md.bak etc.) and not already the new path.
    const re = new RegExp(m.old.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '(?![\\w./-])', 'g');
    text = text.replace(re, () => { n++; return m.neu; });
  }
  if (n > 0) {
    totalRefs += n;
    filesChanged++;
    if (!DRY) fs.writeFileSync(file, text);
  }
}

console.log('');
console.log(`${DRY ? '[dry-run] ' : ''}moved ${map.length} files into ${Object.keys(MOVES).length} folders`);
console.log(`rewrote ${totalRefs} repo-root-style references across ${filesChanged} files`);
console.log('Next: node scripts/check-doc-links.js  (fix any relative-link residue)');
