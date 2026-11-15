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
//  1. Both pages fetch data/skills/xp_curve.json and take its `xp_per_level` and its `base`.
//  2. The copy each page keeps for when the fetch fails (offline) is the table, number for
//     number, so offline the website still shows the game's numbers.
//  3. No cap (skills rung 1, 2026-11-14; the operator: "Let's not focus on a cap."): neither page
//     keeps a highest level, and past the table each uses the game's formula, base x level^1.5.
//
// Seen red 2026-11-04 with the table made the game's and the pages' copies as they were (the
// old 100, 150, 225, ... curve): see the note on test 2. Test 3 seen red 2026-11-14 with the pages
// as they were: "pages/tasks-app.js keeps no highest level" (MAX_LEVEL = 10).

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const read = (rel) => fs.readFileSync(path.join(WEB, rel), "utf8");
const table = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "skills", "xp_curve.json"), "utf8"));

// The page's own copy: `<let|var> <name> = [ ... ];`.
function pageCopy(src, listName) {
  const list = src.match(new RegExp(`(?:let|var)\\s+${listName}\\s*=\\s*\\[([^\\]]*)\\]`));
  assert.ok(list, `${listName} is declared`);
  return list[1].split(",").map((n) => Number(n.trim()));
}

const PAGES = [
  { file: "pages/tasks-app.js", list: "XP_PER_LEVEL", base: "XP_BASE" },
  { file: "pages/profile.html", list: "SKILL_XP_PER_LEVEL", base: "SKILL_XP_BASE" },
];

test("1. both pages level skills from data/skills/xp_curve.json", () => {
  assert.ok(Array.isArray(table.xp_per_level) && table.xp_per_level.length === table.levels_listed, "the table has a step for each level it lists");
  for (const p of PAGES) {
    const src = read(p.file);
    assert.ok(src.includes("fetch('/data/skills/xp_curve.json')"), `${p.file} fetches the table`);
    assert.ok(src.includes(`${p.list} = d.xp_per_level`), `${p.file} takes its steps`);
    assert.ok(new RegExp(`${p.base} = d.base`).test(src), `${p.file} takes its base`);
  }
});

test("2. the copy each page keeps for when it is offline is the table", () => {
  for (const p of PAGES) {
    assert.deepStrictEqual(pageCopy(read(p.file), p.list), table.xp_per_level, `${p.file} keeps the table for when it is offline`);
  }
});

test("3. no page keeps a highest level, and past the table each uses the game's formula", () => {
  for (const p of PAGES) {
    const src = read(p.file);
    assert.ok(!/MAX_LEVEL/.test(src), `${p.file} keeps no highest level`);
    assert.ok(/Math\.floor\((?:[A-Z_]*BASE|b) \* Math\.pow\(level \+ 1, 1\.5\)\)/.test(src), `${p.file} uses base x level^1.5 past the table`);
  }
});
