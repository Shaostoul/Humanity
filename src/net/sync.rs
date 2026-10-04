//! ECS synchronization system: applies network state to the local world.
//!
//! Handles remote player movement (snapshot interpolation, see the long note
//! above `INTERP_DELAY_S`), entity spawn/despawn from server, and decides
//! when the local player's own position goes out, stamped with our steady
//! clock and our real velocity (`PositionSender`; the message itself is
//! built in `engine::net_route::position_update_json`).

use std::collections::VecDeque;

use crate::ecs::components::Transform;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use glam::{Quat, Vec3};
use super::protocol::NetMessage;

/// Component marking an entity as a remote player (not locally controlled).
///
/// Since 2026-10-02 the movement itself lives in the `SnapshotBuffer` that
/// is spawned beside this component. The pose fields below are a read-only
/// mirror of what that buffer drew on the latest frame, kept so anything that
/// wants to inspect another player's motion (a debug overlay, a test) can
/// read it without knowing how the buffer works.
pub struct RemotePlayer {
    pub player_id: u32,
    pub name: String,
    /// How they look, when their client sent it (2026-09-29).
    pub look: Option<crate::player_look::PlayerLook>,
    /// Start of the step the figure is being drawn across: the older of the
    /// two updates the drawn moment falls between.
    pub last_position: Vec3,
    /// End of that step: the newer of the two (the newest update while the
    /// figure is running on past it).
    pub target_position: Vec3,
    pub last_rotation: Quat,
    pub target_rotation: Quat,
    /// The newest real velocity they sent, metres per second.
    pub velocity: Vec3,
    /// How far through that step the figure is drawn, 0 to 1 (1 while it is
    /// running on past the newest update).
    pub interpolation_t: f32,
    /// When their newest update arrived, seconds on this machine's sync clock
    /// (`NetSyncSystem`'s, not theirs).
    pub last_update_time: f64,
}

/// Component marking an entity as a relay-driven crew NPC (v0.663).
/// Spawned/moved by `NetMessage::NpcUpdate` (the relay's `game_npc_update`
/// chore-AI broadcasts). `name` + `activity` carry everything a nameplate
/// needs ("Botanist Yara -- Inspecting the hydroponic racks"); a future
/// nameplate pass should read them from this component (see the machine-label
/// pattern in src/gui/pages/hud.rs for the world_to_screen text path).
/// Where a crew NPC STANDS on this client (floor 0 + 1.0 m body center),
/// regardless of the relay-side deck height its chore site reports.
const NPC_LOCAL_STANDING_Y: f32 = 1.0;

pub struct RemoteNpc {
    pub entity_id: u64,
    pub name: String,
    /// Human-readable current chore label from data/npc/chores.ron.
    pub activity: String,
    /// True while the NPC dwells at its chore site ("working" state).
    pub working: bool,
    /// Crew role id from the relay ("botanist", "navigator", ...). Filled by
    /// the welcome-snapshot NpcProfile (v0.797); empty until it arrives.
    pub role: String,
    /// Walk-up dialogue (v0.797): rotating conversation lines authored by the
    /// relay (its NPC components). Empty vecs = this NPC has nothing to say,
    /// so the talk prompt never offers it. The client only displays these.
    pub dialog: Vec<String>,
    /// Opening lines; the talk card picks one at random when it opens.
    pub greetings: Vec<String>,
    pub last_position: Vec3,
    pub target_position: Vec3,
    pub last_rotation: Quat,
    pub target_rotation: Quat,
    pub interpolation_t: f32,
}

/// Nearest candidate the camera FACES: within [min_dist, max_dist] meters AND
/// inside the look cone (direction-to-target dot camera-forward >= min_dot).
/// Same selection math as the machine / livestock walk-up blocks in lib.rs,
/// extracted pure so the NPC talk targeting is unit-testable. Ties break to
/// the closer candidate.
pub fn nearest_facing_target(
    cam_pos: Vec3,
    cam_forward: Vec3,
    candidates: &[(u64, Vec3)],
    min_dist: f32,
    max_dist: f32,
    min_dot: f32,
) -> Option<u64> {
    let mut best: Option<(u64, f32)> = None;
    for &(id, pos) in candidates {
        let to = pos - cam_pos;
        let dist = to.length();
        if !(min_dist..=max_dist).contains(&dist) {
            continue; // too close to aim at, or out of conversational range
        }
        if (to / dist).dot(cam_forward) < min_dot {
            continue; // outside the look cone (behind / off to the side)
        }
        if best.map_or(true, |b| dist < b.1) {
            best = Some((id, dist));
        }
    }
    best.map(|b| b.0)
}

/// Dialogue cycling for the walk-up talk card (v0.797): the line AFTER `last`,
/// wrapping back to the first once the list is exhausted. `last = None` means
/// "still on the greeting", so the first advance yields index 0. None only
/// for an empty list (nothing to cycle).
pub fn next_dialog_line(lines: &[String], last: Option<usize>) -> Option<(usize, &str)> {
    if lines.is_empty() {
        return None;
    }
    let i = match last {
        None => 0,
        Some(i) => (i + 1) % lines.len(),
    };
    Some((i, lines[i].as_str()))
}

/// Greeting pick for the talk card's opening line. The caller supplies the
/// randomness (any seed -- lib.rs feeds wall-clock nanos) so this stays pure
/// and testable; an out-of-range seed just wraps.
pub fn pick_greeting(lines: &[String], seed: u64) -> Option<&str> {
    if lines.is_empty() {
        return None;
    }
    Some(lines[(seed % lines.len() as u64) as usize].as_str())
}

// ── Smooth movement for other players (snapshot interpolation, 2026-10-02) ──
//
// Another player's client sends where they are about 15 times a second. Those
// messages cross the internet, and the internet does not deliver them evenly:
// one arrives 5 ms after it was sent, the next 30 ms, and now and then one is
// held up for a moment and then several arrive together. The code before this
// walked the figure to each new position over 50 ms with an ease-in-ease-out
// curve and then stood it still until the next message, so other players
// moved in visible stop-go steps and froze whenever a message was late.
//
// What this does instead is the standard technique real-time multiplayer
// games use for other players, "snapshot interpolation":
//
//   1. Every update says when it was sent, read off the SENDER's own steady
//      clock (the `timestamp` field, seconds; `PositionSender` keeps it).
//      Each update that arrives is kept in a short list per player (a
//      "jitter buffer") at that time.
//   2. The two machines' clocks started at different moments, so the buffer
//      learns the difference between them (`offset`: local time minus sender
//      time). It uses the SMALLEST difference seen over the last second,
//      which belongs to the update that crossed the network fastest; every
//      other update was merely held up on the way, so jitter does not move
//      it. When it does change for real, the drawing follows it gently
//      (`OFFSET_SETTLE_S`), so the figure's speed never visibly changes.
//   3. The figure is drawn a fixed moment in the past (`INTERP_DELAY_S`).
//      By then the update AFTER the drawn moment has normally arrived too, so
//      the figure is always between two real updates, and it moves in a
//      straight line between them. Because each update sits at the moment it
//      was really sent, that line is walked at the real speed, however
//      unevenly the updates were sent (a desktop sends every 4 or 5 frames)
//      and however unevenly they arrived.
//   4. If the list runs dry (a message is late), the figure keeps walking
//      along the last real velocity it was sent for a short, capped time
//      (`MAX_EXTRAPOLATION_S`), then stands still.
//   5. When updates resume, any difference between where the figure was
//      guessed to be and where it really was is blended out over about a
//      tenth of a second (`BLEND_BACK_S`) instead of jumping.
//   6. A jump far faster than anyone can move (`TELEPORT_SPEED_MPS`) is a
//      teleport, and the figure snaps to it instead of sliding.
//
// Trusting the sender's clock gives nothing away: only the differences
// between one sender's own stamps are used, and a sender that wanted to
// misplace its figure could simply send a false position instead.
//
// An older client sends `timestamp` 0, which means "no clock". For those the
// buffer falls back to local arrival times, smoothed onto a learned steady
// beat with exactly one beat per update (`stamp_without_clock`). That is the
// first-round design of 2026-10-02, kept only for them: without the sender's
// clock it cannot tell a late update from an unevenly sent one, which is why
// the review of that round measured 2.4 to 6.8 m/s for a 5 m/s walk.
//
// The crew NPC loop further down keeps its own simpler smoothing: the relay
// moves crew about 2 times a second (and on chore changes), so a buffer like
// this would need a delay near a full second, and crew walk slowly between
// fixed chore sites where gliding into each new target already reads well.

/// How far in the past other players are drawn, seconds.
///
/// The desktop sends on the first frame after a fifteenth of a second has
/// passed, so the real spacing between its updates is a whole number of its
/// frames: 67 or 83 ms at 60 frames a second, 67 or 100 ms at 30. For the
/// figure to keep moving between real updates, the next update must arrive
/// within (this delay - the spacing) of the fastest one. 150 ms is two of the
/// desktop's usual spacings (they average 75 ms at 60 frames a second), which
/// leaves 67 to 83 ms of room for lateness (an update held up by most of a
/// whole interval is still in time), and 50 ms even at the longest 100 ms
/// spacing. The first round used 100 ms, only one and a half intervals,
/// which left a mere 17 ms at the 83 ms spacing; its test
/// (`remote_walkers_move_at_a_steady_speed_whatever_rate_they_send_at`) runs
/// dry with it. Longer would hide bigger hiccups but make everyone else lag
/// further behind what they really did; anything longer is bridged by
/// extrapolation.
pub const INTERP_DELAY_S: f64 = 0.150;

/// Without a sender clock only: how often a sender sends its position,
/// seconds (lib.rs sends 15 times a second). Only the starting guess; each
/// buffer measures the real spacing from the arrivals.
const EXPECTED_SEND_INTERVAL_S: f64 = 1.0 / 15.0;
/// Without a sender clock only: the measured spacing is kept inside this
/// range, seconds, so one strange burst of messages cannot teach the buffer a
/// nonsense rate.
const MIN_SEND_INTERVAL_S: f64 = 1.0 / 120.0;
const MAX_SEND_INTERVAL_S: f64 = 0.5;

/// How long a figure keeps walking along its last real velocity once its
/// buffer runs dry, seconds. A quarter second covers a late message or two;
/// past that the guess is more likely wrong than right (they may have stopped
/// or turned), so the figure stands still there until news arrives.
pub const MAX_EXTRAPOLATION_S: f64 = 0.25;

/// A move faster than this between two updates, metres per second, is a
/// teleport, and the figure snaps to it instead of sliding. Walking is
/// 5 m/s and a sprint 9.5 m/s (`CameraController`), so this is over five
/// times a sprint. The sender also reports zero velocity for such a jump
/// (`VelocityMeter`), because a teleport has no speed to carry on with.
pub const TELEPORT_SPEED_MPS: f32 = 50.0;

/// A silence longer than this from one player, seconds, starts their timeline
/// afresh from the next arrival (they left the shared world view, or the link
/// stalled) instead of measuring the next update against the old ones.
const RESYNC_GAP_S: f64 = 1.0;

/// The clock difference (step 2 of the note) is the smallest one seen over
/// this long, seconds. A second holds about 15 updates, so it nearly always
/// includes one that crossed the network without delay; and when the delay
/// really grows (a slower route, or a sender whose frame took over a tenth of
/// a second, so its clock fell behind its wall clock) the old smallest value
/// is gone from the window within a second.
const OFFSET_WINDOW_S: f64 = 1.0;
/// A gap this small between the learned clock difference and the one drawn
/// with is left alone, seconds. The smallest value wanders by a few
/// milliseconds as old updates leave the window; following that would make
/// the speed wobble for no gain.
const OFFSET_DEADBAND_S: f64 = 0.010;
/// How gently the drawing follows a bigger gap: what is left beyond the
/// deadband closes at (that much / this many seconds) per second, so the
/// figure runs that share faster or slower while it does. Startup jitter of
/// up to 45 ms closes at under 4%, which no one can see; a sender's 300 ms
/// hitch closes at the cap below.
const OFFSET_SETTLE_S: f64 = 1.0;
/// The most the drawing may run fast or slow while it follows, as a share of
/// real time.
const OFFSET_MAX_RATE: f64 = 0.25;
/// A learned clock difference this much SMALLER than the one drawn with,
/// seconds, is taken at once instead of eased toward. It means the figure is
/// drawn far further behind than it needs to be (the first update after a
/// long stall set the difference), and catching up a quarter faster would
/// take seconds. Moving the drawn moment forward is safe: the blend in
/// `push` turns it into a quick forward catch-up, never a step back.
const OFFSET_SNAP_S: f64 = 0.25;
/// An update whose clock difference disagrees with the learned one by more
/// than this, seconds, means the sender's clock was reset (they restarted
/// the game): its timeline starts afresh.
const CLOCK_JUMP_S: f64 = 1.0;

