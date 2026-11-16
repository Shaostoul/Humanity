// The website's Skills tab weighs practice the way the game does (skills rung 2, 2026-11-15), run
// under Node:
//
//   node --test scripts/tests/skills-practice-web.test.js      (in `just rig-tests`)
//
// docs/design/skills.md section 3, points 3 and 4: the same task repeated loses value over a day
// and recovers; the Library guide for your stage and a teacher help. The game weighs each practice
// by data/skills/practice_quality.json (crates/humanity-systems/src/skills/quality.rs, its tests in
// quality_tests.rs). The website's Skills tab (web/pages/profile.html) is the real-life twin: each
// "Practised" press is one try at that skill, weighed by the same file's Simplified level, and the
// card says why it gave what it gave in the same words (data/i18n/en.json, skills_page.why_*),
// shows "How practice counts" from the same numbers, and links the Library guide for the stage
// reached (data/skills/guides.json).
//
// What it proves:
//  1. The page reads data/skills/practice_quality.json and data/skills/guides.json.
//  2. Its repeat curve is the file's, try for try: the first `full_attempts` count in full, each
//     after is `per_repeat` of the one before, never under `floor`, and a try a day old no longer
//     counts (the page's own skill_varietyFactor and skill_weighTry, run here).
//  3. Its words are the game's keys: why_first_time, why_full, why_repeat, how_variety,
//     guide_for_stage, and every key it names is in en.json.
//
// Seen red 2026-11-15 before the page had any of it: "1. ... profile.html reads
// data/skills/practice_quality.json" failed (the page fetched only skills.csv, families.json and
// stages.json).

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.join(__dirname, "..", "..");
const WEB = process.env.HOS_WEB_DIR || path.join(ROOT, "web");
const page = fs.readFileSync(path.join(WEB, "pages", "profile.html"), "utf8");
const quality = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "skills", "practice_quality.json"), "utf8"));
const en = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "i18n", "en.json"), "utf8"));

/** The page's function `name`, as source, evaluated on its own (it must use only its arguments). */
function pageFunction(name) {
  const start = page.indexOf(`function ${name}(`);
  assert.ok(start >= 0, `profile.html defines ${name}`);
  let depth = 0;
  let i = page.indexOf("{", start);
  for (; i < page.length; i++) {
    if (page[i] === "{") depth++;
    if (page[i] === "}" && --depth === 0) break;
  }
  return new Function(`return (${page.slice(start, i + 1)});`)();
}

test("1. the Skills tab reads data/skills/practice_quality.json and data/skills/guides.json", () => {
  assert.ok(page.includes("/data/skills/practice_quality.json"), "profile.html reads data/skills/practice_quality.json");
  assert.ok(page.includes("/data/skills/guides.json"), "profile.html reads data/skills/guides.json");
});

test("2. its repeat curve is the file's, try for try, and a day's rest restores it", () => {
  const factor = pageFunction("skill_varietyFactor");
  const weighTry = pageFunction("skill_weighTry");
  for (const level of ["simplified", "realistic"]) {
    const v = quality.levels[level].variety;
    for (let k = 1; k <= 12; k++) {
      const want = Math.max(v.floor, Math.pow(v.per_repeat, Math.max(0, k - Math.max(1, v.full_attempts))));
      assert.ok(Math.abs(factor(v, k) - want) < 1e-9, `${level} try ${k}: ${factor(v, k)} vs ${want}`);
    }
    // Eight tries one second apart, then one a day later.
    const s = {};
    const day = 86400000;
    const got = [];
    for (let k = 0; k < 8; k++) got.push(weighTry(v, s, 1000 * k));
    assert.strictEqual(got[0].first, Math.max(1, v.first_time), `${level}: the first try ever has the first-time bonus`);
    assert.strictEqual(got[1].first, 1, `${level}: only the first`);
    assert.strictEqual(got[7].attempt, 8, `${level}: the eighth try today is counted as the eighth`);
    assert.ok(got[7].variety < got[0].variety, `${level}: repeats are worth less (${got[7].variety} vs ${got[0].variety})`);
    const rested = weighTry(v, s, 8000 + v.rest_days * day);
    assert.strictEqual(rested.attempt, 1, `${level}: a day later it is the first try again`);
    assert.strictEqual(rested.variety, 1, `${level}: full value again after a day's rest`);
  }
});

test("3. its words are the game's keys", () => {
  for (const key of ["why_first_time", "why_full", "why_repeat", "how_variety", "how_title", "guide_for_stage", "guide_open", "web_logged"]) {
    assert.ok(page.includes(`skills_page.${key}`), `profile.html says skills_page.${key}`);
  }
  const named = [...page.matchAll(/'skills_page\.([a-z_]+)'/g)].map((m) => m[1]);
  const missing = [...new Set(named)].filter((k) => !(k in en.skills_page));
  assert.deepStrictEqual(missing, [], "every skills_page key the page names is in en.json");
});
