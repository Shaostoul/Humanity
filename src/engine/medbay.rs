//! The medbay's engine half (2026-11-21, the operator's call on botulism; systems::medbay).
//!
//! Each frame: does the player stand in a medbay? A medbay is any room whose type offers the
//! medbay's treatment (data/rooms.ron `medbay`: General Services' medbay today; the Medical Bay
//! district has no rooms yet), read off the room the HUD's room card names (`hud::room_at`), by the PAGE its action
//! opens ("medbay"), the way the walk-up stations are found (engine/frame_keys.rs): a label can be
//! reworded, the page is the action's identity. The answer goes to the food system, which refuses
//! a treatment asked for anywhere else, and to the Inventory page's button.

use crate::engine::state::EngineState;

/// The page a medbay's room action opens (data/rooms/room_actions.ron `medbay_treatment`).
pub(crate) const PAGE: &str = "medbay";

/// Whether a room offers the medbay's treatment.
pub(crate) fn is_medbay(room: &humanity_ship::room_types::RoomBounds) -> bool {
    room.action_pages.iter().any(|p| p == PAGE)
}

/// Publish whether the player stands in a medbay this frame.
pub(crate) fn publish(state: &mut EngineState) {
    let here = crate::gui::pages::hud::room_at(&state.gui_state.room_bounds, state.camera.position).is_some_and(is_medbay);
    humanity_systems::medbay::publish_here(&mut state.data_store, here);
    crate::gui::pages::medbay::set_here(here);
}
