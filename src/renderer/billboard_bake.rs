//! Automated billboard sprite baker (v0.959, operator call 2026-07-25:
//! "bake our own... one less thing we have to manually make every time we
//! add a 3D model"). Renders a model's parts side-on into a transparent
//! RGBA sprite - the 1990s pre-render pipeline (Total Annihilation's
//! terrain trees, StarCraft/AoE units were 3D models rendered to 2D art),
//! but automated and in-engine, so ANY model (including mods) gets its
//! card sprite for free.
//!
//! Increment 1: the baker itself + a PNG dump reachable from the showcase
//! IPC ("bake":"trees") for eyeball + scripted verification. The alpha-card
//! LOD rung (cards textured with these sprites) is the next increment.
//!
//! Design notes:
//! - UNLIT albedo capture: the sprite stores base color + coverage alpha;
//!   lighting belongs to the consumer (the card shader lights cards like
//!   the terrain around them). Baking lighting in would freeze one sun
//!   angle into every card.
//! - Side-on orthographic view along -Z, model Y-up, framed on the parts'
//!   joint AABB with a 5% margin. Trees are yaw-randomized by the card
//!   stream, so one side view is the right budget; multi-angle imposters
//!   can layer on later if needed.
//! - The color target uses the SWAPCHAIN format (like the hi-res capture
//!   path) so `read_texture_to_png`'s bgra swizzle logic applies unchanged.

use super::mesh::Vertex;
use super::Renderer;
use wgpu::util::DeviceExt;

/// One model part to bake (crown, trunk, ...): CPU geometry + optional
/// RGBA8 base-color texture. Parts render into the same sprite.
pub struct BakePart<'a> {
    pub vertices: &'a [Vertex],
    pub indices: &'a [u32],
    pub texture: Option<(&'a [u8], u32, u32)>,
}

const BAKE_WGSL: &str = r#"
struct BakeUniform {
    mvp: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> u: BakeUniform;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) pos: vec3<f32>, @location(1) nrm: vec3<f32>, @location(2) uv: vec2<f32>) -> VsOut {
    var out: VsOut;
    out.pos = u.mvp * vec4<f32>(pos, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex, samp, in.uv);
    // Foliage cutout: the same 0.5 coverage threshold the type-19 path
    // uses, so the sprite's silhouette matches the real model's.
    if (c.a < 0.5) {
        discard;
    }
    return vec4<f32>(c.rgb, 1.0);
}
"#;

/// Atlas geometry shared by the baker and the type-12 shader branch:
/// 3 columns x 2 rows of TILE_PX sprites (fir v1-3 = tiles 0-2,
/// pine v1-3 = tiles 3-5).
pub const ATLAS_COLS: u32 = 3;
pub const ATLAS_ROWS: u32 = 2;
pub const ATLAS_TILE_PX: u32 = 512;

