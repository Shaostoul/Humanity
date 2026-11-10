//! THE LOOK, ON THE SHIPPED BODIES AND GARMENTS (2026-11-09; docs/design/character-animation.md
//! section 16). Each test builds a real person's own mesh (`humanity_animation::look::build_person`)
//! from the shipped MakeHuman bodies, detail targets and garments, poses it with the shipped clips,
//! and measures what is DRAWN: the mesh skinned on the CPU with the very palette rows the GPU gets
//! (`pose::apply_rows`), never the setup's own numbers (memory `feedback_checks_that_cannot_fail`).
//! Each has a red arm that must fail the same bound. `LOOK_DEBUG=1` prints the readings.

use glam::{Mat4, Quat, Vec3};
use humanity_animation::look::{build_person, look_key, PersonInputs};
use humanity_animation::{FootFrame, FootMode, FootPlacer, GarmentKind, LoadedBody, Part, PersonMesh, Pose};
use humanity_core::skin_data::SkinnedModelData;

use super::*;
use crate::engine::feet_ground::TestGround;

fn repo_data() -> PathBuf {
    Path::new(env!("HUMANITY_REPO_ROOT")).join("data")
}

fn debug() -> bool {
    std::env::var("LOOK_DEBUG").is_ok()
}

fn body_index(lib: &CharacterLibrary, choice: &str) -> usize {
    lib.body_index(choice).unwrap_or_else(|| panic!("no body {choice}"))
}

fn garments(lib: &CharacterLibrary, ids: &[&str]) -> Vec<usize> {
    let look = lib.look.as_ref().expect("the look reads");
    ids.iter().map(|id| look.garment(id).unwrap_or_else(|| panic!("no garment {id}"))).collect()
}

fn build(lib: &CharacterLibrary, bi: usize, detail: &std::collections::BTreeMap<String, f32>, worn: &[usize], covers: bool, soles: bool) -> PersonMesh {
    let b = &lib.bodies[bi];
    let inputs = PersonInputs { shape: &[], detail, garments: worn, covers, soles };
    let key = look_key(bi, &inputs);
    build_person(b, b.base.as_ref().expect("a MakeHuman body has its base file"), lib.look.as_ref().unwrap(), &lib.humanoid, &inputs, key)
}

/// The body as `LoadedBody` with a person's own mesh, skeleton and feet in place of its own, so the
/// clips (retargeted onto the body) pose the person's skeleton the way the frame does.
fn as_body(b: &LoadedBody, m: &PersonMesh) -> LoadedBody {
    let mut out = b.clone();
    out.skeleton = m.skeleton.clone();
    out.feet = m.feet.clone();
    out.model = m.model.clone();
    out.foot_vertices = m.foot_vertices.clone();
    out
}

/// Every vertex of a mesh skinned by palette rows (the shaders' arithmetic), world metres.
fn skin_all(model: &SkinnedModelData, rows: &[[f32; 4]], world: &Mat4) -> Vec<Vec<Vec3>> {
    model
        .primitives
        .iter()
        .map(|p| {
            p.vertices
                .iter()
                .map(|v| {
                    let pos = Vec3::from(v.position);
                    let mut s = Vec3::ZERO;
                    for k in 0..4 {
                        let w = v.weights[k] as f32 / 65535.0;
                        let j = v.joints[k] as usize;
                        if w > 0.0 && j * 3 + 2 < rows.len() {
                            s += pose::apply_rows(&[rows[j * 3], rows[j * 3 + 1], rows[j * 3 + 2]], pos) * w;
                        }
                    }
                    world.transform_point3(s)
                })
                .collect()
        })
        .collect()
}

