use glam::{Quat, Vec3};
use crate::engine::geom::{light_ring_pick_tolerance, light_ring_radius, ray_aabb_hit, ray_ring_closest, snap_node_position, snap_to_alignment};
use crate::engine::home_meshes::{rebuild_homestead, rebuild_machine_objects};
use crate::engine::state::*;
use crate::ship::ship_structure::{zone_body, zone_body_mut, zone_origin};

/// Which of the SELECTED light's rotation rings the cursor ray is over, if any: the shared
/// pick test behind BOTH the click grab (try_pick_light_ring) and the per-frame hover
/// highlight, so hover and grab can never disagree (v0.792). Returns the axis index
/// (0 = X/red, 1 = Y/green, 2 = Z/blue) of the nearest ring within tolerance. SELECTED
/// light only: unselected lights draw rings as a location cue, but arming/highlighting
/// them would steal attention (and clicks) from whatever sits underneath.
pub(crate) fn light_ring_under_cursor(state: &EngineState) -> Option<u8> {
    let li = state.gui_state.construction_light_selected?;
    let zo = active_zone_origin(state); // the light is zone-local; test in world (v0.754)
    let l = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone)
        .and_then(|h| h.lights.get(li))?;
    let center = Vec3::new(l.pos.0, l.pos.1, l.pos.2) + zo;
    let radius = light_ring_radius(state.camera.position, center);
    let tolerance = light_ring_pick_tolerance(radius);
    let sz = state.window.inner_size();
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    // Each ring is the circle of `radius` in the plane perpendicular to its axis -- exactly
    // what push_circle_3d draws. Nearest hit by camera distance wins.
    let mut best: Option<(u8, f32)> = None;
    for (ai, axis) in [Vec3::X, Vec3::Y, Vec3::Z].into_iter().enumerate() {
        if let Some((t, dist)) = ray_ring_closest(origin, dir, center, axis, radius) {
            if dist < tolerance && best.map_or(true, |(_, bt)| t < bt) {
                best = Some((ai as u8, t));
            }
        }
    }
    best.map(|(axis, _)| axis)
}

/// Per-frame while a corner node is grabbed: raycast to the floor, snap, and move EVERY wall
/// endpoint at the grabbed position to the snapped one (so shared corners move together). (v0.541)
/// Pixels the cursor must travel from the press point before a gizmo grab becomes a DRAG (v0.549).
/// How far above the deck a corridor DOOR-MOUTH handle floats (v0.790). Shared by the draw
/// block and try_pick_corridor_mouth so the diamond you see is exactly the point you grab.
pub(crate) const CORRIDOR_MOUTH_HANDLE_LIFT: f32 = 0.3;

pub(crate) const DRAG_THRESHOLD_PX: f32 = 6.0;

/// Every unique wall CORNER (deduped by position) -- the node set the gizmos + dragging act on.
/// (v0.541)
pub(crate) fn unique_corners(hs: &crate::ship::home_structure::HomeStructure) -> Vec<(f32, f32)> {
    let mut out: Vec<(f32, f32)> = Vec::new();
    for wall in &hs.walls {
        for c in [wall.a, wall.b] {
            if !out.iter().any(|o| (o.0 - c.0).abs() < 0.05 && (o.1 - c.1).abs() < 0.05) {
                out.push(c);
            }
        }
    }
    out
}

/// Snapshot the current editor state for undo/redo (v0.575). Structure + machines only -- not the
/// selection (restoring a stale selection would yank the right panel).
pub(crate) fn editor_snapshot(state: &EngineState) -> EditorSnapshot {
    EditorSnapshot {
        structure: state.gui_state.ship_structure.clone(),
        machines: state.gui_state.home_machines.clone(),
    }
}

/// Restore a snapshot into gui_state and rebuild the home DIRECTLY (v0.575). Rebuilding here rather
/// than via the dirty flags means the restore never looks like a fresh edit to the history tick --
/// so it can't spuriously checkpoint and there's no restore/edit frame race.
pub(crate) fn editor_restore(state: &mut EngineState, snap: EditorSnapshot) {
    state.gui_state.ship_structure = snap.structure;
    state.gui_state.home_machines = snap.machines;
    // An undo can remove zones; keep the active-zone index valid (v0.754).
    let max_zone = state
        .gui_state
        .ship_structure
        .as_ref()
        .map_or(0, |s| s.zones.len().saturating_sub(1));
    state.gui_state.construction_zone = state.gui_state.construction_zone.min(max_zone);
    rebuild_homestead(state);
    rebuild_machine_objects(state);
}

/// Per-frame undo-history tick (v0.575). Call BEFORE the dirty-flag rebuild blocks consume them.
/// `edited` = a dirty flag was set this frame. Resets history on editor-open; coalesces a continuous
/// drag -- a gizmo OR a slider -- into ONE undo step by checkpointing only while the left mouse
/// button is NOT held, plus once on release if an edit happened during the hold.
pub(crate) fn construction_history_tick(state: &mut EngineState, edited: bool) {
    let active = state.gui_state.construction_active;
    let prev_active = state.construction_history.prev_active;
    state.construction_history.prev_active = active;
    if active && !prev_active {
        // Editor opened: the current state is the baseline; clear the stacks.
        let base = editor_snapshot(state);
        let h = &mut state.construction_history;
        h.undo.clear();
        h.redo.clear();
        h.baseline = base;
        h.edited_during_hold = false;
        h.prev_held = false;
        return;
    }
    if !active {
        return; // history is editor-only
    }
    let held = state.lmb_held;
    let prev_held = state.construction_history.prev_held;
    state.construction_history.prev_held = held;
    if held {
        if edited {
            state.construction_history.edited_during_hold = true;
        }
        return; // never checkpoint mid-drag (gizmo or slider)
    }
    // Not held: checkpoint on a click-edit, or a release that actually edited during the hold.
    let released_with_edit = prev_held && state.construction_history.edited_during_hold;
    if released_with_edit || edited {
        let cur = editor_snapshot(state);
        let depth = state.gui_state.construction_undo_depth.clamp(1, 4096);
        let h = &mut state.construction_history;
        h.undo.push_back(std::mem::replace(&mut h.baseline, cur));
        while h.undo.len() > depth {
            h.undo.pop_front();
        }
        h.redo.clear();
        h.edited_during_hold = false;
    }
}

/// DUPLICATE the selected object (v0.600, Ctrl+D): clone it offset +1 m in X/Z and select the
/// copy, so you can stamp many. Handles a structure / light / wall / direct machine (the common
/// placeables); a road/conduit node or an array-machine is skipped (graph nodes + array-derived
/// instances aren't duplicated this way). Marks the right dirty flag so the copy renders live.
pub(crate) fn construction_duplicate(state: &mut EngineState) {
    let g = &mut state.gui_state;
    if let Some(i) = g.construction_structure_selected {
        let mut new_idx = None;
        if let Some(hs) = zone_body_mut(&mut g.ship_structure, g.construction_zone) {
            if let Some(mut np) = hs.structures.get(i).cloned() {
                np.pos.0 += 1.0;
                np.pos.2 += 1.0;
                np.pair = None; // a copy starts unpaired
                hs.structures.push(np);
                new_idx = Some(hs.structures.len() - 1);
            }
        }
        if let Some(ni) = new_idx {
            g.construction_structure_selected = Some(ni);
        }
        g.construction_structure_dirty = true;
        return;
    }
    if let Some(i) = g.construction_light_selected {
        let mut new_idx = None;
        if let Some(hs) = zone_body_mut(&mut g.ship_structure, g.construction_zone) {
            if let Some(mut nl) = hs.lights.get(i).cloned() {
                nl.pos.0 += 1.0;
                nl.pos.2 += 1.0;
                hs.lights.push(nl);
                new_idx = Some(hs.lights.len() - 1);
            }
        }
        if let Some(ni) = new_idx {
            g.construction_light_selected = Some(ni);
        }
        g.construction_structure_dirty = true;
        return;
    }
    if let Some(i) = g.construction_wall_selected {
        let mut new_idx = None;
        if let Some(hs) = zone_body_mut(&mut g.ship_structure, g.construction_zone) {
            if let Some(mut nw) = hs.walls.get(i).cloned() {
                nw.a.0 += 1.0;
                nw.a.1 += 1.0;
                nw.b.0 += 1.0;
                nw.b.1 += 1.0;
                hs.walls.push(nw);
                new_idx = Some(hs.walls.len() - 1);
            }
        }
        if let Some(ni) = new_idx {
            g.construction_wall_selected = Some(ni);
        }
        g.construction_structure_dirty = true;
        return;
    }
    if let Some(id) = g.construction_machine_selected.clone() {
        let mut new_id = None;
        if let Some(h) = g.home_machines.as_mut() {
            if let Some(mut ni) = h.instances.iter().find(|m| m.id == id).cloned() {
                let fresh = h.unique_instance_id(&ni.machine);
                ni.id = fresh.clone();
                ni.offset.0 += 1.0;
                ni.offset.2 += 1.0;
                h.instances.push(ni);
                new_id = Some(fresh);
            }
        }
        if let Some(nid) = new_id {
            g.construction_machine_selected = Some(nid);
        }
        g.construction_machines_dirty = true;
    }
}

/// Undo the last construction edit (v0.575): restore the most recent pre-edit snapshot.
pub(crate) fn construction_undo(state: &mut EngineState) {
    if let Some(prev) = state.construction_history.undo.pop_back() {
        let cur = editor_snapshot(state);
        state.construction_history.redo.push(cur);
        state.construction_history.baseline = prev.clone();
        editor_restore(state, prev);
    }
}

/// Redo the last undone construction edit (v0.575).
pub(crate) fn construction_redo(state: &mut EngineState) {
    if let Some(next) = state.construction_history.redo.pop() {
        let cur = editor_snapshot(state);
        state.construction_history.undo.push_back(cur);
        state.construction_history.baseline = next.clone();
        editor_restore(state, next);
    }
}

