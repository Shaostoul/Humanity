// Help outside this server, in the web chat's Report dialog (2026-10-10,
// docs/design/blocking-and-safe-mode.md 10e-ii), the pure part: which reasons offer the block,
// what it shows for a country, and which country it starts on. The words and rules live in
// web/shared/report.js; the dialog that draws them (web/chat/chat-reports.js) is tested in
// scripts/tests/report-web.test.js, over the same real data file.
//
// Run: node --test scripts/tests/outside-help-web.test.js   (in `just rig-tests`)
//
// The data is always the shipped file, data/safety/outside_help.json, so a change to it that the
// dialog cannot read fails here instead of on someone's screen. HOS_WEB_DIR points the test at
// another copy of web/ (used to see each test red).
//
// What it proves (10e-ii's Proof list, the web items):
//  1. The block is offered for exactly the two danger reasons, of all the reasons in the file.
//  2. Every listed country shows its own emergency number, each of its `also` lines and its child
//     report body as a link (https only), with the countries by name in the file's order, then
//     "Another country". The note shows when the entry has one.
//  3. A listed country whose `child_report` is null falls back to the default's INHOPE link, with
//     "Find the hotline for your country".
//  4. "Another country" shows the default's `emergency_text` sentence instead of a number.
//  5. The first-country rule: the saved choice wins; with none, the language tag's region when it
//     is listed (en-GB gives GB); en alone, an unlisted region (en-ZZ) or a numeric one (es-419)
//     give "Another country"; a saved value that is no longer listed falls through.
//  6. The date line reads the file's `researched` date, and report.js never types it.
//  7. The words on screen are 10e-ii's, read out of the spec.
//
// Red first, 2026-10-10: each mutation made in a fresh copy of web/, this test run against it with
// HOS_WEB_DIR, and seen failing with the assertion named (each passing again on the real web/):
//  1: OUTSIDE_HELP_REASONS with 'threats' added: "offered for threats? no".
//  2: outsideHelpView giving `also: []` for a listed country: "Ireland's also lines".
//  3: outsideHelpView's fallback branch taken out (no child line for a null child_report): "a null
//     child_report falls back to the INHOPE link".
//  4: outsideHelpView giving `emergencyText: ''` for "Another country": "Another country gives
//     the default's sentence".
//  5: outsideHelpFirstCountry asking the language before the saved choice: "the saved choice
//     wins over the language"; and returning any saved string unchecked: "a saved value no longer
//     listed falls through to the language".
//  6: outsideHelpDateLine with the date typed in ('2026-10-09'): "the date line reads the file's
//     date, not one typed into the code".

const test = require("node:test");
const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");

const WEB = process.env.HOS_WEB_DIR || path.join(__dirname, "..", "..", "web");
const ROOT = path.join(__dirname, "..", "..");
const REPORT_JS = path.join(WEB, "shared", "report.js");
const report = require(REPORT_JS);

const FILE = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "outside_help.json"), "utf8"));
const REASONS = JSON.parse(fs.readFileSync(path.join(ROOT, "data", "safety", "report_reasons.json"), "utf8"));
const SPEC = fs.readFileSync(path.join(ROOT, "docs", "design", "blocking-and-safe-mode.md"), "utf8");
// 10e-ii with its line wrapping undone (a sentence may wrap in the source).
const TEN_E_II = SPEC.slice(SPEC.indexOf("### 10e-ii."), SPEC.indexOf("## 10f.")).replace(/\s+/g, " ");

const help = () => report.outsideHelpFrom(JSON.parse(JSON.stringify(FILE)));
const DANGER = ["child_danger", "someone_in_danger"];

