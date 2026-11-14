// Test discovery for `just rig-tests` and `just lints` (2026-11-14).
//
// Why: both recipes used to be ONE hand-kept line naming every test file (174 names for
// rig-tests, 29 for lints). Every agent that added a test edited that line, so parallel merges
// conflicted on it, and a conflict resolved in a hurry silently DROPPED names: on 2026-11-14
// seven tests whose own headers say "in `just rig-tests`" (admin-actions-web, key-card-web,
// key-cards-chat-web, person-controls-web, seed-target, tool-guard, video-dock-web) ran nowhere.
// Now the folder is the list. scripts/test-discovery.json holds the few files that must not run
// there, each with its reason; a name there with no file is an error, so the list cannot rot.
"use strict";

const fs = require("fs");
const path = require("path");

const LIST = path.join(__dirname, "..", "test-discovery.json");

/** The excludes for one section ("rig_tests" or "lints"): { name: reason }. */
function excludes(section, listPath = LIST) {
  const all = JSON.parse(fs.readFileSync(listPath, "utf8"));
  const s = all[section];
  if (!s || typeof s.exclude !== "object") throw new Error(`${listPath} has no "${section}.exclude"`);
  return s.exclude;
}

/**
 * Every file in `dir` whose name ends in `suffix`, sorted, minus `exclude` (names WITH the
 * suffix stripped when `stripSuffix`). Returns { names, problems }: problems are an exclude with
 * no file or an exclude without a reason. Sorted by plain code-unit order so every machine runs
 * the same list in the same order.
 */
function discover({ dir, suffix, exclude, stripSuffix = false }) {
  const problems = [];
  const onDisk = fs
    .readdirSync(dir, { withFileTypes: true })
    .filter((e) => e.isFile() && e.name.endsWith(suffix))
    .map((e) => (stripSuffix ? e.name.slice(0, -suffix.length) : e.name))
    .sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  const have = new Set(onDisk);
  for (const [name, reason] of Object.entries(exclude)) {
    if (!have.has(name)) problems.push(`"${name}" is excluded in scripts/test-discovery.json but no such file is in ${dir}`);
    if (typeof reason !== "string" || reason.trim().length < 10) problems.push(`"${name}" is excluded without a reason (say why it must not run)`);
  }
  return { names: onDisk.filter((n) => !(n in exclude)), problems };
}

module.exports = { discover, excludes, LIST };