/// Rebuild ONLY the home machine meshes + floating labels from the live editor state
/// (gui_state.home_machines + room_bounds), so a construction-editor edit (move/add/remove/
/// connect) shows immediately instead of only on the next world entry. Positions come from the
/// tested MachineHome::placements. The live ECS is kept in sync too as of v0.730 (see
/// sync_machine_entities above); the connection pipes rebuild via rebuild_connection_objects.
/// Flush unsaved ship-structure + machine edits to disk (v0.791). Runs on the 60 s
/// autosave tick and on window close, so build edits survive a quit without the
/// explicit Save button (before this, ONLY the button persisted them -- inventory
/// autosaved but the ship didn't, which the operator read as "saves aren't saving").
/// Differences from the button on purpose: no spawn stamp (build_char_pos is only
/// meaningful while the build editor is open) and no corridor pruning (a mid-edit
/// broken corridor row must survive the autosave; load() prunes resiliently now).
pub(crate) fn autosave_ship_structure(state: &mut EngineState, force: bool) {
    if !state.gui_state.construction_unsaved {
        return;
    }
    // Self-throttle to once per 60 s (same pattern as save_load's periodic
    // save); `force` (window close) flushes immediately.
    static LAST_SECS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let last = LAST_SECS.load(std::sync::atomic::Ordering::Relaxed);
    if !force {
        if last == 0 {
            // First armed frame: start the clock, don't save mid-edit instantly.
            LAST_SECS.store(now, std::sync::atomic::Ordering::Relaxed);
            return;
        }
        if now.saturating_sub(last) < 60 {
            return;
        }
    }
    LAST_SECS.store(now, std::sync::atomic::Ordering::Relaxed);
    state.gui_state.construction_unsaved = false;
    if let Some(ship) = &state.gui_state.ship_structure {
        let path = state.data_dir.join("blueprints").join("ship_structure.ron");
        match ship.save(&path) {
            Ok(()) => log::info!("Autosave: ship structure written to ship_structure.ron"),
            Err(e) => log::warn!("Autosave: ship structure save failed: {e}"),
        }
    }
    if let Some(home) = &state.gui_state.home_machines {
        let path = crate::machines::home_ron_path(&state.data_dir);
        match home.save(&path) {
            Ok(()) => log::info!("Autosave: machine layout written to {}", path.display()),
            Err(e) => log::warn!("Autosave: machine save failed: {e}"),
        }
    }
}

/// The slide-gizmo handles for the currently-selected room, with each handle's owning
/// `construction_rooms` index resolved (so a drag writes the offset back to the mirror).
/// Empty when nothing is selected. (v0.468)
pub(crate) fn selected_room_handles(state: &EngineState)
    -> Vec<(usize, crate::ship::fibonacci::OpeningHandle)> {
    let Some(sel) = state.gui_state.construction_selected_room else { return Vec::new(); };
    let Some(sel_room) = state.gui_state.construction_rooms.get(sel) else { return Vec::new(); };
    let sel_id = sel_room.id.clone();
    let Some(layout) = &state.homestead_layout else { return Vec::new(); };
    let positions = crate::ship::fibonacci::resolve_positions(layout);
    crate::ship::fibonacci::opening_handles(layout, &positions)
        .into_iter()
        .filter(|h| layout.rooms.get(h.room_index).map_or(false, |r| r.id == sel_id))
        .map(|h| (sel, h)) // all belong to the selected room -> selected mirror index
        .collect()
}

/// Left-click in the construction astral editor: cast a pick ray from the cursor. First try
/// the selected room's door/window slide handles (so they take precedence over the room
/// grab); otherwise hit-test each room's floor rectangle, select + grab the nearest. (v0.466)
pub(crate) fn try_begin_room_grab(state: &mut EngineState) {
    let sz = state.window.inner_size();
    let viewport = (sz.width as f32, sz.height as f32);
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, viewport);
    // 1. Opening gizmo handles of the selected room. Walls are VERTICAL, so intersect the
    //    pick ray with each handle's wall-FACE plane (no dir.y needed) and classify the
    //    nearest of {Move, and -- for a placed resizable opening -- the edge handles}. (v0.469)
    let handles = selected_room_handles(state);
    // (ri, handle, role, dist)
    let mut best_h: Option<(usize, crate::ship::fibonacci::OpeningHandle, GizmoRole, f32)> = None;
    for (ri, h) in &handles {
        let denom = dir.dot(h.n);
        if denom.abs() < 1e-6 { continue; } // ray parallel to the wall face
        let t = (h.wall_start - origin).dot(h.n) / denom;
        if t <= 0.0 { continue; } // plane behind the camera
        let hit = origin + dir * t;
        // Move always; placed openings add edge handles (width-only when floor-snapped).
        let mut cands: Vec<(GizmoRole, Vec3)> = vec![(GizmoRole::Move, h.base_center)];
        if h.opening_index.is_some() {
            cands.push((GizmoRole::ResizeLeft, h.handle_left));
            cands.push((GizmoRole::ResizeRight, h.handle_right));
            if !h.kind.floor_snapped() {
                cands.push((GizmoRole::ResizeBottom, h.handle_bottom));
                cands.push((GizmoRole::ResizeTop, h.handle_top));
            }
        }
        for (role, p) in cands {
            let d = (hit - p).length();
            let pick_r = if role == GizmoRole::Move { 0.3 } else { 0.18 };
            if d <= pick_r && best_h.map_or(true, |b| d < b.3) {
                best_h = Some((*ri, *h, role, d));
            }
        }
    }
    if let Some((ri, h, role, _)) = best_h {
        state.construction_gizmo_grab = Some(ConstructionGizmoGrab {
            room_index: ri,
            opening_index: h.opening_index,
            wall_index: h.wall_index,
            role,
            snap_floor: h.kind.floor_snapped(),
            wall_start: h.wall_start,
            u_hat: h.u_hat,
            n: h.n,
            wall_len: h.wall_len,
            wall_height: h.wall_height,
            base_t: h.base_t,
            grab_u: h.u,
            grab_v: h.v,
            grab_w: h.w,
            grab_h: h.h,
        });
        return; // grabbed a handle; don't also grab the room
    }
    // 2. Nearest room floor rect (needs dir.y; a horizontal-ish ray can't hit the floor plane).
    if dir.y.abs() < 1e-6 {
        return;
    }
    let mut best: Option<(usize, f32, f32, f32)> = None; // (rb_index, t, hit_x, hit_z)
    for (i, rb) in state.gui_state.room_bounds.iter().enumerate() {
        let t = (rb.min.y - origin.y) / dir.y;
        if t <= 0.0 {
            continue;
        }
        let hx = origin.x + dir.x * t;
        let hz = origin.z + dir.z * t;
        if hx >= rb.min.x && hx <= rb.max.x && hz >= rb.min.z && hz <= rb.max.z {
            if best.map_or(true, |(_, bt, _, _)| t < bt) {
                best = Some((i, t, hx, hz));
            }
        }
    }
    let Some((rb_index, _, hit_x, hit_z)) = best else {
        state.gui_state.construction_selected_room = None; // clicked empty space
        return;
    };
    // room_bounds and construction_rooms cross-walk by id.
    let id = state.gui_state.room_bounds[rb_index].id.clone();
    let Some(ri) = state.gui_state.construction_rooms.iter().position(|r| r.id == id) else {
        return;
    };
    let pos = state.gui_state.construction_rooms[ri].position.unwrap_or([0.0, 0.0, 0.0]);
    state.gui_state.construction_selected_room = Some(ri);
    state.construction_grab = Some(ConstructionGrab {
        room_index: ri,
        floor_y: state.gui_state.room_bounds[rb_index].min.y,
        offset_x: hit_x - pos[0],
        offset_z: hit_z - pos[2],
    });
}

/// Cast a ray from the cursor onto the room floors; return (room_bounds index, hit_x, hit_z) of
/// the nearest room under the cursor. Used by ghost placement (v0.529).
pub(crate) fn cursor_floor_hit(state: &EngineState) -> Option<(usize, f32, f32)> {
    let sz = state.window.inner_size();
    let (origin, dir) =
        state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    if dir.y.abs() < 1e-6 {
        return None;
    }
    let mut best: Option<(usize, f32, f32, f32)> = None; // (i, t, hx, hz)
    for (i, rb) in state.gui_state.room_bounds.iter().enumerate() {
        let t = (rb.min.y - origin.y) / dir.y;
        if t <= 0.0 {
            continue;
        }
        let hx = origin.x + dir.x * t;
        let hz = origin.z + dir.z * t;
        if hx >= rb.min.x && hx <= rb.max.x && hz >= rb.min.z && hz <= rb.max.z {
            if best.map_or(true, |(_, bt, _, _)| t < bt) {
                best = Some((i, t, hx, hz));
            }
        }
    }
    best.map(|(i, _, hx, hz)| (i, hx, hz))
}

/// The construction editor's ACTIVE zone world origin (v0.754, ship-superstructure increment A):
/// gizmos add it to zone-local body coords; ray hits subtract it before writing body coords.
/// ZERO with no ship, so the legacy single-home math (world == box-local) is unchanged.
pub(crate) fn active_zone_origin(state: &EngineState) -> Vec3 {
    zone_origin(&state.gui_state.ship_structure, state.gui_state.construction_zone)
}

/// The active zone's id, for tagging newly placed machines (v0.754). "home" without a ship.
pub(crate) fn active_zone_id(state: &EngineState) -> String {
    state
        .gui_state
        .ship_structure
        .as_ref()
        .and_then(|s| s.zones.get(state.gui_state.construction_zone))
        .map(|z| z.id.clone())
        .unwrap_or_else(|| "home".to_string())
}

