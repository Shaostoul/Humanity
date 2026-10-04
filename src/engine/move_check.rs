//! The game's half of the relay's speed check (increment 4 of
//! docs/design/ship-homes-and-logistics.md, "getting around at ship scale"; the relay's half is
//! src/relay/handlers/move_check.rs).
//!
//! The relay holds each player where their last accepted move left them and answers a move faster
//! than anyone can go with a CORRECTION: where it holds them. This file:
//!   - stands the player there when one arrives (`apply_correction`), says so in one sentence on
//!     screen (at most one every ten seconds), and reports it to the rig's probe;
//!   - stamps every position update with the newest correction applied (`correction`), so the
//!     relay can tell the updates sent before it from those sent after;
//!   - says what a fast move WAS when it was a real one (`declare`, `"moved"`): a teleporter jump
//!     (lib.rs, the teleporter pads, src/ship/transit.rs), shutting the build editor at the build
//!     spot (lib.rs, the editor's close), driving (every update while in a cab);
//!   - while the follow cam watches a vehicle in the shared world, reports where the body stands,
//!     not the camera hanging behind the vehicle, and puts the camera back there when it ends
//!     (`body_position`): watching is not walking, and the camera's first frame behind a summoned
//!     vehicle is a jump of any length;
//!   - walks the camera for the rig (`walk_tick`, the showcase `walk_to` verb), at a speed the
//!     relay takes, where the rig used to move the game in 40 m teleports the old 100 m rule let
//!     through.

use crate::engine::state::EngineState;
use crate::ship::moves::MoveDecl;
use glam::Vec3;

/// The sentence a correction shows when the relay sends none of its own.
const CORRECTED: &str = "The server put you back where it last saw you: that move was faster than anyone can go aboard.";

/// Seconds between two correction notices on screen.
const NOTICE_GAP_S: f32 = 10.0;

/// One correction the game applied (for the probe).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Correction {
    pub seq: u64,
    /// Where it stood the player (where the relay holds them).
    pub at: Vec3,
    /// Where the player stood when it arrived.
    pub from: Vec3,
    pub reason: String,
    /// The computer's clock when it was applied, ms since 1970.
    pub epoch_ms: f64,
}

/// The rig's scripted walk: to `to` at `speed` m/s, facing `yaw` and `pitch`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ScriptedWalk {
    pub to: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
}

/// What the game keeps about the speed check (`EngineState::moves`).
#[derive(Debug, Default)]
pub(crate) struct ClientMoves {
    /// The fast move the next position update declares (taken by it).
    pub pending: Option<MoveDecl>,
    /// The newest correction applied, echoed in every update as `correction`.
    pub applied: u64,
    /// Corrections applied this session, and the newest one.
    pub count: u64,
    pub last: Option<Correction>,
    /// When a correction notice was last shown.
    pub last_notice: Option<std::time::Instant>,
    /// The rig's walk in progress (the showcase `walk_to` verb).
    pub walk: Option<ScriptedWalk>,
    /// Where the body stands while the follow cam watches a vehicle in the shared world.
    pub follow_body: Option<Vec3>,
}

/// The next position update says the move it reports was `decl` (a teleporter jump, shutting the
/// build editor).
pub(crate) fn declare(state: &mut EngineState, decl: MoveDecl) {
    state.moves.pending = Some(decl);
}

/// The `moved` and `correction` fields of one outgoing update: the declared move (taken), else
/// the vehicle being driven, else none. Pure on the books.
pub(crate) fn stamp_fields(moves: &mut ClientMoves, driving: Option<String>) -> (Option<serde_json::Value>, u64) {
    let decl = moves.pending.take().or(driving.map(|vehicle| MoveDecl::Vehicle { vehicle }));
    (decl.map(|d| d.to_json()), moves.applied)
}

/// Add `moved` and `correction` to an outgoing `game_position_update` (net_route.rs
/// `send_game_position`).
pub(crate) fn stamp(state: &mut EngineState, msg: &mut serde_json::Value) {
    let driving = state
        .driving_vehicle
        .and_then(|veh| state.game_world.world.get::<&crate::ecs::components::Vehicle>(veh).ok().map(|v| v.item_id.clone()));
    let (moved, applied) = stamp_fields(&mut state.moves, driving);
    if let Some(m) = moved {
        msg["moved"] = m;
    }
    msg["correction"] = serde_json::json!(applied);
}

