# Score file formats: a first-pass comparison (2026-11-16, DRAFT)

Asked by the operator on 2026-11-16 ("See if you can do a pros/cons of the file formats with available
usage") for the goal he set for the next usage reset: "To some how save orchestral music in a file
format everyone can easily modify ... If we use a special file format then we should also convert the
humanity accord to that format so we can play the humanity accord as instrumental music."
(docs/PRIORITIES.md TIER 0; docs/design/music.md, "The operator's wishes of 2026-11-16").

**Status: a draft from what was already known, written at the end of a week's usage. No source was
read for it.** Every licence and every fact about a tool below is marked "verify" until it is quoted
from its primary source with that source's date (CLAUDE.md: compare before adopting; legal questions
get a dated finding). Step 1 of the reset goal is to turn this into the verified comparison.
The MEI and Humdrum rows, and the licences marked "Verified 2026-11-16", were checked against their
primary sources in `mei-and-humdrum.md` beside this file; every other "verify" still stands.

## What we need from the format

1. **Everyone can easily modify it**: a person with no programming, through a free app, and a
   contributor by hand in a text editor.
2. **Orchestral detail**: many parts, dynamics, articulations, tempo changes, repeats.
3. **Lyrics under notes**, so the Humanity Accord's words can sit under the notes the choir sings
   their vowels on (the sing-along).
4. **Open and free**: a royalty-free specification, readable without a paid tool, and public-domain
   scores available in it.
5. **Readable by our engine**: a parser we can use or write (in Rust, no AI, small).
6. **Diff-friendly** in git, so changes can be reviewed.

## The candidates

| Format | What it is | Pros | Cons | Licence (verify) |
|---|---|---|---|---|
| **MusicXML** (.musicxml, compressed .mxl) | The common interchange format for written music, from the W3C Music Notation Community Group | Nearly every notation program reads and writes it, including the free MuseScore, so anyone can edit with a mouse; full orchestral detail; lyrics per note; chord symbols and Roman numerals (`harmony` / `numeral`); large public-domain libraries can export to it | Verbose XML, unpleasant to edit by hand; programs export it slightly differently; files are big (the .mxl form is zipped); no variants, per-note analysis or text structure (see mei-and-humdrum.md) | Verified 2026-11-16: version 4.0 is "published by the Music Notation Community Group under the W3C Community Final Specification Agreement (FSA)" ("Final Community Group Report 1 June 2021", https://www.w3.org/2021/06/musicxml40/); royalty terms of the FSA itself not read (verify) |
| **MIDI** (.mid, Standard MIDI File) | Performance data: which note, when, how hard | Every music program and keyboard speaks it; tiny files; simple to parse (mature Rust crates exist, verify which); easy import from anywhere | Not notation: no lyrics under notes in a dependable way, no articulations or written dynamics, no clean bar structure; binary, not human-editable; quantisation is lossy | Specification from the MIDI Association, free to read, no royalties (verify) |
| **ABC notation** (.abc) | Plain-text notation designed for folk tunes | The easiest format to type and read by hand; tiny, diff-friendly; lyrics supported; JavaScript renderers exist for the website | Built for single melodies and small bands; multi-part orchestral writing is awkward and tools disagree on its extensions; fewer editors with a mouse | The standard is freely published (verify) |
| **LilyPond** (.ly) | A text language that engraves beautiful scores | Human-readable text with full orchestral detail; diff-friendly; the Mutopia Project shares public-domain classical works in it (verify licences per piece; its piano In the Hall of the Mountain King, piece 1888, reads "Public Domain", checked 2026-11-16) | A programming-like language with a learning curve; to play it we would need our own parser of a large language or to convert it (LilyPond can export MIDI, verify MusicXML) | The program is GPL (verify); the format is plain text |
| **MEI** (Music Encoding Initiative) | An XML encoding framework for written music, from critical editions to performance and analysis; MEI Basic is its interchange profile | Lyrics per note (`verse` / `syl`) plus the text's own line groups; analysis tied to notes (`harm`, `deg`, `intm`, `psolfa`); alternative readings in one file (`app` / `rdg`, `choice`); per-passage sources; **MuseScore 4.2+ imports and exports it** (MEI 5.1 Basic), mei-friend and Verovio edit it on the web; CC BY 4.0 Beethoven piano encodings exist (TROMPA) | Verbose (Op. 120 in MEI: 4.6 MB); a MuseScore save most likely keeps only the Basic core; no pure-Rust crate | Verified 2026-11-16: "MEI is distributed under the Educational Community License Version 2.0, a modified form of the Apache 2.0 license." (music-encoding.org/about); detail in mei-and-humdrum.md |
| **Humdrum \*\*kern** (.krn) | A tab-separated text syntax: one column (spine) per part or per kind of data (`**kern` notes, `**text` lyrics, `**harm`, `**deg`), one line per moment | The most compact (one Beethoven sonata movement: 16.8 KB) and line-diffable; analysis stored as spines and computed by humlib's filters (BSD-2); the largest classical corpora | No mouse editor (MuseScore has no Humdrum importer); an orchestra is 20+ columns wide; many corpora restricted (CCARH MuseData: personal academic use only) or with no licence file (Sapp's Beethoven); the Toolkit and Verovio Humdrum Viewer state no licence | Per corpus, verified 2026-11-16 in mei-and-humdrum.md: TAVERN CC BY-SA 4.0, Sapp's Mozart CC BY-NC-SA 4.0, CCARH restrictive, KernScores unreadable (site under maintenance) |
| **MNX** | The W3C group's newer, JSON-based successor to MusicXML | Designed to be simpler and easier to parse than MusicXML; JSON suits our data style | Still being developed (verify its state); little software writes it yet | W3C community licence (verify) |
| **MuseScore's own** (.mscz) | The free MuseScore editor's native file | Edited in a free, friendly app | Tied to one program and its versions; not a standard; MuseScore exports MusicXML and MEI anyway | Verified 2026-11-16: the program is under "the GNU General Public License version 3" (its `LICENSE.txt`) |
| **Markdown** (.md) | Text with headings | Everyone can open it | Not a music format: we would be inventing one inside it | n/a |

