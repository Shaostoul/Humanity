# Free instrument libraries: which recordings HumanityOS may ship, and which it may not

**Research date: 13 November 2026.** Every licence below was read that day from the library's own
licence file, readme, SFZ header or page, never from memory. **This is not legal advice**: it is an
AI's reading of each licence, written down with the quotation that carries the meaning so a person (or
a lawyer) can check it. No sample, archive or soundfont was downloaded for this research.

It extends [`2026-11-06-music-sources.md`](2026-11-06-music-sources.md) (Part A, the first scan) and
[`2026-11-06-steel-pan-and-deep-drums.md`](2026-11-06-steel-pan-and-deep-drums.md), and corrects
them in four places (finding 14). The full per-library tables, every row with its quotation, are the
helper files of the research folder
[`../research/2026-11-13-instrument-libraries/`](../research/2026-11-13-instrument-libraries/README.md);
this file holds the licences that decide something.

## The question

Which free multi-sampled instrument libraries may HumanityOS ship inside the app (the core set) or offer
as downloadable packs that anyone may redistribute, commercially too, and does each require credit?

## Short answer

- **Public domain (CC0, the Unlicense), shippable anywhere, no credit needed:** the whole Karoryfer
  free catalogue **in its releases dated 2022-11-04 or later** (basses, a cello, a double bass, two
  saxophones, a tuba, an archtop and other guitars, a brushed drum kit and three other kits, the Osiris
  piano, synth sources), Virtuosity Drums (a jazz kit), most FreePats instruments (an accordion, a
  bagpipe, a ukulele, a honky-tonk piano, clean electric guitars and basses, a clarinet, a recorder,
  rendered FM pianos and organs), the AKWF single-cycle waveforms, the LMMS project's own sample
  library (drums, risers, vinyl effects), Sonic Pi's bundled samples (one loop excluded), jSteelDrum
  version 2, and small sets in the Discord SFZ GM Bank.
- **Credit required (CC BY), shippable with a credit line:** epic-choir, the first open choir of real
  quality (from the VocalSet singers); TinySOL (an IRCAM orchestra set with an accordion); Greg
  Sullivan's three electric pianos (now CC BY 4.0 on his own site); the Salamander and Headroom grand
  pianos; EtherealWinds Harp II CE; DrumGizmo's drum kits; MTG's saxophones.
- **Permissive grants without a named licence:** MuseScore General (MIT, keep its notices), the
  University of Iowa samples ("without restrictions", redistribution not addressed), Ethan Winer's
  collection, GeneralUser GS (its author is unsure where all its samples came from).
