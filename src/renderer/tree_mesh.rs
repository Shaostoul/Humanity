//! Procedural planet-surface trees (v0.1066).
//!
//! Operator ask: "Can we see about adding a variety of trees? I'd love to see
//! cherry blossoms in Japan near mount fuji."
//!
//! Before this module the whole planet had exactly TWO tree species, chosen by
//! a hardcoded bit: `species_fir = ((r5 >> 9) & 1) == 0`, fir at 22 m and pine
//! at 16 m, everywhere from the tropics to the treeline. Both were photoscanned
//! glTF, and the release bundle does not ship `assets/models/`, so a downloaded
//! build had no near trees at all.
//!
//! Now `data/vegetation/trees.ron` is the species list (infinite-of-X: adding a
//! tree is a data row), and any species with an empty `model` field is built
//! HERE out of numbers instead of art. Procedural species therefore ship.
//!
//! Geometry follows the same rule the crop generator uses: flat-shaded
//! triangles with per-face colour packed into the UV, plus the organ tag in
//! spare bits, so trees render through material type 20 and pick up the
//! close-range leaf shading (venation, micro-relief, waxy cuticle, backlit
//! transmission) for free.
//!
//! ARCHITECTURE is annual-increment recursion, the same shape the design doc
//! (`docs/design/procedural-plants.md`) settles on: a limb extends, then spawns
//! children with less vigour, and leaves live on the OUTERMOST twigs only, so
//! the canopy is a shell rather than a solid block of foliage.

use super::plant_mesh::{Organ, PlantMeshBuilder};
use serde::Deserialize;

// ── Species data (deserialized from data/vegetation/trees.ron) ───────────

#[derive(Debug, Clone, Deserialize)]
pub struct TreeDef {
    pub id: String,
    pub display: String,
    /// glTF base name under `assets/models/plants/`. EMPTY = procedural.
    pub model: String,
    pub variants: u32,
    pub height_m: f32,
    pub height_jitter: f32,
    pub weight: f32,
    pub lat_min_deg: f32,
    pub lat_max_deg: f32,
    pub elev_min_m: f32,
    pub elev_max_m: f32,
    /// Region lock. `region_radius_km` of 0 disables it (species is global).
    pub region_lat_deg: f32,
    pub region_lon_deg: f32,
    pub region_radius_km: f32,
    /// conifer | broadleaf | umbrella | palm. Unknown falls back to broadleaf.
    pub form: String,
    pub trunk_color: [f32; 3],
    pub leaf_color: [f32; 3],
    pub blossom_color: [f32; 3],
    /// Fraction of leaf clusters replaced by blossom clusters (0 = never).
    pub blossom_frac: f32,
}

impl TreeDef {
    pub fn is_procedural(&self) -> bool {
        self.model.is_empty()
    }

    /// Unit direction of this species' region centre, or None when global.
    fn region_dir(&self) -> Option<[f64; 3]> {
        if self.region_radius_km <= 0.0 {
            return None;
        }
        let lat = (self.region_lat_deg as f64).to_radians();
        let lon = (self.region_lon_deg as f64).to_radians();
        let cl = lat.cos();
        // Matches the spawn site's dir construction in planet_chunks.rs:
        // (cos(lat)cos(lon), sin(lat), -cos(lat)sin(lon)).
        Some([cl * lon.cos(), lat.sin(), -cl * lon.sin()])
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TreeRegistry {
    pub trees: Vec<TreeDef>,
}

impl TreeRegistry {
    pub fn from_ron(text: &str) -> Result<Self, ron::error::SpannedError> {
        ron::from_str(text)
    }

    pub fn get(&self, idx: usize) -> Option<&TreeDef> {
        self.trees.get(idx)
    }

    pub fn len(&self) -> usize {
        self.trees.len()
    }

    pub fn is_empty(&self) -> bool {
        self.trees.is_empty()
    }

    /// Index of a species id, for callers that need a stable handle.
    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.trees.iter().position(|t| t.id == id)
    }

    /// Pick a species for a spawn cell. `dir` is the outward unit direction of
    /// the cell (already computed by the caller), `elev_m` its height above sea
    /// level, `planet_radius_m` the body radius so a region radius in km means
    /// the same thing on any planet, and `roll` a deterministic random word
    /// from the cell stream.
    ///
    /// Returns the registry index, so the picked species survives into
    /// `NearTree.species` and the caller can look up geometry later.
    ///
    /// Weighting is intentional: a region-locked species (sakura) carries a
    /// high weight so that inside its region it DOMINATES rather than showing
    /// up as one pink tree in a thousand firs.
    pub fn pick(
        &self,
        dir: [f64; 3],
        lat_deg: f32,
        elev_m: f32,
        planet_radius_m: f64,
        roll: u32,
    ) -> Option<usize> {
        let mut total = 0.0f32;
        // Two passes rather than an allocation: this runs per candidate cell.
        for t in &self.trees {
            if Self::gates_pass(t, dir, lat_deg, elev_m, planet_radius_m) {
                total += t.weight.max(0.0);
            }
        }
        if total <= 0.0 {
            return None;
        }
        let mut pick = (roll % 100_000) as f32 / 100_000.0 * total;
        for (i, t) in self.trees.iter().enumerate() {
            if !Self::gates_pass(t, dir, lat_deg, elev_m, planet_radius_m) {
                continue;
            }
            pick -= t.weight.max(0.0);
            if pick <= 0.0 {
                return Some(i);
            }
        }
        // Float drift only; fall back to the last species that passed.
        self.trees
            .iter()
            .enumerate()
            .filter(|(_, t)| Self::gates_pass(t, dir, lat_deg, elev_m, planet_radius_m))
            .map(|(i, _)| i)
            .next_back()
    }

    fn gates_pass(
        t: &TreeDef,
        dir: [f64; 3],
        lat_deg: f32,
        elev_m: f32,
        planet_radius_m: f64,
    ) -> bool {
        if lat_deg < t.lat_min_deg || lat_deg > t.lat_max_deg {
            return false;
        }
        if elev_m < t.elev_min_m || elev_m > t.elev_max_m {
            return false;
        }
        if let Some(c) = t.region_dir() {
            // Great-circle angle between the cell and the region centre.
            let d = (dir[0] * c[0] + dir[1] * c[1] + dir[2] * c[2]).clamp(-1.0, 1.0);
            let ang = d.acos();
            let max_ang = (t.region_radius_km as f64 * 1000.0) / planet_radius_m.max(1.0);
            if ang > max_ang {
                return false;
            }
        }
        true
    }
}

/// The shipped species list, compiled in so a build with no `data/` directory
/// still grows trees. The on-disk copy wins when present, which keeps the file
/// editable in a dev checkout.
const EMBEDDED_TREES: &str = include_str!("../../data/vegetation/trees.ron");

static REGISTRY: std::sync::OnceLock<TreeRegistry> = std::sync::OnceLock::new();

/// Process-wide species registry.
///
/// A OnceLock rather than a DataStore entry because the two species-picking
/// sites live in free functions in `terrain::planet_chunks` (the card bake and
/// the near-model mirror) with different signatures, and those two MUST agree
/// exactly or a tree changes species as you walk toward it.
pub fn registry() -> &'static TreeRegistry {
    REGISTRY.get_or_init(|| {
        let disk = std::fs::read_to_string("data/vegetation/trees.ron")
            .ok()
            .and_then(|t| TreeRegistry::from_ron(&t).ok());
        match disk {
            Some(r) if !r.is_empty() => r,
            _ => TreeRegistry::from_ron(EMBEDDED_TREES).unwrap_or_default(),
        }
    })
}