/// Drop the currently-held palette machine where the cursor hits a room floor. Keeps the item
/// held so you can place several; right-click or re-click the palette item to stop. Appears live
/// via construction_machines_dirty. (v0.529; v0.538: box mode stores ABSOLUTE coords)
pub(crate) fn try_place_held_machine(state: &mut EngineState) {
    let Some(mtype) = state.gui_state.construction_place_type.clone() else {
        return;
    };
    let Some((rb_i, hx, hz)) = cursor_floor_hit(state) else {
        return;
    };
    let rb = &state.gui_state.room_bounds[rb_i];
    let room_id = rb.id.clone();
    // v0.538: in a box home, store the ABSOLUTE world floor-hit so the machine survives
    // flood-fill room-id churn. The legacy ship layout keeps the room-center-relative offset.
    // v0.754: the machine is tagged with the ACTIVE zone (the zone selector's choice) and
    // placement clamps it into that zone's footprint -- so tools operate on the selected zone.
    let box_mode = state.gui_state.ship_structure.is_some();
    let zone = active_zone_id(state);
    let offset = if box_mode {
        (hx, 0.0, hz)
    } else {
        let cx = (rb.min.x + rb.max.x) * 0.5;
        let cz = (rb.min.z + rb.max.z) * 0.5;
        (hx - cx, 0.0, hz - cz)
    };
    if let Some(home) = state.gui_state.home_machines.as_mut() {
        if home.catalog.contains_key(&mtype) {
            let id = home.unique_instance_id(&mtype);
            home.instances.push(crate::machines::MachineInstance {
                id,
                machine: mtype,
                room: room_id,
                offset,
                rotation: 0.0,
                zone,
            });
            state.gui_state.construction_machines_dirty = true;
        }
    }
}

/// Drop the currently-held LIGHT type (v0.784, the palette's Lights category)
/// at the cursor floor point, zone-local to the ACTIVE zone. Ceiling types
/// (panels/spot/strip) hang just under the zone's ceiling; the warm lamp sits
/// at table height. The new light is auto-selected so its detail panel opens
/// for tuning. Stays held so you can run a row of lights; right-click cancels.
pub(crate) fn try_place_held_light(state: &mut EngineState) {
    let Some(tid) = state.gui_state.construction_place_light.clone() else {
        return;
    };
    let Some((_, hx, hz)) = cursor_floor_hit(state) else {
        return;
    };
    let zo = active_zone_origin(state);
    let Some(hs) = crate::ship::ship_structure::zone_body_mut(
        &mut state.gui_state.ship_structure,
        state.gui_state.construction_zone,
    ) else {
        return;
    };
    let y = if tid == "warm_lamp" {
        0.8
    } else {
        (hs.height - 0.3).max(0.3)
    };
    hs.lights.push(crate::ship::home_structure::PlacedLight {
        type_id: tid,
        pos: (hx - zo.x, y, hz - zo.z),
        dir: (0.0, -1.0, 0.0),
        on: true,
        color: None,
        intensity: None,
        range: None,
        path: Vec::new(),
        subdivision: crate::ship::home_structure::default_strip_subdivision(),
    });
    state.gui_state.construction_light_selected = Some(hs.lights.len() - 1);
    state.gui_state.construction_structure_dirty = true;
}

/// Drop the currently-held STRUCTURAL piece (stairs/ladder/elevator/...) where the cursor hits a
/// room floor (v0.583). Stores a ZONE-LOCAL pose (the active zone's box min at its origin) at the
/// floor height, with the current placement yaw. Stays held so you can place several;
/// right-click cancels.
pub(crate) fn try_place_structure(state: &mut EngineState) {
    let Some(tid) = state.gui_state.construction_structure_type.clone() else {
        return;
    };
    if crate::ship::structure::structure_type(&tid).is_none() {
        return;
    }
    let Some((rb_i, hx, hz)) = cursor_floor_hit(state) else {
        return;
    };
    let zo = active_zone_origin(state); // world floor hit -> active-zone-local body coords (v0.754)
    let floor_y = state.gui_state.room_bounds[rb_i].min.y;
    let place_y = floor_y + state.gui_state.construction_structure_place_y.max(0.0);
    if let Some(hs) = zone_body_mut(&mut state.gui_state.ship_structure, state.gui_state.construction_zone) {
        hs.structures.push(crate::ship::home_structure::PlacedStructure {
            type_id: tid,
            pos: (hx - zo.x, place_y - zo.y, hz - zo.z),
            rot_deg: state.gui_state.construction_structure_yaw,
            pair: None,
        });
        state.gui_state.construction_structure_dirty = true;
    }
}

/// Drop a corner node while drawing an interior wall (v0.534). The first click sets the wall's
/// start corner; the second click adds a wall segment from the start to here and CHAINS (the new
/// corner becomes the next start), so you can walk a whole floor plan with successive clicks. The
/// point comes from the floor raycast, snapped to 0.25 m, converted into the ACTIVE zone's local
/// coords (v0.754: world x/z equals box-local x/z only for a zone at the world origin).
pub(crate) fn try_place_wall_node(state: &mut EngineState) {
    let Some((_, hx, hz)) = cursor_floor_hit(state) else {
        return;
    };
    let zo = active_zone_origin(state);
    let (hx, hz) = (hx - zo.x, hz - zo.z); // active-zone-local (v0.754)
    // v0.541: snap a drawn corner to an existing corner / the box edge / the grid (same rules as
    // dragging), so successive walls share corners + reach the perimeter for an airtight seal.
    // (NaN "grabbed" sentinel skips nothing, so a new corner CAN snap onto an existing one.)
    let grid = state.gui_state.construction_grid_snap;
    let p = match zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
        Some(hs) => snap_node_position(hs, (f32::NAN, f32::NAN), (hx, hz), grid),
        None => ((hx * 4.0).round() / 4.0, (hz * 4.0).round() / 4.0),
    };
    match state.gui_state.construction_wall_start {
        None => state.gui_state.construction_wall_start = Some(p),
        Some(start) => {
            // Ignore a zero-length segment (a double-click on the same spot).
            if (start.0 - p.0).abs() > 0.05 || (start.1 - p.1).abs() > 0.05 {
                if let Some(hs) = zone_body_mut(&mut state.gui_state.ship_structure, state.gui_state.construction_zone) {
                    let height = hs.height;
                    let material = hs.shell_material;
                    hs.walls.push(crate::ship::home_structure::InteriorWall {
                        a: start,
                        b: p,
                        height,
                        material,
                        openings: Vec::new(),
                        thickness: None,
                        layers: Vec::new(),
                    });
                    state.gui_state.construction_structure_dirty = true;
                    state.gui_state.construction_wall_selected = Some(hs.walls.len() - 1);
                    state.gui_state.construction_machine_selected = None; // keep selection exclusive
                }
                state.gui_state.construction_wall_start = Some(p); // chain into the next segment
            }
        }
    }
}

/// Which build-mode gizmo the cursor is hovering this frame (v0.569), for the hover highlight.
/// Mirrors the grab picks (try_grab_node/_char + the opening pick) but is read-only and picks the
/// NEAREST gizmo across all three kinds. Returns None while drawing a wall, holding a machine, or
/// already dragging (the grabbed one is highlighted instead). Generous pick radii since the orbs
/// are tiny (0.05 m).
pub(crate) fn compute_construction_hover(state: &EngineState) -> HoverGizmo {
    if !state.gui_state.construction_active
        || state.gui_state.construction_wall_mode
        || state.gui_state.construction_place_type.is_some()
        || state.gui_state.construction_structure_type.is_some()
        || state.construction_node_grab.is_some()
        || state.construction_object_grab.is_some()
        || state.construction_opening_grab.is_some()
        || state.construction_char_grab
    {
        return HoverGizmo::None;
    }
    let Some(hs) = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) else {
        return HoverGizmo::None;
    };
    let sz = state.window.inner_size();
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    // Closest approach of the ray to a point p, returning its forward distance t if within pick_r.
    let test = |p: Vec3, pick_r: f32| -> Option<f32> {
        let t = (p - origin).dot(dir);
        if t < 0.0 {
            return None;
        }
        if (p - (origin + dir * t)).length() < pick_r { Some(t) } else { None }
    };
    let zo = active_zone_origin(state); // gizmos live at zone-local + origin (v0.754)
    let mut best_t = f32::INFINITY;
    let mut best = HoverGizmo::None;
    for c in unique_corners(hs) {
        if let Some(t) = test(Vec3::new(c.0 + zo.x, zo.y - 0.05, c.1 + zo.z), 0.45) {
            if t < best_t {
                best_t = t;
                best = HoverGizmo::Corner(c.0, c.1);
            }
        }
    }
    for (idx, p) in opening_gizmos(hs, zo) {
        if let Some(t) = test(p, 0.4) {
            if t < best_t {
                best_t = t;
                best = HoverGizmo::Opening(idx.0, idx.1);
            }
        }
    }
    if let Some((cx, cz)) = state.gui_state.build_char_pos {
        if let Some(t) = test(Vec3::new(cx + zo.x, zo.y + 0.7, cz + zo.z), 0.7) {
            if t < best_t {
                best = HoverGizmo::Char;
            }
        }
    }
    best
}

/// On a build-mode click, try to grab the nearest corner-node gizmo under the cursor (ray vs the
/// pin position). Returns true if a node was grabbed. (v0.541)
pub(crate) fn try_grab_node(state: &mut EngineState) -> bool {
    // Compute the gizmo set as owned values so the home_structure borrow ends before the
    // mutable grab assignment below.
    let (top_y, corners) = match zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
        Some(hs) => (-0.05, unique_corners(hs)), // orb centre (top-at-floor); matches the render (v0.568)
        None => return false,
    };
    let zo = active_zone_origin(state); // corners are zone-local; the orb renders at +origin (v0.754)
    let sz = state.window.inner_size();
    // pick_ray already returns a unit dir (or zero for a degenerate ray); re-normalizing a zero
    // vector would be NaN, so use it as-is. (v0.542)
    let (origin, dir) =
        state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    let mut best: Option<((f32, f32), f32)> = None;
    for c in &corners {
        let p = Vec3::new(c.0 + zo.x, zo.y + top_y, c.1 + zo.z);
        let t = (p - origin).dot(dir);
        if t < 0.0 {
            continue; // behind the camera
        }
        let dd = (p - (origin + dir * t)).length();
        if dd < 0.7 && best.map_or(true, |(_, b)| dd < b) {
            best = Some((*c, dd));
        }
    }
    if let Some((c, _)) = best {
        state.construction_node_grab = Some(c);
        state.construction_grab_press = Some(state.cursor_pos); // tap-vs-drag (v0.549)
        true
    } else {
        false
    }
}

