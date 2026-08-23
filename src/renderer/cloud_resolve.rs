//! Cloud temporal RESOLVE pass (12e, the march/resolve split).
//!
//! Blends the quarter-res per-frame cloud march into the half-res
//! accumulation pair with deep accumulation + variance-clipped history.
//! This is what ended the 12d trade-off the operator hit on the first
//! v0.1198 flight: the single-pass 0.25-0.6 blend was too shallow to
//! converge the jittered march ("clouds look a lot like static") yet
//! deep enough that stale content FADED for half a second instead of
//! dying (the residual ghosting). With the clip, history the current
//! frame cannot corroborate is snapped to the neighbourhood's plausible
//! range in ONE frame while corroborated content accumulates ~1/alpha
//! frames deep - ghost death and convergence stop competing.
//!
//! Shader: assets/shaders/cloud_resolve.wgsl (self-contained - the
//! reprojection basis arrives through this pass's own uniform, not the
//! megashader camera pads).

use wgpu::util::DeviceExt;

/// Per-frame camera state the resolve needs, stashed by the pad-poke
/// site in render_celestial_onto (the same place the octa reprojection
/// delta is written, so both consumers see one consistent motion).
#[derive(Clone, Copy, Debug, Default)]
pub struct CloudResolveFrame {
    /// prev_camera_pos - current_camera_pos, render frame (the same
    /// planet-local-derived delta the octa pads carry).
    pub prev_dpos: [f32; 3],
    /// Previous frame's camera basis (forward, right, up).
    pub prev_basis: [[f32; 3]; 3],
    /// Drop history entirely this frame (teleport / regime entry).
    pub snap: bool,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CloudResolveUniforms {
    cam_pos: [f32; 4],
    cam_fwd: [f32; 4],
    cam_right: [f32; 4],
    cam_up: [f32; 4],
    prev_dpos: [f32; 4],
    prev_fwd: [f32; 4],
    prev_right: [f32; 4],
    prev_up: [f32; 4],
}

/// Base accumulation alpha: ~8-frame effective depth. The variance clip
/// is what makes this depth safe (unclipped, it would be a half-second
/// ghost trail - exactly the 12d failure).
const RESOLVE_ALPHA_BASE: f32 = 0.12;
/// Variance-clip gamma (box half-width in sigmas). 1.0 = tight Karis
/// clipping; raise toward 1.5 if converged detail visibly flickers.
const RESOLVE_CLIP_GAMMA: f32 = 1.0;

pub struct CloudResolvePass {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    param_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
}

impl CloudResolvePass {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Cloud Resolve Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/cloud_resolve.wgsl").into(),
            ),
        });

        let tex_entry = |binding: u32, filterable: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Cloud Resolve BGL"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    tex_entry(1, true),  // march color (bilinear upsample)
                    tex_entry(2, false), // march dist (textureLoad only)
                    tex_entry(3, true),  // history (bilinear reproject)
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let param_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cloud Resolve Params"),
            contents: bytemuck::bytes_of(&CloudResolveUniforms {
                cam_pos: [0.0; 4],
                cam_fwd: [0.0; 4],
                cam_right: [0.0; 4],
                cam_up: [0.0; 4],
                prev_dpos: [0.0; 4],
                prev_fwd: [0.0; 4],
                prev_right: [0.0; 4],
                prev_up: [0.0; 4],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Cloud Resolve Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Cloud Resolve Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Cloud Resolve Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba16Float,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self { pipeline, bind_group_layout, param_buffer, sampler }
    }

    /// Resolve the current march into the accumulation buffer.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        march_color: &wgpu::TextureView,
        march_dist: &wgpu::TextureView,
        history: &wgpu::TextureView,
        target: &wgpu::TextureView,
        frame: &CloudResolveFrame,
        cam_pos: [f32; 3],
        cam_fwd: [f32; 3],
        cam_right: [f32; 3],
        cam_up: [f32; 3],
        tan_half_fov: f32,
        aspect: f32,
        timestamp_writes: Option<wgpu::RenderPassTimestampWrites>,
    ) {
        let b = frame.prev_basis;
        let u = CloudResolveUniforms {
            cam_pos: [cam_pos[0], cam_pos[1], cam_pos[2], tan_half_fov],
            cam_fwd: [cam_fwd[0], cam_fwd[1], cam_fwd[2], aspect],
            cam_right: [cam_right[0], cam_right[1], cam_right[2], RESOLVE_ALPHA_BASE],
            cam_up: [
                cam_up[0],
                cam_up[1],
                cam_up[2],
                if frame.snap { 1.0 } else { 0.0 },
            ],
            prev_dpos: [
                frame.prev_dpos[0],
                frame.prev_dpos[1],
                frame.prev_dpos[2],
                RESOLVE_CLIP_GAMMA,
            ],
            prev_fwd: [b[0][0], b[0][1], b[0][2], 0.0],
            prev_right: [b[1][0], b[1][1], b[1][2], 0.0],
            prev_up: [b[2][0], b[2][1], b[2][2], 0.0],
        };
        queue.write_buffer(&self.param_buffer, 0, bytemuck::bytes_of(&u));
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Cloud Resolve BG"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.param_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(march_color),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(march_dist),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(history),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Cloud Resolve Pass"),
            timestamp_writes,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}