// ── Billboard-atlas tile allocation (v0.1083) ────────────────────────────
//
// Every (species, variant) owns one tile of the baked card atlas, and the tile
// index is a PURE FUNCTION OF THE REGISTRY:
//
//     base_tile(species_i) = sum of trees[0..species_i].variants.max(1)
//     tile(species_i, v)   = base_tile(species_i) + v
//
// It used to be a hand-written `sprite_tile` field in trees.ron (0 for fir, 3
// for pine, and a meaningless 0 on all six procedural rows). That field is
// GONE: a hand-written index that disagrees with the baker's is a silent
// wrong-species card, and there is nothing to deprecate before launch.
//
// Why a pure function and not an allocator that hands indices out at bake
// time: the card emitter runs on background patch-build threads that may
// already be in flight when the bake happens, so any bake-produced assignment
// would be a cross-thread ordering hazard. This form has no ordering at all,
// and it guarantees the CPU emitter and the GPU bake agree by construction.

/// Tiles the atlas can hold. Keep in lockstep with the shader decode - the
/// test `atlas_tile_constants_match_the_shader` scans the WGSL for them.
pub const ATLAS_TILES: u32 = super::billboard_bake::ATLAS_COLS * super::billboard_bake::ATLAS_ROWS;

/// First tile of each species, indexed by registry position.
fn tile_bases() -> &'static Vec<u32> {
    static BASES: std::sync::OnceLock<Vec<u32>> = std::sync::OnceLock::new();
    BASES.get_or_init(|| {
        let reg = registry();
        let mut out = Vec::with_capacity(reg.trees.len());
        let mut acc = 0u32;
        for t in &reg.trees {
            out.push(acc);
            acc += t.variants.max(1);
        }
        if acc > ATLAS_TILES {
            // A data file must never be able to corrupt the renderer: the
            // overflowing species simply get no tile (tile_of returns None and
            // the emitter falls back to coloured cards).
            log::error!(
                "[TreeAtlas] data/vegetation/trees.ron wants {acc} card tiles but the atlas holds \
                 {ATLAS_TILES} ({}x{}). Species past the ceiling render as coloured cards. Grow \
                 ATLAS_COLS/ATLAS_ROWS in billboard_bake.rs (and the matching shader decode).",
                super::billboard_bake::ATLAS_COLS,
                super::billboard_bake::ATLAS_ROWS,
            );
        }
        out
    })
}

/// How many tiles the shipped registry actually uses.
pub fn tiles_in_use() -> u32 {
    registry()
        .trees
        .iter()
        .map(|t| t.variants.max(1))
        .sum::<u32>()
}

/// Atlas tile for one (species index, variant), or None when the species is
/// unknown or its tile would fall outside the atlas. `variant` is clamped into
/// the species' own variant count, so a row with fewer variants than a caller
/// assumes can never bleed into the NEXT species' tiles.
pub fn tile_of(species_i: usize, variant: u32) -> Option<u32> {
    let reg = registry();
    let t = reg.get(species_i)?;
    let base = *tile_bases().get(species_i)?;
    let tile = base + variant.min(t.variants.max(1) - 1);
    (tile < ATLAS_TILES).then_some(tile)
}

// ── Card footprints, filled in by the bake (v0.1083, brief item 3b) ──────
//
// The baker frames a SQUARE on max(width, height) of the model's joint AABB,
// so a crown wider than the tree is tall does not fill its tile: drawing that
// tile as an `h` by `h` card (what `emit_sprite_card` did through v0.1082)
// renders an acacia about 27% too short with its trunk base 13% of the card
// height off the ground. The baker already computes everything needed to fix
// that; it just used to throw it away. Now it lands here, per tile.

/// World-space framing of one baked tile.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardFootprint {
    /// Side of the square frame the tile was baked with, in metres.
    pub frame_m: f32,
    /// Height of the tree that was baked, in metres (the scale reference).
    pub h_nominal_m: f32,
    /// Fraction of the frame between its bottom edge and the tree's base.
    pub base_offset: f32,
}

impl CardFootprint {
    /// Pre-bake default: frame == tree height, base on the frame's bottom edge.
    /// Reproduces the old `let w = h;` square card exactly, so a patch built
    /// before the bake lands looks like it always did (and its cards paint the
    /// flat fallback colour anyway until `tree_atlas_ready` flips).
    pub fn square(h_m: f32) -> Self {
        CardFootprint { frame_m: h_m.max(0.01), h_nominal_m: h_m.max(0.01), base_offset: 0.0 }
    }
}

