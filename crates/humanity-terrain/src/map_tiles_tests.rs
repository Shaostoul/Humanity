//! The tile key's unit tests (whole-earth-maps.md, "Rung 1 in detail"). Each was seen red against
//! a deliberate mutation of `map_tiles.rs` on 2026-11-09; the mutation and what it printed are in
//! the test's comment.

use super::*;
use crate::drawn_surface::drawn_leaf_containing;
use std::collections::HashSet;

/// The plan's levels (whole-earth-maps.md 3.1). Data in rung 2 (`data/maps/levels.ron`); a fixture here.
const LEVELS: [u8; 4] = [3, 6, 9, 12];

/// Silverdale's Dyes Inlet waterfront, the first local pack's home (whole-earth-maps.md 12).
const SILVERDALE: (f64, f64) = (47.6445, -122.6950);

fn bits(v: DVec3) -> [u64; 3] {
    [v.x.to_bits(), v.y.to_bits(), v.z.to_bits()]
}

/// A deterministic spread of directions: a global latitude/longitude grid (poles included), a
/// fine grid around Silverdale, and points on and beside the root faces' edges and vertices,
/// where the descent's ties live.
fn grid_points() -> Vec<DVec3> {
    let mut pts = Vec::new();
    let mut lat = -90.0;
    while lat <= 90.0 {
        let mut lon = -180.0;
        while lon < 180.0 {
            pts.push(latlon_to_dir_f64(lat, lon));
            lon += 9.5;
        }
        lat += 7.25;
    }
    for i in 0..21 {
        for j in 0..21 {
            pts.push(latlon_to_dir_f64(SILVERDALE.0 - 0.01 + 0.001 * i as f64, SILVERDALE.1 - 0.015 + 0.0015 * j as f64));
        }
    }
    for c in root_face_corners() {
        for k in 0..3 {
            let (a, b) = (c[k], c[(k + 1) % 3]);
            pts.push(a);
            for t in [0.25, 0.5, 0.731] {
                let on = (a * (1.0 - t) + b * t).normalize();
                pts.push(on);
                pts.push((on + (c[(k + 2) % 3] - on) * 1e-9).normalize());
            }
        }
    }
    pts
}

/// The plan's rule, `clamp(3 * floor(d / 3), 3, 12)`, from the levels as data; and the tiles a
/// shallow patch reads. Seen red 2026-11-09 with `filter(|l| *l < depth)` (strictly shallower):
/// "depth 6 reads level 6 by the plan; left: Some(3), right: Some(6)".
#[test]
fn levels_follow_the_plan() {
    for d in 0u8..=24 {
        let want = (3 * (d / 3)).clamp(3, 12);
        assert_eq!(level_for_depth(&LEVELS, d), Some(want), "depth {d} reads level {want} by the plan");
    }
    assert_eq!(level_for_depth(&[], 5), None);
    let shallow = PatchId::root(7).child(2);
    let tiles = tiles_for_patch(&LEVELS, shallow);
    assert_eq!(tiles.len(), 16, "a depth-1 patch reads the 4^2 level-3 tiles under it");
    assert!(tiles.iter().all(|t| t.depth == 3 && shallow.is_ancestor_of(t)));
    let deep = tile_containing(SILVERDALE.0, SILVERDALE.1, 17);
    assert_eq!(tiles_for_patch(&LEVELS, deep), vec![ancestor_at(deep, 12).unwrap()]);
    assert_eq!(ancestor_at(deep, 18), None, "no ancestor deeper than the patch");
    assert_eq!(ancestor_at(deep, 17), Some(deep));
}