/// A PERSON'S OWN MESH IS THEIR LOOK: built with nothing chosen it is the body as made (every drawn
/// vertex where the body's own is, the skeleton the body's); a detail slider moves the vertices its
/// targets name and the skeleton follows a target that moves a joint's vertices (a longer neck
/// raises the head joint); garments add their primitives, and a suit's covers drop the body's
/// triangles under it.
/// Seen red 2026-11-09 with `build_person` adding each detail delta to the glTF vertex by the glTF
/// index instead of through the base vertex it follows: first it passed (some vertex moved, just not
/// the nose's), so the test now holds that only the face moves, which the same mistake fails: "the
/// nose slider moved vertices away from the face: [Vec3(0.0, 1.4069183, -0.009236666), ...]".
#[test]
fn a_persons_own_mesh_is_their_look() {
    let lib = read_library(&repo_data()).expect("the shipped character data reads");
    assert!(lib.look.is_some(), "the look reads: {:?}", lib.warnings);
    let bi = body_index(&lib, "female");
    let b = &lib.bodies[bi];
    let none = std::collections::BTreeMap::new();
    let plain = build(&lib, bi, &none, &[], true, true);
    let mut worst = 0.0_f32;
    for (p, q) in plain.model.primitives.iter().zip(&b.model.primitives) {
        for (a, c) in p.vertices.iter().zip(&q.vertices) {
            worst = worst.max((Vec3::from(a.position) - Vec3::from(c.position)).length());
        }
    }
    assert!(worst < 1.0e-5, "the body as made moved {worst} m");
    for j in 0..b.skeleton.joint_count() {
        assert!((plain.skeleton.rest_position(j) - b.skeleton.rest_position(j)).length() < 1.0e-4, "joint {} moved with nothing chosen", b.skeleton.names[j]);
    }
    // A detail slider: the nose's bump, at its full high end.
    let nose: std::collections::BTreeMap<String, f32> = [("nose_hump".to_string(), 1.0)].into_iter().collect();
    let bumped = build(&lib, bi, &nose, &[], true, true);
    let moves: Vec<(Vec3, f32)> = bumped
        .model
        .primitives
        .iter()
        .zip(&plain.model.primitives)
        .flat_map(|(a, c)| a.vertices.iter().zip(&c.vertices).map(|(x, y)| (Vec3::from(y.position), (Vec3::from(x.position) - Vec3::from(y.position)).length())))
        .collect();
    let moved = moves.iter().map(|m| m.1).fold(0.0_f32, f32::max);
    assert!(moved > 0.002, "the nose moved {moved:.3} m");
    // And only the face moved: every vertex that moved a millimetre stands above the neck joint and
    // in front of the head joint (where the nose is).
    let neck = b.skeleton.joint_by_role(&lib.humanoid, "neck").map(|j| b.skeleton.rest_position(j)).expect("a neck joint");
    let head_at = b.skeleton.joint_by_role(&lib.humanoid, "head").map(|j| b.skeleton.rest_position(j)).expect("a head joint");
    let astray: Vec<Vec3> = moves.iter().filter(|m| m.1 > 0.001 && (m.0.y < neck.y || m.0.z < head_at.z)).map(|m| m.0).take(3).collect();
    assert!(astray.is_empty(), "the nose slider moved vertices away from the face: {astray:?}");
    // The skeleton follows: a longer neck stands the head joint higher.
    let neck: std::collections::BTreeMap<String, f32> = [("neck_vert".to_string(), 1.0)].into_iter().collect();
    let tall = build(&lib, bi, &neck, &[], true, true);
    let head = b.skeleton.joint_by_role(&lib.humanoid, "head").expect("a head joint");
    let rise = tall.skeleton.rest_position(head).y - plain.skeleton.rest_position(head).y;
    assert!(rise > 0.005, "a longer neck raised the head joint {rise:.4} m");
    // Garments: a suit and shoes add two primitives and hide body triangles; without covers, none.
    let worn = garments(&lib, &["female_casualsuit01", "shoes01", "bob01", "eyebrow001", "eyelashes01"]);
    let dressed = build(&lib, bi, &none, &worn, true, true);
    let bare = build(&lib, bi, &none, &worn, false, true);
    let tris = |m: &PersonMesh| m.model.primitives.iter().zip(&m.parts).filter(|(_, p)| matches!(p, Part::Region(_))).map(|(p, _)| p.indices.len() / 3).sum::<usize>();
    let garment_parts = dressed.parts.iter().filter(|p| matches!(p, Part::Garment(_))).count();
    assert_eq!(garment_parts, 5, "five garments drawn");
    assert!(tris(&dressed) + 5_000 < tris(&bare), "the suit and shoes hide the body under them: {} against {} triangles", tris(&dressed), tris(&bare));
    // Real brows hide the generated ones.
    let brows = |m: &PersonMesh| m.model.primitives.iter().any(|p| p.material == "brows");
    assert!(brows(&plain) && !brows(&dressed), "the generated brows give way to the worn ones");
    if debug() {
        eprintln!("nose moved {moved:.4} m, head rose {rise:.4} m, body triangles {} dressed / {} without covers", tris(&dressed), tris(&bare));
    }
}

