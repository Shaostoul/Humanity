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
pub struct Pipeline {
    pub render_pipeline: wgpu::RenderPipeline,
    /// Depth-only sun-shadow variant (v0.899): vs_main with no fragment,
    /// standard-z ortho depth into the 4096^2 shadow map. OPAQUE casters
    /// only - see `shadow_pipeline_alpha` and `shadow_for`.
    pub shadow_pipeline: wgpu::RenderPipeline,
    /// Alpha-cutout sun-shadow variant (v0.1106): identical state to
    /// `shadow_pipeline` plus the `fs_shadow` fragment stage, which mirrors
    /// fs_main's four cutout discards so a mostly-transparent caster stops
    /// casting a solid board. Kept SEPARATE rather than folded into
    /// `shadow_pipeline` because a fragment stage forfeits the depth-only
    /// double-rate rasterisation, and terrain meshes, ships, furniture and
    /// water are opaque and would pay it for nothing.
    pub shadow_pipeline_alpha: wgpu::RenderPipeline,
    /// Alpha-blended variant for transparent surfaces (glass windows, the portal). Same
    /// shader + layout, but blends over the scene and does NOT write depth, so you see
    /// THROUGH it. (v0.456)
    pub transparent_pipeline: wgpu::RenderPipeline,
    /// Editor-GIZMO variant (v0.560): alpha-blended, double-sided, and depth-test DISABLED
    /// (depth_compare Always) so build-mode gizmos (corner orbs, the avatar, rings) draw ON TOP of
    /// the world -- visible through walls + floors. No depth write either.
    pub overlay_pipeline: wgpu::RenderPipeline,
    /// The SHELL class's transparent PSO (increment P2 of the frame-cost arc):
    /// the same blend / cull / depth state as `transparent_pipeline`, compiled
    /// with the atmosphere and ocean branches kept and the cloud branch OFF.
    /// The atmosphere shell (type 14) and the water shell (type 16) draw
    /// through it. Never reach for these fields directly from a draw loop:
    /// `transparent_for(shader_class(...))` picks the PSO by the material's
    /// class, so a shell can never be drawn by a pipeline that folded its
    /// branch away.
    pub shell_transparent_pipeline: wgpu::RenderPipeline,
    /// The SHELL class's overlay PSO (P2): `overlay_pipeline`'s state (alpha
    /// blend, no cull, depth WRITE), which the water shell uses when
    /// `water_depth_write` is on (v0.1060), compiled like
    /// `shell_transparent_pipeline`. Selected through `overlay_for`.
    pub shell_overlay_pipeline: wgpu::RenderPipeline,
    /// The CLOUD class's transparent PSO (P2): `transparent_pipeline`'s state
    /// with ONLY the cloud branch kept. The cloud shell (type 15) is the one
    /// material whose fragments genuinely run the volumetric march, and it
    /// is the only pipeline whose fragments pay for the march's private
    /// tables: the whole point of the class split. Selected through
    /// `transparent_for`.
    pub cloud_transparent_pipeline: wgpu::RenderPipeline,
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
    /// PSOs whose fragment entry is `fs_main` or `fs_shadow`: the ten
    /// registered in `PSO_DEAD_BRANCHES` (five general, two shell, one
    /// cloud, two terrain-batch).
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
            render: render_pipeline,
            transparent: transparent_pipeline,
            overlay: overlay_pipeline,
            shell_transparent: shell_transparent_pipeline,
            shell_overlay: shell_overlay_pipeline,
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
            render_pipeline,
            shadow_pipeline,
            shadow_pipeline_alpha,
            transparent_pipeline,
            overlay_pipeline,
            shell_transparent_pipeline,
            shell_overlay_pipeline,
            cloud_transparent_pipeline,
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
            (&mut self.render_pipeline, fresh.render),
            (&mut self.transparent_pipeline, fresh.transparent),
            (&mut self.overlay_pipeline, fresh.overlay),
            (&mut self.shell_transparent_pipeline, fresh.shell_transparent),
            (&mut self.shell_overlay_pipeline, fresh.shell_overlay),
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
        // megashader module too, but through fs_cloud_* entries that never
        // enter fs_main, so they take no permutation: see the exemption
        // note above PSO_DEAD_BRANCHES.)
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

    /// The TRANSPARENT-state PSO (alpha blend, no cull, depth test, no depth
    /// write) a material of `class` must draw with (increment P2). One
    /// accessor so every transparent draw loop picks the same way: the
    /// class comes from `shader_class(material_type)`, never from a field
    /// name typed at the call site, so a shell cannot be handed a pipeline
    /// that compiled its branch away (the fragment would fall through to
    /// the default look, a white blended sphere where the sky should be).
    pub fn transparent_for(&self, class: ShaderClass) -> &wgpu::RenderPipeline {
        match class {
            ShaderClass::General => &self.transparent_pipeline,
            ShaderClass::Shell => &self.shell_transparent_pipeline,
            ShaderClass::Cloud => &self.cloud_transparent_pipeline,
        }
    }

