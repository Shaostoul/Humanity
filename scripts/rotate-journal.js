#!/usr/bin/env node
/**
 * Rotate the orchestrator journal so the JSON that loads every session stays small.
 *
 *   node scripts/rotate-journal.js [keep]     (default keep = 30; or: just rotate-journal)
 *
 * Moves all but the most recent `keep` decisions out of
 * data/coordination/orchestrator_state.json into
 * docs/history/journal-archive-<YYYY-MM>.md (grouped by the entry's month, appended,
 * never overwritten). ORDER IS PRESERVED: the file keeps newest-at-the-BOTTOM (its own
 * protocol, and what every `recent_decisions[last]` reader -- e.g. scripts/brief.js --
 * expects). We prune from the FRONT (oldest); we never reverse the array, which would
 * silently invert every consumer. The live journal's _purpose/_protocol/current_focus
 * and all other fields are untouched.
 *
 * Idempotent-ish: re-running with the same `keep` after a rotation finds nothing to do.
 * Safe: the archive file is written BEFORE the journal is truncated, so history is
 * never lost even if the process dies between writes.
 */
const fs = require('fs');
const path = require('path');

const KEEP = Math.max(1, parseInt(process.argv[2], 10) || 30);
const ROOT = path.join(__dirname, '..');
const JOURNAL = path.join(ROOT, 'data', 'coordination', 'orchestrator_state.json');
const HIST = path.join(ROOT, 'docs', 'history');

const j = JSON.parse(fs.readFileSync(JOURNAL, 'utf8'));
const rd = Array.isArray(j.recent_decisions) ? j.recent_decisions : [];

if (rd.length <= KEEP) {
  console.log(`No rotation needed: ${rd.length} decisions <= keep ${KEEP}.`);
  process.exit(0);
}

const toArchive = rd.slice(0, rd.length - KEEP); // oldest, from the front
const keep = rd.slice(rd.length - KEEP); // newest, stay in the JSON (newest at bottom)

// Group by month from `at` (YYYY-MM), preserving within-group order.
const byMonth = {};
for (const e of toArchive) {
  const m = typeof e.at === 'string' && /^\d{4}-\d{2}/.test(e.at) ? e.at.slice(0, 7) : 'undated';
  (byMonth[m] = byMonth[m] || []).push(e);
}

fs.mkdirSync(HIST, { recursive: true });
let archived = 0;
for (const [month, entries] of Object.entries(byMonth)) {
  const file = path.join(HIST, `journal-archive-${month}.md`);
  let md = fs.existsSync(file)
    ? fs.readFileSync(file, 'utf8')
    : `# Orchestrator journal archive -- ${month}\n\n` +
      `Decisions rotated out of \`data/coordination/orchestrator_state.json\` (oldest first ` +
      `within each batch; newest overall is in the live journal). Source of truth for "why ` +
      `we did X" once it ages past the live tail. See also git log + the GitHub releases.\n`;
  for (const e of entries) {
    md += `\n## ${e.at || 'undated'}\n\n`;
    if (e.decision) md += `**Decision:** ${e.decision}\n\n`;
    if (e.why) md += `**Why:** ${e.why}\n\n`;
    if (Array.isArray(e.files) && e.files.length) md += `**Files:** ${e.files.join(', ')}\n\n`;
    archived++;
  }
  fs.writeFileSync(file, md);
  console.log(`  archived ${entries.length} -> docs/history/journal-archive-${month}.md`);
}

// Only NOW truncate the live journal (archive is safely on disk).
j.recent_decisions = keep;
fs.writeFileSync(JOURNAL, JSON.stringify(j, null, 2) + '\n');
console.log(`Rotated: ${archived} archived, ${keep.length} kept in the live journal (newest at bottom).`);
