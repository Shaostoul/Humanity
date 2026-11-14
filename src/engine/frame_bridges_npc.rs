//! The GUI's requests into the world, second half (the file-size split, step 1, 2026-11-14), moved
//! out of `window_event` in src/lib.rs as it was: the walk-up creature editor, the NPC dialogue
//! card's upkeep, the game clock's settings, backpack transfers and the survival Rest button.

use crate::engine::frame_prelude::*;

/// Carries this frame's GUI requests (creature edits, dialogue, transfers, rest) into the world.
pub(crate) fn run(state: &mut EngineState) {
    // Walk-up creature editor (v0.778): snapshot on open, write the
    // edit buffers back to the entity live, and despawn on request.
    if let Some(bits) = state.gui_state.dev_edit_target {
        match hecs::Entity::from_bits(bits) {
            Some(ent) if state.game_world.world.contains(ent) => {
                use humanity_core::ecs::components::{AIBehavior, Creature, Dead, Health, Name};
                let w = &mut state.game_world.world;
                if state.gui_state.dev_edit_snapshot_pending {
                    // Fill the buffers from the creature's current state (each
                    // get is its own statement so the borrow releases between).
                    state.gui_state.dev_edit_snapshot_pending = false;
                    state.gui_state.dev_edit_name =
                        w.get::<&Name>(ent).map(|n| n.0.clone()).unwrap_or_default();
                    if let Ok(h) = w.get::<&Health>(ent) {
                        state.gui_state.dev_edit_health = h.current;
                        state.gui_state.dev_edit_health_max = h.max;
                    }
                    if let Ok(c) = w.get::<&Creature>(ent) {
                        state.gui_state.dev_edit_tint = c.tint;
                        state.gui_state.dev_edit_scale = c.body_side;
                        state.gui_state.dev_edit_species = c.def_id.clone();
                    }
                    // Remember the ORIGINAL behavior ("" = no AIBehavior) so
                    // the write-back below can leave the AI alone unless the
                    // user actually flips the Hostile toggle. (v0.779)
                    state.gui_state.dev_edit_behavior_orig = w
                        .get::<&AIBehavior>(ent)
                        .map(|a| a.behavior_type.clone())
                        .unwrap_or_default();
                    let orig = &state.gui_state.dev_edit_behavior_orig;
                    state.gui_state.dev_edit_hostile =
                        !orig.is_empty() && orig != "passive";
                } else {
                    // Write the buffers back live (compare-before-write: these
                    // run at 60fps while the panel is open).
                    if let Ok(mut n) = w.get::<&mut Name>(ent) {
                        if n.0 != state.gui_state.dev_edit_name {
                            n.0 = state.gui_state.dev_edit_name.clone();
                        }
                    }
                    // Health 0 = KILL (v0.779): dragging the slider to zero
                    // routes through the same Dead marker the combat pipeline
                    // uses, instead of leaving an alive-at-zero creature that
                    // still wanders, harvests, and drops no loot.
                    if state.gui_state.dev_edit_health <= 0.0 {
                        if let Ok(mut h) = w.get::<&mut Health>(ent) {
                            h.current = 0.0;
                        }
                        let _ = w.insert_one(ent, Dead::default());
                        state.gui_state.close_in_world_modals();
                        log::info!("Dev: edited creature killed via health slider");
                    } else {
                        let hmax = state.gui_state.dev_edit_health_max.max(1.0);
                        if let Ok(mut h) = w.get::<&mut Health>(ent) {
                            if h.max != hmax || h.current != state.gui_state.dev_edit_health.min(hmax) {
                                h.max = hmax;
                                h.current = state.gui_state.dev_edit_health.clamp(0.0, hmax);
                            }
                        }
                        if let Ok(mut c) = w.get::<&mut Creature>(ent) {
                            let side = state.gui_state.dev_edit_scale.clamp(0.05, 5.0);
                            if c.tint != state.gui_state.dev_edit_tint || c.body_side != side {
                                c.tint = state.gui_state.dev_edit_tint;
                                c.body_side = side;
                            }
                        }
                        // AI write-back is TOGGLE-DRIVEN (v0.779). Untouched
                        // toggle = untouched AI: no more collapsing a wolf's
                        // "predator" (or a "guard") to plain "aggressive" just
                        // by opening the editor, and no force-attaching
                        // AIBehavior to placed farm animals (which knocked
                        // them out of anchored grazing). Hostile ON maps to
                        // "predator" -- the one behavior that actually hunts
                        // the PLAYER ("aggressive" targets by faction and no
                        // creature has a Faction, so it never attacked anyone).
                        let orig = state.gui_state.dev_edit_behavior_orig.clone();
                        let orig_hostile = !orig.is_empty() && orig != "passive";
                        if state.gui_state.dev_edit_hostile == orig_hostile {
                            // Back at (or still at) the original setting:
                            // restore the exact original state.
                            if orig.is_empty() {
                                // Originally no AI: remove one we may have added
                                // while the user experimented with the toggle.
                                if w.get::<&AIBehavior>(ent).is_ok() {
                                    let _ = w.remove_one::<AIBehavior>(ent);
                                }
                            } else if let Ok(mut a) = w.get::<&mut AIBehavior>(ent) {
                                if a.behavior_type != orig {
                                    a.behavior_type = orig.clone();
                                }
                            }
                        } else {
                            let want = if state.gui_state.dev_edit_hostile {
                                "predator"
                            } else {
                                "passive"
                            };
                            let has_ai = {
                                let r = w.get::<&mut AIBehavior>(ent).map(|mut a| {
                                    if a.behavior_type != want {
                                        a.behavior_type = want.to_string();
                                    }
                                });
                                r.is_ok()
                            };
                            if !has_ai {
                                let _ = w.insert_one(
                                    ent,
                                    AIBehavior {
                                        behavior_type: want.to_string(),
                                        state: "idle".to_string(),
                                        target: None,
                                    },
                                );
                            }
                        }
                    }
                }
            }
            _ => {
                // Entity gone (despawned elsewhere) - close the editor.
                state.gui_state.dev_edit_target = None;
                state.gui_state.dev_edit_snapshot_pending = false;
            }
        }
    }
    if std::mem::take(&mut state.gui_state.pending_dev_edit_despawn) {
        if let Some(bits) = state.gui_state.dev_edit_target.take() {
            if let Some(ent) = hecs::Entity::from_bits(bits) {
                let _ = state.game_world.world.despawn(ent);
                log::info!("Dev: despawned edited creature {ent:?}");
            }
        }
        state.gui_state.dev_edit_snapshot_pending = false;
    }
    // NPC dialogue card upkeep (v0.797). The relay keeps
    // simulating chores while the card is open (freezing the NPC
    // server-side is deliberately out of scope), so: if the crew
    // member wanders beyond conversational range (4 m -- wider
    // than the 2.5 m open range so a small shuffle doesn't slam
    // the card) or despawns, the card closes itself. While it
    // stays open, the activity line refreshes live so the card
    // tracks their current chore.
    if let Some(npc_id) = state.gui_state.npc_talk_target.filter(|id| *id != crate::gui::talk::DEV_TALK_TARGET) {
        let cam = state.camera.position;
        let mut still_near = false;
        for (_e, (tf, npc)) in state
            .game_world
            .world
            .query::<(
                &humanity_core::ecs::components::Transform,
                &crate::net::sync::RemoteNpc,
            )>()
            .iter()
        {
            if npc.entity_id != npc_id {
                continue;
            }
            if (tf.position - cam).length() <= 4.0 {
                still_near = true;
                state.gui_state.npc_talk_activity = npc.activity.clone();
            }
            break;
        }
        if !still_near {
            state.gui_state.npc_talk_target = None;
        }
    }
    // Vehicles: bridge the Vehicles section's Summon action (Stage 3).
    if let Some(bits) = state.gui_state.pending_summon_vehicle.take() {
        if let Some(slot) = state
            .data_store
            .get::<std::sync::Mutex<Option<u64>>>("summon_vehicle")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(bits);
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
    // Plant a bed/tray/field (v0.738): (machine id, plant id, count).
    if let Some(bed_planting) = state.gui_state.pending_plant_bed.take() {
        if let Some(slot) = state
            .data_store
            .get::<std::sync::Mutex<Option<(String, String, u32)>>>("plant_bed_request")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(bed_planting);
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
    // Bulk harvest (v0.739): the whole ready set in one frame.
    if !state.gui_state.pending_harvest_many.is_empty() {
        let bits = std::mem::take(&mut state.gui_state.pending_harvest_many);
        if let Some(slot) = state
            .data_store
            .get::<std::sync::Mutex<Vec<u64>>>("harvest_many_request")
        {
            if let Ok(mut s) = slot.lock() {
                s.extend(bits);
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
    // PLAY-MODE ENFORCEMENT (task #50): Normal is survival,
    // period -- force the flag off each frame so no stale
    // toggle, old config, or future code path can leave free
    // resources on for a Normal player. Creative/Dev keep the
    // Inventory page's Creative toggle as a live fine-tune
    // (testing real consumption while in Dev is legitimate).
    if !state
        .gui_state
        .settings
        .play_mode
        .allows(crate::config::Capability::FreeResources)
    {
        state.gui_state.creative_mode = false;
    }
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
    // Settings > Gameplay's garden and ship levels (garden pests, soil pH, pollination,
    // picking, ship life support; data/gameplay/levels.json, 2026-11-12) and the
    // Garden panel's amendment, hand pollination and Clear -> sim.
    crate::engine::garden_levels::GardenLevels::of(&state.gui_state.settings.gameplay).publish(&state.data_store, &mut state.gui_state.garden_pests);
    // Garden updates (Settings > Gameplay, 2026-10-31, farming::clock): how often the farm advances.
    humanity_systems::farming::clock::publish(&state.data_store, f64::from(state.gui_state.settings.garden_update_s));
    humanity_systems::farming::humidity::publish_room_bounds(&state.data_store, &state.gui_state.room_bounds); // grow rooms' air
    let gp = &mut state.gui_state.garden_pests;
    humanity_systems::farming::weeds::publish(&state.data_store, gp.weed_pending.take()); // Hoe / Mulch -> sim
    if let Some(req) = state.gui_state.garden_pests.pending.take() {
        if let Some(m) = state.data_store.get::<std::sync::Mutex<Option<(String, String)>>>("pest_control_request") {
            if let Ok(mut s) = m.lock() {
                *s = Some(req);
            }
        }
    }
    // The one game clock's settings -> the TimeSystem (2026-09-27):
    // hours in a day, days in a year, the time speed. Clamped inside.
    let s = &state.gui_state.settings;
    humanity_systems::time::publish_settings(&state.data_store, s.hours_per_day, s.days_per_year, s.time_speed);
    // Backpack <-> container transfers: drain the GUI's pending ops into
    // the InventorySystem channel (it applies them to the player backpack).
    if !state.gui_state.pending_inventory_transfers.is_empty() {
        let ops = std::mem::take(&mut state.gui_state.pending_inventory_transfers);
        let origins = std::mem::take(&mut state.gui_state.pending_take_origins);
        state.gui_state.inflight_take_origins.extend(origins);
        if let Some(slot) = state
            .data_store
            .get::<std::sync::Mutex<Vec<humanity_systems::inventory::TransferOp>>>("inventory_transfer_ops")
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
    // manifest) to DroneSystem. With "Keep mining" checked, the same
    // order also becomes the STANDING order the drone system re-fires
    // at tick level (economy automation, v0.663). Unchecking clears
    // the standing order; RE-checking re-arms it from the last
    // launched order (rising edge), so toggling mid-flight behaves
    // exactly as the checkbox reads (review fix).
    if let Some(order) = state.gui_state.pending_drone_manifest.take() {
        state.gui_state.last_drone_order = Some(order.clone());
        if state.gui_state.auto_mine_enabled {
            if let Some(slot) = state.data_store.get::<std::sync::Mutex<
                Option<(String, Vec<(String, u32)>)>,
            >>("auto_mine_order")
            {
                if let Ok(mut s) = slot.lock() {
                    *s = Some(order.clone());
                }
            }
        }
        if let Some(slot) = state.data_store.get::<std::sync::Mutex<
            Option<(String, Vec<(String, u32)>)>,
        >>("commission_drone")
        {
            if let Ok(mut s) = slot.lock() {
                *s = Some(order);
            }
        }
    }
    if !state.gui_state.auto_mine_enabled {
        if let Some(slot) = state.data_store.get::<std::sync::Mutex<
            Option<(String, Vec<(String, u32)>)>,
        >>("auto_mine_order")
        {
            if let Ok(mut s) = slot.lock() {
                if s.is_some() {
                    *s = None;
                }
            }
        }
    } else if !state.gui_state.prev_auto_mine_enabled {
        // Rising edge: re-arm from the last launched order (if any).
        if let Some(order) = state.gui_state.last_drone_order.clone() {
            if let Some(slot) = state.data_store.get::<std::sync::Mutex<
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
    state.gui_state.prev_auto_mine_enabled = state.gui_state.auto_mine_enabled;
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

}
