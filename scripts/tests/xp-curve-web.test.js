// The website's skill XP table is the game's (BUG-248, 2026-11-04), run under Node:
//
//   node --test scripts/tests/xp-curve-web.test.js      (in `just rig-tests`)
//
// The game levels a skill by ONE rule: the XP to go from level L-1 to level L is the skill's base
// times L to the 1.5, rounded down (crates/humanity-systems/src/skills/mod.rs
// `SkillDef::xp_for_level`). The website's Profile and Tasks pages level a person's skills from
// data/skills/xp_curve.json, which until this date held another curve (each step 1.5 times the
// last), so the two showed different levels for the same XP.
//
// The shared table: data/skills/xp_curve.json, held to the game's rule by the Rust test
// `the_websites_xp_curve_is_the_games_rule` (in `just validate-data` and the lib tests), which
// prints the right table when they differ. This file holds the website to that table.
//
// What it proves:
//  1. Both pages fetch data/skills/xp_curve.json and take its `xp_per_level` and `max_level`.
//  2. The copy each page keeps for when the fetch fails (offline) is the table, number for
//     number, so offline the website still shows the game's numbers.
//
// Seen red 2026-11-04 with the table made the game's and the pages' copies as they were (the
// old 100, 150, 225, ... curve): see the note on test 2.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const read = (rel) => fs.readFileSync(path.join(WEB, rel), "utf8");
const table = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "skills", "xp_curve.json"), "utf8"));

// The page's own copy: `<let|var> <name> = [ ... ];` and `<let|var> <max> = <n>;`.
function pageCopy(src, listName, maxName) {
  const list = src.match(new RegExp(`(?:let|var)\\s+${listName}\\s*=\\s*\\[([^\\]]*)\\]`));
  const max = src.match(new RegExp(`(?:let|var)\\s+${maxName}\\s*=\\s*(\\d+)`));
  assert.ok(list, `${listName} is declared`);
  assert.ok(max, `${maxName} is declared`);
  return { xp_per_level: list[1].split(",").map((n) => Number(n.trim())), max_level: Number(max[1]) };
}

const PAGES = [
  { file: "pages/tasks-app.js", list: "XP_PER_LEVEL", max: "MAX_LEVEL" },
  { file: "pages/profile.html", list: "SKILL_XP_PER_LEVEL", max: "SKILL_MAX_LEVEL" },
];

test("1. both pages level skills from data/skills/xp_curve.json", () => {
  assert.ok(Array.isArray(table.xp_per_level) && table.xp_per_level.length === table.max_level, "the table has a step for every level up to its last");
  for (const p of PAGES) {
    const src = read(p.file);
    assert.ok(src.includes("fetch('/data/skills/xp_curve.json')"), `${p.file} fetches the table`);
    assert.ok(src.includes(`${p.list} = d.xp_per_level`), `${p.file} takes its steps`);
    assert.ok(new RegExp(`${p.max} = d\\.max_level`).test(src), `${p.file} takes its last level`);
  }
});

// Seen red 2026-11-04: "pages/tasks-app.js keeps the table for when it is offline" (actual
// [100, 150, 225, 338, 507, 760, 1140, 1710, 2565, 3848], expected the game's 100, 282, 519, ...).
test("2. the copy each page keeps for when it is offline is the table", () => {
  for (const p of PAGES) {
    const copy = pageCopy(read(p.file), p.list, p.max);
    assert.deepStrictEqual(copy.xp_per_level, table.xp_per_level, `${p.file} keeps the table for when it is offline`);
    assert.strictEqual(copy.max_level, table.max_level, `${p.file} keeps its last level`);
  }
});
