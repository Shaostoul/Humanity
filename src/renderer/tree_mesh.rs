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
    /// Cluster-card foliage (v0.1088). Absent = this species carries only the
    /// geometric blade layer, exactly as it did through v0.1087.
    #[serde(default)]
    pub clusters: Option<ClusterDef>,
}

/// One card LAYER's facts. A species carries two: leaf and blossom.
///
/// Infinite-of-X: these are per-species measurements (a cherry's flowering
/// sleeve is not an oak's leaf tuft), so they live in
/// `data/vegetation/trees.ron`, never as constants in this file.
#[derive(Debug, Clone, Deserialize)]
pub struct ClusterLayerDef {
    /// Side of ONE square card, metres, before the LAI fit nudges it.
    pub size_m: f32,
    /// Fraction of the baked sprite's texels that pass the alpha cutoff. This
    /// is what turns card area into LEAF area, so it must match what the bake
    /// actually measures - `bake_cluster_sprites` logs both.
    pub coverage: f32,
    /// Cards in one sleeve, rotated about the twig axis.
    pub cards_per_sleeve: u32,
    /// Distance between sleeves ALONG a twig, metres.
    pub sleeve_spacing_m: f32,
    /// Elements baked into one sprite: sprigs for a leaf cluster, flower
    /// umbels PER RUN for a blossom sleeve.
    pub sprite_elements: u32,
    /// Parallel twiglets baked side by side into one sprite. 1 is a single
    /// scattered cluster (the leaf ball); a blossom sleeve needs several,
    /// because one flowering twig is a few centimetres wide and would leave
    /// most of a half-metre card empty - which would make the layer's
    /// `coverage`, the number the LAI fit spends, a fiction.
    pub sprite_runs: u32,
}

/// Cluster-card foliage for one species.
#[derive(Debug, Clone, Deserialize)]
pub struct ClusterDef {
    /// One-sided leaf area per unit of crown ground area. Real broadleaf
    /// crowns run 3-5; the geometric blade layer alone reaches 0.31-0.50,
    /// which is why the crown reads as a bare winter tree with sprinkles.
    pub target_lai: f32,
    pub leaf: ClusterLayerDef,
    pub blossom: ClusterLayerDef,
    /// Diameter of ONE flower, metres (Yoshino cherry: 0.035).
    pub flower_size_m: f32,
    /// Flowers per umbel (Yoshino cherry: 3-6).
    pub flowers_per_umbel: u32,
    /// Distance between umbels along a flowering twig, metres. A real cherry
    /// spaces them a few centimetres apart, which is why a photograph reads as
    /// a branch WRAPPED in blossom rather than a pink cloud.
    pub umbel_spacing_m: f32,
    /// Above this `blossom_frac` the species is treated as IN BLOOM: a cherry
    /// flowers before it leafs out, so the leaf layer is cut back to
    /// `bloom_leaf_area_share` of the crown instead of splitting it evenly.
    pub leaf_off_above_blossom_frac: f32,
    /// Share of the crown's card area the leaf layer keeps while in bloom.
    pub bloom_leaf_area_share: f32,
    /// Triangles the near-field geometric blade layer keeps, as a fraction of
    /// the card layer's. The blades out-resolve a 256 px sprite only inside
    /// ~2 m (90 deg FOV, 2560 wide), so they are a close-range detail layer
    /// now, not the canopy.
    pub near_blade_tri_frac: f32,
}

impl ClusterDef {
    pub fn layer(&self, l: ClusterLayer) -> &ClusterLayerDef {
        match l {
            ClusterLayer::Leaf => &self.leaf,
            ClusterLayer::Blossom => &self.blossom,
        }
    }
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

// ── Wood as its own mesh, with real bark UVs (v0.1089) ───────────────────
//
// Through v0.1088 a procedural tree was ONE mesh on material type 20 and every
// bark pixel was invented in the fragment shader from object-space noise. That
// is why bark read as moulded plastic tubing past arm's reach: procedural noise
// has no mip chain, so it aliases the moment a trunk minifies, so the shader
// had to FADE IT OUT with distance (`detail`/`micro` in
// 90-fragment-main.wgsl) - and past ~3 m nothing was left but the flat
// per-face colour, on a 4-to-8-sided cylinder.
//
// A BAKED texture has mips, so it needs no fade at all. Sampling one needs real
// UVs, and the packed-colour transport every plant face uses IS the uv channel
// (`plant_mesh::tri_smooth` overwrites it). So the wood moves onto its own mesh
// with its own material type (22), exactly the way cluster cards (type 21)
// already do - no vertex-format change, no new bind group, no layout change.
//
// ONE build produces THREE meshes:
//   `foliage`      leaves, blossoms, and any tube that is NOT bark (the palm
//                  rachis carries LEAF colour and must stay leaf-green, so it
//                  is deliberately not wood) ................... type 20
//   `wood`         bark tubes with cylindrical UVs .............. type 22
//   `wood_packed`  the SAME bark tubes with packed colour, merged with the
//                  foliage into `TreeBuild::bake` for the two consumers that
//                  need one self-contained mesh whose only decode is the
//                  packed one: the sprite-atlas bake (its shader knows nothing
//                  else) and the shipped-build procedural fallback.

/// Metres of limb covered by ONE tile of the baked bark texture.
///
/// Derived from the species' own `height_m` instead of a new data field: bark
/// plates scale with the tree that grew them (a 22 m fir's plates are
/// hand-sized, a 7 m maple's are thumb-sized), and height is the one size
/// number every registry row already carries. The clamp stops a sapling tiling
/// at millimetre scale and a giant smearing one plate over a metre.
///
/// A per-species `bark_scale_m` in `data/vegetation/trees.ron` is the honest
/// long-term home for this; that file is outside this module's lane, so the
/// derivation lives here and is unit-tested (`bark_tile_scales_with_species`).
///
/// THE CONSTANT IS SET BY WHAT SURVIVES TO THE EYE, not by taste. A tile holds
/// 3-6 plate cells, so cell width is tile/5-ish; at 10 m a screen pixel covers
/// ~1.2 cm (1280 px, 90 deg FOV), and a feature needs 4+ pixels to read as a
/// feature rather than as noise the mip chain will eat. That puts the floor at
/// ~5 cm cells, i.e. a ~0.3 m tile. The first cut of this used height*0.022
/// (a 0.18 m tile on sakura, 3 cm cells) and the probe capture came back with
/// visibly SMOOTH trunks at 8 m even though the baked texture was correct -
/// the detail was real and entirely below Nyquist. Measured, not guessed.
pub fn bark_tile_m(def: &TreeDef) -> f32 {
    (def.height_m * 0.045).clamp(0.30, 1.0)
}

/// The three meshes one tree build emits (see the block comment above).
pub(crate) struct TreeParts {
    pub foliage: PlantMeshBuilder,
    pub wood: PlantMeshBuilder,
    pub wood_packed: PlantMeshBuilder,
    /// Metres per bark texture tile for this species.
    tile_m: f32,
}

impl TreeParts {
    fn new(def: &TreeDef) -> Self {
        TreeParts {
            foliage: PlantMeshBuilder::new(),
            wood: PlantMeshBuilder::new(),
            wood_packed: PlantMeshBuilder::new(),
            tile_m: bark_tile_m(def),
        }
    }

    /// Triangles emitted so far across the drawn parts. `MAX_TRIS` bounds the
    /// TREE, not one of its meshes, so the recursion's budget check must see
    /// wood and foliage together exactly as it did when they shared a builder.
    fn tri_count(&self) -> usize {
        (self.foliage.indices.len() + self.wood.indices.len()) / 3
    }

    /// A bark tube: geometrically identical to `PlantMeshBuilder::tube` (same
    /// ring positions, same smooth cone normals), emitted into the WOOD mesh
    /// with real cylindrical UVs - u around the ring, v along the limb, both
    /// measured in TILES of `tile_m` metres.
    ///
    /// WORLD-SPACE TEXEL DENSITY, not model-space. u spans
    /// `circumference / tile_m` tiles rather than a fixed 0..1, so the bark on
    /// a 0.7 m bole and the bark on a 2 cm twig are the same physical size. A
    /// fixed 0..1 would squeeze the whole texture around a twig and smear it
    /// 30x, which is the classic silent UV failure.
    ///
    /// The repeat count is ROUNDED TO A WHOLE NUMBER so the ring closes exactly
    /// on a texture period: with a tileable bake (`bake_bark_rgba`) that makes
    /// the wrap seam invisible. The ring is also NOT uv-wrapped - every quad
    /// carries its own vertices, so u runs 0..reps monotonically and the
    /// derivative used for mip selection never jumps.
    ///
    /// `v0_m` is the running arc length from the START of this limb, so a
    /// six-segment bole is one continuous texture run instead of restarting the
    /// pattern at each joint.
    #[allow(clippy::too_many_arguments)]
    fn bark_tube(
        &mut self,
        from: [f32; 3],
        to: [f32; 3],
        r0: f32,
        r1: f32,
        sides: u32,
        color: [f32; 3],
        v0_m: f32,
    ) {
        // The packed-colour twin for the single-mesh consumers. Same call, so
        // the two representations can never disagree about geometry.
        self.wood_packed.tube(from, to, r0, r1, sides, color);
        let axis = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let alen = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2])
            .sqrt()
            .max(1e-6);
        let ax = [axis[0] / alen, axis[1] / alen, axis[2] / alen];
        let helper = if ax[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
        let side = norm(cross(ax, helper));
        let up = cross(side, ax);
        let n = sides.max(3);
        let tile = self.tile_m.max(1e-3);
        // Repeats are fixed for the WHOLE tube (one ring count, or a lengthwise
        // edge would carry a du and the fissures would spiral), and they are
        // taken from the FAT end. A tapered tube therefore holds true density
        // where it is thickest - the bole you stand next to - and compresses
        // toward the tip, which is the direction real bark goes anyway: young
        // thin wood carries finer plates than an old butt log. Taking the mean
        // radius instead would let the base STRETCH up to 2x, and stretched
        // bark reads as smeared plastic; compression never does.
        let reps = ((std::f32::consts::TAU * r0) / tile).round().max(1.0);
        let (v_a, v_b) = (v0_m / tile, (v0_m + alen) / tile);
        let slope = (r0 - r1) / alen;
        for i in 0..n {
            let a0 = (i as f32) / (n as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32) / (n as f32) * std::f32::consts::TAU;
            let p = |ang: f32, at: [f32; 3], r: f32| {
                [
                    at[0] + (side[0] * ang.cos() + up[0] * ang.sin()) * r,
                    at[1] + (side[1] * ang.cos() + up[1] * ang.sin()) * r,
                    at[2] + (side[2] * ang.cos() + up[2] * ang.sin()) * r,
                ]
            };
            let rad = |ang: f32| {
                norm([
                    side[0] * ang.cos() + up[0] * ang.sin() + ax[0] * slope,
                    side[1] * ang.cos() + up[1] * ang.sin() + ax[1] * slope,
                    side[2] * ang.cos() + up[2] * ang.sin() + ax[2] * slope,
                ])
            };
            let (b0, b1) = (p(a0, from, r0), p(a1, from, r0));
            let (t0, t1) = (p(a0, to, r1), p(a1, to, r1));
            let (n0, n1) = (rad(a0), rad(a1));
            let u0 = (i as f32) / (n as f32) * reps;
            let u1 = ((i + 1) as f32) / (n as f32) * reps;
            self.wood
                .card_tri([b0, t0, t1], [n0, n0, n1], [[u0, v_a], [u0, v_b], [u1, v_b]]);
            self.wood
                .card_tri([b0, t1, b1], [n0, n1, n1], [[u0, v_a], [u1, v_b], [u1, v_a]]);
        }
    }
}

// ── The baked bark texture (v0.1089) ─────────────────────────────────────
//
// One RGBA8 sRGB image per species, generated on the CPU at world entry and
// handed to the EXISTING per-material albedo slot - the same slot cluster
// cards use. No new @group(3) binding: bindings 11 and 12 look free in the
// WGSL but are the atmosphere LUTs in the Rust layout (pipeline.rs), and a
// new texture_2d<f32> there would type-match, raise no validation error and
// silently sample a 256x64 atmosphere LUT as bark.
//
// CHANNELS. RGB is `trunk_color` modulated by the plate field. ALPHA is that
// same field as a LINEAR height/ambient-occlusion scalar (0 = deepest fissure,
// 1 = ridge crest). Alpha in an Rgba8UnormSrgb texture is NOT gamma-encoded,
// so it is the one clean linear channel available without a second texture,
// and the type-22 fragment branch differentiates it for relief and reads it
// for roughness. That is the whole "full PBR" claim: albedo, normal and
// roughness out of one fetch.
//
// WHY sRGB IS A FIDELITY WIN HERE, not just a convention: a fir's trunk_color
// is 0.075 linear. Encoded sRGB that is ~76/255, and a +/-40% shade swing
// spans ~50 codes; the same swing in the LINEAR framebuffer domain the old
// procedural bark worked in spans about 8. The shipped bark measured 0.27
// luma levels of detail against a 0.258-level quantization floor - it was
// literally at the 8-bit floor. This is off it by an order of magnitude.

