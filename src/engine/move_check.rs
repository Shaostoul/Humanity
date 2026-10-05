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
//!     through; facing the way it walks and turning as a person turns (`walk_step`, BUG-165).

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

/// The rig's scripted walk: to `to` at `speed` m/s, ending facing `yaw` and `pitch` (`walk_step`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct ScriptedWalk {
    pub to: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
    /// How fast the camera is turning, left and right and up and down, radians a second.
    pub yaw_rate: f32,
    pub pitch_rate: f32,
    /// Standing at `to`: from now on the walk only turns to `yaw` and `pitch`.
    pub there: bool,
}

/// Where the camera stands and how it looks: before and after one frame of the rig's walk.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WalkPose {
    pub at: Vec3,
    pub yaw: f32,
    pub pitch: f32,
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

impl ClientMoves {
    /// Out of the shared world on our side (engine/home_plot.rs `forget_shared_world`: stepping
    /// out, Respawn, a dropped connection, a switch of server, a refusal). Nothing of that session
    /// holds for the next one (the review of increment 4):
    ///   - where the body stood when a follow cam began (M2): kept, the next session's first
    ///     frame after the follow ended stood the player back there, wherever its welcome had
    ///     stood them;
    ///   - the newest correction applied (M6): it numbers another server's corrections, or the
    ///     relay's books of a session that is over, so the next server's first ones were ignored
    ///     as older. A reconnect to the same relay inside its grace is forgiven there (the relay
    ///     acknowledges its own count at the rejoin), and a fresh join starts its count at 0;
    ///   - a declared move not yet sent, and the rig's walk (planned from where we stood).
    pub(crate) fn forget_session(&mut self) {
        self.follow_body = None;
        self.applied = 0;
        self.pending = None;
        self.walk = None;
    }

    /// True when a correction numbered `seq` is newer than the newest one applied.
    pub(crate) fn takes(&self, seq: u64) -> bool {
        seq > self.applied
    }
}

/// The next position update says the move it reports was `decl` (a teleporter jump, shutting the
/// build editor).
pub(crate) fn declare(state: &mut EngineState, decl: MoveDecl) {
    state.moves.pending = Some(decl);
}

/// Seconds off every pad before the teleporter pads work again after a jump (`teleporter_tick`).
pub(crate) const TELEPORT_REARM_S: f32 = 1.2;

/// One frame of the teleporter pads' re-arm: the cooldown after a jump, and whether a pad may
/// jump the player now. `on_a_pad`: the player stands in a pad's footprint. The cooldown runs
/// down only while the player stands on NO pad (the review of increment 4, M3): the pad a jump
/// lands on is the entry of the way back, and with a plain timer a player standing there was
/// jumped back after 1.2 s, and back again 1.2 s later, for as long as they stood (a page open
/// meanwhile sent no updates, and each jump overwrote the declaration before it).
pub(crate) fn teleport_rearm(cooldown: f32, on_a_pad: bool, dt: f32) -> (f32, bool) {
    let c = if on_a_pad { cooldown } else { (cooldown - dt).max(0.0) };
    (c, c <= 0.0)
}

