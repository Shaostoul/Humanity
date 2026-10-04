//! The pipes' marker bands in the 3D home (2026-10-04).
//!
//! `ship::pipe_marking` decides WHERE markers go along a routed run and WHICH colours each one
//! carries (from the ship's scheme, ISO 14726, and what the run carries, derived from the machine
//! it leaves and its connection kind: honest by construction). This file turns those bands into geometry: every band of one colour, across
//! every run in the home, is baked into ONE merged mesh, so the whole home's markers cost one
//! draw per colour in use (about a dozen) however many pipes there are. Each mesh keeps its
//! renderer slot and is replaced in place on a rebuild (a machine drag rebuilds every frame), so
//! nothing leaks.
//!
//! The mode (one band, or the scheme's whole marker) comes from Settings > Gameplay > Pipe
//! markings (`AppConfig::pipe_marking_full`), or from the dev showcase pin
//! `{"pipe_marking":"full"|"simplified"|"auto"}` that rig captures use without touching the
//! player's config. A change of either rebuilds the pipes (`rebuild_if_mode_changed`).

use crate::engine::state::EngineState;
use crate::renderer::mesh::{Mesh, Vertex};
use crate::ship::pipe_marking::{self, MarkingMode};
use glam::{Quat, Vec3};
use std::collections::{HashMap, HashSet};

/// The marker state the engine keeps between rebuilds.
#[derive(Default)]
pub(crate) struct PipeMarkerState {
    /// One merged mesh slot per band colour, keyed "<scheme>:<colour>".
    pub band_meshes: HashMap<String, usize>,
    /// One material per band colour, same keys.
    pub band_mats: HashMap<String, usize>,
    /// The mode the pipes were last built in (None until the first build).
    pub built_mode: Option<MarkingMode>,
    /// The dev showcase pin: Some overrides Settings for a capture, None follows Settings.
    pub pin: Option<MarkingMode>,
    /// What the last build drew, for the log and the dev IPC: (markers, bands).
    pub last_counts: (usize, usize),
}

/// The mode the pipes should be drawn in right now.
pub(crate) fn wanted_mode(state: &EngineState) -> MarkingMode {
    state
        .pipe_markers
        .pin
        .unwrap_or_else(|| MarkingMode::from_full(state.gui_state.settings.pipe_marking_full))
}

/// Rebuild the pipes when the marking mode asked for differs from the one they were built in
/// (the Settings row was flipped, or a showcase pin set or released). Cheap when nothing changed:
/// one comparison a frame. `rebuild_connection_objects` records the mode it built with even when
/// the home has no pipes, so an empty home does not rebuild every frame.
pub(crate) fn rebuild_if_mode_changed(state: &mut EngineState) {
    if state.pipe_markers.built_mode != Some(wanted_mode(state)) {
        crate::engine::home_meshes::rebuild_connection_objects(state);
    }
}

/// Band geometry gathered per colour while the runs are built, then uploaded by `flush`.
#[derive(Default)]
pub(crate) struct BandBatch {
    /// Colour key -> (vertices, indices, linear colour).
    geo: HashMap<String, (Vec<Vertex>, Vec<u32>, [f32; 4])>,
    /// Markers already placed, so runs that share a stretch of service run (many power cables
    /// leave the same battery bank together) do not stack identical bands in one spot.
    seen: HashSet<(String, [i32; 6])>,
    markers: usize,
    bands: usize,
}

impl BandBatch {
    /// Add the markers of one routed run (`points`, machine to machine) that carries `content`
    /// (`MachineHome::line_content`: what the machine it leaves puts out, else its connection
    /// kind) in a pipe of `pipe_radius`. A content the ship's scheme does not mark, or marks as
    /// deliberately unmarked, gets none (the registry test keeps every routed content covered).
    pub(crate) fn add_run(&mut self, points: &[Vec3], content: &str, pipe_radius: f32, mode: MarkingMode) {
        let reg = pipe_marking::marking();
        let Some(scheme) = reg.default_scheme() else { return };
        let Some(colours) = scheme.marker_colours(content, mode) else { return };
        if colours.is_empty() {
            return;
        }
        let rules = &reg.placement;
        let group_len = rules.band_m * colours.len() as f32;
        // 5 cm cells: two markers closer than that on the same content are the same marker.
        let q = |v: f32| (v * 20.0).round() as i32;
        for site in pipe_marking::place_markers(points, rules, group_len) {
            let (c, d) = (site.centre, site.dir);
            if !self.seen.insert((content.to_string(), [q(c.x), q(c.y), q(c.z), q(d.x), q(d.y), q(d.z)])) {
                continue;
            }
            self.markers += 1;
            for band in pipe_marking::marker_bands(&site, &colours, rules, pipe_radius) {
                let key = format!("{}:{}", scheme.id, band.colour.id);
                let entry = self
                    .geo
                    .entry(key)
                    .or_insert_with(|| (Vec::new(), Vec::new(), band.colour.linear_rgba()));
                append_sleeve(&mut entry.0, &mut entry.1, band.start, band.dir, band.len, band.radius);
                self.bands += 1;
            }
        }
    }