/// THE BUILD'S COST AND MEMORY, measured (section 8.3 of the plan): a dressed person, built
/// several times, and the bytes their mesh asks of the GPU. Printed with LOOK_DEBUG=1; the bound
/// is generous (a debug build): it catches a build gone quadratic, not a slow machine.
#[test]
fn a_persons_build_is_quick_and_its_memory_known() {
    let lib = read_library(&repo_data()).unwrap();
    let bi = body_index(&lib, "male");
    let detail: std::collections::BTreeMap<String, f32> = [("nose_hump", 0.6), ("chin_width", -0.4), ("m_waist_circ", 0.5), ("ear_scale", 0.3)].into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    let worn = garments(&lib, &["male_casualsuit02", "shoes03", "short02", "eyebrow003", "eyelashes03"]);
    let t0 = std::time::Instant::now();
    let runs = 5;
    let mut m = None;
    for _ in 0..runs {
        m = Some(build(&lib, bi, &detail, &worn, true, true));
    }
    let ms = t0.elapsed().as_secs_f32() * 1000.0 / runs as f32;
    let m = m.unwrap();
    let verts = m.model.vertex_count();
    let indices: usize = m.model.primitives.iter().map(|p| p.indices.len()).sum();
    let gpu_bytes = verts * humanity_core::vertex::SkinnedVertex::BYTES + indices * 4 + 32;
    let near_out = verts * 32;
    eprintln!("a dressed person: {ms:.2} ms a build ({} profile), {verts} vertices, {indices} indices: {:.2} MB of mesh, {:.2} MB more for a near place's skinned copy", if cfg!(debug_assertions) { "debug" } else { "release" }, gpu_bytes as f64 / 1048576.0, near_out as f64 / 1048576.0);
    assert!(ms < 400.0, "a build took {ms:.1} ms");
}

/// Walk a person's own mesh through a clip on a flat floor with the feet on, the way the frame
/// does (their skeleton, their rig, the lift of their soles in the world matrix), and return each
/// planted frame's lowest drawn foot vertex over the floor (metres).
fn planted_clearances(lib: &CharacterLibrary, b: &LoadedBody, m: &PersonMesh, clip_id: &str, secs: f32) -> Vec<f32> {
    let pb = as_body(b, m);
    let rig = pb.feet.as_ref().expect("legs");
    let clip = b.clips.get(b.clips.index(clip_id).unwrap());
    let floor = TestGround { floor: 0.0, stair: None, slope: 0.0 };
    let def = lib.characters.feet;
    let dt = 1.0 / 60.0;
    let (mut pose, mut model, mut rows) = (Pose::default(), Vec::new(), Vec::new());
    let mut placer = FootPlacer::default();
    let mut out = Vec::new();
    for k in 0..(secs / dt) as usize {
        let t = k as f32 * dt;
        clip.sample(&pb.skeleton, t, &mut pose);
        humanity_animation::retarget::apply_twists(&pb.skeleton, &lib.humanoid, &mut pose);
        let world = Mat4::from_rotation_translation(Quat::IDENTITY, Vec3::new(clip.speed_mps * t, m.lift, 0.0));
        pose::model_matrices(&pb.skeleton, &pose, &mut model);
        let contacts = clip.contacts_at(t);
        placer.place(rig, &pb.skeleton, &lib.humanoid, &mut pose, &mut model, &FootFrame { world, contacts, airborne: false, dt }, &floor, FootMode::Full, &def);
        rows.clear();
        pose::palette_rows(&pb.skeleton, &model, &mut rows);
        let feet = crate::engine::foot_probe::skinned_feet(&pb.model, &pb.foot_vertices, &[], &rows, &world);
        if t < 0.5 {
            continue;
        }
        for i in 0..2 {
            if contacts[i] >= 0.9 {
                if let Some(low) = feet[i].iter().map(|p| p.y).reduce(f32::min) {
                    out.push(low);
                }
            }
        }
    }
    out
}

