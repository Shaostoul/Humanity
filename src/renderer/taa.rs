//! TEMPORAL ANTI-ALIASING, rung 1a of the renderer roadmap (2026-11-13; the plan of record is
//! docs/design/renderer-roadmap.md sections 2, 3 and 12, with the operator's answers of 2026-11-12:
//! Temporal on by default, native resolution, depth of field and motion blur off).
//!
//! WHAT IT DOES. The engine had no anti-aliasing in the 3D world: every edge stair-stepped and
//! everything thinner than a pixel (hair, grass, leaves, far railings, highlights on cloth)
//! shimmered as it moved. Now every frame is drawn a fraction of a pixel to one side (the JITTER,
//! a Halton 2,3 sequence of eight, added in `vs_main` after the view-projection), every opaque
//! pass writes where each surface was on the last frame (the MOTION target, beside the colour),
//! and a RESOLVE after the transparent pass blends the frame into a history carried from the
//! frame before, fetched through the motion and clipped to what the pixel's neighbourhood holds
//! now (assets/shaders/taa.wgsl, which names the papers). A contrast-adaptive SHARPEN writes the
//! result back into the scene target, so the overlay, the lines and the particles, which write
//! no motion and would trail, draw over the resolved picture unjittered, and the present copies
//! it to the screen as before.
//!
//! WHAT WRITES MOTION (section 2 of the plan): every opaque megashader PSO, through
//! [`scene_targets`] (`tests` below and `pipeline::PSO_REGISTRY`'s state column pin it):
//! - the camera, each pass with ITS previous view-projection (the scene pass clears depth, so one
//!   end-of-frame depth buffer cannot reproject the whole picture);
//! - the floating origin: on a planet render space is centred on the eye and turned with the
//!   planet, so every frame's previous matrices are re-expressed in this frame's render space
//!   through the locked body's placement ([`frame_delta`]); otherwise every pixel would streak;
//! - wind-moved vegetation (classes 19 to 23): the wind evaluated at the previous clock as well;
//! - the ocean: its displacement at the previous clock (the depth-writing water PSO);
//! - an object that moved: its previous model matrix, kept by mesh for meshes drawn once a frame
//!   ([`MotionBook`]); the bodies' skinning (near, middle) is rung 1a's second step (the doc says
//!   what is built).
//! The motion target is cleared each frame to the SKY SENTINEL (w = 0): those pixels reproject
//! by the camera's turn alone, exact for the stars, the sky and the clouds.
//!
//! THE REUSE SWITCH (CLAUDE.md's no-caches rule, 2026-10-30): the history is data kept across
//! frames, so with `humanity_core::reuse::enabled()` false it is discarded every frame and the
//! picture aliases exactly as Off does: the A/B the Dev page's "Turn off all reuse" exists for.
//! The per-object motion book is reuse too and is emptied with it.
//!
//! WHAT IS NOT JITTERED: the shadow passes (their own camera buffers), the cloud octahedral pass,
//! the off-screen views (camera screens and the hi-res screenshot, which do not use TAA: the
//! screens render at 10 Hz, the capture is supersampled already), and everything drawn after the
//! resolve.
//!
//! THE DETERMINISTIC CAPTURE MODE (section 3.9): a rig capture resets the history and waits for
//! [`SETTLE_FRAMES`] resolved frames before it reads the picture, so a capture does not depend on
//! the frames before it (`Renderer::taa_settled`).

use super::*;
use glam::{DMat4, DQuat, DVec3, Vec2, Vec4};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// The motion target's format: xy = UV offset from last frame, z = the responsive mask, w = the
/// sky sentinel (0 where nothing was drawn). Rgba16Float rather than the plan's Rg16Float: the
/// mask and the sentinel ride the same target instead of a third attachment (8 bytes a pixel; 30
/// MB at 2560 x 1440).
pub const MOTION_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The history's format: the resolved colour, and in alpha the history's weight (the Dev view).
pub const HISTORY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// How many jitter positions the sequence cycles through (Halton 2,3, indices 1 to 8).
pub const JITTER_PHASES: u32 = 8;
/// The Anti-aliasing choices in Settings > Graphics, by id (words: `graphics.anti_aliasing_choice.<id>`).
/// "edges" (SMAA 1x, the Simplified mode) is rung 1c.
pub const MODES: &[&str] = &["off", "temporal"];
/// The operator's answer of 2026-11-12: Temporal on by default.
pub const DEFAULT_MODE: &str = "temporal";
/// A moderate sharpening by default (the plan's recommendation 2).
pub const DEFAULT_SHARPENING: f32 = 0.35;
/// The smallest share of the new frame in the blend: with eight jitter phases the history
/// averages about sixteen frames.
pub const BLEND_FLOOR: f32 = 1.0 / 16.0;
/// A camera that moves farther than this in one frame cut: the history is thrown away (a
/// teleport, a vantage jump, world entry).
pub const CUT_JUMP_M: f32 = 50.0;
/// Resolved frames a deterministic capture waits for after its reset: two full jitter cycles.
pub const SETTLE_FRAMES: u32 = 16;

