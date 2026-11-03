//! From systems/wounds_tests.rs: the falls, walked with the renderer's camera and controller.

use humanity_systems::surface_move::{radial_step, MoveMode};
use humanity_systems::food::fixtures::wounds::SURFACE_SETTLE;
use humanity_core::input::bindings::GameAction;
use crate::renderer::camera::Camera;
use crate::renderer::camera::CameraController;
use crate::renderer::camera::JUMP_SPEED_MPS;
use crate::renderer::camera::WALK_SPEED_MPS;
use humanity_systems::food::fixtures::wounds::EARTH_G;
use humanity_systems::food::fixtures::wounds::FRAME_S;
use humanity_systems::food::fixtures::wounds::free_fall;
use humanity_systems::food::fixtures::wounds::ids;
use humanity_systems::food::fixtures::wounds::land;
use humanity_systems::illness::Mode;
use humanity_core::ecs::components::StatusEffects;
use humanity_systems::food::fixtures::wounds::MOON_G;
use humanity_systems::food::fixtures::wounds::registry;
use humanity_systems::food::fixtures::wounds::shipped;
use humanity_systems::food::fixtures::wounds::shipped_falls;

/// A walker aboard under gravity `g`, standing on a deck `deck_m` above the floor, a few frames
/// in (so the walk knows where it left the body).
fn aboard(g: f32, deck_m: f32) -> (CameraController, Camera) {
    let mut ctl = CameraController::new(WALK_SPEED_MPS, 1.0);
    ctl.set_interior_gravity(g);
    ctl.set_ground_floor(deck_m);
    let mut cam = Camera::new();
    cam.position.y = deck_m + ctl.eye_height();
    for _ in 0..5 {
        ctl.update_camera(&mut cam, FRAME_S);
    }
    assert_eq!(ctl.take_landing(), None, "standing on the deck is not a fall");
    (ctl, cam)
}

/// Run the walk at `dt` until the body stands on its floor (a minute at most): the landing
/// lib.rs would take.
fn until_landed(ctl: &mut CameraController, cam: &mut Camera, dt: f32) -> Option<f32> {
    let floor = ctl.ground_floor() + ctl.eye_height();
    for _ in 0..(60.0 / dt) as usize {
        ctl.update_camera(cam, dt);
        if cam.position.y <= floor {
            break;
        }
    }
    ctl.take_landing()
}

/// Walk off the edge of a deck `drop_m` high, aboard under gravity `g`, at 60 frames a second
/// (the main loop lowers the floor under a player who walks off an edge, as `set_ground_floor`
/// does here): the landing lib.rs would take.
fn step_off(drop_m: f32, g: f32) -> Option<f32> {
    let (mut ctl, mut cam) = aboard(g, drop_m);
    ctl.set_ground_floor(0.0);
    until_landed(&mut ctl, &mut cam, FRAME_S)
}

/// A STEP, OR THE GAME'S OWN JUMP, HURTS NOBODY. Stepping down half a metre aboard lands at
/// 3.1 m/s, and the game's jump comes back down at about its launch speed (5 m/s): neither
/// leaves a wound or takes health, in either mode, and nothing is said.
///
/// Seen red with the thresholds halved in `Falls::wound_at` and `Falls::harm_at` (the same run
/// also reported each landing at the frame's last velocity, hence 3.11): "Realistic: a 0.5 m step
/// (3.11 m/s) left [\"sprained_ankle\"], took 0.0, said \"You land hard. Your ankle turned under
/// you as you landed: a sprain. ...\"".
#[test]
fn a_half_metre_step_and_a_jump_hurt_nobody() {
    let step = step_off(0.5, EARTH_G).expect("a half-metre step is a landing");
    // The jump: Space on the floor for a frame; it comes back down where it left.
    let (mut ctl, mut cam) = aboard(EARTH_G, 0.0);
    ctl.apply_action(GameAction::Jump, true);
    ctl.update_camera(&mut cam, FRAME_S);
    ctl.apply_action(GameAction::Jump, false);
    let jump = until_landed(&mut ctl, &mut cam, FRAME_S).expect("a jump lands");
    for mode in [Mode::Realistic, Mode::Forgiving] {
        for (what, v) in [("a 0.5 m step", step), ("the game's jump", jump)] {
            let l = land(mode, Some(v));
            assert!(
                l.fx.active.is_empty() && l.took == 0.0 && l.said.is_empty(),
                "{mode:?}: {what} ({v:.2} m/s) left {:?}, took {:.1}, said {:?}",
                ids(&l.fx),
                l.took,
                l.said
            );
        }
    }
    assert!((step - free_fall(0.5, EARTH_G)).abs() < 1e-3, "the step landed at {step:.3} m/s, free fall from 0.5 m is {:.3}", free_fall(0.5, EARTH_G));
    assert!((jump - JUMP_SPEED_MPS).abs() < 0.2, "the jump landed at {jump:.3} m/s, its launch speed is {JUMP_SPEED_MPS}");
}

