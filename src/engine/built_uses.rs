//! Built beds and chests in use (2026-09-27): the crosshair prompt, the E
//! press, and the built chests kept in the Inventory page's places tree. And
//! the home's own machines that are used the same way (2026-10-04, first-hour
//! audit F5): the bedroom's bed, whose def `provides: Some("rest")`, sleeps you
//! as a built bed does (`machine_use`, `use_machine`).
//!
//! The rules (which structures are usable, what the look ray meets, how a
//! chest's contents are addressed) live in `systems::construction::uses`,
//! where they are tested without a window. This is the frame glue: lib.rs
//! calls `frame` once per frame and `activate` from the E chain.

use crate::engine::state::EngineState;
use crate::systems::construction::{uses, BlueprintRegistry, DoorOpen, Structure};

/// How far away a built structure can be used from, metres: the same reach
/// the home machines' walk-up cards use.
const REACH_M: f32 = 5.0;

/// Once per frame: keep every built chest in the places tree (so it is a
/// container on the Inventory page and in every Stash to menu), and set the
/// crosshair prompt for the bed or chest the player is looking at.
pub(crate) fn frame(state: &mut EngineState) {
    let stores = uses::built_stores(
        &state.game_world.world,
        state.data_store.get::<BlueprintRegistry>("blueprint_registry"),
    );
    crate::gui::sync_built_stores(&mut state.gui_state.places, &stores);
    state.gui_state.structure_prompt = target(state)
        .map(|(_e, u, name)| u.prompt(&name))
        .unwrap_or_default();
}

/// Once per frame: the crafting stations WHERE THE PLAYER IS, for
/// CraftingSystem's station gate (`placed_machine_types`, `stations_where`)
/// and the Crafting page's mirror of it (v0.749 station gate; built
/// structures since 2026-09-25; by place since 2026-09-27, the planet-build
/// review). Aboard: the home's placed machines and the stations built in
/// the home, none of which count for a guest whose home is put away
/// (`station_types_here`). On a planet's ground: the stations built at the site the player
/// stands in, and nothing of the home's, which is in orbit. Anywhere else:
/// none. Power follows the same place (`station_unpowered_at`).
pub(crate) fn publish_stations(state: &mut EngineState) {
    use crate::systems::construction::StationsWhere;
    let here = match crate::engine::planet_build::player_frame(state) {
        Some(f) => f.site.map_or(StationsWhere::Home, StationsWhere::Site),
        None => StationsWhere::Nowhere,
    };
    let world = &state.game_world.world;
    let machines: Vec<(&str, &str)> = state
        .gui_state
        .home_machines
        .iter()
        .flat_map(|hm| {
            let rows = hm.instances.iter().map(|i| (i.machine.as_str(), i.zone.as_str()));
            rows.chain(hm.arrays.iter().map(|a| (a.machine.as_str(), a.zone.as_str())))
        })
        .collect();
    let types = station_types_here(
        world,
        state.data_store.get::<BlueprintRegistry>("blueprint_registry"),
        &here,
        &machines,
        state.gui_state.ship_structure.as_ref(),
    );
    // Electric stations here with no power, for the Crafting page.
    let mut unpowered = std::collections::HashSet::new();
    if let Some(frame) = here.frame() {
        for (_e, mt) in world.query::<&crate::ecs::components::MachineType>().iter() {
            if crate::systems::crafting::CraftingSystem::station_unpowered_at(world, &mt.0, frame).is_some() {
                unpowered.insert(mt.0.clone());
            }
        }
    }
    // Fail-open without a home layout (headless worlds), as before.
    if state.gui_state.home_machines.is_some() {
        state.data_store.insert("placed_machine_types", std::sync::Mutex::new(types.clone()));
    }
    state.data_store.insert("stations_where", std::sync::Mutex::new(here.clone()));
    // A hand craft also draws on the home's storage (BUG-147), aboard with
    // the home on this ship: a guest's home is put away, not here.
    let home_away = state.gui_state.ship_structure.as_ref().is_some_and(|s| s.home_is_away());
    let storage_here = here == StationsWhere::Home && !home_away;
    state.data_store.insert(
        crate::systems::crafting::home_store::HOME_STORAGE_HERE,
        std::sync::Mutex::new(storage_here),
    );
    state.gui_state.home_storage_here = storage_here;
    state.gui_state.stations_here = types;
    state.gui_state.stations_where = here;
    state.gui_state.unpowered_station_types = unpowered;
}

