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

/// An egui context with the SAME font chains the running app installs.
///
/// This exists because for a long time it did not. Every context in this file
/// was a bare `egui::Context::default()`, which installs no fonts at all,
/// while the app calls `fonts::install_font_fallbacks` at `lib.rs`. So the
/// snapshot rig rendered a font stack no user has ever seen, and the
/// `BROKEN_GLYPHS` list in `tests/icon_glyph_lint.rs` was partly derived from
/// those snapshots. A rig whose output cannot show what the product shows is
/// not evidence.
///
/// One honest consequence: the CJK and emoji fallbacks read fonts from the
/// machine running the test, so snapshots are now machine-dependent for those
/// codepoints. That is the correct trade for a review rig, because the
/// alternative is a picture of something nobody runs. The Hack fallback, which
/// is the one that fixes arrows and box drawing, is machine-independent.
fn snapshot_ctx() -> egui::Context {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    ctx
}

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
    let lib = crate::gui::load_library(data);
    s.library = lib.sections;
    s.library_tags = lib.tag_groups;
    s.curriculum = crate::gui::load_curriculum(data);
    s.garden_areas = crate::gui::load_garden_areas(data);
    s.grow_media = crate::gui::load_grow_media(data);
    s.onboarding_quest_chains = crate::gui::pages::onboarding::load_quest_chains(data);
    // The app loads the donate FAQ at startup (lib.rs); without it the Donate
    // snapshot ended on a "Frequently Asked Questions" heading over nothing.
    s.donate_faq = crate::gui::load_donate_faq(data);
    // The rest of the Donate page's data, loaded at startup the same way.
    s.donate_routes = crate::gui::load_donate_routes(data);
    s.donate_methods = crate::gui::load_donate_methods(data);
    s.donate_charities = crate::gui::load_donate_charities(data);
    // The demo state renders the way every rig runs the game: the Dev mode, with its free
    // resources. Set explicitly since fresh installs start in Normal (2026-10-04), so the
    // snapshots do not change with that default (scripts/lib/rig-gameplay.js pins the same).
    s.settings.play_mode = crate::config::PlayMode::Dev;
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
        breathing: true,
        air_c: 20.0,
        feels_c: 20.0,
        sheltered: false,
        shelter_note: String::new(),
        effects: vec![("Well-fed".into(), 180.0), ("Rested".into(), 90.0)],
    };
    let mut items = vec![
        Some(GuiItemSlot { item_id: "water_bottle_0".into(), name: "Water Bottle".into(), quantity: 2, ..Default::default() }),
        Some(GuiItemSlot { item_id: "bread_0".into(), name: "Bread".into(), quantity: 5, ..Default::default() }),
        Some(GuiItemSlot { item_id: "iron_ore_0".into(), name: "Iron Ore".into(), quantity: 6, ..Default::default() }),
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
            ..Default::default()
        }));
    }
    s.inventory_items = items;
    // The Weight and Volume tiles read the inventory system's numbers
    // (`GuiState::carry`, BUG-136), which the game publishes each frame; the
    // fixture publishes what the items above add up to in data/items.csv's
    // weight_kg and volume_l columns (30.7 kg, 11.85 L; the bell pepper
    // seeds have no row there, so they count as nothing) against the 50 kg,
    // 65 L pack at 1 g. Without it the snapshot read "0.0 / 50.0 kg" beside
    // a full list.
    s.carry = crate::systems::encumbrance::evaluate(
        crate::systems::encumbrance::CarryInput {
            carried_kg: 30.7,
            capacity_kg: 50.0,
            bonus_kg: 0.0,
            volume_l: 11.85,
            volume_capacity_l: 65.0,
        },
        crate::systems::encumbrance::ONE_G_M_S2,
        crate::systems::encumbrance::CarryMode::Forgiving,
    );

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
        GuiSkill { id: "farming".into(), name: "Farming".into(), category: "Survival".into(), level: 4, xp: 120, xp_needed: 200, max_level: 20 },
        GuiSkill { id: "mining".into(), name: "Mining".into(), category: "Survival".into(), level: 2, xp: 40, xp_needed: 120, max_level: 20 },
        GuiSkill { id: "crafting".into(), name: "Crafting".into(), category: "Production".into(), level: 3, xp: 75, xp_needed: 150, max_level: 20 },
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
        local_only: false,
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
        // Formatting-parity coverage (v0.1208): a blockquote + __bold__, a
        // bullet list, and a fenced code block, so the snapshot proves the
        // native renderer draws them (muted quote, real bullets, code panel).
        ChatMessage { sender_name: "Ada".into(), content: "> from the design notes\nWe should try __two towers__ per plot:\n- more light\n- easier harvest".into(), timestamp: "12:36".into(), channel: "general".into(), ..Default::default() },
        ChatMessage { sender_name: "Shaostoul".into(), content: "Here's the tick loop:\n```rust\nfor sys in systems {\n    sys.tick(dt);\n}\n```\nLooks good to me.".into(), timestamp: "12:37".into(), channel: "general".into(), ..Default::default() },
    ];
    s.chat_users = vec![
        ChatUser { name: "Shaostoul".into(), public_key: "dlth3:9a41c2abc".into(), role: "admin".into(), status: "online".into() },
        ChatUser { name: "Ada".into(), public_key: "dlth3:5f77e0def".into(), role: "member".into(), status: "online".into() },
    ];
    s.chat_dms = vec![ChatDm { user_name: "Ada".into(), user_key: "ed25519:def".into(), last_message: "See you at the build".into(), timestamp: "11:02".into(), unread: true }];
    // Ada follows me but I don't follow back — covers the one-way
    // follow-direction badge (v0.721) in the members list.
    s.chat_followers.insert("dlth3:5f77e0def".into());
    // (Legacy ChatGroup snapshot fixture removed 2026-08-23 with the
    // plaintext group system; P2P groups have their own fixtures.)
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
            category: "Tools".into(),
            status: "active".into(),
            ..Default::default()
        },
        GuiListing {
            id: "demo-2".into(),
            title: "Heirloom seed pack".into(),
            description: "Greens + herbs".into(),
            price: "8 SOL".into(),
            seller_name: "Ada".into(),
            category: "Growing".into(),
            status: "active".into(),
            ..Default::default()
        },
    ];
    // Market directory (the signed catalog tab renders these without a relay).
    crate::gui::pages::market_directory::seed_for_snapshot();

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
fn render_page_png(name: &str, w: u32, h: u32, frame: impl Fn(&egui::Context, &mut Theme, &mut GuiState)) {
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
        let ctx = snapshot_ctx();
        let mut theme = load_theme();
        theme.apply_to_egui(&ctx);
        // A snapshot is a still of the SETTLED page. egui fades every Window and
        // Area in over `animation_time` (1/12 s), and this harness captures its
        // second frame at about t = 1/60 s, so every Window-based page (the main
        // menu, onboarding, the inventory modals, the help overlay, the profile
        // modal) was photographed at roughly half opacity with the page behind
        // it showing through. Zero animation time is what a person sees a moment
        // after opening the thing. `apply_to_egui` clones the current style, so
        // pages that re-apply the theme inside the frame keep this setting.
        ctx.all_styles_mut(|s| s.animation_time = 0.0);
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
        //
        // TWO warm-up frames, not one (2026-09-27). A Window's first frame is
        // egui's invisible SIZING pass, which lays justified and centred
        // content out at its minimum size, and the second frame clips the
        // window's content to the size that pass measured. So a window whose
        // content is taller once properly laid out (a TextEdit given a
        // height, a centred button) was photographed with its bottom cut off:
        // the onboarding window lost the lower half of its Back button, a
        // state that lasts one frame in the app. The third frame is settled.
        //
        // The extra frame is pinned at t = 0, so the two frames after it land
        // at 1/60 s and 2/60 s exactly as the old two frames did (egui adds
        // its predicted 1/60 s per frame when no time is given). Anything
        // painted from `input.time` (the animated slider-thumb ring, the RGB
        // selection edges, uptime clocks) looks exactly as it did.
        for warm_t in [Some(0.0_f64), None] {
            let mut warm_input = raw_input.clone();
            warm_input.time = warm_t;
            let warm = ctx.run(warm_input, |ctx| {
                frame(ctx, &mut theme, &mut state);
            });
            for (id, delta) in &warm.textures_delta.set {
                renderer.update_texture(&device, &queue, *id, delta);
            }
        }
        let full_output = ctx.run(raw_input, |ctx| {
            frame(ctx, &mut theme, &mut state);
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
    let ctx = snapshot_ctx();
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
    let ctx = snapshot_ctx();
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

/// REAL interaction test on the readable web view (2026-09-16): a page with
/// links is drawn headlessly, one link is clicked with the canonical synthetic
/// move / press / release sequence, and the view must have QUEUED navigation
/// to the RESOLVED absolute URL, with no fetch dispatched (fetch_enabled is
/// off, so nothing touches the network). This is the "shows != works" guard
/// for the view's links: a label that renders but does not navigate is the
/// failure this exists to catch. Proven red on 2026-09-16 (at review, and
/// again in the review-fix pass) by disabling the `*self.clicked = Some(href)`
/// line in web_view.rs: the first assertion below then fails with
/// `left: None, right: Some("https://example.com/docs/nearby.html")`.
#[test]
fn web_view_link_click_queues_navigation_to_the_resolved_url() {
    use crate::gui::widgets::image_cache::ImageCache;
    use crate::gui::widgets::web_view::{ViewStatus, WebViewState};

    let ctx = snapshot_ctx();
    let theme = load_theme();
    theme.apply_to_egui(&ctx);
    let mut view = WebViewState::new();
    view.fetch_enabled = false;
    let mut images = ImageCache::new();
    // The page exactly as the parser produces it from the fixture, so the
    // href the click must yield is the RESOLVED one, not the fixture's
    // relative "nearby.html".
    let page = crate::web_reader::parse_html(
        include_str!("../../tests/fixtures/web/article.html"),
        "https://example.com/docs/page.html",
        &Default::default(),
    );
    view.page = Some(page);
    view.status = ViewStatus::Ready;

    let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 900.0));
    let run = |ctx: &egui::Context, events: Vec<egui::Event>, view: &mut WebViewState, images: &mut ImageCache| {
        let input = egui::RawInput { screen_rect: Some(screen), events, ..Default::default() };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                view.show(ui, &theme, images, None);
            });
        });
    };

    // Settle twice so the scroll area and the wrapped labels have rects.
    run(&ctx, Vec::new(), &mut view, &mut images);
    run(&ctx, Vec::new(), &mut view, &mut images);
    let target = "https://example.com/docs/nearby.html";
    let rect = view
        .link_rects()
        .iter()
        .find(|(href, _)| href == target)
        .map(|(_, r)| *r)
        .expect("the resolved link should have been drawn and its rect recorded");
    assert!(screen.contains(rect.center()), "link must be on screen to be clickable: {rect:?}");
    assert!(view.queued_navigation().is_none(), "nothing queued before the click");

    let center = rect.center();
    let m = egui::Modifiers::default();
    run(&ctx, vec![egui::Event::PointerMoved(center)], &mut view, &mut images);
    run(
        &ctx,
        vec![egui::Event::PointerButton { pos: center, button: egui::PointerButton::Primary, pressed: true, modifiers: m }],
        &mut view,
        &mut images,
    );
    run(
        &ctx,
        vec![egui::Event::PointerButton { pos: center, button: egui::PointerButton::Primary, pressed: false, modifiers: m }],
        &mut view,
        &mut images,
    );

    assert_eq!(
        view.queued_navigation(),
        Some(target),
        "clicking the link did NOT queue navigation to the resolved URL -- the link \
         renders but is not interactive, or resolved to the wrong address"
    );
    assert_eq!(view.current_url(), Some(target), "the click is on the history stack");
    assert!(matches!(view.status, ViewStatus::Fetching(_)), "status says a fetch is pending: {:?}", view.status);
}

