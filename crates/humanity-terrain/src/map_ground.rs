//! THE MAP GROUND: the installed whole-Earth map packs, drawn into the one ground formula (rung 2,
//! 2026-11-09; docs/design/whole-earth-maps.md sections 3.6, 4, 5 and 9.5).
//!
//! The operator, 2026-11-08: "We did have a problem where the heightmap of the world and the
//! Silverdale waterfront area wasn't even. Like in the water front initially didn't have any water
//! and then we had weird sheer cliffs along water area instead of nice slopes like in real life."
//!
//! Inside an installed pack the ground is no longer the 5.5 km grid with a carve pressed into it:
//! - **Land heights** come from GEDTM30 on the pack's nested lattices: a patch of depth d reads the
//!   level `level_for_depth(d)` tile, and its vertices ARE that lattice's samples (section 4.2).
//! - **Who owns a point** is OpenStreetMap's water polygons (`map_pack::point_class`, exact), and the
//!   distance to the TRUE shoreline (never a tile border: clip edges are skipped) shapes the shore.
//! - **The shore band** (section 5.2) joins land and water in one function of that distance:
//!   within a band the shore is one straight slope through the waterline, from the datum (OSM's
//!   coastline is a high-water line) down to the wet guarantee's depth; the band is at least a
//!   vertex spacing of the patch being drawn wider than the wet distance, so even a coarse patch's
//!   flat triangles draw water at the guarantee (a straight slope survives linear interpolation).
//!   Past it the land returns to its own heights (cliffs stay cliffs) and the water to the
//!   equilibrium beach profile `h = datum - A x^(2/3)`, blending into ETOPO's seabed over one
//!   bathymetry cell. Lakes keep a flat surface at the median of the land around them.
//! - Where a pack's neighbour is not installed, its ground fades into the base heights over
//!   `edge_blend_m` inside its edge, until the downloader brings the neighbour.
//!
//! It is applied INSIDE `planet_chunks::drawn_elevation_at_depth`, the one formula every consumer
//! calls (the mesher, the walk clamp, trees, grass, the water shell's depth bake), so it cannot
//! become BUG-156 again (section 5.4). The joining happens here at run time, never in the files,
//! so a pack stays a Collective Database (section 3.4).
//!
//! RESIDENCY (section 3.6): packs are read whole at world entry (installed data, like the star
//! tiers); what is decoded from them is kept per tile in slots, decoded on first use by whichever
//! thread asks, and dropped when no patch of the current selection has needed it for a while
//! ([`MapSet::tick`]). The shore field (the true shoreline's segments and each level-12 tile's
//! polygons) is built per pack on first use. All of it is reuse kept across frames, tagged by the
//! pack's build id, and obeys the Dev page's "turn off all reuse" switch: with it off the engine
//! publishes a fresh generation every frame ([`MapSet::fresh_generation`]), so everything is
//! decoded again (slow, on purpose: it is the switch that rules a stale-data bug in or out).
//!
//! f64 throughout (CLAUDE.md, "f32 at planet scale is a defect class"); the shore field's plane is
//! the pack's own gnomonic projection in metres, where every great-circle edge is straight.
//!
//! ROAD BEDS AND BUILDING PADS (the rung between 2 and 3, 2026-11-09, sections 6.1 to 6.3) shape
//! the land side inside the same formula, before the shore band: `beds` (map_beds.rs) has the
//! recipe, the identity rule across tile borders and the precedence (the coast wins within its
//! band). `HUMANITY_GROUND_BEDS=0` turns them off, with the mesher's rung-2 lifts and skirt: the
//! gate's red run, never in play.

use crate::map_levels::MapLevels;
use crate::map_pack::{ChunkKind, Pack, WaterRingKind};
use crate::map_tiles::{child_lattice_rc, e7_to_dir, lattice_index, level_for_depth};
use crate::planet_chunks::{child_corners, patch_corners, patch_edge_arc_m, vertex_spacing_m, PatchId};
use glam::DVec3;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

/// The planet radius the packs' metric figures use (they are Earth's).
const R: f64 = 6_371_000.0;
/// The patch depth the vegetation gates read the map ground at (the finest), the same for the
/// patch's tree cards, the near-tree models, far trees and grass, so the streams agree.
pub const GATE_DEPTH: u8 = 24;
/// The shore field's grid cell, metres.
const GRID_M: f64 = 200.0;
/// How far a point is moved into its level-12 tile before the water test (metres). A point ON a
/// tile border (every drawn vertex there: the lattice is nested) sits exactly on the edges of the
/// rings cut at that border, whose corners were rounded to OSM's 1e-7 degree integers (up to about
/// 0.8 cm off the true border), so the even-odd test read half of them as outside: land in the open
/// sea, a ridge to the surface along every tile border. Five centimetres clears the rounding and
/// moves no shoreline that matters.
const EDGE_NUDGE_M: f64 = 0.05;
/// A slot no patch has touched for this many frames is dropped.
const EVICT_FRAMES: u64 = 600;

/// Road beds and building pads (sections 6.1 to 6.3).
#[path = "map_beds.rs"]
pub(crate) mod beds;

/// `HUMANITY_GROUND_BEDS=0` builds the set without road beds and building pads, and the district
/// mesher goes back to rung 2's road lifts and building skirt: the bed gate's red run.
pub const GROUND_BEDS_ENV: &str = "HUMANITY_GROUND_BEDS";

/// Are road beds and building pads on (read once; off only for the gate's red run)?
pub fn beds_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| {
        let on = !std::env::var(GROUND_BEDS_ENV).map(|v| v.trim() == "0").unwrap_or(false);
        if !on {
            log::warn!("[MapGround] {GROUND_BEDS_ENV}=0: no road beds or building pads, rung 2's lifts and skirt (the bed gate's red run; never in play)");
        }
        on
    })
}