    /// The OVERLAY-state PSO (alpha blend, no cull, depth WRITE) for a
    /// material of `class`. Two users: the editor gizmos (general) and the
    /// water shell when `water_depth_write` is on (v0.1060, a shell). There
    /// is deliberately no cloud overlay PSO: nothing routes the cloud shell
    /// through an overlay list, and a PSO nothing draws with would cost a
    /// full megashader compile at every boot and every hot reload for no
    /// pixel. A cloud-class object in an overlay list is a caller bug; it
    /// trips the debug assertion, and in release draws through the shell
    /// overlay PSO where the type-15 dispatch is folded away, so the shell
    /// renders as the default look (visibly wrong, never silently absent).
    pub fn overlay_for(&self, class: ShaderClass) -> &wgpu::RenderPipeline {
        match class {
            ShaderClass::General => &self.overlay_pipeline,
            ShaderClass::Shell => &self.shell_overlay_pipeline,
            ShaderClass::Cloud => {
                debug_assert!(
                    false,
                    "a cloud-class material (type 15) was routed to an overlay list; \
                     the cloud shell only ever draws through transparent_for(Cloud)"
                );
                &self.shell_overlay_pipeline
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
        // The permutation this PSO compiles: see PSO_DEAD_BRANCHES. Terrain
        // patches never draw an atmosphere, cloud or ocean shell, so those
        // three fs_main branches are switched OFF here and the cloud march's
        // per-invocation tables never reach this pipeline's DXIL.
        let label = "Patch Batch Render Pipeline";
        let constants = pso_constants(label);
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
                entry_point: Some("fs_main"),
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
        // Same permutation as the patch render PSO (PSO_DEAD_BRANCHES):
        // fs_shadow never reaches the three shells anyway, but the terrain
        // module is compiled with one consistent set of switches so the two
        // patch PSOs can never drift apart.
        let label = "Patch Batch Shadow Pipeline";
        let constants = pso_constants(label);
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
                entry_point: Some("fs_shadow"),
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

    /// ALL TEN PSO compiles shared by `new` and hot-reload's
    /// `recreate_pipelines`, in ONE thread scope (v0.1142). Measured
    /// 2026-08-15: `Pipeline::new` was 3.9 s of the 4.1 s
    /// shaders_and_pipelines boot span, because only the three PBR variants
    /// compiled in parallel (the 2026-07-12 scope) while the two sun-shadow
    /// and two terrain-patch PSOs compiled serially after them on the main
    /// thread. Each PSO bakes a full megashader fragment through Naga->DXIL,
    /// they are all independent, and `create_render_pipeline` takes `&self`
    /// on a Send+Sync Device, so all of them belong in the same scope: wall
    /// time falls toward the slowest single compile (since P2 that is the
    /// cloud transparent PSO, the only one that still bakes the march; the
    /// general PSOs fold all three shell branches away and compile faster
    /// than they did before the split).
    fn build_all_pipelines(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        shader: &wgpu::ShaderModule,
        batch_shader: &wgpu::ShaderModule,
        pipeline_layout: &wgpu::PipelineLayout,
        patch_pipeline_layout: &wgpu::PipelineLayout,
    ) -> MegashaderPsos {
        // ── Parallel PBR pipeline compile (boot-speed, 2026-07-12) ──
        // The three PBR variants (opaque / transparent glass / editor overlay)
        // each bake the WHOLE pbr_simple.wgsl fragment into a backend PSO, which
        // on this GPU takes ~10 s of Naga->DXIL work apiece -- the dominant cold-
        // boot cost (measured via debug/boot_timing.json). They are otherwise
        // independent: same shader module + pipeline layout, differing only in
        // blend / cull / depth-write. wgpu's `Device` is `Send + Sync` and
        // `create_render_pipeline` takes `&self`, so the three PSO compiles are
        // sound to run CONCURRENTLY, cutting ~3x10 s serial down toward the
        // slowest single compile. `std::thread::scope` lets the worker threads
        // borrow the shared `&device` / `&pipeline_layout` / `shader` without any
        // 'static bound.
        let make_pbr = |label: &'static str,
                        blend: wgpu::BlendState,
                        cull: Option<wgpu::Face>,
                        depth_write: bool|
         -> wgpu::RenderPipeline {
            // Which fs_main branches this PSO keeps is the registry's call
            // (PSO_DEAD_BRANCHES, keyed by label): the general PSOs fold all
            // three shells away, the shell PSOs keep the atmosphere and
            // ocean, the cloud PSO keeps only the march. Switches the map
            // does not name keep the shader's `= true` default.
            let constants = pso_constants(label);
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
                    entry_point: Some("fs_main"),
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
        let make_shadow = |label: &'static str, cutout: bool| -> wgpu::RenderPipeline {
            // Registered as GENERAL (all three switches off) in
            // PSO_DEAD_BRANCHES, and that is a no-op for these two: fs_shadow
            // never reaches the three fs_main shells and vs_main reads no
            // switch (the water shell's VERTEX displacement rides this module
            // untouched), so the constants change nothing the backend keeps.
            // They are named anyway so every megashader PSO is compiled with
            // an explicit permutation rather than an implicit default.
            let constants = pso_constants(label);
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
                fragment: if cutout {
                    Some(wgpu::FragmentState {
                        module: shader,
                        entry_point: Some("fs_shadow"),
                        targets: &[],
                        compilation_options: wgpu::PipelineCompilationOptions {
                            constants: &constants,
                            ..Default::default()
                        },
                    })
                } else {
                    None
                },
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
        // SAME state for a general and a shell PSO of one flavour, by
        // construction: only the label (and so the registry's constant map)
        // differs, which is what makes routing by class a pure perf change
        // with no blend / cull / depth consequence.
        //  - Transparent: alpha blend, double-sided, depth test, no write.
        //  - Overlay: alpha blend, double-sided, depth WRITE.
        let transparent_state = (wgpu::BlendState::ALPHA_BLENDING, None, false);
        let overlay_state = (wgpu::BlendState::ALPHA_BLENDING, None, true);
        let pbr = |label: &'static str, (blend, cull, depth_write): (wgpu::BlendState, Option<wgpu::Face>, bool)| {
            make_pbr(label, blend, cull, depth_write)
        };
        std::thread::scope(|s| {
            let render = s.spawn(|| {
                make_pbr(
                    "PBR-lite Render Pipeline",
                    wgpu::BlendState::REPLACE,
                    Some(wgpu::Face::Back),
                    true,
                )
            });
            let transparent = s.spawn(|| pbr("PBR-lite Transparent Pipeline", transparent_state));
            let overlay = s.spawn(|| pbr("PBR-lite Overlay Pipeline", overlay_state));
            // The class PSOs (P2): identical states, different registry rows.
            let shell_transparent =
                s.spawn(|| pbr("PBR-lite Shell Transparent Pipeline", transparent_state));
            let shell_overlay = s.spawn(|| pbr("PBR-lite Shell Overlay Pipeline", overlay_state));
            let cloud_transparent =
                s.spawn(|| pbr("PBR-lite Cloud Transparent Pipeline", transparent_state));
            let shadow = s.spawn(|| make_shadow("Sun Shadow Pipeline", false));
            let shadow_alpha = s.spawn(|| make_shadow("Sun Shadow Alpha Pipeline", true));
            let patch_render = s.spawn(|| {
                Self::build_patch_render(device, surface_format, batch_shader, patch_pipeline_layout)
            });
            // The tenth compiles on this thread while the nine workers run.
            let patch_shadow =
                Self::build_patch_shadow(device, batch_shader, patch_pipeline_layout);
            MegashaderPsos {
                render: render.join().expect("opaque PBR pipeline compile panicked"),
                transparent: transparent
                    .join()
                    .expect("transparent PBR pipeline compile panicked"),
                overlay: overlay.join().expect("overlay PBR pipeline compile panicked"),
                shell_transparent: shell_transparent
                    .join()
                    .expect("shell transparent PBR pipeline compile panicked"),
                shell_overlay: shell_overlay
                    .join()
                    .expect("shell overlay PBR pipeline compile panicked"),
                cloud_transparent: cloud_transparent
                    .join()
                    .expect("cloud transparent PBR pipeline compile panicked"),
                shadow: shadow.join().expect("sun shadow pipeline compile panicked"),
                shadow_alpha: shadow_alpha
                    .join()
                    .expect("sun shadow alpha pipeline compile panicked"),
                patch_render: patch_render
                    .join()
                    .expect("patch render pipeline compile panicked"),
                patch_shadow,
            }
        })
    }
}

/// Every PSO `build_all_pipelines` compiles from the megashader, by name.
/// A named struct rather than a ten-element tuple so `new` and
/// `recreate_pipelines` cannot pair a fresh PSO with the wrong slot: a tuple
/// of ten identical types would let the shell overlay land in the general
/// overlay's field with no error anywhere, and the water would then draw
/// through a pipeline whose ocean branch is folded away.
struct MegashaderPsos {
    render: wgpu::RenderPipeline,
    transparent: wgpu::RenderPipeline,
    overlay: wgpu::RenderPipeline,
    shell_transparent: wgpu::RenderPipeline,
    shell_overlay: wgpu::RenderPipeline,
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
// fs_main's material dispatch (90-fragment-main.wgsl) reaches three
// heavyweight early-return branches: the atmosphere integrator (type 14), the
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
// THIS TABLE IS THE REGISTRY, and its scope is exact: EVERY PSO WHOSE
// FRAGMENT ENTRY IS `fs_main` (or `fs_shadow`, which shares the module and
// the terrain switch set). A builder asks `pso_constants(label)` for its
// map; an unregistered label panics at boot, so a new PSO must declare its
// permutation here; and `permutation_tests` below pins the table against the
// shader source, so nobody can re-enable a branch on the terrain PSOs, or
// switch one off on a pipeline that draws through it, without a red test.
//
// EXEMPT, by fragment entry name: the six cloud fullscreen PSOs
// (`fs_cloud_screen`, `fs_cloud_light_bake`, `fs_cloud_profile_bake`,
// `fs_cloud_profile_mip`, `fs_cloud_profile_calib`,
// `fs_cloud_profile_calib_reduce`, all built through
// `build_cloud_fullscreen_pipeline`). They compile the same megashader
// module, but their entries never enter fs_main (no material dispatch, so
// no shell branch to switch off) and the cloud march is the program they
// exist to run, so a permutation would have nothing to remove. The test
// `every_megashader_pso_builder_asks_the_registry` lists them by name and
// fails on any other fragment entry compiled without the registry's map.

/// The three fs_main branch switches, by the exact `override` name the
/// shader declares (05-overrides.wgsl). A pipeline-constant key is the
/// identifier string when the declaration carries no `@id`.
pub const BRANCH_ATMOSPHERE: &str = "HAS_ATMOSPHERE_BRANCH";
pub const BRANCH_CLOUD: &str = "HAS_CLOUD_BRANCH";
pub const BRANCH_OCEAN: &str = "HAS_OCEAN_BRANCH";

/// Every switch the megashader declares, in declaration order. The
/// permutation test requires each one to exist with a `= true` default, so
/// every pipeline that does NOT name a switch keeps that branch compiled in.
pub const ALL_BRANCH_SWITCHES: &[&str] = &[BRANCH_ATMOSPHERE, BRANCH_CLOUD, BRANCH_OCEAN];

// ── MATERIAL CLASSES (increment P2 of the frame-cost arc) ──
//
// P1 proved the floor sits under EVERY PSO that compiles fs_main with the
// cloud branch reachable: the Sahara's atmosphere shell, drawn through the
// classic transparent PSO, fell from 10.44 to 0.19 ms the moment cloud_layer
// was unreachable, and the console room's interior walls were paying the
// same floor through the opaque PSO. So the material type space is cut into
// CLASSES, each class gets pipelines compiled with exactly the branches its
// materials dispatch to, and the draw loops pick the pipeline by the class
// of the material in hand (`shader_class`, `Pipeline::transparent_for`,
// `Pipeline::overlay_for`). The atmosphere shell no longer shares a program
// with the cloud march; an interior wall no longer shares one with anything.

/// Which family of fs_main programs a material draws with. The classes are
/// defined by the three GUARDED dispatches in 90-fragment-main.wgsl, and
/// nothing else: `shader_class` is pinned to the shader's own type bands by
/// `shader_class_is_pinned_to_the_guarded_dispatch_bands`, so the classifier
/// cannot drift from the WGSL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderClass {
    /// Everything that is not one of the three shells: the procedural
    /// surfaces 0..11, terrain 12 (which rides the patch pipelines anyway),
    /// the legacy Fresnel atmosphere 13, the gas giants 17 and 18, textured
    /// meshes 19, the vegetation family 20..23 and the screens 24. Its
    /// pipelines fold all three shell branches away.
    General,
    /// The atmosphere shell (type 14, `atmosphere_scattering`) and the ocean
    /// shell (type 16, `ocean_shell`). Its pipelines keep those two branches
    /// and fold the cloud march away, which is what re-prices the limb.
    Shell,
    /// The cloud shell (type 15, `cloud_layer`), alone: the one class whose
    /// fragments genuinely run the march, so the only pipeline that pays
    /// for the march's per-invocation tables.
    Cloud,
}

impl ShaderClass {
    /// The fs_main branch switches this class's pipelines keep ON. Every
    /// switch appears in exactly one class's live set (pinned by the
    /// classifier test), so no two classes compile the same shell program.
    pub const fn live_branches(self) -> &'static [&'static str] {
        match self {
            ShaderClass::General => &[],
            ShaderClass::Shell => &[BRANCH_ATMOSPHERE, BRANCH_OCEAN],
            ShaderClass::Cloud => &[BRANCH_CLOUD],
        }
    }

    /// The complement of `live_branches` over `ALL_BRANCH_SWITCHES`: what
    /// `pso_constants` switches OFF for a pipeline of this class. Spelled
    /// out per class (rather than computed) so it can be a `const` the
    /// registry table below reads at compile time.
    pub const fn dead_branches(self) -> &'static [&'static str] {
        match self {
            ShaderClass::General => ALL_BRANCH_SWITCHES,
            ShaderClass::Shell => &[BRANCH_CLOUD],
            ShaderClass::Cloud => &[BRANCH_ATMOSPHERE, BRANCH_OCEAN],
        }
    }
}

/// The class of a material, from its `material.params.z` type value, using
/// the SAME half-open bands fs_main dispatches on (`>= 13.5 && < 14.5` is
/// type 14, and so on). Pure, so a draw loop can call it per object without
/// touching the renderer, and so the test can sweep the whole type space.
pub fn shader_class(material_type: f32) -> ShaderClass {
    let t = material_type;
    if t >= 14.5 && t < 15.5 {
        ShaderClass::Cloud
    } else if (t >= 13.5 && t < 14.5) || (t >= 15.5 && t < 16.5) {
        ShaderClass::Shell
    } else {
        ShaderClass::General
    }
}

/// (PSO label, the fs_main branches compiled OUT of it), each row written
/// as the class the PSO serves so the table reads as the routing it is.
/// The ten labels are exactly the PSOs `build_all_pipelines` creates from
/// the megashader with an fs_main / fs_shadow fragment entry; the six cloud
/// fullscreen PSOs are exempt (see the banner above) and are deliberately
/// absent.
///
/// Row by row:
/// - The three general colour PSOs draw every general material: opaque
///   props, walls, planet bodies and near trees through the render PSO;
///   glass, holograms, particles and the sun's blended core through the
///   transparent PSO; the editor gizmos through the overlay PSO. None of
///   them can draw a shell any more (the draw loops route by class), so all
///   three shell branches are folded away. The opaque list is asserted
///   shell-free in debug builds at every opaque draw site.
/// - The two shell PSOs draw the atmosphere and water shells (transparent
///   state for both; overlay state for the water when it writes depth) with
///   the cloud march folded away.
/// - The cloud transparent PSO draws the cloud shell and nothing else.
/// - The two sun-shadow PSOs are registered general as a no-op: fs_shadow
///   never reaches the shells and vs_main reads no switch, so the constants
///   change nothing the backend keeps (the water's vertex displacement is
///   untouched). Named so every megashader PSO carries an explicit row.
/// - The two terrain-batch PSOs draw planet-surface patches only (types 12
///   and 13 plus the sprite cards baked into the same meshes), never a
///   shell: all three off, the P1 permutation.
pub const PSO_DEAD_BRANCHES: &[(&str, &[&str])] = &[
    ("PBR-lite Render Pipeline", ShaderClass::General.dead_branches()),
    ("PBR-lite Transparent Pipeline", ShaderClass::General.dead_branches()),
    ("PBR-lite Overlay Pipeline", ShaderClass::General.dead_branches()),
    ("PBR-lite Shell Transparent Pipeline", ShaderClass::Shell.dead_branches()),
    ("PBR-lite Shell Overlay Pipeline", ShaderClass::Shell.dead_branches()),
    ("PBR-lite Cloud Transparent Pipeline", ShaderClass::Cloud.dead_branches()),
    ("Sun Shadow Pipeline", ShaderClass::General.dead_branches()),
    ("Sun Shadow Alpha Pipeline", ShaderClass::General.dead_branches()),
    ("Patch Batch Render Pipeline", ShaderClass::General.dead_branches()),
    ("Patch Batch Shadow Pipeline", ShaderClass::General.dead_branches()),
];

/// The pipeline-constant map for one registered PSO: each dead branch's
/// switch set to 0.0 (wgpu carries every override value as an f64; naga maps
/// it onto a `bool` override as `value != 0.0`). Switches not named in the
/// map keep the shader's `= true` default. Panics on a label that is not in
/// `PSO_DEAD_BRANCHES`: a pipeline built from the megashader without a
/// declared permutation is exactly the silent regression this registry
/// exists to prevent, and the panic lands at first boot, where the
/// boot-verify rig catches it.
pub fn pso_constants(label: &str) -> HashMap<String, f64> {
    let (_, dead) = PSO_DEAD_BRANCHES
        .iter()
        .find(|(l, _)| *l == label)
        .unwrap_or_else(|| {
            panic!(
                "PSO {label:?} is not in PSO_DEAD_BRANCHES (src/renderer/pipeline.rs): \
                 declare which fs_main branches it compiles out (an empty list keeps \
                 every branch)"
            )
        });
    dead.iter().map(|name| ((*name).to_string(), 0.0)).collect()
}

/// The fs_main <-> fs_shadow cutout mirror (v0.1106).
///
/// A shader discard cannot be unit-tested from the CPU, so what IS testable
/// is that the two functions agree. Every check below reads the ONE assembled
/// megashader source, splits it at `fn fs_shadow`, and requires the same
/// literal on both sides - so changing a cutout threshold in fs_main and
/// forgetting the shadow twin fails here instead of silently restoring the
/// solid-board shadows this pass removed. It also pins the DELIBERATE
/// omission (the eye-distance card discards must NOT appear in fs_shadow).
#[cfg(test)]
mod shadow_cutout_tests {
    use super::super::shader_loader::assembled_pbr_source;

