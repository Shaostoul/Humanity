//! Automated billboard sprite baker (v0.959, operator call 2026-07-25:
//! "bake our own... one less thing we have to manually make every time we
//! add a 3D model"). Renders a model's parts side-on into a transparent
//! RGBA sprite - the 1990s pre-render pipeline (Total Annihilation's
//! terrain trees, StarCraft/AoE units were 3D models rendered to 2D art),
//! but automated and in-engine, so ANY model (including mods) gets its
//! card sprite for free.
//!
//! v0.1083 (docs/design/vegetation-cards-every-species.md): EVERY species in
//! `data/vegetation/trees.ron` gets a card, not just the two photoscans.
//! Three things had to change for that:
//!
//! - The atlas grew from 3x2x512 (6 tiles, fir + pine) to 6x8x256 (48 slots
//!   for the 24 in use), and tile indices are now a pure function of the
//!   registry (`tree_mesh::tile_of`) instead of a hand-written data field.
//! - The baker learned the PACKED-COLOUR transport. Procedural species carry
//!   their colour per face in the UV channel (`plant_mesh`, material type 20),
//!   not in a texture, so a texture-only baker rendered all six of them as the
//!   same olive blob (the 1x1 grey-green fallback).
//! - A missing model no longer aborts the bake. The release bundle does not
//!   ship `assets/models/`, so fir and pine cannot be parsed there; before
//!   this, the FIRST empty part list returned Err and killed the whole atlas,
//!   which is why a downloaded build had no correct vegetation card anywhere.
//!   Failed tiles stay zero-filled (alpha 0 = the card discards) and the rest
//!   bake normally.
//!
//! Design notes:
//! - UNLIT albedo capture: the sprite stores base color + coverage alpha;
//!   lighting belongs to the consumer (the card shader lights cards like
//!   the terrain around them). Baking lighting in would freeze one sun
//!   angle into every card.
//! - Side-on orthographic view along -Z, model Y-up, framed on the parts'
//!   joint AABB with a 5% margin. Trees are yaw-randomized by the card
//!   stream, so one side view is the right budget; multi-angle imposters
//!   can layer on later if needed.
//! - The color target uses the SWAPCHAIN format (like the hi-res capture
//!   path) so `read_texture_to_png`'s bgra swizzle logic applies unchanged.
//! - NO GAMMA CORRECTION on the packed path. The target is sRGB, the packed
//!   decode yields LINEAR albedo, and the card samples the atlas with
//!   `textureSampleLevel`, which decodes sRGB back to linear - the round trip
//!   is exact only if nothing hand-encodes in between.

use super::mesh::Vertex;
use super::plant_mesh::{Organ, PlantMeshBuilder};
use super::tree_mesh::{self, CardFootprint, ClusterCards, ClusterLayer};
use super::Renderer;
use wgpu::util::DeviceExt;

/// Where a part's colour comes from. NOT inferrable from `texture.is_some()`:
/// "untextured" (falls back to a neutral grey-green) and "colour packed in the
/// UV channel" are genuinely different states, and a future untextured-but-not-
/// packed part must keep baking grey rather than reading its UVs as an integer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BakeMode {
    /// Sample the base-colour texture, alpha-test the foliage cutout.
    Textured,
    /// Decode the per-face colour packed into the UV channel by
    /// `terrain::planet_surface::pack_color_to_uv` (material type 20). Always
    /// covers, so no alpha test.
    PackedColor,
    /// A cluster card (material type 21): sample the layer's baked sprite with
    /// the AO-carrying UV decode, alpha-test the cutout. This is how a FAR
    /// card can show the same crown the near model does - without it the
    /// atlas tile for a clustered species would bake as a bare stick.
    ClusterCard,
}

/// One model part to bake (crown, trunk, ...): CPU geometry + optional
/// RGBA8 base-color texture. Parts render into the same sprite.
pub struct BakePart<'a> {
    pub vertices: &'a [Vertex],
    pub indices: &'a [u32],
    pub texture: Option<(&'a [u8], u32, u32)>,
    pub mode: BakeMode,
}

/// CPU geometry the caller already has. The atlas bake takes a map of these
/// instead of loading anything itself: the near-model loader parses exactly
/// the same 12 glTF files (and generates exactly the same 18 procedural
/// meshes) seconds earlier, and re-parsing them - each pine bark paying a
/// 2048->1024 texture downscale, 220-275 ms apiece - was the dominant cost of
/// the world-entry freeze this bake used to be blamed for.
///
/// Keys: a model-backed part is keyed by its RELATIVE PATH
/// (`assets/models/plants/<model>/<model>_v<N>[_bark].gltf`); a procedural
/// (species, variant) is keyed by `proc_key`. Anything absent is regenerated
/// (procedural) or skipped (model), so passing an empty map is always valid.
pub struct BakeCpuModel {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub texture: Option<(Vec<u8>, u32, u32)>,
}

/// Cache key for a procedural (species, variant). Same shape as the
/// near-model `decoration_mesh_cache` key so the two never drift.
pub fn proc_key(species_id: &str, variant: u32) -> String {
    format!("proc:{species_id}_v{variant}")
}

/// One baked cluster sprite, CPU-side, with its full mip chain.
///
/// The chain is built HERE rather than on the GPU because each level's alpha
/// has to be rescaled so its above-cutoff coverage matches level 0's. Plain
/// box filtering makes a cutout silhouette THIN OUT as it minifies (the
/// classic alpha-test coverage loss - it is why this baker already had to drop
/// the sprite cutoff from 0.5 to 0.3 and note that "the fir baked nearly
/// bare"), which on a canopy reads as the crown dissolving with distance.
#[derive(Clone, Debug)]
pub struct ClusterSpriteImage {
    /// Species id from `data/vegetation/trees.ron`.
    pub species: String,
    pub layer: ClusterLayer,
    /// Side of level 0, texels.
    pub size: u32,
    /// RGBA8 levels, biggest first. Level i is `size >> i` square.
    pub levels: Vec<Vec<u8>>,
    /// Fraction of level 0's texels above the alpha cutoff - the MEASURED
    /// number behind the species' `coverage` data field.
    pub coverage: f32,
}

/// What one atlas bake did, for the caller's log/telemetry.
#[derive(Clone, Debug, Default)]
pub struct BakeReport {
    /// Tiles that produced real pixels.
    pub tiles_baked: u32,
    /// (species, variant) pairs the registry asked for.
    pub stems: u32,
    /// Stems skipped because their model was missing or unparseable.
    pub missing_models: u32,
    /// Time inside the bake itself (mesh generation + GPU submit), ms.
    pub bake_ms: f32,
    /// Cluster sprites baked this pass, one per (species, layer). The caller
    /// uploads each as its OWN mipped texture and hands it to a material -
    /// they deliberately do NOT go in the 6x8 tree atlas, which cannot be
    /// mipped without tile-border bleed.
    pub cluster_sprites: Vec<ClusterSpriteImage>,
}

const BAKE_WGSL: &str = r#"
struct BakeUniform {
    mvp: mat4x4<f32>,
    // .x: 0 = sample the texture, 1 = decode the packed-UV colour.
    mode: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: BakeUniform;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

// CROWN-DEPTH LOCKSTEP BEGIN
// Multiplier a cluster card at crown depth `ao` applies to its sprite texel:
// an achromatic extinction toward the shaded core, plus the sun-leaf /
// shade-leaf CHROMATIC gradient (outer leaves smaller, thicker, yellower;
// inner ones larger, thinner, bluer - Boardman 1977, Annu. Rev. Plant Physiol.
// 28:355). The two tints are near luminance-neutral against the Rec.709
// weights, so the achromatic part of the gradient lives entirely in the
// (0.35 + 0.65 * ao) term.
//
// This block is DUPLICATED VERBATIM into billboard_bake::BAKE_WGSL and
// `crown_depth_shade_is_identical_in_the_bake_shader` fails if the two copies
// drift by so much as a space. The bake has no way to include a WGSL file, and
// a card that bakes with a different crown depth than it renders with is
// exactly the brightness step this whole block exists to remove.
fn crown_depth_shade(ao: f32) -> vec3<f32> {
    let tint = mix(
        vec3<f32>(0.88, 0.98, 1.10),
        vec3<f32>(1.10, 1.03, 0.84),
        ao);
    return tint * (0.35 + 0.65 * ao);
}
// CROWN-DEPTH LOCKSTEP END

@vertex
fn vs_main(@location(0) pos: vec3<f32>, @location(1) nrm: vec3<f32>, @location(2) uv: vec2<f32>) -> VsOut {
    var out: VsOut;
    out.pos = u.mvp * vec4<f32>(pos, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (u.mode.x > 1.5) {
        // CLUSTER CARD path. uv.x carries a 6-bit AO code in its integer
        // part (uv.x = 2*code + u01) - keep this decode identical to
        // tree_mesh::decode_card_uv and the type-21 branch of
        // 90-fragment-main.wgsl, or a card samples the wrong column.
        let code = floor(in.uv.x * 0.5);
        let cu = in.uv.x - 2.0 * code;
        let cc = textureSampleLevel(tex, samp, vec2<f32>(cu, in.uv.y), 0.0);
        if (cc.a < 0.5) {
            discard;
        }
        // CROWN DEPTH (v0.1110). This line is the whole reason the far-field
        // LOD read brighter than the tree it replaces. The code above was
        // decoded and then THROWN AWAY, so the baked sprite carried the
        // crown's albedo with none of the crown's own extinction, while the
        // type-21 branch that draws the SAME cards up close multiplies by
        // exactly this. Two representations of one tree, disagreeing about
        // roughly 1.5x of brightness, at a radius the eye can see as a circle.
        return vec4<f32>(cc.rgb * crown_depth_shade(code / 63.0), 1.0);
    }
    if (u.mode.x > 0.5) {
        // PACKED-COLOUR path (procedural species). Same three lines as the
        // type-20 decode in 90-fragment-main.wgsl: r and g are integers in
        // uv.x, b rides uv.y. All three corners of a flat-shaded face carry
        // the identical value, so interpolation is a constant.
        // No alpha test: a packed part has no alpha channel and always covers.
        let packed = u32(round(max(in.uv.x, 0.0)));
        return vec4<f32>(
            f32((packed >> 8u) & 255u) / 255.0,
            f32(packed & 255u) / 255.0,
            clamp(in.uv.y, 0.0, 1.0),
            1.0,
        );
    }
    // textureSampleLevel, not textureSample: the source textures are
    // mip_level_count 1 so the two are identical here, and the explicit level
    // keeps this legal inside the branch.
    let c = textureSampleLevel(tex, samp, in.uv, 0.0);
    // Foliage cutout, SOFTER than the model's 0.5 (v0.970 atlas polish):
    // a sprite texel covers many source texels once the tree is card-sized,
    // so thin needles that pass 0.5 up close alias away at sprite scale -
    // the fir baked nearly bare. 0.3 keeps sub-texel needle mass; the card
    // still alpha-tests at 0.5 against the SPRITE's alpha, which is 1.0
    // wherever anything drew, so silhouettes stay crisp.
    if (c.a < 0.3) {
        discard;
    }
    return vec4<f32>(c.rgb, 1.0);
}
"#;

/// Atlas geometry shared by the baker, `tree_mesh::tile_of`, and the type-12
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
pub const ATLAS_TILE_PX: u32 = 256;

// ── Cluster sprites (v0.1088) ────────────────────────────────────────────
//
// Each cluster sprite is its OWN mipped texture, NOT another atlas tile. Two
// reasons, both load-bearing:
//   - a 2D atlas cannot be mipped safely (filtering bleeds across tile
//     borders), and an unmipped cutout crawls the moment it minifies, which
//     is exactly where a forest gets looked at;
//   - converting the atlas to a texture_2d_array would be a binding-TYPE
//     change on a shared layout, i.e. the v0.1029-v0.1038 incident class.
// A dedicated texture through the per-material albedo slot changes no layout
// at all.

/// Side of a finished cluster sprite, texels.
///
/// 256 -> 512 (v0.1090). A card settles at 0.5-0.86 m on sakura, so 256 px is
/// 2.0-3.4 mm per texel: a 35 mm cherry blossom got 10-17 texels across and a
/// 90 mm leaf blade got 26-45, which is enough for a MASS and not enough for a
/// FLOWER. The operator's close-up is exactly the range where that shows. At
/// 512 the same card carries 1.0-1.7 mm texels, so a petal notch - the shape
/// cue that separates a cherry from every other white five-petalled flower -
/// survives the bake instead of being averaged away.
///
/// Memory is not the constraint: one sprite is 512*512*4 = 1 MB plus a third
/// for its mip chain, and the shipped registry bakes two of them.
pub const CLUSTER_SPRITE_PX: u32 = 512;

/// Side it is RENDERED at before the box downsample. The bake pipeline is
/// multisample count 1 and this engine has no MSAA anywhere, so the 4x
/// supersample is the only anti-aliasing a cutout silhouette is ever going to
/// get - and an alpha test amplifies whatever jaggedness survives.
///
/// This must stay an integer multiple of `CLUSTER_SPRITE_PX` (the downsample is
/// a box filter of factor `CLUSTER_BAKE_PX / CLUSTER_SPRITE_PX`), and the
/// multiple must stay 4 or the silhouette loses the only anti-aliasing it gets.
/// 2048 is a quarter of the 8192 minimum guaranteed `max_texture_dimension_2d`,
/// so no device-limit risk (the v0.782 incident class).
pub const CLUSTER_BAKE_PX: u32 = 2048;

/// Alpha at or above which a cluster texel counts as covered. Keep in lockstep
/// with the type-21 discard in `assets/shaders/pbr/90-fragment-main.wgsl` and
/// the cluster branch of BAKE_WGSL above.
pub const CLUSTER_ALPHA_CUTOFF: f32 = 0.5;

/// Smallest mip level built. Stopping at 4x4 is deliberate: below that a level
/// holds fewer texels than the coverage gate can meaningfully measure (a 2x2
/// level quantises coverage to 25% steps), and a 0.5 m card is ~4 px on screen
/// at the 120 m model/card handoff, so nothing smaller is ever sampled.
pub const CLUSTER_MIP_MIN_PX: u32 = 4;

/// Achromatic half of the crown-depth term a cluster card at depth `ao`
/// applies to its sprite texel. Rust twin of `crown_depth_shade` in
/// `assets/shaders/pbr/10-lighting-patterns.wgsl` (which also carries the
/// luminance-neutral sun-leaf/shade-leaf tint, hence "achromatic half").
///
/// Exists so the parity tests below can MEASURE what the near path does rather
/// than restate what it was meant to do.
pub fn crown_depth_extinction(ao: f32) -> f32 {
    0.35 + 0.65 * ao.clamp(0.0, 1.0)
}

/// Visible-area-weighted mean of `max(N . L, 0)` over a crown of isotropically
/// oriented leaves, as a function of `cos_psi = dot(L, V)`.
///
/// Rust twin of `canopy_ndl_mean` in
/// `assets/shaders/pbr/10-lighting-patterns.wgsl`. This is THE number that
/// makes a vegetation card agree with the 3D crown it replaces: a card's
/// shading normal is the radial up, so without it the card is lit as a plate
/// facing the sky, which is the brightest orientation available and nothing
/// like a canopy.
///
/// Derivation and the two tests that hold it honest live beside the WGSL copy.
pub fn canopy_ndl_mean(cos_psi: f32) -> f32 {
    let c = cos_psi.clamp(-1.0, 1.0);
    let psi = c.acos();
    let s = (1.0 - c * c).max(0.0).sqrt();
    (2.0 * s + (std::f32::consts::PI - 2.0 * psi) * c) / (3.0 * std::f32::consts::PI)
}

/// Base roughness of a cluster-card layer's material (v0.1109).
///
/// It was 0.9 for both layers, which is CHALK, and the type-21 branch never
/// overrode it - so every leaf in the forest was a pure matte diffuse surface
/// with no specular lobe worth the name. Measured leaf BRDFs put the adaxial
/// specular lobe at roughness ~0.20-0.40 with a 3-6% normal-incidence specular
/// that rises steeply toward grazing (Bousquet, Lacherade, Jacquemoud & Moya
/// 2005, "Leaf BRDF measurements and model for specular and diffuse components
/// differentiation", Remote Sensing of Environment 98:201-211). The Fresnel
/// side was already right - a dielectric here gets f0 = 0.04 and Schlick takes
/// it up at grazing - so the missing half was the LOBE WIDTH. That lobe is what
/// makes a side-lit or backlit crown come alive under a low sun.
///
/// The shader narrows this further toward the crown's sunlit SHELL and leaves
/// the shaded core near this value, because a shade leaf's cuticle really is
/// thinner and duller than a sun leaf's.
///
/// A PETAL is not a leaf: cherry petals are papery, not waxy, so the blossom
/// layer keeps a much broader lobe.
pub fn cluster_roughness(layer: ClusterLayer) -> f32 {
    match layer {
        ClusterLayer::Leaf => 0.62,
        ClusterLayer::Blossom => 0.88,
    }
}

/// Everything a bake needs that does NOT change per tile. Built once per atlas
/// bake instead of once per tile: the old code compiled the shader module,
/// bind-group layout, pipeline layout, render pipeline and sampler inside the
/// per-tile loop, which was 6 rebuilds and would have been 24.
struct BakeRig {
    bgl: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    /// 1x1 neutral grey-green. Bound wherever a part has no texture (including
    /// every packed-colour part, which never reads it).
    fallback_view: wgpu::TextureView,
}

/// Reusable render targets for one tile size.
struct BakeTargets {
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    depth_view: wgpu::TextureView,
}

impl Renderer {
    /// Bake `parts` into a `size` x `size` sprite and write it as a PNG
    /// (transparent background). Returns the sprite's world-space footprint
    /// so the consumer can size cards to match. Dev tool: reachable from the
    /// showcase IPC, and the way to eyeball tiles without spending a rig run.
    pub fn bake_billboard_to_png(
        &self,
        parts: &[BakePart<'_>],
        size: u32,
        path: &std::path::Path,
    ) -> Result<CardFootprint, String> {
        let (color, footprint) = self.bake_billboard_texture(parts, size)?;
        self.read_texture_to_png(&color, size, size, path)?;
        Ok(footprint)
    }

    /// Bake the whole tree-card atlas FROM THE REGISTRY (v0.1083).
    ///
    /// One tile per (species, variant) in `data/vegetation/trees.ron`, at the
    /// index `tree_mesh::tile_of` computes. Procedural species build their
    /// mesh here with the SAME seed and height the near-model builder uses
    /// (`t.height_m`, `v * 2_654_435_761`) - if those diverged, the card would
    /// be a different tree from the model it hands off to, which is the exact
    /// pop this rung exists to remove. Model-backed species come out of
    /// `models` (keyed by relative path); a stem whose files are absent is
    /// SKIPPED, leaving its tile zero-filled, and the rest of the atlas still
    /// lands. `parse_ms` is the caller's own glTF parse time, for the log line
    /// (it should be ~0 when the near-model loader already parsed them).
    ///
    /// `dump_dir`, when set, also writes each tile as a PNG - the eyeball
    /// surface for all 24 tiles without booting the rig.
    pub fn bake_tree_atlas_from_registry(
        &mut self,
        models: &std::collections::HashMap<String, BakeCpuModel>,
        parse_ms: f32,
        dump_dir: Option<&std::path::Path>,
    ) -> BakeReport {
        let t0 = std::time::Instant::now();
        // Cluster sprites FIRST: the atlas tiles for a clustered species are
        // baked WITH their cards, so the far card shows the same crown the
        // near model does. Without this the tile would bake as a bare stick
        // the moment the blade layer was thinned.
        let sprites = self.bake_cluster_sprites(dump_dir);
        let rig = self.bake_rig();
        let targets = self.bake_targets(ATLAS_TILE_PX);
        if let Some(dir) = dump_dir {
            let _ = std::fs::create_dir_all(dir);
        }
        let reg = tree_mesh::registry();
        let mut report = BakeReport::default();
        for (i, t) in reg.trees.iter().enumerate() {
            for v in 0..t.variants.max(1) {
                report.stems += 1;
                let Some(tile) = tree_mesh::tile_of(i, v) else {
                    log::error!("[Bake] {} v{v}: no atlas tile (registry overflows the atlas)", t.id);
                    continue;
                };
                // Procedural: reuse the near-model loader's CPU buffers when it
                // already generated this (species, variant) - it builds the
                // identical mesh moments earlier - otherwise generate here.
                //
                // The lookup is UNCONDITIONAL (v0.1101.2), not gated on
                // `is_procedural()`: a species can be procedural by DATA (empty
                // `model`) or procedural BY FALLBACK - its scan failed to load,
                // or the v0.1101 scan-stretch guard rejected it - and in the
                // fallback case the near loader has already parked a perfectly
                // good procedural mesh under this exact key while
                // `is_procedural()` still reads false. Gating on the data flag
                // meant fir and pine baked SIX EMPTY ATLAS TILES, so their
                // far-field cards drew nothing at all. `is_procedural()` still
                // gates whether we GENERATE a mesh below; it has no business
                // gating whether we look for one.
                let cached_proc = models.get(&proc_key(&t.id, v));
                let proc_mesh = (t.is_procedural() && cached_proc.is_none()).then(|| {
                    let mut b = PlantMeshBuilder::new();
                    tree_mesh::build_tree(&mut b, t, t.height_m, v.wrapping_mul(2_654_435_761));
                    b
                });
                // Cluster cards for this (species, variant). Regenerated here
                // rather than taken from `models`: the near-model loader
                // caches only the wood mesh, and both come from the same
                // deterministic `build_tree_and_cards`, so they agree.
                let cards: Vec<ClusterCards> = if t.clusters.is_some() {
                    tree_mesh::build_tree_and_cards(t, t.height_m, v.wrapping_mul(2_654_435_761))
                        .cards
                } else {
                    Vec::new()
                };
                // Model-backed: the stem plus its _bark pair, textured.
                let mut parts: Vec<BakePart<'_>> = match (cached_proc, &proc_mesh) {
                    (Some(cpu), _) => vec![BakePart {
                        vertices: &cpu.vertices,
                        indices: &cpu.indices,
                        texture: None,
                        mode: BakeMode::PackedColor,
                    }],
                    (None, Some(b)) => vec![BakePart {
                        vertices: &b.vertices,
                        indices: &b.indices,
                        texture: None,
                        mode: BakeMode::PackedColor,
                    }],
                    (None, None) => ["", "_bark"]
                        .iter()
                        .filter_map(|suffix| {
                            let rel = format!(
                                "assets/models/plants/{m}/{m}_v{}{suffix}.gltf",
                                v + 1,
                                m = t.model
                            );
                            models.get(&rel).map(|cpu| BakePart {
                                vertices: &cpu.vertices,
                                indices: &cpu.indices,
                                texture: cpu
                                    .texture
                                    .as_ref()
                                    .map(|(b, w, h)| (b.as_slice(), *w, *h)),
                                mode: BakeMode::Textured,
                            })
                        })
                        .collect(),
                };
                for c in &cards {
                    let Some(spr) = sprites
                        .iter()
                        .find(|s| s.species == t.id && s.layer == c.layer)
                    else {
                        continue;
                    };
                    parts.push(BakePart {
                        vertices: &c.mesh.vertices,
                        indices: &c.mesh.indices,
                        texture: Some((spr.levels[0].as_slice(), spr.size, spr.size)),
                        mode: BakeMode::ClusterCard,
                    });
                }
                if parts.is_empty() {
                    // Shipped builds have no assets/models/: log once per stem
                    // and leave the tile transparent. NOT an error - the six
                    // procedural species that DO ship still get real cards.
                    report.missing_models += 1;
                    log::info!(
                        "[Bake] {} v{v}: no model parts (assets/models/ absent?), tile {tile} left empty",
                        t.id
                    );
                    continue;
                }
                let Some(fp) = self.bake_parts_into(&rig, &targets, &parts) else {
                    log::warn!("[Bake] {} v{v}: parts contain no geometry, tile {tile} left empty", t.id);
                    continue;
                };
                self.copy_tile_into_atlas(&targets.color, tile);
                tree_mesh::set_card_footprint(tile, fp);
                report.tiles_baked += 1;
                if let Some(dir) = dump_dir {
                    let path = dir.join(format!("tile{tile:02}_{}_v{v}.png", t.id));
                    match self.read_texture_to_png(&targets.color, ATLAS_TILE_PX, ATLAS_TILE_PX, &path) {
                        Ok(()) => log::info!(
                            "[Bake] {} v{v} -> {} (frame {:.2} m, tree {:.2} m, base {:.3})",
                            t.id,
                            path.display(),
                            fp.frame_m,
                            fp.h_nominal_m,
                            fp.base_offset
                        ),
                        Err(e) => log::warn!("[Bake] {} v{v}: png dump failed: {e}", t.id),
                    }
                }
            }
        }
        report.cluster_sprites = sprites;
        report.bake_ms = t0.elapsed().as_secs_f32() * 1000.0;
        self.tree_atlas_ready = report.tiles_baked > 0;
        // The acceptance line for the bake-cost item: parse and bake are
        // SEPARATE numbers. The old combined timer started before the glTF
        // parse loop, so a correct implementation could never pass a gate
        // written against it.
        log::info!(
            "[Bake] parse {:.0} ms, bake {:.0} ms ({} stems, {} tiles baked, {} missing models, \
             atlas {}x{} @ {}px)",
            parse_ms,
            report.bake_ms,
            report.stems,
            report.tiles_baked,
            report.missing_models,
            ATLAS_COLS,
            ATLAS_ROWS,
            ATLAS_TILE_PX,
        );
        if !self.tree_atlas_ready {
            log::error!("[Bake] tree atlas EMPTY: no species produced a tile");
        }
        report
    }

    /// Render `parts` side-on into a fresh `size` x `size` texture
    /// (swapchain format, transparent clear, COPY_SRC). One-shot form for the
    /// PNG dump: builds its own pipeline, so it costs a shader compile.
    pub fn bake_billboard_texture(
        &self,
        parts: &[BakePart<'_>],
        size: u32,
    ) -> Result<(wgpu::Texture, CardFootprint), String> {
        if parts.is_empty() {
            return Err("no parts to bake".to_string());
        }
        let rig = self.bake_rig();
        let targets = self.bake_targets(size);
        let fp = self
            .bake_parts_into(&rig, &targets, parts)
            .ok_or_else(|| "parts contain no vertices".to_string())?;
        Ok((targets.color, fp))
    }

    /// Shader module + BGL + pipeline + sampler + the 1x1 fallback texture.
    /// Built ONCE per atlas bake and reused for every tile.
    fn bake_rig(&self) -> BakeRig {
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("billboard_bake"),
                source: wgpu::ShaderSource::Wgsl(BAKE_WGSL.into()),
            });
        let bgl = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("billboard_bake_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("billboard_bake_layout"),
                bind_group_layouts: &[&bgl],
                push_constant_ranges: &[],
            });
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("billboard_bake_pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex::layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.config.format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None, // foliage cards are double-sided
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview: None,
                cache: None,
            });
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("billboard_bake_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // Untextured parts sample this 1x1 neutral gray-green; packed-colour
        // parts never read it, but the layout still needs something bound.
        let fallback: [u8; 4] = [90, 110, 70, 255];
        let ftex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("billboard_bake_fallback"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &ftex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &fallback,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4), rows_per_image: Some(1) },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        let fallback_view = ftex.create_view(&Default::default());
        BakeRig { bgl, pipeline, sampler, fallback_view }
    }

    /// Colour + depth scratch targets, reused across every tile of one bake.
    fn bake_targets(&self, size: u32) -> BakeTargets {
        let color = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("billboard_bake_color"),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_view = color.create_view(&Default::default());
        let depth = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("billboard_bake_depth"),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&Default::default());
        BakeTargets { color, color_view, depth_view }
    }

    /// The core: frame `parts` on their joint AABB, draw them side-on into
    /// `targets`, and return the world-space footprint of the frame. None when
    /// the parts carry no vertices at all.
    fn bake_parts_into(
        &self,
        rig: &BakeRig,
        targets: &BakeTargets,
        parts: &[BakePart<'_>],
    ) -> Option<CardFootprint> {
        // Joint AABB over every part.
        let mut mn = [f32::MAX; 3];
        let mut mx = [f32::MIN; 3];
        for p in parts {
            for v in p.vertices {
                for i in 0..3 {
                    mn[i] = mn[i].min(v.position[i]);
                    mx[i] = mx[i].max(v.position[i]);
                }
            }
        }
        if mn[0] > mx[0] {
            return None;
        }
        // Frame: widest horizontal extent (the card stream yaws trees
        // randomly, so X vs Z is arbitrary - take the larger), full height.
        let w_m = (mx[0] - mn[0]).max(mx[2] - mn[2]).max(1e-3);
        let h_m = (mx[1] - mn[1]).max(1e-3);
        let half = 0.5 * w_m.max(h_m) * 1.05; // square frame, 5% margin
        let cx = 0.5 * (mn[0] + mx[0]);
        let cy = 0.5 * (mn[1] + mx[1]);
        let cz = 0.5 * (mn[2] + mx[2]);
        let eye = glam::Vec3::new(cx, cy, mx[2] + w_m + 1.0);
        let view = glam::Mat4::look_at_rh(eye, glam::Vec3::new(cx, cy, cz), glam::Vec3::Y);
        let depth_span = (mx[2] - mn[2]) + 2.0 * (w_m + 1.0);
        let proj = glam::Mat4::orthographic_rh(-half, half, -half, half, 0.01, depth_span.max(1.0));
        let mvp = proj * view;
        // The three numbers the card emitter needs, all of which this function
        // already computed and used to throw away (v0.1083, brief item 3b):
        // the frame side, the tree's own height, and where the tree's BASE
        // sits inside the frame as a fraction of it.
        let footprint = CardFootprint {
            frame_m: 2.0 * half,
            h_nominal_m: h_m,
            base_offset: ((mn[1] - (cy - half)) / (2.0 * half)).clamp(0.0, 1.0),
        };

        // Per-part GPU resources, then one render pass drawing them all.
        let mut draws = Vec::new();
        for p in parts {
            let vb = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("billboard_bake_vb"),
                    contents: bytemuck::cast_slice(p.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let ib = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("billboard_bake_ib"),
                    contents: bytemuck::cast_slice(p.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
            // mat4 + a mode vec4. Per PART, not per bake: a photoscan stem is
            // textured while a procedural tree is packed-colour, and one bake
            // can hold both.
            let mut ubytes: Vec<f32> = mvp.to_cols_array().to_vec();
            ubytes.extend_from_slice(&[
                match p.mode {
                    BakeMode::Textured => 0.0,
                    BakeMode::PackedColor => 1.0,
                    BakeMode::ClusterCard => 2.0,
                },
                0.0,
                0.0,
                0.0,
            ]);
            let ubuf = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("billboard_bake_uniform"),
                    contents: bytemuck::cast_slice(&ubytes),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
            // Packed parts never sample, so they bind the fallback view.
            let own_tex = match (p.mode, p.texture) {
                (BakeMode::Textured | BakeMode::ClusterCard, Some((bytes, tw, th))) => {
                    let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("billboard_bake_tex"),
                        size: wgpu::Extent3d { width: tw, height: th, depth_or_array_layers: 1 },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Rgba8UnormSrgb,
                        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                        view_formats: &[],
                    });
                    self.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: &tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        bytes,
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some(4 * tw),
                            rows_per_image: Some(th),
                        },
                        wgpu::Extent3d { width: tw, height: th, depth_or_array_layers: 1 },
                    );
                    let view = tex.create_view(&Default::default());
                    Some((tex, view))
                }
                _ => None,
            };
            let tv = match &own_tex {
                Some((_, view)) => view,
                None => &rig.fallback_view,
            };
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("billboard_bake_bg"),
                layout: &rig.bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(tv) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&rig.sampler) },
                ],
            });
            draws.push((vb, ib, p.indices.len() as u32, bg, own_tex, ubuf));
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("billboard_bake") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("billboard_bake_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Transparent background: alpha 0 everywhere the
                        // model does not cover. Also what clears the scratch
                        // between tiles now that it is reused.
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&rig.pipeline);
            for (vb, ib, n, bg, _tex, _ubuf) in &draws {
                pass.set_bind_group(0, bg, &[]);
                pass.set_vertex_buffer(0, vb.slice(..));
                pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..*n, 0, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
        Some(footprint)
    }

    /// Bake every cluster sprite the registry asks for (v0.1088).
    ///
    /// BAKE, DO NOT RASTERIZE. This function writes no procedural sprite
    /// painter: it hands `bake_parts_into` a sprig of the SAME real-scale
    /// blades the tree grows (or a segment of flowering twig with real 3.5 cm
    /// five-petalled flowers), and the existing orthographic path with its
    /// `wgpu::Color::TRANSPARENT` clear returns true leaf silhouettes and true
    /// overlap alpha for free. A sprite baked from anything OTHER than the
    /// species' own geometry could disagree with the near model it hands off
    /// to, which is the one defect this whole layer exists to avoid.
    pub fn bake_cluster_sprites(&self, dump_dir: Option<&std::path::Path>) -> Vec<ClusterSpriteImage> {
        let reg = tree_mesh::registry();
        if !reg.trees.iter().any(|t| t.clusters.is_some()) {
            return Vec::new();
        }
        let t0 = std::time::Instant::now();
        let rig = self.bake_rig();
        let targets = self.bake_targets(CLUSTER_BAKE_PX);
        let srgb = matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Rgba8UnormSrgb
        );
        let mut out: Vec<ClusterSpriteImage> = Vec::new();
        for t in reg.trees.iter() {
            let Some(cd) = t.clusters.as_ref() else { continue };
            for layer in ClusterLayer::ALL {
                // A species that never blooms emits no blossom CARD (the layer
                // coin in `emit_cluster_cards` needs `blossom_frac > 0`), so
                // baking its blossom sprite is a whole 2048px render, readback
                // and mip chain spent on a texture nothing will ever sample.
                // That was free while sakura was the only clustered species;
                // with four it would be four wasted bakes at world entry.
                if layer == ClusterLayer::Blossom && t.blossom_frac <= 0.0 {
                    continue;
                }
                let Some(mesh) = leaf_shape::sprite_geometry(t, layer, t.height_m) else {
                    continue;
                };
                if mesh.indices.is_empty() {
                    log::warn!("[Cluster] {} {}: sprite geometry is empty", t.id, layer.key());
                    continue;
                }
                let parts = vec![BakePart {
                    vertices: &mesh.vertices,
                    indices: &mesh.indices,
                    texture: None,
                    mode: BakeMode::PackedColor,
                }];
                if self.bake_parts_into(&rig, &targets, &parts).is_none() {
                    continue;
                }
                let Some(hi) =
                    self.read_texture_rgba8(&targets.color, CLUSTER_BAKE_PX, CLUSTER_BAKE_PX)
                else {
                    log::warn!("[Cluster] {} {}: sprite readback failed", t.id, layer.key());
                    continue;
                };
                let base = box_downsample_rgba(
                    &hi,
                    CLUSTER_BAKE_PX,
                    CLUSTER_BAKE_PX,
                    CLUSTER_BAKE_PX / CLUSTER_SPRITE_PX,
                    srgb,
                );
                let coverage = alpha_coverage(&base, CLUSTER_ALPHA_CUTOFF);
                let levels = build_mip_chain(&base, CLUSTER_SPRITE_PX, CLUSTER_ALPHA_CUTOFF, srgb);
                let want = cd.layer(layer).coverage;
                // The data field is what the LAI planner spends; the bake is
                // what actually covers. If they drift the crown misses its
                // target leaf area silently, so say so out loud.
                if (coverage - want).abs() > 0.15 {
                    log::warn!(
                        "[Cluster] {} {}: baked coverage {:.2} vs data {:.2} - the LAI fit is \
                         spending the wrong number; correct `coverage` in trees.ron",
                        t.id,
                        layer.key(),
                        coverage,
                        want
                    );
                }
                log::info!(
                    "[Cluster] {} {}: {} mips from {}px, coverage {:.3} (data {:.2})",
                    t.id,
                    layer.key(),
                    levels.len(),
                    CLUSTER_BAKE_PX,
                    coverage,
                    want
                );
                // DEV AID: every level as a PNG, so the sprite (and the
                // coverage rescale working down the chain) can be eyeballed
                // without booting the world. Same trigger as the atlas dump:
                // a showcase request of {"bake":"trees"}.
                if let Some(dir) = dump_dir {
                    let _ = std::fs::create_dir_all(dir);
                    let mut w = CLUSTER_SPRITE_PX;
                    for (li, lvl) in levels.iter().enumerate() {
                        let path = dir.join(format!("cluster_{}_{}_mip{li}.png", t.id, layer.key()));
                        match image::RgbaImage::from_raw(w, w, lvl.clone()) {
                            Some(img) => {
                                if let Err(e) = img.save(&path) {
                                    log::warn!("[Cluster] png dump failed: {e}");
                                }
                            }
                            None => log::warn!("[Cluster] level {li} is not {w}x{w}"),
                        }
                        w /= 2;
                    }
                }
                out.push(ClusterSpriteImage {
                    species: t.id.clone(),
                    layer,
                    size: CLUSTER_SPRITE_PX,
                    levels,
                    coverage,
                });
            }
        }
        log::info!(
            "[Cluster] {} sprites in {:.0} ms",
            out.len(),
            t0.elapsed().as_secs_f32() * 1000.0
        );
        out
    }

    /// The material one cluster-card layer draws with: material type 21, the
    /// layer's sprite in the per-material albedo slot, and - the point of this
    /// function - its FULL MIP CHAIN behind a trilinear sampler (v0.1090).
    ///
    /// `bake_cluster_sprites` has built that chain since v0.1088 and the only
    /// consumer uploaded `levels[0]` through `add_textured_material`, which
    /// takes ONE level and binds the shared `albedo_sampler` (mipmap filter
    /// Nearest, address mode Repeat in U - both right for equirect planet
    /// imagery, both wrong for an alpha-tested card sprite). So the whole
    /// coverage-preserving mip build, and the `CLUSTER_MIP_MIN_PX` floor, and
    /// the reason cluster sprites are separate textures instead of atlas tiles
    /// at all, were being thrown away at the upload: an unmipped cutout crawls
    /// the moment it minifies, which is exactly where a forest is looked at.
    ///
    /// Bindings are unchanged - level count and sampler are texture and
    /// bind-group STATE, not layout state, so this touches none of the three
    /// `create_bind_group` sites' entry counts (the v0.1029-v0.1038 incident
    /// class).
    ///
    /// The sampler is created per call rather than memoized on `Renderer`
    /// (that struct is not this module's to extend): the shipped registry asks
    /// for two cluster materials per session, so two sampler objects.
    pub fn cluster_sprite_material(&mut self, spr: &ClusterSpriteImage) -> usize {
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Cluster Sprite Sampler"),
            // CLAMP on both axes: a card's UV runs 0..1 and nothing tiles, so
            // repeating would let the linear filter pull the opposite edge in
            // at the border.
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            // wgpu requires all three filters Linear when anisotropy_clamp > 1,
            // and a real chain to filter between - which this has.
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        let refs: Vec<&[u8]> = spr.levels.iter().map(|l| l.as_slice()).collect();
        let bg = self.build_material_texture_bind_group(&refs, spr.size, spr.size, &sampler);
        let idx =
            self.add_material_full([1.0, 1.0, 1.0, 1.0], 0.0, cluster_roughness(spr.layer), 21.0, 0.0);
        self.materials[idx].albedo_bind_group = Some(bg);
        log::info!(
            "[Cluster] {} {}: material {idx} with {} mip levels from {}px",
            spr.species,
            spr.layer.key(),
            spr.levels.len(),
            spr.size
        );
        idx
    }

    /// Read a bake target back to CPU RGBA8, swizzling BGRA when the
    /// swapchain format demands it (same logic as `read_texture_to_png`, which
    /// writes a file instead of handing the pixels back).
    fn read_texture_rgba8(&self, texture: &wgpu::Texture, w: u32, h: u32) -> Option<Vec<u8>> {
        if w == 0 || h == 0 {
            return None;
        }
        let bytes_per_row = ((w * 4 + 255) / 256) * 256; // 256-byte row alignment
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cluster_sprite_readback"),
            size: (bytes_per_row * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cluster_sprite_readback"),
            });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        self.queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = self.device.poll(wgpu::Maintain::Wait);
        let data = slice.get_mapped_range();
        let bgra = matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h {
            let start = (row * bytes_per_row) as usize;
            let row_bytes = &data[start..start + (w * 4) as usize];
            if bgra {
                for px in row_bytes.chunks_exact(4) {
                    pixels.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
                }
            } else {
                pixels.extend_from_slice(row_bytes);
            }
        }
        drop(data);
        buffer.unmap();
        Some(pixels)
    }

    /// Copy the freshly rendered scratch tile into its slot of the persistent
    /// atlas. The atlas texture was created at init and is referenced by every
    /// group-3 bind group, so this is an in-place rewrite - no rebuilds.
    fn copy_tile_into_atlas(&self, tile_tex: &wgpu::Texture, tile: u32) {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("tree_atlas_copy"),
            });
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: tile_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.tree_atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: (tile % ATLAS_COLS) * ATLAS_TILE_PX,
                    y: (tile / ATLAS_COLS) * ATLAS_TILE_PX,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: ATLAS_TILE_PX,
                height: ATLAS_TILE_PX,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
    }
}

