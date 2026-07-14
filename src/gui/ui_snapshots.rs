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
    s.placed_items = crate::gui::flatten_placed_items(&s.places);
    s.homestead_design = crate::gui::load_homestead_design(data);
    // Sample LIVE power so the Home page's "Live power" card renders in the snapshot
    // (in the app these come from ElectricalSystem via PowerStatus, v0.518).
    s.power_generation = 3200.0;
    s.power_consumption = 1850.0;
    s.power_balance = 1350.0;
    s.power_battery_wh = 9600.0;
    s.power_battery_capacity_wh = 16000.0;
    s.power_autonomy_hours = 5.2;
    // Sample LIVE water so the Home page's "Live water" card renders (PlumbingSystem, v0.608).
    s.water_production_lpm = 12.3;
    s.water_demand_lpm = 1.2;
    s.water_stored_l = 7400.0;
    s.water_capacity_l = 8000.0;
    s.water_days_autonomy = 4.3;
    // Sample LIVE air so the Home page's "Live air" card renders (AtmosphereSystem, v0.617).
    s.air_o2_pct = 20.9;
    s.air_co2_pct = 0.04;
    s.air_pressure_atm = 1.0;
    s.air_temp_c = 20.0;
    s.air_breathable = true;
    s.tower_configs = crate::gui::load_tower_configs(data);
    // Loop-closure summary (Home page "Closed-loop self-sufficiency" card) comes from
    // the home machine layout; load the seed home.ron directly (same as
    // snapshot_construction) so the closed loops render adjacent to the
    // "What one home cannot close" panel (which self-loads its own RON).
    s.homestead_loops = crate::machines::MachineHome::load(
        &data.join("machines").join("home.ron"),
    )
    .map(|h| h.loops)
    .unwrap_or_default();
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
        unread: false,
    };
    s.chat_channels = vec![
        mk_channel("general", "general", true, false),
        mk_channel("announcements", "announcements", false, true),
        mk_channel("garden", "garden", true, false),
    ];
    // One unread channel so the sidebar's channel-unread dot (v0.718) stays
    // covered by the chat snapshot.
    s.chat_channels[2].unread = true;
    s.chat_messages = vec![
        ChatMessage { sender_name: "Shaostoul".into(), content: "Welcome to HumanityOS!".into(), timestamp: "12:30".into(), channel: "general".into(), ..Default::default() },
        ChatMessage { sender_name: "Ada".into(), content: "The garden towers are looking great today.".into(), timestamp: "12:32".into(), channel: "general".into(), ..Default::default() },
        ChatMessage { sender_name: "Shaostoul".into(), content: "Shipping the Laws page next.".into(), timestamp: "12:35".into(), channel: "general".into(), ..Default::default() },
    ];
    s.chat_users = vec![
        ChatUser { name: "Shaostoul".into(), public_key: "dlth3:9a41c2abc".into(), role: "admin".into(), status: "online".into() },
        ChatUser { name: "Ada".into(), public_key: "dlth3:5f77e0def".into(), role: "member".into(), status: "online".into() },
    ];
    s.chat_dms = vec![ChatDm { user_name: "Ada".into(), user_key: "ed25519:def".into(), last_message: "See you at the build".into(), timestamp: "11:02".into(), unread: true }];
    // Ada follows me but I don't follow back — covers the one-way
    // follow-direction badge (v0.721) in the members list.
    s.chat_followers.insert("dlth3:5f77e0def".into());
    // One group with an unread dot so the sidebar's group-unread rendering
    // (v0.717) stays covered by the chat snapshot.
    s.chat_groups = vec![crate::gui::ChatGroup {
        name: "Garden Crew".into(),
        id: "grp_garden".into(),
        member_count: 3,
        channels: vec![mk_channel("group:grp_garden", "general", true, false)],
        collapsed: false,
        role: "member".into(),
        unread: true,
    }];
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
        GuiListing {
            id: "demo-1".into(),
            title: "Helix Wide 60 tower".into(),
            description: "33-slot aeroponic tower".into(),
            price: "120 SOL".into(),
            seller_name: "Shaostoul".into(),
            category: "Equipment".into(),
            status: "active".into(),
            ..Default::default()
        },
        GuiListing {
            id: "demo-2".into(),
            title: "Heirloom seed pack".into(),
            description: "Greens + herbs".into(),
            price: "8 SOL".into(),
            seller_name: "Ada".into(),
            category: "Seeds".into(),
            status: "active".into(),
            ..Default::default()
        },
    ];

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