/// REAL rendering test on the readable web view's PRIVACY promise for images
/// (2026-09-16): the Settings hint says only the address of the page you open
/// leaves the machine, "plus the images that scroll into view". That holds
/// only if an image far below the fold is NOT requested while it is off
/// screen. The page here is eighty paragraphs and then one image whose
/// address is a closed loopback port (nothing listens on 127.0.0.1:9, so
/// even if the gate is broken the request goes nowhere). Drawn in a short
/// viewport the image sits well below the clip rect and the cache must still
/// be Idle for it; drawn in a viewport tall enough to show the whole page,
/// the same image is on screen and the cache must be Fetching. Proven red
/// first: with the `is_rect_visible` gate in `web_view.rs` removed, the first
/// assertion fails with "the image below the fold was fetched".
#[test]
fn web_view_requests_an_image_only_when_it_scrolls_into_view() {
    use crate::gui::widgets::image_cache::{ImageCache, ImageStatus};
    use crate::gui::widgets::web_view::{ViewStatus, WebViewState};
    use crate::web_reader::{Block, Inline, Page};

    let ctx = snapshot_ctx();
    let theme = load_theme();
    theme.apply_to_egui(&ctx);
    let mut view = WebViewState::new();
    view.fetch_enabled = false;
    let mut images = ImageCache::new();

    // Nothing listens on the discard port, so a request (which would be the
    // failure this test catches) is refused by the OS at once, no network.
    let src = "http://127.0.0.1:9/below-the-fold.png";
    let mut blocks: Vec<Block> = (0..80)
        .map(|i| Block::Paragraph(vec![Inline::Text(format!("Paragraph {i} of the long article above the picture."))]))
        .collect();
    blocks.push(Block::Image { src: src.to_string(), alt: "the picture at the bottom".to_string() });
    view.page = Some(Page {
        url: "https://example.com/long.html".to_string(),
        title: "A long page".to_string(),
        blocks,
        notice: None,
    });
    view.status = ViewStatus::Ready;

    let run = |ctx: &egui::Context, screen: egui::Rect, view: &mut WebViewState, images: &mut ImageCache| {
        let input = egui::RawInput { screen_rect: Some(screen), events: Vec::new(), ..Default::default() };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                view.show(ui, &theme, images, None);
            });
        });
    };

    // A short viewport: eighty paragraphs of text are far taller than 400 px,
    // so the image's place on the page is outside the scroll area's clip.
    let short = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 400.0));
    run(&ctx, short, &mut view, &mut images);
    run(&ctx, short, &mut view, &mut images);
    assert!(
        matches!(images.status(src), ImageStatus::Idle),
        "the image below the fold was fetched while it was off screen: {:?}",
        images.status(src)
    );

    // A viewport tall enough for the whole page: now the image is visible and
    // its one GET must have been dispatched (the cache marks it Fetching the
    // moment `request` runs, before the thread has even connected).
    let tall = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, 20_000.0));
    run(&ctx, tall, &mut view, &mut images);
    run(&ctx, tall, &mut view, &mut images);
    assert!(
        matches!(images.status(src), ImageStatus::Fetching | ImageStatus::Failed(_)),
        "the image on screen was never requested: {:?}",
        images.status(src)
    );
}

/// REAL interaction test: the "Link a Device" QR action on the Account settings
/// panel is DISCOVERABLE (renders whenever an identity exists, not buried inside
/// the recovery-phrase reveal like v0.837 was) and actually BUILDS the QR when shown.
/// This is the "shows != works" guard for the v0.838 discoverability fix -- the
/// operator reported not seeing the button because it was nested behind the seed
/// reveal. No GPU: pure egui layout + `ctx.load_texture` (CPU-side).
#[test]
fn account_link_device_qr_is_discoverable_and_builds() {
    let ctx = snapshot_ctx();
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

// The in-world HUD with the survival rows and the active quest (2026-09-25):
// thirsty, tiring, exposed and cold, under the default "When low" mode, so
// every kind of row is on screen at once. Drawn over the theme background,
// not a world, which is enough to judge the layout and colours.
#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_hud_vitals() {
    render_page_png("hud_vitals", 900, 360, |ctx, theme, state| {
        state.player_health = 72.0;
        state.player_health_max = 100.0;
        state.wallet_credits = 140;
        state.vitals = crate::gui::GuiVitals {
            satiation: 68.0,
            hydration: 22.0,
            energy: 41.0,
            oxygen: 63.0,
            body_temp_c: 35.4,
            waste: 30.0,
            satiation_max: 100.0,
            hydration_max: 100.0,
            energy_max: 100.0,
            oxygen_max: 100.0,
            waste_max: 100.0,
            sealed: false,
            breathing: false,
            air_c: 5.0,
            feels_c: 5.0,
            sheltered: false,
            shelter_note: "Out of the rain; 100% of the wind gets in".into(),
            effects: Vec::new(),
        };
        state.quests = vec![crate::gui::GuiQuest {
            name: "First Steps".to_string(),
            step_index: 1,
            step_total: 4,
            step_desc: "Plant a seed in the garden".to_string(),
            completed: false,
        }];
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme.bg_primary()))
            .show(ctx, |_ui| {});
        crate::gui::pages::hud::draw(ctx, theme, state, 0.0, glam::Mat4::IDENTITY, glam::Vec3::ZERO);
    });
}

// The death screen in the Death setting's Realistic mode (2026-10-04, engine/death_pack.rs):
// what stayed behind in the pack, where, the key and the time, over the dimmed world. The
// Simplified card is the one-line card it always was.
#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_death_screen_realistic() {
    render_page_png("death_screen_realistic", 1280, 720, |ctx, theme, state| {
        use crate::systems::death_pack::{DeathNote, Landing};
        state.player_death_cause = Some("hypothermia".into());
        state.death_pack.note = Some(DeathNote::Left {
            items: 14,
            place_words: "on Earth".into(),
            landing: Landing::FromDeepWater { dist_m: 230.0 },
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme.bg_primary()))
            .show(ctx, |_ui| {});
        crate::gui::pages::hud::draw_death_screen(ctx, theme, state);
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
    fn snapshot_settings_full() {
    // The WHOLE settings page (left nav + all sections), so the per-section
    // accent tint bands (v0.1114) are visible; the per-section snapshots above
    // render one draw_*_content bare, without the band the page loop adds.
    render_page_png("settings_full", 1400, 1600, |ctx, theme, state| {
        crate::gui::pages::settings::draw(ctx, theme, state);
    });
}

// Hint-display modes (v0.1116): the Audio section rendered under each
// `HintDisplay` so the three layouts can be compared side by side. Audio is a
// good witness because its top card is five sliders each with a one-line
// description, exactly the "control then its help text" rhythm the mode
// reshapes. Full = descriptions inline + tightened; Off = descriptions gone,
// controls keep their air; Hover = each description collapses to a "(?)".
#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_audio_settings_hints_off() {
    render_page_png("audio_settings_hints_off", 960, 1100, |ctx, theme, state| {
        state.settings.hint_display = crate::gui::HintDisplay::Off;
        settings_panel(ctx, theme, state, crate::gui::pages::settings::draw_audio_content);
    });
}

#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_audio_settings_hints_hover() {
    render_page_png("audio_settings_hints_hover", 960, 1100, |ctx, theme, state| {
        state.settings.hint_display = crate::gui::HintDisplay::Hover;
        settings_panel(ctx, theme, state, crate::gui::pages::settings::draw_audio_content);
    });
}

// Settings > Safety (step B of docs/design/blocking-and-safe-mode.md 10c, 2026-10-09): "Who
// can reach me" as the server last said it (messages and calls from people I choose, trades from
// friends), "People I choose" (10c-ii, 2026-10-10) with four friends, each once with Message,
// Call and Trade ticks: one on the defaults, one with all three ticked, one whose new choice
// (Message off, Call on) is still being re-issued, one with nothing ticked; the lines above the
// list read "In use now: Messages and Calls. Trades are set to Friends, ...". Then two requests:
// one the member list names, one from someone not on it right now. The store is built in memory
// and never saved, so nothing lands on disk. Taller since step F (10g, 2026-10-10) so the
// Warnings section and its switch are in the picture, and again for the two lines and the fourth
// friend of 10c-ii.
#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_safety_settings() {
    render_page_png("safety_settings", 960, 1480, |ctx, theme, state| {
        if state.dm_store.is_none() {
            use crate::net::dm_store::SentPass;
            use crate::net::reach::{intended_may_wire, Audience, ContactRequest, FriendTicks, ReachSettings};
            state.profile_public_key = "me".to_string();
            let mut store = crate::net::dm_store::DmStore::load(&[7u8; 32], "me", "wss://snapshot.safety.invalid");
            store.set_reach_settings(ReachSettings { message: Audience::Chosen, ..ReachSettings::default() });
            let t = |message, call, trade| FriendTicks { message, call, trade };
            // (key, name, the ticks chosen, the ticks the pass they hold was given under)
            for (key, name, chosen, given) in [
                ("ann", "Ann", FriendTicks::default(), FriendTicks::default()),
                ("ben", "Ben", t(true, true, true), t(true, true, true)),
                ("cy", "Cy", t(false, true, true), FriendTicks::default()),
                ("dee", "Dee", t(false, false, false), t(false, false, false)),
            ] {
                store.set_following(key, true);
                store.set_follower(key, true);
                store.set_ticks(key, chosen);
                store.record_pass_sent(key, SentPass { serial: format!("{key:0>32}"), may: intended_may_wire(given) });
                state.chat_users.push(crate::gui::ChatUser { name: name.into(), public_key: key.into(), role: String::new(), status: "online".into() });
            }
            state.chat_users.push(crate::gui::ChatUser { name: "Dana Okafor".into(), public_key: "dana".into(), role: String::new(), status: "online".into() });
            store.add_request(ContactRequest { key: "dana".into(), ts: 1, pass: String::new() });
            store.add_request(ContactRequest { key: "e1f2a3b4c5d6".into(), ts: 2, pass: String::new() });
            state.dm_store = Some(store);
        }
        settings_panel(ctx, theme, state, |ui, theme, state| {
            crate::gui::pages::safety::draw_safety_content(ui, theme, state, theme.info())
        });
    });
}

