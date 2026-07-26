use glam::{Quat, Vec3};
use crate::engine::color::hsv_to_rgb;
use crate::engine::home_spawn::spawn_home_machine_entity;
use crate::engine::state::{EngineState, GrowSpot};
use crate::renderer::mesh::Mesh;
use crate::renderer::RenderObject;
use crate::ship::ship_structure::{zone_body, ShipStructure};

/// Upload a freshly generated set of homestead meshes into the renderer + state slots
/// (v0.455). Shared by the initial world load AND the construction editor's live rebuild.
/// v0.531: REUSES the prior mesh/material slots in place (replace_mesh / update_material) so a
/// per-frame rebuild during a room drag does not leak GPU buffers; only an added room/family
/// pushes a new slot, and a removed one orphans a single slot once (bounded).
pub(crate) fn apply_homestead_meshes(state: &mut EngineState, homestead: crate::ship::fibonacci::HomesteadMeshes) {
    // Reuse existing mesh/material SLOTS when present (v0.531), so a per-frame rebuild (a room
    // drag fires this every frame) never leaks GPU buffers -- the renderer was append-only, and
    // a multi-second drag was orphaning ~15-20 buffers/frame. Only an ADDED room/family pushes a
    // new slot; a REMOVED one leaves one orphaned slot (one-time, bounded).
    // Floors (one mesh + material per room): reuse the prior slot at index i when it exists.
    let prior_floors = std::mem::take(&mut state.homestead_floors);
    let mut floors = Vec::with_capacity(homestead.floors.len());
    for (i, (verts, indices, color, material_type)) in homestead.floors.into_iter().enumerate() {
        let mesh = Mesh::from_vertices(&state.renderer.device, &verts, &indices);
        if let Some(&(mi, ma)) = prior_floors.get(i) {
            state.renderer.replace_mesh(mi, mesh);
            state.renderer.update_material_typed(ma, color, 0.0, 0.8, material_type as f32);
            floors.push((mi, ma));
        } else {
            let mi = state.renderer.add_mesh(mesh);
            let ma = state.renderer.add_material_typed(color, 0.0, 0.8, material_type as f32);
            floors.push((mi, ma));
        }
    }
    state.homestead_floors = floors;
    // Combined-mesh families: reuse the prior slot if present, else add; None if empty (so a
    // removed window/mirror disappears -- its prior slot orphans once).
    let prior = state.homestead_walls;
    state.homestead_walls = if !homestead.walls.0.is_empty() {
        let mesh = Mesh::from_vertices(&state.renderer.device, &homestead.walls.0, &homestead.walls.1);
        if let Some((mi, ma)) = prior {
            state.renderer.replace_mesh(mi, mesh);
            state.renderer.update_material_typed(ma, [0.5, 0.5, 0.5, 1.0], 0.1, 0.6, 0.0);
            Some((mi, ma))
        } else {
            Some((state.renderer.add_mesh(mesh), state.renderer.add_material_typed([0.5, 0.5, 0.5, 1.0], 0.1, 0.6, 0.0)))
        }
    } else { None };
    // Per-material home walls (v0.552): one mesh+material per picked wall material so each wall
    // renders in its own color. Reuse prior slots (a per-frame rebuild fires on a drag); the
    // `is_transparent` flag routes glass (alpha < 1) to the transparent pass at render time.
    let prior_mw = std::mem::take(&mut state.homestead_material_walls);
    let mut material_walls = Vec::with_capacity(homestead.material_walls.len());
    for (i, (verts, indices, color)) in homestead.material_walls.into_iter().enumerate() {
        let mesh = Mesh::from_vertices(&state.renderer.device, &verts, &indices);
        let transparent = color[3] < 0.999;
        // Glass: low roughness through the transparent pass; opaque otherwise.
        // Emissive zeroed (v0.780): self-emission made glass GLOW at night.
        let (met, rough, mtype, emis) =
            if transparent { (0.0, 0.1, 1.0, 0.0) } else { (0.1, 0.7, 0.0, 0.0) };
        if let Some(&(mi, ma, _)) = prior_mw.get(i) {
            state.renderer.replace_mesh(mi, mesh);
            state.renderer.update_material_full(ma, color, met, rough, mtype, emis);
            material_walls.push((mi, ma, transparent));
        } else {
            let mi = state.renderer.add_mesh(mesh);
            let ma = state.renderer.add_material_full(color, met, rough, mtype, emis);
            material_walls.push((mi, ma, transparent));
        }
    }
    state.homestead_material_walls = material_walls;
    let prior = state.homestead_trim;
    state.homestead_trim = if !homestead.trim.0.is_empty() {
        let mesh = Mesh::from_vertices(&state.renderer.device, &homestead.trim.0, &homestead.trim.1);
        if let Some((mi, ma)) = prior {
            state.renderer.replace_mesh(mi, mesh);
            state.renderer.update_material_typed(ma, [0.42, 0.30, 0.18, 1.0], 0.0, 0.7, 3.0);
            Some((mi, ma))
        } else {
            Some((state.renderer.add_mesh(mesh), state.renderer.add_material_typed([0.42, 0.30, 0.18, 1.0], 0.0, 0.7, 3.0)))
        }
    } else { None };
    let prior = state.homestead_windows;
    state.homestead_windows = if !homestead.windows.0.is_empty() {
        let mesh = Mesh::from_vertices(&state.renderer.device, &homestead.windows.0, &homestead.windows.1);
        // Tinted glass (alpha 0.45), transparent pass. NO emissive (v0.780):
        // the old 0.12 self-emission bypassed lighting, so windows GLOWED in
        // the dark (operator field report: "the glass texture is emissive").
        // Glass now only shows light that actually falls on/through it.
        if let Some((mi, ma)) = prior {
            state.renderer.replace_mesh(mi, mesh);
            state.renderer.update_material_full(ma, [0.50, 0.74, 0.92, 0.45], 0.0, 0.08, 1.0, 0.0);
            Some((mi, ma))
        } else {
            Some((state.renderer.add_mesh(mesh), state.renderer.add_material_full([0.50, 0.74, 0.92, 0.45], 0.0, 0.08, 1.0, 0.0)))
        }
    } else { None };
    let prior = state.homestead_mirrors;
    state.homestead_mirrors = if !homestead.mirrors.0.is_empty() {
        let mesh = Mesh::from_vertices(&state.renderer.device, &homestead.mirrors.0, &homestead.mirrors.1);
        if let Some((mi, ma)) = prior {
            state.renderer.replace_mesh(mi, mesh);
            state.renderer.update_material_full(ma, [0.30, 0.55, 1.0, 1.0], 0.2, 0.15, 1.0, 1.6);
            Some((mi, ma))
        } else {
            Some((state.renderer.add_mesh(mesh), state.renderer.add_material_full([0.30, 0.55, 1.0, 1.0], 0.2, 0.15, 1.0, 1.6)))
        }
    } else { None };
    // v0.539: a glass roof renders the ceiling TRANSPARENT (you see the stars through the
    // sealed clear roof); otherwise it is the opaque grey ceiling. v0.754 (multi-zone ships):
    // roofs are PER ZONE -- `ShipStructure::generate_meshes` routes glass-roof zones' ceilings
    // into `ceilings` (this transparent slot) and opaque-roof zones' into `ceilings_opaque`
    // (its own slot below, show_roof-gated like the old single opaque roof). The fibonacci
    // fallback still fills this slot with `roof_glass = false` (opaque), unchanged.
    let roof_glass = state
        .gui_state
        .ship_structure
        .as_ref()
        .map_or(false, |s| s.any_glass_roof());
    state.homestead_ceiling_glass = roof_glass;
    let prior = state.homestead_ceiling;
    state.homestead_ceiling = if !homestead.ceilings.0.is_empty() {
        let mesh = Mesh::from_vertices(&state.renderer.device, &homestead.ceilings.0, &homestead.ceilings.1);
        // (color, metallic, roughness, material_type, emissive) for glass; (color, m, r, type) opaque.
        // Glass roof emissive zeroed (v0.780) -- same night-glow fix as the windows.
        let (gcol, gmet, grough, gtype, gemis) = ([0.55, 0.78, 0.92, 0.22], 0.0, 0.05, 1.0, 0.0);
        let (ocol, omet, orough, otype) = ([0.60, 0.62, 0.68, 1.0], 0.0, 0.8, 2.0);
        if let Some((mi, ma)) = prior {
            state.renderer.replace_mesh(mi, mesh);
            if roof_glass {
                state.renderer.update_material_full(ma, gcol, gmet, grough, gtype, gemis);
            } else {
                state.renderer.update_material_typed(ma, ocol, omet, orough, otype);
            }
            Some((mi, ma))
        } else if roof_glass {
            Some((state.renderer.add_mesh(mesh), state.renderer.add_material_full(gcol, gmet, grough, gtype, gemis)))
        } else {
            Some((state.renderer.add_mesh(mesh), state.renderer.add_material_typed(ocol, omet, orough, otype)))
        }
    } else { None };
    // OPAQUE-roof zones' ceilings (v0.754): their own slot with the same opaque material the
    // single opaque roof used ([0.60,0.62,0.68], rough 0.8, concrete type 2), rendered only
    // when show_roof is on (see the render loop) -- per-zone glass/steel roofs both behave.
    let prior = state.homestead_ceiling_opaque;
    state.homestead_ceiling_opaque = if !homestead.ceilings_opaque.0.is_empty() {
        let mesh = Mesh::from_vertices(
            &state.renderer.device,
            &homestead.ceilings_opaque.0,
            &homestead.ceilings_opaque.1,
        );
        let (ocol, omet, orough, otype) = ([0.60, 0.62, 0.68, 1.0], 0.0, 0.8, 2.0);
        if let Some((mi, ma)) = prior {
            state.renderer.replace_mesh(mi, mesh);
            state.renderer.update_material_typed(ma, ocol, omet, orough, otype);
            Some((mi, ma))
        } else {
            Some((state.renderer.add_mesh(mesh), state.renderer.add_material_typed(ocol, omet, orough, otype)))
        }
    } else { None };
}

