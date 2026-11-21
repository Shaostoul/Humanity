//! The GUI's requests carried into the systems' channels (the headless sim runner's increment 3,
//! 2026-11-15; docs/design/headless-sim-runner.md). Every button that asks the world for something
//! sets a `GuiState::pending_*` field; once a frame these hand each request to the DataStore slot its
//! system drains in its next tick (eat, drink, use, plant, harvest, craft, deploy, summon, rest,
//! compost, the backpack's transfers, the drone's orders), and mirror the settings the systems read
//! every frame (creative, irrigation, the garden's levels, the farm's clock, the game clock).
//!
//! Moved verbatim from frame_bridges.rs (craft to deploy) and frame_bridges_npc.rs (summon to the dev
//! seed set), which had moved them out of lib.rs `window_event` on 2026-11-14; `state.gui_state` is
//! `gui` and `state.data_store` is `data`. `frame_sim::bridges_slice` runs them, the one step the
//! windowed frame (frame_bridges.rs) and the soak (sim_home.rs) both run, so a soak's person eats and
//! drinks through the same requests a player's clicks make.
//!
//! No GPU, window, painter or `EngineState` in here (tests/sim_gpu_free_lint.rs).

use humanity_core::hot_reload::data_store::DataStore;

use crate::gui::GuiState;