// Settings > Safety > Blocked people (step C of docs/design/blocking-and-safe-mode.md 10d,
// 2026-10-09): two people blocked, one the member list names and one not on it right now, each
// with the date blocked and Unblock, then the line saying what blocking cannot do. The list is
// built in memory and never saved, so nothing lands on disk.
#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_safety_blocked() {
    render_page_png("safety_blocked", 960, 520, |ctx, theme, state| {
        if state.block_list.is_none() {
            state.profile_public_key = "me".to_string();
            let mut list = crate::net::block_list::BlockList::in_temp(&[7u8; 32], "me", "snapshot");
            list.block("dana", 1_791_504_000_000); // 2026-10-09
            list.block("e1f2a3b4c5d6", 1_790_726_400_000); // 2026-09-30
            state.block_list = Some(list);
            state.chat_users.push(crate::gui::ChatUser { name: "Dana Okafor".into(), public_key: "dana".into(), role: String::new(), status: "online".into() });
        }
        settings_panel(ctx, theme, state, |ui, theme, state| {
            crate::gui::pages::safety::draw_blocked_people(ui, theme, state, theme.info())
        });
    });
}

/// The protected setup's snapshots (step G of docs/design/blocking-and-safe-mode.md 10h,
/// 2026-10-10): the preset from the file built into the exe, and a DM store built in memory and
/// never saved, with two friends (Ann, Ben), a request from Dana, and the safe defaults as the
/// server's word. `on` turns the setup on behind a quick PIN verifier (the PIN is never drawn).
fn protected_fixture(state: &mut GuiState, on: bool) {
    use crate::net::dm_store::SentPass;
    use crate::net::reach::{ContactRequest, ReachSettings};
    state.profile_public_key = "me".to_string();
    let preset = crate::net::protected::parse_presets(crate::embedded_data::SAFETY_PRESETS_JSON.as_bytes()).expect("the built-in presets");
    if on {
        let quick = crate::net::protected::PinVerifier::with_salt("1234", &[1u8; 16], 1_000);
        state.protected.setup = crate::net::protected::ProtectedSetup::applied(&preset, quick, "me", vec!["ann".into(), "ben".into()]);
    }
    state.protected.preset = Some(preset);
    let mut store = crate::net::dm_store::DmStore::load(&[7u8; 32], "me", "wss://snapshot.protected.invalid");
    store.set_reach_settings(ReachSettings::default());
    for (key, name) in [("ann", "Ann"), ("ben", "Ben")] {
        store.set_following(key, true);
        store.set_follower(key, true);
        store.record_pass_sent(key, SentPass { serial: format!("{key:0>32}"), may: "invite,message,trade,voice_message".into() });
        state.chat_users.push(crate::gui::ChatUser { name: name.into(), public_key: key.into(), role: String::new(), status: "online".into() });
    }
    state.chat_users.push(crate::gui::ChatUser { name: "Dana Okafor".into(), public_key: "dana".into(), role: String::new(), status: "online".into() });
    store.add_request(ContactRequest { key: "dana".into(), ts: 1, pass: String::new() });
    state.dm_store = Some(store);
}

/// Settings > Safety > Protected setup, step 1 (10h): the preset's seven sentences in order,
/// then Continue and Cancel.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_protected_read() {
    render_page_png("protected_read", 960, 760, |ctx, theme, state| {
        if state.protected.preset.is_none() {
            protected_fixture(state, false);
            state.protected.step = Some(crate::net::protected::SetupStep::Read);
        }
        settings_panel(ctx, theme, state, |ui, theme, state| {
            crate::gui::pages::safety_protected::draw_protected_section(ui, theme, state, theme.info())
        });
    });
}

/// Step 2: choose a PIN of 4 to 12 digits, twice (the fields masked), after two PINs that did not
/// agree, with the preset's rule repeated under them.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_protected_pin() {
    render_page_png("protected_pin", 960, 420, |ctx, theme, state| {
        if state.protected.preset.is_none() {
            protected_fixture(state, false);
            state.protected.step = Some(crate::net::protected::SetupStep::ChoosePin);
            state.protected.pin_first = "4821".into();
            state.protected.line = state.protected.preset.as_ref().unwrap().pin_rule();
        }
        settings_panel(ctx, theme, state, |ui, theme, state| {
            crate::gui::pages::safety_protected::draw_protected_section(ui, theme, state, theme.info())
        });
    });
}

/// Step 3: who can already reach this device, each with Remove: two friends, a group, a voice
/// room joined; then "Keep the rest".
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_protected_review() {
    render_page_png("protected_review", 960, 640, |ctx, theme, state| {
        if state.protected.preset.is_none() {
            protected_fixture(state, false);
            state.protected.step = Some(crate::net::protected::SetupStep::Review);
            state.p2p_groups.push(crate::net::api_v2::P2pGroupInfo { group_id: "g1".into(), name: "Book club".into(), members: vec!["me".into()], is_creator: false });
            state.chat_channels.push(ChatChannel { id: "lounge".into(), name: "lounge".into(), voice_enabled: true, voice_joined: true, ..Default::default() });
        }
        settings_panel(ctx, theme, state, |ui, theme, state| {
            crate::gui::pages::safety_protected::draw_protected_section(ui, theme, state, theme.info())
        });
    });
}

/// Settings > Safety with the setup on: the always-visible line first, the rows as the server
/// last said them, the Requests list's "Accept (needs the PIN)", and at the bottom the section
/// with the routes line, the public rooms and pictures switches, and Change the PIN, Turn off and
/// "Forgot the PIN?".
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_safety_protected_on() {
    render_page_png("safety_protected_on", 960, 2000, |ctx, theme, state| {
        if state.protected.preset.is_none() {
            protected_fixture(state, true);
        }
        settings_panel(ctx, theme, state, |ui, theme, state| {
            crate::gui::pages::safety::draw_safety_content(ui, theme, state, theme.info())
        });
    });
}

/// The PIN prompt in its three modes, over an empty page: `mode` with Follow waiting.
fn protected_prompt_snapshot(name: &str, mode: crate::net::protected::PromptMode, line: fn(&crate::net::protected::Preset) -> String) {
    render_page_png(name, 720, 420, |ctx, theme, state| {
        if state.protected.preset.is_none() {
            protected_fixture(state, true);
            state.protected.prompt = Some(crate::net::protected::PinPrompt { action: Some(crate::net::protected::ProtectedAction::Follow("ben".into())), mode });
            state.protected.prompt_line = line(state.protected.preset.as_ref().unwrap());
        }
        theme.apply_to_egui(ctx);
        egui::CentralPanel::default().show(ctx, |_| {});
        crate::gui::pages::safety_protected::draw_pin_prompt(ctx, theme, state);
    });
}

/// The PIN prompt a locked action opens (here Follow), after three wrong PINs: the preset's wait
/// line, then Continue, Cancel and "Forgot the PIN?".
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_protected_pin_prompt() {
    protected_prompt_snapshot("protected_pin_prompt", crate::net::protected::PromptMode::Pin, |p| p.labels.pin_wait(60));
}

/// "Forgot the PIN?" in the prompt: who can use it (the preset's line), the masked field for the
/// recovery phrase, after a phrase that did not match.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_protected_forgot() {
    protected_prompt_snapshot("protected_forgot", crate::net::protected::PromptMode::Forgot, |p| p.labels.phrase_wrong.clone());
}

/// The new PIN, twice, after the recovery phrase matched (the prompt then asks for it before
/// Follow runs).
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_protected_new_pin() {
    protected_prompt_snapshot("protected_new_pin", crate::net::protected::PromptMode::NewPin, |_| String::new());
}

/// The chat with the setup on: the always-visible line above the direct messages, "Public rooms
/// are hidden by the protected setup." above the servers' lists, #announcements (read-only) open,
/// and a post there from someone who is not a friend whose picture is left out with the preset's
/// line.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_chat_protected() {
    render_page_png("chat_protected", 1280, 900, |ctx, theme, state| {
        if state.protected.preset.is_none() {
            protected_fixture(state, true);
            // The default state already lists these two; replace them rather than list each twice.
            state.chat_channels.retain(|c| c.id != "announcements" && c.id != "general");
            state.chat_channels.push(ChatChannel { id: "announcements".into(), name: "announcements".into(), read_only: true, ..Default::default() });
            state.chat_channels.push(ChatChannel { id: "general".into(), name: "general".into(), ..Default::default() });
            state.chat_messages.push(ChatMessage {
                sender_name: "Server admin".into(),
                sender_key: "admin0".into(),
                content: "New rules are up. https://example.com/rules.png".into(),
                timestamp: "09:15".into(),
                timestamp_ms: 1_791_480_900_000,
                channel: "announcements".into(),
                ..Default::default()
            });
            state.chat_active_channel = "announcements".into();
        }
        crate::gui::pages::chat::draw(ctx, theme, state);
    });
}