/// Containment and the ancestor mask agree with the terrain's own descent
/// (`drawn_leaf_containing`) on a grid of points: the tile found for a point at every depth is the
/// one the terrain's descent reaches, its ancestors are the tiles found at the shallower depths,
/// and the tile key of the patch DRAWN under a point (a mixed-depth drawn set) is the tile that
/// contains the point. Seen red 2026-11-09 with the child chosen by `min_by` instead of `max_by`:
/// "level 1 at (-0.00000, -1.00000, 0.00000): the terrain's descent does not reach
/// PatchId { face: 11, depth: 1, path: 1 }; left: None".
#[test]
fn ancestor_and_containment_agree_with_drawn_leaf_containing() {
    let depths = [0u8, 1, 3, 6, 9, 12, 16, 20];
    let pts = grid_points();
    for &p in &pts {
        let tiles: Vec<PatchId> = depths.iter().map(|&l| tile_containing_dir(p, l)).collect();
        for (i, &t) in tiles.iter().enumerate() {
            let one: HashSet<PatchId> = [t].into_iter().collect();
            assert_eq!(
                drawn_leaf_containing(&one, p),
                Some(t),
                "level {} at ({:.5}, {:.5}, {:.5}): the terrain's descent does not reach {t:?}",
                depths[i],
                p.x,
                p.y,
                p.z
            );
            for (j, &shallower) in tiles.iter().enumerate().take(i) {
                assert_eq!(ancestor_at(t, depths[j]), Some(shallower), "the depth-{} ancestor of {t:?} by mask", depths[j]);
            }
        }
    }
    // A drawn set of mixed depth: every depth-4 patch, with Silverdale's refined to depth 8.
    let home4 = tile_containing(SILVERDALE.0, SILVERDALE.1, 4);
    let mut drawn: HashSet<PatchId> = (0..20u8).flat_map(|f| descendants_at(PatchId::root(f), 4)).collect();
    drawn.remove(&home4);
    drawn.extend(descendants_at(home4, 8));
    for &p in &pts {
        let leaf = drawn_leaf_containing(&drawn, p).expect("the drawn set covers the sphere");
        let level = level_for_depth(&LEVELS, leaf.depth).unwrap();
        assert_eq!(tiles_for_patch(&LEVELS, leaf), vec![tile_containing_dir(p, level)], "the tile key of the leaf drawn under the point {leaf:?}");
    }
}

/// The nested lattice IS the descendants' corners (whole-earth-maps.md 4.2): a 64-segment lattice
/// at level L holds exactly the corners `patch_corners` gives the depth L+6 patches, bit for bit,
/// and its every 4th sample is the 16-segment lattice of the tile itself (a depth-L patch reads
/// its own vertices out of it). Seen red 2026-11-09 with `tile_lattice` filling vertices by the
/// mesher's normalized barycentric blend, `normalize(c0*(n-r) + c1*(r-c) + c2*c)`: "level 12:
/// 12276 of 12288 descendant corners differ from the lattice" (only the tile's own three corners,
/// met four times each, agree: the plan's point that today's patch vertices are not nested).
#[test]
fn lattice_is_the_depth_plus_six_corners_bit_for_bit() {
    let tiles = [tile_containing(SILVERDALE.0, SILVERDALE.1, 12), tile_containing(-33.9, 151.2, 3), tile_containing(51.13, 1.36, 9)];
    for tile in tiles {
        let lattice = tile_lattice(&tile, 64).expect("64 is a power of two");
        assert_eq!(lattice.len(), lattice_len(64));
        assert!(lattice.iter().all(|v| (v.length() - 1.0).abs() < 1e-12), "every sample written, unit length");
        // Walk the 4096 depth L+6 descendants with their corners' lattice coordinates.
        let mut stack = vec![(tile, [(0u32, 0u32), (64, 0), (64, 64)])];
        let (mut checked, mut differ) = (0, 0);
        while let Some((id, rc)) = stack.pop() {
            if id.depth == tile.depth + 6 {
                for (corner, (r, c)) in patch_corners(&id).into_iter().zip(rc) {
                    checked += 1;
                    if bits(corner) != bits(lattice[lattice_index(r, c)]) {
                        differ += 1;
                    }
                }
                continue;
            }
            let mid = |a: (u32, u32), b: (u32, u32)| ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
            let (m01, m12, m20) = (mid(rc[0], rc[1]), mid(rc[1], rc[2]), mid(rc[2], rc[0]));
            let kids = [[rc[0], m01, m20], [rc[1], m12, m01], [rc[2], m20, m12], [m01, m12, m20]];
            for (i, krc) in kids.into_iter().enumerate() {
                stack.push((id.child(i as u32), krc));
            }
        }
        assert_eq!(checked, 3 * 4096);
        assert_eq!(differ, 0, "level {}: {differ} of {checked} descendant corners differ from the lattice", tile.depth);
        let coarse = tile_lattice(&tile, 16).unwrap();
        for r in 0..=16u32 {
            for c in 0..=r {
                assert_eq!(bits(coarse[lattice_index(r, c)]), bits(lattice[lattice_index(4 * r, 4 * c)]), "every 4th sample is the coarse lattice");
            }
        }
    }
    assert!(tile_lattice(&PatchId::root(0), 48).is_none(), "48 is not a power of two");
}

