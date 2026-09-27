//! Light that lives ABOVE the cloud deck, and the fullscreen pass that adds it.
//!
//! PRIORITIES item 1b. The aurora emits between 99 and 190 km and the cloud
//! deck sits at about 12 km. From above, the clouds are BEHIND the aurora and
//! cannot occlude it. They did, because the fullscreen cloud composite runs
//! after the transparent list that carries the atmosphere shell, and it
//! painted straight over the emission. Cloud cover is regional, so the dimming
//! wore a coastline and read for weeks as an aurora "darker over land masses".
//!
//! WHY THE COMPOSITE CANNOT SIMPLY MOVE. It must run after the dome: the cloud
//! march already applies this engine's aerial perspective at the cloud's
//! first-hit distance, so letting the dome blend over the deck applies the same
//! air twice and the second application is opaque (1.2 percent of the disc
//! written with that order against 99.9 with this one). The comment at
//! `atmo_over` in `engine/frame_shells.rs` says do not flip it, and it is right
//! for scattered AIR. It is wrong for EMISSION that lives above the deck, and
//! this module is where that emission goes instead.
//!
//! HOW, since 2026-09-27: a FULLSCREEN ADDITIVE pass (`fs_emission_pass`,
//! assets/shaders/pbr/95-emission-pass.wgsl) that calls the ONE copy of
//! `aurora_emission` in 30-atmosphere.wgsl. It reads the camera uniform and the
//! `env_regions` storage buffer through the SHARED camera bind group (group 0),
//! and the scene depth plus a small uniform through its own group 1. It
//! replaced a second draw of the atmosphere shell (v0.1331.18) that got the
//! order right from orbit but blended OVER (so a bright curtain erased the
//! cloud behind it, and every pixel fainter than half an 8-bit step vanished,
//! because the blend rounds the source alpha that carried the emission), would
//! have painted the aurora through an overcast sky from below, and could never
//! extend the layer past the shell mesh's 191 km top.
//!
//! WHERE IN THE FRAME, which is the one decision this module makes. Altitude
//! along a straight ray has a single minimum, so:
//!
//! - camera ABOVE the emitting layer's floor: every ray meets the layer before
//!   any cloud (the near crossing is all `aurora_emission` integrates for such
//!   a camera), so the pass runs LAST, after the cloud composite, and nothing
//!   below the aurora can cover it;
//! - camera BELOW the floor (the ground, an aircraft): a ray reaches the layer
//!   only after crossing every cloud it will meet, so the pass runs BEFORE the
//!   celestial transparent pass. The dome then attenuates it by the air column
//!   between the eye and the emission (its alpha IS that column's gray
//!   transmittance), and the deck, whether the shell draws it or the composite
//!   does, covers it exactly where the deck is in front.
//!
//! This module is also where any other above-deck emission belongs (airglow,
//! lightning seen from orbit, city-light bloom): same ordering problem.
//!
//! WHICH BODY. The ovals ride the environment-region buffer, which is uploaded
//! for ONE body per frame (the cloud-bearing body in view, see
//! `engine/frame_shells.rs`). The emission pass must evaluate them around THAT
//! body and no other, so the body announces itself with a MARKER in the
//! celestial transparent list: a `RenderObject` carrying the atmosphere
//! shell's centre and radius and a material whose emissive lane is set. The
//! transparent draw loop skips it (it is never rasterised); this module reads
//! its placement. Riding the per-frame list rather than a renderer field is
//! deliberate: the list is rebuilt every frame and handed to BOTH callers of
//! `render_celestial_onto` (the live frame and the hi-res capture), so a stale
//! marker is impossible and a capture always matches the frame it copies.

use super::{pipeline, Material, RenderObject, Renderer};
use crate::terrain::planet::PlanetDef;
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;

/// GPU twin of `EmissionPassUniforms` in 95-emission-pass.wgsl. Six vec4s, no
/// implicit padding, so the Rust and WGSL layouts are the same bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct EmissionPassUniforms {
    /// xyz = eye in the celestial render frame, w = tan(fov_y / 2).
    pub cam_pos: [f32; 4],
    /// xyz = forward, w = aspect.
    pub cam_fwd: [f32; 4],
    /// xyz = right, w = m22 of the celestial reverse-Z projection.
    pub cam_right: [f32; 4],
    /// xyz = up, w = m32 of the same projection.
    pub cam_up: [f32; 4],
    /// xyz = planet centre (render frame), w = atmosphere shell radius.
    pub center: [f32; 4],
    /// x = planet radius in shell units (rp), y = top of the highest emitting
    /// layer in shell units, zw unused.
    pub layer: [f32; 4],
}

