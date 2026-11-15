//! The home's simulation at world entry, built without the GPU (headless sim runner increment 1,
//! 2026-11-15, docs/design/headless-sim-runner.md section 5): the ship assembled with this
//! player's home on its plot, the save's pieces carried to it, the walls the walk collides with,
//! whose air each place breathes, where the player arrives, the household's machines as live ECS
//! entities, the herd, the wilds and the home's air space. `build` returns all of it with the
//! home's geometry as data; `world_load::load_world` uploads that geometry to the GPU and builds
//! the drawn parts (the machines' bodies, the screens, the decorations, the pipes, the doors), so
//! the windowed game and a test or the headless runner build one and the same simulation.
//!
//! Nothing here may name a GPU, window or egui type, nor `EngineState`, which holds the renderer
//! (tests/sim_gpu_free_lint.rs), and nothing here asks whether it runs headless.
use glam::Vec3;
use humanity_core::ecs::components::{Health, Name, Transform, Velocity};
use humanity_core::hot_reload::data_store::DataStore;
use humanity_ship::fibonacci::{HomesteadLayout, HomesteadMeshes, RoomInfo};
use humanity_ship::starts::HomeContent;
use humanity_systems::inventory::ItemRegistry;
use crate::engine::home_spawn::{spawn_home_air_space, spawn_home_machine_entity};
use crate::gui::GuiState;

/// What `build` works on: the simulation's own parts, nothing that needs a window or a GPU.
pub(crate) struct Parts<'a> {
    /// The game data folder (`data/`).
    pub data_dir: &'a std::path::Path,
    pub world: &'a mut hecs::World,
    pub data_store: &'a mut DataStore,
    /// The settings, the ship and the household's layout live here (CPU only).
    pub gui: &'a mut GuiState,
}

/// Where the player stands on entering the world, and which way they face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Spawn {
    pub position: Vec3,
    pub pitch: f32,
    pub yaw: f32,
}

/// The household's machines this load: where each placed machine's live entity stands, by
/// instance id (the herd and the decorations are anchored to them).
pub(crate) struct MachinesPlaced {
    pub world_pos: std::collections::HashMap<String, Vec3>,
}

/// The home's simulation, as `build` leaves it. The fields that live on `EngineState` are put
/// there by `world_load::load_world`.
pub(crate) struct WorldSim {
    /// The home's geometry as data, for the GPU upload (`home_meshes::apply_homestead_meshes`).
    pub homestead: HomesteadMeshes,
    /// Every room: its box, what it is, whether the player arrives in it.
    pub room_info: Vec<RoomInfo>,
    /// The plot the home stands on (None: the legacy layout).
    pub boot_plot: Option<String>,
    /// The legacy layout, when the ship did not assemble (None: the ship did).
    pub fallback_layout: Option<HomesteadLayout>,
    /// The walls the walk collides with, and the ones sight stops at.
    pub wall_colliders: Vec<humanity_ship::wall_collision::WallSegment>,
    pub sight_colliders: Vec<humanity_ship::wall_collision::WallSegment>,
    /// The box around every room (the hull top reads it).
    pub homestead_bounds: Option<(Vec3, Vec3)>,
    /// Whose air each place breathes, and where aboard ends (`survival_env::ship_spaces`).
    pub ship_air: humanity_ship::ship_space::AirSpaces,
    pub aboard_bounds: Option<humanity_ship::ship_space::Aabb>,
    /// The hologram room's centre, when the home has one.
    pub hologram_room_center: Option<Vec3>,
    /// Where the player arrives (None: no room says, and the camera stays where it was).
    pub spawn: Option<Spawn>,
    /// The household's machines (None: no machine layout this load).
    pub machines: Option<MachinesPlaced>,
    /// What the live save's start carries of the homestead's world (data/homes/starts.ron).
    pub content: Vec<HomeContent>,
}