/// Hit-test the cursor ray against the placed machines (v0.553). On a hit, SELECT the nearest one
/// (its detail shows on the right panel) and clear any wall selection; returns true so the click
/// does not also start a room grab. Build mode only.
pub(crate) fn try_pick_machine(state: &mut EngineState) -> bool {
    if state.machine_pick.is_empty() {
        return false;
    }
    let sz = state.window.inner_size();
    let (origin, dir) =
        state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    let mut best: Option<(String, f32)> = None;
    for (id, center, radius) in &state.machine_pick {
        let t = (*center - origin).dot(dir);
        if t < 0.0 {
            continue; // behind the camera
        }
        let dd = (*center - (origin + dir * t)).length();
        // Within the machine's bounding radius; keep the one nearest the camera (smallest t).
        if dd < *radius && best.as_ref().map_or(true, |(_, bt)| t < *bt) {
            best = Some((id.clone(), t));
        }
    }
    if let Some((id, _)) = best {
        // Arm a drag (v0.593): click-and-hold the machine to move it (keeping its height).
        state.construction_object_grab = Some(ObjectGrab::Machine(id.clone()));
        state.construction_grab_press = Some(state.cursor_pos);
        state.gui_state.construction_machine_selected = Some(id);
        state.gui_state.construction_wall_selected = None;
        state.gui_state.construction_light_selected = None;
        state.gui_state.construction_structure_selected = None;
        true
    } else {
        false
    }
}

/// Hit-test the cursor ray against the SELECTED machine's PORT gizmos (v0.625, viewport drag-to-
/// connect). On a hit, arm a port drag: a rubber-band line then follows the cursor and releasing
/// over a machine with a compatible port wires them (see the release handler). Only the selected
/// machine's ports are grab-able (they're the only ones rendered), so this never fights the machine
/// pick. Returns true on a hit so the click doesn't also start moving the machine.
pub(crate) fn try_pick_port(state: &mut EngineState) -> bool {
    let Some(sel) = state.gui_state.construction_machine_selected.clone() else {
        return false;
    };
    if state.port_pick.is_empty() {
        return false;
    }
    let sz = state.window.inner_size();
    let (origin, dir) =
        state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    let mut best: Option<(f32, crate::utilities::Utility, crate::utilities::PortDir, Vec3)> = None;
    for (mid, _idx, port, wp) in &state.port_pick {
        if mid != &sel {
            continue;
        }
        let t = (*wp - origin).dot(dir);
        if t < 0.0 {
            continue;
        }
        // Generous 0.22 m pick radius (the gizmos are small, the operator wanted easy grabbing).
        if (*wp - (origin + dir * t)).length() < 0.22 && best.as_ref().map_or(true, |(bt, ..)| t < *bt) {
            best = Some((t, port.utility, port.dir, *wp));
        }
    }
    if let Some((_, util, pdir, wp)) = best {
        state.construction_port_drag = Some((sel, util, pdir, wp));
        state.construction_grab_press = Some(state.cursor_pos);
        true
    } else {
        false
    }
}

/// Hit-test the cursor ray against the routed PIPES (v0.626): samples each machine-machine
/// connection's polyline (from `connection_flow_paths`) and selects the nearest within a small
/// radius, so a pipe is a clickable object like a wall -- its detail + a Remove button then show on
/// the right panel. Machine-machine wires only for now (conduit-node edges come with the node tool).
/// Returns true on a hit so the click doesn't fall through to a wall pick / room grab.
pub(crate) fn try_pick_connection(state: &mut EngineState) -> bool {
    if state.connection_flow_paths.is_empty() {
        return false;
    }
    let sz = state.window.inner_size();
    let (origin, dir) =
        state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    let mut best: Option<(f32, String, String)> = None; // (t, from, to)
    for (path, from_id, to_id) in &state.connection_flow_paths {
        if from_id.starts_with("node:") || to_id.starts_with("node:") {
            continue; // machine-machine wires only (v0.626)
        }
        let mut hit_t = f32::INFINITY;
        for seg in path.windows(2) {
            let (p, q) = (seg[0], seg[1]);
            let len = (q - p).length();
            let steps = (len / 0.35).ceil().max(1.0) as usize;
            for s in 0..=steps {
                let pt = p + (q - p) * (s as f32 / steps as f32);
                let t = (pt - origin).dot(dir);
                if t < 0.0 {
                    continue;
                }
                let d = (pt - (origin + dir * t)).length();
                if d < 0.28 && t < hit_t {
                    hit_t = t;
                }
            }
        }
        if hit_t.is_finite() && best.as_ref().map_or(true, |(bt, ..)| hit_t < *bt) {
            best = Some((hit_t, from_id.clone(), to_id.clone()));
        }
    }
    if let Some((_, from, to)) = best {
        let g = &mut state.gui_state;
        g.construction_connection_selected = Some((from, to));
        g.construction_machine_selected = None;
        g.construction_wall_selected = None;
        g.construction_light_selected = None;
        g.construction_structure_selected = None;
        g.construction_road_node_selected = None;
        g.construction_conduit_node_selected = None;
        true
    } else {
        false
    }
}

/// Hit-test the cursor ray against the ZONE boxes (v0.634): ray-vs-AABB (slab method), nearest box
/// entered. On a hit, SELECT the zone (its detail shows on the right + it highlights) and arm a floor
/// drag. Runs LAST in the pick chain (before the room grab) so a zone -- a big background volume --
/// never steals a click from a machine / wall / node in front of it. Returns true on a hit.
pub(crate) fn try_pick_zone(state: &mut EngineState) -> bool {
    let zones: Vec<(String, (f32, f32, f32), (f32, f32, f32))> = match zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
        Some(hs) => hs.zones.iter().map(|z| (z.id.clone(), z.origin, z.size)).collect(),
        None => return false,
    };
    if zones.is_empty() {
        return false;
    }
    let zo = active_zone_origin(state); // intra-zone volumes are zone-local (v0.754)
    let sz = state.window.inner_size();
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    let inv = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z); // 0 component -> inf, slab-safe
    let mut best: Option<(String, f32)> = None;
    for (id, o, s) in &zones {
        let mn = Vec3::new(o.0, o.1, o.2) + zo;
        let mx = Vec3::new(o.0 + s.0, o.1 + s.1, o.2 + s.2) + zo;
        let t1 = (mn - origin) * inv;
        let t2 = (mx - origin) * inv;
        let entry = t1.min(t2).max_element();
        let exit = t1.max(t2).min_element();
        if exit >= entry.max(0.0) {
            let t = entry.max(0.0);
            if best.as_ref().map_or(true, |(_, bt)| t < *bt) {
                best = Some((id.clone(), t));
            }
        }
    }
    if let Some((id, _)) = best {
        // Drag guard (v0.789, operator incident): only arm the FLOOR DRAG when
        // the click lands near the zone's PERIMETER (a 2 m band in plan view).
        // A click deep inside still SELECTS (detail panel + highlight), but a
        // giant region zone that underlies the whole ship no longer gets
        // yanked 30 m sideways because a bare floor click grabbed its body --
        // the operator clicked his home floor and dragged the 120x200 m
        // Residential zone through the house.
        let near_edge = zones
            .iter()
            .find(|(zid, _, _)| zid == &id)
            .map(|(_, o, s)| {
                // Where the pick ray meets the zone's floor plane, in plan view.
                let mn = Vec3::new(o.0, o.1, o.2) + zo;
                let mx = Vec3::new(o.0 + s.0, o.1 + s.1, o.2 + s.2) + zo;
                let t = if dir.y.abs() > 1e-4 { (mn.y - origin.y) / dir.y } else { -1.0 };
                if t <= 0.0 {
                    return true; // grazing ray: keep the old grab behavior
                }
                let hit = origin + dir * t;
                let d_edge = (hit.x - mn.x)
                    .min(mx.x - hit.x)
                    .min(hit.z - mn.z)
                    .min(mx.z - hit.z);
                d_edge <= 2.0
            })
            .unwrap_or(true);
        let g = &mut state.gui_state;
        g.construction_zone_selected = Some(id.clone());
        g.construction_machine_selected = None;
        g.construction_wall_selected = None;
        g.construction_light_selected = None;
        g.construction_structure_selected = None;
        g.construction_road_node_selected = None;
        g.construction_conduit_node_selected = None;
        g.construction_connection_selected = None;
        if near_edge {
            state.construction_object_grab = Some(ObjectGrab::Zone(id));
            state.construction_grab_press = Some(state.cursor_pos);
        }
        true
    } else {
        false
    }
}

