//! THE SCENE TARGET and THE PRESENT PASS (HDR scene target, increments 1
//! and 2; the plan of record is docs/design/hdr-scene-target.md).
//!
//! WHAT CHANGED. Until 2026-09-27 every scene pass drew straight into the
//! display target: the swapchain for the live frame, a camera screen's
//! texture, a hi-res capture target. Each shader tonemapped inline and wrote
//! 8-bit display values, so a slow dark gradient quantised into flat rings
//! one display level apart, and there was nowhere to put one tonemap and one
//! dither. Now the scene is drawn into a [`SceneTarget`], and [`PresentPass`]
//! copies it into the display target at the end; egui draws after that, on
//! the display target, and is never touched by any of this.
//!
//! WHY IT IS BIT-EXACT TODAY. [`scene_format_for`] returns the display format
//! itself, so the scene target has the same format as the thing it is copied
//! into, every pass draws and blends into it exactly as it drew into the
//! display before, and the present shader (assets/shaders/present.wgsl) is a
//! `textureLoad` of the pixel's own texel written back with no blend, alpha
//! included. The GPU test below proves the copy is byte-exact for every code
//! of every channel, in all four 8-bit formats a surface can pick, and that a
//! blended draw through the target equals the same draw straight into the
//! display. Increment 3 changes ONE line, `scene_format_for`, to
//! `Rgba16Float`; the shaders stay untouched and the present pass clamps.
//!
//! THE RULES THIS FILE CARRIES:
//! * Every scene pipeline is built for [`Renderer::scene_format`], never for
//!   the display format. `tests/scene_format_lint.rs` checks every builder
//!   call site and makes every other read of the display format justify
//!   itself with an inline `display-format:` note.
//! * The present bind group layout has ONE `create_bind_group` site,
//!   [`PresentPass::bind`], shared by init, resize and the off-screen view
//!   scratch (the v0.1029 lesson: a layout with several creation sites is
//!   one missed edit away from a world-entry panic). The lint counts it.
//! * An off-screen view (a camera screen, the hi-res screenshot) draws into a
//!   view-sized SCRATCH target and is presented into its display target by
//!   the same pass. The scratch follows `view_depth.rs`'s parked/active rule:
//!   kept for the camera screens (the same size again in 100 ms), dropped
//!   after a one-off screenshot (a 7680 x 4320 scratch must not linger).
//!
//! THE A/B SWITCH. `Renderer::present_direct` (the rig's showcase key
//! `present_direct`) draws the scene straight into the display target again,
//! the pre-2026-09-27 path, so a same-boot A/B can prove the target changes
//! no pixel and can read what the present pass costs. It only acts while the
//! scene and display formats are equal; from increment 3 on it is ignored
//! (logged), because a scene pipeline cannot draw into another format.

use super::{frame_costs, Renderer};
use bytemuck::{Pod, Zeroable};

/// The format the scene is drawn in, given the display's format. THE line
/// increment 3 changes (to `Rgba16Float`). Everything that builds a scene
/// pipeline asks `Renderer::scene_format`, which is this at init.
pub fn scene_format_for(display: wgpu::TextureFormat) -> wgpu::TextureFormat {
    display
}

/// The present pass's uniform (binding 1). See present.wgsl for the lanes.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PresentParams {
    /// x: clamp rgba to 0..1 (1) or not (0). y, z, w: reserved.
    flags: [f32; 4],
}

/// A colour target the scene is drawn into, with its own present bind group
/// (made once, with the texture, so presenting it costs no allocation).
pub struct SceneTarget {
    pub(super) texture: wgpu::Texture,
    view: wgpu::TextureView,
    format: wgpu::TextureFormat,
    bind_group: wgpu::BindGroup,
}

