# Free instrument libraries, and what electronic music needs from our engine (13 November 2026)

The operator, 2026-11-13: "I'd like to find what free libraries we can to really fill out the music
potential. We'll even want to consider how we could make music like dubstep."

Research only: no code, no data and no downloads were changed or made. The licences that decide what
may ship are in the dated finding
[`../../findings/2026-11-13-free-instrument-libraries.md`](../../findings/2026-11-13-free-instrument-libraries.md).
Every library row with its quotation is in the helper files:
[part 1, the SFZ Instruments organisation, Karoryfer and the GM bank](libraries-1-sfz-karoryfer.md);
[part 2, FreePats, Iowa, pianos, orchestral, voices, world and free reeds](libraries-2-orchestral-keys-world.md);
[part 3, drum kits, drum machines, synths and sound effects](libraries-3-drums-synths-effects.md);
[the six electronic genres and the synthesis references](electronic-genres-and-synthesis.md).
What we already have: [`../../instrument-sources.md`](../../instrument-sources.md) and
[`../../../design/music.md`](../../../design/music.md) (sections 6, 7a and 23).

## 0. In plain words

- **We looked at about 280 free sample collections** and read each one's own licence. Most "free"
  sample packs on the web are free to use in your songs but **not free to give away**, and a game that
  ships samples is giving them away, so those are out. What is left is still a lot.
- **Much of what we want is already on our disk and unused.** The two big public-domain libraries we
  downloaded on 6 November (VCSL and VSCO 2) also hold a tenor saxophone, harmonicas, harpsichords, an
  organ, a marimba, kalimbas, a didgeridoo, snare drums, cymbals, hi-hats, claps, congas and bongos.
  Using them needs no new download, only a recipe in the pipeline.
- **The best new public-domain source is Karoryfer**: a dozen basses, a cello, double basses, two
  saxophones, a tuba, guitars, a brushed drum kit (exactly what electro swing and lo-fi want), more drum
  kits and synth sources, all free to share since 4 November 2022. Next come FreePats (an accordion, a
  bagpipe, a ukulele, a honky-tonk piano, clean electric guitars) and a set of 4,358 tiny single-cycle
  waveforms (AKWF, 5.8 MiB) that would give our synthesiser a wavetable oscillator.
- **Some of the best need a credit line** (CC BY): the first real open CHOIR we have found (epic-choir,
  real singers, soprano to bass), real electric pianos (a Wurlitzer, a Yamaha CP80, a Pianet), an IRCAM
  orchestra set. The app already shows credits for other CC BY material, so these fit in optional packs.
- **No drum-machine sample pack is legal to ship.** Every 808 or 909 set we found either has no licence
  or forbids redistribution. That is fine: those sounds were made by circuits, and we make them the same
  way, by synthesis.
- **Dubstep is made, not recorded.** It is 140 beats a minute felt at half speed (the snare on beat
  three), a pure deep sine bass, a "wobble" (a filter opening and closing in time with the beat), growls
  made with frequency modulation and distortion, the bass ducking out of the way of the kick, and a
  build that drops into the heavy part. Our engine already has the filter synced to the beat, frequency
  modulation, noise and drive. What it lacks: a song structure (build and drop), a steeper filter, LFO
  shapes and rate changes, a kick-triggered duck, a tempo-synced delay, harder distortion, glide, and
  drum-machine voices. House, drum and bass, synthwave and ambient need mostly the same pieces, so
  building dubstep's toolkit unlocks all of them (section 6).
- **Nothing is downloaded without the operator's yes**, one library at a time (section 8).

## 1. How this was read

