//! Fullscreen depth-aware cloud composite (Wave D slice 1b, increment 12).
//!
//! Cures THE VANISHING DECK: with the camera inside the cloud shell, a
//! downward ray's only shell geometry lies beyond the planet, and the
//! hardware depth test killed those fragments behind the terrain - every
//! cloud below the camera vanished crossing ~16 km (descent-ladder
//! measurement, v0.1172). This pass composites the temporal cloud map
//! over the finished celestial output with its OWN per-pixel occlusion
//! against the scene depth, so terrain occludes exactly the cloud that is
//! actually behind it - mountains included, which the analytic
//! planet-sphere test alone could never do.
//!
//! Runs only while the temporal map is armed (the same regime where the
//! deck used to vanish); the shell fragment path keeps the direct-march
//! case above the arming altitude, where camera-outside geometry makes
//! the hardware depth test sound. The shell's own temporal-composite
//! branch DISCARDS while this pass is active - one compositor at a time.

use wgpu::util::DeviceExt;

/// Everything the composite needs about the cloud shell's frame, stashed
/// by lib.rs at the cloud material fill site each frame.
#[derive(Clone, Copy, Debug)]
pub struct CloudCompositeFrame {
    /// Planet centre in the render frame.
    pub center: [f32; 3],
    /// Planet radius in world units (visual_scale).
    pub planet_r: f32,
    /// Planet local axes in world space (the shell rotation's columns).
    pub basis: [[f32; 3]; 3],
    /// Slab bounds as planet-radius ratios.
    pub rb: f32,
    pub rt: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct CloudCompositeUniforms {
    cam_pos: [f32; 4],
    cam_fwd: [f32; 4],
    cam_right: [f32; 4],
    cam_up: [f32; 4],
    center: [f32; 4],
    basis_x: [f32; 4],
    basis_y: [f32; 4],
    basis_z: [f32; 4],
    proj: [f32; 4],
}

pub struct CloudCompositePass {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    param_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
}

impl CloudCompositePass {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Cloud Composite Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../assets/shaders/cloud_composite.wgsl").into(),
            ),
        });

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Cloud Composite BGL"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let param_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cloud Composite Params"),
            contents: bytemuck::bytes_of(&CloudCompositeUniforms {
                cam_pos: [0.0; 4],
                cam_fwd: [0.0; 4],
                cam_right: [0.0; 4],
                cam_up: [0.0; 4],
                center: [0.0; 4],
                basis_x: [0.0; 4],
                basis_y: [0.0; 4],
                basis_z: [0.0; 4],
                proj: [0.0; 4],
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Cloud Composite Map Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Cloud Composite Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Cloud Composite Pipeline"),
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
                    format: surface_format,
                    // Premultiplied-over: the map stores premultiplied rgb.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            // Reads depth as a texture; no depth attachment.
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self { pipeline, bind_group_layout, param_buffer, sampler }
    }

    /// Composite the cloud map onto `view`. `m22`/`m32` are the reverse-Z
    /// projection elements the depth buffer was rendered with (the SSAO
    /// convention). `cam_*` is the camera basis in the render frame;
    /// `tan_half_fov`/`aspect` reconstruct the pixel rays.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
        map_view: &wgpu::TextureView,
        view: &wgpu::TextureView,
        frame: &CloudCompositeFrame,
        cam_pos: [f32; 3],
        cam_fwd: [f32; 3],
        cam_right: [f32; 3],
        cam_up: [f32; 3],
        tan_half_fov: f32,
        aspect: f32,
        m22: f32,
        m32: f32,
        timestamp_writes: Option<wgpu::RenderPassTimestampWrites>,
    ) {
        let u = CloudCompositeUniforms {
            cam_pos: [cam_pos[0], cam_pos[1], cam_pos[2], tan_half_fov],
            cam_fwd: [cam_fwd[0], cam_fwd[1], cam_fwd[2], aspect],
            cam_right: [cam_right[0], cam_right[1], cam_right[2], 0.0],
            cam_up: [cam_up[0], cam_up[1], cam_up[2], 0.0],
            center: [frame.center[0], frame.center[1], frame.center[2], frame.planet_r],
            basis_x: [frame.basis[0][0], frame.basis[0][1], frame.basis[0][2], frame.rb],
            basis_y: [frame.basis[1][0], frame.basis[1][1], frame.basis[1][2], frame.rt],
            basis_z: [frame.basis[2][0], frame.basis[2][1], frame.basis[2][2], 0.0],
            proj: [m22, m32, 1.0, 0.0],
        };
        queue.write_buffer(&self.param_buffer, 0, bytemuck::bytes_of(&u));
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Cloud Composite BG"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.param_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Cloud Composite Pass"),
            timestamp_writes,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
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