/// Build the home's simulation for a world entry. Moved from `world_load::load_world`
/// (2026-11-15) in the order it ran there; the GPU work that ran between its steps now runs after
/// all of them, and reads none of what they write (the audit is in the commit that moved it).
pub(crate) fn build(parts: Parts<'_>) -> WorldSim {
    let Parts { data_dir, world, data_store, gui } = parts;
    // ── The home ── (v0.455: load the LAYOUT, keep it for the construction editor; the meshes
    // are generated here as data and uploaded by the world load.)
    // v0.534/v0.754: prefer the SHIP model (many zones, each a fixed outer box + freely-drawn
    // interior walls). Since increment 1a (docs/design/ship-homes-and-logistics.md) that is
    // ASSEMBLED: the ship file (blueprints/ship_structure.ron), plus this player's home design
    // (homes/<kind>.ron) as zone "home" at their plot's origin, plus the plot's door corridor.
    // The plot is the one this identity remembers for the configured server, in offline play
    // too (so stepping into the shared world later only confirms it), else the ship's default
    // plot (p1); see `assemble_for_boot`. Fall back to the legacy AABB-room layout when the ship
    // cannot be assembled. All paths produce HomesteadMeshes, so the render path is identical.
    // The home zone's authored spawn point in ship metres, if it declares one.
    // Used below to decide where the player stands on world entry.
    let mut authored_spawn: Option<Vec3> = None;
    let mut fallback_layout = None;
    // The home goes on the plot this player remembers for the server they are about to join
    // (increment 2, engine/home_plot.rs `assemble_for_boot`), else on the ship's default plot.
    let assembled = crate::engine::home_plot::assemble_for_boot(data_dir, gui);
    let boot_plot = assembled.as_ref().ok().and_then(|s| s.home_plot()).map(|p| p.id.clone());
    let (homestead, room_info) = match assembled {
        Ok(ship) => {
            let meshes = ship.generate_meshes();
            let info = meshes.room_info.clone();
            // Start editing the HOME zone, with no build spot yet: the build editor's first open
            // puts the build-mode avatar where the player stands, on their storey
            // (engine/editor_storeys.rs `seed_build_avatar`, ship homes increment 7). It used to be
            // put at the home's saved spawn here (v0.582), which the first close then stood the
            // player at, on the storey they had left from: upstairs, on a wall over a glass roof.
            // The spawn is still what the editor's Save writes when the avatar is moved.
            let home_idx = ship.home_zone_index();
            gui.construction_zone = home_idx;
            gui.build_char_pos = None;
            gui.construction_storeys.avatar_seed = None;
            authored_spawn = ship.home_spawn_world();
            gui.ship_structure = Some(ship);
            (meshes, info)
        }
        Err(why) => {
            // TELL the player when this is a fallback, not a fresh start (v0.791):
            // a file that cannot be used is quarantined (renamed, never overwritten)
            // and the default loads -- silently, this read as "all my saves are gone".
            gui.construction_save_note = format!(
                "The ship did not load ({why}); the default home is showing instead. Your files were kept. See logs/run.log."
            );
            log::error!("load_world: the ship did not assemble ({why}); the legacy default home is showing instead");
            let layout = humanity_ship::fibonacci::load_layout_or_fallback();
            let meshes = humanity_ship::fibonacci::generate_from_layout(&layout);
            let info = meshes.room_info.clone();
            fallback_layout = Some(layout);
            (meshes, info)
        }
    };
    // Where the home stands, for the save's frame (engine/home_plot.rs `publish_home_frame`):
    // on the default plot here, until a relay's welcome moves it. The save applied at startup
    // (engine/sim_boot.rs, before there was a ship) is carried from the plot its home stood on to
    // this one, its pieces and vehicles with it (`carry_loaded_save_home`, ship homes 1b).
    crate::engine::home_plot::publish_home_frame_on(gui.ship_structure.as_ref(), data_store);
    crate::engine::home_plot::carry_loaded_save_home(gui.ship_structure.as_ref(), world, data_store);
    // Wall collision segments so the player can't walk through walls from the first frame
    // (v0.556; per-zone origin offsets v0.754).
    let wall_colliders = match &gui.ship_structure {
        Some(ship) => humanity_ship::wall_collision::ship_wall_segments(ship),
        None => Vec::new(),
    };
    let sight_colliders = match &gui.ship_structure {
        Some(ship) => humanity_ship::wall_collision::ship_sight_segments(ship),
        None => Vec::new(),
    };

    // The AABB around every room (the hull top reads it). Whose air each place breathes, and
    // where aboard ends, come from the ship (ship homes increment 4, survival_env.rs).
    let homestead_bounds: Option<(Vec3, Vec3)> = room_info.iter().fold(None, |acc, r| {
        let rmin = r.center - r.dimensions * 0.5;
        let rmax = r.center + r.dimensions * 0.5;
        Some(match acc {
            None => (rmin, rmax),
            Some((mn, mx)) => (mn.min(rmin), mx.max(rmax)),
        })
    });
    let (ship_air, aboard_bounds) = crate::engine::survival_env::ship_spaces(gui.ship_structure.as_ref());

    // Hologram + spawn rooms
    let hologram_room_center = room_info.iter()
        .find(|r| r.is_hologram_room)
        .map(|r| r.center);
    let spawn_room = room_info.iter()
        .find(|r| r.is_spawn_room);

    // Where the player arrives. The AUTHORED point wins (2026-09-19): the home
    // body carries a `spawn: Some((x, z))` that the build-mode avatar gizmo
    // sets, and it is the home's own statement of where a person arrives. The
    // fallback below picks the LARGEST room, which was fine while the acre was
    // one open hall with a house in the corner, and became wrong the moment it
    // was partitioned: the biggest room is now the greenhouse, so the player
    // would wake up among the beds instead of at their own front door. Facing
    // west (yaw -PI/2) because the door is in the east wall and the house is
    // inland of it.
    let spawn = if let Some(at) = authored_spawn {
        Some(Spawn { position: at, pitch: -0.05, yaw: -std::f32::consts::FRAC_PI_2 })
    } else if let Some(spawn) = spawn_room {
        Some(Spawn {
            position: Vec3::new(spawn.center.x, 1.7, spawn.center.z + spawn.dimensions.z * 0.35),
            pitch: -0.2,
            yaw: std::f32::consts::PI,
        })
    } else {
        hologram_room_center.map(|holo_center| Spawn {
            position: Vec3::new(holo_center.x, 1.7, holo_center.z + 1.5),
            pitch: -0.2,
            yaw: std::f32::consts::PI,
        })
    };

    // ── Machine layout (data-driven, v0.427) ──
    // The homestead machines as live entities (data/machines/home.ron); their bodies and the
    // pipes between them are drawn by the world load from what is placed here. Falls back
    // silently if the file is absent (distributed builds).
    // The household's food energy is kept for its air space below (ship life support).
    let mut home_kcal = 0.0f32;
    // What the live save's start carries of the homestead's world (its herd, the wilds, the
    // decoration plants, the hologram) and whether its home breathes its own air (ship homes
    // increment 6, data/homes/starts.ron): a cabin carries none of it and is on the ship's air.
    let start_row = crate::engine::save_slot::live_start(data_dir);
    let content = start_row.home_content.clone();
    let machines = {
        let path = humanity_systems::persistence::live_save::active_home_machines_path(data_dir);
        // The data file's layout, with the character's own machines in the household's place when
        // their save holds a home of their own outside the Dev mode (engine/own_home.rs,
        // 2026-10-04). The editor edits this same layout from here on, so what it shows, what is
        // spawned and what is saved are one layout (the editor's used to be the one loaded at
        // startup, before the save and any Settings change of household).
        if let Some(home) = crate::engine::own_home::machines_for_world(gui, &path) {
            gui.home_machines = Some(home.clone());
            // The Home page's loops and the garden follow the household loaded (it can have
            // changed since startup: Settings' "Where you live", a change of save; increment 6).
            crate::engine::save_slot::refresh_household_views(gui, data_dir);
            home_kcal = crate::engine::home_spawn::home_metabolic_kcal(&home);
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
            // What the old machines hold (each bank's charge, tank's litres and
            // vessel's contents), plus a save's contents held since startup, to put
            // back on the new ones below (2026-09-27, systems::machine_levels):
            // respawning used to reset every bank and tank to half and empty
            // every vessel, at each world entry.
            let carried_levels = humanity_systems::machine_levels::take_all(world);
            // Despawn any previously-spawned home machine entities so re-entering the
            // world never duplicates the live power entities (load_world can re-run).
            {
                let old: Vec<hecs::Entity> = world
                    .query::<&humanity_core::ecs::components::HomeMachine>()
                    .iter()
                    .map(|(e, _)| e)
                    .collect();
                for e in old {
                    let _ = world.despawn(e);
                }
            }
            // Room volumes for label occlusion (which room is the camera in), now also
            // carrying each room's FUNCTION joined by id from data/rooms.ron (v0.439):
            // the walkable world finally knows what each room is for.
            let room_types =
                humanity_ship::room_types::RoomTypeRegistry::load(data_dir);
            // Zone-named rooms resolve through their zone's room_type; legacy ids join on
            // themselves (RoomTypeRegistry::room_bounds, the one place a room becomes a RoomBounds).
            gui.room_bounds = room_types.room_bounds(&room_info);
            let mut placed = 0usize;
            // v0.538: a box home positions machines by coords in its zone (zone-local since
            // increment 1a, kept inside the footprint) and skips NO machine on a stale room id -- mirrors
            // MachineHome::placements' zone branch; the two MUST stay in sync. Removing the
            // skip in box mode also restores each machine's live ECS power role below. The
            // legacy ship layout keeps room-center-relative + skip-if-missing. v0.754: the
            // clamp is per machine into ITS zone's footprint at that zone's origin.
            let zone_rects = gui.ship_structure.as_ref().map(|s| s.zone_rects());
            // Explicit instances + every `arrays` grid expanded (dense garden towers).
            let all_instances = home.all_instances();
            // Electrical + plumbing island per machine, so spawned entities flow on their circuit. (v0.607/v0.608)
            let power_islands = home.electrical_islands(&all_instances);
            let water_islands = home.water_islands(&all_instances);
            // Instance id -> world position, for anchoring the starter
            // livestock near the fields below. (v0.751, ladder rung 7)
            let mut machine_world_pos: std::collections::HashMap<String, Vec3> =
                std::collections::HashMap::new();
            // Every machine that is placed (its catalog entry known, its zone or room found), with
            // where it stands, first: their count is how many mesh slots the bodies take.
            let resolved: Vec<(&humanity_ship::machines::MachineInstance, &humanity_ship::machines::MachineDef, Vec3)> = all_instances
                .iter()
                .filter_map(|inst| {
                    let def = home.catalog.get(&inst.machine)?;
                    // Position formula mirrored by the tested MachineHome::placements (the editor's
                    // live-refresh twin); keep the two in sync. (v0.525/v0.538/v0.754)
                    let pos = if let Some(zones) = zone_rects.as_deref() {
                        // Zone-local offset at the zone's origin (increment 1a): the same
                        // `zone_world_pos` placements() uses, so the two cannot drift.
                        let zr = humanity_ship::machines::resolve_zone_rect(zones, &inst.zone)?;
                        let size = humanity_ship::machines::drawn_size(def, inst);
                        let half = humanity_ship::machines::plan_half_extents(&def.shape, size, inst.rotation);
                        let (x, y, z) = humanity_ship::machines::zone_world_pos(zr, inst.offset, half);
                        Vec3::new(x, y, z)
                    } else {
                        let &(center, floor_y, _ceiling_y) = rooms.get(inst.room.as_str())?;
                        Vec3::new(center.x + inst.offset.0, floor_y + inst.offset.1, center.z + inst.offset.2)
                    };
                    Some((inst, def, pos))
                })
                .collect();
            // The bodies, their labels, their pick volumes and the grow anchors are built by the
            // world load from the editor's own builder (home_meshes::build_machine_bodies, stutter
            // survey 2, 2026-10-31): this loop places each machine's live entity and records where
            // it stands.
            for &(inst, def, pos) in resolved.iter() {
                machine_world_pos.insert(inst.id.clone(), pos);
                // Spawn the machine's electrical + water roles as a LIVE ECS entity so the
                // SolarSystem + ElectricalSystem + PlumbingSystem tick against the real home
                // (v0.437/v0.608). One entity per machine (power + water on the same entity).
                spawn_home_machine_entity(
                    world,
                    inst,
                    def,
                    &power_islands,
                    &water_islands,
                    Some(pos),
                    data_store
                        .get::<humanity_systems::inventory::containers::ContainerRegistry>(
                            "container_registry",
                        ),
                );
                placed += 1;
            }
            crate::engine::home_spawn::spawn_home_feed_taps(world, &home, &all_instances, &power_islands);
            log::info!("Machines: placed {placed} machines");
            // Their levels back, by instance id. What is left belongs to a machine
            // no longer in the layout, which takes its contents with it, as
            // removing one in the editor always has.
            let orphaned = humanity_systems::machine_levels::apply(world, &carried_levels);
            if !orphaned.is_empty() {
                log::warn!("Machines: {} saved machine levels had no machine to go to: {:?}", orphaned.len(), orphaned);
            }

            // ── Starter livestock (v0.751, ladder rung 7) ── farm animals near the outdoor
            // fields, only for a home whose start carries a herd (`spawn_herd`).
            spawn_herd(
                world,
                data_store,
                &machine_world_pos,
                content.contains(&HomeContent::Herd),
            );
            Some(MachinesPlaced { world_pos: machine_world_pos })
        } else {
            None
        }
    };
    // ── Wild hostiles (v0.761, combat arc) ── only for a home whose start carries the wilds
    // (`spawn_wilds`).
    spawn_wilds(
        world,
        data_store,
        crate::gui::gameplay_levels::value_of(&gui.settings.gameplay, "hostile_wildlife"),
        content.contains(&HomeContent::WildSpawns),
    );

    // The home's sealed air space for the live AtmosphereSystem readout (v0.617), as big as this
    // home (ship homes increment 4).
    // A home on the ship's ventilation (a cabin, `own_air: false`) has none of its own.
    if start_row.own_air {
        let home_m3 = crate::engine::home_spawn::own_home_air_m3(data_dir, gui.ship_structure.as_ref());
        spawn_home_air_space(world, home_kcal, home_m3);
    }

    WorldSim {
        homestead,
        room_info,
        boot_plot,
        fallback_layout,
        wall_colliders,
        sight_colliders,
        homestead_bounds,
        ship_air,
        aboard_bounds,
        hologram_room_center,
        spawn,
        machines,
        content,
    }
}

