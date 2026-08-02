//! The DRAWN patch surface: the ground you can actually SEE.
//!
//! Promoted out of `terrain::grass` (v0.1097). It landed there in v0.1091
//! because grass was the first layer that had to stand ON the drawn mesh,
//! and that module carried a note saying it wanted promoting the day a
//! second client appeared. The day came: near-field TREES float above the
//! ground for exactly the reason grass tillers used to (operator field
//! report, screenshot, v0.1096 - "visible gaps at trunk bases, most on
//! slopes"), so the sampler is now a peer module both harvests import.
//!
//! Nothing in the move changed: `DrawnPatchSurface` and its `VertexMemo`
//! are byte-identical to what shipped in v0.1091, and `grass` re-exports
//! the type, so every `grass::DrawnPatchSurface` and
//! `planet_chunks::DrawnPatchSurface` path still resolves.
//!
//! WHAT IT IS FOR, in one sentence: `drawn_elevation_normalized` answers
//! "what does the elevation FIELD say here", which is NOT what the player
//! sees; this answers "where does the drawn TRIANGLE sit here", which is.

use glam::DVec3;

use super::planet::PlanetDef;
use super::planet_chunks::{
    child_corners, root_face_corners, smoothstep01, tile_or_base, ElevationSource,
    DETAIL_LAND_FADE_M, PATCH_TESS,
};
use super::planet_surface::{displaced_radius_f64, displaced_radius_f64_true};

// ── The DRAWN surface, exactly (v0.1091) ──────────────────────────────────
//
// `planet_chunks::drawn_elevation_normalized` answers "what does the
// elevation FIELD say here". That is NOT what you can see. What you see is a
// mesh: the
// resident patch samples that field at its own lattice (PATCH_TESS steps
// along each patch edge) and the rasteriser LINEARLY INTERPOLATES between
// those samples. The two answers differ by up to a whole quantization tread
// of the base sampler - measured at Fuji, 1.06 m at the 95th percentile,
// with 23.8% of grass tillers placed entirely underground - because the base
// heightmap sampler is an f32 staircase with ~1.4 m treads on a steep flank
// and the mesh's lattice is 0.84 m at depth 17.
//
// So anything that has to SIT ON the visible ground (a blade base, and in
// time the player's boots) must interpolate the drawn face, not re-sample
// the field. `DrawnPatchSurface` does exactly that, and it is exact at any
// depth and any slope by construction: same patch, same lattice, same
// elevation formula, same linear interpolation.
//
// The two costs that make it practical:
//   * LOCATING the patch is pure barycentric arithmetic. Radially projecting
//     the planar medial subdivision of a triangle gives exactly the spherical
//     child triangles the patch tree uses, so "which child contains this
//     direction" is a compare on three floats and the child's barycentrics
//     are an affine map of the parent's - no point-in-triangle tests, no
//     normalize per level.
//   * SAMPLING is memoized per lattice VERTEX. Neighbouring queries share
//     corners, so on a coarse patch (0.84 m lattice) a few thousand tillers
//     cost a few hundred elevation samples.

/// Lattice-vertex memo: a DIRECT-MAPPED cache, not a HashMap.
///
/// The access pattern is tiny and extremely local - a harvest touches a few
/// thousand distinct lattice vertices and asks for each of them a couple of
/// dozen times, in spatial order - so probing, tombstones and growth are all
/// dead weight. A collision simply overwrites and the loser is recomputed
/// later, which is correct because every entry is a pure function of its key.
///
/// The DIRECTION is cached beside the radius. It is a 3-scale, 2-add and a
/// square root, which turned out to cost as much as the lookup guarding it.
struct VertexMemo {
    slots: Vec<(u64, DVec3, f64)>,
    mask: usize,
}

impl VertexMemo {
    fn new() -> Self {
        // 65,536 slots = 3 MB. A 30 m harvest disc touches ~4,000 distinct
        // lattice vertices at depth 17 and ~50,000 at depth 20, so the table
        // is sized for the deep case and effectively collision-free.
        let n = 1 << 16;
        Self { slots: vec![(0, DVec3::ZERO, 0.0); n], mask: n - 1 }
    }
    #[inline]
    fn slot(&self, key: u64) -> usize {
        let mut h = key;
        h ^= h >> 33;
        h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
        h ^= h >> 29;
        (h as usize) & self.mask
    }
}

