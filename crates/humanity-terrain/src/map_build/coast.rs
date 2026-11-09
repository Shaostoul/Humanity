//! The sea, closed tile by tile (whole-Earth maps rung 2; docs/design/whole-earth-maps.md 3.5).
//!
//! OSM maps the sea as open `natural=coastline` lines with land on the LEFT. Inside one tile, the
//! coastline clipped to it (`map_tiles::clip_polyline`) is a set of pieces, each entering and
//! leaving through the tile's border. The water polygon is closed by walking the border from
//! where one piece leaves to where the next one enters, keeping the water on the right: that is
//! the clockwise way round the tile (the tile's corners are counter-clockwise from outside the
//! planet). The ring is then reversed so the water is on its LEFT, the convention of every water
//! ring in a pack, and every edge the walk laid along the border is flagged as a clip edge: a
//! tile border, never a shoreline, so the shore band at run time never sees it. That flag is what
//! removes the old region-boundary water seam: a tile border through an inlet is invisible to
//! the water. The retired fetcher did the same walk round a rectangle; the tile is a spherical
//! triangle whose edges are great circles, so the walk is exact on the sphere.

use glam::DVec3;

/// One clipped coastline piece (land on its left), unit directions.
#[derive(Clone, Debug)]
pub struct Piece {
    pub pts: Vec<DVec3>,
    pub start_clipped: bool,
    pub end_clipped: bool,
}

/// A closed ring with its clip-edge flags (`clip[i]`: the edge from point i to the next).
pub type FlaggedRing = (Vec<DVec3>, Vec<bool>);

/// Where `p`, a point on the tile's border, lies along it: edge k (corner k to corner k+1) holds
/// t in [k, k+1], counter-clockwise.
pub fn border_t(c: &[DVec3; 3], p: DVec3) -> f64 {
    let mut best = (f64::MAX, 0usize);
    for k in 0..3 {
        let n = c[k].cross(c[(k + 1) % 3]).normalize();
        let d = n.dot(p).abs();
        if d < best.0 {
            best = (d, k);
        }
    }
    let k = best.1;
    let (a, b) = (c[k], c[(k + 1) % 3]);
    let ang = |u: DVec3, v: DVec3| u.cross(v).length().atan2(u.dot(v));
    k as f64 + (ang(a, p) / ang(a, b)).clamp(0.0, 1.0)
}

/// The clockwise distance along the border from `a` down to `b`.
fn cw(a: f64, b: f64) -> f64 {
    (a - b).rem_euclid(3.0)
}

/// Reverse a flagged ring, keeping each flag on its edge.
pub fn reverse_ring(pts: Vec<DVec3>, clip: Vec<bool>) -> FlaggedRing {
    let n = pts.len();
    let rev: Vec<DVec3> = pts.into_iter().rev().collect();
    let flags = (0..n).map(|i| clip[(2 * n - 2 - i) % n]).collect();
    (rev, flags)
}

/// Close the coastline pieces crossing a tile into water rings, water on the left. Pieces with an
/// end inside the tile (a coastline cut off by the extract's edge) cannot be closed and are
/// counted in the error's place: `Ok((rings, skipped))`.
pub fn close_sea(c: &[DVec3; 3], pieces: &[Piece]) -> Result<(Vec<FlaggedRing>, usize), String> {
    let crossing: Vec<&Piece> = pieces.iter().filter(|p| p.start_clipped && p.end_clipped && p.pts.len() >= 2).collect();
    let skipped = pieces.iter().filter(|p| !(p.start_clipped && p.end_clipped)).count();
    let ts: Vec<(f64, f64)> = crossing.iter().map(|p| (border_t(c, p.pts[0]), border_t(c, *p.pts.last().unwrap()))).collect();
    let mut used = vec![false; crossing.len()];
    let mut rings = Vec::new();
    for first in 0..crossing.len() {
        if used[first] {
            continue;
        }
        let (mut pts, mut clip) = (Vec::new(), Vec::new());
        let mut cur = first;
        for _ in 0..=crossing.len() {
            used[cur] = true;
            pts.extend_from_slice(&crossing[cur].pts);
            clip.extend(std::iter::repeat(false).take(crossing[cur].pts.len()));
            *clip.last_mut().unwrap() = true;
            let t_end = ts[cur].1;
            let next = (0..crossing.len())
                .min_by(|a, b| cw(t_end, ts[*a].0).total_cmp(&cw(t_end, ts[*b].0)))
                .ok_or("no piece to walk to")?;
            let reach = cw(t_end, ts[next].0);
            let mut corners: Vec<(f64, usize)> = (0..3).map(|k| (cw(t_end, k as f64), k)).filter(|(d, _)| *d > 0.0 && *d < reach).collect();
            corners.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (_, k) in corners {
                pts.push(c[k]);
                clip.push(true);
            }
            if next == first {
                rings.push(reverse_ring(pts, clip));
                break;
            }
            if used[next] {
                return Err("the coastline pieces do not pair up round the border (a broken coastline)".into());
            }
            cur = next;
        }
        if !used.iter().all(|u| *u) && rings.is_empty() {
            return Err("the border walk did not close".into());
        }
    }
    Ok((rings, skipped))
}