/// Everything one run of the pass needs, decided once per frame.
#[derive(Clone, Copy, Debug)]
pub struct EmissionFrame {
    pub uniforms: EmissionPassUniforms,
    /// True when the camera is below the emitting layer's floor: the pass then
    /// runs BEFORE the celestial transparent pass instead of after the cloud
    /// composite. See the module doc for why this split is exact.
    pub below_floor: bool,
}

/// The bottom and top of the emitting layers, as fractions of the atmosphere
/// shell's thickness above the surface (the units `region_kinds.ron` writes
/// them in): the lowest floor and the highest top across every band-2 region.
/// None when no region emits, which switches the whole pass off.
pub fn emitting_layer_fractions(regions: &[super::env_regions::EnvRegion]) -> Option<(f32, f32)> {
    let mut out: Option<(f32, f32)> = None;
    for r in regions {
        // Band 2 is "high, above the weather": the same test the shader's
        // region walk makes (a storm is not an aurora; an empty row has band 0).
        if (r.band - 2.0).abs() >= 0.5 || r.intensity <= 0.0 || r.kind <= 0.0 {
            continue;
        }
        let (lo, hi) = (r.params[2], r.params[3]);
        if hi <= lo {
            continue;
        }
        out = Some(match out {
            Some((a, b)) => (a.min(lo), b.max(hi)),
            None => (lo, hi),
        });
    }
    out
}

/// Which pass position a camera at `cam_r` (shell units from the planet
/// centre) needs, given the planet radius `rp` and the layer floor fraction.
/// Pure so the one decision the module makes is testable without a GPU.
pub fn camera_below_floor(cam_r: f32, rp: f32, floor_frac: f32) -> bool {
    cam_r < rp + floor_frac * (1.0 - rp)
}

/// True when this material is an emission MARKER rather than an air shell.
///
/// Used by BOTH the celestial transparent loop (to skip it) and this module
/// (to find it), so the two can never disagree about what the marker is.
pub fn is_emission_marker(m: &Material) -> bool {
    m.emissive > 0.5 && pipeline::shader_class(m.material_type) == pipeline::ShaderClass::Shell
}

/// The marker for a body whose aurora this frame should draw: the atmosphere
/// shell's placement (centre + radius) on a material that says "emission, not
/// air". Takes the air shell rather than its parts so the two can never drift
/// out of alignment, and returns None when there is no shell.
///
/// The marker keeps the shell's mesh only because a `RenderObject` must name
/// one; it is never drawn.
pub fn aurora_marker(
    renderer: &mut Renderer,
    cache: &mut HashMap<(String, bool), usize>,
    air: Option<&RenderObject>,
    body_id: &str,
    scatter: bool,
    color: [f32; 4],
    d: &PlanetDef,
) -> Option<RenderObject> {
    let air = air?;
    Some(RenderObject {
        fade: 0.0,
        position: air.position,
        rotation: air.rotation,
        scale: air.scale,
        mesh: air.mesh,
        material: aurora_material(renderer, cache, body_id, scatter, color, d),
    })
}

/// The marker's material. Cached in the same map as the air shell; that map is
/// keyed `(String, bool)`, so the id is mangled with a `#aurora` suffix, which
/// also reads clearly in a debugger. `metallic` (params.x) carries rp, the
/// planet radius in shell units, which is how the pass learns it; the packing
/// is always the PHYSICAL one, because the Fresnel fallback never carries rp.
pub fn aurora_material(
    renderer: &mut Renderer,
    cache: &mut HashMap<(String, bool), usize>,
    body_id: &str,
    scatter: bool,
    color: [f32; 4],
    d: &PlanetDef,
) -> usize {
    let key = (format!("{body_id}#aurora"), scatter);
    if let Some(&m) = cache.get(&key) {
        return m;
    }
    let (rp_ratio, h_rel) = super::atmosphere::shell_packing(
        d.atmosphere_scale,
        d.scale_height_or_default(),
        d.radius,
    );
    let m = renderer.add_material_full(
        color,
        rp_ratio,
        h_rel,
        14.0,
        1.0, // the emissive lane: "this is the emission marker, not the air"
    );
    cache.insert(key, m);
    m
}

