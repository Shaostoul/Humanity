//! THE SUN'S GLARE, ON THE GPU (2026-10-21): how much of the disc each view
//! can see, and the numbers the present pass adds the glare with.
//!
//! The physics (the disc, its light, the glare's shape) is `sun_disc.rs`; this
//! is the plumbing. Per view (the window, a camera screen, a hi-res capture):
//!
//! 1. Right after the celestial pass, [`Renderer::render_sun_vis_onto`] works
//!    out where the Sun is on that view's screen, decides whether its glare can
//!    reach the screen at all, and if so runs `sun_vis.wgsl` once: 64 taps of
//!    the celestial depth over the disc, the share nothing nearer than the Sun
//!    covers, into a 1x1 texture. It writes the present's glare uniform for
//!    this view and remembers the disc for step 2.
//! 2. Right before the present, the same taps over the scene pass's depth (the
//!    hull, walls, everything near) multiply that share, so a Sun behind the
//!    ship's wall throws no glare onto the wall.
//! 3. The present pass (present.wgsl `sun_glare_at`) adds the glare, then the
//!    uniform is reset so a later present with no step 1 adds none.
//!
//! Every queue write here lands before the submit that reads it: each pass
//! submits its own encoder, in frame order, and the views never interleave.

use super::camera::{self, Camera};
use super::{frame_costs, Renderer};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

/// The 1x1 target the visible share is written into.
pub(super) const SUN_VIS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R16Float;

/// The present pass's glare uniform (present.wgsl `GlareParams`).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Default)]
pub struct GlareParams {
    /// xyz: the unit direction to the Sun in view space, w: the drawn disc's
    /// angular radius.
    pub sun: [f32; 4],
    /// x: focal length in pixels, y/z: the image centre, w: 1 = glare on.
    pub view: [f32; 4],
    /// rgb: irradiance through air and clouds times lambda / (pi^3 D), w: the
    /// angle beyond which the glare is not evaluated.
    pub amp: [f32; 4],
}

/// The tap uniform (`sun_vis.wgsl` `SunVisUniform`).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug, PartialEq, Default)]
struct SunVisUniform {
    disc: [f32; 4],
    misc: [f32; 4],
}

/// What the frame knows about the Sun on the CPU side, set by lib.rs each
/// frame ([`Renderer::sun_frame`]) and read by every view of that frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SunFrame {
    /// Unit direction from the camera to the Sun's centre, render frame.
    pub dir: Vec3,
    /// Distance to the Sun, metres.
    pub dist_m: f64,
    /// Transmittance of the camera's air toward the Sun, per channel (the
    /// whole line of sight, unclamped: from orbit it is the limb's air).
    pub air: [f32; 3],
    /// Transmittance of the cloud deck toward the Sun (exp(-4 alpha), the
    /// god rays' own dimming, frame_lock::sun_cloud_alpha).
    pub cloud: f32,
    /// The CPU's estimate of how much of the disc is clear of the frame body
    /// (frame_lock::sun_disc_clear): the value of a tap off the screen.
    pub off_screen: f32,
    /// The frame body's air: its centre in the render frame and the top of
    /// its atmosphere shell, metres. `air`, `cloud` and `off_screen` are the
    /// PLAYER's, so a view whose camera is above that air (the Maps world
    /// layer looking back at Earth while the player stands on it) takes 1 for
    /// all three instead. None: no frame body, and all three are 1 already.
    pub air_top: Option<(Vec3, f32)>,
}

impl SunFrame {
    /// This frame's Sun as seen from a view whose camera is at `eye` (render
    /// frame): the player's air, cloud and horizon only when that camera is
    /// inside the same air.
    pub fn for_eye(&self, eye: Vec3) -> SunFrame {
        let mut f = *self;
        if let Some((centre, top)) = self.air_top {
            if (eye - centre).length() > top {
                f.air = [1.0; 3];
                f.cloud = 1.0;
                f.off_screen = 1.0;
            }
        }
        f
    }
}

/// The disc on a view's screen, kept from step 1 for step 2.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisDisc {
    /// Its centre in pixels (off the screen, or far off for a Sun behind).
    pub centre_px: [f32; 2],
    /// Its drawn radius in pixels (at least half a pixel).
    pub radius_px: f32,
}

/// The GPU side: the 1x1 target, the two tap pipelines (one replaces, one
/// multiplies), their uniforms.
pub struct SunVisPass {
    bgl: wgpu::BindGroupLayout,
    replace: wgpu::RenderPipeline,
    multiply: wgpu::RenderPipeline,
    uniform_celestial: wgpu::Buffer,
    uniform_scene: wgpu::Buffer,
}

