//! THE SCENE TARGET and THE PRESENT PASS (HDR scene target; the plan of
//! record is docs/design/hdr-scene-target.md).
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
//! THE INCREMENTS, AS BUILT.
//! 1 and 2: the target in the display format and a bit-exact passthrough
//!    (proved on the GPU, and by a byte-identical 3840x2160 capture).
//! 3: [`scene_format_for`] returns `Rgba16Float`. The shaders are untouched,
//!    so they write the same values, but every blend now happens at float
//!    precision and the scene is quantised ONCE, at the present, instead of
//!    after every pass. The present clamps to 0..1 first (an 8-bit target
//!    clamped every write; a float one keeps additive sums above 1).
//! 4: the ONE dither, in the present pass, before the 8-bit write
//!    (assets/shaders/present.wgsl `dither`). The aurora's own dither is gone.
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
//! THE A/B SWITCHES (dev, the rig's showcase keys; delete at increment 6):
//! * `scene_format` ("display" or "hdr", renderer/scene_format_ab.rs):
//!   rebuilds the scene target and every scene pipeline in the display
//!   format (increments 1 and 2) or in Rgba16Float, so increment 3 can be
//!   measured in one boot at one park.
//! * `present_direct` draws the scene straight into the display target, the
//!   pre-2026-09-27 path. It only acts while the scene and display formats
//!   are equal (a scene pipeline cannot draw into another format), so from
//!   increment 3 on it needs `scene_format: display` first.
//! * `present_dither` ("0" off, "1" on, the default): the high-frequency
//!   gates pin it off so they read the scene, not the dither.

use super::{frame_costs, Renderer};
use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// The format the scene is drawn in, given the display's format. Increment
/// 3 (2026-09-27): always `Rgba16Float`, a blendable, filterable core format
/// (so `Limits::default()` is enough), 8 bytes a pixel. The display format
/// is taken so the signature did not change from increments 1 and 2, when
/// this returned it; the A/B switch still can (`scene_format_ab.rs`).
pub fn scene_format_for(_display: wgpu::TextureFormat) -> wgpu::TextureFormat {
    wgpu::TextureFormat::Rgba16Float
}

/// How many codes the present's write quantises to, which is what the
/// dither is scaled by: 255 for the 8-bit formats a surface picks, 1023 for
/// 10-bit, 0 (no dither) for a float output, which does not quantise.
pub fn dither_levels(output: wgpu::TextureFormat) -> f32 {
    use wgpu::TextureFormat as F;
    match output {
        F::Rgba8Unorm | F::Rgba8UnormSrgb | F::Bgra8Unorm | F::Bgra8UnormSrgb => 255.0,
        F::Rgb10a2Unorm => 1023.0,
        _ => 0.0,
    }
}

/// The present pass's uniform (binding 1). See present.wgsl for the lanes.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq)]
pub(super) struct PresentParams {
    /// x: clamp to 0..1. y: dither levels (0 = off). z: output encodes sRGB.
    /// w: reserved for the tonemap.
    pub(super) flags: [f32; 4],
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
    /// The format of the scene targets this pass reads (all of one frame's
    /// targets share it): the clamp is on exactly when it is not the output.
    scene_format: wgpu::TextureFormat,
    /// The one dither (increment 4). On by default; the showcase key
    /// `present_dither` turns it off for the high-frequency gates.
    dither: bool,
}

impl PresentPass {
    /// Build the pass for display targets in `output_format` (the swapchain's
    /// format: camera screens and hi-res capture targets use it too), reading
    /// scene targets in `scene_format`. The dither starts on.
    pub fn new(device: &wgpu::Device, output_format: wgpu::TextureFormat, scene_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Present Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../assets/shaders/present.wgsl").into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Present BGL"),
            entries: &[
                // The scene target, read with textureLoad (no sampler). Not
                // filterable, so any float format can bind here, the
                // Rgba16Float of increment 3 included.
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
        let flags = present_flags(output_format, scene_format, true);
        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Present Params"),
            contents: bytemuck::bytes_of(&flags),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
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
        Self { bind_group_layout, pipeline, params, output_format, scene_format, dither: true }
    }

    /// The display format this pass writes.
    pub fn output_format(&self) -> wgpu::TextureFormat {
        self.output_format
    }

    /// Whether the one dither is on.
    pub fn dither(&self) -> bool {
        self.dither
    }

    /// Turn the dither on or off (the showcase key `present_dither`).
    pub fn set_dither(&mut self, queue: &wgpu::Queue, on: bool) {
        self.dither = on;
        self.upload(queue);
    }

    /// Read scene targets in `scene_format` from now on (the A/B switch).
    pub(super) fn set_scene_format(&mut self, queue: &wgpu::Queue, scene_format: wgpu::TextureFormat) {
        self.scene_format = scene_format;
        self.upload(queue);
    }

    fn upload(&self, queue: &wgpu::Queue) {
        let flags = present_flags(self.output_format, self.scene_format, self.dither);
        queue.write_buffer(&self.params, 0, bytemuck::bytes_of(&flags));
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

/// The present uniform for an output format, a scene format and the dither
/// switch. The clamp is on whenever the formats differ: a scene target in
/// the output's own format holds nothing outside 0..1 (that is the
/// increments 1 and 2 path, which must stay bit-exact), a float one can.
pub(super) fn present_flags(
    output: wgpu::TextureFormat,
    scene: wgpu::TextureFormat,
    dither: bool,
) -> PresentParams {
    let clamp = if scene == output { 0.0 } else { 1.0 };
    let levels = if dither { dither_levels(output) } else { 0.0 };
    let srgb = if output.is_srgb() { 1.0 } else { 0.0 };
    PresentParams { flags: [clamp, levels, srgb, 0.0] }
}

/// The off-screen views' scratch target and whether a view is being drawn.
/// Exactly one view is in flight at a time (`render_view_onto` is not
/// re-entrant), so one spare is the whole cache; two camera screens of
/// different sizes alternate re-creating it, the same trade `ViewDepth` made.
#[derive(Default)]
pub(super) struct ViewScene {
    pub(super) spare: Option<SceneTarget>,
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
    /// allocated in: `Rgba16Float` since increment 3 (the display format
    /// under the `scene_format: display` A/B arm).
    pub fn scene_format(&self) -> wgpu::TextureFormat {
        self.scene.format()
    }

    /// Whether the one dither is on (showcase `present_dither`).
    pub fn present_dither(&self) -> bool {
        self.present.dither()
    }

    /// Turn the one dither on or off (showcase `present_dither`).
    pub fn set_present_dither(&mut self, on: bool) {
        self.present.set_dither(&self.queue, on);
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
#[path = "scene_target_tests.rs"]
mod tests;
