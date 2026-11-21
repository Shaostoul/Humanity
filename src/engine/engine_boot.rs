//! `EngineState::boot`: everything the app keeps between frames, built with or without the view's
//! GPU (the headless sim runner, increment 4, 2026-11-15; docs/design/headless-sim-runner.md).
//! The windowed app (lib.rs `resumed`) makes the window, the renderer and egui's window and
//! painter, and passes them as `Some(gpu)`; a headless run (`--sim walk`, engine/sim_walk.rs) passes
//! None, and nothing GPU is created: the star sky's prebuild needs the device and is skipped, the
//! camera takes a 16:9 aspect. Moved verbatim from lib.rs `resumed` but for those, and for the
//! app's outside services (audio, the live weather feed, the updater's clean-up and check, a
//! configured host node, the low-impact rig's confirmation), which run only when
//! `BootOptions::services` asks (the windowed app does; a headless run does not).

#![allow(unused_imports)]
use glam::Vec3;
use crate::engine::{
    color::*, dm::*, editor::*, frame_lock::*, geom::*, home_meshes::*, home_spawn::*, ipc::*, ipc_parse::*, near_tree_models::*, net_route::*, registries::*, world_load::*,
    state::*,
};
use crate::assets::AssetManager;
use humanity_core::ecs::components::Controllable;
use crate::gui::{GuiPage, GuiState};
use crate::gui::widgets::help_modal;
use humanity_core::hot_reload::HotReloadCoordinator;
use crate::renderer::camera::{Camera, CameraController};
use std::path::PathBuf;
use std::time::Instant;

/// What the engine is booted with besides the GPU.
pub(crate) struct BootOptions {
    /// The game's data folder (lib.rs `find_data_dir`).
    pub data_dir: PathBuf,
    /// Where the saves are read from and written to (a headless run gives a scratch folder).
    pub saves_dir: PathBuf,
    /// The config as read (None: the defaults).
    pub config: Option<crate::config::AppConfig>,
    /// The app's outside services: audio, the live weather feed, the updater, a configured host
    /// node, the low-impact rig's confirmation. The windowed app runs them; a headless run does not.
    pub services: bool,
    /// The startup's timer, begun before the window.
    pub boot_timer: humanity_core::boot_timing::BootTimer,
    /// egui's context (CPU only: the pages' state and the theme); the window's egui state and
    /// painter, which need the window and the device, are in the GPU.
    pub egui_ctx: egui::Context,
}

impl EngineState {
    /// The engine, booted: with `Some(gpu)` the windowed app, with None a headless run.
    pub(crate) fn boot(mut opts: BootOptions, gpu: Option<Gpu>) -> EngineState {
        let data_dir = opts.data_dir.clone();
        // Publish it for the cached loaders (laws/glossary/homes) so they resolve
        // the same CWD-independent path instead of a bare relative "data".
        let _ = humanity_core::storage::DATA_DIR.set(data_dir.clone());

        // Minimal camera/controller (needed by struct, but not used until world loads)
        let mut camera = Camera::new();
        camera.aspect = gpu.as_ref().map_or(16.0 / 9.0, |g| g.renderer.aspect_ratio());
        camera.position = Vec3::new(-0.5, 1.7, 4.0);
        camera.pitch = -0.2;
        camera.yaw = std::f32::consts::PI;
        // Sensitivity here is just the frame-0 default; the boot-time settings_dirty
        // sync (below) overrides it with the saved value on the first frame.
        let controller = CameraController::new(crate::renderer::camera::WALK_SPEED_MPS, 0.25);

        // Minimal asset/ECS (lightweight, no file I/O)
        let asset_manager = AssetManager::new(data_dir.clone());
        let hot_reload = HotReloadCoordinator::new(&data_dir);
        // The world's simulation, built without the GPU (engine/sim_boot.rs, the headless sim
        // runner's increment 1): the ECS world, the DataStore's slots, every system, the live
        // save, the fresh player, the asteroids and the menu's live home power.
        let crate::engine::sim_boot::SimBoot {
            mut game_world,
            system_runner,
            mut data_store,
            config: boot_config,
            slot: boot_slot,
            home_save,
        } = crate::engine::sim_boot::boot(crate::engine::sim_boot::SimBootOptions {
            data_dir: data_dir.clone(),
            saves_dir: opts.saves_dir.clone(),
            config: opts.config.take(),
        });

        let egui_ctx = opts.egui_ctx.clone();
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&egui_ctx);
        // Install every font fallback chain: Hack into the Proportional
        // family (so arrows and box drawing stop being tofu on Linux and
        // macOS), plus the OS-installed CJK and emoji fonts, which are
        // read from the user's machine and never redistributed. Each is
        // independent and a missing one is a silent no-op, but set_fonts
        // itself always runs. See src/gui/fonts.rs. The same helper runs
        // for every in-world screen context (gui::screen_surface), so a
        // wall screen never shows tofu the main UI does not.
        crate::gui::install_fonts(&egui_ctx);
        // A repaint any thread asks for wakes a phone's waiting event loop (engine/redraw.rs).
        if gpu.is_some() {
            crate::engine::redraw::install(&egui_ctx);
        }
        // Load + cache the in-app glossary (data/glossary.json) so
        // widgets::definition_text can pop term definitions on
        // Alt+hover. Silent no-op if the file is missing — definition
        // tooltips just won't appear. (v0.195.0.)
        crate::gui::glossary::install();
        let mut gui_state = GuiState::default();