/// The radius of the DRAWN patch surface along a direction, at a given patch
/// tree depth. Reproduces `build_patch_mesh`'s geometry exactly: the same
/// lattice directions, the same depth-gated elevation, and the same flat
/// triangle between them.
///
/// Stateful because it memoizes; create one per harvest and reuse it.
pub struct DrawnPatchSurface<'a, 'b> {
    def: &'a PlanetDef,
    source: &'a ElevationSource<'b>,
    depth: u8,
    bathymetric: bool,
    /// Root icosahedron face of the last query (the fast path: a whole
    /// harvest disc is thousands of times smaller than a root face).
    face: u8,
    /// Child index taken at each level of the last query, and the patch
    /// corners AFTER that step. `corners[0]` is the root face itself, so
    /// `corners[l + 1]` is the patch at depth `l + 1`.
    path: Vec<u8>,
    corners: Vec<[DVec3; 3]>,
    /// Per level, the lengths |c0+c1|/2, |c1+c2|/2, |c2+c0|/2 of that
    /// level's UNNORMALIZED edge midpoints. The descent needs them to
    /// re-express a point's barycentrics in the child's basis (see the note
    /// in `radius_at`); they are a property of the level's corners, so they
    /// are cached beside them and recomputed only where the path diverges.
    mids: Vec<[f64; 3]>,
    /// Running patch-identity key per level, so the vertex memo's key does
    /// not have to fold the whole path on every query.
    pkeys: Vec<u64>,
    /// Deepest level whose patch is known to contain EVERY query (set by
    /// `set_region`). Levels below it are re-walked per query; levels above
    /// are fixed for the whole harvest, so a depth-20 lookup can start at
    /// level 13 and walk 7 levels instead of 20.
    base_level: usize,
    memo: VertexMemo,
    /// Count of elevation samples actually taken (diagnostics + tests).
    pub samples: usize,
}

impl<'a, 'b> DrawnPatchSurface<'a, 'b> {
    pub fn new(def: &'a PlanetDef, source: &'a ElevationSource<'b>, depth: u8) -> Self {
        let bathymetric = matches!(source, ElevationSource::Heightmap { ocean: Some(_), .. });
        Self {
            def,
            source,
            depth,
            bathymetric,
            face: u8::MAX,
            path: Vec::with_capacity(depth as usize),
            corners: Vec::with_capacity(depth as usize + 1),
            mids: Vec::with_capacity(depth as usize + 1),
            pkeys: Vec::with_capacity(depth as usize + 1),
            base_level: 0,
            memo: VertexMemo::new(),
            samples: 0,
        }
    }

    /// Pin the descent to the deepest patch that wholly contains a spherical
    /// cap, so every later query walks only the levels BELOW it.
    ///
    /// A harvest disc is ~30 m and a root face is ~7,000 km, so 12-14 of a
    /// depth-20 walk's levels are the same for every tiller in the harvest.
    /// Walking them 47,000 times is most of the cost of the walk. This runs
    /// the shared prefix ONCE.
    ///
    /// Safe by construction: if a later query somehow falls outside the
    /// pinned patch (a caller that changed centre without re-pinning), the
    /// containment test in `radius_at` fails and it re-walks from the root.
    pub fn set_region(&mut self, center: DVec3, ang_rad: f64) {
        self.face = u8::MAX;
        self.path.clear();
        self.corners.clear();
        self.mids.clear();
        self.pkeys.clear();
        self.base_level = 0;
        let c = center.normalize();
        let Some((f, root)) = root_face_corners()
            .iter()
            .enumerate()
            .find(|(_, rc)| Self::bary(rc, c).is_some())
        else {
            return;
        };
        let root = *root;
        if !Self::cap_inside(&root, c, ang_rad) {
            return;
        }
        self.face = f as u8;
        self.corners.push(root);
        self.mids.push(Self::mid_lens(&root));
        self.pkeys.push(f as u64);
        for _ in 0..self.depth as usize {
            let cur = self.corners[self.base_level];
            let kids = child_corners(&cur);
            let Some((ci, kid)) = kids
                .iter()
                .enumerate()
                .find(|(_, k)| Self::cap_inside(k, c, ang_rad))
            else {
                break;
            };
            let kid = *kid;
            self.corners.push(kid);
            self.mids.push(Self::mid_lens(&kid));
            self.pkeys.push(
                self.pkeys[self.base_level]
                    .wrapping_mul(4)
                    .wrapping_add(ci as u64 + 1),
            );
            self.path.push(ci as u8);
            self.base_level += 1;
        }
    }

    /// Is the spherical cap (`center`, `ang_rad`) wholly inside the spherical
    /// triangle `c`? Each edge is a great circle whose inward unit normal is
    /// the normalized cross product of its endpoints; the cap clears the edge
    /// when the centre is at least `sin(ang)` inside it.
    fn cap_inside(c: &[DVec3; 3], center: DVec3, ang_rad: f64) -> bool {
        let s = ang_rad.sin();
        for i in 0..3 {
            let n = c[i].cross(c[(i + 1) % 3]);
            let nl = n.length();
            if nl < 1e-18 {
                return false;
            }
            // Orient inward using the opposite corner.
            let n = if n.dot(c[(i + 2) % 3]) < 0.0 { -n / nl } else { n / nl };
            if n.dot(center) < s {
                return false;
            }
        }
        true
    }

    /// |c0+c1|/2, |c1+c2|/2, |c2+c0|/2 - the lengths the edge midpoints have
    /// BEFORE `midpoint()` normalizes them.
    #[inline]
    fn mid_lens(c: &[DVec3; 3]) -> [f64; 3] {
        [
            (c[0] + c[1]).length() * 0.5,
            (c[1] + c[2]).length() * 0.5,
            (c[2] + c[0]).length() * 0.5,
        ]
    }

