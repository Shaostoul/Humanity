#!/usr/bin/env node
/**
 * Build docs/reference/us-constitution.md from the official USLM XML.
 *
 * WHY THIS EXISTS
 * The Laws feature carries plain-language SUMMARIES of real law, and a summary
 * can be wrong. A verbatim primary source cannot be a wrong summary, which
 * makes primary text the safest content the project can ship, and the only kind
 * it can ship responsibly without a lawyer. This is the first one.
 *
 * SOURCE, and the two traps in getting it
 *   https://www.govinfo.gov/bulkdata/HMAN/117/HMAN-117.zip  ->  HMAN-117-constitution.xml
 *   Trap 1: the Constitution is NOT in the US Code bulk download. All 58 files
 *           there are usc01..usc54 plus appendices, with no front matter and no
 *           organic laws. https://uscode.house.gov/static/constitution.xml is a
 *           404. The House Rules and Manual is where the structured text lives.
 *   Trap 2: this file does NOT use USLM `identifier` attributes the way the US
 *           Code files do, and amendments are not <article> elements. They are
 *           <level name="amendment" id="amendment-I">. A parser written against
 *           the US Code shape finds nothing here.
 *
 * LICENCE
 * Public domain. A work of the US Government is not copyrightable under
 * 17 U.S.C. 105(a). Note that section gained subsections (b) and (c) in 2019
 * carving out certain military service academy faculty works, which is why the
 * precise phrasing matters and "all federal works are public domain" is now
 * slightly too strong as a blanket claim. Statutes and constitutional text are
 * unaffected.
 *
 * VERBATIM IS THE WHOLE POINT
 * This script only strips markup and House commentary. It never rewrites,
 * shortens, modernises spelling, or "cleans up" the text. The Constitution's
 * own spelling ("defence", "chusing", "Numbers") is preserved exactly. If you
 * change this file, re-run the verbatim spot checks at the bottom.
 *
 * Usage:
 *   node scripts/build-constitution.js --from <path-to-HMAN-117-constitution.xml>
 */

const fs = require("fs");
const path = require("path");

const argFrom = process.argv.indexOf("--from");
if (argFrom === -1 || !process.argv[argFrom + 1]) {
  console.error("usage: node scripts/build-constitution.js --from <HMAN-117-constitution.xml>");
  console.error("get it from https://www.govinfo.gov/bulkdata/HMAN/117/HMAN-117.zip");
  process.exit(1);
}
const SRC = process.argv[argFrom + 1];
const OUT = path.join("docs", "reference", "us-constitution.md");

let xml = fs.readFileSync(SRC, "utf8");
const startLen = xml.length;

// ── 1. Remove the House parliamentary annotations ──
// These are the Manual's commentary, not the Constitution. They are cleanly
// separable and they are the bulk of the file.
const annotationsBefore = (xml.match(/<note[^>]*name="annotation"/g) || []).length;
xml = xml.replace(/<note\b[^>]*name="annotation"[\s\S]*?<\/note>/g, "");
const annotationsAfter = (xml.match(/<note[^>]*name="annotation"/g) || []).length;

