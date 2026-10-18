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
    const t = risk.fileType(c.name, hex(c.head));
    assert.deepEqual([t.id, t.level], [c.type, c.level], `${JSON.stringify(c.name)} over ${c.head}: its badge`);
    assert.equal(t.disguised, c.disguised, `${JSON.stringify(c.name)} over ${c.head}: the badge says disguised`);
  }
});

// A name cannot hide its ending (2026-10-17): every shared plain_names pair, the desktop's too
// (src/net/file_risk.rs plain_name); the shared cases above hold a program whose ".exe" is
// followed by a direction mark. Seen red 2026-10-17 with assess and fileType judging the name as it
// came: the shared case for "evil.exe" followed by U+200F got no word (null, not "program").
test("a name cannot hide its ending behind direction marks", () => {
  const pairs = JSON.parse(fs.readFileSync(path.join(repo, "scripts/tests/fixtures/file-risk-cases.json"), "utf8")).plain_names;
  assert.ok(pairs.length >= 5);
  for (const [given, shown] of pairs) assert.equal(risk.plainName(given), shown, JSON.stringify(given));
});

// The badge (2026-10-17): every file shows its type as a drawing, a word and a colour, in the
// desktop app's shape (gui/widgets/file_badge.rs). Seen red 2026-10-17 with the label left unescaped:
// "a name cannot break out of the badge"; and with the first bytes left off the badge (so a redraw
// turned a disguised file's red badge back to its claimed type): "a badge drawn from the bytes keeps
// them, so a redraw stays red".
test("every file gets a badge: drawing, word, ending and colour, all escaped", () => {
  risk.use(list);
  const zip = risk.badgeHtml("maps.zip", null);
  assert.match(zip, /^<span class="file-badge level-care" data-file-badge="maps.zip" data-file-type="archive"/);
  assert.ok(zip.includes('<svg class="file-badge-icon" viewBox="0 0 48 48"') && zip.includes("<rect "), "its drawing, from the list");
  assert.ok(zip.includes('<span class="file-badge-label">Archive</span><span class="file-badge-ending">.zip</span>'));
  const fake = risk.badgeHtml("holiday.png", hex("4d5a9000"));
  assert.ok(fake.startsWith('<span class="file-badge level-danger disguised"') && fake.includes(">Program<") && fake.includes(">.png<"), "a disguised file shows what it is, red: " + fake);
  assert.ok(fake.includes('data-file-head="4d5a9000"'), "a badge drawn from the bytes keeps them, so a redraw stays red");
  const odd = risk.badgeHtml('"><img src=x onerror=alert(1)>.txt', null);
  assert.ok(!odd.includes("<img") && odd.includes("&quot;&gt;&lt;img"), "a name cannot break out of the badge");
  assert.equal(risk.fileType("mystery.dat", null).label, "File");
  risk.use(null);
  assert.deepEqual(risk.fileType("setup.exe", null), { id: "unknown", label: "File", icon: "unknown", level: "unknown", ending: ".exe", disguised: false }, "before the list is in: a plain grey File");
  risk.use(list);
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