impl SceneTarget {
    /// A `width` x `height` target in `format`, bound for `present`.
    /// COPY_SRC for readbacks and COPY_DST for the round-trip test's upload;
    /// neither changes how the target renders.
    pub fn new(
        device: &wgpu::Device,
        present: &PresentPass,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Scene Target"),
            size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = present.bind(device, &view);
        Self { texture, view, format, bind_group }
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn size(&self) -> (u32, u32) {
        (self.texture.width(), self.texture.height())
    }

    /// The view the scene passes render into. A clone is an `Arc` bump.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

/// The full-screen pass that copies a [`SceneTarget`] into a display target.
pub struct PresentPass {
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    params: wgpu::Buffer,
    output_format: wgpu::TextureFormat,
}

impl PresentPass {
    /// Build the pass for display targets in `output_format` (the swapchain's
    /// format: camera screens and hi-res capture targets use it too).
    pub fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Present Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../assets/shaders/present.wgsl").into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Present BGL"),
            entries: &[
                // The scene target, read with textureLoad (no sampler). Not
                // filterable, so any float format can bind here, including
                // the Rgba16Float of increment 3.
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
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
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<PresentParams>() as u64),
                    },
                    count: None,
                },
            ],
        });
        // Zero-initialised by wgpu: flags (0, 0, 0, 0), the passthrough.
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Present Params"),
            size: std::mem::size_of::<PresentParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Present Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Present Pipeline"),
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
                    format: output_format,
                    // Replace, all four channels: the copy must carry alpha
                    // too, because the capture writes an RGBA PNG.
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
        Self { bind_group_layout, pipeline, params, output_format }
    }

    /// The display format this pass writes.
    pub fn output_format(&self) -> wgpu::TextureFormat {
        self.output_format
    }

    /// THE ONE `create_bind_group` site for the present layout. Every scene
    /// target (the window's, rebuilt on resize, and each view scratch) gets
    /// its bind group here, through `SceneTarget::new`.
    fn bind(&self, device: &wgpu::Device, scene_view: &wgpu::TextureView) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Present Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(scene_view) },
                wgpu::BindGroupEntry { binding: 1, resource: self.params.as_entire_binding() },
            ],
        })
    }

    /// Encode the copy of `scene` into `output`, which must be in
    /// `output_format` and the same size as `scene`.
    pub fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        scene: &SceneTarget,
        output: &wgpu::TextureView,
        timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Present Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output,
                resolve_target: None,
                // Every pixel is overwritten, so the old contents never matter.
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &scene.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// The off-screen views' scratch target and whether a view is being drawn.
/// Exactly one view is in flight at a time (`render_view_onto` is not
/// re-entrant), so one spare is the whole cache; two camera screens of
/// different sizes alternate re-creating it, the same trade `ViewDepth` made.
#[derive(Default)]
pub(super) struct ViewScene {
    spare: Option<SceneTarget>,
    state: ViewSceneState,
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
enum ViewSceneState {
    /// No view in flight.
    #[default]
    Idle,
    /// A view is drawing into `spare`; `end_view_scene` presents it.
    Scratch,
    /// A view is drawing straight into its display target (`present_direct`).
    Direct,
}

impl Renderer {
    /// The format every scene pipeline is built for and the scene target is
    /// allocated in. Equal to the display format until increment 3.
    pub fn scene_format(&self) -> wgpu::TextureFormat {
        self.scene.format()
    }

    /// Whether the A/B switch is in force: asked for, and possible (the
    /// scene pipelines can only draw into the display target while the two
    /// formats agree).
    fn present_bypassed(&self) -> bool {
        self.present_direct && self.scene.format() == self.config.format // display-format: the A/B switch needs both formats equal
    }

    /// The view this frame's scene passes draw into: the scene target, or
    /// `display` itself while the A/B switch is on.
    pub fn scene_view_for(&self, display: &wgpu::TextureView) -> wgpu::TextureView {
        if self.present_bypassed() {
            display.clone()
        } else {
            self.scene.view().clone()
        }
    }

    /// Copy the finished scene into `display` (the swapchain view), timed as
    /// `gpu.present` / `cpu.present`. Call after the last scene pass and
    /// before egui. A no-op while the A/B switch has the scene drawn into
    /// `display` already.
    pub fn present_scene(&self, display: &wgpu::TextureView) {
        if self.present_bypassed() {
            return;
        }
        let (gpu_id, cpu_id) = frame_costs::SceneView::Main.present_ids();
        let _cost = frame_costs::stage(cpu_id);
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Present Encoder"),
        });
        self.present.encode(&mut encoder, &self.scene, display, self.pass_timer(gpu_id));
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Rebuild the window's scene target at a new size (`resize`).
    pub(super) fn resize_scene_target(&mut self, width: u32, height: u32) {
        self.scene = SceneTarget::new(&self.device, &self.present, width, height, self.scene.format());
    }

