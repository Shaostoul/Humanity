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

// ── 1b. Lift out the FOOTNOTES, which are a different thing entirely ──
// BUG, fixed 2026-09-08. The line above strips only notes whose name is
// "annotation" (70 of them). The file also carries 18 notes marked
// type="footnote" with no name attribute, and those are the per-amendment
// ratification histories: "The 16th amendment ... was proposed to the
// legislatures ... Ratification was completed on February 3, 1913 ...".
// Nothing removed them, so the catch-all tag-stripper in text() below merged
// all 31,287 characters of them straight into the constitutional text with no
// separation. A reader could not tell where the Constitution stopped and the
// House Manual's editorial apparatus began, and it was 41 percent of the
// shipped file. The header meanwhile claimed annotations had been removed.
//
// They are not deleted, because when an amendment was ratified is genuinely
// useful. They are captured here, keyed by note id, and re-emitted below each
// amendment as a clearly labelled block quote that cannot be mistaken for
// constitutional text.
const footnotes = new Map();
for (const m of xml.matchAll(/<note\b(?![^>]*name="annotation")[^>]*\bid="([^"]+)"[^>]*>([\s\S]*?)<\/note>/g)) {
  footnotes.set(m[1], m[2]);
}
const footnotesLifted = footnotes.size;
xml = xml.replace(/<note\b(?![^>]*name="annotation")[^>]*>[\s\S]*?<\/note>/g, "");