/// The Dev page's view (0 the picture, 1 the motion target, 2 the history's weight, 3 the
/// responsive mask), a process-wide switch like the reuse switch: the page writes it, the resolve
/// reads it each frame.
static DEV_VIEW: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// The Dev page's TAA view (see `DEV_VIEW`).
pub fn dev_view() -> u32 {
    DEV_VIEW.load(std::sync::atomic::Ordering::Relaxed)
}

/// Set the Dev page's TAA view (the rig's showcase key `taa_view` too).
pub fn set_dev_view(v: u32) {
    DEV_VIEW.store(v.min(3), std::sync::atomic::Ordering::Relaxed);
}

/// Whether `id` names an anti-aliasing mode.
pub fn is_mode(id: &str) -> bool {
    MODES.contains(&id)
}

/// The radical inverse of `index` in `base`: the Halton sequence (1-based index; 0 gives 0).
pub fn halton(mut index: u32, base: u32) -> f32 {
    let mut f = 1.0_f32;
    let mut r = 0.0_f32;
    while index > 0 {
        f /= base as f32;
        r += f * (index % base) as f32;
        index /= base;
    }
    r
}

/// This frame's jitter in PIXELS, each axis in (-0.5, 0.5): +x to the right, +y down the screen.
/// Halton (2, 3) at indices 1 to 8, so the first offset is (0, -1/6).
pub fn jitter_px(frame: u32) -> Vec2 {
    let i = frame % JITTER_PHASES + 1;
    Vec2::new(halton(i, 2) - 0.5, halton(i, 3) - 0.5)
}

/// A pixel offset as the clip-space shift the vertex shader adds (`clip.xy += j * clip.w`): two
/// NDC units span the target, and NDC y points up while pixel rows go down.
pub fn jitter_ndc(px: Vec2, width: u32, height: u32) -> Vec2 {
    Vec2::new(2.0 * px.x / width.max(1) as f32, -2.0 * px.y / height.max(1) as f32)
}

/// THE CPU TWIN of the shader's `taa_motion` (00-bindings-vertex.wgsl): the motion a surface
/// writes, from its unjittered clip position now and on the last frame, as a UV offset
/// (now - then; UV x right, y down). A point behind the camera last frame gets an offset that
/// leaves the screen, so its history is rejected. Pinned to the WGSL by `the_shader_motion_is_the_twin`.
pub fn motion_uv(cur_clip: Vec4, prev_clip: Vec4) -> Vec2 {
    if prev_clip.w <= 1.0e-6 || cur_clip.w <= 1.0e-6 {
        return Vec2::new(2.0, 2.0);
    }
    let cur = Vec2::new(cur_clip.x, cur_clip.y) / cur_clip.w;
    let prev = Vec2::new(prev_clip.x, prev_clip.y) / prev_clip.w;
    (cur - prev) * Vec2::new(0.5, -0.5)
}

/// Whether last frame's history may be used this frame. Every condition that makes it wrong
/// discards it; the reuse switch off discards it every frame (CLAUDE.md, 2026-10-30).
pub fn history_kept(reuse_on: bool, had_history: bool, same_size: bool, reset_asked: bool, jumped: bool, was_on: bool) -> bool {
    reuse_on && had_history && same_size && !reset_asked && !jumped && was_on
}

/// The transform that takes a point in THIS frame's render space to LAST frame's, for a world
/// placed by a body frame (`render = rot * body + render_off`, the celestial loop's placement):
/// `old = T(off_old) * R_old * R_new^-1 * T(-off_new) * new`. In f64 (the offsets are planet radii)
/// and cast at the end, where every term is small.
pub fn frame_delta(prev: (DVec3, DQuat), cur: (DVec3, DQuat)) -> Mat4 {
    let (off_o, r_o) = prev;
    let (off_n, r_n) = cur;
    let m = DMat4::from_translation(off_o) * DMat4::from_quat(r_o * r_n.inverse()) * DMat4::from_translation(-off_n);
    m.as_mat4()
}

/// THE PER-OBJECT MOTION BOOK: last frame's model matrix of each mesh drawn exactly once, so a
/// mesh that moved (a vehicle, a held thing, a person's region mesh) writes its own motion. A mesh
/// drawn more than once a frame (furniture instances, terrain) has no identity across frames and
/// is taken as still (its motion is the camera's). Reuse data: emptied whenever the history is
/// discarded, so it obeys the reuse switch.
#[derive(Default)]
pub struct MotionBook {
    last: HashMap<usize, Mat4>,
    now: HashMap<usize, (Mat4, u32)>,
}

