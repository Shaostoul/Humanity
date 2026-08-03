//! Screen-space ambient occlusion (v0.901; estimator rebuilt v0.1100):
//! depth-only INPUT, normal-aware contact shading in the celestial slot —
//! runs right after the god-ray pass while depth still holds terrain +
//! vegetation, multiplying the color target so creases and tree bases pick
//! up contact shade. The v0.1100 rebuild (BUG-062 "tree aura") reconstructs
//! view-space normals from depth and rejects taps outside a 2x-radius range,
//! so foreground objects no longer shade ground behind them. Same
//! zero-infrastructure pattern as `godrays.rs`: one full-screen triangle,
//! reads the shared depth view, no offscreen copies.
//! Shader: assets/shaders/ssao.wgsl.

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct SsaoUniforms {
    proj: [f32; 4],
    params: [f32; 4],
}

pub struct SsaoPass {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    param_buffer: wgpu::Buffer,
}

impl SsaoPass {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SSAO Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../assets/shaders/ssao.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSAO BGL"),
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
            ],
        });

        let param_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SSAO Params"),
            size: std::mem::size_of::<SsaoUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSAO Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SSAO Pipeline"),
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
                    // Multiply: out = src * dst. The shader's grayscale AO
                    // darkens what is already there; sky outputs 1.0.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Dst,
                            dst_factor: wgpu::BlendFactor::Zero,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self { pipeline, bind_group_layout, param_buffer }
    }

    /// Draw the AO multiply onto `view`. `m22`/`m32` are the celestial
    /// projection's column-major [2][2] and [3][2] elements (for reverse-Z
    /// depth linearization), `focal_px` the true focal length in pixels
    /// ((h/2)/tan(fov/2)) — the shader reconstructs view-space positions
    /// from it (v0.1100 normal-aware estimator).
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        depth_view: &wgpu::TextureView,
        view: &wgpu::TextureView,
        m22: f32,
        m32: f32,
        focal_px: f32,
        radius_m: f32,
        strength: f32,
        timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
    ) {
        if strength <= 0.001 {
            return;
        }
        queue.write_buffer(
            &self.param_buffer,
            0,
            bytemuck::bytes_of(&SsaoUniforms {
                proj: [m22, m32, focal_px, 1.0],
                params: [radius_m, strength, 0.0, 0.0],
            }),
        );
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSAO BG"),
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
            label: Some("SSAO Encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SSAO Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                // Frame-cost measurement (gpu.ssao), threaded from the
                // renderer's timestamp-query ring. `None` on adapters without
                // TIMESTAMP_QUERY, where the CPU submit stage stands in.
                timestamp_writes,
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }
}

#[cfg(test)]
mod tests {
    /// The SSAO shader is compiled at renderer init, so a WGSL error makes
    /// the whole app unbootable while every static Rust check stays green
    /// (the v0.782 class of failure). Parse + validate it here, same pattern
    /// as the star/halo shader tests in stars.rs.
    #[test]
    fn ssao_shader_parses_and_validates() {
        let src = include_str!("../../assets/shaders/ssao.wgsl");
        let module = wgpu::naga::front::wgsl::parse_str(src)
            .unwrap_or_else(|e| panic!("ssao.wgsl failed to parse: {e}"));
        let mut validator = wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        );
        validator
            .validate(&module)
            .unwrap_or_else(|e| panic!("ssao.wgsl failed naga validation: {e:?}"));
    }
}