    /// (fs_main side, fs_shadow side) of the assembled source.
    fn halves() -> (&'static str, &'static str) {
        let src = assembled_pbr_source();
        let at = src
            .find("fn fs_shadow")
            .expect("fs_shadow entry point missing from the megashader");
        (&src[..at], &src[at..])
    }

    #[test]
    fn fs_shadow_mirrors_the_fs_main_cutouts() {
        let (main, shadow) = halves();
        // The four cutouts, each identified by the exact expression both
        // functions must contain. Order matches the fs_shadow comment block.
        let mirrored = [
            // 1. LOD crossfade dither.
            "let b = (f32(bayer_i) + 0.5) / 16.0;",
            "if (b >= lod_fade) { discard; }",
            "if (b < -lod_fade) { discard; }",
            // 2. Type 19 photoscanned foliage.
            "material_type >= 18.5 && material_type < 19.5",
            "if (mesh_tex.a < 0.35) {",
            // 3. Type 21 baked cluster card.
            "material_type >= 20.5 && material_type < 21.5",
            "if (cc_tex.a < 0.5) {",
            // 4. Type 12 sprite tree card (6x8 atlas grid + resident bit).
            "let pw_bits_card = u32(round(max(material.params.w, 0.0)));",
            "if ((pw_bits_card & 4u) != 0u) {",
            "(f32(tile % 6u) + u01) / 6.0,",
            "(f32(tile / 6u) + (1.0 - v01)) / 8.0,",
            "if (spr.a < 0.5) {",
        ];
        for needle in mirrored {
            assert!(
                main.contains(needle),
                "fs_main no longer contains {needle:?} - if the cutout moved or \
                 changed, update fs_shadow and this list in the SAME edit"
            );
            assert!(
                shadow.contains(needle),
                "fs_shadow is missing the fs_main cutout {needle:?} - a \
                 mostly-transparent caster will cast a solid board again"
            );
        }
    }

