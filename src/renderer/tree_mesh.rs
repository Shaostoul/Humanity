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

/// How much of MAX_TRIS the density fit aims to fill. Not 1.0: the second
/// pass' leaf count is predicted exactly but the per-sprig fractional coin
/// toss lands a few dozen triangles either side of the prediction.
const BUDGET_TARGET: f32 = 0.96;

/// Build one tree into `b`, centred on the origin with +Y up.
///
/// `height_m` is the FINAL height (the caller already applied per-instance
/// jitter), so the same species reads as a stand of different-aged trees.
///
/// TWO PASSES, because "spend the budget" cannot be hand-tuned (v0.1086).
/// Foliage density is the one knob that decides whether a canopy reads as
/// foliage, and the right value depends on the species height, the form, AND
/// which way the recursion's coin flips landed for this particular variant -
/// the same broadleaf constants produced 5602 triangles on one variant and
/// 7076 on the next, i.e. anything hand-picked has to be tuned for the WORST
/// variant and then every other variant ships a third of its budget unspent.
///
/// So: build once at the form's baseline density, measure how many triangles
/// went to leaves and how many to wood, and rebuild once at the density that
/// lands on `BUDGET_TARGET`. This is exact rather than iterative because leaf
/// count is strictly linear in the density knob and the WOOD is completely
/// unaffected by it - `leaf_cluster` gives each sprig its own RNG stream, so
/// changing how many leaves a sprig draws cannot move a branch. Cost is one
/// extra mesh build for 24 meshes, once, at world entry.
pub fn build_tree(b: &mut PlantMeshBuilder, def: &TreeDef, height_m: f32, seed: u32) {
    let h = height_m.max(0.5);
    let first = build_at_density(def, h, seed, 1.0);
    let total = first.indices.len() / 3;
    let leaves = leaf_tri_count(&first);
    let wood = total.saturating_sub(leaves);
    let room = MAX_TRIS as f32 * BUDGET_TARGET - wood as f32;
    let best = if leaves > 0 && room > 0.0 {
        let scale = (room / leaves as f32).clamp(0.2, 8.0);
        // Only pay for a second build when it actually buys something.
        if !(0.97..=1.03).contains(&scale) {
            let second = build_at_density(def, h, seed, scale);
            let st = second.indices.len() / 3;
            if st <= MAX_TRIS && (st > total || total > MAX_TRIS) {
                second
            } else {
                first
            }
        } else {
            first
        }
    } else {
        first
    };
    // Merge (the callers hand us a fresh builder, but never assume that).
    let base = b.vertices.len() as u32;
    b.vertices.extend_from_slice(&best.vertices);
    b.indices.extend(best.indices.iter().map(|i| i + base));
}

/// One build pass at a given foliage density multiplier.
fn build_at_density(def: &TreeDef, h: f32, seed: u32, density: f32) -> PlantMeshBuilder {
    let mut b = PlantMeshBuilder::new();
    let mut rng = Rng::new(seed as u64 ^ 0x7ee_5eed);
    match def.form.as_str() {
        "conifer" => conifer(&mut b, def, h, density, &mut rng),
        "umbrella" => umbrella(&mut b, def, h, density, &mut rng),
        "palm" => palm(&mut b, def, h, density, &mut rng),
        // Unknown forms fall back to broadleaf so a new data row always renders.
        _ => broadleaf(&mut b, def, h, density, &mut rng),
    }
    b
}

/// Triangles carrying the leaf organ bit (19), i.e. the part of the mesh the
/// density knob controls.
fn leaf_tri_count(b: &PlantMeshBuilder) -> usize {
    const ORGAN_BIT_LEAF: u32 = 524_288;
    b.indices
        .chunks(3)
        .filter(|f| (b.vertices[f[0] as usize].uv[0].max(0.0).round() as u32) & ORGAN_BIT_LEAF != 0)
        .count()
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
        let seg = len / segs as f32;
        let to = add(p, d, seg);
        // Same joint overshoot as `limb`: the bole sways slightly between
        // segments, and without the overlap each sway opens a hairline slit.
        b.tube(p, add(p, d, seg + rb * 0.5), ra, rb, 8, def.trunk_color);
        p = to;
        // A very slight sway so the bole is not a plumb line.
        d = norm([d[0] + 0.012, d[1], d[2] - 0.008]);
    }
    p
}

// ── Foliage at real leaf scale (v0.1086) ─────────────────────────────────
//
// From v0.1066 to v0.1085 the generator carried exactly ONE foliage number,
// `leaf_size` (~10% of tree height), and used it for two unrelated jobs: how
// far foliage spreads around a twig, AND how big one drawn leaf is. On an 18 m
// oak that made every drawn leaf a 0.76-1.5 m kite. A real oak leaf is
// 0.10-0.15 m, so every leaf was TEN TIMES oversized, and that is the single
// loudest reason the canopy never read as a canopy: at any distance where a
// leaf resolves at all, it resolves as a tarpaulin.
//
// The fix is to split the two numbers and spend the triangle budget that was
// already allocated and sitting unused - before this change the forms used
// 4% (palm), 13% (acacia), 40% (conifer) and 49-55% (broadleaf) of MAX_TRIS.

