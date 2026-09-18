//! Render pipeline management — creates and caches wgpu render pipelines.

use super::camera::CameraUniforms;
use super::mesh::Vertex;
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;

/// GPU-side object transform uniforms (matches shader ObjectUniforms).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ObjectUniforms {
    pub model: [[f32; 4]; 4],
    pub normal_matrix: [[f32; 4]; 4],
}

/// GPU-side material uniforms (matches shader MaterialUniforms).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct MaterialUniforms {
    pub base_color: [f32; 4],
    /// x = metallic, y = roughness, z = material_type, w = emissive strength.
    /// WARNING: w is REPURPOSED as a data channel by several material types
    /// (12: surface bitfield, 15: cloud shell ratio, 18: gas-giant palette
    /// index) - safe only because their shader branches never reach the
    /// generic emissive path. Never treat z/w as free space without checking
    /// the type dispatch in 90-fragment-main.wgsl. (The old comment here said
    /// "z/w unused", which is exactly how accidental-glow bugs get written.)
    pub params: [f32; 4],
    /// Second per-material data vector (clouds depth increment): the first
    /// 8 floats were fully subscribed, so per-planet physical data had no
    /// channel to the shader. Semantics are per-material-type; today only
    /// the cloud shell (type 15) uses it: x = slab BASE as a planet-radius
    /// multiple, y = slab TOP, z = planet radius in KM (converts the
    /// metre-expressed noise ladder into drawn-shell units), w = 1 when the
    /// dev coverage pin is active (cloud_weather then ignores the live
    /// MODIS placement so a verification vantage always has clouds).
    /// Zero (every other material) means "no data" - shader paths must
    /// treat 0 as absent, never as a value.
    pub params2: [f32; 4],
}

/// PBR-lite render pipeline with three bind group layouts.
///
/// Since increment P3 of the frame-cost arc (docs/design/frame-cost-arc.md)
/// the megashader PSOs are one per (material CLASS, fixed-function STATE)
/// pair the draw loops actually use, and each is compiled from its class's
/// OWN fragment entry (`ShaderClass::fragment_entry`, the six `fs_*`
/// entries of `assets/shaders/pbr/90-fragment-main.wgsl`), so a wall
/// fragment never carries the terrain block and only the cloud shell's
/// fragments ever see the volumetric march. Never reach for these fields
/// directly from a draw loop: `opaque_for`, `transparent_for` and
/// `overlay_for` pick the PSO by the material's class (`shader_class`), so
/// a material can never be drawn by a pipeline whose fragment entry does
/// not contain its code.
pub struct Pipeline {
    /// SURFACE class (the procedural surfaces 0..11, the sun's radial glow
    /// 17, gas giant bands 18, textured meshes 19, screens 24), OPAQUE
    /// state: REPLACE blend, back-face cull, depth write. Walls, floors,
    /// props, machines, photoscanned trees, planet bodies that are not
    /// terrain.
    pub surface_render_pipeline: wgpu::RenderPipeline,
    /// Surface class, TRANSPARENT state (v0.456): alpha blend over the
    /// scene, double-sided, depth-TESTED but not written, so you see
    /// THROUGH it. Glass and windows, holograms, particles, the sun's
    /// blended core and its halo.
    pub surface_transparent_pipeline: wgpu::RenderPipeline,
    /// Surface class, OVERLAY state (v0.560): alpha blend, double-sided,
    /// depth WRITE into a depth the pass cleared first, so build-mode gizmos
    /// (corner orbs, the avatar, rings) sort among themselves yet draw ON
    /// TOP of the world, visible through walls and floors.
    pub surface_overlay_pipeline: wgpu::RenderPipeline,
    /// TERRAIN class (type 12), opaque state, CLASSIC per-object source:
    /// the uniform-sphere planet bodies and any other type-12 mesh drawn
    /// through the per-object path. The chunked terrain patches ride
    /// `patch_render_pipeline` (the same entry, batch object source).
    pub terrain_render_pipeline: wgpu::RenderPipeline,
    /// VEGETATION class (20 procedural plant, 21 cluster card, 22 baked
    /// bark, 23 grass strand), opaque state: the near-tree parts, garden
    /// plants and the instanced grass sward.
    pub vegetation_render_pipeline: wgpu::RenderPipeline,
    /// WATER class (16, the ocean shell), transparent state, compiled with
    /// the ocean branch kept (`HAS_OCEAN_BRANCH`): the sea from orbit and
    /// whenever `water_depth_write` is off.
    pub water_transparent_pipeline: wgpu::RenderPipeline,
    /// Water class, overlay state (alpha blend, no cull, depth WRITE): the
    /// sea while `water_depth_write` holds (v0.1060), so a near crest
    /// occludes the wave behind it. Selected through `overlay_for`.
    pub water_overlay_pipeline: wgpu::RenderPipeline,
    /// SHELL class (13 the Fresnel atmosphere fallback, 14 the scattering
    /// atmosphere), transparent state, compiled with the atmosphere branch
    /// kept (`HAS_ATMOSPHERE_BRANCH`). There is no shell overlay PSO:
    /// nothing routes an atmosphere through an overlay list, and a PSO
    /// nothing draws with would cost a compile at every boot and reload.
    pub shell_transparent_pipeline: wgpu::RenderPipeline,
    /// CLOUD class (15), transparent state, compiled with the cloud branch
    /// kept (`HAS_CLOUD_BRANCH`): the ONE pipeline whose fragment entry
    /// reaches the volumetric march, so the only one whose fragments pay
    /// for the march's per-invocation tables (the P1 finding).
    pub cloud_transparent_pipeline: wgpu::RenderPipeline,
    /// Depth-only sun-shadow variant (v0.899): vs_main with no fragment,
    /// standard-z ortho depth into the 4096^2 shadow map. OPAQUE casters
    /// only - see `shadow_pipeline_alpha` and `shadow_for`.
    pub shadow_pipeline: wgpu::RenderPipeline,
    /// Alpha-cutout sun-shadow variant (v0.1106): identical state to
    /// `shadow_pipeline` plus the `fs_shadow` fragment stage, which mirrors
    /// the class entries' four cutout discards so a mostly-transparent
    /// caster stops casting a solid board. Kept SEPARATE rather than folded
    /// into `shadow_pipeline` because a fragment stage forfeits the
    /// depth-only double-rate rasterisation, and terrain meshes, ships,
    /// furniture and water are opaque and would pay it for nothing.
    /// `fs_shadow` is ONE union twin across every class (it carries no
    /// heavyweight storage, and the shadow pass has no class routing).
    pub shadow_pipeline_alpha: wgpu::RenderPipeline,
    /// Terrain-batch opaque variant (draw-batching increments 1+2):
    /// compiled from the BATCH shader module (per-instance attribute
    /// object source), group 1 is `patch_bind_group_layout`. Same
    /// blend/cull/depth as the opaque pipeline -- only where per-draw
    /// data comes from differs.
    pub patch_render_pipeline: wgpu::RenderPipeline,
    /// Shadow variant of the terrain-batch path (near-field patch casters
    /// render into the sun map without per-draw rebinds). ALWAYS carries the
    /// `fs_shadow` fragment stage (v0.1106): a patch's ground triangles and
    /// its tree cards share one index range, so this path cannot be split
    /// the way the classic one is.
    pub patch_shadow_pipeline: wgpu::RenderPipeline,
    /// The sun-shadow cache BAKE pass (performance plan increment 1,
    /// v0.1286): fullscreen triangle over the R16F slice atlas, fragment
    /// `fs_cloud_light_bake` fills one planet-fixed lattice point's sun
    /// optical depth. SAME pipeline layout as the march (the atlas rides
    /// the group-3 albedo slot when the march reads it), so no bind-group
    /// layout changes anywhere. Replaced the dead octa pipeline, whose
    /// `fs_cloud_octa` entry this fragment replaces in the shader.
    pub cloud_light_bake_pipeline: wgpu::RenderPipeline,
    /// Near-field screen cloud pass (12d): shell mesh at half res,
    /// per-pixel march + screen-space reprojection.
    pub cloud_screen_pipeline: wgpu::RenderPipeline,
    /// The cloud PROFILE bake (perf increment 4, the far rung): fullscreen
    /// triangle into the RGBA8 profile atlas (mip 0), scissored by Rust to
    /// the scroll / fill / refresh / global rows of the frame
    /// (`fs_cloud_profile_bake`). Same layout as the march.
    pub cloud_profile_bake_pipeline: wgpu::RenderPipeline,
    /// The profile atlas's global-region mip chain (`fs_cloud_profile_mip`,
    /// one RGBA8 target = mip m, source = mip m - 1 at binding 14).
    pub cloud_profile_mip_pipeline: wgpu::RenderPipeline,
    /// The calibration table, stage 1 (`fs_cloud_profile_calib`: per
    /// archetype / seed / height row, the canonical cloud's cross-section
    /// point test into the mip-2 staging area).
    pub cloud_profile_calib_pipeline: wgpu::RenderPipeline,
    /// The calibration table, stage 2 (`fs_cloud_profile_calib_reduce`:
    /// the eight-seed mean into the mip-1 table).
    pub cloud_profile_calib_reduce_pipeline: wgpu::RenderPipeline,
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub object_bind_group_layout: wgpu::BindGroupLayout,
    /// Group-1 layout for the terrain-batch pipelines: one shared batch
    /// uniform (planet rotation). Per-patch data rides the instance-rate
    /// vertex attribute. No dynamic offsets -- the whole point.
    pub patch_bind_group_layout: wgpu::BindGroupLayout,
    pub material_bind_group_layout: wgpu::BindGroupLayout,
    /// Group 3 (v0.811): albedo texture + sampler for per-pixel planet
    /// imagery. Added to the SHARED layout (not a dedicated pipeline
    /// variant) because every scene pass reuses these three pipelines --
    /// a variant would have to be duplicated across opaque, transparent
    /// AND overlay flavors and threaded through all six draw loops anyway.
    /// The cost of sharing is one extra bind per draw, paid with a 1x1
    /// white fallback texture for everything that isn't a textured planet
    /// (the type-12 params.w flag keeps the shader from ever sampling it
    /// elsewhere). 4 bind groups is exactly wgpu's baseline max_bind_groups,
    /// so no device-limit risk (the v0.782 lesson).
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
}

/// What one megashader hot-reload rebuilt, as COUNTED by
/// [`Pipeline::recreate_pipelines`] from its own slot tables. The reload log
/// line prints these so it can never again say "6 PSOs" while installing
/// thirteen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RebuiltPipelines {
    /// PSOs whose fragment entry is a class entry or `fs_shadow` (or none,
    /// the depth-only shadow PSO): the thirteen registered in
    /// `PSO_REGISTRY`, one row per (class, state) the draw loops use.
    pub megashader: usize,
    /// The cloud fullscreen PSOs (`fs_cloud_*` entries), exempt from the
    /// registry because they never enter `fs_main`.
    pub cloud: usize,
}

impl RebuiltPipelines {
    /// Every PSO the reload installed.
    pub fn total(self) -> usize {
        self.megashader + self.cloud
    }
}

