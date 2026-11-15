// Tests for scripts/check-library-quotes.js, the gate that refuses a verbatim
// quotation from a source the registry says to restate.
//
// Each case below was seen FAILING against the checker as it stood before
// 2026-10-04 (run with CHECKER pointing at a copy carrying the old regex
// pairing and the old first-restate-only-source attribution):
//   - the prose between two short quotes: reported as a Gildan quotation;
//   - "The CDC's wording: ..." after an NCHFP mention: blamed on NCHFP;
//   - a table row: the cell text after a short quote reported as a quotation.
"use strict";

const test = require("node:test");
const assert = require("node:assert");
const Q = require(process.env.CHECKER || "../check-library-quotes.js");

const REGISTRY = [
  { id: "nchfp", name: "National Center for Home Food Preservation", use: "facts", licence: "copyright-all-rights-reserved" },
  { id: "gildan", name: "Gildan", use: "facts", licence: "copyright-all-rights-reserved" },
  { id: "penn-state-extension", name: "Penn State Extension", use: "facts", licence: "copyright-university" },
  { id: "cdc", name: "Centers for Disease Control and Prevention", use: "verbatim", licence: "public-domain-us-gov" },
  { id: "usda", name: "United States Department of Agriculture", use: "verbatim", licence: "public-domain-us-gov" },
  { id: "wikipedia", name: "Wikipedia", use: "library", licence: "cc-by-sa" },
];
const needles = Q.buildNeedles(REGISTRY);
const check = (text) => Q.checkDocument("t.md", text, needles);

test("a real quotation from a restate-only source is still caught", () => {
  const p = check('Penn State Extension says "keep the jars in a cool dark place for a year" in its guide.');
  assert.strictEqual(p.length, 1);
  assert.strictEqual(p[0].id, "penn-state-extension");
});

test("the prose between two short quotes is not a quotation", () => {
  const text =
    'Gildan lists its "Heavy Cotton" T-shirt as 100 percent cotton in solid colours and 50 percent ' +
    'polyester in its heather colours and its "safety" colours.';
  assert.deepStrictEqual(check(text), []);
});

test("a quotation belongs to the source named nearest before it", () => {
  const text =
    "The National Center for Home Food Preservation tests every recipe. " +
    'The CDC\'s wording on the last point: "Do not eat food if you do not know whether it was safely canned."';
  assert.deepStrictEqual(check(text), [], "CDC is named nearer and its words may ship");
});

test("a restate-only source named nearer still wins over a quotable one", () => {
  const text =
    "The CDC agrees on the risk. " +
    'The National Center for Home Food Preservation puts it as "process every jar for the full time in the table".';
  const p = check(text);
  assert.strictEqual(p.length, 1);
  assert.strictEqual(p[0].id, "nchfp");
});

// The licence change (2026-11-14): the Library is CC BY-SA 4.0, so a share-alike source marked
// `use: library` may be quoted there, with its credit. Seen RED against the checker as it stood
// before (CHECKER=<copy of the 2026-11-13 file>): the quotation was blamed on Penn State Extension,
// named earlier, because a use: library source was not a name the checker knew.
test("a share-alike source the Library may copy (use: library) owns its quotation", () => {
  const text =
    "Penn State Extension covers the same ground. " +
    'Wikipedia\'s article on the subject puts it as "a seed bank stores seeds to preserve genetic diversity".';
  assert.deepStrictEqual(check(text), [], "Wikipedia is named nearer and its words may be copied into the Library");
});

test("a short alias matches only as a whole word", () => {
  // "epidemic" contains no word "cdc"; "abcdcd" must not match "cdc" either.
  const n = Q.nearestSource("the abcdcd team and penn state extension said ", needles);
  assert.strictEqual(n.id, "penn-state-extension");
});

test("a table row: cell text after a short quote is not a quotation", () => {
  const text =
    "| Kind | Gildan names |\n" +
    '| **Fake leather** | "vegan leather", including plant-based leathers, which are usually coated |\n' +
    '| **Shiny** | "metallic" trim and sequins, which melt onto the skin |';
  assert.deepStrictEqual(check(text), []);
});

test("quote-ok on the line before skips the quotation", () => {
  const text =
    "Penn State Extension's page title:\n" +
    "<!-- quote-ok: naming the page, not quoting its text -->\n" +
    '"keep the jars in a cool dark place for a year" is how it starts.';
  assert.deepStrictEqual(check(text), []);
});

test("an unpaired mark spoils only its own paragraph", () => {
  const text =
    'A stray " mark here.\n\n' +
    'Penn State Extension says "keep the jars in a cool dark place for a year" in its guide.';
  assert.strictEqual(check(text).length, 1);
});
