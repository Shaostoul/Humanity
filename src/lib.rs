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
    /// On rebuild this APPENDS new meshes and repoints the slots; the old meshes stay in the
    /// renderer's mesh Vec (a small, bounded leak per edit -- fine for an editor session).
    fn apply_homestead_meshes(state: &mut EngineState, homestead: crate::ship::fibonacci::HomesteadMeshes) {
        // Floors (one mesh + material per room).
        state.homestead_floors.clear();
        for (verts, indices, color, material_type) in homestead.floors {
            let mesh_idx = state.renderer.add_mesh(Mesh::from_vertices(&state.renderer.device, &verts, &indices));
            let mat_idx = state.renderer.add_material_typed(color, 0.0, 0.8, material_type as f32);
            state.homestead_floors.push((mesh_idx, mat_idx));
        }
        // Combined-mesh families. Each is rebuilt only when non-empty (else cleared so a
        // removed window/mirror disappears on the next rebuild).
        state.homestead_walls = None;
        if !homestead.walls.0.is_empty() {
            let mi = state.renderer.add_mesh(Mesh::from_vertices(&state.renderer.device, &homestead.walls.0, &homestead.walls.1));
            let ma = state.renderer.add_material_typed([0.5, 0.5, 0.5, 1.0], 0.1, 0.6, 0.0);
            state.homestead_walls = Some((mi, ma));
        }
        state.homestead_trim = None;
        if !homestead.trim.0.is_empty() {
            let mi = state.renderer.add_mesh(Mesh::from_vertices(&state.renderer.device, &homestead.trim.0, &homestead.trim.1));
            let ma = state.renderer.add_material_typed([0.42, 0.30, 0.18, 1.0], 0.0, 0.7, 3.0);
            state.homestead_trim = Some((mi, ma));
        }
        state.homestead_windows = None;
        if !homestead.windows.0.is_empty() {
            let mi = state.renderer.add_mesh(Mesh::from_vertices(&state.renderer.device, &homestead.windows.0, &homestead.windows.1));
            // Tinted glass: alpha 0.45 (base_color.a is the opacity) reads clearly AS glass
            // while still seeing through, + a faint emissive so a pane catches the eye even in
            // a dim room. Rendered through the transparent pass. (v0.456, tuned v0.457)
            let ma = state.renderer.add_material_full([0.50, 0.74, 0.92, 0.45], 0.0, 0.08, 1.0, 0.12);
            state.homestead_windows = Some((mi, ma));
        }
        state.homestead_mirrors = None;
        if !homestead.mirrors.0.is_empty() {
            let mi = state.renderer.add_mesh(Mesh::from_vertices(&state.renderer.device, &homestead.mirrors.0, &homestead.mirrors.1));
            let ma = state.renderer.add_material_full([0.30, 0.55, 1.0, 1.0], 0.2, 0.15, 1.0, 1.6);
            state.homestead_mirrors = Some((mi, ma));
        }
        state.homestead_ceiling = None;
        if !homestead.ceilings.0.is_empty() {
            let mi = state.renderer.add_mesh(Mesh::from_vertices(&state.renderer.device, &homestead.ceilings.0, &homestead.ceilings.1));
            let ma = state.renderer.add_material_typed([0.60, 0.62, 0.68, 1.0], 0.0, 0.8, 2.0);
            state.homestead_ceiling = Some((mi, ma));
        }
    }

    /// Regenerate the homestead meshes from the live layout (the construction editor's apply).
    /// Also refreshes room lights + the sealed-volume bounds, since a height/wall edit changes
    /// them. (v0.455)
    fn rebuild_homestead(state: &mut EngineState) {
        let Some(layout) = state.homestead_layout.clone() else { return; };
        let homestead = crate::ship::fibonacci::generate_from_layout(&layout);
        let room_info = homestead.room_info.clone();
        apply_homestead_meshes(state, homestead);
        // Refresh lights + sealed bounds from the new room_info (height edits move them).
        state.room_lights = room_info.iter().map(|r| {
            let light_pos = Vec3::new(r.center.x, r.center.y + r.dimensions.y * 0.5 - 0.1, r.center.z);
            let room_size = r.dimensions.x.max(r.dimensions.z);
            let intensity = (room_size * 0.5).clamp(2.0, 15.0);
            (light_pos, [1.0, 0.95, 0.85], intensity, room_size * 1.5)
        }).collect();
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
        log::info!("Homestead rebuilt: {} rooms", room_info.len());
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
        let layout = crate::ship::fibonacci::load_layout_or_fallback();
        let homestead = crate::ship::fibonacci::generate_from_layout(&layout);
        let room_info = homestead.room_info.clone();
        state.homestead_layout = Some(layout);
        apply_homestead_meshes(state, homestead);

        // Room ceiling lights
        state.room_lights = room_info.iter().map(|r| {
            let light_pos = Vec3::new(r.center.x, r.center.y + r.dimensions.y * 0.5 - 0.1, r.center.z);
            let room_size = r.dimensions.x.max(r.dimensions.z);
            let intensity = (room_size * 0.5).clamp(2.0, 15.0);
            let radius = room_size * 1.5;
            (light_pos, [1.0, 0.95, 0.85], intensity, radius)
        }).collect();

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

        // ── Aeroponic tower placeholders (v0.383) ──
        // Simple-shape stand-ins until real 3D models exist (operator: "use simple
        // shapes as stand-ins"): one grey cylinder per loaded tower config + a green
        // sphere per planted variety in a vertical helix, placed on the garden floor
        // side by side. Marker count is capped (the full variety lives in the Home
        // page list) to stay within the per-frame object budget.
        state.placeholder_objects.clear();
        {
            let garden = room_info.iter().find(|r| r.id == "garden");
            let floor_y = garden.map(|r| r.center.y - r.dimensions.y * 0.5).unwrap_or(0.0);
            let gx = garden.map(|r| r.center.x).unwrap_or(0.0);
            let gz = garden.map(|r| r.center.z).unwrap_or(0.0);
            // Snapshot each tower's geometry (diameter / height / helix turns / plant
            // count) FIRST so the gui_state borrow is released before the renderer
            // mutations below. Geometry is data-driven (operator: dynamic + scalable).
            let towers: Vec<(f32, f32, f32, usize)> = state
                .gui_state
                .tower_configs
                .iter()
                .map(|t| (t.diameter_m, t.height_m, t.helix_turns, t.plantings.len()))
                .collect();
            let tower_count = towers.len().max(1) as f32;
            let tower_mat = state.renderer.add_material_typed([0.6, 0.62, 0.66, 1.0], 0.3, 0.6, 1.0);
            let sphere_mesh = state.renderer.add_mesh(Mesh::sphere(&state.renderer.device, 0.09, 8, 10));
            let plant_mat = state.renderer.add_material_typed([0.15, 0.7, 0.2, 1.0], 0.0, 0.9, 0.0);
            for (ti, &(diam, height, turns, n_plants)) in towers.iter().enumerate() {
                let radius = (diam * 0.5).max(0.05);
                let h = height.max(0.5);
                let t_turns = turns.max(0.5);
                // Space towers by their width so wide ones do not overlap.
                let tx = gx + (ti as f32 - (tower_count - 1.0) * 0.5) * (1.0 + diam.max(0.3));
                // One cylinder per tower (per-tower diameter + height).
                let cyl_mesh = state.renderer.add_mesh(Mesh::cylinder(&state.renderer.device, radius, h, 20));
                state.placeholder_objects.push((cyl_mesh, tower_mat, Vec3::new(tx, floor_y, gz)));
                // One plant marker per curated variety, up a helix of `t_turns` wraps
                // (capped for the per-frame object budget; the full list is on Home).
                let n = n_plants.min(40).max(1);
                for p in 0..n {
                    let frac = (p as f32 + 0.5) / n as f32;
                    let a = frac * t_turns * std::f32::consts::TAU;
                    let y = floor_y + 0.1 + frac * (h - 0.2);
                    let mr = radius + 0.12; // markers sit just off the column
                    state.placeholder_objects.push((
                        sphere_mesh,
                        plant_mat,
                        Vec3::new(tx + mr * a.cos(), y, gz + mr * a.sin()),
                    ));
                }
            }
            if towers.is_empty() {
                // No configs: still drop one bare tower so the garden spot is visible.
                let cyl_mesh = state.renderer.add_mesh(Mesh::cylinder(&state.renderer.device, 0.2, 2.0, 20));
                state.placeholder_objects.push((cyl_mesh, tower_mat, Vec3::new(gx, floor_y, gz)));
            }
        }

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
                // Anchor (low pipe height) per instance, so connection tubes line up.
                let mut anchors: HashMap<String, Vec3> = HashMap::new();
                // Per-instance (floor_y, ceiling_y) so the pipe router can size run height.
                let mut anchor_rooms: HashMap<String, (f32, f32)> = HashMap::new();
                // Room AABBs (min, max) so the router can sleeve pipes at wall penetrations.
                let room_aabbs: Vec<(Vec3, Vec3)> = room_info
                    .iter()
                    .map(|r| (r.center - r.dimensions * 0.5, r.center + r.dimensions * 0.5))
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
                // Explicit instances + every `arrays` grid expanded (dense garden towers).
                let all_instances = home.all_instances();
                for inst in &all_instances {
                    let Some(&(center, floor_y, ceiling_y)) = rooms.get(inst.room.as_str()) else { continue };
                    let Some(def) = home.catalog.get(&inst.machine) else { continue };
                    let pos = Vec3::new(
                        center.x + inst.offset.0,
                        floor_y + inst.offset.1,
                        center.z + inst.offset.2,
                    );
                    let (sx, sy, sz) = def.size;
                    let mesh = match def.shape.as_str() {
                        "cylinder" => Mesh::cylinder(&state.renderer.device, sx.max(0.02), sy.max(0.05), 16),
                        "sphere" => Mesh::sphere(&state.renderer.device, sx.max(0.02), 10, 12),
                        "pyramid" => Mesh::pyramid(&state.renderer.device, sx.max(0.05), sy.max(0.05)),
                        _ => Mesh::box_xyz(&state.renderer.device, sx.max(0.02), sy.max(0.02), sz.max(0.02)),
                    };
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
                    state.placeholder_objects.push((mesh_idx, mat, draw_pos));
                    anchors.insert(inst.id.clone(), Vec3::new(pos.x, floor_y + 0.35, pos.z));
                    anchor_rooms.insert(inst.id.clone(), (floor_y, ceiling_y));
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
                // Connections as REALISTIC routed pipe runs: orthogonal up-over-down routing
                // (no diagonals through walls/machines) with real fittings, the way a plumber
                // and electrician would install exposed services on a ship. Rules are data
                // (data/routing_rules.ron); the geometry plan comes from the routing module.
                use crate::systems::construction::routing::{plan_pipe, PipePart, RoutingRules};
                let rules = RoutingRules::load(&state.data_dir.join("routing_rules.ron"));
                // Shared fitting meshes/materials reused by translation (keeps mesh count sane).
                let bracket_mesh =
                    state.renderer.add_mesh(Mesh::box_xyz(&state.renderer.device, 0.06, 0.05, 0.06));
                let bracket_mat =
                    state.renderer.add_material_typed([0.45, 0.46, 0.48, 1.0], 0.9, 0.5, 0.0); // steel grey
                let lever_mat =
                    state.renderer.add_material_typed([0.80, 0.20, 0.16, 1.0], 0.5, 0.5, 0.0); // red valve lever
                let sleeve_mat =
                    state.renderer.add_material_typed([0.55, 0.56, 0.58, 1.0], 0.9, 0.45, 0.0); // steel wall sleeve
                let mut linked = 0usize;
                for conn in &home.connections {
                    let (Some(&a), Some(&b)) = (anchors.get(&conn.from), anchors.get(&conn.to))
                    else {
                        continue;
                    };
                    let (fa, ca) = anchor_rooms.get(&conn.from).copied().unwrap_or((a.y - 0.35, a.y + 3.0));
                    let (fb, cb) = anchor_rooms.get(&conn.to).copied().unwrap_or((b.y - 0.35, b.y + 3.0));
                    let floor = fa.max(fb);
                    let ceiling = ca.min(cb);
                    let run_h = rules.run_height(&conn.kind, floor, ceiling);
                    let (_lane, pipe_r) = rules.lane(&conn.kind);
                    let color = crate::machines::MachineHome::connection_color(&conn.kind);
                    // Metallic pipe body; fittings (elbows/collars/valve body) a shade darker.
                    let pipe_mat = state.renderer.add_material_typed(color, 0.7, 0.35, 0.0);
                    let fitting_mat = state.renderer.add_material_typed(
                        [color[0] * 0.7, color[1] * 0.7, color[2] * 0.7, 1.0],
                        0.85,
                        0.4,
                        0.0,
                    );
                    // One elbow sphere per run (all its elbows share a radius), reused by position.
                    let elbow_mesh = state.renderer.add_mesh(Mesh::sphere(
                        &state.renderer.device,
                        pipe_r * rules.elbow_mult,
                        8,
                        10,
                    ));
                    let parts =
                        plan_pipe(a, b, pipe_r, run_h, rules.is_fluid(&conn.kind), &room_aabbs, &rules);
                    for part in &parts {
                        match part {
                            PipePart::Tube { a, b, radius } => {
                                let m = state.renderer.add_mesh(Mesh::tube(
                                    &state.renderer.device,
                                    *a,
                                    *b,
                                    *radius,
                                    8,
                                ));
                                state.placeholder_objects.push((m, pipe_mat, Vec3::ZERO));
                            }
                            PipePart::Elbow { at, .. } => {
                                state.placeholder_objects.push((elbow_mesh, fitting_mat, *at));
                            }
                            PipePart::Bracket { at } => {
                                state.placeholder_objects.push((
                                    bracket_mesh,
                                    bracket_mat,
                                    Vec3::new(at.x, at.y - 0.025, at.z),
                                ));
                            }
                            PipePart::Valve { at, axis, radius } => {
                                let half = *axis * 0.05;
                                let bm = state.renderer.add_mesh(Mesh::tube(
                                    &state.renderer.device,
                                    *at - half,
                                    *at + half,
                                    *radius,
                                    8,
                                ));
                                state.placeholder_objects.push((bm, fitting_mat, Vec3::ZERO));
                                // Lever sticking out (the ball-valve handle).
                                state.placeholder_objects.push((
                                    bracket_mesh,
                                    lever_mat,
                                    Vec3::new(at.x + 0.09, at.y, at.z),
                                ));
                            }
                            PipePart::Penetration { at, axis, radius } => {
                                // Steel sleeve through the wall + a wider escutcheon ring.
                                let half = *axis * 0.09;
                                let sleeve = state.renderer.add_mesh(Mesh::tube(
                                    &state.renderer.device,
                                    *at - half,
                                    *at + half,
                                    *radius,
                                    10,
                                ));
                                state.placeholder_objects.push((sleeve, sleeve_mat, Vec3::ZERO));
                                let band = *axis * 0.02;
                                let ring = state.renderer.add_mesh(Mesh::tube(
                                    &state.renderer.device,
                                    *at - band,
                                    *at + band,
                                    *radius * 1.5,
                                    10,
                                ));
                                state.placeholder_objects.push((ring, sleeve_mat, Vec3::ZERO));
                            }
                        }
                    }
                    linked += 1;
                }
                log::info!("Machines: placed {placed} machines + {linked} connections");
            }
        }

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
        /// Homestead walls mesh + material.
        homestead_walls: Option<(usize, usize)>,
        /// Homestead trim mesh (baseboards, crown, door/window frames) + material. (v0.453)
        homestead_trim: Option<(usize, usize)>,
        /// Homestead window-glass mesh + material. (v0.453)
        homestead_windows: Option<(usize, usize)>,
        /// Homestead mirror / portal panel mesh + material. (v0.453)
        homestead_mirrors: Option<(usize, usize)>,
        /// Homestead ceiling mesh + material — drawn only when `gui_state.show_roof`. (v0.453)
        homestead_ceiling: Option<(usize, usize)>,
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
                std::sync::Mutex::new(Option::<Vec<(String, u32)>>::None),
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
                name: "Asteroid M-12 (metallic)".to_string(),
                classification: "M".to_string(),
                ores: vec![
                    ("iron_ore_0".to_string(), 120.0),
                    ("nickel_ore_0".to_string(), 60.0),
                    ("platinum_ore_0".to_string(), 20.0),
                ],
            },));
            game_world.world.spawn((crate::ecs::components::AsteroidBody {
                name: "Asteroid S-7 (silicaceous)".to_string(),
                classification: "S".to_string(),
                ores: vec![
                    ("iron_ore_0".to_string(), 40.0),
                    ("copper_ore_0".to_string(), 50.0),
                ],
            },));

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
            gui_state.homestead_design = crate::gui::load_homestead_design(&data_dir);
            // Self-sufficiency loops for the Home-page closure summary (v0.432).
            gui_state.homestead_loops = crate::machines::MachineHome::load(
                &data_dir.join("machines").join("home.ron"),
            )
            .map(|h| h.loops)
            .unwrap_or_default();
            gui_state.tower_configs = crate::gui::load_tower_configs(&data_dir);
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
                homestead_trim: None,
                homestead_windows: None,
                homestead_mirrors: None,
                homestead_ceiling: None,
                homestead_layout: None,
                construction_cam_active: false,
                construction_return_pos: Vec3::new(0.0, 1.7, 0.0),
                cursor_pos: (0.0, 0.0),
                construction_grab: None,
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
                    crate::save_load::save_active_home(&state.game_world.world);
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
                            if let Some(t) = state.gui_state.targeted_machine {
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
                    let pressed = btn_state == ElementState::Pressed;
                    // Construction astral editor: LEFT grabs/drops a room (left is a no-op in the
                    // orbit cam, so we own it). Gated on !egui_consumed so panel clicks never
                    // start a grab. (v0.466)
                    if state.gui_state.construction_active && left && !egui_consumed {
                        if pressed {
                            try_begin_room_grab(state);
                        } else {
                            state.construction_grab = None; // release; keep the selection highlighted
                            state.construction_gizmo_grab = None; // release a slid handle too
                        }
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
                        if let Some(floor) = state
                            .gui_state
                            .room_bounds
                            .iter()
                            .find(|r| {
                                p.x >= r.min.x && p.x <= r.max.x && p.z >= r.min.z && p.z <= r.max.z
                            })
                            .map(|r| r.min.y)
                        {
                            state.controller.set_ground_floor(floor);
                        }
                    }

                    // Update camera from input
                    state.controller.update_camera(&mut state.camera, dt);

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
                    // Mining: bridge a commissioned drone MANIFEST to DroneSystem.
                    if let Some(manifest) = state.gui_state.pending_drone_manifest.take() {
                        if let Some(slot) = state
                            .data_store
                            .get::<std::sync::Mutex<Option<Vec<(String, u32)>>>>("commission_drone")
                        {
                            if let Ok(mut s) = slot.lock() {
                                *s = Some(manifest);
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
                    crate::save_load::maybe_periodic_save(&state.game_world.world, 120);

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
                        let (center, size) = state.homestead_bounds
                            .map(|(mn, mx)| ((mn + mx) * 0.5, (mx - mn).length()))
                            .unwrap_or((Vec3::new(0.0, 1.5, 0.0), 20.0));
                        state.camera.switch_mode(crate::renderer::camera::CameraMode::Orbit);
                        state.camera.orbit_target = center;
                        state.camera.orbit_distance = (size * 0.7).clamp(5.0, 400.0);
                        state.camera.orbit_distance_max = (size * 4.0).max(400.0);
                    } else if !state.gui_state.construction_active && state.construction_cam_active {
                        state.construction_cam_active = false;
                        state.camera.switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
                        state.camera.position = state.construction_return_pos;
                        state.gui_state.construction_selected_room = None;
                        state.construction_grab = None;
                        state.construction_gizmo_grab = None;
                    }
                    // 3D room drag (v0.466): a grabbed room follows the cursor on its floor.
                    // Slide-gizmo drag (v0.468) takes precedence: a grabbed door/window handle
                    // slides along its wall; a grabbed room follows the cursor on its floor.
                    if state.construction_gizmo_grab.is_some() {
                        apply_gizmo_drag(state);
                    } else {
                        apply_room_drag(state);
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
                    if state.gui_state.construction_save {
                        state.gui_state.construction_save = false;
                        if let Some(layout) = &state.homestead_layout {
                            match crate::ship::fibonacci::save_layout(layout) {
                                Ok(()) => log::info!("Construction: layout saved to RON"),
                                Err(e) => log::warn!("Construction: save failed: {e}"),
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
                        crate::save_load::save_active_home(&state.game_world.world);
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
                    // Celestial bodies (planet + Sun + solar bodies): rendered in a SEPARATE
                    // pass with a huge far plane (v0.450), since they sit at astronomical
                    // distances the ~500 m gameplay far would clip.
                    let mut celestial_objects: Vec<RenderObject> = Vec::new();
                    // World-space orbit lines, built this frame, drawn
                    // after the scene so they depth-occlude behind planets.
                    let mut orbit_lines: Vec<crate::renderer::line::LineVertex> = Vec::new();

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
                        if state.gui_state.show_roof {
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
                        // Glass windows -> the transparent pass.
                        if let Some((mesh_idx, mat_idx)) = state.homestead_windows {
                            transparent_objects.push(RenderObject {
                                position: Vec3::ZERO,
                                rotation: Quat::IDENTITY,
                                scale: Vec3::ONE,
                                mesh: mesh_idx,
                                material: mat_idx,
                            });
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
                        if sun_dir != glam::DVec3::ZERO {
                            state.renderer.set_sun_light(
                                Vec3::new(
                                    sun_dir.x as f32,
                                    sun_dir.y as f32,
                                    sun_dir.z as f32,
                                ),
                                [1.0, 0.97, 0.92],
                                2.5,
                            );
                        }
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
                    // v0.488: push the live voice input params (gain / filter / transmit
                    // mode / activation threshold) to the worker every frame, so changes
                    // apply without restarting the test. For push-to-talk / push-to-mute,
                    // read whether the bound key is held this frame (egui has key state
                    // while the Settings UI is focused, which is where the test lives).
                    {
                        let uses_key = state.gui_state.voice_transmit_mode.uses_key();
                        let ptt_held = if uses_key {
                            let name = state.gui_state.voice_ptt_key.clone();
                            match egui::Key::ALL.iter().copied().find(|k| k.name().eq_ignore_ascii_case(&name)) {
                                Some(k) => state.egui_ctx.input(|i| i.key_down(k)),
                                None => false,
                            }
                        } else {
                            false
                        };
                        state.gui_state.voice_ptt_held = ptt_held;
                        crate::net::voice::set_input_params(
                            state.gui_state.voice_gain,
                            state.gui_state.voice_filter_mode,
                            state.gui_state.voice_transmit_mode,
                            state.gui_state.voice_vad_threshold,
                            ptt_held,
                        );
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
                            state.gui_state.asteroids.push(crate::gui::GuiAsteroid {
                                name: ast.name.clone(),
                                classification: ast.classification.clone(),
                                ores: ast.ores.iter().map(|(id, q)| (id.clone(), *q)).collect(),
                            });
                        }
                        state.gui_state.drones.clear();
                        for (_e, drone) in state
                            .game_world
                            .world
                            .query::<&crate::ecs::components::Drone>()
                            .iter()
                        {
                            let dur = crate::systems::mining::phase_secs(&drone.phase);
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
                                                let vc_name = vc.get("name")
                                                    .or_else(|| vc.get("id"))
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                if vc_name.is_empty() { continue; }
                                                // Live voice roster (public_key, display_name), v0.481.
                                                let roster: Vec<(String, String)> = vc.get("participants")
                                                    .and_then(|v| v.as_array())
                                                    .map(|arr| arr.iter().filter_map(|p| {
                                                        let k = p.get("public_key").and_then(|v| v.as_str())?.to_string();
                                                        let n = p.get("display_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                                        Some((k, n))
                                                    }).collect())
                                                    .unwrap_or_default();
                                                // Assign the roster onto the matching channel (a text channel
                                                // with the same name, voice merged in UI) OR push a new one.
                                                if let Some(c) = state.gui_state.chat_channels.iter_mut()
                                                    .find(|c| c.name == vc_name || c.id == vc_name)
                                                {
                                                    c.voice_enabled = true;
                                                    c.voice_participants = roster;
                                                } else {
                                                    let vc_id = vc.get("id")
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or(&vc_name)
                                                        .to_string();
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
                                    Some("voice_room_signal") | Some("voice_call") | Some("voice_room") | Some("voice_room_update") => {
                                        // Intentional no-op. Web users in voice rooms still
                                        // talk to each other; native users just don't hear/
                                        // see voice activity yet. (webrtc_signal is now
                                        // handled below — DataChannel P2P, increment 1.)
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
                            // Exponential backoff: 5s -> 10s -> 20s -> 40s -> 60s (max)
                            state.gui_state.ws_reconnect_delay = (state.gui_state.ws_reconnect_delay * 2.0).min(60.0);
                            state.gui_state.ws_status = "Reconnecting...".to_string();
                        }
                    }

                    // ── Reset backoff on successful connection ──
                    if state.gui_state.ws_client.as_ref().map_or(false, |c| c.is_connected()) {
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
                                    crate::net::webrtc::WebrtcEvent::VoiceFrame { peer: _peer, opus: _opus } => {
                                        // Phase B: inbound Opus from a voice peer.
                                        // Decode + mix + playback is wired in a
                                        // later phase (Phase D); dropped for now.
                                    }
                                    crate::net::webrtc::WebrtcEvent::Closed { peer } => {
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