/// The emission pass's own state on the renderer.
pub struct EmissionState {
    /// The emitting layers' floor and top (fractions of the atmosphere
    /// shell's thickness) across this frame's band-2 regions, recorded by
    /// `Renderer::set_env_regions` from the same rows the GPU gets: the floor
    /// decides which side of the frame the pass runs on, the top is its cheap
    /// rejection radius. None = nothing emits, and the pass does not run.
    pub layer: Option<(f32, f32)>,
    /// The pass's uniform (`EmissionPassUniforms`), rewritten each run.
    pub params: wgpu::Buffer,
    /// Dev switch: skip the pass entirely (`showcase {"aurora":"0"}`). A
    /// measuring instrument, not a setting: the aurora-OFF twin of a fixture
    /// subtracted from its lit twin in linear light is exactly the light the
    /// aurora delivers (scripts/aurora-gate.js).
    pub off: bool,
}

impl EmissionState {
    pub fn new(device: &wgpu::Device) -> Self {
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Emission Pass Params"),
            size: std::mem::size_of::<EmissionPassUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { layer: None, params, off: false }
    }
}

/// Upload a body's auroral ovals on their own, when its cloud deck is off,
/// and say whether it carries an emitting layer (so it gets the emission
/// marker). THE AURORA DOES NOT BELONG TO THE CLOUD DECK (2026-09-27): the
/// region upload in frame_shells runs only while the deck is on, so with
/// clouds switched off the buffer kept whatever the last deck frame left in
/// it. A boot with clouds off had no aurora at all, and one switched off
/// mid-session kept a stale weather system too.
pub fn upload_ovals_alone(
    renderer: &mut Renderer,
    kinds: Option<&super::env_regions::RegionKinds>,
    radius_km: f32,
) -> bool {
    let mut regions: Vec<super::env_regions::EnvRegion> = Vec::new();
    super::env_regions::push_auroral_ovals(&mut regions, kinds, radius_km);
    let here = emitting_layer_fractions(&regions).is_some();
    renderer.set_env_regions(&regions);
    here
}

impl Renderer {
    /// Decide this frame's emission pass, or None when there is nothing to
    /// draw: no emitting region uploaded, no marker in the list (no body with
    /// an aurora in view), or the dev switch (`showcase {"aurora":"0"}`) off.
    pub(crate) fn emission_frame(
        &self,
        camera: &super::Camera,
        transparent: &[RenderObject],
    ) -> Option<EmissionFrame> {
        if self.emission.off {
            return None;
        }
        let (floor_frac, top_frac) = self.emission.layer?;
        let marker = transparent.iter().find(|o| {
            self.materials
                .get(o.material)
                .is_some_and(is_emission_marker)
        })?;
        let rp = self.materials[marker.material].metallic.clamp(0.01, 0.9999);
        let shell_r = marker.scale.x.max(1.0e-6);
        let center = marker.position;
        let eye = camera.effective_position();
        // Roll-aware basis, the same three rows the cloud composite reads.
        let vm = camera.view_matrix();
        let fwd = -glam::Vec3::new(vm.row(2).x, vm.row(2).y, vm.row(2).z);
        let right = glam::Vec3::new(vm.row(0).x, vm.row(0).y, vm.row(0).z);
        let up = glam::Vec3::new(vm.row(1).x, vm.row(1).y, vm.row(1).z);
        let m = camera.celestial_projection().to_cols_array_2d();
        let r_top = rp + top_frac * (1.0 - rp);
        let cam_r = (eye - center).length() / shell_r;
        Some(EmissionFrame {
            uniforms: EmissionPassUniforms {
                cam_pos: [eye.x, eye.y, eye.z, (camera.fov_degrees.to_radians() * 0.5).tan()],
                cam_fwd: [fwd.x, fwd.y, fwd.z, camera.aspect],
                cam_right: [right.x, right.y, right.z, m[2][2]],
                cam_up: [up.x, up.y, up.z, m[3][2]],
                center: [center.x, center.y, center.z, shell_r],
                layer: [rp, r_top, 0.0, 0.0],
            },
            below_floor: camera_below_floor(cam_r, rp, floor_frac),
        })
    }

