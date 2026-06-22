//! Headless UI snapshots (v0.495). Renders native egui pages to PNG images
//! offscreen, WITHOUT opening a window, so UI changes can be reviewed (and
//! regression-checked) from an image rather than only by a human at the app.
//!
//! It drives the app's OWN egui + egui-wgpu + wgpu against an offscreen texture,
//! so there is no extra dependency. The PNGs land in `tests/snapshots/`.
//! Generate / refresh them with `just snapshots`, then open the PNGs to review
//! the UI. (These currently GENERATE the images; pixel-diff regression checking
//! is a later add.)
//!
//! Note: this needs a GPU adapter (the dev machine has one). On a headless CI box
//! without a GPU the render is skipped with a printed note rather than failing.
#![cfg(all(test, feature = "native"))]

use crate::gui::theme::{load_theme, Theme};
use crate::gui::{
    ChatChannel, ChatDm, ChatMessage, ChatServer, ChatUser, GuiAsteroid, GuiCalendarEvent, GuiCrop,
    GuiItemSlot, GuiListing, GuiNote, GuiQuest, GuiSkill, GuiState, GuiTask, GuiVitals, TaskPriority,
    TaskStatus, WalletTransaction,
};

/// Build a `GuiState` populated with REALISTIC demo content so the snapshots
/// reflect the loaded app, not the empty first-run state. Data-driven fields use
/// the real loaders (reading `data/`, cwd = repo root under `cargo test`); the
/// ECS-synced + dynamic fields (vitals, crops, asteroids, chat, tasks, …) are
/// filled with representative sample values that mirror what the live main loop
/// bridges in each frame. Keep this in sync with the GuiState fields the pages read.
fn demo_state() -> GuiState {
    let mut s = GuiState::default();
    let data = std::path::Path::new("data");

    // ── Data-driven content (the real loaders) ──
    s.places = crate::gui::load_places(data);
    s.tower_configs = crate::gui::load_tower_configs(data);
    s.equipment_slots = crate::gui::load_equipment_slots(data);
    s.crafting_category_groups = crate::gui::load_crafting_category_groups(data);
    s.craft_recipes = crate::gui::load_crafting_recipes(data);
    s.market_categories = crate::gui::load_market_categories(data);
    s.library = crate::gui::load_library(data);
    s.garden_areas = crate::gui::load_garden_areas(data);
    s.grow_media = crate::gui::load_grow_media(data);
    s.onboarding_quest_chains = crate::gui::pages::onboarding::load_quest_chains(data);
    s.creative_mode = true;
    // Returning-user state so the main menu shows the loaded hub, not first-run onboarding.
    s.onboarding_complete = true;
    // Start the inventory trees expanded so snapshots show the nested contents.
    s.trees_start_collapsed = false;

    // ── Inventory: Status vitals ──
    s.vitals = GuiVitals {
        satiation: 62.0,
        hydration: 48.0,
        energy: 80.0,
        oxygen: 100.0,
        body_temp_c: 37.0,
        waste: 30.0,
        satiation_max: 100.0,
        hydration_max: 100.0,
        energy_max: 100.0,
        oxygen_max: 100.0,
        waste_max: 100.0,
        sealed: true,
        effects: vec![("Well-fed".into(), 180.0), ("Rested".into(), 90.0)],
    };
    let mut items = vec![
        Some(GuiItemSlot { item_id: "water_bottle_0".into(), name: "Water Bottle".into(), quantity: 2 }),
        Some(GuiItemSlot { item_id: "bread_0".into(), name: "Bread".into(), quantity: 5 }),
        Some(GuiItemSlot { item_id: "iron_ore_0".into(), name: "Iron Ore".into(), quantity: 6 }),
    ];
    // A big flat seed list, to exercise the multi-column leaf layout.
    for s_name in [
        "Lettuce", "Spinach", "Kale", "Cabbage", "Broccoli", "Cauliflower", "Beet", "Turnip",
        "Radish", "Carrot", "Parsnip", "Celery", "Leek", "Onion", "Garlic", "Chive", "Cucumber",
        "Zucchini", "Tomato", "Bell Pepper", "Eggplant", "Okra", "Strawberry", "Bean",
    ] {
        items.push(Some(GuiItemSlot {
            item_id: format!("seed_{}_0", s_name.to_lowercase().replace(' ', "_")),
            name: format!("{} Seeds", s_name),
            quantity: 1,
        }));
    }
    s.inventory_items = items;

    // ── Garden: planted crops in towers ──
    s.crops = vec![
        GuiCrop {
            name: "Lettuce".into(),
            stage: "Mature".into(),
            progress: 1.0,
            water: 80.0,
            health: 95.0,
            mature: true,
            tower_id: Some("helix_wide_60".into()),
            tower_slot: Some(0),
            water_per_day: 0.5,
            temp_min: 5.0,
            temp_max: 24.0,
            ..Default::default()
        },
        GuiCrop {
            name: "Basil".into(),
            stage: "Growing".into(),
            progress: 0.55,
            water: 60.0,
            health: 88.0,
            tower_id: Some("helix_slim_32".into()),
            tower_slot: Some(2),
            ..Default::default()
        },
    ];

    // ── Mining: asteroids with remaining ore ──
    s.asteroids = vec![
        GuiAsteroid {
            id: "m12".into(),
            name: "Asteroid M-12 (metallic)".into(),
            classification: "M".into(),
            ores: vec![("iron_ore_0".into(), 120.0), ("nickel_ore_0".into(), 60.0), ("platinum_ore_0".into(), 20.0)],
            position: [60.0, 12.0, -30.0],
            distance: 68.1,
        },
        GuiAsteroid {
            id: "s7".into(),
            name: "Asteroid S-7 (silicaceous)".into(),
            classification: "S".into(),
            ores: vec![("iron_ore_0".into(), 40.0), ("copper_ore_0".into(), 50.0)],
            position: [-45.0, 8.0, 55.0],
            distance: 71.5,
        },
    ];

    // ── Skills + quests ──
    s.skills = vec![
        GuiSkill { id: "farming".into(), name: "Farming".into(), category: "Survival".into(), level: 4, xp: 120, xp_needed: 200 },
        GuiSkill { id: "mining".into(), name: "Mining".into(), category: "Survival".into(), level: 2, xp: 40, xp_needed: 120 },
        GuiSkill { id: "crafting".into(), name: "Crafting".into(), category: "Production".into(), level: 3, xp: 75, xp_needed: 150 },
    ];
    s.quests = vec![
        GuiQuest { name: "First Harvest".into(), step_index: 1, step_total: 3, step_desc: "Plant a seed in a tower".into(), completed: false },
        GuiQuest { name: "Welcome to HumanityOS".into(), step_index: 0, step_total: 0, step_desc: String::new(), completed: true },
    ];

    // ── Chat ──
    s.ws_status = "Connected".into();
    s.chat_active_channel = "general".into();
    let mk_channel = |id: &str, name: &str, voice: bool, ro: bool| ChatChannel {
        id: id.into(),
        name: name.into(),
        description: String::new(),
        category: "Text".into(),
        voice_joined: false,
        voice_enabled: voice,
        read_only: ro,
        federated: true,
        voice_participants: vec![],
    };
    s.chat_channels = vec![
        mk_channel("general", "general", true, false),
        mk_channel("announcements", "announcements", false, true),
        mk_channel("garden", "garden", true, false),
    ];
    s.chat_messages = vec![
        ChatMessage { sender_name: "Shaostoul".into(), content: "Welcome to HumanityOS!".into(), timestamp: "12:30".into(), channel: "general".into(), ..Default::default() },
        ChatMessage { sender_name: "Ada".into(), content: "The garden towers are looking great today.".into(), timestamp: "12:32".into(), channel: "general".into(), ..Default::default() },
        ChatMessage { sender_name: "Shaostoul".into(), content: "Shipping the Laws page next.".into(), timestamp: "12:35".into(), channel: "general".into(), ..Default::default() },
    ];
    s.chat_users = vec![
        ChatUser { name: "Shaostoul".into(), public_key: "ed25519:abc".into(), role: "admin".into(), status: "online".into() },
        ChatUser { name: "Ada".into(), public_key: "ed25519:def".into(), role: "member".into(), status: "online".into() },
    ];
    s.chat_dms = vec![ChatDm { user_name: "Ada".into(), user_key: "ed25519:def".into(), last_message: "See you at the build".into(), timestamp: "11:02".into(), unread: true }];
    s.chat_servers = vec![ChatServer {
        name: "United Humanity".into(),
        channels: s.chat_channels.clone(),
        voice_channels: vec![],
        id: "srv_united".into(),
        url: "https://united-humanity.us".into(),
        connected: true,
    }];

    // ── Tasks (one per kanban column) ──
    s.tasks = vec![
        GuiTask { id: 1, title: "Plant the spring greens".into(), description: String::new(), priority: TaskPriority::High, status: TaskStatus::Todo, assignee: "Shaostoul".into(), labels: vec!["garden".into()] },
        GuiTask { id: 2, title: "Wire the mining drone manifest".into(), description: String::new(), priority: TaskPriority::Medium, status: TaskStatus::InProgress, assignee: "Ada".into(), labels: vec![] },
        GuiTask { id: 3, title: "Ship the Laws page".into(), description: String::new(), priority: TaskPriority::Low, status: TaskStatus::Done, assignee: "Shaostoul".into(), labels: vec!["ui".into()] },
    ];
    s.task_next_id = 4;

    // ── Market listings ──
    s.listings = vec![
        GuiListing { id: 1, title: "Helix Wide 60 tower".into(), description: "33-slot aeroponic tower".into(), price: 120.0, seller: "Shaostoul".into(), category: "Equipment".into() },
        GuiListing { id: 2, title: "Heirloom seed pack".into(), description: "Greens + herbs".into(), price: 8.0, seller: "Ada".into(), category: "Seeds".into() },
    ];
    s.listing_next_id = 3;

    // ── Notes ──
    s.notes = vec![
        GuiNote { id: 1, title: "Garden plan".into(), content: "Tower A: greens. Tower B: herbs.".into(), modified: 0 },
        GuiNote { id: 2, title: "Mining route".into(), content: "M-12 then S-7.".into(), modified: 0 },
    ];
    s.notes_selected = Some(1);
    s.notes_next_id = 3;

    // ── Calendar ──
    s.cal_year = 2026;
    s.cal_month = 6;
    s.cal_selected_day = 21;
    s.cal_events = vec![
        GuiCalendarEvent { title: "Harvest lettuce".into(), year: 2026, month: 6, day: 21, time: "09:00".into(), color: egui::Color32::from_rgb(80, 180, 80) }, // theme-exempt: demo sample event color (test fixture)
        GuiCalendarEvent { title: "Volunteer (Sponsor-a-Can)".into(), year: 2026, month: 6, day: 23, time: "08:00".into(), color: egui::Color32::from_rgb(100, 140, 200) }, // theme-exempt: demo sample event color (test fixture)
    ];

    // ── Wallet ──
    s.wallet_balance = 12.4;
    s.wallet_address = "7xKQ9fAb...3mNp".into();
    s.wallet_sol_price = 150.0;
    s.wallet_transactions = vec![WalletTransaction { signature: "5gH...zP".into(), direction: "in".into(), amount: 2.0, counterparty: "Ada".into(), timestamp: "2026-06-20".into() }];

    // ── Profile ──
    s.profile_name = "Shaostoul".into();
    s.profile_bio = "Building HumanityOS to end poverty and unite humanity.".into();
    s.profile_pronouns = "he/him".into();
    s.profile_location = "Silverdale, WA".into();
    s.profile_website = "united-humanity.us".into();
    s.profile_public_key = "ed25519:abc...def".into();
    // Show the Body & Measurements section so the two-column layout is visible.
    s.profile_section = crate::gui::ProfileSection::BodyMeasurements;
    s.profile_height = "5'10\"".into();
    s.profile_weight = "170 lb".into();
    s.profile_eye_color = "Brown".into();
    s.profile_blood_type = "O+".into();
    s.profile_hair_color = "Brown".into();
    s.profile_shoe_size = "10".into();
    s.profile_shirt_size = "L".into();
    s.profile_pants_size = "32x32".into();

    s
}

