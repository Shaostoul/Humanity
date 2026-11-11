//! FEET ON THE WORLD, ON THE SHIPPED BODIES (character animation rung 2a, 2026-11-09;
//! docs/design/character-animation.md section 4). Each test walks a real MakeHuman body through
//! its real (retargeted) clips over a test ground, puts its feet on the ground the way the frame
//! does (`FootPlacer::place`), and then measures the DRAWN feet: the foot vertices skinned on the
//! CPU with the very palette rows the GPU would get (`foot_probe::skinned_feet`), never the IK's own
//! targets. Each test also runs the same walk with the feet off (rung 1's pose as the clips made
//! it) and holds that it FAILS the same bounds, so a bound that cannot fail is caught here.

use glam::{Mat4, Quat, Vec3};
use humanity_animation::{FeetDef, FootFrame, FootMode, FootPlacer, Ground, LoadedBody, Pose};

use super::*;
use crate::engine::feet_ground::TestGround;

fn repo_data() -> PathBuf {
    Path::new(env!("HUMANITY_REPO_ROOT")).join("data")
}

fn body<'a>(lib: &'a CharacterLibrary, choice: &str) -> &'a LoadedBody {
    &lib.bodies[lib.body_index(choice).unwrap_or_else(|| panic!("no body {choice}"))]
}

/// What one walk measured (after its first half second).
#[derive(Debug, Default)]
struct Measured {
    /// The planted frames' clearances (lowest foot vertex over the ground under it, metres).
    lowest: f32,
    highest: f32,
    planted_frames: usize,
    /// The most the ground under one planted foot's vertices spread (a foot across a riser).
    straddle: f32,
    /// The most a planted foot's centre slid along the ground during one plant, metres.
    drift: f32,
    /// The lowest a foot that is NOT planted reaches against the ground under each of its vertices
    /// (a swing's toe tips through the step above read negative), metres.
    swing_lowest: f32,
}

/// Walk `clip` on `b` along +X for `secs` (the root at `speed` metres a second, on `root_y`'s height
/// at each x), the feet placed in `mode` over `ground`, and measure the drawn feet.
#[allow(clippy::too_many_arguments)]
fn walk(lib: &CharacterLibrary, b: &LoadedBody, clip_id: &str, speed: f32, root_y: &dyn Fn(f32) -> f32, ground: &dyn Ground, mode: FootMode, def: &FeetDef, secs: f32) -> Measured {
    walk_path(lib, b, clip_id, speed, Vec3::ZERO, Vec3::X, &|p| root_y(p.x), ground, &|_| {}, mode, def, secs, None)
}

