//! THE PLAYER'S MOVEMENT MODEL on a planetary surface (v0.1109): which of the
//! two control laws is running this frame, and the radial (vertical) motion
//! each one produces.
//!
//! WHY A SECOND FILE NEXT TO `surface_walk.rs`: that module owns the GEOMETRY
//! of being on a sphere - the tangent basis, the look angles, where the ground
//! is, and how a fall integrates once it has started. This one owns the
//! CONTROL LAW layered on top: what a held key is ALLOWED to do to your height.
//! The two were one inlined block inside `lib.rs`'s frame-lock, which is why
//! neither could be tested headless. `surface_walk.rs` sits at 1_066 of its
//! 1_150-line ratchet and `lib.rs` at 17_932 of 18_100, so the model lands in
//! its own file where it can carry the gates it needs.
//!
//! THE REPORT THIS EXISTS FOR (operator, 2026-08-03): "While I press W I
//! immediately start flying and then when I let go of W I maintain momentum
//! briefly then fall at normal gravity."
//!
//! THE MECHANISM, which is NOT the v0.880 float-residue dead zone: in the walk
//! band `lib.rs` chose the tangent-plane wish (`surface_wish_dir`) only when
//! dev fly mode was OFF. With it ON - which is what the HUD's "FLY x1" is
//! telling the operator - W became `fly_wish_dir_up`, a full-3D thrust along
//! the LOOK direction, and its radial share was handed to `vertical_step` as a
//! commanded climb rate. Any non-level nose was therefore a climb command at
//! walk speed, and because the command entered as a VELOCITY, releasing W left
//! real upward velocity that coasted (the "momentum") before gravity won (the
//! "fall at normal gravity"). Meanwhile the same block ran gravity and the
//! ground clamp regardless of mode, so dev flight could not hover either.
//!
//! So there was no walk model and no flight model - there was one blended
//! thing. This module makes them two:
//!
//!   * `MoveMode::Walk` - gravity, ground clamp, and a radial command that is
//!     EXACTLY 0 unless a deliberate vertical key is down. Forward input can
//!     not lift you, and that is structural (see `split_wish`), not an epsilon.
//!   * `MoveMode::DevFlight` - no gravity, no ground clamp, and a still stick
//!     holds the altitude BIT-EXACTLY, because the operator's stated purpose
//!     is "maintaining some arbitrary height... for taking a hovering picture
//!     of scenery".
//!
//! v0.1109.4 adds the model's other input: WHICH POINT ON THE PLANET it reads
//! the ground at (`ground_sample_dir`). That was a one-frame-stale expression
//! inline in `lib.rs`, and being stale by one frame of a 20-minute day is
//! kilometres of ground - which made the altitude, and through it the stand
//! height, a function of the FRAME RATE. It lands here rather than in
//! `surface_walk` because that file has 57 lines of ratchet slack left and
//! this one has none to defend; the derivation is on the function.
//!
//! Pure glam + `surface_walk`, no GPU, no winit, no cfg gate, so it compiles in
//! every feature set and every gate below runs headless.

use glam::DVec3;

use crate::surface_walk::{rest_radius, settle_radius, vertical_step, TERMINAL_FALL_MPS};
use humanity_core::ecs::components::EYE_HEIGHT_M;

/// Which control law the surface frame-lock runs this frame.
///
/// Built with `from_dev_flight` at the call site, from the F9 hover bit
/// (`GuiState::dev_hover`, v0.1109.2), which is NOT the controller's fly bit
/// (`GuiState::dev_fly_mode` -> `CameraController::fly_mode`): dev travel
/// sets the fly bit alone. The HUD's movement line reads both bits
/// (`gui::pages::hud::movement_line`, BUG-149, 2026-10-04), so it says FLY
/// whenever either changes how the player moves, using these two words.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum MoveMode {
    /// Feet on the ground. Gravity pulls, the ground clamps, and WASD moves
    /// strictly in the tangent plane. Space/Shift remain the only radial
    /// commands (jetpack up / powered descent), and they arrive as an exact
    /// +-1 so they are unmistakable from projection noise.
    #[default]
    Walk,
    /// Dev flight, toggled with F9. No gravity, no ground clamp, altitude held
    /// exactly when nothing is held. W still flies WHERE YOU LOOK - that is
    /// the whole point of a scenery camera - it just no longer does so while
    /// the player believes they are walking.
    DevFlight,
}

