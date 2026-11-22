//! THE MIRROR VIEW: a planar reflection (2026-11-21, docs/design/mirrors.md).
//!
//! The operator, 2026-11-21: "I think we had a mirror in the bedroom at one point. If a player runs
//! in front of the mirror will it show the player's model?" It did not: the bedroom's standing
//! mirror was a wall screen showing the Profile page. Now a machine def with a `mirror` (data, any
//! piece, `humanity_ship::machines::MirrorDef`) is real glass: when one is in view, the scene is
//! drawn once more, from the eye reflected in the glass's plane, into a texture the glass samples.
//!
//! THE TECHNIQUE (the one planar mirrors are drawn with in real-time rendering since the portal
//! renderers, and in VR, where a mirror must be exact for both eyes): the GENERALIZED PERSPECTIVE
//! PROJECTION (Kooima 2008, "Generalized Perspective Projection") from the reflected eye through
//! the glass's rectangle. The eye `E` is reflected in the glass's plane to `E'`; the reflection
//! camera stands at `E'` looking straight through the glass (along its outward normal, into the
//! room), and its frustum is the off-axis one whose near plane IS the glass. Three things follow:
//!
//! * The near plane lies exactly on the glass, so everything behind the glass (the wall the mirror
//!   stands against, the mirror's own body) is clipped, as an oblique near plane (Lengyel 2005,
//!   "Oblique View Frustum Depth Projection and Clipping") clips it, without the oblique matrix's
//!   loss of depth precision: here the plane to clip by is perpendicular to the view axis, so the
//!   ordinary near plane is the oblique one.
//! * The glass's four corners land exactly on the texture's four corners, so the texture holds the
//!   mirror's picture and nothing else (every pixel drawn is a pixel the glass can show), and the
//!   glass samples it by its own UV. The picture a camera behind the glass sees is the mirror image
//!   turned left for right, so the glass quad's u runs the other way (`engine::mirrors`).
//! * The view is a proper rotation (no reflection matrix with determinant -1), so triangles keep
//!   their winding and every pipeline draws the reflection unchanged, its back-face culling included.
//!
//! The texture's size follows the glass's size ON SCREEN (`picture_size`): its projected width and
//! height in pixels times the level's scale (Settings > Graphics > Mirror reflections, the levels in
//! data/gui/mirror_levels.json), so a small or far mirror costs little and a mirror filling the view
//! is sharp. One mirror reflects at a time, the nearest in view (`engine::mirrors::pick`); the
//! others show plain silvered glass. Nothing is drawn while no mirror is in view.
//!
//! WHAT THE VIEW DRAWS: the scene's opaque and see-through lists (the home, its furniture, its
//! lights), every person in them (the near bodies as their meshes, the middle tier skinned in the
//! vertex stage), and the player's own body whole even in first person: the eye's copy has its head
//! and neck folded away, so the view leaves that copy out (`eye_only`) and draws the unfolded twin
//! the shadow maps already use (`Skinning::mirror_twins`). It draws no sky and no planet: a window
//! seen in a mirror shows black, as a camera screen's does (the celestial pass keeps one temporal
//! history, fitted to the player's view; docs/design/mirrors.md, what is left for later).
//!
//! THE TONE CURVE: every scene fragment ends with the ACES curve (80-fragment-shared.wgsl), so the
//! texture holds tone-mapped colour. The glass (a type-24 material with `MIRROR_PARAMS2`) turns it
//! back into light (`aces_inverse`), scales it by the glass's tint and lets the shared tail map it
//! once more, so the reflection is the room's light times the glass's, through one curve.
//!
//! The lights: the scene's tiled light lists are binned for the main view's screen
//! (renderer/light_tiles.rs), which the reflection's pixels do not share, so while the reflection
//! draws, the shadow uniform's tile size is set to 0 and every pixel loops the whole light list
//! (the classic loop; `mirror_begin`), and the main view's tile size is put back after
//! (`mirror_end`).

use super::Renderer;
use glam::{Mat4, Vec3, Vec4};
use serde::Deserialize;
use std::cell::Cell;

