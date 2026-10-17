// What a received file is, said before it is opened, on the website (web/shared/file-risk.js),
// held to the cases the desktop app's twin answers too (src/net/file_risk.rs):
// scripts/tests/fixtures/file-risk-cases.json, against data/safety/file_risks.json.
// Run: node scripts/tests/file-risk-web.test.js
//
// Seen red 2026-10-17 with the disguise check left out: "holiday.png" over a program's bytes got
// no word.

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const repo = path.resolve(__dirname, "..", "..");
const risk = require(path.join(repo, "web/shared/file-risk.js"));
const list = JSON.parse(fs.readFileSync(path.join(repo, "data/safety/file_risks.json"), "utf8"));
const cases = JSON.parse(fs.readFileSync(path.join(repo, "scripts/tests/fixtures/file-risk-cases.json"), "utf8")).cases;
const hex = (s) => Uint8Array.from((s.match(/../g) || []).map((b) => parseInt(b, 16)));

test("every shared case gets the same word as in the desktop app", () => {
  risk.use(list);
  assert.ok(cases.length >= 15);
  for (const c of cases) {
    const got = risk.assess(c.name, hex(c.head));
    assert.equal(got ? got.kind : null, c.kind, `${JSON.stringify(c.name)} over ${c.head}`);
    assert.equal(!!(got && got.disguised), c.disguised, `${JSON.stringify(c.name)} over ${c.head}: disguised`);
    if (c.words) assert.equal(got && got.words, c.words, `${JSON.stringify(c.name)}: the words`);
  }
});

test("before the list is in there is no word, and the list loads once", async () => {
  risk.use(null);
  assert.equal(risk.assess("setup.exe", null), null, "nothing known yet: no word");
  let asked = 0;
  const fetchFn = async (url) => { asked++; assert.equal(url, "/data/safety/file_risks.json"); return { ok: true, json: async () => list }; };
  await Promise.all([risk.load(fetchFn), risk.load(fetchFn)]);
  await risk.load(fetchFn);
  assert.equal(asked, 1, "fetched once");
  assert.ok(risk.ready());
  assert.equal(risk.assess("setup.exe", null).kind, "program");
});