/// DEV FLIGHT HURTS NOBODY. Aboard, flying 20 m up in fly mode and switching it off in the air:
/// the fall that follows is the dev tool's, and reports nothing. On a planet the same for F9's
/// hover. The same 20 m fall walked off an edge reports a landing that kills, so it is the
/// flight that spared it.
///
/// Seen red with the fly branch of `update_first_person` not marking the fall exempt: "fly mode
/// switched off 20.0 m up reported a landing: Some(19.8091)". (The planet half caught a real
/// defect on its first run: `surface_step` ended the watch on the frame the falling walker came
/// within a boot's reach of the ground, one frame before it landed; it now waits for the landing.)
#[test]
fn dev_flight_hurts_nobody() {
    let (mut ctl, mut cam) = aboard(EARTH_G, 0.0);
    ctl.fly_mode = true;
    ctl.apply_action(GameAction::Jump, true);
    let floor = cam.position.y;
    for _ in 0..600 {
        ctl.update_camera(&mut cam, FRAME_S);
        if cam.position.y - floor >= 20.0 {
            break;
        }
    }
    ctl.apply_action(GameAction::Jump, false);
    let up = cam.position.y - floor;
    assert!(up >= 20.0, "the setup: flew {up:.1} m up");
    ctl.fly_mode = false;
    let fell = until_landed(&mut ctl, &mut cam, FRAME_S);
    assert_eq!(fell, None, "fly mode switched off {up:.1} m up reported a landing: {fell:?}");
    let hover = fall_on_planet(20.0, 9.81, f64::from(FRAME_S), false, 30);
    assert_eq!(hover, None, "F9's hover let go 20 m up on a planet reported a landing: {hover:?}");
    let walked = step_off(20.0, EARTH_G).expect("walking off a 20 m edge is a landing");
    assert!(land(Mode::Realistic, Some(walked)).died.is_some(), "the same 20 m fall walked off an edge kills");
}

/// SOMETHING ELSE MOVING THE BODY IS NOT A FALL. The game puts the camera somewhere every frame
/// in some states, after the walk has run (the follow camera hangs 4 m above a vehicle, the cab
/// rides one, the rig's camera hold, a teleport): the walk's own fall speed then builds frame
/// after frame and it "lands" every so often. Five minutes of that report nothing, and a drop
/// after being put 10 m up (a teleport) reports nothing either.
///
/// Seen red with the placement check in `update_camera` removed: "five minutes under the follow
/// camera reported landings: [6.717589, 6.717589, ...]", 21 of them.
#[test]
fn something_else_moving_the_body_is_not_a_fall() {
    let (mut ctl, mut cam) = aboard(EARTH_G, 0.0);
    let hung = 4.0;
    let mut reported = Vec::new();
    for _ in 0..(60 * 300) {
        ctl.update_camera(&mut cam, FRAME_S);
        cam.position.y = hung;
        reported.extend(ctl.take_landing());
    }
    assert!(reported.is_empty(), "five minutes under the follow camera reported landings: {reported:?}");
    let (mut ctl, mut cam) = aboard(EARTH_G, 0.0);
    cam.position.y += 10.0;
    let after = until_landed(&mut ctl, &mut cam, FRAME_S);
    assert_eq!(after, None, "the drop after a 10 m teleport reported a landing: {after:?}");
}

