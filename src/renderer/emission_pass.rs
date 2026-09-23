//! Light that lives ABOVE the cloud deck, and the pass that draws it last.
//!
//! The aurora emits between 99 and 190 km. The cloud deck sits at about 12 km.
//! From above, the clouds are BEHIND the aurora and cannot occlude it. They did,
//! because the fullscreen cloud composite runs after the transparent list that
//! carries the atmosphere shell, and it painted straight over the emission.
//! Cloud cover is regional, so the dimming wore a coastline and read for weeks
//! as an aurora that was "darker over land masses".
//!
//! Measured at lat 67, nadir, 600 km, local midnight, two fixtures differing
//! ONLY in longitude, as mean green-excess per pixel:
//!
//! | arm | before | after |
//! |---|---|---|
//! | over land | 4.288 | 7.409 |
//! | over water | 2.198 | 7.376 |
//!
//! The two-to-one gap is gone; the arms now agree to 0.4 percent.
//!
//! WHY THE COMPOSITE CANNOT SIMPLY MOVE, which is the constraint that shapes
//! this module. It must run after the dome: the cloud march already applies this
//! engine's aerial perspective at the cloud's first-hit distance, so letting the
//! dome blend over the deck applies the same air twice and the second
//! application is opaque. That was measured when it was fixed - 1.2 percent of
//! the disc written with the old order against 99.9 with this one - and the
//! comment at `atmo_over` in `engine/frame_shells.rs` says do not flip it.
//!
//! So the SHELL SPLITS instead. The air draws before the composite exactly as it
//! always did, and the same shell draws a second time afterwards carrying only
//! the emission. The two draws are told apart by the material's EMISSIVE lane
//! (`params.w` in the shader, unused by the atmosphere path, and exposed on the
//! CPU as `Material::emissive`), which means no new material type and no change
//! to `shader_class`.
//!
//! This module is where any future above-deck emission belongs too: airglow,
//! lightning, city-light bloom. They all have the same ordering problem.
//!
//! KNOWN IMPRECISION, stated rather than hidden. The emission draw shares the
//! transparent pipeline, so the blend delivers `emission + dst * (1 - alpha)`
//! where pure emission wants `emission + dst`. The difference is small while the
//! emission is faint and irrelevant where it is bright enough to dominate, but
//! it is a real approximation and an additive blend state would remove it.

use super::{pipeline, Material, RenderObject, Renderer, MAX_OBJECTS};
use crate::terrain::planet::PlanetDef;
use std::collections::HashMap;

/// True when this material is an emission twin rather than an air shell.
///
/// Used by BOTH the main transparent pass (to skip it) and this module's pass
/// (to select it), so the two can never disagree about which draw owns it.
pub fn is_emission_twin(m: &Material) -> bool {
    m.emissive > 0.5 && pipeline::shader_class(m.material_type) == pipeline::ShaderClass::Shell
}

/// The material for the emission twin of a planet's atmosphere shell.
///
/// Cached in the same map as the air shell. That map is keyed
/// `(String, bool)` and widening the tuple would touch several files, so the id
/// is mangled with a `#aurora` suffix instead, which also reads clearly in a
/// debugger.
///
/// The packing is always the PHYSICAL one even when scattering is off, because
/// the aurora path reads `params.x` (planet radius in shell units) and the
/// Fresnel fallback never carries it.
/// The emission twin of an atmosphere shell: the same mesh at the same
/// transform, carrying the emission material instead of the air one.
///
/// Takes the air shell rather than its parts so the two can never drift out
/// of alignment, and returns None when there is no shell to twin.
pub fn aurora_twin(
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
        1.0, // the emissive lane: "this draw is the emission, not the air"
    );
    cache.insert(key, m);
    m
}

impl Renderer {
    /// Draw the emission twins, after the fullscreen cloud composite.
    ///
    /// Everything here matches the celestial transparent pass - same camera
    /// group, same per-object uniform slot, depth TESTED and not written - and
    /// the only thing that differs is WHEN it runs, which is the entire point.
    ///
    /// The twins stay in the `transparent` list so their object uniforms are
    /// uploaded with everything else and they own a slot; only the draw moves,
    /// which is why the slot arithmetic here must match the upload's
    /// (`objects.len() + i`).
    ///
    /// Cheap when there is nothing to draw: the shader discards any ray whose
    /// emission is below a thousandth, so a daylit or equatorial frame pays one
    /// shell rasterisation and no region walk.
    pub(crate) fn run_emission_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        objects_len: usize,
        transparent: &[RenderObject],
    ) {
        let twins: Vec<(usize, &RenderObject)> = transparent
            .iter()
            .enumerate()
            .filter(|(_, o)| {
                self.materials
                    .get(o.material)
                    .is_some_and(is_emission_twin)
            })
            .collect();
        if twins.is_empty() {
            return;
        }
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Celestial Emission Pass"),
            timestamp_writes: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
        render_pass.set_pipeline(self.pipeline.transparent_for(pipeline::ShaderClass::Shell));
        for (i, obj) in twins {
            let slot = objects_len + i;
            if slot >= MAX_OBJECTS {
                break;
            }
            let Some(mesh) = self.meshes.get(obj.mesh) else {
                continue;
            };
            let Some(material) = self.materials.get(obj.material) else {
                continue;
            };
            render_pass.set_bind_group(1, &self.object_bind_group, &[256_u32 * (slot as u32)]);
            render_pass.set_bind_group(2, &material.bind_group, &[]);
            render_pass.set_bind_group(
                3,
                material
                    .albedo_group()
                    .unwrap_or(&self.default_texture_bind_group),
                &[],
            );
            render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }
}
