//! The engine's character tests: the SHIPPED data, the MakeHuman bodies and the committed test
//! figure, read through the real glTF skin reader. Each was seen red first; the red run is named at
//! each test.

use glam::{Mat4, Vec3};
use humanity_animation::{pose, LoadedBody, Pose, SourceClip};

use super::*;

fn repo_data() -> PathBuf {
    Path::new(env!("HUMANITY_REPO_ROOT")).join("data")
}

fn body<'a>(lib: &'a CharacterLibrary, choice: &str) -> &'a LoadedBody {
    &lib.bodies[lib.body_index(choice).unwrap_or_else(|| panic!("no body {choice}"))]
}

/// The shipped character data reads whole: a female and a male MakeHuman body (56 joints, 15 shape
/// targets each, every target saying how it moves the joints) and the test figure as the fallback,
/// every clip retargeted onto each, the graph resolved against them, every body material given a
/// region, nothing standing in for a body that failed. A look's body picks its body; a crew is
/// women and men.
/// Seen red 2026-11-08 with clips.ron's walk naming animation "Walk" (the file's is "walk"): "clip
/// walk: ... has no animation named Walk"; and 2026-11-09 with the female body's region list
/// missing "brows": "body makehuman_female: material brows has no region".
#[test]
fn the_shipped_character_data_reads_whole() {
    let lib = read_library(&repo_data()).expect("the shipped character data reads");
    assert!(lib.warnings.is_empty(), "{:?}", lib.warnings);
    let ids: Vec<&str> = lib.bodies.iter().map(|b| b.choice.as_str()).collect();
    assert_eq!(ids, vec!["female", "male", "hos_test_figure"]);
    for b in &lib.bodies {
        assert!(!b.stand_in, "{} is standing in", b.choice);
        assert_eq!(b.clips.clips.len(), 14, "{}", b.choice);
        assert_eq!(b.primitive_regions.len(), b.model.primitives.len());
        for id in ["idle", "walk", "jog", "sprint", "turn_left", "turn_right", "crouch_walk", "sit_idle", "jump_air"] {
            assert!(b.clips.index(id).is_some(), "{}: {id}", b.choice);
        }
    }
    for choice in ["female", "male"] {
        let b = body(&lib, choice);
        assert_eq!(b.skeleton.joint_count(), 56, "{choice}");
        assert_eq!(b.model.morph_names.len(), 15, "{choice}");
        assert_eq!(b.model.joint_deltas.len(), 15, "{choice}");
        assert!(b.target("height_max").is_some() && b.target("african").is_some());
    }
    assert_eq!(body(&lib, "hos_test_figure").skeleton.joint_count(), 24);
    assert!((body(&lib, "female").height_m - 1.59).abs() < 0.01, "{}", body(&lib, "female").height_m);
    assert!((body(&lib, "male").height_m - 1.73).abs() < 0.01, "{}", body(&lib, "male").height_m);
    assert!(lib.graph.state_index("move").is_some());
    // Bodies by look and by wearer.
    assert_eq!(lib.bodies[lib.body_for(Some("male"), "player", 0.0)].choice, "male");
    assert_eq!(lib.bodies[lib.body_for(Some("no such body"), "player", 0.0)].choice, "female");
    let crew: std::collections::BTreeSet<&str> = (0..20).map(|i| lib.bodies[lib.body_for(None, "crew", stagger(FigureKey::Crowd(i)))].choice.as_str()).collect();
    assert_eq!(crew.into_iter().collect::<Vec<_>>(), vec!["female", "male"], "a crew is women and men");
}