/// `walk` from `start` along the horizontal unit `dir`, the root's height `root_y` of its place, and
/// `set_root` told the root's height each frame before the feet are placed (a footing's step-up
/// reference).
#[allow(clippy::too_many_arguments)]
fn walk_path(
    lib: &CharacterLibrary,
    b: &LoadedBody,
    clip_id: &str,
    speed: f32,
    start: Vec3,
    dir: Vec3,
    root_y: &dyn Fn(Vec3) -> f32,
    ground: &dyn Ground,
    set_root: &dyn Fn(f32),
    mode: FootMode,
    def: &FeetDef,
    secs: f32,
    figure: Option<(&CrowdFigure, f32)>,
) -> Measured {
    let skel = &b.skeleton;
    let rig = b.feet.as_ref().expect("the body has legs");
    let clip = b.clips.get(b.clips.index(clip_id).unwrap_or_else(|| panic!("no clip {clip_id}")));
    let facing = Quat::from_rotation_y(dir.x.atan2(dir.z)); // its front along `dir`
    // Sixty frames a second, or a stand-in path's own rate (the rigs run at 30).
    let dt = figure.map_or(1.0 / 60.0, |(_, fps)| 1.0 / fps);
    let figure = figure.map(|(f, _)| f);
    let mut placer = FootPlacer::default();
    let (mut pose, mut model, mut rows) = (Pose::default(), Vec::new(), Vec::new());
    let mut m = Measured { lowest: f32::INFINITY, highest: f32::NEG_INFINITY, swing_lowest: f32::INFINITY, ..Default::default() };
    // Each foot's vertices and their clearances last frame, and the slide so far in this plant.
    let mut last: [Option<Vec<(Vec3, f32)>>; 2] = [None, None];
    let mut slid = [0.0_f32; 2];
    let steps = (secs / dt) as usize;
    // Driven by the game's own graph along a dev stand-in's path (`figure`): its place from
    // `crowd_place`, its speed and turn read by the motion tracker, the pose and contacts from the
    // animator (stride warping included), exactly as `draw_people` drives a stand-in.
    let mut animator = Animator::new(&lib.graph);
    let mut tracker = MotionTracker::default();
    for k in 0..steps {
        let t = k as f32 * dt;
        let (world, contacts) = match figure {
            Some(fig) => {
                let (feet, face) = crowd_place(fig, t);
                set_root(feet.y);
                let input = tracker.observe(feet, face, dt, lib.graph.smoothing_s, lib.graph.teleport_mps);
                animator.update(&lib.graph, &b.clips, skel, &lib.humanoid, &input, dt, &mut pose);
                (Mat4::from_rotation_translation(Quat::from_rotation_y(face), feet), animator.contacts())
            }
            None => {
                let at = start + dir * speed * t;
                let y = root_y(at);
                set_root(y);
                clip.sample(skel, t, &mut pose);
                humanity_animation::retarget::apply_twists(skel, &lib.humanoid, &mut pose);
                (Mat4::from_rotation_translation(facing, Vec3::new(at.x, y, at.z)), clip.contacts_at(t))
            }
        };
        pose::model_matrices(skel, &pose, &mut model);
        let frame = FootFrame { world, contacts, airborne: false, dt, pelvis_fixed: false, seat_plant: false };
        placer.place(rig, skel, &lib.humanoid, &mut pose, &mut model, &frame, ground, mode, def);
        rows.clear();
        pose::palette_rows(skel, &model, &mut rows);
        let feet = crate::engine::foot_probe::skinned_feet(&b.model, &b.foot_vertices, &[], &rows, &world);
        if t < 0.5 {
            continue;
        }
        for i in 0..2 {
            if contacts[i] < 0.9 {
                last[i] = None;
                slid[i] = 0.0;
                // The swing: every vertex against the ground under it (a toe through a riser).
                let low = feet[i].iter().filter_map(|&p| ground.ground(p).map(|g| p.y - g.point.y)).reduce(f32::min);
                if let Some(low) = low {
                    if low < -0.03 && mode == FootMode::Full && std::env::var("FT_DEBUG").is_ok() {
                        let worst = feet[i].iter().filter_map(|&p| ground.ground(p).map(|g| (p, p.y - g.point.y))).min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
                        let soles: Vec<(Vec3, Option<f32>)> = rig.sole_points(i, &model).iter().map(|&s| world.transform_point3(s)).map(|s| (s, ground.ground(s).map(|g| s.y - g.point.y))).collect();
                        eprintln!("t {t:.2} foot {i} (swinging, contact {:.2}): vertex {:?} {:.3} m under the ground; sole points (world, clearance) {soles:?}; report {:?}", contacts[i], worst.0, -low, placer.report.feet[i]);
                    }
                    m.swing_lowest = m.swing_lowest.min(low);
                }
                continue;
            }
            // Each vertex with the ground under it: (vertex, clearance, ground height).
            let rows: Vec<(Vec3, f32, f32)> = feet[i].iter().filter_map(|&p| ground.ground(p).map(|g| (p, p.y - g.point.y, g.point.y))).collect();
            if rows.len() != feet[i].len() {
                continue;
            }
            let Some(c) = rows.iter().map(|r| r.1).reduce(f32::min) else { continue };
            if c < -0.05 && mode == FootMode::Full && std::env::var("FT_DEBUG").is_ok() {
                let worst = rows.iter().min_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
                let xs: (f32, f32) = (rows.iter().map(|r| r.0.x).fold(f32::INFINITY, f32::min), rows.iter().map(|r| r.0.x).fold(f32::NEG_INFINITY, f32::max));
                eprintln!("t {t:.2} foot {i}: clearance {c:.3} at {:?} over ground {:.3}; foot spans x {:.3}..{:.3}; report {:?}", worst.0, worst.2, xs.0, xs.1, placer.report.feet[i]);
            }
            m.lowest = m.lowest.min(c);
            m.highest = m.highest.max(c);
            m.planted_frames += 1;
            // The sole (the vertices in the foot's lowest 3 cm, by height): a sole lying across a
            // riser, its heel hanging over the tread below, stands over two heights.
            let low_y = rows.iter().map(|r| r.0.y).fold(f32::INFINITY, f32::min);
            let sole: Vec<f32> = rows.iter().filter(|r| r.0.y < low_y + 0.03).map(|r| r.2).collect();
            let spread = sole.iter().copied().fold(f32::NEG_INFINITY, f32::max) - sole.iter().copied().fold(f32::INFINITY, f32::min);
            if spread > 0.05 && mode == FootMode::Full && std::env::var("FT_DEBUG").is_ok() {
                let xs: (f32, f32) = (rows.iter().map(|r| r.0.x).fold(f32::INFINITY, f32::min), rows.iter().map(|r| r.0.x).fold(f32::NEG_INFINITY, f32::max));
                eprintln!("t {t:.2} foot {i}: sole across {spread:.3} m; foot spans x {:.3}..{:.3}, lowest at y {low_y:.3}; report {:?}", xs.0, xs.1, placer.report.feet[i]);
            }
            m.straddle = m.straddle.max(spread);
            // The slide: how far the vertices touching the ground (within a centimetre, this frame
            // and last) moved along it since last frame, added up over the plant. A foot rolling
            // from heel to ball changes which vertices touch, and that is no slide.
            let now: Vec<(Vec3, f32)> = rows.iter().map(|r| (r.0, r.1)).collect();
            if let Some(prev) = &last[i] {
                let pmin = prev.iter().map(|p| p.1).fold(f32::INFINITY, f32::min);
                let moved: Vec<f32> = now.iter().zip(prev).filter(|(a, b)| a.1 < c + 0.01 && b.1 < pmin + 0.01).map(|(a, b)| Vec3::new(a.0.x - b.0.x, 0.0, a.0.z - b.0.z).length()).collect();
                if !moved.is_empty() {
                    let step = moved.iter().sum::<f32>() / moved.len() as f32;
                    if step > 0.03 && mode == FootMode::Full && std::env::var("FT_DEBUG").is_ok() {
                        eprintln!("t {t:.2} foot {i}: the touching vertices moved {step:.3} m this frame (slid {:.3} this plant); report {:?}", slid[i] + step, placer.report.feet[i]);
                    }
                    slid[i] += step;
                    m.drift = m.drift.max(slid[i]);
                }
            }
            last[i] = Some(now);
        }
    }
    m
}

