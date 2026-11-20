//! The rig's held movement keys: arriving somewhere the way a player does
//! (BUG-156, 2026-10-05).
//!
//! Every rig verb that moves the player on a planet teleports: the camera park
//! drops the eye from hundreds of metres up, `stand` puts it on a ground point.
//! A teleport and a walk have arrived at different states before (the memory
//! note feedback_verify_on_player_state: flying in and teleporting in burned
//! three releases), and BUG-156's first question was exactly that: do trees
//! float only after the rig's 300 m drop, or for anyone who walks up to them?
//!
//! So the rig can now hold the movement keys the way a person does, through
//! the controller's own action path (`CameraController::apply_action`, the one
//! the keymap drives): `{"hold":"forward,sprint","hold_s":"40"}` presses
//! forward and sprint for 40 seconds and lets go. Gravity, the ground clamp,
//! the tangent-plane walk, the terrain streaming in as the player moves and the
//! tree harvest following them all run exactly as they do for a player,
//! because nothing here touches any of them. Send it with `stand` and
//! `walk` (engine/ipc.rs) to start from a chosen point on foot.
//!
//! The rig's window is never focused, so it receives no key events, and nothing
//! else releases these keys; the release at the end of the hold is this
//! module's [`tick`], run every frame from `poll_showcase_request` (and after
//! every step of a headless run, engine/sim_verbs.rs).
//!
//! THE STEP'S CLOCK (the headless sim runner, increment 5, 2026-11-20): a hold
//! lasts its seconds of SIMULATION time (`state.clock.elapsed_s`, the sum of the
//! frames' steps), not of the wall clock. In the windowed game at a steady frame
//! rate the two are the same; on a hitch past the step's 0.1 s cap the wall
//! clock ran on while the body did not, so a hold planned as 1.6 s of walking
//! walked less (risk 3 of the design: the judges read the recorded frame times).
//! And a headless run, which steps faster than real time, holds its keys for the
//! seconds of the world it asked for. The held keys live in the engine's state
//! (`EngineState::rig_hold`), so two engines in one process (the lib tests, a
//! soak's threads) never let go of each other's keys.

use crate::engine::state::EngineState;
use humanity_core::input::bindings::GameAction;
use crate::renderer::camera::CameraController;

/// The longest hold one request may ask for, seconds.
pub(crate) const MAX_HOLD_S: f32 = 600.0;

/// The keys a rig holds and the step's time they are let go at.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RigHold {
    /// `state.clock.elapsed_s` at which the keys come up.
    pub until_s: f64,
    pub keys: Vec<GameAction>,
}

/// The movement keys a hold may name: "forward,sprint" and the like.
pub(crate) fn parse_keys(spec: &str) -> Option<Vec<GameAction>> {
    let mut keys = Vec::new();
    for word in spec.split(',').map(str::trim).filter(|w| !w.is_empty()) {
        keys.push(match word {
            "forward" => GameAction::MoveForward,
            "back" => GameAction::MoveBack,
            "left" => GameAction::MoveLeft,
            "right" => GameAction::MoveRight,
            "jump" => GameAction::Jump,
            "sprint" => GameAction::Sprint,
            // Character animation rung 2a: the crouch vantages hold it.
            "crouch" => GameAction::Crouch,
            _ => return None,
        });
    }
    (!keys.is_empty()).then_some(keys)
}

/// Press `keys` for `secs` seconds of the step's clock. A new hold lets go of
/// the previous one first. Returns the line for the log.
pub(crate) fn hold(state: &mut EngineState, keys: Vec<GameAction>, secs: f32) -> String {
    let now_s = state.clock.elapsed_s;
    hold_keys(&mut state.controller, &mut state.probe_hold, &mut state.rig_hold, now_s, keys, secs)
}

/// [`hold`] on the things a key press touches: the controller and the camera
/// park's pin, and the hold itself. Each key goes down through the same
/// function a person's key does (`engine::input::apply_game_action`), so a held
/// key lets go of a camera park exactly as a key press does (2026-10-20: it
/// used to press the controller directly and leave the pin in place, so a rig
/// parked on a planet never moved and every fall it made was exempt).
fn hold_keys(
    controller: &mut CameraController,
    probe_hold: &mut Option<(glam::Vec3, f64)>,
    held: &mut Option<RigHold>,
    now_s: f64,
    keys: Vec<GameAction>,
    secs: f32,
) -> String {
    release(controller, held);
    let secs = if secs.is_finite() { secs.clamp(0.0, MAX_HOLD_S) } else { 0.0 };
    for k in &keys {
        crate::engine::input::apply_game_action(controller, probe_hold, *k, true);
    }
    let note = format!("holding {keys:?} for {secs} s");
    *held = Some(RigHold { until_s: now_s + f64::from(secs), keys });
    note
}

/// Let go of the held keys once their time is up on the step's clock. Every
/// frame, after the step.
pub(crate) fn tick(state: &mut EngineState) {
    if state.rig_hold.as_ref().is_some_and(|h| state.clock.elapsed_s >= h.until_s) {
        release(&mut state.controller, &mut state.rig_hold);
        // Where the walk left the body, so a rig reads where a held walk stopped (a wall, a table)
        // from the log (2026-11-13, the player stopping at furniture).
        let p = state.camera.position;
        log::info!("Showcase: hold -> released, the eye at ({:.2}, {:.2}, {:.2})", p.x, p.y, p.z);
    }
}

