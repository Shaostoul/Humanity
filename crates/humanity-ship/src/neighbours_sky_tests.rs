//! A NEIGHBOUR'S OPEN YARD SEES THE SKY AS YOUR OWN DOES (2026-11-14).
//!
//! WHY: since the near sun view (src/renderer/local_shadows.rs, 2026-11-10) draws the homes near
//! you into the sun's shadow map, everything the ship's meshes put over a yard shades it. A
//! neighbour's home was baked with its GLASS roof drawn as the opaque concrete ceiling
//! (`HomeStructure::bake_shell_groups`: "a glass roof would cost the transparent pass's sort for
//! nothing seen from outside"), an up-facing quad nobody sees from below, so it was harmless until
//! the sun started casting from it: every neighbour's yard (and its upper rooms) sat in shade at
//! noon while your own yard, under the same glass, was lit. Seen in the game at the r1 yard on
//! 2026-11-14: a neighbour's yard floor read about 80 of 255 where your own read 199, and 174 to
//! 201 with the near sun view off (showcase `near_levels` "0").
//!
//! A SECOND COVER, found when the first fix did not change the picture (the same 82.6 codes in the
//! game with the roof made glass): the hull's top plating (crates/humanity-ship/src/hull.rs) is
//! cut open over every glass zone roof, your home's among them, but a neighbour's plot was wrapped
//! shut, so the plating over it shaded its yard too. Both covers had to go.
//!
//! WHAT this measures: the geometry the game draws (`ShipStructure::generate_meshes`, the home on
//! r1, and the hull's plating, drawn while Show hull is on, its default), routed the way the frame
//! routes it into the sun's casters (the opaque families cast in full; glass casts only partially,
//! through the transmittance atlas, and the opaque roof slot is not drawn while Settings' Show roof
//! is off, its default), and from a grid of points on your own yard and on a neighbour's yard of
//! the same design, rays toward the sun. Whatever fraction of your own yard sees the sun, the
//! neighbour's must see as much (the noon suns), and a low sun behind a house must still put its
//! yard in that house's shadow, for both (real shade stays).

use super::*;
use crate::ship_structure::ShipStructure;
use glam::Vec3;

fn data_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data")
}

/// The triangles that block the sun in full, as the frame draws them into the sun's casters
/// (src/lib.rs's shell list and engine/home_meshes.rs): the opaque floors (alpha 1; a glass slab is
/// a floor with alpha under 1, a partial caster), the opaque material walls (alpha 1; glass walls
/// ride the transparent list), the legacy walls, the trim and the mirrors. NOT the ceilings: the
/// glass roof slot is a partial caster when any roof is glass, and both roof slots are drawn only
/// while Show roof is on otherwise (off by default; `ceilings_opaque` likewise).
fn sun_blockers(m: &HomesteadMeshes) -> Vec<[Vec3; 3]> {
    let mut out = Vec::new();
    let mut take = |v: &[Vertex], i: &[u32]| {
        for t in i.chunks_exact(3) {
            out.push([Vec3::from(v[t[0] as usize].position), Vec3::from(v[t[1] as usize].position), Vec3::from(v[t[2] as usize].position)]);
        }
    };
    for (v, i, c, _) in &m.floors {
        if c[3] >= 0.999 {
            take(v, i);
        }
    }
    for (v, i, c) in &m.material_walls {
        if c[3] >= 0.999 {
            take(v, i);
        }
    }
    for (v, i) in [&m.walls, &m.trim, &m.mirrors] {
        take(v, i);
    }
    out
}

/// Moller-Trumbore, both faces (the shadow passes draw a quad whichever way it faces the sun):
/// whether the ray from `o` along unit `d` meets `t` within `max` metres.
fn hits(o: Vec3, d: Vec3, t: &[Vec3; 3], max: f32) -> bool {
    let e1 = t[1] - t[0];
    let e2 = t[2] - t[0];
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 {
        return false;
    }
    let inv = 1.0 / det;
    let s = o - t[0];
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    let dist = e2.dot(q) * inv;
    dist > 1e-4 && dist < max
}

/// The yard of a row house on the plot whose corner is `o` (ship metres): the design's back
/// garden, x 10 to 18 m from the street (the house is x 2 to 10, data/homes/shipped/
/// row_house_two_bedroom.ron) and z 0 to 5.5 m along it, as a grid of points 5 cm above its floor,
/// kept clear of its edges and of the beds and the washing line by staying inside x 11 to 17 and
/// z 0.6 to 4.9.
fn yard_points(o: Vec3) -> Vec<Vec3> {
    let mut out = Vec::new();
    for ix in 0..7 {
        for iz in 0..5 {
            out.push(o + Vec3::new(11.0 + ix as f32, 0.05, 0.6 + iz as f32 * 1.075));
        }
    }
    out
}