/// The mirror levels' file (Settings > Graphics > Mirror reflections).
pub const LEVELS_PATH: &str = "data/gui/mirror_levels.json";
/// The level a new player gets, and the one an unknown id falls back to.
pub const DEFAULT_LEVEL: &str = "high";
/// A type-24 material's `params2` that says "this is a mirror's glass": the texture holds a scene
/// view's tone-mapped colour, to be turned back into light (90-fragment-main.wgsl, type 24).
pub const MIRROR_PARAMS2: [f32; 4] = [0.0, 1.0, 0.0, 0.0];
/// A reflection whose longer side would be under this many pixels is not drawn (the glass shows
/// plain silver): a mirror that small on screen shows nothing anyone could read.
pub const MIN_SIDE_PX: u32 = 24;
/// Picture sizes are rounded up to a multiple of this, so walking about does not make a new texture
/// every frame (a new size is a new texture; within one step the size holds).
pub const SIZE_STEP_PX: u32 = 32;
/// The eye must be at least this far in front of the glass for it to reflect anything.
pub const MIN_EYE_DIST_M: f32 = 0.01;
/// The bytes a reflection pixel holds: the colour (four 16-bit floats), the depth (32-bit) and the
/// motion target every scene pass writes beside its colour (four 16-bit floats, taa.rs).
pub const BYTES_PER_PX: u64 = 8 + 4 + 8;

/// One level of Settings > Graphics > Mirror reflections (data/gui/mirror_levels.json).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MirrorLevel {
    pub id: String,
    /// The reflection's pixels for each pixel the glass covers on screen, along each side: 1 is as
    /// sharp as the screen, 0.5 a quarter of the pixels, 0 Off (nothing reflects).
    #[serde(default)]
    pub scale: f32,
    /// The longest side the reflection is ever drawn at, pixels (its aspect kept).
    #[serde(default = "default_max_side")]
    pub max_side: u32,
}

fn default_max_side() -> u32 {
    2048
}

#[derive(Deserialize)]
struct LevelsFile {
    levels: Vec<MirrorLevel>,
}

/// The levels from `text` (the file's contents), held to what the renderer can draw: a scale in
/// 0..1, a longest side in `MIN_SIDE_PX..4096`. With no file, or one that does not parse, the one
/// level is High at the screen's sharpness, so a mirror still reflects.
pub fn parse_levels(text: Option<&str>) -> Vec<MirrorLevel> {
    let parsed = text.and_then(|t| match serde_json::from_str::<LevelsFile>(t) {
        Ok(f) => Some(f.levels),
        Err(e) => {
            log::warn!("[Mirrors] {LEVELS_PATH} does not parse ({e}); Mirror reflections offers High only");
            None
        }
    });
    let mut out: Vec<MirrorLevel> = Vec::new();
    for mut l in parsed.unwrap_or_default() {
        if l.id.is_empty() || out.iter().any(|k| k.id == l.id) {
            continue;
        }
        l.scale = if l.scale.is_finite() { l.scale.clamp(0.0, 1.0) } else { 0.0 };
        l.max_side = l.max_side.clamp(MIN_SIDE_PX, 4096);
        out.push(l);
    }
    if out.is_empty() {
        out.push(MirrorLevel { id: DEFAULT_LEVEL.to_string(), scale: 1.0, max_side: default_max_side() });
    }
    out
}

/// Every level, from the file, read once a run (a change to it takes effect at the next start).
pub fn levels() -> &'static [MirrorLevel] {
    static LEVELS: std::sync::OnceLock<Vec<MirrorLevel>> = std::sync::OnceLock::new();
    LEVELS.get_or_init(|| {
        let text = std::fs::read_to_string(LEVELS_PATH).ok();
        if text.is_none() {
            log::warn!("[Mirrors] {LEVELS_PATH} not found; Mirror reflections offers High only");
        }
        parse_levels(text.as_deref())
    })
}

pub fn is_level(id: &str) -> bool {
    levels().iter().any(|l| l.id == id)
}

/// The level `id` names, or the default's for an id the file does not list.
pub fn level(id: &str) -> &'static MirrorLevel {
    let all = levels();
    all.iter().find(|l| l.id == id).or_else(|| all.iter().find(|l| l.id == DEFAULT_LEVEL)).unwrap_or(&all[0])
}