## First-pass recommendation (to confirm at the reset)

> **Revised the same day by `mei-and-humdrum.md`** (the in-depth look at MEI and Humdrum the operator
> asked for): store scores as **MEI**, its musical core kept valid MEI Basic so MuseScore (4.2 and
> later) can edit it with a mouse, with MEI's richer layers (lyrics with the Accord's line structure,
> analysis tied to notes, simplified and full readings in one file, per-passage sources) on top;
> MusicXML becomes the main import and export, Humdrum an import and the model for analysis. The list
> below is kept as first written.

- **Store written music as MusicXML**, edited in MuseScore (free) by anyone with a mouse, and by hand
  when needed. It is the only candidate that meets "everyone can modify it" (through a free app),
  orchestral detail and lyrics under notes at once, and it is what public-domain sources can export.
- **Import MIDI as a convenience** (a person drops in a .mid and the player reads its notes), knowing
  it brings no lyrics or written dynamics.
- **Watch MNX**: if it is usable at the reset, it may be the easier format for our engine, with MusicXML
  still the import path.
- **Not chosen**: ABC (too thin for an orchestra, though lovely for folk tunes; maybe a later import),
  LilyPond as the stored format (a large language to parse), MEI and \*\*kern (scholarly), MuseScore's
  own format (one program), Markdown (not a music format).
- **The Humanity Accord as music**: the generator composes it once per section, the result is saved as
  a MusicXML score with the Accord's words as lyrics under the notes, and the choir sings each lyric
  syllable's vowel while the line scrolls as a sing-along. Anyone can then open the Accord's score in
  MuseScore, change it, and share it.

## To do at the reset (step 1 of the goal)

- Read and quote each licence and specification page with its date; correct this table.
- Name the Rust options for reading MusicXML and MIDI (crates and their licences) against writing our
  own small reader, and say which.
- Check what Mutopia, IMSLP and MuseScore's community library offer for Grieg's In the Hall of the
  Mountain King and Beethoven, for the dated legal finding (step 2).
