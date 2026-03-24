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
pub mod updater;

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
        tasks, profile, maps, market, calculator, calendar, notes, civilization,
        wallet, crafting, guilds, trade, files, bugs, resources, donate, tools,
    };
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
    /// Checks: ./data, ./content/data, ../data, ../content/data
    fn find_data_dir() -> PathBuf {
        let exe = std::env::current_exe().unwrap_or_default();
        let exe_dir = exe.parent().unwrap_or(std::path::Path::new("."));

        for candidate in &[
            exe_dir.join("data"),
            exe_dir.join("content").join("data"),
            exe_dir.parent().unwrap_or(exe_dir).join("data"),
            exe_dir.parent().unwrap_or(exe_dir).join("content").join("data"),
            // Dev mode: repo root data directory
            PathBuf::from("data"),
        ] {
            if candidate.exists() && candidate.is_dir() {
                log::info!("Data directory: {}", candidate.display());
                return candidate.clone();
            }
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
        planet: Option<PlanetRenderer>,
        planet_mesh: Option<usize>,
        planet_material: usize,
        cube_mesh: usize,
        plane_mesh: usize,
        cube_material: usize,
        green_material: usize,
        blue_material: usize,
        yellow_material: usize,
        start_time: Instant,
        last_frame: Instant,
        // egui integration
        egui_ctx: egui::Context,
        egui_state: egui_winit::State,
        egui_renderer: egui_wgpu::Renderer,
        gui_state: GuiState,
        theme: Theme,
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
                .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));

            let window = Arc::new(
                event_loop
                    .create_window(window_attrs)
                    .expect("Failed to create window"),
            );

            // Initialize renderer (block on async)
            let mut renderer = pollster::block_on(Renderer::new_native(window.clone()));

            // Create meshes
            let cube_mesh = renderer.add_mesh(Mesh::cube(&renderer.device));
            let plane_mesh = renderer.add_mesh(Mesh::plane(&renderer.device));

            // Create materials
            let cube_material =
                renderer.add_material([0.8, 0.3, 0.2, 1.0], 0.0, 0.5);
            let green_material =
                renderer.add_material([0.3, 0.5, 0.3, 1.0], 0.0, 0.8);
            let blue_material =
                renderer.add_material([0.2, 0.4, 0.8, 1.0], 0.3, 0.4);
            let yellow_material =
                renderer.add_material([0.9, 0.8, 0.2, 1.0], 0.0, 0.6);

            let mut camera = Camera::new();
            camera.aspect = renderer.aspect_ratio();

            let controller = CameraController::new(5.0, 3.0);

            // Initialize data loading system
            let data_dir = find_data_dir();
            let mut asset_manager = AssetManager::new(data_dir.clone());
            let hot_reload = HotReloadCoordinator::new(&data_dir);

            // Load core data files at startup
            // Items
            #[derive(Debug, serde::Deserialize)]
            #[allow(dead_code)]
            struct ItemRow { id: String, name: String }
            match asset_manager.load_csv::<ItemRow>("items.csv") {
                Ok(items) => log::info!("Loaded {} items", items.len()),
                Err(e) => log::warn!("Could not load items.csv: {e}"),
            }

            // Plants
            #[derive(Debug, serde::Deserialize)]
            #[allow(dead_code)]
            struct PlantRow { id: String, name: String }
            match asset_manager.load_csv::<PlantRow>("plants.csv") {
                Ok(plants) => log::info!("Loaded {} plants", plants.len()),
                Err(e) => log::warn!("Could not load plants.csv: {e}"),
            }

            // Recipes
            #[derive(Debug, serde::Deserialize)]
            #[allow(dead_code)]
            struct RecipeRow { id: String, name: String }
            match asset_manager.load_csv::<RecipeRow>("recipes.csv") {
                Ok(recipes) => log::info!("Loaded {} recipes", recipes.len()),
                Err(e) => log::warn!("Could not load recipes.csv: {e}"),
            }

            // Initialize ECS
            let mut game_world = GameWorld::new();
            let mut system_runner = SystemRunner::new();
            let mut data_store = DataStore::new();

            // Seed DataStore with initial shared state
            data_store.insert("input_state", InputState::default());
            data_store.insert("camera_position", Vec3::new(0.0, 2.0, 5.0));
            data_store.insert("camera_forward", Vec3::NEG_Z);
            data_store.insert("camera_yaw", 0.0_f32);

            // Register all game systems (tick order matters)
            system_runner.register(TimeSystem::new());
            system_runner.register(PlayerControllerSystem);
            system_runner.register(InteractionSystem);
            system_runner.register(FarmingSystem::new());
            system_runner.register(InventorySystem::new());
            system_runner.register(CraftingSystem::new());

            // Spawn a player entity with core components
            game_world.world.spawn((
                Transform::default(),
                Velocity::default(),
                Controllable,
                Health::default(),
                Name("Player".to_string()),
                Inventory::new(36),
            ));

            log::info!("ECS initialized: {} systems registered", system_runner.count());

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
            let gui_state = GuiState::default();

            // Try to load a planet from data files
            let planet_material = renderer.add_material([0.3, 0.5, 0.2, 1.0], 0.0, 0.7);
            let (planet, planet_mesh) = match asset_manager.load_ron::<PlanetDef>("planets/earth.ron") {
                Ok(def) => {
                    log::info!("Loaded planet: {} (radius: {}m)", def.name, def.radius);
                    let mut pr = PlanetRenderer::new(def.clone(), Vec3::new(0.0, 0.0, -20.0));
                    // Start at a viewable LOD (subdivision 2 for demo)
                    let ico = pr.icosphere();
                    let mesh_idx = renderer.add_mesh(Mesh::from_icosphere(&renderer.device, ico, 5.0));
                    (Some(pr), Some(mesh_idx))
                }
                Err(e) => {
                    log::warn!("Could not load planet: {e}");
                    (None, None)
                }
            };

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
                planet,
                planet_mesh,
                planet_material,
                cube_mesh,
                plane_mesh,
                cube_material,
                green_material,
                blue_material,
                yellow_material,
                start_time: Instant::now(),
                last_frame: Instant::now(),
                egui_ctx,
                egui_state,
                egui_renderer,
                gui_state,
                theme,
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

                        // Escape: None -> EscapeMenu, EscapeMenu -> None, any page -> EscapeMenu
                        if key == KeyCode::Escape && pressed {
                            let old_page = state.gui_state.active_page;
                            state.gui_state.active_page = match old_page {
                                GuiPage::None => GuiPage::EscapeMenu,
                                GuiPage::EscapeMenu => GuiPage::None,
                                GuiPage::MainMenu => GuiPage::MainMenu, // don't escape from title
                                _ => GuiPage::EscapeMenu, // any tool page -> back to menu
                            };
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

                    // Spinning cube
                    let elapsed = (now - state.start_time).as_secs_f32();
                    let cube_rotation =
                        Quat::from_euler(glam::EulerRot::YXZ, elapsed * 0.7, elapsed * 0.5, 0.0);

                    let objects = [
                        // Center cube (spinning, red)
                        RenderObject {
                            position: Vec3::new(0.0, 1.0, 0.0),
                            rotation: cube_rotation,
                            scale: Vec3::ONE,
                            mesh: state.cube_mesh,
                            material: state.cube_material,
                        },
                        // Blue cube at +X
                        RenderObject {
                            position: Vec3::new(4.0, 0.5, 0.0),
                            rotation: Quat::IDENTITY,
                            scale: Vec3::ONE,
                            mesh: state.cube_mesh,
                            material: state.blue_material,
                        },
                        // Yellow cube at -Z
                        RenderObject {
                            position: Vec3::new(0.0, 0.5, -4.0),
                            rotation: Quat::from_rotation_y(0.5),
                            scale: Vec3::splat(0.7),
                            mesh: state.cube_mesh,
                            material: state.yellow_material,
                        },
                        // Ground plane
                        RenderObject {
                            position: Vec3::ZERO,
                            rotation: Quat::IDENTITY,
                            scale: Vec3::ONE,
                            mesh: state.plane_mesh,
                            material: state.green_material,
                        },
                    ];

                    // Add planet to render list if loaded
                    let mut all_objects = objects.to_vec();
                    if let (Some(_planet), Some(mesh_idx)) = (&state.planet, state.planet_mesh) {
                        all_objects.push(RenderObject {
                            position: Vec3::new(0.0, 5.0, -20.0),
                            rotation: Quat::from_rotation_y(elapsed * 0.1),
                            scale: Vec3::ONE,
                            mesh: mesh_idx,
                            material: state.planet_material,
                        });
                    }

                    // Update FPS counter
                    state.gui_state.fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };

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

                    // Track page before egui frame for cursor grab transitions
                    let page_before_frame = state.gui_state.active_page;

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
                        // In-game: full 3D scene render
                        state.renderer.render_scene(&state.camera, &all_objects)
                    };
                    match scene_result {
                        Ok((surface_texture, view)) => {
                            // Run egui frame
                            let raw_input = state.egui_state.take_egui_input(&state.window);
                            let full_output = state.egui_ctx.run(raw_input, |ctx| {
                                // Show RGB nav bar on all pages except None and MainMenu
                                match state.gui_state.active_page {
                                    GuiPage::None | GuiPage::MainMenu => {}
                                    _ => {
                                        escape_menu::draw_nav_bar(ctx, &mut state.gui_state);
                                    }
                                }

                                // Draw active full-screen page
                                match state.gui_state.active_page {
                                    GuiPage::MainMenu => {
                                        main_menu::draw(ctx, &state.theme, &mut state.gui_state);
                                    }
                                    GuiPage::EscapeMenu => {
                                        escape_menu::draw(ctx, &mut state.gui_state);
                                    }
                                    GuiPage::Settings => {
                                        settings::draw(ctx, &state.theme, &mut state.gui_state);
                                    }
                                    GuiPage::Inventory => {
                                        inventory::draw(ctx, &state.theme, &mut state.gui_state);
                                    }
                                    GuiPage::Chat => {
                                        chat::draw(ctx, &state.theme, &mut state.gui_state);
                                    }
                                    // Placeholder pages (web versions exist, native coming)
                                    GuiPage::Tasks => tasks::draw(ctx, &state.theme, &mut state.gui_state),
                                    GuiPage::Maps => maps::draw(ctx, &state.theme, &mut state.gui_state),
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
                                    GuiPage::None => {}
                                }

                                // Always draw HUD when in-game
                                if state.gui_state.active_page == GuiPage::None && state.gui_state.show_hud {
                                    hud::draw(ctx, &state.theme, &state.gui_state, state.camera.yaw);
                                }

                                // Draw chat overlay if visible (only in-game)
                                if state.gui_state.active_page == GuiPage::None && state.gui_state.show_chat {
                                    chat::draw(ctx, &state.theme, &mut state.gui_state);
                                }

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