// ── Interaction harness (no GPU) ──
// Proves SYNTHETIC pointer input can drive the app's egui headlessly so a CLICK can be
// ASSERTED in a normal lib test -- closing the operator's "shows != works" gap (egui can
// render yet be non-interactive) without a human launching the app. Interaction is pure
// egui layout + hit-testing; no wgpu device is needed, so these run in the standard
// `cargo test --features native --lib` pass (and on Linux CI) unlike the GPU snapshots.

/// Run `build` once per entry in `frames` against a fresh egui Context, feeding that
/// entry's events on that frame. Returns the Context so a caller can read post-run state
/// (egui memory, `ctx.read_response`). A click needs >=2 frames: one to lay out + place
/// the pointer, the next to press/release against the prior frame's widget rects.
#[cfg(test)]
fn headless_run(
    screen: egui::Vec2,
    frames: &[Vec<egui::Event>],
    mut build: impl FnMut(&egui::Context),
) -> egui::Context {
    let ctx = egui::Context::default();
    for ev in frames {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), screen)),
            events: ev.clone(),
            ..Default::default()
        };
        ctx.run(input, |ctx| build(ctx));
    }
    ctx
}

/// Frame sequence for a primary click at `pos`: frame 1 positions the pointer (so the
/// next frame's hit-test has a settled pointer + the prior layout's rects), frame 2
/// presses and releases. Extra leading empty frames let a page settle (Areas/Windows
/// take a frame to position) before the click.
#[cfg(test)]
fn click_frames(pos: egui::Pos2, settle: usize) -> Vec<Vec<egui::Event>> {
    let m = egui::Modifiers::default();
    let mut frames: Vec<Vec<egui::Event>> = Vec::new();
    for _ in 0..settle {
        frames.push(Vec::new());
    }
    frames.push(vec![egui::Event::PointerMoved(pos)]);
    frames.push(vec![
        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m },
        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m },
    ]);
    frames
}

/// SPIKE: confirm the synthetic-click mechanism works on this egui version before any
/// page-level harness is built on it (de-risks the press/release timing).
#[test]
fn spike_synthetic_click_registers() {
    use std::cell::Cell;
    let clicked = Cell::new(false);
    let target = egui::pos2(60.0, 55.0); // inside the button rect below
    let frames = click_frames(target, 0);
    headless_run(egui::vec2(200.0, 200.0), &frames, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let resp = ui.put(
                egui::Rect::from_min_size(egui::pos2(40.0, 40.0), egui::vec2(80.0, 30.0)),
                egui::Button::new("Hit me"),
            );
            if resp.clicked() {
                clicked.set(true);
            }
        });
    });
    assert!(
        clicked.get(),
        "synthetic primary click did not register -- the press+release sequence needs \
         adjusting for this egui version"
    );
}

