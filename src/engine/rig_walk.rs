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
    release(state);
    let secs = if secs.is_finite() { secs.clamp(0.0, MAX_HOLD_S) } else { 0.0 };
    for k in &keys {
        state.controller.apply_action(*k, true);
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
        release(state);
        log::info!("Showcase: hold -> released");
    }
}

/// Let go of whatever is held now.
fn release(state: &mut EngineState) {
    let keys = HELD.lock().ok().and_then(|mut h| h.take()).map(|(_, k)| k).unwrap_or_default();
    for k in keys {
        state.controller.apply_action(k, false);
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
}