impl SunVisPass {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Sun Vis Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../assets/shaders/sun_vis.wgsl").into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Sun Vis BGL"),
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
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<SunVisUniform>() as u64),
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sun Vis Pipeline Layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let make = |label: &str, blend: Option<wgpu::BlendState>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
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
                        format: SUN_VIS_FORMAT,
                        blend,
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
        };
        let replace = make("Sun Vis (celestial)", None);
        // out = src * 0 + dst * src: the scene share multiplies the celestial one.
        let multiply_blend = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::Src,
            operation: wgpu::BlendOperation::Add,
        };
        let multiply = make(
            "Sun Vis (scene)",
            Some(wgpu::BlendState { color: multiply_blend, alpha: multiply_blend }),
        );
        let uniform = |label: &str| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: std::mem::size_of::<SunVisUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Self {
            bgl,
            replace,
            multiply,
            uniform_celestial: uniform("Sun Vis Uniform (celestial)"),
            uniform_scene: uniform("Sun Vis Uniform (scene)"),
        }
    }

    /// Encode one tap pass over `depth_view` into `target` (the 1x1 share).
    /// `scene`: the multiply pipeline and the scene pass's depth (a tap is
    /// covered by anything drawn), else the replace pipeline and the
    /// celestial depth (covered by anything nearer than `threshold`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn encode(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        depth_view: &wgpu::TextureView,
        target: &wgpu::TextureView,
        disc: VisDisc,
        scene: bool,
        threshold: f32,
        off_screen: f32,
        timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
    ) {
        let buffer = if scene { &self.uniform_scene } else { &self.uniform_celestial };
        let u = SunVisUniform {
            disc: [disc.centre_px[0], disc.centre_px[1], disc.radius_px, threshold],
            misc: [off_screen, 0.0, 0.0, 0.0],
        };
        queue.write_buffer(buffer, 0, bytemuck::bytes_of(&u));
        // The depth view changes with resizes and views: bind it fresh, as the
        // god rays do (one tiny bind group a view).
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Sun Vis BG"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(depth_view) },
                wgpu::BindGroupEntry { binding: 1, resource: buffer.as_entire_binding() },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(if scene { "Sun Vis (scene)" } else { "Sun Vis (celestial)" }),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if scene { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(wgpu::Color::BLACK) },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes,
            occlusion_query_set: None,
        });
        pass.set_pipeline(if scene { &self.multiply } else { &self.replace });
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// The view-space direction, the screen position and the glare numbers for
/// one view, or None when the glare cannot reach the screen. Pure, so the
/// glare's placement is testable without a GPU.
///
/// `view`: the camera's view matrix (rotation used). `fov_y_rad`, `width`,
/// `height`: that view's projection and target. `aperture_m`: the glare's
/// aperture. `px_angle`: one pixel's angle (the disc floor).
#[allow(clippy::too_many_arguments)]
pub fn glare_for_view(
    sun: &SunFrame,
    view: Mat4,
    fov_y_rad: f32,
    width: u32,
    height: u32,
    aperture_m: f64,
) -> Option<(GlareParams, VisDisc)> {
    if sun.dir.length_squared() < 0.5 || width == 0 || height == 0 {
        return None;
    }
    let s = (view.transform_vector3(sun.dir)).normalize_or_zero();
    let f = (height as f32 * 0.5) / (fov_y_rad * 0.5).tan().max(1.0e-4);
    let (cx, cy) = (width as f32 * 0.5, height as f32 * 0.5);
    let px_angle = 1.0 / f as f64;
    // The drawn (floored) radius: the glare is shaped round the disc as drawn,
    // and the floor keeps the disc's light, so the irradiance is the true one.
    let r = super::sun_disc::disc_angular_radius(sun.dist_m, px_angle);
    let e = super::sun_disc::irradiance_at(sun.dist_m);
    let mut amp = [0.0f32; 3];
    for c in 0..3 {
        amp[c] = (e[c] as f64 * sun.air[c] as f64 * sun.cloud as f64 * super::sun_disc::glare_coeff(c, aperture_m)) as f32;
    }
    let amp_max = amp.iter().cloned().fold(0.0f32, f32::max);
    if amp_max <= 1.0e-12 {
        return None;
    }
    // Beyond this angle the glare is under 1e-4 scene units, a few thousandths
    // of a display level after the tone curve.
    let reach = (amp_max / 1.0e-4).cbrt().min(std::f32::consts::PI);
    // The angle from the view axis to the nearest screen corner, plus the
    // reach: a Sun further off than that throws no glare on the screen.
    let half_diag = ((cx * cx + cy * cy).sqrt() / f).atan();
    let off_axis = (-s.z).clamp(-1.0, 1.0).acos();
    if off_axis > half_diag + reach {
        return None;
    }
    // Where the disc lands on the screen (off it, or behind the camera, when
    // the glare reaches in from outside: the taps then all take the CPU value).
    let (px, py) = if s.z < -1.0e-4 {
        (cx + f * s.x / -s.z, cy - f * s.y / -s.z)
    } else {
        (-1.0e6, -1.0e6)
    };
    let params = GlareParams {
        sun: [s.x, s.y, s.z, r as f32],
        view: [f, cx, cy, 1.0],
        amp: [amp[0], amp[1], amp[2], reach],
    };
    Some((params, VisDisc { centre_px: [px.clamp(-1.0e6, 1.0e6), py.clamp(-1.0e6, 1.0e6)], radius_px: (r as f32 * f).max(0.5) }))
}

