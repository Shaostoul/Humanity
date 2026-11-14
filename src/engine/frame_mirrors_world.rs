//! The GUI's mirrors of the world (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: the Garden data, asteroids and mining drones, and the
//! mining drone docked at the hangar.

use crate::engine::frame_prelude::*;

/// Mirrors the garden, mining and the drone into the GUI and the draw lists.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars) {
    let dt = f.dt;
    let showroom = f.showroom;
    let hide_machines = f.hide_machines;
    humanity_core::frame_costs::span("cpu.span.mirror_towers");
    // The Garden data the Inventory page shows (crops, pests, weeds, the air) is built only on a
    // frame that draws the Inventory page: engine/garden_mirror.rs (2026-10-30). Here, every frame,
    // only the one-time tower table.
    {
        let plant_reg = state
            .data_store
            .get::<humanity_systems::farming::PlantRegistry>("plant_registry");
        // One-time tower compatibility (operator: "make sure they
        // grow together"): the shared reservoir pH / temperature /
        // humidity window per tower. Static (tower configs + the
        // plant registry), so compute once and cache in GuiState.
        if state.gui_state.tower_compat.is_empty()
            && !state.gui_state.tower_configs.is_empty()
        {
            if let Some(reg) = plant_reg {
                let towers = state.gui_state.tower_configs.clone();
                // The pH each tower's solution is held at (its
                // grow medium in soil_ph.ron), the same setpoint
                // the farming model grows it at.
                let ph = humanity_systems::farming::soil_ph::SoilPhData::load();
                state.gui_state.tower_compat = towers
                    .iter()
                    .map(|t| {
                        let held = ph.medium_for(&t.id).and_then(|m| m.held_ph).map(|p| p as f32);
                        crate::gui::compute_tower_compat(t, reg, held)
                    })
                    .collect();
            }
        }
    }
    humanity_core::frame_costs::span("cpu.span.mirror_mining");
    // Bridge asteroids + active mining drones from ECS for the Mining panel.
    {
        state.gui_state.asteroids.clear();
        for (_e, ast) in state
            .game_world
            .world
            .query::<&humanity_core::ecs::components::AsteroidBody>()
            .iter()
        {
            let p = ast.position;
            let dist = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            state.gui_state.asteroids.push(crate::gui::GuiAsteroid {
                id: ast.id.clone(),
                name: ast.name.clone(),
                classification: ast.classification.clone(),
                ores: ast.ores.iter().map(|(id, q)| (id.clone(), *q)).collect(),
                position: ast.position,
                distance: dist,
            });
        }
        state.gui_state.drones.clear();
        for (_e, drone) in state
            .game_world
            .world
            .query::<&humanity_core::ecs::components::Drone>()
            .iter()
        {
            let dur = drone.phase_duration(drone.phase);
            let phase_progress = if dur > 0.0 {
                (drone.phase_time / dur).clamp(0.0, 1.0)
            } else {
                1.0
            };
            state.gui_state.drones.push(crate::gui::GuiDrone {
                manifest: drone.manifest.clone(),
                phase: format!("{:?}", drone.phase),
                cargo_total: drone.cargo_total(),
                phase_progress,
                target: drone.target.clone(),
                distance: drone.distance(),
                pos: drone.current_pos(),
            });
        }
        // One drone per player: the panel shows the active drone +
        // disables Launch while one is in flight.
        state.gui_state.drone_active = !state.gui_state.drones.is_empty();
        // Keep mining the drone ended by itself (a trip came back
        // empty, or its asteroid is gone) unticks the box to match
        // (first-hour audit 2026-10-04, Friction 5).
        let has_order = humanity_systems::mining::standing_order(&state.data_store).is_some();
        crate::gui::pages::inventory::sync_keep_mining(&mut state.gui_state, has_order);
        // World vehicles (Stage 3, v0.680): name + distance from the
        // player + transit state, for the Inventory Vehicles section.
        state.gui_state.vehicles.clear();
        let cam_pos = state.camera.position;
        for (e, (_v, tf, name, route)) in state
            .game_world
            .world
            .query::<(
                &humanity_core::ecs::components::Vehicle,
                &humanity_core::ecs::components::Transform,
                Option<&humanity_core::ecs::components::Name>,
                Option<&humanity_core::ecs::components::VehicleRoute>,
            )>()
            .iter()
        {
            state.gui_state.vehicles.push(crate::gui::GuiVehicle {
                bits: e.to_bits().into(),
                name: name.map(|n| n.0.clone()).unwrap_or_else(|| "Vehicle".into()),
                distance: (tf.position - cam_pos).length(),
                in_transit: route.is_some(),
            });
        }
        // Live production status lines (v0.681) for the same section.
        if let Some(status) = state
            .data_store
            .get::<std::sync::Mutex<Vec<String>>>("auto_craft_status")
            .and_then(|m| m.lock().ok().map(|s| s.clone()))
        {
            state.gui_state.factory_status = status;
        }
    }
    humanity_core::frame_costs::span("cpu.span.mirror_drone");
    // Mining drone, DOCKED at the hangar (v0.639): the operator's "the homestead
    // should feel like a homestead" ask. `DroneSystem` (systems::mining) is pure
    // simulation with no visual today; this is the first render of it. The model is
    // shown at the "drone_hangar" machine's placement WHENEVER no drone is currently
    // in flight (gui_state.drone_active is the same "one drone per player" flag the
    // Mining panel already uses to grey out Launch) and hidden once a drone undocks
    // (Outbound), reappearing once it delivers + despawns (Done). No new ECS state:
    // purely a function of the existing drone_active flag + the hangar's resolved
    // world placement. This block runs AFTER drone_active is refreshed above (moved
    // here 2026-07-01, an earlier placement before the refresh left a real one-frame-
    // stale window on both launch and return, caught in review), so it is in sync
    // as of the current frame. Hidden in the showroom (avatar-only) and follows the
    // "Machine" declutter toggle, same as the hangar pad it sits on.
    // Docking/undocking SEQUENCE (v0.681.x, operator field test: "the
    // drone did disappear and reappear -- we'll want a proper docking
    // sequence"): instead of popping with drone_active, the dock model
    // animates -- on launch it lifts off the pad over ~2 s and THEN
    // vanishes; on return it appears overhead and settles down onto
    // the pad. drone_dock_anim: 1 = settled on the pad, 0 = departed.
    {
        let target = if state.gui_state.drone_active { 0.0_f32 } else { 1.0_f32 };
        let step = dt * 0.5; // full lift/settle takes ~2 s
        let a = state.drone_dock_anim;
        state.drone_dock_anim = if a < target {
            (a + step).min(target)
        } else {
            (a - step).max(target)
        };
    }
    if crate::drone_dock_visible(showroom, hide_machines, state.drone_dock_anim <= 0.01) {
        if let Some((hangar_pos, hangar_yaw)) = hangar_placement(state) {
            if state.drone_dock_meshes.is_none() {
                let body = state.renderer.add_mesh(Mesh::box_xyz(&state.renderer.device, 0.9, 0.22, 0.55));
                let nose = state.renderer.add_mesh(Mesh::pyramid(&state.renderer.device, 0.5, 0.35));
                let rotor_pod = state.renderer.add_mesh(Mesh::cylinder_capped(&state.renderer.device, 0.12, 0.08, 10));
                state.drone_dock_meshes = Some([body, nose, rotor_pod]);
            }
            if state.drone_dock_mats.is_none() {
                // theme-exempt: placeholder drone hull, neutral gunmetal + dark rotor pods.
                let hull = state.renderer.add_material_typed([0.62, 0.65, 0.68, 1.0], 0.55, 0.35, 0.0);
                let rotor = state.renderer.add_material_typed([0.12, 0.12, 0.14, 1.0], 0.2, 0.6, 0.0);
                state.drone_dock_mats = Some([hull, rotor]);
            }
            let [body_mesh, nose_mesh, pod_mesh] = state.drone_dock_meshes.unwrap();
            let [hull_mat, rotor_mat] = state.drone_dock_mats.unwrap();
            // Rest the body on the pad (box_xyz is y-bottom-origin already) and nose it
            // in the hangar's facing direction (the pyramid points +Y by default, so lay
            // it on its side pointing +X of the hangar's local frame before the yaw).
            let yaw = Quat::from_rotation_y(hangar_yaw.to_radians());
            // The lift: settled (anim 1) = on the pad; departing/arriving
            // rises to +4 m with an ease-in curve before it blinks out.
            let lift = (1.0 - state.drone_dock_anim).powi(2) * 4.0;
            let deck_y = hangar_pos.y + 0.10 + lift;
            let body_pos = Vec3::new(hangar_pos.x, deck_y + 0.11, hangar_pos.z);
            f.all_objects.push(RenderObject { fade: 0.0,
                position: body_pos,
                rotation: yaw,
                scale: Vec3::ONE,
                mesh: body_mesh,
                material: hull_mat,
            });
            let nose_local = Vec3::new(0.55, -0.11, 0.0);
            f.all_objects.push(RenderObject { fade: 0.0,
                position: body_pos + yaw * nose_local,
                rotation: yaw * Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2),
                scale: Vec3::ONE,
                mesh: nose_mesh,
                material: hull_mat,
            });
            const POD_OFFSETS: [(f32, f32); 4] = [(0.4, 0.24), (0.4, -0.24), (-0.4, 0.24), (-0.4, -0.24)];
            for (dx, dz) in POD_OFFSETS {
                let pod_local = Vec3::new(dx, 0.0, dz);
                f.all_objects.push(RenderObject { fade: 0.0,
                    position: body_pos + yaw * pod_local,
                    rotation: yaw,
                    scale: Vec3::ONE,
                    mesh: pod_mesh,
                    material: rotor_mat,
                });
            }
        }
    }

}
