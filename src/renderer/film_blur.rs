//! THE BLUR BEHIND A FROSTED FILM (2026-11-13; docs/design/local-light-shadows.md section 10,
//! "Built: the blur behind the film").
//!
//! The operator, 2026-11-13, approving the fruiting tent's follow-ups: "Let's give those a shot."
//! A real fogged plastic cover scatters the light coming through it, so the shelves behind it read
//! as soft shapes, the softer the farther behind the sheet they are; until this file the film was
//! a plain alpha blend over a sharp scene, so every shelf edge and mushroom showed crisp through a
//! milky veil.
//!
//! WHY THE BLUR GROWS WITH THE GAP. A point of the sheet passes on light from a cone of directions
//! behind it (the sheet's scattering lobe, half-angle theta). That cone meets a surface a gap g
//! behind the sheet in a disc of radius g * tan(theta): a mushroom touching the film is sharp, the
//! back wall half a metre away is a blur (`blur_radius_texels`, the film's `params2.z` is
//! tan(theta), data/garden/grow_media.ron `scatter_spread`). Only the scattered share of what gets
//! through is blurred; the rest goes straight through (the sheet's HAZE, `haze`: its scattered
//! transmission over its whole transmission, 0.65 / 0.8 for the tent).
//!
//! HOW, per frame, only when a film is in the main view's see-through list and Settings > Graphics
//! > Frosted plastic is Blurred, all recorded into the see-through pass's own encoder and sent with
//! its one submit (`film_blur_behind`, called by `render_transparent_onto`):
//! 1. THE FAR SIDES FIRST: each film's faces turned away from the eye (the cover's back and sides
//!    seen through its front) drawn as before, over the opaque scene, through the surface
//!    transparent PSO with the film mode FAR (the near faces discard). The film mode rides the
//!    camera uniform, so this pass binds a camera group of its own (`far_camera`, `far_group`,
//!    made with the others in `rebuild_camera_bind_groups`): one submit cannot hold two values of
//!    one buffer.
//! 2. THE COVERAGE: the films' faces again, depth-tested against the scene and writing no depth, mark
//!    at the render's size every pixel a film covers (`MASK_FORMAT`, cleared to 0 first).
//! 3. THE COLOUR PYRAMID (assets/shaders/film_blur.wgsl): the scene target (now holding the far sides
//!    too) downsampled to half size, every texel weighed by its coverage (layer 0: colour times
//!    coverage, coverage in alpha), each level above a 13-tap downsample of the one below, up to
//!    `MAX_LEVELS`; layer 1's base holds the view depth of the nearest surface. Only the rectangle
//!    of the screen the films cover is processed (`screen_rect`, `level_rect`): the rest of every
//!    level is cleared, coverage 0, which is what it is.
//! 4. THE NEAR SIDES (the see-through pass as before, film mode NEAR): each film's faces toward the
//!    eye discard the far ones and blend the pyramid in (`film_frost`, 90-fragment-main.wgsl): the
//!    level whose texels match the disc's radius on screen, four bilinear taps round it, colour over
//!    coverage (`covered_mean`).
//! Off (Simple), in a camera screen's re-render, under an orthographic camera or without the float
//! scene target, nothing of this runs: the film is drawn exactly as before.
//!
//! WHY THE COVERAGE (2026-11-14). The blur is screen-space, so near a cover's outline its disc
//! reached past the film into what is beside the tent: at night, with the dark room round the
//! nearest tent, its front read 12% darker over the whole tent (a far tent among lit ones did not
//! change). A normalized convolution (Knutsson and Westin, CVPR 1993: weigh each sample by its
//! certainty, divide by the summed weight) reads only what a film covers, whatever the disc's size.
//!
//! WHY ONE SUBMIT (2026-11-14). The merged version had an encoder and a submit of its own, 1.1 to
//! 1.3 ms of the processor against a 0.5 ms plan (docs/design/local-light-shadows.md section 10).
//! Showcase `film_v1` "1" is the red twin: its own encoder and submit again, no coverage, the whole
//! screen processed (the merged behaviour, for a same-boot A/B).
//!
//! A KNOWN LIMIT: other see-through things behind a film (a window beyond a tent) are drawn after
//! the pyramid, so through the film they show only at the sheet's straight-through share (1 - haze).
//!
//! NOTHING KEPT ACROSS FRAMES: the pyramid and the mask are rebuilt every frame they are used, their
//! bind groups made every frame; the textures are resources sized to the window, made when the
//! setting turns on and dropped when it turns off.

