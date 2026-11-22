# Music and the ship's hum: a calm score composed live, tuned to A = 432 Hz

**Status (2026-11-06): rungs 1 to 3 BUILT** (the score plays aboard, the hum and the Ambience
category, the composer and the moods, with the deep drums and the steel pan), **the first rung of
GENRES** (section 7a: lo-fi, blends, places that may pick a genre; since 2026-11-12 the genre selector's
electro swing at 128 bpm) and **the first rung of DANGER
AND ACTION** (section 7b: tension and action moods the game's intensity raises, the hum's breach
alarm, the Dev page's knob). The engine is `crates/humanity-music` (its card:
[docs/crates/humanity-music.md](../crates/humanity-music.md)); what was measured is in sections 13,
14 and 19a. Rung 6's **instrument library** was built beside it (section 23): the CC0 core set, the downloadable packs, Settings > Audio > Instruments; the score plays the best recorded instrument installed for each part its roles name, and the part's synthesised voice without one. Rungs 4, 5 and 7 to 10 remain (section 20). A listenable direction sketch came first (section 19).
The whole feature is planned here with all its parts (CLAUDE.md, "Plan the whole feature, build it
in rungs"), then cut into rungs (section 20), smallest first. What can legally be
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
- **A thin adapter, AS BUILT in the crate itself** (`humanity_music::kira_sound`, behind the crate's
  `native` feature, which only the desktop app turns on): `ScoreSoundData` and `BedSoundData`
  implement kira's `SoundData`/`Sound` over the engines, with `ScoreHandle`/`BedHandle` carrying the
  latest-value context in and status out. The design first put it in `humanity_core::audio`; that
  would have made the core (the bottom of the stack, "rarely edited, so every edit above it skips
  it") depend on the music, so every music edit would rebuild the whole workspace. The core gained
  only `AudioManager::play_data` (play any kira sound) and an ambience volume beside music, SFX and UI.
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

### 6a. As built (2026-11-06)

Twenty instruments in `data/music/instruments/`, of eight kinds: `Synth`, `Choir`, `Modal`, and,
beyond the table above, `Membrane` (a struck skin), `Fm` (two-operator FM), `Plucked`
(Karplus-Strong), `Noise` (filtered noise with a tonal body and crackle) and `Sampler` (the seam).

- **The steel pan** (operator, 2026-11-06: "Could we deep drums and steel drums"): modal synthesis.
  A pan's note area is hammered so its fundamental, octave (2:1) and twelfth (3:1) are in tune with
  each other; around them ring weaker inharmonic modes and a second mode a few cents off the
  fundamental (the shimmer, about 1.5 Hz of beating at A4 = 432); a short rubber-mallet strike sits
  on top. Every partial follows `note_hz`, so the pan is tuned to 432 like everything else. In
  "mothership calm" it is a gentle motif colour (the Melody role, density 0.3, about 9 LU under the
  whole).
- **The deep drum** (taiko, frame drum, timpani-like): modal synthesis of an ideal circular membrane
  (its modes at the Bessel-zero ratios 1, 1.594, 2.136, 2.296, 2.653, 2.918, 3.156, 3.501), struck
  near the centre; the pitch starts 6% sharp and falls back in 80 ms (the skin's tension rising as it
  is hit), a felt mallet's 8 ms rise and a low thud. Tuned to the key's root (D2, 72.08 Hz at 432).
  In "mothership calm" it plays slow heartbeat pulses ("lub-dub" once a bar, in about half the
  phrases, about 8 LU under the whole), never busy. `taiko` and `taiko_hit` are harder-struck
  versions for the danger moods. Samples replace them when a CC0 set is approved.
- **The sampler seam, joined to the instrument library** (rung 6; section 23): a mood's role names
  the PART it plays (`part: Some("deep_drum")`, an id of `data/music/parts.ron`) beside its own
  synthesised `instrument`. When a score starts, `src/instrument_packs/score.rs` asks the instrument
  library for the best recording installed for each part the roles name, decodes its zones into the
  engine's `SampleSet` (`SampleZone`: shared stereo frames, their rate, the frequency recorded at the
  library's own tuning, 0 for an unpitched hit, the keys and velocity it covers, a loop), adds it as a
  `Sampler` instrument whose fallback is the role's synthesised voice, levels it to that voice (one
  note's RMS each) so the mood's balance holds, and points the roles at it; with no recording the
  role keeps its synthesised voice, so the music always plays. Retuning is by playback rate (a
  recording made at A = 440 plays 31.77 cents slower at 432: the seam test measures 432.00 Hz through
  the engine's sampler voice). Recorded definitions share `data/music/instruments/` with the
  synthesised ones; each loader leaves the other's alone. Today three of the score's voices are parts:
  the deep drum's heartbeat (the best installed of the drum part, `big_tom_22` first since
  2026-11-07), the steel pan's colour (calm, night watch: the core set's small steel pan) and the
  garden's harp; the rest of the score stays synthesised until a mood names more parts. A recording
  with exact keys (the Hang, `handpan`: only its nine D-minor notes, as its makers ask) plays the keys
  it has and every other key on its synthesised fallback (the steel pan voice), so no note goes
  silent; no genre uses the handpan yet. `parts.ron`'s synthesised voices are instruments of the
  score (a test holds them to it).

## 7. Moods and compositions as data

`data/music/moods/<id>.ron`. A mood is what the music is about here, the frame the composer improvises in:

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

**As built:** the four moods above plus `tension` and `action` (section 7b) are files in
`data/music/moods/`. Chords are written relative to the key centre (a chord's root in semitones
above it), so a mood's keys are transpositions; the chord graph is weighted moves between named
chords, phrases end on a cadence chord, and each phrase is re-planned if it repeats one played in the
mood's `no_repeat_s` (ten minutes by default). Places: the most specific rule wins (a room 4, a zone
2, the hours 1, added), so the residence by night is the night watch and a greenhouse by day the
garden morning. A room is matched by a word in its id or name (`RoomBounds` carries no type key; a
`type_key` there would be cleaner).

## 7a. Genres: tailoring the music to different tastes

The operator, 2026-11-06: "Your procedural idea is actually really cool. I'm curious how we could
tailor it to different tastes. Like what if someone likes dubstep or electroswing or whatever." And
then: "Loving this idea."

**A genre is a layer of data over a mood** (`data/music/genres/<id>.ron`): the mood says where you
are and what the music is about; the genre says how it sounds. Every field is optional, so the
default genre, "Mothership calm", changes nothing. A genre may set:

| Part | What it is | Lo-fi (built) |
|---|---|---|
| Tempo range, a felt beat | over the mood's | 70 to 82 bpm, felt |
| Groove | swing (where the second of each pair of grid steps lands: 0.5 straight, 0.667 triplet), the grid (eighths or sixteenths), syncopation, humanised timing | swung sixteenths at 0.6, 9 ms |
| Drum pattern grammar | each part an instrument and patterns written as text over the grid (`x` a hit, `o` softer, `g` a ghost, `.` a rest); a phrase picks one | kick, brushed snare, hats |
| Pad and bass patterns | the chord restruck on a pattern (comping), the bass on its own (`x` root, `5` fifth, `o` octave, `3` third) | electric piano comps; bass root and fifth |
| Harmony rules | every chord its diatonic seventh, or plain triads, chord lengths | sevenths, 1 to 2 bars |
| Palette | which instrument plays each role; roles turned off or added | FM electric piano, soft lead, round plucked bass, vinyl crackle; no choir, drone or deep drums |
| Sound-design recipe | tape wow and flutter, a low-pass over the mix, saturation, the reverb's share | wow 7 cents, low-pass 5.2 kHz, drier |
| Level | so the genre still renders at the music category's loudness | +4.9 dB |

**Why lo-fi first** (the brief asked for electroswing or a gentle lo-fi before dubstep): the default
world is calm and lo-fi is calm too, and every timbre it needs synthesises well here (an FM electric
piano, a plucked bass, soft drums, crackle, tape wow). Electroswing leaned on recorded swing-era brass
and clarinet, so it came second, once the core set could carry them (below); dubstep's drop, on a
loud, dense mix, waits for a later rung.

