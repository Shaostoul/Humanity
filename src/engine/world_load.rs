use glam::Vec3;
use std::time::Instant;
use crate::ecs::components::{Controllable, Health, Name, Transform, Velocity};
use crate::engine::home_meshes::{apply_homestead_meshes, home_lights, machine_mesh, rebuild_connection_objects, rebuild_door_panels, rebuild_hull};
use crate::engine::home_spawn::{spawn_home_air_space, spawn_home_machine_entity};
use crate::engine::net_route::reload_planet_defs;
use crate::engine::registries::load_data_registries;
use crate::engine::state::{EngineState, GrowSpot};
use crate::renderer::mesh::Mesh;
use crate::ship::ship_structure::ShipStructure;
use crate::systems::inventory::ItemRegistry;
use crate::terrain::planet::PlanetDef;

/// Place a blockman avatar standing on a podium at `base` (the podium floor position),
/// built from the player's `Appearance` (v0.440). A rudimentary humanoid from boxes +
/// a head sphere on a podium cylinder, drawn via the static placeholder path (cleared +
/// re-added each load, so no duplication). The face/limbs use skin tone; later
/// increments swap this for a skinned mesh with cosmetic slots.
pub(crate) fn place_avatar(
    state: &mut EngineState,
    base: Vec3,
    app: &crate::ecs::components::Appearance,
    colors: &crate::cosmetics::OutfitColors,
) {
    let s = app.height_scale.clamp(0.5, 2.0);
    let skin = [app.skin_tone[0], app.skin_tone[1], app.skin_tone[2], 1.0];
    let rgba = |c: [f32; 3]| [c[0], c[1], c[2], 1.0];
    // Equipped cosmetics tint the matching slot; otherwise default body colors.
    let hair = colors.head.map(rgba).unwrap_or([app.hair_color[0], app.hair_color[1], app.hair_color[2], 1.0]);
    let shirt = colors.chest.map(rgba).unwrap_or([0.28, 0.38, 0.58, 1.0]);
    let pants = colors.legs.map(rgba).unwrap_or([0.22, 0.22, 0.28, 1.0]);
    let podium = [0.45, 0.47, 0.50, 1.0];
    // (w, h, d, color, x, y, z) box parts; y/positions scale with height.
    let podium_h = 0.15_f32;
    let leg_h = 0.85 * s;
    let torso_h = 0.62 * s;
    let head_r = 0.14 * s;
    let leg_base = podium_h;
    let torso_base = leg_base + leg_h;
    let head_cy = torso_base + torso_h + head_r;
    // Helper: push a box part at base + (x,y,z).
    let mut push_box = |st: &mut EngineState, w: f32, h: f32, d: f32, c: [f32; 4], x: f32, y: f32, z: f32| {
        let mi = st.renderer.add_mesh(Mesh::box_xyz(&st.renderer.device, w, h, d));
        let mat = st.renderer.add_material_typed(c, 0.1, 0.75, 0.0);
        st.placeholder_objects.push((mi, mat, base + Vec3::new(x, y, z)));
    };
    // Legs (pants), torso (shirt), arms (skin), at the body's standing pose.
    push_box(state, 0.16, leg_h, 0.22, pants, -0.12, leg_base, 0.0);
    push_box(state, 0.16, leg_h, 0.22, pants, 0.12, leg_base, 0.0);
    push_box(state, 0.46, torso_h, 0.26, shirt, 0.0, torso_base, 0.0);
    push_box(state, 0.13, torso_h, 0.16, skin, -0.30, torso_base, 0.0);
    push_box(state, 0.13, torso_h, 0.16, skin, 0.30, torso_base, 0.0);
    // Hair cap (a thin box sitting on the head).
    push_box(state, 0.30, 0.10 * s, 0.30, hair, 0.0, head_cy + head_r * 0.4, 0.0);
    // Podium cylinder (capped so the top is a visible disc, not an open tube).
    let pm = state.renderer.add_mesh(Mesh::cylinder_capped(&state.renderer.device, 0.5, podium_h, 24));
    let pmat = state.renderer.add_material_typed(podium, 0.3, 0.5, 0.0);
    state.placeholder_objects.push((pm, pmat, base));
    // Head sphere (center-origin; place its center directly).
    let hm = state.renderer.add_mesh(Mesh::sphere(&state.renderer.device, head_r, 12, 14));
    let hmat = state.renderer.add_material_typed(skin, 0.1, 0.7, 0.0);
    state.placeholder_objects.push((hm, hmat, base + Vec3::new(0.0, head_cy, 0.0)));
}

