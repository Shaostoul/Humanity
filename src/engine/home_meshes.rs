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
            state.renderer.update_material_full(ma, [0.30, 0.55, 1.0, 1.0], 0.2, 0.15, 1.0, 0.0);
            Some((mi, ma))
        } else {
            Some((state.renderer.add_mesh(mesh), state.renderer.add_material_full([0.30, 0.55, 1.0, 1.0], 0.2, 0.15, 1.0, 0.0)))
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
        .map(|r| {
            // The function join: a zone-named room resolves through its zone's room_type
            // (rooms.ron); a legacy id joins on itself; an anonymous room_N gets its id as name.
            let f = room_types.function_for(r);
            crate::gui::RoomBounds {
                id: r.id.clone(),
                min: r.center - r.dimensions * 0.5,
                max: r.center + r.dimensions * 0.5,
                display_name: f.display_name,
                purpose: f.purpose,
                actions: f.actions,
                action_pages: room_types.action_pages(r),
                access: f.access,
            }
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

/// The current machine placements, resolved the way `rebuild_machine_objects`
/// resolves them (room bounds -> RoomGeom, the ship's zone rects, the live
/// `home_machines`). `None` before the room geometry exists. Used by the
/// world load to build the in-world screens once the machines are placed,
/// without duplicating the resolve at that call site.
pub(crate) fn current_placements(state: &EngineState) -> Option<Vec<crate::machines::PlacedMachine>> {
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
    Some(home.placements(&rooms, zone_rects.as_deref()))
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
        // In-world screens follow their bodies on a move: geometry only, no
        // surface or renderer slot is touched (a drag must not recreate the
        // page's context and lose its scroll state).
        state.screens.update_poses(&placements);
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
        // v0.993 (textured machine models): the TEXTURED parse runs first;
        // a model that carries an albedo texture (the Kenney furniture
        // palettes) gets a type-19 textured material from the per-model
        // cache instead of the def's flat color.
        let mut model_tex: Option<(Vec<u8>, u32, u32)> = None;
        let mut model_path: Option<String> = None;
        let mesh = p
            .model
            .as_deref()
            .and_then(|m| {
                state
                    .asset_manager
                    // Fitted to the def SIZE height (v0.1324): the pack is authored at
                    // about half real scale, so an unfitted model put a 0.46 m chair
                    // next to a player whose eyes are at 1.7 m.
                    .parse_gltf_mesh_textured_fit_height(&state.renderer.device, m, p.size.1)
                    .map_err(|e| {
                        log::warn!("machine {} model '{m}' failed: {e}; primitive fallback", p.id)
                    })
                    .ok()
                    .map(|(mesh, tex)| {
                        model_tex = tex;
                        model_path = Some(m.to_string());
                        mesh
                    })
            })
            .unwrap_or_else(|| machine_mesh(&state.renderer.device, &p.shape, p.size));
        // Shared textured material per model path, created once. NEVER
        // updated in place: instances of the same model share it.
        let textured_mat: Option<usize> = match (&model_path, model_tex) {
            (Some(mp), Some((rgba, w, h))) => {
                Some(match state.machine_model_materials.get(mp) {
                    Some(&ma) => ma,
                    None => {
                        let ma = state.renderer.add_textured_material(
                            [1.0, 1.0, 1.0, 1.0],
                            0.0,
                            0.85,
                            19.0,
                            0.0,
                            &rgba,
                            w,
                            h,
                        );
                        state.machine_model_materials.insert(mp.clone(), ma);
                        ma
                    }
                })
            }
            _ => None,
        };
        let color = [p.color.0, p.color.1, p.color.2, 1.0];
        let pos = Vec3::new(p.pos.0, p.pos.1, p.pos.2);
        if let Some(&(mi, ma, _, _)) = prior.get(i) {
            state.renderer.replace_mesh(mi, mesh);
            let use_ma = match textured_mat {
                Some(tm) => tm,
                None => {
                    // Only repaint a slot the typed path OWNS: a prior slot
                    // holding a shared textured material (machine switched
                    // model -> primitive across rebuilds) gets a fresh typed
                    // material instead of corrupting its siblings.
                    let is_shared =
                        state.machine_model_materials.values().any(|&v| v == ma);
                    if is_shared {
                        state.renderer.add_material_typed(color, 0.1, 0.7, 0.0)
                    } else {
                        state.renderer.update_material_typed(ma, color, 0.1, 0.7, 0.0);
                        ma
                    }
                }
            };
            objs.push((mi, use_ma, pos, p.rotation));
        } else {
            let mi = state.renderer.add_mesh(mesh);
            let ma = textured_mat
                .unwrap_or_else(|| state.renderer.add_material_typed(color, 0.1, 0.7, 0.0));
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
    publish_grow_plots(state);
    // In-world screens (rung 2): a surface + display quad per placed machine
    // with a `screen` def. Reuses surfaces by instance id (a count change
    // that keeps a screen keeps its page state), drops the ones whose
    // machine is gone. Same prior-slot reuse discipline as the bodies above.
    crate::engine::screens::sync_screens(state, &placements);
    state.plant_mesh_sig = 0; // machine layout may have moved: replant visuals
    rebuild_connection_objects(state);
}

/// Publish the home's grow machines for the farming system (2026-09-26),
/// from the grow anchors just recorded, in placement order:
/// - "grow_plots" (`Vec<lighting::GrowPlot>`): where each stands, its
///   footprint and a tower's cups, for the grow lights (farming::lighting);
/// - "grow_instances" (type -> [(instance id, plots)]): the machines a bed
///   Plant button for a TYPE sows, one crop per plot;
/// - "grow_plot_area_m2" (instance id and type id -> m2 of one plot): the
///   floor one crop in that machine has.
/// A grow machine is one a grow medium (data/garden/grow_media.ron) matches;
/// a medium's `plots` divides a bed, tray, rack or field into plots.
pub(crate) fn publish_grow_plots(state: &mut EngineState) {
    use crate::systems::farming::lighting::GrowPlot;
    use std::collections::HashMap;
    let tower_cfgs = crate::gui::load_tower_configs(&crate::data_dir());
    let mut plots: Vec<GrowPlot> = Vec::new();
    let mut instances: HashMap<String, Vec<(String, u32)>> = HashMap::new();
    let mut plot_area: HashMap<String, f32> = HashMap::new();
    for g in &state.grow_positions {
        let Some(medium) = state.gui_state.grow_media.iter().find(|m| m.matches(&g.ty)) else { continue };
        let footprint = (g.size.0 * g.size.2).max(0.0);
        let tower_cfg = g.ty.strip_prefix("aeroponic_tower_").and_then(|k| tower_cfgs.iter().find(|t| t.id == k));
        plots.push(GrowPlot {
            id: g.id.clone(),
            aliases: tower_cfg.map(|t| vec![t.id.clone()]).unwrap_or_default(),
            pos: g.pos.to_array(),
            footprint_m2: footprint,
            cups: tower_cfg.map_or(0, |t| t.slots),
            outdoors: crate::systems::farming::is_field_area(&g.ty) || crate::systems::farming::is_field_area(&g.id),
        });
        if tower_cfg.is_none() {
            let n = medium.plots.max(1);
            let per_plot = if medium.stacked { footprint } else { footprint / n as f32 };
            instances.entry(g.ty.clone()).or_default().push((g.id.clone(), n));
            plot_area.insert(g.id.clone(), per_plot);
            plot_area.entry(g.ty.clone()).or_insert(per_plot);
        }
    }
    state.data_store.insert("grow_plots", plots);
    state.data_store.insert("grow_instances", instances);
    state.data_store.insert("grow_plot_area_m2", plot_area);
}

/// Procedural plants (v0.862): merged world-space plant meshes built from the
/// live CropInstances, each drawn as one RenderObject.
///
/// - Towers: one mesh per planted tower, a plant per net cup up the helix.
///   Crops bound to a tower CONFIG id ("nutrition", the GUI Plant button)
///   dress the first placed column of that type; a tower INSTANCE id dresses
///   that column.
/// - Beds, trays, racks and fields (2026-09-26): each crop is one plot of its
///   machine and draws the plants that plot really holds, the count the farm
///   harvests, feeds and waters (`farming::units::plants_in_plot`), laid out
///   at the crop's spacing by `engine::plant_layout`. Above the per-plot cap
///   in data/plants_visual.ron (`plot_visual_cap`) a plot draws that many
///   clumps instead, each as wide as the plants it stands for.
/// - A species with converted stage models (assets/models/plants/<set>_1..4)
///   bakes one merged mesh per machine and model, so a plot of 128 wheat
///   clumps is one draw, not 128. Procedural plants share one mesh per machine.
///
/// Cheap change-signature gate: rebuilds only when growth actually moves, and
/// then only the machines whose crops changed (2026-09-26; a full rebuild of
/// the showcase's ~2,000 crops was measured at about 450 ms, a hitch every
/// time any one crop changed stage).
pub(crate) fn rebuild_plant_meshes(state: &mut EngineState) {
    use crate::engine::plant_layout::{bake_copy, plot_draw_cap, plot_plants, plot_rects};
    use crate::renderer::plant_mesh::PlantMeshBuilder;
    use crate::systems::farming::units;
    use std::collections::HashMap;
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
    // Rebuild cost (2026-09-26): the rebuild runs inside the frame that
    // noticed the change, so its milliseconds are a hitch on that frame. It
    // shows on the Performance page (cpu.plant_rebuild, in the vegetation
    // slice) and is summarised in run.log by `note_plant_rebuild`.
    let _cost = crate::renderer::frame_costs::stage("cpu.plant_rebuild");
    let rebuild_t0 = std::time::Instant::now();
    // Hero stage models used to draw one object per crop (v0.992,
    // `hero_plant_objects`, removed 2026-09-26); they bake into
    // `plant_objects` now, one draw per machine and model.
    let mut cache = match state.data_store.get::<PlantMeshCache>(PLANT_MESH_CACHE_KEY) {
        Some(c) => c.clone(),
        None => {
            // No cache yet (or it was lost): any meshes already drawn become
            // spare slots, so nothing leaks and nothing is drawn twice.
            let mut fresh = PlantMeshCache::default();
            for (mi, _) in std::mem::take(&mut state.plant_objects) {
                release_plant_slot(state, &mut fresh, mi);
            }
            fresh
        }
    };

    // Resolved UP FRONT into owned values (v0.992): holding the registry
    // borrows across the crop loop would forbid the &mut state calls the
    // hero-model loader needs. Per species its stage names; per (species,
    // machine) the plants one plot of it holds, counted exactly as the farm
    // counts them (the plot areas `publish_grow_plots` put in the DataStore).
    let (stage_map, plot_plant_count) = {
        let plant_reg = state
            .data_store
            .get::<crate::systems::farming::PlantRegistry>("plant_registry");
        let plot_areas = state.data_store.get::<HashMap<String, f32>>(units::PLOT_AREA_KEY);
        let mut stages: HashMap<String, Vec<String>> = HashMap::new();
        let mut counts: HashMap<(String, String), u32> = HashMap::new();
        for (_e, c) in state
            .game_world
            .world
            .query::<&crate::ecs::components::CropInstance>()
            .iter()
        {
            let def = plant_reg.and_then(|r| r.get(&c.crop_def_id));
            if !stages.contains_key(&c.crop_def_id) {
                let names: Vec<String> = def
                    .map(|d| d.stages().iter().map(|s| s.to_string()).collect())
                    .unwrap_or_else(|| {
                        crate::ecs::components::DEFAULT_GROWTH_STAGES
                            .iter()
                            .map(|s| s.to_string())
                            .collect()
                    });
                stages.insert(c.crop_def_id.clone(), names);
            }
            if let Some(tid) = &c.tower_id {
                counts.entry((c.crop_def_id.clone(), tid.clone())).or_insert_with(|| {
                    def.map_or(1, |d| units::plants_in_plot(d, units::plot_area(plot_areas, Some(tid))))
                });
            }
        }
        (stages, counts)
    };
    // Visual defs: read fresh from disk each rebuild (rebuilds are rare and
    // this is what makes live-editing plants_visual.ron work), with the
    // committed file as the only source (no embedded fallback yet).
    let vis_text = std::fs::read_to_string(crate::data_dir().join("plants_visual.ron"))
        .unwrap_or_default();
    let visuals = crate::renderer::plant_mesh::PlantVisualRegistry::from_ron(&vis_text)
        .unwrap_or_default();
    let tower_cfgs = crate::gui::load_tower_configs(&crate::data_dir());
    let default_stages: Vec<String> =
        crate::ecs::components::DEFAULT_GROWTH_STAGES.iter().map(|s| s.to_string()).collect();

    // Group crops by tower config id / grow machine instance id.
    let mut by_tower: HashMap<String, Vec<(String, String, u32, f32)>> = HashMap::new();
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
    for crops in by_tower.values_mut() {
        crops.sort_by(|x, y| x.2.cmp(&y.2).then_with(|| x.0.cmp(&y.0)));
    }

    // A machine whose plants did not change since the last rebuild keeps its
    // meshes (2026-09-26): a crop changing stage rebuilds its own machine,
    // not all ~100 of them. The machine's signature covers its crops, where
    // it stands, the visual recipes and the plants each plot holds.
    let vis_hash = {
        let mut vh = std::collections::hash_map::DefaultHasher::new();
        vis_text.hash(&mut vh);
        vh.finish()
    };
    let plant_material = match cache.material {
        Some(m) => m,
        None => {
            // Type 20 (v0.1063): albedo comes from the packed per-face UV
            // colors, same as type 12, but plants get their OWN type so they
            // (a) stop inheriting type 12's planet terminator gate, which read
            // base_color as a planet centre and switched direct sun off across
            // half of every garden, and (b) have somewhere to grow close-range
            // leaf/fruit detail. See assets/shaders/pbr/90-fragment-main.wgsl.
            let m = state.renderer.add_material_typed([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 20.0);
            cache.material = Some(m);
            m
        }
    };
    let mut groups: HashMap<String, PlantGroupMeshes> = HashMap::new();
    let mut objs: Vec<(usize, usize)> = Vec::new();
    let (mut machines_rebuilt, mut verts_built) = (0usize, 0usize);
    for (cfg_id, crops) in &by_tower {
        // Resolve the crop group's world layout. Three key shapes:
        // - legacy tower CONFIG id ("nutrition", from the GUI Plant button):
        //   dress the first placed column of that type;
        // - tower INSTANCE id ("ntower_5", from the showcase auto-seed):
        //   dress that exact column;
        // - bed/field/rack INSTANCE id: its plots across the machine.
        // grid_spot is CLONED out of state (v0.992): the hero-model loader
        // below needs &mut state mid-loop, which a live &GrowSpot borrow
        // would forbid. GrowSpot is a few strings + floats; rebuilds are rare.
        let (helix_cfg, base, grid_spot) = if let Some(cfg) =
            tower_cfgs.iter().find(|t| t.id == *cfg_id)
        {
            let cat_key = format!("aeroponic_tower_{cfg_id}");
            match state.grow_positions.iter().find(|g| g.ty == cat_key) {
                Some(g) => (Some(cfg), g.pos, None),
                None => continue,
            }
        } else if let Some(g) = state.grow_positions.iter().find(|g| g.id == *cfg_id).cloned() {
            if let Some(cfg_key) = g.ty.strip_prefix("aeroponic_tower_") {
                match tower_cfgs.iter().find(|t| t.id == cfg_key) {
                    Some(cfg) => (Some(cfg), g.pos, None),
                    None => continue,
                }
            } else {
                let pos = g.pos;
                (None, pos, Some(g))
            }
        } else {
            continue;
        };
        let grid_spot = grid_spot.as_ref();
        let group_sig = {
            let mut gh = std::collections::hash_map::DefaultHasher::new();
            vis_hash.hash(&mut gh);
            base.to_array().map(f32::to_bits).hash(&mut gh);
            if let Some(g) = grid_spot {
                g.ty.hash(&mut gh);
                [g.size.0, g.size.1, g.size.2, g.yaw].map(f32::to_bits).hash(&mut gh);
            }
            if let Some(cfg) = helix_cfg {
                cfg.slots.hash(&mut gh);
                [cfg.height_m, cfg.helix_turns, cfg.diameter_m].map(f32::to_bits).hash(&mut gh);
            }
            for (def_id, stage, slot, health) in crops {
                def_id.hash(&mut gh);
                stage.hash(&mut gh);
                slot.hash(&mut gh);
                ((health / 10.0) as u32).hash(&mut gh);
                plot_plant_count.get(&(def_id.clone(), cfg_id.clone())).hash(&mut gh);
            }
            gh.finish()
        };
        let mut old_slots = Vec::new();
        if let Some(prev) = cache.groups.remove(cfg_id) {
            if prev.sig == group_sig {
                objs.extend_from_slice(&prev.slots);
                groups.insert(cfg_id.clone(), prev);
                continue;
            }
            old_slots = prev.slots;
        }
        let (mut drawn, mut meant) = (0usize, 0u64);
        // A bed/tray/field's plots (data/garden/grow_media.ron): the medium's
        // count, shelves when stacked. The farm gives every crop in the
        // machine one plot's floor, and a crop is sown into plot `slot`.
        let plots = grid_spot.map(|g| {
            let (n, stacked) = state
                .gui_state
                .grow_media
                .iter()
                .find(|m| m.matches(&g.ty))
                .map_or((1, false), |m| (m.plots.max(1), m.stacked));
            plot_rects(g.size, n, stacked)
        });
        let slots = helix_cfg.map(|c| c.slots.max(1)).unwrap_or(1);
        let radius = helix_cfg.map(|c| c.diameter_m * 0.5).unwrap_or(0.0);
        let mut b = PlantMeshBuilder::new();
        // This machine's stage-model meshes: model name -> (mesh, material).
        let mut hero: HashMap<String, (PlantMeshBuilder, usize)> = HashMap::new();
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
            let stages: &Vec<String> = stage_map.get(def_id).unwrap_or(&default_stages);
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
            let seed = {
                let mut sh = std::collections::hash_map::DefaultHasher::new();
                cfg_id.hash(&mut sh);
                slot.hash(&mut sh);
                sh.finish()
            };
            if let Some(cfg) = helix_cfg {
                // Helix slot position up the column, plant facing outward.
                let frac = *slot as f32 / slots as f32;
                let ang = frac * cfg.helix_turns * std::f32::consts::TAU;
                let y = 0.18 + frac * (cfg.height_m - 0.45);
                let out = [ang.cos(), 0.0, ang.sin()];
                let pos = [base.x + out[0] * radius, base.y + y, base.z + out[2] * radius];
                // Tower plants render at reduced scale so a tree in a net cup
                // reads as a dwarf/espalier rather than a full orchard tree.
                let mut vis_scaled = vis.clone();
                if vis_scaled.height_m > 0.6 {
                    let k = 0.6 / vis_scaled.height_m;
                    vis_scaled.height_m *= k;
                    vis_scaled.spread_m *= k;
                    vis_scaled.stem_radius *= k;
                }
                crate::renderer::plant_mesh::build_plant(&mut b, &vis_scaled, pos, out, t, wilt, seed);
                drawn += 1;
                meant += 1;
                continue;
            }
            let (Some(g), Some(rects)) = (grid_spot, plots.as_ref()) else { continue };
            // A slot past the medium's plots (more crops than plots) shares
            // a plot rather than standing outside the machine.
            let rect = rects[*slot as usize % rects.len().max(1)];
            let holds = plot_plant_count.get(&(def_id.clone(), cfg_id.clone())).copied().unwrap_or(1);
            let turn = Quat::from_rotation_y(g.yaw.to_radians());
            // Hero crop models (v0.992, the Quaternius growth stages): a
            // species with a converted stage model set uses the real 3D model
            // at the stage quartile instead of the procedural recipe. The set
            // is the species' own lowercased id by convention, or the one
            // `stage_models` in data/plants_visual.ron names for it. Dead
            // crops keep the procedural wilt.
            let model = if dead {
                None
            } else {
                let q = ((t * 4.0).ceil() as u32).clamp(1, 4);
                let name = format!("{}_{q}", visuals.stage_model_for(def_id));
                hero_plant_model(state, &mut cache, &name).map(|m| (name, m))
            };
            // One plant's vertices at this stage, for the plot's vertex
            // budget: the model's, or one procedural plant built to count.
            let per_plant = match &model {
                Some((_, (cpu, _))) => cpu.vertices.len(),
                None => {
                    let mut probe = PlantMeshBuilder::new();
                    crate::renderer::plant_mesh::build_plant(
                        &mut probe,
                        vis,
                        [0.0; 3],
                        [0.7, 0.0, 0.7],
                        t,
                        wilt,
                        seed,
                    );
                    probe.vertices.len()
                }
            };
            let draw_cap = plot_draw_cap(visuals.plot_visual_cap, visuals.plot_vertex_budget, per_plant);
            let layout = plot_plants(rect.size, holds, draw_cap, seed);
            // A clump is its plants' floor wide at one plant's height.
            let mut vis_clump = vis.clone();
            vis_clump.spread_m *= layout.widen;
            for (k, spot) in layout.spots.iter().enumerate() {
                let local = Vec3::new(rect.center[0] + spot[0], rect.floor, rect.center[1] + spot[1]);
                let at = g.pos + turn * local;
                let plant_seed = seed ^ (k as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                if let Some((name, (cpu, mat))) = &model {
                    // Deterministic per-plant turn and a few percent of
                    // height either way, so a stand is not a lattice of
                    // clones; a rebuild never changes either.
                    let yaw = (plant_seed % 3600) as f32 * (std::f32::consts::TAU / 3600.0);
                    let tall = 0.93 + ((plant_seed >> 24) % 15) as f32 * 0.01;
                    let (mesh, _) = hero
                        .entry(name.clone())
                        .or_insert_with(|| (PlantMeshBuilder::new(), *mat));
                    bake_copy(
                        &mut mesh.vertices,
                        &mut mesh.indices,
                        &cpu.vertices,
                        &cpu.indices,
                        at,
                        yaw,
                        layout.widen,
                        tall,
                    );
                } else {
                    crate::renderer::plant_mesh::build_plant(
                        &mut b,
                        &vis_clump,
                        at.to_array(),
                        [0.7, 0.0, 0.7],
                        t,
                        wilt,
                        plant_seed,
                    );
                }
                drawn += 1;
            }
            meant += u64::from(holds);
        }
        // Each finished mesh with its material: the shared procedural plant
        // material, or a stage model's textured one.
        let mut built: Vec<(PlantMeshBuilder, usize)> = Vec::new();
        if !b.vertices.is_empty() {
            built.push((b, plant_material));
        }
        built.extend(hero.into_values().filter(|(m, _)| !m.vertices.is_empty()));
        // Upload into mesh slots reused in place (renderer free path): this
        // machine's own first, then spare ones, adding only past both. A
        // machine's mesh count moves as its plots change stage model; a slot
        // let go waits in `spare` instead of leaking its buffers.
        let mut slots_now = Vec::with_capacity(built.len());
        let (mut verts, mut bytes) = (0usize, 0u64);
        for (mesh_b, material) in built {
            verts += mesh_b.vertices.len();
            bytes += (std::mem::size_of_val(mesh_b.vertices.as_slice())
                + std::mem::size_of_val(mesh_b.indices.as_slice())) as u64;
            let mesh = crate::renderer::mesh::Mesh::from_vertices(
                &state.renderer.device,
                &mesh_b.vertices,
                &mesh_b.indices,
            );
            let mi = match old_slots.pop().map(|(mi, _)| mi).or_else(|| cache.spare.pop()) {
                Some(mi) => {
                    state.renderer.replace_mesh(mi, mesh);
                    mi
                }
                None => state.renderer.add_mesh(mesh),
            };
            slots_now.push((mi, material));
        }
        for (mi, _) in old_slots {
            release_plant_slot(state, &mut cache, mi);
        }
        machines_rebuilt += 1;
        verts_built += verts;
        objs.extend_from_slice(&slots_now);
        groups.insert(cfg_id.clone(), PlantGroupMeshes { sig: group_sig, slots: slots_now, drawn, meant, verts, bytes });
    }
    // Machines with no crops left give their slots to the spare list.
    for (_, gone) in std::mem::take(&mut cache.groups) {
        for (mi, _) in gone.slots {
            release_plant_slot(state, &mut cache, mi);
        }
    }
    let drawn: usize = groups.values().map(|g| g.drawn).sum();
    let meant: u64 = groups.values().map(|g| g.meant).sum();
    let verts_held: usize = groups.values().map(|g| g.verts).sum();
    let bytes_held: u64 = groups.values().map(|g| g.bytes).sum();
    let machines = groups.len();
    let spare = cache.spare.len();
    cache.groups = groups;
    state.data_store.insert(PLANT_MESH_CACHE_KEY, cache);
    state.plant_objects = objs;
    note_plant_rebuild(PlantRebuild {
        ms: rebuild_t0.elapsed().as_secs_f64() * 1000.0,
        machines,
        machines_rebuilt,
        draws: state.plant_objects.len(),
        spare,
        drawn,
        meant,
        verts_built,
        verts_held,
        bytes_held,
    });
}

/// Park a plant mesh slot the rebuild no longer draws (2026-09-26): its
/// buffers are swapped for a one-triangle stand-in, so the geometry is freed
/// at once, and the slot waits in `spare` for the next mesh that needs one.
/// Without the swap a slot let go kept its last vertices until it was reused
/// (one rice tray plot at its last stage is about 4 MB).
fn release_plant_slot(state: &mut EngineState, cache: &mut PlantMeshCache, mi: usize) {
    let v = crate::renderer::mesh::Vertex { position: [0.0; 3], normal: [0.0, 1.0, 0.0], uv: [0.0; 2] };
    let stub = Mesh::from_vertices(&state.renderer.device, &[v; 3], &[0, 1, 2]);
    state.renderer.replace_mesh(mi, stub);
    cache.spare.push(mi);
}

/// DataStore key of the plant pass's [`PlantMeshCache`].
const PLANT_MESH_CACHE_KEY: &str = "plant_mesh_cache";

/// What the plant pass keeps between rebuilds (2026-09-26). It lives in the
/// DataStore rather than on EngineState only because the state's constructor
/// is in lib.rs; it is renderer bookkeeping, not game data.
#[derive(Default, Clone)]
struct PlantMeshCache {
    /// The type-20 material every procedural plant mesh draws with.
    material: Option<usize>,
    /// Mesh slots the last rebuild did not need, reused before adding more.
    spare: Vec<usize>,
    /// Stage models by name ("wheat_3"): the geometry baked per plant and
    /// the model's textured material.
    models: std::collections::HashMap<String, (std::sync::Arc<crate::assets::GltfCpuMesh>, usize)>,
    /// Each machine's meshes from the last rebuild, by crop group key.
    groups: std::collections::HashMap<String, PlantGroupMeshes>,
}

/// One machine's plant meshes and what they were built from.
#[derive(Clone)]
struct PlantGroupMeshes {
    /// Signature of everything that shaped them; equal means reuse as is.
    sig: u64,
    /// (mesh, material) per draw.
    slots: Vec<(usize, usize)>,
    /// Plants and clumps drawn, and the plants the crops really hold.
    drawn: usize,
    meant: u64,
    /// Vertices in these meshes, and their vertex and index bytes.
    verts: usize,
    bytes: u64,
}

/// Running totals behind the plant-rebuild line in run.log.
struct PlantRebuildLog {
    since: std::time::Instant,
    rebuilds: u32,
    sum_ms: f64,
    max_ms: f64,
    /// Whether a rebuild that built geometry has been logged yet.
    built_logged: bool,
}

/// What one plant rebuild did, for [`note_plant_rebuild`].
struct PlantRebuild {
    ms: f64,
    /// Planted machines (towers, beds, fields), and how many were rebuilt.
    machines: usize,
    machines_rebuilt: usize,
    /// Plant draw calls, the plants and clumps in them, and the plants the
    /// crops really hold (more than `drawn` where a plot is over the cap).
    draws: usize,
    /// Mesh slots parked for reuse (their geometry already freed).
    spare: usize,
    drawn: usize,
    meant: u64,
    /// Vertices generated this rebuild (the rebuilt machines only), and in
    /// all the plant meshes drawn after it.
    verts_built: usize,
    verts_held: usize,
    /// GPU bytes of the plant meshes drawn (vertex + index buffers).
    bytes_held: u64,
}

/// Summarise the garden plant rebuilds in run.log (2026-09-26): the first
/// rebuild and the first that builds any geometry (the full build after the
/// garden is sown, the expensive one) at once, then one line per ten seconds
/// at most. A fresh world's clock runs fast, so some crop changes stage every
/// few seconds and a line per rebuild would bury the log.
fn note_plant_rebuild(r: PlantRebuild) {
    static LOG: std::sync::Mutex<Option<PlantRebuildLog>> = std::sync::Mutex::new(None);
    let Ok(mut guard) = LOG.lock() else { return };
    let first = guard.is_none();
    let s = guard.get_or_insert_with(|| PlantRebuildLog {
        since: std::time::Instant::now(),
        rebuilds: 0,
        sum_ms: 0.0,
        max_ms: 0.0,
        built_logged: false,
    });
    s.rebuilds += 1;
    s.sum_ms += r.ms;
    s.max_ms = s.max_ms.max(r.ms);
    let secs = s.since.elapsed().as_secs_f64();
    if first || secs >= 10.0 || (!s.built_logged && r.verts_built > 0) {
        log::info!(
            "[Plants] {} mesh rebuild(s) in {:.1} s: last {:.2} ms ({} of {} machines, {} vertices), \
             mean {:.2} ms, max {:.2} ms; now {} draws ({} spare slots), {} plants drawn for {} grown, \
             {} vertices, {:.1} MB",
            s.rebuilds,
            secs,
            r.ms,
            r.machines_rebuilt,
            r.machines,
            r.verts_built,
            s.sum_ms / f64::from(s.rebuilds),
            s.max_ms,
            r.draws,
            r.spare,
            r.drawn,
            r.meant,
            r.verts_held,
            r.bytes_held as f64 / 1_048_576.0
        );
        *s = PlantRebuildLog {
            since: std::time::Instant::now(),
            rebuilds: 0,
            sum_ms: 0.0,
            max_ms: 0.0,
            built_logged: s.built_logged || r.verts_built > 0,
        };
    }
}

/// One hero crop stage model by name ("carrot_3"): its geometry on the CPU,
/// for baking a copy per plant into a merged mesh, and its textured type-19
/// material, both cached in the plant pass's [`PlantMeshCache`]. A model
/// that fails to load is remembered in `hero_plant_missing`, so the ~114
/// species without converted models cost one attempt per session, not one
/// per growth rebuild. (v0.992; CPU geometry since 2026-09-26.)
fn hero_plant_model(
    state: &mut EngineState,
    cache: &mut PlantMeshCache,
    model: &str,
) -> Option<(std::sync::Arc<crate::assets::GltfCpuMesh>, usize)> {
    if let Some(hit) = cache.models.get(model) {
        return Some(hit.clone());
    }
    if state.hero_plant_missing.contains(model) {
        return None;
    }
    let rel = format!("assets/models/plants/{model}/{model}.gltf");
    match state.asset_manager.parse_gltf_mesh_with_texture(&rel) {
        Ok((cpu, tex)) => {
            let material = match tex {
                Some((rgba, w, h)) => state.renderer.add_textured_material(
                    [1.0, 1.0, 1.0, 1.0],
                    0.0,
                    0.9,
                    19.0,
                    0.0,
                    &rgba,
                    w,
                    h,
                ),
                None => state.renderer.add_material_full([0.35, 0.5, 0.3, 1.0], 0.0, 0.9, 0.0, 0.0),
            };
            let hit = (std::sync::Arc::new(cpu), material);
            cache.models.insert(model.to_string(), hit.clone());
            Some(hit)
        }
        Err(_) => {
            // Not an error: most species simply have no converted model yet.
            state.hero_plant_missing.insert(model.to_string());
            None
        }
    }
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
    // v0.1005 (operator: "I was hearing doors open as my character moved
    // around the home despite my camera being in some far off distance"):
    // while the player is AWAY (surface-flying a planet, FTL), the local
    // camera coordinates are frozen at their last home-frame values - a
    // stale point usually INSIDE the house - so the "camera" kept counting
    // as a door actor and kept passing the earshot gate below. The camera
    // is only a door actor, and door sounds only reach the ears, while
    // actually aboard (the same gate the wall-collision system uses).
    let mut actors: Vec<Vec3> = if state.aboard_station {
        vec![state.camera.position]
    } else {
        Vec::new()
    };
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
    // Local SFX buffer: the panel loop holds &mut door_panels, so edge sounds
    // collect here and land on state.pending_sfx after the borrow ends.
    let mut sfx: Vec<(&'static str, &'static str)> = Vec::new();
    let state_cam = state.camera.position;
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
        let open_before = *open;
        *open = crate::systems::door_anim::ease_open(*open, target, dt);
        // Door SFX (v0.983): fire on the swing's START edges - rising off
        // fully-closed plays the open sound, dropping off fully-open plays
        // the close. Mid-swing reversals stay quiet (no re-trigger spam when
        // a player hovers at an auto-door's radius), and doors beyond
        // earshot stay silent (a REMOTE actor can open doors far from you;
        // play_sound is non-spatial, so gate by distance until the spatial
        // path is wired).
        let within_earshot = state.aboard_station
            && (p.center - state_cam).length_squared() < 25.0 * 25.0;
        if within_earshot {
            if open_before <= 0.02 && *open > 0.02 {
                sfx.push(("sfx.door_open", "audio/sfx/door_open.ogg"));
            } else if open_before >= 0.98 && *open < 0.98 {
                sfx.push(("sfx.door_close", "audio/sfx/door_close.ogg"));
            }
        }
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
    state.pending_sfx.extend(sfx);
}