/// Without a sender clock only: how the arrival-time smoothing follows the
/// real arrivals (an "alpha-beta filter", a standard tracking filter).
/// ALPHA: the share of each update's lateness or earliness that moves its
/// stamp; small, so one late message barely bends the beat. BETA: the share
/// that corrects the learned send interval; smaller still, so the rate is
/// learned over a second or so.
const TIMELINE_ALPHA: f64 = 0.1;
const TIMELINE_BETA: f64 = 0.005;

/// Without a sender clock only: however the smoothing goes, a stamp stays
/// within this distance of the real arrival, seconds: no more than 50 ms
/// before it (that would eat into the margin `INTERP_DELAY_S` buys) and no
/// more than 100 ms after it (that would add lag).
const MAX_STAMP_EARLY_S: f64 = 0.05;
const MAX_STAMP_LATE_S: f64 = 0.1;

/// When new information moves where a figure should be (updates resuming
/// after a dry spell), the difference is blended out with this time constant,
/// seconds: about 85% of it is gone in two tenths of a second.
const BLEND_BACK_S: f32 = 0.1;

/// Most updates one player's buffer keeps. Normally only two or three are
/// waiting; this only bounds a burst.
const MAX_SNAPSHOTS: usize = 32;

/// A gap longer than this between two of OUR sends, seconds of our own
/// clock, means we were in a menu (sending stops there) or the game hitched;
/// the average movement over it says nothing about how we move now, so that
/// update reports zero velocity.
const MAX_SEND_GAP_S: f64 = 0.5;

/// How often we send our position while in the world, seconds: 15 times a
/// second. lib.rs counts frame time against it, so the real spacing is a
/// whole number of frames (see `INTERP_DELAY_S`).
const SEND_INTERVAL_S: f32 = 1.0 / 15.0;

/// Where `PositionSender`'s clock starts, seconds. Any start works (only
/// differences between stamps are used), but not 0: a `timestamp` of 0 means
/// "no clock", the mark of an older client.
const SENDER_CLOCK_START_S: f64 = 1.0;

/// Whether a received `timestamp` is a sender's clock: 0 (or a missing
/// field, which net_route reads as 0) means an older client without one.
fn has_clock(timestamp: f64) -> bool {
    timestamp.is_finite() && timestamp != 0.0
}

/// One received position update, as kept in a `SnapshotBuffer`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Snapshot {
    /// When it was sent, seconds on the SENDER's clock; it is shown at
    /// `s + offset` on this machine's sync clock. For a sender without a
    /// clock it is the local arrival time smoothed onto a steady beat
    /// (`stamp_without_clock`), and the offset is 0.
    s: f64,
    pos: Vec3,
    rot: Quat,
    /// The real velocity the sender measured, metres per second.
    vel: Vec3,
}

/// What a remote figure was doing on the latest drawn frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionPhase {
    /// The drawn moment is before the oldest update: standing at it. This is
    /// a brand-new figure, or one that just teleported, for one delay.
    Waiting,
    /// Between two real updates: the normal case.
    Interpolating,
    /// Past the newest update (one is late), walking on along its velocity.
    Extrapolating,
    /// Past the newest update for longer than `MAX_EXTRAPOLATION_S`:
    /// standing where the extrapolation stopped until news arrives.
    Holding,
}

/// Where a buffer says a figure should be at one moment, before any blend.
struct Ideal {
    pos: Vec3,
    rot: Quat,
    phase: MotionPhase,
    /// The two updates the moment falls between, and how far between.
    from: (Vec3, Quat),
    to: (Vec3, Quat),
    fraction: f32,
}

/// The jitter buffer for one remote player (component, spawned beside
/// `RemotePlayer`). See the note above `INTERP_DELAY_S` for the technique.
pub struct SnapshotBuffer {
    /// Received updates, oldest first, sender time increasing. Updates the
    /// drawn moment has passed are dropped, except the one it is walking
    /// away from.
    snaps: VecDeque<Snapshot>,
    /// True while this sender stamps its updates with its clock; false for an
    /// older client (timestamp 0), which falls back to arrival times.
    clocked: bool,
    /// Local time minus sender time, seconds, as the figure is drawn with: an
    /// update sent at sender time s is shown at s + offset (sync clock).
    offset: f64,
    /// The recent arrivals, as (local arrival time, arrival minus sender
    /// time), the last `OFFSET_WINDOW_S` of them: `offset` follows the
    /// smallest second value.
    samples: VecDeque<(f64, f64)>,
    /// Without a clock only: the learned time between updates, seconds.
    interval: f64,
    /// When the newest update really arrived, sync clock seconds.
    last_arrival: f64,
    /// Correction still being blended out (`BLEND_BACK_S`): drawn position
    /// minus where the buffer says the figure should be.
    pos_error: Vec3,
    /// The same for facing: drawn rotation = rot_error * buffer rotation.
    rot_error: Quat,
    /// False until the figure has been drawn once. Updates that arrive before
    /// that have nothing on screen to stay continuous with, so they set the
    /// timeline outright: several that arrive together in the first tick put
    /// the figure where it is now, instead of blending it there.
    drawn: bool,
    /// The arrival time (sync clock) of the update the clock difference was
    /// last learned afresh from (the first one, or the first after a long
    /// silence or a clock reset). Others arriving in that same tick were held
    /// up together with it, so a smaller difference among them is taken
    /// outright too.
    learned_at: f64,
    /// What the latest drawn frame was doing.
    pub phase: MotionPhase,
}

impl SnapshotBuffer {
    /// A buffer holding one update, received at `arrival` (sync clock) and
    /// sent at `sender_time` (the message's `timestamp`; 0 for no clock).
    pub fn new(pos: Vec3, rot: Quat, vel: Vec3, arrival: f64, sender_time: f64) -> Self {
        let clocked = has_clock(sender_time);
        let (s, offset) = if clocked { (sender_time, arrival - sender_time) } else { (arrival, 0.0) };
        let mut samples = VecDeque::with_capacity(24);
        if clocked {
            samples.push_back((arrival, offset));
        }
        let mut snaps = VecDeque::with_capacity(8);
        let rot = clean_rotation(rot).unwrap_or(Quat::IDENTITY);
        snaps.push_back(Snapshot { s, pos, rot, vel: clean_velocity(vel) });
        Self {
            snaps,
            clocked,
            offset,
            samples,
            interval: EXPECTED_SEND_INTERVAL_S,
            last_arrival: arrival,
            pos_error: Vec3::ZERO,
            rot_error: Quat::IDENTITY,
            drawn: false,
            learned_at: arrival,
            phase: MotionPhase::Waiting,
        }
    }

    /// Take in an update that arrived at `arrival` (sync clock seconds) and
    /// was sent at `sender_time` (its `timestamp`; 0 for no clock). Called
    /// in the same tick that then draws, so the drawn moment is
    /// `arrival - INTERP_DELAY_S`. Several may arrive in one tick.
    pub fn push(&mut self, pos: Vec3, rot: Quat, vel: Vec3, arrival: f64, sender_time: f64) {
        if !pos.is_finite() {
            return; // a broken message: ignore it rather than draw a figure at NaN
        }
        let rot = clean_rotation(rot).unwrap_or_else(|| self.snaps.back().map_or(Quat::IDENTITY, |n| n.rot));
        let vel = clean_velocity(vel);
        let render_time = arrival - INTERP_DELAY_S;
        // Where the figure would be drawn this frame WITHOUT the new update.
        let before = self.ideal(render_time);
        let s = self.place(arrival, sender_time);
        let snap = Snapshot { s, pos, rot, vel };

        // A teleport: a jump no one could walk, run or fly in the time
        // between the two updates. Snap now, and forget the old path.
        if let Some(prev) = self.snaps.back().copied() {
            let jump = pos.distance(prev.pos) as f64;
            if jump > TELEPORT_SPEED_MPS as f64 * (s - prev.s).max(1e-3) {
                self.snaps.clear();
                self.snaps.push_back(snap);
                self.pos_error = Vec3::ZERO;
                self.rot_error = Quat::IDENTITY;
                return;
            }
        }

        self.snaps.push_back(snap);
        while self.snaps.len() > MAX_SNAPSHOTS {
            self.snaps.pop_front();
        }
        if !self.drawn {
            return; // nothing on screen yet to keep continuous
        }
        // Where the figure should be drawn this frame WITH the new update.
        // These differ when the buffer had run dry (the new update shows the
        // extrapolated guess was off, or ends a hold) or when the timeline was
        // moved (`place`). The difference goes into the blend so the drawn
        // figure does not jump; `advance` then shrinks it away.
        let after = self.ideal(render_time);
        self.pos_error += before.pos - after.pos;
        self.rot_error = (self.rot_error * before.rot * after.rot.inverse()).normalize();
    }

    /// File an update that arrived at `arrival` (sync clock) under the moment
    /// it was sent, keeping the clock difference up to date. Returns that
    /// moment on the sender's clock (or, without one, a smoothed arrival
    /// time).
    fn place(&mut self, arrival: f64, sender_time: f64) -> f64 {
        let gap = arrival - self.last_arrival;
        self.last_arrival = arrival;
        if !has_clock(sender_time) {
            if self.clocked {
                // The sender stopped stamping (an older client took over this
                // id): carry on in local time, keeping where everything is.
                self.rebase(0.0);
                self.clocked = false;
                self.samples.clear();
            }
            return self.stamp_without_clock(arrival, gap);
        }
        // This update's clock difference: the true one plus however long the
        // network held it up.
        let x = arrival - sender_time;
        let newest = self.snaps.back().map(|n| n.s);
        let relearn = !self.clocked // the first stamped update (the buffer began from a join message)
            || gap > RESYNC_GAP_S // a long silence
            || newest.map_or(false, |n| sender_time <= n) // the clock went backwards: restarted
            || (x - self.offset_target()).abs() > CLOCK_JUMP_S; // or jumped
        if relearn {
            // Start the clock difference afresh from this update, keeping
            // the old updates where they were on the local timeline, and
            // drop any that would now come after it.
            self.rebase(x);
            self.clocked = true;
            self.learned_at = arrival;
            self.samples.clear();
            self.samples.push_back((arrival, x));
            while self.snaps.back().map_or(false, |n| n.s >= sender_time) {
                self.snaps.pop_back();
            }
            return sender_time;
        }
        self.samples.push_back((arrival, x));
        while self.samples.len() > 1 && self.samples[0].0 < arrival - OFFSET_WINDOW_S {
            self.samples.pop_front();
        }
        let target = self.offset_target();
        // Take the learned difference outright for the rest of the tick it
        // was learned afresh in: in the buffer's first tick (nothing is on
        // screen yet, and a backlog may arrive all at once) or after a long
        // silence (the burst's first update was the most held up). Otherwise
        // only a big drop is taken at once (`OFFSET_SNAP_S`), and `advance`
        // eases toward everything else.
        if arrival <= self.learned_at || target < self.offset - OFFSET_SNAP_S {
            self.offset = target;
        }
        sender_time
    }

    /// The learned clock difference: the smallest in the window (step 2 of
    /// the note), or the one drawn with if there is nothing to learn from.
    fn offset_target(&self) -> f64 {
        let m = self.samples.iter().map(|&(_, x)| x).fold(f64::INFINITY, f64::min);
        if m.is_finite() {
            m
        } else {
            self.offset
        }
    }

    /// Change the clock difference drawn with to `offset` without moving any
    /// kept update on the local timeline (each keeps the local moment it was
    /// due to be shown at).
    fn rebase(&mut self, offset: f64) {
        let shift = self.offset - offset;
        for snap in self.snaps.iter_mut() {
            snap.s += shift;
        }
        self.offset = offset;
    }