/// EVERY GAIT'S PLANTED FEET REST ON A FLAT FLOOR, on both MakeHuman bodies: idle, walk, jog,
/// sprint, crouch idle and crouch walk, each walked at its own speed with the feet on (the near
/// tier's full solve), every planted foot's lowest drawn vertex within a centimetre under to two
/// over the floor, and no planted foot slides more than 2 cm. With the feet off the same walks sink
/// (sprint 5.1 cm on the female body, 4.8 on the male) and slide (a walk 9 to 10 cm, a crouch walk
/// 17 to 18), which the control below holds, so the bound can fail. Measured 2026-11-09 with the
/// feet on: planted between 0.2 cm under and 1.1 cm over the floor, sliding at most 1.5 cm (the
/// eprintln lines: `just test-lib feet_rest --nocapture`).
/// Seen red 2026-11-09 on the first build (two sole points, under the ankle and the ball, and the
/// lock on the ankle): "female walk (full): a planted foot reaches 0.011 m under the floor" (the
/// back of the heel at a heel strike), and that walk's planted foot slid 0.050 m.
#[test]
fn feet_rest_on_a_flat_floor_in_every_gait() {
    let lib = read_library(&repo_data()).unwrap();
    let def = lib.characters.feet;
    let floor = TestGround { floor: 0.0, stair: None, slope: 0.0 };
    for choice in ["female", "male"] {
        let b = body(&lib, choice);
        let mut off_sprint = 0.0_f32;
        for id in ["idle", "walk", "jog", "sprint", "crouch_idle", "crouch_walk"] {
            let speed = b.clips.get(b.clips.index(id).unwrap()).speed_mps;
            let full = walk(&lib, b, id, speed, &|_| 0.0, &floor, FootMode::Full, &def, 3.0);
            let off = walk(&lib, b, id, speed, &|_| 0.0, &floor, FootMode::Off, &def, 3.0);
            eprintln!(
                "{choice} {id}: planted lowest {:+.3} / highest {:+.3} m, drift {:.3} m over {} frames (feet off: {:+.3} / {:+.3}, drift {:.3})",
                full.lowest, full.highest, full.drift, full.planted_frames, off.lowest, off.highest, off.drift
            );
            assert!(full.planted_frames > 20, "{choice} {id}: only {} planted frames", full.planted_frames);
            assert!(full.lowest > -0.01, "{choice} {id} (full): a planted foot reaches {:.3} m under the floor", -full.lowest);
            assert!(full.highest < 0.02, "{choice} {id} (full): a planted foot hovers {:.3} m over the floor", full.highest);
            assert!(full.drift < 0.02, "{choice} {id} (full): a planted foot slides {:.3} m", full.drift);
            if id == "sprint" {
                off_sprint = off.lowest;
            }
        }
        assert!(off_sprint < -0.03, "{choice}: the control (feet off) should sink in the sprint, read {off_sprint:.3}");
    }
}