    /// Start an off-screen view: returns the view its scene passes draw
    /// into, a `width` x `height` scratch in the scene format (reused when
    /// the parked one fits), or `display` itself under the A/B switch. Pair
    /// with [`end_view_scene`](Self::end_view_scene). A second `begin` before
    /// the `end` is refused (logged) and draws straight into `display`, so a
    /// scratch in flight can never be swapped out from under its passes.
    pub fn begin_view_scene(&mut self, width: u32, height: u32, display: &wgpu::TextureView) -> wgpu::TextureView {
        if self.view_scene.state != ViewSceneState::Idle {
            log::warn!("begin_view_scene called while a view is already in flight; drawing it direct");
            return display.clone();
        }
        if self.present_bypassed() {
            self.view_scene.state = ViewSceneState::Direct;
            return display.clone();
        }
        let (w, h) = (width.max(1), height.max(1));
        let fits = self
            .view_scene
            .spare
            .as_ref()
            .is_some_and(|s| s.size() == (w, h) && s.format() == self.scene.format());
        if !fits {
            self.view_scene.spare = Some(SceneTarget::new(&self.device, &self.present, w, h, self.scene.format()));
        }
        self.view_scene.state = ViewSceneState::Scratch;
        self.view_scene.spare.as_ref().expect("made above").view().clone()
    }