/// A correction as the relay sends it: its number, where to stand, why, and the sentence to show.
pub(crate) fn read_correction(v: &serde_json::Value) -> Option<(u64, Vec3, String, String)> {
    let seq = v.get("seq")?.as_u64()?;
    let a = v.get("position")?.as_array()?;
    let at = Vec3::new(a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32, a.get(2)?.as_f64()? as f32);
    if !at.is_finite() {
        return None;
    }
    let reason = v.get("reason").and_then(|r| r.as_str()).unwrap_or("too_fast").to_string();
    let message = v.get("message").and_then(|m| m.as_str()).unwrap_or(CORRECTED).to_string();
    Some((seq, at, reason, message))
}

/// A `game_position_correction` arrived: stand where the relay holds us, and say so. Taken only
/// while we are in the shared world and aboard, as a welcome is (home_plot.rs `accept_welcome`),
/// and only when it is newer than the last one applied.
pub(crate) fn apply_correction(state: &mut EngineState, v: &serde_json::Value) {
    let Some((seq, at, reason, message)) = read_correction(v) else { return };
    let live = state.game_joined && state.game_welcomed && crate::engine::home_plot::accept_welcome(state.game_joined, state.gui_state.copresence_solo, crate::engine::home_plot::aboard(state));
    if !live || seq <= state.moves.applied {
        return;
    }
    let from = state.camera.position;
    crate::engine::home_plot::put_player_at(state, at);
    // The move it answered was not taken, and the rig's walk stops where the relay holds us.
    state.moves.pending = None;
    state.moves.walk = None;
    state.moves.applied = seq;
    state.moves.count += 1;
    let epoch_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0);
    state.moves.last = Some(Correction { seq, at, from, reason: reason.clone(), epoch_ms });
    log::warn!("Co-presence: the relay corrected our move ({reason}): from {from:?} back to {at:?} (correction {seq})");
    let due = state.moves.last_notice.is_none_or(|t| t.elapsed().as_secs_f32() >= NOTICE_GAP_S);
    if due {
        state.moves.last_notice = Some(std::time::Instant::now());
        state.gui_state.pending_notices.push(message);
    }
}

/// A welcome arrived: a fresh spawn starts the relay's correction count again, so ours starts
/// again too; a reconnect keeps both (the relay forgives a pending one). Nothing declared before
/// it is still true.
pub(crate) fn on_welcome(state: &mut EngineState, v: &serde_json::Value) {
    state.moves.pending = None;
    if v.get("rejoin").and_then(|r| r.as_bool()) != Some(true) {
        state.moves.applied = 0;
    }
}

/// Where this frame's update says we are: the camera, or, while the follow cam watches a vehicle
/// in the shared world, where the body stood when it began (and the camera goes back there the
/// frame it ends). Called once a frame while joined (net_route.rs `drive_position_send`).
pub(crate) fn body_position(state: &mut EngineState) -> Vec3 {
    match (state.follow_vehicle.is_some(), state.moves.follow_body) {
        (true, Some(body)) => body,
        // A follow begun before we joined: the camera is all there is to report.
        (true, None) => state.camera.position,
        (false, Some(body)) => {
            state.moves.follow_body = None;
            crate::engine::home_plot::put_player_at(state, body);
            body
        }
        (false, None) => state.camera.position,
    }
}

/// One frame of the rig's scripted walk: the camera (and the walking body) a step of
/// `speed * dt` toward the target, facing as asked; the walk ends on arrival. `dt` is the
/// movement's own capped step, so a long frame never makes a long stride.
pub(crate) fn walk_tick(state: &mut EngineState, dt: f32) {
    let Some(w) = state.moves.walk else { return };
    let here = state.camera.position;
    let to_go = w.to - here;
    let step = w.speed.max(0.0) * dt.clamp(0.0, 0.1);
    let at = if to_go.length() <= step { w.to } else { here + to_go.normalize_or_zero() * step };
    for (_e, (t, _c)) in state.game_world.world.query_mut::<(&mut crate::ecs::components::Transform, &crate::ecs::components::Controllable)>() {
        t.position = at;
    }
    state.camera.position = at;
    state.camera.yaw = w.yaw;
    state.camera.pitch = w.pitch;
    if at == w.to {
        state.moves.walk = None;
        log::info!("Showcase: walk_to arrived at {at:?}");
    }
}

