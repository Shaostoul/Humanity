//! INTERACTIONS ON THE SHIPPED BODIES (character animation rung 2b, 2026-11-10;
//! docs/design/character-animation.md section 17). Each test poses a real MakeHuman body through its
//! real (retargeted) clips at a real anchor, the way the frame does (the seat's drop measured on the
//! drawn mesh, `humanity_animation::interact::pose`, the feet with the pelvis held by the seat, the
//! hands by `contact_limbs` and `apply_limbs`), and then measures the DRAWN mesh with the interaction
//! probe's own helpers: the seat vertices skinned with the palette rows the GPU would get, the hands'
//! vertices against the rungs or the surface. Each also runs its red twin (warping off, the hands off)
//! and holds that it FAILS the same bound where it must, so a bound that cannot fail is caught here.

use glam::{Mat4, Quat, Vec3};
use humanity_animation::interact::{contact_time, default_back_drop, release_time, ContactPart};
use humanity_animation::{Engagement, FeetDef, FootFrame, FootMode, FootPlacer, LoadedBody, Phase, Pose};

use super::*;
use crate::engine::anchor_probe::{hips_clearance, rung_gap, seat_drop, skinned, surface_clearance};
use crate::engine::feet_ground::TestGround;

fn repo_data() -> PathBuf {
    Path::new(env!("HUMANITY_REPO_ROOT")).join("data")
}

fn body<'a>(lib: &'a CharacterLibrary, choice: &str) -> &'a LoadedBody {
    &lib.bodies[lib.body_index(choice).unwrap_or_else(|| panic!("no body {choice}"))]
}

/// A person at an anchor's root (its object at the origin, unturned) holding it `t` seconds in,
/// posed as the frame poses them: the palette rows, the model matrices and the world matrix.
fn pose_at(lib: &CharacterLibrary, b: &LoadedBody, anchor: &str, phase: Phase, t: f32, warp: bool) -> (Vec<[f32; 4]>, Vec<Mat4>, Mat4, crate::engine::interact::Placed) {
    let ai = lib.anchors.index(anchor).unwrap_or_else(|| panic!("no anchor {anchor}"));
    let def = &lib.anchors.anchors[ai];
    let skel = &b.skeleton;
    let h = b.hands.as_ref().expect("the body has arms");
    let idle = b.clips.index("idle");
    let placed = crate::engine::interact::Placed { anchor: ai, origin: Vec3::ZERO, yaw: 0.0, object: crate::engine::interact::ObjectRef::Machine(0) };
    let (root, facing) = placed.root(def);
    let world = Mat4::from_rotation_translation(Quat::from_rotation_y(facing), root);
    let (contact_s, release_s) = (contact_time(def, &b.clips, skel, h.hips), release_time(def, &b.clips, skel, h.hips));
    let mut loco = Pose::default();
    b.clips.get(idle.expect("an idle clip")).sample(skel, t, &mut loco);
    let (mut out, mut scratch, mut model, mut rows) = (Pose::default(), Pose::default(), Vec::new(), Vec::new());
    // The seat's drop, measured as the frame measures it.
    let hips_c = def.contact(ContactPart::Hips);
    let drop = match hips_c {
        None => 0.0,
        Some(_) => {
            let probe = Engagement { def, phase: Phase::Hold, t: 0.0, hips_target: Some(Vec3::new(0.0, 1.0, 0.0)), warp: false, contact_s, release_s };
            humanity_animation::interact::pose(&probe, &b.clips, skel, &lib.humanoid, h, idle, &loco, &mut out, &mut scratch);
            pose::model_matrices(skel, &out, &mut model);
            pose::palette_rows(skel, &model, &mut rows);
            let forward = (def.kind == humanity_animation::AnchorKind::Sit).then_some(Vec3::Z);
            seat_drop(&b.model, &b.seat_vertices, &[], &rows, model[h.hips].w_axis.truncate(), forward).unwrap_or(default_back_drop())
        }
    };
    let hips_target = hips_c.map(|c| {
        let local = def.in_person_frame(c.at);
        Vec3::new(local.x, local.y + drop, local.z)
    });
    if std::env::var("IT_DEBUG").is_ok() && t == 0.0 && warp {
        let seated = humanity_animation::interact::seated_hips(def, &b.clips, skel, h.hips);
        eprintln!("{} {anchor}: drop {drop:.3} m, the clip's seated hips {seated:?}, the target {hips_target:?} (contact {contact_s:.2} s, release {release_s:.2} s)", b.choice);
    }
    let e = Engagement { def, phase, t, hips_target, warp, contact_s, release_s };
    let w = humanity_animation::interact::pose(&e, &b.clips, skel, &lib.humanoid, h, idle, &loco, &mut out, &mut scratch);
    pose::model_matrices(skel, &out, &mut model);
    // The feet on a flat floor, the pelvis the seat's.
    if def.kind == humanity_animation::AnchorKind::Sit {
        if let Some(rig) = b.feet.as_ref() {
            let contacts = def.phase(phase).clip.as_deref().and_then(|c| b.clips.index(c)).map_or([1.0, 1.0], |c| b.clips.get(c).contacts_at(t));
            let frame = FootFrame { world, contacts, airborne: false, dt: 1.0 / 60.0, pelvis_fixed: w.engage > 0.0 };
            FootPlacer::default().place(rig, skel, &lib.humanoid, &mut out, &mut model, &frame, &TestGround { floor: 0.0, stair: None, slope: 0.0 }, FootMode::Full, &FeetDef::default());
        }
    }
    rows.clear();
    pose::palette_rows(skel, &model, &mut rows);
    let _ = w;
    (rows, model, world, placed)
}

