//! The viewer's GPU half (docs/design/model-viewer.md): an egui paint callback. `prepare` draws the
//! model into the viewer's own picture (depth, 4x multisampling), only when something changed;
//! `paint` copies that picture into the page's rectangle inside egui's own render pass.
//!
//! The pipelines and every open viewer's buffers live in [`Gpu`], kept in the egui renderer's
//! callback resources (put there once at start, `lib.rs`). A renderer without them (an in-world
//! screen, the snapshot tests without a GPU) draws nothing, and the page's own words show through.

use super::load::{Alpha, Model, Sampling, Tex, Vertex, Wrap};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use wgpu::util::DeviceExt as _;

/// The viewer's own picture: sRGB, so the shader writes linear light.
const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const SAMPLES: u32 = 4;
/// A viewer not drawn for this long gives its GPU memory back.
const FORGET_AFTER: Duration = Duration::from_secs(10);

/// The three studio lights: direction towards the light in view space, colour, intensity.
const LIGHTS: [([f32; 3], [f32; 3], f32); 3] = [
    ([-0.45, 0.60, 0.66], [1.00, 0.97, 0.92], 2.6),
    ([0.70, 0.15, 0.50], [0.85, 0.90, 1.00], 0.8),
    ([0.00, 0.45, -0.89], [1.00, 1.00, 1.00], 1.1),
];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrameUniform {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    light_dir: [[f32; 4]; 3],
    light_color: [[f32; 4]; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct MaterialUniform {
    base: [f32; 4],
    emissive: [f32; 4],
    params: [f32; 4],
    extra: [f32; 4],
}

/// The pipelines, shared pieces and every open viewer's buffers.
pub struct Gpu {
    /// Indexed `blend * 2 + cull`.
    pipelines: [wgpu::RenderPipeline; 4],
    blit: wgpu::RenderPipeline,
    frame_layout: wgpu::BindGroupLayout,
    material_layout: wgpu::BindGroupLayout,
    blit_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    blit_sampler: wgpu::Sampler,
    white_srgb: wgpu::TextureView,
    white: wgpu::TextureView,
    flat_normal: wgpu::TextureView,
    viewers: HashMap<u64, ViewerGpu>,
}

struct ChunkGpu {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    draws: Vec<super::load::Draw>,
}

struct Targets {
    size: [u32; 2],
    color: wgpu::TextureView,
    resolve: wgpu::TextureView,
    depth: wgpu::TextureView,
    blit: wgpu::BindGroup,
}

/// One open viewer's GPU side.
struct ViewerGpu {
    model_key: u64,
    chunks: Vec<ChunkGpu>,
    /// Per material: its bind group, alpha mode and whether both sides are drawn.
    materials: Vec<(wgpu::BindGroup, Alpha, bool)>,
    _textures: Vec<wgpu::Texture>,
    frame: wgpu::Buffer,
    frame_bind: wgpu::BindGroup,
    targets: Option<Targets>,
    /// What the picture shows now, so an unchanged frame is not drawn again.
    drawn: Option<u64>,
    last_used: Instant,
}

impl Gpu {
    /// Everything shared, for a window whose pixels are `target` (egui's render target).
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target: wgpu::TextureFormat) -> Gpu {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("model_view"),
            source: wgpu::ShaderSource::Wgsl(include_str!("model_view.wgsl").into()),
        });
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("model_view_blit"),
            source: wgpu::ShaderSource::Wgsl(include_str!("model_view_blit.wgsl").into()),
        });
        let uniform = |binding, vis| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: vis,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false },
            count: None,
        };
        let sampler_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let frame_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("model_view_frame"),
            entries: &[uniform(0, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT)],
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("model_view_material"),
            entries: &[
                uniform(0, wgpu::ShaderStages::FRAGMENT),
                texture(1),
                texture(2),
                texture(3),
                texture(4),
                texture(5),
                sampler_entry(6),
                sampler_entry(7),
                sampler_entry(8),
                sampler_entry(9),
                sampler_entry(10),
            ],
        });
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("model_view_blit"),
            entries: &[texture(0), sampler_entry(1)],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("model_view"),
            bind_group_layouts: &[&frame_layout, &material_layout],
            push_constant_ranges: &[],
        });
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
        };
        let pipeline = |blend: bool, cull: bool| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("model_view"),
                layout: Some(&layout),
                vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs_main"), buffers: &[vertex_layout.clone()], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR,
                        blend: blend.then_some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: cull.then_some(wgpu::Face::Back),
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: !blend,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState { count: SAMPLES, ..Default::default() },
                multiview: None,
                cache: None,
            })
        };
        let pipelines = [pipeline(false, false), pipeline(false, true), pipeline(true, false), pipeline(true, true)];
        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("model_view_blit"),
            bind_group_layouts: &[&blit_layout],
            push_constant_ranges: &[],
        });
        let blit = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("model_view_blit"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState { module: &blit_shader, entry_point: Some("vs_blit"), buffers: &[], compilation_options: Default::default() },
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                // egui's own rule: linear into an sRGB window, encoded into any other.
                entry_point: Some(if target.is_srgb() { "fs_blit_linear" } else { "fs_blit_encode" }),
                targets: &[Some(wgpu::ColorTargetState { format: target, blend: None, write_mask: wgpu::ColorWrites::ALL })], // display-format: drawn inside egui's own pass on the window
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let sampler = sampler_for(device, &Sampling::default());
        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("model_view_blit"), ..Default::default() });
        let one = |label: &str, format: wgpu::TextureFormat, rgba: [u8; 4]| {
            let tex = device.create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &rgba,
            );
            tex.create_view(&Default::default())
        };
        Gpu {
            pipelines,
            blit,
            frame_layout,
            material_layout,
            blit_layout,
            sampler,
            blit_sampler,
            white_srgb: one("model_view_white_srgb", wgpu::TextureFormat::Rgba8UnormSrgb, [255; 4]),
            white: one("model_view_white", wgpu::TextureFormat::Rgba8Unorm, [255; 4]),
            flat_normal: one("model_view_flat_normal", wgpu::TextureFormat::Rgba8Unorm, [128, 128, 255, 255]),
            viewers: HashMap::new(),
        }
    }

    /// Upload `model`: its chunks' buffers, its pictures (with their mipmaps) and a bind group per
    /// material.
    fn upload(&self, device: &wgpu::Device, queue: &wgpu::Queue, model: &Model, model_key: u64) -> ViewerGpu {
        let chunks = model
            .chunks
            .iter()
            .map(|c| ChunkGpu {
                vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("model_view_vertices"),
                    contents: bytemuck::cast_slice(&c.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("model_view_indices"),
                    contents: bytemuck::cast_slice(&c.indices),
                    usage: wgpu::BufferUsages::INDEX,
                }),
                draws: c.draws.clone(),
            })
            .collect();
        // Each picture once, viewable as sRGB (colour) or linear (data).
        let textures: Vec<wgpu::Texture> = model
            .images
            .iter()
            .map(|img| {
                let tex = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("model_view_texture"),
                    size: wgpu::Extent3d { width: img.width, height: img.height, depth_or_array_layers: 1 },
                    mip_level_count: img.levels.len() as u32,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[wgpu::TextureFormat::Rgba8UnormSrgb],
                });
                for (level, data) in img.levels.iter().enumerate() {
                    let (w, h) = ((img.width >> level).max(1), (img.height >> level).max(1));
                    queue.write_texture(
                        wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: level as u32, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                        data,
                        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
                        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                    );
                }
                tex
            })
            .collect();
        // One sampler per way of reading a texture that the file uses.
        let mut samplers: HashMap<Sampling, wgpu::Sampler> = HashMap::new();
        for m in &model.materials {
            for t in [m.base_texture, m.metallic_roughness_texture, m.normal_texture, m.occlusion_texture, m.emissive_texture].into_iter().flatten() {
                samplers.entry(t.sampling).or_insert_with(|| sampler_for(device, &t.sampling));
            }
        }
        let sampler = |t: Option<Tex>| -> &wgpu::Sampler { t.and_then(|t| samplers.get(&t.sampling)).unwrap_or(&self.sampler) };
        let view = |t: Option<Tex>, srgb: bool, fallback: &wgpu::TextureView| -> wgpu::TextureView {
            match t.and_then(|t| textures.get(t.image)) {
                Some(t) => t.create_view(&wgpu::TextureViewDescriptor {
                    format: Some(if srgb { wgpu::TextureFormat::Rgba8UnormSrgb } else { wgpu::TextureFormat::Rgba8Unorm }),
                    ..Default::default()
                }),
                None => fallback.clone(),
            }
        };
        let materials = model
            .materials
            .iter()
            .map(|m| {
                let uniform = MaterialUniform {
                    base: m.base_color,
                    emissive: [m.emissive[0], m.emissive[1], m.emissive[2], 0.0],
                    params: [m.metallic, m.roughness, m.alpha_cutoff, match m.alpha { Alpha::Opaque => 0.0, Alpha::Mask => 1.0, Alpha::Blend => 2.0 }],
                    extra: [m.normal_scale, m.occlusion_strength, if m.normal_texture.is_some() { 1.0 } else { 0.0 }, 0.0],
                };
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("model_view_material"),
                    contents: bytemuck::bytes_of(&uniform),
                    usage: wgpu::BufferUsages::UNIFORM,
                });
                // A missing emissive picture is white, times the emissive factor (zero unless set).
                let views = [
                    view(m.base_texture, true, &self.white_srgb),
                    view(m.metallic_roughness_texture, false, &self.white),
                    view(m.normal_texture, false, &self.flat_normal),
                    view(m.occlusion_texture, false, &self.white),
                    view(m.emissive_texture, true, &self.white_srgb),
                ];
                let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("model_view_material"),
                    layout: &self.material_layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: buffer.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&views[0]) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&views[1]) },
                        wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&views[2]) },
                        wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&views[3]) },
                        wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::TextureView(&views[4]) },
                        wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::Sampler(sampler(m.base_texture)) },
                        wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::Sampler(sampler(m.metallic_roughness_texture)) },
                        wgpu::BindGroupEntry { binding: 8, resource: wgpu::BindingResource::Sampler(sampler(m.normal_texture)) },
                        wgpu::BindGroupEntry { binding: 9, resource: wgpu::BindingResource::Sampler(sampler(m.occlusion_texture)) },
                        wgpu::BindGroupEntry { binding: 10, resource: wgpu::BindingResource::Sampler(sampler(m.emissive_texture)) },
                    ],
                });
                (bind, m.alpha, m.double_sided)
            })
            .collect();
        let frame = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("model_view_frame"),
            size: std::mem::size_of::<FrameUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let frame_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("model_view_frame"),
            layout: &self.frame_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: frame.as_entire_binding() }],
        });
        ViewerGpu { model_key, chunks, materials, _textures: textures, frame, frame_bind, targets: None, drawn: None, last_used: Instant::now() }
    }

    /// The viewer's picture, depth and multisampled colour for a view `size` pixels across.
    fn targets(&self, device: &wgpu::Device, size: [u32; 2]) -> Targets {
        let extent = wgpu::Extent3d { width: size[0], height: size[1], depth_or_array_layers: 1 };
        let make = |label: &str, format: wgpu::TextureFormat, samples: u32, usage: wgpu::TextureUsages| {
            device
                .create_texture(&wgpu::TextureDescriptor { label: Some(label), size: extent, mip_level_count: 1, sample_count: samples, dimension: wgpu::TextureDimension::D2, format, usage, view_formats: &[] })
                .create_view(&Default::default())
        };
        let color = make("model_view_color", COLOR, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let resolve = make("model_view_picture", COLOR, 1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING);
        let depth = make("model_view_depth", DEPTH, SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let blit = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("model_view_blit"),
            layout: &self.blit_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&resolve) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.blit_sampler) },
            ],
        });
        Targets { size, color, resolve, depth, blit }
    }
}