// ── Per-leaf colour variation (v0.1109) ──────────────────────────────────
//
// THE DEFECT, MEASURED. `cluster_oak_leaf.png` covers 130,407 texels and
// carries exactly ONE distinct RGB triple (112,158,101). `cluster_sakura_leaf
// .png` covers 98,094 and carries one. The cause is three lines deep and
// entirely mechanical: `leaf_shape::reshape_blades` stamped the invariant
// `TreeDef::leaf_color` onto every triangle of every leaf, the bake shader is
// UNLIT (it unpacks 8-bit RGB and returns it), and the type-21 card branch
// then multiplied that monochrome sprite by a per-card AO scalar that is
// CONSTANT across the card. So the only thing that varied anywhere in a
// rendered canopy was BRIGHTNESS. Hue-constant and value-shaded is the
// definition of cel shading, and it is precisely what the operator saw: "the
// look kinda flat. Just a single colour without any other details... The over
// simplification seems to make it look cartoony."
//
// THE REFERENCE. This repo's own CC0 photoscan of a real conifer twig
// (`assets/models/plants/pine_sapling_small/textures/
// pine_sapling_small_twig_diff_a_1k.png`) measures 16.9 degrees of hue SD and
// 0.109 of saturation SD over its covered texels. Our rendered canopy measured
// 5.0 degrees of hue SD, and all 5 of those came from the sun-versus-sky light
// COLOUR, because the albedo underneath was a single number.
//
// WHY A REAL CROWN IS NOT ONE COLOUR. None of this is decoration; every term
// below is a named physiological cause, which is also why the spread is a
// measurement rather than a taste knob:
//   - LEAF AGE. A new flush is thinner, yellower and lower in chlorophyll per
//     unit area than the mature blade next to it, and a crown carries several
//     cohorts at once (an evergreen conifer holds 3-7 needle years together).
//   - NITROGEN STATUS and within-crown self-shading set chlorophyll density,
//     which moves VALUE and SATURATION much further than it moves hue.
//   - ANTHOCYANIN in new growth pulls a minority of blades red-purple.
//   - SENESCENCE. Chlorophyll degrades well before the carotenoids do, so a
//     senescing leaf swings hard toward straw and ochre. A real canopy carries
//     a few percent of these in EVERY season, not only in autumn.
//   - HERBIVORY and necrosis leave dead-tissue margins on the same scale.
//   - THE UNDERSIDE. A leaf's abaxial face is paler and less saturated than
//     its adaxial face - dramatically so where stomatal wax bands it (Abies) -
//     and a ball of randomly oriented leaves shows the viewer its underside
//     about 60% of the time (this engine's own leaf-angle distribution was
//     measured against a spherical reference at 61%, v0.1086).
//
// HOW THE JITTER IS APPLIED, and the one trap. Rotate the HUE in a chroma
// plane and SCALE saturation and value. Do NOT jitter R, G and B
// independently: that is a random walk toward the achromatic axis and it
// desaturates the whole canopy toward grey, which is the opposite of the
// defect being fixed. The rotation happens in the sRGB-encoded domain because
// that is the domain the 16.9-degree reference was measured in.
//
// MEAN-PRESERVING, on purpose, in the PERCEPTUAL domain. The abaxial/adaxial
// split derives the adaxial factor FROM the abaxial one so the population's
// mean sRGB value and saturation land back on the species' authored
// `leaf_color`. A row in trees.ron therefore still means what it says - it
// states the crown's mean colour and this spreads a population around it -
// instead of every species silently drifting.
//
// It is NOT mean-preserving radiometrically, and that is a fact about maths
// rather than a bug: sRGB decode is convex, so a spread that is symmetric in
// the encoded domain has a mean LINEAR luminance above the authored colour's
// (Jensen). Measured at +13.5% on the default spread, bounded at +20% by
// `per_leaf_colour_is_deterministic_and_mean_preserving`. A real canopy of
// mixed sun and shade leaves does reflect more than a uniform canopy at its
// mean colour, so the direction is right; the bound is there so it can never
// quietly grow into a re-lit forest.
//
// WHERE THE SPREAD IS PER SPECIES. The five numbers below are measurements of
// a plant (an evergreen turns over a twentieth of its needles a year where a
// deciduous crown turns over all of them; a birch's underside contrast is not
// an oak's), so they are DATA - `leaf_*` fields on the species' row, parsed
// here the same way `leaf_shape::registry` parses the silhouette fields. The
// two residual-noise SDs are a general physiological spread rather than a
// species fact, so they are constants here with their reasoning attached.
pub mod leaf_colour {
    /// Residual chlorophyll-density noise, as a FRACTION of the leaf's own
    /// saturation and value. This is the "no two leaves on one shoot have the
    /// same nitrogen status or the same self-shading history" term; it is
    /// deliberately smaller than the abaxial split, because the split is a
    /// structural fact about which face you are looking at and this is a
    /// gradient within either face.
    const SAT_NOISE_SD: f32 = 0.16;
    const VAL_NOISE_SD: f32 = 0.12;

    /// Where a senescing leaf's hue is heading, degrees: straw to ochre.
    /// Carotenoid + tannin colour, once the chlorophyll masking it is gone.
    const SENESCENT_HUE_DEG: (f32, f32) = (40.0, 58.0);

    /// The hue band that CARRIES chlorophyll, degrees. Senescence is defined
    /// as chlorophyll loss, so it may only act on tissue that has some: this
    /// gate is what keeps a cherry PETAL (hue ~340, and routed through the
    /// same blade code by `tree_mesh::leaf_cluster`) from turning brown.
    const CHLOROPHYLL_HUE_DEG: (f32, f32) = (55.0, 200.0);

