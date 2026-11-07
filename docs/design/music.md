# Music and the ship's hum: a calm score composed live, tuned to A = 432 Hz

**Status: design 2026-11-06.** A listenable direction sketch exists (section 19). **Built the same
day: rung 6's instrument library** (section 23): the CC0 libraries downloaded and recorded, the
pipeline that makes instruments from them (`just instrument-packs`), the core set inside the app,
the downloadable packs, Settings > Audio > Instruments, and the choice of instrument per part with
synthesis as the fallback. The sampler voice inside the score waits for the engine's crate. The whole feature is planned here with all its parts (CLAUDE.md, "Plan the whole
feature, build it in rungs"), then cut into rungs (section 20), smallest first. What can legally be
shipped, and what the research says about 432 Hz, "healing frequencies" and brown noise, rests on
the dated finding
[`docs/reference/findings/2026-11-06-music-sources.md`](../reference/findings/2026-11-06-music-sources.md).

## 0. In plain words

- HumanityOS gets its own music: calm, slow and spacious, orchestral pads and strings, a wordless
  choir, soft bells and piano, deep sci-fi synths, and now and then the warm weight of a low,
  clean electric guitar. It plays in the game's menus and in the world, made for hanging out on the
  mothership.
- **The music is composed live by the app**, the way a musician improvises within rules. It never
  plays the same thing twice, and it follows where you are (your home, the Commons, the garden, near
  the engines), the ship's time of day, and what you are doing. Nothing is downloaded and nothing
  goes over the network.
- **Every note is tuned to A = 432 Hz by default**, the operator's choice; A = 440 Hz is one tap
  away. It is offered as a matter of taste: the app does not claim 432 does anything for health.
- **The ship hums.** A soft, deep sound of engines and the ship's structure plays under everything
  while you are aboard, louder and lower near the drive, softer in small rooms, tuned to the
  music's key so the two never clash. It has its own switch and volume, separate from the music.
- **An optional "tones" layer** plays steady tones for people who like them, off unless you turn it
  on, labelled plainly: some people find these tones calming; there is no good evidence that they
  heal.

## 1. Why, in the operator's words (2026-11-06)

> Can you make music for HumanityOS? Like chill orchestral sci-fi operatic soft metal? We need
> something that can play in the menus and in-game. Designed for a peaceful chill environment
> hanging out on the mothership. In 432Hz. And using other healing frequencies?

And, the same evening:

> I was also thinking we could incorporate a subtle brown noise that acts like the hum of the
> engines and the resonance of everything?

## 2. What exists today (read 2026-11-06)

- **The audio engine runs, for sound effects.** `crates/humanity-core/src/audio/mod.rs` wraps kira
  0.12. The engine builds an `AudioManager` at start (`src/engine/launch_focus.rs`, an `Option`, so a
  machine without a sound device still boots), and every frame `src/lib.rs` pushes the Settings
  volumes (master, music, SFX, UI) into it and plays footsteps, clicks and the in-world video
  screens' sound. The shipped sounds are Kenney's CC0 packs in `assets/audio/ui/` and
  `assets/audio/sfx/`.
- **Music has an API and nothing else.** `AudioManager::play_music` (loop a file, fade the previous
  one out over 500 ms) has no caller. `data/sounds.toml` lists ten `[music.*]` entries (menu_theme,
  exploration, peaceful, space_travel, ...) whose files do not exist. `docs/STATUS.md`'s line "kira
  crate, spatial 3D audio, music, SFX (v0.39.0)" overstated it (corrected the same day).
- **Settings > Audio > Volumes** has Master, Music, SFX and UI sliders and an Interface sounds
  toggle (`src/gui/pages/settings.rs`, `draw_audio_content`). There is no ambience volume. In
  `AudioManager`, every bus other than "ui" plays at the SFX volume, so the catalog's "ambient" bus
  is not yet its own category.
