//! The home's draw lists (the file-size split, step 1, 2026-11-14), moved out of `window_event` in
//! src/lib.rs as they were: the frame's draw lists taken from `EngineState::draw_bufs` (into
//! `FrameVars`), the home's opaque and glass objects, and the placeholder objects (the whole home,
//! or only the avatar).

use crate::engine::frame_prelude::*;

/// Takes this frame's draw lists and fills them with the home.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars) {
    let dt = f.dt;
    humanity_core::frame_costs::span("cpu.span.lists_home");
    f.all_objects = crate::engine::state::DrawBufs::take(&mut state.draw_bufs.opaque); // last frame's memory (DrawBufs, S3)
    // Transparent surfaces (glass windows): rendered in a SEPARATE alpha-blend
    // pass AFTER the opaque scene so you can see through them. (v0.456)
    f.transparent_objects = crate::engine::state::DrawBufs::take(&mut state.draw_bufs.transparent); // last frame's memory (DrawBufs, S3)
    // Editor GIZMOS (corner orbs, the avatar, its pyramid): rendered LAST with depth
    // test off, so they show THROUGH walls + floors in build mode. (v0.560)
    f.overlay_objects = crate::engine::state::DrawBufs::take(&mut state.draw_bufs.overlay); // last frame's memory (DrawBufs, S3)
    // Celestial bodies (planet + Sun + solar bodies): rendered in a SEPARATE
    // pass with a huge far plane (v0.450), since they sit at astronomical
    // distances the ~500 m gameplay far would clip.
    f.celestial_objects = crate::engine::state::DrawBufs::take(&mut state.draw_bufs.celestial); // last frame's memory (DrawBufs, S3)
    // Per-frame defaults for the two near-tree promises the
    // draw loops read (P3 review, 2026-09-18): the card-hide
    // radius and the shadow-only index range (V1) used to be
    // reset only inside the chunked-planet branch below, so
    // the frame after that branch stopped running (leaving the
    // chunked altitude band, a body without chunks) kept the
    // LAST range, and a stale 115..260 would have skipped the
    // colour draw of whatever celestial objects then sat at
    // those indices. Reset here, unconditionally, where the
    // list they index is born; the chunked branch overwrites
    // both when it has something to say.
    state.renderer.tree_card_hide_m = 0.0;
    state.renderer.celestial_colour_skip = 0..0;
    // Batched terrain patches are rebuilt fresh each frame by
    // the chunked-LOD block below; clearing HERE (not there)
    // guarantees no stale draws referencing freed arena
    // ranges survive a frame where chunked mode drops out.
    state.renderer.patch_draws.clear();
    // Same contract for the grass instance set (v0.1091): the
    // planet block below re-publishes it every frame it draws
    // ground, so zeroing here means a frame that never
    // reaches that block (space, menus, a non-chunked body)
    // cannot leave a stale sward hanging in the sky.
    state.renderer.set_grass_instances(&[]);
    // Transparent celestial shells (planet atmospheres, v0.763): alpha-blended
    // after the opaque bodies inside the SAME celestial pass, so they share
    // its far plane and depth-test against the planets.
    f.celestial_transparent = crate::engine::state::DrawBufs::take(&mut state.draw_bufs.celestial_transparent); // last frame's memory (DrawBufs, S3)
    // World-space orbit lines, built this frame, drawn
    // after the scene so they depth-occlude behind planets.
    f.orbit_lines = crate::engine::state::DrawBufs::take(&mut state.draw_bufs.orbit_lines); // last frame's memory (DrawBufs, S3)
    // Build-mode door auto-open RINGS as constant-width line circles (v0.565).
    f.ring_lines = crate::engine::state::DrawBufs::take(&mut state.draw_bufs.ring_lines); // last frame's memory (DrawBufs, S3)

    // Alignment-snap guide lines (v0.613): while dragging an object, draw a faint amber
    // line along the snapped X and/or Z axis (spanning the box) so you see what you are
    // lining up with. Only while a drag is active; cleared implicitly when the grab ends.
    if state.gui_state.construction_active && state.construction_object_grab.is_some() {
        let zo = active_zone_origin(state);
        if let Some(hs) = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
            // Guide coords are WORLD (snap ran in world space); the span covers the
            // ACTIVE zone's box at its origin (v0.754).
            let (bw, bd, gy) = (hs.width, hs.depth, zo.y + 0.05_f32);
            const GUIDE: [f32; 4] = [1.0, 0.85, 0.2, 0.8]; // amber
            if let Some(gx) = state.construction_snap_x {
                crate::renderer::line::push_polyline(&mut f.ring_lines, &[[gx, gy, zo.z], [gx, gy, zo.z + bd]], GUIDE);
            }
            if let Some(gz) = state.construction_snap_z {
                crate::renderer::line::push_polyline(&mut f.ring_lines, &[[zo.x, gy, gz], [zo.x + bw, gy, gz]], GUIDE);
            }
        }
    }

    // During the character-select showroom the home is HIDDEN so the avatar
    // floats against the backdrop; otherwise draw the full home.
    let showroom = state.gui_state.showroom_active;
    // Homestead at origin — vertex positions are in ship-local coords.
    if !showroom {
        for &(mesh_idx, mat_idx) in &state.homestead_floors {
            f.all_objects.push(RenderObject { fade: 0.0,
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                mesh: mesh_idx,
                material: mat_idx,
            });
        }
        // Selected-room highlight (v0.466): a translucent accent quad over the
        // selected room's floor, drawn in the transparent pass so the alpha
        // shows. Built once + cached; scaled per frame; survives rebuilds.
        if let Some(ri) = state.gui_state.construction_selected_room {
            if state.construction_hilite.is_none() {
                let v = vec![
                    humanity_core::vertex::Vertex { position: [-0.5, 0.0, -0.5], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] },
                    humanity_core::vertex::Vertex { position: [0.5, 0.0, -0.5], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0] },
                    humanity_core::vertex::Vertex { position: [0.5, 0.0, 0.5], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0] },
                    humanity_core::vertex::Vertex { position: [-0.5, 0.0, 0.5], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0] },
                ];
                let idx = vec![0u32, 2, 1, 0, 3, 2];
                let m = state.renderer.add_mesh(Mesh::from_vertices(&state.renderer.device, &v, &idx));
                let a = state.theme.accent();
                let mat = state.renderer.add_material_full(
                    [a.r() as f32 / 255.0, a.g() as f32 / 255.0, a.b() as f32 / 255.0, 0.4],
                    0.0, 0.4, 1.0, 0.3,
                );
                state.construction_hilite = Some((m, mat));
            }
            if let Some((hm, hmat)) = state.construction_hilite {
                if let Some(id) = state.gui_state.construction_rooms.get(ri).map(|r| r.id.clone()) {
                    if let Some(rb) = state.gui_state.room_bounds.iter().find(|b| b.id == id) {
                        let center = (rb.min + rb.max) * 0.5;
                        let size = rb.max - rb.min;
                        f.transparent_objects.push(RenderObject { fade: 0.0,
                            position: Vec3::new(center.x, rb.min.y + 0.03, center.z),
                            rotation: Quat::IDENTITY,
                            scale: Vec3::new(size.x, 1.0, size.z),
                            mesh: hm,
                            material: hmat,
                        });
                    }
                }
            }
            // Opening gizmo handles (v0.468, resize v0.469): a glowing accent MOVE
            // cube at each opening's centre, plus warning-tinted RESIZE cubes at the
            // edges of placed (non-floor) openings. Handles float proud of the wall
            // toward the camera so they never z-fight the glass. Cached cube mesh.
            let gizmo_handles = selected_room_handles(state);
            if !gizmo_handles.is_empty() {
                if state.construction_gizmo_handle.is_none() {
                    // Unit cube centred at origin; normals = outward corner dirs
                    // (good enough -- the marker is emissive, so it glows flatly).
                    let c = |x: f32, y: f32, z: f32| {
                        let n = Vec3::new(x, y, z).normalize_or_zero();
                        humanity_core::vertex::Vertex {
                            position: [x, y, z],
                            normal: [n.x, n.y, n.z],
                            uv: [0.0, 0.0],
                        }
                    };
                    let v = vec![
                        c(-0.5, -0.5, -0.5), c(0.5, -0.5, -0.5), c(0.5, 0.5, -0.5), c(-0.5, 0.5, -0.5),
                        c(-0.5, -0.5, 0.5), c(0.5, -0.5, 0.5), c(0.5, 0.5, 0.5), c(-0.5, 0.5, 0.5),
                    ];
                    let idx = vec![
                        4, 5, 6, 4, 6, 7, // +Z
                        1, 0, 3, 1, 3, 2, // -Z
                        5, 1, 2, 5, 2, 6, // +X
                        0, 4, 7, 0, 7, 3, // -X
                        3, 7, 6, 3, 6, 2, // +Y
                        0, 1, 5, 0, 5, 4, // -Y
                    ];
                    let m = state.renderer.add_mesh(Mesh::from_vertices(&state.renderer.device, &v, &idx));
                    let a = state.theme.accent();
                    let mat = state.renderer.add_material_full(
                        [a.r() as f32 / 255.0, a.g() as f32 / 255.0, a.b() as f32 / 255.0, 0.95],
                        0.0, 0.3, 1.0, 0.9, // emissive so it pops against the wall
                    );
                    state.construction_gizmo_handle = Some((m, mat));
                }
                // Resize-handle material (warning tint), sharing the move cube mesh.
                if state.construction_gizmo_resize_handle.is_none() {
                    if let Some((gm, _)) = state.construction_gizmo_handle {
                        let wcol = state.theme.warning();
                        let wmat = state.renderer.add_material_full(
                            [wcol.r() as f32 / 255.0, wcol.g() as f32 / 255.0, wcol.b() as f32 / 255.0, 0.95],
                            0.0, 0.3, 1.0, 0.9,
                        );
                        state.construction_gizmo_resize_handle = Some((gm, wmat));
                    }
                }
                let cam = state.camera.position;
                if let Some((gm, gmat)) = state.construction_gizmo_handle {
                    for (_ri, h) in &gizmo_handles {
                        // Nudge proud of the wall toward the camera (anti z-fight).
                        let s = (cam - h.base_center).dot(h.n);
                        let face = h.n * (if s >= 0.0 { 1.0 } else { -1.0 }) * 0.06;
                        f.transparent_objects.push(RenderObject { fade: 0.0,
                            position: h.base_center + face,
                            rotation: Quat::IDENTITY,
                            scale: Vec3::splat(0.22),
                            mesh: gm,
                            material: gmat,
                        });
                        // Resize handles for placed openings (width for all; height
                        // too for non-floor-snapped kinds). Doors show move-only.
                        if h.opening_index.is_some() {
                            if let Some((rm, rmat)) = state.construction_gizmo_resize_handle {
                                let mut spots = vec![h.handle_left, h.handle_right];
                                if !h.kind.floor_snapped() {
                                    spots.push(h.handle_bottom);
                                    spots.push(h.handle_top);
                                }
                                for p in spots {
                                    f.transparent_objects.push(RenderObject { fade: 0.0,
                                        position: p + face,
                                        rotation: Quat::IDENTITY,
                                        scale: Vec3::splat(0.14),
                                        mesh: rm,
                                        material: rmat,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        // Walls, trim, windows, mirror/portal — all part of the home shell.
        // The roof (ceiling) is gated by the show_roof toggle so the sky stays
        // visible by default. (v0.453)
        // Opaque shell (walls, trim, the emissive portal, optional roof).
        // Windows are TRANSPARENT, so they go in their own list (below).
        let mut shell = vec![
            state.homestead_walls,
            state.homestead_trim,
            state.homestead_mirrors,
        ];
        // Opaque roof: gated on show_roof (off by default so the sky shows). A GLASS
        // roof (v0.539) is a sealed CLEAR ceiling -- always visible, drawn transparent
        // below, so you see the stars through it without hiding the seal.
        if state.gui_state.show_roof && !state.homestead_ceiling_glass {
            shell.push(state.homestead_ceiling);
        }
        // OPAQUE-roof zones' ceilings (v0.754, per-zone roofs): same show_roof gate
        // as the old single opaque roof, regardless of the glass slot's state.
        if state.gui_state.show_roof {
            shell.push(state.homestead_ceiling_opaque);
        }
        // The glass roof's GLAZING BARS (2026-11-14): opaque, shown and hidden exactly with the
        // glass roof they carry (below: first person, never build mode's top-down view).
        if state.homestead_ceiling_glass && !state.gui_state.construction_active {
            shell.push(state.homestead_roof_frame);
        }
        // The HULL WRAP (increment D): exterior plating around the whole zone
        // cluster, holes already cut over glass roofs + glass corridor lids.
        // Gated by Show hull (H / Settings) so interior shots + the top-down
        // editor view can shed it instantly; visible in play AND build mode
        // by default so the operator sees the silhouette while editing zones.
        if state.gui_state.show_hull {
            shell.push(state.homestead_hull);
        }
        for (mesh_idx, mat_idx) in shell.into_iter().flatten() {
            f.all_objects.push(RenderObject { fade: 0.0,
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                mesh: mesh_idx,
                material: mat_idx,
            });
        }
        // Per-material home walls (v0.552): opaque materials in the main pass, glass
        // (transparent) in the transparent pass so it blends behind/through correctly.
        for &(mesh_idx, mat_idx, transparent) in &state.homestead_material_walls {
            let obj = RenderObject { fade: 0.0,
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                mesh: mesh_idx,
                material: mat_idx,
            };
            if transparent {
                f.transparent_objects.push(obj);
            } else {
                f.all_objects.push(obj);
            }
        }
        // Glass windows + a glass roof -> the transparent pass.
        if let Some((mesh_idx, mat_idx)) = state.homestead_windows {
            f.transparent_objects.push(RenderObject { fade: 0.0,
                position: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                mesh: mesh_idx,
                material: mat_idx,
            });
        }
        // The glass roof is hidden in BUILD MODE (v0.540) so it never obscures the
        // top-down editor view; it shows in first-person (a sealed clear roof).
        if state.homestead_ceiling_glass && !state.gui_state.construction_active {
            if let Some((mesh_idx, mat_idx)) = state.homestead_ceiling {
                f.transparent_objects.push(RenderObject { fade: 0.0,
                    position: Vec3::ZERO,
                    rotation: Quat::IDENTITY,
                    scale: Vec3::ONE,
                    mesh: mesh_idx,
                    material: mat_idx,
                });
            }
        }
    }

    // Placeholder objects: the whole home normally; ONLY the avatar (its
    // parts are the tail, from avatar_obj_start) during the showroom.
    let pstart = if showroom { state.avatar_obj_start } else { 0 };
    for &(mesh_idx, mat_idx, pos) in
        state.placeholder_objects.get(pstart..).unwrap_or(&[])
    {
        f.all_objects.push(RenderObject { fade: 0.0,
            position: pos,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            mesh: mesh_idx,
            material: mat_idx,
        });
    }
    // Home machines: a separate list so the construction editor can rebuild just
    // them on an edit. Hidden in the showroom (avatar-only). (v0.525)
    // v0.636: the per-type HIDE set skips a type's meshes to declutter a busy build.
    let hide_machines = state.gui_state.construction_hidden_types.contains("Machine");
    let hide_pipes = state.gui_state.construction_hidden_types.contains("Pipe");
    if !showroom {
        if !hide_machines {
        // The machines and their in-world screens (rung 2), but those the build
        // editor's cut-away leaves out (home_meshes `push_home_machines`).
        crate::engine::home_meshes::push_home_machines(state, &mut f.all_objects);
        // Procedural plants (v0.862): one merged mesh per planted
        // tower, geometry already in world coordinates; and the
        // photoscanned decoration plants (v0.909, CC0 models
        // scattered from data/entities/decorations.ron). Those the
        // editor's cut-away leaves out are not drawn
        // (home_meshes `push_home_plants`).
        crate::engine::home_meshes::push_home_plants(state, &mut f.all_objects);
        // Grow enclosures (2026-09-27): each mushroom rack's
        // fruiting tent (its fitted cover since 2026-11-13: film and seams in the
        // transparent pass, its zips and ties with the opaque objects).
        push_grow_enclosures(state, &mut f.all_objects, &mut f.transparent_objects);
        }
        // Connection cylinders (live, colored by kind; follow rooms). (v0.530)
        if !hide_pipes {
        for &(mesh_idx, mat_idx, pos, rot, scale) in &state.connection_objects {
            f.all_objects.push(RenderObject { fade: 0.0,
                position: pos,
                rotation: rot,
                scale,
                mesh: mesh_idx,
                material: mat_idx,
            });
        }
        } // end !hide_pipes
        // Animated FLOW markers (v0.623): rainbow spheres travel along ONLY the SELECTED
        // machine's connections, so it's obvious which runs go to/from the thing you're
        // looking at. Every other pipe stays a quiet static line (drawn above). The
        // colour cycles through flow_rgb_mats by (marker index + time) for the moving-
        // rainbow look the operator asked for. Build mode + "Helper gizmos" toggle.
        // Cheap: a handful of spheres on one machine's pipes, no per-frame allocation.
        if state.gui_state.construction_active
            && state.gui_state.construction_show_helpers
            && state.gui_state.construction_machine_selected.is_some()
            && !state.flow_rgb_mats.is_empty()
        {
            if let Some(sphere) = state.connection_flow_sphere {
                let sel = state.gui_state.construction_machine_selected.clone();
                let sel = sel.as_deref();
                let t = state.start_time.elapsed().as_secs_f32();
                const FLOW_SPEED: f32 = 1.5; // m/s the markers travel along the pipe
                const SPACING: f32 = 2.0; // m between markers on a pipe
                let nmat = state.flow_rgb_mats.len();
                for (path, from_id, to_id) in &state.connection_flow_paths {
                    // Only the selected machine's runs animate.
                    if sel != Some(from_id.as_str()) && sel != Some(to_id.as_str()) {
                        continue;
                    }
                    let total: f32 = path.windows(2).map(|w| (w[1] - w[0]).length()).sum();
                    if total < 0.1 {
                        continue;
                    }
                    let phase = (t * FLOW_SPEED).rem_euclid(SPACING);
                    let count = (total / SPACING) as usize + 1;
                    for k in 0..count {
                        let mut d = k as f32 * SPACING + phase;
                        if d > total {
                            continue;
                        }
                        // Walk the polyline to arc-length d.
                        let mut pos = path[0];
                        for w in path.windows(2) {
                            let seg = w[1] - w[0];
                            let l = seg.length();
                            if d <= l || l < 1e-4 {
                                pos = w[0] + seg * (d / l.max(1e-4)).clamp(0.0, 1.0);
                                break;
                            }
                            d -= l;
                        }
                        // Cycle the rainbow by marker index + time so the colour flows.
                        let ci = ((k as f32 + t * 2.0) as usize) % nmat;
                        f.all_objects.push(RenderObject { fade: 0.0,
                            position: pos,
                            rotation: Quat::IDENTITY,
                            scale: Vec3::ONE,
                            mesh: sphere,
                            material: state.flow_rgb_mats[ci],
                        });
                    }
                }
            }
        }
        // PORT node gizmo -- central sphere (v0.627): a solid ~10cm node at each of the
        // SELECTED machine's ports, coloured by utility; the 4 in/out arrows (drawn in the
        // line overlay below) radiate from it. A real "node object" the operator wanted,
        // toward eventually plugging into a 3D model with real ports.
        if state.gui_state.construction_active
            && state.gui_state.construction_machine_selected.is_some()
            && !hide_machines
        {
            let sel = state.gui_state.construction_machine_selected.clone().unwrap_or_default();
            let ports: Vec<(Vec3, String)> = state
                .port_pick
                .iter()
                .filter(|(mid, ..)| mid == &sel)
                .map(|(_, _, port, wp)| (*wp, port.utility.id().to_string()))
                .collect();
            if !ports.is_empty() {
                if state.port_node_mesh.is_none() {
                    let m = state.renderer.add_mesh(Mesh::sphere(&state.renderer.device, 0.05, 10, 12));
                    state.port_node_mesh = Some(m);
                }
                let mesh = state.port_node_mesh.unwrap();
                for (wp, kind) in ports {
                    let key = format!("portnode:{kind}");
                    let mat = match state.connection_mats.get(&key) {
                        Some(&m) => m,
                        None => {
                            // The pipes' band colour in linear light (2026-10-04 review).
                            let fill = humanity_ship::machines::MachineHome::gizmo_colours(&kind, [1.0; 4]).fill;
                            let m = state.renderer.add_material_full(fill, 0.0, 0.4, 0.0, 2.0);
                            state.connection_mats.insert(key, m);
                            m
                        }
                    };
                    f.all_objects.push(RenderObject { fade: 0.0,
                        position: wp,
                        rotation: Quat::IDENTITY,
                        scale: Vec3::ONE,
                        mesh,
                        material: mat,
                    });
                }
            }
        }
        // RAIL CARS (v0.637, superstructure M2b): a small car animates along each rail
        // edge in build mode so the line reads as ALIVE -- the visual payoff that the rail
        // graph works. Render-only (no state); mirrors the conduit flow-marker pattern.
        if state.gui_state.construction_active {
            // Collect the edge endpoints first so the structure borrow ends before we
            // mutate the renderer (lazy mesh/material) + push. EVERY zone's rail
            // edges, offset by their zone origins (the track meshes render there
            // too, v0.754).
            let rail_edges: Vec<(Vec3, Vec3)> = match state.gui_state.ship_structure.as_ref() {
                Some(ship) => ship
                    .zones
                    .iter()
                    .flat_map(|z| {
                        let o = z.origin_vec();
                        let hs = &z.body;
                        hs.rail_edges.iter().filter_map(move |e| {
                            let a = hs.rail_node_pos(e.from)?;
                            let b = hs.rail_node_pos(e.to)?;
                            Some((Vec3::new(a.0, 0.15, a.1) + o, Vec3::new(b.0, 0.15, b.1) + o))
                        })
                    })
                    .collect(),
                None => Vec::new(),
            };
            if !rail_edges.is_empty() {
                if state.rail_car_mesh.is_none() {
                    let m = state.renderer.add_mesh(Mesh::box_xyz(&state.renderer.device, 1.6, 0.7, 0.7));
                    state.rail_car_mesh = Some(m);
                }
                if state.rail_car_mat.is_none() {
                    let m = state.renderer.add_material_full([0.95, 0.85, 0.4, 1.0], 0.3, 0.4, 0.0, 1.2);
                    state.rail_car_mat = Some(m);
                }
                let mesh = state.rail_car_mesh.unwrap();
                let mat = state.rail_car_mat.unwrap();
                let t = state.start_time.elapsed().as_secs_f32();
                const SPEED: f32 = 0.12; // fraction of an edge per second
                for (i, (a, b)) in rail_edges.iter().enumerate() {
                    let phase = (t * SPEED + i as f32 * 0.17).rem_euclid(1.0);
                    let pos = *a + (*b - *a) * phase;
                    let diff = *b - *a;
                    let len = diff.length();
                    let rot = if len > 1e-4 {
                        Quat::from_rotation_arc(Vec3::X, diff / len)
                    } else {
                        Quat::IDENTITY
                    };
                    f.all_objects.push(RenderObject { fade: 0.0, position: pos, rotation: rot, scale: Vec3::ONE, mesh, material: mat });
                }
            }
        }
        // Door + window panels: doors ease open as the player nears (by style);
        // windows are fixed glass (transparent pass). (v0.537)
        render_door_panels(state, &mut f.all_objects, &mut f.transparent_objects, &mut f.ring_lines, dt);

        // Elevator CARS (v0.590): a moving floor slab per elevator at its animated
        // height, so you see + stand on the lift. Cached unit box, scaled/posed per car.
        if state.elevator_car_mesh.is_none() {
            let m = state.renderer.add_mesh(Mesh::box_xyz(&state.renderer.device, 1.0, 1.0, 1.0));
            state.elevator_car_mesh = Some(m);
        }
        if state.elevator_car_mat.is_none() {
            // theme-exempt: elevator car platform, neutral brushed metal.
            let m = state.renderer.add_material_typed([0.4, 0.43, 0.48, 1.0], 0.3, 0.5, 0.0);
            state.elevator_car_mat = Some(m);
        }
        let car_mesh = state.elevator_car_mesh.unwrap();
        let car_mat = state.elevator_car_mat.unwrap();
        // EVERY elevator's car at its height, its footprint less a 2 cm running
        // clearance, none the editor's cut-away hides (home_meshes
        // `elevator_car_poses`, the same lifts the cars ride on).
        let cars: Vec<(Vec3, Quat, Vec3)> = state
            .gui_state
            .ship_structure
            .as_ref()
            .map(|ship| crate::engine::home_meshes::elevator_car_poses(ship, &state.elevator_state, state.home_draw.cut.clone()))
            .unwrap_or_default();
        for (position, rotation, scale) in cars {
            f.all_objects.push(RenderObject { fade: 0.0, position, rotation, scale, mesh: car_mesh, material: car_mat });
        }
    }
    f.showroom = showroom;
    f.hide_machines = hide_machines;
}
