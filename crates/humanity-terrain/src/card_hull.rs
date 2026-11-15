//! TIGHT TREE CARD OUTLINES (2026-11-14, the far tree cards' performance rung 2).
//!
//! WHY. A far tree card (the patch-baked sprite card, type 12's card branch in `fs_terrain`) was a
//! full square of its atlas tile, and the baked tree fills only a fifth to a quarter of that
//! square: the frame is square on the tree's larger extent with a 5% margin, and a conifer is a
//! narrow triangle inside it. Every pixel of the rest was rasterized, its fragment shader launched,
//! its sprite texel fetched, and then discarded. Rung 1's profile (docs/ai/renderer.md, the tree
//! card section) put that at 1.5 to 2.5 ms of the world surface pass at the default 1,500 m, and
//! the sun pass paid it again.
//!
//! THE TECHNIQUE: tight-fit sprite geometry, the standard cutout for particles and impostors
//! (Emil Persson, "Particle Trimming", humus.name 2009: fit a polygon of a few vertices around each
//! sprite's visible texels and draw that instead of the quad, trading a few vertices for the
//! pixels; Unreal Engine 4.8's particle cutouts do the same per flipbook frame, with 4 or 8
//! bounding vertices). The
//! card's position is an affine function of its (u, v) on the tile, so any polygon inside the tile
//! maps to the same texels at the same places: the picture is unchanged and only the empty pixels
//! go.
//! - [`fit`] takes one baked tile's alpha (read back from the GPU bake, `billboard_bake.rs`, so it
//!   measures what is drawn and not what was meant to be), builds the convex hull of every texel a
//!   sample can reach (bilinear filtering reaches one texel past a covered texel's centre; a quarter
//!   texel more for safety), and reduces it to at most `k` vertices by removing, one at a time, the
//!   edge whose removal adds the least area (each neighbour extended to meet the other), never
//!   leaving the tile. The result always CONTAINS the hull, so no visible texel is ever cut off:
//!   the hashed-alpha edge and the far dissolve are decided per pixel exactly as before.
//! - [`emit_sprite_card`] (the patch builder's card emitter, moved here from `planet_chunks`) draws
//!   each face of a card as that polygon, fanned from its first vertex, then the same fan wound
//!   backwards so the face survives back-face culling from either side. A face of `n` vertices is
//!   `6 (n - 2)` indices ([`face_indices`]); the square (`n` 4) is exactly the quad it replaced.
//! - The table is filled by the bake, one entry a tile, like the footprints
//!   (`tree_registry::set_card_footprint`). Before the bake, and for a tile whose bake failed, a
//!   card keeps the full square (padded to the face size of the patch).
//! - The showcase pin `card_hull` picks the outline: "0" is the full square (the red twin, the
//!   rung 1 geometry exactly), "4", "6" or "8" a fitted polygon of that many vertices. Every face of
//!   one patch has the same vertex count (one table snapshot a patch build), which is what the GPU
//!   card cull (`renderer/tree_card_cull.rs`) needs to find a patch's cards.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::RwLock;

use crate::planet_surface::SurfaceVertexData;
use crate::tree_registry::{CardFootprint, ATLAS_TILES};

/// The most vertices one card face can have.
pub const MAX_VERTS: usize = 8;

/// The vertex counts a fitted outline can be asked for (the showcase arms).
pub const FIT_KS: [u8; 3] = [4, 6, 8];

/// The default outline's vertex count, chosen by the same-boot A/B of 2026-11-14 (docs/ai/renderer.md,
/// the tree card section): over the shipped atlas six vertices cover 0.340 of the square on average
/// against eight's 0.337 and four's 0.604, so the two extra vertices bought almost no pixels, and
/// the six-vertex arm drew the world surface pass cheaper than the eight-vertex median at three of
/// the four forest vantages (fewer, fatter triangles: less vertex work and less 2x2 overshading
/// along the inner edges of the fan).
pub const DEFAULT_K: u8 = 6;

/// How far past a covered texel's centre a sample can still see it, in texels: bilinear filtering
/// reaches one texel, and a quarter more covers float rounding in the card's (u, v) interpolation.
pub const REACH_TEXELS: f32 = 1.25;

/// One card face's outline on its atlas tile: `n` vertices in (u, v) (u 0 to 1 left to right, v 0
/// at the frame's bottom edge to 1 at its top, the card's own coordinates), counter-clockwise.
/// Slots past `n` are unused.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardHull {
    pub n: u8,
    pub uv: [[f32; 2]; MAX_VERTS],
}