    /// One species' leaf-colour SPREAD, as its row in trees.ron states it.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct LeafVariation {
        /// Standard deviation of the per-leaf hue rotation, degrees.
        pub hue_sd_deg: f32,
        /// Fraction of leaves far enough into senescence to read as straw.
        pub senescent_frac: f32,
        /// Fraction of leaves presenting their paler ABAXIAL face.
        pub underside_frac: f32,
        /// Value multiplier on that face (>1: the underside is lighter).
        pub underside_pale: f32,
        /// Saturation multiplier on it (<1: the underside is greyer).
        pub underside_desat: f32,
    }

    fn d_hue_sd() -> f32 {
        14.0
    }
    fn d_senescent() -> f32 {
        0.045
    }
    fn d_under_frac() -> f32 {
        0.60
    }
    fn d_under_pale() -> f32 {
        1.14
    }
    fn d_under_desat() -> f32 {
        0.78
    }

    impl Default for LeafVariation {
        fn default() -> Self {
            LeafVariation {
                hue_sd_deg: d_hue_sd(),
                senescent_frac: d_senescent(),
                underside_frac: d_under_frac(),
                underside_pale: d_under_pale(),
                underside_desat: d_under_desat(),
            }
        }
    }

    /// A species row, seen through the four fields this module cares about.
    /// Serde ignores everything else on the row, exactly as
    /// `leaf_shape::LeafSilhouette` does.
    #[derive(serde::Deserialize)]
    struct VarRow {
        id: String,
        #[serde(default = "d_hue_sd")]
        leaf_hue_sd_deg: f32,
        #[serde(default = "d_senescent")]
        leaf_senescent_frac: f32,
        #[serde(default = "d_under_frac")]
        leaf_underside_frac: f32,
        #[serde(default = "d_under_pale")]
        leaf_underside_pale: f32,
        #[serde(default = "d_under_desat")]
        leaf_underside_desat: f32,
    }

    #[derive(serde::Deserialize)]
    struct VarRegistry {
        trees: Vec<VarRow>,
    }

    fn registry() -> &'static Vec<VarRow> {
        static REG: std::sync::OnceLock<Vec<VarRow>> = std::sync::OnceLock::new();
        REG.get_or_init(|| {
            let parse = |t: &str| ron::from_str::<VarRegistry>(t).ok().map(|r| r.trees);
            std::fs::read_to_string("data/vegetation/trees.ron")
                .ok()
                .and_then(|t| parse(&t))
                .filter(|v| !v.is_empty())
                .or_else(|| parse(super::leaf_shape::EMBEDDED_TREES))
                .unwrap_or_default()
        })
    }

    /// This species' colour spread, or the temperate-broadleaf default when
    /// its row states none. Every field is clamped: a data typo must never be
    /// able to produce a canopy of solid magenta.
    pub fn of(species_id: &str) -> LeafVariation {
        registry()
            .iter()
            .find(|r| r.id == species_id)
            .map(|r| LeafVariation {
                hue_sd_deg: r.leaf_hue_sd_deg.clamp(0.0, 60.0),
                senescent_frac: r.leaf_senescent_frac.clamp(0.0, 0.60),
                underside_frac: r.leaf_underside_frac.clamp(0.0, 0.90),
                underside_pale: r.leaf_underside_pale.clamp(0.40, 2.50),
                underside_desat: r.leaf_underside_desat.clamp(0.10, 2.00),
            })
            .unwrap_or_default()
    }

    // ── Deterministic per-leaf randomness ────────────────────────────────
    //
    // A HASH, never a draw from the generator that built the scatter. The
    // scatter's `Rng` stream is measured, tuned and gated upstream (sprite
    // coverage, the LAI fit, the triangle budget all read off it), so taking
    // even one extra value from it would move geometry that has nothing to do
    // with colour. Hashing an index or a position keeps the mesh byte for byte
    // what it was and still gives every leaf its own colour.

    /// splitmix64. Full avalanche on a counter-like input, which is what an
    /// index or a quantised coordinate is.
    fn mix64(z: u64) -> u64 {
        let mut x = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
        x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^ (x >> 31)
    }

    /// A stream of uniforms from one key.
    struct Draw(u64);

    impl Draw {
        fn u01(&mut self) -> f32 {
            self.0 = mix64(self.0);
            (self.0 >> 40) as f32 / (1u64 << 24) as f32
        }

        /// Irwin-Hall(3) scaled to unit SD. Bounded at +-3 sigma, which a
        /// Box-Muller normal is not - and an unbounded tail on a hue rotation
        /// is a single lurid leaf somewhere in the forest.
        fn gauss(&mut self) -> f32 {
            (self.u01() + self.u01() + self.u01() - 1.5) * 2.0
        }
    }

    /// A stable key for a leaf that has no index of its own: its position,
    /// quantised to a tenth of a millimetre so the same leaf hashes the same
    /// way on every rebuild.
    pub fn key_at(p: [f32; 3], salt: u64) -> u64 {
        let q = |v: f32| (v * 10_000.0) as i64 as u64;
        mix64(q(p[0]) ^ mix64(q(p[1]) ^ mix64(q(p[2]).wrapping_add(salt))))
    }

    // ── Colour space ─────────────────────────────────────────────────────

    fn to_srgb(v: f32) -> f32 {
        if v <= 0.003_130_8 {
            v * 12.92
        } else {
            1.055 * v.max(0.0).powf(1.0 / 2.4) - 0.055
        }
    }

    fn to_linear(v: f32) -> f32 {
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }

    /// (hue degrees, saturation, value) of an RGB triple, in whatever domain
    /// the triple is already in. Hue is undefined at zero chroma; it is
    /// reported as 0 there and the caller is expected to ignore it (`stats`
    /// does, via a saturation floor).
    pub fn hsv(rgb: [f32; 3]) -> [f32; 3] {
        let mx = rgb[0].max(rgb[1]).max(rgb[2]);
        let mn = rgb[0].min(rgb[1]).min(rgb[2]);
        let c = mx - mn;
        if c <= 1e-6 || mx <= 1e-6 {
            return [0.0, 0.0, mx];
        }
        let h = if mx == rgb[0] {
            60.0 * (((rgb[1] - rgb[2]) / c) % 6.0)
        } else if mx == rgb[1] {
            60.0 * ((rgb[2] - rgb[0]) / c + 2.0)
        } else {
            60.0 * ((rgb[0] - rgb[1]) / c + 4.0)
        };
        [h.rem_euclid(360.0), c / mx, mx]
    }

    /// (hue, saturation, value) of a LINEAR RGB triple, measured in the
    /// sRGB-encoded domain - which is the domain `jitter` works in, the domain
    /// the photoscan reference was measured in, and therefore the only domain
    /// in which a statement about this model's spread or its mean means
    /// anything. Handed out because the gates need exactly this instrument.
    pub fn srgb_hsv(linear: [f32; 3]) -> [f32; 3] {
        hsv([to_srgb(linear[0]), to_srgb(linear[1]), to_srgb(linear[2])])
    }

    /// Inverse of `hsv`.
    pub fn from_hsv(hsv: [f32; 3]) -> [f32; 3] {
        let (h, s, v) = (hsv[0].rem_euclid(360.0), hsv[1].clamp(0.0, 1.0), hsv[2]);
        let c = v * s;
        let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
        let m = v - c;
        let (r, g, b) = match (h / 60.0) as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        [r + m, g + m, b + m]
    }

    // ── The jitter itself ────────────────────────────────────────────────

    /// This leaf's colour, given its species' mean colour (LINEAR RGB, as
    /// `TreeDef::leaf_color` states it), its species' spread, and a key that
    /// identifies the leaf.
    ///
    /// Returns LINEAR RGB, ready for `pack_color_to_uv`.
    pub fn jitter(base_linear: [f32; 3], v: LeafVariation, key: u64) -> [f32; 3] {
        let mut d = Draw(mix64(key ^ 0x1EAF_C019_0000_0001));
        let mut c = hsv([
            to_srgb(base_linear[0]),
            to_srgb(base_linear[1]),
            to_srgb(base_linear[2]),
        ]);
        // (1) HUE, rotated in the chroma plane. Saturation and value are
        //     untouched by this step, which is the whole reason for working in
        //     HSV rather than nudging the channels.
        c[0] += v.hue_sd_deg * d.gauss();
        // (2) WHICH FACE. Mean-preserving: with a fraction p showing a factor
        //     k, the other 1-p carry (1 - p*k)/(1-p), so the population mean
        //     of the factor is exactly 1 and the species' authored colour
        //     stays the crown's mean.
        //     CLAMPED, because exact mean-preservation misbehaves in the
        //     corner: a strong factor on a large majority (fir's chalk-banded
        //     needle undersides, 1.36 on 60% of them) drives the complement to
        //     0.46, i.e. a minority of near-black needles. Bounding it trades
        //     a few percent of mean drift for a population that stays leaves.
        let p = v.underside_frac.clamp(0.0, 0.90);
        let compensate = |k: f32| ((1.0 - p * k) / (1.0 - p).max(1e-3)).clamp(0.60, 1.60);
        let (kv, ks) = if d.u01() < p {
            (v.underside_pale, v.underside_desat)
        } else {
            (compensate(v.underside_pale), compensate(v.underside_desat))
        };
        // (3) RESIDUAL NOISE on top of whichever face this is.
        c[1] *= ks * (1.0 + SAT_NOISE_SD * d.gauss());
        c[2] *= kv * (1.0 + VAL_NOISE_SD * d.gauss());
        // (4) THE SENESCENT MINORITY. Chlorophyll only, and PARTIAL: a leaf
        //     turns gradually, so the population runs from barely-yellowing to
        //     fully straw rather than being a switch.
        let chlorophyll =
            c[0] >= CHLOROPHYLL_HUE_DEG.0 && c[0] <= CHLOROPHYLL_HUE_DEG.1;
        if chlorophyll && d.u01() < v.senescent_frac {
            let target = SENESCENT_HUE_DEG.0
                + (SENESCENT_HUE_DEG.1 - SENESCENT_HUE_DEG.0) * d.u01();
            let t = 0.45 + 0.50 * d.u01();
            c[0] += (target - c[0]) * t;
            // Carotenoid yellow is a SATURATED colour, and dying tissue also
            // stops absorbing, so a straw leaf is both brighter and purer than
            // the green it came from.
            c[1] = (c[1] * (1.15 + 0.30 * d.u01())).min(1.0);
            c[2] = (c[2] * (1.10 + 0.25 * d.u01())).min(1.0);
        }
        let s = from_hsv([
            c[0].rem_euclid(360.0),
            c[1].clamp(0.02, 1.0),
            c[2].clamp(0.01, 1.0),
        ]);
        [to_linear(s[0]), to_linear(s[1]), to_linear(s[2])]
    }

    // ── Measuring what came out ──────────────────────────────────────────

    /// Colour statistics of an sRGB RGBA8 image over its COVERED texels.
    ///
    /// This is the gate's instrument and it is deliberately the same one used
    /// on the photoscan reference, so the two numbers are comparable rather
    /// than merely similar-sounding.
    #[derive(Debug, Clone, Copy, Default)]
    pub struct Stats {
        pub covered: usize,
        /// Distinct RGB triples among the covered texels. ONE, for both
        /// shipped broadleaf sprites, before this increment.
        pub distinct: usize,
        /// Texels that also cleared the chroma floor, i.e. the ones whose hue
        /// means anything.
        pub chromatic: usize,
        pub hue_mean_deg: f32,
        /// CIRCULAR standard deviation, degrees (hue is an angle).
        pub hue_sd_deg: f32,
        pub sat_mean: f32,
        pub sat_sd: f32,
        pub val_mean: f32,
        pub val_sd: f32,
    }

    /// Hue is meaningless on a near-grey texel, so those are counted as
    /// covered but excluded from the hue statistic.
    const CHROMA_FLOOR: f32 = 0.05;

    pub fn stats(rgba: &[u8], alpha_cutoff: f32) -> Stats {
        let cut = (alpha_cutoff.clamp(0.0, 1.0) * 255.0) as u16;
        let mut seen = std::collections::HashSet::new();
        let mut n = 0usize;
        let (mut sx, mut sy, mut nh) = (0.0f64, 0.0f64, 0usize);
        let (mut s1, mut s2, mut v1, mut v2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for px in rgba.chunks_exact(4) {
            if (px[3] as u16) < cut {
                continue;
            }
            n += 1;
            seen.insert((px[0] as u32) << 16 | (px[1] as u32) << 8 | px[2] as u32);
            let c = hsv([
                px[0] as f32 / 255.0,
                px[1] as f32 / 255.0,
                px[2] as f32 / 255.0,
            ]);
            s1 += c[1] as f64;
            s2 += (c[1] * c[1]) as f64;
            v1 += c[2] as f64;
            v2 += (c[2] * c[2]) as f64;
            if c[1] >= CHROMA_FLOOR {
                let r = (c[0] as f64).to_radians();
                sx += r.cos();
                sy += r.sin();
                nh += 1;
            }
        }
        if n == 0 {
            return Stats::default();
        }
        let fn_ = n as f64;
        let sat_mean = s1 / fn_;
        let val_mean = v1 / fn_;
        let (hue_mean, hue_sd) = if nh > 0 {
            let (mx, my) = (sx / nh as f64, sy / nh as f64);
            let r = (mx * mx + my * my).sqrt().clamp(1e-9, 1.0);
            (
                my.atan2(mx).to_degrees().rem_euclid(360.0),
                (-2.0 * r.ln()).sqrt().to_degrees(),
            )
        } else {
            (0.0, 0.0)
        };
        Stats {
            covered: n,
            distinct: seen.len(),
            chromatic: nh,
            hue_mean_deg: hue_mean as f32,
            hue_sd_deg: hue_sd as f32,
            sat_mean: sat_mean as f32,
            sat_sd: (s2 / fn_ - sat_mean * sat_mean).max(0.0).sqrt() as f32,
            val_mean: val_mean as f32,
            val_sd: (v2 / fn_ - val_mean * val_mean).max(0.0).sqrt() as f32,
        }
    }
}

// ── Per-species leaf silhouettes (v0.1100) ───────────────────────────────
//
// THE DEFECT. Every foliage face on every procedural species is one isoceles
// triangle (`tree_mesh::blade`). At 0.09-0.20 m that was defensible while the
// blades WERE the canopy: the count read, not the outline. It stopped being
// defensible the moment cluster cards took over the canopy, because a card
// samples a BAKED SPRITE, and a sprite is a texture - the triangles in it are
// resolved at 1.0-1.7 mm per texel and they read, unmistakably, as triangles.
// The operator's word for it was "little triangle leaves".
//
// WHERE THE FIX BELONGS. In the sprite, not in the runtime mesh, and that is
// not a compromise - it is what baking is FOR. A 7-lobed maple leaf costs
// ~160 triangles. Drawn per frame on 256 near instances that is unaffordable
// and always will be; rasterized ONCE into a 512 px texture that every card in
// the forest then samples, it costs nothing per frame at all. So the runtime
// blade layer keeps its cheap deltoid proxy (it is a sub-metre parallax detail
// living INSIDE the card mass, never the silhouette - see
// `tree_mesh::near_blade_clump_k`), and the silhouette a player actually sees
// is stamped here.
//
// HOW A SPECIES SAYS WHAT ITS LEAF LOOKS LIKE. Through the `leaf_*` fields on
// its row in `data/vegetation/trees.ron`, in the terms a field guide uses:
// where the blade is widest, how drawn out the tip is, how many lobes, how
// deep the cuts, how many marginal teeth. The FAMILIES here are algorithms and
// therefore code; the assignment of a family and its numbers to a plant is a
// per-species measurement and therefore data (`docs/design/infinite-of-x.md`).
// Adding an eighth species with a new leaf is a row, not a patch.
pub mod leaf_shape {
    use super::{Organ, PlantMeshBuilder};
    use super::{tree_mesh, ClusterLayer};

