//! MAP TILES KEYED TO THE TERRAIN'S OWN QUADTREE (whole-Earth maps, rung 1, 2026-11-09;
//! docs/design/whole-earth-maps.md sections 3 and 4.2).
//!
//! The whole planet's map data (roads, water, names, buildings, heights, land cover) is cut into
//! tiles whose id IS a terrain [`PatchId`]: one of the 20 icosahedron faces, a depth and a path.
//! The tile a patch reads is its ancestor at a data level, found by masking the path bits, so the
//! map, the heights and the mesh share one grid, one level of detail and one set of streaming
//! decisions. This module is the key and nothing else: pure std + glam (and this crate's own
//! patch geometry), relay-safe, no I/O. What it holds:
//!
//! - **Levels.** Which data level serves a patch depth ([`level_for_depth`]); the list of levels
//!   is data (the plan's 3, 6, 9 and 12 go in `data/maps/levels.ron` with rung 2), so it is a
//!   parameter here, never a constant.
//! - **The tile key.** A patch's tile by mask ([`ancestor_at`], [`tiles_for_patch`]), and the tile
//!   containing a point ([`tile_containing_dir`], [`tile_containing`]) by the same firmest-child
//!   descent `drawn_surface::drawn_leaf_containing` does, held to it by a test.
//! - **The nested lattice** ([`tile_lattice`]): a tile's height-sample directions built by the
//!   tree's own normalized-midpoint subdivision, so a 64-segment lattice at level L is exactly the
//!   corners of the depth L+6 patches, bit for bit, and a coarse sample is exactly a fine one
//!   (section 4.2; rung 2 moves the mesher onto the same lattice).
//! - **The great-circle clipper** ([`clip_polyline`], [`clip_ring`]) with clip-edge flags. Every
//!   patch edge is a great-circle arc, so clipping is an exact intersection of great circles, and
//!   every crossing is computed from the ORIGINAL feature edge and the tile edge with their
//!   endpoints in a canonical order, so two neighbouring tiles compute the same crossing bit for
//!   bit and roads and coastlines meet across a tile border with no seam (section 3.2). Edges the
//!   clip created are flagged, so the shore distance field can ignore them (section 3.5).
//! - **The coordinate codec** ([`encode_e7`], [`decode_e7`], [`e7_to_dir`]): OSM's own integers,
//!   degrees times 10^7, decoded to f64 and to a unit direction in f64 (section 3.3).
//!
//! f64 THROUGHOUT (CLAUDE.md, "f32 at planet scale is a defect class"): every position and
//! direction here is a `DVec3` or an f64 angle; nothing narrows to f32.
//!
//! COST: none per frame yet (rung 2 calls the key on patch builds, the clipper only offline).

use crate::osm_region::latlon_to_dir_f64;
use crate::planet_chunks::{child_corners, patch_corners, root_face_corners, PatchId};
use glam::DVec3;
use std::cmp::Ordering;

// ── Levels and the tile key ──────────────────────────────────────────────────────────────────

/// The data level that serves a patch of `depth`: the deepest level not deeper than the patch, or
/// the shallowest level when the patch is shallower than all of them (it then reads several of
/// that level's tiles, [`tiles_for_patch`]). `levels` must be ascending; None when it is empty.
/// With the plan's levels 3, 6, 9, 12 this is `clamp(3 * floor(d / 3), 3, 12)`.
pub fn level_for_depth(levels: &[u8], depth: u8) -> Option<u8> {
    let first = *levels.first()?;
    Some(levels.iter().copied().filter(|l| *l <= depth).last().unwrap_or(first))
}

/// The ancestor of `id` at depth `level`, by masking its path (itself when `level` is its own
/// depth); None when `level` is deeper than the patch.
pub fn ancestor_at(id: PatchId, level: u8) -> Option<PatchId> {
    if level > id.depth {
        return None;
    }
    let mask = if level == 0 { 0 } else { (1u64 << (2 * level as u32)) - 1 };
    Some(PatchId { face: id.face, depth: level, path: id.path & mask })
}