/// HIPS ON THIS SEAT, AT EVERY SEAT'S HEIGHT, FROM THE DRAWN MESH: both bodies seated on the chair
/// (0.47 m), the couch (0.40 m) and the bed's edge (0.40 m, facing out) rest the lowest drawn vertex
/// of their hips and thighs within 2 cm into to 2.5 cm over the seat, through the hold. With motion
/// warping off (the clip's own seat), at least two seats FAIL that bound, so it cannot pass by the
/// clip's seat happening to be this one. The seat flesh is the buttocks: within 10 cm across of the
/// drawn hips joint and no more than 3 cm forward of it (`anchor_probe::SEAT_ACROSS_M`,
/// `SEAT_FORWARD_M`; the thighs forward of it are the legs', sloped toward the floor by the seated
/// feet solve). Measured (IT_DEBUG=1, 2026-11-10): warped, the female 0.9 cm into the chair and on
/// the couch and the bed's edge, the male on the chair and 0.4 cm over the couch and the edge; as
/// made, the female 5.6 cm into the chair (passing the couch and the edge by accident, 1.4 cm over:
/// the clip's seat is about their height on her), the male 7.1 cm over the couch and the edge
/// (passing the chair by accident, 0.1 cm over): three seats fail as made.
/// Seen red 2026-11-10 (the seat flesh then read 25 cm across) with the warp's offset not applied
/// (`if false && e.warp` in interact::pose): "female on chair_seat: the drawn hips stand -0.082 to
/// -0.082 m over the seat"; and with the seat's drop not measured (`seat_drop` returning None, the
/// default 0.11 m): "male on chair_seat: the drawn hips stand 0.027 to 0.027 m over the seat".
#[test]
fn hips_rest_on_this_seat_at_every_seats_height() {
    let lib = read_library(&repo_data()).unwrap();
    let mut red_fails = 0;
    for choice in ["female", "male"] {
        let b = body(&lib, choice);
        for anchor in ["chair_seat", "couch_seat_middle", "bed_edge"] {
            let def = &lib.anchors.anchors[lib.anchors.index(anchor).unwrap()];
            let c = def.contact(ContactPart::Hips).unwrap();
            let mut worst = (f32::INFINITY, f32::NEG_INFINITY);
            let mut red = (f32::INFINITY, f32::NEG_INFINITY);
            for k in 0..20 {
                let t = k as f32 * 0.08;
                for (warp, acc) in [(true, &mut worst), (false, &mut red)] {
                    let (rows, model, world, placed) = pose_at(&lib, b, anchor, Phase::Hold, t, warp);
                    let pts = skinned(&b.model, &b.seat_vertices, &[], &rows, &world, 1);
                    let forward = Quat::from_rotation_y(placed.root(def).1) * Vec3::Z;
                    let hips = world.transform_point3(model[b.hands.as_ref().unwrap().hips].w_axis.truncate());
                    let v = hips_clearance(&pts, hips, placed.point(c.at), Vec3::Y, Some(forward)).expect("seat vertices over the seat");
                    acc.0 = acc.0.min(v);
                    acc.1 = acc.1.max(v);
                }
            }
            if std::env::var("IT_DEBUG").is_ok() {
                eprintln!("{choice} {anchor}: warped {:.3} to {:.3} m, as made {:.3} to {:.3} m", worst.0, worst.1, red.0, red.1);
            }
            assert!(worst.0 >= -0.02 && worst.1 <= 0.025, "{choice} on {anchor}: the drawn hips stand {:.3} to {:.3} m over the seat (2 cm in to 2.5 cm over)", worst.0, worst.1);
            if red.0 < -0.02 || red.1 > 0.025 {
                red_fails += 1;
            }
        }
    }
    assert!(red_fails >= 2, "with warping off only {red_fails} seats failed: the bound would pass without the warp");
}

