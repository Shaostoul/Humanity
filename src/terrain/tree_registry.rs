//! The tree species list (`data/vegetation/trees.ron`) and the two tables the card atlas is
//! drawn from: which atlas tile each (species, variant) owns, and how each tile was framed.
//!
//! WHY IT LIVES IN TERRAIN (the crate split's rung 0, 2026-10-31,
//! docs/design/crate-split.md): this was the top of `renderer::tree_mesh`, and the planet's
//! patch builder (`terrain::planet_chunks`, the far cards and the near-model mirror) picks a
//! species, finds its tile and frames its card from it, so terrain reached up into the renderer
//! for plain data. It is plain data: the species rows, a pure tile function and a table the baker
//! fills. The renderer's tree builder and baker read it from here, beneath them both.

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
        // Disk first; the built-in copy says so in the log (a rig refuses a run
        // that served one, BUG-133).
        let why = match std::fs::read_to_string("data/vegetation/trees.ron") {
            Ok(t) => match TreeRegistry::from_ron(&t) {
                Ok(r) if !r.is_empty() => return r,
                Ok(_) => "data/vegetation/trees.ron lists no trees".to_string(),
                Err(e) => format!("data/vegetation/trees.ron does not parse ({e})"),
            },
            Err(e) => format!("data/vegetation/trees.ron could not be read ({e})"),
        };
        crate::embedded_data::note_builtin_copy("vegetation/trees.ron", why);
        TreeRegistry::from_ron(EMBEDDED_TREES).unwrap_or_default()
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
pub const ATLAS_TILES: u32 = ATLAS_COLS * ATLAS_ROWS;

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
                 ATLAS_COLS/ATLAS_ROWS in terrain/tree_registry.rs (and the matching shader decode).",
                ATLAS_COLS,
                ATLAS_ROWS,
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

// ── The card layers ───────────────────────────────────────────────────────

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

// ── The atlas grid ────────────────────────────────────────────────────────

/// Atlas geometry shared by the baker, `tile_of` above, and the type-12
/// sprite branch of `assets/shaders/pbr/90-fragment-main.wgsl`.
///
/// 6 columns x 8 rows of TILE_PX sprites = 48 slots for the 24 the shipped
/// registry uses (8 species x 3 variants), 1536 x 2048 (~12.6 MB). The
/// headroom is deliberate: `renderer/mod.rs` derives the texture size from
/// these and the shader decode hardcodes them, so a grid sized to exactly
/// today's registry would mean the next row somebody adds to trees.ron
/// silently overflows the atlas. A data file must not be able to break the
/// renderer. `tree_mesh::tests::atlas_tile_constants_match_the_shader` locks
/// these against the WGSL literals.
pub const ATLAS_COLS: u32 = 6;
pub const ATLAS_ROWS: u32 = 8;