/// The crafting station types where the player is (`publish_stations`):
/// the pieces built in `here`'s frame and, aboard, the placed machines
/// (`machines`, each as its type and zone). A guest's home is put away
/// (`ShipStructure::put_home_away`, a kilometre off the ship): its machines
/// and the pieces built in it are not aboard, so they serve as no station,
/// the way its storage and tanks do not count (the review of BUG-147). The
/// ship's own machines (the Commons') are aboard for everyone.
pub(crate) fn station_types_here(
    world: &hecs::World,
    registry: Option<&BlueprintRegistry>,
    here: &crate::systems::construction::StationsWhere,
    machines: &[(&str, &str)],
    ship: Option<&crate::ship::ship_structure::ShipStructure>,
) -> std::collections::HashSet<String> {
    use crate::ship::ship_structure::HOME_ZONE_ID;
    use crate::systems::construction::{built_station_types_where, StationsWhere};
    let away = ship.is_some_and(|s| s.home_is_away());
    // Where the put-away home is kept: a piece standing over it was built in
    // that home and went away with it (home_plot::carry_built_pieces).
    let kept = ship.filter(|_| away).and_then(crate::engine::home_plot::home_box);
    let aboard = |pose: Option<&crate::ecs::components::Transform>| match (kept, pose) {
        (Some(b), Some(t)) => !crate::engine::home_plot::over_plot(t.position, b),
        _ => true,
    };
    let mut types = match (here.frame(), registry) {
        // Only the home frame holds a put-away home's pieces; a planet
        // site's pieces stand on the planet.
        (Some(None), Some(reg)) => built_station_types_where(world, reg, None, aboard),
        (Some(frame), Some(reg)) => built_station_types_where(world, reg, frame, |_| true),
        _ => Default::default(),
    };
    if *here == StationsWhere::Home {
        types.extend(machines.iter().filter(|(_, zone)| !(away && *zone == HOME_ZONE_ID)).map(|(m, _)| m.to_string()));
    }
    types
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::components::Transform;
    use crate::ship::ship_structure::{ShipStructure, HOME_AWAY_ORIGIN, HOME_ZONE_ID};
    use crate::systems::construction::{Structure, StationsWhere};
    use glam::Vec3;

    fn shipped_blueprints() -> BlueprintRegistry {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/blueprints/basic.ron");
        BlueprintRegistry::from_ron(&std::fs::read(path).unwrap()).unwrap()
    }

    fn piece(world: &mut hecs::World, blueprint: &str, at: Vec3) {
        let s = Structure { blueprint_id: blueprint.into(), health: 1.0, max_health: 1.0, provides: None, uid: 0 };
        world.spawn((s, Transform { position: at, ..Default::default() }));
    }

    /// A guest on a shared ship (its home put away) has no station of that
    /// home aboard: not its placed machines, not the pieces built in it. The
    /// ship's own machines still serve, and so does a piece the guest built
    /// aboard. With the home on its plot, all of them serve. Seen red before
    /// the fix: "a guest: the put-away home's stove is not aboard" (the home's
    /// machines and pieces were stations wherever the home stood).
    #[test]
    fn a_put_away_home_serves_as_no_station() {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let ship = ShipStructure::load_and_assemble_shipped(&data, Some("p1")).unwrap();
        let away = ship.put_home_away().unwrap();
        let reg = shipped_blueprints();
        let kept = Vec3::from(HOME_AWAY_ORIGIN) + Vec3::new(2.0, 0.0, 2.0);
        let mut world = hecs::World::new();
        piece(&mut world, "furnace", kept);
        piece(&mut world, "loom", Vec3::new(70.0, 0.0, 25.0));
        let machines = [("stove", HOME_ZONE_ID), ("trading_post", "commons")];
        let types = |s: &ShipStructure| station_types_here(&world, Some(&reg), &StationsWhere::Home, &machines, Some(s));

        let guest = types(&away);
        assert!(!guest.contains("stove"), "a guest: the put-away home's stove is not aboard: {guest:?}");
        assert!(!guest.contains("smelter"), "a guest: the furnace built in the put-away home is not aboard: {guest:?}");
        assert!(guest.contains("trading_post"), "the ship's own machines are aboard for everyone: {guest:?}");
        assert!(guest.contains("loom"), "a piece built aboard serves: {guest:?}");

        let home = types(&ship);
        for t in ["stove", "smelter", "trading_post", "loom"] {
            assert!(home.contains(t), "the home on its plot: {t} serves: {home:?}");
        }
    }

    /// THE BEDROOM'S OWN BED SLEEPS YOU, AS A BED YOU BUILT DOES (first-hour audit F5,
    /// 2026-10-04). Its card always said "sleep here", but E only opened the card: only a
    /// built bed slept. In both shipped homes the bed's def now says `provides: Some("rest")`,
    /// the word a built bed's blueprint uses, so its label carries the use, the crosshair
    /// says what a built bed's says, and E asks for the night in it through the same request
    /// (`systems::sleep::request`, by the name the label shows). Nothing else in either home
    /// sleeps you: the court's raised beds are garden beds.
    ///
    /// Seen red 2026-10-04 on 1b66dd36d, with `machine_use` returning None and
    /// `use_machine_label` false, which is what the code did (no machine was used: E opened
    /// every machine's card, the bed's too): "home.ron: the machines E sleeps you in / left:
    /// [] / right: [\"bed\"]".
    #[test]
    fn the_bedrooms_bed_sleeps_you_as_a_built_bed_does() {
        use crate::machines::MachineHome;
        use std::sync::Mutex;
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("machines");
        for file in ["home.ron", "home_solo.ron"] {
            let home = MachineHome::load(&dir.join(file)).expect("the home parses");
            let mut sleepers: Vec<&str> = home
                .catalog
                .iter()
                .filter(|(_, d)| machine_use(d) == Some(uses::StructureUse::Sleep))
                .map(|(k, _)| k.as_str())
                .collect();
            sleepers.sort();
            assert_eq!(sleepers, vec!["bed"], "{file}: the machines E sleeps you in");
            assert!(home.all_instances().iter().any(|i| i.machine == "bed"), "{file}: a bed is placed");
        }
        let home = MachineHome::load(&dir.join("home.ron")).unwrap();
        let label = |machine: &str, name: &str| crate::gui::MachineLabel {
            pos: Vec3::ZERO,
            name: name.into(),
            stats: Vec::new(),
            room: "room-bedroom".into(),
            machine_id: format!("{machine}_1"),
            on_use: machine_use(&home.catalog[machine]),
        };
        let bed = label("bed", "Bed");
        assert_eq!(bed.prompt(false), uses::StructureUse::Sleep.prompt("Bed"), "the crosshair says what a built bed's says");

        // E at it asks for the night in the Bed, as a built bed does.
        let mut data = crate::hot_reload::data_store::DataStore::new();
        data.insert(crate::systems::sleep::REQUEST_SLOT, Mutex::new(None::<String>));
        assert!(use_machine_label(&data, &bed), "E at the bed is used, not opened");
        let asked = data.get::<Mutex<Option<String>>>(crate::systems::sleep::REQUEST_SLOT).unwrap().lock().unwrap().clone();
        assert_eq!(asked.as_deref(), Some("Bed"), "the sleep request, by the bed's name");

        // Any other machine still opens and closes its card.
        let nightstand = label("nightstand", "Nightstand");
        *data.get::<Mutex<Option<String>>>(crate::systems::sleep::REQUEST_SLOT).unwrap().lock().unwrap() = None;
        assert!(!use_machine_label(&data, &nightstand), "a nightstand is opened");
        assert_eq!(nightstand.prompt(false), "[E] open Nightstand");
        assert_eq!(nightstand.prompt(true), "[E] close Nightstand");
        assert!(data.get::<Mutex<Option<String>>>(crate::systems::sleep::REQUEST_SLOT).unwrap().lock().unwrap().is_none());
    }
}