        // Load data-driven catalogs into GUI state
        gui_state.external_catalog = crate::gui::load_external_catalog(&data_dir);
        gui_state.apps.own_tools = crate::gui::pages::apps::own_tools::load(&data_dir);
        gui_state.help_registry = help_modal::load_help_registry(&data_dir);
        // Keymaps used to load lazily on the first F1 press. The top-right help
        // button can open the same panel without F1 ever being touched, so they
        // are loaded up front now; the lazy path below stays as a safety net.
        gui_state.keymaps = crate::gui::pages::keymap::load_keymaps(&data_dir);
        gui_state.map_planets = crate::gui::load_planets();
        // Home storage at the start, from the sim side so the headless homestead has the same stores
        // (the saved pool replaces it below, after the config, when progress is being kept).
        crate::engine::sim_boot::seed_home_storage(&mut gui_state, &data_dir);
        gui_state.homestead_design = crate::gui::load_homestead_design(&data_dir);
        // Load the home's machine layout ONCE: both the Home-page self-sufficiency
        // loops AND the construction editor's editable machine layout come from it
        // (v0.519: machine placement). See docs/design/home-design.md.
        gui_state.home_machines = humanity_systems::grow_machines::load_home(&humanity_systems::persistence::live_save::active_home_machines_path(&data_dir));
        gui_state.homestead_loops = gui_state
            .home_machines
            .as_ref()
            .map(|h| h.loops.clone())
            .unwrap_or_default();
        gui_state.tower_configs = crate::gui::load_tower_configs(&data_dir);
        gui_state.garden_areas = crate::gui::load_garden_areas(&humanity_systems::persistence::live_save::active_home_machines_path(&data_dir), &data_dir);
        gui_state.grow_media = humanity_systems::grow_machines::load_grow_media(&data_dir);
        let lib = crate::gui::load_library(&data_dir);
        gui_state.library = lib.sections;
        gui_state.library_tags = lib.tag_groups;
        gui_state.curriculum = crate::gui::load_curriculum(&data_dir);
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
        let t_boot = std::time::Instant::now();
        load_data_registries(&mut data_store, &data_dir);
        opts.boot_timer.since("eager_data_registries", t_boot);
        // A pages-only start (a phone, engine/renderer_slot.rs) prebuilds neither the star sky nor
        // the planet surfaces below: both are the 3D world's, and on a phone nothing enters it. Should
        // something enter it later (HUMANITY_PAGES_FIRST), world_load.rs builds both itself.
        let pages_only = gpu.as_ref().is_some_and(|g| g.renderer.pages_only());
        // ── Background star-sky prebuild (v0.865 boot speed) ──
        // star_catalog_and_glow measured 4100 ms ON the world-entry path:
        // catalog read + vertex packing + glow PNG decode + GPU upload.
        // None of it needs the main thread (wgpu Device/Queue are Arc-backed
        // and thread-safe), so build the ENTIRE StarRenderer while the user
        // is still on the chat screen. Entering the world receives the
        // finished sky; only someone who beats the thread waits (and then
        // exactly as long as they would have anyway).
        let star_preload_rx = gpu.as_ref().filter(|_| !pages_only).and_then(|g| {
            let renderer = &g.renderer;
            let (tx, rx) = std::sync::mpsc::channel();
            let device = renderer.device.clone();
            let queue = renderer.queue.clone();
            let scene_format = renderer.scene_format();
            let sky_dir = data_dir.clone();
            let tier_setting = gui_state.settings.star_catalog_tier.clone();
            let ultra_glow = gui_state.settings.sky_glow_tier == "ultra";
            std::thread::spawn(move || {
                let t = std::time::Instant::now();
                let cap = crate::renderer::stars::StarCatalogTier::resolve_cap(&tier_setting);
                let built = crate::renderer::stars::StarCatalog::load(&sky_dir, cap)
                    .as_ref()
                    .and_then(|catalog| {
                        crate::renderer::stars::StarRenderer::new(
                            &device,
                            &queue,
                            scene_format,
                            catalog,
                            &sky_dir,
                            ultra_glow && cap >= 2,
                        )
                    });
                log::info!(
                    "[Boot] star sky prebuilt in background: {} ms ({})",
                    t.elapsed().as_millis(),
                    if built.is_some() { "ok" } else { "unavailable" }
                );
                let _ = tx.send(built);
            });
            Some(rx)
        });
        // ── Background planet-surface prebuild (2026-10-18 boot speed) ──
        // planet_defs_bake measured 1.2 s on the world-entry path: reading
        // data/planets/, loading or synthesizing each elevation grid,
        // loading each colour grid and baking each surface texture. All of
        // it is CPU work on files (net_route::bake_planet_defs), so it runs
        // here beside the star sky while the player is still on the menu;
        // the first world entry receives it and only installs and uploads
        // (world_load.rs). A planet RON hot reload before world entry drops
        // the receiver (reload_planet_defs), so a stale prebuild is never
        // installed. The baked textures (about 100 MB of RGBA for the four
        // textured bodies) wait in the channel until then.
        let planet_defs_preload_rx = if pages_only { None } else {
            let (tx, rx) = std::sync::mpsc::channel();
            let dir = data_dir.clone();
            std::thread::Builder::new()
                .name("planet-defs".into())
                .spawn(move || {
                    let t = std::time::Instant::now();
                    let bakes = crate::engine::net_route::bake_planet_defs(&dir);
                    let took = t.elapsed();
                    log::info!(
                        "[Boot] planet surfaces prebuilt in background: {} ms ({} bodies)",
                        took.as_millis(),
                        bakes.len()
                    );
                    let _ = tx.send((bakes, took));
                })
                .ok()
                .map(|_| rx)
        };
        // The starter vehicles of a brand-new home (engine/sim_boot.rs).
        crate::engine::sim_boot::starter_vehicles(&mut game_world, &data_store);
        gui_state.market_categories = crate::gui::load_market_categories(&data_dir);
        gui_state.studio_scene_presets = crate::gui::load_studio_scenes(&data_dir);
        gui_state.studio_source_presets = crate::gui::load_studio_sources(&data_dir);
        gui_state.profile_skills = crate::gui::load_default_player_skills(&data_dir);
        gui_state.studio_streaming_config = crate::gui::load_studio_streaming_config(&data_dir);
        gui_state.donate_faq = crate::gui::load_donate_faq(&data_dir);
        gui_state.donate_methods = crate::gui::load_donate_methods(&data_dir);
        gui_state.donate_charities = crate::gui::load_donate_charities(&data_dir);
        gui_state.donate_routes = crate::gui::load_donate_routes(&data_dir);
        gui_state.qa_test_tasks = crate::gui::load_qa_test_tasks(&data_dir);
        gui_state.web_sites = crate::gui::load_web_sites(&data_dir); // data/web/sites.json, both clients
        gui_state.web_view.rules = crate::gui::load_web_read_rules(&data_dir);
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
        // Prime the Program/Preview split: both panes start on scene 0, with
        // program holding a frozen copy of the just-loaded source arrangement
        // so the Program pane renders sensibly before the first manual Cut.
        gui_state.studio.cut_to_program();
        // Default to Earth if present.
        gui_state.map_selected_planet = gui_state
            .map_planets
            .iter()
            .position(|p| p.name == "Earth")
            .or(if gui_state.map_planets.is_empty() { None } else { Some(0) });

