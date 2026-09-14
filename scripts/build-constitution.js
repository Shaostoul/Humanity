#!/usr/bin/env node
/**
 * Build docs/reference/us-constitution.md from the National Archives
 * transcriptions of the engrossed parchment and the amendments.
 *
 * WHY THIS EXISTS
 * The Laws feature carries plain-language SUMMARIES of real law, and a summary
 * can be wrong. A verbatim primary source cannot be a wrong summary, which
 * makes primary text the safest content the project can ship, and the only kind
 * it can ship responsibly without a lawyer. This is the first one.
 *
 * WHY THE SOURCE CHANGED (2026-09-14)
 * Until v0.1306 this was built from the USLM XML in the House Rules and Manual
 * (govinfo HMAN-117). An audit of the shipped file found that source is the
 * wrong kind of document, and not by a small margin. The Manual is a working
 * procedural text for the House in which the Constitution is subordinate to its
 * annotations, and it had corrupted ours in four separate ways:
 *
 *   1. WRONG WORDS. Three verified against the National Archives:
 *        Amendment XII  "and the number of votes"      -> "and OF the number of votes"
 *        Amendment XII  "in presence of the Senate"    -> "in THE presence of the Senate"
 *        Amendment XVIII "all territorIES subject to"  -> "all territorY subject to"
 *      All three shipped.
 *   2. MANUFACTURED ELISIONS. The Manual splits single constitutional sentences
 *      across annotated clauses and marks the seam with <inline name="three-star">.
 *      Joining the clauses produced 14 runs of "* * * * * *" INSIDE complete
 *      sentences, telling the reader text had been omitted where none was.
 *   3. INVENTED WORDS. To make each annotated fragment read standalone the
 *      Manual inserts a bracketed subject: "[the House of Representatives]",
 *      "[Each House may]", "[the Senators and Representatives]". Five of these
 *      were printed as though constitutional, using the same square brackets the
 *      file used for genuinely superseded text, with no key to tell them apart.
 *   4. A DROPPED GRANT. Article I Section 8's opening words, "The Congress shall
 *      have Power", live in a <chapeau> element. The parser emitted only
 *      <clause> children, so the entire enumerated-powers grant shipped as 18
 *      orphan infinitives with no grantee.
 *
 * None of that is fixable by patching the transform, because the defects are in
 * the source's editorial design. So the source changed.
 *
 * THE SOURCE NOW
 *   https://www.archives.gov/founding-docs/constitution-transcript
 *   https://www.archives.gov/founding-docs/bill-of-rights-transcript
 *   https://www.archives.gov/founding-docs/amendments-11-27
 *
 * The first is NARA's transcription of the parchment Jacob Shallus inscribed,
 * the document in the Rotunda, and the page states that its spelling and
 * punctuation reflect the original. That is what makes it the right answer to
 * "is this what the founders actually wrote": it keeps "Independance",
 * "defence", "chuse", "In witness whereof", and the delegates' own signatures.
 * It carries no annotation apparatus at all, so there is nothing to strip and
 * nothing to mistake for constitutional text.
 *
 * It also marks every superseded passage with a link to the amendment that
 * replaced it, which is how this build knows to label them instead of leaving
 * the reader to guess.
 *
 * LICENCE
 * The text is public domain: a work of the US Government is not copyrightable
 * under 17 U.S.C. 105(a), and the 1787 text predates copyright entirely.
 *
 * VERBATIM IS THE WHOLE POINT
 * This script only turns markup into markdown. It never rewrites, shortens,
 * modernises spelling, or "cleans up" the text. If you change it, the checks at
 * the bottom must still pass, and they are written to fail on OMISSION as well
 * as on corruption, which is the failure the previous generation of checks
 * could not catch.
 *
 * Usage:
 *   node scripts/build-constitution.js                 # uses scripts/sources/nara/
 *   node scripts/build-constitution.js --fetch         # re-download, then build
 */

const fs = require("fs");
const path = require("path");
const https = require("https");

const SRC_DIR = path.join("scripts", "sources", "nara");
const OUT = path.join("docs", "reference", "us-constitution.md");

const PAGES = {
  body: {
    url: "https://www.archives.gov/founding-docs/constitution-transcript",
    file: "constitution-transcript.html",
  },
  bor: {
    url: "https://www.archives.gov/founding-docs/bill-of-rights-transcript",
    file: "bill-of-rights-transcript.html",
  },
  rest: {
    url: "https://www.archives.gov/founding-docs/amendments-11-27",
    file: "amendments-11-27.html",
  },
};