use super::*;

/// The pyramid's texels: layer 0 rgb the covered scene behind the films times its coverage and a the
/// coverage; layer 1 (its base only) r the view depth behind.
pub const PYRAMID_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The coverage mark at the render's size: 1 where a film covers the pixel.
pub const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;
/// The pyramid's levels at most: the base at half the render's size, then halving, so the top
/// level's texel is 64 render pixels and the widest blur about a tenth of a 1440-line screen.
pub const MAX_LEVELS: u32 = 6;
/// The camera's film mode (`camera.light4_spot.x`, a legacy uniform slot no shader read before):
/// 0 the film as before, FAR its far faces only, NEAR its near faces blended over the pyramid.
/// Large values, so no stray number in that slot can match. LOCKSTEP with 90-fragment-main.wgsl.
pub const MODE_FAR: f32 = 7001.0;
pub const MODE_NEAR: f32 = 7002.0;
/// The films the coverage pass marks at most a frame (their model matrices, 64 bytes each, in one
/// buffer made at start: the pass runs inside `&self` drawing, which cannot grow it). A film past
/// this marks nothing and so is drawn whole, unblurred (a tent is one or two films).
pub const MAX_COVER_FILMS: usize = 1024;
const COVER_INSTANCE_BYTES: u64 = 64;
/// How many render pixels the films' rectangle is padded by (anti-aliasing's jitter, rounding).
const RECT_PAD_PX: u32 = 2;

const WGSL: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/assets/shaders/film_blur.wgsl"));

/// The blur's radius in the pyramid base's texels: a sheet `film_z_m` from the eye with a surface
/// `gap_m` behind it, scattering into a cone of half-angle atan(`spread`), seen through a lens of
/// `focal` base texels per metre at a metre (`focal_texels`). CPU twin of `film_blur_radius` in
/// 90-fragment-main.wgsl.
pub fn blur_radius_texels(gap_m: f32, film_z_m: f32, spread: f32, focal: f32) -> f32 {
    gap_m.max(0.0) * spread * focal / film_z_m.max(0.01)
}

/// The gap the blur uses: the distance to what is behind the sheet, held to the sheet's reach
/// (`params2.w`, the cover's own depth; 0 = not held). Past the cover's far side the sheet sees
/// that far side, another milky sheet, which the depth buffer does not hold (see-through surfaces
/// write no depth): unheld, the room metres beyond made every tent's inside the widest blur, pulling
/// the dark room round a tent into it (seen 2026-11-13: the near tent's front 20% darker at night
/// than the same pixels sharp). CPU twin of `film_gap` in 90-fragment-main.wgsl.
pub fn film_gap(gap_m: f32, reach_m: f32) -> f32 {
    if reach_m > 0.0 {
        gap_m.min(reach_m)
    } else {
        gap_m
    }
}

/// The pyramid level whose texels are that radius (base texels), held to the levels there are.
/// CPU twin of `film_blur_lod`.
pub fn blur_lod(radius: f32, top: f32) -> f32 {
    radius.max(1.0).log2().clamp(0.0, top)
}

/// The share of what gets through a film that is scattered (its haze): `params2.x / params2.y`.
pub fn haze(params2: [f32; 4]) -> f32 {
    (params2[0] / params2[1].max(1e-3)).clamp(0.0, 1.0)
}

/// The blurred scene from the pyramid's taps (rgb colour times coverage, a coverage): their
/// quotient, or None where no tap was covered (the film is then drawn whole). CPU twin of
/// `film_covered` in 90-fragment-main.wgsl.
pub fn covered_mean(b: [f32; 4]) -> Option<[f32; 3]> {
    (b[3] > 1e-4).then(|| [b[0] / b[3], b[1] / b[3], b[2] / b[3]])
}