/// Hit-test the cursor ray against the WALL SURFACES (v0.573). On a hit, SELECT that wall (its
/// corners/openings show on the right panel) -- so clicking anywhere on a wall's face picks it,
/// unambiguously, instead of having to click a shared corner orb at a multi-wall intersection.
/// Each interior wall is a vertical slab; we intersect the ray with its centre plane and check the
/// hit lies within the wall's length + height. Returns true (so the click doesn't also grab a room).
pub(crate) fn try_pick_wall(state: &mut EngineState) -> bool {
    let zo = active_zone_origin(state); // wall coords are zone-local; test in world (v0.754)
    let walls: Vec<(usize, Vec3, Vec3, f32)> = match zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
        Some(hs) => hs
            .walls
            .iter()
            .enumerate()
            .map(|(i, w)| (i, Vec3::new(w.a.0, 0.0, w.a.1) + zo, Vec3::new(w.b.0, 0.0, w.b.1) + zo, w.height))
            .collect(),
        None => return false,
    };
    let sz = state.window.inner_size();
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    let mut best: Option<(usize, f32)> = None; // (wall index, ray t)
    for (i, a, b, h) in &walls {
        let along = *b - *a;
        let len = along.length();
        if len < 1e-4 {
            continue;
        }
        let along_n = along / len;
        // Horizontal normal of the (vertical) wall plane.
        let normal = Vec3::new(-along_n.z, 0.0, along_n.x);
        let denom = dir.dot(normal);
        if denom.abs() < 1e-6 {
            continue; // ray parallel to the wall face
        }
        let t = (*a - origin).dot(normal) / denom;
        if t < 0.0 {
            continue; // behind the camera
        }
        let hit = origin + dir * t;
        let s = (hit - *a).dot(along_n); // distance along the wall from a
        // Height window relative to the zone's deck (a.y == the zone origin y, v0.754).
        if s >= -0.1 && s <= len + 0.1 && hit.y >= a.y - 0.1 && hit.y <= a.y + *h + 0.1 {
            if best.map_or(true, |(_, bt)| t < bt) {
                best = Some((*i, t));
            }
        }
    }
    if let Some((i, _)) = best {
        state.gui_state.construction_wall_selected = Some(i);
        state.gui_state.construction_machine_selected = None;
        state.gui_state.construction_light_selected = None;
        true
    } else {
        false
    }
}

/// Hit-test the cursor ray against the placed-LIGHT diamond gizmos (v0.576). On a hit, SELECT that
/// light (its detail shows on the right panel, like a wall). Returns true so the click doesn't also
/// pick a wall / grab a room.
pub(crate) fn try_pick_light(state: &mut EngineState) -> bool {
    let zo = active_zone_origin(state); // lights are zone-local; test in world (v0.754)
    let lights: Vec<(usize, Vec3)> = match zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
        Some(hs) => hs
            .lights
            .iter()
            .enumerate()
            .map(|(i, l)| (i, Vec3::new(l.pos.0, l.pos.1, l.pos.2) + zo))
            .collect(),
        None => return false,
    };
    if lights.is_empty() {
        return false;
    }
    let sz = state.window.inner_size();
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    let mut best: Option<(usize, f32)> = None;
    for (i, p) in &lights {
        let t = (*p - origin).dot(dir);
        if t < 0.0 {
            continue;
        }
        let dd = (*p - (origin + dir * t)).length();
        if dd < 0.4 && best.map_or(true, |(_, bt)| t < bt) {
            best = Some((*i, t));
        }
    }
    if let Some((i, _)) = best {
        // Arm a drag (v0.593): click-and-hold the diamond to move the light (keeping its height).
        state.construction_object_grab = Some(ObjectGrab::Light(i));
        state.construction_grab_press = Some(state.cursor_pos);
        state.gui_state.construction_light_selected = Some(i);
        state.gui_state.construction_wall_selected = None;
        state.gui_state.construction_machine_selected = None;
        state.gui_state.construction_structure_selected = None;
        true
    } else {
        false
    }
}

/// Hit-test the cursor ray against the corridor DOOR-MOUTH handles (v0.790, operator: "I don't
/// see any widgets/gizmos on the door to actually move the door"): the accent diamonds at each
/// end of every corridor that resolves. On a hit, arm a CorridorMouth drag -- the per-frame
/// drag slides the corridor's world `lat` across the run. Corridors are SHIP-level, so there
/// is no zone-origin shift here: `end_from`/`end_to` are already world coordinates.
pub(crate) fn try_pick_corridor_mouth(state: &mut EngineState) -> bool {
    let handles: Vec<(usize, Vec3)> = state
        .gui_state
        .ship_structure
        .as_ref()
        .map(|s| {
            s.corridors
                .iter()
                .enumerate()
                .filter_map(|(ci, c)| s.corridor_geometry(c).ok().map(|g| (ci, g)))
                .flat_map(|(ci, g)| {
                    let lift = Vec3::Y * CORRIDOR_MOUTH_HANDLE_LIFT;
                    [(ci, g.end_from + lift), (ci, g.end_to + lift)]
                })
                .collect()
        })
        .unwrap_or_default();
    if handles.is_empty() {
        return false;
    }
    let sz = state.window.inner_size();
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    let mut best: Option<(usize, f32)> = None;
    for (ci, p) in &handles {
        let t = (*p - origin).dot(dir);
        if t < 0.0 {
            continue; // behind the camera
        }
        let dd = (*p - (origin + dir * t)).length();
        // Generous 0.5 m pick radius (mirrors the opening move-cube), nearest to the camera wins.
        if dd < 0.5 && best.map_or(true, |(_, bt)| t < bt) {
            best = Some((*ci, t));
        }
    }
    if let Some((ci, _)) = best {
        state.construction_object_grab = Some(ObjectGrab::CorridorMouth(ci));
        state.construction_grab_press = Some(state.cursor_pos); // tap-vs-drag
        true
    } else {
        false
    }
}

/// Hit-test the cursor ray against the SELECTED strip light's control-point handles (v0.790,
/// operator: "add the widget/gizmo to the light strip sections so I don't have to adjust the
/// sliders"). Point 0 is the strip's start (`pos`); 1.. are `path[point - 1]` -- the exact
/// fields the right panel's DragValues edit, so viewport drags and panel edits stay in
/// lockstep. Runs BEFORE try_pick_light in the chain so the small handles beat the body
/// diamond. Only fires when the selected light is a strip (Bar).
pub(crate) fn try_pick_strip_point(state: &mut EngineState) -> bool {
    let Some(li) = state.gui_state.construction_light_selected else {
        return false;
    };
    let zo = active_zone_origin(state); // strip points are zone-local; test in world (v0.754)
    let pts: Vec<Vec3> = match zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone)
        .and_then(|h| h.lights.get(li))
    {
        Some(l)
            if crate::renderer::light::light_type(&l.type_id).map(|t| t.kind)
                == Some(crate::renderer::light::LightKind::Bar) =>
        {
            std::iter::once(Vec3::new(l.pos.0, l.pos.1, l.pos.2) + zo)
                .chain(l.path.iter().map(|p| Vec3::new(p.0, p.1, p.2) + zo))
                .collect()
        }
        _ => return false,
    };
    let sz = state.window.inner_size();
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    let mut best: Option<(usize, f32)> = None;
    for (pi, p) in pts.iter().enumerate() {
        let t = (*p - origin).dot(dir);
        if t < 0.0 {
            continue; // behind the camera
        }
        let dd = (*p - (origin + dir * t)).length();
        if dd < 0.35 && best.map_or(true, |(_, bt)| t < bt) {
            best = Some((pi, t));
        }
    }
    if let Some((pi, _)) = best {
        state.construction_object_grab = Some(ObjectGrab::StripPoint { light: li, point: pi });
        state.construction_grab_press = Some(state.cursor_pos); // tap-vs-drag
        true
    } else {
        false
    }
}

/// Hit-test the cursor ray against the SELECTED light's RGB rotation rings (v0.790, operator:
/// "make it so the RGB colored rings around objects allow rotating the object... it'd make
/// adjusting spot lights way easier"; viewport-fixed handles v0.792). A hit arms a LightAim
/// drag: sweeping the cursor around the ring rotates the aim about that ring's axis
/// (0 = X/red, 1 = Y/green, 2 = Z/blue). EVERY light kind arms now (was Spot-only): a
/// pathless strip renders along its `dir`, so rotating it is real editing; on a point light
/// rotation is meaningless but harmless (per the operator, the rings stay for consistency).
pub(crate) fn try_pick_light_ring(state: &mut EngineState) -> bool {
    let Some(li) = state.gui_state.construction_light_selected else {
        return false;
    };
    let Some(axis) = light_ring_under_cursor(state) else {
        return false;
    };
    state.construction_object_grab = Some(ObjectGrab::LightAim { light: li, axis, prev_angle: None });
    state.construction_grab_press = Some(state.cursor_pos); // tap-vs-drag
    true
}

/// Hit-test the cursor ray against the placed STRUCTURE pieces (v0.583). On a hit, SELECT that
/// piece (its detail shows on the right panel). Uses a ray-vs-AABB test against each piece's
/// rotated bounding box so clicking the visible body (the elevator frame, the stair mass) selects
/// it. Returns true so the click doesn't also pick a wall / grab a room.
pub(crate) fn try_pick_structure(state: &mut EngineState) -> bool {
    use crate::ship::structure::{rotated_half_extents, structure_type, StructureKind};
    let zo = active_zone_origin(state); // pieces are zone-local; test in world (v0.754)
    let pieces: Vec<(usize, Vec3, Vec3)> = match zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
        Some(hs) => hs
            .structures
            .iter()
            .enumerate()
            .filter_map(|(i, ps)| {
                let ty = structure_type(&ps.type_id)?;
                if ty.kind == StructureKind::Wall {
                    return None;
                }
                let (hw, h, hd) = rotated_half_extents(ty, ps.rot_deg.to_radians());
                let min = Vec3::new(ps.pos.0 - hw, ps.pos.1, ps.pos.2 - hd) + zo;
                let max = Vec3::new(ps.pos.0 + hw, ps.pos.1 + h, ps.pos.2 + hd) + zo;
                Some((i, min, max))
            })
            .collect(),
        None => return false,
    };
    if pieces.is_empty() {
        return false;
    }
    let sz = state.window.inner_size();
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    let mut best: Option<(usize, f32)> = None;
    for (i, min, max) in &pieces {
        if let Some(t) = ray_aabb_hit(origin, dir, *min, *max) {
            if best.map_or(true, |(_, bt)| t < bt) {
                best = Some((*i, t));
            }
        }
    }
    if let Some((i, _)) = best {
        // Arm a drag (v0.593): click-and-hold the piece to move it (keeping its height).
        state.construction_object_grab = Some(ObjectGrab::Structure(i));
        state.construction_grab_press = Some(state.cursor_pos);
        state.gui_state.construction_structure_selected = Some(i);
        state.gui_state.construction_wall_selected = None;
        state.gui_state.construction_machine_selected = None;
        state.gui_state.construction_light_selected = None;
        true
    } else {
        false
    }
}

