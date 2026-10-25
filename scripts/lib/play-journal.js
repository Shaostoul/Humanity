// Reading the game's play journal (docs/design/play-journal.md): the folder beside the game's logs
// where src/engine/play_journal.rs writes <kind>.jsonl, one JSON line per event, and marks/<n>.png.
// One reader for the dev MCP bridge (scripts/mcp-server.js: dev_play_journal, dev_play_marks) and
// the rigs (scripts/probe-sweep.js: a vantage's mark and, when it fails, what happened since).
"use strict";

const fs = require("fs");
const os = require("os");
const path = require("path");

/** The journal folder: `folder` when given, else the operator's install's (%APPDATA%/HumanityOS/journal). */
function journalDir(folder) {
  if (folder) return path.resolve(String(folder));
  const base = process.env.APPDATA || path.join(os.homedir(), ".local", "share");
  return path.join(base, "HumanityOS", "journal");
}

/** One kind's events, oldest first; a line that does not parse (half-written) is left out. */
function readKind(dir, kind) {
  let text = "";
  try { text = fs.readFileSync(path.join(dir, kind + ".jsonl"), "utf8"); } catch (_) { return []; }
  const out = [];
  for (const line of text.split(/\r?\n/)) {
    if (!line.trim()) continue;
    try { out.push(JSON.parse(line)); } catch (_) { /* a half-written line */ }
  }
  return out;
}

/** The kinds that have a file in the folder. */
function kindsIn(dir) {
  try { return fs.readdirSync(dir).filter((n) => n.endsWith(".jsonl")).map((n) => n.slice(0, -6)).sort(); } catch (_) { return []; }
}

/** Events of `kinds` (default every kind) from `since` to `until` (Unix ms, inclusive), oldest first, each with its kind. */
function eventsBetween(dir, since = 0, until = Infinity, kinds = null) {
  const out = [];
  for (const kind of kinds || kindsIn(dir)) {
    for (const e of readKind(dir, kind)) {
      const t = Number(e.t);
      if (t >= since && t <= until) out.push({ kind, ...e });
    }
  }
  return out.sort((a, b) => a.t - b.t);
}

/** The newest mark with `label` made at or after `after` (Unix ms), or null. */
function markFor(dir, label, after = 0) {
  const marks = readKind(dir, "mark").filter((m) => m.label === label && Number(m.t) >= after);
  return marks.length ? marks[marks.length - 1] : null;
}

/** One event as a line of words: its time, kind, level and words. */
function eventLine(e) {
  const iso = Number.isFinite(e.t) ? new Date(e.t).toISOString() : "?";
  return `${iso} ${e.kind}${e.level ? " " + e.level : ""}: ${e.what || ""}${e.picture ? " [" + e.picture + "]" : ""}`;
}

module.exports = { journalDir, readKind, kindsIn, eventsBetween, markFor, eventLine };