/// THE RETARGET TEST ON THE SHIPPED FILES: the test clip library's walk (a T-pose rig with Mixamo
/// names, rolled bone frames and a 0.01 armature) retargeted onto the A-pose test figure lands on
/// `ref_walk`, the same motion authored directly on the figure by scripts/gen-test-figure.js.
/// Retargeted at a rate whose frames fall on the clips' own keys, every limb bone points within half
/// a degree of `ref_walk` at every key and the hips ride within a centimetre; baked at characters.ron's
/// 30 frames a second, as the game bakes, within three degrees, which is resampling, not retargeting.
/// Seen red 2026-11-08 with the rest alignment left out of retarget (the first bone checked,
/// thigh.L, already 1.40 degrees off at the first key, against 0.04 with it); and, before the exact
/// half, the baked clip alone read 1.72 degrees off on the right shin at a 1.5 degree limit, which
/// is why the exact comparison exists.
#[test]
fn the_librarys_walk_on_the_figure_lands_on_the_walk_made_on_the_figure() {
    let lib = read_library(&repo_data()).unwrap();
    let fig = body(&lib, "hos_test_figure");
    let model = crate::assets::skin::read_skinned_gltf(&resolve_model(&repo_data(), &fig.def.model)).unwrap();
    let reference_data = model.clips.iter().find(|c| c.name == "ref_walk").expect("ref_walk");
    let reference = SourceClip::from_data(reference_data, &fig.skeleton).unwrap();
    let key_times = reference_data.channels[0].times.clone();
    let h = &lib.humanoid;
    let skel = &fig.skeleton;
    // The library walk, retargeted at a rate whose frames land on its keys, and at the game's rate.
    let library = crate::assets::skin::read_skinned_gltf(&resolve_model(&repo_data(), "assets/models/characters/hos_test_clips/hos_test_clips.gltf")).unwrap();
    let rig_map = humanity_animation::RoleMap::parse(&std::fs::read_to_string(repo_data().join("characters/role_maps/hos_test_clips.ron")).unwrap(), h).unwrap();
    let rig = humanity_animation::Skeleton::from_skin(&library.joints, &rig_map, h).unwrap();
    let source = SourceClip::from_data(library.clips.iter().find(|c| c.name == "walk").unwrap(), &rig).unwrap();
    let keys = (key_times.len() - 1) as f32;
    let exact = humanity_animation::retarget::retarget(&source, &rig, skel, h, keys / source.duration * 4.0, true, 1.4);
    let baked = humanity_animation::retarget::retarget(&source, &rig, skel, h, lib.characters.clip_fps, true, 1.4);
    let bones: Vec<(usize, usize)> = h
        .roles
        .iter()
        .filter(|r| r.align)
        .filter_map(|r| Some((skel.joint_by_role(h, &r.role)?, skel.joint_by_role(h, r.aim.as_deref()?)?)))
        .collect();
    assert!(bones.len() >= 10, "the limbs map: {}", bones.len());
    let (mut a, mut b) = (Pose::default(), Pose::default());
    let (mut ma, mut mb): (Vec<Mat4>, Vec<Mat4>) = (Vec::new(), Vec::new());
    let mut compare = |clip: &humanity_animation::Clip, t: f32, limit: f32| -> f32 {
        clip.sample(skel, t, &mut a);
        reference.sample(skel, t, &mut b);
        pose::model_matrices(skel, &a, &mut ma);
        pose::model_matrices(skel, &b, &mut mb);
        let mut worst: f32 = 0.0;
        for &(j, aim) in &bones {
            let da = ma[aim].w_axis.truncate() - ma[j].w_axis.truncate();
            let db = mb[aim].w_axis.truncate() - mb[j].w_axis.truncate();
            let deg = da.normalize().dot(db.normalize()).clamp(-1.0, 1.0).acos().to_degrees();
            worst = worst.max(deg);
            assert!(deg < limit, "{} at {t:.3} s is {deg:.2} degrees off", skel.names[j]);
        }
        let hips = skel.joint_by_role(h, "hips").unwrap();
        let off = (ma[hips].w_axis.truncate() - mb[hips].w_axis.truncate()).length();
        assert!(off < 0.01, "the hips ride {off:.3} m off at {t:.3} s");
        worst
    };
    // The last key is the first again (a loop): sample before it.
    let exact_worst = key_times[..key_times.len() - 1].iter().map(|&t| compare(&exact, t, 0.5)).fold(0.0, f32::max);
    let baked_worst = (0..12).map(|k| compare(&baked, baked.duration * k as f32 / 12.0, 3.0)).fold(0.0, f32::max);
    eprintln!("worst limb direction off ref_walk: {exact_worst:.3} degrees at the keys, {baked_worst:.3} in the 30 fps clip");
}

/// Skin a body on the CPU the way both shaders do: each vertex shaped by the active targets, then
/// bent by its four joints' palette matrices.
fn skin_cpu(b: &LoadedBody, skel: &humanity_animation::Skeleton, p: &Pose, weights: &[(usize, f32)]) -> Vec<Vec3> {
    let mut m = Vec::new();
    let mut rows = Vec::new();
    pose::model_matrices(skel, p, &mut m);
    pose::palette_rows(skel, &m, &mut rows);
    let mut out = Vec::new();
    for prim in &b.model.primitives {
        for (i, v) in prim.vertices.iter().enumerate() {
            let mut pos = Vec3::from(v.position);
            for &(t, w) in weights {
                pos += Vec3::from(prim.morphs[t].positions[i]) * w;
            }
            let mut acc = Vec3::ZERO;
            for k in 0..4 {
                let w = v.weights[k] as f32 / 65535.0;
                if w > 0.0 {
                    let j = v.joints[k] as usize;
                    acc += pose::apply_rows(&[rows[j * 3], rows[j * 3 + 1], rows[j * 3 + 2]], pos) * w;
                }
            }
            out.push(acc);
        }
    }
    out
}