- **kira lets us render our own sound on the audio thread.** kira 0.12's `Sound` trait has
  `process(&mut self, out: &mut [Frame], dt: f64, info: &Info)`, called for each block the sound
  card asks for (kira's default internal block is 128 frames, 2.67 ms at 48 kHz), and its docs say
  its methods "should not allocate or deallocate memory". kira also has `command_writer_and_reader`,
  a latest-value-wins channel from the game to a playing sound.
- **The game knows where the player is aboard.** `src/engine/home_plot.rs` `aboard()` and
  `state.aboard_bounds` (the ship's box, `crates/humanity-ship/src/ship_space.rs`); the room the
  player stands in (`src/gui/pages/hud.rs` `room_at`, over `RoomBounds` with its box, so its size,
  and its purpose from `data/rooms.ron`); the zone's purpose (`ShipZone::purpose`: residence,
  commons, street, bay, agriculture, corridor); whose air a point breathes (`ship_space`). The ship's
  day follows the simulation's clock (`time::local_hour`, `solar::sun_factor`). **What it does not
  know is where the engines are**: the shipped ship (`data/blueprints/ship_structure.ron`) has no
  engine or drive space, and `data/ships/reactor.ron` describes a reactor with no position.
- **Loudness levelling is being designed in parallel** (the 2026-11-06 loudness work: categories
  such as speech, sound effects, music and interface, each with a target loudness, measured with
  ITU-R BS.1770). This design fits it: music and ambience are two of its categories (section 13).
- `docs/dev/adding-sounds-music.md` (2026-07-20) still says "The game currently plays no sound";
  its status table is stale since sounds were wired in v0.960.

## 3. The rules this follows

- **Open source only, build what does not exist** (CLAUDE.md). The composer, the instruments and
  the reverb are ours. Samples, where used, are public domain (CC0) libraries the finding cleared.
  No proprietary tool, no paid service, no AI service.
- **Realistic-first: the 2030 version.** For an endless calm world that is a generative, adaptive
  score composed live (section 4.1), not a playlist of loops.
- **Infinite-of-x.** Instruments, moods, scales, chord graphs, motifs, places and hum sources are
  data files under `data/music/` and `data/audio/`, never arrays in code.
- **Compute and bandwidth are a hard budget.** The score and the hum run locally on the audio
  thread within a stated budget (section 14). Nothing about music is networked, ever.
- **No caches across runs.** Nothing the music engine computes is kept between runs. Sample
  subsets rendered or trimmed at build time and committed under `data/` are data from the
  repository, not a cache.
- **No health claim the evidence cannot carry** (the finding, Part E; memory
  `project_teaching_data_needs_refutation`). The words in section 15 are the only things the app
  says about tuning, tones and the hum.
- **Two realities.** The app's everyday pages are real life; the game is a place you walk into. The
  music belongs to the game first (section 12).
- **GUI-first, words by key, theme tokens, dual-UI parity.** Every control in Settings, every word
  from `data/i18n/en.json`, every control with a hint; the website mirrors it (rung 9).
- **Dev tooling is permanent.** The Dev page's music panel, the offline renderer and the probes
  stay (section 16).

## 4. The approach

### 4.1 Why live and generative, not recorded tracks

A calm place people stay in for hours wears out any loop: a three-minute track heard twenty times
is noise. A live score:

- **never repeats**, because it composes from rules and a seed rather than playing back;
- **follows the world**: the place, the time of day and what the player is doing steer it in real
  time, with no seams;
- **obeys the tuning exactly**: every pitch is computed, so A = 432 or 440 changes every note,
  sampled ones included (section 9);
- **costs almost nothing on disk**: synthesised voices are code; sampled voices need only small
  curated sets;
- **has no licence tangle**: we wrote every note's rules and own nothing anyone else can claim.

Procedural game scores have shipped for years (Spore's and No Man's Sky's are well known); what is
new here is doing it entirely with open, data-driven parts, inside a strict audio budget, and
making it place-aware aboard a ship.

### 4.2 The pieces

```
 game thread (each frame)                 audio thread (each 128-frame block)
 ------------------------                 -----------------------------------
 MusicContext (Copy):                     ScoreSound  (kira::sound::Sound)
   in_menu, aboard, room size + purpose,    - Composer: reads the latest context, picks the
   zone purpose, near-drive distance,         mood, plans phrases, chords, motifs (once a beat)
   time of day, activity, intensity,        - Voice pool (pre-allocated): synth, choir,
   tuning A4, duck amount, music on/off       modal (bells), plucked, sampler voices
         |                                  - Mix: per-role levels, FDN reverb, soft limiter
         |  kira command_writer_and_reader  - Loudness: BS.1770 meter, slow trim to target
         +--------------------------------> AmbientBed  (kira::sound::Sound, its own track)
                                            - brown noise, resonances at the key root,
 Dev page <-- MusicStatus (Copy, a           breathing, place-shaped level and colour
 triple buffer the audio thread writes)
```

- **The composer runs on the audio thread, inside the score's own `Sound`.** It makes decisions
  about once a beat, from pre-allocated tables, so timing is exact to the sample and never depends on
  the frame rate. The game sends only a small `Copy` context through kira's latest-value channel;
  the audio thread sends back a small `Copy` status (mood, chord, key, voices, cost, loudness) for
  the Dev page.
- **Nothing allocates in `process()`**: voices come from a fixed pool (64), buffers are sized at
  creation, mood data is parsed on the game thread and moved into the sound when it is created. Hot
  reload of a mood file starts a new `ScoreSound` and crossfades it in over a phrase.
- **Deterministic for tests and rigs**: the composer's randomness is a seeded generator. The seed is
  random per session for play, fixed by `HUMANITY_MUSIC_SEED` for rigs and tests, and shown on the
  Dev page.
- **The ambient bed is a second `Sound` on its own kira track**, so turning music off leaves the hum
  and the other way round; it reads the same context (it needs the key root and the place).

### 4.3 Where the code goes

- **A new crate, `crates/humanity-music`**: pure Rust, no kira, no GPU, no I/O on the hot path. It
  holds the composer, the voices, the effects, the bed and the loudness trim, and renders into a
  plain `&mut [[f32; 2]]`. Pure means it is testable headless at faster than real time, it can
  render offline (`just music-render`, section 16), and it compiles to WebAssembly for the website's
  AudioWorklet (rung 9). It gets a crate card in `docs/crates/` when built, per the crate split.
- **A thin adapter in `humanity_core::audio`** (native only, like the rest of kira's use):
  `ScoreSound` and `AmbientBedSound` implement kira's `SoundData`/`Sound` over the crate, plus
  `AudioManager::start_score`, `set_music_context`, and an ambience volume beside music, SFX and UI.
- **The engine side** (`src/engine/music.rs`, native only): builds `MusicContext` each frame from
  state the game already has (section 8) and hands it over. No change to `src/lib.rs` beyond one
  call in the audio block.
- **The relay build never sees any of it** (`cargo check --features relay --no-default-features`
  stays green).

## 5. The operator's blend, made concrete

| He said | What it becomes in the score |
|---|---|
| **Chill** | Slow: a 52 to 72 beats-per-minute feel, often floating with no felt beat at all; chords that last two to four bars; soft dynamics (pianissimo to mezzo-forte), swells over whole phrases, no sudden accents; quiet gaps between pieces. |
| **Orchestral** | Sustained string sections (sampled, CC0) and synth string pads, harp and piano arpeggios, soft horn swells; voicings spread over four octaves; smooth voice leading (shared notes held, others moving by step). |
| **Sci-fi** | Detuned saw and wavetable pads through slowly moving filters; FM and modal bells; glassy textures (wine glasses, vibraphone with long tails); a big, modulated reverb; a "shimmer" (the reverb fed an octave up) on high voices; Lydian colour (the raised fourth) for wonder. |
| **Operatic** | A wordless choir ("ah", "oh", "oo") from formant synthesis as a pad; now and then a single high soprano line, slow and legato with real vibrato, carrying a motif. No words. |
| **Soft metal** | Colour, never aggression: a low, clean or lightly driven electric guitar playing slow volume-swelled power chords and open fifths (CC0 hollowbody samples through our own amplifier and cabinet model), drop-tuned sub drones, and in "lift" moments only, slow half-time drums felt more than heard. |

Harmony: modal (Ionian, Lydian, Dorian, Aeolian, Mixolydian), extended chords (add9, maj7, maj9,
sus2, sus4, m9, m11, maj7#11), pedal points under moving chords, key changes only by pivot chords
and only at section ends. Form: phrases of 8 or 16 bars, sections of four phrases, a quiet gap of
20 to 90 seconds now and then. Motifs: each mood has a small bank of three-to-five-note motifs the
composer repeats with variation (transposed to fit the chord, rhythm loosened, a note changed), so
it sounds composed rather than random.

What the research supports (finding E3) is built in: slow tempo, low rhythmic and harmonic
complexity, gentle dynamics, pauses, and choice (moods people can pick and steer, because
preference and familiarity matter as much as the music's properties). None of it is described to
players as therapy.

## 6. Instruments as data

`data/music/instruments/<id>.ron`, one file per instrument; kinds are the engine's code, instruments
are data.

| Kind | What it is | Honest quality | Cost |
|---|---|---|---|
| `synth` | 1 to 4 oscillators (band-limited saw, square, triangle, sine, wavetable, noise), detune, a state-variable filter, envelopes and LFOs routed by the data | Excellent for pads, drones, bass, soft leads: this is synthesis's home ground | low |
| `choir` | formant synthesis: a glottal-like source with vibrato, jitter and breath through five vowel formants that can glide between vowels; 2 to 4 "singers" per note | Good as a choir **pad** ("ah", "oh"). It does not sing words, and a solo voice sounds synthetic up close | medium |
| `modal` | a sum of decaying partials at measured ratios (bells, chimes, celesta, glass, mallets) | Very good for bells and mallets; the physics is simple | low |
| `plucked` | extended Karplus-Strong (string damping, pick position, body filter) | Fair for harp, pizzicato, soft plucks; a clean guitar is recognisable, a convincing one is not | low |
| `sampler` | plays CC0 recordings, pitched by playback rate, velocity layers, release tails, round-robin | Good for piano, harp, bells, vibraphone and slow string pads. Without dedicated legato recordings, string and horn lines do not connect like a real section | medium, memory |
| `guitar` | the sampler on DI-like recordings plus our own amplifier (waveshaper with tone stack) and cabinet (filters) model | Fair to good for clean swells and light crunch. **Heavily distorted, palm-muted metal guitar is the hardest thing on this list**: no CC0 library for it was found, and synthesising it convincingly is not realistic, so the score uses guitar as colour (section 5) | medium |
| `percussion` | sampler hits (timpani, soft toms, gongs, mark trees) | Good, used sparingly | low |

The hardest to make convincing, in order: a distorted metal guitar, a solo operatic voice, a
legato string or brass section, a sung choir with words. The design leans on what each technique
does well and keeps the hard ones to colour.

Example (shape only, values illustrative):

```ron
// data/music/instruments/soft_saw_pad.ron
Instrument(
    id: "soft_saw_pad",
    words: "music.instrument.soft_saw_pad",
    kind: Synth((
        oscillators: [ (wave: Saw, detune_cents: -7), (wave: Saw, detune_cents: 0.5), (wave: Saw, detune_cents: 6.5) ],
        spread: 0.65,
        filter: (kind: LowPass, cutoff_hz: 850, cutoff_lfo_hz: 650, lfo_rate_hz: 0.045, key_track: 1.5, q: 0.75),
        envelope: (attack_s: 3.5, release_s: 5.5),
    )),
    range: (low: "D2", high: "A5"),
    reverb_send: 0.4,
)
```

Sampled instruments name their folder under `data/music/samples/<id>/`, which carries the library's
licence text and a `source.ron` (library, URL, licence, date fetched, which notes and layers were
kept), checked by a lint. **Built 2026-11-06** (section 23): the `Sampler` kind's real shape is
`src/instrument_packs/definition.rs` (zones of key range, velocity range and round robin, each with
the frequency it was recorded at), written by `just instrument-packs`; each definition also names the
`part` of the score it plays and a `rank` (the best installed wins), and carries `licence`, `source`
and `credit`.

## 7. Moods and compositions as data

`data/music/moods/<id>.ron`. A mood is a style the composer improvises in:

- `words` (its name, by key), `keys` (allowed key centres and modes, for example D Lydian, B
  Aeolian), `chord_graph` (chords as degrees with weighted moves between them, plus cadences),
  `phrase_bars` and `chord_bars` ranges, `tempo_bpm` range and whether a beat is felt, `meter`;
- `roles`: which instruments play pad, bass, drone, choir, melody, texture, bells, percussion, each
  with a density, a register and a level range;
- `intensity`: 0 to 1, and which roles join at which intensity (layers in, layers out);
- `dynamics` (loudest and softest, longest swell), `gaps` (how often a quiet gap and how long),
  `motifs` (the bank), `transitions` (which moods it moves to smoothly and how).

The first four:

| Mood | Where and when | Keys and harmony | Tempo | Voices |
|---|---|---|---|---|
| `mothership_calm` | aboard, homes and the Commons by day | D Lydian, B Aeolian; maj9, m9, maj7#11, sus2 | 56 to 64, beat barely felt | synth pads, choir pad, modal bells, drone, sparse piano |
| `menu` | the game's menus, the character showroom | the same keys, simpler; long held chords | 60, no beat | wide pads, slow choir, one motif in bells, a low guitar swell every few minutes |
| `garden_morning` | agriculture zones, ship morning | G Ionian, E Dorian; add9, sus4 | 66 to 72, gentle pulse | harp and piano arpeggios, vibraphone, light strings, birdsong-like glass motifs |
| `night_watch` | ship night, corridors, the observation spaces | A Aeolian, F# Dorian; m11, open fifths | 52 to 58, no beat | deep pads, low strings, distant choir "oo", guitar fifths far away, the hum more present |

`data/music/places.ron` maps a place (zone purpose, room purpose, room id) and a time of day to
mood weights; the composer moves toward the heaviest mood at the next phrase boundary.

## 8. Adaptivity: what steers it

Inputs, all from state the game already has (section 2):

- **menu or world**: the game's menus, the escape menu, the showroom, or the world;
- **where**: aboard or not; zone purpose; room purpose and size; distance to the nearest hum source;
- **when**: the ship's local hour and sun factor (morning, day, evening, night);
- **doing**: idle, walking, building (the construction editor open), tending crops, crafting,
  travelling between spaces; turned into an intensity from 0 to 1, smoothed over tens of seconds;
- **events**: arriving home, a harvest finished, a guest at the door: small musical gestures
  ("stingers": a bell motif, a choir swell) placed on the next beat, never louder than the score;
- **other sound**: a voice call talking, a video playing, a notification.

Responses:

- **Mood changes at phrase boundaries** with a crossfade of one to two phrases (8 to 30 seconds),
  through a shared pivot chord, so nothing jumps.
- **Intensity** adds and removes layers (vertical layering) and widens or narrows the tempo within
  the mood's range; it never makes the score loud.
- **Ducking**: while someone speaks in a voice call the score drops by about 9 dB over 300 ms and
  comes back over 2 s; while a video plays with sound, the score pauses at a phrase end.

## 9. Tuning: A = 432 Hz or A = 440 Hz

- One function makes every frequency: `f = a4 x 2^((midi - 69) / 12)`, with `a4` from Settings
  (432 by default, the operator's wish; 440 the other choice). Equal temperament throughout.
- Synthesised voices take the frequency directly. Sampled voices play at
  `rate = f_wanted / f_recorded`; at 432 every sample plays 31.77 cents lower than at 440 (a 1.8%
  slower rate), far too small to change the instrument's character.
- The hum's resonances follow the score's key root at the same tuning (section 11), so the bed and
  the music agree.
- Changing the setting retunes from the next note; held notes glide over half a second.
- The sketch proves it measurably: at 432 the drone's D2 measures 72.08 Hz and its A2 108.00 Hz; at
  440, 73.42 Hz and 110.00 Hz (section 19).

## 10. The optional "tones" layer

The operator asked for "other healing frequencies". The finding found no good evidence that any
frequency heals (Part E2), and the app will not say one does. What is offered:

- A layer of steady, soft tones (the "solfeggio" set as data: 174, 285, 396, 417, 528, 639, 741, 852
  and 963 Hz, plus any frequency typed in), pure or gently shaped, with a slow swell.
- **Off by default.** In Settings under the music, with the words in section 15.
- **These fixed frequencies are not notes of the score's scale** (528 Hz falls between C5 and C#5 at
  either tuning), so a tone under the music would clash. The tone therefore plays only while the
  music is off, or in the music's quiet gaps, and the hum keeps its own key. (Question 5 offers the
  other way: retune the score to the chosen tone.)
- No binaural beats: they need headphones, and the evidence read is mixed; not planned.
- Cost: one oscillator and an envelope, negligible.

## 11. The ship's ambient bed: the hum

**What it is.** A layer beside the music, in its own category (ambience) with its own volume:
turning music off keeps the hum, and turning the hum off keeps the music.

**How it is made** (per channel, per sample; all of it in the sketch, section 19):

1. Brown noise (1/f^2): a leaky integral of white noise, the two channels partly correlated so it
   is wide but not hollow.
2. A high-pass at about 24 Hz (no rumble a speaker cannot play) and a low-pass at about 420 Hz.
3. **Soft resonances tuned to the score's key root and fifth at the chosen tuning** (in the sketch:
   D1, D2, A2, D3, A3, D4 at A = 432 Hz, that is 36.0, 72.1, 108.0, 144.2, 216.0 and 288.3 Hz),
   narrow band-passes added to the noise so it reads as engines and the structure ringing, not
   plain noise. The higher ones matter: a television or laptop speaker cannot play 36 to 72 Hz, so
   the hum must carry something between 100 and 300 Hz to be heard on them at all.
4. A faint tonal line at the low root, two oscillators 0.07 Hz apart, so it beats slowly like two
   engines slightly out of step.
5. **Breathing**: the level and the resonances move slowly with two long cycles (27 and 41 seconds
   in the sketch), never in step, so it is alive but never pulsing.
6. When the score changes key, the resonances glide to the new root over about 8 seconds. With the
   music off, the hum keeps the last key (D at start).

**Where it plays.**

- **Aboard the mothership only** (`aboard()` and the ship's box). Off the ship, on a planet's open
  ground or outside the hull, it fades out over 3 seconds; those places get their own beds later
  (wind on a planet, a suit's fans and breathing outside the hull), as data in
  `data/audio/beds/*.ron`, chosen by place.
- **Louder and lower near the engines**: hum sources are data (`data/audio/hum_sources.ron`: kind,
  position in ship metres, strength, colour), so any number can exist (a drive, a reactor, life
  support fans). Each source's contribution falls with distance (about 6 dB per doubling past 4 m,
  plus about 6 dB per room boundary crossed). Near a strong source the low-pass drops (420 Hz toward
  250 Hz) and the lowest resonances rise. **The ship has no engine space yet** (section 2), so the
  first rung places one drive source at the ship's stern in that file; when an engine room is built
  it moves there.
- **Softer in small rooms**: level from the room's volume (its `RoomBounds`), from about -4 dB in a
  cabin-sized room (around 25 m3) to 0 dB in the Commons; large halls send a little of the hum to the
  reverb.
- **Smooth between spaces**: every target (level, cutoff, resonance gains) is approached with a 2 to
  4 second time constant, so walking through a door is a crossfade, never a step.

**What it does not claim.** Brown noise has popular claims for focus and sleep; the finding (Part
E4) found no studies of brown noise for attention, very low quality evidence for continuous noise
as a sleep aid, and no difference in arousal between white, pink and brown noise. The hum is there
because a ship should sound like a ship. It is quiet by default (about 18 LU under the music in the
sketch), because Riedy and colleagues note continuous noise "may also negatively affect sleep and
hearing".

**Cost.** In the sketch's JavaScript, 7.3 ms of one core per second of audio (0.73%), six resonators
per channel included; in Rust, a budget of 0.02 ms per 128-frame block (section 14).

## 12. Menus, the world, and the app's other pages

- **The game's menus** (the title and Play screens, the escape menu, the character showroom) play
  the `menu` mood.
- **Entering the world**: the menu mood flows into the place's mood over a phrase, in the same key;
  no stop and start.
- **The escape menu in game**: the score keeps going, ducked about 6 dB and low-passed, as if heard
  through a door; leaving the menu opens it back up over a second.
- **The app's everyday pages** (chat, tasks, the Library: real life) play no music by default, with
  a Settings toggle to have it there too (question 2). People use those pages for real work, often
  with their own music playing.
- **Other sound wins**: a video with sound, the Media page player or a voice call duck or pause the
  score (section 8); interface clicks and notifications pass over it untouched.

## 13. Loudness

- Music and ambience are categories in the loudness design (`docs/design/loudness.md` once it
  lands; if it lands under another name, its category table wins). Proposed targets for that table:
  the menu score at **-23 LUFS** integrated (the EBU R 128 programme level, "an average programme
  loudness of -23 LUFS", version 5.0, November 2023), the in-game score about **4 LU lower** so it
  sits under speech and effects, the hum **15 to 20 LU under the music** by default (the sketch uses
  18), every output at a true peak no higher than **-1 dBTP**.
- **The score meters itself**: it is generated, so it runs the BS.1770 meter (the loudness work's
  `humanity_core::audio` meter, ported into the pure crate or shared) on its own output and trims
  its gain toward the target slowly (a time constant of tens of seconds, at most 1 dB per 10 s), so
  there is no pumping. A soft limiter at -1 dBTP is a safety net, never the sound.
- **Calm means a narrow range**: the calm moods aim for a loudness range of 6 LU or less (the sketch
  measures 4.1 LU at 432 and 4.5 LU at 440), and no change of more than 3 LU within a second of
  short-term loudness.
- The person's own sliders sit on top: Master, Music, Ambience, and the Ship hum's level.
- Nothing is cached: the meter and the trim run live; nothing is stored between runs. If any measured
  value were ever reused across frames it would obey the Dev page's "turn off all reuse" switch.

## 14. The budget

| Where | Budget | Basis |
|---|---|---|
| Audio thread, score | average 0.05 ms per 128-frame block (2% of one core); worst block 0.25 ms (10%) so the sound card never runs dry | The sketch, in JavaScript, costs 59 to 92 ms per second of audio (6 to 9% of a core) for 10 to 20 voices and the reverb; Rust is expected to be several times faster, and rung 1 measures it |
| Audio thread, hum | 0.02 ms per block (under 1%) | The sketch's hum: 7.3 ms per second of audio in JavaScript |
| Voices | 64 at most; the calm moods use 12 to 24 | The pool is fixed; the composer drops the quietest voice before allocating |
| Game thread | under 0.05 ms per frame | Building a `Copy` context and writing it to kira's channel |
| Memory | synth only: under 2 MB; with samples: under 64 MB decoded | Curated sample subsets (rung 6) |
| GPU | none | Nothing in the music touches the GPU |
| Network | none, ever | Each player's game composes its own music; nothing is synchronised |
| Disk | synth only: kilobytes of data; with samples: under 30 MB shipped as Ogg Vorbis | The full libraries stay off the repository |

The audio thread is real time: the budget is checked in tests by rendering offline at the same
block size and timing the worst block, and on the operator's machine by the Dev page's live meter.

## 15. Settings, and the words

In Settings > Audio, beside the existing sliders. Keys go into `data/i18n/en.json` through
`node scripts/i18n-add.js` when each rung builds them; the English below is the draft.

| Control | Key (draft) | Default | Words (draft) |
|---|---|---|---|
| Music (toggle) | `settings.audio.music_on` | on | "Music" / hint: "The game's own calm music, composed live as you play. It never repeats." |
| Music Volume (exists) | `settings.audio.music_volume` | 0.5 | hint: "How loud the music is. Scales together with Master Volume." |
| Tuning | `settings.audio.tuning` | A = 432 Hz | "Tuning: A = 432 Hz / A = 440 Hz" / hint: "The pitch every note is tuned to. 432 sounds very slightly lower than the usual 440. A matter of taste: no health effect is claimed." |
| Music on the app's pages | `settings.audio.music_on_pages` | off | hint: "Also play the music on everyday pages like Chat and Tasks, not only in the game." |
| Quiet gaps | `settings.audio.music_gaps` | Sometimes | "Often / Sometimes / Never" / hint: "Moments of silence between pieces of music." |
| Ambience Volume | `settings.audio.ambience_volume` | 0.6 | hint: "Background sounds of places, like the ship's hum. Separate from the music." |
| Ship hum (toggle) | `settings.audio.ship_hum` | on | hint: "The soft sound of the ship's engines and structure while you are aboard. Louder near the engines, quieter in small rooms." |
| Ship hum level | `settings.audio.ship_hum_level` | subtle (0.35) | hint: "How loud the hum is compared with the music." |
| Tones (toggle) | `settings.audio.tones_on` | off | hint: "Steady tones some people like to listen to. Some people find these tones calming; there is no good evidence that they heal. They play when the music is off or quiet." |
| Tone | `settings.audio.tone_hz` | 528 | a list of the named frequencies and a number field / hint: "The tone's pitch in hertz." |
| Tone volume | `settings.audio.tone_volume` | 0.3 | hint: "How loud the tone is." |
| Lower music while people talk | `settings.audio.duck_for_voice` | on | hint: "Make the music quieter while someone speaks in a call or a video plays." |

Every word a person reads about tuning, tones and the hum is in this table; nothing anywhere else in
the app may say more (no "healing", no "432 Hz is natural", no "brown noise helps you focus").

## 16. Dev tooling (permanent)

- **The Dev page's Music panel**: current mood and its weights, key, chord, phrase position, tempo,
  intensity, the context inputs, active voices, the audio thread's average and worst block time,
  the live loudness (momentary, short-term, integrated) against target, the seed; buttons to force a
  mood, an intensity, a stinger, a key; the hum's place inputs and its level.
- **The dev IPC and MCP**: `music_state` (the same as the panel, as JSON) and `music_force` (mood,
  intensity, seed), so a rig can reach any musical state.
- **`just music-render <mood> <seconds> [--a4 432] [--seed n] [--place ...]`**: renders the real
  engine offline to a WAV, with the levels report (BS.1770 loudness, true peak, loudness range, the
  measured tuning), for listening and for review without booting the game.
- **`HUMANITY_MUSIC_SEED`** fixes the composer's seed for rigs and tests.
- **The sketch** (`scripts/music-sketch.js`, section 19) stays as the reference the engine's first
  rung is compared against.

## 17. What we will not build, and why

- **No health claims, anywhere** (evidence, not law): the finding read every trial it could find and
  none supports them.
- **No AI music generation in the app**: the one openly licensed model read (Magenta RealTime) would
  share the GPU with the renderer, and generated audio cannot follow the tuning or the place the way
  the live score does; the services read either forbid commercial use on free tiers (Suno) or could
  not be read (Udio) (finding, Parts B and C). This is a licence and budget reason; if a player asks,
  the Credits page can say so.
- **No Sonatina Symphonic Orchestra or Philharmonia samples inside the app**, because their licences
  forbid shipping them as samples (a licence, the finding A6 and A7). Nobody meets this on screen;
  it is recorded here and in the finding.
- **No binaural beats** (section 10).

## 18. Contributions

- Composers can add moods, motifs, chord graphs and instrument presets as data, and recorded stems
  or sample sets, under **CC0 1.0**, listing every sample library, plugin and AI service used
  (finding, Part D and its judgement calls). CC BY 4.0 is accepted only with a credit line in
  `CREDITS.md`; NonCommercial and share-alike are not.
- A choir recorded by volunteers singing "ah", "oh" and "oo" on a few notes under CC0 would replace
  the synthesised choir's weakest point.
- In-app, the Music panel grows an editor (rung 10) so moods can be shaped without a text editor.

## 19. The direction sketch (2026-11-06)

A 90-second "mothership calm", rendered offline by a node script with no samples, no downloads and
no models, to hear the direction before the engine exists. **It is a sketch, not the engine**: the
real score composes with a chord graph, moods and places; the sketch plays one written progression.

- Script: `scripts/music-sketch.js` (`node scripts/music-sketch.js --outdir <folder>`; about 30
  seconds to render all three files).
- Layers: pads (three detuned saws through a slowly breathing low-pass), a formant choir drifting
  from "ah" to "oh", celesta-like modal bells on a half-second grid (a 60 bpm feel) playing a
  three-note motif with variations, a drone on D with its fifth, an 8-line feedback delay network
  reverb, and the hum.
- Harmony, two bars per chord in D Lydian colour: Dmaj9, Bm9, Gmaj7#11, Asus2, F#m7, Gmaj9, Em9,
  Asus4, Bm11, Gmaj7#11, Dadd9; fade in over 8 s, out over the last 8 s.
- Measured (the script's own BS.1770 meter, which reads a 997 Hz sine at -23 dBFS in both channels
  as -23.00 LUFS, cross-checked with ffmpeg's `ebur128` filter):

| File | Integrated | True peak | Loudness range | Notes |
|---|---|---|---|---|
| mothership calm at A = 432 Hz | -18.0 LUFS | -5.5 dBTP | 4.1 LU | no clipping |
| mothership calm at A = 440 Hz | -18.0 LUFS | -4.8 dBTP | 4.5 LU | same seed, same notes |
| ship hum alone, 30 s, A = 432 Hz | -36.0 LUFS | -20.2 dBTP | 4.0 LU | at its level in the mix: 18 LU under it |

- Tuning, measured in the rendered mixes by a Goertzel scan: D2 72.08 Hz and A2 108.00 Hz at 432;
  73.42 Hz and 110.00 Hz at 440. Every note used differs by -31.77 cents between the two.
- Balance (stems against all the music): pads -1.7 LU, bells -5.9 LU, choir -6.7 LU, drone -11.7 LU.
  The spectrum is warm by design (41.6% of the energy at 60 to 250 Hz, 55.2% at 250 to 2,000 Hz,
  1.3% above 2 kHz); brightness is a data knob in the real engine.

## 20. The rungs

Each rung is built, tested (every new test seen red first, its red evidence in the test's comment)
and shipped before the next. All native-gated except the pure crate.

1. **The score's skeleton plays aboard.** `crates/humanity-music` with the tuning function, the
   `synth` voice, the drone, the FDN reverb and a soft limiter; one mood file
   (`data/music/moods/mothership_calm.ron`) with a written progression like the sketch's;
   `ScoreSound` in `humanity_core::audio`; Settings: Music on/off and Tuning (432 default); the score
   plays in the world aboard. Tests: a rendered A4 measures 432.00 and 440.00 Hz (zero crossings, to
   0.01 Hz; red with a wrong constant); 60 s rendered offline holds true peak at or under -1 dBTP and
   its loudness within 1 LU of the mood's target; no one-second change of short-term loudness over 3
   LU; same seed, same output hash; the worst 128-frame block under 10% of real time in a release
   test; `process()` allocates nothing (a counting allocator around it); the mood file validates in
   `just validate-data`; the relay build stays green.
2. **The hum and the Ambience category.** The bed in the crate; `AmbientBedSound`; an ambience
   volume in `AudioManager` and Settings (Ambience Volume, Ship hum, Ship hum level); `aboard()` and
   the room's size as inputs; `data/audio/hum_sources.ron` with one drive at the stern; smooth
   crossfades. Tests: silent off the ship; quieter in a small room than in the Commons; louder and
   darker near the drive; its resonances measure at the key root and fifth at 432 and at 440 (a
   Goertzel scan); no step over 1 dB per 100 ms when crossing a doorway in a scripted walk; music off
   leaves the hum, hum off leaves the music.
3. **The composer.** Chord graphs with voice leading, motif banks with variation, phrase planning;
   the `choir` and `modal` voices; the four moods; places and time of day choosing moods; crossfades
   at phrase boundaries. Tests: voice leading moves no voice more than a fourth; an hour rendered at
   faster than real time repeats no 2-minute stretch (hashes of chord and motif sequences);
   `places.ron` maps each zone purpose to a mood; a mood change shows no loudness step.
4. **Menus and the world, and ducking.** The menu mood, entering the world, the escape menu's
   muffle, voice and video ducking, the app-pages toggle. Tests for each transition's smoothness and
   for ducking depth and timing.
5. **Loudness and the Dev panel.** The self-meter and slow trim to the category targets; the Dev
   page's Music panel; `music_state` and `music_force` on the IPC and MCP; `just music-render`.
   Tests: each mood within 1 LU of its target over 5 minutes; the panel's numbers equal the
   renderer's.
6. **Sampled instruments (needs the operator's permission to download, section 21).** The
   `sampler` voice; curated CC0 subsets of piano, harp, vibraphone, glass, bells and string sections
   under `data/music/samples/` with their licence files and `source.ron`; retuning by playback rate.
   Tests: a sampled A4 at 432 measures 432 Hz within 0.5 cents; the memory and disk budgets; a lint
   that every sample folder carries its licence text and source. **The library half is built
   (2026-11-06, section 23)**: the downloads, the pipeline, the core set, the packs, Settings >
   Audio > Instruments, the choice per part with synthesis as the fallback, the licence and source
   check, retuning by playback rate (measured at 432.00 and 440.00 Hz through a stub of the engine's
   instrument trait). Left for the engine's crate: its real `sampler` voice playing them in the score.
7. **Intensity, stingers, quiet gaps, and the tones layer.** Activity to intensity, the event
   gestures, the gaps setting, and the opt-in tones with their words. Tests: a stinger lands on a
   beat and never above the score's level; tones are silent unless on, and play only with the music
   off or in a gap.
8. **The soft-metal colour and the soprano.** The `guitar` voice (Karoryfer CC0 hollowbody samples,
   our amplifier and cabinet), drop-tuned sub drones, half-time drums in lift moments, and a formant
   soprano line with legato glides and vibrato. Tests: drums only above an intensity threshold; the
   guitar's level never above the pads'.
9. **The website.** `humanity-music` compiled to WebAssembly in an AudioWorklet; the website's
   Settings gets the same controls, saved in the browser. Tests: the web build renders the same
   seed to the same notes as native (an event-sequence hash).
10. **Making music in the app.** The Music panel becomes an editor for moods and motifs with live
    reload; a contributor guide for CC0 moods, presets, stems and recorded choirs.

## 21. Questions for the operator, each with a recommendation

1. **Real instruments: which route?** Recommendation: **CC0 sample libraries played live by our own
   engine** (finding, judgement calls), not AI and not pre-recorded tracks. That needs these
   downloads, on the development machine only, with only small curated subsets shipped (under 30 MB
   as Ogg). Each needs your yes, per file:
   - FreePats **Upright Piano KW, small SFZ+FLAC set**, 2.9 MiB, from
     https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html (CC0). The first piano, tiny.
   - From the Versilian Community Sample Library, https://github.com/sgossner/VCSL (CC0), these
     folders: Grand Piano, Kawai (438.6 MiB); Concert Harp (73.1 MiB); Vibraphone (94.9 MiB);
     Glockenspiel (16.4 MiB); Tubular Bells 1 (62.5 MiB); Hand Chimes (41.0 MiB); Wine Glasses
     (41.7 MiB). About 768 MiB together.
   - From VSCO 2 Community Edition, https://github.com/sgossner/VSCO-2-CE (CC0): Strings/Violin
     Section (94.8 MiB), Viola Section (136.5 MiB), Cello Section (177.5 MiB). About 409 MiB.
   - Karoryfer **Black And Green Guitars** from https://shop.karoryfer.com/pages/free-samples (CC0;
     size not shown on the page), for rung 8.
2. **Music on the app's everyday pages?** Recommendation: **off by default**, on in the game's
   menus and the world, with a toggle. Those pages are real life, and people often play their own
   music there.
3. **AI-generated music at all?** Recommendation: **no**, not in the app and not offline for now
   (section 17).
4. **A recorded choir from volunteers under CC0?** Recommendation: **yes, later**; the synthesised
   choir carries it until then.
5. **The tones layer: as designed (only with the music off or in gaps), or retune the whole score
   to the chosen tone?** Recommendation: **as designed**; retuning the score to 528 Hz would mean an
   A of about 444 Hz, which works against your 432 choice.
6. **The hum's default level.** Recommendation: **about 18 LU under the music**, as in the sketch;
   listen to the hum file alone and in the mix and say if it should be more present.

## 22. Not done here

- No code, no data files, no Settings change: this is the design and the evidence. (Rung 6's
  instrument library was built the same day afterwards: section 23.)
- `docs/dev/adding-sounds-music.md`'s stale status table is left for the rung that wires music,
  which rewrites its music half anyway.
- The beds for planets and spacewalks are named, not designed.

## 23. The free instrument library (built 2026-11-06)

The operator, shown the four libraries of question 1: "Awesome, let's build up our free library." and
"Let's go all in. Then in the settings we can have a section for filling out the instrument list,
kind of like we do with the star catalog and skybox." Also: "Could we deep drums and steel drums.
Let's just keep focusing of 432 Hz."

**What was downloaded.** The four approved libraries, whole, and on 2026-11-07 three more (a steel
pan, a Hang, big drums), to `E:\HumanityOS\instruments-src\` (never into git), each verified and recorded with its size, SHA-256, source and licence text in
[`docs/reference/instrument-sources.md`](../reference/instrument-sources.md).

**The pipeline** (`just instrument-packs`, `scripts/instrument-packs.js`, recipes in
`scripts/instrument-recipes.json`): per instrument it keeps the samples it needs (velocity layers,
round robins, a key spread), trims silence and long tails, **measures each sample's pitch** and
encodes **Ogg Vorbis** (decoded by kira and symphonia today; bitexact, so a rebuild is byte for byte
the same). **Nothing is retuned on disk**: each zone records the frequency it was recorded at (its
key at the library's measured tuning) and the engine plays it at `note_hz(key, a4) / recorded_hz`,
section 9's rate, so A = 432 by default and 440 on the other setting both come from the same files.
The measurement earned its keep at once: VSCO 2 names its strings an octave low, Karoryfer names its
guitars an octave high, and VCSL's Kawai names the piano's lowest A "A-1"; each is corrected and
recorded in the definition's `tuning_note`. Every library measured within a few cents of A = 440.

**The core set** ships in the app (`data/music/instruments/`, `data/music/samples/<id>/`), and **five
packs** are one archive each, GitHub release assets under `assets-instruments-1` uploaded by the
operator (`docs/ai/release.md`, "Instrument packs"); sizes and checksums are in
`data/music/packs.ron`:

| Set | Size (Ogg Vorbis) | Instruments, and the part each plays | From |
|---|---|---|---|
| **Core**, in the app | 6.3 MiB | small upright piano (piano); concert harp; vibraphone; glockenspiel; tubular bells; wine glasses (glass); violins (high strings); cellos (low strings); steel pan; bass drum (deep drum) | FreePats, VCSL, VSCO 2, jSteelDrum |
| **Keys** | 69.7 MiB | Steinway B grand, Kawai grand, Kawai upright, Yamaha upright (piano) | VCSL, FreePats |
| **Strings** | 33.3 MiB | violin, viola and cello sections; double bass; solo violin; violins and cellos plucked; concert harp | VSCO 2, VCSL |
| **Mallets and bells** | 20.2 MiB | vibraphone, bowed vibraphone, glockenspiel, tubular bells, hand chimes, wine glasses, steel pan, marimba, xylophone | VCSL, jSteelDrum |
| **Guitars** | 20.1 MiB | Gretsch and Hofner hollowbody electrics, three velocity layers, two round robins | Karoryfer |
| **Percussion and deep drums** | 6.2 MiB | big mallet toms of 22 and 18 inches (the deep drum part's first choice), bass drum, concert bass drum, large hand drum, timpani, frame drum, low tom, gong; the Hang (handpan) | Big Rusty Drums, VCSL, VSCO 2, FreePats |

Measured source tunings: most instruments within 5 cents of A = 440; the concert harp about 8
cents flat (A4 = 438.2 Hz); the glockenspiel, xylophone and wine glasses 13 to 16 cents sharp (A4 =
443.4 to 443.9 Hz), the way orchestral mallets are often tuned. A tubular bell's strike note comes
from its upper partials and is not measured (A = 440 assumed). Timpani keys come from each hit's
measured pitch; hits with no clear pitch are left out. The steel pan measured +2.4 cents and the
Hang +5.7: where the measure disagreed with a file name (an octave out when it locked onto a partial,
a neighbouring note ringing along, or the hardest pan hits up to about 50 cents sharp, the way a
struck pan sounds) the key follows the file name, which the hits measured in tune confirm. Sizes and
checksums: `data/music/packs.ron`.

**Settings > Audio > Instruments** (`src/gui/pages/settings/instruments.rs`): the core set (always
installed), then each pack with what it holds, its size, its licence in plain words, installed or
not, Download / Remove with progress, and for each instrument the part it plays and what plays that
part now. Settings has no Music section yet; the music's controls (section 15) live in Settings >
Audio, so the list sits there under its own heading. A download is checked against its SHA-256
before anything is unpacked and swapped in whole (`src/instrument_packs/install.rs`); a damaged one
is refused and deleted.

**Always music.** Each instrument names its `part` (`data/music/parts.ron`) and a `rank`; for each
part the best installed instrument plays, and when none is installed, that part's synthesised voice
(section 6's kinds, a preset named in `parts.ron`) plays instead
(`instrument_packs::library::InstrumentLibrary::choose`). The seam is tested against a stub of the
engine's instrument trait (the engine's crate was not on main yet): with no drum installed the deep
drum part is synthesised; with the fixture pack installed it plays the pack's samples, and a sampled
A4 recorded at 440 measures 432.00 and 440.00 Hz at the two tunings.

**Not a cache.** An installed pack is data the person chose to install: never worked out again from
anything, checked against its published checksum before it lands, and removing it changes what plays.
Decoded samples kept in memory obey the Dev page's "turn off all reuse" switch
(`instrument_packs::samples::SampleBank`).

**Deep drums and steel drums.** Found in
[`findings/2026-11-06-steel-pan-and-deep-drums.md`](../reference/findings/2026-11-06-steel-pan-and-deep-drums.md)
and approved by the operator on 2026-11-07 ("Let's do it."): Jeff Learman's chromatic C tenor steel
pan (version 1, CC0) plays the new `steel_pan` part (a small set in the core, the full one in the
mallets pack); Karoryfer's Big Rusty Drums give the deep drum part its first choice, a bass drum set
up as a 22-inch floor tom and an 18-inch tom, played with mallets (the close microphone and the
overheads mixed, as the library's own programs do); and FreePats' Hang in D minor plays the new
`handpan` part with `exact_keys` (its nine notes only, as the bank asks: a key it does not have plays
nothing). No CC0 recording of a real taiko exists in what was searched.

**The website** plays no music yet (rung 9), so it has no Instruments list; it mirrors this one when
it does.