/// Render one settings-style page into an offscreen `w`x`h` surface and write
/// `tests/snapshots/<name>.png`.
fn render_page_png(name: &str, w: u32, h: u32, frame: impl Fn(&egui::Context, &Theme, &mut GuiState)) {
    pollster::block_on(async move {
        // ── wgpu device (offscreen) ──
        let instance = wgpu::Instance::default();
        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
        {
            Some(a) => a,
            None => {
                eprintln!("ui_snapshots: no GPU adapter; skipping {name}");
                return;
            }
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .expect("request_device");
        let format = wgpu::TextureFormat::Rgba8Unorm;

        // ── egui frame ──
        let ctx = egui::Context::default();
        let theme = load_theme();
        theme.apply_to_egui(&ctx);
        let mut state = demo_state();
        let ppp = 1.0_f32;
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(w as f32, h as f32),
            )),
            ..Default::default()
        };
        // ── egui-wgpu renderer ──
        let mut renderer = egui_wgpu::Renderer::new(&device, format, None, 1, false);
        // egui Windows/Areas measure their size on the first frame and only settle
        // their position on the second, so a single-frame render leaves Window-based
        // pages (the main menu hub/onboarding, modals) blank. Run a warm-up frame,
        // then capture the second — applying BOTH frames' texture deltas (the font
        // atlas is created on frame 1).
        let warm = ctx.run(raw_input.clone(), |ctx| {
            frame(ctx, &theme, &mut state);
        });
        for (id, delta) in &warm.textures_delta.set {
            renderer.update_texture(&device, &queue, *id, delta);
        }
        let full_output = ctx.run(raw_input, |ctx| {
            frame(ctx, &theme, &mut state);
        });
        for (id, delta) in &full_output.textures_delta.set {
            renderer.update_texture(&device, &queue, *id, delta);
        }
        let clipped = ctx.tessellate(full_output.shapes, ppp);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [w, h],
            pixels_per_point: ppp,
        };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        renderer.update_buffers(&device, &queue, &mut encoder, &clipped, &screen);

        // ── offscreen target ──
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ui_snapshot"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let bg = theme.bg_primary();
        {
            let mut rpass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ui_snapshot_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: srgb_to_lin(bg.r()),
                                g: srgb_to_lin(bg.g()),
                                b: srgb_to_lin(bg.b()),
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            renderer.render(&mut rpass, &clipped, &screen);
        }

        // ── copy texture -> buffer (rows padded to 256) ──
        let bpr = ((w * 4 + 255) / 256) * 256;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ui_snapshot_readback"),
            size: (bpr * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit([encoder.finish()]);

        // ── map + unpad + save ──
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::Maintain::Wait);
        let data = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h {
            let start = (row * bpr) as usize;
            pixels.extend_from_slice(&data[start..start + (w * 4) as usize]);
        }
        drop(data);
        buffer.unmap();

        std::fs::create_dir_all("tests/snapshots").ok();
        let img = image::RgbaImage::from_raw(w, h, pixels).expect("image from pixels");
        let path = format!("tests/snapshots/{name}.png");
        img.save(&path).expect("save png");
        println!("ui_snapshots: wrote {path}");
    });
}