/// SHOD FEET STAND ON THE FLOOR (rung 2a's lesson for the clothes: "a SHOE with a sole of its own
/// changes it"): a person wearing each shipped pair of shoes, standing and walking with the feet on,
/// has every planted foot's lowest DRAWN vertex (the shoe's sole) within the foot probe's gate,
/// 1.5 cm in to 2.5 cm over the floor, because the soles are measured from their own mesh with the
/// shoes on and the person is drawn that much higher. With the soles of the body as made (the red
/// arm, `soles: false`), the shoes with a real sole sink through the floor by it.
/// Seen red 2026-11-09 by that arm: shoes01 (female, idle) 0.0xx m in (the readings below).
#[test]
fn shod_feet_stand_on_the_floor() {
    let lib = read_library(&repo_data()).unwrap();
    let none = std::collections::BTreeMap::new();
    let mut deepest_red = 0.0_f32;
    for choice in ["female", "male"] {
        let bi = body_index(&lib, choice);
        let b = &lib.bodies[bi];
        for shoe in ["shoes01", "shoes02", "shoes03", "shoes04", "shoes05", "shoes06"] {
            let worn = garments(&lib, &[shoe]);
            let shod = build(&lib, bi, &none, &worn, true, true);
            let red = build(&lib, bi, &none, &worn, true, false);
            for clip in ["idle", "walk"] {
                let on = planted_clearances(&lib, b, &shod, clip, 2.5);
                let off = planted_clearances(&lib, b, &red, clip, 2.5);
                let (lo, hi) = (on.iter().copied().fold(f32::INFINITY, f32::min), on.iter().copied().fold(f32::NEG_INFINITY, f32::max));
                let red_lo = off.iter().copied().fold(f32::INFINITY, f32::min);
                if debug() {
                    eprintln!("{choice} {shoe} {clip}: lift {:.3} m; planted {lo:+.3} to {hi:+.3} m over {} readings; soles not measured: {red_lo:+.3}", shod.lift, on.len());
                }
                assert!(on.len() > 20, "{choice} {shoe} {clip}: only {} planted readings", on.len());
                assert!(lo > -0.015, "{choice} {shoe} {clip}: a shod foot reaches {:.3} m into the floor", -lo);
                assert!(hi < 0.025, "{choice} {shoe} {clip}: a shod foot hovers {hi:.3} m over the floor");
                deepest_red = deepest_red.min(red_lo);
            }
        }
    }
    assert!(deepest_red < -0.015, "the red arm (soles not measured) should sink a shoe through the floor, read {deepest_red:.3}");
}

