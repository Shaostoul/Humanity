# Free instrument libraries, part 3: drum kits, drum machines, synths and sound effects for music

> Part 3 of the free instrument library research of **13 November 2026**: the plain-words summary, the
> ranking and the recommendation are in [README.md](README.md); the licences that matter, with their
> quotations, are in the dated finding [2026-11-13-free-instrument-libraries.md](../../findings/2026-11-13-free-instrument-libraries.md). This is an AI's
> reading of each licence, not legal advice: every licence is quoted so a reader can check it. Gathered by
> four research passes on 2026-11-13 and spot-checked the same day (licence files re-read through the
> GitHub API, the TinySOL Zenodo page, Greg Sullivan's site).

Read on 2026-11-13. Every licence below was read from the source named in its row, never from memory.
GitHub was read only through `gh api`; no sample, archive or soundfont was downloaded (sizes come from
API metadata, page text or HTTP HEAD headers). Covered in [libraries-1-sfz-karoryfer.md](libraries-1-sfz-karoryfer.md) and [libraries-2-orchestral-keys-world.md](libraries-2-orchestral-keys-world.md) instead: the
sfzinstruments GitHub organisation's repositories (incl. its SFZ conversions of DRSKit, MuldjordKit,
Naked Drums, Sam's Sonor, Virtuosity Drums), Karoryfer (incl. Swirly/Unruly/Big Rusty Drums,
Cowsynth, Caveman Cosmonaut), the Discord-SFZ-GM-Bank, FreePats, Iowa, pianos, orchestral, voices,
world, accordion. VCSL (on disk) already covers snares, toms, bass drums, hi-hat, cymbals, claps,
shakers, tambourines, cowbells, woodblock, agogo, cabasa, guiro, vibraslap: "already on disk".

## 1. Acoustic drum kits