/// egui clear colors are sRGB bytes; the Rgba8Unorm target wants linear floats.
fn srgb_to_lin(c: u8) -> f64 {
    let s = c as f64 / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// A settings sub-panel (which expects a `&mut Ui`) wrapped into a full
/// ctx-level frame the renderer can drive.
fn settings_panel(
    ctx: &egui::Context,
    theme: &Theme,
    state: &mut GuiState,
    draw: impl Fn(&mut egui::Ui, &Theme, &mut GuiState),
) {
    theme.apply_to_egui(ctx);
    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            draw(ui, theme, state);
        });
    });
}

#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_audio_settings() {
    render_page_png("audio_settings", 960, 1100, |ctx, theme, state| {
        settings_panel(ctx, theme, state, crate::gui::pages::settings::draw_audio_content);
    });
}

#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_graphics_settings() {
    render_page_png("graphics_settings", 960, 900, |ctx, theme, state| {
        settings_panel(ctx, theme, state, crate::gui::pages::settings::draw_graphics_content);
    });
}

#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_controls_settings() {
    render_page_png("controls_settings", 960, 900, |ctx, theme, state| {
        settings_panel(ctx, theme, state, crate::gui::pages::settings::draw_controls_content);
    });
}

#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_laws_page() {
    render_page_png("laws_page", 1400, 1400, |ctx, theme, state| {
        crate::gui::pages::laws::draw(ctx, theme, state);
    });
}

