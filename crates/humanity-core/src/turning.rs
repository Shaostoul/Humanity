//! HOW A PERSON TURNS (BUG-165 and BUG-166, 2026-10-05): toward a heading at no more than a
//! stated rate, speeding up into the turn and slowing out of it, so a turn never snaps and never
//! swings past where it was going.
//!
//! Two things turn this way:
//!   - the rig's scripted walk turns the camera to face where it walks and, on arrival, to the
//!     facing it was asked for (engine/move_check.rs `walk_step`, limits `LOOK`). The operator,
//!     watching the co-presence rig, saw it walk backwards and never turn: it held the final
//!     facing the whole way, "just using WASD instead of also using the mouse";
//!   - the game turns each crew figure to face the way it is drawn walking (net/sync.rs, limits
//!     `BODY`). Facing is cosmetic, so it is worked out on each screen from the figure's own
//!     motion and never sent (CLAUDE.md: cosmetic simulation runs locally).
//!
//! THE TURN is the quickest one that keeps to two limits: its speed never passes `rate`, and the
//! speed changes by no more than `accel` each second, up at the start and down at the end. It
//! slows in time to stop exactly on the heading (the speed it may have with `d` radians to go is
//! the one it can brake to a stop from in `d`, counted in whole steps, so a long frame cannot
//! carry it past), and lands there with its speed at zero. A heading that moves (a walker
//! arriving, a crew member rounding a corner) is followed from whatever speed the turn has.
//!
//! Pure and ungated (plain f32), so its tests run under every feature set.

use std::f32::consts::{PI, TAU};

/// How fast a turn may go, and how quickly it may speed up and slow down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurnLimits {
    /// The fastest the turn goes, radians a second.
    pub rate: f32,
    /// How quickly its speed may change, up or down, radians a second each second.
    pub accel: f32,
}

/// A person turning to look somewhere, the way the rig's walking camera turns: at most 150
/// degrees a second, reached in a quarter of a second. A quarter turn takes about 0.8 s and a half
/// turn about 1.4 s, quick enough to read as a person turning with the mouse and slow enough to
/// follow on a screen (a turn that sweeps the view faster is hard to watch).
pub const LOOK: TurnLimits = TurnLimits { rate: 150.0 * PI / 180.0, accel: (150.0 * PI / 180.0) / 0.25 };

/// A person turning their whole body as they walk, the way the crew figures turn: at most 360
/// degrees a second, reached in a fifth of a second. A half turn takes about 0.7 s, a pivot on
/// the spot, and a quarter turn about 0.45 s.
pub const BODY: TurnLimits = TurnLimits { rate: 360.0 * PI / 180.0, accel: (360.0 * PI / 180.0) / 0.2 };

/// The longest step one call turns through, seconds: a long frame never makes a long swing (the
/// rig's walk caps its stride the same way, engine/move_check.rs).
pub const MAX_STEP_S: f32 = 0.1;

/// The signed shortest turn from angle `from` to angle `to`, radians, in [-PI, PI).
pub fn shortest(from: f32, to: f32) -> f32 {
    let d = (to - from).rem_euclid(TAU);
    if d >= PI {
        d - TAU
    } else {
        d
    }
}

/// An angle turning the way a person turns it (`toward`): where it points now, radians, and how
/// fast it is turning, radians a second (signed).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Turn {
    pub angle: f32,
    pub rate: f32,
}