/// One coastline segment for [`water_side`]: its ends and the points before and after it on the
/// chain (for the pseudo-normal at a shared vertex).
#[derive(Clone, Copy, Debug)]
pub struct CoastSeg {
    pub a: DVec3,
    pub b: DVec3,
    pub prev: Option<DVec3>,
    pub next: Option<DVec3>,
}

/// The nearest-segment side test: is `p` on the water side (the right) of the coastline? Over a
/// set of segments (consecutive coastline points, land on the left); at a shared vertex the two
/// segments' normals are summed (the pseudo-normal), which decides both convex and reflex corners
/// and gives the same answer from either segment. None when `segs` is empty.
pub fn water_side(p: DVec3, segs: &[CoastSeg]) -> Option<bool> {
    let mut best: Option<(f64, f64)> = None;
    for s in segs {
        let (a, b) = (s.a, s.b);
        let n = a.cross(b);
        let len = n.length();
        if len < 1e-18 {
            continue;
        }
        let n = n / len;
        // The nearest point of the arc: the projection onto its great circle, clamped to the ends.
        let proj = (p - n * n.dot(p)).normalize_or_zero();
        let inside = a.cross(proj).dot(n) >= 0.0 && proj.cross(b).dot(n) >= 0.0;
        let (q, side) = if inside {
            (proj, n.dot(p))
        } else if (p - a).length_squared() < (p - b).length_squared() {
            (a, s.prev.map_or(n.dot(p), |pp| (n + pp.cross(a).normalize_or_zero()).dot(p)))
        } else {
            (b, s.next.map_or(n.dot(p), |nn| (n + b.cross(nn).normalize_or_zero()).dot(p)))
        };
        let d = (p - q).length();
        if best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, side));
        }
    }
    // Left of the coastline (positive) is land.
    best.map(|(_, side)| side < 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map_tiles::{clip_polyline, ring_area_sr, tile_containing};
    use crate::osm_region::latlon_to_dir_f64;
    use crate::planet_chunks::patch_corners;

    /// A straight coastline running north through a level-12 tile, land to its west: the walk
    /// closes the eastern part into one water ring, counter-clockwise (positive area), whose border
    /// edges are flagged and whose coastline edges are not, and a point east of the line is water.
    /// Seen red 2026-11-09 with the walk going counter-clockwise (`(b - a).rem_euclid(3.0)` for
    /// `cw`): "the water ring is counter-clockwise; left: -2.36e-8" (it enclosed the land).
    #[test]
    fn a_coastline_through_a_tile_closes_to_the_water_side() {
        let tile = tile_containing(47.64, -122.69, 12);
        let c = patch_corners(&tile);
        let centre = (c[0] + c[1] + c[2]).normalize();
        let (lat, lon) = crate::osm_region::dir_to_latlon_f64(centre);
        let line: Vec<DVec3> = [(-0.02, 0.0), (0.0, 0.0001), (0.02, 0.0)].iter().map(|(dlat, dlon)| latlon_to_dir_f64(lat + dlat, lon + dlon)).collect();
        let pieces: Vec<Piece> = clip_polyline(&tile, &line).into_iter().map(|p| Piece { pts: p.points, start_clipped: p.start_clipped, end_clipped: p.end_clipped }).collect();
        assert_eq!(pieces.len(), 1);
        let (rings, skipped) = close_sea(&c, &pieces).expect("closes");
        assert_eq!((rings.len(), skipped), (1, 0));
        let (pts, clip) = &rings[0];
        let area = ring_area_sr(pts, centre);
        assert!(area > 0.0, "the water ring is counter-clockwise; left: {area:e}");
        let tile_area = ring_area_sr(&c, centre);
        assert!(area < tile_area * 0.7 && area > tile_area * 0.3, "about half the tile: {:.2}", area / tile_area);
        assert!(clip.iter().filter(|f| **f).count() >= 2, "the border edges are flagged");
        assert!(clip.iter().filter(|f| !**f).count() >= 2, "the coastline edges are not");
        let segs: Vec<CoastSeg> = line.windows(2).enumerate().map(|(i, w)| CoastSeg { a: w[0], b: w[1], prev: i.checked_sub(1).map(|j| line[j]), next: line.get(i + 2).copied() }).collect();
        assert_eq!(water_side(latlon_to_dir_f64(lat, lon + 0.005), &segs), Some(true), "east of the coast is water");
        assert_eq!(water_side(latlon_to_dir_f64(lat, lon - 0.005), &segs), Some(false), "west is land");
        assert_eq!(water_side(latlon_to_dir_f64(lat + 0.03, lon + 0.001), &segs), Some(true), "past the end, by the end segment");
    }

    /// Reversing a ring keeps every flag on its own edge. Seen red 2026-11-09 with
    /// `clip[n - 1 - i]`: "the flagged edge is still b to c".
    #[test]
    fn reversing_keeps_flags_on_their_edges() {
        let p = [DVec3::X, DVec3::Y, DVec3::Z, DVec3::NEG_X];
        // Edge 1 (Y to Z) is the clip edge.
        let (rev, flags) = reverse_ring(p.to_vec(), vec![false, true, false, false]);
        let i = flags.iter().position(|f| *f).unwrap();
        assert_eq!((rev[i], rev[(i + 1) % 4]), (DVec3::Z, DVec3::Y), "the flagged edge is still b to c");
        assert_eq!(flags.iter().filter(|f| **f).count(), 1);
    }
}