/// Hit-test the cursor ray against ROAD + CONDUIT (pipe) graph nodes (v0.599). On a hit, SELECT
/// the node (its detail shows on the right) + arm an object grab so click-and-hold drags it across
/// the floor (keeping a conduit node's height) -- the "dedicated widget gizmo like the walls" the
/// operator wanted for nodes. Returns true so the click doesn't also grab a room.
pub(crate) fn try_pick_node(state: &mut EngineState) -> bool {
    enum Sel {
        Road(u32),
        Conduit(String),
    }
    let zo = active_zone_origin(state); // road nodes are zone-local; conduit nodes world (v0.754)
    let sz = state.window.inner_size();
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    let mut best: Option<(f32, Sel)> = None;
    if let Some(hs) = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
        for n in &hs.road_nodes {
            let p = Vec3::new(n.pos.0 + zo.x, zo.y + 0.06, n.pos.1 + zo.z);
            let t = (p - origin).dot(dir);
            if t < 0.0 {
                continue;
            }
            if (p - (origin + dir * t)).length() < 0.5 && best.as_ref().map_or(true, |(bt, _)| t < *bt) {
                best = Some((t, Sel::Road(n.id)));
            }
        }
    }
    if let Some(h) = state.gui_state.home_machines.as_ref() {
        for cn in &h.conduit_nodes {
            let p = Vec3::new(cn.pos.0, cn.pos.1, cn.pos.2);
            let t = (p - origin).dot(dir);
            if t < 0.0 {
                continue;
            }
            if (p - (origin + dir * t)).length() < 0.5 && best.as_ref().map_or(true, |(bt, _)| t < *bt) {
                best = Some((t, Sel::Conduit(cn.id.clone())));
            }
        }
    }
    let Some((_, sel)) = best else {
        return false;
    };
    // Clear every selection, then set the picked node + arm its grab.
    state.gui_state.construction_wall_selected = None;
    state.gui_state.construction_structure_selected = None;
    state.gui_state.construction_light_selected = None;
    state.gui_state.construction_machine_selected = None;
    state.gui_state.construction_road_node_selected = None;
    state.gui_state.construction_conduit_node_selected = None;
    match sel {
        Sel::Road(id) => {
            state.gui_state.construction_road_node_selected = Some(id);
            state.construction_object_grab = Some(ObjectGrab::RoadNode(id));
        }
        Sel::Conduit(id) => {
            state.gui_state.construction_conduit_node_selected = Some(id.clone());
            state.construction_object_grab = Some(ObjectGrab::ConduitNode(id));
        }
    }
    state.construction_grab_press = Some(state.cursor_pos);
    true
}

/// Hit-test the cursor ray against the build-mode avatar (v0.557). On a hit, start dragging it;
/// returns true so the click doesn't also grab a room.
pub(crate) fn try_grab_char(state: &mut EngineState) -> bool {
    let Some((cx, cz)) = state.gui_state.build_char_pos else {
        return false;
    };
    let zo = active_zone_origin(state); // the avatar stands in the ACTIVE zone (v0.754)
    let sz = state.window.inner_size();
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    let c = Vec3::new(cx + zo.x, zo.y + 0.7, cz + zo.z); // mid-body
    let t = (c - origin).dot(dir);
    if t < 0.0 {
        return false;
    }
    if (c - (origin + dir * t)).length() < 0.8 {
        state.construction_char_grab = true;
        true
    } else {
        false
    }
}

/// Per-frame while the avatar is grabbed: move it to the cursor's floor hit, converted into the
/// ACTIVE zone's local coords (v0.754) and clamped into that zone's box.
pub(crate) fn apply_char_drag(state: &mut EngineState) {
    if let Some((_, hx, hz)) = cursor_floor_hit(state) {
        let zo = active_zone_origin(state);
        let (hx, hz) = (hx - zo.x, hz - zo.z);
        let (bw, bd) = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone)
            .map_or((1e6, 1e6), |hs| (hs.width, hs.depth));
        state.gui_state.build_char_pos = Some((hx.clamp(0.3, bw - 0.3), hz.clamp(0.3, bd - 0.3)));
    }
}

pub(crate) fn apply_node_drag(state: &mut EngineState) {
    let Some(grabbed) = state.construction_node_grab else {
        return;
    };
    // Tap-vs-drag (v0.549): hold the corner still until the cursor leaves the press point, so a
    // tap selects (handled on release) and only click-and-drag moves it.
    if let Some(press) = state.construction_grab_press {
        let d = ((state.cursor_pos.0 - press.0).powi(2) + (state.cursor_pos.1 - press.1).powi(2)).sqrt();
        if d < DRAG_THRESHOLD_PX {
            return;
        }
        state.construction_grab_press = None; // armed: this is now a drag
    }
    let Some((_, hx, hz)) = cursor_floor_hit(state) else {
        return;
    };
    // World floor hit -> the ACTIVE zone's local coords (the grabbed corner is local). (v0.754)
    let zo = active_zone_origin(state);
    let (hx, hz) = (hx - zo.x, hz - zo.z);
    let grid = state.gui_state.construction_grid_snap;
    let snapped = match zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
        Some(hs) => snap_node_position(hs, grabbed, (hx, hz), grid),
        None => return,
    };
    if (snapped.0 - grabbed.0).abs() < 1e-4 && (snapped.1 - grabbed.1).abs() < 1e-4 {
        return; // no movement this frame
    }
    if let Some(hs) = zone_body_mut(&mut state.gui_state.ship_structure, state.gui_state.construction_zone) {
        for wall in hs.walls.iter_mut() {
            if (wall.a.0 - grabbed.0).abs() < 0.05 && (wall.a.1 - grabbed.1).abs() < 0.05 {
                wall.a = snapped;
            }
            if (wall.b.0 - grabbed.0).abs() < 0.05 && (wall.b.1 - grabbed.1).abs() < 0.05 {
                wall.b = snapped;
            }
        }
    }
    state.construction_node_grab = Some(snapped);
    state.gui_state.construction_structure_dirty = true;
}

/// Per-frame while an OBJECT (light / machine / structure) is grabbed (v0.593): move it to the
/// cursor's floor hit, keeping its Y (height) -- the operator's "maintain their vertical height
/// while dragging." Tap-vs-drag like the wall corners, and honours the 0.25 m grid-snap toggle.
/// Collect the WORLD floor (x,z) of every placed object EXCEPT the one being dragged, for
/// alignment snapping (v0.613). Walls contribute both corners. World space (v0.754): the
/// active zone's local objects shift by its origin, so they align with the world-coordinate
/// machines/conduit nodes in one shared space.
pub(crate) fn gather_other_positions(state: &EngineState, grab: &ObjectGrab) -> Vec<(f32, f32)> {
    let mut out = Vec::new();
    let zo = active_zone_origin(state);
    if let Some(hs) = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
        for (i, l) in hs.lights.iter().enumerate() {
            // StripPoint 0 IS the light's pos (v0.790), so dragging it must not snap to itself.
            let dragging_this = matches!(grab, ObjectGrab::Light(g) if *g == i)
                || matches!(grab, ObjectGrab::StripPoint { light, point: 0 } if *light == i);
            if !dragging_this { out.push((l.pos.0 + zo.x, l.pos.2 + zo.z)); }
        }
        for (i, s) in hs.structures.iter().enumerate() {
            if !matches!(grab, ObjectGrab::Structure(g) if *g == i) { out.push((s.pos.0 + zo.x, s.pos.2 + zo.z)); }
        }
        for n in &hs.road_nodes {
            if !matches!(grab, ObjectGrab::RoadNode(g) if *g == n.id) { out.push((n.pos.0 + zo.x, n.pos.1 + zo.z)); }
        }
        for w in &hs.walls {
            out.push((w.a.0 + zo.x, w.a.1 + zo.z));
            out.push((w.b.0 + zo.x, w.b.1 + zo.z));
        }
    }
    if let Some(h) = state.gui_state.home_machines.as_ref() {
        for inst in h.all_instances() {
            if !matches!(grab, ObjectGrab::Machine(g) if *g == inst.id) { out.push((inst.offset.0, inst.offset.2)); }
        }
        for cn in &h.conduit_nodes {
            if !matches!(grab, ObjectGrab::ConduitNode(g) if *g == cn.id) { out.push((cn.pos.0, cn.pos.2)); }
        }
    }
    out
}