/// THE MAKEHUMAN BODIES, SKINNED ON THE CPU AS THE SHADERS DO: at rest each is exactly its bind
/// mesh, and playing every gait and stance on it keeps every vertex on the body and the feet near the
/// floor. Measured 2026-11-09 (the lowest vertex over each cycle, female / male): idle -0.005 /
/// -0.012, walk -0.011 / -0.016, jog -0.022 / -0.020, sprint -0.047 / -0.043, crouch walk -0.023 /
/// -0.005, sitting +0.002 / +0.003 m. Putting each foot ON the floor is rung 2's foot IK; this holds
/// the in-place clips to not sinking visibly (6 cm), and standing and walking to 2 cm.
/// Seen red 2026-11-09 twice: with the feet aligned to the clip rig's rest foot bone (humanoid.ron),
/// "female idle: the lowest vertex over the cycle is -0.026 m, more than 2 cm"; and with retarget's
/// rest alignment not passed down to the fingers, which the next test catches (these bounds are too
/// loose to).
#[test]
fn the_makehuman_bodies_skin_to_their_bind_mesh_and_stay_whole_in_every_clip() {
    let lib = read_library(&repo_data()).unwrap();
    for choice in ["female", "male"] {
        let b = body(&lib, choice);
        let skel = &b.skeleton;
        let rest = skin_cpu(b, skel, &skel.rest_pose(), &[]);
        let bind: Vec<Vec3> = b.model.primitives.iter().flat_map(|p| p.vertices.iter().map(|v| Vec3::from(v.position))).collect();
        let worst = rest.iter().zip(&bind).map(|(a, c)| (*a - *c).length()).fold(0.0_f32, f32::max);
        assert!(worst < 1e-4, "{choice}: at rest a vertex moved {worst} m");
        let mut p = Pose::default();
        for id in ["idle", "walk", "jog", "sprint", "crouch_walk", "sit_idle"] {
            let clip = b.clips.get(b.clips.index(id).unwrap());
            let mut lowest = f32::MAX;
            for k in 0..24 {
                clip.sample(skel, clip.duration * k as f32 / 24.0, &mut p);
                humanity_animation::retarget::apply_twists(skel, &lib.humanoid, &mut p);
                for v in skin_cpu(b, skel, &p, &[]) {
                    lowest = lowest.min(v.y);
                    assert!(v.y > -0.06 && v.y < 2.0, "{choice} {id}: a vertex at {v}");
                    assert!(v.x.abs() < 0.9 && v.z.abs() < 0.9, "{choice} {id}: a vertex at {v}");
                }
            }
            eprintln!("{choice} {id}: the lowest vertex over the cycle is {lowest:.3} m");
            if id == "idle" || id == "walk" {
                assert!(lowest > -0.02, "{choice} {id}: the lowest vertex over the cycle is {lowest:.3} m, more than 2 cm");
            }
        }
    }
}

