//! HOW FAST A PERSON WALKS AND RUNS, by the walking pace mode (2026-11-09).
//!
//! The operator, 2026-11-09, deciding the question character animation rung 1 raised (the game
//! walked at 5 m/s, a jog, so a body's walk had nothing to match): the Realistic mode walks at
//! 1.4 m/s and runs at about 4; the Simplified mode keeps the game's 5 m/s walk and 9.5 m/s sprint.
//! The house rule for deep systems (CLAUDE.md, "Every deep system ships two modes") gives every one
//! of them a switch in Settings > Gameplay; movement had none, so this adds the smallest honest one,
//! `settings.walk_realistic` (Simplified by default, like the others), persisted in AppConfig.
//!
//! The speeds are data/game.csv's four `*_speed_*_mps` rows, read at boot, on a game.csv save (hot
//! reload) and whenever the switch changes; a missing row keeps the compiled-in pace
//! (`WALK_SPEED_MPS`, `SPRINT_FACTOR`). They set the controller's walk speed and its sprint factor;
//! every other speed rule (status effects, a carried load, the planet walk's gear) multiplies on top
//! as before. The body's stride follows on its own: the locomotion graph reads the speed the body
//! really moves at (humanity-animation `MotionTracker`) and plays its gaits at that speed.

use crate::engine::state::EngineState;
use crate::renderer::camera::{CROUCH_FACTOR, SPRINT_FACTOR, WALK_SPEED_MPS};

/// A walking pace: metres a second walking and running.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Pace {
    pub walk_mps: f32,
    pub run_mps: f32,
    /// Crouched walking (character animation rung 2a, 2026-11-09: hold C), never faster than the walk.
    pub crouch_mps: f32,
}

/// The pace of a mode from game.csv's knobs (`knob` reads one by key), the compiled-in pace for any
/// row that is missing or not a sensible speed.
pub(crate) fn pace_for(realistic: bool, knob: &mut dyn FnMut(&str) -> Option<f64>) -> Pace {
    let mode = if realistic { "realistic" } else { "simplified" };
    let mut read = |key: String, fallback: f32| -> f32 {
        knob(&key).map(|v| v as f32).filter(|v| v.is_finite() && *v > 0.0).unwrap_or(fallback)
    };
    let walk = read(format!("walk_speed_{mode}_mps"), WALK_SPEED_MPS);
    let run = read(format!("run_speed_{mode}_mps"), WALK_SPEED_MPS * SPRINT_FACTOR);
    let crouch = read(format!("crouch_speed_{mode}_mps"), walk * CROUCH_FACTOR);
    Pace { walk_mps: walk, run_mps: run.max(walk), crouch_mps: crouch.min(walk) }
}

/// Set the controller's walk speed and sprint factor from the walking pace in Settings.
pub(crate) fn apply(state: &mut EngineState) {
    let realistic = state.gui_state.settings.walk_realistic;
    let r = pace_for(true, &mut |k| state.asset_manager.game_setting_f64(k));
    let s = pace_for(false, &mut |k| state.asset_manager.game_setting_f64(k));
    // Both modes' speeds, for Settings' description of the switch.
    state.gui_state.walk_pace_speeds = [r.walk_mps, r.run_mps, s.walk_mps, s.run_mps];
    let pace = if realistic { r } else { s };
    state.controller.speed = pace.walk_mps;
    state.controller.sprint_factor = pace.run_mps / pace.walk_mps;
    state.controller.crouch_factor = pace.crouch_mps / pace.walk_mps;
    log::info!(
        "Walking pace: {} ({:.2} m/s walking, {:.2} m/s running)",
        if realistic { "Realistic" } else { "Simplified" },
        pace.walk_mps,
        pace.run_mps
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped(key: &str) -> Option<f64> {
        let csv = std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data").join("game.csv")).expect("data/game.csv");
        csv.lines().find_map(|l| {
            let mut cols = l.split(',');
            (cols.next()? == key).then(|| cols.next()?.trim().parse().ok()).flatten()
        })
    }

    /// THE SHIPPED PACES are the operator's (2026-11-09): Realistic walks at 1.4 m/s and runs at
    /// 4, Simplified keeps 5 and 9.5. And no pace runs faster than the compiled walk times the
    /// sprint, the speed the relay's on-foot limit is held against
    /// (`the_on_foot_limit_covers_the_fastest_legitimate_walk`), so a faster row in game.csv cannot
    /// leave the shared world correcting an honest player.
    /// Seen red 2026-11-09 with game.csv's run_speed_simplified_mps set to 10.5: "the Simplified
    /// sprint 10.5 runs past 9.5, the on-foot limit's walk".
    #[test]
    fn the_shipped_paces_are_the_operators_and_stay_under_the_on_foot_limit() {
        let r = pace_for(true, &mut |k| shipped(k));
        let s = pace_for(false, &mut |k| shipped(k));
        let cap = WALK_SPEED_MPS * SPRINT_FACTOR + 1e-4;
        for (name, p) in [("Realistic", r), ("Simplified", s)] {
            assert!(p.run_mps <= cap, "the {name} sprint {} runs past {cap:.1}, the on-foot limit's walk", p.run_mps);
        }
        assert_eq!((r.walk_mps, r.run_mps), (1.4, 4.0), "Realistic");
        assert_eq!((s.walk_mps, s.run_mps), (5.0, 9.5), "Simplified");
        // Crouching (rung 2a): slower than walking in both modes, and the Realistic crouch near
        // the crouch clip's own pace (0.72 m/s on its rig), so its feet keep still.
        assert_eq!((r.crouch_mps, s.crouch_mps), (0.8, 1.6), "crouching");
    }

    /// A missing or broken row keeps the compiled-in pace, and a run slower than the walk is
    /// raised to it (Shift never slows you down).
    /// Seen red 2026-11-09 with the fallback taken for a negative value only (a 0 walk froze the
    /// player: "walk 0").
    #[test]
    fn a_missing_or_broken_row_keeps_the_compiled_pace() {
        let none = pace_for(true, &mut |_| None);
        assert_eq!(none, Pace { walk_mps: WALK_SPEED_MPS, run_mps: WALK_SPEED_MPS * SPRINT_FACTOR, crouch_mps: WALK_SPEED_MPS * CROUCH_FACTOR });
        let zero = pace_for(true, &mut |k| if k.starts_with("walk") { Some(0.0) } else { Some(f64::NAN) });
        assert!(zero.walk_mps > 0.0, "walk {}", zero.walk_mps);
        let slow_run = pace_for(true, &mut |k| if k.starts_with("walk") { Some(2.0) } else { Some(1.0) });
        assert_eq!(slow_run.run_mps, 2.0);
    }
}