/// Layer 0 of the pyramid's base texel (`bx`, `by`) from a render of `size`: the 4 x 4 tent
/// (1 3 3 1 each way) over the render texels under it, each weighed by its coverage (`all_count`:
/// every texel by one, the merged look). CPU twin of `fs_base` in film_blur.wgsl.
pub fn base_texel(scene: impl Fn(u32, u32) -> [f32; 3], mask: impl Fn(u32, u32) -> f32, size: (u32, u32), bx: u32, by: u32, all_count: bool) -> [f32; 4] {
    const TENT: [f32; 4] = [1.0, 3.0, 3.0, 1.0];
    let mut acc = [0.0f32; 4];
    for (y, ty) in TENT.iter().enumerate() {
        for (x, tx) in TENT.iter().enumerate() {
            let px = (2 * bx as i64 + x as i64 - 1).clamp(0, size.0 as i64 - 1) as u32;
            let py = (2 * by as i64 + y as i64 - 1).clamp(0, size.1 as i64 - 1) as u32;
            let m = if all_count { 1.0 } else { mask(px, py) };
            let w = m * tx * ty / 64.0;
            let c = scene(px, py);
            acc[0] += c[0] * w;
            acc[1] += c[1] * w;
            acc[2] += c[2] * w;
            acc[3] += w;
        }
    }
    acc
}

/// Base texels per metre at a metre: the projection's focal length times half the base's height.
pub fn focal_texels(proj: &Mat4, render_h: u32) -> f32 {
    proj.y_axis.y * render_h as f32 / 4.0
}

/// The pyramid's base size for a render of `w` x `h`.
pub fn base_size(w: u32, h: u32) -> (u32, u32) {
    ((w / 2).max(1), (h / 2).max(1))
}

/// How many levels a render of `w` x `h` gets.
pub fn levels_for(w: u32, h: u32) -> u32 {
    let (bw, bh) = base_size(w, h);
    (32 - bw.min(bh).leading_zeros()).clamp(1, MAX_LEVELS)
}

/// The blur's video memory for a render of `w` x `h`: the pyramid's two layers at every level (an
/// array texture's layers share their levels, so the depth layer has them too, written at the base
/// only) and the coverage mark at the render's size.
pub fn pyramid_bytes(w: u32, h: u32) -> u64 {
    let (bw, bh) = base_size(w, h);
    let layer: u64 = (0..levels_for(w, h)).map(|i| (bw >> i).max(1) as u64 * (bh >> i).max(1) as u64 * 8).sum();
    2 * layer + w.max(1) as u64 * h.max(1) as u64
}

/// A rectangle (x, y, width, height).
pub type Rect = (u32, u32, u32, u32);

/// THE SCREEN THE FILMS COVER: a rectangle of render pixels round `spheres` (centre in VIEW space,
/// the eye at the origin looking down -z, and radius), through `proj`, padded a little and held to
/// the render. Conservative: each sphere's view-space box corners are projected, and a sphere that
/// reaches the eye's plane (or has no bound) takes the whole render. None when no sphere is in
/// front of the eye at all (nothing to blur).
pub fn screen_rect(spheres: &[(Vec3, f32)], proj: &Mat4, w: u32, h: u32) -> Option<Rect> {
    let (w, h) = (w.max(1), h.max(1));
    let full = Some((0, 0, w, h));
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &(c, r) in spheres {
        if !r.is_finite() || r < 0.0 {
            return full;
        }
        // Wholly behind the eye: off screen.
        if c.z - r >= 0.0 {
            continue;
        }
        // Reaching the eye's plane: its projection is unbounded.
        if c.z + r > -0.01 {
            return full;
        }
        for k in 0..8u32 {
            let corner = c + Vec3::new(if k & 1 == 0 { -r } else { r }, if k & 2 == 0 { -r } else { r }, if k & 4 == 0 { -r } else { r });
            let clip = *proj * corner.extend(1.0);
            if clip.w <= 1e-6 {
                return full;
            }
            let (nx, ny) = (clip.x / clip.w, clip.y / clip.w);
            let (px, py) = ((nx * 0.5 + 0.5) * w as f32, (0.5 - ny * 0.5) * h as f32);
            x0 = x0.min(px);
            y0 = y0.min(py);
            x1 = x1.max(px);
            y1 = y1.max(py);
        }
    }
    if x0 > x1 {
        return None;
    }
    let pad = RECT_PAD_PX as f32;
    let (ax, ay) = ((x0 - pad).floor().clamp(0.0, w as f32) as u32, (y0 - pad).floor().clamp(0.0, h as f32) as u32);
    let (bx, by) = ((x1 + pad).ceil().clamp(0.0, w as f32) as u32, (y1 + pad).ceil().clamp(0.0, h as f32) as u32);
    (bx > ax && by > ay).then_some((ax, ay, bx - ax, by - ay))
}