/// FEET ON STAIRS, A SLOPE AND A MISMATCHED STRIDE, on the female body's walk:
/// - UP A STAIR (the homestead's: 16 treads of 0.281 m rising 0.1875, laid along +X), the root
///   gliding up the line from the first nosing to the last as the dev stand-in's path does (so it
///   is up to a riser under each tread): every planted foot rests on a tread, its lowest drawn vertex
///   within a centimetre under to two over the ground under it, so no toe or heel reaches into the
///   step above (the footstep planner put the foot on the tread's middle, its end up the stair clear
///   of the next riser). The feet off sink through the treads (control), and with the planner off
///   toes go into the step above (control).
/// - UP A 20% SLOPE: every planted foot rests on the slope (turned to it about its pivot).
/// - WALKING A QUARTER FASTER THAN THE CLIP'S STRIDE (stride warping at its limit): a planted foot
///   holds still (locked) to within a centimetre and a half; with locking broken at once (control)
///   it slides.
/// Seen red 2026-11-09 three times: the first build (the footstep moved at touchdown and eased in
/// over the stance), "stair (full): a planted foot reaches 0.131 m under a tread" and a 10 cm slide;
/// with the move worked out in the swing but eased by its own value (it lagged the sweeping foot),
/// toe tips 0.6 mm past the nosing: "a planted foot reaches 0.185 m under a tread"; and with the
/// tread's middle eased (it lagged the last nosing), heels over the tread below: "a planted sole
/// lies across a riser (0.188 m)". And on the slope, with the toe tips read at the ball's ground
/// height rather than along the slope's plane: "slope (full): a planted foot reaches 0.017 m into
/// the slope".
#[test]
fn feet_land_on_treads_turn_to_slopes_and_hold_still() {
    let lib = read_library(&repo_data()).unwrap();
    let def = lib.characters.feet;
    let b = body(&lib, "female");
    let walk_speed = b.clips.get(b.clips.index("walk").unwrap()).speed_mps;
    // The stair, from x = 0.5, and the root up the nosing line (0 at its foot, 3.0 at its head).
    let (x0, tread, riser, n) = (0.5_f32, 0.28125_f32, 0.1875_f32, 16usize);
    let stair = TestGround { floor: 0.0, stair: Some((x0, tread, riser, n)), slope: 0.0 };
    let up_line = move |x: f32| ((x - x0) / tread * riser).clamp(0.0, riser * n as f32);
    let full = walk(&lib, b, "walk", walk_speed, &up_line, &stair, FootMode::Full, &def, 4.5);
    let off = walk(&lib, b, "walk", walk_speed, &up_line, &stair, FootMode::Off, &def, 4.5);
    let unplanned = walk(&lib, b, "walk", walk_speed, &up_line, &stair, FootMode::Full, &FeetDef { plan_steps: false, ..def }, 4.5);
    eprintln!("stair: full {full:?}\n       off {off:?}\n       unplanned {unplanned:?}");
    assert!(full.planted_frames > 40);
    assert!(full.lowest > -0.01, "stair (full): a planted foot reaches {:.3} m under a tread", -full.lowest);
    assert!(full.highest < 0.02, "stair (full): a planted foot hovers {:.3} m over a tread", full.highest);
    assert!(off.lowest < -0.05, "stair control (feet off) should sink through the treads, read {:.3}", off.lowest);
    assert!(unplanned.lowest < -0.05, "stair control (no planner) should put toes into the step above, read {:.3}", unplanned.lowest);

    let slope = TestGround { floor: 0.0, stair: None, slope: 0.2 };
    let on_slope = |x: f32| 0.2 * x.max(0.0);
    let full = walk(&lib, b, "walk", walk_speed, &on_slope, &slope, FootMode::Full, &def, 3.5);
    let off = walk(&lib, b, "walk", walk_speed, &on_slope, &slope, FootMode::Off, &def, 3.5);
    eprintln!("slope: full {full:?}\n       off {off:?}");
    assert!(full.lowest > -0.012, "slope (full): a planted foot reaches {:.3} m into the slope", -full.lowest);
    assert!(full.highest < 0.02, "slope (full): a planted foot hovers {:.3} m over the slope", full.highest);
    assert!(off.lowest < -0.02 || off.highest > 0.03, "slope control (feet off) should sink or hover, read {off:?}");

    let floor = TestGround { floor: 0.0, stair: None, slope: 0.0 };
    let fast = walk(&lib, b, "walk", walk_speed * 1.25, &|_| 0.0, &floor, FootMode::Full, &def, 3.0);
    let unlocked = walk(&lib, b, "walk", walk_speed * 1.25, &|_| 0.0, &floor, FootMode::Full, &FeetDef { lock_break_m: 0.0, ..def }, 3.0);
    eprintln!("stride: locked {fast:?}\n        unlocked {unlocked:?}");
    assert!(fast.drift < 0.015, "a locked planted foot slides {:.3} m", fast.drift);
    assert!(unlocked.drift > 0.03, "the control (no locking) should slide, read {:.3}", unlocked.drift);
}