impl MoveMode {
    /// Build the mode from the engine's dev-flight flag.
    pub fn from_dev_flight(dev_flight: bool) -> Self {
        if dev_flight {
            MoveMode::DevFlight
        } else {
            MoveMode::Walk
        }
    }

    /// True while dev flight is engaged (no gravity, no ground clamp).
    pub fn is_flight(self) -> bool {
        matches!(self, MoveMode::DevFlight)
    }

    /// The word the top-centre HUD shows. Gravity behaves differently between
    /// the two, so the player must never have to guess which one is live.
    pub fn hud_word(self) -> &'static str {
        match self {
            MoveMode::Walk => "WALK",
            MoveMode::DevFlight => "FLY",
        }
    }

    /// WHERE THE MOVEMENT INTENT COMES FROM this frame - the single decision
    /// that produced the operator's report.
    ///
    /// `true`  -> `CameraController::surface_wish_dir`: WASD projected into the
    ///            tangent plane. Walking. Forward is forward, full stop.
    /// `false` -> `CameraController::fly_wish_dir_up`: full-3D thrust along the
    ///            LOOK direction, so a raised nose plus W is a climb.
    ///
    /// The look-direction wish is right for the flight band (10-100 km, where
    /// W flying where you point is the same mental model as FTL), right for
    /// swimming, and right for dev flight. It is WRONG for a player with feet
    /// on the ground, and until v0.1109 dev flight silently applied it there
    /// while gravity and the ground clamp still ran - which is exactly "while
    /// I press W I immediately start flying... then fall at normal gravity".
    pub fn uses_tangent_wish(self, in_walk_band: bool, submerged: bool) -> bool {
        self == MoveMode::Walk && in_walk_band && !submerged
    }
}

/// Dead zone on an ANALOG radial command - the flight band (10-100 km) and
/// swimming, where the wish IS the look direction and a nearly level nose
/// leaves a small radial share that must not read as "climb".
///
/// The WALK path deliberately does not use it: `split_wish` makes a walking
/// radial command exactly 0.0 or exactly +-1.0, so there is nothing left for
/// an epsilon to arbitrate. (v0.880 introduced this constant precisely because
/// the walk path had no such guarantee; it now does.)
pub const RADIAL_WISH_EPS: f64 = 0.05;

/// Within this much of the rest height you are STANDING (the footstep gate).
pub const GROUNDED_REACH_M: f64 = 0.35;

/// Height gap under which a walker is glued to the terrain by the settle ease
/// instead of going ballistic, so a downhill stride tracks the ground rather
/// than micro-falling between every step.
pub const SETTLE_GLUE_M: f64 = 0.5;

/// Split a movement wish into its tangential and radial parts about `up`.
///
/// The tangential part is always the exact projection (so a strafe stays a
/// strafe). The radial part is what the two modes disagree about:
///
///   * `DevFlight` keeps the analog share - flying where you look needs it.
///   * `Walk` QUANTISES it to exactly {-1, 0, +1}. That is lossless, not a
///     hack: `CameraController::surface_wish_dir` adds the vertical keys as a
///     whole `+up` / `-up`, so a deliberate command is always exactly unit
///     magnitude, while the WASD share is a tangent-plane projection whose
///     radial residue is f32 rounding (~1e-7). Rounding therefore collapses to
///     a true zero and "pure lateral input produces zero radial velocity
///     change" becomes a property of the arithmetic instead of a tolerance
///     somebody has to keep re-tuning.
pub fn split_wish(wish: DVec3, up: DVec3, mode: MoveMode) -> (DVec3, f64) {
    let up = up.normalize_or_zero();
    if up.length_squared() < 0.5 {
        // Degenerate up (no body under us): everything is lateral.
        return (wish, 0.0);
    }
    let raw = wish.dot(up);
    // Subtract the RAW share so the remainder is exactly tangent to `up`;
    // quantising before this would leave the residue in the lateral term.
    let tangential = wish - up * raw;
    let radial = match mode {
        MoveMode::Walk => {
            if raw.abs() >= 0.5 {
                raw.signum()
            } else {
                0.0
            }
        }
        MoveMode::DevFlight => raw,
    };
    (tangential, radial)
}