pub(crate) fn load_world(state: &mut EngineState) {
    log::info!("Loading 3D world...");
    let load_start = Instant::now();

    // ── Homestead meshes ── (v0.455: load the LAYOUT, keep it for the construction
    // editor, then generate + upload meshes through the shared path.)
    // v0.534/v0.754: prefer the SHIP model (many zones, each a fixed outer box + freely-drawn
    // interior walls) from ship_structure.ron -- with one-time ADOPTION of a legacy
    // home_structure.ron data dir (wrapped as zone "home"; see ShipStructure::load_or_adopt).
    // Fall back to the legacy AABB-room layout when neither file exists. All paths produce
    // HomesteadMeshes, so the render path is identical.
    let blueprints_dir = state.data_dir.join("blueprints");
    let ship_file_existed = blueprints_dir.join("ship_structure.ron").exists();
    let (homestead, room_info) =
        if let Some(ship) = ShipStructure::load_or_adopt(&blueprints_dir) {
            let meshes = ship.generate_meshes();
            let info = meshes.room_info.clone();
            // Start editing the HOME zone; restore its persisted build-mode spawn point (v0.582).
            let home_idx = ship.home_zone_index();
            state.gui_state.construction_zone = home_idx;
            if let Some(sp) = ship.zones[home_idx].body.spawn {
                state.gui_state.build_char_pos = Some(sp);
            }
            state.gui_state.ship_structure = Some(ship);
            (meshes, info)
        } else {
            // TELL the player when this is a fallback, not a fresh start (v0.791):
            // an unloadable ship_structure.ron is quarantined by load() and the
            // default loads -- silently, this read as "all my saves are gone".
            // (The file existing before load_or_adopt but not after = quarantined.)
            if ship_file_existed {
                state.gui_state.construction_save_note =
                    "ship_structure.ron failed to load; it was preserved as ship_structure.invalid-<time>.ron next to it and the default home loaded instead. See logs/run.log."
                        .to_string();
                log::error!(
                    "load_world: ship_structure.ron existed but did not load; the shipped default home is showing instead (the player's file was quarantined, not overwritten)"
                );
            }
            let layout = crate::ship::fibonacci::load_layout_or_fallback();
            let meshes = crate::ship::fibonacci::generate_from_layout(&layout);
            let info = meshes.room_info.clone();
            state.homestead_layout = Some(layout);
            (meshes, info)
        };
    // Wall collision segments so the player can't walk through walls from the first frame
    // (v0.556; per-zone origin offsets v0.754).
    state.wall_colliders = match &state.gui_state.ship_structure {
        Some(ship) => crate::ship::wall_collision::ship_wall_segments(ship),
        None => Vec::new(),
    };
    state.sight_colliders = match &state.gui_state.ship_structure {
        Some(ship) => crate::ship::wall_collision::ship_sight_segments(ship),
        None => Vec::new(),
    };
    apply_homestead_meshes(state, homestead);
    // The hull wrap (ship-superstructure increment D): the exterior shell around the zone
    // cluster, generated from data/blueprints/hull_profile.ron. Ships with the world load;
    // rebuild_homestead regrows it on every structure edit.
    rebuild_hull(state);

    // Room ceiling lights
    let auto_lights = room_info.iter().map(|r| {
        let light_pos = Vec3::new(r.center.x, r.center.y + r.dimensions.y * 0.5 - 0.1, r.center.z);
        let room_size = r.dimensions.x.max(r.dimensions.z);
        let intensity = (room_size * 0.5).clamp(2.0, 15.0);
        let radius = room_size * 1.5;
        crate::renderer::light::RoomLight::point(light_pos, [1.0, 0.95, 0.85], intensity, radius)
    }).collect();
    // v0.571: placed lights (across ALL zones, v0.754) override the auto synthesis (empty -> auto).
    state.room_lights = home_lights(state.gui_state.ship_structure.as_ref(), auto_lights, state.gui_state.gi_enabled);

    // Sealed-volume AABB (encompasses every room) for the survival environment
    // context — inside it the player is sealed/oxygenated, outside = vacuum.
    state.homestead_bounds = room_info.iter().fold(None, |acc, r| {
        let rmin = r.center - r.dimensions * 0.5;
        let rmax = r.center + r.dimensions * 0.5;
        Some(match acc {
            None => (rmin, rmax),
            Some((mn, mx)) => (mn.min(rmin), mx.max(rmax)),
        })
    });

    // Hologram + spawn rooms
    let hologram_room_center = room_info.iter()
        .find(|r| r.is_hologram_room)
        .map(|r| r.center);
    let spawn_room = room_info.iter()
        .find(|r| r.is_spawn_room);
    state.hologram_room_center = hologram_room_center.unwrap_or(Vec3::new(-0.5, 1.0, 2.5));

    // Camera spawn position
    if let Some(spawn) = spawn_room {
        state.camera.position = Vec3::new(spawn.center.x, 1.7, spawn.center.z + spawn.dimensions.z * 0.35);
        state.camera.pitch = -0.2;
        state.camera.yaw = std::f32::consts::PI;
    } else if let Some(holo_center) = hologram_room_center {
        state.camera.position = Vec3::new(holo_center.x, 1.7, holo_center.z + 1.5);
        state.camera.pitch = -0.2;
        state.camera.yaw = std::f32::consts::PI;
    }

    log::info!("Homestead: {} rooms, {} floors, walls: {}, {} lights",
        room_info.len(), state.homestead_floors.len(),
        state.homestead_walls.is_some(), state.room_lights.len());

    // Clear the per-frame object lists before (re)populating them this load. The old aeroponic
    // tower placeholders (a v0.383 pre-machine-system demo: tower_configs grey cylinders + helix
    // plant-marker spheres) were REMOVED in v0.529 -- the home.ron machine arrays (the
    // aeroponic_tower_* types) now render the real garden towers, which move + delete with the
    // room. The static markers did not respond, showing duplicate non-responsive towers with
    // spheres (operator feedback 2026-06-24).
    state.placeholder_objects.clear();
    state.machine_objects.clear();
    state.grow_positions.clear();

    // ── Machine layout (data-driven, v0.427) ──
    // Rudimentary primitives for the homestead machines + pipes/tubes for the
    // connections between them (data/machines/home.ron). Falls back silently if the
    // file is absent (distributed builds); the tower placeholders above still show.
    {
        let path = crate::machines::home_ron_path(&state.data_dir);
        if let Some(home) = crate::machines::MachineHome::load(&path) {
            use std::collections::HashMap;
            // room id -> (center, floor_y, ceiling_y).
            let rooms: HashMap<&str, (Vec3, f32, f32)> = room_info
                .iter()
                .map(|r| {
                    (
                        r.id.as_str(),
                        (
                            r.center,
                            r.center.y - r.dimensions.y * 0.5,
                            r.center.y + r.dimensions.y * 0.5,
                        ),
                    )
                })
                .collect();
            state.gui_state.machine_labels.clear();
            // Despawn any previously-spawned home machine entities so re-entering the
            // world never duplicates the live power entities (load_world can re-run).
            {
                let old: Vec<hecs::Entity> = state
                    .game_world
                    .world
                    .query::<&crate::ecs::components::HomeMachine>()
                    .iter()
                    .map(|(e, _)| e)
                    .collect();
                for e in old {
                    let _ = state.game_world.world.despawn(e);
                }
            }
            // Room volumes for label occlusion (which room is the camera in), now also
            // carrying each room's FUNCTION joined by id from data/rooms.ron (v0.439):
            // the walkable world finally knows what each room is for.
            let room_types =
                crate::ship::room_types::RoomTypeRegistry::load(&state.data_dir);
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
            let mut placed = 0usize;
            // v0.538: a box home positions machines by ABSOLUTE world coords (clamped into the
            // footprint) and skips NO machine on a stale room id -- mirrors
            // MachineHome::placements' zone branch; the two MUST stay in sync. Removing the
            // skip in box mode also restores each machine's live ECS power role below. The
            // legacy ship layout keeps room-center-relative + skip-if-missing. v0.754: the
            // clamp is per machine into ITS zone's footprint at that zone's origin.
            let zone_rects = state.gui_state.ship_structure.as_ref().map(|s| s.zone_rects());
            // Explicit instances + every `arrays` grid expanded (dense garden towers).
            let all_instances = home.all_instances();
            // Electrical + plumbing island per machine, so spawned entities flow on their circuit. (v0.607/v0.608)
            let power_islands = home.electrical_islands(&all_instances);
            let water_islands = home.water_islands(&all_instances);
            // Instance id -> world position, for anchoring the starter
            // livestock near the fields below. (v0.751, ladder rung 7)
            let mut machine_world_pos: std::collections::HashMap<String, Vec3> =
                std::collections::HashMap::new();
            for inst in &all_instances {
                let Some(def) = home.catalog.get(&inst.machine) else { continue };
                // Position formula mirrored by the tested MachineHome::placements (the editor's
                // live-refresh twin); keep the two in sync. (v0.525/v0.538/v0.754)
                let pos = if let Some(zones) = zone_rects.as_deref() {
                    let Some(zr) = crate::machines::resolve_zone_rect(zones, &inst.zone) else { continue };
                    let (ox, oy, oz) = zr.origin;
                    let (w, d, _h) = zr.size;
                    Vec3::new(
                        inst.offset.0.clamp(ox + 0.3, (ox + w - 0.3).max(ox + 0.3)),
                        oy + inst.offset.1,
                        inst.offset.2.clamp(oz + 0.3, (oz + d - 0.3).max(oz + 0.3)),
                    )
                } else {
                    let Some(&(center, floor_y, _ceiling_y)) = rooms.get(inst.room.as_str()) else { continue };
                    Vec3::new(
                        center.x + inst.offset.0,
                        floor_y + inst.offset.1,
                        center.z + inst.offset.2,
                    )
                };
                let (sx, sy, _sz) = def.size;
                // GLB model when declared (v0.734); primitive fallback on
                // any load error. Per-instance parse — see the editor
                // rebuild's note on why the mesh is never cache-shared.
                let mesh = def
                    .model
                    .as_deref()
                    .and_then(|m| {
                        state
                            .asset_manager
                            .parse_gltf_mesh(&state.renderer.device, m)
                            .map_err(|e| {
                                log::warn!(
                                    "machine {} model '{m}' failed: {e}; primitive fallback",
                                    inst.id
                                )
                            })
                            .ok()
                    })
                    .unwrap_or_else(|| machine_mesh(&state.renderer.device, &def.shape, def.size));
                let mesh_idx = state.renderer.add_mesh(mesh);
                let mat = state.renderer.add_material_typed(
                    [def.color.0, def.color.1, def.color.2, 1.0],
                    0.1,
                    0.7,
                    0.0,
                );
                // sphere is center-origin; lift it so it rests on the floor.
                let draw_pos = if def.shape == "sphere" {
                    Vec3::new(pos.x, pos.y + sx, pos.z)
                } else {
                    pos
                };
                machine_world_pos.insert(inst.id.clone(), pos);
                state.machine_objects.push((mesh_idx, mat, draw_pos, inst.rotation));
                // Floating label anchor: just above the machine's top.
                let top_y = if def.shape == "sphere" { pos.y + 2.0 * sx } else { pos.y + sy };
                // Grow anchor for the procedural plant pass (v0.863): the
                // INITIAL load path must record these too, not just
                // rebuild_machine_objects, or a fresh boot has no anchors
                // and the showcase auto-seed stays idle until an edit.
                state.grow_positions.push(GrowSpot {
                    ty: inst.machine.clone(),
                    id: inst.id.clone(),
                    pos,
                    yaw: inst.rotation,
                    top_y,
                    size: def.size,
                });
                let name = if def.label.is_empty() { inst.machine.clone() } else { def.label.clone() };
                state.gui_state.machine_labels.push(crate::gui::MachineLabel {
                    pos: Vec3::new(pos.x, top_y + 0.4, pos.z),
                    name,
                    stats: def.stats.clone(),
                    room: inst.room.clone(),
                    machine_id: inst.id.clone(),
                });
                // Spawn the machine's electrical + water roles as a LIVE ECS entity so the
                // SolarSystem + ElectricalSystem + PlumbingSystem tick against the real home
                // (v0.437/v0.608). One entity per machine (power + water on the same entity).
                spawn_home_machine_entity(
                    &mut state.game_world.world,
                    inst,
                    def,
                    &power_islands,
                    &water_islands,
                    Some(pos),
                    state
                        .data_store
                        .get::<crate::systems::inventory::containers::ContainerRegistry>(
                            "container_registry",
                        ),
                );
                placed += 1;
            }
            log::info!("Machines: placed {placed} machines");

            // ── Starter livestock (v0.751, ladder rung 7) ── farm animals
            // near the outdoor fields, from data/entities/livestock.ron rows
            // against creatures.csv species. Spawned READY to collect so a
            // fresh homestead demonstrates the loop immediately. Idempotent
            // across world reloads (previous herd despawns first).
            {
                let old: Vec<hecs::Entity> = state
                    .game_world
                    .world
                    .query::<&crate::ecs::components::Creature>()
                    .iter()
                    .map(|(e, _)| e)
                    .collect();
                for e in old {
                    let _ = state.game_world.world.despawn(e);
                }
                let mut bundles = Vec::new();
                if let (Some(reg), Some(list)) = (
                    state
                        .data_store
                        .get::<crate::systems::livestock::CreatureRegistry>("creature_registry"),
                    state
                        .data_store
                        .get::<crate::systems::livestock::LivestockSpawnList>("livestock_spawn_list"),
                ) {
                    let items = state.data_store.get::<ItemRegistry>("item_registry");
                    for (pi, p) in list.animals.iter().enumerate() {
                        let Some(def) = reg.get(&p.creature) else {
                            log::warn!("livestock.ron: unknown creature {}", p.creature);
                            continue;
                        };
                        let Some(product) = def.renewable() else {
                            log::warn!("livestock.ron: {} has no renewable product", p.creature);
                            continue;
                        };
                        let Some(&anchor) = machine_world_pos.get(&p.near) else {
                            log::warn!("livestock.ron: anchor {} not placed", p.near);
                            continue;
                        };
                        for i in 0..p.count {
                            // Golden-angle scatter: deterministic, spread out.
                            let a = i as f32 * 2.399_963 + pi as f32 * 1.7;
                            let r = p.spread * (0.45 + 0.55 * (i as f32 + 1.0) / p.count as f32);
                            let pos = anchor + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
                            bundles.push((
                                crate::ecs::components::Creature {
                                    def_id: def.id.clone(),
                                    anchor,
                                    range: p.spread,
                                    phase: pi as f32 * 0.9 + i as f32 * 1.7,
                                    speed: (def.movement_speed * 0.35).max(0.2),
                                    tint: [p.tint.0, p.tint.1, p.tint.2],
                                    body_side: def.body_side(),
                                },
                                Transform {
                                    position: pos,
                                    ..Default::default()
                                },
                                Name(def.name.clone()),
                                crate::ecs::components::Harvestable {
                                    resource: product.item.clone(),
                                    amount: product.amount as f32,
                                    regrow_time: product.regrow_s,
                                    time_since_harvest: product.regrow_s, // ready on arrival
                                },
                                // Combat arc (v0.760): animals are living
                                // entities - real health from the species
                                // + a resolved loot table for the kill.
                                Health {
                                    current: def.health_base.max(1.0),
                                    max: def.health_base.max(1.0),
                                },
                                crate::ecs::components::LootTable {
                                    entries: def.loot_entries(items),
                                },
                            ));
                        }
                    }
                }
                let herd = bundles.len();
                for b in bundles {
                    state.game_world.world.spawn(b);
                }
                if herd > 0 {
                    log::info!("Livestock: spawned {herd} animals near the fields");
                }
            }

            // ── Decoration plants (v0.909, operator: "get the plants
            // added and the environment decorated") ── photoscanned CC0
            // Poly Haven models scattered around homestead anchors from
            // data/entities/decorations.ron. Meshes/materials cache
            // across world reloads; positions rebuild each load.
            {
                #[derive(serde::Deserialize)]
                struct DecoEntry {
                    model: String,
                    near: String,
                    #[serde(default)]
                    offset: (f32, f32, f32),
                    count: u32,
                    spread: f32,
                    #[serde(default = "one_f32")]
                    scale: f32,
                }
                #[derive(serde::Deserialize)]
                struct DecoList {
                    decorations: Vec<DecoEntry>,
                }
                fn one_f32() -> f32 {
                    1.0
                }
                state.decoration_objects.clear();
                let path = state.asset_manager.data_dir().join("entities/decorations.ron");
                let parsed = std::fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|t| ron::from_str::<DecoList>(&t).map_err(|e| e.to_string()));
                match parsed {
                    Ok(list) => {
                        let mut placed = 0u32;
                        for (di, d) in list.decorations.iter().enumerate() {
                            let Some(&anchor) = machine_world_pos.get(&d.near) else {
                                log::warn!("decorations.ron: anchor {} not placed", d.near);
                                continue;
                            };
                            let cached = state.decoration_mesh_cache.get(&d.model).copied();
                            let (mesh_idx, mat_idx) = match cached {
                                Some(mm) => mm,
                                None => {
                                    // Variant file lives in its base model's
                                    // folder: grass_medium_02_v1 ->
                                    // plants/grass_medium_02/grass_medium_02_v1.gltf.
                                    let base = d
                                        .model
                                        .rfind("_v")
                                        .map(|i| &d.model[..i])
                                        .unwrap_or(d.model.as_str());
                                    let rel = format!(
                                        "assets/models/plants/{}/{}.gltf",
                                        base, d.model
                                    );
                                    match state
                                        .asset_manager
                                        .parse_gltf_mesh_textured(&state.renderer.device, &rel)
                                    {
                                        Ok((mesh, tex)) => {
                                            let mesh_idx = state.renderer.add_mesh(mesh);
                                            let mat_idx = match tex {
                                                Some((rgba, w, h)) => {
                                                    // Type 19: textured mesh
                                                    // (albedo texture + alpha
                                                    // cutout + standard sun-lit
                                                    // shading).
                                                    state.renderer.add_textured_material(
                                                        [1.0, 1.0, 1.0, 1.0],
                                                        0.0,
                                                        0.9,
                                                        19.0,
                                                        0.0,
                                                        &rgba,
                                                        w,
                                                        h,
                                                    )
                                                }
                                                None => state.renderer.add_material_full(
                                                    [0.35, 0.5, 0.3, 1.0],
                                                    0.0,
                                                    0.9,
                                                    0.0,
                                                    0.0,
                                                ),
                                            };
                                            state
                                                .decoration_mesh_cache
                                                .insert(d.model.clone(), (mesh_idx, mat_idx));
                                            (mesh_idx, mat_idx)
                                        }
                                        Err(e) => {
                                            log::warn!(
                                                "decorations.ron: {} failed to load: {e}",
                                                d.model
                                            );
                                            continue;
                                        }
                                    }
                                }
                            };
                            for i in 0..d.count {
                                // Golden-angle scatter with deterministic
                                // yaw/scale jitter: natural-looking, stable
                                // across reloads.
                                let a = i as f32 * 2.399_963 + di as f32 * 1.3;
                                let r = d.spread
                                    * (0.3 + 0.7 * ((i as f32 + 0.7) / d.count.max(1) as f32));
                                let pos = anchor
                                    + Vec3::new(
                                        d.offset.0 + a.cos() * r,
                                        d.offset.1,
                                        d.offset.2 + a.sin() * r,
                                    );
                                let yaw = (i as f32 * 73.13 + di as f32 * 31.7) % 360.0;
                                let scl = d.scale
                                    * (0.85
                                        + 0.3
                                            * (((i * 37 + di as u32 * 11) % 100) as f32
                                                / 100.0));
                                state
                                    .decoration_objects
                                    .push((mesh_idx, mat_idx, pos, yaw, scl));
                                placed += 1;
                            }
                        }
                        log::info!(
                            "Loaded {placed} decoration plants from entities/decorations.ron"
                        );
                    }
                    Err(e) => log::warn!("decorations.ron not loaded: {e}"),
                }
            }
        }
    }
    // ── Wild hostiles (v0.761, combat arc) ── absolute-position spawns
    // away from the homestead; the AISystem drives them (hunt-class rows
    // become predators that stalk prey INCLUDING the player). Runs even
    // without a home layout. Idempotent: previous wild creatures clear
    // first (the livestock pass only despawns when a home exists).
    {
        let old: Vec<hecs::Entity> = state
            .game_world
            .world
            .query::<(
                &crate::ecs::components::Creature,
                &crate::ecs::components::AIBehavior,
            )>()
            .iter()
            .map(|(e, _)| e)
            .collect();
        for e in old {
            let _ = state.game_world.world.despawn(e);
        }
        let mut bundles = Vec::new();
        if let (Some(reg), Some(list)) = (
            state
                .data_store
                .get::<crate::systems::livestock::CreatureRegistry>("creature_registry"),
            state
                .data_store
                .get::<crate::systems::livestock::WildSpawnList>("wild_spawn_list"),
        ) {
            let items = state.data_store.get::<ItemRegistry>("item_registry");
            for (si, s) in list.spawns.iter().enumerate() {
                let Some(def) = reg.get(&s.creature) else {
                    log::warn!("wild_spawns.ron: unknown creature {}", s.creature);
                    continue;
                };
                // Settings > Gameplay "Hostile wildlife" (v0.791): predator/
                // aggressive rows only spawn when enabled (default OFF pre-launch;
                // operator: "disable the wolves"). Dev-page spawns are unaffected.
                if !state.gui_state.settings.hostile_wildlife
                    && matches!(
                        crate::systems::livestock::behavior_type_for(def),
                        "predator" | "aggressive"
                    )
                {
                    continue;
                }
                let center = Vec3::new(s.pos.0, 0.0, s.pos.1);
                for i in 0..s.count {
                    let a = i as f32 * 2.399_963 + si as f32 * 1.7;
                    let r = s.radius * (0.4 + 0.6 * (i as f32 + 1.0) / s.count.max(1) as f32);
                    let pos = center + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
                    bundles.push((
                        crate::ecs::components::Creature {
                            def_id: def.id.clone(),
                            anchor: center,
                            range: s.radius,
                            phase: si as f32 * 0.9 + i as f32 * 1.7,
                            speed: (def.movement_speed * 0.35).max(0.2),
                            tint: [s.tint.0, s.tint.1, s.tint.2],
                            body_side: def.body_side(),
                        },
                        Transform {
                            position: pos,
                            ..Default::default()
                        },
                        Name(def.name.clone()),
                        Health {
                            current: def.health_base.max(1.0),
                            max: def.health_base.max(1.0),
                        },
                        crate::ecs::components::LootTable {
                            entries: def.loot_entries(items),
                        },
                        crate::ecs::components::AIBehavior {
                            behavior_type: crate::systems::livestock::behavior_type_for(def)
                                .to_string(),
                            state: "idle".to_string(),
                            target: None,
                        },
                        Velocity::default(),
                    ));
                }
            }
        }
        let packs = bundles.len();
        for b in bundles {
            state.game_world.world.spawn(b);
        }
        if packs > 0 {
            log::info!("Wild creatures: spawned {packs} in the wilds");
        }
    }

    // The home's sealed air space for the live AtmosphereSystem readout (v0.617).
    spawn_home_air_space(&mut state.game_world.world);
    // Build the live connection cylinders (replaces the old static routed pipes). (v0.530)
    rebuild_connection_objects(state);
    // Build the door/window panels from the home structure's openings. (v0.537)
    rebuild_door_panels(state);

    // ── Solar system hologram (map-sync increment C, v0.262.13) ──
    // Driven by the CANONICAL crate::cosmos model at the live date,
    // so the tabletop matches the Maps page + the FPS sky exactly.
    // Was the drifted solar_system.ron placed at fake golden angles
    // (operator: "isn't working — still showing an old
    // placeholder"). Sun is the room centre; bodies sit at their
    // REAL ecliptic longitude (orbit radii still log-compressed to
    // fit the room — true AU ratios can't show indoors).
    let sim_t_now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
        - 946_728_000.0; // Unix secs at the J2000.0 epoch
    let hologram =
        crate::renderer::hologram::generate_hologram_from_cosmos(sim_t_now);

    let orbit_mat = state.renderer.add_material([0.3, 0.7, 0.9, 0.8], 0.0, 0.3);
    let ring_disc_mat = state.renderer.add_material([0.8, 0.7, 0.5, 0.6], 0.0, 0.4);
    let mut orbit_radii_used: Vec<f32> = Vec::new();

    for body in &hologram.bodies {
        if body.radius <= 0.0 { continue; }

        let stacks = if body.radius > 0.05 { 16 } else { 8 };
        let slices = if body.radius > 0.05 { 24 } else { 12 };
        let mesh_idx = state.renderer.add_mesh(
            crate::renderer::hologram::sphere_mesh(&state.renderer.device, body.radius, stacks, slices)
        );
        let (metallic, roughness, emissive) = if body.body_type == crate::renderer::hologram::BodyType::Star {
            (0.0, 0.2, 5.0) // Stars glow bright
        } else {
            (0.3, 0.5, 0.0)
        };
        let mat_idx = state.renderer.add_material_full(body.color, metallic, roughness, 0.0, emissive);
        state.hologram_objects.push((mesh_idx, mat_idx, body.local_position, body.name.clone()));

        if body.orbit_radius > 0.01
            && body.parent.as_deref() == Some("Sun")
            && !orbit_radii_used.iter().any(|&r| (r - body.orbit_radius).abs() < 0.01)
        {
            let ring_mesh_idx = state.renderer.add_mesh(
                crate::renderer::hologram::orbit_ring_mesh(&state.renderer.device, body.orbit_radius, 128)
            );
            state.hologram_orbits.push((ring_mesh_idx, orbit_mat));
            orbit_radii_used.push(body.orbit_radius);
        }

        if body.has_rings && body.body_type == crate::renderer::hologram::BodyType::Planet {
            let inner_r = body.radius * 1.3;
            let outer_r = body.radius * 2.2;
            let disc_mesh = state.renderer.add_mesh(
                crate::renderer::hologram::ring_disc_mesh(&state.renderer.device, inner_r, outer_r, 32)
            );
            state.hologram_objects.push((disc_mesh, ring_disc_mat, body.local_position, format!("{} Rings", body.name)));
        }

        if body.body_type == crate::renderer::hologram::BodyType::Planet
            || body.body_type == crate::renderer::hologram::BodyType::DwarfPlanet
        {
            let pin_mesh_idx = state.renderer.add_mesh(
                crate::renderer::hologram::pin_marker_mesh(&state.renderer.device, 0.03, 0.12)
            );
            let pin_mat = state.renderer.add_material(body.color, 0.0, 0.5);
            let pin_offset = Vec3::new(0.0, body.radius + 0.13, 0.0);
            state.hologram_pins.push((pin_mesh_idx, pin_mat, body.local_position + pin_offset, body.name.clone()));
        }
    }

    // ── GROUND-TRUTH INSTRUMENTATION (v0.262.24) ──
    // Operator: "the in-home map never updated, whatever you do
    // doesn't affect it" across many builds, while the skybox DID
    // update. Logic says this block runs and is correct; reality
    // disagrees. So stop reasoning — make the next run conclusive.
    //
    // 1) Log exactly what generate_hologram_from_cosmos produced.
    // 2) Spawn an UNCONDITIONAL bright-MAGENTA proof beacon at the
    //    orrery centre. If it is ABSENT in the operator's run, THIS
    //    code is not executing in their binary (a build/launch path
    //    issue) — a totally different root cause. If it is PRESENT
    //    but bodies look old, the generator output is the problem.
    // 3) Then the green HOME beacon, with a tolerant Earth lookup
    //    and a RED fallback at centre if Earth is somehow missing,
    //    so a silent failure becomes a visible one.
    {
        let names: Vec<&str> =
            hologram.bodies.iter().map(|b| b.name.as_str()).take(40).collect();
        let earth_idx = hologram
            .bodies
            .iter()
            .position(|b| b.name.eq_ignore_ascii_case("earth"));
        log::info!(
            "ORRERY-DIAG: generate_hologram_from_cosmos -> {} bodies; earth_at={:?}; names={:?}",
            hologram.bodies.len(),
            earth_idx,
            names
        );

        // Magenta proof beacon removed in v0.262.26 — it confirmed
        // the orrery path executes + updates (operator saw it), so
        // the "in-home map never changes" was a misperception (the
        // rings are circles by nature; the cosmos model DOES drive
        // it). Keeping only the clean green HOME marker + the diag
        // log.

        // Green HOME beacon at Earth (tolerant lookup); RED
        // fallback at centre if Earth is missing from the model.
        let earth = hologram
            .bodies
            .iter()
            .find(|b| b.name.eq_ignore_ascii_case("earth"));
        let (anchor, blip_col, pin_col, label) = match earth {
            Some(e) => (
                e.local_position + Vec3::new(0.0, e.radius, 0.0),
                [0.15, 1.0, 0.45, 1.0],
                [0.15, 1.0, 0.45, 1.0],
                "HOME",
            ),
            None => {
                log::error!("ORRERY-DIAG: NO 'earth' body in hologram — RED fallback");
                (Vec3::new(0.0, 0.10, 0.0), [1.0, 0.1, 0.1, 1.0], [1.0, 0.1, 0.1, 1.0], "HOME?")
            }
        };
        let blip_mesh = state.renderer.add_mesh(
            crate::renderer::hologram::sphere_mesh(&state.renderer.device, 0.045, 10, 14),
        );
        let blip_mat =
            state.renderer.add_material_full(blip_col, 0.0, 0.3, 0.0, 8.0);
        state.hologram_objects.push((
            blip_mesh,
            blip_mat,
            anchor + Vec3::new(0.0, 0.10, 0.0),
            "Home (high Earth orbit)".to_string(),
        ));
        let home_pin_mesh = state.renderer.add_mesh(
            crate::renderer::hologram::pin_marker_mesh(&state.renderer.device, 0.07, 0.75),
        );
        let home_pin_mat =
            state.renderer.add_material_full(pin_col, 0.0, 0.4, 0.0, 6.0);
        state.hologram_pins.push((
            home_pin_mesh,
            home_pin_mat,
            anchor + Vec3::new(0.0, 0.95, 0.0),
            label.to_string(),
        ));
        log::info!("ORRERY-DIAG: HOME marker + magenta proof beacon pushed");
    }

    // ── Star skybox ──
    // The catalog is parsed ONCE here (stars.bin, ~1.8 MB binary; CSV
    // fallback) and shared by the skybox vertex builder and the
    // constellation resolver inside StarRenderer::new. Pre-v0.797 both
    // consumers re-read and re-parsed the 34 MB stars.csv separately.
    // Star-catalog tier CEILING (dev fast path): resolve from the saved
    // setting, with HUMANITY_STAR_TIER overriding it, so a scripted/verify
    // boot can force the shipped 120k catalog (cap 0) and skip the ~350 MB
    // Ultra read entirely. Default "auto" => cap 2 => biggest installed
    // wins (unchanged for players).
    let t_sky = Instant::now();
    if let Some(rx) = state.star_preload_rx.take() {
        // First entry: the boot-time background thread (see resumed())
        // built the sky already - or is about to finish; recv() blocks at
        // most as long as the old synchronous path took. Boot-time
        // settings apply, matching the "tier changes apply next world
        // entry" convention.
        state.star_renderer = rx.recv().ok().flatten();
    } else {
        // Later entries (character switch / tier change): synchronous
        // rebuild with the CURRENT settings, exactly as before.
        let star_tier_cap = crate::renderer::stars::StarCatalogTier::resolve_cap(
            &state.gui_state.settings.star_catalog_tier,
        );
        let star_catalog =
            crate::renderer::stars::StarCatalog::load(&state.data_dir, star_tier_cap);
        state.star_renderer = star_catalog.as_ref().and_then(|catalog| {
            crate::renderer::stars::StarRenderer::new(
                &state.renderer.device,
                &state.renderer.queue,
                state.renderer.surface_format(),
                catalog,
                &state.data_dir,
                // Ultra Milky Way glow tier (2026-07-11): built here with the
                // rest of the sky, so a tier change applies next world entry
                // (same convention as the star catalog tiers). Also gated by the
                // star-tier cap so the dev fast path (cap < 2) ALSO drops the
                // heavy Ultra glow texture, per the operator's "the mega skybox
                // thing AND the other versions".
                state.gui_state.settings.sky_glow_tier == "ultra" && star_tier_cap >= 2,
            )
        });
    }
    state.boot_timer.since("star_catalog_and_glow", t_sky);

    // ── Planets (procedural fractal surfaces, v0.763) ──
    // One shared vertex-color material (shader type 12) renders every
    // procedural planet surface; the per-face colors ride in the mesh
    // itself (packed in the UV channel), so no per-planet material is
    // needed for the ground. Meshes are generated lazily per (body,
    // LOD level) in the per-frame sky loop and cached in
    // state.planet_mesh_cache.
    state.planet_surface_material =
        state.renderer.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 12.0, 0.0);
    // Per-body surface parameters are data files (infinite-of-X): a new
    // planet look = a new data/planets/<body_id>.ron, no code change.
    // Bodies without a def fall back to smooth LOD spheres with the
    // coarse body-type materials.
    let t_planets = Instant::now();
    reload_planet_defs(state);
    state.boot_timer.since("planet_defs_bake", t_planets);

    // ── Ship position (GEO above Silverdale, WA) ──
    let lat_rad = 47.6_f64.to_radians();
    let lon_rad = (-122.3_f64).to_radians();
    let geo_radius = 42_164_000.0_f64;
    state.ship_world_pos = glam::DVec3::new(
        geo_radius * lat_rad.cos() * lon_rad.cos(),
        geo_radius * lat_rad.sin(),
        geo_radius * lat_rad.cos() * lon_rad.sin(),
    );

    // ── Sun setup ──
    // Sun world position: 1 AU from Earth, placed along the existing
    // shader sun_direction vector so the visible Sun disc matches where
    // the world is being lit from. sun_direction uniform is
    // [0.3, 1.0, 0.5] (see renderer/mod.rs:205) — the Sun sits along
    // that ray at 1 AU (149.6 million km).
    let sun_dir = glam::DVec3::new(0.3, 1.0, 0.5).normalize();
    const ONE_AU_M: f64 = 149_597_870_700.0;
    state.sun_world_pos = sun_dir * ONE_AU_M;
    // Emissive yellow-white core. params.w (emissive) cranked high so
    // tone mapping still leaves the Sun near-white on screen.
    // Core = the SAME radial-glow type as the corona (v0.887, operator:
    // "can you see hard seam of the sun's edge inside the corona?").
    // An emissive sphere has a hard geometric silhouette; the type-17
    // profile reaches zero exactly at the mesh edge, so a tight,
    // high-intensity glow saturates white-hot at the center and hands
    // over seamlessly to the wider corona shell - no edge anywhere.
    state.sun_material = state.renderer.add_material_full(
        [1.0, 0.96, 0.88, 1.0],
        0.0,
        1.0,
        17.0,
        30.0,
    );
    // Halo material — warmer orange, lower emissive. Rendered at a
    // larger scale in the scene to suggest a corona around the core.
    // A true bloom post-process would do this properly, but the
    // halo mesh is a cheap approximation that works without one.
    // Radial-glow corona (shader type 17, v0.886): center-bright halo
    // that melts into space around the emissive core - drawn on a 3x
    // sphere in the transparent celestial list at the sun draw site.
    state.sun_halo_material = state.renderer.add_material_full(
        [1.0, 0.82, 0.55, 0.85],
        0.0,
        1.0,
        17.0,
        2.2,
    );

    // ── Real solar-system body materials (map sync, increment B) ──
    // Four simple PBR materials picked by SolBody.body_type so Mars
    // doesn't look like Earth. Not photoreal — that's a later pass;
    // the point of B is that the FPS sky IS the Maps page (real
    // bodies, real positions, real scale) instead of one lone
    // sphere. Colors are coarse real-imagery approximations.
    state.solar_body_materials = [
        state.renderer.add_material([0.62, 0.52, 0.42, 1.0], 0.0, 0.85), // rocky/terrestrial — tan-grey
        state.renderer.add_material([0.80, 0.66, 0.46, 1.0], 0.0, 0.55), // gas giant — banded ochre (fallback)
        state.renderer.add_material([0.72, 0.82, 0.92, 1.0], 0.0, 0.40), // icy / dwarf — pale blue-white
        state.renderer.add_material([0.55, 0.55, 0.58, 1.0], 0.0, 0.80), // default — grey
        // v0.905: per-giant type-18 procedural band materials (params.w
        // selects the palette in the shader).
        state.renderer.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 18.0, 0.0), // jupiter
        state.renderer.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 18.0, 1.0), // saturn
        state.renderer.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 18.0, 2.0), // uranus
        state.renderer.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, 0.9, 18.0, 3.0), // neptune
    ];

    // ── Orbit paths (v0.262.20 — thin world-space lines) ──
    // Was thick tube meshes (operator: "tubes are just too thick …
    // we wouldn't need all the verts … like a single edge"). Now we
    // just cache each body's TRUE Keplerian ellipse points
    // (crate::cosmos::sample_orbit_points → same math the Maps page
    // draws) in PARENT-frame metres. Per frame they're offset to the
    // parent's Earth-relative position and drawn as a 1-px LineList
    // that the depth buffer occludes behind planets. 96 samples is
    // plenty smooth for an ellipse and a fraction of the tube verts.
    for b in crate::cosmos::sol_bodies() {
        // Cache EVERY orbiting body's ring, tagged planet/moon so the Sky
        // settings filter at draw time (v0.786). Direct sun-orbiters =
        // planets/dwarfs; everything else = a moon around its planet.
        // (Fixes the latent v0.262 bug where the lone intended moon ring
        // never drew: the guard checked id "moon" but the data id is
        // "luna".) Sun has no orbit (sample empty) → skipped naturally.
        let Some(parent) = b.parent.clone() else { continue };
        let kind = if parent == "sun" { "planet" } else { "moon" };
        let pts_au = crate::cosmos::sample_orbit_points(b, 96);
        if pts_au.len() < 3 { continue; }
        // Keep the vertices in f64 (v0.791): they are ~1.5e11 m and the
        // per-frame offset is the same magnitude with the opposite sign,
        // so an f32 cache left tens-of-km cancellation jitter near the
        // body -- one of the two reasons Earth sat visibly off its ring.
        let pts_m: Vec<glam::DVec3> = pts_au
            .iter()
            .map(|p| *p * crate::cosmos::M_PER_AU)
            .collect();
        state.solar_orbit_paths.push((pts_m, parent, kind.to_string(), b.id.clone()));
    }
    log::info!("Map-sync: cached {} FPS orbit paths (thin lines)", state.solar_orbit_paths.len());

    // ── Load CSV game-data registries into the runtime DataStore ──
    // Each registry is built from its data file and inserted under the key its
    // owning system reads (item_registry / recipe_registry / plant_registry).
    // Graceful per-registry: a missing or malformed file logs a warning and
    // skips (the system then runs on safe defaults), never panics. Reads from
    // the on-disk data dir so edits/mods to the CSV take effect. Mirrors the
    // container_registry wiring below.
    //
    // BEFORE v0.323 these three were loaded then DISCARDED (`let _ =
    // load_csv(...)` into throwaway {id,name} structs), so the runtime
    // DataStore stayed empty and CraftingSystem (no recipes), item
    // name/stack/mass lookups, and FarmingSystem species data all silently
    // no-op'd — the central finding of the 2026-05-29 game-code audit.
    // The registries are loaded EAGERLY at startup (load_data_registries, called
    // from resumed) — see that fn for why. This call re-loads them when the 3D
    // world opens (idempotent), so editing a data file + re-entering picks it up.
    load_data_registries(&mut state.data_store, state.asset_manager.data_dir());

    // ── Player avatar + character-select showroom (v0.440/441) ──
    // Place a blockman avatar on a podium in the respawner (where you wake) and OPEN the
    // showroom: hide the home, orbit the avatar against a backdrop, let the player edit
    // appearance, then "Enter your home" to emerge into first-person. The avatar is the
    // last thing added to placeholder_objects, so `avatar_obj_start` marks where it
    // begins (the showroom renders + rebuilds only this range).
    // Place the avatar + showroom assets at the "respawner" room (legacy
    // fibonacci layout) OR, when that room id does not exist, the spawn room
    // (v0.706 fix). The default HomeStructure home emits room ids "home" /
    // "room_N" with `is_spawn_room` set on the largest room, never
    // "respawner" — so this whole block used to be skipped on EVERY path,
    // leaving avatar_base at Vec3::ZERO. That made a fresh boot look empty
    // (no avatar body) and made the Play/Characters showroom orbit an empty
    // point. Falling back to the spawn room fixes both.
    if let Some(r) = room_info
        .iter()
        .find(|r| r.id == "respawner")
        .or_else(|| room_info.iter().find(|r| r.is_spawn_room))
    {
        let floor = r.center.y - r.dimensions.y * 0.5;
        let base = Vec3::new(r.center.x, floor, r.center.z - 0.35);
        let (cname, app, outfit) = state
            .game_world
            .world
            .query::<(
                &crate::ecs::components::Name,
                &crate::ecs::components::Appearance,
                &crate::ecs::components::Outfit,
                &Controllable,
            )>()
            .iter()
            .next()
            .map(|(_, (n, a, o, _))| (n.0.clone(), a.clone(), o.clone()))
            .unwrap_or_else(|| ("Wanderer".to_string(), Default::default(), Default::default()));
        state.gui_state.character_name = cname;
        state.cosmetics = crate::cosmetics::load_cosmetics(&state.data_dir);
        state.gui_state.cosmetics_list = state
            .cosmetics
            .iter()
            .map(|c| (c.id.clone(), c.name.clone(), c.slot.clone()))
            .collect();
        state.gui_state.appearance = app.clone();
        state.gui_state.outfit = outfit.clone();
        state.avatar_base = base;
        state.fps_spawn = state.camera.position; // the first-person spawn set above
        state.showroom_return_pos = state.camera.position;
        state.avatar_obj_start = state.placeholder_objects.len();
        let colors = crate::cosmetics::resolve_outfit_colors(&outfit, &state.cosmetics);
        place_avatar(state, base, &app, &colors);

        // Showroom SCENE ASSETS (backdrops, ground disc, body sphere) are loaded on
        // every world-load -- cheap, and needed so the wetroom mirror + bedroom
        // wardrobe can open the showroom later even when Play did not open the picker.
        state.showroom_backdrops = crate::showroom::load_backdrops(&state.data_dir);
        state.gui_state.showroom_backdrop_names =
            state.showroom_backdrops.iter().map(|b| b.name.clone()).collect();
        state.gui_state.showroom_backdrop = 0;
        state.showroom_last_backdrop = usize::MAX;
        let gmesh = state.renderer.add_mesh(Mesh::cylinder(&state.renderer.device, 9.0, 0.06, 32));
        let gmat = state.renderer.add_material_typed([0.1, 0.1, 0.12, 1.0], 0.1, 0.9, 0.0);
        state.showroom_ground = Some((gmesh, gmat));
        // A planet sphere (radius 30) the avatar stands on for body backdrops (Earth/Mars).
        let body = state.renderer.add_mesh(Mesh::sphere(&state.renderer.device, 30.0, 24, 32));
        state.showroom_body = Some(body);
        state.gui_state.appearance_dirty = false;
        state.gui_state.showroom_confirm = false;

        // load_world NO LONGER opens the character-select showroom (v0.476).
        // It just spawns you in first-person at the respawner. The unified
        // character picker is opened OPT-IN by the Play button, via the
        // per-frame open_showroom(0) call that runs right AFTER this load when
        // launcher_open_select is set. Because load_world only runs once (the
        // world_loaded guard) AND Esc enters the world without that flag, the
        // old picker (the "Wanderer" duplicate the operator hit on Esc) never
        // appears on Esc, and Play opens the picker every time, not just the
        // first. This is THE root-cause fix for the duplicate character-select.
        state.gui_state.showroom_active = false;
        state.controller.showroom_lock = false;
        state.camera.switch_mode(crate::renderer::camera::CameraMode::FirstPerson);
        state.camera.position = state.fps_spawn;
    }

    state.world_loaded = true;
    // Orbital home (v0.881): the spawn room is aboard the station by
    // definition - snap the player frame onto the orbit next frame.
    state.station_spawn_snap = true;
    log::info!("3D world loaded in {:.0}ms", load_start.elapsed().as_millis());
    // Boot-timing summary (dev tooling): fires exactly once (load_world is
    // only entered when !world_loaded). Records the whole world-load span,
    // then logs the per-phase breakdown + drops debug/boot_timing.json.
    state
        .boot_timer
        .record("world_load_total", load_start.elapsed());
    let boot_total = state.boot_timer.boot_start.elapsed();
    state.boot_timer.emit(boot_total);
}