impl MotionBook {
    /// The model matrix `mesh` had last frame, in this frame's render space (`new_from_old` takes
    /// last frame's render space to this one's), or `model` itself when it is not known. Records
    /// this frame's `model` for the next frame.
    pub fn prev_model(&mut self, mesh: usize, model: Mat4, new_from_old: Mat4) -> Mat4 {
        let e = self.now.entry(mesh).or_insert((model, 0));
        e.0 = model;
        e.1 += 1;
        match self.last.get(&mesh) {
            Some(m) => new_from_old * *m,
            None => model,
        }
    }

    /// Frame boundary: what was drawn once becomes last frame's; `keep` false forgets everything
    /// (a cut, the reuse switch off).
    pub fn turn(&mut self, keep: bool) {
        self.last.clear();
        if keep {
            for (mesh, (m, n)) in self.now.drain() {
                if n == 1 {
                    self.last.insert(mesh, m);
                }
            }
        }
        self.now.clear();
    }
}

/// What this frame's passes stamp into the camera uniform (`Renderer::taa_camera`).
#[derive(Clone, Copy, Default)]
pub(super) struct FrameNow {
    /// Jitter and motion apply (the main view, between `taa_begin_frame` and the resolve).
    pub live: bool,
    pub jitter_ndc: [f32; 2],
    /// Last frame's unjittered view-projections, re-expressed in this frame's render space.
    pub prev_scene: Mat4,
    pub prev_cel: Mat4,
    /// This frame's unjittered celestial view-projection (the sky reprojection).
    pub cur_cel: Mat4,
    /// This frame's render space from last frame's, for the scene list and the celestial list
    /// (the motion book).
    pub scene_new_from_old: Mat4,
    pub cel_new_from_old: Mat4,
    /// The history is discarded this frame.
    pub reset: bool,
    /// The celestial pass's animation clock last frame (the wind and the waves at the previous
    /// time); `has_clock` false until there is one.
    pub prev_clock: f32,
    pub has_clock: bool,
}

/// Last frame's camera, for the next frame's previous matrices.
#[derive(Clone, Copy)]
struct LastFrame {
    scene: Mat4,
    cel: Mat4,
    eye: Vec3,
    size: (u32, u32),
}

/// The anti-aliasing state the renderer owns.
pub struct Taa {
    /// Settings > Graphics > Anti-aliasing, by id (`MODES`).
    pub mode: String,
    /// The rig's showcase pin `taa` ("off" or "temporal"); None = the setting.
    pub pin: Option<String>,
    /// Settings > Graphics > Sharpening, 0..1.
    pub sharpen: f32,
    frame: u32,
    history: Option<[(wgpu::Texture, wgpu::TextureView); 2]>,
    cur: Cell<usize>,
    resolve_pso: Option<wgpu::ComputePipeline>,
    sharpen_pso: Option<(wgpu::RenderPipeline, wgpu::TextureFormat)>,
    params: Option<wgpu::Buffer>,
    sampler: Option<wgpu::Sampler>,
    last: Option<LastFrame>,
    body_now: Option<(DVec3, DQuat)>,
    body_last: Option<(DVec3, DQuat)>,
    /// Last frame's home offset (`taa_begin_frame`).
    home_last: Option<Vec3>,
    reset_asked: bool,
    was_on: bool,
    pub(super) now: Cell<FrameNow>,
    /// The celestial clock this frame (recorded by the celestial pass, rotated at the next begin).
    pub(super) clock_now: Cell<Option<f32>>,
    clock_last: Option<f32>,
    /// Resolved frames since the last reset (the deterministic capture waits on it).
    settled: Cell<u32>,
    /// Whether this frame's motion target was cleared yet (the celestial pass clears it; a frame
    /// without one clears it in the scene pass).
    pub(super) motion_cleared: Cell<bool>,
    /// The window's `motion_cleared` while an off-screen view draws (renderer/view_depth.rs).
    pub(super) saved_cleared: Cell<bool>,
    pub(super) book: RefCell<MotionBook>,
}

impl Default for Taa {
    fn default() -> Self {
        Self {
            mode: DEFAULT_MODE.to_string(),
            pin: None,
            sharpen: DEFAULT_SHARPENING,
            frame: 0,
            history: None,
            cur: Cell::new(0),
            resolve_pso: None,
            sharpen_pso: None,
            params: None,
            sampler: None,
            last: None,
            body_now: None,
            body_last: None,
            home_last: None,
            reset_asked: true,
            was_on: false,
            now: Cell::new(FrameNow::default()),
            clock_now: Cell::new(None),
            clock_last: None,
            settled: Cell::new(0),
            motion_cleared: Cell::new(false),
            saved_cleared: Cell::new(false),
            book: RefCell::new(MotionBook::default()),
        }
    }
}

impl Taa {
    /// The mode in force: the rig's pin, else the setting.
    pub fn mode_in_force(&self) -> &str {
        self.pin.as_deref().unwrap_or(&self.mode)
    }

