use glam::Vec3;

/// Angular radius of a light gizmo ring as seen from the camera (moved with the ring math).
pub(crate) const LIGHT_RING_ANGULAR_RADIUS: f32 = 0.05;

/// Snap a dragged corner: to the nearest OTHER corner within 0.6 m (a shared node / airtight
/// seal), else to the box perimeter if near an edge, else to the 0.25 m grid when grid snap is
/// on. Always clamped into the box footprint. (v0.541)
pub(crate) fn snap_node_position(
    hs: &crate::ship::home_structure::HomeStructure,
    grabbed: (f32, f32),
    raw: (f32, f32),
    grid: bool,
) -> (f32, f32) {
    // 1. Endpoint snap (strongest): another corner within 0.6 m, for shared nodes + seals.
    let mut best: Option<((f32, f32), f32)> = None;
    for wall in &hs.walls {
        for c in [wall.a, wall.b] {
            if (c.0 - grabbed.0).abs() < 0.05 && (c.1 - grabbed.1).abs() < 0.05 {
                continue; // the grabbed node (+ its shared copies)
            }
            let dd = (c.0 - raw.0).powi(2) + (c.1 - raw.1).powi(2);
            if dd < 0.36 && best.map_or(true, |(_, b)| dd < b) {
                best = Some((c, dd));
            }
        }
    }
    if let Some((c, _)) = best {
        // Snap onto the existing corner, quantized to the corner grid so the two become
        // BYTE-IDENTICAL (one orb, one draggable group; no overlapping-but-distinct duplicate).
        return crate::ship::home_structure::quantize_corner(c);
    }
    // 2. Grid snap, then edge snap to the box perimeter.
    let (w, d) = (hs.width, hs.depth);
    let mut x = raw.0;
    let mut z = raw.1;
    if grid {
        x = (x * 4.0).round() / 4.0;
        z = (z * 4.0).round() / 4.0;
    }
    if x < 0.5 {
        x = 0.0;
    } else if x > w - 0.5 {
        x = w;
    }
    if z < 0.5 {
        z = 0.0;
    } else if z > d - 0.5 {
        z = d;
    }
    crate::ship::home_structure::quantize_corner((x.clamp(0.0, w), z.clamp(0.0, d)))
}

/// Snap (x,z) to the nearest other object's X and/or Z within `tol` metres (independent per axis),
/// so a dragged object lines up with existing ones. Returns the snapped coords + the guide coord per
/// axis (Some when that axis snapped, for drawing the guide line). Pure -> unit-tested. (v0.613)
pub(crate) fn snap_to_alignment(x: f32, z: f32, others: &[(f32, f32)], tol: f32) -> (f32, f32, Option<f32>, Option<f32>) {
    let (mut sx, mut gx, mut bestx) = (x, None, tol);
    let (mut sz, mut gz, mut bestz) = (z, None, tol);
    for &(ox, oz) in others {
        let dx = (x - ox).abs();
        if dx < bestx {
            bestx = dx;
            sx = ox;
            gx = Some(ox);
        }
        let dz = (z - oz).abs();
        if dz < bestz {
            bestz = dz;
            sz = oz;
            gz = Some(oz);
        }
    }
    (sx, sz, gx, gz)
}

#[cfg(test)]
mod snap_align_tests {
    use super::snap_to_alignment;

    #[test]
    fn snaps_each_axis_to_the_nearest_neighbour_within_tolerance() {
        let others = [(5.0_f32, 9.0_f32), (5.2, 20.0)];
        // x=5.05 is within 0.3 of 5.0 -> snaps to 5.0; z=8.9 within 0.3 of 9.0 -> snaps to 9.0.
        let (sx, sz, gx, gz) = snap_to_alignment(5.05, 8.9, &others, 0.3);
        assert!((sx - 5.0).abs() < 1e-6 && gx == Some(5.0), "x snapped to nearest");
        assert!((sz - 9.0).abs() < 1e-6 && gz == Some(9.0), "z snapped to nearest");
    }

    #[test]
    fn leaves_axes_unsnapped_when_no_neighbour_is_close() {
        let others = [(5.0_f32, 9.0_f32)];
        let (sx, sz, gx, gz) = snap_to_alignment(50.0, 50.0, &others, 0.3);
        assert!((sx - 50.0).abs() < 1e-6 && gx.is_none(), "far x is unchanged + no guide");
        assert!((sz - 50.0).abs() < 1e-6 && gz.is_none(), "far z is unchanged + no guide");
    }
}