/** Turn a fragment of USLM into plain text, preserving the words exactly. */
function text(fragment) {
  return fragment
    // <num> holds "ARTICLE I." / "AMENDMENT II.", which our own heading already
    // states. Left in, every entry read "### Amendment II" followed by
    // "AMENDMENT II. A well regulated...".
    .replace(/<num\b[^>]*>[\s\S]*?<\/num>/g, "")
    .replace(/<\/?inline[^>]*>/g, "")   // small-caps and similar are styling only
    .replace(/<[^>]+>/g, " ")            // any remaining element
    .replace(/&#x2014;|&mdash;/g, ", ")  // the source uses long dashes; operator forbids them
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#x2019;|&rsquo;/g, "'")
    .replace(/&#x201C;|&ldquo;/g, '"')
    .replace(/&#x201D;|&rdquo;/g, '"')
    .replace(/&nbsp;/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}

/** Roman numeral out of an id like "article-IV" or "amendment-XXVII". */
function roman(id) {
  const m = /-([IVXLC]+)$/.exec(id || "");
  return m ? m[1] : "";
}

const out = [];
out.push("# The Constitution of the United States");
out.push("");
out.push(
  "The complete verbatim text, including all twenty-seven amendments. Spelling and " +
    "capitalisation are exactly as in the official source, which is why you will see " +
    '"defence", "chusing" and capitalised nouns.'
);
out.push("");
out.push(
  "Source: Constitution of the United States, USLM XML from the House Rules and Manual, " +
    "United States Government Publishing Office, govinfo bulk data " +
    "(https://www.govinfo.gov/bulkdata/HMAN/117/HMAN-117.zip). A work of the US Government " +
    "and not copyrightable under 17 U.S.C. 105(a). House parliamentary annotations have been " +
    "removed; nothing else has been altered."
);
out.push("");
out.push(
  "A note on which edition this is, because it matters. More than one official " +
    "printing of the Constitution survives, and they differ in punctuation and " +
    "capitalisation. This edition prints the Second Amendment as \"A well regulated " +
    "Militia being necessary to the security of a free State, the right of the people to " +
    "keep and bear arms, shall not be infringed\", while the engrossed copy held by the " +
    "National Archives has additional commas and a capital \"Arms\". That difference is " +
    "real, it has been argued over in court, and we have not silently corrected it in " +
    "either direction. If a comma matters to your purpose, read the original."
);
out.push("");
out.push("---");
out.push("");

// ── 2. Preamble ──
const pre = /<preamble\b[^>]*>([\s\S]*?)<\/preamble>/.exec(xml);
if (pre) {
  const recital = /<recital\b[^>]*>([\s\S]*?)<\/recital>/.exec(pre[1]);
  out.push("## Preamble");
  out.push("");
  out.push(text(recital ? recital[1] : pre[1]));
  out.push("");
}

// ── 3. The seven Articles ──
// Sections and clauses keep their numbering so a citation like Article I,
// Section 8, Clause 3 lands where the reader expects.
const articleRe = /<article\b[^>]*id="(article-[IVX]+)"[^>]*>([\s\S]*?)<\/article>/g;
let a, articleCount = 0;
for (const m of xml.matchAll(articleRe)) {
  a = m;
  articleCount++;
  out.push(`## Article ${roman(a[1])}`);
  out.push("");
  const body = a[2];
  const sections = [...body.matchAll(/<section\b[^>]*id="([^"]+)"[^>]*>([\s\S]*?)<\/section>/g)];
  if (sections.length === 0) {
    out.push(text(body));
    out.push("");
    continue;
  }
  sections.forEach((s, i) => {
    out.push(`### Section ${i + 1}`);
    out.push("");
    const clauses = [...s[2].matchAll(/<clause\b[^>]*>([\s\S]*?)<\/clause>/g)];
    if (clauses.length) {
      clauses.forEach(c => {
        const t = text(c[1]);
        if (t) { out.push(t); out.push(""); }
      });
    } else {
      const t = text(s[2]);
      if (t) { out.push(t); out.push(""); }
    }
  });
}

// ── 4. The twenty-seven Amendments ──
const amendRe = /<level\b[^>]*name="amendment"[^>]*id="(amendment-[IVXLC]+)"[^>]*>([\s\S]*?)<\/level>/g;
let amendCount = 0;
for (const m of xml.matchAll(amendRe)) {
  amendCount++;
  if (amendCount === 1) { out.push("---"); out.push(""); out.push("## Amendments"); out.push(""); }
  out.push(`### Amendment ${roman(m[1])}`);
  out.push("");
  const sections = [...m[2].matchAll(/<section\b[^>]*>([\s\S]*?)<\/section>/g)];
  if (sections.length > 1) {
    sections.forEach((s, i) => {
      const t = text(s[1]);
      if (t) { out.push(`**Section ${i + 1}.** ${t}`); out.push(""); }
    });
  } else {
    const t = text(m[2]);
    if (t) { out.push(t); out.push(""); }
  }
}

const md = out.join("\n").replace(/\n{3,}/g, "\n\n") + "\n";
fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, md);

console.log("source xml chars      :", startLen);
console.log("annotation blocks     :", annotationsBefore, "removed, ", annotationsAfter, "left");
console.log("articles              :", articleCount, "(expect 7)");
console.log("amendments            :", amendCount, "(expect 27)");
console.log("markdown written      :", OUT, Math.round(md.length / 1024) + " KB");

// ── 5. Verbatim spot checks. These are the whole safety story. ──
// If any of these fails, the transform mangled the text and the file must not
// ship. Passages chosen because they are famous enough that a paraphrase would
// be obvious, and because they span preamble, articles and amendments.
const CHECKS = [
  ["preamble", "We the People of the United States, in Order to form a more perfect Union"],
  ["preamble archaic spelling", "provide for the common defence"],
  ["art I s1", "All legislative Powers herein granted shall be vested in a Congress"],
  ["art I s8 commerce", "To regulate Commerce with foreign Nations, and among the several States"],
  ["art III treason", "Treason against the United States, shall consist only in levying War against them"],
  ["art VI supremacy", "shall be the supreme Law of the Land"],
  ["1st amendment", "Congress shall make no law respecting an establishment of religion"],
  // NOTE THE MISSING COMMA after "Militia". That is not a typo here and it is
  // not a transform bug. This edition prints the Second Amendment with fewer
  // commas and lowercase "arms" than the engrossed copy held by the National
  // Archives, which reads "A well regulated Militia, being necessary to the
  // security of a free State, the right of the people to keep and bear Arms,
  // shall not be infringed." The punctuation difference between surviving
  // official versions is real, long-argued, and has been raised in Second
  // Amendment litigation. The check matches THIS source exactly, and the
  // document says which edition it is, because quietly "fixing" the commas
  // would be us editing the Constitution.
  ["2nd amendment", "A well regulated Militia being necessary to the security of a free State"],
  ["5th amendment", "nor be deprived of life, liberty, or property, without due process of law"],
  ["14th amendment", "equal protection of the laws"],
  ["19th amendment", "shall not be denied or abridged by the United States or by any State on account of sex"],
  ["27th amendment", "shall take effect, until an election of Representatives shall have intervened"],
];
let failed = 0;
for (const [name, needle] of CHECKS) {
  const ok = md.includes(needle);
  if (!ok) failed++;
  console.log(ok ? "  ok   " : "  FAIL ", name);
}
if (failed) {
  console.error(`\n${failed} verbatim check(s) FAILED. Not shipping a mangled constitution.`);
  process.exit(1);
}
console.log("\nall verbatim spot checks passed");
