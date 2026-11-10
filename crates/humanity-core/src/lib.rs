//! humanity-core: the bottom of the stack, which every other crate of the workspace uses (the crate
//! split, docs/design/crate-split.md; its card is docs/crates/humanity-core.md). The small leaves, the
//! ECS world, the data files' parsers and built-in copies, hot reload, storage, the Sol model, the word
//! source, the cost store and the one switch that turns off all reuse.
//!
//! It uses no other crate of ours, and no GPU, GUI or server code: an edit here re-checks every crate
//! above it, so only what several crates need belongs here. Cut out of the one crate in rung 1
//! (2026-11-01); its modules kept their names, so a caller names them `humanity_core::storage::...`.
//! Feature flags: `native` (the desktop app's parts; the root's `native` turns it on), `wasm`, and
//! `test-support` (the test helpers other crates' tests use, turned on only by their
//! [dev-dependencies]).

pub mod ecs;
pub mod physics;
pub mod audio;
pub mod input;
// The core's modules that came down from above in the crate split's rung 0 (2026-10-31,
// docs/design/crate-split.md); each file's header says why it lives there.
/// The data files' parsers (CSV, TOML, RON, JSON), pure and ungated (was `assets::loader`).
pub mod data_parse;
/// The one mesh vertex: plain data every mesh builder writes (was `renderer::mesh::Vertex`), and
/// the skinned vertex characters bend with (character animation rung 1, 2026-11-08).
pub mod vertex;
/// Skinned models and animation clips as plain imported data: the glTF reader fills it, the
/// animation crate builds skeletons and clips from it (docs/design/character-animation.md).
pub mod skin_data;
/// What a frame cost: the store every system records into (its GPU half is `renderer::gpu_costs`).
pub mod frame_costs;
/// The play journal (2026-10-24, docs/design/play-journal.md): what happened while the game ran, in
/// small capped files beside the logs. Its frame half is engine/play_journal_frame.rs.
#[cfg(feature = "native")]
pub mod play_journal;
/// A frame of pixels read back from the GPU for the live stream (was `net::live`'s).
#[cfg(feature = "native")]
pub mod raw_frame;
/// The one word source: every word a person reads, by key, from data/i18n (was `gui::i18n`).
#[cfg(feature = "native")]
pub mod i18n;
/// The household permit and a refused build's codes (was in `systems::construction::shared`).
pub mod build_permission;
/// The crash-safe file write (was `persistence::write_atomic`).
pub mod atomic_write;
/// The live weather map's size, 1440 x 720 (was the renderer's).
pub mod weather_map;
/// Calendar dates from day counts (was `relay::epoch_days_to_ymd`).
pub mod civil_date;
pub mod embedded_data;
pub mod platform;
pub mod showroom;
/// Utility wiring -- connection ports + cables/conduit with real specs (v0.604). Ungated (pure
/// serde + math) so it compiles under relay too.
pub mod utilities;
/// The one catalog of what a run (a pipe, hose, tube or cable) is made of: looks, behaviour classes,
/// sizes and their electrical, fluid and data specs (data/piping/pipe_materials.ron; spline
/// routing rung 0, 2026-11-02). Here because `utilities` reads its cable specs. Ungated.
pub mod run_materials;
/// Where a data row's numbers come from: a dated finding (R1.., D1..) or `Ours` with a reason, and
/// the check that reads the findings documents' own numbered paragraphs (spline routing rung 0).
pub mod row_source;
pub mod cosmetics;
/// Canonical Sol-system model (one `SolBody` set + one Kepler
/// propagator) shared by the Maps page, the FPS world spawn, and the
/// in-home holo-orrery. Engine-wide on purpose — NOT `#[cfg(native)]` —
/// so terrain / renderer / world placement read the same source of
/// truth instead of drifting per-view. See `crates/humanity-core/src/cosmos.rs`.
pub mod cosmos;
/// How another player looks in a shared world (2026-09-29): compiled into the
/// app and the relay alike. See `crates/humanity-core/src/player_look.rs`.
pub mod player_look;
/// How another player stands (crouching, off the ground) as the movement message carries it
/// (character animation rung 2a, 2026-11-09): compiled into the app and the relay alike, so both
/// read and clamp it the same way. See `crates/humanity-core/src/stance.rs`.
pub mod stance;
/// Dev travel math (teleport viewpoints + camera aim for the Dev page's
/// FTL/teleport tool). Pure glam, ungated like `cosmos` so its unit tests
/// run under every feature set. See `crates/humanity-core/src/dev_travel.rs`.
pub mod dev_travel;
/// Third-party attributions, several of which are licence obligations.
pub mod credits;
/// Which way a camera looks: the planet-surface tangent frame and the yaw and pitch to a direction
/// and back. Pure glam, ungated; in the core since the crate split's rung 0 (it was in `surface_walk`).
pub mod camera_angles;
pub mod turning; // How a person turns (the rig's walking camera, the crew figures); ungated, see crates/humanity-core/src/turning.rs.
/// Curated named viewpoints (data/scenic_views.ron) for the camera-hub arc.
pub mod scenic_views;
/// Reaction emoji palette (data/reactions.json): one source for the native
/// picker, the relay allowlist, and (via HTTP) the web picker. Ungated - both
/// feature sets consume it.
pub mod reactions;
/// Vegetation/creature LOD category registry (data/vegetation/lod_categories.ron).
pub mod lod_registry;
/// Boot-phase timing -> log summary + debug/boot_timing.json (dev tooling).
pub mod boot_timing;
/// The one "turn off all reuse" switch every in-memory reuse obeys (CLAUDE.md's reuse rule, 2026-10-30).
pub mod reuse;
/// A clock a test moves by hand, for code that decides things by elapsed time
/// (BUG-152). Test builds only: this crate's own, and another crate's through the
/// `test-support` feature its [dev-dependencies] turn on (the crate split, rung 1).
#[cfg(any(test, feature = "test-support"))]
pub mod test_clock;
/// Temporary files and folders that tests delete when done (BUG-159). Test builds only, as above.
#[cfg(any(test, feature = "test-support"))]
pub mod test_temp;
/// The warnings a test's own code logs, read back (BUG-163). Test builds only, as above.
#[cfg(any(test, feature = "test-support"))]
pub mod test_log;
/// May a test open a GPU device now? No while the operator's GPU-heavy game runs: a GPU test
/// can hang the whole run beside it, silently (2026-11-01). Native test builds only, as above.
#[cfg(all(any(test, feature = "test-support"), feature = "native"))]
pub mod test_gpu;
pub mod hot_reload;
pub mod storage;
#[cfg(feature = "native")]
pub mod mods;
// v0.278.0: optional silent / quick-PIN unlock paths on top of the
// passphrase-encrypted vault. Native-only — relay/wasm don't carry an
// interactive identity.
#[cfg(feature = "native")]
pub mod auto_unlock;
pub mod debug;