    /// Without a sender clock only: when to show an update that arrived at
    /// `arrival`, `gap` seconds after the previous one: its arrival time with
    /// the network's jitter smoothed onto a learned steady beat.
    ///
    /// Exactly one beat per update, always. The first round counted beats
    /// ("2 when one message in between was lost"), but over a WebSocket a
    /// message is delayed, not lost, and a sender that sends every 4 or 5
    /// frames looked like a 15 Hz sender losing one update in five: the
    /// learned rate locked there and the figure swung between 2.4 and
    /// 6.8 m/s (the 2026-10-02 review).
    fn stamp_without_clock(&mut self, arrival: f64, gap: f64) -> f64 {
        let newest_t = self.snaps.back().map_or(arrival, |n| n.s);
        if gap > RESYNC_GAP_S {
            // After a long silence the old beat says nothing; start from here.
            return arrival.max(newest_t + MIN_SEND_INTERVAL_S);
        }
        let predicted = newest_t + self.interval;
        // How late (+) or early (-) this update is against the beat.
        let residual = arrival - predicted;
        // Several arriving in one tick say nothing about the sender's rate
        // (they were held up together), so they do not teach it.
        if gap >= 0.5 * MIN_SEND_INTERVAL_S {
            self.interval = (self.interval + TIMELINE_BETA * residual).clamp(MIN_SEND_INTERVAL_S, MAX_SEND_INTERVAL_S);
        }
        let t = (predicted + TIMELINE_ALPHA * residual).clamp(arrival - MAX_STAMP_EARLY_S, arrival + MAX_STAMP_LATE_S);
        // Stamps always run forward, so two updates are never shown at once.
        t.max(newest_t + 0.25 * self.interval)
    }

    /// The figure's pose for the moment `render_time` (sync clock seconds),
    /// with `dt` seconds since the last call: eases the clock difference,
    /// shrinks the blend, forgets updates that are no longer needed, and
    /// records the phase.
    fn advance(&mut self, render_time: f64, dt: f32) -> (Vec3, Quat, Ideal) {
        let dt = dt.max(0.0);
        if self.clocked {
            // Ease the clock difference drawn with toward the learned one
            // (step 2 of the note): only the part beyond the deadband, at a
            // rate that grows with it and is capped.
            let err = self.offset_target() - self.offset;
            let excess = err.abs() - OFFSET_DEADBAND_S;
            if excess > 0.0 {
                let rate = (excess / OFFSET_SETTLE_S).min(OFFSET_MAX_RATE);
                self.offset += err.signum() * (rate * dt as f64).min(excess);
            }
        }
        let keep = (-dt / BLEND_BACK_S).exp();
        self.pos_error *= keep;
        self.rot_error = Quat::IDENTITY.slerp(self.rot_error, keep).normalize();
        // Keep the update the drawn moment is walking away from (the start of
        // its step) and everything after it.
        let at = render_time - self.offset;
        while self.snaps.len() >= 2 && self.snaps[1].s <= at {
            self.snaps.pop_front();
        }
        let ideal = self.ideal(render_time);
        self.phase = ideal.phase;
        self.drawn = true;
        (ideal.pos + self.pos_error, (self.rot_error * ideal.rot).normalize(), ideal)
    }

    /// Where the buffer says the figure is at `render_time` (sync clock): at
    /// the oldest update if that is still ahead, on the straight line between
    /// the two updates around it, or walking on past the newest along its
    /// velocity (capped).
    fn ideal(&self, render_time: f64) -> Ideal {
        // The drawn moment on the sender's clock.
        let at = render_time - self.offset;
        let first = self.snaps[0];
        if at <= first.s {
            return Ideal {
                pos: first.pos,
                rot: first.rot,
                phase: MotionPhase::Waiting,
                from: (first.pos, first.rot),
                to: (first.pos, first.rot),
                fraction: 0.0,
            };
        }
        for (a, b) in self.snaps.iter().zip(self.snaps.iter().skip(1)) {
            if at < b.s {
                // Straight line at constant speed: plain linear interpolation,
                // no easing curve (that easing was the old stop-go).
                let f = ((at - a.s) / (b.s - a.s)).clamp(0.0, 1.0) as f32;
                return Ideal {
                    pos: a.pos.lerp(b.pos, f),
                    rot: a.rot.slerp(b.rot, f),
                    phase: MotionPhase::Interpolating,
                    from: (a.pos, a.rot),
                    to: (b.pos, b.rot),
                    fraction: f,
                };
            }
        }
        // Past the newest update: the buffer has run dry.
        let last = *self.snaps.back().expect("a snapshot buffer is never empty");
        let ahead = at - last.s;
        let (phase, run) = if ahead <= MAX_EXTRAPOLATION_S {
            (MotionPhase::Extrapolating, ahead)
        } else {
            (MotionPhase::Holding, MAX_EXTRAPOLATION_S)
        };
        Ideal {
            pos: last.pos + last.vel * run as f32,
            rot: last.rot,
            phase,
            from: (last.pos, last.rot),
            to: (last.pos, last.rot),
            fraction: 1.0,
        }
    }
}

/// A received rotation, unit length, or None if it is broken (NaN or zero).
fn clean_rotation(r: Quat) -> Option<Quat> {
    (r.is_finite() && r.length_squared() > 1e-6).then(|| r.normalize())
}

/// A received velocity, or zero if it is broken or faster than a teleport
/// (an older client, or a bad message, must not fling a figure away).
fn clean_velocity(v: Vec3) -> Vec3 {
    if v.is_finite() && v.length() <= TELEPORT_SPEED_MPS {
        v
    } else {
        Vec3::ZERO
    }
}

/// The sending side's velocity (2026-10-02): how far the local player moved
/// since the previous position update went out, divided by the time since
/// then, in metres per second. Other players' screens use it to keep this
/// player's figure walking when one of these updates is late. Until
/// 2026-10-02 every update said zero.
#[derive(Default)]
pub struct VelocityMeter {
    /// Position and time (seconds, any steady clock) of the previous send.
    last: Option<(Vec3, f64)>,
}

impl VelocityMeter {
    /// Record a send at `position`, time `now_s`, and return the velocity to
    /// put in it. Zero for the first send, after a gap of more than
    /// `MAX_SEND_GAP_S`, and for a teleport-sized jump.
    pub fn sample(&mut self, position: Vec3, now_s: f64) -> Vec3 {
        let Some((p0, t0)) = self.last.replace((position, now_s)) else {
            return Vec3::ZERO;
        };
        let elapsed = now_s - t0;
        if !(elapsed > 1e-4 && elapsed <= MAX_SEND_GAP_S) {
            return Vec3::ZERO;
        }
        let v = (position - p0) / elapsed as f32;
        if !v.is_finite() || v.length() > TELEPORT_SPEED_MPS {
            return Vec3::ZERO;
        }
        v
    }
}

/// One position update for us to send (built into a `game_position_update`
/// message by `engine::net_route::send_game_position`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OutgoingPosition {
    pub position: Vec3,
    /// Facing, radians about the vertical axis (the camera's yaw).
    pub yaw: f32,
    /// Our real velocity, metres per second (`VelocityMeter`).
    pub velocity: Vec3,
    /// Our steady clock when this position held, seconds: the `timestamp`
    /// field (see `PositionSender`).
    pub timestamp: f64,
}

/// The sending side (2026-10-02, round two): decides on each frame whether a
/// position update goes out, and stamps it.
///
/// THE TIMESTAMP: our own steady clock in seconds, the sum of every frame's
/// REAL duration while we are in the shared world (menus included): lib.rs's
/// uncapped frame time, or the movie's fixed step while a clip records.
/// Receivers place our updates by it, so it must keep pace with real time.
/// Round two summed the CAPPED step our movement uses (a frame counts at most
/// a tenth of a second), and the review found the cost (2026-10-03): on any
/// frame over 100 ms the clock fell behind real time, every later update
/// then looked late to the receivers, and a player in a heavy scene on slow
/// hardware (steady 131 ms frames, measured in a real forest) stop-goed on
/// everyone else's screen for good, a slower one froze for seconds after each
/// hitch. Our VELOCITY is still measured on the movement's own capped step
/// (`move_clock`), because that is the speed we really walk at.
/// Receivers use only differences between our stamps, so the start does not
/// matter (`SENDER_CLOCK_START_S`; never 0, which means "no clock").
///
/// THE LAST UPDATE ON LEAVING THE WORLD VIEW: sending stops while a page or
/// the showroom is open, and before round two the last update still carried
/// our walking velocity, so on everyone else's screen our figure walked on
/// a quarter second (1.25 m at a walk, 2.4 m at a sprint) and stood there,
/// even opening doors there, for as long as the menu stayed open. Now the
/// frame we leave, one more update goes out: where we last stood in the
/// world, velocity zero.
pub struct PositionSender {
    /// Our steady clock, seconds: real time (see above).
    clock: f64,
    /// The movement's clock, seconds: the sum of the capped steps our
    /// movement used. Velocity is measured on this one.
    move_clock: f64,
    /// Measures our velocity on the movement's clock.
    meter: VelocityMeter,
    /// The latest frame in the world: where we stood, our facing, and both
    /// clocks then. Taken, and sent standing still, on the frame we leave.
    in_world: Option<(Vec3, f32, f64, f64)>,
    /// The stamp of the newest update sent.
    last_sent: f64,
}

impl Default for PositionSender {
    fn default() -> Self {
        Self {
            clock: SENDER_CLOCK_START_S,
            move_clock: 0.0,
            meter: VelocityMeter::default(),
            in_world: None,
            last_sent: f64::NEG_INFINITY,
        }
    }
}

impl PositionSender {
    /// One frame while we are joined to the shared world: `dt` is the frame's
    /// time step as movement used it (capped), `real_dt` how long the frame
    /// really took (uncapped; the movie's step while recording), `in_world`
    /// whether we are in the world view (no page, showroom or construction
    /// screen open), `position` and `yaw` where we are and face, and `timer`
    /// the real time counted toward the next send (lib.rs keeps it, and zeroes
    /// it on joining). Returns the update to send this frame, if any.
    pub fn frame(
        &mut self,
        dt: f32,
        real_dt: f32,
        in_world: bool,
        position: Vec3,
        yaw: f32,
        timer: &mut f32,
    ) -> Option<OutgoingPosition> {
        let (dt, real_dt) = (dt.max(0.0), real_dt.max(0.0));
        self.clock += real_dt as f64;
        self.move_clock += dt as f64;
        if in_world {
            self.in_world = Some((position, yaw, self.clock, self.move_clock));
            *timer += real_dt;
            if *timer >= SEND_INTERVAL_S {
                *timer = 0.0;
                return Some(self.send(position, yaw, self.clock, self.move_clock, false));
            }
            return None;
        }
        // Out of the world view: one standing-still update on the frame we
        // leave it, then nothing until we are back.
        let (at, face, then, then_moved) = self.in_world.take()?;
        // Stamped when we were last there, unless an update already went out
        // with that stamp (stamps must keep rising: a repeat would read as a
        // restarted clock), in which case now.
        let (stamp, moved) = if then > self.last_sent { (then, then_moved) } else { (self.clock, self.move_clock) };
        Some(self.send(at, face, stamp, moved, true))
    }

    fn send(&mut self, position: Vec3, yaw: f32, timestamp: f64, moved: f64, standing: bool) -> OutgoingPosition {
        let measured = self.meter.sample(position, moved);
        self.last_sent = timestamp;
        let velocity = if standing { Vec3::ZERO } else { measured };
        OutgoingPosition { position, yaw, velocity, timestamp }
    }
}

/// Network synchronization system.
pub struct NetSyncSystem {
    /// This system's own clock, seconds: the sum of every tick's frame time.
    /// Updates are stamped with it when they arrive, and other players are
    /// drawn at `clock - INTERP_DELAY_S` (converted to each sender's clock by
    /// their buffer's learned offset).
    clock: f64,
    /// Decides when our own position goes out, and stamps it.
    sender: PositionSender,
    /// The real duration of the coming tick, set by lib.rs just before it
    /// (`set_clock_step`): the clock above runs on real time, not on the
    /// capped step the System trait passes (2026-10-03, same reason as the
    /// sender's stamps). Taken by the tick; without it the tick's dt is used.
    clock_step: Option<f32>,
    /// Local player ID assigned by server.
    local_player_id: Option<u32>,
    /// Pending messages to process (filled by the engine loop from NetClient::poll).
    pending_messages: Vec<NetMessage>,
}

impl NetSyncSystem {
    pub fn new() -> Self {
        Self {
            clock: 0.0,
            sender: PositionSender::default(),
            clock_step: None,
            local_player_id: None,
            pending_messages: Vec::new(),
        }
    }