test("the file reads, and the block is offered for exactly the two danger reasons", () => {
  const h = help();
  assert.ok(h, "data/safety/outside_help.json reads as 10e-ii's file");
  assert.equal(h.countries.length, FILE.countries.length, "every entry in the file is whole and kept");
  assert.ok(/when the chosen reason is `child_danger` or `someone_in_danger`/.test(TEN_E_II), "10e-ii names the two reasons");
  assert.ok(/Never for other reasons\./.test(TEN_E_II));
  const ids = report.reportReasonsFrom(REASONS).map((r) => r.id);
  assert.ok(ids.length >= 10 && DANGER.every((id) => ids.includes(id)), "the reasons file holds both");
  for (const id of ids) {
    const want = DANGER.includes(id);
    assert.equal(report.outsideHelpShows(id), want, `offered for ${id}? ${want ? "yes" : "no"}`);
    assert.equal(report.outsideHelpView(h, id, "GB") !== null, want, `offered for ${id}? ${want ? "yes" : "no"}`);
  }
  assert.equal(report.outsideHelpView(null, "child_danger", "GB"), null, "nothing while the file is not loaded");
});

test("a listed country shows its number, its also lines and its child report body", () => {
  const h = help();
  const names = FILE.countries.map((c) => c.name).concat(["Another country"]);
  for (const c of FILE.countries) {
    const v = report.outsideHelpView(h, "child_danger", c.code);
    assert.equal(v.code, c.code);
    assert.equal(v.name, c.name);
    assert.deepEqual(v.choices.map((x) => x.name), names, "the countries by name in the file's order, then Another country");
    assert.equal(v.choices[v.choices.length - 1].code, report.OUTSIDE_HELP_OTHER);
    assert.equal(v.emergency, c.emergency, `${c.name}'s emergency number`);
    assert.equal(v.emergencyText, "", `${c.name} shows a number, not the sentence`);
    assert.deepEqual(v.also, (c.also || []).map((a) => ({ number: a.number, for: a.for })), `${c.name}'s also lines`);
    if (c.child_report) {
      assert.deepEqual({ name: v.child.name, url: v.child.url }, c.child_report, `${c.name}'s child report body`);
      assert.equal(v.child.fallback, false);
      assert.ok(v.child.url.startsWith("https://"), "links are https");
    }
    assert.equal(v.note, c.note || "", `${c.name}'s note`);
    assert.equal(v.childSmall, false, "for a child in danger the child line is the main one");
  }
  // Ireland and Brazil: one and three also lines, each with what it is for.
  const ie = report.outsideHelpView(h, "child_danger", "IE");
  assert.deepEqual(ie.also, [{ number: "999", for: "emergency (either number works)" }], "Ireland's also lines");
  assert.equal(report.outsideHelpView(h, "child_danger", "BR").also.length, 3, "Brazil's also lines");
  // The one entry with a note says it.
  assert.ok(report.outsideHelpView(h, "someone_in_danger", "NG").note.startsWith("112 reaches the emergency centre"), "Nigeria's note");
  // For someone in danger the child line is there too, smaller.
  const sid = report.outsideHelpView(h, "someone_in_danger", "US");
  assert.equal(sid.emergency, "911");
  assert.deepEqual({ name: sid.child.name, url: sid.child.url }, { name: "NCMEC CyberTipline", url: "https://report.cybertip.org/" });
  assert.equal(sid.childSmall, true, "for someone in danger the child line is shown smaller");
});

test("a null child_report falls back to the INHOPE link", () => {
  const data = JSON.parse(JSON.stringify(FILE));
  data.countries.find((c) => c.code === "GB").child_report = null;
  // A link that is not https is not shown either: it falls back the same way.
  data.countries.find((c) => c.code === "DE").child_report = { name: "Somewhere", url: "javascript:alert(1)" };
  const h = report.outsideHelpFrom(data);
  for (const code of ["GB", "DE"]) {
    const v = report.outsideHelpView(h, "child_danger", code);
    assert.ok(v.child, "a null child_report falls back to the INHOPE link");
    assert.equal(v.child.url, FILE.default.child_report.url, "a null child_report falls back to the INHOPE link");
    assert.equal(v.child.name, "Find the hotline for your country");
    assert.equal(v.child.fallback, true);
    assert.equal(v.emergency, data.countries.find((c) => c.code === code).emergency, "the country's own number still shows");
  }
});

