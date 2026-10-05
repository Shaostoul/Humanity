//! The quests' per-frame glue (2026-10-04, the opening): what the quest
//! system cannot see from the ECS, handed to it once a frame. lib.rs calls
//! `frame` beside the other GUI-to-system bridges.
//!
//! 1. Views that came on screen. A `View` step ("check your vitals") is done
//!    by opening the view, which only the interface knows about: a page marks
//!    each view it draws (`GuiState::on_screen`), and here each one that was
//!    not on screen the frame before becomes one quest event. Once per
//!    opening, not once per frame, so a progress counter does not grow for as
//!    long as the page stays open.
//! 2. The player's own front door, for the opening's last step: the doorstep
//!    of whichever plot the home stands on, read off the ship the game runs
//!    (`systems::quests::publish_front_door`). A guest's home is put away and
//!    has no door aboard, so nothing is published then.

use crate::engine::state::EngineState;

/// Once per frame: report the views that came on screen and publish the
/// front door.
pub(crate) fn frame(state: &mut EngineState) {
    for view in views_that_came_on_screen(&mut state.gui_state) {
        crate::systems::quests::push_quest_event(&state.data_store, crate::systems::quests::view_event_key(view));
    }
    crate::systems::quests::publish_front_door(&mut state.data_store, state.gui_state.ship_structure.as_ref());
}

/// The views the interface drew since this was last asked that it had not
/// drawn the time before, and the bookkeeping for the next frame: what is on
/// screen now becomes "the frame before", and the list starts empty for the
/// pages to fill again. Pure on the GuiState, so it is tested without a
/// window.
pub(crate) fn views_that_came_on_screen(gui: &mut crate::gui::GuiState) -> Vec<&'static str> {
    let now = std::mem::take(&mut gui.views_on_screen);
    let fresh: Vec<&'static str> = now.iter().copied().filter(|v| !gui.prev_views_on_screen.contains(v)).collect();
    gui.prev_views_on_screen = now;
    fresh
}

/// Draw pages that the player did not open without their views counting:
/// the in-world wall screens draw the real pages against the shared GuiState
/// (engine::screens `frame_surfaces`), so the Entry's inventory wall
/// marked the vitals on screen whenever it was in front of the player, and
/// "check your vitals: press I" would be done by waking up facing it. What
/// was marked before `draw` stays; what `draw` marks is dropped.
pub(crate) fn off_the_record<R>(gui: &mut crate::gui::GuiState, draw: impl FnOnce(&mut crate::gui::GuiState) -> R) -> R {
    let opened = std::mem::take(&mut gui.views_on_screen);
    let out = draw(gui);
    gui.views_on_screen = opened;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A VIEW COUNTS ONCE PER OPENING. The Inventory page marks the vitals
    /// as on screen every frame it shows them; the quest event goes out on
    /// the first of those frames only, and again only after the view was
    /// closed and opened again.
    ///
    /// Red, run with every view on screen reported each frame: "still open:
    /// not again".
    #[test]
    fn a_view_is_reported_once_each_time_it_comes_on_screen() {
        let mut gui = crate::gui::GuiState::default();
        assert!(views_that_came_on_screen(&mut gui).is_empty(), "nothing drawn, nothing reported");
        gui.on_screen("vitals");
        gui.on_screen("vitals");
        assert_eq!(views_that_came_on_screen(&mut gui), vec!["vitals"], "opened: reported once");
        gui.on_screen("vitals");
        assert!(views_that_came_on_screen(&mut gui).is_empty(), "still open: not again");
        assert!(views_that_came_on_screen(&mut gui).is_empty(), "closed: nothing");
        gui.on_screen("vitals");
        assert_eq!(views_that_came_on_screen(&mut gui), vec!["vitals"], "opened again: reported again");
    }

    /// A WALL SCREEN'S PAGE IS NOT ONE THE PLAYER OPENED (2026-10-04). The
    /// Entry's inventory wall draws the real Inventory page, Status
    /// section and all, whenever it is in front of the player; drawn off the
    /// record, the vitals it shows do not finish "check your vitals: press I".
    /// The wall screens' frame goes through it (read from its source, so the
    /// helper cannot pass while the screens stop calling it).
    ///
    /// Red, run with what the wall drew kept on the record: "the wall's
    /// Inventory page does not count as opening it" (left: ["vitals", "map"],
    /// right: ["map"]).
    #[test]
    fn a_page_on_a_wall_screen_does_not_count_as_opened() {
        let mut gui = crate::gui::GuiState::default();
        gui.on_screen("map");
        off_the_record(&mut gui, |gui| gui.on_screen("vitals"));
        assert_eq!(views_that_came_on_screen(&mut gui), vec!["map"], "the wall's Inventory page does not count as opening it");
        let screens = include_str!("screens.rs");
        assert!(
            screens.contains("crate::engine::quest_hooks::off_the_record(gui_state, |gui_state| {"),
            "engine::screens draws the wall screens' pages off the record"
        );
    }
}