// Whole-page reviews. Each is its own test so a panic in one (a page that needs
// state the default GuiState lacks) does not stop the others; run with
// `--no-fail-fast`. Use `just snapshots` then open tests/snapshots/*.png.
macro_rules! page_snapshot {
    ($test:ident, $name:literal, $page:ident, $w:literal, $h:literal) => {
        #[test]
        #[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
        fn $test() {
            render_page_png($name, $w, $h, |ctx, theme, state| {
                crate::gui::pages::$page::draw(ctx, theme, state);
            });
        }
    };
}

page_snapshot!(snapshot_main_menu, "main_menu", main_menu, 1280, 900);
page_snapshot!(snapshot_humanity, "humanity", humanity, 1280, 900);
page_snapshot!(snapshot_chat, "chat", chat, 1280, 900);
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_inventory() {
    render_page_png("inventory", 1280, 1700, |ctx, theme, state| {
        // Reset any modal opened by a prior modal snapshot (shared thread).
        crate::gui::pages::inventory::test_close_garden_edit();
        crate::gui::pages::inventory::test_close_mining_edit();
        crate::gui::pages::inventory::draw(ctx, theme, state);
    });
}

#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_garden_modal_soil() {
    render_page_png("garden_modal_soil", 900, 980, |ctx, theme, state| {
        crate::gui::pages::inventory::test_open_garden_edit("potato_grow_bed");
        crate::gui::pages::inventory::draw(ctx, theme, state);
    });
}

