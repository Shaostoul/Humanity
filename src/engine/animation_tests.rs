//! The engine's character tests: the SHIPPED data and the committed test figure, read through the
//! real glTF skin reader. Each was seen red first; the red run is named at each test.

use glam::{Mat4, Vec3};
use humanity_animation::{pose, Pose, SourceClip};

use super::*;

fn repo_data() -> PathBuf {
    Path::new(env!("HUMANITY_REPO_ROOT")).join("data")
}

/// The shipped character data reads whole: the body file, its role map, the humanoid standard, the
/// five clips retargeted onto the body, the graph resolved against them, every body material given
/// a region.
/// Seen red 2026-11-08 with clips.ron's walk naming animation "Walk" (the file's is "walk"): "clip
/// walk: ... has no animation named Walk".
#[test]
fn the_shipped_character_data_reads_whole() {
    let lib = read_library(&repo_data()).expect("the shipped character data reads");
    assert_eq!(lib.body.id, "hos_test_figure");
    assert_eq!(lib.skeleton.joint_count(), 24);
    assert_eq!(lib.clips.clips.len(), 5);
    assert_eq!(lib.primitive_regions.len(), lib.model.primitives.len());
    for id in ["idle", "walk", "run", "turn_left", "turn_right"] {
        assert!(lib.clips.index(id).is_some(), "{id}");
    }
    assert!(lib.graph.state_index("move").is_some());
}

/// THE RETARGET TEST ON THE SHIPPED FILES: the clip library's walk (a T-pose rig with Mixamo
/// names, rolled bone frames and a 0.01 armature) retargeted onto the A-pose test figure lands on
/// `ref_walk`, the same motion authored directly on the figure by scripts/gen-test-figure.js.
/// Retargeted at a rate whose frames fall on the clips' own keys, every limb bone points within half
/// a degree of `ref_walk` at every key and the hips ride within a centimetre; the clip the game
/// plays (baked at characters.ron's 30 frames a second, between the keys) within three degrees,
/// which is resampling, not retargeting.
/// Seen red 2026-11-08 with the rest alignment left out of retarget (the first bone checked,
/// thigh.L, already 1.40 degrees off at the first key, against 0.04 with it); and, before the exact
/// half, the shipped clip alone read 1.72 degrees off on the right shin at a 1.5 degree limit, which
/// is why the exact comparison exists.
#[test]
fn the_librarys_walk_on_the_figure_lands_on_the_walk_made_on_the_figure() {
    let lib = read_library(&repo_data()).unwrap();
    let body = crate::assets::skin::read_skinned_gltf(&resolve_model(&repo_data(), &lib.body.model)).unwrap();
    let reference_data = body.clips.iter().find(|c| c.name == "ref_walk").expect("ref_walk");
    let reference = SourceClip::from_data(reference_data, &lib.skeleton).unwrap();
    let key_times = reference_data.channels[0].times.clone();
    let h = &lib.humanoid;
    let skel = &lib.skeleton;
    // The library walk, retargeted again at a rate whose frames land on its keys.
    let library = crate::assets::skin::read_skinned_gltf(&resolve_model(&repo_data(), "assets/models/characters/hos_test_clips/hos_test_clips.gltf")).unwrap();
    let rig_map = humanity_animation::RoleMap::parse(&std::fs::read_to_string(repo_data().join("characters/role_maps/hos_test_clips.ron")).unwrap(), h).unwrap();
    let rig = humanity_animation::Skeleton::from_skin(&library.joints, &rig_map, h).unwrap();
    let source = SourceClip::from_data(library.clips.iter().find(|c| c.name == "walk").unwrap(), &rig).unwrap();
    let keys = (key_times.len() - 1) as f32;
    let exact = humanity_animation::retarget::retarget(&source, &rig, skel, h, keys / source.duration * 4.0, true, 1.4);
    let shipped = lib.clips.get(lib.clips.index("walk").unwrap());
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
    let shipped_worst = (0..12).map(|k| compare(shipped, shipped.duration * k as f32 / 12.0, 3.0)).fold(0.0, f32::max);
    eprintln!("worst limb direction off ref_walk: {exact_worst:.3} degrees at the keys, {shipped_worst:.3} in the shipped 30 fps clip");
}

/// Skinned on the CPU the way both shaders do it, the figure at rest is its bind mesh, and walking
/// it keeps every vertex on the body (within 1 m of the hips sideways, under 2 m) and the feet near
/// the floor: no more than 5 cm under it at any moment of idle, walk, run or turn (measured
/// 2026-11-08: 2.3, 4.0, 4.5 and 3.0 cm). Putting each foot ON the floor is rung 2's foot IK; this
/// holds the in-place clips to not sinking visibly.
/// Seen red 2026-11-08 on the first generated walk, whose hips rode lowest at mid-stance (the
/// wrong way round): a toe 10.7 cm under the floor; and on the next, with no toe roll at push-off,
/// 8.8 cm.
#[test]
fn the_figure_skins_to_its_bind_mesh_at_rest_and_stays_whole_walking() {
    let lib = read_library(&repo_data()).unwrap();
    let skel = &lib.skeleton;
    let skin = |p: &Pose| -> Vec<Vec3> {
        let mut m = Vec::new();
        let mut rows = Vec::new();
        pose::model_matrices(skel, p, &mut m);
        pose::palette_rows(skel, &m, &mut rows);
        let mut out = Vec::new();
        for prim in &lib.model.primitives {
            for v in &prim.vertices {
                let pos = Vec3::from(v.position);
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
    };
    let rest = skin(&skel.rest_pose());
    let bind: Vec<Vec3> = lib.model.primitives.iter().flat_map(|p| p.vertices.iter().map(|v| Vec3::from(v.position))).collect();
    let worst = rest.iter().zip(&bind).map(|(a, b)| (*a - *b).length()).fold(0.0_f32, f32::max);
    assert!(worst < 1e-4, "at rest a vertex moved {worst} m");
    let mut p = Pose::default();
    for id in ["walk", "run", "idle", "turn_left"] {
        let clip = lib.clips.get(lib.clips.index(id).unwrap());
        let mut lowest = f32::MAX;
        for k in 0..24 {
            clip.sample(skel, clip.duration * k as f32 / 24.0, &mut p);
            for v in skin(&p) {
                lowest = lowest.min(v.y);
                assert!(v.y > -0.05 && v.y < 2.0, "{id}: a vertex at {v}");
                assert!(v.x.abs() < 1.0 && v.z.abs() < 1.0, "{id}: a vertex at {v}");
            }
        }
        eprintln!("{id}: the lowest vertex over the cycle is {lowest:.3} m");
    }
}

fn person(wearer: &'static str) -> Person {
    Person { key: FigureKey::Crowd(0), feet: Vec3::ZERO, facing: 0.0, height: 1.0, wearer, skin: None, hair: None, outfit: None }
}

/// Region tints: an equipped item's colour, else the wearer kind's default (players' teal top,
/// crew's amber), bare hands in the person's own skin, hair from their look.
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
    me.outfit = Some(humanity_core::cosmetics::OutfitColors { chest: Some([1.0, 0.0, 0.0]), ..Default::default() });
    assert_eq!(region_colour(&lib, &chest, &me), [1.0, 0.0, 0.0]);
    assert_eq!(region_colour(&lib, &Tint::Slot("hands".into()), &me), [0.3, 0.2, 0.1]);
    assert_eq!(region_colour(&lib, &Tint::Slot("head".into()), &me), [0.9, 0.8, 0.1]);
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