/// LYING: both bodies lie in the bed with the lowest drawn vertex of the hips within 2 cm into to
/// 2.5 cm over the covers (0.40 m), and each ankle joint within 15 cm over them (the legs on the bed).
/// Measured (2026-11-10, laid down from the rest pelvis with the legs straight): both hips 0.0 cm on
/// the covers; the ankles 12.2 cm (female) and 11.5 cm (male) over them.
/// Seen red 2026-11-10 with the back's drop not measured (`seat_drop` returning None, the default
/// 0.11 m): "female in bed_lie: the hips stand 0.069 m over the covers"; and laid down from the idle's
/// tipped pelvis (the second rig sweep's raised legs): "female in bed_lie: an ankle stands 0.226 m
/// over the covers (the legs raised)".
#[test]
fn a_body_lies_on_the_covers() {
    let lib = read_library(&repo_data()).unwrap();
    for choice in ["female", "male"] {
        let b = body(&lib, choice);
        let def = &lib.anchors.anchors[lib.anchors.index("bed_lie").unwrap()];
        let c = def.contact(ContactPart::Hips).unwrap();
        let (rows, model, world, placed) = pose_at(&lib, b, "bed_lie", Phase::Hold, 0.5, true);
        let pts = skinned(&b.model, &b.seat_vertices, &[], &rows, &world, 1);
        let hips = world.transform_point3(model[b.hands.as_ref().unwrap().hips].w_axis.truncate());
        let v = hips_clearance(&pts, hips, placed.point(c.at), Vec3::Y, None).expect("hips over the covers");
        if std::env::var("IT_DEBUG").is_ok() {
            eprintln!("{choice} bed_lie: {v:.3} m");
        }
        assert!((-0.02..=0.025).contains(&v), "{choice} in bed_lie: the hips stand {v:.3} m over the covers");
        // The legs lie on the covers too: each ankle joint within 15 cm over them (a heel's height).
        for leg in &b.feet.as_ref().expect("legs").legs {
            let over = world.transform_point3(model[leg.foot].w_axis.truncate()).y - placed.point(c.at).y;
            if std::env::var("IT_DEBUG").is_ok() {
                eprintln!("{choice} bed_lie: an ankle {over:.3} m over the covers");
            }
            assert!(over < 0.15, "{choice} in bed_lie: an ankle stands {over:.3} m over the covers (the legs raised)");
        }
    }
}