/// The rectangle of pyramid level `lvl` (0 = the base, half the render) that a render rectangle
/// needs: halved at each level (rounding outward), padded by two texels for the downsample's reach,
/// held to the level's size. Empty (width or height 0) when it falls off the level.
pub fn level_rect(rect: Rect, lvl: u32, render: (u32, u32)) -> Rect {
    let (mut x0, mut y0, mut x1, mut y1) = (rect.0, rect.1, rect.0 + rect.2, rect.1 + rect.3);
    let mut size = render;
    for i in 0..=lvl {
        size = if i == 0 { base_size(render.0, render.1) } else { ((size.0 / 2).max(1), (size.1 / 2).max(1)) };
        x0 = (x0 / 2).saturating_sub(2);
        y0 = (y0 / 2).saturating_sub(2);
        x1 = (x1.div_ceil(2) + 2).min(size.0);
        y1 = (y1.div_ceil(2) + 2).min(size.1);
        x0 = x0.min(x1);
        y0 = y0.min(y1);
    }
    (x0, y0, x1 - x0, y1 - y0)
}

/// The pyramid and the coverage mark, sized to the render.
pub(crate) struct Pyramid {
    _tex: wgpu::Texture,
    /// Both layers, every level: what group 0 binds at 12.
    view: wgpu::TextureView,
    /// Layer 0's levels, one view each (the passes' targets, the down pass's source).
    levels: Vec<wgpu::TextureView>,
    /// Layer 1's base: the depth behind.
    depth_base: wgpu::TextureView,
    _mask_tex: wgpu::Texture,
    mask: wgpu::TextureView,
    size: (u32, u32),
}

impl Pyramid {
    fn new(device: &wgpu::Device, size: (u32, u32)) -> Self {
        let (bw, bh) = base_size(size.0, size.1);
        let n = levels_for(size.0, size.1);
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Film Blur Pyramid"),
            size: wgpu::Extent3d { width: bw, height: bh, depth_or_array_layers: 2 },
            mip_level_count: n,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PYRAMID_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
        let one = |layer: u32, mip: u32| {
            tex.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_mip_level: mip,
                mip_level_count: Some(1),
                base_array_layer: layer,
                array_layer_count: Some(1),
                ..Default::default()
            })
        };
        let levels = (0..n).map(|i| one(0, i)).collect();
        let depth_base = one(1, 0);
        let mask_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Film Blur Coverage"),
            size: wgpu::Extent3d { width: size.0.max(1), height: size.1.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: MASK_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mask = mask_tex.create_view(&wgpu::TextureViewDescriptor::default());
        Pyramid { _tex: tex, view, levels, depth_base, _mask_tex: mask_tex, mask, size }
    }
}

/// The film blur's GPU state.
pub struct FilmBlur {
    /// Settings > Graphics > Frosted plastic: true = Blurred (set by lib.rs each frame).
    pub on: bool,
    /// Showcase pin `film_blur` "0" / "1" over the setting (its red twin is "0", today's look).
    pub pin: Option<bool>,
    /// Showcase pin `film_v1` "1": the merged version (2026-11-13) for a same-boot A/B: its own
    /// encoder and submit, no coverage, the whole screen processed.
    pub v1: bool,
    base_pso: wgpu::RenderPipeline,
    down_pso: wgpu::RenderPipeline,
    cover_pso: wgpu::RenderPipeline,
    /// Linear, clamped: the downsample's taps.
    down_sampler: wgpu::Sampler,
    /// Linear between texels and between levels, clamped: what the film's shading samples with
    /// (group 0 binding 13).
    pub read_sampler: wgpu::Sampler,
    params: Vec<wgpu::Buffer>,
    cover_cam: wgpu::Buffer,
    cover_inst: wgpu::Buffer,
    /// The far-side pass's camera uniform (film mode FAR) and its group 0, made with the scene's
    /// in `rebuild_camera_bind_groups`.
    pub(crate) far_camera: wgpu::Buffer,
    pub(crate) far_group: Option<wgpu::BindGroup>,
    pyramid: Option<Pyramid>,
    dummy_view: wgpu::TextureView,
    /// Where `cpu.film_blur` goes, for the log line (the dev answer to "what does it spend on?"):
    /// the setup, the far sides, the coverage, the pyramid; microseconds smoothed over about twenty
    /// frames, said every 10 s while a film is in view. Scratch figures, never read back as state.
    split_avg: std::cell::Cell<[f32; 4]>,
    last_log: std::cell::Cell<Option<std::time::Instant>>,
}

