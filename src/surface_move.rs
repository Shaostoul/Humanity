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

use crate::surface_walk::{rest_radius, settle_radius, vertical_step, EYE_HEIGHT_M,
    TERMINAL_FALL_MPS};

/// Which control law the surface frame-lock runs this frame.
///
/// This is a VIEW of the existing dev-flight flag (`GuiState::dev_fly_mode` ->
/// `CameraController::fly_mode`), not a second source of truth: build it with
/// `from_dev_flight` at the call site. The HUD's long-standing "FLY" indicator
/// reads the same flag, so the mode word and the physics can never disagree.
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
        return RadialStep { r, v_r: 0.0, grounded: false };
    }

    let (mut r, mut v_r) = (r, v_r);
    if in_walk_band && !submerged {
        // ── WALKING ── Real ballistics (v0.1005) with a short-range settle
        // "glue" so strides track the terrain. `radial_wish` here is exactly
        // 0 or +-1, so `thrusting` is a fact about the keyboard, not a
        // threshold: forward input CANNOT enter this branch as a climb.
        let thrusting = radial_wish != 0.0;
        if !thrusting && v_r <= 0.0 && (r - rest) < SETTLE_GLUE_M {
            v_r = 0.0;
            r = settle_radius(r, rest, dt, settle_rate);
        } else {
            let climb_rate = step / dt.max(1e-6);
            let thrust = if thrusting { climb_rate * radial_wish } else { 0.0 };
            let vs = vertical_step(r, v_r, rest, g_accel, dt, thrust, TERMINAL_FALL_MPS);
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
    RadialStep { r, v_r, grounded }
}

/// The standing height for a sampled ground radius + its clearance. Thin
/// re-export so a caller that only needs the floor does not have to remember
/// `EYE_HEIGHT_M` belongs between them.
pub fn rest_height(ground_r: f64, clearance: f64) -> f64 {
    rest_radius(ground_r, EYE_HEIGHT_M, clearance)
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
/// planetary rotation BEHIND the ground point under their feet. On the game's
/// 20-minute day (`systems::time::SECONDS_PER_DAY` = 1200) the ground slips
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
/// height (`stand_still_tests`).
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

/// THE REPRODUCTION (v0.1109.4). Operator, on v0.1109.3: "When I look up in
/// the air, I fall down, but when I look parallel with the earth I float up."
///
/// A headless twin of the frame-lock's per-frame ground chain
/// (`lib.rs` 3998-4490), driven with NO INPUT at two frame rates. Everything
/// it calls is the shipped function the engine calls - `frame_lock_capture`,
/// `ground_radius_m`, `walk_ground_radius`, `radial_step` - in the shipped
/// order; the only test-side code is the sequencing, which is why each step
/// carries the lib.rs line it mirrors.
#[cfg(all(test, feature = "native"))]
mod stand_still_tests {
    use crate::dev_travel::frame_lock_capture;
    use crate::engine::frame_lock::ground_radius_m;
    use super::ground_sample_dir;
    use crate::surface_walk::{walk_ground_radius, DRAWN_CLEARANCE_M, LOD_CLEARANCE_M};
    use crate::terrain::ocean_mask::OceanMask;
    use crate::terrain::planet::PlanetDef;
    use crate::terrain::planet_chunks as pc;
    use crate::terrain::planet_chunks::DetailNoise;
    use crate::terrain::planet_heightmap::PlanetHeightmap;
    use glam::{DQuat, DVec3};

    /// The game's own day length: 20 minutes, so the ground under a standing
    /// player slips 2*PI*R/1200 = 33,360 m/s at the equator. This is the
    /// number that turns a one-frame sampling lag into kilometres.
    const DAY_S: f64 = crate::systems::time::SECONDS_PER_DAY;
    const G: f64 = 9.81;
    const SETTLE: f64 = 4.0;
    /// A settled walk-band depth (the operator's tile tier reaches 17-20).
    const DRAWN_DEPTH: u8 = 17;
    /// Camera + selector params the pitch gate drives the real LOD selection
    /// with: the shipped Settings defaults (`terrain_split_px` 4.0,
    /// `terrain_patch_budget` 2048) at a 1080-line viewport.
    const SPLIT_PX: f32 = 4.0;
    const PATCH_BUDGET: usize = 2048;
    const VIEWPORT_H: f32 = 1080.0;

    fn real_earth() -> (PlanetHeightmap, OceanMask, PlanetDef) {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("planets");
        let hm = PlanetHeightmap::load(&base.join("earth_heightmap.bin")).expect("heightmap");
        let om = OceanMask::load(&base.join("earth_ocean_mask.bin")).expect("ocean mask");
        let text = std::fs::read_to_string(base.join("earth.ron")).expect("earth.ron");
        let mut def: PlanetDef = ron::from_str(&text).expect("earth.ron parses");
        def.sea_level = hm.sea_level_normalized();
        (hm, om, def)
    }

    fn dir_of(lat_deg: f64, lon_deg: f64) -> DVec3 {
        let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
        let cl = lat.cos();
        DVec3::new(cl * lon.cos(), lat.sin(), -cl * lon.sin())
    }

    /// One standing player, one planet, one frame at a time.
    struct StandSim<'a> {
        def: &'a PlanetDef,
        hm: &'a PlanetHeightmap,
        dn: &'a DetailNoise,
        om: &'a OceanMask,
        /// The co-rotating anchor: the player's position in the planet's
        /// UNROTATED frame (`state.frame_lock_anchor`).
        anchor: DVec3,
        vr: f64,
        last_ground_r: f64,
        last_clearance: f64,
        spin: f64,
        last_spin: f64,
        /// Last frame's altitude reading (the HUD's number).
        alt: f64,
        /// The depth proxy the clamp is handed - in the engine, the finest
        /// patch the renderer DREW last frame, which the pitch gate below
        /// takes from the real selector.
        depth: u8,
    }

    impl<'a> StandSim<'a> {
        fn new(
            def: &'a PlanetDef,
            hm: &'a PlanetHeightmap,
            dn: &'a DetailNoise,
            om: &'a OceanMask,
            dir: DVec3,
        ) -> Self {
            let field = ground_radius_m(Some(def), Some(hm), Some(dn), None, dir);
            let rest = super::rest_height(field, DRAWN_CLEARANCE_M);
            Self {
                def,
                hm,
                dn,
                om,
                anchor: dir * rest,
                vr: 0.0,
                last_ground_r: 0.0,
                last_clearance: 0.0,
                spin: 0.0,
                last_spin: 0.0,
                alt: 0.0,
                depth: DRAWN_DEPTH,
            }
        }

        /// One frame with NO INPUT AT ALL: no key held, no mouse, no walking.
        fn frame(&mut self, dt: f64) {
            // The planet turns (lib.rs `current_planet_spin`).
            self.last_spin = self.spin;
            self.spin += std::f64::consts::TAU * dt / DAY_S;
            // The camera's world position is where LAST frame's co-rotation
            // put it (lib.rs 4538 `frame_lock_ship_pos`, read back at 3975).
            let cam_world = DQuat::from_rotation_y(self.last_spin) * self.anchor;
            // lib.rs 3998: express it back in the planet's unrotated frame.
            let anchor_pre = frame_lock_capture(DVec3::ZERO, self.spin, cam_world);
            // lib.rs 4002-4032: the ground under the player, and the altitude
            // every band test + the HUD reads. Both halves of the fix are
            // here: the SAMPLE POINT (the shipped chooser) and the ground
            // DEFINITION (detail-inclusive, the same one the clamp uses).
            let sample = ground_sample_dir(anchor_pre, self.anchor, true);
            let ground_r =
                ground_radius_m(Some(self.def), Some(self.hm), Some(self.dn), None, sample);
            let sea_r = if self.def.has_water { self.def.radius } else { 0.0 };
            self.alt = self.anchor.length() - ground_r.max(sea_r);
            // lib.rs 4300-4321: the clamp's OWN ground, at the player's own
            // direction. No input, so the anchor did not move laterally.
            let dir1 = self.anchor.normalize();
            let field_r =
                ground_radius_m(Some(self.def), Some(self.hm), Some(self.dn), None, dir1);
            let (g0, clearance) = walk_ground_radius(
                field_r,
                Some(self.def),
                Some(self.hm),
                Some(self.dn),
                None,
                Some(self.om),
                self.depth,
                self.anchor.length() - field_r,
                dir1,
            );
            // lib.rs 4412-4431: ground-reference pop filter (stationary, so
            // it always arms).
            let mut g = g0;
            if self.last_ground_r > 0.0 {
                let max_dv = 2.0 * dt;
                let dgap = g - self.last_ground_r;
                if dgap.abs() > max_dv {
                    g = self.last_ground_r + dgap.signum() * max_dv;
                }
            }
            self.last_ground_r = g;
            // lib.rs 4442-4450: the clearance ease.
            let clearance = {
                if self.last_clearance <= 0.0 {
                    self.last_clearance = clearance;
                }
                let step = 2.0 * dt;
                let d = clearance - self.last_clearance;
                self.last_clearance += d.clamp(-step, step);
                self.last_clearance
            };
            // lib.rs 4454-4486: rest height, then one frame of radial motion.
            let rest = super::rest_height(g, clearance);
            let vs = super::radial_step(
                super::MoveMode::Walk,
                true,
                false,
                self.anchor.length(),
                self.vr,
                rest,
                G,
                SETTLE,
                dt,
                0.0,
                0.0,
            );
            self.vr = vs.v_r;
            self.anchor = dir1 * vs.r;
        }

        fn run(&mut self, dt: f64, seconds: f64) {
            for _ in 0..((seconds / dt) as usize) {
                self.frame(dt);
            }
        }

        fn height_above_ground(&self) -> f64 {
            let dir = self.anchor.normalize();
            self.anchor.length()
                - ground_radius_m(Some(self.def), Some(self.hm), Some(self.dn), None, dir)
        }
    }

    /// THE GATE: a player standing still, with nothing held, must be at the
    /// same height whatever the frame rate. The operator's report is the
    /// FAILURE of this - looking at heavy vegetation drops the frame rate,
    /// which is the only thing that changed between "I float up" and "I fall
    /// down".
    #[test]
    fn standing_still_holds_altitude_at_every_frame_rate() {
        let (hm, om, def) = real_earth();
        let dn = DetailNoise::new(def.terrain_seed);
        // Real walkable ground, away from the coast fade.
        let sites = [
            ("fuji-flank", 35.29, 138.79),
            ("alps-montblanc", 45.83, 6.865),
            ("kansas-plain", 38.5, -98.0),
            ("iowa-farm", 42.0, -93.5),
            ("andes-flank", -13.2, -72.5),
            ("rockies-flank", 39.6, -105.9),
            ("himalaya-foot", 27.5, 87.0),
            ("sahel", 14.0, 2.0),
        ];
        let mut worst_alt = 0.0_f64;
        let mut worst_drift = 0.0_f64;
        let mut worst_where = String::new();
        let mut straddles = 0;
        for (name, lat, lon) in sites {
            let dir = dir_of(lat, lon);
            if om.is_ocean(dir.as_vec3()) {
                continue;
            }
            // 4 s of standing still at 30 fps and at 7 fps (the operator's
            // 10-16 fps with vegetation maxed, and worse).
            let mut fast = StandSim::new(&def, &hm, &dn, &om, dir);
            fast.run(1.0 / 30.0, 4.0);
            let mut slow = StandSim::new(&def, &hm, &dn, &om, dir);
            slow.run(1.0 / 7.0, 4.0);
            let d_alt = (fast.alt - slow.alt).abs();
            let drift = (fast.height_above_ground() - slow.height_above_ground()).abs();
            if fast.alt < 40.0 && slow.alt > 40.0 || slow.alt < 40.0 && fast.alt > 40.0 {
                straddles += 1;
            }
            println!(
                "{name:>16}: alt 30fps={:6.1} m  7fps={:6.1} m (delta {d_alt:5.1})  |  eye above \
                 ground 30fps={:5.2} m 7fps={:5.2} m (drift {drift:.3} m)",
                fast.alt,
                slow.alt,
                fast.height_above_ground(),
                slow.height_above_ground(),
            );
            if d_alt > worst_alt {
                worst_alt = d_alt;
            }
            if drift > worst_drift {
                worst_drift = drift;
                worst_where = name.to_string();
            }
        }
        println!(
            "worst altitude disagreement {worst_alt:.1} m; worst standing-height drift \
             {worst_drift:.3} m at {worst_where}; {straddles} site(s) straddle the \
             DRAWN_GROUND_MAX_ALT_M gate"
        );
        assert!(
            worst_alt < 1.0,
            "STANDING STILL, the altitude the frame-lock measures moved {worst_alt:.1} m when \
             only the FRAME RATE changed. It is sampled at `anchor_pre`, which trails the player \
             by one frame of planetary spin ({:.0} m/s of ground slip on a {DAY_S} s day), so the \
             HUD altitude AND walk_ground_radius's DRAWN_GROUND_MAX_ALT_M gate are functions of \
             frame time.",
            std::f64::consts::TAU * 6.371e6 / DAY_S
        );
        assert!(
            worst_drift < 0.01,
            "STANDING STILL WITH NOTHING HELD, the player's height above the ground differed by \
             {worst_drift:.3} m at {worst_where} between 30 fps and 7 fps. {} m of that is the \
             clearance flipping between the drawn face and the field fallback because the \
             altitude gate moved with the frame rate.",
            LOD_CLEARANCE_M - DRAWN_CLEARANCE_M
        );
    }

    /// The max patch depth the RENDERER draws for a camera standing at `dir`
    /// and looking at `pitch_deg` - the engine's `cs.last_drawn` proxy, from
    /// the REAL selector with the REAL frustum, on a settled cache.
    fn drawn_depth_at(
        def: &PlanetDef,
        hm: &PlanetHeightmap,
        dn: &DetailNoise,
        dir: DVec3,
        cam_r: f64,
        pitch_deg: f32,
        bands: &std::cell::RefCell<std::collections::HashMap<pc::PatchId, pc::RadialBand>>,
    ) -> u8 {
        let cam_local = dir * cam_r;
        let mut cam = crate::renderer::camera::Camera::new();
        cam.position = glam::Vec3::ZERO;
        cam.aspect = 16.0 / 9.0;
        cam.fov_degrees = 90.0;
        cam.set_surface_up(dir.as_vec3());
        cam.yaw = 0.7;
        cam.pitch = pitch_deg.to_radians();
        // Mirrors lib.rs 8810-8820: same reverse-Z projection, same transform
        // into the planet's unrotated frame (here spin 0, camera at the render
        // origin, so the planet offset is -cam_local).
        let proj = glam::DMat4::perspective_rh(
            (cam.fov_degrees as f64).to_radians(),
            cam.aspect as f64,
            1.0e13,
            1.0,
        );
        let frustum = pc::FrustumPlanes::from_view_proj(&(proj * cam.view_matrix().as_dmat4()))
            .into_local(DQuat::IDENTITY, -cam_local);
        let params = pc::ChunkParams {
            occluder_r_m: None,
            radius_m: def.radius,
            band: pc::RadialBand {
                min_r_m: def.radius
                    * crate::terrain::planet_surface::displaced_radius_f64(def, 0.0),
                max_r_m: def.radius
                    * crate::terrain::planet_surface::displaced_radius_f64(def, 1.0),
            },
            max_depth: pc::TILE_MAX_PATCH_DEPTH,
            split_px: SPLIT_PX,
            px_per_rad: VIEWPORT_H / (90.0_f32).to_radians(),
            max_leaves: PATCH_BUDGET,
            max_build_requests: pc::MAX_BUILD_REQUESTS,
        };
        // A SETTLED cache: every patch built, each carrying a MEASURED band
        // (which is what makes near-camera frustum culling bite - the
        // planet-wide band would leave every near patch trivially visible and
        // the gate could not fail).
        let is_built = |id: &pc::PatchId| -> Option<pc::RadialBand> {
            if let Some(b) = bands.borrow().get(id) {
                return Some(*b);
            }
            let c = pc::patch_corners(id);
            let mids = [
                (c[0] + c[1]).normalize(),
                (c[1] + c[2]).normalize(),
                (c[2] + c[0]).normalize(),
            ];
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for d in c.iter().chain(mids.iter()) {
                let r = ground_radius_m(Some(def), Some(hm), Some(dn), None, *d);
                lo = lo.min(r);
                hi = hi.max(r);
            }
            let b = pc::RadialBand { min_r_m: lo - 60.0, max_r_m: hi + 60.0 };
            bands.borrow_mut().insert(*id, b);
            Some(b)
        };
        pc::select_patches_sticky(cam_local, Some(&frustum), &is_built, &params, None)
            .draws
            .iter()
            .map(|p| p.depth)
            .max()
            .unwrap_or(0)
    }

    /// LOOKING AROUND MUST NOT MOVE THE PLAYER (operator, v0.1109.3).
    ///
    /// The clamp's depth proxy is `cs.last_drawn.max()` - a RENDERER-side,
    /// frustum-culled, budget-limited number - so the camera's aim reaches the
    /// ground reference whether it should or not. This drives the real
    /// selector at nine pitches, feeds each answer to the real clamp, stands
    /// there for two seconds with nothing held, and measures.
    ///
    /// The residue is not zero and the number in the assert says how big it is
    /// allowed to be: on a settled cache the proxy moves by one LOD level
    /// (deep patches near the camera keep the whole leaf budget once the
    /// horizon leaves the frustum), which is centimetres of drawn-surface
    /// difference. A regression to METRES means the proxy has started
    /// collapsing to a coarse depth - or to 0, which flips the 2.45 m
    /// clearance - and the fix then is to stop deriving the ground the player
    /// stands on from what the renderer chose to draw.
    #[test]
    fn looking_around_does_not_move_the_player() {
        let (hm, om, def) = real_earth();
        let dn = DetailNoise::new(def.terrain_seed);
        for (name, lat, lon) in [("fuji-flank", 35.29, 138.79), ("kansas-plain", 38.5, -98.0)] {
            let dir = dir_of(lat, lon);
            let bands = std::cell::RefCell::new(std::collections::HashMap::new());
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            let mut depths = Vec::new();
            for pitch in [-89.0_f32, -60.0, -30.0, 0.0, 30.0, 60.0, 80.0, 89.0, 89.9] {
                let mut sim = StandSim::new(&def, &hm, &dn, &om, dir);
                sim.depth = drawn_depth_at(&def, &hm, &dn, dir, sim.anchor.length(), pitch, &bands);
                depths.push((pitch, sim.depth));
                sim.run(1.0 / 30.0, 2.0);
                let h = sim.height_above_ground();
                lo = lo.min(h);
                hi = hi.max(h);
            }
            println!(
                "{name:>16}: drawn depth by pitch {depths:?} -> stand height {lo:.4}..{hi:.4} m \
                 (spread {:.4} m)",
                hi - lo
            );
            assert!(
                hi - lo < 0.05,
                "{name}: standing still with nothing held, LOOKING AROUND moved the player \
                 {:.3} m. The clamp's ground reference is following the camera through \
                 `last_drawn`; depths seen: {depths:?}",
                hi - lo
            );
        }
    }

    /// The sample-point chooser itself: prefer the carried anchor while
    /// engaged, fall back to the fresh capture when there is no continuous one
    /// (engage frame, teleport, body swap).
    #[test]
    fn ground_sample_dir_prefers_the_carried_anchor() {
        let r = 6.371e6 + 1.75;
        let carried = dir_of(35.0, 139.0) * r;
        // One frame of spin at 7 fps: the worst realistic lag, 4.7 km of arc.
        let lag = std::f64::consts::TAU / DAY_S / 7.0;
        let pre = frame_lock_capture(DVec3::ZERO, lag, DQuat::from_rotation_y(0.0) * carried);
        let arc = (pre.normalize() - carried.normalize()).length() * r;
        assert!(
            (3_000.0..6_000.0).contains(&arc),
            "one frame of spin at 7 fps should be kilometres of ground slip, got {arc:.0} m"
        );
        assert_eq!(
            ground_sample_dir(pre, carried, true),
            carried.normalize(),
            "an engaged frame-lock must sample the ground it is carrying you over"
        );
        assert_eq!(
            ground_sample_dir(pre, DVec3::ZERO, true),
            pre.normalize(),
            "no carried anchor yet (engage frame) must fall back to the capture"
        );
        assert_eq!(
            ground_sample_dir(pre, carried, false),
            pre.normalize(),
            "not engaged means there is nothing to carry"
        );
        // A stale anchor from somewhere else must NOT be trusted.
        let elsewhere = dir_of(-20.0, 100.0) * r;
        assert_eq!(
            ground_sample_dir(pre, elsewhere, true),
            pre.normalize(),
            "an anchor a continent away is a teleport, not one frame of spin"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::camera::{Camera, CameraController};
    use glam::Vec3;

    const G: f64 = 9.81;
    const SETTLE: f64 = 4.0;
    const DT: f64 = 1.0 / 60.0;
    /// A real walking site: 35 N, 20 E on a 6,371 km sphere.
    fn radial_at(lat_deg: f32, lon_deg: f32) -> Vec3 {
        let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
        Vec3::new(lat.cos() * lon.cos(), lat.sin(), -lat.cos() * lon.sin()).normalize()
    }

    /// A surface camera standing at `radial`, looking `pitch_deg` above the
    /// horizon, with the given keys held. Returns the engine's own wish: the
    /// SAME `surface_wish_dir` / `fly_wish_dir_up` the frame-lock calls, so
    /// these gates test the shipped input path and not a restatement of it.
    fn wish_for(radial: Vec3, pitch_deg: f32, keys: &[&str], fly: bool) -> DVec3 {
        let mut cam = Camera::new();
        cam.set_surface_up(radial);
        cam.yaw = 0.7;
        cam.pitch = pitch_deg.to_radians();
        let mut ctl = CameraController::new(5.0, 1.0);
        ctl.fly_mode = fly;
        for k in keys {
            ctl.process_key(k, true);
        }
        let w = if fly {
            ctl.fly_wish_dir_up(&cam, radial)
        } else {
            ctl.surface_wish_dir(&cam)
        };
        DVec3::new(w.x as f64, w.y as f64, w.z as f64)
    }

    /// THE GATE (operator: "While I press W I immediately start flying").
    ///
    /// Pure lateral input must produce ZERO radial velocity change - not a
    /// small one, zero - at every pitch, including the two that used to leak
    /// hardest: nose down at your feet and nose up at the sky. Runs the real
    /// keyboard -> wish -> split -> integrate chain for ten seconds and
    /// demands the player never leaves the ground.
    #[test]
    fn walking_forward_never_lifts() {
        let radial = radial_at(35.0, 20.0);
        let up = DVec3::new(radial.x as f64, radial.y as f64, radial.z as f64);
        let rest = 6.371e6 + 1.75;
        for pitch in [-80.0_f32, -25.0, 0.0, 25.0, 80.0] {
            for keys in [&["KeyW"][..], &["KeyS"][..], &["KeyA"][..], &["KeyW", "KeyD"][..]] {
                let wish = wish_for(radial, pitch, keys, false);
                let (tangential, radial_wish) = split_wish(wish, up, MoveMode::Walk);
                assert_eq!(
                    radial_wish, 0.0,
                    "pitch {pitch} keys {keys:?}: lateral input produced a radial \
                     command of {radial_wish:e} - THIS is the lift, and an epsilon \
                     is not a fix for it"
                );
                assert!(
                    tangential.length() > 0.5,
                    "pitch {pitch} keys {keys:?}: the lateral part vanished ({:.3}); \
                     the fix must not cost you the ability to walk",
                    tangential.length()
                );
                // Ten seconds of held input: never a positive velocity, never
                // a metre of altitude.
                let (mut r, mut v) = (rest, 0.0);
                for _ in 0..600 {
                    let s = radial_step(
                        MoveMode::Walk, true, false, r, v, rest, G, SETTLE, DT,
                        radial_wish, 5.0 * DT,
                    );
                    r = s.r;
                    v = s.v_r;
                    assert!(v <= 0.0, "pitch {pitch} keys {keys:?}: gained climb speed {v}");
                    assert!(
                        r <= rest + 1e-9,
                        "pitch {pitch} keys {keys:?}: rose {:.3} m off the ground",
                        r - rest
                    );
                    assert!(s.grounded, "a walker on flat ground must stay grounded");
                }
            }
        }
    }

    /// The other half of the same claim: releasing the key must not leave
    /// MOMENTUM behind ("when I let go of W I maintain momentum briefly then
    /// fall"). Ten seconds of W then ten seconds of nothing: the velocity is
    /// zero at every single step, so there is nothing to coast on.
    #[test]
    fn releasing_forward_leaves_no_vertical_momentum() {
        let radial = radial_at(-35.0, 140.0);
        let up = DVec3::new(radial.x as f64, radial.y as f64, radial.z as f64);
        let rest = 6.371e6 + 1.75;
        let held = split_wish(wish_for(radial, 20.0, &["KeyW"], false), up, MoveMode::Walk).1;
        let released = split_wish(wish_for(radial, 20.0, &[], false), up, MoveMode::Walk).1;
        let (mut r, mut v) = (rest, 0.0);
        for frame in 0..1200 {
            let wish = if frame < 600 { held } else { released };
            let s = radial_step(
                MoveMode::Walk, true, false, r, v, rest, G, SETTLE, DT, wish, 5.0 * DT,
            );
            r = s.r;
            v = s.v_r;
            assert_eq!(v, 0.0, "frame {frame}: carried vertical momentum {v} m/s");
        }
        assert_eq!(r, rest, "the walker drifted off its rest height");
    }

    /// Do not over-fix: the jetpack must survive. Space is a DELIBERATE radial
    /// command and arrives as exactly +1 (Shift as exactly -1); Space+Shift
    /// cancel to exactly 0.
    #[test]
    fn deliberate_vertical_keys_still_command_exactly_one() {
        let radial = radial_at(0.0, 0.0);
        let up = DVec3::new(radial.x as f64, radial.y as f64, radial.z as f64);
        for (keys, want) in [
            (&["Space"][..], 1.0_f64),
            (&["ShiftLeft"][..], -1.0),
            (&["Space", "ShiftLeft"][..], 0.0),
            (&["KeyW", "Space"][..], 1.0),
        ] {
            let wish = wish_for(radial, -30.0, keys, false);
            let (_, radial_wish) = split_wish(wish, up, MoveMode::Walk);
            assert_eq!(radial_wish, want, "keys {keys:?} must command {want}");
        }
        // And it actually lifts: one second of Space at 5 m/s leaves the floor.
        let wish = split_wish(wish_for(radial, 0.0, &["Space"], false), up, MoveMode::Walk).1;
        let rest = 6.371e6 + 1.75;
        let (mut r, mut v) = (rest, 0.0);
        for _ in 0..60 {
            let s = radial_step(
                MoveMode::Walk, true, false, r, v, rest, G, SETTLE, DT, wish, 5.0 * DT,
            );
            r = s.r;
            v = s.v_r;
        }
        assert!(r > rest + 1.0, "Space no longer jumps (only {:.2} m up)", r - rest);
    }

    /// DEV FLIGHT HOLDS ITS ALTITUDE. Not "approximately", not "settles to":
    /// the operator wants to park the camera 5,000 ft up and photograph
    /// scenery, so a still stick must be a bit-identical radius forever.
    /// 36,000 frames is ten minutes of hovering.
    #[test]
    fn dev_flight_holds_altitude_with_no_input() {
        let radial = radial_at(46.0, 8.0);
        let up = DVec3::new(radial.x as f64, radial.y as f64, radial.z as f64);
        let rest = 6.371e6 + 1.75;
        let start = rest + 1524.0; // 5,000 ft
        // The engine's own idle wish, through the real controller.
        let (_, radial_wish) = split_wish(wish_for(radial, -12.0, &[], true), up, MoveMode::DevFlight);
        assert_eq!(radial_wish, 0.0, "an untouched stick must be an exact zero");
        let mut r = start;
        let mut v = 0.0;
        for frame in 0..36_000 {
            let s = radial_step(
                MoveMode::DevFlight, true, false, r, v, rest, G, SETTLE, DT,
                radial_wish, 50.0,
            );
            r = s.r;
            v = s.v_r;
            assert_eq!(r, start, "frame {frame}: hover drifted to {r} (want {start})");
            assert_eq!(v, 0.0, "frame {frame}: hover picked up vertical speed");
            assert!(!s.grounded, "a hovering camera must not report footsteps");
        }
    }

    /// Dev flight keeps the look-direction model that makes it useful: nose up
    /// 30 degrees and hold W to climb, nose down to descend. The gate is that
    /// this is now confined to dev flight instead of being the walk model.
    #[test]
    fn dev_flight_flies_where_you_look() {
        let radial = radial_at(10.0, -75.0);
        let up = DVec3::new(radial.x as f64, radial.y as f64, radial.z as f64);
        let rest = 6.371e6 + 1.75;
        for (pitch, want_sign) in [(30.0_f32, 1.0_f64), (-30.0, -1.0)] {
            let (_, radial_wish) =
                split_wish(wish_for(radial, pitch, &["KeyW"], true), up, MoveMode::DevFlight);
            assert!(
                (radial_wish - want_sign * 0.5).abs() < 0.01,
                "pitch {pitch}: expected a {want_sign} half-rate climb, got {radial_wish}"
            );
            let s = radial_step(
                MoveMode::DevFlight, true, false, rest + 1000.0, 0.0, rest, G, SETTLE, DT,
                radial_wish, 10.0 * DT,
            );
            assert!(
                (s.r - (rest + 1000.0)) * want_sign > 0.0,
                "pitch {pitch}: W did not move the camera the way it was looking"
            );
        }
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
