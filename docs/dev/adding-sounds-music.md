# Adding Sounds and Music

How audio works in HumanityOS today, and how to add the first real game sounds.
Written for contributors, the operator, and AI agents alike. Everything here is
grounded in the actual code, read the cited files before changing anything.

## Honest current state (2026-07-20)

**The game currently plays no sound.** All the plumbing exists but nothing is
connected:

| Piece | File | Status |
|-------|------|--------|
| `AudioManager` (kira crate) | `src/audio/mod.rs` | Built, ZERO callers. Nothing constructs it. |
| `SoundCatalog` (data-driven IDs) | `src/audio/sounds.rs` | Built, ZERO callers. `SoundCatalog::load` is never invoked. |
| Sound catalog data | `data/sounds.toml` | Exists (ambient/sfx/music/ui entries), never loaded at runtime. |
| Catalog schema | `schemas/sound.toml` | Exists. |
| Audio files | `data/audio/` | Only a README. No .ogg files are committed (see below). |
| Settings volume sliders | `src/gui/pages/settings.rs` `draw_audio_content` | Working UI, saved to config (`src/config.rs` `master_volume` / `music_volume` / `sfx_volume`), but **placebos**: no AudioManager exists to consume them. |
| Voice chat audio | `src/net/voice.rs` (cpal + opus) | This IS wired and working, but it is the voice-call path, completely separate from game audio. Do not confuse the two. |

So the Settings > Audio sliders persist values that nothing reads yet. That is
the gap this doc exists to close.

## What AudioManager already does (`src/audio/mod.rs`)

All native-gated (`#[cfg(feature = "native")]`, kira does not support WASM; the
relay build gets a stub struct):

- `AudioManager::new()` - creates the kira backend. Note: it currently uses
  `.expect(...)` on failure, which would panic on a machine with no audio
  device. Before wiring it in, soften that to a logged fallback (a headless or
  broken-audio machine must still boot the game).
- `play_sound(path)` - one-shot SFX at `master_volume * sfx_volume`.
- `play_music(path, volume)` - looping music, stops the previous track with a
  500 ms fade, volume = `master * music * volume`.
- `stop_music()`.
- `set_master_volume(v)` / `set_music_volume(v)` / `set_sfx_volume(v)` - all
  clamp to 0.0..1.0. **Caveat:** these only affect sounds started AFTER the
  call; already-playing handles are not retuned (the music handle is kept, so
  live music retune is a small follow-up if wanted).
- `play_spatial(path, source_pos, listener_pos)` - simple distance falloff
  (linear to 50 m, silent beyond) + stereo panning from the X offset. Real HRTF
  or Steam Audio is a stub (`src/audio/spatial.rs`).
- Sounds are cached by path in a `HashMap` after first load.

## What SoundCatalog already does (`src/audio/sounds.rs`)

- `SoundCatalog::load(data_dir)` parses `data/sounds.toml` (two-level TOML:
  `[sfx.footstep_grass]`, `[ambient.rain]`, ...) into dotted-ID entries with
  `path`, `volume`, `loop`, `spatial`, `falloff_min/max`, `bus`, `variations`,
  `tags`. Graceful degradation: any parse failure returns an empty catalog.
- `path_or(id, fallback)` falls back to the hardcoded constants at the top of
  the file (`UI_CLICK`, `FOOTSTEP_GRASS`, `MUSIC_MENU`, ...).
- `SurfaceType::footstep_sound_from(catalog)` picks footstep sounds per surface.

**Known path inconsistency to resolve when wiring:** the fallback constants in
`sounds.rs` point at `data/audio/...`, while `data/sounds.toml`'s header says
paths are relative to `assets/audio/` and its entries carry `audio/...` paths.
Pick ONE convention when you do the integration (recommendation below) and fix
whichever side loses.

## The intended integration path

Follow the same ownership pattern every other subsystem uses:

1. **`EngineState` owns it.** Add an `audio: AudioManager` field to
   `EngineState` in `src/lib.rs`, constructed during engine init (next to the
   renderer and asset manager), native-gated. Load the `SoundCatalog` there too,
   from the asset manager's data dir.
