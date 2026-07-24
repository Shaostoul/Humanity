# Code structure plan (2026-07-24 Fable assessment)

The operator asked: should we refactor the file/folder structure, names, and
monolithic files, and can the monoliths be broken down to be modular per the
infinite-of-x rule? This is the assessment and the staged plan. Line numbers
are as of v0.930.1; they will drift, but the cluster names will not.

## Verdict in one paragraph

Folder structure and naming are healthy (audience-first docs/, domain-first
src/, consistent snake_case; no renames worth the churn). The real debt is
concentrated in a handful of monoliths, and one of them dominates: src/lib.rs
at ~22,600 lines. Its true event/frame skeleton is only a few hundred lines;
the rest is inlined subsystem logic that reads/writes EngineState and can move
out in stages with mostly `use` changes. Separately, an infinite-of-x scan
found 11 real cases of domain content trapped in code (one of them a live
correctness bug, the reaction palette). Both are incremental work, no
big-bang rewrite is needed or wanted.

## The monolith league table (v0.930.1)

| File | Lines | Verdict |
|---|---|---|
| src/lib.rs | 22,638 | THE target. Staged extraction below. |
| src/gui/pages/chat.rs | 7,757 | Large but page-shaped; split only if it grows past ~10k. |
| src/gui/mod.rs | 6,740 | Movable clusters identified below. |
| src/relay/relay.rs | 6,146 | WS routing; cohesive; leave until a protocol arc touches it. |
| src/relay/handlers/msg_handlers.rs | 4,638 | Same. |
| src/gui/pages/construction.rs | 4,073 | Page-shaped, fine. |
| src/relay/api.rs | 4,038 | REST handlers, fine. |
| assets/shaders/pbr_simple.wgsl | 3,412 | The megashader. Keep one file (hot-reload + shared bindings); structure with section banners instead. |

Everything else is under 3.5k and domain-shaped. Web JS is healthy (largest
real file ~2.4k).

## lib.rs staged extraction (safest first) - COMPLETE 2026-07-25 (v0.932-v0.941): tiers A-G all shipped; lib.rs 22,638 -> 14,937 (7,701 lines across 14 src/engine/ modules). What remains in lib.rs by design: module decls, init (resumed), the frame loop dispatch, and the scene-assembly block (which owns wgpu surface/encoder lifetimes). Line numbers below are historical.

Shape today: `#[cfg(feature = "native")] mod native_app` spans nearly the whole
file. EngineState (~100+ fields, def at ~7935-8529) is the god-struct; almost
every helper takes `&mut EngineState`. Extraction is mechanical but requires
EngineState (and a few local enums) to become pub(crate) in a shared module.

- **Tier A (zero risk, pure fns + their tests):** ray/geometry math
  (ray_aabb_hit, ray_ring_closest, snap_*), color (hsv_*), parsers
  (parse_screenshot_request, planet_tooltip_info, screenshot_* helpers).
  -> src/engine/geom.rs, src/engine/color.rs, src/engine/ipc_parse.rs
- **Tier B (DataStore/hecs only, no EngineState):** load_data_registries
  (~200 lines), home spawn helpers (spawn_home_*), decrypt_dm_if_encrypted.
  -> src/engine/registries.rs, src/engine/home_spawn.rs
- **Tier C (IPC pollers, native-gated):** poll_screenshot/showcase/camera/
  autopilot_request + capture helpers + pump_live_broadcast.
  -> src/engine/ipc/
- **Tier D (frame-lock + celestial math, read-mostly):** ground_radius_m,
  current_planet_spin, ground_anchor, godray_scale, sun_occlusion_factor,
  bookmarks, SURFACE_*/CO_ROTATE_*/INERTIAL_* consts. -> src/engine/frame_lock.rs
- **Tier E (editor pick/grab/drag, ~2,050 lines, cohesive):** try_pick_*/
  try_grab_*/apply_*_drag families + construction history. -> src/engine/editor/