/// ── Starter livestock (v0.751, ladder rung 7) ── farm animals
/// near the outdoor fields, from data/entities/livestock.ron rows
/// against creatures.csv species. Spawned READY to collect so a
/// fresh homestead demonstrates the loop immediately. Idempotent
/// across world reloads (previous herd despawns first).
/// Each animal carries its HerdSlot, and a saved yield timer comes
/// back onto it (2026-09-27): the save's, held since startup, else
/// the previous herd's, else ready (livestock::HerdSlot).
///
/// Only for a home whose start carries a herd (`carries`: data/homes/starts.ron `home_content`,
/// ship homes increment 6): a cabin has no fields to keep animals on. Either way the previous
/// herd goes first, so a home that stops carrying one (a swap to a cabin save) has none. Pure on
/// the world and the data store; returns how many animals it spawned.
pub(crate) fn spawn_herd(
    world: &mut hecs::World,
    data: &humanity_core::hot_reload::data_store::DataStore,
    machine_world_pos: &std::collections::HashMap<String, Vec3>,
    carries: bool,
) -> usize {
    let saved_timers = humanity_systems::livestock::take_pending_herd(data)
        .unwrap_or_else(|| humanity_systems::livestock::herd_timers(world));
    // How each is kept (fed, watered, how long short) goes to the
    // new herd the same way: the save's, else the previous herd's
    // (systems::animal_care, 2026-10-12).
    humanity_systems::animal_care::hold_current(world, data);
    let old: Vec<hecs::Entity> = world.query::<&humanity_core::ecs::components::Creature>()
        .iter()
        .map(|(e, _)| e)
        .collect();
    for e in old {
        let _ = world.despawn(e);
    }
    if !carries {
        return 0;
    }
    let mut bundles = Vec::new();
    if let (Some(reg), Some(list)) = (
        data.get::<humanity_systems::livestock::CreatureRegistry>("creature_registry"),
        data.get::<humanity_systems::livestock::LivestockSpawnList>("livestock_spawn_list"),
    ) {
        let items = data.get::<ItemRegistry>("item_registry");
        let mut per_species: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
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
                let n = per_species.entry(def.id.as_str()).or_insert(0);
                let slot = humanity_systems::livestock::herd_slot(&def.id, *n);
                *n += 1;
                // An animal that died stays dead (2026-10-12).
                if humanity_systems::animal_care::is_lost(data, &slot) {
                    continue;
                }
                let since = saved_timers
                    .iter()
                    .find(|(s, _)| *s == slot)
                    .map_or(product.regrow_s, |(_, t)| t.clamp(0.0, product.regrow_s));
                bundles.push((
                    humanity_core::ecs::components::Creature {
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
                    humanity_core::ecs::components::Harvestable {
                        resource: product.item.clone(),
                        amount: product.amount as f32,
                        regrow_time: product.regrow_s,
                        time_since_harvest: since, // ready on arrival unless saved
                    },
                    humanity_systems::livestock::HerdSlot(slot),
                    // Combat arc (v0.760): animals are living
                    // entities - real health from the species
                    // + a resolved loot table for the kill.
                    Health {
                        current: def.health_base.max(1.0),
                        max: def.health_base.max(1.0),
                    },
                    humanity_core::ecs::components::LootTable {
                        entries: def.loot_entries(items),
                    },
                ));
            }
        }
    }
    let herd = bundles.len();
    for b in bundles {
        world.spawn(b);
    }
    if herd > 0 {
        log::info!("Livestock: spawned {herd} animals near the fields");
    }
    herd
}