/// HANDS ON THE RUNGS: on the shipped ladder (3.0 m, 10 rungs) a body climbing from 0.3 m to 2.05 m,
/// past its top (where the hands let go for the hatch), has each hand that grips a rung within 2 cm of
/// its surface and no more than 3 cm into it, from the drawn hand vertices; with the hands off their
/// contacts (the red twin) they are far off.
/// Measured: gripping hands 1.7 cm into to 0.0 cm off their rungs (female), 2.3 cm into to 0.0 cm
/// off (male); hands off, at least 19.7 and 17.6 cm from any rung.
/// Seen red 2026-11-10 with the palm's point 6 cm further off the rung (`RUNG_PALM_GAP_M - 0.06` in
/// climb_limbs): "female: a gripping hand is 0.043 m off its rung (at most 0.02)". (Leaving the palm
/// offset out of the wrist target did NOT fail it: that shifts the hand up the rung, not off it.) And
/// run to 2.05 m with the hands held on the top rung past the top (no let-go in climb_limbs, the
/// first rig sweep's 10 to 14 cm): "female: a gripping hand is 0.105 m off its rung (at most 0.02)".
#[test]
fn the_hands_grip_the_rungs_while_climbing() {
    let lib = read_library(&repo_data()).unwrap();
    let ai = lib.anchors.index("ladder_climb").unwrap();
    let def = &lib.anchors.anchors[ai];
    for choice in ["female", "male"] {
        let b = body(&lib, choice);
        let skel = &b.skeleton;
        let h = b.hands.as_ref().unwrap();
        let placed = crate::engine::interact::Placed { anchor: ai, origin: Vec3::ZERO, yaw: 0.0, object: crate::engine::interact::ObjectRef::Ladder { height: 3.0, rungs: 10, width: 0.6 } };
        let (root, facing) = placed.root(def);
        let mut worst = 0.0_f32;
        let mut deepest = 0.0_f32;
        let mut red_best = f32::INFINITY;
        let mut gripped = 0;
        // Up the ladder and past its top rung (out at the hatch the hands let go).
        for k in 0..36 {
            let height = 0.3 + k as f32 * 0.05;
            for ik in [true, false] {
                let world = Mat4::from_rotation_translation(Quat::from_rotation_y(facing), root + Vec3::Y * height);
                let mut loco = Pose::default();
                b.clips.get(b.clips.index("idle").unwrap()).sample(skel, 0.0, &mut loco);
                let mut model = Vec::new();
                pose::model_matrices(skel, &loco, &mut model);
                let p = super::tests::person("other");
                let p = Person { feet: root + Vec3::Y * height, facing, ..p };
                let mut on = [false; 2];
                let limbs = contact_limbs(&lib, def, Some(&placed), &world.inverse(), &p, 1.0, b.feet.as_ref(), Some(h), humanity_animation::interact::climb_reach(h, &model), &mut on).expect("climb limbs");
                if ik {
                    humanity_animation::interact::apply_limbs(h, b.feet.as_ref(), skel, &lib.humanoid, &lib.grips, &mut loco, &mut model, &limbs);
                }
                let mut rows = Vec::new();
                pose::palette_rows(skel, &model, &mut rows);
                for side in 0..2 {
                    if !on[side] {
                        continue;
                    }
                    let pts = skinned(&b.model, &b.hand_vertices[side], &[], &rows, &world, 1);
                    let gap = (0..10).filter_map(|r| {
                        let y = (r as f32 + 0.5) * 0.3;
                        rung_gap(&pts, Vec3::new(-0.26, y, 0.0), Vec3::new(0.26, y, 0.0), RUNG_HALF_M)
                    }).reduce(f32::min).unwrap();
                    if ik {
                        if std::env::var("IT_DEBUG").is_ok() && gap > 0.012 {
                            let arm = &h.arms[side];
                            let t = limbs.hands[side].unwrap();
                            let (_, hr, hp) = model[arm.hand].to_scale_rotation_translation();
                            let palm_at = hp + (hr * skel.rest_world_rotation(arm.hand).inverse()) * arm.palm_offset;
                            let shoulder = model[arm.upper].w_axis.truncate();
                            eprintln!("{choice} height {height:.2} hand {side}: {gap:.3} m off; the palm's middle {:.3} m from its target (model {palm_at:?} for {:?}), the shoulder {:.3} m from it", (palm_at - t.at).length(), t.at, (shoulder - t.at).length());
                        }
                        worst = worst.max(gap);
                        deepest = deepest.min(gap);
                        gripped += 1;
                    } else {
                        red_best = red_best.min(gap);
                    }
                }
            }
        }
        if std::env::var("IT_DEBUG").is_ok() {
            eprintln!("{choice} ladder: gripping hands {deepest:.3} to {worst:.3} m off a rung over {gripped} grips; hands off, at least {red_best:.3} m");
        }
        assert!(gripped > 20, "{choice}: only {gripped} hand-frames gripped a rung");
        assert!(worst <= 0.02, "{choice}: a gripping hand is {worst:.3} m off its rung (at most 0.02)");
        assert!(deepest >= -0.03, "{choice}: a gripping hand reaches {:.3} m into its rung (at most 0.03)", -deepest);
        assert!(red_best > 0.05, "{choice}: with the hands off their contacts a hand was still {red_best:.3} m from a rung: the bound would pass without the IK");
    }
}