impl FilmBlur {
    pub fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Film Blur Shader"), source: wgpu::ShaderSource::Wgsl(WGSL.into()) });
        let target = |format| Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL });
        let pso = |entry: &str, label: &str, targets: &[Option<wgpu::ColorTargetState>]| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: None,
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_full"), buffers: &[], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some(entry), targets, compilation_options: Default::default() }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };
        let base_pso = pso("fs_base", "Film Blur Base Pipeline", &[target(PYRAMID_FORMAT), target(PYRAMID_FORMAT)]);
        let down_pso = pso("fs_down", "Film Blur Down Pipeline", &[target(PYRAMID_FORMAT)]);
        let instance_attrs = [
            wgpu::VertexAttribute { offset: 0, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 16, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 32, shader_location: 3, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 48, shader_location: 4, format: wgpu::VertexFormat::Float32x4 },
        ];
        let position = [wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 }];
        let cover_pso = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Film Blur Coverage Pipeline"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_cover"),
                buffers: &[
                    wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<humanity_core::vertex::Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &position },
                    wgpu::VertexBufferLayout { array_stride: COVER_INSTANCE_BYTES, step_mode: wgpu::VertexStepMode::Instance, attributes: &instance_attrs },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState { module: &module, entry_point: Some("fs_cover"), targets: &[target(MASK_FORMAT)], compilation_options: Default::default() }),
            // Both faces: the pixels the cover is seen in, from in front of it or inside it.
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            // The scene's depth tested as the films' own near faces test it (reverse-Z), never written.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Greater,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        let clamp = wgpu::AddressMode::ClampToEdge;
        let down_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Film Blur Down Sampler"),
            address_mode_u: clamp,
            address_mode_v: clamp,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let read_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Film Blur Read Sampler"),
            address_mode_u: clamp,
            address_mode_v: clamp,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let buffer = |label: &str, size: u64, usage: wgpu::BufferUsages| device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage: usage | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let params = (0..MAX_LEVELS).map(|_| buffer("Film Blur Params", 32, wgpu::BufferUsages::UNIFORM)).collect();
        let cover_cam = buffer("Film Blur Coverage Camera", 64, wgpu::BufferUsages::UNIFORM);
        let cover_inst = buffer("Film Blur Coverage Instances", MAX_COVER_FILMS as u64 * COVER_INSTANCE_BYTES, wgpu::BufferUsages::VERTEX);
        let far_camera = buffer("Film Far Side Camera", std::mem::size_of::<camera::CameraUniforms>() as u64, wgpu::BufferUsages::UNIFORM);
        let dummy = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Film Blur Stand-in"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 2 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PYRAMID_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let dummy_view = dummy.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
        FilmBlur { on: true, pin: None, v1: false, base_pso, down_pso, cover_pso, down_sampler, read_sampler, params, cover_cam, cover_inst, far_camera, far_group: None, pyramid: None, dummy_view, split_avg: std::cell::Cell::new([0.0; 4]), last_log: std::cell::Cell::new(None) }
    }

    /// Whether the blur is asked for: the pin, else the setting.
    pub fn wanted(&self) -> bool {
        self.pin.unwrap_or(self.on)
    }

    /// What group 0 binds at 12: the pyramid (both layers, every level), or the 1x1 stand-in.
    pub fn view(&self) -> &wgpu::TextureView {
        self.pyramid.as_ref().map_or(&self.dummy_view, |p| &p.view)
    }

    /// The blur's video memory now (0 when it is not made).
    pub fn vram_bytes(&self) -> u64 {
        self.pyramid.as_ref().map_or(0, |p| pyramid_bytes(p.size.0, p.size.1))
    }

    /// Record the pyramid (step 3 of the file's header) into `encoder`: the base from `scene` (each
    /// texel weighed by the pyramid's coverage mark, or by one with `all_count`) and `depth`, then
    /// each level from the one below, each pass only over `rect` (render pixels) carried to its
    /// level; the rest of every level cleared. `timer` names the passes' timestamps.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_pyramid<'t>(&self, device: &wgpu::Device, queue: &wgpu::Queue, encoder: &mut wgpu::CommandEncoder, pyr: &Pyramid, scene: &wgpu::TextureView, depth: &wgpu::TextureView, proj: &Mat4, rect: Rect, all_count: bool, timer: &dyn Fn() -> Option<wgpu::RenderPassTimestampWrites<'t>>) {
        let (w, h) = pyr.size;
        let mut src_size = (w, h);
        for (lvl, target) in pyr.levels.iter().enumerate() {
            let p: [f32; 8] = [src_size.0 as f32, src_size.1 as f32, 1.0 / src_size.0 as f32, 1.0 / src_size.1 as f32, proj.z_axis.z, proj.w_axis.z, 0.0, if all_count { 1.0 } else { 0.0 }];
            queue.write_buffer(&self.params[lvl], 0, bytemuck::cast_slice(&p));
            let group = if lvl == 0 {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Film Blur Base BG"),
                    layout: &self.base_pso.get_bind_group_layout(0),
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(scene) },
                        wgpu::BindGroupEntry { binding: 2, resource: self.params[lvl].as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(depth) },
                        wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&pyr.mask) },
                    ],
                })
            } else {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Film Blur Down BG"),
                    layout: &self.down_pso.get_bind_group_layout(0),
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&pyr.levels[lvl - 1]) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.down_sampler) },
                        wgpu::BindGroupEntry { binding: 2, resource: self.params[lvl].as_entire_binding() },
                    ],
                })
            };
            let clear = wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store };
            let attach = |view| Some(wgpu::RenderPassColorAttachment { view, resolve_target: None, ops: clear });
            let colour = [attach(target), attach(&pyr.depth_base)];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(if lvl == 0 { "Film Blur Base Pass" } else { "Film Blur Pyramid Pass" }),
                timestamp_writes: timer(),
                color_attachments: if lvl == 0 { &colour[..] } else { &colour[..1] },
                ..Default::default()
            });
            let (x, y, sw, sh) = level_rect(rect, lvl as u32, (w, h));
            if sw > 0 && sh > 0 {
                pass.set_pipeline(if lvl == 0 { &self.base_pso } else { &self.down_pso });
                pass.set_bind_group(0, &group, &[]);
                pass.set_scissor_rect(x, y, sw, sh);
                pass.draw(0..3, 0..1);
            }
            drop(pass);
            src_size = if lvl == 0 { base_size(w, h) } else { ((src_size.0 / 2).max(1), (src_size.1 / 2).max(1)) };
        }
    }
}