    /// Barycentric weights of `dir` in the spherical triangle `c`, normalized
    /// to sum 1, or None when `dir` is not inside it.
    ///
    /// THE SIGN OF THE SUM IS PART OF THE TEST, not a normalisation detail.
    /// The root faces are CCW from outside, so the scalar triple product is
    /// positive and a contained direction has three positive raw weights. The
    /// ANTIPODAL face has three NEGATIVE raw weights - and dividing three
    /// negatives by their negative sum yields three positives, so a test that
    /// only checks the normalised signs accepts the face on the far side of
    /// the planet. That is not theoretical: it picked the wrong root face at
    /// Amazon (but not at Fuji, which is why it looked like a depth-20 bug)
    /// and returned a radius of exactly -R, the far-side plane crossing.
    #[inline]
    fn bary(c: &[DVec3; 3], dir: DVec3) -> Option<[f64; 3]> {
        let b0 = dir.dot(c[1].cross(c[2]));
        let b1 = dir.dot(c[2].cross(c[0]));
        let b2 = dir.dot(c[0].cross(c[1]));
        if b0 < 0.0 || b1 < 0.0 || b2 < 0.0 {
            return None;
        }
        let s = b0 + b1 + b2;
        if s < 1e-300 {
            return None;
        }
        Some([b0 / s, b1 / s, b2 / s])
    }

    /// The elevation formula `build_patch_mesh` uses, at THIS depth.
    fn mesh_elevation(&self, dir: DVec3) -> f32 {
        match self.source {
            ElevationSource::Heightmap { hm, detail, tiles, .. } => {
                let (base, from_tile) = tile_or_base(hm, *tiles, dir, self.depth);
                let range_m = hm.max_meters() - hm.min_meters();
                if range_m <= 0.0 {
                    return base.clamp(0.0, 1.0);
                }
                let sea = self.def.sea_level.clamp(0.0, 1.0);
                let above_sea_m = (base - sea) * range_m;
                let mask = smoothstep01(above_sea_m / DETAIL_LAND_FADE_M);
                let e = if mask > 0.0 {
                    let dm = if from_tile {
                        detail.sample_m_tile_gated(dir, self.depth)
                    } else {
                        detail.sample_m(dir, self.depth)
                    };
                    base + (dm * mask) / range_m
                } else {
                    base
                };
                e.clamp(0.0, 1.0)
            }
            ElevationSource::Noise(s) => s.elevation_at(dir.as_vec3()),
        }
    }

    /// Planet-local radius in metres of the mesh vertex at lattice (r, c) of
    /// the patch whose corners are `c3` and whose identity key is `pkey`.
    fn vertex(&mut self, pkey: u64, c3: &[DVec3; 3], r: u32, cc: u32) -> (DVec3, f64) {
        let n = PATCH_TESS;
        let w0 = (n - r) as f64;
        let w1 = (r - cc) as f64;
        let w2 = cc as f64;
        // The key is unique per (patch, lattice node), and non-zero so slot
        // 0 can never be mistaken for a filled entry.
        let key = pkey
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ ((r as u64) << 8)
            ^ (cc as u64)
            ^ 0x2545_F491_4F6C_DD1D;
        let si = self.memo.slot(key);
        {
            let e = &self.memo.slots[si];
            if e.0 == key {
                return (e.1, e.2);
            }
        }
        // Bit-identical to build_patch_mesh vertex direction: same integer
        // weights, same product-and-sum order, same normalize.
        let dir = (c3[0] * w0 + c3[1] * w1 + c3[2] * w2).normalize();
        let e = self.mesh_elevation(dir);
        let r_m = self.def.radius
            * if self.bathymetric {
                displaced_radius_f64_true(self.def, e as f64)
            } else {
                displaced_radius_f64(self.def, e as f64)
            };
        self.memo.slots[si] = (key, dir, r_m);
        self.samples += 1;
        (dir, r_m)
    }