/// Every descendant of `id` at depth `level`, in path order (`id` itself at its own depth); empty
/// when `level` is shallower than the patch.
pub fn descendants_at(id: PatchId, level: u8) -> Vec<PatchId> {
    if level < id.depth {
        return Vec::new();
    }
    let mut out = vec![id];
    for _ in id.depth..level {
        out = out.iter().flat_map(|p| (0..4).map(move |i| p.child(i))).collect();
    }
    out
}

/// The tiles a patch reads: its ancestor at its level, or, for a patch shallower than the
/// shallowest level, every tile of that level under it.
pub fn tiles_for_patch(levels: &[u8], id: PatchId) -> Vec<PatchId> {
    let Some(level) = level_for_depth(levels, id.depth) else {
        return Vec::new();
    };
    match ancestor_at(id, level) {
        Some(tile) => vec![tile],
        None => descendants_at(id, level),
    }
}

/// The least of the three raw barycentric weights of `d` in a spherical triangle: positive
/// inside. The SAME expression `drawn_leaf_containing` uses, so ties on an edge resolve alike.
fn firmness(c: &[DVec3; 3], d: DVec3) -> f64 {
    let b0 = d.dot(c[1].cross(c[2]));
    let b1 = d.dot(c[2].cross(c[0]));
    let b2 = d.dot(c[0].cross(c[1]));
    b0.min(b1).min(b2)
}

/// The patch at depth `level` containing the direction `dir` (any length; it is normalized): the
/// firmest root face, then the firmest child at every level, exactly as
/// `drawn_surface::drawn_leaf_containing` descends (a test holds the two together).
pub fn tile_containing_dir(dir: DVec3, level: u8) -> PatchId {
    let d = dir.normalize();
    let by_firmness = |a: &(usize, [DVec3; 3]), b: &(usize, [DVec3; 3])| firmness(&a.1, d).partial_cmp(&firmness(&b.1, d)).unwrap_or(Ordering::Equal);
    let (face, mut corners) = root_face_corners().iter().copied().enumerate().max_by(by_firmness).unwrap_or((0, root_face_corners()[0]));
    let mut id = PatchId::root(face as u8);
    while id.depth < level {
        let (ci, kid) = child_corners(&corners).into_iter().enumerate().max_by(by_firmness).unwrap_or((0, corners));
        corners = kid;
        id = id.child(ci as u32);
    }
    id
}

/// [`tile_containing_dir`] for a latitude and longitude in degrees (f64, through
/// `osm_region::latlon_to_dir_f64`, the region pipeline's one handedness convention).
pub fn tile_containing(lat_deg: f64, lon_deg: f64, level: u8) -> PatchId {
    tile_containing_dir(latlon_to_dir_f64(lat_deg, lon_deg), level)
}

// ── The nested lattice (section 4.2) ─────────────────────────────────────────────────────────

/// Samples in a triangular lattice of `segments` per edge: (n + 1)(n + 2) / 2.
pub fn lattice_len(segments: u32) -> usize {
    let n = segments as usize;
    (n + 1) * (n + 2) / 2
}

/// Index of lattice vertex (`r`, `c`), `c <= r <= segments`, in the mesher's own order
/// (`planet_chunks::build_patch_mesh`: row `r` from corner 0, column `c` toward corner 2; corner 0
/// is (0, 0), corner 1 is (n, 0), corner 2 is (n, n)), so rung 2 swaps one for the other.
pub fn lattice_index(r: u32, c: u32) -> usize {
    let r = r as usize;
    r * (r + 1) / 2 + c as usize
}

/// The sample directions of a tile's lattice of `segments` per edge (a power of two), built by the
/// tree's own normalized-midpoint subdivision from the tile's corners: vertex (r, c) is exactly
/// the corner `patch_corners` gives the depth `id.depth + log2(segments)` descendant there, bit for
/// bit, so every coarse sample is a fine one and a patch of any depth up to that reads its own
/// vertices out of the lattice with no interpolation. Indexed by [`lattice_index`]. None when
/// `segments` is not a power of two (or is over 2^10, a lattice of a million samples).
pub fn tile_lattice(id: &PatchId, segments: u32) -> Option<Vec<DVec3>> {
    lattice_from_corners(&patch_corners(id), segments)
}

