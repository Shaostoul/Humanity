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

use super::billboard_bake::leaf_colour::{self, LeafVariation};
use super::plant_mesh::{ring_basis, ring_dir, ring_point, Organ, PlantMeshBuilder};
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
    /// STEM SLENDERNESS, `H/D` - height over diameter at breast height, both
    /// in the same units. Forestry's h/d ratio, and the one number that says
    /// how thick this species' trunk is (v0.1103). 0, the default, falls back
    /// to the GROWTH FORM's law; see `tree_allometry::stem_base_radius`.
    ///
    /// It belongs in `data/vegetation/trees.ron` beside `height_m` because it
    /// is a measurement of a species, not of a form: a savanna acacia (22-30),
    /// an open-grown oak (25-40), a dominant fir (40-60) and a birch (55-75)
    /// are four different trees, and that spread is most of what makes a mixed
    /// stand read as a stand rather than as one tree at four scales.
    #[serde(default)]
    pub slenderness: f32,
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
    /// the card layer's. The blades out-resolve a `CLUSTER_SPRITE_PX` sprite
    /// only inside ~1 m (90 deg FOV, 2560 wide, 512 px sprites since v0.1090),
    /// so they are a close-range detail layer now, not the canopy - and they
    /// are kept INSIDE the card mass so they never form the silhouette.
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
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn length(v: [f32; 3]) -> f32 {
    dot(v, v).sqrt()
}
fn mul(v: [f32; 3], s: f32) -> [f32; 3] {
    [v[0] * s, v[1] * s, v[2] * s]
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
    /// Shaft radius above which a junction earns extra flare rings and a
    /// two-strip collar. Scales with the INSTANCE height, not the species
    /// default, so a sapling's small junctions are not resolved as if they were
    /// a mature tree's. See `FLARE_RING_MIN_H_FRAC`.
    flare_min_r: f32,
    /// Triangles this build spent RESOLVING base flares: the extra rings packed
    /// into flare runs plus the second collar strip. Instrumentation, so "what
    /// did the flare cost" is a CI number per species rather than an estimate
    /// (the flare gate prints it).
    flare_tris: usize,
    /// Every branch junction this build made, in emission order (v0.1098).
    ///
    /// Permanent instrumentation, not debug scaffolding: the back-poke gate
    /// (`no_branch_pokes_out_the_back_of_its_parent`) reads the REAL numbers
    /// the generator used rather than re-deriving them from a copy of the
    /// placement maths, which is the only way a gate can actually prove the
    /// shipped geometry is clean. ~150 records of 60 bytes per tree.
    pub forks: Vec<Fork>,
    /// One entry per LIMB, recording the single azimuthal repeat count every
    /// tube segment of that limb drew its bark with (v0.1100). Same role as
    /// `forks`: `bark_uv_repeats_are_continuous_along_every_limb` proves the
    /// count never changed mid-limb by reading what was DRAWN.
    pub bark_runs: Vec<BarkRun>,
    /// Tube segments emitted, all limbs. The gate checks this equals the sum
    /// over `bark_runs`, so a segment drawn outside any run cannot hide.
    pub bark_tubes: usize,
}

/// One limb's bark UV run (v0.1100): the ONE azimuthal repeat count its tube
/// segments share, and the radii that run spanned.
///
/// See the block comment above `open_bark_run` for why a limb gets exactly one
/// count. `reps_lo`/`reps_hi` are the extremes actually PASSED to `bark_tube`
/// rather than the value handed out, so a caller that recomputed its own would
/// widen them and fail the gate.
pub(crate) struct BarkRun {
    /// The radius the count was derived from: the limb's widest drawn ring.
    pub ref_r: f32,
    pub reps_lo: f32,
    pub reps_hi: f32,
    pub segments: u32,
    /// Widest and narrowest plain-taper radius any segment of this run drew,
    /// i.e. how far the plate scale foreshortens from root to tip.
    pub r_fat: f32,
    pub r_thin: f32,
}

/// What one tube segment needs to know about the limb it belongs to.
///
/// Bundled rather than passed as five more arguments because every field is a
/// property of the LIMB or of the segment's place along it, and the whole
/// point of both v0.1100 changes is that a limb is one continuous thing: one
/// repeat count, one bark run, one flare law.
#[derive(Clone, Copy)]
pub(crate) struct BarkSeg {
    /// Azimuthal repeats for the whole limb - see `open_bark_run`.
    pub reps: f32,
    /// Arc length from the limb's root ring at the NEAR ring, metres. `v` is
    /// this over the tile, so a limb is one continuous texture run.
    pub v0_m: f32,
    /// The limb's directional flare, evaluated at the PROFILE STATION each
    /// ring took its radius from. That is not the same as the ring's geometric
    /// distance from the root: the joint overshoot draws the far ring past its
    /// own station, and evaluating the flare there instead would make the
    /// overlap step (this segment's far ring would be a different width from
    /// the next segment's near ring at the same station).
    pub near: Option<FlareAt>,
    pub far: Option<FlareAt>,
}

impl TreeParts {
    fn new(def: &TreeDef, height_m: f32) -> Self {
        TreeParts {
            foliage: PlantMeshBuilder::new(),
            wood: PlantMeshBuilder::new(),
            wood_packed: PlantMeshBuilder::new(),
            tile_m: bark_tile_m(def),
            flare_min_r: height_m.max(0.5) * FLARE_RING_MIN_H_FRAC,
            flare_tris: 0,
            forks: Vec::new(),
            bark_runs: Vec::new(),
            bark_tubes: 0,
        }
    }

    /// Triangles emitted so far across the drawn parts. `MAX_TRIS` bounds the
    /// TREE, not one of its meshes, so the recursion's budget check must see
    /// wood and foliage together exactly as it did when they shared a builder.
    fn tri_count(&self) -> usize {
        (self.foliage.indices.len() + self.wood.indices.len()) / 3
    }

    /// Open a bark UV run for one limb and return the azimuthal repeat count
    /// every tube segment in that limb must use (v0.1100).
    ///
    /// ONE COUNT PER LIMB, NOT PER SEGMENT. u is measured in TILES of `tile_m`
    /// metres, so the honest count for a ring is `circumference / tile`, and it
    /// has to be a WHOLE NUMBER or the ring does not close on a texture period
    /// and every limb carries a wrap seam. Through v0.1099 each tube segment
    /// rounded its own fat end, which is exact per segment and DISCONTINUOUS
    /// between them: a tapering bole steps 5, 4, 4, 3, 3, 2, 2, 2, 1, 1 repeats
    /// up its length, and every step is a ring where the baked voronoi plates
    /// visibly change size. The operator saw it as "a large voronoi cell
    /// texture and a smaller one underneath", at the trunk, from two metres.
    ///
    /// Deriving it once from the limb's WIDEST DRAWN RING (the flared weld, or
    /// the root ring where there is no flare) buys continuity at the price of
    /// foreshortening: the plates compress toward the tip in proportion to the
    /// taper, exactly as they already did WITHIN one segment. That is the right
    /// trade twice over - it is the direction real bark goes (young thin wood
    /// carries finer plates than an old butt log), and the reference being the
    /// widest ring means u never STRETCHES anywhere on the limb, which is the
    /// failure mode that reads as smeared plastic.
    fn open_bark_run(&mut self, ref_r: f32) -> f32 {
        let reps = ((std::f32::consts::TAU * ref_r) / self.tile_m.max(1e-3)).round().max(1.0);
        self.bark_runs.push(BarkRun {
            ref_r,
            reps_lo: f32::MAX,
            reps_hi: 0.0,
            segments: 0,
            r_fat: 0.0,
            r_thin: f32::MAX,
        });
        reps
    }