/// Side of one baked bark texture, texels. At the 0.12-0.55 m tile
/// (`bark_tile_m`) that is 0.23-1.07 mm per texel - finer than a real fissure
/// edge - and it mips down to 1x1 for the far field.
pub const BARK_PX: u32 = 512;

/// Hash one lattice cell of a WRAPPING lattice, so every field below tiles
/// exactly at the texture border. Wrapping is the whole reason this is not
/// just a CPU copy of the shader's `hash21`: a texture that does not tile
/// draws a hard seam down every trunk.
fn bark_hash(ix: i32, iy: i32, cx: i32, cy: i32, salt: u32) -> f32 {
    let x = ix.rem_euclid(cx.max(1)) as u32;
    let y = iy.rem_euclid(cy.max(1)) as u32;
    let mut h = 0x9e37_79b9u32 ^ salt.wrapping_mul(0x85eb_ca6b);
    h = h.wrapping_add(x.wrapping_mul(0xc2b2_ae35));
    h ^= h >> 13;
    h = h.wrapping_mul(0x27d4_eb2f).wrapping_add(y.wrapping_mul(0x1656_67b1));
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 15;
    (h >> 8) as f32 / 16_777_216.0
}

/// Distance to the nearest voronoi EDGE (F2 - F1), on a wrapping lattice.
///
/// Same estimator as `voronoi_edge` in `assets/shaders/pbr/10-lighting-
/// patterns.wgsl`, which is what the procedural bark used, so the baked plates
/// are the same FAMILY of shapes the shipped look established - just resolved
/// once at 512 px with mips instead of re-evaluated per pixel with a distance
/// fade papering over the aliasing.
fn bark_voronoi_edge(px: f32, py: f32, cx: i32, cy: i32, salt: u32) -> f32 {
    let ix = px.floor();
    let iy = py.floor();
    let (fx, fy) = (px - ix, py - iy);
    let (mut d1, mut d2) = (8.0f32, 8.0f32);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let cell_x = ix as i32 + dx;
            let cell_y = iy as i32 + dy;
            let jx = bark_hash(cell_x, cell_y, cx, cy, salt);
            let jy = bark_hash(cell_x, cell_y, cx, cy, salt ^ 0x5bf0_3635);
            let ex = dx as f32 + jx - fx;
            let ey = dy as f32 + jy - fy;
            let d = ex * ex + ey * ey;
            if d < d1 {
                d2 = d1;
                d1 = d;
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    d2.sqrt() - d1.sqrt()
}

/// Value noise on the same wrapping lattice (smoothstep-interpolated).
fn bark_value_noise(px: f32, py: f32, cx: i32, cy: i32, salt: u32) -> f32 {
    let ix = px.floor();
    let iy = py.floor();
    let (fx, fy) = (px - ix, py - iy);
    let (sx, sy) = (fx * fx * (3.0 - 2.0 * fx), fy * fy * (3.0 - 2.0 * fy));
    let (i, j) = (ix as i32, iy as i32);
    let a = bark_hash(i, j, cx, cy, salt);
    let b = bark_hash(i + 1, j, cx, cy, salt);
    let c = bark_hash(i, j + 1, cx, cy, salt);
    let d = bark_hash(i + 1, j + 1, cx, cy, salt);
    let top = a + (b - a) * sx;
    let bot = c + (d - c) * sx;
    top + (bot - top) * sy
}

/// Three octaves of wrapping value noise. THREE, not one: a single octave is
/// the "one flat grain frequency" tell, and the acceptance bar for this work
/// is multi-octave detail with no octave holding the whole energy.
fn bark_fbm(u: f32, v: f32, cx: i32, cy: i32, salt: u32) -> f32 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut mul = 1;
    for o in 0..3u32 {
        sum += amp
            * bark_value_noise(
                u * (cx * mul) as f32,
                v * (cy * mul) as f32,
                cx * mul,
                cy * mul,
                salt ^ o.wrapping_mul(0x9e37_79b9),
            );
        amp *= 0.5;
        mul *= 2;
    }
    sum / 0.875
}

fn bark_smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0).max(1e-6)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn linear_to_srgb_u8(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0 + 0.5) as u8
}

/// Bake one species' bark: RGBA8 sRGB, `BARK_PX` square, tileable both ways.
///
/// MORPHOLOGY IS DERIVED FROM DATA, not from a species table (infinite-of-X:
/// adding a tree stays a data row). Two data inputs drive it:
///   - the species id, hashed, sets plate count, elongation and fissure depth,
///     so no two rows share a field;
///   - `trunk_color`'s luminance drives the smooth/papery axis, because pale
///     barks in the real world (birch, aspen, young cherry) are smooth sheets
///     with HORIZONTAL lenticels while dark barks (oak, fir, acacia) are
///     deeply fissured vertical plates. That is a genuine correlation, not a
///     coincidence, and it means the registry already carries the signal.
/// A per-species `bark_style` field in data/vegetation/trees.ron would be the
/// better long-term home; that file is outside this lane.
pub fn bake_bark_rgba(def: &TreeDef) -> Vec<u8> {
    use rayon::prelude::*;
    let n = BARK_PX;
    // Per-species knobs, hashed from the id (FNV-1a).
    let mut hs: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in def.id.bytes() {
        hs ^= byte as u64;
        hs = hs.wrapping_mul(0x100_0000_01b3);
    }
    let k = |shift: u32| ((hs >> shift) & 0xff) as f32 / 255.0;
    let salt = (hs & 0xffff_ffff) as u32;
    // Plates AROUND one tile vs ALONG it. More around than along is what makes
    // a plate taller than it is wide, which is what bark is. Kept LOW (3-6, not
    // 4-8) for the same Nyquist reason `bark_tile_m` is sized the way it is:
    // the primary structure has to be plate-scale (10-25 cm), because that is
    // what is still resolvable across a forest, and the finer octave below
    // carries the close-range fissures.
    let cells_u = 3 + (k(0) * 3.0).round() as i32;
    let cells_v = 1 + (k(8) * 1.0).round() as i32;
    let crack_w = 0.15 + k(16) * 0.14;
    let grain_amp = 0.24 + k(24) * 0.14;
    // Smooth/papery axis from trunk_color luminance (Rec.709).
    let luma = 0.2126 * def.trunk_color[0] + 0.7152 * def.trunk_color[1] + 0.0722 * def.trunk_color[2];
    let papery = bark_smoothstep(0.10, 0.34, luma);
    let base = def.trunk_color;
    let rows: Vec<Vec<u8>> = (0..n)
        .into_par_iter()
        .map(|y| {
            let mut row = vec![0u8; (n * 4) as usize];
            let v = (y as f32 + 0.5) / n as f32;
            for x in 0..n {
                let u = (x as f32 + 0.5) / n as f32;
                // Primary plate field, elongated along the limb.
                let e1 = bark_voronoi_edge(
                    u * cells_u as f32,
                    v * cells_v as f32,
                    cells_u,
                    cells_v,
                    salt,
                );
                let crack = 1.0 - bark_smoothstep(0.0, crack_w, e1);
                // Second, finer plate octave inside the first's cells: real
                // fissures branch and terminate at more than one scale.
                let e2 = bark_voronoi_edge(
                    u * (cells_u * 3) as f32,
                    v * (cells_v * 3) as f32,
                    cells_u * 3,
                    cells_v * 3,
                    salt ^ 0x2545_f491,
                );
                let crack2 = 1.0 - bark_smoothstep(0.0, crack_w * 0.55, e2);
                // Fine grain: three octaves, stretched along the limb.
                let grain = bark_fbm(u, v, cells_u * 4, cells_v * 2, salt ^ 0x1656_67b1);
                // Horizontal lenticels for pale/papery barks: short dashes
                // ACROSS the trunk, the birch signature. Stretched the other
                // way round from the fissures on purpose.
                let lent = if papery > 0.01 {
                    let f = bark_voronoi_edge(
                        u * 3.0,
                        v * 26.0,
                        3,
                        26,
                        salt ^ 0x7ee_5eed,
                    );
                    (1.0 - bark_smoothstep(0.0, 0.34, f)) * papery
                } else {
                    0.0
                };
                // HEIGHT / AO (alpha). Ridges at 1, fissures cut down toward 0.
                let fissure = (crack * (1.0 - papery * 0.75) + crack2 * 0.45).clamp(0.0, 1.0);
                let h = (1.0 - fissure * 0.72 - lent * 0.35 + (grain - 0.5) * 0.20)
                    .clamp(0.06, 1.0);
                // ALBEDO. Same shape as the shipped procedural bark (a scalar
                // multiple of trunk_color: crevices darker, ridges catching
                // more light) so the species keeps its colour - but with real
                // contrast. The shipped fragment version ran 0.72..0.96 of
                // trunk_color BEFORE its distance fade pulled it further
                // toward flat; this runs ~0.27..1.0, a 3.7:1 range, which is
                // where a photographed bark's albedo histogram actually sits.
                let shade = (0.68 + grain_amp * grain) * (1.0 - 0.62 * fissure)
                    - lent * 0.24 * (1.0 - papery * 0.4);
                let shade = shade.max(0.12);
                let i = (x * 4) as usize;
                row[i] = linear_to_srgb_u8(base[0] * shade);
                row[i + 1] = linear_to_srgb_u8(base[1] * shade);
                row[i + 2] = linear_to_srgb_u8(base[2] * shade);
                // Alpha is LINEAR in an sRGB texture: store height directly.
                row[i + 3] = (h * 255.0 + 0.5) as u8;
            }
            row
        })
        .collect();
    rows.concat()
}

/// Everything one tree build hands back.
pub struct TreeBuild {
    /// Leaves, blossoms and non-bark tubes - material type 20.
    pub mesh: PlantMeshBuilder,
    /// Bark tubes with real cylindrical UVs - material type 22.
    pub wood: PlantMeshBuilder,
    /// `mesh` + the wood in its packed-colour form: ONE self-contained mesh on
    /// material type 20, which is what the sprite-atlas bake and the
    /// shipped-build fallback need.
    pub bake: PlantMeshBuilder,
    /// One entry per cluster-card layer - material type 21.
    pub cards: Vec<ClusterCards>,
}

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
    let built = build_tree_and_cards(def, height_m, seed);
    // The SINGLE-MESH form (v0.1089): foliage plus the packed-colour twin of
    // the wood, so this API keeps behaving exactly as it did when a tree was
    // one type-20 mesh. Its two callers - the sprite-atlas bake and the
    // shipped-build procedural fallback - have no second draw to spend on a
    // type-22 wood mesh, and the atlas bake shader only knows the packed decode.
    let merged = built.bake;
    // Merge (the callers hand us a fresh builder, but never assume that).
    let base = b.vertices.len() as u32;
    b.vertices.extend_from_slice(&merged.vertices);
    b.indices.extend(merged.indices.iter().map(|i| i + base));
}

