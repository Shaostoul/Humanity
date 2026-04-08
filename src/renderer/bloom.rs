//! Bloom post-process effect.
//!
//! Extracts bright pixels from the scene, applies Gaussian blur,
//! and composites back for emissive glow (sun, lava, neon, magic).
//!
//! The bloom pipeline operates at half resolution for performance:
//! 1. Threshold pass: extract bright pixels → half-res texture A
//! 2. Horizontal blur: texture A → texture B
//! 3. Vertical blur: texture B → texture A
//! 4. Composite: original scene + texture A → final output
//!
//! Configurable via bloom_threshold and bloom_intensity in GuiState.

use wgpu::util::DeviceExt;

/// Bloom parameters passed to the GPU.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BloomUniforms {
    /// x = threshold, y = intensity, z = texel_width, w = texel_height
    params: [f32; 4],
}

/// GPU resources for the bloom post-process.
pub struct BloomPass {
    /// Half-resolution texture A (threshold output, vertical blur output)
    texture_a: wgpu::Texture,
    view_a: wgpu::TextureView,
    /// Half-resolution texture B (horizontal blur output)
    texture_b: wgpu::Texture,
    view_b: wgpu::TextureView,
    /// Sampler for bloom textures
    sampler: wgpu::Sampler,
    /// Uniform buffer for bloom parameters
    param_buffer: wgpu::Buffer,
    /// Bind group layout for input texture + sampler + params
    bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group layout for the composite pass (bloom texture)
    composite_bind_group_layout: wgpu::BindGroupLayout,
    /// Pipeline for threshold extraction
    threshold_pipeline: wgpu::RenderPipeline,
    /// Pipeline for horizontal blur
    blur_h_pipeline: wgpu::RenderPipeline,
    /// Pipeline for vertical blur
    blur_v_pipeline: wgpu::RenderPipeline,
    /// Pipeline for final composite
    composite_pipeline: wgpu::RenderPipeline,
    /// Half-resolution dimensions
    half_width: u32,
    half_height: u32,
    /// Surface format for compatibility
    surface_format: wgpu::TextureFormat,
}

impl BloomPass {
    pub fn new(device: &wgpu::Device, width: u32, height: u32, surface_format: wgpu::TextureFormat) -> Self {
        let half_width = (width / 2).max(1);
        let half_height = (height / 2).max(1);

        // Bloom textures at half resolution (use Rgba16Float for HDR if available, else surface format)
        let bloom_format = wgpu::TextureFormat::Rgba8UnormSrgb; // Safe fallback
        let (texture_a, view_a) = create_bloom_texture(device, half_width, half_height, bloom_format, "Bloom A");
        let (texture_b, view_b) = create_bloom_texture(device, half_width, half_height, bloom_format, "Bloom B");

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let param_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Bloom Params"),
            contents: bytemuck::bytes_of(&BloomUniforms { params: [0.8, 1.0, 1.0 / half_width as f32, 1.0 / half_height as f32] }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group layout: texture + sampler + uniform
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bloom BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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

        // Composite bind group layout (bloom texture only)
        let composite_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bloom Composite BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Load bloom shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bloom Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../assets/shaders/bloom.wgsl").into()),
        });

        let threshold_pipeline = create_fullscreen_pipeline(
            device, &shader, "fs_threshold", bloom_format,
            &[&bind_group_layout], "Bloom Threshold",
        );
        let blur_h_pipeline = create_fullscreen_pipeline(
            device, &shader, "fs_blur_h", bloom_format,
            &[&bind_group_layout], "Bloom Blur H",
        );
        let blur_v_pipeline = create_fullscreen_pipeline(
            device, &shader, "fs_blur_v", bloom_format,
            &[&bind_group_layout], "Bloom Blur V",
        );
        let composite_pipeline = create_fullscreen_pipeline(
            device, &shader, "fs_composite", surface_format,
            &[&bind_group_layout, &composite_bind_group_layout], "Bloom Composite",
        );

        Self {
            texture_a, view_a,
            texture_b, view_b,
            sampler, param_buffer,
            bind_group_layout, composite_bind_group_layout,
            threshold_pipeline, blur_h_pipeline, blur_v_pipeline, composite_pipeline,
            half_width, half_height,
            surface_format,
        }
    }

    /// Run the bloom post-process on a rendered scene.
    /// `scene_view` is the main scene texture view (rendered scene).
    /// `output_view` is where the final composited result goes.
    pub fn apply(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
        threshold: f32,
        intensity: f32,
    ) {
        // Update params
        queue.write_buffer(
            &self.param_buffer,
            0,
            bytemuck::bytes_of(&BloomUniforms {
                params: [threshold, intensity, 1.0 / self.half_width as f32, 1.0 / self.half_height as f32],
            }),
        );

        let scene_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom Scene BG"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(scene_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: self.param_buffer.as_entire_binding() },
            ],
        });

        let tex_a_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom TexA BG"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.view_a) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: self.param_buffer.as_entire_binding() },
            ],
        });

        let tex_b_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom TexB BG"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.view_b) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                wgpu::BindGroupEntry { binding: 2, resource: self.param_buffer.as_entire_binding() },
            ],
        });

        let bloom_composite_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom Composite BG"),
            layout: &self.composite_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.view_a) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Bloom Encoder"),
        });

        // Pass 1: Threshold (scene → texture A at half res)
        run_fullscreen_pass(&mut encoder, &self.view_a, &self.threshold_pipeline, &scene_bg, None, "Bloom Threshold");

        // Pass 2: Horizontal blur (texture A → texture B)
        run_fullscreen_pass(&mut encoder, &self.view_b, &self.blur_h_pipeline, &tex_a_bg, None, "Bloom Blur H");

        // Pass 3: Vertical blur (texture B → texture A)
        run_fullscreen_pass(&mut encoder, &self.view_a, &self.blur_v_pipeline, &tex_b_bg, None, "Bloom Blur V");

        // Pass 4: Composite (scene + bloom → output)
        run_fullscreen_pass(&mut encoder, output_view, &self.composite_pipeline, &scene_bg, Some(&bloom_composite_bg), "Bloom Composite");

        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Resize bloom textures when the window resizes.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        self.half_width = (width / 2).max(1);
        self.half_height = (height / 2).max(1);
        let bloom_format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let (ta, va) = create_bloom_texture(device, self.half_width, self.half_height, bloom_format, "Bloom A");
        let (tb, vb) = create_bloom_texture(device, self.half_width, self.half_height, bloom_format, "Bloom B");
        self.texture_a = ta;
        self.view_a = va;
        self.texture_b = tb;
        self.view_b = vb;
    }
}

fn create_bloom_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

fn create_fullscreen_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    fs_entry: &str,
    target_format: wgpu::TextureFormat,
    bind_group_layouts: &[&wgpu::BindGroupLayout],
    label: &str,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts,
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_fullscreen"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(fs_entry),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

fn run_fullscreen_pass(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    bind_group_0: &wgpu::BindGroup,
    bind_group_1: Option<&wgpu::BindGroup>,
    label: &str,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        ..Default::default()
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group_0, &[]);
    if let Some(bg1) = bind_group_1 {
        pass.set_bind_group(1, bg1, &[]);
    }
    pass.draw(0..3, 0..1); // fullscreen triangle
}