/// WHAT A CARRIED LOAD LETS SPACE DO WHILE WALKING (BUG-136, 2026-10-04).
///
/// `jump_scale` is `systems::encumbrance`'s share of the jump's launch speed:
/// 1.0 normally, below 1 for a load in the Realistic carrying mode (more mass
/// to launch), 0.0 when overloaded (no jumping under gravity, the operator's
/// rule). It scales an UPWARD walk command anywhere in the walk band, in the
/// air as well as on the ground: on a planet Space is not a one-shot jump but
/// a held climb at walking speed (`radial_step`'s walking branch), so there
/// is no separate launch to gate, and an overloaded walker who has stepped
/// off a ledge cannot climb back up either. Shift's powered descent, the
/// flight band, swimming and dev flight are not walking and pass through
/// untouched. Applied after `split_wish`, so a walk command is exactly +1
/// times the scale; at 0 it is a true 0.0 and `radial_step` sees no thrust.
pub fn carry_gated_radial(
    radial_wish: f64,
    mode: MoveMode,
    in_walk_band: bool,
    submerged: bool,
    jump_scale: f32,
) -> f64 {
    if mode == MoveMode::Walk && in_walk_band && !submerged && radial_wish > 0.0 {
        radial_wish * jump_scale.clamp(0.0, 1.0) as f64
    } else {
        radial_wish
    }
}

/// THE CARRIED LOAD'S SHARE OF WALKING SPEED ON A PLANET (BUG-136).
///
/// `carry_speed_factor` is `CameraController::carry_speed_factor` (1.0
/// normally, below 1 when overloaded in the Realistic carrying mode). The
/// operator's rule is "slower walking", so it applies to WALKING only: the
/// walk band, on foot (`MoveMode::Walk`), out of the water. Dev flight, the
/// flight band (10-100 km) and swimming get 1.0. Dev flight has two flags on
/// a planet: `mode` is the F9 hover (`GuiState::dev_hover`), and `fly_mode`
/// is the controller's fly flag, which dev travel sets on its own when it
/// drops the player at a planet; either one means not walking. The first
/// version folded the load into the controller-wide speed multiplier, which
/// every planet mode reads, so an overloaded player in dev flight crawled at
/// 15% while the HUD said "walking".
pub fn carry_walk_factor(
    carry_speed_factor: f32,
    mode: MoveMode,
    fly_mode: bool,
    in_walk_band: bool,
    submerged: bool,
) -> f64 {
    if !fly_mode && mode.uses_tangent_wish(in_walk_band, submerged) {
        carry_speed_factor.clamp(0.0, 1.0) as f64
    } else {
        1.0
    }
}

/// Result of one frame of radial motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RadialStep {
    /// New radial distance from the body centre (metres).
    pub r: f64,
    /// New radial velocity (m/s, + = up). Always 0 in dev flight.
    pub v_r: f64,
    /// Standing on the ground this frame (the footstep gate). Never true in
    /// dev flight: a hovering camera makes no footsteps.
    pub grounded: bool,
    /// The downward speed the ground stopped this frame, m/s: 0 unless a
    /// falling walker landed (falls, 2026-10-20, `systems::falls`). Only the
    /// walking branch lands: dev flight, the flight band and swimming have no
    /// ground to meet.
    pub impact_m_s: f64,
}

impl RadialStep {
    /// What the fall watch is told this frame (`CameraController::surface_step`):
    /// the speed the ground stopped, when the ground stopped a fall or the walker
    /// stands still on it, else None. Not merely within a boot's reach of the
    /// floor (`grounded`) while still dropping: that frame comes before the one
    /// that lands, and must not end the fall's watch before its landing.
    ///
    /// Here since the crate split's rung 0 (it was inside `surface_step`), so
    /// the renderer's camera is handed numbers rather than the simulation's
    /// `RadialStep`.
    pub fn landing(&self) -> Option<f64> {
        (self.impact_m_s > 0.0 || (self.grounded && self.v_r == 0.0)).then_some(self.impact_m_s)
    }
}