/// LETTING GO OF A LADDER HOLDS ON; STEPPING OFF IT FALLS. Climbing the homestead's 3 m ladder
/// and letting go of Space near the top, the player hangs there instead of dropping (which,
/// with falls that hurt, would break an ankle in Realistic); stepping away from the ladder is a
/// fall from where they hung. Standing at its foot is ordinary standing.
///
/// Seen red with the hold removed (the old ladder, which dropped a player who let go): "let go
/// 2.95 m up: two seconds later 0.00 m up, landing Some(7.6078253)".
#[test]
fn letting_go_of_a_ladder_holds_on_and_stepping_off_it_falls() {
    let (mut ctl, mut cam) = aboard(EARTH_G, 0.0);
    let eye = ctl.eye_height();
    ctl.set_climb_zone(Some((0.0, 3.0)));
    ctl.apply_action(GameAction::Jump, true);
    for _ in 0..600 {
        ctl.update_camera(&mut cam, FRAME_S);
        if cam.position.y - eye >= 2.9 {
            break;
        }
    }
    ctl.apply_action(GameAction::Jump, false);
    let held_at = cam.position.y;
    for _ in 0..120 {
        ctl.update_camera(&mut cam, FRAME_S);
    }
    let landed = ctl.take_landing();
    assert!(
        (cam.position.y - held_at).abs() < 1e-4 && landed.is_none(),
        "let go {:.2} m up: two seconds later {:.2} m up, landing {landed:?}",
        held_at - eye,
        cam.position.y - eye
    );
    ctl.set_climb_zone(None);
    let v = until_landed(&mut ctl, &mut cam, FRAME_S).expect("stepping off the ladder is a fall");
    let h = held_at - eye;
    assert!((v - free_fall(h, EARTH_G)).abs() < 1e-3, "stepped off {h:.2} m up and landed at {v:.3} m/s, free fall says {:.3}", free_fall(h, EARTH_G));
    let (mut ctl, mut cam) = aboard(EARTH_G, 0.0);
    ctl.set_climb_zone(Some((0.0, 3.0)));
    for _ in 0..30 {
        ctl.update_camera(&mut cam, FRAME_S);
    }
    assert!((cam.position.y - eye).abs() < 1e-4, "standing at the ladder's foot: {:.3} m up", cam.position.y - eye);
}

/// A LANDING IS ITS REAL SPEED AT ANY FRAME RATE. Aboard, a 4 m drop lands at the free fall's
/// speed at 30, 60 and 240 frames a second and through a half-second stall mid-fall, and a body
/// standing still through a half-second frame lands at nothing. On a planet, a 10 m fall lands
/// within 2% of it at each frame rate (the planet walk's own frame-by-frame speed).
///
/// Seen red with the aboard walk reporting the frame's last velocity instead (`-vertical_velocity`
/// as it lands): "at 30 frames a second a 4 m drop landed at 8.829 m/s, free fall 8.859".
#[test]
fn a_landing_is_its_real_speed_at_any_frame_rate() {
    let want = free_fall(4.0, EARTH_G);
    for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 240.0] {
        let (mut ctl, mut cam) = aboard(EARTH_G, 4.0);
        ctl.set_ground_floor(0.0);
        let v = until_landed(&mut ctl, &mut cam, dt).expect("a landing");
        assert!((v - want).abs() < 1e-3, "at {:.0} frames a second a 4 m drop landed at {v:.3} m/s, free fall {want:.3}", 1.0 / dt);
    }
    let (mut ctl, mut cam) = aboard(EARTH_G, 4.0);
    ctl.set_ground_floor(0.0);
    for _ in 0..10 {
        ctl.update_camera(&mut cam, FRAME_S);
    }
    ctl.update_camera(&mut cam, 0.5);
    let v = until_landed(&mut ctl, &mut cam, FRAME_S).expect("a landing");
    assert!((v - want).abs() < 1e-3, "through a half-second stall a 4 m drop landed at {v:.3} m/s, free fall {want:.3}");
    let (mut ctl, mut cam) = aboard(EARTH_G, 0.0);
    ctl.update_camera(&mut cam, 0.5);
    assert_eq!(ctl.take_landing(), None, "standing still through a half-second frame");
    let want = (2.0 * 9.81_f64 * 10.0).sqrt() as f32;
    for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 240.0] {
        let v = fall_on_planet(10.0, 9.81, dt, false, 0).expect("a landing");
        assert!((v - want).abs() < 0.02 * want, "on a planet at {:.0} frames a second a 10 m fall landed at {v:.3} m/s, free fall {want:.3}", 1.0 / dt);
    }
}