/// Reasons for the report snapshots, built here (the real list is data/safety/report_reasons.json,
/// written by the relay half of step D): enough to show the list and one help text.
fn snapshot_report_reasons() -> Vec<crate::net::report::ReportReason> {
    crate::net::report::parse_reasons(
        br#"[{"id":"spam","label":"Spam","help":"Unwanted ads or the same message again and again."},
             {"id":"harassment","label":"Harassment","help":"Someone keeps contacting you after you asked them to stop, or tries to frighten or shame you."},
             {"id":"threats","label":"Threats","help":"A threat of harm to you or anyone else."},
             {"id":"child_danger","label":"A child may be in danger","help":"If anyone is in danger right now, contact your local emergency number. This server's admins are volunteers, not police."},
             {"id":"other","label":"Something else","help":"Say what is happening in the note."}]"#,
    )
    .expect("the snapshot reasons")
}

// The Report dialog for a DM report (step D of docs/design/blocking-and-safe-mode.md 10e,
// 2026-10-09): Harassment chosen with its help text, three of the person's messages with the
// newest ticked, the line about messages with files, a note, and "Also block them" ticked as it
// is by default for a DM report. Built in memory; nothing is sent or saved.
#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_report_dialog() {
    render_page_png("report_dialog", 760, 980, |ctx, theme, state| {
        if state.reports.dialog.is_none() {
            use crate::net::report::{DmCandidate, Evidence, ReportContext, ReportDialog};
            state.profile_public_key = "me".to_string();
            state.reports.reasons = snapshot_report_reasons();
            let msg = |ts: u64, text: &str, ticked: bool| DmCandidate {
                item: Evidence::Dm { from: "dana".into(), to: "me".into(), ts, text: text.into(), sig: "c2ln".into() },
                ts,
                text: text.into(),
                ticked,
            };
            state.reports.dialog = Some(ReportDialog {
                target: "dana".into(),
                target_name: "Dana Okafor".into(),
                context: Some(ReportContext::Dm),
                reason: "harassment".into(),
                note: "This started after I left the group on Tuesday.".into(),
                candidates: vec![
                    msg(1_791_503_600_000, "I know which building you live in.", true),
                    msg(1_791_500_000_000, "Answer me. I will keep writing until you do.", false),
                    msg(1_791_417_600_000, "Why did you leave the group?", false),
                ],
                also_block: true,
                ..Default::default()
            });
        }
        egui::CentralPanel::default().show(ctx, |_| {});
        crate::gui::pages::chat::draw_report_dialog(ctx, theme, state);
    });
}

// The Report dialog with the help outside this server open (10e-ii, 2026-10-10): a report from a
// person's profile, "A child may be in danger" chosen with its help text, and under it the block
// for France (the shipped file's entry with two other numbers): the country picker, the emergency
// number, the other numbers, the child report line, and the date the numbers were checked, all
// read from the real data/safety/outside_help.json built into the exe. Built in memory; nothing
// is sent or saved.
#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_report_dialog_outside_help() {
    render_page_png("report_dialog_outside_help", 760, 900, |ctx, theme, state| {
        if state.reports.dialog.is_none() {
            use crate::net::report::{ReportContext, ReportDialog};
            state.profile_public_key = "me".to_string();
            state.reports.reasons = snapshot_report_reasons();
            state.reports.outside_help = Some(
                crate::net::outside_help::parse(crate::embedded_data::OUTSIDE_HELP_JSON.as_bytes()).expect("the shipped outside help"),
            );
            state.reports.dialog = Some(ReportDialog {
                target: "dana".into(),
                target_name: "Dana Okafor".into(),
                context: Some(ReportContext::Profile),
                reason: "child_danger".into(),
                country: "FR".into(),
                ..Default::default()
            });
        }
        egui::CentralPanel::default().show(ctx, |_| {});
        crate::gui::pages::chat::draw_report_dialog(ctx, theme, state);
    });
}

// The Report dialog for a P2P group message (10j of docs/design/blocking-and-safe-mode.md,
// 2026-10-10): the group's creator found, "Send this report to" with the three destinations and
// the creator ticked by default, "The group's creator will see that you sent this.", the message
// with the line saying the creator can check it against their own copy, and the note addressed to
// the creator. Built in memory; nothing is sent or saved.
#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_report_dialog_group() {
    render_page_png("report_dialog_group", 760, 1040, |ctx, theme, state| {
        if state.reports.dialog.is_none() {
            use crate::net::group_report::{GroupTarget, Item};
            use crate::net::report::{Evidence, ReportContext, ReportDialog};
            let (me, cy, ben) = ("a1".repeat(32), "c3".repeat(32), "b2".repeat(32));
            state.profile_public_key = me;
            state.reports.reasons = snapshot_report_reasons();
            let text = "Nobody here wants you. Leave or I will make you.";
            let ts = 1_791_503_600_000;
            state.reports.dialog = Some(ReportDialog {
                target: cy.clone(),
                target_name: "Cy Moreau".into(),
                context: Some(ReportContext::Group),
                reason: "harassment".into(),
                note: "He has said this to three of us this week.".into(),
                fixed: Some(Evidence::GroupText { from: cy.clone(), ts, text: text.into() }),
                fixed_text: text.into(),
                group: Some(GroupTarget {
                    id: "ab".repeat(32),
                    name: "Riverside Hikers".into(),
                    item: Some(Item { id: "01".repeat(32), from: cy, ts, text: text.into() }),
                    creator: Some(ben),
                    finding: false,
                    send_to: None,
                }),
                ..Default::default()
            });
        }
        egui::CentralPanel::default().show(ctx, |_| {});
        crate::gui::pages::chat::draw_report_dialog(ctx, theme, state);
    });
}

// Settings > Safety > Reports about your groups (10j, 2026-10-10), as the creator of "Riverside
// Hikers" sees it: Ann's report about Cy with her note and two messages, one found in the
// creator's copy (signed by Cy) and one not, and the three actions; and an older report about Dee
// already acted on, with "Removed from the group." and "You have blocked them.". The DM store and
// the block list are built in memory and never saved, so nothing lands on disk.
#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_safety_group_reports() {
    render_page_png("safety_group_reports", 960, 900, |ctx, theme, state| {
        if state.dm_store.is_none() {
            use crate::net::group_report::{CheckedItem, KeptReport};
            let (me, ann, cy, dee) = ("a1".repeat(32), "b2".repeat(32), "c3".repeat(32), "d4".repeat(32));
            let g = "ab".repeat(32);
            state.profile_public_key = me.clone();
            state.reports.reasons = snapshot_report_reasons();
            for (name, key) in [("Ann Lindqvist", &ann), ("Cy Moreau", &cy), ("Dee Park", &dee)] {
                state.chat_users.push(crate::gui::ChatUser { name: name.into(), public_key: key.clone(), role: String::new(), status: "online".into() });
            }
            state.p2p_groups = vec![crate::net::api_v2::P2pGroupInfo {
                group_id: g.clone(),
                name: "Riverside Hikers".into(),
                members: vec![me.clone(), ann.clone(), cy.clone()],
                is_creator: true,
            }];
            let mut list = crate::net::block_list::BlockList::in_temp(&[7u8; 32], &me, "snapshot-group-reports");
            list.block(&dee, 1_791_417_600_000);
            state.block_list = Some(list);
            let item = |n: u8, ts: u64, text: &str, found: bool| CheckedItem {
                id: format!("{n:02x}").repeat(32),
                from: cy.clone(),
                ts,
                text: text.into(),
                found,
                signer: if found { cy.clone() } else { String::new() },
            };
            let mut store = crate::net::dm_store::DmStore::load(&[7u8; 32], &me, "wss://snapshot.group-reports.invalid");
            for kept in [
                KeptReport {
                    id: "r1".into(),
                    from: ann.clone(),
                    ts: 1_791_504_000_000,
                    group_id: g.clone(),
                    group_name: "Riverside Hikers".into(),
                    target: cy.clone(),
                    reason: "harassment".into(),
                    note: "He has said this to three of us this week.".into(),
                    items: vec![
                        item(1, 1_791_503_600_000, "Nobody here wants you. Leave or I will make you.", true),
                        item(2, 1_791_503_700_000, "I know where the Saturday walk starts.", false),
                    ],
                    removed: false,
                },
                KeptReport {
                    id: "r2".into(),
                    from: ann.clone(),
                    ts: 1_791_331_200_000,
                    group_id: g.clone(),
                    group_name: "Riverside Hikers".into(),
                    target: dee.clone(),
                    reason: "spam".into(),
                    note: String::new(),
                    items: vec![],
                    removed: true,
                },
            ] {
                store.settle_group_report(&kept.id.clone(), Some(kept));
            }
            state.dm_store = Some(store);
        }
        settings_panel(ctx, theme, state, |ui, theme, state| {
            crate::gui::pages::safety_group_reports::draw_section(ui, theme, state, theme.info())
        });
    });
}

// Server Settings > Moderator > Reports as an admin sees it (step D, 10e): an open DM report with
// one item whose signature the server checked and one it could not prove, the sentence on what a
// checked signature does not prove, and the decision buttons (Ban shown, as for an admin). Built
// in memory; no `reports_list` is sent.
#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_reports_page() {
    render_page_png("reports_page", 960, 900, |ctx, theme, state| {
        if state.reports.list.is_empty() {
            state.profile_public_key = "me".to_string();
            state.reports.reasons = snapshot_report_reasons();
            state.reports.requested = true;
            state.reports.list = crate::net::report::parse_reports(&serde_json::json!({ "type": "reports", "items": [
                { "id": 12, "target": "dana0000000000000000", "target_name": "Dana Okafor", "context": "dm",
                  "reason": "harassment", "reason_label": "Harassment", "note": "This started after I left the group on Tuesday.",
                  "created_at": 1_791_504_000_000u64, "state": "open", "reporter": "me", "reporter_name": "Sam",
                  "evidence": [
                    { "kind": "dm", "from": "dana0000000000000000", "to": "me", "ts": 1_791_500_000_000u64,
                      "text": "Answer me. I will keep writing until you do.", "checked": true },
                    { "kind": "dm", "from": "dana0000000000000000", "to": "me", "ts": 1_791_503_600_000u64,
                      "text": "I know which building you live in.", "checked": false } ] }
            ]}));
        }
        settings_panel(ctx, theme, state, |ui, theme, state| {
            crate::gui::pages::server_settings::reports::draw(ui, theme, state, true)
        });
    });
}

