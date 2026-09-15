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
// HOW IT DECIDES. For every quoted span of 20 characters or more, it looks back
// through the preceding prose for the nearest recognisable source name. If that
// source is `use: facts`, the quotation is flagged. A quotation with no
// identifiable source nearby is ignored, because this checks attribution and
// cannot judge unattributed text.
//
// THE OPT-OUT. Some verbatim quotation of a copyrighted source is legitimate:
// quoting a copyright notice in order to attribute it, for instance. Put
// `<!-- quote-ok: reason -->` on the line before such a quotation.
//
// PROVEN RED: revert any of the three fixes above and this names the document,
// the source and the quotation.
//
//   node scripts/check-library-quotes.js
//   node scripts/check-library-quotes.js --verbose

const fs = require('fs');
const path = require('path');

const DIR = 'data/library';
const REG = 'data/sources/registry.json';
const VERBOSE = process.argv.includes('--verbose');
const LOOKBACK = 220;     // characters of prose before a quote to search for its source
const MIN_QUOTE = 20;

if (!fs.existsSync(DIR) || !fs.existsSync(REG)) {
  console.log('No built Library or no registry; nothing to check.');
  process.exit(0);
}

const registry = JSON.parse(fs.readFileSync(REG, 'utf8')).sources;

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
// over-report and be skipped. Add an alias when you meet one.
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
};

const needles = [];
for (const s of registry) {
  if (s.use !== 'facts') continue;             // only the restate-only sources matter here
  const add = (frag) => {
    const f = String(frag).trim().toLowerCase();
    if (f.length < 5) return;                  // too short to identify anything
    needles.push({ frag: f, id: s.id, name: s.name || s.id, licence: s.licence });
  };
  if (s.name) add(s.name);
  for (const a of ALIASES[s.id] || []) add(a);
}
// Longest fragment first, so the most specific name wins.
needles.sort((a, b) => b.frag.length - a.frag.length);

const problems = [];
let docs = 0, quotes = 0, attributed = 0;

for (const f of fs.readdirSync(DIR).filter(f => f.endsWith('.md'))) {
  const text = fs.readFileSync(path.join(DIR, f), 'utf8');
  docs++;

  // Normalise line endings before anything else. The paragraph split below
  // looks for a blank line as LF-only, so it does not match a CRLF blank line,
  // and on a Windows checkout with autocrlf the whole document collapses into
  // one paragraph and the pairing desynchronises exactly as it did before the
  // paragraph fix. Found by running this over a git checkout-index scratch
  // tree, which applies the conversion: it reported 7 phantom problems on
  // content that is clean with LF.
  const normalised = text.replace(/\r\n/g, '\n');

  // Blank out fenced code so an example does not read as a quotation.
  let scan = normalised.replace(/```[\s\S]*?```/g, m => ' '.repeat(m.length));

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

  for (const para of paragraphs) {
  const re = /"([^"]{20,})"/g;
  let m;
  while ((m = re.exec(para.text)) !== null) {
    quotes++;
    // Positions are paragraph-relative; map back to the document for the
    // quote-ok marker and the lookback, both of which may sit before the
    // paragraph began.
    const abs = para.offset + m.index;
    const lineStart = scan.lastIndexOf('\n', abs) + 1;
    const prevLineStart = scan.lastIndexOf('\n', lineStart - 2) + 1;
    if (scan.slice(prevLineStart, lineStart).includes('quote-ok:')) continue;

    const before = scan.slice(Math.max(0, abs - LOOKBACK), abs).toLowerCase();
    const hit = needles.find(n => before.includes(n.frag));
    if (!hit) continue;
    attributed++;
    problems.push({
      file: f,
      id: hit.id,
      name: hit.name,
      licence: hit.licence,
      quote: m[1].replace(/\s+/g, ' ').slice(0, 90),
    });
  }
  }
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
  console.log('  ' + quotes + ' quotations seen, ' + attributed +
    ' attributable to a restate-only source');
}
console.log('Checked quotations in ' + docs + ' shipped Library documents. Problems: ' +
  problems.length);
process.exit(problems.length ? 1 : 0);