/// REAL interaction test on app code: clicking the "Home" container header in the
/// nested-container inventory toggles its open state. This is exactly the "shows !=
/// works" check the operator otherwise has to do by hand in `just launch` -- now a
/// headless lib test. No GPU: pure egui layout + hit-testing.
#[test]
fn inventory_container_header_click_toggles_open() {
    let ctx = egui::Context::default();
    let theme = load_theme();
    theme.apply_to_egui(&ctx);
    let mut state = demo_state();
    let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 1700.0));
    let run = |ctx: &egui::Context, events: Vec<egui::Event>, state: &mut GuiState, theme: &Theme| {
        let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
        ctx.run(input, |ctx| crate::gui::pages::inventory::draw(ctx, theme, state));
    };

    // Settle: lay the page out twice so the places section expands + records the
    // header rects. Clear shared-thread test state first.
    crate::gui::pages::inventory::test_clear_recorded_rects();
    crate::gui::pages::inventory::test_close_garden_edit();
    crate::gui::pages::inventory::test_close_mining_edit();
    crate::gui::pages::inventory::test_clear_placed();
    run(&ctx, Vec::new(), &mut state, &theme);
    run(&ctx, Vec::new(), &mut state, &theme);

    // "Home" is the second top-level place -> path "1" (You=0, Home=1, vehicle=2).
    let rect = crate::gui::pages::inventory::test_recorded_header_rect("1")
        .expect("Home container header rect should be recorded (places section open + on-screen)");
    // Click the LEFT of the header (over the triangle/label), not rect.center(): the
    // full-row click target claims the scroll's available width, which can run wider
    // than the screen, so the center may be off-screen. The left edge is always on it.
    let center = egui::pos2(rect.left() + 30.0, rect.center().y);
    let open_id = egui::Id::new(("place_open", "1"));
    let before = ctx.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(true);

    // Click the header with the canonical egui sequence in SEPARATE frames: move,
    // then press, then release. (A re-`interact()`ed row needs the press and release
    // on different frames; same-frame works for a plain Button but not here.)
    let m = egui::Modifiers::default();
    run(&ctx, vec![egui::Event::PointerMoved(center)], &mut state, &theme);
    run(
        &ctx,
        vec![egui::Event::PointerButton { pos: center, button: egui::PointerButton::Primary, pressed: true, modifiers: m }],
        &mut state,
        &theme,
    );
    run(
        &ctx,
        vec![egui::Event::PointerButton { pos: center, button: egui::PointerButton::Primary, pressed: false, modifiers: m }],
        &mut state,
        &theme,
    );

    let after = ctx.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(true);
    crate::gui::pages::inventory::test_clear_recorded_rects();
    assert!(
        crate::gui::pages::inventory::test_header_was_clicked("1"),
        "the synthetic click was not attributed to the Home header"
    );
    assert_ne!(
        before, after,
        "clicking the Home container header did NOT toggle its open state -- the header \
         renders but is not interactive (the 'shows != works' failure this harness exists \
         to catch)"
    );
}

/// REAL interaction test: the "Link a Device" QR action on the Account settings
/// panel is DISCOVERABLE (renders whenever an identity exists, not buried inside
/// the seed-phrase reveal like v0.837 was) and actually BUILDS the QR when shown.
/// This is the "shows != works" guard for the v0.838 discoverability fix -- the
/// operator reported not seeing the button because it was nested behind the seed
/// reveal. No GPU: pure egui layout + `ctx.load_texture` (CPU-side).
#[test]
fn account_link_device_qr_is_discoverable_and_builds() {
    let ctx = egui::Context::default();
    let theme = load_theme();
    theme.apply_to_egui(&ctx);
    let mut state = demo_state();
    // Identity present but NO encrypted vault, so the QR action uses the plain
    // show/hide button (the passphrase-gated branch is the same proven
    // lockable_gate the seed reveal uses). user_name feeds the QR payload.
    state.private_key_bytes = Some(vec![7u8; 32]);
    state.user_name = "Tester".to_string();
    state.encrypted_private_key = String::new();
    state.key_salt = String::new();
    state.link_device_qr_show = false;
    state.link_device_qr = None;
    let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(960.0, 1800.0));
    let run = |ctx: &egui::Context, state: &mut GuiState, theme: &Theme| {
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        ctx.run(input, |ctx| {
            settings_panel(ctx, theme, state, crate::gui::pages::settings::draw_account_content)
        });
    };

    // Frame 1: the Account panel (incl. the new "Link a Device" section) lays out
    // with an identity present and does NOT panic. QR not requested yet.
    run(&ctx, &mut state, &theme);
    assert!(state.link_device_qr.is_none(), "QR must not build until it is shown");

    // Simulate the user toggling "Show device-link QR" on (the button sets this).
    state.link_device_qr_show = true;
    // Frame 2: the render path must build + cache the QR texture from the seed.
    run(&ctx, &mut state, &theme);
    let cached = state.link_device_qr.as_ref()
        .expect("toggling Show device-link QR must build the QR texture from the seed");
    // It must be keyed by the device-link URL (fragment form) -- NOT raw JSON --
    // so a system-camera scan navigates to the chat page instead of searching the
    // seed (the 2026-07-12 leak). The chat page decodes the fragment + imports.
    let expect = crate::net::identity::device_link_url(&[7u8; 32], "Tester").unwrap();
    assert_eq!(cached.0, expect, "QR must encode the device-link fragment URL");
    assert!(cached.0.starts_with("https://") && cached.0.contains("#devicelink="),
        "device-link QR must be an https URL carrying the payload in the fragment");
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

