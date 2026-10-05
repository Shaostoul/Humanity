//! Satellites of the native app's main module, extracted from lib.rs
//! (v0.932, tiers A+B of docs/dev/code-structure-plan.md). Everything here is
//! self-contained: pure math and parsers, plus loaders that take only
//! `DataStore` / `hecs::World`, never `EngineState`. lib.rs glob-imports these
//! inside `mod native_app`, so call sites are unchanged. Later tiers (IPC
//! pollers, frame-lock math, the editor cluster) land beside them as they
//! extract.

pub mod color;
pub mod dm;
pub mod editor;
pub mod frame_lock;
pub mod geom;
pub mod home_meshes;
/// The pipes' marker bands in the 3D home: one merged mesh per band colour, and the
/// Settings / showcase mode switch (2026-10-04; the placement and colours are ship::pipe_marking).
pub mod pipe_markers;
/// Your home on the shared ship: applying the relay's plot from `game_welcome`
/// (increment 1b of docs/design/ship-homes-and-logistics.md).
pub mod home_plot;
/// The fleet ledger's engine half (2026-10-04): the server's answers into the fleet panel, a
/// give's items out of the backpack once, and the home's reactor power reported.
pub mod fleet;
pub mod home_spawn;
/// The F10 sidebar's key rules (F10 toggle, Escape-closes-first), the
/// cursor-free predicate and the flag-by-name lookup the dev IPC reports
/// (2026-09-18). Pure GuiState logic: the key, the tests and the IPC share it.
pub mod input;
pub mod ipc;
pub mod ipc_parse;
/// The Settings > Controls key-capture step (rebindable keybinds, 2026-08-12).
pub mod keybind_capture;
pub mod launch_focus;
/// Movie mode: record the live view to video, frame-exact, for the clip
/// maker (scripts/make-clips.js, 2026-09-30).
pub mod movie;
/// Leaving the app: the save every way out runs first (first-hour audit 2026-10-04, Blocker 5).
pub mod quit;
/// The character's own home (2026-10-04): the build editor's edits outside the Dev mode are kept
/// in the character's save, never in the shared data files; what a placement paid comes back.
pub mod own_home;
/// The Death setting's engine half (2026-10-04): a death surfaced, the pack left where the
/// player fell in Realistic, its prompt, E, drawing, marker and clock (systems::death_pack).
pub mod death_pack;
/// The game's half of the relay's speed check: corrections, declared fast moves, the rig's walk
/// (ship homes increment 4).
pub mod move_check;
/// How a mushroom crop is drawn: its fruiting blocks or cased bed, and the
/// mushrooms on them by stage (2026-09-27).
pub mod fungus_mesh;
/// Where a garden plot's plants stand: plots of a machine, the crop's rows at
/// its real spacing, and the clump rule above the visual cap (2026-09-26).
pub mod plant_layout;
/// The garden's plant geometry, built on a worker thread and uploaded a few
/// milliseconds a frame (2026-09-27).
pub mod plant_pass;
/// Near-tree model cache + sprite-atlas bake (extracted from lib.rs, v0.1108).
pub mod near_tree_models;
/// One frame's relay WebSocket message pump: every `type` the relay can send,
/// plus the socket-died teardown (extracted from lib.rs, v0.1320).
pub mod frame_ws_poll;
/// The planet's two water shells (coarse backstop + displaced wave surface),
/// built and pushed once per frame (extracted from lib.rs, v0.1320).
pub mod frame_water;
/// Near-field 3D trees: the gated harvest, the per-tree draw plan, and the
/// card-hide promise the far LOD depends on (extracted from lib.rs, v0.1320).
pub mod frame_near_trees;
/// The rig's readout of the ground under the near trees and the eye, against
/// the ground actually drawn there (BUG-156, 2026-10-05).
pub mod tree_ground;
/// The rig's held movement keys: arriving somewhere on foot the way a player
/// does, instead of by teleport (BUG-156, 2026-10-05).
pub mod rig_walk;
/// A planet's cloud deck and atmosphere dome, plus the view-dependent order
/// the two composite in (extracted from lib.rs, v0.1320).
pub mod frame_shells;
pub mod region_meshes;
/// Room GI rung 1 (2026-09-27): the ship's rooms into probe boxes, and the
/// per-frame hook before the scene pass. The probes live in renderer::room_probes*.
pub mod room_gi;
/// Background relay connections: dial + keep-alive + compact router for
/// every saved server that is not the active one (multi-connection).
pub mod bg_connections;
/// A server confirmed it erased our account: leave it, keep it undialed, and
/// take the game out of its shared world (BUG-135).
pub mod account_erase;
/// Built beds and chests in use: the crosshair prompt, the E press, and
/// built chests as containers in the places tree (2026-09-27).
pub mod built_uses;
/// The quests' per-frame glue (2026-10-04, the opening): views that came on
/// screen as quest events, and the player's own front door for Travel.
pub mod quest_hooks;
/// Placing a built piece: the ghost that follows the crosshair, R to turn
/// it, E to build it there, Esc to stop (2026-09-27).
pub mod build_place;
/// The pieces the server keeps, the game's side (ship homes increment 5): the gate before
/// anything is spent, sending builds and take-downs, and applying what the relay says.
pub mod shared_build;
/// Building on a planet's ground as well as aboard (2026-09-27, BUG-102):
/// the frame the player builds, shelters and looks in, the ground under the
/// crosshair, and drawing built pieces where they stand.
pub mod planet_build;
/// The survival environment context (the home's air or the weather) and the
/// body heat mode, published once a frame (moved out of lib.rs 2026-09-27).
pub mod survival_env;
/// The player's carried load: the walking speed and jump it allows, and the
/// Weight tile and HUD line it publishes (BUG-136, 2026-10-04).
pub mod carry_load;
pub mod net_route;
pub mod registries;
/// In-world screens: native pages on flat displays placed in the 3D world
/// (quads, look-ray hit tests, input routing, per-frame surface drawing).
pub mod screens;
pub mod state;
/// Visible storage: crates in the storage zones and tank level bars.
pub mod stock_piles;
pub mod world_load;
