//! The one "turn off all reuse" switch (CLAUDE.md, the reuse rule, refined 2026-10-30).
//!
//! The operator's rule: nothing kept across runs during development, and data kept in memory
//! across frames only when it cannot go stale (rebuilt on a schedule, or tagged with a version its
//! source bumps), "and every one of them obeys a single 'turn off all reuse' switch in the Dev page.
//! When a bug smells like a cache, one click rules caches in or out." (the proposal he took, "I like
//! this idea, let's do it."). Every reuse in the game asks `enabled()` and, when it is false, works
//! everything out again instead: the draw lists start empty (`engine::state::DrawBufs`), the dev
//! request reader checks every file every frame (`engine::ipc::RequestWatch`), the Garden page data
//! builds every grow area's rows every build instead of keeping them by area while the farm's
//! change numbers, the data's stamps and the clock say nothing they read moved
//! (`engine::garden_mirror`, `systems::farming::versions`, 2026-10-31), and a page lays out every
//! block of a long list, on screen or not (`gui::widgets::offscreen`, 2026-10-31: the Inventory
//! page's Garden groups and rows of slot tiles). The farm itself (systems/farming, farm-clock.md
//! rung 2): an area not yet advanced to now is state kept stale on purpose, so with the switch off
//! every grow area and the air advance every frame (`clock.rs`); and what the farm's body keeps
//! for a round of its rotation is worked out again in every run: which pests eat each plant
//! (`FarmingSystem::pest_hosts`), the grow rooms' layout for the air (`humidity::AirLayout`) and
//! the hives' and devices' reach (`pollination`). A new reuse that does not ask is a bug.
//!
//! The switch is the Dev tab's Performance section ("Turn off all reuse"), for this run only, and
//! `HUMANITY_NO_REUSE=1` for a rig or a scripted run.

use std::sync::atomic::{AtomicU8, Ordering};

const UNREAD: u8 = 0;
const ON: u8 = 1;
const OFF: u8 = 2;

/// Unread until the first question, when `HUMANITY_NO_REUSE` is read once.
static STATE: AtomicU8 = AtomicU8::new(UNREAD);

/// May a reuse keep what it worked out earlier? True unless the switch is off.
pub fn enabled() -> bool {
    match STATE.load(Ordering::Relaxed) {
        ON => true,
        OFF => false,
        _ => {
            let off = std::env::var_os("HUMANITY_NO_REUSE").is_some_and(|v| v == "1");
            if off {
                log::info!("Reuse: OFF for this run (HUMANITY_NO_REUSE=1): everything is worked out again every frame");
            }
            STATE.store(if off { OFF } else { ON }, Ordering::Relaxed);
            !off
        }
    }
}

/// Turn every reuse on or off, from the Dev page.
pub fn set_enabled(on: bool) {
    STATE.store(if on { ON } else { OFF }, Ordering::Relaxed);
    log::info!(
        "Reuse: {} (Dev > Performance)",
        if on { "ON: reused data is kept between frames again" } else { "OFF: everything is worked out again every frame" }
    );
}

#[cfg(test)]
mod tests {
    /// The switch flips both ways and every reader sees it at once.
    #[test]
    fn the_switch_turns_every_reuse_off_and_on() {
        super::set_enabled(false);
        assert!(!super::enabled());
        super::set_enabled(true);
        assert!(super::enabled());
    }
}
