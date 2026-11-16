# MEI and Humdrum, in depth (researched 2026-11-16)

Asked by the operator on 2026-11-16, after the first-pass comparison (`README.md` beside this file)
called MEI and Humdrum "overkill" and unfamiliar. Verbatim: "The fact you say it is overkill is why I
want to know more about this option. We have plenty of usage for research." The goal it serves
(docs/PRIORITIES.md TIER 0; docs/design/music.md, "The operator's wishes of 2026-11-16"): "To some how
save orchestral music in a file format everyone can easily modify", play public-domain classical music
(Grieg's In the Hall of the Mountain King, Beethoven) on the app's own instruments, and "convert the
humanity accord to that format so we can play the humanity accord as instrumental music", with the
five-vowel choir singing the Accord's syllables while the line scrolls as a sing-along.

This is a research note, not a legal finding. Section 4 ends with the list of what the dated legal
finding (step 2 of the goal) must settle.

## Method

- Every site's robots.txt was read first, in a step of its own, before any page on it. Refusals and
  unreachable sites are listed in section 7.
- No corpus, archive or binary was downloaded. Where a fact needs a file, the file is named with its
  size instead.
- GitHub repositories were read through the GitHub REST API (`gh api`: licence detection, a file's
  text, the date of the last commit touching it), not by crawling github.com. Quotes from those files
  are exact, copied from the file; in quotes from Markdown files, link and emphasis markup is shown as
  its visible text.
- Web pages were read with a page-reading tool that extracts the requested sentences. Those quotes
  were asked for verbatim and are given as returned, but they were not compared character by
  character against the page. **The legal finding must re-quote any sentence it relies on from the
  page itself.**
- Rust crates were looked up through the crates.io API (its robots.txt allows every reader).
- Every request used a generic client name and carried no project or personal details.
- "Read" dates are all 2026-11-16. Each source's own date is given where the page or file shows one.

## 1. What each format is, and what the extra power buys

### MEI (Music Encoding Initiative)

**What it is.** From the MEI website's About page (https://music-encoding.org/about/, footer
"© Music Encoding Initiative 2026"): MEI is "a system for encoding musical documents in a
machine-readable structure." It "represents the common expressive features of traditional facsimile,
critical, and performance editions" and "supports scholarly analysis and a variety of both digital
and print rendering possibilities." The same page: "Since 2026, MEI is co-hosted by the RISM Digital
Center in Bern."