fn smoothstep(x: f64) -> f64 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The least raw barycentric weight: positive inside (the tile key's own rule).
fn firmness(c: &[DVec3; 3], d: DVec3) -> f64 {
    let b0 = d.dot(c[1].cross(c[2]));
    let b1 = d.dot(c[2].cross(c[0]));
    let b2 = d.dot(c[0].cross(c[1]));
    b0.min(b1).min(b2)
}

/// The child of (`id`, `c`) holding `d` most firmly.
fn step(id: PatchId, c: &[DVec3; 3], d: DVec3) -> (PatchId, [DVec3; 3], usize) {
    let kids = child_corners(c);
    let mut best = 0;
    for i in 1..4 {
        if firmness(&kids[i], d) > firmness(&kids[best], d) {
            best = i;
        }
    }
    (id.child(best as u32), kids[best], best)
}

/// Normalized barycentric weights of `d` in the flat triangle `c` (where the radial ray meets it).
fn bary(c: &[DVec3; 3], d: DVec3) -> [f64; 3] {
    let b = [d.dot(c[1].cross(c[2])), d.dot(c[2].cross(c[0])), d.dot(c[0].cross(c[1]))];
    let s = b[0] + b[1] + b[2];
    if s.abs() < 1e-300 {
        return [1.0, 0.0, 0.0];
    }
    [b[0] / s, b[1] / s, b[2] / s]
}

// ── Slots: decoded tiles kept while patches need them ───────────────────────────────────────

struct Slot<T> {
    v: RwLock<Option<Arc<T>>>,
    used: AtomicU64,
}

impl<T> Slot<T> {
    fn new() -> Self {
        Slot { v: RwLock::new(None), used: AtomicU64::new(0) }
    }
    fn get(&self, frame: u64, decode: impl FnOnce() -> Option<T>) -> Option<Arc<T>> {
        self.used.store(frame, Ordering::Relaxed);
        if let Some(v) = self.v.read().ok()?.as_ref() {
            return Some(v.clone());
        }
        let d = Arc::new(decode()?);
        let mut w = self.v.write().ok()?;
        if w.is_none() {
            *w = Some(d);
        }
        w.clone()
    }
    fn loaded(&self) -> bool {
        self.v.read().map(|v| v.is_some()).unwrap_or(false)
    }
    fn evict_if_older(&self, frame: u64) -> bool {
        if self.used.load(Ordering::Relaxed) + EVICT_FRAMES < frame && self.loaded() {
            if let Ok(mut w) = self.v.write() {
                *w = None;
                return true;
            }
        }
        false
    }
}

// ── The pack's plane and its shore field ─────────────────────────────────────────────────────

/// The pack's gnomonic plane in metres: great circles are straight lines in it.
#[derive(Clone, Copy)]
struct Plane {
    c: DVec3,
    e: DVec3,
    n: DVec3,
}

impl Plane {
    fn of(c: DVec3) -> Self {
        let e = if c.y.abs() < 0.9 { DVec3::Y.cross(c).normalize() } else { DVec3::X.cross(c).normalize() };
        Plane { c, e, n: c.cross(e) }
    }
    fn to(&self, p: DVec3) -> (f64, f64) {
        let d = p.dot(self.c).max(1e-9);
        (R * p.dot(self.e) / d, R * p.dot(self.n) / d)
    }
    fn from(&self, (x, y): (f64, f64)) -> DVec3 {
        (self.c + self.e * (x / R) + self.n * (y / R)).normalize()
    }
}

/// One water ring of a level-12 tile in the plane.
struct PRing {
    water: bool,
    body: u32,
    pts: Vec<(f64, f64)>,
    area: f64,
}

/// One true shoreline segment (water on its left) and the water body it bounds.
#[derive(Clone, Copy)]
struct Seg {
    a: (f64, f64),
    b: (f64, f64),
    body: u32,
}

/// A water body: the sea, or one lake or river.
#[derive(Clone, Debug)]
pub struct Body {
    pub kind: WaterRingKind,
    pub feature: u64,
    pub surface_m: f64,
}

struct ShoreField {
    plane: Plane,
    tiles: HashMap<u64, Vec<PRing>>,
    segs: Vec<Seg>,
    grid: HashMap<(i32, i32), Vec<u32>>,
    bodies: Vec<Body>,
}

fn cell(p: (f64, f64)) -> (i32, i32) {
    ((p.0 / GRID_M).floor() as i32, (p.1 / GRID_M).floor() as i32)
}

fn seg_dist(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> (f64, (f64, f64)) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    let q = (a.0 + t * dx, a.1 + t * dy);
    (((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt(), q)
}

impl ShoreField {
    /// The water body holding `d` in the level-12 `tile` the descent chose for it, None on land.
    /// The point is first moved `EDGE_NUDGE_M` toward the tile's centre, so a point on the tile's
    /// border is judged against that tile's rings from inside it.
    fn class(&self, tile: &(PatchId, [DVec3; 3]), d: DVec3) -> Option<u32> {
        let p = self.plane.to(d);
        let c = self.plane.to((tile.1[0] + tile.1[1] + tile.1[2]).normalize());
        let (dx, dy) = (c.0 - p.0, c.1 - p.1);
        let len = (dx * dx + dy * dy).sqrt();
        let p = if len > EDGE_NUDGE_M { (p.0 + dx / len * EDGE_NUDGE_M, p.1 + dy / len * EDGE_NUDGE_M) } else { p };
        self.class_at_plane(tile.0.path, p)
    }

    /// The water body holding plane point `p` (in level-12 tile `path`), None on land.
    fn class_at_plane(&self, path: u64, p: (f64, f64)) -> Option<u32> {
        let rings = self.tiles.get(&path)?;
        let mut depth = 0i32;
        let mut best: Option<(f64, u32)> = None;
        for r in rings {
            let k = r.pts.len();
            let mut inside = false;
            for i in 0..k {
                let (a, b) = (r.pts[i], r.pts[(i + 1) % k]);
                if (a.1 > p.1) != (b.1 > p.1) && a.0 + (p.1 - a.1) * (b.0 - a.0) / (b.1 - a.1) > p.0 {
                    inside = !inside;
                }
            }
            if inside {
                if r.water {
                    depth += 1;
                    if best.is_none_or(|(ba, _)| r.area < ba) {
                        best = Some((r.area, r.body));
                    }
                } else {
                    depth -= 1;
                }
            }
        }
        if depth > 0 {
            best.map(|b| b.1)
        } else {
            None
        }
    }

    /// The nearest true shoreline within `reach` metres: (distance, its body, the nearest point).
    fn nearest(&self, p: (f64, f64), reach: f64) -> Option<(f64, u32, (f64, f64))> {
        let (c, r) = (cell(p), (reach / GRID_M).ceil() as i32);
        let mut best: Option<(f64, u32, (f64, f64))> = None;
        for dy in -r..=r {
            for dx in -r..=r {
                for &si in self.grid.get(&(c.0 + dx, c.1 + dy)).map(Vec::as_slice).unwrap_or(&[]) {
                    let s = &self.segs[si as usize];
                    let (d, q) = seg_dist(p, s.a, s.b);
                    if d <= reach && best.is_none_or(|b| d < b.0) {
                        best = Some((d, s.body, q));
                    }
                }
            }
        }
        best
    }
}

// ── One installed pack ───────────────────────────────────────────────────────────────────────

/// One installed pack and what has been decoded from it.
pub struct PackState {
    pub pack: Arc<Pack>,
    /// The file name (the credit and the probe name the pack by it).
    pub name: String,
    id: PatchId,
    corners: [DVec3; 3],
    /// Inward unit normals of the three edges, and whether the pack across each is installed.
    normals: [DVec3; 3],
    neighbour: [bool; 3],
    land: Vec<Slot<Vec<f32>>>,
    sea: Vec<Slot<Vec<f32>>>,
    cover: Vec<Slot<Vec<u8>>>,
    built: Vec<Slot<BuiltMask>>,
    /// Per level-12 tile: the road beds and building pads that can reach it.
    beds: Vec<Slot<beds::BedTile>>,
    shore: OnceLock<ShoreField>,
}

/// The levels' slot offsets: level L's tiles start at `offsets[i]` for `levels[i] == L`.
fn offsets(levels: &[u8], pack_depth: u8) -> Vec<usize> {
    let mut o = Vec::new();
    let mut at = 0;
    for &l in levels {
        o.push(at);
        at += 1usize << (2 * (l - pack_depth) as usize);
    }
    o.push(at);
    o
}

impl PackState {
    fn new(pack: Arc<Pack>, name: String, levels: &MapLevels) -> Self {
        let id = pack.patch();
        let corners = patch_corners(&id);
        let mut normals = [DVec3::ZERO; 3];
        for k in 0..3 {
            let n = corners[k].cross(corners[(k + 1) % 3]).normalize();
            normals[k] = if n.dot(corners[(k + 2) % 3]) < 0.0 { -n } else { n };
        }
        let slots = |lv: &[u8]| -> usize { *offsets(lv, id.depth).last().unwrap() };
        let finest_n = 1usize << (2 * (levels.finest() - id.depth) as usize);
        PackState {
            land: (0..slots(&levels.levels)).map(|_| Slot::new()).collect(),
            sea: (0..slots(&levels.sea_levels)).map(|_| Slot::new()).collect(),
            cover: (0..finest_n).map(|_| Slot::new()).collect(),
            built: (0..finest_n).map(|_| Slot::new()).collect(),
            beds: (0..finest_n).map(|_| Slot::new()).collect(),
            pack,
            name,
            id,
            corners,
            normals,
            neighbour: [false; 3],
            shore: OnceLock::new(),
        }
    }

    /// Does the pack's tile hold the direction `d`?
    pub fn holds(&self, d: DVec3) -> bool {
        self.normals.iter().all(|n| n.dot(d) >= 0.0)
    }

    /// The pack's gnomonic plane, about its tile's centre (the shore field and the beds use it).
    fn plane(&self) -> Plane {
        Plane::of((self.corners[0] + self.corners[1] + self.corners[2]).normalize())
    }

    /// The beds and pads that can reach level-12 tile `tile` of this pack, built on first use.
    fn bed_tile(&self, set: &MapSet, tile: &PatchId) -> Option<Arc<beds::BedTile>> {
        self.beds.get(self.local(tile))?.get(set.frame(), || Some(beds::BedTile::build(set, self, tile)))
    }

    /// The tiles holding `d` at the pack's own depth and every level to `to` (inclusive).
    fn descend(&self, d: DVec3, to: u8) -> Vec<(PatchId, [DVec3; 3])> {
        let mut out = vec![(self.id, self.corners)];
        let (mut id, mut c) = (self.id, self.corners);
        while id.depth < to {
            let (nid, nc, _) = step(id, &c, d);
            id = nid;
            c = nc;
            out.push((id, c));
        }
        out
    }

    /// The local slot index of a tile at `level`.
    fn local(&self, tile: &PatchId) -> usize {
        let bits = 2 * (tile.depth - self.id.depth) as u32;
        ((tile.path >> (2 * self.id.depth as u32)) & ((1u64 << bits) - 1)) as usize
    }

    /// A value interpolated on a tile's lattice at `d`: the cell holding it (six more steps of
    /// the same descent) and its corners' samples, NaN where any is missing.
    fn lattice_value(tile: &(PatchId, [DVec3; 3]), segs: u32, d: DVec3, sample: &dyn Fn(usize) -> f32) -> f64 {
        let (mut id, mut c) = *tile;
        let mut rc = [(0, 0), (segs, 0), (segs, segs)];
        for _ in 0..segs.trailing_zeros() {
            let (nid, nc, k) = step(id, &c, d);
            id = nid;
            c = nc;
            rc = child_lattice_rc(rc)[k];
        }
        let w = bary(&c, d);
        // At a lattice vertex (every vertex of a patch of depth L to L+2 is one) the sample itself,
        // exactly. The triple products of a 27 m cell's unit corners carry about 1e-6 relative
        // error (a tangent vector dotted with a radial one), so a vertex reads weights like
        // 0.9999993: within 1e-5 of a corner (0.3 mm on a level-12 cell) is that corner.
        if let Some(i) = (0..3).find(|i| w[*i] > 1.0 - 1e-5) {
            let s = sample(lattice_index(rc[i].0, rc[i].1));
            return if s.is_finite() { s as f64 } else { f64::NAN };
        }
        let mut v = 0.0;
        for i in 0..3 {
            let s = sample(lattice_index(rc[i].0, rc[i].1));
            if !s.is_finite() {
                return f64::NAN;
            }
            v += w[i] * s as f64;
        }
        v
    }

    fn heights(&self, kind: ChunkKind, levels: &[u8], tile: &PatchId, frame: u64) -> Option<Arc<Vec<f32>>> {
        let li = levels.iter().position(|l| *l == tile.depth)?;
        let slots = if kind == ChunkKind::LandH { &self.land } else { &self.sea };
        let at = offsets(levels, self.id.depth)[li] + self.local(tile);
        slots.get(at)?.get(frame, || self.pack.heights(kind, tile.depth, tile.path))
    }

    fn cover_at(&self, tile: &(PatchId, [DVec3; 3]), segs: u32, d: DVec3, frame: u64) -> u8 {
        let Some(c) = self.cover.get(self.local(&tile.0)).and_then(|s| s.get(frame, || self.pack.chunk(ChunkKind::Cover, tile.0.depth, tile.0.path))) else {
            return 0;
        };
        // Nearest lattice sample, not interpolated (a class).
        let (mut id, mut cc) = *tile;
        let mut rc = [(0, 0), (segs, 0), (segs, segs)];
        for _ in 0..segs.trailing_zeros() {
            let (nid, nc, k) = step(id, &cc, d);
            id = nid;
            cc = nc;
            rc = child_lattice_rc(rc)[k];
        }
        let w = bary(&cc, d);
        let i = if w[0] >= w[1] && w[0] >= w[2] { 0 } else if w[1] >= w[2] { 1 } else { 2 };
        c.get(lattice_index(rc[i].0, rc[i].1)).copied().unwrap_or(0)
    }

    /// The shore field, built on first use: every level-12 tile's water rings in the plane, the true
    /// shoreline's segments in a grid, and each lake's surface (the median of the land along its
    /// shore; a river's, its lowest shore point; never below 0.5 m above the sea).
    fn shore(&self, set: &MapSet) -> &ShoreField {
        self.shore.get_or_init(|| {
            let plane = self.plane();
            let mut bodies = vec![Body { kind: WaterRingKind::Sea, feature: 0, surface_m: 0.0 }];
            let mut body_of: HashMap<u64, u32> = HashMap::new();
            let mut shore_pts: HashMap<u32, Vec<DVec3>> = HashMap::new();
            let (mut tiles, mut segs) = (HashMap::new(), Vec::new());
            let finest = set.levels.finest();
            for e in self.pack.entries().filter(|e| e.kind == ChunkKind::Water && e.level == finest) {
                let mut prs = Vec::new();
                for r in self.pack.water(e.path).unwrap_or_default() {
                    let body = if r.feature == 0 {
                        0
                    } else {
                        *body_of.entry(r.feature).or_insert_with(|| {
                            let kind = if r.kind == WaterRingKind::River { WaterRingKind::River } else { WaterRingKind::Lake };
                            bodies.push(Body { kind, feature: r.feature, surface_m: 0.5 });
                            bodies.len() as u32 - 1
                        })
                    };
                    if r.kind == WaterRingKind::River {
                        bodies[body as usize].kind = WaterRingKind::River;
                    }
                    let dirs: Vec<DVec3> = r.points.iter().map(|p| e7_to_dir(p.0, p.1)).collect();
                    let pts: Vec<(f64, f64)> = dirs.iter().map(|d| plane.to(*d)).collect();
                    let k = pts.len();
                    let mut area = 0.0;
                    for i in 0..k {
                        let (a, b) = (pts[i], pts[(i + 1) % k]);
                        area += a.0 * b.1 - b.0 * a.1;
                        if !r.clip.get(i).copied().unwrap_or(true) {
                            segs.push(Seg { a, b, body });
                            if body != 0 {
                                shore_pts.entry(body).or_default().push(dirs[i]);
                            }
                        }
                    }
                    prs.push(PRing { water: r.kind.is_water(), body, pts, area: area.abs() * 0.5 });
                }
                tiles.insert(e.path, prs);
            }
            // Lake surfaces from the land around them (the land layer's own heights, at run time).
            for (body, pts) in shore_pts {
                let mut hs: Vec<f64> = pts.iter().filter_map(|d| self.land_h(set, *d, finest)).collect();
                if hs.is_empty() {
                    continue;
                }
                hs.sort_by(|a, b| a.total_cmp(b));
                let b = &mut bodies[body as usize];
                b.surface_m = if b.kind == WaterRingKind::River { hs[0] } else { hs[hs.len() / 2] }.max(0.5);
            }
            let mut grid: HashMap<(i32, i32), Vec<u32>> = HashMap::new();
            for (i, s) in segs.iter().enumerate() {
                let (c0, c1) = (cell(s.a), cell(s.b));
                for y in c0.1.min(c1.1)..=c0.1.max(c1.1) {
                    for x in c0.0.min(c1.0)..=c0.0.max(c1.0) {
                        grid.entry((x, y)).or_default().push(i as u32);
                    }
                }
            }
            log::info!("[MapGround] {}: shore field built ({} tiles with water, {} shoreline segments, {} lakes and rivers)", self.name, tiles.len(), segs.len(), bodies.len() - 1);
            ShoreField { plane, tiles, segs, grid, bodies }
        })
    }

    /// The land layer at `d` on the level-`level` lattice (no shore band): metres, None where the
    /// source has no data.
    fn land_h(&self, set: &MapSet, d: DVec3, level: u8) -> Option<f64> {
        let path = self.descend(d, level);
        let tile = path.last()?;
        let hs = self.heights(ChunkKind::LandH, &set.levels.levels, &tile.0, set.frame())?;
        let v = Self::lattice_value(tile, set.levels.lattice_segments, d, &|i| hs.get(i).copied().unwrap_or(f32::NAN));
        v.is_finite().then_some(v)
    }

    /// How much of the pack's ground shows at `d`: 1 inside, fading to 0 at an edge whose
    /// neighbour pack is not installed.
    fn edge_weight(&self, d: DVec3, blend_m: f64) -> f64 {
        let mut w: f64 = 1.0;
        for k in 0..3 {
            if !self.neighbour[k] {
                w = w.min(smoothstep(self.normals[k].dot(d) * R / blend_m));
            }
        }
        w
    }
}

// ── Built-over ground, for the vegetation gates ──────────────────────────────────────────────

/// A level-12 tile's roads, buildings and water rasterized in the pack's plane: where trees and
/// grass must not grow (the old region masks' CELL_BUILT, now per tile).
pub struct BuiltMask {
    x0: f64,
    y0: f64,
    nx: usize,
    ny: usize,
    bits: Vec<u8>,
}

/// The built mask's cell, metres.
const BUILT_CELL_M: f64 = 3.0;
/// The curb strip added around a road's carriageway (trees at the asphalt edge hang over it).
const ROAD_CURB_M: f64 = 1.5;

impl BuiltMask {
    fn sample(&self, p: (f64, f64)) -> f32 {
        let x = ((p.0 - self.x0) / BUILT_CELL_M).floor();
        let y = ((p.1 - self.y0) / BUILT_CELL_M).floor();
        if x < 0.0 || y < 0.0 || x >= self.nx as f64 || y >= self.ny as f64 {
            return 0.0;
        }
        self.bits[y as usize * self.nx + x as usize] as f32
    }

    fn build(ps: &PackState, plane: &Plane, field: &ShoreField, tile: &PatchId) -> Self {
        let c = patch_corners(tile);
        let pc: Vec<(f64, f64)> = c.iter().map(|d| plane.to(*d)).collect();
        let pad = 20.0;
        let (x0, y0) = (pc.iter().map(|p| p.0).fold(f64::MAX, f64::min) - pad, pc.iter().map(|p| p.1).fold(f64::MAX, f64::min) - pad);
        let (x1, y1) = (pc.iter().map(|p| p.0).fold(f64::MIN, f64::max) + pad, pc.iter().map(|p| p.1).fold(f64::MIN, f64::max) + pad);
        let (nx, ny) = (((x1 - x0) / BUILT_CELL_M).ceil() as usize, ((y1 - y0) / BUILT_CELL_M).ceil() as usize);
        let mut m = BuiltMask { x0, y0, nx, ny, bits: vec![0; nx * ny] };
        let fill = |m: &mut BuiltMask, ring: &[(f64, f64)], v: u8| {
            let (ylo, yhi) = (ring.iter().map(|p| p.1).fold(f64::MAX, f64::min), ring.iter().map(|p| p.1).fold(f64::MIN, f64::max));
            let (r0, r1) = (((ylo - m.y0) / BUILT_CELL_M).floor().max(0.0) as usize, (((yhi - m.y0) / BUILT_CELL_M).ceil() as usize).min(m.ny));
            let mut xs = Vec::new();
            for row in r0..r1 {
                let y = m.y0 + (row as f64 + 0.5) * BUILT_CELL_M;
                xs.clear();
                for i in 0..ring.len() {
                    let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
                    if (a.1 > y) != (b.1 > y) {
                        xs.push(a.0 + (y - a.1) * (b.0 - a.0) / (b.1 - a.1));
                    }
                }
                xs.sort_by(|a, b| a.total_cmp(b));
                for pair in xs.chunks_exact(2) {
                    let (c0, c1) = (((pair[0] - m.x0) / BUILT_CELL_M - 0.5).ceil().max(0.0) as usize, (((pair[1] - m.x0) / BUILT_CELL_M - 0.5).floor() as i64).min(m.nx as i64 - 1));
                    for col in c0 as i64..=c1 {
                        m.bits[row * m.nx + col as usize] = v;
                    }
                }
            }
        };
        if let Some(rings) = field.tiles.get(&tile.path) {
            for pass in [(true, 0u32), (false, 0), (true, 1), (false, 1)] {
                for r in rings.iter().filter(|r| r.water == pass.0 && (r.body == 0) == (pass.1 == 0)) {
                    fill(&mut m, &r.pts, pass.0 as u8);
                }
            }
        }
        for b in ps.pack.buildings(tile.path).unwrap_or_default() {
            let ring: Vec<(f64, f64)> = b.ring.iter().map(|p| plane.to(e7_to_dir(p.0, p.1))).collect();
            fill(&mut m, &ring, 1);
        }
        for w in ps.pack.ways(tile.path).unwrap_or_default() {
            let half = if w.width_m > 0.0 { w.width_m as f64 * 0.5 } else { crate::osm_region::road_full_width_m(w.class) as f64 * 0.5 } + ROAD_CURB_M;
            let pts: Vec<(f64, f64)> = w.points.iter().map(|p| plane.to(e7_to_dir(p.0, p.1))).collect();
            for s in pts.windows(2) {
                let (lo_x, hi_x) = (s[0].0.min(s[1].0) - half, s[0].0.max(s[1].0) + half);
                let (lo_y, hi_y) = (s[0].1.min(s[1].1) - half, s[0].1.max(s[1].1) + half);
                let (c0, c1) = (((lo_x - m.x0) / BUILT_CELL_M).floor().max(0.0) as usize, (((hi_x - m.x0) / BUILT_CELL_M).ceil() as usize).min(m.nx));
                let (r0, r1) = (((lo_y - m.y0) / BUILT_CELL_M).floor().max(0.0) as usize, (((hi_y - m.y0) / BUILT_CELL_M).ceil() as usize).min(m.ny));
                for row in r0..r1 {
                    for col in c0..c1 {
                        let p = (m.x0 + (col as f64 + 0.5) * BUILT_CELL_M, m.y0 + (row as f64 + 0.5) * BUILT_CELL_M);
                        if seg_dist(p, s[0], s[1]).0 <= half {
                            m.bits[row * m.nx + col] = 1;
                        }
                    }
                }
            }
        }
        m
    }
}

// ── The installed set ────────────────────────────────────────────────────────────────────────

/// One nearest-shore reading: the distance, its water's surface, whether that water is the sea,
/// the point on the shore and the pack whose shore field it came from.
#[derive(Clone, Copy)]
struct Near<'a> {
    dist: f64,
    surface: f64,
    sea: bool,
    at: DVec3,
    pack: &'a PackState,
}

/// What the map ground says at a point.
#[derive(Clone, Copy, Debug, PartialEq)]
/// reuse: site: the whole-Earth map packs' decoded tiles and each pack's shore field, kept while
/// patches need them, tagged by the pack's build; with the switch off a fresh set is published
/// every frame (`engine::map_meshes`), so every tile and shore field is decoded again.
pub struct MapGround {
    /// The ground, metres above sea level.
    pub h_m: f64,
    /// How much of it shows over the base heights (1 inside a pack, fading at an edge whose
    /// neighbour is not installed).
    pub weight: f64,
    /// Signed distance to the true shoreline, metres, positive on land (`reach_m` when farther).
    pub shore_m: f64,
    /// The water body's kind under the point, None on land.
    pub water: Option<WaterRingKind>,
}

/// The road bed nearest a point ([`MapSet::road_near`]).
#[derive(Clone, Copy, Debug)]
pub struct RoadNear {
    pub osm_id: u64,
    pub class: u8,
    /// Half the carriageway, and half the bed (the carriageway plus a shoulder).
    pub half_w_m: f64,
    pub core_m: f64,
    /// The nearest centreline point and the unit direction across the road there (on the sphere).
    pub centre: DVec3,
    pub across: DVec3,
    /// The bed's height at the centre, metres above sea level (None: no land data under it).
    pub bed_m: Option<f64>,
    /// How far the asked point is from the centreline, metres.
    pub dist_m: f64,
}

/// The building pad holding or nearest a point ([`MapSet::pad_near`]).
#[derive(Clone, Debug)]
pub struct PadNear {
    pub osm_id: u64,
    /// The footprint (open ring) on the sphere.
    pub ring: Vec<DVec3>,
    /// The pad's floor, metres above sea level (None: no land data under it).
    pub floor_m: Option<f64>,
    pub apron_m: f64,
}

/// Every installed pack, published for the patch builders and every ground consumer.
pub struct MapSet {
    pub levels: MapLevels,
    pub packs: Vec<PackState>,
    frame: AtomicU64,
    /// Rung-2 red-evidence switch (`HUMANITY_SHORE_BAND=0`): the pack's land and seabed meet with
    /// no shore band (land on land, ETOPO under water). Read when the set is made; never in play.
    band_on: bool,
    /// Road beds and building pads ([`beds_enabled`]; off only for the bed gate's red run).
    beds_on: bool,
}

/// `HUMANITY_SHORE_BAND=0` builds the set without the shore band (the shore gate's red run).
pub const SHORE_BAND_ENV: &str = "HUMANITY_SHORE_BAND";

impl MapSet {
    /// A set over these packs (each with its file name), the neighbours worked out between them.
    pub fn new(levels: MapLevels, packs: Vec<(Arc<Pack>, String)>) -> Self {
        let band_on = !std::env::var(SHORE_BAND_ENV).map(|v| v.trim() == "0").unwrap_or(false);
        if !band_on {
            log::warn!("[MapGround] built with {SHORE_BAND_ENV}=0: no shore band (the shore gate's red run; never in play)");
        }
        let mut states: Vec<PackState> = packs.into_iter().map(|(p, n)| PackState::new(p, n, &levels)).collect();
        let ids: Vec<PatchId> = states.iter().map(|s| s.id).collect();
        for s in &mut states {
            for k in 0..3 {
                let mid = (s.corners[k] + s.corners[(k + 1) % 3]).normalize();
                let out = (mid - s.normals[k] * 1e-7).normalize();
                s.neighbour[k] = ids.contains(&crate::map_tiles::tile_containing_dir(out, s.id.depth));
            }
        }
        MapSet { levels, packs: states, frame: AtomicU64::new(1), band_on, beds_on: beds_enabled() }
    }

    /// The installed packs' tiles (depth 6): where the terrain may refine to full detail.
    pub fn pack_ids(&self) -> Vec<PatchId> {
        self.packs.iter().map(|p| p.id).collect()
    }

    /// The same packs with nothing decoded: what the engine publishes every frame while the Dev
    /// page's "turn off all reuse" switch is off.
    pub fn fresh_generation(&self) -> Self {
        let packs = self.packs.iter().map(|p| (p.pack.clone(), p.name.clone())).collect();
        let s = MapSet::new(self.levels.clone(), packs);
        s.frame.store(self.frame(), Ordering::Relaxed);
        s
    }

    fn frame(&self) -> u64 {
        self.frame.load(Ordering::Relaxed)
    }

    /// Once a frame, from the main thread: advance the clock, mark every tile the current
    /// selection's patches read as in use, and drop the slots no patch has needed for
    /// `EVICT_FRAMES` frames. Returns how many were dropped.
    pub fn tick(&self, patches: impl Iterator<Item = PatchId>) -> usize {
        let f = self.frame.fetch_add(1, Ordering::Relaxed) + 1;
        for id in patches {
            for ps in &self.packs {
                if id.face != ps.id.face || id.depth < ps.id.depth || crate::map_tiles::ancestor_at(id, ps.id.depth) != Some(ps.id) {
                    continue;
                }
                let Some(level) = level_for_depth(&self.levels.levels, id.depth) else { continue };
                if let Some(t) = crate::map_tiles::ancestor_at(id, level) {
                    let li = self.levels.levels.iter().position(|l| *l == level).unwrap_or(0);
                    if let Some(s) = ps.land.get(offsets(&self.levels.levels, ps.id.depth)[li] + ps.local(&t)) {
                        s.used.store(f, Ordering::Relaxed);
                    }
                }
            }
        }
        // The sweep over every slot runs once every 64 frames (about 25,000 slots for two packs:
        // half a millisecond if it ran every frame); a slot waits at most that much longer.
        let mut dropped = 0;
        if f % 64 != 0 {
            return 0;
        }
        for ps in &self.packs {
            for s in &ps.land {
                dropped += s.evict_if_older(f) as usize;
            }
            for s in &ps.sea {
                dropped += s.evict_if_older(f) as usize;
            }
            for s in &ps.cover {
                dropped += s.evict_if_older(f) as usize;
            }
            for s in &ps.built {
                dropped += s.evict_if_older(f) as usize;
            }
            for s in &ps.beds {
                dropped += s.evict_if_older(f) as usize;
            }
        }
        dropped
    }

    /// How many tiles are decoded now (land, sea, cover, built, beds), for the Dev page and the log.
    pub fn resident(&self) -> usize {
        self.packs
            .iter()
            .map(|p| {
                p.land.iter().filter(|s| s.loaded()).count()
                    + p.sea.iter().filter(|s| s.loaded()).count()
                    + p.cover.iter().filter(|s| s.loaded()).count()
                    + p.built.iter().filter(|s| s.loaded()).count()
                    + p.beds.iter().filter(|s| s.loaded()).count()
            })
            .sum()
    }

    /// Build every pack's shore field now (a worker calls this right after the set is published,
    /// so no patch build waits on it).
    pub fn prepare(&self) {
        for p in &self.packs {
            p.shore(self);
        }
    }

    fn pack_of(&self, d: DVec3) -> Option<&PackState> {
        self.packs.iter().find(|p| p.holds(d))
    }

    /// The land layer at `d` on the level-`level` lattice of whichever pack holds `d` (never a pack
    /// that does not: its descent would end in a cell that does not hold the point and read one of
    /// its corners). The beds' profiles and floors sample through this.
    fn land_at(&self, d: DVec3, level: u8) -> Option<f64> {
        self.pack_of(d)?.land_h(self, d, level)
    }

    /// Does any pack lie within `radius_m` of `center` (one call per vegetation harvest)?
    pub fn any_in_disc(&self, center: DVec3, radius_m: f64) -> bool {
        let c = center.normalize();
        self.packs.iter().any(|p| p.normals.iter().all(|n| n.dot(c) * R >= -radius_m))
    }

    /// THE MAP GROUND at `d` for a patch of `depth` (the shore band, section 5.2), or None outside
    /// every pack and where the land layer has no data. No fine relief: [`MapSet::ground_with`].
    pub fn ground(&self, d: DVec3, depth: u8) -> Option<MapGround> {
        self.ground_with(d, depth, None)
    }

    /// THE MAP GROUND at `d` for a patch of `depth`, with the procedural relief finer than the
    /// level-12 lattice added on land away from any shore when `detail` is given (patches of
    /// `detail_first_depth` and deeper). On land the order is: the land layer, the fine relief, the
    /// road beds and building pads (they hold the ground flat, so the relief does not show through
    /// them), then the shore band, which wins within its band.
    pub fn ground_with(&self, d: DVec3, depth: u8, detail: Option<&crate::planet_chunks::DetailNoise>) -> Option<MapGround> {
        self.ground_opts(d, depth, detail, self.beds_on)
    }

    /// The map ground with no road beds or building pads, for the questions they cannot change:
    /// the vegetation gates' land test ([`apply_normalized_gate`]) and the water shell's depth
    /// (land reads 0 either way). Measured in rung 2b: the far forest's gate samples over Seattle
    /// built a bed tile per level-12 tile on the main thread, 1.6 s a frame on arrival.
    pub fn ground_without_beds(&self, d: DVec3, depth: u8) -> Option<MapGround> {
        self.ground_opts(d, depth, None, false)
    }

    fn ground_opts(&self, d: DVec3, depth: u8, detail: Option<&crate::planet_chunks::DetailNoise>, beds: bool) -> Option<MapGround> {
        let d = d.normalize();
        let ps = self.pack_of(d)?;
        let weight = ps.edge_weight(d, self.levels.edge_blend_m);
        if weight <= 0.0 {
            return None;
        }
        let lv = &self.levels;
        let level = level_for_depth(&lv.levels, depth)?;
        let finest = lv.finest();
        let path = ps.descend(d, finest);
        let tile_at = |l: u8| path.iter().find(|t| t.0.depth == l);
        let frame = self.frame();
        let hs = ps.heights(ChunkKind::LandH, &lv.levels, &tile_at(level)?.0, frame)?;
        let land = PackState::lattice_value(tile_at(level)?, lv.lattice_segments, d, &|i| hs.get(i).copied().unwrap_or(f32::NAN));
        if !land.is_finite() {
            return None;
        }
        let field = ps.shore(self);
        let body = field.class(path.last()?, d);
        let sh = &lv.shore;
        // The nearest true shoreline in this pack AND in every installed pack within reach: a pack's
        // index holds only its own tiles' edges, so near a shared pack border the shore across it
        // counts too and the ground meets its neighbour's without a step.
        let near = self.nearest_shore(ps, d, sh.reach_m);
        let dist = near.map_or(sh.reach_m, |n| n.dist);
        let shore_m = if body.is_some() { -dist } else { dist };
        let water = body.map(|b| field.bodies[b as usize].kind);
        let sea_h = || -> Option<f64> {
            let sl = lv.sea_levels.iter().copied().filter(|s| *s <= level).last().or(lv.sea_levels.first().copied())?;
            let t = tile_at(sl)?;
            let s = ps.heights(ChunkKind::SeaH, &lv.sea_levels, &t.0, frame)?;
            let v = PackState::lattice_value(t, lv.lattice_segments, d, &|i| s.get(i).copied().unwrap_or(f32::NAN));
            v.is_finite().then_some(v)
        };
        let spacing = vertex_spacing_m(depth, R);
        let finest_cell = patch_edge_arc_m(finest, R) / lv.lattice_segments as f64;
        // The land side's own ground: the fine relief, then the beds and pads (map_beds).
        let mut land = land;
        if body.is_none() {
            if let Some(dn) = detail {
                if depth >= lv.detail_first_depth {
                    let mask = smoothstep((shore_m - lv.detail_shore_mask_m) / lv.detail_shore_mask_m.max(1.0));
                    if mask > 0.0 {
                        land += dn.sample_m_fine_below(d, depth, finest_cell, R) as f64 * mask;
                    }
                }
            }
            if beds && depth >= lv.beds.first_depth {
                if let Some(bt) = ps.bed_tile(self, &path.last()?.0) {
                    land = bt.apply(self, field.plane.to(d), land, spacing);
                }
            }
        }
        if !self.band_on {
            // The red run: the two sources meet raw at the coastline.
            let h = if body.is_some() { sea_h().unwrap_or(land) } else { land };
            return Some(MapGround { h_m: h, weight, shore_m, water });
        }
        let band = sh.min_band_m.max(sh.wet_offshore_m + sh.band_spacings * spacing);
        let h = match (body, near) {
            // Land beyond the band's reach: the land layer itself.
            (None, None) => land,
            (None, Some(n)) => {
                let datum = n.surface + if n.sea { sh.datum_m } else { sh.lake_datum_m };
                let k = (datum - n.surface + sh.wet_depth_m) / sh.wet_offshore_m;
                let floor = datum - (dist - sh.floor_cells * finest_cell).max(0.0) * sh.floor_release_grade;
                let cap = datum + k * dist.min(band) + (dist - band).max(0.0) * sh.cap_slope;
                land.clamp(floor.min(cap), cap)
            }
            (Some(bi), _) => {
                let b = &field.bodies[bi as usize];
                let datum = b.surface_m + if bi == 0 { sh.datum_m } else { sh.lake_datum_m };
                let k = (datum - b.surface_m + sh.wet_depth_m) / sh.wet_offshore_m;
                let a = if bi == 0 {
                    // The beach's kind from the land cover at the nearest shore point.
                    near.and_then(|n| n.pack.descend(n.at, finest).last().map(|t| sh.profile_a(n.pack.cover_at(t, lv.lattice_segments, n.at, frame))))
                    .unwrap_or(sh.default_a)
                } else {
                    sh.default_a
                };
                let profile = datum - a * dist.powf(2.0 / 3.0);
                let natural = if bi == 0 {
                    match sea_h() {
                        Some(s) => profile + (s.min(b.surface_m - sh.wet_depth_m) - profile) * smoothstep(dist / sh.sea_blend_m),
                        None => profile,
                    }
                } else {
                    profile
                };
                natural.min(datum - k * dist.min(band))
            }
        };
        Some(MapGround { h_m: h, weight, shore_m, water })
    }

    /// The nearest true shoreline to `d` within `reach` metres, over `home` and every pack whose
    /// tile lies within reach of `d`.
    fn nearest_shore<'a>(&'a self, home: &'a PackState, d: DVec3, reach: f64) -> Option<Near<'a>> {
        let mut best: Option<Near> = None;
        for ps in &self.packs {
            if !std::ptr::eq(ps, home) && !ps.normals.iter().all(|n| n.dot(d) * R >= -reach) {
                continue;
            }
            let f = ps.shore(self);
            if let Some((dist, b, q)) = f.nearest(f.plane.to(d), reach) {
                if best.as_ref().is_none_or(|x| dist < x.dist) {
                    best = Some(Near { dist, surface: f.bodies[b as usize].surface_m, sea: b == 0, at: f.plane.from(q), pack: ps });
                }
            }
        }
        best
    }

    /// The land layer at `d` at the finest level, untouched by the band: the shore probe's
    /// "unmodified land layer" (section 5.5), with the pack's file name.
    pub fn raw_land_m(&self, d: DVec3) -> Option<(f64, &str)> {
        let d = d.normalize();
        let ps = self.pack_of(d)?;
        Some((ps.land_h(self, d, self.levels.finest())?, ps.name.as_str()))
    }

    /// The water body under `d` in an installed pack: `Some((kind or None for land, pack name))`.
    pub fn class_at(&self, d: DVec3) -> Option<(Option<WaterRingKind>, &str)> {
        let d = d.normalize();
        let ps = self.pack_of(d)?;
        let field = ps.shore(self);
        let tile = *ps.descend(d, self.levels.finest()).last()?;
        Some((field.class(&tile, d).map(|b| field.bodies[b as usize].kind), ps.name.as_str()))
    }

    /// A lake's or river's surface (metres above sea level), for its flat sheet; the sea's is 0.
    pub fn surface_of(&self, d: DVec3) -> Option<f64> {
        let d = d.normalize();
        let ps = self.pack_of(d)?;
        let field = ps.shore(self);
        let tile = *ps.descend(d, self.levels.finest()).last()?;
        field.class(&tile, d).map(|b| field.bodies[b as usize].surface_m)
    }

    /// The surface of the lake or river body `feature` in the pack holding `d`.
    pub fn body_surface(&self, d: DVec3, feature: u64) -> Option<f64> {
        let ps = self.pack_of(d.normalize())?;
        ps.shore(self).bodies.iter().find(|b| b.feature == feature).map(|b| b.surface_m)
    }

    /// The sea's weight at `d` (1 in an installed pack's sea, else 0): the water shell's coverage.
    pub fn sea_weight(&self, d: DVec3) -> f32 {
        match self.class_at(d) {
            Some((Some(WaterRingKind::Sea), _)) => 1.0,
            _ => 0.0,
        }
    }

    /// The water depth at `d` inside a pack (0 on land), from the finest ground: the water shell's
    /// depth bake, so the shell feathers out exactly where this ground meets the sea.
    pub fn sea_depth_m(&self, d: DVec3) -> Option<f32> {
        self.ground_without_beds(d, 24).filter(|g| g.weight > 0.999).map(|g| (-g.h_m).max(0.0) as f32)
    }

    /// Built-over weight at (lat, lon): roads, buildings and water, where vegetation must not grow.
    pub fn built_weight_at_deg(&self, lat: f64, lon: f64) -> f32 {
        let d = crate::osm_region::latlon_to_dir_f64(lat, lon);
        let Some(ps) = self.pack_of(d) else { return 0.0 };
        let field = ps.shore(self);
        let Some((tile, _)) = ps.descend(d, self.levels.finest()).last().copied() else { return 0.0 };
        let Some(slot) = ps.built.get(ps.local(&tile)) else { return 0.0 };
        slot.get(self.frame(), || Some(BuiltMask::build(ps, &field.plane, field, &tile))).map_or(0.0, |m| m.sample(field.plane.to(d)))
    }

    /// The road bed whose centreline passes nearest `d` (the bed probe's question, and the tests'):
    /// None outside every pack or with no road piece reaching `d`'s level-12 tile.
    pub fn road_near(&self, d: DVec3) -> Option<RoadNear> {
        let d = d.normalize();
        let ps = self.pack_of(d)?;
        let tile = ps.descend(d, self.levels.finest()).last()?.0;
        let bt = ps.bed_tile(self, &tile)?;
        let plane = ps.plane();
        let (i, k, t, dist) = bt.nearest_road(plane.to(d))?;
        let r = &bt.roads[i];
        let (a, b, bed) = bt.road_point(self, i, k, t);
        let centre = (a * (1.0 - t) + b * t).normalize();
        let along = (b - a).normalize_or_zero();
        let across = centre.cross(along).normalize_or_zero();
        Some(RoadNear { osm_id: r.osm_id, class: r.class, half_w_m: r.half_w, core_m: r.core, centre, across, bed_m: bed, dist_m: dist })
    }

    /// The building pad holding `d`, or else the nearest to it in `d`'s level-12 tile.
    pub fn pad_near(&self, d: DVec3) -> Option<PadNear> {
        let d = d.normalize();
        let ps = self.pack_of(d)?;
        let tile = ps.descend(d, self.levels.finest()).last()?.0;
        let bt = ps.bed_tile(self, &tile)?;
        let i = bt.nearest_pad(ps.plane().to(d))?;
        let (ring, floor) = bt.pad_ring(self, i);
        Some(PadNear { osm_id: bt.pads[i].osm_id, ring, floor_m: floor, apron_m: self.levels.beds.apron_m })
    }

    /// How many road pieces and pads the bed tile holding `d` weighs (None outside every pack).
    pub fn beds_counted_at(&self, d: DVec3) -> Option<(usize, usize)> {
        let d = d.normalize();
        let ps = self.pack_of(d)?;
        let tile = ps.descend(d, self.levels.finest()).last()?.0;
        Some(ps.bed_tile(self, &tile)?.counts())
    }

    /// The packs a camera at `cam_dir` is within `range_m` of: what the map credit names.
    pub fn drawn_names(&self, cam_dir: DVec3, range_m: f64) -> Vec<String> {
        let c = cam_dir.normalize();
        self.packs.iter().filter(|p| p.normals.iter().all(|n| n.dot(c) * R >= -range_m)).map(|p| p.name.clone()).collect()
    }
}

/// The map ground applied to one NORMALIZED elevation `e` (the base formula's), for a patch of
/// `depth`: inside a pack the pack's ground replaces it (faded by the edge weight), with the
/// procedural relief finer than the level-12 lattice added on land away from any shore when
/// `detail` is given ([`MapSet::ground_with`], beds and pads included). `range_m` maps metres to
/// the normalized domain and `sea` is the normalized sea level. The tree gates pass no detail.
pub fn apply_normalized(set: &MapSet, d: DVec3, e: f32, depth: u8, range_m: f32, sea: f32, detail: Option<&crate::planet_chunks::DetailNoise>) -> f32 {
    let Some(g) = set.ground_with(d, depth, detail) else { return e.clamp(0.0, 1.0) };
    let e_pack = sea as f64 + g.h_m / range_m as f64;
    (e as f64 + (e_pack - e as f64) * g.weight).clamp(0.0, 1.0) as f32
}

/// The vegetation gates' elevation (the land, treeline and beach tests of the tree cards, the
/// near-tree models and the far forest, which must agree with one another): the map ground at
/// [`GATE_DEPTH`] with no beds or pads. A gate decides WHETHER a tree grows (the built-over mask
/// keeps them off roads and footprints anyway); where it stands comes from the drawn surface, beds
/// included.
pub fn apply_normalized_gate(set: &MapSet, d: DVec3, e: f32, range_m: f32, sea: f32) -> f32 {
    let Some(g) = set.ground_without_beds(d, GATE_DEPTH) else { return e.clamp(0.0, 1.0) };
    let e_pack = sea as f64 + g.h_m / range_m as f64;
    (e as f64 + (e_pack - e as f64) * g.weight).clamp(0.0, 1.0) as f32
}

// ── The global registry ──────────────────────────────────────────────────────────────────────

static MAP: RwLock<Option<Arc<MapSet>>> = RwLock::new(None);

/// Publish the installed set (world entry, or each frame with reuse off).
pub fn publish(set: Arc<MapSet>) {
    *MAP.write().expect("map ground lock poisoned") = Some(set);
}

/// Drop the set (world exit, tests).
pub fn clear() {
    *MAP.write().expect("map ground lock poisoned") = None;
}

/// One lock, one `Arc` clone: a patch build takes this once and passes it down.
pub fn snapshot() -> Option<Arc<MapSet>> {
    MAP.read().expect("map ground lock poisoned").clone()
}

/// Read every pack in `dir` (data/maps/packs/*.hmpack), in name order.
pub fn load_packs(dir: &std::path::Path) -> Vec<(Arc<Pack>, String)> {
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .map(|r| r.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == crate::map_pack::PACK_EXT)).collect())
        .unwrap_or_default();
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match std::fs::read(&p).map_err(|e| e.to_string()).and_then(Pack::open) {
            Ok(pack) => out.push((Arc::new(pack), name)),
            Err(e) => log::warn!("[MapGround] {}: not loaded: {e}", p.display()),
        }
    }
    out
}

