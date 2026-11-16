//! The GUI's requests into the world, second half (the file-size split, step 1, 2026-11-14), moved
//! out of `window_event` in src/lib.rs as it was: the walk-up creature editor and the NPC dialogue
//! card's upkeep. Its requests (the game clock's settings, backpack transfers, the Rest button and
//! the rest) moved to engine/frame_sim_requests.rs on 2026-11-15 (the headless sim runner's
//! increment 3), run by the bridges' slice before this; frame_bridges.rs has the order audit.

use crate::engine::frame_prelude::*;

/// Carries this frame's walk-up edits and dialogue upkeep into the world.
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
}