/// Carries this step's GUI requests into the systems' channels, in the order the frame ran them.
pub(crate) fn carry(gui: &mut GuiState, data: &mut DataStore, world: &mut hecs::World) {
    // Bridge GUI craft/dev commands into the DataStore so the ECS
    // CraftingSystem (which only gets &DataStore in tick) acts on them
    // this frame. Take/clear the GuiState flags.
    if let Some(recipe_id) = gui.pending_craft_recipe.take() {
        if let Some(slot) = data
            .get::<std::sync::Mutex<Option<String>>>("craft_request")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(recipe_id);
            }
        }
    }
    if gui.dev_stock_materials {
        gui.dev_stock_materials = false;
        if let Some(slot) = data
            .get::<std::sync::Mutex<bool>>("dev_stock_materials")
        {
            if let Ok(mut s) = slot.lock() {
                *s = true;
            }
        }
    }
    // Eat: bridge the clicked food item to FoodSystem's consume channel.
    if let Some(item_id) = gui.pending_consume_item.take() {
        if let Some(slot) = data
            .get::<std::sync::Mutex<Option<String>>>("consume_request")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(item_id);
            }
        }
    }
    // Drink: bridge the clicked beverage to FoodSystem's drink channel.
    if let Some(item_id) = gui.pending_drink_item.take() {
        if let Some(slot) = data
            .get::<std::sync::Mutex<Option<String>>>("drink_request")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(item_id);
            }
        }
    }
    // Use: the clicked medical item to FoodSystem's use channel (BUG-162).
    if let Some(item_id) = gui.pending_use_item.take() {
        humanity_systems::treatment::request_use(data, item_id);
    }
    // Gardening: bridge plant/water/harvest/dev-grow to FarmingSystem.
    if let Some(seed_id) = gui.pending_plant_seed.take() {
        if let Some(slot) = data
            .get::<std::sync::Mutex<Option<String>>>("plant_request")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(seed_id);
            }
        }
    }
    // Vehicles: bridge the item card's Deploy action to VehicleSystem.
    if let Some(kit_id) = gui.pending_deploy_kit.take() {
        if let Some(slot) = data
            .get::<std::sync::Mutex<Option<String>>>("deploy_kit_request")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(kit_id);
            }
        }
    }
    // Vehicles: bridge the Vehicles section's Summon action (Stage 3).
    if let Some(bits) = gui.pending_summon_vehicle.take() {
        if let Some(slot) = data
            .get::<std::sync::Mutex<Option<u64>>>("summon_vehicle")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(bits);
            }
        }
    }
    // Plant a whole tower (v0.386): the GUI sends (tower id, plant ids).
    if let Some(tower_planting) = gui.pending_plant_tower.take() {
        if let Some(slot) = data
            .get::<std::sync::Mutex<Option<(String, Vec<String>)>>>("plant_tower_request")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(tower_planting);
            }
        }
    }
    // Plant a bed/tray/field (v0.738): (machine id, plant id, count).
    if let Some(bed_planting) = gui.pending_plant_bed.take() {
        if let Some(slot) = data
            .get::<std::sync::Mutex<Option<(String, String, u32)>>>("plant_bed_request")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(bed_planting);
            }
        }
    }
    if let Some(bits) = gui.pending_water_crop.take() {
        if let Some(slot) = data
            .get::<std::sync::Mutex<Option<u64>>>("water_request")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(bits);
            }
        }
    }
    if let Some(bits) = gui.pending_harvest_crop.take() {
        if let Some(slot) = data
            .get::<std::sync::Mutex<Option<u64>>>("harvest_request")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(bits);
            }
        }
    }
    // Bulk harvest (v0.739): the whole ready set in one frame.
    if !gui.pending_harvest_many.is_empty() {
        let bits = std::mem::take(&mut gui.pending_harvest_many);
        if let Some(slot) = data
            .get::<std::sync::Mutex<Vec<u64>>>("harvest_many_request")
        {
            if let Ok(mut s) = slot.lock() {
                s.extend(bits);
            }
        }
    }
    if gui.dev_grow_crops {
        gui.dev_grow_crops = false;
        if let Some(slot) =
            data.get::<std::sync::Mutex<bool>>("dev_grow_crops")
        {
            if let Ok(mut s) = slot.lock() {
                *s = true;
            }
        }
    }
    // Creative/survival mode: mirror the flag EVERY frame (not
    // one-shot) so the farming + crafting systems see the current
    // mode. Creative = skip resource requirements + consumption.
    // PLAY-MODE ENFORCEMENT (task #50): Normal is survival,
    // period -- force the flag off each frame so no stale
    // toggle, old config, or future code path can leave free
    // resources on for a Normal player. Creative/Dev keep the
    // Inventory page's Creative toggle as a live fine-tune
    // (testing real consumption while in Dev is legitimate).
    if !gui
        .settings
        .play_mode
        .allows(crate::config::Capability::FreeResources)
    {
        gui.creative_mode = false;
    }
    if let Some(slot) =
        data.get::<std::sync::Mutex<bool>>("creative_mode")
    {
        if let Ok(mut s) = slot.lock() {
            *s = gui.creative_mode;
        }
    }
    // Per-area irrigation: mirror the garden edit modal's water sliders
    // (GuiState.garden_irrigation, keyed by tower_id) into the sim every
    // frame so FarmingSystem keeps configured crops topped up.
    if let Some(slot) = data
        .get::<std::sync::Mutex<std::collections::HashMap<String, f32>>>(
            "garden_irrigation",
        )
    {
        if let Ok(mut s) = slot.lock() {
            if *s != gui.garden_irrigation {
                *s = gui.garden_irrigation.clone();
            }
        }
    }
    if let Some(slot) = data
        .get::<std::sync::Mutex<std::collections::HashMap<String, f32>>>(
            "garden_nutrient",
        )
    {
        if let Ok(mut s) = slot.lock() {
            if *s != gui.garden_nutrient {
                *s = gui.garden_nutrient.clone();
            }
        }
    }
    // Settings > Gameplay's garden and ship levels (garden pests, soil pH, pollination,
    // picking, ship life support; data/gameplay/levels.json, 2026-11-12) and the
    // Garden panel's amendment, hand pollination and Clear -> sim.
    crate::engine::garden_levels::GardenLevels::of(&gui.settings.gameplay).publish(&data, &mut gui.garden_pests);
    // Garden updates (Settings > Gameplay, 2026-10-31, farming::clock): how often the farm advances.
    humanity_systems::farming::clock::publish(&data, f64::from(gui.settings.garden_update_s));
    humanity_systems::farming::humidity::publish_room_bounds(&data, &gui.room_bounds); // grow rooms' air
    let gp = &mut gui.garden_pests;
    humanity_systems::farming::weeds::publish(&data, gp.weed_pending.take()); // Hoe / Mulch -> sim
    if let Some(req) = gui.garden_pests.pending.take() {
        if let Some(m) = data.get::<std::sync::Mutex<Option<(String, String)>>>("pest_control_request") {
            if let Ok(mut s) = m.lock() {
                *s = Some(req);
            }
        }
    }
    // The one game clock's settings -> the TimeSystem (2026-09-27):
    // hours in a day, days in a year, the time speed. Clamped inside.
    let s = &gui.settings;
    humanity_systems::time::publish_settings(&data, s.hours_per_day, s.days_per_year, s.time_speed);
    // Backpack <-> container transfers: drain the GUI's pending ops into
    // the InventorySystem channel (it applies them to the player backpack).
    if !gui.pending_inventory_transfers.is_empty() {
        let ops = std::mem::take(&mut gui.pending_inventory_transfers);
        let origins = std::mem::take(&mut gui.pending_take_origins);
        gui.inflight_take_origins.extend(origins);
        if let Some(slot) = data
            .get::<std::sync::Mutex<Vec<humanity_systems::inventory::TransferOp>>>("inventory_transfer_ops")
        {
            if let Ok(mut s) = slot.lock() {
                s.extend(ops);
            }
        }
    }
    // Skills: bridge the "Dev: max skills" button to SkillSystem.
    if gui.pending_dev_max_skills {
        gui.pending_dev_max_skills = false;
        if let Some(slot) =
            data.get::<std::sync::Mutex<bool>>("dev_max_skills")
        {
            if let Ok(mut s) = slot.lock() {
                *s = true;
            }
        }
    }
    // Mining: bridge a commissioned drone order (target asteroid id +
    // manifest) to DroneSystem. With "Keep mining" checked, the same
    // order also becomes the STANDING order the drone system re-fires
    // at tick level (economy automation, v0.663). Unchecking clears
    // the standing order; RE-checking re-arms it from the last
    // launched order (rising edge), so toggling mid-flight behaves
    // exactly as the checkbox reads (review fix).
    if let Some(order) = gui.pending_drone_manifest.take() {
        gui.last_drone_order = Some(order.clone());
        if gui.auto_mine_enabled {
            if let Some(slot) = data.get::<std::sync::Mutex<
                Option<(String, Vec<(String, u32)>)>,
            >>("auto_mine_order")
            {
                if let Ok(mut s) = slot.lock() {
                    *s = Some(order.clone());
                }
            }
        }
        if let Some(slot) = data.get::<std::sync::Mutex<
            Option<(String, Vec<(String, u32)>)>,
        >>("commission_drone")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(order);
            }
        }
    }
    if !gui.auto_mine_enabled {
        if let Some(slot) = data.get::<std::sync::Mutex<
            Option<(String, Vec<(String, u32)>)>,
        >>("auto_mine_order")
        {
            if let Ok(mut s) = slot.lock() {
                if s.is_some() {
                    *s = None;
                }
            }
        }
    } else if !gui.prev_auto_mine_enabled {
        // Rising edge: re-arm from the last launched order (if any).
        if let Some(order) = gui.last_drone_order.clone() {
            if let Some(slot) = data.get::<std::sync::Mutex<
                Option<(String, Vec<(String, u32)>)>,
            >>("auto_mine_order")
            {
                if let Ok(mut s) = slot.lock() {
                    if s.is_none() {
                        *s = Some(order);
                    }
                }
            }
        }
    }
    gui.prev_auto_mine_enabled = gui.auto_mine_enabled;
    // Survival: bridge the Rest button to FoodSystem's rest channel.
    if gui.pending_rest {
        gui.pending_rest = false;
        if let Some(slot) =
            data.get::<std::sync::Mutex<bool>>("rest_request")
        {
            if let Ok(mut s) = slot.lock() {
                *s = true;
            }
        }
    }
    // Sanitation: bridge Compost (FoodSystem) + Fertilize (FarmingSystem).
    if gui.pending_compost {
        gui.pending_compost = false;
        if let Some(slot) =
            data.get::<std::sync::Mutex<bool>>("compost_request")
        {
            if let Ok(mut s) = slot.lock() {
                *s = true;
            }
        }
    }
    if let Some(bits) = gui.pending_fertilize_crop.take() {
        if let Some(slot) = data
            .get::<std::sync::Mutex<Option<u64>>>("fertilize_crop_request")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(bits);
            }
        }
    }
    // Dev: stock the starter seed set (one of each tower variety).
    if let Some(seeds) = gui.pending_stock_seeds.take() {
        if let Some(slot) = data
            .get::<std::sync::Mutex<Option<Vec<String>>>>("stock_seeds_request")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(seeds);
            }
        }
    }
    // Fill and pour at a water tank (2026-09-26; crates/humanity-systems/src/fluids.rs): the tank
    // card's button names the tank and the action. Carried out here, before the tick, since the
    // headless sim runner's increment 3 (it ran in frame_cards.rs, after the tick, on the frame
    // after the click; the bridges run on that same frame, before it, so the click lands on the
    // same frame as before), so a soak's routine fills its bottles the way a player does.
    if let Some((tank, key)) = gui.machine_card_fluid_pending.take() {
        if let Some(ft) = data.get::<humanity_systems::fluids::FluidTable>("fluid_table") {
            let items = data.get::<humanity_systems::inventory::ItemRegistry>("item_registry");
            let toast = match humanity_systems::fluids::apply_action(ft, world, items, &tank, &key) {
                Ok(m) => (m, crate::gui::ToastKind::Success),
                Err(m) => (m, crate::gui::ToastKind::Info),
            };
            gui.pending_toasts.push(toast);
        }
    }
    // The quest page's Accept, on the live tracker (moved from frame_mirrors.rs, the view's mirror,
    // in the headless sim runner's increment 4: the mirror applied it after the tick, on the frame
    // after the click; here it lands before the tick of that same frame).
    let accept = gui.pending_accept_quest.take();
    // "Show on HUD" (P2), on the same tracker, after the accept so a quest accepted and
    // tracked in one frame is shown.
    let track = gui.pending_track_quest.take();
    if accept.is_some() || track.is_some() {
        for (_e, (tracker, _c)) in world.query_mut::<(&mut humanity_systems::quests::QuestTracker, &humanity_core::ecs::components::Controllable)>() {
            if let Some(qid) = &accept {
                tracker.accept_quest(qid);
            }
            if let Some(qid) = &track {
                tracker.track(qid);
            }
            break;
        }
    }
}
