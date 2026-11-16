//! The GUI's mirrors of the player (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: death and recovery, the inventory, skills and quests
//! bridged from the player entity for the pages.

use crate::engine::frame_prelude::*;

/// Mirrors the player's state into the GUI.
pub(crate) fn run(state: &mut EngineState) {
    humanity_core::frame_costs::span("cpu.span.mirror_player");
    // ── Death & recovery (v0.745, loop-map rung 1) ──
    // A death the systems record is surfaced right after their tick, with
    // what it cost in the Death mode chosen (engine/death_pack.rs after_tick).
    // The Respawn button: back to the spawn room (the fibonacci
    // design's respawner/medbay), full health, survivable vitals,
    // effects cleared, Dead removed. What the death cost was settled
    // when it happened (Realistic leaves the backpack in a pack).
    if state.gui_state.pending_respawn {
        state.gui_state.pending_respawn = false;
        state.gui_state.player_death_cause = None;
        let mut player: Option<hecs::Entity> = None;
        for (e, _c) in state.game_world.world.query::<&Controllable>().iter() {
            player = Some(e);
            break;
        }
        if let Some(e) = player {
            if let Ok(mut h) = state.game_world.world.get::<&mut Health>(e) {
                h.current = h.max;
            }
            if let Ok(mut v) = state
                .game_world
                .world
                .get::<&mut humanity_core::ecs::components::Vitals>(e)
            {
                v.satiation = v.satiation.max(60.0);
                v.hydration = v.hydration.max(60.0);
                v.energy = v.energy.max(60.0);
                v.oxygen = v.oxygen_max;
                v.body_temp_c = 37.0;
            }
            if let Ok(mut fx) = state
                .game_world
                .world
                .get::<&mut humanity_core::ecs::components::StatusEffects>(e)
            {
                fx.active.clear();
            }
            let _ = state
                .game_world
                .world
                .remove_one::<humanity_core::ecs::components::Dead>(e);
            if let Ok(mut t) = state.game_world.world.get::<&mut Transform>(e) {
                t.position = state.fps_spawn;
            }
        }
        state.camera.position = state.fps_spawn;
        // On the floor under the spawn, whatever storey the player died on (ship
        // homes increment 7): the door is on the ground floor, under a slab.
        crate::engine::home_plot::reseat_floor(state);
        // Out of the truck (its seat freed) and off the follow cam, either
        // of which would put the camera back on its vehicle next frame.
        crate::engine::home_plot::step_out_of_vehicles(
            &mut state.game_world.world,
            &mut state.driving_vehicle,
            &mut state.follow_vehicle,
        );
        log::info!("[Vitals] player respawned at the spawn room");
        // In the shared world the relay still holds them where they died, up
        // to 154 m away: step out and join again, so it stands them at their
        // door too (ship homes 1b, engine/home_plot.rs).
        crate::engine::home_plot::respawn_through_relay(state);
    }
    humanity_core::frame_costs::span("cpu.span.mirror_inventory");
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
                    wear: stack.wear,
                    quality: stack.quality,
                    age_s: stack.age_s,
                }
            })
        }).collect();
    }
    humanity_core::frame_costs::span("cpu.span.mirror_vitals");
    // Bridge player vitals + active status effects from ECS for the HUD.
    {
        let fx_registry = state.data_store.get::<
            humanity_systems::status_effects::StatusEffectRegistry,
        >("status_effect_registry");
        for (_entity, (vitals, effects, _ctrl)) in state
            .game_world
            .world
            .query::<(
                &humanity_core::ecs::components::Vitals,
                &humanity_core::ecs::components::StatusEffects,
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
            // What the air feels like beside it: the body
            // heat model's operative temperature (the sun, the
            // clear night sky), 2026-09-28.
            let (sealed, breathing, air_c, feels_c) = state
                .data_store
                .get::<humanity_core::ecs::components::EnvironmentContext>(
                    "environment_context",
                )
                .map(|e| {
                    let ex = humanity_systems::body_heat::Exposure::from_context(e);
                    (e.sealed, e.oxygenated, e.ambient_temp_c, humanity_systems::body_heat::operative_c(&ex, e.activity_met))
                })
                .unwrap_or((true, true, 20.0, 20.0));
            state.gui_state.vitals.sealed = sealed;
            state.gui_state.vitals.breathing = breathing;
            state.gui_state.vitals.air_c = air_c;
            state.gui_state.vitals.feels_c = feels_c;
            state.gui_state.vitals.effects = effects
                .active
                .iter()
                .map(|e| {
                    let name = fx_registry
                        .and_then(|r| r.get(&e.id))
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| e.id.clone());
                    (name, e.remaining as f32)
                })
                .collect();
            // Each wound and illness, with what helps (first-hour audit S8,
            // systems::wounds): the HUD's rows and the Inventory's lines.
            state.gui_state.vitals.care = humanity_systems::wounds::care_lines(effects, fx_registry);
        }
    }
    humanity_core::frame_costs::span("cpu.span.mirror_skills");
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
                // Show EVERY defined skill with the player's progress (0 if untrained): a complete,
                // real skill sheet, each with its named stage, its "I can" list and the progress to
                // the next stage (skills rung 1), in family order, then by name.
                for (id, def) in reg.skills.iter() {
                    let prog = skills.skills.get(id);
                    let level = prog.map(|p| p.level).unwrap_or(0);
                    let xp = prog.map(|p| p.xp).unwrap_or(0);
                    state.gui_state.skills.push(crate::gui::GuiSkill::from_def(def, reg, level, xp));
                }
                state.gui_state.skills.sort_by(|a, b| {
                    a.family_order.cmp(&b.family_order).then(a.name.cmp(&b.name))
                });
                break;
            }
        }
    }
    // Practice quality (skills rung 2): the level, the teachers near, a guide read, the words.
    crate::engine::practice_view::run(state);
    humanity_core::frame_costs::span("cpu.span.mirror_quests");
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
            // The step as the player reads it (the own front door named, 2026-10-04),
            // the chain and the rewards: gui/quest_types.rs.
            humanity_systems::quests::chain::with_store_words(&state.data_store, |w| {
                for active in &tracker.active_quests {
                    state.gui_state.quests.push(crate::gui::GuiQuest::active(quest_reg, active, w));
                }
            });
            for cid in &tracker.completed_quests {
                state.gui_state.quests.push(crate::gui::GuiQuest::completed(quest_reg, cid));
            }
            // The chain window (2026-10-30), built only while it is open on the Quests
            // page (the main screen's, or a wall screen's framed last tick): every
            // step in Dev, only what the player has reached otherwise.
            if !crate::engine::screens::page_drawn(state.gui_state.active_page, &state.screens.framed_pages, GuiPage::Quests) {
                state.gui_state.quest_chain_open = None;
            }
            let reveal = if state.gui_state.dev_cheats_active(&state.theme) { humanity_systems::quests::chain::Reveal::All } else { humanity_systems::quests::chain::Reveal::Learned };
            state.gui_state.quest_chain = state.gui_state.quest_chain_open.as_deref().zip(quest_reg).and_then(|(id, reg)| {
                humanity_systems::quests::chain::with_store_words(&state.data_store, |w| humanity_systems::quests::chain::build(reg, tracker, id, reveal, w))
            });
            // Available quests (v0.748, ladder rung 4): accept-able
            // content the chaining alone never reached — quests with
            // no prerequisite (or a completed one) that aren't active
            // or done. Sorted for a stable list.
            state.gui_state.quests_available.clear();
            if let Some(reg) = quest_reg {
                // Another start's opening is never offered (increment 6: each start
                // has its own, and none has a prerequisite).
                let my_start = humanity_systems::persistence::live_save::active_home().start;
                let openings = humanity_ship::starts::opening_quests(&state.data_dir);
                let mut avail: Vec<crate::gui::GuiAvailableQuest> = reg
                    .quests
                    .values()
                    .filter(|d| !openings.iter().any(|(start, quest)| *quest == d.id && *start != my_start))
                    .filter(|d| {
                        !tracker.is_active(&d.id)
                            && !tracker.is_completed(&d.id)
                            && d.prerequisite
                                .as_deref()
                                .map_or(true, |p| tracker.is_completed(p))
                    })
                    .map(|d| crate::gui::GuiAvailableQuest {
                        id: d.id.clone(),
                        name: d.name.clone(),
                        description: d.description.clone(),
                    })
                    .collect();
                avail.sort_by(|a, b| a.name.cmp(&b.name));
                state.gui_state.quests_available = avail;
            }
            break;
        }
        // Accept clicked: apply to the live tracker.
        if let Some(qid) = state.gui_state.pending_accept_quest.take() {
            for (_e, (tracker, _c)) in state
                .game_world
                .world
                .query_mut::<(&mut QuestTracker, &Controllable)>()
            {
                tracker.accept_quest(&qid);
                break;
            }
        }
    }
}