impl Renderer {
    /// Whether the films can be blurred this frame: the pyramid made at this size, the scene drawn
    /// into the float scene target, and a perspective camera.
    fn film_blur_ready(&self, camera: &Camera) -> bool {
        let size = (self.config.width.max(1), self.config.height.max(1));
        self.film_blur.wanted()
            && self.film_blur.pyramid.as_ref().is_some_and(|p| p.size == size)
            && self.scene_format() == wgpu::TextureFormat::Rgba16Float
            && matches!(camera.projection, camera::Projection::Perspective)
    }

    /// Make, remake or drop the pyramid to match the setting and the window; rebuilds the camera
    /// groups when it changes. Cheap when nothing changed: call once a frame (lib.rs).
    pub fn ensure_film_pyramid(&mut self) {
        let size = (self.config.width.max(1), self.config.height.max(1));
        let want = self.film_blur.wanted() && self.scene_format() == wgpu::TextureFormat::Rgba16Float;
        let have = self.film_blur.pyramid.as_ref().map(|p| p.size);
        if (want && have == Some(size)) || (!want && have.is_none()) {
            return;
        }
        self.film_blur.pyramid = want.then(|| {
            let p = Pyramid::new(&self.device, size);
            let (bw, bh) = base_size(size.0, size.1);
            log::info!("[FilmBlur] pyramid {bw} x {bh}, {} levels, coverage at {} x {} ({:.1} MB)", p.levels.len(), size.0, size.1, pyramid_bytes(size.0, size.1) as f64 / 1_048_576.0);
            p
        });
        self.rebuild_camera_bind_groups();
    }