    /// Radius (planet-local metres from the planet centre) at which a ray
    /// along `dir` leaves the drawn patch surface.
    pub fn radius_at(&mut self, dir: DVec3) -> f64 {
        let d = dir.normalize();
        // ── Entry level ──
        // Normally the pinned region (set_region) - the deepest patch that
        // contains every query in this harvest, so the walk starts there.
        // Falls back to a full root walk when nothing is pinned or when a
        // query lands outside the pinned patch.
        let mut b = [0.0f64; 3];
        let mut start = self.base_level;
        let mut found = false;
        if start > 0 && start < self.corners.len() {
            if let Some(t) = Self::bary(&self.corners[start], d) {
                b = t;
                found = true;
            }
        }
        if !found {
            start = 0;
            if self.face != u8::MAX {
                let c = root_face_corners()[self.face as usize];
                if let Some(t) = Self::bary(&c, d) {
                    b = t;
                    found = true;
                }
            }
            if !found {
                for (f, c) in root_face_corners().iter().enumerate() {
                    if let Some(t) = Self::bary(c, d) {
                        b = t;
                        self.face = f as u8;
                        self.path.clear();
                        self.corners.clear();
                        self.mids.clear();
                        self.pkeys.clear();
                        self.base_level = 0;
                        found = true;
                        break;
                    }
                }
                if !found {
                    // Numerically on an edge of every face: fall back to the
                    // field sample rather than returning nonsense.
                    return self.def.radius
                        * displaced_radius_f64(self.def, self.mesh_elevation(d) as f64);
                }
            }
            if self.corners.is_empty() {
                let c = root_face_corners()[self.face as usize];
                self.corners.push(c);
                self.mids.push(Self::mid_lens(&c));
                self.pkeys.push(self.face as u64);
            }
        }
        // ── Descend ──
        //
        // Radially projecting the PLANAR medial subdivision of a triangle
        // gives exactly the spherical children `patch_corners` builds: an
        // edge midpoint normalizes onto the same ray as the planar midpoint,
        // and a great-circle edge is the projection of the straight one. So
        // "which child contains this direction" is `b[i] >= 0.5` on the
        // parent's barycentrics - three compares, no point-in-triangle test.
        //
        // RE-EXPRESSING the barycentrics in the child's basis is where this
        // gets subtle, and getting it wrong is silent. Write P in the child's
        // UNNORMALIZED basis first (for child 0, that is c0 and the raw
        // midpoints (c0+c1)/2 and (c2+c0)/2):
        //
        //   P = (b0-b1-b2) c0  +  2*b1 * (c0+c1)/2  +  2*b2 * (c2+c0)/2
        //
        // then convert each raw midpoint to the UNIT corner the patch tree
        // actually uses, which scales that coefficient by the midpoint's
        // length. Skipping that scaling looks right (the coefficients still
        // sum to 1 and the child selection still works) and quietly drifts the
        // lattice position: measured 138 m of error at Fuji depth 17, because
        // a root-face midpoint is only 0.851 long and the distortion compounds
        // down the first few levels.
        let depth = self.depth as usize;
        for level in start..depth {
            let m = self.mids[level];
            let (child, nb): (u8, [f64; 3]) = if b[0] >= 0.5 {
                (0, [b[0] - b[1] - b[2], 2.0 * b[1] * m[0], 2.0 * b[2] * m[2]])
            } else if b[1] >= 0.5 {
                (1, [b[1] - b[2] - b[0], 2.0 * b[2] * m[1], 2.0 * b[0] * m[0]])
            } else if b[2] >= 0.5 {
                (2, [b[2] - b[0] - b[1], 2.0 * b[0] * m[2], 2.0 * b[1] * m[1]])
            } else {
                (
                    3,
                    [
                        (1.0 - 2.0 * b[2]) * m[0],
                        (1.0 - 2.0 * b[0]) * m[1],
                        (1.0 - 2.0 * b[1]) * m[2],
                    ],
                )
            };
            let s = nb[0] + nb[1] + nb[2];
            b = if s.abs() < 1e-300 {
                [1.0, 0.0, 0.0]
            } else {
                [nb[0] / s, nb[1] / s, nb[2] / s]
            };
            // Extend / repair the cached corner chain. Levels whose child is
            // unchanged keep their corners, midpoint lengths and patch key,
            // which is why a harvest disc metres wide only ever pays for its
            // last few levels: the prefix is shared by every query in it.
            let want = level + 1;
            let stale = self.path.len() <= level
                || self.path[level] != child
                || self.corners.len() <= want;
            if stale {
                self.path.truncate(level);
                self.corners.truncate(want);
                self.mids.truncate(want);
                self.pkeys.truncate(want);
                let kid = child_corners(&self.corners[level])[child as usize];
                self.corners.push(kid);
                self.mids.push(Self::mid_lens(&kid));
                self.pkeys
                    .push(self.pkeys[level].wrapping_mul(4).wrapping_add(child as u64 + 1));
                self.path.push(child);
            }
        }
        self.path.truncate(depth);
        self.corners.truncate(depth + 1);
        self.mids.truncate(depth + 1);
        self.pkeys.truncate(depth + 1);
        let c3 = self.corners[depth];
        let pkey = self.pkeys[depth];
        // ── Lattice cell ──
        // Vertex (r, c) carries integer weights (n-r, r-c, c), so
        // r = n*(1-b0) and c = n*b2.
        let n = PATCH_TESS;
        let nf = n as f64;
        let v = ((1.0 - b[0]) * nf).clamp(0.0, nf);
        let u = (b[2] * nf).clamp(0.0, v);
        let ri = (v.floor() as u32).min(n - 1);
        let ci = (u.floor() as u32).min(ri);
        let fv = v - ri as f64;
        let fu = u - ci as f64;
        // Upward cell (fu <= fv) or downward cell - the same two triangles
        // build_patch_mesh emits per lattice square.
        let tri = if fu <= fv {
            [(ri, ci), (ri + 1, ci), (ri + 1, ci + 1)]
        } else {
            [(ri, ci), (ri + 1, ci + 1), (ri, ci + 1)]
        };
        let (d0, r0) = self.vertex(pkey, &c3, tri[0].0, tri[0].1);
        let (d1, r1) = self.vertex(pkey, &c3, tri[1].0, tri[1].1);
        let (d2, r2) = self.vertex(pkey, &c3, tri[2].0, tri[2].1);
        let (p0, p1, p2) = (d0 * r0, d1 * r1, d2 * r2);
        // Where the ray leaves the drawn FACE. Plane intersection rather than
        // a weighted mean of the three radii: the face is what rasterises.
        let nrm = (p1 - p0).cross(p2 - p0);
        let den = d.dot(nrm);
        if den.abs() < 1e-9 {
            return (r0 + r1 + r2) / 3.0;
        }
        p0.dot(nrm) / den
    }
}