/// A sampler that reads a texture the way `s` says: sharp or smooth each way, mipmaps or none
/// (level 0 only), repeating, clamped or mirrored; anisotropic filtering when every step is
/// smooth (wgpu allows it only then).
fn sampler_for(device: &wgpu::Device, s: &Sampling) -> wgpu::Sampler {
    let filter = |nearest: bool| if nearest { wgpu::FilterMode::Nearest } else { wgpu::FilterMode::Linear };
    let address = |w: Wrap| match w {
        Wrap::Repeat => wgpu::AddressMode::Repeat,
        Wrap::Clamp => wgpu::AddressMode::ClampToEdge,
        Wrap::Mirror => wgpu::AddressMode::MirrorRepeat,
    };
    let smooth = !s.mag_nearest && !s.min_nearest && s.mip_nearest == Some(false);
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("model_view"),
        address_mode_u: address(s.wrap[0]),
        address_mode_v: address(s.wrap[1]),
        mag_filter: filter(s.mag_nearest),
        min_filter: filter(s.min_nearest),
        mipmap_filter: filter(s.mip_nearest.unwrap_or(true)),
        lod_max_clamp: if s.mip_nearest.is_none() { 0.0 } else { 32.0 },
        anisotropy_clamp: if smooth { 8 } else { 1 },
        ..Default::default()
    })
}