/// HANDS ON THE WORK SURFACE: at the workbench (its top 0.85 m), each hand's lowest drawn vertex is
/// within 1.5 cm under to 2.5 cm over the top; with the hands off (the red twin), well under it.
/// Measured: each hand 0.3 cm (female) and 0.4 cm (male) into the top; hands off, 5.8 to 10.4 cm
/// under it (hanging at the sides).
/// Seen red 2026-11-10 with the palm turned up (`palm: +up` in contact_limbs): "female: hand 0's
/// lowest vertex is 0.033 m over the top".
#[test]
fn the_hands_rest_on_the_work_surface() {
    let lib = read_library(&repo_data()).unwrap();
    let ai = lib.anchors.index("workbench_front").unwrap();
    let def = &lib.anchors.anchors[ai];
    for choice in ["female", "male"] {
        let b = body(&lib, choice);
        let skel = &b.skeleton;
        let h = b.hands.as_ref().unwrap();
        let placed = crate::engine::interact::Placed { anchor: ai, origin: Vec3::ZERO, yaw: 0.0, object: crate::engine::interact::ObjectRef::Machine(0) };
        let (root, facing) = placed.root(def);
        let world = Mat4::from_rotation_translation(Quat::from_rotation_y(facing), root);
        for ik in [true, false] {
            let mut loco = Pose::default();
            b.clips.get(b.clips.index("idle").unwrap()).sample(skel, 0.0, &mut loco);
            let mut model = Vec::new();
            pose::model_matrices(skel, &loco, &mut model);
            let p = Person { feet: root, facing, ..super::tests::person("other") };
            let mut on = [false; 2];
            let limbs = contact_limbs(&lib, def, Some(&placed), &world.inverse(), &p, 1.0, b.feet.as_ref(), Some(h), humanity_animation::interact::climb_reach(h, &model), &mut on).expect("station limbs");
            if ik {
                humanity_animation::interact::apply_limbs(h, b.feet.as_ref(), skel, &lib.humanoid, &lib.grips, &mut loco, &mut model, &limbs);
            }
            let mut rows = Vec::new();
            pose::palette_rows(skel, &model, &mut rows);
            for side in 0..2 {
                let pts = skinned(&b.model, &b.hand_vertices[side], &[], &rows, &world, 1);
                let v = surface_clearance(&pts, 0.85).unwrap();
                if std::env::var("IT_DEBUG").is_ok() {
                    eprintln!("{choice} workbench hand {side} (ik {ik}): {v:.3} m");
                }
                if ik {
                    assert!((-0.015..=0.025).contains(&v), "{choice}: hand {side}'s lowest vertex is {v:.3} m over the top");
                } else {
                    assert!(!(-0.015..=0.025).contains(&v), "{choice}: with the hands off, hand {side} still {v:.3} m over the top: the bound would pass without the IK");
                }
            }
        }
    }
}

