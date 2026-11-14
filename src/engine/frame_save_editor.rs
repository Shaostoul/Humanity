//! Saving and the construction editor (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: paid machines removed since the last frame, the
//! showroom's stage, double-click focus, the construction editor's edits applied, the undo
//! history, the picker's Back, and the cursor's grab.

use crate::engine::frame_prelude::*;

/// Applies saves and construction edits, then sets the cursor for this frame.
pub(crate) fn run(state: &mut EngineState) {
    humanity_core::frame_costs::span("cpu.span.save_editor");
    // A paid machine removed by any path since the last frame gives its item
    // back (engine/own_home.rs), before the save below and before the editor's
    // history tick, so neither ever sees a payment for a machine that is gone.
    crate::engine::own_home::refund_removed_machines(state);
    // Periodic auto-save of the offline home (v0.381). Self-throttles
    // to every 2 minutes; robust to any exit path (in-app quit, crash)
    // where the graceful close-save would not fire. The character's own
    // home goes in as it stands (engine/own_home.rs).
    if crate::save_load::periodic_save_due(120) {
        crate::engine::own_home::refresh_own_home(&mut state.gui_state);
        crate::save_load::save_active_home(
            &mut state.game_world.world,
            &state.gui_state.placed_items,
            &state.data_store,
            !state.gui_state.settings.fresh_world_each_launch,
            state.gui_state.own_home.as_ref(),
        );
    }
    // Ship-structure autosave (v0.791): build edits used to persist
    // ONLY through the explicit Save button; this + the close flush
    // make walls/lights/strips/corridors as durable as inventory.
    autosave_ship_structure(state, false);

    // Settings > Gameplay live wiring (v0.791).
    // 1) Vitals-drain scale: publish the persisted slider to the slot
    //    the food system reads each tick.
    if let Some(m) = state
        .data_store
        .get::<std::sync::Mutex<f32>>("vitals_drain_scale")
    {
        if let Ok(mut g) = m.lock() {
            *g = state.gui_state.settings.vitals_drain;
        }
    }
    // 2) Hostile wildlife: flipping OFF despawns wild hostiles
    //    immediately. Transition-triggered on purpose -- creatures the
    //    operator spawns deliberately from the Dev page while the
    //    setting is off must survive.
    {
        use std::sync::atomic::{AtomicU8, Ordering};
        static PREV_HOSTILE: AtomicU8 = AtomicU8::new(u8::MAX);
        let cur = (crate::gui::gameplay_levels::value_of(&state.gui_state.settings.gameplay, "hostile_wildlife") > 0.0) as u8;
        let prev = PREV_HOSTILE.swap(cur, Ordering::Relaxed);
        if prev == 1 && cur == 0 {
            let doomed: Vec<hecs::Entity> = state
                .game_world
                .world
                .query::<(
                    &humanity_core::ecs::components::Creature,
                    &humanity_core::ecs::components::AIBehavior,
                )>()
                .iter()
                .filter(|(_, (_, ai))| {
                    matches!(ai.behavior_type.as_str(), "predator" | "aggressive")
                })
                .map(|(e, _)| e)
                .collect();
            let n = doomed.len();
            for e in doomed {
                let _ = state.game_world.world.despawn(e);
            }
            if n > 0 {
                log::info!("Hostile wildlife off: despawned {n} hostile(s)");
            }
        }
    }

    // The character stands in the middle of the stage between the picker's
    // columns, not of the window (2026-11-02, docs/design/top-menu-and-game-pages.md
    // section 5): the camera's lens is shifted by the stage's offset, from the
    // panel's last frame; 0 whenever the showroom is closed.
    state.camera.lens_shift_x = if state.gui_state.showroom_active { crate::gui::pages::showroom::stage_shift() } else { 0.0 };
    // ── Character-select showroom sync (v0.441): apply the panel's edits ──
    // Rebuild the avatar when appearance changed (it is the tail of
    // placeholder_objects, so truncate to its start + re-place).
    if state.gui_state.appearance_dirty || state.gui_state.outfit_dirty {
        state.gui_state.appearance_dirty = false;
        state.gui_state.outfit_dirty = false;
        state.placeholder_objects.truncate(state.avatar_obj_start);
        let base = state.avatar_base;
        let app = state.gui_state.appearance.clone();
        let colors = humanity_core::cosmetics::resolve_outfit_colors(
            &state.gui_state.outfit,
            &state.cosmetics,
        );
        place_avatar(state, base, &app, &colors);
        state.camera.orbit_target =
            base + Vec3::new(0.0, 0.9 * app.height_scale, 0.0);
    }
    // Rebuild the ground material when the backdrop changed.
    if state.gui_state.showroom_active
        && state.gui_state.showroom_backdrop != state.showroom_last_backdrop
    {
        state.showroom_last_backdrop = state.gui_state.showroom_backdrop;
        if let Some(bd) =
            state.showroom_backdrops.get(state.gui_state.showroom_backdrop)
        {
            let gmat = state.renderer.add_material_typed(
                [bd.ground.0, bd.ground.1, bd.ground.2, 1.0],
                0.1,
                0.9,
                0.0,
            );
            if let Some((gm, _)) = state.showroom_ground {
                state.showroom_ground = Some((gm, gmat));
            }
        }
    }
    // Astral-projection camera (v0.464): when the construction editor opens, lift
    // into a free ORBIT camera around the home (drag to orbit, middle-drag to
    // pan, wheel to dolly, WASD to fly the focal point, Space/Shift up/down for
    // levels). Reconciled here (not in the B handler) so the panel's Close button
    // restores first person too.
    if state.gui_state.construction_active && !state.construction_cam_active {
        state.construction_cam_active = true;
        state.controller.showroom_lock = false; // full orbit controls, not the fixed showroom orbit
        state.construction_return_pos = state.camera.position;
        // The first time, the build-mode avatar goes where the player stands, on
        // their storey, and the storey selector opens there (v0.557, increment 7).
        crate::engine::editor::seed_build_avatar(state);
        let (_center, size) = state.homestead_bounds
            .map(|(mn, mx)| ((mn + mx) * 0.5, (mx - mn).length()))
            .unwrap_or((Vec3::new(0.0, 1.5, 0.0), 20.0));
        state.camera.switch_mode(crate::renderer::camera::CameraMode::Orbit);
        // Focus the build cam on WHERE THE PLAYER IS (v0.582, operator) -- the player's
        // spot when B was pressed, at roughly chest height -- so you start editing near
        // yourself, not the home centre. Start zoomed in close; you can dolly out to the
        // whole home (distance_max scales with the box).
        // At chest height over the feet, so on an upper floor the build cam opens
        // on that storey, not on the ground under it (ship homes increment 7).
        let focus = state.construction_return_pos;
        state.camera.orbit_target = Vec3::new(focus.x, focus.y - state.controller.eye_height() + 1.2, focus.z);
        state.camera.orbit_distance = 12.0;
        state.camera.orbit_distance_max = (size * 4.0).max(400.0);
    } else if !state.gui_state.construction_active && state.construction_cam_active {
        state.construction_cam_active = false;
        state.camera.switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
        // Spawn at the build-mode avatar (v0.557) -- "where I'm at" when I leave build
        // mode, in the EDITED zone (its origin shifts the avatar to world, v0.754).
        // Fall back to the home zone's saved spawn, then the pre-build position.
        // In the shared world, a pick more than 90 m from where the relay holds the
        // player (the pre-build position, or where a welcome stood them while the
        // editor was open), or off their own plot, leaves them there instead
        // (home_plot.rs `editor_close_spot`, ship homes 1b and 4); a pick it takes is
        // declared as the editor's in the next update, so the relay passes the jump.
        // The avatar stands on the chosen storey, and only where a person can stand
        // (no glass lid, no opening): else the nearest place that is, or where they
        // stood, said in a notice (ship homes increment 7, `editor_close_pick`).
        let (chosen, pick_note) = crate::engine::home_plot::editor_close_pick(state);
        let close = crate::engine::home_plot::editor_close_spot(
            chosen,
            state.construction_return_pos,
            state.game_joined,
            state.gui_state.ship_structure.as_ref().is_some_and(|s| s.home_is_away()),
            state.gui_state.ship_structure.as_ref().and_then(|s| s.home_plot()).map(|p| p.aabb()),
        );
        if state.game_joined && close.at != state.construction_return_pos {
            crate::engine::move_check::declare(state, humanity_ship::moves::MoveDecl::Editor);
        }
        state.camera.position = close.at;
        crate::engine::home_plot::reseat_floor(state);
        if close.held_back {
            state.gui_state.pending_notices.push(
                crate::engine::home_plot::EDITOR_HELD_BACK.to_string(),
            );
        } else if let Some(note) = pick_note {
            state.gui_state.pending_notices.push(note);
        }
        state.gui_state.construction_selected_room = None;
        state.construction_grab = None;
        state.construction_gizmo_grab = None;
        state.construction_node_grab = None; // v0.542: drop a live corner grab on close
        state.construction_object_grab = None; // v0.593: drop a live object grab
        state.construction_opening_grab = None; // v0.546: drop a live opening grab
        state.construction_opening_resize = None; // v0.578: drop a live resize handle
        state.construction_char_grab = false; // v0.557: drop a live avatar grab
        state.construction_grab_press = None;
    }
    // Double-click-to-focus (v0.593): a left-list row asked the camera to snap to an
    // object so you can see what you clicked. Re-aim the orbit target + zoom in if far.
    if state.gui_state.construction_active {
        if let Some((fx, fy, fz)) = state.gui_state.construction_focus_request.take() {
            state.camera.orbit_target = Vec3::new(fx, fy, fz);
            state.camera.orbit_distance = state.camera.orbit_distance.min(9.0);
        }
    }
    // 3D room drag (v0.466): a grabbed room follows the cursor on its floor.
    // Slide-gizmo drag (v0.468) takes precedence: a grabbed door/window handle
    // slides along its wall; a grabbed room follows the cursor on its floor.
    // Gated on the editor being open (v0.542): a grab MUST NOT keep dragging in
    // first-person -- a Close click consumes the mouse-release, so without this gate a
    // live node grab silently rewrote walls to chase the cursor after leaving the editor.
    if state.gui_state.construction_active {
        // Only DRAG while the left button is physically held (v0.593): `lmb_held` is set
        // from raw input before the egui-consumed gate, so if a mouse-up is consumed by a
        // side panel mid-drag (the release-clear is skipped), a grab can no longer keep
        // chasing the cursor -- it just stops. Fixes a latent stuck-grab across every grab
        // type, made easier to hit now that objects are draggable across the floor. (The
        // cursor-floor tracking below still runs every move so the dimension overlay lives.)
        if state.lmb_held {
            if state.construction_char_grab {
                apply_char_drag(state); // v0.557: a grabbed avatar follows the cursor floor
            } else if state.construction_opening_resize.is_some() {
                apply_opening_resize(state); // v0.578: a grabbed resize handle resizes the opening
            } else if state.construction_opening_grab.is_some() {
                apply_opening_drag(state); // v0.546: a grabbed opening slides along its wall
            } else if state.construction_node_grab.is_some() {
                apply_node_drag(state); // v0.541: a grabbed wall corner follows the cursor
            } else if state.construction_object_grab.is_some() {
                apply_object_drag(state); // v0.593: a grabbed light/machine/structure follows the cursor
            } else if state.construction_gizmo_grab.is_some() {
                apply_gizmo_drag(state);
            } else {
                apply_room_drag(state);
            }
        }
        // Track the cursor's floor position for the dimension overlay (v0.545),
        // in the ACTIVE zone's LOCAL coords (v0.754 -- the overlay adds the zone
        // origin back when painting, and wall_start is local too). While DRAWING a
        // wall, snap the preview to the same grid/endpoint/edge the placed node
        // will use, so the selector visibly snaps (v0.559 -- it tracked the raw cursor).
        let azo = active_zone_origin(state);
        // On the plan of the storey being edited (ship homes increment 7).
        let hit = cursor_storey_hit(state).map(|h| (h.x - azo.x, h.z - azo.z));
        state.gui_state.construction_cursor_world = hit.map(|(hx, hz)| {
            if state.gui_state.construction_wall_mode {
                if let Some(hs) = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
                    let grabbed =
                        state.gui_state.construction_wall_start.unwrap_or((f32::NAN, f32::NAN));
                    return snap_node_position(hs, grabbed, (hx, hz), state.gui_state.construction_grid_snap);
                }
            }
            (hx, hz)
        });
    }
    // Construction editor (v0.455/459): apply the edited walls + ceiling height
    // AND room position/size + add/remove to the live layout, then rebuild.
    // Reconcile by ID (both directions) so add/remove/reorder can't desync.
    if state.gui_state.construction_dirty {
        state.gui_state.construction_dirty = false;
        let rooms = state.gui_state.construction_rooms.clone();
        let height = state.gui_state.construction_height;
        if let Some(layout) = &mut state.homestead_layout {
            // "Pin" seeds Some([0,0,0]); turn that into the room's CURRENT
            // resolved (computed) position so pinning freezes-in-place rather
            // than teleporting to the origin.
            let resolved = humanity_ship::fibonacci::resolve_positions(layout);
            let resolved_by_id: std::collections::HashMap<String, Vec3> = layout
                .rooms
                .iter()
                .enumerate()
                .map(|(i, rc)| (rc.id.clone(), resolved[i]))
                .collect();

            // 1. Drop layout rooms the editor removed.
            let keep: std::collections::HashSet<&str> =
                rooms.iter().map(|r| r.id.as_str()).collect();
            layout.rooms.retain(|rc| keep.contains(rc.id.as_str()));

            // 2. Upsert each editor room (patch by id, else append a new one).
            for er in &rooms {
                let mut pos = er.position;
                if pos == Some([0.0, 0.0, 0.0]) {
                    // freshly pinned: snap to where it visually sits now.
                    if let Some(r) = resolved_by_id.get(&er.id) {
                        pos = Some([r.x, r.y, r.z]);
                    }
                }
                // Map the editor's placed openings to engine Openings (v0.469).
                let er_openings: Vec<humanity_ship::fibonacci::Opening> = er.openings.iter().map(|eo| {
                    use crate::gui::EditorOpeningKind as EK;
                    use humanity_ship::fibonacci::OpeningKind as OK;
                    humanity_ship::fibonacci::Opening {
                        wall: eo.wall as u8,
                        kind: match eo.kind { EK::Door => OK::Door, EK::Airlock => OK::Airlock, EK::Window => OK::Window },
                        u: eo.u, v: eo.v, w: eo.w, h: eo.h, profile: None,
                    }
                }).collect();
                if let Some(rc) = layout.rooms.iter_mut().find(|r| r.id == er.id) {
                    rc.walls.north = er.walls[0];
                    rc.walls.south = er.walls[1];
                    rc.walls.west = er.walls[2];
                    rc.walls.east = er.walls[3];
                    rc.walls.offsets = er.wall_offsets;
                    rc.openings = er_openings;
                    rc.level = er.level;
                    rc.position = pos;
                    rc.dimensions = er.dimensions;
                } else {
                    layout.rooms.push(humanity_ship::fibonacci::RoomConfig {
                        id: er.id.clone(),
                        position: pos,
                        dimensions: er.dimensions,
                        material_type: er.material_type,
                        color: er.color,
                        wall_height: 0.0, // global default_wall_height owns Y
                        walls: humanity_ship::fibonacci::WallSet {
                            north: er.walls[0],
                            south: er.walls[1],
                            west: er.walls[2],
                            east: er.walls[3],
                            offsets: er.wall_offsets,
                        },
                        openings: er_openings,
                        level: er.level,
                    });
                }
            }

            // 3. Reorder layout.rooms to the editor order (stable spiral attach).
            let order: std::collections::HashMap<&str, usize> =
                rooms.iter().enumerate().map(|(i, r)| (r.id.as_str(), i)).collect();
            layout.rooms.sort_by_key(|rc| *order.get(rc.id.as_str()).unwrap_or(&usize::MAX));

            layout.default_wall_height = height;
        }
        // Reflect any pin-seeded positions back into the editor mirror so the
        // DragValues show the real coordinates next frame.
        if let Some(layout) = &state.homestead_layout {
            for er in state.gui_state.construction_rooms.iter_mut() {
                if er.position == Some([0.0, 0.0, 0.0]) {
                    if let Some(rc) = layout.rooms.iter().find(|r| r.id == er.id) {
                        er.position = rc.position;
                    }
                }
            }
        }
        rebuild_homestead(state);
    }
    // Undo-history tick (v0.575): checkpoint at the dirty-flag choke point, BEFORE the
    // rebuild blocks below consume the flags. Coalesces a drag into one undo step.
    {
        let edited = state.gui_state.construction_structure_dirty
            || state.gui_state.construction_machines_dirty;
        construction_history_tick(state, edited);
        // Arm the autosave (v0.791): any structure/machine edit means there
        // is unsaved ship state. The 60 s autosave + window-close flush
        // persist it; the explicit Save button also clears it. The rebuild the
        // editor asks for as it opens is no edit (engine/editor.rs `arms_autosave`).
        let entry = std::mem::take(&mut state.construction_entry_rebuild);
        if crate::engine::editor::arms_autosave(edited, entry) {
            state.gui_state.construction_unsaved = true;
            // An edit outside the Dev mode makes the home the character's own:
            // from now on every save records it (engine/own_home.rs).
            if !crate::engine::own_home::authors_data_files(state.gui_state.settings.play_mode) {
                state.gui_state.own_home_live = true;
            }
        }
    }
    // Machine-only edit (offset / add / remove / connect): refresh just the machine
    // meshes so the change shows live, without a full room rebuild. (v0.525)
    if state.gui_state.construction_machines_dirty {
        state.gui_state.construction_machines_dirty = false;
        rebuild_machine_objects(state);
    }
    // Pipe markings (2026-10-04): Settings or a showcase pin changed the mode.
    crate::engine::pipe_markers::rebuild_if_mode_changed(state);
    crate::engine::home_reuse::frame(state); // the reuse switch off: the home rebuilt on its schedule
    // Interior-wall edit (v0.534): the editor mutated gui_state.home_structure
    // (added/removed a wall, moved a corner, changed an opening). Rebuild the home
    // mesh live so the change shows immediately; persistence waits for Save.
    if state.gui_state.construction_structure_dirty {
        state.gui_state.construction_structure_dirty = false;
        rebuild_homestead(state);
    }
    // The editor's Save buttons: "Save home" asks for both halves (the structure and
    // the machines), the legacy room editor's "Save layout" and "Save machines" one
    // each. WHERE they go is the play mode's (engine/own_home.rs, 2026-10-04): the
    // Dev mode writes the data files the AI and the editor share (increment 1a: the
    // home design always, the ship file and the ship's machines with
    // ship editing, offline only (config::ship_editing_for), the household's machines); Normal and Creative keep the
    // character's own home in their save and write no data file at all.
    let save_structure = std::mem::take(&mut state.gui_state.construction_save);
    let save_machines = std::mem::take(&mut state.gui_state.home_machines_save);
    if save_structure || save_machines {
        if state.gui_state.ship_structure.is_some() {
            if save_structure {
                // Persist the build-mode SPAWN point with the EDITED zone (v0.582) so
                // the moved avatar survives the save (was lost -- spawn lived only in
                // GuiState). build_char_pos is zone-local, exactly what body.spawn holds.
                // Only an avatar moved onto the ground floor (`spawn_from_avatar`).
                let spawn = crate::engine::editor::spawn_from_avatar(&state.gui_state);
                if let (Some(spawn), Some(hs)) = (spawn, zone_body_mut(&mut state.gui_state.ship_structure, state.gui_state.construction_zone)) {
                    hs.spawn = Some(spawn);
                }
                let ship = state.gui_state.ship_structure.as_mut().unwrap();
                // Drop corridor rows that no longer resolve (a referenced door/zone was
                // edited away) BEFORE writing: validate() rejects a whole file over one
                // bad corridor, so a saved file must always re-load. Live editing keeps
                // broken rows visible (the Corridors panel shows why); save is the
                // deliberate moment they are let go. (Increment B.)
                let pruned = ship.prune_invalid_corridors();
                if pruned > 0 {
                    log::warn!("Construction: dropped {pruned} corridor(s) whose openings no longer exist or align");
                }
            }
            match crate::engine::own_home::keep_edits_now(state) {
                Ok(note) => {
                    log::info!("Construction: {note}");
                    state.gui_state.construction_save_note = note;
                }
                Err(e) => {
                    log::warn!("Construction: save failed: {e}");
                    state.gui_state.construction_save_note = format!("Save FAILED: {e}");
                }
            }
        } else if !crate::engine::own_home::authors_data_files(state.gui_state.settings.play_mode) {
            // The legacy fallback layout (the ship did not assemble) has no home
            // design of the player's to keep, and outside Dev nothing is written
            // to the data files.
            state.gui_state.construction_save_note = "Not kept: outside the Dev mode the fallback layout's \
                                 changes are not saved (your home did not load from its design)."
                .to_string();
        } else {
            if save_structure {
                if let Some(layout) = &state.homestead_layout {
                    match humanity_ship::fibonacci::save_layout(layout) {
                        Ok(()) => log::info!("Construction: layout saved to RON"),
                        Err(e) => log::warn!("Construction: save failed: {e}"),
                    }
                }
            }
            // Machine layout save (v0.519): the editor's machine panel edits
            // gui_state.home_machines; this writes it back to home.ron (the same
            // file the AI edits -- home-design parity).
            if save_machines {
                if let Some(home) = &state.gui_state.home_machines {
                    let path = humanity_systems::persistence::live_save::active_home_machines_path(&state.data_dir);
                    match home.save(&path) {
                        Ok(()) => {
                            log::info!("Construction: machine layout saved to {}", path.display());
                            state.gui_state.construction_save_note =
                                "Saved home structure + machines.".to_string();
                        }
                        Err(e) => {
                            log::warn!("Construction: machine save failed: {e}");
                            state.gui_state.construction_save_note =
                                format!("Machine save FAILED: {e}");
                        }
                    }
                }
            }
        }
    }
    // Picker "Back": cancel the showroom and return to the menu WITHOUT
    // entering the world (same as Esc in the picker). Mirrors the Esc
    // handler so the visible Back button and the Esc key agree. (v0.476.1)
    if state.gui_state.showroom_cancel {
        state.gui_state.showroom_cancel = false;
        let was_picker = state.gui_state.showroom_mode == 0;
        state.gui_state.showroom_active = false;
        state.gui_state.showroom_confirm = false;
        state.gui_state.showroom_return_to_picker = false;
        state.controller.showroom_lock = false;
        state.camera.switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
        state.camera.position = state.showroom_return_pos;
        if was_picker {
            state.gui_state.leave_world();
        }
    }
    // Enter: commit the WHO/WHERE pairing. Persist appearance + emerge
    // into first-person, and remember the pairing so Play repeats it.
    if state.gui_state.showroom_confirm {
        state.gui_state.showroom_confirm = false;
        state.gui_state.showroom_active = false;
        state.gui_state.showroom_return_to_picker = false;
        // SOLO vs SHARED intent (v0.801, operator: "I tried to join my
        // offline world from the character select but that didn't work"):
        // a RED offline home means solo - no game_join, no avatar in the
        // shared world, and the Dev travel tools stay usable. A server
        // world means shared. The co-presence gate reads this flag;
        // flipping to solo while joined sends game_leave.
        state.gui_state.copresence_solo =
            state.gui_state.launcher_where_kind == crate::gui::LauncherWhere::Home;
        // Automatic last pairing (docs/design/play-characters.md): every
        // successful Enter is what Play repeats, so there is no default
        // toggle to find and no dead end to escape.
        state.gui_state.record_pairing();
        crate::config::AppConfig::from_gui_state(&state.gui_state).save();
        let app = state.gui_state.appearance.clone();
        let outfit = state.gui_state.outfit.clone();
        let cname = if state.gui_state.character_name.trim().is_empty() {
            "Wanderer".to_string()
        } else {
            state.gui_state.character_name.clone()
        };
        for (_e, (n, a, o, _c)) in state.game_world.world.query_mut::<(
            &mut humanity_core::ecs::components::Name,
            &mut humanity_core::ecs::components::Appearance,
            &mut humanity_core::ecs::components::Outfit,
            &Controllable,
        )>() {
            n.0 = cname.clone();
            *a = app.clone();
            *o = outfit.clone();
            break;
        }
        crate::engine::own_home::refresh_own_home(&mut state.gui_state);
        crate::save_load::save_active_home(
            &mut state.game_world.world,
            &state.gui_state.placed_items,
            &state.data_store,
            !state.gui_state.settings.fresh_world_each_launch,
            state.gui_state.own_home.as_ref(),
        );
        state.controller.showroom_lock = false;
        state
            .camera
            .switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
        state.camera.position = state.showroom_return_pos;
        // (cursor re-grabbed by the per-frame reconciliation below)
    }

    // Cursor: free for any menu page / showroom / construction editor (so egui
    // clicks land), grabbed for first-person play. Single authority (v0.460).
    reconcile_cursor(state);

    // Build render objects from homestead meshes
}