#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_onboarding_identity() {
    render_page_png("onboarding_identity", 1280, 900, |ctx, theme, state| {
        // First-run identity step WITH the in-place 24-word backup card
        // (v0.673): a fixed seed makes the rendered words deterministic.
        state.onboarding_complete = false;
        state.onboarding_step = 2;
        state.user_name = "Explorer".to_string();
        state.private_key_bytes = Some(vec![7u8; 32]);
        state.settings.seed_phrase_visible = true;
        crate::gui::pages::main_menu::draw(ctx, theme, state);
    });
}

#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_governance() {
    render_page_png("governance", 1400, 1400, |ctx, theme, state| {
        // Inject a representative feed: the page loads live data on a background
        // thread from the connected server, which a snapshot doesn't have, so
        // seed two proposals (one open with a tally, one closed + already voted)
        // and open the new-proposal form. Setting governance_fetched_for to the
        // current server suppresses the auto-fetch.
        if state.governance_proposals.is_empty() {
            use crate::gui::pages::governance::{ProposalView, TallyView};
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            state.server_connected = true;
            state.governance_fetched_for = state.server_url.trim_end_matches('/').to_string();
            state.governance_show_propose = true;
            state.governance_filter_tab = 1; // "All" so the closed one shows too
            state.governance_proposals = vec![
                ProposalView {
                    id: "prop_open".to_string(),
                    proposer_did: "did:hum:ExampleProposer".to_string(),
                    proposal_type: "local_rule".to_string(),
                    scope: "local".to_string(),
                    opens_at: now - 86_400_000,
                    closes_at: now + 5 * 86_400_000,
                    title: "Quiet hours in the shared workshop".to_string(),
                    body: "Between 22:00 and 06:00, powered tools in the shared workshop stay off so the adjacent bunks can sleep.".to_string(),
                    tally: Some(TallyView {
                        yes_weight: 2.35,
                        no_weight: 0.80,
                        abstain_weight: 0.10,
                        total_weight: 3.25,
                        vote_count: 5,
                        quorum_fraction: Some(0.10),
                        electorate: Some(12),
                        quorum_met: Some(true),
                        passing: Some(true),
                    }),
                },
                ProposalView {
                    id: "prop_closed".to_string(),
                    proposer_did: "did:hum:AnotherMember".to_string(),
                    proposal_type: "parameter_change".to_string(),
                    scope: "civilization".to_string(),
                    opens_at: now - 10 * 86_400_000,
                    closes_at: now - 2 * 86_400_000,
                    title: "Raise the default upload cap to 32 MB".to_string(),
                    body: String::new(),
                    tally: Some(TallyView {
                        yes_weight: 1.20,
                        no_weight: 2.90,
                        abstain_weight: 0.00,
                        total_weight: 4.10,
                        vote_count: 6,
                        quorum_fraction: Some(0.05),
                        electorate: Some(12),
                        quorum_met: Some(true),
                        passing: Some(false),
                    }),
                },
            ];
            state.governance_my_votes.insert("prop_closed".to_string(), "no".to_string());
        }
        crate::gui::pages::governance::draw(ctx, theme, state);
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
page_snapshot!(snapshot_cosmos, "cosmos", cosmos, 1280, 900);
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_inventory() {
    render_page_png("inventory", 1280, 1700, |ctx, theme, state| {
        // Reset any modal/selection opened by a prior snapshot (shared thread).
        crate::gui::pages::inventory::test_close_garden_edit();
        crate::gui::pages::inventory::test_close_mining_edit();
        crate::gui::pages::inventory::test_clear_placed();
        crate::gui::pages::inventory::draw(ctx, theme, state);
    });
}

#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_inventory_transfer() {
    render_page_png("inventory_transfer", 1280, 1700, |ctx, theme, state| {
        crate::gui::pages::inventory::test_close_garden_edit();
        crate::gui::pages::inventory::test_close_mining_edit();
        // Select the first placed item so the inspect + "Move to" transfer card shows.
        crate::gui::pages::inventory::test_select_placed(0);
        crate::gui::pages::inventory::draw(ctx, theme, state);
        crate::gui::pages::inventory::test_clear_placed();
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

/// The chat User-Profile modal (v0.845 redesign — themed `widgets::dialog`
/// backdrop, tokenized buttons, avatar badge, admin controls). Viewer = admin
/// looking at a mutual-follow "verified" user, so every section renders: Send
/// DM / Call / Follow, Moderation, Admin (Ban/Mod/Verify/Role).
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_user_profile_modal() {
    render_page_png("user_profile_modal", 560, 860, |ctx, theme, state| {
        use crate::relay::storage::RoleDef;
        let me = "me00000000000000000000000000000000".to_string();
        let target = "aria1111111111111111111111111111ffff".to_string();
        state.profile_public_key = me.clone();
        state.chat_users = vec![
            crate::gui::ChatUser { name: "You".into(),  public_key: me.clone(),     role: "admin".into(),    status: "online".into() },
            crate::gui::ChatUser { name: "Aria".into(), public_key: target.clone(), role: "verified".into(), status: "online".into() },
        ];
        state.chat_roles = vec![
            RoleDef { id: "unverified".into(), label: "Unverified".into(), color: "#9E9E9E".into(), ..Default::default() },
            RoleDef { id: "verified".into(),   label: "Verified".into(),   color: "#4FC3F7".into(), ..Default::default() },
            RoleDef { id: "moderator".into(),  label: "Moderator".into(),  color: "#81C784".into(), ..Default::default() },
            RoleDef { id: "admin".into(),      label: "Admin".into(),      color: "#E57373".into(), ..Default::default() },
        ];
        // Mutual follow ⇒ "Friends".
        state.chat_following_keys.insert(target.clone());
        state.chat_followers.insert(target.clone());
        state.chat_user_modal_open = true;
        state.chat_user_modal_name = "Aria".into();
        state.chat_user_modal_key = target.clone();
        crate::gui::pages::chat::draw_user_modal(ctx, theme, state);
    });
}
/// The Relay Control Center (v0.846) — left rail of the operator's relays +
/// the Health tab showing the rich signed /api/admin/stats snapshot. Seeds two
/// relays (one connected) and a sample admin-stats payload so the full grid
/// (disk, watchdog, backup) renders.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_relay_control_health() {
    render_page_png("relay_control_health", 1100, 720, |ctx, theme, state| {
        let mk = |name: &str, url: &str, connected: bool| crate::gui::ChatServer {
            name: name.to_string(),
            channels: Vec::new(),
            voice_channels: Vec::new(),
            id: format!("srv_{url}"),
            url: url.to_string(),
            connected,
        };
        state.chat_servers = vec![
            mk("united-humanity", "https://united-humanity.us", true),
            mk("localhost", "http://localhost:3210", false),
        ];
        state.server_url = "https://united-humanity.us".into();
        state.relay_cc_selected = Some("https://united-humanity.us".into());
        state.relay_cc_tab = 0;
        state.relay_admin_stats = Some(crate::gui::RelayAdminStats {
            user_count: 128,
            online_count: 7,
            total_messages: 20_431,
            message_count_24h: 342,
            db_size_bytes: 84_500_000,
            upload_size_bytes: 1_620_000_000,
            uptime_seconds: 3 * 86_400 + 4 * 3600 + 12 * 60,
            version: "77fc620-1720000000".into(),
            watchdog_state: "up".into(),
            disk_used_pct: Some(38),
            disk_total_bytes: Some(50_000_000_000),
            disk_avail_bytes: Some(31_000_000_000),
            backup_age_secs: Some(1800),
            backup_count: Some(20),
        });
        crate::gui::pages::relay_control::draw(ctx, theme, state);
    });
}

/// Relay Control Center — Control tab: watchdog chip + honest CLI-fallback
/// cards (restart/logs) + the Services hand-off.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_relay_control_actions() {
    render_page_png("relay_control_actions", 1100, 720, |ctx, theme, state| {
        state.chat_servers = vec![crate::gui::ChatServer {
            name: "united-humanity".into(),
            channels: Vec::new(),
            voice_channels: Vec::new(),
            id: "srv_uh".into(),
            url: "https://united-humanity.us".into(),
            connected: true,
        }];
        state.server_url = "https://united-humanity.us".into();
        state.relay_cc_selected = Some("https://united-humanity.us".into());
        state.relay_cc_tab = 1;
        state.relay_admin_stats = Some(crate::gui::RelayAdminStats {
            watchdog_state: "up".into(),
            backup_age_secs: Some(1800),
            ..Default::default()
        });
        crate::gui::pages::relay_control::draw(ctx, theme, state);
    });
}

/// Native donate page — the Sponsor-A-Can (501c3) primary card above the crypto
/// section (mirrors web v0.845.1).
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_donate() {
    render_page_png("donate", 1000, 1200, |ctx, theme, state| {
        state.donate_solana_address = "So1anaExampleAddress1111111111111111111111".into();
        state.donate_btc_address = "bc1qexamplebitcoinaddress00000000000000".into();
        state.donate_methods = vec![
            crate::gui::DonateMethod { network: "Patreon".into(), label: "Monthly membership support.".into(), value: "https://www.patreon.com/c/Shaostoul".into(), kind: "url".into(), abbrev: "PAT".into(), color: "#f96854".into() },
            crate::gui::DonateMethod { network: "PayPal".into(), label: "One-time or recurring via PayPal.".into(), value: "https://paypal.me/Shaostoul".into(), kind: "url".into(), abbrev: "PP".into(), color: "#0070ba".into() },
        ];
        state.donate_charities = vec![
            crate::gui::DonateCharity {
                name: "Sponsor-A-Can".into(),
                mission: "A registered 501(c)(3) fighting poverty through sanitation, recycling, and community clean-up.".into(),
                url: "https://www.sponsor-a-can.org/donate/".into(),
                note: "Independent 501(c)(3). The maintainer volunteers as its VP. Deductibility depends on your situation.".into(),
                abbrev: "SAC".into(),
                color: "#3fae49".into(),
            },
        ];
        crate::gui::pages::donate::draw(ctx, theme, state);
    });
}

/// Platform fold with Notes + Calendar rescued into the section nav (v0.847.x).
/// Renders the Notes section so both the new sidebar entries and the page draw.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_platform_notes() {
    render_page_png("platform_notes", 1280, 800, |ctx, theme, state| {
        state.active_platform_section = "notes".into();
        crate::gui::pages::platform::draw(ctx, theme, state);
    });
}

page_snapshot!(snapshot_homes, "homes", homes, 1280, 1400);

// The construction editor needs a selected room with machines, so it gets a custom setup
// (the macro only passes the default state). Garage holds machines + connections in the seed
// home.ron, so this exercises the Machines (place/offset) + Connections panels end to end.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_construction() {
    render_page_png("construction", 1280, 1200, |ctx, theme, state| {
        use crate::ship::fibonacci::WallKind;
        if state.home_machines.is_none() {
            state.home_machines = crate::machines::MachineHome::load(
                &std::path::Path::new("data").join("machines").join("home.ron"),
            );
        }
        state.construction_active = true;
        if state.construction_rooms.is_empty() {
            // Study holds a few machines in the seed (smelter/forge/fuel), so the panel stays
            // readable while still exercising Machines + the Connections add-row + the whole-home
            // Buildability report (which is the same regardless of which room is selected).
            state.construction_rooms = vec![crate::gui::ConstructionRoom {
                id: "study".to_string(),
                walls: [WallKind::Auto; 4],
                wall_offsets: [0.0; 4],
                openings: Vec::new(),
                level: 0,
                position: Some([0.0, 0.0, 0.0]),
                dimensions: [21.0, 5.0, 21.0],
                material_type: 1,
                color: [0.5, 0.5, 0.55, 1.0],
            }];
        }
        state.construction_selected_room = Some(0);
        // Hold a palette item so the snapshot shows the active-item highlight (v0.529).
        state.construction_place_type = Some("aeroponic_tower_nutrition".to_string());
        crate::gui::pages::construction::draw(ctx, theme, state);
    });
}

