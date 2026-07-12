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
    /// x = metallic, y = roughness, z/w unused
    pub params: [f32; 4],
}

/// PBR-lite render pipeline with three bind group layouts.
pub struct Pipeline {
    pub render_pipeline: wgpu::RenderPipeline,
    /// Alpha-blended variant for transparent surfaces (glass windows, the portal). Same
    /// shader + layout, but blends over the scene and does NOT write depth, so you see
    /// THROUGH it. (v0.456)
    pub transparent_pipeline: wgpu::RenderPipeline,
    /// Editor-GIZMO variant (v0.560): alpha-blended, double-sided, and depth-test DISABLED
    /// (depth_compare Always) so build-mode gizmos (corner orbs, the avatar, rings) draw ON TOP of
    /// the world -- visible through walls + floors. No depth write either.
    pub overlay_pipeline: wgpu::RenderPipeline,
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub object_bind_group_layout: wgpu::BindGroupLayout,
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
    /// Create the PBR-lite pipeline from a shader module.
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        shader: &wgpu::ShaderModule,
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

        // Group 2: Material uniforms
        let material_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Material Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
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
                    // Cloud SHAPE volume: 128^3 RGBA8 tiling Perlin-Worley +
                    // Worley octaves (renderer::cloud_noise::generate_shape).
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
                    // Cloud DETAIL volume: 64^3 RGBA8 tiling Worley octaves
                    // (renderer::cloud_noise::generate_detail).
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
                ],
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
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex::layout()],
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
        let (render_pipeline, transparent_pipeline, overlay_pipeline) =
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
                // The third compiles on this thread while the two workers run.
                let overlay = make_pbr(
                    "PBR-lite Overlay Pipeline",
                    wgpu::BlendState::ALPHA_BLENDING,
                    None,
                    true,
                );
                (
                    opaque.join().expect("opaque PBR pipeline compile panicked"),
                    transparent
                        .join()
                        .expect("transparent PBR pipeline compile panicked"),
                    overlay,
                )
            });

        Self {
            render_pipeline,
            transparent_pipeline,
            overlay_pipeline,
            camera_bind_group_layout,
            object_bind_group_layout,
            material_bind_group_layout,
            texture_bind_group_layout,
        }
    }
}
