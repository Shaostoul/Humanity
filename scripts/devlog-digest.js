#!/usr/bin/env node
// Weekly devlog digest generator (2026-08-01, outreach arc).
//
// Turns the week's release history into a ready-to-post draft, so sharing
// progress costs the operator a read-and-paste instead of an hour of
// remembering what happened. Nothing is posted automatically; this only
// writes a draft file for human review.
//
//   node scripts/devlog-digest.js           # last 7 days
//   node scripts/devlog-digest.js 14        # last N days
//
// Output: devlog-drafts/digest-<date>.md (folder is gitignored-optional;
// commit a draft only if you want it archived). Uses `git tag` + `gh release
// view` locally, so it works offline for tags and degrades gracefully if the
// gh CLI is unavailable.

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const days = parseInt(process.argv[2] || '7', 10);
const since = new Date(Date.now() - days * 24 * 3600 * 1000);

function sh(cmd) {
  try { return execSync(cmd, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim(); }
  catch (e) { return ''; }
}

// Tags created in the window, oldest first, with dates.
const raw = sh('git for-each-ref --sort=creatordate --format "%(refname:short)|%(creatordate:iso8601)" refs/tags');
const tags = raw.split('\n').filter(Boolean).map(l => {
  const [tag, date] = l.split('|');
  return { tag, date: new Date(date) };
}).filter(t => t.date >= since && /^v\d/.test(t.tag));

if (!tags.length) {
  console.log(`No release tags in the last ${days} days; nothing to digest.`);
  process.exit(0);
}

// The descriptive one-liner lives in the tagged commit's subject (release
// titles are bare version numbers per the Version SOP), so read it from git:
// works offline, no gh CLI needed.
const items = tags.map(t => {
  const subject = sh(`git log -1 --format=%s ${t.tag}`);
  const title = (subject || t.tag).replace(/^v[\d.]+\s*/, '').trim() || t.tag;
  return { tag: t.tag, date: t.date.toISOString().slice(0, 10), title };
});

const first = tags[0].tag, last = tags[tags.length - 1].tag;
const today = new Date().toISOString().slice(0, 10);

const highlights = items
  .slice(-8)
  .map(i => `- ${i.tag} (${i.date}): ${i.title}`)
  .join('\n');

const shortPost =
`${items.length} releases shipped this week (${first} -> ${last}).
Built in the open, one human + AI, on $380/month.
Full devlog: https://united-humanity.us/devlog`;

const draft = `# Devlog digest: ${first} -> ${last} (${today})

${items.length} releases in the last ${days} days.

## Suggested short post (X/Threads/Facebook; pairs well with 1-2 screenshots)

${shortPost}

## Highlights (edit down to the 3-5 that had visuals)

${highlights || '(gh CLI unavailable; fill highlights from https://united-humanity.us/devlog)'}

## All releases this window

${items.map(i => `- ${i.tag} (${i.date}): ${i.title}`).join('\n')}

## Capture checklist

- In-game beauty shot: drop debug/screenshot_request.json while the game runs,
  grab debug/screenshot_N.png
- UI pages: just snapshot <name> renders any page headlessly
- Probe-rig captures from verify runs live under .probe-rig*/
`;

const outDir = path.join(__dirname, '..', 'devlog-drafts');
fs.mkdirSync(outDir, { recursive: true });
const outFile = path.join(outDir, `digest-${today}.md`);
fs.writeFileSync(outFile, draft);
console.log(`Wrote ${outFile} (${items.length} releases, ${first} -> ${last})`);