type FootprintTable = [CardFootprint; ATLAS_TILES as usize];

fn footprints() -> &'static std::sync::RwLock<FootprintTable> {
    static FP: std::sync::OnceLock<std::sync::RwLock<FootprintTable>> = std::sync::OnceLock::new();
    FP.get_or_init(|| {
        let mut table = [CardFootprint::square(1.0); ATLAS_TILES as usize];
        for (i, t) in registry().trees.iter().enumerate() {
            for v in 0..t.variants.max(1) {
                if let Some(tile) = tile_of(i, v) {
                    table[tile as usize] = CardFootprint::square(t.height_m);
                }
            }
        }
        std::sync::RwLock::new(table)
    })
}

/// Snapshot of the whole table (576 bytes, no allocation). Patch builds take
/// ONE copy and then read it lock-free per card.
pub fn card_footprint_table() -> FootprintTable {
    match footprints().read() {
        Ok(g) => *g,
        Err(p) => *p.into_inner(),
    }
}

/// Record the real framing of a tile. Called by the baker, once per tile.
pub fn set_card_footprint(tile: u32, fp: CardFootprint) {
    if tile >= ATLAS_TILES {
        return;
    }
    let mut g = match footprints().write() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    g[tile as usize] = fp;
}

// ── Deterministic RNG (xorshift64*, same shape as plant_mesh) ────────────

struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).max(1))
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (self.next() >> 40) as f32 / ((1u64 << 24) as f32) * (hi - lo)
    }
}

// ── vector helpers ───────────────────────────────────────────────────────

fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
    [v[0] / l, v[1] / l, v[2] / l]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn add(a: [f32; 3], b: [f32; 3], s: f32) -> [f32; 3] {
    [a[0] + b[0] * s, a[1] + b[1] * s, a[2] + b[2] * s]
}