/// Foliage parameters for one species, in REAL METRES.
#[derive(Clone, Copy)]
struct Foliage {
    /// Radius of the foliage clump a twig carries. Scales with the tree, and
    /// shrinks toward the inner generations - this is the old `leaf_size`, and
    /// it still sets the SHAPE of the crown.
    clump: f32,
    /// Length of ONE drawn leaf element, metres. True to life, and constant
    /// across the whole tree: a leaf on an inner twig is the same size as a
    /// leaf at the tip. That constancy is the entire point of the split.
    leaf: f32,
    /// Leaf width as a fraction of its length. Broadleaves are broad (~0.6);
    /// a conifer needle spray is a narrow strap (~0.3).
    wid: f32,
    /// Baseline elements per sprig - per position that used to hold one kite.
    per_sprig: u32,
    /// Multiplier on `per_sprig`, chosen by `build_tree`'s second pass so the
    /// species lands on its triangle budget instead of near it. Fractional
    /// values are honoured by a per-sprig coin toss, so density is continuous.
    density: f32,
}

impl Foliage {
    /// Shrink the CLUMP only, for inner generations. Leaf size is real and
    /// therefore never scales.
    fn with_clump(self, k: f32) -> Self {
        Foliage { clump: self.clump * k, ..self }
    }

    /// How far a sprig's leaves spread along their shoot.
    ///
    /// Measured in LEAF LENGTHS, not in clump radii, and this is the single
    /// most important number for whether a canopy reads as foliage. The first
    /// cut of the real-scale rewrite spread each sprig over the full run the
    /// old kite used to fill (up to 2.4 m on an oak) and put three 0.16 m
    /// leaves along it: one leaf per 0.8 m of empty air, which the probe
    /// capture showed as a fine mist of dots rather than leaves. At half a leaf
    /// length of spacing the leaves in a sprig OVERLAP by 2x, so each sprig
    /// reads as one leafy tuft - and the crown gets its volume back from
    /// scattering those tufts through the clump instead of from stretching one.
    fn sprig_span(self) -> f32 {
        self.leaf * self.leaves_per_sprig_mean().max(1.0) * 0.5
    }

    /// Mean leaves on one sprig at the current density.
    fn leaves_per_sprig_mean(self) -> f32 {
        (self.per_sprig as f32 * self.density).clamp(1.0, 64.0)
    }
}

/// A leaf cluster: several SPRIGS fanned around a twig tip.
///
/// The skeleton is unchanged from v0.1066 - `n` positions fanned by the golden
/// angle, pushed out from the twig and drooping. What changed is what sits at
/// each position: one oversized kite became a shoot carrying `per_sprig`
/// real-scale leaves.
fn leaf_cluster(
    b: &mut PlantMeshBuilder,
    at: [f32; 3],
    dir: [f32; 3],
    fol: Foliage,
    color: [f32; 3],
    n: u32,
    rng: &mut Rng,
) {
    for k in 0..n {
        let phase = k as f32 * 2.399_963 + rng.range(-0.4, 0.4);
        let d = tilt(dir, rng.range(30.0, 88.0), phase);
        // Sprigs hang slightly: gravity plus the weight of the foliage.
        let d = norm([d[0], d[1] - 0.35, d[2]]);
        // Scatter the sprigs through the WHOLE clump volume, not just its inner
        // half: the clump radius is what carries the crown's silhouette now
        // that no single leaf spans it.
        let off = add(at, d, fol.clump * rng.range(0.08, 0.98));
        let span = fol.sprig_span() * rng.range(0.85, 1.2);
        // Each sprig gets its OWN stream, seeded from one draw of the parent's.
        // That decouples leaf COUNT from everything upstream: `build_tree` can
        // rebuild at any density and get bit-identical wood, which is what
        // makes its one-shot budget fit exact instead of iterative.
        let mut srng = Rng::new(rng.next());
        sprig(b, off, d, span, fol, color, &mut srng);
    }
}

/// One sprig: a shoot's worth of real-scale leaves spaced along `span` and
/// spiralled around it by the golden angle, the way leaves actually sit on a
/// twig. No shoot geometry is drawn - a 3-sided tube per sprig would cost more
/// triangles than the leaves it carries, and at leaf scale it is invisible.
fn sprig(
    b: &mut PlantMeshBuilder,
    at: [f32; 3],
    dir: [f32; 3],
    span: f32,
    fol: Foliage,
    color: [f32; 3],
    rng: &mut Rng,
) {
    // Fractional density: `want` leaves on average, resolved to an integer by
    // one coin toss per sprig so a density of 4.6 gives a real 4.6 mean.
    let want = fol.leaves_per_sprig_mean();
    let m = (want.floor() as u32 + u32::from(rng.range(0.0, 1.0) < want.fract())).max(1);
    for j in 0..m {
        // Spread the leaves along the shoot, biased away from its very base.
        let f = (j as f32 + 0.55) / m as f32;
        let node = add(at, dir, span * f);
        let phase = j as f32 * 2.399_963 + rng.range(-0.45, 0.45);
        // Leaves stand well off the shoot axis, then droop under their weight.
        let ld = tilt(dir, rng.range(48.0, 104.0), phase);
        let ld = norm([ld[0], ld[1] - 0.30, ld[2]]);
        blade(b, node, ld, fol.leaf * rng.range(0.75, 1.25), fol.wid, color, rng);
    }
}