// Server Settings > Members with the "Erase their data" confirm open (10i of
// docs/design/blocking-and-safe-mode.md, 2026-10-10), as an admin sees it: the whole page drawn
// by its own `draw`, the roster behind (the action on Dana's and Cy's rows, none on the admin's
// own row, Bob the admin's or Zed the owner's), and over it 10i's words, the name field with
// Dana's name typed in the wrong case so Erase is still dead. Built in memory; nothing is sent.
#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_admin_erase_confirm() {
    render_page_png("admin_erase_confirm", 960, 640, |ctx, theme, state| {
        if state.admin_erase.confirm.is_none() {
            state.profile_public_key = "me".to_string();
            state.server_settings_tab = 1;
            let user = |name: &str, key: &str, role: &str| ChatUser { name: name.into(), public_key: key.into(), role: role.into(), status: "online".into() };
            state.chat_users = vec![
                user("Sam", "me", "admin"),
                user("Dana", "dana0000000000000000", "member"),
                user("Bob", "bob00000000000000000", "admin"),
                user("Cy", "cy000000000000000000", "mod"),
                user("Zed", "zed00000000000000000", "owner"),
            ];
            state.admin_erase.confirm = Some(crate::net::admin_erase::EraseConfirm {
                target: "dana0000000000000000".into(),
                name: "Dana".into(),
                typed: "dana".into(),
            });
        }
        crate::gui::pages::server_settings::draw(ctx, theme, state);
    });
}

#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_credits_settings() {
    // Third-party attributions. Rendered so the ODbL obligation is REVIEWABLE:
    // a licence notice that silently stops drawing is a compliance failure, not
    // a cosmetic one, and a snapshot is how that gets noticed.
    render_page_png("credits_settings", 960, 1400, |ctx, theme, state| {
        state.credits = crate::credits::Credits::load(std::path::Path::new(
            concat!(env!("CARGO_MANIFEST_DIR"), "/data"),
        ));
        settings_panel(ctx, theme, state, crate::gui::pages::settings::draw_credits_content);
    });
}

#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_gameplay_settings() {
    // Settings > Gameplay (2026-10-04): the realism switches, the home design
    // and the Pipe markings row (Simplified / Full) at the end. Tall canvas so
    // the whole section is one reviewable image.
    render_page_png("gameplay_settings", 960, 2150, |ctx, theme, state| {
        settings_panel(ctx, theme, state, crate::gui::pages::settings::draw_gameplay_content);
    });
}

#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_graphics_settings() {
    // Tall canvas (v0.1117): Graphics gained pronounced subsections (General /
    // Planets seen from space / Ground detail / Detail distances / Light and
    // sky / Experimental / Machine labels and Home view), and a 900 px crop
    // showed only the first two. Render the whole section so the subsection
    // ladder is reviewable in one image.
    render_page_png("graphics_settings", 960, 3800, |ctx, theme, state| {
        settings_panel(ctx, theme, state, crate::gui::pages::settings::draw_graphics_content);
    });
}

#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_cloud_dev() {
    // The F10 Cloud dev panel with its NEEDS TESTING list and per-row TEST
    // tags (v0.1289, data/gui/dev_tests.json). Since 2026-09-05 it is a
    // LEFT SIDEBAR (egui SidePanel, default 420 wide) whose whole body
    // scrolls, with a slim collapse tab on its right edge. 720 wide so the
    // sidebar's right edge, its tab, and the empty world area beside it
    // are all in the capture; 1400 tall shows the scroll bar clipping the
    // long switch list rather than the list running off the canvas.
    render_page_png("cloud_dev", 720, 1400, |ctx, theme, state| {
        state.show_cloud_dev_panel = true;
        state.cloud_dev_collapsed = false;
        crate::gui::pages::cloud_dev::draw(ctx, theme, state);
    });
}

#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_cloud_dev_collapsed() {
    // The same panel collapsed to its edge tab (2026-09-05): only the slim
    // strip with the expand arrow should remain at the left screen edge.
    // Short canvas: there is nothing below the arrow to review.
    render_page_png("cloud_dev_collapsed", 320, 240, |ctx, theme, state| {
        state.show_cloud_dev_panel = true;
        state.cloud_dev_collapsed = true;
        // The tab is only drawn while the cursor is free (a grabbed cursor
        // must find no click target at the screen edge); the headless
        // harness has no cursor, so say it is free, as it would be on a
        // menu page or with Alt held.
        state.cursor_free = true;
        crate::gui::pages::cloud_dev::draw(ctx, theme, state);
    });
}

#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_controls_settings() {
    // 1900 tall (2026-08-12): the Controls section grew the full rebindable
    // keymap (five grouped bind grids) plus the Fixed-keys reference card;
    // at the settings-default 900 everything below the Camera group fell
    // off the bottom of the capture.
    render_page_png("controls_settings", 960, 1900, |ctx, theme, state| {
        settings_panel(ctx, theme, state, crate::gui::pages::settings::draw_controls_content);
    });
}

#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_widgets_settings() {
    // The Widgets tab (theme sliders + live row preview) plus the v0.1214
    // Effects test bench at the bottom (buttons, toggle, hold-to-confirm bar +
    // pinwheel, channeling swatches). Rendered with a mutable Theme because the
    // token sliders write to it (settings_panel only offers &Theme).
    render_page_png("widgets_settings", 960, 3700, |ctx, theme, state| {
        theme.apply_to_egui(ctx);
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                crate::gui::pages::settings::draw_widgets_content(ui, theme, state);
            });
        });
    });
}

#[test]
    #[ignore = "GPU snapshot; run via `just snapshots`"]
    fn snapshot_laws_page() {
    render_page_png("laws_page", 1400, 1400, |ctx, theme, state| {
        crate::gui::pages::laws::draw(ctx, theme, state);
    });
}

/// Inventory > The fleet (2026-10-04, pages/fleet_ledger.rs): a player in the shared world
/// standing at the mess hall's stores, 4 CR in the red after one meal and two loaves given,
/// with the give form showing the backpack's bread.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_fleet_ledger() {
    use crate::gui::pages::fleet_ledger as fleet;
    render_page_png("fleet_ledger", 1000, 900, |ctx, theme, state| {
        state.copresence_active = true;
        state.fleet.ledger = Some(fleet::tests::demo_ledger());
        state.fleet.stores = vec![fleet::FleetStore { entity_id: 12, name: "The mess hall's stores".into(), position: [67.0, 1.0, 22.0] }];
        state.fleet.my_position = Some([68.0, 1.7, 22.0]);
        state.fleet.prices.insert("bread_0".into(), 3.0);
        state.inventory_items = vec![Some(GuiItemSlot { item_id: "bread_0".into(), name: "Bread".into(), quantity: 3, ..Default::default() })];
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
            .show(ctx, |ui| fleet::draw_section(ui, theme, state));
    });
}

#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_watch() {
    render_page_png("watch", 1280, 900, |ctx, theme, state| {
        // Empty-directory state: no streams live, no viewer open. Exercises the
        // page layout, the "Nobody is streaming" state, and the watch-by-name entry
        // without needing a network connection.
        crate::gui::pages::watch::draw(ctx, theme, state);
    });
}

#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_nav_bar_wrapped() {
    // Narrow width (v0.859): the top nav must WRAP its buttons into extra rows
    // rather than clip them off the right edge. 720px is too narrow for all ~18
    // buttons on one row, so this proves the wrap.
    render_page_png("nav_bar_wrapped", 720, 200, |ctx, theme, state| {
        crate::gui::pages::escape_menu::draw_nav_bar(ctx, theme, state);
    });
}

#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_toast() {
    // The confirmation toast that fires on save (v0.861). Push one, render it.
    render_page_png("toast", 700, 320, |ctx, theme, state| {
        // Push on the FIRST frame only: the closure runs once per rendered
        // frame, and pushing every frame stacked a second copy once the
        // harness began rendering three frames instead of two.
        if state.toasts.is_empty() {
            let now = ctx.input(|i| i.time);
            state.toast("Theme saved", crate::gui::ToastKind::Success, now);
        }
        crate::gui::widgets::draw_toasts(ctx, theme, state);
    });
}

/// A long notice stays on the window (first-hour audit 2026-10-04). A toast's text never
/// wrapped, so a notice longer than the window ran off both edges and could not be read: the
/// guest notice, and before it the shared world's refusal sentences (engine/home_plot.rs
/// `refuse_shared_world`). Each toast now wraps inside the window and stands above the one
/// below by its real height. GPU-free: the toasts' laid-out rects, from egui's memory.
///
/// Seen red 2026-10-04 on 8e400d7ed (no wrap, a fixed 40 point step): "toast 0 runs off the
/// window: [[-520.2 269.0] - [1800.2 312.0]] on [[0.0 0.0] - [1280.0 360.0]]".
#[test]
fn a_long_notice_wraps_inside_the_window() {
    let ctx = snapshot_ctx();
    let theme = load_theme();
    theme.apply_to_egui(&ctx);
    let mut state = GuiState::default();
    state.pending_notices.push(crate::engine::home_plot::GUEST_ARRIVAL.to_string());
    state.pending_notices.push(super::first_steps::controls_hint(&state.keybinds));
    let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 360.0));
    for _ in 0..3 {
        let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
        ctx.run(input, |ctx| crate::gui::widgets::draw_toasts(ctx, &theme, &mut state));
    }
    let rects: Vec<egui::Rect> = (0..2)
        .map(|i| ctx.memory(|m| m.area_rect(egui::Id::new(("hos_toast", i)))).expect("the toast was laid out"))
        .collect();
    for (i, r) in rects.iter().enumerate() {
        assert!(screen.contains_rect(*r), "toast {i} runs off the window: {r:?} on {screen:?}");
    }
    assert!(!rects[0].intersects(rects[1]), "the toasts overlap: {rects:?}");
    assert!(rects[0].height() > rects[1].height() + 10.0, "the long notice is still one line: {rects:?}");
}

/// The two notices a new player's first session can show (first-hour audit 2026-10-04): the
/// controls hint on the first entry into the world (gui/first_steps.rs) and, on a shared
/// server with every plot taken, the guest notice (engine/home_plot.rs `GUEST_ARRIVAL`). Both
/// ride the long-lived notice toast; this shows how they read on a 1280 px window.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_first_steps_notices() {
    render_page_png("first_steps_notices", 1280, 360, |ctx, theme, state| {
        // First frame only, as `snapshot_toast` does: the closure runs once per frame.
        if state.toasts.is_empty() {
            let now = ctx.input(|i| i.time);
            state.notice(crate::engine::home_plot::GUEST_ARRIVAL, now);
            state.notice(super::first_steps::controls_hint(&state.keybinds), now);
        }
        crate::gui::widgets::draw_toasts(ctx, theme, state);
    });
}