/// Regenerate + upload the HULL WRAP (ship-superstructure increment D): the generated
/// exterior shell around the zone cluster, lofted from data/blueprints/hull_profile.ron
/// (embedded fallback) with cutouts over glass roofs + glass corridor lids. Called wherever
/// the ship structure rebuilds (load_world + rebuild_homestead), so adding/moving a zone
/// grows the hull next frame. Reuses the prior mesh/material slot in place (the v0.531
/// no-leak discipline: a per-frame editor drag must not orphan GPU buffers). Purely visual:
/// no collision is registered for the hull.
pub(crate) fn rebuild_hull(state: &mut EngineState) {
    // Lazy profile load (disk first, embedded fallback). Cached until the
    // hot-reload poll sees hull_profile.ron change, which clears the cache
    // and calls back in here (v0.770) - silhouette tuning without relaunch.
    if state.hull_profile.is_none() {
        state.hull_profile = crate::ship::hull::HullProfile::load(&state.data_dir);
    }
    let meshes = match (state.gui_state.ship_structure.as_ref(), state.hull_profile.as_ref()) {
        (Some(ship), Some(profile)) => crate::ship::hull::generate_hull(ship, profile),
        // No ship (legacy fibonacci layout) or no parseable profile: no hull.
        _ => crate::ship::hull::HullMeshes::default(),
    };
    let prior = state.homestead_hull;
    state.homestead_hull = if !meshes.plating.0.is_empty() {
        let mesh = Mesh::from_vertices(&state.renderer.device, &meshes.plating.0, &meshes.plating.1);
        // Hull plating colors from the same wall-material palette as zone shells, so the
        // profile's `material` id means the same thing everywhere.
        let mat_id = state.hull_profile.as_ref().map_or(1, |p| p.material);
        let color = crate::ship::home_structure::HomeStructure::material_color(mat_id);
        if let Some((mi, ma)) = prior {
            state.renderer.replace_mesh(mi, mesh);
            state.renderer.update_material_typed(ma, color, 0.35, 0.55, mat_id as f32);
            Some((mi, ma))
        } else {
            Some((
                state.renderer.add_mesh(mesh),
                state.renderer.add_material_typed(color, 0.35, 0.55, mat_id as f32),
            ))
        }
    } else {
        None // an emptied hull orphans its slot once (bounded, same as the other families)
    };
}

/// Regenerate the homestead meshes from the live layout (the construction editor's apply).
/// Also refreshes room lights + the sealed-volume bounds, since a height/wall edit changes
/// them. (v0.455)
pub(crate) fn rebuild_homestead(state: &mut EngineState) {
    // Normalize every corner onto the corner grid (v0.574) so co-located corners are byte-identical
    // -- this self-heals any older home whose snapped corners had sub-tolerance residue (which read
    // as two overlapping orbs that dragged apart). Idempotent: an on-grid corner is unchanged.
    // Runs across EVERY zone (v0.754): all zones render + collide, not just the edited one.
    if let Some(ship) = state.gui_state.ship_structure.as_mut() {
        for zone in ship.zones.iter_mut() {
            for wall in zone.body.walls.iter_mut() {
                wall.a = crate::ship::home_structure::quantize_corner(wall.a);
                wall.b = crate::ship::home_structure::quantize_corner(wall.b);
            }
        }
    }
    // Dev tool (v0.576): write a machine-readable snapshot of the live home so an AI can READ what
    // the operator is building (the act surface -- a text-command console -- is the next stage).
    // Still the ACTIVE ZONE's body (the zone being edited -- what the operator is building right
    // now); a ship-level introspection surface is a follow-up.
    if let Some(hs) = zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
        let json = hs.to_introspection_json();
        let _ = std::fs::create_dir_all("debug");
        let _ = std::fs::write("debug/home_snapshot.json", json);
    }
    // v0.534/v0.754: regenerate the WHOLE SHIP (every zone, offset by its origin) when present,
    // else the legacy AABB-room layout.
    let homestead = if let Some(ship) = state.gui_state.ship_structure.as_ref() {
        ship.generate_meshes()
    } else if let Some(layout) = state.homestead_layout.clone() {
        crate::ship::fibonacci::generate_from_layout(&layout)
    } else {
        return;
    };
    let room_info = homestead.room_info.clone();
    // Rebuild the wall collision segments from the live ship (v0.556; per-zone offsets v0.754)
    // so editing a wall updates what the player walks into. Empty for the legacy AABB layout.
    state.wall_colliders = match &state.gui_state.ship_structure {
        Some(ship) => crate::ship::wall_collision::ship_wall_segments(ship),
        None => Vec::new(),
    };
    state.sight_colliders = match &state.gui_state.ship_structure {
        Some(ship) => crate::ship::wall_collision::ship_sight_segments(ship),
        None => Vec::new(),
    };
    apply_homestead_meshes(state, homestead);
    // The hull wrap follows the structure (increment D): a zone add/move/resize or a roof
    // material change regrows the exterior shell in the same rebuild.
    rebuild_hull(state);
    // Refresh lights + sealed bounds from the new room_info (height edits move them).
    let auto_lights = room_info.iter().map(|r| {
        let light_pos = Vec3::new(r.center.x, r.center.y + r.dimensions.y * 0.5 - 0.1, r.center.z);
        let room_size = r.dimensions.x.max(r.dimensions.z);
        let intensity = (room_size * 0.5).clamp(2.0, 15.0);
        crate::renderer::light::RoomLight::point(light_pos, [1.0, 0.95, 0.85], intensity, room_size * 1.5)
    }).collect();
    // v0.571: placed lights (across ALL zones, v0.754) override the auto synthesis (empty -> auto).
    state.room_lights = home_lights(state.gui_state.ship_structure.as_ref(), auto_lights, state.gui_state.gi_enabled);
    state.homestead_bounds = room_info.iter().fold(None, |acc, r| {
        let rmin = r.center - r.dimensions * 0.5;
        let rmax = r.center + r.dimensions * 0.5;
        Some(match acc { None => (rmin, rmax), Some((mn, mx)) => (mn.min(rmin), mx.max(rmax)) })
    });
    // Refresh the HUD room volumes (the "you are in <room>" detection + occlusion) so a
    // moved/resized/added/removed room is tracked live, not just on restart. (v0.459)
    // (Machine placement + pipes + hologram/spawn still resolve at load_world; they refresh
    // on the next relaunch -- a follow-up will make them live too.)
    let room_types = crate::ship::room_types::RoomTypeRegistry::load(&state.data_dir);
    state.gui_state.room_bounds = room_info
        .iter()
        .map(|r| crate::gui::RoomBounds {
            id: r.id.clone(),
            min: r.center - r.dimensions * 0.5,
            max: r.center + r.dimensions * 0.5,
            display_name: room_types.name(&r.id),
            purpose: room_types.purpose(&r.id),
            actions: room_types.action_labels(&r.id),
            access: room_types.access(&r.id),
        })
        .collect();
    // Room geometry changed, so the machines in those rooms must follow (a moved/resized room
    // carries its machines). Refresh the machine meshes from the new room bounds. (v0.525)
    rebuild_machine_objects(state);
    // Door/window panels follow the structure too (a wall edit can add/move/remove openings).
    rebuild_door_panels(state);
    log::info!("Homestead rebuilt: {} rooms", room_info.len());
}