    /// Video memory the history holds (two frames), for the Performance page and the Settings
    /// cost line (`vram.taa_history`).
    pub fn history_bytes(&self) -> u64 {
        self.history
            .as_ref()
            .map_or(0, |h| h.iter().map(|(t, _)| crate::renderer::gpu_costs::texture_bytes(t)).sum())
    }

    /// The bytes the history WOULD take at `width` x `height` (the Settings line, before it exists).
    pub fn history_bytes_at(width: u32, height: u32) -> u64 {
        2 * width as u64 * height as u64 * 8
    }
}

/// The colour targets of a megashader PSO that draws into the scene: the scene colour, and the
/// motion target. `writes_depth` decides the motion: an opaque or depth-writing PSO writes all of
/// it (REPLACE), a blended one writes only the responsive mask (z), as the larger of what is there
/// and its own coverage. EVERY scene PSO takes its targets from here (`pipeline.rs` tests).
pub fn scene_targets(scene_format: wgpu::TextureFormat, blend: wgpu::BlendState, writes_depth: bool) -> [Option<wgpu::ColorTargetState>; 2] {
    let motion = if writes_depth {
        wgpu::ColorTargetState { format: MOTION_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL }
    } else {
        let max = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Max };
        wgpu::ColorTargetState { format: MOTION_FORMAT, blend: Some(wgpu::BlendState { color: max, alpha: max }), write_mask: wgpu::ColorWrites::BLUE }
    };
    [Some(wgpu::ColorTargetState { format: scene_format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL }), Some(motion)]
}

/// How a scene pass opens the motion target (`Renderer::motion_attachment`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionLoad {
    Clear,
    ClearIfFirst,
    Load,
}