impl Pipeline {
    /// Create the PBR-lite pipeline set from the classic shader module plus
    /// the terrain-batch variant module (same source, batch OBJECT-SOURCE).
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        shader: &wgpu::ShaderModule,
        batch_shader: &wgpu::ShaderModule,
    ) -> Self {
        // Group 0: Camera uniforms + the UNCAPPED light list (v0.782). Lights
        // moved from fixed [8] uniform arrays to a read-only STORAGE buffer so
        // the count is data-driven -- no arbitrary light limit; the practical
        // ceiling is GPU fill cost, found empirically (F2 overlay shows the
        // live count). The old light0..7 uniform fields stay in CameraUniforms
        // (unused) so no byte offset anywhere shifts.
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Camera Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(
                                std::mem::size_of::<CameraUniforms>() as u64,
                            ),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            // One GpuLight = 4 x vec4<f32> = 64 bytes.
                            min_binding_size: wgpu::BufferSize::new(64),
                        },
                        count: None,
                    },
                    // Light-tile lists (clustering L1b, v0.952): per-screen-tile
                    // counts + light indices from renderer/light_tiles.rs. The
                    // fragment loop reads only its tile's list when tiling is
                    // on (shadow_u.params2.z > 0 carries the tile pixel width).
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(4),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(4),
                        },
                        count: None,
                    },
                ],
            });

        // Group 1: Object uniforms (model + normal matrix) with dynamic offset.
        // FRAGMENT visibility added for the analytic atmosphere (v0.807): the
        // type-14 scattering branch recovers the shell's center + radius from
        // object.model per fragment, and wgpu validates shader-stage usage
        // against these flags at pipeline creation (boot-verify caught the
        // VERTEX-only layout as a startup panic -- the v0.782 lesson holds:
        // tests + naga cannot see pipeline-layout mismatches, only booting
        // can). Fragment-stage uniform buffers are a base WebGPU capability,
        // no device-limit risk.
        let object_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Object Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<ObjectUniforms>() as u64,
                        ),
                    },
                    count: None,
                }],
            });

        // Group 2: Material uniforms. VERTEX visibility added for the water
        // shell (v0.876): the type-16 vertex branch reads material.params.z
        // (type gate) + base_color.xyz (planet center) to Gerstner-displace
        // water vertices in planet-local space. Same v0.807 lesson as the
        // object layout below: widen the layout IN THE SAME COMMIT as the
        // shader-stage use, and boot-verify (naga/tests cannot see
        // pipeline-layout mismatches, only booting can).
        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Material Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<MaterialUniforms>() as u64,
                        ),
                    },
                    count: None,
                }],
            });

        // Group 3: albedo texture + sampler (v0.811, per-pixel planet
        // imagery) PLUS the two shared tiling 3D cloud-noise volumes + their
        // repeat sampler (clouds increment 3). All entries are base WebGPU
        // capabilities under default limits (filterable 2D/3D textures,
        // filtering samplers; well under the 16-per-stage texture/sampler
        // caps), and the total bind-group count stays at 4 -- exactly wgpu's
        // baseline max_bind_groups, so no device-limit risk (v0.782 lesson).
        // The cloud volumes ride in the SAME group as the albedo because a
        // fifth group is not available and the volumes are engine-global
        // (every bind group built from this layout shares the same two
        // texture views, wired in renderer::mod).
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Albedo Texture Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Cloud SHAPE volume: RGBA8 tiling Perlin-Worley +
                    // Worley octaves (renderer::cloud_noise::generate_shape;
                    // SHAPE_SIZE^3, 384 as of v0.1188). The layout is
                    // resolution-agnostic - only the D3 dimension matters here.
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D3,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Cloud DETAIL volume: RGBA8 tiling Worley octaves +
                    // ridged filament (renderer::cloud_noise::generate_detail;
                    // DETAIL_SIZE^3, 256 as of v0.1188).
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D3,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Repeat-all-axes sampler for the tiling volumes (the
                    // albedo sampler clamps V/W, so it cannot be reused).
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Live weather map (v0.874): equirect RG8 - R = real cloud
                    // fraction from NASA GIBS, G = validity (0 -> the shader
                    // uses procedural coverage). Zero-filled until data lands.
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Sun shadow map (v0.899): near-field ortho depth from
                    // the sun + comparison sampler + light matrix uniform.
                    wgpu::BindGroupLayoutEntry {
                        binding: 6,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 7,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 8,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Ground PBR texture array (v0.907): 8 layers, colors
                    // 0..3 + normals 4..7, with its own repeat/aniso sampler
                    // (the albedo sampler clamps; tiling needs wrap).
                    wgpu::BindGroupLayoutEntry {
                        binding: 9,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 10,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Atmosphere LUTs (sky arc stage 3a, v0.945): transmittance
                    // 256x64 + multiple-scattering 32x32, Rgba16Float
                    // (filterable under default limits, unlike Rgba32Float),
                    // CPU-generated per planet by renderer/atmo_luts.rs.
                    // Sampled with the albedo sampler (binding 1, clamp +
                    // filter). Consumed by the stage-3b sky-view pass.
                    wgpu::BindGroupLayoutEntry {
                        binding: 11,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 12,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Sky-view LUT target (stage 3c): the per-frame distant-sky
                    // radiance table the near-surface sky samples. Gated by
                    // shadow_u.params2.y (stale when not near an atmosphere).
                    wgpu::BindGroupLayoutEntry {
                        binding: 13,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // Tree-card sprite atlas (v0.961, billboard bake increment
                    // 2): 3x2 grid of side-on baked conifer sprites the
                    // type-12 tree-card branch textures its quads with.
                    // Gated by material.params.w bit 2 (atlas resident).
                    wgpu::BindGroupLayoutEntry {
                        binding: 14,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    // FFT ocean tile (v0.1029 inc 1, v0.1031 inc 2):
                    // 128x128 Rgba32Float (height, slope_u, slope_v,
                    // foam). VS displaces from .r; FS shades normals +
                    // whitecaps from .gba. textureLoad manual bilinear,
                    // hence non-filterable is fine.
                    wgpu::BindGroupLayoutEntry {
                        binding: 15,
                        visibility: wgpu::ShaderStages::VERTEX
                            | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        // Group 1 for the terrain-batch pipelines: ONE shared batch uniform
        // (the planet rotation). Per-patch data rides the instance-rate
        // vertex attribute (Vertex::instance_layout), not a binding.
        // FRAGMENT visibility because the fragment-stage obj_* accessors
        // read the uniform too (the v0.807 lesson: widen the layout in the
        // same commit as the shader use, and boot-verify -- naga cannot see
        // pipeline-layout mismatches, only booting can).
        let patch_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Patch Batch Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        // One mat4x4<f32> = 64 bytes.
                        min_binding_size: wgpu::BufferSize::new(64),
                    },
                    count: None,
                }],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PBR-lite Pipeline Layout"),
            bind_group_layouts: &[
                &camera_bind_group_layout,
                &object_bind_group_layout,
                &material_bind_group_layout,
                &texture_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });
        let patch_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Patch Batch Pipeline Layout"),
                bind_group_layouts: &[
                    &camera_bind_group_layout,
                    &patch_bind_group_layout,
                    &material_bind_group_layout,
                    &texture_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let MegashaderPsos {
            surface_render: surface_render_pipeline,
            surface_transparent: surface_transparent_pipeline,
            surface_overlay: surface_overlay_pipeline,
            terrain_render: terrain_render_pipeline,
            vegetation_render: vegetation_render_pipeline,
            water_transparent: water_transparent_pipeline,
            water_overlay: water_overlay_pipeline,
            shell_transparent: shell_transparent_pipeline,
            cloud_transparent: cloud_transparent_pipeline,
            shadow: shadow_pipeline,
            shadow_alpha: shadow_pipeline_alpha,
            patch_render: patch_render_pipeline,
            patch_shadow: patch_shadow_pipeline,
        } = Self::build_all_pipelines(
            device,
            surface_format,
            shader,
            batch_shader,
            &pipeline_layout,
            &patch_pipeline_layout,
        );
        let cloud_light_bake_pipeline =
            Self::build_cloud_light_bake_pipeline(device, shader, &pipeline_layout);
        let cloud_screen_pipeline =
            Self::build_cloud_screen_pipeline(device, shader, &pipeline_layout);
        let cloud_profile_bake_pipeline = Self::build_cloud_profile_pipeline(
            device, shader, &pipeline_layout, "Cloud Profile Bake Pipeline", "fs_cloud_profile_bake",
        );
        let cloud_profile_mip_pipeline = Self::build_cloud_profile_pipeline(
            device, shader, &pipeline_layout, "Cloud Profile Mip Pipeline", "fs_cloud_profile_mip",
        );
        let cloud_profile_calib_pipeline = Self::build_cloud_profile_pipeline(
            device, shader, &pipeline_layout, "Cloud Profile Calib Pipeline", "fs_cloud_profile_calib",
        );
        let cloud_profile_calib_reduce_pipeline = Self::build_cloud_profile_pipeline(
            device,
            shader,
            &pipeline_layout,
            "Cloud Profile Calib Reduce Pipeline",
            "fs_cloud_profile_calib_reduce",
        );

        Self {
            surface_render_pipeline,
            surface_transparent_pipeline,
            surface_overlay_pipeline,
            terrain_render_pipeline,
            vegetation_render_pipeline,
            water_transparent_pipeline,
            water_overlay_pipeline,
            shell_transparent_pipeline,
            cloud_transparent_pipeline,
            shadow_pipeline,
            shadow_pipeline_alpha,
            patch_render_pipeline,
            patch_shadow_pipeline,
            cloud_light_bake_pipeline,
            cloud_screen_pipeline,
            cloud_profile_bake_pipeline,
            cloud_profile_mip_pipeline,
            cloud_profile_calib_pipeline,
            cloud_profile_calib_reduce_pipeline,
            camera_bind_group_layout,
            object_bind_group_layout,
            patch_bind_group_layout,
            material_bind_group_layout,
            texture_bind_group_layout,
        }
    }

    /// One fullscreen-triangle cloud pipeline over the SHARED group
    /// layouts (camera/object/material/texture), parametrized by the
    /// fragment entry and its colour targets. Zero layout changes, so the
    /// v0.1029 every-site hazard never applies. No depth, no blending:
    /// every consumer writes its target outright. Cull NONE (a fullscreen
    /// triangle has one winding, but the march's inside-camera history
    /// wants the rule stated). Two pipelines are built from it:
    /// - the near-field SCREEN march (`fs_cloud_screen`, MRT: premultiplied
    ///   march + first-hit distance in km);
    /// - the sun-shadow cache BAKE (`fs_cloud_light_bake`, one R16F slice
    ///   atlas, increment 1).
    fn build_cloud_fullscreen_pipeline(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
        label: &str,
        fs_entry: &str,
        targets: &[Option<wgpu::ColorTargetState>],
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: shader,
                // Fullscreen triangle: the fragment builds each pixel's ray
                // (march) or lattice point (bake) analytically. Never the
                // shell mesh - its coarse icosphere chords sag below a
                // ground camera and invert the rays (the under-deck vanish).
                entry_point: Some("vs_cloud_screen"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: Some(fs_entry),
                compilation_options: Default::default(),
                targets,
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    /// The near-field SCREEN cloud pass (12d two-regime architecture):
    /// fullscreen triangle into the quarter-res march pair, fragment =
    /// per-pixel march; the resolve pass reprojects from the MRT distance.
    fn build_cloud_screen_pipeline(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
        Self::build_cloud_fullscreen_pipeline(
            device,
            shader,
            layout,
            "Cloud Screen Temporal Pipeline",
            "fs_cloud_screen",
            // MRT (12e): premultiplied march + first-hit distance in
            // km (R16F) for the resolve pass's reprojection.
            &[
                Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
                Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::R16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                }),
            ],
        )
    }

    /// The sun-shadow cache BAKE pass (increment 1): fullscreen triangle
    /// over the 15360x256 R16F slice atlas; the pass scissors it to the
    /// slices being refreshed this frame. The fragment derives (window,
    /// k, i, j) from its pixel position and writes that lattice point's
    /// far-rung sun optical depth in the red channel.
    fn build_cloud_light_bake_pipeline(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
        Self::build_cloud_fullscreen_pipeline(
            device,
            shader,
            layout,
            "Cloud Light Bake Pipeline",
            "fs_cloud_light_bake",
            &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::R16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        )
    }

    /// Rebuild EVERY PSO compiled from the megashader module (the ten
    /// registry PSOs of `build_all_pipelines` plus the six cloud fullscreen
    /// PSOs) from a NEW module while keeping every bind group layout object
    /// intact (v0.924 megashader hot-reload): the layouts are what live bind
    /// groups reference, so swapping only the pipelines means nothing else
    /// in the renderer needs recreating. Costs a few seconds of PSO compile,
    /// trivial next to the 3+ minute rebuild-and-reboot it replaces.
    ///
    /// Returns how many of each it installed, COUNTED from the slot tables
    /// below rather than typed by hand: the hot-reload log line printed "6
    /// PSOs rebuilt" for months while thirteen were, because the number was
    /// a literal nobody updated when the patch and cloud pipelines joined.
    pub fn recreate_pipelines(
        &mut self,
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        shader: &wgpu::ShaderModule,
        batch_shader: &wgpu::ShaderModule,
    ) -> RebuiltPipelines {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PBR-lite Pipeline Layout (hot-reload)"),
            bind_group_layouts: &[
                &self.camera_bind_group_layout,
                &self.object_bind_group_layout,
                &self.material_bind_group_layout,
                &self.texture_bind_group_layout,
            ],
            push_constant_ranges: &[],
        });
        let patch_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Patch Batch Pipeline Layout (hot-reload)"),
                bind_group_layouts: &[
                    &self.camera_bind_group_layout,
                    &self.patch_bind_group_layout,
                    &self.material_bind_group_layout,
                    &self.texture_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });
        let fresh = Self::build_all_pipelines(
            device,
            surface_format,
            shader,
            batch_shader,
            &pipeline_layout,
            &patch_pipeline_layout,
        );
        // Each fresh PSO is paired with the field it replaces, and the pair
        // tables are what the returned counts are read from. The `&mut`
        // borrows are of DISJOINT fields, which Rust allows side by side.
        // The fresh set is a named struct (not a tuple) so a slot can only
        // ever be paired with the PSO of the same name.
        let megashader_slots = [
            (&mut self.surface_render_pipeline, fresh.surface_render),
            (&mut self.surface_transparent_pipeline, fresh.surface_transparent),
            (&mut self.surface_overlay_pipeline, fresh.surface_overlay),
            (&mut self.terrain_render_pipeline, fresh.terrain_render),
            (&mut self.vegetation_render_pipeline, fresh.vegetation_render),
            (&mut self.water_transparent_pipeline, fresh.water_transparent),
            (&mut self.water_overlay_pipeline, fresh.water_overlay),
            (&mut self.shell_transparent_pipeline, fresh.shell_transparent),
            (&mut self.cloud_transparent_pipeline, fresh.cloud_transparent),
            (&mut self.shadow_pipeline, fresh.shadow),
            (&mut self.shadow_pipeline_alpha, fresh.shadow_alpha),
            (&mut self.patch_render_pipeline, fresh.patch_render),
            (&mut self.patch_shadow_pipeline, fresh.patch_shadow),
        ];
        let megashader = megashader_slots.len();
        for (slot, fresh) in megashader_slots {
            *slot = fresh;
        }
        // The six cloud fullscreen PSOs follow the same hot-reload rule: new
        // module, same layouts, live bind groups intact. (They compile the
        // megashader module too, but through fs_cloud_* entries that are no
        // class entry and reach no material dispatch, so they take no
        // permutation: see the exemption note above PSO_REGISTRY.) Timed as
        // one block, the way the class PSOs are timed one by one.
        let t_cloud = std::time::Instant::now();
        let cloud_slots = [
            (
                &mut self.cloud_light_bake_pipeline,
                Self::build_cloud_light_bake_pipeline(device, shader, &pipeline_layout),
            ),
            (
                &mut self.cloud_screen_pipeline,
                Self::build_cloud_screen_pipeline(device, shader, &pipeline_layout),
            ),
            (
                &mut self.cloud_profile_bake_pipeline,
                Self::build_cloud_profile_pipeline(
                    device,
                    shader,
                    &pipeline_layout,
                    "Cloud Profile Bake Pipeline",
                    "fs_cloud_profile_bake",
                ),
            ),
            (
                &mut self.cloud_profile_mip_pipeline,
                Self::build_cloud_profile_pipeline(
                    device,
                    shader,
                    &pipeline_layout,
                    "Cloud Profile Mip Pipeline",
                    "fs_cloud_profile_mip",
                ),
            ),
            (
                &mut self.cloud_profile_calib_pipeline,
                Self::build_cloud_profile_pipeline(
                    device,
                    shader,
                    &pipeline_layout,
                    "Cloud Profile Calib Pipeline",
                    "fs_cloud_profile_calib",
                ),
            ),
            (
                &mut self.cloud_profile_calib_reduce_pipeline,
                Self::build_cloud_profile_pipeline(
                    device,
                    shader,
                    &pipeline_layout,
                    "Cloud Profile Calib Reduce Pipeline",
                    "fs_cloud_profile_calib_reduce",
                ),
            ),
        ];
        let cloud = cloud_slots.len();
        for (slot, fresh) in cloud_slots {
            *slot = fresh;
        }
        log::info!(
            "[Pipelines] {cloud} cloud fullscreen PSOs recompiled in {:.1}s (serial)",
            t_cloud.elapsed().as_secs_f32()
        );
        RebuiltPipelines { megashader, cloud }
    }

    /// One far-rung pipeline (perf increment 4): fullscreen triangle over
    /// ONE `Rgba8Unorm` target (the profile atlas at some mip), blend None,
    /// the shared layout, parametrized by the fragment entry. The bake,
    /// the mip chain and the two calibration stages are all this shape;
    /// Rust chooses the attachment (mip view) and the scissor per pass.
    fn build_cloud_profile_pipeline(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
        label: &str,
        fs_entry: &str,
    ) -> wgpu::RenderPipeline {
        Self::build_cloud_fullscreen_pipeline(
            device,
            shader,
            layout,
            label,
            fs_entry,
            &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        )
    }

    /// The sun-shadow PSO a CLASSIC caster should draw with (v0.1106).
    /// `cutout` = this draw can discard fragments in `fs_shadow` (foliage,
    /// tree cards, anything carrying its own albedo texture); false takes the
    /// depth-only fast path. One accessor so the choice reads the same at
    /// every call site instead of two field names to keep straight.
    pub fn shadow_for(&self, cutout: bool) -> &wgpu::RenderPipeline {
        if cutout {
            &self.shadow_pipeline_alpha
        } else {
            &self.shadow_pipeline
        }
    }

    /// The OPAQUE-state PSO (REPLACE blend, back-face cull, depth write) a
    /// material of `class` must draw with (increment P3). The three opaque
    /// classes each have one: surfaces, vegetation, terrain. The opaque
    /// draw loops (`Renderer::draw_opaque_objects`, the grass draw, the
    /// instanced batches) pick through this, one pass per class present,
    /// so a list mixing walls and garden plants binds each PSO once.
    ///
    /// A shell, cloud or water material in an OPAQUE list is a caller bug
    /// (those ride the transparent lists by construction): debug builds
    /// stop here naming the class, release builds draw it through the
    /// surface PSO, whose entry has no dispatch for the type, so it renders
    /// as the default look, visibly wrong and never silently absent.
    pub fn opaque_for(&self, class: ShaderClass) -> &wgpu::RenderPipeline {
        match class {
            ShaderClass::Surface => &self.surface_render_pipeline,
            ShaderClass::Terrain => &self.terrain_render_pipeline,
            ShaderClass::Vegetation => &self.vegetation_render_pipeline,
            ShaderClass::Water | ShaderClass::Shell | ShaderClass::Cloud => {
                debug_assert!(
                    false,
                    "a {class:?}-class material was routed to an OPAQUE draw list; shells, \
                     clouds and water draw through the transparent lists only"
                );
                &self.surface_render_pipeline
            }
        }
    }

    /// The TRANSPARENT-state PSO (alpha blend, no cull, depth test, no depth
    /// write) a material of `class` must draw with (increment P2, one per
    /// class since P3). One accessor so every transparent draw loop picks
    /// the same way: the class comes from `shader_class(material_type)`,
    /// never from a field name typed at the call site, so a shell cannot be
    /// handed a pipeline whose entry lacks its code (the fragment would
    /// fall through to the default look, a white blended sphere where the
    /// sky should be). Terrain and vegetation have no transparent PSO
    /// because no transparent list ever carries them; one there is a caller
    /// bug, handled like the opaque case above.
    pub fn transparent_for(&self, class: ShaderClass) -> &wgpu::RenderPipeline {
        match class {
            ShaderClass::Surface => &self.surface_transparent_pipeline,
            ShaderClass::Water => &self.water_transparent_pipeline,
            ShaderClass::Shell => &self.shell_transparent_pipeline,
            ShaderClass::Cloud => &self.cloud_transparent_pipeline,
            ShaderClass::Terrain | ShaderClass::Vegetation => {
                debug_assert!(
                    false,
                    "a {class:?}-class material was routed to a TRANSPARENT draw list; \
                     terrain and vegetation are opaque and draw through opaque_for"
                );
                &self.surface_transparent_pipeline
            }
        }
    }

    /// The OVERLAY-state PSO (alpha blend, no cull, depth WRITE) for a
    /// material of `class`. Two users: the editor gizmos (surface) and the
    /// water shell when `water_depth_write` is on (v0.1060). There is
    /// deliberately no shell, cloud, terrain or vegetation overlay PSO:
    /// nothing routes those through an overlay list, and a PSO nothing
    /// draws with would cost a full compile at every boot and every hot
    /// reload for no pixel. One of those classes in an overlay list is a
    /// caller bug; it trips the debug assertion, and in release draws
    /// through the surface overlay PSO, whose entry has no dispatch for the
    /// type, so it renders as the default look (visibly wrong, never
    /// silently absent).
    pub fn overlay_for(&self, class: ShaderClass) -> &wgpu::RenderPipeline {
        match class {
            ShaderClass::Surface => &self.surface_overlay_pipeline,
            ShaderClass::Water => &self.water_overlay_pipeline,
            ShaderClass::Shell | ShaderClass::Cloud | ShaderClass::Terrain | ShaderClass::Vegetation => {
                debug_assert!(
                    false,
                    "a {class:?}-class material was routed to an OVERLAY list; only surfaces \
                     (gizmos) and the water shell ever draw with the overlay state"
                );
                &self.surface_overlay_pipeline
            }
        }
    }

    /// The terrain-batch OPAQUE PSO, compiled from the BATCH shader module
    /// (different module + pipeline layout from the classic set). Single-PSO
    /// helper so `build_all_pipelines` can compile it on its own thread.
    fn build_patch_render(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        batch_shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
        // The registry row for this PSO (PSO_REGISTRY) names both the
        // fragment entry it compiles (the terrain class's, `fs_terrain`: the
        // planet-surface block and nothing else) and the switches compiled
        // out of it. Terrain patches never draw an atmosphere, cloud or
        // ocean shell, so the cloud march's per-invocation tables never
        // reach this pipeline's DXIL.
        let label = "Patch Batch Render Pipeline";
        let constants = pso_constants(label);
        let entry = pso_fragment_entry(label).expect("the patch render PSO compiles a class entry");
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: batch_shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout(), Vertex::instance_layout()],
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &constants,
                    ..Default::default()
                },
            },
            fragment: Some(wgpu::FragmentState {
                module: batch_shader,
                entry_point: Some(entry),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &constants,
                    ..Default::default()
                },
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                // Back-face cull, matching the classic opaque pipeline these
                // patches drew through until now (vegetation cards are
                // emitted double-sided, so they survive culling either way).
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Greater, // reverse-Z
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        })
    }

    /// The terrain-batch SHADOW PSO: STANDARD z (light ortho maps near->0),
    /// no cull (vegetation cards cast from both sides), and since v0.1106 the
    /// `fs_shadow` ALPHA-CUTOUT fragment stage with NO colour target.
    ///
    /// The patch path has no choice about paying for a fragment stage: a
    /// chunk's ground triangles and its sprite tree cards are one mesh in
    /// one index range, drawn together, so the only place the card's
    /// cutout can be applied is per-fragment. Before this, a 21x21 m card
    /// whose sprite is ~15-25% opaque cast a SOLID 21 m board - the dark
    /// rectangles the operator found lying on open grass. `targets: &[]`
    /// matches the shadow pass's empty `color_attachments`; fs_shadow
    /// returns nothing and only ever discards.
    fn build_patch_shadow(
        device: &wgpu::Device,
        batch_shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
        // The registry row (PSO_REGISTRY) names `fs_shadow` as this PSO's
        // fragment entry and every switch as dead: fs_shadow never reaches
        // the three shells anyway, but the terrain module is compiled with
        // one consistent set of switches so the two patch PSOs can never
        // drift apart.
        let label = "Patch Batch Shadow Pipeline";
        let constants = pso_constants(label);
        let entry = pso_fragment_entry(label).expect("the patch shadow PSO always carries fs_shadow");
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: batch_shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout(), Vertex::instance_layout()],
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &constants,
                    ..Default::default()
                },
            },
            fragment: Some(wgpu::FragmentState {
                module: batch_shader,
                entry_point: Some(entry),
                targets: &[],
                compilation_options: wgpu::PipelineCompilationOptions {
                    constants: &constants,
                    ..Default::default()
                },
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
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
        })
    }

    /// ALL THIRTEEN PSO compiles shared by `new` and hot-reload's
    /// `recreate_pipelines`, in ONE thread scope (v0.1142). Measured
    /// 2026-08-15: `Pipeline::new` was 3.9 s of the 4.1 s
    /// shaders_and_pipelines boot span, because only the three PBR variants
    /// compiled in parallel (the 2026-07-12 scope) while the two sun-shadow
    /// and two terrain-patch PSOs compiled serially after them on the main
    /// thread. Each PSO bakes its fragment entry through Naga->DXIL, they
    /// are all independent, and `create_render_pipeline` takes `&self` on a
    /// Send+Sync Device, so all of them belong in the same scope: wall time
    /// falls toward the slowest single compile (the cloud transparent PSO,
    /// the only one whose entry bakes the march). Since P3 each PSO compiles
    /// only its class's entry, so every non-cloud compile is a fraction of
    /// what the old whole-fs_main bake was; the per-PSO wall times are
    /// logged after every build so the cost of the split is a reading, not
    /// a guess (the P2 reload took 11.3 s for ten PSOs).
    fn build_all_pipelines(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        shader: &wgpu::ShaderModule,
        batch_shader: &wgpu::ShaderModule,
        pipeline_layout: &wgpu::PipelineLayout,
        patch_pipeline_layout: &wgpu::PipelineLayout,
    ) -> MegashaderPsos {
        // ── Parallel PBR pipeline compile (boot-speed, 2026-07-12) ──
        // Every class PSO bakes its fragment entry into a backend PSO, which
        // on this GPU takes seconds of Naga->DXIL work apiece -- the dominant
        // cold-boot cost (measured via debug/boot_timing.json). They are
        // otherwise independent: same shader module + pipeline layout,
        // differing only in entry and blend / cull / depth-write. wgpu's
        // `Device` is `Send + Sync` and `create_render_pipeline` takes
        // `&self`, so the PSO compiles are sound to run CONCURRENTLY, cutting
        // the serial sum down toward the slowest single compile.
        // `std::thread::scope` lets the worker threads borrow the shared
        // `&device` / `&pipeline_layout` / `shader` without any 'static bound.
        let make_pbr = |label: &'static str,
                        blend: wgpu::BlendState,
                        cull: Option<wgpu::Face>,
                        depth_write: bool|
         -> wgpu::RenderPipeline {
            // Both the fragment ENTRY this PSO compiles and the branch
            // switches it compiles out are the registry's call (PSO_REGISTRY,
            // keyed by label): a surface PSO bakes fs_surface with every
            // shell switch off, the water PSO bakes fs_water with the ocean
            // kept, the cloud PSO bakes fs_cloud with the march kept, and so
            // on. Switches the map does not name keep the shader's `= true`
            // default.
            let constants = pso_constants(label);
            let entry = pso_fragment_entry(label).expect("a colour PSO compiles a class entry");
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(pipeline_layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex::layout(), Vertex::instance_layout()],
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &constants,
                        ..Default::default()
                    },
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some(entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &constants,
                        ..Default::default()
                    },
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: cull,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: depth_write,
                    depth_compare: wgpu::CompareFunction::Greater, // reverse-Z for far-field precision
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: 1,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                multiview: None,
                cache: None,
            })
        };
        // Variant states:
        //  - Render (opaque): REPLACE blend, back-face cull, depth WRITE.
        //  - Transparent (v0.456): alpha blend, double-sided (no cull), no depth
        //    WRITE (glass doesn't occlude) but still depth-TEST.
        //  - Overlay (v0.560/563): alpha blend, no cull, depth WRITE (the pass
        //    clears depth first, so gizmos sort among themselves yet draw over
        //    the world -- visible through walls).
        //
        // Sun-shadow pipelines (v0.899; SPLIT IN TWO v0.1106): same vertex
        // path as the colour passes (so ocean vertex displacement casts
        // correctly), STANDARD z because the light ortho maps near->0, unlike
        // the reverse-Z main passes.
        //
        // Two PSOs, identical but for the fragment stage:
        //  - "Sun Shadow Pipeline": depth-only, `fragment: None`. Opaque
        //    casters (ships, furniture, props, water, untextured meshes) keep
        //    the double-rate depth-only rasterisation every desktop GPU gives
        //    a pixel-shader-free draw.
        //  - "Sun Shadow Alpha Pipeline": adds `fs_shadow`, which mirrors
        //    fs_main's cutout discards. Only cutout casters draw with it, so
        //    the fast path above is not lost engine-wide. Splitting rather
        //    than blanket-attaching is the standard arrangement (Eisemann,
        //    Schwarz, Assarsson & Wimmer, "Real-Time Shadows" 2011, ch. 2;
        //    Godot's SHADOW_CASTER alpha-scissor path).
        // `targets: &[]` matches the shadow pass's empty color_attachments.
        let make_shadow = |label: &'static str| -> wgpu::RenderPipeline {
            // Registered with every switch off in PSO_REGISTRY, and that is
            // a no-op for these two: fs_shadow never reaches the three
            // shells and vs_main reads no switch (the water shell's VERTEX
            // displacement rides this module untouched), so the constants
            // change nothing the backend keeps. They are named anyway so
            // every megashader PSO is compiled with an explicit permutation
            // rather than an implicit default. Whether the PSO has a
            // fragment stage at all is the registry's call too: `fs_shadow`
            // for the cutout variant, none for the depth-only one.
            let constants = pso_constants(label);
            let entry = pso_fragment_entry(label);
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(pipeline_layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex::layout(), Vertex::instance_layout()],
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &constants,
                        ..Default::default()
                    },
                },
                fragment: entry.map(|entry| wgpu::FragmentState {
                    module: shader,
                    entry_point: Some(entry),
                    targets: &[],
                    compilation_options: wgpu::PipelineCompilationOptions {
                        constants: &constants,
                        ..Default::default()
                    },
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None, // vegetation cards are two-sided
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
            })
        };
        // The three fixed-function states the class PSOs are built in. The
        // SAME state for every class PSO of one flavour, by construction:
        // only the label (and so the registry's entry and constant map)
        // differs, which is what makes routing by class a pure perf change
        // with no blend / cull / depth consequence.
        //  - Opaque: REPLACE blend, back-face cull, depth write.
        //  - Transparent: alpha blend, double-sided, depth test, no write.
        //  - Overlay: alpha blend, double-sided, depth WRITE.
        let opaque_state = (wgpu::BlendState::REPLACE, Some(wgpu::Face::Back), true);
        let transparent_state = (wgpu::BlendState::ALPHA_BLENDING, None, false);
        let overlay_state = (wgpu::BlendState::ALPHA_BLENDING, None, true);
        let pbr = |label: &'static str, (blend, cull, depth_write): (wgpu::BlendState, Option<wgpu::Face>, bool)| {
            make_pbr(label, blend, cull, depth_write)
        };
        // Wall time per PSO, so the boot and reload logs can say what each
        // class's entry costs to bake (the doc's compile-time table reads
        // these lines).
        let t_all = std::time::Instant::now();
        let (psos, timings) = std::thread::scope(|s| {
            let surface_render =
                s.spawn(|| timed("PBR-lite Surface Render Pipeline", |l| pbr(l, opaque_state)));
            let surface_transparent = s.spawn(|| {
                timed("PBR-lite Surface Transparent Pipeline", |l| pbr(l, transparent_state))
            });
            let surface_overlay =
                s.spawn(|| timed("PBR-lite Surface Overlay Pipeline", |l| pbr(l, overlay_state)));
            let terrain_render =
                s.spawn(|| timed("PBR-lite Terrain Render Pipeline", |l| pbr(l, opaque_state)));
            let vegetation_render =
                s.spawn(|| timed("PBR-lite Vegetation Render Pipeline", |l| pbr(l, opaque_state)));
            let water_transparent = s.spawn(|| {
                timed("PBR-lite Water Transparent Pipeline", |l| pbr(l, transparent_state))
            });
            let water_overlay =
                s.spawn(|| timed("PBR-lite Water Overlay Pipeline", |l| pbr(l, overlay_state)));
            let shell_transparent = s.spawn(|| {
                timed("PBR-lite Shell Transparent Pipeline", |l| pbr(l, transparent_state))
            });
            let cloud_transparent = s.spawn(|| {
                timed("PBR-lite Cloud Transparent Pipeline", |l| pbr(l, transparent_state))
            });
            let shadow = s.spawn(|| timed("Sun Shadow Pipeline", |l| make_shadow(l)));
            let shadow_alpha = s.spawn(|| timed("Sun Shadow Alpha Pipeline", |l| make_shadow(l)));
            let patch_render = s.spawn(|| {
                timed("Patch Batch Render Pipeline", |_| {
                    Self::build_patch_render(device, surface_format, batch_shader, patch_pipeline_layout)
                })
            });
            // The thirteenth compiles on this thread while the twelve
            // workers run.
            let patch_shadow = timed("Patch Batch Shadow Pipeline", |_| {
                Self::build_patch_shadow(device, batch_shader, patch_pipeline_layout)
            });
            fn join<'s, T>(h: std::thread::ScopedJoinHandle<'s, T>, what: &str) -> T {
                h.join().unwrap_or_else(|_| panic!("{what} PSO compile panicked"))
            }
            let surface_render = join(surface_render, "surface render");
            let surface_transparent = join(surface_transparent, "surface transparent");
            let surface_overlay = join(surface_overlay, "surface overlay");
            let terrain_render = join(terrain_render, "terrain render");
            let vegetation_render = join(vegetation_render, "vegetation render");
            let water_transparent = join(water_transparent, "water transparent");
            let water_overlay = join(water_overlay, "water overlay");
            let shell_transparent = join(shell_transparent, "shell transparent");
            let cloud_transparent = join(cloud_transparent, "cloud transparent");
            let shadow = join(shadow, "sun shadow");
            let shadow_alpha = join(shadow_alpha, "sun shadow alpha");
            let patch_render = join(patch_render, "patch render");
            let timings = vec![
                surface_render.1,
                surface_transparent.1,
                surface_overlay.1,
                terrain_render.1,
                vegetation_render.1,
                water_transparent.1,
                water_overlay.1,
                shell_transparent.1,
                cloud_transparent.1,
                shadow.1,
                shadow_alpha.1,
                patch_render.1,
                patch_shadow.1,
            ];
            let psos = MegashaderPsos {
                surface_render: surface_render.0,
                surface_transparent: surface_transparent.0,
                surface_overlay: surface_overlay.0,
                terrain_render: terrain_render.0,
                vegetation_render: vegetation_render.0,
                water_transparent: water_transparent.0,
                water_overlay: water_overlay.0,
                shell_transparent: shell_transparent.0,
                cloud_transparent: cloud_transparent.0,
                shadow: shadow.0,
                shadow_alpha: shadow_alpha.0,
                patch_render: patch_render.0,
                patch_shadow: patch_shadow.0,
            };
            (psos, timings)
        });
        // One line per build (boot and every hot reload): the wall time of
        // the whole scope, the serial sum, and each PSO's own compile.
        let sum: f32 = timings.iter().map(|(_, s)| *s).sum();
        let per_pso: Vec<String> = timings
            .iter()
            .map(|(label, s)| format!("{} {s:.2}s", short_pso_label(label)))
            .collect();
        log::info!(
            "[Pipelines] {} megashader PSOs compiled in {:.1}s wall ({:.1}s serial sum): {}",
            timings.len(),
            t_all.elapsed().as_secs_f32(),
            sum,
            per_pso.join(", ")
        );
        psos
    }
}

