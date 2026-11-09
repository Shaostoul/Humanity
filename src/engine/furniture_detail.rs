//! FURNITURE DETAIL, the engine's half (2026-11-09; docs/design/simready-props.md, sections 9 and
//! 10). The levels, the measured memory and the hardware notice are the GUI's
//! (`crate::gui::furniture_detail`), since Settings and the saved config read them; this half
//! rebuilds the home's bodies when they ask for it.
//!
//! Kept out of lib.rs, which sits under the file-size ratchet: the hook there is one line.

use crate::engine::state::EngineState;

/// Once a frame: rebuild the home's bodies when the Furniture detail level or the installed model
/// packs changed since they were last built.
pub(crate) fn before_scene(state: &mut EngineState) {
    if state.machine_objects.is_empty() || !crate::gui::furniture_detail::needs_rebuild(&state.gui_state.settings.furniture_detail) {
        return;
    }
    log::info!("[FurnitureDetail] level or model packs changed; rebuilding the home's bodies");
    // An empty list takes `rebuild_machine_objects` down its full path (the count no longer
    // matches), which reuses every slot (`take_machine_slots`) and draws each model afresh.
    state.machine_objects.clear();
    crate::engine::home_meshes::rebuild_machine_objects(state);
}
