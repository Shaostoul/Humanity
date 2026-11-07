# Instrument sources: what was downloaded, from where, when

**Manifest dated 6 November 2026** (downloads made 2026-11-07 UTC, the evening of 2026-11-06
Pacific). The raw sample libraries behind the app's free instrument library
([`docs/design/music.md`](../design/music.md) rung 6; the code is `src/instrument_packs/`, the
pipeline `scripts/instrument-packs.js`, run by `just instrument-packs`).

The operator approved these four downloads on 2026-11-06 ("Awesome, let's build up our free
library." and "Let's go all in."), whole libraries, from the official sources named in the dated
finding [`findings/2026-11-06-music-sources.md`](findings/2026-11-06-music-sources.md), which
cleared each as public domain (CC0). Any other library needs his yes first; the candidates found for
a steel pan and deeper drums are in
[`findings/2026-11-06-steel-pan-and-deep-drums.md`](findings/2026-11-06-steel-pan-and-deep-drums.md)
and were **not** downloaded.

**Where they are.** Outside the repository, on the development machine only:
`E:\HumanityOS\instruments-src\` (`download-log.txt` there has each download's time). Never in git:
only the small core set the pipeline makes from them (`data/music/`, under 30 MB) is committed, and
the downloadable packs are GitHub release assets (`docs/ai/release.md`, "Instrument packs").

**How each was verified.** The size, against the source's own figure where it publishes one; the
SHA-256, computed on arrival and recorded below and in `scripts/instrument-recipes.json` (the pipeline
refuses a file that no longer matches); for the two GitHub repositories, every file of the archive
against the git blob hash GitHub's API lists for the pinned commit (a SHA-1 over each file's
contents, so a single changed byte anywhere fails). No source publishes a checksum of its own:
FreePats' page shows none, and the Karoryfer release asset (uploaded 2022) has no digest on GitHub.

**The licence texts** are copied word for word in
[`instrument-licences/`](instrument-licences/), and the pipeline puts each library's licence text in
every sample folder it makes, so it travels with the samples into the app and into every pack.

## 1. FreePats Upright Piano KW

- Source page: https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html#UprightKW (read
  2026-11-06): "Published under the terms of the Creative Commons CC0 1.0 public domain
  dedication."
- Downloaded 2026-11-07 05:29 to 05:30 UTC, the whole library in its SFZ+FLAC form (the WAV and SF2
  downloads hold the same recordings in other formats):

| File (from `https://freepats.zenvoid.org/Piano/UprightPianoKW/`) | Bytes | SHA-256 |
|---|---|---|
| `UprightPianoKW-SFZ+FLAC-20220221.7z` (full, version 2022-02-21; the page says 32 MiB) | 33,294,287 | `3a66da3b25a2fd9499d90bcf09f1aee4431825796e249361b48af59b1201e703` |
| `UprightPianoKW-small-SFZ+FLAC-20190703.7z` (small; the page says 2.9 MiB) | 3,092,715 | `e346ef198fcda0694fc66ddb80c2db2a1c700e96e3a1801930e3cebfed37047f` |
| `UprightPianoKW-small-bright-SFZ+FLAC-20190703.7z` (small, bright) | 3,299,501 | `590f8900aec595a4ad6037c373623837bd34d111ab07251fda4a5c704ffa6742` |

- Each archive's own CRC32 for every file checked on reading (70, 30 and 30 files, all matching).
- Licence text: `cc0.txt` inside each archive, copied as
  [`instrument-licences/freepats-upright-piano-kw-cc0.txt`](instrument-licences/freepats-upright-piano-kw-cc0.txt);
  the archive's `readme.txt` (who recorded it, where, when) as
  [`freepats-upright-piano-kw-readme.txt`](instrument-licences/freepats-upright-piano-kw-readme.txt).
- Used for: the core set's small upright piano; the keys pack's full upright.

## 2. Versilian Community Sample Library (VCSL)

- Source: https://github.com/sgossner/VCSL, licence identified by GitHub as CC0-1.0; README: "This
  collection is under a Creative Commons 0 license."
- Pinned commit `c1ea7bcc3c7309650ab0da9d15c9cd1fbc4a4c7e` (2026-01-14, "Create LICENSE"), downloaded
  2026-11-07 05:59 to 06:46 UTC as GitHub's archive of that commit,
  https://codeload.github.com/sgossner/VCSL/tar.gz/c1ea7bcc3c7309650ab0da9d15c9cd1fbc4a4c7e

| File | Bytes | SHA-256 |
|---|---|---|
| `VCSL-c1ea7bcc3c7309650ab0da9d15c9cd1fbc4a4c7e.tar.gz` | 4,052,372,898 | `e4f9ed1fd698306627bc4911c7b7c8db2506cf3d00d486b630723855cc4ed529` |

- Unpacked: 4,282 files, 6,160,422,703 bytes, **every one matching its git blob hash** in the
  commit's tree (0 different, 0 missing, 0 extra). README: "you can do whatever you want with these
  sounds (even make commercial software), no royalties, no credit, no special terms."
- Licence text: `LICENSE` (CC0 1.0 Universal), copied as
  [`instrument-licences/vcsl-LICENSE.txt`](instrument-licences/vcsl-LICENSE.txt).
- Used for: the core set's harp, vibraphone, glockenspiel, tubular bells, wine glasses and bass drum;
  the keys pack's Kawai grand and Yamaha upright; the strings pack's concert harp; the mallets pack;
  most of the percussion pack (bass drums, timpani, frame drum, low tom, gong).

## 3. VS Chamber Orchestra 2: Community Edition (VSCO 2 CE)

- Source: https://github.com/sgossner/VSCO-2-CE, licence identified by GitHub as CC0-1.0; the
  project page https://versilian-studios.com/vsco-community/ (read 2026-11-06): "The library is
  under a Creative Commons 0 (i.e. public domain) license."
- Pinned commit `440300901dfe9275fd84e0b7763af1f8443ae62e` (2020-08-05, "Updated Readme"), downloaded
  2026-11-07 05:33 to 05:59 UTC as GitHub's archive of that commit,
  https://codeload.github.com/sgossner/VSCO-2-CE/tar.gz/440300901dfe9275fd84e0b7763af1f8443ae62e

| File | Bytes | SHA-256 |
|---|---|---|
| `VSCO-2-CE-440300901dfe9275fd84e0b7763af1f8443ae62e.tar.gz` | 2,302,119,553 | `2050dfd58513fd8058765c3a567636b148206da297a35c8b52f2ec4344035316` |

- Unpacked: 3,187 files, 3,237,679,771 bytes, **every one matching its git blob hash** in the
  commit's tree (0 different, 0 missing, 0 extra).
- Licence text: `LICENSE` (CC0 1.0 Universal), copied as
  [`instrument-licences/vsco-2-ce-LICENSE.txt`](instrument-licences/vsco-2-ce-LICENSE.txt).
- **Worth knowing:** the library's older `Readme.txt` (signed "Sam Gossner, Versilian Studios LLC.
  June 29th, 2016", copied as [`vsco-2-ce-Readme.txt`](instrument-licences/vsco-2-ce-Readme.txt))
  says "You are permitted to use these samples for ANY purpose. We ask that you do not sell the
  samples directly", and asks for credit to "Versilian Studios/Sam Gossner, and/or Ivy Audio/Simon
  Dalzell where applicable" with a link to the homepage. These are requests beside the CC0 licence,
  not conditions of it. HumanityOS gives the samples away free and credits them, with that link, in
  every instrument definition made from them.
- Used for: the core set's violins and cellos; the strings pack's sections, double bass, solo violin
  and pizzicato; the percussion pack's large hand drum (from the library's "VSCO 1 Percussion",
  which its readme offers "on a strictly "as-is" basis").