/// A FINGER BENDS AGAINST ITS HAND AS THE CLIP BENDS IT: Quaternius's walk and idle (made on a rig
/// whose arms rest otherwise than MakeHuman's A-pose) played on each body turn each middle and index
/// finger, in its hand's own frame, by the same angle as on the clip's rig, within a degree. The
/// retargeter aligns the hand's rest onto the clip rig's, and a finger must ride that alignment, not
/// keep its own rest under a hand that has turned. (The angle of a finger's turn in its hand's frame
/// is what retargeting preserves exactly, whatever each rig's rest curl; the angle between the bones'
/// directions is not, so it is not compared.)
/// Seen red 2026-11-09 with the alignment not passed down from the hand (`rest_alignment` giving a
/// finger the identity): "female walk at 0.00 s: the left Middle finger turns 124.3 degrees in its
/// hand, the clip 78.0" (green: 0.00 degrees off).
#[test]
fn a_finger_bends_against_its_hand_as_the_clip_bends_it() {
    let lib = read_library(&repo_data()).unwrap();
    let h = &lib.humanoid;
    let file = crate::assets::skin::read_skinned_gltf(&resolve_model(&repo_data(), "assets/models/characters/quaternius_ual/quaternius_ual.gltf")).unwrap();
    let rig_map = humanity_animation::RoleMap::parse(&std::fs::read_to_string(repo_data().join("characters/role_maps/quaternius_ual.ron")).unwrap(), h).unwrap();
    let rig = humanity_animation::Skeleton::from_skin(&file.joints, &rig_map, h).unwrap();
    let rot = |m: &Mat4| m.to_scale_rotation_translation().1.normalize();
    // A finger's rotation in its hand's frame.
    let in_hand = |skel: &humanity_animation::Skeleton, m: &[Mat4], side: &str, finger: &str| -> glam::Quat {
        let j = |role: String| skel.joint_by_role(h, &role).unwrap_or_else(|| panic!("{} has no {role}", skel.rig));
        (rot(&m[j(format!("{side}Hand"))]).inverse() * rot(&m[j(format!("{side}{finger}Proximal"))])).normalize()
    };
    // How far a finger has turned in its hand's frame since rest, degrees.
    let bend = |skel: &humanity_animation::Skeleton, m: &[Mat4], rest: &[Mat4], side: &str, finger: &str| -> f32 {
        let d = (in_hand(skel, m, side, finger) * in_hand(skel, rest, side, finger).inverse()).normalize();
        2.0 * d.w.abs().clamp(0.0, 1.0).acos().to_degrees()
    };
    let mut worst: f32 = 0.0;
    for (clip_id, anim) in [("walk", "Walk_Loop"), ("idle", "Idle_Loop")] {
        let source = SourceClip::from_data(file.clips.iter().find(|c| c.name == anim).unwrap(), &rig).unwrap();
        for choice in ["female", "male"] {
            let b = body(&lib, choice);
            let clip = b.clips.get(b.clips.index(clip_id).unwrap());
            let (mut sp, mut bp) = (Pose::default(), Pose::default());
            let (mut sm, mut bm, mut s0, mut b0) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
            pose::model_matrices(&rig, &rig.rest_pose(), &mut s0);
            pose::model_matrices(&b.skeleton, &b.skeleton.rest_pose(), &mut b0);
            for k in 0..8 {
                let t = clip.duration * k as f32 / 8.0;
                source.sample(&rig, t, &mut sp);
                clip.sample(&b.skeleton, t, &mut bp);
                pose::model_matrices(&rig, &sp, &mut sm);
                pose::model_matrices(&b.skeleton, &bp, &mut bm);
                for side in ["left", "right"] {
                    for finger in ["Middle", "Index"] {
                        let on_rig = bend(&rig, &sm, &s0, side, finger);
                        let on_body = bend(&b.skeleton, &bm, &b0, side, finger);
                        let off = (on_rig - on_body).abs();
                        worst = worst.max(off);
                        assert!(off < 1.0, "{choice} {clip_id} at {t:.2} s: the {side} {finger} finger turns {on_body:.1} degrees in its hand, the clip {on_rig:.1}");
                    }
                }
            }
        }
    }
    eprintln!("a finger's bend against its hand: at most {worst:.2} degrees off the clip's");
}

