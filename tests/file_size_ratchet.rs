//! File-size RATCHET (2026-07-31, operator: "the larger a file becomes the
//! less likely an AI is to successfully read and modify it").
//!
//! The v0.932-v0.941 extraction arc took lib.rs from 22,638 to 14,937 lines
//! and shipped NO guard, so 140 releases later it had regrown to 17,677:
//! every feature wires into the frame loop because that is where the loop
//! lives, and nothing pushed back. Extraction without a ratchet is a
//! subscription, not a purchase.
//!
//! Mechanism: each monolith has a checked-in line BUDGET. Growing past the
//! budget fails this test with instructions. Shrinking a file lets you (and
//! the failure message asks you to) lower its budget so it can never regrow.
//! Budgets only go DOWN over time; adding a new file to the watch list is
//! normal work when a new monolith emerges.
//!
//! The budgets are the measured size at ratchet installation plus ~3% slack,
//! so normal in-place editing never trips it; only sustained accretion does.
//! When you trip it: move the code you are adding into the module it belongs
//! to (src/engine/ for loop code, a page's own file, a relay handler), or
//! extract a coherent cluster first. docs/dev/code-structure-plan.md has the
//! tier taxonomy that worked last time.
//!
//! Std-only, compiled standalone like the other lints (no native bin link):
//!   CARGO_MANIFEST_DIR=<repo> rustc --test --edition 2021 tests/file_size_ratchet.rs

use std::fs;
use std::path::Path;