/// ── Wild hostiles (v0.761, combat arc) ── absolute-position spawns
/// away from the homestead; the AISystem drives them (hunt-class rows
/// become predators that stalk prey INCLUDING the player). Runs even
/// without a home layout. Idempotent: previous wild creatures clear
/// first (the livestock pass only despawns when a home exists).
///
/// Only for a home whose start carries the wilds (`carries`, ship homes increment 6): their
/// spawn points are fixed homestead-frame points. Either way the previous wild creatures go
/// first. Pure on the world and the data store; returns how many it spawned.
pub(crate) fn spawn_wilds(
    world: &mut hecs::World,
    data: &humanity_core::hot_reload::data_store::DataStore,
    hostile_share: f32,
    carries: bool,
) -> usize {
    let old: Vec<hecs::Entity> = world.query::<(
            &humanity_core::ecs::components::Creature,
            &humanity_core::ecs::components::AIBehavior,
        )>()
        .iter()
        .map(|(e, _)| e)
        .collect();
    for e in old {
        let _ = world.despawn(e);
    }
    if !carries {
        return 0;
    }
    let mut bundles = Vec::new();
    if let (Some(reg), Some(list)) = (
        data.get::<humanity_systems::livestock::CreatureRegistry>("creature_registry"),
        data.get::<humanity_systems::livestock::WildSpawnList>("wild_spawn_list"),
    ) {
        let items = data.get::<ItemRegistry>("item_registry");
        for (si, s) in list.spawns.iter().enumerate() {
            let Some(def) = reg.get(&s.creature) else {
                log::warn!("wild_spawns.ron: unknown creature {}", s.creature);
                continue;
            };
            // Settings > Gameplay "Hostile wildlife" (v0.791): predator/
            // aggressive rows only spawn when enabled (default OFF pre-launch;
            // operator: "disable the wolves"). Dev-page spawns are unaffected.
            // Since 2026-11-11 a level, not a switch (data/gameplay/levels.json): its `value` is the
            // share of each hostile pack that spawns, at least one of a pack once it is above 0
            // (Few: one wolf of a pack of three).
            let hostile = matches!(humanity_systems::livestock::behavior_type_for(def), "predator" | "aggressive");
            if hostile && hostile_share <= 0.0 {
                continue;
            }
            let count = if hostile { ((s.count as f32 * hostile_share).ceil() as u32).max(1) } else { s.count };
            let center = Vec3::new(s.pos.0, 0.0, s.pos.1);
            for i in 0..count {
                let a = i as f32 * 2.399_963 + si as f32 * 1.7;
                let r = s.radius * (0.4 + 0.6 * (i as f32 + 1.0) / s.count.max(1) as f32);
                let pos = center + Vec3::new(a.cos() * r, 0.0, a.sin() * r);
                bundles.push(((
                    humanity_core::ecs::components::Creature {
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
                    humanity_core::ecs::components::LootTable {
                        entries: def.loot_entries(items),
                    },
                    humanity_core::ecs::components::AIBehavior {
                        behavior_type: humanity_systems::livestock::behavior_type_for(def)
                            .to_string(),
                        state: "idle".to_string(),
                        target: None,
                    },
                    Velocity::default(),
                ), def.renewable()));
            }
        }
    }
    let packs = bundles.len();
    for (b, renewable) in bundles {
        let e = world.spawn(b);
        // Renewable-yield wild species (v0.978: berry bush, wild flax -
        // same Harvestable the placed hen/goat carry) support walk-up
        // [E] collection. Spawn ready-to-harvest, like spawn_creature_at.
        if let Some(p) = renewable {
            let _ = world.insert_one(
                e,
                humanity_core::ecs::components::Harvestable {
                    resource: p.item,
                    amount: p.amount as f32,
                    regrow_time: p.regrow_s,
                    time_since_harvest: p.regrow_s,
                },
            );
        }
    }
    if packs > 0 {
        log::info!("Wild creatures: spawned {packs} in the wilds");
    }
    packs
}

#[cfg(test)]
#[path = "world_sim_tests.rs"]
mod tests;