/// ONE FRAME OF RADIAL MOTION, for whichever model is running.
///
/// `rest` is the standing height (`surface_walk::rest_radius`); it doubles as
/// the ground clamp, because `clamp_above_ground` and `rest_radius` share a
/// floor by construction, so `r.max(rest)` IS the clamp.
///
/// `step` is this frame's movement budget in metres (speed * dt, already
/// capped by the caller's approach governor), and `radial_wish` is the split
/// above. Together they are the commanded climb: `step / dt * radial_wish`
/// metres per second.
#[allow(clippy::too_many_arguments)]
pub fn radial_step(
    mode: MoveMode,
    in_walk_band: bool,
    submerged: bool,
    r: f64,
    v_r: f64,
    rest: f64,
    g_accel: f64,
    settle_rate: f64,
    dt: f64,
    radial_wish: f64,
    step: f64,
) -> RadialStep {
    if mode.is_flight() {
        // ── DEV FLIGHT ── No gravity, no ground clamp, and a still stick is
        // a still camera. The zero test is exact on purpose: with no key held
        // the wish vector is exactly `DVec3::ZERO`, so its radial share is a
        // true 0.0 and `r` comes back bit-identical. Anything softer (an
        // epsilon, a lerp toward a target height, a settle) would be the
        // "drift" and the "settle" the operator explicitly ruled out.
        let r = if radial_wish == 0.0 { r } else { r + step * radial_wish };
        return RadialStep { r, v_r: 0.0, grounded: false, impact_m_s: 0.0 };
    }

    let (r_in, v_in) = (r, v_r);
    let (mut r, mut v_r) = (r, v_r);
    // THE LANDING (falls, 2026-10-20): the speed the ground stops when a
    // falling walker meets it, from the energy at the start of the frame
    // (`physics::landing::landing_speed`: v^2 = v0^2 + 2 g h to the floor), so it is the
    // fall's real speed whatever the frame's length. Under thrust (Space
    // braking a fall, Shift's powered descent) it is the speed the frame
    // started at.
    let mut impact_m_s = 0.0;
    if in_walk_band && !submerged {
        // ── WALKING ── Real ballistics (v0.1005) with a short-range settle
        // "glue" so strides track the terrain. `radial_wish` here is exactly
        // 0 or +-1, so `thrusting` is a fact about the keyboard, not a
        // threshold: forward input CANNOT enter this branch as a climb.
        let thrusting = radial_wish != 0.0;
        if !thrusting && v_r <= 0.0 && (r - rest) < SETTLE_GLUE_M {
            // The glue catches a fall within half a metre of the floor: a
            // landing, with that last half metre still to fall.
            if v_in < 0.0 {
                impact_m_s = humanity_core::physics::landing::landing_speed(v_in, r_in - rest, g_accel);
            }
            v_r = 0.0;
            r = settle_radius(r, rest, dt, settle_rate);
        } else {
            let climb_rate = step / dt.max(1e-6);
            let thrust = if thrusting { climb_rate * radial_wish } else { 0.0 };
            let vs = vertical_step(r, v_r, rest, g_accel, dt, thrust, TERMINAL_FALL_MPS);
            if vs.grounded && v_in < 0.0 {
                impact_m_s = if thrusting { -v_in } else { humanity_core::physics::landing::landing_speed(v_in, r_in - rest, g_accel) };
            }
            r = vs.r;
            v_r = vs.v_r;
        }
    } else {
        // ── FLIGHT BAND (10-100 km) / SWIMMING ── Aircraft altitude hold:
        // deliberate climb or descent only, no ballistic carry. The wish is
        // the analog look direction here, hence the dead zone.
        v_r = 0.0;
        if radial_wish > RADIAL_WISH_EPS {
            r += (step * radial_wish).max(1.0 * dt);
        } else if radial_wish < -RADIAL_WISH_EPS {
            r += radial_wish * step;
        }
    }
    // The ground clamp, restored the moment dev flight is switched off.
    r = r.max(rest);
    let grounded = in_walk_band && !submerged && (r - rest).abs() < GROUNDED_REACH_M;
    RadialStep { r, v_r, grounded, impact_m_s }
}

/// The standing height for a sampled ground radius + its clearance. Thin
/// re-export so a caller that only needs the floor does not have to remember
/// `EYE_HEIGHT_M` belongs between them.
pub fn rest_height(ground_r: f64, clearance: f64) -> f64 {
    rest_radius(ground_r, EYE_HEIGHT_M, clearance)
}