/// The rotation-ring world radius for a light at `center` seen from `camera_pos` (v0.792).
/// ONE function shared by the draw block, the click pick, and the hover test, so the ring
/// you see is always exactly the ring you grab. Floored so a camera sitting ON the light
/// can't shrink the ring (and its pick tolerance) to nothing.
pub(crate) fn light_ring_radius(camera_pos: Vec3, center: Vec3) -> f32 {
    (camera_pos.distance(center) * LIGHT_RING_ANGULAR_RADIUS).max(0.05)
}

/// Pick tolerance for a rotation ring, proportional to its radius (v0.792): the rings are
/// viewport-fixed now, so a fixed-metre tolerance would be laughably fat up close and
/// ungrabbable from afar. ~a third of the ring radius keeps the ring's EMPTY middle
/// unclickable (center miss distance = the radius, see ray_ring_tests) while leaving a
/// comfortable grab band around the rim.
pub(crate) fn light_ring_pick_tolerance(ring_radius: f32) -> f32 {
    ring_radius * 0.35
}

/// Unit-sphere wireframe edges for the light RANGE display (v0.792, operator: "some other
/// way to represent the light's radius... maybe an icosphere subdivided twice"): an
/// icosphere at subdivision 2 (320 faces) reduced to its 480 unique undirected edges,
/// computed once and reused for every light. Per frame each edge is scaled by the light's
/// range and offset to its position -- no mesh, just lines, so it shows through walls like
/// the other build-mode helpers and costs nothing when no light is selected.
pub(crate) fn range_sphere_edges() -> &'static [(Vec3, Vec3)] {
    static EDGES: std::sync::OnceLock<Vec<(Vec3, Vec3)>> = std::sync::OnceLock::new();
    EDGES.get_or_init(|| {
        let mut ico = crate::terrain::icosphere::Icosphere::new();
        ico.subdivide_n(2);
        // Faces share edges; dedupe on the (min, max) index pair so each edge draws once.
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for f in &ico.faces {
            for (a, b) in [(f.v0, f.v1), (f.v1, f.v2), (f.v2, f.v0)] {
                let key = if a < b { (a, b) } else { (b, a) };
                if seen.insert(key) {
                    out.push((ico.vertices[a as usize], ico.vertices[b as usize]));
                }
            }
        }
        out
    })
}

#[cfg(test)]
mod light_gizmo_tests {
    use super::*;

    /// The rotation rings are VIEWPORT-FIXED (v0.792): world radius scales linearly with
    /// camera distance (same on-screen size everywhere), floored so a zero-distance camera
    /// can't collapse the ring, and the pick tolerance scales with the ring so the grab
    /// band feels identical at any distance.
    #[test]
    fn light_ring_radius_tracks_camera_distance() {
        let center = Vec3::new(5.0, 2.0, 5.0);
        let near = light_ring_radius(center + Vec3::X * 4.0, center);
        let far = light_ring_radius(center + Vec3::X * 40.0, center);
        assert!((near - 4.0 * LIGHT_RING_ANGULAR_RADIUS).abs() < 1e-5);
        assert!((far / near - 10.0).abs() < 1e-3, "10x the distance = 10x the world radius");
        // The floor: sitting on the light still yields a grabbable ring.
        assert!(light_ring_radius(center, center) >= 0.05);
        // Tolerance stays proportional (a third-ish of the rim, never the whole middle).
        assert!(light_ring_pick_tolerance(near) < near);
    }

    /// The range wireframe is an icosphere subdivided twice: 320 faces -> exactly 480
    /// unique edges (Euler: E = 3F/2), every endpoint on the unit sphere so scaling by
    /// the light's range puts the wire exactly AT the range.
    #[test]
    fn range_sphere_edges_are_unique_unit_edges() {
        let edges = range_sphere_edges();
        assert_eq!(edges.len(), 480, "level-2 icosphere has 480 unique edges");
        for (a, b) in edges {
            assert!((a.length() - 1.0).abs() < 1e-4, "endpoint on the unit sphere");
            assert!((b.length() - 1.0).abs() < 1e-4, "endpoint on the unit sphere");
            assert!(a.distance(*b) > 1e-4, "no degenerate zero-length edges");
        }
    }
}

