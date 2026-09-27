//! Built beds and chests in use (2026-09-27): the crosshair prompt, the E
//! press, and the built chests kept in the Inventory page's places tree.
//!
//! The rules (which structures are usable, what the look ray meets, how a
//! chest's contents are addressed) live in `systems::construction::uses`,
//! where they are tested without a window. This is the frame glue: lib.rs
//! calls `frame` once per frame and `activate` from the E chain.

use crate::engine::state::EngineState;
use crate::systems::construction::{uses, BlueprintRegistry, Structure};

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
        .map(|(u, name)| u.prompt(&name))
        .unwrap_or_default();
}

/// The usable built structure under the crosshair, when nothing else is
/// claiming the E key: a page, the build editor, a vehicle, an animal, a
/// person, a door panel or a home machine all come first, in the same order
/// the E chain in lib.rs tries them.
fn target(state: &EngineState) -> Option<(uses::StructureUse, String)> {
    let g = &state.gui_state;
    let busy = g.active_page != crate::gui::GuiPage::None
        || g.construction_active
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
    let world = &state.game_world.world;
    let (e, u) = uses::looked_at(world, state.camera.position, state.camera.forward(), REACH_M)?;
    let s = world.get::<&Structure>(e).ok()?;
    let name = uses::display_name(&s, state.data_store.get::<BlueprintRegistry>("blueprint_registry"));
    Some((u, name))
}

/// The E press on a built structure. A bed: lie down and sleep the night
/// (`systems::sleep`). A chest: open the Inventory page, where it is a
/// container like any other. Returns false when the crosshair prompt is
/// empty, so the E chain carries on to what comes after.
pub(crate) fn activate(state: &mut EngineState) -> bool {
    if state.gui_state.structure_prompt.is_empty() {
        return false;
    }
    let Some((u, name)) = target(state) else { return false };
    match u {
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
