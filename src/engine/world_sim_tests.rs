//! The homestead's simulation built with no GPU at all (the headless sim runner's increment 1,
//! 2026-11-15): no wgpu Instance, no window, no EngineState. The same three startup calls the
//! windowed game makes (lib.rs `resumed`: `sim_boot::boot`, the data registries, the starter
//! vehicles, the save's progress) and the world entry's `world_sim::build`, then a check that the
//! sim holds what the windowed game holds once it has entered the world: every system, the home's
//! rooms and walls, a live entity for every machine the world load draws, the home's air, and the
//! player standing in their home.
//!
//! Seen red 2026-11-15 before the gate was trusted: with the machines' spawn loop in `build`
//! skipped (`resolved.iter().take(0)`), it failed "no machines placed" at the first machine check.
//! (Its first draft also failed, rightly: it asked for an entity for EVERY drawn machine, and a
//! machine with no simulated role, furniture, has none in the windowed game either; the check is
//! now the power and water machines, and every live machine at its drawn spot.)
use super::*;
use crate::engine::sim_boot::{self, SimBootOptions};
use humanity_core::ecs::components::{Controllable, HomeMachine, MachineInstanceId};

/// The game's data folder, as the windowed game finds it.
fn data_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("data")
}

/// A saves folder of this test's own, empty, so no player's save is read or written.
fn scratch_saves(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("hos-world-sim-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch saves folder");
    dir
}

/// Boot the sim and enter the homestead the way the windowed game does, with no GPU.
fn enter_homestead(tag: &str) -> (sim_boot::SimBoot, GuiState, WorldSim) {
    let data_dir = data_dir();
    let _ = humanity_core::storage::DATA_DIR.set(data_dir.clone());
    let mut sim = sim_boot::boot(SimBootOptions { data_dir: data_dir.clone(), saves_dir: scratch_saves(tag), config: None });
    let mut gui = GuiState::default();
    crate::engine::registries::load_data_registries(&mut sim.data_store, &data_dir);
    sim_boot::starter_vehicles(&mut sim.game_world, &sim.data_store);
    let home_save = sim.home_save.take();
    sim_boot::apply_home_save(&mut sim.game_world, &mut sim.data_store, &mut gui, home_save.as_ref(), &data_dir);
    let built = build(Parts {
        data_dir: &data_dir,
        world: &mut sim.game_world.world,
        data_store: &mut sim.data_store,
        gui: &mut gui,
    });
    (sim, gui, built)
}

#[test]
fn the_homestead_sim_builds_with_no_gpu() {
    let (sim, gui, built) = enter_homestead("homestead");

    // Every system the startup registers is registered (none skipped or conditional), in the
    // order lib.rs ticks them.
    let registrations = include_str!("sim_boot.rs").matches("system_runner.register(").count();
    assert!(registrations >= 24, "sim_boot registers {registrations} systems; the game ran 24 when this was written");
    assert_eq!(sim.system_runner.count(), registrations, "a registration in sim_boot.rs did not run");
    let ids = sim.system_runner.cost_ids();
    let unique: std::collections::BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len(), "two systems share a cost id: {ids:?}");

    // The ship assembled with the home on its plot (not the legacy fallback), with rooms, a
    // spawn room and walls to collide with.
    assert!(gui.ship_structure.is_some(), "the ship did not assemble: {}", gui.construction_save_note);
    assert!(built.fallback_layout.is_none());
    assert!(built.boot_plot.is_some(), "the home stands on no plot");
    assert!(!built.room_info.is_empty(), "the home has no rooms");
    assert!(!built.wall_colliders.is_empty(), "the home has no walls to walk into");
    assert!(built.homestead_bounds.is_some());

    // The machines against what the world load draws (home_meshes::placements_of, what
    // build_machine_bodies is given): every live machine is a drawn one, standing where it is
    // drawn, and every drawn machine with a power or water role is live (a machine with no
    // simulated role, a chair or a shelf, is drawn and has no entity, in both paths).
    let placed = built.machines.as_ref().expect("the default household has a machine layout");
    assert!(!placed.world_pos.is_empty(), "no machines placed");
    let drawn: std::collections::BTreeMap<String, (f32, f32, f32)> = crate::engine::home_meshes::placements_of(&gui)
        .expect("the world load's placements")
        .into_iter()
        .map(|p| (p.id, p.pos))
        .collect();
    let world = &sim.game_world.world;
    let live: std::collections::BTreeMap<String, Vec3> = world
        .query::<(&HomeMachine, &MachineInstanceId, &humanity_core::ecs::components::Transform)>()
        .iter()
        .map(|(_, (_, id, t))| (id.0.clone(), t.position))
        .collect();
    assert!(!live.is_empty(), "no machine is live");
    for (id, at) in &live {
        let pos = drawn.get(id).unwrap_or_else(|| panic!("machine {id} is live but the world load draws no such machine"));
        assert!(
            (at.x - pos.0).abs() < 1e-3 && (at.z - pos.2).abs() < 1e-3,
            "machine {id} is live at {at:?} but drawn at {pos:?}"
        );
    }
    let home = gui.home_machines.as_ref().expect("the household's layout");
    let roles: Vec<String> = home
        .all_instances()
        .iter()
        .filter(|inst| drawn.contains_key(&inst.id))
        .filter(|inst| home.catalog.get(&inst.machine).is_some_and(|d| d.power.is_some() || d.is_water_machine()))
        .map(|inst| inst.id.clone())
        .collect();
    assert!(!roles.is_empty(), "the household draws no machine with a power or water role");
    let missing: Vec<&String> = roles.iter().filter(|id| !live.contains_key(*id)).collect();
    assert!(missing.is_empty(), "the world load draws {} powered or water machines with no live entity: {missing:?}", missing.len());

    // The home breathes its own air (the default start's `own_air`): one sealed air space.
    let air = world.query::<&humanity_systems::atmosphere::HomeAir>().iter().count();
    assert_eq!(air, 1, "the home's air spaces");

    // The player: one controllable body, arriving inside a room of their home.
    assert_eq!(world.query::<&Controllable>().iter().count(), 1, "the player's body");
    let spawn = built.spawn.expect("the home says where the player arrives");
    let inside = built.room_info.iter().any(|r| {
        let (lo, hi) = (r.center - r.dimensions * 0.5, r.center + r.dimensions * 0.5);
        spawn.position.cmpge(lo).all() && spawn.position.cmple(hi).all()
    });
    assert!(inside, "the player arrives at {:?}, inside no room", spawn.position);
}