pub(crate) fn apply_object_drag(state: &mut EngineState) {
    let Some(grab) = state.construction_object_grab.clone() else {
        return;
    };
    if let Some(press) = state.construction_grab_press {
        let d = ((state.cursor_pos.0 - press.0).powi(2) + (state.cursor_pos.1 - press.1).powi(2)).sqrt();
        if d < DRAG_THRESHOLD_PX {
            return; // still a tap -- selection already happened on press
        }
        state.construction_grab_press = None; // armed: this is now a drag
    }
    let Some((bi, hx, hz)) = cursor_floor_hit(state) else {
        return;
    };
    // Handles with their OWN projection math run BEFORE the generic grid/alignment snap
    // below (v0.790): a corridor mouth slides on ONE world axis with its own snap + validity
    // clamp, and a rotation ring wants the RAW cursor angle -- grid-snapping the cursor first
    // would quantize the rotation into visible jumps. Both clear the alignment guides (they
    // never set them, and a stale guide from a previous drag would draw a lie).
    match grab {
        ObjectGrab::CorridorMouth(ci) => {
            state.construction_snap_x = None;
            state.construction_snap_z = None;
            apply_corridor_mouth_drag(state, ci, hx, hz);
            return;
        }
        ObjectGrab::LightAim { light, axis, prev_angle } => {
            state.construction_snap_x = None;
            state.construction_snap_z = None;
            // The floor hit as a 3D point: the room bound the ray landed in supplies the y.
            let hy = state.gui_state.room_bounds.get(bi).map(|rb| rb.min.y).unwrap_or(0.0);
            apply_light_aim_drag(state, light, axis, prev_angle, Vec3::new(hx, hy, hz));
            return;
        }
        _ => {}
    }
    let (mut nx, mut nz) = if state.gui_state.construction_grid_snap {
        ((hx * 4.0).round() / 4.0, (hz * 4.0).round() / 4.0)
    } else {
        (hx, hz)
    };
    // Alignment snap (v0.613): line up with an existing object's X and/or Z (within 0.3 m); stash the
    // snapped axis so the overlay can draw a guide line. Applied after grid-snap, so it wins when an
    // existing object is closer than the grid step.
    let others = gather_other_positions(state, &grab);
    let (ax, az, gx, gz) = snap_to_alignment(nx, nz, &others, 0.3);
    nx = ax;
    nz = az;
    state.construction_snap_x = gx;
    state.construction_snap_z = gz;
    // Snapping ran in WORLD space (machines + conduit nodes are world); zone-local objects
    // (lights, structures, road nodes, intra-zone volumes) subtract the active zone origin
    // before storing body coords. (v0.754)
    let zo = active_zone_origin(state);
    match grab {
        ObjectGrab::Light(i) => {
            if let Some(hs) = zone_body_mut(&mut state.gui_state.ship_structure, state.gui_state.construction_zone) {
                if let Some(l) = hs.lights.get_mut(i) {
                    l.pos.0 = nx - zo.x;
                    l.pos.2 = nz - zo.z; // l.pos.1 (height) preserved
                }
            }
            state.gui_state.construction_structure_dirty = true;
        }
        ObjectGrab::Structure(i) => {
            if let Some(hs) = zone_body_mut(&mut state.gui_state.ship_structure, state.gui_state.construction_zone) {
                if let Some(ps) = hs.structures.get_mut(i) {
                    ps.pos.0 = nx - zo.x;
                    ps.pos.2 = nz - zo.z; // ps.pos.1 (height) preserved
                }
            }
            state.gui_state.construction_structure_dirty = true;
        }
        ObjectGrab::Machine(id) => {
            if let Some(home) = state.gui_state.home_machines.as_mut() {
                // If this is an ARRAY cell (no direct instance), explode its array into instances so
                // it becomes individually movable -- the "I'm trying to move a grain tray but it
                // won't move" fix (array members were positioned procedurally, with no offset to
                // edit). Only here, past the tap-vs-drag threshold, so a mere SELECT never explodes
                // an array; only an actual drag does. Ids + positions are preserved. (v0.625)
                if !home.instances.iter().any(|m| m.id == id) {
                    home.detach_array_member(&id);
                }
                if let Some(inst) = home.instances.iter_mut().find(|m| m.id == id) {
                    inst.offset.0 = nx;
                    inst.offset.2 = nz; // offset.1 (height) preserved; box-mode offset is absolute
                }
            }
            state.gui_state.construction_machines_dirty = true;
        }
        ObjectGrab::RoadNode(id) => {
            // Road nodes live in the XZ plane (v0.599): drag moves both. Zone-local (v0.754).
            if let Some(hs) = zone_body_mut(&mut state.gui_state.ship_structure, state.gui_state.construction_zone) {
                if let Some(n) = hs.road_nodes.iter_mut().find(|n| n.id == id) {
                    n.pos = (nx - zo.x, nz - zo.z);
                }
            }
            state.gui_state.construction_structure_dirty = true;
        }
        ObjectGrab::ConduitNode(id) => {
            if let Some(home) = state.gui_state.home_machines.as_mut() {
                if let Some(cn) = home.conduit_nodes.iter_mut().find(|n| n.id == id) {
                    cn.pos.0 = nx;
                    cn.pos.2 = nz; // cn.pos.1 (height) preserved
                }
            }
            state.gui_state.construction_machines_dirty = true;
        }
        ObjectGrab::Zone(id) => {
            // Drag an intra-zone volume on the floor so its CENTRE follows the cursor (keeps
            // y + size); zone-local coords (v0.754). Volumes render live from the body's
            // zones list, so no rebuild flag is needed. (v0.634)
            if let Some(hs) = zone_body_mut(&mut state.gui_state.ship_structure, state.gui_state.construction_zone) {
                if let Some(z) = hs.zones.iter_mut().find(|z| z.id == id) {
                    z.origin.0 = (nx - zo.x) - z.size.0 * 0.5;
                    z.origin.2 = (nz - zo.z) - z.size.2 * 0.5;
                }
            }
        }
        ObjectGrab::StripPoint { light, point } => {
            // A strip CONTROL POINT follows the cursor across the floor (v0.790): point 0 is
            // the strip's start (`pos`), 1.. index `path` -- the exact fields the right
            // panel's DragValues edit, so viewport drags and panel edits never diverge.
            // Heights are preserved (drag steers the run; y stays a panel edit).
            if let Some(hs) = zone_body_mut(&mut state.gui_state.ship_structure, state.gui_state.construction_zone) {
                if let Some(l) = hs.lights.get_mut(light) {
                    if point == 0 {
                        l.pos.0 = nx - zo.x;
                        l.pos.2 = nz - zo.z; // pos.1 (height) preserved
                    } else if let Some(p) = l.path.get_mut(point - 1) {
                        p.0 = nx - zo.x;
                        p.2 = nz - zo.z; // p.1 (height) preserved
                    }
                }
            }
            state.gui_state.construction_structure_dirty = true;
        }
        // Handled by the early match above (own projection math); unreachable here, kept
        // only so this match stays exhaustive.
        ObjectGrab::CorridorMouth(_) | ObjectGrab::LightAim { .. } => {}
    }
}

/// Per-frame while a corridor DOOR-MOUTH handle is grabbed (v0.790): slide the corridor's
/// world `lat` to follow the cursor on the axis ACROSS the run (world z for an X-run tube,
/// world x for a Z-run). Snaps to the same 0.25 m step every other drag uses (when grid snap
/// is on) and CLAMPS to the validator's legal range so a drag can never strand the row
/// broken. `corridor_geometry` still re-resolves on every rebuild, so even a race (a zone
/// dragged away mid-frame) only means that corridor's mesh skips a rebuild -- the Corridors
/// panel shows the honest reason -- never a crash.
pub(crate) fn apply_corridor_mouth_drag(state: &mut EngineState, ci: usize, hx: f32, hz: f32) {
    use crate::ship::ship_structure::CorridorAxis;
    let grid = state.gui_state.construction_grid_snap;
    let Some(ship) = state.gui_state.ship_structure.as_mut() else {
        return;
    };
    let Some(c) = ship.corridors.get(ci).cloned() else {
        return; // the row was deleted mid-drag (panel X) -- just stop following
    };
    // Which world axis `lat` lives on comes from the RESOLVER (the run axis derives from the
    // zone boxes, never stored); a row that stopped resolving mid-drag stops following.
    let axis = match ship.corridor_geometry(&c) {
        Ok(g) => g.axis,
        Err(_) => return,
    };
    let mut lat = match axis {
        CorridorAxis::X => hz, // X run -> lat is world z
        CorridorAxis::Z => hx, // Z run -> lat is world x
    };
    if grid {
        lat = (lat * 4.0).round() / 4.0; // the editor-wide 0.25 m step
    }
    if let Some((lo, hi)) = ship.corridor_lat_limits(&c) {
        lat = lat.clamp(lo, hi);
    }
    if let Some(row) = ship.corridors.get_mut(ci) {
        row.lat = lat;
    }
    state.gui_state.construction_structure_dirty = true; // rebuild the tube + door cuts live
}

/// Per-frame while a light rotation ring is grabbed (v0.790; any light kind since v0.792):
/// the cursor's floor hit, seen from the light, sweeps an angle in the grabbed ring's
/// plane; the aim rotates by the
/// frame-to-frame DELTA of that angle about the ring's axis. The previous angle rides inside
/// the ObjectGrab variant itself (the way the opening grab carries its captured wall plane),
/// so there is no companion state field: the first drag frame just records the starting
/// angle and moves nothing.
pub(crate) fn apply_light_aim_drag(state: &mut EngineState, li: usize, axis_idx: u8, prev: Option<f32>, hit: Vec3) {
    let axis = match axis_idx {
        0 => Vec3::X,
        1 => Vec3::Y,
        _ => Vec3::Z,
    };
    let zo = active_zone_origin(state); // the light is zone-local; the hit is world (v0.754)
    let center = match zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone)
        .and_then(|h| h.lights.get(li))
    {
        Some(l) => Vec3::new(l.pos.0, l.pos.1, l.pos.2) + zo,
        None => return, // the light was removed mid-drag
    };
    // Cursor angle around the ring: light -> hit, flattened into the ring's plane, measured
    // against a FIXED in-plane basis (stable across frames, so deltas mean rotation).
    let v = hit - center;
    let v_in = v - axis * v.dot(axis);
    if v_in.length_squared() < 1e-6 {
        return; // the cursor sits on the ring's axis -- the angle is undefined this frame
    }
    let seed = if axis_idx == 0 { Vec3::Y } else { Vec3::X };
    let u = seed.cross(axis).normalize();
    let w = axis.cross(u);
    let angle = v_in.dot(w).atan2(v_in.dot(u));
    if let Some(prev) = prev {
        // Shortest signed step, wrapped to +-PI so crossing the atan2 seam never spins the aim.
        let delta = (angle - prev + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        if delta.abs() > 1e-5 {
            if let Some(l) = zone_body_mut(&mut state.gui_state.ship_structure, state.gui_state.construction_zone)
                .and_then(|h| h.lights.get_mut(li))
            {
                let d = Vec3::new(l.dir.0, l.dir.1, l.dir.2);
                let d = if d.length_squared() > 1e-6 { d.normalize() } else { Vec3::NEG_Y };
                // A zero result can only come from numeric collapse; keep aiming down rather
                // than storing a degenerate dir the resolver would have to guard against.
                let nd = (Quat::from_axis_angle(axis, delta) * d).normalize_or_zero();
                let nd = if nd == Vec3::ZERO { Vec3::NEG_Y } else { nd };
                l.dir = (nd.x, nd.y, nd.z);
                state.gui_state.construction_structure_dirty = true; // re-aim the live cone
            }
        }
    }
    // Carry this frame's angle into the grab so the next frame rotates by a fresh delta.
    state.construction_object_grab =
        Some(ObjectGrab::LightAim { light: li, axis: axis_idx, prev_angle: Some(angle) });
}