impl Renderer {
    /// Bake `parts` into a `size` x `size` sprite and write it as a PNG
    /// (transparent background). Returns the sprite's world-space footprint
    /// (width_m, height_m) so the consumer can size cards to match.
    pub fn bake_billboard_to_png(
        &self,
        parts: &[BakePart<'_>],
        size: u32,
        path: &std::path::Path,
    ) -> Result<(f32, f32), String> {
        let (color, footprint) = self.bake_billboard_texture(parts, size)?;
        self.read_texture_to_png(&color, size, size, path)?;
        Ok(footprint)
    }

    /// Bake all six conifer sprites into the persistent tree-card atlas
    /// (increment 2): each entry in `trees` is one stem's parts (crown +
    /// bark), baked at ATLAS_TILE_PX and copied into its 3x2 grid slot.
    /// The atlas texture was created at init and is referenced by every
    /// group-3 bind group, so this is an in-place rewrite - no rebuilds.
    pub fn bake_tree_atlas(&mut self, trees: &[Vec<BakePart<'_>>]) -> Result<(), String> {
        for (i, parts) in trees.iter().enumerate().take((ATLAS_COLS * ATLAS_ROWS) as usize) {
            let (tex, _fp) = self.bake_billboard_texture(parts, ATLAS_TILE_PX)?;
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("tree_atlas_copy"),
                });
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &self.tree_atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: (i as u32 % ATLAS_COLS) * ATLAS_TILE_PX,
                        y: (i as u32 / ATLAS_COLS) * ATLAS_TILE_PX,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: ATLAS_TILE_PX,
                    height: ATLAS_TILE_PX,
                    depth_or_array_layers: 1,
                },
            );
            self.queue.submit([encoder.finish()]);
        }
        self.tree_atlas_ready = true;
        Ok(())
    }

    /// Render `parts` side-on into a fresh `size` x `size` texture
    /// (swapchain format, transparent clear, COPY_SRC). The core the PNG
    /// dump and the atlas builder share.
    pub fn bake_billboard_texture(
        &self,
        parts: &[BakePart<'_>],
        size: u32,
    ) -> Result<(wgpu::Texture, (f32, f32)), String> {
        if parts.is_empty() {
            return Err("no parts to bake".to_string());
        }
        // Joint AABB over every part.
        let mut mn = [f32::MAX; 3];
        let mut mx = [f32::MIN; 3];
        for p in parts {
            for v in p.vertices {
                for i in 0..3 {
                    mn[i] = mn[i].min(v.position[i]);
                    mx[i] = mx[i].max(v.position[i]);
                }
            }
        }
        if mn[0] > mx[0] {
            return Err("parts contain no vertices".to_string());
        }
        // Frame: widest horizontal extent (the card stream yaws trees
        // randomly, so X vs Z is arbitrary - take the larger), full height.
        let w_m = (mx[0] - mn[0]).max(mx[2] - mn[2]).max(1e-3);
        let h_m = (mx[1] - mn[1]).max(1e-3);
        let half = 0.5 * w_m.max(h_m) * 1.05; // square frame, 5% margin
        let cx = 0.5 * (mn[0] + mx[0]);
        let cy = 0.5 * (mn[1] + mx[1]);
        let cz = 0.5 * (mn[2] + mx[2]);
        let eye = glam::Vec3::new(cx, cy, mx[2] + w_m + 1.0);
        let view = glam::Mat4::look_at_rh(eye, glam::Vec3::new(cx, cy, cz), glam::Vec3::Y);
        let depth_span = (mx[2] - mn[2]) + 2.0 * (w_m + 1.0);
        let proj = glam::Mat4::orthographic_rh(-half, half, -half, half, 0.01, depth_span.max(1.0));
        let mvp = proj * view;

        // One-shot pipeline: this is a dev-facing bake, not a per-frame
        // path; building it fresh keeps the renderer's hot state untouched.
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("billboard_bake"),
                source: wgpu::ShaderSource::Wgsl(BAKE_WGSL.into()),
            });
        let bgl = self
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("billboard_bake_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("billboard_bake_layout"),
                bind_group_layouts: &[&bgl],
                push_constant_ranges: &[],
            });
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("billboard_bake_pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Vertex::layout()],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.config.format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None, // foliage cards are double-sided
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
            });

        let color = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("billboard_bake_color"),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_view = color.create_view(&Default::default());
        let depth = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("billboard_bake_depth"),
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&Default::default());

        let ubuf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("billboard_bake_mvp"),
                contents: bytemuck::cast_slice(&mvp.to_cols_array()),
                usage: wgpu::BufferUsages::UNIFORM,
            });
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("billboard_bake_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // Untextured parts sample this 1x1 neutral gray-green.
        let fallback: [u8; 4] = [90, 110, 70, 255];

        // Per-part GPU resources, then one render pass drawing them all.
        let mut draws = Vec::new();
        for p in parts {
            let vb = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("billboard_bake_vb"),
                    contents: bytemuck::cast_slice(p.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let ib = self
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("billboard_bake_ib"),
                    contents: bytemuck::cast_slice(p.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
            let (bytes, tw, th) = match p.texture {
                Some((b, w, h)) => (b, w, h),
                None => (&fallback[..], 1, 1),
            };
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("billboard_bake_tex"),
                size: wgpu::Extent3d { width: tw, height: th, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * tw),
                    rows_per_image: Some(th),
                },
                wgpu::Extent3d { width: tw, height: th, depth_or_array_layers: 1 },
            );
            let tv = tex.create_view(&Default::default());
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("billboard_bake_bg"),
                layout: &bgl,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&tv) },
                    wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
                ],
            });
            draws.push((vb, ib, p.indices.len() as u32, bg, tex, tv));
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("billboard_bake") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("billboard_bake_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Transparent background: alpha 0 everywhere the
                        // model does not cover.
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&pipeline);
            for (vb, ib, n, bg, _tex, _tv) in &draws {
                pass.set_bind_group(0, bg, &[]);
                pass.set_vertex_buffer(0, vb.slice(..));
                pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..*n, 0, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);

        Ok((color, (2.0 * half, 2.0 * half)))
    }
}