/// Build one PSO and return it with its (label, wall seconds) so the
/// per-class compile cost is a logged reading. A plain function rather
/// than a closure so the scoped threads can call it without capturing
/// anything.
fn timed(
    label: &'static str,
    build: impl FnOnce(&'static str) -> wgpu::RenderPipeline,
) -> (wgpu::RenderPipeline, (&'static str, f32)) {
    let t = std::time::Instant::now();
    let pso = build(label);
    (pso, (label, t.elapsed().as_secs_f32()))
}

/// A PSO label without the "PBR-lite " prefix and " Pipeline" suffix, for
/// the compile-time log line ("Surface Render", "Sun Shadow Alpha").
fn short_pso_label(label: &str) -> &str {
    label
        .strip_prefix("PBR-lite ")
        .unwrap_or(label)
        .strip_suffix(" Pipeline")
        .unwrap_or(label)
}

/// Every PSO `build_all_pipelines` compiles from the megashader, by name.
/// A named struct rather than a thirteen-element tuple so `new` and
/// `recreate_pipelines` cannot pair a fresh PSO with the wrong slot: a tuple
/// of thirteen identical types would let the water overlay land in the
/// surface overlay's field with no error anywhere, and the sea would then
/// draw through a pipeline whose entry has no ocean code.
struct MegashaderPsos {
    surface_render: wgpu::RenderPipeline,
    surface_transparent: wgpu::RenderPipeline,
    surface_overlay: wgpu::RenderPipeline,
    terrain_render: wgpu::RenderPipeline,
    vegetation_render: wgpu::RenderPipeline,
    water_transparent: wgpu::RenderPipeline,
    water_overlay: wgpu::RenderPipeline,
    shell_transparent: wgpu::RenderPipeline,
    cloud_transparent: wgpu::RenderPipeline,
    shadow: wgpu::RenderPipeline,
    shadow_alpha: wgpu::RenderPipeline,
    patch_render: wgpu::RenderPipeline,
    patch_shadow: wgpu::RenderPipeline,
}

// ── SHADER PERMUTATIONS (increment P1 of the frame-cost arc,
//    docs/design/frame-cost-arc.md section 1) ──
//
// Every PSO in `build_all_pipelines` compiles the ONE megashader module, and
// the material dispatch (90-fragment-main.wgsl) reaches three heavyweight
// early-return branches: the atmosphere integrator (type 14), the
// volumetric cloud march (type 15) and the ocean shell (type 16). A branch a
// pipeline can never take still costs it. The cloud march declares
// per-invocation `var<private>` tables (2.9 KB of zero-initialised,
// dynamically indexed storage in 41-cloud-bodies.wgsl), and the backend must
// materialise that frame for EVERY fragment of every pipeline whose entry
// point can reach it: terrain patches included, which never draw a shell.
// Phase A of P1 measured that reach directly (numbers in the design doc).
//
// The fix is the industry-standard one (Unreal calls them material
// permutations, Unity shader variants): WGSL `override` switches declared in
// assets/shaders/pbr/05-overrides.wgsl guard the three dispatch sites, and
// each PSO is compiled with the switches its material class needs through
// `wgpu::PipelineCompilationOptions::constants`. Naga substitutes the values
// before the backend ever sees the source, so a switched-off branch reaches
// DXC as `if (false && ...)` and is folded away together with everything
// only it referenced. One authored source, no duplicated shader text.
//
// Since increment P3 the module also has one fragment ENTRY per material
// class (`ShaderClass::fragment_entry`), and each PSO compiles only the
// entry of the class it draws, so the split is structural as well as
// switched: a surface PSO's DXIL never contains the terrain block, a
// terrain PSO's never contains the vegetation family, and the three
// switches guard their dispatches inside their own class entries.
//
// THIS TABLE IS THE REGISTRY, and its scope is exact: EVERY PSO WHOSE
// FRAGMENT ENTRY IS A CLASS ENTRY OR `fs_shadow` (or that has no fragment
// stage but compiles the megashader's vertex stage: the depth-only shadow
// PSO). A builder asks `pso_constants(label)` for its map and
// `pso_fragment_entry(label)` for its entry; an unregistered label panics at
// boot, so a new PSO must declare its row here; and `permutation_tests`
// below pins the table against the shader source, so nobody can re-enable a
// branch on the terrain PSOs, switch one off on a pipeline that draws
// through it, or compile a class's PSO from another class's entry, without
// a red test.
//
// EXEMPT, by fragment entry name: the six cloud fullscreen PSOs
// (`fs_cloud_screen`, `fs_cloud_light_bake`, `fs_cloud_profile_bake`,
// `fs_cloud_profile_mip`, `fs_cloud_profile_calib`,
// `fs_cloud_profile_calib_reduce`, all built through
// `build_cloud_fullscreen_pipeline`). They compile the same megashader
// module, but their entries are no class entry and reach no material
// dispatch (so no shell branch to switch off), and the cloud march is the
// program they exist to run, so a permutation would have nothing to remove.
// The test `every_megashader_pso_builder_asks_the_registry` lists them by
// name and fails on any other fragment entry compiled without the
// registry's map.

/// The three branch switches, by the exact `override` name the shader
/// declares (05-overrides.wgsl). A pipeline-constant key is the identifier
/// string when the declaration carries no `@id`.
pub const BRANCH_ATMOSPHERE: &str = "HAS_ATMOSPHERE_BRANCH";
pub const BRANCH_CLOUD: &str = "HAS_CLOUD_BRANCH";
pub const BRANCH_OCEAN: &str = "HAS_OCEAN_BRANCH";

/// Every switch the megashader declares, in declaration order. The
/// permutation test requires each one to exist with a `= true` default, so
/// every pipeline that does NOT name a switch keeps that branch compiled in.
pub const ALL_BRANCH_SWITCHES: &[&str] = &[BRANCH_ATMOSPHERE, BRANCH_CLOUD, BRANCH_OCEAN];

/// The fragment entry of the two alpha-cutout sun-shadow PSOs: ONE union
/// twin of the class entries' cutout discards (pinned to them by
/// `shadow_cutout_tests`), shared by every class because it carries no
/// heavyweight storage and the shadow pass has no class routing.
pub const SHADOW_FRAGMENT_ENTRY: &str = "fs_shadow";

// ── MATERIAL CLASSES (increment P2 of the frame-cost arc, six since P3) ──
//
// P1 proved the floor sits under EVERY PSO that compiles a fragment with the
// cloud branch reachable: the Sahara's atmosphere shell, drawn through the
// classic transparent PSO, fell from 10.44 to 0.19 ms the moment cloud_layer
// was unreachable, and the console room's interior walls were paying the
// same floor through the opaque PSO. So the material type space is cut into
// CLASSES, each class gets pipelines compiled from exactly the code its
// materials dispatch to (its own entry, with the switches it keeps), and the
// draw loops pick the pipeline by the class of the material in hand
// (`shader_class`, `Pipeline::opaque_for`, `transparent_for`,
// `overlay_for`). The atmosphere shell no longer shares a program with the
// cloud march; an interior wall no longer shares one with anything.

/// Which fragment ENTRY (and so which family of programs) a material draws
/// with. The classes are defined by the type tests in the six entries of
/// 90-fragment-main.wgsl and nothing else: `shader_class` is pinned to them
/// by `shader_class_is_pinned_to_the_class_entries`, so the classifier cannot
/// drift from the WGSL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderClass {
    /// The plain surfaces: the procedural surfaces 0..11, the sun's radial
    /// glow 17, gas giant bands 18, textured meshes 19, screens 24, and the
    /// default look for any type nothing else claims. Entry `fs_surface`.
    /// Its pipelines fold all three shell branches away.
    Surface,
    /// The planet surface, type 12: terrain patches, the sprite tree cards
    /// baked into them, the far canopy sheet and orbital water. Entry
    /// `fs_terrain`, the only entry that carries the ground detail stack.
    Terrain,
    /// The vegetation family: 20 procedural plant, 21 cluster card, 22
    /// baked bark, 23 grass strand. Entry `fs_vegetation`.
    Vegetation,
    /// The ocean shell, type 16 (`ocean_shell`). Entry `fs_water`, the only
    /// one that keeps `HAS_OCEAN_BRANCH`.
    Water,
    /// The atmosphere shells: 13 the Fresnel fallback, 14 the scattering
    /// atmosphere (`atmosphere_scattering`). Entry `fs_shell`, the only one
    /// that keeps `HAS_ATMOSPHERE_BRANCH`.
    Shell,
    /// The cloud shell, type 15 (`cloud_layer`), alone: the one class whose
    /// fragments genuinely run the march, so the only pipeline that pays
    /// for the march's per-invocation tables. Entry `fs_cloud`.
    Cloud,
}

impl ShaderClass {
    /// Every class, in a fixed order (the `present` bookkeeping in the
    /// opaque draw loops indexes by this position).
    pub const ALL: [ShaderClass; 6] = [
        ShaderClass::Surface,
        ShaderClass::Terrain,
        ShaderClass::Vegetation,
        ShaderClass::Water,
        ShaderClass::Shell,
        ShaderClass::Cloud,
    ];

    /// The class's name as the PSO labels spell it ("PBR-lite Surface
    /// Render Pipeline").
    pub const fn name(self) -> &'static str {
        match self {
            ShaderClass::Surface => "Surface",
            ShaderClass::Terrain => "Terrain",
            ShaderClass::Vegetation => "Vegetation",
            ShaderClass::Water => "Water",
            ShaderClass::Shell => "Shell",
            ShaderClass::Cloud => "Cloud",
        }
    }

    /// The `@fragment` entry point in 90-fragment-main.wgsl that carries
    /// this class's material code (P3). A PSO of this class compiles this
    /// entry and no other, so its DXIL never holds another class's blocks.
    pub const fn fragment_entry(self) -> &'static str {
        match self {
            ShaderClass::Surface => "fs_surface",
            ShaderClass::Terrain => "fs_terrain",
            ShaderClass::Vegetation => "fs_vegetation",
            ShaderClass::Water => "fs_water",
            ShaderClass::Shell => "fs_shell",
            ShaderClass::Cloud => "fs_cloud",
        }
    }

    /// The branch switches this class's pipelines keep ON, which since P3
    /// are exactly the switches its own entry guards a dispatch with (the
    /// other guards are not in the entry at all). Every switch appears in
    /// exactly one class's live set (pinned by the classifier test), so no
    /// two classes compile the same shell program.
    pub const fn live_branches(self) -> &'static [&'static str] {
        match self {
            ShaderClass::Surface | ShaderClass::Terrain | ShaderClass::Vegetation => &[],
            ShaderClass::Water => &[BRANCH_OCEAN],
            ShaderClass::Shell => &[BRANCH_ATMOSPHERE],
            ShaderClass::Cloud => &[BRANCH_CLOUD],
        }
    }

    /// The complement of `live_branches` over `ALL_BRANCH_SWITCHES`: what
    /// `pso_constants` switches OFF for a pipeline of this class. Spelled
    /// out per class (rather than computed) so it can be a `const` the
    /// registry table below reads at compile time.
    pub const fn dead_branches(self) -> &'static [&'static str] {
        match self {
            ShaderClass::Surface | ShaderClass::Terrain | ShaderClass::Vegetation => ALL_BRANCH_SWITCHES,
            ShaderClass::Water => &[BRANCH_ATMOSPHERE, BRANCH_CLOUD],
            ShaderClass::Shell => &[BRANCH_CLOUD, BRANCH_OCEAN],
            ShaderClass::Cloud => &[BRANCH_ATMOSPHERE, BRANCH_OCEAN],
        }
    }

    /// Whether materials of this class draw in the OPAQUE lists (and so
    /// have an opaque PSO, `Pipeline::opaque_for`). The shells, the clouds
    /// and the water are alpha-blended and ride the transparent lists only.
    pub const fn draws_opaque(self) -> bool {
        matches!(self, ShaderClass::Surface | ShaderClass::Terrain | ShaderClass::Vegetation)
    }
}