        // Load persistent config and apply to GUI state. Fresh installs
        // (no config.json yet) SKIP the apply so GuiState keeps its
        // designed defaults -- AppConfig's derived Default is all-zeroes
        // and used to stomp first boots to fov 0 / planet detail off /
        // no atmosphere / no clouds (see AppConfig::load_if_exists).
        if let Some(config) = boot_config {
            config.apply_to_gui_state(&mut gui_state);
        } else if crate::engine::low_impact::on() {
            // A rig sandbox with no config.json yet still runs light: the overlay over the
            // built-in defaults (none of it is ever saved back; engine::low_impact).
            crate::engine::low_impact::lay_over_config(crate::config::AppConfig::from_gui_state(&gui_state))
                .apply_to_gui_state(&mut gui_state);
        }
        // The proof a rig waits for (debug/low_impact.json) and the run.log line. A no-op in
        // a normal game.
        if opts.services { crate::engine::low_impact::write_confirmation(); }
        gui_state.save_slot = boot_slot;
        // The protected setup has its own file, read whether or not config.json exists or reads (10h).
        gui_state.protected.setup = crate::config::AppConfig::load_protected_setup();
        // The save's progress, now that the settings have landed (engine/sim_boot.rs).
        crate::engine::sim_boot::apply_home_save(&mut game_world, &mut data_store, &mut gui_state, home_save.as_ref(), &data_dir);
        // Bring the self-hosted relay node back up if it was running at
        // last exit (host_node_autostart, armed by Start / disarmed by
        // Stop). Must run after the config lands so port/db/name match.
        // A script-launched instance holds it back until the first click
        // (no firewall prompt from an agent's boot; see host_node::autostart).
        if opts.services { crate::gui::pages::host_node::autostart_if_configured(&gui_state, crate::engine::launch_focus::launch_in_background()); }
        // Push the LOADED settings into the engine on the first frame. Without this the
        // camera boots at CameraController::new's hardcoded sensitivity (and the camera
        // FOV / far-plane stay at their constructor defaults) until the user nudges a
        // slider, so a saved low/high mouse sensitivity never took effect on startup.
        // The settings_dirty block (in the render loop) applies fov + sensitivity +
        // window mode + render distance from gui_state.settings, so trip it once here.
        gui_state.settings_dirty = true;