// ── fetching (opt-in; the committed copies are the reproducible default) ──
function get(url) {
  return new Promise((resolve, reject) => {
    https
      .get(url, { headers: { "user-agent": "HumanityOS-constitution-build" } }, res => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          return resolve(get(new URL(res.headers.location, url).toString()));
        }
        if (res.statusCode !== 200) return reject(new Error(url + " -> HTTP " + res.statusCode));
        let b = "";
        res.setEncoding("utf8");
        res.on("data", c => (b += c));
        res.on("end", () => resolve(b));
      })
      .on("error", reject);
  });
}

async function main() {
  if (process.argv.includes("--fetch")) {
    fs.mkdirSync(SRC_DIR, { recursive: true });
    for (const k of Object.keys(PAGES)) {
      const html = await get(PAGES[k].url);
      fs.writeFileSync(path.join(SRC_DIR, PAGES[k].file), html);
      console.log("fetched " + PAGES[k].file + " (" + html.length + " chars)");
    }
  }

  const html = {};
  for (const k of Object.keys(PAGES)) {
    const p = path.join(SRC_DIR, PAGES[k].file);
    if (!fs.existsSync(p)) {
      console.error("missing source: " + p);
      console.error("run: node scripts/build-constitution.js --fetch");
      process.exit(1);
    }
    html[k] = fs.readFileSync(p, "utf8");
  }
  build(html);
}

// ── text helpers ──

