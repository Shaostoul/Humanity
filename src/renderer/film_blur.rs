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
//! > Frosted plastic is Blurred:
//! 1. THE FAR SIDES FIRST (`draw_film_far_sides`): each film's faces turned away from the eye (the
//!    cover's back and sides seen through its front) drawn as before, over the opaque scene, through
//!    the surface transparent PSO with the camera's film mode set to FAR (the near faces discard).
//! 2. THE COLOUR PYRAMID (`build_film_pyramid`, assets/shaders/film_blur.wgsl): the scene target
//!    (now holding the far sides too) downsampled to half size, each level a 13-tap downsample of the
//!    one below, up to `MAX_LEVELS`; the base's alpha holds the view depth of the nearest surface.
//!    It is the scene BEHIND the films (TAA's history would hold the films themselves, a loop).
//! 3. THE NEAR SIDES (the see-through pass as before, film mode NEAR): each film's faces toward the
//!    eye discard the far ones and blend the pyramid in (`film_frost`, 90-fragment-main.wgsl): the
//!    level whose texels match the disc's radius on screen, four bilinear taps round it.
//! Off (Simple), in a camera screen's re-render, under an orthographic camera or without the float
//! scene target, nothing of this runs: the film is drawn exactly as before.
//!
//! A KNOWN LIMIT: other see-through things behind a film (a window beyond a tent) are drawn after
//! the pyramid, so through the film they show only at the sheet's straight-through share (1 - haze).
//!
//! NOTHING KEPT ACROSS FRAMES: the pyramid is rebuilt every frame it is used; the texture is a
//! resource sized to the window, made when the setting turns on and dropped when it turns off.

use super::*;

/// The pyramid's texels: rgb the scene behind the films, a (base only) the view depth behind.
pub const PYRAMID_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The pyramid's levels at most: the base at half the render's size, then halving, so the top
/// level's texel is 64 render pixels and the widest blur about a tenth of a 1440-line screen.
pub const MAX_LEVELS: u32 = 6;
/// The camera's film mode (`camera.light4_spot.x`, a legacy uniform slot no shader read before):
/// 0 the film as before, FAR its far faces only, NEAR its near faces blended over the pyramid.
/// Large values, so no stray number in that slot can match. LOCKSTEP with 90-fragment-main.wgsl.
pub const MODE_FAR: f32 = 7001.0;
pub const MODE_NEAR: f32 = 7002.0;

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

/// The pyramid's video memory for a render of `w` x `h`.
pub fn pyramid_bytes(w: u32, h: u32) -> u64 {
    let (bw, bh) = base_size(w, h);
    (0..levels_for(w, h)).map(|i| (bw >> i).max(1) as u64 * (bh >> i).max(1) as u64 * 8).sum()
}

struct Pyramid {
    _tex: wgpu::Texture,
    view: wgpu::TextureView,
    levels: Vec<wgpu::TextureView>,
    size: (u32, u32),
}

/// The film blur's GPU state.
pub struct FilmBlur {
    /// Settings > Graphics > Frosted plastic: true = Blurred (set by lib.rs each frame).
    pub on: bool,
    /// Showcase pin `film_blur` "0" / "1" over the setting (its red twin is "0", today's look).
    pub pin: Option<bool>,
    base_pso: wgpu::RenderPipeline,
    down_pso: wgpu::RenderPipeline,
    /// Linear, clamped: the downsample's taps.
    down_sampler: wgpu::Sampler,
    /// Linear between texels and between levels, clamped: what the film's shading samples with
    /// (group 0 binding 13).
    pub read_sampler: wgpu::Sampler,
    params: Vec<wgpu::Buffer>,
    pyramid: Option<Pyramid>,
    dummy_view: wgpu::TextureView,
}