/// The size the reflection is drawn at for a glass that covers `projected` (its width and height
/// on screen, pixels) under `level`: scaled, the longer side held to the level's `max_side` with
/// the aspect kept, each side rounded up to a multiple of `SIZE_STEP_PX`. None when the level is
/// Off or the picture would be under `MIN_SIDE_PX` (nothing is drawn). Pure.
pub fn picture_size(level: &MirrorLevel, projected: (f32, f32)) -> Option<(u32, u32)> {
    if level.scale <= 0.0 {
        return None;
    }
    let (pw, ph) = projected;
    if !(pw.is_finite() && ph.is_finite()) || pw <= 0.0 || ph <= 0.0 {
        return None;
    }
    let (mut w, mut h) = (pw * level.scale, ph * level.scale);
    let longest = w.max(h);
    if longest < MIN_SIDE_PX as f32 {
        return None;
    }
    let cap = level.max_side as f32;
    if longest > cap {
        let k = cap / longest;
        w *= k;
        h *= k;
    }
    // Rounded up to the step, and never past the cap rounded up to it.
    let cap_px = level.max_side.div_ceil(SIZE_STEP_PX) * SIZE_STEP_PX;
    let step = |x: f32| -> u32 { ((x.ceil().max(1.0) as u32).div_ceil(SIZE_STEP_PX) * SIZE_STEP_PX).min(cap_px) };
    Some((step(w), step(h)))
}

/// The video memory a reflection of `size` holds (colour, depth and motion).
pub fn reflection_bytes(size: (u32, u32)) -> u64 {
    size.0 as u64 * size.1 as u64 * BYTES_PER_PX
}

/// The size of the reflection drawn this frame, (width << 32 | height), 0 when none was: what
/// Settings > Graphics says the mirrors are doing now (`publish_now`, `published_now`).
static NOW: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Say what the mirrors drew this frame (the engine, once a frame): a reflection's size, or None.
pub fn publish_now(size: Option<(u32, u32)>) {
    let v = size.map_or(0, |(w, h)| ((w as u64) << 32) | h as u64);
    NOW.store(v, std::sync::atomic::Ordering::Relaxed);
}

/// The size of the reflection drawn on the last frame, if one was.
pub fn published_now() -> Option<(u32, u32)> {
    let v = NOW.load(std::sync::atomic::Ordering::Relaxed);
    (v != 0).then_some(((v >> 32) as u32, (v & 0xFFFF_FFFF) as u32))
}

/// The reflection camera for a glass rectangle (`origin` its top-left corner as seen from the front,
/// `u_axis` its full width to the viewer's right, `v_axis` its full height downward, `normal` out of
/// the front: `engine::screens::QuadGeom`'s convention) and the eye at `eye`: the eye reflected in
/// the glass's plane, looking straight through the glass, with the off-axis frustum whose near plane
/// is the glass and whose edges are the glass's edges (Kooima 2008). Its picture is the mirror image
/// turned left for right: texture x = 1 - the glass's u, texture y = the glass's v. None when the
/// eye is not in front of the glass. Pure.
pub fn reflected_view(origin: Vec3, u_axis: Vec3, v_axis: Vec3, normal: Vec3, eye: Vec3, far: f32) -> Option<super::camera::OffAxis> {
    let n = normal.normalize_or_zero();
    let (uw, vh) = (u_axis.length(), v_axis.length());
    if n == Vec3::ZERO || uw < 1.0e-4 || vh < 1.0e-4 {
        return None;
    }
    let d = (eye - origin).dot(n);
    if !(d > MIN_EYE_DIST_M) {
        return None;
    }
    let eye_r = eye - 2.0 * d * n;
    let (u_hat, v_hat) = (u_axis / uw, v_axis / vh);
    // Looking along the outward normal with the glass's up (-v): glam's look_to_rh gives the
    // camera's right as n x (-v) = -u (v x u = n, QuadGeom's handedness) and its up as -v. So the
    // camera sees the glass's u running RIGHT TO LEFT: the mirror image, left for right.
    let view = Mat4::look_to_rh(eye_r, n, -v_hat);
    let rel = origin - eye_r;
    let x_tl = rel.dot(-u_hat);
    let y_tl = rel.dot(-v_hat);
    let (left, right) = (x_tl - uw, x_tl);
    let (bottom, top) = (y_tl - vh, y_tl);
    let proj = off_axis_reverse_z(left, right, bottom, top, d, far.max(d * 2.0));
    Some(super::camera::OffAxis { view, proj, eye: eye_r })
}

