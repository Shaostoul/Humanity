# humanity-music

## What it is for

The game's own music, composed live, and the ship's hum (docs/design/music.md). A composer
improvises within a mood and a style (chord graphs with voice leading, motifs it varies, phrases it
never repeats within ten minutes, danger moods the game's intensity raises), a pool of synthesised
voices plays it (pads, a formant choir, modal bells and the steel pan, membrane deep drums and taiko,
FM keys, plucked strings, noise, and the sampler seam the instrument library fills), and a reverb,
the style's recipe and a limiter finish it; the hum and the optional tones are a second sound. It is
a crate of its own because it is pure (no device, no GPU, no I/O on the hot path: it renders into a
plain buffer, so tests run faster than real time and `just music-render` works offline, and it can
compile to WebAssembly for the website later) and because it is edited often while the music grows,
so an edit to it rebuilds only the GUI and the engine above it. The server build never compiles it.

## At a glance (generated)

<!-- generated: crate-cards.js; `just code-map` and `just brief` rewrite this block from scripts/crate-layers.json and the code, so never edit it by hand -->
- **Owns** all of `crates/humanity-music/`, a crate of its own since the crate split cut it (`scripts/crate-layers.json` gives its `dir`; the rung is in `docs/design/crate-split.md`); the modules its `lib.rs` declares, by the gate each compiles under:
  - no gate (every build): `crates/humanity-music/src/bed.rs`, `crates/humanity-music/src/composer.rs`, `crates/humanity-music/src/dsp.rs`, `crates/humanity-music/src/instrument.rs`, `crates/humanity-music/src/intensity.rs`, `crates/humanity-music/src/library.rs`, `crates/humanity-music/src/mood.rs`, `crates/humanity-music/src/places.rs`, `crates/humanity-music/src/render.rs`, `crates/humanity-music/src/resolved.rs`, `crates/humanity-music/src/rng.rs`, `crates/humanity-music/src/score.rs`, `crates/humanity-music/src/settings.rs`, `crates/humanity-music/src/style.rs`, `crates/humanity-music/src/tuning.rs`, `crates/humanity-music/src/voice.rs`
  - `feature = "kira"`: `crates/humanity-music/src/kira_sound.rs`
  - test builds only: `crates/humanity-music/src/tests.rs`, `crates/humanity-music/src/tests_audio.rs`
- **Size:** 6,431 lines in 20 files, 869 of them in 2 test files.
- **Gates inside its files** (`#[cfg]` attributes in non-test code, by the feature they name): `kira` 1.
- **May use** (the plan): core. **Uses today** (references in non-test code): nothing.
- **May be used by** (the plan): gui, engine. **Used today by:** engine 11, gui 7.
- **Wrong-way references:** 0 in code; 0 in test code. `node scripts/check-crate-layers.js` fails on any that appears.
- **Its lib tests:** 18 tests, and this runs exactly them, checked against every test's name:

```
just test-lib --crate humanity-music
```

**Interface: the items other crates reference, most first** (non-test code; a `use a::{x, y}` group counts once, as `a`; an item is a path up to its first capitalised name, inside `humanity_music::`):

| Item | References | From |
|---|---:|---|
| `MusicSettings` | 3 | gui 3 |
| `BedContext` | 1 | engine 1 |
| `BedEngine` | 1 | engine 1 |
| `BedStatus` | 1 | gui 1 |
| `MusicContext` | 1 | engine 1 |
| `MusicLibrary` | 1 | engine 1 |
| `MusicStatus` | 1 | gui 1 |
| `Resolved` | 1 | engine 1 |
| `SampleLibrary` | 1 | engine 1 |
| `ScoreEngine` | 1 | engine 1 |
| `Tuning` | 1 | gui 1 |
| `hum_place` | 1 | engine 1 |
| `kira_sound` | 1 | engine 1 |
| `library::DEFAULT_STYLE` | 1 | gui 1 |
| `places::Place` | 1 | engine 1 |

And 1 more items, 1 references (`node scripts/crate-cards.js --interface humanity-music` lists them all).
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
- **Instruments, moods, styles, places, intensity bands, tones and hum sources are data**
  (`data/music/`, `data/audio/hum_sources.ron`); kinds are code. A new sound, mood or style is a file.
  Source: [infinite-of-x.md](../design/infinite-of-x.md); CLAUDE.md, Infinite-of-X.
- **The budget is a gate.** The calm score averages at most 0.05 ms per 128-frame block, the hum
  0.02 ms (`the_audio_thread_cost_per_block_is_within_the_budget`); measure a change with
  `just music-render` before and after. Source: music.md section 14; CLAUDE.md, "Compute and
  bandwidth are a hard budget".
- **Loudness by the app's one meter.** Each mood and style is levelled (its `level_db`) so it renders
  at the music category's target in data/audio/loudness.toml (-18 LUFS), the hum at the ambient
  one (-26), every output under -1 dBTP; re-measure after changing an instrument's level. Source:
  [loudness.md](../design/loudness.md); music.md section 13.
- **No health claim, anywhere.** The only words about tuning, tones and the hum are music.md section
  15's. Source: the dated finding [2026-11-06-music-sources.md](../reference/findings/2026-11-06-music-sources.md), Part E.
- **No caches across runs.** The data files are read each time a sound starts; nothing the engine
  computes is kept between runs. Source: CLAUDE.md, "No caches kept across runs".
- **Never networked.** Each player's game composes its own music. Source: music.md section 14.

## How to test it

- **Its lib tests:** the `just test-lib --crate humanity-music` line above. `src/tests.rs` holds the
  ones that render nothing (the pitch table, the shipped data, places, an hour of the composer for
  repeats and voice leading, the style layer, the intensity bands); `src/tests_audio.rs` the ones that
  render and measure (tuning in the audio, the steel pan's and deep drum's partials and decay, the
  hum and its alarm, loudness and true peak with humanity_core's BS.1770 meter, determinism, the
  cost per block, no allocation, the sampler seam, the duck, a sustain held past its recording by a
  crossfaded loop). The crate is optimised in dev and test builds (the root Cargo.toml), so its
  timing tests measure the game's speed.
- **What starting the score costs, optimised:** the ignored `kira_sound::device_tests::
  decode_cost_of_starting_the_score` (`cargo test --release -p humanity-music --lib decode_cost --
  --ignored --nocapture`): the data read and the core set decoded as the engine's build does.
- **The engine's side** (`src/engine/music_tests.rs`): where the music is heard from, the gains and
  ducks, the build on its worker thread, a new score taking over at a phrase after an install, the
  reuse switch, and the ignored `the_score_with_recordings_measured` (each mood with the core set's
  recordings attached: loudness, true peak, cost per block, decoded memory).
- **The shipped data:** `the_shipped_music_data_parses` runs in `just validate-data`.
- **Listen and measure offline:** `just music-render <mood> <style> <seconds>` (options: `--ramp`
  for an intensity over time, `--hum`, `--breach`, `--blend`, `--stems` for each role's loudness
  against the whole, `--a4 440`); the WAV goes to the temp folder or `--out`, never the repo.
- **On a real sound device:** the ignored tests `kira_sound::device_tests` (run with `cargo test -p
  humanity-music --lib -- --ignored device`): a script's boot of the game opens no audio device, so
  `just verify-runtime` cannot hear the music; these do.
- **In the running game:** the Dev page's Music card (the danger knob and the readout), and the
  `music_intensity` / `music_breach` showcase verbs through the MCP's `game_showcase`.

## Open work

- **Rungs 4 to 10 of the music** (the menu mood on the title screens, voice-call ducking, the self-meter and `music_state` on the IPC,
  sampled instruments, stingers and gaps, the soft-metal guitar and the soprano, the website,
  making music in the app), and the styles' later rungs (electroswing, dubstep, a danger palette
  per style). Source: [music.md](../design/music.md), sections 7a, 7b and 20.
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
