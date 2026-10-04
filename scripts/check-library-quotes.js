#!/usr/bin/env node
// Refuses a verbatim quotation in a shipped Library document when the source it
// is attributed to is marked `use: facts` in the registry.
//
// WHY THIS EXISTS
// The operator's licensing rule is that only public-domain material ships in
// the bundle and anything else is fetched at runtime. data/sources/registry.json
// encodes that per source: `use: verbatim` means the source's own words may
// ship, `use: facts` means restate the fact in our own words, because facts are
// not copyrightable (Feist v. Rural Telephone) but expression is.
//
// On 2026-09-15 three separate guides were found breaking that rule in one day:
//
//   cold_and_hypothermia.md  5 verbatim quotations from MedlinePlus /ency/
//   treating_burns.md        2 from the same place
//   reading_the_tide.md      3 from Washington DOH, while its OWN Sources
//                            heading read "cited as the authority, facts
//                            restated"
//
// None of the writers was careless. The licence question is invisible at the
// moment of writing: a source says something crisply, quoting it is the obvious
// move, and nothing on the page says which half of the registry that source
// sits in. Two of the three cases were made worse by a registry entry that was
// itself wrong (MedlinePlus was flagged verbatim/bundle-true until that day).
//
// HOW IT DECIDES. Quotation marks are paired in order within a paragraph (first
// with second, third with fourth), and every pair enclosing 20 characters or
// more is a quotation. For each one it looks back through the preceding prose
// for the NEAREST recognisable source name, among every source in the registry,
// quotable or not. If that nearest source is `use: facts`, the quotation is
// flagged; if it is `use: verbatim` (CDC, USDA and other public-domain
// publishers), the quotation is that source's own words and may ship. A
// quotation with no identifiable source nearby is ignored, because this checks
// attribution and cannot judge unattributed text.
//
// Both halves of that were wrong until 2026-10-04 (six false reports, found
// while merging two Library batches):
//   - PAIRING. A regex took the closing mark of a short quote as the opening of
//     the next, so in `its "Heavy Cotton" T-shirt as 100 percent cotton ... and
//     its "safety" colours` the prose BETWEEN the two quotes was reported as a
//     quotation from Gildan.
//   - ATTRIBUTION. It took the first restate-only source named anywhere in the
//     lookback, and never considered quotable sources at all, so "The CDC's
//     wording: ..." was blamed on the NCHFP named two sentences earlier.
//
// THE OPT-OUT. Some verbatim quotation of a copyrighted source is legitimate:
// quoting a copyright notice in order to attribute it, for instance. Put
// `<!-- quote-ok: reason -->` on the line before such a quotation.
//
// PROVEN RED: scripts/tests/check-library-quotes.test.js holds the cases, each
// seen failing against the version before 2026-10-04's fix.
//
//   node scripts/check-library-quotes.js
//   node scripts/check-library-quotes.js --verbose
const fs = require('fs');
const path = require('path');

const DIR = 'data/library';
const REG = 'data/sources/registry.json';
const LOOKBACK = 220;     // characters of prose before a quote to search for its source
const MIN_QUOTE = 20;