fn repo() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// (path, line budget). Measured 2026-07-31 + ~3% slack. LOWER these as files
/// shrink; never raise one without an operator decision recorded in the
/// commit message.
///
/// RATCHET CLICKS (newest first):
/// - 2026-09-19: THE TWO GUI FILES. `gui/pages/chat.rs` 8_000 -> 4_750 and
///   `gui/mod.rs` 7_050 -> 4_800, together the largest click this list has
///   taken. Both were red (9_053, +1_053; 7_915, +865) and, for the third
///   session running, that meant `just verify` could not run AT ALL: `just
///   lints` aborts at the first failure, so everything behind the ratchet was
///   being silently skipped. Six clusters came out, each its own commit, each
///   proven to be pure motion by accounting every removed line against the file
///   it landed in.
///   OUT OF `chat.rs` (9_053 -> 4_616), into `pages/chat/`, which is the plain
///   2018 directory form: `chat.rs` stays the page and its children live beside
///   it, so the ratchet path is unchanged and nothing outside the page moved.
///     * `left_panel.rs` (2_003) - the whole left rail: connect box, scratchpad,
///       DMs, Groups, Commons, Servers and the expanded server's channel list.
///       One cluster because picking a row in ANY of those sections is the
///       page's one way of setting `chat_active_channel`.
///     * `modals.rs` (1_470) - the eleven overlays. One cluster by SHAPE, not
///       subject: each appears on a `show_*` flag, takes the pointer, does one
///       job and closes itself, and `draw` calls them in a single block.
///     * `p2p_groups.rs` (721) - everything done TO a group that is not drawing
///       it (list, load, decrypt, post, invite, leave, disband, peer objects).
///       Drawing a group is ordinary chat drawing and stayed.
///     * `right_panel.rs` (466) - Friends, Members, and the live strip: the same
///       thing drawn twice, both through one `draw_user_row`.
///   OUT OF `gui/mod.rs` (7_915 -> 4_660):
///     * `gui/loaders.rs` (1_566) - every `data/` file the GUI reads and the
///       shapes it reads them into. One job, one input, one failure mode.
///     * `gui/state_types.rs` (1_791) - the value types pages are drawn from
///       (item slot, task, listing, chat message, channel, studio scene, ...)
///       plus their `from_relay_json` mappers.
///   THE PRIVACY DELTA of the whole click is sixteen `pub(super)`s in the chat
///   children (5 + 1 + 2 + 8, in the order the files are listed above), each
///   commented where it is declared as private-only-because-its-caller-stayed.
///   The two `gui/` children needed ZERO: every item in them was
///   already `pub`, so `pub use loaders::*` / `pub use state_types::*` keeps
///   every `crate::gui::NAME` spelling in the crate resolving untouched. All six
///   children take `use super::*` the way `tree_species.rs` does. A glob
///   re-export in the parent plus a glob import in the child is a CYCLE and it
///   resolves fine, because the names collide on the same item.
///   THREE SCANS HAD TO FOLLOW THE CODE, which is the v0.1320 lesson repeating:
///   `page_parity_lint`'s three native lists gained `src/gui/loaders.rs`
///   (Library's only native mention of `library/index.json` was `load_library`,
///   so the move broke it honestly); `theme_token_lint` lost its
///   `src/gui/mod.rs` entry because the file's last unexempted literal moved out
///   and its `theme-exempt` note, which had been sitting one line ABOVE the
///   literal and exempting nothing, moved onto the literal's own line; and
///   `page_registry_lint` reads the `GuiPage` enum out of `src/gui/mod.rs` by
///   hand, which is WHY that enum and its three config-string helpers were left
///   behind rather than travelling with the other value types.
///   ONE THING COULD NOT MOVE: `default_water_clarity`, `default_precip_density`
///   and `default_fog_density` sit inside the loaders run but are serde
///   `default = "..."` targets for `SettingsState`. A serde default resolves as
///   a path in the scope of the struct that names it, so moving them meant
///   rewriting attributes, which is a change and not a motion. They stay, with a
///   comment saying so.
///   THE NAMED NEXT EXTRACTION for each file, so the next author does not have
///   to go looking:
///     - `chat.rs`: `draw_center_panel` (1_610 lines, over a third of what is
///       left) into `chat/center_panel.rs` - the channel header, the message
///       feed and the composer. `draw_ingame_chat` (471) is the same surface
///       drawn for the in-world monitor and could go with it or after it.
///     - `gui/mod.rs`: `GuiState` (2_305), `impl GuiState` and
///       `impl Default for GuiState` (910) into `gui/state.rs` - half of what
///       is left, and one thing. MIND THE GATE: `src/engine/input.rs` scans
///       `src/gui/mod.rs`'s SOURCE TEXT for every `cloud_dev_*` field and
///       asserts it finds more than 40, so that scan must be pointed at the new
///       file in the SAME commit or it reports a wiring break that is really a
///       file move.
///   Eight watch entries added, at measured plus roughly 5% (more for the small
///   ones, for the v0.1093 reason: 3% of a 466-line file is 14 lines, which
///   would fire on the first honest addition).
/// - 2026-09-19: SIX RED ENTRIES GO GREEN. Two remain, and they are not
///   orphans: `gui/pages/chat.rs` (9_053 of 8_000) and `gui/mod.rs` (7_915 of
///   7_050) were being extracted by another session while this one ran, so
///   they were deliberately left alone rather than edited underneath it. Every
///   OTHER entry on this list is green, with room.
///   The two sessions before this one cleared the two big offenders (lib.rs
///   19_892 -> 16_211, renderer/mod.rs 5_652 -> 2_742); what was left was six
///   modest overruns, +58 to +280, and the reason to care is the one the
///   v0.1320 note gives: `just lints` aborts at the first failure, so ONE red
///   entry switches off every lint behind it. A small overrun is not a small
///   problem.
///   SIX extractions, each its own commit, each a coherent cluster rather than
///   a slice at a line number:
///     * `relay/api_market.rs` (705) - the MARKETPLACE. api.rs is one route
///       table's worth of handlers for about twenty unrelated features, and
///       trading was three banners (Marketplace, Reviews, Order Book) that are
///       plainly one feature: a listing has images, a listing has reviews, a
///       seller's rating summarises them, and the order book is the same goods
///       offered standing. 4_480 -> 3_819.
///     * `relay/handlers/stream.rs` (443) - LIVESTREAMING: eleven handlers, a
///       WebRTC signalling triangle that only makes sense as a set, and their
///       own test module, which travelled with them. 4_936 -> 4_530.
///     * `terrain/water_patches.rs` (323) - THE SEA'S OWN PATCH MESHES.
///       planet_chunks is the quadtree; the water shell borrows all of it and
///       then builds something else (undisplaced, ocean-masked, no
///       vegetation). 4_878 -> 4_607.
///     * `terrain/grass_fields.rs` (238) - THE SWARD'S PURE FIELDS. grass.rs
///       already said in its own header that the layer is "three pure
///       functions plus a harvest"; this is the pure functions. 2_705 -> 2_564.
///     * `renderer/material_bind_groups.rs` (232) - THE GROUP 3 ENTRY LISTS.
///       materials.rs registers and rewrites material SLOTS; this writes the
///       sixteen-binding bind groups those slots carry, which was where all the
///       length was. The v0.1029-v0.1038 rule (touch the layout and EVERY
///       creation site must carry EVERY binding) travelled with the code rather
///       than being left behind pointing at functions no longer under it.
///       602 -> 407.
///     * `renderer/view_depth.rs` (124) - WHICH DEPTH TEXTURE IS CURRENT, the
///       window's or an off-screen view's. It was in capture.rs and was never
///       capture: it is what an off-screen render needs BEFORE any pixels
///       exist, and its other caller (the in-world camera screens) captures
///       nothing. 367 -> 277.
///   Five of the six are `#[path]` CHILD modules with a `pub use` in the
///   parent, which is now the house pattern for this repo (`grass_mesh`,
///   `tree_species`, `near_trees` set it): the child sees the parent's private
///   items through one `use super::*`, and every existing path
///   (`chunks::water_band`, `api::get_listings`, `grass::grass_density_at`)
///   keeps resolving, so no call site and no module file needed an edit.
///   `view_depth` is a sibling instead, because `renderer/mod.rs` names the
///   `ViewDepth` enum as a field type and a child's `pub(super)` would not
///   reach it.
///   THE ONE THING THAT WAS NOT PURE MOTION, and it is the v0.1320 lesson
///   arriving on schedule in a second place: TWO gates prove a thing by READING
///   `src/terrain/grass.rs`'s source text (the quality-slider scan inside
///   `near_grass_density_matches_a_real_sward`, and the Settings page's
///   `engine_source`). Both named exactly one file, so the grass extraction
///   would have made each go quietly blind to half of what it guards while
///   still passing. They now share one list, `grass::coverage_path_files()`,
///   exactly as the frame loop's three scans share
///   `near_trees::frame_loop_source()`. ADD ANY FILE YOU EXTRACT FROM THE
///   COVERAGE PATH TO THAT LIST. It was proved RED before being trusted: a
///   planted `grass_detail()` call in grass_fields.rs fails the gate naming
///   `src/terrain/grass_fields.rs:241`.
///   `materials.rs` (408 of 500) and `capture.rs` (277 of 300) keep their
///   budgets rather than ratcheting to measured+3%: both are already at the
///   small-file FLOOR the v0.1093 note argues for, where 3% is a handful of
///   lines and a click would fire on the first honest addition.
///   The six new files join at measured + ~40%, not +3%, for that same reason.
///   IF YOU NEED ROOM NEXT: in `api.rs` the Admin Analytics block (~460 lines)
///   and the Guilds block (~280) are the next two coherent banners; in
///   `msg_handlers.rs` it is the GAME-STATE half (`handle_game_join` through
///   `persist_player_progress`, ~1_060 lines), which has a sibling file named
///   `handlers/game_state.rs` already waiting for it.
/// - v0.1319: `renderer/mod.rs` 3_883 -> 2_800, the biggest single click this
///   list has taken. The file had reached 5,652 lines (+1_769 over budget) and
///   was the second-worst of ELEVEN red entries, which mattered more than the
///   number says: `just lints` aborts at the first failure, so with this one
///   red the later lints in `just verify` never ran at all.
///   FOUR clusters came out, each its own commit, each proven to be pure
///   motion by accounting every removed line against the file it landed in:
///     * `renderer/celestial.rs` (1_887) - `render_celestial_onto` and the
///       `run_cloud_composite` it is the only caller of. 1_816 lines removed,
///       1_816 accounted for, not one signature changed. It stayed ONE piece
///       because the pass is a single ordered sequence whose steps depend on
///       each other; splitting it would have made this a refactor.
///     * `renderer/scene_draw.rs` (627) - the near-world draw loops, from
///       `upload_object_uniforms` through the opaque, transparent and overlay
///       lists to god rays and SSAO, plus the whole-frame wrappers.
///     * `renderer/overlay_draw.rs` (390) - the post-passes: orbit lines
///       (both far planes) and particle billboards, CPU and GPU.
///     * `renderer/surface.rs` (260) - the swapchain and render-target
///       lifecycle, including BUG-077's deferred present-mode change and its
///       two pure helpers with their unit tests.
///   The whole privacy delta of the move is FOUR `pub(super)`s
///   (`create_scene_texture`, `create_depth_texture`,
///   `upload_object_uniforms`, `draw_opaque_objects`), each of which was
///   private only because its caller used to sit in the same file; every one
///   is commented as such. The three big files take `use super::*` the way
///   `tree_species.rs` does - a child module sees its parent's private items
///   AND its private `use` bindings - so no `Renderer` FIELD was widened at
///   all. No re-export shim was needed for any method (inherent methods
///   resolve by receiver type, not module path); the only `pub use` is for the
///   two free VSync helpers, so `renderer::vsync_present_mode` still resolves.
///   5_652 -> 2_720, so 1_083 lines are banked and 80 stay as working room.
///   The four new files join the watch list at more than measured+3%, for the
///   reason the v0.1093 note gives: 3% of a 260-line file is 8 lines, which
///   would fire on the first honest addition.
///   WHAT IS LEFT IN mod.rs is now one coherent thing - the `Renderer` struct,
///   its ~1,030-line `init`, and the registries and per-frame setters
///   (meshes, patch arena, grass, lights, weather map, atmosphere LUTs). The
///   next increment that wants room should extract `init` itself into
///   `renderer/init.rs`; it is a single function with one job (build every
///   GPU resource the struct holds) and it is over a third of what remains.
/// - v0.1320: `src/lib.rs` 18_100 -> 16_700, THE BIGGEST CLICK SINCE THE
///   RATCHET WAS INSTALLED, and the first one where the gate had been red
///   long enough to do real damage. lib.rs stood at 19_892 (+1_792), and
///   because `just lints` stops at the first failure, `just verify` could not
///   run AT ALL: `focus_optin_lint`, `account_sql_lint` and `rig_pin_lint`
///   were being silently skipped, so the project's standard pre-push gate was
///   effectively off. Lesson worth keeping: a red ratchet is not a local
///   nuisance, it disables every lint behind it.
///   FOUR extractions, all pure code motion, each its own commit so a bisect
///   can name one:
///     * `engine/frame_ws_poll.rs` (1_660) - the relay WebSocket message pump
///       and the socket-died teardown. One job, one input (`&mut EngineState`),
///       and it grows every time the protocol does.
///     * `engine/frame_shells.rs` (1_012) - a planet's cloud deck, its
///       atmosphere dome, and the view-dependent order the two composite in.
///     * `engine/frame_water.rs` (549) - the sea's two quadtree shells.
///     * `engine/frame_near_trees.rs` (459) - the near-tree harvest, the draw
///       plan, and the card-hide promise.
///   The three celestial ones could NOT take `&mut EngineState`: the caller
///   holds `def` borrowed out of `state.planet_defs` for the whole body loop.
///   They take context structs that name each borrowed field instead, which
///   is `ensure_near_tree_models`'s rule with the fields grouped - and because
///   the struct is named `state`, the moved bodies are byte-identical to the
///   lines they replaced apart from a handful of `*` the compiler demands.
///   That is what let "zero behaviour change" be checked rather than asserted.
///   19_892 -> 16_211, so 1_400 of the 3_681 lines are banked and 489 stay as
///   working room.
///   THE FILE HAS NO NAMED NEXT EXTRACTION; whoever next needs room should
///   name one rather than nibble. The obvious candidate if nothing better
///   presents itself is the ECS-to-GuiState BRIDGE REGION (roughly 2_300 lines
///   of "publish the world into the panels", from the `Bridge ECS/DataStore
///   state into GuiState` banner to the auto-connect block), which reads only
///   `state`, `dt` and `showroom` and would therefore be the easiest move on
///   this list.
///   Four new watch entries, at measured + ~5% rather than the usual 3%,
///   because a brand-new file needs ordinary editing room before its first
///   click: `frame_ws_poll` 1_800, `frame_shells` 1_300, `frame_water` 750,
///   `frame_near_trees` 700. If `frame_ws_poll` is the first to trip, the
///   extraction to make is BY DOMAIN - the voice/room arms, or the game-state
///   arms, are each a coherent group of `match` arms.
///   ONE MORE LESSON, and it cost three test failures to learn: this repo has
///   several gates that read the frame loop's SOURCE TEXT to prove a helper is
///   actually called (`the_frame_loop_uses_the_measured_coverage_radius`,
///   `the_frame_loop_sources_the_density_once_for_both_streams`,
///   `the_page_tells_the_truth_about_what_the_engine_reads`). All three read
///   `src/lib.rs` and only `src/lib.rs`, so every extraction makes them report
///   a wiring break that is really a file move. They now share one list,
///   `terrain::near_trees::frame_loop_source()`. ADD ANY FILE YOU EXTRACT
///   FROM THE FRAME LOOP TO THAT LIST in the same commit.
/// - v0.1110: `renderer/tree_mesh.rs` 6_450 -> 6_400, and the gate WORKED
///   exactly as designed for the second time in seven releases. The cube fix
///   (golden-angle azimuths + solved per-card tilts, with its own gate) added
///   306 lines and put the file at 6_723 (+273). Rather than raise the number,
///   the FOLIAGE CARD ARRANGEMENT moved out verbatim to
///   `renderer/tree_cards.rs`: `emit_card`, `emit_sleeve`, `sleeve_tilts`,
///   `PHYLLOTAXIS_RAD`, `CLUSTER_SLEEVE_OFFSET` and the box gate - 417 lines,
///   one job ("where does a card go and which way does it face"). 6_723 ->
///   6_351, so 50 of the 372 lines are banked and the rest is working room.
///   `emit_cluster_cards` deliberately stayed behind: it is the PLANNER (LAI
///   fit, layer assignment, budget) and belongs with the kernel. Like
///   `tree_species` it is a `#[path]` CHILD module, so the vector helpers,
///   `PlantMeshBuilder` and the normal-blend constants arrive through one
///   `use super::*` instead of a dozen new `pub(crate)`s.
/// - v0.1108: `src/lib.rs` 18_300 -> 18_100, because THE OWED EXTRACTION
///   FINALLY LANDED. Three increments in a row trimmed comments instead; this
///   one moved the near-tree MODEL LOADER (glTF parse, scan-stretch guard,
///   procedural fallback and their material registration - 418 lines,
///   verbatim) to `src/engine/near_tree_models.rs`. Note it went to `engine/`
///   rather than the `terrain/` the older note guessed: it needs the wgpu
///   renderer and the asset manager, both native-only, while `terrain/` is
///   ungated and compiles into the relay build. 18_300 -> 17_932, so 200 of
///   the 368 lines are banked and 168 stay as working room. THE FILE NO LONGER
///   HAS A NAMED NEXT EXTRACTION - whoever next needs room should name one
///   rather than nibble. New watch entry `src/surface_walk.rs` at 1_150: it
///   went 450 -> 1_066 landing the drawn-ground stand, about 620 of that being
///   its gate suite, which is load-bearing and NOT the thing to extract.
/// - v0.1103 (lib.rs, no budget change): the gate FIRED at 18_333 (+33) and
///   was paid back honestly rather than raised. Two ~30-line INCIDENT
///   NARRATIVES had accreted in comments (the BUG-063 sapling-scan story and
///   the BUG-066 mis-scoped-timer story), both already written up properly in
///   docs/BUGS.md. Source comments should say what the code does and why it is
///   shaped that way, and point at the bug log for the story; duplicating the
///   narrative is exactly the accretion this ratchet exists to catch. Trimmed
///   to pointers: 18_333 -> 18_298.
///   THE REAL EXTRACTION IS STILL OWED, and this is the second increment in a
///   row to nibble instead: the NEAR-TREE MODEL LOADER (the glTF parse, the
///   scan-stretch guard, the procedural fallback and their material
///   registration, ~250 lines around the `for name in &tree_names` loop) is a
///   coherent cluster with one job. The next increment that wants room in
///   lib.rs should move it to `src/terrain/near_tree_models.rs` rather than
///   find another comment to shorten.
/// - v0.1103: the extraction the v0.1100 note below ASKED FOR landed, and the
///   ratchet is why. A card-form increment took tree_mesh.rs to 6_531 (+81 over
///   budget), and instead of raising the number the author moved the 678 lines
///   of species architecture - `limb` and the four crown builders - into
///   `renderer/tree_species.rs`, leaving tree_mesh.rs as the geometry kernel
///   (tubes, rings, junctions, welds, flare) plus its gates. 6_531 -> 6_343,
///   back under budget with the new content still in. The new file joins at
///   750 (measured 721 + slack). Note it is a `#[path]` CHILD module of
///   tree_mesh, not a sibling: a sibling would have needed `pub(crate)` on
///   dozens of kernel internals, where a child sees them through one
///   `use super::*` - and that is also why `renderer/mod.rs` was untouched.
/// - v0.1100: new watch-list entry `renderer/tree_mesh.rs` at 6_450. It is
///   6_267 lines today and was NOT on the list while the whole junction saga
///   (v0.1096-v0.1100: surface roots, collars, the pipe model, directional
///   flare, continuous bark UVs) grew it - exactly the blind spot this
///   ratchet exists to close, caught only because a lint pass noticed the
///   file was bigger than several watched ones. Roughly a third of it is the
///   gate suite, which is load-bearing and should NOT be the thing extracted.
///   The next increment that wants room should move the SPECIES ARCHITECTURE
///   (the conifer/umbrella/palm/broadleaf crown builders and their constants)
///   into `renderer/tree_species.rs`, leaving tree_mesh.rs as the geometry
///   kernel - tubes, rings, junctions, welds, flare - plus its gates.
/// - v0.1097: `terrain/grass.rs` 2_470 -> 2_645, and a new watch-list entry
///   `terrain/drawn_surface.rs` at 1_100. THE ONE NUMBER HERE THAT GOES UP,
///   with the reason, because the rule above says never to raise one quietly:
///   grass.rs was 2_401 lines when v0.1092 put it on the list at 2_470, and
///   v0.1093's filler-stubble class took it to 3_022 WITHOUT touching the
///   budget - so this gate has been failing (+552) since that release, and
///   nobody noticed because it is an integration test rather than part of
///   `cargo test --lib`. This increment paid 461 of those lines back by
///   promoting `DrawnPatchSurface` out (trees needed it too, so it is no
///   longer grass's private sampler), leaving 2_568. 2_645 is that measurement
///   plus the standard ~3%: still 377 lines BELOW where the file actually sits
///   today at HEAD, so the budget resumes ratcheting from a real number
///   instead of staying red and being ignored. The follow-up that pays the
///   rest back is the filler class's own extraction.
///   `terrain/planet_chunks.rs` stays at 4_820 and is now 8 lines under it -
///   which is the ratchet working as designed: the next increment that wants
///   room in that file should extract the NEAR-TREE HARVEST (`NearTree`,
///   `near_tree_instances`, `near_tree_instances_on_drawn` and the tree-ground
///   constants, ~215 lines) into `terrain/near_trees.rs`, exactly the way the
///   grass layer moved out in v0.1092.
/// - v0.1093: `renderer/mod.rs` 4_000 -> 3_883. It was the one file OVER
///   budget (4_090, +90). The material system moved verbatim to
///   `src/renderer/materials.rs`: registration, the textured variants, the
///   in-place updates, and the two bind-group builders they all funnel
///   through - 322 lines, no behaviour change (the whole delta is one `use`
///   path and one `pub(super)`). The new file joins the watch list at 500
///   rather than measured+3%: 3% of a 365-line file is 11 lines, which would
///   fire on the first legitimate new material method and send its author
///   hunting for an extraction inside a 365-line module. 500 still trips long
///   before `materials.rs` could become a second monolith, and stays inside
///   the 600-line slack window so it does not immediately demand a click.
/// - v0.1092: `planet_chunks` 5_950 -> 4_820. The v0.1091 grass-strand
///   increment raised the budget to 5_950 with the extraction of the layer
///   recorded IN THAT COMMIT as the immediate next increment; that extraction
///   is this one. The layer moved verbatim to `src/terrain/grass.rs`, which
///   joins the watch list at its own measured size, so the file it left
///   cannot quietly reabsorb it and the new file cannot quietly grow into a
///   second monolith.
const BUDGETS: &[(&str, usize)] = &[
    ("src/lib.rs", 16_700),
    ("src/engine/frame_ws_poll.rs", 1_800),
    ("src/engine/frame_shells.rs", 1_300),
    ("src/engine/frame_water.rs", 750),
    ("src/engine/frame_near_trees.rs", 700),
    ("src/surface_walk.rs", 1_150),
    ("src/gui/pages/chat.rs", 4_750),
    ("src/gui/pages/chat/left_panel.rs", 2_100),
    ("src/gui/pages/chat/modals.rs", 1_550),
    ("src/gui/pages/chat/p2p_groups.rs", 800),
    ("src/gui/pages/chat/right_panel.rs", 600),
    ("src/gui/mod.rs", 4_800),
    ("src/gui/state_types.rs", 1_880),
    ("src/gui/loaders.rs", 1_650),
    ("src/relay/relay.rs", 6_500),
    ("src/relay/handlers/msg_handlers.rs", 4_660),
    ("src/relay/handlers/stream.rs", 550),
    ("src/terrain/planet_chunks.rs", 4_740),
    ("src/terrain/water_patches.rs", 450),
    ("src/terrain/grass.rs", 2_640),
    ("src/terrain/grass_fields.rs", 350),
    ("src/terrain/grass_mesh.rs", 1_000),
    ("src/terrain/drawn_surface.rs", 1_100),
    ("src/gui/pages/construction.rs", 4_250),
    ("src/relay/api.rs", 3_930),
    ("src/relay/api_market.rs", 800),
    ("src/renderer/mod.rs", 2_800),
    ("src/renderer/celestial.rs", 1_950),
    ("src/renderer/scene_draw.rs", 750),
    ("src/renderer/overlay_draw.rs", 550),
    ("src/renderer/surface.rs", 500),
    ("src/renderer/materials.rs", 500),
    ("src/renderer/material_bind_groups.rs", 350),
    ("src/renderer/capture.rs", 300),
    ("src/renderer/view_depth.rs", 250),
    ("src/renderer/shadow_cutout.rs", 250),
    ("src/renderer/tree_mesh.rs", 6_400),
    ("src/renderer/tree_species.rs", 750),
    ("src/renderer/tree_cards.rs", 450),
    ("src/renderer/tree_allometry.rs", 560),
];