/// The order the opaque draw loops walk the classes present in a list (one
/// pass per class, `Renderer::draw_opaque_objects`). Within a class the
/// list order is kept (the lists are built roughly front to back and the
/// terrain patches are sorted so), and across classes the cheap fragments
/// go first so the expensive ones behind them are z-rejected: a wall or a
/// tree in front of a planet body saves that body's terrain fragment.
/// Opaque draws with depth test and write resolve to the same image in any
/// order, so this is a cost choice, not a correctness one.
pub const OPAQUE_CLASS_ORDER: [ShaderClass; 3] =
    [ShaderClass::Surface, ShaderClass::Vegetation, ShaderClass::Terrain];

/// The class of a material, from its `material.params.z` type value, using
/// the SAME half-open bands the class entries test (`>= 13.5 && < 14.5` is
/// type 14, and so on). Pure, so a draw loop can call it per object without
/// touching the renderer, and so the test can sweep the whole type space.
pub fn shader_class(material_type: f32) -> ShaderClass {
    let t = material_type;
    if t >= 11.5 && t < 12.5 {
        ShaderClass::Terrain
    } else if t >= 12.5 && t < 14.5 {
        ShaderClass::Shell
    } else if t >= 14.5 && t < 15.5 {
        ShaderClass::Cloud
    } else if t >= 15.5 && t < 16.5 {
        ShaderClass::Water
    } else if t >= 19.5 && t < 23.5 {
        ShaderClass::Vegetation
    } else {
        ShaderClass::Surface
    }
}