    #[test]
    fn fs_shadow_omits_the_eye_distance_card_discards() {
        let (main, shadow) = halves();
        let eye_gate = "card_dist < shadow_u.params.w";
        assert!(main.contains(eye_gate), "fs_main lost its card LOD window");
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
            .find("fn fs_shadow(in: VertexOutput) {")
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

/// The shader-permutation registry against the shader source (P1).
///
/// Three things must agree or a terrain fragment silently pays for the cloud
/// march again: the `override` declarations in 05-overrides.wgsl, the guards
/// on fs_main's three shell dispatches, and PSO_DEAD_BRANCHES. None of that
/// is visible to the GPU-less test runner, so every check reads the ONE
/// assembled source (and this file's own text for the builder wiring), the
/// same way the cutout-mirror tests above do.
#[cfg(test)]
mod permutation_tests {
    use super::super::shader_loader::{assembled_pbr_batch_source, assembled_pbr_source};
    use super::*;

    /// The fs_main side of the assembled source (everything before fs_shadow).
    fn fs_main_side() -> &'static str {
        let src = assembled_pbr_source();
        let at = src.find("fn fs_shadow").expect("fs_shadow missing");
        &src[..at]
    }

    /// `src` with every `//` comment blanked to spaces, BYTE for byte, so an
    /// offset into the result is the same offset into the original and a
    /// function named in prose can never be mistaken for a call to it. (WGSL
    /// and this file's builders have no string literals containing `//`.)
    fn code_only(src: &str) -> String {
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

    /// 1-based line number of byte offset `at` in `text`, for messages.
    fn line_of(text: &str, at: usize) -> usize {
        text[..at].matches('\n').count() + 1
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

    #[test]
    fn fs_main_guards_each_shell_dispatch_with_its_switch() {
        // Comment-blanked, so a function named in prose is not a use of it,
        // and so the offsets below are offsets into the real source.
        let main = code_only(fs_main_side());
        // (switch, the dispatch condition it guards, the function it makes
        // dead). The switch is written FIRST in the `&&` by convention, the
        // one shape this test pins so every guard reads the same and can be
        // found here; the operand order changes nothing about how a false
        // constant folds the condition.
        let sites = [
            (BRANCH_ATMOSPHERE, "material_type >= 13.5 && material_type < 14.5", "atmosphere_scattering"),
            (BRANCH_CLOUD, "material_type >= 14.5 && material_type < 15.5", "cloud_layer"),
            (BRANCH_OCEAN, "material_type >= 15.5 && material_type < 16.5", "ocean_shell"),
        ];
        for (switch, cond, func) in sites {
            let guard = format!("if ({switch} && {cond}) {{");
            let at = main.find(&guard).unwrap_or_else(|| {
                panic!("fs_main no longer guards the shell dispatch with {guard:?}")
            });
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
            // else in fs_main (or in a helper fs_main calls) would make the
            // branch reachable regardless of the switch.
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
                 dispatch), found {}:\n  {}",
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

    /// The expected routing, PSO by PSO (P2). A hand-written twin of the
    /// registry so a row cannot be changed without this test naming the
    /// change: the general five and the terrain two fold every shell away,
    /// the shell two keep the atmosphere and ocean, the cloud one keeps the
    /// march alone.
    const EXPECTED_PSO_CLASSES: &[(&str, ShaderClass)] = &[
        ("PBR-lite Render Pipeline", ShaderClass::General),
        ("PBR-lite Transparent Pipeline", ShaderClass::General),
        ("PBR-lite Overlay Pipeline", ShaderClass::General),
        ("PBR-lite Shell Transparent Pipeline", ShaderClass::Shell),
        ("PBR-lite Shell Overlay Pipeline", ShaderClass::Shell),
        ("PBR-lite Cloud Transparent Pipeline", ShaderClass::Cloud),
        ("Sun Shadow Pipeline", ShaderClass::General),
        ("Sun Shadow Alpha Pipeline", ShaderClass::General),
        ("Patch Batch Render Pipeline", ShaderClass::General),
        ("Patch Batch Shadow Pipeline", ShaderClass::General),
    ];

    #[test]
    fn the_registry_covers_exactly_the_ten_megashader_psos() {
        let labels: Vec<&str> = PSO_DEAD_BRANCHES.iter().map(|(l, _)| *l).collect();
        let expected: Vec<&str> = EXPECTED_PSO_CLASSES.iter().map(|(l, _)| *l).collect();
        assert_eq!(labels, expected, "the registry's rows (or their order) changed");
        for ((label, dead), (_, class)) in PSO_DEAD_BRANCHES.iter().zip(EXPECTED_PSO_CLASSES) {
            assert_eq!(
                *dead,
                class.dead_branches(),
                "{label}: must compile out exactly the {class:?} class's dead branches"
            );
            // The dead and live sets of a class partition the switch list:
            // nothing is both, nothing is neither.
            for name in ALL_BRANCH_SWITCHES {
                let live = class.live_branches().contains(name);
                let is_dead = dead.contains(name);
                assert!(
                    live != is_dead,
                    "{label}: {name} must be exactly one of live / dead for {class:?} \
                     (live = {live}, dead = {is_dead})"
                );
            }
            // Every dead name is a declared switch, and the map carries 0.0
            // (naga's bool mapping: value != 0.0) for exactly those names.
            let map = pso_constants(label);
            assert_eq!(map.len(), dead.len(), "{label}: one constant per dead branch");
            for name in *dead {
                assert!(ALL_BRANCH_SWITCHES.contains(name), "{label}: {name} is not a declared switch");
                assert_eq!(map.get(*name), Some(&0.0), "{label}: {name} must be switched OFF (0.0)");
            }
        }
        // The general colour PSOs and the terrain PSOs carry the SAME
        // permutation (all off): a general fragment never enters a shell.
        for label in [
            "PBR-lite Render Pipeline",
            "PBR-lite Transparent Pipeline",
            "PBR-lite Overlay Pipeline",
            "Patch Batch Render Pipeline",
            "Patch Batch Shadow Pipeline",
        ] {
            assert_eq!(pso_constants(label).len(), ALL_BRANCH_SWITCHES.len(), "{label}: all three off");
        }
    }

    /// Every `if (HAS_X && material_type >= a && material_type < b)` guard
    /// in fs_main, as (switch, a, b), read out of the SHADER so the test
    /// cannot agree with a stale copy of the bands.
    fn guarded_dispatch_bands() -> Vec<(&'static str, f32, f32)> {
        let main = code_only(fs_main_side());
        let mut out = Vec::new();
        for switch in ALL_BRANCH_SWITCHES {
            let prefix = format!("if ({switch} && material_type >= ");
            let at = main
                .find(&prefix)
                .unwrap_or_else(|| panic!("fs_main has no guard reading {prefix:?}"));
            let rest = &main[at + prefix.len()..];
            let (lo, rest) = rest
                .split_once(" && material_type < ")
                .expect("a guard's band is `>= a && material_type < b`");
            let hi: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            out.push((*switch, lo.trim().parse().expect("band low"), hi.parse().expect("band high")));
        }
        out
    }

    /// THE CLASSIFIER IS PINNED TO THE WGSL (P2). `shader_class` must send
    /// every type inside a guarded dispatch band to a class whose pipelines
    /// keep that band's switch, and every other type in the shader's type
    /// space (0 to 24, swept at half steps so both band edges are exercised)
    /// to General. Proven red once by widening the ocean band's class to
    /// General for `t >= 16.0` (the test named type 16.0 and the ocean
    /// switch), then restored.
    #[test]
    fn shader_class_is_pinned_to_the_guarded_dispatch_bands() {
        let bands = guarded_dispatch_bands();
        assert_eq!(bands.len(), ALL_BRANCH_SWITCHES.len(), "one guarded band per switch");
        // The three bands must not overlap, or a type would have two
        // classes and the loops' "switch on class change" would thrash.
        for (i, (sa, la, ha)) in bands.iter().enumerate() {
            for (sb, lb, hb) in bands.iter().skip(i + 1) {
                assert!(ha <= lb || hb <= la, "guard bands overlap: {sa} [{la}, {ha}) and {sb} [{lb}, {hb})");
            }
        }
        // Sweep the type space. 0.0, 0.5, 1.0 ... 24.5: the whole-number
        // points are the shipped types, the half points are the band edges
        // (13.5 is IN the atmosphere band, 14.5 in the cloud band, ...).
        let mut t = 0.0_f32;
        while t <= 24.5 {
            let class = shader_class(t);
            let in_band = bands.iter().find(|(_, lo, hi)| t >= *lo && t < *hi);
            match in_band {
                Some((switch, lo, hi)) => {
                    assert!(
                        class.live_branches().contains(switch),
                        "material type {t} is inside the {switch} band [{lo}, {hi}) but \
                         shader_class sends it to {class:?}, whose pipelines compile that \
                         branch OUT; it would render as the default look"
                    );
                    assert_ne!(class, ShaderClass::General, "type {t}: a guarded band is never General");
                }
                None => assert_eq!(
                    class,
                    ShaderClass::General,
                    "material type {t} is outside every guarded band but shader_class \
                     sends it to {class:?}; it would pay for a shell program it never runs"
                ),
            }
            t += 0.5;
        }
        // Each switch is live in exactly ONE class, and the cloud switch's
        // class keeps nothing else: the march's per-invocation frame is
        // charged to cloud-shell fragments and to no other material. That
        // is the number P2 exists to move.
        let classes = [ShaderClass::General, ShaderClass::Shell, ShaderClass::Cloud];
        for switch in ALL_BRANCH_SWITCHES {
            let owners: Vec<ShaderClass> =
                classes.iter().copied().filter(|c| c.live_branches().contains(switch)).collect();
            assert_eq!(owners.len(), 1, "{switch} must be live in exactly one class, found {owners:?}");
        }
        assert_eq!(ShaderClass::Cloud.live_branches(), &[BRANCH_CLOUD], "the cloud class keeps only the march");
        assert!(ShaderClass::General.live_branches().is_empty(), "general keeps no shell branch");
        // And the shipped shell types land where the doc table says.
        assert_eq!(shader_class(14.0), ShaderClass::Shell, "the atmosphere shell");
        assert_eq!(shader_class(15.0), ShaderClass::Cloud, "the cloud shell");
        assert_eq!(shader_class(16.0), ShaderClass::Shell, "the ocean shell");
        assert_eq!(shader_class(13.0), ShaderClass::General, "the legacy Fresnel atmosphere");
        assert_eq!(shader_class(super::super::materials::MATERIAL_TYPE_SCREEN), ShaderClass::General);
    }

    #[test]
    #[should_panic(expected = "not in PSO_DEAD_BRANCHES")]
    fn pso_constants_panics_on_an_unregistered_label() {
        let _ = pso_constants("Some Future Pipeline");
    }

    /// Fragment entry points allowed to compile the megashader WITHOUT the
    /// registry's map, by name. These are the six cloud fullscreen passes:
    /// their entries never enter fs_main (no material dispatch, so there is
    /// no shell branch to switch off), and the cloud march is the program
    /// they exist to run, so a permutation would have nothing to remove.
    /// Every other fragment entry compiled from the module must be wired to
    /// `pso_constants(label)`. Add a name here ONLY for an entry that is not
    /// fs_main and cannot reach it; the banner above PSO_DEAD_BRANCHES lists
    /// the same six.
    const REGISTRY_EXEMPT_FRAGMENT_ENTRIES: &[&str] = &[
        "fs_cloud_screen",
        "fs_cloud_light_bake",
        "fs_cloud_profile_bake",
        "fs_cloud_profile_mip",
        "fs_cloud_profile_calib",
        "fs_cloud_profile_calib_reduce",
    ];

    /// The one builder whose fragment entry is a PARAMETER rather than a
    /// literal: `build_cloud_fullscreen_pipeline`, which every cloud PSO goes
    /// through. Its callers supply the entry, so the test checks every
    /// `"fs_..."` literal in the impl block against the exempt list and
    /// forbids `"fs_main"` / `"fs_shadow"` from ever being passed as an
    /// argument (they may appear only as a direct `entry_point: Some(...)`).
    const PARAMETRIC_EXEMPT_BUILDER: &str = "build_cloud_fullscreen_pipeline";

    /// The fragment entry a PSO descriptor names.
    #[derive(Debug)]
    enum Entry<'a> {
        /// `entry_point: Some("fs_main")`.
        Literal(&'a str),
        /// `entry_point: Some(fs_entry)`: a variable the builder was given.
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

    /// Byte offset of the `}` that closes the `{` at `open`.
    fn matching_brace(code: &str, open: usize) -> usize {
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

    /// The builders must actually ASK the registry, and this test must be
    /// able to FAIL for the likeliest regression: a new builder that
    /// compiles the megashader with default options. An adversarial review
    /// of the first version showed it could not (it asserted one spelling
    /// of "default", it compared counts a non-asking builder does not
    /// perturb, and its scan started after the cloud builders). So this
    /// version reads the WHOLE `impl Pipeline` block, finds every
    /// `create_render_pipeline` descriptor, and judges each on its own:
    /// every one of its `compilation_options:` sites is wired to
    /// `&constants` from `pso_constants(label)`, OR its fragment entry is
    /// one of the exempt cloud entries above. Proven red by adding a fifth
    /// builder spelled `compilation_options: Default::default()` with an
    /// fs_main fragment entry (the reviewer's mutation), then restored.
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
        for (label, _) in PSO_DEAD_BRANCHES {
            assert!(
                code.contains(&format!("\"{label}\"")),
                "registry label {label:?} is not built by any builder in impl Pipeline"
            );
        }

        // The parametric exempt builder takes its entry from its callers, so
        // pin every `"fs_..."` literal in the block: each is fs_main / fs_shadow
        // (registry) or an exempt name; fs_main / fs_shadow appear ONLY as a
        // direct `entry_point: Some("...")`, never as an argument that could
        // reach the exempt builder; and every exempt name is still built.
        let mut fs_literals: Vec<(usize, &str)> = Vec::new();
        let mut f = 0;
        while let Some(r) = code[f..].find("\"fs_") {
            let p = f + r + 1;
            let e = p + code[p..].find('"').expect("an unterminated string literal?");
            fs_literals.push((p, &code[p..e]));
            f = e + 1;
        }
        for (p, lit) in &fs_literals {
            if *lit == "fs_main" || *lit == "fs_shadow" {
                assert!(
                    code[..*p - 1].ends_with("entry_point: Some("),
                    "{lit:?} (pipeline.rs line {}) may only appear as a direct \
                     `entry_point: Some(\"{lit}\")`; passing it to a builder as an argument \
                     would route it around the registry",
                    line_of(&me, start + *p)
                );
            } else {
                assert!(
                    REGISTRY_EXEMPT_FRAGMENT_ENTRIES.contains(lit),
                    "fragment entry {lit:?} (pipeline.rs line {}) compiles the megashader but is \
                     neither registered (fs_main / fs_shadow through pso_constants) nor in the \
                     exempt list",
                    line_of(&me, start + *p)
                );
            }
        }
        for name in REGISTRY_EXEMPT_FRAGMENT_ENTRIES {
            assert!(
                fs_literals.iter().any(|(_, l)| l == name),
                "exempt entry {name:?} is no longer built anywhere in impl Pipeline; \
                 remove it from the exempt list"
            );
        }
    }
}