/// THE PLANET WALK'S FLOOR (2026-10-20): the standing height over the higher of the terrain
/// (`ground_r` with its `clearance`, `walk_ground_radius`'s pair) and the top of a built piece
/// under the body (`built_top_r`, `engine::planet_build::built_floor_radius`, None when there is
/// none). A built top is an exact surface, the box the player built, so it takes no clearance.
/// With nothing built it is `rest_height(ground_r, clearance)` to the bit. It is also the ground
/// clamp, like `rest` everywhere in `radial_step`, so standing on a roof is the same thing as
/// standing on the ground, walking off its edge is the same fall as off a cliff, and the landing
/// reaches the falls code the same way (`CameraController::surface_step`).
pub fn walk_rest(ground_r: f64, clearance: f64, built_top_r: Option<f64>) -> f64 {
    let terrain = rest_height(ground_r, clearance);
    match built_top_r {
        Some(top) => terrain.max(rest_height(top, 0.0)),
        None => terrain,
    }
}

/// Past this much disagreement the carried anchor is not the same place as the
/// camera: a teleport, a body swap, or a fresh session. One frame of planetary
/// spin cannot move the sample point anywhere near this far (see
/// `ground_sample_dir`), so this only rejects a genuinely stale anchor.
pub const ANCHOR_CONTINUITY_M: f64 = 100_000.0;

/// WHICH POINT ON THE PLANET THE TERRAIN IS SAMPLED AT this frame (v0.1109.4).
///
/// `anchor_pre` is the camera's CURRENT world position expressed in the
/// planet's unrotated frame (`dev_travel::frame_lock_capture`). It is NOT
/// where the player is standing: the camera was placed by LAST frame's
/// co-rotation, so un-rotating it with THIS frame's spin lands one frame of
/// planetary rotation BEHIND the ground point under their feet. On a day that
/// passes in 20 real minutes (the 24-hour day at time speed 72) the ground slips
/// 33,360 m/s at the equator, so the lag is 556 m at 60 fps, 1.1 km at 30 and
/// 4.7 km at 7 - PROPORTIONAL TO FRAME TIME.
///
/// Sampling terrain there made the altitude a function of the frame rate,
/// which is the operator's v0.1109.3 report: "when I look up in the air, I
/// fall down, but when I look parallel with the earth I float up". Looking at
/// heavy vegetation costs frames, the altitude reading grows with the frame
/// time, it crosses `surface_walk::DRAWN_GROUND_MAX_ALT_M`, and the stand
/// height flips between the drawn face (`DRAWN_CLEARANCE_M`) and the field
/// fallback (`LOD_CLEARANCE_M`): a 2.45 m ride up at the clearance ease's
/// 2 m/s, then a fall at gravity when the frames come back. Measured on the
/// shipped Earth data, standing perfectly still: 428 m of altitude
/// disagreement between 30 and 7 fps and a real 2.46 m difference in stand
/// height (engine/crossing_tests/surface_move_stand_still.rs).
///
/// The co-rotating anchor the frame-lock carries IS the point the player
/// stands on, so prefer it. `anchor_pre` stays correct on the frame surface
/// mode ENGAGES (no carried anchor yet) and whenever the carried one belongs
/// somewhere else, which the continuity guard catches.
pub fn ground_sample_dir(anchor_pre: DVec3, carried: DVec3, engaged: bool) -> DVec3 {
    if engaged && (carried - anchor_pre).length() < ANCHOR_CONTINUITY_M {
        let d = carried.normalize_or_zero();
        if d.length_squared() > 0.5 {
            return d;
        }
    }
    anchor_pre.normalize_or_zero()
}

#[cfg(any(test, feature = "test-support"))]
pub mod fixtures {
    //! The gravity, settle rate and frame the fall tests use, which the GUI's tests use too: apart
    //! from the tests so that the crate split can put them behind its `test-support` feature (rung 2).
    pub const G: f64 = 9.81;
    pub const SETTLE: f64 = 4.0;
    pub const DT: f64 = 1.0 / 60.0;
}

#[cfg(test)]
mod tests {
    use super::fixtures::*;
    use super::*;

