//! What the player looks at (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: the camera kept in the ship's own coordinates, and the
//! walk-up targets (animals, crew, doors and their control panels) the prompts show.

use crate::engine::frame_prelude::*;

/// Finds this frame's walk-up targets for the prompts.
pub(crate) fn run(state: &mut EngineState) {
    humanity_core::frame_costs::span("cpu.span.look_env");
    // Camera stays in local ship coords (no floating origin reset)
    // Floating origin is only used for rendering distant bodies

    // Sync camera state into DataStore for game systems
    // Home-LOCAL camera for proximity systems (v0.881):
    // aboard this equals the raw camera; away it is far from
    // every home machine, silencing interactions naturally.
    state
        .data_store
        .insert("camera_position", state.camera.position - state.station_off);
    // v0.697 fix: this used to recompute forward as (-sin, -cos),
    // X-MIRRORING the camera's real (sin, -cos) convention -- correct
    // facing north/south, inverted east/west (the operator's "east/
    // west is inverted" drive report; deploy/summon spots too). Use
    // the camera's own forward, never a reimplementation.
    let forward = state.camera.forward_xz();
    state.data_store.insert("camera_forward", forward);
    state.data_store.insert("camera_yaw", state.camera.yaw);

    // ── Sight blockers (v0.975, nameplate-through-wall polish; in three
    // dimensions since the review of ship homes increment 7, ship/sight.rs):
    // the static sight spans (windows open) + live CLOSED opaque doors, each
    // standing between its own heights, + the SOLID floor slabs, translated
    // into the frame the HUD projects labels in (label.pos + station_off).
    // The HUD hides a machine card or crew nameplate whose camera->anchor
    // line meets one, so cards no longer bleed through the house's walls or
    // its floors, and the walk-up target below must be in sight by the same
    // test (built here, ahead of it, for that). Skipped in the build editor:
    // its orbit camera is above the roofline, where the walls and the solid
    // floors would wrongly hide everything.
    state.gui_state.sight_blockers.clear();
    if !state.gui_state.construction_active
        && !state.sight_colliders.is_empty()
        && (!state.gui_state.machine_labels.is_empty()
            || !state.gui_state.crew_labels.is_empty())
    {
        // Engine-side station_off: updated earlier THIS frame (the
        // gui_state copy syncs later, with the crew labels).
        let off = state.station_off;
        let blockers = &mut state.gui_state.sight_blockers;
        blockers.walls.extend(state.sight_colliders.iter().map(|s| s.moved(off)));
        if let Some(ship) = state.gui_state.ship_structure.as_ref() {
            blockers.slabs.extend(humanity_ship::sight::ship_solid_slabs(ship).map(|b| b.moved(off)));
            // The placed boxes and stairs, by their solids (2026-10-22).
            blockers.pieces.extend(humanity_ship::sight::ship_solid_pieces(ship).map(|p| p.moved(off)));
        }
        for (i, (p, open)) in state.door_panels.iter().enumerate() {
            // Same blocking rule as the collision doors, except
            // window PANES never block sight (glass).
            let locked = door_locked_now(p, state.door_locks.get(i));
            if p.is_window || (*open >= 0.5 && !locked) {
                continue;
            }
            // From its sill to its head, as the walk's door: a door shut
            // upstairs hides nothing on the ground floor.
            blockers.walls.push(humanity_ship::wall_collision::door_panel_segment(p.center + off, p.rotation, p.size));
        }
    }

    // Walk-up interaction (v0.431): the machine the player is looking at,
    // nearest within range inside a look-cone. Drives the [E] prompt + card.
    // Since the review of ship homes increment 7 only a machine within the
    // relay's reach, on the player's storey (the home's own storeys, read at
    // the machine's foot and the player's feet) and in sight (no wall across
    // the line at its height, no solid floor between): E used to reach up
    // through the dining room's ceiling and sleep in the bed upstairs. Labels
    // and the ship are home-local; the camera is in the frame the home is
    // drawn at, station_off from it (as the HUD projects).
    {
        let off = state.station_off;
        let feet = state.camera.position.y - state.controller.eye_height();
        let ship = state.gui_state.ship_structure.as_ref();
        state.gui_state.targeted_machine = humanity_ship::sight::walk_up_target(
            state.camera.position,
            feet,
            state.camera.forward(),
            state.gui_state.machine_labels.iter().map(|l| (l.pos + off, l.foot_y + off.y)),
            &state.gui_state.sight_blockers,
            |p, foot| ship.and_then(|s| humanity_ship::sight::storey_floor(s, p - off, foot)).map(|f| f + off.y),
        );
    }

    // Walk-up to a VEHICLE (Stage 3 take-over, v0.690): the nearest
    // Vehicle entity within reach inside the look cone. Drives the
    // "[E] drive X" prompt; E enters, E again exits. While driving,
    // the prompt is a constant "[E] exit vehicle".
    {
        if state.driving_vehicle.is_some() {
            state.targeted_vehicle = None;
            state.gui_state.vehicle_prompt = "[E] exit vehicle".to_string();
        } else {
            let cp = state.camera.position;
            let cf = state.camera.forward();
            let mut best: Option<(hecs::Entity, f32, String)> = None;
            for (e, (_v, tf, name)) in state
                .game_world
                .world
                .query::<(
                    &humanity_core::ecs::components::Vehicle,
                    &humanity_core::ecs::components::Transform,
                    Option<&humanity_core::ecs::components::Name>,
                )>()
                .iter()
            {
                let to = tf.position + Vec3::Y * 1.0 - cp;
                let dist = to.length();
                if !(0.5..=6.0).contains(&dist) {
                    continue;
                }
                if (to / dist).dot(cf) < 0.85 {
                    continue;
                }
                if best.as_ref().map_or(true, |b| dist < b.1) {
                    let label = name.map(|n| n.0.clone()).unwrap_or_else(|| "vehicle".into());
                    best = Some((e, dist, label));
                }
            }
            state.targeted_vehicle = best.as_ref().map(|b| b.0);
            state.gui_state.vehicle_prompt = match &best {
                Some((_, _, label)) => format!("[E] drive {label}"),
                None => String::new(),
            };
        }
    }

    // Walk-up to an ANIMAL (v0.751, ladder rung 7): the nearest
    // Creature within reach inside the look cone. Ready shows
    // "[E] collect Egg (Chicken)"; regrowing shows the countdown.
    {
        let cp = state.camera.position;
        let cf = state.camera.forward();
        let mut best: Option<(hecs::Entity, f32)> = None;
        for (e, (c, tf, _h, dead)) in state
            .game_world
            .world
            .query::<(
                &humanity_core::ecs::components::Creature,
                &humanity_core::ecs::components::Transform,
                Option<&humanity_core::ecs::components::Harvestable>,
                Option<&humanity_core::ecs::components::Dead>,
            )>()
            .iter()
        {
            if dead.is_some() {
                continue; // corpses are not collect/attack targets
            }
            let to = tf.position + Vec3::Y * (c.body_side * 0.8) - cp;
            let dist = to.length();
            if !(0.3..=4.0).contains(&dist) {
                continue;
            }
            // Wider cone than machines: animals are small and moving.
            if (to / dist).dot(cf) < 0.8 {
                continue;
            }
            if best.map_or(true, |b| dist < b.1) {
                best = Some((e, dist));
            }
        }
        state.targeted_livestock = best.map(|b| b.0);
        // The prompt and, for a cared-for animal, how it is and what it
        // needs (2026-10-12, first-hour S6, systems::animal_care::walk_up_lines).
        let (prompt, care) = match best {
            Some((e, _)) => humanity_systems::animal_care::walk_up_lines(&state.game_world.world, e, &state.data_store),
            None => (String::new(), String::new()),
        };
        state.gui_state.livestock_prompt = prompt;
        state.gui_state.livestock_care = care;
    }

    // Walk-up to a CREW NPC (v0.797, operator: "I can't interact
    // with NPCs at all"): the nearest relay-driven crew member
    // within talk range inside the look cone. Drives the
    // "[E] Talk to X" prompt; E opens the dialogue card. Only crew
    // that actually HAVE lines (dialog/greetings from the relay)
    // are offered -- a silent NPC never prompts, keeping the
    // client free of fallback dialogue (infinite-of-X).
    {
        let cp = state.camera.position;
        let cf = state.camera.forward();
        // (entity_id, head-ish anchor) pairs for the pure selection
        // helper + a name lookup for the prompt text. Crew body
        // center sits at ~1.0 m (NPC_LOCAL_STANDING_Y); +0.6 puts
        // the aim point at the head, level with the camera eye.
        let mut cands: Vec<(u64, Vec3)> = Vec::new();
        let mut names: std::collections::HashMap<u64, String> =
            std::collections::HashMap::new();
        for (_e, (tf, npc)) in state
            .game_world
            .world
            .query::<(
                &humanity_core::ecs::components::Transform,
                &crate::net::sync::RemoteNpc,
            )>()
            .iter()
        {
            if npc.dialog.is_empty() && npc.greetings.is_empty() {
                continue; // nothing to say -> no talk prompt
            }
            cands.push((npc.entity_id, tf.position + Vec3::Y * 0.6));
            names.insert(npc.entity_id, npc.name.clone());
        }
        // ~2.5 m talk range; the 0.8 cone matches livestock (crew
        // move, so a tight machine-style cone would flicker).
        let best = crate::net::sync::nearest_facing_target(
            cp, cf, &cands, 0.3, 2.5, 0.8,
        );
        state.gui_state.targeted_npc = best;
        // A faced person outranks the animal behind them: clear the
        // livestock target so the E chain and the prompts agree
        // (the collect prompt would otherwise promise an E the
        // talk branch takes first).
        if best.is_some() {
            state.targeted_livestock = None;
            state.gui_state.livestock_prompt.clear();
            state.gui_state.livestock_care.clear();
        }
        state.gui_state.npc_prompt = if state.gui_state.npc_talk_target.is_some() {
            // Card already open: no crosshair prompt under a modal.
            String::new()
        } else {
            match best.and_then(|id| names.get(&id)) {
                Some(name) => format!("[E] Talk to {name}"),
                None => String::new(),
            }
        };
    }

    // Walk-up to a door you can interact with (v0.567 control panel + v0.570 locks): the
    // nearest within arm's reach that the player is facing. A MANUAL control-panel door
    // (open/close) OR any LOCKED door with locks (unlock) qualifies -- so a locked AUTO
    // door, or a panel-less locked door, can still be unlocked at its lock indicators
    // instead of being a dead-end. Drives the prompt; E unlocks-then-opens.
    {
        let cp_pos = state.camera.position;
        let cf = state.camera.forward();
        let mut best: Option<(usize, f32)> = None;
        if state.camera.mode == crate::renderer::camera::CameraMode::FirstPerson
            && !state.gui_state.construction_active
            && state.gui_state.active_page == GuiPage::None
        {
            let door_locks = state.door_locks.clone();
            for (i, (p, _open)) in state.door_panels.iter().enumerate() {
                if p.is_window {
                    continue;
                }
                let panel_door = p.control_panel && !p.auto_open;
                // A door with locks is interactable while LOCKED (to unlock), and -- if
                // MANUAL -- also once unlocked (its locks double as the open surface, so
                // a manual lock-door needs no separate control panel). An AUTO door drops
                // out once unlocked because it then opens on its own.
                let lock_door = !p.locks.is_empty()
                    && (door_locked_now(p, door_locks.get(i)) || !p.auto_open);
                if !panel_door && !lock_door {
                    continue;
                }
                // Interact point: the control panel if any, else the first lock indicator.
                let ipos = if panel_door {
                    p.control_panel_pos
                } else {
                    p.locks.first().map_or(Vec3::new(p.center.x, 1.2, p.center.z), |l| l.pos)
                };
                let to = ipos - cp_pos;
                let dist = to.length();
                if !(0.1..=2.5).contains(&dist) {
                    continue;
                }
                if (to / dist).dot(cf) < 0.55 {
                    continue; // not facing it (~57-degree cone)
                }
                if best.map_or(true, |b| dist < b.1) {
                    best = Some((i, dist));
                }
            }
        }
        state.gui_state.targeted_control_panel = best.map(|b| b.0);
        // Precompute the crosshair prompt (the HUD can't see the door's open/locked
        // state, which lives here in EngineState). (v0.567)
        state.gui_state.control_panel_prompt = match best.map(|b| b.0) {
            Some(i) => {
                let locked = state
                    .door_panels
                    .get(i)
                    .map_or(false, |p| door_locked_now(&p.0, state.door_locks.get(i)));
                let open = state.door_manual_open.get(i).copied().unwrap_or(false);
                if locked {
                    "[E] unlock door".to_string()
                } else if open {
                    "[E] close door".to_string()
                } else {
                    "[E] open door".to_string()
                }
            }
            None => String::new(),
        };
    }
}