    /// One frame of our own position sending while joined to the shared world
    /// (`engine::net_route::drive_position_send`; see `PositionSender::frame`
    /// for the arguments). Returns the update to send this frame, if any.
    pub fn position_to_send(
        &mut self,
        dt: f32,
        real_dt: f32,
        in_world: bool,
        position: Vec3,
        yaw: f32,
        timer: &mut f32,
    ) -> Option<OutgoingPosition> {
        self.sender.frame(dt, real_dt, in_world, position, yaw, timer)
    }

    /// How long the coming tick really lasts, seconds (uncapped; the movie's
    /// step while recording). lib.rs calls this just before ticking, so other
    /// players are drawn on real time even after a frame over 100 ms.
    pub fn set_clock_step(&mut self, real_dt: f32) {
        self.clock_step = Some(real_dt);
    }

    /// Queue messages for processing on next tick.
    /// Called by the engine loop after NetClient::poll().
    pub fn queue_messages(&mut self, messages: Vec<NetMessage>) {
        self.pending_messages.extend(messages);
    }

    /// Set the local player ID (from Welcome message).
    pub fn set_player_id(&mut self, id: u32) {
        self.local_player_id = Some(id);
    }
}

impl System for NetSyncSystem {
    fn name(&self) -> &str {
        "NetSync"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        // Advance the local sync clock first, on real time: every update
        // processed below counts as arriving at this moment.
        let step = self.clock_step.take().unwrap_or(dt);
        self.clock += step.max(0.0) as f64;
        let now = self.clock;
        let messages: Vec<NetMessage> = self.pending_messages.drain(..).collect();

        for msg in messages {
            match msg {
                NetMessage::Welcome { player_id, .. } => {
                    self.local_player_id = Some(player_id);
                    log::info!("Connected as player {}", player_id);
                    // The relay broadcasts our own join + position back to us; if a remote-player
                    // entity was spawned for our own id before the Welcome arrived (a race), drop it.
                    let mut me = Vec::new();
                    for (entity, remote) in world.query_mut::<&RemotePlayer>() {
                        if remote.player_id == player_id {
                            me.push(entity);
                        }
                    }
                    for entity in me {
                        let _ = world.despawn(entity);
                    }
                }

                NetMessage::PlayerJoined { player_id, name, position, look } => {
                    // Never spawn a remote avatar for ourselves (the relay echoes our join).
                    if self.local_player_id == Some(player_id) {
                        continue;
                    }
                    // Idempotent for SPAWNING, but a duplicate join still carries the
                    // real display name -- and that matters (v0.796): position updates
                    // can arrive before the welcome snapshot / joined broadcast (they
                    // stream at 15 Hz while the join is one message), so the player
                    // lazy-spawns below as a "Player N" placeholder and the real name
                    // then bounced off this exists-check FOREVER. Found in the
                    // two-instance co-presence proof: the roster stayed on the
                    // placeholder instead of the game_join player_name.
                    let mut exists = false;
                    for (_e, r) in world.query_mut::<&mut RemotePlayer>() {
                        if r.player_id == player_id {
                            exists = true;
                            if !name.is_empty() && r.name != name {
                                r.name = name.clone();
                            }
                            // Same for the look: a lazy-spawned figure has none.
                            if look.is_some() {
                                r.look = look;
                            }
                            break;
                        }
                    }
                    if exists {
                        continue;
                    }
                    let pos = Vec3::from_array(position);
                    // Spawn a remote player entity, with its jitter buffer
                    // starting from the join position. A join message has no
                    // sender clock (0), so the buffer starts in local time and
                    // learns their clock from their first position update.
                    world.spawn((
                        Transform {
                            position: pos,
                            rotation: Quat::IDENTITY,
                            scale: Vec3::ONE,
                        },
                        RemotePlayer {
                            player_id,
                            name: name.clone(),
                            look,
                            last_position: pos,
                            target_position: pos,
                            last_rotation: Quat::IDENTITY,
                            target_rotation: Quat::IDENTITY,
                            velocity: Vec3::ZERO,
                            interpolation_t: 1.0,
                            last_update_time: now,
                        },
                        SnapshotBuffer::new(pos, Quat::IDENTITY, Vec3::ZERO, now, 0.0),
                    ));
                    log::info!("Player {} ({}) joined", name, player_id);
                }

                NetMessage::PlayerLeft { player_id } => {
                    // Find and despawn the remote player entity
                    let mut to_despawn = Vec::new();
                    for (entity, remote) in world.query_mut::<&RemotePlayer>() {
                        if remote.player_id == player_id {
                            to_despawn.push(entity);
                        }
                    }
                    for entity in to_despawn {
                        let _ = world.despawn(entity);
                    }
                    log::info!("Player {} left", player_id);
                }

                NetMessage::PositionUpdate {
                    player_id,
                    position,
                    rotation,
                    velocity,
                    timestamp,
                } => {
                    // Never track ourselves (the relay echoes our own updates back).
                    if self.local_player_id == Some(player_id) {
                        continue;
                    }
                    let pos = Vec3::from_array(position);
                    let rot = Quat::from_array(rotation);
                    let vel = Vec3::from_array(velocity);
                    // Add the update to the matching remote player's jitter
                    // buffer with both times it carries: when it arrived HERE
                    // (`now`) and when it was sent on the sender's own clock
                    // (`timestamp`, 0 from an older client without one). The
                    // buffer places it by the second and learns how the two
                    // clocks line up from the first (see `SnapshotBuffer`).
                    let existing = world
                        .query_mut::<&RemotePlayer>()
                        .into_iter()
                        .find(|(_, r)| r.player_id == player_id)
                        .map(|(e, _)| e);
                    if let Some(e) = existing {
                        // A RemotePlayer spawned somewhere without a buffer
                        // (tests do this) gets one, starting where it stands.
                        if world.get::<&SnapshotBuffer>(e).is_err() {
                            let (p, r) = world
                                .get::<&Transform>(e)
                                .map(|t| (t.position, t.rotation))
                                .unwrap_or((pos, rot));
                            let _ = world.insert_one(e, SnapshotBuffer::new(p, r, Vec3::ZERO, now, 0.0));
                        }
                        if let Ok((remote, buffer)) =
                            world.query_one_mut::<(&mut RemotePlayer, &mut SnapshotBuffer)>(e)
                        {
                            buffer.push(pos, rot, vel, now, timestamp);
                            remote.velocity = clean_velocity(vel);
                            remote.last_update_time = now;
                        }
                        continue;
                    }
                    // Lazy-spawn: a player already in the world when we joined never sent us a
                    // PlayerJoined (it fired before we connected), so their first position update
                    // is where we first learn of them. Spawn them so co-presence is join-order
                    // independent.
                    let rot = clean_rotation(rot).unwrap_or(Quat::IDENTITY);
                    world.spawn((
                        Transform { position: pos, rotation: rot, scale: Vec3::ONE },
                        RemotePlayer {
                            player_id,
                            name: format!("Player {player_id}"),
                            look: None,
                            last_position: pos,
                            target_position: pos,
                            last_rotation: rot,
                            target_rotation: rot,
                            velocity: clean_velocity(vel),
                            interpolation_t: 1.0,
                            last_update_time: now,
                        },
                        SnapshotBuffer::new(pos, rot, vel, now, timestamp),
                    ));
                }

                NetMessage::NpcUpdate { entity_id, name, position, activity, working } => {
                    // Update-or-spawn the crew NPC. Updates arrive at ~2 Hz
                    // while traveling (plus on every chore state change), so
                    // interpolation below smooths movement between them.
                    //
                    // GROUND the Y to the local floor (v0.681, operator screenshot
                    // 2026-07-03): the relay simulates chores on ITS multi-deck ship
                    // layout, so upper-deck room sites carry high Y values -- but this
                    // client renders the flat homestead, so crew showed up floating
                    // mid-sky. Keep the relay X/Z walk, override Y with the local
                    // standing height. Since ship homes increment 3 the relay's
                    // world IS the ship the game draws (the crew work the Commons
                    // and its mess hall, in the same frame, on its one deck at
                    // y 0), so X/Z and this Y agree; the override stays until the
                    // ship has a second deck.
                    let pos = Vec3::new(position[0], NPC_LOCAL_STANDING_Y, position[2]);
                    let mut found = false;
                    for (_e, (transform, npc)) in
                        world.query_mut::<(&mut Transform, &mut RemoteNpc)>()
                    {
                        if npc.entity_id == entity_id {
                            // Face the direction of travel (yaw only) when moving.
                            let delta = pos - transform.position;
                            if delta.length_squared() > 0.0001 {
                                npc.last_rotation = transform.rotation;
                                npc.target_rotation = Quat::from_rotation_y(delta.x.atan2(delta.z));
                            }
                            npc.last_position = transform.position;
                            npc.target_position = pos;
                            npc.activity = activity.clone();
                            npc.working = working;
                            npc.interpolation_t = 0.0;
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        world.spawn((
                            Transform { position: pos, rotation: Quat::IDENTITY, scale: Vec3::ONE },
                            RemoteNpc {
                                entity_id,
                                name: name.clone(),
                                activity: activity.clone(),
                                working,
                                // Dialogue arrives via NpcProfile (welcome snapshot);
                                // an update-first spawn starts silent and the profile
                                // fills it in when it lands (v0.797).
                                role: String::new(),
                                dialog: Vec::new(),
                                greetings: Vec::new(),
                                last_position: pos,
                                target_position: pos,
                                last_rotation: Quat::IDENTITY,
                                target_rotation: Quat::IDENTITY,
                                interpolation_t: 1.0,
                            },
                        ));
                        log::info!("Crew NPC {} ({}) appeared: {}", name, entity_id, activity);
                    }
                }

                NetMessage::NpcProfile {
                    entity_id,
                    name,
                    role,
                    position,
                    activity,
                    dialog,
                    greetings,
                } => {
                    // Welcome-snapshot crew capture (v0.797): the relay's welcome
                    // carries every crew NPC with the dialog[]/greetings[] its
                    // components author server-side. Update-or-spawn, mirroring
                    // NpcUpdate:
                    //   (a) spawn -- a crew member DWELLING at a chore site sends
                    //       no NpcUpdate until its next move, so before v0.797 a
                    //       fresh joiner could not see (or talk to) it at all;
                    //   (b) update -- if the 2 Hz stream spawned the NPC first,
                    //       the profile just fills in the dialogue lines.
                    // Same local-floor Y grounding as NpcUpdate (relay decks vs
                    // the flat homestead, v0.681).
                    let pos = Vec3::new(position[0], NPC_LOCAL_STANDING_Y, position[2]);
                    let mut found = false;
                    for (_e, npc) in world.query_mut::<&mut RemoteNpc>() {
                        if npc.entity_id == entity_id {
                            npc.role = role.clone();
                            npc.dialog = dialog.clone();
                            npc.greetings = greetings.clone();
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        world.spawn((
                            Transform { position: pos, rotation: Quat::IDENTITY, scale: Vec3::ONE },
                            RemoteNpc {
                                entity_id,
                                name: name.clone(),
                                activity: activity.clone(),
                                // Snapshot has no chore state; the next NpcUpdate
                                // corrects it (false = the muted nameplate style).
                                working: false,
                                role: role.clone(),
                                dialog: dialog.clone(),
                                greetings: greetings.clone(),
                                last_position: pos,
                                target_position: pos,
                                last_rotation: Quat::IDENTITY,
                                target_rotation: Quat::IDENTITY,
                                interpolation_t: 1.0,
                            },
                        ));
                        log::info!(
                            "Crew NPC {} ({}) loaded from welcome snapshot ({} dialog lines)",
                            name,
                            entity_id,
                            dialog.len()
                        );
                    }
                }

                NetMessage::TimeSync { game_time, .. } => {
                    log::debug!("Time sync: game_time={}", game_time);
                }

                // Another player or a crew member went out of our view (ship homes increment 4,
                // the relay's game_interest.rs `game_out_of_view`): nothing more about them comes
                // until they are in view again, so they are taken off the screen rather than left
                // standing where they were last seen. A player's id is their entity id.
                NetMessage::EntityDespawn { entity_id } => {
                    let mut gone = Vec::new();
                    for (e, r) in world.query_mut::<&RemotePlayer>() {
                        if u64::from(r.player_id) == entity_id {
                            gone.push(e);
                        }
                    }
                    for (e, n) in world.query_mut::<&RemoteNpc>() {
                        if n.entity_id == entity_id {
                            gone.push(e);
                        }
                    }
                    for e in gone {
                        let _ = world.despawn(e);
                    }
                }

                _ => {}
            }
        }

        // Draw every remote player where their jitter buffer says they were
        // INTERP_DELAY_S ago (snapshot interpolation, see the note above
        // INTERP_DELAY_S), and mirror the step being drawn into RemotePlayer.
        let render_time = now - INTERP_DELAY_S;
        for (_entity, (transform, remote, buffer)) in
            world.query_mut::<(&mut Transform, &mut RemotePlayer, &mut SnapshotBuffer)>()
        {
            let (pos, rot, ideal) = buffer.advance(render_time, dt);
            transform.position = pos;
            transform.rotation = rot;
            remote.last_position = ideal.from.0;
            remote.last_rotation = ideal.from.1;
            remote.target_position = ideal.to.0;
            remote.target_rotation = ideal.to.1;
            remote.interpolation_t = ideal.fraction;
        }

        // Interpolate crew NPC positions. Updates arrive at ~2 Hz (see
        // NPC_POSITION_BROADCAST_INTERVAL relay-side), so complete the lerp
        // in 0.5s. No dead reckoning: crew walk slowly and stop at chore
        // sites, so holding the last target beats drifting past it.
        for (_entity, (transform, npc)) in
            world.query_mut::<(&mut Transform, &mut RemoteNpc)>()
        {
            if npc.interpolation_t < 1.0 {
                npc.interpolation_t += dt * 2.0;
                npc.interpolation_t = npc.interpolation_t.min(1.0);
                let t = smooth_step(npc.interpolation_t);
                transform.position = npc.last_position.lerp(npc.target_position, t);
                transform.rotation = npc.last_rotation.slerp(npc.target_rotation, t);
            }
        }
    }
}

/// Smooth step interpolation (ease in-out). Used by the crew NPC loop only:
/// other players move in straight lines between updates (`SnapshotBuffer`),
/// because easing every step is what made them stop and go.
fn smooth_step(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::systems::System;

    /// The lazy-spawn name race from the two-instance co-presence proof
    /// (v0.796): a position update arriving BEFORE the join/welcome spawns
    /// the player as a "Player N" placeholder; the real name in the later
    /// PlayerJoined must UPDATE it, not bounce off the exists-check.
    #[test]
    fn a_late_join_message_fixes_a_lazy_spawned_placeholder_name() {
        let mut sys = NetSyncSystem::new();
        let mut world = hecs::World::new();
        let data = crate::hot_reload::data_store::DataStore::new();
        sys.queue_messages(vec![
            NetMessage::Welcome { player_id: 1, world_snapshot: Vec::new() },
            // 15 Hz position stream outruns the one-shot join broadcast.
            NetMessage::PositionUpdate {
                player_id: 42,
                position: [1.0, 0.0, 2.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                velocity: [0.0, 0.0, 0.0],
                timestamp: 0.0,
            },
        ]);
        sys.tick(&mut world, 0.016, &data);
        let placeholder: Vec<String> = world
            .query_mut::<&RemotePlayer>()
            .into_iter()
            .map(|(_, r)| r.name.clone())
            .collect();
        assert_eq!(placeholder, vec!["Player 42".to_string()], "lazy-spawn placeholder");

        let look = crate::player_look::PlayerLook { skin: [0.7, 0.5, 0.4], hair: [0.1, 0.1, 0.1], height: 1.05 };
        sys.queue_messages(vec![NetMessage::PlayerJoined {
            player_id: 42,
            name: "Test Pilot A".to_string(),
            position: [1.0, 0.0, 2.0],
            look: Some(look),
        }]);
        sys.tick(&mut world, 0.016, &data);
        let named: Vec<(String, u32)> = world
            .query_mut::<&RemotePlayer>()
            .into_iter()
            .map(|(_, r)| (r.name.clone(), r.player_id))
            .collect();
        assert_eq!(named.len(), 1, "no duplicate spawn from the late join");
        assert_eq!(named[0].0, "Test Pilot A", "the real name replaced the placeholder");
        // And the look arrived with the join (2026-09-29).
        let looks: Vec<Option<crate::player_look::PlayerLook>> =
            world.query_mut::<&RemotePlayer>().into_iter().map(|(_, r)| r.look).collect();
        assert_eq!(looks, vec![Some(look)], "the look replaced the placeholder's none");
        // A fresh join (no earlier position update) spawns with its look.
        sys.queue_messages(vec![NetMessage::PlayerJoined {
            player_id: 43,
            name: "Test Pilot B".to_string(),
            position: [0.0, 0.0, 0.0],
            look: Some(look),
        }]);
        sys.tick(&mut world, 0.016, &data);
        let b = world.query_mut::<&RemotePlayer>().into_iter().find(|(_, r)| r.player_id == 43).map(|(_, r)| r.look);
        assert_eq!(b, Some(Some(look)), "a fresh join spawns with its look");
    }

    // ── Smooth movement for other players (snapshot interpolation, 2026-10-02) ──
    //
    // These tests play a remote player walking past at a steady pace, the way
    // the network really delivers it (irregular lateness, a held-up burst, a
    // lost message), into a real NetSyncSystem ticking at 60 frames a second,
    // and check where the figure is drawn on every frame. Since round two
    // every update carries its sender's clock, stamped the way
    // `PositionSender` stamps it.

    /// Walking speed in these tests, m/s: the game's walk (CameraController 5.0).
    const WALK: f32 = 5.0;
    /// The regular test sender's beat: 15 updates a second.
    const SEND_DT: f64 = 1.0 / 15.0;
    /// The receiver's frame time: 60 frames a second.
    const FRAME_DT: f32 = 1.0 / 60.0;
    /// Every update spends 50 ms on the wire plus this much irregular extra,
    /// seconds (a fixed list, so every run is identical): up to 28 ms, which
    /// is ordinary home-internet jitter.
    const JITTER: [f64; 11] = [0.004, 0.021, 0.0, 0.013, 0.028, 0.007, 0.017, 0.002, 0.025, 0.011, 0.019];
    /// How far a frame's speed may stray from WALK in the steady-speed checks,
    /// as a share: 4%. With the sender's clock, the only thing that changes
    /// the drawn speed is the drawing easing toward the learned clock
    /// difference, at (gap - OFFSET_DEADBAND_S) per OFFSET_SETTLE_S. In these
    /// deliveries the first update is at most 28 ms of jitter plus one 17 ms
    /// receiver frame later than the fastest, so while it settles the drawing
    /// runs at most 3.5% fast, and exactly at walking pace once settled.
    /// Measured with the fix: 0.7% at worst (5.033 m/s). Round one, at the
    /// desktop's real send pattern, was 35 to 50% off.
    const STEADY_BAND: f32 = 0.04;

    /// One update as the network delivers it.
    #[derive(Clone, Copy)]
    struct Delivery {
        arrival: f64,
        /// The sender's clock when it was sent (0 = an older client, no clock).
        stamp: f64,
        pos: Vec3,
        vel: Vec3,
    }

    /// A player walking along +X at WALK on a perfectly regular 15 Hz beat,
    /// sending `count` updates: update k leaves at k * SEND_DT (stamped with
    /// that on a sender clock that started at 1 s, as `PositionSender`'s
    /// does) and arrives 50 ms + JITTER later. Updates whose number is in
    /// `lost` never arrive.
    fn walker(count: usize, lost: &[usize]) -> Vec<Delivery> {
        (0..count)
            .filter(|k| !lost.contains(k))
            .map(|k| {
                let sent = k as f64 * SEND_DT;
                Delivery {
                    arrival: sent + 0.05 + JITTER[k % JITTER.len()],
                    stamp: SENDER_CLOCK_START_S + sent,
                    pos: Vec3::new(WALK * sent as f32, 0.0, 0.0),
                    vel: Vec3::new(WALK, 0.0, 0.0),
                }
            })
            .collect()
    }

    /// Frame times for a sender's frame loop at `fps` for `seconds`, each off
    /// by up to 0.3 ms either way, the way real frame times wobble (a fixed-seed
    /// generator, so every run is identical).
    fn frame_times(fps: f64, seconds: f64, seed: u64) -> Vec<f32> {
        let mut state = seed;
        (0..(fps * seconds) as usize)
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let unit = (state >> 33) as f64 / (1u64 << 31) as f64; // 0 to 1
                (1.0 / fps + (unit - 0.5) * 0.0006) as f32
            })
            .collect()
    }

    /// A player walking along +X at WALK through a real `PositionSender`,
    /// driven the way lib.rs drives it: each frame (times from `frame_dts`)
    /// they move WALK * dt and the sender decides whether an update goes out.
    /// Each update arrives 50 ms + JITTER after its frame. Returns the
    /// deliveries and, for each, how many of the sender's frames passed since
    /// the one before (since the start, for the first).
    fn sender_walk(frame_dts: &[f32]) -> (Vec<Delivery>, Vec<usize>) {
        let mut sender = PositionSender::default();
        let mut timer = 0.0_f32;
        let (mut pos, mut real, mut since) = (Vec3::ZERO, 0.0_f64, 0_usize);
        let (mut out, mut spacing) = (Vec::new(), Vec::new());
        for &dt in frame_dts {
            real += dt as f64;
            pos.x += WALK * dt;
            since += 1;
            if let Some(u) = sender.frame(dt, dt, true, pos, 0.0, &mut timer) {
                let arrival = real + 0.05 + JITTER[out.len() % JITTER.len()];
                out.push(Delivery { arrival, stamp: u.timestamp, pos: u.position, vel: u.velocity });
                spacing.push(since);
                since = 0;
            }
        }
        (out, spacing)
    }

    /// Where the remote figure was drawn on one frame.
    struct Frame {
        t: f64,
        pos: Vec3,
        phase: MotionPhase,
    }

    /// Run a NetSyncSystem at 60 fps for `seconds` from time `start`, handing
    /// it each update on the first frame at or after its arrival (the way the
    /// engine loop polls the socket once a frame), and record where player 42
    /// is drawn.
    fn play_from(deliveries: &[Delivery], start: f64, seconds: f64) -> Vec<Frame> {
        let mut sys = NetSyncSystem::new();
        let mut world = hecs::World::new();
        let data = crate::hot_reload::data_store::DataStore::new();
        let mut next = 0;
        let mut frames = Vec::new();
        let count = (seconds / FRAME_DT as f64).ceil() as usize;
        for n in 1..=count {
            let t = start + n as f64 * FRAME_DT as f64;
            let mut msgs = Vec::new();
            while next < deliveries.len() && deliveries[next].arrival <= t {
                let d = deliveries[next];
                msgs.push(NetMessage::PositionUpdate {
                    player_id: 42,
                    position: d.pos.to_array(),
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    velocity: d.vel.to_array(),
                    timestamp: d.stamp,
                });
                next += 1;
            }
            sys.queue_messages(msgs);
            sys.tick(&mut world, FRAME_DT, &data);
            if let Some((_, (tr, _, buf))) = world
                .query_mut::<(&Transform, &RemotePlayer, &SnapshotBuffer)>()
                .into_iter()
                .find(|(_, (_, r, _))| r.player_id == 42)
            {
                frames.push(Frame { t, pos: tr.position, phase: buf.phase });
            }
        }
        frames
    }

    fn play(deliveries: &[Delivery], seconds: f64) -> Vec<Frame> {
        play_from(deliveries, 0.0, seconds)
    }

    /// Distance moved along the walk (+X) between each pair of frames whose
    /// later frame lies in [from, to], paired with that later frame.
    fn steps<'a>(frames: &'a [Frame], from: f64, to: f64) -> Vec<(f32, &'a Frame)> {
        frames
            .windows(2)
            .filter(|w| w[1].t >= from && w[1].t <= to)
            .map(|w| (w[1].pos.x - w[0].pos.x, &w[1]))
            .collect()
    }

