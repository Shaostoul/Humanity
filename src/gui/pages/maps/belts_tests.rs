//! The belts' tests (moved out of belts.rs to keep it within its line budget,
//! tests/file_size_ratchet.rs; 2026-10-21).

use super::*;

/// The shipped belts are real: 20,000 orbits in four classes, with the
/// MPC's credit and its header shipped beside them; the main belt's
/// orbits lie between Mars and Jupiter, the trojans at Jupiter's distance,
/// the Kuiper belt beyond Neptune.
///
/// Seen red 2026-10-21 with the header file left out of the data folder:
/// "data/maps/MPCORB-header.txt does not ship beside the belts".
#[test]
fn the_shipped_belts_are_the_real_minor_planets_with_their_credit() {
    let set = belt_set();
    assert!(set.len() >= 19_000, "only {} belt orbits", set.len());
    assert!(set.credit.contains("Minor Planet Center"), "no MPC credit: {:?}", set.credit);
    let header = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/data/maps/MPCORB-header.txt"));
    let header = header.expect("data/maps/MPCORB-header.txt does not ship beside the belts");
    assert!(header.contains("Software programs may include this datafile"), "the header is not MPCORB's");
    let class = |id: &str| set.classes.iter().position(|c| c.id == id).unwrap() as u8;
    let a_of = |c: u8| set.orbits.iter().filter(|o| o.class == c).map(|o| o.a).collect::<Vec<_>>();
    let median = |mut v: Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    };
    let (main, trojan, distant) = (median(a_of(class("main"))), median(a_of(class("trojan"))), median(a_of(class("distant"))));
    assert!((2.2..3.3).contains(&main), "the main belt's median orbit is {main:.2} AU");
    assert!((5.0..5.4).contains(&trojan), "the trojans' median orbit is {trojan:.2} AU");
    assert!(distant > 30.0, "the distant objects' median orbit is {distant:.1} AU");
}

/// The trojans gather at Jupiter's L4 and L5: on the page's date, most of
/// them stand 40 to 80 degrees from Jupiter round the Sun, the two swarms
/// either side of it.
///
/// Seen red 2026-10-21 with each orbit's mean anomaly left at its epoch
/// value plus a full extra half turn (`m0 + PI`): "only 1% of the trojans
/// are 40 to 80 degrees from Jupiter".
#[test]
fn the_trojans_swarm_sixty_degrees_from_jupiter() {
    let set = belt_set();
    let t = 843_782_400.0;
    let mut cache = BeltCache::default();
    cache.update(set, t, false);
    let jupiter = crate::cosmos::body_world_position_3d_au(crate::cosmos::find_body("jupiter").unwrap(), t);
    let tro = set.classes.iter().position(|c| c.id == "trojan").unwrap() as u8;
    let flat = |v: DVec3| DVec3::new(v.x, 0.0, v.z).normalize();
    let (mut near60, mut all, mut ahead, mut behind) = (0, 0, 0, 0);
    for (o, p) in set.orbits.iter().zip(&cache.pos_au) {
        if o.class != tro {
            continue;
        }
        all += 1;
        let ang = flat(*p).dot(flat(jupiter)).clamp(-1.0, 1.0).acos().to_degrees();
        if (40.0..80.0).contains(&ang) {
            near60 += 1;
            // Ahead of Jupiter: the turn from Jupiter to it is the way the
            // planets go round, counter-clockwise seen from ecliptic north (+Y).
            if flat(jupiter).cross(flat(*p)).y > 0.0 { ahead += 1 } else { behind += 1 }
        }
    }
    let share = near60 as f64 / all as f64;
    assert!(share > 0.6, "only {:.0}% of the trojans are 40 to 80 degrees from Jupiter", share * 100.0);
    assert!(ahead > all / 5 && behind > all / 5, "the two swarms: {ahead} ahead, {behind} behind of {all}");
}

/// Seen from far out, the inner belts thin with the square of the zoom so
/// their points never pile up into a solid blob: at the page's 60 AU
/// overview no main-belt point adds more than 3% of its colour, while at
/// 11 AU (the belts 165 px across) they draw at full strength.
///
/// Seen red 2026-10-21 with the dust premultiplied by egui's
/// `from_rgba_unmultiplied` (in linear light): "at 60 AU a main-belt point
/// adds 33 of 255 of its colour, over 8".
#[test]
fn far_out_the_belts_thin_instead_of_piling_up() {
    use crate::gui::pages::maps::camera::{MapCamera, MapProjector, MapScene};
    let set = belt_set();
    let t = 843_782_400.0;
    let mut cache = BeltCache::default();
    cache.update(set, t, false);
    let scene = MapScene::new(None, t, false);
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(755.0, 745.0));
    let rock = Color32::from_rgba_unmultiplied(209, 189, 158, 140); // theme-exempt: fixed test colours, the theme token values
    let ice = Color32::from_rgba_unmultiplied(158, 189, 224, 140); // theme-exempt: fixed test colours, the theme token values
    let main = set.classes.iter().position(|c| c.id == "main").unwrap() as u8;
    // The brightest premultiplied red a main-belt point adds, at a view.
    let brightest = |cam: MapCamera| {
        let proj = MapProjector::new(cam.pose(&scene), rect);
        let mut mesh = Mesh::default();
        add_dust(&mut mesh, set, &mut cache.clone(), &proj, &scene, rock, ice);
        let mut i = 0usize;
        let mut best = 0u8;
        for (o, p) in set.orbits.iter().zip(&cache.pos_au) {
            let Some((at, _)) = proj.project((*p - scene.earth_display_au()) * M_PER_AU) else { continue };
            if !rect.contains(at) {
                continue;
            }
            if o.class == main {
                best = best.max(mesh.vertices[i * 3].color.r());
            }
            i += 1;
        }
        best
    };
    let far = brightest(MapCamera::default());
    assert!(far <= 8, "at 60 AU a main-belt point adds {far} of 255 of its colour, over 8");
    let near = brightest(MapCamera::overview(11.0 * M_PER_AU));
    assert!(near >= 60, "at 11 AU a main-belt point adds only {near} of 255");
}