/// Build a machine's primitive mesh from its shape + size. Shared by load_world (initial spawn)
/// and rebuild_machine_objects (the editor's live refresh) so both draw a machine identically.
pub(crate) fn machine_mesh(device: &wgpu::Device, shape: &str, size: (f32, f32, f32)) -> Mesh {
    let (sx, sy, sz) = size;
    match shape {
        // Capped so tanks/cisterns have a visible top + bottom (v0.623: "the tops are missing").
        "cylinder" => Mesh::cylinder_capped(device, sx.max(0.02), sy.max(0.05), 16),
        "sphere" => Mesh::sphere(device, sx.max(0.02), 10, 12),
        "pyramid" => Mesh::pyramid(device, sx.max(0.05), sy.max(0.05)),
        _ => Mesh::box_xyz(device, sx.max(0.02), sy.max(0.02), sz.max(0.02)),
    }
}

/// Resolve the home's drone hangar (the "drone_hangar" catalog machine, e.g. `drone_hangar_1`
/// in data/machines/home.ron) to its world DECK position + yaw, so the mining-drone visual
/// (v0.639) can sit exactly where the hangar pad is drawn. Reuses the same tested
/// `MachineHome::placements` the machine meshes themselves are built from, so the drone never
/// drifts from the pad under it even as the home layout is edited live.
///
/// Design choice (v1, documented per the operator's ask): today's home.ron places exactly ONE
/// "drone_hangar" instance, and `DroneSystem` allows only ONE drone in flight at a time (see
/// `systems::mining::DroneSystem::tick`'s "one drone per player" gate -- a second commission
/// while one is flying is refused). So "the hangar" and "the drone" are both singular right
/// now -- this picks the FIRST drone_hangar instance found, and the caller treats hangar
/// occupancy as a simple binary (any drone in flight => pad empty; no drone => docked). That is
/// not a simplifying guess, it is EXACT given today's one-drone rule. If a future change adds
/// multiple hangars or multiple simultaneous drones, this needs real per-drone-per-hangar
/// assignment (e.g. tagging `Drone` with the hangar id it launched from).
pub(crate) fn hangar_placement(state: &EngineState) -> Option<(Vec3, f32)> {
    use std::collections::HashMap;
    let home = state.gui_state.home_machines.as_ref()?;
    let rooms: HashMap<String, crate::machines::RoomGeom> = state
        .gui_state
        .room_bounds
        .iter()
        .map(|rb| {
            (
                rb.id.clone(),
                crate::machines::RoomGeom {
                    center_x: (rb.min.x + rb.max.x) * 0.5,
                    center_z: (rb.min.z + rb.max.z) * 0.5,
                    floor_y: rb.min.y,
                    ceiling_y: rb.max.y,
                },
            )
        })
        .collect();
    if rooms.is_empty() {
        return None;
    }
    let zone_rects = state.gui_state.ship_structure.as_ref().map(|s| s.zone_rects());
    // Match by CATALOG TYPE ("drone_hangar"), not by id or label text, so a renamed label or a
    // re-numbered instance id still resolves correctly.
    let hangar_id = home.all_instances().into_iter().find(|i| i.machine == "drone_hangar")?.id;
    home.placements(&rooms, zone_rects.as_deref())
        .into_iter()
        .find(|p| p.id == hangar_id)
        .map(|p| (Vec3::new(p.pos.0, p.pos.1, p.pos.2), p.rotation))
}

/// World position of a machine's port gizmo (v0.625, the viewport drag-to-connect handles). Ports
/// carry no authored anchor (they default to 0,0,0), so spread a machine's N ports in a ring just
/// ABOVE the body, so each reads as its own grab-able handle that never overlaps the machine mesh.
pub(crate) fn port_gizmo_pos(p: &crate::machines::PlacedMachine, i: usize, n: usize) -> Vec3 {
    let top = p.top_y + 0.3;
    if n <= 1 {
        return Vec3::new(p.pos.0, top, p.pos.2);
    }
    let r = (p.size.0.max(p.size.2)) * 0.5 + 0.3;
    let a = (i as f32 / n as f32) * std::f32::consts::TAU;
    Vec3::new(p.pos.0 + r * a.cos(), top, p.pos.2 + r * a.sin())
}

/// Keep the live machine ECS in sync with the editor placements (v0.730).
/// Spawns entities for newly-placed machines (with their power/water/
/// AutoRefine/Container roles), despawns removed ones, and updates the
/// Transform of survivors so behaviors anchored to the machine's pose
/// (the assembler's factory pad) follow a move. Idempotent + cheap.
pub(crate) fn sync_machine_entities(state: &mut EngineState, placements: &[crate::machines::PlacedMachine]) {
    use crate::ecs::components::{MachineInstanceId, Transform};
    let Some(home) = state.gui_state.home_machines.as_ref() else { return };
    let all = home.all_instances();
    let power_islands = home.electrical_islands(&all);
    let water_islands = home.water_islands(&all);
    let inst_by_id: std::collections::HashMap<&str, &crate::machines::MachineInstance> =
        all.iter().map(|i| (i.id.as_str(), i)).collect();
    let placed_ids: std::collections::HashSet<&str> =
        placements.iter().map(|p| p.id.as_str()).collect();
    // Snapshot the existing machine entities (id -> entity) in one scope so
    // the query borrow ends before we mutate the world.
    let existing: std::collections::HashMap<String, hecs::Entity> = state
        .game_world
        .world
        .query::<&MachineInstanceId>()
        .iter()
        .map(|(e, mid)| (mid.0.clone(), e))
        .collect();
    // Despawn entities whose machine no longer exists in the editor state.
    for (id, e) in &existing {
        if !placed_ids.contains(id.as_str()) {
            let _ = state.game_world.world.despawn(*e);
            log::info!("[Machines] despawned removed machine entity {id}");
        }
    }
    let containers = state
        .data_store
        .get::<crate::systems::inventory::containers::ContainerRegistry>("container_registry");
    for p in placements {
        let pos = Vec3::new(p.pos.0, p.pos.1, p.pos.2);
        if let Some(&e) = existing.get(&p.id) {
            // Keep the pose current (moves are count-unchanged edits).
            if let Ok(mut t) = state.game_world.world.get::<&mut Transform>(e) {
                t.position = pos;
                t.rotation = Quat::from_rotation_y(p.rotation.to_radians());
            }
        } else if let Some(inst) = inst_by_id.get(p.id.as_str()) {
            if let Some(def) = home.catalog.get(&inst.machine) {
                spawn_home_machine_entity(
                    &mut state.game_world.world,
                    inst,
                    def,
                    &power_islands,
                    &water_islands,
                    Some(pos),
                    containers,
                );
                log::info!("[Machines] spawned live entity for placed machine {}", p.id);
            }
        }
    }
}

