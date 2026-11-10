//! ROAD BEDS AND BUILDING PADS, inside the one ground formula (whole-Earth maps, the rung between 2
//! and 3, 2026-11-09; docs/design/whole-earth-maps.md sections 6.1 to 6.3). A child module of
//! `map_ground`, which calls it from `MapSet::ground` for every land point of a deep enough patch.
//!
//! The operator approved the rung on 2026-11-09: "These all sound reasonable. LEt's proceed."
//!
//! Until this rung a road's ribbon floated 0.18 m (a footway's 0.12 m) over whatever the ground did
//! under it, and a building stood on the lowest ground under its walls with a 2.5 m skirt buried
//! below that. Now the ground itself is shaped for them, at run time, from the land layer only
//! (never stored in a pack, section 3.4, so a pack stays a Collective Database):
//! - **Road beds.** Each piece of a road (an OSM way clipped to a level-12 tile) gets a centreline
//!   profile: the natural ground sampled along it every `profile_step_m`, averaged over its class's
//!   window, held to its class's grade (the mean of the lowest graded line above it and the highest
//!   below it, so cut and fill balance), and pinned to the natural ground at both of its ends.
//!   Across, a flat bed the carriageway's width plus a shoulder each side; past it, cut or fill back
//!   to the natural ground at 1 in 2.
//! - **Building pads.** A footprint's floor is the median of the natural ground over it; the
//!   footprint and a 1.5 m apron are held at that floor; past it, 1 in 2 back to natural.
//!
//! WHY EACH PIECE IS PINNED AT ITS ENDS (the identity rule, the reason this needs no new pack
//! format). Where a road crosses a tile border, the two tiles hold two pieces that end and start at
//! the same point: the clip's crossing of the shared great-circle edge (section 3.2), written as the
//! same OSM integers on both sides. A piece's profile depends only on the piece, and both pieces are
//! pinned to the natural ground at that shared point, so they meet there exactly, and every tile
//! that reads either piece computes the same numbers. A point near a border also sees the
//! neighbouring tiles' pieces and pads (read from the same pack, or from the neighbouring pack when
//! it is installed), so the ground does not depend on which tile evaluates it. A margin stored per
//! tile would have duplicated every border road and still needed a profile whose reach fits the
//! margin; a profile stored per way would have written land-layer heights into the OSM layer.
//!
//! HOW SEVERAL COMBINE. Inside one or more road beds the ground is their weighted mean (each weight
//! 1 across the carriageway, fading to 0 across the shoulder), so an intersection warps smoothly
//! between its roads; inside a pad and no bed, the pad's floor (pads sharing a wall average the
//! same way); a bed crossing a pad fades into it across the shoulder. Outside every core, each
//! modifier within reach clamps the ground to its slope room around its target (where a slope
//! already reaches the natural ground, the ground stays natural), the nearest applied last, so it
//! wins at its own edge and the ground is continuous there. The SHORE BAND runs after this, in
//! `MapSet::ground`: the coast wins within its band (the operator's priority), so a bed or a pad
//! never dries a harbour, never floods the shore and never lifts land above the band's cap.
//!
//! SECTION 6.3: a modifier changes a patch's mesh only where it is wider than about one vertex
//! spacing of that patch (`geometry_spacings`), and never on patches shallower than `first_depth`;
//! coarser patches keep the natural ground (a building there stands on its plinth, which the mesher
//! carries down to the natural ground). The per-patch material mask that would paint roads on
//! coarse patches is still deferred.
//!
//! COST: a [`BedTile`] is built per level-12 tile on first use (the decoded ways and footprints of
//! the tile and its neighbours, in a 16 m grid in the pack's plane); each piece's profile and each
//! pad's floor are computed the first time a query needs them. A bed tile is a residency slot like
//! the land tiles: dropped when no patch has needed it for a while, tagged by its pack, and built
//! afresh every frame with the Dev page's "turn off all reuse" switch off. f64 throughout.

use super::{MapSet, PackState, R};
use crate::map_levels::BedParams;
use crate::map_tiles::{ancestor_at, decode_e7, e7_to_dir, tile_containing_dir};
use crate::osm_region::{latlon_to_dir_f64, road_full_width_m};
use crate::planet_chunks::{patch_corners, PatchId};
use glam::DVec3;
use std::sync::OnceLock;