Four research passes on 13 November 2026, each reading a site's robots.txt before any page, identifying
honestly, and using GitHub only through its REST API (`gh api`). No sample, archive or soundfont was
downloaded. freesound.org (its robots.txt bars automated reading) and kenney.nl (bars Claude) were not
read; nor were the other sites whose robots.txt refuses (the finding's "Sources not reached"). Two pages
were fetched in the same command as their robots.txt before the refusal was seen; nothing from them is
used. The key licences were re-read afterwards as a check: the LICENSE files of Osiris Piano, AKWF,
LMMS assets, epic-choir, Swirly Drums, Virtuosity Drums and Fashionbass, No Budget Orchestra's root
licence, Sonic Pi's sample licence, the TinySOL Zenodo record and Greg Sullivan's site (whose bare
domain turned out to be a parked page; the licence is on www.sullivang.net).

**The classes.** GREEN: public domain (CC0, the Unlicense) or MIT-style, ships anywhere, no credit
needed. CREDIT: CC BY, ships with a credit line. SHARE-ALIKE: CC BY-SA or GPL on the samples, left out
of the music library (the finding's judgement calls say why). REJECT: NonCommercial, Sampling Plus, "do
not redistribute", a shop EULA, or no licence at all.

## 2. Already on disk and not used yet

From `E:\HumanityOS\instruments-src` (listed 2026-11-13; nothing unpacked anew). Another agent is adding
the Martin guitar, a violin, the tenor saxophone, a piano and the vibraphone right now; the rest waits.

| Family | In VCSL (CC0) | In VSCO 2 CE (CC0) |
|---|---|---|
| Keys | Steinway B and Kawai grands, Yamaha and Knight uprights, five harpsichords, pipe organ, Renaissance organ, three Yamaha TX81Z FM patches | organ, two uprights |
| Plucked | concert harp, folk harp, strumstick, bowed and plucked psaltery, dan tranh | harp |
| Strings | | violin, viola and cello sections, solo violin, solo double bass |
| Brass, winds | tenor saxophone, saxello, four recorders, two ocarinas, didgeridoo, three harmonicas | F horn, trumpet, two trombones, tuba, flute, piccolo, oboe, clarinet, bassoon |
| Mallets | marimba, xylophone, vibraphone, glockenspiel, tubular glockenspiel, tubular bells, hand chimes, wine glasses | marimba, xylophone, glockenspiel |
| World | two kalimbas, three mbiras, balafon, slit drum, Nepalese hand bells | log drum |
| Drums | snare drums (three modern, rope tension, legacy), toms, bass drums, hi-hat, crash and suspended cymbals, claps, cajón, bongos, conga, darbuka, frame drum, ocean drum, timpani | concert snare, tenor and bass drums, cymbals, gongs, triangle |
| Percussion | agogo, anvil, bell tree, brake drum, cabasa, claves, cowbells, finger cymbals, flexatone, guiro, ratchet, shakers, slapstick, sleigh bells, tambourines, vibraslap, woodblock, mark trees | shakers, various metals and woods |

Karoryfer's Big Rusty Drums (on disk) is a complete 1980s kit (kick, snare, toms, cymbals), of which
the packs use only two toms so far. **For the lo-fi and electro swing kits, and for the acoustic layers
of the electronic genres, the snares, hats, claps and cymbals above are free to use today.**

## 3. The libraries, ranked per family

Best first. Size is the download as published (MB as GitHub or the page states it; MiB where the page
does). "Core?" says whether a slice is small enough for the app's core set (30 MiB limit, 7.9 MiB used
before the instruments being added now). Every row's quotation and source: the helper files.

### Keys

| # | Library | Class | Size | Why |
|---|---|---|---|---|
| 1 | Greg Sullivan E-Pianos (Wurlitzer EP200, Yamaha CP80, Hohner Pianet T) | CREDIT (CC BY 4.0 since 21 Oct 2026) | 18.8 MB | the only real electric pianos found free to ship: lo-fi, synthwave, house |
| 2 | FreePats Old Piano FB (honky-tonk upright) | GREEN | 34 MiB, small 6.2 MiB (core?) | "an authentic honky-tonk tone": electro swing, lo-fi |
| 3 | Osiris Piano (Karoryfer and Versilian) | GREEN | 437 MB | quiet, consistent, several microphones; still marked beta 0.925 |
| 4 | FreePats drawbar, rock and percussive organ emulations | GREEN | 5.8 to 12 MiB each | rendered tonewheel organ (house organ stabs) |
| 5 | FreePats FM Synthesized Pianos 1 and 2 | GREEN | 24 and 8.7 MiB | rendered DX7 piano; our `Fm` kind can already make this sound |
| 6 | Salamander Grand V3; Headroom Piano | CREDIT | 707 MiB; 156 MB | fine grands, but the CC0 Steinway and Kawai on disk cover them |

Rejected: jRhodes 3c and 3d (NonCommercial), Maestro Concert Grand ("All rights reserved."), Ivy Audio's
Piano in 162 (no licence stated), Splendid Grand (public-domain claim with no Akai source), Sofia MZ
pianos (withdrawn by their author), LivingRoom Upright (share-alike), Pianobook (EULA), the no-licence
harpsichord and organ conversions.

### Guitars, plucked and bass guitars

| # | Library | Class | Size | Why |
|---|---|---|---|---|
| 1 | Karoryfer Fashionbass (jazz-type electric bass) | GREEN | 301.7 MB; its GM-bank slice 21.5 MB raw (core?) | five velocity layers, three round robins: house, lo-fi, funk basslines |
| 2 | FreePats FSBS Electric Guitar Clean #1 and #2 (Jazz) | GREEN | 118 and 63 MiB; small 3.0 and 2.4 MiB (core?) | clean electric guitar, recorded 2026 |
| 3 | Karoryfer Shinyguitar | GREEN | 351.3 MB | up to 846 samples, pickup and microphone; its page: "quite suitable for pop, EDM, hip-hop, funk, indie etc." |
| 4 | FreePats Bass Guitar YR (picked, finger) | GREEN | 2.7 and 3.1 MiB (core) | a tiny electric bass |
| 5 | FreePats Ukulele | GREEN | 2.3 MiB (core) | new in August 2026 |
| 6 | Ganjo (banjo-guitar) | GREEN | 23.2 MB | a bright chop for electro swing |
| 7 | FreePats FSBS Distorted #1 and #2 | GREEN | 300 and 129 MiB | a CC0 distorted electric guitar, the thing music.md section 6 found "the hardest"; worth a listen for the soft-metal colour |
| 8 | More Karoryfer basses and plucked (Growlybass, Swagbass, Pastabass, Black And Blue Basses, Big Little Bass, Ergo EUB, Emilyguitar, Hungarian zither, cithara barbarica) | GREEN | 99 to 962 MB | depth, later |

Rejected: Kay 5-String Banjo (GPL 3), FreePats FSS steel-string (GPL 3 with an exception, as settled),
Rickenbacker 4001 (NonCommercial share-alike), five no-licence conversions.

### Bowed strings

| # | Library | Class | Size | Why |
|---|---|---|---|---|
| 1 | Karoryfer Sneakybass (double bass, quiet pizzicato and ghost notes) | GREEN | 324.7 MB | its page: "about as warm as double bass gets": the walking bass electro swing and lo-fi jazz want, in place of today's VSCO pizzicato |
| 2 | Karoryfer Meatbass (1958 double bass) | GREEN | 243.6 MB | arco five layers, pizzicato four layers by four round robins |
| 3 | Karoryfer x bigcat cello | GREEN | 126.5 MB | 479 samples: sustain, staccato, pizzicato, harmonics |
| 4 | GM-bank tremolo and pizzicato string ensembles (Versilian) | GREEN | 10.5 and 6.1 MB raw (core?) | small CC0 ensembles |
| 5 | D. Smolken's double bass; Aliexpress erhu | GREEN | 252.9 MB; 77.8 MB | depth; a world colour |

### Brass

| # | Library | Class | Size | Why |
|---|---|---|---|---|
| 1 | Karoryfer War Tuba | GREEN | 104.1 MB | over 1,000 samples, "A raw and impolite tuba": a street-band bass line |
| 2 | TinySOL tuba, horn, trombone, trumpet | CREDIT | part of 1.0 GB | IRCAM studio, pianissimo to fortissimo |

VSCO 2 (on disk) already gives a horn, a trumpet, two trombones and a tuba. No free muted or jazz
trumpet was found, so electro swing's brass stabs stay partly synthesised.

### Woodwinds and reeds

| # | Library | Class | Size | Why |
|---|---|---|---|---|
| 1 | Karoryfer Weresax (alto, though Karoryfer's free list says tenor) | GREEN | 188.4 MB | 256 samples: lo-fi and electro swing solos |
| 2 | Karoryfer Bear Sax (1926 baritone) | GREEN | 125.4 MB | subtone and growl |
| 3 | FreePats Clarinet; Wooden Recorder | GREEN | 6.7 MiB; 8.9 MiB | small; VSCO's clarinet is already in the core set |
| 4 | MTG Solo Sax (soprano, alto, tenor, baritone) | CREDIT | 102 MB | three round robins; its freesound sources unread |
| 5 | GM-bank Shanai | GREEN | 8.6 MB | a small double reed |

### Voices and choirs

| # | Library | Class | Size | Why |
|---|---|---|---|---|
| 1 | epic-choir v1.0 (from VocalSet's 20 singers) | CREDIT | 695.6 MB (four zips) | soprano, alto, tenor, bass, five vowels, three dynamics, 3 to 10 real singers per sample: the first open choir of real quality; released 24 Oct 2026 by one author |
| 2 | Karoryfer legato vocal tutorial | GREEN | 156.1 MB | one sung "a" vowel with legato: a solo voice pad |
| 3 | Berklee / OLPC voices | CREDIT | 46 MB | one-shots at a few pitches, not a playable choir |
| 4 | Karoryfer 272 Merry Orks | GREEN | 38.6 MB | death-metal growls: a creature or danger colour |

Still no public-domain choir. Rejected: Sonatina's chorus (Sampling Plus), No Budget Orchestra's choir
(no licence), Decent Samples' Dave Choir (EULA), Ivy Audio's Poeesia (NonCommercial).

### World instruments

| # | Library | Class | Size | Why |
|---|---|---|---|---|
| 1 | jSteelDrum version 2 | GREEN (the Unlicense) | 38.8 MB | five velocity layers; its author calls it "much better" than the version 1 we ship |
| 2 | Karoryfer Gogodze Phu Vol I (Ghanaian bobobo drums, cajón) | GREEN | 29.8 MB | hand drums with five strike spots and rolls |
| 3 | FreePats Bagpipe in G with drones | GREEN | 15 MiB, small 5.2 MiB (core?) | real recording, 2026 |
| 4 | GM-bank Sitar | GREEN | 9.7 MB | untested quality |
| 5 | Karoryfer Horse Pulse (bass tagelharpa) | GREEN | 140.7 MB | six round robins |
| 6 | FreePats Jaw Harp | GREEN | 1.8 MiB | its page: "the quality is not very good" |

No real chromatic koto, oud, shakuhachi or sitar under an open licence beyond these.

### Accordion and free reeds

| # | Library | Class | Size | Why |
|---|---|---|---|---|
| 1 | FreePats Button Accordion HN (Hohner) | GREEN | 4.8 MiB (core) | recorded 2023; the musette accordion belongs to gypsy jazz and electro swing |
| 2 | TinySOL accordion | CREDIT | part of 1.0 GB | chromatic, pianissimo to fortissimo |

VCSL's three harmonicas are on disk. No open concertina, bandoneon or harmonium was found (the Euterpea
harmonium's samples need a person's look at freesound).

### Drums and percussion

| # | Library | Class | Size | Why |
|---|---|---|---|---|
| 1 | Karoryfer Swirly Drums | GREEN | 828.4 MB | "brushed drums with snare stirs and flutters": electro swing and lo-fi brushes |
| 2 | LMMS assets (drums) | GREEN | 43 MiB of 379 one-shots (core: a dozen) | house and drum-and-bass kicks, jazz hats, claps, every sample recorded by its contributor |
| 3 | Virtuosity Drums (Versilian and Karoryfer) | GREEN | 1,170.3 MB | jazz kit with sticks, six microphones, a full General MIDI map |
| 4 | Malaclypse's The Clap | GREEN (MIT, keep the notice) | 4.5 MB | hand claps and snaps as a kit: house and electro swing |
| 5 | Karoryfer body percussion; Ethan Winer collection | GREEN | 38.2 MB; 14.8 MB | claps, slaps; small auxiliary percussion |
| 6 | Karoryfer Unruly Drums, Frankensnare, Gogodze Phu Vol II, The Hat With The Phat | GREEN | 133 to 645 MB | variety, later |
| 7 | DrumGizmo DRSKit, Naked Drums | CREDIT | 736 MB to 1.25 GB | good, but the CC0 kits cover them |

Rejected: AVL Drumkits and the Salamander Drumkit (share-alike), SM Drums (no licence), Orange Tree's jazz
kit ("All Rights Reserved"), the GM bank's standard kit (no licence, mixed freesound sources), Sam's
Sonor (share-alike).

### Drum machines

**None shippable** (the finding, sections 10 and 11): Dirt-Samples and Strudel's drum machines have no
licence and admittedly unknown provenance; MusicRadar, 99Sounds, Samples From Mars, Legowelt and Wave
Alchemy allow use in music only; the 1994 TR-808 set grants nothing; machines with sample ROMs
(Roland's own manuals) reserve their sounds. **The engine synthesises them** (section 6, rung E4).
Small CC0 electronic one-shots exist in LMMS assets and Sonic Pi (leaving out its two Amen-break loops).

### Synth sources and wavetables

| # | Library | Class | Size | Why |
|---|---|---|---|---|
| 1 | AKWF, the `AKWF` folder | GREEN | 5.81 MiB, 4,358 single-cycle waves (core) | the raw material for a wavetable oscillator (synthwave, house, dubstep); used by Elektron and QU-Bit products |
| 2 | Karoryfer Cowsynth; Caveman Cosmonaut; String Cyborgs | GREEN | 13.4; 93.0; 60.8 MB | bagpipe, transistor-organ and looped-string oscillators |
| 3 | SHLD Pro-1 and Minifreak sets | GREEN | small; 265 MB | recordings of analog synths (its Wavestate and SR-16 sets are left out: maker ROMs) |
| 4 | FreePats' twenty synth banks; Sonic Pi's The Black Dog pads | GREEN | 0.7 to 13 MiB | rendered; our own synth makes these natively |

Rejected: Legowelt's synth packs and SampleRadar's polysynths (no redistribution), Surge XT's wavetables
(GPL).

### Sound effects for music

| # | Library | Class | Size | Why |
|---|---|---|---|---|
| 1 | LMMS assets SFX | GREEN | 2.8 MiB (core) | riser, up and down sweeps, sub drop, noise blast, vinyl backspin and rewind, cymbal swell |
| 2 | Sonic Pi vinyl and swoosh samples | GREEN | small | `vinyl_hiss` is the only CC0 crackle bed found |
| 3 | Karoryfer Zamenhof glitches | GREEN | 75.4 MB | glitch textures |
| 4 | BigSoundBank | GREEN per sound | 3,500+ sounds | foley and whooshes |

Rejected: BBC Sound Effects (non-commercial), Mixkit, 99Sounds. Not read: Sonniss, Zapsplat, Pixabay,
Freesound, Kenney, OpenGameArt. Risers, impacts and tape stops synthesise well anyway (rung E3).

### General MIDI soundfonts (for completeness)

MuseScore General is MIT (38 MB as SF3; keep its notices) and Fluid R3 MIT, both soundfont-era quality;
GeneralUser GS's author is unsure where all its samples came from, so it stays out. None is needed while
the libraries above cover the same instruments.

## 4. What changed from what we believed

- **No Budget Orchestra** is not CC BY-SA 4.0: its root licence forbids commercial sample libraries and
  its folders mix CC BY-SA, Sampling Plus, NonCommercial and no licence. Still out, for a new reason.
- **Karoryfer's older releases** (2019 to 2022) carry CC BY 4.0, one GPL 3; take releases dated
  2022-11-04 or later.
- **Greg Sullivan's electric pianos** are CC BY 4.0 since 21 October 2026.
- **Shinyguitar** was never GPL in its history (music.md 7a's note about a GPL listing is answered).
- **jSteelDrum version 2** is public domain too (the Unlicense).
- **MuseScore General** (MIT) and **GeneralUser GS** (unsure provenance), "not settled" before, are now
  settled.

## 5. Electronic genres: what defines them

Sourced and dated in [electronic-genres-and-synthesis.md](electronic-genres-and-synthesis.md) (Wikipedia,
Sound On Sound, MusicRadar, Attack Magazine, Ableton's Learning Music; read 13 November 2026). Grids are
one bar of sixteen steps, beats on steps 1, 5, 9, 13.

### Dubstep (first)

| Part | What it is |
|---|---|
| Tempo, feel | 132 to 142 bpm, most at 140; **half-time**: the snare on beat 3 makes 140 feel like 70; hats often shuffled or in triplets |
| Drums | kick `x...............` with "lazy" off-beat kicks; snare and clap layered `........x.......`; hats velocity-varied |
| Sub bass | one or more sine waves under everything; the kick tuned to sit above the sub, which ducks when the kick hits |
| Wobble | a 24 dB low-pass filter whose cutoff an LFO sweeps in time with the beat (1/8, the 1/8 triplet, rates changed bar by bar), mono, resonant, distortion after |
| Growl | frequency modulation (more FM is buzzier) with a looping envelope or LFO on the FM depth, formant (vowel) filters, bit and sample-rate reduction, parallel distortion, and resampling (render the sound, replay it, process again) |
| Structure | intro, a main section with a drop, a middle, a second drop, an outro; the drop after bar 16 or 32; the drums stop, often to silence, then return harder with the sub |
| Harmony | minor key or the Phrygian mode, tritone riffs |
| Kinds | deep or "140" (sub-led, spacious, the calm-friendly one), brostep (midrange, aggressive), riddim (repetitive sub, triplet percussion) |

### The others

| Genre | Tempo, feel | Drums | Bass and sound | Structure |
|---|---|---|---|---|
| House | 115 to 130 bpm, four on the floor, light swing | kick on every beat, clap on 2 and 4, open hat on every off-beat (808, 909, 707 voices) | deep synth bassline ducked by the kick (pumping); acid: a resonant filter with accent and slide | eight-bar sections, DJ intros and outros, breakdown and build |
| Drum and bass | 165 to 185 bpm, mostly 170 to 180, breakbeat | two-step: kick on 1 and the and of 3, snare on 2 and 4, ride on eighths, ghost snares; breakbeat-STYLE patterns played by our own drums | sine sub; the reese bass (two detuned saws beating against each other) | build, the drop, breakdown |
| Synthwave | 80 to 118 bpm (upbeat 128 to 140), straight | backbeat with a gated-reverb snare, '80s drum-machine voices | steady sixteenth-note analog bass ducked by the drums; supersaw and brass pads; four-note arpeggios; mono lead with glide; long reverbs, tape delay | song form |
| Ambient | free or very slow | usually none | drones, long evolving pads, granular textures, long reverb with the lows cut, tempo-synced tape delay | slow evolution, no verse or chorus |
| Lo-fi hip hop (built) | 60 to 80 bpm, MPC swing about 57%, snare a touch late | boom bap: kick on the downbeats, snare on the upbeats, swung sixteenth hats | jazzy sevenths, electric piano, hiss, crackle, wow, reduced bit depth | loops, steady energy |

**The copyright line holds** ([`2026-11-12-music-genre-copyright.md`](../../findings/2026-11-12-music-genre-copyright.md)):
these are a genre's building blocks. Never an artist's recordings, melodies or riffs: the Amen break is
a recording with a named owner, so drum and bass plays our own drums in a breakbeat-style pattern, never
that audio; the reese is a technique (two detuned saws), the record it is named after is not ours.

## 6. What the engine has, and what it needs

The engine is `crates/humanity-music` (read 2026-11-13). The genre layer (section 7a of music.md) already
carries tempo, groove, drum patterns as text, bass and pad patterns, harmony, palette, a mix recipe and
a level, so each electronic genre is mostly a data file once the sounds exist.

| Need | Today | Missing | Rung |
|---|---|---|---|
| Half-time and triplets | drum patterns as text over a grid; `subdivision` is a number (2 eighths, 4 sixteenths) | 3 or 6 steps a beat for triplet hats is probably already expressible, untested; swing assumes pairs | E1 |
| Build and drop | phrases, dynamics per phrase | a `structure` in the genre: sections with bar counts (intro, build, drop, break, outro), the roles and patterns of each, a riser and snare roll in builds, a silent beat before the drop | E1 |
| Sub bass | `Synth` with a sine oscillator | a clean sub that stays mono and undistorted under a distorted bass | E2 |
| Wobble | `FilterSpec.lfo_per_beat` (the LFO in cycles per beat, so 2 is 1/8 and 3 the 1/8 triplet), `env_hz`, key tracking | a 24 dB slope (two filter stages), LFO shapes (only a sine today: triangle, saw, square, sample and hold), a rate pattern that changes per bar, the LFO restarting at each note, filter resonance driven harder | E2 |
| Growl | `Fm`, two operators | FM depth moved by an LFO or a looping envelope, three or four operators, a formant (vowel) filter on any synth (the choir already has the formant table), a comb filter | E2, E7 |
| Distortion | a soft `drive` per synth voice; saturation over the mix | waveshaper curves (hard clip, foldback, asymmetric), bit and sample-rate reduction, parallel (wet and dry) distortion | E2 |
| Glide | none | mono, legato and portamento for basses and leads | E2 |
| Sidechain | a duck of the whole score under game sound | a kick-triggered duck per role, driven by the composer's own drum events (exact and cheap), its depth and release per genre | E3 |
| Space | one FDN reverb, per-role send, wow and flutter, a low-pass over the mix | a tempo-synced delay (ping-pong, tape-filtered), gated reverb (synthwave), a low cut on the reverb, a frozen reverb (ambient) | E3 |
| Risers and impacts | `Noise` with a sweep that follows the envelope | a riser over N bars (noise and a rising pitch, filter opening), a downlifter, an impact (a pitch-diving sub hit), a reversed cymbal, a tape stop (the score slowing to nothing) | E3 |
| Drum machines | `Membrane` (pitch drop), `Noise` | 808 and 909-style kick, snare, clap (several noise bursts through a band-pass), hats (six square oscillators or band-passed noise), toms, cowbell, rimshot, from published circuit descriptions | E4 |
| Oscillators | sine, triangle, saw, square with polyBLEP, up to four per synth with detune | a supersaw (seven saws, the measured detune and mix curves, a high-pass), a wavetable oscillator (the AKWF waves, a position an LFO or envelope can sweep), pulse width modulation | E5 |
| Loudness | each genre at -18 LUFS, a limiter | a drop measured at the same loudness as the rest, so "heavy" never means louder than the category | E6 |

### The rungs

- **E1. Structure and groove.** The genre's `structure` field (sections, bar counts, what each plays),
  half-time patterns, a triplet grid, builds with a snare roll (eighths, then sixteenths, then 32nds) and a
  silent beat before the drop. Data plus composer work, no new sound.
- **E2. The bass toolkit.** Two-stage filter, LFO shapes, per-bar rate patterns, LFO restart, glide,
  waveshaper and bitcrush, parallel drive, a protected sub, FM depth modulation.
- **E3. The mix.** Kick-triggered ducking per role, a tempo-synced delay, gated reverb, a reverb low
  cut, risers, impacts, reversed cymbals and the tape stop.
- **E4. Drum-machine voices** synthesised, as presets of the existing kinds where they fit.
- **E5. Oscillators.** Supersaw; the wavetable oscillator with the AKWF waves (needs that download);
  pulse width.
- **E6. The genres as files.** Deep dubstep first (140, half-time, sine sub, a gentle wobble), then house,
  drum and bass, synthwave and ambient, each measured at -18 LUFS, each a person's choice and never a
  default; a test walks each through a danger mood as the existing genres do.
- **E7. Later.** Formant ("talking") basses, resampling (render a growl and replay it: a buffer kept in
  memory that must obey the Dev page's reuse switch), granular textures for ambient, the heavier brostep
  and riddim kinds.

Each new block costs per voice on the music's own thread; each rung states its cost from the render
tests (`just music-render`, `tests_audio.rs`) before it is built, the way the existing voices were
measured.

## 7. Open synthesis references, compared

How should the new pieces be written? The options, best first (licences quoted in
[electronic-genres-and-synthesis.md](electronic-genres-and-synthesis.md), part 2):

| Option | Licence | Fit | Verdict |
|---|---|---|---|
| Extend our own `dsp.rs` and `voice.rs`, writing published methods in our own code | ours (CC0) | the engine already has polyBLEP oscillators, a topology-preserving state-variable filter, an FDN reverb and tests that measure rendered audio | **best**: no second framework to keep in step |
| Copy single modules from permissive code, keeping their notices | fundsp (Rust, MIT or Apache-2.0); Mutable Instruments' STM32 module code (MIT per file, never the trademarked name); Google's DX7 engine (Apache-2.0, from google/music-synthesizer-for-android, not Dexed); Airwindows (MIT); Freeverb (public domain); STK (MIT-style); DaisySP's MIT tree | real, tested code for exactly the missing pieces (fundsp's Simper filters and Moog ladder, Plaits' macro-oscillators and drum models, Airwindows' saturation and tape) | **use for E2 to E5**, file by file |
| Depend on fundsp as a crate | MIT or Apache-2.0 | a whole audio-graph model beside our voice pool | not now: it duplicates what exists |
| Compile Faust to Rust | Faust libraries' exception frees the compiled output | a large vetted library, but a new toolchain in the build | later, if a module is easier that way |
| Learn only | Surge XT, Vital, Helm, Dexed, ZynAddSubFX, Odin 2, VCV Rack, Cardinal (GPL); Csound, oxisynth, DaisySP's LGPL modules (LGPL); musicdsp.org (no licence) | the best sounding open synths | read for ideas, never copy |

Published methods (free to implement): polyBLEP (Välimäki, Pekonen and Nam, 2012), Andrew Simper's
trapezoidal filters, Vadim Zavalishin's "The Art of VA Filter Design", Adam Szabo's supersaw thesis
(2010: seven saws, an eleventh-order detune curve, a high-pass on the sum), Jon Dattorro's plate reverb
(1997), John Chowning's FM (1973; its patent expired in 1995). CCRMA's and the AES's sites refuse
automated reading, so Dattorro, Chowning and Julius O. Smith's books were cited, not read in the original.

## 8. Recommendation

### Downloads, each a separate yes from the operator

In this order (sizes as published; each verified and recorded in `instrument-sources.md` on arrival, the
way the first seven were):

1. **AKWF waveforms** (CC0): github.com/KristofferKarlAxelEkstrand/AKWF-FREE, only the `AKWF` folder at
   commit `8de90bf` (2025-12-04), 4,358 files, 5.81 MiB, fetched file by file as the Martin HD-28 was (the
   whole repository is over 800 MiB of the same waves in other products' formats). For rung E5.
2. **Karoryfer Swirly Drums** (CC0): release v1.104 of github.com/sfzinstruments/karoryfer.swirly-drums,
   828.4 MB. Brushes for electro swing and lo-fi.
3. **Karoryfer Sneakybass** (CC0): release v1.000 of karoryfer.sneakybass, 324.7 MB. The walking upright
   bass.
4. **FreePats small set** (CC0, from freepats.zenvoid.org): Button Accordion HN 4.8 MiB, Old Piano FB 34
   MiB, Ukulele 2.3 MiB, Bagpipe 15 MiB, FSBS Electric Guitar Clean #2 (Jazz) 63 MiB, Bass Guitar YR 3.1
   MiB: about 122 MiB.
5. **LMMS assets** (CC0): github.com/LMMS/assets, 49.4 MiB. Electronic kicks, claps and hats, risers and
   vinyl effects (optional: synthesis covers most of them).
6. **Karoryfer Weresax and Bear Sax** (CC0): 188.4 and 125.4 MB. Alto and baritone beside VCSL's tenor.
7. **Karoryfer Fashionbass** (CC0): release v1.001, 301.7 MB. An electric bass.
8. **Virtuosity Drums** (CC0): release v0.925, 1,170.3 MB. The jazz kit with a ride.
9. **Karoryfer War Tuba** (CC0, 104.1 MB) and **jSteelDrum version 2** (the Unlicense, 38.8 MB).

If the operator agrees to credited instruments in optional packs (question 1):

10. **epic-choir v1.0** (CC BY 4.0): four zips from github.com/thepok/epic-choir, 695.6 MB. The score's
    choir role in the calm moods, with the synthesised choir as its fallback.
11. **Greg Sullivan E-Pianos** (CC BY 4.0): github.com/sfzinstruments/GregSullivan.E-Pianos, 18.8 MB.
    Wurlitzer, CP80, Pianet for lo-fi and synthwave.
12. **TinySOL** (CC BY 4.0): zenodo.org/records/3685367, 1.0 GB. Later: it overlaps VSCO 2.

Not recommended now: Osiris Piano, Shinyguitar, Salamander, Headroom, DRSKit, Naked Drums and the other
Karoryfer kits (good, but covered by what is on disk or listed above).

### Core set or optional pack

- **Core** (public domain only; about 22 MiB of room before today's additions): a curated slice of AKWF
  kept as plain PCM (a lossy codec smears a single cycle; 200 waves are about 0.3 MiB, all 4,358 are 5.8
  MiB), the FreePats accordion, ukulele and small bass, a dozen LMMS drum one-shots and its risers, and
  the small electric guitar. Each is a recipe in `scripts/instrument-recipes.json`.
- **New packs**, each one archive as today: *Band and jazz* (Sneakybass, the saxophones, War Tuba,
  Fashionbass, Old Piano FB, the jazz guitar), *Brushes and jazz kits* (Swirly, Virtuosity), *Synth
  sources* (all of AKWF, Cowsynth, Caveman Cosmonaut), *Folk and world* (bagpipe, sitar, Gogodze Phu,
  steel pan version 2), and, credited, *Choir* (epic-choir) and *Electric pianos* (Greg Sullivan).
- **For the electronic genres no download is needed** until rung E5: dubstep, house, drum and bass,
  synthwave and ambient are synthesis-first, and the snares, claps and hats on disk give acoustic layers.

## 9. Questions for the operator, each with a recommendation

1. **Credited (CC BY) instruments in optional packs?** The music has been public domain only. The app
   already credits CC BY material (NVIDIA's models) and fails a test if a credit is missing.
   *Recommendation: yes for optional packs (the choir, the electric pianos), never in the core set.*
2. **Which downloads first?** *Recommendation: AKWF and Swirly Drums now; Sneakybass and the FreePats set
   next; the choir and electric pianos if question 1 is yes.*
3. **Dubstep's character.** *Recommendation: the deep, sub-led "140" kind first, gentle enough for the
   ship; the heavier brostep and riddim later, as their own choices, never a default.*
4. **Share-alike sets** (AVL drumkits, the Salamander drumkit, church organs). *Recommendation: leave
   them out; CC0 and CC BY sets cover the same instruments.*
5. **Ask the jRhodes author?** His own GM-bank header says CC0 while his main repositories say
   NonCommercial. *Recommendation: not now; Greg Sullivan's pianos fill the electric piano part.*
6. **The order of the electronic rungs.** *Recommendation: E1 (structure) and E2 (bass toolkit) first,
   then deep dubstep as a genre file, then E3 and E4 for house and drum and bass, then E5 for synthwave.*