#[test]
fn monolith_budgets_are_not_exceeded() {
    let mut over = Vec::new();
    let mut slack = Vec::new();
    for (rel, budget) in BUDGETS {
        let path = repo().join(rel);
        let lines = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {rel}: {e}"))
            .lines()
            .count();
        if lines > *budget {
            over.push(format!(
                "  {rel}: {lines} lines > budget {budget} (+{})",
                lines - budget
            ));
        } else if budget - lines > 600 {
            // The file shrank well below its budget: ask for the ratchet click.
            slack.push(format!(
                "  {rel}: {lines} lines, budget {budget} -- LOWER the budget to \
                 about {} so the win cannot be silently spent",
                lines + (lines / 33)
            ));
        }
    }
    assert!(
        over.is_empty(),
        "\n\nMONOLITH GREW PAST ITS BUDGET:\n{}\n\n\
         Do not raise the budget. Move the new code into the module it belongs \
         to (src/engine/ for frame-loop code, the page's own file, a relay \
         handler), or extract a coherent cluster first -- the tier taxonomy in \
         docs/dev/code-structure-plan.md completed a 7,700-line extraction and \
         works. The whole point of this ratchet is that the v0.941 extraction \
         regrew 2,740 lines in 140 releases because nothing pushed back.\n",
        over.join("\n")
    );
    assert!(
        slack.is_empty(),
        "\n\nRATCHET CLICK AVAILABLE (files shrank; lock in the win):\n{}\n",
        slack.join("\n")
    );
}