/// One leaf: a double-sided triangle, TWO triangles.
///
/// It used to be a four-triangle diamond. Halving it is what pays for four
/// times as many leaves in the same budget, and a triangle is the more
/// area-efficient shape anyway: a double-sided triangle covers `w*len/2` for 2
/// triangles where the diamond covered ~`w*len*0.6` for 4, so per triangle the
/// triangle wins by 1.7x. At 0.10-0.20 m the silhouette difference between a
/// deltoid leaf and a kite is far below one pixel; the COUNT is what reads.
///
/// MIDRIB ROLL (v0.1086). The old blade built its width axis from
/// `cross(dir, world_up)`, which pins every leaf's plane to the same family of
/// orientations - the measured result was 74-78% of visible leaf faces showing
/// their camera-facing UNDERSIDE, against 61% for a real spherical leaf-angle
/// distribution, so the canopy lit flat and wrong. Rolling each leaf by a
/// random angle about its own midrib makes the face-normal azimuth uniform
/// about the midrib, which lands all three measured statistics on the
/// spherical reference. It costs one sine and one cosine per leaf.
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
/// all. The same bit gates the leaf-flutter branch of the wind vertex shader,
/// so v0.1080's foliage wind had no flutter on these species either. Set it
/// here, reset after, exactly like the primitives do.
fn blade(
    b: &mut PlantMeshBuilder,
    at: [f32; 3],
    dir: [f32; 3],
    len: f32,
    wid_frac: f32,
    color: [f32; 3],
    rng: &mut Rng,
) {
    // Orthonormal frame about the midrib, then roll the blade plane around it.
    let up = if dir[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let s1 = norm(cross(dir, up));
    let s2 = norm(cross(dir, s1));
    let roll = rng.range(0.0, std::f32::consts::TAU);
    let (sr, cr) = (roll.sin(), roll.cos());
    let side = norm([
        s1[0] * cr + s2[0] * sr,
        s1[1] * cr + s2[1] * sr,
        s1[2] * cr + s2[2] * sr,
    ]);
    let wid = len * wid_frac * rng.range(0.82, 1.18);
    let tip = add(at, dir, len);
    let l = add(at, side, -wid * 0.5);
    let r = add(at, side, wid * 0.5);
    b.set_organ(Organ::Leaf);
    b.tri2(l, tip, r, color);
    b.set_organ(Organ::Stem);
}

// ── Seam welding (v0.1086, RUNG B) ───────────────────────────────────────
//
// `PlantMeshBuilder::tube` emits a side wall and NO end caps, so every limb is
// an open pipe. Where a child left its parent exactly at the parent's tip, the
// two open rings had the same radius but different planes: they cross at two
// points and gape everywhere else, so you could see straight down the inside
// of both tubes. That is the "visible open seam" the operator has reported
// three times.
//
// The robust fix is not exact ring sharing (which needs a shared vertex ring
// and breaks the moment a limb curves): it is to start the child's root ring
// INSIDE the parent's solid. Push the ring back along the child's own axis by
// at least the parent's local radius and two things become true at once:
//   1. the child's ring is enclosed by the parent's wall, so its open end is
//      never visible; and
//   2. the child's tube crosses the parent's end plane, where its cross
//      section is an ellipse with semi-axes r and r/cos(angle) - which always
//      contains the parent's end disc of radius r, so the parent's open tip is
//      plugged too.
// It costs ZERO extra triangles: the limb's first spine segment simply starts
// further back and is that much longer.

/// How many parent radii to bury a branch root. 1.0 is the minimum that makes
/// the junction watertight; the margin covers the parent's taper.
const WELD_EMBED: f32 = 1.4;

/// Root point for a limb leaving a parent of local radius `parent_r`, plus how
/// far it was buried. Pure geometry so it can be unit-tested directly.
fn welded_root(from: [f32; 3], dir: [f32; 3], parent_r: f32, r0: f32, len: f32) -> ([f32; 3], f32) {
    // Never bury more than half the limb: a twig shorter than its parent is
    // thick would otherwise vanish inside it.
    let embed = (parent_r.max(r0) * WELD_EMBED).min(len * 0.5);
    (add(from, dir, -embed), embed)
}

/// Recursive limb. Emits a tapered segment, then either children or foliage.
///
/// `parent_r` is the radius of the limb this one leaves, at the junction; it
/// drives the seam weld (see `welded_root`). A root limb passes the radius of
/// whatever it grows out of - the bole top.
#[allow(clippy::too_many_arguments)]
fn limb(
    b: &mut PlantMeshBuilder,
    def: &TreeDef,
    from: [f32; 3],
    dir: [f32; 3],
    len: f32,
    r0: f32,
    parent_r: f32,
    depth: u32,
    max_depth: u32,
    fol: Foliage,
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
    // Seam weld: the spine starts BURIED in the parent and the first segment
    // absorbs the extra length, so the tip lands exactly where it always did
    // and no triangle is added.
    let (start, embed) = welded_root(from, dir, parent_r, r0, len);
    let mut p = start;
    let mut d = dir;
    let mut x = 0.0f32; // distance travelled from the buried root
    // Taper measured from `from`, so the buried root keeps the full r0 and the
    // VISIBLE limb tapers exactly as it did before the weld.
    let taper = |x: f32| ((x - embed) / len.max(1e-4)).clamp(0.0, 1.0);
    for s in 0..segs {
        let seg_len = len / segs as f32 + if s == 0 { embed } else { 0.0 };
        let to = add(p, d, seg_len);
        let ra = r0 + (r1 - r0) * taper(x);
        let rb = r0 + (r1 - r0) * taper(x + seg_len);
        // Overshoot the joint by a fraction of the radius so the kink between
        // two spine segments cannot open a slit on the outside of the bow. The
        // spine itself still advances by exactly `seg_len`.
        b.tube(p, add(p, d, seg_len + rb * 0.7), ra, rb, sides_for(depth), def.trunk_color);
        p = to;
        x += seg_len;
        // Bow: droop grows toward the tip, and the trunk stays straighter than
        // the twigs (a bole that sagged would read as a sick tree).
        let droop = if depth == 0 { 0.04 } else { 0.10 } * taper(x);
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
        let f = if tip { fol } else { fol.with_clump(0.78) };
        leaf_cluster(b, at, dir, f, color, if tip { 16 } else { 8 }, rng);
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
        // The child's radius equals the parent's tip radius, so `r1` is both
        // its own r0 and the parent radius it must bury itself in.
        limb(b, def, to, d, child_len, r1, r1, depth + 1, max_depth, fol.with_clump(0.86), rng);
    }
}

fn broadleaf(b: &mut PlantMeshBuilder, def: &TreeDef, h: f32, density: f32, rng: &mut Rng) {
    // A clear bole, then the crown. Cherry and maple branch low; oak higher.
    let bole = h * rng.range(0.26, 0.36);
    let r_base = h * 0.030;
    let lean = norm([rng.range(-0.06, 0.06), 1.0, rng.range(-0.06, 0.06)]);
    let top = trunk(b, def, [0.0, 0.0, 0.0], lean, bole, r_base, 0.74);
    // 3 primary limbs off the bole top. Three rather than four: each primary
    // costs a whole subtree, and the budget buys more by spending it on
    // foliage than on a fourth scaffold.
    let n = 3;
    // Clumps stay at ~10% of tree height (the crown SHAPE is unchanged), but a
    // clump is now filled with real leaves instead of being one. Leaf length
    // rides on the species height so an oak leaf is bigger than a maple leaf
    // without inventing a data field: the band 0.09-0.18 m brackets every real
    // temperate broadleaf (oak 0.10-0.15, cherry ~0.09, birch ~0.06, and the
    // low clamp keeps the smallest ones from vanishing at draw scale).
    // Leaf length rides on the species height so an oak leaf is bigger than a
    // maple leaf without inventing a data field, and it sits at the TOP of the
    // real range for each: an oak leaf is 0.10-0.22 m and a big-leaf oak more,
    // so 0.20 m is honest, and every square centimetre of honest leaf is
    // canopy coverage that costs no extra triangle. The 0.09 m floor is where
    // cherry and maple land, and they are genuinely that small.
    let fol = Foliage {
        clump: h * 0.092,
        leaf: (h * 0.011).clamp(0.09, 0.22),
        wid: 0.70,
        per_sprig: 3,
        density,
    };
    for k in 0..n {
        let phase = k as f32 * 2.399_963 + rng.range(-0.3, 0.3);
        let d = tilt(lean, rng.range(26.0, 46.0), phase);
        // The bole top is the parent: burying the primaries in it also plugs
        // the bole's open top ring, which used to be a hole you could see the
        // sky through from above.
        let bole_top_r = r_base * 0.74;
        limb(b, def, top, d, (h - bole) * rng.range(0.40, 0.52), bole_top_r, bole_top_r, 0, 3, fol, rng);
    }
}

fn conifer(b: &mut PlantMeshBuilder, def: &TreeDef, h: f32, density: f32, rng: &mut Rng) {
    // A single straight leader with whorls of short, steeply drooping branches
    // that shorten toward the top: the classic conical silhouette.
    let r_base = h * 0.022;
    let top = [0.0, h, 0.0];
    // Leader radius at height fraction `f`, so a whorl branch can size itself
    // against the trunk it leaves instead of against the trunk BASE. With the
    // old flat `r_base * 0.22`, the topmost whorl's root ring (0.22 r_base) was
    // FATTER than the leader there (0.194 r_base) and stuck out through it.
    let leader_r = |f: f32| r_base * (1.0 + (0.16 - 1.0) * f);
    b.tube([0.0, 0.0, 0.0], top, r_base, r_base * 0.16, 8, def.trunk_color);
    // Close the leader's open apex ring with a short cone (16 triangles). It is
    // the one hole on a conifer you look straight down into from the air.
    b.tube(top, [0.0, h + h * 0.012, 0.0], r_base * 0.16, 0.0, 8, def.trunk_color);
    let whorls = 9;
    // A conifer's drawn element is a NEEDLE SPRAY, not a single needle: one
    // 30 mm needle would be far below a pixel and there is no budget for the
    // ~200k of them a real fir carries. 0.30-0.50 m of narrow strap is the
    // honest unit, and it is 3x smaller than the 1.2 m kite it replaces.
    let fol = Foliage {
        clump: h * 0.050,
        leaf: (h * 0.022).clamp(0.30, 0.50),
        wid: 0.40,
        per_sprig: 4,
        density,
    };
    for w in 0..whorls {
        let f = 0.22 + 0.74 * (w as f32 / (whorls - 1) as f32);
        let y = h * f;
        let lr = leader_r(f);
        // Branch length tapers linearly to the apex.
        let blen = h * 0.30 * (1.0 - f) + h * 0.03;
        let per = 5;
        for k in 0..per {
            let phase = (w * per + k) as f32 * 2.399_963;
            let d = tilt([0.0, 1.0, 0.0], rng.range(74.0, 96.0), phase);
            let d = norm([d[0], d[1] - 0.30, d[2]]);
            let tip = add([0.0, y, 0.0], d, blen);
            // Rooted ON the leader's axis, which is as buried as a root ring
            // can get, and at 0.42 of the local leader radius it is strictly
            // inside the trunk wall at every height.
            b.tube([0.0, y, 0.0], tip, lr * 0.42, lr * 0.14, 4, def.trunk_color);
            leaf_cluster(b, tip, d, fol, def.leaf_color, 10, rng);
            // A second clump midway keeps the branch from reading as a bare stick.
            let mid = add([0.0, y, 0.0], d, blen * 0.55);
            leaf_cluster(b, mid, d, fol.with_clump(0.8), def.leaf_color, 7, rng);
        }
    }
}

fn umbrella(b: &mut PlantMeshBuilder, def: &TreeDef, h: f32, density: f32, rng: &mut Rng) {
    // Acacia: a tall bare bole, then limbs that flatten hard into a wide,
    // level crown. The giveaway is that the crown is WIDER than the tree is
    // tall and its underside is flat.
    let bole = h * rng.range(0.52, 0.62);
    let r_base = h * 0.034;
    let top = [0.0, bole, 0.0];
    b.tube([0.0, 0.0, 0.0], top, r_base, r_base * 0.66, 8, def.trunk_color);
    let n = 5;
    // An acacia leaf is bipinnate: what reads at any real distance is a pinna,
    // a 0.10-0.18 m feather of leaflets, not a metre-wide blade. Acacia was the
    // most under-spent form in the tree budget (13% of MAX_TRIS), so it can
    // afford by far the densest sprigs, which is also what its flat crown wants.
    let fol = Foliage {
        clump: h * 0.080,
        leaf: (h * 0.016).clamp(0.10, 0.20),
        wid: 0.58,
        per_sprig: 14,
        density,
    };
    for k in 0..n {
        let phase = k as f32 * 2.399_963 + rng.range(-0.3, 0.3);
        // Steeply out, barely up.
        let d = tilt([0.0, 1.0, 0.0], rng.range(58.0, 74.0), phase);
        let seg = h * rng.range(0.30, 0.40);
        let mid = add(top, d, seg);
        // Bury the primary in the bole (this also plugs the bole's open top).
        // The root radius is a touch FATTER than the bole top (0.70 vs 0.66)
        // because these tubes taper across the buried run: the extra covers the
        // taper so the limb is still wider than the hole where it crosses it,
        // and it reads as a branch collar, which is anatomically right anyway.
        let (root, _) = welded_root(top, d, r_base * 0.66, r_base * 0.70, seg);
        b.tube(root, mid, r_base * 0.70, r_base * 0.34, 5, def.trunk_color);
        // The crown layer: near-horizontal fans of foliage.
        for j in 0..3 {
            let p2 = phase + j as f32 * 1.9;
            let d2 = tilt([0.0, 1.0, 0.0], rng.range(80.0, 94.0), p2);
            let flen = h * rng.range(0.16, 0.26);
            let tip = add(mid, d2, flen);
            // Same again one level down: the fan is wider than the primary's
            // open tip ring, so burying it plugs that ring completely.
            let (froot, _) = welded_root(mid, d2, r_base * 0.34, r_base * 0.38, flen);
            b.tube(froot, tip, r_base * 0.38, r_base * 0.12, 4, def.trunk_color);
            leaf_cluster(b, tip, [0.0, 1.0, 0.0], fol, def.leaf_color, 16, rng);
        }
    }
}

fn palm(b: &mut PlantMeshBuilder, def: &TreeDef, h: f32, density: f32, rng: &mut Rng) {
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
    // Cap the stem's open top ring; the crown hides it from the side but not
    // from above, and a palm is a thing you fly over.
    let cap_r = r_base * 0.65;
    b.tube(p, add(p, d, cap_r * 0.8), cap_r, 0.0, 7, def.trunk_color);
    // Crown: long fronds arching out and down. A frond is PINNATE - a rachis
    // carrying two ranks of strap leaflets - not one solid blade. The old code
    // drew each of 15 fronds as a single `b.leaf`, i.e. a 4 m x 1 m sheet, and
    // that is the giant-kite defect in its purest form. Palm was also the most
    // under-spent form in the whole generator at 4% of MAX_TRIS.
    let frond = h * 0.34;
    for k in 0..28 {
        let phase = k as f32 * 2.399_963;
        let dd = tilt([0.0, 1.0, 0.0], rng.range(46.0, 92.0), phase);
        let dd = norm([dd[0], dd[1] - 0.28, dd[2]]);
        pinnate_frond(b, p, dd, frond, density, def.leaf_color, rng);
    }
}

/// One palm frond: an arching rachis with two ranks of leaflets.
///
/// Leaflet length is ~20% of the frond, which on a 12 m palm is 0.8 m - the
/// real figure for a coconut pinna, and 5x smaller than the sheet it replaces.
#[allow(clippy::too_many_arguments)]
fn pinnate_frond(
    b: &mut PlantMeshBuilder,
    base: [f32; 3],
    dir: [f32; 3],
    len: f32,
    density: f32,
    color: [f32; 3],
    rng: &mut Rng,
) {
    const SEGS: usize = 4;
    let mut pts = [[0.0f32; 3]; SEGS + 1];
    let mut q = base;
    let mut d = dir;
    pts[0] = q;
    for s in 0..SEGS {
        let seg = len / SEGS as f32;
        let r = |f: f32| len * 0.018 * (1.0 - f) + len * 0.003;
        let (ra, rb) = (r(s as f32 / SEGS as f32), r((s + 1) as f32 / SEGS as f32));
        // Joint overshoot, same trick as the limb spine.
        b.tube(q, add(q, d, seg + rb * 0.8), ra, rb, 3, color);
        q = add(q, d, seg);
        pts[s + 1] = q;
        // The rachis arches over: a straight frond reads as a plank.
        d = norm([d[0], d[1] - 0.15, d[2]]);
    }
    let leaflet = len * 0.20;
    // Leaflet pairs are the palm's density knob. A real coconut frond carries
    // ~100 leaflets a side, so there is no honest ceiling short of the budget.
    let pairs = ((26.0 * density).round() as usize).clamp(6, 70);
    for i in 0..pairs {
        let f = (i as f32 + 0.5) / pairs as f32;
        let t = f * SEGS as f32;
        let si = (t as usize).min(SEGS - 1);
        let ft = t - si as f32;
        let a = pts[si];
        let bb = pts[si + 1];
        let node = [
            a[0] + (bb[0] - a[0]) * ft,
            a[1] + (bb[1] - a[1]) * ft,
            a[2] + (bb[2] - a[2]) * ft,
        ];
        let axis = norm([bb[0] - a[0], bb[1] - a[1], bb[2] - a[2]]);
        let up = if axis[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
        let side = norm(cross(axis, up));
        // Leaflets shorten toward the tip and sweep forward along the rachis.
        let ll = leaflet * (1.0 - 0.45 * f) * rng.range(0.85, 1.12);
        for rank in [-1.0f32, 1.0] {
            let ld = norm([
                axis[0] * 0.42 + side[0] * rank * 0.9,
                axis[1] * 0.42 + side[1] * rank * 0.9 - 0.42,
                axis[2] * 0.42 + side[2] * rank * 0.9,
            ]);
            blade(b, node, ld, ll, 0.16, color, rng);
        }
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

    // ── Real leaf scale + the triangle budget (v0.1086) ──────────────────

    /// Exactly the seed the two call sites use (`lib.rs` near-mesh build and
    /// `billboard_bake::bake_tree_atlas_from_registry`). Testing a species at a
    /// seed nobody ships would prove nothing about the meshes that ship.
    fn shipped_seed(variant: u32) -> u32 {
        variant.wrapping_mul(2_654_435_761)
    }

    /// `build_tree` dispatches on `form` and never reads `model`, so ANY row
    /// can be built procedurally. The budget/scale tests use this rather than
    /// filtering on `is_procedural()`, because fir and pine are the only two
    /// CONIFER rows and both are model-backed today: filtering would silently
    /// leave the whole conifer form untested, which is how a 94%-of-budget
    /// conifer would have shipped unnoticed the moment a row flipped to
    /// procedural (and the release bundle, which carries no assets/models/,
    /// is exactly where that flip has to happen).
    fn as_procedural(t: &TreeDef) -> TreeDef {
        TreeDef { model: String::new(), ..t.clone() }
    }

    /// Length of one drawn leaf, recovered from a face: the distance from the
    /// apex to the midpoint of the opposite edge. `blade` emits an isoceles
    /// triangle whose two long edges meet at the tip, so the largest of the
    /// three vertex-to-opposite-midpoint distances IS the blade length.
    fn face_blade_len(p: [[f32; 3]; 3]) -> f32 {
        let mut best = 0.0f32;
        for i in 0..3 {
            let (a, b2) = (p[(i + 1) % 3], p[(i + 2) % 3]);
            let m = [(a[0] + b2[0]) * 0.5, (a[1] + b2[1]) * 0.5, (a[2] + b2[2]) * 0.5];
            let d = [p[i][0] - m[0], p[i][1] - m[1], p[i][2] - m[2]];
            best = best.max((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt());
        }
        best
    }

    /// EVERY (species, variant) that ships must fit MAX_TRIS, and must not
    /// leave the budget on the floor.
    ///
    /// Both halves matter. Over the ceiling, `limb`'s guard stops the recursion
    /// mid-crown and the tree comes out with bald subtrees. Far UNDER it, the
    /// generator is buying nothing with triangles it was given - which is
    /// exactly the state v0.1085 was in (palm 4%, acacia 13%) while its leaves
    /// were ten times life size for want of budget that was sitting right there.
    #[test]
    fn every_species_variant_fits_and_spends_the_triangle_budget() {
        let r = registry();
        for t in r.trees.iter().map(as_procedural) {
            for v in 0..t.variants.max(1) {
                let mut b = PlantMeshBuilder::new();
                build_tree(&mut b, &t, t.height_m, shipped_seed(v));
                let tris = b.indices.len() / 3;
                eprintln!(
                    "[budget] {:>7} v{v}: {tris:>5} tris ({:>3.0}% of {MAX_TRIS})",
                    t.id,
                    tris as f32 / MAX_TRIS as f32 * 100.0
                );
                assert!(
                    tris < MAX_TRIS,
                    "{} v{v}: {tris} triangles reaches the {MAX_TRIS} ceiling - limb() will \
                     truncate the crown mid-recursion and the tree ships with bald subtrees",
                    t.id
                );
                assert!(
                    tris > MAX_TRIS / 5,
                    "{} v{v}: only {tris} triangles of {MAX_TRIS} - the budget is going unspent \
                     while foliage detail is what needs it",
                    t.id
                );
            }
        }
    }

    /// The rung-A acceptance test: a drawn leaf must be a LEAF, not a tarp.
    ///
    /// Before v0.1086 the mean was 0.76-1.5 m on an 18 m oak because one
    /// number ("~10% of tree height") served as both clump radius and leaf
    /// size. Real temperate broadleaves run 0.06-0.20 m; the band here is
    /// 0.08-0.25 m so species tuning has room without letting a kite back in.
    #[test]
    fn broadleaf_leaves_are_drawn_at_real_scale() {
        const ORGAN_BIT_LEAF: u32 = 524_288;
        let r = registry();
        for t in r.trees.iter().filter(|t| t.form == "broadleaf").map(as_procedural) {
            for v in 0..t.variants.max(1) {
                let mut b = PlantMeshBuilder::new();
                build_tree(&mut b, &t, t.height_m, shipped_seed(v));
                let mut sum = 0.0f64;
                let mut n = 0usize;
                let mut longest = 0.0f32;
                for f in b.indices.chunks(3) {
                    let uv = b.vertices[f[0] as usize].uv;
                    if (uv[0].max(0.0).round() as u32) & ORGAN_BIT_LEAF == 0 {
                        continue;
                    }
                    let p = [
                        b.vertices[f[0] as usize].position,
                        b.vertices[f[1] as usize].position,
                        b.vertices[f[2] as usize].position,
                    ];
                    let l = face_blade_len(p);
                    sum += l as f64;
                    n += 1;
                    longest = longest.max(l);
                }
                assert!(n > 200, "{} v{v}: only {n} leaf faces", t.id);
                let mean = (sum / n as f64) as f32;
                eprintln!(
                    "[leafscale] {:>7} v{v}: {n:>5} leaf faces, mean {mean:.3} m, max {longest:.3} m \
                     (tree {:.0} m)",
                    t.id, t.height_m
                );
                assert!(
                    (0.08..=0.25).contains(&mean),
                    "{} v{v}: mean drawn leaf is {mean:.3} m on a {:.0} m tree - a real broadleaf \
                     leaf is 0.06-0.20 m, so this reads as a tarpaulin, not foliage",
                    t.id,
                    t.height_m
                );
                assert!(
                    longest < 0.45,
                    "{} v{v}: a single drawn leaf reaches {longest:.2} m",
                    t.id
                );
            }
        }
    }

    /// Conifer needle sprays are the coarser unit (a single 30 mm needle is
    /// below a pixel and there is no budget for 200k of them), but they still
    /// have to be sprays, not 1.2 m blades.
    #[test]
    fn conifer_needle_sprays_are_sub_half_metre() {
        const ORGAN_BIT_LEAF: u32 = 524_288;
        let r = registry();
        let conifers: Vec<TreeDef> =
            r.trees.iter().filter(|t| t.form == "conifer").map(as_procedural).collect();
        assert!(!conifers.is_empty(), "no conifer rows to test");
        for t in &conifers {
            let mut b = PlantMeshBuilder::new();
            build_tree(&mut b, t, t.height_m, shipped_seed(0));
            let mut sum = 0.0f64;
            let mut n = 0usize;
            for f in b.indices.chunks(3) {
                let uv = b.vertices[f[0] as usize].uv;
                if (uv[0].max(0.0).round() as u32) & ORGAN_BIT_LEAF == 0 {
                    continue;
                }
                let p = [
                    b.vertices[f[0] as usize].position,
                    b.vertices[f[1] as usize].position,
                    b.vertices[f[2] as usize].position,
                ];
                sum += face_blade_len(p) as f64;
                n += 1;
            }
            assert!(n > 200, "{}: only {n} needle faces", t.id);
            let mean = (sum / n as f64) as f32;
            eprintln!("[leafscale] {:>7} conifer: mean spray {mean:.3} m", t.id);
            assert!(
                (0.20..=0.62).contains(&mean),
                "{}: mean needle spray {mean:.3} m is outside the 0.3-0.5 m target band",
                t.id
            );
        }
    }

    /// MIDRIB ROLL (rung A, second half). Leaf plane normals must be spread
    /// around the whole sphere, not clustered on one axis.
    ///
    /// The old `blade` derived its width axis from `cross(dir, world_up)`, so
    /// every leaf's normal lived in a narrow family and the measured
    /// camera-facing-underside share was 74-78% against 61% for a spherical
    /// leaf-angle distribution. A uniform random roll about the midrib makes
    /// the normal azimuth uniform; this asserts the observable consequence,
    /// that the vertical component of the leaf normal is not piled up near 0
    /// (all-edge-on) or near 1 (all face-up).
    #[test]
    fn leaf_normals_are_not_pinned_to_one_orientation() {
        const ORGAN_BIT_LEAF: u32 = 524_288;
        let r = registry();
        let t = r.get(r.index_of("oak").unwrap()).unwrap();
        let mut b = PlantMeshBuilder::new();
        build_tree(&mut b, t, t.height_m, shipped_seed(0));
        // 5 buckets over |normal.y|, which is 0 for an edge-on leaf and 1 for a
        // flat-facing one. A spherical distribution fills all five.
        let mut buckets = [0usize; 5];
        let mut n = 0usize;
        for f in b.indices.chunks(3) {
            let v = &b.vertices[f[0] as usize];
            if (v.uv[0].max(0.0).round() as u32) & ORGAN_BIT_LEAF == 0 {
                continue;
            }
            let k = ((v.normal[1].abs() * 5.0) as usize).min(4);
            buckets[k] += 1;
            n += 1;
        }
        assert!(n > 400, "only {n} leaf faces sampled");
        eprintln!("[roll] oak |n.y| buckets: {buckets:?} of {n}");
        for (i, &c) in buckets.iter().enumerate() {
            assert!(
                c * 25 > n,
                "leaf-normal bucket {i} holds only {c} of {n} faces - the blades are pinned to \
                 one orientation family, so the midrib roll is gone and the canopy will light \
                 flat (buckets: {buckets:?})"
            );
        }
    }

    /// SEAM WELD (rung B). A branch root ring must be displaced backwards along
    /// its own axis, INTO the parent, by at least the parent's local radius -
    /// that is what makes the junction watertight from every angle given that
    /// `tube` emits no end caps.
    #[test]
    fn branch_roots_are_buried_inside_their_parent() {
        // A limb long enough that the clamp does not bite: burial must be at
        // least one parent radius, in the direction OPPOSITE the limb.
        for &parent_r in &[0.02f32, 0.1, 0.4] {
            for &r0 in &[0.01f32, 0.4] {
                let len = 8.0;
                let from = [1.0, 5.0, -2.0];
                let dir = norm([0.6, 0.7, -0.2]);
                let (root, embed) = welded_root(from, dir, parent_r, r0, len);
                assert!(
                    embed >= parent_r,
                    "embed {embed} is shallower than the parent radius {parent_r}: the root ring \
                     is not enclosed and the seam stays open"
                );
                let back = [root[0] - from[0], root[1] - from[1], root[2] - from[2]];
                let along = back[0] * dir[0] + back[1] * dir[1] + back[2] * dir[2];
                assert!(
                    (along + embed).abs() < 1e-4,
                    "the root moved {along}, expected {} along the limb axis",
                    -embed
                );
                assert!(along <= -parent_r, "displacement {along} does not clear {parent_r}");
            }
        }
        // ...and a stubby twig off a fat parent must not disappear inside it.
        let (_, embed) = welded_root([0.0; 3], [0.0, 1.0, 0.0], 5.0, 0.01, 0.4);
        assert!((embed - 0.2).abs() < 1e-6, "burial should clamp to half the limb, got {embed}");
    }

    /// The weld must not move the tree: the tip of every limb lands exactly
    /// where it did before, because the first spine segment absorbs the burial.
    /// Cheap proxy - the crown's extent is unchanged in kind (this rides along
    /// with `every_procedural_form_builds_finite_geometry`, which bounds the
    /// top, by additionally bounding the horizontal spread).
    #[test]
    fn welding_does_not_inflate_the_crown() {
        let r = registry();
        for t in r.trees.iter().map(as_procedural) {
            let mut b = PlantMeshBuilder::new();
            build_tree(&mut b, &t, t.height_m, shipped_seed(1));
            let mut lo = 0.0f32;
            let mut wide = 0.0f32;
            for v in &b.vertices {
                lo = lo.min(v.position[1]);
                wide = wide.max(v.position[0].hypot(v.position[2]));
            }
            // Nothing may sink far below the ground plane the tree is placed on
            // (a buried root ring on a PRIMARY limb travels down into the bole,
            // never below the trunk base).
            assert!(lo > -t.height_m * 0.06, "{}: geometry reaches {lo} m below its base", t.id);
            assert!(wide < t.height_m * 1.6, "{}: crown spreads {wide} m", t.id);
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