**The genre selector (2026-11-12).** The operator: "for the procedural music, let's add a genre
selector. That way we can change how it sounds. I'd like to hear how the math interprets electro
swing music similar to the band Caravan Palance." and "Let's have ours be 128bpm". Settings > Music
already offered "styles" (the ship's own score and lo-fi), so the concept took his word, GENRE,
everywhere (the names rule): `data/music/genres/`, `genre.rs`, the settings' `genre` and
`places_pick_genre`, the words keys. The ship's own score, "Mothership calm", is the first genre and
the default (A = 432 Hz stays the tuning setting it was, beside the genre, for every genre).

**Electro swing (built 2026-11-12, `data/music/genres/electro_swing.ron`)**, the third genre: swing-era
jazz laid over house music, as the genre is generally understood.

| Part | Electro swing |
|---|---|
| Tempo, meter | 128 bpm (the operator's number), 4/4 |
| Groove | swung eighths: the second eighth of each beat at 0.64 of it (straight is 0.5, a triplet 0.667), 4 ms of human timing |
| Drums (house) | a kick four on the floor, a clap on 2 and 4, an open hi-hat on every off-beat, a jazz ride's "ding, ding-a ding" on the closed hat; synthesised, which for house drums is the genuine article |
| Bass | an upright bass that WALKS (`w` steps: the root on a chord's first beat, chord tones and scale steps heading for the next chord, a half step into its root on the beat before, now and then its fifth) or plays a TWO-FEEL (root on 1, fifth on 3); a phrase picks |
| Comping | a gypsy-jazz (manouche) guitar's "la pompe": a chord on every beat, 2 and 4 accented, strummed low to high in 9 ms and choked after 0.3 of a beat |
| Brass | horn STABS pushing the off-beats (the choir role's voice-led upper chord tones struck short on a pattern: the "and" of 2 and 4, the Charleston, an anticipation into the next bar) |
| Melody | CALL AND RESPONSE: the clarinet calls on one chord, the trombone answers on the next with the same rhythm from another chord tone, ending a step lower; riffs in the minor blues scale, short punched notes |
| Harmony (its own progressions) | D harmonic minor (G for a section), so the hum stays in tune: minor sixth chords (i6, iv6), ii-V-i (E half-diminished, A7 or A7 flat nine, D minor six), diminished passing chords (D sharp diminished between i and ii, C sharp diminished leading home), the iv, D7 into it, B flat seven sliding into A7; chords one or two bars; phrases end on i6 or on V7 (a turnaround) |
| Dynamics | each phrase at its own level, 0.75 to 1 of full (a softer breakdown, a louder chorus) |
| Recipe, level | a little vinyl crackle and wow, saturation 0.15, a drier room (reverb x 0.5); +1.6 dB: -18 LUFS within 1 LU over 60 s, with the core set's recordings (-17.8 measured) and with its stand-ins alone |

The genre layer grew these, all data any genre may use: its own `progressions` (key centres and a
chord graph) and `motifs` in place of the mood's in calm moods; `beats_per_bar`; lists of
`pad_patterns`, `bass_patterns` and `stab_patterns` a phrase picks from; `comping` (strum and hold);
`stab_hold_beats`; `call_and_response`; `motif_scale` (two scales joined the modes: the harmonic
minor and the minor blues); a motif's `hold` per note; `dynamics`. A DANGER mood still plays as
written in every genre (its own keys, chords, tempo and drums): the intensity raises tension and
action over electro swing exactly as over the ship's score, and on settling the genre's own harmony
and 128 bpm come back (a test walks every genre through it). Places still choose the mood (its
reverb, its level); in electro swing a mood's own voices and chords give way to the genre's, so the
ship sounds like one band wherever you are. A later rung could let a place lean the band (a quieter
two-feel set at night).

**What is captured, and what is not (copyright).** Only the GENRE's general characteristics: the
tempo, the swing, the house kit, the walking bass, la pompe, minor-key jazz harmony, stabs, call and
response. Never any band's recordings, melodies, lyrics, riffs or one song's distinctive combination
of elements, and never a band's name in the app (the operator's mention stays a quotation in the
data file's comment and here). The dated finding
[`2026-11-12-music-genre-copyright.md`](../reference/findings/2026-11-12-music-genre-copyright.md)
sets out why from the primary sources: copyright protects a composition (words included) and a sound
recording; it leaves ideas and methods, scales, standard chord progressions and a genre's
commonplace building blocks free. Its riffs are short scale fragments the composer varies live, so it
improvises in the style rather than quoting anyone.

**Instruments, and what plays them.** Four new parts (`trumpet`, `trombone`, `clarinet`,
`double_bass_pizzicato`) and the `guitar` part each play the best recording installed and a
synthesised stand-in without one (`swing_horns`: detuned saws with a filter that snaps open;
`clarinet`: odd harmonics from a square wave, a breath filter and vibrato; `upright_bass` and
`jazz_guitar`: plucked strings, dark and bright). Out of the box they are RECORDINGS: the core set
that ships inside the app gained a small trumpet (short notes), an old trombone (short notes), a
clarinet, a pizzicato double bass (VS Chamber Orchestra 2 Community Edition) and the Gretsch
hollowbody (Karoryfer), 1.8 MB together, from libraries the operator already approved and downloaded
whole on 2026-11-06 (`docs/reference/instrument-sources.md`), so no new download was needed. The
pizzicato bass's pitch is taken from its file names at A = 440 (the pitch measure locks onto
sub-harmonics on its low, fast-dying notes; the library's other instruments measure within 5 cents of
440). Settings > Music (the Music page since 2026-11-14) shows the chosen genre's sentence and says
plainly which of its parts play a synthesised stand-in (a genre's `roles_add` and palette parts with no installed recording).

**The genre pass (built 2026-11-13): the new instruments get parts to play.** All data, each with
its synthesised stand-in when its pack is not installed:

| Genre or mood | What a piece may now draw (by weight, from its seed) | From |
|---|---|---|
| Electro swing | its KIT: the house kit, or the BRUSHES (a brushed snare on 2 and 4, the brush stirring on every beat, the ride played with brushes, the hi-hat's foot), the house kick four on the floor under either; its COMPING: the Martin's la pompe or a HONKY-TONK PIANO's chop; its BASS: the upright bass or a TUBA, walking or two to the bar | the "Brushes and jazz drum kits" and "Band and jazz" packs |
| Lo-fi | its KIT: the soft drum machine or a JAZZ KIT played with sticks (kick, snare, closed hi-hat), on the same boom-bap patterns; its COMPING: the piano or an ELECTRIC PIANO (the Wurlitzer, the CP80 or the Pianet) | the "Brushes and jazz drum kits" and the credited "Electric pianos" packs |
| The calm moods (mothership calm, garden morning, night watch, menu) | their PAD: the strings, or the four CHOIR voices singing "ah" as one choir (one piece in three), the choir role resting while it does | the credited "Choir" pack |

What the engine grew for it: a drum line may name a `part` (so a kit's recording plays it) and a
`kit`, and a genre lists its `kits` (a piece draws one; a drum of no kit plays under every kit); a
genre's `leads` already served any role, so comping and bass choices are lead entries (the Music page
lists them beside the call and the answer); a mood's `options` are voices a piece may draw for a role
in place of the mood's own (each with the roles it `rests`); and a part may be several VOICES
(`parts.ron`'s `choir_ah`: the basses, tenors, altos and sopranos), played as one instrument where
every voice is installed, each note from the voice whose range holds it, overlapping ranges split
halfway (`score::voice_bands`). The genre's stand-in line (Settings > Music's then, the Music page's
since 2026-11-14) now lists every part that can sound
with the chosen genre (its moods' too) and names the pack whose recording would play each ("tuba (in
Band and jazz)"). The choir pad's level is set per mood so the same piece sung by the choir or played
by the strings measures within 1 LU (`a_calm_mood_sung_by_the_choir_is_as_loud_as_its_strings`).

**Not yet: a manouche guitar.** The guitar that comps is a hollowbody electric, an honest stand-in for
the steel-string Selmer-Maccaferri acoustic of gypsy jazz. A search on 2026-11-12 found no CC0
multi-sample of that guitar. The CC0 candidates, best first: a Martin HD-28 steel-string flat-top
(Jeff Learman, in sfzinstruments' "Discord-SFZ-GM-Bank", the SFZ header reads "License: Creative
Commons CC0"; plucked notes a minor third apart from E2 to B5, one layer, no mutes or strums; size not
stated); Karoryfer's Shinyguitar archtop (GitHub lists CC0-1.0, an old KVR listing says GPL 3 so the
repository's LICENSE must be checked; 352 MB, 846 samples, acoustic and pickup programs); the Ganjo
banjo-guitar (CC0 legal code; MIDI 38 to 72, up to 11 round robins; a bright layer for the chop);
FreePats' Spanish Classical Guitar (nylon, CC0, 4.5 MiB, its page calls the recording conditions "far
from ideal"). Rejected: the Pianobook MDM guitars (the best mutes, but their EULA forbids
redistribution), the Philharmonia guitar (not as samples), FreePats' FSS steel-string (GPL 3 with an
exception), several sets with no stated licence. Freesound refuses automated reading (its robots.txt),
so its CC0 packs need a person's look. Any of these is a new download, so each waits for the
operator's yes; the engine already builds the chop from plucked notes (strum and hold), so plucked
notes are all a recording needs.

- **Dubstep**: a half-time feel around 140 bpm (snare on beat 3), a wobble bass (a saw stack through
  a low-pass swept by an LFO in time with the beat: the `lfo_per_beat` field already exists in the
  synth's filter), a sine sub bass, and a build-and-drop structure (a phrase plan of build bars with a
  riser, then the drop: a new `structure` field in the genre). Kept calm-friendly as a choice, never a
  default. What dubstep, house, drum and bass, synthwave and ambient need from the engine (the gaps
  and the rungs E1 to E7): section 6 of
  [`2026-11-13-instrument-libraries/README.md`](../reference/research/2026-11-13-instrument-libraries/README.md).

**A person's choice** is on the Music page (since 2026-11-14 the one place that chooses what plays,
[music-page.md](music-page.md) section 4; it was in Settings > Music before): the genre (the list is
the files, so a dropped-in file appears), and while the game chooses, a second genre to **blend** with and a slider (tempo, swing and the recipe move smoothly;
each role takes the second genre's instrument with a chance equal to the blend; both genres' drums
play, each at its share), and **"Let places pick a livelier genre"** (off by default): with it on, a
place in `places.ron`'s `genre_suggestions` (a gym, a workshop) plays its genre while you are there.

**Community genres** are files: anyone can write one and share it, as a mod. A file with no words key
in en.json gives its name in plain words (`name`, `about`).

**What we will not do, and why** (said in Settings too): genres built on RECORDED timbres (swing-era
brass, vocal chops) need samples; the app uses only recordings free to share (CC0), and **never
samples copyrighted recordings without permission, because copyright law forbids it** (a law, not a
platform rule); a genre never copies any artist's recordings, melodies or lyrics (the 2026-11-12
finding above).
Public-domain recordings (old 78s) might be usable, but that needs a dated legal finding first
(CLAUDE.md, "Research a legal question, then write it down and date it"); none has been written,
and nothing here decides it.

## 7c. Pieces: variety by seed (built 2026-11-13)

The operator, 2026-11-13: "we don't want the same electro swing song for hours on end, we
definitely want variety to the music. Even different chords and what not. Are we able to have like
procedural seeds for music? So it generates a track based on the seed and then we can vary the seed
X amount of minutes to mix up the music?"

**A piece is a seed** (`crates/humanity-music/src/piece.rs`). A score plays one piece, and its seed
draws, within the genre's rules (every rule is data in the genre's file):

| What | From the genre's data | Electro swing |
|---|---|---|
| The key | its `keys`, by weight; a bridge may move to another (about a third of pieces) | D, G, A, C, E or F harmonic minor |
| The tempo | its tempo range (the piece keeps one tempo) | 128, its range being 128 to 128 |
| The form | its `forms`, by weight: the order its sections play in, a letter a section | AABA, ABAB, AABB, ABAC |
| Each section's chords | its `grammar` (below), or for a genre with only a chord graph, a walk of that graph with this piece's own leanings (each move's weight times 0.3 to 1.7, fixed by the seed) | shapes of tonic, subdominant and dominant chords plus the style's embellishments |
| Its riffs | its `motif_rules`: how many, notes each, rhythm cells strung together, contours by weight, steps of its scale with a leap now and then, a range, how much of each note sounds | five riffs of 3 to 6 notes in the minor blues |
| Its lead voices | its `leads`, by weight, one per role, never two roles on one voice | the call: clarinet, violin, tenor saxophone, vibraphone or piano; the answer: trombone, violin or trumpet |
| Section lengths | its `phrase_bars` over the mood's | eight bars (the swing era's 32-bar song is four of them) |