    /// A planet fall lands at its real speed at any frame rate (BUG-209,
    /// 2026-10-20). `vertical_step`'s free fall was semi-implicit Euler (the
    /// speed first, then the height from the new speed), which drops the body
    /// further than the speed it has reached: after N frames of fall the landing
    /// read sqrt(N/(N+1)) of the true speed, 0.979 over a 2.62 m drop at 30 fps
    /// (`just verify-falls` measured 7.03 m/s where the drop gives 7.17), and
    /// worse the longer the frames. A 2.62 m drop from rest must land at
    /// sqrt(2 g h) within 0.1% at 4 ms, 33 ms and 100 ms frames. Seen red on the
    /// Euler step, 2026-10-20, already at the fastest frames: "at 4 ms frames a
    /// 2.62 m drop landed at 7.152 m/s, the drop gives 7.170".
    #[test]
    fn a_planet_fall_lands_at_its_real_speed_at_any_frame_rate() {
        let rest = 6.371e6 + 1.75;
        let drop = 2.62;
        let want = (2.0 * G * drop).sqrt();
        for dt in [0.004, 0.033, 0.1] {
            let (mut r, mut v) = (rest + drop, 0.0);
            let mut landed = None;
            for _ in 0..2000 {
                let s = radial_step(MoveMode::Walk, true, false, r, v, rest, G, SETTLE, dt, 0.0, 5.0 * dt);
                r = s.r;
                v = s.v_r;
                if s.impact_m_s > 0.0 {
                    landed = Some(s.impact_m_s);
                    break;
                }
            }
            let got = landed.expect("the fall never landed");
            assert!(
                (got - want).abs() < want * 1e-3,
                "at {:.0} ms frames a {drop} m drop landed at {got:.3} m/s, the drop gives {want:.3}",
                dt * 1000.0
            );
        }
    }

    /// BUG-136 on a planet: an overloaded walker's Space does not leave the
    /// ground, a loaded one rises slower, and nothing that is not a jump is
    /// touched (Shift, the flight band, swimming, dev flight). Seen red with
    /// `carry_gated_radial` passing the wish through unchanged: "an
    /// overloaded walker stays on the ground (rose 4.62 m)".
    #[test]
    fn an_overload_keeps_a_walker_on_the_ground() {
        let rest = 6.371e6 + 1.75;
        let rise = |scale: f32| {
            let wish = carry_gated_radial(1.0, MoveMode::Walk, true, false, scale);
            let (mut r, mut v) = (rest, 0.0);
            for _ in 0..60 {
                let s = radial_step(MoveMode::Walk, true, false, r, v, rest, G, SETTLE, DT, wish, 5.0 * DT);
                r = s.r;
                v = s.v_r;
            }
            r - rest
        };
        let none = rise(0.0);
        assert!(none == 0.0, "an overloaded walker stays on the ground (rose {none:.2} m)");
        let (full, loaded) = (rise(1.0), rise(0.84));
        assert!(loaded > 0.5 && loaded < full, "a loaded jump rises less: {loaded:.2} of {full:.2} m");
        // Not jumps: passed through whatever the load.
        assert_eq!(carry_gated_radial(-1.0, MoveMode::Walk, true, false, 0.0), -1.0, "Shift still descends");
        assert_eq!(carry_gated_radial(1.0, MoveMode::Walk, false, false, 0.0), 1.0, "flight band");
        assert_eq!(carry_gated_radial(1.0, MoveMode::Walk, true, true, 0.0), 1.0, "swimming");
        assert_eq!(carry_gated_radial(0.4, MoveMode::DevFlight, true, false, 0.0), 0.4, "dev flight");
        // A held climb in mid-air is gated too: the walking branch has no
        // separate launch, so an overloaded walker who stepped off a ledge
        // gets no thrust while falling, where an unloaded one brakes.
        let falling = |scale: f32| {
            let wish = carry_gated_radial(1.0, MoveMode::Walk, true, false, scale);
            radial_step(MoveMode::Walk, true, false, rest + 5.0, -2.0, rest, G, SETTLE, DT, wish, 5.0 * DT).v_r
        };
        assert!(falling(1.0) > -2.0, "an unloaded climb brakes the fall ({:.3} m/s)", falling(1.0));
        assert!(falling(0.0) < -2.0, "an overloaded faller keeps falling ({:.3} m/s)", falling(0.0));
    }