    /// Upload one merged mesh per colour into its own slot (replaced in place on later builds)
    /// and queue it as a connection object at the origin (its vertices are already placed).
    pub(crate) fn flush(self, state: &mut EngineState) {
        let rules = &pipe_marking::marking().placement;
        let (metallic, roughness) = (rules.band_metallic, rules.band_roughness);
        if state.pipe_markers.last_counts != (self.markers, self.bands) {
            log::info!(
                "[Pipes] {} markers, {} bands in {} colours ({:?})",
                self.markers,
                self.bands,
                self.geo.len(),
                state.pipe_markers.built_mode
            );
        }
        state.pipe_markers.last_counts = (self.markers, self.bands);
        for (key, (verts, indices, colour)) in self.geo {
            if indices.is_empty() {
                continue;
            }
            let mesh = Mesh::from_vertices(&state.renderer.device, &verts, &indices);
            let mesh_idx = match state.pipe_markers.band_meshes.get(&key) {
                Some(&slot) => {
                    state.renderer.replace_mesh(slot, mesh);
                    slot
                }
                None => {
                    let slot = state.renderer.add_mesh(mesh);
                    state.pipe_markers.band_meshes.insert(key.clone(), slot);
                    slot
                }
            };
            let mat = match state.pipe_markers.band_mats.get(&key) {
                Some(&m) => m,
                None => {
                    let m = state.renderer.add_material_full(colour, metallic, roughness, 0.0, 0.0);
                    state.pipe_markers.band_mats.insert(key, m);
                    m
                }
            };
            state.connection_objects.push((mesh_idx, mat, Vec3::ZERO, Quat::IDENTITY, Vec3::ONE));
        }
    }
}

/// Append one band, a sleeve `len` long of `radius` round the axis from `start` along `dir`, to
/// a merged mesh. The geometry is `Mesh::cylinder_data` (the same open tube the pipes are,
/// wound to face out), turned onto `dir` and moved to `start`.
fn append_sleeve(v: &mut Vec<Vertex>, ix: &mut Vec<u32>, start: Vec3, dir: Vec3, len: f32, radius: f32) {
    let (cv, ci) = Mesh::cylinder_data(radius, len, 12);
    let rot = Quat::from_rotation_arc(Vec3::Y, dir.normalize_or_zero());
    let base = v.len() as u32;
    for p in cv {
        let pos = start + rot * Vec3::from(p.position);
        let n = rot * Vec3::from(p.normal);
        v.push(Vertex { position: pos.into(), normal: n.into(), uv: p.uv });
    }
    ix.extend(ci.into_iter().map(|i| i + base));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sleeve lies along its direction from its start, at its radius, and its triangles still
    /// face outward after the turn (the winding `Mesh::cylinder_data` is tested for).
    #[test]
    fn a_band_sleeve_lies_along_its_pipe_and_faces_out() {
        let (mut v, mut ix) = (Vec::new(), Vec::new());
        let start = Vec3::new(1.0, 2.7, 3.0);
        append_sleeve(&mut v, &mut ix, start, Vec3::X, 0.06, 0.016);
        assert!(!v.is_empty() && ix.len() % 3 == 0);
        for p in &v {
            let p = Vec3::from(p.position);
            let along = (p - start).dot(Vec3::X);
            assert!((-1e-5..=0.06 + 1e-5).contains(&along), "within the band's length: {along}");
            let off_axis = (p - start - Vec3::X * along).length();
            assert!((off_axis - 0.016).abs() < 1e-4, "at the band's radius: {off_axis}");
        }
        for t in ix.chunks(3) {
            let (a, b, c) = (Vec3::from(v[t[0] as usize].position), Vec3::from(v[t[1] as usize].position), Vec3::from(v[t[2] as usize].position));
            let face_n = (b - a).cross(c - a);
            let centre = (a + b + c) / 3.0;
            let out = centre - start - Vec3::X * (centre - start).dot(Vec3::X);
            assert!(face_n.dot(out) > 0.0, "every triangle faces away from the pipe's axis");
        }
    }
}
