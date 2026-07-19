//! Crepuscular god rays (v0.895): one additive full-screen pass that marches
//! the DEPTH buffer toward the sun's screen position, so terrain silhouettes
//! (still in depth right after the celestial pass) carve visible light
//! shafts at low sun angles. No offscreen scene copy, no post chain — the
//! pass samples only the shared depth texture, so it slots between the
//! celestial and interior passes with zero frame-graph surgery.
//! Shader: assets/shaders/godrays.wgsl.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GodrayUniforms {
    sun_uv: [f32; 2],
    aspect: f32,
    intensity: f32,
    color: [f32; 4],
}

pub struct GodrayPass {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    param_buffer: wgpu::Buffer,
}

impl GodrayPass {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Godray Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../assets/shaders/godrays.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Godray BGL"),
            entries: &[
                // Depth texture (sampled with textureLoad — no sampler).
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
            ],
        });

        let param_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Godray Params"),
            size: std::mem::size_of::<GodrayUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Godray Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Godray Pipeline"),
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
                    // Additive: the shader's rgb IS the added sunlight.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            // No depth attachment: the pass READS depth as a texture.
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self { pipeline, bind_group_layout, param_buffer }
    }

    /// Draw the shafts onto `view`. `view_proj` must be the SAME matrix the
    /// depth buffer was rendered with (the celestial camera), `cam_pos` its
    /// eye position, `sun_dir` the world-space direction TOWARD the sun.
    /// Skips itself when the sun projects behind the camera.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        depth_view: &wgpu::TextureView,
        view: &wgpu::TextureView,
        view_proj: Mat4,
        cam_pos: Vec3,
        sun_dir: Vec3,
        aspect: f32,
        intensity: f32,
    ) {
        if intensity <= 0.0 || sun_dir.length_squared() < 0.5 {
            return;
        }
        // Project a point far along the sun direction into clip space.
        let clip = view_proj * (cam_pos + sun_dir * 1.0e9).extend(1.0);
        if clip.w <= 0.0 {
            return; // sun behind the camera — no shafts to draw
        }
        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;
        // 1 Hz diag: where the sun lands on screen (dev tooling; the sun-uv
        // placement is unverifiable from screenshots alone when the disc is
        // washed out by the atmosphere).
        {
            use std::sync::atomic::{AtomicU64, Ordering};
            static LAST: AtomicU64 = AtomicU64::new(0);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if LAST.swap(now, Ordering::Relaxed) != now {
                log::info!("[Godray] sun ndc=({ndc_x:.2},{ndc_y:.2}) w={:.0}", clip.w);
            }
        }
        // Well off-screen: the glow falloff would zero everything anyway.
        if ndc_x.abs() > 2.5 || ndc_y.abs() > 2.5 {
            return;
        }
        let sun_uv = [ndc_x * 0.5 + 0.5, 1.0 - (ndc_y * 0.5 + 0.5)];

        queue.write_buffer(
            &self.param_buffer,
            0,
            bytemuck::bytes_of(&GodrayUniforms {
                sun_uv,
                aspect,
                intensity,
                // Warm low-sun light; the daylight gate on the Rust side
                // scales intensity, the tint stays constant.
                color: [1.0, 0.86, 0.62, 0.0],
            }),
        );

        // The depth view can be recreated on resize, so bind fresh each call
        // (same per-apply pattern as the bloom pass).
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Godray BG"),
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
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Godray Encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Godray Pass"),
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
        queue.submit(std::iter::once(encoder.finish()));
    }
}