/// The full build: the wood-and-blades mesh (material type 20) plus one card
/// mesh per cluster layer (material type 21), which are separate MESHES
/// because each layer samples its own mipped sprite through the per-material
/// albedo slot.
///
/// A species with no `clusters` block returns an empty card list and a mesh
/// bit-identical to what `build_tree` produced through v0.1087.
pub fn build_tree_and_cards(def: &TreeDef, height_m: f32, seed: u32) -> TreeBuild {
    let h = height_m.max(0.5);
    let mut twigs: Vec<Twig> = Vec::new();
    let first = build_at_density(def, h, seed, 1.0, &mut twigs);
    let total = first.tri_count();
    let leaves = leaf_tri_count(&first.foliage);
    let wood_tris = total.saturating_sub(leaves);

    let cards = match &def.clusters {
        Some(cd) => emit_cluster_cards(def, cd, &twigs, seed),
        None => Vec::new(),
    };
    let card_tris: usize = cards.iter().map(|c| c.mesh.indices.len() / 3).sum();

    // How many triangles the GEOMETRIC blade layer should get.
    //
    // Without cards it is "everything the budget has left", which is what
    // v0.1086 established. WITH cards the blades stop being the canopy and
    // become a close-range detail layer - a 256 px sprite out-resolves the
    // screen past ~1.7 m at 2560 wide - so they take a fraction of the card
    // layer instead, and the tree comes out CHEAPER than it was.
    let want_leaf = match &def.clusters {
        Some(cd) => card_tris as f32 * cd.near_blade_tri_frac.max(0.0),
        None => MAX_TRIS as f32 * BUDGET_TARGET - wood_tris as f32,
    };
    let best = if leaves > 0 && want_leaf > 0.0 {
        let lo = if def.clusters.is_some() { 0.05 } else { 0.2 };
        let scale = (want_leaf / leaves as f32).clamp(lo, 8.0);
        // Only pay for a second build when it actually buys something.
        if !(0.97..=1.03).contains(&scale) {
            let mut sink = Vec::new();
            let second = build_at_density(def, h, seed, scale, &mut sink);
            let st = second.tri_count();
            if def.clusters.is_some() {
                // Clustered species are aiming DOWN, so the v0.1086 rule
                // ("only accept a rebuild that grew") would reject every
                // useful result. Accept anything that fits with its cards.
                if st + card_tris <= MAX_TRIS {
                    second
                } else {
                    first
                }
            } else if st <= MAX_TRIS && (st > total || total > MAX_TRIS) {
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
    // The single-mesh form: foliage first, then the wood's packed-colour twin.
    // Draw order is irrelevant (opaque, depth-tested) and the atlas bake reads
    // one decode for both halves.
    let mut bake = PlantMeshBuilder::new();
    bake.vertices.extend_from_slice(&best.foliage.vertices);
    bake.indices.extend_from_slice(&best.foliage.indices);
    let base = bake.vertices.len() as u32;
    bake.vertices.extend_from_slice(&best.wood_packed.vertices);
    bake.indices
        .extend(best.wood_packed.indices.iter().map(|i| i + base));
    TreeBuild { mesh: best.foliage, wood: best.wood, bake, cards }
}

/// Crown envelope of a species as the card planner sees it. Public so the
/// LAI gate and any caller sizing a card material can ask the generator
/// rather than re-deriving it.
pub fn crown_envelope(def: &TreeDef, height_m: f32, seed: u32) -> CrownEnvelope {
    let mut twigs = Vec::new();
    let _ = build_at_density(def, height_m.max(0.5), seed, 1.0, &mut twigs);
    crown_of(&twigs)
}

/// One build pass at a given foliage density multiplier. `twigs` collects the
/// outer two limb generations, which is where cluster cards are sleeved.
fn build_at_density(
    def: &TreeDef,
    h: f32,
    seed: u32,
    density: f32,
    twigs: &mut Vec<Twig>,
) -> TreeParts {
    let mut b = TreeParts::new(def);
    let mut rng = Rng::new(seed as u64 ^ 0x7ee_5eed);
    match def.form.as_str() {
        "conifer" => conifer(&mut b, def, h, density, &mut rng),
        "umbrella" => umbrella(&mut b, def, h, density, &mut rng),
        "palm" => palm(&mut b, def, h, density, &mut rng),
        // Unknown forms fall back to broadleaf so a new data row always renders.
        _ => broadleaf(&mut b, def, h, density, &mut rng, twigs),
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
    b: &mut TreeParts,
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
    // Running arc length for the bark v coordinate: the bole is ONE texture
    // run, not six restarts (v0.1089).
    let mut v = 0.0f32;
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
        b.bark_tube(p, add(p, d, seg + rb * 0.5), ra, rb, 8, def.trunk_color, v);
        p = to;
        // The SPINE advances by `seg`; the extra `rb * 0.5` is joint overshoot
        // that overlaps the next segment, so v must not count it twice.
        v += seg;
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

// ── Cluster cards (v0.1088) ──────────────────────────────────────────────
//
// THE ARITHMETIC THAT FORCED THIS. A drawn blade covers 0.0014 m^2 per
// triangle on sakura; reaching a real cherry's leaf area index of 3 that way
// needs ~85 m^2 of leaf = ~30,000 drawn leaves = 60,000 triangles, seven times
// MAX_TRIS, on every one of 256 near instances. A 0.5 m square cluster CARD,
// double-sided (4 triangles, because the opaque pipeline back-culls) at ~55%
// alpha coverage, covers 0.034 m^2 per triangle - 24x better. Cluster cards
// are not a compromise here; they are the first configuration in which honest
// leaf area is affordable at all, and the per-tree triangle count DROPS.
//
// WHAT A CARD CARRIES. Position, a SPHERIFIED normal, and a UV. Nothing else,
// and no vertex-format change: the card's ambient-occlusion scalar rides in
// the integer part of `uv.x` (see `encode_card_uv`), which is free because a
// card samples a real texture instead of the packed-colour channel every other
// plant face uses.
//
// WHY NOT THE 6x8 TREE ATLAS. Tiles cannot be mipped (filtering bleeds across
// tile borders) and a cutout sprite without mips crawls the moment it minifies
// - which is exactly where a forest is looked at. Each cluster sprite gets its
// own mipped texture instead, bound through the per-material albedo slot that
// already exists, so no bind-group LAYOUT changes (the v0.1029-v0.1038
// incident class).

/// Fraction of the open-sky ambient a card at the crown's CORE keeps.
///
/// A leaf deep inside a crown sees perhaps 20% of the sky, and PAR at the base
/// of a real crown is 5-20% of the open value. That bright-shell / dark-core
/// gradient is most of what makes a tree read as a solid object rather than a
/// decal, and it is also what keeps backlit foliage SATURATED: without it the
/// achromatic sky ambient lands on every leaf equally and washes the crown to
/// pale grey-green (measured 0.30 against a leaf albedo saturation of 0.55).
const CLUSTER_CORE_AO: f32 = 0.20;

/// Quantisation of the baked AO scalar carried in the packed UV: 6 bits.
const CLUSTER_AO_LEVELS: f32 = 63.0;

/// How far a card's corner normals bend toward "outward from the cluster
/// centre". A quad lit by its own flat normal is cardboard: every card with
/// the same facing renders at exactly the same brightness, with hard steps
/// where two cards abut. Real tufts shade as rounded volumes.
const CLUSTER_NORMAL_BLEND: f32 = 0.70;

/// A second, weaker bend toward "outward from the CROWN centre", so the crown
/// as a whole reads as a lit sphere with a terminator running through it
/// instead of a uniformly bright cloud of correctly-rounded tufts.
const CROWN_NORMAL_BLEND: f32 = 0.30;

/// Triangles the card layer may spend per tree. Sized so that cards, the
/// near-field blade layer and the wood all fit MAX_TRIS with room to spare -
/// the point of this arc is that honest leaf area gets CHEAPER, so the ceiling
/// must never need raising.
const CARD_TRI_BUDGET: usize = 3400;

/// Radial offset of a card from its twig, as a fraction of the card side.
const CLUSTER_SLEEVE_OFFSET: f32 = 0.30;

/// Which baked sprite a card layer samples.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClusterLayer {
    Leaf,
    Blossom,
}

impl ClusterLayer {
    /// Stable key for mesh/material caches and log lines.
    pub fn key(self) -> &'static str {
        match self {
            ClusterLayer::Leaf => "leaf",
            ClusterLayer::Blossom => "blossom",
        }
    }
    pub const ALL: [ClusterLayer; 2] = [ClusterLayer::Leaf, ClusterLayer::Blossom];
}

/// One card layer's mesh, ready to become its own draw (material type 21 with
/// this layer's sprite in the per-material albedo slot).
pub struct ClusterCards {
    pub layer: ClusterLayer,
    pub mesh: PlantMeshBuilder,
    /// Cards emitted (each is 4 triangles: two windings of a quad).
    pub cards: u32,
    /// Side of one card after the LAI fit, metres.
    pub card_side_m: f32,
    /// One-sided leaf area this layer contributes: cards * side^2 * coverage.
    pub leaf_area_m2: f32,
}

/// Crown envelope, recovered from the twig tips the wood build recorded.
#[derive(Clone, Copy, Debug)]
pub struct CrownEnvelope {
    pub centre: [f32; 3],
    /// 3D radius: how deep a card can sit inside the crown, which is what the
    /// baked ambient occlusion measures against.
    pub radius_m: f32,
    /// HORIZONTAL radius about the crown's vertical axis. Leaf area index is
    /// leaf area over the ground area the crown SHADES, so the denominator is
    /// this and not the 3D radius - a tall narrow crown would otherwise be
    /// asked for twice the leaf area it actually needs.
    pub spread_m: f32,
}

impl CrownEnvelope {
    /// Ground area the crown projects, m^2 - the denominator of leaf area index.
    pub fn projected_area_m2(self) -> f32 {
        std::f32::consts::PI * self.spread_m * self.spread_m
    }
}

/// Pack a card's texture coordinate and its baked AO into the two floats a
/// vertex has.
///
/// `uv.x = 2 * ao_code + u01` with `ao_code` an integer 0..63. The decode is
/// exact in f32 for every value we emit (`u01` is 0 or 1 at a corner, the code
/// is a small integer), and it costs the shader one floor and one multiply -
/// against the alternative of a vertex-format change, which would touch every
/// pipeline in the engine.
pub fn encode_card_uv(u01: f32, v01: f32, ao: f32) -> [f32; 2] {
    let code = (ao.clamp(0.0, 1.0) * CLUSTER_AO_LEVELS).round();
    [2.0 * code + u01.clamp(0.0, 1.0), v01.clamp(0.0, 1.0)]
}

/// Inverse of `encode_card_uv`: (u01, v01, ao). Mirrors the type-21 decode in
/// `assets/shaders/pbr/90-fragment-main.wgsl` and the cluster-card branch of
/// the bake shader; all three must agree exactly.
pub fn decode_card_uv(uv: [f32; 2]) -> (f32, f32, f32) {
    let code = (uv[0] * 0.5).floor();
    (uv[0] - 2.0 * code, uv[1], code / CLUSTER_AO_LEVELS)
}

/// One limb of the outer two generations, recorded by `limb` while the wood is
/// being built. Cards are emitted ALONG these, as a sleeve hugging the twig -
/// not scattered through a clump volume, which is what made the old foliage
/// read as a dust cloud floating around the branch ends.
#[derive(Clone, Copy)]
struct Twig {
    /// Junction with the parent (the visible start, not the buried root ring).
    from: [f32; 3],
    /// Axis at the junction.
    dir: [f32; 3],
    /// Spine end. On a TERMINAL twig this is an open, uncapped tube ring -
    /// `PlantMeshBuilder::tube` emits no end cap - so a card must cover it.
    end: [f32; 3],
    len: f32,
    tip: bool,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    let d = sub(a, b);
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}
fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Crown envelope of a recorded twig set.
fn crown_of(twigs: &[Twig]) -> CrownEnvelope {
    if twigs.is_empty() {
        return CrownEnvelope { centre: [0.0, 0.0, 0.0], radius_m: 1.0, spread_m: 1.0 };
    }
    let n = twigs.len() as f32;
    let mut c = [0.0f32; 3];
    for t in twigs {
        c = [c[0] + t.end[0], c[1] + t.end[1], c[2] + t.end[2]];
    }
    let centre = [c[0] / n, c[1] / n, c[2] / n];
    let mut r = 0.0f32;
    let mut s = 0.0f32;
    for t in twigs {
        r = r.max(dist(t.end, centre));
        s = s.max((t.end[0] - centre[0]).hypot(t.end[2] - centre[2]));
    }
    CrownEnvelope { centre, radius_m: r.max(0.25), spread_m: s.max(0.25) }
}

/// Emit ONE card: two windings of a quad (4 triangles), with each corner's
/// normal spherified twice - toward the cluster centre, then weakly toward the
/// crown centre - and the card's AO baked into its UV.
///
/// The same normal rides both windings on purpose. A card is a stand-in for a
/// ball of blades, and a blade lit from behind glows rather than going black
/// (the type-21 branch carries the leaf transmission term), so flipping the
/// normal on the back face would darken exactly the half that should be
/// luminous.
#[allow(clippy::too_many_arguments)]
fn emit_card(
    b: &mut PlantMeshBuilder,
    centre: [f32; 3],
    facing: [f32; 3],
    wide: [f32; 3],
    tall: [f32; 3],
    half: f32,
    ao: f32,
    cluster_c: [f32; 3],
    crown_c: [f32; 3],
) {
    let corner = |sx: f32, sy: f32| add(add(centre, wide, sx * half), tall, sy * half);
    let p = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
    let uv01 = [[0.0f32, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let mut n = [[0.0f32; 3]; 4];
    let mut uv = [[0.0f32; 2]; 4];
    for i in 0..4 {
        // (a) CLUSTER SCALE: the tuft is a ball, so its normal at a point is
        //     mostly the direction out of the ball's centre.
        let out_cluster = norm(sub(p[i], cluster_c));
        let m1 = norm(mix3(facing, out_cluster, CLUSTER_NORMAL_BLEND));
        // (b) CROWN SCALE: and the crown is a bigger ball made of those.
        let out_crown = norm(sub(p[i], crown_c));
        n[i] = norm(mix3(m1, out_crown, CROWN_NORMAL_BLEND));
        uv[i] = encode_card_uv(uv01[i][0], uv01[i][1], ao);
    }
    b.card_tri([p[0], p[1], p[2]], [n[0], n[1], n[2]], [uv[0], uv[1], uv[2]]);
    b.card_tri([p[0], p[2], p[3]], [n[0], n[2], n[3]], [uv[0], uv[2], uv[3]]);
    // Reversed windings: the opaque pipeline back-culls, so without these a
    // card is invisible from one side.
    b.card_tri([p[0], p[2], p[1]], [n[0], n[2], n[1]], [uv[0], uv[2], uv[1]]);
    b.card_tri([p[0], p[3], p[2]], [n[0], n[3], n[2]], [uv[0], uv[3], uv[2]]);
}

/// One SLEEVE: `cards` cards spaced around the twig axis at one station,
/// each tangent to a cylinder about the twig and facing outward.
///
/// Facing outward rather than edge-on matters: the spherified normal then
/// AGREES with the card's own facing near its centre and only bends at the
/// corners, so a tuft rounds instead of fighting itself.
#[allow(clippy::too_many_arguments)]
fn emit_sleeve(
    b: &mut PlantMeshBuilder,
    station: [f32; 3],
    axis: [f32; 3],
    side: f32,
    cards: u32,
    ao: f32,
    crown_c: [f32; 3],
    phase: f32,
) -> u32 {
    let up = if axis[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let s1 = norm(cross(axis, up));
    let s2 = norm(cross(axis, s1));
    let half = side * 0.5;
    let off = side * CLUSTER_SLEEVE_OFFSET;
    let n = cards.max(1);
    for k in 0..n {
        let th = phase + k as f32 / n as f32 * std::f32::consts::TAU;
        let (st, ct) = (th.sin(), th.cos());
        let r = norm([
            s1[0] * ct + s2[0] * st,
            s1[1] * ct + s2[1] * st,
            s1[2] * ct + s2[2] * st,
        ]);
        let t = norm(cross(axis, r));
        let c = add(station, r, off);
        emit_card(b, c, r, t, axis, half, ao, station, crown_c);
    }
    n
}

/// Plan and emit every cluster card on one tree.
///
/// The plan is deliberately explicit rather than tuned by eye: leaf area is
/// the thing being bought, so the card SIDE is solved for the species'
/// `target_lai` while the sleeve SPACING stays the physical fact it is. That
/// makes `cluster_cards_reach_target_lai_and_fit_the_budget` a check on the
/// arithmetic rather than a tuning treadmill.
fn emit_cluster_cards(
    def: &TreeDef,
    cd: &ClusterDef,
    twigs: &[Twig],
    seed: u32,
) -> Vec<ClusterCards> {
    if twigs.is_empty() {
        return Vec::new();
    }
    let crown = crown_of(twigs);
    // Ambient occlusion falls off exponentially with depth inside the crown
    // envelope, normalised so the CORE always lands on CLUSTER_CORE_AO no
    // matter how big the species is.
    let k_ao = -(CLUSTER_CORE_AO.max(1e-3).ln()) / crown.radius_m;

    // ── Layer assignment: one deterministic coin per twig ────────────────
    // A cherry in bloom is bare wood sheathed in blossom, so the coin is the
    // species' own blossom_frac and the leaf layer is what is left over.
    let in_bloom = def.blossom_frac > cd.leaf_off_above_blossom_frac;
    let mut layer_of: Vec<ClusterLayer> = Vec::with_capacity(twigs.len());
    for i in 0..twigs.len() {
        // Its OWN stream, seeded from the twig index: the card layer must not
        // be able to move the wood, and the wood's own blossom coin (which
        // colours the blade layer) must not be able to move the cards.
        let mut r = Rng::new(seed as u64 ^ (i as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let blossom = def.blossom_frac > 0.0 && r.range(0.0, 1.0) < def.blossom_frac;
        layer_of.push(if blossom { ClusterLayer::Blossom } else { ClusterLayer::Leaf });
    }

    // ── Stations: physical spacing along each twig ───────────────────────
    let mut stations: Vec<u32> = Vec::with_capacity(twigs.len());
    for (i, t) in twigs.iter().enumerate() {
        let ld = cd.layer(layer_of[i]);
        let n = (t.len / ld.sleeve_spacing_m.max(0.01)).round().max(1.0);
        stations.push(n as u32);
    }
    let card_count = |stations: &[u32]| -> usize {
        let mut n = 0usize;
        for (i, t) in twigs.iter().enumerate() {
            let ld = cd.layer(layer_of[i]);
            n += stations[i] as usize * ld.cards_per_sleeve.max(1) as usize;
            if t.tip {
                n += 1; // the cap card over the open terminal ring
            }
        }
        n
    };
    // BACKSTOP, never a truncation: if the physical spacing overruns the
    // triangle budget, stretch every twig's spacing by the SAME factor. The
    // old MAX_TRIS guard stopped the recursion mid-crown and shipped bald
    // subtrees; a uniform stretch thins the whole crown evenly instead.
    let mut tris = card_count(&stations) * 4;
    if tris > CARD_TRI_BUDGET {
        let f = tris as f32 / CARD_TRI_BUDGET as f32;
        for s in stations.iter_mut() {
            *s = ((*s as f32 / f).round() as u32).max(1);
        }
        tris = card_count(&stations) * 4;
        if tris > CARD_TRI_BUDGET {
            log::debug!(
                "[Cluster] {}: {tris} card triangles after the stretch (budget {CARD_TRI_BUDGET}) \
                 - the one-station-per-twig floor is the binding constraint",
                def.id
            );
        }
    }

    // ── Card side: solved so the crown lands on target_lai ───────────────
    let area_total = cd.target_lai * crown.projected_area_m2();
    let mut out: Vec<ClusterCards> = Vec::new();
    for layer in ClusterLayer::ALL {
        let ld = cd.layer(layer);
        let mine: Vec<usize> = (0..twigs.len()).filter(|&i| layer_of[i] == layer).collect();
        if mine.is_empty() {
            continue;
        }
        let mut cards_here = 0usize;
        for &i in &mine {
            cards_here += stations[i] as usize * ld.cards_per_sleeve.max(1) as usize;
            if twigs[i].tip {
                cards_here += 1;
            }
        }
        // Share of the crown's leaf area this layer is meant to carry.
        let share = match (in_bloom, layer) {
            (true, ClusterLayer::Leaf) => cd.bloom_leaf_area_share,
            (true, ClusterLayer::Blossom) => 1.0 - cd.bloom_leaf_area_share,
            // Not in bloom: area follows the twig split.
            _ => mine.len() as f32 / twigs.len() as f32,
        };
        let want = area_total * share.clamp(0.0, 1.0);
        let natural = cards_here as f32 * ld.size_m * ld.size_m * ld.coverage.max(0.01);
        // Solve the card SIDE for the target leaf area rather than adding
        // cards: our skeleton carries ~66 outer twigs where a real cherry
        // ramifies to thousands, so each twig has to stand in for a bigger
        // chunk of crown. Bounded by the crown's own scale at the top (a card
        // a third of the crown wide is a tarpaulin again) and by a hard floor
        // at the bottom (below ~0.12 m a "cluster" is one leaf).
        let scale = if natural > 1e-4 { (want / natural).sqrt() } else { 1.0 };
        let side = (ld.size_m * scale).clamp(0.12, crown.spread_m * 0.30);

        let mut mesh = PlantMeshBuilder::new();
        let mut cards = 0u32;
        for &i in &mine {
            let t = &twigs[i];
            let n = stations[i].max(1);
            let mut r = Rng::new(
                (seed as u64 ^ 0x0C1A_57E5) ^ (i as u64 + 7).wrapping_mul(0x9E37_79B9_7F4A_7C15),
            );
            for s in 0..n {
                // Stations run from just clear of the junction to the tip, so
                // the sleeve hugs the whole visible twig.
                let f = (s as f32 + 0.5) / n as f32;
                let station = mix3(t.from, t.end, f);
                let depth = (crown.radius_m - dist(station, crown.centre)).max(0.0);
                let ao = (-k_ao * depth).exp().clamp(CLUSTER_CORE_AO * 0.5, 1.0);
                cards += emit_sleeve(
                    &mut mesh,
                    station,
                    t.dir,
                    side,
                    ld.cards_per_sleeve,
                    ao,
                    crown.centre,
                    r.range(0.0, std::f32::consts::TAU),
                );
            }
            if t.tip {
                // CAP CARD over the terminal ring. `tube` emits no end cap and
                // the v0.1086 weld only plugs junctions where a CHILD buries
                // itself in a parent, so a terminal tip is an open pipe you
                // can look down against the sky. The card sits just BEYOND the
                // tip, facing along the twig, so it covers the hole from every
                // angle the hole is visible from.
                let up = if t.dir[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
                let w = norm(cross(t.dir, up));
                let h = norm(cross(t.dir, w));
                let c = add(t.end, t.dir, side * CLUSTER_SLEEVE_OFFSET);
                let depth = (crown.radius_m - dist(t.end, crown.centre)).max(0.0);
                let ao = (-k_ao * depth).exp().clamp(CLUSTER_CORE_AO * 0.5, 1.0);
                emit_card(&mut mesh, c, t.dir, w, h, side * 0.5, ao, t.end, crown.centre);
                cards += 1;
            }
        }
        let leaf_area_m2 = cards as f32 * side * side * ld.coverage;
        out.push(ClusterCards { layer, mesh, cards, card_side_m: side, leaf_area_m2 });
    }
    out
}

// ── Cluster sprite geometry (fed to the billboard baker) ─────────────────
//
// NOT a CPU rasterizer. `billboard_bake` already renders arbitrary CPU parts
// orthographically against a `wgpu::Color::TRANSPARENT` clear, so handing it a
// sprig of the real v0.1087 blades gives back true leaf silhouettes, true
// overlap alpha and the species' own colour for free - with no new art, no new
// code path, and no chance of the sprite disagreeing with the geometry it
// stands in for.

/// The species' foliage facts, in real metres. Extracted from the four form
/// builders so a cluster sprite is baked from exactly the leaves the tree
/// grows - a sprite drawn at a different leaf scale from the near geometry
/// would pop at the handoff.
fn foliage_of(def: &TreeDef, h: f32, density: f32) -> Foliage {
    match def.form.as_str() {
        "conifer" => Foliage {
            clump: h * 0.050,
            leaf: (h * 0.022).clamp(0.30, 0.50),
            wid: 0.40,
            per_sprig: 4,
            density,
        },
        "umbrella" => Foliage {
            clump: h * 0.080,
            leaf: (h * 0.016).clamp(0.10, 0.20),
            wid: 0.58,
            per_sprig: 14,
            density,
        },
        // A palm builds its foliage inside `pinnate_frond` rather than through
        // `Foliage`; these are its leaflet facts, for the sprite path only.
        "palm" => Foliage {
            clump: h * 0.34 * 0.5,
            leaf: h * 0.34 * 0.20,
            wid: 0.16,
            per_sprig: 6,
            density,
        },
        _ => Foliage {
            clump: h * 0.092,
            leaf: (h * 0.011).clamp(0.09, 0.22),
            wid: 0.70,
            per_sprig: 3,
            density,
        },
    }
}

/// One five-petalled flower with NOTCHED petals and a stamen boss.
///
/// A Yoshino cherry blossom is 3.5 cm across with five notched petals, pink in
/// bud opening near-white. Through v0.1087 it was a single 9 cm pink triangle
/// - 2.6x life size and the wrong shape entirely - which is why the operator's
/// reference photos and our capture read as different plants.
fn flower(b: &mut PlantMeshBuilder, at: [f32; 3], dir: [f32; 3], size: f32, color: [f32; 3]) {
    let r = size * 0.5;
    let up = if dir[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let s1 = norm(cross(dir, up));
    let s2 = norm(cross(dir, s1));
    b.set_organ(Organ::Leaf); // a petal is blade tissue: same thin-tissue shading
    let n = 5u32; // Prunus is pentamerous, always
    for k in 0..n {
        let a = k as f32 / n as f32 * std::f32::consts::TAU;
        let (sa, ca) = (a.sin(), a.cos());
        let pd = norm([
            s1[0] * ca + s2[0] * sa,
            s1[1] * ca + s2[1] * sa,
            s1[2] * ca + s2[2] * sa,
        ]);
        let side = norm(cross(dir, pd));
        // Notched tip: two lobes either side of a notch that stops short of
        // the petal's full reach. That notch is the shape cue that separates a
        // cherry from every other white five-petalled flower.
        let notch = add(add(at, pd, r * 0.80), dir, r * 0.10);
        let l1 = add(add(add(at, pd, r), side, -r * 0.30), dir, r * 0.10);
        let l2 = add(add(add(at, pd, r), side, r * 0.30), dir, r * 0.10);
        let b1 = add(at, side, -r * 0.16);
        let b2 = add(at, side, r * 0.16);
        b.tri2(b1, l1, notch, color);
        b.tri2(b2, notch, l2, color);
    }
    // Stamen boss: a warm centre, derived from the petal colour rather than
    // invented, so a data row with a different blossom colour stays coherent.
    let stamen = [
        (color[0] * 0.85 + 0.20).min(1.0),
        (color[1] * 0.80 + 0.16).min(1.0),
        (color[2] * 0.45).min(1.0),
    ];
    for k in 0..5u32 {
        let a = k as f32 / 5.0 * std::f32::consts::TAU + 0.6;
        let (sa, ca) = (a.sin(), a.cos());
        let pd = norm([
            s1[0] * ca + s2[0] * sa,
            s1[1] * ca + s2[1] * sa,
            s1[2] * ca + s2[2] * sa,
        ]);
        let tipp = add(add(at, dir, r * 0.42), pd, r * 0.22);
        let base = add(at, pd, r * 0.05);
        let w = norm(cross(dir, pd));
        b.tri2(add(base, w, -r * 0.035), tipp, add(base, w, r * 0.035), stamen);
    }
    b.set_organ(Organ::Stem);
}

/// Mean card side this species' variants actually settle on, metres.
///
/// The LAI fit solves the card SIDE per variant (our skeleton has ~66 outer
/// twigs where a real cherry ramifies to thousands, so a card has to stand in
/// for a bigger tuft than the data's nominal size), and the sprite has to be
/// baked at that size with LIFE-SIZE flowers and leaves inside it. Baking at
/// the nominal size and then drawing the sprite bigger would scale a 3.5 cm
/// cherry blossom up to 6.7 cm - the exact defect (a 2.6x oversized flower)
/// this layer was built to remove.
pub fn mean_card_side(def: &TreeDef, layer: ClusterLayer) -> f32 {
    let Some(cd) = def.clusters.as_ref() else { return 0.0 };
    let nominal = cd.layer(layer).size_m;
    let mut sum = 0.0f32;
    let mut n = 0u32;
    for v in 0..def.variants.max(1) {
        let built = build_tree_and_cards(def, def.height_m, v.wrapping_mul(2_654_435_761));
        if let Some(c) = built.cards.iter().find(|c| c.layer == layer) {
            sum += c.card_side_m;
            n += 1;
        }
    }
    if n == 0 {
        nominal
    } else {
        sum / n as f32
    }
}

/// CPU geometry for ONE cluster sprite, centred on the origin with the TWIG
/// AXIS along +Y (the baker looks along -Z with Y up, and the card's tall axis
/// is its twig axis, so the sprite and the card agree by construction).
///
/// The sprite is built at `mean_card_side` with its element COUNTS scaled to
/// that size and its element SIZES left at life scale, so a bigger card
/// carries more flowers, never bigger ones.
///
/// Returns None for a species with no `clusters` block.
pub fn cluster_sprite_geometry(
    def: &TreeDef,
    layer: ClusterLayer,
    height_m: f32,
) -> Option<PlantMeshBuilder> {
    let cd = def.clusters.as_ref()?;
    let ld = cd.layer(layer);
    let h = height_m.max(0.5);
    // Element counts scale with the card the sprite will be drawn on: by area
    // for a scattered ball of sprigs, by length along each axis for a sleeve
    // of flowering twiglets.
    let k = (mean_card_side(def, layer) / ld.size_m.max(1e-3)).clamp(0.25, 4.0);
    let mut b = PlantMeshBuilder::new();
    // Seeded from the species id so a sprite is stable across runs and two
    // species never bake the identical scatter.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in def.id.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x1_0000_0001_b3);
    }
    let mut rng = Rng::new(hash ^ (layer as u64 + 1));
    let half = ld.size_m * k * 0.5;
    match layer {
        ClusterLayer::Leaf => {
            // A ball of real-scale blades: the same `sprig` the tree grows,
            // scattered through the cluster volume so the sprite's alpha is a
            // true overlap statistic rather than a guess.
            let fol = Foliage { clump: half, ..foliage_of(def, h, 1.0) };
            // Sprig roots sit far enough in that the leaves they carry still
            // land inside the card: a sprig reaches span + one leaf beyond its
            // root, and a sprite that overflows its frame is drawn at the
            // wrong scale (the baker frames on the geometry, not the card).
            let reach = fol.sprig_span() + fol.leaf;
            let rmax = (half - reach * 0.5).max(half * 0.15);
            let sprigs = ((ld.sprite_elements as f32 * k * k).round() as u32).clamp(4, 400);
            for k in 0..sprigs {
                let phase = k as f32 * 2.399_963 + rng.range(-0.4, 0.4);
                let d = tilt([0.0, 1.0, 0.0], rng.range(20.0, 160.0), phase);
                let at = add([0.0, 0.0, 0.0], d, rmax * rng.range(0.10, 1.0));
                let span = fol.sprig_span() * rng.range(0.85, 1.2);
                let mut srng = Rng::new(rng.next());
                sprig(&mut b, at, d, span, fol, def.leaf_color, &mut srng);
            }
        }
        ClusterLayer::Blossom => {
            // A SEGMENT OF FLOWERING TWIG: umbels every `umbel_spacing_m`
            // along the axis, each a rosette of real 3.5 cm flowers on short
            // pedicels. Baking the along-twig spacing INTO the sprite is what
            // reproduces the photograph (a branch wrapped in white-pink) at a
            // third of the cards a one-umbel-per-card layout would need.
            // Umbels along a run and runs across the card both scale with the
            // card's length, so the umbel SPACING stays the real few
            // centimetres at every card size.
            let n = ((ld.sprite_elements as f32 * k).round() as u32).clamp(2, 64);
            let sp = cd.umbel_spacing_m.max(0.005);
            let run = (n - 1) as f32 * sp;
            // PARALLEL TWIGLETS. One flowering twig is a few centimetres wide,
            // so a single run would leave a 0.45 m card almost entirely empty
            // and the layer's coverage - the number the LAI fit spends - would
            // be a fiction. A patch of blooming crown that size really does
            // hold several twigs of the last generation side by side.
            let runs = ((ld.sprite_runs as f32 * k).round() as u32).clamp(1, 24);
            for r in 0..runs {
                let fx = if runs > 1 { r as f32 / (runs - 1) as f32 - 0.5 } else { 0.0 };
                let lane = fx * half * 1.4;
                let fan = fx * 0.35; // the twiglets splay, they are not a comb
                // Each twiglet starts its own umbel series at a random phase,
                // or the runs line up into a visible grid: real blossom is a
                // mass, not a lattice. (Caught by eye on the first baked
                // sprite dump, .probe-rig-clusters/debug/bakes.)
                let phase0 = rng.range(0.0, sp);
                for k in 0..n {
                    let y = -run * 0.5 + phase0 + k as f32 * sp + rng.range(-sp * 0.3, sp * 0.3);
                    let base =
                        [lane + y * fan + rng.range(-0.03, 0.03), y, rng.range(-0.03, 0.03)];
                    for f in 0..cd.flowers_per_umbel.max(1) {
                        let a = f as f32 * 2.399_963 + rng.range(-0.3, 0.3);
                        // Flowers stand off the twig on short pedicels and
                        // face outward: an umbel is a little bouquet, not a
                        // disc.
                        let d = tilt([0.0, 1.0, 0.0], rng.range(40.0, 130.0), a);
                        let reach = cd.flower_size_m * rng.range(0.8, 1.6);
                        let at = add(base, d, reach);
                        b.tube(
                            base,
                            at,
                            cd.flower_size_m * 0.045,
                            cd.flower_size_m * 0.03,
                            3,
                            def.trunk_color,
                        );
                        flower(
                            &mut b,
                            at,
                            d,
                            cd.flower_size_m * rng.range(0.85, 1.15),
                            def.blossom_color,
                        );
                    }
                }
            }
        }
    }
    Some(b)
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
    b: &mut TreeParts,
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
    twigs: &mut Vec<Twig>,
) {
    if b.tri_count() > MAX_TRIS || len < 0.05 {
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
        // v runs from the BURIED root, so a bowing limb's bark is one
        // continuous run across its spine joints (v0.1089).
        b.bark_tube(
            p,
            add(p, d, seg_len + rb * 0.7),
            ra,
            rb,
            sides_for(depth),
            def.trunk_color,
            x,
        );
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
        // The twig sleeve cluster cards ride on (v0.1088). Recorded here, not
        // re-derived later, because "the outer two generations" is exactly the
        // set that carries foliage and the two must never disagree.
        twigs.push(Twig { from, dir, end: to, len, tip });
        // A clustered species keeps a much thinner blade layer: the cards are
        // the canopy now, and blades only earn their triangles inside ~2 m.
        let sprigs = if def.clusters.is_some() {
            if tip {
                5
            } else {
                3
            }
        } else if tip {
            16
        } else {
            8
        };
        leaf_cluster(&mut b.foliage, at, dir, f, color, sprigs, rng);
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
        limb(b, def, to, d, child_len, r1, r1, depth + 1, max_depth, fol.with_clump(0.86), rng, twigs);
    }
}

fn broadleaf(
    b: &mut TreeParts,
    def: &TreeDef,
    h: f32,
    density: f32,
    rng: &mut Rng,
    twigs: &mut Vec<Twig>,
) {
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
    let fol = foliage_of(def, h, density);
    for k in 0..n {
        let phase = k as f32 * 2.399_963 + rng.range(-0.3, 0.3);
        let d = tilt(lean, rng.range(26.0, 46.0), phase);
        // The bole top is the parent: burying the primaries in it also plugs
        // the bole's open top ring, which used to be a hole you could see the
        // sky through from above.
        let bole_top_r = r_base * 0.74;
        limb(
            b,
            def,
            top,
            d,
            (h - bole) * rng.range(0.40, 0.52),
            bole_top_r,
            bole_top_r,
            0,
            3,
            fol,
            rng,
            twigs,
        );
    }
}

fn conifer(b: &mut TreeParts, def: &TreeDef, h: f32, density: f32, rng: &mut Rng) {
    // A single straight leader with whorls of short, steeply drooping branches
    // that shorten toward the top: the classic conical silhouette.
    let r_base = h * 0.022;
    let top = [0.0, h, 0.0];
    // Leader radius at height fraction `f`, so a whorl branch can size itself
    // against the trunk it leaves instead of against the trunk BASE. With the
    // old flat `r_base * 0.22`, the topmost whorl's root ring (0.22 r_base) was
    // FATTER than the leader there (0.194 r_base) and stuck out through it.
    let leader_r = |f: f32| r_base * (1.0 + (0.16 - 1.0) * f);
    b.bark_tube([0.0, 0.0, 0.0], top, r_base, r_base * 0.16, 8, def.trunk_color, 0.0);
    // Close the leader's open apex ring with a short cone (16 triangles). It is
    // the one hole on a conifer you look straight down into from the air. Its
    // bark v continues from the leader's top so the pattern does not restart.
    b.bark_tube(
        top,
        [0.0, h + h * 0.012, 0.0],
        r_base * 0.16,
        0.0,
        8,
        def.trunk_color,
        h,
    );
    let whorls = 9;
    // A conifer's drawn element is a NEEDLE SPRAY, not a single needle: one
    // 30 mm needle would be far below a pixel and there is no budget for the
    // ~200k of them a real fir carries. 0.30-0.50 m of narrow strap is the
    // honest unit, and it is 3x smaller than the 1.2 m kite it replaces.
    let fol = foliage_of(def, h, density);
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
            b.bark_tube([0.0, y, 0.0], tip, lr * 0.42, lr * 0.14, 4, def.trunk_color, 0.0);
            leaf_cluster(&mut b.foliage, tip, d, fol, def.leaf_color, 10, rng);
            // A second clump midway keeps the branch from reading as a bare stick.
            let mid = add([0.0, y, 0.0], d, blen * 0.55);
            leaf_cluster(&mut b.foliage, mid, d, fol.with_clump(0.8), def.leaf_color, 7, rng);
        }
    }
}

fn umbrella(b: &mut TreeParts, def: &TreeDef, h: f32, density: f32, rng: &mut Rng) {
    // Acacia: a tall bare bole, then limbs that flatten hard into a wide,
    // level crown. The giveaway is that the crown is WIDER than the tree is
    // tall and its underside is flat.
    let bole = h * rng.range(0.52, 0.62);
    let r_base = h * 0.034;
    let top = [0.0, bole, 0.0];
    b.bark_tube([0.0, 0.0, 0.0], top, r_base, r_base * 0.66, 8, def.trunk_color, 0.0);
    let n = 5;
    // An acacia leaf is bipinnate: what reads at any real distance is a pinna,
    // a 0.10-0.18 m feather of leaflets, not a metre-wide blade. Acacia was the
    // most under-spent form in the tree budget (13% of MAX_TRIS), so it can
    // afford by far the densest sprigs, which is also what its flat crown wants.
    let fol = foliage_of(def, h, density);
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
        b.bark_tube(root, mid, r_base * 0.70, r_base * 0.34, 5, def.trunk_color, 0.0);
        // The crown layer: near-horizontal fans of foliage.
        for j in 0..3 {
            let p2 = phase + j as f32 * 1.9;
            let d2 = tilt([0.0, 1.0, 0.0], rng.range(80.0, 94.0), p2);
            let flen = h * rng.range(0.16, 0.26);
            let tip = add(mid, d2, flen);
            // Same again one level down: the fan is wider than the primary's
            // open tip ring, so burying it plugs that ring completely.
            let (froot, _) = welded_root(mid, d2, r_base * 0.34, r_base * 0.38, flen);
            b.bark_tube(froot, tip, r_base * 0.38, r_base * 0.12, 4, def.trunk_color, 0.0);
            leaf_cluster(&mut b.foliage, tip, [0.0, 1.0, 0.0], fol, def.leaf_color, 16, rng);
        }
    }
}

fn palm(b: &mut TreeParts, def: &TreeDef, h: f32, density: f32, rng: &mut Rng) {
    // No branches at all and no secondary thickening: a palm is a single
    // unbranched stem with a crown of fronds at the top, and an old palm is
    // not a fatter palm. Modelled as a gently curved stack of segments.
    let segs = 7;
    let r_base = h * 0.028;
    let curve = rng.range(-0.10, 0.10);
    let mut p = [0.0f32, 0.0, 0.0];
    let mut d = norm([curve, 1.0, rng.range(-0.08, 0.08)]);
    // Running arc length: a palm stem is one continuous column of leaf scars,
    // so its bark v must not restart at each of the seven segments (v0.1089).
    let mut v = 0.0f32;
    for i in 0..segs {
        let f = i as f32 / segs as f32;
        let seg = h / segs as f32;
        let to = add(p, d, seg);
        b.bark_tube(
            p,
            to,
            r_base * (1.0 - f * 0.35),
            r_base * (1.0 - (f + 0.15) * 0.35),
            7,
            def.trunk_color,
            v,
        );
        p = to;
        v += seg;
        d = norm([d[0] + curve * 0.06, d[1], d[2]]);
    }
    // Cap the stem's open top ring; the crown hides it from the side but not
    // from above, and a palm is a thing you fly over.
    let cap_r = r_base * 0.65;
    b.bark_tube(p, add(p, d, cap_r * 0.8), cap_r, 0.0, 7, def.trunk_color, v);
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
        // The frond stays on the FOLIAGE mesh, deliberately: its rachis carries
        // `leaf_color`, not `trunk_color`, so routing it onto the bark material
        // would paint 28 green rachises per palm bark-brown and tile a
        // trunk-scale bark texture onto a 3 cm stalk.
        pinnate_frond(&mut b.foliage, p, dd, frond, density, def.leaf_color, rng);
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
                // BOTH meshes count against the tree's budget (v0.1088): a
                // clustered species draws wood-and-blades plus one card mesh
                // per layer, and what MAX_TRIS bounds is the TREE.
                let built = build_tree_and_cards(&t, t.height_m, shipped_seed(v));
                // v0.1089: the wood is its own mesh, so the tree's budget is
                // foliage + wood + cards.
                let wood_tris = (built.mesh.indices.len() + built.wood.indices.len()) / 3;
                let card_tris: usize =
                    built.cards.iter().map(|c| c.mesh.indices.len() / 3).sum();
                let tris = wood_tris + card_tris;
                eprintln!(
                    "[budget] {:>7} v{v}: {tris:>5} tris ({:>3.0}% of {MAX_TRIS}) = {wood_tris} wood+blade \
                     + {card_tris} card",
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

    // ── Cluster cards (v0.1088) ──────────────────────────────────────────

    fn sakura() -> TreeDef {
        let r = registry();
        r.get(r.index_of("sakura").expect("sakura row")).unwrap().clone()
    }

    fn twigs_of(t: &TreeDef, seed: u32) -> Vec<Twig> {
        let mut tw = Vec::new();
        let _ = build_at_density(t, t.height_m, seed, 1.0, &mut tw);
        tw
    }

    /// Cards only, for the tests that measure the card layer alone.
    fn build_tree_and_cards_cards(t: &TreeDef, height_m: f32, seed: u32) -> Vec<ClusterCards> {
        build_tree_and_cards(t, height_m, seed).cards
    }

    // ── Wind coverage (v0.1089) ──────────────────────────────────────────

    /// A tree is now up to four materials - photoscan (19), foliage (20),
    /// cluster card (21), bark (22) - and the wind gate must reach all of them
    /// or the parts detach in a storm. It must ALSO stay opt-in for type 19,
    /// which is not plant-exclusive: the same type draws furniture and machine
    /// glTFs (engine/home_meshes.rs) and world decorations
    /// (engine/world_load.rs), and a type-keyed stamp would sway every bed and
    /// fridge indoors, permanently, shadows included.
    #[test]
    fn the_wind_gate_covers_every_tree_material_and_type_19_stays_opt_in() {
        let wgsl = crate::renderer::shader_loader::assembled_pbr_source();
        let at = wgsl.find("var wind_class = 0.0;").expect("the wind class block");
        let end = wgsl[at..].find("if (wind_class >= 0.5)").expect("the wind gate") + at;
        let block = &wgsl[at..end];
        for (lo, hi) in [(19.5, 20.5), (20.5, 21.5), (21.5, 22.5), (18.5, 19.5)] {
            let want = format!("wind_mt >= {lo} && wind_mt < {hi}");
            assert!(block.contains(&want), "the wind class block does not gate `{want}`");
        }
        // Exactly one arm may read params.w, and it is the type-19 arm.
        assert_eq!(
            block.matches("material.params.w").count(),
            1,
            "more than one material type opts into wind through params.w"
        );
        let pw = block.find("material.params.w").expect("checked above");
        let arm = block[..pw].rfind("wind_mt >=").expect("an arm precedes it");
        assert!(
            block[arm..].starts_with("wind_mt >= 18.5 && wind_mt < 19.5"),
            "params.w is read by a material type other than 19 - furniture would sway"
        );
        // And the displacement must be normalised by the instance scale, or a
        // photoscan (0.70-1.27 model units for a 16-22 m tree) leans ~11% of
        // what it should.
        assert!(
            wgsl.contains("let iscale = max(length(obj_model()[0].xyz), 1.0e-4);"),
            "the wind branch lost its per-mesh height normalisation"
        );
    }

    /// FURNITURE MUST NOT SWAY.
    ///
    /// Material type 19 is "textured mesh", and it is shared: besides the
    /// near-tree photoscans it draws every machine and furniture glTF
    /// (`engine/home_meshes.rs`, ~15 models in data/machines/home.ron - bed,
    /// sofa, bookcase, desk, fridge, table, rug, mirror, coat rack) and world
    /// decorations (`engine/world_load.rs`). Those sites must keep passing a
    /// ZERO in the params.w slot, because a non-zero there is now a wind class:
    /// the fallback breeze never reaches zero, so a stamped bookcase would
    /// shear and oscillate ~4 cm at ~0.9 Hz, indoors, forever, shadow included.
    ///
    /// Neither acceptance vantage can catch this - both are forest cameras -
    /// so it is caught here instead, by reading the call sites.
    #[test]
    fn no_furniture_or_decoration_site_opts_into_wind() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for rel in ["src/engine/home_meshes.rs", "src/engine/world_load.rs"] {
            let src = std::fs::read_to_string(root.join(rel)).expect(rel);
            let lines: Vec<&str> = src.lines().map(|l| l.trim()).collect();
            let mut found = 0;
            for (i, l) in lines.iter().enumerate() {
                if *l != "19.0," {
                    continue;
                }
                found += 1;
                // The emissive / wind-class argument is the next code line.
                let next = lines[i + 1..]
                    .iter()
                    .find(|l| !l.is_empty() && !l.starts_with("//"))
                    .copied()
                    .unwrap_or("");
                assert_eq!(
                    next, "0.0,",
                    "{rel}:{}: a type-19 material passes `{next}` in the params.w slot. That slot \
                     is the WIND CLASS (assets/shaders/pbr/00-bindings-vertex.wgsl); anything \
                     non-zero makes this mesh lean and sway in the wind, and this file draws \
                     furniture, machines and static decorations.",
                    i + 1
                );
            }
            assert!(found > 0, "{rel}: no type-19 material site found - did the file move?");
        }
    }

    // ── Baked bark (v0.1089) ─────────────────────────────────────────────

    /// The tile scales with the species and stays inside its clamp. A tile
    /// that tracked nothing would either tile bark at millimetre scale on a
    /// sapling or smear one plate over a metre of oak.
    #[test]
    fn bark_tile_scales_with_species() {
        let r = registry();
        let mut seen = std::collections::HashSet::new();
        for t in &r.trees {
            let tile = bark_tile_m(t);
            assert!(
                (0.30..=1.0).contains(&tile),
                "{}: bark tile {tile} m outside the clamp",
                t.id
            );
            seen.insert((tile * 1000.0) as i32);
        }
        assert!(seen.len() > 1, "every species got the same bark tile - the derivation is dead");
        // Taller species tile coarser, up to the clamp.
        let tall = r.trees.iter().max_by(|a, b| a.height_m.total_cmp(&b.height_m)).unwrap();
        let short = r.trees.iter().min_by(|a, b| a.height_m.total_cmp(&b.height_m)).unwrap();
        assert!(bark_tile_m(tall) > bark_tile_m(short), "tile does not track height");
    }

    /// WORLD-SPACE texel density, the silent-and-permanent UV failure.
    ///
    /// Along the limb, v is arc length over the tile, so every lengthwise edge
    /// must measure exactly `tile_m` of world per unit of v - no exceptions,
    /// no rounding. Around the ring, u is `circumference / tile` ROUNDED to a
    /// whole number of repeats (so the ring closes on a texture period), so the
    /// density is only approximate, and only where the tube is at least as fat
    /// as one tile. A twig thinner than a tile is pinned at one repeat, which
    /// is the deliberate floor: the alternative is a seam down every branch.
    #[test]
    fn bark_uvs_carry_world_scale_not_model_scale() {
        let r = registry();
        for t in r.trees.iter().map(as_procedural) {
            let built = build_tree_and_cards(&t, t.height_m, shipped_seed(0));
            let tile = bark_tile_m(&t);
            assert!(!built.wood.indices.is_empty(), "{}: no wood emitted", t.id);
            let mut v_edges = 0u32;
            let mut u_edges = 0u32;
            for f in built.wood.indices.chunks(3) {
                for (a, b) in [(0, 1), (1, 2), (2, 0)] {
                    let va = &built.wood.vertices[f[a] as usize];
                    let vb = &built.wood.vertices[f[b] as usize];
                    let du = vb.uv[0] - va.uv[0];
                    let dv = vb.uv[1] - va.uv[1];
                    let world = ((vb.position[0] - va.position[0]).powi(2)
                        + (vb.position[1] - va.position[1]).powi(2)
                        + (vb.position[2] - va.position[2]).powi(2))
                    .sqrt();
                    if du.abs() < 1e-6 && dv.abs() > 1e-6 {
                        // v counts AXIAL metres, so a lengthwise edge measures
                        // the cone's SLANT: axis length times sqrt(1 + taper^2).
                        // Steeply tapered stubs (the conifer apex cap, the palm
                        // crown cap) reach ~1.6x; a model-scale UV would be off
                        // by 10x or more, which is the failure being caught.
                        let m_per_tile = world / dv.abs();
                        assert!(
                            (tile * 0.98..=tile * 1.7).contains(&m_per_tile),
                            "{}: lengthwise bark density {m_per_tile:.4} m/tile against {tile:.4}",
                            t.id
                        );
                        v_edges += 1;
                    } else if dv.abs() < 1e-6 && du.abs() > 1e-6 && world > 1e-5 {
                        // Around the ring the density may COMPRESS freely (one
                        // repeat is the floor on a twig thinner than a tile,
                        // and a tapered tube compresses toward its tip), but it
                        // must never STRETCH past the rounding bound: repeats
                        // are round(circumference/tile) at the fat end, so the
                        // worst case is a ring 1.5 tiles round pinned at one
                        // repeat. A model-scale or fixed-0..1 ring - the silent
                        // failure this guards - lands 10x to 30x out.
                        let m_per_tile = world / du.abs();
                        assert!(
                            m_per_tile < tile * 1.6,
                            "{}: ring bark density {m_per_tile:.4} m/tile against {tile:.4} - \
                             a fixed 0..1 ring would show up exactly like this",
                            t.id
                        );
                        u_edges += 1;
                    }
                }
            }
            assert!(v_edges > 100 && u_edges > 100, "{}: {v_edges} v / {u_edges} u edges", t.id);
        }
    }

    /// The split itself: leaves on the foliage mesh, bark on the wood mesh, and
    /// the merged form still decodes as packed colour for the atlas bake.
    #[test]
    fn wood_splits_off_the_foliage_and_the_merged_form_stays_packed() {
        let r = registry();
        for t in r.trees.iter().map(as_procedural) {
            let built = build_tree_and_cards(&t, t.height_m, shipped_seed(1));
            assert!(!built.wood.indices.is_empty(), "{}: no wood", t.id);
            assert!(!built.mesh.indices.is_empty(), "{}: no foliage", t.id);
            // No wood face may carry the leaf/fruit organ bits: those are read
            // out of the PACKED channel, and wood's uv.x is a tile coordinate.
            // (This is also the guard against wood accidentally re-entering the
            // type-20 branch, where a small uv.x would decode as near-black.)
            for v in &built.wood.vertices {
                assert!(v.uv[0] < 1024.0, "{}: a wood uv.x of {} is a packed colour", t.id, v.uv[0]);
            }
            // The merged form is exactly foliage + wood, and every one of its
            // faces still round-trips the packed decode the atlas bake uses.
            let merged_tris = built.bake.indices.len() / 3;
            assert_eq!(
                merged_tris,
                (built.mesh.indices.len() + built.wood.indices.len()) / 3,
                "{}: merged mesh lost triangles",
                t.id
            );
            for f in built.bake.indices.chunks(3) {
                let uv = built.bake.vertices[f[0] as usize].uv;
                let (c, water) =
                    crate::terrain::planet_surface::unpack_uv_to_color(uv);
                assert!(!water, "{}: a merged face decoded as water", t.id);
                assert!(c.iter().all(|x| (0.0..=1.0).contains(x)), "{}: colour {c:?}", t.id);
            }
        }
    }

    /// The bake must TILE. A texture that does not wrap draws one hard seam
    /// down every trunk and one across every limb - and the seam is invisible
    /// in a unit test unless you look for it exactly like this: the step
    /// across the wrap must be no worse than a typical interior step.
    #[test]
    fn baked_bark_tiles_seamlessly_on_both_axes() {
        let t = registry().trees.iter().map(as_procedural).next().expect("a species");
        let px = BARK_PX as usize;
        let img = bake_bark_rgba(&t);
        let at = |x: usize, y: usize, c: usize| img[(y * px + x) * 4 + c] as f32;
        for c in 0..4 {
            let (mut wrap_x, mut inner_x, mut wrap_y, mut inner_y) = (0.0, 0.0, 0.0, 0.0);
            for i in 0..px {
                wrap_x += (at(px - 1, i, c) - at(0, i, c)).abs();
                inner_x += (at(px / 2, i, c) - at(px / 2 + 1, i, c)).abs();
                wrap_y += (at(i, px - 1, c) - at(i, 0, c)).abs();
                inner_y += (at(i, px / 2, c) - at(i, px / 2 + 1, c)).abs();
            }
            assert!(
                wrap_x <= inner_x * 3.0 + 64.0,
                "channel {c}: u wrap step {wrap_x:.0} vs interior {inner_x:.0} - the bake does \
                 not tile around the trunk"
            );
            assert!(
                wrap_y <= inner_y * 3.0 + 64.0,
                "channel {c}: v wrap step {wrap_y:.0} vs interior {inner_y:.0} - the bake does \
                 not tile along the limb"
            );
        }
    }

    /// One bark for every tree is the failure this whole increment exists to
    /// avoid, so measure it: no two species may share a plate field. The
    /// height channel (alpha) is the structural one - colour alone would pass
    /// this on a per-species TINT of one shared pattern, which is exactly the
    /// thing that must fail.
    #[test]
    fn every_species_gets_its_own_bark() {
        let r = registry();
        let fields: Vec<(String, Vec<f32>)> = r
            .trees
            .iter()
            .map(|t| {
                let img = bake_bark_rgba(t);
                // Sample every 4th texel: 16k samples is plenty for a
                // correlation and keeps the test quick.
                let s: Vec<f32> = img
                    .chunks_exact(4)
                    .step_by(4)
                    .map(|p| p[3] as f32 / 255.0)
                    .collect();
                (t.id.clone(), s)
            })
            .collect();
        for i in 0..fields.len() {
            for j in (i + 1)..fields.len() {
                let (a, b) = (&fields[i].1, &fields[j].1);
                let n = a.len() as f32;
                let (ma, mb) = (a.iter().sum::<f32>() / n, b.iter().sum::<f32>() / n);
                let mut cov = 0.0;
                let mut va = 0.0;
                let mut vb = 0.0;
                for k in 0..a.len() {
                    let (da, db) = (a[k] - ma, b[k] - mb);
                    cov += da * db;
                    va += da * da;
                    vb += db * db;
                }
                let corr = cov / (va.sqrt() * vb.sqrt()).max(1e-6);
                assert!(
                    corr < 0.9,
                    "{} and {} bake the same bark (correlation {corr:.3})",
                    fields[i].0,
                    fields[j].0
                );
            }
        }
    }

    /// DEV AID (permanent): dump every species' baked bark, plus two mip
    /// levels, to `debug/bark_*.png` so the plate field can be eyeballed
    /// without booting the world - the same role `dump_species_svg` plays for
    /// the geometry. Ignored by default; run with
    /// `cargo test --features native --lib dump_bark_png -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn dump_bark_png() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("debug");
        std::fs::create_dir_all(&dir).expect("debug dir");
        for t in registry().trees.iter() {
            let base = bake_bark_rgba(t);
            let levels =
                crate::renderer::billboard_bake::build_opaque_mip_chain(&base, BARK_PX);
            for li in [0usize, 3, 5] {
                let w = (BARK_PX >> li).max(1);
                let path = dir.join(format!("bark_{}_mip{li}.png", t.id));
                let img = image::RgbaImage::from_raw(w, w, levels[li].clone()).expect("size");
                img.save(&path).expect("write png");
            }
            let luma: Vec<f32> = base
                .chunks_exact(4)
                .map(|p| 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32)
                .collect();
            let n = luma.len() as f32;
            let m = luma.iter().sum::<f32>() / n;
            let sd = (luma.iter().map(|l| (l - m).powi(2)).sum::<f32>() / n).sqrt();
            let lo = luma.iter().cloned().fold(f32::MAX, f32::min);
            let hi = luma.iter().cloned().fold(f32::MIN, f32::max);
            eprintln!(
                "[bark] {:>7}: tile {:.2} m, luma mean {m:.1} sd {sd:.2} range {lo:.0}..{hi:.0}",
                t.id,
                bark_tile_m(t)
            );
        }
    }

    /// The height/AO channel must carry real relief, and the albedo must carry
    /// real contrast. Both are what the type-22 fragment branch spends: a flat
    /// alpha means no normal perturbation and no roughness break, i.e. plastic
    /// tubing with a texture on it.
    #[test]
    fn baked_bark_has_relief_and_contrast() {
        for t in registry().trees.iter() {
            let img = bake_bark_rgba(t);
            let n = (img.len() / 4) as f32;
            let alpha: Vec<f32> = img.chunks_exact(4).map(|p| p[3] as f32 / 255.0).collect();
            let mean = alpha.iter().sum::<f32>() / n;
            let sd = (alpha.iter().map(|a| (a - mean).powi(2)).sum::<f32>() / n).sqrt();
            let lo = alpha.iter().cloned().fold(f32::MAX, f32::min);
            assert!(sd > 0.06, "{}: bark height sd {sd:.3} - the surface is flat", t.id);
            assert!(
                lo < 0.55,
                "{}: deepest bark fissure only reaches {lo:.2} - no crevices",
                t.id
            );
            // Albedo contrast, measured the way the fidelity report measures it:
            // the spread of luma across the plate field, in 8-bit levels.
            let luma: Vec<f32> = img
                .chunks_exact(4)
                .map(|p| 0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32)
                .collect();
            let lm = luma.iter().sum::<f32>() / n;
            let lsd = (luma.iter().map(|l| (l - lm).powi(2)).sum::<f32>() / n).sqrt();
            assert!(
                lsd > 3.0,
                "{}: bark luma sd {lsd:.2} levels (mean {lm:.1}) - at the 8-bit floor, which is \
                 the defect this bake exists to fix",
                t.id
            );
        }
    }

    /// AABB centre of one card. A card is 4 triangles of 3 unshared vertices,
    /// so every card owns exactly 12 consecutive vertices and its quad is
    /// planar - the AABB centre IS the card centre.
    fn card_centres(m: &PlantMeshBuilder) -> Vec<[f32; 3]> {
        m.vertices
            .chunks(12)
            .map(|c| {
                let mut mn = [f32::MAX; 3];
                let mut mx = [f32::MIN; 3];
                for v in c {
                    for i in 0..3 {
                        mn[i] = mn[i].min(v.position[i]);
                        mx[i] = mx[i].max(v.position[i]);
                    }
                }
                [
                    0.5 * (mn[0] + mx[0]),
                    0.5 * (mn[1] + mx[1]),
                    0.5 * (mn[2] + mx[2]),
                ]
            })
            .collect()
    }

    fn point_segment_dist(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> f32 {
        let ab = sub(b, a);
        let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
        if l2 < 1e-9 {
            return dist(p, a);
        }
        let ap = sub(p, a);
        let t = ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / l2).clamp(0.0, 1.0);
        dist(p, add(a, ab, t))
    }

    /// THE GENERATOR-SIDE TWIN of the crown-gap image gate.
    ///
    /// The image gate ("sky through an isolated crown <= 30%") cannot run in
    /// CI, so this asserts the arithmetic behind it: the card layer must
    /// deliver the species' `target_lai` from cards * card_area * coverage
    /// over the crown's projected area, and the whole tree must still fit
    /// MAX_TRIS. A crown at LAI 2.6 transmits exp(-0.5 * 2.6) = 27% of the
    /// sky by Beer-Lambert; the geometric blade layer alone reached 0.31-0.50
    /// and transmitted 78-86%, which is why every tree read as a bare winter
    /// tree with sprinkles.
    #[test]
    fn cluster_cards_reach_target_lai_and_fit_the_budget() {
        let r = registry();
        for t in r.trees.iter().map(as_procedural) {
            let Some(cd) = t.clusters.clone() else { continue };
            for v in 0..t.variants.max(1) {
                let seed = shipped_seed(v);
                let built = build_tree_and_cards(&t, t.height_m, seed);
                let cards = &built.cards;
                assert!(!cards.is_empty(), "{} v{v}: a clustered species emitted no cards", t.id);
                let crown = crown_envelope(&t, t.height_m, seed);
                let area: f32 = cards.iter().map(|c| c.leaf_area_m2).sum();
                let lai = area / crown.projected_area_m2();
                let card_tris: usize = cards.iter().map(|c| c.mesh.indices.len() / 3).sum();
                let total = (built.mesh.indices.len() + built.wood.indices.len()) / 3
                    + card_tris;
                let n_cards: u32 = cards.iter().map(|c| c.cards).sum();
                // Expected ALPHA-TEST LAYERS per canopy pixel: every card
                // covers only `coverage` of its own area, so the depth
                // complexity a canopy pixel pays is LAI / coverage. This is
                // the one number that decides whether this arc goes wrong, and
                // it is invisible at 720p - print it every run.
                let mean_cov = cards
                    .iter()
                    .map(|c| cd.layer(c.layer).coverage * c.leaf_area_m2)
                    .sum::<f32>()
                    / area.max(1e-4);
                eprintln!(
                    "[lai] {:>7} v{v}: crown r {:.2} m, spread {:.2} m ({:.1} m2), {n_cards} cards, \
                     {:.1} m2 leaf, LAI {lai:.2} (target {:.2}), overdraw {:.1} layers, {total} tris \
                     ({card_tris} card)",
                    t.id,
                    crown.radius_m,
                    crown.spread_m,
                    crown.projected_area_m2(),
                    area,
                    cd.target_lai,
                    lai / mean_cov.max(0.01)
                );
                for c in cards.iter() {
                    eprintln!(
                        "        {:>8}: {} cards at {:.3} m, {:.1} m2",
                        c.layer.key(),
                        c.cards,
                        c.card_side_m,
                        c.leaf_area_m2
                    );
                }
                let lo = cd.target_lai * 0.75;
                let hi = cd.target_lai * 1.25;
                assert!(
                    (lo..=hi).contains(&lai),
                    "{} v{v}: cluster cards deliver LAI {lai:.2}, outside {lo:.2}..{hi:.2} - a real \
                     broadleaf crown carries 3-5 and the blade layer alone reaches 0.3-0.5, so a \
                     miss here means the crown still reads as a bare winter tree",
                    t.id
                );
                assert!(
                    total < MAX_TRIS,
                    "{} v{v}: {total} triangles (wood+blades+cards) reaches the {MAX_TRIS} ceiling",
                    t.id
                );
                assert!(
                    card_tris <= CARD_TRI_BUDGET,
                    "{} v{v}: {card_tris} card triangles over the {CARD_TRI_BUDGET} card budget",
                    t.id
                );
            }
        }
    }

    /// Cards must SLEEVE the twigs, not float in a clump volume.
    ///
    /// This is the placement half of the blossom finding, and it is a unit
    /// test rather than an image gate because "within 0.15 m of a branch axis"
    /// is not reliably measurable from a screenshot. Through v0.1087 the
    /// foliage was scattered through a 0.74 m ball around each twig end, which
    /// is why a cherry rendered as a pink dust cloud instead of a branch
    /// wrapped in blossom.
    #[test]
    fn cluster_cards_sleeve_the_twigs_they_belong_to() {
        let t = sakura();
        let seed = shipped_seed(0);
        let twigs = twigs_of(&t, seed);
        assert!(twigs.len() > 20, "only {} twigs recorded", twigs.len());
        let cards = build_tree_and_cards_cards(&t, t.height_m, seed);
        for c in &cards {
            let centres = card_centres(&c.mesh);
            assert_eq!(centres.len(), c.cards as usize, "card count disagrees with the mesh");
            let mut near = 0usize;
            let mut sum = 0.0f64;
            for p in &centres {
                let d = twigs
                    .iter()
                    .map(|w| point_segment_dist(*p, w.from, w.end))
                    .fold(f32::MAX, f32::min);
                sum += d as f64;
                if d <= c.card_side_m * 0.45 {
                    near += 1;
                }
            }
            let mean = sum / centres.len() as f64;
            eprintln!(
                "[sleeve] {} {}: {}/{} cards within {:.3} m of a twig, mean {:.3} m",
                t.id,
                c.layer.key(),
                near,
                centres.len(),
                c.card_side_m * 0.45,
                mean
            );
            assert!(
                near * 10 >= centres.len() * 9,
                "{}: only {near} of {} {} cards sit on a twig axis - they are scattered in the \
                 clump volume again",
                t.id,
                centres.len(),
                c.layer.key()
            );
            assert!(mean < 0.25, "{}: mean card-to-twig distance {mean:.3} m", c.layer.key());
        }
    }

    /// A cherry flowers BEFORE it leafs out, so a sakura in bloom must not
    /// render as a green-and-pink mix.
    #[test]
    fn a_blooming_species_keeps_its_leaf_layer_a_minority() {
        let t = sakura();
        let cd = t.clusters.clone().expect("sakura carries a cluster block");
        assert!(
            t.blossom_frac > cd.leaf_off_above_blossom_frac,
            "this test is about the in-bloom branch; sakura must trip it"
        );
        let cards = build_tree_and_cards_cards(&t, t.height_m, shipped_seed(0));
        let total: f32 = cards.iter().map(|c| c.leaf_area_m2).sum();
        let leaf: f32 = cards
            .iter()
            .filter(|c| c.layer == ClusterLayer::Leaf)
            .map(|c| c.leaf_area_m2)
            .sum();
        let share = leaf / total.max(1e-4);
        eprintln!("[bloom] sakura leaf share of card area: {share:.3}");
        assert!(
            share < 0.15,
            "leaf cards carry {share:.2} of the crown while in bloom - a real cherry is bare wood \
             sheathed in blossom, and this is what the NO GREEN-AND-PINK MIX gate measures"
        );
        assert!(share > 0.0, "the leaf layer vanished entirely; young leaves do appear");
    }

    /// The AO code rides in the integer part of `uv.x` and must not disturb
    /// the texture coordinate. Same arithmetic as the type-21 shader decode.
    #[test]
    fn card_uv_round_trips_the_ao_code_exactly() {
        for &u in &[0.0f32, 1.0] {
            for &v in &[0.0f32, 1.0] {
                for i in 0..=63u32 {
                    let ao = i as f32 / 63.0;
                    let uv = encode_card_uv(u, v, ao);
                    let (du, dv, dao) = decode_card_uv(uv);
                    assert!((du - u).abs() < 1e-5, "u {u} decoded as {du} (uv {uv:?})");
                    assert!((dv - v).abs() < 1e-6, "v {v} decoded as {dv}");
                    assert!((dao - ao).abs() < 0.01, "ao {ao} decoded as {dao}");
                }
            }
        }
    }

    /// A card lit by its own flat normal is cardboard. Every corner must carry
    /// a normal bent toward "outward from the cluster centre" (and weakly
    /// toward the crown centre), so a tuft shades as a rounded volume with a
    /// bright sun side, a dark far side and a soft terminator.
    #[test]
    fn card_normals_are_spherified_not_flat() {
        let t = sakura();
        let cards = build_tree_and_cards_cards(&t, t.height_m, shipped_seed(0));
        let c = cards.first().expect("at least one card layer");
        let mut worst_spread: f32 = 0.0;
        let mut flat_cards = 0usize;
        let mut n = 0usize;
        for card in c.mesh.vertices.chunks(12) {
            // Spread = the largest angle between any two of the card's own
            // corner normals. A flat quad scores exactly 0.
            let mut spread: f32 = 0.0;
            for a in card {
                for b in card {
                    let d = (a.normal[0] * b.normal[0]
                        + a.normal[1] * b.normal[1]
                        + a.normal[2] * b.normal[2])
                        .clamp(-1.0, 1.0);
                    spread = spread.max(d.acos().to_degrees());
                }
            }
            if spread < 5.0 {
                flat_cards += 1;
            }
            worst_spread = worst_spread.max(spread);
            n += 1;
        }
        eprintln!("[spherify] {n} cards, max corner-normal spread {worst_spread:.1} deg, {flat_cards} flat");
        assert!(n > 0, "no cards to check");
        assert_eq!(flat_cards, 0, "{flat_cards} cards still carry one flat quad normal");
        assert!(
            worst_spread > 30.0,
            "corner normals span only {worst_spread:.1} deg - the spherify blend is gone and every \
             card will render as a uniform-brightness sticker"
        );
    }

    /// Nothing occludes ambient inside a crown unless something bakes it. The
    /// AO scalar must actually vary: shell cards near 1, interior cards down
    /// at the core value, or the crown has no lit-shell / dark-core gradient
    /// and backlit foliage washes out to pale grey-green.
    #[test]
    fn cluster_ao_darkens_the_crown_interior() {
        let t = sakura();
        let cards = build_tree_and_cards_cards(&t, t.height_m, shipped_seed(0));
        let mut lo = 1.0f32;
        let mut hi = 0.0f32;
        let mut sum = 0.0f64;
        let mut n = 0usize;
        for c in &cards {
            for v in &c.mesh.vertices {
                let (_, _, ao) = decode_card_uv(v.uv);
                lo = lo.min(ao);
                hi = hi.max(ao);
                sum += ao as f64;
                n += 1;
            }
        }
        let mean = sum / n as f64;
        eprintln!("[ao] sakura card AO: min {lo:.3}, max {hi:.3}, mean {mean:.3}");
        assert!(hi > 0.85, "no card sits on the lit shell (max AO {hi:.2})");
        assert!(
            lo < 0.55,
            "the darkest card still keeps {lo:.2} of its ambient - the crown has no interior, so \
             it will read as a uniformly bright cloud"
        );
        assert!(
            lo >= CLUSTER_CORE_AO * 0.5 - 1e-3,
            "AO fell below the {CLUSTER_CORE_AO} core floor"
        );
    }

    /// The baked sprite must fit the card that samples it, or the cluster is
    /// drawn at the wrong scale. The baker frames on the geometry's own AABB,
    /// so the sprite content has to span the card side (within the framing
    /// margin) in BOTH axes.
    #[test]
    fn cluster_sprite_geometry_fits_its_card() {
        let r = registry();
        for t in r.trees.iter() {
            let Some(cd) = t.clusters.as_ref() else { continue };
            for layer in ClusterLayer::ALL {
                let b = cluster_sprite_geometry(t, layer, t.height_m).expect("sprite geometry");
                assert!(!b.vertices.is_empty(), "{} {}: empty sprite", t.id, layer.key());
                let mut mn = [f32::MAX; 3];
                let mut mx = [f32::MIN; 3];
                for v in &b.vertices {
                    for i in 0..3 {
                        mn[i] = mn[i].min(v.position[i]);
                        mx[i] = mx[i].max(v.position[i]);
                    }
                }
                // The sprite is baked at the size the CARDS settle on, not at
                // the data's nominal size (see `mean_card_side`).
                let side = mean_card_side(t, layer);
                let w = (mx[0] - mn[0]).max(mx[2] - mn[2]);
                let h = mx[1] - mn[1];
                eprintln!(
                    "[sprite] {} {}: {} tris, {w:.3} x {h:.3} m in a {side:.3} m card (nominal {:.2})",
                    t.id,
                    layer.key(),
                    b.indices.len() / 3,
                    cd.layer(layer).size_m
                );
                assert!(
                    w > side * 0.35 && w < side * 1.5,
                    "{} {}: sprite is {w:.2} m wide in a {side:.2} m card",
                    t.id,
                    layer.key()
                );
                assert!(
                    h > side * 0.35 && h < side * 1.5,
                    "{} {}: sprite is {h:.2} m tall in a {side:.2} m card",
                    t.id,
                    layer.key()
                );
            }
        }
    }

    /// Flower morphology, from the data the sprite is built out of: a Yoshino
    /// cherry blossom is 3.5 cm across in 3-6 flower umbels a few centimetres
    /// apart, not a 9 cm triangle scattered through a 0.74 m ball.
    #[test]
    fn blossom_data_matches_a_real_cherry() {
        let t = sakura();
        let cd = t.clusters.expect("sakura cluster block");
        assert!(
            (0.02..=0.05).contains(&cd.flower_size_m),
            "flower {} m: a Yoshino cherry blossom is 3.5 cm",
            cd.flower_size_m
        );
        assert!(
            (3..=6).contains(&cd.flowers_per_umbel),
            "{} flowers per umbel: a cherry carries 3-6",
            cd.flowers_per_umbel
        );
        assert!(
            (0.02..=0.07).contains(&cd.umbel_spacing_m),
            "umbels {} m apart: a flowering twig spaces them a few centimetres",
            cd.umbel_spacing_m
        );
        // ...and the sprite has to be a coherent piece of that twig: the
        // umbels it carries must span roughly the card it is drawn on.
        let run = (cd.blossom.sprite_elements.max(1) - 1) as f32 * cd.umbel_spacing_m;
        assert!(
            run > cd.blossom.size_m * 0.5 && run < cd.blossom.size_m * 1.3,
            "{} umbels at {} m span {run:.2} m on a {:.2} m card - the sprite and the card \
             disagree about how much twig they show",
            cd.blossom.sprite_elements,
            cd.umbel_spacing_m,
            cd.blossom.size_m
        );
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
