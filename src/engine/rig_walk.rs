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
//! module's [`tick`], run every frame from `poll_showcase_request`.

use crate::engine::state::EngineState;
use crate::input::bindings::GameAction;
use crate::renderer::camera::CameraController;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The longest hold one request may ask for, seconds.
pub(crate) const MAX_HOLD_S: f32 = 600.0;

/// The keys held and when they are let go. A static rather than a state field
/// because only the rig ever sets it, and a field would sit in every save of
/// the engine state's shape for a dev verb.
static HELD: Mutex<Option<(Instant, Vec<GameAction>)>> = Mutex::new(None);

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
            _ => return None,
        });
    }
    (!keys.is_empty()).then_some(keys)
}

/// Press `keys` for `secs` seconds. A new hold lets go of the previous one
/// first. Returns the line for the log.
pub(crate) fn hold(state: &mut EngineState, keys: Vec<GameAction>, secs: f32) -> String {
    hold_keys(&mut state.controller, &mut state.probe_hold, keys, secs)
}

/// [`hold`] on the two things a key press touches: the controller and the
/// camera park's pin. Each key goes down through the same function a
/// person's key does (`engine::input::apply_game_action`), so a held key
/// lets go of a camera park exactly as a key press does (2026-10-20: it
/// used to press the controller directly and leave the pin in place, so a
/// rig parked on a planet never moved and every fall it made was exempt).
fn hold_keys(
    controller: &mut CameraController,
    probe_hold: &mut Option<(glam::Vec3, Instant)>,
    keys: Vec<GameAction>,
    secs: f32,
) -> String {
    release(controller);
    let secs = if secs.is_finite() { secs.clamp(0.0, MAX_HOLD_S) } else { 0.0 };
    for k in &keys {
        crate::engine::input::apply_game_action(controller, probe_hold, *k, true);
    }
    let note = format!("holding {keys:?} for {secs} s");
    if let Ok(mut h) = HELD.lock() {
        *h = Some((Instant::now() + Duration::from_secs_f32(secs), keys));
    }
    note
}

/// Let go of the held keys once their time is up. Every frame.
pub(crate) fn tick(state: &mut EngineState) {
    let due = HELD
        .lock()
        .ok()
        .and_then(|h| h.as_ref().map(|(until, _)| Instant::now() >= *until))
        .unwrap_or(false);
    if due {
        release(&mut state.controller);
        log::info!("Showcase: hold -> released");
    }
}

/// Let go of whatever is held now.
fn release(controller: &mut CameraController) {
    let keys = HELD.lock().ok().and_then(|mut h| h.take()).map(|(_, k)| k).unwrap_or_default();
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
        let mut probe_hold = Some((glam::Vec3::new(1.0, 2.0, 3.0), Instant::now()));
        let note = hold_keys(&mut controller, &mut probe_hold, vec![GameAction::Jump], 0.4);
        assert!(note.contains("Jump"), "{note}");
        assert!(
            probe_hold.is_none(),
            "a rig holding Space after a camera park must let go of the park, as a person's key press does: {probe_hold:?}"
        );
        // And the key is down: the walk's wish has the climb in it; let go, and it has none.
        let camera = crate::renderer::camera::Camera::new();
        assert!(controller.surface_wish_dir(&camera).dot(camera.up) > 0.5, "Space is held");
        release(&mut controller);
        assert_eq!(controller.surface_wish_dir(&camera), glam::Vec3::ZERO, "and let go");
        // A key coming up leaves a park alone, as a person's does.
        let mut park = Some((glam::Vec3::ONE, Instant::now()));
        crate::engine::input::apply_game_action(&mut controller, &mut park, GameAction::Jump, false);
        assert!(park.is_some(), "a key coming up does not release a park");
    }

    /// "EXACTLY AS A KEY PRESS DOES" stays true only while both paths are one function: lib.rs's
    /// keyboard arm hands a person's key to `engine::input::apply_game_action`, and `hold_keys`
    /// hands the rig's to the same, with no other press into the controller in either.
    #[test]
    fn a_held_key_and_a_key_press_go_through_one_function() {
        let lib = include_str!("../lib.rs").replace("\r\n", "\n");
        assert!(
            lib.contains("crate::engine::input::apply_game_action(&mut state.controller, &mut state.probe_hold, act, pressed);"),
            "lib.rs's keyboard arm no longer hands a key to engine::input::apply_game_action"
        );
        assert!(!lib.contains("state.controller.apply_action("), "lib.rs presses the controller directly again, past the probe-hold rule");
        let this = include_str!("rig_walk.rs").replace("\r\n", "\n");
        let body = &this[this.find("fn hold_keys(").unwrap()..this.find("/// Let go of the held keys once").unwrap()];
        assert!(body.contains("crate::engine::input::apply_game_action(controller, probe_hold, *k, true);"), "hold_keys must press through apply_game_action");
        assert!(!body.contains(".apply_action("), "hold_keys presses the controller directly again");
    }
}
