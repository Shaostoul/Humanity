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
        wallet, crafting, guilds, trade, files, bugs, resources, donate, tools, studio,
        onboarding, server_settings, identity, governance, recovery, testing,
        browser, category_overview, settings_pages, cosmos,
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
    use crate::systems::interaction::InteractionSystem;
    use crate::systems::inventory::{Inventory, InventorySystem, ItemRegistry};
    use crate::systems::player::PlayerControllerSystem;
    use crate::systems::time::{GameTime, TimeSystem};
    use crate::systems::weather::Weather;
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

        // Prefer a "full" data dir (with world/ subdirectory) over extracted-only
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
            "quests/construction.ron", "quests/exploration.ron",
            "quests/farming.ron", "quests/tutorial.ron",
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

    /// Lazy-load the 3D world: homestead, hologram, stars, planet, CSV data.
    /// Called once on first Enter World. Keeps app startup instant (chat-first).
    fn load_world(state: &mut EngineState) {
        log::info!("Loading 3D world...");
        let load_start = Instant::now();

        // ── Homestead meshes ──
        let homestead = crate::ship::fibonacci::generate_homestead();
        for (verts, indices, color, material_type) in homestead.floors {
            let mesh_idx = state.renderer.add_mesh(Mesh::from_vertices(&state.renderer.device, &verts, &indices));
            let mat_idx = state.renderer.add_material_typed(color, 0.0, 0.8, material_type as f32);
            state.homestead_floors.push((mesh_idx, mat_idx));
        }
        if !homestead.walls.0.is_empty() {
            let mesh_idx = state.renderer.add_mesh(Mesh::from_vertices(&state.renderer.device, &homestead.walls.0, &homestead.walls.1));
            let mat_idx = state.renderer.add_material_typed([0.5, 0.5, 0.5, 1.0], 0.1, 0.6, 0.0);
            state.homestead_walls = Some((mesh_idx, mat_idx));
        }

        // Room ceiling lights
        state.room_lights = homestead.room_info.iter().map(|r| {
            let light_pos = Vec3::new(r.center.x, r.center.y + r.dimensions.y * 0.5 - 0.1, r.center.z);
            let room_size = r.dimensions.x.max(r.dimensions.z);
            let intensity = (room_size * 0.5).clamp(2.0, 15.0);
            let radius = room_size * 1.5;
            (light_pos, [1.0, 0.95, 0.85], intensity, radius)
        }).collect();

        // Hologram + spawn rooms
        let hologram_room_center = homestead.room_info.iter()
            .find(|r| r.is_hologram_room)
            .map(|r| r.center);
        let spawn_room = homestead.room_info.iter()
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
            homestead.room_info.len(), state.homestead_floors.len(),
            state.homestead_walls.is_some(), state.room_lights.len());

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

        // ── Load CSV game data ──
        #[derive(Debug, serde::Deserialize)]
        #[allow(dead_code)]
        struct ItemRow { id: String, name: String }
        #[derive(Debug, serde::Deserialize)]
        #[allow(dead_code)]
        struct PlantRow { id: String, name: String }
        #[derive(Debug, serde::Deserialize)]
        #[allow(dead_code)]
        struct RecipeRow { id: String, name: String }

        let _ = state.asset_manager.load_csv::<ItemRow>("items.csv");
        let _ = state.asset_manager.load_csv::<PlantRow>("plants.csv");
        let _ = state.asset_manager.load_csv::<RecipeRow>("recipes.csv");

        state.world_loaded = true;
        log::info!("3D world loaded in {:.0}ms", load_start.elapsed().as_millis());
    }

    struct App {
        state: Option<EngineState>,
    }

    impl App {
        fn new() -> Self {
            Self { state: None }
        }
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
        /// Homestead walls mesh + material.
        homestead_walls: Option<(usize, usize)>,
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

            let window_attrs = Window::default_attributes()
                .with_title(format!("HumanityOS v{}", env!("CARGO_PKG_VERSION")))
                .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
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
            let controller = CameraController::new(5.0, 3.0);

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
            system_runner.register(TimeSystem::new());
            system_runner.register(PlayerControllerSystem);
            data_store.insert("interaction_prompt", std::sync::Mutex::new(String::new()));
            system_runner.register(InteractionSystem::new());
            system_runner.register(FarmingSystem::new());
            system_runner.register(InventorySystem::new());
            system_runner.register(CraftingSystem::new());
            game_world.world.spawn((
                Transform::default(),
                Velocity::default(),
                Controllable,
                Health::default(),
                Name("Player".to_string()),
                Inventory::new(36),
            ));

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
            gui_state.equipment_slots = crate::gui::load_equipment_slots(&data_dir);
            let (sevs, cats) = crate::gui::load_bug_taxonomy(&data_dir);
            gui_state.bug_severities = sevs;
            gui_state.bug_categories = cats;
            gui_state.crafting_categories = crate::gui::load_crafting_categories(&data_dir);
            gui_state.market_categories = crate::gui::load_market_categories(&data_dir);
            gui_state.resource_categories = crate::gui::load_resource_categories(&data_dir);
            gui_state.studio_scene_presets = crate::gui::load_studio_scenes(&data_dir);
            gui_state.studio_source_presets = crate::gui::load_studio_sources(&data_dir);
            gui_state.profile_skills = crate::gui::load_default_player_skills(&data_dir);
            gui_state.studio_streaming_config = crate::gui::load_studio_streaming_config(&data_dir);
            gui_state.donate_faq = crate::gui::load_donate_faq(&data_dir);
            gui_state.qa_test_tasks = crate::gui::load_qa_test_tasks(&data_dir);
            gui_state.browser_bookmarks = crate::gui::load_browser_bookmarks(&data_dir);
            gui_state.onboarding_concepts = crate::gui::load_onboarding_concepts(&data_dir);
            gui_state.onboarding_core_pages = crate::gui::load_onboarding_core_pages(&data_dir);
            // v0.197.0: ai_usage_filters loader removed.
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

            // Clean up .old files from previous updates
            crate::updater::Updater::cleanup_old_versions();

            // Auto-check for updates on startup (if enabled)
            if gui_state.updater.channel == crate::updater::UpdateChannel::AlwaysLatest {
                gui_state.updater.check_now();
            }

            // Post-identity routing (v0.198.0, v0.220.0 boot page):
            //   - !onboarding_complete: stay on MainMenu (identity / seed setup)
            //   - onboarding_complete && !concept_tour_seen: land on Onboarding
            //   - onboarding_complete && concept_tour_seen: user's chosen boot page
            if gui_state.onboarding_complete {
                gui_state.active_page = if gui_state.concept_tour_seen {
                    gui_state.default_page
                } else {
                    GuiPage::Onboarding
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
                homestead_walls: None,
                hologram_objects: Vec::new(),
                hologram_orbits: Vec::new(),
                hologram_pins: Vec::new(),
                targeted_planet: None,
                hologram_room_center: Vec3::new(-0.5, 1.0, 2.5),
                room_lights: Vec::new(),
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

            // Pass events to egui first
            let egui_response = state.egui_state.on_window_event(&state.window, &event);
            let egui_consumed = egui_response.consumed;

            match event {
                WindowEvent::CloseRequested => {
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
                            // Update cursor grab based on page transition
                            let new_page = state.gui_state.active_page;
                            if old_page == GuiPage::None && new_page != GuiPage::None {
                                // Entering a menu: release cursor
                                state.window.set_cursor_visible(true);
                                state.window.set_cursor_grab(winit::window::CursorGrabMode::None).ok();
                            } else if old_page != GuiPage::None && new_page == GuiPage::None {
                                // Returning to FPS mode: grab cursor
                                state.window.set_cursor_visible(false);
                                state.window.set_cursor_grab(winit::window::CursorGrabMode::Confined)
                                    .or_else(|_| state.window.set_cursor_grab(winit::window::CursorGrabMode::Locked))
                                    .ok();
                            }
                            return;
                        }

                        // Enter toggles chat overlay (only when in-game)
                        if key == KeyCode::Enter && pressed
                            && state.gui_state.active_page == GuiPage::None
                            && !egui_consumed
                        {
                            state.gui_state.show_chat = !state.gui_state.show_chat;
                        }

                        // Tab toggles inventory (only when in-game)
                        if key == KeyCode::Tab && pressed
                            && state.gui_state.active_page == GuiPage::None
                        {
                            state.gui_state.active_page = GuiPage::Inventory;
                            // Release cursor for inventory page
                            state.window.set_cursor_visible(true);
                            state.window.set_cursor_grab(winit::window::CursorGrabMode::None).ok();
                            return;
                        }

                        // Don't pass input to game when egui consumed it or a menu is open
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
                WindowEvent::MouseInput { button, state: btn_state, .. } => {
                    if !egui_consumed && state.gui_state.active_page == GuiPage::None {
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

                    // Tick all ECS systems
                    state.system_runner.tick(
                        &mut state.game_world.world,
                        dt,
                        &state.data_store,
                    );

                    // Build render objects from homestead meshes
                    let mut all_objects: Vec<RenderObject> = Vec::new();
                    // World-space orbit lines, built this frame, drawn
                    // after the scene so they depth-occlude behind planets.
                    let mut orbit_lines: Vec<crate::renderer::line::LineVertex> = Vec::new();

                    // Homestead at origin — vertex positions are in ship-local coords
                    for &(mesh_idx, mat_idx) in &state.homestead_floors {
                        all_objects.push(RenderObject {
                            position: Vec3::ZERO,
                            rotation: Quat::IDENTITY,
                            scale: Vec3::ONE,
                            mesh: mesh_idx,
                            material: mat_idx,
                        });
                    }
                    if let Some((mesh_idx, mat_idx)) = state.homestead_walls {
                        all_objects.push(RenderObject {
                            position: Vec3::ZERO,
                            rotation: Quat::IDENTITY,
                            scale: Vec3::ONE,
                            mesh: mesh_idx,
                            material: mat_idx,
                        });
                    }

                    // Solar system hologram centered in the designated hologram room (1m above floor)
                    let hologram_center = state.hologram_room_center;

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
                        all_objects.push(RenderObject {
                            position: render_pos,
                            rotation,
                            scale: Vec3::splat(scale),
                            mesh: mesh_idx,
                            material: state.planet_material,
                        });

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
                            all_objects.push(RenderObject {
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
                        for (pts_m, parent_id) in &state.solar_orbit_paths {
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

                    // Bridge game time from DataStore (if TimeSystem writes it)
                    if let Some(gt) = state.data_store.get::<GameTime>("game_time") {
                        state.gui_state.game_time = Some(GuiGameTime {
                            hour: gt.hour,
                            day_count: gt.day_count,
                            season: format!("{:?}", gt.season),
                            is_daytime: gt.hour >= 6.0 && gt.hour <= 18.0,
                        });
                    }

                    // Bridge weather from DataStore (if WeatherSystem writes it)
                    if let Some(w) = state.data_store.get::<Weather>("weather") {
                        state.gui_state.weather = Some(GuiWeather {
                            condition: format!("{:?}", w.condition),
                            temperature: w.temperature,
                            wind_speed: w.wind_speed,
                        });
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
                                            if msg.starts_with("__sync_data__")
                                                || msg.starts_with("__game__:")
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
                                                // If a text channel with same name exists, skip (voice is merged in UI)
                                                let exists = state.gui_state.chat_channels.iter().any(|c| c.name == vc_name || c.id == vc_name);
                                                if !exists {
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
                                        crate::debug::push_debug(format!(
                                            "WebRTC: frame from {}: {}", short(&peer), text
                                        ));
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

                                // Pass 2: Scene objects (LoadOp::Load preserves stars)
                                state.renderer.render_scene_onto(&state.camera, &all_objects, &view);
                                // Pass 3: orbit lines — after the scene so
                                // the depth buffer occludes segments behind
                                // planets (thin single-edge, not tubes).
                                state.renderer.draw_lines_onto(&state.camera, &orbit_lines, &view);
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
                                    GuiPage::Resources => resources::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Donate => donate::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Tools => tools::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Studio => studio::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Onboarding => onboarding::draw(ctx, &state.theme, &mut state.gui_state),
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

                                // Always draw HUD when in-game
                                if state.gui_state.active_page == GuiPage::None && state.gui_state.show_hud {
                                    hud::draw(ctx, &state.theme, &state.gui_state, state.camera.yaw);
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

                            // ── Update cursor grab if page changed during egui frame ──
                            let page_after_frame = state.gui_state.active_page;
                            if page_before_frame != page_after_frame {
                                if page_after_frame == GuiPage::None {
                                    // Returning to FPS mode: grab cursor
                                    state.window.set_cursor_visible(false);
                                    state.window.set_cursor_grab(winit::window::CursorGrabMode::Confined)
                                        .or_else(|_| state.window.set_cursor_grab(winit::window::CursorGrabMode::Locked))
                                        .ok();
                                } else if page_before_frame == GuiPage::None {
                                    // Leaving FPS mode: release cursor
                                    state.window.set_cursor_visible(true);
                                    state.window.set_cursor_grab(winit::window::CursorGrabMode::None).ok();
                                }
                            }

                            // ── Apply settings changes from GUI to engine ──
                            if state.gui_state.settings_dirty {
                                state.gui_state.settings_dirty = false;

                                // FOV
                                state.camera.fov_degrees = state.gui_state.settings.fov;

                                // Mouse sensitivity
                                state.controller.mouse_sensitivity = state.gui_state.settings.mouse_sensitivity;

                                // Fullscreen
                                let fullscreen = if state.gui_state.settings.fullscreen {
                                    Some(winit::window::Fullscreen::Borderless(None))
                                } else {
                                    None
                                };
                                state.window.set_fullscreen(fullscreen);

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
                // Only pass mouse motion to camera when no GUI page is active
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