    /// Every frame in [from, to] moves forward along the walk at WALK to
    /// within `band` (a share of WALK), and stays on the line.
    fn assert_steady(frames: &[Frame], from: f64, to: f64, band: f32, label: &str) {
        let s = steps(frames, from, to);
        assert!(s.len() > 30, "{label}: enough frames checked: {}", s.len());
        for (dx, f) in &s {
            assert!(*dx >= -1e-5, "{label}: frame at {:.3} s moved backwards by {dx} m ({:?})", f.t, f.phase);
            let speed = dx / FRAME_DT;
            assert!(
                (speed - WALK).abs() <= band * WALK,
                "{label}: frame at {:.3} s moved at {speed:.3} m/s, not {WALK} m/s within {:.0}% ({:?})",
                f.t,
                band * 100.0,
                f.phase
            );
            assert!(f.pos.y.abs() < 1e-5 && f.pos.z.abs() < 1e-5, "{label}: stays on the line: {:?}", f.pos);
        }
    }

    /// OTHER PLAYERS WALK AT A STEADY SPEED WHATEVER RATE THEY SEND AT
    /// (2026-10-02, round two). Three real senders (`PositionSender`, driven
    /// the way lib.rs drives it), each walking at 5 m/s: one running at 12
    /// frames a second (it sends every frame, 83 ms apart); one at 60 frames a
    /// second whose frame times wobble the way real ones do, so it sends after
    /// 4 or 5 frames (67 or 83 ms) in no fixed order, the desktop's real
    /// pattern; and one at 30 frames a second (after 2 or 3 frames, 67 or
    /// 100 ms). Updates arrive with irregular lateness. On every frame the
    /// figure moves forward at walking pace to within STEADY_BAND (4%, the
    /// reason is there), and it is always between two real updates: at these
    /// rates the buffer never runs dry, which is what INTERP_DELAY_S buys.
    ///
    /// Red checks, run 2026-10-03: (1) ignoring the sender's clock
    /// (`has_clock` always false, so updates are placed by arrival time) FAILS
    /// with "12 frames a second: frame at 0.550 s moved at 4.698 m/s, not 5
    /// m/s within 4% (Interpolating)"; (2) INTERP_DELAY_S back at round one's
    /// 100 ms FAILS with "30 frames a second: the buffer ran dry at 0.933 s".
    /// With the fix every checked frame is between 5.000 and 5.033 m/s.
    #[test]
    fn remote_walkers_move_at_a_steady_speed_whatever_rate_they_send_at() {
        let senders: [(&str, Vec<f32>, &[usize]); 3] = [
            ("12 frames a second", frame_times(12.0, 8.0, 1), &[1]),
            ("60 frames a second", frame_times(60.0, 8.0, 2), &[4, 5]),
            ("30 frames a second", frame_times(30.0, 8.0, 3), &[2, 3]),
        ];
        for (label, frame_dts, pattern) in senders {
            let (stream, spacing) = sender_walk(&frame_dts);
            // The sender really sends in that pattern, every variant of it.
            for &n in &spacing[1..] {
                assert!(pattern.contains(&n), "{label}: sent after {n} frames, expected one of {pattern:?}");
            }
            for &n in pattern {
                assert!(spacing[1..].contains(&n), "{label}: never sent after {n} frames");
            }
            let frames = play(&stream, 8.0);
            let (from, to) = (stream[0].arrival + 0.4, stream.last().unwrap().arrival);
            assert_steady(&frames, from, to, STEADY_BAND, label);
            for (_, f) in steps(&frames, from, to) {
                assert_eq!(f.phase, MotionPhase::Interpolating, "{label}: the buffer ran dry at {:.3} s", f.t);
            }
        }
    }