        // Downloadable star catalogs installed? (v0.800 extended;
        // 2026-07-11 ultra) One metadata stat per tier at boot;
        // downloads/removals update the fields themselves.
        for tier in crate::renderer::stars::StarCatalogTier::ALL {
            gui_state.star_catalog_installed[tier.index()] =
                std::fs::metadata(data_dir.join(tier.filename())).ok().map(|m| m.len());
        }
        // Ultra Milky Way glow texture installed? (2026-07-11) Same
        // one-stat-at-boot pattern as the catalog tiers above.
        gui_state.galaxy_glow_installed =
            std::fs::metadata(data_dir.join(crate::renderer::stars::GALAXY_GLOW_ULTRA_FILE))
                .ok()
                .map(|m| m.len());

        // Clean up .old files from previous updates
        if opts.services { crate::updater::Updater::cleanup_old_versions(); }
        // A restart after an update comes back behind when this launch was (launch_focus).
        gui_state.updater.relaunch_behind = crate::engine::launch_focus::launch_in_background();
        gui_state.updater.relaunch_quiet = crate::engine::launch_focus::quiet_asked();

        // Auto-check for updates on startup (if enabled)
        if opts.services && gui_state.updater.channel == crate::updater::UpdateChannel::AlwaysLatest {
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
            let first = if gui_state.concept_tour_seen {
                gui_state.default_page
            } else {
                gui_state.concept_tour_seen = true;
                GuiPage::Humanity
            };
            // The first screen is a place the app opens by itself (gui/history.rs).
            gui_state.go(first, crate::gui::How::Replace);
        }