/// Let go of whatever is held now.
fn release(controller: &mut CameraController, held: &mut Option<RigHold>) {
    let keys = held.take().map(|h| h.keys).unwrap_or_default();
    // A key coming up touches no pin, so there is none to hand over.
    let mut no_pin = None;
    for k in keys {
        crate::engine::input::apply_game_action(controller, &mut no_pin, k, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_names_only_movement_keys() {
        assert_eq!(
            parse_keys("forward, sprint"),
            Some(vec![GameAction::MoveForward, GameAction::Sprint])
        );
        assert_eq!(parse_keys("back,left,right,jump").map(|k| k.len()), Some(4));
        assert_eq!(parse_keys(""), None, "an empty hold presses nothing");
        assert_eq!(parse_keys("forward,interact"), None, "only the movement keys");
    }

    /// A RIG'S HELD KEY LETS GO OF THE CAMERA PARK, AS A PERSON'S KEY DOES (2026-10-20). A
    /// camera park (`camera_request`) pins the eye where it was parked (`probe_hold`) until
    /// real movement input arrives, and lib.rs's keyboard arm lets go of that pin on any key
    /// press. `hold` pressed the keys and left the pin in place, so a rig that parked on a
    /// planet and then held forward or Space never moved the body (the pin put the eye back
    /// every frame), and every fall it made was exempt: the probe hold is one of the
    /// exemptions `CameraController::surface_step` is handed (verify-falls' planet legs).
    #[test]
    fn a_held_key_releases_the_camera_park_as_a_key_press_does() {
        let mut controller = CameraController::new(5.0, 1.0);
        let mut probe_hold = Some((glam::Vec3::new(1.0, 2.0, 3.0), 0.0));
        let mut held = None;
        let note = hold_keys(&mut controller, &mut probe_hold, &mut held, 12.0, vec![GameAction::Jump], 0.4);
        assert!(note.contains("Jump"), "{note}");
        assert!(
            probe_hold.is_none(),
            "a rig holding Space after a camera park must let go of the park, as a person's key press does: {probe_hold:?}"
        );
        // Held until 0.4 s of the step's clock after it was pressed (increment 5).
        let until = held.as_ref().map_or(0.0, |h| h.until_s);
        assert!((until - 12.4).abs() < 1e-6, "held until {until}");
        // And the key is down: the walk's wish has the climb in it; let go, and it has none.
        let camera = crate::renderer::camera::Camera::new();
        assert!(controller.surface_wish_dir(&camera).dot(camera.up) > 0.5, "Space is held");
        release(&mut controller, &mut held);
        assert_eq!(controller.surface_wish_dir(&camera), glam::Vec3::ZERO, "and let go");
        assert!(held.is_none(), "nothing is held after the release");
        // A key coming up leaves a park alone, as a person's does.
        let mut park = Some((glam::Vec3::ONE, 0.0));
        crate::engine::input::apply_game_action(&mut controller, &mut park, GameAction::Jump, false);
        assert!(park.is_some(), "a key coming up does not release a park");
    }

    /// A HOLD LASTS ITS SECONDS OF THE STEP'S CLOCK (increment 5, 2026-11-20), however long the
    /// steps took on the wall clock: a body held at a slow frame rate walks as far as at a fast one,
    /// and a headless run (faster than real time) holds as long as it asked. Two holds in two
    /// engines' states do not touch each other (the hold is a state field, not a static).
    #[test]
    fn a_hold_ends_on_the_step_clock_and_belongs_to_its_engine() {
        let camera = crate::renderer::camera::Camera::new();
        let (mut ca, mut cb) = (CameraController::new(5.0, 1.0), CameraController::new(5.0, 1.0));
        let (mut ha, mut hb) = (None, None);
        let mut no_park = None;
        hold_keys(&mut ca, &mut no_park, &mut ha, 0.0, vec![GameAction::MoveForward], 1.0);
        hold_keys(&mut cb, &mut no_park, &mut hb, 0.0, vec![GameAction::MoveForward], 0.5);
        // Engine b's clock passes its hold; a's is not touched by b's release.
        let due = |h: &Option<RigHold>, t: f64| h.as_ref().is_some_and(|h| t >= h.until_s);
        assert!(due(&hb, 0.5) && !due(&ha, 0.5));
        release(&mut cb, &mut hb);
        assert_eq!(cb.surface_wish_dir(&camera), glam::Vec3::ZERO, "b let go");
        assert!(ca.surface_wish_dir(&camera).length() > 0.5, "a still holds forward after b let go");
        assert!(!due(&ha, 0.999) && due(&ha, 1.0), "a lets go at 1.0 s of its own clock");
    }

    /// "EXACTLY AS A KEY PRESS DOES" stays true only while both paths are one function: lib.rs's
    /// keyboard arm hands a person's key to `engine::input::apply_game_action`, and `hold_keys`
    /// hands the rig's to the same, with no other press into the controller in either.
    #[test]
    fn a_held_key_and_a_key_press_go_through_one_function() {
        let lib = humanity_terrain::planet_chunks::frame_loop_source().replace("\r\n", "\n");
        assert!(
            lib.contains("crate::engine::input::apply_game_action(&mut state.controller, &mut state.probe_hold, act, pressed);"),
            "lib.rs's keyboard arm no longer hands a key to engine::input::apply_game_action"
        );
        assert!(!lib.contains("state.controller.apply_action("), "lib.rs presses the controller directly again, past the probe-hold rule");
        let this = include_str!("rig_walk.rs").replace("\r\n", "\n");
        let body = &this[this.find("fn hold_keys(").unwrap()..this.find("/// Let go of the held keys once").unwrap()];
        assert!(body.contains("crate::engine::input::apply_game_action(controller, probe_hold, *k, true);"), "hold_keys must press through apply_game_action");        assert!(!body.contains(".apply_action("), "hold_keys presses the controller directly again");
    }
}