    /// A SHORT STALL NEITHER STEPS BACK NOR RUSHES (2026-10-02, round two).
    /// Over a WebSocket a late message holds up the ones behind it, then they
    /// all arrive together. A desktop-pattern sender (60 frames a second, 4 or
    /// 5 frames between updates) walks at 5 m/s; from update 20 on, everything
    /// is held for 150, 200 or 250 ms and released in one burst. The buffer
    /// runs dry during the hold (the figure walks on along the last velocity)
    /// and when the burst lands, each update goes where its stamp says: no
    /// frame moves backwards, and every frame stays at walking pace within
    /// STEADY_BAND.
    ///
    /// Red checks, run 2026-10-03: (1) placing updates by arrival time instead
    /// of the sender's clock (`has_clock` always false) FAILS with "150 ms
    /// stall: frame at 0.533 s moved at 4.623 m/s, not 5 m/s within 4%
    /// (Interpolating)": arrival times miss the band even before the stall.
    /// At the stall itself, the same placement steps 3 to 7 frames backwards
    /// and then rushes at up to 16.7 m/s (250 ms) or 22.8 m/s (300 ms), in a
    /// scratch copy of this buffer driven the same way; round one's beat
    /// counting stepped 4 to 8 frames backwards, then rushed at up to
    /// 19.7 m/s (the review's harness). (2) Extrapolating with zero velocity
    /// FAILS with "150 ms stall: frame at 1.750 s moved at 3.587 m/s, not
    /// 5 m/s within 4% (Extrapolating)", which shows the hold really empties
    /// the buffer.
    #[test]
    fn a_short_stall_then_a_burst_neither_steps_back_nor_rushes() {
        for stall in [0.15, 0.2, 0.25] {
            let label = format!("{:.0} ms stall", stall * 1000.0);
            let (mut stream, _) = sender_walk(&frame_times(60.0, 6.0, 7));
            let release = stream[20].arrival + stall;
            let mut held = 0;
            for d in stream.iter_mut().skip(20) {
                if d.arrival <= release {
                    d.arrival = release;
                    held += 1;
                }
            }
            assert!(held >= 2, "{label}: the burst holds two updates or more: {held}");
            let frames = play(&stream, 6.0);
            let dry = frames
                .iter()
                .filter(|f| f.t > stream[19].arrival && f.t < release + INTERP_DELAY_S)
                .any(|f| f.phase == MotionPhase::Extrapolating);
            assert!(dry, "{label}: the hold ran the buffer dry");
            assert_steady(&frames, stream[0].arrival + 0.4, stream.last().unwrap().arrival, STEADY_BAND, &label);
        }
    }

    /// A BACKLOG TAKEN IN ALL AT ONCE DOES NOT DISTURB LATER MOTION
    /// (2026-10-02, round two). The relay sends position updates to every
    /// socket, and before round two net_route queued them even while we sat
    /// on a menu, not yet joined, so the first tick after joining took the
    /// whole lot in at once (net_route::position_update_from now drops them;
    /// its own test covers that). This is the buffer's half of the fix: ten
    /// seconds of a walker's updates handed over in ONE tick, then the live
    /// stream. The figure starts where the walker was a moment ago, not ten
    /// seconds back, never moves backwards, and from the very first frame
    /// walks at walking pace within BACKLOG_BAND (12%), then within
    /// STEADY_BAND once settled. The wider band at first: the newest
    /// update in the backlog may have waited up to a whole send interval
    /// plus jitter (83 + 28 ms) beyond its natural arrival, so the clock
    /// difference first learned from it can be about 110 ms more than the
    /// real one, and the drawing then eases that away at up to (110 - 10) ms
    /// per second: 10% fast at the start (measured: 3.9%), decaying with a
    /// one-second time constant, so two seconds later it is under 2%.
    ///
    /// Red checks, run 2026-10-03: (1) without taking the learned clock
    /// difference outright for the rest of the buffer's first tick (no
    /// `arrival <= self.learned_at ||` in `place`, so only drops of more than
    /// OFFSET_SNAP_S jump) FAILS with "first drawn at x = 47.99, where the
    /// walker was a moment ago is about 49": a fifth of a second further
    /// behind than needed, to be eased back over seconds; (2) blending
    /// updates that arrive before the first draw (no `if !self.drawn {
    /// return; }` in `push`) FAILS with "first drawn at x = 7.85, where the
    /// walker was a moment ago is about 49": the figure starts 41 m back and
    /// slides. Round one, given the same backlog, swung between 4 and 10 m/s
    /// for the rest of the session (the review's harness).
    #[test]
    fn a_backlog_taken_in_one_tick_does_not_disturb_later_motion() {
        const JOIN: f64 = 10.0;
        let (mut stream, _) = sender_walk(&frame_times(60.0, JOIN + 6.0, 11));
        let backlog = stream.iter().filter(|d| d.arrival <= JOIN).count();
        assert!(backlog > 100, "ten seconds were queued: {backlog} updates");
        for d in stream.iter_mut() {
            d.arrival = d.arrival.max(JOIN);
        }
        let frames = play_from(&stream, JOIN, 5.5);
        // Drawn INTERP_DELAY_S behind the newest update, which left about
        // 50 ms before it arrived.
        let expected = WALK * (JOIN - 0.05 - INTERP_DELAY_S) as f32;
        let first = frames[0].pos.x;
        assert!((first - expected).abs() < 0.5, "first drawn at x = {first:.2}, where the walker was a moment ago is about {expected:.0}");
        // See the note above the test for why 12% at first.
        const BACKLOG_BAND: f32 = 0.12;
        assert_steady(&frames, JOIN, JOIN + 2.0, BACKLOG_BAND, "just after the backlog");
        assert_steady(&frames, JOIN + 2.0, stream.last().unwrap().arrival, STEADY_BAND, "once settled");
    }

    /// LEAVING THE WORLD VIEW LEAVES THE FIGURE WHERE WE STOPPED (2026-10-02,
    /// round two). A player walks at 5 m/s, then opens a page, and sending
    /// stops while it is open. Before round two their last update still said
    /// "walking at 5 m/s", so on everyone else's screen the figure walked on
    /// a quarter second (1.25 m) past where they stopped and stood there,
    /// opening doors there, for as long as the page stayed open. Now
    /// `PositionSender` sends one more update on the frame they leave: where
    /// they last stood in the world, velocity zero. Checked on the receiving
    /// screen first (the figure ends exactly where they stopped and never
    /// passes it), then in the updates themselves.
    ///
    /// Red check, run 2026-10-03: without the standing update (`frame`
    /// returning None out of the world view, as before) FAILS with "the
    /// figure walked on to x = 6.250 past where they stopped (5.000)": the
    /// whole quarter second, 1.25 m.
    #[test]
    fn leaving_the_world_view_leaves_the_figure_where_we_stopped() {
        let mut sender = PositionSender::default();
        let mut timer = 0.0_f32;
        let (mut pos, mut real) = (Vec3::ZERO, 0.0_f64);
        let mut sent: Vec<(f64, OutgoingPosition)> = Vec::new(); // (when, update)
        // One second walking in the world, then two on a page (standing), at
        // 60 frames a second.
        for n in 0..180 {
            real += FRAME_DT as f64;
            let in_world = n < 60;
            if in_world {
                pos.x += WALK * FRAME_DT;
            }
            if let Some(u) = sender.frame(FRAME_DT, FRAME_DT, in_world, pos, 0.3, &mut timer) {
                sent.push((real, u));
            }
        }
        let stop = pos;
        let stream: Vec<Delivery> = sent
            .iter()
            .enumerate()
            .map(|(k, (when, u))| Delivery {
                arrival: when + 0.05 + JITTER[k % JITTER.len()],
                stamp: u.timestamp,
                pos: u.position,
                vel: u.velocity,
            })
            .collect();
        let frames = play(&stream, 3.0);
        let furthest = frames.iter().map(|f| f.pos.x).fold(f32::MIN, f32::max);
        assert!(furthest <= stop.x + 1e-3, "the figure walked on to x = {furthest:.3} past where they stopped ({:.3})", stop.x);
        let last = frames.last().unwrap();
        assert!((last.pos.x - stop.x).abs() < 1e-3, "and stands where they stopped: {} vs {}", last.pos.x, stop.x);
        // The updates: walking until we left, then exactly one standing
        // update, then nothing while the page is open.
        let left_at = 60.0 * FRAME_DT as f64 + 1e-9;
        let after: Vec<OutgoingPosition> = sent.iter().filter(|(when, _)| *when > left_at).map(|(_, u)| *u).collect();
        assert_eq!(after.len(), 1, "exactly one update after leaving the world view: {after:?}");
        let standing = after[0];
        assert_eq!(standing.velocity, Vec3::ZERO, "it stands still");
        assert_eq!((standing.position, standing.yaw), (stop, 0.3), "where and facing as we last stood in the world");
        let walking = sent.iter().filter(|(when, _)| *when <= left_at).last().unwrap().1;
        assert!(walking.velocity.x > 0.99 * WALK, "the update before it was still walking: {:?}", walking.velocity);
        assert!(standing.timestamp > walking.timestamp, "its stamp comes after: {} vs {}", standing.timestamp, walking.timestamp);
        // And back in the world, sending resumes.
        let resumed = (0..30).filter(|_| sender.frame(FRAME_DT, FRAME_DT, true, pos, 0.3, &mut timer).is_some()).count();
        assert!(resumed >= 6, "sending resumes in the world: {resumed} in half a second");
    }