/// A LOOK'S HEIGHT RESHAPES THE BODY, AND ITS SKELETON FOLLOWS: at look height 1.2 the female body
/// takes MakeHuman's height target (not a uniform scale), stands 1.2 times its own height (within a
/// centimetre), its knees rise with its longer legs, and its reshaped skeleton skins the reshaped mesh
/// at rest exactly onto itself (so the joints sit where the shaped flesh bends). At look height 1.0
/// nothing is reshaped.
/// Seen red 2026-11-09 with `Skeleton::reshaped` keeping the file's inverse binds: "at rest the
/// reshaped mesh moved 0.30523464 m" (the vertices dragged back toward the unshaped joints).
#[test]
fn a_looks_height_reshapes_the_body_and_its_skeleton_follows() {
    let lib = read_library(&repo_data()).unwrap();
    let b = body(&lib, "female");
    let (none, one) = b.shape_for(1.0);
    assert!(none.is_empty() && (one - 1.0).abs() < 1e-6, "{none:?} {one}");
    assert!(b.reshaped(&none).is_none());
    let (weights, scale) = b.shape_for(1.2);
    assert_eq!(weights.len(), 1);
    assert_eq!(weights[0].0, b.target("height_max").unwrap());
    assert!(weights[0].1 > 0.2 && weights[0].1 < 1.0, "{weights:?}");
    let skel = b.reshaped(&weights).expect("a reshaped skeleton");
    let shaped = skin_cpu(b, &skel, &skel.rest_pose(), &weights);
    let mut want = Vec::new();
    for prim in &b.model.primitives {
        for (i, v) in prim.vertices.iter().enumerate() {
            want.push(Vec3::from(v.position) + Vec3::from(prim.morphs[weights[0].0].positions[i]) * weights[0].1);
        }
    }
    let worst = shaped.iter().zip(&want).map(|(a, c)| (*a - *c).length()).fold(0.0_f32, f32::max);
    assert!(worst < 1e-4, "at rest the reshaped mesh moved {worst} m");
    let top = want.iter().map(|v| v.y).fold(f32::MIN, f32::max) * scale;
    assert!((top - 1.2 * b.height_m).abs() < 0.01, "stands {top:.3} m, wanted {:.3}", 1.2 * b.height_m);
    let knee = skel.joint_by_role(&lib.humanoid, "leftLowerLeg").unwrap();
    assert!(skel.rest_position(knee).y > b.skeleton.rest_position(knee).y + 0.02, "the knee rose with the leg");
    // Its stride is longer, so its clips play slower for a given speed.
    assert!(stride_scale(&skel, &b.skeleton, &lib.humanoid, scale) > 1.05);
}

/// The palette's morph block is laid out the same on both sides of the crate line (the renderer
/// compiles in the relay build without the animation crate, so it writes the number itself), and a
/// body's morph targets reach the GPU buffer at (target * vertices + vertex) * 2.
/// Seen red 2026-11-09 with the renderer's MORPH_SLOTS set to 3: the engine no longer compiles,
/// because `skin_push_palette` takes the block as an array of the renderer's size ("expected an array
/// with a size of 9, found one with a size of 6"), so the two cannot drift silently; the assertion
/// below says the same in words. The buffer layout seen red with the targets written in reverse
/// order: "target 0's position delta" (a first version that probed one target alone passed that
/// mutation, both swapped targets being zero at its vertex, so every target is compared).
#[test]
fn the_morph_block_and_the_morph_buffer_are_laid_out_as_the_shaders_read_them() {
    assert_eq!(crate::renderer::skinning::MORPH_SLOTS as usize, humanity_animation::MORPH_SLOTS, "the renderer and the animation crate disagree on the morph block");
    let rows = pose::morph_rows(&[(3, 0.5), (7, 0.25), (9, 0.0)], 1234);
    assert_eq!(rows[0], [2.0, 1234.0, 0.0, 0.0], "two targets on (the zero weight left out), 1234 vertices");
    assert_eq!(rows[1], [3.0, 0.5, 7.0, 0.25]);
    let lib = read_library(&repo_data()).unwrap();
    let b = body(&lib, "male");
    let deltas = crate::renderer::skinning::morph_deltas(&b.model);
    let n = b.model.vertex_count();
    assert_eq!(deltas.len(), 15 * n * 2);
    // The second primitive's third vertex under every target (the height targets move it, so the
    // targets cannot all agree there by being zero).
    let (first, second) = (&b.model.primitives[0], &b.model.primitives[1]);
    let i = first.vertices.len() + 2;
    assert!(second.morphs.iter().any(|m| m.positions[2] != [0.0; 3]), "a target moves the vertex checked");
    for k in 0..15 {
        let p = second.morphs[k].positions[2];
        assert_eq!(deltas[(k * n + i) * 2], [p[0], p[1], p[2], 0.0], "target {k}'s position delta");
        let q = second.morphs[k].normals[2];
        assert_eq!(deltas[(k * n + i) * 2 + 1], [q[0], q[1], q[2], 0.0], "target {k}'s normal delta");
    }
}

fn person(wearer: &'static str) -> Person {
    Person { key: FigureKey::Crowd(0), feet: Vec3::ZERO, facing: 0.0, height: 1.0, wearer, body: None, skin: None, hair: None, eyes: None, outfit: None }
}