/// THE FIRST-PERSON HEAD IS FOLDED AWAY: with the neck's subtree collapsed, no drawn vertex of the
/// body is within 10 cm of the eyes (the camera's near plane), while the body under them (the
/// shoulders, the legs) is drawn; with it drawn (the red twin), the head's vertices are within a few
/// centimetres.
/// Measured: folded, the nearest vertex 15.9 cm (female) and 16.6 cm (male) from the eyes; drawn,
/// 1.3 and 1.5 cm.
/// Seen red 2026-11-10 with the fold rooted at the head instead of the neck (`first_person_fold`):
/// "female: a vertex 0.075 m from the eye with the head folded away" (the neck's own skin).
#[test]
fn the_first_person_head_is_folded_away() {
    let lib = read_library(&repo_data()).unwrap();
    for choice in ["female", "male"] {
        let b = body(&lib, choice);
        let skel = &b.skeleton;
        let h = b.hands.as_ref().unwrap();
        let mut loco = Pose::default();
        b.clips.get(b.clips.index("idle").unwrap()).sample(skel, 0.0, &mut loco);
        let mut model = Vec::new();
        pose::model_matrices(skel, &loco, &mut model);
        let eye = match h.eyes {
            [Some(l), Some(r)] => (model[l].w_axis.truncate() + model[r].w_axis.truncate()) * 0.5,
            _ => panic!("the body has eyes"),
        };
        for hide in [true, false] {
            let mut m = model.clone();
            if hide {
                humanity_animation::interact::collapse_subtree(skel, &mut m, first_person_fold(h).expect("a neck"));
            }
            let mut rows = Vec::new();
            pose::palette_rows(skel, &m, &mut rows);
            let pts = crate::engine::anchor_probe::skinned_drawn(&b.model, &[], &rows, &Mat4::IDENTITY, 1);
            let near = crate::engine::anchor_probe::nearest(&pts, eye).unwrap();
            let below = pts.iter().map(|q| eye.y - q.y).fold(f32::MIN, f32::max);
            if std::env::var("IT_DEBUG").is_ok() {
                eprintln!("{choice} first person (hidden {hide}): nearest {near:.3} m, body below {below:.3} m");
            }
            assert!(below > 1.0, "{choice}: no body drawn under the eye");
            if hide {
                assert!(near >= 0.10, "{choice}: a vertex {near:.3} m from the eye with the head folded away");
            } else {
                assert!(near < 0.05, "{choice}: drawn, the head's nearest vertex is still {near:.3} m from the eye");
            }
        }
    }
}

/// EVERY ANCHOR AN OBJECT NAMES IS A ROW: the shipped machine definitions, structure types and
/// opening styles name only anchors interactions.ron has, so a typo never leaves a chair unusable.
/// Seen red 2026-11-10 with "chair_seat" misspelt in home.ron: "data/machines/home.ron's chair names
/// anchor chair_sat, which interactions.ron does not have".
#[test]
fn every_anchor_an_object_names_is_a_row() {
    let lib = read_library(&repo_data()).unwrap();
    for file in ["data/machines/home.ron", "data/machines/home_solo.ron", "data/machines/ship.ron"] {
        let text = std::fs::read_to_string(Path::new(env!("HUMANITY_REPO_ROOT")).join(file)).unwrap();
        let home: humanity_ship::machines::MachineHome = ron::from_str(&text).unwrap_or_else(|e| panic!("{file}: {e}"));
        for (id, def) in &home.catalog {
            for a in &def.anchors {
                assert!(lib.anchors.index(a).is_some(), "{file}'s {id} names anchor {a}, which interactions.ron does not have");
            }
        }
    }
    for ty in humanity_ship::structure::structure_types() {
        for a in &ty.anchors {
            assert!(lib.anchors.index(a).is_some(), "structure type {} names anchor {a}", ty.id);
        }
    }
    for s in humanity_systems::door_anim::opening_styles() {
        for a in &s.anchors {
            assert!(lib.anchors.index(a).is_some(), "opening style {} names anchor {a}", s.id);
        }
    }
    // And the network's compiled-in ids are the file's.
    let ids = humanity_core::stance::AnchorIds::shipped();
    for a in &lib.anchors.anchors {
        assert!(ids.ids.contains(&a.id), "the network's anchor ids lack {}", a.id);
    }
}
