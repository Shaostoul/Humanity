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
use super::plant_mesh::PlantMeshBuilder;
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
        return vec4<f32>(cc.rgb, 1.0);
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
pub const CLUSTER_SPRITE_PX: u32 = 256;

/// Side it is RENDERED at before the box downsample. The bake pipeline is
/// multisample count 1 and this engine has no MSAA anywhere, so the 4x
/// supersample is the only anti-aliasing a cutout silhouette is ever going to
/// get - and an alpha test amplifies whatever jaggedness survives.
pub const CLUSTER_BAKE_PX: u32 = 1024;

/// Alpha at or above which a cluster texel counts as covered. Keep in lockstep
/// with the type-21 discard in `assets/shaders/pbr/90-fragment-main.wgsl` and
/// the cluster branch of BAKE_WGSL above.
pub const CLUSTER_ALPHA_CUTOFF: f32 = 0.5;

/// Smallest mip level built. Stopping at 4x4 is deliberate: below that a level
/// holds fewer texels than the coverage gate can meaningfully measure (a 2x2
/// level quantises coverage to 25% steps), and a 0.5 m card is ~4 px on screen
/// at the 120 m model/card handoff, so nothing smaller is ever sampled.
pub const CLUSTER_MIP_MIN_PX: u32 = 4;

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
                let cached_proc = t
                    .is_procedural()
                    .then(|| models.get(&proc_key(&t.id, v)))
                    .flatten();
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
                    tree_mesh::build_tree_and_cards(t, t.height_m, v.wrapping_mul(2_654_435_761)).1
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
                let Some(mesh) = tree_mesh::cluster_sprite_geometry(t, layer, t.height_m) else {
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

// ── Cluster mip chain (pure CPU, so the gate runs in CI) ─────────────────

fn srgb_to_linear(u: u8) -> f32 {
    let c = u as f32 / 255.0;
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
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

    /// The production path in miniature: a BINARY silhouette rendered at
    /// `CLUSTER_BAKE_PX` and box-downsampled to `CLUSTER_SPRITE_PX`, which is
    /// what gives level 0 its smooth partial-coverage alpha. Testing the mip
    /// build against a binary 256 px image instead would be testing a case
    /// the baker never produces.
    fn baked_sprite(blobs: u32, r: f32) -> Vec<u8> {
        let hi = synthetic_sprite(CLUSTER_BAKE_PX, blobs, r * (CLUSTER_BAKE_PX / CLUSTER_SPRITE_PX) as f32);
        box_downsample_rgba(
            &hi,
            CLUSTER_BAKE_PX,
            CLUSTER_BAKE_PX,
            CLUSTER_BAKE_PX / CLUSTER_SPRITE_PX,
            true,
        )
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
        for line in ["floor(in.uv.x * 0.5)", "in.uv.x - 2.0 * code"] {
            assert!(
                wgsl.contains(line),
                "the type-21 cluster decode in 90-fragment-main.wgsl does not contain `{line}` - \
                 it must match tree_mesh::decode_card_uv and the bake shader exactly"
            );
        }
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
