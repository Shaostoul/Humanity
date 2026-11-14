# Instrument sources: what was downloaded, from where, when

**Manifest dated 6 November 2026** (downloads made 2026-11-07 UTC, the evening of 2026-11-06
Pacific); **sections 5 to 7 added 7 November 2026** (downloaded 2026-11-07 09:53 to 09:56 UTC). The raw sample libraries behind the app's free instrument library
([`docs/design/music.md`](../design/music.md) rung 6; the code is `src/instrument_packs/`, the
pipeline `scripts/instrument-packs.js`, run by `just instrument-packs`).

The operator approved the first four downloads on 2026-11-06 ("Awesome, let's build up our free
library." and "Let's go all in."), whole libraries, from the official sources named in the dated
finding [`findings/2026-11-06-music-sources.md`](findings/2026-11-06-music-sources.md), which
cleared each as public domain (CC0). On 2026-11-07 he approved three more ("Let's do it."), sections
5 to 7: the steel pan, the Hang and the big drums found in
[`findings/2026-11-06-steel-pan-and-deep-drums.md`](findings/2026-11-06-steel-pan-and-deep-drums.md).
Nothing else in that finding was downloaded (not the steel pan's version 2, not the CC BY taiko). Any
other library needs his yes first.

**See also:** the scan of every other free library we could use, ranked by family with each licence
quoted and a download list for his permission,
[`research/2026-11-13-instrument-libraries/README.md`](research/2026-11-13-instrument-libraries/README.md),
and its licence finding
[`findings/2026-11-13-free-instrument-libraries.md`](findings/2026-11-13-free-instrument-libraries.md).

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
- Used for: the core set's violins and cellos, and since 2026-11-12 its small trumpet (short notes),
  old trombone (short notes), clarinet and pizzicato double bass (the electro swing genre's brass,
  clarinet and upright bass, docs/design/music.md section 7a; no new download); the strings pack's sections, double bass, solo violin
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
  Brian Wood), and since 2026-11-12 the core set's small Gretsch (the guitar part out of the box).

## 5. jSteelDrum version 1 (Jeff Learman): a C tenor steel pan

- Source: https://github.com/jlearman/jSteelDrum1 (GitHub reports no licence identifier; the
  repository's own `LICENSE` file, read 2026-11-07, is the whole statement): "This is free and
  unencumbered software released into the public domain according to Creative Commons CC0:
  https://creativecommons.org/publicdomain/zero/1.0/ Created by Jeff Learman."
- Downloaded 2026-11-07 09:53 UTC as the repository's "Download ZIP" link,
  https://github.com/jlearman/jSteelDrum1/archive/refs/heads/main.zip; the archive's own comment
  names the commit, `8f37bc7472d89ce3f7ca53265794466ab0cbf23e` (2023-09-26, "Update README.md point
  to latest version").

| File | Bytes | SHA-256 |
|---|---|---|
| `jSteelDrum1-main.zip` | 56,638,307 | `aba369742514652ef61c365c4d52ac1cd4bdc737ca60d4ecd510e5973b39de42` |

- Unpacked: 450 files, 61,331,840 bytes, **every one matching its git blob hash** in that commit's
  tree (0 different, 0 missing, 0 extra).
- Licence text: copied as
  [`instrument-licences/jsteeldrum1-LICENSE.txt`](instrument-licences/jsteeldrum1-LICENSE.txt); the
  README (a steel pan hand-made in Trinidad, recorded in September 2021, 27 notes from C4 to B5 plus
  C#6, D#6 and F6, five velocity layers) as
  [`jsteeldrum1-README.txt`](instrument-licences/jsteeldrum1-README.txt).
- Worth knowing: two files are named "A5" though their MIDI number is 069 (A4); the pipeline takes
  the MIDI number. Its README calls version 2 "much better"; version 2 was not approved and not
  downloaded.
- Used for: the core set's small steel pan; the mallets pack's steel pan.

## 6. FreePats Hang in D minor

- Source page: https://freepats.zenvoid.org/ChromaticPercussion/hang.html ("Version 2022-03-30");
  the download is the release asset of https://github.com/freepats/hang-D-minor (release
  `2022-03-30`, published on GitHub 2024-08-28).
- Downloaded 2026-11-07 09:54 UTC:

| File (https://github.com/freepats/hang-D-minor/releases/download/2022-03-30/) | Bytes | SHA-256 |
|---|---|---|
| `Hang-D-minor-SFZ+FLAC-20220330.7z` | 13,478,020 | `dea56a898c88b838e37f92967fe0edfe41b1bd2aa16ed9a240e3f44efb525a5c` |

- The size matches the 13,478,020 bytes GitHub lists for the asset (no digest published); the
  archive's own CRC32 for each of its 55 files checked on reading, all matching.
- Licence: the archive's README.txt, "Published under the terms of Creative Commons CC0 public
  domain dedication", and its LICENSE.txt, the CC0 1.0 Universal legal code; copied as
  [`freepats-hang-d-minor-LICENSE.txt`](instrument-licences/freepats-hang-d-minor-LICENSE.txt) and
  [`freepats-hang-d-minor-README.txt`](instrument-licences/freepats-hang-d-minor-README.txt).
- Worth knowing: nine notes are sampled (A3, D4, E4, F4, G4, A4, Bb4, C5, D5; the README's list
  also names C4, which has no samples), and the README asks that the semitones between them not be
  filled by pitching neighbours ("Other notes are silent"). The instrument is marked
  `exact_keys`, so a key the Hang does not have plays nothing.
- Used for: the percussion pack's Hang.

## 7. Karoryfer Big Rusty Drums

- Source page: https://shop.karoryfer.com/pages/free-big-rusty-drums; the free-samples page,
  https://shop.karoryfer.com/pages/free-samples: "All our free sample libraries are under a Creative
  Commons Zero license." The download is the release asset of
  https://github.com/sfzinstruments/karoryfer.big-rusty-drums (release `v1.100`, 2023-04-29, tag
  commit `f07ce00df34a46b6b08375be56fe116cf15782bc`).
- Downloaded 2026-11-07 09:54 to 09:56 UTC:

| File (https://github.com/sfzinstruments/karoryfer.big-rusty-drums/releases/download/v1.100/) | Bytes | SHA-256 |
|---|---|---|
| `Big_Rusty_Drums_1100.zip` | 619,600,370 | `d4a9990acd19376d91ce446dae415c81428728b5adebb6a88eddbb3a6aac8744` |

- The size matches the 619,600,370 bytes GitHub lists (no digest published). Unpacked: 4,871
  files. Against the tag's git tree: all 4,493 sample and binary files byte for byte identical; the
  378 text files (programs, GUI, presets, LICENSE) identical once their line endings are made LF
  (the release was zipped from a Windows checkout); only `.gitattributes` is not in the zip.
- Licence text: the archive's `LICENSE` is the CC0 1.0 Universal legal code, copied as
  [`karoryfer-big-rusty-drums-LICENSE.txt`](instrument-licences/karoryfer-big-rusty-drums-LICENSE.txt).
- Used for: the percussion pack's two big mallet toms (the 22-inch, a bass drum set up as a floor
  tom, and the 18-inch), each the close microphone and the overheads mixed, as the library's own
  programs do.

## 8. Martin HD-28 steel-string guitar (Discord SFZ GM Bank, "026-Acoustic Guitar (steel)")

- Approved by the operator 2026-11-13 ("Let's do it"), for that ONE folder of
  https://github.com/sfzinstruments/Discord-SFZ-GM-Bank (the repository is 179 MB of a whole General
  MIDI bank; only this instrument was approved, so nothing else of it was fetched).
- Licence: the repository has no licence file (GitHub reports none); the instrument's own SFZ,
  `Discord GM/Melodic/026-Acoustic Guitar (steel).sfz`, read 2026-11-13 through the GitHub API at
  commit `7a9c478fe331f94f246d33332f0adedb25bbbe27` (2024-03-04), opens with this header, quoted
  exactly:

  ```
  // GM Acoustic Guitar
  // 2017 Martin HD28 Vintage Series
  // Author: Jeff Learman, for Kinwie's Discord SFZ GM: https://github.com/kinwie/Discord-SFZ-GM-Bank
  // License: Creative Commons CC0
  ```

  The pipeline copies those four lines as the instrument's `LICENSE.txt` (`licence_lines: 4`).
- Downloaded 2026-11-13 from `https://raw.githubusercontent.com/sfzinstruments/Discord-SFZ-GM-Bank/7a9c478fe331f94f246d33332f0adedb25bbbe27/Discord%20GM/Melodic/`,
  file by file (the folder last changed 2021-10-01, commit `2b58d62da87d9cf16dcb6299abd1d298b67bfc4b`),
  into `E:\HumanityOS\instruments-src\discord-sfz-gm-martin-hd28\`, 17 files, 3,158,269 bytes; logged
  in `download-log.txt`. Every file's size and SHA-256 is in its library entry in
  `scripts/instrument-recipes.json` (a folder of files rather than one archive; the pipeline refuses a
  changed or missing file), and **all 16 files of the folder match their git blob hash** in that
  commit's tree.
- What it is: plucked notes a minor third apart from E2 to B5 (15 WAV files, one layer, no mutes or
  strums), a steel-string flat-top, the nearest CC0 sound found to the Selmer-Maccaferri of gypsy
  jazz (docs/design/music.md section 7a).
- Worth knowing: the pitch measure locks onto the F5 sample's second harmonic (an octave up); the
  key follows the file name's MIDI number, as for the steel pan. Measured median -8.5 cents from
  A = 440 (the engine plays each sample from its own tuning).
- Used for: the core set's steel-string guitar (`guitar_martin_small`, 337 KB, all 15 notes up to
  4 s), the `acoustic_guitar` part: electro swing's comping guitar, la pompe.

## 9. Further instruments from the libraries already here (2026-11-13, no download)

From VSCO 2 CE (section 3) and VCSL (section 2), into the core set for electro swing's leads:
`solo_violin_small` (VSCO 2 CE `Strings/Solo Violin/Arco Vib`, the soft layer, 15 notes, 984 KB),
the hot-club violin; `tenor_saxophone_small` (VCSL `Aerophones/Reed Aerophones/Tenor Saxophone/Vibrato`,
19 notes, 1.2 MB; its file names sit an octave under the sounding pitch, which the pitch measure
corrects). The piano and the vibraphone leads play the core set's upright piano and vibraphone
already there (sections 1 and 2; the keys and mallets packs' grands and full vibraphone when
installed). The core set is 10.2 MiB of its 30 MiB with them.

## What the app gets from them

The pipeline keeps what each instrument needs and encodes it as Ogg Vorbis; nothing is retuned on
disk (the engine plays each sample at the rate that puts it at A = 432 Hz or 440 Hz; each sample's
recorded pitch, measured by the pipeline, is in its instrument's definition). The core set and the
packs, their sizes and checksums: `data/music/packs.ron`, and the summary in
[`docs/design/music.md`](../design/music.md) section 23.

## The 2026-11-13 batch: the research list, approved

**Added 13 November 2026** (downloaded 2026-11-13, 23:18 to 23:37 UTC). The operator, shown the
download list in
[`research/2026-11-13-instrument-libraries/README.md`](research/2026-11-13-instrument-libraries/README.md)
section 8: "Let's go for it and see what kind of procedural music we can make." and "Let's go with
your recommendations" (credit-only CC BY libraries allowed in OPTIONAL packs, never in the core set;
share-alike libraries left out). Exactly the eleven items below were downloaded, each from the source
and version the research names, each into its own new folder under `E:\HumanityOS\instruments-src\`
and logged in `download-log.txt` there (name, URL, version, time, bytes, SHA-256). Not TinySOL, not
anything else. Each archive was unpacked into an empty folder beside it and nothing inside was run.
The licence findings: [`findings/2026-11-13-free-instrument-libraries.md`](findings/2026-11-13-free-instrument-libraries.md).

How each was checked: every size against the research's figure (all within 1%) and GitHub's listed
size; the SHA-256 against GitHub's published digest where GitHub has one (the ukulele, the bagpipe,
the jazz guitar, Weresax, Virtuosity Drums: all matched); the three GitHub snapshots (LMMS, the
electric pianos, the steel pan) and AKWF file by file against the git blob hashes of the pinned
commit (every file identical); and each licence read from the download's own file after it arrived
and copied word for word into [`instrument-licences/`](instrument-licences/).

| Library | File (folder under instruments-src) | Bytes | SHA-256 |
|---|---|---|---|
| AKWF, the AKWF folder at commit `8de90bf` | `akwf-free/`: 4,358 WAVs and LICENSE.md, packed by us into `AKWF-FREE-8de90bf-AKWF-folder.tar` for the pipeline | 6,100,107 (the files); 9,135,104 (the tar) | tar `3046ac3c199158ef3cad8e82768cb4d623db47a6592f54285ee4e6b054c78dd4`; per-file list `manifest-8de90bf.txt` (`410db1d3...1ead71`) |
| Karoryfer Swirly Drums v1.104 | `karoryfer-swirly-drums/Swirly.Drums_1104.zip` | 868,594,745 | `dbeb1ad04052da1bada490ced8cc7d9fdd3b21ea90f7c6020269968b70f836c3` |
| Karoryfer Sneakybass v1.000 | `karoryfer-sneakybass/Sneakybass_v1.000.zip` | 340,462,855 | `df57ef935f2d006d83f898f615893cd2e9f93d00d01fb03921c54cd6dd5014dd` |
| FreePats Button Accordion HN 2024-03-29 | `freepats-button-accordion-hn/ButtonAccordionHN-SFZ+FLAC-20240329.7z` | 5,018,321 | `e5326a8830f0358faa195609fd21d0c1dbab899e39bd7f44b1590ebf2ce5035e` |
| FreePats Piano FB (Old Piano FB) 2020-04-01 | `freepats-old-piano-fb/PianoFB-SFZ+FLAC-20200401.7z` | 35,745,634 | `da35c93967c421b17f7219c12a37830ffd2b19f54f8a0a71203fc6161b079b45` |
| FreePats Ukulele 2026-08-11 | `freepats-ukulele/Ukulele-SFZ+FLAC-20260811.7z` | 2,402,413 | `d71b66382687175dca8aad53cad3290f6b418d48b6a2c0722af58b372f054bf4` |
| FreePats Bagpipe 2026-08-06 | `freepats-bagpipe/Bagpipe-SFZ+FLAC-20260806.7z` | 16,201,625 | `0cd9771ba6f2d48603b9163d94b0bcb4ca718950e6eab76cd9142e6f9a14e378` |
| FreePats FSBS Electric Guitar Clean #2 (Jazz) 2026-08-07 | `freepats-eguitar-fsbs-jazz/EGuitarFSBS-jazz-SFZ+FLAC-20260807.7z` | 65,919,794 | `c584f2e176575ba78c2ae8f45e04309badd0c912e654582880dba3146715c688` |
| FreePats Bass Guitar YR (finger) 2019-09-30 | `freepats-finger-bass-yr/FingerBassYR-SFZ+FLAC-20190930.7z` | 3,252,347 | `36f87ec9eb086ef25050312522aacf71213c582f46989e0c5a12681304596587` |
| LMMS assets, commit `bc09e07` | `lmms-assets/LMMS-assets-bc09e0745c69c73bd6c3112122031e4963ce3876.tar.gz` | 51,475,469 | `f7cfffd87a3dcb4e472372bce8d5615096164f8341f03d72a9c4bae76984feb4` |
| Karoryfer Weresax v1.003 | `karoryfer-weresax/Karoryfer.Weresax.v.1.003.zip` | 197,565,664 | `916dcb44b2ec8f03a3578c374dd1f1fdaa43e9e0264d921f65da2a339539d2ba` |
| Karoryfer Bear Sax v1.004 | `karoryfer-bear-sax/Karoryfer.Bear_Sax.v1.004.zip` | 131,505,201 | `66c28ea361c86943b2e2bcda6ca48cf105fec4f60a8f40e7c6bbf584be45c654` |
| Karoryfer Fashionbass v1.001 | `karoryfer-fashionbass/Karoryfer.Fashionbass.v1.001.zip` | 316,328,061 | `3beaa956b8ff97d9f0e7f31d263453fe113a2d07d4be99146b3590c8f5dd5442` |
| Virtuosity Drums v0.925 | `virtuosity-drums/Virtuosity_Drums_v0.925.zip` | 1,227,151,376 | `c6c5d0fe11a394e94be3146a950c3377ec102cb57d189d5a23cec26183d1963a` |
| Karoryfer War Tuba v1.002 | `karoryfer-war-tuba/Karoryfer_War_Tuba_v1002.zip` | 109,135,588 | `4f0b065c18b89111df4281df34c5120cd2d5b9a0c6b316835ec08d7891fd88d9` |
| jSteelDrum version 2, commit `dc15a36` | `jsteeldrum2/jlearman.SteelDrum-dc15a36ad69a43b0d240fcbd0cc78fbc61b2b47e.tar.gz` | 39,785,926 | `c6dc676c0dd80cc2db97f2f0e87828519d961f43afcf1f3aad6f58df07d97672` |
| epic-choir v1.0, soprano | `epic-choir/S/epic-choir-v1.0-S.zip` | 175,828,851 | `06943e75d638ccaf630eedff51cf177fe5bbd87b4ce7fe799c4db46b8df48209` |
| epic-choir v1.0, alto | `epic-choir/A/epic-choir-v1.0-A.zip` | 171,659,141 | `a0ab193e78f45a9e8925280bc19b6dfee215b87f968481d14fd3dcbb2d58b7cf` |
| epic-choir v1.0, tenor | `epic-choir/T/epic-choir-v1.0-T.zip` | 174,248,456 | `72a8cbe9e7ee8e99a49ad0e771c70b4bcd0b7ac6f23cfd75bf8376c03257a26f` |
| epic-choir v1.0, bass | `epic-choir/B/epic-choir-v1.0-B.zip` | 173,859,746 | `3f78e9b59018dc2ef45bb30d1b57cb68f22768d15e1f1c1a4f93d0b5e4b3002f` |
| Greg Sullivan's electric pianos, the SFZ conversion at commit `8c3e581` | `gregsullivan-e-pianos/GregSullivan.E-Pianos-8c3e581acda3594b553948ff0222d4f84a698376.tar.gz` | 19,254,201 | `e88ff3a3536fb559657c2ae4e09ae201e01b60f30f97dec5c4ca98a85f7f7554` |

Where they came from, and each licence as its own file words it:

- **AKWF** (Kristoffer Ekstrand): `https://raw.githubusercontent.com/KristofferKarlAxelEkstrand/AKWF-FREE/8de90bf94376670947369e69de0af6b9fbd19286/AKWF/...`,
  only the `AKWF` folder and the repository's `LICENSE.md` (the repository is over 800 MiB of the
  same waves in other products' formats). `LICENSE.md` is the CC0 1.0 Universal legal code; it opens
  "CC0 1.0 Universal" ([`akwf-free-LICENSE.md`](instrument-licences/akwf-free-LICENSE.md)).
- **Karoryfer** (Swirly Drums, Sneakybass, Weresax, Bear Sax, Fashionbass, War Tuba): the release
  assets of `github.com/sfzinstruments/karoryfer.<name>`. Swirly's `license`, and the `LICENSE` of
  Weresax, Bear Sax, Fashionbass and War Tuba, are the CC0 1.0 Universal legal code ("Creative
  Commons Legal Code / CC0 1.0 Universal"). **Sneakybass's zip has no licence file**: its readme says
  "Royalty-free for all commercial and non-commercial use. CC-0 license.", and the repository's
  `LICENSE` (read through GitHub's API on 2026-11-13) is the CC0 legal code; both are copied
  ([`karoryfer-sneakybass-readme.txt`](instrument-licences/karoryfer-sneakybass-readme.txt),
  [`karoryfer-sneakybass-repository-LICENSE.txt`](instrument-licences/karoryfer-sneakybass-repository-LICENSE.txt)),
  and the pack's licence text says so before the readme.
- **FreePats** (accordion, Old Piano FB, ukulele, bagpipe, jazz guitar, finger bass): the release
  assets the bank pages on `freepats.zenvoid.org` link (its robots.txt is absent: a 404). Each
  archive's `LICENSE.txt` is the CC0 1.0 legal code and each README ends "Published under the terms
  of Creative Commons CC0 public domain dedication" (the finger bass: "under the terms of the
  Creative Commons CC0 1.0 public domain dedication").
- **LMMS assets**: `https://codeload.github.com/LMMS/assets/tar.gz/bc09e07...` (the working tree, 49.4
  MiB; the repository with its history is 725 MiB). `LICENSE` is the CC0 1.0 legal code; the samples'
  README: "Samples must be CC0 (dedicated to the public domain)." and "Samples must be
  created/recorded by you."
- **Virtuosity Drums** (Versilian Studios): the v0.925 release asset of
  `github.com/sfzinstruments/virtuosity_drums`; `LICENSE` is the CC0 1.0 legal code.
- **jSteelDrum version 2** (Jeff Learman): `https://codeload.github.com/sfzinstruments/jlearman.SteelDrum/tar.gz/dc15a36...`;
  `LICENSE`: "This is free and unencumbered software released into the public domain." (the
  Unlicense).
- **epic-choir v1.0** (thepok), credited: the four release assets of `github.com/thepok/epic-choir`.
  The zips carry `ATTRIBUTION.txt`, not the licence; the repository's `LICENSE` (GitHub's API,
  2026-11-13): "Creative Commons Attribution 4.0 International (CC BY 4.0)" and "The samples, SFZ
  programs and demos of epic-choir are a derived work of VocalSet (Wilkins, J., Prem Seetharaman,
  Alison Wahl, & Bryan Pardo. (2018). VocalSet: A Singing Voice Dataset (Version 1.2) [Dataset].
  Zenodo. https://doi.org/10.5281/zenodo.1442513), licensed under CC BY 4.0, and are released under
  the same licence by thepok (2026)." `ATTRIBUTION.txt` asks for this credit: "Choir
  samples derived from VocalSet (Wilkins, Seetharaman, Wahl, Pardo 2018), CC BY 4.0,
  https://doi.org/10.5281/zenodo.1442513" ([`epic-choir-LICENSE.txt`](instrument-licences/epic-choir-LICENSE.txt),
  [`epic-choir-ATTRIBUTION.txt`](instrument-licences/epic-choir-ATTRIBUTION.txt)). The four zips sat
  in one folder at first; they were moved into `epic-choir/S`, `A`, `T` and `B` (bytes unchanged) so
  the pipeline can unpack each beside itself.
- **Greg Sullivan's electric pianos**, credited: the brief named `www.sullivang.net`. Its robots.txt
  returns the site's home page (no rules); the home page says: "Licensing (updated 21-Oct-2026) : All
  these instruments I host here now have Creative Commons Attribution 4.0 International licensing.
  (CC BY 4.0)". The site itself hosts Gigastudio files; its Conversions page names the SFZ
  conversion, "SFZ: https://github.com/sfzinstruments/GregSullivan.E-Pianos", which is the 18.8 MB
  source the research's download list names, so that is what was downloaded. Its own `LICENSE` is
  Creative Commons Attribution 3.0 Unported, and its README says the instruments were mapped by
  kinwie "with the author permission with the request for attribution." Both licences ask only for
  credit; the pack's licence text quotes the author's 4.0 grant first, then the conversion's 3.0 text
  ([`gregsullivan-e-pianos-LICENSE.txt`](instrument-licences/gregsullivan-e-pianos-LICENSE.txt),
  [`gregsullivan-e-pianos-README.md`](instrument-licences/gregsullivan-e-pianos-README.md)).

**What the app gets from them** (recipes in `scripts/instrument-recipes.json`; sizes and checksums
in `data/music/packs.ron`): the core set gains the accordion, the ukulele, the small electric bass, a
small jazz guitar, a dozen LMMS hits and risers, and 207 AKWF single cycles kept as plain PCM; six new
packs hold the rest: Band and jazz, Brushes and jazz drum kits, Folk and world, Synth waves, and the
two credited packs, Electric pianos and Choir (each credited on the Credits page, `data/credits.ron`,
and in its own licence line in Settings > Audio > Instruments). The summary table is in
[`docs/design/music.md`](../design/music.md) section 23.