/// A 4 M DROP ON EARTH BREAKS AN ANKLE IN REALISTIC. Walking off a 4 m deck lands at the free
/// fall's 8.86 m/s, past falls.ron's 10 ft line: a Broken Ankle, the health falls.ron gives for
/// that speed, the player told it happened, how long it takes and what helps, walking slowed
/// while it heals, and the HUD naming it as a fall's.
///
/// Seen red with the body pass's `Injury::Fall` arm doing nothing, as before 2026-10-20: "a 4 m
/// drop on Earth (8.86 m/s) left []".
#[test]
fn a_four_metre_drop_on_earth_breaks_an_ankle_in_realistic() {
    let f = shipped_falls();
    let v = step_off(4.0, EARTH_G).expect("a 4 m drop is a landing");
    let l = land(Mode::Realistic, Some(v));
    assert!(l.fx.has("broken_ankle"), "a 4 m drop on Earth ({v:.2} m/s) left {:?}", ids(&l.fx));
    assert_eq!(l.fx.active.len(), 1, "one wound a fall: {:?}", ids(&l.fx));
    let want = f.harm_at(v);
    assert!(want > 0.0 && (l.took - want).abs() < 1e-3, "it took {:.3} health; falls.ron says {want:.3}", l.took);
    let w = shipped();
    let row = w.get("broken_ankle").expect("broken_ankle in wounds.ron");
    assert!(
        l.said.starts_with(&f.landed) && l.said.contains(&row.onset) && l.said.contains("42 days") && l.said.contains(&row.helps),
        "told: {:?}",
        l.said
    );
    let reg = registry();
    let slow = reg.net_stat_multiplier(ids(&l.fx).iter().map(|s| s.as_str()), "speed");
    assert!(slow < 1.0, "walking with a broken ankle is not slowed: x{slow}");
    let hud = w.lines(&l.fx, Some(&reg));
    assert!(
        hud.iter().any(|c| c.name == "Broken Ankle" && c.short.contains("from a fall") && c.severity == 2),
        "the HUD: {hud:?}"
    );
    assert!((v - free_fall(4.0, EARTH_G)).abs() < 1e-3, "landed at {v:.4} m/s; free fall from 4 m is {:.4}", free_fall(4.0, EARTH_G));
}

/// THE SAME LANDING SPEED HURTS THE SAME, WHATEVER THE GRAVITY. A 4 m drop under the Moon's
/// gravity lands at 3.6 m/s and does nothing; it takes a drop of about 24 m there to land at
/// Earth's 4 m speed, and that does exactly what Earth's 4 m drop does. The speed, not the
/// height, is the rule.
///
/// Seen red with `CameraController::set_interior_gravity` ignoring its argument (the walk stuck
/// at 9.81; the same run reported landings at the frame's last velocity, hence 8.83): "on the
/// Moon a 4 m drop landed at 8.83 m/s, not 3.60". And with the body pass's `Injury::Fall` arm
/// doing nothing, as before 2026-10-20: "Earth's 4 m drop left []".
#[test]
fn the_same_landing_speed_hurts_the_same_whatever_the_gravity() {
    let earth = step_off(4.0, EARTH_G).expect("Earth: a landing");
    let moon_4m = step_off(4.0, MOON_G).expect("the Moon: a landing");
    assert!(
        (moon_4m - free_fall(4.0, MOON_G)).abs() < 1e-3,
        "on the Moon a 4 m drop landed at {moon_4m:.2} m/s, not {:.2}",
        free_fall(4.0, MOON_G)
    );
    let same_h = earth * earth / (2.0 * MOON_G);
    let moon = step_off(same_h, MOON_G).expect("the Moon: a landing");
    assert!((moon - earth).abs() < 0.01, "a {same_h:.1} m drop on the Moon landed at {moon:.3} m/s, Earth's 4 m one at {earth:.3}");
    let (e, m4, m) = (land(Mode::Realistic, Some(earth)), land(Mode::Realistic, Some(moon_4m)), land(Mode::Realistic, Some(moon)));
    assert!(e.fx.has("broken_ankle"), "Earth's 4 m drop left {:?}", ids(&e.fx));
    assert!(m4.fx.active.is_empty() && m4.took == 0.0, "the Moon's 4 m drop ({moon_4m:.2} m/s) left {:?} and took {:.1}", ids(&m4.fx), m4.took);
    assert_eq!(ids(&m.fx), ids(&e.fx), "the same speed on the Moon, the same wound");
    assert!((m.took - e.took).abs() < 0.05, "the same speed, the same harm: {:.2} on the Moon, {:.2} on Earth", m.took, e.took);
    assert!(same_h > 20.0, "the Moon needs the bigger drop: {same_h:.1} m");
}