        let mut state = EngineState {
            // None for a background (script-launched) instance until a
            // person clicks into it (engine::launch_focus::launch_audio).
            audio: if opts.services { crate::engine::launch_focus::launch_audio() } else { None },
            // The data folder the app resolved, not a relative "data" (BUG-268).
            sound_catalog: humanity_core::audio::sounds::SoundCatalog::load(&data_dir),
            audio_volumes_applied: [-1.0; 6],
            music: Default::default(),
            animation: Default::default(),
            interact: Default::default(),
            tree_atlas_attempted: false,
            near_tree_retry_s: 0.0,
            particle_system: {
                let mut ps = crate::renderer::particles::ParticleSystem::new();
                ps.load_defs(&data_dir);
                ps
            },
            prev_ship_world_pos: glam::DVec3::ZERO,
            house_light_consumer: None,
            stride_accum: 0.0,
            prev_stride_anchor: glam::DVec3::ZERO,
            prev_stride_cam: Vec3::ZERO,
            debug_test_light_count: 0,
            debug_test_light_intensity: 3.0,
            gpu,
            gpu_dirty: Vec::new(),
            camera,
            controller,
            asset_manager,
            hot_reload,
            game_world,
            system_runner,
            data_store,
            // 3D world state: empty defaults, loaded lazily on first Enter World
            star_renderer: None,
            star_preload_rx,
            planet_defs_preload_rx,
            terrain_tiles: humanity_terrain::terrain_tiles::TerrainTiles::new(
                data_dir.join("planets/earth_tiles"),
            ),
            map_meshes: Default::default(),
            // Connected-ocean mask (v0.876): bathymetric terrain + the
            // water shell need it; absent/corrupt file = the pre-v0.876
            // clamped sea sphere, never a crash.
            ocean_mask: match humanity_terrain::ocean_mask::OceanMask::load(
                &data_dir.join("planets/earth_ocean_mask.bin"),
            ) {
                Ok(m) => Some(m),
                Err(e) => {
                    log::warn!("[Ocean] mask unavailable ({e}) - clamped sea sphere");
                    None
                }
            },
            // Live weather (v0.874): spawn the NASA GIBS fetcher only
            // when the setting allows network use. The renderer's
            // weather texture stays zero (= procedural sky) until the
            // first grid arrives.
            weather_grid: None,
            weather_rx: if opts.services && gui_state.settings.live_weather {
                let (tx, rx) = std::sync::mpsc::channel();
                crate::net::live_weather::spawn(tx);
                Some(rx)
            } else {
                None
            },
            current_spin: 0.0,
            clock: crate::engine::frame_sim::FrameClock::default(),
            station_world_pos: glam::DVec3::ZERO,
            station_world_rot: glam::DQuat::IDENTITY,
            station_def: humanity_ship::station::default_home(),
            station_ride: false,
            station_spawn_snap: false,
            station_off: Vec3::ZERO,
            aboard_station: false,
            planet_body_frame: None,
            flight_data: humanity_systems::flight::FlightData::load(&data_dir),
            flight: humanity_systems::flight::FlightState::default(),
            surface_walk_band: false,
            surface_owns_translation: false,
            planet_defs: std::collections::HashMap::new(),
            planet_heightmaps: std::collections::HashMap::new(),
            planet_albedos: std::collections::HashMap::new(),
            planet_mesh_cache: std::collections::HashMap::new(),
            sky_mesh_pending: std::collections::HashMap::new(),
            planet_chunk_states: std::collections::HashMap::new(),
            planet_patch_free_slots: Vec::new(),
            planet_surface_material: 0,
            planet_textured_materials: std::collections::HashMap::new(),
            planet_atmo_materials: std::collections::HashMap::new(),
            planet_cloud_materials: std::collections::HashMap::new(),
            planet_water_materials: std::collections::HashMap::new(),
            sun_world_pos: glam::DVec3::ZERO,
            sun_material: 0,
            solar_body_materials: [0; 8],
            solar_orbit_paths: Vec::new(),
            solar_orbit_paths_at: None,
            homestead_floors: Vec::new(),
            placeholder_objects: Vec::new(),
            machine_objects: Vec::new(),
            decoration_objects: Vec::new(),
            hero_plant_missing: std::collections::HashSet::new(),
            near_tree_new: Vec::new(),
            on_ground_planet: false,
            surface_vr: 0.0,
            last_ground_dir: glam::DVec3::ZERO,
            last_ground_r: 0.0,
            last_clearance: 0.0,
            near_tree_born_s: 0.0,
            machine_model_materials: std::collections::HashMap::new(),
            decoration_mesh_cache: std::collections::HashMap::new(),
            near_trees: Vec::new(),
            near_trees_center: glam::DVec3::splat(f64::MAX),
            near_grass: Vec::new(),
            near_grass_center: glam::DVec3::splat(f64::MAX),
            near_grass_depth: 0,
            near_tree_inputs: Default::default(),
            near_grass_at_s: -1.0,
            grass_instances: Vec::new(),
            grow_positions: Vec::new(),
            plant_objects: Vec::new(),
            plant_mesh_sig: 0,
            garden_draw: Default::default(),
            home_draw: Default::default(),
            machine_pick: Vec::new(),
            port_pick: Vec::new(),
            wall_colliders: Vec::new(),
            shut_doors: Vec::new(),
            pending_sfx: Vec::new(),
            pending_notice_sfx: Vec::new(),
            sight_colliders: Vec::new(),
            connection_objects: Vec::new(),
            connection_flow_paths: Vec::new(),
            connection_flow_sphere: None,
            port_node_mesh: None,
            rail_car_mesh: None,
            rail_car_mat: None,
            drone_dock_meshes: None,
            vehicle_meshes: None,
            structure_mesh: None,
            structure_mats: None,
            driving_vehicle: None,
            follow_vehicle: None,
            targeted_vehicle: None,
            vehicle_mats: None,
            stock_pile_mesh: None,
            stock_pile_mats: None,
            stock_pile_cache: (Default::default(), Vec::new()),
            targeted_livestock: None,
            pending_livestock_harvest: None,
            livestock_mesh: None,
            livestock_mats: std::collections::HashMap::new(),
            drone_dock_mats: None,
            drone_dock_anim: 1.0,
            flow_rgb_mats: Vec::new(),
            connection_cyl: None,
            connection_mats: std::collections::HashMap::new(),
            pipe_markers: Default::default(),
            door_panels: Vec::new(),
            door_manual_open: Vec::new(),
            door_locks: Vec::new(),
            door_swing: Vec::new(),
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
            ui_ipc: None,
            homestead_walls: None,
            homestead_material_walls: Vec::new(),
            homestead_trim: Vec::new(),
            homestead_windows: None,
            homestead_mirrors: None,
            homestead_ceiling: None,
            homestead_ceiling_opaque: None,
            homestead_ceiling_glass: false,
            homestead_roof_frame: None,
            homestead_hull: None,
            hull_profile: None,
            homestead_layout: None,
            construction_cam_active: false,
            construction_return_pos: Vec3::new(0.0, 1.7, 0.0),
            construction_entry_rebuild: false,
            cursor_pos: (0.0, 0.0),
            construction_grab: None,
            construction_ghost: None,
            construction_structure_ghost: None,
            teleport_cooldown: 0.0,
            elevator_state: std::collections::HashMap::new(),
            elevator_car_mesh: None,
            elevator_car_mat: None,
            wall_tool_mesh: None,
            wall_tool_mat: None,
            construction_node_grab: None,
            construction_object_grab: None,
            construction_port_drag: None,
            construction_snap_x: None,
            construction_snap_z: None,
            construction_grab_press: None,
            construction_node_mesh: None,
            construction_node_mat: None,
            construction_node_mat_hot: None,
            construction_light_mesh: None,
            construction_light_mat: None,
            construction_corridor_mouth_mat: None,
            light_fixture_sphere: None,
            light_fixture_panel: None,
            light_fixture_tube: None,
            light_fixture_mats: std::collections::HashMap::new(),
            light_fixture_mat_off: None,
            light_strip_meshes: std::collections::HashMap::new(),
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
            game_welcomed: false,
            copresence_refused: None,
            home_arrived_on: None,
            copresence_server: String::new(),
            boot_plot: None,
            last_welcome: None,
            last_welcome_rejoin: None,
            game_pos_timer: 0.0,
            remote_avatar: None,
            remote_look_materials: std::collections::HashMap::new(),
            remote_npc_avatar: None,
            hologram_objects: Vec::new(),
            hologram_orbits: Vec::new(),
            hologram_pins: Vec::new(),
            targeted_planet: None,
            hologram_room_center: Vec3::new(-0.5, 1.0, 2.5),
            room_lights: Vec::new(),
            homestead_bounds: None,
            ship_air: Default::default(),
            aboard_bounds: None,
            moves: Default::default(),
            map_view: Default::default(),
            shared_build: Default::default(),
            death_packs: Default::default(),
            screenshot_counter: 0,
            ship_world_pos: glam::DVec3::ZERO,
            dev_travel_home: None,
            dev_travel_stepped_out: false,
            frame_lock_body: None,
            probe_hold: None,
            rig_hold: None,
            station_park: None,
            draw_bufs: Default::default(),
            probe_descend_mps: 0.0,
            cloud_ref_frame: None,
            cloud_map_anchor: None,
            cloud_map_reanchors: 0,
            weather_anchor: None,
            cloud_map_regime: 0,
            cloud_prev_cam_local: None,
            cloud_prev_rot: None,
            sea_state_override: None,
            // Rig determinism pins, both off by default (showcase_request
            // {"wind":...} / {"anim_clock":...} turn them on).
            foliage_wind_override: None,
            fov_pin: None,
            anim_clock_pin: None,
            ocean_event_pin_request: None,
            ocean_event_pin: None,
            cloud_cover_override: None,
            cloud_type_override: None,
            ocean_fft_wind: 8.0,
            ocean_fft_built_wind: 8.0,
            ocean_fft_rebuild_cooldown: 0.0,
            bookmark_save_requested: false,
            frame_lock_anchor: glam::DVec3::ZERO,
            frame_lock_last_rot: glam::DQuat::IDENTITY,
            start_time: Instant::now(),
            last_frame: Instant::now(),
            window_focused: true,
            tray: Default::default(),
            redraw: Default::default(),
            // Same policy as window creation (cached, so they always
            // agree): a script-launched boot must not grab the cursor
            // either. See src/engine/launch_focus.rs.
            background_no_cursor: crate::engine::launch_focus::launch_in_background(),
            far_tree_mesh: None,
            far_tree_anchor: glam::DVec3::ZERO,
            far_tree_built_cam: glam::DVec3::splat(1.0e30),
            far_tree_pending: None,
            ocean_fft: None,
            cloud_advect: 0.0,
            cloud_advect_decaying: 0.0,
            cloud_event_boost: 0.0,
            cloud_event_tint: [1.0; 3],
            event_core: None,
            event_hazard_near: false,
            egui_ctx,
            gui_state,
            theme,
            screens: crate::engine::screens::Screens::default(),
            world_loaded: false,
            boot_timer: opts.boot_timer,
            data_dir,
            saves_dir: opts.saves_dir.clone(),
            ctrl_held: false,
            shift_held: false,
            alt_held: false,
            lmb_held: false,
            construction_history: ConstructionHistory::default(),
            live_publisher: None,
            movie: None,
            stream_capture: crate::renderer::stream_capture::StreamCapture::new(),
        };
        // Interior (ship / homestead) gravity from data/game.csv, the
        // first of that file's knobs to gain a real reader (v0.1119).
        // Missing/garbled knob keeps the controller's compiled-in 1 g.
        // The hot-reload poll re-applies this on every game.csv save.
        {
            let st = &mut state;
            if let Some(g) = st.asset_manager.game_setting_f64("gravity_m_s2") {
                st.controller.set_interior_gravity(g as f32);
            }
        }
        state
    }
}