/// World positions of every door/window opening gizmo: ((wall index, opening index), centre).
/// `zo` is the owning zone's world origin (body coords are zone-local, v0.754). (v0.546)
pub(crate) fn opening_gizmos(hs: &crate::ship::home_structure::HomeStructure, zo: Vec3) -> Vec<((usize, usize), Vec3)> {
    let mut out = Vec::new();
    for (wi, wall) in hs.walls.iter().enumerate() {
        let (ax, az) = wall.a;
        let (dx, dz) = (wall.b.0 - ax, wall.b.1 - az);
        let len = (dx * dx + dz * dz).sqrt();
        if len < 1e-4 {
            continue;
        }
        let (ux, uz) = (dx / len, dz / len);
        for (oi, op) in wall.openings.iter().enumerate() {
            let s = (op.at + op.width * 0.5).clamp(0.0, len);
            let cy = op.sill + op.height * 0.5;
            out.push(((wi, oi), Vec3::new(ax + ux * s, cy, az + uz * s) + zo));
        }
    }
    out
}

/// Per-frame while an opening gizmo is grabbed: project the cursor onto that opening's wall and
/// slide the opening ALONG it (update `at`), grid-snapped + clamped within the wall. (v0.546)
pub(crate) fn apply_opening_drag(state: &mut EngineState) {
    let Some((wi, oi)) = state.construction_opening_grab else {
        return;
    };
    // Tap-vs-drag (v0.549): hold until the cursor leaves the press point; a tap selects.
    if let Some(press) = state.construction_grab_press {
        let d = ((state.cursor_pos.0 - press.0).powi(2) + (state.cursor_pos.1 - press.1).powi(2)).sqrt();
        if d < DRAG_THRESHOLD_PX {
            return;
        }
        state.construction_grab_press = None;
    }
    let Some((_, hx, hz)) = cursor_floor_hit(state) else {
        return;
    };
    // World floor hit -> the active zone's local coords (wall data is zone-local). (v0.754)
    let zo = active_zone_origin(state);
    let (hx, hz) = (hx - zo.x, hz - zo.z);
    let grid = state.gui_state.construction_grid_snap;
    if let Some(hs) = zone_body_mut(&mut state.gui_state.ship_structure, state.gui_state.construction_zone) {
        let Some(wall) = hs.walls.get_mut(wi) else {
            return;
        };
        let (ax, az) = wall.a;
        let (dx, dz) = (wall.b.0 - ax, wall.b.1 - az);
        let len = (dx * dx + dz * dz).sqrt();
        if len < 1e-4 {
            return;
        }
        let (ux, uz) = (dx / len, dz / len);
        let mut along = ((hx - ax) * ux + (hz - az) * uz).clamp(0.0, len);
        if grid {
            along = (along * 4.0).round() / 4.0;
        }
        if let Some(op) = wall.openings.get_mut(oi) {
            let half = op.width * 0.5;
            op.at = (along - half).clamp(0.0, (len - op.width).max(0.0));
        }
    }
    state.gui_state.construction_structure_dirty = true;
}

/// World positions of every opening RESIZE handle (v0.578): 4 per opening at the aperture edges --
/// left/right (mid-height) resize width, top/bottom (mid-width) resize height. Returns
/// ((wall, opening, edge), pos) with edge 0=left 1=right 2=top 3=bottom. `zo` is the owning
/// zone's world origin (v0.754).
pub(crate) fn opening_resize_handles(hs: &crate::ship::home_structure::HomeStructure, zo: Vec3) -> Vec<((usize, usize, u8), Vec3)> {
    let mut out = Vec::new();
    for (wi, wall) in hs.walls.iter().enumerate() {
        let (ax, az) = wall.a;
        let (dx, dz) = (wall.b.0 - ax, wall.b.1 - az);
        let len = (dx * dx + dz * dz).sqrt();
        if len < 1e-4 {
            continue;
        }
        let (ux, uz) = (dx / len, dz / len);
        for (oi, op) in wall.openings.iter().enumerate() {
            let s_l = op.at.clamp(0.0, len);
            let s_r = (op.at + op.width).clamp(0.0, len);
            let s_c = (op.at + op.width * 0.5).clamp(0.0, len);
            let cy_c = op.sill + op.height * 0.5;
            out.push(((wi, oi, 0), Vec3::new(ax + ux * s_l, cy_c, az + uz * s_l) + zo));
            out.push(((wi, oi, 1), Vec3::new(ax + ux * s_r, cy_c, az + uz * s_r) + zo));
            out.push(((wi, oi, 2), Vec3::new(ax + ux * s_c, op.sill + op.height, az + uz * s_c) + zo));
            out.push(((wi, oi, 3), Vec3::new(ax + ux * s_c, op.sill, az + uz * s_c) + zo));
        }
    }
    out
}

/// Per-frame: while a room is grabbed, intersect the pick ray with its floor plane, move
/// the room so it follows the cursor (minus the grab offset), snap to 0.25 m, and flag a
/// rebuild. Computed from the live cursor (not deltas) so it never drifts. (v0.466)
pub(crate) fn apply_room_drag(state: &mut EngineState) {
    let Some(grab) = state.construction_grab else { return; };
    let sz = state.window.inner_size();
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    if dir.y.abs() < 1e-6 {
        return;
    }
    let t = (grab.floor_y - origin.y) / dir.y;
    if t <= 0.0 {
        return;
    }
    let hit_x = origin.x + dir.x * t;
    let hit_z = origin.z + dir.z * t;
    let snap = |v: f32| (v / 0.25).round() * 0.25;
    let new_x = snap(hit_x - grab.offset_x);
    let new_z = snap(hit_z - grab.offset_z);
    if let Some(room) = state.gui_state.construction_rooms.get_mut(grab.room_index) {
        let mut p = room.position.unwrap_or([0.0, 0.0, 0.0]);
        if (p[0] - new_x).abs() > f32::EPSILON || (p[2] - new_z).abs() > f32::EPSILON {
            p[0] = new_x;
            p[2] = new_z;
            room.position = Some(p);
            state.gui_state.construction_dirty = true;
        }
    }
}

/// Per-frame: while an opening handle is grabbed, intersect the pick ray with the wall-FACE
/// plane, decompose the hit into (u along the wall, v up the wall), and apply the grab role.
/// A placed opening (Some) moves/resizes its `openings[i]`; a legacy face (None) slides its
/// `wall_offsets`. Everything clamps to the wall, so the panel value equals the real on-wall
/// placement (the 20m-vs-2m fix). Computed from the live cursor so it never drifts. (v0.469)
pub(crate) fn apply_gizmo_drag(state: &mut EngineState) {
    let Some(g) = state.construction_gizmo_grab else { return; };
    let sz = state.window.inner_size();
    let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
    // Ray vs the wall's vertical plane through wall_start with normal n.
    let denom = dir.dot(g.n);
    if denom.abs() < 1e-6 {
        return;
    }
    let t = (g.wall_start - origin).dot(g.n) / denom;
    if t <= 0.0 {
        return;
    }
    let hit = origin + dir * t;
    let rel = hit - g.wall_start;
    let u_raw = rel.dot(g.u_hat); // metres along the wall from the start corner
    let v_raw = rel.y; // metres up from the floor (wall_start is at floor y)
    let snap = |x: f32| (x / 0.1).round() * 0.1;
    let len = g.wall_len;
    let wh = g.wall_height;

    let Some(room) = state.gui_state.construction_rooms.get_mut(g.room_index) else { return; };

    // Legacy WallKind slide (no placed opening): write wall_offsets, build clamps the rest.
    let Some(oi) = g.opening_index else {
        let u_clamped = u_raw.clamp(g.grab_w * 0.5, (len - g.grab_w * 0.5).max(g.grab_w * 0.5));
        let new_off = snap(u_clamped - g.base_t);
        if (room.wall_offsets[g.wall_index] - new_off).abs() > f32::EPSILON {
            room.wall_offsets[g.wall_index] = new_off;
            state.gui_state.construction_dirty = true;
        }
        return;
    };
    let Some(op) = room.openings.get_mut(oi) else { return; };

    let before = *op;
    match g.role {
        GizmoRole::Move => {
            let hw = op.w * 0.5;
            op.u = snap(u_raw).clamp(hw, (len - hw).max(hw));
            if g.snap_floor {
                op.v = op.h * 0.5;
            } else {
                let hh = op.h * 0.5;
                op.v = snap(v_raw).clamp(hh, (wh - hh).max(hh));
            }
        }
        GizmoRole::ResizeRight => {
            let left = (g.grab_u - g.grab_w * 0.5).max(0.0);
            let right = snap(u_raw).clamp(left + 0.3, len);
            op.w = right - left;
            op.u = left + op.w * 0.5;
        }
        GizmoRole::ResizeLeft => {
            let right = (g.grab_u + g.grab_w * 0.5).min(len);
            let left = snap(u_raw).clamp(0.0, right - 0.3);
            op.w = right - left;
            op.u = left + op.w * 0.5;
        }
        GizmoRole::ResizeTop => {
            let bottom = (g.grab_v - g.grab_h * 0.5).max(0.0);
            let top = snap(v_raw).clamp(bottom + 0.3, wh);
            op.h = top - bottom;
            op.v = bottom + op.h * 0.5;
        }
        GizmoRole::ResizeBottom => {
            let top = (g.grab_v + g.grab_h * 0.5).min(wh);
            let bottom = snap(v_raw).clamp(0.0, top - 0.3);
            op.h = top - bottom;
            op.v = bottom + op.h * 0.5;
        }
    }
    if *op != before {
        state.gui_state.construction_dirty = true;
    }
}