/// The dust moves only when the clock has moved a visible amount (or
/// Diagram is switched), so a live map does not solve 20,000 Kepler
/// equations every frame.
///
/// Seen red 2026-10-21 with the cache never reused: "assertion `left ==
/// right` failed: re-propagated within 21600 s".
#[test]
fn the_dust_is_propagated_only_when_the_clock_moves_far_enough() {
    let set = belt_set();
    let mut cache = BeltCache::default();
    cache.update(set, 0.0, false);
    cache.update(set, 60.0, false);
    cache.update(set, REPROPAGATE_S, false);
    assert_eq!(cache.propagations, 1, "re-propagated within {REPROPAGATE_S} s");
    cache.update(set, REPROPAGATE_S + 1.0, false);
    assert_eq!(cache.propagations, 2);
    cache.update(set, REPROPAGATE_S + 1.0, true);
    assert_eq!(cache.propagations, 3, "switching Diagram did not move the dust");
}

/// A map that has not moved draws the dust from its last mesh: the same
/// points, built once; turning the camera builds it again.
///
/// Seen red 2026-10-21 with the reuse test forced false (every frame
/// rebuilt, as before the cache): "assertion `left == right` failed: a
/// still map built the dust again / left: 2 / right: 1". The release sweep
/// that found it: the overview's symbols took 3.5 ms, 2.7 of them the dust.
/// Then, the first cache still at 3.7 ms in the sweep, seen red again with the
/// pose compared exactly (a recomputed pose carries float noise): "left: 2 /
/// right: 1" at the millimetre-off pose.
#[test]
fn a_still_map_reuses_the_dust_and_a_turned_one_builds_it_again() {
    use crate::gui::pages::maps::camera::{MapCamera, MapProjector, MapScene};
    let set = belt_set();
    let t = 843_782_400.0;
    let mut cache = BeltCache::default();
    cache.update(set, t, false);
    let scene = MapScene::new(None, t, false);
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(755.0, 745.0));
    let rock = Color32::from_rgba_unmultiplied(209, 189, 158, 140); // theme-exempt: fixed test colours, the theme token values
    let ice = Color32::from_rgba_unmultiplied(158, 189, 224, 140); // theme-exempt: fixed test colours, the theme token values
    let cam = MapCamera::overview(11.0 * M_PER_AU);
    let proj = MapProjector::new(cam.pose(&scene), rect);
    let mut first = Mesh::default();
    let n1 = add_dust(&mut first, set, &mut cache, &proj, &scene, rock, ice);
    let mut second = Mesh::default();
    let n2 = add_dust(&mut second, set, &mut cache, &proj, &scene, rock, ice);
    assert_eq!(cache.mesh_builds, 1, "a still map built the dust again");
    assert!(n1 > 1000, "only {n1} points drawn");
    assert_eq!((n1, first.vertices.len()), (n2, second.vertices.len()), "the reused dust differs");
    assert_eq!(first.vertices.first().map(|v| v.pos), second.vertices.first().map(|v| v.pos));
    // The live clock: a second on, Earth (the frame's centre) and the camera
    // over the Sun have both moved in the map frame, and the picture has not.
    let scene_later = MapScene::new(None, t + 1.0, false);
    let proj_later = MapProjector::new(cam.pose(&scene_later), rect);
    add_dust(&mut Mesh::default(), set, &mut cache, &proj_later, &scene_later, rock, ice);
    assert_eq!(cache.mesh_builds, 1, "a second of live clock built the dust again");
    // A pose recomputed with float noise in it (a millimetre at 11 AU) is the
    // same picture; compared exactly it never matched twice.
    let mut noisy = cam.pose(&scene);
    noisy.eye += glam::DVec3::splat(1e-3);
    noisy.look_at += glam::DVec3::splat(1e-3);
    add_dust(&mut Mesh::default(), set, &mut cache, &MapProjector::new(noisy, rect), &scene, rock, ice);
    assert_eq!(cache.mesh_builds, 1, "a pose a millimetre off built the dust again");
    let turned = MapProjector::new(MapCamera::overview(12.0 * M_PER_AU).pose(&scene), rect);
    add_dust(&mut Mesh::default(), set, &mut cache, &turned, &scene, rock, ice);
    assert_eq!(cache.mesh_builds, 2, "a moved camera reused the old dust");
}
