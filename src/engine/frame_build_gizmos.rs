//! Build mode's light and door-mouth gizmos (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as they were: each placed light's centre marker and range rings,
//! and the corridor door-mouth handles.

use crate::engine::frame_prelude::*;

/// Draws the placed lights' gizmos and the corridor door-mouth handles.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars) {
    // Nothing to draw into before the world's renderer is built (a pages-only start, a phone:
    // engine/renderer_slot.rs): build mode's gizmos.
    if !gpu_ref!(state).renderer.world_built() {
        return;
    }
    // Placed-LIGHT gizmos (v0.572): a DIAMOND centre-marker + an RGB range "sphere"
    humanity_core::frame_costs::span("cpu.span.lists_fixtures");
    // ── PHYSICAL LIGHT FIXTURES (v0.780) ── every placed light gets visible
    // geometry in the world, play mode AND build mode (operator: "none of the
    // lights have physical bulbs, they're all just empty points"). Cached UNIT
    // meshes scaled per light: warm_lamp = bulb sphere, panels = thin ceiling
    // panel, spot = bulb + snout tube along its aim, strip = a bar of the
    // type's length_m along the light's horizontal dir (real multi-point strip
    // paths are the next increment). ON fixtures glow with the light's color
    // (emissive, cached per quantized color); OFF fixtures are dark housings.
    if state.gui_state.ship_structure.is_some() && !state.gui_state.showroom_active {
        if state.light_fixture_sphere.is_none() {
            let gpu = gpu!(state);
            let m = gpu.renderer.add_mesh(Mesh::sphere(&gpu.renderer.device, 1.0, 10, 14));
            state.light_fixture_sphere = Some(m);
        }
        if state.light_fixture_panel.is_none() {
            let gpu = gpu!(state);
            let m = gpu.renderer.add_mesh(Mesh::box_xyz(&gpu.renderer.device, 1.0, 1.0, 1.0));
            state.light_fixture_panel = Some(m);
        }
        if state.light_fixture_tube.is_none() {
            // Unit tube along +X, length 1, radius 1: scale.x = length,
            // scale.y/z = radius; rotate X onto the strip/aim axis.
            let gpu = gpu!(state);
            let m = gpu.renderer.add_mesh(Mesh::tube(
                &gpu.renderer.device,
                Vec3::new(-0.5, 0.0, 0.0),
                Vec3::new(0.5, 0.0, 0.0),
                1.0,
                12,
            ));
            state.light_fixture_tube = Some(m);
        }
        if state.light_fixture_mat_off.is_none() {
            // theme-exempt: dark fixture housing for a switched-off light.
            let m = gpu!(state).renderer.add_material_full([0.16, 0.16, 0.18, 1.0], 0.2, 0.6, 0.0, 0.0);
            state.light_fixture_mat_off = Some(m);
        }
        let sphere = state.light_fixture_sphere.unwrap();
        let panel = state.light_fixture_panel.unwrap();
        let tube = state.light_fixture_tube.unwrap();
        let mat_off = state.light_fixture_mat_off.unwrap();
        // Collect (pos, dir, color, on, kind, length, type_id-is-panel) for
        // EVERY zone's lights, world-shifted; then resolve materials (the
        // material cache needs &mut renderer, so collect first).
        struct Fx {
            pos: Vec3,
            dir: Vec3,
            color: (f32, f32, f32),
            on: bool,
            kind: crate::renderer::light::LightKind,
            length: f32,
            flat_panel: bool,
            /// Strip control path in WORLD coords (pos first); empty
            /// = the old straight bar. (v0.781)
            path: Vec<Vec3>,
            /// Corner subdivision (v0.792): 0 = sharp, N = samples per span.
            subdivision: u32,
            /// (zone idx, light idx) - the strip mesh cache key.
            key: (usize, usize),
        }
        let fixtures: Vec<Fx> = state
            .gui_state
            .ship_structure
            .as_ref()
            .map(|s| {
                s.zones
                    .iter()
                    .enumerate()
                    .flat_map(|(zi, z)| {
                        let o = z.origin_vec();
                        z.body.lights.iter().enumerate().filter_map(move |(li, l)| {
                            let t = crate::renderer::light::light_type(&l.type_id)?;
                            let pos = Vec3::new(l.pos.0, l.pos.1, l.pos.2) + o;
                            let path: Vec<Vec3> = if l.path.is_empty() {
                                Vec::new()
                            } else {
                                std::iter::once(pos)
                                    .chain(l.path.iter().map(|p| Vec3::new(p.0, p.1, p.2) + o))
                                    .collect()
                            };
                            Some(Fx {
                                pos,
                                dir: Vec3::new(l.dir.0, l.dir.1, l.dir.2),
                                color: l.color.unwrap_or(t.color),
                                on: l.on,
                                kind: t.kind,
                                length: t.length_m.max(0.3),
                                flat_panel: l.type_id.contains("panel"),
                                path,
                                subdivision: l.subdivision,
                                key: (zi, li),
                            })
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        for fx in fixtures {
            use crate::renderer::light::LightKind;
            let mat = if fx.on {
                let key = [
                    (fx.color.0.clamp(0.0, 1.0) * 255.0) as u8,
                    (fx.color.1.clamp(0.0, 1.0) * 255.0) as u8,
                    (fx.color.2.clamp(0.0, 1.0) * 255.0) as u8,
                ];
                match state.light_fixture_mats.get(&key) {
                    Some(&m) => m,
                    None => {
                        // theme-exempt: fixture glow IS the light's own color.
                        let m = gpu!(state).renderer.add_material_full(
                            [fx.color.0, fx.color.1, fx.color.2, 1.0],
                            0.0,
                            0.4,
                            0.0,
                            2.5,
                        );
                        state.light_fixture_mats.insert(key, m);
                        m
                    }
                }
            } else {
                mat_off
            };
            match fx.kind {
                LightKind::Bar if fx.path.len() >= 2 => {
                    // REAL strip (v0.781): a world-space tube through the
                    // control path -- sharp mitered corners at subdivision
                    // 0, a Catmull-Rom curve with N samples per span above
                    // (v0.792). Mesh cached per (zone, light) and rebuilt
                    // only when the quantized path/subdivision hash changes.
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    fx.subdivision.hash(&mut h);
                    for p in &fx.path {
                        ((p.x * 1000.0).round() as i64).hash(&mut h);
                        ((p.y * 1000.0).round() as i64).hash(&mut h);
                        ((p.z * 1000.0).round() as i64).hash(&mut h);
                    }
                    let hash = h.finish();
                    let cached = state.light_strip_meshes.get(&fx.key).copied();
                    let mesh_idx = match cached {
                        Some((mi, mh)) if mh == hash => mi,
                        _ => {
                            let pts = crate::renderer::light::sample_strip_path(
                                &fx.path, fx.subdivision,
                            );
                            let m = Mesh::polytube(&gpu!(state).renderer.device, &pts, 0.035, 10);
                            let mi = match cached {
                                Some((mi, _)) => {
                                    gpu!(state).renderer.replace_mesh(mi, m);
                                    mi
                                }
                                None => gpu!(state).renderer.add_mesh(m),
                            };
                            state.light_strip_meshes.insert(fx.key, (mi, hash));
                            mi
                        }
                    };
                    // World-space mesh: identity transform.
                    f.all_objects.push(RenderObject { fade: 0.0,
                        position: Vec3::ZERO,
                        rotation: Quat::IDENTITY,
                        scale: Vec3::ONE,
                        mesh: mesh_idx,
                        material: mat,
                    });
                }
                LightKind::Bar => {
                    // Pathless strip: the classic straight bar along the
                    // light's horizontal dir (fall back to X).
                    let flat = Vec3::new(fx.dir.x, 0.0, fx.dir.z);
                    let axis = if flat.length_squared() > 1e-4 {
                        flat.normalize()
                    } else {
                        Vec3::X
                    };
                    f.all_objects.push(RenderObject { fade: 0.0,
                        position: fx.pos,
                        rotation: Quat::from_rotation_arc(Vec3::X, axis),
                        scale: Vec3::new(fx.length, 0.035, 0.035),
                        mesh: tube,
                        material: mat,
                    });
                }
                LightKind::Spot => {
                    // Bulb + a short snout tube along the aim direction.
                    let aim = fx.dir.normalize_or_zero();
                    let aim = if aim == Vec3::ZERO { Vec3::NEG_Y } else { aim };
                    f.all_objects.push(RenderObject { fade: 0.0,
                        position: fx.pos,
                        rotation: Quat::IDENTITY,
                        scale: Vec3::splat(0.07),
                        mesh: sphere,
                        material: mat,
                    });
                    f.all_objects.push(RenderObject { fade: 0.0,
                        position: fx.pos + aim * 0.1,
                        rotation: Quat::from_rotation_arc(Vec3::X, aim),
                        scale: Vec3::new(0.2, 0.05, 0.05),
                        mesh: tube,
                        material: mat,
                    });
                }
                _ if fx.flat_panel => {
                    // Ceiling/cool panel: a thin flat luminous slab.
                    f.all_objects.push(RenderObject { fade: 0.0,
                        position: fx.pos,
                        rotation: Quat::IDENTITY,
                        scale: Vec3::new(0.6, 0.03, 0.6),
                        mesh: panel,
                        material: mat,
                    });
                }
                _ => {
                    // Lamp/point: a bulb sphere.
                    f.all_objects.push(RenderObject { fade: 0.0,
                        position: fx.pos,
                        rotation: Quat::IDENTITY,
                        scale: Vec3::splat(0.09),
                        mesh: sphere,
                        material: mat,
                    });
                }
            }
        }
    }

    // (three axis great-circles -- X red, Y green, Z blue) at each placed light, so the
    // operator (and an AI) can see where each light sits + how far it reaches. Build mode.
    if state.gui_state.construction_active && state.gui_state.ship_structure.is_some() {
        if state.construction_light_mesh.is_none() {
            let gpu = gpu!(state);
            let m = gpu.renderer.add_mesh(Mesh::octahedron(&gpu.renderer.device, 1.0));
            state.construction_light_mesh = Some(m);
        }
        if state.construction_light_mat.is_none() {
            // theme-exempt: light gizmo marker, emissive so it reads at a glance.
            let m = gpu!(state).renderer.add_material_full([1.0, 0.95, 0.6, 1.0], 0.0, 0.3, 0.0, 1.2);
            state.construction_light_mat = Some(m);
        }
        let dmesh = state.construction_light_mesh.unwrap();
        let dmat = state.construction_light_mat.unwrap();
        // The selected light's diamond uses the RGB hot material (created by the corner
        // block, which runs first under the same condition), so it shifts colour. (v0.576)
        let hot_light = state.construction_node_mat_hot.unwrap();
        let sel_light = state.gui_state.construction_light_selected;
        // Resolve each light's range + (for a SPOT) its aim direction + cone half-angle,
        // so a spotlight draws a CONE instead of the omni range sphere (v0.582, operator).
        // ACTIVE zone's lights (the editable set), shifted to world by its origin (v0.754).
        let lzo = active_zone_origin(state);
        // Not the lights of the storeys the cut-away hides (ship homes increment 7).
        let cut = edit_storey(state);
        let lights: Vec<(usize, Vec3, f32, Option<(Vec3, f32)>)> =
            zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone)
            .map(|h| {
                h.lights
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| cut.map_or(true, |es| es.shows(l.pos.1)))
                    .map(|(i, l)| {
                        let t = crate::renderer::light::light_type(&l.type_id);
                        let range = l.range.or_else(|| t.map(|t| t.range)).unwrap_or(4.0);
                        let spot = t
                            .filter(|t| t.kind == crate::renderer::light::LightKind::Spot)
                            .map(|t| {
                                let d = Vec3::new(l.dir.0, l.dir.1, l.dir.2).normalize_or_zero();
                                let d = if d == Vec3::ZERO { Vec3::NEG_Y } else { d };
                                (d, t.cone_outer_deg.max(1.0).to_radians())
                            });
                        (i, Vec3::new(l.pos.0, l.pos.1, l.pos.2) + lzo, range, spot)
                    })
                    .collect()
            })
            .unwrap_or_default();
        // Which rotation ring the cursor hovers, recomputed EVERY frame from the
        // SAME pick test a click runs (light_ring_under_cursor), so the ring that
        // brightens is exactly the ring a press would grab. While a ring drag is
        // live the grabbed axis stays highlighted instead (idle -> hover ->
        // active, like the header buttons); any other drag suppresses hover so a
        // light being MOVED never flashes its rings in passing. (v0.792)
        state.gui_state.construction_light_ring_hover = match &state.construction_object_grab {
            Some(ObjectGrab::LightAim { axis, .. }) => Some(*axis),
            Some(_) => None,
            // A held placement tool / wall draw takes the click before the ring
            // pick in the press chain, so hovering must not promise a grab then
            // (the same guards compute_construction_hover applies).
            None if state.gui_state.construction_wall_mode
                || state.gui_state.construction_place_type.is_some()
                || state.gui_state.construction_place_light.is_some()
                || state.gui_state.construction_structure_type.is_some() => None,
            None => light_ring_under_cursor(state),
        };
        // The range wireframe's colour comes from the theme's muted text token
        // (a passive readout should read quieter than the RGB handles), faded
        // further by alpha since 480 edges of it wrap the whole light.
        let range_wire_color = {
            let c = state.theme.text_muted();
            [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, 0.35]
        };
        for (i, pos, range, spot) in &lights {
            // Diamond centre marker (overlay -> visible through walls); RGB if selected.
            f.overlay_objects.push(RenderObject { fade: 0.0,
                position: *pos,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(if sel_light == Some(*i) { 0.16 } else { 0.12 }),
                mesh: dmesh,
                material: if sel_light == Some(*i) { hot_light } else { dmat },
            });
            let p = [pos.x, pos.y, pos.z];
            if let Some((dir, half)) = spot {
                // SPOT: a cone gizmo -- the base circle at `range` + edge lines from the
                // apex, warm yellow, using the reusable line primitive.
                let base_c = *pos + *dir * *range;
                let base_r = (*range) * half.tan();
                const COL: [f32; 4] = [1.0, 0.9, 0.4, 0.8];
                crate::renderer::line::push_circle_3d(&mut f.ring_lines, base_c.into(), base_r, (*dir).into(), COL, 32);
                let seed = if dir.x.abs() > 0.9 { Vec3::Y } else { Vec3::X };
                let u = seed.cross(*dir).normalize();
                let v = dir.cross(u);
                for k in 0..8 {
                    let a = (k as f32 / 8.0) * std::f32::consts::TAU;
                    let edge = base_c + (u * a.cos() + v * a.sin()) * base_r;
                    crate::renderer::line::push_polyline(&mut f.ring_lines, &[p, edge.into()], COL);
                }
            }
            // ROTATION rings: three axis circles R/G/B at a VIEWPORT-FIXED size
            // (v0.792, operator: "I'd rather the rings be for rotate") -- world
            // radius = camera distance * a fixed angle via light_ring_radius, the
            // SAME function the pick + hover tests use, so every light wears the
            // same predictable on-screen handles instead of the old range-sized
            // circles that crossed half the room. On the SELECTED light they are
            // live handles (grab one to swing the aim about that axis; see
            // try_pick_light_ring) and the hovered ring draws brighter + doubled
            // for thickness; on unselected lights they are a passive location cue.
            // The RANGE now reads from the wireframe icosphere below.
            let rr = light_ring_radius(state.camera.position, *pos);
            let hover = state.gui_state.construction_light_ring_hover;
            let rings: [(Vec3, [f32; 4]); 3] = [
                (Vec3::X, [1.0, 0.30, 0.30, 0.7]),
                (Vec3::Y, [0.35, 1.0, 0.35, 0.7]),
                (Vec3::Z, [0.45, 0.55, 1.0, 0.7]),
            ];
            for (ai, (axis, base)) in rings.into_iter().enumerate() {
                if sel_light == Some(*i) && hover == Some(ai as u8) {
                    // Hovered/active handle: lerp the ring colour toward white
                    // (computed highlight of the axis colour, like the RGB hot
                    // material) and sandwich the rim with two extra circles so
                    // it reads thicker with a 1-px line primitive.
                    let hot = [
                        base[0] + (1.0 - base[0]) * 0.6,
                        base[1] + (1.0 - base[1]) * 0.6,
                        base[2] + (1.0 - base[2]) * 0.6,
                        1.0,
                    ];
                    crate::renderer::line::push_circle_3d(&mut f.ring_lines, p, rr, axis.into(), hot, 40);
                    crate::renderer::line::push_circle_3d(&mut f.ring_lines, p, rr * 1.05, axis.into(), hot, 40);
                    crate::renderer::line::push_circle_3d(&mut f.ring_lines, p, rr * 0.95, axis.into(), hot, 40);
                } else {
                    crate::renderer::line::push_circle_3d(&mut f.ring_lines, p, rr, axis.into(), base, 40);
                }
            }
            // RANGE readout (v0.792, operator: "maybe an icosphere subdivided
            // twice?"): the SELECTED light's falloff radius drawn as a muted
            // wireframe sphere -- point lights, strips and spotlights alike --
            // replacing the old range-sized rings so range and rotation no
            // longer share one confusing gizmo.
            if sel_light == Some(*i) {
                for (a, b) in range_sphere_edges() {
                    crate::renderer::line::push_polyline(
                        &mut f.ring_lines,
                        &[(*pos + *a * *range).into(), (*pos + *b * *range).into()],
                        range_wire_color,
                    );
                }
            }
        }
        // STRIP control-point handles (v0.790, operator: "add the widget/gizmo to
        // the light strip sections so I don't have to adjust the sliders"): when
        // the SELECTED light is a strip (Bar), a small diamond marks each path
        // point so it can be grabbed in the viewport (see try_pick_strip_point).
        // `pos` itself already wears the big selected diamond above -- dragging
        // that one is StripPoint 0 -- so only the path tail needs extra handles.
        if let Some(sel) = sel_light {
            let strip_pts: Vec<Vec3> =
                zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone)
                    .and_then(|h| h.lights.get(sel))
                    .filter(|l| {
                        crate::renderer::light::light_type(&l.type_id).map(|t| t.kind)
                            == Some(crate::renderer::light::LightKind::Bar)
                    })
                    .map(|l| l.path.iter().map(|p| Vec3::new(p.0, p.1, p.2) + lzo).collect())
                    .unwrap_or_default();
            for p in strip_pts {
                f.overlay_objects.push(RenderObject { fade: 0.0,
                    position: p,
                    rotation: Quat::IDENTITY,
                    scale: Vec3::splat(0.11),
                    mesh: dmesh,
                    material: hot_light,
                });
            }
        }
    }

    // CORRIDOR DOOR-MOUTH handles (v0.790, operator: "I don't see any widgets/gizmos
    // on the door to actually move the door"): an accent diamond floats over EACH
    // mouth of every corridor that resolves; grab one to slide the corridor along
    // the wall (its world `lat` -- the same field the panel's Lat DragValue edits;
    // see try_pick_corridor_mouth). A broken row draws nothing here -- the Corridors
    // panel already shows the honest reason it cannot generate.
    if state.gui_state.construction_active && state.gui_state.ship_structure.is_some() {
        if state.construction_light_mesh.is_none() {
            let gpu = gpu!(state);
            let m = gpu.renderer.add_mesh(Mesh::octahedron(&gpu.renderer.device, 1.0));
            state.construction_light_mesh = Some(m);
        }
        if state.construction_corridor_mouth_mat.is_none() {
            // Accent-toned emissive (the theme's accent, like the opening move cube)
            // so a corridor handle reads "grabbable" apart from the warm light gizmos.
            let a = state.theme.accent();
            let m = gpu!(state).renderer.add_material_full(
                [a.r() as f32 / 255.0, a.g() as f32 / 255.0, a.b() as f32 / 255.0, 1.0],
                0.0, 0.3, 0.0, 1.2,
            );
            state.construction_corridor_mouth_mat = Some(m);
        }
        let dmesh = state.construction_light_mesh.unwrap();
        let mmat = state.construction_corridor_mouth_mat.unwrap();
        let mouths: Vec<Vec3> = state
            .gui_state
            .ship_structure
            .as_ref()
            .map(|s| {
                s.corridors
                    .iter()
                    .filter_map(|c| s.corridor_geometry(c).ok())
                    .flat_map(|g| [g.end_from, g.end_to])
                    .collect()
            })
            .unwrap_or_default();
        for p in mouths {
            f.overlay_objects.push(RenderObject { fade: 0.0,
                // Lifted off the deck so the handle hangs in the doorway, not in the
                // floor slab; the pick fn uses the SAME lift so you grab what you see.
                position: p + Vec3::Y * CORRIDOR_MOUTH_HANDLE_LIFT,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.14),
                mesh: dmesh,
                material: mmat,
            });
        }
    }

}