/// The fraction of `points` from which a ray toward the sun (`to_sun`) meets none of `tris`
/// within 40 m. The triangles are first cut to those that can stand in the way (inside the
/// points' box grown by 40 m along the sun's horizontal, above the points).
fn sunlit_fraction(points: &[Vec3], to_sun: Vec3, tris: &[[Vec3; 3]]) -> f32 {
    let d = to_sun.normalize();
    let reach = 40.0;
    let lo = points.iter().fold(Vec3::splat(f32::MAX), |a, p| a.min(*p));
    let hi = points.iter().fold(Vec3::splat(f32::MIN), |a, p| a.max(*p));
    let far = d * reach;
    let (blo, bhi) = (lo.min(lo + far) - Vec3::splat(0.5), hi.max(hi + far) + Vec3::splat(0.5));
    let near: Vec<&[Vec3; 3]> = tris
        .iter()
        .filter(|t| {
            let tlo = t[0].min(t[1]).min(t[2]);
            let thi = t[0].max(t[1]).max(t[2]);
            tlo.x <= bhi.x && thi.x >= blo.x && tlo.y <= bhi.y && thi.y >= blo.y && tlo.z <= bhi.z && thi.z >= blo.z
        })
        .collect();
    let lit = points.iter().filter(|p| !near.iter().any(|t| hits(**p, d, t, reach))).count();
    lit as f32 / points.len() as f32
}

/// THE CHECK. Your home on r1 (a two-bedroom row house), its neighbour r2 the same design: from
/// every point of each yard, rays toward five noon suns (straight up and 17 degrees off it each
/// way) and one low sun from the street's side (20 degrees up, behind the houses). At noon the
/// neighbour's yard sees the sun from at least as much of it as your own (within one point in
/// twenty); at the low sun both yards lie in their houses' shadow (less than half lit) and within
/// one point in ten of each other, so the fix took away only what was never there.
///
/// Seen red 2026-11-14 on main as it stood (73b817e7a, the neighbour's glass roof baked opaque):
/// "at the noon sun (0.00, 1.00, 0.00) the neighbour r2's yard sees the sun from 0% of its points,
/// your own r1's from 100%"; and the same words again with the roof made glass but the hull's
/// plating left shut over the neighbours (the hull hole held back), so each cover alone fails it.
#[test]
fn a_neighbours_open_yard_sees_the_sky_as_your_own_does() {
    let ship = ShipStructure::load_and_assemble_shipped(&data_dir(), Some("r1")).expect("the shipped ship assembles with the home on r1");
    let corner = |id: &str| {
        let p = ship.plots.iter().find(|p| p.id == id).unwrap_or_else(|| panic!("no plot {id} in the ship file"));
        Vec3::new(p.origin.0, p.origin.1, p.origin.2)
    };
    let view = neighbour_view(&ship);
    let r2 = view.on_plot("r2").expect("r2 is drawn as a neighbour's");
    assert_eq!(r2.design.kind, "row_house_two_bedroom", "r2 is the same design as the home on r1");
    let m = ship.generate_meshes();
    let mut tris = sun_blockers(&m);
    // The hull's plating: drawn in the opaque scene list while Show hull is on (its default), so
    // the near sun view casts from it like any wall; its top is open only over glass roofs.
    let profile = crate::hull::HullProfile::load(&data_dir()).expect("the hull profile loads");
    let hull = crate::hull::generate_hull(&ship, &profile).plating;
    for t in hull.1.chunks_exact(3) {
        tris.push([Vec3::from(hull.0[t[0] as usize].position), Vec3::from(hull.0[t[1] as usize].position), Vec3::from(hull.0[t[2] as usize].position)]);
    }
    let own = yard_points(corner("r1"));
    let theirs = yard_points(corner("r2"));
    let noon = [Vec3::Y, Vec3::new(0.3, 1.0, 0.0), Vec3::new(-0.3, 1.0, 0.0), Vec3::new(0.0, 1.0, 0.3), Vec3::new(0.0, 1.0, -0.3)];
    for sun in noon {
        let (a, b) = (sunlit_fraction(&own, sun, &tris), sunlit_fraction(&theirs, sun, &tris));
        let s = sun.normalize();
        // Straight overhead nothing of your own covers your yard; a sun 17 degrees toward the street
        // lets the house's back wall shade the strip of yard beside it (5 of the 35 points), real
        // shade that the neighbour's yard must show alike.
        let floor = if sun == Vec3::Y { 0.99 } else { 0.8 };
        assert!(a >= floor, "at the noon sun ({:.2}, {:.2}, {:.2}) your own r1's yard sees the sun from only {:.0}% of its points: the check is not looking at an open yard", s.x, s.y, s.z, a * 100.0);
        assert!(
            b >= a - 0.05,
            "at the noon sun ({:.2}, {:.2}, {:.2}) the neighbour r2's yard sees the sun from {:.0}% of its points, your own r1's from {:.0}%",
            s.x,
            s.y,
            s.z,
            b * 100.0,
            a * 100.0
        );
    }
    // The low sun from the street's side (west, -x): each house stands between it and its yard.
    let low = Vec3::new(-1.0, 0.364, 0.0);
    let (a, b) = (sunlit_fraction(&own, low, &tris), sunlit_fraction(&theirs, low, &tris));
    assert!(a < 0.5 && b < 0.5, "at a low sun behind the houses both yards lie in their house's shadow: your own r1's is {:.0}% lit, the neighbour r2's {:.0}%", a * 100.0, b * 100.0);
    assert!((a - b).abs() <= 0.1, "at a low sun the two yards are shaded alike: your own r1's is {:.0}% lit, the neighbour r2's {:.0}%", a * 100.0, b * 100.0);
}