#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_markdown_features() {
    // The markdown renderer's three v0.1305 behaviours, in one picture, because
    // the Library snapshot happens to open a document that exercises none of
    // them. Each block below renders WRONG under the old line-at-a-time
    // renderer: the paragraph would be four ragged one-line paragraphs, the
    // bullet would end at "that", and the table would be raw pipe text.
    // Mirrored by the web renderer's unit cases in web/shared/markdown.js.
    const SAMPLE: &str = "\
# Markdown features

A paragraph hard-wrapped in the source across
several lines, which markdown joins into one
paragraph, and **bold that spans a line
break** has to survive the join.

- A bullet whose text runs past the end of
  the first line and continues indented, which
  is how 42 of the 81 Library documents write.
- A short bullet.

| Material | Fails at | How |
|---|---|---|
| Epoxy matrix | 120 C | Softens |
| Kevlar | 450 C | Chars |
| Titanium | 1668 C | Melts |

> **Ratification history.** A multi-line block quote, which is how the
> Constitution separates House Manual editorial matter from constitutional
> text. Before v0.1305 this had no branch at all and the marker rendered
> as literal text.

A paragraph carrying a cross-reference to the
[self-hosting guide](/library#self-hosting) and another to
[the roadmap](/library#roadmap), which before v0.1306.9 printed their own
brackets and URLs on screen because the native reader had no link handling.

```bash
# This comment is NOT a heading. The renderer had no fence handling at all,
# so SELF-HOSTING.md drew 50 shell comments at title size and buried its
# real sections among them.
sudo certbot --nginx -d united-humanity.us
```

Closing paragraph after the table.";

    render_page_png("markdown_features", 760, 660, |_ctx, theme, _state| {
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
            .show(_ctx, |ui| {
                let mut _link: Option<String> = None;
                crate::gui::widgets::markdown::render_markdown_linked(ui, theme, SAMPLE, &mut _link);
            });
    });
}

#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_nav_bar_icon_only() {
    // Icon-only compact mode (v0.859): labels hidden, icons remain.
    render_page_png("nav_bar_icon_only", 720, 200, |ctx, theme, state| {
        state.nav_display_mode = crate::gui::NavDisplayMode::IconOnly;
        crate::gui::pages::escape_menu::draw_nav_bar(ctx, theme, state);
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

/// The last onboarding page (first-hour audit 2026-10-04, Blocker 1): it says where Enter puts
/// the player, their own home and alone unless they connected to a server on the server step,
/// and where the shared world is (Characters, then a server).
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_onboarding_ready() {
    render_page_png("onboarding_ready", 1280, 900, |ctx, theme, state| {
        state.onboarding_complete = false;
        state.onboarding_step = 3;
        state.user_name = "Explorer".to_string();
        state.server_connected = false;
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

/// The main menu prints the build version, so it is pinned to a fixed,
/// plainly fake one; unpinned, this baseline changed on every release.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_main_menu() {
    crate::gui::pages::main_menu::set_version_for_snapshot(Some("0.0.0-snapshot"));
    render_page_png("main_menu", 1280, 900, |ctx, theme, state| {
        crate::gui::pages::main_menu::draw(ctx, theme, state);
    });
    crate::gui::pages::main_menu::set_version_for_snapshot(None);
}
page_snapshot!(snapshot_humanity, "humanity", humanity, 1280, 900);
page_snapshot!(snapshot_chat, "chat", chat, 1280, 900);

/// The Commons MERGED view, rendered from two populated background carrier
/// connections (field test 5: the operator's Commons rooms rendered empty).
/// This is the falsifiable repro: if the merge breaks, this snapshot shows
/// the empty welcome instead of the four lines.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_chat_commons() {
    render_page_png("chat_commons", 1280, 900, |ctx, theme, state| {
        // The closure runs once per rendered frame; only seed on the first
        // (a second push doubled every carrier and every line, which
        // usefully exposed the native-line dedup gap, now fixed).
        if !state.connections.is_empty() {
            state.chat_active_channel = "commons:general".into();
            crate::gui::pages::chat::draw(ctx, theme, state);
            return;
        }
        let mk_conn = |url: &str, lines: &[(&str, &str, u64)]| crate::gui::ServerConnection {
            url: url.into(),
            display_url: url.into(),
            identified: true,
            channels: vec![ChatChannel {
                id: "general".into(),
                name: "general".into(),
                federated: true,
                ..Default::default()
            }],
            messages: lines
                .iter()
                .map(|(who, text, ts)| ChatMessage {
                    sender_name: (*who).into(),
                    sender_key: format!("k_{who}"),
                    content: (*text).into(),
                    timestamp: "12:00".into(),
                    timestamp_ms: *ts,
                    channel: "general".into(),
                    server: url.into(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        state.connections.push(mk_conn(
            "https://a.example",
            &[("Alice", "hello from server A", 1000), ("Ada", "good morning", 3000)],
        ));
        state.connections.push(mk_conn(
            "https://b.example",
            &[("Bela", "hello from server B", 2000), ("Bix", "hey all", 4000)],
        ));
        state.chat_active_channel = "commons:general".into();
        crate::gui::pages::chat::draw(ctx, theme, state);
    });
}

/// Step F (docs/design/blocking-and-safe-mode.md 10g): a direct message from someone who is not a
/// friend, asking for a gift card while claiming to be an admin, with both warnings under it,
/// and a second holding a link, with the link line and its Open button. Built in memory from the
/// shipped warnings file; nothing is sent.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_chat_warning() {
    render_page_png("chat_warning", 1280, 900, |ctx, theme, state| {
        if state.chat_active_channel != "dm:dana0" {
            state.profile_public_key = "me".into();
            state.warnings.list = Some(
                crate::net::warnings::parse_warnings(crate::embedded_data::WARNINGS_JSON.as_bytes()).expect("the shipped warnings"),
            );
            let dm = |text: &str, ts: u64, at: &str| ChatMessage {
                sender_name: "Dana".into(),
                sender_key: "dana0".into(),
                content: text.into(),
                timestamp: at.into(),
                timestamp_ms: ts,
                channel: "dm:dana0".into(),
                ..Default::default()
            };
            state.chat_messages.push(dm("Hi! I'm an admin here. To keep your account open, buy a gift card and send me the code.", 1_791_500_000_000, "12:40"));
            state.chat_messages.push(dm("Here is the form: https://example.com/verify", 1_791_500_060_000, "12:41"));
            state.chat_active_channel = "dm:dana0".into();
        }
        crate::gui::pages::chat::draw(ctx, theme, state);
    });
}
/// Maps, Solar System view. The page opens on TODAY's sky and reads the wall
/// clock for "days from today", so it is pinned to 2026-09-27 12:00 UTC
/// (843,782,400 s after J2000.0); unpinned, the picture changed on every run.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_cosmos() {
    crate::gui::pages::cosmos::set_clock_for_snapshot(Some(843_782_400.0));
    render_page_png("cosmos", 1280, 900, |ctx, theme, state| {
        crate::gui::pages::cosmos::draw(ctx, theme, state);
    });
    crate::gui::pages::cosmos::set_clock_for_snapshot(None);
}
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
    // 2600 tall (2026-09-27): the inspect + "Move to" card this snapshot exists
    // to show sits BELOW every place tree, and the Home place grew a Barn, which
    // pushed the card past the old 1700 px canvas. The capture then came out
    // byte-identical to snapshot_inventory and guarded nothing.
    render_page_png("inventory_transfer", 1280, 2600, |ctx, theme, state| {
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
    render_page_png("relay_control_health", 1100, 1500, |ctx, theme, state| {
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

/// Relay Control Center — "Host a node on this PC". Rendered with NO relay
/// selected, which is what a person with no server of their own actually sees:
/// the page used to be a dead end there, and now it leads with the form that
/// starts a server on this machine (port / database / name + Start).
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_relay_control_host_node() {
    // The form's defaults come from this machine (its name, its profile
    // folder); fixed values keep the picture the same on every PC. They match
    // the running-node snapshot below.
    crate::gui::pages::host_node::seed_setup_for_snapshot(
        "Shaostoul's node",
        r"C:\Users\Shaos\AppData\Roaming\HumanityOS\relay\relay.db",
    );
    render_page_png("relay_control_host_node", 1100, 900, |ctx, theme, state| {
        // No saved servers and no connected relay: the empty-selection path.
        state.chat_servers.clear();
        state.server_url.clear();
        state.relay_cc_selected = None;
        crate::gui::pages::relay_control::draw(ctx, theme, state);
    });
    crate::gui::pages::host_node::reset_for_snapshot();
}

/// Relay Control Center — a node that IS serving: the addresses to hand to a
/// friend, the database file, uptime, and Stop. Seeded through a test-only hook
/// so no port is bound and no database is opened.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_relay_control_host_node_running() {
    crate::gui::pages::host_node::force_running_for_snapshot(
        3210,
        "Shaostoul's node",
        r"C:\Users\Shaos\AppData\Roaming\HumanityOS\relay\relay.db",
        Some("192.168.1.24"),
    );
    render_page_png("relay_control_host_node_running", 1100, 900, |ctx, theme, state| {
        state.chat_servers.clear();
        state.server_url.clear();
        state.relay_cc_selected = None;
        crate::gui::pages::relay_control::draw(ctx, theme, state);
    });
    // The node state is process-global; leaving it Running would put a "this PC"
    // row in the rail of every later Relays snapshot.
    crate::gui::pages::host_node::reset_for_snapshot();
}

/// Native donate page: the two ways to give (the nonprofit Sponsor-a-Can and the
/// maintainer on Patreon, from the shipped data/donate/routes.json) above the
/// other direct links and the crypto cards. The data files come from the base
/// fixture, so the snapshot shows what ships; only the two example crypto
/// addresses are made up, to show those cards.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_donate() {
    render_page_png("donate", 1000, 1600, |ctx, theme, state| {
        state.donate_solana_address = "So1anaExampleAddress1111111111111111111111".into();
        state.donate_btc_address = "bc1qexamplebitcoinaddress00000000000000".into();
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

/// Planet Tuner (v0.1120): seeded as if standing on Mars in Dev mode, so the
/// live readout, every editor group, and the save row all render instead of
/// the gate cards. The def is parsed from real mars.ron field values.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_planet_tuner() {
    render_page_png("planet_tuner", 1280, 2200, |ctx, theme, state| {
        state.active_platform_section = "planet_tuner".into();
        state.settings.play_mode = crate::config::PlayMode::Dev;
        theme.cheats_enabled = true;
        let def: crate::terrain::planet::PlanetDef = ron::from_str(
            r#"(
                name: "Mars",
                radius: 3389500.0,
                gravity: 3.71,
                terrain_seed: 7,
                ore_seed: 8,
                atmosphere_color: Some((0.75, 0.45, 0.25, 0.12)),
                atmosphere_scale: 0.012,
                scale_height_m: Some(11100.0),
                sea_level: 0.0,
                surface_relief: 0.006,
            )"#,
        )
        .expect("snapshot def parses");
        let t = &mut state.planet_tuner;
        t.readout.locked = true;
        t.readout.body_id = "mars".into();
        t.readout.body_name = "Mars".into();
        t.readout.has_def = true;
        t.readout.g_at_player = 3.71;
        t.readout.altitude_m = 1250.0;
        t.readout.latitude_deg = -14.6;
        t.readout.temp_at_player_c = -63.0;
        t.readout.temp_global_c = -60.0;
        t.readout.breathable_outside = false;
        t.readout.day_length_hours = 24.6;
        t.curve_text = crate::gui::pages::planet_tuner::fmt_curve(&def.gravity_curve);
        t.current = Some(def.clone());
        t.working = Some(def);
        t.working_body = "mars".into();
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
        // The room-type registry (data/rooms.ron) the app loads on entering
        // the editor (lib.rs), and the Add-room picker's default, the first
        // key. Unseeded, the picker drew as an empty box.
        if state.construction_room_types.is_empty() {
            let reg = crate::ship::room_types::RoomTypeRegistry::load(std::path::Path::new("data"));
            let mut keys: Vec<String> = reg.types.keys().cloned().collect();
            keys.sort();
            state.construction_add_type = keys.first().cloned().unwrap_or_default();
            state.construction_room_types = keys;
            state.room_type_registry = reg;
        }
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

/// The Maps page's Galaxy view (v0.1145): the real HYG catalog rendered
/// top-down, proving the load + magnitude-ladder draw path headlessly.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_maps_galaxy() {
    render_page_png("maps_galaxy", 1280, 900, |ctx, theme, state| {
        state.cosmos_view = crate::gui::pages::cosmos::CosmosView::Galactic;
        crate::gui::pages::cosmos::draw(ctx, theme, state);
    });
}

/// The WHO/WHERE picker (v0.1147): the Play/Characters restructure's mode-0
/// surface with the bottom pairing bar, proving the three-panel layout.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_showroom_picker() {
    render_page_png("showroom_picker", 1280, 900, |ctx, theme, state| {
        state.showroom_mode = 0;
        state.launcher_open_select = true;
        // Seed the WHERE column instead of letting the page rescan the saves
        // directory. Left to itself it read the saves of whatever machine ran
        // the test (the operator's own "My Homestead", "1 days ago"), so the
        // picture changed with the day and with the machine. One home, played
        // 26 hours ago, selected the way `preselect` would select it.
        if !state.launcher_homes_loaded {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            state.launcher_homes = vec![crate::gui::LauncherHome {
                world: "My Homestead".into(),
                character: "Wanderer".into(),
                design: "fibonacci".into(),
                timestamp: now.saturating_sub(26 * 3600),
            }];
            state.launcher_homes_loaded = true;
            state.launcher_who = "My Homestead".into();
            state.launcher_where_kind = crate::gui::LauncherWhere::Home;
            state.launcher_selected_world = "My Homestead".into();
        }
        crate::gui::pages::showroom::draw(ctx, theme, state);
    });
}

/// The Maps page's Planet view (v0.1146): the Seattle Center OSM region
/// rendered GPS-style (roads by class, building footprints, labels).
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_maps_planet() {
    render_page_png("maps_planet", 1280, 900, |ctx, theme, state| {
        state.cosmos_view = crate::gui::pages::cosmos::CosmosView::Planet;
        state.cosmos_zoom = 2.0;
        crate::gui::pages::cosmos::draw(ctx, theme, state);
    });
}

/// The Market's in-app publish view (v0.1143): the shop + offering forms
/// rendered open, proving the whole form lays out (the plain market snapshot
/// shows the Directory browse; this one shows "+ Publish" clicked).
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_market_publish() {
    render_page_png("market_publish", 1280, 1100, |ctx, theme, state| {
        // An identity is required for the form to render (real key material
        // is irrelevant to layout; publishing is never triggered here).
        if state.private_key_bytes.is_none() {
            state.private_key_bytes = Some(vec![7u8; 32]);
            state.server_url = "https://united-humanity.us".into();
            crate::gui::pages::market_publish::open(None);
        }
        crate::gui::pages::market::draw(ctx, theme, state);
    });
}
page_snapshot!(snapshot_profile, "profile", profile, 1280, 900);
page_snapshot!(snapshot_crafting, "crafting", crafting, 1280, 900);

/// The Crafting page with a spacecraft pod selected (BUG-147): each part
/// split between the backpack and home storage, the last part 2 short in
/// both together, and the pod (far bigger than the backpack) bound for home
/// storage. The plain page snapshot selects no recipe, so it shows none of it.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_crafting_home_storage() {
    // Tall enough to show the Produces card (where the pod will go) below
    // the pod's 27 parts.
    render_page_png("crafting_home_storage", 1280, 1500, |ctx, theme, state| {
        if state.craft_selected.is_none() {
            let idx = state.craft_recipes.iter().position(|r| r.id == "build_spacecraft_pod").expect("the pod recipe ships");
            state.craft_selected = Some(idx);
            let parts = state.craft_recipes[idx].inputs.clone();
            let last = parts.len() - 1;
            for (i, (id, need)) in parts.into_iter().enumerate() {
                let slot = GuiItemSlot { item_id: id.clone(), name: id.clone(), quantity: need / 3, ..Default::default() };
                state.inventory_items.push(Some(slot));
                state.home_stock.insert(id, if i == last { need - need / 3 - 2 } else { need });
            }
        }
        crate::gui::pages::crafting::draw(ctx, theme, state);
    });
}

/// The Tools card and the line under a greyed Craft button name a tool the
/// way the Ingredients and Produces cards name a part ("Wrench Adjustable"),
/// never by its item id. Headless (no GPU): the drawn text is read back from
/// the frame's shapes. Every part is in home storage and no tool is carried,
/// so the button is greyed for the tool alone. Seen red before the fix: "the
/// Tools card names wrench_adjustable_0 by its item id".
#[test]
fn crafting_names_tools_like_parts() {
    let mut state = demo_state();
    let idx = state.craft_recipes.iter().position(|r| r.id == "build_spacecraft_pod").expect("the pod recipe ships");
    let recipe = state.craft_recipes[idx].clone();
    assert!(!recipe.tools.is_empty(), "the pod needs hand tools");
    state.craft_selected = Some(idx);
    state.home_storage_here = true;
    state.stations_where = crate::systems::construction::StationsWhere::Home;
    for (id, need) in &recipe.inputs {
        state.home_stock.insert(id.clone(), *need);
    }
    let ctx = snapshot_ctx();
    let theme = load_theme();
    theme.apply_to_egui(&ctx);
    let mut out = None;
    for _ in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 4000.0))),
            ..Default::default()
        };
        out = Some(ctx.run(input, |ctx| crate::gui::pages::crafting::draw(ctx, &theme, &mut state)));
    }
    let shapes = out.expect("frames ran").shapes;
    let drawn = |t: &str| crate::gui::screen_surface::find_text_in_shapes(&shapes, t).is_some();
    for tool in &recipe.tools {
        assert!(!drawn(tool), "the Tools card names {tool} by its item id");
        assert!(drawn(&crate::gui::pages::crafting::pretty_id(tool)), "{tool} is named as a part is");
    }
    let first = crate::gui::pages::crafting::pretty_id(&recipe.tools[0]);
    assert!(drawn(&format!("Needs a {first} in your backpack")), "the greyed button says which tool, by name");
}
page_snapshot!(snapshot_library, "library", library, 1280, 900);

/// A SHORT Library document that has siblings, so the ladder footer at the foot of the
/// page is actually photographed. The plain page snapshot opens whichever
/// document happens to be first, which today is alone on its shelf, so the
/// footer never appears there and a regression in it would be invisible.
/// The document is chosen to have a neighbour on EACH side, so one picture
/// guards both buttons. Short matters too: on a long document the footer sits far below the captured
/// viewport, and a snapshot that cannot show the thing it guards is not a guard.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_library_ladder() {
    render_page_png("library_ladder", 1280, 900, |ctx, theme, state| {
        assert!(
            crate::gui::pages::library::show_doc(state, "04-contributing"),
            "the Library no longer ships 04-contributing; pick another SHORT document with a neighbour on each side"
        );
        crate::gui::pages::library::draw(ctx, theme, state);
    });
}

/// The Library's third face: the syllabus, which is its map of ITSELF. Worth its
/// own picture because the page snapshot opens on a document and this view is
/// otherwise only reachable by clicking, so a regression here would be invisible.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_curriculum() {
    render_page_png("curriculum", 1280, 900, |ctx, theme, state| {
        crate::gui::pages::library::show_curriculum();
        crate::gui::pages::library::draw(ctx, theme, state);
    });
}
// snapshot_governance is a hand-written test above (needs injected proposal
// state to show the real feed; the bare macro version rendered an empty page).
page_snapshot!(snapshot_identity, "identity", identity, 1280, 900);
page_snapshot!(snapshot_wallet, "wallet", wallet, 1280, 900);
page_snapshot!(snapshot_quests, "quests", quests, 1280, 900);
page_snapshot!(snapshot_calendar, "calendar", calendar, 1280, 900);
page_snapshot!(snapshot_notes, "notes", notes, 1280, 900);

/// Performance page (resource budgets increment 1): the four live pies.
///
/// Headless, so there is no renderer writing real costs into the store. This
/// seeds REPRESENTATIVE numbers first - the same convention `demo_state` uses
/// for every other page - so the snapshot shows the laid-out pies instead of
/// four "no measurements yet" rings. The numbers are a plausible surface-level
/// frame, not a measurement; the real ones come from the probe rig's
/// `debug/frame_costs.json`.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_performance() {
    use crate::renderer::frame_costs as fc;
    render_page_png("performance", 1280, 1200, |ctx, theme, state| {
        fc::set_gpu_timing(true);
        fc::begin_frame();
        for (id, ms) in [
            ("gpu.celestial", 6.9_f32), ("gpu.shadow", 1.7), ("gpu.scene", 1.1),
            ("gpu.transparent", 0.4), ("gpu.overlay", 0.2), ("gpu.particles", 0.6),
            ("gpu.gpu_particles", 0.3), ("gpu.lines", 0.1), ("gpu.celestial_lines", 0.05),
        ] {
            // 40 folds so the smoothing settles on the seeded value.
            for _ in 0..40 {
                fc::record_gpu(id, ms);
            }
        }
        // Every stage must report in the SAME frame, then the frame closes:
        // a stage that misses a frame decays, which is the whole point of the
        // idle-decay rule.
        for _ in 0..40 {
            for (id, us) in [
                ("cpu.celestial", 2600_u64), ("cpu.scene", 700), ("cpu.transparent", 200),
                ("cpu.patch_upload", 900), ("cpu.grass_upload", 400), ("cpu.water_upload", 300),
                ("cpu.lights", 120), ("cpu.godrays", 90), ("cpu.ssao", 80),
            ] {
                fc::record_cpu(id, std::time::Duration::from_micros(us));
            }
            fc::begin_frame();
        }
        for (id, bytes) in [
            ("vram.patch_arena", 742_000_000_u64), ("vram.meshes", 180_000_000),
            ("vram.textures", 96_000_000), ("vram.render_targets", 44_000_000),
            ("vram.particles", 12_000_000), ("vram.grass", 8_000_000),
            ("vram.uniforms", 5_000_000), ("vram.driver_reserved", 1_450_000_000),
            ("vram.patch_arena_reserved", 1_541_000_000),
        ] {
            fc::set_vram(id, bytes);
        }
        fc::set_ram("ram.resident", 2_900_000_000);
        fc::set_ram("ram.committed_extra", 640_000_000);
        // Set LAST: the begin_frame calls above smooth the frame clock toward
        // the microseconds this test takes, which would leave no remainder.
        fc::set_frame_ms(13.9);
        crate::gui::pages::performance::draw(ctx, theme, state);
    });
}

/// The NARROW layout (operator: "single stacking the pie charts on mobile;
/// on PC it keeps the widget small"): under 620 px the four pies stack in one
/// column instead of the 2x2 grid above. Same seeded numbers.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots` (single-threaded)"]
fn snapshot_performance_narrow() {
    use crate::renderer::frame_costs as fc;
    render_page_png("performance_narrow", 560, 2000, |ctx, theme, state| {
        fc::set_gpu_timing(true);
        for _ in 0..40 {
            for (id, ms) in [
                ("gpu.celestial", 6.9_f32), ("gpu.shadow", 1.7), ("gpu.scene", 1.1),
                ("gpu.transparent", 0.4), ("gpu.overlay", 0.2), ("gpu.particles", 0.6),
                ("gpu.gpu_particles", 0.3), ("gpu.lines", 0.1), ("gpu.celestial_lines", 0.05),
            ] {
                fc::record_gpu(id, ms);
            }
            for (id, us) in [
                ("cpu.celestial", 2600_u64), ("cpu.scene", 700), ("cpu.patch_upload", 900),
                ("cpu.grass_upload", 400), ("cpu.godrays", 90),
            ] {
                fc::record_cpu(id, std::time::Duration::from_micros(us));
            }
            fc::begin_frame();
        }
        for (id, bytes) in [
            ("vram.patch_arena", 742_000_000_u64), ("vram.meshes", 180_000_000),
            ("vram.textures", 96_000_000), ("vram.render_targets", 44_000_000),
            ("vram.particles", 12_000_000), ("vram.grass", 8_000_000),
            ("vram.uniforms", 5_000_000), ("vram.driver_reserved", 1_450_000_000),
        ] {
            fc::set_vram(id, bytes);
        }
        fc::set_ram("ram.resident", 2_900_000_000);
        fc::set_ram("ram.committed_extra", 640_000_000);
        fc::set_frame_ms(13.9);
        crate::gui::pages::performance::draw(ctx, theme, state);
    });
}

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

/// The top-right help toggle in its CLOSED state: a "?" sitting in the corner the
/// nav bar reserves for it. The point of the snapshot is the geometry, not the
/// glyph: the button must not be sitting on top of a nav tab.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_help_button_closed() {
    render_page_png("help_button_closed", 1280, 200, |ctx, theme, state| {
        let data = std::path::Path::new("data");
        state.keymaps = crate::gui::pages::keymap::load_keymaps(data);
        state.help_registry = crate::gui::widgets::help_modal::load_help_registry(data);
        state.active_page = crate::gui::GuiPage::Chat;
        state.help_panel_pinned = false;
        crate::gui::pages::escape_menu::draw_nav_bar(ctx, theme, state);
        crate::gui::pages::keymap::draw_help_toggle(ctx, theme, state);
    });
}

