//! The fruiting tent's fitted cover (`engine::fruiting_tent`), against the shipped cover data
//! (data/garden/grow_media.ron, the mushroom rack's `enclosure`).
//!
//! SEEN RED FIRST (2026-11-13), one run with the module made to draw the old tent: the cover
//! returning the five flat quads of the old tent geometry (plant_layout, removed since) at the
//! RACK's size as its film and nothing else, and the film's material returning the old glass
//! constants (type 4, roughness 0.12, alpha 0.18, no transmission). Five of the six failed:
//! - the clearance: "film vertex [-0.6, 0.0, 0.3] is 0.000 m from the rack's frame (at least 0.02)";
//! - the sag: "the back panel bows in only 0.0000 m between shelves (sag 0.015)";
//! - the door: "hanging down, the door stops 340282346638528859811704183484516925440.000 m above
//!   the floor" (no door panel to find: the old front face was one quad, corner to corner);
//! - the material: "material type 4 is the glass type";
//! - the clean mesh: no seams or fittings at all.
//! The roll timing passed, as nothing of it was changed.

use super::*;
use humanity_systems::grow_machines::{load_grow_media, Cover, Enclosure};

/// The mushroom rack's size (width, height, depth, metres: `mushroom_rack` in
/// data/machines/home.ron) and its shelves (its medium's five stacked plots).
const RACK: (f32, f32, f32) = (1.2, 1.8, 0.6);

fn shipped() -> (Enclosure, RackFrame) {
    let media = load_grow_media(&std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data"));
    let m = media.iter().find(|m| m.matches("mushroom_rack")).expect("the mushroom rack's medium");
    let e = m.enclosure.clone().expect("the rack's tent");
    (e, RackFrame::of(RACK, m.plots, m.stacked))
}

fn positions(g: &Geometry) -> Vec<Vec3> {
    g.0.iter().map(|v| Vec3::from(v.position)).collect()
}

/// The panels only: no ports (a sleeve hangs out, a hose runs down) and the door hanging.
fn panels_only(cover: &Cover) -> Cover {
    Cover { ports: Vec::new(), ..cover.clone() }
}

#[test]
fn the_cover_encloses_the_racks_frame_with_a_small_clearance() {
    let (e, frame) = shipped();
    // The frame drawn (`plant_layout::shelf_rack_geometry`): its posts and its decks.
    let (fv, _) = crate::engine::plant_layout::shelf_rack_geometry(frame.size, frame.shelves.len() as u32);
    let (flo, fhi) = fv.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), v| {
        let p = Vec3::from(v.position);
        (lo.min(p), hi.max(p))
    });
    // How far a point is outside the frame's box (positive) or inside it (negative), by the
    // box's sides and top (the cover has no floor).
    let outside = |p: Vec3| (p.x - fhi.x).max(flo.x - p.x).max(p.z - fhi.z).max(flo.z - p.z).max(p.y - fhi.y);
    for rolled in [0.0, 1.0] {
        let g = cover_geometry(e.size, &frame, &e.cover, rolled);
        let film = positions(&g.film);
        assert!(!film.is_empty(), "no film");
        for p in &film {
            assert!(outside(*p) >= 0.02, "film vertex {:?} is {:.3} m from the rack's frame (at least 0.02)", p.to_array(), outside(*p));
        }
        // It covers the frame on every side and over the top...
        let (lo, hi) = film.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
        assert!(lo.x < flo.x && hi.x > fhi.x && lo.z < flo.z && hi.z > fhi.z && hi.y > fhi.y, "the film {lo:?}..{hi:?} does not cover the frame {flo:?}..{fhi:?}");
        assert!(lo.y <= 0.01, "the film stops {:.3} m above the floor", lo.y);
    }
    // ...and fits it: the panels (ports aside, the door hanging) stay within 12 cm of it.
    let g = cover_geometry(e.size, &frame, &panels_only(&e.cover), 0.0);
    for p in positions(&g.film) {
        assert!(outside(p) <= 0.12, "film vertex {:?} stands {:.3} m off the frame (a fitted cover: at most 0.12)", p.to_array(), outside(p));
    }
}

#[test]
fn the_panels_sag_between_the_frames_supports() {
    let (e, frame) = shipped();
    let g = cover_geometry(e.size, &frame, &panels_only(&e.cover), 0.0);
    let (x, h) = (e.size.0 * 0.5, e.size.1);
    // The back panel (its plane at z = -depth/2): at its corner seams and its hem it lies on
    // its plane; between two shelves in its middle it bows in by about the cover's sag.
    let back: Vec<Vec3> = positions(&g.film).into_iter().filter(|p| p.z < -e.size.2 * 0.5 + 0.03 && p.x.abs() < x - 1e-3).collect();
    let mid = frame.shelves.windows(2).map(|w| 0.5 * (w[0] + w[1])).nth(1).expect("two shelves");
    let bow = back
        .iter()
        .filter(|p| p.x.abs() < 0.08 && (p.y - mid).abs() < 0.04)
        .map(|p| p.z + e.size.2 * 0.5)
        .fold(0.0_f32, f32::max);
    assert!(bow > e.cover.sag_m * 0.5, "the back panel bows in only {bow:.4} m between shelves (sag {})", e.cover.sag_m);
    for p in back.iter().filter(|p| p.y < 1e-4 || (p.y - h).abs() < 1e-4) {
        assert!((p.z + e.size.2 * 0.5).abs() < 1e-3, "the back panel's edge {:?} is off its seam line", p.to_array());
    }
    // The fold lines whiten the film (uv.x, read by the type 27 shader) by the cover's
    // crease_white at their middle; no other part carries any.
    let white = g.film.0.iter().map(|v| v.uv[0]).fold(0.0_f32, f32::max);
    assert!((white - e.cover.crease_white).abs() < 1e-4, "the folds whiten the film by {white} (crease_white {})", e.cover.crease_white);
    assert!(g.seams.0.iter().chain(&g.fittings.0).all(|v| v.uv[0] == 0.0), "a seam or a fitting carries a fold's whitening");
}

