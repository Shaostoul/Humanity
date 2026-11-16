//! Other players and the showroom ground (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as they were: the remote players' bodies, the showroom's ground,
//! the solar-system hologram and the planet-pin raycast.

use crate::engine::frame_prelude::*;

/// Draws the players around you, the showroom ground and the hologram.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars) {
    let dt = state.clock.dt;
    let showroom = f.showroom;
    // ── Remote players (multiplayer co-presence, v0.472) ──
    // Draw a simple humanoid marker (body + head, a distinct teal) at each remote
    // player's interpolated position. The sent position is their eye; the figure
    // stands on the floor 1.7 m below it and is built UP from the feet, body box
    // to the shoulders, head on top (engine::net_route::remote_figure_parts,
    // 2026-10-02). The name floats over it (net_route::nameplate_labels).
    // Since character animation rung 1 (2026-11-08) every person is a skinned,
    // animated body (engine/animation.rs: the player in third person, the other
    // players, the crew, the showroom figure, the dev crowd); the box figures
    // below stand in only when the character data did not load, or for the A/B
    // switch (showcase `figures`: `boxes`).
    // On a planet the player's own body is already in render space: it joins the
    // scene list AFTER the station shift below, as `planet_ghost` does (rung 2a).
    f.people_render_space = Vec::new();
    let bodies_drawn = crate::engine::animation::draw_people(state, &mut f.all_objects, &mut f.people_render_space, &mut f.transparent_objects, &mut f.overlay_objects, showroom, dt);
    // Each door's swing this frame, for the door hand's "is it opening" (rung 2b).
    crate::engine::interact::note_doors(state);
    if !showroom && !bodies_drawn {
        if state.remote_avatar.is_none() {
            // Sized by the constants remote_figure_parts places the parts by
            // (2026-10-02): the box's base at its origin, the sphere centred on
            // it, so a change to either size moves the parts with it. The head
            // (and the hair, the same mesh stretched) is figure_head_mesh_data:
            // the engine's sphere at the finer 16 x 24 the hair needs.
            use crate::engine::net_route::{FIGURE_BODY_MESH_H_M, FIGURE_HEAD_MESH_R_M};
            let gpu = gpu!(state);
            let body = gpu.renderer.add_mesh(
                Mesh::box_xyz(&gpu.renderer.device, 0.42, FIGURE_BODY_MESH_H_M, 0.26));
            let (hv, hi) = crate::engine::net_route::figure_head_mesh_data(FIGURE_HEAD_MESH_R_M);
            let gpu = gpu!(state);
            let head = gpu.renderer.add_mesh(Mesh::from_vertices(&gpu.renderer.device, &hv, &hi));
            // Teal, slightly emissive so a remote player reads at a glance.
            let mat = gpu!(state).renderer.add_material_full(
                [0.15, 0.75, 0.85, 1.0], 0.0, 0.5, 1.0, 0.25);
            state.remote_avatar = Some((body, head, mat));
        }
        if let Some((body, head, mat)) = state.remote_avatar {
            // Each figure wears its player's look when their
            // client sent one (2026-09-29): head in their skin
            // tone, a cap of hair, sized by their height; the
            // body stays teal, the marker that says "a player".
            for (_e, (t, r)) in state
                .game_world
                .world
                .query::<(&humanity_core::ecs::components::Transform, &crate::net::sync::RemotePlayer)>()
                .iter()
            {
                // Someone we blocked is not drawn (engine/block.rs hides_player).
                if crate::engine::block::hides_player(&state.gui_state, r) {
                    continue;
                }
                let look_mats = r.look.as_ref().map(|l| {
                    let key = crate::engine::net_route::look_material_key(l);
                    *state.remote_look_materials.entry(key).or_insert_with(|| {
                        let skin = gpu!(state).renderer.add_material_full(
                            [l.skin[0], l.skin[1], l.skin[2], 1.0], 0.0, 0.6, 1.0, 0.1);
                        let hair = gpu!(state).renderer.add_material_full(
                            [l.hair[0], l.hair[1], l.hair[2], 1.0], 0.0, 0.8, 1.0, 0.0);
                        (skin, hair)
                    })
                });
                for p in crate::engine::net_route::remote_figure_parts(t.position, t.rotation, r.look.as_ref()) {
                    let (m, material) = match (p.part, look_mats) {
                        (crate::engine::net_route::FigurePart::Body, _) => (body, mat),
                        (crate::engine::net_route::FigurePart::Head, Some((skin, _))) => (head, skin),
                        (crate::engine::net_route::FigurePart::Head, None) => (head, mat),
                        (crate::engine::net_route::FigurePart::Hair, Some((_, hair))) => (head, hair),
                        (crate::engine::net_route::FigurePart::Hair, None) => continue,
                    };
                    f.all_objects.push(RenderObject { fade: 0.0,
                        position: p.at,
                        rotation: p.rotation,
                        scale: p.scale,
                        mesh: m,
                        material,
                    });
                }
            }
        }
        // ── Crew NPCs (relay chore AI, v0.663) ──
        // The same figure as a player's, amber instead of teal, at each
        // crew member's interpolated position: standing on the floor, head
        // on the shoulders (net_route::crew_figure_parts, 2026-10-03; their
        // own fixed offsets floated the feet 0.7 m and sank the head into
        // the body). It reuses the player figure's meshes, built just above.
        if state.remote_npc_avatar.is_none() {
            if let Some((body, head, _)) = state.remote_avatar {
                // Amber, slightly emissive, so crew read distinctly from
                // the teal remote players at a glance.
                let mat = gpu!(state).renderer.add_material_full(
                    [0.92, 0.62, 0.18, 1.0], 0.0, 0.55, 1.0, 0.22);
                state.remote_npc_avatar = Some((body, head, mat));
            }
        }
        if let Some((body, head, mat)) = state.remote_npc_avatar {
            for (_e, (t, _n)) in state
                .game_world
                .world
                .query::<(&humanity_core::ecs::components::Transform, &crate::net::sync::RemoteNpc)>()
                .iter()
            {
                for p in crate::engine::net_route::crew_figure_parts(t.position, t.rotation) {
                    let m = if p.part == crate::engine::net_route::FigurePart::Body { body } else { head };
                    f.all_objects.push(RenderObject { fade: 0.0,
                        position: p.at,
                        rotation: p.rotation,
                        scale: p.scale,
                        mesh: m,
                        material: mat,
                    });
                }
            }
        }
    }
    // Showroom ground under the avatar (tinted by the backdrop): a planet
    // SPHERE the avatar stands on for body backdrops (Earth/Mars/Moon), else
    // a flat disc. The sphere (radius 30) is centered 30 below the avatar so
    // its top is the standing surface; you see the surface curve away. (v0.449)
    if showroom {
        if let Some((gm, gmat)) = state.showroom_ground {
            let is_sphere = state
                .showroom_backdrops
                .get(state.gui_state.showroom_backdrop)
                .map(|b| b.sphere)
                .unwrap_or(false);
            if is_sphere {
                if let Some(bm) = state.showroom_body {
                    f.all_objects.push(RenderObject { fade: 0.0,
                        position: state.avatar_base - Vec3::new(0.0, 30.0, 0.0),
                        rotation: Quat::IDENTITY,
                        scale: Vec3::ONE,
                        mesh: bm,
                        material: gmat,
                    });
                }
            } else {
                f.all_objects.push(RenderObject { fade: 0.0,
                    position: state.avatar_base,
                    rotation: Quat::IDENTITY,
                    scale: Vec3::ONE,
                    mesh: gm,
                    material: gmat,
                });
            }
        }
    }

    // Solar system hologram centered in the designated hologram room (1m above floor)
    let hologram_center = state.hologram_room_center;

    // Hologram (solar-system map) is part of the home, so hide it in the
    // showroom too (it was leaking into the void when panning, v0.443).
    if !showroom {
        // Orbit rings (centered on hologram)
        for &(mesh_idx, mat_idx) in &state.hologram_orbits {
            f.all_objects.push(RenderObject { fade: 0.0,
                position: hologram_center,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                mesh: mesh_idx,
                material: mat_idx,
            });
        }

        // Planet bodies
        for (mesh_idx, mat_idx, local_pos, _name) in &state.hologram_objects {
            f.all_objects.push(RenderObject { fade: 0.0,
                position: hologram_center + *local_pos,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                mesh: *mesh_idx,
                material: *mat_idx,
            });
        }

        // Pin markers above each planet
        for (mesh_idx, mat_idx, local_pos, _name) in &state.hologram_pins {
            f.all_objects.push(RenderObject { fade: 0.0,
                position: hologram_center + *local_pos,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                mesh: *mesh_idx,
                material: *mat_idx,
            });
        }
    }

    // Raycast from camera to detect which planet pin is targeted
    {
        let ray_origin = state.camera.position;
        let ray_dir = state.camera.forward();
        let pin_hit_radius = 0.06; // slightly larger than pin head for easy targeting
        let mut closest_hit: Option<(f32, String)> = None;

        for (_mesh_idx, _mat_idx, local_pos, name) in &state.hologram_pins {
            let pin_world = hologram_center + *local_pos;
            // Sphere-ray intersection with pin head center
            let oc = ray_origin - pin_world;
            let b = oc.dot(ray_dir);
            let c = oc.dot(oc) - pin_hit_radius * pin_hit_radius;
            let discriminant = b * b - c;
            if discriminant >= 0.0 {
                let t = -b - discriminant.sqrt();
                if t > 0.0 {
                    if closest_hit.as_ref().map_or(true, |(d, _)| t < *d) {
                        closest_hit = Some((t, name.clone()));
                    }
                }
            }
        }

        state.targeted_planet = closest_hit.map(|(_, name)| name);
    }

}
