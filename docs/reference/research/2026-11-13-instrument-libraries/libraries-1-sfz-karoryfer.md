# Free instrument libraries, part 1: the SFZ Instruments organisation, Karoryfer and the Discord SFZ GM Bank

> Part 1 of the free instrument library research of **13 November 2026**: the plain-words summary, the
> ranking and the recommendation are in [README.md](README.md); the licences that matter, with their
> quotations, are in the dated finding [2026-11-13-free-instrument-libraries.md](../../findings/2026-11-13-free-instrument-libraries.md). This is an AI's
> reading of each licence, not legal advice: every licence is quoted so a reader can check it. Gathered by
> four research passes on 2026-11-13 and spot-checked the same day (licence files re-read through the
> GitHub API, the TinySOL Zenodo page, Greg Sullivan's site).

Researched 2026-11-13. GitHub read only through `gh api` (LICENSE, README, .sfz text, release metadata,
commit history). Web pages read: shop.karoryfer.com (robots.txt `User-agent: *` allows /pages/), the
sfzinstruments.github.io source data file `data/sfz/instruments.yml` (via gh api, last commit
2025-07-26), ethanwiner.com, smmdrums.wordpress.com. No sample, archive or zip downloaded.


## Key finding: Karoryfer's licence history (answers "Older downloads may include a less permissive license")

Commit history of every pre-2022 Karoryfer repository in the org shows the same pattern:
- 2019-05-02: a `LICENSE.md` (or `license.MD`) was created holding **"Attribution 4.0 International"** (CC BY 4.0).
- Caveman Cosmonaut instead had `LICENSE` = **"GNU GENERAL PUBLIC LICENSE / Version 3, 29 June 2007"** from 2019-12-28.
- 2022-11-04 (06:53 to 07:36 UTC): each was replaced by `LICENSE` = **"Creative Commons Legal Code / CC0 1.0 Universal"**, and a new release (v1.001 / v1.002 / v1.004 ...) was published the same day 2 to 3 hours later.
- So: use only releases dated 2022-11-04 or later (the tags named in the tables). The older release assets
  (v1.000 etc., 2021) are still listed on GitHub and almost certainly carry CC BY 4.0 (or GPL 3 for
  Caveman Cosmonaut) inside the zip (zip contents not inspected: no downloads).
- **Shinyguitar**: GitHub's CC0-1.0 identifier is correct for the current repo and v1.002. Its 2019-05-02
  to 2022-11-04 licence was CC BY 4.0, not GPL 3. No GPL text appears anywhere in its history; the only GPL 3
  in the Karoryfer repos was Caveman Cosmonaut's. The old KVR listing could not be found (see Sources not reached).
- Libraries first published after 2022 (Horse Pulse, Black And Green, Sneakybass, the 2023 drum kits,
  Black And Blue Basses, Big Little Bass, Osiris Piano, the erhu, the cithara) were CC0 from their first licence file.

All Karoryfer READMEs also say "Royalty-free for all commercial and non-commercial use." with a
"Copyright 20xx Karoryfer Lecolds." line beside the CC0 LICENSE (the copyright line is normal; CC0 is the grant).

The free page (shop.karoryfer.com/pages/free-samples, no page date, footer "© 2026"), quoted:
"All our free sample libraries are under a Creative Commons Zero license." / "Older downloads may include a
less permissive license, but we changed that and now they are all CC0." / "Only Marie Ork has a different
license, as she uses a different technology which uses monolithic voice bank files".
It lists 35 libraries; all 35 are in the tables below.

Abbreviations: LIC = the repo's LICENSE file; "LIC CC0 2022-11-04" = CC0 1.0 Universal legal code
committed that day. Size = latest release asset (MB = MiB as GitHub reports bytes) unless "repo" (the git
repo size from the API, which includes history and can be larger than the samples).

---

## 1. Keys (pianos, electric pianos, organs, harpsichord)

| Library | Publisher/author | Licence (exact quote; where read; source date) | Class | Size | Samples, dynamics, round robins | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| Osiris Piano | Karoryfer Samples + Versilian Studios | LIC "CC0 1.0 Universal" (LICENSE, 2026-07-04); product page "Open source, released under CC-0 license." (no date); beta_notes "Beta 0.923: CC0 license added." | GREEN | 437.4 MB (v0.925, 2026-07-04) | "Over 1,200" samples (product page); several mic folders (UC, UC-Sus, -noisy variants); Sforzando 1.981+ | own page: "a quiet piano sample library with low noise, consistent playability"; 2026 KVR Developer Challenge entry; still "beta" 0.925 | github.com/sfzinstruments/Osiris_Piano |
| Scarypiano | Karoryfer Samples | LIC CC0 2022-11-04 (before: CC BY 4.0 from 2019-05-02) | GREEN | 342.5 MB (v1.002, 2022-11-04) | README: made "from the public domain piano samples created by the University of Iowa processed with Krotos Dehumaniser" | an effect-processed horror piano, not a clean piano | github.com/sfzinstruments/karoryfer.scarypiano |
| Salamander Grand Piano v3 (org port) | Alexander Holm; SFZ by kinwie | LIC "Creative Commons Legal Code / Attribution 3.0 Unported" (LICENSE, 2020-05-14); API says NOASSERTION | CREDIT | repo 731 MB | README: "16 Velocity layers Sampled in minor thirds"; 48 kHz 24-bit; hammer and resonance releases | settled as CC BY 3.0; this repo agrees | github.com/sfzinstruments/SalamanderGrandPiano |
| Headroom Piano | Bengt Nilsson; SFZ by kinwie | LIC "Attribution 4.0 International" (2020-05-28); README: mapped "with the author's permission as long keeping the original instrument name" | CREDIT | repo 137 MB (156 MB FLAC per README) | Yamaha C3; "Two mic positions", "5 velocity layers", "300 samples" | keep the name "Headroom Piano" | github.com/sfzinstruments/BengtNilsson.HeadroomPiano |
| Greg Sullivan E-Pianos (CP80, Pianet T, Wurlitzer EP200) | Greg Sullivan; SFZ by kinwie | LIC "Attribution 3.0 Unported" (2020-05-24); README: "with the author permission with the request for attribution." | CREDIT | repo 18.8 MB | three electric pianos | see GM bank 003 conflict below | github.com/sfzinstruments/GregSullivan.E-Pianos |
| Splendid Grand Piano | AKAI (per readme); SFZ by kinwie | No LICENSE. README: "This samples set was released as public domain in early 2000 by Akai company." (repo last commit 2024-05-06) | GREEN? (claim only in the porter's readme; no Akai primary statement read) | repo 72.8 MB | Steinway; "4 velocity layers" (+ a filtered 5th) | 256 MB original set; confirm Akai's release before shipping | github.com/sfzinstruments/SplendidGrandPiano |
| Maestro Concert Grand | Mats Helgesson; SFZ by kinwie | mcgreadme.txt: "All rights reserved. You may not sell this sound set or any of the samples" / "You may not modify and spread this soundfont without the author´s written permission." (2003 text; repo 2022-01-03) | REJECT | repo 256 MB | Yamaha CF-3, 452 stereo samples, 5 velocity layers | permission to sfzinstruments by e-mail only | github.com/sfzinstruments/MatsHelgesson.MaestroConcertGrandPiano |
| jRhodes3c | Jeff Learman | LIC: "the jRhodes samples are licensed under CC BY-NC-SA 4.0" (text links by-nc/4.0); "NC: Only noncommercial use of the work is permitted." (since 2020-05-07; updated 2025-11-25 adding a Strudel-only CC BY-SA grant) | REJECT (NC) | repo 17.8 MB | 1977 Rhodes Mark I, short looped samples | Strudel grant does not extend to us | github.com/sfzinstruments/jlearman.jRhodes3c |
| jRhodes3d | Jeff Learman | LIC: "License for samples: CC BY-NC" (2022-09-24) | REJECT (NC) | repo 163 MB | full-length Rhodes samples | | github.com/sfzinstruments/jlearman.jRhodes3d |
| Clavecin (harpsichord) | unknown; Giga conversion by S. Christian Collins | No LICENSE, README has no licence (2020-10-09) | REJECT (no licence) | repo 4.1 MB | | | github.com/sfzinstruments/Clavecin |
| Orgue Eglise (church organ) | unknown; Giga conversion by S. Christian Collins | No LICENSE, README has no licence (2020-10-09) | REJECT (no licence) | repo 27.4 MB | Doux / Full / Reeds / Strings patches | | github.com/sfzinstruments/OrgueEglise |
| GM bank 001 Acoustic Grand Piano | Salamander, mapped by jlearman | .sfz header: "License: Creative Commons CC-BY, Alexander Holm" (2021-10-13) | CREDIT | 16 files, 16.2 MB | GM-sized subset, looped | small credit-licensed piano | Discord-SFZ-GM-Bank |
| GM bank 002 Bright Acoustic Piano | VCSL Yamaha upright, mapped by jlearman | header: "License: Creative Commons CC0, Versilian Studios" (2021-10-13) | GREEN | 14 files, 14.7 MB | one layer, looped | small CC0 piano, core-size | Discord-SFZ-GM-Bank |
| GM bank 003 Electric Grand Piano | Greg Sullivan CP80, looped by jlearman | header: "License: Creative Commons CC0, Greg Sullivan" (2021-10-13) BUT source repo LIC is CC BY 3.0 "with the request for attribution" | CREDIT (header contradicts source; treat as CC BY 3.0) | 18 files, 5.1 MB | | header written by jlearman, not Greg Sullivan | Discord-SFZ-GM-Bank |
| GM bank 005 Electric Piano 1 | jRhodes GM version, Jeff Learman | header: "License: Creative Commons CC0, Jeff Learman" (committed by Jeff Learman, 2020-10-18 / 2021-10-13) BUT his jRhodes3c LICENSE (2020-05-07 onward) says NC for distributing the samples | CONFLICT: GREEN by the author's own header, REJECT by his main repo; ask the author before shipping | 15 files, 5.9 MB | relooped jRhodes | | Discord-SFZ-GM-Bank |

## 2. Guitars and plucked (incl. electric and upright basses)

| Library | Publisher/author | Licence | Class | Size | Samples, dynamics, round robins | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| Shinyguitar | Karoryfer (played/mapped by D. Smolken) | LIC CC0 2022-11-04; before: LICENSE.md "Attribution 4.0 International" 2019-05-02; README "royalty-free for all commercial and non-commercial use." | GREEN (v1.002) | 351.3 MB (v1.002, 2022-11-04) | "Up to 846" samples; 24-bit 44.1 kHz WAV; pickup + microphone blend | own page: "quite suitable for pop, EDM, hip-hop, funk, indie etc." | github.com/sfzinstruments/karoryfer.shinyguitar |
| Emilyguitar | Karoryfer (D. Smolken) | LIC CC0 2022-11-04 (CC BY 4.0 before) | GREEN | 98.7 MB (v1.001) | README: "Notes have four velocity layers and three round robins, release samples four round robins, noises five round robins." | Epiphone, direct pickups | github.com/sfzinstruments/karoryfer.emilyguitar |
| Black And Green Guitars | Karoryfer (rec. Brian Wood) | LIC CC0 2022-07-21 (settled) | GREEN | 459.5 MB (v1.000) | Gretsch Anniversary + Hofner Club | settled, on disk | github.com/sfzinstruments/karoryfer.black-and-green-guitars |
| Ganjo | itsclipping (repo by D. Smolken) | LICENSE.md CC0 1.0 (2023-02-08) | GREEN | 23.2 MB (v1.000) | SX 6-string guitar banjo | small; pack or even core | github.com/sfzinstruments/ganjo |
| Hungarian zither | (D. Smolken, Kim T commits) | LIC CC0 (2022-09-14) | GREEN | 168.5 MB (v1.001; v1.000 was 347.8 MB) | prime zither in G, strummed down/up, melody strings | own readme: "decent for relaxing, meditational strums" | github.com/sfzinstruments/hungarian_zither |
| Cithara barbarica | D. Smolken | LIC CC0 (2023-07-11) | GREEN | 168.5 MB (v1.000) | 10-string metal lyre; finger, nail, muted, sul tasto plucks; strum patch | | github.com/sfzinstruments/cithara-barbarica |
| Fashionbass | Karoryfer (D. Smolken) | LIC CC0 2022-11-04 (CC BY 4.0 before) | GREEN | 301.7 MB (v1.001) | "Notes have five velocity layers and three round robins" | Jazz-type bass, fifths tuning CGDA | github.com/sfzinstruments/karoryfer.fashionbass |
| Growlybass | Karoryfer | LIC CC0 2022-11-04 | GREEN | 159.7 MB (v1.002) | "Sustained notes have four velocity layers and four round robins" | Squier Jazz, EADG | github.com/sfzinstruments/karoryfer.growlybass |
| Swagbass | Karoryfer (D. Smolken) | LIC CC0 2022-11-04 | GREEN | 137.9 MB (v1.001) | "Notes have three velocity layers and four round robins" | flatwounds, fifths tuning | github.com/sfzinstruments/karoryfer.swagbass |
| Pastabass | Karoryfer (rec. spacecoyote) | LIC CC0 2022-11-04 | GREEN | 300.7 MB (v1.101) | Squier Bass VI; README: "everything's converted to 16 bits" | | github.com/sfzinstruments/karoryfer.pastabass |
| Black And Blue Basses | Karoryfer | `license` CC0 (2023-07-01); page "Open source, released under CC-0 license." | GREEN | 962.0 MB (v1.002, 2024-12-04) | "Over 2,000" samples; two 5-strings, finger and pick | 2023 KVR DC entry | github.com/sfzinstruments/karoryfer.black-and-blue-basses |
| Big Little Bass | Karoryfer | LIC CC0 (2024-12-05); page "Open source, released under CC-0 license." | GREEN | 251.3 MB (v1.000) | 440 samples | own page: "thumpy and a little weird" | github.com/sfzinstruments/karoryfer.big-little-bass |
| Ergo EUB | Karoryfer (D. Smolken) | LIC CC0 2022-11-04 | GREEN | 191.3 MB (v1.001) | electric upright, incl. very low detuned samples | | github.com/sfzinstruments/karoryfer.ergo |
| Kay 5-String Banjo | FlameStudios (Gary Campion) | LICENSE.txt: "This sample is released under the GNU GPL license." / "either version 3 of the License, or (at your option) any later version." (2008 text; repo 2020-10-09) | SHARE-ALIKE | repo 246 MB | closed-back banjo | same class as FreePats FSS (rejected) | github.com/sfzinstruments/FlameStudios.Kay5StringBanjo |
| Project16 Rickenbacker 4001 | Project 16 (Olivier Loiseau) | README: "License: Attribution-NonCommercial-ShareAlike 3.0 Unported (CC BY-NC-SA 3.0), modified" / "You are not allowed to use this product in a sampling library" | REJECT | repo 348 MB | fingered, muted, slap; 5 layers | | github.com/sfzinstruments/Project16Rickenbacker4001 |
| Damien's Funky Guitar | Ben Garden (Giga conv. S. C. Collins) | no licence in repo (2020-10-09) | REJECT (no licence) | repo 1.7 MB | | | github.com/sfzinstruments/DamiensFunkyGuitar |
| Ovation Guitar | unknown (Giga conv.) | no licence (2020-10-09) | REJECT (no licence) | repo 6.6 MB | 12-string mod-wheel patch | | github.com/sfzinstruments/OvationGuitar |
| Terkelsen's Mandolin | Lars Terkelsen (Giga conv.) | no licence (2020-10-09) | REJECT (no licence) | repo 14 MB | | | github.com/sfzinstruments/Terkelsen.Mandolin |
| Picked Lapharp | Ken Theriot (Giga conv.) | no licence (2020-10-08) | REJECT (no licence) | repo 2.3 MB | dulcimer-type | | github.com/sfzinstruments/PickedLapharp |
| GM bank 026 Acoustic Guitar (steel) | Jeff Learman, Martin HD-28 | settled CC0 | GREEN | 15 files, 3.0 MB | | settled | Discord-SFZ-GM-Bank |
| GM bank 034 Electric Bass (finger) | from "Killer Bass by Karyrofer" (= Fashionbass), header edited by D. Smolken 2021-09-28 | header: "License: CC0" (2021-10-13) | GREEN | 45 files, 21.5 MB | GM subset of Fashionbass | Karoryfer's own author edited it | Discord-SFZ-GM-Bank |

## 3. Bowed strings

| Library | Publisher/author | Licence | Class | Size | Samples, dynamics, round robins | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| Karoryfer x bigcat cello | Karoryfer + bigcat instruments | LIC CC0 2022-11-04 (CC BY 4.0 before); README "This instrument is open source, and you can do whatever you want with it." | GREEN | 126.5 MB (v1.001) | 479 samples (page); sustain, staccato, pizz, harmonic glissandi; velocity-layer and mod-wheel patches; also a Kontakt version | own page: "intended to be flexible and easy to use" | github.com/sfzinstruments/karoryfer-bigcat.cello |
| Meatbass | Karoryfer (D. Smolken; rec. L. Zamenhof) | LIC CC0 2022-11-04 | GREEN | 243.6 MB (v1.001) | "434 samples"; arco 3 or 5 velocity layers with two round robins; "Pizzicato - plucked notes with four velocity layers and four round robins" | 1958 Otto Rubner double bass; own page: "huge, wide and organic basses" | github.com/sfzinstruments/karoryfer.meatbass |
| D. Smolken's double bass | D. Smolken | LIC CC0 2022-11-04 (CC BY 4.0 before) | GREEN | 252.9 MB (v1.001) | arco + pizz + noises, keyswitch file | first Smolken library (2013) | github.com/sfzinstruments/dsmolken.double-bass |
| Sneakybass | Karoryfer | LIC CC0 (2022-12-05); README "CC-0 license." | GREEN | 324.7 MB (v1.000) | quiet pizz, ghost notes, noises, three octaves | own page: "about as warm as double bass gets" | github.com/sfzinstruments/karoryfer.sneakybass |
| Aliexpress erhu | D. Smolken | LIC CC0 (2023-07-10) | GREEN | 77.8 MB (v1.000) | long, short, sul tasto, layered marcato; close stereo | own readme: a cheap erhu "played by someone who's more of a violinist" | github.com/sfzinstruments/aliexpress-erhu |
| GM bank 045 Tremolo Strings | Versilian (Samulis) mix of VCSL + VSCO 2 strings | header: "License: Creative Commons CC0" (2020-10-19) | GREEN | 15 files, 10.5 MB | pre-rendered ensemble, m3rds | small CC0 ensemble | Discord-SFZ-GM-Bank |
| GM bank 046 Pizzicato Strings | same | header: "License: Creative Commons CC0" (2020-10-19) | GREEN | 16 files, 6.1 MB | | small CC0 ensemble pizz | Discord-SFZ-GM-Bank |
| Ethan's Cello (in Ethan Winer Collection) | Ethan Winer; SFZ by kinwie | see Drums/percussion row | GREEN? | (in 14.8 MB repo) | legato and random-attack controls | | github.com/sfzinstruments/EthanWiner.Soundfonts |

## 4. Brass

| Library | Publisher/author | Licence | Class | Size | Samples | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| War Tuba | Karoryfer (played by Jakub Lewicki, mapped by D. Smolken) | LIC CC0 2022-11-04 (CC BY 4.0 before) | GREEN | 104.1 MB (v1.002) | free page: "Over 1000 samples of folk-punk tuba."; staccatissimo, staccato, sustain, breath, valve noises | own page: "A raw and impolite tuba" | github.com/sfzinstruments/karoryfer.war-tuba |

No other brass in the org. (Site-listed leads outside the org: VGTrumpet "Northern Trumpets", site label "Free", licence not read.)

## 5. Woodwinds and reeds

| Library | Publisher/author | Licence | Class | Size | Samples | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| Bear Sax | Karoryfer | LIC CC0 2022-11-04 (CC BY 4.0 before) | GREEN | 125.4 MB (v1.004) | 1926 Conn baritone; sustain, marcato, staccato, subtone, growl; crossfaded dynamics | own page: "a very nice Conn baritone saxophone" | github.com/sfzinstruments/karoryfer.bear-sax |
| Weresax | Karoryfer | LIC CC0 2022-11-04 (CC BY 4.0 before) | GREEN | 188.4 MB (v1.003, 2025-06-14) | "256 WAV samples", 24-bit; sax, saxcordion, sxnth patches | README and product page say ALTO; the free list says "Tenor sax samples." (contradiction in Karoryfer's own pages) | github.com/sfzinstruments/karoryfer.weresax |
| MTG Solo Sax (S, A, T, Bari) | MTG (UPF); SFZ by kinwie | LIC "Attribution 4.0 International" (2020-09-19) | CREDIT | repo 102 MB | 24-bit 48 kHz mono; "3 round-robins using neighbour keys" | samples from freesound MTG packs 20251/20239/20247/20253: original pack licence needs a person's look (freesound barred) | github.com/sfzinstruments/MTG.SoloSax |
| Ixox Flute | Xavier Hosxe; SFZ by kinwie | LIC "Attribution 4.0 International" (2020-07-04) | CREDIT | repo 9.3 MB | flute with staccato and interval keyswitches | samples came from the Ixox VSTi's SoundFont; author's own page not read | github.com/sfzinstruments/Ixox.Flute |
| Squidpipes | Karoryfer | LIC CC0 2022-11-04 (CC BY 4.0 before) | GREEN | 42.1 MB (v1.001) | "More than 400" samples; chromatic + switchable drone; squidotron, squynth | novelty bagpipe (squid mantle bag, PVC pipes, oboe reed) | github.com/sfzinstruments/karoryfer.squidpipes |
| Ethan's Bassoon (Ethan Winer Collection) | Ethan Winer | see percussion row | GREEN? | (14.8 MB repo) | legato control | | EthanWiner.Soundfonts |
| GM bank 112 Shanai | Dr. Narayan Bhagawan Raikar | header: "License: CC), Narayan Bhagawan Raikar" (typo); readme: "Shanai contributed by Dr. Narayan Bhagawan Raikar under CC 0 license." (2021-10-13) | GREEN | 22 files, 8.6 MB | | readme says "for testing purpose only" in the sfz header | Discord-SFZ-GM-Bank |
| GM bank 110 Bagpipe | uploaded by D. Smolken (2021-05-15) | no licence line; .sfz is a 30-byte placeholder; readme only describes the instrument; GM-plan.txt marks it "OMIT" | REJECT until confirmed (likely CC0 by Karoryfer policy, but unstated) | 16 WAV, 33.2 MB, unmapped | "replica of a Scandinavian Marktsackpipa in A" with A, E, A drones | | Discord-SFZ-GM-Bank |
| Krumhorn | "Worra" (Giga conv. S. C. Collins) | no licence (2020-10-08) | REJECT (no licence) | repo 0.2 MB | from an Ensoniq EPS | | github.com/sfzinstruments/Krumhorn |

## 6. Voices and choirs

| Library | Publisher/author | Licence | Class | Size | Samples | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| 272 Merry Orks | Karoryfer | LIC CC0 2022-11-04 (license.MD CC BY 4.0 before) | GREEN | 38.6 MB (v1.001) | female death-metal vocals; 3 SFZ mappings | niche (growls, monster voices) | github.com/sfzinstruments/karoryfer.272-merry-orks |
| Legato vocal tutorial (vowel "a") | Karoryfer (D. Smolken) | LIC CC0 (2025-09-09) | GREEN | 156.1 MB (v0.100) | readme: "the a vowel from the Hadzi-Fia sample library by Karoryfer Samples", legato patches | a CC0 slice of a commercial vocal library; one vowel only | github.com/sfzinstruments/legato_vocal_tutorial |
| Marie Ork | Karoryfer | Free page: "Only Marie Ork has a different license"; her own page (shop.karoryfer.com/pages/free-marie-ork) states no licence; the licence is inside a 652 MB zip (not downloaded) | REJECT (licence not readable; Plogue Alter/Ego monolithic voice bank our engine cannot use anyway) | 652 MB | four voices (clean singing, talker, growl, space), 50+ presets | 2016 KVR DC entry | shop.karoryfer.com/pages/free-marie-ork |

## 7. World instruments

| Library | Publisher/author | Licence | Class | Size | Samples | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| Horse Pulse | Karoryfer | LIC CC0 (2022-03-30); page: "Open source, released under CC license." (variant unnamed; LICENSE is CC0) | GREEN | 140.7 MB (v1.000) | "Over 500" samples; five pizzicato types; "Pluck ... recorded with six round robins" | bass tagelharpa | github.com/sfzinstruments/Karoryfer.HorsePulse |
| Gogodze Phu Vol I | Karoryfer | LIC CC0 2022-11-04 (CC BY 4.0 before) | GREEN | 29.8 MB (v1.001) | five Ghanaian bobobo drums + cajon (5 strike spots, rolls) | small; core candidate for hand drums | github.com/sfzinstruments/karoryfer.gogodze-phu-vol-i |
| jSteelDrum v2 | Jeff Learman | LIC "This is free and unencumbered software released into the public domain." (Unlicense, 2021-10-12; repo pushed 2026-10-01) | GREEN | repo 38.8 MB | "24 notes sampled", "5 velocity layers", "3 or 4 samples per zone" | v1 settled CC0; v2 is Unlicense (also GREEN) | github.com/sfzinstruments/jlearman.SteelDrum |
| GM bank 105 Sitar | Dr. Narayan Bhagawan Raikar | header: "License: CC0, Dr. Narayan Bhagawan Raikar"; readme: "contributed by Dr. Narayan Bhagawan Raikar with CC0 license. Will need testing for quality etc." (2021-10-13) | GREEN | 45 files, 9.7 MB | normal + sliding patches | untested quality | Discord-SFZ-GM-Bank |
| Starbirth Kudu Shofar | Martin Schiff | README: "you MAY NOT sell the samples, or include them in a compilation for samples for sale." (2020-10-08) | REJECT (no commercial redistribution of samples) | repo 7 MB | three traditional calls | | github.com/sfzinstruments/Starbirth.KuduShofar |
| Kastendieck Steel Drum | M. Kastendieck | no licence (confirmed, 2020-10-09) | REJECT (settled) | repo 15.4 MB | 3 velocity levels | | github.com/sfzinstruments/Kastendieck.SteelDrum |
| (Hungarian zither, cithara barbarica, erhu: see families 2 and 3) | | | | | | | |

## 8. Accordion and free reeds

None in the org, the Karoryfer free list or the GM bank (GM slots 022 Accordion, 023 Harmonica, 024 Tango
Accordion are 30-byte placeholders). Weresax's "saxcordion" is a fake accordion made from sax samples.

## 9. Drums and percussion

| Library | Publisher/author | Licence | Class | Size | Samples | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| Big Rusty Drums | Karoryfer | LIC CC0 (2023-04-29) (settled) | GREEN | 590.9 MB (v1.100) | 1980s Polish kit | settled, on disk | github.com/sfzinstruments/karoryfer.big-rusty-drums |
| Unruly Drums | Karoryfer | LIC CC0 (2023-04-29) | GREEN | 645.0 MB (v1.100) | "Over 5,600" samples; 20" kick, 22"/14"/13" snares, hat at six openings, crash, ride, woodblocks; sticks and brushes | own page: "Every drum in this kit is a snare." | github.com/sfzinstruments/karoryfer.unruly-drums |
| Swirly Drums | Karoryfer | `license` CC0 (2023-04-29) | GREEN | 828.4 MB (v1.104) | "Over 4,700" samples; brushed kit, 5 toms, 6 cymbals, cajon kick, cowbell, djembe, darbouka, bongos | own page: "brushed drums with snare stirs and flutters" | github.com/sfzinstruments/karoryfer.swirly-drums |
| Frankensnare | Karoryfer | `license` CC0 (2023-04-29) | GREEN | 348.7 MB (v2.100) | "Over 3500 samples of snare drums", 17 snares + claps, chiks, tambourine | build-your-own snare | github.com/sfzinstruments/karoryfer.frankensnare |
| The Hat With The Phat | Karoryfer | LIC CC0 (2021-06-09) | GREEN | 635.2 MB (v1.001) | giant hi-hat from two rides | specialist | github.com/sfzinstruments/Karoryfer.TheHatWithThePhat |
| Gogodze Phu Vol II | Karoryfer | LIC CC0 2022-11-04 (CC BY 4.0 before) | GREEN | 133.1 MB (v1.001) | 1,701 samples; kick, snare, 3 deadened toms, hat (5 openings) | variable lo-fi to hi-fi | github.com/sfzinstruments/karoryfer.gogodze-phu-vol-ii |
| Virtuosity Drums | Versilian Studios + Karoryfer | LIC CC0 (2021-07-12) | GREEN | 1170.3 MB (v0.925, 2026-04-21) | jazz kit, sticks, ~40 articulations, "six mixable mic positions"; GM auxiliary percussion "from VSCO 2 Pro" | full GM drum map in CC0; biggest single win for a GM kit | github.com/sfzinstruments/virtuosity_drums |
| Body percussion | D. Smolken | LIC CC0 (2021-07-20); readme "CC 0 license." | GREEN | repo 38.2 MB | "Stereo, 44.1 kHz 24-bit files. 4-6 round robins, sometimes 2 velocity layers." | claps, slaps etc. | github.com/sfzinstruments/body_percussion |
| Ethan Winer Collection | Ethan Winer; SFZ by kinwie | repo: no licence. Author's page ethanwiner.com/ewsf2.html (no page date): "These files are made available for you to use in any way you'd like, royalty free," "including for commercial projects, with no attribution required." | GREEN? (permissive statement, no named licence; like Iowa MIS) | repo 14.8 MB | slit drums, brush snare, tubular bells, claves, glock, shakers, sleigh bells, tambourine, temple blocks, triangle, wind chimes, slide whistle, turtle drum, bassoon, cello | small, broad; core-size | github.com/sfzinstruments/EthanWiner.Soundfonts |
| DrumGizmo DRS Kit | DrumGizmo team; SFZ by kinwie | LIC "Attribution 4.0 International" (2020-05-13; "Update license with permission from Muldjord" 2020-07-03) | CREDIT | repo 736 MB | multi-mic, mono channels, bleed controls | | github.com/sfzinstruments/DrumGizmo.DRSKit |
| DrumGizmo Muldjord Kit | DrumGizmo team | LIC CC BY 4.0 (2020-05-13) | CREDIT | repo 337 MB | recorded 2010 | | github.com/sfzinstruments/DrumGizmo.MuldjordKit |
| Naked Drums | Wilkinson Audio; SFZ by kinwie | LIC CC BY 4.0 (2020-06-30) | CREDIT | repo 1.25 GB | "10 round-robins, up to 5 velocity layers and several mic layers"; 16 mics | | github.com/sfzinstruments/WilkinsonAudio.NakedDrums |
| Dim Cabasa | kinwie (played by Dimas Syah) | LIC CC BY 4.0 (2020-04-28) | CREDIT | repo 10.5 MB | "250 direct, unprocessed samples 24bit 48kHz" | | github.com/sfzinstruments/kinwie.dim-cabasa |
| Sam's Sonor | Sam Greene | LIC "Attribution-ShareAlike 4.0 International" (2020-10-10) | SHARE-ALIKE | repo 35 MB | "at least 4 velocity levels for each sound" | | github.com/sfzinstruments/SamsSonor |
| SM Drums | Scott McLean, Tod Stillwell, Suleiman Ali | no licence in repo; author site smmdrums.wordpress.com states none ("the Sforzando vst/standalone is free, as is this drum library."); sfzinstruments site labels it "Public Domain" without a source | REJECT (no stated licence; site label unsupported) | repo 2.35 GB | "many velocity layer", "plenty of round robins" | | github.com/sfzinstruments/SMDrums |
| Terkelsen's Marimba | Lars Terkelsen (Giga conv. S. C. Collins) | no licence (repo 2022-10-17); site leaves licence blank | REJECT (no licence) | repo 24.6 MB | 3 velocity layers, every note | good marimba if a licence were found | github.com/sfzinstruments/Terkelsen.Marimba |
| GM bank Drums 001 Standard Kit | kinwie (2020-10-17/18) | no licence line anywhere in the 25 KB .sfz; README lists "freesound.org" among sources; file names include freesound user tags (__seidhepriest__, __lizellekleynhans__, __ethanchase) and "Roland-SC-" clicks | REJECT (no licence; mixed sources) | 224 files, 115.5 MB | | the other 9 GM kits are placeholders | Discord-SFZ-GM-Bank |

## 10. Drum machines

| Library | Publisher/author | Licence | Class | Size | Notes | URL |
|---|---|---|---|---|---|---|
| Tic Tok Men Moogdrums 1 | Seven Graylands | no licence (2020-10-30) | REJECT (no licence) | repo 1.8 MB | 79 Moog Rogue drum sounds | github.com/sfzinstruments/TicTokMen.Moogdrums1 |
| Tic Tok Men Retro Drums 1 | Seven Graylands | README: "Not all recordings were done in house (but all are free use licenses)." (2020-10-09) | REJECT (no named licence; third-party recordings of Casio, Roland CR-8000 etc.) | repo 1.9 MB | | github.com/sfzinstruments/TicTokMen.RetroDrums1 |

## 11. Synth one-shots (sample-based synths)

| Library | Publisher/author | Licence | Class | Size | Samples | Notes | URL |
|---|---|---|---|---|---|---|---|
| Caveman Cosmonaut | Karoryfer | LIC CC0 2022-11-04; before: "GNU GENERAL PUBLIC LICENSE Version 3" from 2019-12-28 | GREEN (v1.001) | 93.0 MB (v1.001) | 370 samples of a 1983 Unitra B-11 transistor organ | own page: "great raw material for an old-school synth" | github.com/sfzinstruments/karoryfer.caveman-cosmonaut |
| String Cyborgs | Karoryfer | LIC CC0 2022-11-04 (CC BY 4.0 before) | GREEN | 60.8 MB (v1.001) | looped double bass and cello as oscillators (Zinc, Blackheart, Ironface) | | github.com/sfzinstruments/karoryfer.string-cyborgs |
| Cowsynth | Karoryfer | LIC CC0 2022-11-04 (CC BY 4.0 before) | GREEN | 13.4 MB (v1.001) | replica medieval bagpipes as oscillators; organ, pad, pluck | core-size | github.com/sfzinstruments/karoryfer.cowsynth |

## 12. Sound effects for music

| Library | Publisher/author | Licence | Class | Size | Notes | URL |
|---|---|---|---|---|---|---|
| Zamenhof Stickmusterstuch Error (Eins + Zwei) | Karoryfer (credited to "Ludwik Zamenhof") | LIC CC0 (2021-06-16) | GREEN | 75.4 MB (v1.000) | "A double library of glitches" | github.com/sfzinstruments/zamenhof_stickmusterstuch_error |
| G-Town Church Sampling Project | Tobias Marberger (conv. S. C. Collins) | README: "licensed under the Creative Commons Sampling Plus 1.0 license" | REJECT (Sampling Plus) | repo 446 MB | church instruments and sounds | github.com/sfzinstruments/GTownChurchSamplingProject |

## Org repositories that are not sample libraries

`mappings` (SFZ mappings for samples hosted elsewhere, no licence), `sfzinstruments.github.io` (website, CC0),
`sfztemplates` (blank SFZ templates, CC0).

Counted: 76 org repos = 73 sample repos + 3 tooling. GM bank: 12 real instruments (11 melodic incl. the
unmapped bagpipe, 1 drum kit). Of its 138 slot .sfz files, 11 are real and 127 (the bagpipe's included) are
30-byte placeholders.

## Leads from the sfzinstruments site listing (NOT in the org; site's label only, own licence NOT read)

From data/sfz/instruments.yml (2025-07-26), some of them are covered in [libraries-2-orchestral-keys-world.md](libraries-2-orchestral-keys-world.md) and [libraries-3-drums-synths-effects.md](libraries-3-drums-synths-effects.md):
- Malaclypse the Younger, site label "MIT", all on Google Drive: Kalimbass (electric kalimba bass), Blue Jeans
  And Moonbeams (guitars), The Clap (hand percussion as a kit), Breezy Day (wind chime), Tofurkey (no-sample
  synth), Party Pipes (winds). Info doc: docs.google.com/document/d/15XBK9LqTsNRG14JD23bidiAvaEaj8UXcT8O8PDTnS3o
- SHLD Music, label "CC0-1.0": Minifreak Pads (265 MB), Wavestate Pads (160 MB), drolez.com/blog/music/ableton-free-sound-packs.php
- Versilian Studios, label "CC0-1.0": Etherealwinds Harp II CE (200 MB), versilian-studios.com/etherealwinds-harp/
- Bandshed MSLP Vibes, label "CC-BY-3.0", 19 MB, bandshed.net/sounds/sfz/
- Accurate-Salamander Project (Chisato Yamauchi), label "CC-BY", 1.6 GB
- Salamander Drumkit, label "CC-BY-SA-3.0"; KeyPleezer LivingRoom Upright Micro, "CC-BY-SA-4.0"; BJDNielPercussions, "CC-BY-SA-4.0"
- MF Tin Whistle, "CC BY-NC-SA 3.0 (with modifications)" (reject); Unreal Instruments (Standard Bass/Guitar, Koto,
  1912, Metal GTX, Slapper, Epic Tom, Kitchen X, Wind Chime): "Custom"; VGTrumpet Northern Trumpets: "Free";
  Wave Alchemy MFB Tanzbar and BPB Game Boy Drum Kit: blank; Plogue Sforzatron (Taijiguy Mellotron): blank;
  Virtual Playing Orchestra: blank (settled as mixed).

---

## Sources not reached

- Old KVR listing for Shinyguitar (hoped: the "GPL 3" wording and its date). kvraudio.com/product/shinyguitar-by-karoryfer-samples
  landed on a product list with no licence text; not searched further. Repo history (above) shows no GPL for Shinyguitar.
- Marie Ork's licence: not on shop.karoryfer.com/pages/free-marie-ork; it ships inside
  www.karoryfer.com/karoryfer-samples/wydawnictwa/marie-ork/zip (application/zip, 684,363,661 bytes per a HEAD request): not downloaded.
- freesound.org MTG packs 20251, 20239, 20247, 20253 (hoped: the original licence of MTG Solo Sax samples): barred by robots.txt, needs a person's look.
- Ixox Flute author page xhosxe.free.fr/ixoxflute.html (hoped: the original SoundFont's licence): not read.
- Akai's own statement for the Splendid Grand Piano public-domain release: not found (claim only in kinwie's README).
- Release zip contents for every Karoryfer library (hoped: confirm each post-2022-11-04 zip carries the CC0 file): not downloaded by rule.
- Karoryfer product pages not individually read (repo LICENSE/README used instead): Emilyguitar, Ergo, Fashionbass,
  Growlybass, Swagbass, Pastabass, Cowsynth, Zamenhof, 272 Merry Orks, D. Smolken double bass, String Cyborgs,
  Scarypiano, The Hat With The Phat, Gogodze Phu Vol I, Black And Green Guitars.
- Karoryfer drum kit user guides (PDF) for velocity/round-robin counts: not opened.

## Top picks per family

- **Keys**: Osiris Piano (CC0, 437 MB, 1,200+ samples, quiet, multi-mic) for a pack; GM 002 VCSL upright subset (CC0, 14.7 MB) for core. Credit option: Headroom Piano (CC BY 4.0, 300 samples, 5 layers, 156 MB).
- **Guitars and plucked**: Shinyguitar (CC0, 351 MB, up to 846 samples, pickup + mic) and Black And Green (settled); basses: Fashionbass (CC0, 5 layers x 3 RR, 302 MB) with its GM subset (034, 21.5 MB, CC0) as the core bass; Ganjo (CC0, 23 MB) and Hungarian zither (CC0, 169 MB) for colour.
- **Bowed strings**: Karoryfer x bigcat cello (CC0, 479 samples, 127 MB) and Meatbass (CC0, arco 5 layers x 2 RR, pizz 4 x 4, 244 MB); GM 045/046 Versilian ensembles (CC0, 10.5 + 6.1 MB) for core.
- **Brass**: War Tuba (CC0, 1000+ samples, 104 MB), the only free brass here.
- **Woodwinds and reeds**: Bear Sax (CC0, bari, 125 MB) and Weresax (CC0, alto, 256 samples, 188 MB); Shanai (CC0, 8.6 MB) for a small reed.
- **Voices and choirs**: Legato vocal tutorial (CC0, one "a" vowel with legato, 156 MB); nothing choir-like.
- **World instruments**: Gogodze Phu Vol I (CC0, bobobo + cajon, 30 MB), Horse Pulse (CC0, 141 MB), Sitar (CC0, 9.7 MB, untested), jSteelDrum v2 (Unlicense, 39 MB).
- **Accordion and free reeds**: none found.
- **Drums and percussion**: Virtuosity Drums (CC0, full GM map, 6 mics, 1.17 GB) as the pack kit; Big Rusty Drums (settled) and Unruly/Swirly for variety; Ethan Winer Collection (permissive statement, 14.8 MB) and Body percussion (CC0, 38 MB) for small aux percussion.
- **Drum machines**: none usable (Tic Tok Men sets unlicensed).
- **Synth one-shots**: Cowsynth (CC0, 13.4 MB) for core, Caveman Cosmonaut (CC0, 93 MB) for a pack.
- **Sound effects for music**: Zamenhof Stickmusterstuch Error (CC0, 75 MB glitches).
