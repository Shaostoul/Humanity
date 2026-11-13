//! Satellites of the native app's main module, extracted from lib.rs
//! (v0.932, tiers A+B of docs/dev/code-structure-plan.md). Everything here is
//! self-contained: pure math and parsers, plus loaders that take only
//! `DataStore` / `hecs::World`, never `EngineState`. lib.rs glob-imports these
//! inside `mod native_app`, so call sites are unchanged. Later tiers (IPC
//! pollers, frame-lock math, the editor cluster) land beside them as they
//! extract.

pub mod color;
pub mod dm;
/// The server's answers to the `dm_put`s that change friendship state (10l, 2026-10-10): a pass
/// counts as given only once the server took it.
pub mod put_answer;
/// Household permits travel between devices (ship homes increment 5b, 2026-10-12): giving,
/// renewing and taking back a permit for my plot, a friend's permit arriving, and the server's
/// answers to those sends. The rules without a socket are src/net/household.rs.
pub mod household;
/// The household permits in the running world: the plots this game may walk into, opened on the
/// live ship each frame, and the dev IPC's report (split from household.rs in the crate split's
/// rung 0, part B: household.rs works on the GUI's state and is the GUI crate's, these on the
/// engine's).
pub mod household_world;
/// "Who can reach me" (step B of docs/design/blocking-and-safe-mode.md, 2026-10-09): the
/// relay's reach frames, contact requests sent and received, and the "People I choose" ticks.
pub mod reach;
/// Block (step C of docs/design/blocking-and-safe-mode.md, 2026-10-09): Block and Unblock, the
/// notes to oneself, and the rules that hide a blocked key on every path in.
pub mod block;
/// Key cards (2026-10-22, docs/design/key-cards.md): our own card signed and sent with each
/// sign-in, other people's asked for, checked and pinned, and the change notice. Every seal to a
/// person reads the keys kept here.
pub mod key_cards;
/// The choice for each friend syncs as its own note (10n of docs/design/blocking-and-safe-mode.md,
/// 2026-10-10): making it, the notes to oneself, and Unfollows that wait for a connection.
pub mod choice;
/// The mailbox of the server we are on, page by page and live (10o O1, 2026-10-10): only pages
/// carrying this connection's own ref count, and a live DM moves the read position only once its
/// own fetch is done.
pub mod mailbox;
/// Notifications (2026-10-30, docs/design/notifications.md): a live DM or mention handed to the
/// rule with what only the engine knows (focus, what is on screen), and the sound and notice done.
pub mod notify;
/// The always-allow list: the people whose calls and messages come through quiet hours, and the
/// notes to myself that carry it between my devices (notifications rung 3).
pub mod quiet_allow;
/// Tasks of my own (docs/design/tasks.md, 2026-11-02): made, sent as notes to myself and taken
/// from them; the board's list never replaces them (BUG-217).
pub mod mine_tasks;
/// Reports the admins can check (step D of docs/design/blocking-and-safe-mode.md, 2026-10-09):
/// the Report dialog's opening and sending, and the relay's `report_received` and `reports`.
pub mod report;
/// A report about a group reaches the group's creator (10j of docs/design/blocking-and-safe-mode.md,
/// 2026-10-10): the dialog's creator lookup and sending, the checks on arrival, and the creator's
/// Remove, Block and Dismiss.
pub mod group_report;
/// Calls through the server (step E of docs/design/blocking-and-safe-mode.md, 2026-10-09): asking
/// for a call or voice room's `call_credentials`, handing them to the WebRTC manager, and the line
/// the call UI shows when the server cannot carry the call.
pub mod call_relay;
/// Device-to-device big files (docs/design/files-showcase.md, answer 10, 2026-10-11): offering a
/// file, the offer notes as they arrive, Accept and Decline, each transfer's connection through
/// the server's call forwarder, Pause, Resume and Cancel.
pub mod file_transfer;
/// Warnings, and the recovery-phrase guard (step F of docs/design/blocking-and-safe-mode.md,
/// 2026-10-10): which messages get which warnings, "Got it", and the guard every send path calls.
pub mod warnings;
/// The protected setup (step G of docs/design/blocking-and-safe-mode.md, 2026-10-10): the PIN
/// gate every locked action asks, turning it on (one `reach_set`, nothing else), the review
/// step's Remove, and what the chat leaves out while it is on.
pub mod protected;
/// Friend codes (2026-10-10): making one (locked by the protected setup), showing the code the
/// server made, and following a redeemed code's owner through Follow's own gate.
pub mod friend_code;
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
/// Which save is live (your own home, or one server's, made fresh from the start you chose there)
/// and the home a save's start builds (ship homes increment 6).
pub mod save_slot;
/// The fleet ledger's engine half (2026-10-04): the server's answers into the fleet panel, a
/// give's items out of the backpack once, and the home's reactor power reported.
pub mod fleet;
/// The mess hall's serving counter (ship homes increment 6): a free crew meal, the server's in
/// the shared world and the ship's stores on this computer elsewhere, and drinking water.
pub mod mess_hall;
/// The live music and the ship's hum, wired into the frame (2026-11-06, crates/humanity-music).
pub mod music;
/// Characters on screen: every humanoid as one skinned, animated body (character animation rung
/// 1, 2026-11-08, crates/humanity-animation and renderer/skinning.rs).
pub mod animation;
/// The ground under a person's feet for humanity-animation's feet (character animation rung 2a,
/// 2026-11-09): the footing aboard, the drawn terrain on a planet.
pub mod feet_ground;
/// The foot probe: the drawn feet measured against the ground under them (rung 2a's proof).
pub mod foot_probe;
/// Interactions in the world (character animation rung 2b, 2026-11-10): where the anchors stand,
/// who is granted one (a seat, the bed, a ladder, a station, a door), the capsule moved only inside
/// the grant.
pub mod interact;
/// The interaction probe: the drawn body measured against what it touches (rung 2b's proof).
pub mod anchor_probe;
/// The talk card each frame (embeddable-ai.md rung 3, 2026-11-12): its AI box while it is open, the
/// way it is showing on the HUD, and its dev IPC for the ai-talk rig (debug/talk_request.json).
pub mod talk;
/// The garments' textures read off the frame and packed for the character materials (the look,
/// 2026-11-09).
pub mod look_textures;
/// The game's one-shot sounds on the volume slider each belongs to, and the volumes' push (BUG-265).
pub mod sfx;
pub mod home_spawn;
/// The F10 sidebar's key rules (F10 toggle, Escape-closes-first), the
/// cursor-free predicate and the flag-by-name lookup the dev IPC reports
/// (2026-09-18). Pure GuiState logic: the key, the tests and the IPC share it.
pub mod input;
/// What a changed data file rebuilds in the running game, once a frame (the gates of hot reload:
/// planets, plants, the hull, game.csv). The watcher is the core's `hot_reload`; the gates touch
/// the engine's state, so they are here (the crate split's rung 0, part B, 2026-10-31).
pub mod hot_reload_gates;
pub mod ipc;
pub mod ipc_parse;
/// The Settings > Controls key-capture step (rebindable keybinds, 2026-08-12).
pub mod keybind_capture;
pub mod launch_focus;
/// LOW IMPACT (HUMANITY_LOW_IMPACT, 2026-10-22): the game a behaviour rig boots while the operator
/// uses his PC: minimum graphics, a low frame cap and a small window, never saved, nothing the game
/// does changed. Which rigs ask for it, and when: scripts/lib/rig-needs.js.
pub mod low_impact;
/// Maps increment 1 (2026-10-21): the engine tells the Maps page where you are
/// (`MapSelf`), and renders the page's world layer (your ship from its meshes).
pub mod map_feed;
pub mod map_view;
/// The Maps page's dev verbs and the rig's report (Maps increments 1 and 2).
pub mod map_rig;
/// Maps increment 3 (2026-10-22): the level verbs and the ship block of the rig's report, the
/// cutaway of the level shown (the editor's cut-away rule), and the plan tiles.
pub mod map_rig_ship;
pub mod map_cut;
pub mod map_tiles;
/// The dev MCP endpoint (2026-10-22): the running game serves the Model Context Protocol on
/// 127.0.0.1 so an AI session drives it as tools, each a thin call into the dev IPC above. Off
/// unless HUMANITY_DEV_MCP=1 in a portable sandbox. docs/ai/mcp.md.
pub mod mcp;
/// The home's air, water and power as the HUD reads them, for the recorder and the dev MCP.
pub mod home_probe;
/// Movie mode: record the live view to video, frame-exact, for the clip
/// maker (scripts/make-clips.js, 2026-09-30).
pub mod movie;
/// The play journal's frame half (2026-10-24): a mark's picture through the renderer, and what the
/// frame watcher sees each frame. The store is the core's crates/humanity-core/src/play_journal.rs.
pub mod play_journal_frame;
/// Leaving the app: the save every way out runs first (first-hour audit 2026-10-04, Blocker 5).
pub mod quit;
/// The character's own home (2026-10-04): the build editor's edits outside the Dev mode are kept
/// in the character's save, never in the shared data files; what a placement paid comes back.
pub mod own_home;
/// The Death setting's engine half (2026-10-04): a death surfaced, the pack left where the
/// player fell in Realistic, its prompt, E, drawing, marker and clock (systems::death_pack).
pub mod death_pack;
/// The world's systems held still and quiet until the person enters a world (BUG-253): the frame's
/// one tick of the systems and its notices (the rule: gui/world_entry.rs).
pub(crate) mod world_hold;
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
/// The frame's whole network pump: dialling, reading, reconnecting, WebRTC, chat history and the
/// saved servers' links (extracted from lib.rs 2026-10-31, so the hidden window's tick runs the
/// same one, notifications rung 2 part 2).
pub mod frame_net;
/// The notification-area (tray) icon: its menu, its muted and unread looks, its clicks, and a
/// click on an operating-system notice (notifications rung 2 part 2, docs/design/notifications.md
/// "The tray").
pub mod tray;
/// Closing to the tray: the hidden window that draws nothing and reads its sockets a few times a
/// second, showing it again, and the event loop's wake-up and outside-close hooks (rung 2 part 2).
pub mod tray_window;
/// The planet's two water shells (coarse backstop + displaced wave surface),
/// built and pushed once per frame (extracted from lib.rs, v0.1320).
pub mod frame_water;
/// Near-field 3D trees: the gated harvest, the per-tree draw plan, and the
/// card-hide promise the far LOD depends on (extracted from lib.rs, v0.1320).
pub mod frame_near_trees;
/// The rig's readout of the ground under the near trees and the eye, against
/// the ground actually drawn there (BUG-156, 2026-10-05).
pub mod tree_ground;
/// The rig's readout of the ground along a transect across a coast, against the coastline data
/// and the raw land layer (whole-Earth maps rung 1, 2026-11-09, the `shore_probe` gate).
pub mod shore_probe;
/// The rig's readout of a road's cross-section and a building's footing against the mesher's own
/// rules (whole-Earth maps, the rung between 2 and 3, 2026-11-09, the `bed_probe` gate).
pub mod bed_probe;
/// The rig's held movement keys: arriving somewhere on foot the way a player
/// does, instead of by teleport (BUG-156, 2026-10-05).
pub mod rig_walk;
/// What the dev IPC reports of the player's body: health, wounds and the landings the walk
/// handed to it, and the showcase `illness` verb's words (2026-10-20, scripts/verify-falls.js).
pub mod body_probe;
/// What the dev IPC reports of the walk between storeys: the floor stood on, the ceiling, the
/// room the HUD names, the ladder and the elevator cars, and the showcase `face` verb (ship homes
/// increment 7, 2026-10-21, scripts/verify-storeys.js).
pub mod storey_probe;
/// A planet's cloud deck and atmosphere dome, plus the view-dependent order
/// the two composite in (extracted from lib.rs, v0.1320).
pub mod frame_shells;
/// The whole-Earth map packs in the world (rung 2): the ground published into the one formula,
/// tile residency riding the patches, and the OSM districts standing on it (was region_meshes).
pub mod map_meshes;
/// Room GI rung 1 (2026-09-27): the ship's rooms into probe boxes, and the
/// per-frame hook before the scene pass. The probes live in renderer::room_probes*.
pub mod room_gi;
/// Settings > Graphics > Furniture detail (2026-11-09), the engine's half: the per-frame rebuild of
/// the home's bodies when the level or the model packs change (the levels are gui::furniture_detail).
pub mod furniture_detail;
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
/// How fast a person walks and runs, by Settings' walking pace (Realistic 1.4 and 4 m/s,
/// Simplified 5 and 9.5; data/game.csv), 2026-11-09.
pub mod walk_pace;
/// The floor under a body aboard: its room's, raised to the stair tread, deck or elevator car it
/// can step onto. Asked before the walk and between a long frame's substeps (BUG-207).
pub mod footing;
/// The mining drone's engine half (first-hour audit S5, 2026-10-12): its Settings mode,
/// the Mining panel's Repair and Cancel, and its battery, wear and service onto the panel.
pub mod drone;
pub mod net_route;
pub mod registries;
/// In-world screens: native pages on flat displays placed in the 3D world
/// (quads, look-ray hit tests, input routing, per-frame surface drawing).
pub mod screens;
// The Garden data the Inventory page shows, built only on a frame that draws it (2026-10-30).
pub mod garden_mirror;
/// Settings > Gameplay's garden and ship levels (garden pests, pollination, soil pH, picking,
/// ship life support; data/gameplay/levels.json) into their systems, once a frame (2026-11-12).
pub mod garden_levels;
pub mod state;
/// Visible storage: crates in the storage zones and tank level bars.
pub mod stock_piles;
pub mod world_load;

// Tests of the crates below that reach the engine to do it, moved up from beside their code (the crate
// split, 2026-10-31, docs/design/crate-split.md): src/engine/crossing_tests/mod.rs.
#[cfg(test)]
mod crossing_tests;