/// FORGIVING IS SOFTER THAN REALISTIC FOR THE SAME FALL. For drops of 4, 6.1, 9.2 and 12 m
/// Forgiving takes less health and leaves a milder wound (or none), and where the wound is the
/// same it heals sooner; the 4 m drop that breaks an ankle in Realistic does nothing at all.
///
/// Seen red with `Falls::read_speed` ignoring the mode (the same run reported landings at the
/// frame's last velocity, hence 18.6): "Forgiving, 4 m: took 18.6 health, Realistic 18.6". And
/// with the `Injury::Fall` arm doing nothing, as before 2026-10-20: "Realistic, 4 m: [], took 0.0".
#[test]
fn forgiving_is_softer_than_realistic_for_the_same_fall() {
    let f = shipped_falls();
    let rank = |fx: &StatusEffects| f.steps.iter().rposition(|s| fx.has(&s.effect));
    for drop in [4.0, 6.1, 9.2, 12.0] {
        let v = step_off(drop, EARTH_G).expect("a landing");
        let (r, g) = (land(Mode::Realistic, Some(v)), land(Mode::Forgiving, Some(v)));
        assert!(r.took > 0.0 && rank(&r.fx).is_some(), "Realistic, {drop} m: {:?}, took {:.1}", ids(&r.fx), r.took);
        assert!(g.took < r.took, "Forgiving, {drop} m: took {:.1} health, Realistic {:.1}", g.took, r.took);
        let (rr, gr) = (rank(&r.fx), rank(&g.fx));
        assert!(gr <= rr, "Forgiving, {drop} m: {:?}, milder than Realistic's {:?}", ids(&g.fx), ids(&r.fx));
        if let (Some(a), Some(b)) = (gr, rr) {
            if a == b {
                let id = &f.steps[a].effect;
                assert!(g.fx.remaining(id) < r.fx.remaining(id), "Forgiving, {drop} m: {id} lasts {:?}, Realistic {:?}", g.fx.remaining(id), r.fx.remaining(id));
            }
        }
    }
    let v = step_off(4.0, EARTH_G).expect("a landing");
    let g = land(Mode::Forgiving, Some(v));
    assert!(g.fx.active.is_empty() && g.took == 0.0, "Forgiving, 4 m: {:?}, took {:.1}", ids(&g.fx), g.took);
}

/// A HIGH FALL KILLS IN REALISTIC, AND SAYS SO. Walking off a 20 m edge (a little over six
/// storeys of the triage guideline's 10 ft) lands at 19.8 m/s: in Realistic it kills a body at
/// full health and the death screen says "a fall"; Forgiving, reading it at its share of the
/// speed, leaves the body badly hurt but alive.
///
/// Seen red with the body pass's `Injury::Fall` arm doing nothing, as before 2026-10-20: "a 20
/// m fall (19.81 m/s): health 100.0 left, died None".
#[test]
fn a_high_fall_kills_in_realistic_and_says_so() {
    let v = step_off(20.0, EARTH_G).expect("a 20 m drop is a landing");
    let r = land(Mode::Realistic, Some(v));
    assert_eq!(r.died.as_deref(), Some(shipped_falls().cause.as_str()), "a 20 m fall ({v:.2} m/s): health {:.1} left, died {:?}", 100.0 - r.took, r.died);
    let g = land(Mode::Forgiving, Some(v));
    assert!(g.died.is_none() && g.took > 50.0, "Forgiving: the same fall took {:.1} and died {:?}", g.took, g.died);
}

/// A walker on a planet with gravity `g`, `drop_m` above its standing height, falling through
/// the planet walk (`radial_step`, walking, nothing held) at `dt` until it stands, each frame
/// handed to the controller as lib.rs does (`surface_step`). The first `flying_for` frames are
/// dev flight (the F9 hover, held still). The landing lib.rs would take.
pub(crate) fn fall_on_planet(drop_m: f64, g: f64, dt: f64, on_water: bool, flying_for: u32) -> Option<f32> {
    let mut ctl = CameraController::new(WALK_SPEED_MPS, 1.0);
    let rest = 6_371_000.0;
    let (mut r, mut v) = (rest + drop_m, 0.0);
    for i in 0..(120.0 / dt) as u32 {
        let flying = i < flying_for;
        let mode = if flying { MoveMode::DevFlight } else { MoveMode::Walk };
        let s = radial_step(mode, true, false, r, v, rest, g, SURFACE_SETTLE, dt, 0.0, 5.0 * dt);
        ctl.surface_step(r, s.r, s.landing(), flying, on_water);
        (r, v) = (s.r, s.v_r);
        if s.grounded && s.v_r == 0.0 && !flying {
            break;
        }
    }
    ctl.take_landing()
}