/// THE SHIPPED CLIPS ARE LABELLED WITH THEIR CONTACTS, on both bodies: standing, both feet are
/// planted nearly all the time; walking, each foot about half to three quarters of the time, the
/// two alternating (each has frames planted alone); running, each foot planted for a few frames and
/// less than half the time (there is a flight; Quaternius's runs plant a foot for 10 to 14% of their
/// cycle on these bodies, measured 2026-11-09); in the jump's air loop (tagged `air`), neither.
/// Seen red 2026-11-09 twice: with height alone deciding (the first labeller), "female jump_air: a
/// foot planted 61% of the time in the air" (its hanging right foot); and with two sole points and
/// the vertical-speed limit fixed at a walk's, "female sprint: [0.0952381, 0.0]" (the right foot
/// never planted).
#[test]
fn the_shipped_clips_are_labelled_with_their_contacts() {
    let lib = read_library(&repo_data()).unwrap();
    for choice in ["female", "male"] {
        let b = body(&lib, choice);
        let share = |id: &str| -> ([f32; 2], f32) {
            let c = &b.clips.get(b.clips.index(id).unwrap()).contacts;
            assert!(!c.is_empty(), "{choice} {id}: no contacts labelled");
            let n = c.len() as f32;
            let alone = c.iter().filter(|f| (f[0] > 0.5) != (f[1] > 0.5)).count() as f32 / n;
            ([c.iter().filter(|f| f[0] > 0.5).count() as f32 / n, c.iter().filter(|f| f[1] > 0.5).count() as f32 / n], alone)
        };
        for id in ["idle", "walk", "jog", "sprint", "crouch_walk", "jump_air"] {
            let (s, alone) = share(id);
            eprintln!("{choice} {id}: planted left {:.0}%, right {:.0}%, one foot alone {:.0}%", s[0] * 100.0, s[1] * 100.0, alone * 100.0);
        }
        let (idle, _) = share("idle");
        assert!(idle[0] > 0.9 && idle[1] > 0.9, "{choice} idle: {idle:?}");
        let (walk, alone) = share("walk");
        assert!(walk.iter().all(|s| (0.4..0.85).contains(s)), "{choice} walk: {walk:?}");
        assert!(alone > 0.3, "{choice} walk: the feet alternate ({alone})");
        for id in ["jog", "sprint"] {
            let (run, _) = share(id);
            assert!(run.iter().all(|s| *s > 0.05 && *s < 0.5), "{choice} {id}: {run:?}");
        }
        let (air, _) = share("jump_air");
        assert!(air.iter().all(|s| *s < 0.01), "{choice} jump_air: a foot planted {:.0}% of the time in the air", air[0].max(air[1]) * 100.0);
    }
}

