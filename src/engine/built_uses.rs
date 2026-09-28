//! Built beds and chests in use (2026-09-27): the crosshair prompt, the E
//! press, and the built chests kept in the Inventory page's places tree.
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
/// the home. On a planet's ground: the stations built at the site the player
/// stands in, and nothing of the home's, which is in orbit. Anywhere else:
/// none. Power follows the same place (`station_unpowered_at`).
pub(crate) fn publish_stations(state: &mut EngineState) {
    use crate::systems::construction::{built_station_types, StationsWhere};
    let here = match crate::engine::planet_build::player_frame(state) {
        Some(f) => f.site.map_or(StationsWhere::Home, StationsWhere::Site),
        None => StationsWhere::Nowhere,
    };
    let world = &state.game_world.world;
    let built = match (here.frame(), state.data_store.get::<BlueprintRegistry>("blueprint_registry")) {
        (Some(frame), Some(reg)) => built_station_types(world, reg, frame),
        _ => Default::default(),
    };
    let mut types = built;
    if here == StationsWhere::Home {
        if let Some(hm) = &state.gui_state.home_machines {
            types.extend(hm.instances.iter().map(|i| i.machine.clone()));
            types.extend(hm.arrays.iter().map(|a| a.machine.clone()));
        }
    }
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
    state.gui_state.stations_here = types;
    state.gui_state.stations_where = here;
    state.gui_state.unpowered_station_types = unpowered;
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
    let (e, u) = uses::looked_at(world, registry, f.eye, f.forward, REACH_M, f.site.as_ref())?;
    let s = world.get::<&Structure>(e).ok()?;
    let name = uses::display_name(&s, registry);
    Some((e, u, name))
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
            if world.get::<&DoorOpen>(e).is_ok() {
                let _ = world.remove_one::<DoorOpen>(e);
            } else {
                let _ = world.insert_one(e, DoorOpen);
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
