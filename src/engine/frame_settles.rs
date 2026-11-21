//! Settling the world's state into the GUI (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: the ECS and data store bridged into `GuiState`, the
//! collect notice, loot, blueprint building, the station gate, buying and selling, abilities, armour
//! from worn gear, equipping, and the Respawn button. The end of the simulation's step (the headless
//! sim runner's increment 4): it runs right after the systems, before any drawing.

use crate::engine::frame_prelude::*;

/// Settles this frame's world results into the GUI and the GUI's intents into the world.
pub(crate) fn run(state: &mut EngineState) {
    humanity_core::frame_costs::span("cpu.span.settles");
    // ── Bridge ECS/DataStore state into GuiState for GUI pages ──

    // Bridge player health and inventory from ECS
    let now_s = state
        .data_store
        .get::<std::sync::Mutex<humanity_systems::time::GameTime>>("game_time")
        .and_then(|m| m.lock().ok().map(|t| t.elapsed_seconds as f64))
        .unwrap_or(0.0);
    for (_entity, (health, _ctrl)) in state.game_world.world.query::<(&Health, &Controllable)>().iter() {
        // Any health DROP pulses the attack indicator (v0.745):
        // the nav separator flashes red so damage is legible
        // even mid-menu; it clears after 3 quiet game-seconds.
        if health.current < state.gui_state.player_health - 0.01 {
            state.gui_state.attack_pulse_active = true;
            state.gui_state.attack_pulse_last_hit_at = now_s;
        }
        state.gui_state.player_health = health.current;
        state.gui_state.player_health_max = health.max;
    }
    if state.gui_state.attack_pulse_active
        && now_s - state.gui_state.attack_pulse_last_hit_at > 3.0
    {
        state.gui_state.attack_pulse_active = false;
    }

    humanity_core::frame_costs::span("cpu.span.settle_livestock");
    // ── Livestock collect bridge (v0.751, ladder rung 7) ──
    // E on a ready animal or forage moves its product into the
    // pack, volume-gated: what does not fit stays at the source,
    // ready, never lost (livestock::collect_into_pack; first-hour
    // audit Friction 7, 2026-10-04: as much as fits is taken).
    if let Some(animal) = state.pending_livestock_harvest.take() {
        let mut notice = String::new();
        let player = state.game_world.world.query::<&Controllable>().iter().next().map(|(e, _)| e);
        if let (true, Some(player)) = (state.game_world.world.contains(animal), player) {
            let animal_name = state
                .game_world
                .world
                .get::<&Name>(animal)
                .map(|n| n.0.clone())
                .unwrap_or_else(|_| "animal".to_string());
            let creature_id = state
                .game_world
                .world
                .get::<&humanity_core::ecs::components::Creature>(animal)
                .map(|c| c.def_id.clone())
                .unwrap_or_default();
            let items = state.data_store.get::<ItemRegistry>("item_registry");
            let outcome = humanity_systems::livestock::collect_into_pack(
                &mut state.game_world.world,
                animal,
                player,
                items,
            );
            if let humanity_systems::livestock::CollectOutcome::Took { fresh, item, .. } = &outcome {
                // Collect SFX (v0.983): eggs, milk, berries,
                // stone - every successful [E] gather clicks.
                state.pending_sfx.push(("sfx.inventory_pickup", "audio/ui/inventory_pickup.ogg"));
                // One yield earns its XP and its quest event once,
                // however many presses it takes to carry it off.
                if *fresh {
                    // Skills rung 1: the yield's own skill (eggs and milk keeping animals,
                    // berries foraging, a log forestry, stone mining: data/skills/practice.json),
                    // where every yield, stone included, used to train farming.
                    humanity_systems::skills::practice(&state.data_store, &format!("collect.{item}"), None);
                    if let Some(events) =
                        state.data_store.get::<std::sync::Mutex<Vec<String>>>("quest_events")
                    {
                        if let Ok(mut ev) = events.lock() {
                            ev.push(format!("harvest_{creature_id}"));
                        }
                    }
                }
            }
            notice = humanity_systems::livestock::collect_notice(&outcome, &animal_name, items);
        }
        state.gui_state.livestock_notice = notice;
        state.gui_state.livestock_notice_at = now_s;
    }
    // The collect notice fades after 3 game-seconds (same clock
    // as the attack pulse above).
    if !state.gui_state.livestock_notice.is_empty()
        && now_s - state.gui_state.livestock_notice_at > 3.0
    {
        state.gui_state.livestock_notice.clear();
    }

    humanity_core::frame_costs::span("cpu.span.settle_melee");
    // ── Melee swing settle (v0.765) ── F swings the worn
    // hands-slot weapon (its equipment.csv damage + reach) or
    // bare hands at the faced creature; the hit rides the one
    // damage pipeline, so armor/death/loot/kill-XP all apply.
    if std::mem::take(&mut state.gui_state.pending_swing) {
        if now_s >= state.gui_state.swing_ready_at {
            state.gui_state.swing_ready_at = now_s + 0.8;
            // What is in the hands? A tool row wins; bare
            // hands are the honest fallback.
            let hands_item: Option<String> = state
                .game_world
                .world
                .query::<(&humanity_core::ecs::components::Outfit, &Controllable)>()
                .iter()
                .next()
                // The main hand's grip socket (character animation rung 2b: what is held goes in
                // the two grip sockets; gloves stay worn in "hands").
                .and_then(|(_, (o, _))| o.equipped.get("grip_r").cloned());
            let (weapon_name, skill, damage, range) = hands_item
                .as_deref()
                .and_then(|id| {
                    let def = state
                        .data_store
                        .get::<humanity_systems::economy::EquipmentRegistry>(
                            "equipment_registry",
                        )?
                        .get(id)?;
                    if def.damage <= 0.0 {
                        return None; // gloves are not weapons
                    }
                    let name = state
                        .data_store
                        .get::<ItemRegistry>("item_registry")
                        .and_then(|r| r.items.get(id))
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| id.to_string());
                    // The weapon's own skill (data/equipment.csv), trained by a landed strike.
                    let skill = if def.skill.is_empty() { "unarmed".to_string() } else { def.skill.clone() };
                    Some((name, skill, def.damage, def.range_m.max(1.5)))
                })
                .unwrap_or(("Bare hands".to_string(), "unarmed".to_string(), 3.0, 1.5));
            let status = match state.targeted_livestock {
                Some(target) => humanity_systems::combat::player_swing(
                    &mut state.game_world.world,
                    &state.data_store,
                    target,
                    state.camera.position,
                    &weapon_name,
                    &skill,
                    damage,
                    range,
                ),
                None => "Nothing in reach".to_string(),
            };
            state.gui_state.ability_status = status;
            state.gui_state.ability_status_at = now_s;
        }
    }

    humanity_core::frame_costs::span("cpu.span.settle_loot");
    // ── Loot settle (v0.760, combat arc) ── kills roll their
    // loot into the loot_drops channel; it lands in the
    // killer's pack volume-gated (overflow is dropped with an
    // honest note - ground items are a later arc).
    let drops: Vec<(String, u32)> = state
        .data_store
        .get::<std::sync::Mutex<Vec<(String, u32)>>>("loot_drops")
        .and_then(|m| m.lock().ok().map(|mut v| std::mem::take(&mut *v)))
        .unwrap_or_default();
    if !drops.is_empty() {
        let mut player: Option<hecs::Entity> = None;
        for (e, _c) in state.game_world.world.query::<&Controllable>().iter() {
            player = Some(e);
            break;
        }
        let items = state.data_store.get::<ItemRegistry>("item_registry");
        let mut gained: Vec<String> = Vec::new();
        let mut overflowed = false;
        if let Some(e) = player {
            if let Ok(mut inv) =
                state.game_world.world.get::<&mut Inventory>(e)
            {
                for (item, count) in &drops {
                    let max_stack =
                        items.map(|r| r.max_stack_for(item)).unwrap_or(99);
                    let unit_vol =
                        items.map(|r| r.volume_for(item)).unwrap_or(0.0);
                    let lost = inv.add_item_volume_gated(
                        item, *count, max_stack, unit_vol,
                    );
                    let kept = count - lost;
                    if lost > 0 {
                        overflowed = true;
                    }
                    if kept > 0 {
                        let name = items
                            .and_then(|r| r.items.get(item))
                            .map(|d| d.name.clone())
                            .unwrap_or_else(|| item.clone());
                        gained.push(format!("+{kept} {name}"));
                    }
                }
            }
        }
        if !gained.is_empty() || overflowed {
            let mut notice = gained.join("  ");
            if overflowed {
                if !notice.is_empty() {
                    notice.push_str("  ");
                }
                notice.push_str("(pack full - some loot lost)");
            }
            state.gui_state.livestock_notice = notice;
            state.gui_state.livestock_notice_at = now_s;
        }
    }

    humanity_core::frame_costs::span("cpu.span.settle_build");
    // ── Blueprint building bridges (v0.746, ladder rung 2) ──
    // Build clicked puts the piece in hand; the ghost, R to turn
    // and E to build it there live in engine/build_place.rs.
    crate::engine::build_place::frame(state);
    // Status line + the blueprint catalog (bridged once).
    if let Some(slot) =
        state.data_store.get::<std::sync::Mutex<String>>("build_status")
    {
        if let Ok(s) = slot.lock() {
            if !s.is_empty() && *s != state.gui_state.build_status {
                state.gui_state.build_status = s.clone();
            }
        }
    }
    if state.gui_state.blueprints.is_empty() {
        if let Some(reg) = state
            .data_store
            .get::<humanity_systems::construction::BlueprintRegistry>(
                "blueprint_registry",
            )
        {
            let mut bps: Vec<crate::gui::GuiBlueprint> = reg
                .blueprints
                .values()
                .map(|bp| crate::gui::GuiBlueprint {
                    id: bp.id.clone(),
                    name: bp.name.clone(),
                    category: bp.category.clone(),
                    materials: bp.materials.clone(),
                    build_time: bp.build_time,
                    provides: bp.provides.clone().unwrap_or_default(),
                    stations: bp.stations.clone(),
                })
                .collect();
            bps.sort_by(|a, b| {
                (a.category.clone(), a.name.clone())
                    .cmp(&(b.category.clone(), b.name.clone()))
            });
            state.gui_state.blueprints = bps;
        }
    }

    // Station-gate input (v0.749; where the player is since
    // 2026-09-27): the station types and their power for the
    // CraftingSystem and the Crafting page (engine/built_uses.rs).
    crate::engine::built_uses::publish_stations(state);

    humanity_core::frame_costs::span("cpu.span.settle_vendor");
    // ── Vendor + wallet bridges (v0.747, ladder rung 3) ──
    // Live credit balance for the HUD + vendor modal.
    for (_e, (wallet, _c)) in state
        .game_world
        .world
        .query::<(&humanity_core::ecs::components::Wallet, &Controllable)>()
        .iter()
    {
        state.gui_state.wallet_credits = wallet.credits;
        break;
    }
    // Vendor catalog, built once: goods that exist in BOTH
    // trade_goods.ron and items.csv (never sell an id the game
    // cannot hold), sorted by category then name.
    if state.gui_state.vendor_goods.is_empty() {
        if let (Some(goods), Some(items)) = (
            state
                .data_store
                .get::<humanity_systems::economy::TradeGoodsRegistry>(
                    "trade_goods_registry",
                ),
            state.data_store.get::<ItemRegistry>("item_registry"),
        ) {
            let mut list: Vec<crate::gui::GuiTradeGood> = goods
                .goods
                .values()
                .filter(|g| items.items.contains_key(&g.id))
                .map(|g| crate::gui::GuiTradeGood {
                    id: g.id.clone(),
                    name: g.name.clone(),
                    category: g.category.clone(),
                    buy_price: goods.vendor_sell_price(&g.id).unwrap_or(1),
                    sell_price: goods.vendor_buy_price(&g.id).unwrap_or(0),
                    parts_price: goods.parts_price(&g.id),
                })
                .collect();
            list.sort_by(|a, b| {
                (a.category.clone(), a.name.clone())
                    .cmp(&(b.category.clone(), b.name.clone()))
            });
            state.gui_state.vendor_goods = list;
        }
    }
    // Settle Buy/Sell intents against the ECS inventory + Wallet.
    let vendor_ops = (
        state.gui_state.pending_vendor_buy.take(),
        state.gui_state.pending_vendor_sell.take(),
    );
    if vendor_ops.0.is_some() || vendor_ops.1.is_some() {
        let mut player: Option<hecs::Entity> = None;
        for (e, _c) in state.game_world.world.query::<&Controllable>().iter() {
            player = Some(e);
            break;
        }
        if let (Some(e), Some(goods)) = (
            player,
            state
                .data_store
                .get::<humanity_systems::economy::TradeGoodsRegistry>(
                    "trade_goods_registry",
                ),
        ) {
            let items = state.data_store.get::<ItemRegistry>("item_registry");
            let qlevels = state
                .data_store
                .get::<humanity_systems::crafting::quality::QualityLevels>("quality_levels");
            let result = (|| -> Result<String, String> {
                let mut inv = state
                    .game_world
                    .world
                    .get::<&mut Inventory>(e)
                    .map_err(|_| "no inventory".to_string())?;
                let mut wallet = state
                    .game_world
                    .world
                    .get::<&mut humanity_core::ecs::components::Wallet>(e)
                    .map_err(|_| "no wallet".to_string())?;
                if let Some((id, qty)) = vendor_ops.0 {
                    humanity_systems::economy::vendor_buy(
                        &mut inv,
                        &mut wallet.credits,
                        goods,
                        items,
                        &id,
                        qty,
                    )
                } else if let Some((id, qty, grade)) = vendor_ops.1 {
                    humanity_systems::economy::vendor_sell(
                        &mut inv,
                        &mut wallet.credits,
                        goods,
                        &id,
                        qty,
                        grade,
                        qlevels,
                    )
                } else {
                    Ok(String::new())
                }
            })();
            // Skills rung 1: a sale or purchase that went through is trading practice.
            if matches!(&result, Ok(m) if !m.is_empty()) {
                humanity_systems::skills::practice(&state.data_store, "vendor_trade", None);
            }
            state.gui_state.vendor_status = match result {
                Ok(msg) => msg,
                Err(e) => e,
            };
        }
    }

    humanity_core::frame_costs::span("cpu.span.settle_abilities");
    // ── Abilities bridge (v0.753, ladder rung 8) ──
    // Cast click -> the ability_request channel AbilitySystem
    // drains at its tick; the status line rides back the same
    // frame it lands.
    if let Some((id, target)) = state.gui_state.pending_cast.take() {
        if let Some(req) = state
            .data_store
            .get::<std::sync::Mutex<Vec<(String, Option<u64>)>>>("ability_request")
        {
            if let Ok(mut v) = req.lock() {
                v.push((id, target));
            }
        }
    }
    if let Some(s) = state
        .data_store
        .get::<std::sync::Mutex<String>>("ability_status")
    {
        if let Ok(mut slot) = s.lock() {
            if !slot.is_empty() {
                // TAKE the line (leave the channel empty) so the
                // fade below cannot re-latch a stale status.
                state.gui_state.ability_status = std::mem::take(&mut *slot);
                state.gui_state.ability_status_at = now_s;
            }
        }
    }
    // Fade the cast feedback after 4 game-seconds (the Profile
    // panel and the HUD ability bar both draw it while set).
    if !state.gui_state.ability_status.is_empty()
        && now_s - state.gui_state.ability_status_at > 4.0
    {
        state.gui_state.ability_status.clear();
    }
    // Panel rows, rebuilt while in the world (the HUD ability
    // bar needs them for 1-9 casting, not just the Profile
    // page; cooldowns move every frame, gates on level-up).
    if state.gui_state.active_page == GuiPage::Profile || state.world_loaded {
        if let Some(reg) = state
            .data_store
            .get::<humanity_systems::abilities::AbilityRegistry>("ability_registry")
        {
            let cooldowns = state
                .data_store
                .get::<std::sync::Mutex<std::collections::HashMap<String, f32>>>(
                    "ability_cooldowns",
                )
                .and_then(|m| m.lock().ok().map(|g| g.clone()))
                .unwrap_or_default();
            let mut rows: Vec<crate::gui::GuiAbility> = Vec::new();
            {
                let mut q = state
                    .game_world
                    .world
                    .query::<(&PlayerSkills, &Controllable)>();
                if let Some((_e, (skills, _c))) = q.iter().next() {
                    for def in reg.defs.values() {
                        let gate_ok = def.skill_gate_met(skills);
                        // Healing rows self-cast; damage rows
                        // cast at the faced creature (v0.760).
                        let has_effect =
                            def.self_castable() || def.damage_base > 0.0;
                        let castable = has_effect && gate_ok;
                        let locked_reason = if !has_effect {
                            "Does nothing yet - a later arc wires it".to_string()
                        } else if !gate_ok {
                            format!(
                                "Needs {} level {}",
                                def.skill_required, def.skill_level
                            )
                        } else {
                            String::new()
                        };
                        rows.push(crate::gui::GuiAbility {
                            id: def.id.clone(),
                            name: def.name.clone(),
                            school: def.school.clone(),
                            flavor: def.flavor.clone(),
                            cost: def.energy_cost(),
                            cooldown_s: def.cooldown_s,
                            cooldown_remaining: cooldowns
                                .get(&def.id)
                                .copied()
                                .unwrap_or(0.0),
                            heals: def.healing_base,
                            castable_now: castable,
                            locked_reason,
                            description: def.description.clone(),
                        });
                    }
                }
            }
            rows.sort_by(|a, b| {
                (!a.castable_now, &a.name).cmp(&(!b.castable_now, &b.name))
            });
            state.gui_state.abilities = rows;
        }
    }

    humanity_core::frame_costs::span("cpu.span.settle_armor");
    // ── Armor from worn gear (v0.761, combat arc) ── the
    // equipment.csv armor columns populate the player's Armor
    // component every frame (cheap: a few map sums) so saved
    // outfits protect from load and equip changes apply
    // instantly. CombatSystem mitigates hostile bites by it.
    {
        let mut player: Option<hecs::Entity> = None;
        let mut worn: Vec<String> = Vec::new();
        for (e, (outfit, _c)) in state
            .game_world
            .world
            .query::<(&humanity_core::ecs::components::Outfit, &Controllable)>()
            .iter()
        {
            player = Some(e);
            worn = outfit.equipped.values().cloned().collect();
            break;
        }
        if let (Some(e), Some(reg)) = (
            player,
            state
                .data_store
                .get::<humanity_systems::economy::EquipmentRegistry>(
                    "equipment_registry",
                ),
        ) {
            let resistance = reg.armor_from_worn(worn.iter().map(|s| s.as_str()));
            let _ = state
                .game_world
                .world
                .insert_one(e, humanity_core::ecs::components::Armor { resistance });
        }
    }

    humanity_core::frame_costs::span("cpu.span.settle_equip");
    // ── Equip/unequip bridge (v0.750, ladder rung 8) ──
    // Equipping MOVES the item from the pack into the ECS
    // Outfit (slot from equipment.csv); anything already worn
    // there swaps back to the pack. Unequip is the reverse.
    // The Outfit mirror below the match keeps the GUI in sync.
    let equip_op = (
        state.gui_state.pending_equip.take(),
        state.gui_state.pending_unequip.take(),
    );
    if equip_op.0.is_some() || equip_op.1.is_some() {
        let mut player: Option<hecs::Entity> = None;
        for (e, _c) in state.game_world.world.query::<&Controllable>().iter() {
            player = Some(e);
            break;
        }
        let equip_reg = state
            .data_store
            .get::<humanity_systems::economy::EquipmentRegistry>(
                "equipment_registry",
            );
        let items = state.data_store.get::<ItemRegistry>("item_registry");
        // The grips that take both hands (data/characters/grips.ron's rows with a
        // second hand: two_handed, staff), character animation rung 2b.
        let two_hand_grips: Vec<String> = state
            .animation
            .library()
            .map(|l| l.grips.grips.iter().filter(|g| g.second_hand_m.is_some()).map(|g| g.id.clone()).collect())
            .unwrap_or_default();
        if let Some(e) = player {
            let result: Result<String, String> = (|| {
                let mut inv = state
                    .game_world
                    .world
                    .get::<&mut Inventory>(e)
                    .map_err(|_| "no inventory".to_string())?;
                let mut outfit = state
                    .game_world
                    .world
                    .get::<&mut humanity_core::ecs::components::Outfit>(e)
                    .map_err(|_| "no outfit".to_string())?;
                if let Some(item_id) = equip_op.0 {
                    let def = equip_reg
                        .and_then(|r| r.get(&item_id))
                        .ok_or("That item has no wearable stats")?;
                    if inv.count_item(&item_id) == 0 {
                        return Err("Not in your pack".to_string());
                    }
                    // Swap out the current occupant first (it
                    // must FIT back in the pack, or refuse).
                    if let Some(prev) = outfit.equipped.get(&def.slot).cloned() {
                        let max_stack =
                            items.map(|r| r.max_stack_for(&prev)).unwrap_or(99);
                        let vol =
                            items.map(|r| r.volume_for(&prev)).unwrap_or(0.0);
                        let lost =
                            inv.add_item_volume_gated(&prev, 1, max_stack, vol);
                        if lost > 0 {
                            return Err(
                                "No room to take off what you're wearing".into()
                            );
                        }
                    }
                    // Two hands on one shaft (character animation rung 2b): a held
                    // thing whose grip takes the second hand (an axe, a spear) frees
                    // the left hand's grip socket into the pack; nothing goes in the
                    // left hand while the right holds one.
                    let takes_both = |id: &str| equip_reg.and_then(|r| r.get(id)).is_some_and(|d| two_hand_grips.iter().any(|g| *g == d.grip));
                    if def.slot == "grip_l" && outfit.equipped.get("grip_r").is_some_and(|h| takes_both(h)) {
                        return Err("Both hands are on what you hold".into());
                    }
                    if def.slot == "grip_r" && takes_both(&item_id) {
                        if let Some(prev) = outfit.equipped.get("grip_l").cloned() {
                            let max_stack = items.map(|r| r.max_stack_for(&prev)).unwrap_or(99);
                            let vol = items.map(|r| r.volume_for(&prev)).unwrap_or(0.0);
                            if inv.add_item_volume_gated(&prev, 1, max_stack, vol) > 0 {
                                return Err("No room to put down what is in your other hand".into());
                            }
                            outfit.equipped.remove("grip_l");
                        }
                    }
                    inv.remove_item(&item_id, 1);
                    outfit.equipped.insert(def.slot.clone(), item_id.clone());
                    Ok(format!("Equipped {item_id}"))
                } else if let Some(slot) = equip_op.1 {
                    let Some(prev) = outfit.equipped.get(&slot).cloned() else {
                        return Ok(String::new());
                    };
                    // Cosmetic ids (no equipment.csv row) just
                    // clear; real gear returns to the pack.
                    if equip_reg.and_then(|r| r.get(&prev)).is_some() {
                        let max_stack =
                            items.map(|r| r.max_stack_for(&prev)).unwrap_or(99);
                        let vol =
                            items.map(|r| r.volume_for(&prev)).unwrap_or(0.0);
                        let lost =
                            inv.add_item_volume_gated(&prev, 1, max_stack, vol);
                        if lost > 0 {
                            return Err("No room in your pack".to_string());
                        }
                    }
                    outfit.equipped.remove(&slot);
                    Ok(format!("Removed {prev}"))
                } else {
                    Ok(String::new())
                }
            })();
            state.gui_state.equip_status = match result {
                Ok(msg) => msg,
                Err(e) => e,
            };
            // Refresh the GUI mirror immediately.
            if let Ok(outfit) = state
                .game_world
                .world
                .get::<&humanity_core::ecs::components::Outfit>(e)
            {
                state.gui_state.outfit = (*outfit).clone();
            }
        }
    }

    humanity_core::frame_costs::span("cpu.span.settle_respawn");
    // (Moved from frame_mirrors.rs in the headless sim runner's increment 4: a game rule, so part of
    // the step, after the systems as it ran before, now before the view's draw lists.)
    // ── Death & recovery (v0.745, loop-map rung 1) ──
    // A death the systems record is surfaced right after their tick, with
    // what it cost in the Death mode chosen (engine/death_pack.rs after_tick).
    // The Respawn button: back to the spawn room (the fibonacci
    // design's respawner/medbay), full health, survivable vitals,
    // effects cleared, Dead removed. What the death cost was settled
    // when it happened (Realistic leaves the backpack in a pack).
    // Where the dead player may wake, for the death screen's list: filled once per death, the
    // usual place first (game-setting-and-intro.md 5.6, rung 2, engine/wake.rs).
    crate::engine::wake::refresh_view(state);
    if state.gui_state.pending_respawn {
        state.gui_state.pending_respawn = false;
        state.gui_state.player_death_cause = None;
        // At the respawner they chose (or their usual one, or home's, or General Services');
        // their front door only when no respawner is theirs to use (rung 2, engine/wake.rs).
        let wake_at = crate::engine::wake::respawn_spot(state).unwrap_or(state.fps_spawn);
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
                t.position = wake_at;
            }
        }
        state.camera.position = wake_at;
        // A rig's camera park pins the camera where it parked (`probe_hold`, released by any key a
        // person presses); a respawn is a placement, so the pin moves with it, as the far-frame
        // split's does (ipc.rs), or the pin drags the body straight back to the park (seen
        // 2026-11-21: verify-respawn's first run woke at home and stood at the front door again).
        if let Some((_, t)) = state.probe_hold.take() {
            state.probe_hold = Some((wake_at, t));
        }
        // On the floor under the spawn, whatever storey the player died on (ship
        // homes increment 7): the door is on the ground floor, under a slab, and the
        // Homestead's respawner is upstairs (rung 2): the floor is measured from where
        // the eye was put, so each lands on its own storey.
        crate::engine::home_plot::reseat_floor(state);
        // Out of the truck (its seat freed) and off the follow cam, either
        // of which would put the camera back on its vehicle next frame.
        crate::engine::home_plot::step_out_of_vehicles(
            &mut state.game_world.world,
            &mut state.driving_vehicle,
            &mut state.follow_vehicle,
        );
        log::info!("[Vitals] player respawned at {wake_at:?}");
        // In the shared world the relay still holds them where they died, up
        // to 154 m away: step out and join again, so it stands them at their
        // door too (ship homes 1b, engine/home_plot.rs).
        crate::engine::home_plot::respawn_through_relay(state);
    }
}