    /// A bark tube: the same ring positions and smooth normals
    /// `PlantMeshBuilder::tube` draws for a plain circular limb, emitted into
    /// the WOOD mesh with real cylindrical UVs - u around the ring, v along the
    /// limb, both measured in TILES of `tile_m` metres - and into the
    /// packed-colour twin from the SAME numbers, so the two representations
    /// cannot drift.
    ///
    /// WORLD-SPACE TEXEL DENSITY, not model-space. u spans
    /// `circumference / tile_m` tiles rather than a fixed 0..1, so the bark on
    /// a 0.7 m bole and the bark on a 2 cm twig are the same physical size. A
    /// fixed 0..1 would squeeze the whole texture around a twig and smear it
    /// 30x, which is the classic silent UV failure. The repeat count comes from
    /// `seg.reps` - ONE value for the whole limb (`open_bark_run`) - and the
    /// ring is NOT uv-wrapped: every quad carries its own vertices, so u runs
    /// 0..reps monotonically and the derivative used for mip selection never
    /// jumps.
    ///
    /// THE RING IS NOT ALWAYS A CIRCLE (v0.1100). When the segment carries a
    /// `Flare`, each vertex's radius is the plain taper times a DIRECTIONAL
    /// multiplier, so a branch base is an ellipse standing in the plane of its
    /// parent's axis - see the `Flare` block comment. The surface normal picks
    /// up the ellipse's azimuthal slope as well as the axial one, or the collar
    /// would light as though it were still round.
    #[allow(clippy::too_many_arguments)]
    fn bark_tube(
        &mut self,
        from: [f32; 3],
        to: [f32; 3],
        r0: f32,
        r1: f32,
        sides: u32,
        color: [f32; 3],
        seg: BarkSeg,
    ) {
        let axis = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
        let alen = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2])
            .sqrt()
            .max(1e-6);
        let ax = [axis[0] / alen, axis[1] / alen, axis[2] / alen];
        let (side, up) = ring_basis(ax);
        let n = sides.max(3);
        let tile = self.tile_m.max(1e-3);
        let reps = seg.reps;
        let (v_a, v_b) = (seg.v0_m / tile, (seg.v0_m + alen) / tile);
        // What this segment contributed to its limb's run, read back by the UV
        // gate: a segment that arrived with a different count than its
        // neighbours widens `reps_lo..reps_hi` and the gate fires.
        self.bark_tubes += 1;
        if let Some(run) = self.bark_runs.last_mut() {
            run.reps_lo = run.reps_lo.min(reps);
            run.reps_hi = run.reps_hi.max(reps);
            run.segments += 1;
            run.r_fat = run.r_fat.max(r0.max(r1));
            // Only POSITIVE radii: a terminal cap tube runs to exactly 0 (the
            // conifer apex, the palm crown), and letting that set `r_thin`
            // would report an infinite plate foreshortening for a cone tip
            // that is one texel wide.
            for r in [r0, r1] {
                if r > 0.0 {
                    run.r_thin = run.r_thin.min(r);
                }
            }
        }
        for i in 0..n {
            let a0 = (i as f32) / (n as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32) / (n as f32) * std::f32::consts::TAU;
            // Radius AND azimuthal slope at each of the quad's four corners.
            let (ra0, da0) = ring_at(seg.near, r0, side, up, a0);
            let (ra1, da1) = ring_at(seg.near, r0, side, up, a1);
            let (rb0, db0) = ring_at(seg.far, r1, side, up, a0);
            let (rb1, db1) = ring_at(seg.far, r1, side, up, a1);
            let b0 = ring_point(from, side, up, a0, ra0);
            let b1 = ring_point(from, side, up, a1, ra1);
            let t0 = ring_point(to, side, up, a0, rb0);
            let t1 = ring_point(to, side, up, a1, rb1);
            // The surface normal of a flared tube: the radial direction tilted
            // back along the axis by the AXIAL slope (a truncated cone, which
            // is what a fat trunk tapering to a twig needs to light correctly)
            // and around the ring by the AZIMUTHAL slope of the ellipse.
            let nrm = |ang: f32, r: f32, dr_da: f32, r_near: f32, r_far: f32| {
                let m = ring_dir(side, up, ang);
                let t = ring_dir(side, up, ang + std::f32::consts::FRAC_PI_2);
                let slope = (r_near - r_far) / alen;
                let k = if r > 1e-6 { dr_da / r } else { 0.0 };
                norm([
                    m[0] + ax[0] * slope - t[0] * k,
                    m[1] + ax[1] * slope - t[1] * k,
                    m[2] + ax[2] * slope - t[2] * k,
                ])
            };
            let n0a = nrm(a0, ra0, da0, ra0, rb0);
            let n1a = nrm(a1, ra1, da1, ra1, rb1);
            let n0b = nrm(a0, rb0, db0, ra0, rb0);
            let n1b = nrm(a1, rb1, db1, ra1, rb1);
            let u0 = (i as f32) / (n as f32) * reps;
            let u1 = ((i + 1) as f32) / (n as f32) * reps;
            self.wood
                .card_tri([b0, t0, t1], [n0a, n0b, n1b], [[u0, v_a], [u0, v_b], [u1, v_b]]);
            self.wood
                .card_tri([b0, t1, b1], [n0a, n1b, n1a], [[u0, v_a], [u1, v_b], [u1, v_a]]);
            // The packed-colour twin for the single-mesh consumers (the sprite
            // atlas bake and the shipped-build fallback), built from the SAME
            // corner values rather than from a second call that has to be kept
            // in step by hand.
            self.wood_packed.tri_smooth([b0, t0, t1], [n0a, n0b, n1b], color);
            self.wood_packed.tri_smooth([b0, t1, b1], [n0a, n1b, n1a], color);
        }
    }

    /// Flat disc closing a limb's terminal ring, emitted ONCE per fork.
    ///
    /// `tube` writes no end caps, so the parent's last ring is an open pipe.
    /// Through v0.1097 that hole was plugged by burying the children inside the
    /// parent, which is exactly the defect this release removes; the honest
    /// replacement is to actually close the hole. `sides` triangles, and it is
    /// only spent where children are spawned - a terminal shoot's hole is
    /// already covered by its cluster cap card.
    ///
    /// `flare` is the limb's own flare at the capped ring's station (v0.1100),
    /// so the disc's rim IS that ring, vertex for vertex. A circular cap on an
    /// elliptical ring would sink into the wood on the strong side and stand
    /// proud on the flush side: a lip all the way round every fork.
    fn bark_cap(
        &mut self,
        at: [f32; 3],
        ax: [f32; 3],
        r: f32,
        sides: u32,
        color: [f32; 3],
        flare: Option<FlareAt>,
    ) {
        if r <= 1e-5 {
            return;
        }
        let (side, up) = ring_basis(ax);
        let n = sides.max(3);
        for i in 0..n {
            let a0 = (i as f32) / (n as f32) * std::f32::consts::TAU;
            let a1 = ((i + 1) as f32) / (n as f32) * std::f32::consts::TAU;
            let p0 = ring_point(at, side, up, a0, ring_at(flare, r, side, up, a0).0);
            let p1 = ring_point(at, side, up, a1, ring_at(flare, r, side, up, a1).0);
            // Wound so the face looks the way the limb points, matching the
            // outward winding `tube` uses for the wall it closes.
            self.wood_packed.tri_smooth([at, p1, p0], [ax, ax, ax], color);
            self.wood.card_tri([at, p1, p0], [ax, ax, ax], [[0.5, 0.5], [0.0, 0.0], [1.0, 0.0]]);
        }
    }

    /// Weld a child limb onto its parent and return the point its spine starts
    /// at - which is ON the parent's surface, never inside it (v0.1098).
    ///
    /// Emits the BRANCH COLLAR: a skirt whose outer edge is the child's first
    /// ring, vertex for vertex, and whose inner edge is that same ring
    /// projected onto the parent's surface. That is what carries the visual
    /// join now that no child geometry is buried, and it is the anatomically
    /// real thing too - a branch collar is a swelling of parent tissue around
    /// the branch base, not a stick pushed into a hole.
    /// THE COLLAR MEETS THE FLARE (v0.1099, DIRECTIONAL in v0.1100). The limb's
    /// first ring is the FLARED ring, so the skirt's outer edge grows with it
    /// and the whole join gets wider rather than pinching - and since the flare
    /// is now an ellipse rather than a sleeve, the skirt's outer edge is that
    /// ellipse, vertex for vertex. The root is placed with the WIDEST drawn
    /// radius (`weld_r_max`, which the flare puts on the crotch azimuth, the
    /// same azimuth that sits deepest toward the parent): `surface_root`'s
    /// clearance is what keeps the drawn ring out of the parent's wood, so it
    /// has to be given the radius that is actually drawn there or the back-poke
    /// guarantee is void.
    ///
    /// A junction big enough to resolve (`flare_min_r`) gets TWO strips instead
    /// of one, with the middle ring pushed out along the average of the two
    /// surface normals. One strip meets the flared ring at a hard shoulder -
    /// the skirt is a straight cone and the limb's own profile is curving hard
    /// there, so the silhouette breaks at exactly the place the operator was
    /// looking. The bulged middle ring is the fillet that closes that angle,
    /// and it is anatomy rather than fudge: a branch collar is a rounded
    /// swelling of parent tissue, convex on the underside and filling the axil
    /// above.
    ///
    /// Returns the spine start AND the limb's azimuthal repeat count, because
    /// the collar and every tube segment of the limb draw with the same one
    /// (v0.1100): the bark pattern flows out of the trunk, through the collar
    /// and up the limb at one plate scale.
    fn weld_child(&mut self, j: Junction, s: LimbShape, color: [f32; 3]) -> ([f32; 3], f32) {
        let n = s.sides.max(3);
        // The WIDEST radius drawn at the weld, and therefore what has to clear
        // the parent's wood.
        let rw = s.weld_r_max();
        let start = surface_root(j, s.dir, rw);
        self.forks.push(Fork {
            parent: j,
            start,
            shape: LimbShape { sides: n, ..s },
            weld_r: rw,
            rings: Vec::new(),
        });
        let (side, up) = ring_basis(s.dir);
        let tile = self.tile_m.max(1e-3);
        // The limb's ONE repeat count, from its widest ring - see
        // `open_bark_run`. Opening the run here rather than in the callers is
        // what makes "collar and limb share a count" true by construction.
        let reps = self.open_bark_run(rw);
        let steps = if s.r0 >= self.flare_min_r { 2usize } else { 1 };
        self.flare_tris += (steps - 1) * n as usize * 2;
        // Rows of the skirt, from the feet on the parent out to the limb's own
        // first ring. Each entry is (position, normal, v in tiles).
        let mut rows: Vec<Vec<([f32; 3], [f32; 3], f32)>> = Vec::with_capacity(steps + 1);
        for _ in 0..=steps {
            rows.push(Vec::with_capacity(n as usize));
        }
        for i in 0..n {
            let a = (i as f32) / (n as f32) * std::f32::consts::TAU;
            // Shading continuity at both edges: the collar's outer edge takes
            // the child's radial normal (so it lights as one surface with the
            // limb) and its inner edge takes the parent's, so the skirt melts
            // into the trunk instead of ringing it with a hard rim.
            //
            // The outer edge is the limb's own first ring - the ELLIPSE the
            // flare draws, not a circle of `rw` - and its normal carries the
            // ellipse's azimuthal slope for the same reason the tube's does.
            let (rv, dr_da) = ring_at(s.at(0.0), s.base_radius_at(0.0), side, up, a);
            let v = ring_point(start, side, up, a, rv);
            let m = ring_dir(side, up, a);
            let t = ring_dir(side, up, a + std::f32::consts::FRAC_PI_2);
            let k = if rv > 1e-6 { dr_da / rv } else { 0.0 };
            let nv = norm([m[0] - t[0] * k, m[1] - t[1] * k, m[2] - t[2] * k]);
            let f = j.project(v);
            let nf = j.surface_normal(f);
            let mut pos: Vec<[f32; 3]> = Vec::with_capacity(steps + 1);
            let mut nrm: Vec<[f32; 3]> = Vec::with_capacity(steps + 1);
            pos.push(f);
            nrm.push(nf);
            for k in 1..steps {
                let t = k as f32 / steps as f32;
                // The two normals cancel where the parent's surface and the
                // limb's face each other - the rear azimuth of a perpendicular
                // branch, which is buried in the parent and invisible - so fall
                // back to the limb's own rather than normalising noise.
                let blend = mix3(nf, nv, t);
                let nm = if length(blend) > 1e-3 { norm(blend) } else { nv };
                // The fillet: bulge out along the blended normal by a fraction
                // of the skirt's own height, peaking mid-span.
                pos.push(add(mix3(f, v, t), nm, dist(f, v) * 0.16 * (t * (1.0 - t) * 4.0)));
                nrm.push(nm);
            }
            pos.push(v);
            nrm.push(nv);
            // v runs BACKWARDS from the limb's own v=0 by the collar's real
            // ARC LENGTH, so the bark pattern flows out of the trunk and into
            // the branch instead of restarting at the joint - and so a bulged
            // fillet measures its own longer path rather than the chord, which
            // is what keeps world-space texel density exact through the weld.
            let mut vt = vec![0.0f32; steps + 1];
            for k in (0..steps).rev() {
                vt[k] = vt[k + 1] - dist(pos[k], pos[k + 1]) / tile;
            }
            for k in 0..=steps {
                rows[k].push((pos[k], nrm[k], vt[k]));
            }
        }
        for k in 0..steps {
            for i in 0..n as usize {
                let ii = (i + 1) % n as usize;
                let (a0, a1) = (rows[k][i], rows[k][ii]);
                let (b0, b1) = (rows[k + 1][i], rows[k + 1][ii]);
                let (u0, u1) =
                    ((i as f32) / (n as f32) * reps, ((i + 1) as f32) / (n as f32) * reps);
                // Same winding as `tube` with the inner ring as `from`.
                self.wood_packed.tri_smooth([a0.0, b0.0, b1.0], [a0.1, b0.1, b1.1], color);
                self.wood_packed.tri_smooth([a0.0, b1.0, a1.0], [a0.1, b1.1, a1.1], color);
                self.wood.card_tri(
                    [a0.0, b0.0, b1.0],
                    [a0.1, b0.1, b1.1],
                    [[u0, a0.2], [u0, b0.2], [u1, b1.2]],
                );
                self.wood.card_tri(
                    [a0.0, b1.0, a1.0],
                    [a0.1, b1.1, a1.1],
                    [[u0, a0.2], [u1, b1.2], [u1, a1.2]],
                );
            }
        }
        (start, reps)
    }

    /// One straight run of flared limb, welded onto `j`: the collar, then a
    /// tube whose rings follow `limb_base_radius_at` times the limb's
    /// directional flare, densified through the flare run (`ring_stations`).
    /// Returns (root ring centre, drawn far end).
    ///
    /// The forms that draw a branch as a single frustum - the conifer's whorl
    /// branches, the acacia's primaries and fans - go through here rather than
    /// calling `bark_tube` themselves, so there is exactly ONE radius law in
    /// the generator. `limb` runs the same law over a bowed spine.
    fn flared_run(&mut self, j: Junction, s: LimbShape, color: [f32; 3]) -> ([f32; 3], [f32; 3]) {
        let (root, reps) = self.weld_child(j, s, color);
        let stations = ring_stations(s.len, s.r0, 1, self.flare_min_r);
        self.flare_tris += (stations.len() - 2) * s.sides.max(3) as usize * 2;
        let mut prev = root;
        for w in stations.windows(2) {
            // PLAIN taper radii: the directional flare rides on top of them
            // per vertex, inside `bark_tube` (v0.1100).
            let (ra, rb) = (s.base_radius_at(w[0]), s.base_radius_at(w[1]));
            let to = add(root, s.dir, w[1]);
            let seg = BarkSeg { reps, v0_m: w[0], near: s.at(w[0]), far: s.at(w[1]) };
            self.bark_tube(prev, to, ra, rb, s.sides, color, seg);
            if w[0] == 0.0 {
                self.note_fork_ring(prev, s.dir, ra, s.sides, s.at(w[0]));
            }
            self.note_fork_ring(to, s.dir, rb, s.sides, s.at(w[1]));
            prev = to;
        }
        (root, prev)
    }

    /// Record one ring of the limb that most recently welded itself on, for the
    /// back-poke gate. Only the first `FORK_GATE_RINGS` are kept - past those
    /// the limb is unambiguously clear of its parent.
    fn note_fork_ring(
        &mut self,
        centre: [f32; 3],
        ax: [f32; 3],
        r: f32,
        sides: u32,
        flare: Option<FlareAt>,
    ) {
        if let Some(f) = self.forks.last_mut() {
            if f.rings.len() < FORK_GATE_RINGS {
                f.rings.push(ForkRing { centre, ax, r, sides: sides.max(3), flare });
            }
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
    let (best, twigs) = build_accepted(def, height_m.max(0.5), seed);
    // Cards are planned from the ACCEPTED pass's twigs (v0.1090). They used to
    // be planned from the FIRST pass's, and the two are not the same set: the
    // recursion's `tri_count() > MAX_TRIS` guard fires at a different point once
    // the density changes, so a second pass that draws fewer leaves grows MORE
    // wood - and every twig it grew beyond the first pass's set had no card
    // sleeve at all. Bare branch ends standing out of the blossom is exactly
    // what the operator's v0.1088.4 capture shows.
    let cards = match &def.clusters {
        Some(cd) => emit_cluster_cards(def, cd, &twigs, seed),
        None => Vec::new(),
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

/// The density fit, returning the ACCEPTED pass together with the twig set
/// THAT pass actually grew.
///
/// Split out of `build_tree_and_cards` (v0.1090) so the card planner and
/// `crown_envelope` both see the same wood the drawn mesh has. Anything that
/// derives geometry from twigs has to derive it from the accepted pass's
/// twigs, or it is describing a tree that was thrown away.
///
/// TWO PASSES BECAME UP TO FOUR (v0.1096; the loop below bounds at 4). The two-pass form assumed the
/// correction is exact, which it is only while the FIRST pass stays under
/// `limb`'s own `MAX_TRIS` guard: leaf count is strictly linear in the density
/// knob and wood is completely unaffected by it, so one measurement predicts
/// the right density. The moment a build is heavy enough for that guard to
/// fire, the guard prunes subtrees, `wood_tris` is measured on a TRUNCATED
/// tree, the correction lands short, and the rebuilt full tree overshoots.
/// That is exactly how oak v0 came out at 8648 triangles against a 8600
/// ceiling the moment the crown got deeper. Iterating re-measures on a
/// complete tree and converges; keeping the LARGEST candidate that fits also
/// replaces the old "only accept a rebuild that grew" rule with the thing that
/// rule was approximating.
fn build_accepted(def: &TreeDef, h: f32, seed: u32) -> (TreeParts, Vec<Twig>) {
    // A PREDICTION of the card cost, used to size the blade layer's share of
    // the budget and to judge whether a candidate fits. The cards that actually
    // ship are re-planned from whichever pass wins.
    let cards_of = |tw: &[Twig]| -> usize {
        match &def.clusters {
            Some(cd) => emit_cluster_cards(def, cd, tw, seed)
                .iter()
                .map(|c| c.mesh.indices.len() / 3)
                .sum(),
            None => 0,
        }
    };
    let lo = if def.clusters.is_some() { 0.05 } else { 0.2 };
    let mut density = 1.0f32;
    let mut best: Option<(TreeParts, Vec<Twig>, usize)> = None;
    // How much of MAX_TRIS to aim at. It STEPS DOWN after a pass that did not
    // fit, because a pass that did not fit is a pass where `limb`'s guard
    // pruned subtrees, and every prune makes the next build grow MORE wood
    // than the one that was measured - so re-aiming at the same target chases
    // a ceiling that keeps moving. Backing off converges in one or two steps
    // (oak v0: 8648 -> 8606 -> under, measured 2026-08-02).
    let mut aim = BUDGET_TARGET;

    for _ in 0..4 {
        let mut tw: Vec<Twig> = Vec::new();
        let parts = build_at_density(def, h, seed, density, &mut tw);
        let cards = cards_of(&tw);
        let total = parts.tri_count() + cards;
        let leaves = leaf_tri_count(&parts.foliage);
        let wood_tris = parts.tri_count().saturating_sub(leaves);

        // Keep the LARGEST candidate that fits the ceiling; while nothing fits
        // yet, keep the SMALLEST, so a species that cannot be made to fit still
        // ships the least truncated tree instead of the first one tried.
        let keep = match &best {
            None => true,
            Some((bp, _, bc)) => {
                let bt = bp.tri_count() + bc;
                match (total <= MAX_TRIS, bt <= MAX_TRIS) {
                    (true, false) => true,
                    (true, true) => total > bt,
                    (false, false) => total < bt,
                    (false, true) => false,
                }
            }
        };

        // How many triangles the GEOMETRIC blade layer should get.
        //
        // Without cards it is "everything the budget has left", which is what
        // v0.1086 established. WITH cards the blades stop being the canopy and
        // become a close-range detail layer - a 512 px sprite (v0.1090) out
        // resolves the screen past ~0.9 m at 2560 wide - so they take a
        // fraction of the card layer instead, and the tree comes out CHEAPER
        // than it was.
        let fits = total <= MAX_TRIS;
        // STEP THE AIM DOWN BEFORE sizing the next blade layer, not after. A
        // tree that overran was measured with subtrees pruned, so `wood_tris`
        // is under-read and the correction it implies can be a NO-OP - the
        // linear model says "you are already spending the target" while the
        // tree is over the ceiling, the loop sees a zero step and stops. Oak v0
        // sat at 8622 of 8600 in exactly that state. Backing the target off
        // first guarantees a real step every time a pass misses.
        if !fits {
            aim = (aim - 0.05).max(0.60);
        }
        let want_leaf = match &def.clusters {
            Some(cd) => cards as f32 * cd.near_blade_tri_frac.max(0.0),
            None => MAX_TRIS as f32 * aim - wood_tris as f32,
        };
        if keep {
            best = Some((parts, tw, cards));
        }
        if leaves == 0 || want_leaf <= 0.0 {
            break;
        }
        let step = want_leaf / leaves as f32;
        // Converged: already fitting and within 3% of the target spend, which
        // is inside the per-sprig fractional coin's own scatter.
        if fits && (0.97..=1.03).contains(&step) {
            break;
        }
        let next = (density * step).clamp(lo, 8.0);
        if (next - density).abs() < 1e-3 {
            break;
        }
        density = next;
    }
    let (parts, twigs, _) = best.expect("the loop always runs at least one build");
    (parts, twigs)
}

/// Crown envelope of a species as the card planner sees it. Public so the
/// LAI gate and any caller sizing a card material can ask the generator
/// rather than re-deriving it.
///
/// Built from the ACCEPTED pass, exactly like the cards (v0.1090): this is the
/// denominator of the leaf-area-index fit, so measuring it against a tree that
/// was discarded would make the fit spend the wrong crown.
pub fn crown_envelope(def: &TreeDef, height_m: f32, seed: u32) -> CrownEnvelope {
    let (_, twigs) = build_accepted(def, height_m.max(0.5), seed);
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
    let mut b = TreeParts::new(def, h);
    let mut rng = Rng::new(seed as u64 ^ 0x7ee_5eed);
    match def.form.as_str() {
        // EVERY form records its twigs (v0.1102). Through v0.1101 only
        // `broadleaf` was handed this vector, so the other three forms emitted
        // no cluster cards for any species, ever - see
        // `every_form_emits_cards_when_given_a_cluster_block`.
        "conifer" => conifer(&mut b, def, h, density, &mut rng, twigs),
        "umbrella" => umbrella(&mut b, def, h, density, &mut rng, twigs),
        "palm" => palm(&mut b, def, h, density, &mut rng, twigs),
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

/// One sample of the main stem: where the spine is, which way it is going, and
/// how thick it is there. Laterals are shed from these, so a lateral can size
/// and weld itself against the stem AT ITS OWN HEIGHT instead of against the
/// bole top (v0.1096).
#[derive(Clone, Copy)]
struct StemSample {
    p: [f32; 3],
    dir: [f32; 3],
    r: f32,
    /// Fraction of the stem's length this sample sits at, 0 at the base.
    f: f32,
}

/// The main STEM: one continuous leader from the ground up through the crown,
/// curved and ROOT-FLARED (v0.1067). The flare is the detail that reads as "a
/// tree grew here" rather than "a cylinder was placed here": real trunks swell
/// sharply in the last half-metre where they meet the ground, and a
/// dead-straight constant-taper post is an instant giveaway.
///
/// v0.1096: this used to be the BOLE ALONE - it stopped at the first branching
/// and returned one point, and every primary limb left from that single point.
/// A decurrent broadleaf does not do that: it keeps a leader and sheds laterals
/// over 1-3 m of stem, older and lower laterals being longer, which is what
/// gives a crown its DEPTH and its ragged lower boundary. So the stem now runs
/// to ~0.85 of tree height and returns its spine samples; `broadleaf` picks
/// stations off them.
///
/// `flare_run_m` is how far up the flare reaches, in METRES rather than as a
/// fraction of the stem: the stem got about three times longer in v0.1096 and a
/// fractional flare would have smeared a half-metre buttress over two metres.
#[allow(clippy::too_many_arguments)]
fn trunk(
    b: &mut TreeParts,
    def: &TreeDef,
    base: [f32; 3],
    dir: [f32; 3],
    len: f32,
    r_base: f32,
    r_top_frac: f32,
    flare_run_m: f32,
    segs: u32,
) -> (Vec<StemSample>, [f32; 3]) {
    let segs = segs.max(2);
    let mut p = base;
    let mut d = dir;
    // Running arc length for the bark v coordinate: the stem is ONE texture
    // run, not one restart per segment (v0.1089).
    let mut v = 0.0f32;
    let run = (flare_run_m / len.max(1e-4)).clamp(0.01, 1.0);
    // Flare: an extra radius bump near the ground. Kept gentle and spread over
    // a longer run - a short sharp flare reads as a rocket fin, not buttress
    // roots.
    let radius = |f: f32| {
        let flare = 1.0 + (TRUNK_FLARE_PEAK - 1.0) * (1.0 - (f / run).min(1.0)).powi(2);
        r_base * (1.0 + (r_top_frac - 1.0) * f) * flare
    };
    // ONE bark repeat count for the whole stem, from its widest ring - the
    // flared butt (v0.1100). The visual consequence at the trunk, stated
    // plainly: the plates hold true world scale where you stand next to the
    // tree and foreshorten with the taper going up, reaching about 4-5x
    // compression at the leader top of a broadleaf (r_top_frac 0.26 under a
    // 1.28x root flare), where the stem is 6-8 cm across and buried in crown.
    // Through v0.1099 that same range was drawn at true density but in TEN
    // discrete steps, and every step was a ring where the voronoi plates
    // visibly changed size - which is the seam the operator photographed.
    // Continuous-and-foreshortened beats correct-and-stepped, and it is what
    // real bark does anyway: fine plates on young thin wood, coarse on a butt.
    let reps = b.open_bark_run(radius(0.0));
    let mut out: Vec<StemSample> = Vec::with_capacity(segs as usize + 1);
    out.push(StemSample { p, dir: d, r: radius(0.0), f: 0.0 });
    // Where the DRAWN tube ends, which is past the last spine point by the
    // joint overshoot. The terminal limb welds onto that plane, so the caller
    // needs the drawn end and not the spine end (v0.1098).
    let mut drawn_end = p;
    for s in 0..segs {
        let f0 = s as f32 / segs as f32;
        let f1 = (s + 1) as f32 / segs as f32;
        let ra = radius(f0);
        let rb = radius(f1);
        let seg = len / segs as f32;
        let to = add(p, d, seg);
        // Same joint overshoot as `limb`: the stem sways slightly between
        // segments, and without the overlap each sway opens a hairline slit.
        drawn_end = add(p, d, seg + rb * 0.5);
        b.bark_tube(
            p,
            drawn_end,
            ra,
            rb,
            8,
            def.trunk_color,
            BarkSeg { reps, v0_m: v, near: None, far: None },
        );
        p = to;
        // The SPINE advances by `seg`; the extra `rb * 0.5` is joint overshoot
        // that overlaps the next segment, so v must not count it twice.
        v += seg;
        // A very slight sway so the stem is not a plumb line.
        d = norm([d[0] + 0.012, d[1], d[2] - 0.008]);
        out.push(StemSample { p, dir: d, r: rb, f: f1 });
    }
    (out, drawn_end)
}

/// Interpolate the stem at a fraction `f` of its length.
///
/// Laterals sit wherever the crown wants them, not on segment boundaries, so
/// this returns the exact spine point, axis and radius there. The radius is
/// what a lateral welds itself into, and getting it from the stem AT THAT
/// HEIGHT (rather than from the bole top, as every primary did through
/// v0.1095) is what keeps a high lateral thinner than a low one.
fn stem_at(stem: &[StemSample], f: f32) -> StemSample {
    if stem.is_empty() {
        return StemSample { p: [0.0; 3], dir: [0.0, 1.0, 0.0], r: 0.05, f: 0.0 };
    }
    let f = f.clamp(0.0, 1.0);
    for w in stem.windows(2) {
        let (a, c) = (w[0], w[1]);
        if f <= c.f || c.f >= 1.0 {
            let span = (c.f - a.f).max(1e-6);
            let t = ((f - a.f) / span).clamp(0.0, 1.0);
            return StemSample {
                p: mix3(a.p, c.p, t),
                dir: norm(mix3(a.dir, c.dir, t)),
                r: a.r + (c.r - a.r) * t,
                f,
            };
        }
    }
    *stem.last().unwrap()
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
    /// How far this species' leaves stray from its authored `leaf_color`
    /// (v0.1109). Carried here because `sprig` is the only place that knows a
    /// single leaf is about to be emitted, and the spread is a per-species
    /// measurement - see `billboard_bake::leaf_colour`.
    var: LeafVariation,
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

/// How far to shrink a clustered species' blade clump so the near-field blade
/// layer stays INSIDE the card mass it is meant to add parallax to (v0.1090).
///
/// A sleeve card sits `CLUSTER_SLEEVE_OFFSET * side` off the twig axis and
/// reaches `side / 2` sideways, so the card mass around a twig ends at
/// `(CLUSTER_SLEEVE_OFFSET + 0.5) * side`. Blades scatter out to roughly one
/// clump radius from the twig, so the clump must sit inside that with margin.
///
/// The NOMINAL card side out of the species data is deliberately used rather
/// than the fitted one: the fit only ever grows the card (it is solved for
/// target leaf area and clamped at the crown's own scale), so sizing against
/// the nominal is the conservative direction, and it also breaks what would
/// otherwise be a circular dependency - the blade layer is emitted while the
/// wood is being built, and the LAI fit that decides the real card side cannot
/// run until every twig exists.
fn near_blade_clump_k(cd: &ClusterDef, fol: Foliage) -> f32 {
    let nominal = cd.leaf.size_m.max(cd.blossom.size_m).max(1e-3);
    let mass = (CLUSTER_SLEEVE_OFFSET + 0.5) * nominal;
    // What a sprig adds BEYOND its root, which is where the clump radius stops
    // measuring: the shoot's own run (jittered up to 1.2x) plus one leaf
    // standing off its far end (up to 1.25x). Budgeting for this explicitly is
    // the difference between a cap that holds at every density and a magic
    // fraction that quietly stops holding the moment the density fit moves -
    // and the fit DOES move, by design.
    let reach = fol.sprig_span() * 1.2 + fol.leaf * 1.25;
    let cap = (mass * 0.80 - reach).max(nominal * 0.10);
    (cap / fol.clump.max(1e-4)).min(1.0)
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
        // THIS leaf's colour (v0.1109). Keyed on the leaf's own position, NOT
        // drawn from `rng`: the scatter stream is measured and gated upstream
        // (sprite coverage, the LAI fit, the triangle budget all read off it),
        // so one extra draw here would move geometry to change a colour.
        let lc = leaf_colour::jitter(color, fol.var, leaf_colour::key_at(node, j as u64));
        blade(b, node, ld, fol.leaf * rng.range(0.75, 1.25), fol.wid, lc, rng);
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





/// Where the `n` sleeve stations of one twig sit, as fractions of its chord.
///
/// SPANNING, not inset (v0.1090). Through v0.1089 station `s` sat at
/// `(s + 0.5) / n`, so the sleeve stopped half a step short of BOTH ends: the
/// last card's centre was `len / 2n` inside the tip and the first was the same
/// distance out from the junction. On a 0.42 m terminal shoot at two stations
/// that is 10 cm of bare wood at each end of every twig in the crown - the
/// operator's "tubes spear through the blossom masses and exit the far side".
/// Spanning `0..=1` instead puts a card centred ON the tip and one ON the
/// junction, so the card mass reaches half a card BEYOND the wood at both ends,
/// and it costs exactly zero extra cards.
fn sleeve_station_frac(s: u32, n: u32) -> f32 {
    if n <= 1 {
        0.5
    } else {
        s.min(n - 1) as f32 / (n - 1) as f32
    }
}

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
    /// VERTICAL extent of the foliage-bearing wood, metres: highest minus
    /// lowest point on any twig a card or a blade rides on (v0.1096).
    ///
    /// This is the numerator of LIVE CROWN RATIO, the forestry measure of how
    /// much of a stem carries crown. Recorded because it is the one crown
    /// number nothing measured, and the thing it measures was badly wrong:
    /// through v0.1095 every broadleaf hung all of its foliage off a single
    /// fan rooted at ONE point on the bole, so the crown came out a
    /// flat-bottomed plate about 0.25 of tree height deep against a 7 m
    /// spread - a 3.5:1 mushroom cap whose underside was a straight
    /// horizontal cut, and a stand of them all bottomed out at the same world
    /// height. Real forest-grown temperate broadleaves run 0.35-0.60, and an
    /// open-grown Prunus x yedoensis carries foliage from a 1.5-2.5 m crown
    /// base to the apex, i.e. 0.7-0.8.
    pub depth_m: f32,
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
    /// Far end of the LAST bark tube this limb emitted, INCLUDING the joint
    /// overshoot (`rb * 0.7`, see `limb`). This - not `end` - is the point a
    /// card mass has to envelop, and the two differ by up to a tip radius.
    /// Recorded rather than re-derived because the spine bows: reconstructing
    /// it in the coverage test would mean copying `limb`'s droop integration,
    /// and a copy that drifts is a test that certifies the wrong geometry.
    tube_end: [f32; 3],
    /// Radius of that last ring, metres.
    tip_r: f32,
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
///
/// `centre`, `radius_m` and `spread_m` are measured on twig ENDS only, exactly
/// as they were before `depth_m` existed - they are the denominators of the LAI
/// fit and the AO falloff, and moving them would re-open every measured
/// coverage number. `depth_m` measures the whole foliage-bearing RUN (junction
/// to tip), because that is what a card sleeve covers and what the eye reads as
/// the crown's vertical extent.
fn crown_of(twigs: &[Twig]) -> CrownEnvelope {
    if twigs.is_empty() {
        return CrownEnvelope {
            centre: [0.0, 0.0, 0.0],
            radius_m: 1.0,
            spread_m: 1.0,
            depth_m: 0.0,
        };
    }
    let n = twigs.len() as f32;
    let mut c = [0.0f32; 3];
    for t in twigs {
        c = [c[0] + t.end[0], c[1] + t.end[1], c[2] + t.end[2]];
    }
    let centre = [c[0] / n, c[1] / n, c[2] / n];
    let mut r = 0.0f32;
    let mut s = 0.0f32;
    let mut y_lo = f32::MAX;
    let mut y_hi = f32::MIN;
    for t in twigs {
        r = r.max(dist(t.end, centre));
        s = s.max((t.end[0] - centre[0]).hypot(t.end[2] - centre[2]));
        y_lo = y_lo.min(t.from[1]).min(t.end[1]);
        y_hi = y_hi.max(t.from[1]).max(t.end[1]);
    }
    CrownEnvelope {
        centre,
        radius_m: r.max(0.25),
        spread_m: s.max(0.25),
        depth_m: (y_hi - y_lo).max(0.0),
    }
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
    // ITERATED, not one shot (v0.1096). A single division by the overrun ratio
    // is not a guarantee: `round` can hand back the same station count it was
    // given (round(3 / 1.2) = round(2.5) = 3), so the budget survived only
    // because no species had ever overrun it by a small margin. The first
    // deeper crown did - sakura came out at 3692 card triangles against a 3400
    // budget, and the CI twin caught it. Floor division plus a loop always
    // converges, and it stops the moment nothing moved so a species pinned at
    // the one-station-per-twig floor cannot spin.
    let mut passes = 0;
    while tris > CARD_TRI_BUDGET && passes < 12 {
        passes += 1;
        let f = (tris as f32 / CARD_TRI_BUDGET as f32).max(1.02);
        let mut moved = false;
        for s in stations.iter_mut() {
            let ns = ((*s as f32 / f).floor() as u32).max(1);
            moved |= ns != *s;
            *s = ns;
        }
        tris = card_count(&stations) * 4;
        if !moved {
            break;
        }
    }
    if tris > CARD_TRI_BUDGET {
        log::debug!(
            "[Cluster] {}: {tris} card triangles after {passes} stretch passes (budget \
             {CARD_TRI_BUDGET}) - the one-station-per-twig floor is the binding constraint",
            def.id
        );
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
                // Stations SPAN the twig, junction to tip (see
                // `sleeve_station_frac`), so the sleeve envelops the whole
                // visible run of wood instead of stopping half a step short of
                // each end.
                let f = sleeve_station_frac(s, n);
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
                // `bark_cap` is only spent where a limb FORKS (v0.1098), so a
                // terminal tip is an open pipe you can look down against the
                // sky - and covering it with foliage the twig was going to
                // carry anyway is cheaper than a disc. The card sits just
                // BEYOND the
                // tip, facing along the twig, so it covers the hole from every
                // angle the hole is visible from.
                //
                // Measured against `tube_end`, not `end` (v0.1090): the last
                // bark ring overshoots the spine end by 0.7 of a tip radius,
                // and the cap has to clear the metal it is capping. The offset
                // therefore takes whichever is larger, the card-relative stand
                // off or two tip radii past the true ring.
                let up = if t.dir[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
                let w = norm(cross(t.dir, up));
                let h = norm(cross(t.dir, w));
                let stand = (side * CLUSTER_SLEEVE_OFFSET).max(t.tip_r * 2.0);
                let c = add(t.tube_end, t.dir, stand);
                let depth = (crown.radius_m - dist(t.end, crown.centre)).max(0.0);
                let ao = (-k_ao * depth).exp().clamp(CLUSTER_CORE_AO * 0.5, 1.0);
                emit_card(&mut mesh, c, t.dir, w, h, side * 0.5, ao, t.end, crown.centre);
                cards += 1;
            }
        }
        // The layer's cards now stand in for a canopy, so their normals have
        // to average to nothing the way a canopy's do - see
        // `balance_card_normals`. Shading only; positions are already final.
        balance_card_normals(&mut mesh);
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
///
/// ── ONE SPRIG MUST FIT ONE CARD (v0.1102) ────────────────────────────────
///
/// A carded species' foliage facts are not free: `cluster_sprite_geometry`
/// bakes a BALL OF THESE SPRIGS into the sprite a card samples, and the baker
/// frames on the geometry, so a sprig bigger than the card is not clipped - it
/// is SHRUNK, and every needle on the tree silently renders at the wrong scale.
///
/// THE MEASUREMENT, all from this file's `dump_crown_png` on 2026-08-03. Write
/// `reach = sprig_span + leaf`: what one sprig adds beyond its own root. The
/// baked sprite's extent against the card the LAI fit settles on:
///
///   species  reach   card    reach/card   baked sprite   sprite/card
///   oak      0.495   1.72        0.29        1.78 m          1.04
///   sakura   0.225   0.67        0.34        0.68 m          1.02
///   birch    0.385   1.07        0.36        1.11 m          1.04
///   acacia   0.576   1.06        0.54        1.41 m          1.33
///   pine     0.500   0.75        0.67        1.13 m          1.51  REJECTED
///   fir      0.660   1.02        0.65        1.57 m          1.54  REJECTED
///   palm     3.264   0.67        4.87        5.73 m          8.55  REJECTED
///
/// It is not linear: past `reach/card ~ 0.4` the sprig scatter radius hits its
/// own floor and the extent runs away with `reach` instead of with the card.
/// The three shipped broadleaves all sit at 0.29-0.36 and bake within 4% of
/// their card, so that is the band to design into; `cluster_sprite_geometry_
/// fits_its_card` rejects at 1.5x, which `reach/card >= 0.65` reaches.
///
/// So a CARDED conifer and umbrella draw a different element from an uncarded
/// one, exactly as `limb` already draws a different NUMBER of sprigs for a
/// carded species (3 against 16): once cards carry the canopy, the blade layer
/// is close-range parallax detail and its unit is the real botanical one.
///   conifer  a 0.10-0.15 m BRANCHLET (a fir's last-year shoot) rather than a
///            0.30-0.50 m needle SPRAY -> reach 0.40 m on a 22 m fir against a
///            ~1.02 m card, i.e. 0.39, just outside the broadleaf band.
///            Uncarded it KEEPS the spray, because with no cards that layer is
///            the canopy and its area per triangle goes as element length
///            squared - shrinking it 3.7x with nothing to replace it would thin
///            the crown 13-fold.
///   umbrella the element (a 0.10-0.20 m bipinnate PINNA) was always right; the
///            SPRIG was not, at 14 pinnae strung over a 1.0 m run. 4 -> reach
///            0.43 m against a ~1.06 m card. Uncarded it keeps 14, which is
///            what its unspent triangle budget bought (see the acacia note in
///            `data/vegetation/trees.ron`, whose second blocker this closes).
///
/// NOTE FOR THE NEXT PERSON: `per_sprig` is NOT the knob that compensates for a
/// smaller element. It multiplies `sprig_span` directly, so raising it walks
/// straight back into the rejected rows above (8 per sprig was tried and baked
/// fir at 1.54x its card). What buys back the lost leaf area is the number of
/// SPRIGS in the sprite (`leaf.sprite_elements`, data) and the number of cards,
/// neither of which touches the sprig's own size.
fn foliage_of(def: &TreeDef, h: f32, density: f32) -> Foliage {
    // Cards change what the blade layer IS, so they change what it draws.
    let carded = def.clusters.is_some();
    let var = leaf_colour::of(&def.id);
    match def.form.as_str() {
        "conifer" => Foliage {
            clump: h * 0.050,
            leaf: if carded {
                (h * 0.0060).clamp(0.10, 0.15)
            } else {
                (h * 0.022).clamp(0.30, 0.50)
            },
            wid: 0.40,
            per_sprig: 4,
            density,
            var,
        },
        "umbrella" => Foliage {
            clump: h * 0.080,
            leaf: (h * 0.016).clamp(0.10, 0.20),
            wid: 0.58,
            per_sprig: if carded { 4 } else { 14 },
            density,
            var,
        },
        // A palm builds its foliage inside `pinnate_frond` rather than through
        // `Foliage`; these are its leaflet facts, for the sprite path only.
        "palm" => Foliage {
            clump: h * 0.34 * 0.5,
            leaf: h * 0.34 * 0.20,
            wid: 0.16,
            per_sprig: 6,
            density,
            var,
        },
        _ => Foliage {
            clump: h * 0.092,
            leaf: (h * 0.011).clamp(0.09, 0.22),
            wid: 0.70,
            per_sprig: 3,
            density,
            var,
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
    // PER-FLOWER COLOUR (v0.1109). A Yoshino cherry opens deep pink in bud and
    // fades to near-white over the few days it is open, and a tree in bloom
    // carries every stage at once - which is why a photograph of one reads as
    // a pink CLOUD with white lights in it rather than as a flat pink surface.
    // The default spread's two-population split IS that bud/open population,
    // and the senescent-straw term self-gates off a petal because a petal has
    // no chlorophyll to lose (see `leaf_colour`'s CHLOROPHYLL_HUE_DEG).
    let color = leaf_colour::jitter(color, LeafVariation::default(), leaf_colour::key_at(at, 0x50));
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
/// MEMOIZED (v0.1090). This builds every variant of the species - three whole
/// trees, each of which is now a two-pass density fit with a card plan on each
/// pass - and it is called from `cluster_sprite_geometry`, which the baker
/// calls once per layer. Uncached that is 6 full tree builds per species per
/// bake, and the bake itself runs more than once per session (the atlas bake
/// and the near-mesh block each ask for the sprites). The result is a pure
/// function of the registry row, so cache it on (species id, layer): the
/// BUG-059 lesson is that anything expensive reached from a per-frame block is
/// memoized at its OWN call site rather than assumed cheap upstream.
pub fn mean_card_side(def: &TreeDef, layer: ClusterLayer) -> f32 {
    static CACHE: std::sync::OnceLock<
        std::sync::RwLock<std::collections::HashMap<(String, &'static str), f32>>,
    > = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(|| std::sync::RwLock::new(std::collections::HashMap::new()));
    let key = (def.id.clone(), layer.key());
    if let Some(&v) = cache.read().ok().and_then(|c| c.get(&key).copied()).as_ref() {
        return v;
    }
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
    let out = if n == 0 { nominal } else { sum / n as f32 };
    if let Ok(mut c) = cache.write() {
        c.insert(key, out);
    }
    out
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

// ── Welded junctions: collar, not burial (v0.1098) ───────────────────────
//
// `PlantMeshBuilder::tube` emits a side wall and NO end caps, so every limb is
// an open pipe. v0.1086 plugged that by starting the child's root ring INSIDE
// the parent's solid - push it back along the child's own axis by 1.4 parent
// radii and the parent's wall hides the child's open end.
//
// THAT IS THE BUG THE OPERATOR KEPT SEEING. Pushing back along the CHILD'S
// axis only stays inside the parent while the two are near-parallel. At a real
// fork angle the push has a large component ACROSS the parent, so the buried
// ring lands off-axis on the FAR side: for a child leaving its parent's tip at
// 42 deg with the radius continuity the generator uses (child r0 = parent tip
// radius R), the ring centre lands 1.4*R*sin(42) = 0.94 R off-axis and its far
// vertices another R*cos(42) = 0.74 R beyond that - 1.68 R from the axis
// through a wall that is only R thick. The branch pokes out the back of the
// trunk by two thirds of a radius, on EVERY fork in the tree. v0.1096 noticed
// this for the six laterals shed off the leader and pre-compensated their start
// point so the burial landed back on the stem AXIS; every other junction in
// every species (all recursive `limb` children, the acacia's primaries and
// fans) kept the raw backward push, and even the compensated ones still ran the
// child's first ring through the middle of the parent's wood.
//
// THE FIX IS THE REAL TECHNIQUE, not a bigger fudge. A branch does not start
// inside its parent; it starts ON its parent, and the junction is closed by a
// COLLAR - a swelling of parent tissue skinned from the parent's surface up to
// the branch's base. So:
//   1. the child's spine starts at the point where it leaves the parent's
//      surface (`surface_root`), with NO interior run at all;
//   2. `TreeParts::weld_child` skins a collar from the child's first ring to
//      that ring projected back onto the parent's surface, which is what makes
//      the join read as welded from the front; and
//   3. a fork's parent gets a real end cap (`bark_cap`), because the hole the
//      burial used to plug is a hole that should simply be closed.
// Cost is 2*sides triangles per junction plus sides per fork; correctness is
// now geometric rather than statistical, and `no_branch_pokes_out_the_back_of_
// its_parent` proves it over every species and variant.

/// How many of a child's leading rings the back-poke gate checks.
///
/// Raised 3 -> 8 in v0.1099, and the reason is coverage rather than appetite:
/// a limb's first rings are now packed into its base flare (`FLARE_RINGS`), so
/// keeping the count at 3 would have shrunk the gate's reach from half the limb
/// to about three shaft radii. Eight keeps every flare ring AND the first
/// uniform spine stations under the gate, which is strictly more geometry
/// checked than v0.1098 examined.
const FORK_GATE_RINGS: usize = 8;

/// The parent surface a child limb leaves from.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Junction {
    /// Point on the PARENT'S SPINE the child leaves from.
    pub axis: [f32; 3],
    /// The parent's spine direction there, unit.
    pub up: [f32; 3],
    /// The parent's radius there.
    pub r: f32,
    /// `true` when the parent's drawn tube ENDS at `axis` (a fork), `false`
    /// when the parent continues past (a lateral off the flank of a stem).
    /// A tip junction has no far wall to poke through, so its child roots on
    /// the end plane; a side junction must clear the flank.
    pub tip: bool,
}

impl Junction {
    /// The direction a child leaving along `dir` heads AWAY from the parent's
    /// axis in. Unit, perpendicular to `up`. Falls back to a stable
    /// perpendicular when the child is parallel to its parent.
    fn outward(&self, dir: [f32; 3]) -> [f32; 3] {
        let perp = sub(dir, mul(self.up, dot(dir, self.up)));
        if length(perp) > 1e-4 {
            norm(perp)
        } else {
            ring_basis(self.up).0
        }
    }

    /// `v` pushed onto the parent's surface: the cylinder wall at `v`'s own
    /// height for a side junction, and for a fork the terminal RIM, because a
    /// fork's parent has no wall past its end plane.
    fn project(&self, v: [f32; 3]) -> [f32; 3] {
        let rel = sub(v, self.axis);
        let along = dot(rel, self.up);
        // The radial direction comes from the TRUE perpendicular component,
        // never from the clamped one: clamping first leaves the axial part in
        // the vector being normalised, and the foot then lands short of the
        // surface by a factor of sin - which is exactly how far off the collar
        // feet sat on the first cut of this (2.4 mm on a 48 mm sakura limb).
        let radial = sub(rel, mul(self.up, along));
        let m = if length(radial) > 1e-5 { norm(radial) } else { ring_basis(self.up).0 };
        // A fork's parent has no wall past its end plane, so its feet land on
        // the terminal RIM; a side junction's parent runs on in both directions.
        let h = if self.tip { along.min(0.0) } else { along };
        add(add(self.axis, self.up, h), m, self.r)
    }

    /// Outward normal of the parent's surface at a projected foot.
    fn surface_normal(&self, foot: [f32; 3]) -> [f32; 3] {
        let rel = sub(foot, self.axis);
        let radial = sub(rel, mul(self.up, dot(rel, self.up)));
        if length(radial) > 1e-5 {
            norm(radial)
        } else {
            self.up
        }
    }
}

/// The limb a junction is about to grow: everything both the weld and the tube
/// need, so the collar and the first ring can never be sized differently.
#[derive(Clone, Copy, Debug)]
pub(crate) struct LimbShape {
    pub dir: [f32; 3],
    pub len: f32,
    /// SHAFT radius - what the limb settles at past its base flare.
    pub r0: f32,
    /// Tip radius.
    pub r1: f32,
    pub sides: u32,
    /// Base flare, already capped against the parent and already oriented
    /// against it (`Flare::new`). Carried on the shape rather than recomputed
    /// per call site so the collar, the tube, the cap and the gates cannot
    /// possibly disagree about the shape of the weld.
    pub flare: Flare,
}

impl LimbShape {
    /// The limb `j` is about to grow, with its base flare resolved against the
    /// parent it leaves.
    fn new(j: Junction, dir: [f32; 3], len: f32, r0: f32, r1: f32, sides: u32) -> Self {
        LimbShape { dir, len, r0, r1, sides: sides.max(3), flare: Flare::new(j, dir, r0) }
    }

    /// This limb's PLAIN TAPER radius `x` metres along its own spine.
    fn base_radius_at(&self, x: f32) -> f32 {
        limb_base_radius_at(x, self.len, self.r0, self.r1)
    }

    /// This limb's flare at station `x`, ready to hand to a ring.
    fn at(&self, x: f32) -> Option<FlareAt> {
        Some(FlareAt { flare: self.flare, x })
    }

    /// The DRAWN radius at station `x` for a vertex whose outward radial unit
    /// is `m`.
    fn radius_at_dir(&self, m: [f32; 3], x: f32) -> f32 {
        self.base_radius_at(x) * self.flare.mul(m, x)
    }

    /// The widest radius drawn anywhere on the weld ring (the crotch). What
    /// has to clear the parent's wood, and what the limb's bark repeat count
    /// is derived from.
    fn weld_r_max(&self) -> f32 {
        self.base_radius_at(0.0) * self.flare.mul_max(0.0)
    }
}

/// One recorded junction: what the child was welded to, the limb it grew, and
/// where its leading rings ended up. Read by the back-poke and flare gates.
pub(crate) struct Fork {
    pub parent: Junction,
    pub start: [f32; 3],
    pub shape: LimbShape,
    /// The CROTCH radius of the weld ellipse - the widest point of the limb
    /// where it welds on (`LimbShape::weld_r_max`). Was a single radius back
    /// when the weld was a circle; since v0.1100 the ring is directional, so
    /// this is one number describing a shape and NOT the collar's outer edge
    /// (that is the full ellipse, `ring_at`). Its only consumer is the
    /// `dump_fork_png` dev-aid camera framing.
    pub weld_r: f32,
    pub rings: Vec<ForkRing>,
}

/// One ring of drawn limb: enough to reconstruct its vertices exactly, because
/// `ring_basis`/`ring_point`/`ring_at` are shared with the tube that drew it.
#[derive(Clone, Copy)]
pub(crate) struct ForkRing {
    pub centre: [f32; 3],
    pub ax: [f32; 3],
    /// PLAIN taper radius. The drawn radius varies by azimuth (v0.1100), so
    /// this is the circle the flare multiplies, never a vertex distance.
    pub r: f32,
    pub sides: u32,
    /// The limb's flare at this ring's own profile station, so `vertices()`
    /// reproduces the ELLIPSE that shipped rather than a circle - without
    /// which the back-poke gate would check geometry nobody drew.
    pub flare: Option<FlareAt>,
}

impl ForkRing {
    /// The vertices `bark_tube` put on this ring, in order.
    pub fn vertices(&self) -> Vec<[f32; 3]> {
        let (side, up) = ring_basis(self.ax);
        let n = self.sides.max(3);
        (0..n)
            .map(|i| {
                let a = (i as f32) / (n as f32) * std::f32::consts::TAU;
                let (r, _) = ring_at(self.flare, self.r, side, up, a);
                ring_point(self.centre, side, up, a, r)
            })
            .collect()
    }

    /// The drawn radius at weight `w` of the peak: the profile of ONE angular
    /// sector of this ring. The flare gate walks a limb's rings at fixed `w`,
    /// which is a per-direction monotonicity check that is immune to the ring
    /// frame rotating as the limb bows.
    pub fn radius_at_weight(&self, w: f32) -> f32 {
        match self.flare {
            None => self.r,
            Some(f) => {
                self.r * (1.0 + f.flare.gain * w * (-f.x / f.flare.decay_m).exp())
            }
        }
    }
}

/// Where a child limb's spine starts: ON the parent's surface.
///
/// SIDE junction - step out from the parent's axis along the branch's own
/// outward direction by `r + r0 * cos(angle)`. The `r0 * cos` term is what
/// makes the guarantee total rather than typical: it puts the ring's most
/// rearward vertex EXACTLY on the parent's surface, so every vertex of the
/// first ring sits at radius >= r with a strictly positive outward component,
/// for any fork angle and any radius ratio. Nothing is inside the wood and
/// nothing is behind the trunk.
///
/// TIP junction (a fork) - the parent stops here, so there is no far wall to
/// clear, only the end plane. A ring of radius `r0` whose normal sits `angle`
/// off the parent's axis reaches `r0 * sin` back along that axis, so `r0 * tan`
/// of travel puts the whole ring in front of the plane. The collar then spans
/// from the parent's terminal rim out to it.
///
/// A tip child can also leave SIDEWAYS or BACKWARDS across its parent (the
/// acacia's fans open to 94 deg from vertical off a primary that rose at 58-74
/// deg, so the relative angle reaches ~170 deg and the fan runs back across the
/// limb it grew from). Those have to clear the FLANK like a side branch or the
/// half of the ring behind the end plane is buried again, so the two clearances
/// are taken together whenever the child is not leaving forward.
fn surface_root(j: Junction, dir: [f32; 3], r0: f32) -> [f32; 3] {
    let d = norm(dir);
    let cos = dot(d, j.up).clamp(-1.0, 1.0);
    let sin = (1.0 - cos * cos).max(0.0).sqrt();
    if j.tip {
        // The 0.6 floor caps the step at 1.67 root radii: past ~53 deg the ring
        // is BESIDE the end plane rather than behind it, so chasing the exact
        // tangent would buy nothing and would fling the branch base off its
        // parent.
        let front = r0 * sin / cos.max(0.6);
        let flank = (j.r + r0 * cos.abs()) / sin.max(0.2);
        add(j.axis, d, if cos > 0.35 { front } else { front.max(flank) })
    } else {
        add(j.axis, j.outward(d), j.r + r0 * cos.abs())
    }
}

// The radius and taper LAWS - what a stem, a limb, a fork and a shoot measure
// in metres - live in `tree_allometry.rs` since v0.1103. This file is the
// geometry kernel that draws them.

// ── THE BRANCH BASE FLARE (v0.1099) ──────────────────────────────────────
//
// THE FIELD REPORT. "Skinny at the connection point and then gets very wide
// shortly after and then tapers down." That is not poly count, it is the
// RADIUS PROFILE, and through v0.1098 it was a straight lerp: `r0 + (r1 - r0)
// * x/len`, monotonic from `r0` at the root ring. So a limb was already at its
// MAXIMUM radius the instant it left the parent's bark, and the only thing
// bridging the parent's surface to that full-width ring was the collar skirt -
// a few centimetres of skirt read as a pinch, then the branch snapped to full
// width. Read off the v0.1098 fork renders: the acacia's bole is 0.40 m across,
// each of its five primaries is ALSO 0.40 m across from its very first ring,
// and the only join geometry is a 2 cm wedge.
//
// WHAT A REAL BRANCH DOES IS THE OPPOSITE. The junction is the WIDEST part of
// the branch. A branch collar is a swelling of parent tissue that wraps the
// base, and the branch flares smoothly out of it and then tapers monotonically
// for the rest of its length. Every tree generator in the industry models this
// the same way - a profile curve with a base flare multiplying the taper - and
// this file already contains the technique, applied to the ONE junction that
// had it: `trunk` flares the stem where it meets the ground (v0.1067, "the
// detail that reads as 'a tree grew here'"). This generalises it to every
// junction in the tree.
//
// THE LAW: `radius(x) = base_taper(x) * (1 + FLARE_GAIN * exp(-x / (FLARE_DECAY
// * r0)))`, with x measured from the root ring. It is monotone non-increasing
// for any `r1 <= r0` by construction (both a non-positive slope and a strictly
// decreasing positive factor), which is the property the CI gate asserts.
//
// THE OTHER HALF, and the reason a flare alone would have made the report
// WORSE: a fork's children were sized at exactly their parent's tip radius, so
// two children already carried 2x the parent's cross-sectional AREA and the
// acacia's five carried 5x. Flaring on top of that would have put 1.45x the
// parent's radius at every weld - a bulge. Wood is conserved instead
// (`fork_child_radius`), which thins the SHAFT by n^(-1/2.3) and lets the flare
// bring the WELD back to ~1.0x the parent. Net at the junction: the silhouette
// is what it was; net a few radii out: the branch is honestly thinner. That is
// exactly the defect the operator described, removed from both ends.
//
// ── THE FLARE IS DIRECTIONAL (v0.1100) ───────────────────────────────────
//
// THE FIELD REPORT ON v0.1099: branch bases "still way too bulky". Correct,
// and the reason is that a scalar `radius(x)` can only make a SLEEVE - the
// same 45% of extra wood on every azimuth, all the way round, including the
// side where a real branch is nearly flush with its parent. A sleeve at 1.45x
// carries 2.1x the shaft's cross-sectional AREA at the weld; the operator was
// looking at the extra 110%.
//
// WHAT A REAL JUNCTION DOES. A branch attachment is ELLIPTICAL, not circular,
// and its long axis lies in the plane of the parent's axis. Material stacks in
// two places: the CROTCH (the acute angle between child and parent, where the
// branch bark ridge forms as the two cambiums press together) and the
// UNDERSIDE (the collar proper, buttressed by reaction wood carrying the
// branch's own weight). The FLANKS - the two azimuths at right angles to that
// plane - are close to flush; there is nothing structural for them to do.
//
// So the flare stops being a radius multiplier and becomes a PER-VERTEX one:
// `radius(m, x) = base_taper(x) * (1 + GAIN * w(m) * exp(-x / decay))`, with
// `w` peaking at 1 in the crotch, ~0.88 under the branch, and floored at
// `FLARE_FLUSH_W` on the flanks. The peak also comes DOWN, 0.45 -> 0.25.
// Net at the weld: 1.25x on the strong side, 1.045x flush, and the weld's
// cross-sectional area falls from 2.10x the shaft to 1.31x - a 38% cut in
// exactly the bulk that was reported, while the join gets MORE filled where a
// junction is supposed to be filled.
//
// The crotch direction is also the direction that sits deepest toward the
// parent (`surface_root` places the root ring by it), so the widest part of
// the ellipse is the part that touches the parent's bark - which is what makes
// the crotch read as filled rather than as a gap bridged by a skirt.

/// PEAK flare, on the strong side of the ellipse (crotch and underside).
///
/// 0.25 puts that side at 1.25x the shaft radius, down from the v0.1099 sleeve
/// at 1.45x. Paired with the pipe-model split at a two-way fork (0.74x), the
/// weld's widest point lands at 0.93x the parent's tip: wood is continuous
/// across the join and never proud of it, which is what real bark does.
const FLARE_GAIN: f32 = 0.25;

/// Flare weight on the FLANKS - the azimuths at right angles to the plane of
/// the parent's axis, where a real branch is nearly flush with its parent.
///
/// 0.18 of the peak, so those vertices sit at 1.045x the shaft radius: enough
/// that the collar skirt still meets a slightly proud ring rather than a hard
/// cylinder, far too little to read as a sleeve. Never zero, because the flare
/// must stay a strictly positive decreasing factor for the profile to be
/// provably monotone in EVERY direction (see the flare gate).
const FLARE_FLUSH_W: f32 = 0.18;

/// Flare weight at the far end of the parent-axis plane - the OBTUSE side,
/// diametrically opposite the crotch.
///
/// 0.85 of the crotch, which is what makes the base an ellipse rather than a
/// half-moon: the collar swelling below a branch is comparable to the ridge
/// above it, just smoother.
const FLARE_FAR_W: f32 = 0.85;

/// Flare weight on the gravity-DOWN side, wherever that differs from the
/// parent-axis plane (any limb off a limb, which is most of the tree).
///
/// The compression/reaction wood a branch lays down to carry its own weight is
/// on its underside, so this lobe rides on top of the axial ellipse rather
/// than replacing it. Same 0.85 as the obtuse side because on a VERTICAL
/// parent the two coincide exactly, and a discontinuity between the two cases
/// would show as a step in the collar as a stem leans.
const FLARE_UNDER_W: f32 = 0.85;

/// Sharpness of each flare lobe: `max(0, cos)^p`.
///
/// 2.0 would be a true ellipse (a small-eccentricity ellipse's radius is
/// `b + (a - b) cos^2`); 1.6 is deliberately a little broader, because the
/// generator draws 4-8 sided rings (`sides_for`) and a lobe narrower than the
/// angular step between vertices aliases into "one fat vertex" - a lump rather
/// than a collar. Measured across the quarter turn from crotch to flank, 1.6
/// runs 1.25x, 1.21x, 1.16x, 1.11x, 1.07x, 1.045x of the shaft radius: an
/// ellipse you can see, with no vertex-scale spike.
///
/// This constant is what separates "directional collar" from the v0.1099
/// SLEEVE it replaced, and it is gated by
/// `flare_lobe_stays_an_ellipse_not_a_sleeve` — which exists because an
/// adversarial review found it was the ONE flare parameter nothing could
/// react to: `strong` and `flush` are both independent of p, and the mean
/// ratio only crosses its 1.20 ceiling below p = 0.6, so p = 0.8 (a profile
/// still above 1.17x at 60 degrees off the crotch — most of a sleeve) would
/// have shipped green. An earlier version of this comment claimed the flare
/// gate caught exactly that; it could not. Now something does.
const FLARE_LOBE_P: f32 = 1.6;

/// e-folding distance of the flare, in SHAFT RADII.
///
/// Forced, not chosen: the flare has to be within 2% of the plain taper by 3
/// shaft radii (past that a branch is just a branch), and `GAIN * exp(-3/DECAY)
/// < 0.02` needs DECAY < 1.24 at GAIN 0.25. At 0.9 the strong side runs 25% at
/// the weld, 8.2% at one radius, 2.7% at two and 0.9% at three. Anatomically
/// that is right too: a branch collar's swelling extends about one branch
/// DIAMETER.
const FLARE_DECAY: f32 = 0.9;

/// How far out the flare is resolved with extra rings, in shaft radii.
const FLARE_SPAN: f32 = 3.0;

/// Extra rings packed into the flare run, front-loaded (see `ring_stations`).
///
/// FOUR, because that is where the error stops mattering. The stations land at
/// 0.19, 0.75, 1.69 and 3.0 shaft radii, and a straight frustum between
/// consecutive stations tracks the true exponential to better than 1% of the
/// shaft radius anywhere in the run - well under one pixel at any distance the
/// junction is a junction rather than a smudge. More rings would be measurably
/// unnecessary; fewer (the uniform spacing this replaces, which puts the next
/// ring 10-40 shaft radii out) cannot resolve the flare AT ALL, which is the
/// honest half of the operator's "is it poly count" question: the profile was
/// wrong, and the sampling could not have drawn it even if it had been right.
const FLARE_RINGS: u32 = 4;

/// Junctions worth spending extra rings on, as a fraction of TREE HEIGHT.
///
/// Self-scaling instead of an absolute metre threshold: it is "the top quarter
/// of the tree's own radius range" - the structural forks you can walk up to.
/// Below it the flare still applies to the profile (it costs nothing to make
/// the first ring wider), it is simply drawn as one straight ramp rather than
/// a resolved curve, because a 2 cm twig's collar is a sub-pixel feature and
/// the crown needs those triangles for leaves.
///
/// 0.0075 -> 0.0036 in v0.1103, and the constant did not change MEANING, only
/// units: it was pinned to the old flat `r_base = height * 0.030`, and a real
/// broadleaf stem (`stem_base_radius`) is 0.0143 of its height. Same quarter,
/// same junctions resolved, same triangle cost - measured, the resolved count
/// per species is unchanged. Leaving it at 0.0075 would have resolved NOTHING
/// on any species (a sakura's biggest limb shaft is 0.053 m against a 0.060 m
/// threshold), silently dropping every collar to one strip and taking gate 6
/// of the flare gate with it.
const FLARE_RING_MIN_H_FRAC: f32 = 0.0036;

/// The flare a limb of shaft radius `r0` actually gets off a parent of radius
/// `parent_r`: THE WELD IS NEVER WIDER THAN THE PARENT IT LEAVES.
///
/// Found by the back-poke gate rather than by taste. A broadleaf's terminal
/// limb is not a fork at all - it is the leader CONTINUING, sized at 0.92 of
/// the leader's tip - and giving a continuation the full collar swelling (45%
/// when this was written, 25% now) put its base 33% proud of the stem's own
/// rim: a mushroom lip on top of every tree, and 24 ring vertices overhanging
/// the back of the parent (sakura seed 0, 20.7 mm). Real wood does not do
/// that, and the rule that stops it is the one nature uses: the surface is
/// continuous across the join, so a limb's weld swells up to - and never past
/// - the wood feeding it. A genuine two-way fork (child 0.74 of parent) lands
/// at 0.93 of the parent's radius; a small lateral off a big stem gets the
/// full swelling; a continuation gets almost none.
fn flare_gain_at(r0: f32, parent_r: f32) -> f32 {
    let cap = parent_r.max(r0) / r0.max(1e-5) - 1.0;
    FLARE_GAIN.min(cap.max(0.0))
}

/// ONE LIMB'S DIRECTIONAL BASE FLARE: the ellipse its junction is drawn as.
///
/// Everything is resolved once, at `LimbShape::new`, and then read per vertex,
/// so the collar skin, the tube, the end cap and the CI gates cannot possibly
/// disagree about the shape of a junction. See the block comment above
/// `FLARE_GAIN` for the anatomy this encodes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Flare {
    /// The PARENT'S AXIS projected into this limb's ring plane, unit. The long
    /// axis of the junction ellipse.
    pub axial: [f32; 3],
    /// Which end of `axial` the CROTCH is on: +1 when the limb leaves along
    /// its parent (the acute angle is on the `+axial` side), -1 when it droops
    /// back against it.
    pub acute: f32,
    /// Gravity-down, projected into the ring plane, unit: the underside, where
    /// a branch lays down the reaction wood that carries its own weight. Falls
    /// back to `axial` for a vertical limb, which has no underside.
    pub down: [f32; 3],
    /// Peak gain, already capped against the parent (`flare_gain_at`).
    pub gain: f32,
    /// e-folding distance in METRES (`FLARE_DECAY` shaft radii).
    pub decay_m: f32,
}

impl Flare {
    /// The flare a limb of shaft radius `r0` leaving `j` along `dir` gets.
    fn new(j: Junction, dir: [f32; 3], r0: f32) -> Flare {
        let d = norm(dir);
        // The parent's axis, projected into the child's ring plane. Degenerate
        // only for a CONTINUATION (child parallel to parent), which by
        // `flare_gain_at` has almost no flare to place anyway.
        let axial_raw = sub(j.up, mul(d, dot(j.up, d)));
        let axial =
            if length(axial_raw) > 1e-4 { norm(axial_raw) } else { ring_basis(d).0 };
        let down_raw = sub([0.0, -1.0, 0.0], mul(d, -d[1]));
        let down = if length(down_raw) > 1e-4 { norm(down_raw) } else { axial };
        Flare {
            axial,
            acute: if dot(d, j.up) >= 0.0 { 1.0 } else { -1.0 },
            down,
            gain: flare_gain_at(r0, j.r),
            decay_m: FLARE_DECAY * r0.max(1e-5),
        }
    }

    /// How much of the peak flare a ring vertex whose outward radial unit is
    /// `m` gets: 1 in the crotch, `FLARE_FAR_W` at the obtuse end of the same
    /// axis, `FLARE_UNDER_W` on the gravity-down side, `FLARE_FLUSH_W` on the
    /// flanks. Strictly positive everywhere, which is what keeps every
    /// direction's profile provably monotone.
    fn weight(&self, m: [f32; 3]) -> f32 {
        let t = dot(m, self.axial) * self.acute;
        let crotch = t.max(0.0).powf(FLARE_LOBE_P);
        let far = (-t).max(0.0).powf(FLARE_LOBE_P) * FLARE_FAR_W;
        let under = dot(m, self.down).max(0.0).powf(FLARE_LOBE_P) * FLARE_UNDER_W;
        FLARE_FLUSH_W + (1.0 - FLARE_FLUSH_W) * crotch.max(far).max(under)
    }

    /// Multiplier on the plain taper at a vertex `m`, `x` metres along the limb.
    fn mul(&self, m: [f32; 3], x: f32) -> f32 {
        1.0 + self.gain * self.weight(m) * (-x / self.decay_m).exp()
    }

    /// The largest multiplier anywhere on the ring at `x` - the crotch. Also
    /// the azimuth that sits deepest toward the parent, which is why
    /// `surface_root` can use this one number and still guarantee that NO
    /// vertex is behind the parent's far wall.
    fn mul_max(&self, x: f32) -> f32 {
        1.0 + self.gain * (-x / self.decay_m).exp()
    }
}

/// One limb's flare, evaluated at ONE station along it.
///
/// The station is carried rather than re-derived because a ring's radius comes
/// from its PROFILE station while its geometry may sit further along (the
/// joint overshoot) - see `BarkSeg`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FlareAt {
    pub flare: Flare,
    pub x: f32,
}

impl FlareAt {
    fn mul(&self, m: [f32; 3]) -> f32 {
        self.flare.mul(m, self.x)
    }
}

/// Radius AND azimuthal slope (`dr/da`) of one ring vertex: the plain circle
/// when a run has no directional flare, the flare's ellipse when it does.
///
/// THE one definition, called by the tube, the collar skin, the end cap, the
/// back-poke gate and the flare gate. The slope is central-differenced rather
/// than derived in closed form because `Flare::weight` takes a `max` of three
/// lobes: a numeric difference averages cleanly through the crossovers, where
/// the analytic derivative would jump.
fn ring_at(
    fa: Option<FlareAt>,
    base: f32,
    side: [f32; 3],
    up: [f32; 3],
    ang: f32,
) -> (f32, f32) {
    match fa {
        None => (base, 0.0),
        Some(f) => {
            const H: f32 = 0.03;
            let r = base * f.mul(ring_dir(side, up, ang));
            let d = base * (f.mul(ring_dir(side, up, ang + H)) - f.mul(ring_dir(side, up, ang - H)))
                / (2.0 * H);
            (r, d)
        }
    }
}

/// Where along a limb its rings go: the uniform spine stations, plus rings
/// packed into the flare run so the curve is actually drawn.
///
/// The extra stations are QUADRATICALLY front-loaded (`(k/n)^2` of the span),
/// which is the geometric-in-spirit spacing the flare wants: the profile falls
/// fastest in the first half radius, so that is where the rings go. Returns a
/// sorted list from 0 to `len`; stations closer together than a quarter of the
/// flare's own finest step are merged, because a sliver ring costs triangles
/// and buys no silhouette.
///
/// `min_r` is the shaft radius below which no extra rings are spent
/// (`FLARE_RING_MIN_H_FRAC`); pass 0 to always densify.
fn ring_stations(len: f32, r0: f32, segs: u32, min_r: f32) -> Vec<f32> {
    let segs = segs.max(1);
    let len = len.max(1e-4);
    let uniform = |i: u32| len * i as f32 / segs as f32;
    if r0 < min_r || r0 <= 0.0 {
        return (0..=segs).map(uniform).collect();
    }
    // Never spend more than the first half of a short limb on its own flare.
    let span = (FLARE_SPAN * r0).min(len * 0.5);
    let n = FLARE_RINGS.max(1);
    let mut xs: Vec<f32> = (0..=segs).map(uniform).collect();
    xs.extend((1..=n).map(|k| span * (k as f32 / n as f32).powi(2)));
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let gap = span / n as f32 * 0.25;
    let mut out: Vec<f32> = Vec::with_capacity(xs.len());
    for x in xs.into_iter().filter(|&x| x < len - gap) {
        if out.last().is_none_or(|&p| x - p > gap) {
            out.push(x);
        }
    }
    if out.first().is_none_or(|&f| f > 0.0) {
        out.insert(0, 0.0);
    }
    // The tip station is structural - the limb ends there and the next junction
    // welds onto it - so it is appended rather than merged.
    out.push(len);
    out
}

/// The RADIUS AND TAPER MODEL: how thick wood is, in metres. Its own file
/// since v0.1103 (see the module header there for the allometry and its
/// sources). A CHILD module for the same reason `tree_species` is - it needs
/// `TreeDef`, and `tree_species` needs it - and re-exported here privately so
/// both this file and `tree_species` name the laws the same way.
#[path = "tree_allometry.rs"]
mod tree_allometry;
use tree_allometry::*;

/// The crown builders, one per `form`: their own file since v0.1102 (see the
/// module header there). `#[path]` keeps them a CHILD module of this one, so
/// they reach the kernel's private geometry through `use super::*` without
/// widening any of it.
#[path = "tree_species.rs"]
mod tree_species;

/// The foliage-card arrangement (v0.1110). A CHILD module for the same
/// reason `tree_species` is - `emit_sleeve` needs the kernel's vector
/// helpers, `PlantMeshBuilder` and the normal-blend constants.
#[path = "tree_cards.rs"]
mod tree_cards;
use tree_cards::{balance_card_normals, emit_card, emit_sleeve, CLUSTER_SLEEVE_OFFSET};
use tree_species::{broadleaf, conifer, palm, umbrella};


#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn registry() -> TreeRegistry {
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

    /// The shipped registry is ALL-PROCEDURAL as of v0.1103, and the
    /// model-backed CODE PATH is still alive and gated separately.
    ///
    /// This used to assert that the shipped data contained at least one of
    /// each kind, which was true while fir and pine named photoscans. It is no
    /// longer: the v0.1101 scan-stretch guard rejects every one of those scans
    /// at every variant (they are ~1 m saplings standing in for 22 m and 16 m
    /// trees), so naming a model bought nothing and COST both species their
    /// baked bark and their canopy cards - `is_procedural()` is
    /// `model.is_empty()`, and the site that registers card meshes and bark
    /// filters on it.
    ///
    /// So the assertion moved deliberately rather than being deleted: the data
    /// claim is now "everything ships procedural", and the capability claim -
    /// that a model-backed def is still understood - is checked against a
    /// synthetic def so the path cannot rot silently while unused. If a
    /// correctly-scaled scan is ever added, the first assertion is the one to
    /// revisit, and it will fail loudly rather than pass vacuously.
    #[test]
    fn shipped_registry_is_all_procedural_and_the_model_path_still_works() {
        let r = registry();
        assert!(r.len() >= 6, "expected a real species list, got {}", r.len());
        assert!(r.index_of("sakura").is_some(), "sakura missing");
        assert!(r.trees.iter().any(|t| t.is_procedural()), "no procedural species");

        let model_backed: Vec<&str> = r
            .trees
            .iter()
            .filter(|t| !t.is_procedural())
            .map(|t| t.id.as_str())
            .collect();
        assert!(
            model_backed.is_empty(),
            "{model_backed:?} name a model. Every photoscan we have is rejected by the \
             scan-stretch guard, and naming one locks the species out of baked bark and \
             cluster cards (is_procedural() gates both). If you added a correctly-scaled \
             scan, update this test with the measurement that says so."
        );

        // The capability, independent of what the data happens to use.
        let mut def = r.trees[0].clone();
        def.model = "some_scan".to_string();
        assert!(!def.is_procedural(), "a def naming a model must read as model-backed");
        def.model = String::new();
        assert!(def.is_procedural(), "a def naming no model must read as procedural");
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
    pub(super) fn shipped_seed(variant: u32) -> u32 {
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
    pub(super) fn as_procedural(t: &TreeDef) -> TreeDef {
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

    /// Mean and longest drawn foliage element on one built tree, metres, plus
    /// the face count.
    fn drawn_element_len(t: &TreeDef, seed: u32) -> (f32, f32, usize) {
        const ORGAN_BIT_LEAF: u32 = 524_288;
        let mut b = PlantMeshBuilder::new();
        build_tree(&mut b, t, t.height_m, seed);
        let (mut sum, mut n, mut longest) = (0.0f64, 0usize, 0.0f32);
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
            longest = longest.max(l);
            n += 1;
        }
        (if n == 0 { 0.0 } else { (sum / n as f64) as f32 }, longest, n)
    }

    /// A conifer's drawn element is a SHOOT, and WHICH shoot depends on whether
    /// cards carry the canopy.
    ///
    /// Renamed from `conifer_needle_sprays_are_sub_half_metre` in v0.1102, and
    /// the rename is half the fix: the old name said "sub half metre", its
    /// failure message said "the 0.3-0.5 m target band", and its assert
    /// admitted 0.20-0.62 - three different claims, none of which mentioned
    /// that the per-blade jitter takes a 0.484 m spray to 0.605 m. All of it
    /// states the same numbers now, and BOTH arms of `foliage_of` are checked:
    ///
    ///   UNCARDED (fir and pine as they ship today): a 0.30-0.50 m NEEDLE
    ///   SPRAY. One 30 mm needle is below a pixel and there is no budget for
    ///   the ~200k a real fir carries, so the spray is the honest coarse unit -
    ///   and with no cards, that layer IS the canopy.
    ///
    ///   CARDED: a 0.10-0.15 m BRANCHLET, the real last-year shoot, because the
    ///   cards are the canopy then and ONE SPRIG MUST FIT ONE CARD (see the
    ///   measurement block above `foliage_of`).
    #[test]
    fn conifer_needle_sprays_are_shoot_scale_carded_or_not() {
        let r = registry();
        let lent = borrowed_cluster_def();
        let conifers: Vec<TreeDef> =
            r.trees.iter().filter(|t| t.form == "conifer").map(as_procedural).collect();
        assert!(!conifers.is_empty(), "no conifer rows to test");
        for base in &conifers {
            for carded in [false, true] {
                let mut t = base.clone();
                // BOTH ARMS ARE CONSTRUCTED, never assumed (v0.1107). Until fir
                // and pine gained their own `clusters:` blocks this loop only
                // ever ADDED one, so the `false` arm was whatever the row
                // happened to ship - and the moment the rows shipped a block
                // that arm silently became a second carded case and then failed
                // the 0.28-0.52 m band. A test that can only build one of the
                // two states it claims to cover is not testing the branch.
                if carded {
                    t.clusters = Some(lent.clone());
                } else {
                    t.clusters = None;
                }
                let (mean, longest, n) = drawn_element_len(&t, shipped_seed(0));
                // Authored 0.30-0.50 m x 1.25 jitter = 0.625 m; authored
                // 0.10-0.15 m x 1.25 = 0.1875 m. The bands below say exactly
                // that and nothing else.
                let (lo, hi, max) = if carded { (0.09, 0.16, 0.19) } else { (0.28, 0.52, 0.63) };
                let arm = if carded { "carded  " } else { "uncarded" };
                eprintln!(
                    "[leafscale] {:>7} conifer {arm}: {n} faces, mean {mean:.3} m, longest \
                     {longest:.3} m (band {lo:.2}-{hi:.2} m, ceiling {max:.2} m)",
                    t.id
                );
                assert!(n > 200, "{}: only {n} needle faces", t.id);
                assert!(
                    (lo..=hi).contains(&mean),
                    "{} ({arm}): mean drawn element {mean:.3} m is outside the {lo:.2}-{hi:.2} m \
                     band this arm of `foliage_of` authors",
                    t.id
                );
                assert!(
                    longest <= max,
                    "{} ({arm}): longest drawn element {longest:.3} m over the {max:.2} m \
                     ceiling (the authored maximum plus its 1.25x jitter)",
                    t.id
                );
            }
        }
    }

    /// EVERY procedural species must DRAW the element it AUTHORS, and no
    /// species may draw one the eye reads as a tarpaulin.
    ///
    /// Two failures, one gate, and neither was covered before v0.1102:
    ///
    ///   (a) DRAWN == AUTHORED. `foliage_of` is the single statement of what a
    ///   species' foliage element is, and it is also what the CLUSTER SPRITE is
    ///   baked from - so a tree that draws something else disagrees with its own
    ///   baked card and the near-to-far handoff pops. Measured against
    ///   `foliage_of` itself rather than a hand-written table, so the two cannot
    ///   drift apart.
    ///
    ///   (b) AN ABSOLUTE CEILING, argued from angular size rather than taste.
    ///   The near-tree path draws these from ~1 m away; at 2560 px across a
    ///   90 deg FOV that is ~1280 px per metre, so a 1 m element is a THOUSAND
    ///   PIXELS of one flat triangle. The largest honest element in the
    ///   registry is a 0.91 m coconut leaflet, which is why the ceiling sits
    ///   just above it.
    ///
    /// COVERAGE was the real hole: `broadleaf_leaves_are_drawn_at_real_scale`
    /// filters to `form == "broadleaf"` and the conifer gate to `"conifer"`, so
    /// umbrella and palm - half the forms, and one of them the species in the
    /// operator's capture - had no element-scale gate of any kind.
    #[test]
    fn every_species_draws_the_foliage_element_it_authored() {
        /// Metres; see (b) above.
        const CEILING_M: f32 = 1.00;
        let r = registry();
        let mut seen = 0usize;
        for t in r.trees.iter().map(as_procedural) {
            for v in 0..t.variants.max(1) {
                let (mean, longest, n) = drawn_element_len(&t, shipped_seed(v));
                let authored = foliage_of(&t, t.height_m, 1.0).leaf;
                assert!(n > 100, "{} v{v}: only {n} foliage faces", t.id);
                eprintln!(
                    "[element] {:>7} v{v} ({:>9}): authored {authored:.3} m, drawn mean \
                     {mean:.3} m ({:.2}x), longest {longest:.3} m ({:.2}x)",
                    t.id,
                    t.form,
                    mean / authored,
                    longest / authored
                );
                // 0.55 at the bottom because a palm frond tapers its leaflets
                // to 0.55 of the base length toward the tip; 1.15 at the top is
                // the 1.25x per-blade jitter's own mean plus slack.
                assert!(
                    (0.55..=1.15).contains(&(mean / authored)),
                    "{} v{v}: draws a {mean:.3} m element while `foliage_of` authors \
                     {authored:.3} m - the tree and the cluster sprite baked for it are built \
                     from different leaves",
                    t.id
                );
                assert!(
                    longest <= authored * 1.30,
                    "{} v{v}: longest drawn element {longest:.3} m against an authored \
                     {authored:.3} m (the 1.25x jitter allows {:.3} m)",
                    t.id,
                    authored * 1.25
                );
                assert!(
                    longest <= CEILING_M,
                    "{} v{v}: one drawn element reaches {longest:.2} m. The near-tree path draws \
                     these from ~1 m, where that is ~{:.0} px of a single flat triangle at 2560 \
                     wide - the deltoid-dart artifact",
                    t.id,
                    longest * 1280.0
                );
                seen += 1;
            }
        }
        assert!(seen >= 8, "only {seen} (species, variant) pairs measured");
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

    /// SURFACE WELD (v0.1098). Replaces `branch_roots_are_buried_inside_their_
    /// parent`, whose whole assertion - "the root ring must be displaced
    /// BACKWARDS along its own axis, INTO the parent" - is the defect. Pushing
    /// back along the CHILD'S axis only stays inside the parent while the two
    /// are near-parallel; at a real fork angle it lands the ring out through
    /// the far wall. The property that replaces it is the one that was actually
    /// wanted all along: the root ring sits ON the parent's surface, with every
    /// vertex outside the wood and on the branch's side of the axis.
    #[test]
    fn branch_roots_sit_on_the_parent_surface() {
        // SIDE junctions, across the whole plausible span of fork angles and
        // radius ratios - including a child FATTER than its parent, which the
        // acacia genuinely builds.
        for &parent_r in &[0.02f32, 0.1, 0.4] {
            for &ratio in &[0.05f32, 0.5, 0.95, 1.2] {
                for &deg in &[8.0f32, 25.0, 45.0, 60.0, 80.0, 95.0] {
                    let r0 = parent_r * ratio;
                    let up = norm([0.1, 1.0, -0.05]);
                    let j = Junction { axis: [1.0, 5.0, -2.0], up, r: parent_r, tip: false };
                    let d = tilt(up, deg, 0.7);
                    let start = surface_root(j, d, r0);
                    let out = j.outward(d);
                    let (side, upr) = ring_basis(d);
                    for i in 0..12 {
                        let a = i as f32 / 12.0 * std::f32::consts::TAU;
                        let v = ring_point(start, side, upr, a, r0);
                        let c = dot(sub(v, j.axis), out);
                        assert!(
                            c >= parent_r - 1e-4,
                            "r={parent_r} ratio={ratio} deg={deg}: a root-ring vertex sits \
                             {c} m along the branch direction, inside the parent's {parent_r} m \
                             of wood"
                        );
                    }
                }
            }
        }
        // TIP junctions: the child clears the parent's END PLANE, so no part of
        // it is inside the parent's solid either.
        for &deg in &[5.0f32, 20.0, 40.0] {
            let up = [0.0, 1.0, 0.0];
            let j = Junction { axis: [0.0, 3.0, 0.0], up, r: 0.2, tip: true };
            let d = tilt(up, deg, 0.3);
            let start = surface_root(j, d, 0.2);
            let (side, upr) = ring_basis(d);
            for i in 0..12 {
                let a = i as f32 / 12.0 * std::f32::consts::TAU;
                let v = ring_point(start, side, upr, a, 0.2);
                let h = dot(sub(v, j.axis), up);
                assert!(h >= -1e-4, "deg={deg}: a root-ring vertex sits {h} m behind the fork");
            }
        }
    }

    /// THE BACK-POKE GATE (v0.1098). The operator's field report, as geometry:
    /// a branch must never emerge through the FAR side of the limb it grew
    /// from.
    ///
    /// Stated so it cannot be satisfied by luck: project every vertex of every
    /// limb's first `FORK_GATE_RINGS` rings onto its parent's local
    /// cross-section plane. A vertex whose radial distance from the parent's
    /// axis exceeds the parent's local radius is OUTSIDE the parent's wood, and
    /// a vertex outside the wood is only allowed on the branch's own side -
    /// `dot(v - axis, outward) > 0`. Anything else is wood coming out of the
    /// back of the trunk.
    ///
    /// Every species, every seed variant, every junction the generator makes.
    #[test]
    fn no_branch_pokes_out_the_back_of_its_parent() {
        let r = registry();
        let mut checked = 0usize;
        let mut violations = 0usize;
        let mut worst = 0.0f32;
        let mut worst_at = String::new();
        for t in r.trees.iter().map(as_procedural) {
            for seed in 0..6u32 {
                let (parts, _) = build_accepted(&t, t.height_m, shipped_seed(seed));
                let mut per_tree = 0usize;
                for f in &parts.forks {
                    let j = f.parent;
                    let out = j.outward(f.shape.dir);
                    for (ri, ring) in f.rings.iter().enumerate() {
                        for v in ring.vertices() {
                            let rel = sub(v, j.axis);
                            let radial = sub(rel, mul(j.up, dot(rel, j.up)));
                            let side = dot(rel, out);
                            checked += 1;
                            // Inside the parent's wood: hidden, and allowed.
                            // The 1e-4 slack is float noise on metre-scale
                            // coordinates, not a tolerance for real overhang.
                            if length(radial) <= j.r + 1e-4 || side > 0.0 {
                                continue;
                            }
                            per_tree += 1;
                            violations += 1;
                            let over = length(radial) - j.r;
                            if over > worst {
                                worst = over;
                                worst_at = format!(
                                    "{} seed {seed}, ring {ri} of {} (parent r {:.3} tip={}, \
                                     limb shaft r {:.3} len {:.2} m)",
                                    t.id,
                                    f.rings.len(),
                                    j.r,
                                    j.tip,
                                    f.shape.r0,
                                    f.shape.len
                                );
                            }
                        }
                    }
                }
                assert_eq!(
                    per_tree, 0,
                    "{} seed {seed}: {per_tree} branch vertices emerge through the FAR side of \
                     their parent (worst overhang {worst} m at {worst_at})",
                    t.id
                );
            }
        }
        assert!(checked > 20_000, "only {checked} ring vertices reached the gate");
        eprintln!(
            "[back-poke] {checked} leading-ring vertices across {} species x 6 variants: \
             {violations} on the far side of their parent",
            r.trees.len()
        );
    }

    /// THE FLARE GATE (v0.1099, PER-DIRECTION in v0.1100). The operator's field
    /// reports as measurements: "skinny at the connection point and then gets
    /// very wide shortly after" is a radius profile that RISES after the
    /// junction, and "branch bases still way too bulky" is a profile that rises
    /// too far, in every direction at once.
    ///
    /// Six properties, over every species x variant x junction. The monotone
    /// half is now stated HONESTLY for an elliptical junction: every angular
    /// sector has its own profile, and each one has to be non-increasing after
    /// its own peak - a single scalar profile would no longer describe the
    /// geometry that ships.
    ///
    ///   1. MONOTONE PER SECTOR. For each of 12 azimuths round the weld ring,
    ///      the drawn radius is non-increasing from the weld to the tip.
    ///      Checked on the law at 200 stations AND on the rings that were
    ///      actually drawn (at four flare weights, which is a per-sector check
    ///      immune to the ring frame rotating as the limb bows), because a
    ///      correct law drawn wrong is still a wrong tree.
    ///   2. FLARED WHERE IT SHOULD BE. The crotch is 1.2-1.3x the shaft radius
    ///      wherever the parent has room for it, never more than 1.3x, and
    ///      never wider than the parent itself (`flare_gain_at` - a
    ///      continuation like the leader's terminal limb is 0.92 of its parent
    ///      and can only swell 8%).
    ///   3. FLUSH WHERE IT SHOULD BE. The flanks are at most 1.10x the shaft:
    ///      the junction must be an ELLIPSE, not the sleeve v0.1099 drew. This
    ///      is the assertion the operator's second report turns into CI.
    ///   4. ANISOTROPIC. The strong side is at least 1.12x the flush side
    ///      wherever there is room to swell, so "directional" cannot quietly
    ///      decay back into "radial" through a bad constant.
    ///   5. DECAYED. By three shaft radii out the flare is within 2% of the
    ///      plain taper in EVERY direction, so it is a base swelling and not a
    ///      fat branch.
    ///   6. RESOLVED. A junction thick enough to earn extra rings has at least
    ///      four of them inside the flare run - the honest half of "is it poly
    ///      count": the profile is only as real as its sampling.
    /// THE LOBE IS AN ELLIPSE, NOT A SLEEVE: `FLARE_LOBE_P` has to fall off
    /// fast enough that the swelling is a collar you can point at.
    ///
    /// This gate exists because an adversarial review of the v0.1100 flare
    /// found `FLARE_LOBE_P` was the one parameter of the shape that NOTHING
    /// could react to. `strong` (= 1 + gain) and `flush` (= 1 + gain *
    /// FLARE_FLUSH_W) are both algebraically independent of it, and the mean
    /// ratio only crosses its 1.20 ceiling below p = 0.6 - so p = 0.8, which
    /// holds above 1.17x a full 60 degrees off the crotch and reads as the
    /// v0.1099 sleeve, passed all six properties green. A gate that cannot
    /// fail for the defect it names is worse than no gate: it ends the
    /// investigation.
    ///
    /// Measured through the real `Flare::weight`, not against the constant,
    /// so a refactor that broadens the lobe some other way trips it too. The
    /// gravity lobe is put perpendicular to the sampled plane, isolating one
    /// lobe's own profile: `(weight(60deg) - FLUSH) / (1 - FLUSH)` is exactly
    /// `cos(60deg)^p = 0.5^p`. Upper bound 0.35 rejects p < 1.5 (sleeve);
    /// lower bound 0.20 rejects p > 2.32, where the lobe gets narrower than
    /// the angular step between ring vertices and aliases into one fat vertex
    /// - a lump, which is the same complaint from the other side.
    #[test]
    fn flare_lobe_stays_an_ellipse_not_a_sleeve() {
        let flare = Flare {
            axial: [1.0, 0.0, 0.0],
            acute: 1.0,
            // Perpendicular to every direction sampled below, so the
            // underside lobe contributes nothing and we read ONE lobe.
            down: [0.0, 0.0, -1.0],
            gain: FLARE_GAIN,
            decay_m: 1.0,
        };
        let at = |deg: f32| {
            let a = deg.to_radians();
            let w = flare.weight([a.cos(), a.sin(), 0.0]);
            (w - FLARE_FLUSH_W) / (1.0 - FLARE_FLUSH_W)
        };
        assert!(
            (at(0.0) - 1.0).abs() < 1e-4,
            "the crotch is supposed to be the peak of the lobe, got {}",
            at(0.0)
        );
        let sixty = at(60.0);
        assert!(
            sixty <= 0.35,
            "the flare lobe holds {sixty:.3} of its peak a full 60 degrees off \
             the crotch (FLARE_LOBE_P = {FLARE_LOBE_P}). That is a SLEEVE, which \
             is the 'bases of the branches look way too bulky' report - the \
             whole point of the directional flare is that a junction is an \
             ellipse. Needs p >= 1.5."
        );
        assert!(
            sixty >= 0.20,
            "the flare lobe is down to {sixty:.3} of its peak by 60 degrees \
             (FLARE_LOBE_P = {FLARE_LOBE_P}) - narrower than the angular step \
             between ring vertices on a 4-8 sided ring (`sides_for`), so it \
             aliases into one fat vertex: a lump, not a collar. Needs p <= 2.3."
        );
        // And the flank really is the flush end of the same field.
        assert!(
            at(90.0) < 1e-6,
            "the flank should carry none of the lobe, got {}",
            at(90.0)
        );
    }

    #[test]
    fn branch_radius_profile_is_monotonic_and_flared() {
        let r = registry();
        let (mut junctions, mut resolved, mut flared) = (0usize, 0usize, 0usize);
        let (mut lo_ratio, mut hi_ratio) = (f32::MAX, f32::MIN);
        let mut mean_ratio = 0.0f64;
        let mut worst_excess = 0.0f32;
        for t in r.trees.iter().map(as_procedural) {
            let min_r = t.height_m.max(0.5) * FLARE_RING_MIN_H_FRAC;
            for seed in 0..6u32 {
                let (parts, _) = build_accepted(&t, t.height_m, shipped_seed(seed));
                if seed == 0 {
                    // WHAT THE FLARE COSTS, per species, measured rather than
                    // estimated: the extra rings inside flare runs plus the
                    // second collar strip, against the wood the tree draws.
                    let wood = parts.wood.indices.len() / 3;
                    eprintln!(
                        "[flare-cost] {:>7}: +{} tris of {wood} wood ({:.1}%), {} junctions, \
                         {} resolved",
                        t.id,
                        parts.flare_tris,
                        parts.flare_tris as f32 / wood.max(1) as f32 * 100.0,
                        parts.forks.len(),
                        parts.forks.iter().filter(|f| f.shape.r0 >= min_r).count(),
                    );
                }
                for f in &parts.forks {
                    let s = f.shape;
                    assert!(
                        s.len > 0.0 && s.r0 > 0.0 && s.r1 > 0.0,
                        "{}: a junction recorded no limb ({s:?}) - the weld and the tube \
                         disagree about what is being drawn",
                        t.id
                    );
                    junctions += 1;
                    // The sectors this junction is measured in, as radial units
                    // of its own weld ring: the same directions the tube put
                    // vertices in, so every number below is about drawn
                    // geometry rather than about an idealised circle. 72 for
                    // the shape measurements (5 degrees resolves the lobes),
                    // every sixth of them for the 200-station monotone sweep.
                    let (side, up) = ring_basis(s.dir);
                    let sectors: Vec<[f32; 3]> = (0..72)
                        .map(|i| ring_dir(side, up, i as f32 / 72.0 * std::f32::consts::TAU))
                        .collect();

                    // 2/3/4. THE JUNCTION IS AN ELLIPSE: strong in the crotch,
                    // flush on the flanks.
                    //
                    // `flush` is the MINIMUM over a dense sweep, which is the
                    // honest flush side of the real field: a limb off a tilted
                    // parent carries its gravity lobe out of the attachment
                    // plane, so the two flanks are not equivalent and one of
                    // them is partly filled. (Measuring at the geometric flank
                    // instead reads up to 1.13x on a sakura and would be
                    // measuring the sampling, not the shape.) `mean` is the
                    // bulk number - it tracks the cross-sectional area, which
                    // is what "way too bulky" actually was.
                    let strong = s.weld_r_max() / s.r0;
                    let welds: Vec<f32> =
                        sectors.iter().map(|&m| s.radius_at_dir(m, 0.0) / s.r0).collect();
                    let flush = welds.iter().cloned().fold(f32::MAX, f32::min);
                    let mean = welds.iter().sum::<f32>() / welds.len() as f32;
                    lo_ratio = lo_ratio.min(flush);
                    hi_ratio = hi_ratio.max(strong);
                    mean_ratio += mean as f64;
                    assert!(
                        (1.0..=1.3).contains(&strong),
                        "{}: the crotch of a junction is {strong}x its shaft radius, outside \
                         1.0-1.3 - a real branch collar is the widest part of the limb but not \
                         a bulb",
                        t.id
                    );
                    assert!(
                        flush <= 1.10,
                        "{}: the FLANK of a junction is {flush}x its shaft radius - a branch \
                         base is an ellipse standing in the plane of its parent's axis, and a \
                         flank this proud means the flare has gone back to being a sleeve \
                         (which is the 'still way too bulky' report)",
                        t.id
                    );
                    assert!(
                        mean <= 1.20,
                        "{}: the junction's MEAN radius is {mean}x its shaft - the weld is \
                         carrying {:.0}% more cross-section than the limb it feeds, which is \
                         the bulk the operator sees whatever the peak says (the v0.1099 sleeve \
                         measured 1.45x mean, 2.10x area)",
                        t.id,
                        (mean * mean - 1.0) * 100.0
                    );
                    assert!(
                        s.weld_r_max() <= f.parent.r.max(s.r0) * 1.001,
                        "{}: a {} m weld stands proud of the {} m parent it leaves - wood does \
                         not get wider crossing a join",
                        t.id,
                        s.weld_r_max(),
                        f.parent.r
                    );
                    if f.parent.r >= s.r0 * (1.0 + FLARE_GAIN) {
                        // Room for the full collar swelling: it must be there,
                        // and it must be DIRECTIONAL.
                        assert!(
                            (1.2..=1.3).contains(&strong),
                            "{}: the crotch is only {strong}x the shaft radius with a {} m \
                             parent to swell into - the flare is dead and the join reads pinched",
                            t.id,
                            f.parent.r
                        );
                        assert!(
                            strong >= flush * 1.12,
                            "{}: crotch {strong}x against flank {flush}x - the flare has \
                             stopped being directional and is a sleeve again",
                            t.id
                        );
                        flared += 1;
                    }

                    // 1. MONOTONE PER SECTOR, on the law.
                    for &m in sectors.iter().step_by(6) {
                        let mut prev = f32::MAX;
                        for i in 0..=200 {
                            let x = s.len * i as f32 / 200.0;
                            let rr = s.radius_at_dir(m, x);
                            assert!(
                                rr <= prev + 1e-7,
                                "{}: radius RISES to {rr} m at {x} m along a {} m limb (was \
                                 {prev}) in sector {m:?} - this is the defect: skinny, then \
                                 suddenly wide",
                                t.id,
                                s.len
                            );
                            prev = rr;
                        }
                    }
                    // 1. MONOTONE PER SECTOR, on the rings that shipped. Read
                    // at fixed flare WEIGHTS rather than at fixed azimuths:
                    // the ring frame rotates a little as the limb bows, so
                    // azimuth i of ring k is not quite the same material line
                    // as azimuth i of ring k+1, whereas the weight sweep
                    // covers the crotch, the underside, mid-lobe and the flank
                    // exactly, on the numbers each ring was drawn from.
                    for w in [1.0f32, FLARE_UNDER_W, 0.5, FLARE_FLUSH_W] {
                        let mut prev = f32::MAX;
                        for ring in &f.rings {
                            let rr = ring.radius_at_weight(w);
                            assert!(
                                rr <= prev + 1e-6,
                                "{}: a drawn ring is {rr} m at flare weight {w} where the one \
                                 before it was {prev} m",
                                t.id
                            );
                            prev = rr;
                        }
                    }

                    // 5. DECAYED by three shaft radii, in every direction (the
                    // crotch is the worst case, so checking it covers all).
                    let x3 = (FLARE_SPAN * s.r0).min(s.len);
                    let base = s.base_radius_at(x3);
                    let excess = base * s.flare.mul_max(x3) / base - 1.0;
                    worst_excess = worst_excess.max(excess);
                    assert!(
                        excess < 0.02,
                        "{}: {:.1}% of flare is still left {x3} m out - a base flare that has \
                         not decayed is just a fat branch",
                        t.id,
                        excess * 100.0
                    );

                    // 6. RESOLVED where it is worth resolving.
                    if s.r0 >= min_r {
                        let near = f
                            .rings
                            .iter()
                            .filter(|r| dist(r.centre, f.start) <= FLARE_SPAN * s.r0 * 1.6)
                            .count();
                        assert!(
                            near >= 4,
                            "{}: a {} m junction is drawn with only {near} rings inside its \
                             flare run - the profile is right and the sampling cannot draw it",
                            t.id,
                            s.r0
                        );
                        resolved += 1;
                    }
                }
            }
        }
        assert!(junctions > 2_500, "only {junctions} junctions reached the flare gate");
        assert!(
            flared * 3 > junctions,
            "only {flared} of {junctions} junctions had room for the full collar swelling - \
             either the forms stopped conserving wood or the cap is eating the flare"
        );
        eprintln!(
            "[flare] {junctions} junctions across {} species x 6 variants: weld flank \
             {lo_ratio:.3}x - crotch {hi_ratio:.3}x shaft, mean {:.3}x (area {:.0}% over the \
             shaft, against 110% for the v0.1099 sleeve); {flared} with full room to swell, \
             worst residual at 3 r0 {:.2}%, {resolved} resolved with extra rings",
            r.trees.len(),
            mean_ratio / junctions as f64,
            ((mean_ratio / junctions as f64).powi(2) - 1.0) * 100.0,
            worst_excess * 100.0
        );
    }

    /// THE JOIN STILL READS AS WELDED - the front-of-the-fork half of the
    /// v0.1098 change, and the reason removing the burial is safe.
    ///
    /// The burial made a junction opaque by hiding the child's open root ring
    /// inside the parent. With nothing hidden, the collar has to close the join
    /// instead, and two properties make that airtight rather than hopeful:
    ///
    ///   1. its INNER edge lies exactly on the parent's surface, so there is no
    ///      slit between skirt and trunk; and
    ///   2. its OUTER edge is the limb's own first ring, vertex for vertex and
    ///      to the bit - both come from `ring_basis`/`ring_point`, so the two
    ///      surfaces cannot drift apart into a crack.
    ///
    /// Both edges are then confirmed to be present in the WOOD MESH THAT
    /// SHIPPED, not merely computable, so a collar that were silently skipped
    /// would fail here.
    #[test]
    fn every_junction_is_skinned_by_a_collar() {
        let r = registry();
        let mut collars = 0usize;
        for t in r.trees.iter().map(as_procedural) {
            let (parts, _) = build_accepted(&t, t.height_m, shipped_seed(2));
            // Rounded to 0.1 mm: the collar and the tube emit the same f32
            // expression, so this is an identity check with float-noise slack,
            // not a proximity check.
            let key = |p: [f32; 3]| {
                (
                    (p[0] * 10_000.0).round() as i64,
                    (p[1] * 10_000.0).round() as i64,
                    (p[2] * 10_000.0).round() as i64,
                )
            };
            let drawn: std::collections::HashSet<(i64, i64, i64)> =
                parts.wood.vertices.iter().map(|v| key(v.position)).collect();
            for f in &parts.forks {
                let j = f.parent;
                let (side, up) = ring_basis(f.shape.dir);
                for i in 0..f.shape.sides {
                    let a = i as f32 / f.shape.sides as f32 * std::f32::consts::TAU;
                    // The collar's outer edge is the limb's first ring, which
                    // is the flare's ELLIPSE (v0.1100) - so the identity check
                    // has to be per vertex too, through the same `ring_at` the
                    // generator drew with. A circle of `weld_r` would miss
                    // every vertex but the crotch.
                    let (rv, _) =
                        ring_at(f.shape.at(0.0), f.shape.base_radius_at(0.0), side, up, a);
                    let v = ring_point(f.start, side, up, a, rv);
                    let foot = j.project(v);
                    let rel = sub(foot, j.axis);
                    let radial = length(sub(rel, mul(j.up, dot(rel, j.up))));
                    assert!(
                        (radial - j.r).abs() < 1e-3,
                        "{}: a collar foot sits {radial} m from the parent axis, not on its \
                         {} m surface - the skirt does not meet the trunk and the join opens",
                        t.id,
                        j.r
                    );
                    assert!(
                        drawn.contains(&key(foot)),
                        "{}: a collar foot at {foot:?} is not in the drawn wood - the junction \
                         was never skinned",
                        t.id
                    );
                    assert!(
                        drawn.contains(&key(v)),
                        "{}: the collar's outer edge at {v:?} is not on the limb's own first \
                         ring - collar and limb have drifted apart and the join is a crack",
                        t.id
                    );
                }
                collars += 1;
            }
            assert!(!parts.forks.is_empty() || t.form == "palm", "{}: no junctions at all", t.id);
        }
        eprintln!("[collar] {collars} junctions skinned across {} species", r.trees.len());
    }

    /// The weld must not move the tree. Cheap proxy - the crown's extent stays
    /// in kind (this rides along with `every_procedural_form_builds_finite_
    /// geometry`, which bounds the top, by additionally bounding the horizontal
    /// spread).
    ///
    /// v0.1098 is the change this guards hardest. Rooting every limb on its
    /// parent's SURFACE instead of inside it moves each root out by roughly one
    /// parent radius, and that offset compounds down a four-generation chain -
    /// so the bound below is the statement that a collar-welded crown is the
    /// same size as a buried-root crown, not a visibly fatter one. It is also
    /// why the floor still matters in the other direction: with nothing buried,
    /// no limb travels DOWN into the bole any more, so the measured `lo` is now
    /// the trunk base rather than a root ring below it.
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
            // Nothing may sink below the ground plane the tree is placed on.
            assert!(lo > -t.height_m * 0.06, "{}: geometry reaches {lo} m below its base", t.id);
            assert!(wide < t.height_m * 1.6, "{}: crown spreads {wide} m", t.id);
        }
    }

    /// LIVE CROWN RATIO, the CI twin of the "NO FLAT-BOTTOMED CANOPY PLATE"
    /// gate on fuji-forest-ground (v0.1096).
    ///
    /// Two halves, matching the two halves of the image gate:
    ///   (1) the foliage-bearing wood must span at least 0.35 of tree height.
    ///       Real forest-grown temperate broadleaves run 0.35-0.60 and an
    ///       open-grown yoshino 0.7-0.8; v0.1095 measured ~0.25 because every
    ///       primary left one point on the bole.
    ///   (2) the crown's LOWER boundary must be ragged, not a level cut. The
    ///       screenshot version fits the per-column lowest foliage pixel; the
    ///       CI version takes the lowest twig in each of 8 azimuth sectors and
    ///       requires their spread to be a real fraction of the crown depth. A
    ///       flat plate scores ~0 on both.
    ///
    /// Broadleaf only, and that is a statement about the generator rather than
    /// a gap in the test: `build_at_density` records twigs for the broadleaf
    /// path alone, so `crown_of` has nothing to measure on a conifer, umbrella
    /// or palm. Those forms build their crowns without `limb`, and the same
    /// defect on the conifer path is a separate finding with its own fix.
    #[test]
    fn crown_depth_is_a_real_live_crown_ratio() {
        let r = registry();
        let mut seen = 0usize;
        for t in r.trees.iter().filter(|t| t.form == "broadleaf").map(as_procedural) {
            for v in 0..t.variants.max(1) {
                seen += 1;
                let seed = shipped_seed(v);
                let (_, twigs) = build_accepted(&t, t.height_m, seed);
                assert!(!twigs.is_empty(), "{} v{v}: no foliage-bearing twigs at all", t.id);
                let crown = crown_of(&twigs);
                let lcr = crown.depth_m / t.height_m;

                // Lowest twig per azimuth sector about the crown axis.
                const SECTORS: usize = 8;
                let mut lowest = [f32::MAX; SECTORS];
                for w in &twigs {
                    for p in [w.from, w.end] {
                        let a = (p[2] - crown.centre[2]).atan2(p[0] - crown.centre[0]);
                        let i = (((a / std::f32::consts::TAU) + 1.0) * SECTORS as f32) as usize
                            % SECTORS;
                        lowest[i] = lowest[i].min(p[1]);
                    }
                }
                let hit: Vec<f32> = lowest.iter().copied().filter(|y| *y < f32::MAX).collect();
                let mean = hit.iter().sum::<f32>() / hit.len() as f32;
                let sd = (hit.iter().map(|y| (y - mean) * (y - mean)).sum::<f32>()
                    / hit.len() as f32)
                    .sqrt();
                let ragged = sd / crown.depth_m.max(1e-3);
                eprintln!(
                    "[lcr] {:>7} v{v}: {} twigs, crown depth {:.2} m on a {:.1} m tree = LCR \
                     {lcr:.2}, spread {:.2} m (aspect {:.2}:1), underside SD {sd:.2} m = \
                     {ragged:.2} of depth",
                    t.id,
                    twigs.len(),
                    crown.depth_m,
                    t.height_m,
                    crown.spread_m,
                    2.0 * crown.spread_m / crown.depth_m.max(1e-3)
                );
                assert!(
                    lcr >= 0.35,
                    "{} v{v}: live crown ratio {lcr:.2} - the foliage spans {:.2} m of a {:.1} m \
                     tree, which is a flat-bottomed plate, not a crown. A forest-grown temperate \
                     broadleaf runs 0.35-0.60 and an open-grown yoshino 0.7-0.8.",
                    t.id,
                    crown.depth_m,
                    t.height_m
                );
                assert!(
                    ragged >= 0.08,
                    "{} v{v}: the lowest twig in each of {SECTORS} azimuth sectors varies by only \
                     {sd:.2} m ({ragged:.2} of the {:.2} m crown depth) - the crown bottoms out at \
                     one level all the way round, which is the horizontal cut the vantage gate \
                     rejects",
                    t.id,
                    crown.depth_m
                );
            }
        }
        assert!(seen > 0, "no broadleaf rows in the registry to measure");
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

    /// SPECIES THAT SHIP WITHOUT A `clusters` BLOCK, and are therefore skipped
    /// by every card gate below. Dated, visible and asserted in both
    /// directions, because an entry here is a species rendering with no canopy
    /// layer at all - never a preference, always a debt.
    ///
    /// 2026-08-03 (v0.1102): fir, pine, acacia, palm. Their FORMS became
    /// card-capable in this increment - `every_form_emits_cards_when_given_a_
    /// cluster_block` proves each one emits cards on a measured crown with a
    /// sprig that fits its card - and what is still missing is only the data
    /// row, which lives in `data/vegetation/trees.ron`, another lane. Delete a
    /// name the moment its row gains a `clusters:` block; `assert_card_gate_
    /// coverage` FAILS on a name that has one, so this list cannot rot in
    /// either direction.
    /// Species with no `clusters` block, dated so the exemption cannot become
    /// permanent by inattention. ACACIA CAME OFF THIS LIST in v0.1103: real
    /// allometry took its terminal twigs from 11.3 mm radius to 4.0 mm, which
    /// is what `cluster_cards_envelop_every_twig_tip` was rightly refusing.
    ///
    /// FIR AND PINE CAME OFF IT in v0.1107, which is what this whole list was
    /// counting down to: they were 7508 double-sided deltoids at 0.483 m and
    /// 0.351 m mean size - 91% of an 8260-triangle tree and 100% of its
    /// silhouette - and those are the "grass leaves" the operator reported
    /// three builds running. Their rows now carry needle-shoot cluster blocks,
    /// the carded arm of `foliage_of` drops the drawn blade to a 0.10-0.15 m
    /// branchlet, and the tree comes out CHEAPER (5612 and 5692 triangles
    /// against 8260, card layers 3060 and 3140 inside the 3400 budget).
    ///
    /// Palm is the last one, and it needs its card sized deliberately: its
    /// drawn element is a life-size 0.82 m coconut leaflet, so a palm card is
    /// a ~2 m chunk of frond and its lent-block sprite bakes 8.55x its card.
    const UNCARDED_SPECIES: &[&str] = &["palm"];

    /// THE NON-VACUITY GUARD every card gate needs.
    ///
    /// A card gate opens with `let Some(cd) = t.clusters else { continue }`, so
    /// it certifies nothing about a species that has no block - and through
    /// v0.1101 that was HALF THE REGISTRY (fir, pine, acacia, palm), including
    /// every species in the operator's bare-sticks capture. A skip that nobody
    /// counts is a gate that cannot fail for exactly the species that need it.
    ///
    /// So: assert the gate saw someone, assert every skip is a KNOWN skip, and
    /// assert every known skip is still genuinely uncarded. The pattern is the
    /// one `crown_depth_is_a_real_live_crown_ratio` already uses for its
    /// broadleaf filter (`assert!(seen > 0)`), carried to its full form.
    fn assert_card_gate_coverage(gate: &str, seen: usize, skipped: &[String]) {
        assert!(
            seen > 0,
            "{gate} certified NOTHING: every species was skipped, so this gate cannot fail"
        );
        for id in skipped {
            assert!(
                UNCARDED_SPECIES.contains(&id.as_str()),
                "{gate} silently skipped `{id}`: it carries no `clusters` block and is not on \
                 UNCARDED_SPECIES, so it renders with no card canopy AND no card gate looked at \
                 it. Give it a block, or add it to that list with a dated reason"
            );
        }
        let r = registry();
        for id in UNCARDED_SPECIES {
            let i = r
                .index_of(id)
                .unwrap_or_else(|| panic!("UNCARDED_SPECIES names `{id}`, which is not a row"));
            assert!(
                r.get(i).and_then(|t| t.clusters.as_ref()).is_none(),
                "`{id}` has gained a `clusters` block - take it off UNCARDED_SPECIES so the card \
                 gates start binding on it"
            );
        }
    }

    /// Every distinct `form` the registry ships, in a stable order.
    ///
    /// Read off the data rather than written out, so a form added to
    /// `data/vegetation/trees.ron` tomorrow is covered by the gates below
    /// without anyone remembering to widen a list here.
    fn shipped_forms() -> Vec<String> {
        let mut f: Vec<String> = registry().trees.iter().map(|t| t.form.clone()).collect();
        f.sort();
        f.dedup();
        f
    }

    /// The first cluster block the registry ships, to LEND to a species that
    /// has none yet. Data, not a literal: a hand-written `ClusterDef` here
    /// would drift from the shipped shape the moment the struct grows a field.
    fn borrowed_cluster_def() -> ClusterDef {
        registry()
            .trees
            .iter()
            .find_map(|t| t.clusters.clone())
            .expect("the registry ships at least one cluster block to lend")
    }

    /// A `ClusterDef` ON ANY FORM MUST ACTUALLY PRODUCE CARDS.
    ///
    /// THE HOLE THIS CLOSES (v0.1102). Every other card gate in this file opens
    /// with `let Some(cd) = t.clusters else { continue }`, so a species with no
    /// cluster block is silently exempt from all of them - and a FORM that
    /// structurally cannot record a `Twig` is exempt for every species it will
    /// ever carry. That is a check which cannot fail for exactly the species
    /// that most need it. Measured before the fix: `conifer`, `umbrella` and
    /// `palm` never pushed a `Twig`, so fir, pine, acacia and palm emitted ZERO
    /// cards against a degenerate `crown_of` fallback of radius 1.0 m - while
    /// every card gate in the file stayed green, because none of them ran.
    ///
    /// So this gate supplies the missing half itself: it takes one
    /// representative species of every form the registry ships, LENDS it a real
    /// cluster block, and asserts the form answers with cards on a measured
    /// crown. It needs no data row to exist, which is the entire point - the
    /// capability is proven before the data lands, not after it fails.
    #[test]
    fn every_form_emits_cards_when_given_a_cluster_block() {
        let r = registry();
        let cd = borrowed_cluster_def();
        let forms = shipped_forms();
        assert!(forms.len() >= 4, "only {} forms in the registry: {forms:?}", forms.len());
        for form in &forms {
            let base = r
                .trees
                .iter()
                .find(|t| &t.form == form)
                .unwrap_or_else(|| panic!("no species of form {form}"));
            let mut t = as_procedural(base);
            t.clusters = Some(cd.clone());
            let seed = shipped_seed(0);
            let twigs = twigs_of(&t, seed);
            let built = build_tree_and_cards(&t, t.height_m, seed);
            let crown = crown_envelope(&t, t.height_m, seed);
            let cards: u32 = built.cards.iter().map(|c| c.cards).sum();
            let card_tris: usize = built.cards.iter().map(|c| c.mesh.indices.len() / 3).sum();
            let tris = (built.mesh.indices.len() + built.wood.indices.len()) / 3 + card_tris;
            eprintln!(
                "[formcards] {form:>9} ({:>7}): {} twigs, {cards} cards at {:.2} m, crown r \
                 {:.2} m / spread {:.2} m / depth {:.2} m, {tris} tris ({card_tris} card)",
                t.id,
                twigs.len(),
                built.cards.iter().map(|c| c.card_side_m).fold(0.0f32, f32::max),
                crown.radius_m,
                crown.spread_m,
                crown.depth_m
            );
            assert!(
                !twigs.is_empty(),
                "{form} ({}): the form records no Twig, so cluster cards can never sleeve it - \
                 this is the conifer/umbrella/palm blocker, back again",
                t.id
            );
            assert!(
                !built.cards.is_empty() && cards >= 20,
                "{form} ({}): a species carrying a cluster block emitted {cards} cards - the \
                 card layer is silently absent and every other card gate skips it",
                t.id
            );
            // A REAL crown, not `crown_of`'s empty-twig fallback. Without this
            // the gate above could be satisfied by one stray twig while the LAI
            // fit spends a 1 m fictional crown.
            assert!(
                crown.radius_m > 0.5 && crown.spread_m > 0.5 && crown.depth_m > 0.5,
                "{form} ({}): crown measures r {:.2} / spread {:.2} / depth {:.2} m - that is \
                 the r=1.0 empty-twig fallback, so the LAI fit is solving against a fiction",
                t.id,
                crown.radius_m,
                crown.spread_m,
                crown.depth_m
            );

            // THE SPRITE MUST FIT THE CARD - the other half of "this form can
            // carry cards", and the half `cluster_sprite_geometry_fits_its_card`
            // cannot check for a species with no data row yet. Same 0.35-1.5x
            // band that gate uses, measured the same way (the baker frames on
            // the geometry's own AABB, so a sprite bigger than its card is not
            // clipped - it is SHRUNK, and every element on the tree silently
            // renders at the wrong scale).
            //
            // PALM IS EXEMPT, dated 2026-08-03, and the reason is botanical
            // rather than a shrug: its drawn element is a 0.82 m COCONUT
            // LEAFLET, which is life size (a real pinna is 0.6-0.9 m), so a
            // palm card is a ~2 m chunk of frond and the 0.50 m block lent here
            // is not a fair stand-in. Remove this exemption in the increment
            // that gives palm its own `clusters` block, and size that block so
            // the fit lands at 1.9 m or more (its sprite measures 5.73 m).
            let side = built.cards.iter().map(|c| c.card_side_m).fold(0.0f32, f32::max);
            let fol = foliage_of(&t, t.height_m, 1.0);
            let sprite = cluster_sprite_geometry(&t, ClusterLayer::Leaf, t.height_m)
                .expect("a lent cluster block always yields sprite geometry");
            let (mut mn, mut mx) = ([f32::MAX; 3], [f32::MIN; 3]);
            for v in &sprite.vertices {
                for i in 0..3 {
                    mn[i] = mn[i].min(v.position[i]);
                    mx[i] = mx[i].max(v.position[i]);
                }
            }
            let ext = (mx[0] - mn[0]).max(mx[2] - mn[2]).max(mx[1] - mn[1]);
            eprintln!(
                "            sprig reach {:.3} m ({:.2} of card), sprite {ext:.2} m in a \
                 {side:.2} m card ({:.2}x)",
                fol.sprig_span() + fol.leaf,
                (fol.sprig_span() + fol.leaf) / side,
                ext / side
            );
            if form != "palm" {
                assert!(
                    ext > side * 0.35 && ext < side * 1.5,
                    "{form} ({}): its cluster sprite bakes {ext:.2} m into a {side:.2} m card \
                     ({:.2}x). One sprig reaches {:.2} m; see the measured table above \
                     `foliage_of` - the shipped broadleaves sit at 0.29-0.36 of their card and \
                     bake within 4% of it",
                    t.id,
                    ext / side,
                    fol.sprig_span() + fol.leaf
                );
            }

            // AND THE BLADES MUST LIVE INSIDE THE CARD MASS, the same measure
            // `near_blades_stay_inside_the_card_shell` applies to the carded
            // broadleaves - which, again, it cannot apply to a species with no
            // data row. Only `limb` shrank its blade clump for a carded species
            // before v0.1102, so `conifer` and `umbrella` scattered blades to a
            // full clump radius (1.10 m on a 22 m fir) around twigs whose card
            // shell ends at 0.82 m: a fringe of raw triangles round every
            // silhouette, which is exactly what the operator's capture shows.
            const ORGAN_BIT_LEAF: u32 = 524_288;
            let shell = side * (CLUSTER_SLEEVE_OFFSET + 0.5);
            let (mut blades, mut outside, mut worst) = (0usize, 0usize, 0.0f32);
            for f in built.mesh.indices.chunks(3) {
                if (built.mesh.vertices[f[0] as usize].uv[0].max(0.0).round() as u32)
                    & ORGAN_BIT_LEAF
                    == 0
                {
                    continue;
                }
                blades += 1;
                let far = f
                    .iter()
                    .map(|&i| {
                        let p = built.mesh.vertices[i as usize].position;
                        twigs
                            .iter()
                            .map(|w| point_segment_dist(p, w.from, w.end))
                            .fold(f32::MAX, f32::min)
                    })
                    .fold(0.0f32, f32::max);
                worst = worst.max(far);
                if far > shell {
                    outside += 1;
                }
            }
            let frac = outside as f32 / blades.max(1) as f32;
            eprintln!(
                "            {blades} blade faces, {outside} ({:.1}%) outside the {shell:.2} m \
                 card mass, worst {worst:.2} m",
                frac * 100.0
            );
            assert!(blades > 20, "{form} ({}): only {blades} blade faces", t.id);
            // Palm exempt for the SAME reason as the sprite check above, and it
            // is the same arithmetic: its 0.82 m leaflet cannot sit inside a
            // shell derived from a 0.67 m borrowed card. On the ~2 m card its
            // own block will settle at, the shell is 1.6 m and the measured
            // worst blade (0.66 m) is comfortably inside it.
            assert!(
                frac < 0.02 || form == "palm",
                "{form} ({}): {:.0}% of blade faces stand proud of the {shell:.2} m card mass \
                 (worst {worst:.2} m) - raw triangles poking out of the crown",
                t.id,
                frac * 100.0
            );
        }
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
        // what it should. `vs_model()` rather than `obj_model()` since
        // v0.1091: grass strands have no object uniform at all, so the vertex
        // stage's model matrix comes from an accessor that returns the
        // per-instance transform for type 23 and the object uniform for
        // everything else.
        assert!(
            wgsl.contains("let iscale = max(length(vs_model()[0].xyz), 1.0e-4);"),
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
                        // and a limb compresses toward its tip), but it must
                        // never STRETCH past the rounding bound: repeats are
                        // round(circumference/tile) at the limb's WIDEST ring
                        // (v0.1100), so the worst case is a ring 1.5 tiles
                        // round pinned at one repeat. A model-scale or
                        // fixed-0..1 ring - the silent failure this guards -
                        // lands 10x to 30x out.
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

    /// THE UV CONTINUITY GATE (v0.1100). The operator's second field report as
    /// a measurement: "visible seams at trunk ring boundaries where the voronoi
    /// bark cells JUMP SIZE... a large voronoi cell texture and a smaller one
    /// underneath".
    ///
    /// The cause was one line: `bark_tube` derived its azimuthal repeat count
    /// from each SEGMENT's own fat end and rounded it to an integer, so a
    /// tapering limb stepped through repeat counts (a 10-segment broadleaf stem
    /// runs 5,4,4,3,3,2,2,2,1,1) and every step is a ring where the plate scale
    /// changes - the 2 -> 1 step DOUBLES the cell width. Now the count is
    /// derived ONCE per limb, from its widest drawn ring (`open_bark_run`).
    ///
    /// The gate reads what was DRAWN rather than what the law says: every tube
    /// segment reports the count it was handed into its limb's run, and a run
    /// whose `reps_lo` and `reps_hi` differ is a limb that changed plate scale
    /// somewhere along itself. It also proves the accounting is total - every
    /// tube segment belongs to a run, and there is exactly one run per limb
    /// (one per junction, plus the stem/leader/bole/palm-column that no
    /// junction opens).
    #[test]
    fn bark_uv_repeats_are_continuous_along_every_limb() {
        let r = registry();
        let mut runs_seen = 0usize;
        for t in r.trees.iter().map(as_procedural) {
            for seed in 0..3u32 {
                let (parts, _) = build_accepted(&t, t.height_m, shipped_seed(seed));
                let mut attributed = 0u32;
                let mut worst_foreshorten = 1.0f32;
                for run in &parts.bark_runs {
                    assert!(
                        run.segments > 0,
                        "{}: a bark run drew no tube at all - `open_bark_run` was called \
                         somewhere that emits no bark",
                        t.id
                    );
                    assert!(
                        (run.reps_hi - run.reps_lo).abs() < 1e-6,
                        "{}: one limb drew bark at {} repeats in one segment and {} in \
                         another ({} segments, reference radius {:.4} m) - that is a ring \
                         where the voronoi plates change size, which is exactly the seam \
                         the operator photographed",
                        t.id,
                        run.reps_lo,
                        run.reps_hi,
                        run.segments,
                        run.ref_r
                    );
                    // A whole number of periods, or the ring does not close and
                    // every limb carries a wrap seam instead.
                    assert!(
                        (run.reps_lo - run.reps_lo.round()).abs() < 1e-6 && run.reps_lo >= 1.0,
                        "{}: a limb's bark repeats are {} - not a whole number of texture \
                         periods, so the ring cannot close",
                        t.id,
                        run.reps_lo
                    );
                    attributed += run.segments;
                    runs_seen += 1;
                    if run.r_thin > 0.0 {
                        worst_foreshorten = worst_foreshorten.max(run.r_fat / run.r_thin);
                    }
                }
                assert_eq!(
                    attributed as usize, parts.bark_tubes,
                    "{}: {} of {} tube segments belong to a bark run - a segment drawn \
                     outside a run carries whatever count the previous limb left behind",
                    t.id, attributed, parts.bark_tubes
                );
                // ONE run per limb: one per junction (opened by the weld), plus
                // the single trunk/leader/bole/palm column each form draws
                // before any junction exists.
                assert_eq!(
                    parts.bark_runs.len(),
                    parts.forks.len() + 1,
                    "{}: {} bark runs against {} junctions + 1 stem - a limb either opened \
                     two runs or shared one with its neighbour",
                    t.id,
                    parts.bark_runs.len(),
                    parts.forks.len()
                );
                if seed == 0 {
                    eprintln!(
                        "[bark-uv] {:>7}: {} limbs, {} tube segments, one repeat count each; \
                         worst plate foreshortening within a limb {:.1}x (root to tip)",
                        t.id,
                        parts.bark_runs.len(),
                        parts.bark_tubes,
                        worst_foreshorten
                    );
                }
            }
        }
        assert!(runs_seen > 1_000, "only {runs_seen} bark runs reached the gate");
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

    /// Bark brown, foliage green, cluster-card green: the three tints the
    /// software dev aids draw with, so a dumped frame is readable as wood
    /// against blades against cards without any material system.
    // theme-exempt: a debug rasteriser's own palette, never a UI colour.
    const WOOD_TINT: [f32; 3] = [1.0, 0.86, 0.66];
    // theme-exempt: as above.
    const BLADE_TINT: [f32; 3] = [0.42, 1.0, 0.34];
    // theme-exempt: as above.
    const CARD_TINT: [f32; 3] = [0.20, 0.72, 0.26];

    /// THE SOFTWARE RASTERISER the dev aids share: draw one mesh
    /// orthographically into an RGBA buffer, depth-tested, flat-lambert shaded
    /// against the view direction plus a vertical key so a face turned away
    /// still reads.
    ///
    /// No GPU, so it runs on any build machine in milliseconds and never takes
    /// the operator's one GPU (the ONE GPU rule in CLAUDE.md). `project` maps
    /// world to (pixel x, pixel y, depth-toward-camera, smaller is nearer).
    fn raster_mesh(
        buf: &mut [u8],
        depth: &mut [f32],
        px: usize,
        m: &PlantMeshBuilder,
        project: &dyn Fn([f32; 3]) -> (f32, f32, f32),
        fwd: [f32; 3],
        tint: [f32; 3],
    ) {
        for tri in m.indices.chunks(3) {
            let v: Vec<_> = tri.iter().map(|&i| m.vertices[i as usize]).collect();
            let s: Vec<_> = v.iter().map(|q| project(q.position)).collect();
            let n = v[0].normal;
            let l = (-dot(n, fwd) * 0.45 + n[1] * 0.30 + 0.48).clamp(0.18, 1.0);
            let (lo_x, hi_x) = (
                s.iter().fold(f32::MAX, |a, q| a.min(q.0)).floor().max(0.0) as usize,
                s.iter().fold(f32::MIN, |a, q| a.max(q.0)).ceil().min(px as f32 - 1.0) as usize,
            );
            let (lo_y, hi_y) = (
                s.iter().fold(f32::MAX, |a, q| a.min(q.1)).floor().max(0.0) as usize,
                s.iter().fold(f32::MIN, |a, q| a.max(q.1)).ceil().min(px as f32 - 1.0) as usize,
            );
            let area =
                (s[1].0 - s[0].0) * (s[2].1 - s[0].1) - (s[2].0 - s[0].0) * (s[1].1 - s[0].1);
            if area.abs() < 1e-6 {
                continue;
            }
            for yy in lo_y..=hi_y {
                for xx in lo_x..=hi_x {
                    let (fx, fy) = (xx as f32 + 0.5, yy as f32 + 0.5);
                    let w0 = ((s[1].0 - fx) * (s[2].1 - fy) - (s[2].0 - fx) * (s[1].1 - fy))
                        / area;
                    let w1 = ((s[2].0 - fx) * (s[0].1 - fy) - (s[0].0 - fx) * (s[2].1 - fy))
                        / area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    let z = w0 * s[0].2 + w1 * s[1].2 + w2 * s[2].2;
                    let i = yy * px + xx;
                    if z >= depth[i] {
                        continue;
                    }
                    depth[i] = z;
                    for c in 0..3 {
                        buf[i * 4 + c] = (l * 255.0 * tint[c]).clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
    }

    /// DEV AID (permanent): render the BARE WOOD of a species around its
    /// biggest junction, from four sides, to `debug/fork_<id>_<view>.png`.
    ///
    /// Built for the back-poke class of defect (v0.1098) and kept for it: the
    /// tell is only visible from BEHIND the trunk, with the foliage off, at a
    /// junction close enough to fill the frame - which is a view the probe rig
    /// cannot reach cheaply and a full boot shows buried under leaves. This
    /// software rasteriser needs no GPU, so it runs anywhere, in ~50 ms, and it
    /// picks the camera pose off the recorded junction rather than asking
    /// anyone to guess where to stand.
    ///
    /// `cargo test --features native --lib dump_fork_png -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn dump_fork_png() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("debug");
        std::fs::create_dir_all(&dir).expect("debug dir");
        let r = registry();
        for t in r.trees.iter().map(as_procedural) {
            let (parts, _) = build_accepted(&t, t.height_m, shipped_seed(0));
            // The junction the operator would walk up to: the thickest one.
            let Some(f) = parts
                .forks
                .iter()
                .max_by(|a, b| a.parent.r.partial_cmp(&b.parent.r).unwrap())
            else {
                continue;
            };
            let out = f.parent.outward(f.shape.dir);
            let across = norm(cross(out, f.parent.up));
            // Six parent radii of frame, centred on the JOIN rather than on the
            // parent's axis: a few millimetres of overhang is then several
            // pixels, and the collar fills a useful part of the shot.
            let span = (f.parent.r * 6.0).max(0.4);
            let centre = mix3(f.parent.axis, f.start, 0.5);
            // A dead-astern view foreshortens the branch to a dot (it points
            // straight away from the camera), so the pose that actually shows a
            // back-poke is three-quarter rear: the trunk's silhouette edge is
            // in frame and anything protruding through it is unmissable.
            let rear3q = norm(add(mul(out, -1.0), across, 0.9));
            // THE PROFILE SHOT (v0.1099). The four poses above are framed on the
            // PARENT, which is the right frame for "does anything poke out the
            // back" and the wrong one for "what shape is the join": at six
            // parent radii the first few shaft radii of the branch - the entire
            // base flare - are a dozen pixels. This one is framed on the branch
            // BASE and looks square across the limb, so the silhouette the
            // operator described (pinch, then a sudden full-width shaft) is
            // what fills the frame.
            let collar_at = add(f.start, f.shape.dir, f.weld_r * 1.6);
            let collar_span = f.weld_r * 7.0;
            // THE ELLIPSE SHOT (v0.1100). "collar" looks ACROSS the plane the
            // junction ellipse stands in, so its silhouette is the LONG axis -
            // crotch fill and collar swelling. This one looks ALONG that
            // plane, from the obtuse side, so its silhouette is the SHORT axis
            // across the flanks. The pair is how you tell a directional collar
            // from the radially symmetric sleeve v0.1099 drew: a sleeve gives
            // two identical silhouettes and an ellipse does not.
            let flank = mul(f.shape.flare.axial, -f.shape.flare.acute);
            for (name, eye, centre, span) in [
                ("behind", mul(out, -1.0), centre, span),
                ("rear3q", rear3q, centre, span),
                ("side", across, centre, span),
                ("under", norm([out[0] * 0.5, -1.0, out[2] * 0.5]), centre, span),
                ("collar", across, collar_at, collar_span),
                ("flank", flank, collar_at, collar_span),
            ] {
                let px = 420usize;
                let fwd = norm(eye);
                let helper = if fwd[1].abs() < 0.9 { [0.0, 1.0, 0.0] } else { [1.0, 0.0, 0.0] };
                let right = norm(cross(helper, fwd));
                let cam_up = cross(fwd, right);
                let mut buf = vec![18u8, 22, 15, 255].repeat(px * px);
                let mut depth = vec![f32::MAX; px * px];
                let project = |p: [f32; 3]| {
                    let rel = sub(p, centre);
                    let x = dot(rel, right) / span * 0.5 + 0.5;
                    let y = 0.5 - dot(rel, cam_up) / span * 0.5;
                    (x * px as f32, y * px as f32, -dot(rel, fwd))
                };
                raster_mesh(&mut buf, &mut depth, px, &parts.wood, &project, fwd, WOOD_TINT);
                let path = dir.join(format!("fork_{}_{name}.png", t.id));
                image::RgbaImage::from_raw(px as u32, px as u32, buf)
                    .expect("size")
                    .save(&path)
                    .expect("write png");
            }
            // THE BARE SKELETON (v0.1103): the whole tree, WOOD ONLY, framed on
            // its own height at a fixed metres-per-pixel. It lives here rather
            // than in `dump_crown_png` for two reasons: this test runs in 12 s
            // where that one spends 5 minutes in sprite bakes, and that one
            // draws the card layer OVER the wood, so the branches it is meant
            // to show are hidden behind foliage in every shot.
            //
            // This is the view that answers "are the branches plumbing", and
            // the one the radius work of this increment was judged on. Framed
            // on `height_m` (not on a junction), so two runs at different
            // constants are directly comparable pixel for pixel.
            let px = 700usize;
            let fwd = [1.0f32, 0.0, 0.0];
            let right = norm(cross([0.0, 1.0, 0.0], fwd));
            let cam_up = cross(fwd, right);
            let (centre, span) = ([0.0, t.height_m * 0.5, 0.0], t.height_m * 0.58);
            let mut buf = vec![18u8, 22, 34, 255].repeat(px * px);
            let mut depth = vec![f32::MAX; px * px];
            let project = |p: [f32; 3]| {
                let rel = sub(p, centre);
                (
                    (dot(rel, right) / span * 0.5 + 0.5) * px as f32,
                    (0.5 - dot(rel, cam_up) / span * 0.5) * px as f32,
                    -dot(rel, fwd),
                )
            };
            raster_mesh(&mut buf, &mut depth, px, &parts.wood, &project, fwd, WOOD_TINT);
            image::RgbaImage::from_raw(px as u32, px as u32, buf)
                .expect("size")
                .save(dir.join(format!("wood_{}.png", t.id)))
                .expect("write png");

            // The three numbers that say whether the flare is DIRECTIONAL, on
            // the drawn ring: the crotch, the flank, and the mean, each as a
            // multiple of the shaft radius.
            let (side_b, up_b) = ring_basis(f.shape.dir);
            let ring: Vec<f32> = (0..48)
                .map(|i| {
                    let a = i as f32 / 48.0 * std::f32::consts::TAU;
                    f.shape.radius_at_dir(ring_dir(side_b, up_b, a), 0.0) / f.shape.r0
                })
                .collect();
            let hi = ring.iter().cloned().fold(f32::MIN, f32::max);
            let lo = ring.iter().cloned().fold(f32::MAX, f32::min);
            let mean = ring.iter().sum::<f32>() / ring.len() as f32;
            eprintln!(
                "[fork] {}: parent r {:.3} at {:?} tip={} | child shaft r {:.3} welds at \
                 {:.3} m crotch / {:.3} m flank ({hi:.2}x / {lo:.2}x shaft, mean {mean:.2}x, \
                 ellipse {:.2}:1) starts {:?} ({:.3} m out, {:.3} m up) -> \
                 debug/fork_{}_*.png",
                t.id,
                f.parent.r,
                f.parent.axis,
                f.parent.tip,
                f.shape.r0,
                hi * f.shape.r0,
                lo * f.shape.r0,
                hi / lo.max(1e-6),
                f.start,
                dot(sub(f.start, f.parent.axis), out),
                dot(sub(f.start, f.parent.axis), f.parent.up),
                t.id
            );
        }
    }

    /// DEV AID (permanent): render a WHOLE TREE of every form - wood, near
    /// blades and cluster cards, colour-coded - to `debug/crown_<id>.png`,
    /// beside its baked cluster sprite at `debug/sprite_<id>.png`, and print
    /// the four numbers that decide whether a form's card layer can work: the
    /// card side the LAI fit settles on, the sprite's extent against that card,
    /// the sprite's measured ALPHA COVERAGE, and its triangle count.
    ///
    /// WHY IT EXISTS (v0.1102). "Does a fir read as a fir" is a question only a
    /// picture answers, and the only pictures available before this were a GPU
    /// boot or a probe sweep - both far too heavy to run per iteration while
    /// tuning a foliage element, and both contending for the operator's ONE GPU
    /// (see CLAUDE.md). This is `dump_fork_png`'s rasteriser plus
    /// `billboard_bake`'s CPU twin of the sprite bake, so what it reports is
    /// what the GPU would have baked, in about a second, on any machine.
    ///
    /// A species with no cluster block of its own is LENT one, which is the
    /// whole point: it shows what the form does the moment the data lands.
    ///
    /// `cargo test --features native --lib dump_crown_png -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn dump_crown_png() {
        use crate::renderer::billboard_bake::{
            cpu_cluster_sprite, CLUSTER_BAKE_PX, CLUSTER_SPRITE_PX,
        };
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("debug");
        std::fs::create_dir_all(&dir).expect("debug dir");
        let r = registry();
        let lent = borrowed_cluster_def();
        for form in shipped_forms() {
            for base in r.trees.iter().filter(|t| t.form == form) {
                let mut t = as_procedural(base);
                let borrowed = t.clusters.is_none();
                if borrowed {
                    t.clusters = Some(lent.clone());
                }
                let seed = shipped_seed(0);
                let built = build_tree_and_cards(&t, t.height_m, seed);
                let side = mean_card_side(&t, ClusterLayer::Leaf);
                let sprite = cluster_sprite_geometry(&t, ClusterLayer::Leaf, t.height_m)
                    .expect("every species here carries a cluster block");
                let (mut mn, mut mx) = ([f32::MAX; 3], [f32::MIN; 3]);
                for v in &sprite.vertices {
                    for i in 0..3 {
                        mn[i] = mn[i].min(v.position[i]);
                        mx[i] = mx[i].max(v.position[i]);
                    }
                }
                let cpu = cpu_cluster_sprite(
                    &t,
                    ClusterLayer::Leaf,
                    CLUSTER_BAKE_PX,
                    CLUSTER_SPRITE_PX,
                );
                let fol = foliage_of(&t, t.height_m, 1.0);
                let cards: u32 = built.cards.iter().map(|c| c.cards).sum();
                eprintln!(
                    "[crown] {:>9} {:>7}{}: element {:.3} m x{} per sprig (reach {:.3} m), \
                     {cards} cards at {side:.2} m, sprite {:.2} x {:.2} m, coverage {:.3}, \
                     {} sprite tris -> debug/crown_{}.png",
                    form,
                    t.id,
                    if borrowed { " (lent)" } else { "" },
                    fol.leaf,
                    fol.per_sprig,
                    fol.sprig_span() + fol.leaf,
                    mx[0] - mn[0],
                    mx[1] - mn[1],
                    cpu.as_ref().map(|c| c.coverage).unwrap_or(0.0),
                    cpu.as_ref().map(|c| c.triangles).unwrap_or(0),
                    t.id
                );
                if let Some(c) = cpu {
                    let img =
                        image::RgbaImage::from_raw(c.sprite_px, c.sprite_px, c.rgba).expect("size");
                    img.save(dir.join(format!("sprite_{}.png", t.id))).expect("write png");
                }

                // The whole tree, side on, framed on its full height.
                let px = 700usize;
                let fwd = [1.0f32, 0.0, 0.0];
                let right = norm(cross([0.0, 1.0, 0.0], fwd));
                let cam_up = cross(fwd, right);
                let centre = [0.0, t.height_m * 0.52, 0.0];
                let span = t.height_m * 1.12;
                let mut buf = vec![18u8, 22, 34, 255].repeat(px * px);
                let mut depth = vec![f32::MAX; px * px];
                let project = |p: [f32; 3]| {
                    let rel = sub(p, centre);
                    (
                        (dot(rel, right) / span * 0.5 + 0.5) * px as f32,
                        (0.5 - dot(rel, cam_up) / span * 0.5) * px as f32,
                        -dot(rel, fwd),
                    )
                };
                raster_mesh(&mut buf, &mut depth, px, &built.wood, &project, fwd, WOOD_TINT);
                raster_mesh(&mut buf, &mut depth, px, &built.mesh, &project, fwd, BLADE_TINT);
                for c in &built.cards {
                    raster_mesh(&mut buf, &mut depth, px, &c.mesh, &project, fwd, CARD_TINT);
                }
                image::RgbaImage::from_raw(px as u32, px as u32, buf)
                    .expect("size")
                    .save(dir.join(format!("crown_{}.png", t.id)))
                    .expect("write png");
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

    /// ALPHA-TEST LAYERS one canopy pixel may pay, at most.
    ///
    /// Every card covers only `coverage` of its own area, so a pixel looking
    /// into a crown of leaf area index L passes through L / coverage cards
    /// before it is opaque - and each of those is a fetch, an alpha test and a
    /// discard at full canopy resolution. Card COUNT cancels out of that ratio
    /// entirely, so the only two levers are the sprite's real density and the
    /// species' target LAI.
    ///
    /// 8 is a RATCHET, not a taste: the shipped registry runs 3.8 (acacia),
    /// 4.8 (birch), 5.9 (sakura), 7.2 (pine), 7.3 (momiji), 7.5 (fir) and 7.8
    /// (oak), so this admits everything that ships today and nothing worse.
    /// Lower it as species improve; never raise it to make a number fit.
    const MAX_CANOPY_OVERDRAW: f32 = 8.0;

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
    ///
    /// ── AND THE OTHER DIRECTION (v0.1107) ────────────────────────────────
    ///
    /// This test COMPUTED and PRINTED `overdraw` from the day the layer landed
    /// and asserted nothing about it, which is the "check whose evidence is its
    /// own setup" pattern: a number nobody can fail is a number nobody reads.
    /// It is now a gate, and it is the one that catches the opposite mistake
    /// from a bare crown - a crown that reaches its LAI by stacking cards that
    /// are each mostly sky. Fir's first pass, with a broadleaf's `coverage`
    /// lent to it, sat at 19.1 layers and pine at 27.4; both would have shipped
    /// green and cost 2.5-3.5x the fill rate of every broadleaf beside them.
    #[test]
    fn cluster_cards_reach_target_lai_and_fit_the_budget() {
        let r = registry();
        let (mut seen, mut skipped) = (0usize, Vec::new());
        for t in r.trees.iter().map(as_procedural) {
            let Some(cd) = t.clusters.clone() else {
                skipped.push(t.id.clone());
                continue;
            };
            seen += 1;
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
                let overdraw = lai / mean_cov.max(0.01);
                eprintln!(
                    "[lai] {:>7} v{v}: crown r {:.2} m, spread {:.2} m ({:.1} m2), {n_cards} cards, \
                     {:.1} m2 leaf, LAI {lai:.2} (target {:.2}), overdraw {overdraw:.1} layers, \
                     {total} tris ({card_tris} card)",
                    t.id,
                    crown.radius_m,
                    crown.spread_m,
                    crown.projected_area_m2(),
                    area,
                    cd.target_lai,
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
                assert!(
                    overdraw <= MAX_CANOPY_OVERDRAW,
                    "{} v{v}: a canopy pixel pays {overdraw:.1} alpha-test layers (LAI {lai:.2} \
                     over coverage {mean_cov:.3}), past the {MAX_CANOPY_OVERDRAW:.0} ceiling. \
                     Raise the layer's `coverage` - which means making the sprite genuinely \
                     denser and RE-MEASURING it, not writing a bigger number - or lower \
                     `target_lai`. Adding cards does not help: overdraw is LAI / coverage and \
                     card COUNT cancels out of it entirely",
                    t.id
                );
            }
        }
        assert_card_gate_coverage(
            "cluster_cards_reach_target_lai_and_fit_the_budget",
            seen,
            &skipped,
        );
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

    fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }

    /// THE BRANCH-THROUGH-CROWN GATE (v0.1090).
    ///
    /// The operator's v0.1088.4 close-up: "bare branch tubes spear through the
    /// blossom card masses and exit the far side". A terminal twig is an open,
    /// uncapped pipe whose LAST tube ring sits at `Twig::tube_end`; if the card
    /// sleeve stops short of that ring, the bare wood pokes out of the crown
    /// and the tree reads as scaffolding wearing decals rather than as one
    /// object.
    ///
    /// Measured exactly as the eye sees it: project everything onto the twig's
    /// own chord, take the outermost CARD material within one card side of that
    /// chord, and compare it with where the tube actually ends. A positive gap
    /// is exposed pipe, in metres.
    #[test]
    fn cluster_cards_envelop_every_twig_tip() {
        let r = registry();
        let (mut seen, mut skipped) = (0usize, Vec::new());
        for t in r.trees.iter().map(as_procedural) {
            if t.clusters.is_none() {
                skipped.push(t.id.clone());
                continue;
            }
            seen += 1;
            for v in 0..t.variants.max(1) {
                let seed = shipped_seed(v);
                let twigs = twigs_of(&t, seed);
                let built = build_tree_and_cards(&t, t.height_m, seed);
                let verts: Vec<[f32; 3]> = built
                    .cards
                    .iter()
                    .flat_map(|c| c.mesh.vertices.iter().map(|x| x.position))
                    .collect();
                assert!(!verts.is_empty(), "{} v{v}: no cards to envelop with", t.id);
                let side = built
                    .cards
                    .iter()
                    .map(|c| c.card_side_m)
                    .fold(0.0f32, f32::max);
                let mut worst = f32::MIN;
                let mut exposed = 0usize;
                let mut tips = 0usize;
                let mut fattest = 0.0f32;
                for w in twigs.iter().filter(|w| w.tip) {
                    tips += 1;
                    fattest = fattest.max(w.tip_r);
                    let a = norm(sub(w.end, w.from));
                    let te = dot3(sub(w.tube_end, w.from), a);
                    // Outermost card material lying on THIS twig's own sleeve.
                    // A sleeve card sits `CLUSTER_SLEEVE_OFFSET * side` off the
                    // axis and reaches `side / 2` sideways, so its corners live
                    // at ~0.58 side of perpendicular distance; anything beyond
                    // that belongs to a NEIGHBOURING twig and must not be
                    // allowed to certify this twig's tip as covered.
                    let mut cr = f32::MIN;
                    for p in &verts {
                        let rel = sub(*p, w.from);
                        let ax = dot3(rel, a);
                        let perp = (dot3(rel, rel) - ax * ax).max(0.0).sqrt();
                        if perp <= side * 0.6 {
                            cr = cr.max(ax);
                        }
                    }
                    let gap = te - cr;
                    if gap > 0.0 {
                        exposed += 1;
                    }
                    worst = worst.max(gap);
                }
                eprintln!(
                    "[tipcover] {:>7} v{v}: {exposed}/{tips} tips poke past their cards, worst \
                     {worst:+.3} m (card side {side:.3} m, fattest tip radius {fattest:.4} m)",
                    t.id
                );
                assert!(tips > 10, "{} v{v}: only {tips} terminal twigs", t.id);
                assert!(
                    worst <= 0.0,
                    "{} v{v}: {exposed} of {tips} terminal twigs end {worst:.3} m OUTSIDE their \
                     card cover - that is the bare pipe spearing through the blossom mass",
                    t.id
                );
                // A terminal shoot is a twig, not a pipe: a cherry's last-year
                // wood is 5-10 mm ACROSS. Anything fatter reads as plumbing no
                // matter how well the cards cover it.
                assert!(
                    fattest <= 0.010,
                    "{} v{v}: terminal twigs end at {:.1} mm radius ({:.0} mm across) - a real \
                     last-year shoot is ~5 mm across",
                    t.id,
                    fattest * 1000.0,
                    fattest * 2000.0
                );
            }
        }
        assert_card_gate_coverage("cluster_cards_envelop_every_twig_tip", seen, &skipped);
    }

    /// THE NEAR-BLADE SILHOUETTE GATE (v0.1090).
    ///
    /// With cards carrying the canopy, the geometric blade layer is a
    /// close-range parallax detail, and detail that pokes OUT of the card mass
    /// it is meant to sit inside is just raw triangles against the sky - the
    /// operator's second v0.1088.4 finding.
    ///
    /// The measure is LOCAL, not against the whole crown: a blade can be well
    /// inside the crown envelope and still stand a clear half-metre proud of
    /// the blossom mass on its own twig, which is exactly what the capture
    /// shows. A sleeve card sits `CLUSTER_SLEEVE_OFFSET * side` off the twig
    /// axis and reaches `side / 2` sideways, so the card mass around a twig
    /// ends at `(CLUSTER_SLEEVE_OFFSET + 0.5) * side`. Blades must live inside
    /// that.
    #[test]
    fn near_blades_stay_inside_the_card_shell() {
        const ORGAN_BIT_LEAF: u32 = 524_288;
        let r = registry();
        let (mut seen, mut skipped) = (0usize, Vec::new());
        for t in r.trees.iter().map(as_procedural) {
            if t.clusters.is_none() {
                skipped.push(t.id.clone());
                continue;
            }
            seen += 1;
            for v in 0..t.variants.max(1) {
                let seed = shipped_seed(v);
                let twigs = twigs_of(&t, seed);
                let built = build_tree_and_cards(&t, t.height_m, seed);
                let side = built
                    .cards
                    .iter()
                    .map(|c| c.card_side_m)
                    .fold(0.0f32, f32::max);
                let shell = side * (CLUSTER_SLEEVE_OFFSET + 0.5);
                let mut blade_max = 0.0f32;
                let mut blades = 0usize;
                let mut outside = 0usize;
                let m = &built.mesh;
                for f in m.indices.chunks(3) {
                    let uv = m.vertices[f[0] as usize].uv;
                    if (uv[0].max(0.0).round() as u32) & ORGAN_BIT_LEAF == 0 {
                        continue;
                    }
                    blades += 1;
                    let mut far = 0.0f32;
                    for i in f {
                        let p = m.vertices[*i as usize].position;
                        let d = twigs
                            .iter()
                            .map(|w| point_segment_dist(p, w.from, w.end))
                            .fold(f32::MAX, f32::min);
                        far = far.max(d);
                    }
                    blade_max = blade_max.max(far);
                    if far > shell {
                        outside += 1;
                    }
                }
                let frac = outside as f32 / blades.max(1) as f32;
                eprintln!(
                    "[blades] {:>7} v{v}: {blades} blade faces, {outside} ({:.1}%) outside the \
                     {shell:.2} m card mass, worst reach {blade_max:.2} m",
                    t.id,
                    frac * 100.0
                );
                assert!(blades > 20, "{} v{v}: only {blades} blade faces", t.id);
                assert!(
                    frac < 0.02,
                    "{} v{v}: {:.0}% of blade faces stand proud of the {shell:.2} m card mass \
                     (worst {blade_max:.2} m) - those are the raw triangles poking out of the \
                     crown",
                    t.id,
                    frac * 100.0
                );
                assert!(
                    blade_max <= shell * 1.25,
                    "{} v{v}: a blade reaches {blade_max:.2} m from its twig against a \
                     {shell:.2} m card mass",
                    t.id
                );
            }
        }
        assert_card_gate_coverage("near_blades_stay_inside_the_card_shell", seen, &skipped);
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
        let (mut seen, mut skipped) = (0usize, Vec::new());
        for t in r.trees.iter() {
            let Some(cd) = t.clusters.as_ref() else {
                skipped.push(t.id.clone());
                continue;
            };
            seen += 1;
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
        assert_card_gate_coverage("cluster_sprite_geometry_fits_its_card", seen, &skipped);
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