/// THE SHIPPED HOMESTEAD'S OWN STAIR, through the real footing (`engine::feet_ground::ShipGround`
/// over `Footing`, the person's root as the step-up reference, `Footing::tread_at` for the treads'
/// middles): both bodies walk the anim-stairs vantage's stand-in path (west up stairs-1 from its foot
/// at x 41.0 to its head at 36.5, the root gliding up the line of the nosings at the path's 0.75 m/s
/// across the ground) and every planted foot rests on its tread, its lowest drawn vertex within a
/// centimetre under to two over the ground under it (nothing in the step above); with the feet off
/// it sinks (control).
/// Seen red 2026-11-09 twice: with the footstep planned for the foot as it touches down (pitched
/// toes-up, so shorter than when it flattens), "male (full): a planted foot reaches 0.181 m under a
/// tread", its toe tips 4 mm past the nosing of a 28 cm tread (the anim-stairs rig read the female's
/// left foot 0.176 m deep the same way); and with the flat foot exactly centred, 0.178 m (the man's
/// 26 cm foot leaves 1 cm either side, and the touchdown came 1.5 cm forward), which is why the
/// planner keeps the end pointing up the stair clear of the next riser instead. And driven along the
/// vantage's own path at the rig's 30 frames a second, with the touchdown frame's plan taken from the
/// frame before: "female, that path at 30 frames a second (full): a planted foot reaches 0.169 m
/// under a tread" (60 frames a second passed). And no SWINGING foot goes into a step: with the
/// swing reading only the planes under its heel and ball (the second rig sweep's build), "female,
/// the clip at its own pace (full): a swinging foot goes 0.166 m into a step", the toe tips through
/// the riser above as the foot swung over a nosing (the rig read 15 to 16 cm the same way); with
/// all four sole points reading their own ground, still "a swinging foot goes 0.094 m into a step":
/// the toe cap's front, a vertex 1.4 cm ahead of the toe tip point with the toes pitched down, hence
/// `FootRig::toe_ahead`. And the path's whole round (the rig's probe window can fall on the way
/// down): "female, the path's whole round, up and down, at 30 frames a second (full): a swinging foot
/// goes 0.063 m into a step", the heel counter behind the heel's back point cutting into the riser
/// of the step the foot was leaving (the fourth rig sweep read 12.4 cm), hence `heel_behind`.
#[test]
fn feet_land_on_the_homesteads_own_stair() {
    use std::cell::Cell;
    let lib = read_library(&repo_data()).unwrap();
    let def = lib.characters.feet;
    let w = crate::engine::footing::test_aboard::Aboard::homestead();
    struct Ref<'a> {
        footing: crate::engine::footing::Footing<'a>,
        root: Cell<f32>,
    }
    impl Ground for Ref<'_> {
        fn ground(&self, at: Vec3) -> Option<humanity_animation::GroundHit> {
            crate::engine::feet_ground::ShipGround { footing: self.footing, reference: self.root.get() }.ground(at)
        }
    }
    let ground = Ref { footing: w.footing(), root: Cell::new(0.0) };
    let up_line = |p: Vec3| ((41.0 - p.x) / 4.5 * 3.0).clamp(0.0, 3.0);
    for choice in ["female", "male"] {
        let b = body(&lib, choice);
        eprintln!("{choice}: the toe cap ahead of the toe tips and the heel counter behind the heel, -90 to 90 degrees: {:?}", b.feet.as_ref().map(|r| (r.toe_ahead[0], r.heel_behind[0])));
        let set = |y: f32| ground.root.set(y);
        let full = walk_path(&lib, b, "walk", 0.75, Vec3::new(41.6, 0.0, 38.65), -Vec3::X, &up_line, &ground, &set, FootMode::Full, &def, 6.0, None);
        let off = walk_path(&lib, b, "walk", 0.75, Vec3::new(41.6, 0.0, 38.65), -Vec3::X, &up_line, &ground, &set, FootMode::Off, &def, 6.0, None);
        // And driven exactly as the anim-stairs vantage drives its stand-in: its own path through
        // the game's graph (stride warping at the path's slower pace up the slope).
        let fig = parse_path("42.6,0,38.65;41.0,0,38.65;36.5,3.0,38.65;35.5,3.0,38.65;0.9").expect("the vantage's path");
        let graph = walk_path(&lib, b, "walk", 0.0, Vec3::ZERO, Vec3::X, &|_| 0.0, &ground, &set, FootMode::Full, &def, 8.0, Some((&fig, 60.0)));
        let graph30 = walk_path(&lib, b, "walk", 0.0, Vec3::ZERO, Vec3::X, &|_| 0.0, &ground, &set, FootMode::Full, &def, 8.0, Some((&fig, 30.0)));
        // And the whole round the rig sees: up, the turn on the landing, down, the turn at the foot.
        let round = walk_path(&lib, b, "walk", 0.0, Vec3::ZERO, Vec3::X, &|_| 0.0, &ground, &set, FootMode::Full, &def, 20.0, Some((&fig, 30.0)));
        eprintln!("homestead stair, {choice}: full {full:?}\n                         off {off:?}\n                         graph {graph:?}\n                         graph at 30 fps {graph30:?}\n                         the round at 30 fps {round:?}");
        for (what, m) in [("the clip at its own pace", &full), ("the vantage's path through the graph", &graph), ("that path at 30 frames a second", &graph30), ("the path's whole round, up and down, at 30 frames a second", &round)] {
            assert!(m.planted_frames > 60, "{choice}, {what}: {} planted frames", m.planted_frames);
            assert!(m.lowest > -0.01, "{choice}, {what} (full): a planted foot reaches {:.3} m under a tread", -m.lowest);
            assert!(m.highest < 0.02, "{choice}, {what} (full): a planted foot hovers {:.3} m over a tread", m.highest);
            assert!(m.swing_lowest > -0.01, "{choice}, {what} (full): a swinging foot goes {:.3} m into a step", -m.swing_lowest);
        }
        assert!(off.lowest < -0.05, "{choice}: the control (feet off) should sink, read {:.3}", off.lowest);
    }
}