    /// BUG-136: the load slows WALKING on a planet and nothing else. Seen red
    /// against a stub applying the factor in every mode (the first version,
    /// which folded it into the controller-wide multiplier): "assertion
    /// `left == right` failed: dev flight ignores the load; left:
    /// 0.15000000596046448, right: 1.0".
    #[test]
    fn the_carried_load_slows_walking_only() {
        assert_eq!(carry_walk_factor(0.8, MoveMode::Walk, false, true, false), 0.8f32 as f64, "walking slows");
        assert_eq!(carry_walk_factor(1.0, MoveMode::Walk, false, true, false), 1.0);
        assert_eq!(carry_walk_factor(0.15, MoveMode::DevFlight, false, true, false), 1.0, "dev flight ignores the load");
        assert_eq!(carry_walk_factor(0.15, MoveMode::Walk, false, false, false), 1.0, "the flight band ignores it");
        assert_eq!(carry_walk_factor(0.15, MoveMode::Walk, false, true, true), 1.0, "swimming ignores it");
        // Dev travel drops the player at a planet with the controller's fly
        // flag on and the F9 hover off: still flight, not walking. Seen red
        // with the flag ignored: "assertion `left == right` failed: dev
        // travel's fly flag ignores the load; left: 0.15000000596046448,
        // right: 1.0".
        assert_eq!(
            carry_walk_factor(0.15, MoveMode::Walk, true, true, false),
            1.0,
            "dev travel's fly flag ignores the load"
        );
    }

    /// F9 OFF puts gravity and the ground clamp back. Hover at 500 m, switch
    /// to Walk with nothing held, and the camera falls at the planet's own g
    /// (about 10 s from 500 m), lands exactly on the rest height, and is
    /// reported grounded. Then prove the clamp itself is live by starting a
    /// walk step from BELOW the floor.
    #[test]
    fn leaving_dev_flight_restores_gravity_and_the_clamp() {
        let rest = 6.371e6 + 1.75;
        let mut r = rest + 500.0;
        let mut v = 0.0;
        // While flying: nothing moves.
        for _ in 0..120 {
            let s = radial_step(
                MoveMode::DevFlight, true, false, r, v, rest, G, SETTLE, DT, 0.0, 0.0,
            );
            r = s.r;
            v = s.v_r;
        }
        assert_eq!(r, rest + 500.0, "the hover leaked before the toggle");
        // F9 off: a real fall, at the planet's own g and capped at human
        // terminal velocity.
        let mut t = 0.0;
        let mut landed = None;
        for _ in 0..3600 {
            let s = radial_step(
                MoveMode::Walk, true, false, r, v, rest, G, SETTLE, DT, 0.0, 0.0,
            );
            r = s.r;
            v = s.v_r;
            t += DT;
            assert!(v >= -TERMINAL_FALL_MPS - 1e-9, "fell past terminal velocity: {v}");
            if landed.is_none() && s.grounded {
                landed = Some(t);
            }
        }
        let t = landed.expect("switching dev flight off did not start a fall");
        // 5.6 s of acceleration covers 154 m, then 346 m at the 55 m/s
        // terminal velocity: 11.9 s to touchdown.
        assert!((11.0..13.0).contains(&t), "500 m fall took {t:.2} s, expected ~11.9 s");
        // The settle glue then irons out the last half metre; a minute of
        // standing leaves the eye ON the rest height (the residue is the f64
        // asymptote of the ease, nanometres).
        assert!(
            (r - rest).abs() < 1e-6,
            "the fall did not settle onto the standing height ({:e} m off)",
            r - rest
        );
        assert_eq!(v, 0.0, "a standing player must have no vertical velocity");
        // The clamp is back: a step that starts underground snaps up to it.
        let s = radial_step(
            MoveMode::Walk, true, false, rest - 200.0, -30.0, rest, G, SETTLE, DT, 0.0, 0.0,
        );
        assert_eq!(s.r, rest, "the ground clamp did not come back with gravity");
        assert!(s.grounded, "clamped to the floor but not reported grounded");
        // ...and it is genuinely ABSENT in dev flight (that is the noclip).
        let f = radial_step(
            MoveMode::DevFlight, true, false, rest - 200.0, 0.0, rest, G, SETTLE, DT, 0.0, 0.0,
        );
        assert_eq!(f.r, rest - 200.0, "dev flight must not clamp to the ground");
    }