/// The tile across edge `k` of `t`, found by a step just outside the edge's midpoint, and checked
/// to share that edge's two corners exactly.
fn neighbour(t: &PatchId, k: usize) -> PatchId {
    let c = patch_corners(t);
    let (a, b) = (c[k], c[(k + 1) % 3]);
    let mid = (a + b).normalize();
    let inward = a.cross(b).normalize();
    let n = tile_containing_dir(mid - inward * 1e-7, t.depth);
    let nc: Vec<[u64; 3]> = patch_corners(&n).into_iter().map(bits).collect();
    assert!(nc.contains(&bits(a)) && nc.contains(&bits(b)), "{n:?} shares edge {k} of {t:?}");
    n
}

/// Every tile a road's arc may pass through at `depth`: those found by sampling the arc densely,
/// and their neighbours (so a corner the arc only clips is never missed; a tile it misses clips to
/// nothing).
fn tiles_along(line: &[DVec3], depth: u8) -> Vec<PatchId> {
    let mut out: Vec<PatchId> = Vec::new();
    for w in line.windows(2) {
        for i in 0..=4000 {
            let t = tile_containing_dir(w[0].lerp(w[1], i as f64 / 4000.0), depth);
            if !out.contains(&t) {
                out.push(t);
            }
        }
    }
    for t in out.clone() {
        for k in 0..3 {
            let n = neighbour(&t, k);
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out
}

/// The points where a tile's pieces of a road were cut by its border.
fn border_points(pieces: &[ClippedLine]) -> Vec<[u64; 3]> {
    let mut v = Vec::new();
    for p in pieces {
        if p.start_clipped {
            v.push(bits(p.points[0]));
        }
        if p.end_clipped {
            v.push(bits(*p.points.last().unwrap()));
        }
    }
    v
}

/// A road crossing a tile border clips to the same point from both sides, bit for bit, whichever
/// way it is travelled, including roads that cross two or three tiles (the crossing is computed
/// from the ORIGINAL road edge, never from a piece already clipped). Tiles at level 12 by
/// Silverdale and at level 9 across a root face's edge, each against its three neighbours. Seen
/// red 2026-11-09 with the entering vertex re-based on the clipped piece
/// (`out: Along::Feature(x, w.p)` in `clip_polyline`): "a border point of PatchId { face: 0,
/// depth: 12, path: 7967627 } is in no other tile: the tiles disagree on a crossing".
#[test]
fn a_road_clips_to_the_same_point_from_both_sides() {
    let home = tile_containing(SILVERDALE.0, SILVERDALE.1, 12);
    // A level-9 tile beside root face 0's first edge.
    let r0 = root_face_corners()[0];
    // 0.3 of the way along: not a vertex at any depth, so the tile there has an edge ON the root edge.
    let on_root_edge = (r0[0] * 0.7 + r0[1] * 0.3).normalize();
    let by_face_edge = tile_containing_dir((on_root_edge + (r0[2] - on_root_edge) * 1e-9).normalize(), 9);
    let mut pairs = Vec::new();
    for t in [home, by_face_edge] {
        for k in 0..3 {
            pairs.push((t, neighbour(&t, k)));
        }
    }
    assert!(pairs.iter().any(|(a, b)| a.face != b.face), "one pair straddles two root faces");
    let mut crossings = 0;
    for (a, b) in &pairs {
        let (ca, cb) = (patch_corners(a), patch_corners(b));
        let shared: Vec<DVec3> = cb.iter().copied().filter(|x| ca.iter().any(|y| bits(*y) == bits(*x))).collect();
        assert_eq!(shared.len(), 2);
        let fa = ca.into_iter().find(|x| !shared.iter().any(|y| bits(*y) == bits(*x))).unwrap();
        let fb = cb.into_iter().find(|x| !shared.iter().any(|y| bits(*y) == bits(*x))).unwrap();
        let m = (shared[0] + shared[1]).normalize();
        let starts = [(m * 0.7 + fa * 0.3).normalize(), (shared[0] * 0.3 + shared[1] * 0.2 + fa * 0.5).normalize(), (m * 0.95 + fa * 0.05).normalize()];
        let mut ends = vec![(m * 0.5 + fb * 0.5).normalize(), (shared[0] * 0.2 + shared[1] * 0.4 + fb * 0.4).normalize()];
        // Beyond b's far corner: the road leaves b again through one of its other edges.
        for side in [-0.15, -0.05, 0.05, 0.15] {
            ends.push((fb + (fb - m) * 0.3 + (shared[0] - shared[1]) * side).normalize());
        }
        for s in starts {
            for &e in &ends {
                for line in [[s, e], [e, s]] {
                    let tiles = tiles_along(&line, a.depth);
                    assert!(tiles.contains(a) && tiles.contains(b), "the road runs in both {a:?} and {b:?}");
                    let per_tile: Vec<(PatchId, Vec<[u64; 3]>)> = tiles.iter().map(|t| (*t, border_points(&clip_polyline(t, &line)))).collect();
                    for (t, pts) in &per_tile {
                        for p in pts {
                            assert!(
                                per_tile.iter().any(|(u, q)| u != t && q.contains(p)),
                                "a border point of {t:?} is in no other tile: the tiles disagree on a crossing"
                            );
                            crossings += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(crossings > 6 * 3 * 6 * 2, "{crossings} crossings checked");
}

/// A tile's centre direction.
fn centre(t: &PatchId) -> DVec3 {
    let c = patch_corners(t);
    (c[0] + c[1] + c[2]).normalize()
}

/// A concave 24-pointed star of `radius_rad` (alternate points at 60%) around `at`.
fn star(at: DVec3, radius_rad: f64) -> Vec<DVec3> {
    let east = DVec3::Y.cross(at).normalize();
    let north = at.cross(east);
    (0..24)
        .map(|i| {
            let a = i as f64 * std::f64::consts::TAU / 24.0 + 0.123;
            let r = if i % 2 == 0 { radius_rad } else { radius_rad * 0.6 };
            (at + (east * a.cos() + north * a.sin()) * r).normalize()
        })
        .collect()
}

/// A ring clipped into the four children of a tile and put back together keeps its area to a
/// relative 1e-9; so does one clipped into the six tiles around a subdivision vertex, where the
/// pieces' corners are tile corners. Measured green 2026-11-09: relative errors 5.2e-14 (four
/// children) and 2.4e-13 (around the vertex). Seen red the same day twice: with `shared_corner`
/// returning edge k's first corner whatever j was, "around a vertex: pieces sum to 4.07e-10 sr
/// against 2.05e-8, relative error 9.80e-1"; and with the antipode fix in `crossing` removed,
/// "four children: pieces sum to 5.99e0 sr against 1.64e-7".
#[test]
fn a_ring_clipped_into_tiles_keeps_its_area() {
    // Four children of Silverdale's depth-10 tile (edge about 6.9 km).
    let parent = tile_containing(SILVERDALE.0, SILVERDALE.1, 10);
    let pc = patch_corners(&parent);
    let o = centre(&parent);
    // The parent's inradius, as an angle: the centre's distance to the nearest edge plane.
    let inradius = (0..3).map(|k| o.dot(pc[k].cross(pc[(k + 1) % 3]).normalize()).asin()).fold(f64::MAX, f64::min);
    let ring = star(o, inradius * 0.85);
    let whole = ring_area_sr(&ring, o);
    assert!(whole > 0.0, "the star runs counter-clockwise from outside");
    let pieces: Vec<ClippedRing> = (0..4).filter_map(|i| clip_ring(&parent.child(i), &ring)).collect();
    assert_eq!(pieces.len(), 4, "the ring reaches all four children");
    assert!(pieces.iter().all(|p| p.clip_edge.iter().any(|f| *f) && p.clip_edge.iter().any(|f| !*f)), "each piece has real edges and clip edges");
    let sum: f64 = pieces.iter().map(|p| ring_area_sr(&p.points, o)).sum();
    let rel = ((sum - whole) / whole).abs();
    assert!(rel < 1e-9, "four children: pieces sum to {sum:e} sr against {whole:e}, relative error {rel:e}");

    // Six depth-11 tiles around the parent's first edge midpoint, a corner of each of them.
    let v = child_corners(&pc)[0][1];
    let mut around: Vec<PatchId> = Vec::new();
    for i in 0..36 {
        let east = DVec3::Y.cross(v).normalize();
        let north = v.cross(east);
        let a = i as f64 * std::f64::consts::TAU / 36.0;
        let t = tile_containing_dir(v + (east * a.cos() + north * a.sin()) * 1e-6, 11);
        if !around.contains(&t) {
            around.push(t);
        }
    }
    assert_eq!(around.len(), 6, "six depth-11 tiles meet at a subdivision vertex");
    let ring = star(v, inradius * 0.3);
    let whole = ring_area_sr(&ring, v);
    let pieces: Vec<ClippedRing> = around.iter().filter_map(|t| clip_ring(t, &ring)).collect();
    assert_eq!(pieces.len(), 6);
    let sum: f64 = pieces.iter().map(|p| ring_area_sr(&p.points, v)).sum();
    let rel = ((sum - whole) / whole).abs();
    assert!(rel < 1e-9, "around a vertex: pieces sum to {sum:e} sr against {whole:e}, relative error {rel:e}");
    assert!(pieces.iter().all(|p| p.points.iter().any(|q| bits(*q) == bits(v))), "every piece holds the shared corner exactly");
}

/// The 10^7-degree codec round-trips within 6 mm north and 6 mm east anywhere on the planet
/// (0.5e-7 degree is 5.56 mm on the 6,371 km sphere; the diagonal is then under 7.9 mm), and the
/// integers themselves round-trip exactly. Seen red 2026-11-09 with truncation, `(deg * E7) as i32`:
/// "east error 6.05 mm at (-52.67040694076985, -41.05522198969257)".
#[test]
fn codec_round_trips_within_6_mm() {
    const R: f64 = 6_371_000.0;
    let m_per_deg = R * std::f64::consts::PI / 180.0;
    let mut seed: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 11) as f64 / (1u64 << 53) as f64
    };
    let (mut worst_n, mut worst_e, mut worst_d) = (0.0f64, 0.0f64, 0.0f64);
    for i in 0..200_000 {
        let lat = if i == 0 { 89.999_999_95 } else { -90.0 + 180.0 * next() };
        let lon = if i == 1 { 179.999_999_95 } else { -180.0 + 360.0 * next() };
        let (la, lo) = (encode_e7(lat), encode_e7(lon));
        assert_eq!(encode_e7(decode_e7(la)), la, "the integer round-trips exactly");
        let n = (decode_e7(la) - lat).abs() * m_per_deg;
        let e = (decode_e7(lo) - lon).abs() * m_per_deg * lat.to_radians().cos();
        assert!(n <= 0.006, "north error {:.2} mm at ({lat}, {lon})", n * 1000.0);
        assert!(e <= 0.006, "east error {:.2} mm at ({lat}, {lon})", e * 1000.0);
        // The chord, not `angle_between`: acos near 1 is good to about 1e-8 rad, 90 mm.
        let d = (e7_to_dir(la, lo) - latlon_to_dir_f64(lat, lon)).length() * R;
        worst_n = worst_n.max(n);
        worst_e = worst_e.max(e);
        worst_d = worst_d.max(d);
    }
    assert!(worst_d < 0.0079, "diagonal error {:.2} mm", worst_d * 1000.0);
    assert!(worst_n > 0.005 && worst_e > 0.005, "the sample reached the worst case ({:.2}, {:.2} mm)", worst_n * 1000.0, worst_e * 1000.0);
    assert_eq!(encode_e7(180.0), 1_800_000_000, "180 degrees fits in 32 bits");
    assert_eq!(encode_e7(-180.0), -1_800_000_000);
}