/// The usable built structure under the crosshair, when nothing else is
/// claiming the E key: a page, the build editor, a vehicle, an animal, a
/// person, a door panel or a home machine all come first, in the same order
/// the E chain in lib.rs tries them. A built piece in hand comes before all
/// of them (lib.rs asks `build_place::key` first).
fn target(state: &EngineState) -> Option<(hecs::Entity, uses::StructureUse, String)> {
    let g = &state.gui_state;
    let busy = g.active_page != crate::gui::GuiPage::None
        || g.construction_active
        // A piece in hand: E builds it (engine/build_place.rs).
        || g.build_placing.is_some()
        || g.showroom_active
        || state.camera.mode != crate::renderer::camera::CameraMode::FirstPerson
        || state.driving_vehicle.is_some()
        || state.targeted_vehicle.is_some()
        || state.targeted_livestock.is_some()
        || g.targeted_npc.is_some()
        || g.targeted_control_panel.is_some()
        || g.targeted_machine.is_some();
    if busy {
        return None;
    }
    // In the frame the player is in (2026-09-27, BUG-102): the home aboard,
    // or the build site they stand in on a planet, eye and look converted
    // into it. It used to be the raw camera against every piece.
    let world = &state.game_world.world;
    let f = crate::engine::planet_build::player_frame(state)?;
    let registry = state.data_store.get::<BlueprintRegistry>("blueprint_registry");
    // Aboard, the home's own walls hide what is behind them.
    let walls: &[crate::ship::wall_collision::WallSegment] = if f.site.is_none() { state.wall_colliders.as_slice() } else { &[] };
    let (e, u) = uses::looked_at(world, registry, f.eye, f.forward, REACH_M, f.site.as_ref(), walls)?;
    let s = world.get::<&Structure>(e).ok()?;
    let name = uses::display_name(&s, registry);
    Some((e, u, name))
}

