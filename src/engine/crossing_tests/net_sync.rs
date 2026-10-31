//! From net/sync.rs's tests: a crew figure walks, turns and stops the way the engine poses it
//! (`net_route::crew_figure_parts`).

use crate::ecs::systems::System;
use crate::net::sync::*;
use crate::ecs::components::Transform;
use crate::net::protocol::NetMessage;
use crate::net::sync::NPC_LOCAL_STANDING_Y;
use crate::net::sync::tests::CREW_MPS;
use crate::net::sync::tests::CREW_UPDATE_S;
use crate::net::sync::tests::degrees;
use glam::Vec3;

/// A crew member walking as the relay walks one, drawn at 60 frames a second: an update
/// where it stands and a second standing still, then an update every half second along each
/// leg, `(way, seconds)` (a zero way stands still, the way the relay's arrival, chore-done and
/// next-chore updates repeat where it stands). Returns every frame's time since the legs
/// began and the way the DRAWN figure's face looks across the floor (+z of the figure
/// `engine::net_route::crew_figure_parts` builds, which the game draws).
fn crew_walk(legs: &[(Vec3, f32)]) -> Vec<(f32, Vec3)> {
    let data = crate::hot_reload::data_store::DataStore::new();
    let mut sys = NetSyncSystem::new();
    let mut world = hecs::World::new();
    let update = |p: Vec3| NetMessage::NpcUpdate { entity_id: 9, name: "Cook Ana".to_string(), position: [p.x, p.y, p.z], activity: "Walking".to_string(), working: false };
    let dt = 1.0 / 60.0;
    let mut at = Vec3::new(80.0, NPC_LOCAL_STANDING_Y, 30.0);
    sys.queue_messages(vec![update(at)]);
    for _ in 0..60 {
        sys.tick(&mut world, dt, &data);
    }
    let mut out = Vec::new();
    let mut t = 0.0;
    for &(way, secs) in legs {
        for _ in 0..(secs / CREW_UPDATE_S).round() as usize {
            at += way * CREW_MPS * CREW_UPDATE_S;
            sys.queue_messages(vec![update(at)]);
            for _ in 0..(CREW_UPDATE_S / dt).round() as usize {
                sys.tick(&mut world, dt, &data);
                t += dt;
                let (pos, rot) = world.query_mut::<(&Transform, &RemoteNpc)>().into_iter().map(|(_, (tr, _))| (tr.position, tr.rotation)).next().expect("the crew member is drawn");
                let face = crate::engine::net_route::crew_figure_parts(pos, rot)[0].rotation * Vec3::Z;
                out.push((t, Vec3::new(face.x, 0.0, face.z).normalize_or_zero()));
            }
        }
    }
    out
}

/// A CREW FIGURE FACES THE WAY IT WALKS (BUG-166): walking east (+x) it is drawn facing east,
/// walking north (-z) facing north, from a second into the walk (a quarter turn takes about
/// 0.45 s, a half turn 0.7 s) to its end.
///
/// Green on main (0f8b30944) as well: the game already turned crew along their path on each
/// relay update (the NpcUpdate arm, since 2026-07-01), so the faults BUG-166 fixed are the
/// two tests after this one. Kept as the guard of the facing the new turn keeps.
#[test]
fn a_crew_figure_faces_the_way_it_walks() {
    for (name, way) in [("east (+x)", Vec3::X), ("north (-z)", Vec3::NEG_Z)] {
        for (t, face) in crew_walk(&[(way, 4.0)]) {
            if t >= 1.0 {
                assert!(degrees(face, way) < 3.0, "{t:.2} s into a walk {name} the figure faced {:.1} degrees off its way", degrees(face, way));
            }
        }
    }
}

/// A CREW FIGURE TURNS ROUND SMOOTHLY, NOT IN A SNAP (BUG-166): walking east, then back west,
/// it turns no faster than a person turning on the spot (`turning::BODY`, 360 degrees a
/// second) on any frame, and faces west a second after it turned back.
///
/// Seen red 2026-10-05 on main (0f8b30944): "2.25 s in the figure turned at 539 degrees a
/// second; a person turns on the spot at 360". Every turn took the half second between two
/// relay updates whatever its size, so a turn back was three times as fast at its middle.
#[test]
fn a_crew_figure_turns_round_smoothly() {
    let frames = crew_walk(&[(Vec3::X, 2.0), (Vec3::NEG_X, 2.0)]);
    let dt = 1.0 / 60.0;
    let limit = crate::turning::BODY.rate.to_degrees();
    let (fastest, when) = frames.windows(2).map(|w| (degrees(w[0].1, w[1].1) / dt, w[1].0)).fold((0.0f32, 0.0f32), |a, b| if b.0 > a.0 { b } else { a });
    assert!(fastest <= limit * 1.001, "{when:.2} s in the figure turned at {fastest:.0} degrees a second; a person turns on the spot at {limit:.0}");
    for (t, face) in &frames {
        if *t >= 3.0 {
            assert!(degrees(*face, Vec3::NEG_X) < 3.0, "{t:.2} s in, a second after turning back, the figure faced {:.1} degrees off west", degrees(*face, Vec3::NEG_X));
        }
    }
}

/// A CREW MEMBER THAT STOPS KEEPS ITS LAST FACING (BUG-166). It steps half a metre east to
/// a chore beside the last one (one relay update moves it), then stands: the relay's
/// arrival, chore-done and next-chore updates all repeat where it stands. On every frame
/// from the time its turn has had to the end it faces east.
///
/// Seen red 2026-10-05 on main (0f8b30944): "0.52 s in, standing still, the figure faced
/// 89.7 degrees away from east, the way it last walked". An update that did not move the
/// figure kept the old turn and started it again, so the figure snapped back to the facing
/// it had before its last turn and turned again.
#[test]
fn a_crew_member_that_stops_keeps_its_last_facing() {
    for (t, face) in crew_walk(&[(Vec3::X, 0.5), (Vec3::ZERO, 3.0)]) {
        if t >= 0.48 {
            let off = degrees(face, Vec3::X);
            assert!(off < 3.0, "{t:.2} s in, standing still, the figure faced {off:.1} degrees away from east, the way it last walked");
        }
    }
}