impl FilmBlur {
    pub fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Film Blur Shader"), source: wgpu::ShaderSource::Wgsl(WGSL.into()) });
        let pso = |entry: &str, label: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: None,
                vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_full"), buffers: &[], compilation_options: Default::default() },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(entry),
                    targets: &[Some(wgpu::ColorTargetState { format: PYRAMID_FORMAT, blend: None, write_mask: wgpu::ColorWrites::ALL })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };
        let base_pso = pso("fs_base", "Film Blur Base Pipeline");
        let down_pso = pso("fs_down", "Film Blur Down Pipeline");
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
        let params = (0..MAX_LEVELS)
            .map(|_| device.create_buffer(&wgpu::BufferDescriptor { label: Some("Film Blur Params"), size: 32, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }))
            .collect();
        let dummy = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Film Blur Stand-in"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: PYRAMID_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let dummy_view = dummy.create_view(&wgpu::TextureViewDescriptor::default());
        FilmBlur { on: true, pin: None, base_pso, down_pso, down_sampler, read_sampler, params, pyramid: None, dummy_view }
    }

    /// Whether the blur is asked for: the pin, else the setting.
    pub fn wanted(&self) -> bool {
        self.pin.unwrap_or(self.on)
    }

    /// What group 0 binds at 12: the pyramid (every level), or the 1x1 stand-in.
    pub fn view(&self) -> &wgpu::TextureView {
        self.pyramid.as_ref().map_or(&self.dummy_view, |p| &p.view)
    }

    /// The pyramid's video memory now (0 when it is not made).
    pub fn vram_bytes(&self) -> u64 {
        self.pyramid.as_ref().map_or(0, |p| pyramid_bytes(p.size.0, p.size.1))
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
            let (bw, bh) = base_size(size.0, size.1);
            let n = levels_for(size.0, size.1);
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Film Blur Pyramid"),
                size: wgpu::Extent3d { width: bw, height: bh, depth_or_array_layers: 1 },
                mip_level_count: n,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: PYRAMID_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            let levels = (0..n).map(|i| tex.create_view(&wgpu::TextureViewDescriptor { base_mip_level: i, mip_level_count: Some(1), ..Default::default() })).collect();
            log::info!("[FilmBlur] pyramid {bw} x {bh}, {n} levels ({:.1} MB)", pyramid_bytes(size.0, size.1) as f64 / 1_048_576.0);
            Pyramid { _tex: tex, view, levels, size }
        });
        self.rebuild_camera_bind_groups();
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

    /// Step 1 and 2 (see the file's header): the films' far faces over the opaque scene, then the
    /// colour pyramid of the scene behind the films. Called by `render_transparent_onto` for the
    /// main view when `film_blur_ready`; returns false (and draws nothing) when it is not.
    pub(crate) fn film_blur_behind(&self, camera: &Camera, objects: &[RenderObject], films: &[usize], view: &wgpu::TextureView) -> bool {
        if films.is_empty() || !self.film_blur_ready(camera) {
            return false;
        }
        let Some(pyr) = self.film_blur.pyramid.as_ref() else { return false };
        let Some(pso) = self.pipeline.transparent_for(ShaderClass::Surface) else { return false };
        let _cost = frame_costs::stage("cpu.film_blur");
        let mut u = self.colour_uniform(camera);
        self.film_uniform(&mut u, MODE_FAR, camera);
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&u));
        self.upload_object_uniforms(films.iter().map(|&i| &objects[i]));
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Film Blur Encoder") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Film Far Side Pass"),
                timestamp_writes: self.pass_timer("gpu.film_blur"),
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
            pass.set_bind_group(0, &self.camera_bind_group, &[]);
            pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
            let mut bound_material = usize::MAX;
            for (j, &i) in films.iter().enumerate() {
                let obj = &objects[i];
                let (Some(mesh), Some(material)) = (self.meshes.get(obj.mesh), self.materials.get(obj.material)) else { continue };
                pass.set_bind_group(1, &self.object_bind_group, &[256 * j as u32]);
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
        // The pyramid: the base from the scene (and the depth), then each level from the one below.
        let (w, h) = pyr.size;
        let proj = camera.projection_matrix();
        let mut src_size = (w, h);
        for (lvl, target) in pyr.levels.iter().enumerate() {
            let p: [f32; 8] = [src_size.0 as f32, src_size.1 as f32, 1.0 / src_size.0 as f32, 1.0 / src_size.1 as f32, proj.z_axis.z, proj.w_axis.z, 0.0, 0.0];
            self.queue.write_buffer(&self.film_blur.params[lvl], 0, bytemuck::cast_slice(&p));
            let (pso, group) = if lvl == 0 {
                let g = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Film Blur Base BG"),
                    layout: &self.film_blur.base_pso.get_bind_group_layout(0),
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(self.scene.view()) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.film_blur.down_sampler) },
                        wgpu::BindGroupEntry { binding: 2, resource: self.film_blur.params[lvl].as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&self.depth_view) },
                    ],
                });
                (&self.film_blur.base_pso, g)
            } else {
                let g = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Film Blur Down BG"),
                    layout: &self.film_blur.down_pso.get_bind_group_layout(0),
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&pyr.levels[lvl - 1]) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.film_blur.down_sampler) },
                        wgpu::BindGroupEntry { binding: 2, resource: self.film_blur.params[lvl].as_entire_binding() },
                    ],
                });
                (&self.film_blur.down_pso, g)
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Film Blur Pyramid Pass"),
                timestamp_writes: self.pass_timer("gpu.film_blur"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: target, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                ..Default::default()
            });
            pass.set_pipeline(pso);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
            drop(pass);
            src_size = if lvl == 0 { base_size(w, h) } else { ((src_size.0 / 2).max(1), (src_size.1 / 2).max(1)) };
        }
        self.queue.submit(std::iter::once(encoder.finish()));
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
mod tests {
    use super::*;