page_snapshot!(snapshot_tasks, "tasks", tasks, 1280, 900);
page_snapshot!(snapshot_market, "market", market, 1280, 900);
page_snapshot!(snapshot_profile, "profile", profile, 1280, 900);
page_snapshot!(snapshot_crafting, "crafting", crafting, 1280, 900);
page_snapshot!(snapshot_library, "library", library, 1280, 900);
// snapshot_governance is a hand-written test above (needs injected proposal
// state to show the real feed; the bare macro version rendered an empty page).
page_snapshot!(snapshot_identity, "identity", identity, 1280, 900);
page_snapshot!(snapshot_wallet, "wallet", wallet, 1280, 900);
page_snapshot!(snapshot_quests, "quests", quests, 1280, 900);
page_snapshot!(snapshot_calendar, "calendar", calendar, 1280, 900);
page_snapshot!(snapshot_notes, "notes", notes, 1280, 900);

// Studio needs the scene/source presets loaded (demo_state leaves them empty) and a
// staged-vs-live divergence so the Program/Preview split is actually visible: program
// holds the cut "Main" layout while "Screen Share" sits staged in preview.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_studio() {
    render_page_png("studio", 1280, 900, |ctx, theme, state| {
        if state.studio.scenes.is_empty() {
            let data = std::path::Path::new("data");
            state.studio.sources = crate::gui::load_studio_sources(data)
                .iter()
                .map(crate::gui::studio_source_from_preset)
                .collect();
            state.studio.scenes = crate::gui::load_studio_scenes(data)
                .iter()
                .map(crate::gui::studio_scene_from_preset)
                .collect();
            state.studio_streaming_config = crate::gui::load_studio_streaming_config(data);
            // Cut "Main" live, then stage a different scene into preview.
            state.studio.cut_to_program();
            if let Some(idx) = state.studio.scenes.iter().position(|s| s.name == "Screen Share") {
                state.studio.select_preview_scene(idx);
            }
            state.studio.is_live = true;
        }
        crate::gui::pages::studio::draw(ctx, theme, state);
    });
}