impl Turn {
    /// One step of `dt` seconds (at most `MAX_STEP_S`) toward the heading `want`, within `lim`.
    /// True once it points at `want` and has stopped turning; it then stays exactly there. The
    /// angle is kept continuous (it may run past PI, or below -PI), the way a camera's yaw does.
    pub fn toward(&mut self, want: f32, dt: f32, lim: TurnLimits) -> bool {
        if !(self.angle.is_finite() && self.rate.is_finite()) {
            // Nothing sensible to turn from: stand on the heading.
            self.angle = want;
            self.rate = 0.0;
            return true;
        }
        let dt = if dt.is_finite() { dt.clamp(0.0, MAX_STEP_S) } else { 0.0 };
        let gap = shortest(self.angle, want);
        let dv = lim.accel * dt;
        if gap == 0.0 && self.rate.abs() <= dv {
            self.rate = 0.0;
            return true;
        }
        if dt <= 0.0 {
            return false;
        }
        // The fastest it may turn with `gap` to go and still stop on the heading, braking at
        // `accel` in steps of `dt`: the v for which one step at v plus the stepped braking from v
        // (v^2 / 2a - v dt / 2) is exactly the gap.
        let g = gap.abs();
        let brake = lim.accel * ((dt * dt / 4.0 + 2.0 * g / lim.accel).sqrt() - dt / 2.0);
        let wanted = brake.min(lim.rate).copysign(gap);
        self.rate += (wanted - self.rate).clamp(-dv, dv);
        let step = self.rate * dt;
        if step * gap > 0.0 && step.abs() >= g {
            // This step reaches the heading: stop on it.
            self.angle += gap;
            self.rate = 0.0;
            return true;
        }
        self.angle += step;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEG: f32 = PI / 180.0;

    /// Turns from `from` to `want` at `dt` a step; returns every angle stepped through (the first
    /// is `from`) and whether it arrived within ten seconds.
    fn run(from: f32, want: f32, dt: f32, lim: TurnLimits) -> (Vec<f32>, bool) {
        let mut t = Turn { angle: from, rate: 0.0 };
        let mut seen = vec![from];
        for _ in 0..(10.0 / dt) as usize {
            let done = t.toward(want, dt, lim);
            seen.push(t.angle);
            if done {
                return (seen, true);
            }
        }
        (seen, false)
    }

    /// THE TURN KEEPS ITS LIMITS AND LANDS ON THE HEADING, at any frame rate: never faster than
    /// the stated rate, never past the heading, starting and ending gently (the first and last
    /// steps are small), and standing exactly on it at the end. Both limit sets, turns from a
    /// degree to a half turn, at 240, 60 and 30 frames a second and at the 0.1 s step cap.
    #[test]
    fn a_turn_keeps_its_limits_and_lands_on_the_heading() {
        for (name, lim) in [("LOOK", LOOK), ("BODY", BODY)] {
            for dt in [1.0 / 240.0, 1.0 / 60.0, 1.0 / 30.0, 0.1] {
                for deg in [1.0f32, 10.0, 45.0, 90.0, 179.0, -120.0] {
                    let want = deg * DEG;
                    let (seen, arrived) = run(0.0, want, dt, lim);
                    let label = format!("{name}, {deg} degrees at {:.0} frames a second", 1.0 / dt);
                    assert!(arrived, "{label}: never arrived");
                    assert!((seen.last().unwrap() - want).abs() < 1e-5, "{label}: stopped at {} rad, not {want}", seen.last().unwrap());
                    for w in seen.windows(2) {
                        let rate = (w[1] - w[0]).abs() / dt;
                        assert!(rate <= lim.rate * 1.0001, "{label}: turned at {:.1} degrees a second, the limit is {:.1}", rate / DEG, lim.rate / DEG);
                        assert!((w[1] - want) * want.signum() <= 1e-6, "{label}: swung past the heading to {} rad", w[1]);
                    }
                    let first = (seen[1] - seen[0]).abs() / dt;
                    let last = (seen[seen.len() - 1] - seen[seen.len() - 2]).abs() / dt;
                    assert!(first <= lim.accel * dt * 1.0001, "{label}: the first step turned at {:.1} degrees a second, a snap", first / DEG);
                    assert!(last <= 2.0 * lim.accel * dt + 1e-4, "{label}: the last step turned at {:.1} degrees a second, a sudden stop", last / DEG);
                }
            }
        }
    }

    /// THE SHORT WAY ROUND, and the angle stays continuous: from 170 degrees to -170 the turn goes
    /// up through 180 (20 degrees), not down through 0 (340), and ends at 190 degrees, which is
    /// -170. Also: a turn already on its heading is over at once and does not move.
    #[test]
    fn a_turn_goes_the_short_way_round() {
        let (seen, arrived) = run(170.0 * DEG, -170.0 * DEG, 1.0 / 60.0, LOOK);
        assert!(arrived);
        assert!(seen.windows(2).all(|w| w[1] >= w[0]), "turned the long way round");
        assert!((seen.last().unwrap() - 190.0 * DEG).abs() < 1e-5, "ended at {} degrees", seen.last().unwrap() / DEG);
        let mut t = Turn { angle: 0.25, rate: 0.0 };
        assert!(t.toward(0.25, 1.0 / 60.0, LOOK));
        assert_eq!(t, Turn { angle: 0.25, rate: 0.0 });
        assert!((shortest(0.0, 1.5 * PI) - -0.5 * PI).abs() < 1e-5, "three quarters of a turn one way is a quarter the other");
        assert!((shortest(0.1, -0.1) - -0.2).abs() < 1e-6 && (shortest(-3.0, 3.0) - (6.0 - TAU)).abs() < 1e-5);
    }

    /// A HEADING THAT MOVES MID-TURN IS FOLLOWED FROM THE SPEED THE TURN HAS: no jump in speed
    /// (the change between two steps stays within the limit), and it still lands. A walker that
    /// arrives while turning one way and must then face the other turns back smoothly.
    #[test]
    fn a_heading_that_moves_is_followed_smoothly() {
        let dt = 1.0 / 60.0;
        let mut t = Turn::default();
        let mut rates = Vec::new();
        for _ in 0..20 {
            let before = t.angle;
            t.toward(PI / 2.0, dt, LOOK);
            rates.push((t.angle - before) / dt);
        }
        let mut done = false;
        for _ in 0..600 {
            let before = t.angle;
            done = t.toward(-PI / 2.0, dt, LOOK);
            rates.push((t.angle - before) / dt);
            if done {
                break;
            }
        }
        assert!(done, "never turned to the new heading");
        assert!((t.angle - -PI / 2.0).abs() < 1e-5);
        for w in rates.windows(2) {
            assert!((w[1] - w[0]).abs() <= 2.0 * LOOK.accel * dt + 1e-3, "the speed jumped from {:.1} to {:.1} degrees a second", w[0] / DEG, w[1] / DEG);
        }
    }

    /// A long frame turns no further than `MAX_STEP_S` of turning, and a broken angle stands on
    /// the heading instead of spreading NaN.
    #[test]
    fn a_long_frame_turns_no_further_than_the_cap() {
        let mut t = Turn::default();
        t.toward(PI, 5.0, BODY);
        assert!(t.angle.abs() <= BODY.accel * MAX_STEP_S * MAX_STEP_S + 1e-6, "a five-second frame turned {} rad", t.angle);
        let mut broken = Turn { angle: f32::NAN, rate: 0.0 };
        assert!(broken.toward(1.0, 1.0 / 60.0, BODY));
        assert_eq!(broken.angle, 1.0);
    }
}