test("Another country gives the default's sentence instead of a number", () => {
  const h = help();
  for (const code of [report.OUTSIDE_HELP_OTHER, "ZZ", null]) {
    const v = report.outsideHelpView(h, "child_danger", code);
    assert.equal(v.code, report.OUTSIDE_HELP_OTHER, "anything not listed reads as Another country");
    assert.equal(v.name, "Another country");
    assert.equal(v.emergency, null);
    assert.equal(v.emergencyText, FILE.default.emergency_text, "Another country gives the default's sentence");
    assert.equal(v.emergencyText, "Call your local emergency number.");
    assert.deepEqual(v.also, []);
    assert.deepEqual({ name: v.child.name, url: v.child.url }, FILE.default.child_report, "and the INHOPE directory");
  }
});

test("which country first: the saved choice, then the language's region, then Another country", () => {
  const h = help();
  const first = (saved, lang) => report.outsideHelpFirstCountry(h, saved, lang);
  const OTHER = report.OUTSIDE_HELP_OTHER;
  // Nothing saved: the region of the language tag when it is listed.
  assert.equal(first(null, "en-GB"), "GB", "en-GB gives GB");
  assert.equal(first(null, "pt-BR"), "BR");
  assert.equal(first(null, "en_gb"), "GB", "written with an underscore or in lower case");
  assert.equal(first(null, "zh-Hant-TW"), OTHER, "a script before the region is skipped (TW is not listed)");
  assert.equal(first(null, "sr-Latn-DE"), "DE", "a script before the region is skipped");
  assert.equal(first(null, "en"), OTHER, "en alone gives Another country");
  assert.equal(first(null, "en-ZZ"), OTHER, "an unlisted region gives Another country");
  assert.equal(first(null, "es-419"), OTHER, "a numeric region names no one country");
  assert.equal(first(null, null), OTHER, "no language at all");
  assert.equal(first(null, ""), OTHER);
  // The saved choice wins.
  assert.equal(first("DE", "en-GB"), "DE", "the saved choice wins over the language");
  assert.equal(first(OTHER, "en-GB"), OTHER, "Another country, once picked, is kept too");
  // A saved value that is no longer listed falls through.
  assert.equal(first("TW", "en-GB"), "GB", "a saved value no longer listed falls through to the language");
  assert.equal(first("TW", "en"), OTHER, "and then to Another country");
  assert.equal(first("", "en-GB"), "GB");
  // The spec's order, in its words.
  assert.ok(/The last one this person picked on this device/.test(TEN_E_II));
  assert.ok(/`en-GB` gives GB/.test(TEN_E_II));
  assert.ok(/\*\*Never look up the person's location\*\*/.test(TEN_E_II));
});

test("the date line reads the file's date", () => {
  const v = report.outsideHelpView(help(), "someone_in_danger", "GB");
  assert.equal(v.dateLine, `Numbers checked on ${FILE.researched}. If one is wrong, tell us.`);
  // Another date in the file, another date on screen.
  const data = JSON.parse(JSON.stringify(FILE));
  data.researched = "2031-02-03";
  const later = report.outsideHelpView(report.outsideHelpFrom(data), "child_danger", report.OUTSIDE_HELP_OTHER);
  assert.equal(later.dateLine, "Numbers checked on 2031-02-03. If one is wrong, tell us.", "the date line reads the file's date, not one typed into the code");
  // And report.js never types the date it was checked (its comments may name the day a step
  // shipped, which can be the same day, so comment lines are left out).
  const code = fs.readFileSync(REPORT_JS, "utf8").split(/\r?\n/).filter((l) => !/^\s*(\/\/|\*|\/\*)/.test(l)).join("\n");
  assert.ok(!code.includes(FILE.researched), "the date line reads the file's date, not one typed into the code");
});

test("the words on screen are 10e-ii's", () => {
  for (const words of [
    `"${report.OUTSIDE_HELP_TITLE}"`,
    `"${report.OUTSIDE_HELP_OTHER_LABEL}"`,
    `"${report.OUTSIDE_HELP_FIND_HOTLINE}"`,
    `"${report.outsideHelpDateLine("<researched>")}"`,
    '"Emergency:"',
    '"Country: <name>"',
  ]) {
    assert.ok(TEN_E_II.includes(words), `10e-ii says ${words}`);
  }
  assert.equal(report.OUTSIDE_HELP_URL, "/data/safety/outside_help.json");
  assert.ok(TEN_E_II.includes("web from `/data/safety/outside_help.json`"));
});