/// Tests that publish a set take this first (the registry is process-wide, the harness parallel).
#[cfg(any(test, feature = "test-support"))]
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A set published for one test, cleared when dropped.
#[cfg(any(test, feature = "test-support"))]
pub struct PublishedForTest {
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(any(test, feature = "test-support"))]
impl PublishedForTest {
    pub fn publish(set: Arc<MapSet>) -> Self {
        let lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        publish(set);
        Self { _lock: lock }
    }
    pub fn empty() -> Self {
        let lock = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear();
        Self { _lock: lock }
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Drop for PublishedForTest {
    fn drop(&mut self) {
        clear();
    }
}

/// The shipped home packs as a set (data/maps/packs), for tests across the crates; None when the
/// packs are not installed.
#[cfg(any(test, feature = "test-support"))]
pub fn home_set() -> Option<Arc<MapSet>> {
    static SET: OnceLock<Option<Arc<MapSet>>> = OnceLock::new();
    SET.get_or_init(|| {
        let data = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
        let packs = load_packs(&data.join("maps").join("packs"));
        let levels = MapLevels::load(&data).ok()?;
        (!packs.is_empty()).then(|| Arc::new(MapSet::new(levels, packs)))
    })
    .clone()
}

#[cfg(test)]
#[path = "map_ground_tests.rs"]
mod tests;