    /// `radial_step` clamps with `r.max(rest)` rather than calling
    /// `surface_walk::clamp_above_ground`, on the claim that the two are the
    /// same floor. That is a CLAIM, so hold it: if `clamp_above_ground` or
    /// `rest_radius` ever stop agreeing, the walk band would silently start
    /// resting somewhere other than where the clamp allows.
    #[test]
    fn the_clamp_and_the_rest_height_are_one_floor() {
        use crate::surface_walk::{clamp_above_ground, DRAWN_CLEARANCE_M, LOD_CLEARANCE_M};
        let ground = 6.371e6;
        for clearance in [DRAWN_CLEARANCE_M, LOD_CLEARANCE_M, 0.0] {
            let rest = rest_height(ground, clearance);
            for r in [ground - 500.0, ground, rest - 1e-6, rest, rest + 0.4, rest + 9_000.0] {
                assert_eq!(
                    r.max(rest),
                    clamp_above_ground(r, ground, EYE_HEIGHT_M, clearance),
                    "r={r} clearance={clearance}: radial_step's clamp drifted from \
                     surface_walk::clamp_above_ground"
                );
            }
        }
    }

    /// The mode is a VIEW of the engine's existing dev-flight flag, so the HUD
    /// word and the physics read the same bit.
    #[test]
    fn mode_mirrors_the_dev_flight_flag() {
        assert_eq!(MoveMode::from_dev_flight(false), MoveMode::Walk);
        assert_eq!(MoveMode::from_dev_flight(true), MoveMode::DevFlight);
        assert!(!MoveMode::from_dev_flight(false).is_flight());
        assert!(MoveMode::from_dev_flight(true).is_flight());
        assert_eq!(MoveMode::Walk.hud_word(), "WALK");
        assert_eq!(MoveMode::DevFlight.hud_word(), "FLY");
    }

    /// THE DECISION THAT CAUSED THE REPORT. A grounded walker gets the
    /// tangent-plane wish; the look-direction wish belongs to the flight band,
    /// to swimming, and to dev flight - never to feet on the ground.
    #[test]
    fn only_a_grounded_walker_gets_the_tangent_wish() {
        assert!(
            MoveMode::Walk.uses_tangent_wish(true, false),
            "a walker in the walk band must move in the tangent plane"
        );
        assert!(
            !MoveMode::Walk.uses_tangent_wish(false, false),
            "the flight band flies where you look"
        );
        assert!(
            !MoveMode::Walk.uses_tangent_wish(true, true),
            "swimming moves where you look"
        );
        // The pre-v0.1109 hole: dev flight ran the LOOK wish while the walk
        // band still applied gravity and the ground clamp, so W read as a
        // climb command to a player who believed they were walking. Dev
        // flight still flies where you look - but radial_step now removes the
        // gravity and the clamp that made it feel like a bug.
        assert!(
            !MoveMode::DevFlight.uses_tangent_wish(true, false),
            "dev flight flies where you look at every altitude"
        );
        assert!(
            MoveMode::DevFlight.is_flight(),
            "and it must therefore take the no-gravity, no-clamp branch"
        );
    }

    /// The 10-100 km flight band and swimming keep the analog altitude hold
    /// (and therefore the dead zone) - this change is about the walk band, and
    /// must not quietly re-time an aircraft climb.
    #[test]
    fn flight_band_keeps_its_analog_altitude_hold() {
        let rest = 6.371e6 + 1.75;
        let r0 = rest + 50_000.0;
        // Below the dead zone: nothing happens.
        let s = radial_step(
            MoveMode::Walk, false, false, r0, 0.0, rest, G, SETTLE, DT, 0.02, 100.0,
        );
        assert_eq!(s.r, r0, "a level nose must hold altitude in the flight band");
        // Above it: a real climb, and no ballistic carry.
        let s = radial_step(
            MoveMode::Walk, false, false, r0, 0.0, rest, G, SETTLE, DT, 0.5, 100.0,
        );
        assert!(s.r > r0 + 49.0, "flight-band climb stalled: {:.1} m", s.r - r0);
        assert_eq!(s.v_r, 0.0, "the flight band must not accumulate velocity");
        assert!(!s.grounded, "50 km up is not grounded");
    }
}