/** Turn a fragment of USLM into plain text, preserving the words exactly. */
function text(fragment) {
  return fragment
    // <num> holds "ARTICLE I." / "AMENDMENT II.", which our own heading already
    // states. Left in, every entry read "### Amendment II" followed by
    // "AMENDMENT II. A well regulated...".
    .replace(/<num\b[^>]*>[\s\S]*?<\/num>/g, "")
    // Footnote reference markers ("<ref class="footnote">7</ref>") are Manual
    // apparatus. Drop the marker, not just its tags, or a bare "7" lands in the
    // middle of a sentence.
    .replace(/<ref\b[^>]*class="footnote"[^>]*>[\s\S]*?<\/ref>/g, "")
    .replace(/<\/?inline[^>]*>/g, "")   // small-caps and similar are styling only
    // Superscripts close up rather than separate: "G<sup>o</sup>" is "Go" and
    // "Presi<sup>dt</sup>." is "Presidt.". Going through the generic rule below
    // would space them into "G o" and "Presi dt".
    .replace(/<\/?sup[^>]*>/g, "")
    .replace(/<[^>]+>/g, " ")            // any remaining element
    // NOTE: em dashes are NOT rewritten. An earlier version replaced them with
    // ", " to satisfy the project's no-em-dash house style. That rule is about
    // copy WE write; applying it here silently altered the punctuation of a
    // primary source while the file's own header claimed nothing had been
    // altered. It was also not even uniform: entity-encoded dashes became
    // commas while four literal U+2014 characters survived, so the document
    // carried both treatments. The Constitution's punctuation is the
    // Constitution's. (The emdash lint only scans src/gui, so this is safe.)
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
  "The complete verbatim text: the Preamble, all seven Articles, the attestation and " +
    "signatures of the delegates, and all twenty-seven amendments. Spelling, punctuation " +
    "and capitalisation are exactly as in the official source, which is why you will see " +
    '"defence", "chusing" and capitalised nouns.'
);
out.push("");
out.push(
  "Source: Constitution of the United States, USLM XML from the House Rules and Manual, " +
    "United States Government Publishing Office, govinfo bulk data " +
    "(https://www.govinfo.gov/bulkdata/HMAN/117/HMAN-117.zip). A work of the US Government " +
    "and not copyrightable under 17 U.S.C. 105(a). The House parliamentary annotations have " +
    "been removed. The per-amendment ratification histories have been kept, but moved out of " +
    "the constitutional text into clearly marked block quotes, so that nothing editorial " +
    "reads as though the Constitution says it. No word of the text itself has been changed."
);
out.push("");
out.push(
  "What this text is NOT. It is the Constitution as amended, which is not the same thing " +
    "as the document the framers signed in 1787. Several passages below were deliberately " +
    "overturned by later amendments and are no longer operative: the three-fifths and " +
    "slave-importation clauses (Article I, Sections 2 and 9, ended by Amendments XIII and " +
    "XIV), the original method of electing a President (Article II, Section 1, replaced by " +
    "Amendment XII), and the election of Senators by state legislatures (Article I, " +
    "Section 3, replaced by Amendment XVII). They are printed here because they are part of " +
    "the document's text and erasing them would be its own kind of falsification. Read the " +
    "amendments to know what is actually in force."
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
out.push(
  "Two more places this edition differs from the parchment the delegates actually " +
    "signed. The attestation below reads \"of the Independence of the United States\"; " +
    "the engrossed original reads \"Independance\", which is how the framers spelled it " +
    "that day. And the signatures here are grouped New Hampshire first, running north " +
    "to south, while the original is laid out in two columns beginning with Delaware. " +
    "Neither is an error and neither changes a word of meaning, but if you came here " +
    "asking what the founders themselves wrote, those are the two places where the " +
    "answer is \"something very slightly different from this\". The engrossed text is at " +
    "https://www.archives.gov/founding-docs/constitution-transcript."
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

// ── 3b. The attestation and the signatures ──
// MISSING UNTIL 2026-09-08, and this was the worst of the defects. The
// generator read <preamble>, <article> and <level name="amendment"> and nothing
// else, so it never saw <level id="signatures"> at all. Article VII ended at
// "...between the States so ratifying the Same." and the file jumped straight to
// the Amendments. Everything the delegates actually signed was absent: the
// "done in Convention ... Seventeenth Day of September ... In Witness whereof We
// have hereunto subscribed our Names" attestation, George Washington's
// signature, and all 38 other deputies. None of the 12 verbatim spot checks
// looked for any of it, so the omission shipped silently while the header
// claimed "the complete verbatim text".
const sigLevel = /<level\b[^>]*id="signatures"[^>]*>([\s\S]*?)<\/level>/.exec(xml);
let signerCount = 0;
if (sigLevel) {
  const body = sigLevel[1];
  out.push("## Signatures");
  out.push("");

  // The attestation is the first paragraph, before any signature.
  const firstP = /<p\b[^>]*>([\s\S]*?)<\/p>/.exec(body);
  if (firstP) {
    out.push(text(firstP[1]));
    out.push("");
  }

  // Washington signs first and alone, as President of the Convention.
  const wash = /<signature>([\s\S]*?)<\/signature>/.exec(body);
  if (wash) {
    const nm = text((/<name>([\s\S]*?)<\/name>/.exec(wash[1]) || ["", ""])[1]);
    const role = text((/<role>([\s\S]*?)<\/role>/.exec(wash[1]) || ["", ""])[1]);
    if (nm) {
      signerCount++;
      out.push(`**${nm}** ${role}`.trim());
      out.push("");
    }
  }

  // Then the deputies, grouped by state, in document order. The layout is a
  // two-column table in the source; state is a <header>, signers are <row>s
  // beneath it, so names are accumulated per state and flushed on the next
  // header rather than emitted row by row.
  const sigs = /<signatures\b[^>]*>([\s\S]*?)<\/signatures>/.exec(body);
  if (sigs) {
    const manualNote = /<p\b[^>]*>([\s\S]*?)<\/p>/.exec(sigs[1]);
    if (manualNote) {
      const t = text(manualNote[1]);
      if (t) { out.push(t); out.push(""); }
    }
    let state = null;
    let names = [];
    let attest = null;
    const flush = () => {
      if (state && names.length) {
        signerCount += names.length;
        // Joined with a SPACE, not ", ". Each <name> already carries its own
        // trailing punctuation in the source ("John Langdon," but
        // "Nicholas Gilman."), so adding a separator produced "Langdon,,".
        out.push(`**${state}** ${names.join(" ")}`);
        out.push("");
      }
      names = [];
    };
    for (const m of sigs[1].matchAll(/<header\b[^>]*>([\s\S]*?)<\/header>|<row\b[^>]*>([\s\S]*?)<\/row>/g)) {
      if (m[1] !== undefined) {
        flush();
        state = text(m[1]).replace(/\.$/, "");
      } else if (m[2] !== undefined) {
        // The final row is not a delegate. It is the Secretary's attestation:
        // a plain "Attest:" column beside a signature carrying a <role>. Filed
        // under the last state header it would read as a fortieth Georgia
        // signer, which he was not: 39 delegates signed, Jackson attested.
        const role = /<role>([\s\S]*?)<\/role>/.exec(m[2]);
        const nm = /<name>([\s\S]*?)<\/name>/.exec(m[2]);
        if (role && nm) {
          attest = { name: text(nm[1]), role: text(role[1]) };
          continue;
        }
        for (const n of m[2].matchAll(/<name>([\s\S]*?)<\/name>/g)) {
          const t = text(n[1]);
          if (t) names.push(t);
        }
      }
    }
    flush();
    if (attest) {
      out.push(`Attest: **${attest.name}** ${attest.role}`.trim());
      out.push("");
    }
  }
  out.push("");
}

// ── 4. The twenty-seven Amendments ──
let notesEmitted = 0;
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

  // The amendment's ratification history, if it has one, as a labelled block
  // quote. It used to be concatenated into the sentence above with no marker at
  // all. The <num> carries the idref to its footnote, and <num> is stripped by
  // text(), so the id is read off the raw body before that happens.
  const ref = /idref="([^"]+)"/.exec(m[2]);
  const note = ref && footnotes.get(ref[1]);
  if (note) {
    const t = text(note).replace(/^\d+\s*/, "");
    if (t) {
      out.push("> **Ratification history.** This is editorial matter from the House");
      out.push("> Rules and Manual, not part of the Constitution.");
      out.push(">");
      out.push(`> ${t}`);
      out.push("");
      notesEmitted++;
    }
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
  // Added 2026-09-08 after an audit found the entire signature block missing.
  // The old 12 checks all probed passages the parser DID emit, so none of them
  // could fail on a whole section the parser never looked for. A check that
  // cannot fail on the defect class you care about is not a check.
  ["attestation", "Seventeenth Day of September in the Year of our Lord"],
  ["attestation witness", "subscribed our Names"],
  ["signature: Washington", "WASHINGTON"],
  ["signature: a deputy", "John Langdon"],
  ["signature: last state", "Georgia"],
];