/// A PERSON'S OWN RIG CARRIES WHAT THE BODY MEASURED: the game builds each person's legs on their
/// reshaped skeleton (`person_rig`, as `draw_people` does), and what the body's mesh measured at
/// load (the reach, the toe cap ahead of the toe tips, the heel counter behind the heel) must come
/// with it. The control, the construction before this test (the reach copied alone), is checked to
/// lose the toe cap and the heel counter: that is the build the fourth and fifth rig sweeps ran,
/// anim-stairs' swinging feet 8 to 12 cm into a step while every walk here, on the body's own rig,
/// passed.
#[test]
fn a_persons_own_rig_carries_what_the_body_measured() {
    let lib = read_library(&repo_data()).unwrap();
    for choice in ["female", "male"] {
        let b = body(&lib, choice);
        let made = b.feet.as_ref().expect("the body has legs");
        let skel = b.reshaped(&[(0, 0.5)]).expect("a shape target reshapes the skeleton");
        let mine = person_rig(b, &skel, &lib.humanoid).expect("the person has legs");
        assert!(made.toe_ahead[0].iter().any(|&a| a > 0.005) && made.heel_behind[0].iter().any(|&a| a > 0.005), "{choice}: the body measured a toe cap and a heel counter");
        assert_eq!(mine.reach, made.reach, "{choice}: the reach");
        assert_eq!(mine.toe_ahead, made.toe_ahead, "{choice}: the toe cap");
        assert_eq!(mine.heel_behind, made.heel_behind, "{choice}: the heel counter");
        // The control: the reach copied alone.
        let old = humanity_animation::FootRig::new(&skel, &lib.humanoid).map(|mut r| {
            r.reach = made.reach;
            r
        });
        assert!(old.is_some_and(|r| r.toe_ahead != made.toe_ahead && r.heel_behind != made.heel_behind), "{choice}: the control should lose the toe cap and the heel counter");
    }
}