    /// The far-side pass's group 0 (its own camera buffer, everything else the scene's): called by
    /// `rebuild_camera_bind_groups` beside the scene's.
    pub(crate) fn film_far_group(&self) -> wgpu::BindGroup {
        pipeline::camera_bind_group(
            &self.device,
            &self.pipeline.camera_bind_group_layout,
            "Film Far Side Camera BG",
            self.film_blur.far_camera.as_entire_buffer_binding(),
            &self.lights_buffer,
            &self.tile_counts_buffer,
            &self.tile_indices_buffer,
            &self.env_regions_buffer,
            &self.room_gi,
            self.local_shadows.colour_binding(&self.film_blur),
        )
    }

    /// The films in a see-through list: frosted film (type 27) that is see-through.
    pub(crate) fn film_objects(&self, objects: &[RenderObject]) -> Vec<usize> {
        objects
            .iter()
            .enumerate()
            .take(MAX_OBJECTS)
            .filter(|(_, o)| self.materials.get(o.material).is_some_and(|m| m.material_type >= 26.5 && m.material_type < 27.5 && m.base_color[3] < 0.999))
            .map(|(i, _)| i)
            .collect()
    }

    /// Set the camera uniform's film mode and the pyramid's numbers (the legacy light4/light5 spot
    /// slots, which no shader read before: `camera.light4_spot`, `camera.light5_spot`).
    fn film_uniform(&self, u: &mut camera::CameraUniforms, mode: f32, camera: &Camera) {
        let (w, h) = (self.config.width.max(1), self.config.height.max(1));
        let top = self.film_blur.pyramid.as_ref().map_or(0, |p| p.levels.len() as u32).saturating_sub(1);
        u.light_spot[4] = [mode, focal_texels(&camera.projection_matrix(), h), top as f32, 0.0];
        u.light_spot[5] = [1.0 / w as f32, 1.0 / h as f32, 0.0, 0.0];
    }

    /// The films' bounding spheres in view space (for `screen_rect`).
    fn film_view_spheres(&self, camera: &Camera, objects: &[RenderObject], films: &[usize]) -> Vec<(Vec3, f32)> {
        let view = camera.view_matrix();
        films
            .iter()
            .filter_map(|&i| {
                let o = objects.get(i)?;
                let [bx, by, bz, br] = self.meshes.get(o.mesh)?.bounds;
                if br < 0.0 {
                    return Some((Vec3::ZERO, f32::INFINITY));
                }
                let centre = o.position + o.rotation * (o.scale * Vec3::new(bx, by, bz));
                Some((view.transform_point3(centre), br * o.scale.abs().max_element()))
            })
            .collect()
    }