// Structural assertions. The spot checks above prove the text that IS present
// is right; these prove nothing is absent or contaminated.
const STRUCTURE = [
  ["all 27 amendments emitted", () => amendCount === 27],
  ["all 7 articles emitted", () => articleCount === 7],
  ["signature block emitted", () => signerCount === 39],
  ["ratification notes lifted out", () => footnotesLifted === 18],
  ["ratification notes re-emitted", () => notesEmitted >= 17],
  // The contamination test: ratification history must appear ONLY inside a
  // block quote. If a line contains this phrasing and does not start with ">",
  // House editorial matter has leaked back into the constitutional text.
  ["no editorial text outside block quotes", () =>
    !md.split("\n").some(l => !l.trimStart().startsWith(">") &&
      /(was proposed to the (legislatures|conventions)|Ratification was completed)/.test(l))],
];
let failed = 0;
for (const [name, needle] of CHECKS) {
  const ok = md.includes(needle);
  if (!ok) failed++;
  console.log(ok ? "  ok   " : "  FAIL ", name);
}
for (const [name, fn] of STRUCTURE) {
  let ok = false;
  try { ok = !!fn(); } catch (e) { ok = false; }
  if (!ok) failed++;
  console.log(ok ? "  ok   " : "  FAIL ", name);
}
if (failed) {
  console.error(`\n${failed} check(s) FAILED. Not shipping a mangled constitution.`);
  process.exit(1);
}
console.log("\nall verbatim and structural checks passed");
console.log("signers                :", signerCount, "(expect 39 delegates; Jackson attests separately)");
console.log("footnotes lifted       :", footnotesLifted, "re-emitted as block quotes:", notesEmitted);