    /// Outline family. The list is closed on purpose: each of these is a
    /// distinct CONSTRUCTION (one strip, several strips radiating, strips of
    /// strips), not a parameter setting of the others.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum LeafFamily {
        /// The plain isoceles blade every species drew before this. Kept as
        /// the default so a species row with no `leaf_shape` is unchanged.
        Deltoid,
        /// One simple blade with an entire (smooth) margin.
        Ovate,
        /// One simple blade with a doubly toothed margin.
        SerrateOvate,
        /// One blade cut into lobe PAIRS either side of the midrib (oak).
        PinnateLobed,
        /// Several lobes radiating from the petiole (maple).
        Palmate,
        /// A frond: pinna pairs along a rachis, each carrying leaflet pairs
        /// (acacia and the other mimosoid legumes).
        Bipinnate,
        /// A CONIFER SHOOT, two-ranked: needles set pectinately either side of
        /// one twig axis, all in one plane, so the spray reads flat and
        /// comb-like. Abies (fir), Taxus, Tsuga.
        NeedleFlatRank,
        /// A CONIFER SHOOT, radial: needles bundled in fascicles of 2-5 borne
        /// in a spiral all round the twig, so the shoot reads tufted and
        /// bottle-brushy rather than flat. Pinus.
        NeedleFascicle,
    }

    impl LeafFamily {
        pub fn parse(s: &str) -> LeafFamily {
            match s {
                "ovate" => LeafFamily::Ovate,
                "serrate-ovate" => LeafFamily::SerrateOvate,
                "pinnate-lobed" => LeafFamily::PinnateLobed,
                "palmate" => LeafFamily::Palmate,
                "bipinnate" => LeafFamily::Bipinnate,
                "needle-flat-rank" => LeafFamily::NeedleFlatRank,
                "needle-fascicle" => LeafFamily::NeedleFascicle,
                // Unknown falls back rather than failing, the same way
                // `TreeDef::form` does: a typo in a data file must never be
                // able to stop the forest growing.
                _ => LeafFamily::Deltoid,
            }
        }

        pub fn key(self) -> &'static str {
            match self {
                LeafFamily::Deltoid => "deltoid",
                LeafFamily::Ovate => "ovate",
                LeafFamily::SerrateOvate => "serrate-ovate",
                LeafFamily::PinnateLobed => "pinnate-lobed",
                LeafFamily::Palmate => "palmate",
                LeafFamily::Bipinnate => "bipinnate",
                LeafFamily::NeedleFlatRank => "needle-flat-rank",
                LeafFamily::NeedleFascicle => "needle-fascicle",
            }
        }

        /// True for the families whose drawn element is a whole SHOOT rather
        /// than one blade.
        ///
        /// This is the distinction that makes the conifers a different kind of
        /// thing rather than a broadleaf with smaller leaves: a broadleaf's
        /// element is one leaf and its silhouette is that leaf's margin, while
        /// a conifer's element is a needled twig and its silhouette is the
        /// ARRANGEMENT of a few hundred needles on it. Everything that reads
        /// `leaf_needle_*` is gated on this.
        pub fn is_needle_shoot(self) -> bool {
            matches!(self, LeafFamily::NeedleFlatRank | LeafFamily::NeedleFascicle)
        }
    }

    /// One species' leaf, as its row in `data/vegetation/trees.ron` states it.
    ///
    /// Every field carries a serde default, so a row states only what its
    /// plant actually has: an oak declares lobes and no teeth, a birch
    /// declares teeth and no lobes.
    #[derive(Clone, Debug, serde::Deserialize)]
    pub struct LeafSilhouette {
        pub id: String,
        #[serde(default)]
        pub leaf_shape: String,
        /// Leaf WIDTH over petiole-to-tip LENGTH. 0 keeps whatever width the
        /// mesh builder handed the blade (which is what Deltoid wants).
        #[serde(default)]
        pub leaf_aspect: f32,
        /// Where along the midrib the blade is widest. Below 0.5 is ovate,
        /// above is obovate. The single strongest silhouette cue there is.
        #[serde(default = "default_widest")]
        pub leaf_widest_frac: f32,
        /// 0 = a blunt or rounded tip, 1 = a long acuminate drip tip.
        #[serde(default = "default_tip")]
        pub leaf_tip_frac: f32,
        /// Palmate: lobe COUNT. Pinnate-lobed: lobe PAIRS. Bipinnate: pinna
        /// PAIRS along the rachis.
        #[serde(default)]
        pub leaf_lobes: u32,
        /// How far the cuts between lobes run toward the midrib (or, palmate,
        /// back toward the petiole), 0..1.
        #[serde(default)]
        pub leaf_sinus_frac: f32,
        /// Marginal teeth per side. 0 is an entire margin.
        #[serde(default)]
        pub leaf_teeth: u32,
        /// Bipinnate: leaflet PAIRS carried on one pinna. Needle-fascicle:
        /// needles bundled in ONE fascicle (2 on a Scots pine, 3 on a
        /// ponderosa, 5 on a white pine).
        #[serde(default)]
        pub leaf_leaflets: u32,
        /// Needle families: one NEEDLE's length as a fraction of the whole
        /// shoot's length. A fir needle is a fiftieth of its shoot; a pine
        /// needle is a fifth of it, and that ratio alone is most of why the two
        /// read as different plants at any range.
        #[serde(default)]
        pub leaf_needle_len_frac: f32,
        /// Needle families: a needle's WIDTH as a fraction of its own length.
        /// Fir needles are flat straps (~0.09); pine needles are round in
        /// section and far finer (~0.03).
        #[serde(default)]
        pub leaf_needle_width_frac: f32,
        /// Needle families: the angle a needle (flat-rank) or a whole fascicle
        /// (fascicle) leaves the shoot at, DEGREES OFF THE SHOOT AXIS. 90 is
        /// square to the shoot; smaller rakes forward toward its tip. Abies
        /// needles stand nearly square, pine fascicles ascend steeply.
        #[serde(default)]
        pub leaf_needle_angle_deg: f32,
        /// Needle-fascicle only: TOTAL angular splay of the needles inside one
        /// fascicle, degrees. This is the shaving-brush spread that separates a
        /// bundle from a single thick needle.
        #[serde(default)]
        pub leaf_fascicle_spread_deg: f32,
    }

    fn default_widest() -> f32 {
        0.42
    }
    fn default_tip() -> f32 {
        0.35
    }

    impl LeafSilhouette {
        pub fn family(&self) -> LeafFamily {
            LeafFamily::parse(&self.leaf_shape)
        }

        /// A species that never declared a leaf: the pre-v0.1100 deltoid.
        pub fn deltoid(id: &str) -> LeafSilhouette {
            LeafSilhouette {
                id: id.to_string(),
                leaf_shape: String::new(),
                leaf_aspect: 0.0,
                leaf_widest_frac: default_widest(),
                leaf_tip_frac: default_tip(),
                leaf_lobes: 0,
                leaf_sinus_frac: 0.0,
                leaf_teeth: 0,
                leaf_leaflets: 0,
                leaf_needle_len_frac: 0.0,
                leaf_needle_width_frac: 0.0,
                leaf_needle_angle_deg: 0.0,
                leaf_fascicle_spread_deg: 0.0,
            }
        }

        /// Needle length in shoot-length units, floored so a needle family
        /// declared with no numbers still draws a plausible spray instead of a
        /// degenerate one. Same principle as `LeafFamily::parse` falling back
        /// to Deltoid: a data file must never be able to stop the forest
        /// growing.
        fn needle_len(&self) -> f32 {
            if self.leaf_needle_len_frac > 1e-4 {
                self.leaf_needle_len_frac.min(0.9)
            } else {
                0.06
            }
        }

        fn needle_halfwidth(&self) -> f32 {
            let w = if self.leaf_needle_width_frac > 1e-4 {
                self.leaf_needle_width_frac.clamp(0.005, 0.5)
            } else {
                0.06
            };
            0.5 * self.needle_len() * w
        }

        /// Radians off the shoot axis. Clamped short of 0 and 90 + a little:
        /// a needle exactly along the shoot has no silhouette at all, and one
        /// swept BACKWARD past the horizontal would put needle tissue behind
        /// the shoot's own attachment point.
        fn needle_angle(&self) -> f32 {
            let d = if self.leaf_needle_angle_deg > 1.0 {
                self.leaf_needle_angle_deg
            } else {
                80.0
            };
            d.clamp(12.0, 92.0).to_radians()
        }

        fn fascicle_spread(&self) -> f32 {
            let d = if self.leaf_fascicle_spread_deg > 0.5 {
                self.leaf_fascicle_spread_deg
            } else {
                20.0
            };
            d.clamp(2.0, 70.0).to_radians()
        }
    }

    /// The shipped rows, compiled in so a build with no `data/` still knows
    /// what a maple leaf looks like. Same file and same precedence as
    /// `tree_mesh::registry` (disk wins in a dev checkout).
    pub(super) const EMBEDDED_TREES: &str = include_str!("../../data/vegetation/trees.ron");

    #[derive(serde::Deserialize)]
    struct SilhouetteRegistry {
        trees: Vec<LeafSilhouette>,
    }

    /// Leaf silhouettes, keyed by species id.
    ///
    /// Parsed out of the SAME rows `tree_mesh::registry` reads, through a
    /// struct that names only the `leaf_*` fields (serde ignores the rest).
    /// It is a separate parse rather than a field on `TreeDef` because leaf
    /// silhouette is a BAKE-TIME property and nothing outside this module can
    /// observe it: the runtime mesh, the card planner, the LAI fit and the
    /// species picker all work identically whether a leaf is a triangle or a
    /// maple. Keeping it out of `TreeDef` keeps a bake-only concern out of the
    /// hot registry every spawn cell touches.
    pub fn registry() -> &'static Vec<LeafSilhouette> {
        static REG: std::sync::OnceLock<Vec<LeafSilhouette>> = std::sync::OnceLock::new();
        REG.get_or_init(|| {
            let parse = |t: &str| ron::from_str::<SilhouetteRegistry>(t).ok().map(|r| r.trees);
            std::fs::read_to_string("data/vegetation/trees.ron")
                .ok()
                .and_then(|t| parse(&t))
                .filter(|v| !v.is_empty())
                .or_else(|| parse(EMBEDDED_TREES))
                .unwrap_or_default()
        })
    }

    /// This species' leaf, or the plain deltoid when it never declared one.
    pub fn of(species_id: &str) -> LeafSilhouette {
        registry()
            .iter()
            .find(|s| s.id == species_id)
            .cloned()
            .unwrap_or_else(|| LeafSilhouette::deltoid(species_id))
    }

    // ── Construction ─────────────────────────────────────────────────────
    //
    // Everything below builds in LEAF SPACE: y runs 0 at the petiole to 1 at
    // the tip, x is across and spans exactly 1 (so the caller scales x by the
    // leaf's real width and y by its real length). Shapes are symmetric about
    // x = 0.

    /// A leaf is assembled out of exactly two primitives.
    #[derive(Clone, Debug)]
    pub enum LeafPart {
        /// A strip symmetric about a straight axis. A whole blade, a maple
        /// lobe, a pinna rachis and a single leaflet are all this.
        ///
        /// Triangulated as a STRIP between its two margins, never as a fan
        /// from a centroid: a lobed blade is concave, and a centroid fan would
        /// bridge straight across the sinuses and fill in exactly the cuts
        /// that make an oak an oak.
        Ribbon {
            at: [f32; 2],
            /// Unit axis direction.
            dir: [f32; 2],
            len: f32,
            /// `(t, halfwidth)` samples along the axis, t rising 0..1.
            spine: Vec<[f32; 2]>,
        },
        /// A polygon that is STAR-SHAPED about its first point, so fanning
        /// from that point is exact. Used for the connective centre of a
        /// palmate leaf, whose boundary scallops in and out between lobes.
        Poly(Vec<[f32; 2]>),
    }

    /// Marginal half-width profile of a simple blade at midrib fraction `t`.
    ///
    /// The envelope is `t^a (1-t)^b` normalised to peak at 1. That is not an
    /// arbitrary curve: `a/(a+b)` IS the position of the widest point, which
    /// is the number a field guide gives you (ovate, obovate, elliptic), so
    /// the data states `leaf_widest_frac` and this solves the exponents for
    /// it. `b` alone sets how drawn out the tip is.
    fn blade_halfwidth(s: &LeafSilhouette, t: f32) -> f32 {
        let peak = s.leaf_widest_frac.clamp(0.08, 0.92);
        // 0.6 = blunt/rounded, 2.2 = a long acuminate drip tip.
        let b = 0.6 + 1.6 * s.leaf_tip_frac.clamp(0.0, 1.0);
        let a = b * peak / (1.0 - peak);
        let norm = peak.powf(a) * (1.0 - peak).powf(b);
        let t = t.clamp(0.0, 1.0);
        let mut w = if norm > 1e-6 {
            t.powf(a) * (1.0 - t).powf(b) / norm
        } else {
            0.0
        };
        // LOBES: a cut that is deepest in each sinus and absent on each lobe
        // axis, phased so the blade starts NARROW at the petiole (a lobed leaf
        // has a cuneate base, not a shoulder).
        //
        // The cut is a RAISED cosine to a power, not a plain cosine, and the
        // power is the whole difference between an oak and a caltrop. A plain
        // cosine spends equal arc on lobe and sinus, so the lobes come out as
        // sharp symmetric spikes; an oak has BROAD ROUNDED lobes separated by
        // NARROW sinuses, which is a cut concentrated near the sinus centre.
        // Raising the cosine to 2.2 does exactly that concentration.
        if s.leaf_lobes > 0 && s.leaf_sinus_frac > 0.0 {
            let l = s.leaf_lobes as f32;
            let cut = s.leaf_sinus_frac.clamp(0.0, 0.95);
            let g = 0.5 + 0.5 * (std::f32::consts::TAU * l * t).cos();
            w *= 1.0 - cut * g.powf(2.2);
        }
        // TEETH: a sawtooth whose slow side climbs toward the tip and whose
        // fast side drops back, so every tooth POINTS AT THE TIP the way a
        // serrate margin does. A second, finer, shallower saw at 3x the
        // frequency is what "doubly serrate" means, and it is the margin
        // signature of a birch.
        if s.leaf_teeth > 0 {
            let n = s.leaf_teeth as f32;
            let saw = |f: f32| 1.0 - (f * t).fract();
            w *= 1.0 - 0.13 * saw(n) - 0.05 * saw(3.0 * n);
        }
        w.max(0.0)
    }

    /// Samples along one margin. Enough to resolve the finest feature the
    /// margin carries, and no more: every extra sample is two triangles on
    /// every leaf of every sprig of the sprite, and a sprite carries hundreds
    /// of leaves.
    ///
    /// Four per tooth and ten per lobe, both found by looking at the dumps
    /// rather than reasoned: at three per tooth a serrate margin came out as a
    /// visible staircase, and at six per lobe an oak's rounded lobes came out
    /// as angular spikes.
    fn margin_samples(s: &LeafSilhouette) -> usize {
        let by_teeth = if s.leaf_teeth > 0 { s.leaf_teeth as usize * 4 } else { 0 };
        let by_lobes = if s.leaf_lobes > 0 { s.leaf_lobes as usize * 10 } else { 0 };
        by_teeth.max(by_lobes).clamp(14, 56)
    }

    fn spine_of(s: &LeafSilhouette, n: usize, scale: f32) -> Vec<[f32; 2]> {
        (0..=n)
            .map(|i| {
                let t = i as f32 / n as f32;
                [t, blade_halfwidth(s, t) * scale]
            })
            .collect()
    }

    /// The parts of one leaf, in leaf space and NOT yet normalised.
    fn raw_parts(s: &LeafSilhouette) -> Vec<LeafPart> {
        match s.family() {
            LeafFamily::Deltoid => vec![LeafPart::Ribbon {
                at: [0.0, 0.0],
                dir: [0.0, 1.0],
                len: 1.0,
                // The exact pre-v0.1100 outline: straight sides from a
                // full-width base to a point. Two samples, two triangles.
                spine: vec![[0.0, 0.5], [1.0, 0.0]],
            }],
            LeafFamily::Ovate | LeafFamily::SerrateOvate | LeafFamily::PinnateLobed => {
                let n = margin_samples(s);
                vec![LeafPart::Ribbon {
                    at: [0.0, 0.0],
                    dir: [0.0, 1.0],
                    len: 1.0,
                    spine: spine_of(s, n, 0.5),
                }]
            }
            LeafFamily::Palmate => palmate(s),
            LeafFamily::Bipinnate => bipinnate(s),
            LeafFamily::NeedleFlatRank => needle_flat_rank(s),
            LeafFamily::NeedleFascicle => needle_fascicle(s),
        }
    }

    /// How far the outermost lobes of a palmate leaf swing off the midrib.
    ///
    /// 80 degrees: an Acer palmatum leaf is very nearly orbicular, its basal
    /// lobes reaching sideways almost to the horizontal. Past 90 they would
    /// point BACK past the petiole, which a cordate maple base does do but
    /// which would put leaf tissue behind the twig the leaf is attached to.
    const PALMATE_SPREAD_DEG: f32 = 80.0;

    /// Width of one maple lobe as a fraction of its own length. A palmatum
    /// lobe is lanceolate: long, narrow, and drawn to a point.
    const PALMATE_LOBE_WIDTH: f32 = 0.34;

    /// How much shorter the outermost lobe pair is than the central lobe.
    /// On a real palmatum the basal pair is roughly half the middle one, and
    /// that gradient is most of what stops the leaf reading as a starfish.
    const PALMATE_LOBE_FALLOFF: f32 = 0.45;

    fn palmate(s: &LeafSilhouette) -> Vec<LeafPart> {
        // Odd, so one lobe sits on the midrib. 5 and 7 are the usual counts
        // on Acer palmatum; 9 and 11 exist on the dissected cultivars.
        let l = (s.leaf_lobes.max(3) | 1).min(11) as i32;
        let mid = (l - 1) / 2;
        let step = if l > 1 {
            2.0 * PALMATE_SPREAD_DEG / (l - 1) as f32
        } else {
            0.0
        };
        // Driven by TEETH alone, deliberately, and NOT by `margin_samples`: a
        // maple lobe is a little blade, not a lobed one, so the leaf's lobe
        // count says nothing about how finely its lobes' margins need
        // sampling. Coupling the two would multiply a 7-lobed leaf's cost by
        // its own lobe count for detail nothing can resolve - a momiji leaf
        // bakes about 64 px tall, so a lobe is ~12 px across.
        let n = (s.leaf_teeth as usize * 3).clamp(10, 20);
        // Each lobe is a narrow blade in its own right, so it reuses the
        // simple-blade profile - the same teeth, the same acuminate tip - with
        // the lobe modulation turned OFF (a maple lobe is not itself lobed).
        let lobe_profile = LeafSilhouette { leaf_lobes: 0, leaf_sinus_frac: 0.0, ..s.clone() };
        let lobe_len = |i: i32| -> f32 {
            let x = (i - mid).abs() as f32 / mid.max(1) as f32;
            1.0 - PALMATE_LOBE_FALLOFF * x.powf(1.3)
        };
        let mut parts: Vec<LeafPart> = Vec::with_capacity(l as usize + 1);
        let mut dirs: Vec<([f32; 2], f32)> = Vec::with_capacity(l as usize);
        for i in 0..l {
            let ang = ((i - mid) as f32 * step).to_radians();
            let dir = [ang.sin(), ang.cos()];
            let len = lobe_len(i);
            dirs.push((dir, len));
            parts.push(LeafPart::Ribbon {
                at: [0.0, 0.0],
                dir,
                len,
                spine: spine_of(&lobe_profile, n, 0.5 * PALMATE_LOBE_WIDTH * len),
            });
        }
        // THE CONNECTIVE CENTRE. Lobes pinch to nothing at the petiole, so
        // without this the leaf is a star of detached slivers. Its boundary
        // runs out to `1 - sinus` along each lobe axis and dips between them,
        // which is exactly what "incised two thirds of the way to the base"
        // describes. Star-shaped about the petiole, so the fan is exact even
        // though the boundary is not convex.
        let keep = (1.0 - s.leaf_sinus_frac.clamp(0.0, 0.95)).max(0.05);
        let mut centre = vec![[0.0f32, 0.0f32]];
        for (i, (dir, len)) in dirs.iter().enumerate() {
            let r = keep * len;
            centre.push([dir[0] * r, dir[1] * r]);
            if let Some((nd, nl)) = dirs.get(i + 1) {
                // The sinus floor between two lobes sits lower than either.
                let md = [0.5 * (dir[0] + nd[0]), 0.5 * (dir[1] + nd[1])];
                let m = (md[0] * md[0] + md[1] * md[1]).sqrt().max(1e-6);
                let r = 0.72 * keep * 0.5 * (len + nl);
                centre.push([md[0] / m * r, md[1] / m * r]);
            }
        }
        parts.push(LeafPart::Poly(centre));
        parts
    }

    /// Angle a pinna leaves the rachis at, degrees off the rachis axis.
    ///
    /// 74, nearly square. The first dump used 62 - a 28 degree forward rake -
    /// and the frond came out as a chevron rather than a feather, because
    /// every pinna pair swept up and out together. A real mimosoid frond is
    /// flat and comb-like, its pinnae only slightly ascending.
    const PINNA_ANGLE_DEG: f32 = 74.0;

    fn bipinnate(s: &LeafSilhouette) -> Vec<LeafPart> {
        let pairs = s.leaf_lobes.clamp(2, 12) as usize;
        let leaflets = s.leaf_leaflets.clamp(2, 16) as usize;
        let mut parts: Vec<LeafPart> = Vec::new();
        // The rachis itself. Sub-millimetre on a real frond and close to
        // sub-texel in the sprite, but it is what makes a frond read as ONE
        // leaf instead of a swarm of specks.
        parts.push(LeafPart::Ribbon {
            at: [0.0, 0.0],
            dir: [0.0, 1.0],
            len: 1.0,
            spine: vec![[0.0, 0.010], [1.0, 0.004]],
        });
        let ang = PINNA_ANGLE_DEG.to_radians();
        for j in 0..pairs {
            // Pinnae start above the bare basal stretch of the rachis.
            let t = 0.20 + 0.80 * (j as f32 + 0.5) / pairs as f32;
            let plen = 0.42 * (1.0 - 0.12 * j as f32 / pairs as f32);
            for sign in [-1.0f32, 1.0] {
                let pdir = [sign * ang.sin(), ang.cos()];
                let root = [0.0, t];
                parts.push(LeafPart::Ribbon {
                    at: root,
                    dir: pdir,
                    len: plen,
                    spine: vec![[0.0, 0.006], [1.0, 0.002]],
                });
                // Leaflets: tiny oblongs in opposite pairs along the pinna,
                // standing almost square to it. On a real Vachellia they are
                // 1-5 mm, and their SEPARATION is the whole visual: a frond
                // has sky between its specks, which is why an acacia crown
                // reads as feathery rather than as a solid paddle.
                let llen = plen / leaflets as f32 * 1.55;
                let lperp = [-pdir[1], pdir[0]];
                for k in 0..leaflets {
                    let f = (k as f32 + 0.55) / leaflets as f32;
                    let base = [root[0] + pdir[0] * f * plen, root[1] + pdir[1] * f * plen];
                    for ls in [-1.0f32, 1.0] {
                        // Rake each leaflet slightly toward the pinna tip.
                        let d = [
                            ls * lperp[0] + 0.32 * pdir[0],
                            ls * lperp[1] + 0.32 * pdir[1],
                        ];
                        let m = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-6);
                        parts.push(LeafPart::Ribbon {
                            at: base,
                            dir: [d[0] / m, d[1] / m],
                            len: llen,
                            // Oblong with rounded ends: three samples is all a
                            // 3 px speck can carry, and this primitive is
                            // emitted 100+ times per frond.
                            spine: vec![
                                [0.0, llen * 0.16],
                                [0.5, llen * 0.24],
                                [1.0, llen * 0.05],
                            ],
                        });
                    }
                }
            }
        }
        parts
    }

    // ── Conifers: the drawn element is a SHOOT, not a leaf ───────────────
    //
    // Everything above builds ONE BLADE and lets the generator scatter
    // hundreds of them. A conifer cannot be built that way. The unit a conifer
    // canopy is actually made of is a NEEDLE SPRAY - a twig carrying a hundred
    // or more needles - and this engine's conifer form already knows that: its
    // `Foliage::leaf` is 0.30-0.50 m of "narrow strap", three times the size of
    // a broadleaf's blade and the size of a real branchlet, and the comment
    // there says so outright ("a conifer's drawn element is a NEEDLE SPRAY, not
    // a single needle"). Until now that spray was drawn as ONE TRIANGLE, which
    // is what the operator's fuji-forest capture shows: two big conifers wearing
    // a few hundred scattered dark darts on otherwise bare branches.
    //
    // So the two families below stamp a whole needled shoot into that element,
    // in the same leaf space every other family uses (y 0 at the shoot's base
    // to 1 at its tip, x spanning exactly 1). The needles are the repeated
    // primitive; the shoot itself is one thin ribbon down the middle.
    //
    // WHY THE TWO ARE SEPARATE CONSTRUCTIONS AND NOT ONE WITH A FLAG:
    //   - Abies (fir) sets its needles PECTINATELY, in two ranks either side of
    //     the shoot and all in ONE PLANE. Every needle is broadside to a viewer
    //     looking at the flat of the spray, so the spray reads as an even comb
    //     with a near-constant reach.
    //   - Pinus sets its needles in FASCICLES of 2-5 sheathed at the base,
    //     borne in a spiral all round the shoot. A viewer sees each bundle at a
    //     different azimuth, so bundles pointing across the view read full
    //     length and bundles pointing at the viewer read foreshortened - which
    //     is exactly why a pine shoot looks ragged and tufted where a fir spray
    //     looks combed. That foreshortening is computed here, not faked: the
    //     needle's real 3D direction is built and then projected.
    //
    // COST. Both keep the needle at the cheapest primitive that still carries a
    // silhouette: a two-sample ribbon, i.e. ONE QUAD, two triangles, whose base
    // and tip widths carry the whole shape difference (a fir needle is a
    // parallel-sided blunt strap, a pine needle tapers to a point). There is no
    // tube, no cross-section and no midrib. The whole element lands under a
    // maple leaf's 278 triangles.

    /// Tip width of a needle as a fraction of its base width, per family.
    ///
    /// Abies needles are parallel-sided straps with a BLUNT, usually notched
    /// tip - that bluntness is the field mark that separates a fir from a
    /// spruce at arm's length, so it is worth the one number it costs. Pinus
    /// needles are acicular and drawn to a fine point.
    const NEEDLE_FLAT_TAPER: f32 = 0.80;
    const NEEDLE_FASCICLE_TAPER: f32 = 0.10;

    /// Shoot half-thickness as a fraction of ONE NEEDLE's length.
    ///
    /// A fir branchlet is ~2.5 mm across carrying 25 mm needles (0.05); a pine
    /// shoot is ~4 mm across carrying 60 mm needles (0.033). Sub-millimetre in
    /// the finished sprite either way, and drawn for the same reason the
    /// acacia's rachis is: without it the element is a swarm of loose specks
    /// instead of one shoot.
    const NEEDLE_SHOOT_FLAT: f32 = 0.05;
    const NEEDLE_SHOOT_FASCICLE: f32 = 0.033;

    /// Fraction of the shoot's base that carries no needles.
    ///
    /// Real: the season's growth starts bare where it left its parent, and on a
    /// pine the lowest fascicles have usually been shed. It also matters
    /// mechanically here - `reshape_blades` roots the element at y = 0, which is
    /// where it meets the twig, so needles at y = 0 would grow out of the wood.
    const NEEDLE_BASE_BARE: f32 = 0.06;

    /// How much shorter the needles get toward the shoot's tip.
    ///
    /// 0.30 on both. The current season's needles at the very tip of a shoot
    /// have not finished extending, so a real spray tapers rather than ending
    /// square, and a square-ended spray is the single thing that most makes a
    /// procedural conifer look stamped.
    const NEEDLE_TIP_FALLOFF: f32 = 0.30;

    /// The golden angle, the same 2.399963 rad the tree generator spirals
    /// everything else by. Used here for the azimuth of successive fascicles
    /// about the shoot, which is what a real pine's phyllotaxis does.
    const NEEDLE_SPIRAL: f32 = 2.399_963;

    /// One needle: the cheapest primitive that still has a silhouette.
    fn needle(at: [f32; 2], dir: [f32; 2], len: f32, hw: f32, taper: f32) -> LeafPart {
        LeafPart::Ribbon {
            at,
            dir,
            len,
            spine: vec![[0.0, hw], [1.0, hw * taper]],
        }
    }

    /// The shoot every needle family hangs its needles on.
    fn needle_shoot(hw: f32) -> LeafPart {
        LeafPart::Ribbon {
            at: [0.0, 0.0],
            dir: [0.0, 1.0],
            len: 1.0,
            // Tapering, because a shoot does: it is thinnest at the growing tip.
            spine: vec![[0.0, hw], [1.0, hw * 0.55]],
        }
    }

    /// Where the `j`th of `n` needle stations sits along the shoot.
    fn needle_station(j: usize, n: usize) -> f32 {
        NEEDLE_BASE_BARE + (1.0 - NEEDLE_BASE_BARE) * (j as f32 + 0.5) / n as f32
    }

    /// FIR (Abies): needles in two ranks, one plane, near-square to the shoot.
    ///
    /// The two ranks are offset along the shoot by HALF a station rather than
    /// sitting exactly opposite. That is what a real pectinate shoot does (the
    /// needles are spirally inserted and then twisted into the plane, so they
    /// interleave), and it is also the difference between a spray that reads as
    /// foliage and one that reads as a fish skeleton.
    fn needle_flat_rank(s: &LeafSilhouette) -> Vec<LeafPart> {
        // 6..96 needle PAIRS. The ceiling is a triangle ceiling, not a
        // botanical one: a real Abies branchlet sets its needles 3-5 mm apart,
        // which on the 0.48 m spray this engine's conifer form hands us would
        // be ~120 pairs and ~490 triangles per element. See the note on the
        // fir row in trees.ron for why the shipped count is sparser than life.
        let pairs = s.leaf_lobes.clamp(6, 96) as usize;
        let nlen = s.needle_len();
        let hw = s.needle_halfwidth();
        let ang = s.needle_angle();
        let (sa, ca) = (ang.sin(), ang.cos());
        let mut parts = Vec::with_capacity(2 * pairs + 1);
        parts.push(needle_shoot(nlen * NEEDLE_SHOOT_FLAT));
        for j in 0..pairs {
            for (i, sign) in [-1.0f32, 1.0].into_iter().enumerate() {
                // Half-station offset between the two ranks.
                let t = needle_station(j, pairs) + (i as f32 - 0.5) * 0.5 / pairs as f32;
                let t = t.clamp(NEEDLE_BASE_BARE * 0.5, 1.0);
                let len = nlen * (1.0 - NEEDLE_TIP_FALLOFF * t * t);
                parts.push(needle([0.0, t], [sign * sa, ca], len, hw, NEEDLE_FLAT_TAPER));
            }
        }
        parts
    }

    /// PINE (Pinus): fascicles of 2-5 needles, spiralled all round the shoot.
    ///
    /// The bundle is the unit, and the arrangement is RADIAL, so this function
    /// does the one thing `needle_flat_rank` does not need to: it builds each
    /// fascicle's true 3D direction (tilted `needle_angle` off the shoot, at
    /// azimuth `j * golden angle` about it) and PROJECTS it into the sprite
    /// plane. A bundle lying across the view keeps its full length; a bundle
    /// pointing at or away from the viewer collapses toward the shoot axis.
    /// That variation IS the tufted, ragged read - a pine shoot drawn with
    /// every bundle at full length comes out as a herringbone, which is a fir
    /// with bigger needles and exactly the mistake this family exists to avoid.
    fn needle_fascicle(s: &LeafSilhouette) -> Vec<LeafPart> {
        let stations = s.leaf_lobes.clamp(4, 72) as usize;
        // 2 (sylvestris, densiflora, nigra), 3 (ponderosa, taeda) or 5
        // (strobus, lambertiana). Every pine on earth is in this range.
        let per = s.leaf_leaflets.clamp(2, 5) as usize;
        let nlen = s.needle_len();
        let hw = s.needle_halfwidth();
        let ang = s.needle_angle();
        let (sa, ca) = (ang.sin(), ang.cos());
        let spread = s.fascicle_spread();
        let mut parts = Vec::with_capacity(stations * per + 1);
        parts.push(needle_shoot(nlen * NEEDLE_SHOOT_FASCICLE));
        for j in 0..stations {
            let t = needle_station(j, stations);
            let phi = j as f32 * NEEDLE_SPIRAL;
            // Project the fascicle's 3D axis (sa*cos φ, ca, sa*sin φ) onto the
            // sprite plane. `m` is the foreshortening: 1 across the view, down
            // to |ca| straight at it.
            let (px, py) = (sa * phi.cos(), ca);
            let m = (px * px + py * py).sqrt().max(1e-4);
            let axis = [px / m, py / m];
            let base_len = nlen * m * (1.0 - NEEDLE_TIP_FALLOFF * t * t);
            // The splay a viewer SEES also foreshortens: a bundle pointing at
            // the camera splays around the view axis, not across it, so it
            // reads as a tight rosette rather than a fan. Scaling the drawn
            // splay by the same `m` errs toward the tight read, which is the
            // honest direction for a projection that cannot draw a rosette.
            let half = 0.5 * spread * m;
            for i in 0..per {
                let f = if per > 1 { i as f32 / (per - 1) as f32 - 0.5 } else { 0.0 };
                let (sd, cd) = (2.0 * f * half).sin_cos();
                // Rotate the projected axis by the splay offset.
                let dir = [axis[0] * cd - axis[1] * sd, axis[0] * sd + axis[1] * cd];
                parts.push(needle([0.0, t], dir, base_len, hw, NEEDLE_FASCICLE_TAPER));
            }
        }
        parts
    }

    /// The leaf ASPECT (width over length) this species' outline would have if
    /// it were drawn undistorted, measured off the raw construction.
    ///
    /// `normalized` maps x and y by DIFFERENT factors so that x spans exactly 1
    /// and the tip reaches exactly 1, and `reshape_blades` then rescales x by
    /// the data's `leaf_aspect`. Those two cancel - leaving the shape
    /// undistorted - only when the data states the aspect the construction
    /// actually has, i.e. `2 * max|x| / max(y)`.
    ///
    /// It matters far more for a needle family than for a blade. A blade's
    /// aspect is a stated botanical measurement and a 10% error just makes a
    /// slightly narrow leaf; a needle spray's aspect is DERIVED from the needle
    /// length and the angle it leaves the shoot at, so a wrong aspect silently
    /// stretches or squashes every needle on the shoot and the species stops
    /// being the species. `conifer_sprays_are_drawn_undistorted` gates it.
    pub fn natural_aspect(s: &LeafSilhouette) -> f32 {
        let parts = raw_parts(s);
        let (mut ymax, mut xabs) = (1e-6f32, 1e-6f32);
        for p in &parts {
            match p {
                LeafPart::Ribbon { at, dir, len, spine } => {
                    let (l, r) = ribbon_margins(*at, *dir, *len, spine);
                    for q in l.iter().chain(r.iter()) {
                        ymax = ymax.max(q[1]);
                        xabs = xabs.max(q[0].abs());
                    }
                }
                LeafPart::Poly(pts) => {
                    for q in pts {
                        ymax = ymax.max(q[1]);
                        xabs = xabs.max(q[0].abs());
                    }
                }
            }
        }
        2.0 * xabs / ymax
    }

    fn ribbon_margins(
        at: [f32; 2],
        dir: [f32; 2],
        len: f32,
        spine: &[[f32; 2]],
    ) -> (Vec<[f32; 2]>, Vec<[f32; 2]>) {
        let perp = [-dir[1], dir[0]];
        let mut lft = Vec::with_capacity(spine.len());
        let mut rgt = Vec::with_capacity(spine.len());
        for sp in spine {
            let (t, w) = (sp[0], sp[1]);
            let axis = [at[0] + dir[0] * t * len, at[1] + dir[1] * t * len];
            lft.push([axis[0] + perp[0] * w, axis[1] + perp[1] * w]);
            rgt.push([axis[0] - perp[0] * w, axis[1] - perp[1] * w]);
        }
        (lft, rgt)
    }

    /// Every triangle of one leaf, in leaf space, x spanning exactly 1 and y
    /// running 0 at the petiole to 1 at the tip.
    pub fn triangles(s: &LeafSilhouette) -> Vec<[[f32; 2]; 3]> {
        let parts = normalized(s);
        let mut tris = Vec::new();
        for p in &parts {
            match p {
                LeafPart::Ribbon { at, dir, len, spine } => {
                    let (l, r) = ribbon_margins(*at, *dir, *len, spine);
                    for i in 0..spine.len().saturating_sub(1) {
                        tris.push([l[i], r[i], r[i + 1]]);
                        tris.push([l[i], r[i + 1], l[i + 1]]);
                    }
                }
                LeafPart::Poly(pts) => {
                    for i in 1..pts.len().saturating_sub(1) {
                        tris.push([pts[0], pts[i], pts[i + 1]]);
                    }
                }
            }
        }
        tris.retain(|t| {
            let a = (t[1][0] - t[0][0]) * (t[2][1] - t[0][1])
                - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1]);
            a.abs() > 1e-9
        });
        tris
    }

    /// The closed boundary of every part, for drawing and for tests. Same
    /// source of truth as `triangles`, so the two cannot drift.
    pub fn outlines(s: &LeafSilhouette) -> Vec<Vec<[f32; 2]>> {
        normalized(s)
            .iter()
            .map(|p| match p {
                LeafPart::Ribbon { at, dir, len, spine } => {
                    let (l, mut r) = ribbon_margins(*at, *dir, *len, spine);
                    r.reverse();
                    let mut o = l;
                    o.extend(r);
                    o
                }
                LeafPart::Poly(pts) => pts.clone(),
            })
            .collect()
    }

    /// Parts scaled so y reaches exactly 1 at the tip and x spans exactly 1.
    ///
    /// The leaf's real width enters later, at `emit_leaf`, as
    /// `length * leaf_aspect` - so the ASPECT is a stated botanical fact and
    /// the outline generator never has to know about it.
    fn normalized(s: &LeafSilhouette) -> Vec<LeafPart> {
        let mut parts = raw_parts(s);
        let (mut ymax, mut xabs) = (1e-6f32, 1e-6f32);
        let mut visit = |p: [f32; 2], ymax: &mut f32, xabs: &mut f32| {
            *ymax = ymax.max(p[1]);
            *xabs = xabs.max(p[0].abs());
        };
        for p in &parts {
            match p {
                LeafPart::Ribbon { at, dir, len, spine } => {
                    let (l, r) = ribbon_margins(*at, *dir, *len, spine);
                    for q in l.iter().chain(r.iter()) {
                        visit(*q, &mut ymax, &mut xabs);
                    }
                }
                LeafPart::Poly(pts) => {
                    for q in pts {
                        visit(*q, &mut ymax, &mut xabs);
                    }
                }
            }
        }
        let (sy, sx) = (1.0 / ymax, 0.5 / xabs);
        for p in parts.iter_mut() {
            match p {
                LeafPart::Ribbon { at, dir, len, spine } => {
                    // Scaling x and y by different factors is an anisotropic
                    // map, so the axis direction and every half-width have to
                    // be re-derived rather than scaled in place. Convert the
                    // ribbon to its endpoints, map both, and rebuild.
                    let end = [at[0] + dir[0] * *len, at[1] + dir[1] * *len];
                    let a = [at[0] * sx, at[1] * sy];
                    let e = [end[0] * sx, end[1] * sy];
                    let d = [e[0] - a[0], e[1] - a[1]];
                    let m = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-9);
                    // Half-widths are measured across the axis; under the same
                    // anisotropic map that scale depends on the axis angle.
                    // Exact for the two cases that matter (an axis along y
                    // scales widths by sx, an axis along x by sy) and a smooth
                    // interpolation between.
                    let ux = (d[0] / m).abs();
                    let ws = sx * (1.0 - ux) + sy * ux;
                    *at = a;
                    *dir = [d[0] / m, d[1] / m];
                    *len = m;
                    for q in spine.iter_mut() {
                        q[1] *= ws;
                    }
                }
                LeafPart::Poly(pts) => {
                    for q in pts.iter_mut() {
                        q[0] *= sx;
                        q[1] *= sy;
                    }
                }
            }
        }
        parts
    }

    // ── Stamping the silhouette into a sprite mesh ───────────────────────

    /// Rebuild a cluster-sprite LEAF mesh with this species' real leaf outline
    /// in place of every deltoid blade.
    ///
    /// `tree_mesh::cluster_sprite_geometry`'s leaf arm emits nothing but
    /// `sprig`s of `blade`s, and a blade is one `tri2` - front face `(l, tip,
    /// r)` then back face `(l, r, tip)`, six vertices, no sharing. So each
    /// six-vertex group recovers a leaf's exact frame: base, midrib direction,
    /// length, width axis, and the per-leaf size jitter and midrib roll the
    /// generator already applied. Reshaping in that recovered frame keeps the
    /// SCATTER (which is measured, tuned and gated upstream) byte for byte and
    /// changes only the outline of what sits at each position.
    ///
    /// Returns None, and the caller keeps the original mesh, if the input is
    /// not that exact shape - so a future change to the leaf arm degrades to
    /// the old triangles instead of silently corrupting the sprite.
    ///
    /// PER-LEAF COLOUR (v0.1109). The six-vertex group index IS a leaf index,
    /// so it is also the jitter key: leaf `g` of this sprite gets its own hue
    /// rotation, its own face (adaxial or the paler abaxial) and its own place
    /// in the senescent tail, deterministically and without touching the
    /// scatter's random stream. Before this, `color` went to every triangle of
    /// every leaf unchanged, which is why the shipped sprites measured ONE
    /// distinct RGB value across 130,407 covered texels.
    pub fn reshape_blades(
        src: &PlantMeshBuilder,
        s: &LeafSilhouette,
        color: [f32; 3],
        var: super::leaf_colour::LeafVariation,
    ) -> Option<PlantMeshBuilder> {
        if s.family() == LeafFamily::Deltoid {
            return None;
        }
        let v = &src.vertices;
        if v.is_empty() || v.len() % 6 != 0 {
            return None;
        }
        let tris = triangles(s);
        if tris.is_empty() {
            return None;
        }
        let mut out = PlantMeshBuilder::new();
        out.set_organ(Organ::Leaf);
        for (leaf_i, g) in v.chunks_exact(6).enumerate() {
            let (p0, p1, p2) = (g[0].position, g[1].position, g[2].position);
            // The tri2 signature: the back face repeats a, then c, then b.
            if g[3].position != p0 || g[4].position != p2 || g[5].position != p1 {
                return None;
            }
            let base = [
                0.5 * (p0[0] + p2[0]),
                0.5 * (p0[1] + p2[1]),
                0.5 * (p0[2] + p2[2]),
            ];
            let axis = [p1[0] - base[0], p1[1] - base[1], p1[2] - base[2]];
            let len = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
            let cross = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            let wid_in = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
            if len < 1e-6 || wid_in < 1e-9 {
                return None;
            }
            let dir = [axis[0] / len, axis[1] / len, axis[2] / len];
            // Re-orthogonalise the width axis against the midrib. It already
            // is by construction; doing it anyway means a future blade that
            // skews its base cannot shear the outline.
            let mut side = [cross[0] / wid_in, cross[1] / wid_in, cross[2] / wid_in];
            let d = side[0] * dir[0] + side[1] * dir[1] + side[2] * dir[2];
            for i in 0..3 {
                side[i] -= dir[i] * d;
            }
            let sm = (side[0] * side[0] + side[1] * side[1] + side[2] * side[2]).sqrt();
            if sm < 1e-6 {
                return None;
            }
            for i in 0..3 {
                side[i] /= sm;
            }
            // A stated aspect is a measurement of the plant and overrides the
            // generic width the mesh builder gives every broadleaf; 0 keeps
            // whatever the blade already had.
            let wid = if s.leaf_aspect > 0.0 { len * s.leaf_aspect } else { wid_in };
            let map = |p: [f32; 2]| -> [f32; 3] {
                [
                    base[0] + dir[0] * p[1] * len + side[0] * p[0] * wid,
                    base[1] + dir[1] * p[1] * len + side[1] * p[0] * wid,
                    base[2] + dir[2] * p[1] * len + side[2] * p[0] * wid,
                ]
            };
            // THIS leaf's colour, not the species'. Keyed on the group index
            // so it is stable across rebuilds and independent of the scatter.
            let lc = super::leaf_colour::jitter(color, var, leaf_i as u64);
            for t in &tris {
                out.tri2(map(t[0]), map(t[1]), map(t[2]), lc);
            }
        }
        out.set_organ(Organ::Stem);
        Some(out)
    }

    /// The cluster-sprite geometry a species should actually be baked from:
    /// `tree_mesh`'s scatter, wearing this species' leaf.
    ///
    /// One place, so the GPU baker and the CPU twin below can never disagree
    /// about what was baked.
    pub fn sprite_geometry(
        def: &tree_mesh::TreeDef,
        layer: ClusterLayer,
        height_m: f32,
    ) -> Option<PlantMeshBuilder> {
        let mesh = tree_mesh::cluster_sprite_geometry(def, layer, height_m)?;
        if layer != ClusterLayer::Leaf {
            return Some(mesh);
        }
        let s = of(&def.id);
        let var = super::leaf_colour::of(&def.id);
        match reshape_blades(&mesh, &s, def.leaf_color, var) {
            Some(m) => Some(m),
            None => {
                if s.family() != LeafFamily::Deltoid {
                    log::warn!(
                        "[Leaf] {}: sprite mesh is not a plain run of blades, keeping the deltoid \
                         outline (the {} silhouette is not being baked)",
                        def.id,
                        s.family().key()
                    );
                }
                Some(mesh)
            }
        }
    }
}