/// Each body vertex drawn under a garment that stands OUTSIDE it, in a pose: the worst (metres the
/// skin stands through the cloth) and how many. For every body vertex still drawn in a region the
/// garment covers, a ray along the skin's own outward normal (from the skinned triangles: the body's
/// winding is what the renderer culls by, so it faces out) is cast both ways against the garment's
/// skinned triangles within 3 cm: cloth in front of the skin covers it; cloth only BEHIND it means
/// the skin is through the cloth by that much. A vertex with no cloth either way within 3 cm is not
/// under the garment (a bare forearm under a short sleeve, a hand beside a trouser leg).
fn poke_through(lib: &CharacterLibrary, b: &LoadedBody, m: &PersonMesh, clip_id: &str, t: f32) -> (f32, usize) {
    const REACH: f32 = 0.03;
    let look = lib.look.as_ref().unwrap();
    let base = b.base.as_ref().unwrap();
    let pb = as_body(b, m);
    let clip = b.clips.get(b.clips.index(clip_id).unwrap());
    let (mut pose, mut model, mut rows) = (Pose::default(), Vec::new(), Vec::new());
    clip.sample(&pb.skeleton, t, &mut pose);
    humanity_animation::retarget::apply_twists(&pb.skeleton, &lib.humanoid, &mut pose);
    pose::model_matrices(&pb.skeleton, &pose, &mut model);
    pose::palette_rows(&pb.skeleton, &model, &mut rows);
    let drawn = skin_all(&pb.model, &rows, &Mat4::IDENTITY);
    // Each body primitive's first glTF vertex, by region (the own mesh copies a body primitive's
    // vertices whole and keeps only some of its triangles, so a vertex index is the glTF one).
    let mut first = Vec::new();
    let mut at = 0usize;
    for (pi, p) in b.model.primitives.iter().enumerate() {
        first.push((b.primitive_regions[pi], at));
        at += p.vertices.len();
    }
    let mut worst = 0.0_f32;
    let mut count = 0usize;
    for (gp, part) in m.parts.iter().enumerate() {
        let Part::Garment(g) = *part else { continue };
        let gar = &look.garments[g];
        if !matches!(gar.def.kind, GarmentKind::Clothes | GarmentKind::Shoes) {
            continue;
        }
        let covered: Vec<u8> = gar.def.covers.iter().filter_map(|c| look.shared.region(c)).collect();
        let prim = &m.model.primitives[gp];
        let pts = &drawn[gp];
        // The garment's triangles in a grid by their corners, 3 cm cells.
        let cell = REACH;
        let key = |p: Vec3| ((p.x / cell).floor() as i32, (p.y / cell).floor() as i32, (p.z / cell).floor() as i32);
        let mut grid: std::collections::HashMap<(i32, i32, i32), Vec<usize>> = std::collections::HashMap::new();
        for (ti, tri) in prim.indices.chunks_exact(3).enumerate() {
            let mut keys: Vec<(i32, i32, i32)> = tri.iter().map(|&k| key(pts[k as usize])).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                grid.entry(k).or_default().push(ti);
            }
        }
        for (bp, bpart) in m.parts.iter().enumerate() {
            let Part::Region(r) = *bpart else { continue };
            let Some(&(_, g0)) = first.iter().find(|(rr, _)| *rr == r) else { continue };
            let region_prim = &m.model.primitives[bp];
            let used: std::collections::HashSet<u32> = region_prim.indices.iter().copied().collect();
            let bpts = &drawn[bp];
            let mut bn = vec![Vec3::ZERO; bpts.len()];
            for tri in region_prim.indices.chunks_exact(3) {
                let f = (bpts[tri[1] as usize] - bpts[tri[0] as usize]).cross(bpts[tri[2] as usize] - bpts[tri[0] as usize]);
                for &k in tri {
                    bn[k as usize] += f;
                }
            }
            for (vi, p) in bpts.iter().enumerate() {
                if !used.contains(&(vi as u32)) {
                    continue;
                }
                let Some(bv) = base.sole(g0 + vi) else { continue };
                if !covered.contains(&look.shared.regions[bv as usize]) {
                    continue;
                }
                let n = bn[vi].normalize_or_zero();
                let (cx, cy, cz) = key(*p);
                let (mut front, mut behind) = (f32::INFINITY, f32::INFINITY);
                let mut seen = std::collections::HashSet::new();
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        for dz in -1..=1 {
                            let Some(list) = grid.get(&(cx + dx, cy + dy, cz + dz)) else { continue };
                            for &ti in list {
                                if !seen.insert(ti) {
                                    continue;
                                }
                                let tri = &prim.indices[ti * 3..ti * 3 + 3];
                                if let Some(d) = ray_hit(*p, n, pts[tri[0] as usize], pts[tri[1] as usize], pts[tri[2] as usize]) {
                                    if d >= 0.0 && d <= REACH {
                                        front = front.min(d);
                                    } else if d < 0.0 && -d <= REACH {
                                        behind = behind.min(-d);
                                    }
                                }
                            }
                        }
                    }
                }
                if front.is_infinite() && behind.is_finite() && behind > 0.002 {
                    count += 1;
                    worst = worst.max(behind);
                    if debug() && count <= 3 {
                        eprintln!("    {} through {}: {:?} {:.4} m through", region_prim.material, gar.def.id, p, behind);
                    }
                }
            }
        }
    }
    (worst, count)
}