/// One row of the pipeline registry: a PSO by label, the material class it
/// serves (None for the two sun-shadow PSOs, which serve every caster), the
/// fragment entry it compiles (None for the depth-only shadow PSO) and the
/// branch switches compiled OUT of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PsoRow {
    pub label: &'static str,
    pub class: Option<ShaderClass>,
    pub fragment: Option<&'static str>,
    pub dead: &'static [&'static str],
}

/// A row for a colour PSO of `class`: its entry and its dead set come from
/// the class, so a row cannot pair a label with another class's entry.
const fn class_row(label: &'static str, class: ShaderClass) -> PsoRow {
    PsoRow { label, class: Some(class), fragment: Some(class.fragment_entry()), dead: class.dead_branches() }
}

/// A row for a sun-shadow PSO: no class (every caster draws with it), the
/// union `fs_shadow` entry or none, every switch off (a no-op the row
/// states explicitly rather than leaving to a default).
const fn shadow_row(label: &'static str, fragment: Option<&'static str>) -> PsoRow {
    PsoRow { label, class: None, fragment, dead: ALL_BRANCH_SWITCHES }
}

/// THE REGISTRY: every PSO `build_all_pipelines` compiles from the
/// megashader, by label, with the class it serves, the fragment entry it
/// compiles and the switches folded out of it. Thirteen rows, one per
/// (class, fixed-function state) the draw loops use, plus the two shadow
/// PSOs; the six cloud fullscreen PSOs are exempt (see the banner above)
/// and are deliberately absent.
///
/// Row by row:
/// - Surface: opaque (walls, floors, props, machines, planet bodies that
///   are not terrain, photoscanned trees), transparent (glass, holograms,
///   particles, the sun's core and halo) and overlay (the editor gizmos).
///   `fs_surface`, every shell switch off.
/// - Terrain: the classic opaque PSO (uniform-sphere planet bodies) and the
///   two terrain-batch PSOs below. `fs_terrain`, every shell switch off.
/// - Vegetation: opaque (near-tree parts, garden plants, the instanced
///   grass). `fs_vegetation`, every shell switch off.
/// - Water: transparent and, while `water_depth_write` holds, overlay.
///   `fs_water` with the ocean branch kept.
/// - Shell: transparent (the atmosphere shells). `fs_shell` with the
///   atmosphere branch kept. No overlay row: nothing routes an atmosphere
///   through an overlay list.
/// - Cloud: transparent, the cloud shell and nothing else. `fs_cloud` with
///   the march kept.
/// - The two sun-shadow PSOs: no class, `fs_shadow` for the cutout variant
///   and no fragment stage for the depth-only one, every switch off as a
///   stated no-op (fs_shadow never reaches the shells and vs_main reads no
///   switch, so the water's vertex displacement is untouched).
/// - The two terrain-batch PSOs draw planet-surface patches only (type 12
///   plus the sprite cards baked into the same meshes), never a shell:
///   `fs_terrain` and `fs_shadow`, every switch off.
pub const PSO_REGISTRY: &[PsoRow] = &[
    class_row("PBR-lite Surface Render Pipeline", ShaderClass::Surface),
    class_row("PBR-lite Surface Transparent Pipeline", ShaderClass::Surface),
    class_row("PBR-lite Surface Overlay Pipeline", ShaderClass::Surface),
    class_row("PBR-lite Terrain Render Pipeline", ShaderClass::Terrain),
    class_row("PBR-lite Vegetation Render Pipeline", ShaderClass::Vegetation),
    class_row("PBR-lite Water Transparent Pipeline", ShaderClass::Water),
    class_row("PBR-lite Water Overlay Pipeline", ShaderClass::Water),
    class_row("PBR-lite Shell Transparent Pipeline", ShaderClass::Shell),
    class_row("PBR-lite Cloud Transparent Pipeline", ShaderClass::Cloud),
    shadow_row("Sun Shadow Pipeline", None),
    shadow_row("Sun Shadow Alpha Pipeline", Some(SHADOW_FRAGMENT_ENTRY)),
    class_row("Patch Batch Render Pipeline", ShaderClass::Terrain),
    shadow_row("Patch Batch Shadow Pipeline", Some(SHADOW_FRAGMENT_ENTRY)),
];

/// The registry row for one PSO label. Panics on a label that is not in
/// `PSO_REGISTRY`: a pipeline built from the megashader without a declared
/// row is exactly the silent regression this registry exists to prevent,
/// and the panic lands at first boot, where the boot-verify rig catches it.
pub fn pso_row(label: &str) -> &'static PsoRow {
    PSO_REGISTRY.iter().find(|r| r.label == label).unwrap_or_else(|| {
        panic!(
            "PSO {label:?} is not in PSO_REGISTRY (src/renderer/pipeline.rs): declare which \
             class it serves, which fragment entry it compiles and which branches it \
             compiles out"
        )
    })
}

/// The pipeline-constant map for one registered PSO: each dead branch's
/// switch set to 0.0 (wgpu carries every override value as an f64; naga maps
/// it onto a `bool` override as `value != 0.0`). Switches not named in the
/// map keep the shader's `= true` default. Panics on an unregistered label
/// (see `pso_row`).
pub fn pso_constants(label: &str) -> HashMap<String, f64> {
    pso_row(label).dead.iter().map(|name| ((*name).to_string(), 0.0)).collect()
}

/// The fragment entry one registered PSO compiles: its class's entry, the
/// union `fs_shadow`, or None for the depth-only shadow PSO. The builders
/// take their entry from here rather than typing it, so a class's PSO can
/// never be compiled from another class's entry. Panics on an unregistered
/// label (see `pso_row`).
pub fn pso_fragment_entry(label: &str) -> Option<&'static str> {
    pso_row(label).fragment
}

/// Shared by the two test modules below: what the tests need from the ONE
/// assembled megashader source, read the same way the compiler reads it.
#[cfg(test)]
mod source_tests_support {
    /// `src` with every `//` comment blanked to spaces, BYTE for byte, so an
    /// offset into the result is the same offset into the original and a
    /// function named in prose can never be mistaken for a call to it. (WGSL
    /// and this file's builders have no string literals containing `//`.)
    pub fn code_only(src: &str) -> String {
        let mut out = String::with_capacity(src.len());
        for line in src.split_inclusive('\n') {
            let Some(at) = line.find("//") else {
                out.push_str(line);
                continue;
            };
            out.push_str(&line[..at]);
            let rest = &line[at..];
            let (body, newline) = match rest.strip_suffix("\r\n") {
                Some(b) => (b, "\r\n"),
                None => match rest.strip_suffix('\n') {
                    Some(b) => (b, "\n"),
                    None => (rest, ""),
                },
            };
            // One space per BYTE (a comment may hold multi-byte glyphs).
            out.push_str(&" ".repeat(body.len()));
            out.push_str(newline);
        }
        out
    }

