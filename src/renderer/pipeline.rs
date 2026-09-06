//! Render pipeline management — creates and caches wgpu render pipelines.

use super::camera::CameraUniforms;
use super::mesh::Vertex;
use bytemuck::{Pod, Zeroable};

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

        let (
            render_pipeline,
            transparent_pipeline,
            overlay_pipeline,
            shadow_pipeline,
            shadow_pipeline_alpha,
            patch_render_pipeline,
            patch_shadow_pipeline,
        ) = Self::build_all_pipelines(
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

    /// Rebuild the four PSOs from a NEW shader module while keeping every
    /// bind group layout object intact (v0.924 megashader hot-reload): the
    /// layouts are what live bind groups reference, so swapping only the
    /// pipelines means nothing else in the renderer needs recreating.
    /// Costs a few seconds of PSO compile - trivial next to the 3+ minute
    /// rebuild-and-reboot it replaces.
    pub fn recreate_pipelines(
        &mut self,
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        shader: &wgpu::ShaderModule,
        batch_shader: &wgpu::ShaderModule,
    ) {
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
        let (render, transparent, overlay, shadow, shadow_alpha, patch_render, patch_shadow) =
            Self::build_all_pipelines(
                device,
                surface_format,
                shader,
                batch_shader,
                &pipeline_layout,
                &patch_pipeline_layout,
            );
        self.render_pipeline = render;
        self.transparent_pipeline = transparent;
        self.overlay_pipeline = overlay;
        self.shadow_pipeline = shadow;
        self.shadow_pipeline_alpha = shadow_alpha;
        self.patch_render_pipeline = patch_render;
        self.patch_shadow_pipeline = patch_shadow;
        self.cloud_light_bake_pipeline =
            Self::build_cloud_light_bake_pipeline(device, shader, &pipeline_layout);
        self.cloud_screen_pipeline =
            Self::build_cloud_screen_pipeline(device, shader, &pipeline_layout);
        // The four far-rung pipelines (increment 4) follow the same
        // hot-reload rule: new module, same layouts, live bind groups intact.
        self.cloud_profile_bake_pipeline = Self::build_cloud_profile_pipeline(
            device, shader, &pipeline_layout, "Cloud Profile Bake Pipeline", "fs_cloud_profile_bake",
        );
        self.cloud_profile_mip_pipeline = Self::build_cloud_profile_pipeline(
            device, shader, &pipeline_layout, "Cloud Profile Mip Pipeline", "fs_cloud_profile_mip",
        );
        self.cloud_profile_calib_pipeline = Self::build_cloud_profile_pipeline(
            device, shader, &pipeline_layout, "Cloud Profile Calib Pipeline", "fs_cloud_profile_calib",
        );
        self.cloud_profile_calib_reduce_pipeline = Self::build_cloud_profile_pipeline(
            device,
            shader,
            &pipeline_layout,
            "Cloud Profile Calib Reduce Pipeline",
            "fs_cloud_profile_calib_reduce",
        );
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

    /// The terrain-batch OPAQUE PSO, compiled from the BATCH shader module
    /// (different module + pipeline layout from the classic set). Single-PSO
    /// helper so `build_all_pipelines` can compile it on its own thread.
    fn build_patch_render(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        batch_shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
    ) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Patch Batch Render Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: batch_shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout(), Vertex::instance_layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: batch_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Patch Batch Shadow Pipeline"),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module: batch_shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout(), Vertex::instance_layout()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: batch_shader,
                entry_point: Some("fs_shadow"),
                targets: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
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

    /// ALL SEVEN PSO compiles shared by `new` and hot-reload's
    /// `recreate_pipelines`, in ONE thread scope (v0.1142). Measured
    /// 2026-08-15: `Pipeline::new` was 3.9 s of the 4.1 s
    /// shaders_and_pipelines boot span, because only the three PBR variants
    /// compiled in parallel (the 2026-07-12 scope) while the two sun-shadow
    /// and two terrain-patch PSOs compiled serially after them on the main
    /// thread. Each PSO bakes a full megashader fragment through Naga->DXIL,
    /// they are all independent, and `create_render_pipeline` takes `&self`
    /// on a Send+Sync Device, so all seven belong in the same scope: wall
    /// time falls toward the slowest single compile.
    fn build_all_pipelines(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        shader: &wgpu::ShaderModule,
        batch_shader: &wgpu::ShaderModule,
        pipeline_layout: &wgpu::PipelineLayout,
        patch_pipeline_layout: &wgpu::PipelineLayout,
    ) -> (
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
        wgpu::RenderPipeline,
    ) {
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
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(pipeline_layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex::layout(), Vertex::instance_layout()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
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
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(pipeline_layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex::layout(), Vertex::instance_layout()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: if cutout {
                    Some(wgpu::FragmentState {
                        module: shader,
                        entry_point: Some("fs_shadow"),
                        targets: &[],
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
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
        std::thread::scope(|s| {
            let opaque = s.spawn(|| {
                make_pbr(
                    "PBR-lite Render Pipeline",
                    wgpu::BlendState::REPLACE,
                    Some(wgpu::Face::Back),
                    true,
                )
            });
            let transparent = s.spawn(|| {
                make_pbr(
                    "PBR-lite Transparent Pipeline",
                    wgpu::BlendState::ALPHA_BLENDING,
                    None,
                    false,
                )
            });
            let overlay = s.spawn(|| {
                make_pbr(
                    "PBR-lite Overlay Pipeline",
                    wgpu::BlendState::ALPHA_BLENDING,
                    None,
                    true,
                )
            });
            let shadow = s.spawn(|| make_shadow("Sun Shadow Pipeline", false));
            let shadow_alpha = s.spawn(|| make_shadow("Sun Shadow Alpha Pipeline", true));
            let patch_render = s.spawn(|| {
                Self::build_patch_render(device, surface_format, batch_shader, patch_pipeline_layout)
            });
            // The seventh compiles on this thread while the six workers run.
            let patch_shadow =
                Self::build_patch_shadow(device, batch_shader, patch_pipeline_layout);
            (
                opaque.join().expect("opaque PBR pipeline compile panicked"),
                transparent
                    .join()
                    .expect("transparent PBR pipeline compile panicked"),
                overlay.join().expect("overlay PBR pipeline compile panicked"),
                shadow.join().expect("sun shadow pipeline compile panicked"),
                shadow_alpha
                    .join()
                    .expect("sun shadow alpha pipeline compile panicked"),
                patch_render
                    .join()
                    .expect("patch render pipeline compile panicked"),
                patch_shadow,
            )
        })
    }
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