/// Read the showcase `walk_to` verb: "x,y,z,yaw,pitch,speed" (metres, radians, m/s).
pub(crate) fn parse_walk(spec: &str) -> Option<ScriptedWalk> {
    let v: Vec<f32> = spec.split(',').map(|p| p.trim().parse().ok()).collect::<Option<Vec<f32>>>()?;
    match v.as_slice() {
        [x, y, z, yaw, pitch, speed] if v.iter().all(|f| f.is_finite()) && *speed > 0.0 => {
            Some(ScriptedWalk { to: Vec3::new(*x, *y, *z), yaw: *yaw, pitch: *pitch, speed: *speed })
        }
        _ => None,
    }
}

/// For the rig's probe (engine/ipc.rs): the corrections applied and the walk in progress.
pub(crate) fn probe_json(state: &EngineState) -> serde_json::Value {
    let v = |p: Vec3| serde_json::json!([p.x, p.y, p.z]);
    serde_json::json!({
        "count": state.moves.count,
        "applied": state.moves.applied,
        "last": state.moves.last.as_ref().map(|c| serde_json::json!({
            "seq": c.seq,
            "at": v(c.at),
            "from": v(c.from),
            "reason": c.reason,
            "epoch_ms": c.epoch_ms,
        })),
        "walking": state.moves.walk.is_some(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EVERY UPDATE CARRIES THE NEWEST CORRECTION APPLIED, and a declared move goes out once: the
    /// teleporter jump in the update after it, never again; while driving, every update says so;
    /// with nothing declared and no cab, no `moved` at all. Seen red 2026-10-04 with `stamp_fields`
    /// cloning the pending declaration instead of taking it: "the jump was declared again in the
    /// next update".
    #[test]
    fn a_declared_move_goes_out_once_and_the_correction_count_always() {
        let mut m = ClientMoves { applied: 3, ..Default::default() };
        m.pending = Some(MoveDecl::Editor);
        let (moved, applied) = stamp_fields(&mut m, None);
        assert_eq!(moved, Some(MoveDecl::Editor.to_json()));
        assert_eq!(applied, 3);
        let (again, _) = stamp_fields(&mut m, None);
        assert_eq!(again, None, "the jump was declared again in the next update");
        let (driving, _) = stamp_fields(&mut m, Some("rover_0".into()));
        assert_eq!(driving, Some(MoveDecl::Vehicle { vehicle: "rover_0".into() }.to_json()));
    }

    /// A correction reads, and nonsense does not.
    #[test]
    fn a_correction_reads() {
        let c = serde_json::json!({"type": "game_position_correction", "seq": 2, "position": [76.0, 1.7, 64.0], "reason": "too_fast", "message": "back"});
        let (seq, at, reason, message) = read_correction(&c).expect("reads");
        assert_eq!((seq, at, reason.as_str(), message.as_str()), (2, Vec3::new(76.0, 1.7, 64.0), "too_fast", "back"));
        assert!(read_correction(&serde_json::json!({"seq": 2, "position": [1.0, 2.0]})).is_none());
        assert!(read_correction(&serde_json::json!({"position": [1.0, 2.0, 3.0]})).is_none());
    }

    /// The rig's walk verb: six numbers, a speed above zero.
    #[test]
    fn the_walk_verb_reads_six_numbers() {
        assert_eq!(parse_walk("76,1.7,64,0.5,0,6"), Some(ScriptedWalk { to: Vec3::new(76.0, 1.7, 64.0), yaw: 0.5, pitch: 0.0, speed: 6.0 }));
        assert_eq!(parse_walk("76,1.7,64,0.5,0"), None);
        assert_eq!(parse_walk("76,1.7,64,0.5,0,0"), None, "a walk that never arrives");
        assert_eq!(parse_walk("76,1.7,64,0.5,0,NaN"), None);
    }
}