    /// Byte offset of the `}` that closes the `{` at `open`.
    pub fn matching_brace(code: &str, open: usize) -> usize {
        let mut depth = 0usize;
        for (i, b) in code.bytes().enumerate().skip(open) {
            match b {
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return i;
                    }
                }
                _ => {}
            }
        }
        panic!("unbalanced braces after byte {open}");
    }

    /// The byte range `[start, end)` of the BODY (inside the braces) of the
    /// function `name` in comment-blanked `code`, located by its `fn name(`
    /// signature. Panics if the function is not defined exactly once.
    pub fn function_body(code: &str, name: &str) -> (usize, usize) {
        let sig = format!("fn {name}(");
        let sites: Vec<usize> = code.match_indices(&sig).map(|(p, _)| p).collect();
        assert_eq!(sites.len(), 1, "expected exactly one definition of `{sig}`, found {}", sites.len());
        let open = sites[0] + code[sites[0]..].find('{').expect("a function body opens a brace");
        (open + 1, matching_brace(code, open))
    }

    /// 1-based line number of byte offset `at` in `text`, for messages.
    pub fn line_of(text: &str, at: usize) -> usize {
        text[..at].matches('\n').count() + 1
    }
}

/// The class entries <-> fs_shadow cutout mirror (v0.1106, per class since
/// P3).
///
/// A shader discard cannot be unit-tested from the CPU, so what IS testable
/// is that the entries and their shadow twin agree. Every check below reads
/// the ONE assembled megashader source and requires the same literal inside
/// the named class entry (or the shared prologue) and inside fs_shadow, so
/// changing a cutout threshold in one entry and forgetting the shadow twin
/// fails here instead of silently restoring the solid-board shadows this
/// pass removed. It also pins the DELIBERATE omission (the eye-distance card
/// discards must NOT appear in fs_shadow).
#[cfg(test)]
mod shadow_cutout_tests {
    use super::source_tests_support::{code_only, function_body};
    use super::super::shader_loader::assembled_pbr_source;
    use super::{ShaderClass, SHADOW_FRAGMENT_ENTRY};

    /// (class-entries side, fs_shadow side) of the assembled source.
    fn halves() -> (&'static str, &'static str) {
        let src = assembled_pbr_source();
        let at = src
            .find("fn fs_shadow")
            .expect("fs_shadow entry point missing from the megashader");
        (&src[..at], &src[at..])
    }

    /// The four cutouts, each identified by the exact expressions both sides
    /// must contain, and the function on the colour side that owns it: the
    /// shared prologue for the LOD dither (every class runs it), otherwise
    /// the one class entry whose material carries the cutout.
    const MIRRORED: &[(&str, &[&str])] = &[
        // 1. LOD crossfade dither, in the prologue every entry runs first.
        (
            "frag_prologue",
            &[
                "let b = (f32(bayer_i) + 0.5) / 16.0;",
                "if (b >= lod_fade) { discard; }",
                "if (b < -lod_fade) { discard; }",
            ],
        ),
        // 2. Type 19 photoscanned foliage: a surface.
        (
            "fs_surface",
            &["material_type >= 18.5 && material_type < 19.5", "if (mesh_tex.a < 0.35) {"],
        ),
        // 3. Type 21 baked cluster card: vegetation.
        (
            "fs_vegetation",
            &["material_type >= 20.5 && material_type < 21.5", "if (cc_tex.a < 0.5) {"],
        ),
        // 4. Type 12 sprite tree card (6x8 atlas grid + resident bit): terrain.
        (
            "fs_terrain",
            &[
                "let pw_bits_card = u32(round(max(material.params.w, 0.0)));",
                "if ((pw_bits_card & 4u) != 0u) {",
                "(f32(tile % 6u) + u01) / 6.0,",
                "(f32(tile / 6u) + (1.0 - v01)) / 8.0,",
                "if (spr.a < 0.5) {",
            ],
        ),
    ];

    #[test]
    fn fs_shadow_mirrors_the_class_entry_cutouts() {
        let (colour, shadow) = halves();
        let code = code_only(colour);
        for (owner, needles) in MIRRORED {
            let (start, end) = function_body(&code, owner);
            let body = &colour[start..end];
            for needle in *needles {
                assert!(
                    body.contains(needle),
                    "{owner} no longer contains {needle:?} - if the cutout moved to another \
                     entry or changed, update fs_shadow and this table in the SAME edit"
                );
                assert!(
                    shadow.contains(needle),
                    "fs_shadow is missing the {owner} cutout {needle:?} - a \
                     mostly-transparent caster will cast a solid board again"
                );
            }
        }
        // The owners named above are the shared prologue and real class
        // entries, so a renamed entry cannot leave a needle unowned.
        for (owner, _) in MIRRORED {
            assert!(
                *owner == "frag_prologue" || ShaderClass::ALL.iter().any(|c| c.fragment_entry() == *owner),
                "{owner} is neither the prologue nor a class entry"
            );
        }
    }

    #[test]
    fn fs_shadow_omits_the_eye_distance_card_discards() {
        let (colour, shadow) = halves();
        let eye_gate = "card_dist < shadow_u.params.w";
        assert!(colour.contains(eye_gate), "the terrain entry lost its card LOD window");
        // Only inside a comment, never as code: the shadow of a card the EYE
        // has swapped for a 3D model belongs to the model, and gating it on
        // viewer distance makes shadows blink as the player walks.
        for line in shadow.lines() {
            let code = line.split("//").next().unwrap_or("");
            assert!(
                !code.contains("card_dist"),
                "fs_shadow must not gate casters on EYE distance: {}",
                line.trim()
            );
        }
    }

    /// The whole point of the split: `fs_shadow` exists as a real entry point
    /// and the megashader still validates with it present. (The assembled
    /// source is what both PSO sets compile, and the hot-reload gate parses
    /// this same string.)
    #[test]
    fn fs_shadow_is_a_declared_fragment_entry_point() {
        let src = assembled_pbr_source();
        let at = src
            .find(&format!("fn {SHADOW_FRAGMENT_ENTRY}(in: VertexOutput) {{"))
            .expect("fs_shadow must take VertexOutput and return nothing");
        // Whitespace-tolerant rather than "@fragment\nfn ...": this repo's
        // WGSL is checked out CRLF on Windows and LF on the Linux CI runner,
        // so a hardcoded newline would be a test that passes on one machine
        // and fails on the other.
        assert!(
            src[..at].trim_end().ends_with("@fragment"),
            "fs_shadow needs its @fragment attribute IMMEDIATELY above it - \
             naga silently accepts an attribute orphaned onto something else, \
             and the module then validates fine and dies at pipeline creation \
             with \"Unable to find entry point\" (the v0.876 lesson)"
        );
    }
}

/// The shader-permutation registry against the shader source (P1, P2, P3).
///
/// Four things must agree or a terrain fragment silently pays for the cloud
/// march again: the `override` declarations in 05-overrides.wgsl, the guards
/// on the three shell dispatches, the class entries those guards live in,
/// and PSO_REGISTRY. None of that is visible to the GPU-less test runner, so
/// every check reads the ONE assembled source (and this file's own text for
/// the builder wiring), the same way the cutout-mirror tests above do.
#[cfg(test)]
mod permutation_tests {
    use super::source_tests_support::{code_only, function_body, line_of, matching_brace};
    use super::super::shader_loader::{assembled_pbr_batch_source, assembled_pbr_source};
    use super::*;