    /// Finish an off-screen view: present its scratch into `display` (timed
    /// under `who`'s present ids), then keep the scratch parked (`retain`,
    /// the camera screens) or drop it (the one-off screenshot).
    pub fn end_view_scene(&mut self, display: &wgpu::TextureView, who: frame_costs::SceneView, retain: bool) {
        let state = std::mem::take(&mut self.view_scene.state);
        if state == ViewSceneState::Scratch {
            if let Some(scratch) = self.view_scene.spare.as_ref() {
                let (gpu_id, cpu_id) = who.present_ids();
                let _cost = frame_costs::stage(cpu_id);
                let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("View Present Encoder"),
                });
                self.present.encode(&mut encoder, scratch, display, self.pass_timer(gpu_id));
                self.queue.submit(std::iter::once(encoder.finish()));
            }
        }
        if !retain {
            self.view_scene.spare = None;
        }
    }

    /// Bytes held by the scene targets (the window's and a parked view
    /// scratch), for the Performance page's render-target slice.
    pub(super) fn scene_target_bytes(&self) -> u64 {
        frame_costs::texture_bytes(&self.scene.texture)
            + self.view_scene.spare.as_ref().map_or(0, |s| frame_costs::texture_bytes(&s.texture))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The present shader is compiled at renderer init, so a WGSL error would
    /// make the app unbootable while every static check stays green (the
    /// v0.782 class). Parse and validate it here, as ssao.rs does its own.
    #[test]
    fn present_shader_parses_and_validates() {
        let src = include_str!("../../assets/shaders/present.wgsl");
        let module = wgpu::naga::front::wgsl::parse_str(src)
            .unwrap_or_else(|e| panic!("present.wgsl failed to parse: {e}"));
        let mut validator = wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        );
        validator
            .validate(&module)
            .unwrap_or_else(|e| panic!("present.wgsl failed naga validation: {e:?}"));
        let entries: Vec<&str> = module.entry_points.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(entries, vec!["vs_main", "fs_main"]);
    }

    /// Until increment 3 the scene is drawn in the display format itself;
    /// that equality is what makes increments 1 and 2 bit-exact.
    #[test]
    fn scene_format_is_the_display_format_until_increment_3() {
        for f in [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
        ] {
            assert_eq!(scene_format_for(f), f);
        }
    }

    /// THE BIT-EXACT PROOF, on a real device (skips with a note without an
    /// adapter; the relay build has no `pollster`, so native only).
    ///
    /// Two halves, for each 8-bit format a surface can pick:
    /// 1. Round trip: upload known bytes into a scene target, present it into
    ///    a display texture, read that back. Every channel takes all 256
    ///    codes across a row, alpha included, so a lossy sRGB decode and
    ///    re-encode, a half-texel offset or a dropped alpha all fail here.
    /// 2. The architecture: draw an alpha-blended gradient over a cleared
    ///    colour into a scene target and present it, and draw the same thing
    ///    straight into a display texture; the two must match byte for byte.
    ///    That is the claim increments 1 and 2 rest on, made on the GPU.
    #[cfg(feature = "native")]
    #[test]
    fn present_pass_is_byte_exact_on_a_real_device() {
        let instance = wgpu::Instance::default();
        let Some(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            println!("no GPU adapter; skipping the present-pass round trip");
            return;
        };
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor { label: Some("present pass test"), ..Default::default() },
            None,
        ))
        .expect("device");
        // Not square and not a power of two in height, so a transposed or
        // offset read cannot pass by symmetry. 256 wide = 1024 bytes a row,
        // already a multiple of the 256-byte copy alignment.
        let (w, h) = (256u32, 83u32);
        for format in [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Rgba8Unorm,
        ] {
            let present = PresentPass::new(&device, format);
            let scene = SceneTarget::new(&device, &present, w, h, scene_format_for(format));

            // 1. Round trip of every code.
            let mut bytes = vec![0u8; (w * h * 4) as usize];
            for y in 0..h {
                for x in 0..w {
                    let i = ((y * w + x) * 4) as usize;
                    // Each channel is a bijection of x for a fixed y (odd
                    // multipliers and xor), so every row holds all 256 codes
                    // in every channel.
                    bytes[i] = x as u8;
                    bytes[i + 1] = (x * 5 + y) as u8;
                    bytes[i + 2] = (x as u8) ^ ((y * 7) as u8);
                    bytes[i + 3] = (x * 3 + y * 11) as u8;
                }
            }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &scene.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &bytes,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
            let out = display_texture(&device, w, h, format);
            let mut enc = device.create_command_encoder(&Default::default());
            present.encode(&mut enc, &scene, &out.create_view(&Default::default()), None);
            queue.submit(std::iter::once(enc.finish()));
            let back = read_back(&device, &queue, &out, w, h);
            let first_bad = bytes.iter().zip(&back).position(|(a, b)| a != b);
            assert!(
                first_bad.is_none(),
                "{format:?}: the present pass changed byte {} (pixel {}, channel {}): {} -> {}",
                first_bad.unwrap(),
                first_bad.unwrap() / 4,
                first_bad.unwrap() % 4,
                bytes[first_bad.unwrap()],
                back[first_bad.unwrap()]
            );

            // 2. A blended draw through the target equals the same draw direct.
            let gradient = gradient_pipeline(&device, format);
            let direct = display_texture(&device, w, h, format);
            for view in [scene.view().clone(), direct.create_view(&Default::default())] {
                let mut enc = device.create_command_encoder(&Default::default());
                {
                    let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("gradient"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.02, g: 0.3, b: 0.7, a: 1.0 }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    pass.set_pipeline(&gradient);
                    pass.draw(0..3, 0..1);
                }
                queue.submit(std::iter::once(enc.finish()));
            }
            let presented = display_texture(&device, w, h, format);
            let mut enc = device.create_command_encoder(&Default::default());
            present.encode(&mut enc, &scene, &presented.create_view(&Default::default()), None);
            queue.submit(std::iter::once(enc.finish()));
            let a = read_back(&device, &queue, &presented, w, h);
            let b = read_back(&device, &queue, &direct, w, h);
            let diffs = a.iter().zip(&b).filter(|(x, y)| x != y).count();
            assert_eq!(diffs, 0, "{format:?}: drawing through the scene target differs from drawing direct in {diffs} bytes");
            // And the gradient really drew (a blank frame would pass the
            // comparison above without proving anything).
            let distinct: std::collections::BTreeSet<u8> = a.iter().step_by(4).copied().collect();
            assert!(distinct.len() > 100, "{format:?}: the gradient drew only {} distinct codes", distinct.len());
        }
    }

    #[cfg(feature = "native")]
    fn display_texture(device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("present test display"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    #[cfg(feature = "native")]
    fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, tex: &wgpu::Texture, w: u32, h: u32) -> Vec<u8> {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("present test readback"),
            size: (w * h * 4) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit(std::iter::once(enc.finish()));
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::Maintain::Wait);
        let out = slice.get_mapped_range().to_vec();
        buffer.unmap();
        out
    }

    /// A full-screen gradient with a varying alpha, drawn with ordinary
    /// alpha blending: the blend the scene's transparent passes use.
    #[cfg(feature = "native")]
    fn gradient_pipeline(device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("present test gradient"),
            source: wgpu::ShaderSource::Wgsl(
                "@vertex fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
                    return vec4<f32>(f32((vi << 1u) & 2u) * 2.0 - 1.0, f32(vi & 2u) * 2.0 - 1.0, 0.0, 1.0);
                }
                @fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
                    let u = p.x / 256.0;
                    let v = p.y / 83.0;
                    return vec4<f32>(u, v, fract(u * 7.0 + v), 0.25 + 0.7 * fract(u * 3.0 - v));
                }"
                .into(),
            ),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("present test gradient"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }
}