/// Rotate `v` away from `axis` by `deg`, around a stable perpendicular chosen
/// from `phase` so successive children fan out instead of stacking.
fn tilt(v: [f32; 3], deg: f32, phase: f32) -> [f32; 3] {
    let up = if v[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let t1 = norm(cross(v, up));
    let t2 = cross(v, t1);
    let side = norm([
        t1[0] * phase.cos() + t2[0] * phase.sin(),
        t1[1] * phase.cos() + t2[1] * phase.sin(),
        t1[2] * phase.cos() + t2[2] * phase.sin(),
    ]);
    let a = deg.to_radians();
    norm([
        v[0] * a.cos() + side[0] * a.sin(),
        v[1] * a.cos() + side[1] * a.sin(),
        v[2] * a.cos() + side[2] * a.sin(),
    ])
}

// ── Geometry ─────────────────────────────────────────────────────────────

/// Triangle ceiling per tree. Every triangle costs 108 bytes (faces are
/// unshared, the flat-shading contract), but a mesh is built ONCE PER
/// (species, variant) and then instanced by transform, so the resident cost is
/// ~18 meshes, not one per tree on screen.
///
/// The ceiling is a backstop, not a target: when it used to bite, it stopped
/// whole subtrees mid-recursion and the first branches ate the entire budget
/// while the rest of the crown came out bare. Branch tube cost is kept low
/// (see `sides_for`) precisely so foliage, not scaffolding, fills this.
const MAX_TRIS: usize = 8600;

/// Radial segments for a limb at `depth`. A twig does not need the 8 sides a
/// trunk does, and halving them here is what buys the crown its leaves.
///
/// These stay low on purpose: with SMOOTH normals (see `tri_smooth`) a 5-sided
/// branch shades like a cylinder, so sides buy silhouette only, and silhouette
/// at twig scale is a pixel wide.
fn sides_for(depth: u32) -> u32 {
    match depth {
        0 => 7,
        1 => 5,
        2 => 4,
        _ => 3,
    }
}

/// Lengthwise segments per limb: how finely the curved spine is sampled.
/// The trunk gets the most because its bow is the one you stand next to.
fn segments_for(depth: u32) -> u32 {
    match depth {
        0 => 4,
        1 => 3,
        _ => 2,
    }
}

/// Build one tree into `b`, centred on the origin with +Y up.
///
/// `height_m` is the FINAL height (the caller already applied per-instance
/// jitter), so the same species reads as a stand of different-aged trees.
pub fn build_tree(b: &mut PlantMeshBuilder, def: &TreeDef, height_m: f32, seed: u32) {
    let mut rng = Rng::new(seed as u64 ^ 0x7ee_5eed);
    let h = height_m.max(0.5);
    match def.form.as_str() {
        "conifer" => conifer(b, def, h, &mut rng),
        "umbrella" => umbrella(b, def, h, &mut rng),
        "palm" => palm(b, def, h, &mut rng),
        // Unknown forms fall back to broadleaf so a new data row always renders.
        _ => broadleaf(b, def, h, &mut rng),
    }
}

/// A bole: the clear trunk from the ground to the first branching. Curved and
/// ROOT-FLARED (v0.1067). The flare is the detail that reads as "a tree grew
/// here" rather than "a cylinder was placed here": real trunks swell sharply in
/// the last half-metre where they meet the ground, and a dead-straight
/// constant-taper post is an instant giveaway.
///
/// Returns the top of the bole so the caller can branch from it.
fn trunk(
    b: &mut PlantMeshBuilder,
    def: &TreeDef,
    base: [f32; 3],
    dir: [f32; 3],
    len: f32,
    r_base: f32,
    r_top_frac: f32,
) -> [f32; 3] {
    let segs = 6;
    let mut p = base;
    let mut d = dir;
    for s in 0..segs {
        let f0 = s as f32 / segs as f32;
        let f1 = (s + 1) as f32 / segs as f32;
        // Flare: an extra radius bump near the ground. Kept gentle and spread
        // over a longer run - a short sharp flare reads as a rocket fin, not
        // buttress roots.
        let flare = |f: f32| 1.0 + 0.28 * (1.0 - (f / 0.30).min(1.0)).powi(2);
        let ra = r_base * (1.0 + (r_top_frac - 1.0) * f0) * flare(f0);
        let rb = r_base * (1.0 + (r_top_frac - 1.0) * f1) * flare(f1);
        let to = add(p, d, len / segs as f32);
        b.tube(p, to, ra, rb, 8, def.trunk_color);
        p = to;
        // A very slight sway so the bole is not a plumb line.
        d = norm([d[0] + 0.012, d[1], d[2] - 0.008]);
    }
    p
}

/// A leaf cluster: several blades fanned around a twig tip. Individual leaves
/// on an 18 m tree would be sub-pixel and cost thousands of triangles, so one
/// "leaf" here stands for a clump of foliage, which is what every foliage
/// renderer does at this scale.
fn leaf_cluster(
    b: &mut PlantMeshBuilder,
    at: [f32; 3],
    dir: [f32; 3],
    size: f32,
    color: [f32; 3],
    n: u32,
    rng: &mut Rng,
) {
    for k in 0..n {
        let phase = k as f32 * 2.399_963 + rng.range(-0.4, 0.4);
        let d = tilt(dir, rng.range(30.0, 88.0), phase);
        // Blades hang slightly: gravity plus the weight of the clump.
        let d = norm([d[0], d[1] - 0.35, d[2]]);
        // Push each blade out from the twig so a clump reads as a spray of
        // foliage rather than a rosette pinned to one point.
        let off = add(at, d, size * rng.range(0.05, 0.45));
        blade(b, off, d, size * rng.range(0.7, 1.25), color, rng);
    }
}

/// One foliage blade: a double-sided diamond, FOUR triangles.
///
/// `PlantMeshBuilder::leaf` is a folded 8-quad fan at 16 triangles, which is
/// right for a crop you stand over and wrong for tree foliage: at clump scale
/// the fold is invisible, so those 16 triangles bought nothing while starving
/// the canopy. Four triangles per blade buys 4x the coverage for the same
/// budget, and coverage is the whole difference between a canopy and a few
/// leaves stapled to a stick. (Palm fronds still use the real `leaf`, where
/// the elongated shape genuinely reads.)
///
/// ORGAN TAG (v0.1081, the "black canopy" fix). `tri2` bakes whatever organ the
/// builder is currently set to into the packed UV, and it defaults to
/// `Organ::Stem`. Only `plant_mesh::leaf`/`petal` ever set `Organ::Leaf`, and
/// this function calls neither - so from v0.1066 to v0.1080 EVERY foliage face
/// on sakura, momiji, oak, birch and acacia (and every blossom, since
/// `leaf_cluster` routes `blossom_color` through here too) carried the stem tag.
/// Bit 19 was clear, `90-fragment-main.wgsl` computed `is_leaf = false`, and the
/// canopy was shaded by the BARK branch: stretched voronoi fissures, 0.42x
/// crevice darkening, roughness 0.78-0.96, and NO subsurface transmission at
/// all. Palm was the only species that looked right, because its fronds go
/// through `b.leaf()`. The same bit gates the leaf-flutter branch of the wind
/// vertex shader, so v0.1080's foliage wind had no flutter on these species
/// either. Set it here, reset after, exactly like the primitives do.
fn blade(
    b: &mut PlantMeshBuilder,
    at: [f32; 3],
    dir: [f32; 3],
    len: f32,
    color: [f32; 3],
    rng: &mut Rng,
) {
    let up = if dir[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let side = norm(cross(dir, up));
    let wid = len * rng.range(0.42, 0.68);
    let mid = add(at, dir, len * rng.range(0.38, 0.5));
    let tip = add(at, dir, len);
    let l = add(mid, side, -wid * 0.5);
    let r = add(mid, side, wid * 0.5);
    b.set_organ(Organ::Leaf);
    b.tri2(at, l, tip, color);
    b.tri2(at, tip, r, color);
    b.set_organ(Organ::Stem);
}

/// Recursive limb. Emits a tapered segment, then either children or foliage.
#[allow(clippy::too_many_arguments)]
fn limb(
    b: &mut PlantMeshBuilder,
    def: &TreeDef,
    from: [f32; 3],
    dir: [f32; 3],
    len: f32,
    r0: f32,
    depth: u32,
    max_depth: u32,
    leaf_size: f32,
    rng: &mut Rng,
) {
    if b.indices.len() / 3 > MAX_TRIS || len < 0.05 {
        return;
    }
    let r1 = r0 * 0.68;
    // A limb is a CURVED SPINE, not one straight frustum (v0.1067). Real
    // branches bow: they leave the parent at an angle, then gravity pulls the
    // far end down while the tip reaches back toward the light. Drawing that as
    // a single cone gave every junction a hard kink and every branch a dead
    // straight silhouette, which is most of what read as "early 2000s".
    let segs = segments_for(depth);
    let mut p = from;
    let mut d = dir;
    for s in 0..segs {
        let f0 = s as f32 / segs as f32;
        let f1 = (s + 1) as f32 / segs as f32;
        let seg_len = len / segs as f32;
        let to = add(p, d, seg_len);
        // Radius interpolates along the whole limb, so the taper is continuous
        // across segments instead of stepping at each one.
        let ra = r0 + (r1 - r0) * f0;
        let rb = r0 + (r1 - r0) * f1;
        b.tube(p, to, ra, rb, sides_for(depth), def.trunk_color);
        p = to;
        // Bow: droop grows toward the tip, and the trunk stays straighter than
        // the twigs (a bole that sagged would read as a sick tree).
        let droop = if depth == 0 { 0.04 } else { 0.10 } * f1;
        d = norm([d[0], d[1] - droop, d[2]]);
    }
    let to = p;

    // Foliage on the outer TWO generations, not just the tips: one generation
    // of leaf clumps leaves a crown you can see straight through.
    if depth + 1 >= max_depth {
        let blossom = def.blossom_frac > 0.0 && rng.range(0.0, 1.0) < def.blossom_frac;
        let color = if blossom { def.blossom_color } else { def.leaf_color };
        let tip = depth >= max_depth;
        let at = if tip { to } else { add(from, dir, len * 0.72) };
        let size = if tip { leaf_size } else { leaf_size * 0.78 };
        leaf_cluster(b, at, dir, size, color, if tip { 16 } else { 8 }, rng);
        if tip {
            return;
        }
    }

    // 2-3 children, fanned by the golden angle so they do not stack, with a
    // dominant leader early on (apical dominance) that eases off with depth.
    let n = if rng.range(0.0, 1.0) < 0.45 { 3 } else { 2 };
    for k in 0..n {
        let phase = k as f32 * 2.399_963 + rng.range(-0.5, 0.5) + depth as f32;
        let spread = rng.range(22.0, 42.0) - if depth == 0 { 6.0 } else { 0.0 };
        let d = tilt(dir, spread, phase);
        // Phototropism: every generation bends back toward vertical, which is
        // what rounds a crown instead of leaving it a flat fan.
        let d = norm([d[0], d[1] + 0.22, d[2]]);
        // Twigs shorten faster than limbs, so foliage clumps sit close together
        // instead of leaving long bare runs between them.
        let child_len = len * if depth + 2 >= max_depth { rng.range(0.42, 0.56) } else { rng.range(0.62, 0.78) };
        limb(b, def, to, d, child_len, r1, depth + 1, max_depth, leaf_size * 0.86, rng);
    }
}

fn broadleaf(b: &mut PlantMeshBuilder, def: &TreeDef, h: f32, rng: &mut Rng) {
    // A clear bole, then the crown. Cherry and maple branch low; oak higher.
    let bole = h * rng.range(0.26, 0.36);
    let r_base = h * 0.030;
    let lean = norm([rng.range(-0.06, 0.06), 1.0, rng.range(-0.06, 0.06)]);
    let top = trunk(b, def, [0.0, 0.0, 0.0], lean, bole, r_base, 0.74);
    // 3 primary limbs off the bole top. Three rather than four: each primary
    // costs a whole subtree, and the budget buys more by spending it on
    // foliage than on a fourth scaffold.
    let n = 3;
    // Clumps are ~10% of tree height. Individual leaves at true scale would be
    // sub-pixel and cost thousands of triangles, so one blade here stands for
    // a clump of foliage, which is what every foliage renderer does.
    let leaf_size = h * 0.105;
    for k in 0..n {
        let phase = k as f32 * 2.399_963 + rng.range(-0.3, 0.3);
        let d = tilt(lean, rng.range(26.0, 46.0), phase);
        limb(b, def, top, d, (h - bole) * rng.range(0.40, 0.52), r_base * 0.74, 0, 3, leaf_size, rng);
    }
}

fn conifer(b: &mut PlantMeshBuilder, def: &TreeDef, h: f32, rng: &mut Rng) {
    // A single straight leader with whorls of short, steeply drooping branches
    // that shorten toward the top: the classic conical silhouette.
    let r_base = h * 0.022;
    let top = [0.0, h, 0.0];
    b.tube([0.0, 0.0, 0.0], top, r_base, r_base * 0.16, 8, def.trunk_color);
    let whorls = 9;
    let leaf_size = h * 0.055;
    for w in 0..whorls {
        let f = 0.22 + 0.74 * (w as f32 / (whorls - 1) as f32);
        let y = h * f;
        // Branch length tapers linearly to the apex.
        let blen = h * 0.30 * (1.0 - f) + h * 0.03;
        let per = 5;
        for k in 0..per {
            let phase = (w * per + k) as f32 * 2.399_963;
            let d = tilt([0.0, 1.0, 0.0], rng.range(74.0, 96.0), phase);
            let d = norm([d[0], d[1] - 0.30, d[2]]);
            let tip = add([0.0, y, 0.0], d, blen);
            b.tube([0.0, y, 0.0], tip, r_base * 0.22, r_base * 0.07, 4, def.trunk_color);
            leaf_cluster(b, tip, d, leaf_size, def.leaf_color, 10, rng);
            // A second clump midway keeps the branch from reading as a bare stick.
            let mid = add([0.0, y, 0.0], d, blen * 0.55);
            leaf_cluster(b, mid, d, leaf_size * 0.8, def.leaf_color, 7, rng);
        }
    }
}

fn umbrella(b: &mut PlantMeshBuilder, def: &TreeDef, h: f32, rng: &mut Rng) {
    // Acacia: a tall bare bole, then limbs that flatten hard into a wide,
    // level crown. The giveaway is that the crown is WIDER than the tree is
    // tall and its underside is flat.
    let bole = h * rng.range(0.52, 0.62);
    let r_base = h * 0.034;
    let top = [0.0, bole, 0.0];
    b.tube([0.0, 0.0, 0.0], top, r_base, r_base * 0.66, 8, def.trunk_color);
    let n = 5;
    let leaf_size = h * 0.085;
    for k in 0..n {
        let phase = k as f32 * 2.399_963 + rng.range(-0.3, 0.3);
        // Steeply out, barely up.
        let d = tilt([0.0, 1.0, 0.0], rng.range(58.0, 74.0), phase);
        let seg = h * rng.range(0.30, 0.40);
        let mid = add(top, d, seg);
        b.tube(top, mid, r_base * 0.66, r_base * 0.34, 5, def.trunk_color);
        // The crown layer: near-horizontal fans of foliage.
        for j in 0..3 {
            let p2 = phase + j as f32 * 1.9;
            let d2 = tilt([0.0, 1.0, 0.0], rng.range(80.0, 94.0), p2);
            let tip = add(mid, d2, h * rng.range(0.16, 0.26));
            b.tube(mid, tip, r_base * 0.30, r_base * 0.12, 4, def.trunk_color);
            leaf_cluster(b, tip, [0.0, 1.0, 0.0], leaf_size, def.leaf_color, 16, rng);
        }
    }
}

fn palm(b: &mut PlantMeshBuilder, def: &TreeDef, h: f32, rng: &mut Rng) {
    // No branches at all and no secondary thickening: a palm is a single
    // unbranched stem with a crown of fronds at the top, and an old palm is
    // not a fatter palm. Modelled as a gently curved stack of segments.
    let segs = 7;
    let r_base = h * 0.028;
    let curve = rng.range(-0.10, 0.10);
    let mut p = [0.0f32, 0.0, 0.0];
    let mut d = norm([curve, 1.0, rng.range(-0.08, 0.08)]);
    for i in 0..segs {
        let f = i as f32 / segs as f32;
        let seg = h / segs as f32;
        let to = add(p, d, seg);
        b.tube(p, to, r_base * (1.0 - f * 0.35), r_base * (1.0 - (f + 0.15) * 0.35), 7, def.trunk_color);
        p = to;
        d = norm([d[0] + curve * 0.06, d[1], d[2]]);
    }
    // Crown: long fronds arching out and down.
    let frond = h * 0.34;
    for k in 0..15 {
        let phase = k as f32 * 2.399_963;
        let dd = tilt([0.0, 1.0, 0.0], rng.range(52.0, 88.0), phase);
        let dd = norm([dd[0], dd[1] - 0.28, dd[2]]);
        b.leaf(p, dd, frond, frond * 0.26, 0.9, def.leaf_color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> TreeRegistry {
        let text = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/vegetation/trees.ron"),
        )
        .expect("data/vegetation/trees.ron exists");
        TreeRegistry::from_ron(&text).expect("trees.ron parses")
    }

    const EARTH_R: f64 = 6_371_000.0;

    fn dir_of(lat_deg: f64, lon_deg: f64) -> [f64; 3] {
        let lat = lat_deg.to_radians();
        let lon = lon_deg.to_radians();
        let cl = lat.cos();
        [cl * lon.cos(), lat.sin(), -cl * lon.sin()]
    }

    #[test]
    fn shipped_registry_parses_and_covers_both_kinds() {
        let r = registry();
        assert!(r.len() >= 6, "expected a real species list, got {}", r.len());
        assert!(r.trees.iter().any(|t| !t.is_procedural()), "no model-backed species");
        assert!(r.trees.iter().any(|t| t.is_procedural()), "no procedural species");
        assert!(r.index_of("sakura").is_some(), "sakura missing");
    }

    /// The whole point of the region gate: cherry blossom near Fuji, nowhere else.
    #[test]
    fn sakura_is_local_to_fuji() {
        let r = registry();
        let sakura = r.index_of("sakura").unwrap();
        let fuji = dir_of(35.36, 138.73);
        // Sample many rolls at Fuji: sakura must be reachable there.
        let hit = (0..400).any(|i| r.pick(fuji, 35.36, 700.0, EARTH_R, i * 7919) == Some(sakura));
        assert!(hit, "sakura never picked at Fuji");

        // ...and unreachable at similar latitude/elevation on other continents.
        for (name, lat, lon) in [
            ("Oregon", 45.0, -122.0),
            ("Alps", 46.5, 8.0),
            ("Patagonia", -45.0, -71.0),
        ] {
            let d = dir_of(lat, lon);
            let any = (0..400).any(|i| r.pick(d, lat as f32, 700.0, EARTH_R, i * 7919) == Some(sakura));
            assert!(!any, "sakura leaked to {name}");
        }
    }

    /// Gates must actually restrict: no palms in the arctic, no birch at the equator.
    #[test]
    fn latitude_gates_hold() {
        let r = registry();
        let palm = r.index_of("palm").unwrap();
        let birch = r.index_of("birch").unwrap();
        let arctic = dir_of(68.0, 20.0);
        let equator = dir_of(2.0, 20.0);
        assert!(
            !(0..300).any(|i| r.pick(arctic, 68.0, 200.0, EARTH_R, i * 7919) == Some(palm)),
            "palm grew in the arctic"
        );
        assert!(
            !(0..300).any(|i| r.pick(equator, 2.0, 200.0, EARTH_R, i * 7919) == Some(birch)),
            "birch grew on the equator"
        );
    }

    /// Above every species' elevation ceiling nothing may be picked, which is
    /// what keeps the treeline a treeline.
    #[test]
    fn nothing_grows_above_every_ceiling() {
        let r = registry();
        let d = dir_of(45.0, 10.0);
        assert_eq!(r.pick(d, 45.0, 9000.0, EARTH_R, 12345), None);
    }

    /// Every procedural form must produce real, finite, budgeted geometry.
    #[test]
    fn every_procedural_form_builds_finite_geometry() {
        let r = registry();
        for t in r.trees.iter().filter(|t| t.is_procedural()) {
            for seed in [1u32, 2, 99] {
                let mut b = PlantMeshBuilder::new();
                build_tree(&mut b, t, t.height_m, seed);
                let tris = b.indices.len() / 3;
                assert!(tris > 40, "{} seed {seed}: only {tris} triangles", t.id);
                assert!(tris <= MAX_TRIS + 400, "{} seed {seed}: {tris} triangles blows the budget", t.id);
                assert!(
                    b.vertices.iter().all(|v| v.position.iter().all(|c| c.is_finite())),
                    "{}: non-finite vertex",
                    t.id
                );
                // Trees grow UP from the origin and must stay near their height.
                let top = b.vertices.iter().map(|v| v.position[1]).fold(f32::MIN, f32::max);
                assert!(
                    top > t.height_m * 0.35 && top < t.height_m * 1.8,
                    "{}: top {top} vs height {}",
                    t.id,
                    t.height_m
                );
            }
        }
    }

    /// EVERY procedural species must tag its foliage as leaf tissue.
    ///
    /// This is the unit-test half of the v0.1081 "black canopy" fix. From
    /// v0.1066 to v0.1080 `blade()` emitted through `tri2` without ever setting
    /// `Organ::Leaf`, so bit 19 was clear on every foliage face and the shader
    /// shaded the whole canopy as BARK (no subsurface transmission, no leaf
    /// flutter in the wind vertex shader, voronoi fissures on the leaves).
    /// Nothing caught it, because the geometry was perfectly valid - only the
    /// material tag was wrong. This asserts the tag directly off the packed UV
    /// so a future refactor of the blade primitive cannot silently lose it.
    ///
    /// Keep the constants in sync with `plant_mesh::ORGAN_BIT_LEAF` and the
    /// type-20 decode in `assets/shaders/pbr/90-fragment-main.wgsl`.
    #[test]
    fn every_procedural_species_tags_its_foliage_as_leaf() {
        const ORGAN_BIT_LEAF: u32 = 524_288;
        let r = registry();
        for t in r.trees.iter().filter(|t| t.is_procedural()) {
            let mut b = PlantMeshBuilder::new();
            build_tree(&mut b, t, t.height_m, 7);
            let total = b.vertices.len();
            let leaf = b
                .vertices
                .iter()
                .filter(|v| (v.uv[0].max(0.0).round() as u32) & ORGAN_BIT_LEAF != 0)
                .count();
            // Foliage is the bulk of a tree's triangles, so a healthy tag rate
            // is far above zero; 5% is a floor loose enough to survive form
            // tuning and tight enough that "nothing is tagged" always fails.
            assert!(
                leaf * 20 > total,
                "{}: only {leaf} of {total} vertices carry the leaf organ bit - \
                 blade()/leaf() stopped tagging foliage, so the shader will \
                 shade this species' canopy as BARK",
                t.id
            );
            // And stems must NOT be tagged, or the bark branch never runs.
            assert!(
                leaf < total,
                "{}: every vertex carries the leaf bit - set_organ was never reset to Stem",
                t.id
            );
        }
    }

    /// DEV AID (not run by default). Dumps a side-on SVG of every species so
    /// the silhouettes can be eyeballed without booting the game:
    ///   cargo test --features native --lib tree_mesh::tests::dump -- --ignored
    /// Writes to $TREE_DUMP or ./tree_dump.svg. Uses the REAL generator, so
    /// what you see is the geometry the planet gets.
    #[test]
    #[ignore]
    fn dump_species_svg() {
        let r = registry();
        let procedural: Vec<&TreeDef> = r.trees.iter().filter(|t| t.is_procedural()).collect();
        let (cw, ch) = (260.0f32, 340.0f32);
        let mut svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" \
             viewBox=\"0 0 {} {}\"><rect width=\"100%\" height=\"100%\" fill=\"#12160f\"/>",
            cw * procedural.len() as f32,
            ch + 40.0,
            cw * procedural.len() as f32,
            ch + 40.0
        );
        for (i, t) in procedural.iter().enumerate() {
            let mut b = PlantMeshBuilder::new();
            build_tree(&mut b, t, t.height_m, 11);
            let scale = (ch - 60.0) / t.height_m;
            let ox = cw * i as f32 + cw * 0.5;
            let oy = ch - 20.0;
            // Painter's algorithm: farthest (most negative z) drawn first.
            let mut faces: Vec<(f32, String)> = Vec::new();
            for f in b.indices.chunks(3) {
                let p: Vec<[f32; 3]> = f.iter().map(|&i| b.vertices[i as usize].position).collect();
                let z = (p[0][2] + p[1][2] + p[2][2]) / 3.0;
                let (c, _) = crate::terrain::planet_surface::unpack_uv_to_color(
                    b.vertices[f[0] as usize].uv,
                );
                // Cheap lambert against a fixed key so the form reads.
                let n = b.vertices[f[0] as usize].normal;
                let l = (n[0] * 0.4 + n[1] * 0.75 + n[2] * 0.5).max(0.0) * 0.75 + 0.35;
                let col = format!(
                    "rgb({},{},{})",
                    (c[0] * l * 255.0).min(255.0) as u32,
                    (c[1] * l * 255.0).min(255.0) as u32,
                    (c[2] * l * 255.0).min(255.0) as u32
                );
                let pts: Vec<String> = p
                    .iter()
                    .map(|v| {
                        format!("{:.1},{:.1}", ox + v[0] * scale + v[2] * scale * 0.25, oy - v[1] * scale)
                    })
                    .collect();
                faces.push((z, format!("<polygon points=\"{}\" fill=\"{}\"/>", pts.join(" "), col)));
            }
            faces.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            for (_, s) in faces {
                svg.push_str(&s);
            }
            svg.push_str(&format!(
                "<text x=\"{ox}\" y=\"{}\" fill=\"#cfe3bd\" font-family=\"monospace\" \
                 font-size=\"13\" text-anchor=\"middle\">{} ({:.0} m, {} tris)</text>",
                ch + 12.0,
                t.id,
                t.height_m,
                b.indices.len() / 3
            ));
        }
        svg.push_str("</svg>");
        let path = std::env::var("TREE_DUMP").unwrap_or_else(|_| "tree_dump.svg".to_string());
        std::fs::write(&path, svg).expect("write dump");
        eprintln!("wrote {path}");
    }

    /// The card bake and the near-model mirror both call `pick` with the same
    /// inputs. If it were not a pure function of those inputs, a tree would
    /// change species as you walked toward it - the exact defect the old
    /// duplicated `species_fir` expression was written to avoid.
    #[test]
    fn pick_is_deterministic() {
        let r = registry();
        for (lat, lon, elev) in [(35.36, 138.73, 700.0), (45.0, -122.0, 300.0), (5.0, 30.0, 100.0)] {
            let d = dir_of(lat, lon);
            for roll in [0u32, 1, 99, 123_456, u32::MAX] {
                let a = r.pick(d, lat as f32, elev, EARTH_R, roll);
                let b = r.pick(d, lat as f32, elev, EARTH_R, roll);
                assert_eq!(a, b, "pick differed for lat {lat} roll {roll}");
            }
        }
    }

    #[test]
    fn same_seed_is_deterministic_and_seeds_differ() {
        let r = registry();
        let t = r.get(r.index_of("sakura").unwrap()).unwrap();
        let gen = |s: u32| {
            let mut b = PlantMeshBuilder::new();
            build_tree(&mut b, t, t.height_m, s);
            b.vertices.len()
        };
        assert_eq!(gen(7), gen(7), "same seed must rebuild identically");
        assert_ne!(gen(7), gen(8), "different seeds must differ");
    }

    // ── Atlas tile allocation (v0.1083) ──────────────────────────────────

    /// The registry must fit the atlas. If someone adds a species row and this
    /// fails, grow ATLAS_COLS/ATLAS_ROWS in billboard_bake.rs AND the three
    /// literals in the shader decode (the lockstep test below catches half of
    /// that mistake; this one catches the other half).
    #[test]
    fn every_species_variant_gets_a_tile_inside_the_atlas() {
        let r = registry();
        let want: u32 = r.trees.iter().map(|t| t.variants.max(1)).sum();
        assert!(
            want <= ATLAS_TILES,
            "trees.ron needs {want} tiles, atlas holds {ATLAS_TILES}"
        );
        assert_eq!(want, tiles_in_use(), "tiles_in_use disagrees with the registry");
        // Contiguous from 0, unique, in registry order.
        let mut expect = 0u32;
        let mut seen = std::collections::HashSet::new();
        for (i, t) in r.trees.iter().enumerate() {
            for v in 0..t.variants.max(1) {
                let tile = tile_of(i, v).unwrap_or_else(|| panic!("{} v{v} has no tile", t.id));
                assert_eq!(tile, expect, "{} v{v}: tile {tile}, expected {expect}", t.id);
                assert!(seen.insert(tile), "tile {tile} handed out twice");
                expect += 1;
            }
        }
        // Out-of-range species/variants must not bleed into a neighbour.
        assert_eq!(tile_of(r.len(), 0), None, "a nonexistent species got a tile");
        let last = r.len() - 1;
        let nv = r.trees[last].variants.max(1);
        assert_eq!(
            tile_of(last, nv + 5),
            tile_of(last, nv - 1),
            "an over-range variant escaped its species' tile block"
        );
    }

    /// The two photoscans keep tiles 0-2 and 3-5, which is what the old
    /// hand-written `sprite_tile` field said. Documents the mapping so a
    /// reorder of trees.ron is a visible test change, not a silent card swap.
    #[test]
    fn conifer_tiles_match_the_historic_hand_written_indices() {
        let r = registry();
        let fir = r.index_of("fir").expect("fir row");
        let pine = r.index_of("pine").expect("pine row");
        assert_eq!(tile_of(fir, 0), Some(0));
        assert_eq!(tile_of(fir, 2), Some(2));
        assert_eq!(tile_of(pine, 0), Some(3));
        assert_eq!(tile_of(pine, 2), Some(5));
    }

    /// LOCKSTEP: the atlas grid is compile-time in TWO places - the Rust
    /// constants and the type-12 sprite decode in
    /// `assets/shaders/pbr/90-fragment-main.wgsl`. Neither is a uniform (a
    /// grid shape that can change per frame buys nothing and costs a slot),
    /// so this scans the shipped shader source for the exact literals the
    /// Rust side implies. Same idiom as renderer::clouds / water / atmosphere.
    #[test]
    fn atlas_tile_constants_match_the_shader() {
        use super::super::billboard_bake::{ATLAS_COLS, ATLAS_ROWS};
        let wgsl = crate::renderer::shader_loader::assembled_pbr_source();
        let expect = [
            // tile index range
            format!("clamp(u32(floor(a_enc)) - 1u, 0u, {}u)", ATLAS_TILES - 1),
            // column / row split
            format!("f32(tile % {ATLAS_COLS}u)"),
            format!("f32(tile / {ATLAS_COLS}u)"),
            // normalisation into 0..1 atlas UV
            format!(") / {ATLAS_COLS}.0"),
            format!(") / {ATLAS_ROWS}.0"),
        ];
        for e in expect {
            assert!(
                wgsl.contains(&e),
                "the tree-atlas sprite decode in 90-fragment-main.wgsl does not contain `{e}`.\n\
                 ATLAS_COLS={ATLAS_COLS}, ATLAS_ROWS={ATLAS_ROWS}, ATLAS_TILES={ATLAS_TILES} in \
                 billboard_bake.rs - the shader's three literals must match or every card samples \
                 the wrong tile."
            );
        }
    }

    /// Before the bake runs, every tile must frame exactly like the old
    /// `let w = h;` square card, so nothing changes shape until real
    /// footprints land - and after the bake writes one, the table returns it.
    #[test]
    fn card_footprints_default_to_the_square_frame_and_accept_bake_values() {
        let r = registry();
        let oak = r.index_of("oak").expect("oak row");
        let tile = tile_of(oak, 1).expect("oak v1 tile");
        let h = r.get(oak).unwrap().height_m;
        let before = card_footprint_table()[tile as usize];
        assert_eq!(before, CardFootprint::square(h), "default footprint is not the square frame");
        // side = frame_m * (h / h_nominal_m) must reduce to h at nominal size.
        assert!((before.frame_m * (h / before.h_nominal_m) - h).abs() < 1e-3);
        let baked = CardFootprint { frame_m: 12.5, h_nominal_m: 9.0, base_offset: 0.134 };
        set_card_footprint(tile, baked);
        assert_eq!(card_footprint_table()[tile as usize], baked);
        // Restore so test order cannot matter.
        set_card_footprint(tile, before);
    }

    /// Blossom species must actually emit petal-coloured faces, or "cherry
    /// blossom" is just a green tree with a pink entry in a data file.
    #[test]
    fn sakura_emits_blossom_coloured_faces() {
        let r = registry();
        let t = r.get(r.index_of("sakura").unwrap()).unwrap();
        let mut b = PlantMeshBuilder::new();
        build_tree(&mut b, t, t.height_m, 5);
        // Unpack each face colour and look for the pink.
        let mut pink = 0;
        for f in b.indices.chunks(3) {
            let uv = b.vertices[f[0] as usize].uv;
            let (c, _) = crate::terrain::planet_surface::unpack_uv_to_color(uv);
            if c[0] > 0.85 && c[1] > 0.55 && c[1] < 0.9 && c[2] > 0.6 {
                pink += 1;
            }
        }
        assert!(pink > 30, "only {pink} blossom faces on a sakura");
    }
}