- **Tier F (mesh/scene rebuild, ~2,000 lines):** rebuild_hull/homestead/
  machine_objects/plant_meshes/door_panels, home_lights. -> src/engine/home_meshes.rs
- **Tier G (hot, most entangled):** route_game_message, load_world (~1,030
  lines). Last.

Not extractable as units: `resumed` init and the ~7,500-line scene-assembly
block inside RedrawRequested (they own wgpu surface/encoder lifetimes); they
get THINNED by calling the extracted helpers instead.

Per-stage verify bar: `cargo check --features native` AND
`--features relay --no-default-features`, `cargo test --features native --lib`,
boot the release exe (0 PANICs), `just perf-sweep` unchanged, ship as its own
minor release. One tier (or half a tier) per release; never mix with feature
work.

## gui/mod.rs movable clusters

1. Relay-mapped model types (GuiListing/GuiReview/GuiCivStats/GuiTrade/
   GuiGuild + from_relay_json + tests, ~940 lines) -> src/gui/relay_model.rs
2. Data loaders (load_tools_catalog ... load_default_task_projects, ~1,390
   lines) -> src/gui/loaders.rs (or fold into their page files)
3. StudioState + presets -> src/gui/studio_model.rs
4. Chat model types -> src/gui/chat_model.rs
5. SettingsState block -> src/gui/settings_state.rs

GuiPage enum + GuiState stay in mod.rs (every page references them).
Splitting GuiState's ~2,000-line field list into per-domain sub-structs is a
separate, later decision.

## Infinite-of-x migration queue (from the 2026-07-24 scan)

Ranked by trapped content; each is its own small release. Borderline cases
judged NOT violations are listed in the scan output and should not be
re-litigated (UI action menus, picker palettes, security allowlists, fallback
defaults, generated JS, test fixtures).

1. **Reaction palette, LIVE BUG:** three divergent lists (native chat.rs ~45,
   relay ALLOWED_REACTIONS 8, web chat-messages.js 8 with a different set).
   Native reactions beyond the relay's 8 are silently rejected.
   -> data/reactions.json consumed by all three. (DONE 2026-07-24)
2. Ship NPC crew roster + dialogue (relay game_state.rs, 6 NPCs x ~6 lines)
   -> data/npc/crew.ron (chores already load from data, follow that pattern)
3. Ship room equipment catalog (game_state.rs, 6 room types -> ~27 items)
   -> data/ships/room_equipment.ron
4. Wallet guide sections (web wallet-guide-app.js, ~9 prose sections)
   -> data/wallet/guide.json
5. Web keybinding registry (settings-app.js KEYBIND_DEFS ~30; native already
   loads data/keymaps.ron) -> shared data file for both
6. Fibonacci scope taxonomy (tasks-app.js SCOPES, 10) -> data/tasks/scopes.json
7. Crypto glossary (wallet-guide-app.js, 18 terms) -> merge into data/glossary.json
8. Onboarding tour steps (onboarding-tour.js, 7) -> data/onboarding/tour.json
9. XP curve duplicated across web pages -> data/skills/xp_curve.json
10. OPENING_STYLES (construction.rs, 8 door/window styles; self-described as
    growable) -> data/blueprints/opening_styles.ron (DONE 2026-07-24)
11. Wardrobe cosmetic slots (showroom.rs) -> reuse data/inventory/equipment_slots.json

Flagged for a 30-second human look, not asserted: ground_textures.rs FILES
manifest (8 splat-layer filenames; reads as a fixed asset manifest).

## Naming / folder answer

- Folders: keep. src/ is domain-first and matches CLAUDE.md's map; docs/ is
  audience-first post-v0.422; web mirrors native per design.
- Names: consistent; no rename sweep. pbr_simple.wgsl understates what it is,
  but renaming would churn loader paths + hot-reload for zero behavior gain.
- New homes created by this plan: src/engine/ (lib.rs satellites), and the
  gui model/loader files above.