/// What E does at a home machine instead of opening its card (first-hour audit F5,
/// 2026-10-04): what its def `provides`, read the way a built piece's blueprint is read, so
/// the home's own bed (`provides: Some("rest")` in data/machines/home.ron) sleeps you exactly
/// as a bed you built does. Only sleeping acts at a machine: a machine's storage is used
/// through its card, so any other use leaves E opening the card. Which machines carry it is
/// data, so a bunk or a hammock in a home is a data edit.
pub(crate) fn machine_use(def: &crate::machines::MachineDef) -> Option<uses::StructureUse> {
    def.provides
        .as_deref()
        .and_then(uses::StructureUse::from_provides)
        .filter(|u| *u == uses::StructureUse::Sleep)
}

/// The E press at the home machine the player looks at, when it is one that is used (the
/// home's bed): true when it was, so the E chain in lib.rs goes no further. False for every
/// other machine, whose card E opens.
pub(crate) fn use_machine(state: &mut EngineState) -> bool {
    let Some(label) = state.gui_state.targeted_machine.and_then(|t| state.gui_state.machine_labels.get(t)) else {
        return false;
    };
    use_machine_label(&state.data_store, label)
}

/// `use_machine` for one machine's label: a machine that is slept in asks for the night in
/// it, by the name its label shows, through the request a built bed makes
/// (`systems::sleep::request`), so everything after is the built bed's (the clock runs fast
/// through the night, the shared world refuses it).
pub(crate) fn use_machine_label(data: &crate::hot_reload::data_store::DataStore, label: &crate::gui::MachineLabel) -> bool {
    match label.on_use {
        Some(uses::StructureUse::Sleep) => {
            crate::systems::sleep::request(data, &label.name);
            true
        }
        _ => false,
    }
}

/// The E press on a built structure. A bed: lie down and sleep the night
/// (`systems::sleep`). A chest: open the Inventory page, where it is a
/// container like any other. Returns false when the crosshair prompt is
/// empty, so the E chain carries on to what comes after.
pub(crate) fn activate(state: &mut EngineState) -> bool {
    if state.gui_state.structure_prompt.is_empty() {
        return false;
    }
    let Some((e, u, name)) = target(state) else { return false };
    match u {
        // A door in a wall (2026-09-28): open it if shut, shut it if open.
        // The open one swings a quarter turn out of the gap and stops blocking
        // (`doorway::parts`, `build_place::built_piece_segments`).
        uses::StructureUse::Door => {
            let world = &mut state.game_world.world;
            // The same sounds the home's own doors make (home_meshes.rs); the
            // player pressing E is at the door, so no earshot test is needed.
            if world.get::<&DoorOpen>(e).is_ok() {
                let _ = world.remove_one::<DoorOpen>(e);
                state.pending_sfx.push(("sfx.door_close", "audio/sfx/door_close.ogg"));
            } else {
                let _ = world.insert_one(e, DoorOpen);
                state.pending_sfx.push(("sfx.door_open", "audio/sfx/door_open.ogg"));
            }
        }
        uses::StructureUse::Sleep => crate::systems::sleep::request(&state.data_store, &name),
        uses::StructureUse::Store => {
            state.gui_state.active_page = crate::gui::GuiPage::Inventory;
            // Opening a page mid-walk swallows the key releases: clear held
            // movement in both movers, as the NPC talk card does.
            state.controller.stop_movement();
            state.data_store.insert("input_state", crate::input::InputState::default());
            state.gui_state.pending_toasts.push((
                format!("The {name} is under You & your places: drag items onto it to store them."),
                crate::gui::ToastKind::Info,
            ));
        }
    }
    state.gui_state.structure_prompt.clear();
    true
}