    /// OUR STAMPS RUN ON REAL TIME, OUR VELOCITY ON THE MOVEMENT'S STEP
    /// (2026-10-03, the round-two review). lib.rs caps a frame's step at a
    /// tenth of a second, so after a 250 ms frame we moved a tenth of a
    /// second's worth. The stamp must still count the real 250 ms (receivers
    /// place updates by it against real time), and the velocity must still
    /// say 5 m/s (it is the speed we walk at). A walker at 60 frames a second
    /// hits one such frame: every stamp is the sum of the REAL frame times,
    /// and every update after the first says 5 m/s.
    ///
    /// Red check, run 2026-10-03: stamping on the capped step (round two's
    /// contract) FAILS with "update 5 stamped 1.4333333522081375, not real
    /// time 1.5833333507180214" (the figures below are round two's record).
    ///
    /// Round two's red check, run 2026-10-03: running the clock on wall time instead (as
    /// round one measured velocity, on time since an epoch) FAILS with
    /// "update 0 stamped 506.41698384284973, not the movement's clock
    /// 1.0666666701436043". The same break shows the velocity half too: the
    /// test's frames take microseconds of wall time, so every velocity reads
    /// zero, and `leaving_the_world_view...` FAILS with "the update before it
    /// was still walking: Vec3(0.0, 0.0, 0.0)".
    #[test]
    fn our_stamps_run_on_real_time_and_velocity_on_the_movement_step() {
        let mut sender = PositionSender::default();
        let mut timer = 0.0_f32;
        let mut pos = Vec3::new(2.0, 1.7, -3.0);
        let mut real = SENDER_CLOCK_START_S;
        let mut sends = 0;
        let frame_steps = std::iter::repeat(FRAME_DT).take(20).chain([0.25]).chain(std::iter::repeat(FRAME_DT).take(20));
        for real_dt in frame_steps {
            let dt = real_dt.min(0.1); // lib.rs's cap
            pos.x += WALK * dt;
            real += real_dt as f64;
            if let Some(u) = sender.frame(dt, real_dt, true, pos, 0.0, &mut timer) {
                if sends > 0 {
                    let v = u.velocity;
                    assert!((v - Vec3::new(WALK, 0.0, 0.0)).length() < 1e-3, "update {sends} says {v:?}, not walking +X at 5 m/s");
                }
                assert!((u.timestamp - real).abs() < 1e-6, "update {sends} stamped {}, not real time {real}", u.timestamp);
                sends += 1;
            }
        }
        assert!(sends >= 9, "enough updates: {sends}");
    }

    /// A SLOW SENDER STILL WALKS SMOOTHLY ON EVERYONE ELSE'S SCREEN
    /// (2026-10-03, the round-two review). A player on slow hardware in a
    /// heavy scene runs at a steady 131 ms a frame (measured in a real
    /// forest). lib.rs caps each movement step at 0.1 s, so they really cover
    /// 5 m/s x 0.1 = 0.5 m per 131 ms, 3.82 m/s. On a 60 fps receiver their
    /// figure must always be between two real updates (never run dry, the
    /// failure this exists for), never move backwards, and keep their speed:
    /// 95% of frames within 2%, and none off by more than 20%. The few
    /// exceptions are single frames where the buffer re-learns the clock
    /// offset between such sparse updates (measured 2026-10-03: 4.1 to 4.4
    /// m/s for one frame, a correction of about 1 cm, then back on pace).
    ///
    /// Red check, run 2026-10-03: stamping on the capped step instead of real
    /// time (`self.clock += dt`) FAILS with "the buffer ran dry at 0.700 s":
    /// the stamps fall 31 ms behind real time every frame.
    #[test]
    fn a_slow_sender_still_walks_smoothly_on_everyone_elses_screen() {
        let mut sender = PositionSender::default();
        let mut timer = 0.0_f32;
        let (mut pos, mut real) = (Vec3::ZERO, 0.0_f64);
        let mut stream = Vec::new();
        for _ in 0..61 {
            let (real_dt, dt) = (0.131_f32, 0.1_f32);
            real += real_dt as f64;
            pos.x += WALK * dt;
            if let Some(u) = sender.frame(dt, real_dt, true, pos, 0.0, &mut timer) {
                let arrival = real + 0.05 + JITTER[stream.len() % JITTER.len()];
                stream.push(Delivery { arrival, stamp: u.timestamp, pos: u.position, vel: u.velocity });
            }
        }
        let frames = play(&stream, 8.0);
        let speed = WALK * 0.1 / 0.131;
        let (from, to) = (stream[0].arrival + 0.5, stream.last().unwrap().arrival);
        let s = steps(&frames, from, to);
        assert!(s.len() > 100, "enough frames checked: {}", s.len());
        let mut close = 0;
        for (dx, f) in &s {
            assert_eq!(f.phase, MotionPhase::Interpolating, "the buffer ran dry at {:.3} s", f.t);
            assert!(*dx >= -1e-5, "frame at {:.3} s moved backwards by {dx} m", f.t);
            let v = dx / FRAME_DT;
            assert!((v - speed).abs() <= 0.2 * speed, "frame at {:.3} s moved at {v:.3} m/s, not {speed:.3}", f.t);
            if (v - speed).abs() <= 0.02 * speed {
                close += 1;
            }
        }
        assert!(close * 100 >= s.len() * 95, "only {close} of {} frames within 2% of {speed:.3} m/s", s.len());
    }

    /// OUR OWN LONG FRAME DOES NOT MAKE OTHER PLAYERS RUSH (2026-10-03, the
    /// round-two review). The receiver's clock runs on real time too
    /// (`set_clock_step`): after one 300 ms frame of ours, a 5 m/s walker is
    /// drawn at 5 m/s again on the very next frames, not hurried along for a
    /// second while a clock that only counted 0.1 s of it catches up.
    ///
    /// Red check, run 2026-10-03: ignoring the real step in `tick` FAILS with
    /// "after our long frame: frame at 3.317 s moved at 5.619 m/s, not 5 m/s
    /// within 4% (Interpolating)".
    #[test]
    fn our_own_long_frame_does_not_make_other_players_rush() {
        let (stream, _) = sender_walk(&frame_times(60.0, 8.0, 4));
        let mut sys = NetSyncSystem::new();
        let mut world = hecs::World::new();
        let data = crate::hot_reload::data_store::DataStore::new();
        let (mut t, mut next) = (0.0_f64, 0);
        let mut frames = Vec::new();
        while t < 6.0 {
            // One 300 ms frame at 3 s; every other frame at 60 fps.
            let real_dt = if (3.0..3.0 + FRAME_DT as f64).contains(&t) { 0.3_f32 } else { FRAME_DT };
            t += real_dt as f64;
            let mut msgs = Vec::new();
            while next < stream.len() && stream[next].arrival <= t {
                let d = stream[next];
                msgs.push(NetMessage::PositionUpdate {
                    player_id: 42,
                    position: d.pos.to_array(),
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    velocity: d.vel.to_array(),
                    timestamp: d.stamp,
                });
                next += 1;
            }
            sys.queue_messages(msgs);
            sys.set_clock_step(real_dt);
            sys.tick(&mut world, real_dt.min(0.1), &data);
            if let Some((_, (tr, _, buf))) = world
                .query_mut::<(&Transform, &RemotePlayer, &SnapshotBuffer)>()
                .into_iter()
                .find(|(_, (_, r, _))| r.player_id == 42)
            {
                frames.push(Frame { t, pos: tr.position, phase: buf.phase });
            }
        }
        let hitch = frames.iter().position(|f| f.t > 3.2).expect("frames after the hitch");
        assert_steady(&frames[hitch..], frames[hitch].t + FRAME_DT as f64, 5.0, STEADY_BAND, "after our long frame");
    }

    /// AN OLDER CLIENT WITHOUT A CLOCK STILL MOVES SENSIBLY (2026-10-02, round
    /// two). A `timestamp` of 0 means "no clock", and the buffer then places
    /// updates by arrival time on a learned beat, exactly one beat per update.
    /// Without the sender's clock it cannot be exact (that is why the clock
    /// exists), but at the desktop's real 4-or-5-frame pattern the figure must
    /// still never move backwards and stay within 20% of walking pace.
    ///
    /// Red check, run 2026-10-03: putting round one's beat counting back
    /// (`steps` = the rounded number of beats since the last update, "2 when
    /// one in between was lost") FAILS with "no clock: frame at 1.133 s moved
    /// at 6.228 m/s, not 5 m/s within 20% (Interpolating)". With the fix this
    /// run stays between 4.27 and 5.89 m/s.
    #[test]
    fn an_older_sender_without_a_clock_still_moves_sensibly() {
        let (mut stream, _) = sender_walk(&frame_times(60.0, 8.0, 5));
        for d in stream.iter_mut() {
            d.stamp = 0.0;
        }
        let frames = play(&stream, 8.0);
        assert_steady(&frames, stream[0].arrival + 1.0, stream.last().unwrap().arrival, 0.20, "no clock");
    }

    /// OTHER PLAYERS WALK AT A STEADY SPEED, NOT STOP-GO (2026-10-02). A
    /// player walking at 5 m/s, updates arriving with irregular lateness and
    /// one lost, drawn at 60 fps: the figure never moves backwards and every
    /// frame it covers walking speed to within STEADY_BAND.
    ///
    /// Red checks: round one (2026-10-02, then with a 15% band) put the old
    /// 50 ms smooth_step loop back and this FAILED with "frame at 0.483 s
    /// moved at 2.59 m/s, not about 5 m/s". Round two (stamped updates, the
    /// 4% band, run 2026-10-03): ignoring the sender's clock (`has_clock`
    /// always false) FAILS with "lost update: frame at 1.500 s moved at
    /// 6.203 m/s, not 5 m/s within 4% (Interpolating)"; extrapolating with
    /// zero velocity FAILS with "lost update: frame at 1.483 s moved at
    /// 3.772 m/s, not 5 m/s within 4% (Interpolating)".
    #[test]
    fn a_remote_walker_moves_at_walking_speed_through_jitter_and_a_lost_update() {
        let stream = walker(45, &[20]);
        let frames = play(&stream, 3.5);
        // From once the figure is under way until the last update arrives
        // (after that it extrapolates and stops, which is tested separately).
        assert_steady(&frames, stream[0].arrival + 0.4, stream.last().unwrap().arrival, STEADY_BAND, "lost update");
    }

    /// A LATE UPDATE IS BRIDGED, THEN CAUGHT UP WITHOUT A JUMP (2026-10-02).
    /// Updates 20 and 21 never arrive (round one lost only 20; round two's
    /// 150 ms delay bridges a single missing update without running dry, so it
    /// takes two). The buffer runs dry for a few frames: the figure must keep
    /// walking along the last real velocity there (not freeze), and the frame
    /// where real updates take over again must not jump.
    ///
    /// Red check, run in both rounds: extrapolating with zero velocity instead
    /// of the last real one (a hold) FAILS; round two's message (2026-10-03)
    /// is "Extrapolating frame at 1.483 s moved at 3.54 m/s": the figure
    /// stops at the last update partway through that frame. (Ignoring the
    /// sender's clock also fails it: "Interpolating frame at 1.533 s moved
    /// at 5.79 m/s".)
    #[test]
    fn a_lost_update_is_bridged_by_extrapolation_then_caught_up_without_a_jump() {
        let stream = walker(45, &[20, 21]);
        let frames = play(&stream, 3.5);
        let s = steps(&frames, stream[0].arrival + 0.4, stream.last().unwrap().arrival);
        let dry: Vec<usize> = (0..s.len()).filter(|&i| s[i].1.phase == MotionPhase::Extrapolating).collect();
        assert!(!dry.is_empty(), "the lost updates must make the buffer run dry at least one frame");
        // Every dry frame is around the loss: after update 19 arrived, before
        // update 22 did plus the drawing delay.
        let (u19, u22) = (stream[19].arrival, stream[20].arrival);
        for &i in &dry {
            let t = s[i].1.t;
            assert!(t > u19 && t < u22 + INTERP_DELAY_S, "dry only around the loss, not at {t:.3} s");
        }
        // Walking on through the dry frames, and the first frame after them
        // (real updates again) continues at walking pace: no freeze, no jump.
        let resume = dry.last().unwrap() + 1;
        for &i in dry.iter().chain(std::iter::once(&resume)) {
            let speed = s[i].0 / FRAME_DT;
            assert!(
                (speed - WALK).abs() <= STEADY_BAND * WALK,
                "{:?} frame at {:.3} s moved at {speed:.2} m/s",
                s[i].1.phase,
                s[i].1.t
            );
        }
        assert_eq!(s[resume].1.phase, MotionPhase::Interpolating, "real updates took over again");
    }