// Matching a registry source inside running prose is the hard part, and the
// first version of this got it badly wrong: it indexed every word of six or
// more characters from a source's name, so "State natural resources or public
// lands agency" matched the bare word "resources" and the gate reported 329
// problems, almost all of them nonsense. A check that cries wolf teaches people
// to ignore it, which is worse than not having one.
//
// So: match the source's FULL registry name, or one of a small set of aliases
// written down here on purpose. Nothing is derived automatically. That misses a
// source a writer names in some form nobody anticipated, which is the right way
// to be wrong: this gate should under-report and be trusted rather than
// over-report and be skipped. Add an alias when you meet one. Every name is
// matched as whole words, so an alias may be short ("CDC") without matching
// inside another word.
const ALIASES = {
  medlineplus: ['medlineplus', 'medline plus'],
  'wa-doh': ['washington state department of health', 'washington department of health',
    'washington doh', 'state department of health'],
  nchfp: ['nchfp', 'national center for home food preservation'],
  'penn-state-extension': ['penn state extension', 'penn state'],
  'iowa-state-extension': ['iowa state extension', 'iowa state university extension', 'iowa state'],
  'nc-state-extension': ['nc state extension', 'north carolina state extension', 'nc state'],
  'umn-extension': ['university of minnesota extension', 'umn extension', 'university of minnesota'],
  'umaine-extension': ['university of maine extension', 'umaine extension'],
  wdfw: ['wdfw', 'washington department of fish and wildlife'],
  ashrae: ['ashrae'],
  'dupont-technical': ['dupont'],
  who: ['world health organization', 'world health organisation'],
  cadth: ['cadth'],
  // Quotable (public-domain) publishers as writers name them. These matter
  // because a quotation introduced as "The CDC's wording" belongs to the CDC
  // even when a restate-only source was named a sentence earlier.
  cdc: ['cdc'],
  usda: ['usda'],
  epa: ['epa'],
  osha: ['osha'],
  niosh: ['niosh'],
  fema: ['fema'],
  usgs: ['usgs'],
  nps: ['national park service'],
};

const escapeRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/// Every name the checker recognises, quotable sources included.
function buildNeedles(registry) {
  const needles = [];
  for (const s of registry) {
    if (s.use !== 'facts' && s.use !== 'verbatim') continue;
    const add = (frag, deliberate) => {
      const f = String(frag).trim().toLowerCase();
      if (!deliberate && f.length < 5) return; // a derived name this short identifies nothing
      if (!f) return;
      needles.push({
        frag: f,
        re: new RegExp('(^|[^a-z0-9])' + escapeRe(f) + '(?![a-z0-9])', 'g'),
        id: s.id,
        name: s.name || s.id,
        licence: s.licence,
        use: s.use,
      });
    };
    if (s.name) add(s.name, false);
    for (const a of ALIASES[s.id] || []) add(a, true);
  }
  return needles;
}

/// The source named closest before the quotation (the latest match in the
/// lookback); on a tie, the longer, more specific name.
function nearestSource(before, needles) {
  let best = null;
  for (const n of needles) {
    n.re.lastIndex = 0;
    let m, last = -1;
    while ((m = n.re.exec(before)) !== null) {
      last = m.index + m[1].length;
      if (m[0].length === 0) n.re.lastIndex++;
    }
    if (last < 0) continue;
    if (!best || last > best.pos || (last === best.pos && n.frag.length > best.needle.frag.length)) {
      best = { pos: last, needle: n };
    }
  }
  return best ? best.needle : null;
}

/// The quotations in one paragraph: marks paired in order, so a stray mark
/// spoils its own paragraph and nothing else.
function quotationsIn(paraText) {
  const marks = [];
  for (let i = 0; i < paraText.length; i++) if (paraText[i] === '"') marks.push(i);
  const out = [];
  for (let k = 0; k + 1 < marks.length; k += 2) {
    const inner = paraText.slice(marks[k] + 1, marks[k + 1]);
    if (inner.length >= MIN_QUOTE) out.push({ index: marks[k], text: inner });
  }
  return out;
}

