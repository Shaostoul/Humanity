//! Build mode's helper gizmos (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as they were: bounds boxes on placed structures and machines, the
//! build-mode avatar and its drag gizmo, and the door and window opening gizmos.

use crate::engine::frame_prelude::*;

/// Draws build mode's helper gizmos, the build avatar and the opening gizmos.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars) {
    let hover = f.hover;
    // HELPER GIZMOS (v0.583/586/587): bounds boxes on placed structures + machines, the
    // road graph (node rings + edge centerlines), and conduit-node markers -- the helper
    // widgets the operator asked for "on everything." All drawn with the reusable line
    // primitive (shows through walls), gated by the master toggle so a busy view can quiet
    // them. The interactive editing handles (corner orbs, resize cubes) are NOT gated.
    if state.gui_state.construction_active && state.gui_state.construction_show_helpers {
        if let Some(hs) = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
            let sel = state.gui_state.construction_structure_selected;
            // WALL WIREFRAME (v0.594, operator): the layout outline -- top + bottom edge
            // of every wall (box perimeter + interior walls) + a vertical at each corner,
            // like looking at the floor-plan wireframe. Param-closure so it doesn't hold a
            // borrow on ring_lines across the rest of the helper block.
            const WF: [f32; 4] = [0.80, 0.86, 0.92, 0.45];
            // Helper gizmos draw the ACTIVE zone's body, shifted into world by its
            // origin (v0.754). Baked into the `wire` closure + each loop below.
            let zo = active_zone_origin(state);
            // Each wall from its base (`base` and `h` body-local; ship homes increment 7).
            let wire = |lines: &mut Vec<crate::renderer::line::LineVertex>, a: (f32, f32), b: (f32, f32), base: f32, h: f32| {
                let (a, b) = ((a.0 + zo.x, a.1 + zo.z), (b.0 + zo.x, b.1 + zo.z));
                let (y0, y1) = (zo.y + base, zo.y + base + h);
                crate::renderer::line::push_polyline(lines, &[[a.0, y0, a.1], [b.0, y0, b.1]], WF); // bottom
                crate::renderer::line::push_polyline(lines, &[[a.0, y1, a.1], [b.0, y1, b.1]], WF); // top
                crate::renderer::line::push_polyline(lines, &[[a.0, y0, a.1], [a.0, y1, a.1]], WF); // vert @a
                crate::renderer::line::push_polyline(lines, &[[b.0, y0, b.1], [b.0, y1, b.1]], WF); // vert @b
            };
            let (bw, bd, bh) = (hs.width, hs.depth, hs.height);
            wire(&mut f.ring_lines, (0.0, 0.0), (bw, 0.0), 0.0, bh);
            wire(&mut f.ring_lines, (bw, 0.0), (bw, bd), 0.0, bh);
            wire(&mut f.ring_lines, (bw, bd), (0.0, bd), 0.0, bh);
            wire(&mut f.ring_lines, (0.0, bd), (0.0, 0.0), 0.0, bh);
            // What the storey cut-away hides is not outlined either (ship homes
            // increment 7): no wall standing above it, none higher than its top.
            let es = EditStorey::of(hs, state.gui_state.construction_storeys.chosen);
            let cap = if es.has_above() { es.storey.top } else { f32::INFINITY };
            for w in hs.walls.iter().filter(|w| es.shows(w.base)) {
                wire(&mut f.ring_lines, w.a, w.b, w.base, w.height.min(cap - w.base).max(0.1));
            }
            // FLOOR SLABS (ship homes increment 7): each drawn in the cut-away outlined
            // at its top, the selected one bright (its corner handles are drawn with the
            // corner orbs).
            for (pts, selected) in slab_outlines(state) {
                let col: [f32; 4] = if selected { [1.0, 0.85, 0.25, 0.95] } else { [0.45, 0.85, 1.0, 0.6] };
                let ring: Vec<[f32; 3]> = pts.iter().chain(pts.first()).map(|p| [p.x, p.y, p.z]).collect();
                crate::renderer::line::push_polyline(&mut f.ring_lines, &ring, col);
            }
            for (i, ps) in hs.structures.iter().enumerate() {
                let Some(ty) = humanity_ship::structure::structure_type(&ps.type_id) else { continue };
                if ty.kind == humanity_ship::structure::StructureKind::Wall || !es.shows(ps.pos.1) {
                    continue;
                }
                let (w, h, d) = ty.size;
                let (hw, hd) = (w * 0.5, d * 0.5);
                let yaw = ps.rot_deg.to_radians();
                let (s, c) = yaw.sin_cos();
                // The 4 footprint corners, yaw-rotated around the piece centre (world).
                let corner = |lx: f32, lz: f32| {
                    let rx = lx * c + lz * s;
                    let rz = -lx * s + lz * c;
                    (ps.pos.0 + rx + zo.x, ps.pos.2 + rz + zo.z)
                };
                let fc = [corner(-hw, -hd), corner(hw, -hd), corner(hw, hd), corner(-hw, hd)];
                let (y0, y1) = (ps.pos.1 + zo.y, ps.pos.1 + zo.y + h.max(0.1));
                let col: [f32; 4] = if sel == Some(i) {
                    [1.0, 0.85, 0.25, 0.95] // selected: bright amber
                } else {
                    [0.45, 0.85, 1.0, 0.6] // cyan, like other build gizmos
                };
                for k in 0..4 {
                    let (x0, z0) = fc[k];
                    let (x1, z1) = fc[(k + 1) % 4];
                    // Bottom ring, top ring, and a vertical riser at each corner.
                    crate::renderer::line::push_polyline(&mut f.ring_lines, &[[x0, y0, z0], [x1, y0, z1]], col);
                    crate::renderer::line::push_polyline(&mut f.ring_lines, &[[x0, y1, z0], [x1, y1, z1]], col);
                    crate::renderer::line::push_polyline(&mut f.ring_lines, &[[x0, y0, z0], [x0, y1, z0]], col);
                }
            }

            // ROAD-GRAPH gizmo (v0.586): a ring marker at each node + a centerline along
            // each edge, drawn with the line primitive so the graph reads at a glance in
            // build mode (the ribbon mesh shows the carriageway; this shows the topology).
            const RN: [f32; 4] = [1.0, 0.75, 0.2, 0.9]; // node ring (amber)
            const RN_SEL: [f32; 4] = [1.0, 1.0, 1.0, 1.0]; // selected node (white, double ring)
            const RE: [f32; 4] = [0.5, 0.8, 1.0, 0.8]; // edge centerline (cyan)
            let sel_road = state.gui_state.construction_road_node_selected;
            let hide_road = state.gui_state.construction_hidden_types.contains("Road"); // v0.636
            for n in hs.road_nodes.iter().filter(|_| !hide_road) {
                // Grabbable handle: a ring at the node (white double-ring when selected,
                // so you see which you're dragging). Click + hold to drag it. (v0.599)
                let c = [n.pos.0 + zo.x, zo.y + 0.06, n.pos.1 + zo.z];
                if sel_road == Some(n.id) {
                    crate::renderer::line::push_circle(&mut f.ring_lines, c, 0.4, RN_SEL, 24);
                    crate::renderer::line::push_circle(&mut f.ring_lines, c, 0.6, RN_SEL, 24);
                } else {
                    crate::renderer::line::push_circle(&mut f.ring_lines, c, 0.4, RN, 20);
                }
            }
            for e in hs.road_edges.iter().filter(|_| !hide_road) {
                // Draw the CURVED centerline (v0.591) so the gizmo matches the rendered
                // road -- a polyline through the Catmull-Rom samples.
                let center = hs.road_edge_centerline(e);
                if center.len() >= 2 {
                    let pts: Vec<[f32; 3]> = center.iter().map(|p| [p.0 + zo.x, zo.y + 0.08, p.1 + zo.z]).collect();
                    crate::renderer::line::push_polyline(&mut f.ring_lines, &pts, RE);
                }
            }
            // RAIL-GRAPH gizmo (v0.635, superstructure M2): a ring at each rail stop + a
            // straight track line per edge, so the transit line reads at a glance (cars
            // run it in M2b). Mirrors the road-graph gizmo.
            const RAILN: [f32; 4] = [0.88, 0.86, 0.45, 0.95]; // pale-gold stop ring
            const RAILE: [f32; 4] = [0.92, 0.80, 0.35, 0.9]; // track line
            for n in &hs.rail_nodes {
                crate::renderer::line::push_circle(&mut f.ring_lines, [n.pos.0 + zo.x, zo.y + 0.15, n.pos.1 + zo.z], 0.5, RAILN, 18);
            }
            for e in &hs.rail_edges {
                if let (Some(a), Some(b)) = (hs.rail_node_pos(e.from), hs.rail_node_pos(e.to)) {
                    crate::renderer::line::push_polyline(&mut f.ring_lines, &[[a.0 + zo.x, zo.y + 0.18, a.1 + zo.z], [b.0 + zo.x, zo.y + 0.18, b.1 + zo.z]], RAILE);
                }
            }
            // ZONE wireframe boxes (v0.631, superstructure M1): each macro district
            // (residential / industrial / hangar / the civic mall / ...) drawn as a
            // coloured wire box from its origin to origin+size, so the mothership layout
            // reads at a glance. Edited in the Zones panel.
            let sel_zone = state.gui_state.construction_zone_selected.as_deref();
            for z in hs.zones.iter().filter(|z| es.shows(z.origin.1)) {
                let (ox, oy, oz) = (z.origin.0 + zo.x, z.origin.1 + zo.y, z.origin.2 + zo.z);
                let (sw, sh, sd) = z.size;
                let c = humanity_ship::structure::zone_type(&z.type_id).map(|t| t.color).unwrap_or((0.6, 0.6, 0.6));
                // The SELECTED zone draws bright white so you see which the panel edits. (v0.634)
                let col = if sel_zone == Some(z.id.as_str()) { [1.0, 1.0, 1.0, 1.0] } else { [c.0, c.1, c.2, 0.9] };
                let (x1, y1, z1) = (ox + sw, oy + sh, oz + sd);
                let bot = [[ox, oy, oz], [x1, oy, oz], [x1, oy, z1], [ox, oy, z1]];
                let top = [[ox, y1, oz], [x1, y1, oz], [x1, y1, z1], [ox, y1, z1]];
                for k in 0..4 {
                    crate::renderer::line::push_polyline(&mut f.ring_lines, &[bot[k], bot[(k + 1) % 4]], col);
                    crate::renderer::line::push_polyline(&mut f.ring_lines, &[top[k], top[(k + 1) % 4]], col);
                    crate::renderer::line::push_polyline(&mut f.ring_lines, &[bot[k], top[k]], col);
                }
            }
            // Rail-link gizmo (v0.592): a line between PAIRED train platforms (j>i dedup).
            for (i, ps) in hs.structures.iter().enumerate() {
                let is_train = humanity_ship::structure::structure_type(&ps.type_id)
                    .map_or(false, |t| t.kind == humanity_ship::structure::StructureKind::Train);
                if !is_train {
                    continue;
                }
                if let Some(j) = hs.pair_index(i) {
                    if j > i {
                        let pj = &hs.structures[j];
                        crate::renderer::line::push_polyline(
                            &mut f.ring_lines,
                            &[[ps.pos.0 + zo.x, zo.y + 0.2, ps.pos.2 + zo.z], [pj.pos.0 + zo.x, zo.y + 0.2, pj.pos.2 + zo.z]],
                            [0.9, 0.7, 0.3, 0.8],
                        );
                    }
                }
            }

            // MACHINE bounds gizmos (v0.587): a wireframe cube around each placed machine
            // (from its pick volume centre+radius), so every machine has a helper widget
            // like the structures. Trims the click margin so the cube ~ the body. (v0.636:
            // skipped when the Machine type is hidden.)
            // Those the storey cut-away leaves out with their machines are not
            // drawn (`HomeDraw::cut`, by the machine's placement as the body is;
            // machine_pick is in placement order with machine_objects).
            const MB: [f32; 4] = [0.55, 0.8, 0.95, 0.5];
            let cut = state.home_draw.cut.clone();
            let shown_machine = |i: usize| !state.machine_objects.get(i).is_some_and(|m| cut.hides(m.2));
            for (_id, center, radius) in state
                .machine_pick
                .iter()
                .enumerate()
                .filter(|(i, _)| shown_machine(*i) && !state.gui_state.construction_hidden_types.contains("Machine"))
                .map(|(_, m)| m)
            {
                let r = (radius - 0.3).max(0.2);
                let c = *center;
                let fc = [(c.x - r, c.z - r), (c.x + r, c.z - r), (c.x + r, c.z + r), (c.x - r, c.z + r)];
                let (y0, y1) = ((c.y - r).max(0.0), c.y + r);
                for k in 0..4 {
                    let (x0, z0) = fc[k];
                    let (x1, z1) = fc[(k + 1) % 4];
                    crate::renderer::line::push_polyline(&mut f.ring_lines, &[[x0, y0, z0], [x1, y0, z1]], MB);
                    crate::renderer::line::push_polyline(&mut f.ring_lines, &[[x0, y1, z0], [x1, y1, z1]], MB);
                    crate::renderer::line::push_polyline(&mut f.ring_lines, &[[x0, y0, z0], [x0, y1, z0]], MB);
                }
            }

            // PORT gizmos + the DRAG-TO-CONNECT rubber band (v0.625). The SELECTED
            // machine's ports show as coloured handles above it, in the pipes' marking
            // scheme (blue water, orange power, ...); an OUT port gets an outer "target"
            // ring. Drag a handle and a line follows to the cursor / hovered machine --
            // utility-coloured when it can wire, RED when the target has no matching
            // port. Release to connect. The lines take the gizmo OUTLINE colour (linear,
            // 2026-10-04 review): the band colour, or the theme's secondary text where the
            // band colour (ISO 14726's black for waste) is too dark to see.
            let gizmo_light = {
                let c = state.theme.text_secondary();
                let l = humanity_ship::pipe_materials::srgb_to_linear;
                [l(c.r()), l(c.g()), l(c.b()), 1.0]
            };
            {
                let sel = state.gui_state.construction_machine_selected.clone();
                // A machine the storey cut-away leaves out (one upstairs, kept
                // selected while the ground is edited) has no port gizmos drawn
                // either: by the machine, as its body (a greenhouse unit's gizmos
                // stand over the cut's height under the 6 m roof, and are shown
                // with their machine).
                let sel_shown = sel.as_deref().map_or(true, |s| {
                    state.machine_pick.iter().position(|(id, ..)| id == s).map_or(true, shown_machine)
                });
                if let Some(sel_id) = sel.as_deref().filter(|_| sel_shown) {
                    for (mid, _i, port, wp) in &state.port_pick {
                        if mid != sel_id {
                            continue;
                        }
                        let col = humanity_ship::machines::MachineHome::gizmo_colours(port.utility.id(), gizmo_light).outline;
                        // 4 CARDINAL ARROWS around the central node sphere (v0.627): arrows
                        // pointing IN (heads near the sphere) = an INPUT/draw port; pointing
                        // OUT = an OUTPUT/supply port; both heads = bidirectional. Far more
                        // legible at a glance than the old in/out rings.
                        let (head_in, head_out) = match port.dir {
                            humanity_core::utilities::PortDir::In => (true, false),
                            humanity_core::utilities::PortDir::Out => (false, true),
                            humanity_core::utilities::PortDir::Bidirectional => (true, true),
                        };
                        for (dx, dz) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                            let (px, pz) = (-dz, dx); // perpendicular in XZ for the barbs
                            let inner = [wp.x + dx * 0.09, wp.y, wp.z + dz * 0.09];
                            let outer = [wp.x + dx * 0.28, wp.y, wp.z + dz * 0.28];
                            crate::renderer::line::push_polyline(&mut f.ring_lines, &[inner, outer], col);
                            if head_out {
                                // V-head at the OUTER tip, pointing away from the node.
                                let b = [outer[0] - dx * 0.08, wp.y, outer[2] - dz * 0.08];
                                crate::renderer::line::push_polyline(&mut f.ring_lines, &[outer, [b[0] + px * 0.06, wp.y, b[2] + pz * 0.06]], col);
                                crate::renderer::line::push_polyline(&mut f.ring_lines, &[outer, [b[0] - px * 0.06, wp.y, b[2] - pz * 0.06]], col);
                            }
                            if head_in {
                                // V-head at the INNER end, pointing toward the node centre.
                                let b = [inner[0] + dx * 0.08, wp.y, inner[2] + dz * 0.08];
                                crate::renderer::line::push_polyline(&mut f.ring_lines, &[inner, [b[0] + px * 0.06, wp.y, b[2] + pz * 0.06]], col);
                                crate::renderer::line::push_polyline(&mut f.ring_lines, &[inner, [b[0] - px * 0.06, wp.y, b[2] - pz * 0.06]], col);
                            }
                        }
                    }
                }
                if let Some((src_id, util, _pdir, wp)) = state.construction_port_drag.clone() {
                    let c = humanity_ship::machines::MachineHome::gizmo_colours(util.id(), gizmo_light).outline;
                    // A conduit NODE under the cursor takes precedence (v0.629): dropping
                    // there branches the machine onto the main line, always valid.
                    let node_center = port_drop_node_target(state).and_then(|nid| {
                        state.gui_state.home_machines.as_ref().and_then(|h| {
                            h.conduit_nodes.iter().find(|n| n.id == nid).map(|n| Vec3::new(n.pos.0, n.pos.1, n.pos.2))
                        })
                    });
                    let target = if node_center.is_none() { port_drop_target(state, &src_id) } else { None };
                    let mach_center = target
                        .as_ref()
                        .and_then(|t| state.machine_pick.iter().find(|(id, ..)| id == t).map(|(_, c, _)| *c));
                    let center = node_center.or(mach_center);
                    let ok = node_center.is_some() || target.as_ref().map_or(false, |t| machine_has_port(state, t, util));
                    let end_pt = center
                        .map(|c| [c.x, c.y, c.z])
                        .or_else(|| cursor_floor_hit(state).map(|(_, hx, hz)| [hx, 0.05, hz]))
                        .unwrap_or([wp.x, wp.y, wp.z]);
                    let line_col = if target.is_some() && !ok {
                        [0.95, 0.3, 0.25, 0.95] // incompatible target -> red
                    } else {
                        [c[0], c[1], c[2], 0.95]
                    };
                    crate::renderer::line::push_polyline(&mut f.ring_lines, &[[wp.x, wp.y, wp.z], end_pt], line_col);
                    if let Some(center) = center {
                        crate::renderer::line::push_circle(&mut f.ring_lines, [center.x, center.y, center.z], 0.5, line_col, 18);
                    }
                }
                // Highlight the SELECTED pipe/wire (v0.626): trace its routed polyline in
                // bright white so you see exactly which connection the panel Remove acts on.
                if let Some((fa, fb)) = state.gui_state.construction_connection_selected.clone() {
                    for (path, from_id, to_id) in &state.connection_flow_paths {
                        if ((from_id == &fa && to_id == &fb) || (from_id == &fb && to_id == &fa)) && path.len() >= 2 {
                            let pts: Vec<[f32; 3]> = path.iter().map(|p| [p.x, p.y, p.z]).collect();
                            crate::renderer::line::push_polyline(&mut f.ring_lines, &pts, [1.0, 1.0, 1.0, 1.0]);
                        }
                    }
                }
            }

            // CONDUIT-NODE markers (v0.587): a small ring at each pipe-graph junction --
            // grabbable (v0.599): white double-ring when selected, so you see which
            // you're dragging; click + hold drags it (keeping its height).
            if let Some(hm) = state.gui_state.home_machines.as_ref() {
                const CN: [f32; 4] = [0.4, 0.85, 0.95, 0.85];
                const CN_SEL: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
                const GRID_TIE: [f32; 4] = [1.0, 0.55, 0.15, 0.95]; // amber service-entrance
                let sel_pipe = state.gui_state.construction_conduit_node_selected.as_deref();
                let hide_pipe_nodes = state.gui_state.construction_hidden_types.contains("Pipe"); // v0.636
                // Not those the storey cut-away leaves out with the runs over it.
                for n in hm
                    .conduit_nodes
                    .iter()
                    .filter(|n| !hide_pipe_nodes && !cut.hides(Vec3::new(n.pos.0, n.pos.1, n.pos.2)))
                {
                    let c = [n.pos.0, n.pos.1, n.pos.2];
                    // TIER sizes the node (v0.632): tier 0 main = a big ring, subs smaller,
                    // so the trunk-and-branch hierarchy reads at a glance.
                    let r = 0.30 - (n.tier as f32) * 0.07; // 0.30 / 0.23 / 0.16
                    if sel_pipe == Some(n.id.as_str()) {
                        crate::renderer::line::push_circle(&mut f.ring_lines, c, r, CN_SEL, 18);
                        crate::renderer::line::push_circle(&mut f.ring_lines, c, r + 0.12, CN_SEL, 18);
                    } else {
                        crate::renderer::line::push_circle(&mut f.ring_lines, c, r, CN, 14);
                    }
                    // GRID TIE (v0.632): an amber double ring marks the external-grid
                    // connection (the service entrance) distinctly from interior junctions.
                    if n.grid_tie {
                        crate::renderer::line::push_circle(&mut f.ring_lines, c, r + 0.18, GRID_TIE, 20);
                        crate::renderer::line::push_circle(&mut f.ring_lines, c, r + 0.26, GRID_TIE, 20);
                    }
                }
            }
        }
    }

    // Build-mode AVATAR (v0.557): a little figure you drag by its pyramid gizmo to set
    // where you spawn when you leave build mode. Body box + sphere head + a bright
    // pyramid handle on the floor (white while grabbed).
    if state.gui_state.construction_active {
        if let Some((cx, cz)) = state.gui_state.build_char_pos {
            if state.construction_char_mesh.is_none() {
                // Player-sized body (~0.5 wide x 1.55 tall x 0.3 deep). (v0.560)
                let m = state.renderer.add_mesh(Mesh::box_xyz(&state.renderer.device, 0.5, 1.55, 0.3));
                state.construction_char_mesh = Some(m);
            }
            if state.construction_char_pyramid_mesh.is_none() {
                let m = state.renderer.add_mesh(Mesh::pyramid(&state.renderer.device, 1.0, 1.0));
                state.construction_char_pyramid_mesh = Some(m);
            }
            if state.construction_char_mat.is_none() {
                // theme-exempt: build-mode avatar marker, friendly teal.
                let m = state.renderer.add_material_full([0.30, 0.72, 0.80, 1.0], 0.0, 0.5, 0.0, 0.25);
                state.construction_char_mat = Some(m);
            }
            if state.construction_node_mesh.is_none() {
                let m = state.renderer.add_mesh(Mesh::sphere(&state.renderer.device, 1.0, 12, 16));
                state.construction_node_mesh = Some(m);
            }
            if state.construction_node_mat.is_none() {
                // theme-exempt: gizmo handle.
                let m = state.renderer.add_material_full([1.0, 0.82, 0.2, 1.0], 0.0, 0.4, 0.0, 0.6);
                state.construction_node_mat = Some(m);
            }
            if state.construction_node_mat_hot.is_none() {
                // theme-exempt: grabbed-gizmo highlight.
                let m = state.renderer.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.3, 0.0, 1.0);
                state.construction_node_mat_hot = Some(m);
            }
            if state.construction_node_mat_hover.is_none() {
                // theme-exempt: hover highlight.
                let m = state.renderer.add_material_full([1.0, 0.96, 0.62, 1.0], 0.0, 0.35, 0.0, 0.95);
                state.construction_node_mat_hover = Some(m);
            }
            let body_mesh = state.construction_char_mesh.unwrap();
            let pyr_mesh = state.construction_char_pyramid_mesh.unwrap();
            let head_mesh = state.construction_node_mesh.unwrap();
            let char_mat = state.construction_char_mat.unwrap();
            // Pyramid handle: idle -> hover -> active (grabbed) by colour (v0.569).
            let pyr_mat = if state.construction_char_grab {
                state.construction_node_mat_hot.unwrap()
            } else if hover == HoverGizmo::Char {
                state.construction_node_mat_hover.unwrap()
            } else {
                state.construction_node_mat.unwrap()
            };
            // Player-sized avatar STANDING on the floor (v0.560): body box ~1.55 m + a
            // head, so the marker reads as the player's height/width. Overlay pass ->
            // visible through walls. The avatar stands in the ACTIVE zone, so its
            // zone-local (cx, cz) shifts to world by the zone origin (v0.754). It
            // stands on the chosen storey's floor, where it is grabbed and dragged
            // (ship homes increment 7, `build_avatar_floor_y`).
            let zo = active_zone_origin(state).with_y(crate::engine::editor::build_avatar_floor_y(state));
            let (cx, cz) = (cx + zo.x, cz + zo.z);
            f.overlay_objects.push(RenderObject { fade: 0.0,
                position: Vec3::new(cx, zo.y, cz),
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
                mesh: body_mesh,
                material: char_mat,
            });
            f.overlay_objects.push(RenderObject { fade: 0.0,
                position: Vec3::new(cx, zo.y + 1.66, cz),
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.2),
                mesh: head_mesh,
                material: char_mat,
            });
            // Pyramid gizmo BELOW the floor with its top vertex at the floor (operator
            // note): apex at y=0, base at -0.4.
            f.overlay_objects.push(RenderObject { fade: 0.0,
                position: Vec3::new(cx, zo.y - 0.4, cz),
                rotation: Quat::IDENTITY,
                scale: Vec3::new(0.5, 0.4, 0.5),
                mesh: pyr_mesh,
                material: pyr_mat,
            });
        }
    }

    // Door/window OPENING gizmos (v0.546): a distinct cyan CUBE at each opening,
    // draggable along its wall (vs the yellow corner spheres). Cached mesh + material.
    if state.gui_state.construction_active && state.gui_state.ship_structure.is_some() {
        if state.construction_opening_mesh.is_none() {
            let m = state.renderer.add_mesh(Mesh::box_xyz(&state.renderer.device, 1.0, 1.0, 1.0));
            state.construction_opening_mesh = Some(m);
        }
        if state.construction_opening_mat.is_none() {
            // theme-exempt: editor gizmo, distinct cyan + emissive so it stands out.
            let m = state.renderer.add_material_full([0.2, 0.9, 1.0, 1.0], 0.0, 0.4, 0.0, 0.7);
            state.construction_opening_mat = Some(m);
        }
        let mesh = state.construction_opening_mesh.unwrap();
        let mat = state.construction_opening_mat.unwrap();
        // Grabbed/hover highlight materials (created by the corner block, which runs first
        // under the same condition). Idle cyan -> hover cream -> active RGB. (v0.569)
        let hot_mat = state.construction_node_mat_hot.unwrap();
        let hover_mat = state.construction_node_mat_hover.unwrap();
        let node_mat = state.construction_node_mat.unwrap(); // yellow, for resize handles
        let grabbed_op = state.construction_opening_grab;
        let resize_grab = state.construction_opening_resize;
        let ozo = active_zone_origin(state); // gizmo world positions (v0.754)
        // The openings of the storey being edited (ship homes increment 7).
        let (gizmos, storey_k) = {
            let hs = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone).unwrap();
            let k = EditStorey::of(hs, state.gui_state.construction_storeys.chosen).index;
            (opening_gizmos_on_storey(hs, ozo, k), k)
        };
        const S: f32 = 0.35;
        let hs = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone).unwrap();
        for (idx, p) in &gizmos {
            // Align the cube to ITS wall (v0.560) instead of fixed north/south: rotate
            // by the wall's heading so its faces sit square to the wall at any angle.
            let yaw = hs
                .walls
                .get(idx.0)
                .map_or(0.0, |w| (w.b.1 - w.a.1).atan2(w.b.0 - w.a.0));
            let m = if grabbed_op == Some(*idx) {
                hot_mat
            } else if hover == HoverGizmo::Opening(idx.0, idx.1) {
                hover_mat
            } else {
                mat
            };
            f.all_objects.push(RenderObject { fade: 0.0,
                position: Vec3::new(p.x, p.y - S * 0.5, p.z), // box_xyz y-bottom -> centre
                rotation: Quat::from_rotation_y(-yaw),
                scale: Vec3::splat(S),
                mesh,
                material: m,
            });
        }
        // RESIZE handles (v0.578): 4 smaller YELLOW cubes per opening at its aperture
        // edges (left/right resize width, top/bottom resize height); centred on the seam
        // between the opening and the wall/frame. The grabbed one shifts RGB.
        const RS: f32 = 0.16;
        for ((wi, oi, edge), p) in opening_resize_handles_on_storey(hs, ozo, storey_k) {
            let yaw = hs.walls.get(wi).map_or(0.0, |w| (w.b.1 - w.a.1).atan2(w.b.0 - w.a.0));
            let m = if resize_grab == Some((wi, oi, edge)) { hot_mat } else { node_mat };
            f.all_objects.push(RenderObject { fade: 0.0,
                position: Vec3::new(p.x, p.y - RS * 0.5, p.z),
                rotation: Quat::from_rotation_y(-yaw),
                scale: Vec3::splat(RS),
                mesh,
                material: m,
            });
        }
    }

    humanity_core::frame_costs::span("cpu.span.lists_people");
}