#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_mining_modal() {
    render_page_png("mining_modal", 900, 980, |ctx, theme, state| {
        crate::gui::pages::inventory::test_open_mining_edit("m12");
        crate::gui::pages::inventory::draw(ctx, theme, state);
    });
}

#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_mining_map() {
    render_page_png("mining_map", 600, 240, |ctx, theme, state| {
        let drones = vec![crate::gui::GuiDrone {
            manifest: vec![("iron_ore_0".into(), 6)],
            phase: "Outbound".into(),
            cargo_total: 0,
            phase_progress: 0.5,
            target: "m12".into(),
            distance: 68.1,
            pos: [30.0, 6.0, -15.0],
        }];
        theme.apply_to_egui(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme.bg_primary()).inner_margin(12.0))
            .show(ctx, |ui| {
                crate::gui::pages::inventory::draw_mining_map_for_test(ui, theme, &state.asteroids, &drones);
            });
    });
}

#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_garden_modal_tower() {
    render_page_png("garden_modal_tower", 900, 980, |ctx, theme, state| {
        crate::gui::pages::inventory::test_open_garden_edit("aeroponic_tower_nutrition");
        crate::gui::pages::inventory::draw(ctx, theme, state);
    });
}
page_snapshot!(snapshot_tasks, "tasks", tasks, 1280, 900);
page_snapshot!(snapshot_market, "market", market, 1280, 900);
page_snapshot!(snapshot_profile, "profile", profile, 1280, 900);
page_snapshot!(snapshot_crafting, "crafting", crafting, 1280, 900);
page_snapshot!(snapshot_library, "library", library, 1280, 900);
page_snapshot!(snapshot_governance, "governance", governance, 1280, 900);
page_snapshot!(snapshot_identity, "identity", identity, 1280, 900);
page_snapshot!(snapshot_wallet, "wallet", wallet, 1280, 900);
page_snapshot!(snapshot_quests, "quests", quests, 1280, 900);
page_snapshot!(snapshot_calendar, "calendar", calendar, 1280, 900);
page_snapshot!(snapshot_notes, "notes", notes, 1280, 900);