pub(crate) fn rebuild_machine_objects(state: &mut EngineState) {
    use std::collections::HashMap;
    let rooms: HashMap<String, crate::machines::RoomGeom> = state
        .gui_state
        .room_bounds
        .iter()
        .map(|rb| {
            (
                rb.id.clone(),
                crate::machines::RoomGeom {
                    center_x: (rb.min.x + rb.max.x) * 0.5,
                    center_z: (rb.min.z + rb.max.z) * 0.5,
                    floor_y: rb.min.y,
                    ceiling_y: rb.max.y,
                },
            )
        })
        .collect();
    // Guard: if there is no room geometry yet (room_bounds not populated), do NOT wipe the
    // machines load_world already placed -- otherwise an edit before bounds are ready blanks
    // the whole home. (v0.528)
    if rooms.is_empty() {
        return;
    }
    // v0.538: a box home positions machines by ABSOLUTE world coords (not room-center-relative)
    // so they survive flood-fill room-id churn. v0.754: clamped per machine into ITS ship
    // zone's footprint at that zone's origin.
    let zone_rects = state.gui_state.ship_structure.as_ref().map(|s| s.zone_rects());
    let placements = match &state.gui_state.home_machines {
        Some(h) => h.placements(&rooms, zone_rects.as_deref()),
        None => return,
    };
    // Port pick volumes (v0.625): every machine's derived ports -> a grab-able world gizmo, so the
    // viewport can DRAG a port onto another machine to wire them. Keyed by id (placements may skip a
    // machine whose catalog/room is missing), so zip-by-index is unsafe -- look the type up by id.
    {
        let pp = {
            let home = state.gui_state.home_machines.as_ref().unwrap();
            let type_by_id: HashMap<String, String> =
                home.all_instances().into_iter().map(|i| (i.id, i.machine)).collect();
            let mut pp: Vec<(String, usize, crate::utilities::Port, Vec3)> = Vec::new();
            for p in &placements {
                let Some(ty) = type_by_id.get(&p.id) else { continue };
                let Some(def) = home.catalog.get(ty) else { continue };
                let ports = def.derive_ports();
                let n = ports.len();
                for (i, port) in ports.into_iter().enumerate() {
                    pp.push((p.id.clone(), i, port, port_gizmo_pos(p, i, n)));
                }
            }
            pp
        };
        state.port_pick = pp;
    }
    // ── Live ECS sync (v0.730, operator field report): the editor previously
    // rebuilt VISUALS only, so a machine placed in the construction editor had
    // NO entity until the next Enter World — no AutoRefine (a placed vehicle
    // assembler never assembled and showed no recipe selector), no Container
    // (a placed silo's card stayed on the static RON stats), no power/water
    // roles. Diff the placements against the live entities by machine id:
    // spawn missing, despawn removed, keep Transforms current so a moved
    // assembler's factory pad follows. Runs on every editor commit (and per
    // frame during drags) — one small map + query, cheap at homestead scale.
    // NOTE: electrical/plumbing ISLANDS for pre-existing entities are not
    // recomputed here (a new connection re-islands on the next world entry,
    // same as before).
    sync_machine_entities(state, &placements);

    // Fast path: the machine COUNT is unchanged (an offset drag / room move, not add/remove).
    // Reuse the existing meshes + materials and only update positions, so a per-frame drag does
    // NOT leak a fresh mesh per machine every frame (the v0.527 regression). placements() is
    // deterministically ordered (instances then array cells), so index i is the same machine.
    if placements.len() == state.machine_objects.len()
        && placements.len() == state.gui_state.machine_labels.len()
        && placements.len() == state.machine_pick.len()
    {
        for (i, p) in placements.iter().enumerate() {
            state.machine_objects[i].2 = Vec3::new(p.pos.0, p.pos.1, p.pos.2);
            state.machine_objects[i].3 = p.rotation; // keep the yaw in sync on a position-only update
            state.gui_state.machine_labels[i].pos = Vec3::new(p.pos.0, p.top_y + 0.4, p.pos.2);
            // Keep the pick volume in sync (v0.553) -- else a move WITHOUT a count change (a room
            // drag, a clamp-on-resize) leaves the click ray-test + the highlight ring at the OLD
            // position. Same math as the slow-path build below.
            let half_h = ((p.top_y - p.pos.1) * 0.5).max(0.2);
            let half_w = p.size.0.max(p.size.1).max(p.size.2) * 0.5;
            state.machine_pick[i] = (
                p.id.clone(),
                Vec3::new(p.pos.0, (p.pos.1 + p.top_y) * 0.5, p.pos.2),
                half_h.max(half_w) + 0.35,
            );
        }
        rebuild_connection_objects(state);
        return;
    }
    // Count changed (add / remove) or first build. Reuse prior mesh/material SLOTS where they
    // exist (replace in place) instead of clear()+re-add, so a single add/remove doesn't orphan
    // the whole ~100-mesh home; only the growth pushes new slots, and a shrink orphans the tail
    // once (bounded). (v0.531 -- the renderer free path.)
    let prior = std::mem::take(&mut state.machine_objects);
    state.gui_state.machine_labels.clear();
    state.machine_pick.clear();
    let mut objs = Vec::with_capacity(placements.len());
    for (i, p) in placements.iter().enumerate() {
        // GLB model when the def declares one (v0.734): parsed fresh PER
        // INSTANCE (no shared cache), so the replace/reuse slot logic
        // below stays safe — each machine owns its mesh slot. Primitive
        // fallback on any load error, so a bad file never blanks it.
        let mesh = p
            .model
            .as_deref()
            .and_then(|m| {
                state
                    .asset_manager
                    .parse_gltf_mesh(&state.renderer.device, m)
                    .map_err(|e| {
                        log::warn!("machine {} model '{m}' failed: {e}; primitive fallback", p.id)
                    })
                    .ok()
            })
            .unwrap_or_else(|| machine_mesh(&state.renderer.device, &p.shape, p.size));
        let color = [p.color.0, p.color.1, p.color.2, 1.0];
        let pos = Vec3::new(p.pos.0, p.pos.1, p.pos.2);
        if let Some(&(mi, ma, _, _)) = prior.get(i) {
            state.renderer.replace_mesh(mi, mesh);
            state.renderer.update_material_typed(ma, color, 0.1, 0.7, 0.0);
            objs.push((mi, ma, pos, p.rotation));
        } else {
            let mi = state.renderer.add_mesh(mesh);
            let ma = state.renderer.add_material_typed(color, 0.1, 0.7, 0.0);
            objs.push((mi, ma, pos, p.rotation));
        }
        state.gui_state.machine_labels.push(crate::gui::MachineLabel {
            pos: Vec3::new(p.pos.0, p.top_y + 0.4, p.pos.2),
            name: p.label.clone(),
            stats: p.stats.clone(),
            room: p.room.clone(),
            machine_id: p.id.clone(),
        });
        // Pick volume for viewport selection: a sphere covering the machine body. Center at its
        // mid-height; radius the larger of half-height / half-width plus a click margin.
        let half_h = ((p.top_y - p.pos.1) * 0.5).max(0.2);
        let half_w = p.size.0.max(p.size.1).max(p.size.2) * 0.5;
        state.machine_pick.push((
            p.id.clone(),
            Vec3::new(p.pos.0, (p.pos.1 + p.top_y) * 0.5, p.pos.2),
            half_h.max(half_w) + 0.35,
        ));
    }
    state.machine_objects = objs;
    // Record grow anchors for the procedural plant pass (v0.862/0.863):
    // EVERY placed machine, with its catalog type, footprint and top
    // height, so the plant pass can dress towers (helix) and beds/fields
    // (grid across the footprint at top_y). Type looked up by instance id
    // (placements carry no catalog key).
    state.grow_positions.clear();
    if let Some(home) = state.gui_state.home_machines.as_ref() {
        let type_by_id: HashMap<String, String> =
            home.all_instances().into_iter().map(|i| (i.id, i.machine)).collect();
        for p in &placements {
            if let Some(ty) = type_by_id.get(&p.id) {
                state.grow_positions.push(GrowSpot {
                    ty: ty.clone(),
                    id: p.id.clone(),
                    pos: Vec3::new(p.pos.0, p.pos.1, p.pos.2),
                    yaw: p.rotation,
                    top_y: p.top_y,
                    size: p.size,
                });
            }
        }
    }
    state.plant_mesh_sig = 0; // machine layout may have moved: replant visuals
    rebuild_connection_objects(state);
}