/// The same button PINNED open: it must read "X" in the identical spot, with the
/// panel below showing this screen's keys plus the prose topics paired with the
/// page in data/help/topics.json (Chat has three).
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_help_panel_open() {
    render_page_png("help_panel_open", 1280, 900, |ctx, theme, state| {
        let data = std::path::Path::new("data");
        state.keymaps = crate::gui::pages::keymap::load_keymaps(data);
        state.help_registry = crate::gui::widgets::help_modal::load_help_registry(data);
        state.active_page = crate::gui::GuiPage::Chat;
        state.help_panel_pinned = true;
        crate::gui::pages::escape_menu::draw_nav_bar(ctx, theme, state);
        crate::gui::pages::keymap::draw_help_toggle(ctx, theme, state);
        crate::gui::pages::keymap::draw(ctx, theme, state);
    });
}

/// The F1-held glance in the first-person world, unchanged since v0.465: centred,
/// World keys, and crucially NO top-right toggle button. The FPS view is the one
/// screen the button stays out of, and this is the proof it does.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_help_f1_world() {
    render_page_png("help_f1_world", 1280, 900, |ctx, theme, state| {
        let data = std::path::Path::new("data");
        state.keymaps = crate::gui::pages::keymap::load_keymaps(data);
        state.help_registry = crate::gui::widgets::help_modal::load_help_registry(data);
        // active_page None with nothing else active IS the world context.
        state.active_page = crate::gui::GuiPage::None;
        state.help_panel_pinned = false;
        crate::gui::pages::keymap::draw_help_toggle(ctx, theme, state);
        crate::gui::pages::keymap::draw(ctx, theme, state);
    });
}