/// The bed grid's cell, metres.
const CELL_M: f64 = 16.0;
/// At most this many modifiers are weighed at one point (the nearest are kept).
const MAX_HITS: usize = 24;
/// An item standing for a whole pad rather than one road segment.
const WHOLE: u32 = u32::MAX;

fn smoothstep(x: f64) -> f64 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// One road piece: an OSM way clipped to one level-12 tile, with no bridge or tunnel.
pub(crate) struct Road {
    pub osm_id: u64,
    pub class: u8,
    /// Half the carriageway (the ribbon's half width), and half the bed (plus the shoulder).
    pub half_w: f64,
    pub core: f64,
    start_clipped: bool,
    end_clipped: bool,
    /// The piece's points on the sphere (the profile samples between these, the same numbers in
    /// every pack) and in the evaluating pack's plane (distances).
    dirs: Vec<DVec3>,
    pts: Vec<(f64, f64)>,
    profile: OnceLock<Option<Profile>>,
}

/// A piece's centreline heights: per segment, its first sample and its number of steps.
struct Profile {
    start: Vec<u32>,
    steps: Vec<u32>,
    vals: Vec<f32>,
}

impl Profile {
    fn at(&self, k: usize, t: f64) -> f64 {
        let (st, m) = (self.start[k] as usize, self.steps[k] as usize);
        let f = t.clamp(0.0, 1.0) * m as f64;
        let i = (f.floor() as usize).min(m - 1);
        let fr = f - i as f64;
        self.vals[st + i] as f64 * (1.0 - fr) + self.vals[st + i + 1] as f64 * fr
    }
}

/// One building's pad.
pub(crate) struct Pad {
    pub osm_id: u64,
    ring: Vec<(f64, f64)>,
    ring_e7: Vec<(i32, i32)>,
    /// The square root of the footprint's area (the 6.3 size test).
    size_m: f64,
    /// The footprint's box in the plane (x0, y0, x1, y1): a point farther from it than a pad's
    /// reach skips the polygon tests (most of a dense cell's pads are out of reach).
    bbox: (f64, f64, f64, f64),
    floor: OnceLock<Option<f32>>,
}

/// The beds and pads that can reach one level-12 tile, gridded in the evaluating pack's plane.
pub(crate) struct BedTile {
    pub(crate) roads: Vec<Road>,
    pub(crate) pads: Vec<Pad>,
    x0: f64,
    y0: f64,
    nx: usize,
    ny: usize,
    cell_start: Vec<u32>,
    /// (feature, segment): a feature below `roads.len()` is a road and the segment its index; above
    /// it a pad, with [`WHOLE`].
    items: Vec<(u32, u32)>,
    /// The widest core of any modifier here (the 6.3 early out).
    max_width: f64,
}

/// One modifier near a query point.
#[derive(Clone, Copy)]
struct Hit {
    feature: u32,
    road: bool,
    /// Distance from the centreline (a road) or outside the footprint (a pad, 0 inside).
    r: f64,
    core: f64,
    half: f64,
    k: u32,
    t: f64,
}

/// Point in a ring (even-odd), plane or degrees alike.
fn inside(ring: &[(f64, f64)], p: (f64, f64)) -> bool {
    let mut c = false;
    let n = ring.len();
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && a.0 + (p.1 - a.1) * (b.0 - a.0) / (b.1 - a.1) > p.0 {
            c = !c;
        }
    }
    c
}