/// Where the line through `o` along `d` meets triangle (a, b, c): the signed distance along `d`, or
/// None (Moller and Trumbore's test, both directions).
fn ray_hit(o: Vec3, d: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let e1 = b - a;
    let e2 = c - a;
    let h = d.cross(e2);
    let det = e1.dot(h);
    if det.abs() < 1e-12 {
        return None;
    }
    let s = o - a;
    let u = s.dot(h) / det;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) / det;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    Some(e2.dot(q) / det)
}

/// NO SKIN THROUGH THE CLOTHES, IN POSES (design 2.4: "so the skin under a sleeve is not drawn and
/// never pokes through"): each of four shipped suits on its body, walking, crouched and sitting, the
/// DRAWN mesh skinned with the clips' palettes: the casual outfits and the overalls leave no more
/// than 4 body vertices through them (by more than 2 mm), none by a centimetre (measured: 0 in every
/// pose); the sport top leaves at most 24 (measured: 2 walking, 19 crouched, 9 seated, at the small
/// of the back where its hem rides over the skin; a known limit). With covers off (the red arm: the
/// body under the suit drawn), skin stands through every suit in every pose, 215 to 766 vertices,
/// 3 cm (the ray's reach).
/// Seen red 2026-11-09, four times on the way: with MakeHuman's delete_verts kept only in regions a
/// fifth hidden, 304 vertices through, 2.4 cm (female_casualsuit01, walk); with every region its
/// list touches, 67 (the painted shirt's smoothing stood the waist out); drawn unsmoothed under the
/// cloth, 6 to 8 in every pose, 2.8 cm (tight cloth over skin MakeHuman's list keeps); with skin
/// within 1.2 cm of the cloth hidden too, 0 for the casual suit and 4 at the sport top's hem, 2.2 cm,
/// and 13 and 19 crouched and seated; with skin under the cloth along its normal hidden instead
/// (`COVER_REACH_M`), as now, every outfit but the sport top at 0. The readings: `LOOK_DEBUG=1`.
#[test]
fn no_skin_pokes_through_the_clothes_in_poses() {
    let lib = read_library(&repo_data()).unwrap();
    let none = std::collections::BTreeMap::new();
    let mut red_total = 0usize;
    let mut fails = Vec::new();
    // Each outfit with the most body vertices it may leave through it in a pose: none to speak of for
    // the casual outfits and the overalls; the sport top (a fitted top over leggings) leaves up to 19
    // at the small of the back in a crouch and 9 in a seat, its hem riding over the skin there (a
    // known limit, docs/design/character-animation.md section 16).
    for (choice, suit, shoe, most) in [("female", "female_casualsuit01", "shoes01", 4usize), ("female", "female_sportsuit01", "shoes02", 24), ("male", "male_casualsuit02", "shoes03", 4), ("male", "male_worksuit01", "shoes05", 4)] {
        let bi = body_index(&lib, choice);
        let b = &lib.bodies[bi];
        let worn = garments(&lib, &[suit, shoe]);
        let covered = build(&lib, bi, &none, &worn, true, true);
        let bare = build(&lib, bi, &none, &worn, false, true);
        for (clip, t) in [("walk", 0.3), ("walk", 0.8), ("crouch_walk", 0.4), ("sit_idle", 1.0)] {
            let (w, n) = poke_through(&lib, b, &covered, clip, t);
            let (rw, rn) = poke_through(&lib, b, &bare, clip, t);
            if debug() {
                eprintln!("{choice} {suit} {clip} @{t}: covers on {n} vertices out, worst {w:.4} m; covers off {rn} out, worst {rw:.4} m");
            }
            if n > most || (most <= 4 && w >= 0.01) {
                fails.push(format!("{choice} {suit} {clip} at {t} s: {n} body vertices through the clothes, the worst {w:.3} m out"));
            }
            red_total += rn;
        }
    }
    assert!(fails.is_empty(), "{fails:#?}");
    assert!(red_total > 400, "the red arm (covers off) should show skin through the clothes, found {red_total}");
}
