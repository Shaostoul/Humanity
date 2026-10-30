# Splitting the crate: what goes where, and in what order

Status: **proposal, 2026-10-30, for the operator's decision.** Nothing is moved yet. It would
reverse the 2026-06-30 one-crate decision (CLAUDE.md, Architecture: "one `Cargo.toml` at the repo
root, no workspace, no sub-crates"), so the operator decides whether, and the layering below is
the part to agree first. Every number here comes from `node scripts/crate-graph.js` (and
`--edges`) on 2026-10-30, or from `docs/ai/builds.md` ("Measured 2026-10-24"); re-run the script
before acting, because the code moves daily.

## Why

- **One compile of our crate, cold: 241 s** on the operator's PC. Type checking 46 s and borrow
  checking 50 s run on ONE core inside one crate (about 40% of every compile); code generation
  105 s is already spread across the cores; linking 5 s (`docs/ai/builds.md`).
- **A one-line edit re-checks all of it: 56 s.** The crate is 639,084 lines of Rust in 1,037 files
  (the biggest single file is `src/lib.rs`, 16,672 lines), and an edit anywhere re-checks
  everything.
- **The server build** (the relay, `--features relay`) compiles the same crate: 10 to 11 minutes on
  the VPS's two cores, about 9.5 minutes on a GitHub runner. Since 2026-10-30 it runs on GitHub
  (`.github/workflows/deploy.yml`), but its length is the crate's.

Separate crates let Cargo re-check only the crate an edit touches and the ones above it, and check
crates that do not depend on each other at the same time, on different cores.

## What the code looks like today

- **50 top-level modules** (declared in `src/lib.rs`). **31 of them, 618,218 of 622,612 lines, form
  one cycle**: each reaches the others directly or round a loop (the shared simulation `systems`
  reaches `gui`, `engine` and `save_load`; `ship` reaches `engine`, `gui` and `renderer`; `terrain`
  and `renderer` reach each other; `ecs` reaches `systems` and `renderer`). Cargo forbids cycles
  between crates, so the split cannot follow today's module lines as they stand.
- **But the cycle is held together by very little.** Under the layering below, only **102
  references in 30 edges** of non-test code point the wrong way (upward). The large raw counts
  (`systems -> gui` 15 and the like) are mostly tests and comments.

## The layering (each crate may use only its own layer and the ones below)

| Crate | Layer | Modules | Lines |
|---|---|---|---|
| `hos-core` | 1 | the small leaves (input, audio, utilities, physics, debug, platform, turning, reactions, lod_registry, boot_timing, credits, scenic_views, showroom, mods, player_look, cosmetics, auto_unlock) and the core data (embedded_data, ecs, hot_reload, storage, cosmos, dev_travel) | ~11,000 |
| `hos-sim` | 2 | systems, machines, ship, persistence, station, surface_walk, surface_move | ~111,000 |
| `hos-relay` | 3 | relay, how_to, release_update (the server) | ~123,000 |
| `hos-render` | 3 | renderer, terrain, assets (beside the server, not above it) | ~93,000 |
| `humanity` (the app and the binary) | 4 | engine, gui, net, config, save_load, media, games, software, web_reader, updater, wasm_entry, and `lib.rs` | ~301,000 |

The feature flags become crate choices: the server build is `hos-relay` and what it needs (core,
sim), with no GPU crate in it at all, instead of `--no-default-features` on one crate.

## The 102 references to re-home first (the full list: `node scripts/crate-graph.js --edges`)

| Edge | Refs | What crosses | The cut |
|---|---|---|---|
| systems -> assets | 24 | `assets::loader::parse_csv`, `parse_ron` | Move the CSV/RON loaders down into `hos-core` (pure parsing). Also fixes embedded_data -> assets (3) and ship -> assets (1). |
| ship -> renderer | 12 | `renderer::mesh::Vertex` (7), camera constants | Move the vertex type and the few constants down to core; the renderer uploads what ship builds. |
| ship -> engine | 8 | `engine::footing` (Footing, STEP_UP_M, test_aboard), `engine::home_plot` | Move footing into `hos-sim`; it is how a body stands on a deck, not engine plumbing. |
| surface_walk, surface_move, dev_travel, systems -> terrain | 19 | planet heightmap, PlanetDef, ocean mask, planet chunks' height and noise | Move the planet DATA modules (planet, planet_heightmap, planet_surface, ocean_mask, terrain_tiles, drawn_surface) into `hos-sim`, and split `planet_chunks.rs` (4,621 lines, 13 renderer references) into its height and noise math (sim) and its meshes (render). **The hardest single cut.** |
| hot_reload -> engine | 6 | `EngineState`, `reload_planet_defs`, `rebuild_plant_meshes` and the like | Invert it: hot reload reports what changed; the engine reacts. |
| machines, systems -> save_load | 5 | `active_home`, `apply_save_to_world`, `catch_up_world` | Move the active-home notion and the world catch-up into `hos-sim`. |
| systems, ship -> gui | 4 | `gui::i18n::t`, `gui::RoomBounds` | Move the word lookup (`t`) and RoomBounds down; the server already reads the words without the GUI. |
| surface_move -> renderer | 3 | `Camera::new`, `celestial_projection_f64` | Move the projection math down. |
| the rest | 21 | one to two references each (ecs -> `frame_costs`, `MET_STANDING`; relay -> `net::dm_pq::build_signed_inner`; renderer -> `games::registry`, `net::live::RawFrame`; storage -> `HomeDesign`; frame_lock's `ground_radius_m`; `AppConfig::default` in terrain; how_to -> the app's page list; cosmetics and player_look -> ecs components) | Each moves to the lowest crate that uses it, or the caller takes it as an argument. |

## What it would buy, by how the project is actually edited

Commits touching `src/` in the 30 days to 2026-10-30, by the crate they would touch (a commit can
touch more than one): **app 360, sim 157, server 151, core 48, render 39** (of 642).

- **An app-only edit** (the most common) would re-check about 301,000 lines instead of 639,000.
  If re-checking scales with lines, the 56 s becomes about 26 s. **Estimate, to be measured.**
- **A server-only edit** would rebuild `hos-relay` on top of core and sim already built: the
  relay's compile is about a fifth of the crate's.
- **A simulation edit** rebuilds sim and everything above it: little gain (157 commits).
- **A cold build** checks the server and render crates at the same time, on two cores, which
  today run one after the other inside one crate.
- **A catch for CI:** a fresh checkout gives every file a new timestamp, so Cargo rebuilds path
  crates whatever the cache holds. The CI gain needs either `git restore-mtime` before building, or
  Cargo's content-hash freshness, which is still unstable. Locally, the gain is immediate.

## Costs and risks

- **A Cargo workspace** replaces the one `Cargo.toml` (`[workspace]` with five members). CLAUDE.md's
  Architecture section, the build slot (`rustc-workspace-wrapper` already wraps every member), the
  lint and test recipes, CI's three workflows and the release scripts all change in step.
- **Speed across crates:** a small function in one crate is only inlined into another with
  `#[inline]` or link-time optimisation. Release builds may want `lto = "thin"`; measure the frame
  cost with `just perf-sweep` before and after.
- **Visibility:** `pub(crate)` items used across a new boundary become `pub`; the move makes those
  uses visible, which is a review, not a risk.
- **Tests** move with their code; a test that reaches across layers becomes an integration test of
  the crate above.
- **No runtime change** is intended anywhere. Each rung must keep the farming golden state
  (`src/systems/farming/golden_tests.rs`), the lib tests, the relay battery and a
  `just verify-runtime` boot exactly as they were.

## Order of work (each rung shippable alone, and measured)

0. **Inside the one crate, re-home the 102 references** (the table above), a few edges per commit,
   with no Cargo change. Then add a lint (`tests/layer_lint.rs`, the same reading as
   `scripts/crate-graph.js --edges`) that fails on any new upward reference, so the cycle cannot grow
   back while the rest is done. **This rung is worth doing even if the split never happens:** it
   untangles the dependency structure.
1. **The workspace and `hos-core`** (the smallest and lowest). Measure a cold build and a one-line
   app edit against 241 s and 56 s.
2. **`hos-sim`.** Measure.
3. **`hos-relay` and `hos-render`.** Measure the server build on GitHub; the VPS deploy then
   rebuilds only what changed.
4. **What is left is `humanity`,** the app and the binary.

## For the operator to decide

1. **Whether to split at all** (it reverses 2026-06-30's one crate), knowing rung 0 is worth doing
   either way.
2. **The layering:** these five crates, and which side of the line each module sits on. The table is
   a proposal; the module that most deserves a second look is `terrain`, split between sim and
   render.
3. **The names.** `hos-` is a placeholder prefix.