    /// Steps 1 to 3 (see the file's header), recorded into the see-through pass's own `encoder`: the
    /// films' far faces over the opaque scene, their coverage, then the colour pyramid of the scene
    /// behind them. The films' object uniforms are the see-through list's own slots (`films` index
    /// `objects`; the caller uploads them first). Returns false (and records nothing) when the blur
    /// cannot run or no film is in front of the eye. Showcase `film_v1` "1" records into an encoder
    /// of its own and submits it, as the merged version did.
    pub(crate) fn film_blur_behind(&self, encoder: &mut wgpu::CommandEncoder, camera: &Camera, objects: &[RenderObject], films: &[usize], view: &wgpu::TextureView) -> bool {
        if films.is_empty() || !self.film_blur_ready(camera) {
            return false;
        }
        let fb = &self.film_blur;
        let (Some(pyr), Some(far_group)) = (fb.pyramid.as_ref(), fb.far_group.as_ref()) else { return false };
        let Some(pso) = self.pipeline.transparent_for(ShaderClass::Surface) else { return false };
        let _cost = frame_costs::stage("cpu.film_blur");
        let mut mark = std::time::Instant::now();
        let mut split = [0f32; 4];
        let mut lap = |i: usize| {
            let now = std::time::Instant::now();
            split[i] = now.duration_since(mark).as_secs_f32() * 1.0e6;
            mark = now;
        };
        let (w, h) = pyr.size;
        let rect = if fb.v1 {
            (0, 0, w, h)
        } else {
            match screen_rect(&self.film_view_spheres(camera, objects, films), &camera.projection_matrix(), w, h) {
                Some(r) => r,
                None => return false,
            }
        };
        let mut own = fb.v1.then(|| self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Film Blur Encoder") }));
        let encoder = own.as_mut().unwrap_or(encoder);
        let mut u = self.colour_uniform(camera);
        self.film_uniform(&mut u, MODE_FAR, camera);
        self.queue.write_buffer(&fb.far_camera, 0, bytemuck::bytes_of(&u));
        lap(0);
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Film Far Side Pass"),
                timestamp_writes: self.pass_timer("gpu.film_far"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment { view, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } }),
                    self.motion_attachment(taa::MotionLoad::Load),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(pso);
            pass.set_bind_group(0, far_group, &[]);
            pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
            let mut bound_material = usize::MAX;
            for &i in films {
                let obj = &objects[i];
                let (Some(mesh), Some(material)) = (self.meshes.get(obj.mesh), self.materials.get(obj.material)) else { continue };
                pass.set_bind_group(1, &self.object_bind_group, &[256 * i as u32]);
                if bound_material != obj.material {
                    bound_material = obj.material;
                    pass.set_bind_group(2, &material.bind_group, &[]);
                    pass.set_bind_group(3, material.albedo_group().unwrap_or(&self.default_texture_bind_group), &[]);
                }
                mesh.bind_vertices(&mut pass);
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(mesh.indices(), mesh.base_vertex, 0..1);
            }
        }
        lap(1);
        // The coverage: every film's faces over the scene's depth, 1 where one is seen.
        if !fb.v1 {
            self.queue.write_buffer(&fb.cover_cam, 0, bytemuck::cast_slice(&u.view_proj));
            let shown: Vec<usize> = films.iter().copied().take(MAX_COVER_FILMS).collect();
            let models: Vec<[f32; 16]> = shown.iter().map(|&i| Mat4::from_scale_rotation_translation(objects[i].scale, objects[i].rotation, objects[i].position).to_cols_array()).collect();
            self.queue.write_buffer(&fb.cover_inst, 0, bytemuck::cast_slice(&models));
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Film Blur Coverage BG"),
                layout: &fb.cover_pso.get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry { binding: 5, resource: fb.cover_cam.as_entire_binding() }],
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Film Coverage Pass"),
                timestamp_writes: self.pass_timer("gpu.film_blur"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &pyr.mask, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.depth_view, depth_ops: None, stencil_ops: None }),
                ..Default::default()
            });
            pass.set_pipeline(&fb.cover_pso);
            pass.set_bind_group(0, &group, &[]);
            pass.set_vertex_buffer(1, fb.cover_inst.slice(..));
            for (j, &i) in shown.iter().enumerate() {
                let Some(mesh) = self.meshes.get(objects[i].mesh) else { continue };
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(mesh.indices(), mesh.base_vertex, j as u32..j as u32 + 1);
            }
        }
        lap(2);
        let timer = || self.pass_timer("gpu.film_blur");
        fb.encode_pyramid(&self.device, &self.queue, encoder, pyr, self.scene.view(), &self.depth_view, &camera.projection_matrix(), rect, fb.v1, &timer);
        if let Some(own) = own {
            self.queue.submit(std::iter::once(own.finish()));
        }
        lap(3);
        let mut avg = fb.split_avg.get();
        for (a, s) in avg.iter_mut().zip(split) {
            *a += (s - *a) * 0.05;
        }
        fb.split_avg.set(avg);
        let now = std::time::Instant::now();
        if fb.last_log.get().is_none_or(|t| now.duration_since(t).as_secs_f32() > 10.0) {
            fb.last_log.set(Some(now));
            log::info!(
                "[FilmBlur] cpu this frame (microseconds; smoothed in brackets){}: setup {:.0} ({:.0}), far sides of {} films {:.0} ({:.0}), coverage {:.0} ({:.0}), pyramid over {:?} of {} x {} {:.0} ({:.0})",
                if fb.v1 { ", film_v1" } else { "" },
                split[0],
                avg[0],
                films.len(),
                split[1],
                avg[1],
                split[2],
                avg[2],
                rect,
                w,
                h,
                split[3],
                avg[3]
            );
        }
        true
    }

    /// The camera uniform for the see-through pass: the films' near faces blend the pyramid in when
    /// `blurred`, else everything is drawn as before.
    pub(crate) fn transparent_uniform(&self, camera: &Camera, blurred: bool) -> camera::CameraUniforms {
        let mut u = self.colour_uniform(camera);
        if blurred {
            self.film_uniform(&mut u, MODE_NEAR, camera);
        }
        u
    }
}

#[cfg(test)]
#[path = "film_blur_tests.rs"]
mod tests;
