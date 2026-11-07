//! The world's systems held still and quiet until the person enters a world (BUG-253, 2026-11-06;
//! the rule and its why are in gui/world_entry.rs). lib.rs ticks the systems through `tick` and
//! takes their notices through `take_notices`, once a frame, so the headless test below drives the
//! same two functions the game does.

use humanity_core::ecs::systems::SystemRunner;
use humanity_core::hot_reload::data_store::DataStore;

use crate::gui::GuiState;

/// One frame of the world's systems. Whether the world runs (`GuiState::note_world_frame`: once
/// the person has entered a world this session, and not while the character picker is open), the
/// save stamp's hold kept with it (save_load `hold_world` / `run_world`), and the systems ticked by
/// `dt` while it runs and by no time while it is held: they still take what a page asks of them
/// (a transfer on the Inventory page), but nothing grows, ages, flies, wears or is eaten. Returns
/// the time the world moved on by, for the frame's other world clocks (the packs left where the
/// player fell, engine/death_pack.rs).
pub(crate) fn tick(runner: &mut SystemRunner, world: &mut hecs::World, data: &DataStore, gui: &mut GuiState, world_loaded: bool, dt: f32) -> f32 {
    let runs = gui.note_world_frame(world_loaded);
    if runs {
        crate::save_load::run_world();
    } else {
        crate::save_load::hold_world();
    }
    let world_dt = if runs { dt } else { 0.0 };
    let _cost_systems = humanity_core::frame_costs::stage("cpu.systems");
    runner.tick(world, world_dt, data);
    world_dt
}

/// What the systems said this frame, on their two channels (humanity_systems::notices): their
/// replies to what the person did on a page, shown at once, in a world or not (BUG-258: held, a
/// craft refused on the picker got no answer); and the world's own lines, shown while the world
/// runs and held for the person's entry while it does not (`GuiState::world_notice`, BUG-253).
pub(crate) fn take_notices(data: &DataStore, gui: &mut GuiState) {
    use humanity_systems::notices;
    for msg in notices::take(data, notices::REPLIES) {
        gui.reply_notice(msg);
    }
    for msg in notices::take(data, notices::WORLD) {
        gui.world_notice(msg);
    }
}

#[cfg(test)]
#[path = "world_hold_tests.rs"]
mod tests;