/// One motion target texture (the window's, or an off-screen view's scratch).
pub(super) fn create_motion_texture(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("TAA Motion Target"),
        size: wgpu::Extent3d { width: width.max(1), height: height.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: MOTION_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// The params uniform (taa.wgsl `TaaParams`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TaaParams {
    size: [f32; 4],
    ctl: [f32; 4],
    extra: [f32; 4],
    pad: [f32; 4],
    sky_reproj: [[f32; 4]; 4],
}

const SHADER: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/assets/shaders/taa.wgsl"));

impl Renderer {
    /// Whether temporal anti-aliasing runs this frame: chosen, and possible (the scene drawn into
    /// the float scene target, not straight into the display under the present A/B switch).
    pub fn taa_on(&self) -> bool {
        self.taa.mode_in_force() == "temporal" && self.scene_format() == wgpu::TextureFormat::Rgba16Float
    }

    /// Ask for the history to be thrown away on the next frame (a camera cut the renderer cannot
    /// see: a vantage jump, world entry) and the deterministic capture's count restarted.
    pub fn taa_reset(&mut self) {
        self.taa.reset_asked = true;
    }

    /// Resolved frames since the last reset: a deterministic capture waits for [`SETTLE_FRAMES`].
    /// Always "settled" while TAA is off (nothing to wait for).
    pub fn taa_settled(&self) -> bool {
        !self.taa_on() || self.taa.settled.get() >= SETTLE_FRAMES
    }

    /// The celestial loop's placement of the body the player is locked to, this frame
    /// (engine/planet_build.rs `note_body`): the floating origin's frame-to-frame change.
    pub fn taa_note_body(&mut self, render_off: DVec3, rot: DQuat) {
        self.taa.body_now = Some((render_off, rot));
    }

    /// Start a frame: this frame's jitter, last frame's matrices in this frame's render space, and
    /// whether the history survives. `home_offset` is where the home frame sits in render space this
    /// frame (the engine's station offset: the scene list is the home's contents moved by it), so the
    /// scene pass's last frame is re-expressed by how far it moved; the people in the scene list who
    /// stand in the planet's render space ride the motion book (their last model, carried the same
    /// way), not this matrix. Measured 2026-11-13: taking the planet's frame delta for the whole
    /// scene list (whenever the player was frame-locked and not riding) gave the parked crowd
    /// vantage's walls 4 pixels of motion they did not have.
    /// render space (the player on the ground, not aboard): it moves with the body frame too.
    pub fn taa_begin_frame(&mut self, camera: &Camera, home_offset: Vec3) {
        let on = self.taa_on();
        let size = (self.config.width.max(1), self.config.height.max(1));
        let reuse = humanity_core::reuse::enabled();
        // The floating origin: last frame's render space from this frame's.
        let delta = match (self.taa.body_last, self.taa.body_now) {
            (Some(p), Some(c)) => Some(frame_delta(p, c)),
            _ => None,
        };
        let body_changed = self.taa.body_last.is_some() != self.taa.body_now.is_some();
        let cel_old_from_new = delta.unwrap_or(Mat4::IDENTITY);
        // The home: render = home + offset, so last frame's render space is this one's moved by
        // (offset then - offset now).
        let scene_old_from_new = match self.taa.home_last {
            Some(h) => Mat4::from_translation(h - home_offset),
            None => Mat4::IDENTITY,
        };
        self.taa.home_last = Some(home_offset);
        let cur_scene = camera.view_projection_matrix();
        let cur_cel = camera.celestial_projection() * camera.view_matrix();
        let eye = camera.effective_position();
        let jumped = self.taa.last.is_some_and(|l| {
            let eye_then = scene_old_from_new.inverse().transform_point3(l.eye);
            (eye - eye_then).length() > CUT_JUMP_M
        }) || body_changed;
        let kept = history_kept(
            reuse,
            self.taa.last.is_some() && self.taa.history.is_some(),
            self.taa.last.is_some_and(|l| l.size == size),
            self.taa.reset_asked,
            jumped,
            self.taa.was_on,
        );
        let (prev_scene, prev_cel) = match self.taa.last {
            Some(l) if kept => (l.scene * scene_old_from_new, l.cel * cel_old_from_new),
            _ => (cur_scene, cur_cel),
        };
        self.taa.book.borrow_mut().turn(kept && on);
        // The bodies' previous palette (renderer/skinning.rs): the middle tier's placement rows ride
        // the scene list's render space.
        self.skin_upload_prev(scene_old_from_new.inverse(), kept && on);
        if !kept {
            self.taa.settled.set(0);
        }
        self.taa.frame = self.taa.frame.wrapping_add(1);
        let j = if on { jitter_ndc(jitter_px(self.taa.frame), size.0, size.1) } else { Vec2::ZERO };
        let prev_clock = self.taa.clock_now.take().or(self.taa.clock_last);
        self.taa.clock_last = prev_clock;
        self.taa.now.set(FrameNow {
            live: on,
            jitter_ndc: [j.x, j.y],
            prev_scene,
            prev_cel,
            cur_cel,
            scene_new_from_old: scene_old_from_new.inverse(),
            cel_new_from_old: cel_old_from_new.inverse(),
            reset: !kept,
            prev_clock: prev_clock.unwrap_or(0.0),
            has_clock: prev_clock.is_some() && kept,
        });
        self.taa.motion_cleared.set(false);
        self.taa.last = Some(LastFrame { scene: cur_scene, cel: cur_cel, eye, size });
        self.taa.body_last = self.taa.body_now.take();
        self.taa.reset_asked = false;
        self.taa.was_on = on;
        if on {
            self.ensure_taa_resources(size);
        } else if self.taa.history.is_some() {
            // Off: give the history's memory back (Settings says what Off costs: nothing).
            self.taa.history = None;
        }
    }

    /// Stamp this frame's jitter and previous matrices into a camera uniform about to be written
    /// for a colour pass. `celestial` picks the celestial pass's previous matrix. Off-screen views
    /// and every pass after the resolve get none (no jitter, no motion).
    pub(super) fn taa_camera(&self, u: &mut camera::CameraUniforms, celestial: bool) {
        let f = self.taa.now.get();
        u.prev_view_proj = u.view_proj;
        u.taa = [0.0, 0.0, 0.0, u.sun_color[3]];
        if !f.live || self.offscreen_view_active() {
            return;
        }
        u.prev_view_proj = if celestial { f.prev_cel } else { f.prev_scene }.to_cols_array_2d();
        u.taa = [f.jitter_ndc[0], f.jitter_ndc[1], 1.0, u.sun_color[3]];
    }

    /// The celestial pass's colour uniform (`render_celestial_onto`): the celestial camera with this
    /// frame's jitter, the celestial pass's previous view-projection and the animation clock LAST
    /// frame (the wind and the waves are evaluated at both clocks), the clock being the one the pass
    /// stamps into sun_color.w (`time_s`, or the dev cloud-clock pin).
    pub(super) fn celestial_colour_uniform(&self, camera: &Camera, time_s: f32) -> camera::CameraUniforms {
        let clock = if self.cloud_clock_pin >= 0.0 { self.cloud_clock_pin } else { time_s };
        let mut u = camera.celestial_uniforms();
        self.taa_camera(&mut u, true);
        u.taa[3] = self.taa_celestial_clock(clock);
        u
    }

    /// The celestial pass's clock this frame, stamped as the previous clock (taa.w) and recorded
    /// for the next frame. `clock` is what the pass writes to sun_color.w.
    pub(super) fn taa_celestial_clock(&self, clock: f32) -> f32 {
        if self.offscreen_view_active() {
            return clock;
        }
        self.taa.clock_now.set(Some(clock));
        let f = self.taa.now.get();
        if f.live && f.has_clock { f.prev_clock } else { clock }
    }

    /// The previous model matrix for one object about to be uploaded (`upload_object_uniforms`):
    /// recorded only for the main view's colour passes (`list`: Some(celestial?)), the object's
    /// own model otherwise.
    pub(super) fn taa_prev_model(&self, mesh: usize, model: Mat4, list: Option<bool>) -> Mat4 {
        let f = self.taa.now.get();
        match list {
            Some(celestial) if f.live && !self.offscreen_view_active() => {
                let nfo = if celestial { f.cel_new_from_old } else { f.scene_new_from_old };
                self.taa.book.borrow_mut().prev_model(mesh, model, nfo)
            }
            _ => model,
        }
    }

    /// The motion attachment a scene pass adds beside its colour. `Clear` (the celestial opaque
    /// pass, the first of every view) clears it to the sky sentinel; `ClearIfFirst` (the scene
    /// opaque pass) clears it only when no pass did yet this view (a frame with no celestial
    /// pass); `Load` keeps what is there.
    pub(super) fn motion_attachment(&self, how: MotionLoad) -> Option<wgpu::RenderPassColorAttachment<'_>> {
        let clear = match how {
            MotionLoad::Clear => {
                self.taa.motion_cleared.set(true);
                true
            }
            MotionLoad::ClearIfFirst => !self.taa.motion_cleared.replace(true),
            MotionLoad::Load => false,
        };
        Some(wgpu::RenderPassColorAttachment {
            view: &self.motion_view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: if clear { wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT) } else { wgpu::LoadOp::Load },
                store: wgpu::StoreOp::Store,
            },
        })
    }

    /// The history textures, the two pipelines, the params buffer and the sampler, made when TAA
    /// first runs and the history remade at a new size.
    fn ensure_taa_resources(&mut self, size: (u32, u32)) {
        let fits = self.taa.history.as_ref().is_some_and(|h| h[0].0.width() == size.0 && h[0].0.height() == size.1);
        if !fits {
            let make = |label: &str| {
                let t = self.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: HISTORY_FORMAT,
                    usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                });
                let v = t.create_view(&wgpu::TextureViewDescriptor::default());
                (t, v)
            };
            self.taa.history = Some([make("TAA History A"), make("TAA History B")]);
            let mut f = self.taa.now.get();
            f.reset = true;
            self.taa.now.set(f);
            self.taa.settled.set(0);
        }
        if self.taa.resolve_pso.is_none() || self.taa.params.is_none() {
            let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("TAA Shader"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
            self.taa.resolve_pso = Some(self.device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("TAA Resolve Pipeline"),
                layout: None,
                module: &module,
                entry_point: Some("cs_resolve"),
                compilation_options: Default::default(),
                cache: None,
            }));
            self.taa.params = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("TAA Params"),
                size: std::mem::size_of::<TaaParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.taa.sampler = Some(self.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("TAA History Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }));
        }
        let fmt = self.scene_format();
        if self.taa.sharpen_pso.as_ref().is_none_or(|(_, f)| *f != fmt) {
            let module = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("TAA Sharpen Shader"),
                source: wgpu::ShaderSource::Wgsl(SHADER.into()),
            });
            let pso = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("TAA Sharpen Pipeline"),
                layout: None,
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_full"), buffers: &[], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_sharpen"),
                    targets: &[Some(wgpu::ColorTargetState { format: fmt, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });
            self.taa.sharpen_pso = Some((pso, fmt));
        }
    }

    /// THE RESOLVE: blend this frame into the history and write the sharpened result back into
    /// the scene target. Call after the transparent pass and before the overlay, the lines and
    /// the particles. Ends the frame's jitter: every pass after it draws unjittered.
    pub fn taa_resolve(&self) {
        let f = self.taa.now.get();
        let mut ended = f;
        ended.live = false;
        self.taa.now.set(ended);
        if !f.live {
            return;
        }
        let (Some(history), Some(resolve), Some((sharpen, _)), Some(params), Some(sampler)) = (
            self.taa.history.as_ref(),
            self.taa.resolve_pso.as_ref(),
            self.taa.sharpen_pso.as_ref(),
            self.taa.params.as_ref(),
            self.taa.sampler.as_ref(),
        ) else {
            return;
        };
        let _cost = frame_costs::stage("cpu.taa");
        let (w, h) = (self.config.width.max(1), self.config.height.max(1));
        let read = self.taa.cur.get();
        let write = 1 - read;
        // The sky's reprojection: last frame's celestial matrix after this frame's inverse.
        let sky = f.prev_cel * f.cur_cel.inverse();
        let p = TaaParams {
            size: [w as f32, h as f32, 1.0 / w as f32, 1.0 / h as f32],
            ctl: [if f.reset { 1.0 } else { 0.0 }, self.taa.sharpen.clamp(0.0, 1.0), dev_view() as f32, BLEND_FLOOR],
            extra: [0.0; 4],
            pad: [0.0; 4],
            sky_reproj: sky.to_cols_array_2d(),
        };
        self.queue.write_buffer(params, 0, bytemuck::bytes_of(&p));
        // Bind groups are made per frame (a few microseconds): nothing here can go stale when the
        // scene target, the depth or the motion target is remade.
        let resolve_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TAA Resolve BG"),
            layout: &resolve.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(self.scene.view()) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.motion_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&self.depth_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&history[read].1) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&history[write].1) },
                wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(sampler) },
                wgpu::BindGroupEntry { binding: 6, resource: params.as_entire_binding() },
            ],
        });
        let sharpen_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TAA Sharpen BG"),
            layout: &sharpen.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&self.motion_view) },
                wgpu::BindGroupEntry { binding: 6, resource: params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 7, resource: wgpu::BindingResource::TextureView(&history[write].1) },
            ],
        });
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("TAA Encoder") });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("TAA Resolve Pass"),
                timestamp_writes: self.compute_pass_timer("gpu.taa"),
            });
            pass.set_pipeline(resolve);
            pass.set_bind_group(0, &resolve_bg, &[]);
            pass.dispatch_workgroups(w.div_ceil(8), h.div_ceil(8), 1);
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("TAA Sharpen Pass"),
                timestamp_writes: self.pass_timer("gpu.taa"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.scene.view(),
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                ..Default::default()
            });
            pass.set_pipeline(sharpen);
            pass.set_bind_group(0, &sharpen_bg, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        self.taa.cur.set(write);
        self.taa.settled.set(self.taa.settled.get().saturating_add(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The jitter sequence is Halton (2, 3), indices 1 to 8, centred on the pixel. Seen red
    /// 2026-11-13 with `jitter_px` indexing from 0 (the first offset came out (-0.5, -0.5), a
    /// corner, and the eight positions were not the published table).
    #[test]
    fn the_jitter_is_halton_2_3_in_pixels() {
        let base2 = [0.5, 0.25, 0.75, 0.125, 0.625, 0.375, 0.875, 0.0625];
        let base3 = [1.0 / 3.0, 2.0 / 3.0, 1.0 / 9.0, 4.0 / 9.0, 7.0 / 9.0, 2.0 / 9.0, 5.0 / 9.0, 8.0 / 9.0];
        for i in 0..8u32 {
            assert!((halton(i + 1, 2) - base2[i as usize]).abs() < 1e-6, "halton({}, 2)", i + 1);
            assert!((halton(i + 1, 3) - base3[i as usize]).abs() < 1e-6, "halton({}, 3)", i + 1);
            let j = jitter_px(i);
            assert!((j.x - (base2[i as usize] - 0.5)).abs() < 1e-6 && (j.y - (base3[i as usize] - 0.5)).abs() < 1e-6, "phase {i}: {j:?}");
            assert!(j.x.abs() < 0.5 && j.y.abs() < 0.5, "inside the pixel");
        }
        // Eight phases, then it repeats.
        assert_eq!(jitter_px(3), jitter_px(3 + JITTER_PHASES));
        // The mean of a full cycle is near the pixel's centre (a biased cycle would shift the image).
        let mean = (0..8).map(jitter_px).fold(Vec2::ZERO, |a, b| a + b) / 8.0;
        assert!(mean.length() < 0.07, "mean {mean:?}");
        // In NDC: a whole pixel to the right on a 2560-wide target is 2/2560; a pixel DOWN is negative y.
        let n = jitter_ndc(Vec2::new(1.0, 1.0), 2560, 1440);
        assert!((n.x - 2.0 / 2560.0).abs() < 1e-9 && (n.y + 2.0 / 1440.0).abs() < 1e-9, "{n:?}");
    }

    /// A known moved object: a point 1 m to the right of where it stood last frame, seen by a
    /// still camera, writes exactly the screen motion its two projections differ by, in UV;
    /// worked out here independently, through the viewport transform in pixels. Seen red
    /// 2026-11-13 with the y factor written +0.5 (the motion pointed up for a thing moving down).
    #[test]
    fn a_moved_objects_motion_is_its_screen_motion() {
        let mut cam = Camera::new();
        cam.aspect = 16.0 / 9.0;
        let vp = cam.view_projection_matrix();
        let (w, h) = (2560.0_f32, 1440.0_f32);
        let to_px = |c: Vec4| Vec2::new((c.x / c.w * 0.5 + 0.5) * w, (0.5 - c.y / c.w * 0.5) * h);
        let fwd = cam.view_matrix().inverse().transform_vector3(-Vec3::Z).normalize();
        let then = cam.effective_position() + fwd * 10.0;
        for step in [Vec3::X, Vec3::Y * -0.5, Vec3::new(0.3, 0.2, -2.0)] {
            let now = then + step;
            let (c_now, c_then) = (vp * now.extend(1.0), vp * then.extend(1.0));
            let mv = motion_uv(c_now, c_then);
            let px = to_px(c_now) - to_px(c_then);
            assert!((mv.x * w - px.x).abs() < 1e-2 && (mv.y * h - px.y).abs() < 1e-2, "step {step:?}: motion {:?} px vs screen {px:?}", mv * Vec2::new(w, h));
            // The history is fetched where it was: uv(now) - motion = uv(then).
            let uv_then = to_px(c_then) / Vec2::new(w, h);
            let fetched = to_px(c_now) / Vec2::new(w, h) - mv;
            assert!((fetched - uv_then).length() < 1e-5);
        }
        // Moved through the object book: last frame's model comes back for a mesh drawn once.
        let mut book = MotionBook::default();
        let a = Mat4::from_translation(then);
        let _ = book.prev_model(7, a, Mat4::IDENTITY);
        book.turn(true);
        let b = Mat4::from_translation(then + Vec3::X);
        assert_eq!(book.prev_model(7, b, Mat4::IDENTITY), a, "a mesh drawn once remembers where it was");
        // A mesh drawn twice last frame has no identity: taken as still.
        let _ = book.prev_model(9, a, Mat4::IDENTITY);
        let _ = book.prev_model(9, b, Mat4::IDENTITY);
        book.turn(true);
        assert_eq!(book.prev_model(9, b, Mat4::IDENTITY), b);
        // Behind the camera last frame: leaves the screen, so the history is rejected.
        assert!(motion_uv(Vec4::new(0.0, 0.0, 0.5, 1.0), Vec4::new(0.0, 0.0, 0.5, -1.0)).x >= 1.0);
    }

    /// With the reuse switch off the history is thrown away EVERY frame, whatever else holds, and
    /// the motion book forgets too. Seen red 2026-11-13 with `history_kept` ignoring `reuse_on`.
    #[test]
    fn the_history_is_discarded_with_the_reuse_switch_off() {
        for had in [false, true] {
            for same in [false, true] {
                for asked in [false, true] {
                    for jumped in [false, true] {
                        for was_on in [false, true] {
                            assert!(!history_kept(false, had, same, asked, jumped, was_on));
                        }
                    }
                }
            }
        }
        assert!(history_kept(true, true, true, false, false, true), "and kept when nothing is wrong");
        assert!(!history_kept(true, true, false, false, false, true), "a resize discards it");
        assert!(!history_kept(true, true, true, true, false, true), "a cut discards it");
        assert!(!history_kept(true, true, true, false, true, true), "a jump discards it");
        assert!(!history_kept(true, true, true, false, false, false), "turning it on starts fresh");
        let mut book = MotionBook::default();
        let _ = book.prev_model(1, Mat4::from_translation(Vec3::X), Mat4::IDENTITY);
        book.turn(false);
        assert_eq!(book.prev_model(1, Mat4::IDENTITY, Mat4::IDENTITY), Mat4::IDENTITY, "the book forgets with the history");
    }

    /// The floating origin: a point still on the planet, placed by last frame's and this frame's
    /// body frames, maps from this frame's render space to last frame's through `frame_delta`,
    /// with the planet turned and the eye walked 3 m (planet-radius offsets, so f64 matters).
    #[test]
    fn the_floating_origin_reexpresses_last_frames_space() {
        let r = 6_371_000.0_f64;
        let body_pt = DVec3::new(10.0, r + 2.0, -30.0);
        let rot_o = DQuat::from_rotation_y(0.3);
        let rot_n = DQuat::from_rotation_y(0.3 + 7.3e-5 / 120.0);
        let eye_o = DVec3::new(0.0, r + 1.7, 0.0);
        let eye_n = eye_o + DVec3::new(3.0, 0.0, 0.0);
        let off_o = -(rot_o * eye_o);
        let off_n = -(rot_n * eye_n);
        let old = (rot_o * body_pt + off_o).as_vec3();
        let new = (rot_n * body_pt + off_n).as_vec3();
        let m = frame_delta((off_o, rot_o), (off_n, rot_n));
        let back = m.transform_point3(new);
        assert!((back - old).length() < 2e-3, "old {old:?} vs mapped {back:?}");
    }

    /// The shader's motion is the CPU twin's formula, so the test above speaks for the GPU.
    #[test]
    fn the_shader_motion_is_the_twin() {
        // The working copy is CRLF on Windows: read it the way the compiler does, line by line.
        let src = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/assets/shaders/pbr/00-bindings-vertex.wgsl")).replace("\r\n", "\n");
        let at = src.find("fn taa_motion(").expect("taa_motion in 00-bindings-vertex.wgsl");
        let body = &src[at..at + src[at..].find("\n}\n").expect("end of taa_motion")];
        let flat: String = body.split_whitespace().collect();
        assert!(flat.contains("(cur-prev)*vec2<f32>(0.5,-0.5)"), "the shader's motion must be (now - then) * (0.5, -0.5): {body}");
        assert!(flat.contains("<=1.0e-6"), "and reject a point behind the camera: {body}");
    }
}