/// The teleporter pads, once a frame (lib.rs): every zone's teleporters (v0.754), as transit
/// links by id (ship homes increment 4, src/ship/transit.rs). Standing in a pad's footprint, in
/// first person, not building and not in Dev fly mode (flying through a pad must not yank the
/// traveller across the ship), with the pads re-armed (`teleport_rearm`), jumps the player to the
/// partner pad; the next position update declares it (`declare`), so the relay passes the jump
/// instead of correcting it. A jump ends the rig's walk where it lands: that walk was planned
/// from the other side.
pub(crate) fn teleporter_tick(state: &mut EngineState, dt: f32) {
    let p = state.camera.position;
    let link = state.gui_state.ship_structure.as_ref().and_then(|ship| ship.transit_link_at(p));
    let (cooldown, armed) = teleport_rearm(state.teleport_cooldown, link.is_some(), dt);
    state.teleport_cooldown = cooldown;
    let walking_in_person = state.camera.mode == crate::renderer::camera::CameraMode::FirstPerson && !state.gui_state.construction_active && !state.controller.fly_mode;
    let Some(link) = link.filter(|_| armed && walking_in_person) else { return };
    state.camera.position.x = link.to_at.x;
    state.camera.position.z = link.to_at.z;
    state.teleport_cooldown = TELEPORT_REARM_S;
    state.moves.walk = None;
    declare(state, link.declaration());
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
    if !live || !state.moves.takes(seq) {
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
    let (report, stand) = body_report(state.follow_vehicle.is_some(), &mut state.moves.follow_body, state.camera.position);
    if let Some(body) = stand {
        crate::engine::home_plot::put_player_at(state, body);
    }
    report
}

/// `body_position` on the books alone: what this frame's update reports, and where to stand the
/// body first (the frame a follow ends). `following`: the follow cam watches a vehicle;
/// `follow_body`: where the body stood when it began (taken when the follow is over).
pub(crate) fn body_report(following: bool, follow_body: &mut Option<Vec3>, camera: Vec3) -> (Vec3, Option<Vec3>) {
    match (following, *follow_body) {
        (true, Some(body)) => (body, None),
        // A follow begun before we joined: the camera is all there is to report.
        (true, None) => (camera, None),
        (false, Some(body)) => {
            *follow_body = None;
            (body, Some(body))
        }
        (false, None) => (camera, None),
    }
}

/// One frame of the rig's scripted walk (`walk_step`): the camera, and the walking body with it,
/// turned and moved the way a person walks it; the walk ends once it stands at the point looking
/// as asked. `dt` is the movement's own step, capped in `walk_step`, so a long frame never makes
/// a long stride.
pub(crate) fn walk_tick(state: &mut EngineState, dt: f32) {
    let Some(mut w) = state.moves.walk else { return };
    let now = WalkPose { at: state.camera.position, yaw: state.camera.yaw, pitch: state.camera.pitch };
    let (pose, over) = walk_step(&mut w, now, dt);
    for (_e, (t, _c)) in state.game_world.world.query_mut::<(&mut crate::ecs::components::Transform, &crate::ecs::components::Controllable)>() {
        t.position = pose.at;
    }
    state.camera.position = pose.at;
    state.camera.yaw = pose.yaw;
    state.camera.pitch = pose.pitch;
    state.moves.walk = if over { None } else { Some(w) };
    if over {
        log::info!("Showcase: walk_to arrived at {:?}", pose.at);
    }
}

/// Where the rig's walk looks while it walks, radians up: level, the way a person looks where
/// they are going.
const WALK_PITCH: f32 = 0.0;

/// Looking within this of the way it walks (radians, 15 degrees), the rig's walk goes at its
/// whole pace.
const FULL_PACE_OFF: f32 = 15.0 * std::f32::consts::PI / 180.0;

/// Looking this far off the way it walks or more (radians, 60 degrees), it does not walk on at
/// all: it turns where it stands first.
const NO_PACE_OFF: f32 = 60.0 * std::f32::consts::PI / 180.0;

/// The share of its pace a walker keeps while looking `off` radians away from the way it walks:
/// all of it within `FULL_PACE_OFF`, none from `NO_PACE_OFF`, easing in between. A person starts
/// to walk as they come round, and never crabs sideways at full speed.
fn pace(off: f32) -> f32 {
    let t = ((NO_PACE_OFF - off.abs()) / (NO_PACE_OFF - FULL_PACE_OFF)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// One frame of the rig's walk, on the walk and the camera's pose alone: the pose after the
/// frame, and true once the walk is over. The walk the way a person walks it with the mouse and
/// W (BUG-165; the operator watched it walk backwards and never turn, because every frame set
/// the camera to the FINAL facing while it moved):
///   1. turn to face the way to the point, looking level (`turning::LOOK`: at most 150 degrees
///      a second, speeding up into the turn and slowing out of it), walking on as the facing
///      comes round (`pace`); the way is fixed, a straight line, so the route the rig planned
///      through the doors is walked exactly;
///   2. at the point, turn where it stands to the facing asked for;
///   3. only then is the walk over: the probe's `moves.walking`, which the rig waits on, stays
///      true until it is.
/// The step is capped at 0.1 s (the stride rule): a long frame never makes a long stride or a
/// long turn.
pub(crate) fn walk_step(w: &mut ScriptedWalk, now: WalkPose, dt: f32) -> (WalkPose, bool) {
    use crate::turning::{shortest, Turn, LOOK};
    let dt = if dt.is_finite() { dt.clamp(0.0, 0.1) } else { 0.0 };
    let mut yaw = Turn { angle: now.yaw, rate: w.yaw_rate };
    let mut pitch = Turn { angle: now.pitch, rate: w.pitch_rate };
    let mut at = w.to;
    let mut over = false;
    if !w.there {
        let to_go = w.to - now.at;
        // The camera at yaw a looks along (sin a, 0, -cos a) (renderer/camera.rs `forward`).
        let way = if to_go.x.hypot(to_go.z) > 1e-3 { to_go.x.atan2(-to_go.z) } else { now.yaw };
        yaw.toward(way, dt, LOOK);
        pitch.toward(WALK_PITCH, dt, LOOK);
        let step = w.speed.max(0.0) * pace(shortest(yaw.angle, way)) * dt;
        if to_go.length() > step {
            at = now.at + to_go.normalize_or_zero() * step;
        }
        w.there = at == w.to;
    } else {
        let turned = yaw.toward(w.yaw, dt, LOOK);
        let tilted = pitch.toward(w.pitch, dt, LOOK);
        if turned && tilted {
            // Exactly as asked (the turn lands on it; this drops any whole turns the camera's
            // yaw had wound up).
            yaw.angle = w.yaw;
            pitch.angle = w.pitch;
            over = true;
        }
    }
    w.yaw_rate = yaw.rate;
    w.pitch_rate = pitch.rate;
    (WalkPose { at, yaw: yaw.angle, pitch: pitch.angle }, over)
}

/// Read the showcase `walk_to` verb: "x,y,z,yaw,pitch,speed" (metres, radians, m/s).
pub(crate) fn parse_walk(spec: &str) -> Option<ScriptedWalk> {
    let v: Vec<f32> = spec.split(',').map(|p| p.trim().parse().ok()).collect::<Option<Vec<f32>>>()?;
    match v.as_slice() {
        [x, y, z, yaw, pitch, speed] if v.iter().all(|f| f.is_finite()) && *speed > 0.0 => {
            Some(ScriptedWalk { to: Vec3::new(*x, *y, *z), yaw: *yaw, pitch: *pitch, speed: *speed, ..Default::default() })
        }
        _ => None,
    }
}

/// For the rig's probe (engine/ipc.rs): every transit link of the ship as it stands now, each
/// way: its zone, its pads' ids and their floor points in ship metres.
pub(crate) fn transit_probe_json(state: &EngineState) -> serde_json::Value {
    let v = |p: Vec3| serde_json::json!([p.x, p.y, p.z]);
    let links = state.gui_state.ship_structure.as_ref().map(|s| s.transit_links()).unwrap_or_default();
    serde_json::Value::Array(
        links.iter().map(|l| serde_json::json!({ "zone": l.zone, "from": l.from, "to": l.to, "from_at": v(l.from_at), "to_at": v(l.to_at), "reach_m": l.reach_m })).collect(),
    )
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

    /// A correction reads, and nonsense does not. Seen red 2026-10-04 with a missing third number
    /// read as 0: "assertion failed: read_correction(&serde_json::json!({\"seq\": 2, \"position\":
    /// [1.0, 2.0]})).is_none()".
    #[test]
    fn a_correction_reads() {
        let c = serde_json::json!({"type": "game_position_correction", "seq": 2, "position": [76.0, 1.7, 64.0], "reason": "too_fast", "message": "back"});
        let (seq, at, reason, message) = read_correction(&c).expect("reads");
        assert_eq!((seq, at, reason.as_str(), message.as_str()), (2, Vec3::new(76.0, 1.7, 64.0), "too_fast", "back"));
        assert!(read_correction(&serde_json::json!({"seq": 2, "position": [1.0, 2.0]})).is_none());
        assert!(read_correction(&serde_json::json!({"position": [1.0, 2.0, 3.0]})).is_none());
    }

    /// THE FOLLOW CAM IS WATCHING, NOT WALKING: while it watches a vehicle, every update reports
    /// where the body stood when it began, however far the camera hangs behind the vehicle; the
    /// frame it ends, the body is stood back there and that is reported; then the camera again.
    /// A follow begun before joining has no body kept, and reports the camera. (The review of
    /// increment 4, R7: nothing checked `body_position`.) Seen red 2026-10-04 with the first arm
    /// reporting the camera: "while following, the update reported the camera behind the
    /// vehicle".
    #[test]
    fn the_follow_cam_reports_where_the_body_stands() {
        let body = Vec3::new(40.0, 1.7, 30.0);
        let behind_the_vehicle = Vec3::new(-20.0, 6.0, 80.0);
        let mut kept = Some(body);
        assert_eq!(body_report(true, &mut kept, behind_the_vehicle), (body, None), "while following, the update reported the camera behind the vehicle");
        assert_eq!(kept, Some(body));
        assert_eq!(body_report(false, &mut kept, behind_the_vehicle), (body, Some(body)), "the follow ended: the body is stood back where it stood");
        assert_eq!(kept, None);
        assert_eq!(body_report(false, &mut kept, body), (body, None));
        let mut none = None;
        assert_eq!(body_report(true, &mut none, behind_the_vehicle), (behind_the_vehicle, None), "a follow begun before joining");
    }

    /// A FOLLOW FROM AN EARLIER SESSION NEVER STANDS US BACK THERE (the review of increment 4,
    /// M2). The body kept while the follow cam watched a vehicle from B outlived the shared-world
    /// session: Respawn, stepping out to solo, a dropped connection or a switch of server, then a
    /// fresh welcome at the door, and the next frame stood the player back at B (a jump the
    /// relay corrects, or one it passes). Forgetting the shared world forgets it, and the rig's
    /// walk with it.
    ///
    /// Seen red 2026-10-04 on the code before the fix (`forget_session` emptied nothing): "a follow
    /// begun in the last session stood us back at [80, 1.7, 60] after the welcome stood us at
    /// [53.5, 1.7, 40.5]" (left: Some(Vec3(80.0, 1.7, 60.0)), right: None).
    #[test]
    fn a_follow_from_an_earlier_session_never_stands_us_back_there() {
        let b = Vec3::new(80.0, 1.7, 60.0);
        let door = Vec3::new(53.5, 1.7, 40.5);
        let mut m = ClientMoves { follow_body: Some(b), walk: parse_walk("76,1.7,64,0,0,6"), ..Default::default() };
        m.forget_session();
        let (reported, stand) = body_report(false, &mut m.follow_body, door);
        assert_eq!(stand, None, "a follow begun in the last session stood us back at {b} after the welcome stood us at {door}");
        assert_eq!(reported, door);
        assert!(m.walk.is_none(), "the rig's walk outlived the session");
    }

    /// CORRECTIONS FROM ANOTHER SERVER NEVER HIDE THIS ONE'S (the review of increment 4, M6). The
    /// newest correction applied on server A carried over to server B when B's welcome was a
    /// rejoin, so B's first corrections (numbered from 1) were ignored as older and the player
    /// looked frozen there until B's count passed A's. A switch of server forgets the shared
    /// world (home_plot.rs `follow_server`), and with it the count.
    ///
    /// Seen red 2026-10-04 on the code before the fix (`forget_session` emptied nothing): "the next
    /// server's first correction was ignored as older than the last server's fifth".
    #[test]
    fn corrections_from_another_server_never_hide_this_ones() {
        let mut m = ClientMoves { applied: 5, ..Default::default() };
        m.forget_session();
        assert!(m.takes(1), "the next server's first correction was ignored as older than the last server's fifth");
    }

    /// STANDING ON THE PAD YOU LANDED ON NEVER JUMPS YOU BACK (the review of increment 4, M3). The
    /// pad a teleporter lands you on is the entry of the way back, and its cooldown was a plain
    /// timer: standing there 1.2 s jumped you back, and again 1.2 s later, for as long as you
    /// stood (and with a page open, every jump overwrote the one declared before it). Now the
    /// cooldown runs down only while you stand on no pad: step off, wait it out, step on again.
    ///
    /// Seen red 2026-10-04 on the code before the fix (a plain timer): "standing on the pad we
    /// landed on jumped us back after 1.20 s" (left: Some(1.2), right: None).
    #[test]
    fn standing_on_the_pad_you_landed_on_never_jumps_you_back() {
        let dt = 1.0 / 60.0;
        let mut c = TELEPORT_REARM_S;
        let mut fired_at = None;
        for i in 0..300 {
            let (n, may) = teleport_rearm(c, true, dt);
            c = n;
            if may && fired_at.is_none() {
                fired_at = Some(i as f32 * dt);
            }
        }
        assert_eq!(fired_at, None, "standing on the pad we landed on jumped us back after {:.2} s", fired_at.unwrap_or(0.0));
        // Off the pad, the cooldown runs out; then a pad works again.
        let mut off = 0.0;
        loop {
            let (n, may) = teleport_rearm(c, false, dt);
            c = n;
            off += dt;
            if may {
                break;
            }
            assert!(off < 5.0, "the pads never re-armed off the pad");
        }
        assert!((off - TELEPORT_REARM_S).abs() < 0.05, "re-armed {off:.2} s after stepping off");
        assert!(teleport_rearm(0.0, true, dt).1, "a re-armed pad jumps");
    }

    /// The rig's walk verb: six numbers, a speed above zero. Seen red 2026-10-04 with the speed
    /// check taken out: "a walk that never arrives".
    #[test]
    fn the_walk_verb_reads_six_numbers() {
        assert_eq!(parse_walk("76,1.7,64,0.5,0,6"), Some(ScriptedWalk { to: Vec3::new(76.0, 1.7, 64.0), yaw: 0.5, pitch: 0.0, speed: 6.0, ..Default::default() }));
        assert_eq!(parse_walk("76,1.7,64,0.5,0"), None);
        assert_eq!(parse_walk("76,1.7,64,0.5,0,0"), None, "a walk that never arrives");
        assert_eq!(parse_walk("76,1.7,64,0.5,0,NaN"), None);
    }

    // ── The rig's walk, the way a person walks it with the mouse and W (BUG-165) ──

    const DT: f32 = 1.0 / 60.0;

    /// The walk every BUG-165 test takes: from the origin at eye height looking along +x (yaw a
    /// quarter turn) and 17 degrees up, 20 m along +z, where it is asked to end looking back the
    /// way it came (-z, yaw 0) and 23 degrees down. The route heads opposite to the final facing,
    /// the case the operator saw walked backwards.
    const WALK: &str = "0,1.7,20,0,-0.4,6";
    const START: WalkPose = WalkPose { at: Vec3::new(0.0, 1.7, 0.0), yaw: std::f32::consts::FRAC_PI_2, pitch: 0.3 };
    const THERE: Vec3 = Vec3::new(0.0, 1.7, 20.0);

    /// Every frame of `WALK` at 60 frames a second: the pose after it and whether the walk was
    /// over. A minute at most.
    fn walk_frames() -> Vec<(WalkPose, bool)> {
        let mut w = parse_walk(WALK).expect("a walk");
        let mut now = START;
        let mut out = Vec::new();
        for _ in 0..3600 {
            let (p, over) = walk_step(&mut w, now, DT);
            out.push((p, over));
            now = p;
            if over {
                break;
            }
        }
        out
    }

    /// Degrees between where a camera at `yaw` looks across the floor (renderer/camera.rs
    /// `forward_xz`: (sin yaw, 0, -cos yaw)) and the way the walk goes, +z.
    fn off_the_way(yaw: f32) -> f32 {
        Vec3::new(yaw.sin(), 0.0, -yaw.cos()).angle_between(Vec3::Z).to_degrees()
    }

    /// THE RIG FACES THE WAY IT WALKS (BUG-165; the operator, watching the co-presence rig: "the
    /// walk through that you're showing is walking backwards instead of forwards"). Once the
    /// camera has had time to turn (1.5 s; a half turn takes about 1.4 s), and until it reaches
    /// the point, it looks along the way it walks, and level, the way a person walking with the
    /// mouse and W does. It never walks while looking 60 degrees or more away from that way (it
    /// turns where it stands first), and goes at 90% of its pace or more only within 25 degrees
    /// of it (its whole pace within 15, `pace`): a person can walk and turn together, but does
    /// not crab sideways.
    ///
    /// Seen red 2026-10-05 on main (0f8b30944, the camera held the asked facing every frame of
    /// the walk): "0.02 s into the walk it moved 0.100 m while looking 180.0 degrees away from
    /// the way it walked".
    #[test]
    fn the_rig_faces_the_way_it_walks() {
        let mut prev = START;
        for (i, (p, _)) in walk_frames().iter().enumerate() {
            let t = (i + 1) as f32 * DT;
            let moved = (p.at - prev.at).length();
            let off = off_the_way(p.yaw);
            if moved > 0.0 {
                assert!(off < 60.01, "{t:.2} s into the walk it moved {moved:.3} m while looking {off:.1} degrees away from the way it walked");
            }
            if moved >= 0.9 * 6.0 * DT {
                assert!(off <= 25.0, "{t:.2} s into the walk it went at {:.0}% of its pace while looking {off:.1} degrees off the way it walked: a crab, not a walk", 100.0 * moved / (6.0 * DT));
            }
            if t >= 1.5 && p.at != THERE {
                assert!(off < 3.0, "{t:.2} s into the walk, {:.1} m along, the camera looked {off:.1} degrees away from the way it walked", p.at.z);
                assert!(p.pitch.abs() < 0.02, "{t:.2} s into the walk the camera looked {:.1} degrees up or down, not ahead", p.pitch.to_degrees());
            }
            prev = *p;
        }
    }

    /// AT THE POINT IT TURNS TO THE FACING IT WAS ASKED FOR, AND ONLY THEN HAS IT ARRIVED (BUG-165):
    /// the frame the camera reaches the point it still looks the way it walked and the walk goes
    /// on (the probe's `moves.walking`, which the rig waits on, stays true); it turns, a half
    /// turn taking over a second, and the walk is over once it looks exactly as asked.
    ///
    /// Seen red 2026-10-05 on main (0f8b30944): "the walk was over the frame it reached the
    /// point, looking 180.0 degrees away from the way it walked: it never turned there".
    #[test]
    fn the_rig_turns_to_the_asked_facing_only_after_it_arrives() {
        let frames = walk_frames();
        let (end, over) = *frames.last().expect("frames");
        assert!(over, "the walk never ended");
        assert_eq!(end.at, THERE);
        assert!(crate::turning::shortest(end.yaw, 0.0).abs() < 1e-5, "it ended looking along yaw {}, not the asked 0", end.yaw);
        assert!((end.pitch - -0.4).abs() < 1e-5, "it ended looking {} rad up, not the asked -0.4", end.pitch);
        let first = frames.iter().position(|(p, _)| p.at == THERE).expect("it never reached the point");
        let (p, over) = frames[first];
        assert!(
            !over,
            "the walk was over the frame it reached the point, looking {:.1} degrees away from the way it walked: it never turned there",
            off_the_way(p.yaw)
        );
        assert!(off_the_way(p.yaw) < 3.0, "it reached the point looking {:.1} degrees off the way it walked", off_the_way(p.yaw));
        assert!(frames[first..].iter().all(|(q, _)| q.at == THERE), "it moved again after it got there");
        let turning_s = (frames.len() - 1 - first) as f32 * DT;
        assert!(turning_s > 1.0, "it turned half way round in {turning_s:.2} s after arriving: a snap, not a person turning");
    }

    /// THE CAMERA TURNS NO FASTER THAN A PERSON, AND EASES INTO IT (BUG-165): on every frame of
    /// the walk, from the facing it started with to the one it ends with, it turns left or right
    /// and up or down at no more than `turning::LOOK`'s 150 degrees a second, and its first frame
    /// turns no more than the turn's speeding-up allows.
    ///
    /// Seen red 2026-10-05 on main (0f8b30944): "frame 1 turned the camera at 5400 degrees a
    /// second; a person turns at most 150".
    #[test]
    fn the_rig_turns_no_faster_than_a_person() {
        let look = crate::turning::LOOK;
        let mut prev = START;
        for (i, (p, _)) in walk_frames().iter().enumerate() {
            let yaw_rate = crate::turning::shortest(prev.yaw, p.yaw).abs() / DT;
            let pitch_rate = (p.pitch - prev.pitch).abs() / DT;
            for (what, rate) in [("the camera", yaw_rate), ("the camera up or down", pitch_rate)] {
                assert!(rate <= look.rate * 1.0001, "frame {} turned {what} at {:.0} degrees a second; a person turns at most {:.0}", i + 1, rate.to_degrees(), look.rate.to_degrees());
                if i == 0 {
                    assert!(rate <= look.accel * DT * 1.0001, "the first frame turned {what} at {:.0} degrees a second: a turn starts gently", rate.to_degrees());
                }
            }
            prev = *p;
        }
    }

    /// A LONG FRAME NEVER MAKES A LONG STRIDE (the stride rule, kept): a two-second frame moves a
    /// walk already facing its way on by at most a tenth of a second of walking. A guard of the
    /// rule the walk always had, so green before BUG-165 too; a long frame's turn is capped the
    /// same way (turning.rs `a_long_frame_turns_no_further_than_the_cap`).
    #[test]
    fn a_long_frame_never_makes_a_long_stride() {
        let mut w = parse_walk("0,1.7,20,3.14159265,0,6").expect("a walk");
        let facing_the_way = WalkPose { at: Vec3::new(0.0, 1.7, 0.0), yaw: std::f32::consts::PI, pitch: 0.0 };
        let (p, over) = walk_step(&mut w, facing_the_way, 2.0);
        assert!(!over);
        let strode = (p.at - facing_the_way.at).length();
        assert!(strode > 0.0 && strode <= 6.0 * 0.1 + 1e-5, "a two-second frame strode {strode} m");
    }
}