/// [`tile_lattice`] for a triangle given by its corners (a patch's own, as the mesher holds
/// them): the one implementation of the nested lattice, which `planet_chunks::build_patch_mesh`
/// and `drawn_surface` build every patch's vertices from (rung 2, 2026-11-09).
pub fn lattice_from_corners(corners: &[DVec3; 3], segments: u32) -> Option<Vec<DVec3>> {
    if !segments.is_power_of_two() || segments > 1 << 10 {
        return None;
    }
    let levels = segments.trailing_zeros();
    let mut out = vec![DVec3::ZERO; lattice_len(segments)];
    fill_lattice(&mut out, *corners, [(0, 0), (segments, 0), (segments, segments)], levels);
    Some(out)
}

/// The lattice coordinates of a cell's four children, in `child_corners`' order (0 keeps
/// corner 0, 1 corner 1, 2 corner 2, 3 is the centre): what a descent through the nested
/// lattice tracks beside the corners themselves.
pub fn child_lattice_rc(rc: [(u32, u32); 3]) -> [[(u32, u32); 3]; 4] {
    let mid = |a: (u32, u32), b: (u32, u32)| ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
    let (m01, m12, m20) = (mid(rc[0], rc[1]), mid(rc[1], rc[2]), mid(rc[2], rc[0]));
    [[rc[0], m01, m20], [rc[1], m12, m01], [rc[2], m20, m12], [m01, m12, m20]]
}

/// Write `c` at its lattice coordinates `rc`, then the same for the four children, `levels` deep.
/// A vertex shared by several triangles is written once per triangle with the same bits: the
/// midpoint is commutative, so whichever edge reaches it computes the same value.
fn fill_lattice(out: &mut [DVec3], c: [DVec3; 3], rc: [(u32, u32); 3], levels: u32) {
    for (p, (r, col)) in c.iter().zip(rc) {
        out[lattice_index(r, col)] = *p;
    }
    if levels == 0 {
        return;
    }
    // The same child layout as `child_corners`: 0 keeps corner 0, 1 corner 1, 2 corner 2, 3 is the centre.
    for (kid, krc) in child_corners(&c).into_iter().zip(child_lattice_rc(rc)) {
        fill_lattice(out, kid, krc, levels - 1);
    }
}

// ── The great-circle clipper (sections 3.2 and 3.5) ──────────────────────────────────────────

/// Total order on directions by their components (x, then y, then z), so a pair of endpoints is
/// always taken in the same order whichever tile or travel direction asks.
fn canonical(a: DVec3, b: DVec3) -> (DVec3, DVec3) {
    let key = |v: DVec3| [v.x, v.y, v.z];
    let (ka, kb) = (key(a), key(b));
    let ord = ka[0].total_cmp(&kb[0]).then(ka[1].total_cmp(&kb[1])).then(ka[2].total_cmp(&kb[2]));
    if ord == Ordering::Greater {
        (b, a)
    } else {
        (a, b)
    }
}

/// Where the arc between `a` and `b` (a feature edge, as originally given) crosses the great
/// circle through the tile corners `e0` and `e1`. Both pairs are taken in canonical order, so the
/// arithmetic, and therefore every bit of the answer, is the same in the two tiles that share the
/// edge and for either direction of travel. (With this two-term formula the swapped orders would
/// agree anyway, because IEEE negation is exact: removing the sort left the border test green on
/// 2026-11-09. The sort keeps the guarantee from resting on the formula's shape.) What IS
/// load-bearing is computing from the ORIGINAL feature edge, never from a piece already clipped
/// (`Along::Feature`). Callers ask only when `a` and `b` lie on opposite sides.
fn crossing(a: DVec3, b: DVec3, e0: DVec3, e1: DVec3) -> DVec3 {
    let (p, q) = canonical(a, b);
    let (u, v) = canonical(e0, e1);
    let n = u.cross(v);
    let (dp, dq) = (n.dot(p), n.dot(q));
    // On the edge's plane (n . x = dp*dq - dq*dp = 0) and on the arc's great circle.
    let x = (q * dp - p * dq).normalize();
    if x.dot(p + q) < 0.0 {
        -x
    } else {
        x
    }
}

/// A tile as three closed half-spaces: a point is inside edge k (corners k and k+1) when its side
/// of the edge's canonical plane, times the edge's orientation, is not negative. Points exactly on
/// a shared edge belong to both tiles.
struct TileEdges {
    corners: [DVec3; 3],
    normal: [DVec3; 3],
    sign: [f64; 3],
}