/// Region tints: an equipped item's colour, else the wearer kind's default (players' teal top,
/// crew's amber), bare hands in the person's own skin, hair from their look, the iris their eye
/// colour, the white of the eye fixed.
/// Seen red 2026-11-08 with the wearer kind ignored (a crew member drawn in the player's teal).
#[test]
fn regions_take_the_outfit_then_the_wearers_defaults() {
    let lib = read_library(&repo_data()).unwrap();
    let chest = Tint::Slot("chest".into());
    let player = region_colour(&lib, &chest, &person("player"));
    let crew = region_colour(&lib, &chest, &person("crew"));
    assert_ne!(player, crew, "players and crew wear different defaults");
    assert!(player[2] > player[0], "the player's default top is the teal {player:?}");
    assert!(crew[0] > crew[2], "the crew's default top is the amber {crew:?}");
    let mut me = person("player");
    me.skin = Some([0.3, 0.2, 0.1]);
    me.hair = Some([0.9, 0.8, 0.1]);
    me.eyes = Some([0.1, 0.6, 0.2]);
    me.outfit = Some(humanity_core::cosmetics::OutfitColors { chest: Some([1.0, 0.0, 0.0]), ..Default::default() });
    assert_eq!(region_colour(&lib, &chest, &me), [1.0, 0.0, 0.0]);
    assert_eq!(region_colour(&lib, &Tint::Slot("hands".into()), &me), [0.3, 0.2, 0.1]);
    assert_eq!(region_colour(&lib, &Tint::Slot("head".into()), &me), [0.9, 0.8, 0.1]);
    assert_eq!(region_colour(&lib, &Tint::Eyes, &me), [0.1, 0.6, 0.2]);
    assert_eq!(region_colour(&lib, &Tint::Fixed(0.8, 0.8, 0.7), &me), [0.8, 0.8, 0.7]);
    assert_ne!(region_colour(&lib, &Tint::Eyes, &person("crew")), [0.1, 0.6, 0.2], "crew take their wearer's eyes");
}

/// The body faces where its camera looks; a turn of +Z about +Y reads back as its angle.
/// Seen red 2026-11-08 with the facing taken as the camera's yaw itself (the figure faced back
/// toward the camera at yaw 0).
#[test]
fn a_body_faces_where_its_camera_looks() {
    for yaw in [0.0_f32, 0.7, 2.0, -1.3, 3.0] {
        let f = facing_for_camera_yaw(yaw);
        let body_forward = Vec3::new(f.sin(), 0.0, f.cos());
        let cam_forward = Vec3::new(yaw.sin(), 0.0, -yaw.cos());
        assert!((body_forward - cam_forward).length() < 1e-5, "yaw {yaw}");
        let q = glam::Quat::from_rotation_y(1.1);
        assert!((facing_of_turn(q) - 1.1).abs() < 1e-5);
    }
}

/// The dev crowd: N stand-ins on rings inside the radius, walking at the speed they were given
/// along their circle's tangent; a path walked back and forth; nonsense refused.
/// Seen red 2026-11-08 with the circle's facing written along the radius (stand-ins walked
/// sideways).
#[test]
fn the_dev_crowd_walks_its_circles_and_paths() {
    let crowd = parse_crowd("12,8,1.4", Vec3::ZERO).unwrap();
    assert_eq!(crowd.len(), 12);
    for c in &crowd {
        let (p0, f) = crowd_place(c, 0.0);
        let (p1, _) = crowd_place(c, 0.1);
        assert!(p0.length() <= 8.0 + 1e-3);
        let v = (p1 - p0) / 0.1;
        assert!((v.length() - 1.4).abs() < 0.05, "speed {}", v.length());
        let facing = Vec3::new(f.sin(), 0.0, f.cos());
        assert!(facing.dot(v.normalize()) > 0.99, "faces the way it walks");
    }
    assert_eq!(parse_crowd("0", Vec3::ZERO).unwrap().len(), 0);
    assert!(parse_crowd("many", Vec3::ZERO).is_none());
    assert!(parse_crowd("99999", Vec3::ZERO).is_none());
    let path = parse_path("0,0,0; 0,0,4; 1.0").unwrap();
    let (a, _) = crowd_place(&path, 2.0);
    let (b, fb) = crowd_place(&path, 6.0);
    assert!((a - Vec3::new(0.0, 0.0, 2.0)).length() < 1e-4);
    assert!((b - Vec3::new(0.0, 0.0, 2.0)).length() < 1e-4, "on the way back at {b}");
    assert!((Vec3::new(fb.sin(), 0.0, fb.cos()) - Vec3::new(0.0, 0.0, -1.0)).length() < 1e-4, "facing back");
    assert!(parse_path("0,0,0;1.0").is_none());
}