/// Problems in one document's text. Pure, so the tests can feed it fixtures.
function checkDocument(file, text, needles, stats = { quotes: 0, attributed: 0 }) {
  // Normalise line endings before anything else. The paragraph split below
  // looks for a blank line as LF-only, so it does not match a CRLF blank line,
  // and on a Windows checkout with autocrlf the whole document collapses into
  // one paragraph and the pairing desynchronises exactly as it did before the
  // paragraph fix. Found by running this over a git checkout-index scratch
  // tree, which applies the conversion: it reported 7 phantom problems on
  // content that is clean with LF.
  const normalised = text.replace(/\r\n/g, '\n');

  // Blank out fenced code so an example does not read as a quotation.
  const scan = normalised.replace(/```[\s\S]*?```/g, m => ' '.repeat(m.length));

  // Quote pairing is POSITIONAL, so a single unbalanced quotation mark
  // anywhere desynchronises every pairing after it and the rest of the file
  // goes silently unchecked. That is not hypothetical: it hid an A.D.A.M.
  // copyright notice from this very gate, in a document the gate had just
  // reported clean.
  //
  // Scanning paragraph by paragraph contains the damage. A stray mark now
  // spoils its own paragraph and nothing else, and a quotation does not span
  // a blank line anyway.
  const paragraphs = [];
  {
    let offset = 0;
    for (const p of scan.split(/\n[ \t]*\n/)) {
      paragraphs.push({ text: p, offset });
      offset += p.length + 2;
    }
  }

  const problems = [];
  for (const para of paragraphs) {
    for (const q of quotationsIn(para.text)) {
      stats.quotes++;
      // Positions are paragraph-relative; map back to the document for the
      // quote-ok marker and the lookback, both of which may sit before the
      // paragraph began.
      const abs = para.offset + q.index;
      const lineStart = scan.lastIndexOf('\n', abs) + 1;
      const prevLineStart = scan.lastIndexOf('\n', lineStart - 2) + 1;
      if (scan.slice(prevLineStart, lineStart).includes('quote-ok:')) continue;

      const before = scan.slice(Math.max(0, abs - LOOKBACK), abs).toLowerCase();
      const hit = nearestSource(before, needles);
      if (!hit || hit.use !== 'facts') continue;
      stats.attributed++;
      problems.push({
        file,
        id: hit.id,
        name: hit.name,
        licence: hit.licence,
        quote: q.text.replace(/\s+/g, ' ').slice(0, 90),
      });
    }
  }
  return problems;
}

module.exports = { buildNeedles, nearestSource, quotationsIn, checkDocument, ALIASES };

if (require.main === module) {
  const VERBOSE = process.argv.includes('--verbose');
  if (!fs.existsSync(DIR) || !fs.existsSync(REG)) {
    console.log('No built Library or no registry; nothing to check.');
    process.exit(0);
  }
  const registry = JSON.parse(fs.readFileSync(REG, 'utf8')).sources;
  const needles = buildNeedles(registry);
  const stats = { quotes: 0, attributed: 0 };
  let docs = 0;
  const problems = [];
  for (const f of fs.readdirSync(DIR).filter(f => f.endsWith('.md'))) {
    docs++;
    problems.push(...checkDocument(f, fs.readFileSync(path.join(DIR, f), 'utf8'), needles, stats));
  }

  if (problems.length) {
    console.log('');
    console.log('Verbatim quotations from sources the registry says to RESTATE:');
    for (const p of problems) {
      console.log('  ' + p.file);
      console.log('    source: ' + p.id + ' (' + p.name + ', licence ' + p.licence + ', use: facts)');
      console.log('    quote : "' + p.quote + '"');
    }
    console.log('');
    console.log('  Facts are not copyrightable and expression is, so restate the fact');
    console.log('  in our own words and keep the citation. Only sources marked');
    console.log('  `use: verbatim` may ship their own wording in the bundle.');
    console.log('');
    console.log('  If a quotation is genuinely legitimate (attributing a copyright');
    console.log('  notice, say), put <!-- quote-ok: reason --> on the line before it.');
    console.log('');
  }

  if (VERBOSE) {
    console.log('  ' + stats.quotes + ' quotations seen, ' + stats.attributed +
      ' attributable to a restate-only source');
  }
  console.log('Checked quotations in ' + docs + ' shipped Library documents. Problems: ' +
    problems.length);
  process.exit(problems.length ? 1 : 0);
}
