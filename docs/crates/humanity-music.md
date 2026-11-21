# humanity-music

## What it is for

The game's own music, composed live, and the ship's hum (docs/design/music.md). A composer
improvises within a mood and a genre (chord graphs with voice leading, motifs it varies, phrases it
never repeats within ten minutes, danger moods the game's intensity raises), a pool of synthesised
voices plays it (pads, a formant choir, modal bells and the steel pan, membrane deep drums and taiko,
FM keys, plucked strings, noise, a drum machine's voices, synths whose oscillators may be a band-limited
wavetable of single cycles, a unison supersaw or a modulated pulse, and the sampler seam the instrument
library fills), and a reverb,
the genre's recipe and a limiter finish it; the hum and the optional tones are a second sound. It is
a crate of its own because it is pure (no device, no GPU, no I/O on the hot path: it renders into a
plain buffer, so tests run faster than real time and `just music-render` works offline, and it can
compile to WebAssembly for the website later) and because it is edited often while the music grows,
so an edit to it rebuilds only the GUI and the engine above it. The server build never compiles it.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** all of `crates/humanity-music/`, a crate of its own since the crate split cut it (`scripts/crate-layers.json` gives its `dir`; the rung is in `docs/design/crate-split.md`); the modules its `lib.rs` declares, by the gate each compiles under:
  - no gate (every build): `crates/humanity-music/src/bed.rs`, `crates/humanity-music/src/composer.rs`, `crates/humanity-music/src/drum_machine.rs`, `crates/humanity-music/src/dsp.rs`, `crates/humanity-music/src/favourite.rs`, `crates/humanity-music/src/fx.rs`, `crates/humanity-music/src/instrument.rs`, `crates/humanity-music/src/intensity.rs`, `crates/humanity-music/src/library.rs`, `crates/humanity-music/src/mei.rs`, `crates/humanity-music/src/mood.rs`, `crates/humanity-music/src/osc.rs`, `crates/humanity-music/src/piece.rs`, `crates/humanity-music/src/places.rs`, `crates/humanity-music/src/render.rs`, `crates/humanity-music/src/resolved.rs`, `crates/humanity-music/src/rng.rs`, `crates/humanity-music/src/score.rs`, `crates/humanity-music/src/scores.rs`, `crates/humanity-music/src/settings.rs`, `crates/humanity-music/src/sheet.rs`, `crates/humanity-music/src/stream.rs`, `crates/humanity-music/src/genre.rs`, `crates/humanity-music/src/tune.rs`, `crates/humanity-music/src/tuning.rs`, `crates/humanity-music/src/voice.rs`, `crates/humanity-music/src/wavetable.rs`, `crates/humanity-music/src/written.rs`
  - `feature = "kira"`: `crates/humanity-music/src/decode.rs`, `crates/humanity-music/src/kira_sound.rs`
  - test builds only: `crates/humanity-music/src/tests.rs`, `crates/humanity-music/src/tests_audio.rs`, `crates/humanity-music/src/tests_cost.rs`, `crates/humanity-music/src/tests_piece.rs`, `crates/humanity-music/src/tests_page.rs`, `crates/humanity-music/src/tests_electronic.rs`, `crates/humanity-music/src/tests_e5.rs`, `crates/humanity-music/src/tests_e6.rs`, `crates/humanity-music/src/tests_written.rs`, `crates/humanity-music/src/tests_written2.rs`, `crates/humanity-music/src/tests_scores.rs`
  - `all(test, feature = "kira")`: `crates/humanity-music/src/tests_stream.rs`
- **Size:** 20,250 lines in 43 files, 5,507 of them in 12 test files.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `kira` 7.
- **May use** (the plan): core. **Uses today** (references in non-test code): nothing.
- **May be used by** (the plan): gui, engine. **Used today by:** gui 62, engine 23.
- **Wrong-way references:** 0 in code; 0 in test code. `node scripts/check-crate-layers.js` fails on any that appears.
- **Its lib tests:** 113 tests, and this runs exactly them, checked against every test's name:

```
just test-lib --crate humanity-music
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name, inside `humanity_music::`):

| Item | References | From |
|---|---:|---|
| `MusicLibrary` | 7 | gui 6, engine 1 |
| `scores::WrittenPiece` | 6 | engine 4, gui 2 |
| `MusicSettings` | 5 | gui 4, engine 1 |
| `tune::Tune` | 4 | gui 4 |
| `tuning::parse_note` | 4 | gui 4 |
| `MusicStatus` | 3 | gui 2, engine 1 |
| `Resolved` | 3 | gui 2, engine 1 |
| `BedStatus` | 2 | engine 1, gui 1 |
| `MusicContext` | 2 | engine 1, gui 1 |
| `decode::decode_threads` | 2 | gui 2 |
| `decode::each` | 2 | gui 2 |
| `favourite` | 2 | gui 2 |
| `instrument::SAMPLE_SCALE` | 2 | gui 2 |
| `library::DEFAULT_MOOD` | 2 | gui 2 |
| `scores::ScoreEntry` | 2 | gui 2 |

And 36 more items, 37 references (`node scripts/crate-cards.js --interface humanity-music` lists them all).
<!-- end generated -->

## Rules

- **Every pitch from `tuning::note_hz`, A = 432 Hz by default.** Never a frequency typed into code
  or data except through it (the hum's resonances, the klaxon and the sampler's playback rate
  included); nothing is rendered or tuned specially for 440, which is one number. Source: the
  operator, 2026-11-06 ("Let's just keep focusing of 432 Hz"); [music.md](../design/music.md) section 9.
- **Nothing allocates on the audio thread.** `ScoreEngine::render`, `BedEngine::render` and kira's
  `process` allocate and free nothing (a counting-allocator test, `render_allocates_nothing`, holds
  it); everything is made in the constructors, and a `SampleBank` is kept alive by its owner so no
  recording is freed there. Source: kira 0.12's `Sound` docs; music.md section 4.2.
- **A long recording streams, and the audio thread never waits for it** (`stream.rs`, 2026-11-14):
  a zone with a `stream` holds only its attack; its note claims a ring from the `StreamPool` with a
  compare-and-swap, reads frames the worker published with a release store, and when they will run
  out within a block and a fade it fades (no click), never blocks. Only an offline render
  (`ScoreEngine::set_offline(true)`: the export, a test) may wait for the worker. The streamed read
  is the in-memory read operation for operation, so a streamed note equals the whole-file note to the
  bit (`tests_stream`); keep the two in step when changing either. Source: music.md section 23,
  "Memory".
- **Instruments, moods, genres, places, intensity bands, tones and hum sources are data**
  (`data/music/`, `data/audio/hum_sources.ron`); kinds are code. A new sound, mood or genre is a file.
  Source: [infinite-of-x.md](../design/infinite-of-x.md); CLAUDE.md, Infinite-of-X.
- **The budget is a gate.** The calm score averages at most 0.05 ms per 128-frame block, the hum
  0.02 ms (`the_audio_thread_cost_per_block_is_within_the_budget`); measure a change with
  `just music-render` before and after. Source: music.md section 14; CLAUDE.md, "Compute and
  bandwidth are a hard budget".
- **Loudness by the app's one meter.** Each mood and genre is levelled (its `level_db`) so it renders
  at the music category's target in data/audio/loudness.toml (-18 LUFS), the hum at the ambient
  one (-26), every output under -1 dBTP; re-measure after changing an instrument's level. Source:
  [loudness.md](../design/loudness.md); music.md section 13.
- **No health claim, anywhere.** The only words about tuning, tones and the hum are music.md section
  15's. Source: the dated finding [2026-11-06-music-sources.md](../reference/findings/2026-11-06-music-sources.md), Part E.
- **No caches across runs.** The data files are read each time a sound starts; nothing the engine
  computes is kept between runs. Source: CLAUDE.md, "No caches kept across runs".
- **Never networked.** Each player's game composes its own music. Source: music.md section 14.
- **A favourite is the person's data, saved by ids** (`favourite.rs`): a genre's, a mood's, a role's
  and a lead voice's id, never a list position, so it plays the same after a data file gains an
  entry; written through a temporary file and a rename; an unreadable file moved aside, never written
  over. A change to `SavedTune` or the share code's layout keeps `a_favourite_saved_and_loaded_plays_the_same_piece`
  and `a_share_code_round_trips_and_a_mistyped_one_is_refused` green. Source: [music-page.md](../design/music-page.md) section 7.
- **Written music plays through the score, never a second audio path** (`mei.rs`, `written.rs`,
  `scores.rs`, 2026-11-20): an MEI file is read and timed on the build thread, and
  `ScoreEngine::written` plays the timeline in place of the composer through the same voices, reverb,
  level and limiter, at the person's tuning. The reader skips and counts what it does not know, never
  crashes; the shipped pieces read with nothing skipped and sound at the music target
  (`the_shipped_score_parses_with_nothing_skipped`, `the_shipped_pieces_sound_at_the_music_target`).
  Pieces are data (`data/music/scores/index.ron`). Source: [score-player.md](../design/score-player.md).
- **A part is everything heard as that part** (`tune::part_of`, 2026-11-20): the eight roles, plus a
  genre's drum machine and build rolls (role 255) as the drums and its risers, stingers and swells
  (254) as the texture, for the mixer's gains, Mute and Solo, the meters (`VoicePool::set_metering`
  sums each voice's dry samples by part, only while the page is open) and the MIDI and MEI exports. A
  new note role must say which part it is heard in. Source: music-page.md section 8 (soloing the bass
  left the drum machine playing until rung 2).
- **MIDI through midly, MEI through our own writer** (`smf.rs`, `mei_write.rs`, `own.rs`, 2026-11-20):
  midly 0.5.3 (the Unlicense, the operator's yes of 2026-11-20) without its `parallel` feature, so
  rayon stays out; the server build never compiles this crate, so midly never reaches it. A piece in
  ticks (`Notated`, 960 a quarter) is the one shape both formats go through; what the importer keeps is
  read back by our own MEI reader first, and a written piece's notes survive export and import exactly
  (`export_then_import_round_trips_a_written_pieces_notes`). Source: music-page.md sections 12 and 15a.
- **The sheet is the score's own loop** (`sheet.rs`): `transcribe` cuts time where `ScoreEngine::render`
  does (128-frame blocks, chunks of at most 64 samples), so its notes are the score's to the sample;
  a change to the score's chunking changes both. Source: music-page.md section 9.

## How to test it

- **Its lib tests:** the `just test-lib --crate humanity-music` line above. `src/tests.rs` holds the
  ones that render nothing (the pitch table, the shipped data, places, an hour of the composer for
  repeats and voice leading, the genre layer, every genre's words and roles, danger rising and settling
  in every genre, the intensity bands); `src/tests_audio.rs` the ones that
  render and measure (electro swing's 128 bpm and swing found in the rendered sound, tuning in the audio, the steel pan's and deep drum's partials and decay, the
  hum and its alarm, loudness and true peak with humanity_core's BS.1770 meter, determinism, the
  cost per block, no allocation, the sampler seam, the duck, a sustain held past its recording by a
  crossfaded loop); `src/tests_piece.rs` the pieces (music.md section 7c: two seeds make two pieces
  and one seed the same piece twice, an hour of pieces repeats no two-minute stretch in any genre,
  every piece stays inside its genre's rules); `src/tests_electronic.rs` and `src/tests_e5.rs` the
  electronic rungs E1, E2, E4 and E5 (sections 7d to 7f); `src/tests_e6.rs` rungs E3 and E6 (section
  7g: the delay's repeats at the tempo's division, the gate cutting the room's tail, the reverb's low
  cut, the downlifter falling and the reversed cymbal swelling, the tape stop's pitch falling, fills at
  phrase ends, each new genre's tempo, drum signature and structure found in its rendered sound, its
  loudness and its cost); `src/tests_stream.rs` streaming (music.md section 23:
  the packet reader decodes what kira decodes, a streamed note equals the whole-file note to the bit,
  a stalled stream or a note with no ring fades without a click, and the audio thread's cost with
  every voice streaming); `src/tests_written.rs` the score player (score-player.md: the MEI reader on
  small snippets one element at a time, the timeline's seconds, a note landing on its sample in the
  rendered sound, the tuning counted in the sound, the loudness, no allocation, the shipped piece read
  with nothing skipped); `src/tests_written2.rs` the score player's rung 2 (repeats and endings, grace
  notes, octave lines, fermatas, hairpins and accents, articulations, measured tremolos, the piano
  roll's sheet of a written piece) and `src/tests_scores.rs` the three shipped pieces (their bars,
  their length at the written tempo and their openings, worked out by hand). The crate is optimised in dev and test builds (the
  root Cargo.toml), so its timing tests measure the game's speed.
- **What starting the score costs, optimised:** the ignored `kira_sound::device_tests::
  decode_cost_of_starting_the_score` (`cargo test --release -p humanity-music --lib decode_cost --
  --ignored --nocapture`): the data read and the core set decoded as the engine's build does, and
  electro swing's recordings decoded one after another and on `decode::decode_files`' threads.
- **The engine's side** (`src/engine/music_tests.rs`): where the music is heard from, the gains and
  ducks, the build on its worker thread, a new score taking over at a phrase after an install, the
  reuse switch, and the ignored `the_score_with_recordings_measured` (each mood with the core set's
  recordings attached: loudness, true peak, cost per block, decoded memory).
- **The shipped data:** `the_shipped_music_data_parses` runs in `just validate-data`.
- **Listen and measure offline:** `just music-render <mood> <genre> <seconds>` (options: `--ramp`
  for an intensity over time, `--hum`, `--breach`, `--blend`, `--stems` for each role's loudness
  against the whole, `--a4 440`); the WAV goes to the temp folder or `--out`, never the repo.
- **On a real sound device:** the ignored tests `kira_sound::device_tests` (run with `cargo test -p
  humanity-music --lib -- --ignored device`): a script's boot of the game opens no audio device, so
  `just verify-runtime` cannot hear the music; these do.
- **In the running game:** the Dev page's Music card (the danger knob and the readout), and the
  `music_intensity` / `music_breach` showcase verbs through the MCP's `game_showcase`.

## Open work

- **The score player's rungs 3 to 8** (the Accord sing-along on `Timeline::syllables`; the simple and
  full readings; a genre's palette; the score view; the website; imports). Rungs 1 and 2 were built
  2026-11-20 (rung 2: repeats and endings, grace notes, octave lines, fermatas, hairpins, articulations,
  measured tremolos, recorded parts, the piano roll, and three public-domain pieces). Source: [score-player.md](../design/score-player.md), section 16.
- **The electronic rung E7** (resampling, granular textures and a frozen reverb for ambient, the
  heavier dubstep kinds brostep and riddim, trance as the supersaw's other home). E1 and E2 (structure,
  the bass toolkit) were built 2026-11-13; E4 and E5 (the drum machine, the wavetable, unison and pulse
  oscillators: `drum_machine.rs`, `wavetable.rs`, `osc.rs`) and E3 and E6 (the mix's tempo delay,
  gated reverb, reverb low cut, downlifter, reversed cymbal and tape stop in `fx.rs`; fills down the
  toms; house, drum and bass, synthwave and ambient as genres) 2026-11-14. Source:
  [music.md](../design/music.md), sections 7d to 7g.
- **Rungs 4 to 10 of the music** (the menu mood on the title screens, voice-call ducking, the self-meter and `music_state` on the IPC,
  sampled instruments, stingers and gaps, the soft-metal guitar and the soprano, the website,
  making music in the app), and the genres' later rungs (dubstep, a danger palette per genre, a
  place leaning a genre; electro swing was built 2026-11-12, a manouche guitar recording waits for a
  CC0 source). Source: [music.md](../design/music.md), sections 7a, 7b and 20.
- **Recorded instruments play by part** (2026-11-06): roles name parts, and
  `src/instrument_packs/score.rs` (the instrument library's module, in the engine) attaches the best
  installed recording of each into this crate's `SampleSet` (16-bit frames, `SAMPLE_SCALE`); since
  2026-11-07 the moods' pads are string sections, their bells a glockenspiel or a vibraphone, their
  drones the double bass, and lo-fi's keys, lead and melody the piano, a guitar and the handpan
  (BUG-266), each falling back to its synthesised voice. A recording with exact keys (the Hang) plays
  its own notes and the rest on its fallback (`SamplerSpec::fallback_index`); a sustained one with no
  loop points of its own is held by a crossfaded loop (`SampleZone::loop_xfade`). The engine builds
  on a worker thread and a new score takes over at a phrase (src/engine/music.rs). With the core
  set's recordings the calm moods measure 1 to 2 LU under the music target (-19.1 to -20.2 LUFS,
  2026-11-07): the self-meter of rung 5 is the planned fix. Source: [music.md](../design/music.md),
  sections 6a, 14 and 23.
- **The danger intensity is a dev knob today.** Game systems (a hull breach, a meteor storm, an
  invasion, combat) are to raise it; none exists yet. Source: music.md section 7b.
- **The danger moods cost more than the calm budget** (action at its peak about 0.13 ms a block,
  measured 2026-11-06); a cheaper taiko (fewer modes, shared strike noise) is the first lever.
  Source: music.md section 14.

## Requests from other crates

This crate's mailbox. A session working in another crate that needs something here (an item made
`pub`, a function moved down, a behaviour changed) adds one line; the session that does the work
deletes it in the same commit, so the commit keeps the record. One line per request, in this form
(`node scripts/crate-cards.js --check` holds it):

`- 2026-11-02, from humanity-gui (the Garden page session): what is needed. Why: the reason.`

None yet.