impl TileEdges {
    fn of(id: &PatchId) -> Self {
        let corners = patch_corners(id);
        let mut normal = [DVec3::ZERO; 3];
        let mut sign = [1.0; 3];
        for k in 0..3 {
            let (a, b) = (corners[k], corners[(k + 1) % 3]);
            let (u, v) = canonical(a, b);
            normal[k] = u.cross(v);
            // Inward is cross(corner k, corner k+1) for the counter-clockwise-from-outside corners.
            sign[k] = if u == a { 1.0 } else { -1.0 };
        }
        TileEdges { corners, normal, sign }
    }

    fn inside(&self, k: usize, p: DVec3) -> bool {
        self.sign[k] * self.normal[k].dot(p) >= 0.0
    }

    /// The corner shared by edges `j` and `k` (j != k): exact, the corner itself.
    fn shared_corner(&self, j: usize, k: usize) -> DVec3 {
        // Edge k runs from corner k to corner k+1.
        let a = [k, (k + 1) % 3];
        let b = [j, (j + 1) % 3];
        let i = a.into_iter().find(|i| b.contains(i)).unwrap_or(k);
        self.corners[i]
    }
}

/// The great circle a working edge lies on: a feature edge (its two ORIGINAL endpoints, so every
/// later crossing on it is canonical) or tile edge k (an edge the clip created).
#[derive(Clone, Copy, Debug)]
enum Along {
    Feature(DVec3, DVec3),
    Border(usize),
}

/// A vertex and the edge that leaves it.
#[derive(Clone, Copy, Debug)]
struct Vtx {
    p: DVec3,
    out: Along,
}

impl TileEdges {
    /// Where the working edge leaving `v` crosses tile edge `k`.
    fn cross(&self, v: &Vtx, k: usize) -> DVec3 {
        match v.out {
            Along::Feature(a, b) => crossing(a, b, self.corners[k], self.corners[(k + 1) % 3]),
            Along::Border(j) => self.shared_corner(j, k),
        }
    }
}

/// One piece of a polyline inside a tile. `start_clipped` / `end_clipped` are true where the
/// piece begins or ends on the tile's border because the clip cut it there (not at the feature's
/// own end).
#[derive(Debug, Clone, PartialEq)]
pub struct ClippedLine {
    pub points: Vec<DVec3>,
    pub start_clipped: bool,
    pub end_clipped: bool,
}

/// A ring clipped to a tile. `clip_edge[i]` is true when the edge from `points[i]` to the next
/// point (cyclically) lies along the tile's border, made by the clip: not a real shoreline, so the
/// shore distance field skips it (section 3.5).
#[derive(Debug, Clone, PartialEq)]
pub struct ClippedRing {
    pub points: Vec<DVec3>,
    pub clip_edge: Vec<bool>,
}

/// Clip a polyline (unit directions; each segment the minor great-circle arc between its ends) to
/// the tile `id`. Returns the pieces inside, in order; a piece is at least two points.
pub fn clip_polyline(id: &PatchId, line: &[DVec3]) -> Vec<ClippedLine> {
    if line.len() < 2 {
        return Vec::new();
    }
    let t = TileEdges::of(id);
    // A piece: its vertices (the last one's `out` unused) and its two end flags.
    let first: Vec<Vtx> = line.windows(2).map(|w| Vtx { p: w[0], out: Along::Feature(w[0], w[1]) }).chain(std::iter::once(Vtx { p: line[line.len() - 1], out: Along::Border(0) })).collect();
    let mut pieces: Vec<(Vec<Vtx>, bool, bool)> = vec![(first, false, false)];
    for k in 0..3 {
        let mut next = Vec::new();
        for (piece, start_flag, end_flag) in pieces {
            let mut cur: Vec<Vtx> = Vec::new();
            let mut cur_start = start_flag;
            for i in 0..piece.len() {
                let v = piece[i];
                let v_in = t.inside(k, v.p);
                if v_in {
                    cur.push(v);
                }
                let Some(w) = piece.get(i + 1) else { break };
                let w_in = t.inside(k, w.p);
                if v_in && !w_in {
                    cur.push(Vtx { p: t.cross(&v, k), out: Along::Border(k) });
                    next.push((std::mem::take(&mut cur), cur_start, true));
                } else if !v_in && w_in {
                    cur_start = true;
                    cur.push(Vtx { p: t.cross(&v, k), out: v.out });
                }
            }
            if !cur.is_empty() {
                // The piece reached the input's last vertex inside: it keeps the input's end flag.
                next.push((cur, cur_start, end_flag));
            }
        }
        pieces = next;
    }
    pieces
        .into_iter()
        .filter(|(v, _, _)| v.len() >= 2)
        .map(|(v, s, e)| ClippedLine { points: v.iter().map(|x| x.p).collect(), start_clipped: s, end_clipped: e })
        .collect()
}