/// Distance from `p` to the segment a-b, and the unclamped parameter of the foot.
fn foot(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2 } else { 0.0 };
    let tc = t.clamp(0.0, 1.0);
    let q = (a.0 + tc * dx, a.1 + tc * dy);
    (((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt(), t)
}

/// The level-12 tiles touching `tile` (itself, across its edges and round its corners): a small
/// circle round each corner meets every tile sharing it, and the tiles across the edges share
/// two corners each.
pub(crate) fn neighbour_tiles(tile: &PatchId) -> Vec<PatchId> {
    let c = patch_corners(tile);
    let mut v = vec![*tile];
    for a in c {
        let e = if a.y.abs() < 0.9 { DVec3::Y.cross(a).normalize() } else { DVec3::X.cross(a).normalize() };
        let n = a.cross(e);
        for i in 0..12 {
            let th = i as f64 * std::f64::consts::TAU / 12.0;
            let d = (a + (e * th.cos() + n * th.sin()) * (2.0 / R)).normalize();
            let t = tile_containing_dir(d, tile.depth);
            if !v.contains(&t) {
                v.push(t);
            }
        }
    }
    v
}

impl Road {
    /// The bed's height at segment `k`, parameter `t`; None where the land layer has no data.
    fn target(&self, set: &MapSet, k: usize, t: f64) -> Option<f64> {
        self.profile.get_or_init(|| profile_of(set, self)).as_ref().map(|p| p.at(k, t))
    }
}

/// A piece's profile (the module header's recipe).
fn profile_of(set: &MapSet, road: &Road) -> Option<Profile> {
    let bp = &set.levels.beds;
    let cls = bp.class(road.class)?;
    let finest = set.levels.finest();
    let n = road.dirs.len();
    if n < 2 {
        return None;
    }
    let (mut start, mut steps) = (Vec::with_capacity(n - 1), Vec::with_capacity(n - 1));
    let (mut g, mut s) = (Vec::new(), Vec::new());
    g.push(set.land_at(road.dirs[0], finest)?);
    s.push(0.0);
    let mut acc = 0.0;
    for k in 0..n - 1 {
        let (a, b) = (road.dirs[k], road.dirs[k + 1]);
        let len = (b - a).length() * R;
        let m = ((len / bp.profile_step_m).ceil() as u32).max(1);
        start.push((g.len() - 1) as u32);
        steps.push(m);
        for i in 1..=m {
            let t = i as f64 / m as f64;
            g.push(set.land_at((a * (1.0 - t) + b * t).normalize(), finest)?);
            s.push(acc + t * len);
        }
        acc += len;
    }
    // Averaged over the class's window (truncated at the ends), by prefix sums.
    let len = g.len();
    let mut pre = vec![0.0; len + 1];
    for i in 0..len {
        pre[i + 1] = pre[i] + g[i];
    }
    let half = cls.smooth_m * 0.5;
    let (mut lo, mut hi) = (0usize, 0usize);
    let mut m = vec![0.0; len];
    for i in 0..len {
        while s[lo] < s[i] - half {
            lo += 1;
        }
        while hi + 1 < len && s[hi + 1] <= s[i] + half {
            hi += 1;
        }
        m[i] = (pre[hi + 1] - pre[lo]) / (hi + 1 - lo) as f64;
    }
    // Held to the grade: the lowest graded line above and the highest below, averaged.
    let gr = cls.grade;
    let (mut up, mut dn) = (m.clone(), m);
    for i in 1..len {
        let ds = s[i] - s[i - 1];
        up[i] = up[i].max(up[i - 1] - gr * ds);
        dn[i] = dn[i].min(dn[i - 1] + gr * ds);
    }
    for i in (0..len - 1).rev() {
        let ds = s[i + 1] - s[i];
        up[i] = up[i].max(up[i + 1] - gr * ds);
        dn[i] = dn[i].min(dn[i + 1] + gr * ds);
    }
    let mut p: Vec<f64> = up.iter().zip(&dn).map(|(a, b)| (a + b) * 0.5).collect();
    // Pinned to the natural ground at both ends.
    let total = s[len - 1];
    if total > 0.0 {
        let (d0, d1) = (g[0] - p[0], g[len - 1] - p[len - 1]);
        let r0 = bp.pin_blend_m.max(d0.abs() / bp.pin_grade).min(total);
        let r1 = bp.pin_blend_m.max(d1.abs() / bp.pin_grade).min(total);
        for i in 0..len {
            p[i] += d0 * (1.0 - s[i] / r0).max(0.0) + d1 * (1.0 - (total - s[i]) / r1).max(0.0);
        }
    } else {
        p = g;
    }
    Some(Profile { start, steps, vals: p.into_iter().map(|v| v as f32).collect() })
}

impl Pad {
    /// The pad's floor: the median of the land layer over the footprint, sampled at its corners
    /// and on a grid inside it laid in degrees from its own OSM integers (so every pack that reads
    /// it samples the same points); None where the land layer has no data.
    fn floor(&self, set: &MapSet) -> Option<f64> {
        self.floor.get_or_init(|| floor_of(set, self)).map(|f| f as f64)
    }
}

fn floor_of(set: &MapSet, pad: &Pad) -> Option<f32> {
    let finest = set.levels.finest();
    let ll: Vec<(f64, f64)> = pad.ring_e7.iter().map(|p| (decode_e7(p.0), decode_e7(p.1))).collect();
    let mut hs = Vec::with_capacity(ll.len() * 2);
    for &(la, lo) in &ll {
        hs.push(set.land_at(latlon_to_dir_f64(la, lo), finest)?);
    }
    let step_lat = set.levels.beds.pad_sample_m / 111_195.0;
    let step_lon = step_lat / ll[0].0.to_radians().cos().max(0.05);
    let (mut s, mut n, mut w, mut e) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(la, lo) in &ll {
        s = s.min(la);
        n = n.max(la);
        w = w.min(lo);
        e = e.max(lo);
    }
    let mut la = s + step_lat * 0.5;
    while la < n {
        let mut lo = w + step_lon * 0.5;
        while lo < e {
            if inside(&ll, (la, lo)) {
                hs.push(set.land_at(latlon_to_dir_f64(la, lo), finest)?);
            }
            lo += step_lon;
        }
        la += step_lat;
    }
    hs.sort_by(|a, b| a.total_cmp(b));
    Some(hs[hs.len() / 2] as f32)
}

impl BedTile {
    /// The beds and pads that can reach `tile` (level 12, in pack `ps`), from it and its
    /// neighbours in every installed pack.
    pub(crate) fn build(set: &MapSet, ps: &PackState, tile: &PatchId) -> Self {
        let bp = &set.levels.beds;
        let plane = ps.plane();
        let tc: Vec<(f64, f64)> = patch_corners(tile).iter().map(|d| plane.to(*d)).collect();
        let (tx0, ty0) = (tc.iter().map(|p| p.0).fold(f64::MAX, f64::min), tc.iter().map(|p| p.1).fold(f64::MAX, f64::min));
        let (tx1, ty1) = (tc.iter().map(|p| p.0).fold(f64::MIN, f64::max), tc.iter().map(|p| p.1).fold(f64::MIN, f64::max));
        let overlaps = |pts: &[(f64, f64)], pad: f64| {
            let (x0, y0) = (pts.iter().map(|p| p.0).fold(f64::MAX, f64::min), pts.iter().map(|p| p.1).fold(f64::MAX, f64::min));
            let (x1, y1) = (pts.iter().map(|p| p.0).fold(f64::MIN, f64::max), pts.iter().map(|p| p.1).fold(f64::MIN, f64::max));
            x1 + pad >= tx0 && x0 - pad <= tx1 && y1 + pad >= ty0 && y0 - pad <= ty1
        };
        let (mut roads, mut pads) = (Vec::new(), Vec::new());
        for n in neighbour_tiles(tile) {
            let Some(owner) = set.packs.iter().find(|p| ancestor_at(n, p.id.depth) == Some(p.id)) else { continue };
            for w in owner.pack.ways(n.path).unwrap_or_default() {
                if w.class > 5 || w.bridge || w.tunnel || w.layer > 0 || w.points.len() < 2 {
                    continue;
                }
                let half_w = if w.width_m > 0.0 { w.width_m as f64 } else { road_full_width_m(w.class) as f64 } * 0.5;
                let core = half_w + bp.shoulder_m;
                let dirs: Vec<DVec3> = w.points.iter().map(|p| e7_to_dir(p.0, p.1)).collect();
                let pts: Vec<(f64, f64)> = dirs.iter().map(|d| plane.to(*d)).collect();
                if !overlaps(&pts, core + bp.reach_m) {
                    continue;
                }
                roads.push(Road { osm_id: w.osm_id, class: w.class, half_w, core, start_clipped: w.start_clipped, end_clipped: w.end_clipped, dirs, pts, profile: OnceLock::new() });
            }
            for b in owner.pack.buildings(n.path).unwrap_or_default() {
                if b.min_height_m > 0.0 || b.ring.len() < 3 {
                    continue;
                }
                let ring: Vec<(f64, f64)> = b.ring.iter().map(|p| plane.to(e7_to_dir(p.0, p.1))).collect();
                if !overlaps(&ring, bp.apron_m + bp.reach_m) {
                    continue;
                }
                let k = ring.len();
                let area = (0..k).map(|i| ring[i].0 * ring[(i + 1) % k].1 - ring[(i + 1) % k].0 * ring[i].1).sum::<f64>().abs() * 0.5;
                let bbox = (ring.iter().map(|q| q.0).fold(f64::MAX, f64::min), ring.iter().map(|q| q.1).fold(f64::MAX, f64::min), ring.iter().map(|q| q.0).fold(f64::MIN, f64::max), ring.iter().map(|q| q.1).fold(f64::MIN, f64::max));
                pads.push(Pad { osm_id: b.osm_id, ring, ring_e7: b.ring, size_m: area.sqrt(), bbox, floor: OnceLock::new() });
            }
        }
        let (x0, y0) = (tx0 - CELL_M, ty0 - CELL_M);
        let (nx, ny) = ((((tx1 - tx0) / CELL_M).ceil() as usize) + 2, (((ty1 - ty0) / CELL_M).ceil() as usize) + 2);
        // Each item into every cell its reach box covers (two passes: count, then fill).
        let mut boxes: Vec<((u32, u32), (f64, f64, f64, f64))> = Vec::new();
        for (fi, r) in roads.iter().enumerate() {
            let pad = r.core + bp.reach_m;
            for k in 0..r.pts.len() - 1 {
                let (a, b) = (r.pts[k], r.pts[k + 1]);
                boxes.push(((fi as u32, k as u32), (a.0.min(b.0) - pad, a.1.min(b.1) - pad, a.0.max(b.0) + pad, a.1.max(b.1) + pad)));
            }
        }
        for (pi, p) in pads.iter().enumerate() {
            let pad = bp.apron_m + bp.reach_m;
            let (bx0, by0) = (p.ring.iter().map(|q| q.0).fold(f64::MAX, f64::min), p.ring.iter().map(|q| q.1).fold(f64::MAX, f64::min));
            let (bx1, by1) = (p.ring.iter().map(|q| q.0).fold(f64::MIN, f64::max), p.ring.iter().map(|q| q.1).fold(f64::MIN, f64::max));
            boxes.push((((roads.len() + pi) as u32, WHOLE), (bx0 - pad, by0 - pad, bx1 + pad, by1 + pad)));
        }
        let span = |lo: f64, hi: f64, o: f64, n: usize| -> Option<(usize, usize)> {
            let a = ((lo - o) / CELL_M).floor().max(0.0) as usize;
            let b = (((hi - o) / CELL_M).floor() as i64).min(n as i64 - 1);
            (b >= a as i64).then_some((a, b as usize))
        };
        let mut counts = vec![0u32; nx * ny + 1];
        for (_, bx) in &boxes {
            if let (Some((c0, c1)), Some((r0, r1))) = (span(bx.0, bx.2, x0, nx), span(bx.1, bx.3, y0, ny)) {
                for row in r0..=r1 {
                    for col in c0..=c1 {
                        counts[row * nx + col + 1] += 1;
                    }
                }
            }
        }
        for i in 1..counts.len() {
            counts[i] += counts[i - 1];
        }
        let mut fill = counts.clone();
        let mut items = vec![(0u32, 0u32); *counts.last().unwrap_or(&0) as usize];
        for (it, bx) in &boxes {
            if let (Some((c0, c1)), Some((r0, r1))) = (span(bx.0, bx.2, x0, nx), span(bx.1, bx.3, y0, ny)) {
                for row in r0..=r1 {
                    for col in c0..=c1 {
                        let c = row * nx + col;
                        items[fill[c] as usize] = *it;
                        fill[c] += 1;
                    }
                }
            }
        }
        let max_width = roads.iter().map(|r| 2.0 * r.core).chain(pads.iter().map(|p| p.size_m + 2.0 * bp.apron_m)).fold(0.0, f64::max);
        BedTile { roads, pads, x0, y0, nx, ny, cell_start: counts, items, max_width }
    }

    /// The modifiers near plane point `p` that a patch of vertex spacing `spacing` can draw.
    fn hits(&self, bp: &BedParams, p: (f64, f64), spacing: f64, out: &mut [Option<Hit>; MAX_HITS]) -> usize {
        let min_w = spacing * bp.geometry_spacings;
        let (col, row) = (((p.0 - self.x0) / CELL_M).floor(), ((p.1 - self.y0) / CELL_M).floor());
        if col < 0.0 || row < 0.0 || col >= self.nx as f64 || row >= self.ny as f64 {
            return 0;
        }
        let c = row as usize * self.nx + col as usize;
        let mut n = 0usize;
        for &(fi, k) in &self.items[self.cell_start[c] as usize..self.cell_start[c + 1] as usize] {
            let hit = if (fi as usize) < self.roads.len() {
                let road = &self.roads[fi as usize];
                if 2.0 * road.core <= min_w {
                    continue;
                }
                let last = road.pts.len() - 2;
                let (r, t) = foot(p, road.pts[k as usize], road.pts[k as usize + 1]);
                // A clipped end is not an end of the road: past it, the neighbouring tile's piece
                // carries on from the same point.
                if (k == 0 && road.start_clipped && t < 0.0) || (k as usize == last && road.end_clipped && t > 1.0) || r > road.core + bp.reach_m {
                    continue;
                }
                Hit { feature: fi, road: true, r, core: road.core, half: road.half_w, k, t: t.clamp(0.0, 1.0) }
            } else {
                let pad = &self.pads[fi as usize - self.roads.len()];
                if pad.size_m + 2.0 * bp.apron_m <= min_w {
                    continue;
                }
                let (bx, by) = ((pad.bbox.0 - p.0).max(p.0 - pad.bbox.2).max(0.0), (pad.bbox.1 - p.1).max(p.1 - pad.bbox.3).max(0.0));
                if bx * bx + by * by > (bp.apron_m + bp.reach_m).powi(2) {
                    continue;
                }
                let r = if inside(&pad.ring, p) {
                    0.0
                } else {
                    (0..pad.ring.len()).map(|i| foot(p, pad.ring[i], pad.ring[(i + 1) % pad.ring.len()]).0).fold(f64::MAX, f64::min)
                };
                if r > bp.apron_m + bp.reach_m {
                    continue;
                }
                Hit { feature: fi, road: false, r, core: bp.apron_m, half: 0.0, k: 0, t: 0.0 }
            };
            // One hit per feature (a road's nearest segment), the nearest MAX_HITS features kept.
            if let Some(h) = out[..n].iter_mut().flatten().find(|h| h.feature == fi) {
                if hit.r < h.r {
                    *h = hit;
                }
            } else if n < MAX_HITS {
                out[n] = Some(hit);
                n += 1;
            } else if let Some(far) = out.iter_mut().flatten().max_by(|a, b| (a.r - a.core).total_cmp(&(b.r - b.core))) {
                if hit.r - hit.core < far.r - far.core {
                    *far = hit;
                }
            }
        }
        n
    }

    fn target(&self, set: &MapSet, h: &Hit) -> Option<f64> {
        if h.road {
            self.roads[h.feature as usize].target(set, h.k as usize, h.t)
        } else {
            self.pads[h.feature as usize - self.roads.len()].floor(set)
        }
    }

    /// THE BEDS AND PADS at plane point `p`, whose natural ground (with any fine relief) is
    /// `natural` metres, for a patch of vertex spacing `spacing` metres: the shaped ground (the
    /// module header's combination rule).
    pub(crate) fn apply(&self, set: &MapSet, p: (f64, f64), natural: f64, spacing: f64) -> f64 {
        let bp = &set.levels.beds;
        if self.max_width <= spacing * bp.geometry_spacings {
            return natural;
        }
        let mut buf = [None; MAX_HITS];
        let n = self.hits(bp, p, spacing, &mut buf);
        if n == 0 {
            return natural;
        }
        let mut hs: Vec<(Hit, f64)> = Vec::with_capacity(n);
        for h in buf[..n].iter().flatten() {
            if let Some(t) = self.target(set, h) {
                hs.push((*h, t));
            }
        }
        // Inside a core: the beds' weighted mean, then the pads'.
        let (mut sr, mut wr, mut wr_max, mut near_r) = (0.0, 0.0, 0.0f64, None::<(f64, f64)>);
        let (mut sp, mut wp, mut near_p) = (0.0, 0.0, None::<(f64, f64)>);
        for (h, t) in &hs {
            if h.r > h.core {
                continue;
            }
            if h.road {
                let w = smoothstep((h.core - h.r) / (h.core - h.half).max(0.25));
                sr += w * t;
                wr += w;
                wr_max = wr_max.max(w);
                if near_r.is_none_or(|(r, _)| h.r < r) {
                    near_r = Some((h.r, *t));
                }
            } else {
                let w = smoothstep((h.core - h.r) / h.core.max(0.25));
                sp += w * t;
                wp += w;
                if near_p.is_none_or(|(r, _)| h.r < r) {
                    near_p = Some((h.r, *t));
                }
            }
        }
        let tr = near_r.map(|(_, t)| if wr > 1e-9 { sr / wr } else { t });
        let tp = near_p.map(|(_, t)| if wp > 1e-9 { sp / wp } else { t });
        match (tr, tp) {
            (Some(r), None) => return r,
            (None, Some(p)) => return p,
            (Some(r), Some(p)) => return r * wr_max + p * (1.0 - wr_max),
            (None, None) => {}
        }
        // Outside every core: each clamps to its slope room, the nearest last.
        hs.sort_by(|a, b| (b.0.r - b.0.core).total_cmp(&(a.0.r - a.0.core)));
        let mut h = natural;
        for (hit, t) in &hs {
            let room = bp.slope_room(hit.r - hit.core);
            h = h.clamp(t - room, t + room);
        }
        h
    }

    /// The road piece whose centreline passes nearest `p` (the bed probe's question): its index,
    /// the nearest segment and parameter, and the distance.
    pub(crate) fn nearest_road(&self, p: (f64, f64)) -> Option<(usize, usize, f64, f64)> {
        let mut best: Option<(usize, usize, f64, f64)> = None;
        for (i, r) in self.roads.iter().enumerate() {
            for k in 0..r.pts.len() - 1 {
                let (d, t) = foot(p, r.pts[k], r.pts[k + 1]);
                if best.is_none_or(|b| d < b.3) {
                    best = Some((i, k, t.clamp(0.0, 1.0), d));
                }
            }
        }
        best
    }

    /// The pad holding `p`, or else the nearest one.
    pub(crate) fn nearest_pad(&self, p: (f64, f64)) -> Option<usize> {
        let dist = |pad: &Pad| if inside(&pad.ring, p) { 0.0 } else { (0..pad.ring.len()).map(|i| foot(p, pad.ring[i], pad.ring[(i + 1) % pad.ring.len()]).0).fold(f64::MAX, f64::min) };
        (0..self.pads.len()).min_by(|a, b| dist(&self.pads[*a]).total_cmp(&dist(&self.pads[*b])))
    }

    /// A road's centreline point at (k, t) on the sphere, and the bed's height there.
    pub(crate) fn road_point(&self, set: &MapSet, i: usize, k: usize, t: f64) -> (DVec3, DVec3, Option<f64>) {
        let r = &self.roads[i];
        let (a, b) = (r.dirs[k], r.dirs[k + 1]);
        (a, b, r.target(set, k, t))
    }

    /// A pad's footprint on the sphere and its floor.
    pub(crate) fn pad_ring(&self, set: &MapSet, i: usize) -> (Vec<DVec3>, Option<f64>) {
        let p = &self.pads[i];
        (p.ring_e7.iter().map(|q| e7_to_dir(q.0, q.1)).collect(), p.floor(set))
    }

    /// The modifiers whose core holds plane point `p` for the finest patches: (is a road, its OSM
    /// id). Where two share a point their heights blend by design (a junction, a road across an
    /// apron), so the tests judge flatness only where one owns it.
    #[cfg(test)]
    pub(crate) fn cores_at(&self, set: &MapSet, p: (f64, f64)) -> Vec<(bool, u64)> {
        let mut buf = [None; MAX_HITS];
        let n = self.hits(&set.levels.beds, p, 0.0, &mut buf);
        buf[..n]
            .iter()
            .flatten()
            .filter(|h| h.r <= h.core)
            .map(|h| (h.road, if h.road { self.roads[h.feature as usize].osm_id } else { self.pads[h.feature as usize - self.roads.len()].osm_id }))
            .collect()
    }

    /// How many road pieces and pads this tile weighs (the Dev readout and the log).
    pub(crate) fn counts(&self) -> (usize, usize) {
        (self.roads.len(), self.pads.len())
    }
}

#[cfg(test)]
#[path = "map_beds_tests.rs"]
mod tests;