/// F1 held on a page that HAS help topics. This is the regression guard for the bug
/// that shipped in the first cut of the help panel: the prose was drawn into the body
/// shared by both modes, so holding F1 on /chat (3 topics, 14 paragraphs) grew the
/// overlay to 1213 px inside a 900 px window. The F1 overlay is centred, does not
/// scroll and is deliberately non-interactable, so everything past the edge was
/// unreachable, including the first seven KEY ROWS clipped off the top.
///
/// What this must show: the key grid, whole and on screen, and NO prose. Prose belongs
/// to the pinned panel, which can scroll. If someone later moves the topic loop back
/// out of its `if pinned` guard, this snapshot is how they find out.
#[test]
#[ignore = "GPU snapshot; run via `just snapshots`"]
fn snapshot_help_f1_prose_page() {
    render_page_png("help_f1_prose_page", 1280, 900, |ctx, theme, state| {
        let data = std::path::Path::new("data");
        state.keymaps = crate::gui::pages::keymap::load_keymaps(data);
        state.help_registry = crate::gui::widgets::help_modal::load_help_registry(data);
        // Chat is the densest help page we ship, so it is the worst case for overflow.
        state.active_page = crate::gui::GuiPage::Chat;
        state.help_panel_pinned = false;
        crate::gui::pages::keymap::draw(ctx, theme, state);
    });
}


/// REAL interaction test on the F11 time scrubber (v0.1224). Clicking the
/// "Noon" stop must publish a clock request, and it must publish the GLOBAL
/// hour, not the local one the button is labelled with.
///
/// The conversion is the whole point of the test. The engine clock is lon-0
/// solar time; the panel (like the HUD) speaks LOCAL solar time. Getting this
/// backwards is not hypothetical - it is the sun-clock incident of 2026-08-18,
/// where the HUD printed 20:04 beside a noon sun because the two were exactly
/// lon/15 = 8.2 h apart. Here the site is 8 h east of lon 0, so asking for
/// local noon must set the global clock to 04:00.
#[test]
fn weather_panel_time_stop_publishes_converted_global_hour() {
    let ctx = snapshot_ctx();
    let theme = load_theme();
    theme.apply_to_egui(&ctx);
    let mut state = demo_state();
    state.show_weather_panel = true;
    state.game_time = Some(crate::gui::GuiGameTime {
        hour: 0.0,
        day_count: 0,
        season: "Spring".to_string(),
        is_daytime: false,
        local_hour: Some(8.0),
        hours_per_day: 24,
    });
    let screen = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 1400.0));
    let run = |events: Vec<egui::Event>, state: &mut GuiState| {
        let input = egui::RawInput {
            screen_rect: Some(screen),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            crate::gui::pages::weather_panel::draw(ctx, &theme, state);
        });
    };

    // Two settle frames so the window lays out and records its stop rects.
    run(Vec::new(), &mut state);
    run(Vec::new(), &mut state);
    let rect = crate::gui::pages::weather_panel::test_stop_rect("Noon")
        .expect("the Noon time stop should be laid out and hit-testable");
    let at = rect.center();

    state.time_hour_request = None;
    let m = egui::Modifiers::default();
    run(vec![egui::Event::PointerMoved(at)], &mut state);
    run(
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: m,
        }],
        &mut state,
    );
    run(
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: m,
        }],
        &mut state,
    );

    let got = state
        .time_hour_request
        .expect("clicking a time stop must publish a clock request");
    assert!(
        (got - 4.0).abs() < 1.0e-3,
        "local noon at a site 8 h east of lon 0 is global 04:00, got {got}"
    );
    assert!(
        (state.time_pick_hour - 12.0).abs() < 1.0e-3,
        "the slider should read the LOCAL hour the operator asked for, got {}",
        state.time_pick_hour
    );
}

/// The clock-speed readout must describe a day in units a person can picture.
#[test]
fn clock_speed_readout_describes_a_real_day_length() {
    // A 24-hour day of 3,600 s hours (the one clock, 2026-09-27).
    assert_eq!(crate::gui::pages::weather_panel::fmt_day_length(24, 1.0), "24 hours");
    assert_eq!(crate::gui::pages::weather_panel::fmt_day_length(24, 72.0), "20 minutes");
}
