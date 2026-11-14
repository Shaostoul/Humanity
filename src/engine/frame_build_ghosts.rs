//! Build mode's previews (the file-size split, step 1, 2026-11-14), moved out of `window_event` in
//! src/lib.rs as they were: the placement ghost of the held palette item, the wall-drawing tool's
//! preview, and which build gizmo the cursor hovers this frame.

use crate::engine::frame_prelude::*;

/// Draws build mode's previews and finds the hovered gizmo.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars) {
    // Placement ghost: the held palette item, previewed (semi-transparent, faintly
    // glowing) on the room floor under the cursor, so you see where a click drops it.
    // The ghost mesh is cached + rebuilt only when the held type changes. (v0.529)
    if state.gui_state.construction_active {
        if let Some(mtype) = state.gui_state.construction_place_type.clone() {
            let need = state
                .construction_ghost
                .as_ref()
                .map_or(true, |(t, _, _)| t != &mtype);
            if need {
                let def = state
                    .gui_state
                    .home_machines
                    .as_ref()
                    .and_then(|h| h.catalog.get(&mtype))
                    .cloned();
                if let Some(def) = def {
                    let mesh =
                        machine_mesh(&state.renderer.device, &def.shape, def.size);
                    let color = [def.color.0, def.color.1, def.color.2, 0.45];
                    // Reuse the prior ghost slot so switching held type does not
                    // leak a mesh+material each time. (v0.531)
                    let slot =
                        state.construction_ghost.as_ref().map(|(_, m, mt)| (*m, *mt));
                    let (mesh_idx, mat) = if let Some((mi, ma)) = slot {
                        state.renderer.replace_mesh(mi, mesh);
                        state.renderer.update_material_typed(ma, color, 0.1, 0.6, 0.4);
                        (mi, ma)
                    } else {
                        let mi = state.renderer.add_mesh(mesh);
                        let ma = state.renderer.add_material_typed(color, 0.1, 0.6, 0.4);
                        (mi, ma)
                    };
                    state.construction_ghost = Some((mtype.clone(), mesh_idx, mat));
                }
            }
            let ghost = state.construction_ghost.as_ref().map(|(_, m, mt)| (*m, *mt));
            if let Some((mesh_idx, mat)) = ghost {
                // On the floor of the storey being edited (ship homes increment 7).
                if let Some(StoreyFloorHit { x: hx, y: floor_y, z: hz, .. }) = cursor_storey_hit(state) {
                    f.transparent_objects.push(RenderObject { fade: 0.0,
                        position: Vec3::new(hx, floor_y, hz),
                        rotation: Quat::IDENTITY,
                        scale: Vec3::ONE,
                        mesh: mesh_idx,
                        material: mat,
                    });
                }
            }
        } else {
            state.construction_ghost = None;
        }
        // STRUCTURE placement ghost (v0.583): the held structural piece, previewed
        // translucent on the floor under the cursor with the current placement yaw.
        // The key encodes type + yaw so rotating ([ / ]) rebuilds the cached mesh.
        if let Some(tid) = state.gui_state.construction_structure_type.clone() {
            let yaw = state.gui_state.construction_structure_yaw;
            let key = format!("{tid}@{:.0}", yaw);
            let need = state
                .construction_structure_ghost
                .as_ref()
                .map_or(true, |(k, _, _)| k != &key);
            if need {
                if let Some(ty) = humanity_ship::structure::structure_type(&tid) {
                    let (verts, indices) =
                        humanity_ship::structure::structure_mesh(ty, Vec3::ZERO, yaw.to_radians());
                    let mesh = Mesh::from_vertices(&state.renderer.device, &verts, &indices);
                    let color = [ty.color.0, ty.color.1, ty.color.2, 0.45];
                    let slot = state
                        .construction_structure_ghost
                        .as_ref()
                        .map(|(_, m, mt)| (*m, *mt));
                    let (mesh_idx, mat) = if let Some((mi, ma)) = slot {
                        state.renderer.replace_mesh(mi, mesh);
                        state.renderer.update_material_typed(ma, color, 0.1, 0.6, 0.4);
                        (mi, ma)
                    } else {
                        let mi = state.renderer.add_mesh(mesh);
                        let ma = state.renderer.add_material_typed(color, 0.1, 0.6, 0.4);
                        (mi, ma)
                    };
                    state.construction_structure_ghost = Some((key.clone(), mesh_idx, mat));
                }
            }
            let ghost = state.construction_structure_ghost.as_ref().map(|(_, m, mt)| (*m, *mt));
            if let Some((mesh_idx, mat)) = ghost {
                if let Some(StoreyFloorHit { x: hx, y: floor_y, z: hz, .. }) = cursor_storey_hit(state) {
                    // Show the ghost at the place-height (v0.588) so you see where a deck
                    // lands at an upper level; a faint riser line marks the lift. On the
                    // floor of the storey being edited (ship homes increment 7).
                    let py = floor_y + state.gui_state.construction_structure_place_y.max(0.0);
                    if py > floor_y + 0.01 {
                        crate::renderer::line::push_polyline(
                            &mut f.ring_lines,
                            &[[hx, floor_y, hz], [hx, py, hz]],
                            [0.45, 0.85, 1.0, 0.6],
                        );
                    }
                    f.transparent_objects.push(RenderObject { fade: 0.0,
                        position: Vec3::new(hx, py, hz),
                        rotation: Quat::IDENTITY,
                        scale: Vec3::ONE,
                        mesh: mesh_idx,
                        material: mat,
                    });
                }
            }
        } else {
            state.construction_structure_ghost = None;
        }
    }

    // Wall-drawing tool preview (v0.534): a corner-node marker under the cursor and,
    // once the first corner is set, a translucent preview wall from that corner to the
    // cursor -- so you see the segment before clicking the second corner. Uses one
    // cached unit-box mesh, scaled/rotated per frame (no per-frame allocation).
    if state.gui_state.construction_active && state.gui_state.construction_wall_mode {
        if state.wall_tool_mesh.is_none() {
            let m = state.renderer.add_mesh(Mesh::box_xyz(&state.renderer.device, 1.0, 1.0, 1.0));
            state.wall_tool_mesh = Some(m);
        }
        if state.wall_tool_mat.is_none() {
            // theme-exempt: translucent editor overlay, not a themed surface.
            let ma = state.renderer.add_material_typed([0.25, 0.8, 1.0, 0.5], 0.1, 0.6, 0.4);
            state.wall_tool_mat = Some(ma);
        }
        let mesh = state.wall_tool_mesh.unwrap();
        let mat = state.wall_tool_mat.unwrap();
        // On the plan of the storey being edited (ship homes increment 7), even
        // where a new floor has no room yet.
        if let Some(StoreyFloorHit { x: hx, y: floor_y, z: hz, .. }) = cursor_storey_hit(state) {
            // Corner-node marker: a slim post where the next click lands. (box_xyz is
            // y-bottom-origin, so position at floor_y -> the post spans [floor, +3].)
            f.transparent_objects.push(RenderObject { fade: 0.0,
                position: Vec3::new(hx, floor_y, hz),
                rotation: Quat::IDENTITY,
                scale: Vec3::new(0.3, 3.0, 0.3),
                mesh,
                material: mat,
            });
            // Preview wall from the pending start corner (zone-LOCAL -> world by the
            // active zone origin, v0.754) to the cursor (already world).
            if let Some((sx, sz)) = state.gui_state.construction_wall_start {
                let zo = active_zone_origin(state);
                let a = Vec3::new(sx + zo.x, floor_y, sz + zo.z);
                let b = Vec3::new(hx, floor_y, hz);
                let dx = b.x - a.x;
                let dz = b.z - a.z;
                let len = (dx * dx + dz * dz).sqrt();
                if len > 0.05 {
                    // As tall as the storey it stands on (ship homes increment 7).
                    let height = edit_storey(state).map_or(3.0, |es| es.storey.height());
                    let dir = Vec3::new(dx, 0.0, dz) / len;
                    let rot = Quat::from_rotation_arc(Vec3::X, dir);
                    // box_xyz is y-bottom-origin -> position at floor_y so the preview
                    // wall fills [floor, floor+height].
                    f.transparent_objects.push(RenderObject { fade: 0.0,
                        position: Vec3::new((a.x + b.x) * 0.5, floor_y, (a.z + b.z) * 0.5),
                        rotation: rot,
                        scale: Vec3::new(len, height, 0.15),
                        mesh,
                        material: mat,
                    });
                }
            }
        }
        // The FLOOR-SLAB tool's rectangle (ship homes increment 7): from its first
        // corner to the cursor on the storey's plan, and the same outline at the
        // height the slab will go when that is another (a floor drawn above).
        if let Some((pts, top_y)) = slab_draw_preview(state) {
            const SLAB_TOOL: [f32; 4] = [0.25, 0.8, 1.0, 0.95];
            let ring: Vec<[f32; 3]> = pts.iter().chain(pts.first()).map(|p| [p.x, p.y, p.z]).collect();
            crate::renderer::line::push_polyline(&mut f.ring_lines, &ring, SLAB_TOOL);
            if (top_y - pts[0].y).abs() > 0.01 {
                let lid: Vec<[f32; 3]> = pts.iter().chain(pts.first()).map(|p| [p.x, top_y, p.z]).collect();
                crate::renderer::line::push_polyline(&mut f.ring_lines, &lid, SLAB_TOOL);
                for p in &pts {
                    crate::renderer::line::push_polyline(&mut f.ring_lines, &[[p.x, p.y, p.z], [p.x, top_y, p.z]], [0.25, 0.8, 1.0, 0.4]);
                }
            }
        }
    }

    // Which build-mode gizmo the cursor is hovering this frame (v0.569) -- drives the
    // hover highlight on the corner orbs, the opening cubes, and the avatar pyramid.
    let hover = compute_construction_hover(state);

    // Corner-node gizmos (v0.541): a bright pin above each wall corner in BUILD MODE.
    // Click + drag one to reposition the corner (walls sharing it move together) with
    // snapping; idle -> hover -> active (grabbed) by colour. Cached sphere mesh + three
    // materials, reused -- no per-frame allocation.
    if state.gui_state.construction_active && state.gui_state.ship_structure.is_some() {
        if state.construction_node_mesh.is_none() {
            let m = state.renderer.add_mesh(Mesh::sphere(&state.renderer.device, 1.0, 12, 16));
            state.construction_node_mesh = Some(m);
        }
        if state.construction_node_mat.is_none() {
            // theme-exempt: editor gizmo overlay, emissive so it stands out.
            let m = state.renderer.add_material_full([1.0, 0.82, 0.2, 1.0], 0.0, 0.4, 0.0, 0.6);
            state.construction_node_mat = Some(m);
        }
        if state.construction_node_mat_hot.is_none() {
            // theme-exempt: grabbed-gizmo highlight.
            let m = state.renderer.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.3, 0.0, 1.0);
            state.construction_node_mat_hot = Some(m);
        }
        if state.construction_node_mat_hover.is_none() {
            // theme-exempt: hover highlight -- brighter/whiter than idle, calmer than the
            // active RGB cycle (idle yellow -> hover cream -> active RGB).
            let m = state.renderer.add_material_full([1.0, 0.96, 0.62, 1.0], 0.0, 0.35, 0.0, 0.95);
            state.construction_node_mat_hover = Some(m);
        }
        let node_mesh = state.construction_node_mesh.unwrap();
        let node_mat = state.construction_node_mat.unwrap();
        let hot_mat = state.construction_node_mat_hot.unwrap();
        let hover_mat = state.construction_node_mat_hover.unwrap();
        // RGB-cycle the ACTIVE (grabbed/selected) gizmo material like the menu header
        // buttons (v0.562). Only HOT gizmos use this material, so static ones keep
        // their colour. door_anim_time advances each build-mode frame (with doors).
        let hue = (state.door_anim_time * 0.25).rem_euclid(1.0);
        let (rr, gg, bb) = hsv_rgb(hue, 0.85, 1.0);
        state.renderer.update_material_full(hot_mat, [rr, gg, bb, 1.0], 0.0, 0.3, 0.0, 1.2);
        let grabbed = state.construction_node_grab;
        // The ACTIVE zone's origin shifts every gizmo into world space (v0.754).
        let zo = active_zone_origin(state);
        // The corners of the storey being edited, on its floor (ship homes
        // increment 7, engine/editor_storeys.rs `storey_wall_corners`).
        let (corners, floor) = {
            let hs = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone).unwrap();
            let es = EditStorey::of(hs, state.gui_state.construction_storeys.chosen);
            (storey_wall_corners(hs, es.index), zo.y + es.storey.floor)
        };
        let slab_grab = state.gui_state.construction_storeys.corner_grab;
        for c in &corners {
            // A slab corner's drag rides the node grab: only a wall corner is hot then.
            let hot = slab_grab.is_none() && grabbed.map_or(false, |g| (g.0 - c.0).abs() < 0.05 && (g.1 - c.1).abs() < 0.05);
            let hovered = hover == HoverGizmo::Corner(c.0, c.1);
            let r = 0.05; // operator: orbs at 0.05 m; state shown by COLOUR (active = RGB), not size
            // The orb's TOP touches the wall-corner BASE (operator note): centre at -r
            // so the top vertex is at the floor. Overlay pass -> visible through walls
            // + the floor it sits under. (v0.560). Idle -> hover -> active by colour (v0.569).
            f.overlay_objects.push(RenderObject { fade: 0.0,
                position: Vec3::new(c.0 + zo.x, floor - r, c.1 + zo.z),
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(r),
                mesh: node_mesh,
                material: if hot { hot_mat } else if hovered { hover_mat } else { node_mat },
            });
            // Ground angle-circle (v0.568): a constant-width LINE circle (line::push_circle
            // into ring_lines, like the orbit paths) instead of a polygon ring whose band
            // thickened with radius. Radius 1.1 matches the overlay's RING_R; the per-slice
            // angle labels are painted separately by the egui overlay.
            crate::renderer::line::push_circle(
                &mut f.ring_lines, [c.0 + zo.x, floor + 0.1, c.1 + zo.z], 1.1, [0.30, 0.85, 1.0, 0.85], 48,
            );
        }
        // The SELECTED FLOOR SLAB's corner handles (ship homes increment 7): drag one
        // to resize the slab (`try_grab_slab_corner`); the grabbed one shifts RGB.
        for (c, p) in slab_corner_handles(state) {
            let hot = slab_grab.is_some_and(|(_, gc)| gc == c);
            f.overlay_objects.push(RenderObject { fade: 0.0,
                position: p,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.09),
                mesh: node_mesh,
                material: if hot { hot_mat } else { hover_mat },
            });
        }
        // Wall-SELECT orbs (v0.573): a RED sphere at each wall's bottom-middle. Click
        // anywhere on a wall (its surface or this orb) to select it -- unambiguous at a
        // multi-wall intersection. The SELECTED wall's orb uses the RGB hot material, so
        // it shifts colour like the header menu buttons.
        if state.construction_wall_mat.is_none() {
            // theme-exempt: wall-select gizmo, red emissive.
            let m = state.renderer.add_material_full([1.0, 0.2, 0.2, 1.0], 0.0, 0.4, 0.0, 0.8);
            state.construction_wall_mat = Some(m);
        }
        let wall_mat = state.construction_wall_mat.unwrap();
        let sel_wall = state.gui_state.construction_wall_selected;
        // The walls of the storey being edited, each at its own base (ship homes
        // increment 7).
        let wall_mids: Vec<(usize, f32, f32, f32)> =
            zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone)
            .map(|h| {
                let es = EditStorey::of(h, state.gui_state.construction_storeys.chosen);
                let storeys = h.storeys();
                h.walls
                    .iter()
                    .enumerate()
                    .filter(|(_, w)| humanity_ship::home_structure::storey_index(&storeys, w.base) == es.index)
                    .map(|(i, w)| (i, (w.a.0 + w.b.0) * 0.5 + zo.x, zo.y + w.base, (w.a.1 + w.b.1) * 0.5 + zo.z))
                    .collect()
            })
            .unwrap_or_default();
        for (i, mx, my, mz) in &wall_mids {
            let selected = sel_wall == Some(*i);
            f.overlay_objects.push(RenderObject { fade: 0.0,
                position: Vec3::new(*mx, *my - 0.07, *mz), // orb top at the floor base
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.07),
                mesh: node_mesh,
                material: if selected { hot_mat } else { wall_mat },
            });
        }
        // Highlight the selected machine with a ground ring (v0.553) -> also a LINE circle
        // now (v0.568), in the RGB active colour so it pulses like the header buttons.
        if let Some(sel_id) = &state.gui_state.construction_machine_selected {
            if let Some((_, center, radius)) =
                state.machine_pick.iter().find(|(mid, _, _)| mid == sel_id)
            {
                crate::renderer::line::push_circle(
                    &mut f.ring_lines, [center.x, 0.12, center.z], radius + 0.2, [rr, gg, bb, 1.0], 48,
                );
            }
        }
    }

    f.hover = hover;
}