/// The same answer as [`DrawnPatchSurface::radius_at`], for a caller that is
/// ALREADY INSIDE the patch and holding its lattice: where a ray from the
/// planet centre along `dir` leaves the drawn face.
///
/// This exists for `build_patch_mesh`'s vegetation pass (v0.1097). That pass
/// plants cards into the very mesh it is building, so it needs no patch walk,
/// no memo and no second elevation sample - it has the finished lattice in
/// hand. What it was doing instead was taking a FRESH direct sample of the
/// elevation field (`tile_or_base`, and with no `DetailNoise` term at all,
/// unlike the vertices beside it), which is the same "the field is not the
/// mesh" defect that had grass tillers buried and near-field tree models
/// hovering. Same patch, same lattice, same triangle: exact by construction.
///
/// `vert_dirs` / `vert_radii` are indexed the way `build_patch_mesh` lays its
/// triangular grid out, row r column c at `r*(r+1)/2 + c` (its `grid_idx`).
/// Returns None when `dir` is outside the patch.
pub fn patch_lattice_radius(
    corners: &[DVec3; 3],
    vert_dirs: &[DVec3],
    vert_radii: &[f64],
    dir: DVec3,
) -> Option<f64> {
    let d = dir.normalize();
    let b = DrawnPatchSurface::bary(corners, d)?;
    let n = PATCH_TESS;
    let nf = n as f64;
    // Vertex (r, c) carries integer weights (n-r, r-c, c), so r = n*(1-b0)
    // and c = n*b2 - the inverse of the mesh's own vertex placement.
    let v = ((1.0 - b[0]) * nf).clamp(0.0, nf);
    let u = (b[2] * nf).clamp(0.0, v);
    let ri = (v.floor() as u32).min(n - 1);
    let ci = (u.floor() as u32).min(ri);
    let (fv, fu) = (v - ri as f64, u - ci as f64);
    // Upward cell (fu <= fv) or downward cell: the same two triangles
    // build_patch_mesh emits per lattice square.
    let tri = if fu <= fv {
        [(ri, ci), (ri + 1, ci), (ri + 1, ci + 1)]
    } else {
        [(ri, ci), (ri + 1, ci + 1), (ri, ci + 1)]
    };
    let idx = |(r, c): (u32, u32)| -> usize { (r * (r + 1) / 2 + c) as usize };
    let mut p = [DVec3::ZERO; 3];
    for (i, t) in tri.iter().enumerate() {
        let k = idx(*t);
        p[i] = *vert_dirs.get(k)? * *vert_radii.get(k)?;
    }
    // Plane intersection, not a weighted mean of the three radii: the face is
    // what rasterises.
    let nrm = (p[1] - p[0]).cross(p[2] - p[0]);
    let den = d.dot(nrm);
    if den.abs() < 1e-9 {
        return Some((p[0].length() + p[1].length() + p[2].length()) / 3.0);
    }
    Some(p[0].dot(nrm) / den)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::planet_albedo::PlanetAlbedo;
    use super::super::planet_chunks::{
        build_patch_mesh, near_tree_instances, near_tree_instances_on_drawn, patch_corners,
        patch_edge_arc_m, tests::earth_like, tree_flare_radius_m, DetailNoise, PatchId, PatchMesh,
        TREE_GROUND_SINK_FLARE_FRAC,
    };
    use super::super::planet_heightmap::PlanetHeightmap;
    use std::collections::HashMap;

    /// The shipped Earth grids plus a def carrying the real sea level - the
    /// same fixture every grass and tree test runs against, because the whole
    /// point of these layers is that they stand where the real data says.
    fn real_earth() -> (PlanetHeightmap, PlanetAlbedo, PlanetDef) {
        let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data").join("planets");
        let hm = PlanetHeightmap::load(&base.join("earth_heightmap.bin"))
            .expect("earth heightmap loads");
        let albedo =
            PlanetAlbedo::load(&base.join("earth_albedo.bin")).expect("earth albedo loads");
        let mut def = earth_like();
        def.sea_level = hm.sea_level_normalized();
        (hm, albedo, def)
    }

    fn dir_of(lat_deg: f64, lon_deg: f64) -> DVec3 {
        let (lat, lon) = (lat_deg.to_radians(), lon_deg.to_radians());
        let cl = lat.cos();
        DVec3::new(cl * lon.cos(), lat.sin(), -cl * lon.sin())
    }

    /// The patch of `depth` whose spherical triangle contains `dir`.
    fn patch_containing(dir: DVec3, depth: u8) -> PatchId {
        let inside = |id: &PatchId, d: DVec3| -> bool {
            let c = patch_corners(id);
            let cn = [c[0].normalize(), c[1].normalize(), c[2].normalize()];
            let en = [cn[0].cross(cn[1]), cn[1].cross(cn[2]), cn[2].cross(cn[0])];
            let es = [en[0].dot(cn[2]), en[1].dot(cn[0]), en[2].dot(cn[1])];
            (0..3).all(|i| en[i].dot(d) * es[i] >= 0.0)
        };
        let d = dir.normalize();
        let mut id = (0..20u8)
            .map(PatchId::root)
            .find(|r| inside(r, d))
            .expect("some root face contains the direction");
        while id.depth < depth {
            let mut next = None;
            for c in 0..4u32 {
                let ch = id.child(c);
                if inside(&ch, d) {
                    next = Some(ch);
                    break;
                }
            }
            id = next.expect("some child contains the direction");
        }
        id
    }

    /// Where a ray from the planet centre along `dir` crosses the built
    /// patch's GROUND, in metres of radius. Moller-Trumbore over the first
    /// `PATCH_TESS^2` triangles only: the vegetation cards and the skirt come
    /// after them in the index buffer, and the skirt is a near-RADIAL apron
    /// that a centre-outward ray grazes at an arbitrary distance (it produced
    /// 0.6-1.5 m phantom readings on the grass twin before it was excluded).
    fn drawn_radius_along(pm: &PatchMesh, dir: DVec3) -> Option<f64> {
        let d = dir.normalize();
        let v = |i: u32| -> DVec3 {
            let p = pm.mesh.vertices[i as usize].position;
            pm.anchor + DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)
        };
        let grid_idx = (PATCH_TESS * PATCH_TESS * 3) as usize;
        let mut best: Option<f64> = None;
        for tri in pm.mesh.indices[..grid_idx].chunks_exact(3) {
            let (a, b, c) = (v(tri[0]), v(tri[1]), v(tri[2]));
            let (e1, e2) = (b - a, c - a);
            let h = d.cross(e2);
            let det = e1.dot(h);
            if det.abs() < 1e-12 {
                continue;
            }
            let inv = 1.0 / det;
            let s = -a;
            let u = inv * s.dot(h);
            if !(-1e-9..=1.0 + 1e-9).contains(&u) {
                continue;
            }
            let q = s.cross(e1);
            let w = inv * d.dot(q);
            if w < -1e-9 || u + w > 1.0 + 1e-9 {
                continue;
            }
            let t = inv * e2.dot(q);
            if t > 0.0 && best.map_or(true, |bt| t > bt) {
                best = Some(t);
            }
        }
        best
    }

    /// Every offset a harvest's trees have from the ground actually drawn
    /// under them, in metres (+ floating, - buried), measured by ray-casting
    /// each trunk base against the REAL built patch mesh at `depth`. Returns
    /// (offsets, flare radii) so a caller can hold burial to a fraction of the
    /// tree's own root flare rather than to one number for a 4 m sapling and
    /// an 18 m conifer alike.
    ///
    /// A tree disc is hundreds of metres across and a depth-17 patch is 54 m,
    /// so unlike the grass twin this spans MANY patches - they are built once
    /// each and cached, and the probe stops at `max_patches` so a debug-build
    /// test run stays in seconds.
    fn base_offsets(
        def: &PlanetDef,
        src: &ElevationSource,
        albedo: &PlanetAlbedo,
        center: DVec3,
        depth: u8,
        radius_m: f64,
        max_patches: usize,
    ) -> (Vec<f64>, Vec<f64>) {
        let trees =
            near_tree_instances_on_drawn(def, src, Some(albedo), center, radius_m, depth, 600);
        assert!(
            trees.len() >= 30,
            "only {} trees harvested - the gates, not the ground, are what this test would be \
             measuring",
            trees.len()
        );
        let mut meshes: HashMap<PatchId, PatchMesh> = HashMap::new();
        let (mut offs, mut flares) = (Vec::new(), Vec::new());
        for t in &trees {
            let id = patch_containing(t.dir, depth);
            if !meshes.contains_key(&id) {
                if meshes.len() >= max_patches {
                    continue;
                }
                meshes.insert(id, build_patch_mesh(def, src, Some(albedo), &id));
            }
            if let Some(r) = drawn_radius_along(&meshes[&id], t.dir) {
                offs.push(t.r_m - r);
                flares.push(tree_flare_radius_m(t.height_m));
            }
        }
        (offs, flares)
    }

    /// THE GROUND-CONTACT GATE FOR TREES (v0.1097). The twin of
    /// `grass::tests::grass_bases_sit_on_the_drawn_surface`, and it exists for
    /// the same reason at the other end of the scale: the operator
    /// photographed near-field trees HOVERING over their slopes, with daylight
    /// under the trunks.
    ///
    /// A tree is 4-18 m tall, so unlike a 30 cm tiller it cannot be buried by
    /// the disagreement - it floats instead, and a metre of float on a 6 m
    /// sapling is unmissable. Same root cause either way: the base came from a
    /// direct sample of the elevation FIELD (worse here - at a FIXED depth 20
    /// and with no detail-noise term at all) while the ground you see is a
    /// MESH that lerps between lattice samples 3.36 m apart at depth 17.
    ///
    /// Measured against the real built patch mesh at the depth the LOD
    /// selector really reaches at each site, never against the field the tree
    /// came from.
    #[test]
    fn tree_bases_sit_on_the_drawn_surface() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        // (name, lat, lon, depth, harvest radius, patch budget). Fuji at 17 is
        // the slope case the report came from; the Amazon floodplain reaches
        // 20, where a patch is 6.7 m across and each one holds about one tree.
        let sites = [
            ("fuji", 35.29_f64, 138.79_f64, 17u8, 240.0_f64, 48usize),
            ("amazon", -3.0, -60.0, 20, 90.0, 64),
        ];
        for (name, lat, lon, depth, radius_m, max_patches) in sites {
            let center = dir_of(lat, lon);
            let (offs, flares) =
                base_offsets(&def, &src, &albedo, center, depth, radius_m, max_patches);
            assert!(
                offs.len() >= 30,
                "{name}: only {} trees hit a depth-{depth} patch",
                offs.len()
            );
            let mut sorted = offs.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
            let med = sorted[sorted.len() / 2];
            let p95 = sorted[sorted.len() * 95 / 100];
            // FLOATING is the failure the operator can see: daylight under the
            // trunk. Held per tree, not as a percentile, because one hovering
            // tree in a stand is exactly what the screenshot showed.
            let floating = offs.iter().filter(|o| **o > 0.15).count();
            // BURIED is held against the tree's OWN root flare: past half of
            // it the buttress stops reading and the trunk becomes a post stuck
            // in the dirt.
            let buried = offs
                .iter()
                .zip(&flares)
                .filter(|(o, f)| **o < -(*f * 0.5))
                .count();
            println!(
                "[tree ground] {name} depth {depth} ({:.2} m triangles): n={} \
                 mean {mean:+.3}  median {med:+.3}  p95 {p95:+.3}  max {:+.3}  min {:+.3}  \
                 floating>0.15m {}  buried-past-half-flare {}",
                patch_edge_arc_m(depth, def.radius) / PATCH_TESS as f64,
                sorted.len(),
                sorted.last().unwrap(),
                sorted[0],
                floating,
                buried
            );
            assert!(
                p95 < 0.30,
                "{name} (depth {depth}): 5% of trees sit more than {p95:.3} m off the drawn \
                 ground (median {med:+.3}) - the base is no longer coming from the drawn patch \
                 face, or the sink ({TREE_GROUND_SINK_FLARE_FRAC} of the flare) drifted"
            );
            assert_eq!(
                floating, 0,
                "{name} (depth {depth}): {floating} of {} trees float more than 0.15 m above the \
                 drawn ground - that is the operator's screenshot, reproduced",
                sorted.len()
            );
            assert_eq!(
                buried, 0,
                "{name} (depth {depth}): {buried} of {} trees are sunk past half their root \
                 flare - the sink is eating the buttress it was meant to hide",
                sorted.len()
            );
        }
    }

    /// THE CARD-EMITTER GATE (v0.1097). The mid-field tree CARDS are emitted
    /// inside `build_patch_mesh`, so they were assumed correct by
    /// construction - "they are built INTO the patch mesh". They were not.
    /// The vegetation pass took a FRESH `tile_or_base` sample of the elevation
    /// field, with no `DetailNoise` term, while every grid vertex beside it
    /// carries `base + detail * land_mask`: cards were planted on a different
    /// surface from the triangles they are stitched into.
    ///
    /// The emitter now calls [`patch_lattice_radius`] with the patch's own
    /// lattice, so this gate holds that function against the REAL built mesh
    /// (both halves, exactly like the grass reference twin): interpolated must
    /// be the mesh to within a centimetre, and the direct sample the emitter
    /// used to take must be provably far off, or the machinery is pointless.
    #[test]
    fn card_bases_use_the_patch_lattice_not_a_fresh_field_sample() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        let center = dir_of(35.29, 138.79);
        let depth = 17u8;
        let id = patch_containing(center, depth);
        let pm = build_patch_mesh(&def, &src, Some(&albedo), &id);
        // The lattice the emitter now interpolates, rebuilt exactly as
        // build_patch_mesh builds it (this IS the contract being tested).
        let corners = patch_corners(&id);
        let n = PATCH_TESS;
        let (mut vdirs, mut vradii) = (Vec::new(), Vec::new());
        let mut surf = DrawnPatchSurface::new(&def, &src, depth);
        for r in 0..=n {
            for c in 0..=r {
                let (w0, w1, w2) = ((n - r) as f64, (r - c) as f64, c as f64);
                let d = (corners[0] * w0 + corners[1] * w1 + corners[2] * w2).normalize();
                vdirs.push(d);
                vradii.push(surf.radius_at(d));
            }
        }
        // A 40 m transect across the patch at tree spacing.
        let east = DVec3::new(-center.z, 0.0, center.x).normalize();
        let (mut worst_interp, mut worst_direct, mut n_probe) = (0.0f64, 0.0f64, 0);
        for i in -20..20 {
            let dir = (center + east * (i as f64 * 2.0 / def.radius)).normalize();
            let Some(mesh_r) = drawn_radius_along(&pm, dir) else { continue };
            let Some(lat_r) = patch_lattice_radius(&corners, &vdirs, &vradii, dir) else {
                continue;
            };
            let (e, _) = tile_or_base(&hm, None, dir, depth);
            let direct_r = def.radius * displaced_radius_f64(&def, e as f64);
            worst_interp = worst_interp.max((lat_r - mesh_r).abs());
            worst_direct = worst_direct.max((direct_r - mesh_r).abs());
            n_probe += 1;
        }
        println!(
            "[card ground] fuji depth {depth}: worst |lattice - mesh| {worst_interp:.4} m, \
             worst |direct field sample - mesh| {worst_direct:.3} m over {n_probe} probes"
        );
        assert!(n_probe >= 20, "only {n_probe} transect probes hit the patch");
        assert!(
            worst_interp < 0.01,
            "patch_lattice_radius is {worst_interp:.4} m off the mesh it interpolates - its \
             lattice indexing or sub-triangle pick has drifted from build_patch_mesh"
        );
        assert!(
            worst_direct > 0.5,
            "a direct field sample is only {worst_direct:.3} m off the drawn mesh here, so the \
             defect this fix exists for is gone - re-derive the note before simplifying"
        );
    }

    /// The fix is the REFERENCE SURFACE, not a tuned offset, so prove the two
    /// references really do disagree on the ground trees stand on - and by how
    /// much. Without this, a later "simplification" back to a direct elevation
    /// sample would only turn the gate above red with no explanation.
    ///
    /// Runs the SAME harvest twice at one site: once unwired (the v0.1096
    /// path, a direct depth-20 `tile_or_base` sample with no detail term) and
    /// once on the drawn surface. Same stream, same gates, same trees - the
    /// only difference is where their bases sit.
    #[test]
    fn tree_bases_on_the_drawn_surface_beat_a_direct_elevation_sample() {
        let (hm, albedo, def) = real_earth();
        let detail = DetailNoise::new(def.terrain_seed);
        let src = ElevationSource::Heightmap {
            hm: &hm,
            detail: &detail,
            tiles: None,
            ocean: None,
        };
        let center = dir_of(35.29, 138.79);
        let depth = 17u8;
        let (drawn, _) = base_offsets(&def, &src, &albedo, center, depth, 240.0, 48);
        // The unwired path, measured against the same meshes.
        let trees = near_tree_instances(&def, &src, Some(&albedo), center, 240.0, 600);
        let mut meshes: HashMap<PatchId, PatchMesh> = HashMap::new();
        let mut direct: Vec<f64> = Vec::new();
        for t in &trees {
            let id = patch_containing(t.dir, depth);
            if !meshes.contains_key(&id) {
                if meshes.len() >= 48 {
                    continue;
                }
                meshes.insert(id, build_patch_mesh(&def, &src, Some(&albedo), &id));
            }
            if let Some(r) = drawn_radius_along(&meshes[&id], t.dir) {
                direct.push(t.r_m - r);
            }
        }
        let worst = |v: &[f64]| v.iter().fold(0.0f64, |m, o| m.max(o.abs()));
        let (wd, wi) = (worst(&direct), worst(&drawn));
        println!(
            "[tree ground ref] fuji depth {depth}: worst |direct - mesh| {wd:.3} m over {} trees, \
             worst |drawn-surface - mesh| {wi:.3} m over {} trees",
            direct.len(),
            drawn.len()
        );
        assert!(
            direct.len() >= 30 && drawn.len() >= 30,
            "too few probes ({} direct, {} drawn) to compare",
            direct.len(),
            drawn.len()
        );
        assert!(
            wi < 0.30,
            "the drawn-surface base is {wi:.3} m off the mesh it is supposed to BE - the lattice \
             walk or the sub-triangle pick has drifted from build_patch_mesh"
        );
        assert!(
            wd > 0.5,
            "a direct elevation sample is only {wd:.3} m off the drawn mesh here, so the \
             coarse-mesh staircase this machinery exists for is gone - re-derive the note on \
             DrawnPatchSurface before deleting anything"
        );
    }
}