    /// THE BLUR GROWS WITH THE GAP: a film 1.5 m from the eye (the racks at arm's length), the
    /// tent's own spread from its data, a 60-degree lens on a 1387-line render. The blur's radius is
    /// in proportion to the gap (twice the gap, twice the radius), so a mushroom touching the film
    /// (5 mm) stays under one base texel (sharp, level 0), the shelf's front lip (5 cm) is a few
    /// texels, and the back of the tent (40 cm) is several times wider, a higher level; a surface in
    /// front of the film (a negative gap) is not blurred; the level never passes the top one.
    ///
    /// Seen red 2026-11-13 with `blur_radius_texels` written without the gap (spread * focal / z, a
    /// fixed blur): the first assertion failed, "twice the gap, twice the blur" (every gap blurred by
    /// the same 144 texels, at the first spread tried, 0.36).
    #[test]
    fn the_blur_grows_with_the_gap() {
        let spread = 0.15;
        let proj = Mat4::perspective_rh(60f32.to_radians(), 2560.0 / 1387.0, 500.0, 0.05);
        let focal = focal_texels(&proj, 1387);
        assert!((focal - 1387.0 / 4.0 / (30f32.to_radians().tan())).abs() < 0.5, "focal {focal}");
        let z = 1.5;
        let r = |gap: f32| blur_radius_texels(gap, z, spread, focal);
        assert!((r(0.4) / r(0.2) - 2.0).abs() < 1e-5, "twice the gap, twice the blur: {} texels at 20 cm, {} at 40 cm", r(0.2), r(0.4));
        assert!(r(0.005) < 1.0, "a mushroom touching the film is sharp: {}", r(0.005));
        assert!(r(0.05) > 2.0 && r(0.05) < 10.0, "a 5 cm gap blurs by {} texels and a 40 cm gap by {}", r(0.05), r(0.4));
        assert!(r(0.4) > 5.0 * r(0.05) && r(0.4) > 20.0, "the back of the tent blurs much more: {}", r(0.4));
        assert_eq!(r(-0.3), 0.0, "in front of the film: no blur");
        assert_eq!(blur_lod(r(0.005), 5.0), 0.0, "level 0");
        // Eight times the gap, three levels up (while under the top).
        let climb = blur_lod(r(0.4), 8.0) - blur_lod(r(0.05), 8.0);
        assert!((climb - 3.0).abs() < 1e-4, "the level climbs with the gap: {climb}");
        assert_eq!(blur_lod(1.0e6, 5.0), 5.0, "never past the top level");
        // Seen from twice as far, the same gap covers half the screen texels.
        assert!((blur_radius_texels(0.4, 3.0, spread, focal) - r(0.4) / 2.0).abs() < 1e-4);
        // The gap is held to the sheet's reach (the cover's depth): the room 4 m behind blurs as the
        // cover's far side 0.7 m behind does; no reach, no hold.
        assert_eq!(film_gap(4.0, 0.7), 0.7);
        assert_eq!(film_gap(0.3, 0.7), 0.3);
        assert_eq!(film_gap(4.0, 0.0), 4.0);
        // The haze from the tent's numbers: 0.65 of 0.8 scattered.
        assert!((haze([0.65, 0.8, 0.36, 0.0]) - 0.8125).abs() < 1e-6);
    }

    /// The shader's formulas are the CPU twins' and its modes are the renderer's.
    #[test]
    fn film_blur_formula_matches_the_shader() {
        let pbr = crate::renderer::shader_loader::assembled_pbr_source();
        assert!(pbr.contains("return max(gap, 0.0) * spread * focal / max(z, 0.01);"), "film_blur_radius");
        assert!(pbr.contains("return clamp(log2(max(r, 1.0)), 0.0, top);"), "film_blur_lod");
        assert!(pbr.contains("return select(gap, min(gap, reach), reach > 0.0);"), "film_gap");
        assert!(pbr.contains(&format!("const FILM_MODE_FAR: f32 = {MODE_FAR:.1};")), "MODE_FAR");
        assert!(pbr.contains(&format!("const FILM_MODE_NEAR: f32 = {MODE_NEAR:.1};")), "MODE_NEAR");
        assert!(pbr.contains("@group(0) @binding(12) var film_behind: texture_2d<f32>;"), "the pyramid's binding");
        let module = wgpu::naga::front::wgsl::parse_str(WGSL).expect("the pyramid shader parses");
        let mut v = wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(), wgpu::naga::valid::Capabilities::all());
        v.validate(&module).expect("the pyramid shader validates");
    }

    /// The pyramid's size and memory: half the render at its base, halving, at most six levels.
    #[test]
    fn the_pyramid_is_sized_from_the_render() {
        assert_eq!(base_size(2560, 1440), (1280, 720));
        assert_eq!(levels_for(2560, 1440), 6);
        assert_eq!(levels_for(8, 8), 3);
        let b = pyramid_bytes(2560, 1440);
        assert_eq!(b, (1280 * 720 + 640 * 360 + 320 * 180 + 160 * 90 + 80 * 45 + 40 * 22) as u64 * 8);
        assert!(b > 9_500_000 && b < 10_000_000, "about 9.8 MB at 2560 x 1440: {b}");
    }
}