/// Closest approach of a pick ray to a RING -- the circle of `radius` about `center` in the
/// plane perpendicular to unit `axis` (exactly what `push_circle_3d` draws). Returns
/// (ray t, distance to the ring) for the best candidate in front of the camera, or None when
/// everything lands behind it. Gizmo math, not a quartic solve: a face-on ring uses the
/// ray/plane intersection (measure how far the landing sits from the circle); an edge-on ring
/// (ray nearly parallel to the plane, where the plane hit explodes) projects the ray's closest
/// approach to the center out to the circle and measures the ray's distance to THAT point.
/// Both candidates are tried and the closer one wins. Pure -> unit tested. (v0.790)
pub(crate) fn ray_ring_closest(origin: Vec3, dir: Vec3, center: Vec3, axis: Vec3, radius: f32) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32)> = None;
    // Candidate 1: where the ray pierces the ring's plane.
    let denom = dir.dot(axis);
    if denom.abs() > 1e-6 {
        let t = (center - origin).dot(axis) / denom;
        if t > 0.0 {
            let p = origin + dir * t;
            let d = ((p - center).length() - radius).abs();
            best = Some((t, d));
        }
    }
    // Candidate 2: closest approach to the center, flattened into the plane and pushed out to
    // the circle. Covers the edge-on view where candidate 1 degenerates (denom -> 0 sends t
    // to infinity and the landing far off the visible ring).
    let tc = (center - origin).dot(dir);
    if tc > 0.0 {
        let v = (origin + dir * tc) - center;
        let v_in = v - axis * v.dot(axis); // flatten into the ring plane
        if v_in.length_squared() > 1e-8 {
            let ring_pt = center + v_in.normalize() * radius;
            let t = (ring_pt - origin).dot(dir);
            if t > 0.0 {
                let d = (ring_pt - (origin + dir * t)).length();
                if best.map_or(true, |(_, bd)| d < bd) {
                    best = Some((t, d));
                }
            }
        }
    }
    best
}

#[cfg(test)]
mod ray_ring_tests {
    use super::ray_ring_closest;
    use glam::Vec3;

    #[test]
    fn hits_a_face_on_ring_and_reports_the_radius_at_the_center() {
        // Ring: radius 2 about the origin in the XZ plane (axis Y). A ray straight down
        // through a point ON the circle hits at distance ~0.
        let (t, d) = ray_ring_closest(Vec3::new(2.0, 5.0, 0.0), Vec3::NEG_Y, Vec3::ZERO, Vec3::Y, 2.0)
            .expect("in front of the camera");
        assert!((t - 5.0).abs() < 1e-4, "plane hit at t=5, got {t}");
        assert!(d < 1e-4, "on the circle, got {d}");
        // A ray straight down the AXIS is `radius` away from the ring everywhere; the 0.4 m
        // pick tolerance rejects it, so clicking the middle never grabs a ring.
        let (_, d) = ray_ring_closest(Vec3::new(0.0, 5.0, 0.0), Vec3::NEG_Y, Vec3::ZERO, Vec3::Y, 2.0)
            .expect("still in front");
        assert!((d - 2.0).abs() < 1e-4, "center miss distance is the radius, got {d}");
    }

    #[test]
    fn hits_an_edge_on_ring_via_the_projection_fallback() {
        // Same ring seen edge-on: a horizontal ray at the ring's height grazing the rim at
        // x = 2. The plane candidate degenerates (dir lies IN the plane); the projection
        // fallback must still land within a hair of the rim.
        let (t, d) = ray_ring_closest(Vec3::new(2.0, 0.0, -10.0), Vec3::Z, Vec3::ZERO, Vec3::Y, 2.0)
            .expect("in front of the camera");
        assert!(d < 1e-3, "grazing ray touches the rim, got {d}");
        assert!((t - 10.0).abs() < 0.1, "tangency near z=0, got t={t}");
        // A ray passing 1 m outside the rim reads ~1 m away -- outside the pick tolerance.
        let (_, d) = ray_ring_closest(Vec3::new(3.0, 0.0, -10.0), Vec3::Z, Vec3::ZERO, Vec3::Y, 2.0)
            .expect("still in front");
        assert!(d > 0.9, "clearly a miss, got {d}");
    }
}

/// Ray vs axis-aligned box (the slab method): the ray `origin + t*dir` against the box [min,max].
/// Returns the nearest positive `t` of entry, or None if the ray misses / the box is behind. Used
/// to pick placed structures by their bounding box. (v0.583)
pub(crate) fn ray_aabb_hit(origin: Vec3, dir: Vec3, min: Vec3, max: Vec3) -> Option<f32> {
    let mut tmin = 0.0_f32;
    let mut tmax = f32::INFINITY;
    for a in 0..3 {
        let (o, d, lo, hi) = (origin[a], dir[a], min[a], max[a]);
        if d.abs() < 1e-8 {
            if o < lo || o > hi {
                return None; // parallel + outside the slab
            }
        } else {
            let inv = 1.0 / d;
            let mut t1 = (lo - o) * inv;
            let mut t2 = (hi - o) * inv;
            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }
            tmin = tmin.max(t1);
            tmax = tmax.min(t2);
            if tmin > tmax {
                return None;
            }
        }
    }
    if tmax < 0.0 {
        None
    } else {
        Some(tmin)
    }
}