/// A right-handed off-axis perspective projection into wgpu's 0..1 depth, REVERSE-Z (the near
/// plane at depth 1, the far at 0, as `Camera::projection_matrix`): the edges `left..right`,
/// `bottom..top` measured on the near plane, `near` and `far` distances along the view axis.
pub fn off_axis_reverse_z(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
    let (rl, tb) = (right - left, top - bottom);
    let a = near / (far - near);
    let b = near * far / (far - near);
    Mat4::from_cols(
        Vec4::new(2.0 * near / rl, 0.0, 0.0, 0.0),
        Vec4::new(0.0, 2.0 * near / tb, 0.0, 0.0),
        Vec4::new((right + left) / rl, (top + bottom) / tb, a, -1.0),
        Vec4::new(0.0, 0.0, b, 0.0),
    )
}

/// The reflection's textures at one size.
struct Target {
    size: (u32, u32),
    format: wgpu::TextureFormat,
    _colour: wgpu::Texture,
    colour_view: wgpu::TextureView,
    depth: (wgpu::Texture, wgpu::TextureView),
    motion: (wgpu::Texture, wgpu::TextureView),
}

/// The mirror view's state on the renderer.
#[derive(Default)]
pub struct MirrorView {
    /// The textures the reflection is drawn into, at the size last drawn.
    /// reuse: not a reuse: the picture is drawn anew on every frame a mirror is in view; only the
    /// textures' memory is kept while the size holds, as a camera screen keeps its depth buffer.
    target: Option<Target>,
    /// The window's depth buffer and motion target while a reflection is drawn into the mirror's.
    parked: Option<((wgpu::Texture, wgpu::TextureView), (wgpu::Texture, wgpu::TextureView))>,
    /// A reflection is being drawn: its passes take no jitter and write no motion anything reads
    /// (`offscreen_view_active`), and the middle tier's pass adds the player's unfolded twin.
    pub(super) active: bool,
    /// Scene passes drawn into a mirror since the start (what the mirror probe counts).
    pub passes: Cell<u64>,
    /// Meshes drawn only for the player's eye this frame: the first-person body with its head
    /// folded away, which the reflection leaves out (it draws the unfolded twin instead). Set by
    /// the engine each frame, cleared with the frame's skinning (`skin_begin_frame`).
    pub eye_only: Vec<usize>,
    /// The showcase pin of the level (`{"mirror":...}`): None follows Settings.
    pub pin: Option<String>,
    /// The showcase's `{"mirror_me":"0"}`: the player's own body left out of the reflection (the
    /// rig's hidden picture, the red twin of drawing it).
    pub hide_me: bool,
}