/** Decode the HTML entities these pages actually use. */
function decode(s) {
  return s
    .replace(/&nbsp;/g, " ")
    .replace(/&#160;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;/g, "'")
    .replace(/&rsquo;|&#8217;/g, "’")
    .replace(/&lsquo;|&#8216;/g, "‘")
    .replace(/&ldquo;|&#8220;/g, "“")
    .replace(/&rdquo;|&#8221;/g, "”")
    .replace(/&deg;|&#176;/g, "°")
    .replace(/&mdash;|&#8212;/g, "—")
    .replace(/&ndash;|&#8211;/g, "–")
    .replace(/&hellip;|&#8230;/g, "…");
}

/** Markup to plain text, preserving the words exactly. */
function textOf(frag) {
  return decode(frag.replace(/<[^>]+>/g, " "))
    .replace(/\s+/g, " ")
    .trim();
}

/** Strip the trailing period NARA puts on "Article. I." / "Section. 2.". */
function romanOf(s) {
  const m = /(?:Article|Amendment)\.?\s*([IVXLC]+)\.?/i.exec(s);
  return m ? m[1].toUpperCase() : "";
}

function build(html) {
  const out = [];
  let signerCount = 0;
  let supersededCount = 0;
  let articleCount = 0;
  let amendmentCount = 0;

  out.push("# The Constitution of the United States");
  out.push("");
  out.push(
    "The complete text: the Preamble, all seven Articles, the attestation and the " +
      "signatures of all thirty-nine delegates, and all twenty-seven amendments."
  );
  out.push("");
  out.push(
    "This is the parchment. The main body below is the National Archives " +
      "transcription of the document Jacob Shallus inscribed and the delegates signed " +
      "on 17 September 1787, the one on display in the Rotunda, and its spelling and " +
      "punctuation are the original's. That is why you will read \"defence\", \"chuse\", " +
      "\"Independance\" and \"In witness whereof\", and why the capitalisation wanders: " +
      "it wandered on the parchment."
  );
  out.push("");
  out.push(
    "Sources: https://www.archives.gov/founding-docs/constitution-transcript, " +
      "https://www.archives.gov/founding-docs/bill-of-rights-transcript and " +
      "https://www.archives.gov/founding-docs/amendments-11-27, transcribed by the " +
      "National Archives and Records Administration, which holds the original. Public " +
      "domain. Nothing here has been reworded, reordered, modernised or abridged; the " +
      "build only turns the Archives' markup into headings and paragraphs."
  );
  out.push("");
  out.push(
    "What this text is NOT. It is the document as written plus every amendment since, " +
      "which is not the same as the law in force today. Passages that later amendments " +
      "overturned are still printed, because they are part of the document and deleting " +
      "them would be its own falsification, but each one is labelled with the amendment " +
      "that replaced it. The three-fifths clause, the fugitive-slave clause, the original " +
      "method of electing a President and the election of Senators by state legislatures " +
      "all appear below and are all dead letters. Read the amendments to know what stands."
  );
  out.push("");
  out.push("---");
  out.push("");

  // ── Main body: Preamble, Articles I-VII, attestation, signatures ──
  {
    const h = html.body;
    const start = h.indexOf("We the People");
    if (start < 0) throw new Error("preamble not found in constitution-transcript");
    // The signature block ends the document; everything after the last state
    // column is site chrome.
    // The transcription lives in one <section>; everything after its close is
    // site chrome (Back to Main Page, Shop the Store, Privacy Policy, the
    // sidebar table of contents). Bounding on </main> let all of that through.
    const end = h.indexOf("</section>", start) > 0 ? h.indexOf("</section>", start) : h.length;
    const region = h.slice(start - 200, end);

    const STATES = [
      "Delaware", "Maryland", "Virginia", "North Carolina", "South Carolina",
      "Georgia", "New Hampshire", "Massachusetts", "Connecticut", "New York",
      "New Jersey", "Pennsylvania",
    ];

    let inSignatures = false;
    let state = null;
    let names = [];
    const flushState = () => {
      if (state && names.length) {
        signerCount += names.length;
        out.push("**" + state + "** " + names.join(", "));
        out.push("");
      }
      names = [];
    };

    // Walk headings and paragraphs in document order.
    const nodes = [...region.matchAll(/<(h2|h3|p)\b[^>]*>([\s\S]*?)<\/\1>/g)];
    for (const n of nodes) {
      const tag = n[1];
      const inner = n[2];
      const txt = textOf(inner);
      if (!txt) continue;

      if (tag === "h2" && /^Article\b/i.test(txt)) {
        flushState();
        articleCount++;
        out.push("## Article " + romanOf(txt));
        out.push("");
        continue;
      }
      if (tag === "h3" && /^Section\b/i.test(txt)) {
        out.push("### Section " + (txt.match(/(\d+)/) || ["", "?"])[1]);
        out.push("");
        continue;
      }
      // A state name as a heading means the signature block has begun.
      if (tag === "h3" && STATES.includes(txt.replace(/\.$/, ""))) {
        if (!inSignatures) {
          inSignatures = true;
          out.push("## Signatures");
          out.push("");
        }
        flushState();
        state = txt.replace(/\.$/, "");
        continue;
      }
      if (tag !== "p") continue;

      // NARA runs the preamble as an unheaded paragraph, but both clients build
      // their navigation rail from headings, so it needs one.
      if (/^We the People/.test(txt)) {
        out.push("## Preamble");
        out.push("");
        out.push(txt);
        out.push("");
        continue;
      }

      // The attestation closes the instrument and opens the signature block.
      if (/subscribed our Names/i.test(txt)) {
        inSignatures = true;
        out.push("## Signatures");
        out.push("");
        out.push(txt);
        out.push("");
        continue;
      }

      // Washington signs first and alone, as President of the Convention.
      if (/Washington/.test(txt) && /deputy from Virginia/i.test(txt)) {
        const parts = inner.split(/<br\s*\/?>/i).map(textOf).filter(Boolean);
        signerCount++;
        out.push("**" + parts[0] + "**" + (parts[1] ? " " + parts[1] : ""));
        out.push("");
        continue;
      }

      if (inSignatures) {
        // A signer paragraph is identifiable by its links: every name is an
        // anchor into that state's founding-fathers page. Testing for that
        // rather than "we are past the attestation" is what stops the walk from
        // swallowing the page footer, which previously appended "Shop the
        // National Archives Store" to Pennsylvania's delegation.
        if (!/href="\/founding-docs\/founding-fathers-[a-z-]+#/i.test(inner)) continue;
        for (const part of inner.split(/<br\s*\/?>/i)) {
          const t = textOf(part);
          if (t) names.push(t);
        }
        continue;
      }

      // Ordinary constitutional paragraph. NARA wraps each superseded passage in
      // a link to the amendment that replaced it, so split those out and label
      // them rather than leaving the reader to guess which words are dead.
      const pieces = [];
      let last = 0;
      const linkRe = /<a\b[^>]*href="([^"]*amendment[^"]*)"[^>]*>([\s\S]*?)<\/a>/gi;
      let m;
      while ((m = linkRe.exec(inner)) !== null) {
        const before = textOf(inner.slice(last, m.index));
        if (before) pieces.push({ kind: "text", text: before });
        const label = romanOf((m[1].match(/amendment-([ivxlc]+)/i) || ["", ""])[1] ?
          "Amendment " + (m[1].match(/amendment-([ivxlc]+)/i)[1]) : "");
        pieces.push({ kind: "superseded", text: textOf(m[2]), by: label });
        last = m.index + m[0].length;
      }
      const tail = textOf(inner.slice(last));
      if (tail) pieces.push({ kind: "text", text: tail });

      if (!pieces.length) pieces.push({ kind: "text", text: txt });

      for (const p of pieces) {
        if (p.kind === "superseded") {
          supersededCount++;
          out.push("**Superseded" + (p.by ? " by Amendment " + p.by : "") + ".** " + p.text);
        } else {
          out.push(p.text);
        }
        out.push("");
      }
    }
    flushState();
  }

  // ── Amendments ──
  out.push("---");
  out.push("");
  out.push("## Amendments");
  out.push("");

  /** Emit amendments from one NARA page. */
  function emitAmendments(h, startMarker) {
    const b = h.indexOf("</head>");
    const at = h.indexOf(startMarker, b);
    if (at < 0) throw new Error("amendments start marker not found: " + startMarker);
    // Back up to the opening tag of the HEADING that contains the marker.
    // Slicing at the marker text itself cut off the <h2>/<h3>, so the first
    // amendment on each page was silently dropped. Backing up a single tag is
    // not enough either: Amendment XI is written
    // `<h3><a id="xi" name="xi"></a>AMENDMENT XI</h3>`, so one step lands on the
    // nested anchor. Search for the heading tag itself.
    const start = Math.max(h.lastIndexOf("<h2", at), h.lastIndexOf("<h3", at));
    if (start < 0) throw new Error("no heading tag before marker: " + startMarker);
    // The transcription lives in one <section>; everything after its close is
    // site chrome (Back to Main Page, Shop the Store, Privacy Policy, the
    // sidebar table of contents). Bounding on </main> let all of that through.
    const end = h.indexOf("</section>", start) > 0 ? h.indexOf("</section>", start) : h.length;
    const region = h.slice(start, end);

    const nodes = [...region.matchAll(/<(h2|h3|p)\b([^>]*)>([\s\S]*?)<\/\1>/g)];
    for (const n of nodes) {
      const attrs = n[2] || "";
      const inner = n[3];
      const txt = textOf(inner);
      if (!txt) continue;

      if (/^AMENDMENT\b/i.test(txt) && (n[1] === "h2" || n[1] === "h3")) {
        amendmentCount++;
        out.push("### Amendment " + romanOf(txt));
        out.push("");
        continue;
      }
      if (n[1] === "h3" && /^Section\b/i.test(txt)) {
        out.push("**Section " + (txt.match(/(\d+)/) || ["", "?"])[1] + ".**");
        out.push("");
        continue;
      }
      if (n[1] !== "p") continue;

      // "Passed by Congress ... Ratified ..." and the Archives' cross-reference
      // notes are provenance, not constitutional text. Block-quoted so they can
      // never be read as part of an amendment, which is exactly the mistake the
      // previous source's footnotes caused.
      if (/class="smaller"/.test(attrs) || /^Passed by Congress/i.test(txt)) {
        out.push("> " + txt);
        out.push("");
        continue;
      }
      if (/^Note:/i.test(txt)) {
        out.push("> " + txt);
        out.push("");
        continue;
      }
      // NARA's own asterisk footnotes, e.g. "*Superseded by section 3 of the
      // 20th amendment." They are the Archives speaking, not the Constitution,
      // so they get the same block quote as the other provenance notes rather
      // than sitting loose in the text as a stray line beginning with "*".
      if (/^\*/.test(txt)) {
        out.push("> " + txt.replace(/^\*\s*/, ""));
        out.push("");
        continue;
      }
      // Navigation buttons ("Back to Constitution Main Page").
      if (/class="btn\b/.test(attrs) || /class="btn\b/.test(inner)) continue;
      // Page furniture on the Bill of Rights page.
      if (/^Constitutional Amendments 1-10/i.test(txt)) continue;
      if (/^The U\.S\. Bill of Rights/i.test(txt)) continue;

      out.push(txt);
      out.push("");
    }
  }

  emitAmendments(html.bor, "Amendment I");
  emitAmendments(html.rest, "AMENDMENT XI");

  const md = out.join("\n").replace(/\n{3,}/g, "\n\n") + "\n";
  fs.mkdirSync(path.dirname(OUT), { recursive: true });
  fs.writeFileSync(OUT, md);

  console.log("articles              :", articleCount, "(expect 7)");
  console.log("amendments            :", amendmentCount, "(expect 27)");
  console.log("signers               :", signerCount, "(expect 39)");
  console.log("superseded passages   :", supersededCount, "labelled");
  console.log("markdown written      :", OUT, Math.round(md.length / 1024) + " KB");

  // ── Verbatim checks. These must fail on OMISSION, not only on corruption. ──
  const CHECKS = [
    ["preamble", "We the People of the United States, in Order to form a more perfect Union"],
    ["preamble archaic spelling", "provide for the common defence"],
    ["art I s1", "All legislative Powers herein granted shall be vested in a Congress"],
    ["art I s2 archaic", "chuse"],
    ["art I s8 THE GRANT ITSELF", "The Congress shall have Power"],
    ["art I s8 commerce", "To regulate Commerce with foreign Nations, and among the several States"],
    ["art II oath", "I do solemnly swear (or affirm) that I will faithfully execute the Office of President"],
    ["art III treason", "Treason against the United States, shall consist only in levying War against them"],
    ["art VI supremacy", "shall be the supreme Law of the Land"],
    ["art VII", "The Ratification of the Conventions of nine States"],
    ["attestation", "Seventeenth Day of September in the Year of our Lord"],
    ["attestation engrossed spelling", "Independance"],
    ["attestation witness", "subscribed our Names"],
    ["signature: Washington", "Washington"],
    ["signature: a Delaware deputy", "Geo: Read"],
    ["signature: Franklin", "Franklin"],
    ["signature: Hamilton", "Alexander Hamilton"],
    ["signature: Madison", "Madison"],
    ["1st amendment", "Congress shall make no law respecting an establishment of religion"],
    ["2nd amendment", "A well regulated Militia, being necessary to the security of a free State"],
    ["5th amendment", "nor be deprived of life, liberty, or property, without due process of law"],
    ["12th amendment: the word OF", "and of the number of votes for each"],
    ["12th amendment: the word THE", "in the presence of the Senate and House of Representatives"],
    ["13th amendment", "Neither slavery nor involuntary servitude"],
    ["14th amendment", "equal protection of the laws"],
    ["18th amendment: territorY singular", "all territory subject to the jurisdiction thereof"],
    ["19th amendment", "on account of sex"],
    ["26th amendment", "who are eighteen years of age or older"],
    ["27th amendment", "shall take effect, until an election of Representatives shall have intervened"],
  ];

  // Structural assertions: nothing absent, nothing contaminated.
  const STRUCTURE = [
    ["preamble has a heading", () => /^## Preamble$/m.test(md)],
    ["7 articles", () => articleCount === 7],
    ["27 amendments", () => amendmentCount === 27],
    ["39 signers", () => signerCount === 39],
    ["superseded passages labelled", () => supersededCount >= 6],
    // The four defects that came from the old source must not reappear.
    ["no manufactured elision marks", () => !/\*\s\*\s\*/.test(md)],
    ["no bracketed editorial insertions", () => !/\[(the House of Representatives|Each House may|House|the Senators and Representatives)\]/.test(md)],
    ["no House Manual ratification apparatus", () =>
      !md.split("\n").some(l => !l.trimStart().startsWith(">") && /was proposed to the (legislatures|conventions)/.test(l))],
    ["Article I Section 8 begins with its grant", () => {
      const s8 = md.split(/^### Section 8$/m)[1] || "";
      return /^\s*The Congress shall have Power/.test(s8);
    }],
    // Scraping a live page means the page's own furniture is one bad boundary
    // away from being printed as constitutional text. It happened: bounding on
    // </main> instead of </section> put "Shop the National Archives Store" and
    // the privacy-policy footer inside the amendments.
    ["no site furniture", () => !/Shop the National Archives|This page was last reviewed|Privacy Policy|Freedom of Information Act|USA\.gov|NARA-NARA|Back to (Main )?Constitution|For biographies of the non-signing/i.test(md)],
    // A loose line starting with "*" is a NARA footnote that escaped its quote.
    ["no stray footnote markers", () => !md.split("\n").some(l => /^\*[A-Za-z]/.test(l))],
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
    console.error("\n" + failed + " check(s) FAILED. Not shipping a mangled constitution.");
    process.exit(1);
  }
  console.log("\nall verbatim and structural checks passed");
}

main().catch(e => {
  console.error(e.stack || String(e));
  process.exit(1);
});