## 4. Karoryfer Black And Green Guitars

- Source page: https://shop.karoryfer.com/pages/free-samples (read 2026-11-06): "All our free sample
  libraries are under a Creative Commons Zero license." Its Download link is the GitHub release
  asset below (no checkout, no sign-up).
- Downloaded 2026-11-07 05:30 to 05:33 UTC:

| File (https://github.com/sfzinstruments/karoryfer.black-and-green-guitars/releases/download/v1.000/) | Bytes | SHA-256 |
|---|---|---|
| `Karoryfer_Black_And_Green_Guitars_1000.zip` (release v1.000, 2022-07-21) | 481,838,512 | `ba95221d32fb6386c5d0a0799b93904fd55520aca017c5f121da290075c140c0` |

- The size matches the 481,838,512 bytes GitHub's API lists for the asset (which has no digest).
- Licence text: the archive's `LICENSE` is the CC0 1.0 Universal legal code (checked, as the
  finding advised, because the shop warns older downloads may carry an older licence), copied as
  [`instrument-licences/karoryfer-black-and-green-guitars-LICENSE.txt`](instrument-licences/karoryfer-black-and-green-guitars-LICENSE.txt);
  its `readme.txt` ("Royalty-free for all commercial and non-commercial use. Copyright 2022
  Karoryfer Lecolds.") as
  [`karoryfer-black-and-green-guitars-readme.txt`](instrument-licences/karoryfer-black-and-green-guitars-readme.txt).
  Line endings of the two copies were made plain LF; the words are unchanged.
- Used for: the guitars pack (a green Gretsch Anniversary and a black Hofner Club, recorded by
  Brian Wood).

## What the app gets from them

The pipeline keeps what each instrument needs and encodes it as Ogg Vorbis; nothing is retuned on
disk (the engine plays each sample at the rate that puts it at A = 432 Hz or 440 Hz; each sample's
recorded pitch, measured by the pipeline, is in its instrument's definition). The core set and the
packs, their sizes and checksums: `data/music/packs.ron`, and the summary in
[`docs/design/music.md`](../design/music.md) section 23.
