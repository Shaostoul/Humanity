// The website's Dev > Claude (web/pages/dev-claude.js), the web twin of the app's Dev > Claude
// (src/gui/pages/dev_agents/). Pure node, no browser: runs in `just rig-tests`.
//
// Run:  just rig-tests
// Or:   node --test scripts/tests/dev-claude-web.test.js
//
// What it holds: the web's journal tail and TIER 0 headings agree with the app's readers on the
// same fixtures as src/gui/pages/dev_agents/tests.rs (newest decision first, one line each; the
// headings stop at the next tier), the "ago" wording uses the app's thresholds, every word the
// page shows is a dev_claude key in data/i18n/en.json, and the real journal and priorities in
// this checkout read the way the page will read them.
//
// Seen red 2026-10-22 with the reversal taken out of journalTail: "the journal tail is the newest
// three, newest first ... AssertionError [ERR_ASSERTION]: newest first" (tests 5, pass 4, fail 1).

const test = require("node:test");
const assert = require("node:assert");
const fs = require("fs");
const path = require("path");

const ROOT = path.join(__dirname, "..", "..");
const DevClaude = require("../../web/pages/dev-claude.js");
const en = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "i18n", "en.json"), "utf8")).dev_claude;
const T = (key, vars) => DevClaude.fill(en[key], vars);

test("the journal tail is the newest three, newest first, one line each (the app's fixture)", () => {
  const json = {
    recent_decisions: [
      { date: "2026-10-18", decision: "oldest" },
      { date: "2026-10-19", decision: "older" },
      { date: "2026-10-20", decision: "third newest" },
      { at: "2026-10-21T08:00:00Z", decision: "second\n  newest,\twith   breaks" },
      { date: "2026-10-22", decision: "the newest", why: "because" },
    ],
  };
  const tail = DevClaude.journalTail(JSON.stringify(json), 3);
  assert.strictEqual(tail.length, 3);
  assert.strictEqual(tail[0].date, "2026-10-22", "newest first");
  assert.strictEqual(tail[0].text, "the newest");
  assert.strictEqual(tail[1].date, "2026-10-21T08:00:00Z", "an older entry's `at` dates it");
  assert.strictEqual(tail[1].text, "second newest, with breaks", "one line each");
  assert.strictEqual(tail[2].text, "third newest");
  assert.throws(() => DevClaude.journalTail("{}", 3), /recent_decisions/, "no list is said, not shown as an empty journal");
});

test("TIER 0's headings stop at the next tier (the app's fixture)", () => {
  const md =
    "# HumanityOS: Priorities\n\n## How to read this\n### Not this one\n\n## TIER 0: the next thing to work on\n" +
    "Some prose.\n#3) not a heading\n### DONE 2026-10-21/22: the five pieces\ntext\n### THIS WEEK: playable multiplayer\n\n" +
    "## TIER 1: hardening\n### Hardening\n";
  assert.deepStrictEqual(DevClaude.tier0Headings(md, 6), ["DONE 2026-10-21/22: the five pieces", "THIS WEEK: playable multiplayer"]);
  assert.strictEqual(DevClaude.tier0Headings(md, 1).length, 1, "at most max");
  assert.deepStrictEqual(DevClaude.tier0Headings(md.replace(/\n/g, "\r\n"), 6).length, 2, "Windows line endings");
});

test("ago speaks with the app's thresholds", () => {
  const now = Date.UTC(2026, 9, 22, 12, 0, 0);
  assert.strictEqual(DevClaude.ago(now - 2000, now, T), "just now");
  assert.strictEqual(DevClaude.ago(now - 40000, now, T), "40 s ago");
  assert.strictEqual(DevClaude.ago(now - 600000, now, T), "10 min ago");
  assert.strictEqual(DevClaude.ago(now - 3 * 3600000, now, T), "3 h ago");
  assert.strictEqual(DevClaude.ago(now - 3 * 86400000, now, T), "3 days ago");
});

test("every word the page shows is a dev_claude key in en.json", () => {
  const src = fs.readFileSync(path.join(ROOT, "web", "pages", "dev-claude.js"), "utf8");
  const keys = [...src.matchAll(/\bT\('([a-z0-9_]+)'/g)].map((m) => m[1]);
  assert.ok(keys.length >= 15, `the page's keys were found: ${keys}`);
  for (const k of keys) {
    assert.strictEqual(typeof en[k], "string", `en.json has dev_claude.${k}`);
    assert.ok(!en[k].includes("—"), `dev_claude.${k} has no em dash`);
  }
});

test("this checkout's own journal and priorities read the way the page reads them", () => {
  const journal = fs.readFileSync(path.join(ROOT, "data", "coordination", "orchestrator_state.json"), "utf8");
  const tail = DevClaude.journalTail(journal, 3);
  assert.strictEqual(tail.length, 3);
  for (const d of tail) assert.ok(d.date !== "?" && d.text.length > 0 && !d.text.includes("\n"), JSON.stringify(d));
  const md = fs.readFileSync(path.join(ROOT, "docs", "PRIORITIES.md"), "utf8");
  assert.ok(DevClaude.tier0Headings(md, 6).length >= 1, "TIER 0 has headings");
});