impl Renderer {
    /// Start drawing a reflection of `size`: the mirror's textures made (or remade at a new size),
    /// its depth buffer and motion target swapped in for the window's, its colour cleared, and the
    /// light tiles set to the whole-list loop. Returns true when the colour texture is new (the
    /// glass's material must bind it again). Pair with [`mirror_end`](Self::mirror_end).
    pub fn mirror_begin(&mut self, size: (u32, u32)) -> bool {
        let (w, h) = (size.0.max(1), size.1.max(1));
        if self.mirror.active {
            log::warn!("[Mirrors] mirror_begin while a reflection is already being drawn; ignored");
            return false;
        }
        // A new size, or a new scene format (the pipelines draw in it; the scene-format A/B switch).
        let format = self.scene_format();
        let fresh = !self.mirror.target.as_ref().is_some_and(|t| t.size == (w, h) && t.format == format);
        if fresh {
            let colour = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Mirror Reflection"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let colour_view = colour.create_view(&wgpu::TextureViewDescriptor::default());
            self.mirror.target = Some(Target {
                size: (w, h),
                format,
                _colour: colour,
                colour_view,
                depth: Self::create_depth_texture(&self.device, w, h),
                motion: super::taa::create_motion_texture(&self.device, w, h),
            });
            humanity_core::frame_costs::set_vram("vram.mirror", reflection_bytes((w, h)));
        }
        let t = self.mirror.target.as_mut().expect("made above");
        // The mirror's depth and motion in, the window's parked (every scene pass binds
        // `depth_view` and `motion_view`).
        let depth = (std::mem::replace(&mut self.depth_texture, t.depth.0.clone()), std::mem::replace(&mut self.depth_view, t.depth.1.clone()));
        let motion = (std::mem::replace(&mut self.motion_texture, t.motion.0.clone()), std::mem::replace(&mut self.motion_view, t.motion.1.clone()));
        self.mirror.parked = Some((depth, motion));
        self.mirror.active = true;
        // The reflection's own motion target starts uncleared; the window's state comes back at the end.
        self.taa.saved_cleared.set(self.taa.motion_cleared.replace(false));
        // Every pixel loops the whole light list (the tiles are the main view's): params2.zw = 0.
        self.queue.write_buffer(&self.shadow_uniform_buffer, 88, bytemuck::cast_slice(&[0.0_f32, 0.0]));
        // Cleared to black: the scene pass loads its colour (it is drawn over the sky elsewhere).
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Mirror Clear Encoder") });
        {
            let view = &self.mirror.target.as_ref().expect("made above").colour_view;
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Mirror Clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        fresh
    }

    /// The reflection's colour texture, once one was made: what the glass samples.
    pub fn mirror_target_view(&self) -> Option<wgpu::TextureView> {
        self.mirror.target.as_ref().map(|t| t.colour_view.clone())
    }

    /// The size of the reflection last drawn.
    pub fn mirror_target_size(&self) -> Option<(u32, u32)> {
        self.mirror.target.as_ref().map(|t| t.size)
    }

    /// Finish drawing a reflection: the window's depth buffer and motion target back, the main
    /// view's light tiles back. Without a matching begin this does nothing.
    pub fn mirror_end(&mut self) {
        if !self.mirror.active {
            return;
        }
        if let Some(((dt, dv), (mt, mv))) = self.mirror.parked.take() {
            self.depth_texture = dt;
            self.depth_view = dv;
            self.motion_texture = mt;
            self.motion_view = mv;
        }
        self.mirror.active = false;
        self.taa.motion_cleared.set(self.taa.saved_cleared.get());
        self.queue.write_buffer(&self.shadow_uniform_buffer, 88, bytemuck::cast_slice(&[self.tile_px.0, self.tile_px.1]));
    }

    /// Give the reflection's textures back (the level went Off, or no mirror is placed).
    pub fn mirror_release(&mut self) {
        if !self.mirror.active && self.mirror.target.take().is_some() {
            humanity_core::frame_costs::set_vram("vram.mirror", 0);
        }
    }

    /// Whether a reflection is being drawn right now.
    pub(super) fn mirror_view_active(&self) -> bool {
        self.mirror.active
    }

    /// The objects of `objects` whose bounding sphere (`Mesh::bounds`, placed by the object) touches
    /// the reflection's frustum (`view_proj`): the glass shows a narrow window of the room, and a
    /// draw costs the processor about a microsecond whatever it covers (local_shadows.rs measured it),
    /// so the home's thousands of draws outside the window are not sent. A mesh of unknown bounds is
    /// always kept.
    pub fn cull_to_view(&self, objects: &[super::RenderObject], view_proj: Mat4) -> Vec<super::RenderObject> {
        let planes = super::local_shadows::frustum_planes(&view_proj);
        objects
            .iter()
            .filter(|o| {
                let Some(mesh) = self.meshes.get(o.mesh) else { return false };
                let [bx, by, bz, br] = mesh.bounds;
                if br < 0.0 {
                    return true;
                }
                let centre = o.position + o.rotation * (o.scale * Vec3::new(bx, by, bz));
                super::local_shadows::sphere_in(&planes, centre, br * o.scale.abs().max_element())
            })
            .cloned()
            .collect()
    }
}

#[cfg(test)]
#[path = "mirror_tests.rs"]
mod tests;
