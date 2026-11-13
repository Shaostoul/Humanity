# Six electronic genres, and the licences of open synthesis references

> The genres and synthesis part of the free instrument library research of **13 November 2026**: the plain-words summary, the
> ranking and the recommendation are in [README.md](README.md); the licences that matter, with their
> quotations, are in the dated finding [2026-11-13-free-instrument-libraries.md](../../findings/2026-11-13-free-instrument-libraries.md). This is an AI's
> reading of each licence, not legal advice: every licence is quoted so a reader can check it. Gathered by
> four research passes on 2026-11-13 and spot-checked the same day (licence files re-read through the
> GitHub API, the TinySOL Zenodo page, Greg Sullivan's site).

**Read on 13 November 2026** (Pacific). Every page below was read that day by an automated reader
that identified itself as `Claude-User`; each robots.txt was read first (results at the end). Each
source carries its OWN date beside it (a Wikipedia "last edited" date, a magazine's published date).
GitHub facts come from the REST API through `gh api` (licence identifiers, licence-file first lines,
source-file headers). crates.io facts come from its API (`/api/v1/crates/<name>`), one request a second.
Quotations are exact and short; anything not quoted is the researcher's summary. Typographic variants only are
normalised to plain ASCII inside quotations (curly apostrophes and quotes, Sound On Sound's
non-breaking hyphens, en dashes in ranges such as Wikipedia's "132-142"; an em dash is shown as
"--"); words, spelling (typos included) and all other punctuation are as published. Where a quotation
itself contains quotation marks, it is wrapped in curly marks and the inner straight marks are the
source's own. Grids marked *illustrative* are the researcher's own typing of a common pattern, not a quotation.

**Grid notation.** One bar of 4/4 as sixteen 16th-note steps. Beats fall on steps 1, 5, 9, 13.
`x` = hit, `.` = rest. "The and of 3" = step 11.

**The copyright line (settled in `docs/reference/findings/2026-11-12-music-genre-copyright.md`; not
redone here).** Tempo, meter, a drum pattern's general shape, sidechain pumping, a wobble, a supersaw,
a riser: a genre's building blocks, free to use. Never an artist's recording (no sample, loop, stem or
single hit lifted from a track), never their melody, riff or hook, never a particular song's
distinctive combination. Two cases this research touched:
- **The Amen break** is a RECORDING with an owner: “The copyright owner of "Amen, Brother", including
  the Amen break, was the Winstons bandleader, Richard Lewis Spencer.” (Wikipedia, "Amen break", last
  edited 29 Oct 2026). Synthesise our own drums playing a breakbeat-style pattern; never the audio.
- **"The Reese bassline"** names one record's riff: “One of his more famous basslines (Reese--"Just
  Want Another Chance", Incognito Records, 1988)” (Wikipedia, "Drum and bass", 12 Nov 2026). The
  TECHNIQUE (two detuned saws) is free; that riff is not.

---

# PART 1: GENRES

## 1. Dubstep (deepest)

| Aspect | Finding |
|---|---|
| Tempo | 132-142 bpm, most releases at 140 ("140" is a nickname); Ableton gives 135-145; SOS "around the 140bpm mark" |
| Meter / feel | 4/4, **half-time** ("half-step"): the snare on beat 3 makes 140 feel like 70; often shuffled or triplet-swung |
| Drums (half-step) | kick `x...............` (+ "lazy" off-beat kicks, e.g. *illustrative* `x.........x.....`)<br>snare+clap `........x.......` (beat 3)<br>hats `x.x.x.x.x.x.x.x.` *illustrative*, shuffled/triplet and velocity-varied |
| Drums (older two-step) | garage-derived two-step kick/snare; more percussive |
| Sub | sine (or several sines) under everything; the kick chosen to sit ABOVE the sub, not an 808 boom |
| Wobble | low-pass filter (24 dB) whose cutoff is swept by an LFO synced to note values (1/8 shown; 1/8 triplet; automate the rate between 1/4, 1/8, 1/16 per bar); mono; resonance; distortion after |
| Growl / talking | FM (more FM = buzzier), a looping envelope on FM depth as the "wobble", formant (vowel) band-pass filters, bit/sample-rate reduction, parallel distortion, then RESAMPLING (render the patch to audio, replay it as a sampler instrument, process again) |
| Structure | intro, main section with a drop, midsection, second main section with another drop, outro; the drop usually after bar 16 or 32; percussion pauses (often to silence) then returns harder with the sub |
| Signature techniques | sidechain ducking of sub by kick (even a hat-triggered duck), layered snare+clap with big reverb, minor key / Phrygian, tritone riffs, bitcrusher, tape stop, risers and snare rolls before the drop |
| Brostep / riddim | brostep moves energy into the MIDRANGE (distorted, guitar-register bass); riddim = repetitive minimal sub and triplet percussion |

**Sources and quotations**
- Wikipedia, "Dubstep", https://en.wikipedia.org/wiki/Dubstep (last edited 2 Nov 2026):
  - "The tempo is nearly always in the range of 132-142 beats per minute, with a clap or snare usually inserted every third beat in a bar."
  - “With a large majority of releases at 140bpm, the genre ... is sometimes referred to as "140".”
  - "Dubstep rhythms are usually syncopated, and often shuffled or incorporating tuplets."
  - "tracks frequently use a minor key or the Phrygian mode, and can feature dissonant harmonies such as the tritone interval within a riff."
  - "This style of bass is typically produced by using a low-frequency oscillator to manipulate certain parameters of a synthesiser such as volume, distortion or filter cutoff."
  - "It is very common for the bass to drop either after the 16th or 32nd measure of a song."
  - "Typically, the percussion will pause, often reducing the track to silence, and then resume with more intensity, accompanied by a dominant sub-bass."
  - Structure: "an intro, a main section (often incorporating a bass drop), a midsection, a second main section similar to the first (often with another drop), and an outro."
  - Brostep “accentuates the middle register and features "robotic fluctuations and metal-esque aggression".”
  - "Riddim is characterised by repetitive and minimalist sub-bass and triplet percussion arrangements"
- Sound On Sound, "Dubstep Basics" (Peter Jenkins), https://www.soundonsound.com/techniques/dubstep-basics (published July 2010):
  - "Initially based around a garage-influenced, two-step kick and snare beat, the genre has evolved ... with an increasing number of tracks containing a half-step rhythm."
  - "The tempo of dubstep is generally around the 140bpm mark"
  - "The use of single or multiple sine waves is a good starting point for making dubstep sub-bass."
  - "start with a kick drum that hits higher up the frequency spectrum than the sub-bass"
  - "a side-chained compressor to 'duck' the volume of the sub-bass when the kick drum hits."
  - "To obtain a typical dubstep snare drum, you need to layer a snare sample, or multiple snare drums, with a clap."
  - "a pretty basic half-step pattern, with 'lazy' off-beat kick-drum beats that increase the feeling of swing."
  - "A sure-fire way of adding swing to your beats is to create them in a triplet pattern."
  - "The application of an LFO to the cutoff will automate this effect, creating the infamous wobble."
  - "Synchronising the LFO speed to the track's tempo will often sound best"
  - Resampling: three Massive patches "rendered each of them as an audio file", then played from a sampler (Kontakt).
- Sound On Sound, "Dubstep Secrets" (Geoff Smith), https://www.soundonsound.com/techniques/dubstep-secrets (published November 2010):
  - "modulate the fIlter cutoff with a tempo-sync'ed LFO." ... "set type to 24dB" ... "Choose 1/8th note." ... monophonic.
  - "brighter, buzzier sounds require more FM, while mellower, fatter sounds are created with less."
  - "Parallel distortion is often a much better option on bass sounds"
  - "The Bitcrusher plug-in can produce some great clipping and distortion effects"
- MusicRadar, "How to build an LFO wobble bass", https://www.musicradar.com/how-to/lfo-wobble-bass (published 9 June 2024): "Below the Rate pot, select 1/8 trip from the drop-down menu." (a triplet rate)
- Attack Magazine, "Dirty FM Bass", https://www.attackmagazine.com/technique/tutorials/dirty-fm-bass/ (19 Sept 2014):
  - "rather than using an LFO to modulate a filter cutoff and remove harmonics from a sound, we're doing the opposite and adding harmonics with a varying amount of frequency modulation."
  - A looping envelope on the modulator's level "works much like an LFO"; then FM8's "bit resolution control" (lower than "the DX7's 12 bits").
- Splice blog, "Creating 'Talking' Wobble Bass Sounds in Ableton Live", https://splice.com/blog/creating-talking-wobble-bass-sounds-ableton-live (28 May 2015):
  - "Formant - Prominent bands of frequency that determine the phonetic quality of an audio signal."
  - "downsample audio to add bite and harmonic grit to patches."
- MusicRadar, "How to program 6 different jungle and drum 'n' bass grooves" (17 Feb 2022): half-time = "a single snare is placed on beat 3." (applies to dubstep's half-step too)
- Ableton Learning Music, "Tempo and genre", https://learningmusic.ableton.com/make-beats/tempo-and-genre.html (no date on page): "Dubstep: 135-145 bpm"

## 2. House

| Aspect | Finding |
|---|---|
| Tempo | 115-130 bpm (typical 120-130) |
| Meter / feel | 4/4, **four on the floor**, often light swing |
| Drums | kick `x...x...x...x...`<br>clap/snare `....x.......x...`<br>off-beat hat `..x...x...x...x.`<br>(TR-808 / TR-909 / TR-707 voices; clave-based riffs in early Chicago house) |
| Bass | deep synthesizer basslines or loops; acid house = the squelchy TB-303 (resonant filter + accent + slide); bass ducked by the kick (sidechain pumping) |
| Structure | repeated eight-bar sections; DJ-friendly intros/outros; breakdown and build |
| Signature techniques | sidechain "pumping" in time with the kick; saturated drums; filter sweeps |

**Sources and quotations**
- Wikipedia, "House music", https://en.wikipedia.org/wiki/House_music (last edited 7 Nov 2026):
  - "characterized by a repetitive four-on-the-floor beat and a typical tempo of 115-130 beats per minute."
  - "repetitive 4/4 rhythms including bass drums, off-beat hi-hats, snare drums, claps, and/or snaps at a tempo of between 120 and 130 beats per minute"
  - "the bass drum is usually played on beats one, two, three, and four, and the snare drum, claps, or other higher-pitched percussion on beats two and four."
  - "almost always provided by an electronic drum machine, often a Roland TR-808, TR-909, or a TR-707."
  - "acid house, is defined through the squelchy sounds created by the Roland TB-303 bass synthesizer."
  - "House music tracks are often based on eight-bar sections which are repeated."
- Ableton Learning Music, "Backbeats" (no date): "a clap or snare on the second and fourth beat, a pattern known as a backbeat." ... "four-on-the-floor kicks with backbeat claps or snares is commonly used in many genres such as techno and house." "Tempo and genre": "House: 115-130 bpm".
- Wikipedia, "Dynamic range compression", https://en.wikipedia.org/wiki/Dynamic_range_compression (last edited 14 Oct 2026):
  - "In electronic dance music, side-chaining is often used on basslines, controlled by the kick drum or a similar percussive trigger"
  - "many dance and hip-hop musicians purposefully use this phenomenon, causing the mix to alter in volume rhythmically in time with the beat."

## 3. Drum and bass

| Aspect | Finding |
|---|---|
| Tempo | 165-185 bpm; since about 1996 mostly 170-180, the mid-170s the hallmark |
| Meter / feel | 4/4, fast **breakbeat** syncopation; the **two-step** as the skeleton; half-time variant |
| Two-step | kick `x.........x.....` (beat 1 and the and of 3: "the eighth-note before the second snare")<br>snare `....x.......x...` (beats 2 and 4)<br>ride/hat `x.x.x.x.x.x.x.x.` (ride "on every 1/8th"), plus soft ghost snares and swung 16th ghost hats |
| Variants | second snare displaced an eighth; neuro: second kick on the last 16th before beat 3 (`x......x........`); half-time: one snare on beat 3 |
| Amen-STYLE break (pattern only) | Four bars: bars 1-2 the same beat, bar 3 delays a snare, bar 4 leaves beat 1 empty, then a syncopated run and an early crash. *Illustrative* bar 1-2 typing (common transcription, NOT quoted, verify by ear against our own synthesis):<br>kick `x.x.......xx....`<br>snare `....x..x.x..x..x`<br>ride `x.x.x.x.x.x.x.x.` |
| Bass | deep sub-bass felt physically; the **reese** = two (or more) saws detuned against each other so phase cancellation "beats", mono, low-passed, high-passed with a separate sine sub beneath |
| Structure | ambient/DJ intro, build, the **drop** (switch of rhythm or bassline, after a build and breakdown), DJ rewinds |
| Signature techniques | layered breaks over a programmed skeleton; time-stretched breaks; bitcrushed ride; snare layering (low punch, mid body, bright top through hall reverb) |

**Sources and quotations**
- Wikipedia, "Drum and bass", https://en.wikipedia.org/wiki/Drum_and_bass (last edited 12 Nov 2026):
  - "characterised by fast breakbeats (typically 165-185 beats per minute) with heavy bass and sub-bass lines"
  - "Since around 1996, drum and bass tempos have predominantly stayed in the 170-180 range."
  - "The drop is the point in a track where a switch of rhythm or bassline occurs and usually follows a recognisable build section and breakdown."
  - "Syncopated breakbeats remain the most distinctive element as without these a high-tempo 4/4 dance track could be classified as techno or gabber."
  - "a technique of switching between two breaks after each bar developed."
- MusicRadar, "How to program 6 different jungle and drum 'n' bass grooves", https://www.musicradar.com/how-to/how-to-program-6-different-jungle-and-drum-n-bass-grooves (17 Feb 2022):
  - "the two-step beat, whereby the second kick in a bar is pushed later in time to the eighth-note before the second snare"
  - "Instead of having the snare firing on beats 2 and 4 of the bar, that second snare is often displaced either back or forward by an eighth-note."
  - "the second kick in a bar is shifted back to the last 16th-note before beat 3." (neuro)
- Attack Magazine, "Raw Drum & Bass" (26 June 2012) and "Incessant Drum & Bass" (28 June 2012), https://www.attackmagazine.com/technique/beat-dissected/raw-drum-bass/ and .../incessant-drum-bass-beat/:
  - "168-178bpm"; "The 16th-measure ghost notes at the end of the beat are swung back slightly and play at a lower volume"
  - "The live-style on-the-eighth ride ... then a bitcrusher effect is used to change its harmonics."
  - "Enter the ride ..., which hits on every 1/8th."; "a new, busy ghost snare part."
- Wikipedia, "Amen break", https://en.wikipedia.org/wiki/Amen_break (last edited 29 Oct 2026):
  - "For two bars, Coleman plays the previous beat. In the third bar, he delays a snare hit."
  - "In the fourth bar, he leaves the first beat empty, then plays a syncopated pattern and an early crash cymbal."
  - Recording tempo "approximately 136 BPM" (sped up in DnB).
- MusicRadar, "How to make a classic reese bass like Renegade's Terrorist", https://www.musicradar.com/how-to/classic-reese-bass-renegade-terrorist (16 Feb 2023):
  - "the more characterful two-oscillator approach where you get phase cancellation between the two oscillators."
  - "We've set both of ours to sawtooth"; "We've set our offset to + and -55 cents."; "the phase cancellation causing the distinctive beating effect."
- Ableton Learning Music, "Tempo and genre" (no date): "Drum and bass: 160-180 bpm".

## 4. Synthwave

| Aspect | Finding |
|---|---|
| Tempo | 80-118 bpm common; upbeat tracks 128-140 |
| Meter / feel | 4/4, straight; backbeat or four on the floor for upbeat tracks |
| Drums | *illustrative* kick `x.......x.......` (or `x...x...x...x...`), snare `....x.......x...` with **gated reverb**, hats 8ths/16ths; '80s drum-machine voices |
| Bass | step-sequenced analogue bass: steady 16th notes, same length and velocity, through chord roots; sidechained to the drums so off-notes pulse |
| Pads / leads | analogue polysynth pads (supersaw-type detuned saws, brass, strings), four-note ascending arpeggios, mono lead with portamento |
| Structure | song form (instrumental verse/chorus), long reverb-washed sections |
| Signature techniques | gated reverb (compress a non-linear reverb tail, gate it), long reverbs, compressed reverb as a pad, tape-modelled delay, '80s film/game references (Carpenter, Vangelis, Tangerine Dream) |

**Sources and quotations**
- Wikipedia, "Synthwave", https://en.wikipedia.org/wiki/Synthwave (last edited 16 Oct 2026):
  - "Common tempos are between 80 and 118 BPM, while more upbeat tracks may be between 128 and 140 BPM."
  - "draws predominantly from 1980s films, video games, and cartoons, as well as composers such as John Carpenter, Jean-Michel Jarre, Vangelis, and Tangerine Dream."
  - "It is primarily an instrumental genre"
- MusicRadar, "How to make an '80s-style synth track in your DAW", https://www.musicradar.com/how-to/how-to-make-an-80s-style-synth-track-in-your-daw (5 Aug 2020):
  - "program a sequence of sixteenth notes, all with the same length and velocity, on the notes C, G, Bb and F"
  - "compressor sidechaining livened up the dynamics of sequenced basslines like this by allowing non-accent notes to poke out in between drum beats."
  - "a simple, ascending four-note arpeggio based on notes within each chord."
  - "Make your own gated reverb effect by heavily compressing a non-linear reverb tail, then placing a noise gate plugin across it to chop off the release."
- MusicRadar, "11 quick-fire tips for creating Stranger Things-style synthwave", https://musicradar.com/how-to/11-quick-fire-tips-for-creating-stranger-things-style-synthwave (5 July 2017):
  - "The use of long reverbs on melodic elements creates a massive reverby background."
  - "Try compressing the reverb to bring the ambience up in the mix for a sustained, pad-like timbre."
  - "use a delay modelled on a classic tape delay machine"
- Wikipedia, "Gated reverb", https://en.wikipedia.org/wiki/Gated_reverb (last edited 9 Oct 2026): "combines strong reverb and a noise gate that cuts the tail of the reverb."
- Wikipedia, "Roland JP-8000", https://en.wikipedia.org/wiki/Roland_JP-8000 (last edited 31 Oct 2026): "It emulates the sound of multiple detuned sawtooth oscillators." ... "resembles 7 sawtooth oscillators detuned against each other". (The supersaw's home genre is trance: "the JP-8000's Supersaw leads became characteristic of a certain type of trance music".)

## 5. Ambient

| Aspect | Finding |
|---|---|
| Tempo | free or very slow; often no beat; sometimes a slow pulse |
| Meter / feel | tone and atmosphere over rhythm; "as ignorable as it is interesting" |
| Drums | usually none (`................`) |
| Sound | drones and sustained/repeated notes; long evolving pads; textures from extreme processing (time-stretch, pitch-correct, granular); field/nature recordings; warm unobtrusive bass |
| Structure | slow evolution, layering in and out; no verse/chorus |
| Signature techniques | long reverb with low end cut (6-12 dB/oct at 300-400 Hz), tempo-synced tape delay band-limited 600 Hz-1.2 kHz, slow rotary speaker, freeze reverb, granular clouds (long grains + high density + time-stretch = smooth drone; jittered position) |

**Sources and quotations**
- Wikipedia, "Ambient music", https://en.wikipedia.org/wiki/Ambient_music (last edited 11 Nov 2026):
  - "a genre of music that emphasizes tone and atmosphere over traditional musical structure or rhythm."
  - "some works use sustained or repeated notes, as in drone music."
  - Eno: ambient music "must be as ignorable as it is interesting".
- Sound On Sound, "Sound Design For Ambient Music" (Paul White), https://www.soundonsound.com/techniques/sound-design-ambient-music (published November 2018):
  - "long, evolving sounds that can be used in place of traditional synth pads or drones"
  - "a generous helping of long reverb plus some simulated tape delay"
  - "a 6 or 12 dB/octave low-cut filter at 300-400 Hz, to stop the sound getting too muddy."
  - "a tape-delay emulation, sync'ed to the project tempo and with the lows rolled off at 600Hz and the highs at 1200Hz"
- Sound On Sound, "Granular Synthesis: A Practical Introduction" (Robin Bigwood), https://www.soundonsound.com/techniques/granular-synthesis-practical-introduction (published November 2022):
  - "splitting audio into small slices or 'grains', often just fractions of a second long, before later reconstituting them"
  - "a static Position will let you generate sustaining timbres from percussive and discontinuous source material"
  - "A dedicated parameter, variously called Jitter, Spray or similar"; "The Size (or length) of your grains, and the Density ... has a huge bearing on the final effect."

## 6. Lo-fi hip hop

| Aspect | Finding |
|---|---|
| Tempo | 60-80 bpm sweet spot (hip-hop generally 85-100) |
| Meter / feel | 4/4, **swing** (MPC-style, about 55-60%), loose, not snapped to grid, snare a touch late |
| Drums | boom bap: kick on downbeats, snare on upbeats; *illustrative* kick `x.........x.....`, snare `....x.......x...`, hats 16ths swung; low end filtered so the kick does not throb |
| Harmony / keys | jazzy seventh chords, ii-V-i and V-i; piano, Rhodes-like keys, clean guitar, organ; detuned/wobbling tape pitch |
| Texture | hiss, saturation, crackle, hum, wow/flutter, reduced bit depth/sample rate, tape emulation; samplers like the SP-404 |
| Structure | loop-based, steady energy, few fills, no dramatic choruses |

**Sources and quotations**
- Wikipedia, "Lo-fi hip hop", https://en.wikipedia.org/wiki/Lo-fi_hip_hop (last edited 26 May 2026):
  - "a style of lo-fi music that combines hip-hop beats with elements of chill-out."
  - Pitchfork, quoted: "poky tempos, cloying piano or guitar melodies, ersatz vinyl hiss, and other signifiers of inoffensive chill."
  - "features audio imperfections, distorted sound quality, and less professional audio equipment."
- MusicRadar, "Unpacking the modern lo-fi genre", https://www.musicradar.com/tutorials/we-have-the-youtube-generation-to-thank-for-its-explosion-unpacking-the-modern-lo-fi-genre (22 Oct 2024):
  - "a BPM sweet spot for lo-fi somewhere between 60-80bpm"
  - "Typically sub-frequencies and low-end information is filtered out, so the throb of the kick doesn't become too oppressive."
  - "Fills and human-like performance should be avoided, but so also should perfectly synchronised, snapped-to-grid patterns"
  - "lots of emotionally neutral sevenths and straighforward progressions (e.g: ii-V-i and V-i)."
  - "hiss, saturation, crackle, hum and digital processing errors (or a reduced bitrate)."
- Attack Magazine, "90s Boom Bap Hip Hop", https://www.attackmagazine.com/technique/beat-dissected/90s-boom-bap-hip-hop/ (8 May 2021):
  - "a hard-hitting, acoustic bass drum sample on the downbeats, a snappy acoustic snare drum sample on the upbeats"
  - snare: "We want it to feel 'late' to help with the swing."; kick swing "Swing MPC 3000 8ths 57"; "16th hi-hats".
- MusicRadar, "Beat Building: how to make a lo-fi hip-hop beat", https://www.musicradar.com/news/beat-building-how-to-make-lo-fi-hip-hop-beat (22 Aug 2022): "A classic bpm for hip-hop is between 85-100."; "we've added about 5% swing".

## 7. Cross-genre production techniques (for the engine's effect set)

| Technique | What it is | Source (date) |
|---|---|---|
| Sidechain compression | duck bass/pads when the kick hits; pumping | Wikipedia "Dynamic range compression" (14 Oct 2026); SOS Dubstep Basics (Jul 2010) |
| Supersaw | 7 detuned saws, nonlinear detune, mix of centre vs sides, high-passed sum | Wikipedia "Roland JP-8000" (31 Oct 2026); Szabo 2010 (Part 2) |
| Tempo-synced delay | delay time locked to note values; tape-style, band-limited repeats | SOS Ambient (Nov 2018) |
| White-noise riser | noise only, "draw in a note for eight bars", cutoff automated "from around 1000 Hz to 10 kHz", volume up toward the drop | MusicRadar "How to create an effective buildup and breakdown" (21 Feb 2024) |
| Snare roll | "add snare hits as eighth notes, then cut the note value in half to 1/16 notes, and then to 1/32 notes." | same (21 Feb 2024) |
| Breakdown filter | "begin with the filter at 200 Hz and open up the frequency of the filter to 20 kHz over the duration of the breakdown." | same |
| Bitcrushing / downsampling | lower bit depth or sample rate for grit | SOS Dubstep Secrets (Nov 2010); Attack Raw DnB (2012); Splice (2015) |
| Tape stop | "emulating the effect of a tape deck slowing down"; "The classic down-pitching effect is a staple of electronic music" | MusicRadar, VPS Tapestop how-to (30 Aug 2017) |
| Pitch-dived impact | NOT directly sourced. The same mechanism as a kick's pitch drop: SOS Synth Secrets on bass drums names "the pitch shift that occurs every time you excite the batter head" and the beater click ("a short noise burst") | SOS "Synthesizing Drums: The Bass Drum", Gordon Reid (Jan 2002) |
| Kick synthesis | "a triangle wave or a filtered sawtooth wave will be an ideal starting point"; single VCA/EG decay; pitch bend + click | same (Jan 2002) |

---

# PART 2: SYNTHESIS REFERENCES

**Rule applied.** A published METHOD (an algorithm, an equation, a measured curve) is free to implement
in our own words and code. CODE may be copied into a CC0 project only under CC0/public domain, MIT,
BSD, zlib, Apache-2.0 (keeping the required notices: MIT/BSD/Apache notices and the Apache NOTICE file
still travel with copied code even inside a CC0 repository; the copied file stays under its own
licence). GPL, LGPL and AGPL code is never copied in; learn from it only.

| Reference | Publisher | Licence quoted | Where read, date | Use | What it teaches |
|---|---|---|---|---|---|
| **fundsp** (Rust) | Sami Perttu | crates.io `MIT OR Apache-2.0`; README: "Licensed under either of Apache License, Version 2.0 or MIT license at your option." | GitHub API + crates.io API, 13 Nov 2026 (crate 0.23.0, 2026-01-07) | **COPY-OK** (or depend on it) | Audio graph notation; PolyBLEP saw/square/pulse ("`poly_saw` PolyBLEP"), DSF oscillators, wavetables; Simper SVF filter family (`src/svf.rs`: "Andrew Simper's state variable filters."); Moog ladder; reverb, chorus, granular, FFT resynthesis, oversampling |
| **dasp** (Rust) | RustAudio | crates.io `MIT OR Apache-2.0`; LICENSE-APACHE: "Copyright 2016 RustAudio Developers" | GitHub API + crates.io, 13 Nov 2026 (0.11.0, 2020) | **COPY-OK** | Sample/frame types, conversion, interpolation (linear, sinc), signal combinators, ring buffers, envelope detection |
| **glicol** (Rust) | Qichao Lan | LICENSE: "The MIT License (MIT) / Copyright (c) 2020-present Qichao Lan" (crates.io shows "non-standard" only because it uses `license-file`; the file is MIT) | GitHub API + crates.io, 13 Nov 2026 | **COPY-OK** | Graph-oriented live-coding language; sequencer syntax; `glicol_synth` node library |
| **rustysynth** (Rust) | Nobuaki Tanaka (sinshu) | `MIT`; LICENSE.txt: "MIT License / MeltySynth: Copyright (C) 2021 Nobuaki Tanaka" | GitHub API + crates.io, 13 Nov 2026 (1.3.8, 2026-10-20) | **COPY-OK** (each SoundFont has its own licence) | SoundFont 2 playback: voices, envelopes, modulators, reverb/chorus |
| **oxisynth** (Rust) | PolyMeilex | crates.io `LGPL-2.1`; repo has no root licence file (API 404); README: "inspired by the widely known `FluidSynth`" | GitHub API + crates.io, 13 Nov 2026 | **LEARN-ONLY** | SoundFont synthesis in Rust (FluidSynth lineage) |
| **twang** (Rust) | Jeron Aldaron Lau (repo now FuchsiaBytes/twang) | crates.io `Apache-2.0 OR BSL-1.0 OR MIT`; repo file "Apache License / Version 2.0, January 2004" | GitHub API + crates.io, 13 Nov 2026 (0.9.0, 2022-10-23) | **COPY-OK** | Pure-Rust oscillators and synthesis building blocks (stale since 2022) |
| **knyst** (Rust) | Erik Natanael (ErikNatanael) | crates.io `MIT OR Apache-2.0`; repo LICENSE_APACHE2 "Apache License / Version 2.0" | GitHub API + crates.io, 13 Nov 2026 (0.5.1, 2024-11-25) | **COPY-OK** | Real-time audio graph that can be changed while running; scheduling |
| **Surge XT** | Surge Synth Team | `GPL-3.0`: "GNU GENERAL PUBLIC LICENSE / Version 3, 29 June 2007" | GitHub API, 13 Nov 2026 | **LEARN-ONLY** | Hybrid synth: many oscillator types, filters, modulation, effects |
| **Vital** | Matt Tytel | `GPL-3.0`; README: "The source code is licensed under the GPLv3."; "Do not distribute the presets that come with the free version of Vital." Names are trademarks. | GitHub API, 13 Nov 2026 | **LEARN-ONLY** | Spectral-warping wavetable synthesis, modulation UX |
| **Vitalium** | DISTRHO (fork of Vital in DISTRHO-Ports) | GPL-3.0 by derivation (DISTRHO-Ports has no root licence file per API; not separately verified) | GitHub API, 13 Nov 2026 | **LEARN-ONLY** | Same as Vital |
| **Helm** | Matt Tytel | `GPL-3.0` (COPYING) | GitHub API, 13 Nov 2026 | **LEARN-ONLY** | Subtractive polysynth, modulation |
| **Dexed** | asb2m10 (Pascal Gauthier) | Repo `GPL-3.0`. **But** `source/msfa/*` headers: "Copyright 2012 Google Inc. / Licensed under the Apache License, Version 2.0"; upstream google/music-synthesizer-for-android is `Apache-2.0` | GitHub API, 13 Nov 2026 | Dexed: **LEARN-ONLY**. MSFA engine: **COPY-OK from Google's upstream repo** (not Dexed's GPL-era edits) | DX7 6-operator FM: envelopes, exp/log tables, operator kernel, algorithms |
| **ZynAddSubFX** | ZynAddSubFX project | `GPL-2.0` (COPYING) | GitHub API, 13 Nov 2026 | **LEARN-ONLY** | Additive/subtractive/PAD synthesis |
| **Odin 2** | TheWaveWarden | LICENSE: "The Odin 2 source code is distributed under the GNU GPLv3 license" (font under OFL) | GitHub API, 13 Nov 2026 | **LEARN-ONLY** | Semi-modular synth, analog-modelled filters |
| **VCV Rack** | VCV | LICENSE.md: GPL-3.0-or-later plus “you are granted the "VCV Rack Non-Commercial Plugin License Exception"” and a commercial option | GitHub API, 13 Nov 2026 | **LEARN-ONLY** | Modular patching model |
| **Cardinal** | DISTRHO | `GPL-3.0` | GitHub API, 13 Nov 2026 | **LEARN-ONLY** | Rack as a plugin |
| **Mutable Instruments eurorack** | Emilie Gillet | README: "Code (AVR projects): GPL3.0. Code (STM32F projects): MIT license. Hardware: cc-by-sa-3.0"; `plaits/.../variable_saw_oscillator.h` header is MIT ("Copyright 2016 Emilie Gillet. Permission is hereby granted, free of charge"). "Mutable Instruments is a registered trademark." | GitHub API, 13 Nov 2026 | **COPY-OK for STM32 module code** (Plaits, Braids, Rings, Elements, Clouds, Beads, Warps, Marbles...) after checking each file's header; AVR projects LEARN-ONLY; never use the names | Macro-oscillators (Plaits: VA, FM, formant, wavetable, chords, drums), modal/physical resonators (Rings, Elements), granular textures (Clouds/Beads); band-limited oscillators |
| **Airwindows** | Chris Johnson | `MIT`: "MIT License / Copyright (c) 2018 Chris Johnson" | GitHub API, 13 Nov 2026 | **COPY-OK** | Hundreds of small effects: saturation, tape, console emulation, reverbs, dither |
| **DaisySP** | Electrosmith (repo now daisyaudio/DaisySP) | LICENSE: "DaisySP, copyright (c) 2020 Electrosmith, Corp. / Published under the MIT license". **But** the `DaisySP-LGPL` submodule: "a set of LGPL licensed DSP ... modules" (reverbsc, moogladder, blosc, pluck, compressor, bitcrush, biquad, allpass, comb, tone, port, jitter ...) | GitHub API, 13 Nov 2026 | **COPY-OK for the MIT tree only**; DaisySP-LGPL **LEARN-ONLY** | Compact embedded-style oscillators, envelopes, filters, drums (analog bass/snare drum models), effects |
| **STK** (Synthesis ToolKit) | Perry R. Cook and Gary P. Scavone | LICENSE: "Permission is hereby granted, free of charge..." plus a request to send modifications back: "This is, however, not a binding provision of this license." | GitHub API, 13 Nov 2026 | **COPY-OK** (MIT-style) | Physical models (plucked string, bowed, blown), FreeVerb port, filters, envelopes |
| **Faust compiler** | GRAME / INRIA | COPYING.txt: "GNU Lesser General Public License as published by the Free Software Foundation; either version 2.1" | GitHub API, 13 Nov 2026 | Tool only | Has a Rust backend (`compiler/generator/rust`) |
| **Faust libraries** | GRAME (some functions others) | LGPL with exception: "grants you the right to freely choose the license for the resulting compiled code." ("Except where noted otherwise"; a `licenses/stk-4.3.0.md` exists) | GitHub API (`oscillators.lib` header), 13 Nov 2026 | **COPY-OK only for Faust-COMPILED output** (e.g. Faust to Rust, then edited); hand-porting `.lib` source is LGPL, LEARN-ONLY | Huge vetted DSP library: PolyBLEP oscillators, VA filters (`vaeffects.lib`), reverbs, physical models, a DX7 set |
| **Csound** | Csound community | `LGPL-2.1` | GitHub API, 13 Nov 2026 | **LEARN-ONLY** | Opcodes (reverbsc, moogladder, pluck) |
| **SuperCollider** | SuperCollider project | `GPL-3.0` | GitHub API, 13 Nov 2026 | **LEARN-ONLY** | UGens, patterns, server/language split |
| **Pure Data** | Miller Puckette and others | LICENSE.txt: “terms (the "Standard Improved BSD License") apply to all files associated with the software unless explicitly disclaimed in individual files” | GitHub API, 13 Nov 2026 (puredata.info itself: robots.txt returned 403, not read) | **COPY-OK** (BSD-3, keep notice; check per-file) | Dataflow patching; tilde objects (osc~, vcf~, lop~) |
| **Sonic Pi** | Samuel Aaron and contributors | LICENSE.md: main source "The MIT License (MIT)"; "distributors of Sonic Pi GUI binaries must comply with the terms of the GPL."; `app/web/` "GNU Affero General Public License v3.0 or later" | GitHub API, 13 Nov 2026 | **COPY-OK for MIT `app/` code**; never `app/web/` | Live-coding patterns; synth definitions written for teaching |
| **Freeverb** | Jezar at Dreampoint | Original readme: "Freeverb - Free, studio-quality reverb SOURCE CODE in the public domain"; "Why is the Freeverb code now public domain?" | Copy of the original readme in muse-sequencer/muse (`src/plugins/freeverb/readme.txt`), GitHub API, 13 Nov 2026 | **COPY-OK** (public domain) | STK: "uses 8 lowpass-feedback-comb-filters in parallel, followed by 4 Schroeder allpass filters in series"; JOS: based on "the June 2000 version of Freeverb" |
| **musicdsp.org archive** | Music-DSP community (site source bdejong/musicdsp) | No licence for the archive: footer "© Copyright 2019, Too many to list"; repo has no licence file (API 404) | www.musicdsp.org + GitHub API, 13 Nov 2026 | **LEARN-ONLY** unless a snippet states its own licence | Classic snippets: filters, oscillators, envelopes |
| Välimäki, Pekonen, Nam (2012), "Perceptually informed synthesis of bandlimited classical waveforms using integrated polynomial interpolation", J. Acoust. Soc. Am. 131(1): 974-986, DOI 10.1121/1.3651227 | Acoustical Society of America | "(c) 2012 Acoustical Society of America" | Author-hosted PDF, mac.kaist.ac.kr, 13 Nov 2026 | **LEARN-METHOD** | polyBLEP: "The polynomial correction function can be added onto samples around each discontinuity in a non-bandlimited waveform to suppress aliasing."; best: "integrated third-order B-spline", alias-free saw "up to the fundamental frequency of 7.8 kHz at the sample rate of 44.1 kHz" |
| Nam, Välimäki, Abel, Smith (2010), "Efficient Antialiasing Oscillator Algorithms Using Low-Order Fractional Delay Filters", IEEE TASLP 18(4): 773- | IEEE | IEEE copyright (author-hosted copy) | mac.kaist.ac.kr PDF, 13 Nov 2026 | **LEARN-METHOD** | BLIT with low-order fractional delays; "pulse width modulation, hard-sync, and super-saw are also presented" |
| Välimäki and Huovilainen (2007), "Antialiasing Oscillators in Subtractive Synthesis", IEEE Signal Processing Magazine | IEEE | not read | Search result only | **LEARN-METHOD** | The original closed-form polyBLEP |
| Andrew Simper (Cytomic), "SvfLinearTrapOptimised2.pdf" | Cytomic | not read | **cytomic.com robots.txt returned 403; not read.** Known via fundsp's source comment and README | **LEARN-METHOD** (fundsp's implementation is COPY-OK) | Trapezoidal (zero-delay-feedback) state-variable filter, all modes from one structure |
| Vadim Zavalishin, "The Art of VA Filter Design" (rev. 2.1.0 / 2.1.2) | Native Instruments (hosted) | Not read. A search-result snippet of the 2.1.0 PDF says the author grants the right to freely copy that revision in full with its notice (unverified) | NI URLs for 2.1.0 and 2.1.2 returned 404; the mirror (discodsp.net) disallows all readers in robots.txt; KVR thread (page 10) read for the links | **LEARN-METHOD** | Topology-preserving transform, zero-delay feedback, ladder/SVF/Sallen-Key nonlinear models |
| Adam Szabo (2010), "How to Emulate the Super Saw", BSc thesis, KTH (TRITA-CSC-E 2010:131) | KTH Royal Institute of Technology | No licence or copyright statement found in the text | www.adamszabo.com PDF, 13 Nov 2026 | **LEARN-METHOD** | "layering seven sawtooth oscillators"; detune curve "not linear" (an "11th order polynomial" fit); mix: centre falls linearly, sides rise as a parabola; "the added signal of the 7 waveforms goes through a high pass filter"; phases analysed |
| Jon Dattorro (1997), "Effect Design Part 1: Reverberator and Other Filters", J. Audio Eng. Soc. 45(9) (Part 2 likewise) | AES | not read | **Not read: robots.txt** (ccrma.stanford.edu and aes2.org disallow ClaudeBot; aes.org robots returned 403). Citation from the AES table of contents in a search result | **LEARN-METHOD** | The plate-reverb topology most VA reverbs follow (unverified here) |
| Julius O. Smith III, online books (PASP, Spectral Audio, Filters, Mathematics of the DFT) | CCRMA / W3K | No licence statement on the DSPRelated mirror pages read | **CCRMA: not read, robots.txt** (disallows ClaudeBot). DSPRelated mirror (dsprelated.com/freebooks/pasp/) read, 13 Nov 2026 | **LEARN-METHOD** | Schroeder/Freeverb/FDN reverb, digital waveguides, filters. Freeverb page: “a public domain C++ program by ``Jezar at Dreampoint''” |
| John Chowning (1973), "The Synthesis of Complex Audio Spectra by Means of Frequency Modulation", J. Audio Eng. Soc. 21(7) | AES | Paper not read (AES site barred, above) | Cited via Wikipedia "Frequency modulation synthesis" (last edited 1 Nov 2026) | **LEARN-METHOD**; patent gone: "With the expiration of the Stanford University FM patent in 1995, digital FM synthesis can now be implemented freely by other manufacturers." (US 4,018,121) | FM: carrier/modulator ratio and index set the sidebands |
| Wavetable synthesis | (method) | n/a | Wikipedia "Wavetable synthesis" (last edited 8 Nov 2026) | **LEARN-METHOD** | "Each waveform normally consists of a single cycle of the wave."; "Digital interpolation between adjacent waveforms allows for dynamic and smooth changes of the timbre"; Palm (PPG) published 1979 |
| Sound On Sound, Synth Secrets (Gordon Reid, 1999-2004) | SOS Publications | Magazine copyright (text) | soundonsound.com, bass-drum part (Jan 2002), 13 Nov 2026 | **LEARN-METHOD** | Synthesising drums and classic patches from first principles |

### Licence surprises
1. **Faust libraries' exception** lets Faust-COMPILED output (Faust has a Rust backend) carry any licence we choose, CC0 included by its wording; hand-translating the `.lib` source does not get the exception.
2. **DaisySP is MIT, but its `DaisySP-LGPL` submodule is not**: reverbsc, moogladder, blosc, pluck, compressor, bitcrush, biquad and others are LGPL. Copy only from the MIT tree.
3. **Dexed is GPL-3, but its DX7 engine (MSFA) is Apache-2.0 from Google**: take it from google/music-synthesizer-for-android, not from Dexed.
4. **Mutable Instruments splits by chip**: STM32 module code MIT, AVR projects GPL-3, hardware CC-BY-SA; the name is a registered trademark.
5. **musicdsp.org has no licence at all** for its archive: treat snippets as learn-only unless one states its own terms.
6. **Sonic Pi**: main code MIT, but its web edition is AGPL-3 and its binaries carry GPL obligations.
7. **oxisynth** (Rust) is LGPL-2.1 (FluidSynth lineage), unlike rustysynth (MIT).
8. **glicol** shows "non-standard" on crates.io; its licence file is plain MIT.
9. **Vital's presets** are under a separate licence that forbids redistribution, apart from the GPL code.

---

# Sources not reached
- **ccrma.stanford.edu** (Julius O. Smith's books, Dattorro's Effect Design PDFs): not read, robots.txt disallows ClaudeBot / Claude-Web / anthropic-ai. JOS read via the DSPRelated mirror instead.
- **cytomic.com** (Simper SVF papers): robots.txt returned HTTP 403; could not read, so not read.
- **aes.org**: robots.txt returned 403, not read. **aes2.org** (AES e-library): robots.txt disallows ClaudeBot; not read. So Chowning 1973 and Dattorro 1997 were not read in the original.
- **valhalladsp.com** (Sean Costello's reverb-papers guide): robots.txt allows, but the page returned HTTP 403; could not read.
- **discodsp.net** (mirror of Zavalishin's book): robots.txt disallows everything but /news/; not read. Native Instruments' PDF URLs for revisions 2.1.0 and 2.1.2 returned 404.
- **puredata.info**: robots.txt returned 403; licence taken from the GitHub repository instead.
- **lib.rs**: robots.txt returned 403; crates.io used instead.
- **musictech.com**: robots.txt disallows ClaudeBot (not needed in the end).
- **freesound.org, kenney.nl**: barred by the brief; not touched.
- Välimäki and Huovilainen 2007 (IEEE SPM): seen only in search results.
- No source read quoted a typical "pitch-dived impact"; the kick-drum pitch-drop source stands in for the mechanism.
- The Amen-style and several other grids are marked *illustrative*: typed by the researcher from common transcriptions, not from a quotation.