- **Not shippable:** every free drum-machine sample set found (none grants redistribution; the
  "free" sample sites allow use in your music only), NonCommercial sets (jRhodes, Good-sounds),
  share-alike sets for a CC0 music library (AVL drumkits, the Salamander drumkit, Lars Palo's organs),
  CC Sampling Plus (Sonatina, G-Town Church), shop EULAs, and every set with no stated licence (SM
  Drums, Dirt-Samples, tidal-drum-machines, Splendid Grand until Akai's own statement is found).
- **Corrections:** No Budget Orchestra is a mix of licences, not CC BY-SA 4.0; Karoryfer's older
  releases carry CC BY 4.0 (one GPL 3); Greg Sullivan now says CC BY 4.0; Shinyguitar was never GPL.

## The findings

Licence classes used throughout: **GREEN** (CC0, public domain, the Unlicense, MIT-style on the
samples), **CREDIT** (CC BY: shippable with credit), **SHARE-ALIKE** (CC BY-SA, GPL on samples, with
or without a "sound exception"), **REJECT** (NonCommercial, NoDerivatives, Sampling Plus, no
redistribution, a shop EULA, or no licence at all).

### 1. Karoryfer Samples: CC0 for every free library, from 2022-11-04

- Source: https://shop.karoryfer.com/pages/free-samples (no page date; footer "© 2026"):
  "All our free sample libraries are under a Creative Commons Zero license." and "Older downloads may
  include a less permissive license, but we changed that and now they are all CC0." The exception:
  "Only Marie Ork has a different license".
- The repositories' history (GitHub REST API, `gh api`, commit history of each repository in
  github.com/sfzinstruments): every pre-2022 Karoryfer library got a licence file holding
  "Attribution 4.0 International" (CC BY 4.0) on **2019-05-02**; Caveman Cosmonaut instead held
  "GNU GENERAL PUBLIC LICENSE / Version 3, 29 June 2007" from **2019-12-28**. On **2022-11-04** each was
  replaced by a `LICENSE` beginning "Creative Commons Legal Code / CC0 1.0 Universal" and a new
  release was published the same day. **So only releases dated 2022-11-04 or later are CC0**; the
  2021 v1.000 assets are still listed on GitHub. Libraries first published later (Horse Pulse, the
  2023 drum kits, Black And Blue Basses, Big Little Bass, Osiris Piano, the erhu, the cithara) were CC0
  from their first licence file.
- Karoryfer's readmes add "Royalty-free for all commercial and non-commercial use." beside the CC0
  file.
- Checked again on 2026-11-13 through the API: `sfzinstruments/karoryfer.swirly-drums` (CC0-1.0, last
  pushed 2023-04-29), `karoryfer.fashionbass` (CC0-1.0, pushed 2022-11-04), `Osiris_Piano` (CC0-1.0,
  pushed 2026-07-04; its product page: "Open source, released under CC-0 license."),
  `virtuosity_drums` (CC0-1.0, pushed 2026-04-21), each licence file beginning "Creative Commons Legal
  Code / CC0 1.0 Universal".
- Virtuosity Drums' own page, https://versilian-studios.com/virtuosity-drums/ (modified 2026-05-11):
  "Virtuosity Drums is an open source (Creative Commons 0) contemporary Jazz drum kit library,
  performed with sticks".
- **Shinyguitar** was CC BY 4.0 from 2019-05-02 to 2022-11-04, then CC0; no GPL text appears in its
  history (the GPL 3 an old listing mentioned was Caveman Cosmonaut's).
- **Marie Ork**: her page (shop.karoryfer.com/pages/free-marie-ork) states no licence; it is inside a
  652 MB zip, not read. It is a Plogue Alter/Ego voice bank the engine cannot play in any case.
- **Weresax**: the free list says "Tenor sax samples."; its readme and product page say alto.
- Class: **GREEN** (releases of 2022-11-04 or later).

### 2. The Discord SFZ GM Bank (github.com/sfzinstruments/Discord-SFZ-GM-Bank)

Read through `gh api` (the `.sfz` headers, committed 2021-10-13 unless said). Of its 138 slot files,
11 are real instruments and 127 are 30-byte placeholders.

- 002 Bright Acoustic Piano (the VCSL Yamaha upright): "License: Creative Commons CC0, Versilian
  Studios". **GREEN.**
- 034 Electric Bass (finger), a subset of Karoryfer's Fashionbass: "License: CC0". **GREEN.**
- 045 Tremolo Strings and 046 Pizzicato Strings (Versilian mixes of VCSL and VSCO 2): "License:
  Creative Commons CC0" (2020-10-19). **GREEN.**
- 105 Sitar: "License: CC0, Dr. Narayan Bhagawan Raikar"; its readme: "Will need testing for quality
  etc." **GREEN.** 112 Shanai: readme "Shanai contributed by Dr. Narayan Bhagawan Raikar under CC 0
  license." **GREEN.**
- 003 Electric Grand Piano (Greg Sullivan's CP80): the header says "License: Creative Commons CC0,
  Greg Sullivan", but the source repository is CC BY 3.0 "with the request for attribution", and the
  header was written by the bank's mapper, not by Greg Sullivan. **Treat as CREDIT.**
- 005 Electric Piano 1 (Jeff Learman's jRhodes): his own header says "License: Creative Commons CC0,
  Jeff Learman", but his main jRhodes repositories say NonCommercial (finding 3). **Conflict: ask the
  author before shipping.**
- Drums 001 Standard Kit: no licence line; its readme lists freesound.org among its sources and file
  names carry freesound uploaders' names. **REJECT.** 110 Bagpipe: no licence line, unmapped, and the
  bank's own plan marks it "OMIT". **REJECT until confirmed.**

### 3. The rest of the SFZ Instruments organisation

- **Headroom Piano** (Bengt Nilsson): LICENSE "Attribution 4.0 International" (2020-05-28); readme:
  mapped "with the author's permission as long keeping the original instrument name". **CREDIT** (keep
  the name).
- **Salamander Grand Piano** (Alexander Holm): readme "This work is licensed under a Creative Commons
  Attribution 3.0 Unported License." (LICENSE first committed 2020-05-14). **CREDIT**, as settled.
- **Greg Sullivan E-Pianos** (Yamaha CP80, Hohner Pianet T, Wurlitzer EP200): the repository's LICENSE
  is "Attribution 3.0 Unported" (2020-05-24). His own site, https://www.sullivang.net/ (read and
  re-read 2026-11-13; the bare sullivang.net is now a parked page): "Licensing (updated 21-Oct-2026) :
  All these instruments I host here now have Creative Commons Attribution 4.0 International licensing.
  (CC BY 4.0)". **CREDIT.**
- **jSteelDrum version 2** (github.com/jlearman/jlearman.SteelDrum, pushed 2026-09-01): LICENSE "This
  is free and unencumbered software released into the public domain." (the Unlicense). **GREEN.**
- **Ethan Winer Collection** (no licence in the repository): the author's page
  https://ethanwiner.com/ewsf2.html (no date): "These files are made available for you to use in any
  way you'd like, royalty free," "including for commercial projects, with no attribution required."
  **GREEN by an unnamed grant.**
- **MTG Solo Sax**: LICENSE "Attribution 4.0 International" (2020-09-19), but its samples came from
  freesound packs whose own licence could not be read (Sources not reached, below). **CREDIT, provenance open.**
- **Splendid Grand Piano**: no LICENSE; the porter's readme: "This samples set was released as public
  domain in early 2000 by Akai company." No statement from Akai was found. **Not shippable until one
  is.**
- Rejected, with the words: **Maestro Concert Grand** ("All rights reserved." and "You may not modify
  and spread this soundfont without the author´s written permission."); **jRhodes3c** ("the jRhodes
  samples are licensed under CC BY-NC-SA 4.0") and **jRhodes3d** ("License for samples: CC BY-NC");
  **Kay 5-String Banjo** ("This sample is released under the GNU GPL license.", SHARE-ALIKE);
  **Project16 Rickenbacker 4001** ("Attribution-NonCommercial-ShareAlike 3.0 Unported (CC BY-NC-SA
  3.0), modified"); **Starbirth Kudu Shofar** ("you MAY NOT sell the samples, or include them in a
  compilation for samples for sale."); **Sam's Sonor** ("Attribution-ShareAlike 4.0 International",
  SHARE-ALIKE); **G-Town Church Sampling Project** ("licensed under the Creative Commons Sampling Plus
  1.0 license"); **SM Drums** (no licence in the repository or on its authors' pages; the catalogue's
  "Public Domain" label has no source); and the no-licence conversions (Clavecin, Orgue Eglise,
  Krumhorn, Terkelsen's Marimba and Mandolin, the Tic Tok Men drum sets, Damien's Funky Guitar, Ovation
  Guitar, Picked Lapharp).

### 4. FreePats

- Source: each bank's page under https://freepats.zenvoid.org (no robots.txt; pages undated, each bank
  carries a version date). Unless said otherwise the bank's line is: "Published under the terms of the
  Creative Commons CC0 1.0 public domain dedication." **GREEN.**
- Banks of note (version date): Old Piano FB, a honky-tonk upright (2020-04-01); Button Accordion HN
  (2024-03-29); Bagpipe in G with two drones (2026-08-06); Ukulele (2026-08-11); FSBS Electric Guitar
  Clean #1 and #2 (2026-08-07); Bass Guitar YR ("sent for inclusion in FreePats on September 2019, under
  the terms of the Creative Commons CC0 1.0 public domain dedication.", 2019-09-30); Clarinet
  (2019-08-18); Wooden Recorder (2020-12-05); Ocarina (2024-10-02); Jaw Harp (2020-06-06); FM
  Synthesized Pianos 1 and 2 (rendered from the free Hexter DX7 emulator); drawbar, rock, percussive
  and church organ emulations (rendered); world percussion (2020-09-05); twenty synth banks.
- **CREDIT:** YDP Grand Piano and the Salamander Grand ("Creative Commons Attribution 3.0 license."),
  MuldjordKit ("Creative Commons Attribution 4.0 license.").
- **SHARE-ALIKE:** the FreePats General MIDI set and its percussion set, and FSS Steel-String ("GNU
  General Public License version 3 or later, with a special exception."); ColomboADK ("License: GNU
  GPL v2 or later + exception"). The licences page, https://freepats.zenvoid.org/licenses.html (no
  date), says the GPL with exception "has been deprecated and it won't be accepted anymore". The
  exception covers a composition made with the sounds; shipping the samples in an app is still GPL
  distribution.

### 5. University of Iowa Electronic Music Studios, Musical Instrument Samples

- Source: https://theremin.music.uiowa.edu/MIS.html (Last-Modified 2017-12-23): "Since 1997, these
  recordings have been freely available on this website and may be downloaded and used for any
  projects, without restrictions."
- No page names a licence or speaks of redistributing the samples themselves. **GREEN by an unnamed
  grant**, as settled; nothing newer found.

### 6. Orchestral sets

- **TinySOL** (IRCAM's Studio On Line subset), https://zenodo.org/records/3685367 ("Published January
  31, 2020 | Version 6.0", re-read 2026-11-13): "TinySOL is offered free of charge under the terms of
  the Creative Commons Attribution 4.0 International (CC BY 4.0) license" and "The Creative Commons
  Attribution license allows re-distribution and re-use of a licensed work". 14 instruments (tuba,
  horn, trombone, trumpet, accordion, double bass, violin, viola, cello, bassoon, clarinet, flute,
  oboe, alto saxophone), 1.0 GB. The OrchideaSOL paper calls the set non-commercial; the Zenodo record
  is the licensor's own statement. **CREDIT.**
- **No Budget Orchestra** (github.com/ssj71/No-Budget-Orchestra, the repository the Virtual Playing
  Orchestra cites), root `license.txt` (committed 2016-03-11), re-read 2026-11-13: "1.You may not use
  it for commercial sample libraries (put it on a CD and sell it, sell it online, etc)". Its folders
  carry their own terms: CC BY-SA 4.0 in some, Sampling Plus in others, "CC BY-NC" in three, and about
  55 folders (its Choir, Accordion and Harp among them) with no licence file. **REJECT as a whole.**
- **Sonatina Symphonic Orchestra**, maintained at github.com/peastman/sso (pushed 2026-08-22): "It
  may be used and distributed / under the terms of the Creative Commons Sampling Plus 1.0 license". No
  re-licensed fork exists. **REJECT**, as settled.
- **Virtual Playing Orchestra**, http://virtualplaying.com/virtual-playing-orchestra/ (version 3.2.4
  dated 25 September 2022): "different licenses apply to different parts of the library". **REJECT as
  a bundle**, as settled.
- **EtherealWinds Harp II Community Edition**, https://versilian-studios.com/etherealwinds-harp/
  (Last-Modified 2026-11-13): "we put together this reduced version of the new library under a
  Creative Commons Attribution 4.0 International license." **CREDIT.**
- **VCSL Keys**, https://versilian-studios.com/vcsl-keys/: "VCSL Keys is under a Creative Commons 0
  (CC0) license, as a derivative of VCSL." Already on disk as VCSL.
- **Good-sounds** (MTG), https://zenodo.org/records/4588740 (Published June 29, 2017): "licensed under
  the Creative Commons Attribution-NonCommercial 4.0 International License." **REJECT.**
- **NSynth** (Google Magenta): "made available by Google Inc. under a Creative Commons Attribution 4.0
  International (CC BY 4.0) license", but its notes come from commercial sample libraries at 16 kHz.
  **CREDIT on paper, not recommended.**
- **Versilian's paid sample packs**: "direct redistribution/sharing of samples is strictly
  prohibited." **REJECT.**

### 7. Voices and choirs

- **epic-choir v1.0**, https://github.com/thepok/epic-choir (release v1.0, 2026-10-24; four zips,
  695,596,194 bytes in all), LICENSE re-read 2026-11-13: "Creative Commons Attribution 4.0
  International (CC BY 4.0)" and "The samples, SFZ programs and demos of epic-choir are a derived work
  of VocalSet", "licensed under CC BY 4.0, and are released under the same licence by thepok (2026)."
  Soprano, alto, tenor and bass, five vowels, three dynamics, 3 to 10 real singers summed per sample.
  **CREDIT** (to thepok and to VocalSet's authors: Wilkins, Seetharaman, Wahl, Pardo, 2018).
- **VocalSet 1.2**, https://zenodo.org/records/1442513 (Published March 8, 2018): licence "Creative
  Commons Attribution 4.0 International". 20 professional singers. **CREDIT.**
- **OLPC / Berklee sample library** (voices in Berklee44v5), Berklee's page
  college.berklee.edu/node/1030448 (no date): "also offered to all computer musicians under the
  Creative Commons Attribution License."; archive.org lists CC BY 3.0. One-shots, not a playable choir.
  **CREDIT.**
- **Karoryfer's legato vocal tutorial** (one "a" vowel, CC0, 2025-09-09) and **272 Merry Orks**
  (female death-metal vocals, CC0). **GREEN.**
- **Decent Samples' Dave Choir**: "You may not redistribute any of the samples or sample libraries on
  their own, unless you have been given written permission by Decidedly, LLC." **REJECT.**
- No CC0 choir of real quality exists in what was searched.

### 8. General MIDI soundfonts

- **MuseScore General**, MuseScore_General_License.md on the official mirror
  ftp.osuosl.org/pub/musescore/soundfont/MuseScore_General/ (Last-Modified 2020-07-10):
  "MuseScore_General.sf2 is shared under the MIT license as described in COPYING, as was FluidR3Mono
  and FluidR3 before it." and "The acknowledgements and copyright notices above must be included in any
  derivative work." **GREEN (MIT, keep the notices).** Settles the 2026-11-06 "not settled".
- **GeneralUser GS 2.0.3**, `documentation/LICENSE.txt` in github.com/mrbumpy409/GeneralUser-GS
  (committed 2026-02-23): "Please feel free to use it in your software projects, and to modify the
  SoundFont bank or its packaging to suit your needs." and "I cannot be 100% sure where all of the
  samples originated". **A permissive grant with self-declared uncertain provenance: not for the
  app.** Settles the 2026-11-06 "not settled". (Musical Artifacts files it as "public", which its
  author's licence does not say.)

### 9. Drum kits beyond Karoryfer

- **DrumGizmo** (https://drumgizmo.org/wiki, robots.txt allows the wiki): DRSKit "released under the
  Creative Commons Attribution 4.0 International License." with "any context in which other credits
  (e.g. to performers) are presented must include an attribution." (last modified 2021-03-22);
  MuldjordKit's attribution "must be made in the album credits (or similar album liner notes)"; Crocell,
  Aasimonster, ShittyKit the same licence. **CREDIT.**
- **LMMS assets** (github.com/LMMS/assets, CC0-1.0, last pushed 2021-08-24): README "LMMS will only
  accept `CC0` licenced content."; its Samples readme: "Samples must be created/recorded by you."
  49.4 MiB in all, 379 drum one-shots and a set of risers, sweeps and vinyl effects. **GREEN.** (The
  LMMS application's older factory samples are not this: finding 10.)
- **Shortcircuit XT factory content** (github.com/surge-synthesizer/shortcircuit-factory-content):
  README "By submitting your samples here you are making them available under CC0 in perpituity."
  [sic]; no LICENSE file. Includes Andreyas Drums (67 MiB, five dynamics, four round robins). **GREEN by
  its contribution terms.**
- **AVL Drumkits** (Glen MacArthur), x42/avldrums.lv2 `sf2/README`: "CC-BY-SA with the following
  exception: The by-sa condition is imperative if you modify the samples themselves or create new sample
  libraries". **SHARE-ALIKE.** **Salamander Drumkit**: "Licence: [CC BY-SA]" with share-alike "only
  applies if you modify the samples themselves or create new sample libraries with them."
  **SHARE-ALIKE** (unlike the CC BY Salamander piano).
- **Orange Tree Jazz Funk kit** (in Hydrogen's feed): "You may not copy, edit, distribute or sell the
  original soundsets without the written permission of Orange Tree Samples." **REJECT.** Hydrogen's
  other kits are a mix of GPL, CC BY-SA, CC BY and none.

### 10. Drum machines: no sample set is shippable

- **Sonic Pi**, LICENSE.md (commit 2026-09-23): "All the bundled samples are individually licensed
  under a CC0 1.0 Universal Public Domain Dedication." **GREEN**, but `loop_amen` and `loop_amen_full`
  are excluded: the Amen break is a recording whose owner is named (the genre finding of 2026-11-12, and
  Wikipedia's "Amen break"), not the uploader's to dedicate.
- **TidalCycles Dirt-Samples**: no licence; its author, issue 19 (2020-04-30): "A lot of them have
  unknown provenance." **REJECT.**
- **tidal-drum-machines** (Strudel's drum machines): no licence; its maintainer, issue 2
  (2026-10-26): "it's practically impossible to 100% verify". **REJECT.**
- **LMMS factory samples**: a maintainer, issue 5126 (2019-08-15): "all of LMMS's stock samples are
  questionably legal." **REJECT.**
- **Michael Fischer's TR-808 set** (1994): "these samples are ABSOLUTELY FREE!"; no licence granted.
  **REJECT.**
- **"Free" sample sites, use in your music only:** MusicRadar SampleRadar ("all we ask is that you
  don't re-distribute them.", 2025-08-12); 99Sounds ("You do not have the right to sell or redistribute
  (even for free) the audio files", licence page modified 2023-07-25); Samples From Mars ("you may not
  use these sounds in any website, application, or software."); Legowelt ("free to download and use in
  your productions", no redistribution grant); Wave Alchemy (no licence; "all our intellectual property
  rights are reserved."). **REJECT.**
- **SHLD Music** (drolez.com, updated 2024-05-09): "100% royalty-free , licensed under CC0-1.0". Its
  Pro-1 and Minifreak sets are recordings of analog synths: **GREEN**. Its Alesis SR-16 and Korg
  Wavestate sets are recordings of the makers' own sample ROMs, which a CC0 from the recordist cannot
  cover: **doubtful, leave out.**

### 11. Are recordings of a drum machine's sounds protected?

- US Copyright Office, Circular 56 (revised 03/2021): "Short sound recordings may lack a sufficient
  amount of authorship to warrant copyright protection". "May lack" is guidance, not a rule.
- *VMG Salsoul v. Ciccone* (9th Cir., 2016-06-02) holds the "de minimis" exception applies to sound
  recordings, against *Bridgeport* (6th Cir. 2005): "any unauthorized copying--no matter how
  trivial--constitutes infringement." Both concern a sample inside a new work, never redistributing
  whole sample files.
- EU, CJEU press release 98/19 (29 July 2019), *Pelham*: "the reproduction by a user of a sound sample,
  even if very short, taken from a phonogram must, in principle, be regarded as a reproduction `in
  part' of that phonogram".
- Roland, SP-404MKII Reference Manual v5.50: "The copyright of content in this product (the sound
  waveform data, style data, accompaniment patterns, phrase data, audio loops and image data) is
  reserved by Roland Corporation."
- No Roland statement about recordings of its analog machines (TR-808, TR-606, CR-78) and no case about
  drum-machine one-shots as such were found. So a drum-machine pack is safe only with an explicit
  licence from the recordist, and none of the classic sets has one. The engine synthesises these
  sounds from published methods instead.

### 12. Synth sources and wavetables

- **AKWF (Adventure Kid Waveforms)**, github.com/KristofferKarlAxelEkstrand/AKWF-FREE (head commit
  `8de90bf94376670947369e69de0af6b9fbd19286`, 2025-12-04): LICENSE.md "CC0 1.0 Universal" (committed
  2018-09-09). Its `AKWF` folder holds 4,358 single-cycle waves (600 samples, 16-bit, 44.1 kHz,
  mono), 5.81 MiB; the other folders are the same waves prepared for other products (the repository
  is over 800 MiB with them). **GREEN.**
- **Malaclypse the Younger's instruments** (catalogue, newest entry 2026-09-19): "The MIT license is
  used so that anyone distributing changes is forced to take credit for them". The Clap, 4.5 MB.
  **GREEN (MIT, keep the notice).**
- **Karoryfer Cowsynth, Caveman Cosmonaut, String Cyborgs**: CC0 (finding 1).
- **Surge XT factory wavetables**: inside a GPL-3.0 repository, no separate data licence. **SHARE-ALIKE.**

### 13. Sound effects for music

- **LMMS assets SFX** (CC0, finding 9): down and up sweeps, a riser, a sub drop, a noise blast, vinyl
  backspin and rewind, a cymbal swell. **GREEN.**
- **BigSoundBank**, https://bigsoundbank.com/licenses.html (no date): "Share, copy, distribute and
  communicate the material by all means and in all formats." under "Creative Commons CC0 1.0 Universal"
  for sounds marked free. **GREEN, per sound.**
- **BBC Sound Effects**, RemArc terms (2016-09-27): "For non-commercial, personal or research
  purposes". **REJECT.** **Mixkit**: users must not "make Mixkit or any item available to any third
  party" (mixkit.co/llm-info, 2026-11-12). **REJECT.**

### 14. Corrections to the earlier findings

1. **No Budget Orchestra** was recorded on 2026-11-06 as "CC BY-SA 4.0, per a secondary page". Its own
   files show a mix, including NonCommercial and no licence (finding 6). It stays unshippable, for a
   different reason.
2. **Karoryfer's "older downloads"** are now dated: CC BY 4.0 from 2019-05-02 (GPL 3 for Caveman
   Cosmonaut from 2019-12-28) until 2022-11-04 (finding 1).
3. **Greg Sullivan's electric pianos** moved from CC BY 3.0 to CC BY 4.0 on 21 October 2026 (finding 3).
4. **Shinyguitar** was never GPL in its repository history (finding 1), which answers the 2026-11-12 note
   in `docs/design/music.md` ("an old KVR listing says GPL 3"); the only GPL 3 in Karoryfer's
   repositories was Caveman Cosmonaut's. The KVR listing itself was not found.
5. Newer than the settled list: **jSteelDrum version 2** is the Unlicense (GREEN).

## What is still unknown, and what would settle it

- **Iowa's samples, redistributed:** the terms say "used for any projects, without restrictions" and
  nothing about redistribution. One e-mail to the Electronic Music Studios would settle it; not needed
  while VSCO 2, VCSL and TinySOL cover the same instruments.
- **jRhodes in the GM bank** (CC0 header against NonCommercial elsewhere): a message to Jeff Learman.
- **Splendid Grand Piano**: an Akai statement of the 2000 release, if one exists in an archive of
  Akai's site.
- **epic-choir's provenance** rests on its LICENSE and VocalSet's Zenodo record, both read; it is three
  weeks old with one author. Listening to it and reading its ATTRIBUTION.txt on download settles the
  rest.
- **TinySOL**: the Zenodo record says CC BY 4.0, a paper by its authors says non-commercial. The
  record is the licensor's statement; a question to IRCAM would end the doubt.
- **Inside the archives** (Karoryfer zips, SM Drums, Ivy Audio's Piano in 162, Marie Ork): each
  download's own licence file is read on arrival, as the 2026-11-07 downloads were.
- **Freesound-sourced sets** (MTG Solo Sax, Sonic Pi's per-file sources, the Euterpea harmonium): a
  person reading each freesound page; automated reading is barred there.

## Sources not reached

- freesound.org (MTG packs 20251, 20239, 20247, 20253; Sonic Pi's per-sound sources; the Euterpea
  harmonium, sound 330410): robots.txt bars automated reading. kenney.nl: robots.txt bars Claude.
- opengameart.org, pixabay.com, goldbaby.co.nz, bandshed.net (AVL Drumkits, No Budget Orchestra's
  samples), signaturesounds.org, lists.linuxaudio.org (Holm's 2010 Salamander posts),
  mattiaswestlund.net (parked), codeberg.org: robots.txt bars Claude or every reader. Two of these
  (bandshed.net's No Budget Orchestra index and signaturesounds.org's about page) were fetched once in
  the same command as their robots.txt before the rule was seen; nothing from either is used here.
- sonniss.com, gdc.sonniss.com, zapsplat.com's licence page, bedroomproducersblog.com: HTTP 403.
- sound-effects.bbcrewind.co.uk/licensing and mixkit.co/license: script-only pages (the RemArc PDF and
  mixkit.co/llm-info were read instead).
- eur-lex.europa.eu (the Pelham judgment): robots.txt bars every reader; the CJEU press release was read.
- wiki.laptop.org (the OLPC index): 401. luciphercode.wixsite.com/sofia-mz (Sofia MZ pianos): 404,
  withdrawn by their author. libreorchestra.net: dead. hydrogen-music.org: certificate mismatch (its
  feed was read through GitHub).
- The KVR listing that called Shinyguitar GPL 3: not found.

## What it means for this project

### Certain

- Any Karoryfer free library may ship in the app or a pack, with no credit required, **if it comes
  from a release dated 2022-11-04 or later** and its own LICENSE file says CC0 on arrival.
- AKWF's `AKWF` folder (5.81 MiB), the LMMS assets library (49.4 MiB), the FreePats CC0 banks,
  Virtuosity Drums and jSteelDrum version 2 may ship anywhere.
- epic-choir, TinySOL, Greg Sullivan's pianos, the Salamander and Headroom pianos, EtherealWinds
  Harp II CE and DrumGizmo's kits may ship only with their credit shown (the app's credits table,
  `crates/humanity-core/src/credits.rs`, already fails a test when a source marked
  `attribution_required` is not credited).
- No free drum-machine sample set found may be redistributed; drum-machine sounds come from synthesis.
- NonCommercial, Sampling Plus, no-licence and shop-EULA sets stay out.

### Judgement calls (an AI's reading; reject freely)

- Keep the core set public domain only, and put CC BY instruments in optional packs, each credited in
  the pack's licence text and the credits table.
- Leave out share-alike sets (AVL, the Salamander drumkit, Lars Palo's organs, FreePats' GPL sets):
  the project has carried share-alike data (the star catalogues), but the music library has stayed
  CC0, and CC0 and CC BY sets cover the same instruments.
- Treat unnamed permissive grants (Iowa, Ethan Winer) as usable but second choice, and GeneralUser
  GS as unusable while its author doubts its origins.

**Not legal advice.** This is an AI's reading of licences quoted above, dated 13 November 2026; a
licence can change after that date, so re-read the source before relying on it.

## Downloaded (addendum, 13 November 2026)

The operator approved the research's download list the same day ("Let's go for it and see what kind
of procedural music we can make." and "Let's go with your recommendations": CC BY only in optional
packs, never the core set; share-alike left out). Each library below was downloaded on 2026-11-13
(23:18 to 23:37 UTC) and its licence read again from the download's own file; the sizes, checksums and
word-for-word licence copies are in [`../instrument-sources.md`](../instrument-sources.md) ("The
2026-11-13 batch"). The body above is unchanged; these notes only record what arrived.

- **AKWF** (section 12): the `AKWF` folder at commit `8de90bf`, 4,358 files, 6,100,107 bytes, every
  file matching the commit's git blob; `LICENSE.md` is CC0 1.0, as found. Core set (207 waves) and the
  Synth waves pack.
- **Karoryfer Swirly Drums v1.104** (section 1): `license` is CC0 1.0, as found. Brushes and jazz drum
  kits pack.
- **Karoryfer Sneakybass v1.000** (section 1): the release zip carries no licence file, only a readme
  line, "CC-0 license."; the repository's `LICENSE` is CC0 1.0. Recorded, not a change of licence.
  Band and jazz pack.
- **FreePats Button Accordion HN, Old Piano FB, Ukulele, Bagpipe, FSBS Clean #2 (Jazz), Bass Guitar
  YR (finger)** (section 4): each archive's `LICENSE.txt` is CC0 1.0, as found. The accordion,
  ukulele, finger bass and a small jazz guitar in the core set; the rest in the Band and jazz and Folk
  and world packs.
- **LMMS assets, commit `bc09e07`** (sections 9 and 13): `LICENSE` is CC0 1.0, as found. Twelve hits
  and risers in the core set.
- **Karoryfer Weresax v1.003 and Bear Sax v1.004** (section 1): CC0 1.0, as found. Band and jazz pack.
- **Karoryfer Fashionbass v1.001** (section 1): CC0 1.0, as found. Band and jazz pack.
- **Virtuosity Drums v0.925** (section 1): CC0 1.0, as found. Brushes and jazz drum kits pack.
- **Karoryfer War Tuba v1.002 and jSteelDrum version 2** (sections 1 and 3): CC0 1.0 and the
  Unlicense, as found. Band and jazz pack; Folk and world pack.
- **epic-choir v1.0** (section 7), credited: the zips carry `ATTRIBUTION.txt`; the repository's
  `LICENSE` is CC BY 4.0, as found. Choir pack, credited on the Credits page (`data/credits.ron`, row
  `epic_choir`) and in the pack's licence line.
- **Greg Sullivan's electric pianos** (section 3), credited: the author's site still says CC BY 4.0
  ("Licensing (updated 21-Oct-2026)"), and its Conversions page links the SFZ conversion that was
  downloaded, whose own `LICENSE` is CC BY 3.0 Unported. Both ask only for credit. Electric pianos
  pack, credited (row `greg_sullivan_epianos`).

Nothing was stopped: no source differed from the research in licence, size (all within 1%) or content.