/// Clip a ring (unit directions, closed implicitly, every edge a real edge) to the tile `id`, one
/// tile edge at a time (Sutherland-Hodgman on the sphere; the tile is convex). A concave ring that
/// leaves and re-enters comes back as one ring joined by zero-area runs along the border, flagged
/// as clip edges like every border run. None when nothing of it is inside.
pub fn clip_ring(id: &PatchId, ring: &[DVec3]) -> Option<ClippedRing> {
    if ring.len() < 3 {
        return None;
    }
    let t = TileEdges::of(id);
    let n = ring.len();
    let mut work: Vec<Vtx> = (0..n).map(|i| Vtx { p: ring[i], out: Along::Feature(ring[i], ring[(i + 1) % n]) }).collect();
    for k in 0..3 {
        let mut next = Vec::with_capacity(work.len() + 4);
        for i in 0..work.len() {
            let v = work[i];
            let w = work[(i + 1) % work.len()];
            let (v_in, w_in) = (t.inside(k, v.p), t.inside(k, w.p));
            if v_in {
                next.push(v);
            }
            if v_in && !w_in {
                // Leaving: the edge out of `v` (still on v.out) now ends at the crossing, and
                // the border carries the ring from there until it re-enters.
                next.push(Vtx { p: t.cross(&v, k), out: Along::Border(k) });
            } else if !v_in && w_in {
                next.push(Vtx { p: t.cross(&v, k), out: v.out });
            }
        }
        work = next;
        if work.is_empty() {
            return None;
        }
    }
    if work.len() < 3 {
        return None;
    }
    Some(ClippedRing {
        clip_edge: work.iter().map(|v| matches!(v.out, Along::Border(_))).collect(),
        points: work.into_iter().map(|v| v.p).collect(),
    })
}

/// Signed area of a ring on the unit sphere in steradians (counter-clockwise from outside is
/// positive), as a fan of spherical triangles from `origin` (a unit direction near the ring: the
/// same origin for pieces that are to be summed). Each triangle by Van Oosterom and Strackee's
/// formula, with the triple product taken on differences from the origin so a kilometre-sized
/// ring keeps its precision in f64.
pub fn ring_area_sr(ring: &[DVec3], origin: DVec3) -> f64 {
    let o = origin.normalize();
    let n = ring.len();
    (0..n)
        .map(|i| {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            let num = o.dot((a - o).cross(b - o));
            let den = 1.0 + o.dot(a) + a.dot(b) + b.dot(o);
            2.0 * num.atan2(den)
        })
        .sum()
}

// ── The coordinate codec (section 3.3) ───────────────────────────────────────────────────────

/// OSM's integer scale: degrees times 10^7 in 32 bits (the OSM wiki's Node page puts the worst
/// longitude error at about 5.6 mm at the equator).
pub const E7: f64 = 1e7;

/// Degrees to OSM's integer, rounded to the nearest (a truncation would double the error).
pub fn encode_e7(deg: f64) -> i32 {
    (deg * E7).round() as i32
}

/// OSM's integer to degrees, in f64 (never f32: at 180 degrees an f32 step is about 1.7 m).
pub fn decode_e7(v: i32) -> f64 {
    v as f64 / E7
}

/// OSM's integer latitude and longitude to a unit direction, f64 end to end.
pub fn e7_to_dir(lat_e7: i32, lon_e7: i32) -> DVec3 {
    latlon_to_dir_f64(decode_e7(lat_e7), decode_e7(lon_e7))
}

#[cfg(test)]
#[path = "map_tiles_tests.rs"]
mod tests;
