//! The mining drone's engine half (first-hour audit S5, 2026-10-12): the Settings >
//! Gameplay > Mining drone mode into the DataStore, the Mining panel's Repair and Cancel
//! into the drone system, and what the drone system last showed (its battery, its wear,
//! its service) onto the panel. The rules live in systems::mining.

use crate::gui::GuiState;
use crate::hot_reload::data_store::DataStore;

/// Once a frame, before the systems tick (lib.rs).
pub(crate) fn bridge(state: &mut crate::engine::state::EngineState) {
    bridge_gui(&mut state.gui_state, &state.data_store);
}

/// The bridge on its two halves, so it is tested without an engine.
pub(crate) fn bridge_gui(gui: &mut GuiState, data: &DataStore) {
    crate::systems::mining::publish_mode(data, gui.settings.drone_realistic);
    if let Some(req) = gui.pending_drone_request.take() {
        crate::systems::mining::request(data, req);
    }
    gui.drone_readout = crate::systems::mining::readout(data);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::mining::{DroneReadout, DroneRequest, Mode, MODE_KEY, READOUT_KEY, REQUEST_KEY};
    use std::sync::Mutex;

    /// The panel's Repair reaches the drone system, the Settings mode reaches the
    /// DataStore, and what the drone system showed reaches the panel. Seen red with
    /// `bridge_gui` empty: "the panel's Repair is handed over" (left: None).
    #[test]
    fn the_bridge_hands_the_panel_and_the_mode_across() {
        let mut data = DataStore::new();
        data.insert(MODE_KEY, Mutex::new(false));
        data.insert(REQUEST_KEY, Mutex::new(Option::<DroneRequest>::None));
        let shown = DroneReadout { trip_wh: 4264.0, ..Default::default() };
        data.insert(READOUT_KEY, Mutex::new(Some(shown.clone())));
        let mut gui = GuiState::default();
        gui.settings.drone_realistic = true;
        gui.pending_drone_request = Some(DroneRequest::Repair);
        bridge_gui(&mut gui, &data);
        let handed = *data.get::<Mutex<Option<DroneRequest>>>(REQUEST_KEY).unwrap().lock().unwrap();
        assert_eq!(handed, Some(DroneRequest::Repair), "the panel's Repair is handed over");
        assert_eq!(gui.pending_drone_request, None, "and taken off the panel");
        assert_eq!(Mode::from_store(&data), Mode::Realistic, "the Settings mode is published");
        assert_eq!(gui.drone_readout, Some(shown), "the panel shows what the drone system showed");
    }
}
