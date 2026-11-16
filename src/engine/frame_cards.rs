//! Machine cards (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: the home power readout, the machine labels and the
//! pinned cards' auto-recipe selector.

use crate::engine::frame_prelude::*;

/// Updates the power readout, the machine labels and the pinned cards.
pub(crate) fn run(state: &mut EngineState) {
    humanity_core::frame_costs::span("cpu.span.cards");
    // Bridge the live home power readout (ElectricalSystem writes it via
    // Mutex). Drives the HUD power line that swings with day/night.
    if let Some(ps) = state
        .data_store
        .get::<std::sync::Mutex<humanity_systems::electrical::PowerStatus>>("power_status")
        .and_then(|m| m.lock().ok())
    {
        state.gui_state.power_generation = ps.generation;
        state.gui_state.power_consumption = ps.consumption;
        state.gui_state.power_balance = ps.balance;
        state.gui_state.power_battery_wh = ps.battery_wh;
        state.gui_state.power_battery_capacity_wh = ps.battery_capacity_wh;
        state.gui_state.power_autonomy_hours = ps.autonomy_hours;
        state.gui_state.power_ship = ps.ship;
    }
    // The machines' day record (2026-10-22), copied when an hour completes.
    if humanity_ship::machines::day_energy::revision(&state.data_store) != state.gui_state.machine_day_energy.revision {
        state.gui_state.machine_day_energy = humanity_ship::machines::day_energy::snapshot(&state.data_store);
    }

    // Bridge the live home WATER readout (PlumbingSystem writes it via Mutex). (v0.608)
    if let Some(ws) = state
        .data_store
        .get::<std::sync::Mutex<humanity_systems::plumbing::WaterStatus>>("water_status")
        .and_then(|m| m.lock().ok())
    {
        state.gui_state.water_production_lpm = ws.production_lpm;
        state.gui_state.water_demand_lpm = ws.demand_lpm;
        state.gui_state.water_stored_l = ws.stored_l;
        state.gui_state.water_capacity_l = ws.capacity_l;
        state.gui_state.water_days_autonomy = ws.days_autonomy;
    }
    if state.gui_state.quality_levels.levels.is_empty() {
        if let Some(q) = state.data_store.get::<humanity_systems::crafting::quality::QualityLevels>("quality_levels") {
            state.gui_state.quality_levels = q.clone();
        }
    }
    if state.gui_state.tap_litres.is_empty() {
        if let Some(ft) = state.data_store.get::<humanity_systems::fluids::FluidTable>("fluid_table") {
            state.gui_state.tap_litres = ft.tap.clone();
        }
    }

    // Bridge the live home AIR readout (AtmosphereSystem writes it via Mutex). (v0.617)
    if let Some(asr) = state
        .data_store
        .get::<std::sync::Mutex<humanity_systems::atmosphere::AirStatus>>("air_status")
        .and_then(|m| m.lock().ok())
    {
        state.gui_state.air_o2_pct = asr.o2_pct;
        state.gui_state.air_co2_pct = asr.co2_pct;
        state.gui_state.air_pressure_atm = asr.pressure_atm;
        state.gui_state.air_temp_c = asr.temp_c;
        state.gui_state.air_breathable = asr.breathable;
    }

    // ── Live machine walk-up card stats (v0.724, machine
    // info-window overhaul): patch each label's stat rows
    // from its entity's live components, replacing the
    // static RON placeholders (the cistern's hardcoded
    // "33 days", the battery's "~4 kWh"). Cheap: a few
    // dozen machines, one small hashmap per frame.
    if !state.gui_state.machine_labels.is_empty() {
        use humanity_core::ecs::components::{Battery, MachineInstanceId, WaterTank};
        // Patch (or append) the first stat row whose kind is in
        // `kinds`; keeps the RON author's icon choice when a row
        // already exists.
        fn patch_stat(
            stats: &mut Vec<humanity_ship::machines::MachineStat>,
            kinds: &[&str],
            new_kind: &str,
            value: String,
            status: &str,
        ) {
            if let Some(row) = stats.iter_mut().find(|s| kinds.contains(&s.kind.as_str())) {
                row.value = value;
                row.status = status.to_string();
            } else {
                stats.push(humanity_ship::machines::MachineStat {
                    kind: new_kind.to_string(),
                    value,
                    status: status.to_string(),
                });
            }
        }
        // (tank, battery, container-fill + contents-line)
        type LiveStats = (
            Option<(f32, f32)>,
            Option<(f32, f32)>,
            Option<(String, String, &'static str)>,
        );
        let item_reg = state
            .data_store
            .get::<humanity_systems::inventory::ItemRegistry>("item_registry");
        let mut live: std::collections::HashMap<String, LiveStats> =
            std::collections::HashMap::new();
        for (_e, (mid, tank, bat, cont)) in state
            .game_world
            .world
            .query::<(
                &MachineInstanceId,
                Option<&WaterTank>,
                Option<&Battery>,
                Option<&humanity_systems::inventory::containers::Container>,
            )>()
            .iter()
        {
            if tank.is_none() && bat.is_none() && cont.is_none() {
                continue;
            }
            // Typed container -> a fill line + a contents line
            // ("containers show contents", v0.728).
            let cont_rows = cont.map(|c| {
                let fill = format!("{:.0} / {:.0} L", c.used_liters, c.capacity_liters);
                let contents = if c.is_broken() {
                    ("BROKEN (spilled)".to_string(), "off")
                } else if let Some(item) = &c.current_content_item {
                    let name = item_reg
                        .and_then(|r| r.items.get(item))
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| item.clone());
                    (format!("{}x {}", c.current_qty, name), "ok")
                } else {
                    ("empty".to_string(), "off")
                };
                (fill, contents.0, contents.1)
            });
            live.insert(
                mid.0.clone(),
                (
                    tank.map(|t| (t.liters, t.capacity_l)),
                    bat.map(|b| (b.charge_wh, b.capacity_wh)),
                    cont_rows,
                ),
            );
        }
        for label in &mut state.gui_state.machine_labels {
            let Some((tank, bat, cont)) = live.get(&label.machine_id) else { continue };
            if let Some((liters, cap)) = tank {
                let frac = if *cap > 0.0 { liters / cap } else { 0.0 };
                let status = if frac < 0.15 { "low" } else { "ok" };
                patch_stat(
                    &mut label.stats,
                    &["water", "storage"],
                    "water",
                    format!("{:.0} / {:.0} L", liters, cap),
                    status,
                );
            }
            if let Some((wh, cap)) = bat {
                let frac = if *cap > 0.0 { wh / cap } else { 0.0 };
                let status = if frac < 0.15 { "low" } else { "ok" };
                patch_stat(
                    &mut label.stats,
                    &["power", "storage"],
                    "power",
                    format!("{:.1} / {:.1} kWh", wh / 1000.0, cap / 1000.0),
                    status,
                );
            }
            if let Some((fill, contents, cstatus)) = cont {
                // Fill goes on the storage/fuel row (keeps the
                // RON author's icon); contents get their own row.
                patch_stat(
                    &mut label.stats,
                    &["storage", "fuel"],
                    "storage",
                    fill.clone(),
                    "ok",
                );
                patch_stat(
                    &mut label.stats,
                    &["contents"],
                    "contents",
                    contents.clone(),
                    cstatus,
                );
            }
        }
    }

    // (The HUD's sight blockers are built earlier this frame, ahead of the
    // walk-up target, which tests its line of sight with them too: search
    // "Sight blockers (v0.975".)

    // ── Pinned-card auto-recipe selector (v0.725, info-window
    // overhaul part 2): publish the E-opened machine's
    // AutoRefine recipe + its same-station alternatives to the
    // GUI, and apply the player's dropdown pick back onto the
    // entity. Infinite-of-X: the options are recipes.csv rows
    // sharing the machine's station (the fixed auto_recipe in
    // home.ron is now just the DEFAULT, not the only choice).
    {
        use humanity_core::ecs::components::{AutoRefine, MachineInstanceId};
        let mut current: Option<String> = None;
        let mut options: Vec<(String, String)> = Vec::new();
        let mut container_pub: Option<(String, String, u32)> = None;
        let sel_mid = state
            .gui_state
            .selected_machine
            .and_then(|i| state.gui_state.machine_labels.get(i))
            .map(|l| l.machine_id.clone());
        // Cleared here; the selected-machine block below
        // repopulates it when a vessel is pinned.
        state.gui_state.machine_card_storable.clear();
        state.gui_state.machine_card_fluid_actions.clear();
        if let Some(mid) = sel_mid {
            let pending = state.gui_state.machine_card_recipe_pending.take();
            for (_e, (id, auto)) in state
                .game_world
                .world
                .query_mut::<(&MachineInstanceId, &mut AutoRefine)>()
            {
                if id.0 != mid {
                    continue;
                }
                if let Some(p) = &pending {
                    auto.recipe_id = p.clone();
                }
                current = Some(auto.recipe_id.clone());
                break;
            }
            if let Some(cur) = &current {
                if let Some(reg) = state
                    .data_store
                    .get::<humanity_systems::crafting::RecipeRegistry>("recipe_registry")
                {
                    if let Some(station) =
                        reg.recipes.get(cur).and_then(|r| r.required_station.clone())
                    {
                        let mut opts: Vec<(String, String)> = reg
                            .recipes_for_station(&station)
                            .iter()
                            .map(|r| (r.id.clone(), r.name.clone()))
                            .collect();
                        opts.sort_by(|a, b| a.1.cmp(&b.1));
                        options = opts;
                    }
                }
            }

            // ── Container contents + the Take action (v0.731):
            // deposits into vessels are automatic (harvest
            // surplus), so the card panel is the WITHDRAW path —
            // "Take" moves as much as fits (volume-gated) from
            // the machine's Container into the backpack.
            let take = std::mem::take(&mut state.gui_state.machine_card_take_pending);
            let item_reg = state
                .data_store
                .get::<humanity_systems::inventory::ItemRegistry>("item_registry");
            let mut cont_entity: Option<hecs::Entity> = None;
            let mut contents: Option<(String, u32)> = None;
            // The whole container, for what it remembers (2026-09-26).
            let mut cont_snapshot: Option<humanity_systems::inventory::containers::Container> = None;
            for (e, (id, c)) in state
                .game_world
                .world
                .query::<(
                    &MachineInstanceId,
                    &humanity_systems::inventory::containers::Container,
                )>()
                .iter()
            {
                if id.0 != mid {
                    continue;
                }
                cont_entity = Some(e);
                cont_snapshot = Some(c.clone());
                if let Some(item) = &c.current_content_item {
                    if c.current_qty > 0 {
                        contents = Some((item.clone(), c.current_qty));
                    }
                }
                break;
            }
            // When this vessel is a generator's fuel drum, the
            // fuels its engine burns (BUG-154), and when it is
            // the feed of a machine that runs a recipe by itself
            // (the refinery's drum, 2026-10-20), that recipe's
            // feedstock: it takes and offers only those.
            let fuels = cont_entity.and_then(|e| {
                humanity_systems::inventory::containers::vessel_takes(
                    &state.game_world.world,
                    &state.data_store,
                    e,
                )
            });
            // Clean (2026-09-26): wash an emptied vessel of its
            // residue with water from the home tanks.
            if std::mem::take(&mut state.gui_state.machine_card_clean_pending) {
                if let Some(e) = cont_entity {
                    let water_ok = state
                        .data_store
                        .get::<std::sync::Mutex<humanity_systems::plumbing::WaterStatus>>("water_status")
                        .and_then(|m| m.lock().ok().map(|ws| ws.capacity_l <= 0.0 || ws.stored_l > ws.capacity_l * 0.02))
                        .unwrap_or(true);
                    if !water_ok {
                        state.gui_state.pending_toasts.push((
                            "The water tanks are empty: nothing to clean with.".to_string(),
                            crate::gui::ToastKind::Info,
                        ));
                    } else {
                        // Water, and soap for a food residue (2026-09-26).
                        let toast = match humanity_systems::inventory::containers::clean_container(&mut state.game_world.world, &state.data_store, e) {
                            Ok((l, None)) => (format!("Cleaned with {l:.0} L of water."), crate::gui::ToastKind::Success),
                            Ok((l, Some(a))) => (format!("Washed with {l:.0} L of water and your {a}."), crate::gui::ToastKind::Success),
                            Err(m) => (m, crate::gui::ToastKind::Info),
                        };
                        state.gui_state.pending_toasts.push(toast);
                        cont_snapshot = state.game_world.world.get::<&humanity_systems::inventory::containers::Container>(e).ok().map(|c| (*c).clone());
                    }
                }
            }
            if take {
                if let (Some(e), Some((item, qty))) = (cont_entity, contents.clone()) {
                    let unit_vol = item_reg.map(|r| r.volume_for(&item)).unwrap_or(0.0);
                    let max_stack = item_reg.map(|r| r.max_stack_for(&item)).unwrap_or(99);
                    let age = cont_snapshot.as_ref().map_or(0.0, |c| c.content_age_s); // food keeps its age (S6)
                    let mut taken = 0u32;
                    for (_pe, (inv, _c)) in state.game_world.world.query_mut::<(
                        &mut humanity_systems::inventory::Inventory,
                        &humanity_core::ecs::components::Controllable,
                    )>() {
                        let overflow =
                            inv.add_item_volume_gated_aged(&item, qty, max_stack, unit_vol, 0, age);
                        taken = qty - overflow;
                        break;
                    }
                    if taken > 0 {
                        if let Ok(mut c) = state.game_world.world.get::<&mut humanity_systems::inventory::containers::Container>(e) {
                            c.current_qty -= taken;
                            c.used_liters =
                                (c.used_liters - taken as f32 * unit_vol).max(0.0);
                            if c.current_qty == 0 {
                                c.current_content_item = None;
                                c.used_liters = 0.0;
                            }
                        }
                        contents = if qty > taken {
                            Some((item.clone(), qty - taken))
                        } else {
                            None
                        };
                        log::info!("[Machines] took {taken}x {item} from {mid}");
                    }
                }
            }
            // Store (v0.733): move a chosen pack item into the
            // vessel: crude oil into the refinery's drum, its
            // gasoline into the genset's. Compatibility is
            // guaranteed by construction (the Store buttons only
            // list accepted items), and a generator's drum takes
            // only its fuels (BUG-154), the refinery's only its
            // crude oil (2026-10-20), whatever asks.
            let store = state
                .gui_state
                .machine_card_store_pending
                .take()
                .filter(|i| humanity_systems::inventory::containers::vessel_takes_item(fuels.as_ref(), i));
            if let (Some(e), Some(item)) = (cont_entity, store) {
                let unit_vol =
                    item_reg.map(|r| r.volume_for(&item)).unwrap_or(0.0);
                let class = item_reg
                    .map(|r| r.class_for(&item).to_string())
                    .unwrap_or_else(|| "solid".to_string());
                let mut have = 0u32;
                for (_pe, (inv, _c)) in state.game_world.world.query_mut::<(
                    &humanity_systems::inventory::Inventory,
                    &humanity_core::ecs::components::Controllable,
                )>() {
                    have = inv.count_item(&item);
                    break;
                }
                if have > 0 {
                    let mut stored = 0u32;
                    if let Some(creg) = state
                        .data_store
                        .get::<humanity_systems::inventory::containers::ContainerRegistry>(
                            "container_registry",
                        )
                    {
                        if let Ok(mut c) = state
                            .game_world
                            .world
                            .get::<&mut humanity_systems::inventory::containers::Container>(e)
                        {
                            if let humanity_systems::inventory::containers::StoreOutcome::Stored { quantity } =
                                creg.try_store(&mut c, &item, &class, unit_vol, have)
                            {
                                stored = quantity;
                            }
                            contents = c
                                .current_content_item
                                .clone()
                                .filter(|_| c.current_qty > 0)
                                .map(|i| (i, c.current_qty));
                            cont_snapshot = Some(c.clone());
                        }
                    }
                    if stored > 0 {
                        let mut age = 0.0;
                        for (_pe, (inv, _c)) in state.game_world.world.query_mut::<(
                            &mut humanity_systems::inventory::Inventory,
                            &humanity_core::ecs::components::Controllable,
                        )>() {
                            age = inv.remove_item_aged(&item, stored).1;
                            break;
                        }
                        // Food goes into the vessel at the age it had (S6).
                        if let Ok(mut c) = state.game_world.world.get::<&mut humanity_systems::inventory::containers::Container>(e) {
                            c.arrived_aged(stored, age);
                        }
                        log::info!("[Machines] stored {stored}x {item} into {mid}");
                    }
                }
            }
            // Storable pack items (for the per-item Store buttons):
            // what this vessel would take now, a generator's drum
            // only its fuels (BUG-154), the refinery's only its
            // crude oil (2026-10-20). See `store_offers`. And why
            // what its class would take has no button
            // (`refused_note`), for the card's note below.
            let mut storable: Vec<(String, String, u32)> = Vec::new();
            let mut refused: Option<String> = None;
            if let (Some(c), Some(creg)) = (
                cont_snapshot.as_ref(),
                state
                    .data_store
                    .get::<humanity_systems::inventory::containers::ContainerRegistry>(
                        "container_registry",
                    ),
            ) {
                for (_pe, (inv, _c)) in state.game_world.world.query::<(
                    &humanity_systems::inventory::Inventory,
                    &humanity_core::ecs::components::Controllable,
                )>().iter() {
                    let pack = inv.summary();
                    storable = humanity_systems::inventory::containers::store_offers(
                        creg,
                        item_reg,
                        c,
                        fuels.as_ref(),
                        &pack,
                    );
                    refused = humanity_systems::inventory::containers::refused_note(
                        creg,
                        item_reg,
                        c,
                        fuels.as_ref(),
                        &pack,
                    );
                    break;
                }
            }
            state.gui_state.machine_card_storable = storable;
            // What the vessel remembers, for the card (2026-09-26).
            let name_of = |id: &str| {
                item_reg
                    .and_then(|r| r.items.get(id))
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| id.to_string())
            };
            state.gui_state.machine_card_can_clean =
                cont_snapshot.as_ref().map_or(false, |c| c.needs_cleaning());
            state.gui_state.machine_card_container_note = cont_snapshot.as_ref().and_then(|c| {
                let mut parts = Vec::new();
                if c.needs_cleaning() {
                    if let Some(last) = &c.last_content {
                        parts.push(format!("Empty, with {} residue: clean it before it holds anything else.", name_of(last)));
                    }
                }
                if let Some(t) = &c.toxic_from {
                    parts.push(format!("Held {}: never food or drinking water again.", name_of(t)));
                }
                if let Some(r) = refused.take() {
                    parts.push(r);
                }
                if parts.is_empty() { None } else { Some(parts.join(" ")) }
            });
            // Fill and pour at a water tank (2026-09-26): crates/humanity-systems/src/fluids.rs. The
            // card's button is carried out by the frame's bridges, before the tick
            // (engine/frame_sim_requests.rs, the headless sim runner's increment 3); here the card
            // only lists what can be done now.
            if let Some(ft) = state.data_store.get::<humanity_systems::fluids::FluidTable>("fluid_table") {
                use humanity_systems::fluids::card_actions;
                state.gui_state.machine_card_fluid_actions = card_actions(ft, &state.game_world.world, item_reg, &mid)
                    .into_iter()
                    .map(|a| (a.key, a.label, a.tip))
                    .collect();
            }
            container_pub = contents.map(|(item, qty)| {
                let name = item_reg
                    .and_then(|r| r.items.get(&item))
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| item.clone());
                (item, name, qty)
            });
        }
        // Patch the pinned machine's "progress" row with its LIVE
        // production status ("42%", "waiting for Coal x1", "pad
        // full") — the v0.681 status lines only showed on the
        // Inventory page, so a stalled smelter looked idle at the
        // machine itself (operator field report 2026-07-06).
        if let (Some(sel), Some(cur)) =
            (state.gui_state.selected_machine, current.as_ref())
        {
            let recipe_name = options
                .iter()
                .find(|(id, _)| id == cur)
                .map(|(_, n)| n.clone());
            if let Some(name) = recipe_name {
                if let Some(line) = state
                    .gui_state
                    .factory_status
                    .iter()
                    .find(|l| l.contains(&name))
                    .cloned()
                {
                    // Lines read "Recipe Name — detail"; the card
                    // value column is narrow, keep the detail.
                    let mut val = line
                        .split('—')
                        .nth(1)
                        .map(|s| s.trim().to_string())
                        .unwrap_or(line);
                    if val.chars().count() > 20 {
                        val = format!("{}...", val.chars().take(17).collect::<String>());
                    }
                    if let Some(lbl) = state.gui_state.machine_labels.get_mut(sel) {
                        if let Some(row) =
                            lbl.stats.iter_mut().find(|s| s.kind == "progress")
                        {
                            row.value = val;
                            row.status = "ok".to_string();
                        } else {
                            lbl.stats.push(humanity_ship::machines::MachineStat {
                                kind: "progress".to_string(),
                                value: val,
                                status: "ok".to_string(),
                            });
                        }
                    }
                }
            }
        }
        state.gui_state.machine_card_recipe = current;
        state.gui_state.machine_card_recipe_options = options;
        state.gui_state.machine_card_container = container_pub;
        // Trading post? (v0.747, ladder rung 3): the pinned
        // card grows a Trade button that opens the vendor.
        state.gui_state.machine_card_vendor = state
            .gui_state
            .selected_machine
            .and_then(|i| state.gui_state.machine_labels.get(i))
            .map(|l| l.machine_id.clone())
            .and_then(|mid| {
                state.gui_state.home_machines.as_ref().map(|hm| {
                    hm.instances
                        .iter()
                        .any(|inst| inst.id == mid && inst.machine == "trading_post")
                })
            })
            .unwrap_or(false);
        // The mess hall's serving counter (ship homes increment 6): its card's
        // flag, its button's ask, the offline meter (engine/mess_hall.rs).
        crate::engine::mess_hall::frame(state);
    }

}