    /// EXTRAPOLATION IS CAPPED, AND A LONG GAP BLENDS BACK (2026-10-02).
    /// A full second of updates is lost (20 to 34). The figure walks on for
    /// at most MAX_EXTRAPOLATION_S past the last update and then stands still;
    /// when updates resume it is far behind, and it catches up over several
    /// frames instead of jumping in one.
    ///
    /// Red checks, run in both rounds (round two's messages, 2026-10-03):
    /// (1) without the cap (extrapolating as long as the gap lasts) this FAILS
    /// with "walked on too far in the gap: 7.6098 > 7.584333 at 1.733 s";
    /// (2) without the blend (no `pos_error` carried over in `push`) it FAILS
    /// with "a frame jumped 3.325715 m while catching up". With both, the
    /// biggest catch-up frame is about 0.52 m (the one-second gap leaves the
    /// figure over 3 m behind).
    #[test]
    fn extrapolation_stops_after_a_quarter_second_and_a_long_gap_blends_back() {
        let lost: Vec<usize> = (20..35).collect();
        let stream = walker(60, &lost);
        let frames = play(&stream, 4.5);
        let last_before = stream[19]; // update 19, the last before the gap
        let resumed = stream[20].arrival; // update 35 arrives
        let limit = last_before.pos.x + WALK * MAX_EXTRAPOLATION_S as f32 + 1e-3;
        let in_gap: Vec<&Frame> = frames.iter().filter(|f| f.t > last_before.arrival && f.t < resumed).collect();
        for f in &in_gap {
            assert!(f.pos.x <= limit, "walked on too far in the gap: {} > {limit} at {:.3} s", f.pos.x, f.t);
        }
        assert!(in_gap.iter().any(|f| f.phase == MotionPhase::Holding), "stands still once the cap is reached");
        // Catch-up: the figure is over 3 m behind when update 35 arrives. No
        // single frame may cover even a third of that.
        let catch_up = steps(&frames, resumed, resumed + 0.6);
        let biggest = catch_up.iter().map(|(dx, _)| *dx).fold(0.0_f32, f32::max);
        assert!(biggest < 1.0, "a frame jumped {biggest} m while catching up");
        assert!(catch_up.iter().all(|(dx, _)| *dx >= -1e-5), "never moves backwards while catching up");
        // And it is back to walking pace once caught up.
        for (dx, f) in steps(&frames, resumed + 0.6, stream.last().unwrap().arrival) {
            let speed = dx / FRAME_DT;
            assert!((speed - WALK).abs() <= 0.15 * WALK, "after catching up, frame at {:.3} s moved at {speed:.2} m/s", f.t);
        }
    }

    /// A TELEPORT SNAPS INSTEAD OF SLIDING (2026-10-02). From update 20 on,
    /// the walker is 40 m further along (a respawn, a teleporter). The figure
    /// must cross those 40 m in exactly one frame, the frame that update
    /// arrives, and never be drawn anywhere in between.
    ///
    /// Red check, run in both rounds: without the teleport branch in `push`
    /// the jump is interpolated like a step, and this FAILS with "exactly one
    /// frame crosses the jump": in round two (2026-10-03) five frames did
    /// (3.0, 10.1, 10.1, 10.1 and 7.1 m), the figure sliding across the 40 m.
    #[test]
    fn a_teleport_snaps_instead_of_sliding() {
        let mut stream = walker(40, &[]);
        for (k, d) in stream.iter_mut().enumerate() {
            if k >= 20 {
                d.pos.x += 40.0;
            }
            if k == 20 {
                d.vel = Vec3::ZERO; // the sender reports no speed for a jump
            }
        }
        let frames = play(&stream, 3.0);
        let big: Vec<(f32, &Frame)> = steps(&frames, 0.0, 10.0).into_iter().filter(|(dx, _)| dx.abs() > 2.0).collect();
        assert_eq!(big.len(), 1, "exactly one frame crosses the jump: {:?}", big.iter().map(|(dx, f)| (*dx, f.t)).collect::<Vec<_>>());
        let (_, snap) = big[0];
        let arrived = stream[20].arrival;
        assert!(snap.t >= arrived && snap.t < arrived + FRAME_DT as f64 + 1e-9, "it snaps the frame the update arrives ({:.3} s, arrived {arrived:.3} s)", snap.t);
        assert!((snap.pos - stream[20].pos).length() < 1e-4, "lands exactly on the new place: {:?}", snap.pos);
        let (old_end, new_start) = (stream[19].pos.x + 1.0, stream[20].pos.x - 1.0);
        for f in &frames {
            assert!(f.pos.x <= old_end || f.pos.x >= new_start, "drawn in between the two places at {:.3} s: {:?}", f.t, f.pos);
        }
    }

    /// THE SENDER REPORTS ITS REAL VELOCITY (2026-10-02): movement since the
    /// previous send over the time since then, m/s, for any spacing; zero
    /// for the first send, after a pause, and for a teleport-sized jump.
    ///
    /// Red check, run 2026-10-02: returning zero always (what every update
    /// said before) FAILS with "walking +X at 5 m/s, got Vec3(0.0, 0.0, 0.0)".
    #[test]
    fn the_sender_measures_its_real_velocity() {
        let mut m = VelocityMeter::default();
        let close = |a: Vec3, b: Vec3| (a - b).length() < 1e-3;
        assert_eq!(m.sample(Vec3::new(0.0, 1.7, 0.0), 10.0), Vec3::ZERO, "nothing to measure from yet");
        // 4 frames at 60 fps, walking 5 m/s along +X.
        let v = m.sample(Vec3::new(1.0 / 3.0, 1.7, 0.0), 10.0 + 4.0 / 60.0);
        assert!(close(v, Vec3::new(5.0, 0.0, 0.0)), "walking +X at 5 m/s, got {v:?}");
        // An irregular gap (5 frames), now sprinting along -Z at 9.5 m/s.
        let v = m.sample(Vec3::new(1.0 / 3.0, 1.7, -9.5 * 5.0 / 60.0), 10.0 + 9.0 / 60.0);
        assert!(close(v, Vec3::new(0.0, 0.0, -9.5)), "sprinting -Z at 9.5 m/s, got {v:?}");
        // A 40 m jump in 67 ms is a teleport, not a speed.
        let v = m.sample(Vec3::new(40.0, 1.7, 0.0), 10.0 + 13.0 / 60.0);
        assert_eq!(v, Vec3::ZERO, "a teleport has no velocity");
        // A pause (a menu was open) says nothing about how we move now...
        let v = m.sample(Vec3::new(41.0, 1.7, 0.0), 12.0);
        assert_eq!(v, Vec3::ZERO, "after a 1.8 s pause");
        // ...and the next normal send measures again.
        let v = m.sample(Vec3::new(41.25, 1.7, 0.0), 12.05);
        assert!(close(v, Vec3::new(5.0, 0.0, 0.0)), "measuring again, got {v:?}");
    }

    // ── NPC walk-up talk helpers (v0.797) ──

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn facing_selection_picks_the_nearest_in_cone_npc_only() {
        let cam = Vec3::new(0.0, 1.7, 0.0);
        let fwd = Vec3::new(0.0, 0.0, 1.0); // looking down +Z
        let candidates = vec![
            (1_u64, Vec3::new(0.0, 1.6, 2.0)),  // dead ahead, 2 m -- the pick
            (2_u64, Vec3::new(0.0, 1.6, 2.4)),  // dead ahead but farther
            (3_u64, Vec3::new(0.0, 1.6, -1.5)), // BEHIND the camera
            (4_u64, Vec3::new(3.0, 1.6, 0.5)),  // in range, far outside the cone
            (5_u64, Vec3::new(0.0, 1.6, 9.0)),  // ahead, out of talk range
        ];
        assert_eq!(
            nearest_facing_target(cam, fwd, &candidates, 0.3, 2.5, 0.8),
            Some(1),
            "nearest in-cone in-range candidate wins"
        );
        // Nobody in range/cone -> no target (prompt stays empty).
        assert_eq!(
            nearest_facing_target(cam, fwd, &candidates[2..4], 0.3, 2.5, 0.8),
            None
        );
        // Standing INSIDE a candidate (below min_dist) does not target it:
        // the min bound guards the divide-by-distance and point-blank jitter.
        assert_eq!(
            nearest_facing_target(cam, fwd, &[(9, cam + Vec3::new(0.0, 0.0, 0.01))], 0.3, 2.5, 0.8),
            None
        );
    }

    #[test]
    fn dialog_cycling_wraps_and_greeting_pick_is_seed_stable() {
        let d = lines(&["a", "b", "c"]);
        // None = "still on the greeting" -> first advance is line 0, then 1, 2, wrap to 0.
        assert_eq!(next_dialog_line(&d, None), Some((0, "a")));
        assert_eq!(next_dialog_line(&d, Some(0)), Some((1, "b")));
        assert_eq!(next_dialog_line(&d, Some(1)), Some((2, "c")));
        assert_eq!(next_dialog_line(&d, Some(2)), Some((0, "a")), "wraps to the start");
        assert_eq!(next_dialog_line(&[], None), None, "nothing to say");

        let g = lines(&["hi", "yo"]);
        assert_eq!(pick_greeting(&g, 0), Some("hi"));
        assert_eq!(pick_greeting(&g, 1), Some("yo"));
        assert_eq!(pick_greeting(&g, 7), Some("yo"), "seed wraps, never panics");
        assert_eq!(pick_greeting(&[], 3), None);
    }

    /// Dialogue must survive BOTH arrival orders: profile-then-update (the
    /// normal welcome-first case) and update-then-profile (a chore broadcast
    /// racing the snapshot parse). Either way the RemoteNpc ends up with the
    /// relay's lines and exactly one entity exists per entity_id.
    #[test]
    fn npc_profile_and_update_merge_in_either_order() {
        let profile = |eid: u64| NetMessage::NpcProfile {
            entity_id: eid,
            name: "Botanist Yara".to_string(),
            role: "botanist".to_string(),
            position: [4.0, 7.0, 2.0], // relay deck Y is IGNORED (local grounding)
            activity: "Tending the racks".to_string(),
            dialog: lines(&["Look at these tomatoes."]),
            greetings: lines(&["Welcome! Mind the spore filter."]),
        };
        let update = |eid: u64| NetMessage::NpcUpdate {
            entity_id: eid,
            name: "Botanist Yara".to_string(),
            position: [5.0, 7.0, 2.0],
            activity: "Walking to hydroponics".to_string(),
            working: false,
        };
        let data = crate::hot_reload::data_store::DataStore::new();

        for (label, msgs) in [
            ("profile first", vec![profile(7), update(7)]),
            ("update first", vec![update(7), profile(7)]),
        ] {
            let mut sys = NetSyncSystem::new();
            let mut world = hecs::World::new();
            sys.queue_messages(msgs);
            sys.tick(&mut world, 0.016, &data);
            let npcs: Vec<(u64, usize, usize, f32)> = world
                .query_mut::<(&RemoteNpc, &Transform)>()
                .into_iter()
                .map(|(_, (n, t))| {
                    (n.entity_id, n.dialog.len(), n.greetings.len(), t.position.y)
                })
                .collect();
            assert_eq!(npcs.len(), 1, "{label}: exactly one entity per entity_id");
            assert_eq!(npcs[0].1, 1, "{label}: dialog lines present");
            assert_eq!(npcs[0].2, 1, "{label}: greetings present");
            // Both paths ground Y to the local floor (v0.681 rule).
            assert!(
                (npcs[0].3 - NPC_LOCAL_STANDING_Y).abs() < 0.51,
                "{label}: Y grounded locally, not the relay deck height (got {})",
                npcs[0].3
            );
        }
    }
}