2. **Settings wiring.** On startup and whenever `state.settings_dirty` commits
   volume changes (the `draw_audio_content` sliders in
   `src/gui/pages/settings.rs`), push the three values into the manager via
   `set_master_volume` etc. That single step turns the placebo sliders real.
3. **Frame access.** Game systems that want to make noise need reach into the
   manager. The simplest first wiring is direct calls from `lib.rs` event sites
   (the places that already know "a click happened", "a harvest completed").
   The cleaner end state is a small sound-event queue: systems push
   `(sound_id, Option<world_pos>)`, and one drain point per frame in the main
   loop resolves IDs through the catalog and calls `play_sound` /
   `play_spatial` with the camera position as the listener. That keeps
   `System::tick` implementations free of renderer/audio references.
4. **Hot reload.** The data file watcher (`src/assets/watcher.rs`) already
   reports changed paths; add a `sounds.toml` check alongside the existing
   `plants_visual.ron` reload in `lib.rs` so sound tuning is live.

## Asset conventions (establish these as you add the first files)

- **Location:** `assets/audio/<bus>/<name>.ogg` in the repo, mirrored into the
  distributed `data/audio/` tree the same way models are (see the resolution
  rule in `docs/game/model-pipeline.md`: data dir first, repo root second).
  `data/sounds.toml` entries should then reference `audio/<bus>/<name>.ogg`.
- **Buses:** `ambient`, `music`, `sfx`, `voice`, `ui` (already the catalog's
  vocabulary).
- **Format:** OGG Vorbis preferred. Kira 0.9 with default features decodes
  ogg, wav, mp3, and flac. Use ogg for everything shipped (small, patent-free);
  wav is fine for tiny UI blips if quality demands it.
- **Loudness:** normalize so `volume = 0.5` in the catalog sounds right;
  per-sound trim lives in the TOML, not in re-exported files.
- **Naming:** snake_case, describing the event not the file source:
  `footstep_grass.ogg`, `craft_complete.ogg`.
- Large audio packs are NOT committed to git (`data/audio/README.md` states
  this). Small essential SFX can be committed; a full music/ambience pack
  should ship as a release download, like the terrain tile packs.

## Licensing rules

- **CC0 strongly preferred** (the whole project is CC0; CC0 sounds keep it
  clean). Good sources: freesound.org (filter license = CC0), Sonniss GDC
  packs (check each pack's terms), kenney.nl (CC0).
- CC-BY is acceptable if CC0 truly is not available, but then attribution is
  MANDATORY.
- Every imported sound gets a line in `CREDITS.md` (repo root) under a "Sound
  and music sources" section: source name, URL, license. The albedo bake
  scripts treat their source list as the credits record; audio follows the
  same rule.
- Never import anything ripped from a commercial game, film, or a "free"
  YouTube rip. Provenance must be a real license.

## Wiring checklist for the FIRST sound

The first sound proves the whole chain. Suggested target: UI click (no world
position needed, triggers constantly, instantly verifiable).

1. Add `assets/audio/ui/click.ogg` (CC0, credit it in `CREDITS.md`).
2. Fix the path convention mismatch (see above) in `data/sounds.toml` +
   `src/audio/sounds.rs` fallbacks.
3. Add `audio: AudioManager` + `sound_catalog: SoundCatalog` to `EngineState`;
   soften `AudioManager::new` to non-panicking.
4. Push Settings volumes into the manager at init + on settings change.
5. Call `play_sound` from one real event site (a nav button press in
   `src/gui/pages/escape_menu.rs`, or a machine placement in the construction
   flow).
6. Verify like a renderer change: `cargo build --features native --release`,
   boot the exe, click the thing, hear the sound, drag the SFX slider to 0,
   confirm silence. Also `cargo check --features relay --no-default-features`
   (audio is native-gated; the relay build must stay green).
7. Update the status table at the top of THIS file, it stops being true the
   moment the first caller lands.

## Related docs

- [adding-game-data.md](adding-game-data.md) - the data-driven content system
  `sounds.toml` belongs to.
- [docs/design/infinite-of-x.md](../design/infinite-of-x.md) - why sound IDs
  live in a TOML, not in code ("Audio clips, music tracks, sound effects" is on
  its must-be-data list).
- `schemas/sound.toml` - the schema for catalog entries.
