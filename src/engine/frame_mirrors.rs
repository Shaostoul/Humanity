//! The GUI's mirrors of the player (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: the inventory, skills and quests bridged from the player
//! entity for the pages. (The Respawn button's settle moved to frame_settles.rs, the end of the
//! simulation's step, in the headless sim runner's increment 4.)

use crate::engine::frame_prelude::*;

/// Mirrors the player's state into the GUI.
pub(crate) fn run(state: &mut EngineState) {
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
    // The player's skills (live levels and XP) for the Skills section and the Crafting page: built
    // only on a frame that draws one of them, and then only when the skills, the skill data or the
    // language moved (engine/skills_mirror.rs, 2026-11-21; it was every skill on every frame).
    crate::engine::skills_mirror::run(state);
    // Practice quality (skills rung 2): the level, the teachers near, a guide read, the words.
    crate::engine::practice_view::run(state);
    humanity_core::frame_costs::span("cpu.span.mirror_quests");
    // Bridge player quests (active steps + completed) from ECS for
    // the profile Quests panel.
    {
        let quest_reg =
            state.data_store.get::<QuestRegistry>("quest_registry");
        state.gui_state.quests.clear();
        let now_s = humanity_systems::time::elapsed_now(&state.data_store);
        for (_e, (tracker, _ctrl, vitals, intake)) in state
            .game_world
            .world
            .query::<(&QuestTracker, &Controllable, Option<&humanity_core::ecs::components::Vitals>, Option<&humanity_systems::intake::IntakeLedger>)>()
            .iter()
        {
            // What a Keep step reads (E2): the body and its intake ledger, on the game clock.
            let body = humanity_systems::quests::readings::Body { vitals, intake, now_s };
            // The step as the player reads it (the own front door named, 2026-10-04),
            // the chain and the rewards: gui/quest_types.rs.
            humanity_systems::quests::chain::with_store_words(&state.data_store, |w| {
                // The HUD's quest (P2): the tracked one, else the most recently accepted.
                let hud = tracker.hud_quest().map(|a| a.quest_id.as_str());
                for active in &tracker.active_quests {
                    let mut q = crate::gui::GuiQuest::active(quest_reg, active, w);
                    q.tracked = hud == Some(active.quest_id.as_str());
                    q.keep_line = quest_reg
                        .and_then(|r| r.get(&active.quest_id))
                        .and_then(|d| d.steps.get(active.current_step))
                        .and_then(|s| crate::gui::keep_line(&s.objective, active.current_step, &active.progress, &body));
                    state.gui_state.quests.push(q);
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
            // content the chaining alone never reached: quests whose
            // `after` is met (none, or finished) that aren't active
            // or done. Sorted for a stable list.
            state.gui_state.quests_available.clear();
            if let Some(reg) = quest_reg {
                // Another start's opening is never offered (increment 6: each start
                // has its own, and none has an `after`).
                let my_start = humanity_systems::persistence::live_save::active_home().start;
                let openings = humanity_ship::starts::opening_quests(&state.data_dir);
                let mut avail: Vec<crate::gui::GuiAvailableQuest> = reg
                    .quests
                    .values()
                    .filter(|d| !openings.iter().any(|(start, quest)| *quest == d.id && *start != my_start))
                    // Never one whose home lacks what its steps use (`QuestDef::starts`).
                    .filter(|d| d.offered_to(&my_start))
                    .filter(|d| {
                        !tracker.is_active(&d.id)
                            && !tracker.is_completed(&d.id)
                            && d.after.met(|p| tracker.is_completed(p))
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
    }
}