The Guidelines (https://music-encoding.org/guidelines/v5/content/introduction.html, MEI version 5.1,
commit #9d551da, no date shown) set out the design:

- "The encoding scheme is based on eXtensible Markup Language (XML),"
- "MEI is an encoding framework, not a data format."
- "The encoding scheme is intended to facilitate interchange between notational tools."
- It describes four musical domains (logical, gestural, visual, analytical), noting that "not all four
  domains are available throughout the MEI schema." and "Very often, MEI prioritizes the visual domain
  over the gestural domain".

**Profiles (customizations).** The same introduction names the shipped customizations: mei-CMN ("For
most users, this will be the best starting point into music encoding with MEI."), mei-Mensural,
mei-Neumes, mei-Basic and mei-all ("This is the full definition of MEI." and "In general, you should
almost never use mei-all except for testing purposes."). On MEI Basic:

- "MEI Basic simplifies this by providing only one approach for each music feature"
- "As noted, MEI Basic only supports Common Western Music Notation."
- "Many of the more complex encoding mechanisms for editorial and analytical workflows are also
  removed in MEI Basic."
- "The purpose of mei-Basic is to serve as common ground for data interchange"

**The chapters that are the "overkill"** (Guidelines v5.1 table of contents): Common Music Notation,
Mensural Notation, Neume Notation, String Tablature, Lyrics and Performance Directions, Text Encoding,
Analysis Markup and Harmonies, Scholarly Editing with MEI, Facsimiles and Recordings, Linking Data,
Integrating MEI with other Standards and Formats.

- **Editorial markup** (https://music-encoding.org/guidelines/v5/content/scholarlyediting.html): the
  chapter "covers editorial observations in and interventions to the text". Its elements include
  `choice` ("Groups a number of alternative encodings for the same point in a text."), `sic` / `corr`,
  `orig` / `reg`, `abbr` / `expan`, `add`, `del`, `unclear` ("Contains material that cannot be
  transcribed with certainty because it is illegible or inaudible in the source."), `supplied` and
  `gap`.
- **Critical apparatus** (same page): "This chapter describes how to encode differences between
  multiple exemplars of the same musical work". "An app element always encapsulates the differences
  between varying sources." `lem` "Contains the lemma, or base text, of a textual variation." and `rdg`
  "Contains a single reading within a textual variation." Readings "use the source attribute to point
  to one or more descriptions of the bibliographic sources", each source declared once in the header
  (`sourceDesc`). So one file holds every source's version of a passage, and each version says where
  it came from.
- **Lyrics** (https://music-encoding.org/guidelines/v5/content/lyricsperfdir.html): "Each lyric
  syllable can be encoded directly within an associated note, either by using the syl attribute on
  note or the verse element." "When there is more than one line of text, more than one verse element
  can be used." On a `syl`, `wordpos` marks the syllable's place in its word (i: "at the beginning of
  a word", m: "in the middle of a word", t: "at the end of a word") and `con` the connector drawn after
  it (s space, d dash, u underscore for a held syllable, t tilde for an elision). Text that is not
  under notes is kept as line groups: `lg` "May be used for any section of text that is organized as
  a group of lines;" (the page names "a stanza, refrain, verse paragraph") and `l` "Contains a single
  line of text within a line group."
- **Analysis** (https://music-encoding.org/guidelines/v5/content/analysisharm.html): `harm` holds
  "the symbols of harmonic analysis like Roman numerals and their interpretation", `fb` / `f` figured
  bass. Per-note analytical attributes: `inth` ("Encodes the harmonic interval between pitches
  occurring at the same time."), `intm` ("Encodes the melodic interval from the previous pitch."),
  `deg` (scale degree), `mfunc` (melodic function), `pclass` ("Holds pitch class information."),
  `psolfa` ("Contains sol-fa designation"). "Scale-degree values are relative to the prevailing major
  or minor key." A `harm` "must define a point of attachment using one of the following attributes"
  (`startid`, `tstamp`, `tstamp.ges`, `tstamp.real`), so an analysis label is tied to the exact note
  or beat it describes.
- **Facsimiles and recordings**: a chapter of its own; the mei-friend README (below) describes
  displaying `zone` elements "ontop of the surface images", that is, linking each bar or note to the
  region of a scanned page it was read from.

### Humdrum (\*\*kern and its kin)

**What it is.** Humdrum is a plain-text syntax, not one format: "The Humdrum syntax provides a
framework within which representation schemes can be defined."
(https://www.humdrum.org/guide/ch05/). Layout: "Humdrum encodings consist of a set of one or more lines
or _records._" "When more than one token is present, tokens are separated from each other by tabs."
"When several data records are present, multiple tokens are aligned in columns through the file."
Those columns are spines. Exclusive interpretations (`**kern`, `**text`, ...) name what a spine holds;
tandem interpretations (`*M4/4`, `*C:`) "merely modify or supplement the exclusive interpretation."
`*^` will "split a spine (into two)" and `*v` will "join (two or more) spines into one". Comments start
with `!` (local) or `!!` (global).

**\*\*kern** (https://www.humdrum.org/rep/kern/, "Site last generated: Aug 29, 2024", author David
Huron): "The kern scheme can be used to represent basic or core information for period-of-common-
practice Western music." "kern is designed to facilitate analytic applications rather than music
printing or sound generation." "Pitch information is encoded through a scheme of upper- and
lower-case letters." "Successive octaves are designated by letter repetition, thus C5 is represented
by `cc`". The page itself says "Other Humdrum representations should be used for these latter
purposes" (printing and sound).

**Its kin.** The representations page (https://www.humdrum.org/rep/, same generation date) lists, among
others, `**text` and `**silbe` (lyrics), `**harm` (functional harmony), `**deg` (scale degree),
`**solfa`, `**pitch`, `**freq`, `**MIDI`, `**semits`, `**mint` (melodic interval), `**hint` (harmonic
interval), `**Tonh`, `**embel` and `**IPA`. Any number of these sit side by side as columns aligned to
the same moments, so a melody, its lyric, its chord label and its scale degrees share one line per
time slice.

**Form.** Humdrum guide chapter 20 (https://www.humdrum.org/guide/ch20/, "Page last edited: 11 Apr
2020"): "A section label is a type of tandem interpretation" (`*>Name`), "An expansion list is another
tandem interpretation that contains an ordered list of section labels." "The Humdrum thru command
expands abbreviated format representations to a so-called through-composed format." "In order to
distinguish different versions, each expansion list is given a unique version label." (its example: a
Landowska order with a repeat and a Gould 1982 order without).

**Tools.** The Humdrum Toolkit (repository humdrum-tools/humdrum) is, by its README, a set of unix
command-line tools, written as AWK, shell and C programs (`toolkit-source/README.txt`: "Humdrum
Toolkit source code, version 2.3.1", dated Tue May 27 2014). Its source listing includes programs
such as `key`, `hint`, `pc` and `kern`. humlib (craigsapp/humlib) is "a set of C++ classes for parsing
Humdrum data files" whose command-line tools "also can be used via JavaScript in web applications"
(README). In Verovio Humdrum Viewer, humlib's tools run as filters: "Filters are embedded commands
within the musical data that modify the data." (https://doc.verovio.humdrum.org/). The filter index
(https://doc.verovio.humdrum.org/filter/, "Site last generated: 4 Jun 2026") names, among others,
`deg`, `tspos`, `colortriads`, `fb`, `cint` ("can be used to study counterpoint"), `autocadence`,
`addkey`, `dissonant`, `humdiff`, `extract` and `transpose`. The `deg` page: "The deg filter analyzes
scale degrees in `**kern` data." and "The key signature is not used in the analysis of scale
degrees;" (the key comes from a key designation, or `--default-key`).

### The "overkill", said plainly

The first pass meant: most of what MEI and Humdrum can say is never needed to just play notes. Here is
what that extra power lets a person or the app DO that MusicXML or MIDI cannot (MusicXML 4.0 does have
chord symbols and Roman numerals: its `numeral` element "represents the Roman numeral or Nashville
number part of a harmony", https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/numeral/).

| With MEI or Humdrum you can | MEI | Humdrum | MusicXML | MIDI |
|---|---|---|---|---|
| Keep two or more versions of a passage in ONE file (a simplified reading beside the full one; one edition's reading beside another's) | Yes: `app` / `rdg`, `choice` | Partly: labelled expansion lists choose different section orders; variant notes are not a core feature | No | No |
| Say where each passage came from (which public-domain edition, which scan) | Yes: `@source` on readings, sources in `sourceDesc`; facsimile zones link notes to page regions | Per file, in reference records (`!!!` lines, such as the `YEM` licence record) | Per file only | No |
| Store analysis bound to the notes (scale degree, interval, melodic function, chord function) for a "what am I hearing?" overlay | Yes: `harm` attached by `startid`/`tstamp`; `deg`, `intm`, `inth`, `mfunc`, `pclass`, `psolfa` on notes | Yes, as whole spines: `**harm`, `**deg`, `**mint`, `**hint`; several analyses side by side (TAVERN keeps two annotators' analyses) | Chord symbols and Roman numerals only (`harmony` / `numeral`) | No |
| Compute analysis from the notes with existing open tools | Not directly: Verovio writes Humdrum only from MusicXML input (section 3) | Yes: humlib filters `deg`, `tspos`, `colortriads`, `fb`, `cint` (BSD-2 library) | Through Verovio's MusicXML to Humdrum conversion, then humlib | No |
| Keep a text's own structure beside its sung syllables (stanzas, refrains, lines) | Yes: `verse`/`syl` under notes plus `lg`/`l` line groups and the Text Encoding chapter | Lyrics as `**text` / `**silbe` spines; section labels name the form | Lyrics per note only | Lyric meta-events, not bound to notes dependably |
| Mark editorial doubt and correction (illegible, supplied, corrected) | Yes: `unclear`, `supplied`, `sic`/`corr` | Comments only | No | No |
| Encode early notations (mensural, neumes) and tablature | Yes (modules) | Not checked (no mensural representation is among those listed on humdrum.org/rep/) | Not checked here | No |

## 2. Does that power serve HumanityOS?

**Teaching ("teach everyone everything").** Yes, and this is the strongest argument for the overkill.
A "what am I hearing?" overlay needs chord and key labels at each moment. Stored, human-written
analysis is better teaching material than a live guess, and both MEI and Humdrum store it tied to the
notes; MusicXML holds only chord symbols and Roman numerals. Humdrum is where the open, computed
analysis already exists (humlib's filters run without any AI), and where an open analysed corpus of
Beethoven exists (TAVERN, CC BY-SA 4.0, two annotators' Roman numeral analyses, section 4). Ear
training (scale degrees, intervals, solfege) maps directly onto MEI's `deg`, `intm`, `psolfa` and
Humdrum's `**deg`, `**mint`, `**solfa`.

**The Accord as a text-and-music work.** Yes. Every format here can put a syllable under a note; the
difference is keeping the Accord's own structure. MEI holds the sung syllables (`verse`/`syl`, with
`wordpos` telling the sing-along where a word starts and ends) and, in the same file, the text as line
groups (`lg`/`l`) and sections, so the scrolling line can show the Accord's real line and section, not
just a stream of syllables. Humdrum can do it with a `**text` spine plus section labels, more tersely.
For the sing-along highlight on the website, Verovio's timemap gives, per event, a time in
milliseconds and the IDs of the notes that start and stop (section 3).

**Editing by everyone.** This is where the first pass was wrong about MEI. MuseScore 4.2.0 (released
2023-12-18) added, per its release notes on GitHub: "The option to import/export scores in the MEI file
format (Thanks to community contributors @lpugin and @rettinghaus)." Its exporter writes MEI 5.1
Basic (`src/importexport/mei/internal/meiexporter.cpp`, line 124:
`std::string schema = "https://music-encoding.org/schema/5.1/mei-basic.rng";`, last changed
2026-07-28), lyrics included (`writeVerses`, `writeSyl`). So anyone with the free MuseScore can open,
edit and save MEI with a mouse. MuseScore has no Humdrum importer (its `src/importexport/` folder holds
bb, bww, capella, guitarpro, mei, midi, mlscore, mnx, musedata, musicxml, ove, tabledit and export
folders; read 2026-11-16). For hand editing in a browser: mei-friend (MEI, text beside live notation,
GitHub integration) and Verovio Humdrum Viewer (Humdrum, text beside live notation, filters). Neither is
a mouse-first notation editor: mei-friend calls itself "a ‘last mile’ editor".

The catch: MEI Basic removes "the more complex encoding mechanisms for editorial and analytical
workflows", and MuseScore writes Basic. A score opened and saved in MuseScore will most likely come
back without the richer layers (variants, analysis). Not tested here (open question 1). The design
answer is to keep those layers where a MuseScore round-trip cannot drop them, or to re-apply them by
note ID after an edit.

**Git-friendly diffs.** Humdrum is the most diff-friendly for small ensembles: one line per time slice,
so changing a note changes one line. But adding or removing a part rewrites every line (a new column),
and an orchestra is 20 or more tab-separated columns wide, which is hard to read or edit by hand. MEI
puts one element per line in typical output, so a note change is a small diff, but it is verbose, and
whether MuseScore keeps the same `xml:id`s between saves (otherwise every save is a noisy diff) is not
known (open question 2). MusicXML is the most verbose of the three.

**The app's rules.** All three are open specifications needing no AI and no paid tool. All are data
files anyone can mod (Humdrum by hand most easily; MEI and MusicXML through MuseScore). The website can
render MEI, Humdrum and MusicXML through Verovio's WebAssembly build; the native app would read the
file in Rust for playback and needs a notation renderer only if it shows the score (open question 4).

## 3. Tools and licences (each quoted from its primary source)

| Tool or specification | Licence, quoted | Source and its date |
|---|---|---|
| MEI schema and Guidelines | "MEI is distributed under the Educational Community License Version 2.0, a modified form of the Apache 2.0 license." LICENSE file: "The Educational Community License version 2.0 ("ECL") consists of the Apache 2.0 license, modified to change the scope of the patent grant in section 3 to be specific to the needs of the education communities using this license." | https://music-encoding.org/about/ (© 2026); music-encoding/music-encoding `LICENSE` (last changed 2023-10-25); the Guidelines repository (music-encoding/guidelines) is detected as ECL-2.0 too |
| MEI sample encodings | "This repo is released under the ECL-2.0, and original creations contributed to this repo are accepted under the same license." | music-encoding/sample-encodings `README.md` (2025-01-22) |
| Verovio (engraver and converter) | "Verovio is available under the LGPL license (see COPYING and COPYING.LESSER)." (COPYING holds the GPL version 3 text, COPYING.LESSER the LGPL; detected LGPL-3.0) | rism-digital/verovio `README.md` (2026-04-21), `COPYING` (2021-01-19) |
| mei-friend (browser MEI editor) | "The mei-friend Web application is published under the GNU AGPL 3.0 license." (linked to https://www.gnu.org/licenses/agpl-3.0.html) | mei-friend/mei-friend `README.md` (2025-05-23) |
| humlib (C++ Humdrum library and tools) | "Copyright (c) 2015-2021 Craig Stuart Sapp / All rights reserved. / Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:" (detected BSD-2-Clause) | craigsapp/humlib `LICENSE.txt` (2021-01-17) |
| Humdrum Toolkit | **No licence file.** Script headers read "# Copyright (c) 1993 David Huron" (`toolkit-source/shell-scripts/key`). A SourceForge catalog entry seen only in a search result lists LGPLv2; not read, not a primary source | humdrum-tools/humdrum (last push 2025-06-18; the script's last change 2014-06-01) |
| Verovio Humdrum Viewer | **No licence stated** in its repository (no licence file; `README.md` 2020-11-11) or on its documentation home page | humdrum-tools/verovio-humdrum-viewer; https://doc.verovio.humdrum.org/ ("Site last generated: 4 Jun 2026") |
| MuseScore (for comparison: the free mouse editor) | "This program is free software; you can redistribute it and/or modify it under the terms of the GNU General Public License version 3 as published by the Free Software Foundation." | musescore/MuseScore `LICENSE.txt` (read 2026-11-16) |
| MusicXML 4.0 (in passing) | "published by the Music Notation Community Group under the W3C Community Final Specification Agreement (FSA)" | https://www.w3.org/2021/06/musicxml40/ ("Final Community Group Report 1 June 2021") |

**What Verovio converts and renders** (https://book.verovio.org/, pages engraved with "Verovio
6.4.0-cb5fcfa"; README: Verovio "is written in standard C++20"):

- Input: MEI, Humdrum, MusicXML (compressed .mxl: "Input of MXL files is auto detected"), Plaine and
  Easie, ABC and CMME; Humdrum-enabled builds add MuseData and EsAC. Limits stated on the page: MEI
  "elements that are not supported by Verovio will be ignored."; ABC "Multi-voice music is not
  supported" and "Tuplets are not supported".
- Output: SVG; MEI ("With its MEI output, Verovio can serve as a converter to MEI."); MIDI ("Verovio
  provides a basic MIDI output feature", built on the MidiFile library; in JavaScript `renderToMIDI()`
  returns base64); the timemap ("The timemap is an array of JSON objects", each with a time in
  milliseconds, a time in quarter notes and the IDs of notes that start and end); the expansion map;
  Plaine and Easie; and Humdrum, which "is available only from MusicXML input". Humdrum to MEI:
  `verovio file.krn --no-layout --all-pages -t mei`.
- **WebAssembly for the website: yes.** "To build the JavaScript toolkit you need to have the
  Emscripten compiler installed on your machine." "The recommended version is one built as WebAssembly
  because it is the fastest". "npm install verovio"; CDN file
  `https://www.verovio.org/javascript/latest/verovio-toolkit-wasm.js`. The Humdrum-enabled file
  `verovio-toolkit-hum.js` "increases the size of the toolkit by about one third." No absolute file
  size is given on the page. Bindings listed: Python, Java and Android, Swift and Objective-C, Go.
- Embedded libraries (README): humlib, JSON++, MidiFile, pugixml, MINIZ-CPP, tuning-library.

**Rust options** (crates.io API, read 2026-11-16):

| Crate | Version, licence | What it is |
|---|---|---|
| `verovio` (+ `verovio-sys`, `verovio-data`) | 0.3.6, LGPL-3.0-or-later, updated 2026-07-08 | "Safe Rust bindings to the Verovio music engraving library." (a cxx bridge to the C++) |
| `verovioxide` (+ `-sys`, `-data`) | 0.3.0, Apache-2.0, updated 2026-02-07 | Bindings; README: "Zero runtime dependencies: Verovio statically linked" and "The first build compiles the Verovio C++ library from source, which takes several minutes." The crate is Apache-2.0, but the Verovio it links stays LGPL |
| `rustovio` | 0.1.0, MIT OR Apache-2.0, updated 2021-06-26 | "bindings to verovio" (stale) |
| `rameau_kern` | 0.2.1, AGPL-3.0-or-later, updated 2026-09-11 | "A Humdrum **kern parser: spines, chords, ties and barlines into timed notes" (15 downloads) |
| `music21` / `music21-rs` | 0.20.1 / 0.16.0, AGPL-3.0-only | "A Rust port of music21"; README: "ABC, MIDI, TinyNotation, Humdrum, MEI and RomanText are read with no feature at all" |
| `acorde-io` | 1.3.13, MIT OR Apache-2.0 | "MusicXML / MIDI / ABC / MSCZ parsers and serializer" (no MEI, no Humdrum) |

No standalone pure-Rust MEI crate was found. Scale reference for a full MEI reader and writer:
MuseScore's MEI module is `meiimporter.cpp` (116,847 bytes), `meiexporter.cpp` (91,177 bytes) and
`meiconverter.cpp` (136,982 bytes) of C++ plus a third-party libmei. A player needs far less (notes,
rests, chords, ties, durations, tempo, dynamics, parts, lyrics); a pure-Rust MEI Basic reader on an
XML crate is an estimate of a few thousand lines, not measured. A \*\*kern reader is smaller still.

## 4. Public-domain classical sources in these formats

| Source | Format | What it holds | Terms, quoted | Usable? |
|---|---|---|---|---|
| KernScores (kern.humdrum.org) | Humdrum | The large Humdrum score library | **Could not read**: robots.txt answered 503 with a maintenance page of the Narodowy Instytut Fryderyka Chopina ("Przepraszamy, trwają prace techniczne. Serwis chwilowo niedostępny.", "technical work in progress, service temporarily unavailable"), twice on 2026-11-16 | Unknown until its terms are read |
| Beethoven piano sonatas, Craig Sapp (craigsapp/beethoven-piano-sonatas) | Humdrum | "a digital edition of the 32 piano sonatas composed by Ludwig van Beethoven, encoded in the Humdrum file format" (README, 2014-06-03); repository description: "based on the Durand 1915 edition edited by Paul Dukas" | **No licence file.** The humdrum-data README says licence terms live in each file's reference records, "particularly `YEC` (copyright notice) and `YEM` (license)". File to check (not read): `kern/sonata01-1.krn`, 16,790 bytes | Unknown until the per-file `YEM` records are read |
| Beethoven string quartets, Craig Sapp (craigsapp/beethoven-string-quartets) | Humdrum | "Digital edition of L. van Beethoven's string quartests in the Humdrum file format." (sic) | **No licence file**; same per-file `YEM` question | Unknown |
| CCARH MuseData editions in Humdrum (for example musedata/humdrum-haydn-symphonies) | Humdrum | Haydn symphonies, Bach, Corelli, Mozart and Haydn quartets, Vivaldi (humdrum-data catalogue) | "The enclosed MuseData files are for personal use (single-user)* only." "MuseData files are made available only for the purpose of academic research; MuseData files may be used in the course of teaching, but under no circumstance is the data to be embedded or included in teaching materials for commercial or non-commercial distribution." "this license does not authorize the use of the enclosed MuseData files in the production of derivative editions intended for commercial distribution, nor for public performance (including broadcast), nor for sound recording." (`LICENSE.txt`, 2024-03-01) | **No.** A negative finding: not for an app that ships to everyone |
| Mozart piano sonatas, Craig Sapp | Humdrum | From the Alte Mozart-Ausgabe (1878) | "Licensed with Attribution-NonCommercial-ShareAlike 4.0 International (CC BY-NC-SA 4.0)" (`LICENSE.txt`, 2018-06-21) | Doubtful: NonCommercial conflicts with an AGPL app anyone may sell |
| TAVERN (jcdevaney/TAVERN) | Humdrum | "27 sets of theme and variations (T&V) for piano by Mozart and Beethoven"; "The analyses contain both Roman numeral analysis as well as functional analysis", one per annotator | "We publish the contents of this corpus under an Attribution-ShareAlike 4.0 International (CC BY-SA 4.0) license." (`LICENSE`, 2023-06-19) | Yes, with attribution and share-alike |
| TROMPA Music Encodings (trompamusic-encodings, 25 Beethoven repositories found) | MEI | Beethoven piano works, "encoding of Breitkopf und Härtel edition, 1862–90" (for example Op. 120, `Beethoven_Op120-Breitkopf.mei`, 4,610,353 bytes) | "This work is released by the licensor, Werner Goebl, University of Music and Performing Arts Vienna, on behalf of the TROMPA Project under the Creative Commons Attribution 4.0 International (CC-BY-4.0) license." (Op. 120 `LICENSE.md`, 2020-04-07; each repository carries its own) | Yes, with attribution (check each repository) |
| MEI sample encodings | MEI | Teaching samples (folders Complete_examples, Editorial_markup, Encoding_alternatives, Lyrics, Music_structure) | ECL-2.0 (quoted in section 3) | Yes, as references |
| DCML Annotated Beethoven Corpus (DCMLab/ABC) and DCMLab/beethoven_piano_sonatas | MuseScore and TSV (not MEI or Humdrum) | Harmonic analyses of the string quartets; piano sonatas | `LICENSE` begins "Attribution-NonCommercial-ShareAlike 4.0 International" (ABC, 2021-05-19) | Doubtful: NonCommercial |
| When in Rome (MarkGotham/When-in-Rome) | RomanText analyses with scores | A meta-corpus of functional harmonic analyses | "New content in this repository, including the new analyses, code, and the conversion (specifically) of existing analyses is available under the CC BY-SA licence (a free culture licence) except by arrangement."; "These external licences vary." (README, 2025-01-29) | The new analyses, yes; each external score separately |
| OpenScore Lieder and String Quartets | MuseScore / MusicXML (not MEI or Humdrum) | 19th-century songs and quartets | Lieder's `LICENSE.txt` reads "CC0 1.0 Universal"; GitHub detects both repositories' licence files as CC0-1.0 | Yes (CC0); convert to MEI through MuseScore or Verovio |

**In the Hall of the Mountain King.** No MEI or Humdrum encoding was found. What exists:

- **Mutopia Project**, piece 1888 (https://www.mutopiaproject.org/cgibin/piece-info.cgi?id=1888): "In the
  Hall of the Mountain King", "E. Grieg (1843–1907)", "Op. 46, No. 4", "Piano", licence "Public Domain"
  ("CC: No rights reserved"), source "The University Society, 1918", typeset with "LilyPond, version
  2.16.1", "Last updated: 2013/Dec/07"; offers the LilyPond file, a MIDI file and PDFs. A piano
  arrangement, not the orchestral score.
- **IMSLP**, Peer Gynt Suite No. 1, Op. 46 (https://imslp.org/wiki/Peer_Gynt_Suite_No.1,_Op.46_(Grieg,_Edvard)):
  every file for No. 4 in all three parts of the page is a PDF. The page gives composition "1874-75"
  and first publication "1888 - Leipzig: C.F. Peters, Plate 7188-89." Editable files found are Finale
  string parts for other movements under "Creative Commons Attribution Non-commercial Share Alike 3.0".
  A Violins I part of No. 4 "Typeset using MuseScore version 2.1." is offered only as a PDF, under
  "Creative Commons Attribution-NonCommercial 4.0".
- **Sao Mai Music Library** (a library for blind musicians): an `In_hall_of_the_mountain_king.mxl` for
  piano, composer field "pink", with only the site's "Copyright © 2001-2026 by Sao Mai Center for the
  Blind (SMCB)". Provenance unclear; not usable as it stands.

So the orchestral Mountain King would be encoded by us (or by contributors in MuseScore) from a
public-domain orchestral edition, after the legal finding names which one.

**What the dated legal finding (step 2) must settle:**

1. The compositions: Grieg (died 1907) and Beethoven (died 1827) in the United States, the EU and
   Norway.
2. The editions encodings were made from: Durand 1915 (ed. Paul Dukas) for Sapp's sonatas;
   Breitkopf & Härtel 1862 to 1890 for TROMPA; The University Society 1918 for Mutopia's Mountain King;
   C.F. Peters 1888 for an orchestral Mountain King; and whether any jurisdiction gives editors or
   first publishers a separate right that still runs.
3. Each encoding's own licence, file by file: TROMPA (CC BY 4.0: the attribution form inside the app);
   TAVERN and When in Rome (CC BY-SA 4.0); OpenScore (CC0); Mutopia ("Public Domain"); Sapp's
   Beethoven sonatas and quartets (read the `!!!YEC` and `!!!YEM` records; KernScores' own terms once
   it is back); and confirm the exclusions (CCARH MuseData; NonCommercial corpora such as Sapp's Mozart
   and DCML; the Sao Mai file).
4. How attribution and share-alike files sit beside the project's licences (plan B, 2026-11-14:
   AGPL-3.0 code, CC BY-SA 4.0 Library and docs, CC0 data): a CC BY or CC BY-SA score cannot be
   relabelled CC0, so score files need a per-file licence record.
5. The tools if adopted: Verovio LGPL-3.0 linked into an AGPL-3.0-or-later exe and served as
   WebAssembly on the website; the MEI schema (ECL-2.0) if the RNG ships with the app; mei-friend
   (AGPL-3.0) if hosted; humlib (BSD-2) notices; `music21-rs` (AGPL-3.0-only) if linked.
6. The Humdrum Toolkit and Verovio Humdrum Viewer state no licence: their code is not used until that
   is clarified with the authors.

## 5. MEI, Humdrum and MusicXML side by side, for HumanityOS

| Need | MEI | Humdrum (\*\*kern) | MusicXML |
|---|---|---|---|
| Edit with a mouse in a free app | Yes: MuseScore 4.2+ imports and exports (MEI 5.1 Basic) | No: MuseScore has no Humdrum importer; convert from MusicXML through Verovio | Yes: MuseScore and nearly every notation program |
| Edit by hand | Verbose XML; mei-friend and Verovio's editor show the notation live | Best for small scores; hard for 20+ orchestral columns; Verovio Humdrum Viewer shows it live | Very verbose XML |
| Orchestral detail | Full | Core notation by design ("basic or core information"); layout and printing elsewhere | Full |
| Lyrics under notes | `verse` / `syl` with word position and connector | `**text` / `**silbe` spines | `lyric` per note |
| The Accord's text structure kept | Yes: `lg` / `l` line groups and text chapters in the same file | Section labels and comments | No |
| Analysis for teaching | Stored per note (`harm`, `deg`, `intm`, `psolfa`) | Stored as spines, several side by side; computed by humlib filters | Chord symbols and Roman numerals only |
| Simplified and full versions in one file | Yes: `app` / `rdg`, `choice` | Section orders only | No |
| Where each passage came from | Per reading (`@source`) and facsimile zones | Per file (`!!!` records) | Per file |
| File size (measured examples) | Op. 120 Diabelli Variations (TROMPA): 4,610,353 bytes | All 32 Beethoven sonatas, 103 files: 2.5 MB (humdrum-data table); one movement, `sonata01-1.krn`: 16,790 bytes | Not measured here; the most verbose of the three |
| Git diffs | Element per line, small note diffs; ID churn unknown | One line per time slice; adding a part rewrites every line | Large, exporter-dependent |
| Reader in our Rust engine | Write our own MEI Basic reader (no MEI crate; `music21-rs` reads MEI, AGPL-3.0-only); or link Verovio (LGPL C++) | Small to write; `rameau_kern` exists (AGPL-3.0-or-later) | Existing crates (`acorde-io`, MIT OR Apache-2.0) |
| Website rendering | Verovio WebAssembly (its native format) | Verovio WebAssembly (Humdrum build, about a third larger) | Verovio WebAssembly (basic single-file MusicXML) |
| Open public-domain classical data | TROMPA Beethoven piano works (CC BY 4.0) | Largest collections, but many restricted (CCARH) or unstated (Sapp's Beethoven); TAVERN (CC BY-SA) | OpenScore (CC0), via MuseScore |
| Specification and tool licences | ECL-2.0 schema; Verovio LGPL-3.0; mei-friend AGPL-3.0 | humlib BSD-2; Toolkit and VHV unstated | W3C FSA, royalty-free |

## 6. Verdict

**Store HumanityOS scores as MEI**, written so that the musical core stays valid MEI Basic (what
MuseScore reads and writes) and the richer MEI layers (analysis labels, alternative readings, the
Accord's line groups, per-passage sources) sit on top. **Import** MusicXML (OpenScore and every
notation program), MIDI (a convenience, no lyrics) and Humdrum (TAVERN's analyses and, once their terms
are read, Sapp's Beethoven) through Verovio's converters or our own readers. **Export** MusicXML when
someone needs it, through MuseScore or Verovio. Humdrum is not the stored format, but its analysis
spines and humlib's filters are the model for the teaching overlay.

The three strongest reasons:

1. **The first pass's main objection is gone.** MuseScore, the free editor the plan already relied on
   for MusicXML, has imported and exported MEI since 4.2.0 (2023-12-18) and writes MEI 5.1 Basic with
   lyrics. "Everyone can easily modify it" holds for MEI exactly as it did for MusicXML, and mei-friend
   and Verovio's editor add hand editing with live notation on the web.
2. **Only MEI holds everything the goal needs in one open file:** syllables under notes with word
   positions for the sing-along (`syl` `wordpos`), the Accord's own lines and sections beside them
   (`lg` / `l`), analysis tied to the exact notes for a "what am I hearing?" overlay (`harm` by
   `startid`, `deg`, `intm`, `psolfa`), and alternative readings (`app` / `rdg`, `choice`), which can
   carry the project's two modes as a simplified and a full reading of the same passage and record
   which public-domain edition each note came from (`@source`), useful for the legal record itself.
3. **Its licences and tools fit the rules:** the schema is ECL-2.0, Verovio is LGPL-3.0 with a
   WebAssembly build the website can use (rendering, MIDI, and a timemap of note IDs per millisecond
   for the sing-along highlight), and it converts MusicXML, Humdrum and ABC into MEI. CC BY 4.0
   Beethoven piano encodings already exist in MEI (TROMPA).

**What the overkill buys us:** teaching material that carries its own analysis, a text-and-music
Accord whose words keep their structure, one file holding both the easy and the full arrangement, and
a score that can say where every passage came from.

**The costs:** a MuseScore save most likely drops what MEI Basic leaves out (open question 1); MEI is
verbose (4.6 MB for the Diabelli Variations, where all 32 sonatas in \*\*kern take 2.5 MB); there is no
pure-Rust MEI crate, so the app needs its own MEI Basic reader (an estimated few thousand lines) or the
C++ Verovio linked in (a build of several minutes, LGPL); humlib cannot read MEI and Verovio writes
Humdrum only from MusicXML, so computed analysis on our MEI files would be our own Rust code modelled
on humlib's filters; and Humdrum's large classical corpora arrive only through conversion, with their
licences still to be read.

**Changes this makes to the first pass:** MEI moves from "not chosen (scholarly)" to the recommended
stored format; MusicXML moves from the stored format to the main import and export; Humdrum stays out
as the stored format but becomes an import and the analysis model.

## 7. Refusals and sites that could not be read

| Site | What happened (2026-11-16) | What was used instead |
|---|---|---|
| musescore.org | robots.txt disallows `ClaudeBot`, `anthropic-ai` and `Claude-Web` for `/`. Not read | MuseScore's own source repository and release notes through the GitHub API (its code, a different primary source) |
| lib.rs | robots.txt request answered with a "Just a moment..." robot check (HTTP 403). Not read, not worked around | The crates.io API |
| kern.humdrum.org (KernScores) | robots.txt answered 503 with a maintenance page, twice. Not read | Craig Sapp's own repositories and the humdrum-data README (no re-hosted KernScores copy was read; the third-party hleveillegauvin/KernScores repository was not opened) |
| ccarh.org, kern.ccarh.org, humdrum.org (without www) | The names did not resolve. Not read | www.humdrum.org and doc.verovio.humdrum.org |
| www.humdrum.org/man/thru/ | 404 | Humdrum guide chapter 20 |
| sourceforge.net (Humdrum Toolkit catalog entry) | Not visited; seen only as a search-result summary | Recorded as unverified |

## 8. Open questions

1. **Does a MuseScore MEI round-trip keep what MEI Basic leaves out** (`harm`, `app` / `rdg`, `lg`,
   analytical attributes) and the `xml:id`s? Test by saving a sample from music-encoding/sample-encodings
   (ECL-2.0) through MuseScore 4.6. If it drops them, the layers live in a companion file keyed by note
   ID, or the app re-applies them after an edit.
2. **Do MuseScore's MEI saves keep stable IDs** between saves (git diff noise)? MuseScore 4.5's release
   notes list "MEI: support for voice assignment of dynamics, staff line color, harp pedal diagrams &
   MuseScore element IDs"; what that means for ID stability was not read.
3. **Our own MEI Basic reader or Verovio linked in** for the native app? Reading for playback suggests
   our own small reader; showing the score natively, with the website mirroring it, suggests the
   `verovio` crate. A decision for the build plan.
4. **Notation on screen in the native app**: Verovio's SVG through a Rust SVG renderer, our own egui
   engraver, or no score view at first (play and sing-along only).
5. **MNX**: MuseScore's source has an `mnx` import and export folder; its state was not examined here
   and stays "verify" in the first pass.
6. **KernScores' terms and the `YEM` records** of Sapp's Beethoven files, once KernScores is back.
7. **The orchestral Mountain King source**: which public-domain orchestral edition to encode from
   (C.F. Peters 1888 is the first publication per IMSLP), and whether to encode it in MuseScore by hand.