impl CardHull {
    /// The full square of the tile: the card as it was before this rung.
    pub const SQUARE: CardHull = CardHull { n: 4, uv: [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.0, 1.0], [0.0, 1.0], [0.0, 1.0], [0.0, 1.0]] };

    /// The outline's vertices.
    pub fn verts(&self) -> &[[f32; 2]] {
        &self.uv[..self.n as usize]
    }

    /// Its area as a share of the tile (the shoelace formula).
    pub fn area(&self) -> f32 {
        polygon_area(self.verts())
    }

    /// The same outline with exactly `k` vertices: the last one repeated (degenerate triangles,
    /// no pixels) when it has fewer. An outline with more is returned as it is.
    pub fn padded(mut self, k: u8) -> CardHull {
        let k = k.clamp(3, MAX_VERTS as u8);
        let n = self.n.max(1) as usize;
        for i in n..k as usize {
            self.uv[i] = self.uv[n - 1];
        }
        self.n = self.n.max(k);
        self
    }

    /// Does the outline contain the point (counter-clockwise, so inside is left of every edge)?
    pub fn contains(&self, p: [f32; 2]) -> bool {
        let v = self.verts();
        (0..v.len()).all(|i| {
            let (a, b) = (v[i], v[(i + 1) % v.len()]);
            if a == b {
                return true;
            }
            cross(sub(b, a), sub(p, a)) >= -1.0e-6
        })
    }
}

