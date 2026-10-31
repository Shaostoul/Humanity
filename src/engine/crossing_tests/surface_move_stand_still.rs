//! From surface_move.rs (`stand_still_tests`): standing still on a planet, on the ground the engine's
//! frame lock reads (`frame_lock::ground_radius_m`).

//! THE REPRODUCTION (v0.1109.4). Operator, on v0.1109.3: "When I look up in
//! the air, I fall down, but when I look parallel with the earth I float up."
//!
//! A headless twin of the frame-lock's per-frame ground chain
//! (`lib.rs` 3998-4490), driven with NO INPUT at two frame rates. Everything
//! it calls is the shipped function the engine calls - `frame_lock_capture`,
//! `ground_radius_m`, `walk_ground_radius`, `radial_step` - in the shipped
//! order; the only test-side code is the sequencing, which is why each step
//! carries the lib.rs line it mirrors.

use crate::dev_travel::frame_lock_capture;
use crate::engine::frame_lock::ground_radius_m;
use crate::surface_move::ground_sample_dir;
use crate::surface_walk::{walk_ground_radius, DRAWN_CLEARANCE_M, LOD_CLEARANCE_M};
use crate::terrain::ocean_mask::OceanMask;
use crate::terrain::planet::PlanetDef;
use crate::terrain::planet_chunks as pc;
use crate::terrain::planet_chunks::DetailNoise;
use crate::terrain::planet_heightmap::PlanetHeightmap;
use glam::{DQuat, DVec3};

/// A day in 20 real minutes (the 24-hour day at the Simplified time
/// speed, 72x; the fastest setting is faster still), so the ground under
/// a standing player slips 2*PI*R/1200 = 33,360 m/s at the equator. This
/// is the number that turns a one-frame sampling lag into kilometres.
const DAY_S: f64 = 1200.0;
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
        let rest = crate::surface_move::rest_height(field, DRAWN_CLEARANCE_M);
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
        let anchor_pre = frame_lock_capture(DVec3::ZERO, DQuat::from_rotation_y(self.spin), cam_world);
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
        let rest = crate::surface_move::rest_height(g, clearance);
        let vs = crate::surface_move::radial_step(
            crate::surface_move::MoveMode::Walk,
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
    let proj = crate::terrain::celestial_projection::celestial_projection_f64(cam.fov_degrees as f64, cam.aspect as f64);
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
    let pre = frame_lock_capture(DVec3::ZERO, DQuat::from_rotation_y(lag), DQuat::from_rotation_y(0.0) * carried);
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