impl Renderer {
    /// Step 1 (module doc), for the view `camera` is drawing: call right after
    /// `render_celestial_onto`, before the orbit lines (which write depth), at
    /// every place the celestial pass runs.
    pub fn render_sun_vis_onto(&self, camera: &Camera) {
        let _cost = frame_costs::stage("cpu.sun_vis");
        self.sun_vis_disc.set(None);
        let off = GlareParams::default();
        let Some(sun) = self.sun_frame.filter(|_| self.sun_glare_on).map(|f| f.for_eye(camera.effective_position())) else {
            self.queue.write_buffer(self.present.glare_buffer(), 0, bytemuck::bytes_of(&off));
            return;
        };
        let (w, h) = (self.depth_texture.width(), self.depth_texture.height());
        let Some((params, disc)) =
            glare_for_view(&sun, camera.view_matrix(), camera.fov_degrees.to_radians(), w, h, self.sun_aperture_m)
        else {
            self.queue.write_buffer(self.present.glare_buffer(), 0, bytemuck::bytes_of(&off));
            return;
        };
        self.queue.write_buffer(self.present.glare_buffer(), 0, bytemuck::bytes_of(&params));
        // 1 Hz diagnostic (dev tooling, like the god rays' sun-uv line): where
        // the disc lands and what the glare is built from, so a capture can be
        // checked against the numbers that drew it.
        {
            use std::sync::atomic::{AtomicU64, Ordering};
            static LAST: AtomicU64 = AtomicU64::new(0);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if LAST.swap(now, Ordering::Relaxed) != now {
                log::info!(
                    "[SunVis] disc at ({:.1},{:.1}) r {:.2} px in {w}x{h} (fov {:.1} deg), true r {:.6} rad at {:.4} AU, glare amp {:.3e} reach {:.1} deg, air {:.3?} cloud {:.3} off-screen {:.2}",
                    disc.centre_px[0],
                    disc.centre_px[1],
                    disc.radius_px,
                    camera.fov_degrees,
                    super::sun_disc::angular_radius(sun.dist_m),
                    sun.dist_m / super::sun_disc::AU_M,
                    params.amp[1],
                    params.amp[3].to_degrees(),
                    sun.air,
                    sun.cloud,
                    sun.off_screen
                );
            }
        }
        // Covered by anything nearer than 99% of the Sun's distance (reverse-Z:
        // a nearer surface has the larger depth).
        let threshold = camera::celestial_depth_at((sun.dist_m * 0.99) as f32);
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Sun Vis Encoder") });
        self.sun_vis.encode(
            &self.device,
            &self.queue,
            &mut encoder,
            &self.depth_view,
            self.present.sun_vis_view(),
            disc,
            false,
            threshold,
            sun.off_screen.clamp(0.0, 1.0),
            // Claimed here, past every early return (the godrays lesson: a slot
            // claimed for a pass that then skips reads as a frozen cost).
            self.pass_timer("gpu.sun_vis"),
        );
        self.queue.submit(std::iter::once(encoder.finish()));
        self.sun_vis_disc.set(Some(disc));
    }

    /// Step 2 (module doc): encode the scene-depth taps into `encoder`, right
    /// before the present. Does nothing when step 1 found no glare.
    pub(super) fn encode_sun_vis_scene(&self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(disc) = self.sun_vis_disc.get() {
            self.sun_vis.encode(
                &self.device,
                &self.queue,
                encoder,
                &self.depth_view,
                self.present.sun_vis_view(),
                disc,
                true,
                0.0,
                1.0,
                None,
            );
        }
    }