#[test]
fn rolling_the_door_up_shortens_it() {
    let (e, frame) = shipped();
    let cover = panels_only(&e.cover);
    let (x, h, z) = (e.size.0 * 0.5, e.size.1, e.size.2 * 0.5);
    let inset = cover.door.zip_inset_m;
    // The door: the front face's film between its zips.
    let door_foot = |rolled: f32| {
        let g = cover_geometry(e.size, &frame, &cover, rolled);
        positions(&g.film).into_iter().filter(|p| p.z > z - 0.04 && p.x.abs() < x - inset - 1e-3).map(|p| p.y).fold(f32::MAX, f32::min)
    };
    let (down, half, up) = (door_foot(0.0), door_foot(0.5), door_foot(1.0));
    assert!(down < 0.01, "hanging down, the door stops {down:.3} m above the floor");
    assert!(up > h * 0.85, "rolled up, the door still reaches {up:.3} m from the floor");
    assert!(half > down + 0.5 && half < up - 0.5, "half rolled, its foot is at {half:.3} m (down {down:.3}, up {up:.3})");
    // Rolled, its film is wound into a roll in front of the door (the seams' layers); hanging, none is.
    let in_front = |rolled: f32| positions(&cover_geometry(e.size, &frame, &cover, rolled).seams).iter().filter(|p| p.z > z + 0.01).count();
    assert_eq!(in_front(0.0), 0, "a roll in front of a door hanging down");
    assert!(in_front(1.0) > 0, "no roll in front of the rolled-up door");
}

#[test]
fn the_film_is_frosted_plastic_not_the_glass_type() {
    let (e, _) = shipped();
    let m = film_material(&e.cover.film);
    assert!(!(3.5..4.5).contains(&m.material_type), "material type {} is the glass type", m.material_type);
    assert_eq!(m.material_type, MATERIAL_TYPE_FILM);
    assert!(m.roughness >= 0.4, "roughness {} mirrors the room (frosted film is matte)", m.roughness);
    assert_eq!(m.metallic, 0.0);
    assert!(m.base_color[3] > 0.2 && m.base_color[3] < 0.9, "opacity {} is not a translucent film's", m.base_color[3]);
    assert!(m.params2[0] > 0.0, "the film passes on no light from its far side");
    // Milky, not tinted: near white and neutral.
    let [r, g, b, _] = m.base_color;
    assert!(r.min(g).min(b) >= 0.7 && r.max(g).max(b) - r.min(g).min(b) <= 0.08, "colour {:?} is tinted", [r, g, b]);
    // The seams are the same film, doubled; the fittings are opaque and transmit nothing.
    let s = seam_material(&e.cover);
    assert!(s.base_color[3] > m.base_color[3] && s.material_type == m.material_type);
    let f = fittings_material(&e.cover);
    assert_eq!((f.base_color[3], f.params2[0]), (1.0, 0.0));
}

#[test]
fn every_part_is_a_clean_mesh_down_and_rolled() {
    let (e, frame) = shipped();
    assert!(!e.cover.ports.is_empty(), "the shipped cover has its ports");
    for rolled in [0.0, 0.37, 1.0] {
        let g = cover_geometry(e.size, &frame, &e.cover, rolled);
        for (name, (v, ix)) in [("film", &g.film), ("seams", &g.seams), ("fittings", &g.fittings)] {
            assert!(!ix.is_empty() && ix.len() % 3 == 0, "{name}: {} indices", ix.len());
            assert!(ix.iter().all(|&i| (i as usize) < v.len()), "{name}: an index past its vertices");
            for vt in v {
                let n = Vec3::from(vt.normal);
                assert!(Vec3::from(vt.position).is_finite() && (n.length() - 1.0).abs() < 1e-3, "{name}: vertex {vt:?}");
            }
        }
    }
}

#[test]
fn the_door_rolls_while_its_rack_is_tended() {
    // Eased both ends, the whole way in roll_s, part of the way in part of the time.
    assert_eq!(roll_at(0.0, 1.0, 0.0, 2.0), 0.0);
    assert!((roll_at(0.0, 1.0, 1.0, 2.0) - 0.5).abs() < 1e-6);
    assert_eq!(roll_at(0.0, 1.0, 2.0, 2.0), 1.0);
    assert_eq!(roll_at(0.5, 0.0, 1.0, 2.0), 0.0);
    assert_eq!(roll_at(1.0, 1.0, 0.0, 2.0), 1.0);
    // Tended: this rack's card pinned and standing at it, on its storey.
    let foot = Vec3::new(2.0, 0.0, 53.0);
    let eye = Vec3::new(2.5, 1.6, 54.2);
    assert!(tended(Some("mush_0_0"), "mush_0_0", eye, foot, 2.0));
    assert!(!tended(Some("mush_0_1"), "mush_0_0", eye, foot, 2.0), "another rack's card");
    assert!(!tended(None, "mush_0_0", eye, foot, 2.0), "no card pinned");
    assert!(!tended(Some("mush_0_0"), "mush_0_0", eye + Vec3::new(3.0, 0.0, 0.0), foot, 2.0), "walked away");
    assert!(!tended(Some("mush_0_0"), "mush_0_0", eye + Vec3::new(0.0, 3.0, 0.0), foot, 2.0), "the storey above");
    assert_eq!((roll_step(0.0), roll_step(0.5), roll_step(1.0), roll_step(2.0)), (0, 32, 64, 64));
}