/// One frame of one viewer: what to draw and from where. Built by the page every frame it shows
/// the viewer (`super::Viewer::show`).
pub struct Paint {
    pub viewer: u64,
    pub model: Arc<Model>,
    pub model_key: u64,
    pub view: Mat4,
    pub view_proj: Mat4,
    pub eye: Vec3,
    /// The view's size in pixels.
    pub size: [u32; 2],
    /// The background, linear.
    pub background: [f32; 4],
}

impl Paint {
    /// What the picture would show: a frame with the same key is not drawn again.
    fn key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.model_key.hash(&mut h);
        self.size.hash(&mut h);
        for f in self.view_proj.to_cols_array().iter().chain(self.background.iter()) {
            f.to_bits().hash(&mut h);
        }
        h.finish()
    }

    fn frame_uniform(&self) -> FrameUniform {
        let to_world = self.view.inverse();
        let mut light_dir = [[0.0; 4]; 3];
        let mut light_color = [[0.0; 4]; 3];
        for (i, (dir, color, intensity)) in LIGHTS.iter().enumerate() {
            let d = to_world.transform_vector3(Vec3::from_array(*dir)).normalize_or(Vec3::Y);
            light_dir[i] = [d.x, d.y, d.z, *intensity];
            light_color[i] = [color[0], color[1], color[2], 1.0];
        }
        FrameUniform { view_proj: self.view_proj.to_cols_array_2d(), eye: [self.eye.x, self.eye.y, self.eye.z, 1.0], light_dir, light_color }
    }
}