/// Procedural plants (v0.862): build ONE merged world-space mesh per planted
/// tower config from the live CropInstances, colored via the type-12 packed-UV
/// trick, and draw it as a single RenderObject. Crops bind to a tower CONFIG
/// id ("nutrition"), not a physical column, so each config's 50 slots render
/// on the FIRST placed tower of that type until per-instance planting exists.
/// Cheap change-signature gate: rebuilds only when growth actually moves.
pub(crate) fn rebuild_plant_meshes(state: &mut EngineState) {
    use std::hash::{Hash, Hasher};
    // Signature over everything that changes a plant's look.
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (_e, c) in state
        .game_world
        .world
        .query::<&crate::ecs::components::CropInstance>()
        .iter()
    {
        c.crop_def_id.hash(&mut h);
        c.growth_stage.hash(&mut h);
        c.tower_id.hash(&mut h);
        c.tower_slot.hash(&mut h);
        ((c.health / 10.0) as u32).hash(&mut h);
    }
    state.grow_positions.len().hash(&mut h);
    let sig = h.finish().max(1);
    if sig == state.plant_mesh_sig {
        return;
    }
    state.plant_mesh_sig = sig;

    let plant_reg = state
        .data_store
        .get::<crate::systems::farming::PlantRegistry>("plant_registry");
    // Visual defs: read fresh from disk each rebuild (rebuilds are rare and
    // this is what makes live-editing plants_visual.ron work), with the
    // committed file as the only source (no embedded fallback yet).
    let vis_text = std::fs::read_to_string(crate::data_dir().join("plants_visual.ron"))
        .unwrap_or_default();
    let visuals = crate::renderer::plant_mesh::PlantVisualRegistry::from_ron(&vis_text)
        .unwrap_or_default();
    let tower_cfgs = crate::gui::load_tower_configs(&crate::data_dir());

    // Group crops by tower config id.
    let mut by_tower: std::collections::HashMap<String, Vec<(String, String, u32, f32)>> =
        std::collections::HashMap::new();
    for (_e, c) in state
        .game_world
        .world
        .query::<&crate::ecs::components::CropInstance>()
        .iter()
    {
        let (Some(tid), Some(slot)) = (c.tower_id.clone(), c.tower_slot) else { continue };
        by_tower.entry(tid).or_default().push((
            c.crop_def_id.clone(),
            c.growth_stage.clone(),
            slot,
            c.health,
        ));
    }

    let mut builders: Vec<crate::renderer::plant_mesh::PlantMeshBuilder> = Vec::new();
    for (cfg_id, crops) in &by_tower {
        // Resolve the crop group's world layout. Three key shapes:
        // - legacy tower CONFIG id ("nutrition", from the GUI Plant button):
        //   dress the first placed column of that type;
        // - tower INSTANCE id ("ntower_5", from the showcase auto-seed):
        //   dress that exact column;
        // - bed/field/rack INSTANCE id: grid across the machine footprint.
        let (helix_cfg, base, grid_spot) = if let Some(cfg) =
            tower_cfgs.iter().find(|t| t.id == *cfg_id)
        {
            let cat_key = format!("aeroponic_tower_{cfg_id}");
            match state.grow_positions.iter().find(|g| g.ty == cat_key) {
                Some(g) => (Some(cfg), g.pos, None),
                None => continue,
            }
        } else if let Some(g) = state.grow_positions.iter().find(|g| g.id == *cfg_id) {
            if let Some(cfg_key) = g.ty.strip_prefix("aeroponic_tower_") {
                match tower_cfgs.iter().find(|t| t.id == cfg_key) {
                    Some(cfg) => (Some(cfg), g.pos, None),
                    None => continue,
                }
            } else {
                (None, g.pos, Some(g))
            }
        } else {
            continue;
        };
        let slots = helix_cfg.map(|c| c.slots.max(1)).unwrap_or(1);
        let radius = helix_cfg.map(|c| c.diameter_m * 0.5).unwrap_or(0.0);
        let mut b = crate::renderer::plant_mesh::PlantMeshBuilder::new();
        for (def_id, stage, slot, health) in crops {
            // v0.903 (operator: "the potato garden is just a plain slab
            // of brown"): only 10 of ~134 crops had visual recipes, and
            // every crop WITHOUT one silently skipped mesh generation -
            // bare beds, empty tower net cups. Unrecipe'd crops now get
            // a generic leafy plant (deterministically varied per
            // species) so every garden visibly GROWS; hand-authored
            // recipes in data/plants_visual.ron still win when present.
            let generic;
            let vis = match visuals.get(def_id) {
                Some(v) => v,
                None => {
                    generic = crate::renderer::plant_mesh::generic_visual(def_id);
                    &generic
                }
            };
            // Stage index -> growth t (same bucketing the GUI shows).
            let stages: Vec<&str> = plant_reg
                .and_then(|r| r.get(def_id))
                .map(|d| d.stages())
                .unwrap_or_else(|| {
                    crate::ecs::components::DEFAULT_GROWTH_STAGES.iter().copied().collect()
                });
            let dead = stage.as_str() == crate::ecs::components::STAGE_DEAD;
            let t = if dead {
                0.6
            } else {
                stages
                    .iter()
                    .position(|s| *s == stage.as_str())
                    .map(|i| (i as f32 + 1.0) / stages.len().max(1) as f32)
                    .unwrap_or(0.1)
            };
            let wilt = if dead { 1.0 } else { (1.0 - health / 100.0).clamp(0.0, 1.0) };
            let (pos, out) = if let Some(cfg) = helix_cfg {
                // Helix slot position up the column, plant facing outward.
                let frac = *slot as f32 / slots as f32;
                let ang = frac * cfg.helix_turns * std::f32::consts::TAU;
                let y = 0.18 + frac * (cfg.height_m - 0.45);
                let out = [ang.cos(), 0.0, ang.sin()];
                (
                    [base.x + out[0] * radius, base.y + y, base.z + out[2] * radius],
                    out,
                )
            } else if let Some(g) = grid_spot {
                // Bed/field/rack: grid across the footprint at the machine top.
                let n = crops.len().max(1) as u32;
                let cols = (n as f32).sqrt().ceil().max(1.0) as u32;
                let rows = n.div_ceil(cols);
                let (cx, rz) = (*slot % cols, (*slot / cols).min(rows - 1));
                let fx = (cx as f32 + 0.5) / cols as f32 - 0.5;
                let fz = (rz as f32 + 0.5) / rows as f32 - 0.5;
                (
                    [
                        g.pos.x + fx * g.size.0 * 0.85,
                        g.top_y,
                        g.pos.z + fz * g.size.2 * 0.85,
                    ],
                    [0.7, 0.0, 0.7],
                )
            } else {
                continue;
            };
            // Tower plants render at reduced scale so a tree in a net cup
            // reads as a dwarf/espalier rather than a full orchard tree.
            // Bed/field plants grow full size.
            let mut vis_scaled = vis.clone();
            if helix_cfg.is_some() && vis_scaled.height_m > 0.6 {
                let k = 0.6 / vis_scaled.height_m;
                vis_scaled.height_m *= k;
                vis_scaled.spread_m *= k;
                vis_scaled.stem_radius *= k;
            }
            let seed = {
                let mut sh = std::collections::hash_map::DefaultHasher::new();
                cfg_id.hash(&mut sh);
                slot.hash(&mut sh);
                sh.finish()
            };
            crate::renderer::plant_mesh::build_plant(&mut b, &vis_scaled, pos, out, t, wilt, seed);
        }
        if !b.vertices.is_empty() {
            builders.push(b);
        }
    }

    // Upload: reuse existing mesh/material slots in place (renderer free path).
    let prior = std::mem::take(&mut state.plant_objects);
    let mut objs = Vec::with_capacity(builders.len());
    for (i, b) in builders.into_iter().enumerate() {
        let mesh = crate::renderer::mesh::Mesh::from_vertices(
            &state.renderer.device,
            &b.vertices,
            &b.indices,
        );
        if let Some(&(mi, ma)) = prior.get(i) {
            state.renderer.replace_mesh(mi, mesh);
            objs.push((mi, ma));
        } else {
            let mi = state.renderer.add_mesh(mesh);
            // Type 12: albedo comes from the packed per-face UV colors.
            let ma = state.renderer.add_material_typed([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 12.0);
            objs.push((mi, ma));
        }
    }
    state.plant_objects = objs;
}

/// Rebuild the home connection cylinders from the live machine layout (gui_state.home_machines
/// + room_bounds): one colored cylinder per connection, between the two machines' low pipe
/// anchors. Uses a cached unit cylinder + a material cached per kind, so a per-frame rebuild
/// never leaks. Replaces the old static routed pipes -- connections now follow rooms. (v0.530)
pub(crate) fn rebuild_connection_objects(state: &mut EngineState) {
    use std::collections::HashMap;
    state.connection_objects.clear();
    state.connection_flow_paths.clear();
    let rooms: HashMap<String, crate::machines::RoomGeom> = state
        .gui_state
        .room_bounds
        .iter()
        .map(|rb| {
            (
                rb.id.clone(),
                crate::machines::RoomGeom {
                    center_x: (rb.min.x + rb.max.x) * 0.5,
                    center_z: (rb.min.z + rb.max.z) * 0.5,
                    floor_y: rb.min.y,
                    ceiling_y: rb.max.y,
                },
            )
        })
        .collect();
    if rooms.is_empty() {
        return;
    }
    // v0.538: box-mode absolute positioning when a ship home is active (mirrors
    // rebuild_machine_objects so the conduit anchors match the machine meshes). v0.754:
    // per-zone footprints; conduit NODES clamp into the whole ship's AABB (they carry no zone).
    let zone_rects = state.gui_state.ship_structure.as_ref().map(|s| s.zone_rects());
    let node_bounds = match state.gui_state.ship_structure.as_ref() {
        Some(ship) => {
            let (mn, mx) = ship.world_bounds();
            ((mn.x, mn.y, mn.z), (mx.x, mx.y, mx.z))
        }
        None => ((0.0, 0.0, 0.0), (0.0, 0.0, 0.0)),
    };
    let (placements, connections) = match &state.gui_state.home_machines {
        Some(h) => (h.placements(&rooms, zone_rects.as_deref()), h.connections.clone()),
        None => return,
    };
    // Low pipe-height anchor per machine id (the fixture port the conduit drops to).
    let anchors: HashMap<String, Vec3> = placements
        .iter()
        .map(|p| (p.id.clone(), Vec3::new(p.pos.0, p.floor_y + 0.35, p.pos.2)))
        .collect();
    // Combined routing list (v0.581): both the legacy point-to-point connections AND the conduit
    // NODE GRAPH edges (machine/node -> machine/node) become (a, b, kind) routes, fed through the
    // SAME route_conduit + emit below. A node edge renders as a real routed pipe with zero new mesh.
    // (a, b, kind, from_id, to_id) -- the ids let the flow markers light up only the SELECTED
    // machine's connections (v0.623). A conduit-NODE endpoint is keyed "node:<id>".
    let mut routes: Vec<(Vec3, Vec3, String, String, String)> = {
        // v0.627 (grid S1): a wire TERMINATES at the matching-utility PORT NODE (the sphere+arrow
        // gizmo above the machine) instead of the generic floor anchor, so a cable visibly plugs
        // into its node -- a water pipe to the water node, the power wire to the power node, so the
        // two also leave the machine at different points (less overlap). Falls back to the floor
        // anchor if the machine declares no port of that utility.
        let port_pick = &state.port_pick;
        let port_pos = |id: &str, kind: &str| -> Option<Vec3> {
            port_pick
                .iter()
                .find(|(mid, _, port, _)| mid == id && port.utility.id() == kind)
                .map(|(_, _, _, wp)| *wp)
        };
        connections
            .iter()
            .filter_map(|c| {
                let a = port_pos(&c.from, &c.kind).or_else(|| anchors.get(&c.from).copied())?;
                let b = port_pos(&c.to, &c.kind).or_else(|| anchors.get(&c.to).copied())?;
                Some((a, b, c.kind.clone(), c.from.clone(), c.to.clone()))
            })
            .collect()
    };
    {
        let placement_tuples: Vec<(String, (f32, f32, f32), f32)> =
            placements.iter().map(|p| (p.id.clone(), p.pos, p.floor_y)).collect();
        let end_id = |e: &crate::machines::ConduitEnd| match e {
            crate::machines::ConduitEnd::Machine(id) => id.clone(),
            crate::machines::ConduitEnd::Node(id) => format!("node:{id}"),
        };
        if let Some(home) = state.gui_state.home_machines.as_ref() {
            for e in &home.conduit_edges {
                if let (Some(a), Some(b)) = (
                    home.conduit_anchor(&e.from, &placement_tuples, node_bounds),
                    home.conduit_anchor(&e.to, &placement_tuples, node_bounds),
                ) {
                    routes.push((Vec3::new(a.0, a.1, a.2), Vec3::new(b.0, b.1, b.2), e.kind.clone(), end_id(&e.from), end_id(&e.to)));
                }
            }
        }
    }
    // Rainbow emissive materials for the selected line's flow markers (v0.623), created once.
    // Moderate emissive (1.4) + some roughness so the little beads still READ AS SPHERES (a
    // gradient across the curve) instead of flat-bright discs -- the v0.622 markers were emissive
    // 3.0, which washed out the shading and looked inside-out ("inverted normals", operator).
    if state.flow_rgb_mats.is_empty() {
        for k in 0..16u32 {
            let h = k as f32 / 16.0;
            let (r, g, b) = hsv_to_rgb(h, 0.85, 1.0);
            let m = state.renderer.add_material_full([r, g, b, 1.0], 0.0, 0.55, 0.0, 1.4);
            state.flow_rgb_mats.push(m);
        }
    }
    if routes.is_empty() {
        return;
    }
    // Cached unit cylinder mesh (+Y, base at origin, radius 0.05, height 1) -- reused for every
    // conduit segment + fitting, scaled/rotated, so a rebuild never leaks.
    let cyl = match state.connection_cyl {
        Some(m) => m,
        None => {
            let m = state
                .renderer
                .add_mesh(Mesh::cylinder(&state.renderer.device, 0.05, 1.0, 8));
            state.connection_cyl = Some(m);
            m
        }
    };
    // Home geometry for routing (v0.536): run conduits UP to a service height near the ceiling
    // and ACROSS in Manhattan legs (never a straight diagonal through the room -- the operator's
    // "the straight lines that pass through everything is wrong"), placing material-aware
    // passthroughs where a run crosses an interior wall. v0.754 (multi-zone): the service
    // height + shell material come from the HOME zone (one shared service level -- honest for
    // the single-deck v1; per-zone service heights land with corridors, increment B), and the
    // wall-passthrough list is EVERY zone's walls shifted to WORLD coords so a run through any
    // zone gaskets where it crosses that zone's walls.
    let (home_h, shell_mat, walls) = match &state.gui_state.ship_structure {
        Some(ship) => {
            let home = &ship.zones[ship.home_zone_index()];
            let mut walls: Vec<crate::ship::home_structure::InteriorWall> = Vec::new();
            for z in &ship.zones {
                let (ox, oz) = (z.origin.0, z.origin.2);
                walls.extend(z.body.walls.iter().cloned().map(|mut w| {
                    w.a = (w.a.0 + ox, w.a.1 + oz);
                    w.b = (w.b.0 + ox, w.b.1 + oz);
                    w
                }));
            }
            (home.body.height, home.body.shell_material, walls)
        }
        None => (3.0, 1, Vec::new()),
    };
    let service_y = (home_h - 0.3).max(0.6);
    const CYL_R: f32 = 0.05; // the unit cylinder's modeled radius
    // Dedup the support fittings ACROSS all connections (v0.626): many pipes share the same service-
    // height run, so without this their ceiling hangers + wall gaskets stack at the SAME spot --
    // invisible overlap that still costs polygons (the operator's "brackets overlap, more polys than
    // we should"). Key by rounded position so one bracket serves all pipes passing that point.
    let mut placed_fittings: HashMap<(i32, i32, i32), ()> = HashMap::new();
    for (a, b, kind_str, from_id, to_id) in &routes {
        let (a, b) = (*a, *b);
        let kind = crate::ship::conduits::ConduitKind::for_resource(kind_str);
        let route = crate::ship::conduits::route_conduit(a, b, kind, service_y, shell_mat, &walls);
        // Stash the routed path + the from/to ids, so the SELECTED machine's connections animate
        // their flow markers (v0.623) while every other pipe stays a quiet static line.
        if route.points.len() >= 2 {
            if state.connection_flow_sphere.is_none() {
                let s = state.renderer.add_mesh(Mesh::sphere(&state.renderer.device, 0.10, 10, 10));
                state.connection_flow_sphere = Some(s);
            }
            state.connection_flow_paths.push((route.points.clone(), from_id.clone(), to_id.clone()));
        }
        // Pipe material: a slightly-emissive UTILITY COLOUR (the connection_color legend) so each
        // run reads as its own utility (yellow=power, blue=water, ...) instead of all-grey pipes
        // (v0.623, the operator's "varied pipes"); rigid vs flexible still varies metal/roughness.
        let pkey = format!("conduit:{kind_str}");
        let pipe_mat = match state.connection_mats.get(&pkey) {
            Some(&m) => m,
            None => {
                let (met, rough) = if kind.is_rigid() { (0.6, 0.3) } else { (0.0, 0.7) };
                let c = crate::machines::MachineHome::connection_color(kind_str);
                // A touch of emissive (0.5) so the pipe is faintly visible in the dark, but far less
                // than the selected line's flow markers (so the selection still stands out).
                let m = state.renderer.add_material_full([c[0], c[1], c[2], 1.0], met, rough, 0.0, 0.5);
                state.connection_mats.insert(pkey.clone(), m);
                m
            }
        };
        let rscale = kind.radius() / CYL_R;
        // The routed pipe: one cylinder per leg (up, across, across, down).
        for seg in route.points.windows(2) {
            let (p, q) = (seg[0], seg[1]);
            let diff = q - p;
            let len = diff.length();
            if len < 1e-4 {
                continue;
            }
            let rot = Quat::from_rotation_arc(Vec3::Y, diff / len);
            state
                .connection_objects
                .push((cyl, pipe_mat, p, rot, Vec3::new(rscale, len, rscale)));
        }
        // Procedural support structures: a ceiling hanger at each service-height bracket + a
        // material-aware gasket collar at each wall passthrough. The fitting colour comes from the
        // material it attaches to, so a steel vs wood wall reads differently.
        for f in &route.fittings {
            // Skip a fitting whose spot is already bracketed by another pipe's run (v0.626 dedup).
            let pos_key = ((f.at.x * 5.0) as i32, (f.at.y * 5.0) as i32, (f.at.z * 5.0) as i32);
            if placed_fittings.insert(pos_key, ()).is_some() {
                continue;
            }
            let fkey = format!("fitting:{}", f.material);
            let fmat = match state.connection_mats.get(&fkey) {
                Some(&m) => m,
                None => {
                    let col = match f.material {
                        1 => [0.58, 0.60, 0.65, 1.0], // steel
                        2 => [0.64, 0.64, 0.62, 1.0], // concrete
                        3 => [0.52, 0.37, 0.22, 1.0], // wood
                        _ => [0.50, 0.52, 0.56, 1.0],
                    };
                    let m = state.renderer.add_material_typed(col, 0.6, 0.4, f.material as f32);
                    state.connection_mats.insert(fkey.clone(), m);
                    m
                }
            };
            match f.kind {
                crate::ship::conduits::FittingKind::Bracket => {
                    // Ceiling hanger (a thin post up to the ceiling) for the horizontal service
                    // runs; the short vertical drops are held at their ends, so skip them.
                    if f.at.y >= service_y - 0.1 {
                        let drop = (home_h - f.at.y).max(0.05);
                        state.connection_objects.push((
                            cyl,
                            fmat,
                            f.at,
                            Quat::IDENTITY,
                            Vec3::new(0.5, drop, 0.5),
                        ));
                    }
                }
                crate::ship::conduits::FittingKind::Passthrough => {
                    // A short gasket collar straddling the wall at the crossing.
                    state.connection_objects.push((
                        cyl,
                        fmat,
                        f.at - Vec3::new(0.0, 0.12, 0.0),
                        Quat::IDENTITY,
                        Vec3::new(2.4, 0.24, 2.4),
                    ));
                }
                crate::ship::conduits::FittingKind::Elbow => {}
            }
        }
    }
}

/// Recompute the door/window panel placements from the live HomeStructure (v0.537). Called after
/// a structure rebuild + on load. Preserves the per-panel open fraction when the panel COUNT is
/// unchanged (so editing a far wall does not slam every door shut); otherwise resets to closed.
/// Corridor-mouth door pairs (v0.795) are part of the same list -- `ship_panel_placements`
/// appends them -- so a corridor add/move/remove in the editor re-derives its doors right here,
/// and their open fractions live in the same `door_panels` Vec as every other door's.
pub(crate) fn rebuild_door_panels(state: &mut EngineState) {
    // v0.754: every zone's doors, at world positions (per-zone origin offsets).
    let placements = match &state.gui_state.ship_structure {
        Some(ship) => crate::ship::door_panels::ship_panel_placements(ship),
        None => Vec::new(),
    };
    if placements.len() == state.door_panels.len() {
        for (i, p) in placements.into_iter().enumerate() {
            state.door_panels[i].0 = p;
        }
    } else {
        state.door_panels = placements.into_iter().map(|p| (p, 0.0)).collect();
    }
    // Reset every manual door to CLOSED on a structural rebuild (v0.567). This runs only on a
    // structure edit / world load (build mode, orbit cam), never while walking, so we deliberately
    // do NOT trust positional parallelism across an edit -- an open-flag must never land on the
    // wrong door just because the opening count happened to stay equal.
    state.door_manual_open = vec![false; state.door_panels.len()];
    // Reset live lock state to each door's AUTHORED states on a rebuild (v0.570), parallel to
    // door_panels. Same reasoning as the manual-open reset above.
    state.door_locks = state
        .door_panels
        .iter()
        .map(|(p, _)| p.locks.iter().map(|l| l.state).collect())
        .collect();
}

/// Is door `panel` currently locked, using its LIVE lock states when present (v0.570)? A door with
/// locks is locked iff any live lock is not open; an empty lock list falls back to the legacy
/// `panel.locked` bool, so v0.567 doors are unchanged. `live` is `door_locks[i]`.
pub(crate) fn door_locked_now(
    panel: &crate::ship::door_panels::PanelPlacement,
    live: Option<&Vec<crate::ship::lock_types::LockState>>,
) -> bool {
    if panel.locks.is_empty() {
        return panel.locked;
    }
    match live {
        Some(states) if states.len() == panel.locks.len() => states.iter().any(|s| !s.is_open()),
        _ => panel.locks.iter().any(|l| !l.state.is_open()), // fall back to authored
    }
}

/// The room point-lights to upload (v0.571, refined v0.572). A home's PLACED lights (resolved from
/// light_types.ron + per-instance overrides) take over once ANY are placed; otherwise the crude
/// `auto` one-per-room fill is used, and ONLY when GI is on. Rationale (operator v0.572 feedback):
/// the auto fill is a single bright point light at room centre that reads as an ugly "sun spotlight"
/// pool -- so once the operator places their own lights, we drop it entirely (their lights ARE the
/// room lighting; the directional SUN, gated separately by GI, still provides the even base when GI
/// is on). With NO placed lights the old behaviour is unchanged (auto fill when GI on, dark when off).
pub(crate) fn home_lights(
    ship: Option<&ShipStructure>,
    auto: Vec<crate::renderer::light::RoomLight>,
    gi_on: bool,
) -> Vec<crate::renderer::light::RoomLight> {
    use crate::renderer::light::{LightKind, RoomLight};
    // v0.754: EVERY zone's placed lights, each offset by its zone's world origin.
    let placed: Vec<RoomLight> = ship
        .map(|s| {
            s.zones
                .iter()
                .flat_map(|z| {
                    let o = z.origin_vec();
                    z.body.lights.iter().filter(|l| l.on).filter_map(move |l| {
                        let t = crate::renderer::light::light_type(&l.type_id)?;
                        let c = l.color.unwrap_or(t.color);
                        let pos = Vec3::new(l.pos.0, l.pos.1, l.pos.2) + o;
                        let color = [c.0, c.1, c.2];
                        let intensity = l.intensity.unwrap_or(t.intensity);
                        let range = l.range.unwrap_or(t.range);
                        Some(match t.kind {
                            LightKind::Spot => {
                                let dir = Vec3::new(l.dir.0, l.dir.1, l.dir.2);
                                vec![RoomLight::spot(pos, color, intensity, range, dir, t.cone_inner_deg, t.cone_outer_deg)]
                            }
                            // A strip is a LINE LIGHT (v0.786, operator: "make the
                            // full length of the bar emit light onto surfaces"):
                            // one segment light per sampled leg, each lit from its
                            // closest point in the shader. Intensity is split
                            // across segments by length so the strip's total
                            // output matches its dial (energy conservation).
                            // Emission follows the SAME subdivided curve as the
                            // rendered tube (v0.792) -- it used to follow the
                            // straight CONTROL polyline, so a curved strip's
                            // rounded sections glowed without lighting anything
                            // (operator screenshot). strip_emission_segments caps
                            // the per-strip segment count so a 100-subdivision
                            // strip can't flood the (uncapped, v0.782) light
                            // buffer with useless segments.
                            LightKind::Bar => {
                                let (pts, sub): (Vec<Vec3>, u32) = if l.path.is_empty() {
                                    // Pathless: the classic straight bar of the
                                    // type's length along the horizontal dir. A
                                    // straight segment has nothing to subdivide.
                                    let flat = Vec3::new(l.dir.0, 0.0, l.dir.2);
                                    let axis = if flat.length_squared() > 1e-4 {
                                        flat.normalize()
                                    } else {
                                        Vec3::X
                                    };
                                    let half = t.length_m.max(0.3) * 0.5;
                                    (vec![pos - axis * half, pos + axis * half], 0)
                                } else {
                                    (
                                        std::iter::once(pos)
                                            .chain(l.path.iter().map(|p| Vec3::new(p.0, p.1, p.2) + o))
                                            .collect(),
                                        l.subdivision,
                                    )
                                };
                                crate::renderer::light::strip_emission_segments(&pts, sub)
                                    .into_iter()
                                    .map(|(a, b, share)| {
                                        RoomLight::line(a, b, color, intensity * share, range)
                                    })
                                    .collect()
                            }
                            _ => vec![RoomLight::point(pos, color, intensity, range)],
                        })
                    })
                    .flatten()
                })
                .collect()
        })
        .unwrap_or_default();
    // Any placed lights -> the home is manually lit, no auto centre-spot. Else auto fill if GI on.
    if !placed.is_empty() {
        placed
    } else if gi_on {
        auto
    } else {
        Vec::new()
    }
}

/// Per-frame: animate + emit the door/window panels (v0.537). A door eases open as an actor
/// approaches (v0.795: the nearest of local player / remote players / creatures -- see the
/// actor gather below) by its data-driven style via systems::door_anim; a window is a fixed
/// glass pane. Corridor-mouth door pairs (v0.795) are ordinary entries in the same list, so
/// they animate + collide with zero extra machinery here. Reuses one cached unit-box mesh + a
/// slab + a glass material (scaled/rotated/animated per frame), so nothing leaks. Doors go to
/// the opaque pass, glass to the transparent pass.
pub(crate) fn render_door_panels(
    state: &mut EngineState,
    opaque: &mut Vec<RenderObject>,
    transparent: &mut Vec<RenderObject>,
    ring_lines: &mut Vec<crate::renderer::line::LineVertex>,
    dt: f32,
) {
    if state.door_panels.is_empty() {
        return;
    }
    let mesh = match state.door_panel_mesh {
        Some(m) => m,
        None => {
            let m = state.renderer.add_mesh(Mesh::box_xyz(&state.renderer.device, 1.0, 1.0, 1.0));
            state.door_panel_mesh = Some(m);
            m
        }
    };
    let slab_mat = match state.door_slab_mat {
        Some(m) => m,
        None => {
            // theme-exempt: world-object material, not a themed UI surface.
            let m = state.renderer.add_material_typed([0.36, 0.38, 0.43, 1.0], 0.3, 0.5, 1.0);
            state.door_slab_mat = Some(m);
            m
        }
    };
    let glass_mat = match state.door_glass_mat {
        Some(m) => m,
        None => {
            // theme-exempt: tinted glass, transparent pass.
            let m = state.renderer.add_material_full([0.55, 0.78, 0.92, 0.34], 0.0, 0.08, 1.0, 0.10);
            state.door_glass_mat = Some(m);
            m
        }
    };
    // Energy + nanowall door materials (v0.554), all rendered in the transparent pass: an ENERGY
    // door is a glowing FIELD -- green while operable, red while LOCKED; a NANOWALL is a metallic
    // semi-transparent surface you see through as it dissolves open.
    let energy_open_mat = match state.door_energy_open_mat {
        Some(m) => m,
        None => {
            // theme-exempt: glowing green energy field.
            let m = state.renderer.add_material_full([0.20, 1.0, 0.40, 0.42], 0.0, 0.3, 1.0, 1.4);
            state.door_energy_open_mat = Some(m);
            m
        }
    };
    let energy_locked_mat = match state.door_energy_locked_mat {
        Some(m) => m,
        None => {
            // theme-exempt: glowing red energy field (locked).
            let m = state.renderer.add_material_full([1.0, 0.18, 0.20, 0.50], 0.0, 0.3, 1.0, 1.4);
            state.door_energy_locked_mat = Some(m);
            m
        }
    };
    let nanowall_mat = match state.door_nanowall_mat {
        Some(m) => m,
        None => {
            // theme-exempt: metallic gray nanowall, semi-transparent.
            let m = state.renderer.add_material_full([0.62, 0.64, 0.70, 0.60], 0.85, 0.15, 1.0, 0.15);
            state.door_nanowall_mat = Some(m);
            m
        }
    };
    // Nanowall shimmer (v0.554): drift the metallic gray + emissive over time so the surface reads
    // as a live, shifting "water" field rather than a static slab. One shared-material write/frame.
    state.door_anim_time += dt.max(0.0);
    let shimmer = 0.5 + 0.5 * (state.door_anim_time * 1.6).sin();
    let g = 0.58 + 0.10 * shimmer;
    state.renderer.update_material_full(nanowall_mat, [g * 0.94, g, g * 1.06, 0.60], 0.85, 0.10 + 0.08 * shimmer, 1.0, 0.08 + 0.16 * shimmer);
    // Auto-doors open for ANY nearby actor, not just the local player (v0.795, with the
    // corridor doors): a REMOTE player must be able to walk through a corridor mouth on your
    // screen (you SEE the door part for them, and its collider clears for you both), and a
    // wandering animal should not phase through a shut door's visual. Gathered once per frame:
    // the camera (the local player) + every RemotePlayer + every Creature transform. The
    // per-door check below takes the NEAREST actor, so one loiterer holds the door open.
    let mut actors: Vec<Vec3> = vec![state.camera.position];
    for (_e, (t, _)) in state
        .game_world
        .world
        .query::<(&crate::ecs::components::Transform, &crate::net::sync::RemotePlayer)>()
        .iter()
    {
        actors.push(t.position);
    }
    for (_e, (t, _)) in state
        .game_world
        .world
        .query::<(&crate::ecs::components::Transform, &crate::ecs::components::Creature)>()
        .iter()
    {
        actors.push(t.position);
    }
    // v0.547: per-door open distance. The interaction ring shows it in build mode / dev overlay.
    // The ring is a constant-width LINE circle now (v0.568), so there is no polygon-ring mesh.
    let show_widgets = state.gui_state.construction_active || state.gui_state.construction_dev_overlay;
    // Snapshot the per-door manual-open flags (v0.567) so the loop can read them while it holds a
    // &mut on door_panels (a disjoint-field borrow the checker won't always see through).
    let manual = state.door_manual_open.clone();
    let locks_live = state.door_locks.clone();
    for (di, (p, open)) in state.door_panels.iter_mut().enumerate() {
        // An operable DOOR opens on approach; a window or a "fixed"-styled opening stays shut
        // (v0.538: consult door_anim::is_operable so a door explicitly styled "fixed" does not
        // chase an open target it can never animate to).
        let operable = !p.is_window && crate::systems::door_anim::is_operable(&p.style);
        // Is the door LOCKED right now (v0.570)? Live lock states if present, else the legacy bool.
        let locked_now = door_locked_now(p, locks_live.get(di));
        // Interaction-distance ring on the floor at the door (v0.547), drawn as a LINE circle
        // (v0.565, operator's idea -- like the orbit paths) so its width is CONSTANT regardless of
        // radius, instead of a polygon strip that thickened as open_dist grew.
        if show_widgets && operable && p.auto_open {
            const RING_COL: [f32; 4] = [0.35, 0.85, 1.0, 0.9]; // cyan
            crate::renderer::line::push_circle(
                ring_lines, [p.center.x, 0.04, p.center.z], p.open_dist, RING_COL, 72,
            );
        }
        // Wall-mounted CONTROL PANEL beside a manual/controlled door (v0.567): a glowing tech panel
        // the player walks up to and presses E. Green while openable, red while LOCKED. Routed to the
        // transparent pass since it glows. Drawn before the door's hidden-check so it always shows.
        // Only on a MANUAL door -- an auto door opens by itself, so its panel would be a dead control.
        if p.control_panel && !p.auto_open {
            let cp = p.control_panel_pos;
            let mat = if locked_now { energy_locked_mat } else { energy_open_mat };
            transparent.push(RenderObject { fade: 0.0,
                position: Vec3::new(cp.x, cp.y - 0.14, cp.z),
                rotation: p.rotation,
                scale: Vec3::new(0.18, 0.28, 0.06),
                mesh,
                material: mat,
            });
        }
        // Lock indicators (v0.570): a small box per lock on the door face -- RED locked, GREEN
        // unlocked, GREY broken. Shows whether (and how) a door is secured even without a panel.
        // Doors only -- a window is a fixed pane (locks on a hand-authored window are inert).
        if !p.is_window {
            for (li, lock) in p.locks.iter().enumerate() {
                let st = locks_live.get(di).and_then(|v| v.get(li)).copied().unwrap_or(lock.state);
                let lm = match st {
                    crate::ship::lock_types::LockState::Locked => energy_locked_mat,
                    crate::ship::lock_types::LockState::Unlocked => energy_open_mat,
                    crate::ship::lock_types::LockState::Broken => slab_mat,
                };
                transparent.push(RenderObject { fade: 0.0,
                    position: Vec3::new(lock.pos.x, lock.pos.y - 0.05, lock.pos.z),
                    rotation: p.rotation,
                    scale: Vec3::new(0.1, 0.1, 0.05),
                    mesh,
                    material: lm,
                });
            }
        }
        // Nearest actor's HORIZONTAL distance -- eye/body height must not count, or a tall
        // camera would never trigger a short door.
        let dist = actors
            .iter()
            .map(|a| {
                let dx = a.x - p.center.x;
                let dz = a.z - p.center.z;
                (dx * dx + dz * dz).sqrt()
            })
            .fold(f32::MAX, f32::min);
        let target = if !operable || locked_now {
            // A fixed pane or a LOCKED door never opens (v0.570: lock-list aware).
            0.0
        } else if !p.auto_open {
            // A MANUAL door (v0.564) opens only when toggled at its control panel (v0.567).
            if manual.get(di).copied().unwrap_or(false) { 1.0 } else { 0.0 }
        } else {
            // Proximity + hysteresis (v0.540, pure + tested in door_anim since v0.795).
            crate::systems::door_anim::auto_open_target(dist, p.open_dist, *open)
        };
        // Frame-rate-independent exponential ease toward the target: smooth open/close, ~0.4 s
        // to settle, no snapping (v0.540, pure + tested in door_anim since v0.795).
        *open = crate::systems::door_anim::ease_open(*open, target, dt);
        let m = crate::systems::door_anim::panel_motion(&p.style, *open, p.size.x, p.size.y);
        if m.hidden {
            continue;
        }
        let hinge_rot = Quat::from_rotation_y(m.hinge);
        let world_off = p.rotation * Vec3::new(m.offset.0, m.offset.1, m.offset.2);
        let c = p.center + world_off;
        let pos = p.hinge + hinge_rot * (c - p.hinge);
        let rot = hinge_rot * p.rotation;
        let scale = Vec3::new(p.size.x * m.scale.0, p.size.y * m.scale.1, p.size.z * m.scale.2);
        // Pick the panel material by style + lock state, and route glowing / glassy panels through
        // the transparent pass so they blend (v0.554).
        let (material, is_transparent) = if p.is_window {
            (glass_mat, true)
        } else if p.style == "energy" {
            // v0.570: lock-list aware (was `p.locked`), so an energy door driven by a lock list
            // glows red while actually impassable instead of a misleading green.
            (if locked_now { energy_locked_mat } else { energy_open_mat }, true)
        } else if p.style == "nanowall" {
            (nanowall_mat, true)
        } else {
            (slab_mat, false)
        };
        let obj = RenderObject { fade: 0.0, position: pos, rotation: rot, scale, mesh, material };
        if is_transparent {
            transparent.push(obj);
        } else {
            opaque.push(obj);
        }
    }
}