// ── The CPU twin of the cluster bake (v0.1100 dev aid) ───────────────────
//
// `bake_cluster_sprites` needs a GPU, an adapter, a swapchain format and a
// booted app. That is the right way to BAKE, and the wrong way to ANSWER "does
// a maple leaf come out looking like a maple leaf" - a question that has to be
// answerable from a test, on a build machine, without booting anything and
// without taking the operator's one GPU (see the ONE GPU rule in CLAUDE.md).
//
// So this is the same pass on the CPU: the same orthographic framing
// `bake_parts_into` computes, the same packed-colour decode the bake shader
// does, the same 4x supersample, the same `box_downsample_rgba`, the same
// `alpha_coverage`. What comes out is what the GPU would have produced, minus
// the rasteriser's own fill-rule tie-breaks - close enough that the coverage it
// measures is the number a species' `coverage` field should state, which is
// exactly what it was used for when this arc's data was written.

/// One species' baked sprite, measured on the CPU.
pub struct CpuSprite {
    pub species: String,
    pub layer: ClusterLayer,
    /// RGBA8, `sprite_px` square, sRGB-encoded exactly like the bake target.
    pub rgba: Vec<u8>,
    pub sprite_px: u32,
    /// Fraction of texels above `CLUSTER_ALPHA_CUTOFF`. THE number that
    /// belongs in the species' `coverage` field.
    pub coverage: f32,
    pub triangles: usize,
}

/// Rasterise a packed-colour plant mesh orthographically, side on, framed
/// exactly the way `bake_parts_into` frames it.
///
/// Kept deliberately in lockstep with that function: the frame is the joint
/// AABB's widest horizontal extent against its height, squared off with the
/// same 5% margin. If the two ever disagree the CPU twin stops predicting the
/// bake, which is its only job.
pub fn rasterize_packed_ortho(vertices: &[Vertex], indices: &[u32], px: u32) -> Vec<u8> {
    let mut out = vec![0u8; (px * px * 4) as usize];
    if vertices.is_empty() || indices.len() < 3 || px == 0 {
        return out;
    }
    let mut mn = [f32::MAX; 3];
    let mut mx = [f32::MIN; 3];
    for v in vertices {
        for i in 0..3 {
            mn[i] = mn[i].min(v.position[i]);
            mx[i] = mx[i].max(v.position[i]);
        }
    }
    let w_m = (mx[0] - mn[0]).max(mx[2] - mn[2]).max(1e-3);
    let h_m = (mx[1] - mn[1]).max(1e-3);
    let half = 0.5 * w_m.max(h_m) * 1.05;
    let cx = 0.5 * (mn[0] + mx[0]);
    let cy = 0.5 * (mn[1] + mx[1]);
    let fpx = px as f32;
    // Eye is at +Z looking down -Z, so a LARGER z is nearer and wins.
    let mut depth = vec![f32::MIN; (px * px) as usize];
    for f in indices.chunks_exact(3) {
        let v: [&Vertex; 3] = [
            &vertices[f[0] as usize],
            &vertices[f[1] as usize],
            &vertices[f[2] as usize],
        ];
        // Same decode as the bake shader's packed branch and the megashader's
        // type-20 branch: r and g are 8-bit integers in uv.x (above whatever
        // organ bits ride there), b is a float in uv.y, all LINEAR.
        let packed = v[0].uv[0].max(0.0).round() as u32;
        let lin = [
            ((packed >> 8) & 255) as f32 / 255.0,
            (packed & 255) as f32 / 255.0,
            v[0].uv[1].clamp(0.0, 1.0),
        ];
        // The bake target is an sRGB format, so the hardware encodes on write.
        let rgb = [
            linear_to_srgb(lin[0]),
            linear_to_srgb(lin[1]),
            linear_to_srgb(lin[2]),
        ];
        let sp: Vec<[f32; 3]> = v
            .iter()
            .map(|q| {
                [
                    ((q.position[0] - cx) / half * 0.5 + 0.5) * fpx,
                    (0.5 - (q.position[1] - cy) / half * 0.5) * fpx,
                    q.position[2],
                ]
            })
            .collect();
        let x0 = sp.iter().fold(f32::MAX, |a, p| a.min(p[0])).floor().max(0.0) as u32;
        let x1 = (sp.iter().fold(f32::MIN, |a, p| a.max(p[0])).ceil() + 1.0).clamp(0.0, fpx) as u32;
        let y0 = sp.iter().fold(f32::MAX, |a, p| a.min(p[1])).floor().max(0.0) as u32;
        let y1 = (sp.iter().fold(f32::MIN, |a, p| a.max(p[1])).ceil() + 1.0).clamp(0.0, fpx) as u32;
        let area = (sp[1][0] - sp[0][0]) * (sp[2][1] - sp[0][1])
            - (sp[2][0] - sp[0][0]) * (sp[1][1] - sp[0][1]);
        if area.abs() < 1e-9 {
            continue;
        }
        let inv = 1.0 / area;
        for y in y0..y1 {
            for x in x0..x1 {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let e = |a: &[f32; 3], b: &[f32; 3]| {
                    (b[0] - a[0]) * (fy - a[1]) - (fx - a[0]) * (b[1] - a[1])
                };
                let (w0, w1, w2) = (
                    e(&sp[1], &sp[2]) * inv,
                    e(&sp[2], &sp[0]) * inv,
                    e(&sp[0], &sp[1]) * inv,
                );
                // The bake pipeline culls nothing (foliage is double sided),
                // so accept either winding.
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = w0 * sp[0][2] + w1 * sp[1][2] + w2 * sp[2][2];
                let di = (y * px + x) as usize;
                if z <= depth[di] {
                    continue;
                }
                depth[di] = z;
                let o = di * 4;
                out[o] = rgb[0];
                out[o + 1] = rgb[1];
                out[o + 2] = rgb[2];
                out[o + 3] = 255;
            }
        }
    }
    out
}

/// Bake one species' cluster sprite on the CPU and measure its coverage.
///
/// `bake_px` is the supersampled render size and must be a whole multiple of
/// `sprite_px`, exactly as `CLUSTER_BAKE_PX` is of `CLUSTER_SPRITE_PX`.
pub fn cpu_cluster_sprite(
    def: &tree_mesh::TreeDef,
    layer: ClusterLayer,
    bake_px: u32,
    sprite_px: u32,
) -> Option<CpuSprite> {
    let mesh = leaf_shape::sprite_geometry(def, layer, def.height_m)?;
    if mesh.indices.is_empty() {
        return None;
    }
    let hi = rasterize_packed_ortho(&mesh.vertices, &mesh.indices, bake_px);
    let base = box_downsample_rgba(&hi, bake_px, bake_px, (bake_px / sprite_px).max(1), true);
    let coverage = alpha_coverage(&base, CLUSTER_ALPHA_CUTOFF);
    Some(CpuSprite {
        species: def.id.clone(),
        layer,
        rgba: base,
        sprite_px,
        coverage,
        triangles: mesh.indices.len() / 3,
    })
}

/// Write one leaf, big, as its own PNG. The most direct answer there is to
/// "what does this species' leaf look like": no scatter, no overlap, no
/// minification, just the outline the data asked for.
pub fn dump_leaf_silhouettes(dir: &std::path::Path, px: u32) -> Vec<(String, String, usize)> {
    let _ = std::fs::create_dir_all(dir);
    let mut out = Vec::new();
    for s in leaf_shape::registry() {
        let tris = leaf_shape::triangles(s);
        if tris.is_empty() {
            continue;
        }
        let aspect = if s.leaf_aspect > 0.0 { s.leaf_aspect } else { 0.70 };
        // OPAQUE black, not a transparent clear. A transparent PNG is
        // composited against whatever the viewer feels like (white, in every
        // image tool tried), and a white leaf on a white page is a blank page
        // - which is exactly how the first dump of this came out.
        let mut img: Vec<u8> = std::iter::repeat([0u8, 0, 0, 255])
            .take((px * px) as usize)
            .flatten()
            .collect();
        // Fit the leaf in the square with a small margin, petiole at the
        // bottom, tip at the top - the way a field guide prints one.
        //
        // ...EXCEPT for a very narrow element, which is drawn LYING DOWN, base
        // at the left. A conifer shoot's aspect is around a tenth, so upright
        // it would occupy a 50 px sliver of a 512 px page and the arrangement
        // of its needles - the entire thing being judged - would be unreadable.
        // Rotating it fills the page and is also how every field guide prints a
        // conifer shoot. No broadleaf in the registry is near this threshold
        // (the narrowest, sakura, is 0.45), so nothing else moves.
        let lying = aspect < 0.35;
        let m = 0.06 * px as f32;
        let span = px as f32 - 2.0 * m;
        let scale = span.min(span / aspect.max(1e-3));
        for t in &tris {
            let p: Vec<[f32; 2]> = t
                .iter()
                .map(|q| {
                    if lying {
                        [m + q[1] * scale, px as f32 * 0.5 - q[0] * aspect * scale]
                    } else {
                        [
                            px as f32 * 0.5 + q[0] * aspect * scale,
                            px as f32 - m - q[1] * scale,
                        ]
                    }
                })
                .collect();
            fill_tri_mask(&mut img, px, &p);
        }
        let path = dir.join(format!("leaf_{}_{}.png", s.id, s.family().key()));
        if let Some(i) = image::RgbaImage::from_raw(px, px, img) {
            let _ = i.save(&path);
        }
        out.push((s.id.clone(), s.family().key().to_string(), tris.len()));
    }
    out
}

/// Opaque white, so a silhouette is unmistakable against the black clear.
fn fill_tri_mask(img: &mut [u8], px: u32, p: &[[f32; 2]]) {
    let area = (p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1]);
    if area.abs() < 1e-9 {
        return;
    }
    let inv = 1.0 / area;
    let x0 = p.iter().fold(f32::MAX, |a, q| a.min(q[0])).floor().max(0.0) as u32;
    let x1 = (p.iter().fold(f32::MIN, |a, q| a.max(q[0])).ceil() + 1.0).clamp(0.0, px as f32) as u32;
    let y0 = p.iter().fold(f32::MAX, |a, q| a.min(q[1])).floor().max(0.0) as u32;
    let y1 = (p.iter().fold(f32::MIN, |a, q| a.max(q[1])).ceil() + 1.0).clamp(0.0, px as f32) as u32;
    for y in y0..y1 {
        for x in x0..x1 {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let e = |a: &[f32; 2], b: &[f32; 2]| {
                (b[0] - a[0]) * (fy - a[1]) - (fx - a[0]) * (b[1] - a[1])
            };
            let (w0, w1, w2) = (
                e(&p[1], &p[2]) * inv,
                e(&p[2], &p[0]) * inv,
                e(&p[0], &p[1]) * inv,
            );
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let o = ((y * px + x) * 4) as usize;
            img[o] = 255;
            img[o + 1] = 255;
            img[o + 2] = 255;
            img[o + 3] = 255;
        }
    }
}

/// CPU-bake every clustered species' sprite and write two PNGs each: the true
/// RGBA sprite, and an opaque black-and-white ALPHA sheet, because a
/// transparent PNG of dark green foliage is close to unreadable and the
/// silhouette is the thing being judged.
///
/// Returns `(species, layer, coverage, triangles)` per sprite - the coverage
/// column is what the species' `coverage` data field should say.
pub fn dump_cluster_sprites_cpu(
    dir: &std::path::Path,
    bake_px: u32,
    sprite_px: u32,
) -> Vec<(String, &'static str, f32, usize)> {
    let _ = std::fs::create_dir_all(dir);
    let mut rows = Vec::new();
    for t in tree_mesh::registry().trees.iter() {
        if t.clusters.is_none() {
            continue;
        }
        for layer in ClusterLayer::ALL {
            if layer == ClusterLayer::Blossom && t.blossom_frac <= 0.0 {
                continue;
            }
            let Some(spr) = cpu_cluster_sprite(t, layer, bake_px, sprite_px) else {
                continue;
            };
            let stem = format!("cluster_{}_{}", t.id, layer.key());
            if let Some(i) =
                image::RgbaImage::from_raw(sprite_px, sprite_px, spr.rgba.clone())
            {
                let _ = i.save(dir.join(format!("{stem}.png")));
            }
            let mask: Vec<u8> = spr
                .rgba
                .chunks_exact(4)
                .flat_map(|p| {
                    let a = if p[3] as f32 / 255.0 >= CLUSTER_ALPHA_CUTOFF { 255u8 } else { 0 };
                    [a, a, a, 255]
                })
                .collect();
            if let Some(i) = image::RgbaImage::from_raw(sprite_px, sprite_px, mask) {
                let _ = i.save(dir.join(format!("{stem}_alpha.png")));
            }
            rows.push((t.id.clone(), layer.key(), spr.coverage, spr.triangles));
        }
    }
    rows
}

// ── Cluster mip chain (pure CPU, so the gate runs in CI) ─────────────────

fn srgb_to_linear(u: u8) -> f32 {
    // EXACT, not an approximation: the input is a u8, so the whole function has
    // 256 possible answers and a table is the same numbers without the powf.
    // That matters because `box_downsample_rgba` calls this once per source
    // CHANNEL - at the v0.1090 cluster bake size that is 2048*2048*3 = 12.6
    // million calls per sprite, and `powf` is ~40 ns, so the table is the
    // difference between a ~500 ms bake and a ~300 ms one. Built once per
    // process by `OnceLock`.
    static LUT: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    LUT.get_or_init(|| {
        let mut t = [0.0f32; 256];
        for (i, v) in t.iter_mut().enumerate() {
            let c = i as f32 / 255.0;
            *v = if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        }
        t
    })[u as usize]
}

fn linear_to_srgb(v: f32) -> u8 {
    let c = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8
}