impl egui_wgpu::CallbackTrait for Paint {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(gpu) = resources.get_mut::<Gpu>() else { return Vec::new() };
        if self.size[0] == 0 || self.size[1] == 0 {
            return Vec::new();
        }
        let now = Instant::now();
        gpu.viewers.retain(|_, v| now.duration_since(v.last_used) < FORGET_AFTER);
        if gpu.viewers.get(&self.viewer).map(|v| v.model_key) != Some(self.model_key) {
            let fresh = gpu.upload(device, queue, &self.model, self.model_key);
            gpu.viewers.insert(self.viewer, fresh);
        }
        if gpu.viewers.get(&self.viewer).and_then(|v| v.targets.as_ref()).map(|t| t.size) != Some(self.size) {
            let targets = gpu.targets(device, self.size);
            if let Some(v) = gpu.viewers.get_mut(&self.viewer) {
                v.targets = Some(targets);
                v.drawn = None;
            }
        }
        let Gpu { pipelines, viewers, .. } = gpu;
        let Some(v) = viewers.get_mut(&self.viewer) else { return Vec::new() };
        v.last_used = now;
        let key = self.key();
        if v.drawn == Some(key) {
            return Vec::new(); // nothing changed: the picture is shown again as it is
        }
        let Some(t) = v.targets.as_ref() else { return Vec::new() };
        queue.write_buffer(&v.frame, 0, bytemuck::bytes_of(&self.frame_uniform()));
        let [r, g, b, a] = self.background;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("model_view"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &t.color,
                resolve_target: Some(&t.resolve),
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: r as f64, g: g as f64, b: b as f64, a: a as f64 }), store: wgpu::StoreOp::Discard },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &t.depth,
                depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        pass.set_bind_group(0, &v.frame_bind, &[]);
        // Opaque and masked parts first, then blended ones over them.
        for blended in [false, true] {
            for chunk in &v.chunks {
                pass.set_vertex_buffer(0, chunk.vertices.slice(..));
                pass.set_index_buffer(chunk.indices.slice(..), wgpu::IndexFormat::Uint32);
                for d in &chunk.draws {
                    let Some((bind, alpha, double_sided)) = v.materials.get(d.material) else { continue };
                    if (*alpha == Alpha::Blend) != blended {
                        continue;
                    }
                    pass.set_pipeline(&pipelines[usize::from(blended) * 2 + usize::from(!double_sided)]);
                    pass.set_bind_group(1, bind, &[]);
                    pass.draw_indexed(d.first..d.first + d.count, 0, 0..1);
                }
            }
        }
        drop(pass);
        v.drawn = Some(key);
        Vec::new()
    }

    fn paint(&self, _info: egui::PaintCallbackInfo, render_pass: &mut wgpu::RenderPass<'static>, resources: &egui_wgpu::CallbackResources) {
        let Some(gpu) = resources.get::<Gpu>() else { return };
        let Some(t) = gpu.viewers.get(&self.viewer).filter(|v| v.drawn.is_some()).and_then(|v| v.targets.as_ref()) else { return };
        render_pass.set_pipeline(&gpu.blit);
        render_pass.set_bind_group(0, &t.blit, &[]);
        render_pass.draw(0..3, 0..1);
    }
}
