# Free instrument libraries, part 2: FreePats, the Iowa samples, pianos, orchestral, Musical Artifacts, voices, world instruments and free reeds

> Part 2 of the free instrument library research of **13 November 2026**: the plain-words summary, the
> ranking and the recommendation are in [README.md](README.md); the licences that matter, with their
> quotations, are in the dated finding [2026-11-13-free-instrument-libraries.md](../../findings/2026-11-13-free-instrument-libraries.md). This is an AI's
> reading of each licence, not legal advice: every licence is quoted so a reader can check it. Gathered by
> four research passes on 2026-11-13 and spot-checked the same day (licence files re-read through the
> GitHub API, the TinySOL Zenodo page, Greg Sullivan's site).

Researched 2026-11-13. Pages read with a plain `research tool` user agent (node fetch) or WebSearch result pages; GitHub only through `gh api`; archive.org only through its metadata API and zip listing pages; Musical Artifacts only through its JSON listing at 6 s per request (its robots.txt says Crawl-delay 5). Nothing downloaded. "Already on disk" = VCSL / VSCO 2 CE content the project holds.

Size notation: FreePats sizes are "compressed (unpacked)" as the page prints them. GitHub sizes are the repository size from the API (KB, includes history).

---------------------------------------------------------------------------------------------------

## 1. FreePats (https://freepats.zenvoid.org)

robots.txt: 404 (2026-11-13), so all 36 instrument pages and the licences page were read. FreePats pages carry no page date; each bank has its own "Version" date, used as the source date below. Unless the row says otherwise, the licence line on the bank's own page is exactly: **"Published under the terms of the Creative Commons CC0 1.0 public domain dedication."** (written "CC0 line" below).

Licence policy, https://freepats.zenvoid.org/licenses.html (no date): CC0 preferred ("you are encouraged to contribute content to the FreePats project under CC0 license"); CC BY 4.0/3.0 accepted; GPL + exception "has been deprecated and it won't be accepted anymore". The exception only says a composition using the sounds is not "covered by the GNU General Public License"; shipping the samples inside an app is still GPL distribution, so GPL+exception = SHARE-ALIKE for us.

GitHub mirrors (`gh api orgs/freepats/repos`, 2026-11-13): 42 instrument repos; GitHub licence id CC0-1.0 on all except `muldjordkit` (CC-BY-4.0) and `colomboADK` (GPL-2.0). Repos exist for every CC0 bank below except Kalimba, Jaw Harp, Clarinet, Recorder, Tenor Sax, Xylophone, Concert Harp, YDP, Salamander, Church/Drawbar organ emulations (site-only).

### 1a. FreePats keys, organs, free reeds
| Library | Publisher/author | Licence (exact quote; where read; source date) | Class | Size | Samples, dynamics, round robins | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| Upright Piano KW | Gonzalo & Roberto (FreePats) | CC0 line; bank page; v2022-02-21 | GREEN (already on disk) | 32 MiB SFZ+FLAC; small 2.9 MiB | 2 velocity layers, stereo; bass loops | own page: "Created from a Kawai upright piano located in a living room", Zoom H1 | freepats.zenvoid.org/Piano/acoustic-grand-piano.html |
| Old Piano FB (honky-tonk) | Piotr Barcz | CC0 line; bank page; v2020-04-01 | GREEN | 34 MiB SFZ+FLAC (35); small 6.2 MiB | stereo, Zoom H1n; "Best quality" + "Small size" | own page: "a wonderful historic piano with an authentic honky-tonk tone" (Francis Bacon player piano) | freepats.zenvoid.org/Piano/honky-tonk-piano.html |
| YDP Grand Piano | Zenph Studios / OLPC samples, FreePats SF2 | "Creative Commons Attribution 3.0 license."; bank page; v2016-08-04 | CREDIT | 36 MiB SF2 only | Disklavier Pro, computer-performed | from OLPC sound library donated to Dr. Richard Boulanger | same page |
| Salamander Grand Piano V3 | Alexander Holm | "Creative Commons Attribution 3.0 license."; bank page; vV3+2020-06-02 | CREDIT | 707 MiB SFZ+FLAC 48/24; 394 MiB 44/16 WAV; 296 MiB SF2 | Yamaha C5, 16 velocity layers, minor thirds, hammer-noise and string-resonance releases | own page: "two AKG c414 disposed in an AB position" | same page |
| FM Synthesized Piano #1 | Roberto (Hexter, DX7 "E. Piano 1") | CC0 line; v2019-09-16 | GREEN | 24 MiB SFZ+FLAC | synthesized | rendered from the free Hexter DX7 emulator | freepats.zenvoid.org/ElectricPiano/synthesized-piano.html |
| FM Synthesized Piano #2 | Roberto (Hexter) | CC0 line; v2016-11-12 | GREEN | 8.7 MiB SFZ+FLAC | synthesized, bright EP | | same page |
| Drawbar organ emulation | Roberto (setBfree) | CC0 line; v2019-07-12 | GREEN | 5.8 MiB SFZ | rendered tonewheel organ | own page: "Imitation of an electromechanical organ" | freepats.zenvoid.org/Organ/electric-organ.html |
| Percussive organ emulation | Strix SoundFont Team (setBfree) | CC0 line; v2019-07-15 | GREEN | 12 MiB | rendered | | same page |
| Rock organ emulation | Strix SoundFont Team (setBfree) | CC0 line; v2019-07-15 | GREEN | 12 MiB | rendered | | same page |
| Church Organ Emulation | Roberto (Aeolus) | CC0 line; v2019-09-24 | GREEN | 13 MiB SFZ | rendered from Aeolus pipe-organ synth | VCSL pipe organ already on disk is a real recording | freepats.zenvoid.org/Organ/pipe-organ.html |
| Button Accordion HN | Jeff Stauffer (recording), michael02022 (processing) | CC0 line; v2024-03-29 | GREEN | 4.8 MiB SFZ+FLAC (5.1); WAV 5.7 MiB | Hohner button accordion | own page: "recorded by Jeff Stauffer in November 2023" | freepats.zenvoid.org/Organ/accordion.html |

### 1b. FreePats strings, guitars, basses
| Library | Publisher/author | Licence | Class | Size | Samples | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| Concert Harp | Versilian Studios (VCSL) / Roberto | CC0 line; v2020-07-02 (the CC BY-NC line on the page is for the PHOTO only) | GREEN (VCSL-derived; raw is on disk) | 4.8 MiB SFZ+FLAC; small 2.5 MiB | 2 velocity layers | performer K. Smith, Boston, 2015 | freepats.zenvoid.org/OrchestralStrings/harp.html |
| Spanish classical guitar | Roberto | CC0 line; v2019-06-18 | GREEN | 4.5 MiB SFZ+FLAC | single layer | own page: "recording conditions were far from ideal" | freepats.zenvoid.org/Guitar/acoustic-guitar.html |
| FSS Steel-String Acoustic Guitar | FlameStudios (Gary Campion) | "GNU General Public License version 3 or later, with a special exception."; v2020-05-21 | SHARE-ALIKE (settled reject) | 13 MiB | subset of a GIG | | freepats.zenvoid.org/Guitar/steel-acoustic-guitar.html |
| Ukulele | Mateusz Dąbrowski | CC0 line; v2026-08-11 (NEW since last scan) | GREEN | 2.3 MiB SFZ+FLAC | C4-C6, plus body tap and chuck | Flight Fireball tenor, RØDE Podcaster | freepats.zenvoid.org/GuitarFamily/ukulele.html |
| FSBS Electric Guitar Clean #1 | FreePats (Fender, direct + amp rack) | CC0 line; v2026-08-07 | GREEN | 118 MiB SFZ+FLAC; small 3.0 MiB | bridge pickup | | freepats.zenvoid.org/ElectricGuitar/clean-electric-guitar.html |
| FSBS Clean #2 (Jazz) | FreePats | CC0 line; v2026-08-07 | GREEN | 63 MiB; small 2.4 MiB | | | same page |
| FSBS Electric Guitar Direct | FreePats | CC0 line; v2022-09-11 | GREEN | 74 MiB | raw DI, needs an amp sim | | same page |
| FSBS Distorted #1 / #2 | FreePats | CC0 line; v2022-09-11 | GREEN | 300 MiB / 129 MiB | | | freepats.zenvoid.org/ElectricGuitar/distorted-electric-guitar.html |
| Bass Guitar YR (picked, finger) | Andrea Biasior | "sent for inclusion in FreePats on September 2019, under the terms of the Creative Commons CC0 1.0 public domain dedication."; v2019-09-30 | GREEN | 2.7 / 3.1 MiB SFZ+FLAC | Yamaha RBX | | freepats.zenvoid.org/ElectricGuitar/clean-electric-bass.html |

### 1c. FreePats winds, reeds, world, chromatic percussion
| Library | Publisher/author | Licence | Class | Size | Samples | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| Clarinet | Tyler Dence (engineer), Kaili Dence (performer) | CC0 line; v2019-08-18 | GREEN | 6.7 MiB SFZ | single layer | AT873R mic "a foot and a half away from the bell" | freepats.zenvoid.org/Reed/clarinet.html |
| Tenor Saxophone | VCSL / Roberto | CC0 line; v2020-07-17 | GREEN (already on disk as VCSL) | 31 MiB SFZ+FLAC; small 4.7 MiB | looped ("infinite sustain loops") | | freepats.zenvoid.org/Reed/saxophone.html |
| Wooden Recorder | Eugene Vlaskin | "Creative Commons CC0 1.0"; v2020-12-05 | GREEN | 8.9 MiB SFZ+FLAC | randomized layers | | freepats.zenvoid.org/Wind/recorder.html |
| Ocarina | Xavimart, Gonzalo, Roberto | CC0 line; v2024-10-02 | GREEN | 4.2 MiB SFZ+FLAC | | two Zoom H1 | freepats.zenvoid.org/Wind/ocarina.html |
| Bagpipe (in G) | Gilles Sadowski | CC0 line; v2026-08-06 | GREEN | 15 MiB SFZ+FLAC; small 5.2 MiB | F4-G6 chanter + 2 drones (G2, G3) | Sennheiser MKE2, standing 80 cm | freepats.zenvoid.org/Ethnic/bagpipe.html |
| Kalimba | Xavimart, Gonzalo, Roberto | CC0 line; v2019-05-31 (files 20190723) | GREEN (VCSL kalimbas also on disk) | 10.3 MiB SFZ WAV | round-robin layers | | freepats.zenvoid.org/Ethnic/kalimba.html |
| Jaw Harp | Eugene Vlaskin, Michail Chayka (Nivkh musician) | CC0 line; v2020-06-06 | GREEN | 1.8 MiB | | own page: "the quality is not very good" | freepats.zenvoid.org/Ethnic/jaw-harp.html |
| Glasses of water | Roberto | CC0 line; v2019-12-27 | GREEN | 11 MiB SFZ+FLAC | round-robin, A4-G6 | | freepats.zenvoid.org/ChromaticPercussion/glass.html |
| Hang (D minor) | Gonzalo & Roberto | CC0 line; v2022-03-30 | GREEN (already on disk) | 13 MiB | randomized layers | | freepats.zenvoid.org/ChromaticPercussion/hang.html |
| Tubular Bells | VCSL / Roberto | CC0 line; v2024-11-30 | GREEN (VCSL-derived) | 13 MiB; small 2.1 MiB | randomized layers | | freepats.zenvoid.org/ChromaticPercussion/tubular-bells.html |
| Xylophone (medium mallets) | VCSL / Roberto | CC0 line; v2020-07-06 | GREEN (VCSL-derived) | 2.3 MiB | | | freepats.zenvoid.org/ChromaticPercussion/xylophone.html |

### 1d. FreePats percussion, synths, sets (listed for completeness; drums and synths are in [libraries-3-drums-synths-effects.md](libraries-3-drums-synths-effects.md))
| Library | Licence | Class | Size | URL |
|---|---|---|---|---|
| MuldjordKit (Lars Muldjord) | "Creative Commons Attribution 4.0 license."; v2020-10-18 | CREDIT | 157 MiB SFZ+FLAC | Percussion/acoustic-drum-kit.html |
| Timpani (VCSL) | CC0 line; v2024-08-10 | GREEN (VCSL) | 8.9 MiB | Percussion/orchestral-percussion.html |
| World percussion (cajón, bongos, shakers, tambourine, claves, castanets, conga, maracas, darbuka, hand clap) | CC0 line; v2020-09-05 | GREEN | 6.6 MiB | Percussion/world-and-rare-percussion.html |
| FreePats synthesizer percussion | CC0 line; v2022-07-18 | GREEN | 1.8 MiB | Percussion/electric-percussion.html |
| 20 synth banks: Lately Bass, Synth Bass 1/2, Synth Brass 1/2, Synth Strings 1/2, Bass & Lead, Fifths, Square, Calliope, Pad Choir, Sweep Pad, New Age, Pad Bowed, Goblins, Soundtrack, Sci-Fi, Crystal | CC0 line on each; versions 2019-2024 | GREEN | 0.7-13 MiB each | Synthesizer/*.html |
| FreePats General MIDI set | "GNU General Public License version 3 or later, with a special exception."; v2022-10-26 | SHARE-ALIKE | 127 MiB SFZ+FLAC | SoundSets/general-midi.html |
| FreePats GM percussion set | same GPL 3 + exception; v2020-08-22 | SHARE-ALIKE | 20 MiB | same page |
| ColomboADK (GitHub only) | README: "License: GNU GPL v2 or later + exception" (`gh api repos/freepats/colomboADK/readme`, v2020-05-30) | SHARE-ALIKE | 9 MB repo | github.com/freepats/colomboADK |

---------------------------------------------------------------------------------------------------

## 2. University of Iowa Electronic Music Studios, Musical Instrument Samples

robots.txt 404 (2026-11-13). Main page https://theremin.music.uiowa.edu/MIS.html (Last-Modified 2017-12-23):
- Terms, exact: "Since 1997, these recordings have been freely available on this website and may be downloaded and used for any projects, without restrictions." Lawrence Fritts' own 2012 announcement (auditory.org/postings/2012/569.html): "have been freely available without any restrictions on their use."
- No page names a licence, and none mentions redistribution of the samples themselves (searched MIS, index, documentation, history, post-2012 intro, instrument pages). The grant "used for any projects, without restrictions" is the only wording. VPO and SSO both redistribute Iowa samples on this basis (VPO quotes the same sentence).
- Class: GREEN by an unnamed permissive grant (no named licence, redistribution not mentioned explicitly). Settled entry confirmed, nothing newer.
- Recording: until 2011, one Neumann KM 84, 16/44.1 mono, "chromatic scales played note-by-note at pp, mf, and ff dynamic levels", anechoic chamber (piano excepted). From 2011/2012: 24/96, three Earthworks QTC-40 in a Decca tree; stereo 24/96 single-note zips plus 16/44.1 mono scales.
- Pre-2012 list: flute, alto flute, bass flute, oboe, Eb clarinet, Bb clarinet, bass clarinet, bassoon, Bb soprano sax, Eb alto sax, horn, Bb trumpet, tenor trombone, bass trombone, tuba, violin, viola, cello, double bass, violin/viola/cello/double bass (2012, 24-bit stereo), marimba, xylophone, vibraphone, bells, crotales, cymbals, gongs/tamtams, hand percussion, tambourines, piano, balloon pop, guitar, found objects 1.
- Post-2012 list (MISPost2012Intro.html, Last-Modified 2017-12-23): the same woodwinds, brass, strings, percussion "edited sound files extracted ... from the pre-2012 category", only the "ff" files, converted to 24-bit 44.1 kHz stereo (mid-side widened, denoised); excludes guitar and piano.
- Sizes: piano page lists 260 AIFF notes (pp/mf/ff), summed 1,340 MB as listed. Other pages list per-file sizes (e.g. 2012 violin ff notes ~0.5-1.1 MB each). No total stated.
- Quality: anechoic and dry; single dynamic per file, no round robins; famous but needs looping/editing to play well (SSO and VPO did that work).

---------------------------------------------------------------------------------------------------

## 3. Pianos (and electric pianos, organs, harpsichords)

| Library | Publisher/author | Licence (exact quote; where read; source date) | Class | Size | Samples, dynamics, RR | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| Salamander Grand Piano V3 | Alexander Holm | sfzinstruments README: "This work is licensed under a Creative Commons Attribution 3.0 Unported License." + LICENSE = CC BY 3.0 legal code (`gh api`, LICENSE first commit 2020-05-14, repo pushed 2022-01-09); FreePats page same | CREDIT | repo 714 MB (48/24) | Yamaha C5, 16 velocity layers, minor thirds | No licence change found; Holm's 2010 LAU announcement not read (robots) | github.com/sfzinstruments/SalamanderGrandPiano |
| Accurate-Salamander | Chisato Yamauchi (derivative) | MA listing "by-3"; WebSearch summary of its page: keeps the original licence | CREDIT | n/a | re-tuned/re-voiced Salamander | | ir.isas.jaxa.jp/~cyamauch/AccurateSalamander/ |
| Slender Salamander | Signal Experiments (derivative) | MA artifact 534 JSON: "License: Creative Commons Attribution 3.0" | CREDIT | 238 MB (44/16) | 3 of 16 layers, phase aligned | | musical-artifacts.com/artifacts/534 |
| YDP Grand | Zenph/OLPC, FreePats | see 1a | CREDIT | 36 MiB SF2 | | | FreePats |
| Splendid Grand Piano | "AKAI" | README (`gh api`, pushed 2024-07-10): "This samples set was released as public domain in early 2000 by Akai company." No LICENSE file; GitHub licence none | GREEN by claim only: provenance UNVERIFIED (no Akai source found) | repo 71 MB | Steinway, 4 velocity layers | treat as unsafe for core until an Akai source is found | github.com/sfzinstruments/SplendidGrandPiano |
| Headroom Piano | Bengt Nilsson | README: "Creative Commons Attribution 4.0 International License." (`gh api`, pushed 2022-01-03; GitHub id CC-BY-4.0) | CREDIT | repo 134 MB (sfzinstruments says 156 MB) | Yamaha C3 | in [libraries-1-sfz-karoryfer.md](libraries-1-sfz-karoryfer.md) | github.com/sfzinstruments/BengtNilsson.HeadroomPiano |
| Maestro Concert Grand | Mats Helgesson | mcgreadme.txt: "You may not modify and spread this soundfont without the author´s written permission." (`gh api`) | REJECT | 250 MB | Yamaha CF-3 | | github.com/sfzinstruments/MatsHelgesson.MaestroConcertGrandPiano |
| Piano in 162 | Ivy Audio | NO licence on the freebies page (ivyaudio.com/freebies, Last-Modified 2025-07-29; only torrent links); sfzinstruments lists no licence, only "please donate" | REJECT (no stated licence; one may sit inside the archive: needs a person's look) | 4.9 GB | Steinway B, 5 dynamics, 2 RR, pedal on/off, 2 mics | | ivyaudio.com/freebies |
| Ivy Audio Poeesia (choir, paid) | Ivy Audio | home page "License / GPLv3 open source / CC+ Noncommercial" | REJECT (NC, paid) | n/a | 6 sopranos + 6 altos | | ivyaudio.com |
| LivingRoom Upright Micro SFZ | KeyPleezer | sfzinstruments listing: CC-BY-SA-4.0 (own page not read) | SHARE-ALIKE | 107 MB | 2 layers × pedal on/off | | keypleezer.com |
| Church Steinway | Pianobook | Pianobook EULA (settled) | REJECT | 104 MB | | | pianobook.co.uk |
| Greg Sullivan E-Pianos (Yamaha CP80, Hohner Pianet T, Wurlitzer EP200/EP203W) | Greg Sullivan | OWN SITE www.sullivang.net (no date on page; the bare sullivang.net is now a parked page): "Licensing (updated 21-Oct-2026) : All these instruments I host here now have Creative Commons Attribution 4.0 International licensing. (CC BY 4.0)"; sfzinstruments README says CC BY 3.0 (repo pushed 2020-08-23) | CREDIT (NEWER: author now says CC BY 4.0) | repo 18 MB (listing 21.5 MB) | 3 electric pianos | own site hosts the Gigastudio originals | www.sullivang.net; github.com/sfzinstruments/GregSullivan.E-Pianos |
| jRhodes3c / jRhodes3d | Jeff Learman | jRhodes3d LICENSE: "License for samples: CC BY-NC" (`gh api`, repo pushed 2026-06-22); sfzinstruments page for 3c: CC-BY-NC-SA-4.0 | REJECT (NC) | 18 MB / 160 MB | Rhodes Mark I, 5 velocity layers, 67 samples | | github.com/jlearman/jlearman.jRhodes3d |
| jlearman.SteelDrum (v2) | Jeff Learman | LICENSE = Unlicense: "This is free and unencumbered software released into the public domain." (`gh api`, pushed 2026-09-01) | GREEN (NEWER than settled jSteelDrum v1) | repo 38 MB | C tenor pan, 5 velocity layers, crossfades | | github.com/jlearman/jlearman.SteelDrum |
| Stevie Clavinet | anonymous (jlearman fix) | README: "License is unknown." | REJECT | 47 MB | | | github.com/jlearman/stevie-clavinet |
| Terkelsen's Marimba | Lars Terkelsen | no licence anywhere in repo | REJECT (no licence) | 24 MB | multi-velocity | | github.com/jlearman/Terkelsen.Marimba |
| VCSL Keys (3 grands, 2 uprights, 5 harpsichords) | Versilian Studios | own page: "VCSL Keys is under a Creative Commons 0 (CC0) license, as a derivative of VCSL." (versilian-studios.com/vcsl-keys, Last-Modified 2026-11-13) | GREEN (already on disk: same samples as VCSL) | 680 MB FLAC, 1,466 samples | releases on most; pedal samples on "S" and "Knight" | includes the old "VS Upright No. 1" (piano "Y"), 3 vel, 2 RR | versilian-studios.com/vcsl-keys/ |
| Iowa piano | U. Iowa EMS | see section 2 | GREEN (unnamed grant) | 1,340 MB AIFF | 88 notes × pp/mf/ff | not anechoic | theremin.music.uiowa.edu/MISpiano.html |
| Sofia MZ grand pianos (Blüthner "Contessa Bazzi", Steinway D, Bösendorfer Imperial, Fazioli F308, Bechstein D282) | Sofia MZ | only Musical Artifacts uploader field "by-3" (artifacts 3275, 3358-3362); author site luciphercode.wixsite.com/sofia-mz now 404; a pianoclack thread says the author took the site and SourceForge files down | REJECT for now (withdrawn by author; licence never read from the author) | 4.8 GB (Blüthner) | 20 velocity layers, minor thirds, pedal up/down | forum users praise it | musical-artifacts.com/artifacts/3360 |
| Lars Palo GrandOrgue pipe organs (Bureå, Bygdsiljum, Jukkasjärvi, Kalvträsk, Norrfjärden...) | Lars Palo | own download page: "licensed under a Creative Commons Attribution-ShareAlike 2.5 license, Swedish jurisdiction"; "The more recent (2023) sample set of Norrfjärden is licensed under a CC BY-SA 4.0 International license." (familjenpalo.se/vpo/download, no date) | SHARE-ALIKE | 92 MB to ~2 GB per set; jOrgan SF2 versions 3.8-84 MB | full ranks, releases | real Swedish church organs; VCSL organ already on disk | familjenpalo.se/vpo/download/ |
| Piotr Grabowski organ sets | Piotr Grabowski | FAQ (piotrgrabowski.pl/info) states no licence; newer sets paid | REJECT (no stated licence on pages read) | | | | piotrgrabowski.pl |

Electric piano / organ / harpsichord, CC0 real recordings: none found beyond VCSL (harpsichords, organ: already on disk) and FreePats' rendered FM pianos and organ emulations (CC0, synthesized). GitHub searches "electric piano sfz", "organ sfz samples", "harpsichord sfz" found nothing else usable (sverweij/pluck-sfz is MIT but a synthetic "harpsichord-y" patch).

---------------------------------------------------------------------------------------------------

## 4. Orchestral

| Library | Publisher/author | Licence (exact quote; where read; source date) | Class | Size | Samples, dynamics, RR | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| Sonatina Symphonic Orchestra (peastman/sso, the maintained continuation) | Mattias Westlund, Peter Eastman | LICENSE: "It may be used and distributed / under the terms of the Creative Commons Sampling Plus 1.0 license" (`gh api`, repo pushed 2026-08-22) | REJECT (settled; no re-licensed fork exists) | repo 1.29 GB | | README quotes Westlund: some "very old soundfonts I have no idea who the original authors were" | github.com/peastman/sso |
| SSO forks: davem2/sso, TheAviv/sfz | various | only SFZ fixes, no licence change | REJECT | | | | github.com |
| SSO XRNI (Renoise) | zeograd | WebSearch summary: conversion relabelled CC-BY-SA; cannot override Sampling Plus | REJECT | | | | files.renoise.com |
| No Budget Orchestra (ssj71 fork = the repository VPO cites) | Mihai Sorohan + many sources | ROOT license.txt (`gh api`, committed 2016-03-11): "1.You may not use it for commercial sample libraries (put it on a CD and sell it, sell it online, etc)"; per-folder licence files are MIXED: CC BY-SA 4.0 (Block, Cabasa, Castanets, CelloSect, Claves, JgViolins, OrchCym, SaxSect, SleighBell, Snare, SoloTrumpet, SoloTuba, Tambourine, Triangle, TromboneSect, TrumpetSect), Sampling+ (Chimes, FenderPick, Harmonica, JazzBass, MellowBass, Recorder, SoloCello, SoloClarinet, SoloFlute, SoloViolin, SoloViolin2, glock), "CC BY-NC" (Banjo, MslpVibes, OrchBassDrum), Karoryfer royalty-free (AltoSax, SoloBass, pastabass), and ~55 folders (Choir, Accordion, Melodica, Harp, Timpani, SoloViola, SoloBassoon...) with NO licence file | REJECT as a whole; the CC BY-SA 4.0 folders alone = SHARE-ALIKE | repo 136 KB (SFZ only; samples on bandshed.net, not read) | | CONTRADICTS the settled "CC BY-SA 4.0 (secondary page)": its own files show a mix | github.com/ssj71/No-Budget-Orchestra |
| Virtual Playing Orchestra 3.2.4 / 3.3 | Paul Battersby | own page (virtualplaying.com/virtual-playing-orchestra, read 2026-11-13; v3.2.4 dated Sep 25, 2022): "different licenses apply to different parts of the library" and redistribution only if "any derived samples or library is kept for personal use only or is given away for free"; parts: SSO Sampling Plus 1.0, Westlund extra samples CC BY-SA 3.0, NBO "CC BY-SA 4.0", VSCO 2 CE CC0, Iowa, stamperadam CC0 | REJECT as a bundle (settled); take parts from originals | wave files 603 MB | 2 velocity layers on brass, RR on staccato strings, looped sustains | | virtualplaying.com/virtual-playing-orchestra/ |
| Mattias Westlund additional samples | Mattias Westlund | only via VPO page: "Creative Commons Attribution-ShareAlike 3.0 Unported license"; mattiaswestlund.net robots.txt disallows all (site parked) | SHARE-ALIKE (secondary source only) | | | | not read |
| Libre Orchestra (2017, the "100% libre" subset of NBO) | Lyberta | announcement (lists.endsoftwarepatents.org, 2017-09-19); libreorchestra.net is dead; GitLab API barred by robots | not reached | | | | |
| TinySOL (IRCAM Studio On Line subset) | IRCAM / Lostanlen et al. | Zenodo record 3685367 (Published January 31, 2020, v6.0): "TinySOL is offered free of charge under the terms of the Creative Commons Attribution 4.0 International (CC BY 4.0) license"; also "TinySOL is the only subset of SOL which is released under a Creative Commons License." (the OrchideaSOL paper calls it non-commercial: the Zenodo record is the licensor's own statement) | CREDIT | 1.0 GB tar.gz, 2,913 clips | 14 instruments: bass tuba, French horn, trombone, trumpet, ACCORDION, cello, contrabass, viola, violin, alto sax, bassoon, clarinet, flute, oboe; ordinario, chromatic, pp to ff; ~1% pitch-shifted fills | 44.1/16 mono, 2-10 s notes, IRCAM studio 1996-99 | zenodo.org/records/3685367 |
| Good-sounds (MTG-UPF) | MTG, Universitat Pompeu Fabra | Zenodo 4588740 (Published June 29, 2017, v1.1): "licensed under the Creative Commons Attribution-NonCommercial 4.0 International License." | REJECT (NC) | 13.9 GB | 12 instruments | | zenodo.org/records/4588740 |
| NSynth | Google Magenta | own page: "The dataset is made available by Google Inc. under a Creative Commons Attribution 4.0 International (CC BY 4.0) license." (no date) | CREDIT, but NOT RECOMMENDED | ~30 GB | 1,006 instruments × 88 pitches × 5 velocities, includes a "vocal" family | own page: notes from "commercial sample libraries", 16 kHz mono 4 s: low fidelity and provenance worry | magenta.tensorflow.org/datasets/nsynth |
| Versilian: VSCO 2 CE | Versilian | settled CC0 | GREEN (on disk) | 3 GB | | | |
| Versilian: EtherealWinds Harp II Community Edition | Versilian Studios / Etherealwinds | own page: "we put together this reduced version of the new library under a Creative Commons Attribution 4.0 International license." (versilian-studios.com/etherealwinds-harp, Last-Modified 2026-11-13); blog 2024-11-27: attribution needed only for derivative sample libraries ("You can and should use the harp however you wish provided you are not making a sample library out of it") | CREDIT (an app shipping the samples is sharing them: credit it) | 400 MB, 255 samples | chromatic stereo lever harp, 2 velocity layers, 2 RR, "a sneak peek at some of the effects and vocals we recorded" | | versilian-studios.com/etherealwinds-harp/ |
| Versilian: VCSL Keys | see section 3 | CC0 | GREEN (on disk) | | | | |
| Versilian: Virtuosity Drums | Karoryfer × Versilian | (drums: [libraries-3-drums-synths-effects.md](libraries-3-drums-synths-effects.md)) | | | | | |
| Versilian: Legacy page (Upright Piano No. 1, Fretless Zither ZOTY, The Ebuzz, Tune Me) | Versilian | legacy page gives no licence; Upright No. 1 now lives in VCSL Keys (CC0) | Upright: GREEN via VCSL Keys; others REJECT (no stated licence, Kontakt/VST) | | | | versilian-studios.com/legacy/ |
| Versilian: Sample Packs (paid) | Versilian | own page: "direct redistribution/sharing of samples is strictly prohibited." | REJECT | | | | versilian-studios.com/sample-packs/ |
| Iowa strings/winds/brass | U. Iowa | section 2 | GREEN (unnamed grant) | | pp/mf/ff, 2012 strings 24-bit stereo | | |
| Philharmonia | settled | REJECT | | | | | |

---------------------------------------------------------------------------------------------------

## 5. Musical Artifacts (https://musical-artifacts.com)

robots.txt: "Crawl-delay: 5" only (Last-Modified 2017-07-14). Read through `artifacts.json?formats=<sfz|sf2>&license=<public|by|by-3|wtfpl>&page=N`, 6 s apart, plus licenses.json. CC0 is filed under "public" (Public Domain); there is no separate CC0 or MIT slug. 1,317 records: public 361, by 304, by-3 335, wtfpl 317. About 800 are General MIDI ROM rips, game/console soundfonts and memes; their licence fields are the uploader's own claim and are worthless (e.g. "VSL Bösendorfer 280VC [Vienna Symphonic Library]" filed as WTFPL, "Nexus Brilliant Piano" and "VSL Bosendorfer Imperial" filed as CC BY: commercial-library rips). The real-quality, plausibly owned multisamples are mostly mirrors of FreePats, Karoryfer, VCSL and sfzinstruments (better taken from the source).

| Library | Author (MA field) | Licence (MA field; artifact id) | Class | Notes |
|---|---|---|---|---|
| D. Smolken's double bass | DSmolken (Karoryfer) | by (95) | CREDIT on MA; the sfzinstruments mirror says CC0 (smpldsnds mirror id CC0-1.0) | part 1 (Karoryfer, [libraries-1-sfz-karoryfer.md](libraries-1-sfz-karoryfer.md)); arco 5 velocity layers, 2 RR |
| Greg Sullivan E-Pianos (SFZ) | Greg Sullivan, kinwie | by-3 (4549) | CREDIT | author now says CC BY 4.0 (section 3) |
| Headroom Piano mirror | Bengt Nilsson | by (6952) | CREDIT | |
| Tiny Piano 00 | Signal Experiments | by (402, 403) | CREDIT | experimental tiny piano, 10.8 MB |
| melodica soundfont | BAGEL | by-3 (3395) | CREDIT (MA field only) | 7 samples F4-B6, looped; small |
| Celesta / Natural whistle / Triangle | BAGEL | by-3 (2656, 3029, 3178) | CREDIT (MA field only) | "pseudo-celesta" from a metallophone |
| Real Honky tonk piano | Milton Paredes | by (630) | CREDIT (MA field only) | his own 1987 Samick upright, sampled 2007 |
| MPJ vocal collection, Dianita's voice | Milton Paredes | by (631) | CREDIT (MA field only) | vocal PHRASES, not a playable vowel instrument |
| Ixox Don Quixote Flute | Xavier Hosxe, adapt. Mike77154 | by (5312) | unclear (source licence not shown) | |
| Yingchun Soul Voice Aahs / Doohs / Doohs 2 | Yingchun Soul | public (9836, 9192, 9834) | GREEN (MA field; self-recorded) | own text: "Choir Aahs recorded by me", phone-recorded, unlooped; low fidelity |
| LibrePlumber Koto | Yukinisuzume, Yingchun Soul | by-3 (9918) | CREDIT (MA field only) | "The only libre koto." Source not stated |
| CC0 Real Kalimba | Vini | public (9028) | GREEN (MA field only) | VCSL/FreePats kalimbas better |
| Schulze Pollmann Grand / Schulmann Upright | Simone Piervergili, Yingchun Soul | public (8184, 10127) | GREEN (MA field only) | recorded on a Samsung phone: low fidelity |
| Child's Vowel SF | GooglyMoogly | wtfpl (7922) | GREEN (MA field) | a child's vowels, 5 files, novelty |
| The Choral Soundfont Pack Freemium | RE MEDIA PRODUCTION | by-3 (8114) | REJECT (unverified; "Freemium" commercial teaser) | |
| Mellotron soundfonts | "unknown" | by (500) | REJECT (Mellotron tape rips) | |
| Warren Trachtman Steinway | Warren Trachtman | by (731), but its own text: "copyright 1999&2000 by Warren Trachtman" | REJECT (contradiction; no licence from author) | |
| Sofia MZ grand pianos | Sofia MZ | by-3 (3275, 3358-3362) | REJECT for now (section 3) | |
| GeneralUser GS (re-upload 9291, 9730) | "Mateo Isaias Official, S. Christian Collins" | public | the MA "public" field is WRONG; see below | |
| Fluid R3 (737, 738, 749) | Frank Wen | by | settled MIT | |

GeneralUser GS (the earlier "not settled"): author's own LICENSE.txt, `gh api repos/mrbumpy409/GeneralUser-GS/contents/documentation/LICENSE.txt` (committed 2026-02-23, v2.0.3, "License v2.0"): "Please feel free to use it in your software projects, and to modify the SoundFont bank or its packaging to suit your needs." But also: "I cannot be 100% sure where all of the samples originated" and "This uncertainty may concern you if you intend to use GeneralUser GS in a commercial software product." Class: custom permissive (no named licence), provenance self-declared uncertain: GREEN in wording, not for core. Size 31 MiB SF2.

MuseScore General (earlier "not settled"): MuseScore_General_License.md on the official mirror ftp.osuosl.org/pub/musescore/soundfont/MuseScore_General/ (Last-Modified 2020-07-10, v0.2 13 May 2020): "MuseScore_General.sf2 is shared under the MIT license as described in COPYING, as was FluidR3Mono and FluidR3 before it." and "The acknowledgements and copyright notices above must be included in any derivative work." Class: GREEN (MIT; keep the notices). Size 206 MB SF2, 38 MB SF3.

---------------------------------------------------------------------------------------------------

## 6. Voices, choirs, world instruments, free reeds

### 6a. Voices and choirs
| Library | Publisher/author | Licence (exact quote; where read; source date) | Class | Size | Samples, dynamics, RR | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| **epic-choir v1.0** (SATB vowel choir) | thepok, from VocalSet | LICENSE (`gh api`, release v1.0 2026-10-24): "Creative Commons Attribution 4.0 International (CC BY 4.0)" ... "a derived work of VocalSet ... licensed under CC BY 4.0, and are released under the same licence by thepok (2026)." | CREDIT (credit both thepok and VocalSet: Wilkins, Seetharaman, Wahl, Pardo 2018) | 4 zips 163-167 MiB (~740 MB) | S/A/T/B × 5 vowels (ah oh oo ee eh), p/mf/f crossfaded on CC1, 600 samples, one per whole tone, 3-10 real singers summed per sample, 44.1/16 stereo | own README: "sounds like a section, not one voice copied"; wordless vowels only, sustains extended from own audio | github.com/thepok/epic-choir |
| VocalSet 1.2 (the source) | Wilkins, Seetharaman, Wahl, Pardo | Zenodo 1442513 (Published March 8, 2018, v1.2): licence "Creative Commons Attribution 4.0 International" | CREDIT | 6.0-8.1 GB zips (2.1 GB v1.1) | 20 professional singers (9 m, 11 f), 5 vowels, standard and extended techniques, 10.1 h | raw dataset, needs cutting into an instrument (epic-choir did it) | zenodo.org/records/1442513 |
| OLPC / Berklee sample library (voices in Berklee44v5, OpenPathMusic44V5) | Berklee students, Open Path Music, curated by Richard Boulanger | Berklee news page (college.berklee.edu/node/1030448, no date): "also offered to all computer musicians under the Creative Commons Attribution License."; archive.org metadata licenseurl "http://creativecommons.org/licenses/by/3.0/" on all 61 items | CREDIT | Berklee44v5 46 MB zip; OpenPathMusic44V5 18 MB | Berklee44v5: male/female solo and group "aa" and "oo" at ~5 pitches each; OPM V5: a few sung vowels on C | 16-bit mono, sparse pitches: one-shot material, not a playable choir | archive.org/details/Berklee44v5 |
| Yingchun Soul Voice Aahs/Doohs | Yingchun Soul | MA "public" | GREEN (MA field) | small | phone-recorded single voice | low fidelity | MA 9836 |
| NSynth "vocal" family | Google | CC BY 4.0 | CREDIT, not recommended | | 16 kHz | | |
| Dave Choir | Decent Samples (Dave Hilowitz) | site EULA: "You may not redistribute any of the samples or sample libraries on their own, unless you have been given written permission by Decidedly, LLC." | REJECT | | 16 takes of one voice | | decentsamples.com/2019/04/09/new-free-sample-dave-choir/ |
| Ivy Audio Clare Solo v2 (vocal legato, Kontakt) | Ivy Audio | no licence on the freebies page | REJECT (no stated licence) | | | Kontakt only | ivyaudio.com/freebies |
| Ivy Audio Poeesia | Ivy Audio | "CC+ Noncommercial" | REJECT | | | | ivyaudio.com |
| Polyphone "choir choral aahhs" | author unknown | page: licence "public domain", author name blank | REJECT (unverifiable provenance; likely a ROM rip) | 5.08 MB SF2 | | | polyphone.io/en/soundfonts/vocals/224-choir-choral-aahhs |
| Karoryfer vocals (272 Merry Orks, Hadziha, Dandelion Witch...) | Karoryfer | in [libraries-1-sfz-karoryfer.md](libraries-1-sfz-karoryfer.md) | | | | | sfzinstruments.github.io/vocals |
| EtherealWinds Harp II CE extras | Versilian | CC BY 4.0 | CREDIT | | "a sneak peek at some of the effects and vocals" | not a vocal instrument | |

No CC0/public-domain multisampled choir of real quality was found again; the best open choir is CC BY 4.0 (epic-choir / VocalSet).

### 6b. World instruments (VCSL on disk already: harmonicas, kalimbas, mbiras, balafon, dan tranh, didgeridoo, ocarinas, recorders, strumstick, folk harp, psaltery, saxello, tenor sax, slit drum, bongos, conga, darbuka, cajón, frame drum)
| Library | Licence | Class | Size | Notes |
|---|---|---|---|---|
| FreePats Bagpipe (G, with drones) | CC0 line, v2026-08-06 | GREEN | 15 MiB / small 5.2 MiB | real recording, Gilles Sadowski |
| FreePats Jaw Harp | CC0 line, v2020-06-06 | GREEN | 1.8 MiB | Sakhalin, Nivkh player; rough quality |
| FreePats Kalimba, Ocarina, Recorder | CC0 | GREEN (VCSL has these families) | 4-10 MiB | |
| FreePats Hang | CC0 | GREEN (on disk) | | |
| OLPC Berklee44v1 (sitar ×6, tabla, khaen, kalimba, jew's harp, rainstick, gongs, temple bell...) and OpenPathMusic44V1 (tabla strokes) | CC BY 3.0 (archive.org metadata; Berklee statement) | CREDIT | 79 MB / 22 MB zips | one-shots, not chromatic multisamples |
| sfzinstruments aliexpress-erhu | GitHub id CC0-1.0 | GREEN | 78 MB repo | in [libraries-1-sfz-karoryfer.md](libraries-1-sfz-karoryfer.md) (found by GitHub search) |
| LibrePlumber Koto | MA by-3 | CREDIT (MA field only) | | source unstated |
| AEmotionStudio HF "sitar" / "shakuhachi" packs | HF README: sitar "CC0-1.0 (no third-party recordings — pure DSP synthesis)"; shakuhachi "CC0-1.0", one Wikimedia Commons A4 note | GREEN but worthless (synthesized / single note) | <1 MB | huggingface.co/AEmotionStudio |
| SampleScience Sitar Renaissance | not read (WebSearch: no CC licence stated) | REJECT (no stated licence) | | |
| koto, oud, shakuhachi, sitar (real chromatic multisamples under CC0/CC BY) | none found (GitHub and web searches) | | | |

### 6c. Accordion and free reeds
| Library | Licence | Class | Size | Notes |
|---|---|---|---|---|
| FreePats Button Accordion HN | CC0 line, v2024-03-29 | GREEN | 4.8 MiB SFZ+FLAC | Hohner, recorded 2023 |
| TinySOL Accordion | CC BY 4.0 (Zenodo 3685367) | CREDIT | part of 1.0 GB | chromatic, pp-ff, mono 44.1/16, IRCAM |
| Euterpea Studio Harmonium | repo LICENSE CC0-1.0, but README: "Samples: see the original Freesound page ... for the sample license terms" (freesound.org/people/donyaquick/sounds/330410) | needs a person's look (freesound barred) | 75 MB repo, Decent Sampler, 3 RR per zone | github.com/evanyerburgh/euterpea-harmonium |
| melodica soundfont (BAGEL) | MA by-3 | CREDIT (MA field only) | small | 7 samples |
| NBO Accordion / Melodica / Harmonica folders | no licence file / Sampling+ | REJECT | | |
| concertina, bandoneon, harmonium (other), real CC0 | none found | | | Akkordica (Syntheway), Sonokinetic, Payhip Balkan accordion: commercial |

---------------------------------------------------------------------------------------------------

## Sources not reached

- lists.linuxaudio.org (Holm's 2010 Salamander release posts): robots.txt "Disallow: /" for all. Hoped for the author's own licence words.
- wiki.laptop.org/go/Sound_samples (OLPC library index): 401 Unauthorized. Used archive.org metadata and Berklee's page instead.
- bandshed.net/sounds/sfz (No Budget Orchestra 2 original downloads): robots.txt disallows ClaudeBot, Claude-Web, anthropic-ai. (The directory index was fetched once in the same command as robots.txt before the rule was seen; nothing from it is used here.)
- mattiaswestlund.net/samples (Westlund's CC BY-SA extra samples): robots.txt "Disallow: /" for all; the domain is parked.
- signaturesounds.org (CC0 loops/one-shots incl. voice): robots.txt lists ClaudeBot and anthropic-ai in its block group; not used. signaturesound.org (KVR's spelling): DNS failure.
- codeberg.org (Ivy Audio's code host): robots.txt "Disallow: /" for ClaudeBot/Claude-Web.
- libreorchestra.net (Libre Orchestra, 2017): fetch failed (dead). Its GitLab repository: gitlab.com robots.txt disallows /api/v*; not found otherwise.
- luciphercode.wixsite.com/sofia-mz (Sofia MZ pianos' own licence): 404.
- freesound.org (Euterpea harmonium source, NBO Sampling+ packs, stamperadam): barred, needs a person's look.
- polyphone.io category listings: entries load by script; only the single choir page was readable.
- Ivy Audio Piano in 162 / Clare Solo licence inside the torrent archives: not downloadable under the rules; needs a person's look.

## Top picks per family

- Pianos: FreePats Old Piano FB (GREEN, 34 MiB / small 6.2 MiB, real honky-tonk upright) as a second core piano beside Upright KW; Salamander V3 (CREDIT, 707 MiB FLAC, 16 layers) as the big optional grand; Headroom (CREDIT, ~150 MB) as a smaller grand pack.
- Electric pianos: Greg Sullivan E-Pianos (CREDIT, author now CC BY 4.0, ~20 MB; CP80, Pianet T, Wurlitzer). No real CC0 EP exists; FreePats FM Piano 1 (GREEN, 24 MiB) for a DX7 sound.
- Organs: VCSL organ (on disk) for pipe; FreePats Drawbar/Rock/Percussive organ emulations (GREEN, 6-12 MiB) for tonewheel. Lars Palo sets are SHARE-ALIKE.
- Orchestral: TinySOL (CREDIT, 1.0 GB, 14 instruments pp-ff, IRCAM quality) is the one new open orchestral source; Iowa MIS (unnamed grant) for 2012 stereo strings; VSCO 2 CE (on disk).
- Harp: EtherealWinds Harp II CE (CREDIT, 400 MB, 2 layers, 2 RR) over FreePats' VCSL harp.
- Voices: epic-choir v1.0 (CREDIT CC BY 4.0, ~740 MB, SATB × 5 vowels × 3 dynamics, real singers) is the first open choir of real quality; no CC0 choir exists.
- World: FreePats Bagpipe (GREEN, 15 MiB / 5.2 MiB small); FreePats Jaw Harp (GREEN, 1.8 MiB) as a curiosity; erhu (aliexpress-erhu, CC0, [libraries-1-sfz-karoryfer.md](libraries-1-sfz-karoryfer.md)).
- Free reeds: FreePats Button Accordion HN (GREEN, 4.8 MiB) for core; TinySOL accordion (CREDIT) for depth.
- Other chromatic: FreePats Ukulele (GREEN, 2.3 MiB, new 2026); jlearman.SteelDrum v2 (GREEN Unlicense, 38 MB, 5 layers).
- Soundfont sets: MuseScore General (GREEN MIT, 38 MB SF3) is cleaner than GeneralUser GS (custom licence, author unsure of sample origins).