/// A FALL INTO WATER HURTS NOBODY. A 10 m fall on a planet onto the sea (the drawn water is the
/// walker's floor there) reports no landing; the same fall onto land breaks a leg, so it is the
/// water that spared it.
///
/// Seen red with `FallWatch::landed` ignoring `on_water`: "a 10 m fall onto the sea reported a
/// landing: Some(13.926755)".
#[test]
fn a_fall_into_water_hurts_nobody() {
    let sea = fall_on_planet(10.0, 9.81, f64::from(FRAME_S), true, 0);
    assert_eq!(sea, None, "a 10 m fall onto the sea reported a landing: {sea:?}");
    let land_v = fall_on_planet(10.0, 9.81, f64::from(FRAME_S), false, 0).expect("onto land: a landing");
    let l = land(Mode::Realistic, Some(land_v));
    assert!(l.fx.has("broken_leg") && l.took > 0.0, "the same fall onto land ({land_v:.2} m/s) left {:?} and took {:.1}", ids(&l.fx), l.took);
}

/// THE SHIPPED FALL DATA IS WHOLE. The steps and the harm rise with the speed; every step is a
/// wound row in wounds.ron and a `wound` status effect that slows walking and says it came from
/// a fall; nothing the game does on purpose reaches the first harm (its own jump, a drop jump
/// from 60 cm); the speeds are the cited heights in standard gravity (2 m, 10 ft and 20 ft for
/// the steps, a whole body's health at five storeys of 10 ft); Forgiving reads a share of the
/// speed.
///
/// Seen red with the sprain's step at 4.9 m/s in falls.ron: "the first harm is at 4.9 m/s".
#[test]
fn the_shipped_fall_data_is_whole() {
    let f = shipped_falls();
    let w = shipped();
    let reg = registry();
    assert!(!f.steps.is_empty() && !f.harm.is_empty() && !f.cause.is_empty() && !f.landed.is_empty());
    let first = f.first_harm_m_s().expect("something hurts");
    assert!(first > JUMP_SPEED_MPS && first > free_fall(0.6, 9.80665), "the first harm is at {first} m/s");
    assert!(f.steps.windows(2).all(|s| s[0].from_m_s < s[1].from_m_s), "the steps get worse in order");
    assert!(f.harm.windows(2).all(|p| p[0].m_s < p[1].m_s && p[0].health < p[1].health), "the harm rises with the speed");
    for s in &f.steps {
        let row = w.get(&s.effect).unwrap_or_else(|| panic!("{} has no row in wounds.ron", s.effect));
        let def = reg.get(&s.effect).unwrap_or_else(|| panic!("{} is not a status effect", s.effect));
        assert_eq!(def.kind, humanity_systems::wounds::WOUND, "{} is a wound", s.effect);
        assert!(
            def.modifier().is_some_and(|(stat, v, op)| stat == "speed" && op == "multiply" && v > 0.0 && v < 1.0),
            "{} slows walking: {:?}",
            s.effect,
            def.modifier()
        );
        assert!(row.hud.contains("fall") && row.goes_bad.is_none() && row.health_per_h == 0.0, "{}: {row:?}", s.effect);
    }
    const FT: f32 = 0.3048;
    const G0: f32 = 9.80665;
    for (s, h) in f.steps.iter().zip([2.0, 10.0 * FT, 20.0 * FT]) {
        assert!((s.from_m_s - free_fall(h, G0)).abs() < 0.01, "{} from {} m/s; {h:.3} m is {:.3}", s.effect, s.from_m_s, free_fall(h, G0));
    }
    let kills = f.speed_taking(100.0).expect("a fall can kill");
    assert!((kills - free_fall(50.0 * FT, G0)).abs() < 0.01, "a whole body's health at {kills:.3} m/s; five storeys is {:.3}", free_fall(50.0 * FT, G0));
    assert!(f.forgiving.speed_share > 0.0 && f.forgiving.speed_share < 1.0);
}