    /// Step 3's reset: after a present, no glare until the next step 1.
    pub(super) fn end_sun_glare(&self) {
        if self.sun_vis_disc.take().is_some() {
            self.queue.write_buffer(self.present.glare_buffer(), 0, bytemuck::bytes_of(&GlareParams::default()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The tap shader is compiled at renderer init, so a WGSL error would make
    /// the app unbootable while every Rust check stays green (the v0.782
    /// class): parse and validate it here, as ssao.rs does its own.
    #[test]
    fn sun_vis_shader_parses_and_validates() {
        let src = include_str!("../../assets/shaders/sun_vis.wgsl");
        let module = wgpu::naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("sun_vis.wgsl failed to parse: {e}"));
        let mut validator = wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        );
        validator.validate(&module).unwrap_or_else(|e| panic!("sun_vis.wgsl failed naga validation: {e:?}"));
    }

    fn frame(dir: Vec3) -> SunFrame {
        SunFrame { dir, dist_m: super::super::sun_disc::AU_M, air: [1.0; 3], cloud: 1.0, off_screen: 1.0, air_top: None }
    }

    /// A Sun straight ahead lands in the middle of the screen at its true
    /// radius in pixels, and a Sun behind the camera throws no glare.
    #[test]
    fn the_glare_is_placed_where_the_sun_is() {
        let view = Mat4::look_at_rh(Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), Vec3::Y);
        let fov = 60.0_f32.to_radians();
        let (p, d) = glare_for_view(&frame(Vec3::new(0.0, 0.0, -1.0)), view, fov, 2560, 1440, 0.03).expect("ahead");
        assert!((d.centre_px[0] - 1280.0).abs() < 0.01 && (d.centre_px[1] - 720.0).abs() < 0.01, "{d:?}");
        let f = 720.0 / (30.0_f32.to_radians()).tan();
        let want = super::super::sun_disc::angular_radius(super::super::sun_disc::AU_M) as f32 * f;
        assert!((d.radius_px - want).abs() < 0.01, "radius {} px, want {want}", d.radius_px);
        assert!(p.view[3] == 1.0 && p.amp[1] > 0.0);
        assert!(glare_for_view(&frame(Vec3::new(0.0, 0.0, 1.0)), view, fov, 2560, 1440, 0.03).is_none(), "behind the camera");
        // Up and to the right: x right, y up in view space, y down in pixels.
        let (_, d) = glare_for_view(&frame(Vec3::new(0.2, 0.1, -1.0).normalize()), view, fov, 2560, 1440, 0.03).unwrap();
        assert!(d.centre_px[0] > 1280.0 && d.centre_px[1] < 720.0, "{d:?}");
    }

    /// The air, cloud and horizon numbers are the player's: a view inside the
    /// same air takes them, a view above that air (the Maps world layer
    /// looking back at Earth while the player stands on its night side) takes
    /// the plain Sun, so its glare is not the player's sunset.
    #[test]
    fn a_view_above_the_air_takes_the_plain_sun() {
        let mut f = frame(Vec3::new(0.0, 0.0, -1.0));
        f.air = [0.2, 0.03, 0.0];
        f.cloud = 0.1;
        f.off_screen = 0.0;
        f.air_top = Some((Vec3::new(0.0, -6.371e6, 0.0), 6.5e6));
        let near = f.for_eye(Vec3::ZERO); // standing on the ground under it
        assert_eq!((near.air, near.cloud, near.off_screen), ([0.2, 0.03, 0.0], 0.1, 0.0));
        let far = f.for_eye(Vec3::new(0.0, 1.0e8, 0.0)); // 100,000 km up
        assert_eq!((far.air, far.cloud, far.off_screen), ([1.0; 3], 1.0, 1.0));
        f.air_top = None;
        assert_eq!(f.for_eye(Vec3::new(0.0, 1.0e8, 0.0)).air, [0.2, 0.03, 0.0], "no frame body: the frame's own numbers");
    }

    /// The glare's strength is the Sun's light through the air and clouds: a
    /// thick cloud deck takes it away, a red sunset air reddens it.
    #[test]
    fn the_glare_is_the_light_that_gets_through() {
        let view = Mat4::IDENTITY;
        let mut sun = frame(Vec3::new(0.0, 0.0, -1.0));
        let (clear, _) = glare_for_view(&sun, view, 1.0, 1920, 1080, 0.03).unwrap();
        sun.air = [0.5, 0.1, 0.01];
        let (red, _) = glare_for_view(&sun, view, 1.0, 1920, 1080, 0.03).unwrap();
        assert!(red.amp[0] / clear.amp[0] > 0.49 && red.amp[2] / clear.amp[2] < 0.011);
        sun.cloud = 0.0;
        assert!(glare_for_view(&sun, view, 1.0, 1920, 1080, 0.03).is_none(), "no light, no glare");
    }
}
