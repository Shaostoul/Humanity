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
440). Settings > Music shows the chosen genre's sentence and says plainly which of its parts play a
synthesised stand-in (a genre's `roles_add` and palette parts with no installed recording).

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

**A person's choice** is in Settings > Music: the genre (the list is the files, so a dropped-in file
appears), a second genre to **blend** with and a slider (tempo, swing and the recipe move smoothly;
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

**A new piece every few minutes** (Settings > Music, "A new piece": every 2, 3, 4, 6 or 10 minutes,
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
| Memory | synth only: under 2 MB; with samples: under 64 MB decoded | Curated sample subsets (rung 6) |
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
`music` object (`humanity_music::MusicSettings`). The website plays no music yet (rung 9), so it has
no mirror of this section; it gains one with the WebAssembly score. Quiet gaps, "Lower music while
people talk" and Music on the menus wait for rungs 4 and 7.

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

## The operator on electro swing (2026-11-13, after v0.1558.0)

- **Switching genre is too slow.** "I clicked to enable electro swing but, the transition was really slow, felt like it took a minute or so." A bug to fix: a genre change should be heard within seconds, crossfading at the next bar.
- **Variety, by seed.** "Like we don't want the same electro swing song for hours on end, we definitely want variety to the music. Even different chords and what not. Are we able to have like procedural seeds for music? So it generates a track based on the seed and then we can vary the seed X amount of minutes to mix up the music?" The composer is already seeded (section 4.2), but a genre's fixed progressions and motifs make its hours sound alike. So: a piece is a seed; each new piece draws its key, its chord walk, freshly generated motifs, its form (sections that return) and its lead instrument from that seed within the genre's rules; a new piece every few minutes (a setting, with Never), crossfading at a phrase.
- **The manouche guitar download is approved.** "Let's do it." (the Martin HD-28 steel-string SFZ, CC0 in its own header, github.com/sfzinstruments/Discord-SFZ-GM-Bank, folder "026-Acoustic Guitar (steel)").
- **More instruments.** "Any other instruments we can add?" From the libraries already approved and on disk (no new download): solo violin (the hot-club violin), tenor saxophone, grand and upright pianos, vibraphone, harmonica, marimba, glockenspiel and the orchestral strings and winds VSCO 2 CE holds.
- **Built 2026-11-13:** the switch heard at the next bar (BUG-297: 2 to 62 s before, 1.3 to 3.2 s after); pieces by seed (section 7c); the Martin as electro swing's comping guitar, and a solo violin and a tenor saxophone in the core set beside its upright piano and vibraphone, all four choosable lead voices (`docs/reference/instrument-sources.md` sections 8 and 9).
- **Fill out the libraries, and electronic genres.** "I'd like to find what free libraries we can to really fill out the music potential. We'll even want to consider how we could make music like dubstep." A research pass scans every free library (licences quoted) and what dubstep, house, drum and bass and the like need from our own synthesis (docs/reference/research/2026-11-13-instrument-libraries/, in progress); each download is still asked first.
- **The downloads approved, and the recommendations.** On the research's list of eleven: "Let's go for it and see what kind of procedural music we can make." and "Let's go with your recommendations": credit-only (CC BY) libraries allowed in optional packs, never in the core set; AKWF and Swirly Drums first, then Sneakybass and the FreePats set; deep, sub-led dubstep first, heavier styles later as their own choices; share-alike libraries left out.
- **What makes dubstep sound goopy or wet?** (he asked, 2026-11-13). Answered: resonant filter sweeps and formant (vowel) filters morphing, pitch glide, comb filters and phasers (moving notches), FM index and wavetable sweeps, multiband upward-and-downward compression that brings up the squelch, distortion then filtering, resampling, and in mixing "wet" is the reverb, delay and chorus share; short pitch-dropping resonant blips for drips. These join the electronic rungs (E2: formant, comb and phaser, glide, resonance; E3: multiband compression).