fn sub(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn cross(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[1] - a[1] * b[0]
}

fn polygon_area(v: &[[f32; 2]]) -> f32 {
    let n = v.len();
    if n < 3 {
        return 0.0;
    }
    0.5 * (0..n).map(|i| cross(v[i], v[(i + 1) % n])).sum::<f32>()
}

/// The convex hull of `pts`, counter-clockwise, collinear points dropped (Andrew's monotone chain).
fn convex_hull(mut pts: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    pts.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    pts.dedup();
    if pts.len() < 3 {
        return pts;
    }
    let mut lower: Vec<[f32; 2]> = Vec::new();
    for &p in &pts {
        while lower.len() >= 2 && cross(sub(lower[lower.len() - 1], lower[lower.len() - 2]), sub(p, lower[lower.len() - 2])) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<[f32; 2]> = Vec::new();
    for &p in pts.iter().rev() {
        while upper.len() >= 2 && cross(sub(upper[upper.len() - 1], upper[upper.len() - 2]), sub(p, upper[upper.len() - 2])) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

/// Where the line through `a0`-`a1` meets the line through `b0`-`b1`, or None when parallel.
fn meet(a0: [f32; 2], a1: [f32; 2], b0: [f32; 2], b1: [f32; 2]) -> Option<[f32; 2]> {
    let (da, db) = (sub(a1, a0), sub(b1, b0));
    let den = cross(da, db);
    if den.abs() < 1.0e-12 {
        return None;
    }
    let t = cross(sub(b0, a0), db) / den;
    Some([a0[0] + da[0] * t, a0[1] + da[1] * t])
}

/// Reduce a convex counter-clockwise polygon to at most `k` vertices, never giving up any of its
/// area and never leaving the unit square: remove the edge whose two neighbours, extended to meet,
/// add the least area, until `k` remain. Falls back to the bounding box when no edge can go.
fn reduce(mut p: Vec<[f32; 2]>, k: usize) -> Vec<[f32; 2]> {
    const EPS: f32 = 1.0e-6;
    while p.len() > k {
        let n = p.len();
        let mut best: Option<(f32, usize, [f32; 2])> = None;
        for i in 0..n {
            let (a0, a1, b0, b1) = (p[(i + n - 1) % n], p[i], p[(i + 1) % n], p[(i + 2) % n]);
            // The neighbours must turn by less than half a turn together, or they never meet
            // beyond the edge.
            if cross(sub(a1, a0), sub(b1, b0)) <= 1.0e-12 {
                continue;
            }
            let Some(x) = meet(a0, a1, b0, b1) else { continue };
            if x.iter().any(|&c| !(-EPS..=1.0 + EPS).contains(&c)) {
                continue;
            }
            let added = 0.5 * cross(sub(x, a1), sub(b0, a1)).abs();
            if best.is_none_or(|(b, _, _)| added < b) {
                best = Some((added, i, x));
            }
        }
        let Some((_, i, x)) = best else {
            // No edge can go without leaving the tile: the bounding box always can.
            let (mut lo, mut hi) = ([1.0f32; 2], [0.0f32; 2]);
            for q in &p {
                lo = [lo[0].min(q[0]), lo[1].min(q[1])];
                hi = [hi[0].max(q[0]), hi[1].max(q[1])];
            }
            return vec![lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]];
        };
        let x = [x[0].clamp(0.0, 1.0), x[1].clamp(0.0, 1.0)];
        // Vertices i and i+1 become their meeting point.
        let j = (i + 1) % n;
        p[i] = x;
        p.remove(j);
    }
    p
}

/// The outline of one baked tile: `alpha` is its alpha channel, one byte a texel, `px` texels
/// square, row 0 the tile's TOP (the frame's top edge, v 1). At most `k` vertices (3 to
/// [`MAX_VERTS`]), padded to exactly `k`. None when no texel is covered (the tile is empty).
pub fn fit(alpha: &[u8], px: u32, k: u8) -> Option<CardHull> {
    let px_us = px as usize;
    if px == 0 || alpha.len() < px_us * px_us {
        return None;
    }
    let k = k.clamp(3, MAX_VERTS as u8);
    let r = REACH_TEXELS / px as f32;
    let mut pts = Vec::new();
    for y in 0..px_us {
        let row = &alpha[y * px_us..(y + 1) * px_us];
        let (Some(xl), Some(xr)) = (row.iter().position(|&a| a > 0), row.iter().rposition(|&a| a > 0)) else { continue };
        let v = 1.0 - (y as f32 + 0.5) / px as f32;
        let ul = (xl as f32 + 0.5) / px as f32;
        let ur = (xr as f32 + 0.5) / px as f32;
        for (u, du) in [(ul, -r), (ur, r)] {
            for dv in [-r, r] {
                pts.push([(u + du).clamp(0.0, 1.0), (v + dv).clamp(0.0, 1.0)]);
            }
        }
    }
    if pts.is_empty() {
        return None;
    }
    let hull = reduce(convex_hull(pts), k as usize);
    let mut out = CardHull { n: hull.len().min(MAX_VERTS) as u8, uv: [[0.0; 2]; MAX_VERTS] };
    for (i, q) in hull.iter().take(MAX_VERTS).enumerate() {
        out.uv[i] = *q;
    }
    Some(out.padded(k))
}

/// The index pattern of one card face of `n` vertices: the fan from vertex 0, then the same fan
/// wound backwards (`n` 4 gives the quad's `[0,1,2, 0,2,3, 0,2,1, 0,3,2]`). The sun pass takes the
/// first half (one winding: it draws without face culling).
pub fn face_indices(n: u32) -> Vec<u32> {
    let t = n.saturating_sub(2);
    let mut out = Vec::with_capacity(6 * t as usize);
    for i in 0..t {
        out.extend_from_slice(&[0, i + 1, i + 2]);
    }
    for i in 0..t {
        out.extend_from_slice(&[0, i + 2, i + 1]);
    }
    out
}

// ── The table, filled by the bake ─────────────────────────────────────────

/// The showcase pin `card_hull`: 0 = the full square, else the fitted outline's vertex count.
static PIN: AtomicU8 = AtomicU8::new(DEFAULT_K);

/// The pin a requested count stands for: 0, or the smallest of [`FIT_KS`] at least `k`.
fn pin_for(k: u8) -> u8 {
    if k == 0 {
        0
    } else {
        FIT_KS.iter().copied().find(|&f| f >= k).unwrap_or(MAX_VERTS as u8)
    }
}

/// Set the showcase pin (0 = the square, the red twin; 4, 6 or 8 = a fitted outline).
pub fn set_pin(k: u8) {
    PIN.store(pin_for(k), Ordering::Relaxed);
}

/// The showcase pin (0 = the square).
pub fn pin() -> u8 {
    PIN.load(Ordering::Relaxed)
}

/// Per tile, the fitted outline for each of [`FIT_KS`], or None (not baked, or empty: see
/// [`Fitted`]).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fitted {
    /// Not baked (yet): the card keeps the square.
    Unbaked,
    /// Baked with no covered texel: the card draws nothing, as a point.
    Empty,
    /// Baked: the outline for each of [`FIT_KS`].
    Hulls([CardHull; FIT_KS.len()]),
}

fn table() -> &'static RwLock<[Fitted; ATLAS_TILES as usize]> {
    static T: std::sync::OnceLock<RwLock<[Fitted; ATLAS_TILES as usize]>> = std::sync::OnceLock::new();
    T.get_or_init(|| RwLock::new([Fitted::Unbaked; ATLAS_TILES as usize]))
}

/// Record a baked tile's alpha (`px` square, row 0 the top): its outlines for every [`FIT_KS`].
/// Returns the default outline's area as a share of the tile (0 for an empty tile), for the bake
/// log.
pub fn set_tile_alpha(tile: u32, alpha: &[u8], px: u32) -> f32 {
    if tile >= ATLAS_TILES {
        return 1.0;
    }
    let fitted = match fit(alpha, px, FIT_KS[0]) {
        None => Fitted::Empty,
        Some(first) => {
            let mut h = [first; FIT_KS.len()];
            for (i, &k) in FIT_KS.iter().enumerate().skip(1) {
                h[i] = fit(alpha, px, k).unwrap_or(first);
            }
            Fitted::Hulls(h)
        }
    };
    let area = match fitted {
        Fitted::Hulls(h) => FIT_KS.iter().position(|&k| k == DEFAULT_K).map_or(1.0, |i| h[i].area()),
        _ => 0.0,
    };
    let mut g = match table().write() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    g[tile as usize] = fitted;
    area
}

/// Every tile's outline under the current pin, every one with the SAME vertex count (the face
/// size of the patch being built): the square at 4 with the pin at 0, else each tile's fitted
/// outline at the pin's count (the square padded to it before the bake). Patch builds take ONE
/// snapshot and read it lock-free per card.
pub fn hull_table() -> [CardHull; ATLAS_TILES as usize] {
    let k = pin();
    if k == 0 {
        return [CardHull::SQUARE; ATLAS_TILES as usize];
    }
    let g = match table().read() {
        Ok(g) => *g,
        Err(p) => *p.into_inner(),
    };
    table_for(k, &g)
}

/// [`hull_table`] for pin `k` over the fitted table `g`.
fn table_for(k: u8, g: &[Fitted; ATLAS_TILES as usize]) -> [CardHull; ATLAS_TILES as usize] {
    if k == 0 {
        return [CardHull::SQUARE; ATLAS_TILES as usize];
    }
    let i = FIT_KS.iter().position(|&f| f == k).unwrap_or(FIT_KS.len() - 1);
    let mut out = [CardHull::SQUARE.padded(k); ATLAS_TILES as usize];
    for (o, f) in out.iter_mut().zip(g.iter()) {
        match f {
            Fitted::Unbaked => {}
            Fitted::Empty => *o = CardHull { n: k, uv: [[0.5, 0.0]; MAX_VERTS] },
            Fitted::Hulls(h) => *o = h[i],
        }
    }
    out
}

/// The mean area of the baked tiles' outlines under the current pin, as a share of the square
/// (1 with the pin at 0 or before the bake), for the tree card counter.
pub fn mean_area() -> f32 {
    let g = match table().read() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    let baked: Vec<usize> = (0..g.len()).filter(|&i| matches!(g[i], Fitted::Hulls(_))).collect();
    drop(g);
    if pin() == 0 || baked.is_empty() {
        return 1.0;
    }
    let t = hull_table();
    baked.iter().map(|&i| t[i].area()).sum::<f32>() / baked.len() as f32
}

// ── The emitter ───────────────────────────────────────────────────────────

/// Emit one face of a sprite tree card (v0.961; framed by the tile's footprint since v0.1083;
/// outlined by its hull since 2026-11-14): standing at `base`, `up` the local vertical, `side` the
/// face's horizontal direction, `h` the height this tree stands, `tile` its atlas tile, `fp` that
/// tile's footprint, `hull` its outline.
///
/// `v` spans the baked FRAME (square on the tree's larger extent), so the face is sized and
/// dropped from the footprint (`planet_chunks::sprite_card_frame`). Each vertex carries its tile
/// and u on the smooth uv channel (`color[1] = -((1 + tile) + u * 0.5)`, the shader's decode) and
/// v in `color[2]`; every vertex of a face is the same tile, so the interpolated u stays exact.
#[allow(clippy::too_many_arguments)]
pub fn emit_sprite_card(
    base: glam::Vec3,
    up: glam::Vec3,
    side: glam::Vec3,
    h: f32,
    tile: u32,
    fp: CardFootprint,
    hull: &CardHull,
    vertices: &mut Vec<SurfaceVertexData>,
    indices: &mut Vec<u32>,
) {
    let (s, drop_m) = crate::planet_chunks::sprite_card_frame(fp, h);
    let foot = base - up * drop_m;
    let nrm = up.to_array();
    let base_i = vertices.len() as u32;
    for &[u01, v01] in hull.verts() {
        let p = foot + up * (s * v01) + side * (s * (u01 - 0.5));
        let enc_x = -((1 + tile) as f32 + u01 * 0.5);
        vertices.push(SurfaceVertexData { position: p.to_array(), normal: nrm, color: [-1.0, enc_x, v01], water: false, tree_card: true });
    }
    for i in face_indices(hull.n as u32) {
        indices.push(base_i + i);
    }
}

#[cfg(test)]
#[path = "card_hull_tests.rs"]
mod tests;
