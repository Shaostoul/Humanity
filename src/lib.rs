//! HumanityOS Game Engine
//!
//! Custom engine built on wgpu, hecs, rapier3d, and kira.
//! Designed for multi-scale space simulation with hot-reloadable data files.
//!
//! Compiles to both native (desktop via Tauri/winit) and WASM (browser via WebGPU).
//! Feature flags: `native` (default) or `wasm`.

pub mod renderer;
pub mod ecs;
pub mod physics;
pub mod audio;
pub mod input;
pub mod assets;
pub mod embedded_data;
pub mod platform;
pub mod terrain;
pub mod ship;
// Data-driven machine layout for the 3D home (pure data, relay-safe). The renderer
// placement lives in load_world (native); the structs + RON loader compile everywhere.
pub mod machines;
pub mod showroom;
pub mod cosmetics;

/// Canonical Sol-system model (one `SolBody` set + one Kepler
/// propagator) shared by the Maps page, the FPS world spawn, and the
/// in-home holo-orrery. Engine-wide on purpose — NOT `#[cfg(native)]` —
/// so terrain / renderer / world placement read the same source of
/// truth instead of drifting per-view. See `src/cosmos.rs`.
pub mod cosmos;

#[cfg(feature = "relay")]
pub mod relay;

pub mod hot_reload;

/// The resolved data directory, set once at startup (from `find_data_dir`) so cached
/// loaders (laws, glossary, garden, homes) read the SAME CWD-independent path the main
/// app uses, instead of a bare relative "data" that only resolves when the process CWD
/// happens to contain data/. Falls back to "data" if never set (tests/snapshots, which
/// run from the repo root).
pub(crate) static DATA_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// The resolved data dir (or "data" if not yet set).
pub(crate) fn data_dir() -> std::path::PathBuf {
    DATA_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| std::path::PathBuf::from("data"))
}

pub mod systems;

#[cfg(feature = "native")]
pub mod persistence;
// save_load consumes persistence (the offline-home save is the local player's,
// not the relay's), so it carries the same native gate — an ungated save_load
// broke every relay/CI build from v0.381 to v0.414 (E0432 on persistence).
#[cfg(feature = "native")]
pub mod save_load;

#[cfg(feature = "native")]
pub mod gui;

#[cfg(feature = "native")]
pub mod mods;

#[cfg(feature = "native")]
pub mod net;

#[cfg(feature = "native")]
pub mod config;

// v0.278.0: optional silent / quick-PIN unlock paths on top of the
// passphrase-encrypted vault. Native-only — relay/wasm don't carry an
// interactive identity.
#[cfg(feature = "native")]
pub mod auto_unlock;

#[cfg(feature = "native")]
pub mod updater;

// Signed-release verification + the operator-side signing/keygen tooling (the
// supply-chain root of trust; audit 2026-06-12 CRITICAL fix). Self-gated with
// `#![cfg(feature = "native")]` at the top of the file.
pub mod release_update;

pub mod debug;

#[cfg(feature = "wasm")]
pub mod wasm_entry;

#[cfg(feature = "native")]
mod native_app {
    use glam::{Quat, Vec3};
    use crate::assets::AssetManager;
    use crate::ecs::GameWorld;
    use crate::ecs::components::{Controllable, Health, Name, Transform, Velocity};
    use crate::ecs::systems::SystemRunner;
    use crate::gui::{GuiGameTime, GuiItemSlot, GuiPage, GuiState, GuiWeather};
    use crate::gui::theme::Theme;
    use crate::gui::pages::{
        main_menu, escape_menu, settings, inventory, chat, hud, placeholder,
        tasks, profile, market, calculator, calendar, notes, civilization,
        wallet, crafting, guilds, trade, files, bugs, donate, tools, studio,
        onboarding, server_settings, identity, governance, recovery, testing,
        browser, category_overview, settings_pages, cosmos, real,
        platform, humanity, library, quests, homes,
    };
    use crate::gui::widgets::help_modal;
    use crate::hot_reload::HotReloadCoordinator;
    use crate::hot_reload::data_store::DataStore;
    use crate::input::InputState;
    use crate::renderer::camera::{Camera, CameraController};
    use crate::renderer::mesh::Mesh;
    use crate::renderer::{RenderObject, Renderer};
    use crate::systems::crafting::CraftingSystem;
    use crate::systems::farming::FarmingSystem;
    use crate::systems::food::FoodSystem;
    use crate::systems::mining::DroneSystem;
    use crate::systems::skills::{PlayerSkills, SkillRegistry, SkillSystem, SkillXPEvent};
    use crate::systems::quests::{QuestRegistry, QuestSystem, QuestTracker};
    use crate::systems::interaction::InteractionSystem;
    use crate::systems::inventory::{Inventory, InventorySystem, ItemRegistry};
    use crate::systems::inventory::containers::ContainerCompatibilitySystem;
    use crate::systems::player::PlayerControllerSystem;
    use crate::systems::time::{GameTime, TimeSystem};
    use crate::systems::weather::{Weather, WeatherSystem};
    use crate::terrain::planet::{PlanetDef, PlanetRenderer};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Instant;
    use winit::application::ApplicationHandler;
    use winit::event::{DeviceEvent, DeviceId, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, EventLoop};
    use winit::keyboard::{KeyCode, PhysicalKey};
    use winit::window::{Window, WindowId};

    /// Locate the data directory relative to the exe.
    /// Prefers a data/ with world/ subdirectory (full repo) over extracted-only data/.
    /// Helper: decrypt an encrypted DM content if we have the keys.
    /// Returns the decrypted plaintext, or the original content with a marker if decryption fails.
    fn decrypt_dm_if_encrypted(
        raw_content: &str,
        encrypted: bool,
        nonce: &str,
        peer_key: &str,
        gui_state: &GuiState,
    ) -> String {
        let _ = (nonce, peer_key); // full-PQ: envelope is self-contained; KEM needs no peer key
        if !encrypted {
            return raw_content.to_string();
        }
        // Full-PQ: decapsulate with OUR OWN Kyber768 secret (deterministic
        // from the BIP39 seed). The {v:1,r,s} dual-seal envelope means this
        // opens both received messages and our own from history, on any
        // device with the seed. No peer key needed (ML-KEM).
        let seed = match gui_state.private_key_bytes.as_ref() {
            Some(s) => s,
            None => return "[encrypted — unlock your identity to read]".to_string(),
        };
        let me = match crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed) {
            Ok(k) => k,
            Err(_) => return "[encrypted — key derivation failed]".to_string(),
        };
        match crate::net::dm_pq::open_envelope(&me, raw_content) {
            Ok(plain) => plain,
            Err(e) => {
                log::warn!("PQ DM decryption failed for {}: {}", peer_key, e);
                "[encrypted — decryption failed]".to_string()
            }
        }
    }

    /// Spawn ONLY the electrical-role ECS entities for the home's machines (no meshes),
    /// so SolarSystem + ElectricalSystem tick against the real home + publish a live
    /// PowerStatus even in MENU mode (the Home page reads it, instead of authored
    /// strings). load_world re-spawns these WITH meshes on Enter World after despawning
    /// every HomeMachine, so there is no double-spawn. Silent no-op if home.ron is absent.
    fn spawn_home_power_entities(world: &mut hecs::World, data_dir: &std::path::Path) {
        use crate::ecs::components::{Battery, HomeMachine, PowerConsumer, PowerGenerator, SolarPanel};
        use crate::machines::MachinePower;
        let path = data_dir.join("machines").join("home.ron");
        let Some(home) = crate::machines::MachineHome::load(&path) else {
            return;
        };
        let all = home.all_instances();
        for inst in &all {
            let Some(def) = home.catalog.get(&inst.machine) else {
                continue;
            };
            let Some(power) = &def.power else {
                continue;
            };
            match power {
                MachinePower::Solar { peak_watts } => {
                    world.spawn((
                        HomeMachine,
                        PowerGenerator { output_watts: *peak_watts, fuel_per_second: 0.0, active: true },
                        SolarPanel { peak_watts: *peak_watts },
                    ));
                }
                MachinePower::Generator { watts } => {
                    world.spawn((
                        HomeMachine,
                        PowerGenerator { output_watts: *watts, fuel_per_second: 0.0, active: true },
                    ));
                }
                MachinePower::Consumer { watts, priority } => {
                    world.spawn((
                        HomeMachine,
                        PowerConsumer { draw_watts: *watts, priority: *priority, enabled: true },
                    ));
                }
                MachinePower::Battery { capacity_wh, max_charge_w, max_discharge_w } => {
                    world.spawn((
                        HomeMachine,
                        Battery {
                            charge_wh: capacity_wh * 0.5,
                            capacity_wh: *capacity_wh,
                            max_charge_w: *max_charge_w,
                            max_discharge_w: *max_discharge_w,
                        },
                    ));
                }
            }
        }
    }

    fn find_data_dir() -> PathBuf {
        let exe = std::env::current_exe().unwrap_or_default();
        let exe_dir = exe.parent().unwrap_or(std::path::Path::new("."));

        // Helper: is this a "full" data dir (has world/ with solar_system.ron)?
        let is_full_data = |p: &PathBuf| -> bool {
            p.join("world").join("solar_system.ron").exists()
        };

        // Collect all candidate data dirs in priority order
        let mut candidates: Vec<PathBuf> = Vec::new();

        // 1. data/ next to exe
        let beside_exe = exe_dir.join("data");
        if beside_exe.exists() && beside_exe.is_dir() {
            candidates.push(beside_exe);
        }

        // 2. Walk up parents (handles target/release/ -> repo root)
        let mut dir = exe_dir.to_path_buf();
        for _ in 0..6 {
            if let Some(parent) = dir.parent() {
                let candidate = parent.join("data");
                if candidate.exists() && candidate.is_dir() && !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
                dir = parent.to_path_buf();
            } else {
                break;
            }
        }

        // 3. CWD/data/ (cargo run)
        let cwd_data = std::env::current_dir()
            .unwrap_or_default()
            .join("data");
        if cwd_data.exists() && cwd_data.is_dir() && !candidates.contains(&cwd_data) {
            candidates.push(cwd_data);
        }

        // A "repo" data dir = one whose parent holds Cargo.toml (the source tree).
        // Prefer it over a FROZEN extracted copy beside a build exe, so dev runs —
        // including `target/release/HumanityOS.exe` directly — always read LIVE data
        // edits instead of a stale `target/release/data` snapshot a prior run
        // extracted. (Distributed builds have no sibling Cargo.toml, so they fall
        // through to the beside-exe data as before.) This was a real footgun:
        // `extract_data_if_needed` writes embedded data beside the exe once and
        // never refreshes it, so a stale copy silently shadowed the repo's data.
        let is_repo_data = |p: &PathBuf| -> bool {
            p.parent()
                .map(|parent| parent.join("Cargo.toml").exists())
                .unwrap_or(false)
        };

        // 1st preference: a full repo data dir (live source-tree data).
        for c in &candidates {
            if is_full_data(c) && is_repo_data(c) {
                log::info!("Data directory (repo, full): {}", c.display());
                return c.clone();
            }
        }

        // 2nd: any "full" data dir (with world/ subdirectory) over extracted-only.
        for c in &candidates {
            if is_full_data(c) {
                log::info!("Data directory (full, with world/): {}", c.display());
                return c.clone();
            }
        }

        // Otherwise use first available
        if let Some(first) = candidates.first() {
            log::info!("Data directory (partial): {}", first.display());
            return first.clone();
        }

        log::warn!("No data directory found, using ./data");
        PathBuf::from("data")
    }

    /// Extract embedded data files to disk on first run.
    /// If the data directory already exists, this is a no-op.
    /// This enables modding: users can edit the extracted files.
    fn extract_data_if_needed() {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        let data_dir = exe_dir.join("data");
        if data_dir.exists() {
            return;
        }

        log::info!("First run: extracting game data to {:?}", data_dir);

        // All embedded files with their relative paths
        let files: &[&str] = &[
            "items.csv", "recipes.csv", "materials.csv", "components.csv",
            "plants.csv", "game.csv", "skills/skills.csv",
            "chemistry/elements.csv", "chemistry/alloys.csv",
            "chemistry/compounds.csv", "chemistry/gases.csv", "chemistry/toxins.csv",
            "asteroids/types.csv",
            "glossary.json", "solar_system/bodies.json", "solar-system.json",
            "tools/catalog.json", "cities.json", "coastlines.json",
            "constellations.json", "milky-way.json", "stars-catalog.json",
            "stars-nearby.json",
            "config.toml", "calendar.toml", "input.toml", "player.toml",
            "gui/theme.ron",
            "planets/earth.ron", "planets/mars.ron", "planets/moon.ron",
            "solar_system/earth.ron", "solar_system/mars.ron", "solar_system/sun.ron",
            "ships/bridge.ron", "ships/layout_medium.ron", "ships/reactor.ron",
            "ships/starter_fleet.ron",
            // NOTE: this hand-maintained list has drifted from the full data set
            // (e.g. status_effects.csv, containers/, food_system.ron are loaded at
            // runtime but not listed here) — distributed-build completeness is
            // tracked as a follow-up to derive this list from embedded_data's keys.
            // Dev runs are unaffected (find_data_dir prefers the live repo data).
            "quests/construction.ron", "quests/exploration.ron",
            "quests/farming.ron", "quests/tutorial.ron", "quests/getting_started.ron",
            "blueprints/basic.ron", "blueprints/construction.ron",
            "blueprints/habitat.ron", "blueprints/materials.ron",
            "blueprints/objects.ron",
            "entities/human/human_001.ron", "entities/plants/plant_001.ron",
            "entities/plants/tomato.ron", "entities/substrates/loam_basic.ron",
            "entities/substrates/substrate_001.ron",
            "plots/plot_001.ron",
            "world/solar_system.ron", "world/spawn.ron", "world/player.ron",
            "resources/fertilizer_basic.ron", "resources/water_clean.ron",
            "i18n/en.json", "i18n/es.json", "i18n/fr.json",
            "i18n/ja.json", "i18n/zh.json",
            "language/acronyms.json", "language/dictionary.json",
            "language/parts_of_speech.json",
        ];

        for relative_path in files {
            if let Some(content) = crate::embedded_data::get_embedded(relative_path) {
                let file_path = data_dir.join(relative_path);
                if let Some(parent) = file_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = std::fs::write(&file_path, content) {
                    log::warn!("Failed to extract {}: {e}", relative_path);
                }
            }
        }

        log::info!("Data extraction complete");
    }

    /// Run the engine standalone — opens a window, renders a test scene.
    /// Supports three camera modes (Tab to cycle, F to toggle FP/TP, M for orbit).
    pub fn run() {
        env_logger::init();
        let event_loop = EventLoop::new().expect("Failed to create event loop");
        let mut app = App::new();
        event_loop.run_app(&mut app).expect("Event loop error");
    }

    /// Place a blockman avatar standing on a podium at `base` (the podium floor position),
    /// built from the player's `Appearance` (v0.440). A rudimentary humanoid from boxes +
    /// a head sphere on a podium cylinder, drawn via the static placeholder path (cleared +
    /// re-added each load, so no duplication). The face/limbs use skin tone; later
    /// increments swap this for a skinned mesh with cosmetic slots.
    fn place_avatar(
        state: &mut EngineState,
        base: Vec3,
        app: &crate::ecs::components::Appearance,
        colors: &crate::cosmetics::OutfitColors,
    ) {
        let s = app.height_scale.clamp(0.5, 2.0);
        let skin = [app.skin_tone[0], app.skin_tone[1], app.skin_tone[2], 1.0];
        let rgba = |c: [f32; 3]| [c[0], c[1], c[2], 1.0];
        // Equipped cosmetics tint the matching slot; otherwise default body colors.
        let hair = colors.head.map(rgba).unwrap_or([app.hair_color[0], app.hair_color[1], app.hair_color[2], 1.0]);
        let shirt = colors.chest.map(rgba).unwrap_or([0.28, 0.38, 0.58, 1.0]);
        let pants = colors.legs.map(rgba).unwrap_or([0.22, 0.22, 0.28, 1.0]);
        let podium = [0.45, 0.47, 0.50, 1.0];
        // (w, h, d, color, x, y, z) box parts; y/positions scale with height.
        let podium_h = 0.15_f32;
        let leg_h = 0.85 * s;
        let torso_h = 0.62 * s;
        let head_r = 0.14 * s;
        let leg_base = podium_h;
        let torso_base = leg_base + leg_h;
        let head_cy = torso_base + torso_h + head_r;
        // Helper: push a box part at base + (x,y,z).
        let mut push_box = |st: &mut EngineState, w: f32, h: f32, d: f32, c: [f32; 4], x: f32, y: f32, z: f32| {
            let mi = st.renderer.add_mesh(Mesh::box_xyz(&st.renderer.device, w, h, d));
            let mat = st.renderer.add_material_typed(c, 0.1, 0.75, 0.0);
            st.placeholder_objects.push((mi, mat, base + Vec3::new(x, y, z)));
        };
        // Legs (pants), torso (shirt), arms (skin), at the body's standing pose.
        push_box(state, 0.16, leg_h, 0.22, pants, -0.12, leg_base, 0.0);
        push_box(state, 0.16, leg_h, 0.22, pants, 0.12, leg_base, 0.0);
        push_box(state, 0.46, torso_h, 0.26, shirt, 0.0, torso_base, 0.0);
        push_box(state, 0.13, torso_h, 0.16, skin, -0.30, torso_base, 0.0);
        push_box(state, 0.13, torso_h, 0.16, skin, 0.30, torso_base, 0.0);
        // Hair cap (a thin box sitting on the head).
        push_box(state, 0.30, 0.10 * s, 0.30, hair, 0.0, head_cy + head_r * 0.4, 0.0);
        // Podium cylinder (capped so the top is a visible disc, not an open tube).
        let pm = state.renderer.add_mesh(Mesh::cylinder_capped(&state.renderer.device, 0.5, podium_h, 24));
        let pmat = state.renderer.add_material_typed(podium, 0.3, 0.5, 0.0);
        state.placeholder_objects.push((pm, pmat, base));
        // Head sphere (center-origin; place its center directly).
        let hm = state.renderer.add_mesh(Mesh::sphere(&state.renderer.device, head_r, 12, 14));
        let hmat = state.renderer.add_material_typed(skin, 0.1, 0.7, 0.0);
        state.placeholder_objects.push((hm, hmat, base + Vec3::new(0.0, head_cy, 0.0)));
    }

    /// Open the character showroom in the given mode (1 = appearance / wetroom mirror,
    /// 2 = wardrobe / bedroom). Syncs the edit buffers from the live player, orbits the
    /// avatar, and frees the cursor. The avatar lives at the respawner; the home is hidden
    /// while the showroom is open, so it does not matter which room you opened it from.
    fn open_showroom(state: &mut EngineState, mode: u8) {
        let (name, app, outfit) = state
            .game_world
            .world
            .query::<(
                &crate::ecs::components::Name,
                &crate::ecs::components::Appearance,
                &crate::ecs::components::Outfit,
                &Controllable,
            )>()
            .iter()
            .next()
            .map(|(_, (n, a, o, _))| (n.0.clone(), a.clone(), o.clone()))
            .unwrap_or_else(|| ("Wanderer".to_string(), Default::default(), Default::default()));
        state.gui_state.character_name = name;
        state.gui_state.appearance = app.clone();
        state.gui_state.outfit = outfit;
        state.gui_state.showroom_mode = mode;
        state.gui_state.showroom_active = true;
        state.gui_state.appearance_dirty = true; // rebuild the avatar to match the player
        state.gui_state.outfit_dirty = true;
        state.showroom_return_pos = state.camera.position;
        state.camera.switch_mode(crate::renderer::camera::CameraMode::Orbit);
        state.camera.orbit_target = state.avatar_base + Vec3::new(0.0, 0.9 * app.height_scale, 0.0);
        state.camera.orbit_distance = 3.2;
        state.camera.orbit_distance_min = 1.5;
        state.camera.orbit_distance_max = 8.0;
        state.controller.showroom_lock = true;
        // (cursor freed by the per-frame reconciliation in the update loop)
    }

    /// Apply a window presentation mode to the live window (v0.454). Decorations = the OS
    /// title bar; borderless drops it; the fullscreen variants use winit's Fullscreen.
    fn apply_window_mode(window: &winit::window::Window, mode: crate::config::WindowMode) {
        use crate::config::WindowMode;
        use winit::window::Fullscreen;
        match mode {
            WindowMode::Windowed => {
                window.set_fullscreen(None);
                window.set_decorations(true);
                window.set_maximized(false);
            }
            WindowMode::WindowedFullscreen => {
                window.set_fullscreen(None);
                window.set_decorations(true);
                window.set_maximized(true);
            }
            WindowMode::BorderlessWindowed => {
                window.set_fullscreen(None);
                window.set_decorations(false);
                window.set_maximized(false);
            }
            WindowMode::BorderlessFullscreen => {
                window.set_decorations(false);
                window.set_fullscreen(Some(Fullscreen::Borderless(None)));
            }
            WindowMode::ExclusiveFullscreen => {
                // Exclusive needs a concrete video mode; fall back to borderless if the
                // monitor exposes none (some platforms/headless).
                let vm = window.current_monitor().and_then(|m| m.video_modes().next());
                match vm {
                    Some(mode) => {
                        window.set_decorations(true);
                        window.set_fullscreen(Some(Fullscreen::Exclusive(mode)));
                    }
                    None => window.set_fullscreen(Some(Fullscreen::Borderless(None))),
                }
            }
        }
    }

    /// THE single authority for the OS cursor (v0.460). Free (visible + ungrabbed) for any menu
    /// page, the showroom, or the construction editor -- so egui clicks land -- and grabbed for
    /// first-person play. Called every frame AND right after the egui frame (to catch a page
    /// change made by an egui click without a frame of lag). Keeping this the ONLY place that
    /// touches the cursor keeps `cursor_free` in sync; earlier, three handlers grabbed/freed the
    /// cursor directly without updating it, which desynced the flag so a panel could render but
    /// not be clicked (recurring "I can see the UI but can't interact" bug).
    fn reconcile_cursor(state: &mut EngineState) {
        let want_free = state.gui_state.active_page != GuiPage::None
            || state.gui_state.showroom_active
            || state.gui_state.construction_active;
        if want_free == state.cursor_free {
            return;
        }
        state.cursor_free = want_free;
        state.window.set_cursor_visible(want_free);
        if want_free {
            state.window.set_cursor_grab(winit::window::CursorGrabMode::None).ok();
        } else {
            state
                .window
                .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                .or_else(|_| state.window.set_cursor_grab(winit::window::CursorGrabMode::Locked))
                .ok();
        }
    }

    /// Upload a freshly generated set of homestead meshes into the renderer + state slots
    /// (v0.455). Shared by the initial world load AND the construction editor's live rebuild.
    /// v0.531: REUSES the prior mesh/material slots in place (replace_mesh / update_material) so a
    /// per-frame rebuild during a room drag does not leak GPU buffers; only an added room/family
    /// pushes a new slot, and a removed one orphans a single slot once (bounded).
    fn apply_homestead_meshes(state: &mut EngineState, homestead: crate::ship::fibonacci::HomesteadMeshes) {
        // Reuse existing mesh/material SLOTS when present (v0.531), so a per-frame rebuild (a room
        // drag fires this every frame) never leaks GPU buffers -- the renderer was append-only, and
        // a multi-second drag was orphaning ~15-20 buffers/frame. Only an ADDED room/family pushes a
        // new slot; a REMOVED one leaves one orphaned slot (one-time, bounded).
        // Floors (one mesh + material per room): reuse the prior slot at index i when it exists.
        let prior_floors = std::mem::take(&mut state.homestead_floors);
        let mut floors = Vec::with_capacity(homestead.floors.len());
        for (i, (verts, indices, color, material_type)) in homestead.floors.into_iter().enumerate() {
            let mesh = Mesh::from_vertices(&state.renderer.device, &verts, &indices);
            if let Some(&(mi, ma)) = prior_floors.get(i) {
                state.renderer.replace_mesh(mi, mesh);
                state.renderer.update_material_typed(ma, color, 0.0, 0.8, material_type as f32);
                floors.push((mi, ma));
            } else {
                let mi = state.renderer.add_mesh(mesh);
                let ma = state.renderer.add_material_typed(color, 0.0, 0.8, material_type as f32);
                floors.push((mi, ma));
            }
        }
        state.homestead_floors = floors;
        // Combined-mesh families: reuse the prior slot if present, else add; None if empty (so a
        // removed window/mirror disappears -- its prior slot orphans once).
        let prior = state.homestead_walls;
        state.homestead_walls = if !homestead.walls.0.is_empty() {
            let mesh = Mesh::from_vertices(&state.renderer.device, &homestead.walls.0, &homestead.walls.1);
            if let Some((mi, ma)) = prior {
                state.renderer.replace_mesh(mi, mesh);
                state.renderer.update_material_typed(ma, [0.5, 0.5, 0.5, 1.0], 0.1, 0.6, 0.0);
                Some((mi, ma))
            } else {
                Some((state.renderer.add_mesh(mesh), state.renderer.add_material_typed([0.5, 0.5, 0.5, 1.0], 0.1, 0.6, 0.0)))
            }
        } else { None };
        // Per-material home walls (v0.552): one mesh+material per picked wall material so each wall
        // renders in its own color. Reuse prior slots (a per-frame rebuild fires on a drag); the
        // `is_transparent` flag routes glass (alpha < 1) to the transparent pass at render time.
        let prior_mw = std::mem::take(&mut state.homestead_material_walls);
        let mut material_walls = Vec::with_capacity(homestead.material_walls.len());
        for (i, (verts, indices, color)) in homestead.material_walls.into_iter().enumerate() {
            let mesh = Mesh::from_vertices(&state.renderer.device, &verts, &indices);
            let transparent = color[3] < 0.999;
            // Glass: low roughness + faint emissive through the transparent pass; opaque otherwise.
            let (met, rough, mtype, emis) =
                if transparent { (0.0, 0.1, 1.0, 0.05) } else { (0.1, 0.7, 0.0, 0.0) };
            if let Some(&(mi, ma, _)) = prior_mw.get(i) {
                state.renderer.replace_mesh(mi, mesh);
                state.renderer.update_material_full(ma, color, met, rough, mtype, emis);
                material_walls.push((mi, ma, transparent));
            } else {
                let mi = state.renderer.add_mesh(mesh);
                let ma = state.renderer.add_material_full(color, met, rough, mtype, emis);
                material_walls.push((mi, ma, transparent));
            }
        }
        state.homestead_material_walls = material_walls;
        let prior = state.homestead_trim;
        state.homestead_trim = if !homestead.trim.0.is_empty() {
            let mesh = Mesh::from_vertices(&state.renderer.device, &homestead.trim.0, &homestead.trim.1);
            if let Some((mi, ma)) = prior {
                state.renderer.replace_mesh(mi, mesh);
                state.renderer.update_material_typed(ma, [0.42, 0.30, 0.18, 1.0], 0.0, 0.7, 3.0);
                Some((mi, ma))
            } else {
                Some((state.renderer.add_mesh(mesh), state.renderer.add_material_typed([0.42, 0.30, 0.18, 1.0], 0.0, 0.7, 3.0)))
            }
        } else { None };
        let prior = state.homestead_windows;
        state.homestead_windows = if !homestead.windows.0.is_empty() {
            let mesh = Mesh::from_vertices(&state.renderer.device, &homestead.windows.0, &homestead.windows.1);
            // Tinted glass (alpha 0.45) + faint emissive, rendered through the transparent pass.
            if let Some((mi, ma)) = prior {
                state.renderer.replace_mesh(mi, mesh);
                state.renderer.update_material_full(ma, [0.50, 0.74, 0.92, 0.45], 0.0, 0.08, 1.0, 0.12);
                Some((mi, ma))
            } else {
                Some((state.renderer.add_mesh(mesh), state.renderer.add_material_full([0.50, 0.74, 0.92, 0.45], 0.0, 0.08, 1.0, 0.12)))
            }
        } else { None };
        let prior = state.homestead_mirrors;
        state.homestead_mirrors = if !homestead.mirrors.0.is_empty() {
            let mesh = Mesh::from_vertices(&state.renderer.device, &homestead.mirrors.0, &homestead.mirrors.1);
            if let Some((mi, ma)) = prior {
                state.renderer.replace_mesh(mi, mesh);
                state.renderer.update_material_full(ma, [0.30, 0.55, 1.0, 1.0], 0.2, 0.15, 1.0, 1.6);
                Some((mi, ma))
            } else {
                Some((state.renderer.add_mesh(mesh), state.renderer.add_material_full([0.30, 0.55, 1.0, 1.0], 0.2, 0.15, 1.0, 1.6)))
            }
        } else { None };
        // v0.539: a HomeStructure with a glass roof renders the ceiling TRANSPARENT (you see the
        // stars through the sealed clear roof); otherwise it is the opaque grey ceiling.
        let roof_glass = state
            .gui_state
            .home_structure
            .as_ref()
            .map_or(false, |hs| hs.roof_is_glass());
        state.homestead_ceiling_glass = roof_glass;
        let prior = state.homestead_ceiling;
        state.homestead_ceiling = if !homestead.ceilings.0.is_empty() {
            let mesh = Mesh::from_vertices(&state.renderer.device, &homestead.ceilings.0, &homestead.ceilings.1);
            // (color, metallic, roughness, material_type, emissive) for glass; (color, m, r, type) opaque.
            let (gcol, gmet, grough, gtype, gemis) = ([0.55, 0.78, 0.92, 0.22], 0.0, 0.05, 1.0, 0.06);
            let (ocol, omet, orough, otype) = ([0.60, 0.62, 0.68, 1.0], 0.0, 0.8, 2.0);
            if let Some((mi, ma)) = prior {
                state.renderer.replace_mesh(mi, mesh);
                if roof_glass {
                    state.renderer.update_material_full(ma, gcol, gmet, grough, gtype, gemis);
                } else {
                    state.renderer.update_material_typed(ma, ocol, omet, orough, otype);
                }
                Some((mi, ma))
            } else if roof_glass {
                Some((state.renderer.add_mesh(mesh), state.renderer.add_material_full(gcol, gmet, grough, gtype, gemis)))
            } else {
                Some((state.renderer.add_mesh(mesh), state.renderer.add_material_typed(ocol, omet, orough, otype)))
            }
        } else { None };
    }

    /// Regenerate the homestead meshes from the live layout (the construction editor's apply).
    /// Also refreshes room lights + the sealed-volume bounds, since a height/wall edit changes
    /// them. (v0.455)
    /// Snapshot the current editor state for undo/redo (v0.575). Structure + machines only -- not the
    /// selection (restoring a stale selection would yank the right panel).
    fn editor_snapshot(state: &EngineState) -> EditorSnapshot {
        EditorSnapshot {
            structure: state.gui_state.home_structure.clone(),
            machines: state.gui_state.home_machines.clone(),
        }
    }

    /// Restore a snapshot into gui_state and rebuild the home DIRECTLY (v0.575). Rebuilding here rather
    /// than via the dirty flags means the restore never looks like a fresh edit to the history tick --
    /// so it can't spuriously checkpoint and there's no restore/edit frame race.
    fn editor_restore(state: &mut EngineState, snap: EditorSnapshot) {
        state.gui_state.home_structure = snap.structure;
        state.gui_state.home_machines = snap.machines;
        rebuild_homestead(state);
        rebuild_machine_objects(state);
    }

    /// Per-frame undo-history tick (v0.575). Call BEFORE the dirty-flag rebuild blocks consume them.
    /// `edited` = a dirty flag was set this frame. Resets history on editor-open; coalesces a continuous
    /// drag -- a gizmo OR a slider -- into ONE undo step by checkpointing only while the left mouse
    /// button is NOT held, plus once on release if an edit happened during the hold.
    fn construction_history_tick(state: &mut EngineState, edited: bool) {
        let active = state.gui_state.construction_active;
        let prev_active = state.construction_history.prev_active;
        state.construction_history.prev_active = active;
        if active && !prev_active {
            // Editor opened: the current state is the baseline; clear the stacks.
            let base = editor_snapshot(state);
            let h = &mut state.construction_history;
            h.undo.clear();
            h.redo.clear();
            h.baseline = base;
            h.edited_during_hold = false;
            h.prev_held = false;
            return;
        }
        if !active {
            return; // history is editor-only
        }
        let held = state.lmb_held;
        let prev_held = state.construction_history.prev_held;
        state.construction_history.prev_held = held;
        if held {
            if edited {
                state.construction_history.edited_during_hold = true;
            }
            return; // never checkpoint mid-drag (gizmo or slider)
        }
        // Not held: checkpoint on a click-edit, or a release that actually edited during the hold.
        let released_with_edit = prev_held && state.construction_history.edited_during_hold;
        if released_with_edit || edited {
            let cur = editor_snapshot(state);
            let depth = state.gui_state.construction_undo_depth.clamp(1, 4096);
            let h = &mut state.construction_history;
            h.undo.push_back(std::mem::replace(&mut h.baseline, cur));
            while h.undo.len() > depth {
                h.undo.pop_front();
            }
            h.redo.clear();
            h.edited_during_hold = false;
        }
    }

    /// Undo the last construction edit (v0.575): restore the most recent pre-edit snapshot.
    fn construction_undo(state: &mut EngineState) {
        if let Some(prev) = state.construction_history.undo.pop_back() {
            let cur = editor_snapshot(state);
            state.construction_history.redo.push(cur);
            state.construction_history.baseline = prev.clone();
            editor_restore(state, prev);
        }
    }

    /// Redo the last undone construction edit (v0.575).
    fn construction_redo(state: &mut EngineState) {
        if let Some(next) = state.construction_history.redo.pop() {
            let cur = editor_snapshot(state);
            state.construction_history.undo.push_back(cur);
            state.construction_history.baseline = next.clone();
            editor_restore(state, next);
        }
    }

    fn rebuild_homestead(state: &mut EngineState) {
        // Normalize every corner onto the corner grid (v0.574) so co-located corners are byte-identical
        // -- this self-heals any older home whose snapped corners had sub-tolerance residue (which read
        // as two overlapping orbs that dragged apart). Idempotent: an on-grid corner is unchanged.
        if let Some(hs) = state.gui_state.home_structure.as_mut() {
            for wall in hs.walls.iter_mut() {
                wall.a = crate::ship::home_structure::quantize_corner(wall.a);
                wall.b = crate::ship::home_structure::quantize_corner(wall.b);
            }
        }
        // Dev tool (v0.576): write a machine-readable snapshot of the live home so an AI can READ what
        // the operator is building (the act surface -- a text-command console -- is the next stage).
        if let Some(hs) = state.gui_state.home_structure.as_ref() {
            let json = hs.to_introspection_json();
            let _ = std::fs::create_dir_all("debug");
            let _ = std::fs::write("debug/home_snapshot.json", json);
        }
        // v0.534: regenerate from the new HomeStructure (fixed box + interior walls) when present,
        // else the legacy AABB-room layout.
        let homestead = if let Some(hs) = &state.gui_state.home_structure {
            hs.generate_meshes()
        } else if let Some(layout) = state.homestead_layout.clone() {
            crate::ship::fibonacci::generate_from_layout(&layout)
        } else {
            return;
        };
        let room_info = homestead.room_info.clone();
        // Rebuild the wall collision segments from the live home (v0.556) so editing a wall updates
        // what the player walks into. Empty for the legacy AABB layout (no per-segment home walls).
        state.wall_colliders = match &state.gui_state.home_structure {
            Some(hs) => crate::ship::wall_collision::wall_segments(hs),
            None => Vec::new(),
        };
        apply_homestead_meshes(state, homestead);
        // Refresh lights + sealed bounds from the new room_info (height edits move them).
        let auto_lights = room_info.iter().map(|r| {
            let light_pos = Vec3::new(r.center.x, r.center.y + r.dimensions.y * 0.5 - 0.1, r.center.z);
            let room_size = r.dimensions.x.max(r.dimensions.z);
            let intensity = (room_size * 0.5).clamp(2.0, 15.0);
            (light_pos, [1.0, 0.95, 0.85], intensity, room_size * 1.5)
        }).collect();
        // v0.571: a home's PLACED lights override the auto one-per-room synthesis (empty -> auto).
        state.room_lights = home_lights(state.gui_state.home_structure.as_ref(), auto_lights, state.gui_state.gi_enabled);
        state.homestead_bounds = room_info.iter().fold(None, |acc, r| {
            let rmin = r.center - r.dimensions * 0.5;
            let rmax = r.center + r.dimensions * 0.5;
            Some(match acc { None => (rmin, rmax), Some((mn, mx)) => (mn.min(rmin), mx.max(rmax)) })
        });
        // Refresh the HUD room volumes (the "you are in <room>" detection + occlusion) so a
        // moved/resized/added/removed room is tracked live, not just on restart. (v0.459)
        // (Machine placement + pipes + hologram/spawn still resolve at load_world; they refresh
        // on the next relaunch -- a follow-up will make them live too.)
        let room_types = crate::ship::room_types::RoomTypeRegistry::load(&state.data_dir);
        state.gui_state.room_bounds = room_info
            .iter()
            .map(|r| crate::gui::RoomBounds {
                id: r.id.clone(),
                min: r.center - r.dimensions * 0.5,
                max: r.center + r.dimensions * 0.5,
                display_name: room_types.name(&r.id),
                purpose: room_types.purpose(&r.id),
                actions: room_types.action_labels(&r.id),
                access: room_types.access(&r.id),
            })
            .collect();
        // Room geometry changed, so the machines in those rooms must follow (a moved/resized room
        // carries its machines). Refresh the machine meshes from the new room bounds. (v0.525)
        rebuild_machine_objects(state);
        // Door/window panels follow the structure too (a wall edit can add/move/remove openings).
        rebuild_door_panels(state);
        log::info!("Homestead rebuilt: {} rooms", room_info.len());
    }

    /// Build a machine's primitive mesh from its shape + size. Shared by load_world (initial spawn)
    /// and rebuild_machine_objects (the editor's live refresh) so both draw a machine identically.
    fn machine_mesh(device: &wgpu::Device, shape: &str, size: (f32, f32, f32)) -> Mesh {
        let (sx, sy, sz) = size;
        match shape {
            "cylinder" => Mesh::cylinder(device, sx.max(0.02), sy.max(0.05), 16),
            "sphere" => Mesh::sphere(device, sx.max(0.02), 10, 12),
            "pyramid" => Mesh::pyramid(device, sx.max(0.05), sy.max(0.05)),
            _ => Mesh::box_xyz(device, sx.max(0.02), sy.max(0.02), sz.max(0.02)),
        }
    }

    /// Rebuild ONLY the home machine meshes + floating labels from the live editor state
    /// (gui_state.home_machines + room_bounds), so a construction-editor edit (move/add/remove/
    /// connect) shows immediately instead of only on the next world entry. Positions come from the
    /// tested MachineHome::placements. Does NOT touch the live power ECS (that refreshes on world
    /// entry) or the connection pipes (a follow-up). (v0.525)
    fn rebuild_machine_objects(state: &mut EngineState) {
        use std::collections::HashMap;
        let rooms: HashMap<String, crate::machines::RoomGeom> = state
            .gui_state
            .room_bounds
            .iter()
            .map(|rb| {
                (
                    rb.id.clone(),
                    crate::machines::RoomGeom {
                        center_x: (rb.min.x + rb.max.x) * 0.5,
                        center_z: (rb.min.z + rb.max.z) * 0.5,
                        floor_y: rb.min.y,
                        ceiling_y: rb.max.y,
                    },
                )
            })
            .collect();
        // Guard: if there is no room geometry yet (room_bounds not populated), do NOT wipe the
        // machines load_world already placed -- otherwise an edit before bounds are ready blanks
        // the whole home. (v0.528)
        if rooms.is_empty() {
            return;
        }
        // v0.538: a HomeStructure home positions machines by ABSOLUTE world coords (box mode), not
        // room-center-relative -- so they survive flood-fill room-id churn.
        let (box_mode, box_dims) = match &state.gui_state.home_structure {
            Some(hs) => (true, (hs.width, hs.depth, hs.height)),
            None => (false, (0.0, 0.0, 0.0)),
        };
        let placements = match &state.gui_state.home_machines {
            Some(h) => h.placements(&rooms, box_mode, box_dims),
            None => return,
        };
        // Fast path: the machine COUNT is unchanged (an offset drag / room move, not add/remove).
        // Reuse the existing meshes + materials and only update positions, so a per-frame drag does
        // NOT leak a fresh mesh per machine every frame (the v0.527 regression). placements() is
        // deterministically ordered (instances then array cells), so index i is the same machine.
        if placements.len() == state.machine_objects.len()
            && placements.len() == state.gui_state.machine_labels.len()
            && placements.len() == state.machine_pick.len()
        {
            for (i, p) in placements.iter().enumerate() {
                state.machine_objects[i].2 = Vec3::new(p.pos.0, p.pos.1, p.pos.2);
                state.gui_state.machine_labels[i].pos = Vec3::new(p.pos.0, p.top_y + 0.4, p.pos.2);
                // Keep the pick volume in sync (v0.553) -- else a move WITHOUT a count change (a room
                // drag, a clamp-on-resize) leaves the click ray-test + the highlight ring at the OLD
                // position. Same math as the slow-path build below.
                let half_h = ((p.top_y - p.pos.1) * 0.5).max(0.2);
                let half_w = p.size.0.max(p.size.1).max(p.size.2) * 0.5;
                state.machine_pick[i] = (
                    p.id.clone(),
                    Vec3::new(p.pos.0, (p.pos.1 + p.top_y) * 0.5, p.pos.2),
                    half_h.max(half_w) + 0.35,
                );
            }
            rebuild_connection_objects(state);
            return;
        }
        // Count changed (add / remove) or first build. Reuse prior mesh/material SLOTS where they
        // exist (replace in place) instead of clear()+re-add, so a single add/remove doesn't orphan
        // the whole ~100-mesh home; only the growth pushes new slots, and a shrink orphans the tail
        // once (bounded). (v0.531 -- the renderer free path.)
        let prior = std::mem::take(&mut state.machine_objects);
        state.gui_state.machine_labels.clear();
        state.machine_pick.clear();
        let mut objs = Vec::with_capacity(placements.len());
        for (i, p) in placements.iter().enumerate() {
            let mesh = machine_mesh(&state.renderer.device, &p.shape, p.size);
            let color = [p.color.0, p.color.1, p.color.2, 1.0];
            let pos = Vec3::new(p.pos.0, p.pos.1, p.pos.2);
            if let Some(&(mi, ma, _)) = prior.get(i) {
                state.renderer.replace_mesh(mi, mesh);
                state.renderer.update_material_typed(ma, color, 0.1, 0.7, 0.0);
                objs.push((mi, ma, pos));
            } else {
                let mi = state.renderer.add_mesh(mesh);
                let ma = state.renderer.add_material_typed(color, 0.1, 0.7, 0.0);
                objs.push((mi, ma, pos));
            }
            state.gui_state.machine_labels.push(crate::gui::MachineLabel {
                pos: Vec3::new(p.pos.0, p.top_y + 0.4, p.pos.2),
                name: p.label.clone(),
                stats: p.stats.clone(),
                room: p.room.clone(),
            });
            // Pick volume for viewport selection: a sphere covering the machine body. Center at its
            // mid-height; radius the larger of half-height / half-width plus a click margin.
            let half_h = ((p.top_y - p.pos.1) * 0.5).max(0.2);
            let half_w = p.size.0.max(p.size.1).max(p.size.2) * 0.5;
            state.machine_pick.push((
                p.id.clone(),
                Vec3::new(p.pos.0, (p.pos.1 + p.top_y) * 0.5, p.pos.2),
                half_h.max(half_w) + 0.35,
            ));
        }
        state.machine_objects = objs;
        rebuild_connection_objects(state);
    }

    /// Rebuild the home connection cylinders from the live machine layout (gui_state.home_machines
    /// + room_bounds): one colored cylinder per connection, between the two machines' low pipe
    /// anchors. Uses a cached unit cylinder + a material cached per kind, so a per-frame rebuild
    /// never leaks. Replaces the old static routed pipes -- connections now follow rooms. (v0.530)
    fn rebuild_connection_objects(state: &mut EngineState) {
        use std::collections::HashMap;
        state.connection_objects.clear();
        let rooms: HashMap<String, crate::machines::RoomGeom> = state
            .gui_state
            .room_bounds
            .iter()
            .map(|rb| {
                (
                    rb.id.clone(),
                    crate::machines::RoomGeom {
                        center_x: (rb.min.x + rb.max.x) * 0.5,
                        center_z: (rb.min.z + rb.max.z) * 0.5,
                        floor_y: rb.min.y,
                        ceiling_y: rb.max.y,
                    },
                )
            })
            .collect();
        if rooms.is_empty() {
            return;
        }
        // v0.538: box-mode absolute positioning when a HomeStructure home is active (mirrors
        // rebuild_machine_objects so the conduit anchors match the machine meshes).
        let (box_mode, box_dims) = match &state.gui_state.home_structure {
            Some(hs) => (true, (hs.width, hs.depth, hs.height)),
            None => (false, (0.0, 0.0, 0.0)),
        };
        let (placements, connections) = match &state.gui_state.home_machines {
            Some(h) => (h.placements(&rooms, box_mode, box_dims), h.connections.clone()),
            None => return,
        };
        // Low pipe-height anchor per machine id (the fixture port the conduit drops to).
        let anchors: HashMap<String, Vec3> = placements
            .iter()
            .map(|p| (p.id.clone(), Vec3::new(p.pos.0, p.floor_y + 0.35, p.pos.2)))
            .collect();
        // Combined routing list (v0.581): both the legacy point-to-point connections AND the conduit
        // NODE GRAPH edges (machine/node -> machine/node) become (a, b, kind) routes, fed through the
        // SAME route_conduit + emit below. A node edge renders as a real routed pipe with zero new mesh.
        let mut routes: Vec<(Vec3, Vec3, String)> = connections
            .iter()
            .filter_map(|c| Some((*anchors.get(&c.from)?, *anchors.get(&c.to)?, c.kind.clone())))
            .collect();
        {
            let placement_tuples: Vec<(String, (f32, f32, f32), f32)> =
                placements.iter().map(|p| (p.id.clone(), p.pos, p.floor_y)).collect();
            if let Some(home) = state.gui_state.home_machines.as_ref() {
                for e in &home.conduit_edges {
                    if let (Some(a), Some(b)) = (
                        home.conduit_anchor(&e.from, &placement_tuples, box_dims),
                        home.conduit_anchor(&e.to, &placement_tuples, box_dims),
                    ) {
                        routes.push((Vec3::new(a.0, a.1, a.2), Vec3::new(b.0, b.1, b.2), e.kind.clone()));
                    }
                }
            }
        }
        if routes.is_empty() {
            return;
        }
        // Cached unit cylinder mesh (+Y, base at origin, radius 0.05, height 1) -- reused for every
        // conduit segment + fitting, scaled/rotated, so a rebuild never leaks.
        let cyl = match state.connection_cyl {
            Some(m) => m,
            None => {
                let m = state
                    .renderer
                    .add_mesh(Mesh::cylinder(&state.renderer.device, 0.05, 1.0, 8));
                state.connection_cyl = Some(m);
                m
            }
        };
        // Home geometry for routing (v0.536): run conduits UP to a service height near the ceiling
        // and ACROSS in Manhattan legs (never a straight diagonal through the room -- the operator's
        // "the straight lines that pass through everything is wrong"), placing material-aware
        // passthroughs where a run crosses an interior wall.
        let (home_h, shell_mat, walls) = match &state.gui_state.home_structure {
            Some(hs) => (hs.height, hs.shell_material, hs.walls.clone()),
            None => (3.0, 1, Vec::new()),
        };
        let service_y = (home_h - 0.3).max(0.6);
        const CYL_R: f32 = 0.05; // the unit cylinder's modeled radius
        for (a, b, kind_str) in &routes {
            let (a, b) = (*a, *b);
            let kind = crate::ship::conduits::ConduitKind::for_resource(kind_str);
            let route = crate::ship::conduits::route_conduit(a, b, kind, service_y, shell_mat, &walls);
            // Pipe material cached per conduit kind (copper / rubber hose / black cord).
            let pkey = format!("conduit:{kind:?}");
            let pipe_mat = match state.connection_mats.get(&pkey) {
                Some(&m) => m,
                None => {
                    let (met, rough) = if kind.is_rigid() { (0.85, 0.25) } else { (0.0, 0.7) };
                    let m = state.renderer.add_material_typed(kind.color(), met, rough, 0.0);
                    state.connection_mats.insert(pkey.clone(), m);
                    m
                }
            };
            let rscale = kind.radius() / CYL_R;
            // The routed pipe: one cylinder per leg (up, across, across, down).
            for seg in route.points.windows(2) {
                let (p, q) = (seg[0], seg[1]);
                let diff = q - p;
                let len = diff.length();
                if len < 1e-4 {
                    continue;
                }
                let rot = Quat::from_rotation_arc(Vec3::Y, diff / len);
                state
                    .connection_objects
                    .push((cyl, pipe_mat, p, rot, Vec3::new(rscale, len, rscale)));
            }
            // Procedural support structures: a ceiling hanger at each service-height bracket + a
            // material-aware gasket collar at each wall passthrough. The fitting colour comes from the
            // material it attaches to, so a steel vs wood wall reads differently.
            for f in &route.fittings {
                let fkey = format!("fitting:{}", f.material);
                let fmat = match state.connection_mats.get(&fkey) {
                    Some(&m) => m,
                    None => {
                        let col = match f.material {
                            1 => [0.58, 0.60, 0.65, 1.0], // steel
                            2 => [0.64, 0.64, 0.62, 1.0], // concrete
                            3 => [0.52, 0.37, 0.22, 1.0], // wood
                            _ => [0.50, 0.52, 0.56, 1.0],
                        };
                        let m = state.renderer.add_material_typed(col, 0.6, 0.4, f.material as f32);
                        state.connection_mats.insert(fkey.clone(), m);
                        m
                    }
                };
                match f.kind {
                    crate::ship::conduits::FittingKind::Bracket => {
                        // Ceiling hanger (a thin post up to the ceiling) for the horizontal service
                        // runs; the short vertical drops are held at their ends, so skip them.
                        if f.at.y >= service_y - 0.1 {
                            let drop = (home_h - f.at.y).max(0.05);
                            state.connection_objects.push((
                                cyl,
                                fmat,
                                f.at,
                                Quat::IDENTITY,
                                Vec3::new(0.5, drop, 0.5),
                            ));
                        }
                    }
                    crate::ship::conduits::FittingKind::Passthrough => {
                        // A short gasket collar straddling the wall at the crossing.
                        state.connection_objects.push((
                            cyl,
                            fmat,
                            f.at - Vec3::new(0.0, 0.12, 0.0),
                            Quat::IDENTITY,
                            Vec3::new(2.4, 0.24, 2.4),
                        ));
                    }
                    crate::ship::conduits::FittingKind::Elbow => {}
                }
            }
        }
    }

    /// Recompute the door/window panel placements from the live HomeStructure (v0.537). Called after
    /// a structure rebuild + on load. Preserves the per-panel open fraction when the panel COUNT is
    /// unchanged (so editing a far wall does not slam every door shut); otherwise resets to closed.
    fn rebuild_door_panels(state: &mut EngineState) {
        let placements = match &state.gui_state.home_structure {
            Some(hs) => crate::ship::door_panels::panel_placements(hs),
            None => Vec::new(),
        };
        if placements.len() == state.door_panels.len() {
            for (i, p) in placements.into_iter().enumerate() {
                state.door_panels[i].0 = p;
            }
        } else {
            state.door_panels = placements.into_iter().map(|p| (p, 0.0)).collect();
        }
        // Reset every manual door to CLOSED on a structural rebuild (v0.567). This runs only on a
        // structure edit / world load (build mode, orbit cam), never while walking, so we deliberately
        // do NOT trust positional parallelism across an edit -- an open-flag must never land on the
        // wrong door just because the opening count happened to stay equal.
        state.door_manual_open = vec![false; state.door_panels.len()];
        // Reset live lock state to each door's AUTHORED states on a rebuild (v0.570), parallel to
        // door_panels. Same reasoning as the manual-open reset above.
        state.door_locks = state
            .door_panels
            .iter()
            .map(|(p, _)| p.locks.iter().map(|l| l.state).collect())
            .collect();
    }

    /// Is door `panel` currently locked, using its LIVE lock states when present (v0.570)? A door with
    /// locks is locked iff any live lock is not open; an empty lock list falls back to the legacy
    /// `panel.locked` bool, so v0.567 doors are unchanged. `live` is `door_locks[i]`.
    fn door_locked_now(
        panel: &crate::ship::door_panels::PanelPlacement,
        live: Option<&Vec<crate::ship::lock_types::LockState>>,
    ) -> bool {
        if panel.locks.is_empty() {
            return panel.locked;
        }
        match live {
            Some(states) if states.len() == panel.locks.len() => states.iter().any(|s| !s.is_open()),
            _ => panel.locks.iter().any(|l| !l.state.is_open()), // fall back to authored
        }
    }

    /// The room point-lights to upload (v0.571, refined v0.572). A home's PLACED lights (resolved from
    /// light_types.ron + per-instance overrides) take over once ANY are placed; otherwise the crude
    /// `auto` one-per-room fill is used, and ONLY when GI is on. Rationale (operator v0.572 feedback):
    /// the auto fill is a single bright point light at room centre that reads as an ugly "sun spotlight"
    /// pool -- so once the operator places their own lights, we drop it entirely (their lights ARE the
    /// room lighting; the directional SUN, gated separately by GI, still provides the even base when GI
    /// is on). With NO placed lights the old behaviour is unchanged (auto fill when GI on, dark when off).
    fn home_lights(
        home: Option<&crate::ship::home_structure::HomeStructure>,
        auto: Vec<(Vec3, [f32; 3], f32, f32)>,
        gi_on: bool,
    ) -> Vec<(Vec3, [f32; 3], f32, f32)> {
        let placed: Vec<(Vec3, [f32; 3], f32, f32)> = home
            .map(|h| {
                h.lights
                    .iter()
                    .filter(|l| l.on)
                    .filter_map(|l| {
                        let t = crate::renderer::light::light_type(&l.type_id)?;
                        let c = l.color.unwrap_or(t.color);
                        Some((
                            Vec3::new(l.pos.0, l.pos.1, l.pos.2),
                            [c.0, c.1, c.2],
                            l.intensity.unwrap_or(t.intensity),
                            l.range.unwrap_or(t.range),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Any placed lights -> the home is manually lit, no auto centre-spot. Else auto fill if GI on.
        if !placed.is_empty() {
            placed
        } else if gi_on {
            auto
        } else {
            Vec::new()
        }
    }

    /// Per-frame: animate + emit the door/window panels (v0.537). A door eases open as the player
    /// approaches (by its data-driven style via systems::door_anim); a window is a fixed glass pane.
    /// Reuses one cached unit-box mesh + a slab + a glass material (scaled/rotated/animated per frame),
    /// so nothing leaks. Doors go to the opaque pass, glass to the transparent pass.
    fn render_door_panels(
        state: &mut EngineState,
        opaque: &mut Vec<RenderObject>,
        transparent: &mut Vec<RenderObject>,
        ring_lines: &mut Vec<crate::renderer::line::LineVertex>,
        dt: f32,
    ) {
        if state.door_panels.is_empty() {
            return;
        }
        let mesh = match state.door_panel_mesh {
            Some(m) => m,
            None => {
                let m = state.renderer.add_mesh(Mesh::box_xyz(&state.renderer.device, 1.0, 1.0, 1.0));
                state.door_panel_mesh = Some(m);
                m
            }
        };
        let slab_mat = match state.door_slab_mat {
            Some(m) => m,
            None => {
                // theme-exempt: world-object material, not a themed UI surface.
                let m = state.renderer.add_material_typed([0.36, 0.38, 0.43, 1.0], 0.3, 0.5, 1.0);
                state.door_slab_mat = Some(m);
                m
            }
        };
        let glass_mat = match state.door_glass_mat {
            Some(m) => m,
            None => {
                // theme-exempt: tinted glass, transparent pass.
                let m = state.renderer.add_material_full([0.55, 0.78, 0.92, 0.34], 0.0, 0.08, 1.0, 0.10);
                state.door_glass_mat = Some(m);
                m
            }
        };
        // Energy + nanowall door materials (v0.554), all rendered in the transparent pass: an ENERGY
        // door is a glowing FIELD -- green while operable, red while LOCKED; a NANOWALL is a metallic
        // semi-transparent surface you see through as it dissolves open.
        let energy_open_mat = match state.door_energy_open_mat {
            Some(m) => m,
            None => {
                // theme-exempt: glowing green energy field.
                let m = state.renderer.add_material_full([0.20, 1.0, 0.40, 0.42], 0.0, 0.3, 1.0, 1.4);
                state.door_energy_open_mat = Some(m);
                m
            }
        };
        let energy_locked_mat = match state.door_energy_locked_mat {
            Some(m) => m,
            None => {
                // theme-exempt: glowing red energy field (locked).
                let m = state.renderer.add_material_full([1.0, 0.18, 0.20, 0.50], 0.0, 0.3, 1.0, 1.4);
                state.door_energy_locked_mat = Some(m);
                m
            }
        };
        let nanowall_mat = match state.door_nanowall_mat {
            Some(m) => m,
            None => {
                // theme-exempt: metallic gray nanowall, semi-transparent.
                let m = state.renderer.add_material_full([0.62, 0.64, 0.70, 0.60], 0.85, 0.15, 1.0, 0.15);
                state.door_nanowall_mat = Some(m);
                m
            }
        };
        // Nanowall shimmer (v0.554): drift the metallic gray + emissive over time so the surface reads
        // as a live, shifting "water" field rather than a static slab. One shared-material write/frame.
        state.door_anim_time += dt.max(0.0);
        let shimmer = 0.5 + 0.5 * (state.door_anim_time * 1.6).sin();
        let g = 0.58 + 0.10 * shimmer;
        state.renderer.update_material_full(nanowall_mat, [g * 0.94, g, g * 1.06, 0.60], 0.85, 0.10 + 0.08 * shimmer, 1.0, 0.08 + 0.16 * shimmer);
        let cam = state.camera.position;
        // v0.547: per-door open distance. The interaction ring shows it in build mode / dev overlay.
        // The ring is a constant-width LINE circle now (v0.568), so there is no polygon-ring mesh.
        let show_widgets = state.gui_state.construction_active || state.gui_state.construction_dev_overlay;
        // Frame-rate-independent exponential ease toward the target (v0.540): smooth open/close,
        // no linear stepping, no extra keyframes. ~0.3 s to settle.
        let ease = 1.0 - (-dt.max(0.0) * 9.0).exp();
        // Snapshot the per-door manual-open flags (v0.567) so the loop can read them while it holds a
        // &mut on door_panels (a disjoint-field borrow the checker won't always see through).
        let manual = state.door_manual_open.clone();
        let locks_live = state.door_locks.clone();
        for (di, (p, open)) in state.door_panels.iter_mut().enumerate() {
            // An operable DOOR opens on approach; a window or a "fixed"-styled opening stays shut
            // (v0.538: consult door_anim::is_operable so a door explicitly styled "fixed" does not
            // chase an open target it can never animate to).
            let operable = !p.is_window && crate::systems::door_anim::is_operable(&p.style);
            // Is the door LOCKED right now (v0.570)? Live lock states if present, else the legacy bool.
            let locked_now = door_locked_now(p, locks_live.get(di));
            // Interaction-distance ring on the floor at the door (v0.547), drawn as a LINE circle
            // (v0.565, operator's idea -- like the orbit paths) so its width is CONSTANT regardless of
            // radius, instead of a polygon strip that thickened as open_dist grew.
            if show_widgets && operable && p.auto_open {
                const RING_COL: [f32; 4] = [0.35, 0.85, 1.0, 0.9]; // cyan
                crate::renderer::line::push_circle(
                    ring_lines, [p.center.x, 0.04, p.center.z], p.open_dist, RING_COL, 72,
                );
            }
            // Wall-mounted CONTROL PANEL beside a manual/controlled door (v0.567): a glowing tech panel
            // the player walks up to and presses E. Green while openable, red while LOCKED. Routed to the
            // transparent pass since it glows. Drawn before the door's hidden-check so it always shows.
            // Only on a MANUAL door -- an auto door opens by itself, so its panel would be a dead control.
            if p.control_panel && !p.auto_open {
                let cp = p.control_panel_pos;
                let mat = if locked_now { energy_locked_mat } else { energy_open_mat };
                transparent.push(RenderObject {
                    position: Vec3::new(cp.x, cp.y - 0.14, cp.z),
                    rotation: p.rotation,
                    scale: Vec3::new(0.18, 0.28, 0.06),
                    mesh,
                    material: mat,
                });
            }
            // Lock indicators (v0.570): a small box per lock on the door face -- RED locked, GREEN
            // unlocked, GREY broken. Shows whether (and how) a door is secured even without a panel.
            // Doors only -- a window is a fixed pane (locks on a hand-authored window are inert).
            if !p.is_window {
                for (li, lock) in p.locks.iter().enumerate() {
                    let st = locks_live.get(di).and_then(|v| v.get(li)).copied().unwrap_or(lock.state);
                    let lm = match st {
                        crate::ship::lock_types::LockState::Locked => energy_locked_mat,
                        crate::ship::lock_types::LockState::Unlocked => energy_open_mat,
                        crate::ship::lock_types::LockState::Broken => slab_mat,
                    };
                    transparent.push(RenderObject {
                        position: Vec3::new(lock.pos.x, lock.pos.y - 0.05, lock.pos.z),
                        rotation: p.rotation,
                        scale: Vec3::new(0.1, 0.1, 0.05),
                        mesh,
                        material: lm,
                    });
                }
            }
            let dx = cam.x - p.center.x;
            let dz = cam.z - p.center.z;
            let dist = (dx * dx + dz * dz).sqrt(); // horizontal -- the camera's eye height must not count
            // Hysteresis (v0.540): a closed door opens within open_dist; an open one stays open until
            // you back past open_dist + 0.8, so standing near the threshold no longer flickers it.
            let target = if !operable || locked_now {
                // A fixed pane or a LOCKED door never opens (v0.570: lock-list aware).
                0.0
            } else if !p.auto_open {
                // A MANUAL door (v0.564) opens only when toggled at its control panel (v0.567).
                if manual.get(di).copied().unwrap_or(false) { 1.0 } else { 0.0 }
            } else if *open > 0.5 {
                if dist < p.open_dist + 0.8 { 1.0 } else { 0.0 }
            } else if dist < p.open_dist {
                1.0
            } else {
                0.0
            };
            *open = (*open + (target - *open) * ease).clamp(0.0, 1.0);
            let m = crate::systems::door_anim::panel_motion(&p.style, *open, p.size.x, p.size.y);
            if m.hidden {
                continue;
            }
            let hinge_rot = Quat::from_rotation_y(m.hinge);
            let world_off = p.rotation * Vec3::new(m.offset.0, m.offset.1, m.offset.2);
            let c = p.center + world_off;
            let pos = p.hinge + hinge_rot * (c - p.hinge);
            let rot = hinge_rot * p.rotation;
            let scale = Vec3::new(p.size.x * m.scale.0, p.size.y * m.scale.1, p.size.z * m.scale.2);
            // Pick the panel material by style + lock state, and route glowing / glassy panels through
            // the transparent pass so they blend (v0.554).
            let (material, is_transparent) = if p.is_window {
                (glass_mat, true)
            } else if p.style == "energy" {
                // v0.570: lock-list aware (was `p.locked`), so an energy door driven by a lock list
                // glows red while actually impassable instead of a misleading green.
                (if locked_now { energy_locked_mat } else { energy_open_mat }, true)
            } else if p.style == "nanowall" {
                (nanowall_mat, true)
            } else {
                (slab_mat, false)
            };
            let obj = RenderObject { position: pos, rotation: rot, scale, mesh, material };
            if is_transparent {
                transparent.push(obj);
            } else {
                opaque.push(obj);
            }
        }
    }

    /// The slide-gizmo handles for the currently-selected room, with each handle's owning
    /// `construction_rooms` index resolved (so a drag writes the offset back to the mirror).
    /// Empty when nothing is selected. (v0.468)
    fn selected_room_handles(state: &EngineState)
        -> Vec<(usize, crate::ship::fibonacci::OpeningHandle)> {
        let Some(sel) = state.gui_state.construction_selected_room else { return Vec::new(); };
        let Some(sel_room) = state.gui_state.construction_rooms.get(sel) else { return Vec::new(); };
        let sel_id = sel_room.id.clone();
        let Some(layout) = &state.homestead_layout else { return Vec::new(); };
        let positions = crate::ship::fibonacci::resolve_positions(layout);
        crate::ship::fibonacci::opening_handles(layout, &positions)
            .into_iter()
            .filter(|h| layout.rooms.get(h.room_index).map_or(false, |r| r.id == sel_id))
            .map(|h| (sel, h)) // all belong to the selected room -> selected mirror index
            .collect()
    }

    /// Left-click in the construction astral editor: cast a pick ray from the cursor. First try
    /// the selected room's door/window slide handles (so they take precedence over the room
    /// grab); otherwise hit-test each room's floor rectangle, select + grab the nearest. (v0.466)
    fn try_begin_room_grab(state: &mut EngineState) {
        let sz = state.window.inner_size();
        let viewport = (sz.width as f32, sz.height as f32);
        let (origin, dir) = state.camera.pick_ray(state.cursor_pos, viewport);
        // 1. Opening gizmo handles of the selected room. Walls are VERTICAL, so intersect the
        //    pick ray with each handle's wall-FACE plane (no dir.y needed) and classify the
        //    nearest of {Move, and -- for a placed resizable opening -- the edge handles}. (v0.469)
        let handles = selected_room_handles(state);
        // (ri, handle, role, dist)
        let mut best_h: Option<(usize, crate::ship::fibonacci::OpeningHandle, GizmoRole, f32)> = None;
        for (ri, h) in &handles {
            let denom = dir.dot(h.n);
            if denom.abs() < 1e-6 { continue; } // ray parallel to the wall face
            let t = (h.wall_start - origin).dot(h.n) / denom;
            if t <= 0.0 { continue; } // plane behind the camera
            let hit = origin + dir * t;
            // Move always; placed openings add edge handles (width-only when floor-snapped).
            let mut cands: Vec<(GizmoRole, Vec3)> = vec![(GizmoRole::Move, h.base_center)];
            if h.opening_index.is_some() {
                cands.push((GizmoRole::ResizeLeft, h.handle_left));
                cands.push((GizmoRole::ResizeRight, h.handle_right));
                if !h.kind.floor_snapped() {
                    cands.push((GizmoRole::ResizeBottom, h.handle_bottom));
                    cands.push((GizmoRole::ResizeTop, h.handle_top));
                }
            }
            for (role, p) in cands {
                let d = (hit - p).length();
                let pick_r = if role == GizmoRole::Move { 0.3 } else { 0.18 };
                if d <= pick_r && best_h.map_or(true, |b| d < b.3) {
                    best_h = Some((*ri, *h, role, d));
                }
            }
        }
        if let Some((ri, h, role, _)) = best_h {
            state.construction_gizmo_grab = Some(ConstructionGizmoGrab {
                room_index: ri,
                opening_index: h.opening_index,
                wall_index: h.wall_index,
                role,
                snap_floor: h.kind.floor_snapped(),
                wall_start: h.wall_start,
                u_hat: h.u_hat,
                n: h.n,
                wall_len: h.wall_len,
                wall_height: h.wall_height,
                base_t: h.base_t,
                grab_u: h.u,
                grab_v: h.v,
                grab_w: h.w,
                grab_h: h.h,
            });
            return; // grabbed a handle; don't also grab the room
        }
        // 2. Nearest room floor rect (needs dir.y; a horizontal-ish ray can't hit the floor plane).
        if dir.y.abs() < 1e-6 {
            return;
        }
        let mut best: Option<(usize, f32, f32, f32)> = None; // (rb_index, t, hit_x, hit_z)
        for (i, rb) in state.gui_state.room_bounds.iter().enumerate() {
            let t = (rb.min.y - origin.y) / dir.y;
            if t <= 0.0 {
                continue;
            }
            let hx = origin.x + dir.x * t;
            let hz = origin.z + dir.z * t;
            if hx >= rb.min.x && hx <= rb.max.x && hz >= rb.min.z && hz <= rb.max.z {
                if best.map_or(true, |(_, bt, _, _)| t < bt) {
                    best = Some((i, t, hx, hz));
                }
            }
        }
        let Some((rb_index, _, hit_x, hit_z)) = best else {
            state.gui_state.construction_selected_room = None; // clicked empty space
            return;
        };
        // room_bounds and construction_rooms cross-walk by id.
        let id = state.gui_state.room_bounds[rb_index].id.clone();
        let Some(ri) = state.gui_state.construction_rooms.iter().position(|r| r.id == id) else {
            return;
        };
        let pos = state.gui_state.construction_rooms[ri].position.unwrap_or([0.0, 0.0, 0.0]);
        state.gui_state.construction_selected_room = Some(ri);
        state.construction_grab = Some(ConstructionGrab {
            room_index: ri,
            floor_y: state.gui_state.room_bounds[rb_index].min.y,
            offset_x: hit_x - pos[0],
            offset_z: hit_z - pos[2],
        });
    }

    /// Cast a ray from the cursor onto the room floors; return (room_bounds index, hit_x, hit_z) of
    /// the nearest room under the cursor. Used by ghost placement (v0.529).
    fn cursor_floor_hit(state: &EngineState) -> Option<(usize, f32, f32)> {
        let sz = state.window.inner_size();
        let (origin, dir) =
            state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        if dir.y.abs() < 1e-6 {
            return None;
        }
        let mut best: Option<(usize, f32, f32, f32)> = None; // (i, t, hx, hz)
        for (i, rb) in state.gui_state.room_bounds.iter().enumerate() {
            let t = (rb.min.y - origin.y) / dir.y;
            if t <= 0.0 {
                continue;
            }
            let hx = origin.x + dir.x * t;
            let hz = origin.z + dir.z * t;
            if hx >= rb.min.x && hx <= rb.max.x && hz >= rb.min.z && hz <= rb.max.z {
                if best.map_or(true, |(_, bt, _, _)| t < bt) {
                    best = Some((i, t, hx, hz));
                }
            }
        }
        best.map(|(i, _, hx, hz)| (i, hx, hz))
    }

    /// Drop the currently-held palette machine where the cursor hits a room floor. Keeps the item
    /// held so you can place several; right-click or re-click the palette item to stop. Appears live
    /// via construction_machines_dirty. (v0.529; v0.538: box mode stores ABSOLUTE coords)
    fn try_place_held_machine(state: &mut EngineState) {
        let Some(mtype) = state.gui_state.construction_place_type.clone() else {
            return;
        };
        let Some((rb_i, hx, hz)) = cursor_floor_hit(state) else {
            return;
        };
        let rb = &state.gui_state.room_bounds[rb_i];
        let room_id = rb.id.clone();
        // v0.538: in a HomeStructure box home, store the ABSOLUTE world floor-hit (world == box-local,
        // box min corner at origin), so the machine survives flood-fill room-id churn. The legacy
        // ship layout keeps the room-center-relative offset.
        let box_mode = state.gui_state.home_structure.is_some();
        let offset = if box_mode {
            (hx, 0.0, hz)
        } else {
            let cx = (rb.min.x + rb.max.x) * 0.5;
            let cz = (rb.min.z + rb.max.z) * 0.5;
            (hx - cx, 0.0, hz - cz)
        };
        if let Some(home) = state.gui_state.home_machines.as_mut() {
            if home.catalog.contains_key(&mtype) {
                let id = home.unique_instance_id(&mtype);
                home.instances.push(crate::machines::MachineInstance {
                    id,
                    machine: mtype,
                    room: room_id,
                    offset,
                });
                state.gui_state.construction_machines_dirty = true;
            }
        }
    }

    /// Drop the currently-held STRUCTURAL piece (stairs/ladder/elevator/...) where the cursor hits a
    /// room floor (v0.583). Stores an ABSOLUTE home-local pose (box min at origin) at the floor height,
    /// with the current placement yaw. Stays held so you can place several; right-click cancels.
    fn try_place_structure(state: &mut EngineState) {
        let Some(tid) = state.gui_state.construction_structure_type.clone() else {
            return;
        };
        if crate::ship::structure::structure_type(&tid).is_none() {
            return;
        }
        let Some((rb_i, hx, hz)) = cursor_floor_hit(state) else {
            return;
        };
        let floor_y = state.gui_state.room_bounds[rb_i].min.y;
        let place_y = floor_y + state.gui_state.construction_structure_place_y.max(0.0);
        if let Some(hs) = state.gui_state.home_structure.as_mut() {
            hs.structures.push(crate::ship::home_structure::PlacedStructure {
                type_id: tid,
                pos: (hx, place_y, hz),
                rot_deg: state.gui_state.construction_structure_yaw,
                pair: None,
            });
            state.gui_state.construction_structure_dirty = true;
        }
    }

    /// Drop a corner node while drawing an interior wall (v0.534). The first click sets the wall's
    /// start corner; the second click adds a wall segment from the start to here and CHAINS (the new
    /// corner becomes the next start), so you can walk a whole floor plan with successive clicks. The
    /// point comes from the floor raycast, snapped to 0.25 m. (World x/z equals box-local x/z because
    /// the box min corner sits at the world origin.)
    fn try_place_wall_node(state: &mut EngineState) {
        let Some((_, hx, hz)) = cursor_floor_hit(state) else {
            return;
        };
        // v0.541: snap a drawn corner to an existing corner / the box edge / the grid (same rules as
        // dragging), so successive walls share corners + reach the perimeter for an airtight seal.
        // (NaN "grabbed" sentinel skips nothing, so a new corner CAN snap onto an existing one.)
        let grid = state.gui_state.construction_grid_snap;
        let p = match state.gui_state.home_structure.as_ref() {
            Some(hs) => snap_node_position(hs, (f32::NAN, f32::NAN), (hx, hz), grid),
            None => ((hx * 4.0).round() / 4.0, (hz * 4.0).round() / 4.0),
        };
        match state.gui_state.construction_wall_start {
            None => state.gui_state.construction_wall_start = Some(p),
            Some(start) => {
                // Ignore a zero-length segment (a double-click on the same spot).
                if (start.0 - p.0).abs() > 0.05 || (start.1 - p.1).abs() > 0.05 {
                    if let Some(hs) = state.gui_state.home_structure.as_mut() {
                        let height = hs.height;
                        let material = hs.shell_material;
                        hs.walls.push(crate::ship::home_structure::InteriorWall {
                            a: start,
                            b: p,
                            height,
                            material,
                            openings: Vec::new(),
                            thickness: None,
                            layers: Vec::new(),
                        });
                        state.gui_state.construction_structure_dirty = true;
                        state.gui_state.construction_wall_selected = Some(hs.walls.len() - 1);
                        state.gui_state.construction_machine_selected = None; // keep selection exclusive
                    }
                    state.gui_state.construction_wall_start = Some(p); // chain into the next segment
                }
            }
        }
    }

    /// Every unique wall CORNER (deduped by position) -- the node set the gizmos + dragging act on.
    /// (v0.541)
    fn unique_corners(hs: &crate::ship::home_structure::HomeStructure) -> Vec<(f32, f32)> {
        let mut out: Vec<(f32, f32)> = Vec::new();
        for wall in &hs.walls {
            for c in [wall.a, wall.b] {
                if !out.iter().any(|o| (o.0 - c.0).abs() < 0.05 && (o.1 - c.1).abs() < 0.05) {
                    out.push(c);
                }
            }
        }
        out
    }

    /// Snap a dragged corner: to the nearest OTHER corner within 0.6 m (a shared node / airtight
    /// seal), else to the box perimeter if near an edge, else to the 0.25 m grid when grid snap is
    /// on. Always clamped into the box footprint. (v0.541)
    /// HSV (h,s,v in 0..1) -> linear RGB, for the gizmo colour cycle. (v0.562)
    fn hsv_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
        let i = (h * 6.0).floor();
        let f = h * 6.0 - i;
        let p = v * (1.0 - s);
        let q = v * (1.0 - f * s);
        let t = v * (1.0 - (1.0 - f) * s);
        match (i as i32).rem_euclid(6) {
            0 => (v, t, p),
            1 => (q, v, p),
            2 => (p, v, t),
            3 => (p, q, v),
            4 => (t, p, v),
            _ => (v, p, q),
        }
    }

    fn snap_node_position(
        hs: &crate::ship::home_structure::HomeStructure,
        grabbed: (f32, f32),
        raw: (f32, f32),
        grid: bool,
    ) -> (f32, f32) {
        // 1. Endpoint snap (strongest): another corner within 0.6 m, for shared nodes + seals.
        let mut best: Option<((f32, f32), f32)> = None;
        for wall in &hs.walls {
            for c in [wall.a, wall.b] {
                if (c.0 - grabbed.0).abs() < 0.05 && (c.1 - grabbed.1).abs() < 0.05 {
                    continue; // the grabbed node (+ its shared copies)
                }
                let dd = (c.0 - raw.0).powi(2) + (c.1 - raw.1).powi(2);
                if dd < 0.36 && best.map_or(true, |(_, b)| dd < b) {
                    best = Some((c, dd));
                }
            }
        }
        if let Some((c, _)) = best {
            // Snap onto the existing corner, quantized to the corner grid so the two become
            // BYTE-IDENTICAL (one orb, one draggable group; no overlapping-but-distinct duplicate).
            return crate::ship::home_structure::quantize_corner(c);
        }
        // 2. Grid snap, then edge snap to the box perimeter.
        let (w, d) = (hs.width, hs.depth);
        let mut x = raw.0;
        let mut z = raw.1;
        if grid {
            x = (x * 4.0).round() / 4.0;
            z = (z * 4.0).round() / 4.0;
        }
        if x < 0.5 {
            x = 0.0;
        } else if x > w - 0.5 {
            x = w;
        }
        if z < 0.5 {
            z = 0.0;
        } else if z > d - 0.5 {
            z = d;
        }
        crate::ship::home_structure::quantize_corner((x.clamp(0.0, w), z.clamp(0.0, d)))
    }

    /// Which build-mode gizmo the cursor is hovering this frame (v0.569), for the hover highlight.
    /// Mirrors the grab picks (try_grab_node/_char + the opening pick) but is read-only and picks the
    /// NEAREST gizmo across all three kinds. Returns None while drawing a wall, holding a machine, or
    /// already dragging (the grabbed one is highlighted instead). Generous pick radii since the orbs
    /// are tiny (0.05 m).
    fn compute_construction_hover(state: &EngineState) -> HoverGizmo {
        if !state.gui_state.construction_active
            || state.gui_state.construction_wall_mode
            || state.gui_state.construction_place_type.is_some()
            || state.gui_state.construction_structure_type.is_some()
            || state.construction_node_grab.is_some()
            || state.construction_opening_grab.is_some()
            || state.construction_char_grab
        {
            return HoverGizmo::None;
        }
        let Some(hs) = state.gui_state.home_structure.as_ref() else {
            return HoverGizmo::None;
        };
        let sz = state.window.inner_size();
        let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        // Closest approach of the ray to a point p, returning its forward distance t if within pick_r.
        let test = |p: Vec3, pick_r: f32| -> Option<f32> {
            let t = (p - origin).dot(dir);
            if t < 0.0 {
                return None;
            }
            if (p - (origin + dir * t)).length() < pick_r { Some(t) } else { None }
        };
        let mut best_t = f32::INFINITY;
        let mut best = HoverGizmo::None;
        for c in unique_corners(hs) {
            if let Some(t) = test(Vec3::new(c.0, -0.05, c.1), 0.45) {
                if t < best_t {
                    best_t = t;
                    best = HoverGizmo::Corner(c.0, c.1);
                }
            }
        }
        for (idx, p) in opening_gizmos(hs) {
            if let Some(t) = test(p, 0.4) {
                if t < best_t {
                    best_t = t;
                    best = HoverGizmo::Opening(idx.0, idx.1);
                }
            }
        }
        if let Some((cx, cz)) = state.gui_state.build_char_pos {
            if let Some(t) = test(Vec3::new(cx, 0.7, cz), 0.7) {
                if t < best_t {
                    best = HoverGizmo::Char;
                }
            }
        }
        best
    }

    /// On a build-mode click, try to grab the nearest corner-node gizmo under the cursor (ray vs the
    /// pin position). Returns true if a node was grabbed. (v0.541)
    fn try_grab_node(state: &mut EngineState) -> bool {
        // Compute the gizmo set as owned values so the home_structure borrow ends before the
        // mutable grab assignment below.
        let (top_y, corners) = match state.gui_state.home_structure.as_ref() {
            Some(hs) => (-0.05, unique_corners(hs)), // orb centre (top-at-floor); matches the render (v0.568)
            None => return false,
        };
        let sz = state.window.inner_size();
        // pick_ray already returns a unit dir (or zero for a degenerate ray); re-normalizing a zero
        // vector would be NaN, so use it as-is. (v0.542)
        let (origin, dir) =
            state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        let mut best: Option<((f32, f32), f32)> = None;
        for c in &corners {
            let p = Vec3::new(c.0, top_y, c.1);
            let t = (p - origin).dot(dir);
            if t < 0.0 {
                continue; // behind the camera
            }
            let dd = (p - (origin + dir * t)).length();
            if dd < 0.7 && best.map_or(true, |(_, b)| dd < b) {
                best = Some((*c, dd));
            }
        }
        if let Some((c, _)) = best {
            state.construction_node_grab = Some(c);
            state.construction_grab_press = Some(state.cursor_pos); // tap-vs-drag (v0.549)
            true
        } else {
            false
        }
    }

    /// Hit-test the cursor ray against the placed machines (v0.553). On a hit, SELECT the nearest one
    /// (its detail shows on the right panel) and clear any wall selection; returns true so the click
    /// does not also start a room grab. Build mode only.
    fn try_pick_machine(state: &mut EngineState) -> bool {
        if state.machine_pick.is_empty() {
            return false;
        }
        let sz = state.window.inner_size();
        let (origin, dir) =
            state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        let mut best: Option<(String, f32)> = None;
        for (id, center, radius) in &state.machine_pick {
            let t = (*center - origin).dot(dir);
            if t < 0.0 {
                continue; // behind the camera
            }
            let dd = (*center - (origin + dir * t)).length();
            // Within the machine's bounding radius; keep the one nearest the camera (smallest t).
            if dd < *radius && best.as_ref().map_or(true, |(_, bt)| t < *bt) {
                best = Some((id.clone(), t));
            }
        }
        if let Some((id, _)) = best {
            state.gui_state.construction_machine_selected = Some(id);
            state.gui_state.construction_wall_selected = None;
            state.gui_state.construction_light_selected = None;
            true
        } else {
            false
        }
    }

    /// Hit-test the cursor ray against the WALL SURFACES (v0.573). On a hit, SELECT that wall (its
    /// corners/openings show on the right panel) -- so clicking anywhere on a wall's face picks it,
    /// unambiguously, instead of having to click a shared corner orb at a multi-wall intersection.
    /// Each interior wall is a vertical slab; we intersect the ray with its centre plane and check the
    /// hit lies within the wall's length + height. Returns true (so the click doesn't also grab a room).
    fn try_pick_wall(state: &mut EngineState) -> bool {
        let walls: Vec<(usize, Vec3, Vec3, f32)> = match state.gui_state.home_structure.as_ref() {
            Some(hs) => hs
                .walls
                .iter()
                .enumerate()
                .map(|(i, w)| (i, Vec3::new(w.a.0, 0.0, w.a.1), Vec3::new(w.b.0, 0.0, w.b.1), w.height))
                .collect(),
            None => return false,
        };
        let sz = state.window.inner_size();
        let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        let mut best: Option<(usize, f32)> = None; // (wall index, ray t)
        for (i, a, b, h) in &walls {
            let along = *b - *a;
            let len = along.length();
            if len < 1e-4 {
                continue;
            }
            let along_n = along / len;
            // Horizontal normal of the (vertical) wall plane.
            let normal = Vec3::new(-along_n.z, 0.0, along_n.x);
            let denom = dir.dot(normal);
            if denom.abs() < 1e-6 {
                continue; // ray parallel to the wall face
            }
            let t = (*a - origin).dot(normal) / denom;
            if t < 0.0 {
                continue; // behind the camera
            }
            let hit = origin + dir * t;
            let s = (hit - *a).dot(along_n); // distance along the wall from a
            if s >= -0.1 && s <= len + 0.1 && hit.y >= -0.1 && hit.y <= *h + 0.1 {
                if best.map_or(true, |(_, bt)| t < bt) {
                    best = Some((*i, t));
                }
            }
        }
        if let Some((i, _)) = best {
            state.gui_state.construction_wall_selected = Some(i);
            state.gui_state.construction_machine_selected = None;
            state.gui_state.construction_light_selected = None;
            true
        } else {
            false
        }
    }

    /// Hit-test the cursor ray against the placed-LIGHT diamond gizmos (v0.576). On a hit, SELECT that
    /// light (its detail shows on the right panel, like a wall). Returns true so the click doesn't also
    /// pick a wall / grab a room.
    fn try_pick_light(state: &mut EngineState) -> bool {
        let lights: Vec<(usize, Vec3)> = match state.gui_state.home_structure.as_ref() {
            Some(hs) => hs
                .lights
                .iter()
                .enumerate()
                .map(|(i, l)| (i, Vec3::new(l.pos.0, l.pos.1, l.pos.2)))
                .collect(),
            None => return false,
        };
        if lights.is_empty() {
            return false;
        }
        let sz = state.window.inner_size();
        let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        let mut best: Option<(usize, f32)> = None;
        for (i, p) in &lights {
            let t = (*p - origin).dot(dir);
            if t < 0.0 {
                continue;
            }
            let dd = (*p - (origin + dir * t)).length();
            if dd < 0.4 && best.map_or(true, |(_, bt)| t < bt) {
                best = Some((*i, t));
            }
        }
        if let Some((i, _)) = best {
            state.gui_state.construction_light_selected = Some(i);
            state.gui_state.construction_wall_selected = None;
            state.gui_state.construction_machine_selected = None;
            true
        } else {
            false
        }
    }

    /// Ray vs axis-aligned box (the slab method): the ray `origin + t*dir` against the box [min,max].
    /// Returns the nearest positive `t` of entry, or None if the ray misses / the box is behind. Used
    /// to pick placed structures by their bounding box. (v0.583)
    fn ray_aabb_hit(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
        let mut tmin = 0.0_f32;
        let mut tmax = f32::INFINITY;
        for a in 0..3 {
            let (o, d, lo, hi) = (origin[a], dir[a], min[a], max[a]);
            if d.abs() < 1e-8 {
                if o < lo || o > hi {
                    return None; // parallel + outside the slab
                }
            } else {
                let inv = 1.0 / d;
                let mut t1 = (lo - o) * inv;
                let mut t2 = (hi - o) * inv;
                if t1 > t2 {
                    std::mem::swap(&mut t1, &mut t2);
                }
                tmin = tmin.max(t1);
                tmax = tmax.min(t2);
                if tmin > tmax {
                    return None;
                }
            }
        }
        if tmax < 0.0 {
            None
        } else {
            Some(tmin)
        }
    }

    /// Hit-test the cursor ray against the placed STRUCTURE pieces (v0.583). On a hit, SELECT that
    /// piece (its detail shows on the right panel). Uses a ray-vs-AABB test against each piece's
    /// rotated bounding box so clicking the visible body (the elevator frame, the stair mass) selects
    /// it. Returns true so the click doesn't also pick a wall / grab a room.
    fn try_pick_structure(state: &mut EngineState) -> bool {
        use crate::ship::structure::{rotated_half_extents, structure_type, StructureKind};
        let pieces: Vec<(usize, Vec3, Vec3)> = match state.gui_state.home_structure.as_ref() {
            Some(hs) => hs
                .structures
                .iter()
                .enumerate()
                .filter_map(|(i, ps)| {
                    let ty = structure_type(&ps.type_id)?;
                    if ty.kind == StructureKind::Wall {
                        return None;
                    }
                    let (hw, h, hd) = rotated_half_extents(ty, ps.rot_deg.to_radians());
                    let min = Vec3::new(ps.pos.0 - hw, ps.pos.1, ps.pos.2 - hd);
                    let max = Vec3::new(ps.pos.0 + hw, ps.pos.1 + h, ps.pos.2 + hd);
                    Some((i, min, max))
                })
                .collect(),
            None => return false,
        };
        if pieces.is_empty() {
            return false;
        }
        let sz = state.window.inner_size();
        let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        let mut best: Option<(usize, f32)> = None;
        for (i, min, max) in &pieces {
            if let Some(t) = ray_aabb_hit(origin, dir, *min, *max) {
                if best.map_or(true, |(_, bt)| t < bt) {
                    best = Some((*i, t));
                }
            }
        }
        if let Some((i, _)) = best {
            state.gui_state.construction_structure_selected = Some(i);
            state.gui_state.construction_wall_selected = None;
            state.gui_state.construction_machine_selected = None;
            state.gui_state.construction_light_selected = None;
            true
        } else {
            false
        }
    }

    /// Hit-test the cursor ray against the build-mode avatar (v0.557). On a hit, start dragging it;
    /// returns true so the click doesn't also grab a room.
    fn try_grab_char(state: &mut EngineState) -> bool {
        let Some((cx, cz)) = state.gui_state.build_char_pos else {
            return false;
        };
        let sz = state.window.inner_size();
        let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        let c = Vec3::new(cx, 0.7, cz); // mid-body
        let t = (c - origin).dot(dir);
        if t < 0.0 {
            return false;
        }
        if (c - (origin + dir * t)).length() < 0.8 {
            state.construction_char_grab = true;
            true
        } else {
            false
        }
    }

    /// Per-frame while the avatar is grabbed: move it to the cursor's floor hit, clamped into the box.
    fn apply_char_drag(state: &mut EngineState) {
        if let Some((_, hx, hz)) = cursor_floor_hit(state) {
            let (bw, bd) = state
                .gui_state
                .home_structure
                .as_ref()
                .map_or((1e6, 1e6), |hs| (hs.width, hs.depth));
            state.gui_state.build_char_pos = Some((hx.clamp(0.3, bw - 0.3), hz.clamp(0.3, bd - 0.3)));
        }
    }

    /// Per-frame while a corner node is grabbed: raycast to the floor, snap, and move EVERY wall
    /// endpoint at the grabbed position to the snapped one (so shared corners move together). (v0.541)
    /// Pixels the cursor must travel from the press point before a gizmo grab becomes a DRAG (v0.549).
    const DRAG_THRESHOLD_PX: f32 = 6.0;

    fn apply_node_drag(state: &mut EngineState) {
        let Some(grabbed) = state.construction_node_grab else {
            return;
        };
        // Tap-vs-drag (v0.549): hold the corner still until the cursor leaves the press point, so a
        // tap selects (handled on release) and only click-and-drag moves it.
        if let Some(press) = state.construction_grab_press {
            let d = ((state.cursor_pos.0 - press.0).powi(2) + (state.cursor_pos.1 - press.1).powi(2)).sqrt();
            if d < DRAG_THRESHOLD_PX {
                return;
            }
            state.construction_grab_press = None; // armed: this is now a drag
        }
        let Some((_, hx, hz)) = cursor_floor_hit(state) else {
            return;
        };
        let grid = state.gui_state.construction_grid_snap;
        let snapped = match state.gui_state.home_structure.as_ref() {
            Some(hs) => snap_node_position(hs, grabbed, (hx, hz), grid),
            None => return,
        };
        if (snapped.0 - grabbed.0).abs() < 1e-4 && (snapped.1 - grabbed.1).abs() < 1e-4 {
            return; // no movement this frame
        }
        if let Some(hs) = state.gui_state.home_structure.as_mut() {
            for wall in hs.walls.iter_mut() {
                if (wall.a.0 - grabbed.0).abs() < 0.05 && (wall.a.1 - grabbed.1).abs() < 0.05 {
                    wall.a = snapped;
                }
                if (wall.b.0 - grabbed.0).abs() < 0.05 && (wall.b.1 - grabbed.1).abs() < 0.05 {
                    wall.b = snapped;
                }
            }
        }
        state.construction_node_grab = Some(snapped);
        state.gui_state.construction_structure_dirty = true;
    }

    /// World positions of every door/window opening gizmo: ((wall index, opening index), centre).
    /// (v0.546)
    fn opening_gizmos(hs: &crate::ship::home_structure::HomeStructure) -> Vec<((usize, usize), Vec3)> {
        let mut out = Vec::new();
        for (wi, wall) in hs.walls.iter().enumerate() {
            let (ax, az) = wall.a;
            let (dx, dz) = (wall.b.0 - ax, wall.b.1 - az);
            let len = (dx * dx + dz * dz).sqrt();
            if len < 1e-4 {
                continue;
            }
            let (ux, uz) = (dx / len, dz / len);
            for (oi, op) in wall.openings.iter().enumerate() {
                let s = (op.at + op.width * 0.5).clamp(0.0, len);
                let cy = op.sill + op.height * 0.5;
                out.push(((wi, oi), Vec3::new(ax + ux * s, cy, az + uz * s)));
            }
        }
        out
    }

    /// On a build-mode click, try to grab the nearest door/window opening gizmo (ray vs the cube).
    /// Returns true if one was grabbed. (v0.546)
    fn try_grab_opening(state: &mut EngineState) -> bool {
        let gizmos = match state.gui_state.home_structure.as_ref() {
            Some(hs) => opening_gizmos(hs),
            None => return false,
        };
        if gizmos.is_empty() {
            return false;
        }
        let sz = state.window.inner_size();
        let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        let mut best: Option<((usize, usize), f32)> = None;
        for (id, p) in &gizmos {
            let t = (*p - origin).dot(dir);
            if t < 0.0 {
                continue;
            }
            let dd = (*p - (origin + dir * t)).length();
            if dd < 0.5 && best.map_or(true, |(_, b)| dd < b) {
                best = Some((*id, dd));
            }
        }
        if let Some((id, _)) = best {
            state.construction_opening_grab = Some(id);
            state.construction_grab_press = Some(state.cursor_pos); // tap-vs-drag (v0.549)
            true
        } else {
            false
        }
    }

    /// Per-frame while an opening gizmo is grabbed: project the cursor onto that opening's wall and
    /// slide the opening ALONG it (update `at`), grid-snapped + clamped within the wall. (v0.546)
    fn apply_opening_drag(state: &mut EngineState) {
        let Some((wi, oi)) = state.construction_opening_grab else {
            return;
        };
        // Tap-vs-drag (v0.549): hold until the cursor leaves the press point; a tap selects.
        if let Some(press) = state.construction_grab_press {
            let d = ((state.cursor_pos.0 - press.0).powi(2) + (state.cursor_pos.1 - press.1).powi(2)).sqrt();
            if d < DRAG_THRESHOLD_PX {
                return;
            }
            state.construction_grab_press = None;
        }
        let Some((_, hx, hz)) = cursor_floor_hit(state) else {
            return;
        };
        let grid = state.gui_state.construction_grid_snap;
        if let Some(hs) = state.gui_state.home_structure.as_mut() {
            let Some(wall) = hs.walls.get_mut(wi) else {
                return;
            };
            let (ax, az) = wall.a;
            let (dx, dz) = (wall.b.0 - ax, wall.b.1 - az);
            let len = (dx * dx + dz * dz).sqrt();
            if len < 1e-4 {
                return;
            }
            let (ux, uz) = (dx / len, dz / len);
            let mut along = ((hx - ax) * ux + (hz - az) * uz).clamp(0.0, len);
            if grid {
                along = (along * 4.0).round() / 4.0;
            }
            if let Some(op) = wall.openings.get_mut(oi) {
                let half = op.width * 0.5;
                op.at = (along - half).clamp(0.0, (len - op.width).max(0.0));
            }
        }
        state.gui_state.construction_structure_dirty = true;
    }

    /// World positions of every opening RESIZE handle (v0.578): 4 per opening at the aperture edges --
    /// left/right (mid-height) resize width, top/bottom (mid-width) resize height. Returns
    /// ((wall, opening, edge), pos) with edge 0=left 1=right 2=top 3=bottom.
    fn opening_resize_handles(hs: &crate::ship::home_structure::HomeStructure) -> Vec<((usize, usize, u8), Vec3)> {
        let mut out = Vec::new();
        for (wi, wall) in hs.walls.iter().enumerate() {
            let (ax, az) = wall.a;
            let (dx, dz) = (wall.b.0 - ax, wall.b.1 - az);
            let len = (dx * dx + dz * dz).sqrt();
            if len < 1e-4 {
                continue;
            }
            let (ux, uz) = (dx / len, dz / len);
            for (oi, op) in wall.openings.iter().enumerate() {
                let s_l = op.at.clamp(0.0, len);
                let s_r = (op.at + op.width).clamp(0.0, len);
                let s_c = (op.at + op.width * 0.5).clamp(0.0, len);
                let cy_c = op.sill + op.height * 0.5;
                out.push(((wi, oi, 0), Vec3::new(ax + ux * s_l, cy_c, az + uz * s_l)));
                out.push(((wi, oi, 1), Vec3::new(ax + ux * s_r, cy_c, az + uz * s_r)));
                out.push(((wi, oi, 2), Vec3::new(ax + ux * s_c, op.sill + op.height, az + uz * s_c)));
                out.push(((wi, oi, 3), Vec3::new(ax + ux * s_c, op.sill, az + uz * s_c)));
            }
        }
        out
    }

    /// On a build-mode click, try to grab an opening RESIZE handle under the cursor (v0.578). Returns
    /// true (so the click doesn't also grab the move-cube or a wall).
    fn try_grab_opening_resize(state: &mut EngineState) -> bool {
        let handles = match state.gui_state.home_structure.as_ref() {
            Some(hs) => opening_resize_handles(hs),
            None => return false,
        };
        if handles.is_empty() {
            return false;
        }
        let sz = state.window.inner_size();
        let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        let mut best: Option<((usize, usize, u8), f32)> = None;
        for (id, p) in &handles {
            let t = (*p - origin).dot(dir);
            if t < 0.0 {
                continue;
            }
            let dd = (*p - (origin + dir * t)).length();
            if dd < 0.3 && best.map_or(true, |(_, b)| dd < b) {
                best = Some((*id, dd));
            }
        }
        if let Some((id, _)) = best {
            state.construction_opening_resize = Some(id);
            state.gui_state.construction_wall_selected = Some(id.0); // show the wall on the panel
            true
        } else {
            false
        }
    }

    /// Per-frame while an opening resize handle is grabbed (v0.578): left/right project the cursor onto
    /// the wall axis (resize width, keeping the opposite edge fixed); top/bottom intersect the cursor
    /// ray with the wall plane and take its height (resize height/sill). Grid-snapped, min 0.2 m.
    fn apply_opening_resize(state: &mut EngineState) {
        let Some((wi, oi, edge)) = state.construction_opening_resize else {
            return;
        };
        // Copy the wall axis to locals so the cursor calls below don't conflict with the home borrow.
        let (ax, az, ux, uz, len, wall_h) = match state.gui_state.home_structure.as_ref().and_then(|hs| hs.walls.get(wi)) {
            Some(wall) => {
                let (ax, az) = wall.a;
                let (dx, dz) = (wall.b.0 - ax, wall.b.1 - az);
                let len = (dx * dx + dz * dz).sqrt();
                if len < 1e-4 {
                    return;
                }
                (ax, az, dx / len, dz / len, len, wall.height)
            }
            None => return,
        };
        let grid = state.gui_state.construction_grid_snap;
        let val = if edge <= 1 {
            let Some((_, hx, hz)) = cursor_floor_hit(state) else {
                return;
            };
            let mut along = ((hx - ax) * ux + (hz - az) * uz).clamp(0.0, len);
            if grid {
                along = (along * 4.0).round() / 4.0;
            }
            along
        } else {
            let sz = state.window.inner_size();
            let (origin, ddir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
            let normal = Vec3::new(-uz, 0.0, ux);
            let denom = ddir.dot(normal);
            if denom.abs() < 1e-6 {
                return;
            }
            let t = (Vec3::new(ax, 0.0, az) - origin).dot(normal) / denom;
            if t < 0.0 {
                return;
            }
            let mut y = (origin + ddir * t).y.clamp(0.0, wall_h);
            if grid {
                y = (y * 4.0).round() / 4.0;
            }
            y
        };
        if let Some(op) = state
            .gui_state
            .home_structure
            .as_mut()
            .and_then(|hs| hs.walls.get_mut(wi))
            .and_then(|w| w.openings.get_mut(oi))
        {
            match edge {
                0 => {
                    let right = op.at + op.width;
                    let at = val.min(right - 0.2).max(0.0);
                    op.width = right - at;
                    op.at = at;
                }
                1 => op.width = (val - op.at).max(0.2).min(len - op.at),
                2 => op.height = (val - op.sill).max(0.2),
                3 => {
                    let top = op.sill + op.height;
                    let sill = val.min(top - 0.2).max(0.0);
                    op.height = top - sill;
                    op.sill = sill;
                }
                _ => {}
            }
        }
        state.gui_state.construction_structure_dirty = true;
    }

    /// Per-frame: while a room is grabbed, intersect the pick ray with its floor plane, move
    /// the room so it follows the cursor (minus the grab offset), snap to 0.25 m, and flag a
    /// rebuild. Computed from the live cursor (not deltas) so it never drifts. (v0.466)
    fn apply_room_drag(state: &mut EngineState) {
        let Some(grab) = state.construction_grab else { return; };
        let sz = state.window.inner_size();
        let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        if dir.y.abs() < 1e-6 {
            return;
        }
        let t = (grab.floor_y - origin.y) / dir.y;
        if t <= 0.0 {
            return;
        }
        let hit_x = origin.x + dir.x * t;
        let hit_z = origin.z + dir.z * t;
        let snap = |v: f32| (v / 0.25).round() * 0.25;
        let new_x = snap(hit_x - grab.offset_x);
        let new_z = snap(hit_z - grab.offset_z);
        if let Some(room) = state.gui_state.construction_rooms.get_mut(grab.room_index) {
            let mut p = room.position.unwrap_or([0.0, 0.0, 0.0]);
            if (p[0] - new_x).abs() > f32::EPSILON || (p[2] - new_z).abs() > f32::EPSILON {
                p[0] = new_x;
                p[2] = new_z;
                room.position = Some(p);
                state.gui_state.construction_dirty = true;
            }
        }
    }

    /// Per-frame: while an opening handle is grabbed, intersect the pick ray with the wall-FACE
    /// plane, decompose the hit into (u along the wall, v up the wall), and apply the grab role.
    /// A placed opening (Some) moves/resizes its `openings[i]`; a legacy face (None) slides its
    /// `wall_offsets`. Everything clamps to the wall, so the panel value equals the real on-wall
    /// placement (the 20m-vs-2m fix). Computed from the live cursor so it never drifts. (v0.469)
    fn apply_gizmo_drag(state: &mut EngineState) {
        let Some(g) = state.construction_gizmo_grab else { return; };
        let sz = state.window.inner_size();
        let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        // Ray vs the wall's vertical plane through wall_start with normal n.
        let denom = dir.dot(g.n);
        if denom.abs() < 1e-6 {
            return;
        }
        let t = (g.wall_start - origin).dot(g.n) / denom;
        if t <= 0.0 {
            return;
        }
        let hit = origin + dir * t;
        let rel = hit - g.wall_start;
        let u_raw = rel.dot(g.u_hat); // metres along the wall from the start corner
        let v_raw = rel.y; // metres up from the floor (wall_start is at floor y)
        let snap = |x: f32| (x / 0.1).round() * 0.1;
        let len = g.wall_len;
        let wh = g.wall_height;

        let Some(room) = state.gui_state.construction_rooms.get_mut(g.room_index) else { return; };

        // Legacy WallKind slide (no placed opening): write wall_offsets, build clamps the rest.
        let Some(oi) = g.opening_index else {
            let u_clamped = u_raw.clamp(g.grab_w * 0.5, (len - g.grab_w * 0.5).max(g.grab_w * 0.5));
            let new_off = snap(u_clamped - g.base_t);
            if (room.wall_offsets[g.wall_index] - new_off).abs() > f32::EPSILON {
                room.wall_offsets[g.wall_index] = new_off;
                state.gui_state.construction_dirty = true;
            }
            return;
        };
        let Some(op) = room.openings.get_mut(oi) else { return; };

        let before = *op;
        match g.role {
            GizmoRole::Move => {
                let hw = op.w * 0.5;
                op.u = snap(u_raw).clamp(hw, (len - hw).max(hw));
                if g.snap_floor {
                    op.v = op.h * 0.5;
                } else {
                    let hh = op.h * 0.5;
                    op.v = snap(v_raw).clamp(hh, (wh - hh).max(hh));
                }
            }
            GizmoRole::ResizeRight => {
                let left = (g.grab_u - g.grab_w * 0.5).max(0.0);
                let right = snap(u_raw).clamp(left + 0.3, len);
                op.w = right - left;
                op.u = left + op.w * 0.5;
            }
            GizmoRole::ResizeLeft => {
                let right = (g.grab_u + g.grab_w * 0.5).min(len);
                let left = snap(u_raw).clamp(0.0, right - 0.3);
                op.w = right - left;
                op.u = left + op.w * 0.5;
            }
            GizmoRole::ResizeTop => {
                let bottom = (g.grab_v - g.grab_h * 0.5).max(0.0);
                let top = snap(v_raw).clamp(bottom + 0.3, wh);
                op.h = top - bottom;
                op.v = bottom + op.h * 0.5;
            }
            GizmoRole::ResizeBottom => {
                let top = (g.grab_v + g.grab_h * 0.5).min(wh);
                let bottom = snap(v_raw).clamp(0.0, top - 0.3);
                op.h = top - bottom;
                op.v = bottom + op.h * 0.5;
            }
        }
        if *op != before {
            state.gui_state.construction_dirty = true;
        }
    }

    /// Route a `__game__:`-tagged relay message into the multiplayer sync system (v0.472).
    /// `payload` is the JSON AFTER the `__game__:` prefix. Maps the relay's `game_*` wire types
    /// (game_welcome / game_player_joined / game_position_update / game_player_left) to NetMessage
    /// and queues them for `net_sync` to apply. Other game_* events (quests, perception) are
    /// ignored here -- they are not part of co-presence. Reuses the authenticated chat socket.
    fn route_game_message(state: &mut EngineState, payload: &str) {
        use crate::net::protocol::NetMessage;
        let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) else { return; };
        let arr3 = |val: &serde_json::Value| -> Option<[f32; 3]> {
            let a = val.as_array()?;
            if a.len() != 3 { return None; }
            Some([a[0].as_f64()? as f32, a[1].as_f64()? as f32, a[2].as_f64()? as f32])
        };
        let arr4 = |val: &serde_json::Value| -> Option<[f32; 4]> {
            let a = val.as_array()?;
            if a.len() != 4 { return None; }
            Some([a[0].as_f64()? as f32, a[1].as_f64()? as f32, a[2].as_f64()? as f32, a[3].as_f64()? as f32])
        };
        match v.get("type").and_then(|t| t.as_str()) {
            Some("game_welcome") => {
                if let Some(id) = v.get("player_id").and_then(|x| x.as_u64()) {
                    let own_id = id as u32;
                    // Welcome first (sets our local_player_id so the self-filter +
                    // idempotency in NetSyncSystem work for the entries below).
                    let mut msgs = vec![NetMessage::Welcome {
                        player_id: own_id,
                        world_snapshot: Vec::new(),
                    }];
                    // World-snapshot prefill (v0.474): the relay's welcome carries
                    // every current entity. Spawn the OTHER players right away so a
                    // joiner sees players who are already present even if they never
                    // move (previously they only appeared on their next position
                    // update -- two stationary players were invisible to each other).
                    if let Some(snap) = v.get("world_snapshot").and_then(|s| s.as_array()) {
                        for e in snap {
                            if e.get("entity_type").and_then(|t| t.as_str()) != Some("player") {
                                continue;
                            }
                            let Some(eid) = e.get("entity_id").and_then(|x| x.as_u64()) else { continue; };
                            if eid as u32 == own_id {
                                continue; // skip ourselves
                            }
                            let Some(pos) = e.get("position").and_then(&arr3) else { continue; };
                            msgs.push(NetMessage::PlayerJoined {
                                player_id: eid as u32,
                                name: "Player".to_string(),
                                position: pos,
                            });
                        }
                    }
                    state.net_sync.queue_messages(msgs);
                }
            }
            Some("game_player_joined") => {
                if let (Some(id), Some(pos)) = (
                    v.get("player_id").and_then(|x| x.as_u64()),
                    v.get("position").and_then(&arr3),
                ) {
                    let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("Player").to_string();
                    state.net_sync.queue_messages(vec![NetMessage::PlayerJoined {
                        player_id: id as u32,
                        name,
                        position: pos,
                    }]);
                }
            }
            Some("game_position_update") => {
                if let (Some(id), Some(pos)) = (
                    v.get("player_id").and_then(|x| x.as_u64()),
                    v.get("position").and_then(&arr3),
                ) {
                    let rotation = v.get("rotation").and_then(&arr4).unwrap_or([0.0, 0.0, 0.0, 1.0]);
                    let velocity = v.get("velocity").and_then(&arr3).unwrap_or([0.0, 0.0, 0.0]);
                    let timestamp = v.get("timestamp").and_then(|x| x.as_f64()).unwrap_or(0.0);
                    state.net_sync.queue_messages(vec![NetMessage::PositionUpdate {
                        player_id: id as u32,
                        position: pos,
                        rotation,
                        velocity,
                        timestamp,
                    }]);
                }
            }
            Some("game_player_left") => {
                if let Some(id) = v.get("player_id").and_then(|x| x.as_u64()) {
                    state.net_sync.queue_messages(vec![NetMessage::PlayerLeft { player_id: id as u32 }]);
                }
            }
            // Game admin (v0.474): the relay's private reply to a
            // game_banned_list_request. Admin-only by construction (targeted at
            // the requesting admin). Populates the Game Admin page list.
            Some("game_banned_list") => {
                if let Some(arr) = v.get("users") {
                    if let Ok(bans) = serde_json::from_value::<Vec<crate::relay::storage::GameBan>>(arr.clone()) {
                        state.gui_state.game_bans = bans;
                    }
                }
            }
            // The relay refused our own join (we are game-banned). Surface it; do
            // NOT touch chat (it stays connected by design).
            Some("game_join_denied") => {
                let reason = v.get("reason").and_then(|x| x.as_str()).unwrap_or("");
                let msg = v.get("message").and_then(|x| x.as_str())
                    .unwrap_or("You are banned from the game world. Chat is unaffected.");
                state.gui_state.game_admin_status = if reason.is_empty() {
                    msg.to_string()
                } else {
                    format!("{msg} ({reason})")
                };
                log::warn!("Game-join denied: {msg} (reason: {reason})");
            }
            Some("game_admin_error") => {
                if let Some(m) = v.get("message").and_then(|x| x.as_str()) {
                    state.gui_state.game_admin_status = m.to_string();
                }
            }
            _ => {}
        }
    }

    /// Send the local player's position to the relay (reused chat socket). Throttled by the caller.
    /// The relay validates (anti-teleport) and broadcasts `game_position_update` to other clients.
    fn send_game_position(state: &EngineState) {
        let Some(ref ws) = state.gui_state.ws_client else { return; };
        let p = state.camera.position;
        // Yaw-only facing quaternion (rotation about Y): enough for avatars to face their heading.
        let half = state.camera.yaw * 0.5;
        let (qy, qw) = (half.sin(), half.cos());
        let msg = serde_json::json!({
            "type": "game_position_update",
            "position": [p.x, p.y, p.z],
            "rotation": [0.0, qy, 0.0, qw],
            "velocity": [0.0, 0.0, 0.0],
            "timestamp": 0.0,
        });
        ws.send(&msg.to_string());
    }

    /// Lazy-load the 3D world: homestead, hologram, stars, planet, CSV data.
    /// Called once on first Enter World. Keeps app startup instant (chat-first).
    fn load_world(state: &mut EngineState) {
        log::info!("Loading 3D world...");
        let load_start = Instant::now();

        // ── Homestead meshes ── (v0.455: load the LAYOUT, keep it for the construction
        // editor, then generate + upload meshes through the shared path.)
        // v0.534: prefer the new HomeStructure model (a FIXED outer box + freely-designed interior
        // walls) for the home; fall back to the legacy AABB-room layout if home_structure.ron is
        // absent. Both produce HomesteadMeshes, so the render path is identical.
        let hs_path = state.data_dir.join("blueprints").join("home_structure.ron");
        let (homestead, room_info) =
            if let Some(hs) = crate::ship::home_structure::HomeStructure::load(&hs_path) {
                let meshes = hs.generate_meshes();
                let info = meshes.room_info.clone();
                // Restore the persisted build-mode spawn point (v0.582).
                if let Some(sp) = hs.spawn {
                    state.gui_state.build_char_pos = Some(sp);
                }
                state.gui_state.home_structure = Some(hs);
                (meshes, info)
            } else {
                let layout = crate::ship::fibonacci::load_layout_or_fallback();
                let meshes = crate::ship::fibonacci::generate_from_layout(&layout);
                let info = meshes.room_info.clone();
                state.homestead_layout = Some(layout);
                (meshes, info)
            };
        // Wall collision segments so the player can't walk through walls from the first frame (v0.556).
        state.wall_colliders = match &state.gui_state.home_structure {
            Some(hs) => crate::ship::wall_collision::wall_segments(hs),
            None => Vec::new(),
        };
        apply_homestead_meshes(state, homestead);

        // Room ceiling lights
        let auto_lights = room_info.iter().map(|r| {
            let light_pos = Vec3::new(r.center.x, r.center.y + r.dimensions.y * 0.5 - 0.1, r.center.z);
            let room_size = r.dimensions.x.max(r.dimensions.z);
            let intensity = (room_size * 0.5).clamp(2.0, 15.0);
            let radius = room_size * 1.5;
            (light_pos, [1.0, 0.95, 0.85], intensity, radius)
        }).collect();
        // v0.571: placed lights override the auto synthesis (empty -> auto).
        state.room_lights = home_lights(state.gui_state.home_structure.as_ref(), auto_lights, state.gui_state.gi_enabled);

        // Sealed-volume AABB (encompasses every room) for the survival environment
        // context — inside it the player is sealed/oxygenated, outside = vacuum.
        state.homestead_bounds = room_info.iter().fold(None, |acc, r| {
            let rmin = r.center - r.dimensions * 0.5;
            let rmax = r.center + r.dimensions * 0.5;
            Some(match acc {
                None => (rmin, rmax),
                Some((mn, mx)) => (mn.min(rmin), mx.max(rmax)),
            })
        });

        // Hologram + spawn rooms
        let hologram_room_center = room_info.iter()
            .find(|r| r.is_hologram_room)
            .map(|r| r.center);
        let spawn_room = room_info.iter()
            .find(|r| r.is_spawn_room);
        state.hologram_room_center = hologram_room_center.unwrap_or(Vec3::new(-0.5, 1.0, 2.5));

        // Camera spawn position
        if let Some(spawn) = spawn_room {
            state.camera.position = Vec3::new(spawn.center.x, 1.7, spawn.center.z + spawn.dimensions.z * 0.35);
            state.camera.pitch = -0.2;
            state.camera.yaw = std::f32::consts::PI;
        } else if let Some(holo_center) = hologram_room_center {
            state.camera.position = Vec3::new(holo_center.x, 1.7, holo_center.z + 1.5);
            state.camera.pitch = -0.2;
            state.camera.yaw = std::f32::consts::PI;
        }

        log::info!("Homestead: {} rooms, {} floors, walls: {}, {} lights",
            room_info.len(), state.homestead_floors.len(),
            state.homestead_walls.is_some(), state.room_lights.len());

        // Clear the per-frame object lists before (re)populating them this load. The old aeroponic
        // tower placeholders (a v0.383 pre-machine-system demo: tower_configs grey cylinders + helix
        // plant-marker spheres) were REMOVED in v0.529 -- the home.ron machine arrays (the
        // aeroponic_tower_* types) now render the real garden towers, which move + delete with the
        // room. The static markers did not respond, showing duplicate non-responsive towers with
        // spheres (operator feedback 2026-06-24).
        state.placeholder_objects.clear();
        state.machine_objects.clear();

        // ── Machine layout (data-driven, v0.427) ──
        // Rudimentary primitives for the homestead machines + pipes/tubes for the
        // connections between them (data/machines/home.ron). Falls back silently if the
        // file is absent (distributed builds); the tower placeholders above still show.
        {
            let path = state.data_dir.join("machines").join("home.ron");
            if let Some(home) = crate::machines::MachineHome::load(&path) {
                use std::collections::HashMap;
                // room id -> (center, floor_y, ceiling_y).
                let rooms: HashMap<&str, (Vec3, f32, f32)> = room_info
                    .iter()
                    .map(|r| {
                        (
                            r.id.as_str(),
                            (
                                r.center,
                                r.center.y - r.dimensions.y * 0.5,
                                r.center.y + r.dimensions.y * 0.5,
                            ),
                        )
                    })
                    .collect();
                state.gui_state.machine_labels.clear();
                // Despawn any previously-spawned home machine entities so re-entering the
                // world never duplicates the live power entities (load_world can re-run).
                {
                    let old: Vec<hecs::Entity> = state
                        .game_world
                        .world
                        .query::<&crate::ecs::components::HomeMachine>()
                        .iter()
                        .map(|(e, _)| e)
                        .collect();
                    for e in old {
                        let _ = state.game_world.world.despawn(e);
                    }
                }
                // Room volumes for label occlusion (which room is the camera in), now also
                // carrying each room's FUNCTION joined by id from data/rooms.ron (v0.439):
                // the walkable world finally knows what each room is for.
                let room_types =
                    crate::ship::room_types::RoomTypeRegistry::load(&state.data_dir);
                state.gui_state.room_bounds = room_info
                    .iter()
                    .map(|r| crate::gui::RoomBounds {
                        id: r.id.clone(),
                        min: r.center - r.dimensions * 0.5,
                        max: r.center + r.dimensions * 0.5,
                        display_name: room_types.name(&r.id),
                        purpose: room_types.purpose(&r.id),
                        actions: room_types.action_labels(&r.id),
                        access: room_types.access(&r.id),
                    })
                    .collect();
                let mut placed = 0usize;
                // v0.538: a HomeStructure home positions machines by ABSOLUTE world coords (box mode,
                // clamped into the footprint) and skips NO machine on a stale room id -- mirrors
                // MachineHome::placements' box-mode branch; the two MUST stay in sync. Removing the
                // skip in box mode also restores each machine's live ECS power role below. The legacy
                // ship layout keeps room-center-relative + skip-if-missing.
                let (box_mode, box_w, box_d) = match &state.gui_state.home_structure {
                    Some(hs) => (true, hs.width, hs.depth),
                    None => (false, 0.0, 0.0),
                };
                // Explicit instances + every `arrays` grid expanded (dense garden towers).
                let all_instances = home.all_instances();
                for inst in &all_instances {
                    let Some(def) = home.catalog.get(&inst.machine) else { continue };
                    // Position formula mirrored by the tested MachineHome::placements (the editor's
                    // live-refresh twin); keep the two in sync. (v0.525/v0.538)
                    let pos = if box_mode {
                        Vec3::new(
                            inst.offset.0.clamp(0.3, (box_w - 0.3).max(0.3)),
                            inst.offset.1,
                            inst.offset.2.clamp(0.3, (box_d - 0.3).max(0.3)),
                        )
                    } else {
                        let Some(&(center, floor_y, _ceiling_y)) = rooms.get(inst.room.as_str()) else { continue };
                        Vec3::new(
                            center.x + inst.offset.0,
                            floor_y + inst.offset.1,
                            center.z + inst.offset.2,
                        )
                    };
                    let (sx, sy, _sz) = def.size;
                    let mesh = machine_mesh(&state.renderer.device, &def.shape, def.size);
                    let mesh_idx = state.renderer.add_mesh(mesh);
                    let mat = state.renderer.add_material_typed(
                        [def.color.0, def.color.1, def.color.2, 1.0],
                        0.1,
                        0.7,
                        0.0,
                    );
                    // sphere is center-origin; lift it so it rests on the floor.
                    let draw_pos = if def.shape == "sphere" {
                        Vec3::new(pos.x, pos.y + sx, pos.z)
                    } else {
                        pos
                    };
                    state.machine_objects.push((mesh_idx, mat, draw_pos));
                    // Floating label anchor: just above the machine's top.
                    let top_y = if def.shape == "sphere" { pos.y + 2.0 * sx } else { pos.y + sy };
                    let name = if def.label.is_empty() { inst.machine.clone() } else { def.label.clone() };
                    state.gui_state.machine_labels.push(crate::gui::MachineLabel {
                        pos: Vec3::new(pos.x, top_y + 0.4, pos.z),
                        name,
                        stats: def.stats.clone(),
                        room: inst.room.clone(),
                    });
                    // Spawn the machine's electrical role as a LIVE ECS entity so the
                    // SolarSystem + ElectricalSystem tick against the real home (v0.437).
                    if let Some(power) = &def.power {
                        use crate::ecs::components::{HomeMachine, PowerConsumer, PowerGenerator, SolarPanel};
                        use crate::machines::MachinePower;
                        match power {
                            MachinePower::Solar { peak_watts } => {
                                state.game_world.world.spawn((
                                    HomeMachine,
                                    PowerGenerator { output_watts: *peak_watts, fuel_per_second: 0.0, active: true },
                                    SolarPanel { peak_watts: *peak_watts },
                                ));
                            }
                            MachinePower::Generator { watts } => {
                                state.game_world.world.spawn((
                                    HomeMachine,
                                    PowerGenerator { output_watts: *watts, fuel_per_second: 0.0, active: true },
                                ));
                            }
                            MachinePower::Consumer { watts, priority } => {
                                state.game_world.world.spawn((
                                    HomeMachine,
                                    PowerConsumer { draw_watts: *watts, priority: *priority, enabled: true },
                                ));
                            }
                            MachinePower::Battery { capacity_wh, max_charge_w, max_discharge_w } => {
                                state.game_world.world.spawn((
                                    HomeMachine,
                                    crate::ecs::components::Battery {
                                        // Start half-charged so the swing is visible immediately.
                                        charge_wh: capacity_wh * 0.5,
                                        capacity_wh: *capacity_wh,
                                        max_charge_w: *max_charge_w,
                                        max_discharge_w: *max_discharge_w,
                                    },
                                ));
                            }
                        }
                    }
                    placed += 1;
                }
                log::info!("Machines: placed {placed} machines");
            }
        }
        // Build the live connection cylinders (replaces the old static routed pipes). (v0.530)
        rebuild_connection_objects(state);
        // Build the door/window panels from the home structure's openings. (v0.537)
        rebuild_door_panels(state);

        // ── Solar system hologram (map-sync increment C, v0.262.13) ──
        // Driven by the CANONICAL crate::cosmos model at the live date,
        // so the tabletop matches the Maps page + the FPS sky exactly.
        // Was the drifted solar_system.ron placed at fake golden angles
        // (operator: "isn't working — still showing an old
        // placeholder"). Sun is the room centre; bodies sit at their
        // REAL ecliptic longitude (orbit radii still log-compressed to
        // fit the room — true AU ratios can't show indoors).
        let sim_t_now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
            - 946_728_000.0; // Unix secs at the J2000.0 epoch
        let hologram =
            crate::renderer::hologram::generate_hologram_from_cosmos(sim_t_now);

        let orbit_mat = state.renderer.add_material([0.3, 0.7, 0.9, 0.8], 0.0, 0.3);
        let ring_disc_mat = state.renderer.add_material([0.8, 0.7, 0.5, 0.6], 0.0, 0.4);
        let mut orbit_radii_used: Vec<f32> = Vec::new();

        for body in &hologram.bodies {
            if body.radius <= 0.0 { continue; }

            let stacks = if body.radius > 0.05 { 16 } else { 8 };
            let slices = if body.radius > 0.05 { 24 } else { 12 };
            let mesh_idx = state.renderer.add_mesh(
                crate::renderer::hologram::sphere_mesh(&state.renderer.device, body.radius, stacks, slices)
            );
            let (metallic, roughness, emissive) = if body.body_type == crate::renderer::hologram::BodyType::Star {
                (0.0, 0.2, 5.0) // Stars glow bright
            } else {
                (0.3, 0.5, 0.0)
            };
            let mat_idx = state.renderer.add_material_full(body.color, metallic, roughness, 0.0, emissive);
            state.hologram_objects.push((mesh_idx, mat_idx, body.local_position, body.name.clone()));

            if body.orbit_radius > 0.01
                && body.parent.as_deref() == Some("Sun")
                && !orbit_radii_used.iter().any(|&r| (r - body.orbit_radius).abs() < 0.01)
            {
                let ring_mesh_idx = state.renderer.add_mesh(
                    crate::renderer::hologram::orbit_ring_mesh(&state.renderer.device, body.orbit_radius, 128)
                );
                state.hologram_orbits.push((ring_mesh_idx, orbit_mat));
                orbit_radii_used.push(body.orbit_radius);
            }

            if body.has_rings && body.body_type == crate::renderer::hologram::BodyType::Planet {
                let inner_r = body.radius * 1.3;
                let outer_r = body.radius * 2.2;
                let disc_mesh = state.renderer.add_mesh(
                    crate::renderer::hologram::ring_disc_mesh(&state.renderer.device, inner_r, outer_r, 32)
                );
                state.hologram_objects.push((disc_mesh, ring_disc_mat, body.local_position, format!("{} Rings", body.name)));
            }

            if body.body_type == crate::renderer::hologram::BodyType::Planet
                || body.body_type == crate::renderer::hologram::BodyType::DwarfPlanet
            {
                let pin_mesh_idx = state.renderer.add_mesh(
                    crate::renderer::hologram::pin_marker_mesh(&state.renderer.device, 0.03, 0.12)
                );
                let pin_mat = state.renderer.add_material(body.color, 0.0, 0.5);
                let pin_offset = Vec3::new(0.0, body.radius + 0.13, 0.0);
                state.hologram_pins.push((pin_mesh_idx, pin_mat, body.local_position + pin_offset, body.name.clone()));
            }
        }

        // ── GROUND-TRUTH INSTRUMENTATION (v0.262.24) ──
        // Operator: "the in-home map never updated, whatever you do
        // doesn't affect it" across many builds, while the skybox DID
        // update. Logic says this block runs and is correct; reality
        // disagrees. So stop reasoning — make the next run conclusive.
        //
        // 1) Log exactly what generate_hologram_from_cosmos produced.
        // 2) Spawn an UNCONDITIONAL bright-MAGENTA proof beacon at the
        //    orrery centre. If it is ABSENT in the operator's run, THIS
        //    code is not executing in their binary (a build/launch path
        //    issue) — a totally different root cause. If it is PRESENT
        //    but bodies look old, the generator output is the problem.
        // 3) Then the green HOME beacon, with a tolerant Earth lookup
        //    and a RED fallback at centre if Earth is somehow missing,
        //    so a silent failure becomes a visible one.
        {
            let names: Vec<&str> =
                hologram.bodies.iter().map(|b| b.name.as_str()).take(40).collect();
            let earth_idx = hologram
                .bodies
                .iter()
                .position(|b| b.name.eq_ignore_ascii_case("earth"));
            log::info!(
                "ORRERY-DIAG: generate_hologram_from_cosmos -> {} bodies; earth_at={:?}; names={:?}",
                hologram.bodies.len(),
                earth_idx,
                names
            );

            // Magenta proof beacon removed in v0.262.26 — it confirmed
            // the orrery path executes + updates (operator saw it), so
            // the "in-home map never changes" was a misperception (the
            // rings are circles by nature; the cosmos model DOES drive
            // it). Keeping only the clean green HOME marker + the diag
            // log.

            // Green HOME beacon at Earth (tolerant lookup); RED
            // fallback at centre if Earth is missing from the model.
            let earth = hologram
                .bodies
                .iter()
                .find(|b| b.name.eq_ignore_ascii_case("earth"));
            let (anchor, blip_col, pin_col, label) = match earth {
                Some(e) => (
                    e.local_position + Vec3::new(0.0, e.radius, 0.0),
                    [0.15, 1.0, 0.45, 1.0],
                    [0.15, 1.0, 0.45, 1.0],
                    "HOME",
                ),
                None => {
                    log::error!("ORRERY-DIAG: NO 'earth' body in hologram — RED fallback");
                    (Vec3::new(0.0, 0.10, 0.0), [1.0, 0.1, 0.1, 1.0], [1.0, 0.1, 0.1, 1.0], "HOME?")
                }
            };
            let blip_mesh = state.renderer.add_mesh(
                crate::renderer::hologram::sphere_mesh(&state.renderer.device, 0.045, 10, 14),
            );
            let blip_mat =
                state.renderer.add_material_full(blip_col, 0.0, 0.3, 0.0, 8.0);
            state.hologram_objects.push((
                blip_mesh,
                blip_mat,
                anchor + Vec3::new(0.0, 0.10, 0.0),
                "Home (high Earth orbit)".to_string(),
            ));
            let home_pin_mesh = state.renderer.add_mesh(
                crate::renderer::hologram::pin_marker_mesh(&state.renderer.device, 0.07, 0.75),
            );
            let home_pin_mat =
                state.renderer.add_material_full(pin_col, 0.0, 0.4, 0.0, 6.0);
            state.hologram_pins.push((
                home_pin_mesh,
                home_pin_mat,
                anchor + Vec3::new(0.0, 0.95, 0.0),
                label.to_string(),
            ));
            log::info!("ORRERY-DIAG: HOME marker + magenta proof beacon pushed");
        }

        // ── Star skybox ──
        let star_csv = state.data_dir.join("stars.csv");
        state.star_renderer = crate::renderer::stars::StarRenderer::new(
            &state.renderer.device,
            &state.renderer.queue,
            state.renderer.surface_format(),
            &star_csv,
        );

        // ── Planet ──
        state.planet_material = state.renderer.add_material([0.3, 0.5, 0.2, 1.0], 0.0, 0.7);
        match state.asset_manager.load_ron::<PlanetDef>("planets/earth.ron") {
            Ok(def) => {
                let mut pr = PlanetRenderer::new(def.clone(), glam::DVec3::ZERO);
                pr.update_lod(state.camera.world_position);
                let ico = pr.icosphere();
                let mesh_idx = state.renderer.add_mesh(Mesh::from_icosphere(&state.renderer.device, ico, 1.0));
                state.planet = Some(pr);
                state.planet_mesh = Some(mesh_idx);
            }
            Err(e) => log::warn!("Could not load planet: {e}"),
        }

        // ── Ship position (GEO above Silverdale, WA) ──
        let lat_rad = 47.6_f64.to_radians();
        let lon_rad = (-122.3_f64).to_radians();
        let geo_radius = 42_164_000.0_f64;
        state.ship_world_pos = glam::DVec3::new(
            geo_radius * lat_rad.cos() * lon_rad.cos(),
            geo_radius * lat_rad.sin(),
            geo_radius * lat_rad.cos() * lon_rad.sin(),
        );

        // ── Sun setup ──
        // Sun world position: 1 AU from Earth, placed along the existing
        // shader sun_direction vector so the visible Sun disc matches where
        // the world is being lit from. sun_direction uniform is
        // [0.3, 1.0, 0.5] (see renderer/mod.rs:205) — the Sun sits along
        // that ray at 1 AU (149.6 million km).
        let sun_dir = glam::DVec3::new(0.3, 1.0, 0.5).normalize();
        const ONE_AU_M: f64 = 149_597_870_700.0;
        state.sun_world_pos = sun_dir * ONE_AU_M;
        // Emissive yellow-white core. params.w (emissive) cranked high so
        // tone mapping still leaves the Sun near-white on screen.
        state.sun_material = state.renderer.add_material_full(
            [1.0, 0.98, 0.85, 1.0],
            0.0,
            1.0,
            0.0,
            10.0,
        );
        // Halo material — warmer orange, lower emissive. Rendered at a
        // larger scale in the scene to suggest a corona around the core.
        // A true bloom post-process would do this properly, but the
        // halo mesh is a cheap approximation that works without one.
        state.sun_halo_material = state.renderer.add_material_full(
            [1.0, 0.75, 0.4, 1.0],
            0.0,
            1.0,
            0.0,
            1.5,
        );

        // ── Real solar-system body materials (map sync, increment B) ──
        // Four simple PBR materials picked by SolBody.body_type so Mars
        // doesn't look like Earth. Not photoreal — that's a later pass;
        // the point of B is that the FPS sky IS the Maps page (real
        // bodies, real positions, real scale) instead of one lone
        // sphere. Colors are coarse real-imagery approximations.
        state.solar_body_materials = [
            state.renderer.add_material([0.62, 0.52, 0.42, 1.0], 0.0, 0.85), // rocky/terrestrial — tan-grey
            state.renderer.add_material([0.80, 0.66, 0.46, 1.0], 0.0, 0.55), // gas giant — banded ochre
            state.renderer.add_material([0.72, 0.82, 0.92, 1.0], 0.0, 0.40), // icy / dwarf — pale blue-white
            state.renderer.add_material([0.55, 0.55, 0.58, 1.0], 0.0, 0.80), // default — grey
        ];

        // ── Orbit paths (v0.262.20 — thin world-space lines) ──
        // Was thick tube meshes (operator: "tubes are just too thick …
        // we wouldn't need all the verts … like a single edge"). Now we
        // just cache each body's TRUE Keplerian ellipse points
        // (crate::cosmos::sample_orbit_points → same math the Maps page
        // draws) in PARENT-frame metres. Per frame they're offset to the
        // parent's Earth-relative position and drawn as a 1-px LineList
        // that the depth buffer occludes behind planets. 96 samples is
        // plenty smooth for an ellipse and a fraction of the tube verts.
        for b in crate::cosmos::sol_bodies() {
            // Direct sun-orbiters (planets, dwarfs, named belt) + Moon.
            // Sun has no orbit (sample empty) → skipped naturally.
            let direct_solar = b.parent.as_deref() == Some("sun");
            if !direct_solar && b.id != "moon" { continue; }
            let pts_au = crate::cosmos::sample_orbit_points(b, 96);
            if pts_au.len() < 3 { continue; }
            let pts_m: Vec<[f32; 3]> = pts_au
                .iter()
                .map(|p| {
                    [
                        (p.x * crate::cosmos::M_PER_AU) as f32,
                        (p.y * crate::cosmos::M_PER_AU) as f32,
                        (p.z * crate::cosmos::M_PER_AU) as f32,
                    ]
                })
                .collect();
            state
                .solar_orbit_paths
                .push((pts_m, b.parent.clone().unwrap_or_else(|| "sun".to_string())));
        }
        log::info!("Map-sync: cached {} FPS orbit paths (thin lines)", state.solar_orbit_paths.len());

        // ── Load CSV game-data registries into the runtime DataStore ──
        // Each registry is built from its data file and inserted under the key its
        // owning system reads (item_registry / recipe_registry / plant_registry).
        // Graceful per-registry: a missing or malformed file logs a warning and
        // skips (the system then runs on safe defaults), never panics. Reads from
        // the on-disk data dir so edits/mods to the CSV take effect. Mirrors the
        // container_registry wiring below.
        //
        // BEFORE v0.323 these three were loaded then DISCARDED (`let _ =
        // load_csv(...)` into throwaway {id,name} structs), so the runtime
        // DataStore stayed empty and CraftingSystem (no recipes), item
        // name/stack/mass lookups, and FarmingSystem species data all silently
        // no-op'd — the central finding of the 2026-05-29 game-code audit.
        // The registries are loaded EAGERLY at startup (load_data_registries, called
        // from resumed) — see that fn for why. This call re-loads them when the 3D
        // world opens (idempotent), so editing a data file + re-entering picks it up.
        load_data_registries(&mut state.data_store, state.asset_manager.data_dir());

        // ── Player avatar + character-select showroom (v0.440/441) ──
        // Place a blockman avatar on a podium in the respawner (where you wake) and OPEN the
        // showroom: hide the home, orbit the avatar against a backdrop, let the player edit
        // appearance, then "Enter your home" to emerge into first-person. The avatar is the
        // last thing added to placeholder_objects, so `avatar_obj_start` marks where it
        // begins (the showroom renders + rebuilds only this range).
        if let Some(r) = room_info.iter().find(|r| r.id == "respawner") {
            let floor = r.center.y - r.dimensions.y * 0.5;
            let base = Vec3::new(r.center.x, floor, r.center.z - 0.35);
            let (cname, app, outfit) = state
                .game_world
                .world
                .query::<(
                    &crate::ecs::components::Name,
                    &crate::ecs::components::Appearance,
                    &crate::ecs::components::Outfit,
                    &Controllable,
                )>()
                .iter()
                .next()
                .map(|(_, (n, a, o, _))| (n.0.clone(), a.clone(), o.clone()))
                .unwrap_or_else(|| ("Wanderer".to_string(), Default::default(), Default::default()));
            state.gui_state.character_name = cname;
            state.cosmetics = crate::cosmetics::load_cosmetics(&state.data_dir);
            state.gui_state.cosmetics_list = state
                .cosmetics
                .iter()
                .map(|c| (c.id.clone(), c.name.clone(), c.slot.clone()))
                .collect();
            state.gui_state.appearance = app.clone();
            state.gui_state.outfit = outfit.clone();
            state.avatar_base = base;
            state.fps_spawn = state.camera.position; // the first-person spawn set above
            state.showroom_return_pos = state.camera.position;
            state.avatar_obj_start = state.placeholder_objects.len();
            let colors = crate::cosmetics::resolve_outfit_colors(&outfit, &state.cosmetics);
            place_avatar(state, base, &app, &colors);

            // Showroom SCENE ASSETS (backdrops, ground disc, body sphere) are loaded on
            // every world-load -- cheap, and needed so the wetroom mirror + bedroom
            // wardrobe can open the showroom later even when Play did not open the picker.
            state.showroom_backdrops = crate::showroom::load_backdrops(&state.data_dir);
            state.gui_state.showroom_backdrop_names =
                state.showroom_backdrops.iter().map(|b| b.name.clone()).collect();
            state.gui_state.showroom_backdrop = 0;
            state.showroom_last_backdrop = usize::MAX;
            let gmesh = state.renderer.add_mesh(Mesh::cylinder(&state.renderer.device, 9.0, 0.06, 32));
            let gmat = state.renderer.add_material_typed([0.1, 0.1, 0.12, 1.0], 0.1, 0.9, 0.0);
            state.showroom_ground = Some((gmesh, gmat));
            // A planet sphere (radius 30) the avatar stands on for body backdrops (Earth/Mars).
            let body = state.renderer.add_mesh(Mesh::sphere(&state.renderer.device, 30.0, 24, 32));
            state.showroom_body = Some(body);
            state.gui_state.appearance_dirty = false;
            state.gui_state.showroom_confirm = false;

            // load_world NO LONGER opens the character-select showroom (v0.476).
            // It just spawns you in first-person at the respawner. The unified
            // character picker is opened OPT-IN by the Play button, via the
            // per-frame open_showroom(0) call that runs right AFTER this load when
            // launcher_open_select is set. Because load_world only runs once (the
            // world_loaded guard) AND Esc enters the world without that flag, the
            // old picker (the "Wanderer" duplicate the operator hit on Esc) never
            // appears on Esc, and Play opens the picker every time, not just the
            // first. This is THE root-cause fix for the duplicate character-select.
            state.gui_state.showroom_active = false;
            state.controller.showroom_lock = false;
            state.camera.switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
            state.camera.position = state.fps_spawn;
        }

        state.world_loaded = true;
        log::info!("3D world loaded in {:.0}ms", load_start.elapsed().as_millis());
    }

    /// Load the small, runtime-critical data registries (items, recipes, plants,
    /// status effects, skills, quests, containers) into the DataStore.
    ///
    /// These are cheap CSV/RON parses the GAME SYSTEMS read every tick, so they MUST
    /// load EAGERLY at startup — not lazily in `load_world` (which only runs when you
    /// switch to the 3D world view). The menu-driven loops (inventory / crafting /
    /// skills / quests) otherwise run against empty registries: raw item ids, no
    /// recipes to craft, no skill names, the quest shown by its raw id. (The heavy 3D
    /// mesh generation stays lazy in `load_world`.) Idempotent — safe to call twice.
    fn load_data_registries(store: &mut DataStore, data_dir: &std::path::Path) {
        fn load_csv_registry<T: Send + Sync + 'static>(
            store: &mut DataStore,
            path: std::path::PathBuf,
            key: &str,
            build: impl Fn(&[u8]) -> Result<T, String>,
        ) {
            match std::fs::read(&path) {
                Ok(bytes) => match build(&bytes) {
                    Ok(reg) => {
                        store.insert(key, reg);
                        log::info!("Loaded {key} from {}", path.display());
                    }
                    Err(e) => log::warn!("Failed to build {key} from {}: {e}", path.display()),
                },
                Err(e) => log::warn!(
                    "Data file {} not found ({e}); {key} unavailable (system on defaults)",
                    path.display()
                ),
            }
        }
        load_csv_registry(
            store,
            data_dir.join("items.csv"),
            "item_registry",
            crate::systems::inventory::ItemRegistry::from_csv,
        );
        load_csv_registry(
            store,
            data_dir.join("recipes.csv"),
            "recipe_registry",
            crate::systems::crafting::RecipeRegistry::from_csv,
        );
        load_csv_registry(
            store,
            data_dir.join("plants.csv"),
            "plant_registry",
            crate::systems::farming::PlantRegistry::from_csv,
        );
        load_csv_registry(
            store,
            data_dir.join("status_effects.csv"),
            "status_effect_registry",
            crate::systems::status_effects::StatusEffectRegistry::from_csv,
        );
        load_csv_registry(
            store,
            data_dir.join("skills").join("skills.csv"),
            "skill_registry",
            SkillRegistry::from_csv,
        );
        store.insert(
            "quest_registry",
            QuestRegistry::from_ron_dir(&data_dir.join("quests")),
        );
        // Container types + content-class compatibility (graceful on missing files).
        {
            use crate::systems::inventory::containers::ContainerRegistry;
            let types_path = data_dir.join("containers").join("types.csv");
            let classes_path = data_dir.join("containers").join("content_classes.ron");
            match (std::fs::read(&types_path), std::fs::read(&classes_path)) {
                (Ok(types_bytes), Ok(classes_bytes)) => {
                    match ContainerRegistry::from_bytes(&types_bytes, &classes_bytes) {
                        Ok(reg) => {
                            log::info!(
                                "Loaded ContainerRegistry: {} container types, {} content classes",
                                reg.types.len(),
                                reg.content_classes.len()
                            );
                            store.insert("container_registry", reg);
                        }
                        Err(e) => log::warn!("ContainerRegistry parse failed: {e}"),
                    }
                }
                _ => log::warn!(
                    "Container data not found ({} / {}); container compatibility disabled",
                    types_path.display(),
                    classes_path.display()
                ),
            }
        }
    }

    struct App {
        state: Option<EngineState>,
    }

    impl App {
        fn new() -> Self {
            Self { state: None }
        }
    }

    /// Construction 3D drag state (v0.466): which editor room is grabbed + the world floor
    /// plane and the offset from the room's min-corner to the grab hit point, so the room
    /// tracks the cursor without jumping.
    #[derive(Clone, Copy)]
    struct ConstructionGrab {
        room_index: usize,
        floor_y: f32,
        offset_x: f32,
        offset_z: f32,
    }

    /// Which part of an opening gizmo is grabbed. (v0.469)
    #[derive(Clone, Copy, PartialEq)]
    enum GizmoRole {
        Move,
        ResizeLeft,
        ResizeRight,
        ResizeBottom,
        ResizeTop,
    }

    /// Which build-mode gizmo the cursor is hovering (v0.569), so a gizmo reads idle -> hover ->
    /// active (grabbed) by colour, like the menu header buttons. Computed each frame by
    /// `compute_construction_hover` and consumed by the gizmo render.
    #[derive(Clone, Copy, PartialEq)]
    enum HoverGizmo {
        None,
        Corner(f32, f32),
        Opening(usize, usize),
        Char,
    }

    /// Construction opening-gizmo drag (v0.468, rebuilt v0.469): which room+opening is grabbed,
    /// the captured wall-face plane (so the cursor projects onto the VERTICAL wall, giving u along
    /// + v up), and the grab role. `room_index` indexes `gui_state.construction_rooms` (the editor
    /// mirror). `opening_index` Some(i) drives `rooms[ri].openings[i]` (move + resize); None is a
    /// legacy `WallSet.offsets` slide (back-compat, Move only).
    #[derive(Clone, Copy)]
    struct ConstructionGizmoGrab {
        room_index: usize,
        opening_index: Option<usize>,
        wall_index: usize,
        role: GizmoRole,
        snap_floor: bool,
        wall_start: Vec3,
        /// Unit vector along the wall (start -> end).
        u_hat: Vec3,
        /// Wall-face plane normal (horizontal) the pick ray intersects.
        n: Vec3,
        wall_len: f32,
        wall_height: f32,
        /// Legacy slide base (offset = u - base_t); only used when `opening_index` is None.
        base_t: f32,
        /// Opening extents captured at grab time (for resize anchoring).
        grab_u: f32,
        grab_v: f32,
        grab_w: f32,
        grab_h: f32,
    }

    struct EngineState {
        window: Arc<Window>,
        renderer: Renderer,
        camera: Camera,
        controller: CameraController,
        asset_manager: AssetManager,
        hot_reload: HotReloadCoordinator,
        game_world: GameWorld,
        system_runner: SystemRunner,
        data_store: DataStore,
        star_renderer: Option<crate::renderer::stars::StarRenderer>,
        floating_origin: crate::renderer::floating_origin::FloatingOrigin,
        planet: Option<PlanetRenderer>,
        planet_mesh: Option<usize>,
        planet_material: usize,
        /// World-space position of the Sun (Earth-centred coordinates).
        sun_world_pos: glam::DVec3,
        /// Emissive material index for the Sun core.
        sun_material: usize,
        /// Emissive material index for the Sun halo (larger sphere, warmer,
        /// lower emissive — gives the Sun a faked corona without bloom).
        sun_halo_material: usize,
        /// Materials for the real solar-system bodies rendered around the
        /// home (v0.262.9, map sync increment B): [0]=rocky, [1]=gas
        /// giant, [2]=icy/dwarf, [3]=default grey. Picked by SolBody
        /// `body_type`. The Sun reuses `sun_material`.
        solar_body_materials: [usize; 4],
        /// Orbit paths for the FPS world (v0.262.20 — thin world-space
        /// lines, replacing the old too-thick tube meshes). Each entry
        /// is (PARENT-frame ellipse points in metres, parent_id);
        /// per frame they're offset to the parent's Earth-relative
        /// position and drawn as a single-edge LineList that is
        /// depth-occluded behind planets.
        solar_orbit_paths: Vec<(Vec<[f32; 3]>, String)>,
        /// Homestead floor meshes (mesh_idx, material_idx) per room.
        homestead_floors: Vec<(usize, usize)>,
        /// Placeholder world objects (mesh_idx, material_idx, world position) drawn
        /// alongside the homestead. Used for simple-shape stand-ins like the
        /// aeroponic tower cylinders + plant-marker spheres (v0.383).
        placeholder_objects: Vec<(usize, usize, Vec3)>,
        /// Home machine meshes, kept SEPARATE from `placeholder_objects` so the construction editor
        /// can rebuild JUST the machines on an edit (a move/add/remove) without touching towers,
        /// pipes, or the avatar. Built by load_world on entry + rebuild_machine_objects on edit;
        /// positions come from the tested `MachineHome::placements`. Drawn when not in the showroom.
        /// (v0.525, the live-edit preview that makes the build mode feel real.)
        machine_objects: Vec<(usize, usize, Vec3)>,
        /// Pick volumes for viewport machine SELECTION (v0.553): (id, world center, bounding radius)
        /// per placed machine. Rebuilt alongside machine_objects; the build-mode click ray-tests this
        /// to select a machine (its detail then shows on the right panel).
        machine_pick: Vec<(String, Vec3, f32)>,
        /// Static wall/perimeter collision segments for the home (v0.556): the player (= the camera)
        /// is pushed out of these in first person so you can no longer walk through walls. Rebuilt
        /// from the home_structure on every structural edit + on world load. Doors collide live.
        wall_colliders: Vec<crate::ship::wall_collision::WallSegment>,
        /// Home machine CONNECTIONS as live colored cylinders (v0.530): (mesh, material, position,
        /// rotation, scale). Replaces the static routed pipes so connections appear immediately +
        /// follow rooms in the editor. Rebuilt with the machines; uses one cached unit cylinder mesh
        /// (`connection_cyl`) transformed per link + a material cached per kind (`connection_mats`),
        /// so a per-frame drag does not leak meshes.
        connection_objects: Vec<(usize, usize, Vec3, Quat, Vec3)>,
        connection_cyl: Option<usize>,
        connection_mats: std::collections::HashMap<String, usize>,
        /// Door + window panels (v0.537): each opening's world placement + its current open fraction
        /// (0 closed, 1 open). Doors animate open on the player's approach by their data-driven style
        /// (systems::door_anim); windows are fixed glass. One cached unit-box mesh + a slab + a glass
        /// material, reused (scaled/rotated/animated per frame), so it never leaks.
        door_panels: Vec<(crate::ship::door_panels::PanelPlacement, f32)>,
        /// Runtime "opened via its control panel" flag per door (v0.567), parallel to door_panels. A
        /// MANUAL door with this set opens; the player toggles it at the panel. Reset on rebuild.
        door_manual_open: Vec<bool>,
        /// Live LOCK STATE per door (v0.570): door_locks[i][j] is the runtime state of door i's lock j,
        /// parallel to door_panels[i].0.locks. The player unlocks/breaks locks at runtime; reset to the
        /// authored states on a structural rebuild (mirrors door_manual_open). A door is passable only
        /// when all of its locks are open.
        door_locks: Vec<Vec<crate::ship::lock_types::LockState>>,
        door_panel_mesh: Option<usize>,
        door_slab_mat: Option<usize>,
        door_glass_mat: Option<usize>,
        /// Energy/nanowall door materials (v0.554): glowing green (open) / red (locked) energy field
        /// + a metallic semi-transparent nanowall. All render in the transparent pass.
        door_energy_open_mat: Option<usize>,
        door_energy_locked_mat: Option<usize>,
        door_nanowall_mat: Option<usize>,
        /// Accumulated time (s) driving the nanowall's shifting "water" shimmer. (v0.554)
        door_anim_time: f32,
        /// Index in `placeholder_objects` where the player avatar's parts begin (the avatar
        /// is added last in load_world). Lets the showroom render only the avatar + rebuild
        /// it on appearance change by truncating to this index. (v0.441)
        avatar_obj_start: usize,
        /// Podium floor position the avatar stands on (respawner center). (v0.441)
        avatar_base: Vec3,
        /// First-person spawn position to drop the player at when leaving the showroom.
        fps_spawn: Vec3,
        /// Loaded character-select showroom backdrops. (v0.441)
        showroom_backdrops: Vec<crate::showroom::Backdrop>,
        /// The showroom ground disc (mesh, material), material rebuilt on backdrop change.
        showroom_ground: Option<(usize, usize)>,
        /// The showroom planet-body sphere mesh (v0.449): used instead of the flat disc when
        /// the backdrop is a body (Earth/Mars/Moon), so the avatar stands on a planet.
        showroom_body: Option<usize>,
        /// Last backdrop index the ground material was built for (usize::MAX = none yet).
        showroom_last_backdrop: usize,
        /// Cosmetic outfit catalog (data/cosmetics/cosmetics.csv). (v0.442)
        cosmetics: Vec<crate::cosmetics::Cosmetic>,
        /// First-person position to return to when leaving the showroom (the spawn for the
        /// initial character-select, or where you were standing when you opened the mirror /
        /// wardrobe from the wetroom / bedroom). (v0.442)
        showroom_return_pos: Vec3,
        /// Tracks whether the OS cursor is currently freed (visible + ungrabbed), so the
        /// per-frame reconciliation only toggles grab on a real change. (v0.443)
        cursor_free: bool,
        /// Homestead walls mesh + material (legacy fibonacci ship path).
        homestead_walls: Option<(usize, usize)>,
        /// Per-material home walls (v0.552): (mesh, material, is_transparent) for each picked wall
        /// material, so each wall renders in its own color (the home-structure path).
        homestead_material_walls: Vec<(usize, usize, bool)>,
        /// Homestead trim mesh (baseboards, crown, door/window frames) + material. (v0.453)
        homestead_trim: Option<(usize, usize)>,
        /// Homestead window-glass mesh + material. (v0.453)
        homestead_windows: Option<(usize, usize)>,
        /// Homestead mirror / portal panel mesh + material. (v0.453)
        homestead_mirrors: Option<(usize, usize)>,
        /// Homestead ceiling mesh + material — drawn only when `gui_state.show_roof`. (v0.453)
        homestead_ceiling: Option<(usize, usize)>,
        /// True when the ceiling is a CLEAR/GLASS roof (v0.539): the renderer draws it in the
        /// transparent pass (you see the stars through it) instead of as an opaque ceiling. Set by
        /// apply_homestead_meshes from the HomeStructure's roof_material.
        homestead_ceiling_glass: bool,
        /// The live homestead layout (v0.455). Held so the construction editor can mutate it
        /// (per-wall kinds, heights) and regenerate the meshes without a restart.
        homestead_layout: Option<crate::ship::fibonacci::HomesteadLayout>,
        /// Astral-projection construction camera (v0.464): true while the orbit cam is engaged
        /// for the editor, so we switch in/out exactly once (works for B AND the panel Close).
        construction_cam_active: bool,
        /// First-person position to return to when leaving the construction editor. (v0.464)
        construction_return_pos: Vec3,
        /// Last cursor position in physical pixels (top-left origin), for 3D picking. (v0.466)
        cursor_pos: (f32, f32),
        /// The room currently grabbed (left-drag) in the 3D astral editor. (v0.466)
        construction_grab: Option<ConstructionGrab>,
        /// Cached placement-ghost mesh for the held palette item (v0.529): (machine type, mesh idx,
        /// material idx). Rebuilt only when the held type changes, so the cursor-following ghost
        /// does not leak a fresh mesh every frame.
        construction_ghost: Option<(String, usize, usize)>,
        /// Held-STRUCTURE placement ghost (v0.583): (type_id ‖ yaw key, mesh idx, material idx).
        /// Rebuilt when the held type OR the placement yaw changes (the key encodes both), so the
        /// cursor-following structure preview is correct without leaking a mesh per frame.
        construction_structure_ghost: Option<(String, usize, usize)>,
        /// Teleporter re-fire cooldown in seconds (v0.584): set when the player jumps through a
        /// teleport pad; counts down each frame so standing on the destination pad doesn't ping-pong.
        teleport_cooldown: f32,
        /// Cached unit-box mesh + translucent material for the wall-drawing tool (v0.534): the corner
        /// node marker under the cursor and the preview wall from the pending start to the cursor.
        /// Lazy-created once and reused (scaled/rotated per frame) so the preview never leaks a mesh.
        wall_tool_mesh: Option<usize>,
        wall_tool_mat: Option<usize>,
        /// Corner-node editing (v0.541): the position of the wall corner currently grabbed by its
        /// gizmo (None = not dragging). Dragging moves EVERY wall endpoint at this position (shared
        /// corners move together), snapped to the grid / box edges / other corners. Plus the cached
        /// gizmo sphere mesh + its normal + highlighted materials.
        construction_node_grab: Option<(f32, f32)>,
        construction_node_mesh: Option<usize>,
        construction_node_mat: Option<usize>,
        construction_node_mat_hot: Option<usize>,
        /// Placed-LIGHT gizmo (v0.572): a cached DIAMOND (octahedron) centre-marker mesh + an emissive
        /// material, drawn at each placed light in build mode (the range "sphere" is RGB line circles).
        construction_light_mesh: Option<usize>,
        construction_light_mat: Option<usize>,
        /// Wall-SELECT gizmo material (v0.573): a RED sphere at each wall's bottom-middle so you can
        /// click the wall (surface or orb) to select it; the SELECTED wall's orb uses the RGB hot mat.
        construction_wall_mat: Option<usize>,
        /// Build-mode gizmo HOVER material (v0.569): a brightened idle colour shown on the gizmo the
        /// cursor is over (idle -> hover -> active, like the header buttons).
        construction_node_mat_hover: Option<usize>,
        /// Build-mode player avatar (v0.557): whether its pyramid gizmo is grabbed, plus its cached
        /// body box / pyramid-gizmo meshes + material.
        construction_char_grab: bool,
        construction_char_mesh: Option<usize>,
        construction_char_pyramid_mesh: Option<usize>,
        construction_char_mat: Option<usize>,
        /// Door/window OPENING editing (v0.546): the (wall index, opening index) of the opening whose
        /// gizmo is grabbed -- dragging slides it ALONG its wall (updates `at`). A visually distinct
        /// cube gizmo (vs the corner spheres) + its cached mesh/material.
        construction_opening_grab: Option<(usize, usize)>,
        /// Opening RESIZE-handle grab (v0.578): (wall, opening, edge) where edge 0=left 1=right 2=top
        /// 3=bottom. Dragging a left/right handle changes width+at; top/bottom changes height+sill.
        construction_opening_resize: Option<(usize, usize, u8)>,
        construction_opening_mesh: Option<usize>,
        construction_opening_mat: Option<usize>,
        /// Cursor pixel position when a corner/opening gizmo was first pressed (v0.549). While this is
        /// Some, the press has NOT moved past the drag threshold yet -- so a release here is a CLICK
        /// (select + show on the right panel), not a move. It clears to None once the cursor moves past
        /// the threshold, which is what arms the actual drag (so click-and-HOLD moves, a tap selects).
        construction_grab_press: Option<(f32, f32)>,
        /// The door/window slide-gizmo handle currently grabbed in the 3D editor. (v0.468)
        construction_gizmo_grab: Option<ConstructionGizmoGrab>,
        /// Cached (mesh, material) for the gizmo MOVE handle marker, built once. (v0.468)
        construction_gizmo_handle: Option<(usize, usize)>,
        /// Cached (mesh, material) for the gizmo RESIZE handle markers (warning-tinted). (v0.469)
        construction_gizmo_resize_handle: Option<(usize, usize)>,
        /// Cached (mesh, material) for the selected-room highlight quad, built once. (v0.466)
        construction_hilite: Option<(usize, usize)>,
        // ── Multiplayer co-presence (v0.472) ──
        /// Processes inbound game messages + interpolates remote players. Reuses the authenticated
        /// chat WebSocket (`gui_state.ws_client`) -- no second socket, no second auth.
        net_sync: crate::net::sync::NetSyncSystem,
        /// True once we have sent `game_join` for this world session (cleared on leave/disconnect).
        game_joined: bool,
        /// Throttle for outbound position updates (send ~15/sec).
        game_pos_timer: f32,
        /// Cached (body_mesh, head_mesh, material) for the remote-player avatar marker, built once.
        remote_avatar: Option<(usize, usize, usize)>,
        /// Solar system hologram bodies (mesh_idx, material_idx, local_position, name).
        hologram_objects: Vec<(usize, usize, Vec3, String)>,
        /// Hologram orbit rings (mesh_idx, material_idx).
        hologram_orbits: Vec<(usize, usize)>,
        /// Hologram pin markers (mesh_idx, material_idx, local_position, name).
        hologram_pins: Vec<(usize, usize, Vec3, String)>,
        /// Currently targeted hologram planet (name, if crosshair is on a pin).
        targeted_planet: Option<String>,
        /// Hologram room center (from data-driven layout).
        hologram_room_center: Vec3,
        /// Room ceiling lights: (position, color, intensity, radius).
        room_lights: Vec<(Vec3, [f32; 3], f32, f32)>,
        /// Sealed homestead volume AABB (min, max), encompassing all rooms — the
        /// survival environment context: inside = oxygenated/heated, outside =
        /// vacuum/cold. None until the homestead generates.
        homestead_bounds: Option<(Vec3, Vec3)>,
        /// Ship world position (GEO orbit coordinates).
        ship_world_pos: glam::DVec3,
        start_time: Instant,
        last_frame: Instant,
        // egui integration
        egui_ctx: egui::Context,
        egui_state: egui_winit::State,
        egui_renderer: egui_wgpu::Renderer,
        gui_state: GuiState,
        theme: Theme,
        /// Whether the 3D world has been fully initialized.
        world_loaded: bool,
        /// Reserved for future use.
        window_shown: bool,
        /// Data directory path (resolved once at startup, used for deferred loading).
        data_dir: PathBuf,
        /// Whether a Ctrl/Cmd modifier key is currently held. Tracked from
        /// raw winit KeyboardInput because egui-winit swallows Ctrl+V at
        /// the winit layer (translates it to Event::Paste(text) and returns
        /// early WITHOUT pushing the V key event) — so egui's input never
        /// sees Ctrl+V for an image clipboard. We detect it here instead
        /// and set gui_state.pending_clipboard_paste. v0.234.
        ctrl_held: bool,
        /// Shift modifier state (v0.575), for Ctrl+Shift+Z redo in the construction editor.
        shift_held: bool,
        /// Left-mouse-button held state (v0.575): true while dragging a gizmo or a slider, so the undo
        /// history coalesces a continuous drag into one step (checkpoint on release).
        lmb_held: bool,
        /// Construction-editor undo/redo history (v0.575): bounded snapshot stacks of the editable
        /// home (structure + machines), captured at the dirty-flag choke point.
        construction_history: ConstructionHistory,
    }

    /// One captured editor state for undo/redo (v0.575): a clone of the editable home (structure +
    /// machines). Selection is intentionally NOT captured -- restoring it would yank the right panel to
    /// a stale wall; the current selection is kept (and self-clamps if it falls out of range).
    #[derive(Clone, Default)]
    struct EditorSnapshot {
        structure: Option<crate::ship::home_structure::HomeStructure>,
        machines: Option<crate::machines::MachineHome>,
    }

    /// Bounded undo/redo history for the construction editor (v0.575). Snapshot model: cheap (the home
    /// is tens of KB) and robust (no per-action inverse). A continuous DRAG -- a gizmo OR a slider --
    /// is coalesced into ONE step by only checkpointing while the left mouse button is NOT held, plus
    /// once on release if an edit happened during the hold.
    #[derive(Default)]
    struct ConstructionHistory {
        undo: std::collections::VecDeque<EditorSnapshot>,
        redo: Vec<EditorSnapshot>,
        /// The committed state as of the last checkpoint -- pushed onto `undo` when the next edit lands.
        baseline: EditorSnapshot,
        /// Whether an edit happened during the current LMB hold (so a click/drag that changed nothing
        /// won't checkpoint).
        edited_during_hold: bool,
        /// Whether the LMB was held last frame (to detect release).
        prev_held: bool,
        /// Whether the editor was open last frame (to reset history on open).
        prev_active: bool,
    }

    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            if self.state.is_some() {
                return;
            }

            // Extract embedded data files on first run (enables modding)
            extract_data_if_needed();

            // Boot MAXIMIZED with decorations = "windowed fullscreen" (title bar + taskbar
            // still visible), the operator's preferred default (v0.454). The loaded
            // window_mode is then applied once the config is read (apply_window_mode).
            let window_attrs = Window::default_attributes()
                .with_title(format!("HumanityOS v{}", env!("CARGO_PKG_VERSION")))
                .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
                .with_maximized(true)
                .with_visible(false);

            let window = Arc::new(
                event_loop
                    .create_window(window_attrs)
                    .expect("Failed to create window"),
            );

            // Set window icon from embedded PNG
            {
                let icon_bytes = include_bytes!("../assets/icon.png");
                if let Ok(img) = image::load_from_memory(icon_bytes) {
                    let rgba = img.to_rgba8();
                    let (w, h) = (rgba.width(), rgba.height());
                    if let Ok(icon) = winit::window::Icon::from_rgba(rgba.into_raw(), w, h) {
                        window.set_window_icon(Some(icon));
                    }
                }
            }

            // Initialize renderer (block on async)
            let mut renderer = pollster::block_on(Renderer::new_native(window.clone()));

            window.set_visible(true);

            // ── DEFERRED: 3D world init is skipped here, done lazily on first Enter World ──
            // Only set up the data directory path for later use.
            let data_dir = find_data_dir();
            // Publish it for the cached loaders (laws/glossary/homes) so they resolve
            // the same CWD-independent path instead of a bare relative "data".
            let _ = crate::DATA_DIR.set(data_dir.clone());

            // Minimal camera/controller (needed by struct, but not used until world loads)
            let mut camera = Camera::new();
            camera.aspect = renderer.aspect_ratio();
            camera.position = Vec3::new(-0.5, 1.7, 4.0);
            camera.pitch = -0.2;
            camera.yaw = std::f32::consts::PI;
            // Sensitivity here is just the frame-0 default; the boot-time settings_dirty
            // sync (below) overrides it with the saved value on the first frame.
            let controller = CameraController::new(5.0, 0.25);

            // Minimal asset/ECS (lightweight, no file I/O)
            let asset_manager = AssetManager::new(data_dir.clone());
            let hot_reload = HotReloadCoordinator::new(&data_dir);
            let mut game_world = GameWorld::new();
            let mut system_runner = SystemRunner::new();
            let mut data_store = DataStore::new();
            data_store.insert("input_state", InputState::default());
            data_store.insert("camera_position", Vec3::new(0.0, 2.0, 5.0));
            data_store.insert("camera_forward", Vec3::NEG_Z);
            data_store.insert("camera_yaw", 0.0_f32);
            // GameTime lives behind a Mutex in the DataStore so TimeSystem (which
            // only gets &DataStore in tick) can write the advanced time each frame
            // and farming/ecology/weather/hydrology + the HUD can read it. Same
            // interior-mutability pattern as interaction_prompt below.
            data_store.insert(
                "game_time",
                std::sync::Mutex::new(crate::systems::time::GameTime::default()),
            );
            // Weather behind a Mutex (the TimeSystem/game_time pattern): WeatherSystem
            // writes it each tick; the survival env (exposed temp) + HUD read it.
            data_store.insert(
                "weather",
                std::sync::Mutex::new(crate::systems::weather::Weather::default()),
            );
            // Live home electrical readout (gen/use/balance watts): ElectricalSystem writes
            // it each tick, the HUD + Home page read it. Same Mutex pattern as game_time.
            data_store.insert(
                "power_status",
                std::sync::Mutex::new(crate::systems::electrical::PowerStatus::default()),
            );
            system_runner.register(TimeSystem::new());
            // WeatherSystem ticks after TimeSystem (reads the exported season) and
            // exports Weather; the exposed-environment temperature consumes it.
            system_runner.register(WeatherSystem::new());
            // SolarSystem scales solar PowerGenerators by the time of day, then
            // ElectricalSystem sums supply/demand and sheds load. These tick the LIVE
            // home power sim against the machine entities spawned in load_world (v0.437).
            system_runner.register(crate::systems::solar::SolarSystem::new());
            system_runner.register(crate::systems::electrical::ElectricalSystem::new(&data_dir));
            system_runner.register(PlayerControllerSystem);
            data_store.insert("interaction_prompt", std::sync::Mutex::new(String::new()));
            // GUI -> ECS command channels (interior-mutable; the main loop writes
            // these from GuiState before each tick, the owning System reads + acts in
            // its tick). craft_request = a recipe id the player clicked Craft on;
            // dev_stock_materials = dev/creative provisioning (stock every recipe input);
            // consume_request = an item id the player clicked Eat on (FoodSystem applies
            // its nutrition to the player's Vitals).
            data_store.insert(
                "craft_request",
                std::sync::Mutex::new(Option::<String>::None),
            );
            data_store.insert("dev_stock_materials", std::sync::Mutex::new(false));
            data_store.insert(
                "consume_request",
                std::sync::Mutex::new(Option::<String>::None),
            );
            data_store.insert(
                "drink_request",
                std::sync::Mutex::new(Option::<String>::None),
            );
            // Gardening command channels (inventory page -> FarmingSystem): plant a
            // seed by item id, water/harvest a crop by entity bits, dev-grow all.
            data_store.insert(
                "plant_request",
                std::sync::Mutex::new(Option::<String>::None),
            );
            // Plant a whole aeroponic tower at once (v0.386): (tower config id, plant ids).
            data_store.insert(
                "plant_tower_request",
                std::sync::Mutex::new(Option::<(String, Vec<String>)>::None),
            );
            data_store.insert("water_request", std::sync::Mutex::new(Option::<u64>::None));
            data_store.insert("harvest_request", std::sync::Mutex::new(Option::<u64>::None));
            data_store.insert("dev_grow_crops", std::sync::Mutex::new(false));
            // Per-area irrigation targets the garden edit modal publishes (tower_id ->
            // water level 0..1). FarmingSystem tops matching crops up to it; mirrored
            // from GuiState.garden_irrigation each frame by the bridge below.
            data_store.insert(
                "garden_irrigation",
                std::sync::Mutex::new(std::collections::HashMap::<String, f32>::new()),
            );
            // Per-area nutrient strength (garden edit slider, tower_id -> 0..1) scaling
            // crop growth speed in FarmingSystem; mirrored from GuiState each frame.
            data_store.insert(
                "garden_nutrient",
                std::sync::Mutex::new(std::collections::HashMap::<String, f32>::new()),
            );
            // Backpack <-> container transfers (organize-layer inventory): the GUI pushes
            // (item_id, qty, is_add) ops; InventorySystem applies them to the player's
            // backpack. Mirrored from GuiState.pending_inventory_transfers each frame.
            data_store.insert(
                "inventory_transfer_ops",
                std::sync::Mutex::new(Vec::<(String, u32, bool)>::new()),
            );
            // Creative mode (default ON during early dev): the resource-consuming
            // systems (farming seeds/fertilizer, crafting materials) skip the
            // inventory requirement + consumption when this is true. Mirrored from
            // GuiState.creative_mode each frame by the bridge below.
            data_store.insert("creative_mode", std::sync::Mutex::new(true));
            // Dev: stock the "one seed of each" starter set into the player inventory
            // (FarmingSystem drains it). Lets survival mode be tested in early dev.
            data_store.insert(
                "stock_seeds_request",
                std::sync::Mutex::new(Option::<Vec<String>>::None),
            );
            // Mining: commission the player's drone with a MANIFEST (ores + units to
            // fetch); DroneSystem launches one drone per player.
            data_store.insert(
                "commission_drone",
                std::sync::Mutex::new(Option::<(String, Vec<(String, u32)>)>::None),
            );
            // Survival: rest to refill energy (FoodSystem drains it).
            data_store.insert("rest_request", std::sync::Mutex::new(false));
            // Sanitation: compost accumulated waste -> fertilizer (FoodSystem);
            // fertilize a crop by entity bits (FarmingSystem).
            data_store.insert("compost_request", std::sync::Mutex::new(false));
            data_store.insert(
                "fertilize_crop_request",
                std::sync::Mutex::new(Option::<u64>::None),
            );
            // Skill XP grants: the action systems (crafting/farming/mining) push
            // SkillXPEvents onto this channel; SkillSystem (registered LAST) drains
            // + applies them the same frame → level-ups.
            data_store.insert(
                "xp_grants",
                std::sync::Mutex::new(Vec::<SkillXPEvent>::new()),
            );
            // Dev: max all skills (testing affordance — keeps every recipe craftable
            // under the #8b skill-gate, like "Dev: stock materials" for inventory).
            data_store.insert("dev_max_skills", std::sync::Mutex::new(false));
            // Quest progress events: action systems push "craft_<recipe>" /
            // "harvest_<crop>" keys; QuestSystem (registered after them) drains them
            // each frame to advance count-based Craft/Harvest objectives.
            data_store.insert("quest_events", std::sync::Mutex::new(Vec::<String>::new()));
            system_runner.register(InteractionSystem::new());
            system_runner.register(FarmingSystem::new());
            system_runner.register(InventorySystem::new());
            // Enforces typed-container / content-class compatibility (wrong
            // material damages then breaks the vessel). Reads the
            // "container_registry" loaded into the DataStore above.
            system_runner.register(ContainerCompatibilitySystem::new());
            system_runner.register(CraftingSystem::new());
            // FoodSystem: nutrition (eat -> Vitals + buffs), hunger/thirst decay,
            // and spoilage. Reads consume_request + status_effect_registry from the
            // DataStore. Loads food_system.ron from the data dir at construction.
            system_runner.register(FoodSystem::new(&data_dir));
            // DroneSystem: autonomous mining drones (commission → trip → mine a finite
            // asteroid → deliver ore home). Reads commission_drone + item_registry.
            system_runner.register(DroneSystem::new());
            // SkillSystem ticks LAST so it drains the xp_grants the action systems
            // pushed THIS frame (craft → recipe skill, harvest → farming, mine → mining)
            // and applies level-ups before the frame's ECS→GUI sync reads them.
            system_runner.register(SkillSystem::new());
            // QuestSystem ticks after the action + skill systems so it sees this
            // frame's quest_events; advances/completes quests + grants rewards.
            system_runner.register(QuestSystem::new());
            // The player starts the "First Steps" quest (auto-accepted); completing
            // it auto-accepts its dependents (prerequisite chaining in QuestSystem).
            let mut player_quests = QuestTracker::default();
            player_quests.accept_quest("gs_first_steps");
            game_world.world.spawn((
                Transform::default(),
                Velocity::default(),
                Controllable,
                Health::default(),
                Name("Player".to_string()),
                Inventory::new(36),
                crate::ecs::components::Vitals::default(),
                crate::ecs::components::StatusEffects::default(),
                crate::ecs::components::Appearance::default(),
                crate::ecs::components::Outfit::default(),
                PlayerSkills::new(),
                player_quests,
            ));
            // Restore the active offline home's progress (inventory + skills) onto
            // the freshly-spawned player, if a save exists (v0.381, homes inc 3).
            // The ECS player is authoritative (systems tick every frame), so we apply
            // HERE at startup, not on 3D-enter; this also makes the exit-save safe
            // (the player carries the loaded state, so a no-play session round-trips
            // it instead of overwriting with empty).
            if let Some(save) = crate::save_load::load_active_home() {
                crate::save_load::apply_save_to_world(&mut game_world.world, &save);
                log::info!(
                    "Loaded offline home: {} item stacks, {} skills",
                    save.inventory.len(),
                    save.skills.len()
                );
            }
            // Test asteroids for the mining loop (finite ore; DroneSystem deletes one
            // when fully consumed). Dev/testing content — MMO asteroids are server-side.
            game_world.world.spawn((crate::ecs::components::AsteroidBody {
                id: "m12".to_string(),
                name: "Asteroid M-12 (metallic)".to_string(),
                classification: "M".to_string(),
                ores: vec![
                    ("iron_ore_0".to_string(), 120.0),
                    ("nickel_ore_0".to_string(), 60.0),
                    ("platinum_ore_0".to_string(), 20.0),
                ],
                position: [60.0, 12.0, -30.0],
            },));
            game_world.world.spawn((crate::ecs::components::AsteroidBody {
                id: "s7".to_string(),
                name: "Asteroid S-7 (silicaceous)".to_string(),
                classification: "S".to_string(),
                ores: vec![
                    ("iron_ore_0".to_string(), 40.0),
                    ("copper_ore_0".to_string(), 50.0),
                ],
                position: [-45.0, 8.0, 55.0],
            },));

            // Live HOME POWER in MENU mode (v0.518): spawn the home's electrical-role
            // entities now (no meshes) so SolarSystem + ElectricalSystem publish a live
            // PowerStatus the Home page reads even before Enter World. load_world
            // despawns every HomeMachine then re-spawns these WITH meshes on entry, so
            // there is no double-spawn.
            spawn_home_power_entities(&mut game_world.world, &data_dir);

            // Initialize egui
            let egui_ctx = egui::Context::default();
            let egui_state = egui_winit::State::new(
                egui_ctx.clone(),
                egui_ctx.viewport_id(),
                &window,
                None,
                None,
                None,
            );
            let egui_renderer = egui_wgpu::Renderer::new(
                &renderer.device,
                renderer.surface_format(),
                None,
                1,
                false,
            );
            let theme = crate::gui::theme::load_theme();
            theme.apply_to_egui(&egui_ctx);
            // Install the OS-installed emoji font as a fallback so glyphs
            // outside egui's bundled subset (colored hearts, hand gestures,
            // most emoji past the base set) render properly instead of
            // showing as tofu (▢). Silent no-op if the platform font is
            // unavailable. See src/gui/fonts.rs.
            crate::gui::fonts::install_system_emoji_fallback(&egui_ctx);
            // Load + cache the in-app glossary (data/glossary.json) so
            // widgets::definition_text can pop term definitions on
            // Alt+hover. Silent no-op if the file is missing — definition
            // tooltips just won't appear. (v0.195.0.)
            crate::gui::glossary::install();
            let mut gui_state = GuiState::default();

            // Load data-driven catalogs into GUI state
            gui_state.tools_catalog = crate::gui::load_tools_catalog(&data_dir);
            gui_state.help_registry = help_modal::load_help_registry(&data_dir);
            gui_state.onboarding_quest_chains = onboarding::load_quest_chains(&data_dir);
            gui_state.map_planets = crate::gui::load_planets(&data_dir);
            gui_state.places = crate::gui::load_places(&data_dir);
            // Organize-layer inventory pool: restore the SAVED container contents if the
            // active home has any (transfers persisted, v0.517), else seed from the
            // places spine (every leaf item tagged with its container path). The live
            // backpack stays ECS-driven (restored separately by apply_save_to_world).
            gui_state.placed_items = crate::save_load::load_active_home()
                .map(|s| s.placed_items)
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| crate::gui::flatten_placed_items(&gui_state.places));
            gui_state.homestead_design = crate::gui::load_homestead_design(&data_dir);
            // Load the home's machine layout ONCE: both the Home-page self-sufficiency
            // loops AND the construction editor's editable machine layout come from it
            // (v0.519: machine placement). See docs/design/home-design.md.
            gui_state.home_machines = crate::machines::MachineHome::load(
                &data_dir.join("machines").join("home.ron"),
            );
            gui_state.homestead_loops = gui_state
                .home_machines
                .as_ref()
                .map(|h| h.loops.clone())
                .unwrap_or_default();
            gui_state.tower_configs = crate::gui::load_tower_configs(&data_dir);
            gui_state.garden_areas = crate::gui::load_garden_areas(&data_dir);
            gui_state.grow_media = crate::gui::load_grow_media(&data_dir);
            gui_state.library = crate::gui::load_library(&data_dir);
            gui_state.equipment_slots = crate::gui::load_equipment_slots(&data_dir);
            let (sevs, cats) = crate::gui::load_bug_taxonomy(&data_dir);
            gui_state.bug_severities = sevs;
            gui_state.bug_categories = cats;
            gui_state.crafting_category_groups = crate::gui::load_crafting_category_groups(&data_dir);
            // Populate the Crafting page's recipe browser from data/recipes.csv.
            // (The runtime RecipeRegistry is loaded separately into the DataStore for
            // CraftingSystem; this is the GUI-facing projection so the page lists
            // recipes instead of showing the empty "No recipes match" state.)
            gui_state.craft_recipes = crate::gui::load_crafting_recipes(&data_dir);
            // Load the runtime ECS registries (items / recipes / plants / status
            // effects / skills / quests / containers) into the DataStore EAGERLY at
            // startup — so the menu-driven loops (inventory / crafting / skills /
            // quests) work WITHOUT first opening the 3D world. This was the bug: they
            // used to load only in lazy load_world (3D-world view), leaving raw item
            // ids, no recipes, empty skills + the quest shown by id. (load_world
            // re-loads them; idempotent.)
            load_data_registries(&mut data_store, &data_dir);
            gui_state.market_categories = crate::gui::load_market_categories(&data_dir);
            gui_state.studio_scene_presets = crate::gui::load_studio_scenes(&data_dir);
            gui_state.studio_source_presets = crate::gui::load_studio_sources(&data_dir);
            gui_state.profile_skills = crate::gui::load_default_player_skills(&data_dir);
            gui_state.studio_streaming_config = crate::gui::load_studio_streaming_config(&data_dir);
            gui_state.donate_faq = crate::gui::load_donate_faq(&data_dir);
            gui_state.qa_test_tasks = crate::gui::load_qa_test_tasks(&data_dir);
            gui_state.browser_bookmarks = crate::gui::load_browser_bookmarks(&data_dir);
            // v0.197.0: ai_usage_filters loader removed.
            // v0.415.0: resource_categories + onboarding concepts/core-pages
            // loaders removed with their retired pages.
            // Note: TaskPageState reads data/tasks/default_projects.json itself
            // on its first lazy-init (it's a thread-local in pages/tasks.rs).
            // Populate the live studio state from the loaded presets.
            gui_state.studio.sources = gui_state
                .studio_source_presets
                .iter()
                .map(crate::gui::studio_source_from_preset)
                .collect();
            gui_state.studio.scenes = gui_state
                .studio_scene_presets
                .iter()
                .map(crate::gui::studio_scene_from_preset)
                .collect();
            // Default to Earth if present.
            gui_state.map_selected_planet = gui_state
                .map_planets
                .iter()
                .position(|p| p.name == "Earth")
                .or(if gui_state.map_planets.is_empty() { None } else { Some(0) });

            // Load persistent config and apply to GUI state
            let config = crate::config::AppConfig::load();
            config.apply_to_gui_state(&mut gui_state);
            // Push the LOADED settings into the engine on the first frame. Without this the
            // camera boots at CameraController::new's hardcoded sensitivity (and the camera
            // FOV / far-plane stay at their constructor defaults) until the user nudges a
            // slider, so a saved low/high mouse sensitivity never took effect on startup.
            // The settings_dirty block (in the render loop) applies fov + sensitivity +
            // window mode + render distance from gui_state.settings, so trip it once here.
            gui_state.settings_dirty = true;

            // Clean up .old files from previous updates
            crate::updater::Updater::cleanup_old_versions();

            // Auto-check for updates on startup (if enabled)
            if gui_state.updater.channel == crate::updater::UpdateChannel::AlwaysLatest {
                gui_state.updater.check_now();
            }

            // Post-identity routing (v0.198.0, v0.220.0 boot page; v0.415.0 the
            // standalone onboarding page is retired, the Mission Dashboard is the
            // first landing):
            //   - !onboarding_complete: stay on MainMenu (identity / seed setup)
            //   - onboarding_complete && !concept_tour_seen: land on Humanity once
            //     (and mark the tour seen — the dashboard IS the orientation now)
            //   - onboarding_complete && concept_tour_seen: user's chosen boot page
            if gui_state.onboarding_complete {
                gui_state.active_page = if gui_state.concept_tour_seen {
                    gui_state.default_page
                } else {
                    gui_state.concept_tour_seen = true;
                    GuiPage::Humanity
                };
            }

            self.state = Some(EngineState {
                window,
                renderer,
                camera,
                controller,
                asset_manager,
                hot_reload,
                game_world,
                system_runner,
                data_store,
                // 3D world state: empty defaults, loaded lazily on first Enter World
                star_renderer: None,
                floating_origin: crate::renderer::floating_origin::FloatingOrigin::new(),
                planet: None,
                planet_mesh: None,
                planet_material: 0,
                sun_world_pos: glam::DVec3::ZERO,
                sun_material: 0,
                sun_halo_material: 0,
                solar_body_materials: [0; 4],
                solar_orbit_paths: Vec::new(),
                homestead_floors: Vec::new(),
                placeholder_objects: Vec::new(),
                machine_objects: Vec::new(),
                machine_pick: Vec::new(),
                wall_colliders: Vec::new(),
                connection_objects: Vec::new(),
                connection_cyl: None,
                connection_mats: std::collections::HashMap::new(),
                door_panels: Vec::new(),
                door_manual_open: Vec::new(),
                door_locks: Vec::new(),
                door_panel_mesh: None,
                door_slab_mat: None,
                door_glass_mat: None,
                door_energy_open_mat: None,
                door_energy_locked_mat: None,
                door_nanowall_mat: None,
                door_anim_time: 0.0,
                avatar_obj_start: 0,
                avatar_base: Vec3::ZERO,
                fps_spawn: Vec3::new(0.0, 1.7, 0.0),
                showroom_backdrops: Vec::new(),
                showroom_ground: None,
                showroom_body: None,
                showroom_last_backdrop: usize::MAX,
                cosmetics: Vec::new(),
                showroom_return_pos: Vec3::new(0.0, 1.7, 0.0),
                cursor_free: false,
                homestead_walls: None,
                homestead_material_walls: Vec::new(),
                homestead_trim: None,
                homestead_windows: None,
                homestead_mirrors: None,
                homestead_ceiling: None,
                homestead_ceiling_glass: false,
                homestead_layout: None,
                construction_cam_active: false,
                construction_return_pos: Vec3::new(0.0, 1.7, 0.0),
                cursor_pos: (0.0, 0.0),
                construction_grab: None,
                construction_ghost: None,
                construction_structure_ghost: None,
                teleport_cooldown: 0.0,
                wall_tool_mesh: None,
                wall_tool_mat: None,
                construction_node_grab: None,
                construction_grab_press: None,
                construction_node_mesh: None,
                construction_node_mat: None,
                construction_node_mat_hot: None,
                construction_light_mesh: None,
                construction_light_mat: None,
                construction_wall_mat: None,
                construction_node_mat_hover: None,
                construction_char_grab: false,
                construction_char_mesh: None,
                construction_char_pyramid_mesh: None,
                construction_char_mat: None,
                construction_opening_grab: None,
                construction_opening_resize: None,
                construction_opening_mesh: None,
                construction_opening_mat: None,
                construction_gizmo_grab: None,
                construction_gizmo_handle: None,
                construction_gizmo_resize_handle: None,
                construction_hilite: None,
                net_sync: crate::net::sync::NetSyncSystem::new(),
                game_joined: false,
                game_pos_timer: 0.0,
                remote_avatar: None,
                hologram_objects: Vec::new(),
                hologram_orbits: Vec::new(),
                hologram_pins: Vec::new(),
                targeted_planet: None,
                hologram_room_center: Vec3::new(-0.5, 1.0, 2.5),
                room_lights: Vec::new(),
                homestead_bounds: None,
                ship_world_pos: glam::DVec3::ZERO,
                start_time: Instant::now(),
                last_frame: Instant::now(),
                egui_ctx,
                egui_state,
                egui_renderer,
                gui_state,
                theme,
                world_loaded: false,
                window_shown: false,
                data_dir,
                ctrl_held: false,
                shift_held: false,
                lmb_held: false,
                construction_history: ConstructionHistory::default(),
            });
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            event: WindowEvent,
        ) {
            let state = match self.state.as_mut() {
                Some(s) => s,
                None => return,
            };

            // Pass events to egui first, EXCEPT the Tab key while in-game (GuiPage::None).
            // Tab is egui's focus-traversal key; letting egui see it in FPS mode makes egui
            // grab keyboard focus and then swallow WASD, freezing movement until a menu
            // round-trip resets focus (v0.429 bug). In-game, Tab is our reveal-peek (handled
            // in the KeyboardInput arm below), so egui never needs it there.
            let ingame_tab = state.gui_state.active_page == GuiPage::None
                && matches!(
                    &event,
                    WindowEvent::KeyboardInput { event: ke, .. }
                        if ke.physical_key == PhysicalKey::Code(KeyCode::Tab)
                );
            let egui_consumed = if ingame_tab {
                false
            } else {
                state.egui_state.on_window_event(&state.window, &event).consumed
            };

            match event {
                WindowEvent::CloseRequested => {
                    // Persist the active offline home before quitting (v0.381). The
                    // player entity exists from startup, so this captures the loaded
                    // or modified inventory + skills, round-tripping the save.
                    crate::save_load::save_active_home(&state.game_world.world, &state.gui_state.placed_items);
                    event_loop.exit();
                }
                WindowEvent::Resized(size) => {
                    state.renderer.resize(size.width, size.height);
                    state.camera.aspect = state.renderer.aspect_ratio();
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if let PhysicalKey::Code(key) = event.physical_key {
                        let pressed = event.state.is_pressed();

                        // Track Ctrl/Cmd modifier state from raw winit input.
                        // (egui-winit doesn't expose this in a way we can
                        // read for our pre-egui Ctrl+V detection.)
                        if matches!(key, KeyCode::ControlLeft | KeyCode::ControlRight
                            | KeyCode::SuperLeft | KeyCode::SuperRight)
                        {
                            state.ctrl_held = pressed;
                        }
                        if matches!(key, KeyCode::ShiftLeft | KeyCode::ShiftRight) {
                            state.shift_held = pressed; // for Ctrl+Shift+Z redo (v0.575)
                        }
                        // Undo/redo in the construction editor (v0.575): Ctrl+Z undo, Ctrl+Shift+Z (or
                        // Ctrl+Y) redo. Gated to build mode so it never fights the chat Ctrl+V path.
                        if pressed && state.ctrl_held && state.gui_state.construction_active {
                            if key == KeyCode::KeyZ && state.shift_held {
                                construction_redo(state);
                            } else if key == KeyCode::KeyZ {
                                construction_undo(state);
                            } else if key == KeyCode::KeyY {
                                construction_redo(state);
                            }
                        }
                        // Rotate the held STRUCTURE piece with [ and ] (v0.583): 15-degree steps. Only
                        // while a structure is held for placement, so it never fights other keys.
                        if pressed
                            && state.gui_state.construction_active
                            && state.gui_state.construction_structure_type.is_some()
                        {
                            if key == KeyCode::BracketLeft {
                                state.gui_state.construction_structure_yaw =
                                    (state.gui_state.construction_structure_yaw - 15.0).rem_euclid(360.0);
                            } else if key == KeyCode::BracketRight {
                                state.gui_state.construction_structure_yaw =
                                    (state.gui_state.construction_structure_yaw + 15.0).rem_euclid(360.0);
                            }
                        }

                        // F1 (hold) shows the keymap for the current screen/mode. Works on every
                        // page so you can always see what keys do something here. (v0.465)
                        if key == KeyCode::F1 {
                            state.gui_state.keymap_visible = pressed;
                            if pressed && state.gui_state.keymaps.is_empty() {
                                state.gui_state.keymaps =
                                    crate::gui::pages::keymap::load_keymaps(&state.data_dir);
                            }
                            return;
                        }
                        // Diagnostics dev-HUD overlays (v0.482): F2 perf, F3 network,
                        // F4 system. Toggle on press; stack in the top-right corner.
                        if key == KeyCode::F2 && pressed {
                            state.gui_state.show_perf_overlay = !state.gui_state.show_perf_overlay;
                            return;
                        }
                        if key == KeyCode::F3 && pressed {
                            state.gui_state.show_network_overlay = !state.gui_state.show_network_overlay;
                            return;
                        }
                        if key == KeyCode::F4 && pressed {
                            state.gui_state.show_system_overlay = !state.gui_state.show_system_overlay;
                            return;
                        }

                        // Voice push key (v0.490): tracked from RAW winit input so
                        // it works in-game (where egui isn't focused) AND supports
                        // keys egui lacks, like CapsLock (the default push key).
                        // The stored name is the KeyCode debug string (e.g.
                        // "CapsLock", "KeyV"). Also captures a new binding when the
                        // Settings UI is waiting for one.
                        {
                            let name = format!("{:?}", key);
                            if state.gui_state.voice_binding_key {
                                if pressed {
                                    if key != KeyCode::Escape {
                                        // Escape cancels; any other key binds.
                                        state.gui_state.voice_ptt_key = name;
                                        state.gui_state.settings_dirty = true;
                                    }
                                    state.gui_state.voice_binding_key = false;
                                    return;
                                }
                            } else if name == state.gui_state.voice_ptt_key {
                                state.gui_state.voice_ptt_held = pressed;
                            }
                        }

                        // Ctrl+V clipboard image paste — detected HERE at the
                        // raw winit layer because egui-winit intercepts the
                        // paste shortcut, reads clipboard TEXT only, and
                        // returns early without emitting a V key event. For
                        // an image clipboard egui therefore sees no signal
                        // at all. We set a flag the Chat page consumes next
                        // frame (the actual clipboard read + upload lives in
                        // chat.rs so the networking/state code stays there).
                        // Operator-reported 2026-05-15 (3rd attempt).
                        if key == KeyCode::KeyV && pressed && state.ctrl_held
                            && state.gui_state.active_page == GuiPage::Chat
                        {
                            state.gui_state.pending_clipboard_paste = true;
                        }

                        // Escape behavior (v0.195.0):
                        //   1. If the nav back-stack has entries, pop one
                        //      — this is "go back" inside a nested page
                        //      flow (Chat → cog → ServerSettings → Esc
                        //      should return to Chat, not jump to FPS).
                        //   2. If we're on a non-None page and the stack
                        //      is empty, save the page as last_page and
                        //      go to None (FPS mode). Same as before.
                        //   3. If we're already on None, reopen last_page.
                        //      Same as before.
                        //   4. MainMenu always stays put — operator can't
                        //      Esc out of the title screen.
                        if key == KeyCode::Escape && pressed {
                            // Esc cancels the showroom cleanly. Leaving showroom_active
                            // set would make the next Play render the showroom instead
                            // of the world. The character picker (mode 0, opened from a
                            // menu via Play/Characters) returns to that menu; the
                            // in-world mirror/wardrobe (modes 1/2, opened from FPS)
                            // returns to first-person. (v0.476.1)
                            if state.gui_state.showroom_active {
                                let was_picker = state.gui_state.showroom_mode == 0;
                                state.gui_state.showroom_active = false;
                                state.gui_state.showroom_confirm = false;
                                state.controller.showroom_lock = false;
                                state.camera.switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
                                state.camera.position = state.showroom_return_pos;
                                if was_picker {
                                    state.gui_state.active_page = state.gui_state.last_page;
                                }
                                return;
                            }

                            let old_page = state.gui_state.active_page;

                            if state.gui_state.pop_nav_back() {
                                // Did the back-pop. active_page is now the previous nested page.
                            } else {
                                state.gui_state.active_page = match old_page {
                                    GuiPage::None => state.gui_state.last_page,
                                    GuiPage::MainMenu => GuiPage::MainMenu,
                                    other => {
                                        state.gui_state.last_page = other;
                                        GuiPage::None
                                    }
                                };
                            }
                            // Cursor grab is handled by the single per-frame reconciliation
                            // (keys off active_page/showroom/construction). Manipulating it
                            // here too desynced `cursor_free` and broke later panels. (v0.460)
                            let _ = old_page;
                            return;
                        }

                        // Enter toggles chat overlay (only when in-game)
                        if key == KeyCode::Enter && pressed
                            && state.gui_state.active_page == GuiPage::None
                            && !egui_consumed
                        {
                            state.gui_state.show_chat = !state.gui_state.show_chat;
                        }

                        // Tab = HOLD to reveal hidden labels (v0.429): peek markers
                        // through walls across owned/explored rooms at x3 distance.
                        // Tracks held state (true on press, false on release).
                        if key == KeyCode::Tab {
                            state.gui_state.reveal_held = pressed;
                            if state.gui_state.active_page == GuiPage::None {
                                return; // consume Tab in-game (it was inventory pre-v0.429)
                            }
                        }
                        // I opens inventory (took over from Tab when Tab became the reveal peek).
                        if key == KeyCode::KeyI && pressed
                            && state.gui_state.active_page == GuiPage::None
                        {
                            state.gui_state.active_page = GuiPage::Inventory;
                            // Cursor freed by the per-frame reconciliation (active_page != None).
                            return;
                        }
                        // E opens/pins the targeted machine's info card (walk-up
                        // interaction, v0.431). No early return: E still drives
                        // input.interact for the ECS interaction system below.
                        if key == KeyCode::KeyE && pressed
                            && state.gui_state.active_page == GuiPage::None
                        {
                            if let Some(cp) = state.gui_state.targeted_control_panel {
                                // Looking at a door control panel (v0.567 + v0.570 locks): if the door
                                // is LOCKED, E UNLOCKS it (Stage 1 unlocks every lock -- key/code/skill
                                // enforcement is a follow-up); once unlocked, E opens/closes a manual
                                // door. So a locked door takes two presses: unlock, then open.
                                let locked = state
                                    .door_panels
                                    .get(cp)
                                    .map_or(false, |panel| door_locked_now(&panel.0, state.door_locks.get(cp)));
                                if locked {
                                    if let Some(live) = state.door_locks.get_mut(cp) {
                                        for s in live.iter_mut() {
                                            if *s == crate::ship::lock_types::LockState::Locked {
                                                *s = crate::ship::lock_types::LockState::Unlocked;
                                            }
                                        }
                                    }
                                } else if let Some(panel) = state.door_panels.get(cp) {
                                    if !panel.0.auto_open {
                                        if let Some(m) = state.door_manual_open.get_mut(cp) {
                                            *m = !*m;
                                        }
                                    }
                                }
                            } else if let Some(t) = state.gui_state.targeted_machine {
                                // Looking at a machine: toggle its card open/closed.
                                state.gui_state.selected_machine =
                                    if state.gui_state.selected_machine == Some(t) {
                                        None
                                    } else {
                                        Some(t)
                                    };
                            } else if state.gui_state.selected_machine.is_some() {
                                // Not looking at any machine but a card is pinned: E closes it
                                // (so "[E] close" works from anywhere, not just at the machine).
                                state.gui_state.selected_machine = None;
                            } else if !state.gui_state.showroom_active {
                                // Walk-up to a character station: the wetroom mirror opens the
                                // appearance editor, the bedroom opens the wardrobe (v0.442).
                                let p = state.camera.position;
                                let room = state
                                    .gui_state
                                    .room_bounds
                                    .iter()
                                    .find(|r| {
                                        p.x >= r.min.x && p.x <= r.max.x
                                            && p.y >= r.min.y && p.y <= r.max.y
                                            && p.z >= r.min.z && p.z <= r.max.z
                                    })
                                    .map(|r| r.id.clone());
                                match room.as_deref() {
                                    Some("wetroom") => open_showroom(state, 1),
                                    Some("bedroom") => open_showroom(state, 2),
                                    _ => {}
                                }
                            }
                        }

                        // R toggles the home roof/ceiling (construction mode, v0.453). Off by
                        // default so the sky (stars + the real solar system) shows through the
                        // open top; on for a sealed look or atmosphere tests. Also exposed as a
                        // checkbox on the Settings page (GUI-first).
                        if key == KeyCode::KeyR && pressed
                            && state.gui_state.active_page == GuiPage::None
                            && !state.gui_state.showroom_active
                        {
                            state.gui_state.show_roof = !state.gui_state.show_roof;
                            return;
                        }

                        // B toggles the construction editor (v0.455): craft each room's walls
                        // live. On open, mirror the live layout into the editable GuiState.
                        if key == KeyCode::KeyB && pressed
                            && state.gui_state.active_page == GuiPage::None
                            && !state.gui_state.showroom_active
                        {
                            state.gui_state.construction_active = !state.gui_state.construction_active;
                            // Clear any held placement item on entering/leaving build mode, so a
                            // stale held type can't make the next viewport click drop a machine in
                            // the wrong context. (v0.531)
                            state.gui_state.construction_place_type = None;
                            state.construction_ghost = None;
                            if state.gui_state.construction_active {
                                if let Some(layout) = &state.homestead_layout {
                                    // PIN EVERY room to its current resolved position on open, so
                                    // editing one room no longer reshuffles the auto-laid-out
                                    // others (the operator's "I felt lost as the rooms rearranged
                                    // themselves"). The whole home becomes an explicit floor plan.
                                    let resolved = crate::ship::fibonacci::resolve_positions(layout);
                                    state.gui_state.construction_rooms = layout.rooms.iter()
                                        .enumerate()
                                        .map(|(i, rc)| {
                                            let w = &rc.walls;
                                            let pos = rc.position.unwrap_or_else(|| {
                                                let r = resolved[i];
                                                [r.x, r.y, r.z]
                                            });
                                            crate::gui::ConstructionRoom {
                                                id: rc.id.clone(),
                                                walls: [w.north, w.south, w.west, w.east],
                                                wall_offsets: w.offsets,
                                                openings: rc.openings.iter().map(|o| {
                                                    use crate::ship::fibonacci::OpeningKind as OK;
                                                    crate::gui::EditorOpening {
                                                        kind: match o.kind {
                                                            OK::Door => crate::gui::EditorOpeningKind::Door,
                                                            OK::Airlock => crate::gui::EditorOpeningKind::Airlock,
                                                            // Window + Hatch both edit as Window in the mirror.
                                                            _ => crate::gui::EditorOpeningKind::Window,
                                                        },
                                                        wall: (o.wall as usize).min(3),
                                                        u: o.u, v: o.v, w: o.w, h: o.h,
                                                    }
                                                }).collect(),
                                                level: rc.level,
                                                position: Some(pos),
                                                dimensions: rc.dimensions,
                                                material_type: rc.material_type,
                                                color: rc.color,
                                            }
                                        })
                                        .collect();
                                    state.gui_state.construction_height = if layout.default_wall_height > 0.0 {
                                        layout.default_wall_height
                                    } else {
                                        3.0
                                    };
                                }
                                // Add-Room picker options: room-type ids from the registry (sorted).
                                let reg = crate::ship::room_types::RoomTypeRegistry::load(&state.data_dir);
                                let mut types: Vec<String> = reg.types.keys().cloned().collect();
                                types.sort();
                                if state.gui_state.construction_add_type.is_empty() {
                                    state.gui_state.construction_add_type = types.first().cloned().unwrap_or_default();
                                }
                                state.gui_state.construction_room_types = types;
                            }
                            return;
                        }

                        // Don't pass input to the game when egui consumed it or a menu is open.
                        // During construction we DO pass it: WASD flies the orbit focal point,
                        // Space/Shift change level (egui_consumed still wins, so typing in a
                        // field never moves the camera). (v0.464)
                        if egui_consumed || state.gui_state.active_page != GuiPage::None {
                            return;
                        }

                        state.controller.process_keyboard(key, event.state);

                        // Update InputState in DataStore for game systems
                        let mut input = state.data_store
                            .get::<InputState>("input_state")
                            .cloned()
                            .unwrap_or_default();
                        match key {
                            KeyCode::KeyW => input.forward = pressed,
                            KeyCode::KeyS => input.backward = pressed,
                            KeyCode::KeyA => input.left = pressed,
                            KeyCode::KeyD => input.right = pressed,
                            KeyCode::Space => input.jump = pressed,
                            KeyCode::KeyE => input.interact = pressed,
                            _ => {}
                        }
                        state.data_store.insert("input_state", input);
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    // Cache the cursor pixel position for 3D picking (v0.466). Unconditional so
                    // it stays fresh regardless of which mode we're in.
                    state.cursor_pos = (position.x as f32, position.y as f32);
                }
                WindowEvent::MouseInput { button, state: btn_state, .. } => {
                    use winit::event::{ElementState, MouseButton};
                    let left = button == MouseButton::Left;
                    let right = button == MouseButton::Right;
                    let pressed = btn_state == ElementState::Pressed;
                    if left {
                        state.lmb_held = pressed; // undo drag-coalescing (v0.575)
                    }
                    // Construction astral editor: LEFT grabs/drops a room (left is a no-op in the
                    // orbit cam, so we own it). Gated on !egui_consumed so panel clicks never
                    // start a grab. (v0.466)
                    if state.gui_state.construction_active && left && !egui_consumed {
                        if pressed {
                            // Wall-drawing mode owns the click first (v0.534): drop a corner node.
                            // Else holding a palette item -> drop it; else grab a room.
                            if state.gui_state.construction_wall_mode {
                                try_place_wall_node(state);
                            } else if state.gui_state.construction_place_type.is_some() {
                                try_place_held_machine(state);
                            } else if state.gui_state.construction_structure_type.is_some() {
                                // Holding a STRUCTURAL piece (v0.583) -> drop it on the floor.
                                try_place_structure(state);
                            } else if state.gui_state.home_structure.is_some() && try_grab_opening_resize(state) {
                                // Grabbed an opening RESIZE handle (v0.578): the per-frame drag resizes
                                // the door/window (width via left/right, height via top/bottom).
                            } else if state.gui_state.home_structure.is_some() && try_grab_opening(state) {
                                // Grabbed a door/window opening gizmo (v0.546): the per-frame drag
                                // slides it along its wall.
                            } else if state.gui_state.home_structure.is_some() && try_grab_node(state) {
                                // Grabbed a wall corner-node gizmo (v0.541): the per-frame drag moves
                                // it (+ any walls sharing it) with snapping.
                            } else if try_grab_char(state) {
                                // Grabbed the build-mode avatar (v0.557): drag it across the floor; you
                                // spawn right there when you leave build mode.
                            } else if state.gui_state.home_structure.is_some() && try_pick_light(state) {
                                // Clicked a placed-LIGHT diamond gizmo (v0.576) -> its detail shows on
                                // the right panel, like a wall.
                            } else if state.gui_state.home_structure.is_some() && try_pick_structure(state) {
                                // Clicked a placed STRUCTURE (v0.583) -> its detail shows on the right.
                            } else if state.gui_state.home_structure.is_some() && try_pick_machine(state) {
                                // Selected a machine in the viewport (v0.553) -> its detail shows on
                                // the right panel. Click only; machines are not dragged here. Gated to
                                // the box-home path so it never shadows the legacy room-grab.
                            } else if state.gui_state.home_structure.is_some() && try_pick_wall(state) {
                                // Clicked a WALL SURFACE (v0.573): select that wall for editing --
                                // unambiguous vs hunting for the right corner orb at an intersection.
                            } else {
                                try_begin_room_grab(state);
                            }
                        } else {
                            // Release. A gizmo grab that never crossed the drag threshold (grab_press
                            // still set) is a TAP -> SELECT its wall + show it on the right panel,
                            // instead of moving it (v0.549). Click to inspect, click-and-hold to move.
                            if state.construction_grab_press.is_some() {
                                if let Some(c) = state.construction_node_grab {
                                    let sel = state.gui_state.home_structure.as_ref().and_then(|hs| {
                                        hs.walls.iter().position(|w| {
                                            ((w.a.0 - c.0).abs() < 0.05 && (w.a.1 - c.1).abs() < 0.05)
                                                || ((w.b.0 - c.0).abs() < 0.05 && (w.b.1 - c.1).abs() < 0.05)
                                        })
                                    });
                                    if let Some(i) = sel {
                                        state.gui_state.construction_wall_selected = Some(i);
                                        state.gui_state.construction_machine_selected = None;
                                    }
                                } else if let Some((wi, _)) = state.construction_opening_grab {
                                    state.gui_state.construction_wall_selected = Some(wi);
                                    state.gui_state.construction_machine_selected = None;
                                }
                            }
                            state.construction_grab = None; // release; keep the selection highlighted
                            state.construction_gizmo_grab = None; // release a slid handle too
                            state.construction_node_grab = None; // release a dragged corner node
                            state.construction_opening_grab = None; // release a dragged opening
                            state.construction_opening_resize = None; // release a resize handle (v0.578)
                            state.construction_char_grab = false; // release a dragged avatar (v0.557)
                            state.construction_grab_press = None;
                        }
                    } else if state.gui_state.construction_active
                        && right
                        && pressed
                        && (state.gui_state.construction_place_type.is_some()
                            || state.gui_state.construction_structure_type.is_some()
                            || state.gui_state.construction_wall_mode)
                    {
                        // Right-click cancels the held placement item / structure OR exits
                        // wall-drawing and clears the pending corner (v0.529/v0.534/v0.583).
                        state.gui_state.construction_place_type = None;
                        state.gui_state.construction_structure_type = None;
                        state.gui_state.construction_wall_mode = false;
                        state.gui_state.construction_wall_start = None;
                    } else if !egui_consumed && state.gui_state.active_page == GuiPage::None {
                        state.controller.process_mouse_button(button, btn_state);
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    if !egui_consumed && state.gui_state.active_page == GuiPage::None {
                        let scroll = match delta {
                            winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                            winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
                        };
                        state.controller.process_scroll(scroll);
                    }
                }
                WindowEvent::RedrawRequested => {
                    let now = Instant::now();
                    let dt = (now - state.last_frame).as_secs_f32().min(0.1);
                    state.last_frame = now;

                    // Poll hot-reload for file changes
                    let changes = state.hot_reload.poll(&mut state.asset_manager);
                    for changed in &changes {
                        log::info!("Hot-reload: {changed}");
                    }

                    // Apply the player's active status-effect SPEED modifiers to movement
                    // (well_nourished speeds you up; thirsty/flu slow you down). Look is
                    // unaffected. Collect the effect ids first (owned → releases the world
                    // borrow), then resolve the net multiplier from the registry.
                    {
                        let ids: Vec<String> = state
                            .game_world
                            .world
                            .query::<(
                                &crate::ecs::components::StatusEffects,
                                &Controllable,
                            )>()
                            .iter()
                            .next()
                            .map(|(_, (fx, _))| fx.active.iter().map(|e| e.id.clone()).collect())
                            .unwrap_or_default();
                        let mult = state
                            .data_store
                            .get::<crate::systems::status_effects::StatusEffectRegistry>(
                                "status_effect_registry",
                            )
                            .map(|reg| reg.net_stat_multiplier(ids.iter().map(|s| s.as_str()), "speed"))
                            .unwrap_or(1.0);
                        state.controller.speed_multiplier = mult;
                    }

                    // Keep the player grounded on the floor of whatever room they are in
                    // (so gravity + jump land on the right deck). Falls back to the last
                    // floor when outside every room. Room floors are coplanar in the home.
                    {
                        let p = state.camera.position;
                        if let Some(mut floor) = state
                            .gui_state
                            .room_bounds
                            .iter()
                            .find(|r| {
                                p.x >= r.min.x && p.x <= r.max.x && p.z >= r.min.z && p.z <= r.max.z
                            })
                            .map(|r| r.min.y)
                        {
                            // Walk UP stairs / ramps + ONTO platforms (v0.584): raise the ground to the
                            // highest reachable structure surface under the player. A STEP_UP cap means a
                            // tall solid box can't yank you up its side -- you use the stairs. Descending
                            // is always allowed (a lower surface just lowers the floor next frame).
                            // `feet` for the STEP_UP cap: the lagging REST floor normally (so a jump
                            // can't cheese you onto a tall box -- the v0.584 intent), but the player's
                            // LIVE height while at a ladder, so a deck at the ladder top is reachable as
                            // you climb (the rest floor lags at the base). `max` never lowers it.
                            // Stairs need neither -- you are grounded on each step, so the rest floor
                            // already tracks them. (v0.589, gated after a movement review.)
                            if let Some(hs) = state.gui_state.home_structure.as_ref() {
                                let feet = if state.controller.in_climb_zone() {
                                    (p.y - state.controller.eye_height()).max(state.controller.ground_floor())
                                } else {
                                    state.controller.ground_floor()
                                };
                                const STEP_UP: f32 = 0.6;
                                for ps in &hs.structures {
                                    if let Some(ty) = crate::ship::structure::structure_type(&ps.type_id) {
                                        if let Some(top) = crate::ship::structure::walk_surface(
                                            ty, ps.pos, ps.rot_deg.to_radians(), p.x, p.z,
                                        ) {
                                            if top <= feet + STEP_UP && top > floor {
                                                floor = top;
                                            }
                                        }
                                    }
                                }
                            }
                            state.controller.set_ground_floor(floor);
                        }
                    }

                    // Teleporter pads (v0.584): stepping onto a teleporter that has a linked pair jumps
                    // the player to the partner pad. A cooldown (set on jump, also blocks arrival re-fire)
                    // prevents ping-ponging while you stand on the destination. First person, not build.
                    if state.teleport_cooldown > 0.0 {
                        state.teleport_cooldown = (state.teleport_cooldown - dt).max(0.0);
                    }
                    if state.camera.mode == crate::renderer::camera::CameraMode::FirstPerson
                        && !state.gui_state.construction_active
                        && state.teleport_cooldown <= 0.0
                    {
                        let p = state.camera.position;
                        let jump: Option<(f32, f32, f32)> =
                            state.gui_state.home_structure.as_ref().and_then(|hs| {
                                for ps in &hs.structures {
                                    let ty = crate::ship::structure::structure_type(&ps.type_id)?;
                                    if ty.kind != crate::ship::structure::StructureKind::Teleporter {
                                        continue;
                                    }
                                    let Some(pair) = ps.pair else { continue };
                                    if pair >= hs.structures.len() {
                                        continue;
                                    }
                                    if crate::ship::structure::in_footprint(
                                        ty, ps.pos, ps.rot_deg.to_radians(), p.x, p.z,
                                    ) {
                                        return Some(hs.structures[pair].pos);
                                    }
                                }
                                None
                            });
                        if let Some(dest) = jump {
                            state.camera.position.x = dest.0;
                            state.camera.position.z = dest.2;
                            state.teleport_cooldown = 1.2; // seconds; clears once you step off the pad
                        }
                    }

                    // Ladder CLIMB zone (v0.589): if the player stands at a ladder, tell the controller
                    // its span so an up/down input climbs it (instead of jumping/falling). First person,
                    // not build. Proximity (XZ within the ladder footprint + a reach margin) is the
                    // trigger -- you do not have to be pixel-perfect on the rungs.
                    if state.camera.mode == crate::renderer::camera::CameraMode::FirstPerson
                        && !state.gui_state.construction_active
                    {
                        let p = state.camera.position;
                        let zone = state.gui_state.home_structure.as_ref().and_then(|hs| {
                            for ps in &hs.structures {
                                let ty = crate::ship::structure::structure_type(&ps.type_id)?;
                                if ty.kind != crate::ship::structure::StructureKind::Ladder {
                                    continue;
                                }
                                let (dx, dz) = (p.x - ps.pos.0, p.z - ps.pos.2);
                                // Generous reach (v0.589 review fix): a ladder mounted flush against a
                                // wall gets the player pushed ~radius+half_thickness off the wall by the
                                // XZ collider; a wider zone keeps them ON the ladder instead of dropping.
                                let reach = ty.size.0.max(ty.size.2) * 0.5 + 0.9;
                                if dx * dx + dz * dz <= reach * reach {
                                    return Some((ps.pos.1, ps.pos.1 + ty.size.1));
                                }
                            }
                            None
                        });
                        state.controller.set_climb_zone(zone);
                    } else {
                        state.controller.set_climb_zone(None);
                    }

                    // Update camera from input (capture the pre-move position for swept collision).
                    let prev_cam_pos = state.camera.position;
                    state.controller.update_camera(&mut state.camera, dt);

                    // Wall collision (v0.556): the player IS the camera, so push it out of the home's
                    // walls / closed doors in first person -- no more walking through walls. Doors that
                    // are open + unlocked are gaps; closed or locked doors block; windows are part of
                    // their wall span. Skipped in the build editor (orbit cam) + when there are no
                    // home walls (legacy layout / showroom -> wall_colliders empty).
                    if state.camera.mode == crate::renderer::camera::CameraMode::FirstPerson
                        && !state.gui_state.construction_active
                        && !state.wall_colliders.is_empty()
                    {
                        let door_locks = state.door_locks.clone();
                        let doors: Vec<crate::ship::wall_collision::WallSegment> = state
                            .door_panels
                            .iter()
                            .enumerate()
                            .filter_map(|(i, (p, open))| {
                                // Windows are handled by their (uncut) wall span; a door blocks only
                                // when closed or locked (v0.570: lock-list aware).
                                let locked = door_locked_now(p, door_locks.get(i));
                                if p.is_window || (*open >= 0.5 && !locked) {
                                    return None;
                                }
                                let half_w = p.size.x * 0.5;
                                let dir = p.rotation * Vec3::new(1.0, 0.0, 0.0);
                                let c = p.center;
                                Some(crate::ship::wall_collision::WallSegment {
                                    a: (c.x - dir.x * half_w, c.z - dir.z * half_w),
                                    b: (c.x + dir.x * half_w, c.z + dir.z * half_w),
                                    half_thickness: (p.size.z * 0.5).max(0.05),
                                })
                            })
                            .collect();
                        let resolved = crate::ship::wall_collision::resolve(
                            prev_cam_pos,
                            state.camera.position,
                            crate::ship::wall_collision::PLAYER_RADIUS,
                            &state.wall_colliders,
                            &doors,
                        );
                        state.camera.position = resolved;
                    }

                    // Camera stays in local ship coords (no floating origin reset)
                    // Floating origin is only used for rendering distant bodies

                    // Sync camera state into DataStore for game systems
                    state.data_store.insert("camera_position", state.camera.position);
                    let (yaw_sin, yaw_cos) = state.camera.yaw.sin_cos();
                    let forward = Vec3::new(-yaw_sin, 0.0, -yaw_cos).normalize();
                    state.data_store.insert("camera_forward", forward);
                    state.data_store.insert("camera_yaw", state.camera.yaw);

                    // Walk-up interaction (v0.431): the machine the player is looking at,
                    // nearest within range inside a look-cone. Drives the [E] prompt + card.
                    {
                        let cp = state.camera.position;
                        let cf = state.camera.forward();
                        let mut best: Option<(usize, f32)> = None;
                        for (i, label) in state.gui_state.machine_labels.iter().enumerate() {
                            let to = label.pos - cp;
                            let dist = to.length();
                            if !(0.05..=5.0).contains(&dist) {
                                continue;
                            }
                            if (to / dist).dot(cf) < 0.9 {
                                continue; // outside the ~25-degree look cone
                            }
                            if best.map_or(true, |b| dist < b.1) {
                                best = Some((i, dist));
                            }
                        }
                        state.gui_state.targeted_machine = best.map(|b| b.0);
                    }

                    // Walk-up to a door you can interact with (v0.567 control panel + v0.570 locks): the
                    // nearest within arm's reach that the player is facing. A MANUAL control-panel door
                    // (open/close) OR any LOCKED door with locks (unlock) qualifies -- so a locked AUTO
                    // door, or a panel-less locked door, can still be unlocked at its lock indicators
                    // instead of being a dead-end. Drives the prompt; E unlocks-then-opens.
                    {
                        let cp_pos = state.camera.position;
                        let cf = state.camera.forward();
                        let mut best: Option<(usize, f32)> = None;
                        if state.camera.mode == crate::renderer::camera::CameraMode::FirstPerson
                            && !state.gui_state.construction_active
                            && state.gui_state.active_page == GuiPage::None
                        {
                            let door_locks = state.door_locks.clone();
                            for (i, (p, _open)) in state.door_panels.iter().enumerate() {
                                if p.is_window {
                                    continue;
                                }
                                let panel_door = p.control_panel && !p.auto_open;
                                // A door with locks is interactable while LOCKED (to unlock), and -- if
                                // MANUAL -- also once unlocked (its locks double as the open surface, so
                                // a manual lock-door needs no separate control panel). An AUTO door drops
                                // out once unlocked because it then opens on its own.
                                let lock_door = !p.locks.is_empty()
                                    && (door_locked_now(p, door_locks.get(i)) || !p.auto_open);
                                if !panel_door && !lock_door {
                                    continue;
                                }
                                // Interact point: the control panel if any, else the first lock indicator.
                                let ipos = if panel_door {
                                    p.control_panel_pos
                                } else {
                                    p.locks.first().map_or(Vec3::new(p.center.x, 1.2, p.center.z), |l| l.pos)
                                };
                                let to = ipos - cp_pos;
                                let dist = to.length();
                                if !(0.1..=2.5).contains(&dist) {
                                    continue;
                                }
                                if (to / dist).dot(cf) < 0.55 {
                                    continue; // not facing it (~57-degree cone)
                                }
                                if best.map_or(true, |b| dist < b.1) {
                                    best = Some((i, dist));
                                }
                            }
                        }
                        state.gui_state.targeted_control_panel = best.map(|b| b.0);
                        // Precompute the crosshair prompt (the HUD can't see the door's open/locked
                        // state, which lives here in EngineState). (v0.567)
                        state.gui_state.control_panel_prompt = match best.map(|b| b.0) {
                            Some(i) => {
                                let locked = state
                                    .door_panels
                                    .get(i)
                                    .map_or(false, |p| door_locked_now(&p.0, state.door_locks.get(i)));
                                let open = state.door_manual_open.get(i).copied().unwrap_or(false);
                                if locked {
                                    "[E] unlock door".to_string()
                                } else if open {
                                    "[E] close door".to_string()
                                } else {
                                    "[E] open door".to_string()
                                }
                            }
                            None => String::new(),
                        };
                    }

                    // Survival environment context: is the player inside the sealed
                    // homestead volume (oxygenated/heated) or exposed (vacuum/cold)?
                    // FoodSystem reads this to drive oxygen + body temperature.
                    {
                        // Exposed ambient temperature comes from the current weather
                        // (winter / storms make the outside deadlier); -40 fallback.
                        let exposed_temp = state
                            .data_store
                            .get::<std::sync::Mutex<Weather>>("weather")
                            .and_then(|m| m.lock().ok())
                            .map(|w| w.temperature)
                            .unwrap_or(-40.0);
                        let pos = state.camera.position;
                        let env = match state.homestead_bounds {
                            Some((mn, mx))
                                if pos.x >= mn.x && pos.x <= mx.x
                                    && pos.y >= mn.y && pos.y <= mx.y
                                    && pos.z >= mn.z && pos.z <= mx.z =>
                            {
                                // Inside the homestead — sealed, oxygenated, comfortable.
                                crate::ecs::components::EnvironmentContext::default()
                            }
                            Some(_) => crate::ecs::components::EnvironmentContext {
                                // Outside the hull — vacuum + deep cold.
                                sealed: false,
                                oxygenated: false,
                                ambient_temp_c: exposed_temp,
                            },
                            // Homestead not generated yet → assume safe.
                            None => crate::ecs::components::EnvironmentContext::default(),
                        };
                        state.data_store.insert("environment_context", env);
                    }

                    // Bridge GUI craft/dev commands into the DataStore so the ECS
                    // CraftingSystem (which only gets &DataStore in tick) acts on them
                    // this frame. Take/clear the GuiState flags.
                    if let Some(recipe_id) = state.gui_state.pending_craft_recipe.take() {
                        if let Some(slot) = state
                            .data_store
                            .get::<std::sync::Mutex<Option<String>>>("craft_request")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = Some(recipe_id);
                            }
                        }
                    }
                    if state.gui_state.dev_stock_materials {
                        state.gui_state.dev_stock_materials = false;
                        if let Some(slot) = state
                            .data_store
                            .get::<std::sync::Mutex<bool>>("dev_stock_materials")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = true;
                            }
                        }
                    }
                    // Eat: bridge the clicked food item to FoodSystem's consume channel.
                    if let Some(item_id) = state.gui_state.pending_consume_item.take() {
                        if let Some(slot) = state
                            .data_store
                            .get::<std::sync::Mutex<Option<String>>>("consume_request")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = Some(item_id);
                            }
                        }
                    }
                    // Drink: bridge the clicked beverage to FoodSystem's drink channel.
                    if let Some(item_id) = state.gui_state.pending_drink_item.take() {
                        if let Some(slot) = state
                            .data_store
                            .get::<std::sync::Mutex<Option<String>>>("drink_request")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = Some(item_id);
                            }
                        }
                    }
                    // Gardening: bridge plant/water/harvest/dev-grow to FarmingSystem.
                    if let Some(seed_id) = state.gui_state.pending_plant_seed.take() {
                        if let Some(slot) = state
                            .data_store
                            .get::<std::sync::Mutex<Option<String>>>("plant_request")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = Some(seed_id);
                            }
                        }
                    }
                    // Plant a whole tower (v0.386): the GUI sends (tower id, plant ids).
                    if let Some(tower_planting) = state.gui_state.pending_plant_tower.take() {
                        if let Some(slot) = state
                            .data_store
                            .get::<std::sync::Mutex<Option<(String, Vec<String>)>>>("plant_tower_request")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = Some(tower_planting);
                            }
                        }
                    }
                    if let Some(bits) = state.gui_state.pending_water_crop.take() {
                        if let Some(slot) = state
                            .data_store
                            .get::<std::sync::Mutex<Option<u64>>>("water_request")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = Some(bits);
                            }
                        }
                    }
                    if let Some(bits) = state.gui_state.pending_harvest_crop.take() {
                        if let Some(slot) = state
                            .data_store
                            .get::<std::sync::Mutex<Option<u64>>>("harvest_request")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = Some(bits);
                            }
                        }
                    }
                    if state.gui_state.dev_grow_crops {
                        state.gui_state.dev_grow_crops = false;
                        if let Some(slot) =
                            state.data_store.get::<std::sync::Mutex<bool>>("dev_grow_crops")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = true;
                            }
                        }
                    }
                    // Creative/survival mode: mirror the flag EVERY frame (not
                    // one-shot) so the farming + crafting systems see the current
                    // mode. Creative = skip resource requirements + consumption.
                    if let Some(slot) =
                        state.data_store.get::<std::sync::Mutex<bool>>("creative_mode")
                    {
                        if let Ok(mut s) = slot.lock() {
                            *s = state.gui_state.creative_mode;
                        }
                    }
                    // Per-area irrigation: mirror the garden edit modal's water sliders
                    // (GuiState.garden_irrigation, keyed by tower_id) into the sim every
                    // frame so FarmingSystem keeps configured crops topped up.
                    if let Some(slot) = state
                        .data_store
                        .get::<std::sync::Mutex<std::collections::HashMap<String, f32>>>(
                            "garden_irrigation",
                        )
                    {
                        if let Ok(mut s) = slot.lock() {
                            if *s != state.gui_state.garden_irrigation {
                                *s = state.gui_state.garden_irrigation.clone();
                            }
                        }
                    }
                    if let Some(slot) = state
                        .data_store
                        .get::<std::sync::Mutex<std::collections::HashMap<String, f32>>>(
                            "garden_nutrient",
                        )
                    {
                        if let Ok(mut s) = slot.lock() {
                            if *s != state.gui_state.garden_nutrient {
                                *s = state.gui_state.garden_nutrient.clone();
                            }
                        }
                    }
                    // Backpack <-> container transfers: drain the GUI's pending ops into
                    // the InventorySystem channel (it applies them to the player backpack).
                    if !state.gui_state.pending_inventory_transfers.is_empty() {
                        let ops = std::mem::take(&mut state.gui_state.pending_inventory_transfers);
                        if let Some(slot) = state
                            .data_store
                            .get::<std::sync::Mutex<Vec<(String, u32, bool)>>>("inventory_transfer_ops")
                        {
                            if let Ok(mut s) = slot.lock() {
                                s.extend(ops);
                            }
                        }
                    }
                    // Skills: bridge the "Dev: max skills" button to SkillSystem.
                    if state.gui_state.pending_dev_max_skills {
                        state.gui_state.pending_dev_max_skills = false;
                        if let Some(slot) =
                            state.data_store.get::<std::sync::Mutex<bool>>("dev_max_skills")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = true;
                            }
                        }
                    }
                    // Mining: bridge a commissioned drone order (target asteroid id +
                    // manifest) to DroneSystem.
                    if let Some(order) = state.gui_state.pending_drone_manifest.take() {
                        if let Some(slot) = state.data_store.get::<std::sync::Mutex<
                            Option<(String, Vec<(String, u32)>)>,
                        >>("commission_drone")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = Some(order);
                            }
                        }
                    }
                    // Survival: bridge the Rest button to FoodSystem's rest channel.
                    if state.gui_state.pending_rest {
                        state.gui_state.pending_rest = false;
                        if let Some(slot) =
                            state.data_store.get::<std::sync::Mutex<bool>>("rest_request")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = true;
                            }
                        }
                    }
                    // Sanitation: bridge Compost (FoodSystem) + Fertilize (FarmingSystem).
                    if state.gui_state.pending_compost {
                        state.gui_state.pending_compost = false;
                        if let Some(slot) =
                            state.data_store.get::<std::sync::Mutex<bool>>("compost_request")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = true;
                            }
                        }
                    }
                    if let Some(bits) = state.gui_state.pending_fertilize_crop.take() {
                        if let Some(slot) = state
                            .data_store
                            .get::<std::sync::Mutex<Option<u64>>>("fertilize_crop_request")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = Some(bits);
                            }
                        }
                    }
                    // Dev: stock the starter seed set (one of each tower variety).
                    if let Some(seeds) = state.gui_state.pending_stock_seeds.take() {
                        if let Some(slot) = state
                            .data_store
                            .get::<std::sync::Mutex<Option<Vec<String>>>>("stock_seeds_request")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = Some(seeds);
                            }
                        }
                    }

                    // Tick all ECS systems
                    state.system_runner.tick(
                        &mut state.game_world.world,
                        dt,
                        &state.data_store,
                    );

                    // ── Multiplayer co-presence (v0.472) ──────────────────────────────────────
                    // While actually in the 3D world AND connected to a relay, join the shared game
                    // world once (over the existing authenticated chat socket), stream our position,
                    // and apply remote players. The relay validates + broadcasts; net_sync spawns /
                    // moves / interpolates RemotePlayer entities, which the render pass draws.
                    {
                        let in_world = state.gui_state.active_page == GuiPage::None
                            && !state.gui_state.showroom_active
                            && !state.gui_state.construction_active;
                        let connected = state
                            .gui_state
                            .ws_client
                            .as_ref()
                            .map_or(false, |w| w.is_connected());
                        if in_world && connected {
                            if !state.game_joined {
                                let name = if state.gui_state.character_name.trim().is_empty() {
                                    "Wanderer".to_string()
                                } else {
                                    state.gui_state.character_name.clone()
                                };
                                if let Some(ref ws) = state.gui_state.ws_client {
                                    // character_mode is reserved for the open/closed-server model
                                    // (relay ignores extra fields today; envelope right from day one).
                                    let join = serde_json::json!({
                                        "type": "game_join",
                                        "player_name": name,
                                        "character_mode": "local",
                                    });
                                    ws.send(&join.to_string());
                                }
                                state.game_joined = true;
                                state.game_pos_timer = 0.0;
                            }
                            state.game_pos_timer += dt;
                            if state.game_pos_timer >= 1.0 / 15.0 {
                                state.game_pos_timer = 0.0;
                                send_game_position(state);
                            }
                            // `tick` is the System trait method; call it fully-qualified.
                            crate::ecs::systems::System::tick(
                                &mut state.net_sync,
                                &mut state.game_world.world,
                                dt,
                                &state.data_store,
                            );
                        } else if state.game_joined {
                            // Left the world: allow a fresh join next time + clear remote avatars.
                            state.game_joined = false;
                            let remotes: Vec<hecs::Entity> = state
                                .game_world
                                .world
                                .query::<&crate::net::sync::RemotePlayer>()
                                .iter()
                                .map(|(e, _)| e)
                                .collect();
                            for e in remotes {
                                let _ = state.game_world.world.despawn(e);
                            }
                        }
                    }

                    // Periodic auto-save of the offline home (v0.381). Self-throttles
                    // to every 2 minutes; robust to any exit path (in-app quit, crash)
                    // where the graceful close-save would not fire.
                    crate::save_load::maybe_periodic_save(
                        &state.game_world.world,
                        &state.gui_state.placed_items,
                        120,
                    );

                    // ── Character-select showroom sync (v0.441): apply the panel's edits ──
                    // Rebuild the avatar when appearance changed (it is the tail of
                    // placeholder_objects, so truncate to its start + re-place).
                    if state.gui_state.appearance_dirty || state.gui_state.outfit_dirty {
                        state.gui_state.appearance_dirty = false;
                        state.gui_state.outfit_dirty = false;
                        state.placeholder_objects.truncate(state.avatar_obj_start);
                        let base = state.avatar_base;
                        let app = state.gui_state.appearance.clone();
                        let colors = crate::cosmetics::resolve_outfit_colors(
                            &state.gui_state.outfit,
                            &state.cosmetics,
                        );
                        place_avatar(state, base, &app, &colors);
                        state.camera.orbit_target =
                            base + Vec3::new(0.0, 0.9 * app.height_scale, 0.0);
                    }
                    // Rebuild the ground material when the backdrop changed.
                    if state.gui_state.showroom_active
                        && state.gui_state.showroom_backdrop != state.showroom_last_backdrop
                    {
                        state.showroom_last_backdrop = state.gui_state.showroom_backdrop;
                        if let Some(bd) =
                            state.showroom_backdrops.get(state.gui_state.showroom_backdrop)
                        {
                            let gmat = state.renderer.add_material_typed(
                                [bd.ground.0, bd.ground.1, bd.ground.2, 1.0],
                                0.1,
                                0.9,
                                0.0,
                            );
                            if let Some((gm, _)) = state.showroom_ground {
                                state.showroom_ground = Some((gm, gmat));
                            }
                        }
                    }
                    // Astral-projection camera (v0.464): when the construction editor opens, lift
                    // into a free ORBIT camera around the home (drag to orbit, middle-drag to
                    // pan, wheel to dolly, WASD to fly the focal point, Space/Shift up/down for
                    // levels). Reconciled here (not in the B handler) so the panel's Close button
                    // restores first person too.
                    if state.gui_state.construction_active && !state.construction_cam_active {
                        state.construction_cam_active = true;
                        state.controller.showroom_lock = false; // full orbit controls, not the fixed showroom orbit
                        state.construction_return_pos = state.camera.position;
                        // Seed the build-mode avatar at the player's CURRENT spot the first time
                        // (v0.557), clamped into the box, so toggling build mode without moving it keeps
                        // you put; it then persists where you leave it and spawns you there on close.
                        if state.gui_state.build_char_pos.is_none() {
                            if let Some(hs) = &state.gui_state.home_structure {
                                let p = state.construction_return_pos;
                                state.gui_state.build_char_pos =
                                    Some((p.x.clamp(0.3, hs.width - 0.3), p.z.clamp(0.3, hs.depth - 0.3)));
                            }
                        }
                        let (_center, size) = state.homestead_bounds
                            .map(|(mn, mx)| ((mn + mx) * 0.5, (mx - mn).length()))
                            .unwrap_or((Vec3::new(0.0, 1.5, 0.0), 20.0));
                        state.camera.switch_mode(crate::renderer::camera::CameraMode::Orbit);
                        // Focus the build cam on WHERE THE PLAYER IS (v0.582, operator) -- the player's
                        // spot when B was pressed, at roughly chest height -- so you start editing near
                        // yourself, not the home centre. Start zoomed in close; you can dolly out to the
                        // whole home (distance_max scales with the box).
                        let focus = state.construction_return_pos;
                        state.camera.orbit_target = Vec3::new(focus.x, 1.2, focus.z);
                        state.camera.orbit_distance = 12.0;
                        state.camera.orbit_distance_max = (size * 4.0).max(400.0);
                    } else if !state.gui_state.construction_active && state.construction_cam_active {
                        state.construction_cam_active = false;
                        state.camera.switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
                        // Spawn at the build-mode avatar (v0.557) -- "where I'm at" when I leave build
                        // mode; fall back to the pre-build position if no avatar was placed.
                        state.camera.position = match state.gui_state.build_char_pos {
                            Some((x, z)) => Vec3::new(x, 1.7, z),
                            None => state.construction_return_pos,
                        };
                        state.gui_state.construction_selected_room = None;
                        state.construction_grab = None;
                        state.construction_gizmo_grab = None;
                        state.construction_node_grab = None; // v0.542: drop a live corner grab on close
                        state.construction_opening_grab = None; // v0.546: drop a live opening grab
                        state.construction_opening_resize = None; // v0.578: drop a live resize handle
                        state.construction_char_grab = false; // v0.557: drop a live avatar grab
                        state.construction_grab_press = None;
                    }
                    // 3D room drag (v0.466): a grabbed room follows the cursor on its floor.
                    // Slide-gizmo drag (v0.468) takes precedence: a grabbed door/window handle
                    // slides along its wall; a grabbed room follows the cursor on its floor.
                    // Gated on the editor being open (v0.542): a grab MUST NOT keep dragging in
                    // first-person -- a Close click consumes the mouse-release, so without this gate a
                    // live node grab silently rewrote walls to chase the cursor after leaving the editor.
                    if state.gui_state.construction_active {
                        if state.construction_char_grab {
                            apply_char_drag(state); // v0.557: a grabbed avatar follows the cursor floor
                        } else if state.construction_opening_resize.is_some() {
                            apply_opening_resize(state); // v0.578: a grabbed resize handle resizes the opening
                        } else if state.construction_opening_grab.is_some() {
                            apply_opening_drag(state); // v0.546: a grabbed opening slides along its wall
                        } else if state.construction_node_grab.is_some() {
                            apply_node_drag(state); // v0.541: a grabbed wall corner follows the cursor
                        } else if state.construction_gizmo_grab.is_some() {
                            apply_gizmo_drag(state);
                        } else {
                            apply_room_drag(state);
                        }
                        // Track the cursor's floor position for the dimension overlay (v0.545). While
                        // DRAWING a wall, snap the preview to the same grid/endpoint/edge the placed node
                        // will use, so the selector visibly snaps (v0.559 -- it tracked the raw cursor).
                        let hit = cursor_floor_hit(state).map(|(_, hx, hz)| (hx, hz));
                        state.gui_state.construction_cursor_world = hit.map(|(hx, hz)| {
                            if state.gui_state.construction_wall_mode {
                                if let Some(hs) = &state.gui_state.home_structure {
                                    let grabbed =
                                        state.gui_state.construction_wall_start.unwrap_or((f32::NAN, f32::NAN));
                                    return snap_node_position(hs, grabbed, (hx, hz), state.gui_state.construction_grid_snap);
                                }
                            }
                            (hx, hz)
                        });
                    }
                    // Construction editor (v0.455/459): apply the edited walls + ceiling height
                    // AND room position/size + add/remove to the live layout, then rebuild.
                    // Reconcile by ID (both directions) so add/remove/reorder can't desync.
                    if state.gui_state.construction_dirty {
                        state.gui_state.construction_dirty = false;
                        let rooms = state.gui_state.construction_rooms.clone();
                        let height = state.gui_state.construction_height;
                        if let Some(layout) = &mut state.homestead_layout {
                            // "Pin" seeds Some([0,0,0]); turn that into the room's CURRENT
                            // resolved (computed) position so pinning freezes-in-place rather
                            // than teleporting to the origin.
                            let resolved = crate::ship::fibonacci::resolve_positions(layout);
                            let resolved_by_id: std::collections::HashMap<String, Vec3> = layout
                                .rooms
                                .iter()
                                .enumerate()
                                .map(|(i, rc)| (rc.id.clone(), resolved[i]))
                                .collect();

                            // 1. Drop layout rooms the editor removed.
                            let keep: std::collections::HashSet<&str> =
                                rooms.iter().map(|r| r.id.as_str()).collect();
                            layout.rooms.retain(|rc| keep.contains(rc.id.as_str()));

                            // 2. Upsert each editor room (patch by id, else append a new one).
                            for er in &rooms {
                                let mut pos = er.position;
                                if pos == Some([0.0, 0.0, 0.0]) {
                                    // freshly pinned: snap to where it visually sits now.
                                    if let Some(r) = resolved_by_id.get(&er.id) {
                                        pos = Some([r.x, r.y, r.z]);
                                    }
                                }
                                // Map the editor's placed openings to engine Openings (v0.469).
                                let er_openings: Vec<crate::ship::fibonacci::Opening> = er.openings.iter().map(|eo| {
                                    use crate::gui::EditorOpeningKind as EK;
                                    use crate::ship::fibonacci::OpeningKind as OK;
                                    crate::ship::fibonacci::Opening {
                                        wall: eo.wall as u8,
                                        kind: match eo.kind { EK::Door => OK::Door, EK::Airlock => OK::Airlock, EK::Window => OK::Window },
                                        u: eo.u, v: eo.v, w: eo.w, h: eo.h, profile: None,
                                    }
                                }).collect();
                                if let Some(rc) = layout.rooms.iter_mut().find(|r| r.id == er.id) {
                                    rc.walls.north = er.walls[0];
                                    rc.walls.south = er.walls[1];
                                    rc.walls.west = er.walls[2];
                                    rc.walls.east = er.walls[3];
                                    rc.walls.offsets = er.wall_offsets;
                                    rc.openings = er_openings;
                                    rc.level = er.level;
                                    rc.position = pos;
                                    rc.dimensions = er.dimensions;
                                } else {
                                    layout.rooms.push(crate::ship::fibonacci::RoomConfig {
                                        id: er.id.clone(),
                                        position: pos,
                                        dimensions: er.dimensions,
                                        material_type: er.material_type,
                                        color: er.color,
                                        wall_height: 0.0, // global default_wall_height owns Y
                                        walls: crate::ship::fibonacci::WallSet {
                                            north: er.walls[0],
                                            south: er.walls[1],
                                            west: er.walls[2],
                                            east: er.walls[3],
                                            offsets: er.wall_offsets,
                                        },
                                        openings: er_openings,
                                        level: er.level,
                                    });
                                }
                            }

                            // 3. Reorder layout.rooms to the editor order (stable spiral attach).
                            let order: std::collections::HashMap<&str, usize> =
                                rooms.iter().enumerate().map(|(i, r)| (r.id.as_str(), i)).collect();
                            layout.rooms.sort_by_key(|rc| *order.get(rc.id.as_str()).unwrap_or(&usize::MAX));

                            layout.default_wall_height = height;
                        }
                        // Reflect any pin-seeded positions back into the editor mirror so the
                        // DragValues show the real coordinates next frame.
                        if let Some(layout) = &state.homestead_layout {
                            for er in state.gui_state.construction_rooms.iter_mut() {
                                if er.position == Some([0.0, 0.0, 0.0]) {
                                    if let Some(rc) = layout.rooms.iter().find(|r| r.id == er.id) {
                                        er.position = rc.position;
                                    }
                                }
                            }
                        }
                        rebuild_homestead(state);
                    }
                    // Undo-history tick (v0.575): checkpoint at the dirty-flag choke point, BEFORE the
                    // rebuild blocks below consume the flags. Coalesces a drag into one undo step.
                    {
                        let edited = state.gui_state.construction_structure_dirty
                            || state.gui_state.construction_machines_dirty;
                        construction_history_tick(state, edited);
                    }
                    // Machine-only edit (offset / add / remove / connect): refresh just the machine
                    // meshes so the change shows live, without a full room rebuild. (v0.525)
                    if state.gui_state.construction_machines_dirty {
                        state.gui_state.construction_machines_dirty = false;
                        rebuild_machine_objects(state);
                    }
                    // Interior-wall edit (v0.534): the editor mutated gui_state.home_structure
                    // (added/removed a wall, moved a corner, changed an opening). Rebuild the home
                    // mesh live so the change shows immediately; persistence waits for Save.
                    if state.gui_state.construction_structure_dirty {
                        state.gui_state.construction_structure_dirty = false;
                        rebuild_homestead(state);
                    }
                    if state.gui_state.construction_save {
                        state.gui_state.construction_save = false;
                        // v0.534: the home is a HomeStructure when present -> save it; else the
                        // legacy AABB layout. One file per model; the AI + editor share it.
                        if state.gui_state.home_structure.is_some() {
                            // Persist the build-mode SPAWN point with the home (v0.582) so the moved
                            // avatar survives the save (was lost -- spawn lived only in GuiState).
                            let spawn = state.gui_state.build_char_pos;
                            if let Some(hs) = state.gui_state.home_structure.as_mut() {
                                hs.spawn = spawn;
                            }
                            let path = state.data_dir.join("blueprints").join("home_structure.ron");
                            let hs = state.gui_state.home_structure.as_ref().unwrap();
                            match hs.save(&path) {
                                Ok(()) => log::info!("Construction: home structure saved to home_structure.ron"),
                                Err(e) => log::warn!("Construction: home structure save failed: {e}"),
                            }
                        } else if let Some(layout) = &state.homestead_layout {
                            match crate::ship::fibonacci::save_layout(layout) {
                                Ok(()) => log::info!("Construction: layout saved to RON"),
                                Err(e) => log::warn!("Construction: save failed: {e}"),
                            }
                        }
                    }
                    // Machine layout save (v0.519): the editor's machine panel edits
                    // gui_state.home_machines; this writes it back to home.ron (the same
                    // file the AI edits -- home-design parity).
                    if state.gui_state.home_machines_save {
                        state.gui_state.home_machines_save = false;
                        if let Some(home) = &state.gui_state.home_machines {
                            let path = state.data_dir.join("machines").join("home.ron");
                            match home.save(&path) {
                                Ok(()) => log::info!("Construction: machine layout saved to home.ron"),
                                Err(e) => log::warn!("Construction: machine save failed: {e}"),
                            }
                        }
                    }
                    // Picker "Back": cancel the showroom and return to the menu WITHOUT
                    // entering the world (same as Esc in the picker). Mirrors the Esc
                    // handler so the visible Back button and the Esc key agree. (v0.476.1)
                    if state.gui_state.showroom_cancel {
                        state.gui_state.showroom_cancel = false;
                        let was_picker = state.gui_state.showroom_mode == 0;
                        state.gui_state.showroom_active = false;
                        state.gui_state.showroom_confirm = false;
                        state.controller.showroom_lock = false;
                        state.camera.switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
                        state.camera.position = state.showroom_return_pos;
                        if was_picker {
                            state.gui_state.active_page = state.gui_state.last_page;
                        }
                    }
                    // "Enter your home": persist appearance + emerge into first-person.
                    if state.gui_state.showroom_confirm {
                        state.gui_state.showroom_confirm = false;
                        state.gui_state.showroom_active = false;
                        let app = state.gui_state.appearance.clone();
                        let outfit = state.gui_state.outfit.clone();
                        let cname = if state.gui_state.character_name.trim().is_empty() {
                            "Wanderer".to_string()
                        } else {
                            state.gui_state.character_name.clone()
                        };
                        for (_e, (n, a, o, _c)) in state.game_world.world.query_mut::<(
                            &mut crate::ecs::components::Name,
                            &mut crate::ecs::components::Appearance,
                            &mut crate::ecs::components::Outfit,
                            &Controllable,
                        )>() {
                            n.0 = cname.clone();
                            *a = app.clone();
                            *o = outfit.clone();
                            break;
                        }
                        crate::save_load::save_active_home(&state.game_world.world, &state.gui_state.placed_items);
                        state.controller.showroom_lock = false;
                        state
                            .camera
                            .switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
                        state.camera.position = state.showroom_return_pos;
                        // (cursor re-grabbed by the per-frame reconciliation below)
                    }

                    // Cursor: free for any menu page / showroom / construction editor (so egui
                    // clicks land), grabbed for first-person play. Single authority (v0.460).
                    reconcile_cursor(state);

                    // Build render objects from homestead meshes
                    let mut all_objects: Vec<RenderObject> = Vec::new();
                    // Transparent surfaces (glass windows): rendered in a SEPARATE alpha-blend
                    // pass AFTER the opaque scene so you can see through them. (v0.456)
                    let mut transparent_objects: Vec<RenderObject> = Vec::new();
                    // Editor GIZMOS (corner orbs, the avatar, its pyramid): rendered LAST with depth
                    // test off, so they show THROUGH walls + floors in build mode. (v0.560)
                    let mut overlay_objects: Vec<RenderObject> = Vec::new();
                    // Celestial bodies (planet + Sun + solar bodies): rendered in a SEPARATE
                    // pass with a huge far plane (v0.450), since they sit at astronomical
                    // distances the ~500 m gameplay far would clip.
                    let mut celestial_objects: Vec<RenderObject> = Vec::new();
                    // World-space orbit lines, built this frame, drawn
                    // after the scene so they depth-occlude behind planets.
                    let mut orbit_lines: Vec<crate::renderer::line::LineVertex> = Vec::new();
                    // Build-mode door auto-open RINGS as constant-width line circles (v0.565).
                    let mut ring_lines: Vec<crate::renderer::line::LineVertex> = Vec::new();

                    // During the character-select showroom the home is HIDDEN so the avatar
                    // floats against the backdrop; otherwise draw the full home.
                    let showroom = state.gui_state.showroom_active;
                    // Homestead at origin — vertex positions are in ship-local coords.
                    if !showroom {
                        for &(mesh_idx, mat_idx) in &state.homestead_floors {
                            all_objects.push(RenderObject {
                                position: Vec3::ZERO,
                                rotation: Quat::IDENTITY,
                                scale: Vec3::ONE,
                                mesh: mesh_idx,
                                material: mat_idx,
                            });
                        }
                        // Selected-room highlight (v0.466): a translucent accent quad over the
                        // selected room's floor, drawn in the transparent pass so the alpha
                        // shows. Built once + cached; scaled per frame; survives rebuilds.
                        if let Some(ri) = state.gui_state.construction_selected_room {
                            if state.construction_hilite.is_none() {
                                let v = vec![
                                    crate::renderer::mesh::Vertex { position: [-0.5, 0.0, -0.5], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] },
                                    crate::renderer::mesh::Vertex { position: [0.5, 0.0, -0.5], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0] },
                                    crate::renderer::mesh::Vertex { position: [0.5, 0.0, 0.5], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0] },
                                    crate::renderer::mesh::Vertex { position: [-0.5, 0.0, 0.5], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0] },
                                ];
                                let idx = vec![0u32, 2, 1, 0, 3, 2];
                                let m = state.renderer.add_mesh(Mesh::from_vertices(&state.renderer.device, &v, &idx));
                                let a = state.theme.accent();
                                let mat = state.renderer.add_material_full(
                                    [a.r() as f32 / 255.0, a.g() as f32 / 255.0, a.b() as f32 / 255.0, 0.4],
                                    0.0, 0.4, 1.0, 0.3,
                                );
                                state.construction_hilite = Some((m, mat));
                            }
                            if let Some((hm, hmat)) = state.construction_hilite {
                                if let Some(id) = state.gui_state.construction_rooms.get(ri).map(|r| r.id.clone()) {
                                    if let Some(rb) = state.gui_state.room_bounds.iter().find(|b| b.id == id) {
                                        let center = (rb.min + rb.max) * 0.5;
                                        let size = rb.max - rb.min;
                                        transparent_objects.push(RenderObject {
                                            position: Vec3::new(center.x, rb.min.y + 0.03, center.z),
                                            rotation: Quat::IDENTITY,
                                            scale: Vec3::new(size.x, 1.0, size.z),
                                            mesh: hm,
                                            material: hmat,
                                        });
                                    }
                                }
                            }
                            // Opening gizmo handles (v0.468, resize v0.469): a glowing accent MOVE
                            // cube at each opening's centre, plus warning-tinted RESIZE cubes at the
                            // edges of placed (non-floor) openings. Handles float proud of the wall
                            // toward the camera so they never z-fight the glass. Cached cube mesh.
                            let gizmo_handles = selected_room_handles(state);
                            if !gizmo_handles.is_empty() {
                                if state.construction_gizmo_handle.is_none() {
                                    // Unit cube centred at origin; normals = outward corner dirs
                                    // (good enough -- the marker is emissive, so it glows flatly).
                                    let c = |x: f32, y: f32, z: f32| {
                                        let n = Vec3::new(x, y, z).normalize_or_zero();
                                        crate::renderer::mesh::Vertex {
                                            position: [x, y, z],
                                            normal: [n.x, n.y, n.z],
                                            uv: [0.0, 0.0],
                                        }
                                    };
                                    let v = vec![
                                        c(-0.5, -0.5, -0.5), c(0.5, -0.5, -0.5), c(0.5, 0.5, -0.5), c(-0.5, 0.5, -0.5),
                                        c(-0.5, -0.5, 0.5), c(0.5, -0.5, 0.5), c(0.5, 0.5, 0.5), c(-0.5, 0.5, 0.5),
                                    ];
                                    let idx = vec![
                                        4, 5, 6, 4, 6, 7, // +Z
                                        1, 0, 3, 1, 3, 2, // -Z
                                        5, 1, 2, 5, 2, 6, // +X
                                        0, 4, 7, 0, 7, 3, // -X
                                        3, 7, 6, 3, 6, 2, // +Y
                                        0, 1, 5, 0, 5, 4, // -Y
                                    ];
                                    let m = state.renderer.add_mesh(Mesh::from_vertices(&state.renderer.device, &v, &idx));
                                    let a = state.theme.accent();
                                    let mat = state.renderer.add_material_full(
                                        [a.r() as f32 / 255.0, a.g() as f32 / 255.0, a.b() as f32 / 255.0, 0.95],
                                        0.0, 0.3, 1.0, 0.9, // emissive so it pops against the wall
                                    );
                                    state.construction_gizmo_handle = Some((m, mat));
                                }
                                // Resize-handle material (warning tint), sharing the move cube mesh.
                                if state.construction_gizmo_resize_handle.is_none() {
                                    if let Some((gm, _)) = state.construction_gizmo_handle {
                                        let wcol = state.theme.warning();
                                        let wmat = state.renderer.add_material_full(
                                            [wcol.r() as f32 / 255.0, wcol.g() as f32 / 255.0, wcol.b() as f32 / 255.0, 0.95],
                                            0.0, 0.3, 1.0, 0.9,
                                        );
                                        state.construction_gizmo_resize_handle = Some((gm, wmat));
                                    }
                                }
                                let cam = state.camera.position;
                                if let Some((gm, gmat)) = state.construction_gizmo_handle {
                                    for (_ri, h) in &gizmo_handles {
                                        // Nudge proud of the wall toward the camera (anti z-fight).
                                        let s = (cam - h.base_center).dot(h.n);
                                        let face = h.n * (if s >= 0.0 { 1.0 } else { -1.0 }) * 0.06;
                                        transparent_objects.push(RenderObject {
                                            position: h.base_center + face,
                                            rotation: Quat::IDENTITY,
                                            scale: Vec3::splat(0.22),
                                            mesh: gm,
                                            material: gmat,
                                        });
                                        // Resize handles for placed openings (width for all; height
                                        // too for non-floor-snapped kinds). Doors show move-only.
                                        if h.opening_index.is_some() {
                                            if let Some((rm, rmat)) = state.construction_gizmo_resize_handle {
                                                let mut spots = vec![h.handle_left, h.handle_right];
                                                if !h.kind.floor_snapped() {
                                                    spots.push(h.handle_bottom);
                                                    spots.push(h.handle_top);
                                                }
                                                for p in spots {
                                                    transparent_objects.push(RenderObject {
                                                        position: p + face,
                                                        rotation: Quat::IDENTITY,
                                                        scale: Vec3::splat(0.14),
                                                        mesh: rm,
                                                        material: rmat,
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // Walls, trim, windows, mirror/portal — all part of the home shell.
                        // The roof (ceiling) is gated by the show_roof toggle so the sky stays
                        // visible by default. (v0.453)
                        // Opaque shell (walls, trim, the emissive portal, optional roof).
                        // Windows are TRANSPARENT, so they go in their own list (below).
                        let mut shell = vec![
                            state.homestead_walls,
                            state.homestead_trim,
                            state.homestead_mirrors,
                        ];
                        // Opaque roof: gated on show_roof (off by default so the sky shows). A GLASS
                        // roof (v0.539) is a sealed CLEAR ceiling -- always visible, drawn transparent
                        // below, so you see the stars through it without hiding the seal.
                        if state.gui_state.show_roof && !state.homestead_ceiling_glass {
                            shell.push(state.homestead_ceiling);
                        }
                        for (mesh_idx, mat_idx) in shell.into_iter().flatten() {
                            all_objects.push(RenderObject {
                                position: Vec3::ZERO,
                                rotation: Quat::IDENTITY,
                                scale: Vec3::ONE,
                                mesh: mesh_idx,
                                material: mat_idx,
                            });
                        }
                        // Per-material home walls (v0.552): opaque materials in the main pass, glass
                        // (transparent) in the transparent pass so it blends behind/through correctly.
                        for &(mesh_idx, mat_idx, transparent) in &state.homestead_material_walls {
                            let obj = RenderObject {
                                position: Vec3::ZERO,
                                rotation: Quat::IDENTITY,
                                scale: Vec3::ONE,
                                mesh: mesh_idx,
                                material: mat_idx,
                            };
                            if transparent {
                                transparent_objects.push(obj);
                            } else {
                                all_objects.push(obj);
                            }
                        }
                        // Glass windows + a glass roof -> the transparent pass.
                        if let Some((mesh_idx, mat_idx)) = state.homestead_windows {
                            transparent_objects.push(RenderObject {
                                position: Vec3::ZERO,
                                rotation: Quat::IDENTITY,
                                scale: Vec3::ONE,
                                mesh: mesh_idx,
                                material: mat_idx,
                            });
                        }
                        // The glass roof is hidden in BUILD MODE (v0.540) so it never obscures the
                        // top-down editor view; it shows in first-person (a sealed clear roof).
                        if state.homestead_ceiling_glass && !state.gui_state.construction_active {
                            if let Some((mesh_idx, mat_idx)) = state.homestead_ceiling {
                                transparent_objects.push(RenderObject {
                                    position: Vec3::ZERO,
                                    rotation: Quat::IDENTITY,
                                    scale: Vec3::ONE,
                                    mesh: mesh_idx,
                                    material: mat_idx,
                                });
                            }
                        }
                    }

                    // Placeholder objects: the whole home normally; ONLY the avatar (its
                    // parts are the tail, from avatar_obj_start) during the showroom.
                    let pstart = if showroom { state.avatar_obj_start } else { 0 };
                    for &(mesh_idx, mat_idx, pos) in
                        state.placeholder_objects.get(pstart..).unwrap_or(&[])
                    {
                        all_objects.push(RenderObject {
                            position: pos,
                            rotation: Quat::IDENTITY,
                            scale: Vec3::ONE,
                            mesh: mesh_idx,
                            material: mat_idx,
                        });
                    }
                    // Home machines: a separate list so the construction editor can rebuild just
                    // them on an edit. Hidden in the showroom (avatar-only). (v0.525)
                    if !showroom {
                        for &(mesh_idx, mat_idx, pos) in &state.machine_objects {
                            all_objects.push(RenderObject {
                                position: pos,
                                rotation: Quat::IDENTITY,
                                scale: Vec3::ONE,
                                mesh: mesh_idx,
                                material: mat_idx,
                            });
                        }
                        // Connection cylinders (live, colored by kind; follow rooms). (v0.530)
                        for &(mesh_idx, mat_idx, pos, rot, scale) in &state.connection_objects {
                            all_objects.push(RenderObject {
                                position: pos,
                                rotation: rot,
                                scale,
                                mesh: mesh_idx,
                                material: mat_idx,
                            });
                        }
                        // Door + window panels: doors ease open as the player nears (by style);
                        // windows are fixed glass (transparent pass). (v0.537)
                        render_door_panels(state, &mut all_objects, &mut transparent_objects, &mut ring_lines, dt);
                    }
                    // Placement ghost: the held palette item, previewed (semi-transparent, faintly
                    // glowing) on the room floor under the cursor, so you see where a click drops it.
                    // The ghost mesh is cached + rebuilt only when the held type changes. (v0.529)
                    if state.gui_state.construction_active {
                        if let Some(mtype) = state.gui_state.construction_place_type.clone() {
                            let need = state
                                .construction_ghost
                                .as_ref()
                                .map_or(true, |(t, _, _)| t != &mtype);
                            if need {
                                let def = state
                                    .gui_state
                                    .home_machines
                                    .as_ref()
                                    .and_then(|h| h.catalog.get(&mtype))
                                    .cloned();
                                if let Some(def) = def {
                                    let mesh =
                                        machine_mesh(&state.renderer.device, &def.shape, def.size);
                                    let color = [def.color.0, def.color.1, def.color.2, 0.45];
                                    // Reuse the prior ghost slot so switching held type does not
                                    // leak a mesh+material each time. (v0.531)
                                    let slot =
                                        state.construction_ghost.as_ref().map(|(_, m, mt)| (*m, *mt));
                                    let (mesh_idx, mat) = if let Some((mi, ma)) = slot {
                                        state.renderer.replace_mesh(mi, mesh);
                                        state.renderer.update_material_typed(ma, color, 0.1, 0.6, 0.4);
                                        (mi, ma)
                                    } else {
                                        let mi = state.renderer.add_mesh(mesh);
                                        let ma = state.renderer.add_material_typed(color, 0.1, 0.6, 0.4);
                                        (mi, ma)
                                    };
                                    state.construction_ghost = Some((mtype.clone(), mesh_idx, mat));
                                }
                            }
                            let ghost = state.construction_ghost.as_ref().map(|(_, m, mt)| (*m, *mt));
                            if let Some((mesh_idx, mat)) = ghost {
                                if let Some((rb_i, hx, hz)) = cursor_floor_hit(state) {
                                    let floor_y = state.gui_state.room_bounds[rb_i].min.y;
                                    transparent_objects.push(RenderObject {
                                        position: Vec3::new(hx, floor_y, hz),
                                        rotation: Quat::IDENTITY,
                                        scale: Vec3::ONE,
                                        mesh: mesh_idx,
                                        material: mat,
                                    });
                                }
                            }
                        } else {
                            state.construction_ghost = None;
                        }
                        // STRUCTURE placement ghost (v0.583): the held structural piece, previewed
                        // translucent on the floor under the cursor with the current placement yaw.
                        // The key encodes type + yaw so rotating ([ / ]) rebuilds the cached mesh.
                        if let Some(tid) = state.gui_state.construction_structure_type.clone() {
                            let yaw = state.gui_state.construction_structure_yaw;
                            let key = format!("{tid}@{:.0}", yaw);
                            let need = state
                                .construction_structure_ghost
                                .as_ref()
                                .map_or(true, |(k, _, _)| k != &key);
                            if need {
                                if let Some(ty) = crate::ship::structure::structure_type(&tid) {
                                    let (verts, indices) =
                                        crate::ship::structure::structure_mesh(ty, Vec3::ZERO, yaw.to_radians());
                                    let mesh = Mesh::from_vertices(&state.renderer.device, &verts, &indices);
                                    let color = [ty.color.0, ty.color.1, ty.color.2, 0.45];
                                    let slot = state
                                        .construction_structure_ghost
                                        .as_ref()
                                        .map(|(_, m, mt)| (*m, *mt));
                                    let (mesh_idx, mat) = if let Some((mi, ma)) = slot {
                                        state.renderer.replace_mesh(mi, mesh);
                                        state.renderer.update_material_typed(ma, color, 0.1, 0.6, 0.4);
                                        (mi, ma)
                                    } else {
                                        let mi = state.renderer.add_mesh(mesh);
                                        let ma = state.renderer.add_material_typed(color, 0.1, 0.6, 0.4);
                                        (mi, ma)
                                    };
                                    state.construction_structure_ghost = Some((key.clone(), mesh_idx, mat));
                                }
                            }
                            let ghost = state.construction_structure_ghost.as_ref().map(|(_, m, mt)| (*m, *mt));
                            if let Some((mesh_idx, mat)) = ghost {
                                if let Some((rb_i, hx, hz)) = cursor_floor_hit(state) {
                                    // Show the ghost at the place-height (v0.588) so you see where a deck
                                    // lands at an upper level; a faint riser line marks the lift.
                                    let floor_y = state.gui_state.room_bounds[rb_i].min.y;
                                    let py = floor_y + state.gui_state.construction_structure_place_y.max(0.0);
                                    if py > floor_y + 0.01 {
                                        crate::renderer::line::push_polyline(
                                            &mut ring_lines,
                                            &[[hx, floor_y, hz], [hx, py, hz]],
                                            [0.45, 0.85, 1.0, 0.6],
                                        );
                                    }
                                    transparent_objects.push(RenderObject {
                                        position: Vec3::new(hx, py, hz),
                                        rotation: Quat::IDENTITY,
                                        scale: Vec3::ONE,
                                        mesh: mesh_idx,
                                        material: mat,
                                    });
                                }
                            }
                        } else {
                            state.construction_structure_ghost = None;
                        }
                    }

                    // Wall-drawing tool preview (v0.534): a corner-node marker under the cursor and,
                    // once the first corner is set, a translucent preview wall from that corner to the
                    // cursor -- so you see the segment before clicking the second corner. Uses one
                    // cached unit-box mesh, scaled/rotated per frame (no per-frame allocation).
                    if state.gui_state.construction_active && state.gui_state.construction_wall_mode {
                        if state.wall_tool_mesh.is_none() {
                            let m = state.renderer.add_mesh(Mesh::box_xyz(&state.renderer.device, 1.0, 1.0, 1.0));
                            state.wall_tool_mesh = Some(m);
                        }
                        if state.wall_tool_mat.is_none() {
                            // theme-exempt: translucent editor overlay, not a themed surface.
                            let ma = state.renderer.add_material_typed([0.25, 0.8, 1.0, 0.5], 0.1, 0.6, 0.4);
                            state.wall_tool_mat = Some(ma);
                        }
                        let mesh = state.wall_tool_mesh.unwrap();
                        let mat = state.wall_tool_mat.unwrap();
                        if let Some((rb_i, hx, hz)) = cursor_floor_hit(state) {
                            let floor_y = state.gui_state.room_bounds[rb_i].min.y;
                            // Corner-node marker: a slim post where the next click lands. (box_xyz is
                            // y-bottom-origin, so position at floor_y -> the post spans [floor, +3].)
                            transparent_objects.push(RenderObject {
                                position: Vec3::new(hx, floor_y, hz),
                                rotation: Quat::IDENTITY,
                                scale: Vec3::new(0.3, 3.0, 0.3),
                                mesh,
                                material: mat,
                            });
                            // Preview wall from the pending start corner to the cursor.
                            if let Some((sx, sz)) = state.gui_state.construction_wall_start {
                                let a = Vec3::new(sx, floor_y, sz);
                                let b = Vec3::new(hx, floor_y, hz);
                                let dx = b.x - a.x;
                                let dz = b.z - a.z;
                                let len = (dx * dx + dz * dz).sqrt();
                                if len > 0.05 {
                                    let height = state
                                        .gui_state
                                        .home_structure
                                        .as_ref()
                                        .map_or(3.0, |h| h.height);
                                    let dir = Vec3::new(dx, 0.0, dz) / len;
                                    let rot = Quat::from_rotation_arc(Vec3::X, dir);
                                    // box_xyz is y-bottom-origin -> position at floor_y so the preview
                                    // wall fills [floor, floor+height].
                                    transparent_objects.push(RenderObject {
                                        position: Vec3::new((a.x + b.x) * 0.5, floor_y, (a.z + b.z) * 0.5),
                                        rotation: rot,
                                        scale: Vec3::new(len, height, 0.15),
                                        mesh,
                                        material: mat,
                                    });
                                }
                            }
                        }
                    }

                    // Which build-mode gizmo the cursor is hovering this frame (v0.569) -- drives the
                    // hover highlight on the corner orbs, the opening cubes, and the avatar pyramid.
                    let hover = compute_construction_hover(state);

                    // Corner-node gizmos (v0.541): a bright pin above each wall corner in BUILD MODE.
                    // Click + drag one to reposition the corner (walls sharing it move together) with
                    // snapping; idle -> hover -> active (grabbed) by colour. Cached sphere mesh + three
                    // materials, reused -- no per-frame allocation.
                    if state.gui_state.construction_active && state.gui_state.home_structure.is_some() {
                        if state.construction_node_mesh.is_none() {
                            let m = state.renderer.add_mesh(Mesh::sphere(&state.renderer.device, 1.0, 12, 16));
                            state.construction_node_mesh = Some(m);
                        }
                        if state.construction_node_mat.is_none() {
                            // theme-exempt: editor gizmo overlay, emissive so it stands out.
                            let m = state.renderer.add_material_full([1.0, 0.82, 0.2, 1.0], 0.0, 0.4, 0.0, 0.6);
                            state.construction_node_mat = Some(m);
                        }
                        if state.construction_node_mat_hot.is_none() {
                            // theme-exempt: grabbed-gizmo highlight.
                            let m = state.renderer.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.3, 0.0, 1.0);
                            state.construction_node_mat_hot = Some(m);
                        }
                        if state.construction_node_mat_hover.is_none() {
                            // theme-exempt: hover highlight -- brighter/whiter than idle, calmer than the
                            // active RGB cycle (idle yellow -> hover cream -> active RGB).
                            let m = state.renderer.add_material_full([1.0, 0.96, 0.62, 1.0], 0.0, 0.35, 0.0, 0.95);
                            state.construction_node_mat_hover = Some(m);
                        }
                        let node_mesh = state.construction_node_mesh.unwrap();
                        let node_mat = state.construction_node_mat.unwrap();
                        let hot_mat = state.construction_node_mat_hot.unwrap();
                        let hover_mat = state.construction_node_mat_hover.unwrap();
                        // RGB-cycle the ACTIVE (grabbed/selected) gizmo material like the menu header
                        // buttons (v0.562). Only HOT gizmos use this material, so static ones keep
                        // their colour. door_anim_time advances each build-mode frame (with doors).
                        let hue = (state.door_anim_time * 0.25).rem_euclid(1.0);
                        let (rr, gg, bb) = hsv_rgb(hue, 0.85, 1.0);
                        state.renderer.update_material_full(hot_mat, [rr, gg, bb, 1.0], 0.0, 0.3, 0.0, 1.2);
                        let grabbed = state.construction_node_grab;
                        let corners = {
                            let hs = state.gui_state.home_structure.as_ref().unwrap();
                            unique_corners(hs)
                        };
                        for c in &corners {
                            let hot = grabbed.map_or(false, |g| (g.0 - c.0).abs() < 0.05 && (g.1 - c.1).abs() < 0.05);
                            let hovered = hover == HoverGizmo::Corner(c.0, c.1);
                            let r = 0.05; // operator: orbs at 0.05 m; state shown by COLOUR (active = RGB), not size
                            // The orb's TOP touches the wall-corner BASE (operator note): centre at -r
                            // so the top vertex is at the floor. Overlay pass -> visible through walls
                            // + the floor it sits under. (v0.560). Idle -> hover -> active by colour (v0.569).
                            overlay_objects.push(RenderObject {
                                position: Vec3::new(c.0, -r, c.1),
                                rotation: Quat::IDENTITY,
                                scale: Vec3::splat(r),
                                mesh: node_mesh,
                                material: if hot { hot_mat } else if hovered { hover_mat } else { node_mat },
                            });
                            // Ground angle-circle (v0.568): a constant-width LINE circle (line::push_circle
                            // into ring_lines, like the orbit paths) instead of a polygon ring whose band
                            // thickened with radius. Radius 1.1 matches the overlay's RING_R; the per-slice
                            // angle labels are painted separately by the egui overlay.
                            crate::renderer::line::push_circle(
                                &mut ring_lines, [c.0, 0.1, c.1], 1.1, [0.30, 0.85, 1.0, 0.85], 48,
                            );
                        }
                        // Wall-SELECT orbs (v0.573): a RED sphere at each wall's bottom-middle. Click
                        // anywhere on a wall (its surface or this orb) to select it -- unambiguous at a
                        // multi-wall intersection. The SELECTED wall's orb uses the RGB hot material, so
                        // it shifts colour like the header menu buttons.
                        if state.construction_wall_mat.is_none() {
                            // theme-exempt: wall-select gizmo, red emissive.
                            let m = state.renderer.add_material_full([1.0, 0.2, 0.2, 1.0], 0.0, 0.4, 0.0, 0.8);
                            state.construction_wall_mat = Some(m);
                        }
                        let wall_mat = state.construction_wall_mat.unwrap();
                        let sel_wall = state.gui_state.construction_wall_selected;
                        let wall_mids: Vec<(usize, f32, f32)> = state
                            .gui_state
                            .home_structure
                            .as_ref()
                            .map(|h| {
                                h.walls
                                    .iter()
                                    .enumerate()
                                    .map(|(i, w)| (i, (w.a.0 + w.b.0) * 0.5, (w.a.1 + w.b.1) * 0.5))
                                    .collect()
                            })
                            .unwrap_or_default();
                        for (i, mx, mz) in &wall_mids {
                            let selected = sel_wall == Some(*i);
                            overlay_objects.push(RenderObject {
                                position: Vec3::new(*mx, -0.07, *mz), // orb top at the floor base
                                rotation: Quat::IDENTITY,
                                scale: Vec3::splat(0.07),
                                mesh: node_mesh,
                                material: if selected { hot_mat } else { wall_mat },
                            });
                        }
                        // Highlight the selected machine with a ground ring (v0.553) -> also a LINE circle
                        // now (v0.568), in the RGB active colour so it pulses like the header buttons.
                        if let Some(sel_id) = &state.gui_state.construction_machine_selected {
                            if let Some((_, center, radius)) =
                                state.machine_pick.iter().find(|(mid, _, _)| mid == sel_id)
                            {
                                crate::renderer::line::push_circle(
                                    &mut ring_lines, [center.x, 0.12, center.z], radius + 0.2, [rr, gg, bb, 1.0], 48,
                                );
                            }
                        }
                    }

                    // Placed-LIGHT gizmos (v0.572): a DIAMOND centre-marker + an RGB range "sphere"
                    // (three axis great-circles -- X red, Y green, Z blue) at each placed light, so the
                    // operator (and an AI) can see where each light sits + how far it reaches. Build mode.
                    if state.gui_state.construction_active && state.gui_state.home_structure.is_some() {
                        if state.construction_light_mesh.is_none() {
                            let m = state.renderer.add_mesh(Mesh::octahedron(&state.renderer.device, 1.0));
                            state.construction_light_mesh = Some(m);
                        }
                        if state.construction_light_mat.is_none() {
                            // theme-exempt: light gizmo marker, emissive so it reads at a glance.
                            let m = state.renderer.add_material_full([1.0, 0.95, 0.6, 1.0], 0.0, 0.3, 0.0, 1.2);
                            state.construction_light_mat = Some(m);
                        }
                        let dmesh = state.construction_light_mesh.unwrap();
                        let dmat = state.construction_light_mat.unwrap();
                        // The selected light's diamond uses the RGB hot material (created by the corner
                        // block, which runs first under the same condition), so it shifts colour. (v0.576)
                        let hot_light = state.construction_node_mat_hot.unwrap();
                        let sel_light = state.gui_state.construction_light_selected;
                        // Resolve each light's range + (for a SPOT) its aim direction + cone half-angle,
                        // so a spotlight draws a CONE instead of the omni range sphere (v0.582, operator).
                        let lights: Vec<(usize, Vec3, f32, Option<(Vec3, f32)>)> = state
                            .gui_state
                            .home_structure
                            .as_ref()
                            .map(|h| {
                                h.lights
                                    .iter()
                                    .enumerate()
                                    .map(|(i, l)| {
                                        let t = crate::renderer::light::light_type(&l.type_id);
                                        let range = l.range.or_else(|| t.map(|t| t.range)).unwrap_or(4.0);
                                        let spot = t
                                            .filter(|t| t.kind == crate::renderer::light::LightKind::Spot)
                                            .map(|t| {
                                                let d = Vec3::new(l.dir.0, l.dir.1, l.dir.2).normalize_or_zero();
                                                let d = if d == Vec3::ZERO { Vec3::NEG_Y } else { d };
                                                (d, t.cone_outer_deg.max(1.0).to_radians())
                                            });
                                        (i, Vec3::new(l.pos.0, l.pos.1, l.pos.2), range, spot)
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        for (i, pos, range, spot) in &lights {
                            // Diamond centre marker (overlay -> visible through walls); RGB if selected.
                            overlay_objects.push(RenderObject {
                                position: *pos,
                                rotation: Quat::IDENTITY,
                                scale: Vec3::splat(if sel_light == Some(*i) { 0.16 } else { 0.12 }),
                                mesh: dmesh,
                                material: if sel_light == Some(*i) { hot_light } else { dmat },
                            });
                            let p = [pos.x, pos.y, pos.z];
                            if let Some((dir, half)) = spot {
                                // SPOT: a cone gizmo -- the base circle at `range` + edge lines from the
                                // apex, warm yellow, using the reusable line primitive.
                                let base_c = *pos + *dir * *range;
                                let base_r = (*range) * half.tan();
                                const COL: [f32; 4] = [1.0, 0.9, 0.4, 0.8];
                                crate::renderer::line::push_circle_3d(&mut ring_lines, base_c.into(), base_r, (*dir).into(), COL, 32);
                                let seed = if dir.x.abs() > 0.9 { Vec3::Y } else { Vec3::X };
                                let u = seed.cross(*dir).normalize();
                                let v = dir.cross(u);
                                for k in 0..8 {
                                    let a = (k as f32 / 8.0) * std::f32::consts::TAU;
                                    let edge = base_c + (u * a.cos() + v * a.sin()) * base_r;
                                    crate::renderer::line::push_polyline(&mut ring_lines, &[p, edge.into()], COL);
                                }
                            } else {
                                // POINT (etc.): the omni range "sphere" -- three axis great-circles R/G/B.
                                crate::renderer::line::push_circle_3d(&mut ring_lines, p, *range, [1.0, 0.0, 0.0], [1.0, 0.30, 0.30, 0.7], 40);
                                crate::renderer::line::push_circle_3d(&mut ring_lines, p, *range, [0.0, 1.0, 0.0], [0.35, 1.0, 0.35, 0.7], 40);
                                crate::renderer::line::push_circle_3d(&mut ring_lines, p, *range, [0.0, 0.0, 1.0], [0.45, 0.55, 1.0, 0.7], 40);
                            }
                        }
                    }

                    // HELPER GIZMOS (v0.583/586/587): bounds boxes on placed structures + machines, the
                    // road graph (node rings + edge centerlines), and conduit-node markers -- the helper
                    // widgets the operator asked for "on everything." All drawn with the reusable line
                    // primitive (shows through walls), gated by the master toggle so a busy view can quiet
                    // them. The interactive editing handles (corner orbs, resize cubes) are NOT gated.
                    if state.gui_state.construction_active && state.gui_state.construction_show_helpers {
                        if let Some(hs) = state.gui_state.home_structure.as_ref() {
                            let sel = state.gui_state.construction_structure_selected;
                            for (i, ps) in hs.structures.iter().enumerate() {
                                let Some(ty) = crate::ship::structure::structure_type(&ps.type_id) else { continue };
                                if ty.kind == crate::ship::structure::StructureKind::Wall {
                                    continue;
                                }
                                let (w, h, d) = ty.size;
                                let (hw, hd) = (w * 0.5, d * 0.5);
                                let yaw = ps.rot_deg.to_radians();
                                let (s, c) = yaw.sin_cos();
                                // The 4 footprint corners, yaw-rotated around the piece centre.
                                let corner = |lx: f32, lz: f32| {
                                    let rx = lx * c + lz * s;
                                    let rz = -lx * s + lz * c;
                                    (ps.pos.0 + rx, ps.pos.2 + rz)
                                };
                                let fc = [corner(-hw, -hd), corner(hw, -hd), corner(hw, hd), corner(-hw, hd)];
                                let (y0, y1) = (ps.pos.1, ps.pos.1 + h.max(0.1));
                                let col: [f32; 4] = if sel == Some(i) {
                                    [1.0, 0.85, 0.25, 0.95] // selected: bright amber
                                } else {
                                    [0.45, 0.85, 1.0, 0.6] // cyan, like other build gizmos
                                };
                                for k in 0..4 {
                                    let (x0, z0) = fc[k];
                                    let (x1, z1) = fc[(k + 1) % 4];
                                    // Bottom ring, top ring, and a vertical riser at each corner.
                                    crate::renderer::line::push_polyline(&mut ring_lines, &[[x0, y0, z0], [x1, y0, z1]], col);
                                    crate::renderer::line::push_polyline(&mut ring_lines, &[[x0, y1, z0], [x1, y1, z1]], col);
                                    crate::renderer::line::push_polyline(&mut ring_lines, &[[x0, y0, z0], [x0, y1, z0]], col);
                                }
                            }

                            // ROAD-GRAPH gizmo (v0.586): a ring marker at each node + a centerline along
                            // each edge, drawn with the line primitive so the graph reads at a glance in
                            // build mode (the ribbon mesh shows the carriageway; this shows the topology).
                            const RN: [f32; 4] = [1.0, 0.75, 0.2, 0.9]; // node ring (amber)
                            const RE: [f32; 4] = [0.5, 0.8, 1.0, 0.8]; // edge centerline (cyan)
                            for n in &hs.road_nodes {
                                crate::renderer::line::push_circle(&mut ring_lines, [n.pos.0, 0.06, n.pos.1], 0.4, RN, 20);
                            }
                            for e in &hs.road_edges {
                                if let (Some(a), Some(b)) = (hs.road_node_pos(e.from), hs.road_node_pos(e.to)) {
                                    crate::renderer::line::push_polyline(&mut ring_lines, &[[a.0, 0.08, a.1], [b.0, 0.08, b.1]], RE);
                                }
                            }

                            // MACHINE bounds gizmos (v0.587): a wireframe cube around each placed machine
                            // (from its pick volume centre+radius), so every machine has a helper widget
                            // like the structures. Trims the click margin so the cube ~ the body.
                            const MB: [f32; 4] = [0.55, 0.8, 0.95, 0.5];
                            for (_id, center, radius) in &state.machine_pick {
                                let r = (radius - 0.3).max(0.2);
                                let c = *center;
                                let fc = [(c.x - r, c.z - r), (c.x + r, c.z - r), (c.x + r, c.z + r), (c.x - r, c.z + r)];
                                let (y0, y1) = ((c.y - r).max(0.0), c.y + r);
                                for k in 0..4 {
                                    let (x0, z0) = fc[k];
                                    let (x1, z1) = fc[(k + 1) % 4];
                                    crate::renderer::line::push_polyline(&mut ring_lines, &[[x0, y0, z0], [x1, y0, z1]], MB);
                                    crate::renderer::line::push_polyline(&mut ring_lines, &[[x0, y1, z0], [x1, y1, z1]], MB);
                                    crate::renderer::line::push_polyline(&mut ring_lines, &[[x0, y0, z0], [x0, y1, z0]], MB);
                                }
                            }

                            // CONDUIT-NODE markers (v0.587): a small ring at each pipe-graph junction --
                            // the edges already render as solid pipes, this gives the nodes a helper too.
                            if let Some(hm) = state.gui_state.home_machines.as_ref() {
                                const CN: [f32; 4] = [0.4, 0.85, 0.95, 0.85];
                                for n in &hm.conduit_nodes {
                                    crate::renderer::line::push_circle(&mut ring_lines, [n.pos.0, n.pos.1, n.pos.2], 0.18, CN, 14);
                                }
                            }
                        }
                    }

                    // Build-mode AVATAR (v0.557): a little figure you drag by its pyramid gizmo to set
                    // where you spawn when you leave build mode. Body box + sphere head + a bright
                    // pyramid handle on the floor (white while grabbed).
                    if state.gui_state.construction_active {
                        if let Some((cx, cz)) = state.gui_state.build_char_pos {
                            if state.construction_char_mesh.is_none() {
                                // Player-sized body (~0.5 wide x 1.55 tall x 0.3 deep). (v0.560)
                                let m = state.renderer.add_mesh(Mesh::box_xyz(&state.renderer.device, 0.5, 1.55, 0.3));
                                state.construction_char_mesh = Some(m);
                            }
                            if state.construction_char_pyramid_mesh.is_none() {
                                let m = state.renderer.add_mesh(Mesh::pyramid(&state.renderer.device, 1.0, 1.0));
                                state.construction_char_pyramid_mesh = Some(m);
                            }
                            if state.construction_char_mat.is_none() {
                                // theme-exempt: build-mode avatar marker, friendly teal.
                                let m = state.renderer.add_material_full([0.30, 0.72, 0.80, 1.0], 0.0, 0.5, 0.0, 0.25);
                                state.construction_char_mat = Some(m);
                            }
                            if state.construction_node_mesh.is_none() {
                                let m = state.renderer.add_mesh(Mesh::sphere(&state.renderer.device, 1.0, 12, 16));
                                state.construction_node_mesh = Some(m);
                            }
                            if state.construction_node_mat.is_none() {
                                // theme-exempt: gizmo handle.
                                let m = state.renderer.add_material_full([1.0, 0.82, 0.2, 1.0], 0.0, 0.4, 0.0, 0.6);
                                state.construction_node_mat = Some(m);
                            }
                            if state.construction_node_mat_hot.is_none() {
                                // theme-exempt: grabbed-gizmo highlight.
                                let m = state.renderer.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.3, 0.0, 1.0);
                                state.construction_node_mat_hot = Some(m);
                            }
                            if state.construction_node_mat_hover.is_none() {
                                // theme-exempt: hover highlight.
                                let m = state.renderer.add_material_full([1.0, 0.96, 0.62, 1.0], 0.0, 0.35, 0.0, 0.95);
                                state.construction_node_mat_hover = Some(m);
                            }
                            let body_mesh = state.construction_char_mesh.unwrap();
                            let pyr_mesh = state.construction_char_pyramid_mesh.unwrap();
                            let head_mesh = state.construction_node_mesh.unwrap();
                            let char_mat = state.construction_char_mat.unwrap();
                            // Pyramid handle: idle -> hover -> active (grabbed) by colour (v0.569).
                            let pyr_mat = if state.construction_char_grab {
                                state.construction_node_mat_hot.unwrap()
                            } else if hover == HoverGizmo::Char {
                                state.construction_node_mat_hover.unwrap()
                            } else {
                                state.construction_node_mat.unwrap()
                            };
                            // Player-sized avatar STANDING on the floor (v0.560): body box ~1.55 m + a
                            // head, so the marker reads as the player's height/width. Overlay pass ->
                            // visible through walls.
                            overlay_objects.push(RenderObject {
                                position: Vec3::new(cx, 0.0, cz),
                                rotation: Quat::IDENTITY,
                                scale: Vec3::ONE,
                                mesh: body_mesh,
                                material: char_mat,
                            });
                            overlay_objects.push(RenderObject {
                                position: Vec3::new(cx, 1.66, cz),
                                rotation: Quat::IDENTITY,
                                scale: Vec3::splat(0.2),
                                mesh: head_mesh,
                                material: char_mat,
                            });
                            // Pyramid gizmo BELOW the floor with its top vertex at the floor (operator
                            // note): apex at y=0, base at -0.4.
                            overlay_objects.push(RenderObject {
                                position: Vec3::new(cx, -0.4, cz),
                                rotation: Quat::IDENTITY,
                                scale: Vec3::new(0.5, 0.4, 0.5),
                                mesh: pyr_mesh,
                                material: pyr_mat,
                            });
                        }
                    }

                    // Door/window OPENING gizmos (v0.546): a distinct cyan CUBE at each opening,
                    // draggable along its wall (vs the yellow corner spheres). Cached mesh + material.
                    if state.gui_state.construction_active && state.gui_state.home_structure.is_some() {
                        if state.construction_opening_mesh.is_none() {
                            let m = state.renderer.add_mesh(Mesh::box_xyz(&state.renderer.device, 1.0, 1.0, 1.0));
                            state.construction_opening_mesh = Some(m);
                        }
                        if state.construction_opening_mat.is_none() {
                            // theme-exempt: editor gizmo, distinct cyan + emissive so it stands out.
                            let m = state.renderer.add_material_full([0.2, 0.9, 1.0, 1.0], 0.0, 0.4, 0.0, 0.7);
                            state.construction_opening_mat = Some(m);
                        }
                        let mesh = state.construction_opening_mesh.unwrap();
                        let mat = state.construction_opening_mat.unwrap();
                        // Grabbed/hover highlight materials (created by the corner block, which runs first
                        // under the same condition). Idle cyan -> hover cream -> active RGB. (v0.569)
                        let hot_mat = state.construction_node_mat_hot.unwrap();
                        let hover_mat = state.construction_node_mat_hover.unwrap();
                        let node_mat = state.construction_node_mat.unwrap(); // yellow, for resize handles
                        let grabbed_op = state.construction_opening_grab;
                        let resize_grab = state.construction_opening_resize;
                        let gizmos = {
                            let hs = state.gui_state.home_structure.as_ref().unwrap();
                            opening_gizmos(hs)
                        };
                        const S: f32 = 0.35;
                        let hs = state.gui_state.home_structure.as_ref().unwrap();
                        for (idx, p) in &gizmos {
                            // Align the cube to ITS wall (v0.560) instead of fixed north/south: rotate
                            // by the wall's heading so its faces sit square to the wall at any angle.
                            let yaw = hs
                                .walls
                                .get(idx.0)
                                .map_or(0.0, |w| (w.b.1 - w.a.1).atan2(w.b.0 - w.a.0));
                            let m = if grabbed_op == Some(*idx) {
                                hot_mat
                            } else if hover == HoverGizmo::Opening(idx.0, idx.1) {
                                hover_mat
                            } else {
                                mat
                            };
                            all_objects.push(RenderObject {
                                position: Vec3::new(p.x, p.y - S * 0.5, p.z), // box_xyz y-bottom -> centre
                                rotation: Quat::from_rotation_y(-yaw),
                                scale: Vec3::splat(S),
                                mesh,
                                material: m,
                            });
                        }
                        // RESIZE handles (v0.578): 4 smaller YELLOW cubes per opening at its aperture
                        // edges (left/right resize width, top/bottom resize height); centred on the seam
                        // between the opening and the wall/frame. The grabbed one shifts RGB.
                        const RS: f32 = 0.16;
                        for ((wi, oi, edge), p) in opening_resize_handles(hs) {
                            let yaw = hs.walls.get(wi).map_or(0.0, |w| (w.b.1 - w.a.1).atan2(w.b.0 - w.a.0));
                            let m = if resize_grab == Some((wi, oi, edge)) { hot_mat } else { node_mat };
                            all_objects.push(RenderObject {
                                position: Vec3::new(p.x, p.y - RS * 0.5, p.z),
                                rotation: Quat::from_rotation_y(-yaw),
                                scale: Vec3::splat(RS),
                                mesh,
                                material: m,
                            });
                        }
                    }

                    // ── Remote players (multiplayer co-presence, v0.472) ──
                    // Draw a simple humanoid marker (body + head, a distinct teal) at each remote
                    // player's interpolated position. The sent position is the eye/camera height, so
                    // the head sits there and the body hangs below it. (Nameplates are a follow-up.)
                    if !showroom {
                        if state.remote_avatar.is_none() {
                            let body = state.renderer.add_mesh(
                                Mesh::box_xyz(&state.renderer.device, 0.42, 1.4, 0.26));
                            let head = state.renderer.add_mesh(
                                Mesh::sphere(&state.renderer.device, 0.17, 12, 14));
                            // Teal, slightly emissive so a remote player reads at a glance.
                            let mat = state.renderer.add_material_full(
                                [0.15, 0.75, 0.85, 1.0], 0.0, 0.5, 1.0, 0.25);
                            state.remote_avatar = Some((body, head, mat));
                        }
                        if let Some((body, head, mat)) = state.remote_avatar {
                            for (_e, (t, _r)) in state
                                .game_world
                                .world
                                .query::<(&crate::ecs::components::Transform, &crate::net::sync::RemotePlayer)>()
                                .iter()
                            {
                                all_objects.push(RenderObject {
                                    position: t.position - Vec3::new(0.0, 0.85, 0.0),
                                    rotation: t.rotation,
                                    scale: Vec3::ONE,
                                    mesh: body,
                                    material: mat,
                                });
                                all_objects.push(RenderObject {
                                    position: t.position + Vec3::new(0.0, 0.05, 0.0),
                                    rotation: t.rotation,
                                    scale: Vec3::ONE,
                                    mesh: head,
                                    material: mat,
                                });
                            }
                        }
                    }
                    // Showroom ground under the avatar (tinted by the backdrop): a planet
                    // SPHERE the avatar stands on for body backdrops (Earth/Mars/Moon), else
                    // a flat disc. The sphere (radius 30) is centered 30 below the avatar so
                    // its top is the standing surface; you see the surface curve away. (v0.449)
                    if showroom {
                        if let Some((gm, gmat)) = state.showroom_ground {
                            let is_sphere = state
                                .showroom_backdrops
                                .get(state.gui_state.showroom_backdrop)
                                .map(|b| b.sphere)
                                .unwrap_or(false);
                            if is_sphere {
                                if let Some(bm) = state.showroom_body {
                                    all_objects.push(RenderObject {
                                        position: state.avatar_base - Vec3::new(0.0, 30.0, 0.0),
                                        rotation: Quat::IDENTITY,
                                        scale: Vec3::ONE,
                                        mesh: bm,
                                        material: gmat,
                                    });
                                }
                            } else {
                                all_objects.push(RenderObject {
                                    position: state.avatar_base,
                                    rotation: Quat::IDENTITY,
                                    scale: Vec3::ONE,
                                    mesh: gm,
                                    material: gmat,
                                });
                            }
                        }
                    }

                    // Solar system hologram centered in the designated hologram room (1m above floor)
                    let hologram_center = state.hologram_room_center;

                    // Hologram (solar-system map) is part of the home, so hide it in the
                    // showroom too (it was leaking into the void when panning, v0.443).
                    if !showroom {
                        // Orbit rings (centered on hologram)
                        for &(mesh_idx, mat_idx) in &state.hologram_orbits {
                            all_objects.push(RenderObject {
                                position: hologram_center,
                                rotation: Quat::IDENTITY,
                                scale: Vec3::ONE,
                                mesh: mesh_idx,
                                material: mat_idx,
                            });
                        }

                        // Planet bodies
                        for (mesh_idx, mat_idx, local_pos, _name) in &state.hologram_objects {
                            all_objects.push(RenderObject {
                                position: hologram_center + *local_pos,
                                rotation: Quat::IDENTITY,
                                scale: Vec3::ONE,
                                mesh: *mesh_idx,
                                material: *mat_idx,
                            });
                        }

                        // Pin markers above each planet
                        for (mesh_idx, mat_idx, local_pos, _name) in &state.hologram_pins {
                            all_objects.push(RenderObject {
                                position: hologram_center + *local_pos,
                                rotation: Quat::IDENTITY,
                                scale: Vec3::ONE,
                                mesh: *mesh_idx,
                                material: *mat_idx,
                            });
                        }
                    }

                    // Raycast from camera to detect which planet pin is targeted
                    {
                        let ray_origin = state.camera.position;
                        let ray_dir = state.camera.forward();
                        let pin_hit_radius = 0.06; // slightly larger than pin head for easy targeting
                        let mut closest_hit: Option<(f32, String)> = None;

                        for (_mesh_idx, _mat_idx, local_pos, name) in &state.hologram_pins {
                            let pin_world = hologram_center + *local_pos;
                            // Sphere-ray intersection with pin head center
                            let oc = ray_origin - pin_world;
                            let b = oc.dot(ray_dir);
                            let c = oc.dot(oc) - pin_hit_radius * pin_hit_radius;
                            let discriminant = b * b - c;
                            if discriminant >= 0.0 {
                                let t = -b - discriminant.sqrt();
                                if t > 0.0 {
                                    if closest_hit.as_ref().map_or(true, |(d, _)| t < *d) {
                                        closest_hit = Some((t, name.clone()));
                                    }
                                }
                            }
                        }

                        state.targeted_planet = closest_hit.map(|(_, name)| name);
                    }

                    // Render Earth relative to the player's GEO-orbit position.
                    // The player spawns inside their homestead which sits at the ship's
                    // world position (state.ship_world_pos) ~42,164 km from Earth's
                    // centre. Earth itself lives at world origin; we render it as a
                    // single giant object offset by -ship_world_pos so it appears
                    // below the player as expected for geostationary orbit.
                    let elapsed = (now - state.start_time).as_secs_f32();
                    if let (Some(ref mut planet), Some(mesh_idx)) = (&mut state.planet, state.planet_mesh) {
                        // Earth position relative to ship (ship at GEO, Earth at world origin)
                        let earth_offset = -state.ship_world_pos;
                        let cam_world = glam::DVec3::new(
                            state.camera.position.x as f64,
                            state.camera.position.y as f64,
                            state.camera.position.z as f64,
                        );
                        let dist_to_earth = (earth_offset - cam_world).length();

                        // Update LOD based on distance
                        planet.world_position = earth_offset;
                        if planet.update_lod(cam_world) {
                            let ico = planet.icosphere();
                            state.renderer.meshes[mesh_idx] = Mesh::from_icosphere(&state.renderer.device, ico, 1.0);
                            log::info!("Planet LOD changed: {:?}, {} faces", planet.lod(), planet.face_count());
                        }

                        // Render position: Earth center relative to camera
                        let render_pos = Vec3::new(
                            earth_offset.x as f32,
                            earth_offset.y as f32,
                            earth_offset.z as f32,
                        );
                        let scale = planet.def.radius as f32;

                        // One-shot debug line so we can see render params in the console.
                        static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                        if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                            crate::debug::push_debug(format!(
                                "Earth: offset=({:.0},{:.0},{:.0}), scale={:.0}, dist={:.0}km, LOD={:?}",
                                render_pos.x, render_pos.y, render_pos.z, scale, dist_to_earth / 1000.0, planet.lod()
                            ));
                        }

                        let rotation = Quat::from_rotation_y(elapsed * 0.01);
                        // Earth -> the CELESTIAL pass (huge far so it is not clipped). Hidden
                        // in the showroom (its dark limb would fill the view). (v0.450)
                        if !showroom {
                            celestial_objects.push(RenderObject {
                                position: render_pos,
                                rotation,
                                scale: Vec3::splat(scale),
                                mesh: mesh_idx,
                                material: state.planet_material,
                            });
                        }

                        // ── The real solar system around the home ──
                        // (map sync, increment B). Was: a single hardcoded
                        // Sun along a fake [0.3,1,0.5] vector. Now every
                        // body the Maps page shows is spawned at its TRUE
                        // position relative to Earth, from the SAME
                        // canonical Keplerian model (crate::cosmos) the
                        // Maps page reads — so the FPS sky IS the Maps
                        // page, just real size. Earth itself stays the
                        // dedicated PlanetRenderer at world origin (above);
                        // every other body is placed at
                        //   (helio(body) - helio(earth)) * metres-per-AU
                        // i.e. Earth-centred world space, then offset into
                        // the camera/floating-origin frame by -ship_pos
                        // exactly like Earth. Live sim time = seconds
                        // since J2000 from the real clock, so the sky
                        // matches the real date (same as Maps "Now").
                        let sim_t = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs_f64())
                            .unwrap_or(0.0)
                            - 946_728_000.0; // Unix secs at J2000.0 epoch
                        let earth_helio_au = crate::cosmos::find_body("earth")
                            .map(|e| crate::cosmos::body_world_position_3d_au(e, sim_t))
                            .unwrap_or(glam::DVec3::ZERO);
                        let mut sun_rel_earth_m = glam::DVec3::ZERO;
                        for b in crate::cosmos::sol_bodies() {
                            // Earth is the dedicated PlanetRenderer; its
                            // moons orbit a planet (parent != sun) so skip
                            // them to keep the sky readable. Render the
                            // Sun + everything that directly orbits it
                            // (planets, dwarfs, named belt bodies) + our
                            // Moon. This mirrors the Maps left-list.
                            if b.id == "earth" { continue; }
                            let is_sun = b.body_type == "star";
                            let direct_solar = b.parent.as_deref() == Some("sun");
                            if !is_sun && !direct_solar && b.id != "moon" { continue; }

                            let helio_au =
                                crate::cosmos::body_world_position_3d_au(b, sim_t);
                            let rel_earth_m =
                                (helio_au - earth_helio_au) * crate::cosmos::M_PER_AU;
                            if is_sun { sun_rel_earth_m = rel_earth_m; }
                            let render_off = rel_earth_m - state.ship_world_pos;
                            let dist = render_off.length().max(1.0);
                            let radius_m = (b.radius_km * 1000.0) as f64;

                            // Visibility floor: without a bloom/billboard
                            // pass a real planet from tens of millions of
                            // km is sub-pixel. Clamp the on-screen disc to
                            // a minimum angular size so it reads as a body
                            // (true POSITION is always exact — only the
                            // disc is floored, same trick the old Sun used
                            // and how Elite/KSP draw distant bodies). The
                            // Sun gets a bigger floor so it reads as a sun.
                            let min_ang = if is_sun { 0.045 } else { 0.0028 };
                            let visual_scale = radius_m.max(dist * min_ang) as f32;

                            let material = if is_sun {
                                state.sun_material
                            } else {
                                match b.body_type.as_str() {
                                    "gas_giant" | "gas giant" => state.solar_body_materials[1],
                                    "ice_giant" | "ice giant" => state.solar_body_materials[1],
                                    "dwarf_planet" | "dwarf" | "kuiper" => {
                                        state.solar_body_materials[2]
                                    }
                                    "terrestrial" | "moon" | "asteroid" => {
                                        state.solar_body_materials[0]
                                    }
                                    _ => state.solar_body_materials[3],
                                }
                            };
                            if !showroom {
                                celestial_objects.push(RenderObject {
                                    position: Vec3::new(
                                        render_off.x as f32,
                                        render_off.y as f32,
                                        render_off.z as f32,
                                    ),
                                    rotation: Quat::from_rotation_y(elapsed * 0.01),
                                    scale: Vec3::splat(visual_scale),
                                    mesh: mesh_idx,
                                    material,
                                });
                            }
                        }

                        // ── Orbit paths (v0.262.20 — thin world lines) ──
                        // Per frame: offset each cached parent-frame
                        // ellipse to its parent's Earth-relative position
                        // (SAME frame as the bodies, so a planet sits
                        // exactly on its ring; the Moon's ring is centred
                        // on Earth) and emit a LineList (2 verts/segment).
                        // draw_lines_onto depth-occludes any segment that
                        // passes behind a planet — the directional cue
                        // the operator asked for, via real occlusion.
                        // Operator-chosen near-black #020305 (barely
                        // there). Full alpha — the colour itself is the
                        // dimness, not the blend.
                        const ORBIT_RGBA: [f32; 4] =
                            [0.0078, 0.0118, 0.0196, 1.0];
                        // Skip the rings in the showroom (no AU-scale sky-rings in the
                        // void — they'd reappear now that the celestial pass un-clips
                        // them). v0.451. The bodies are already showroom-gated above.
                        for (pts_m, parent_id) in
                            state.solar_orbit_paths.iter().filter(|_| !showroom)
                        {
                            let parent_helio_au = if parent_id == "sun" {
                                glam::DVec3::ZERO
                            } else {
                                crate::cosmos::find_body(parent_id)
                                    .map(|p| crate::cosmos::body_world_position_3d_au(p, sim_t))
                                    .unwrap_or(glam::DVec3::ZERO)
                            };
                            let off = (parent_helio_au - earth_helio_au)
                                * crate::cosmos::M_PER_AU
                                - state.ship_world_pos;
                            let off = [off.x as f32, off.y as f32, off.z as f32];
                            for seg in pts_m.windows(2) {
                                let a = [seg[0][0] + off[0], seg[0][1] + off[1], seg[0][2] + off[2]];
                                let b = [seg[1][0] + off[0], seg[1][1] + off[1], seg[1][2] + off[2]];
                                orbit_lines.push(crate::renderer::line::LineVertex {
                                    position: a,
                                    color: ORBIT_RGBA,
                                });
                                orbit_lines.push(crate::renderer::line::LineVertex {
                                    position: b,
                                    color: ORBIT_RGBA,
                                });
                            }
                        }

                        // Keep the cached Sun pos + the shader's light
                        // direction pointed at the REAL Sun so Earth's lit
                        // hemisphere matches the visible Sun disc (was a
                        // fixed [0.3,1,0.5] that disagreed with the disc).
                        state.sun_world_pos = sun_rel_earth_m;
                        let sun_dir = sun_rel_earth_m.normalize_or_zero();
                        // GI master switch (v0.571): when OFF, zero the sun + fill so only LOCAL placed
                        // lights illuminate -- the operator's "turn off global illumination and still
                        // see" test. Default ON restores the normal sun (2.5) + the cool fill (0.6).
                        let gi = state.gui_state.gi_enabled;
                        if sun_dir != glam::DVec3::ZERO {
                            state.renderer.set_sun_light(
                                Vec3::new(
                                    sun_dir.x as f32,
                                    sun_dir.y as f32,
                                    sun_dir.z as f32,
                                ),
                                [1.0, 0.97, 0.92],
                                if gi { 2.5 } else { 0.0 },
                            );
                        }
                        // The fill is otherwise set once at init; re-assert it each frame so the GI
                        // toggle is authoritative (restores the default when GI is back on).
                        state.renderer.set_fill_light(
                            Vec3::new(-0.5, 0.3, -0.3),
                            [0.4, 0.5, 0.7],
                            if gi { 0.6 } else { 0.0 },
                        );
                    }

                    // Update FPS counter
                    state.gui_state.fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
                    // Frame-time ring buffer for the F2 performance overlay sparkline.
                    {
                        let ft = &mut state.gui_state.frame_times;
                        ft.push(dt * 1000.0);
                        if ft.len() > 120 {
                            let excess = ft.len() - 120;
                            ft.drain(0..excess);
                        }
                    }
                    // Sample EngineState diagnostics for the F2/F4 overlays, but ONLY
                    // while the relevant overlay is open so the syscall/entity-walk
                    // cost nothing when hidden. (v0.482)
                    if state.gui_state.show_perf_overlay || state.gui_state.show_system_overlay {
                        state.gui_state.diag_uptime_secs = state.start_time.elapsed().as_secs();
                    }
                    if state.gui_state.show_perf_overlay {
                        state.gui_state.diag_entity_count = state.game_world.world.iter().count();
                    }
                    if state.gui_state.show_system_overlay {
                        if let Some(u) = memory_stats::memory_stats() {
                            state.gui_state.diag_mem_mb = u.physical_mem as f32 / 1_048_576.0;
                        }
                    }
                    // Settings audio "Test microphone" toggle: keep the mic loopback
                    // running while mic_test_active, on the chosen devices (v0.485).
                    {
                        let want = state.gui_state.mic_test_active;
                        // Start/stop only on the toggle EDGE, never every frame: a
                        // failing start drops MIC_RUNNING back to false within a frame,
                        // so a per-frame "start if not running" would spin-retry and
                        // flicker the status between "Starting..." and "Failed".
                        if want && !state.gui_state.mic_test_prev {
                            crate::net::voice::start_mic_test(
                                state.gui_state.audio_input_device.clone(),
                                state.gui_state.audio_output_device.clone(),
                            );
                        } else if !want && state.gui_state.mic_test_prev {
                            crate::net::voice::stop_mic_test();
                        }
                        state.gui_state.mic_test_prev = want;
                        // If the start failed (async, in the worker thread), flip the
                        // toggle back off so the button resets to "Test microphone" and
                        // does not look stuck on. The "Failed: ..." status stays visible.
                        if want
                            && !crate::net::voice::mic_test_running()
                            && crate::net::voice::mic_status().starts_with("Failed")
                        {
                            state.gui_state.mic_test_active = false;
                            state.gui_state.mic_test_prev = false;
                        }
                        // Decayed peak-hold meter for the UI.
                        let lvl = crate::net::voice::mic_level();
                        state.gui_state.mic_meter = (state.gui_state.mic_meter * 0.85).max(lvl);
                    }
                    // v0.488/490: push the live voice input params (gain / filter /
                    // transmit mode / activation threshold) to the worker every frame, so
                    // changes apply without restarting the test. `voice_ptt_held` is
                    // maintained by the raw winit key handler above (works in-game + for
                    // CapsLock); it only matters for the push-to-talk / push-to-mute modes.
                    {
                        crate::net::voice::set_input_params(
                            state.gui_state.voice_gain,
                            state.gui_state.voice_filter_mode,
                            state.gui_state.voice_transmit_mode,
                            state.gui_state.voice_vad_threshold,
                            state.gui_state.voice_ptt_held,
                        );
                    }
                    // v0.494 Phase D: run the live voice session while joined to a
                    // voice room (capture + encode + decode/play), and pump captured
                    // Opus to every connected peer. Edge-triggered start/stop.
                    {
                        let want_session = state.gui_state.voice_active_room.is_some();
                        if want_session && !state.gui_state.voice_session_prev {
                            crate::net::voice::start_voice_session(
                                state.gui_state.audio_input_device.clone(),
                                state.gui_state.audio_output_device.clone(),
                            );
                        } else if !want_session && state.gui_state.voice_session_prev {
                            crate::net::voice::stop_voice_session();
                            state.gui_state.voice_connected_peers.clear();
                        }
                        state.gui_state.voice_session_prev = want_session;
                        // Send captured mic frames to each connected voice peer.
                        if want_session && !state.gui_state.voice_connected_peers.is_empty() {
                            let frames = crate::net::voice::drain_voice_send();
                            if !frames.is_empty() {
                                let peers: Vec<String> = state.gui_state.voice_connected_peers.iter().cloned().collect();
                                if let Some(ref webrtc) = state.gui_state.webrtc {
                                    for opus in frames {
                                        for peer in &peers {
                                            webrtc.send_voice(peer.clone(), opus.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Poll updater for background thread results
                    if state.gui_state.updater.poll(dt as f64) {
                        // An update just became available -- show toast
                        state.gui_state.update_toast_visible = true;
                    }

                    // ── Bridge ECS/DataStore state into GuiState for GUI pages ──

                    // Bridge player health and inventory from ECS
                    for (_entity, (health, _ctrl)) in state.game_world.world.query::<(&Health, &Controllable)>().iter() {
                        state.gui_state.player_health = health.current;
                        state.gui_state.player_health_max = health.max;
                    }
                    // Bridge inventory from the player entity
                    let item_registry = state.data_store.get::<ItemRegistry>("item_registry");
                    for (_entity, (inv, _ctrl)) in state.game_world.world.query::<(&Inventory, &Controllable)>().iter() {
                        state.gui_state.inventory_max_slots = inv.max_slots;
                        state.gui_state.inventory_items = inv.slots.iter().map(|slot| {
                            slot.as_ref().map(|stack| {
                                let name = item_registry
                                    .and_then(|reg| reg.items.get(&stack.item_id))
                                    .map(|def| def.name.clone())
                                    .unwrap_or_else(|| stack.item_id.clone());
                                GuiItemSlot {
                                    item_id: stack.item_id.clone(),
                                    name,
                                    quantity: stack.quantity,
                                }
                            })
                        }).collect();
                    }
                    // Bridge player vitals + active status effects from ECS for the HUD.
                    {
                        let fx_registry = state.data_store.get::<
                            crate::systems::status_effects::StatusEffectRegistry,
                        >("status_effect_registry");
                        for (_entity, (vitals, effects, _ctrl)) in state
                            .game_world
                            .world
                            .query::<(
                                &crate::ecs::components::Vitals,
                                &crate::ecs::components::StatusEffects,
                                &Controllable,
                            )>()
                            .iter()
                        {
                            state.gui_state.vitals.satiation = vitals.satiation;
                            state.gui_state.vitals.hydration = vitals.hydration;
                            state.gui_state.vitals.energy = vitals.energy;
                            state.gui_state.vitals.oxygen = vitals.oxygen;
                            state.gui_state.vitals.body_temp_c = vitals.body_temp_c;
                            state.gui_state.vitals.satiation_max = vitals.satiation_max;
                            state.gui_state.vitals.hydration_max = vitals.hydration_max;
                            state.gui_state.vitals.energy_max = vitals.energy_max;
                            state.gui_state.vitals.oxygen_max = vitals.oxygen_max;
                            state.gui_state.vitals.waste = vitals.waste;
                            state.gui_state.vitals.waste_max = vitals.waste_max;
                            state.gui_state.vitals.sealed = state
                                .data_store
                                .get::<crate::ecs::components::EnvironmentContext>(
                                    "environment_context",
                                )
                                .map(|e| e.sealed)
                                .unwrap_or(true);
                            state.gui_state.vitals.effects = effects
                                .active
                                .iter()
                                .map(|e| {
                                    let name = fx_registry
                                        .and_then(|r| r.get(&e.id))
                                        .map(|d| d.name.clone())
                                        .unwrap_or_else(|| e.id.clone());
                                    (name, e.remaining)
                                })
                                .collect();
                        }
                    }
                    // Bridge player skills (live levels + XP) from ECS for the profile
                    // Skills panel. Reads SkillRegistry for the display name + the
                    // per-level XP curve (xp_needed = XP to reach the next level).
                    {
                        let skill_reg = state
                            .data_store
                            .get::<SkillRegistry>("skill_registry");
                        state.gui_state.skills.clear();
                        if let Some(reg) = skill_reg {
                            for (_e, (skills, _ctrl)) in state
                                .game_world
                                .world
                                .query::<(&PlayerSkills, &Controllable)>()
                                .iter()
                            {
                                // Show EVERY defined skill with the player's progress
                                // (0 if untrained) — a complete, real skill sheet, not
                                // the old static placeholder list. Grouped by category.
                                for (id, def) in reg.skills.iter() {
                                    let prog = skills.skills.get(id);
                                    let level = prog.map(|p| p.level).unwrap_or(0);
                                    let xp = prog.map(|p| p.xp).unwrap_or(0);
                                    state.gui_state.skills.push(crate::gui::GuiSkill {
                                        id: id.clone(),
                                        name: def.name.clone(),
                                        category: def.category.clone(),
                                        level,
                                        xp,
                                        xp_needed: def.xp_for_level(level + 1),
                                    });
                                }
                                state.gui_state.skills.sort_by(|a, b| {
                                    a.category.cmp(&b.category).then(a.name.cmp(&b.name))
                                });
                                break;
                            }
                        }
                    }
                    // Bridge player quests (active steps + completed) from ECS for
                    // the profile Quests panel.
                    {
                        let quest_reg =
                            state.data_store.get::<QuestRegistry>("quest_registry");
                        state.gui_state.quests.clear();
                        for (_e, (tracker, _ctrl)) in state
                            .game_world
                            .world
                            .query::<(&QuestTracker, &Controllable)>()
                            .iter()
                        {
                            for active in &tracker.active_quests {
                                let def = quest_reg.and_then(|r| r.get(&active.quest_id));
                                let name = def
                                    .map(|d| d.name.clone())
                                    .unwrap_or_else(|| active.quest_id.clone());
                                let step_total = def.map(|d| d.steps.len()).unwrap_or(0);
                                let step_desc = def
                                    .and_then(|d| d.steps.get(active.current_step))
                                    .map(|s| s.description.clone())
                                    .unwrap_or_default();
                                state.gui_state.quests.push(crate::gui::GuiQuest {
                                    name,
                                    step_index: active.current_step,
                                    step_total,
                                    step_desc,
                                    completed: false,
                                });
                            }
                            for cid in &tracker.completed_quests {
                                let name = quest_reg
                                    .and_then(|r| r.get(cid))
                                    .map(|d| d.name.clone())
                                    .unwrap_or_else(|| cid.clone());
                                state.gui_state.quests.push(crate::gui::GuiQuest {
                                    name,
                                    step_index: 0,
                                    step_total: 0,
                                    step_desc: String::new(),
                                    completed: true,
                                });
                            }
                            break;
                        }
                    }
                    // Bridge growing crops from ECS for the gardening (Garden) panel.
                    {
                        let plant_reg = state
                            .data_store
                            .get::<crate::systems::farming::PlantRegistry>("plant_registry");
                        state.gui_state.crops.clear();
                        for (entity, crop) in state
                            .game_world
                            .world
                            .query::<&crate::ecs::components::CropInstance>()
                            .iter()
                        {
                            let def = plant_reg.and_then(|r| r.get(&crop.crop_def_id));
                            let name = def
                                .map(|d| d.name.clone())
                                .unwrap_or_else(|| crop.crop_def_id.clone());
                            let stages: Vec<&str> = def.map(|d| d.stages()).unwrap_or_else(|| {
                                crate::ecs::components::DEFAULT_GROWTH_STAGES
                                    .iter()
                                    .copied()
                                    .collect()
                            });
                            let dead =
                                crop.growth_stage.as_str() == crate::ecs::components::STAGE_DEAD;
                            let last = stages.last().copied().unwrap_or("");
                            let mature = !dead && crop.growth_stage.as_str() == last;
                            let progress = if dead {
                                0.0
                            } else {
                                stages
                                    .iter()
                                    .position(|s| *s == crop.growth_stage.as_str())
                                    .map(|i| (i as f32 + 1.0) / stages.len().max(1) as f32)
                                    .unwrap_or(0.0)
                            };
                            state.gui_state.crops.push(crate::gui::GuiCrop {
                                entity_bits: entity.to_bits().into(),
                                name,
                                stage: crop.growth_stage.clone(),
                                progress,
                                water: crop.water_level,
                                health: crop.health,
                                mature,
                                dead,
                                tower_id: crop.tower_id.clone(),
                                tower_slot: crop.tower_slot,
                                n: def.map(|d| d.nutrient_n).unwrap_or(0.0),
                                p: def.map(|d| d.nutrient_p).unwrap_or(0.0),
                                k: def.map(|d| d.nutrient_k).unwrap_or(0.0),
                                water_per_day: def.map(|d| d.water_per_day).unwrap_or(0.0),
                                temp_min: def.map(|d| d.temp_min_c).unwrap_or(0.0),
                                temp_max: def.map(|d| d.temp_max_c).unwrap_or(0.0),
                            });
                        }
                        // One-time tower compatibility (operator: "make sure they
                        // grow together"): the shared reservoir pH / temperature /
                        // humidity window per tower. Static (tower configs + the
                        // plant registry), so compute once and cache in GuiState.
                        if state.gui_state.tower_compat.is_empty()
                            && !state.gui_state.tower_configs.is_empty()
                        {
                            if let Some(reg) = plant_reg {
                                let towers = state.gui_state.tower_configs.clone();
                                state.gui_state.tower_compat = towers
                                    .iter()
                                    .map(|t| crate::gui::compute_tower_compat(t, reg))
                                    .collect();
                            }
                        }
                    }
                    // Bridge asteroids + active mining drones from ECS for the Mining panel.
                    {
                        state.gui_state.asteroids.clear();
                        for (_e, ast) in state
                            .game_world
                            .world
                            .query::<&crate::ecs::components::AsteroidBody>()
                            .iter()
                        {
                            let p = ast.position;
                            let dist = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
                            state.gui_state.asteroids.push(crate::gui::GuiAsteroid {
                                id: ast.id.clone(),
                                name: ast.name.clone(),
                                classification: ast.classification.clone(),
                                ores: ast.ores.iter().map(|(id, q)| (id.clone(), *q)).collect(),
                                position: ast.position,
                                distance: dist,
                            });
                        }
                        state.gui_state.drones.clear();
                        for (_e, drone) in state
                            .game_world
                            .world
                            .query::<&crate::ecs::components::Drone>()
                            .iter()
                        {
                            let dur = drone.phase_duration(drone.phase);
                            let phase_progress = if dur > 0.0 {
                                (drone.phase_time / dur).clamp(0.0, 1.0)
                            } else {
                                1.0
                            };
                            state.gui_state.drones.push(crate::gui::GuiDrone {
                                manifest: drone.manifest.clone(),
                                phase: format!("{:?}", drone.phase),
                                cargo_total: drone.cargo_total(),
                                phase_progress,
                                target: drone.target.clone(),
                                distance: drone.distance(),
                                pos: drone.current_pos(),
                            });
                        }
                        // One drone per player: the panel shows the active drone +
                        // disables Launch while one is in flight.
                        state.gui_state.drone_active = !state.gui_state.drones.is_empty();
                    }

                    // Bridge game time from DataStore (if TimeSystem writes it)
                    if let Some(gt) = state
                        .data_store
                        .get::<std::sync::Mutex<GameTime>>("game_time")
                        .and_then(|m| m.lock().ok())
                    {
                        state.gui_state.game_time = Some(GuiGameTime {
                            hour: gt.hour,
                            day_count: gt.day_count,
                            season: format!("{:?}", gt.season),
                            is_daytime: gt.hour >= 6.0 && gt.hour <= 18.0,
                        });
                    }

                    // Bridge weather from DataStore (WeatherSystem writes it via Mutex).
                    if let Some(w) = state
                        .data_store
                        .get::<std::sync::Mutex<Weather>>("weather")
                        .and_then(|m| m.lock().ok())
                    {
                        state.gui_state.weather = Some(GuiWeather {
                            condition: format!("{:?}", w.condition),
                            temperature: w.temperature,
                            wind_speed: w.wind_speed,
                        });
                    }

                    // Bridge the live home power readout (ElectricalSystem writes it via
                    // Mutex). Drives the HUD power line that swings with day/night.
                    if let Some(ps) = state
                        .data_store
                        .get::<std::sync::Mutex<crate::systems::electrical::PowerStatus>>("power_status")
                        .and_then(|m| m.lock().ok())
                    {
                        state.gui_state.power_generation = ps.generation;
                        state.gui_state.power_consumption = ps.consumption;
                        state.gui_state.power_balance = ps.balance;
                        state.gui_state.power_battery_wh = ps.battery_wh;
                        state.gui_state.power_battery_capacity_wh = ps.battery_capacity_wh;
                        state.gui_state.power_autonomy_hours = ps.autonomy_hours;
                    }

                    // ── Auto-connect to server if configured AND seed unlocked ──
                    // Full-PQ guard (was the limited-mode squat bug): we must
                    // NOT auto-connect with an encrypted/locked seed. A locked
                    // identity can't derive Kyber, so a "connect" in that
                    // state registers a keyless name-squatter on the relay
                    // (Shaostoul → key-with-no-kyber), and every subsequent
                    // generate / restart bounces to a new DesktopUser_NNNN.
                    // Refuse to connect until private_key_bytes is in memory
                    // (Settings → Security → Unlock, or Recover from seed,
                    // or Generate New Identity).
                    let seed_unlocked = state.gui_state.private_key_bytes.is_some();
                    if !seed_unlocked
                        && state.gui_state.ws_client.is_none()
                        && !state.gui_state.encrypted_private_key.is_empty()
                        && state.gui_state.ws_status != "Identity locked — Settings → Security → Unlock (or Recover from seed) to connect"
                    {
                        // Surface the actionable reason once.
                        state.gui_state.ws_status = "Identity locked — Settings → Security → Unlock (or Recover from seed) to connect".to_string();
                    }
                    if !state.gui_state.server_url.is_empty()
                        && state.gui_state.ws_client.is_none()
                        && !state.gui_state.user_name.is_empty()
                        && state.gui_state.onboarding_complete
                        && !state.gui_state.ws_manually_disconnected
                        && state.gui_state.ws_reconnect_timer <= 0.0
                        && state.gui_state.ws_reconnect_attempts == 0
                        && seed_unlocked
                    {
                        let ws_url = crate::gui::pages::chat::derive_ws_url(&state.gui_state.server_url);
                        let name = state.gui_state.user_name.clone();
                        let pubkey = if state.gui_state.profile_public_key.is_empty() {
                            crate::gui::pages::chat::generate_random_hex_key()
                        } else {
                            state.gui_state.profile_public_key.clone()
                        };

                        // Full-PQ: advertise our Kyber768 public key so peers
                        // can dual-seal DMs to us. It is derived from the BIP39
                        // seed on recovery/unlock (kyber_public_b64); if the
                        // seed isn't in memory yet we re-derive it here when
                        // available, else connect without it (degraded: can
                        // receive once unlocked). The secret is never stored.
                        if state.gui_state.kyber_public_b64.is_empty() {
                            if let Some(ref seed) = state.gui_state.private_key_bytes {
                                if let Ok(pq) = crate::net::identity::derive_pq_identity(seed) {
                                    state.gui_state.kyber_public_b64 = pq.kyber_public_b64;
                                }
                            }
                        }
                        let kyber_public = state.gui_state.kyber_public_b64.clone();

                        state.gui_state.ws_client = Some(
                            crate::net::ws_client::WsClient::connect_with_kyber(&ws_url, &name, &pubkey, &kyber_public),
                        );
                        state.gui_state.ws_status = "Connecting...".to_string();
                    }

                    // ── Poll WebSocket messages from relay server ──
                    let mut ws_dropped = false;
                    if let Some(ref mut ws) = state.gui_state.ws_client {
                        let messages = ws.poll_messages();
                        if !ws.is_connected() {
                            if !ws_dropped {
                                crate::debug::push_debug("WS connection lost");
                            }
                            ws_dropped = true;
                        }
                        for raw in messages {
                            // Network overlay (v0.482): count every received frame.
                            state.gui_state.ws_msgs_in = state.gui_state.ws_msgs_in.saturating_add(1);
                            // Log raw message to debug console (truncate long messages)
                            {
                                let preview = if raw.len() > 300 { format!("{}...", &raw[..300]) } else { raw.clone() };
                                crate::debug::push_debug(format!("WS <<< {}", preview));
                            }
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) {
                                let msg_type = val.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
                                log::debug!("WS recv: type={}", msg_type);
                                match val.get("type").and_then(|t| t.as_str()) {
                                    Some("identify_challenge") => {
                                        // Inc3b — relay challenged us after `identify`.
                                        // Sign the canonical preimage with Dilithium3 from
                                        // our BIP39 seed and return `identify_response`.
                                        // Closes HIGH-2 (identity spoofing at identify).
                                        let nonce = val.get("nonce").and_then(|v| v.as_str()).unwrap_or("");
                                        if nonce.is_empty() {
                                            log::warn!("identify_challenge missing nonce");
                                        } else if let Some(ref seed) = state.gui_state.private_key_bytes {
                                            let preimage = format!(
                                                "hum/identify/v1\n{}\n{}",
                                                nonce, state.gui_state.profile_public_key
                                            );
                                            let sig = crate::net::identity::pq_sign_raw(seed, preimage.as_bytes());
                                            use base64::{engine::general_purpose::STANDARD as B64, Engine};
                                            let sig_b64 = B64.encode(&sig);
                                            let response = serde_json::json!({
                                                "type": "identify_response",
                                                "sig_b64": sig_b64,
                                            });
                                            if let Some(ref ws_client) = state.gui_state.ws_client {
                                                ws_client.send(&response.to_string());
                                            }
                                        } else {
                                            log::error!("identify_challenge received but seed not unlocked — cannot sign. Unlock identity to connect.");
                                            state.gui_state.ws_status = "Unlock your identity to complete connect (server requested challenge).".to_string();
                                        }
                                    }
                                    Some("chat") => {
                                        let sender_key = val.get("from")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let msg_timestamp = val.get("timestamp")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0);
                                        // Skip only messages WE sent from THIS client
                                        // (already added locally). Check by matching
                                        // our key + exact timestamp in recent sent list.
                                        if sender_key == state.gui_state.profile_public_key
                                            && state.gui_state.chat_sent_timestamps.contains(&msg_timestamp)
                                        {
                                            state.gui_state.chat_sent_timestamps.retain(|&t| t != msg_timestamp);
                                            continue;
                                        }
                                        let sender_name = val.get("from_name")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("Anonymous")
                                            .to_string();
                                        let content = val.get("content")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let timestamp = val.get("timestamp")
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0);
                                        let channel = val.get("channel")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("general")
                                            .to_string();
                                        // Robust dedup (fixes the 2026-05-20 "reply duplicated
                                        // after a while" report). The chat_sent_timestamps
                                        // fast-path above only catches the FIRST echo of our
                                        // OWN sends — it removes the entry on match, so it's
                                        // one-shot. A LATER replay of the same message (the WS
                                        // reconnect history re-fetch resets history_fetched and
                                        // re-pulls the last 50; or a duplicate broadcast) would
                                        // otherwise sail past the consumed fast-path and append
                                        // a second copy that only clears on app restart (in-
                                        // memory only; the relay always had one copy). A message
                                        // is uniquely identified by (sender_key, timestamp_ms)
                                        // — ms precision, per-sender — so skip if we already
                                        // hold it. Cheap: chat_messages is bounded to 200.
                                        if state.gui_state.chat_messages.iter()
                                            .any(|m| m.sender_key == sender_key && m.timestamp_ms == timestamp)
                                        {
                                            continue;
                                        }
                                        // Decode reply_to context if present (threads).
                                        let reply_to = val.get("reply_to").and_then(|r| {
                                            let from = r.get("from")?.as_str()?.to_string();
                                            let from_name = r.get("from_name")?.as_str()?.to_string();
                                            let preview = r.get("content")?.as_str()?.to_string();
                                            let ts = r.get("timestamp")?.as_u64()?;
                                            Some(crate::gui::ReplyContext {
                                                sender_key: from,
                                                sender_name: from_name,
                                                preview,
                                                timestamp_ms: ts,
                                            })
                                        });
                                        state.gui_state.chat_messages.push(
                                            crate::gui::ChatMessage {
                                                sender_name,
                                                sender_key,
                                                content,
                                                timestamp: crate::gui::pages::chat::format_timestamp(timestamp),
                                                timestamp_ms: timestamp,
                                                channel,
                                                reply_to,
                                                ..Default::default()
                                            },
                                        );
                                        // Bound message buffer
                                        while state.gui_state.chat_messages.len() > 200 {
                                            state.gui_state.chat_messages.remove(0);
                                        }
                                    }
                                    Some("peer_list") => {
                                        let peer_count = val.get("peers").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
                                        log::info!("peer_list received: {} peers", peer_count);
                                        crate::debug::push_debug(format!("Identified OK, {} peers online", peer_count));
                                        state.gui_state.chat_users.clear();
                                        state.gui_state.ws_status = "Connected".to_string();
                                        state.gui_state.server_connected = true;
                                        // Request tasks from server on connect
                                        if let Some(ref ws_client) = state.gui_state.ws_client {
                                            let get_tasks = serde_json::json!({"type": "task_list"});
                                            ws_client.send(&get_tasks.to_string());
                                        }
                                        if let Some(peers) = val.get("peers").and_then(|v| v.as_array()) {
                                            for peer in peers {
                                                let name = peer.get("display_name")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("Anonymous")
                                                    .to_string();
                                                let key = peer.get("public_key")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let role = peer.get("role")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let status = peer.get("status")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("online")
                                                    .to_string();
                                                // Capture peer's Kyber768 public key for full-PQ DM sealing
                                                if let Some(kyber) = peer.get("kyber_public").and_then(|v| v.as_str()) {
                                                    if !kyber.is_empty() && !key.is_empty() {
                                                        state.gui_state.peer_kyber_keys.insert(key.clone(), kyber.to_string());
                                                    }
                                                }
                                                // If this peer is us and our local name is empty, adopt the server's display_name
                                                if key == state.gui_state.profile_public_key
                                                    && state.gui_state.user_name.is_empty()
                                                    && name != "Anonymous"
                                                {
                                                    log::info!("Adopting display name from server: {}", name);
                                                    state.gui_state.user_name = name.clone();
                                                    crate::config::AppConfig::from_gui_state(&state.gui_state).save();
                                                }
                                                state.gui_state.chat_users.push(
                                                    crate::gui::ChatUser { name, public_key: key, role, status },
                                                );
                                            }
                                        }
                                    }
                                    Some("peer_joined") => {
                                        let name = val.get("display_name")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("Anonymous")
                                            .to_string();
                                        let key = val.get("public_key")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        let role = val.get("role")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        // Capture peer's Kyber768 public key for full-PQ DMs
                                        if let Some(kyber) = val.get("kyber_public").and_then(|v| v.as_str()) {
                                            if !kyber.is_empty() && !key.is_empty() {
                                                state.gui_state.peer_kyber_keys.insert(key.clone(), kyber.to_string());
                                            }
                                        }
                                        // Add if not already present
                                        if !state.gui_state.chat_users.iter().any(|u| u.public_key == key) {
                                            state.gui_state.chat_users.push(
                                                crate::gui::ChatUser { name, public_key: key.clone(), role, status: "online".into() },
                                            );
                                        }
                                    }
                                    Some("peer_left") => {
                                        if let Some(key) = val.get("public_key").and_then(|v| v.as_str()) {
                                            state.gui_state.chat_users.retain(|u| u.public_key != key);
                                        }
                                    }
                                    Some("channel_list") => {
                                        if let Some(channels) = val.get("channels").and_then(|v| v.as_array()) {
                                            state.gui_state.chat_channels.clear();
                                            for ch in channels {
                                                let id = ch.get("id")
                                                    .or_else(|| ch.get("name"))
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("general")
                                                    .to_string();
                                                let name = ch.get("name")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or(&id)
                                                    .to_string();
                                                let description = ch.get("description")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let category = ch.get("category_name")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("Text")
                                                    .to_string();
                                                // Read the persisted flags from the server so admin
                                                // toggles in Server Settings → Channels survive
                                                // restarts. Pre-v0.192 servers omit voice_enabled
                                                // entirely; treat missing-field as true so old
                                                // servers don't accidentally disable voice.
                                                let voice_enabled = ch.get("voice_enabled")
                                                    .and_then(|v| v.as_bool())
                                                    .unwrap_or(true);
                                                let read_only = ch.get("read_only")
                                                    .and_then(|v| v.as_bool())
                                                    .unwrap_or(false);
                                                let federated = ch.get("federated")
                                                    .and_then(|v| v.as_bool())
                                                    .unwrap_or(false);
                                                state.gui_state.chat_channels.push(
                                                    crate::gui::ChatChannel {
                                                        id,
                                                        name,
                                                        description,
                                                        category,
                                                        voice_joined: false,
                                                        voice_enabled,
                                                        read_only,
                                                        federated,
                                                        voice_participants: Vec::new(),
                                                    },
                                                );
                                            }
                                        }
                                    }
                                    Some("server_settings_state") => {
                                        // v0.200.0: relay broadcasts current server-wide
                                        // settings (per-role char limits, sharing toggles,
                                        // etc.). Cache them so the admin UI can show
                                        // current values + non-admin clients know what
                                        // limits apply to their messages.
                                        if let Some(s) = val.get("settings") {
                                            match serde_json::from_value::<crate::relay::storage::ServerSettings>(s.clone()) {
                                                Ok(settings) => {
                                                    log::info!(
                                                        "Server settings updated (max_chars: u={} v={} m={} a={})",
                                                        settings.max_chars_unverified, settings.max_chars_verified,
                                                        settings.max_chars_mod, settings.max_chars_admin
                                                    );
                                                    state.gui_state.server_settings = Some(settings);
                                                }
                                                Err(e) => {
                                                    log::warn!("Failed to parse server_settings_state: {e}");
                                                }
                                            }
                                        }
                                    }
                                    Some("role_list") => {
                                        // v0.241 (roles Phase R2): relay sends the full
                                        // role list on connect + after any role change.
                                        // Cache it for the user-modal role dropdown +
                                        // badge colors.
                                        if let Some(arr) = val.get("roles") {
                                            match serde_json::from_value::<Vec<crate::relay::storage::RoleDef>>(arr.clone()) {
                                                Ok(roles) => {
                                                    log::info!("Received {} role definitions", roles.len());
                                                    state.gui_state.chat_roles = roles;
                                                }
                                                Err(e) => log::warn!("Failed to parse role_list: {e}"),
                                            }
                                        }
                                    }
                                    Some("service_state") => {
                                        // v0.262.16 (Server→Services): admin-only reply
                                        // to service_control. Caches the daemon/soft
                                        // snapshot for the Services panel.
                                        if let Some(arr) = val.get("services") {
                                            match serde_json::from_value::<Vec<crate::relay::services::ServiceInfo>>(arr.clone()) {
                                                Ok(svcs) => {
                                                    log::info!("Received {} service states", svcs.len());
                                                    state.gui_state.service_state = svcs;
                                                }
                                                Err(e) => log::warn!("Failed to parse service_state: {e}"),
                                            }
                                        }
                                    }
                                    Some("banned_list") => {
                                        // v0.245: relay sends this only to admins, in
                                        // reply to banned_list_request + after any
                                        // ban/unban. Drives the Server Settings →
                                        // Banned users panel.
                                        if let Some(arr) = val.get("users") {
                                            match serde_json::from_value::<Vec<crate::relay::storage::BannedUser>>(arr.clone()) {
                                                Ok(users) => {
                                                    log::info!("Received {} banned users", users.len());
                                                    state.gui_state.chat_banned_users = users;
                                                }
                                                Err(e) => log::warn!("Failed to parse banned_list: {e}"),
                                            }
                                        }
                                    }
                                    Some("muted_list") => {
                                        // v0.246: relay sends this only to mods/admins,
                                        // in reply to muted_list_request + after any
                                        // mute/unmute. Drives the Server Settings →
                                        // Muted users panel.
                                        if let Some(arr) = val.get("users") {
                                            match serde_json::from_value::<Vec<crate::relay::storage::MutedUser>>(arr.clone()) {
                                                Ok(users) => {
                                                    log::info!("Received {} muted users", users.len());
                                                    state.gui_state.chat_muted_users = users;
                                                }
                                                Err(e) => log::warn!("Failed to parse muted_list: {e}"),
                                            }
                                        }
                                    }
                                    Some("system") => {
                                        if let Some(msg) = val.get("message").and_then(|v| v.as_str()) {
                                            log::info!("Relay system message: {}", msg);
                                            crate::debug::push_debug(format!("System: {}", msg));
                                            // Relay throttled this connection (per-IP identify rate
                                            // limit). Mirror the web client: back the reconnect off
                                            // PAST the 60s window. Without this the native looped every
                                            // 5s -- the backoff reset fires when the WS OPENS, before
                                            // the identify that gets rate-limited, so it never grew.
                                            // The flag stops that reset clobbering this delay until we
                                            // next actually retry. (v0.544)
                                            if msg.starts_with("Too many connection attempts")
                                                || msg.contains("Try again in a minute")
                                            {
                                                state.gui_state.ws_rate_limited = true;
                                                state.gui_state.ws_reconnect_delay = 65.0;
                                            }
                                            // Filter out internal sync + game messages from chat display.
                                            // `__game__:` prefix tags game-engine traffic (ambient
                                            // chatter, quest events, NPC dialog, world ticks) — those
                                            // belong on the game/perception channel, NOT in #general
                                            // where humans are talking. (Bug fix 2026-05-03.)
                                            // Multiplayer (v0.472): route game traffic into the sync
                                            // system (remote players join / move / leave) instead of
                                            // discarding it, then skip the chat display.
                                            if let Some(payload) = msg.strip_prefix("__game__:") {
                                                let payload = payload.to_string();
                                                route_game_message(state, &payload);
                                                continue;
                                            }
                                            if msg.starts_with("__sync_data__")
                                                || msg == "sync_ack"
                                            {
                                                continue;
                                            }
                                            // Add as a system message in current channel
                                            let now_ms = std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap_or_default()
                                                .as_millis() as u64;
                                            state.gui_state.chat_messages.push(
                                                crate::gui::ChatMessage {
                                                    sender_name: "System".to_string(),
                                                    sender_key: String::new(),
                                                    content: msg.to_string(),
                                                    timestamp: crate::gui::pages::chat::format_timestamp(now_ms),
                                                    timestamp_ms: now_ms,
                                                    // Don't leak into an open P2P group / DM (it'd vanish on reload).
                                                    channel: crate::gui::pages::chat::notice_channel(&state.gui_state.chat_active_channel),
                                                    ..Default::default()
                                                },
                                            );
                                        }
                                    }
                                    Some("full_user_list") => {
                                        let user_count = val.get("users").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
                                        log::info!("full_user_list received: {} users", user_count);
                                        // Full user list includes online + offline users
                                        if let Some(users) = val.get("users").and_then(|v| v.as_array()) {
                                            state.gui_state.chat_users.clear();
                                            for user in users {
                                                let name = user.get("name")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("Anonymous")
                                                    .to_string();
                                                let key = user.get("public_key")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let role = user.get("role")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let online = user.get("online")
                                                    .and_then(|v| v.as_bool())
                                                    .unwrap_or(false);
                                                let status = if online {
                                                    user.get("status")
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("online")
                                                        .to_string()
                                                } else {
                                                    "offline".to_string()
                                                };
                                                // Capture peer's Kyber768 public key for full-PQ DMs
                                                if let Some(kyber) = user.get("kyber_public").and_then(|v| v.as_str()) {
                                                    if !kyber.is_empty() && !key.is_empty() {
                                                        state.gui_state.peer_kyber_keys.insert(key.clone(), kyber.to_string());
                                                    }
                                                }
                                                state.gui_state.chat_users.push(
                                                    crate::gui::ChatUser { name, public_key: key, role, status },
                                                );
                                            }
                                            log::info!("Received full user list: {} users", state.gui_state.chat_users.len());
                                        }
                                    }
                                    Some("voice_channel_list") => {
                                        // Voice channels received from server
                                        if let Some(channels) = val.get("channels").and_then(|v| v.as_array()) {
                                            log::info!("Received {} voice channels", channels.len());
                                            // Add voice channels that don't already exist as text channels
                                            for vc in channels {
                                                // Voice is per-channel (v0.493): the entry's id IS a
                                                // text channel's string id, carrying that channel's
                                                // live voice roster.
                                                let vc_id = vc.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let vc_name = vc.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                if vc_id.is_empty() { continue; }
                                                // Live voice roster (public_key, display_name), v0.481.
                                                let roster: Vec<(String, String)> = vc.get("participants")
                                                    .and_then(|v| v.as_array())
                                                    .map(|arr| arr.iter().filter_map(|p| {
                                                        let k = p.get("public_key").and_then(|v| v.as_str())?.to_string();
                                                        let n = p.get("display_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                        Some((k, n))
                                                    }).collect())
                                                    .unwrap_or_default();
                                                // Attach the roster onto the matching text channel by id
                                                // (channel_list already created it). Fallback: stub it.
                                                if let Some(c) = state.gui_state.chat_channels.iter_mut()
                                                    .find(|c| c.id == vc_id)
                                                {
                                                    c.voice_enabled = true;
                                                    c.voice_participants = roster;
                                                } else if !vc_name.is_empty() {
                                                    state.gui_state.chat_channels.push(
                                                        crate::gui::ChatChannel {
                                                            id: vc_id,
                                                            name: vc_name,
                                                            description: String::new(),
                                                            category: "Text".to_string(),
                                                            voice_joined: false,
                                                            voice_enabled: true,
                                                            read_only: false,
                                                            federated: false,
                                                            voice_participants: roster,
                                                        },
                                                    );
                                                }
                                            }
                                            // Phase C (v0.492): if we are joined to a
                                            // voice room, dial the incumbents present in
                                            // our first post-join roster (the web's
                                            // "newcomer offers, incumbents wait" rule).
                                            // Later joiners will offer to us instead.
                                            if let Some(room_id) = state.gui_state.voice_active_room.clone() {
                                                if !state.gui_state.voice_incumbents_captured {
                                                    let my_key = state.gui_state.profile_public_key.clone();
                                                    let mut me_present = false;
                                                    let mut incumbents: Vec<String> = Vec::new();
                                                    for vc in channels {
                                                        let id = vc.get("id").and_then(|v| v.as_str());
                                                        if id != Some(room_id.as_str()) { continue; }
                                                        if let Some(arr) = vc.get("participants").and_then(|v| v.as_array()) {
                                                            for p in arr {
                                                                if let Some(k) = p.get("public_key").and_then(|v| v.as_str()) {
                                                                    if k == my_key { me_present = true; }
                                                                    else { incumbents.push(k.to_string()); }
                                                                }
                                                            }
                                                        }
                                                    }
                                                    // Only act on the roster that lists US (post-join),
                                                    // so we capture the correct incumbent set.
                                                    if me_present {
                                                        state.gui_state.voice_incumbents_captured = true;
                                                        crate::debug::push_debug(format!(
                                                            "Voice: in room {}, dialing {} incumbent(s)", room_id, incumbents.len()
                                                        ));
                                                        if let Some(ref webrtc) = state.gui_state.webrtc {
                                                            for peer in &incumbents {
                                                                webrtc.offer_to_voice(peer.clone(), room_id.clone());
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Some("profile_data") => {
                                        // Our own profile data from the server
                                        if let Some(name) = val.get("name").and_then(|v| v.as_str()) {
                                            state.gui_state.profile_name = name.to_string();
                                        }
                                        if let Some(bio) = val.get("bio").and_then(|v| v.as_str()) {
                                            state.gui_state.profile_bio = bio.to_string();
                                        }
                                        if let Some(avatar) = val.get("avatar_url").and_then(|v| v.as_str()) {
                                            state.gui_state.profile_network_avatar = avatar.to_string();
                                        }
                                        log::info!("Received profile data from server");
                                    }
                                    Some("sync_data") | Some("sync_ack") | Some("vault_sync") => {
                                        // Vault sync messages - handle silently, don't display in chat
                                        crate::debug::push_debug("Vault sync message received (hidden)");
                                    }
                                    Some("follow_list") => {
                                        if let Some(following) = val.get("following").and_then(|v| v.as_array()) {
                                            let follow_keys: Vec<String> = following.iter()
                                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                                .collect();
                                            state.gui_state.chat_friends = state.gui_state.chat_users.iter()
                                                .filter(|u| follow_keys.contains(&u.public_key))
                                                .cloned()
                                                .collect();
                                            log::info!("Follow list received: {} friends matched from {} keys", state.gui_state.chat_friends.len(), follow_keys.len());
                                        }
                                    }
                                    Some("dm_list") => {
                                        if let Some(conversations) = val.get("conversations").and_then(|v| v.as_array()) {
                                            state.gui_state.chat_dms.clear();
                                            for conv in conversations {
                                                let partner_name = conv.get("partner_name")
                                                    .or_else(|| conv.get("name"))
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("Unknown")
                                                    .to_string();
                                                let partner_key = conv.get("partner_key")
                                                    .or_else(|| conv.get("key"))
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let last_message = conv.get("last_message")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let timestamp = conv.get("timestamp")
                                                    .and_then(|v| v.as_u64())
                                                    .map(crate::gui::pages::chat::format_timestamp)
                                                    .unwrap_or_default();
                                                let unread = conv.get("unread")
                                                    .and_then(|v| v.as_bool())
                                                    .unwrap_or(false);
                                                state.gui_state.chat_dms.push(crate::gui::ChatDm {
                                                    user_name: partner_name,
                                                    user_key: partner_key,
                                                    last_message,
                                                    timestamp,
                                                    unread,
                                                });
                                            }
                                            log::info!("DM list received: {} conversations", state.gui_state.chat_dms.len());
                                        }
                                    }
                                    Some("group_list") => {
                                        if let Some(groups) = val.get("groups").and_then(|v| v.as_array()) {
                                            state.gui_state.chat_groups.clear();
                                            for g in groups {
                                                let name = g.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let id = g.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                // Groups render like servers: an expandable header with
                                                // nested channels. The channel id is `group:<id>` so the
                                                // send path routes correctly (see chat.rs ~1609). When
                                                // server-side multi-channel support for groups lands,
                                                // additional channels get ids like `group:<id>:<name>`.
                                                let active_id = format!("group:{}", id);
                                                state.gui_state.chat_groups.push(crate::gui::ChatGroup {
                                                    name: name.clone(),
                                                    id: id.clone(),
                                                    member_count: 0,
                                                    channels: vec![crate::gui::ChatChannel {
                                                        id: active_id,
                                                        name: "general".to_string(),
                                                        description: String::new(),
                                                        category: "Text".to_string(),
                                                        voice_joined: false,
                                                        // Group channels are voice-capable by default,
                                                        // matching server channels. Actual voice routing
                                                        // still needs server multi-channel support.
                                                        voice_enabled: true,
                                                        read_only: false,
                                                        federated: false,
                                                        voice_participants: Vec::new(),
                                                    }],
                                                    collapsed: false,
                                                });
                                            }
                                            log::info!("Group list received: {} groups", state.gui_state.chat_groups.len());
                                        }
                                    }
                                    Some("dm") => {
                                        // Incoming DM message
                                        let from_key = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let from_name = val.get("from_name").and_then(|v| v.as_str()).unwrap_or("Anonymous").to_string();
                                        let raw_content = val.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let ts = val.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                                        let encrypted = val.get("encrypted").and_then(|v| v.as_bool()).unwrap_or(false);
                                        let nonce = val.get("nonce").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        // Determine partner key: "from us" means the from_name matches our display name
                                        // (we may have multiple keys all registered to the same name).
                                        let is_from_me = from_key == state.gui_state.profile_public_key
                                            || (!state.gui_state.user_name.is_empty() && from_name == state.gui_state.user_name);
                                        let partner = if is_from_me {
                                            val.get("to").and_then(|v| v.as_str()).unwrap_or("").to_string()
                                        } else {
                                            from_key.clone()
                                        };
                                        let content = decrypt_dm_if_encrypted(
                                            &raw_content, encrypted, &nonce, &partner, &state.gui_state,
                                        );
                                        let dm_channel = format!("dm:{}", partner);
                                        state.gui_state.chat_messages.push(crate::gui::ChatMessage {
                                            sender_name: from_name,
                                            sender_key: from_key,
                                            content,
                                            timestamp: crate::gui::pages::chat::format_timestamp(ts),
                                            timestamp_ms: ts,
                                            channel: dm_channel,
                                            ..Default::default()
                                        });
                                        while state.gui_state.chat_messages.len() > 200 {
                                            state.gui_state.chat_messages.remove(0);
                                        }
                                    }
                                    Some("dm_history") => {
                                        // DM conversation history
                                        let partner = val.get("partner").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let dm_channel = format!("dm:{}", partner);
                                        // Clear existing DM messages for this partner
                                        state.gui_state.chat_messages.retain(|m| m.channel != dm_channel);
                                        if let Some(msgs) = val.get("messages").and_then(|v| v.as_array()) {
                                            let mut decrypted_count = 0;
                                            let mut total = 0;
                                            for m in msgs {
                                                total += 1;
                                                let from_key = m.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let from_name = m.get("from_name").and_then(|v| v.as_str()).unwrap_or("Anonymous").to_string();
                                                let raw_content = m.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let ts = m.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                                                let encrypted = m.get("encrypted").and_then(|v| v.as_bool()).unwrap_or(false);
                                                let nonce = m.get("nonce").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                // "From us" matches on name too (account may have multiple keys all registered under same name)
                                                let is_from_me = from_key == state.gui_state.profile_public_key
                                                    || (!state.gui_state.user_name.is_empty() && from_name == state.gui_state.user_name);
                                                let peer_key = if is_from_me {
                                                    partner.clone()
                                                } else {
                                                    from_key.clone()
                                                };
                                                let content = decrypt_dm_if_encrypted(
                                                    &raw_content, encrypted, &nonce, &peer_key, &state.gui_state,
                                                );
                                                if encrypted && content != raw_content {
                                                    decrypted_count += 1;
                                                }
                                                state.gui_state.chat_messages.push(crate::gui::ChatMessage {
                                                    sender_name: from_name,
                                                    sender_key: from_key,
                                                    content,
                                                    timestamp: crate::gui::pages::chat::format_timestamp(ts),
                                                    timestamp_ms: ts,
                                                    channel: dm_channel.clone(),
                                                    ..Default::default()
                                                });
                                            }
                                            log::info!("DM history for {}: {} messages ({} decrypted)", partner, total, decrypted_count);
                                        }
                                    }
                                    Some("group_msg") => {
                                        // Incoming group message
                                        let group_id = val.get("group_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let from_key = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let from_name = val.get("from_name").and_then(|v| v.as_str()).unwrap_or("Anonymous").to_string();
                                        let content = val.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let ts = val.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                                        let group_channel = format!("group:{}", group_id);
                                        state.gui_state.chat_messages.push(crate::gui::ChatMessage {
                                            sender_name: from_name,
                                            sender_key: from_key,
                                            content,
                                            timestamp: crate::gui::pages::chat::format_timestamp(ts),
                                            timestamp_ms: ts,
                                            channel: group_channel,
                                            ..Default::default()
                                        });
                                        while state.gui_state.chat_messages.len() > 200 {
                                            state.gui_state.chat_messages.remove(0);
                                        }
                                    }
                                    Some("group_history") => {
                                        let group_id = val.get("group_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let group_channel = format!("group:{}", group_id);
                                        // Clear existing messages for this group
                                        state.gui_state.chat_messages.retain(|m| m.channel != group_channel);
                                        if let Some(msgs) = val.get("messages").and_then(|v| v.as_array()) {
                                            for m in msgs {
                                                let from_key = m.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let from_name = m.get("from_name").and_then(|v| v.as_str()).unwrap_or("Anonymous").to_string();
                                                let content = m.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let ts = m.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                                                state.gui_state.chat_messages.push(crate::gui::ChatMessage {
                                                    sender_name: from_name,
                                                    sender_key: from_key,
                                                    content,
                                                    timestamp: crate::gui::pages::chat::format_timestamp(ts),
                                                    timestamp_ms: ts,
                                                    channel: group_channel.clone(),
                                                    ..Default::default()
                                                });
                                            }
                                            log::info!("Group history for {}: {} messages", group_id, msgs.len());
                                        }
                                    }
                                    Some("reaction") => {
                                        // Single reaction: target_from + target_timestamp + emoji + from
                                        let target_from = val.get("target_from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let target_ts = val.get("target_timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                                        let emoji = val.get("emoji").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        if !emoji.is_empty() && target_ts > 0 {
                                            for msg in state.gui_state.chat_messages.iter_mut() {
                                                if msg.sender_key == target_from && msg.timestamp_ms == target_ts {
                                                    let entry = msg.reactions.entry(emoji.clone()).or_insert_with(Vec::new);
                                                    // Toggle: if already reacted, remove. Otherwise add.
                                                    if let Some(idx) = entry.iter().position(|k| k == &from) {
                                                        entry.remove(idx);
                                                    } else {
                                                        entry.push(from.clone());
                                                    }
                                                    if entry.is_empty() {
                                                        msg.reactions.remove(&emoji);
                                                    }
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                    Some("reactions_sync") => {
                                        // Bulk sync: array of {target_from, target_timestamp, emoji, from}.
                                        if let Some(arr) = val.get("reactions").and_then(|v| v.as_array()) {
                                            for r in arr {
                                                let target_from = r.get("target_from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let target_ts = r.get("target_timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                                                let emoji = r.get("emoji").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let from = r.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                if emoji.is_empty() || target_ts == 0 { continue; }
                                                for msg in state.gui_state.chat_messages.iter_mut() {
                                                    if msg.sender_key == target_from && msg.timestamp_ms == target_ts {
                                                        let entry = msg.reactions.entry(emoji.clone()).or_insert_with(Vec::new);
                                                        if !entry.contains(&from) {
                                                            entry.push(from.clone());
                                                        }
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Some("edit") => {
                                        // Edited message broadcast — find by sender + timestamp, replace content.
                                        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let ts = val.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                                        let new_content = val.get("new_content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        for msg in state.gui_state.chat_messages.iter_mut() {
                                            if msg.sender_key == from && msg.timestamp_ms == ts {
                                                msg.content = new_content;
                                                break;
                                            }
                                        }
                                    }
                                    Some("delete") => {
                                        // v0.281.0: deletion broadcast (own delete OR admin/mod
                                        // moderation). Drop the matching message from the local
                                        // view by sender_key + timestamp_ms. We don't try to be
                                        // clever about preserving thread context — pin/reaction
                                        // state for an absent message just lingers harmlessly until
                                        // the next channel refetch.
                                        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let ts = val.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                                        if !from.is_empty() && ts > 0 {
                                            state.gui_state.chat_messages.retain(|m| !(m.sender_key == from && m.timestamp_ms == ts));
                                        }
                                    }
                                    Some("message_deleted") => {
                                        // v0.282.0: admin/mod deletion via DeleteById (broadcast
                                        // by relay when web admin uses the by-id path; native
                                        // admins use the simpler `delete` arm above). Payload
                                        // carries `from` + `timestamp` for client-side removal —
                                        // the message_id is for the web's DOM-keyed approach,
                                        // we use (sender_key, timestamp_ms) like everywhere else
                                        // on native.
                                        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let ts = val.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                                        if !from.is_empty() && ts > 0 {
                                            state.gui_state.chat_messages.retain(|m| !(m.sender_key == from && m.timestamp_ms == ts));
                                        }
                                    }
                                    Some("typing") => {
                                        // v0.282.0: typing indicator broadcast. Insert/refresh the
                                        // sender's entry; the renderer prunes anything older than
                                        // 3 seconds (matches web's auto-clear). Skip our own typing
                                        // event — the relay echoes broadcasts to all sockets, so
                                        // we'd otherwise see "Shaostoul is typing…" every time we
                                        // touched the input on a different tab.
                                        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        if from.is_empty() || from == state.gui_state.profile_public_key {
                                            // Continue silently — own echoes are noise.
                                        } else {
                                            let from_name = val.get("from_name").and_then(|v| v.as_str())
                                                .unwrap_or_else(|| {
                                                    // Fallback: look up display name in the user list.
                                                    // Empty string yields "user" downstream when nothing matches.
                                                    ""
                                                })
                                                .to_string();
                                            let display = if from_name.is_empty() {
                                                state.gui_state.chat_users.iter()
                                                    .find(|u| u.public_key == from)
                                                    .map(|u| u.name.clone())
                                                    .unwrap_or_else(|| "Someone".to_string())
                                            } else {
                                                from_name
                                            };
                                            state.gui_state.chat_typing_users.insert(
                                                from,
                                                (display, std::time::Instant::now()),
                                            );
                                        }
                                    }
                                    // v0.283.0: voice_room_signal / webrtc_signal stubs.
                                    // The relay broadcasts these for active voice rooms but
                                    // native has no WebRTC stack yet (the channel-list voice
                                    // icon's click handler is a TODO in chat.rs:1060). Stubs
                                    // keep the dispatcher exhaustive so future arms can detect
                                    // unknown types via the catch-all without false positives.
                                    // Implementing real voice means adding webrtc-rs + audio
                                    // capture/playback + mute/deafen UI — weeks of work,
                                    // tracked separately, not a propagation bug.
                                    #[cfg(feature = "native")]
                                    Some("voice_room_signal") => {
                                        // Phase C: inbound voice-room signaling (offer /
                                        // answer / ice / new_participant) relayed to us.
                                        // Route into the WebRTC manager's voice path. Note
                                        // `data` here is a JSON OBJECT (the browser
                                        // RTCSessionDescription / candidate shape), unlike
                                        // the webrtc_signal path where it is a string.
                                        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let room_id = val.get("room_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let signal_type = val.get("signal_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let data = val.get("data").cloned().unwrap_or(serde_json::Value::Null);
                                        if !from.is_empty() && !signal_type.is_empty() {
                                            if let Some(ref webrtc) = state.gui_state.webrtc {
                                                webrtc.submit_voice_signal(from, room_id, signal_type, data);
                                            }
                                        }
                                    }
                                    Some("voice_call") | Some("voice_room") | Some("voice_room_update") => {
                                        // Control/legacy voice messages: not consumed by the
                                        // native client (join/leave are outbound; the roster
                                        // arrives via voice_channel_list).
                                    }
                                    #[cfg(feature = "native")]
                                    Some("webrtc_signal") => {
                                        // P2P DataChannel signaling (offer/answer/ICE) from a
                                        // peer, relayed to us. Route it into the WebRTC
                                        // manager (lazily started after WS connect, just
                                        // below). The relay set `from` to the authenticated
                                        // sender key; `data` is a JSON string per contract.
                                        let from = val.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let signal_type = val.get("signal_type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let data = val.get("data").cloned().unwrap_or(serde_json::Value::Null);
                                        if !from.is_empty() && !signal_type.is_empty() {
                                            if let Some(ref webrtc) = state.gui_state.webrtc {
                                                webrtc.submit_signal(from, signal_type, data);
                                            }
                                        }
                                    }
                                    Some("federated_chat") => {
                                        // v0.282.0: chat from a federated peer server. Display
                                        // alongside local chat in the same channel, prefixed by
                                        // the originating server's name so users can tell where
                                        // it came from. The relay only delivers federated_chat
                                        // for channels it's federating, so we don't need a
                                        // per-channel federation flag client-side.
                                        let channel = val.get("channel").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let content = val.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let ts = val.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                                        let from_name = val.get("from_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let server_name = val.get("server_name").and_then(|v| v.as_str()).unwrap_or("federated").to_string();
                                        let server_id = val.get("server_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        if !content.is_empty() && ts > 0 {
                                            let ts_str = {
                                                let secs = ts / 1000;
                                                let h = (secs / 3600) % 24;
                                                let m = (secs / 60) % 60;
                                                format!("{:02}:{:02}", h, m)
                                            };
                                            state.gui_state.chat_messages.push(crate::gui::ChatMessage {
                                                // Tag the displayed name with the origin server
                                                // so federated messages are visually distinct.
                                                // E.g. "Alice (other-server)".
                                                sender_name: format!("{} ({})", from_name, server_name),
                                                // sender_key uses the server_id so reactions/replies
                                                // can target the federated origin (the relay won't
                                                // honor cross-server reactions today, but the field
                                                // shape is preserved for forward compat).
                                                sender_key: server_id,
                                                content,
                                                timestamp: ts_str,
                                                timestamp_ms: ts,
                                                channel,
                                                reactions: std::collections::HashMap::new(),
                                                reply_to: None,
                                            });
                                        }
                                    }
                                    Some("search_results") => {
                                        // Server-returned search results. Populate the search modal.
                                        if let Some(results) = val.get("results").and_then(|v| v.as_array()) {
                                            state.gui_state.chat_search_results.clear();
                                            for r in results {
                                                let channel = r.get("channel").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let sender_name = r.get("from_name").and_then(|v| v.as_str())
                                                    .or_else(|| r.get("from").and_then(|v| v.as_str()))
                                                    .unwrap_or("Anonymous").to_string();
                                                let content = r.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                let timestamp_ms = r.get("timestamp").and_then(|v| v.as_u64()).unwrap_or(0);
                                                state.gui_state.chat_search_results.push(crate::gui::ChatSearchResult {
                                                    channel, sender_name, content, timestamp_ms,
                                                });
                                            }
                                        }
                                    }
                                    Some("pins_sync") => {
                                        // Replace the channel's pin list.
                                        let channel = val.get("channel").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        if let Some(arr) = val.get("pins").and_then(|v| v.as_array()) {
                                            let pins: Vec<crate::gui::ChatPin> = arr.iter().map(|p| crate::gui::ChatPin {
                                                from_key: p.get("from_key").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                from_name: p.get("from_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                content: p.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                original_timestamp: p.get("original_timestamp").and_then(|v| v.as_u64()).unwrap_or(0),
                                                pinned_by: p.get("pinned_by").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                pinned_at: p.get("pinned_at").and_then(|v| v.as_u64()).unwrap_or(0),
                                            }).collect();
                                            state.gui_state.chat_pins.insert(channel, pins);
                                        }
                                    }
                                    Some("pin_added") => {
                                        let channel = val.get("channel").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        if let Some(p) = val.get("pin") {
                                            let pin = crate::gui::ChatPin {
                                                from_key: p.get("from_key").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                from_name: p.get("from_name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                content: p.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                original_timestamp: p.get("original_timestamp").and_then(|v| v.as_u64()).unwrap_or(0),
                                                pinned_by: p.get("pinned_by").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                                pinned_at: p.get("pinned_at").and_then(|v| v.as_u64()).unwrap_or(0),
                                            };
                                            state.gui_state.chat_pins.entry(channel).or_insert_with(Vec::new).push(pin);
                                        }
                                    }
                                    Some("pin_removed") => {
                                        let channel = val.get("channel").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                        let index = val.get("index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                        if let Some(pins) = state.gui_state.chat_pins.get_mut(&channel) {
                                            if index < pins.len() {
                                                pins.remove(index);
                                            }
                                        }
                                    }
                                    Some("member_joined") => {
                                        log::debug!("Received server message type: member_joined");
                                    }
                                    Some("member_left") => {
                                        // Server-broadcast when a member is kicked / banned /
                                        // leaves voluntarily. Prune them from the live user list
                                        // + DMs + friends so the sidebar updates immediately.
                                        if let Some(pk) = val.get("public_key").and_then(|v| v.as_str()) {
                                            let pk = pk.to_string();
                                            state.gui_state.chat_users.retain(|u| u.public_key != pk);
                                            state.gui_state.chat_friends.retain(|f| f.public_key != pk);
                                            state.gui_state.chat_dms.retain(|d| d.user_key != pk);
                                            // If the user modal is open for this user, close it.
                                            if state.gui_state.chat_user_modal_key == pk {
                                                state.gui_state.chat_user_modal_open = false;
                                            }
                                            log::info!("Member left/kicked/banned: {}", &pk[..pk.len().min(16)]);
                                        }
                                    }
                                    Some("task_list_response") => {
                                        // Task list response from the WebSocket task_list request
                                        if let Some(tasks) = val.get("tasks").and_then(|v| v.as_array()) {
                                            state.gui_state.tasks.clear();
                                            for task in tasks {
                                                let id = task.get("id")
                                                    .and_then(|v| v.as_u64())
                                                    .unwrap_or(0) as u32;
                                                let title = task.get("title")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let description = task.get("description")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let status_str = task.get("status")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("todo");
                                                let status = match status_str {
                                                    "in_progress" => crate::gui::TaskStatus::InProgress,
                                                    "done" => crate::gui::TaskStatus::Done,
                                                    _ => crate::gui::TaskStatus::Todo,
                                                };
                                                let priority_str = task.get("priority")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("medium");
                                                let priority = match priority_str {
                                                    "low" => crate::gui::TaskPriority::Low,
                                                    "high" => crate::gui::TaskPriority::High,
                                                    "critical" => crate::gui::TaskPriority::Critical,
                                                    _ => crate::gui::TaskPriority::Medium,
                                                };
                                                let assignee = task.get("assignee")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let labels: Vec<String> = task.get("labels")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .split(',')
                                                    .filter(|s| !s.is_empty())
                                                    .map(|s| s.trim().to_string())
                                                    .collect();
                                                state.gui_state.tasks.push(
                                                    crate::gui::GuiTask { id, title, description, priority, status, assignee, labels },
                                                );
                                                if id >= state.gui_state.task_next_id {
                                                    state.gui_state.task_next_id = id + 1;
                                                }
                                            }
                                            log::info!("Received {} tasks from server (task_list_response)", state.gui_state.tasks.len());
                                        }
                                    }
                                    Some("name_taken") => {
                                        let msg = val.get("message").and_then(|v| v.as_str()).unwrap_or("Name taken");
                                        log::warn!("Name taken: {}. Disconnecting and reconnecting with unique name.", msg);
                                        crate::debug::push_debug(format!("Name taken, reconnecting: {}", msg));

                                        // Generate a fallback name: "DesktopUser_XXXX"
                                        let suffix: u16 = (std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .subsec_nanos() % 10000) as u16;
                                        let fallback = format!("DesktopUser_{:04}", suffix);
                                        state.gui_state.profile_name = fallback.clone();
                                        // Persist the fallback as the user_name + save the
                                        // config so the NEXT launch reuses this name (relay
                                        // accepts re-registration from the same key, so no
                                        // collision). Previously, profile_name was set but
                                        // user_name stayed at the conflicting value, so each
                                        // boot tried "Shaostoul" → name_taken → a brand-new
                                        // DesktopUser_NNNN → permanent server-side drip
                                        // (operator reported 6+ entries on one key).
                                        // The user can rename back to a preferred name from
                                        // Settings once the conflicting registration clears.
                                        state.gui_state.user_name = fallback.clone();
                                        crate::config::AppConfig::from_gui_state(&state.gui_state).save();

                                        // Disconnect current connection
                                        if let Some(ref mut client) = state.gui_state.ws_client {
                                            client.disconnect();
                                        }
                                        state.gui_state.ws_client = None;

                                        // Reconnect with new name (full fresh handshake so
                                        // server sends channel_list, dm_list, group_list, etc.)
                                        let url = state.gui_state.server_url.clone();
                                        let ws_url = url.replace("https://", "wss://").replace("http://", "ws://");
                                        let ws_url = format!("{}/ws", ws_url.trim_end_matches('/'));
                                        // Full-PQ: keep advertising our Kyber key on the
                                        // name-collision reconnect — the 3-arg connect()
                                        // sent empty kyber, which silently broke DMs for
                                        // any DesktopUser-fallback session.
                                        let new_client = crate::net::ws_client::WsClient::connect_with_kyber(&ws_url, &fallback, &state.gui_state.profile_public_key, &state.gui_state.kyber_public_b64);
                                        state.gui_state.ws_client = Some(new_client);
                                        state.gui_state.ws_status = format!("Reconnecting as {}...", fallback);
                                        log::info!("Reconnecting as: {}", fallback);
                                    }
                                    Some("task_list") => {
                                        if let Some(tasks) = val.get("tasks").and_then(|v| v.as_array()) {
                                            state.gui_state.tasks.clear();
                                            for task in tasks {
                                                let id = task.get("id")
                                                    .and_then(|v| v.as_u64())
                                                    .unwrap_or(0) as u32;
                                                let title = task.get("title")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let description = task.get("description")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let status_str = task.get("status")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("todo");
                                                let status = match status_str {
                                                    "in_progress" => crate::gui::TaskStatus::InProgress,
                                                    "done" => crate::gui::TaskStatus::Done,
                                                    _ => crate::gui::TaskStatus::Todo,
                                                };
                                                let priority_str = task.get("priority")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("medium");
                                                let priority = match priority_str {
                                                    "low" => crate::gui::TaskPriority::Low,
                                                    "high" => crate::gui::TaskPriority::High,
                                                    "critical" => crate::gui::TaskPriority::Critical,
                                                    _ => crate::gui::TaskPriority::Medium,
                                                };
                                                let assignee = task.get("assignee")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let labels: Vec<String> = task.get("labels")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .split(',')
                                                    .filter(|s| !s.is_empty())
                                                    .map(|s| s.trim().to_string())
                                                    .collect();
                                                state.gui_state.tasks.push(
                                                    crate::gui::GuiTask { id, title, description, priority, status, assignee, labels },
                                                );
                                                if id >= state.gui_state.task_next_id {
                                                    state.gui_state.task_next_id = id + 1;
                                                }
                                            }
                                            log::info!("Received {} tasks from server", state.gui_state.tasks.len());
                                        }
                                    }
                                    Some("private") => {
                                        // Private server-to-user message (rate limit, errors, command responses)
                                        if let Some(msg) = val.get("message").and_then(|v| v.as_str()) {
                                            crate::debug::push_debug(format!("Private: {}", msg));
                                            // Multiplayer (v0.472): the game_welcome (our player id +
                                            // world snapshot) arrives as a private __game__ message.
                                            if let Some(payload) = msg.strip_prefix("__game__:") {
                                                let payload = payload.to_string();
                                                route_game_message(state, &payload);
                                                continue;
                                            }
                                            // Filter out profile validation noise (not relevant to chat)
                                            let is_profile_noise = msg.contains("Profile URL")
                                                || msg.contains("must start with https://")
                                                || msg.starts_with("__sync_data__")
                                                || msg == "sync_ack";
                                            if !is_profile_noise {
                                                // Show as system message in chat
                                                let now_ms = std::time::SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)
                                                    .unwrap_or_default()
                                                    .as_millis() as u64;
                                                state.gui_state.chat_messages.push(
                                                    crate::gui::ChatMessage {
                                                        sender_name: "System".to_string(),
                                                        sender_key: String::new(),
                                                        content: msg.to_string(),
                                                        timestamp: crate::gui::pages::chat::format_timestamp(now_ms),
                                                        timestamp_ms: now_ms,
                                                        // Don't leak into an open P2P group / DM (it'd vanish on reload).
                                                        channel: crate::gui::pages::chat::notice_channel(&state.gui_state.chat_active_channel),
                                                        ..Default::default()
                                                    },
                                                );
                                            }
                                        }
                                    }
                                    _ => {
                                        // Log unhandled message types to debug console
                                        let msg_type = val.get("type").and_then(|t| t.as_str()).unwrap_or("unknown");
                                        crate::debug::push_debug(format!("Unhandled WS type: {}", msg_type));
                                    }
                                }
                            }
                        }
                    }

                    // ── Drop dead WebSocket client and start reconnect timer ──
                    if ws_dropped {
                        state.gui_state.ws_client = None;
                        // Tear down the WebRTC manager too: its signaling rides
                        // the WS, so without a live WS it can't negotiate. The
                        // thread stops when its handle (and thus the command
                        // sender) drops. It re-starts lazily on reconnect.
                        #[cfg(feature = "native")]
                        {
                            state.gui_state.webrtc = None;
                        }
                        // Force the Banned-users panel to re-request after a
                        // reconnect (the relay only sends it on demand). The
                        // cached list itself is harmless to keep until then.
                        state.gui_state.chat_banned_requested = false;
                        state.gui_state.chat_muted_requested = false;
                        // Same for the Game Admin game-ban list (v0.474).
                        state.gui_state.game_bans_requested = false;
                        if !state.gui_state.ws_manually_disconnected {
                            log::info!("WebSocket disconnected, will reconnect in {}s (attempt {})",
                                state.gui_state.ws_reconnect_delay as u32,
                                state.gui_state.ws_reconnect_attempts + 1);
                            state.gui_state.ws_reconnect_timer = state.gui_state.ws_reconnect_delay;
                            state.gui_state.ws_status = format!("Reconnecting in {}s...",
                                state.gui_state.ws_reconnect_delay as u32);
                        } else {
                            state.gui_state.ws_status = "Disconnected".to_string();
                        }
                    }

                    // ── WebSocket auto-reconnect with exponential backoff ──
                    if state.gui_state.ws_client.is_none()
                        && !state.gui_state.ws_manually_disconnected
                        && state.gui_state.ws_reconnect_timer > 0.0
                    {
                        state.gui_state.ws_reconnect_timer -= dt;
                        let secs_left = state.gui_state.ws_reconnect_timer.ceil() as u32;
                        state.gui_state.ws_status = format!("Reconnecting in {}s...", secs_left.max(1));

                        if state.gui_state.ws_reconnect_timer <= 0.0 {
                            // Attempt reconnect
                            let ws_url = crate::gui::pages::chat::derive_ws_url(&state.gui_state.server_url);
                            let name = state.gui_state.user_name.clone();
                            let pubkey = if state.gui_state.profile_public_key.is_empty() {
                                crate::gui::pages::chat::generate_random_hex_key()
                            } else {
                                state.gui_state.profile_public_key.clone()
                            };
                            log::info!("Attempting WebSocket reconnect (attempt {})", state.gui_state.ws_reconnect_attempts + 1);
                            // Full-PQ: backoff reconnect must also carry the
                            // Kyber key, else DMs break after any drop.
                            state.gui_state.ws_client = Some(
                                crate::net::ws_client::WsClient::connect_with_kyber(&ws_url, &name, &pubkey, &state.gui_state.kyber_public_b64),
                            );
                            state.gui_state.ws_reconnect_attempts += 1;
                            // Clear the rate-limit guard now that we are actually retrying: if this
                            // attempt is throttled again, the system handler re-arms it. (v0.544)
                            state.gui_state.ws_rate_limited = false;
                            // Exponential backoff: 5s -> 10s -> 20s -> 40s -> 60s (max)
                            state.gui_state.ws_reconnect_delay = (state.gui_state.ws_reconnect_delay * 2.0).min(60.0);
                            state.gui_state.ws_status = "Reconnecting...".to_string();
                        }
                    }

                    // ── Reset backoff on successful connection ──
                    // Skipped while rate-limited (v0.544): the socket OPENS before the identify that
                    // gets throttled, so resetting on mere connection would clobber the 65s back-off
                    // and loop straight back into the limit.
                    if !state.gui_state.ws_rate_limited
                        && state.gui_state.ws_client.as_ref().map_or(false, |c| c.is_connected())
                    {
                        if state.gui_state.ws_reconnect_attempts > 0 {
                            log::info!("WebSocket reconnected after {} attempts", state.gui_state.ws_reconnect_attempts);
                        }
                        state.gui_state.ws_reconnect_delay = 5.0;
                        state.gui_state.ws_reconnect_attempts = 0;
                        state.gui_state.ws_reconnect_timer = 0.0;
                    }

                    // ── Native WebRTC DataChannel P2P (increment 1) ──
                    // Lazily start the manager once the WS is connected AND we
                    // have our pubkey hex (it's the identity peers know us by
                    // and the value the offerer rule compares). Then, each
                    // frame, relay its outbound signaling to the WS and surface
                    // its events (channel open / inbound frames) as debug lines.
                    #[cfg(feature = "native")]
                    {
                        let ws_connected = state
                            .gui_state
                            .ws_client
                            .as_ref()
                            .map_or(false, |c| c.is_connected());
                        let have_key = !state.gui_state.profile_public_key.is_empty();

                        // Lazy start.
                        if ws_connected && have_key && state.gui_state.webrtc.is_none() {
                            let my_key = state.gui_state.profile_public_key.clone();
                            let handle = crate::net::webrtc::WebrtcManager::start(my_key);
                            state.gui_state.webrtc = Some(handle);
                            crate::debug::push_debug("WebRTC P2P manager started");
                        }

                        // Per-frame pump: relay outbound webrtc_signal JSON to
                        // the WS, and drain events into the debug console. We
                        // collect first (immutable borrow of webrtc), then act
                        // (the WS send + debug push) to avoid overlapping
                        // borrows of gui_state.
                        if state.gui_state.webrtc.is_some() {
                            // Short, log-friendly form of a long pubkey hex.
                            fn short(k: &str) -> String {
                                if k.len() > 12 { format!("{}…", &k[..12]) } else { k.to_string() }
                            }
                            let (outbound, events) = {
                                let w = state.gui_state.webrtc.as_ref().unwrap();
                                (w.poll_outbound(), w.poll_events())
                            };
                            for json in outbound {
                                if let Some(ref ws) = state.gui_state.ws_client {
                                    ws.send(&json);
                                }
                            }
                            for ev in events {
                                match ev {
                                    crate::net::webrtc::WebrtcEvent::ChannelOpen { peer } => {
                                        crate::debug::push_debug(format!(
                                            "WebRTC: channel OPEN with {}", short(&peer)
                                        ));
                                        // On open, fire the dev test frame if the
                                        // chat page armed one for this peer.
                                        if state.gui_state.webrtc_test_peer.as_deref() == Some(peer.as_str()) {
                                            if let Some(ref w) = state.gui_state.webrtc {
                                                w.send_text(peer.clone(), "native p2p test".to_string());
                                            }
                                            crate::debug::push_debug(format!(
                                                "WebRTC: sent test frame to {}", short(&peer)
                                            ));
                                        }
                                    }
                                    crate::net::webrtc::WebrtcEvent::Frame { peer, text } => {
                                        // inc-2: a frame may be a P2P group-object
                                        // push ({type:"p2p_group_obj", submission}).
                                        // handle_p2p_group_obj verifies + dedups +
                                        // gates + decrypts + renders it, and returns
                                        // true if it consumed the frame (even when it
                                        // legitimately dropped it). Only fall back to
                                        // the inc-1 debug line if it was NOT a group
                                        // obj frame (e.g. the "native p2p test" text).
                                        let consumed = crate::gui::pages::chat::handle_p2p_group_obj(
                                            &mut state.gui_state, &peer, &text,
                                        );
                                        if !consumed {
                                            crate::debug::push_debug(format!(
                                                "WebRTC: frame from {}: {}", short(&peer), text
                                            ));
                                        }
                                    }
                                    crate::net::webrtc::WebrtcEvent::VoiceConnected { peer } => {
                                        // Phase C/D: a voice peer's transport is up. Track it
                                        // so the session pumps our mic Opus to it.
                                        crate::debug::push_debug(format!(
                                            "Voice: CONNECTED with {}", short(&peer)
                                        ));
                                        state.gui_state.voice_connected_peers.insert(peer);
                                    }
                                    crate::net::webrtc::WebrtcEvent::VoiceFrame { peer, opus } => {
                                        // Phase D: inbound Opus from a voice peer -> the voice
                                        // session decodes, mixes, and plays it. Count frames
                                        // for the debug log (~50 frames ~= 1s of speech).
                                        crate::net::voice::push_remote_opus(peer.clone(), opus.clone());
                                        state.gui_state.voice_rx_frames = state.gui_state.voice_rx_frames.wrapping_add(1);
                                        if state.gui_state.voice_rx_frames % 100 == 1 {
                                            crate::debug::push_debug(format!(
                                                "Voice: receiving audio from {} ({} frames)",
                                                short(&peer), state.gui_state.voice_rx_frames
                                            ));
                                        }
                                    }
                                    crate::net::webrtc::WebrtcEvent::Closed { peer } => {
                                        state.gui_state.voice_connected_peers.remove(&peer);
                                        crate::debug::push_debug(format!(
                                            "WebRTC: channel CLOSED with {}", short(&peer)
                                        ));
                                    }
                                }
                            }
                        }
                    }

                    // ── Fetch channel history via HTTP after connecting ──
                    if !state.gui_state.history_fetched
                        && state.gui_state.ws_client.as_ref().map_or(false, |c| c.is_connected())
                        && !state.gui_state.server_url.is_empty()
                    {
                        state.gui_state.history_fetched = true;
                        let base_url = state.gui_state.server_url.trim_end_matches('/').to_string();
                        let channel = state.gui_state.chat_active_channel.clone();
                        let api_url = format!("{}/api/messages?limit=50&channel={}", base_url, channel);
                        match ureq::get(&api_url).call() {
                            Ok(resp) => {
                                if let Ok(body) = resp.into_string() {
                                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&body) {
                                        if let Some(messages) = data.get("messages").and_then(|v| v.as_array()) {
                                            let my_key = state.gui_state.profile_public_key.clone();
                                            let mut fetched = 0usize;
                                            let mut skipped = 0usize;
                                            for msg in messages {
                                                let sender_name = msg.get("sender_name")
                                                    .or_else(|| msg.get("from_name"))
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("Anonymous")
                                                    .to_string();
                                                let sender_key = msg.get("sender_key")
                                                    .or_else(|| msg.get("from"))
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let content = msg.get("content")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let timestamp = msg.get("timestamp")
                                                    .and_then(|v| v.as_u64())
                                                    .unwrap_or(0);
                                                let ch = msg.get("channel")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("general")
                                                    .to_string();
                                                // Dedup: if this is a message WE sent that we already
                                                // local-echoed, skip the server's copy (BUG-035 part 2).
                                                // Match logic mirrors the WS broadcast dedup at line ~1139.
                                                if !my_key.is_empty()
                                                    && sender_key == my_key
                                                    && state.gui_state.chat_sent_timestamps.contains(&timestamp)
                                                {
                                                    state.gui_state.chat_sent_timestamps.retain(|&t| t != timestamp);
                                                    skipped += 1;
                                                    continue;
                                                }
                                                // Robust content dedup (2026-05-20 fix): this fetch
                                                // runs on EVERY reconnect (history_fetched resets on
                                                // disconnect just below), so without checking the
                                                // existing buffer it would re-append every message
                                                // already on screen from the live broadcast — the
                                                // duplication the operator saw. (sender_key,
                                                // timestamp_ms) uniquely identifies a message; skip
                                                // anything we already hold. The chat_sent_timestamps
                                                // path above is a one-shot fast-path that this
                                                // backstops.
                                                if state.gui_state.chat_messages.iter()
                                                    .any(|m| m.sender_key == sender_key && m.timestamp_ms == timestamp)
                                                {
                                                    skipped += 1;
                                                    continue;
                                                }
                                                state.gui_state.chat_messages.push(
                                                    crate::gui::ChatMessage {
                                                        sender_name,
                                                        sender_key,
                                                        content,
                                                        timestamp: crate::gui::pages::chat::format_timestamp(timestamp),
                                                        timestamp_ms: timestamp,
                                                        channel: ch,
                                                        ..Default::default()
                                                    },
                                                );
                                                fetched += 1;
                                            }
                                            log::info!(
                                                "Fetched {} history messages for #{} (skipped {} local-echo dedup)",
                                                fetched, channel, skipped
                                            );
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                log::warn!("Failed to fetch message history: {}", e);
                            }
                        }
                    }

                    // ── Reset history_fetched when disconnected so we re-fetch on reconnect ──
                    if state.gui_state.ws_client.as_ref().map_or(true, |c| !c.is_connected()) {
                        if state.gui_state.history_fetched {
                            state.gui_state.history_fetched = false;
                        }
                    }

                    // Track page before egui frame for cursor grab transitions
                    let page_before_frame = state.gui_state.active_page;

                    // Lazy-load 3D world on first Enter World (code LOD: chat loads fast, 3D loads when needed)
                    if state.gui_state.active_page == GuiPage::None && !state.world_loaded {
                        load_world(state);
                    }

                    // ── Unified character launcher (v0.476) ──
                    // Play asks for the character picker by setting launcher_open_select
                    // + active_page = None. We open the showroom (mode 0) here, AFTER
                    // load_world, so it works the FIRST time (world loads this same frame)
                    // and EVERY later time (world already loaded, so load_world is skipped
                    // -- a gate inside load_world would never fire again). The showroom is
                    // the single unified two-pane character/server surface; the old flat
                    // launcher page is gone.
                    if state.gui_state.launcher_open_select && state.world_loaded {
                        state.gui_state.launcher_open_select = false;
                        // Always land on the character (Home) tab, not a stale server selection.
                        state.gui_state.launcher_selected_kind = crate::gui::LauncherSel::Home;
                        state.gui_state.active_page = GuiPage::None;
                        open_showroom(state, 0); // mode 0 = character select
                    }

                    // Apply a launcher-selected character once the world exists
                    // (v0.474). Finds the local save by its display name and
                    // applies its inventory + skills + name + look to the live
                    // player. Idempotent: re-applying the active home is harmless.
                    if state.world_loaded {
                        if let Some(name) = state.gui_state.launcher_pending_load.take() {
                            let dir = crate::persistence::saves_dir();
                            if let Ok(entries) = std::fs::read_dir(&dir) {
                                for entry in entries.filter_map(|e| e.ok()) {
                                    let path = entry.path();
                                    if path.extension().and_then(|x| x.to_str()) != Some("json") {
                                        continue;
                                    }
                                    if let Ok(save) = crate::persistence::load_world(&path) {
                                        if save.name == name {
                                            crate::save_load::apply_save_to_world(
                                                &mut state.game_world.world,
                                                &save,
                                            );
                                            // Sync the editing copies + rebuild the avatar.
                                            state.gui_state.appearance = save.appearance.clone();
                                            state.gui_state.outfit = save.outfit.clone();
                                            if !save.character_name.is_empty() {
                                                state.gui_state.character_name = save.character_name.clone();
                                            }
                                            state.gui_state.appearance_dirty = true;
                                            state.gui_state.outfit_dirty = true;
                                            log::info!("Launcher: loaded character '{name}'");
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Decide whether to render 3D scene or just a cleared surface
                    let page_active = state.gui_state.active_page != GuiPage::None;
                    let scene_result = if page_active {
                        // UI-only frame: skip 3D render, clear to dark background
                        state.renderer.acquire_surface_cleared(wgpu::Color {
                            r: 0.07,
                            g: 0.07,
                            b: 0.086,
                            a: 1.0,
                        })
                    } else {
                        // In-game: render stars first, then scene objects on top
                        match state.renderer.acquire_surface() {
                            Ok((output, view)) => {
                                // Pass 1: Stars (clear to black + draw star points)
                                if let Some(ref star_r) = state.star_renderer {
                                    star_r.update_camera(&state.renderer.queue, &state.camera);
                                    let mut encoder = state.renderer.device.create_command_encoder(
                                        &wgpu::CommandEncoderDescriptor { label: Some("Star Encoder") },
                                    );
                                    {
                                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                            label: Some("Star Pass"),
                                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                                view: &view,
                                                resolve_target: None,
                                                ops: wgpu::Operations {
                                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                                    store: wgpu::StoreOp::Store,
                                                },
                                            })],
                                            depth_stencil_attachment: None,
                                            ..Default::default()
                                        });
                                        star_r.render_pass(&mut pass);
                                    }
                                    state.renderer.queue.submit(std::iter::once(encoder.finish()));
                                }

                                // Update point lights (up to 8 nearest to camera)
                                {
                                    let cam_pos = state.camera.position;
                                    let mut lights = state.room_lights.clone();
                                    // EMISSIVE surfaces emit light (v0.576): an ENERGY door is a glowing
                                    // field, so add a local point light at it (green unlocked / red
                                    // locked) -- regardless of the GI toggle, since it's a local source.
                                    // (Generalizing to any emissive material -- TVs, lava, emissive walls
                                    // -- is a follow-up; the energy door is the case the operator flagged.)
                                    let dl = state.door_locks.clone();
                                    for (i, (p, _open)) in state.door_panels.iter().enumerate() {
                                        if p.style != "energy" {
                                            continue;
                                        }
                                        let locked = door_locked_now(p, dl.get(i));
                                        let color = if locked { [1.0, 0.25, 0.28] } else { [0.30, 1.0, 0.45] };
                                        let c = p.center;
                                        lights.push((Vec3::new(c.x, c.y + p.size.y * 0.5, c.z), color, 5.0, 4.5));
                                    }
                                    // Sort by distance to camera, take nearest 8
                                    lights.sort_by(|a, b| {
                                        let da = (a.0 - cam_pos).length_squared();
                                        let db = (b.0 - cam_pos).length_squared();
                                        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
                                    });
                                    lights.truncate(8);
                                    state.renderer.set_point_lights(&lights);
                                }

                                // Pass 1.5: celestial bodies (planet + Sun + solar bodies)
                                // with a HUGE far plane, behind the interior, lit by the
                                // REAL Sun direction. (v0.450 bodies, v0.451 lighting)
                                let sun_dir_f = {
                                    let d = state.sun_world_pos.normalize_or_zero();
                                    Vec3::new(d.x as f32, d.y as f32, d.z as f32)
                                };
                                state.renderer.render_celestial_onto(&state.camera, &celestial_objects, sun_dir_f, &view);
                                // Pass 1.6: orbit rings at celestial scale — between the
                                // bodies and the interior so a ring behind a planet is
                                // occluded by that body, and walls then draw over the
                                // rings. AU-scale, so the gameplay far would clip them.
                                // (v0.451; empty in the showroom — build is !showroom-gated.)
                                state.renderer.draw_celestial_lines_onto(&state.camera, &orbit_lines, &view);
                                // Pass 2: Scene objects (LoadOp::Load preserves stars + bodies)
                                state.renderer.render_scene_onto(&state.camera, &all_objects, &view);
                                // Pass 2.5: transparent surfaces (glass windows) blended over
                                // the opaque scene so you can see through them. (v0.456)
                                state.renderer.render_transparent_onto(&state.camera, &transparent_objects, &view);
                                // Pass 2.6: editor gizmos on top (depth off), visible through walls. (v0.560)
                                state.renderer.render_overlay_onto(&state.camera, &overlay_objects, &view);
                                // Pass 2.7: door auto-open rings as constant-width lines (v0.565).
                                state.renderer.draw_lines_onto(&state.camera, &ring_lines, &view);
                                Ok((output, view))
                            }
                            Err(e) => Err(e),
                        }
                    };
                    match scene_result {
                        Ok((surface_texture, view)) => {
                            // Drain global debug log into GuiState each frame
                            {
                                let new_entries = crate::debug::drain_debug_log();
                                state.gui_state.debug_log.extend(new_entries);
                                // Cap at 500 entries
                                while state.gui_state.debug_log.len() > 500 {
                                    state.gui_state.debug_log.remove(0);
                                }
                            }

                            // Run egui frame
                            let raw_input = state.egui_state.take_egui_input(&state.window);
                            let full_output = state.egui_ctx.run(raw_input, |ctx| {
                                // Show RGB nav bar on all pages except None and MainMenu
                                match state.gui_state.active_page {
                                    GuiPage::None | GuiPage::MainMenu => {}
                                    _ => {
                                        escape_menu::draw_nav_bar(ctx, &state.theme, &mut state.gui_state);
                                    }
                                }

                                // Draw active full-screen page
                                match state.gui_state.active_page {
                                    GuiPage::MainMenu => {
                                        main_menu::draw(ctx, &state.theme, &mut state.gui_state);
                                    }
                                    GuiPage::Settings => {
                                        settings::draw(ctx, &mut state.theme, &mut state.gui_state);
                                    }
                                    GuiPage::Inventory => {
                                        inventory::draw(ctx, &state.theme, &mut state.gui_state);
                                    }
                                    GuiPage::Chat => {
                                        chat::draw(ctx, &state.theme, &mut state.gui_state);
                                    }
                                    // Placeholder pages (web versions exist, native coming)
                                    GuiPage::Tasks => tasks::draw(ctx, &state.theme, &mut state.gui_state),
                                    // v0.203.2: GuiPage::Maps routed below to the new Cosmos page.
                                    GuiPage::Market => market::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Profile => profile::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Real => real::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Platform => platform::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Humanity => humanity::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Library => library::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Civilization => civilization::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Calculator => calculator::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Notes => notes::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Calendar => calendar::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Crafting => crafting::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Wallet => wallet::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Guilds => guilds::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Trade => trade::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Files => files::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::BugReport => bugs::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Donate => donate::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Tools => tools::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Studio => studio::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Quests => quests::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Homes => homes::draw(ctx, &state.theme, &mut state.gui_state),
                                    // v0.415.0: Play / Resources / Onboarding arms removed with their pages.
                                    GuiPage::ServerSettings => server_settings::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Identity => identity::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Governance => governance::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Laws => crate::gui::pages::laws::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Recovery => recovery::draw(ctx, &state.theme, &mut state.gui_state),
                                    // v0.197.0: GuiPage::Agents and GuiPage::AiUsage removed.
                                    GuiPage::Cosmos => cosmos::draw(ctx, &state.theme, &mut state.gui_state),
                                    // v0.203.2: GuiPage::Maps now forwards to the new
                                    // Cosmos page. Operator clicked "Maps" in the
                                    // single-row nav (which still listed the OLD
                                    // pages/maps.rs page that was just an empty
                                    // placeholder + dead-code orbit visualization
                                    // since v0.197 dropped the Real/Sim toggle).
                                    // The new Cosmos page IS the universal map.
                                    GuiPage::Maps => cosmos::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Testing => testing::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Browser => browser::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::OverviewReality  => category_overview::draw(ctx, &state.theme, &mut state.gui_state, "reality"),
                                    GuiPage::OverviewSim      => category_overview::draw(ctx, &state.theme, &mut state.gui_state, "sim"),
                                    GuiPage::OverviewTools    => category_overview::draw(ctx, &state.theme, &mut state.gui_state, "tools"),
                                    GuiPage::OverviewSettings => category_overview::draw(ctx, &state.theme, &mut state.gui_state, "settings"),
                                    GuiPage::OverviewDev      => category_overview::draw(ctx, &state.theme, &mut state.gui_state, "dev"),
                                    GuiPage::SettingsAccount       => settings_pages::draw_account(ctx, &mut state.theme, &mut state.gui_state),
                                    GuiPage::SettingsAppearance    => settings_pages::draw_appearance(ctx, &mut state.theme, &mut state.gui_state),
                                    GuiPage::SettingsAnimations    => settings_pages::draw_animations(ctx, &mut state.theme, &mut state.gui_state),
                                    GuiPage::SettingsWidgets       => settings_pages::draw_widgets(ctx, &mut state.theme, &mut state.gui_state),
                                    GuiPage::SettingsNotifications => settings_pages::draw_notifications(ctx, &mut state.theme, &mut state.gui_state),
                                    GuiPage::SettingsWallet        => settings_pages::draw_wallet(ctx, &mut state.theme, &mut state.gui_state),
                                    GuiPage::SettingsAudio         => settings_pages::draw_audio(ctx, &mut state.theme, &mut state.gui_state),
                                    GuiPage::SettingsGraphics      => settings_pages::draw_graphics(ctx, &mut state.theme, &mut state.gui_state),
                                    GuiPage::SettingsControls      => settings_pages::draw_controls(ctx, &mut state.theme, &mut state.gui_state),
                                    GuiPage::SettingsPrivacy       => settings_pages::draw_privacy(ctx, &mut state.theme, &mut state.gui_state),
                                    GuiPage::SettingsData          => settings_pages::draw_data(ctx, &mut state.theme, &mut state.gui_state),
                                    GuiPage::SettingsUpdates       => settings_pages::draw_updates(ctx, &mut state.theme, &mut state.gui_state),
                                    GuiPage::None => {}
                                }

                                // Drain any freshly-decoded chat images into egui textures.
                                state.gui_state.image_cache.poll(ctx);

                                // Universal help modal overlay — draws on top of any page
                                // when state.gui_state.active_help_topic is Some.
                                help_modal::draw(
                                    ctx,
                                    &state.theme,
                                    &state.gui_state.help_registry,
                                    &mut state.gui_state.active_help_topic,
                                );

                                // Full-screen image viewer when a chat image was clicked.
                                crate::gui::widgets::image_cache_view::draw(
                                    ctx,
                                    &state.theme,
                                    &mut state.gui_state,
                                );

                                // Draw HUD when in-game. SKIP it during the showroom AND the
                                // construction editor: the HUD allocates a full-screen Area
                                // (hud.rs) which sits OVER an in-world side panel and eats its
                                // clicks -- the real cause of "panel shows but won't click".
                                // (v0.461; the showroom already skipped it, which is why it
                                // worked and the editor did not.)
                                if state.gui_state.active_page == GuiPage::None
                                    && state.gui_state.show_hud
                                    && !state.gui_state.showroom_active
                                    && !state.gui_state.construction_active
                                {
                                    hud::draw(
                                        ctx,
                                        &state.theme,
                                        &state.gui_state,
                                        state.camera.yaw,
                                        state.camera.view_projection_matrix(),
                                        state.camera.position,
                                    );
                                }
                                // Build-mode CAD dimension overlay (v0.545): wall lengths, corner
                                // angles, live drawing readout. Also shows in play when the dev
                                // overlay is on (v0.547).
                                if state.gui_state.active_page == GuiPage::None
                                    && (state.gui_state.construction_active
                                        || state.gui_state.construction_dev_overlay)
                                    && !state.gui_state.showroom_active
                                {
                                    hud::draw_construction_overlay(
                                        ctx,
                                        &state.theme,
                                        &state.gui_state,
                                        state.camera.view_projection_matrix(),
                                    );
                                }

                                // Character-select showroom panel (v0.441): appearance +
                                // backdrop + Enter, over the orbiting avatar.
                                if state.gui_state.active_page == GuiPage::None
                                    && state.gui_state.showroom_active
                                {
                                    crate::gui::pages::showroom::draw(ctx, &state.theme, &mut state.gui_state);
                                }

                                // Construction editor panel (v0.455): per-wall kinds + height,
                                // rebuilds the home live. Toggled with B.
                                if state.gui_state.active_page == GuiPage::None
                                    && state.gui_state.construction_active
                                {
                                    crate::gui::pages::construction::draw(ctx, &state.theme, &mut state.gui_state);
                                }

                                // Draw chat overlay if visible (only in-game)
                                if state.gui_state.active_page == GuiPage::None && state.gui_state.show_chat {
                                    chat::draw(ctx, &state.theme, &mut state.gui_state);
                                }

                                // Passphrase modal overlay (blocks interaction until resolved)
                                if state.gui_state.passphrase_needed {
                                    crate::gui::pages::passphrase_modal::draw(ctx, &state.theme, &mut state.gui_state);
                                }

                                // Planet info tooltip when targeting a hologram pin
                                if let Some(ref planet_name) = state.targeted_planet {
                                    egui::Area::new(egui::Id::new("planet_info_tooltip"))
                                        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -60.0))
                                        .show(ctx, |ui| {
                                            egui::Frame::popup(ui.style())
                                                .inner_margin(egui::Margin::same(12))
                                                .show(ui, |ui| {
                                                    ui.heading(planet_name);
                                                    ui.separator();
                                                    let (resources, status) = match planet_name.as_str() {
                                                        "Mercury" => ("Iron, Nickel, Silicates", "Unmined"),
                                                        "Venus" => ("CO2, Sulfuric acid, N2", "Hostile atmosphere"),
                                                        "Earth" => ("Water, O2, Biomass, Metals", "Inhabited (8B+ pop)"),
                                                        "Mars" => ("Iron oxide, Water ice, CO2", "Colonization target"),
                                                        "Jupiter" => ("H2, He, Deuterium", "Gas harvesting potential"),
                                                        "Saturn" => ("H2, He, Ring ice, Titan CH4", "Ring mining potential"),
                                                        "Uranus" => ("CH4, H2O, NH3, H2", "Deep ice giant"),
                                                        "Neptune" => ("CH4, H2, He", "Remote ice giant"),
                                                        "Ceres" => ("Water ice, Clays, Salts", "Asteroid belt dwarf"),
                                                        "Pluto" => ("N2 ice, CH4, CO, H2O", "Kuiper belt object"),
                                                        "Haumea" => ("Crystalline ice", "Elongated, fast spinner"),
                                                        "Makemake" => ("CH4, C2H6 ices", "Distant TNO"),
                                                        "Eris" => ("N2, CH4 ices", "Most massive dwarf planet"),
                                                        _ => ("Unknown", "Uncharted"),
                                                    };
                                                    ui.label(format!("Resources: {resources}"));
                                                    ui.label(format!("Status: {status}"));
                                                });
                                        });
                                }

                                // Crosshair (small dot at screen center when in game)
                                if state.gui_state.active_page == GuiPage::None {
                                    let screen = ctx.screen_rect();
                                    let center = screen.center();
                                    let painter = ctx.layer_painter(egui::LayerId::new(
                                        egui::Order::Foreground,
                                        egui::Id::new("crosshair"),
                                    ));
                                    let color = if state.targeted_planet.is_some() {
                                        egui::Color32::from_rgb(100, 200, 255) // highlight blue when targeting
                                    } else {
                                        egui::Color32::from_white_alpha(180)
                                    };
                                    painter.circle_filled(center, 3.0, color);
                                }

                                // Keymap reference overlay while F1 is held (v0.465).
                                if state.gui_state.keymap_visible {
                                    crate::gui::pages::keymap::draw(ctx, &state.theme, &state.gui_state);
                                }
                                // Diagnostics dev-HUD overlays (F2/F3/F4), v0.482.
                                crate::gui::pages::diagnostics::draw(ctx, &state.theme, &state.gui_state);

                                // Draw debug console overlay (F12 toggle, on top of everything)
                                crate::debug::draw_debug_console(
                                    ctx,
                                    &mut state.gui_state.debug_log,
                                    &mut state.gui_state.debug_console_visible,
                                );

                                // Quit requested from main menu
                                if state.gui_state.quit_requested {
                                    event_loop.exit();
                                }
                            });

                            // Handle egui platform output (cursor changes, clipboard, etc.)
                            state.egui_state.handle_platform_output(&state.window, full_output.platform_output);

                            // Tessellate and render egui
                            let paint_jobs = state.egui_ctx.tessellate(
                                full_output.shapes,
                                full_output.pixels_per_point,
                            );

                            // Handle egui texture updates
                            for (id, image_delta) in &full_output.textures_delta.set {
                                state.egui_renderer.update_texture(
                                    &state.renderer.device,
                                    &state.renderer.queue,
                                    *id,
                                    image_delta,
                                );
                            }

                            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                                size_in_pixels: [
                                    state.renderer.surface_size().0,
                                    state.renderer.surface_size().1,
                                ],
                                pixels_per_point: full_output.pixels_per_point,
                            };

                            // Render egui overlay on top of the 3D scene.
                            // Use two encoders: one for buffer updates, one for rendering.
                            // This avoids lifetime issues with wgpu 24's render pass borrows.
                            {
                                let mut encoder = state.renderer.device.create_command_encoder(
                                    &wgpu::CommandEncoderDescriptor {
                                        label: Some("egui Buffer Update"),
                                    },
                                );

                                state.egui_renderer.update_buffers(
                                    &state.renderer.device,
                                    &state.renderer.queue,
                                    &mut encoder,
                                    &paint_jobs,
                                    &screen_descriptor,
                                );

                                state.renderer.queue.submit(std::iter::once(encoder.finish()));
                            }

                            {
                                let mut encoder = state.renderer.device.create_command_encoder(
                                    &wgpu::CommandEncoderDescriptor {
                                        label: Some("egui Render"),
                                    },
                                );

                                // SAFETY: The render pass is created, used, and dropped
                                // within this block before encoder.finish(). The 'static
                                // lifetime on egui_wgpu::Renderer::render() is overly
                                // conservative. The render pass does not actually outlive
                                // the encoder since we drop it before calling finish().
                                let render_pass = unsafe {
                                    std::mem::transmute::<
                                        wgpu::RenderPass<'_>,
                                        wgpu::RenderPass<'static>,
                                    >(encoder.begin_render_pass(
                                        &wgpu::RenderPassDescriptor {
                                            label: Some("egui Render Pass"),
                                            color_attachments: &[Some(
                                                wgpu::RenderPassColorAttachment {
                                                    view: &view,
                                                    resolve_target: None,
                                                    ops: wgpu::Operations {
                                                        load: wgpu::LoadOp::Load,
                                                        store: wgpu::StoreOp::Store,
                                                    },
                                                },
                                            )],
                                            depth_stencil_attachment: None,
                                            ..Default::default()
                                        },
                                    ))
                                };
                                let mut render_pass = render_pass;

                                state.egui_renderer.render(
                                    &mut render_pass,
                                    &paint_jobs,
                                    &screen_descriptor,
                                );
                                drop(render_pass);

                                state.renderer.queue.submit(std::iter::once(encoder.finish()));
                            }

                            // Free egui textures that are no longer needed
                            for id in &full_output.textures_delta.free {
                                state.egui_renderer.free_texture(id);
                            }

                            surface_texture.present();

                            // Catch a page change made by an egui click this frame (the
                            // per-frame reconciliation ran before the egui frame). Same single
                            // authority, so no `cursor_free` desync. (v0.460)
                            let _ = page_before_frame;
                            reconcile_cursor(state);

                            // ── Apply settings changes from GUI to engine ──
                            if state.gui_state.settings_dirty {
                                state.gui_state.settings_dirty = false;

                                // FOV
                                state.camera.fov_degrees = state.gui_state.settings.fov;

                                // Mouse sensitivity
                                state.controller.mouse_sensitivity = state.gui_state.settings.mouse_sensitivity;

                                // Window presentation mode (v0.454).
                                apply_window_mode(&state.window, state.gui_state.settings.window_mode);

                                // Render distance → camera far plane
                                state.camera.far = state.gui_state.settings.render_distance;

                                // Persist settings to config file
                                crate::config::AppConfig::from_gui_state(&state.gui_state).save();
                            }
                        }
                        Err(wgpu::SurfaceError::Lost) => {
                            let size = state.window.inner_size();
                            state.renderer.resize(size.width, size.height);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            log::error!("Out of GPU memory");
                            event_loop.exit();
                        }
                        Err(e) => {
                            log::warn!("Render error: {:?}", e);
                        }
                    }
                }
                _ => {}
            }
        }

        fn device_event(
            &mut self,
            _event_loop: &ActiveEventLoop,
            _device_id: DeviceId,
            event: DeviceEvent,
        ) {
            let state = match self.state.as_mut() {
                Some(s) => s,
                None => return,
            };

            if let DeviceEvent::MouseMotion { delta } = event {
                // Pass mouse motion to the camera when no GUI page is active. In FPS this is
                // look; in the showroom / construction orbit cam it only rotates while a mouse
                // button is held (so moving over a panel does nothing). (v0.464)
                if state.gui_state.active_page == GuiPage::None {
                    state.controller.process_mouse_motion(delta.0, delta.1);
                }
            }
        }

        fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
            if let Some(state) = &self.state {
                state.window.request_redraw();
            }
        }
    }
}

/// Run the native engine (only available with the `native` feature).
#[cfg(feature = "native")]
pub fn run() {
    native_app::run();
}