**The grammar** (a genre's `progressions.grammar`): each chord's function (tonic, subdominant side,
dominant) and SHAPES, runs of functions such as "T S D T", by weight. A section takes a shape, fills
each slot with a chord of that function (leaning by the seed, avoiding the chord just played), ends
on a tonic (the form's last section) or, as a turnaround, a tonic or a dominant; then, each by its
chance, a secondary dominant (the seventh chord a fifth above the next chord, in the last bar
before it), a tritone substitute (a dominant played as the seventh chord a tritone away) and a
passing diminished chord (a half step under the next chord). The derived chords are made when the
data is resolved (a derived chord that is already one of the named ones, the V7 of the tonic, is
that chord), so the generator only picks indices and allocates nothing on the audio thread. These
are the commonplace building blocks of the style (the 2026-11-12 finding); no tune's changes or
melody is written anywhere.

**Sections that return.** The same letter is the same chords all piece (each section has its own
seed, from the piece's seed and the harmony playing). The first time through the form a letter
also keeps its tune (its riffs drawn from the section's seed), so the second A is the first A
again; after that the band improvises new riffs over the same chords, the second half of each
section reharmonised for that time through (its own seed), re-drawn while a phrase repeats one in
the mood's `no_repeat_s`, so even "Never" plays no two minutes twice in an hour. A danger mood plays
free, as written, and the form picks up where it was when the danger settles.

**A new piece every few minutes** (the Music page's What plays, "A new piece", while the game
chooses; Settings > Music's until 2026-11-14: every 2, 3, 4, 6 or 10 minutes,
or Never; the choices are `data/music/pieces.ron`, 4 minutes by default): the runtime builds the
next piece (the next seed) while the old one plays and starts it at the old one's next phrase, the
two crossfading (each score's 1.5 s fade). A rebuild that is not a new piece (the data read again
with reuse off, a pack installed) carries on the SAME piece from the phrase it was at
(`ScoreEngine::resume`). The first piece's seed is the session's seed (`HUMANITY_MUSIC_SEED` for
rigs); each next piece's is the session's plus the piece's number times the golden ratio's constant.
The Dev page's Music card shows the playing piece's seed, its section and time through the form,
and the time to the next piece.

The ship's own score ("Mothership calm") and lo-fi have forms too, their sections walked from their
chord graphs; only electro swing has a grammar, generated riffs and leads so far.

A piece's draws (its form, riffs and section chords) are keyed by its genre's and mood's ids, never
their places in the list of files, so adding a genre changes no other genre's pieces (BUG-299, fixed
2026-11-13). The ship's own genre plays no ABAB: its long sections made the first time through
repeat over two minutes note for note.

## 7d. The electronic rungs: structure and the bass toolkit (E1 and E2, built 2026-11-13)

The research's section 6 (`docs/reference/research/2026-11-13-instrument-libraries/README.md`) named
what electronic genres need from the engine; the first two rungs are built, all in our own code
(`dsp.rs`, `voice.rs`, `composer.rs`) from published methods, no code copied.

**E1, a genre's STRUCTURE** (`structure` in a genre file, `genre::Structure`): its sections in order,
each a phrase of its own bar count, in place of the genre's forms. A section names the roles that play
in it; a drum line names the sections it plays in (`sections`); a section that returns by name plays
its chords again, as a form's letter does. A BUILD rolls a snare that speeds up across it (eighth
notes for its first half, sixteenths for the next quarter, 32nds for the last, louder as it goes)
under a riser that starts `beats` before the end, and ends in `gap_beats` of silence in which nothing
is struck and everything held lets go; a DROP begins with an impact, the bass in full. Each section
start is an event (`Event::Structure`), so the Music page and the tests can follow it.

**E2, the bass toolkit**, every piece opt-in data on a synth (or an FM voice), so no existing
instrument changed:

| Piece | Field | What it does |
|---|---|---|
| Two-stage filter | `filter.stages: 2` | 24 dB an octave, the wobble's slope |
| Filter drive | `filter.drive` | a resonant filter pushed harder, so it squelches |
| LFO shapes | `filter.lfo_shape` | sine, triangle, ramps up and down, square, sample and hold (a new value each cycle, the same each time a seed plays) |
| Per-bar rates | `filter.lfo_rates` | cycles a beat, one per bar in turn (`[2.0, 3.0]`: eighth notes, then eighth-note triplets); the LFO runs on the music's beat clock, so its rate locks to the tempo |
| LFO restart | `filter.lfo_restart` | the sweep starts afresh at each note instead |
| Glide | `glide_s` | each note slides from the pitch its role last played |
| Waveshaping | `fx.shaper` | soft, hard, folding or asymmetric curves, mixed with the clean sound (parallel distortion), then fewer bits and a lower sample rate |
| Protected sub | `fx.sub` | a clean sine at the note (or an octave under), added after every distortion and filter, the processed sound high-passed above it |
| FM depth | `index_lfo`, `index_lfo_depth` | an FM voice's index moved in time (more FM is buzzier: a growl) |
| Formant (vowel) filter | `fx.formant` | three resonances of the vowels a, e, i, o, u (the bass voice's formants, Csound manual Appendix D, read 2026-11-13) morphing along a list in time with the music: "talking" bass |
| Comb | `fx.comb` | the sound plus itself a moment later, again and again: rings at the note (`key_ratio`) or a set delay, swept by an LFO (the wet, metallic edge) |
| Phaser | `fx.phaser` | four all-pass stages swept between two frequencies, so notches move through the sound |

These answer the operator's question of what makes dubstep "goopy or wet": resonant sweeps, vowels
morphing, glide, comb and phaser movement, FM sweeps, distortion then filtering. With any of the
chain present a voice is processed in mono (a bass sits in the centre). **The cost**: one voice with
every piece at once measures 0.009 to 0.013 ms a 128-frame block on the operator's i7-8700K (the gate
holds it under 0.02); a genre's basses use a few pieces each, and a genre's whole score is held to the
calm budget (section 14) by its own test. Ducking under the kick (`sidechain`, E3 in the research's
order) came with them, for deep dubstep (section 7e): a drum line marked `ducks` tells the score the
moment it hits (the composer's own event, exact, with no detector listening), and the roles the
genre's `sidechain` names dip by its depth and come back over its release, a straight line across
each block so the dip never clicks.

**What remains of the electronic rungs** (the research's section 6): E7's resampling, granular
textures and the heavier dubstep kinds (brostep, riddim) as choices of their own. E4's drum-machine
voices and E5's oscillators (the wavetable, unison and the supersaw, the pulse with its width
modulated) were built on 2026-11-14 (section 7f), and E3's mix effects (the tempo-synced delay, gated
reverb, the reverb's low cut, the downlifter, reversed cymbal and tape stop) with E6's house, drum and
bass, synthwave and ambient genres the same day (section 7g).

## 7e. Deep dubstep (built 2026-11-13)

The operator: "We'll even want to consider how we could make music like dubstep." and, choosing
among its kinds, deep, sub-led dubstep first, the heavier styles later as their own choices.
`data/music/genres/deep_dubstep.ron`, the fourth genre:

| Part | Deep dubstep |
|---|---|
| Tempo, feel | 140 bpm in half-time: the snare (and in the drops a clap) on beat 3, so 140 feels like 70; the kick on 1 with a lazy one later in the bar; sixteenth-note hats |
| Sub | a clean sine at the note under everything (the wobble bass's protected sub), ducking 9 dB under each kick and back over a quarter second |
| Wobble | two detuned saws and a square an octave down through a resonant 24 dB low-pass swept in time, eighth notes, eighth notes, eighth-note triplets, eighth notes, bar by bar; a little drive into the filter, a soft shaper half wet after it, a short glide |
| Growl | two-operator FM, its depth moved by a triangle LFO at a rate changing bar by bar, an asymmetric shaper with fewer bits, a formant filter morphing o, a, e, a, a slow phaser; short riffs low down (two to four notes of the Phrygian mode), in the drops only |
| Structure | a 16-bar intro, an 8-bar build (the snare rolling faster under a riser, a beat of silence), a 16-bar drop (the sub drop, the bass and growls), an 8-bar middle, a second build and drop, an 8-bar outro: 80 bars, 137 s, then from the top |
| Harmony | D (or E, G, A, F) in the Phrygian or Aeolian mode: the minor chord, the flat second leaning on it, the fourth, the flat sixth and seventh; chords two to four bars long |
| Instruments | the core set's LMMS hits (electronic kick, punchy snare, layered clap, closed and open hats, the sweep riser and the sub drop), each with a synthesised stand-in; the wobble bass and the growl synthesised (`wobble_bass.ron`, `fm_growl.ron`), as electronic basses genuinely are; a dub pad (`dub_pad.ron`: the dark pad's saws without its own sub, a 2.5 s release), sparse celesta and crackle |
| Level | levelled so one time through its structure measures -18 LUFS, its drop no more than 2 LU over the whole (E6: heavy never means louder than the category) and its intro no more than 5 LU under the drop (measured with the stand-ins: the intro -20.4, the drop -16.6, the whole -17.8) |
| Cost | its drop 0.035 ms a 128-frame block on average, the calm score 0.037 in the same run (the gate: the calm budget, 0.05) |

Pieces by seed work as in the other genres (section 7c): the structure is the piece's form, each
section name a letter (the second drop plays the first drop's chords), each piece its own key, chords
and growl riffs. Only the genre's commonplace building blocks are written anywhere (the 2026-11-12
copyright finding): no artist's recordings, melodies or riffs, no name of any.

## 7f. The oscillators and the drum machine (E5 and E4, built 2026-11-14)

The operator: "I'm interested in having all the instruments." A single-cycle wave makes no sound
until something reads it over and over, so the core set's 207 AKWF waves waited for a WAVETABLE
oscillator; synthwave, trance, house and the heavier dubstep kinds need the supersaw and the pulse
too; and house and hip-hop need their drum machines. All of it is our own code (`osc.rs`,
`wavetable.rs`, `drum_machine.rs` in `crates/humanity-music/src/`), from published methods, each cited
where it is used; every piece is opt-in data, so no existing instrument changed (the synth's
oscillators moved into `osc.rs` and now fill a buffer the filter then reads; renders of the calm,
deep dubstep and lo-fi scores before and after compared byte for byte identical).

**The wavetable** (`wave: Table`, with the synth's `wavetable`). Waves are named `<set>/<file>` as
their set's `waves.ron` lists them (`akwf_core/AKWF_hvoice/AKWF_hvoice_0001.wav`), from the core set
(`data/music/waves/`) or any installed pack (`data/music/packs/<pack>/waves/`). When the data is read
(every time a score starts; nothing kept across runs), each wave's harmonics come from a discrete
Fourier transform of its one cycle and are written out again as tables: one for every half octave of
harmonic count (1, 2, 3, 4, 6, 8, 11, 16 ... up to the cycle's own 300), each at a power-of-two length
of at least eight samples a cycle of its top harmonic, each wave levelled to a sawtooth's loudness.
Playing, the oscillator picks the table whose top harmonic stays under Nyquist at the note's pitch, so
no harmonic folds back. That is Nigel Redmon's method ("A wavetable oscillator", earlevel.com, Part
2, 2012-05-08: tables an octave apart, each dropping "the upper half of our harmonics"), at half-octave
steps so a note loses at most half an octave of its top harmonics, and without the small fold his
lowest table accepts. The morph walks the list from `position`, swept by an LFO in time with the
music (as E2's: per beat, per-bar rates, shapes, restart) or at a rate in hertz, over `depth`, and
moved by the envelope times `env`. A wave that is missing (a pack not installed, or a data folder
without the waves) plays the instrument's `fallback` (the shipped wavetables name one each: the FM
growl, the soft pad); a missing wave with no fallback is an error naming the instrument and the wave. Measured: the AKWF guitar cycle's first 24 harmonics within 1 dB of the file's own (19 loud
enough to compare); the AKWF saw at C7 (2055 Hz) with every frequency a harmonic above Nyquist would
fold to at least 140 dB under the fundamental, where a polyBLEP saw leaves -29.9 dB; the morph from
the sine cycle to the saw moving the second harmonic from -113 to -6 dB. The two shipped wavetables
hold nine waves, about 425 KiB of tables.

**Unison and the supersaw** (`unison` on any oscillator). Adam Szabo measured the Roland JP-8000's
super saw ("How to Emulate the Super Saw", KTH, Stockholm 2010, read 2026-11-14) and these are his
numbers: seven oscillators at fixed offsets from the centre (table 1, "1 - 0.11002313" to "1 +
0.10745242"), the detune knob through his 11th-degree polynomial (0.003 at 0, 0.098 at 0.5, 0.99 at
1), the centre fading in a straight line and the six sides rising in a parabola as the mix knob turns
("y = -0.55366*x + 0.99785", "y = -0.73764*x^2 + 1.2841*x + 0.044372"), each oscillator "strictly
random" in phase at every note, and a high-pass "at the fundamental harmonic". Fewer copies keep the
centre (an odd count) and the pairs from the outside in; each copy is panned by its offset (`spread`).
A supersaw is `wave: Saw` with seven; a wavetable or a pulse can be unison too. Measured: at full
detune the lowest copy sounds at 0.8900 of the note, where Szabo's numbers put it.

**The pulse** (`wave: Pulse`, its `width`, and the synth's `pwm`). High for `width` of the cycle,
polyBLEP at both edges (Valimaki and Huovilainen, 2007, as the saw and square), never narrower than two
samples, its offset (2w - 1) taken out so the sound stays centred as the width moves; `pwm` sweeps
every pulse's width by `depth` with an LFO in time with the music or a free one. Measured: width 0.2
above zero 20.4% of the time with its second harmonic at -1.8 dB (a 20% pulse's, |sin 2 pi w / 2 sin
pi w|); swept by depth 0.3, the duty ran from 0.22 to 0.80.

**The drum machine** (kind `DrumMachine`, rung E4). One voice of optional parts, so every drum is a
file: up to two TONES (a sine at a ratio of the note, starting `pitch` times sharp and falling over
`pitch_s`, dying over `decay_s`), DRIVE into a soft saturation, a CLICK of low-passed noise, NOISE
through a filter (and a high-pass under it), struck in quick BURSTS for a clap before its tail, METAL
(square waves at inharmonic frequencies through a band-pass and a high-pass), and a CHOKE group (a
closed hi-hat cuts the open one short). The circuits followed are Gordon Reid's "Synth Secrets"
descriptions (Sound On Sound: "Practical Bass Drum Synthesis", February 2002, "Practical Snare Drum
Synthesis", April 2002, "Practical Cymbal Synthesis", July 2002; read 2026-11-14): the 909 kick's pitch
contour with an "instant Attack and slow Decay", its waveshaper and click; the 808 kick's resonance
that rings down by itself and "goes slightly flat at long decays"; the snares' two modes ("the 180Hz
and 330Hz partials") over high-passed noise; the hats' "six square-wave oscillators tuned
enharmonically" through band-passes. The six frequencies here are our own: the paper that gives the
808's (Werner, Abel and Smith, ICMC 2014) is on hosts whose robots.txt bars automated readers
(ccrma.stanford.edu, quod.lib.umich.edu), so it was not read and no value of it is claimed. The claps,
the rimshot and the cowbell are our own designs.

| Voice | What it is | In a genre |
|---|---|---|
| `kick_808` | a deep sine ringing about a second, 6% sharp at the strike | lo-fi's 808 kit |
| `snare_808` | two tones 1.83 apart over noise high-passed at 1.8 kHz | lo-fi's 808 kit |
| `hat_808_closed`, `hat_808_open` | six squares through a band-pass at 10 kHz and a high-pass at 8 kHz, 70 ms and 550 ms, one choke group | lo-fi's 808 kit |
| `clap_808` | band-passed noise struck three times 10 ms apart, then its tail | house's 808 kit; synthwave's choruses, layered on the gated snare (section 7g) |
| `tom_808` | a sine sliding down from 18% sharp; its pitch is the note | fills in house's 808 kit and synthwave (section 7g) |
| `cowbell_808`, `rimshot_808` | two squares through a band-pass; two high tones, a click and noise | house's 808 kit (the rimshot playing the clave); drum and bass's ghost notes on the rimshot (section 7g) |
| `kick_909` | a sine swept down from 3.2 times the note, driven, with a click | electro swing's 909 kit |
| `snare_909` | two tones falling 35% in pitch, over noise low-passed then high-passed into a band | drum and bass's snare; under house's 909 clap now and then; both genres' build rolls (section 7g) |
| `clap_909` | band-passed noise struck four times 8 ms apart, a roomier tail | electro swing's 909 kit |
| `hat_909_closed`, `hat_909_open` | six squares tuned higher, a brighter band and a little noise, one choke group | electro swing's 909 kit |
| `tom_909` | a sine with a deeper sweep than the 808's, driven, a click | fills in house's 909 kit and drum and bass (section 7g) |

Measured (each voice struck once): the 909 kick starts 2.1 times its settled pitch, the 808's 1.06
times; each tom over 1.08; the snares' two tones each over 20 dB clear of the gap between them, with
27 to 36% of their energy above 3 kHz; the claps struck three times in their first 40 ms; the closed
hats 20 dB down within 60 ms, the open ones still within 20 dB at 150 ms, all with 80% of their
energy above 5 kHz; an open hat choked by a closed one 100 ms later more than 15 dB under the open
hat alone.

**Where they play.** A KIT where a genre lists kits: lo-fi's third kit, the 808 (its long boom of a
kick, its snare, its closed hat and now and then its open hat on the same boom-bap patterns), and
electro swing's third, the 909 (its kick four on the floor in place of the house kick, its clap on 2
and 4, its open hat on the off-beats, its closed hat on the ride's pattern; listed first, so the pieces
that drew the brushes before it still do). A drum line can now name several `kits` (electro swing's
house kick plays under the house kit and the brushes, not the 909). Deep dubstep's pieces draw their
growl (the FM growl, or the WAVETABLE GROWL by weight 2 to 1: the AKWF voice, FM and distorted cycles
morphing in time, eighths, triplets, sixteenths, an octave doubled under, a shaper and a phaser) and
their pad (the dub pad, or the PULSE-WIDTH PAD an octave higher: a pulse carries twice a saw's
fundamental, so at the dub pad's A2 it crowded the kick's band). The supersaw and the wavetable pad
found their homes in E6 (section 7g): the supersaw arpeggiates synthwave's chords and is one of
house's two chord voices; the wavetable pad holds synthwave's chords and is ambient's and drum and
bass's pad (trance waits for E7). These are synthesised instruments, not parts of the
instrument library, so no `not_used_yet` mark applies; Settings > Audio > Instruments now says the
core set's single-cycle waves play in the music.

**Levels.** Each new voice that stands beside an old one in a piece is within 2 dB of it (a kit's drums
of the ones they replace, a kick by its body under 150 Hz; the growls held, the pads across their
registers, with their LFOs on the beat clock), so deep dubstep still measures -18 LUFS within 1 LU
whichever growl and pad a piece draws (`every_new_option_is_as_loud_as_what_it_stands_beside`).

**The cost**, one voice each, the mean of a 128-frame block (the best of three runs), measured in the
same run as the calm score (which took 0.068 ms that run against its quiet 0.037: other agents were
building), the gate 0.02 ms scaled by that ratio (`every_new_instrument_fits_the_per_voice_budget`):

| Voice | ms a block |
|---|---|
| `wavetable_growl` (two table oscillators, the shaper, the phaser) | 0.0143 |
| `wavetable_pad` (two table oscillators morphing) | 0.0122 |
| `supersaw` (seven saws, the high-pass) | 0.0081 |
| `kick_808`, `hat_909_open`, `hat_808_open` (the longest rings) | 0.0082, 0.0082, 0.0068 |
| `pwm_pad` (two pulses) | 0.0061 |
| the other drum voices (the rimshot cheapest, the cowbell dearest) | 0.0006 to 0.0039 |

Deep dubstep's drop with them among its draws: 0.0525 ms a block in that run, its gate (0.05) scaled
the same way; its loudness -18.0 and -17.7 LUFS for its two measured seeds.

**To listen**: `pieces_to_listen_to` takes `instrument:<id>` items (a drum in a groove at 120 bpm; any
other voice the chords D minor, B flat, F and C held, then arpeggiated; a growl or a bass a riff low
down), beside a genre's pieces (`deep_dubstep:1:melody=wavetable_growl`, `lofi:1:kit=808`).

## 7g. The mix's effects, fills, and four more genres (E3 and E6, built 2026-11-14)

The operator: "I'm interested in having all the instruments." and "how we could make music like
dubstep". Rung E5 left the supersaw, the wavetable pad, the toms, the cowbell, the rimshot, the 909
snare and the 808 clap in no genre, waiting for these; each now has a home. All our own code
(`fx.rs`, and small additions to `score.rs`, `composer.rs`, `voice.rs` and `drum_machine.rs`), every
piece opt-in data, so no existing genre changed: renders of the calm score, lo-fi, electro swing and
deep dubstep before and after compared byte for byte identical. What each effect is FOR, and its
numbers, come from the production sources the research read on 2026-11-13
([`electronic-genres-and-synthesis.md`](../reference/research/2026-11-13-instrument-libraries/electronic-genres-and-synthesis.md),
sections 2 to 5 and 7), quoted where they are used.

**E3, the mix's effects:**

| Effect | Data | What it does |
|---|---|---|
| Tempo delay | a genre's `delay`: `division` ("1/8." dotted, "1/4", "1/8t" a triplet), `feedback`, `low_cut_hz` and `high_cut_hz`, `ping_pong`, each role's send, `wet`, `reverb` | a recirculating stereo delay line whose time is the note value at the music's tempo (a new tempo glides over 60 ms, as a tape machine's would), its repeats filtered inside the loop (each pass thinner and darker) and softly saturated, so feedback never runs away; ping-pong sends both sides in on the left and crosses each pass to the other side; a share of the repeats goes on into the reverb. Sound On Sound ("Sound Design For Ambient Music", November 2018): "a tape-delay emulation, sync'ed to the project tempo and with the lows rolled off at 600Hz and the highs at 1200Hz" |
| Gated reverb | a genre's `gated_reverb` (`t60_s`, `hold_s`, `release_s`, `wet`) and a drum line's `gated` share | a small dense room (four delay lines and a Hadamard matrix after two all-pass diffusers) with a long decay, so its tail stays level, through a gate the composer opens at each gated hit (its own event: exact, no detector), held open, then closed in a straight line. "Make your own gated reverb effect by heavily compressing a non-linear reverb tail, then placing a noise gate plugin across it to chop off the release" (MusicRadar, 5 August 2020); "a noise gate that cuts the tail of the reverb" (Wikipedia, "Gated reverb", 9 October 2026). The long decay stands in for the compressor: no compressor is built |
| Reverb low cut | a genre's `recipe.reverb_low_cut_hz` | a two-pole high-pass (12 dB an octave) on the reverb's send: "a 6 or 12 dB/octave low-cut filter at 300-400 Hz, to stop the sound getting too muddy" (Sound On Sound, November 2018) |
| A genre's own hall | `recipe.reverb_t60_s` | the reverb's decay over the mood's (ambient's nine seconds) |
| Downlifter | `downlifter.ron`; a structure's `downlifter`, a section's `fall` | noise through a band-pass whose centre falls with its envelope, from about 7 kHz to 250 Hz over three seconds, at the start of a section marked `fall` (out of a drop) |
| Reversed cymbal | a drum machine's `reverse`; `reverse_cymbal.ron`; a structure's `reverse_cymbal` (its `beats`), a section's `swell` | a crash's decay run backwards: it swells from far down to the strike and stops dead (2 ms) exactly on the downbeat of a section marked `swell`, the composer giving it the time to that downbeat |
| Tape stop | a section's `tape_stop_beats` (at most a bar) | over the section's last beats nothing new is struck and the whole score slows to nothing: the score renders on into a short read-back buffer that is read at the falling speed (1 to 0 in a straight line, interpolated), so the pitch, the tempo, the reverb and the delay all fall together, and the level falls with the speed (a playback head's voltage follows the tape's speed); at its end everything the tape held is silenced and the next section strikes afresh. It stands in for a build's silence. "emulating the effect of a tape deck slowing down" (MusicRadar, 30 August 2017) |
| Arpeggio | a genre's `comping.strum_beats` | the struck chord's notes a set part of a beat apart, lowest first, locked to the tempo: a quarter of a beat makes "a simple, ascending four-note arpeggio" (MusicRadar, 5 August 2020) in sixteenths |

Measured (`tests_e6.rs`, each test seen red first): a click through the delay at 120 bpm repeats at
0.375 s for "1/8.", 0.5 s for "1/4" and 0.167 s for "1/8t" (0.5 s for "1/8." at 90 bpm), each repeat
within 2 samples of its time and quieter than the last, and its first repeat through a 600 Hz to
1.2 kHz loop sits 42 dB under the unfiltered repeat over 7 kHz and 29 dB under it below 150 Hz, 2.9
dB under it from 700 Hz to 1.1 kHz; the gated room's tail stays within 3.3 dB of its start while the
gate is open and is silent once it shuts, where the same room left open is only 10.3 dB down at that
time; white noise into the hall with the low cut at 350 Hz leaves a tail 22.4 dB weaker under 120 Hz
and unchanged over 1.4 kHz; the downlifter's spectral centroid falls from 7.2 kHz to 785 Hz in its
first two seconds; the reversed cymbal rises about 12 dB every half second, ends 45.7 dB louder than
it began, and is silent (67.6 dB down) 10 ms after its end; a held 216 Hz note across a two-beat tape
stop at 120 bpm measures 189, 136, 82 and 35 Hz in the stop's four quarters (a straight-line fall of
the speed predicts 189, 135, 81 and 27), its last 20 ms 37 dB under where it began, and the next
section strikes it again at 216 Hz. A delay whose note value is longer than its line holds at the
genre's slowest tempo (1.36 s at 48 kHz) is refused when the data is read, never played short (the
first ambient file's dotted quarter at 64.7 bpm, 1.391 s, was).

**Fills (E6).** A drum line marked `fill` plays only in a phrase's last bar (never a build's, whose
roll is its fill), and from its first hit to the bar's end the other lines rest, except those marked
`under_fills` (house's kick keeps the floor); a line's `notes` list lets its patterns strike several
pitches (`1` to `9`, here high, middle and low toms), and a pattern of rests is a phrase with no
fill (one in three or four, by the piece's draws). The 808 and 909 toms now play fills in house, drum
and bass and synthwave.

**The four genres** (`data/music/genres/`, each a person's choice on the Music page, never a default; only each genre's commonplace building blocks are written, per the 2026-11-12
copyright finding: no artist's recordings, melodies or riffs, and no name of any):

| | House | Drum and bass | Synthwave | Ambient |
|---|---|---|---|---|
| Tempo, feel | 120 to 126 bpm, a light swing (0.54) | 170 to 176, straight | 80 to 118, straight | 60 to 68, no beat felt |
| Drums | a piece draws the 909 kit (kick four on the floor, clap on 2 and 4, open hat on every off-beat, closed hats between, now and then the 909 snare) or the 909 kick under the 808's percussion (its clap and hats, a cowbell now and then, the rimshot playing the three-two clave); fills down the 909 or 808 toms | the two-step (the kick on 1 and the "and" of 3, now and then a second), the 909 snare on 2 and 4 (its second an eighth late or early now and then), the 909 closed hat as a ride on every eighth, ghost notes on the rimshot, fills down the 909 toms | the 909 kick on 1 and 3 (or four on the floor), the 808 snare on 2 and 4 through the gated room, the 808 clap layered on it in the choruses, eighth or sixteenth 808 hats, fills down the 808 toms | none |
| Bass | the pulse bass off the beat, syncopated or jumping octaves, pumping under the kick | the REESE (two saws 55 cents either side of the note, a two-stage low-pass sweeping over two bars, a clean sine sub under it), ducking under the kick | steady sixteenths through the chord's root, or octaves, pumping under the kick | none (the mood's drone) |
| Chords and lead | minor sevenths struck short off the beat on the FM electric piano or the supersaw, pumping; a riff in the minor pentatonic on the soft or the gliding lead | the wavetable pad or the soft pad; sparse electric-piano phrases through a dotted-eighth delay | the SUPERSAW arpeggiating each chord (four notes a sixteenth apart, lowest first); the WAVETABLE PAD or the pulse-width pad holding it; the gliding lead in the choruses through a tape-style dotted-eighth delay | the WAVETABLE PAD or the dark pad over the drone; the celesta or the harp, a few notes far apart, through a quarter-note tape delay band-limited 600 Hz to 1.2 kHz |
| Harmony | Aeolian keys: i7, iv7, v7, bIIImaj7, bVImaj7, bVII, a chord a bar or two | Aeolian: i9, iv9, v7, bIIImaj7, bVImaj7, bVII, two to four bars | Aeolian: i (and i add 9), iv, v, bIII, bVI, bVII (the minor chord to the flat sixth, the flat third, the flat seventh), a bar or two | Ionian: Imaj9, Isus2, ii7, iii7, IVmaj7, Vsus4, vi7, four to eight bars |
| Structure | an intro of drums alone (8 bars), the bass joining (8), the main groove (16), a breakdown (8, a downlifter as it begins), a build (8: the 909 snare rolling under the riser, a beat of silence), the drop (16, a reversed cymbal swelling into it), an outro (8, a downlifter) | an intro (16), a build ending in a two-beat TAPE STOP (8), the drop (16, the sub drop), a breakdown (16, a downlifter), a build into a beat of silence (8), a second drop (16, a reversed cymbal into it), an outro (8) | intro (8), verse (16), chorus (16, a reversed cymbal swelling into each), verse, chorus, bridge (8, a downlifter), chorus, outro (8) | a form of two or three sixteen-bar sections (AB, ABC or AABB) |
| Mix | the chords and the bass duck 6 dB under the kick; a ping-pong dotted-eighth delay on the stabs and the riff; the reverb's lows cut at 250 Hz | the bass ducks 6 dB; the reverb's lows cut at 300 Hz | the gated room on the snare and clap (2.5 s decay, open 0.25 s, closed over 30 ms); a five-second hall with its lows cut at 220 Hz; a little tape wow and a 9 kHz low-pass | a nine-second hall, wetter, its lows cut at 350 Hz; a little tape wow |

**Measured in the sound** (each drum line's own sound taken as the piece less the same piece with
that line struck at a ten-thousandth of its velocity: the composer decides the very same notes, so
the difference is that line in the mix): house's kick 58 times in its first 16 bars on a grid of
0.4977 s (120.55 bpm, the composer's tempo 120.53), every one within 20 ms of a beat, its clap on 2
and 4 every time, its breakdown's low band 53 dB under the main section's; drum and bass's snare 31 times in its first drop on a grid of 0.7037 s (170.53 bpm, the composer's 170.53), every one on the backbeat (2, and 4 or the second snare an eighth either side of it), its kick on 1 and on the "and" of 3 in 14 of 15 bars and never on 2 or 4; synthwave's gated room beginning 31 times, on 2 and 4 every time, on a grid of 1.4391 s (83.39 bpm), from 60 to 180 ms after each hit 4.1 dB under the whole mix (a room you hear) and from 330 ms silent;
ambient no drum onsets, and the delay's own sound following the bells by 0.928 s, its quarter note at
64.7 bpm being 0.927 s. Fills landed in 6 and 8 of the 11 phrases over six minutes of house that may
have one (two seeds), every tom in a phrase's last bar.

**Levels and costs.** Each genre is levelled so one time through its structure (ambient: 150 s)
measures -18 LUFS within 1 LU for two seeds: house +4.5 dB (-18.5 and -18.2 LUFS, its first drop
-17.7 and -17.0), drum and bass +3.0 (-17.8 and -17.8, its first drop -16.0 and -15.9: never more
than 2 LU over the whole, so heavy is not louder), synthwave +3.5 (-17.9 and -17.7 over 299 and 260
s), ambient +4.8 (-17.9 and -18.2); true peaks -3.0 dBTP or lower. Each genre's busiest stretch (house
and drum and bass in their first drop, synthwave in its first chorus, ambient 30 s in), the mean of a
128-frame block, the best of three seeds, measured in one run beside the calm score (0.0448 ms that
run): house 0.0453 ms, drum and bass 0.0431, synthwave 0.0452, ambient 0.0288, every 99th percentile
under 0.094 ms; the gate is section 14's 0.05 ms scaled by how much slower than its quiet 0.037 the
calm score ran (`the_new_genres_fit_the_audio_budget`). The new voices, one each: the reverse cymbal
0.0047 ms, the reese bass 0.0038, the gliding lead 0.0033, the downlifter 0.0013. Synthwave first
measured 0.0914 ms beside a 0.0645 calm score, nearly all of it its held pad: muting each role in turn
(the Music page's mutes, through `Tune::mute`) showed the wavetable pad's 2.5 s release overlapping
the next chord's notes, so the pad's release became 1.2 s (and the supersaw's 0.12 s), synthwave's
held chords two voices two bars long, and the gated room is emptied and skipped while its gate is
shut. Memory: the delay's two lines 512 KiB, made only when some genre has a delay; the gated room 72
KiB; the tape's read-back about 1 KiB.

Pieces by seed work as in the other genres (section 7c): a structure is the piece's form (ambient's
is a form of two or three sixteen-bar sections), and each piece draws its key, tempo, chords, riffs,
kit (house) and voices (the pads, the riff's lead, the bells).

**What remains (E7):** resampling (render a growl and replay it: a buffer that must obey the Dev
page's reuse switch), granular textures and a frozen reverb for ambient, the heavier dubstep kinds
(brostep, riddim) as genres of their own, trance (the supersaw's other home), the reese's own
movement (two filters drifting apart), and a compressor for the gated room should the long decay ever
prove too plain.

## 7h. Seekable pieces and the long arc (built 2026-11-21)

The operator, 2026-11-20: "couldn't we just say, here's how the song will look X minutes out? Then
isn't a 'new song' just a new seed? Would that help us have more complex songs?" The answers given
that day: yes, on the same version of the app and without the adaptive layers; a new song is a new
seed; and yes, being able to compute any moment from the seed is what makes long-range structure
possible. Both halves are built.

**The long arc** (`crates/humanity-music/src/arc.rs`, `data/music/arcs/*.ron`, infinite-of-X). A
genre names an ARC (`arc: Some("song")` in its file); an arc is the stages a piece passes through over
its whole length, planned before a note is played. Each stage says:

| Field | What it is |
|---|---|
| `kind` | Intro, Theme, Develop, Build, Climax, Return or Coda (the page names it; the planner places the climax by it) |
| `share` | its share of the piece's length |
| `energy` | its energy at its start and end (1 = the genre as levelled): velocities follow it (`arc::velocity_of`, 0.4 + 0.6 x energy) and so does how often the riffs, the choir and the heartbeat play (`arc::density_of`, a factor on each role's density) |
| `letters` / `sections` | the sections it plays in turn: letters of a form ("BA"; none: the piece's own form), or for a genre with a structure, section names (each genre plays the names its structure has; none left: the structure between its first and last sections) |
| `roles`, `drums` | the roles that may play in it (instruments entering and leaving over the whole piece) and whether the drums do |
| `key` | `Home` (a bridge may still move, as a form's does), `Away` (the piece's second key) or `Up(n)` (the harmony's key nearest n semitones above home: a lifted climax) |
| `tune` | its sections play their own tunes (the theme stated or returning); otherwise the band improvises over each section's chords, the second half reharmonised |

and the arc's `climax_at` is where the climax stage may begin (a piece's seed picks within it). The
planner (`arc::plan`) scales the shares to the piece's length in bars, places the climax, the stages
before it scaled to end there and the rest to fill the piece, and lays out every phrase: its letter,
its bars (a letter's length drawn once from the plan's seed, so the same letter is as long all piece;
a structure's section as long as the data says), its first bar, its stage, its energy (by the phrase's
middle along the stage), its roles, its key move, and whether it plays its tune (and if the tune was
heard already: a return). A stage gets at least one phrase. The composer's section and bar writers
fill what the plan says. Three arcs ship: `song` (electro swing, lo-fi: an intro on the theme's chords
with the band thinned, the theme stated, a development away from home, a build, the climax two
semitones up at 1.4, the theme's return, a coda), `drift` (the ship's own score and ambient: no
drums added, a gentle swell at 1.2) and `club` (deep dubstep, drum and bass, house and synthwave, by
their sections' names). These are the commonplace shapes of song, jazz and club arrangement (the
2026-11-12 copyright finding); no piece's own arrangement is written anywhere.

**The length** is the piece's: "A new piece" on the Music page (2, 3, 4, 6, 10, and now 20, 30 or 60
minutes, `data/music/pieces.ron`), which reaches the score as `MusicContext::piece_s`; a piece that
plays on (Never, or a chosen piece, a favourite) goes round its arc every `endless_minutes` (8), each
round's improvised sections numbered on from the last so no two rounds reharmonise alike. A context
with no length (offline renders and tests that set none) plays the genre's form over and over: still a
plan (one stage, one time through the form, its tunes the first time), so still seekable. A genre with
no arc does the same.

**The seed scheme** (`composer.rs`, one documented derivation: `piece::mix`, SplitMix64's finaliser
over `a + b x 0x9e3779b97f4a7c15`, every seed a child of the piece's seed S):

| Seed | Is | Draws |
|---|---|---|
| S | the piece (session seed + n x the golden-ratio constant, section 7c) | its key, tempo, lead voices, kit, bridge (`Piece::new`, `mix(S, k)` for fixed k) |
| `mix(S, 0xf0 ^ genre)` | the form | which of the genre's forms |
| `plan_seed(S, genre) = mix(S, 0xa2c0 ^ genre)` | the plan | the climax's place, each letter's length |
| `mix(mix(S, harmony + 0x5ec), letter + 1)` | a section | its chords (the same letter, the same chords), then its tune |
| `mix(section, chorus)` | an improvised time through a section | its second half's reharmonisation |
| `phrase_seed(S, i) = mix(mix(S, 0x9a5e), i)`, then `mix(that, attempt)` | planned phrase i (counted over every round) | its riffs, choir, heartbeat, drum and comping patterns, dynamics (attempt 0, or a later one if it sounds like a phrase of its letter in the mood's no-repeat window) |
| `bar_seed(S, n) = mix(mix(S, 0xba70), n)` | piece bar n | everything decided within the bar: drum syncopation and timing, velocities, pans, the walking bass's choices, the heartbeat's hits |
| `mix(mix(S, 0xda90), bars since the start)` | a bar outside the plan (danger) | the same, not seekable |

Note tags are per bar too (the bar's number shifted 12 bits, the top bit for a bar outside the plan),
so a bar's tags are the same however it was reached (they repeat after 2^19 bars, eleven days at 128
bpm). Three more things made a bar independent of the bars long before it: a beat's start is computed
from its number (`tempo_origin + beats x beat_len`), never summed, so it lands on the same sample at
any distance; a step fires on the first sample at or after its time (it fired up to half a sample
early before, when a block happened to end there, so where blocks fell changed the music); and pads
and choir are VOICED nearest the register's reference voicing (`register_ref`, evenly spaced notes)
rather than led from the last voicing, so a voicing depends on its chord alone (consecutive voicings
still move no voice more than a fourth: the rung 3 test holds), and a phrase's first bass note is
placed from the middle of its register. The no-repeat window (section 7c) is now checked from the plan:
an improvised phrase is drawn again (up to six times) while it sounds like (same key, chords and
fingerprint as any attempt of) an earlier phrase of the same letter within the window, each earlier
phrase recomputed from its seeds, so a seek makes the same choice. Danger moods keep the remembered
history.

**Seeking** (`Composer::seek_to_bar`): plan the piece for the steer as it is, reset to the start of
the phrase before the one bar N is in (its time computed), and compose silently from there to bar N's
first sample. What a phrase plays depends only on the phrase before it, so the next bar played is bar
N exactly as playing from the start would have played it, note for note, sample for sample (the test
compares four genres up to twenty minutes in, and a bar two days in). The work is at most two phrases,
whatever N. The score seeks on the audio thread when the page asks (`MusicContext::seek` now moves a
composed piece too) and strikes again what is held there (the chord, the drone), so the harmony
sounds from the bar; a rebuild of the same piece (`ScoreEngine::resume`) carries on from the bar the
playing phrase began on (the status's `form_pos` is now that bar). Players in one place hearing one
piece need only its seed and when it started.

**What the seed does not decide**: the adaptive layers on top, which steer the composer as they
happen: danger and action (a danger mood plays outside the plan; when it settles, the plan picks up at
the phrase it had reached, so the piece's clock no longer matches the wall clock), a move to another
place's mood (another harmony and its voices over the same plan), a blend or a genre chosen, and the
Music page's Tune (a tempo change re-plans the piece in bars). Same version only: a change to the
composer or the data changes what a seed plays.

**The cost** (section 14, measured 2026-11-21): a bar computed alone in about 80 microseconds at
minute 1, minute 60 or day 3; a 60-minute piece planned in about 9 microseconds.

## 7b. Danger and action: the score rising with the game

The operator, 2026-11-06: "Let's also see what we can generate for high octane situations like a
meteor storm puncturing a bunch of holes through the ship or an alien invasion."

- **An intensity, 0..1, that the game sets** (a hull breach, a meteor storm, an invasion, combat).
  Today it is the Dev page's Music card (a slider and a hull-breach toggle) and the
  `music_intensity` / `music_breach` showcase verbs (a rig drives them through the MCP's
  `game_showcase`); game systems raise the same value when they exist.
- **Bands as data** (`data/music/intensity.ron`): from 0 the place's own mood (calm is wherever you
  are), from 0.35 `tension`, from 0.7 `action`, with a hysteresis of 0.08 so an intensity hovering
  at an edge never flips the band every bar.
- **Transitions never cut.** Rising into a band: on the next beat a STINGER (the big drum and a
  two-note alarm in the bells), a RISER (noise through an opening band-pass) building to the next bar
  line (the one after if the bar is nearly over), and at that bar line the danger mood begins a new
  phrase. Falling back: at the next two-bar boundary, quietly; the long pad envelopes carry the settle.
- **Tension** (D Phrygian, the flat second's dread; the key stays on D, so the hum stays in tune):
  taiko on the beat, a second taiko past 0.5, a pulsing eighth-note bass, the celesta as an alarm (a
  minor third rocking), a dark pad, a low choir; 84 to 100 bpm rising with the intensity.
  **Action** (D Phrygian power chords, the soft-metal colour): the big drum on the downbeats, a taiko
  ostinato, a backbeat, the taiko's sixteenths from 0.85, low brass stabs on a pattern, a galloping
  bass, the bells' alarm, the choir only at the very peak; 118 to 132 bpm. Both are URGENT moods:
  they play as written in every genre (danger sounds like danger), and the genre keeps only its
  recipe and level.
- **The hum strains in a breach** (`hum_sources.ron`'s `alarm`): louder by 5 dB, its low-pass opened,
  its resonances wavering 25 cents, and a distant two-tone klaxon tuned to the key (A4 and D5, 432.0
  and 576.7 Hz at A = 432), heard through the walls (a low-pass); the alarm follows the intensity
  above 0.35 while the breach toggle is on.
- **Loudness**: a danger peak sits louder than calm (tension about -17 LUFS, action about -16.5,
  calm -18) and under the same -1 dBTP ceiling; the limiter catches the drums' transients (under 1%
  of samples in the meteor storm render).
- **Later rungs**: a danger palette per genre (lo-fi's own tension), game systems setting the
  intensity, a "dread" band under tension for slow threats, stingers per event kind.

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

## 12a. As built (2026-11-07): ducking, the world behind a page, the build on its own thread

- **A chat video wins** (BUG-269): while any chat video's sound plays (on the app's screen, docked,
  in its Full window, or on a screen in the world: `ChatVideos::sounding`), the score sits 12 dB
  under itself (`engine::music::VIDEO_DUCK_DB`; a voice gets about 9 dB in section 8, a video carries
  music of its own, so a little more), going down within 1 dB of that in 0.3 s and coming back within
  1 dB in 1.5 s (`MusicContext::duck_db` and the score's own duck, `DUCK_DOWN_S` 0.08 s and
  `DUCK_UP_S` 0.5 s). The hum is not ducked. Not paused at a phrase end (section 8's first plan): the
  score stays faintly there, the way a film's room keeps its sound. Still to come: the Media page's
  player and voice calls.
- **The world behind a page** (section 12's escape menu, BUG-269): the escape menu became the
  navigation of 2026-11-03, where Escape in the world opens the page you came from. So a page opened
  from the world (Escape, or a key such as the inventory's) keeps the world's music and hum playing
  6 dB down (`OVER_WORLD_DB`) until the person moves on to another page or goes back in; any other
  page is an app page (no music unless "on the app's pages" is on). The low-pass "as if through a
  door" is not built.
- **The build runs on a worker thread** (BUG-270): reading the data, choosing and decoding the
  recordings and resolving them took the game thread about half a second at world entry (445 ms to
  decode main's 35 recordings, optimised); now `engine::music::build` runs on its own thread and the
  frame only starts it, polls it and starts the score when it is done.
- **A new genre is heard at the next bar** (BUG-297, 2026-11-13: it took up to a minute): choosing
  another genre builds a new piece for it at once, and it takes over at the playing score's next bar
  line, crossfading; until then the playing score keeps its own genre (no stand-ins), and a build for
  a genre no longer chosen is dropped. The build decodes its recordings on up to four threads
  (`decode::decode_files`), nothing kept past the build.
- **A new score takes over at a phrase**: a build that finishes while a score plays starts at the
  playing score's next phrase boundary and the old one fades out (the score's 1.5 s fade). Builds are
  asked for when what is installed changes (a pack installed or removed, BUG-266), when the genres in
  use change (only their parts' recordings are decoded), and, with the Dev page's "turn off all
  reuse" switch off, once every phrase (BUG-267), so nothing stale survives a phrase.

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

**Measured 2026-11-06** (`just music-render`, humanity_core's BS.1770 meter, 60 s, A = 432, the
tests in crates/humanity-music hold each to its target within 1 LU): mothership calm -18.0 LUFS,
true peak -5.5 dBTP, loudness range 3.3 LU, largest one-second change of short-term loudness 3.2 LU;
lo-fi -18.1; menu, garden morning and night watch levelled to -18; tension -16.9 and action about
-16.5, true peak held at -2.7 dBTP by the limiter. The hum at gain 1 in an open space -26 LUFS (the
ambient target); at its default level (0.35) about 17 LU under the music.

## 14. The budget

| Where | Budget | Basis |
|---|---|---|
| Audio thread, score | average 0.05 ms per 128-frame block (2% of one core); worst block 0.25 ms (10%) so the sound card never runs dry | The sketch, in JavaScript, costs 59 to 92 ms per second of audio (6 to 9% of a core) for 10 to 20 voices and the reverb; Rust is expected to be several times faster, and rung 1 measures it |
| Audio thread, hum | 0.02 ms per block (under 1%) | The sketch's hum: 7.3 ms per second of audio in JavaScript |
| Voices | 64 at most; the calm moods use 12 to 24 | The pool is fixed; the composer drops the quietest voice before allocating |
| Game thread | under 0.05 ms per frame | Building a `Copy` context and writing it to kira's channel |
| Memory | synth only: under 2 MB; with samples: under 64 MB decoded; a long recording holds only its attack (half a second) and its notes stream the rest through 32 rings (4 MiB in all) | Curated sample subsets (rung 6); streaming (section 23, "Memory", 2026-11-14) |
| GPU | none | Nothing in the music touches the GPU |
| Network | none, ever | Each player's game composes its own music; nothing is synchronised |
| Disk | synth only: kilobytes of data; with samples: under 30 MB shipped as Ogg Vorbis | The full libraries stay off the repository |

The audio thread is real time: the budget is checked in tests by rendering offline at the same
block size and timing the worst block, and on the operator's machine by the Dev page's live meter.

**Measured 2026-11-07 with the core set's recordings playing their parts** (BUG-266; the ignored
`engine::music::runtime_tests::the_score_with_recordings_measured`, 60 s, 128-frame blocks): mothership
calm 0.034 ms a block on average (0.054 ms at the 99th percentile), night watch 0.029, garden
morning 0.016, menu 0.031, lo-fi 0.028, action at 0.9 0.116 (0.165): the recordings cost less than
the synthesised voices they replace. **Memory**: recordings are kept as 16-bit frames and only the
parts the genres in use can sound are decoded, 54.7 MiB in the default genre and 30.6 MiB with
lo-fi, under the 64 MB above (all ten core instruments as 32-bit floats would be 182.7 MiB). The
calm moods measure 1 to 2 LU under the music target with them (-19.1 to -20.2 LUFS; they are
levelled at -18 with the synthesised voices): rung 5's self-meter is the fix.

**Measured 2026-11-14 (rungs E3 and E6):** house, drum and bass, synthwave and ambient in their
busiest stretches each within 1% of the calm score's own cost in the same run or under it, their delays, gated
room and tape stop included (section 7g).

**Measured 2026-11-06** (release-optimised, 128-frame blocks at 48 kHz, on the operator's i7-8700K
while other builds ran): the calm score 0.048 ms a block on average (1.8% of the block), 0.1 ms at
the 99th percentile (the gate: `the_audio_thread_cost_per_block_is_within_the_budget`, the best of
five runs); the hum 0.006 ms; menu 0.042, garden morning 0.025, night watch 0.045, lo-fi 0.057,
tension about 0.08 and **action at its peak about 0.13 ms (5%) with more than 40 voices**: over the
calm budget, accepted for the moments of danger (the first lever: a cheaper taiko). Getting the calm
score under 0.05 took the reverb's lines as fixed arrays with its modulation moved once per 16
samples (a third of its cost), the choir's vibrato once a block, its 50-dB-down fifth formant
skipped, and voices ended 60 dB down. The worst single block in an offline loop includes the OS
preempting it, so the gate reads the 99th percentile; kira's own buffering covers a rare slow block.

**Seeking and planning (2026-11-21, section 7h; `the_cost_of_a_bar_does_not_grow_with_its_position`,
the fastest of seven, on the i7-8700K while other agents compiled).** Computing a bar of an electro
swing piece (a fresh composer planning the piece, composing the phrase before silently, then the bar):
78 us at minute 1, 76 us at minute 60, 105 us on day 3, the same within the machine's noise (the gate
holds each to twice minute 1's). Planning a whole 60-minute piece: 8.6 us. Both run on the audio thread
(a seek the page asks for, a new piece's first phrase) inside the worst-block budget above (0.25 ms);
the plan is a fixed array of 512 phrases (about 20 KiB inside the composer), nothing allocated. The
score's own cost per block is unchanged (the calm score's gate holds it).

**How the gates measure on a shared machine (2026-11-15, `crates/humanity-music/src/tests_cost.rs`).**
The genre, deep dubstep, streaming and Music page gates kept failing in full test runs beside other
agents' builds and passing alone. Each scaled its budget by the calm score "in the same run", but
read that calm score once, at another moment than the piece it judged, and a block's time includes
whatever the OS takes from it mid-block. Now the piece and the calm score are rendered from scratch
several times, TAKING TURNS, and each block's time is its fastest across those renders: the engine
is deterministic, so an expensive block stays expensive in every render, while an interruption has
to strike the same block every time to survive. The calm score's own gate and the hum do the same,
and so, from the second pass that day, do the per-voice gates (rungs E5 and E6) and the bass
toolkit's whole chain, whose 0.02 ms had been a fixed figure: it now scales by the calm score as the
others do, never below 1, so a quiet machine still holds it to 0.02 ms. Measured that way beside
other builds: synthwave 0.0443 ms against the calm score's 0.0420, deep
dubstep's drop 0.0330 against 0.0414, 32 streamed notes 9% over 32 held whole. One judgement still
moved beside release builds: the streamed notes' quarter over notes held whole (one round of five
read 26% over, the streamed side more sensitive to builds crowding the processor's shared cache), so
the suite holds it to twice, and `just music-quiet-budgets` judges the quarter when nothing is
building (scripts/music-quiet-budgets.js waits for that, and fails a run a build walked into).

**And through a slowdown that lasts the whole run (2026-11-21, the same file).** The gates still
failed now and then in full runs beside other agents' builds (the calm score at 0.0634 and 0.0505 ms
against its 0.05, the page's analyser, the style budget), each passing alone. The fastest-of-renders
had removed the interruptions; what was left was the core itself running slower. With 12 busy
threads beside the test, the calm score's fastest blocks read 0.075 ms by the wall clock and 0.076
ms by the thread's own CPU cycles, against 0.041 ms alone: the thread ran the whole time, on a core
whose other hyperthread, cache and memory it shared, so neither its own CPU time nor a minimum could
tell that from a slower engine. So every gate now times a YARDSTICK among the subject's own blocks
(fixed arithmetic that shares no code with the engine: ten band-pass filters and four feedback
delay lines, 0.002 ms a unit) and divides the subject's cost by how much slower than quiet the
yardstick ran, never by less than 1, so a quiet machine holds every figure in the table exactly.
Under every load measured the yardstick slowed at least as much as the engine, so a loaded machine
reads the engine at or a little under its quiet cost. The calm-score baseline it replaces is gone
from the other gates, so a slower synth no longer loosens the budgets that scaled by it. Measured
beside other agents' builds (the machine 1.64 to 1.72 times slower than quiet), in quiet-machine
ms: the calm score 0.0391, the hum 0.0056, the page's analyser +0.0026, deep dubstep's drop 0.0295,
synthwave 0.0433, the symphony as written 0.133 in its densest ten seconds; with 12 more busy
threads of multiply-adds or integer work on top, within 10% of that and still passing. The old gate
failed the multiply-adds: "the calm score: 0.0683 ms a block on average (budget 0.0500)". A spin of
0.02 ms planted in the score's render fails the calm score's gate on a quiet machine (0.0612 ms) and
beside the busy threads (0.0524); the reverb run twice more each block (+0.0117 ms, work shaped like
the engine's) fails it quiet and loaded alike (0.0528, 0.0511). The limits: work that load slows less
than the engine is judged at its cost over the slowdown (a plain spin at about two thirds of it), and
beside 12 threads each walking 64 MB, an extreme memory load, the yardstick slowed 2.2 to 2.6 times
and the engine about 1.45, so there the engine reads low and a regression has to be bigger to fail.
A quiet machine judges every slowdown at its full cost. Whether the yardstick's quiet figure is
still right (a new compiler can change it) is `just music-quiet-budgets`' second check.

## 15. Settings, and the words

In Settings > Audio, beside the existing sliders. Keys go into `data/i18n/en.json` through
`node scripts/i18n-add.js` when each rung builds them; the English below is the draft.

| Control | Key (draft) | Default | Words (draft) |
|---|---|---|---|
| Music (toggle) | `settings.audio.music_on` | on | "Music" / hint: "The game's own calm music, composed live as you play. It never repeats." |
| Music Volume (exists) | `settings.audio.music_volume` | 0.5 | hint: "How loud the music is. Scales together with Master Volume." |
| Tuning | `settings.audio.tuning` | A = 432 Hz | "Tuning: A = 432 Hz / A = 440 Hz" / hint: "The pitch every note is tuned to. 432 sounds very slightly lower than the usual 440. A matter of taste: no health effect is claimed." |
| Music on the app's pages | `settings.audio.music_on_pages` | on (2026-11-11) | hint: the music on the app's pages too, from the moment HumanityOS opens and all session, alongside your own music player, each at its own volume in Windows |
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

**As built (2026-11-06):** Settings > Music is its own section (`SettingsCategory::Music`,
`src/gui/pages/settings_music.rs`): Music on, Music on the app's pages, Tuning (A = 432 Hz default),
Genre (the genre selector, renamed from Style on 2026-11-12; under it the chosen genre's one sentence
and, where any of its parts has no recording installed, the line naming the synthesised stand-ins),
Blend with and its slider, Let places pick a livelier genre, the recordings note, Ambience
Volume, Ship hum and its level, Tones (off by default) with its tone and volume. Every word by key
(`settings.music.*` in data/i18n/en.json), every control with its hint. Saved as config.json's
`music` object (`humanity_music::MusicSettings`).

**One place chooses what plays (2026-11-14,** [music-page.md](music-page.md) **section 4).** The
operator: "When I go to the settings it plays one music and when I go to the music page it goes to
another. We should have one place ideally". So the genre, the blend, "Let places pick a livelier
genre", "A new piece", the genre's sentence, its stand-in line and the recordings note moved to the
Music page, and the person's choice (`game_chooses`, `mood`, `seed`, `tune`) joined `genre` in
`MusicSettings`, the one state the score, the Music page and Settings > Music read. Settings > Music
now opens with What plays: what is playing now in the Music page's own words
(`music::now_playing_words`), a Choose the music button that opens the page, and where the volume
is; then Music on, Music on the app's pages, Tuning, the hum and the tones, how the music sounds.
The website plays no music yet (rung 9); its Settings > Audio & Video has a Music group with the
same Choose the music link and says why. Quiet gaps, "Lower music while
people talk" and Music on the menus wait for rungs 4 and 7.

## 16. Dev tooling (permanent)

**The Music page** (2026-11-13) is the person's side of this: explore pieces by genre, mood and
seed, tune them, keep favourites ("my music" for a mood, share codes), save them as WAV, and see the
music (a beat visualizer and a piano roll). Its design and rungs: [music-page.md](music-page.md).

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

**As built (2026-11-06):** the Dev page's Music card (`src/gui/pages/dev_music.rs`) holds the
danger knob (intensity slider, hull-breach toggle, "Restart the music", which reads the files again)
and the readout (mood and the place's mood, key, tempo, phrase and bar, voices, the audio thread's
mean and worst block, the hum's level, cutoff and alarm, the zone, the seed); the showcase verbs
`music_intensity` and `music_breach` (engine/ipc.rs) reach the knob from a rig. `just music-render
<mood> <genre> <seconds>` is built (`--ramp` automates the intensity, `--breach` the alarm,
`--stems` gives each role's loudness against the whole). `music_state` on the IPC and the force
buttons wait for rung 5.
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

## 19a. What the operator can listen to (2026-11-06)

Rendered by `just music-render` from the real engine at A = 432 Hz into the session's scratchpad
(never the repo): 90 s of "mothership calm" with the deep drums and the steel pan, 60 s of lo-fi, and
a meteor storm (calm, the intensity rising to 0.95 with the hull-breach alarm, the peak, and calm
again), each with the ship's hum under it at its default level, faded out over the last 6 s:

| File | Integrated | True peak | Loudness range | Notes |
|---|---|---|---|---|
| mothership calm, 90 s | -18.1 LUFS | -5.0 dBTP | 3.0 LU | the hum 17.0 LU under the music; the drone's D2 found at 72.07 Hz |
| lo-fi, 60 s | -18.1 LUFS | -3.6 dBTP | 2.2 LU | the hum 17.1 LU under |
| meteor storm, 85 s (`--ramp "0:0,18:0,30:0.6,36:0.95,60:0.95,66:0,85:0" --breach`) | -16.5 LUFS | -1.55 dBTP | 5.9 LU | the music alone -16.8 LUFS; the limiter caught the drums' peaks on 0.8% of samples |

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
   (Built: the sampler plays them, and since 2026-11-07 the moods and lo-fi name the parts their
   synthesised voices stood in for, section 23.)
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

**Rungs 1 to 3 as built (2026-11-06):** all three, with these departures: the adapter lives in the
crate (section 4.3); the drone, pads and choir are written per mood rather than a single written
progression (rung 3's chord graph came at once); the Dev page's Music card arrived early (rung 5's
readout, without its force buttons). The tests (each seen red first, the red in its comment) are
listed in docs/crates/humanity-music.md, "How to test it".

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
   music there. **Answered 2026-11-11, ON by default:** "we should have the music start playing when
   HumanityOS starts", and, asked whether it should stop at the first trip into the world, "Play on
   the app pages as well." One switch, "Music on the app's pages", from the app's start and all
   session; its hint says it plays alongside the person's own music player, each at its own volume.
3. **AI-generated music at all?** Recommendation: **no**, not in the app and not offline for now
   (section 17).
4. **A recorded choir from volunteers under CC0?** Recommendation: **yes, later**; the synthesised
   choir carries it until then.
5. **The tones layer: as designed (only with the music off or in gaps), or retune the whole score
   to the chosen tone?** Recommendation: **as designed**; retuning the score to 528 Hz would mean an
   A of about 444 Hz, which works against your 432 choice.
6. **The hum's default level.** Recommendation: **about 18 LU under the music**, as in the sketch;
   listen to the hum file alone and in the mix and say if it should be more present.

Added with the build (2026-11-06):

7. **Which genre next: electroswing or dubstep?** ANSWERED 2026-11-12: the operator asked for electro
   swing at 128 bpm (built, section 7a; its brass, clarinet and bass came from the already-approved
   CC0 libraries, so it needed no new download). The original recommendation: **electroswing**, once a CC0
   brass and clarinet set is approved (its groove, walking bass and four-on-the-floor are ready to
   write as data now; without recordings the brass is a synthesised stand-in). Dubstep after, as an
   opt-in genre, never a place's default.
8. **Public-domain recordings (old 78s) as sample sources?** Recommendation: **not without a dated
   legal finding first** (copyright terms for sound recordings differ from compositions and by
   country); nothing has been decided here.
9. **The danger moods' cost** (action at its peak about 0.13 ms a block, 2.5 times the calm budget,
   for the moments of danger). Recommendation: **accept it for now**, and make the taiko cheaper in
   the next rung if a rig shows frame or audio trouble during a storm.
10. **The hum's place**: today's stern, past p12, per the master plan's section 6.6; it moves to Sector
    1's spine with the wipe. Recommendation: **as placed**; listen at the end of First Street.

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

**What the music plays from it** (2026-11-07, BUG-266): the calm moods' pads are the string sections
(`strings_high` for the soft saw pad, `strings_low` for the dark pad), their bells the glockenspiel or
the vibraphone, their drones the double bass (the strings pack); lo-fi's comping keys are the piano,
its lead a guitar (the guitars pack) and its melody the handpan, low (the percussion pack's Hang,
with the steel pan for its missing notes and without the pack: `fallback_part`). A sustained part's
recordings made without loop points are held by a crossfaded loop and levelled over a chord's
length. Parts no mood or genre plays yet are marked `not_used_yet` in `parts.ron`, and Settings >
Audio > Instruments says so for them; a data test holds the marks to the music. Danger (action)
keeps its brass stabs and taiko: no CC0 recordings of those were found, and rung 8's guitar is the
soft-metal colour's own work.

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

**See also: the next libraries** (2026-11-13). Every other free library we could use, ranked by family
with its licence quoted, what is already on disk and unused, and a download list awaiting the
operator's yes: [`../reference/research/2026-11-13-instrument-libraries/README.md`](../reference/research/2026-11-13-instrument-libraries/README.md);
the licences in [`../reference/findings/2026-11-13-free-instrument-libraries.md`](../reference/findings/2026-11-13-free-instrument-libraries.md).

**The 2026-11-13 batch, downloaded and built.** The operator approved that list ("Let's go for it and
see what kind of procedural music we can make." and "Let's go with your recommendations": credited CC BY
libraries in optional packs only, never in the core set; share-alike left out). All eleven arrived
matching the research (sizes, checksums, licences: [`instrument-sources.md`](../reference/instrument-sources.md),
"The 2026-11-13 batch"). What they became:

| Set | Size | Instruments, and the part each plays | From |
|---|---|---|---|
| **Core**, added | the core set is now 10.5 MiB | button accordion (accordion); ukulele (ukulele); a small electric bass (electric bass); a small jazz guitar (jazz guitar); twelve hits and risers: house kick, punchy snare, layered clap, closed and open hi-hats (the electronic kit's parts), crash and ride (kit crash, kit ride), sweeping riser, down sweep, sub drop, record spin-back, cymbal swell; and 207 AKWF single-cycle waves, kept as plain PCM (0.29 MiB) | FreePats, LMMS, AKWF |
| **Band and jazz** | 37.6 MiB | upright bass, plucked (Sneakybass: the plucked double bass part, so electro swing's walking bass plays it once installed); alto and baritone saxophones; tuba; electric bass (Fashionbass); honky-tonk piano (falls back to the piano part); jazz guitar | Karoryfer, FreePats |
| **Brushes and jazz drum kits** | 3.7 MiB | a jazz kit with sticks (kick, snare, closed and open hi-hat, ride, crash, tom: Virtuosity's middle microphone); a brushed kit (snare, stirs, hi-hat, ride, cajon) | Virtuosity Drums, Swirly Drums |
| **Folk and world** | 5.9 MiB | bagpipe with its drones; steel pan version 2 (five loudness layers; outranks the mallets pack's pan) | FreePats, jSteelDrum 2 |
| **Synth waves** | 9.1 MiB | all 4,358 AKWF single cycles (a wave set, no instruments) | AKWF |
| **Electric pianos** (credited, CC BY 4.0) | 5.4 MiB | Wurlitzer EP200, Yamaha CP80, Hohner Pianet T (electric piano) | Greg Sullivan |
| **Choir** (credited, CC BY 4.0) | 16.6 MiB | sopranos, altos, tenors and basses singing "ah" (one part per voice, ready for four-part writing) | epic-choir, from VocalSet |

Three things the pipeline learned for them: its .7z reader follows a chain of coders (the ukulele is
Deflate then 7-Zip's Delta filter); an SFZ's release-triggered regions are not notes, and takes it
picks at random are numbered round robins; and **wave sets** (`scripts/instrument-recipes.json`
`wavesets`, `src/instrument_packs/waves.rs`): single cycles kept as the library's own PCM bytes,
because a lossy codec smears a 600-sample cycle, for the wavetable rung (E5) to read (built
2026-11-14, section 7f). A pack now says what it adds in a sentence under its name, and a
credited pack names its `data/credits.ron` rows (a test holds each to a row shown on the Credits page
and in the pack's licence line). The 37 new parts are marked `not_used_yet`: which genre plays which
is the next genre pass's choice (the electronic rungs, and electro swing and lo-fi for the brushes, the
honky-tonk piano and the electric pianos), not a change to the synthesis.

**The full packs (2026-11-13).** The operator: "I'm interested in having all the instruments." The
core set stays small and public domain; the packs built from the first libraries now hold them WHOLE:
every instrument and way of playing in VSCO 2, VCSL, Big Rusty Drums and Black And Green Guitars,
the full FreePats upright piano, the Hang, jSteelDrum 1 and the Martin, with every velocity layer and
round robin and up to 30 s of each sample (a recipe with no `keep`). Nothing was downloaded; what was
left out (release samples, which the engine does not play yet; recordings VCSL re-cut from VSCO 2;
effects that are not notes) and each licence, read again from its own file, are in
[`instrument-sources.md`](../reference/instrument-sources.md) section 10. All public domain.

| Pack | Size | Instruments | Samples | What it adds |
|---|---|---|---|---|
| **Keys** (version 2) | 222.7 MiB | 25 | 1,343 | Steinway B grand with and without the sustain pedal, Kawai grand and its earlier recording, the full KW upright, the Knight, Yamaha and No. 1 uprights, five harpsichords (two with a second register), the pipe organ (two registrations and its pedals), the Renaissance organ (three stops), the TX81Z's FM piano, "Piano 1" and Clavisynth |
| **Strings** (version 2) | 67.6 MiB | 22 | 789 | violin, viola and cello sections and the double bass, each held, plucked, spiccato and tremolo (the bass also without vibrato); the solo violin the same; the concert harp whole |
| **Orchestral winds and brass** (new) | 104.8 MiB | 49 | 1,479 | French horn (held, staccato, muted), trumpet (held, with vibrato, staccato, harmon and straight mutes), tenor trombone and the old trombone (held, vibrato, short, buzzed, falls), tuba, flute (four ways), piccolo, oboe, clarinet, bassoon, tenor saxophone and saxello (each held, without vibrato, staccato) and four baroque recorders |
| **Mallets and bells** (version 2) | 39.2 MiB | 15 | 689 | the vibraphone with soft and hard mallets and bowed, xylophone with three mallets, a second set of tubular bells, the tubular glockenspiel, the bell tree's bells, and the steel pan with all 443 of its takes |
| **Guitars** (version 2) | 41.2 MiB | 7 | 1,411 | both hollowbody electrics with every layer and take (430 samples each), staccato, the Gretsch's hammer-ons, the Hofner behind the bridge, and the Martin uncut |
| **Percussion and deep drums** (version 2) | 130.8 MiB | 260 | 2,607 | Big Rusty Drums whole: 89 ways of playing its kick, snare, hi-hat, four toms and six cymbals (sticks, brushes, mallets, rims, chokes), mixed from its close and overhead microphones; and every VCSL drum and small percussion instrument and way of playing it (concert snares, toms and bass drum, timpani hits and rolls, congas, bongos, darbuka, frame drums, cymbals, gongs, triangles, shakers, whistles and the rest) |
| **Folk and world** (version 2) | 42.5 MiB | 39 | 756 | beside the bagpipe and steel pan 2: the Hang, folk harp, strumstick, dan tranh, psaltery (bowed and plucked), kalimbas from Kenya and Tanzania, three Zimbabwean and Mozambican mbiras, balafon, ocarinas, three harmonicas and the didgeridoo |

Each pack instrument that is the whole recording of a core one names it (`full_of`), and the catalog
records what each instrument holds (velocity layers, round robins, samples, seconds); a data test holds
every such pair to more layers, round robins or samples, or, for the Martin and the tenor saxophone
whose 15 and 19 notes the core set already kept, to longer ones. All 350 new parts are marked
`not_used_yet`: a part per way of playing, shared where instruments are alternatives (the harpsichords,
the kalimbas, the mbiras, a kit's kick). Settings folds the list of a set of more than 12 instruments
under a header that counts them (the percussion pack's 260 made the page a scroll through every take).

**Memory: long recordings stream from the disk (built 2026-11-14).** A part's recording was decoded
whole into memory when a score used it (16-bit stereo at 48 kHz, 192 kB a second): the full Steinway
is 2,687 s of samples, about 490 MiB, the Kawai about 440 MiB, the Gretsch about 380 MiB. Now, as
disk-streaming samplers do, a recording longer than 2 s that a note plays straight through (no loop
of its own, a part that does not sustain) keeps only its ATTACK in memory, its first half second, and
a worker thread decodes the rest just ahead of the note into a ring buffer for that note
(`humanity_music::stream`, `data/music/streaming.ron`). The reference is sfizz, the open SFZ player
(BSD-2-Clause; its `src/sfizz/Config.h`, read 2026-11-13): it keeps 8,192 frames of every file
(`preloadSize`) and loads the rest on four background threads (`numBackgroundThreads`), a promise per
voice; ours keeps more of each file (half a second, since one thread serves every stream and Windows
sleeps in 15.6 ms steps) and streams into a fixed ring per note rather than a growing buffer.

- **The rings:** 32 (the budget), each 32,768 frames (0.68 s), 4 MiB together, made once. A note
  claims one with a compare-and-swap and gives it back when it ends; each ring has one writer (the
  worker) and one reader (the note), so it needs no lock and the audio thread neither allocates nor
  waits.
- **Exact:** the worker decodes each file from its start and lets its attack go (a Vorbis packet
  depends on the one before it, so seeking would not give the same frames), so a streamed note is
  the whole-file note to the bit, interpolation across the attack's end included.
- **When the stream cannot keep up** (the disk stalls, or all 32 rings are in use), the note sees
  that its frames will run out within a block and a fade and fades out over the frames it has (20
  ms), so it plays its attack and a release, never a click. Offline renders (the Music page's export,
  the tests) wait for the worker instead (`ScoreEngine::set_offline`).
- **What stays whole:** a looped recording (its loop is read at will) and every recording of a part
  that sustains (the strings' crossfaded loop). The full strings pack's held notes are up to 30 s
  each and stay whole for now: streaming a loop is the next rung.
- **Not a cache:** a ring is refilled from the file for every note, and the attacks obey the reuse
  switch exactly as whole recordings did.
- **Measured** (2026-11-14, the keys pack version 2 installed from the staged archive, lo-fi built as
  the game builds it; `instrument_packs::stream_tests`, ignored, run by hand with
  `HUMANITY_INSTRUMENT_PACKS_OUT`): the full Steinway part held 452.0 MiB decoded whole and holds
  14.6 MiB streaming (its 126 attacks 10.6 MiB, the rings 4.0 MiB), with every one of its zones played
  and up to 23 notes streaming at once; lo-fi's recordings together 475.2 MiB before, 21.5 MiB now; the
  build that decodes them 21.2 s before, 1.5 s now. A 60 s render of lo-fi's calm mood, decoded whole
  and then streamed at real time with the worker never waited for (up to 12 notes streaming), is the
  same to the last bit: 0 of 2,880,000 frames differ. The audio thread: 16 streamed notes 0.025 to
  0.033 ms a 128-frame block, 32 streamed 0.051 ms against 0.044 held whole (`tests_stream`).
- **Shown:** Settings > Audio > Instruments says, for the core set and each installed pack, the
  memory its parts take while they play (the instruments the music would choose, each recording's
  length read from its file's header, nothing decoded) and what it would take held whole; the Dev
  page's music card says the playing score's memory, its recordings and its rings, and how many notes
  stream now.

## The operator on electro swing (2026-11-13, after v0.1558.0)

- **Switching genre is too slow.** "I clicked to enable electro swing but, the transition was really slow, felt like it took a minute or so." A bug to fix: a genre change should be heard within seconds, crossfading at the next bar.
- **Variety, by seed.** "Like we don't want the same electro swing song for hours on end, we definitely want variety to the music. Even different chords and what not. Are we able to have like procedural seeds for music? So it generates a track based on the seed and then we can vary the seed X amount of minutes to mix up the music?" The composer is already seeded (section 4.2), but a genre's fixed progressions and motifs make its hours sound alike. So: a piece is a seed; each new piece draws its key, its chord walk, freshly generated motifs, its form (sections that return) and its lead instrument from that seed within the genre's rules; a new piece every few minutes (a setting, with Never), crossfading at a phrase.
- **The manouche guitar download is approved.** "Let's do it." (the Martin HD-28 steel-string SFZ, CC0 in its own header, github.com/sfzinstruments/Discord-SFZ-GM-Bank, folder "026-Acoustic Guitar (steel)").
- **More instruments.** "Any other instruments we can add?" From the libraries already approved and on disk (no new download): solo violin (the hot-club violin), tenor saxophone, grand and upright pianos, vibraphone, harmonica, marimba, glockenspiel and the orchestral strings and winds VSCO 2 CE holds.
- **Built 2026-11-13:** the switch heard at the next bar (BUG-297: 2 to 62 s before, 1.3 to 3.2 s after); pieces by seed (section 7c); the Martin as electro swing's comping guitar, and a solo violin and a tenor saxophone in the core set beside its upright piano and vibraphone, all four choosable lead voices (`docs/reference/instrument-sources.md` sections 8 and 9).
- **Fill out the libraries, and electronic genres.** "I'd like to find what free libraries we can to really fill out the music potential. We'll even want to consider how we could make music like dubstep." A research pass scans every free library (licences quoted) and what dubstep, house, drum and bass and the like need from our own synthesis (docs/reference/research/2026-11-13-instrument-libraries/, in progress); each download is still asked first.
- **The downloads approved, and the recommendations.** On the research's list of eleven: "Let's go for it and see what kind of procedural music we can make." and "Let's go with your recommendations": credit-only (CC BY) libraries allowed in optional packs, never in the core set; AKWF and Swirly Drums first, then Sneakybass and the FreePats set; deep, sub-led dubstep first, heavier styles later as their own choices; share-alike libraries left out.
- **What makes dubstep sound goopy or wet?** (he asked, 2026-11-13). Answered: resonant filter sweeps and formant (vowel) filters morphing, pitch glide, comb filters and phasers (moving notches), FM index and wavetable sweeps, multiband upward-and-downward compression that brings up the squelch, distortion then filtering, resampling, and in mixing "wet" is the reverb, delay and chorus share; short pitch-dropping resonant blips for drips. These join the electronic rungs (E2: formant, comb and phaser, glide, resonance; E3: multiband compression).
- **Pieces by seed, accepted.** A new piece every 4 minutes by default, returning sections for the ship's own score too, and the 2 s overlap of two keys at a change: "Sounds good." Full-length instruments: "I'm interested in having all the instruments." (optional packs of every layer and round robin of the approved libraries).
- **A Music page.** "We need to add a way in the settings menu or like a dedicated music page to try infinite seeds and variations and genres and what not. That way we or a random person can find the exact seeds we want using the genre we want and all that and set some as permanent. Like maybe seed X has a particular sound I love and want to combat or just to listen to in my real-life car. We'll want to add all the features we can for like visualizing and tuning the music. Like it'd be cool if we could see the song in some sort of music sheet kind of thing and have a visualizer that moves around to the beat." Designed in a music page document (in progress).
- **Not a DAW, for now.** On editing what the engine made (lock, regenerate, edit notes, hum a motif): "I love the idea of eventually making it a full holistic tool but, I'm okay with keeping it simple while we develop more important things. We mostly just need the music for the game right now but, we might find tracks we like. I'm okay with exporting and editing in another app until we're much further along in development and are looking for things to expand beyond their first stage of development." So the Music page stops at explore, tune, keep, export (WAV, then MIDI and stems) and see; editing waits.

## The operator's wishes of 2026-11-16 (not built; each needs its plan)

Verbatim: "For the music, could we somehow play music like Beethoven? I'd love to hear In The Hall of the
Mountain King by Edward Grieg in-game." And: "Also, there's this AEIOU cat for music... Could we say, use
the Humanity Accord as a template to generate that kind of music for Humanity OS? That way we could get
more in-depth music with something that sounds like lyrics but, could just be a technical doc turned into
vowels only."

- **Playing written classical pieces (a score player).** PLANNED AND RUNG 1 BUILT 2026-11-20 in
  [score-player.md](score-player.md): pieces stored as MEI (approved that day: "MEI sounds right unless
  there's better"), read by our own reader, played through this score on our instruments at the
  tuning. What follows is the wish as first written. Today the engine only composes. Playing a fixed
  piece means notes as data (a public-domain edition imported from MusicXML, MIDI or LilyPond into a
  score file), played on our own instruments at our tuning, possibly arranged into a genre's palette. The
  compositions of Grieg (died 1907) and Beethoven (died 1827) are generally public domain, but recordings
  and modern editions can carry their own rights, so per CLAUDE.md a dated finding in
  docs/reference/findings/ comes first (the compositions, which editions and files are free to use, and
  from where), then the plan. Separately, a genre in a composer's STYLE (Romantic harmony, an orchestral
  palette, sonata-like form) needs no score at all and fits the genre system as it is.
- **Vowel singing from the Humanity Accord.** The choir is already formant synthesis gliding between
  "ah" and "oh" (choir_ah.ron); extend it to the five vowels, take the Accord's text, keep each
  syllable's vowel, and sing one per melody note, so it sounds like lyrics in an unknown language while
  being the Accord turned into vowels; the line being sung could scroll on screen like a karaoke line.
  No AI (it is synthesis), and nothing is taken from the meme but the idea of singing vowels. The meme
  he meant (he linked its Know Your Meme page the same day) is the "Oo Ee A E A" (OIIAOIIA) spinning-cat
  sound, a chant of pure vowels.
- **Made the goal for the next usage reset** (the same day): "Okay, then that's our goal next usage
  reset. To some how save orchestral music in a file format everyone can easily modify. I guess they
  could technically be .md files but, there are probably purpose built file formats for this kind of
  thing. If we use a special file format then we should also convert the humanity accord to that format
  so we can play the humanity accord as instrumental music." On the sing-along: "I love that sing along
  idea! That'll make reading and hearing the accord instrumentally a unique experience." The order and
  the steps are in docs/PRIORITIES.md TIER 0. A note for the comparison: a score format that carries
  lyrics per note would let the Accord's own words sit under the notes the choir sings their vowels on.