/// Box-downsample an RGBA8 image by an integer `factor`.
///
/// Two details that are not decoration:
///   - RGB is averaged ALPHA-WEIGHTED, so the transparent black the bake
///     clears to cannot bleed a dark fringe into a leaf edge as it minifies;
///   - and when the source is sRGB-encoded (it is, the bake target is the
///     swapchain format) the average is taken in LINEAR light, because
///     averaging encoded values darkens every mixed texel.
/// Alpha is a coverage fraction and is always linear, so it averages directly.
pub fn box_downsample_rgba(src: &[u8], w: u32, h: u32, factor: u32, srgb: bool) -> Vec<u8> {
    let f = factor.max(1);
    let (dw, dh) = ((w / f).max(1), (h / f).max(1));
    let mut out = vec![0u8; (dw * dh * 4) as usize];
    let n = (f * f) as f32;
    for y in 0..dh {
        for x in 0..dw {
            let (mut r, mut g, mut b, mut a) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            for sy in 0..f {
                for sx in 0..f {
                    let px = ((y * f + sy) * w + (x * f + sx)) as usize * 4;
                    if px + 3 >= src.len() {
                        continue;
                    }
                    let av = src[px + 3] as f32 / 255.0;
                    let (cr, cg, cb) = if srgb {
                        (
                            srgb_to_linear(src[px]),
                            srgb_to_linear(src[px + 1]),
                            srgb_to_linear(src[px + 2]),
                        )
                    } else {
                        (
                            src[px] as f32 / 255.0,
                            src[px + 1] as f32 / 255.0,
                            src[px + 2] as f32 / 255.0,
                        )
                    };
                    r += cr * av;
                    g += cg * av;
                    b += cb * av;
                    a += av;
                }
            }
            let wsum = a.max(1e-6);
            let (mr, mg, mb) = (r / wsum, g / wsum, b / wsum);
            let d = ((y * dw + x) * 4) as usize;
            if srgb {
                out[d] = linear_to_srgb(mr);
                out[d + 1] = linear_to_srgb(mg);
                out[d + 2] = linear_to_srgb(mb);
            } else {
                out[d] = (mr.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                out[d + 1] = (mg.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
                out[d + 2] = (mb.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            }
            out[d + 3] = ((a / n).clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
        }
    }
    out
}

/// Fraction of texels whose alpha passes `cutoff`.
pub fn alpha_coverage(rgba: &[u8], cutoff: f32) -> f32 {
    coverage_scaled(rgba, cutoff, 1.0)
}

fn coverage_scaled(rgba: &[u8], cutoff: f32, scale: f32) -> f32 {
    if rgba.len() < 4 {
        return 0.0;
    }
    let thr = cutoff * 255.0;
    let mut hit = 0usize;
    let mut n = 0usize;
    for px in rgba.chunks_exact(4) {
        if px[3] as f32 * scale >= thr {
            hit += 1;
        }
        n += 1;
    }
    if n == 0 {
        0.0
    } else {
        hit as f32 / n as f32
    }
}

/// Scale a level's alpha so its above-cutoff coverage matches `target`.
///
/// Binary search on the scale, the standard alpha-coverage-preserving mip
/// build. Without it a cutout foliage sprite loses coverage every level and
/// the canopy visibly thins with distance - the same failure mode that made
/// the fir sprite bake "nearly bare" and forced this baker's 0.3 cutoff.
fn rescale_alpha_to_coverage(rgba: &mut [u8], cutoff: f32, target: f32) {
    if target <= 0.0 || rgba.len() < 4 {
        return;
    }
    // The upper bound has to be generous: by the 8x8 level a sparse foliage
    // silhouette has averaged down to alphas well under the cutoff, and a
    // timid range would leave the level under-covered (the canopy dissolving
    // at distance, which is the whole failure being prevented).
    let (mut lo, mut hi) = (0.0f32, 16.0f32);
    let mut best = 1.0f32;
    let mut best_err = f32::MAX;
    for _ in 0..16 {
        let mid = 0.5 * (lo + hi);
        let cov = coverage_scaled(rgba, cutoff, mid);
        let err = (cov - target).abs();
        if err < best_err {
            best_err = err;
            best = mid;
        }
        if cov > target {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    for px in rgba.chunks_exact_mut(4) {
        px[3] = (px[3] as f32 * best).min(255.0) as u8;
    }
}

/// Full mip chain for a square cluster sprite, biggest first.
///
/// Each level is box-filtered from the previous UNSCALED level (so the
/// coverage corrections cannot compound into saturation) and then rescaled to
/// level 0's coverage.
/// Mip chain for an OPAQUE, TILING texture - the baked bark (v0.1089).
///
/// Deliberately NOT `build_mip_chain`: that one serves alpha-cutout cluster
/// sprites and does two things that would corrupt bark. (1) It rescales alpha
/// per level to preserve silhouette coverage; bark's alpha is a linear
/// height/AO channel, not coverage, so rescaling would flatten its relief with
/// distance. (2) `box_downsample_rgba` averages RGB alpha-WEIGHTED (right for
/// a cutout: transparent black must not bleed into a leaf edge), which here
/// would weight colour by height and darken every ridge.
///
/// So: a straight box filter. RGB averaged in LINEAR light because the texture
/// is sRGB-encoded (averaging encoded values darkens every mixed texel), alpha
/// averaged directly, all the way down to 1x1 - a trunk at 200 m is a fraction
/// of a pixel and must still sample something sane rather than shimmer.
pub fn build_opaque_mip_chain(base: &[u8], size: u32) -> Vec<Vec<u8>> {
    let mut levels = vec![base.to_vec()];
    let mut w = size;
    while w >= 2 {
        let src = levels.last().expect("level 0 pushed above");
        let dw = w / 2;
        let mut out = vec![0u8; (dw * dw * 4) as usize];
        for y in 0..dw {
            for x in 0..dw {
                let (mut r, mut g, mut b, mut a) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
                for sy in 0..2u32 {
                    for sx in 0..2u32 {
                        let px = (((y * 2 + sy) * w) + (x * 2 + sx)) as usize * 4;
                        r += srgb_to_linear(src[px]);
                        g += srgb_to_linear(src[px + 1]);
                        b += srgb_to_linear(src[px + 2]);
                        a += src[px + 3] as f32 / 255.0;
                    }
                }
                let d = ((y * dw + x) * 4) as usize;
                out[d] = linear_to_srgb(r * 0.25);
                out[d + 1] = linear_to_srgb(g * 0.25);
                out[d + 2] = linear_to_srgb(b * 0.25);
                out[d + 3] = (a * 0.25 * 255.0 + 0.5) as u8;
            }
        }
        levels.push(out);
        w = dw;
    }
    levels
}

pub fn build_mip_chain(base: &[u8], size: u32, cutoff: f32, srgb: bool) -> Vec<Vec<u8>> {
    let mut levels = vec![base.to_vec()];
    let target = alpha_coverage(base, cutoff);
    let mut unscaled = base.to_vec();
    let mut w = size;
    while w > CLUSTER_MIP_MIN_PX && w >= 2 {
        let down = box_downsample_rgba(&unscaled, w, w, 2, srgb);
        let mut scaled = down.clone();
        rescale_alpha_to_coverage(&mut scaled, cutoff, target);
        levels.push(scaled);
        unscaled = down;
        w /= 2;
    }
    levels
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The atlas must hold the shipped registry with room to grow. If this
    /// fails, either trees.ron gained rows or the grid shrank - grow
    /// ATLAS_COLS/ATLAS_ROWS and the shader's three literals together.
    #[test]
    fn atlas_grid_holds_the_registry_with_headroom() {
        let used = tree_mesh::tiles_in_use();
        let cap = ATLAS_COLS * ATLAS_ROWS;
        assert!(used <= cap, "registry needs {used} tiles, atlas holds {cap}");
        assert_eq!(cap, tree_mesh::ATLAS_TILES);
        // Encoding headroom: the card packs |uv.x| = (1 + tile) + u01 * 0.5,
        // so the tile index must stay an exact integer in f32 and the 0.5
        // fraction must survive. 48.5 is nowhere near the 2^24 ceiling.
        let worst = (1 + cap) as f32 + 0.5;
        assert_eq!(worst.floor() as u32, cap + 1, "tile index lost integrality");
        assert!((worst.fract() - 0.5).abs() < 1e-6, "u01 fraction lost precision");
    }

    // ── Bark mip chain (v0.1089) ─────────────────────────────────────────

    /// The bark chain must run all the way to 1x1 and must PRESERVE the alpha
    /// channel, because on bark alpha is a linear height/AO field, not
    /// coverage. If it were routed through `build_mip_chain` instead, the
    /// coverage rescale would drive every level's alpha toward saturation and
    /// the relief would flatten with distance - the exact fade this increment
    /// exists to delete.
    #[test]
    fn opaque_mip_chain_reaches_1x1_and_keeps_the_height_channel() {
        let size = 64u32;
        let mut base = vec![0u8; (size * size * 4) as usize];
        let mut sum = 0.0f64;
        for y in 0..size {
            for x in 0..size {
                let i = ((y * size + x) * 4) as usize;
                // A plate-like field: alpha ramps, rgb tracks it.
                let a = (((x / 8 + y / 4) % 5) * 50).min(255) as u8;
                base[i] = a / 2;
                base[i + 1] = a / 3;
                base[i + 2] = a / 4;
                base[i + 3] = a;
                sum += a as f64;
            }
        }
        let levels = build_opaque_mip_chain(&base, size);
        assert_eq!(levels.len(), 7, "64 -> 1 is seven levels");
        for (i, l) in levels.iter().enumerate() {
            let w = (size >> i).max(1) as usize;
            assert_eq!(l.len(), w * w * 4, "level {i} is {w}x{w}");
        }
        let mean0 = sum / (size * size) as f64;
        let last = levels.last().unwrap();
        assert!(
            (last[3] as f64 - mean0).abs() < 2.0,
            "1x1 level's height {} drifted from the mean {mean0:.1}",
            last[3]
        );
        // Level 1's alpha must still SPREAD - a flattened chain is the bug.
        let l1 = &levels[1];
        let lo = l1.chunks_exact(4).map(|p| p[3]).min().unwrap();
        let hi = l1.chunks_exact(4).map(|p| p[3]).max().unwrap();
        assert!(hi - lo > 100, "level 1 height range {lo}..{hi} collapsed");
    }

    // ── Cluster sprite mip chain (v0.1088) ───────────────────────────────

    /// A synthetic foliage-like cutout: scattered opaque discs on a
    /// transparent field, which is the alpha statistic a real leaf-cluster
    /// sprite has (many small blobs with gaps between them) and the hardest
    /// case for coverage-preserving minification.
    fn synthetic_sprite(size: u32, blobs: u32, r: f32) -> Vec<u8> {
        let mut px = vec![0u8; (size * size * 4) as usize];
        let mut s: u64 = 0x1234_5678_9abc_def0;
        let mut next = || {
            s ^= s >> 12;
            s ^= s << 25;
            s ^= s >> 27;
            (s.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32
        };
        for _ in 0..blobs {
            let cx = next() * size as f32;
            let cy = next() * size as f32;
            let rr = r * (0.6 + 0.8 * next());
            let (x0, x1) = ((cx - rr).max(0.0) as u32, ((cx + rr) as u32 + 1).min(size));
            let (y0, y1) = ((cy - rr).max(0.0) as u32, ((cy + rr) as u32 + 1).min(size));
            for y in y0..y1 {
                for x in x0..x1 {
                    let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                    if d <= rr {
                        let i = ((y * size + x) * 4) as usize;
                        px[i] = 60;
                        px[i + 1] = 120;
                        px[i + 2] = 40;
                        px[i + 3] = 255;
                    }
                }
            }
        }
        px
    }

    /// Reference sprite side the test cases below are written against.
    ///
    /// `blobs` and `r` describe a 256 px sprite; `baked_sprite` rescales both
    /// to whatever `CLUSTER_SPRITE_PX` currently is so the COVERAGE statistic
    /// the mip gate exercises is invariant. Without this the 256 -> 512 bump
    /// (v0.1090) quartered every case's coverage and the sparsest one fell
    /// through the gate's own "degenerate test sprite" floor - a test failing
    /// because the constant it is parameterised by moved, not because the code
    /// under test broke.
    const REF_PX: u32 = 256;

    /// The production path in miniature: a BINARY silhouette rendered at
    /// `CLUSTER_BAKE_PX` and box-downsampled to `CLUSTER_SPRITE_PX`, which is
    /// what gives level 0 its smooth partial-coverage alpha. Testing the mip
    /// build against a binary sprite instead would be testing a case the baker
    /// never produces.
    fn baked_sprite(blobs: u32, r: f32) -> Vec<u8> {
        // Radius scales with the SIDE, count with the AREA, so coverage holds.
        let k = CLUSTER_SPRITE_PX as f32 / REF_PX as f32;
        let n = ((blobs as f32 * k * k).round() as u32).max(1);
        let ss = (CLUSTER_BAKE_PX / CLUSTER_SPRITE_PX) as f32;
        let hi = synthetic_sprite(CLUSTER_BAKE_PX, n, r * k * ss);
        box_downsample_rgba(
            &hi,
            CLUSTER_BAKE_PX,
            CLUSTER_BAKE_PX,
            CLUSTER_BAKE_PX / CLUSTER_SPRITE_PX,
            true,
        )
    }

    /// The cluster bake is a 4x SUPERSAMPLE followed by a box downsample, and
    /// that is the only anti-aliasing an alpha-tested cutout ever gets in this
    /// engine (multisample count 1, no MSAA anywhere).
    ///
    /// Raising `CLUSTER_SPRITE_PX` without raising `CLUSTER_BAKE_PX` with it
    /// would silently drop the factor to 2 or 1 - the silhouette would still
    /// bake, still mip and still pass every other test in this file, and the
    /// only symptom would be jaggier flower edges in a screenshot. Lock it.
    #[test]
    fn cluster_bake_keeps_its_four_times_supersample() {
        assert_eq!(
            CLUSTER_BAKE_PX % CLUSTER_SPRITE_PX,
            0,
            "the box downsample factor {}/{} is not a whole number",
            CLUSTER_BAKE_PX,
            CLUSTER_SPRITE_PX
        );
        assert_eq!(
            CLUSTER_BAKE_PX / CLUSTER_SPRITE_PX,
            4,
            "cluster sprites bake at {}x supersample, not 4x",
            CLUSTER_BAKE_PX / CLUSTER_SPRITE_PX
        );
        assert!(
            CLUSTER_SPRITE_PX.is_power_of_two() && CLUSTER_BAKE_PX.is_power_of_two(),
            "the mip chain halves from {CLUSTER_SPRITE_PX} and needs a power of two"
        );
        // wgpu's downlevel-guaranteed `max_texture_dimension_2d` is 8192; the
        // bake target is square and the readback allocates a 4-byte row per
        // texel, so this also bounds the readback at 64 MB.
        assert!(
            CLUSTER_BAKE_PX <= 8192,
            "a {CLUSTER_BAKE_PX} px bake target risks a device-limit rejection at world entry"
        );
    }

    /// THE CI GATE for the mip build: every level's above-cutoff coverage
    /// must stay within 5% of level 0's.
    ///
    /// Plain box filtering makes an alpha-tested silhouette LOSE coverage at
    /// every level - a 256 px sprite drawn 6 px wide keeps a fraction of the
    /// texels it should - so the canopy visibly thins with distance and the
    /// cutout crawls as the camera moves. That failure already bit this baker
    /// once (the fir "baked nearly bare", and the cutoff was dropped 0.5 ->
    /// 0.3 to paper over it).
    #[test]
    fn cluster_mip_chain_preserves_alpha_coverage() {
        let size = CLUSTER_SPRITE_PX;
        for (blobs, r) in [(60u32, 9.0f32), (160, 5.0), (25, 20.0)] {
            let base = baked_sprite(blobs, r);
            let levels = build_mip_chain(&base, size, CLUSTER_ALPHA_CUTOFF, true);
            let target = alpha_coverage(&base, CLUSTER_ALPHA_CUTOFF);
            assert!(target > 0.05 && target < 0.95, "degenerate test sprite: coverage {target}");
            let mut w = size;
            for (i, lvl) in levels.iter().enumerate() {
                assert_eq!(
                    lvl.len(),
                    (w * w * 4) as usize,
                    "level {i} is not {w}x{w}"
                );
                let cov = alpha_coverage(lvl, CLUSTER_ALPHA_CUTOFF);
                let texels = (w * w) as f32;
                // Below 16x16 a level holds so few texels that its coverage
                // quantises coarser than 5% (an 8x8 level steps in 1.6%
                // chunks and a 4x4 in 6.25%), so the gate there is what one
                // texel is worth rather than a percentage nothing could hit.
                let tol = if texels >= 256.0 { target * 0.05 } else { 1.5 / texels };
                eprintln!(
                    "[mip] {blobs} blobs r{r}: level {i} {w}x{w} coverage {cov:.4} vs {target:.4} \
                     (tol {tol:.4})"
                );
                assert!(
                    (cov - target).abs() <= tol.max(0.005),
                    "mip level {i} ({w}x{w}) covers {cov:.3} against level 0's {target:.3} - the \
                     alpha-coverage rescale is gone, so this sprite will thin out with distance"
                );
                w /= 2;
            }
            assert!(levels.len() >= 6, "chain stopped at {} levels", levels.len());
        }
    }

    /// The chain must stop where a level stops being measurable, and every
    /// level must be square and complete (wgpu validates the level count
    /// against the texture size at upload).
    #[test]
    fn cluster_mip_chain_is_complete_down_to_the_floor() {
        let base = baked_sprite(80, 8.0);
        let levels = build_mip_chain(&base, CLUSTER_SPRITE_PX, CLUSTER_ALPHA_CUTOFF, true);
        let mut w = CLUSTER_SPRITE_PX;
        for lvl in &levels {
            assert_eq!(lvl.len(), (w * w * 4) as usize);
            w /= 2;
        }
        assert_eq!(w * 2, CLUSTER_MIP_MIN_PX, "chain did not stop at the {CLUSTER_MIP_MIN_PX}px floor");
    }

    /// The 4x supersample downsample is the only anti-aliasing a cutout
    /// silhouette gets (the bake pipeline is multisample count 1 and this
    /// engine has no MSAA anywhere), so it has to actually average.
    #[test]
    fn box_downsample_averages_coverage_and_shrinks_by_the_factor() {
        // A 4x4 field, half covered: downsampling by 4 must give ONE texel at
        // half alpha, not a nearest-neighbour pick.
        let mut src = vec![0u8; 4 * 4 * 4];
        for i in 0..8 {
            src[i * 4] = 255;
            src[i * 4 + 1] = 255;
            src[i * 4 + 2] = 255;
            src[i * 4 + 3] = 255;
        }
        let out = box_downsample_rgba(&src, 4, 4, 4, false);
        assert_eq!(out.len(), 4, "expected a single RGBA texel");
        assert!((out[3] as i32 - 128).abs() <= 1, "alpha averaged to {}", out[3]);
        // ...and RGB is alpha-weighted, so the transparent half cannot drag
        // the covered half toward black.
        assert!(out[0] > 250, "rgb bled toward the transparent clear: {}", out[0]);
        let big = synthetic_sprite(64, 20, 6.0);
        let small = box_downsample_rgba(&big, 64, 64, 4, true);
        assert_eq!(small.len(), (16 * 16 * 4) as usize);
    }

    /// The bake shader's cluster-card decode must be the SAME arithmetic as
    /// the generator's `encode_card_uv`, or a card samples the wrong column of
    /// its sprite. (The megashader's type-21 branch is checked by
    /// `tree_mesh::tests::card_uv_round_trips_the_ao_code_exactly` plus the
    /// lockstep scan below once the branch lands.)
    #[test]
    fn bake_shader_cluster_decode_matches_the_generator() {
        for line in ["let code = floor(in.uv.x * 0.5);", "let cu = in.uv.x - 2.0 * code;"] {
            assert!(BAKE_WGSL.contains(line), "the bake shader lost `{line}`");
        }
        // And the WGSL cutoff literal must be the Rust constant.
        assert!(
            BAKE_WGSL.contains(&format!("cc.a < {CLUSTER_ALPHA_CUTOFF:.1}")),
            "bake cutoff drifted from CLUSTER_ALPHA_CUTOFF = {CLUSTER_ALPHA_CUTOFF}"
        );
    }

    /// LOCKSTEP with the megashader, conditional until the type-21 branch is
    /// wired: the moment `90-fragment-main.wgsl` grows a cluster-card branch,
    /// its decode and its cutoff must match this baker's exactly. Written this
    /// way so the generator half can ship and be verified on its own without
    /// asserting a shader branch that is not there yet.
    #[test]
    fn megashader_cluster_branch_when_present_matches_the_bake() {
        let wgsl = crate::renderer::shader_loader::assembled_pbr_source();
        let has_21 = wgsl.contains("material_type >= 20.5 && material_type < 21.5");
        if !has_21 {
            eprintln!(
                "[lockstep] 90-fragment-main.wgsl has no type-21 branch yet - cluster cards are \
                 generated and baked but not yet drawn (wiring pending)"
            );
            return;
        }
        // NAME-AGNOSTIC (fixed v0.1089). The check used to demand the literal
        // string `in.uv.x - 2.0 * code`, but the megashader's branch names its
        // local `cc_code` (every local in that 1500-line function carries a
        // branch prefix), so this test went red the day the type-21 branch
        // landed in v0.1088 and stayed red - asserting a variable NAME, not the
        // arithmetic it was written to protect. Recover the identifier from the
        // floor() line and check the decode uses that same one.
        assert!(
            wgsl.contains("floor(in.uv.x * 0.5)"),
            "the type-21 cluster decode in 90-fragment-main.wgsl lost `floor(in.uv.x * 0.5)` - \
             it must match tree_mesh::decode_card_uv and the bake shader exactly"
        );
        let at = wgsl.find("= floor(in.uv.x * 0.5)").expect("checked above");
        let ident: String = wgsl[..at]
            .trim_end()
            .chars()
            .rev()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let want = format!("in.uv.x - 2.0 * {ident}");
        assert!(
            wgsl.contains(&want),
            "the type-21 cluster decode in 90-fragment-main.wgsl does not contain `{want}` - \
             it must match tree_mesh::decode_card_uv and the bake shader exactly"
        );
    }

    // ── Per-species leaf silhouettes (v0.1100) ───────────────────────────

    /// Every species that declares a leaf must produce a leaf: closed parts,
    /// real area, the stated aspect, and normalised into the unit leaf box the
    /// emitter maps from. A silhouette that fails any of these would still
    /// bake something - just something wrong - so the gate is arithmetic
    /// rather than a look at the PNG.
    #[test]
    fn every_declared_leaf_builds_a_normalised_silhouette() {
        let reg = leaf_shape::registry();
        assert!(
            reg.len() >= 8,
            "the silhouette side-parse found {} rows; it reads the same trees.ron the species \
             registry does, so this means the parse broke",
            reg.len()
        );
        let mut declared = 0;
        for s in reg {
            let tris = leaf_shape::triangles(s);
            assert!(!tris.is_empty(), "{}: no leaf triangles at all", s.id);
            if s.family() != leaf_shape::LeafFamily::Deltoid {
                declared += 1;
            }
            let (mut ymin, mut ymax, mut xabs, mut area) = (f32::MAX, f32::MIN, 0.0f32, 0.0f32);
            for t in &tris {
                for q in t {
                    ymin = ymin.min(q[1]);
                    ymax = ymax.max(q[1]);
                    xabs = xabs.max(q[0].abs());
                }
                area += 0.5
                    * ((t[1][0] - t[0][0]) * (t[2][1] - t[0][1])
                        - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1]))
                        .abs();
            }
            eprintln!(
                "[leaf] {:>7} {:>14}: {:>5} tris, y {ymin:.2}..{ymax:.2}, |x| {xabs:.3}, \
                 area {area:.3} of the unit box",
                s.id,
                s.family().key(),
                tris.len()
            );
            assert!(
                (ymax - 1.0).abs() < 1e-3,
                "{}: tip reaches {ymax:.3}, not 1.0 - the normalisation is wrong and the leaf \
                 will bake at the wrong length",
                s.id
            );
            // 1% rather than exact: the normalisation maps x and y by
            // different factors, and under an anisotropic map a ribbon's
            // half-width scale depends on its axis angle. That is solved
            // exactly for an axis along x or y and interpolated between, so a
            // frond full of obliquely set leaflets (acacia) lands a fraction
            // under. Visually irrelevant, arithmetically worth stating.
            assert!(
                (xabs - 0.5).abs() < 0.01,
                "{}: half-width is {xabs:.3}, not 0.5 - x must span 1 so the emitter can scale it \
                 by the leaf's real width",
                s.id
            );
            // A little tissue BEHIND the petiole is correct, not a bug: a
            // palmate leaf has a cordate base and its basal lobes reach out
            // almost horizontally, so their lower margins dip below the
            // attachment point (momiji measures -0.06). What must never
            // happen is a leaf hanging backwards down its own shoot.
            assert!(
                ymin >= -0.15,
                "{}: leaf tissue at y {ymin:.3}, a sixth of a leaf length behind its own petiole",
                s.id
            );
            // Summed triangle area OVERCOUNTS a palmate leaf (seven lobes all
            // rooted at the petiole overlap near it), so this bound is
            // deliberately loose and the real shape comparison below measures
            // the rasterised UNION instead.
            assert!(
                (0.10..1.20).contains(&area),
                "{}: the outline covers {area:.3} of its bounding box - outside anything a leaf \
                 shape reaches",
                s.id
            );
        }
        assert!(
            declared >= 4,
            "only {declared} species declare a leaf shape; the whole point of this arc is that \
             the broadleaves stopped being triangles"
        );
    }

    /// The families must be DISTINGUISHABLE, which is the actual requirement -
    /// a palmate maple and a lobed oak that measure the same are two names for
    /// one blob. Checked through the numbers a silhouette is made of: how much
    /// of the bounding box is filled, and how many times the margin turns
    /// (teeth and lobes both show up as margin reversals).
    #[test]
    fn the_leaf_families_are_actually_different_shapes() {
        // The UNION of the outline, measured by rasterising it, NOT the sum of
        // its triangle areas. On a palmate leaf those differ by a third: seven
        // lobes and a connective centre all overlap around the petiole, and a
        // summed area counts that overlap several times over - which is
        // precisely the reading that would hide a leaf whose sinuses had been
        // webbed shut. What the eye sees is the union.
        let fill = |s: &leaf_shape::LeafSilhouette| -> f32 {
            const PX: u32 = 256;
            let mut img = vec![0u8; (PX * PX * 4) as usize];
            for t in &leaf_shape::triangles(s) {
                let p: Vec<[f32; 2]> = t
                    .iter()
                    .map(|q| [(q[0] + 0.5) * PX as f32, (1.0 - q[1]) * PX as f32])
                    .collect();
                fill_tri_mask(&mut img, PX, &p);
            }
            img.chunks_exact(4).filter(|p| p[3] > 0).count() as f32 / (PX * PX) as f32
        };
        let get = |id: &str| leaf_shape::of(id);
        let (momiji, oak, birch, sakura) =
            (get("momiji"), get("oak"), get("birch"), get("sakura"));
        assert_eq!(momiji.family(), leaf_shape::LeafFamily::Palmate);
        assert_eq!(oak.family(), leaf_shape::LeafFamily::PinnateLobed);
        assert_eq!(birch.family(), leaf_shape::LeafFamily::SerrateOvate);
        // A palmate leaf is CUT, so it fills far less of its box than a simple
        // blade. If this inverts, the sinuses have been webbed over - which is
        // exactly what a centroid fan would do to a concave outline.
        let (fm, fo, fb) = (fill(&momiji), fill(&oak), fill(&birch));
        eprintln!("[family] box fill: momiji {fm:.3}, oak {fo:.3}, birch {fb:.3}");
        assert!(
            fm + 0.03 < fo,
            "the palmate maple fills {fm:.3} against the lobed oak's {fo:.3} - a 7-lobed leaf \
             incised two thirds of the way back cannot be as solid as an oak paddle"
        );
        // Margin turns: count sign changes in the outline's x as it walks. A
        // toothed or lobed margin reverses many times; a plain blade twice.
        let turns = |s: &leaf_shape::LeafSilhouette| -> usize {
            let outs = leaf_shape::outlines(s);
            let o = outs
                .iter()
                .max_by_key(|o| o.len())
                .expect("every leaf has at least one part");
            let mut n = 0usize;
            let mut prev = 0.0f32;
            for w in o.windows(2) {
                let d = w[1][0] - w[0][0];
                if d.abs() > 1e-5 {
                    if prev != 0.0 && d.signum() != prev.signum() {
                        n += 1;
                    }
                    prev = d;
                }
            }
            n
        };
        let (tb, ts) = (turns(&birch), turns(&sakura));
        eprintln!("[family] margin turns: birch {tb}, sakura {ts}");
        assert!(
            tb >= 8,
            "the birch margin reverses only {tb} times - a doubly serrate margin is this \
             species' whole close-range signature"
        );
        assert!(ts >= 6, "the cherry margin reverses only {ts} times, so it is not serrate");
    }

    // ── Conifer needle sprays (v0.1101) ──────────────────────────────────

    /// Rasterise a silhouette into a binary mask, so the tests below can ask
    /// questions about the SHAPE rather than about the parts it was built out
    /// of. Same framing as `the_leaf_families_are_actually_different_shapes`
    /// uses: x -0.5..0.5 across the image, y 1 at the top.
    fn silhouette_mask(s: &leaf_shape::LeafSilhouette, px: u32) -> Vec<u8> {
        let mut img = vec![0u8; (px * px * 4) as usize];
        for t in &leaf_shape::triangles(s) {
            let p: Vec<[f32; 2]> = t
                .iter()
                .map(|q| [(q[0] + 0.5) * px as f32, (1.0 - q[1]) * px as f32])
                .collect();
            fill_tri_mask(&mut img, px, &p);
        }
        img.chunks_exact(4).map(|p| u8::from(p[3] > 0)).collect()
    }

    fn mean_and_cv(v: &[f32]) -> (f32, f32) {
        if v.is_empty() {
            return (0.0, 0.0);
        }
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        let var = v.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / v.len() as f32;
        (mean, var.sqrt() / mean.max(1e-6))
    }

    /// Where every needle's ROOT and TIP sit, recovered from the outlines.
    ///
    /// A needle is a two-sample ribbon, so `outlines` hands back exactly four
    /// points: left base, left tip, right tip, right base. The axis endpoints
    /// are the midpoints of the first/last and the middle pair. Recovering them
    /// this way rather than exposing the parts keeps the test measuring what
    /// the BAKER will draw, through the same public surface.
    ///
    /// The shoot (the one part that spans the whole element) is dropped.
    fn needle_axes(s: &leaf_shape::LeafSilhouette) -> Vec<([f32; 2], [f32; 2])> {
        let outs = leaf_shape::outlines(s);
        let span = |o: &Vec<[f32; 2]>| {
            o.iter().fold(f32::MIN, |m, p| m.max(p[1])) - o.iter().fold(f32::MAX, |m, p| m.min(p[1]))
        };
        let shoot = outs
            .iter()
            .enumerate()
            .max_by(|a, b| span(a.1).partial_cmp(&span(b.1)).expect("finite"))
            .map(|(i, _)| i)
            .expect("a needle family always emits its shoot");
        outs.iter()
            .enumerate()
            .filter(|(i, o)| *i != shoot && o.len() == 4)
            .map(|(_, o)| {
                let mid = |a: [f32; 2], b: [f32; 2]| [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])];
                (mid(o[0], o[3]), mid(o[1], o[2]))
            })
            .collect()
    }

    /// THE CONIFER GATE. A fir spray must read FLAT AND TWO-RANKED; a pine
    /// shoot must read as SPLAYED BUNDLES. Both must read as needles on a twig
    /// rather than as any kind of blade.
    ///
    /// Written as measurements rather than as a look at the PNG because the
    /// failure this replaces is not subtle-but-visible, it is invisible: a
    /// conifer drawn as a broadleaf with narrow leaves passes every existing
    /// gate in this file (it normalises, it has area, it bakes, it covers) and
    /// only fails by being the wrong plant.
    #[test]
    fn the_conifer_sprays_are_needled_shoots_not_blades() {
        const PX: u32 = 512;
        let fir = leaf_shape::of("fir");
        let pine = leaf_shape::of("pine");
        assert_eq!(fir.family(), leaf_shape::LeafFamily::NeedleFlatRank);
        assert_eq!(pine.family(), leaf_shape::LeafFamily::NeedleFascicle);
        assert!(fir.family().is_needle_shoot() && pine.family().is_needle_shoot());

        // STRUCTURE. The part count is exactly the arrangement: a shoot plus
        // two needles per station for the fir, a shoot plus one FASCICLE of
        // `leaf_leaflets` needles per station for the pine. If either of these
        // drifts, the species stopped being built the way its data says.
        let fir_parts = leaf_shape::outlines(&fir).len();
        let pine_parts = leaf_shape::outlines(&pine).len();
        assert_eq!(
            fir_parts,
            1 + 2 * fir.leaf_lobes as usize,
            "the fir spray is not one shoot plus {} needle PAIRS",
            fir.leaf_lobes
        );
        assert_eq!(
            pine_parts,
            1 + pine.leaf_lobes as usize * pine.leaf_leaflets as usize,
            "the pine shoot is not one twig plus {} fascicles of {}",
            pine.leaf_lobes,
            pine.leaf_leaflets
        );

        // BUNDLES. A pine's needles must share their roots in groups of
        // `leaf_leaflets`; a fir's must not share them at all, because a fir
        // has no fascicles - Abies bears its needles singly.
        let cluster = |s: &leaf_shape::LeafSilhouette| -> Vec<usize> {
            let mut groups: Vec<([f32; 2], usize)> = Vec::new();
            for (root, _) in needle_axes(s) {
                match groups
                    .iter_mut()
                    .find(|(g, _)| (g[0] - root[0]).abs() < 1e-4 && (g[1] - root[1]).abs() < 1e-4)
                {
                    Some((_, n)) => *n += 1,
                    None => groups.push((root, 1)),
                }
            }
            groups.into_iter().map(|(_, n)| n).collect()
        };
        let pine_groups = cluster(&pine);
        eprintln!(
            "[needle] pine: {} needles in {} fascicles, sizes {:?}",
            pine_parts - 1,
            pine_groups.len(),
            &pine_groups[..pine_groups.len().min(6)]
        );
        assert_eq!(
            pine_groups.len(),
            pine.leaf_lobes as usize,
            "the pine's needles do not group into {} fascicles - they are being emitted \
             individually, which is a fir with longer needles",
            pine.leaf_lobes
        );
        assert!(
            pine_groups.iter().all(|&n| n == pine.leaf_leaflets as usize),
            "a pine fascicle came out with the wrong needle count: {pine_groups:?}"
        );
        assert!(
            cluster(&fir).iter().all(|&n| n == 1),
            "the fir grew fascicles; Abies bears its needles singly, in two ranks"
        );

        // SHAPE. Both are mostly air - that is what a needled shoot IS, and it
        // is the property a blade cannot have (the broadleaves in this registry
        // fill 0.43-0.77 of their box).
        let (fm, pm) = (silhouette_mask(&fir, PX), silhouette_mask(&pine, PX));
        let fill = |m: &[u8]| m.iter().filter(|v| **v != 0).count() as f32 / (PX * PX) as f32;
        let (ff, pf) = (fill(&fm), fill(&pm));
        eprintln!("[needle] box fill: fir {ff:.3}, pine {pf:.3}");
        for (id, f) in [("fir", ff), ("pine", pf)] {
            assert!(
                (0.02..0.35).contains(&f),
                "{id}: the spray fills {f:.3} of its box - a needled shoot is mostly sky, and \
                 anything solid enough to leave this band is a blade wearing a conifer's name"
            );
        }

        // THE READ, and the measurement that actually separates the two
        // families: how far each needle's TIP stands off the shoot axis.
        //
        // A pectinate fir spray is a COMB - every needle is broadside to the
        // viewer at the same angle, so the tips all stand off by nearly the
        // same amount and the only spread is the gentle shortening toward the
        // shoot's growing tip. A pine's fascicles spiral all round the twig, so
        // a viewer sees each at its own azimuth: bundles lying across the view
        // reach full length, bundles pointing at or away from the camera
        // collapse toward the axis. That spread IS the tufted read, and a pine
        // drawn without it is a herringbone, i.e. a fir with longer needles.
        //
        // (Measured on tips rather than on image rows: a comb is mostly GAPS,
        // so a row-by-row reach is dominated by the bare rows between needles
        // and says nothing about either arrangement.)
        let tip_reach = |s: &leaf_shape::LeafSilhouette| -> Vec<f32> {
            needle_axes(s).iter().map(|(_, tip)| tip[0].abs()).collect()
        };
        let (fmean, fcv) = mean_and_cv(&tip_reach(&fir));
        let (pmean, pcv) = mean_and_cv(&tip_reach(&pine));
        eprintln!(
            "[needle] tip stand-off: fir mean {fmean:.3} cv {fcv:.3}, pine mean {pmean:.3} \
             cv {pcv:.3}"
        );
        assert!(
            fcv < 0.20,
            "the fir's needle tips stand off by {fcv:.2} coefficient of variation - a pectinate \
             Abies spray is an EVEN COMB, every needle broadside at the same angle, so a spread \
             this wide means the two ranks are not being drawn flat"
        );
        assert!(
            pcv > 0.45,
            "the pine's needle tips vary by only {pcv:.2} - its fascicles spiral all round the \
             twig, so the ones pointing at the viewer must FORESHORTEN"
        );

        // RAKE. Every needle must point at least slightly toward the shoot's
        // GROWING TIP, never back down it. A needle raked backward is not a
        // stylistic wrong note, it is an anatomical impossibility - needles are
        // borne on the shoot as it extends - and it is the one error the
        // anisotropic normalisation could introduce silently, because that map
        // squashes the forward component of every needle by the aspect while
        // leaving the sideways component alone.
        for (id, s) in [("fir", &fir), ("pine", &pine)] {
            let axes = needle_axes(s);
            let rake: Vec<f32> = axes.iter().map(|(r, t)| t[1] - r[1]).collect();
            let (mean, _) = mean_and_cv(&rake);
            let back = rake.iter().filter(|d| **d <= 0.0).count();
            eprintln!("[needle] {id} rake: mean {mean:+.4} of a shoot length, {back} backward");
            assert!(
                back == 0,
                "{id}: {back} of {} needles rake BACK down the shoot - a needle is borne on the \
                 shoot as it extends, so it cannot point at its own base",
                axes.len()
            );
        }

        // ...and the fir's two ranks must actually be TWO RANKS: an equal
        // count either side, and a silhouette that is mirror-symmetric about
        // the shoot. The pine's spiral has no reason to be either.
        let (mut lft, mut rgt) = (0usize, 0usize);
        for (_, tip) in needle_axes(&fir) {
            if tip[0] < 0.0 {
                lft += 1;
            } else {
                rgt += 1;
            }
        }
        assert_eq!(lft, rgt, "the fir's ranks carry {lft} and {rgt} needles, not one each per pair");
        let half_fill = |m: &[u8]| -> (f32, f32) {
            let (mut l, mut r) = (0usize, 0usize);
            for y in 0..PX {
                for x in 0..PX {
                    if m[(y * PX + x) as usize] != 0 {
                        if x < PX / 2 {
                            l += 1;
                        } else {
                            r += 1;
                        }
                    }
                }
            }
            (l as f32, r as f32)
        };
        let (fl, fr) = half_fill(&fm);
        let asym = (fl - fr).abs() / (fl + fr);
        eprintln!("[needle] fir left/right asymmetry: {asym:.4}");
        assert!(
            asym < 0.05,
            "the fir spray is {:.1}% lopsided - two ranks either side of one shoot is the \
             definition of pectinate, so this is not one",
            asym * 100.0
        );

        // COST. The whole point of a two-sample ribbon per needle. Compared
        // against the maple, which is the most expensive leaf in the registry
        // and the one flagged as a triangle risk when it landed.
        let (ft, pt) = (leaf_shape::triangles(&fir).len(), leaf_shape::triangles(&pine).len());
        let momiji = leaf_shape::triangles(&leaf_shape::of("momiji")).len();
        eprintln!("[needle] triangles: fir {ft}, pine {pt}, momiji {momiji}");
        for (id, n) in [("fir", ft), ("pine", pt)] {
            assert!(
                n <= momiji,
                "{id}: {n} triangles per spray against the maple leaf's {momiji}. A conifer draws \
                 FAR more of these elements than a broadleaf draws leaves, so a spray that costs \
                 more than the registry's dearest leaf is the wrong trade"
            );
        }
    }

    /// A needle family's `leaf_aspect` is DERIVED, not chosen: the spray's
    /// width falls out of its needle length and the angle the needles leave the
    /// shoot at. State it wrong and `reshape_blades` stretches or squashes
    /// every needle, which is invisible in every other gate here (the outline
    /// still normalises, still has area, still bakes).
    #[test]
    fn conifer_sprays_are_drawn_undistorted() {
        for s in leaf_shape::registry() {
            if !s.family().is_needle_shoot() {
                continue;
            }
            let want = leaf_shape::natural_aspect(s);
            eprintln!(
                "[needle] {:>7}: stated aspect {:.4}, geometry wants {want:.4}",
                s.id, s.leaf_aspect
            );
            assert!(
                s.leaf_aspect > 0.0,
                "{}: a needle family must state its aspect - 0 would keep the generic 0.40 strap \
                 width the mesh builder hands every conifer, which has nothing to do with how long \
                 this species' needles are",
                s.id
            );
            let err = (s.leaf_aspect - want).abs() / want;
            assert!(
                err < 0.05,
                "{}: leaf_aspect {:.4} against the {want:.4} its own needle length and angle \
                 imply ({:.1}% out) - every needle on this shoot is being drawn at the wrong \
                 width-to-length",
                s.id,
                s.leaf_aspect,
                err * 100.0
            );
        }
    }

    /// The reshape must keep the SCATTER and change only the outline. That is
    /// the property the whole approach rests on: the sprig placement, the card
    /// fit and the LAI planner upstream are all measured against a scatter
    /// that must not move.
    #[test]
    fn reshaping_a_sprite_keeps_every_leaf_where_the_generator_put_it() {
        let reg = tree_mesh::registry();
        let t = reg
            .trees
            .iter()
            .find(|t| t.id == "momiji")
            .expect("momiji is in the shipped registry");
        let src = tree_mesh::cluster_sprite_geometry(t, ClusterLayer::Leaf, t.height_m)
            .expect("momiji carries a cluster block");
        let s = leaf_shape::of("momiji");
        let out = leaf_shape::reshape_blades(&src, &s, t.leaf_color, leaf_colour::of("momiji"))
            .expect("the leaf arm emits nothing but blades, so the reshape must apply");
        let leaves = src.vertices.len() / 6;
        assert!(leaves > 50, "only {leaves} blades in the momiji sprite");
        // Every original blade's BASE must still be occupied. Measured as the
        // nearest reshaped vertex, which for a leaf rooted at that base is
        // zero to within the outline's own petiole rounding.
        let mut worst = 0.0f32;
        for g in src.vertices.chunks_exact(6) {
            let (p0, p2) = (g[0].position, g[2].position);
            let base = [
                0.5 * (p0[0] + p2[0]),
                0.5 * (p0[1] + p2[1]),
                0.5 * (p0[2] + p2[2]),
            ];
            let d = out
                .vertices
                .iter()
                .map(|v| {
                    let q = v.position;
                    ((q[0] - base[0]).powi(2) + (q[1] - base[1]).powi(2) + (q[2] - base[2]).powi(2))
                        .sqrt()
                })
                .fold(f32::MAX, f32::min);
            worst = worst.max(d);
        }
        eprintln!(
            "[reshape] momiji: {leaves} blades -> {} tris, worst base drift {worst:.5} m",
            out.indices.len() / 3
        );
        assert!(
            worst < 1e-4,
            "a leaf base moved {worst:.4} m under the reshape - the scatter the LAI fit was \
             measured against has shifted"
        );
        // And the organ tag must survive, or the shader lights the whole
        // canopy as BARK (the v0.1081 black-canopy bug).
        const ORGAN_BIT_LEAF: u32 = 524_288;
        assert!(
            out.vertices
                .iter()
                .all(|v| (v.uv[0].max(0.0).round() as u32) & ORGAN_BIT_LEAF != 0),
            "reshaped foliage lost its Organ::Leaf tag"
        );
    }

    /// A mesh that is not a plain run of `tri2` blades must be REFUSED, not
    /// reinterpreted. The blossom sprite is the live example: it carries tubes
    /// and five-petalled flowers, and reading it six vertices at a time would
    /// turn a cherry blossom into confetti.
    #[test]
    fn the_reshape_refuses_a_mesh_that_is_not_blades() {
        let reg = tree_mesh::registry();
        let t = reg.trees.iter().find(|t| t.id == "sakura").expect("sakura ships");
        let blossom = tree_mesh::cluster_sprite_geometry(t, ClusterLayer::Blossom, t.height_m)
            .expect("sakura carries a blossom layer");
        let s = leaf_shape::of("sakura");
        assert!(
            leaf_shape::reshape_blades(&blossom, &s, t.leaf_color, leaf_colour::of("sakura"))
                .is_none(),
            "the reshape accepted the blossom mesh, which is tubes and petals - it must only \
             ever rewrite a pure run of blades"
        );
        // And the whole-sprite entry point must therefore hand the blossom
        // layer straight through, untouched.
        let through = leaf_shape::sprite_geometry(t, ClusterLayer::Blossom, t.height_m)
            .expect("blossom geometry");
        assert_eq!(
            through.indices.len(),
            blossom.indices.len(),
            "the blossom sprite was rewritten on its way to the baker"
        );
    }

    /// THE SILHOUETTE GATE, and the one that answers the operator's actual
    /// complaint: CPU-bake each clustered species' sprite and check that what
    /// comes out covers what its `coverage` field promised.
    ///
    /// `coverage` is not decoration - it is the number the LAI planner spends
    /// to turn card area into leaf area, so a species whose sprite bakes
    /// thinner than it claims silently misses its target canopy density. The
    /// GPU baker logs this mismatch at world entry; this catches it in CI,
    /// before a build exists.
    ///
    /// Set `HUMANITY_DUMP_LEAF_PNG=1` to also write the sprites and the
    /// individual leaves as PNGs under `debug/leaf_shapes/`.
    #[test]
    fn cpu_baked_sprites_cover_what_the_data_promised() {
        // 512 -> 128 rather than the production 2048 -> 512: coverage is a
        // geometric area fraction and barely moves with resolution (measured
        // within 0.02 across that whole range), while the rasteriser's cost is
        // quadratic in it and this test runs UNOPTIMISED on every `cargo test`.
        // The PNG dump path below still runs at full production size, because
        // the thing being looked at there is detail, not area.
        let (bake, sprite) = (512u32, 128u32);
        let dump = std::env::var("HUMANITY_DUMP_LEAF_PNG").is_ok();
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("debug/leaf_shapes");
        let rows = if dump {
            // The env var doubles as the page size when it parses as one. A
            // maple leaf is legible at 512; a conifer shoot is 48 needle
            // stations end to end, which is 10 px apart at that size - enough
            // to see that it IS a comb, not enough to see which way the teeth
            // point. `HUMANITY_DUMP_LEAF_PNG=2048` is the close-up.
            let page = std::env::var("HUMANITY_DUMP_LEAF_PNG")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .map(|v| v.clamp(128, 4096))
                .unwrap_or(512);
            let leaves = dump_leaf_silhouettes(&dir, page);
            for (id, fam, n) in &leaves {
                eprintln!("[dump] leaf_{id}_{fam}.png ({n} tris)");
            }
            dump_cluster_sprites_cpu(&dir, CLUSTER_BAKE_PX, CLUSTER_SPRITE_PX)
        } else {
            let mut rows = Vec::new();
            for t in tree_mesh::registry().trees.iter() {
                if t.clusters.is_none() {
                    continue;
                }
                for layer in ClusterLayer::ALL {
                    if layer == ClusterLayer::Blossom && t.blossom_frac <= 0.0 {
                        continue;
                    }
                    if let Some(s) = cpu_cluster_sprite(t, layer, bake, sprite) {
                        rows.push((t.id.clone(), layer.key(), s.coverage, s.triangles));
                    }
                }
            }
            rows
        };
        assert!(
            rows.len() >= 4,
            "only {} cluster sprites baked; the registry carries more clustered species",
            rows.len()
        );
        for (id, layer, cov, tris) in &rows {
            let t = tree_mesh::registry()
                .trees
                .iter()
                .find(|t| &t.id == id)
                .expect("row came from the registry");
            let cd = t.clusters.as_ref().expect("clustered");
            let want = cd
                .layer(if *layer == "leaf" { ClusterLayer::Leaf } else { ClusterLayer::Blossom })
                .coverage;
            eprintln!(
                "[cpu-bake] {id:>7} {layer:>7}: {tris:>6} tris, coverage {cov:.3} (data {want:.2})"
            );
            assert!(
                *cov > 0.05,
                "{id} {layer}: the sprite baked all but empty ({cov:.3}) - a card sampling it \
                 would draw nothing"
            );
            // The same 0.15 band the GPU baker warns on, asserted instead of
            // logged: if the two drift, the crown misses its target leaf area.
            assert!(
                (cov - want).abs() <= 0.15,
                "{id} {layer}: bakes {cov:.3} coverage against the {want:.2} its data spends on \
                 the LAI fit - correct `coverage` in data/vegetation/trees.ron"
            );
        }
    }

    /// THE COLOUR GATE (v0.1109), and the one that answers "why does the
    /// canopy look cartoony".
    ///
    /// It measured 1 - ONE distinct RGB triple over 130,407 covered texels on
    /// oak, one over 98,094 on sakura - which is the whole complaint stated as
    /// a number. A gate that can only pass is worthless, so this one was run
    /// RED first and reported at 1 before the jitter existed.
    ///
    /// The three assertions are different questions:
    ///   - DISTINCT catches the total-collapse case (an invariant colour, a
    ///     jitter accidentally keyed on a constant, a bake that lost the
    ///     packed channel).
    ///   - HUE SD catches the case that number cannot: 4000 shades of one hue,
    ///     which is still cel shading. The band is centred on the 16.9 degrees
    ///     measured on this repo's own CC0 conifer photoscan.
    ///   - SATURATION SD catches a jitter that walks toward grey, which is
    ///     what jittering R, G and B independently does.
    ///
    /// MEASURED at the time of writing (against the photoscan's 17.4 deg /
    /// 0.113 / 0.093 through this same function): fir 21405 distinct, 12.6
    /// deg, 0.089, 0.118; pine 18437, 13.3, 0.079, 0.073; sakura 4235, 18.8,
    /// 0.091, 0.117; momiji 6511, 16.7, 0.118, 0.134; oak 7678, 14.7, 0.091,
    /// 0.103; birch 6138, 16.3, 0.129, 0.157; acacia 19657, 11.4, 0.057,
    /// 0.069. SAKURA IS THE THIN MARGIN on the distinct count (4235 against
    /// 4000): its leaf sprite carries the fewest elements in the registry, so
    /// if this gate ever fails on sakura alone, check `leaf.sprite_elements`
    /// before suspecting the colour model.
    #[test]
    fn cluster_sprites_carry_real_per_leaf_colour_variation() {
        // 1024 -> 256 rather than the production 2048 -> 512. Colour spread is
        // a per-leaf statistic and a leaf is ~10 texels across at 256, so it
        // is fully resolved; the rasteriser is quadratic in resolution and
        // this runs unoptimised on every `cargo test`.
        let (bake, sprite) = (1024u32, 256u32);
        // The reference, measured with THIS function so the comparison is
        // apples to apples rather than two similar-sounding numbers. Dev
        // checkout only (the release bundle ships no assets/models), so it is
        // informational and never gates.
        let refpath = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "assets/models/plants/pine_sapling_small/textures/\
             pine_sapling_small_twig_diff_a_1k.png",
        );
        if let Ok(img) = image::open(&refpath) {
            let r = leaf_colour::stats(img.to_rgba8().as_raw(), CLUSTER_ALPHA_CUTOFF);
            eprintln!(
                "[reference] real conifer photoscan: {} covered, hue SD {:.1} deg, sat SD \
                 {:.3}, val SD {:.3}, {} distinct",
                r.covered, r.hue_sd_deg, r.sat_sd, r.val_sd, r.distinct
            );
        }
        // Measure and PRINT every species before asserting anything: a table
        // that stops at the first failure hides whether the fault is one
        // species' data or the model itself.
        let mut rows = Vec::new();
        for t in tree_mesh::registry().trees.iter() {
            if t.clusters.is_none() {
                continue;
            }
            let Some(spr) = cpu_cluster_sprite(t, ClusterLayer::Leaf, bake, sprite) else {
                continue;
            };
            // `HUMANITY_DUMP_LEAF_PNG=1` writes exactly what was measured, so
            // the number and the picture can never be about different images.
            // A rising distinct count is NOT evidence the crown looks better;
            // the PNG is.
            if std::env::var("HUMANITY_DUMP_LEAF_PNG").is_ok() {
                let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("debug/leaf_shapes");
                let _ = std::fs::create_dir_all(&dir);
                // BOTH layers. The blossom sprite is not measured here (its
                // spread is a bud-to-open population, not a leaf's), but the
                // same jitter runs through `tree_mesh::flower`, so it has to
                // be LOOKED at or that path ships unseen.
                for layer in ClusterLayer::ALL {
                    if layer == ClusterLayer::Blossom && t.blossom_frac <= 0.0 {
                        continue;
                    }
                    let Some(s) = cpu_cluster_sprite(t, layer, bake, sprite) else {
                        continue;
                    };
                    if let Some(i) = image::RgbaImage::from_raw(sprite, sprite, s.rgba) {
                        let _ = i.save(dir.join(format!("colour_{}_{}.png", t.id, layer.key())));
                    }
                }
            }
            rows.push((t.id.clone(), leaf_colour::stats(&spr.rgba, CLUSTER_ALPHA_CUTOFF)));
        }
        for (id, s) in &rows {
            eprintln!(
                "[colour] {id:>7} leaf: {} covered, {} distinct, hue {:.0}+-{:.1} deg, sat \
                 {:.3}+-{:.3}, val {:.3}+-{:.3}",
                s.covered,
                s.distinct,
                s.hue_mean_deg,
                s.hue_sd_deg,
                s.sat_mean,
                s.sat_sd,
                s.val_mean,
                s.val_sd
            );
        }
        for (id, s) in &rows {
            assert!(
                s.covered > 5_000,
                "{id}: only {} covered texels - the sprite is too empty to measure",
                s.covered
            );
            assert!(
                s.distinct >= 4_000,
                "{id}: the baked leaf sprite holds {} distinct RGB triples over {} covered \
                 texels. A canopy painted in one colour and shaded only by brightness is cel \
                 shading; see billboard_bake::leaf_colour",
                s.distinct,
                s.covered
            );
            assert!(
                (8.0..28.0).contains(&s.hue_sd_deg),
                "{id}: hue SD {:.1} deg against the 16.9 measured on a real conifer photoscan. \
                 Below the band the crown is one hue value-shaded; above it, the leaves no \
                 longer read as one species",
                s.hue_sd_deg
            );
            assert!(
                s.sat_sd >= 0.055,
                "{id}: saturation SD {:.3} (reference 0.109). A jitter that moves R, G and B \
                 independently walks toward grey and lands here",
                s.sat_sd
            );
        }
        assert!(rows.len() >= 4, "only {} clustered species measured", rows.len());
    }

    /// The jitter must be a HASH of the leaf, not a draw from the scatter's
    /// generator: same key, same colour, forever, and a different key gives a
    /// different colour. Without this the sprite would change every rebuild
    /// and `mean_card_side`'s cache would be handing out stale geometry.
    #[test]
    fn per_leaf_colour_is_deterministic_and_mean_preserving() {
        let v = leaf_colour::LeafVariation::default();
        let base = [0.16, 0.34, 0.13]; // oak
        assert_eq!(
            leaf_colour::jitter(base, v, 7),
            leaf_colour::jitter(base, v, 7),
            "the jitter is not a pure function of its key"
        );
        assert_ne!(
            leaf_colour::jitter(base, v, 7),
            leaf_colour::jitter(base, v, 8),
            "two different leaves got the same colour"
        );
        // The population mean must land back on the species' authored colour,
        // or every row in trees.ron silently means something else. The
        // abaxial/adaxial split is derived to make this exact IN THE
        // PERCEPTUAL DOMAIN, which is where the split's factors are defined;
        // hue rotation and the senescent tail move it a little, which is why
        // the tolerance is a few percent rather than zero.
        let n = 4000;
        let (mut sv, mut ss, mut slin) = (0.0f64, 0.0f64, 0.0f64);
        for k in 0..n {
            let c = leaf_colour::jitter(base, v, k);
            let h = leaf_colour::srgb_hsv(c);
            sv += h[2] as f64;
            ss += h[1] as f64;
            slin += (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]) as f64;
        }
        let want = leaf_colour::srgb_hsv(base);
        let (gv, gs) = (sv / n as f64, ss / n as f64);
        assert!(
            (gv - want[2] as f64).abs() < 0.03 * want[2] as f64 + 0.01,
            "mean value drifted to {gv:.4} from the authored {:.4}",
            want[2]
        );
        assert!(
            (gs - want[1] as f64).abs() < 0.08 * want[1] as f64 + 0.01,
            "mean saturation drifted to {gs:.4} from the authored {:.4}",
            want[1]
        );
        // THE CONSEQUENCE, asserted rather than left to be discovered. A
        // spread that is symmetric in a PERCEPTUAL domain is not symmetric in
        // a RADIOMETRIC one: sRGB decode is convex, so by Jensen's inequality
        // the mean linear luminance of the population sits ABOVE the authored
        // colour's. That is the direction that matters least (crowns get
        // marginally brighter, never darker and never hue-shifted) and it is
        // also the honest one - a real canopy of mixed sun and shade leaves
        // does reflect more than a uniform canopy at the mean colour. Bounded
        // here so it can never quietly grow into a re-lit forest.
        let lin_want =
            (0.2126 * base[0] + 0.7152 * base[1] + 0.0722 * base[2]) as f64;
        let lin_got = slin / n as f64;
        eprintln!(
            "[mean] sRGB value {gv:.4} (authored {:.4}), sat {gs:.4} (authored {:.4}), \
             linear luminance {lin_got:.4} (authored {lin_want:.4}, {:+.1}%)",
            want[2],
            want[1],
            (lin_got / lin_want - 1.0) * 100.0
        );
        assert!(
            lin_got < lin_want * 1.20,
            "mean linear luminance rose to {lin_got:.4} from {lin_want:.4} - the spread is \
             now re-lighting the canopy, not varying it"
        );
    }

    /// A cherry PETAL is routed through the same blade code as a leaf
    /// (`tree_mesh::leaf_cluster` passes `blossom_color` down the same path),
    /// so the senescent-straw term has to be gated on tissue that actually
    /// carries chlorophyll. Without the gate, 4.5% of a blooming cherry's
    /// flowers turn brown.
    #[test]
    fn the_senescent_tail_never_touches_a_petal() {
        let mut v = leaf_colour::LeafVariation::default();
        v.senescent_frac = 1.0; // every element, if it were eligible at all
        let petal = [0.85, 0.55, 0.62]; // sakura blossom_color territory: pink
        for k in 0..200u64 {
            let h = leaf_colour::hsv(leaf_colour::jitter(petal, v, k));
            assert!(
                !(20.0..90.0).contains(&h[0]),
                "a petal turned straw (hue {:.0} deg) on key {k}",
                h[0]
            );
        }
        // ...and on a green leaf the same setting MUST fire, or the gate above
        // is passing because nothing ever senesces (the check-that-cannot-fail
        // class).
        let leaf = [0.16, 0.34, 0.13];
        let straw = (0..200u64)
            .filter(|k| {
                let h = leaf_colour::hsv(leaf_colour::jitter(leaf, v, *k));
                (20.0..90.0).contains(&h[0])
            })
            .count();
        assert!(straw > 100, "only {straw}/200 leaves senesced at frac 1.0");
    }

    /// The bake shader's packed decode must be the SAME arithmetic as the
    /// megashader's type-20 decode, or a procedural card is a different colour
    /// from the 3D tree it replaces.
    #[test]
    fn packed_decode_matches_the_megashader() {
        let wgsl = crate::renderer::shader_loader::assembled_pbr_source();
        for line in [
            "f32((packed >> 8u) & 255u) / 255.0,",
            "f32(packed & 255u) / 255.0,",
        ] {
            assert!(wgsl.contains(line), "megashader lost `{line}`");
            assert!(BAKE_WGSL.contains(line), "bake shader lost `{line}`");
        }
        // And the bake must NOT hand-encode gamma anywhere: the target is
        // sRGB, the decode is linear, the card sampler decodes back.
        assert!(
            !BAKE_WGSL.contains("pow(") && !BAKE_WGSL.contains("2.2"),
            "the bake shader applies a gamma curve - the packed round trip is exact only without one"
        );
    }
}

// ── VEGETATION LIGHTING PARITY (v0.1110) ─────────────────────────────────
//
// Three representations stand in for one tree - the near 3D crown of cluster
// cards (type 21), the patch-baked sprite card (type 12), and the far-sheet
// canopy card (type 12 again) - and the operator can see any disagreement
// between them as a CIRCLE on the ground at the handoff radius. These tests
// are the headless half of keeping them equal; the other half is a probe
// capture, which no static check can replace.
//
// They live here rather than in a lint file because the bake shader is a Rust
// string constant in THIS module, and half the contract is "the bake and the
// render apply the same crown depth".
#[cfg(test)]
mod canopy_parity_tests {
    use super::{canopy_ndl_mean, crown_depth_extinction, BAKE_WGSL};
    use crate::renderer::shader_loader::assembled_pbr_source;

    /// Text between two marker comments, with each line trimmed, so indentation
    /// differences between a Rust raw string and a .wgsl file cannot make two
    /// identical blocks compare unequal.
    fn marked_block(src: &str, name: &str) -> String {
        let begin = format!("// {name} LOCKSTEP BEGIN");
        let end = format!("// {name} LOCKSTEP END");
        let a = src
            .find(&begin)
            .unwrap_or_else(|| panic!("`{begin}` not found - the lockstep markers were removed"));
        let b = src[a..]
            .find(&end)
            .unwrap_or_else(|| panic!("`{end}` not found after `{begin}`"))
            + a;
        src[a + begin.len()..b]
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// THE defect the operator reported, in its albedo half. The atlas bake
    /// decoded each cluster card's crown-depth code out of its uv and then
    /// discarded it, so a baked sprite carried the crown's albedo with none of
    /// the crown's own extinction while the type-21 branch drawing the SAME
    /// cards up close multiplied by exactly that. Byte-identical or nothing.
    #[test]
    fn crown_depth_shade_is_identical_in_the_bake_shader() {
        let render = marked_block(assembled_pbr_source(), "CROWN-DEPTH");
        let bake = marked_block(BAKE_WGSL, "CROWN-DEPTH");
        assert_eq!(
            render, bake,
            "crown_depth_shade has drifted between the megashader and the atlas bake. A card that \
             BAKES with a different crown depth than it RENDERS with is a brightness step at the \
             LOD handoff radius, which reads as a circle on the ground."
        );
        assert!(
            render.contains("fn crown_depth_shade(ao: f32) -> vec3<f32>"),
            "the lockstep block no longer defines crown_depth_shade - the markers are guarding \
             the wrong text"
        );
        // And it must actually be CALLED on both sides, or two identical dead
        // copies would pass.
        assert!(
            BAKE_WGSL.contains("crown_depth_shade("),
            "the bake shader defines crown_depth_shade but never calls it"
        );
        assert!(
            assembled_pbr_source().matches("crown_depth_shade(").count() >= 2,
            "the megashader defines crown_depth_shade but the type-21 branch never calls it"
        );
    }

    /// The bake shader is compiled at runtime on a GPU that no test has, so
    /// parse + validate it here. Same gate the megashader gets.
    #[test]
    fn the_bake_shader_parses_and_validates() {
        let module = wgpu::naga::front::wgsl::parse_str(BAKE_WGSL)
            .unwrap_or_else(|e| panic!("BAKE_WGSL parse error: {e}"));
        let mut v = wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        );
        v.validate(&module)
            .unwrap_or_else(|e| panic!("BAKE_WGSL validation error: {e:?}"));
        // A function returning an array validates here and then fails at
        // device init on the DX12 HLSL backend (the v0.1101 incident).
        for line in BAKE_WGSL.lines() {
            let code = line.split("//").next().unwrap_or("");
            assert!(
                !(code.contains("->")
                    && code
                        .split("->")
                        .nth(1)
                        .is_some_and(|r| r.trim_start().starts_with("array<"))),
                "BAKE_WGSL returns an array from a function: {}",
                code.trim()
            );
        }
    }

    /// The closed form against the thing it is a closed form OF: sample
    /// isotropic leaf normals, weight each by how visible it is, and average
    /// its sun response. If the algebra is wrong the card lands at the wrong
    /// brightness at every sun angle, which is the whole bug.
    #[test]
    fn canopy_ndl_mean_matches_monte_carlo() {
        // Deterministic low-discrepancy sphere sampling (Fibonacci lattice).
        const N: usize = 200_003;
        let ga = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
        let mut dirs = Vec::with_capacity(N);
        for i in 0..N {
            let z = 1.0 - 2.0 * (i as f64 + 0.5) / N as f64;
            let r = (1.0 - z * z).max(0.0).sqrt();
            let th = ga * i as f64;
            dirs.push([r * th.cos(), r * th.sin(), z]);
        }
        for deg in [0.0_f64, 20.0, 45.0, 90.0, 135.0, 180.0] {
            let psi = deg.to_radians();
            let l = [0.0_f64, 0.0, 1.0];
            let v = [psi.sin(), 0.0, psi.cos()];
            let (mut num, mut den) = (0.0_f64, 0.0_f64);
            for n in &dirs {
                let ndl = n[0] * l[0] + n[1] * l[1] + n[2] * l[2];
                let ndv = (n[0] * v[0] + n[1] * v[1] + n[2] * v[2]).abs();
                num += ndl.max(0.0) * ndv;
                den += ndv;
            }
            let measured = num / den;
            let closed = canopy_ndl_mean(psi.cos() as f32) as f64;
            assert!(
                (measured - closed).abs() < 2.0e-3,
                "psi = {deg} deg: sampled {measured:.5}, closed form {closed:.5}"
            );
        }
        // The three anchor values quoted in the shader comment.
        assert!((canopy_ndl_mean(1.0) - 1.0 / 3.0).abs() < 1.0e-4);
        assert!((canopy_ndl_mean(0.0) - 2.0 / (3.0 * std::f32::consts::PI)).abs() < 1.0e-5);
        assert!((canopy_ndl_mean(-1.0) - 1.0 / 3.0).abs() < 1.0e-4);
    }

    /// THE MECHANISM behind the parity ratio, measured directly.
    ///
    /// `canopy_ndl_mean` is the mean of `max(N.L, 0)` over a leaf-normal
    /// distribution with NO net direction. A crown whose shading normals DO
    /// have a net direction therefore cannot match it, and the gap does not
    /// average out, because the same normal rides both windings of every card
    /// (`tree_cards::emit_card`) so `max()` never sees the opposing face.
    ///
    /// Before `balance_card_normals`, this printed, as (|m|, m_y):
    ///
    ///   fir leaf    0.1017  -0.1015      oak leaf     0.0367  +0.0218
    ///   pine leaf   0.0995  -0.0992      sakura blsm  0.0242  +0.0210
    ///   sakura leaf 0.0521  +0.0160      acacia leaf  0.0197  -0.0030
    ///   birch leaf  0.0436  +0.0315      momiji leaf  0.0166  +0.0025
    ///
    /// With the sun straight up, the SIGN of m_y predicts the sign of
    /// (card/crown - 1) for all seven species without exception: the two
    /// crowns whose normals lean net DOWNWARD (fir, pine) were the two whose
    /// cards came out brighter than them, at 1.30 and 1.29. That much is
    /// derived. The magnitude ordering is only ROUGHLY monotone - acacia's
    /// moment is nearly horizontal (-0.0136, -0.0030, +0.0139) yet it still
    /// deviated 0.064, which is why the correction barely moves it and why
    /// its residual survives in the ratio gate. Nothing here explains WHY a
    /// given crown's moment points where it does; several geometric stories
    /// have been offered and refuted, so do not add one without establishing
    /// it.
    ///
    /// This is the sharper of the two gates: the ratio test sees only the
    /// consequence, which other effects also move.
    #[test]
    fn card_layer_shading_normals_have_no_net_direction() {
        use crate::renderer::tree_mesh;
        let mut checked = 0usize;
        for def in tree_mesh::registry().trees.iter().filter(|t| t.clusters.is_some()) {
            let built = tree_mesh::build_tree_and_cards(def, def.height_m, 0x51F0_A11C);
            for layer in &built.cards {
                let vs = &layer.mesh.vertices;
                assert!(vs.len() >= 600, "{}: only {} card vertices", def.id, vs.len());
                // Area-weighted, exactly the weighting the correction solves
                // against: each corner carries a third of its triangle.
                let (mut m, mut tot) = ([0.0_f64; 3], 0.0_f64);
                for t in vs.chunks_exact(3) {
                    let e1 = std::array::from_fn::<f64, 3, _>(|k| {
                        (t[1].position[k] - t[0].position[k]) as f64
                    });
                    let e2 = std::array::from_fn::<f64, 3, _>(|k| {
                        (t[2].position[k] - t[0].position[k]) as f64
                    });
                    let x = [
                        e1[1] * e2[2] - e1[2] * e2[1],
                        e1[2] * e2[0] - e1[0] * e2[2],
                        e1[0] * e2[1] - e1[1] * e2[0],
                    ];
                    let a = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt() / 6.0;
                    for v in t {
                        for k in 0..3 {
                            m[k] += a * v.normal[k] as f64;
                        }
                    }
                    tot += a * 3.0;
                }
                assert!(tot > 0.0, "{}: zero card area", def.id);
                let m = [m[0] / tot, m[1] / tot, m[2] / tot];
                let mag = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt();
                eprintln!(
                    "[moment] {:>12} {:?}: |m| = {mag:.5}  m = ({:.5}, {:.5}, {:.5})",
                    def.id, layer.layer, m[0], m[1], m[2]
                );
                checked += 1;
                // 0.004 sits between the two measured scales: 4.1x under the
                // SMALLEST pre-fix moment (momiji, 0.0166) and 2.3x over the
                // LARGEST residual the solve leaves (sakura's leaf layer,
                // 0.00172 - the solve is exact along m_hat, so what is left is
                // perpendicular drift from renormalising). So it catches a
                // regression without gating on bisection noise.
                assert!(
                    mag <= 0.004,
                    "{}/{:?}: the area-weighted mean shading normal is ({:.4}, {:.4}, {:.4}), \
                     |m| = {mag:.4}. A card layer standing in for a canopy has to average to no \
                     net direction, because the far card's kernel `canopy_ndl_mean` assumes \
                     exactly that - a moment this size IS the card/crown brightness step at the \
                     LOD handoff radius. Check that `emit_cluster_cards` still calls \
                     `tree_cards::balance_card_normals` on every finished layer, and that \
                     nothing rewrites the normals after it.",
                    def.id,
                    layer.layer,
                    m[0],
                    m[1],
                    m[2],
                );
            }
        }
        assert!(checked >= 7, "only {checked} card layers reached the moment gate");
    }

    /// And the closed form against the REAL crown: build the shipped species'
    /// cluster cards, weight every card by how much of it a viewer sees, and
    /// compare the crown's own mean sun response with the kernel a card uses
    /// to stand in for it.
    ///
    /// This is the check that would have caught the bug: a flat card facing
    /// the sky returns `max(up . L, 0)`, which at a high sun is ~1.0 against
    /// the crown's ~0.33 - a 3x step at the handoff radius.
    #[test]
    fn canopy_kernel_matches_the_near_crown() {
        use crate::renderer::tree_mesh;
        let reg = tree_mesh::registry();
        assert!(!reg.is_empty(), "no species registry - the parity has nothing to measure");

        // Sun straight up, viewer sweeping from head-on to across: the
        // geometry a player walking a forest at midday actually has.
        let l = [0.0_f32, 1.0, 0.0];
        let mut measured_extinction = Vec::new();
        for def in reg.trees.iter().filter(|t| t.clusters.is_some()) {
            let built = tree_mesh::build_tree_and_cards(def, def.height_m, 0x51F0_A11C);
            let mut tris: Vec<([f32; 3], f32, f32)> = Vec::new(); // normal, area, ao
            for layer in &built.cards {
                let vs = &layer.mesh.vertices;
                for f in layer.mesh.indices.chunks_exact(3) {
                    let (a, b, c) =
                        (&vs[f[0] as usize], &vs[f[1] as usize], &vs[f[2] as usize]);
                    let e1 = [
                        b.position[0] - a.position[0],
                        b.position[1] - a.position[1],
                        b.position[2] - a.position[2],
                    ];
                    let e2 = [
                        c.position[0] - a.position[0],
                        c.position[1] - a.position[1],
                        c.position[2] - a.position[2],
                    ];
                    let cr = [
                        e1[1] * e2[2] - e1[2] * e2[1],
                        e1[2] * e2[0] - e1[0] * e2[2],
                        e1[0] * e2[1] - e1[1] * e2[0],
                    ];
                    let area =
                        0.5 * (cr[0] * cr[0] + cr[1] * cr[1] + cr[2] * cr[2]).sqrt();
                    if area <= 0.0 {
                        continue;
                    }
                    // Shading normal is the per-vertex one the shader gets.
                    let mut n = [0.0_f32; 3];
                    for v in [a, b, c] {
                        for k in 0..3 {
                            n[k] += v.normal[k] / 3.0;
                        }
                    }
                    let ln = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
                    let n = [n[0] / ln, n[1] / ln, n[2] / ln];
                    // AO code, decoded exactly as the type-21 branch does.
                    let ao = (a.uv[0] * 0.5).floor() / 63.0;
                    tris.push((n, area, ao));
                }
            }
            assert!(tris.len() > 200, "{}: only {} card triangles", def.id, tris.len());

            for deg in [0.0_f32, 30.0, 60.0, 90.0] {
                let psi = deg.to_radians();
                let v = [psi.sin(), psi.cos(), 0.0];
                let (mut num, mut den, mut ext) = (0.0_f32, 0.0_f32, 0.0_f32);
                for (n, area, ao) in &tris {
                    let ndv = (n[0] * v[0] + n[1] * v[1] + n[2] * v[2]).abs() * area;
                    let ndl = (n[0] * l[0] + n[1] * l[1] + n[2] * l[2]).max(0.0);
                    num += ndv * ndl;
                    den += ndv;
                    ext += ndv * crown_depth_extinction(*ao);
                }
                let crown = num / den;
                let card = canopy_ndl_mean(
                    l[0] * v[0] + l[1] * v[1] + l[2] * v[2],
                );
                if deg == 0.0 {
                    measured_extinction.push((def.id.clone(), ext / den));
                }
                let ratio = card / crown.max(1e-6);
                // THE CHECK MUST BE ABLE TO FAIL. `plate` is precisely what
                // the card did through v0.1109: evaluate_light against the
                // card's radial-up shading normal, i.e. max(up . L, 0). With
                // the sun overhead that is 1.0 against a crown's ~0.30, so the
                // band below rejects it - on every run, not just in theory.
                let plate = (l[0] * 0.0 + l[1] * 1.0 + l[2] * 0.0_f32).max(0.0);
                let plate_ratio = plate / crown.max(1e-6);
                eprintln!(
                    "[canopy] {:>12} psi={deg:>5.1}  crown {crown:.4}  card {card:.4}  \
                     card/crown {ratio:.3}  (pre-v0.1110 flat plate {plate:.4}, \
                     x{plate_ratio:.2})",
                    def.id
                );
                assert!(
                    !(0.80..=1.30).contains(&plate_ratio),
                    "the pre-v0.1110 flat-plate response would PASS this band, so the band is \
                     not measuring anything"
                );
                // ONE BAND, ONE LEVEL, EVERY SPECIES.
                //
                // This table used to carry a 1.28 exception for fir and pine
                // inside a +/-0.12 band, on the reading that a narrow conifer
                // crown genuinely has a non-spherical leaf-angle distribution.
                // That reading was wrong. The cause was the ODD component of
                // the shading normals - see
                // `card_layer_shading_normals_have_no_net_direction`, which
                // measures it directly - and zeroing it in
                // `tree_cards::balance_card_normals` brought every species to
                // the same level, so the exception is gone and the band is
                // tighter. Measured card/crown over the 4 sun angles, before
                // and after (fir is the extreme, acacia the leftover):
                //
                //   fir     1.304 1.286 1.261 1.280  ->  0.989 0.994 1.008 1.042
                //   pine    1.292 1.275 1.253 1.273  ->  0.988 0.993 1.008 1.043
                //   birch   0.919 0.929 0.942 0.954  ->  0.996 0.996 0.995 1.013
                //   oak     0.929 0.934 0.949 0.972  ->  0.982 0.977 0.982 1.014
                //   sakura  0.944 0.947 0.969 0.958  ->  0.993 0.992 1.009 0.998
                //   momiji  0.996 1.013 1.050 1.037  ->  1.001 1.006 1.041 1.044
                //   acacia  1.064 1.078 1.044 1.020  ->  1.060 1.064 1.026 1.013
                //
                // Worst deviation from 1.00 fell from 0.304 to 0.064, so 0.08
                // is a snug band rather than a comfortable one: acacia has
                // 0.016 to spare. Acacia is the honest leftover - its first
                // moment was already the second smallest (0.0270), so the
                // correction barely moved it and what remains is a HIGHER
                // moment of its normal distribution, which this fix does not
                // address. If you need room here, measure that; do not widen
                // the band, and do not reintroduce a per-species level.
                let expect: f32 = 1.00;
                assert!(
                    (ratio - expect).abs() <= 0.08,
                    "{id} at psi={deg}: the card kernel returns {card:.4} where the real crown \
                     averages {crown:.4} (x{ratio:.2}, expected x{expect:.2}). A ratio like that \
                     IS the bright ring at the LOD handoff radius. First read the companion gate \
                     `card_layer_shading_normals_have_no_net_direction`: if it also failed, the \
                     cause is the crown's first moment and the fix belongs in \
                     `tree_cards::balance_card_normals`, not here. If it PASSED, the moment is \
                     already zero and something else moved the geometry - check \
                     tree_cards::sleeve_tilts, CLUSTER_NORMAL_BLEND and CROWN_NORMAL_BLEND. \
                     Never widen the tolerance, and never re-add a per-species level, to make a \
                     number fit.",
                    id = def.id,
                );
            }
        }
        for (id, e) in &measured_extinction {
            eprintln!("[canopy] {id:>12} mean crown-depth extinction {e:.4}");
            assert!(
                (0.4..=1.0).contains(e),
                "{id}: mean crown extinction {e:.3} is outside the range crown_depth_shade can \
                 produce (0.35..1.0) - the AO decode has drifted"
            );
        }
    }

    /// `CANOPY_SKY_FRACTION` is not a taste value: it is exactly what
    /// `sky_ambient` returns for the isotropic-leaf mean of its own
    /// orientation weight, so it has to track `SKY_GROUND_BOUNCE`. It is
    /// written as a literal only because that constant is declared in a LATER
    /// shader part.
    #[test]
    fn canopy_sky_fraction_matches_the_ground_bounce() {
        let src = assembled_pbr_source();
        let num = |name: &str| -> f32 {
            let at = src
                .find(&format!("const {name}: f32 = "))
                .unwrap_or_else(|| panic!("{name} missing from the megashader"));
            let rest = &src[at + format!("const {name}: f32 = ").len()..];
            rest.chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect::<String>()
                .parse()
                .unwrap_or_else(|_| panic!("{name} is not a plain literal any more"))
        };
        let g = num("SKY_GROUND_BOUNCE");
        let f = num("CANOPY_SKY_FRACTION");
        let want = 0.5 * (1.0 + g);
        assert!(
            (f - want).abs() < 0.005,
            "CANOPY_SKY_FRACTION is {f} but SKY_GROUND_BOUNCE = {g} makes the isotropic-leaf \
             mean {want}. A card would then take a different share of the sky than the crown it \
             replaces."
        );
    }

    /// BUG-064's shape, generalised: a rule applied to two of three foliage
    /// branches. Every card family in the fragment shader must apply the SAME
    /// canopy block, and every sun-derived transmission term must be gated by
    /// the shadow map (BUG-060).
    #[test]
    fn every_vegetation_branch_agrees_about_light() {
        let src = assembled_pbr_source();
        let fs = &src[src.find("fn fs_main").expect("fs_main missing")
            ..src.find("fn fs_shadow").expect("fs_shadow missing")];

        // 1. The canopy block, verbatim, once per card family. Card families
        //    in fs_main: the type-12 sprite branch, and the type-12 packed
        //    branch (legacy silhouettes + the far sheet).
        let block = [
            "let vc = canopy_card_shading(normal, camera.sun_direction.xyz, view_dir);",
            "sun_gate = sun_gate * vc.sun_gain;",
            "ao = vc.sky_frac;",
        ];
        let calls = fs.matches(block[0]).count();
        assert_eq!(
            calls, 2,
            "fs_main applies the canopy kernel at {calls} sites; there are 2 card families \
             (the sprite branch and the packed branch). A card family without it is lit as a \
             flat plate facing the sky, which is the v0.1109 bright ring."
        );
        for line in block {
            assert_eq!(
                fs.matches(line).count(),
                calls,
                "the canopy block is not identical at every site - `{line}` appears a different \
                 number of times than the call to canopy_card_shading"
            );
        }

        // 2. No sun-derived additive term may skip the shadow map (BUG-060).
        for stmt in fs.split("proc_emissive = proc_emissive").skip(1) {
            let body = &stmt[..stmt.find(';').unwrap_or(stmt.len())];
            if !body.contains("camera.sun_color") {
                continue;
            }
            assert!(
                body.contains("shadow"),
                "a sun-derived proc_emissive term has no shadow factor - a leaf in shadow \
                 receives no sun to transmit:\n{body}"
            );
        }
    }
}