    /// The class-entries side of the assembled source (everything before
    /// fs_shadow: the shared parts, the prologue and tail, the six entries).
    fn class_entries_side() -> &'static str {
        let src = assembled_pbr_source();
        let at = src.find("fn fs_shadow").expect("fs_shadow missing");
        &src[..at]
    }

    /// Byte offsets of every WHOLE-WORD occurrence of `ident` in `code`: a
    /// match with an identifier character on either side (`cloud_layer_x`,
    /// `my_cloud_layer`) is a different name and is skipped.
    fn identifier_sites(code: &str, ident: &str) -> Vec<usize> {
        let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        let bytes = code.as_bytes();
        let mut sites = Vec::new();
        let mut from = 0;
        while let Some(rel) = code[from..].find(ident) {
            let at = from + rel;
            let end = at + ident.len();
            let before_ok = at == 0 || !is_word(bytes[at - 1]);
            let after_ok = end >= bytes.len() || !is_word(bytes[end]);
            if before_ok && after_ok {
                sites.push(at);
            }
            // `ident` is ASCII, so one byte past its start is a char boundary.
            from = at + 1;
        }
        sites
    }

    #[test]
    fn the_megashader_declares_every_switch_defaulting_on() {
        // Both modules the boot compiles (classic + terrain-batch variant)
        // must carry the declarations: the batch substitution only swaps the
        // OBJECT-SOURCE block, and a switch missing from either module makes
        // naga silently ignore the constant for it (process_overrides
        // returns early on a module with no overrides), which would re-enable
        // the branch with no error anywhere.
        for (which, src) in [("classic", assembled_pbr_source()), ("batch", assembled_pbr_batch_source())] {
            for name in ALL_BRANCH_SWITCHES {
                let decl = format!("override {name}: bool = true;");
                assert!(
                    src.contains(&decl),
                    "{which} megashader lacks {decl:?} - every pipeline that does not \
                     name the switch relies on that `= true` default"
                );
            }
        }
    }

    /// (switch, the dispatch condition it guards, the function it makes
    /// dead, the class whose entry the guard must live in). The switch is
    /// written FIRST in the `&&` by convention, the one shape this test pins
    /// so every guard reads the same and can be found here; the operand
    /// order changes nothing about how a false constant folds the condition.
    const GUARDED_DISPATCHES: &[(&str, &str, &str, ShaderClass)] = &[
        (BRANCH_ATMOSPHERE, "material_type >= 13.5 && material_type < 14.5", "atmosphere_scattering", ShaderClass::Shell),
        (BRANCH_CLOUD, "material_type >= 14.5 && material_type < 15.5", "cloud_layer", ShaderClass::Cloud),
        (BRANCH_OCEAN, "material_type >= 15.5 && material_type < 16.5", "ocean_shell", ShaderClass::Water),
    ];

    #[test]
    fn each_shell_dispatch_is_guarded_by_its_switch_inside_its_class_entry() {
        // Comment-blanked, so a function named in prose is not a use of it,
        // and so the offsets below are offsets into the real source.
        let main = code_only(class_entries_side());
        for (switch, cond, func, class) in GUARDED_DISPATCHES {
            let guard = format!("if ({switch} && {cond}) {{");
            let at = main.find(&guard).unwrap_or_else(|| {
                panic!("no class entry guards the shell dispatch with {guard:?}")
            });
            // The guard must sit INSIDE the entry of the class whose
            // pipelines keep the switch (P3): that is what makes the split
            // structural. A guard in another entry would compile the shell
            // program into that class's PSOs too.
            let entry = class.fragment_entry();
            let (start, end) = function_body(&main, entry);
            assert!(
                at > start && at < end,
                "the {switch} guard is at line {} but must be inside {entry} (lines {}..{})",
                line_of(&main, at),
                line_of(&main, start),
                line_of(&main, end)
            );
            let after = &main[at + guard.len()..];
            let call = format!("return {func}(");
            let next_line = after.lines().nth(1).unwrap_or("").trim();
            assert!(
                next_line.starts_with(&call),
                "the line after {guard:?} must be the {call:?} return, found {next_line:?}"
            );
            // Where the guarded call's function name begins, as a byte offset
            // into `main`.
            let guarded_at = at
                + guard.len()
                + after.find(&call).expect("the call was found on the next line")
                + "return ".len();

            // The bare token, not `return cloud_layer(`: EVERY whole-word use
            // of the function name before fs_shadow, minus its `fn`
            // definition, must be exactly one, and that one must be the
            // guarded dispatch. A `let c = cloud_layer(p, ff);` anywhere
            // else in an entry, the prologue, the tail or a helper they call
            // would make the branch reachable regardless of the switch.
            let uses: Vec<usize> = identifier_sites(&main, func)
                .into_iter()
                .filter(|&p| !main[..p].ends_with("fn "))
                .collect();
            let where_ = |p: &usize| {
                let line = line_of(&main, *p);
                let text = main[*p..].lines().next().unwrap_or("").trim();
                format!("line {line}: {text}")
            };
            assert_eq!(
                uses.len(),
                1,
                "{func} must be used in exactly ONE place before fs_shadow (the guarded \
                 dispatch inside {entry}), found {}:\n  {}",
                uses.len(),
                uses.iter().map(where_).collect::<Vec<_>>().join("\n  ")
            );
            assert_eq!(
                uses[0],
                guarded_at,
                "the one use of {func} must be the guarded dispatch, but it is at {}",
                where_(&uses[0])
            );
            assert!(
                main[uses[0] + func.len()..].trim_start().starts_with('('),
                "the one use of {func} must be a call"
            );
        }
    }

    /// Every `HAS_*` identifier a class entry's body reads. The shared
    /// prologue and tail must read none (they are compiled into every class).
    fn switches_read_by(code: &str, func: &str) -> Vec<String> {
        let (start, end) = function_body(code, func);
        let body = &code[start..end];
        let mut out: Vec<String> = Vec::new();
        for (p, _) in body.match_indices("HAS_") {
            let word: String = body[p..]
                .chars()
                .take_while(|c| c.is_ascii_uppercase() || *c == '_' || c.is_ascii_digit())
                .collect();
            if !out.contains(&word) {
                out.push(word);
            }
        }
        out
    }

    /// The registry's live sets ARE the switches the entries read (P3): a
    /// class keeps exactly the switches its own entry guards a dispatch
    /// with, and the code every class shares reads none. Proven red once by
    /// moving the ocean guard's `HAS_OCEAN_BRANCH && ` into fs_shell's
    /// atmosphere guard (the test named fs_shell and the extra switch), then
    /// restored.
    #[test]
    fn each_class_entry_reads_exactly_its_live_switches() {
        let code = code_only(class_entries_side());
        for class in ShaderClass::ALL {
            let entry = class.fragment_entry();
            let mut read = switches_read_by(&code, entry);
            read.sort();
            let mut live: Vec<String> = class.live_branches().iter().map(|s| s.to_string()).collect();
            live.sort();
            assert_eq!(
                read, live,
                "{entry} reads the switches {read:?} but the {class:?} class keeps {live:?} live; \
                 the registry and the entry must agree"
            );
        }
        for shared in ["frag_prologue", "frag_tail"] {
            let read = switches_read_by(&code, shared);
            assert!(read.is_empty(), "{shared} is compiled into every class and must read no switch, reads {read:?}");
        }
    }

    /// The expected registry, PSO by PSO. A hand-written twin of
    /// PSO_REGISTRY so a row cannot be changed without this test naming the
    /// change: nine class rows over six classes, two sun-shadow rows, two
    /// terrain-batch rows.
    const EXPECTED_ROWS: &[(&str, Option<ShaderClass>, Option<&str>)] = &[
        ("PBR-lite Surface Render Pipeline", Some(ShaderClass::Surface), Some("fs_surface")),
        ("PBR-lite Surface Transparent Pipeline", Some(ShaderClass::Surface), Some("fs_surface")),
        ("PBR-lite Surface Overlay Pipeline", Some(ShaderClass::Surface), Some("fs_surface")),
        ("PBR-lite Terrain Render Pipeline", Some(ShaderClass::Terrain), Some("fs_terrain")),
        ("PBR-lite Vegetation Render Pipeline", Some(ShaderClass::Vegetation), Some("fs_vegetation")),
        ("PBR-lite Water Transparent Pipeline", Some(ShaderClass::Water), Some("fs_water")),
        ("PBR-lite Water Overlay Pipeline", Some(ShaderClass::Water), Some("fs_water")),
        ("PBR-lite Shell Transparent Pipeline", Some(ShaderClass::Shell), Some("fs_shell")),
        ("PBR-lite Cloud Transparent Pipeline", Some(ShaderClass::Cloud), Some("fs_cloud")),
        ("Sun Shadow Pipeline", None, None),
        ("Sun Shadow Alpha Pipeline", None, Some("fs_shadow")),
        ("Patch Batch Render Pipeline", Some(ShaderClass::Terrain), Some("fs_terrain")),
        ("Patch Batch Shadow Pipeline", None, Some("fs_shadow")),
    ];

    #[test]
    fn the_registry_covers_exactly_the_thirteen_megashader_psos() {
        let labels: Vec<&str> = PSO_REGISTRY.iter().map(|r| r.label).collect();
        let expected: Vec<&str> = EXPECTED_ROWS.iter().map(|(l, _, _)| *l).collect();
        assert_eq!(labels, expected, "the registry's rows (or their order) changed");
        for (row, (label, class, entry)) in PSO_REGISTRY.iter().zip(EXPECTED_ROWS) {
            assert_eq!(row.class, *class, "{label}: class");
            assert_eq!(row.fragment, *entry, "{label}: fragment entry");
            assert_eq!(pso_fragment_entry(label), *entry, "{label}: pso_fragment_entry");
            let dead_expected = class.map_or(ALL_BRANCH_SWITCHES, |c| c.dead_branches());
            assert_eq!(row.dead, dead_expected, "{label}: dead set must be its class's (or all, for a shadow PSO)");
            if let Some(class) = class {
                // A class row compiles its class's entry, and its dead and
                // live sets partition the switch list: nothing is both,
                // nothing is neither.
                assert_eq!(row.fragment, Some(class.fragment_entry()), "{label}: entry must be the class's");
                for name in ALL_BRANCH_SWITCHES {
                    let live = class.live_branches().contains(name);
                    let is_dead = row.dead.contains(name);
                    assert!(
                        live != is_dead,
                        "{label}: {name} must be exactly one of live / dead for {class:?} \
                         (live = {live}, dead = {is_dead})"
                    );
                }
            }
            // Every dead name is a declared switch, and the map carries 0.0
            // (naga's bool mapping: value != 0.0) for exactly those names.
            let map = pso_constants(label);
            assert_eq!(map.len(), row.dead.len(), "{label}: one constant per dead branch");
            for name in row.dead {
                assert!(ALL_BRANCH_SWITCHES.contains(name), "{label}: {name} is not a declared switch");
                assert_eq!(map.get(*name), Some(&0.0), "{label}: {name} must be switched OFF (0.0)");
            }
        }
        // The three opaque classes carry the SAME permutation (all off): an
        // opaque fragment never enters a shell.
        for label in [
            "PBR-lite Surface Render Pipeline",
            "PBR-lite Terrain Render Pipeline",
            "PBR-lite Vegetation Render Pipeline",
            "Patch Batch Render Pipeline",
        ] {
            assert_eq!(pso_constants(label).len(), ALL_BRANCH_SWITCHES.len(), "{label}: all three off");
        }
        // Every class that draws opaque has an opaque row and vice versa.
        for class in ShaderClass::ALL {
            let has_opaque_row = PSO_REGISTRY
                .iter()
                .any(|r| r.class == Some(class) && r.label.ends_with(" Render Pipeline"));
            assert_eq!(
                has_opaque_row,
                class.draws_opaque(),
                "{class:?}: draws_opaque() and the registry's opaque rows disagree"
            );
        }
    }

    /// The `@fragment` entry points the assembled module declares, in
    /// source order, read the way naga reads them (the attribute and the
    /// `fn` may be separated by whitespace only).
    fn declared_fragment_entries(src: &str) -> Vec<String> {
        let code = code_only(src);
        let mut out = Vec::new();
        for (p, _) in code.match_indices("@fragment") {
            let rest = code[p + "@fragment".len()..].trim_start();
            let Some(sig) = rest.strip_prefix("fn ") else {
                panic!("@fragment at line {} is not followed by `fn`", line_of(&code, p));
            };
            let name: String = sig.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            out.push(name);
        }
        out
    }

    /// The module's fragment entries are EXACTLY the six class entries, the
    /// shadow twin and the six exempt cloud passes: no fs_main lingering
    /// (a pipeline could compile it and silently pay the whole-fragment
    /// floor again), and no new colour entry without a class row.
    #[test]
    fn the_fragment_entries_are_the_class_entries_the_shadow_twin_and_the_exempt_passes() {
        for (which, src) in [("classic", assembled_pbr_source()), ("batch", assembled_pbr_batch_source())] {
            let mut declared = declared_fragment_entries(src);
            declared.sort();
            let mut expected: Vec<String> = ShaderClass::ALL.iter().map(|c| c.fragment_entry().to_string()).collect();
            expected.push(SHADOW_FRAGMENT_ENTRY.to_string());
            expected.extend(REGISTRY_EXEMPT_FRAGMENT_ENTRIES.iter().map(|s| s.to_string()));
            expected.sort();
            assert_eq!(
                declared, expected,
                "{which} megashader's @fragment entries are not the class entries + fs_shadow + \
                 the exempt cloud passes"
            );
            assert!(!src.contains("fn fs_main("), "{which}: fs_main must not exist any more (P3)");
        }
    }

    /// Every `if (HAS_X && material_type >= a && material_type < b)` guard
    /// in the class entries, as (switch, a, b), read out of the SHADER so
    /// the test cannot agree with a stale copy of the bands.
    fn guarded_dispatch_bands() -> Vec<(&'static str, f32, f32)> {
        let main = code_only(class_entries_side());
        let mut out = Vec::new();
        for switch in ALL_BRANCH_SWITCHES {
            let prefix = format!("if ({switch} && material_type >= ");
            let at = main
                .find(&prefix)
                .unwrap_or_else(|| panic!("no class entry has a guard reading {prefix:?}"));
            let rest = &main[at + prefix.len()..];
            let (lo, rest) = rest
                .split_once(" && material_type < ")
                .expect("a guard's band is `>= a && material_type < b`");
            let hi: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            out.push((*switch, lo.trim().parse().expect("band low"), hi.parse().expect("band high")));
        }
        out
    }

    /// The class of every shipped material type, by hand, so the
    /// classifier's answer for each is written down where a reader can see
    /// it and a change to `shader_class` is a visible diff here too.
    const EXPECTED_CLASS_BY_TYPE: &[(u32, ShaderClass)] = &[
        (0, ShaderClass::Surface),
        (1, ShaderClass::Surface),
        (2, ShaderClass::Surface),
        (3, ShaderClass::Surface),
        (4, ShaderClass::Surface),
        (5, ShaderClass::Surface),
        (6, ShaderClass::Surface),
        (7, ShaderClass::Surface),
        (8, ShaderClass::Surface),
        (9, ShaderClass::Surface),
        (10, ShaderClass::Surface),
        (11, ShaderClass::Surface),
        (12, ShaderClass::Terrain),
        (13, ShaderClass::Shell),
        (14, ShaderClass::Shell),
        (15, ShaderClass::Cloud),
        (16, ShaderClass::Water),
        (17, ShaderClass::Surface),
        (18, ShaderClass::Surface),
        (19, ShaderClass::Surface),
        (20, ShaderClass::Vegetation),
        (21, ShaderClass::Vegetation),
        (22, ShaderClass::Vegetation),
        (23, ShaderClass::Vegetation),
        (24, ShaderClass::Surface),
    ];

    /// The two shapes a class entry tests a type with: the chain arm
    /// `material_type < T.5` (types 0..11, whose arms are ordered so the
    /// lower bound is the previous arm) and the explicit band
    /// `material_type >= (T-1).5 && material_type < T.5` (everything else,
    /// including the switch-guarded shells).
    fn dispatch_literals(t: u32) -> [String; 2] {
        let hi = t as f32 + 0.5;
        let lo = t as f32 - 0.5;
        [
            format!("material_type < {hi:.1}"),
            format!("material_type >= {lo:.1} && material_type < {hi:.1}"),
        ]
    }

    /// THE CLASSIFIER IS PINNED TO THE WGSL (P2, per entry since P3).
    /// Three ways: every type inside a switch-guarded band goes to the class
    /// whose pipelines keep that switch; every shipped type's class is the
    /// hand table above AND that class's entry is the ONE entry whose body
    /// tests for the type; and the half-step band edges land where the
    /// entries' half-open tests put them. Proven red once by sending type
    /// 23 to Surface in `shader_class` (the test named type 23, fs_surface
    /// and fs_vegetation), then restored.
    #[test]
    fn shader_class_is_pinned_to_the_class_entries() {
        // (a) The guarded bands, read from the shader.
        let bands = guarded_dispatch_bands();
        assert_eq!(bands.len(), ALL_BRANCH_SWITCHES.len(), "one guarded band per switch");
        for (i, (sa, la, ha)) in bands.iter().enumerate() {
            for (sb, lb, hb) in bands.iter().skip(i + 1) {
                assert!(ha <= lb || hb <= la, "guard bands overlap: {sa} [{la}, {ha}) and {sb} [{lb}, {hb})");
            }
        }
        for (switch, lo, hi) in &bands {
            let mut t = *lo;
            while t < *hi {
                let class = shader_class(t);
                assert!(
                    class.live_branches().contains(switch),
                    "material type {t} is inside the {switch} band [{lo}, {hi}) but shader_class \
                     sends it to {class:?}, whose pipelines compile that branch OUT; it would \
                     render as the default look"
                );
                t += 0.5;
            }
        }

        // (b) Every shipped type: the hand table, and the entry bodies.
        let code = code_only(class_entries_side());
        let bodies: Vec<(ShaderClass, &str)> = ShaderClass::ALL
            .iter()
            .map(|c| {
                let (s, e) = function_body(&code, c.fragment_entry());
                (*c, &code[s..e])
            })
            .collect();
        assert_eq!(EXPECTED_CLASS_BY_TYPE.len(), 25, "types 0..=24 are the shipped type space");
        for (t, expected) in EXPECTED_CLASS_BY_TYPE {
            let got = shader_class(*t as f32);
            assert_eq!(got, *expected, "shader_class({t}) must be {expected:?}");
            let literals = dispatch_literals(*t);
            let owners: Vec<ShaderClass> = bodies
                .iter()
                .filter(|(_, body)| literals.iter().any(|l| body.contains(l.as_str())))
                .map(|(c, _)| *c)
                .collect();
            assert_eq!(
                owners,
                vec![*expected],
                "material type {t} must be dispatched by exactly one entry, {}'s, but the entries \
                 testing for it are {:?} (looked for {literals:?})",
                expected.fragment_entry(),
                owners.iter().map(|c| c.fragment_entry()).collect::<Vec<_>>()
            );
        }

        // (c) Band edges: the bands are half-open at .5, so t + 0.5 belongs
        // to type t + 1, and the space outside 0..24.5 is the default look.
        for (t, _) in EXPECTED_CLASS_BY_TYPE.iter().take(24) {
            let edge = *t as f32 + 0.5;
            let next = EXPECTED_CLASS_BY_TYPE[*t as usize + 1].1;
            assert_eq!(shader_class(edge), next, "the band edge {edge} belongs to type {}", t + 1);
        }
        assert_eq!(shader_class(-0.5), ShaderClass::Surface, "below the type space: the default look");
        assert_eq!(shader_class(24.5), ShaderClass::Surface, "above the type space: the default look");
        assert_eq!(shader_class(super::super::materials::MATERIAL_TYPE_SCREEN), ShaderClass::Surface);

        // (d) Each switch is live in exactly ONE class, the cloud switch's
        // class keeps nothing else, and the opaque classes keep none: the
        // march's per-invocation frame is charged to cloud-shell fragments
        // and to no other material. That is the number P2 exists to move.
        for switch in ALL_BRANCH_SWITCHES {
            let owners: Vec<ShaderClass> =
                ShaderClass::ALL.iter().copied().filter(|c| c.live_branches().contains(switch)).collect();
            assert_eq!(owners.len(), 1, "{switch} must be live in exactly one class, found {owners:?}");
        }
        assert_eq!(ShaderClass::Cloud.live_branches(), &[BRANCH_CLOUD], "the cloud class keeps only the march");
        for class in ShaderClass::ALL.iter().filter(|c| c.draws_opaque()) {
            assert!(class.live_branches().is_empty(), "{class:?} draws opaque and keeps no shell branch");
        }
        // `ShaderClass::ALL` is in discriminant order: the opaque draw
        // loops index their `present` table by `class as usize`.
        for (i, class) in ShaderClass::ALL.iter().enumerate() {
            assert_eq!(*class as usize, i, "ShaderClass::ALL must list the classes in discriminant order");
        }
        // Each class entry name is unique and starts with `fs_`.
        let mut entries: Vec<&str> = ShaderClass::ALL.iter().map(|c| c.fragment_entry()).collect();
        entries.sort();
        entries.dedup();
        assert_eq!(entries.len(), ShaderClass::ALL.len(), "two classes share a fragment entry");
        assert!(entries.iter().all(|e| e.starts_with("fs_")), "class entries are fs_* names");
    }

    #[test]
    #[should_panic(expected = "not in PSO_REGISTRY")]
    fn pso_constants_panics_on_an_unregistered_label() {
        let _ = pso_constants("Some Future Pipeline");
    }

    #[test]
    #[should_panic(expected = "not in PSO_REGISTRY")]
    fn pso_fragment_entry_panics_on_an_unregistered_label() {
        let _ = pso_fragment_entry("Some Future Pipeline");
    }

    /// Fragment entry points allowed to compile the megashader WITHOUT the
    /// registry's map, by name. These are the six cloud fullscreen passes:
    /// their entries are no class entry and reach no material dispatch (so
    /// there is no shell branch to switch off), and the cloud march is the
    /// program they exist to run, so a permutation would have nothing to
    /// remove. Every other fragment entry compiled from the module must be
    /// wired to `pso_constants(label)` and `pso_fragment_entry(label)`. Add
    /// a name here ONLY for an entry that is not a class entry and cannot
    /// reach one; the banner above PSO_REGISTRY lists the same six.
    pub(super) const REGISTRY_EXEMPT_FRAGMENT_ENTRIES: &[&str] = &[
        "fs_cloud_screen",
        "fs_cloud_light_bake",
        "fs_cloud_profile_bake",
        "fs_cloud_profile_mip",
        "fs_cloud_profile_calib",
        "fs_cloud_profile_calib_reduce",
    ];

    /// The one builder whose fragment entry is a PARAMETER rather than the
    /// registry's: `build_cloud_fullscreen_pipeline`, which every cloud PSO
    /// goes through. Its callers supply the entry, so the test checks every
    /// `"fs_..."` literal in the impl block against the exempt list: since
    /// P3 no registry entry is ever a literal inside `impl Pipeline` (the
    /// builders take theirs from `pso_fragment_entry`), so any other literal
    /// is a PSO routed around the registry.
    const PARAMETRIC_EXEMPT_BUILDER: &str = "build_cloud_fullscreen_pipeline";

    /// The fragment entry a PSO descriptor names.
    #[derive(Debug)]
    enum Entry<'a> {
        /// `entry_point: Some("fs_cloud_screen")`.
        Literal(&'a str),
        /// `entry_point: Some(entry)`: a variable the builder holds.
        Ident(&'a str),
    }

    /// The fragment entry of one `create_render_pipeline` descriptor, or
    /// None when it has no fragment stage. Relies on the vertex state
    /// preceding the fragment state, which every descriptor in this file
    /// does (so the first `entry_point:` after `fragment:` is the fragment's).
    fn fragment_entry(desc: &str) -> Option<Entry<'_>> {
        let frag = desc.find("fragment:")?;
        let ep = desc[frag..].find("entry_point: Some(")?;
        let arg_start = frag + ep + "entry_point: Some(".len();
        let arg_end = arg_start + desc[arg_start..].find(')')?;
        let arg = desc[arg_start..arg_end].trim();
        Some(match arg.strip_prefix('"').and_then(|a| a.strip_suffix('"')) {
            Some(literal) => Entry::Literal(literal),
            None => Entry::Ident(arg),
        })
    }

    /// The builders must actually ASK the registry, and this test must be
    /// able to FAIL for the likeliest regression: a new builder that
    /// compiles the megashader with default options, or one that types its
    /// own fragment entry. An adversarial review of the first version showed
    /// it could not (it asserted one spelling of "default", it compared
    /// counts a non-asking builder does not perturb, and its scan started
    /// after the cloud builders). So this version reads the WHOLE `impl
    /// Pipeline` block, finds every `create_render_pipeline` descriptor, and
    /// judges each on its own: every one of its `compilation_options:` sites
    /// is wired to `&constants` from `pso_constants(label)` AND its fragment
    /// entry is the `entry` from `pso_fragment_entry(label)` (or absent), OR
    /// its fragment entry is one of the exempt cloud entries above. Proven
    /// red by adding a builder spelled `compilation_options:
    /// Default::default()` with a class fragment entry (the reviewer's
    /// mutation), and again in P3 by typing `entry_point: Some("fs_surface")`
    /// into make_pbr (the test named the literal and the line), then
    /// restored.
    #[test]
    fn every_megashader_pso_builder_asks_the_registry() {
        let me = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/renderer/pipeline.rs"))
            .expect("pipeline.rs readable");
        // The scan window: the whole `impl Pipeline` block, cloud helpers
        // included, up to the registry banner (which keeps this test
        // module's own text out of it).
        let start = me.find("\nimpl Pipeline {").expect("impl Pipeline block");
        let end = me.find("// ── SHADER PERMUTATIONS").expect("registry banner");
        assert!(start < end, "the impl block must precede the registry banner");
        let region = &me[start..end];
        assert!(
            !region.contains("#[cfg(test)]"),
            "the scan window must hold builders only, never test text"
        );
        let code = code_only(region);
        const OPTIONS_KEY: &str = "compilation_options:";
        const REGISTRY_FORM: &str = "wgpu::PipelineCompilationOptions{constants:&constants,";

        let mut descriptors = 0usize;
        let mut sites_seen = 0usize;
        let mut registry_builders: Vec<&str> = Vec::new();
        let mut from = 0usize;
        while let Some(rel) = code[from..].find("create_render_pipeline(") {
            let at = from + rel;
            descriptors += 1;
            let open = at + code[at..].find('{').expect("a descriptor opens a brace");
            let close = matching_brace(&code, open);
            let desc = &code[at..=close];
            from = close;

            // The builder this descriptor belongs to: the nearest `fn` or
            // closure (`= |`) opening before it, named for the messages.
            let builder_start = [
                code[..at].rfind("\n    fn "),
                code[..at].rfind("\n    pub fn "),
                code[..at].rfind("= |"),
            ]
            .into_iter()
            .flatten()
            .max()
            .expect("a builder encloses every descriptor");
            // The `fn` patterns match starting AT the newline before the
            // signature; step past it so the line lookup lands on the
            // signature line, not the (blanked) doc comment above it.
            let name_from = builder_start + usize::from(code.as_bytes()[builder_start] == b'\n');
            let line_start = code[..name_from].rfind('\n').map_or(0, |p| p + 1);
            let builder_line = code[line_start..].lines().next().unwrap_or("");
            let builder_name = builder_line
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .filter(|w| !w.is_empty())
                .find(|w| !matches!(*w, "pub" | "fn" | "let" | "mut"))
                .unwrap_or("?");
            let builder_text = &code[builder_start..at];

            // Every stage's compilation options in this descriptor.
            let mut sites = Vec::new();
            let mut f = 0;
            while let Some(r) = desc[f..].find(OPTIONS_KEY) {
                sites.push(f + r);
                f = f + r + 1;
            }
            sites_seen += sites.len();
            assert!(!sites.is_empty(), "{builder_name}: a PSO descriptor with no compilation options?");
            // Whitespace-insensitive: the site must read
            // `wgpu::PipelineCompilationOptions { constants: &constants, ...`
            let registry_wired = |p: usize| -> bool {
                let tail: String = desc[p + OPTIONS_KEY.len()..]
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .take(REGISTRY_FORM.len())
                    .collect();
                tail == REGISTRY_FORM
            };
            let all_wired = sites.iter().all(|&p| registry_wired(p));
            let entry = fragment_entry(desc);

            if all_wired {
                // `&constants` must be the registry's map for THIS label,
                // not some other local, and the lookup must key on the PSO's
                // own label so the registry row and the PSO cannot disagree.
                assert!(
                    builder_text.contains("let constants = pso_constants(label);"),
                    "{builder_name}: wires `&constants` into its compilation options but never \
                     asks the registry for it (expected `let constants = pso_constants(label);` \
                     before the descriptor)"
                );
                assert!(
                    desc.contains("label: Some(label),"),
                    "{builder_name}: the registry lookup must key on the PSO's own label \
                     (`label: Some(label),`)"
                );
                // And the fragment ENTRY must be the registry's too (P3):
                // `let entry = pso_fragment_entry(label)` in the builder and
                // `entry_point: Some(entry)` in the descriptor, or no
                // fragment stage at all.
                assert!(
                    builder_text.contains("let entry = pso_fragment_entry(label)"),
                    "{builder_name}: asks the registry for its constants but not for its fragment \
                     entry (expected `let entry = pso_fragment_entry(label)` before the descriptor)"
                );
                match entry {
                    Some(Entry::Ident("entry")) | None => {}
                    Some(other) => panic!(
                        "{builder_name}: a registry PSO's fragment entry must be `Some(entry)` from \
                         pso_fragment_entry(label), found {other:?}"
                    ),
                }
                registry_builders.push(builder_name);
            } else {
                // Some stage compiles without the registry's map. Allowed
                // ONLY for an exempt fragment entry.
                match entry {
                    Some(Entry::Literal(name)) => assert!(
                        REGISTRY_EXEMPT_FRAGMENT_ENTRIES.contains(&name),
                        "{builder_name}: compiles the megashader for fragment entry {name:?} \
                         without asking the registry (pso_constants(label)); only these entries \
                         are exempt: {REGISTRY_EXEMPT_FRAGMENT_ENTRIES:?}"
                    ),
                    Some(Entry::Ident(param)) => assert_eq!(
                        builder_name, PARAMETRIC_EXEMPT_BUILDER,
                        "{builder_name}: compiles the megashader without the registry's map for \
                         a fragment entry held in a variable ({param}); only \
                         {PARAMETRIC_EXEMPT_BUILDER} may do that, and only for the exempt cloud \
                         entries"
                    ),
                    None => panic!(
                        "{builder_name}: compiles the megashader without asking the registry and \
                         has no fragment entry to be exempt by; wire it to pso_constants(label)"
                    ),
                }
            }
        }

        // Nothing slipped past the descriptor walk.
        assert!(descriptors >= 5, "expected at least the five known PSO descriptors, found {descriptors}");
        assert_eq!(
            sites_seen,
            code.matches(OPTIONS_KEY).count(),
            "a compilation_options site sits outside every PSO descriptor"
        );
        for known in ["build_patch_render", "build_patch_shadow", "make_pbr", "make_shadow"] {
            assert!(
                registry_builders.contains(&known),
                "{known} no longer asks the registry (or was renamed; update this list)"
            );
        }
        // Every registered label is built by some builder in the block.
        for row in PSO_REGISTRY {
            assert!(
                code.contains(&format!("\"{}\"", row.label)),
                "registry label {:?} is not built by any builder in impl Pipeline",
                row.label
            );
        }

        // The parametric exempt builder takes its entry from its callers, so
        // pin every `"fs_..."` literal in the block: since P3 a registry
        // entry is NEVER a literal here (the builders take theirs from
        // pso_fragment_entry), so every literal must be an exempt name, and
        // every exempt name must still be built.
        let mut fs_literals: Vec<(usize, &str)> = Vec::new();
        let mut f = 0;
        while let Some(r) = code[f..].find("\"fs_") {
            let p = f + r + 1;
            let e = p + code[p..].find('"').expect("an unterminated string literal?");
            fs_literals.push((p, &code[p..e]));
            f = e + 1;
        }
        for (p, lit) in &fs_literals {
            assert!(
                REGISTRY_EXEMPT_FRAGMENT_ENTRIES.contains(lit),
                "fragment entry {lit:?} (pipeline.rs line {}) is typed as a literal inside impl \
                 Pipeline; a registry PSO must take its entry from pso_fragment_entry(label), and \
                 only the exempt cloud entries may be literals",
                line_of(&me, start + *p)
            );
        }
        for name in REGISTRY_EXEMPT_FRAGMENT_ENTRIES {
            assert!(
                fs_literals.iter().any(|(_, l)| l == name),
                "exempt entry {name:?} is no longer built anywhere in impl Pipeline; \
                 remove it from the exempt list"
            );
        }
        // And the class entries the registry names are none of the exempt
        // ones (a class cannot be routed through the exempt builder).
        for class in ShaderClass::ALL {
            assert!(
                !REGISTRY_EXEMPT_FRAGMENT_ENTRIES.contains(&class.fragment_entry()),
                "{class:?}'s entry is in the exempt list"
            );
        }
    }
}