| Library | Publisher/author | Licence (exact quote; where read; source date) | Class | Size | Samples, dynamics, round robins | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| DRSKit 2.1 | DrumGizmo team + Jes Eiler (DRSDrums) | "The DRSKit drumkit source files are released under the Creative Commons Attribution 4.0 International License." Also: "any context in which other credits (e.g. to performers) are presented must include an attribution." (drumgizmo.org wiki kits:drskit, last modified 2021-03-22) | CREDIT | 2,673.5 MiB zip (HEAD, file dated 2018-11-28); SFZ conversion 754.2 MB in the sfzinstruments org ([libraries-1-sfz-karoryfer.md](libraries-1-sfz-karoryfer.md)) | 13 mono mic channels (close, OH, room); multi-hit, velocity-sorted DrumGizmo format; per-drum hit counts not stated | own page: "The kit should be usable for everything from jazz to rock." Paiste 602 hats/ride | https://drumgizmo.org/wiki/doku.php?id=kits:drskit |
| CrocellKit 1.1 | DrumGizmo team (band Crocell, JBOSound studio) | "The CrocellKit drumkit is released under the Creative Commons Attribution 4.0 International License." (wiki kits:crocellkit, last modified 2023-07-16) | CREDIT | 5,384.9 MiB zip ("5.5 Gb") | 15 channels; double-pedal kick, 3 crashes, 2 china, 2 splash | own page: "This should be considered a metal or rock kit." | https://drumgizmo.org/wiki/doku.php?id=kits:crocellkit |
| MuldjordKit 3 | Lars Muldjord / DrumGizmo | CC BY 4.0, and "When used in compositions, attribution must be made in the album credits (or similar album liner notes) as "Drum samples provided by DrumGizmo.org"." (wiki kits:muldjordkit, last modified 2021-01-10) | CREDIT | 2,419.1 MiB zip; SFZ conversion 347.3 MB (sfzinstruments org) | 16 channels; Tama Superstar, 2 kicks, 3 rack toms | own page: "a metal or rock kit"; two known dual-hit bugs in low velocity layers | https://drumgizmo.org/wiki/doku.php?id=kits:muldjordkit |
| The Aasimonster 2.1 | DrumGizmo (DIE / Execution Room) | "The Aasimonster drumkit source files are released under the Creative Commons Attribution 4.0 International License." (wiki, last modified 2021-03-22) | CREDIT | 2,304.4 MiB zip | 14 mics + trigger channels | death-metal kit; own page notes ambience/kick timing double-hits | https://drumgizmo.org/wiki/doku.php?id=kits:the_aasimonster |
| ShittyKit 1.2 | Sardonicus | "The ShittyKit drumkit source files are released under the Attribution 4.0 International license." (wiki, last modified 2019-10-18) | CREDIT | 352.5 MiB zip | 5 channels; own page: "old drumkit format with fixed velocity groups" | Stagg kit, unprocessed | https://drumgizmo.org/wiki/doku.php?id=kits:shittykit |
| Sommerhack 2016 Kit | DrumGizmo | CC BY 4.0; "When using the samples in a composition, attributions must be made in the album credits." (wiki, last modified 2021-01-07) | CREDIT | 11.5 MiB zip | 2 channels; clap, slapped chair, log, glass, mug, finger snap | found-object percussion | https://drumgizmo.org/wiki/doku.php?id=kits:sommerhack-kit |
| Andreyas Drums Sound Like This (Shortcircuit XT factory content) | Surge Synth Team contributors | Repo README: "By submitting your samples here you are making them available under CC0 in perpituity." [sic] (README commit 2025-12-31; no LICENSE file, GitHub licence field empty) | GREEN (by contribution terms; no LICENSE file) | 67.0 MiB, 261 WAV | file names show pp/p/m/f/ff layers and rr1-rr4 (crash, closed hat, flam ...) | Shortcircuit XT part (.scp), not SFZ; mapping is in the file names | https://github.com/surge-synthesizer/shortcircuit-factory-content |
| LMMS assets sample library (drums part) | LMMS project contributors (AUTHORS: Markus Hakala kicks, Subaqueous acoustic/perc, Deficio risers, Ian Sannar acoustic drums/foley) | Repo licence CC0-1.0; README: "LMMS will only accept `CC0` licenced content." Samples/README: "Samples must be created/recorded by you." (last Samples commit 2021-07-09) | GREEN | 43.1 MiB drums (379 FLAC); whole library ~49 MiB | one-shots, numbered variants not velocity layers: 56 kicks, 81 snares, 59 hats (incl. "Jazz CHH/OHH"), 26 crashes, 13 rides, 28 toms, 15 claps, hand drums, bells | mix of acoustic and electronic styles (house, DnB, hardstyle kicks) | https://github.com/LMMS/assets |
| Ben Burnes / Mark Powers brushed packs (Brushed Drum, Brushed Toms, Brushed Cymbals, Stacked Cymbals) | Ben Burnes with drummer Mark Powers | "These sounds are public domain (Creative Commons 0). You are free to enhance, reuse, remix, or do whatever with these files." (each Gumroad page; no date shown) | GREEN, but sold (a price per pack) | not shown | Brushed Drum: "over 150 individual brushed drum samples, 28 unique loops" (snare, Vic Firth Gadd + Live Wires brushes); Brushed Cymbals: "12 different cymbals ... over 350 unique sounds"; Stacked: 240 samples, five-sample round robin | the only clearly CC0 BRUSH sounds found (electro swing / lo-fi). Paid: noted, not proposed | https://ben-burnes.gumroad.com/l/bb_brushed |
| Virtuosity Drums | Versilian Studios | own page: "Virtuosity Drums is an open source (Creative Commons 0) contemporary Jazz drum kit library, performed with sticks" (page modified 2026-05-11) | GREEN (repo in the sfzinstruments org: [libraries-1-sfz-karoryfer.md](libraries-1-sfz-karoryfer.md)) | 1.1 GB | "Up to 36 Dynamic Layers" (wave technique, no RR) on drums; cymbals 4 RR; 6 mic positions | jazz kit but STICKS ONLY, no brushes | https://versilian-studios.com/virtuosity-drums/ |
| AVL Drumkits (Black Pearl, Blonde Bop, Blonde Bop HotRod, Buskman's Holiday, Red Zeppelin) | Glen MacArthur | x42/avldrums.lv2 sf2/README: "CC-BY-SA with the following exception: The by-sa condition is imperative if you modify the samples themselves or create new sample libraries" + link to by-sa/3.0 (commit 2023-09-06). Hydrogen index: "CC BY-SA" (2026-05-27) | SHARE-ALIKE | 15-24 MB each (Hydrogen); 19-44 MB each as SF2 | multi-velocity (README: "Reordered all affected samples that were in incorrect velocity order") | own site bandshed.net not read (robots bars ClaudeBot) | https://github.com/x42/avldrums.lv2 |
| Salamander Drumkit v1 | Alexander Holm | README quoting original: "Licence: [CC BY-SA](http://creativecommons.org/licenses/by-sa/3.0/)"; "The "share-alike" condition in the license only applies if you modify the samples themselves or create new sample libraries with them." (endolith repo, pushed 2026-05-11); archive.org licenseurl by-sa/3.0 (added 2012-02-25) | SHARE-ALIKE | 369.7 MiB (archive.org) | author: "It doesn't have many velocity layers, but instead many samples per velocity"; closed hat 20 per velocity x 2 layers | home-built birch kit | https://archive.org/details/SalamanderDrumkit |
| SM Drums 1.2 (SFZ) | Scott McLean, Tod Stillwell, Suleiman Ali | NO licence on its own pages (/, /about/, /for-sfz-sforzando/, page dated 2018-03-12). sfzinstruments catalog (instruments.yml, 2025-07-26) says license: "Public Domain" (secondary) | REJECT until a person reads the licence inside the download | 2.2 GB | own page: "many velocity layer and plenty of round robins" | 60s Ludwig kit (secondary) | https://smmdrums.wordpress.com/for-sfz-sforzando/ |
| Hydrogen GMRockKit (bundled) | Glen MacArthur / Sebastian Moors | drumkit.xml `<license>GPL</license>` (hydrogen repo) | SHARE-ALIKE | 10.9 MiB | 87 files | Hydrogen default kit | https://github.com/hydrogen-music/hydrogen/tree/main/data/drumkits |
| Hydrogen downloadable kits, acoustic (feed index.json created 2026-05-27; drumkit_list.php commit 2023-10-03) | various | BJA_Pacific "Attribution-Share Alike 3.0 United States"; Forzee Stereo "distributed under the GPL version 2 license" (124 MiB); The Black Pearl 1.0 / BeatBuddy "GPL"; K-27 Trash Kit "Green Open Music License (copylef..."; DeathMetal "CC-Attribution" (15 MiB) | SHARE-ALIKE (CREDIT for DeathMetal) | 1-124 MiB | Hydrogen .h2drumkit layers | DeathMetal is a metal kit | https://github.com/hydrogen-music/hydrogen-music/blob/master/feeds/index.json |
| Hydrogen: Guilherme Passos kits (Dave Grohl, Ian Paice, John Bonham, Travis Barker, Tre Cool) | Guilherme Passos | feed: "CC BY" | CREDIT on paper, provenance doubtful | ~1 MB each | unknown | kits named after famous drummers at ~1 MB each: may be lifted from their records; do not use without provenance | same feed |
| Hydrogen: JazzFunkKit / Orange Tree Jazz Funk | Orange Tree Samples | feed: "Copyright 2008 Orange Tree Samples, All Rights Reserved"; own licence page: "You may not copy, edit, distribute or sell the original soundsets without the written permission of Orange Tree Samples." (no date) | REJECT | 37.6 MiB | small jazz kit | jazz kit, sadly reserved | https://www.orangetreesamples.com/license-agreement |
| Hydrogen: GSCW Kit 1 and 2 | Stefano Carbonelli | "Do not redistribute or publish the inluded samples." [sic] | REJECT | 15-17 MiB | | | feed |
| Hydrogen kits with an empty licence field (ColomboAcousticDrumkit, YamahaVintageKit, Erny's percussion, Millo 1/2/3, Lightning1024 = "Free!!!", Drumkit excepcional = "creative commons") | various | no licence (or no named licence) in the feed | REJECT (no stated licence); a person could check each archive's drumkit.xml | 0.6-6 MiB | | | feed |
| GeneralUser GS 2.0.3 (has "40: Brush Kit", "24: Electronic Kit", "25: 808/909 Kit") | S. Christian Collins | "Please feel free to use it in your software projects, and to modify the SoundFont bank or its packaging to suit your needs." BUT "I cannot be 100% sure where all of the samples originated" (documentation/LICENSE.txt, commit 2026-02-23) | GREEN grant, provenance uncertain: flag | 30.8 MiB SF2 | 261 presets, 13 kits | the only free Brush Kit found besides Ben Burnes | https://github.com/mrbumpy409/GeneralUser-GS |

## 2. Drum machines

| Library | Publisher/author | Licence (exact quote; where read; source date) | Class | Size | Samples | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| Sonic Pi bundled samples | Sonic Pi (sounds from freesound + Arovane + The Black Dog) | LICENSE.md: "All the bundled samples are individually licensed under a CC0 1.0 Universal Public Domain Dedication." (commit 2026-09-23); etc/samples/README.md lists each freesound source (2025-06-08). Repo licence field NOASSERTION (mixed code licences) | GREEN, with exclusions | 33.4 MiB, 206 FLAC | single hits: bd_808 + 14 kicks, 21 hats, 20 acoustic drum_*, 25 elec_*, 4 vinyl_*, 17 loops, 26 tabla | EXCLUDE loop_amen / loop_amen_full: an Amen-break recording is not the uploader's to dedicate; per-file freesound pages need a person's look | https://github.com/sonic-pi-net/sonic-pi/tree/dev/etc/samples |
| TidalCycles Dirt-Samples | TidalCycles community | NO licence file. Issue #19 "Add a license", Alex McLean (yaxu) 2020-04-30: "A lot of them have unknown provenance. Tidying them up would probably involve removing most of them." | REJECT | 174.5 MB repo, 218 folders | 808, 909, breaks, etc. | | https://github.com/tidalcycles/Dirt-Samples |
| tidal-drum-machines (Strudel's drum machines) | geikha (was ritchse) | NO licence file. Issue #2, geikha 2026-10-26: "Practically all drum machines are from a pack that grew organically while Google Drive links were shared around back in the day" and "it's practically impossible to 100% verify" | REJECT | 172.6 MB | dozens of machines | its stated source, archive.org "drum-machines-collection" (3,895 MiB), carries no licence in its metadata | https://github.com/geikha/tidal-drum-machines |
| dough-samples (Strudel defaults) | Felix Roos | no repo licence; README credits only piano (Salamander, CC BY), VCSL (CC0), mridangam (CC BY-SA 4.0); drum machines and EmuSP12 come from tidal-drum-machines | REJECT for the drum machines | 5.4 MB repo + submodules | | | https://github.com/felixroos/dough-samples |
| Dough-Samples (Topos default pack) | Bubobubobubobubo | GitHub licence GPL-3.0; README gives no provenance | SHARE-ALIKE, provenance unknown | 117.5 MB | kick, snare, hat, fx, synth folders | | https://github.com/Bubobubobubobubo/Dough-Samples |
| yaxu/clean-breaks, yaxu/spicule, felixroos/samples, Dough-Waveforms, Dough-Amiga | various | no licence, no README licence | REJECT | 5-200 MB | | breaks are usually lifted from records | GitHub |
| LMMS factory samples (data/samples in the app repo) | LMMS | repo GPL-2.0 but maintainer Spekular, issue #5126, 2019-08-15: "we don't know where exactly the samples are from and what license they're under. In essence, all of LMMS's stock samples are questionably legal." | REJECT | small | drums, drumsynth, basses, beats, effects | replaced by LMMS/assets (CC0, section 1) | https://github.com/LMMS/lmms/issues/5126 |
| Hydrogen TR808EmulationKit (bundled) | ArtemioLabs | drumkit.xml `<license>GPL</license>` (file commit 2026-03-27) | SHARE-ALIKE | 0.2 MiB, 17 files | 808-style emulation | | https://github.com/hydrogen-music/hydrogen/tree/main/data/drumkits/TR808EmulationKit |
| Hydrogen feed: electronic kits | various | rumpf_kit_z01_gm "cc-by ==> http://creativecommons.org/licenses/by/3.0/" ("Drum kit from analog samples", 7.0 MiB); SF3007-2011-Set-03 "What the Fuck Public License (WTFPL)" (5.8 MiB, contents not described); Audiophob "Creative Commons 0 License" (0.7 MiB, freesound sounds); Denon CRB-90 "CC-BY-SA" (0.3 MiB); ElectricEmpireKit, HardElectro1, VariBreaks "GPL" | CREDIT / GREEN / GREEN / SHARE-ALIKE / SHARE-ALIKE | 0.2-7 MiB | | small; quality unknown | feed (above) |
| Hydrogen feed: Roland MC-307 kits (CR78&Cheaps, TR-606, TR-808, TR-909, Techno1) | Michael Wolkstein | "Do not redistribute or publish the inluded samples." | REJECT | 23-35 MiB | | | feed |
| Hydrogen feed: Artemio / ArtemioLabs kits (Classic-626, TR808909, Boss DR-110, 3355606kit, HipHop-1/2, Techno-1, Synthie-1, TD-7kit, EasternHop-1) | Artemiy Pavlov, Ed Vargo | licence field empty | REJECT (no stated licence) | 0.2-0.9 MiB | | | feed |
| Michael Fischer TR-808 Sound Sample Set 1.0.0 (machines.hyperreal.org; repackaged in fluid-music/open-drums) | Michael Fischer | TR808.TXT (09/08/94): "these samples are ABSOLUTELY FREE!" No licence grant. fluid-music README: "no licensing restrictions" (its reading). open-drums repo has no licence | REJECT (free of charge, no licence) | 116 samples | 25 BD, 25 SD, 25 cymbal at 5 knob positions | the classic 1994 808 set | https://github.com/fluid-music/open-drums |
| MusicRadar SampleRadar (e.g. "167 free processed 808 and 909 samples") | Future plc | "you're welcome to use the samples in your music in any way you like - all we ask is that you don't re-distribute them." (published 2025-08-12) | REJECT | | | applies to all SampleRadar packs incl. "935 free analogue polysynths" | https://www.musicradar.com/music-tech/samples/sampleradar-167-free-processed-808-and-909-samples |
| 99Sounds (all packs) | 99Sounds | "You do not have the right to sell or redistribute (even for free) the audio files downloaded from 99Sounds on their own"; also bars "sleep sound apps" (licence page modified 2023-07-25) | REJECT | | | | https://99sounds.org/license/ |
| Samples From Mars (freebies and shop) | Samples From Mars | "You may not re-distribute these products or sounds in any way, free or paid" and "you may not use these sounds in any website, application, or software." (FAQ, no date) | REJECT | | | | https://samplesfrommars.com/pages/faq |
| Wave Alchemy free MFB Tanzbar samples | Wave Alchemy | no licence on the free page; site terms: "all our intellectual property rights are reserved." | REJECT | 6.5 MB | | | https://www.wavealchemy.co.uk/blog/free-mfb-tanzbar-drum-samples/ |
| Goldbaby freebies | Goldbaby | not read: robots.txt (disallows ClaudeBot) | not assessed | | | | https://goldbaby.co.nz |
| SHLD Pro-1 analog drum kit, Alesis SR-16 kits | SHLD Music (Ludo, drolez.com) | "100% royalty-free , licensed under CC0-1.0" (page "Updated: Thu 09 May 2024") | Pro-1 kit GREEN; SR-16 kits doubtful | small (13 Pro-1 drum samples) | | SR-16 kits are recordings of Alesis' ROM samples: a CC0 from SHLD cannot cover Alesis' recordings | https://drolez.com/blog/music/ableton-free-sound-packs.php |

### Are recordings of a drum machine's factory sounds copyrightable? (primary sources read 2026-11-13)

- US Copyright Office, Circular 56 "Copyright Registration for Sound Recordings" (revised 03/2021):
  "Short sound recordings may lack a sufficient amount of authorship to warrant copyright protection,
  just as words and short textual phrases are not copyrightable." And: "Sound recordings captured by
  purely mechanical means without originality of any kind also lack a sufficient amount of authorship".
  "May lack" is guidance, not a rule that one-shots are free.
- US courts split on tiny samples: VMG Salsoul v. Ciccone (9th Cir., opinion filed 2016-06-02, PDF on
  cdn.ca9.uscourts.gov) describes Bridgeport (6th Cir. 2005) as "For copyrighted sound recordings, any
  unauthorized copying--no matter how trivial--constitutes infringement." and itself holds "the "de
  minimis" exception applies to infringement actions concerning" sound recordings. De minimis is about
  a sample buried in a new work, not about redistributing whole sample files, so it never helps a pack.
- EU: CJEU press release No 98/19 (Luxembourg, 29 July 2019), Case C-476/17 Pelham (Kraftwerk, a
  2-second rhythm sequence): "the reproduction by a user of a sound sample, even if very short, taken
  from a phonogram must, in principle, be regarded as a reproduction `in part' of that phonogram".
  The phonogram producer's right needs no originality, so in the EU a recording of a single hit is
  protected whoever made it. (EUR-Lex not read: its robots.txt disallows everyone; the full judgment on
  curia redirects to a script-only page; the press release PDF was read.)
- Roland, current products: SP-404MKII Reference Manual v5.50 "Intellectual Property Right" page
  (static.roland.com, no date): "The copyright of content in this product (the sound waveform data,
  style data, accompaniment patterns, phrase data, audio loops and image data) is reserved by Roland
  Corporation." and purchasers are "NOT permitted to extract said content in original or modified form,
  for the purpose of distributing recorded medium of said content or making them available on a
  computer network."
- NOT found: any Roland statement about recordings of its ANALOG machines (TR-808, TR-606, CR-78), and
  any court case about drum-machine one-shots as such.
- Reading for us: for a sample-ROM machine the factory sounds are themselves the maker's recordings, so a
  re-recording reproduces them; for an analog machine the only recording is the recordist's (thin in the
  US, fully protected in the EU). Either way a pack is only safe with an explicit licence from the
  recordist, and none of the classic-machine sets above has one. Since the engine can synthesise
  808/909/CR-78-style voices from published circuit descriptions, no drum-machine sample set is worth it.

## 3. Synth one-shots, multisampled synths, wavetables

| Library | Publisher/author | Licence (exact quote; where read; source date) | Class | Size | Samples | Quality notes | URL |
|---|---|---|---|---|---|---|---|
| AKWF (Adventure Kid Waveforms) FREE | Kristoffer Ekstrand | LICENSE.md "CC0 1.0 Universal" (commit 2018-09-09); GitHub CC0-1.0 | GREEN, core-sized | 5.8 MiB (4,358 single-cycle WAVs, 600 samples each, 16-bit 44.1 kHz) | single cycles for wavetable/oscillator use; also C and JS arrays | used by Sonic Pi, Elektron, Surge ports | https://github.com/KristofferKarlAxelEkstrand/AKWF-FREE |
| SHLD Minifreak / Pro-1 / Wavestate / Minilogue xd free packs | SHLD Music | "100% royalty-free , licensed under CC0-1.0 , allowing you to use them in your sound design projects or music productions however you please." (drolez.com, updated 2024-05-09) | GREEN (Minifreak, Pro-1); Wavestate doubtful | Minifreak Pads 265 MB, Wavestate Pads 160 MB (sfzinstruments catalog) | SFZ + Ableton; 16/24-bit multisamples | Wavestate is a sample-based synth, so its pads may carry Korg's ROM recordings; downloads are via Patreon posts | https://drolez.com/blog/music/ableton-free-sound-packs.php |
| Sonic Pi tbd_ (The Black Dog) pads/keys/perc | The Black Dog | README: "Donated by The Black Dog under a CC0 license" | GREEN | 11 files (in the 33.4 MiB) | tbd_pad_1-4, tbd_highkey_c4, tbd_fxbed_loop | | Sonic Pi etc/samples |
| Shortcircuit XT factory content: Crushed Clari Syn, Modulated String Machine, Taped Warm Scrapes, Ze Bellas, Violix, Smokey Amp | Surge Synth Team contributors | CC0 by README contribution terms (see section 1) | GREEN (no LICENSE file) | 18.3 / 11.0 / 22.2 / 490.1 / 22.0 / 17.0 MiB | SCXT parts | | https://github.com/surge-synthesizer/shortcircuit-factory-content |
| Malaclypse the Younger instruments (The Clap, Breezy Day, Tofurkey) | Malaclypse the Younger | "The MIT license is used so that anyone distributing changes is forced to take credit for them" (Google Doc catalogue, newest entry 2026-09-19) | GREEN (MIT: keep the notice) | The Clap 4.5 MB | The Clap: hand claps, snaps, forearm hits as a full kit; Tofurkey is script-only (no samples); Breezy Day wind chimes | The Clap suits electro swing hand claps | https://docs.google.com/document/d/15XBK9LqTsNRG14JD23bidiAvaEaj8UXcT8O8PDTnS3o |
| Surge XT factory wavetables | Surge Synth Team | repo LICENSE GPL-3.0; no separate data licence found | SHARE-ALIKE | | Basic, Generated, Sampled, 3rd-party folders | | https://github.com/surge-synthesizer/surge/tree/main/resources/data/wavetables |
| Legowelt sample packs (Jupiter 8, Matrix 1000, ...) | Legowelt | "The samples are free to download and use in your productions" (no redistribution grant; no date) | REJECT | | 500 Jupiter 8 samples etc. | high quality, but no licence to redistribute | https://legowelt.org/samples |
| Karoryfer Cowsynth, Caveman Cosmonaut | Karoryfer | see [libraries-1-sfz-karoryfer.md](libraries-1-sfz-karoryfer.md) | | | | | |
| OpenGameArt CC0 instrument packs | | not read: robots.txt (disallows Claude-User and ClaudeBot) | not assessed | | | | https://opengameart.org |

## 4. Sound effects for music (risers, sweeps, impacts, vinyl, tape stops, reverse cymbals)

| Library | Publisher/author | Licence (exact quote; where read; source date) | Class | Size | Content | Notes | URL |
|---|---|---|---|---|---|---|---|
| LMMS assets: SFXs, Foley, Loops | LMMS contributors (Deficio: risers) | CC0-1.0 (see section 1) | GREEN | SFXs 2.8 MiB (11), Foley 1.4 MiB (25) | Down Sweep, Fast Sweep up, Sweep Riser, Sub Drop, Noise Blast, Hall Kick, Vinyl Backspin, Vinyl Rewind; Drums/Crashes has "Cymbal Swell" (reverse-cymbal-like) | best CC0 riser/vinyl set found | https://github.com/LMMS/assets |
| Sonic Pi vinyl and fx samples | freesound uploaders via Sonic Pi | CC0 (Sonic Pi LICENSE.md) | GREEN (per-file freesound check needs a person's look) | small | vinyl_hiss, vinyl_backspin, vinyl_rewind, vinyl_scratch, ambi_swoosh, perc_swoosh, perc_impact1/2, ambi_dark_woosh | vinyl_hiss is the only vinyl crackle bed found | Sonic Pi etc/samples |
| BigSoundBank | Joseph Sardin | "Share, copy, distribute and communicate the material by all means and in all formats." ... "Creative Commons CC0 1.0 Universal" (licenses.html, no date; applies to sounds marked "Free and Royalty Free") | GREEN, per sound | 3,500+ sounds | mostly foley/ambience; 12 whooshes | site warns that others register CC0 sounds in Content ID | https://bigsoundbank.com/licenses.html |
| BBC Sound Effects | BBC | RemArc Content Terms of Use (PDF 2016-09-27, github bbcarchdev/Remarc): "For non-commercial, personal or research purposes"; other uses "you'll need to get our permission first, and you might have to pay a fee." | REJECT | 30,000+ | | live site's licence page needs JavaScript, not read | https://sound-effects.bbcrewind.co.uk |
| Mixkit sound effects | Envato | Mixkit's own summary page: users must not "Rent, license, sublicense, sell, resell or otherwise commercially exploit or make Mixkit or any item available to any third party" (mixkit.co/llm-info, Last-Modified 2026-11-12; the licence modal itself needs JavaScript) | REJECT | | | | https://mixkit.co/license/ |
| 99Sounds | | see section 2 | REJECT | | | | |
| Pixabay sound effects | | not read: robots.txt (disallows ClaudeBot) | not assessed | | | | https://pixabay.com |
| Sonniss GDC bundles | | could not read: robots.txt and gdc.sonniss.com return HTTP 403 | not assessed | | | | https://sonniss.com |
| Zapsplat | | could not read: licence page returned HTTP 403 (robots.txt itself allows) | not assessed | | | | https://www.zapsplat.com/license-type/standard-license/ |
| Freesound | | needs a person's look (robots bars automated reading) | | | | | https://freesound.org |
| Kenney | | not read: robots.txt bars Claude | | | | | |
| OpenGameArt | | not read: robots.txt | | | | | |

Tape stops, risers, downlifters, sweeps and reverse cymbals are all cheap to SYNTHESISE or derive in
the engine (pitch/speed ramp, filtered noise sweeps, reversed cymbal buffer), so the CC0 sets above
are a starting palette, not a dependency.

## Settled-list cross-checks

- Kastendieck Steel Drum (settled: no licence): corroborated. Malaclypse's catalogue, which redistributes a
  remaster of it, says: "I have no idea what license this is under (although everything seems
  permissive), so I'll stop distributing this if requested".
- Salamander Grand Piano (settled: CC BY 3.0): dough-samples README agrees ("License: CC-by
  <http://creativecommons.org/licenses/by/3.0/> Author: Alexander Holm"). Note the same author's
  Salamander DRUMKIT is CC BY-SA 3.0, not CC BY.
- No contradiction found for any settled entry.

## Sources not reached

- https://www.bandshed.net/avldrumkits/ : robots.txt disallows ClaudeBot (wanted AVL's own licence and whether any kit is brushed). Used x42's README and Hydrogen's index instead.
- https://opengameart.org : robots.txt disallows Claude-User/ClaudeBot (CC0 instrument packs).
- https://pixabay.com : robots.txt disallows ClaudeBot (sound-effects licence).
- https://goldbaby.co.nz : robots.txt disallows ClaudeBot (free drum-machine packs' terms).
- https://sonniss.com, https://gdc.sonniss.com : robots.txt HTTP 403 (GDC bundle licence).
- https://www.zapsplat.com/license-type/standard-license/ : HTTP 403 (licence text).
- https://bedroomproducersblog.com (SM Drums articles, Game Boy Drum Kit) : HTTP 403.
- https://sound-effects.bbcrewind.co.uk/licensing : JavaScript-only page (current licence wording).
- https://mixkit.co/license/ : licence text sits in a JavaScript modal; used mixkit.co/llm-info.
- https://hydrogen-music.org : TLS certificate name mismatch; read the same feed via the GitHub API.
- https://eur-lex.europa.eu : robots.txt disallows everyone (Pelham judgment text); curia judgment page redirects to a script page; read the CJEU press release PDF.
- https://pdsounds.org : page body empty to a plain fetch (public-domain sounds).
- https://freesound.org (Sonic Pi's per-sound sources, a CC0 brushed kit at freesound.org/s/459170) : needs a person's look.
- Wikimedia Commons audio of drum machines: not searched (its API is disallowed in robots.txt; low yield expected).

## Top picks per family

- Acoustic kit, core: LMMS assets drums (CC0, 43 MiB, 379 one-shots; trim to fit): clean licence, recorded-by-contributor rule.
- Acoustic kit, pack: DRSKit (CC BY 4.0, all-round jazz-to-rock; use the 754 MB SFZ conversion) plus Andreyas Drums (CC0, 67 MiB, 5 dynamics x 4 RR).
- Brushes (electro swing, lo-fi): Karoryfer Swirly Drums (CC0, free, 828 MB; [libraries-1-sfz-karoryfer.md](libraries-1-sfz-karoryfer.md)); GeneralUser GS's Brush Kit (permissive, provenance uncertain). The Ben Burnes / Mark Powers packs are CC0 but sold: recorded, never proposed as a purchase.
- Drum machines: synthesise them; no classic-machine sample set has a usable licence. Small CC0 electronic one-shots exist in LMMS assets and Sonic Pi.
- Synths: AKWF (CC0, 5.8 MiB, core) for oscillators; SHLD Minifreak/Pro-1 packs (CC0) for multisampled pads.
- Music SFX: LMMS assets SFXs (CC0, 2.8 MiB) + Sonic Pi vinyl_* (CC0) in core; BigSoundBank (CC0) for more; synthesise the rest.
- Claps: Malaclypse's The Clap (MIT, 4.5 MB).