    /// Add the frame's above-deck emission onto `view`: one fullscreen
    /// triangle, colour blend One/One, no depth attachment (the fragment reads
    /// the scene depth and clips each ray itself). Group 0 is the SAME camera
    /// bind group every celestial draw uses, which is how the camera uniform
    /// and the `env_regions` buffer reach this pass without a copy of either.
    ///
    /// Cheap when the aurora is out of view: a ray that never reaches the top
    /// of the emitting layer leaves after one dot product, and one that does
    /// but finds no emission discards, so the target is left untouched.
    pub(crate) fn run_emission_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        frame: &EmissionFrame,
    ) {
        self.queue.write_buffer(
            &self.emission.params,
            0,
            bytemuck::bytes_of(&frame.uniforms),
        );
        let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Emission Pass Bind Group"),
            layout: &self.pipeline.emission_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.emission.params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&self.depth_view),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Emission Pass"),
            timestamp_writes: self.pass_timer("gpu.aurora"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline.emission_pass_pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_bind_group(1, &group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::env_regions::EnvRegion;

    fn region(band: f32, lo: f32, hi: f32) -> EnvRegion {
        EnvRegion {
            dir: [0.0, 1.0, 0.0],
            angular_radius: 0.4,
            kind: 8.0,
            intensity: 1.0,
            softness: 0.5,
            band,
            params: [0.385, 0.408, lo, hi],
        }
    }

    /// The Rust struct and the WGSL struct must be the same bytes: six vec4s.
    /// The WGSL side is read out of the shader so the two cannot drift.
    #[test]
    fn uniforms_match_the_shader_struct() {
        assert_eq!(std::mem::size_of::<EmissionPassUniforms>(), 96);
        let src = crate::renderer::shader_loader::assembled_pbr_source();
        let at = src.find("struct EmissionPassUniforms {").expect("WGSL struct missing");
        let body = &src[at..at + src[at..].find('}').expect("unterminated struct")];
        let fields = body.matches(": vec4<f32>,").count();
        assert_eq!(fields, 6, "EmissionPassUniforms must stay six vec4s on both sides");
        assert!(src.contains("@group(1) @binding(1) var<uniform> emission_u: EmissionPassUniforms;"));
        assert!(src.contains("@group(1) @binding(2) var emission_depth: texture_depth_2d;"));
    }

    /// The pass is the ONLY caller of aurora_emission: one copy of the body,
    /// one call site, and no second draw of the shell hiding in the air path.
    #[test]
    fn aurora_emission_lives_once_and_only_the_pass_calls_it() {
        // Code only: a comment that names the function must not count.
        let src: String = crate::renderer::shader_loader::assembled_pbr_source()
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(src.matches("fn aurora_emission(").count(), 1, "one definition");
        let calls = src.matches("aurora_emission(").count() - 1;
        assert_eq!(calls, 1, "exactly one call site");
        let pass = src.find("fn fs_emission_pass(").expect("pass entry missing");
        let call = src.find("= aurora_emission(").expect("call missing");
        assert!(call > pass, "the call must sit inside fs_emission_pass");
        assert!(
            !src.contains("aurora_only"),
            "the shell's emission branch must not come back: the pass replaced it"
        );
    }

    /// Band 2 only, lowest floor and highest top; a storm or an empty row
    /// contributes nothing, and no emitting region switches the pass off.
    #[test]
    fn layer_fractions_come_from_band_two_regions_only() {
        assert_eq!(emitting_layer_fractions(&[]), None);
        assert_eq!(emitting_layer_fractions(&[region(0.0, 0.0, 0.0)]), None);
        assert_eq!(emitting_layer_fractions(&[region(1.0, 0.1, 0.9)]), None);
        assert_eq!(
            emitting_layer_fractions(&[region(2.0, 0.52, 0.995), region(2.0, 0.40, 0.90)]),
            Some((0.40, 0.995))
        );
        let mut off = region(2.0, 0.52, 0.995);
        off.intensity = 0.0;
        assert_eq!(emitting_layer_fractions(&[off]), None);
    }

    /// Earth's numbers (region_kinds.ron: rp = 1 / 1.03, floor 0.52 of the
    /// 191 km shell, so the floor sits at 99 km): a camera at 600 km is above
    /// it and the pass runs last; a ground camera and one at 50 km are below it
    /// and the pass runs before the transparent list, so the deck covers it.
    #[test]
    fn the_pass_position_follows_the_camera_across_the_layer_floor() {
        let rp = 1.0 / 1.03_f32;
        let shell_km = 6371.0 / rp;
        let r = |alt_km: f32| (6371.0 + alt_km) / shell_km;
        assert!(!camera_below_floor(r(600.0), rp, 0.52));
        assert!(!camera_below_floor(r(150.0), rp, 0.52));
        assert!(camera_below_floor(r(50.0), rp, 0.52));
        assert!(camera_below_floor(r(0.5), rp, 0.52));
        // The boundary is the floor itself, 99 km for Earth.
        assert!(camera_below_floor(r(98.0), rp, 0.52));
        assert!(!camera_below_floor(r(101.0), rp, 0.52));
    }
}
