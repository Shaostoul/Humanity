//! THE INTERACTION PROBE: how far the DRAWN body is from what it touches (character animation rung
//! 2b, 2026-11-10; docs/design/character-animation.md section 17 and its proof). The foot probe's
//! pattern (engine::foot_probe): never the IK's own targets, always the mesh the GPU draws. Each near
//! person in an interaction has the vertices that touch skinned on the CPU with the very palette rows
//! the frame hands the GPU, put in the world by the same matrix, and measured against the thing:
//!
//! - `hips_m` (sitting, lying): the lowest of the vertices that rest on a seat (the hips and thighs,
//!   trousers on: `LoadedBody::seat_vertices`, `PersonMesh::seat_vertices`) within a quarter metre
//!   across of the anchor's hips contact, over the seat's (or the mattress's) height. About 0 for
//!   hips on the seat, negative through it, positive hovering over it.
//! - `left_hand_m`, `right_hand_m` (a ladder: the gap from the hand's nearest drawn vertex to the
//!   surface of the nearest rung, only while the climb has that hand on a rung, not moving between
//!   rungs; a workbench: the lowest drawn vertex of the hand over the work surface).
//! - `handle_m` (a door): the gap from the hand's nearest drawn vertex to the handle.
//! - `eye_nearest_m` (the player in first person): the nearest drawn vertex of their own body to the
//!   eye the view is drawn from. The head hidden, nothing is within the camera's near plane (0.1 m);
//!   drawn round the eye (the red twin), the inside of the head is millimetres away. `body_below_m`
//!   says a body was drawn under the eye at all (the lowest vertex under it), so a vantage with no
//!   body cannot pass by having nothing near the eye.
//!
//! Asked with the showcase verb `{"anchor_probe":"1.5"}` (seconds); it then writes
//! `debug/anchor_probe.json` for scripts/lib/anchor-probe-check.js, the vantages' `anchor_probe`
//! check. Dev tooling, permanent.

use std::collections::BTreeMap;

use glam::{Mat4, Vec3};
use humanity_core::skin_data::SkinnedModelData;

use crate::engine::animation::FigureKey;

/// Where the readout lands, relative to the working directory like every other dev IPC file.
pub(crate) const REPORT_PATH: &str = "debug/anchor_probe.json";

/// Every this-many-th vertex is measured (a hand or the hips are dense; every second one still finds
/// the nearest point to a few millimetres and keeps the probe's frames short).
pub(crate) const VERTEX_STRIDE: usize = 2;

/// How far across from the anchor's hips contact (or the hips joint) a seat vertex counts (metres):
/// the buttocks under the hips, where a seat carries a person. Wider, the first rig sweep read a
/// skirt's cloth hanging between the knees and a thigh sloping off the chair's front edge as hips
/// 11 to 13 cm into the seat, the warp's drop and the probe alike.
const SEAT_ACROSS_M: f32 = 0.10;
/// How far forward of the hips joint (the way the person faces) a seat vertex still counts, for a
/// seat (metres): the buttocks under and behind the joint carry a seated person. The thighs forward
/// of it are the legs': the seated feet solve slopes them toward the floor when the seat is higher
/// than the clip's, and their underside then dips into the cushion as a thigh does; read with them,
/// the second rig sweep had the female 2.4 cm "into" the chair with her hips on it (the headless
/// pose, 0.4 cm). Lying passes no way forward and reads the whole circle.
const SEAT_FORWARD_M: f32 = 0.03;

/// One measure over the frames it was taken.
#[derive(Debug, Clone, Default)]
pub(crate) struct Measure {
    pub frames: u32,
    pub lowest: f32,
    pub highest: f32,
    pub sum_abs: f32,
}

impl Measure {
    fn add(&mut self, v: f32) {
        if !v.is_finite() {
            return;
        }
        if self.frames == 0 {
            self.lowest = v;
            self.highest = v;
        } else {
            self.lowest = self.lowest.min(v);
            self.highest = self.highest.max(v);
        }
        self.sum_abs += v.abs();
        self.frames += 1;
    }
}

/// One person's measures, by name.
#[derive(Debug, Clone, Default)]
pub(crate) struct ProbeRow {
    pub body: String,
    pub anchor: String,
    pub kind: String,
    /// Frames in each phase (enter, hold, exit) while the probe watched.
    pub phases: [u32; 3],
    pub measures: BTreeMap<&'static str, Measure>,
}

/// The probe while it runs.
#[derive(Debug, Clone, Default)]
pub(crate) struct AnchorProbe {
    /// The runtime clock when it stops and writes.
    pub until: f32,
    pub frames: u32,
    pub rows: BTreeMap<String, ProbeRow>,
}

impl AnchorProbe {
    /// Keep one measure of one person this frame.
    pub fn note(&mut self, who: FigureKey, body: &str, anchor: &str, kind: &str, phase: u8, name: &'static str, value: Option<f32>) {
        let row = self.rows.entry(key_name(who)).or_default();
        row.body = body.to_string();
        row.anchor = anchor.to_string();
        row.kind = kind.to_string();
        if let Some(v) = value {
            row.measures.entry(name).or_default().add(v);
        }
        let _ = phase;
    }

    /// Count a frame of one person in a phase.
    pub fn phase(&mut self, who: FigureKey, phase: u8) {
        let row = self.rows.entry(key_name(who)).or_default();
        if let Some(p) = row.phases.get_mut(phase as usize) {
            *p += 1;
        }
    }
}

/// A figure's name in the readout (the foot probe's names).
pub(crate) fn key_name(k: FigureKey) -> String {
    match k {
        FigureKey::Me => "me".to_string(),
        FigureKey::Showroom => "showroom".to_string(),
        FigureKey::Player(id) => format!("player:{id}"),
        FigureKey::Crew(id) => format!("crew:{id}"),
        FigureKey::Crowd(i) => format!("crowd:{i}"),
    }
}

/// The vertices `verts` (every `VERTEX_STRIDE`-th) as the GPU draws them this frame, world metres:
/// shaped by the person's shape targets, skinned by the palette rows (the shaders' arithmetic,
/// `pose::apply_rows`), put in the world by the body's matrix.
pub(crate) fn skinned(model: &SkinnedModelData, verts: &[(usize, usize)], weights: &[(usize, f32)], rows: &[[f32; 4]], world: &Mat4, stride: usize) -> Vec<Vec3> {
    let mut out = Vec::with_capacity(verts.len() / stride.max(1) + 1);
    for &(pi, vi) in verts.iter().step_by(stride.max(1)) {
        let Some(prim) = model.primitives.get(pi) else { continue };
        let Some(v) = prim.vertices.get(vi) else { continue };
        let mut pos = Vec3::from(v.position);
        for &(t, w) in weights {
            if let Some(m) = prim.morphs.get(t) {
                pos += Vec3::from(m.positions[vi]) * w;
            }
        }
        let mut s = Vec3::ZERO;
        for k in 0..4 {
            let w = v.weights[k] as f32 / 65535.0;
            if w <= 0.0 {
                continue;
            }
            let j = v.joints[k] as usize;
            if j * 3 + 2 >= rows.len() {
                continue;
            }
            s += humanity_animation::pose::apply_rows(&[rows[j * 3], rows[j * 3 + 1], rows[j * 3 + 2]], pos) * w;
        }
        out.push(world.transform_point3(s));
    }
    out
}

/// Every drawn vertex of a mesh (those a triangle uses) as the GPU draws them, world metres, every
/// `stride`-th: the first-person eye's nearest is read from them.
pub(crate) fn skinned_drawn(model: &SkinnedModelData, weights: &[(usize, f32)], rows: &[[f32; 4]], world: &Mat4, stride: usize) -> Vec<Vec3> {
    let mut verts = Vec::new();
    for (pi, p) in model.primitives.iter().enumerate() {
        let mut drawn = vec![false; p.vertices.len()];
        for &i in &p.indices {
            if let Some(d) = drawn.get_mut(i as usize) {
                *d = true;
            }
        }
        verts.extend(drawn.iter().enumerate().filter(|(_, d)| **d).map(|(vi, _)| (pi, vi)));
    }
    skinned(model, &verts, weights, rows, world, stride)
}

/// THE SEAT'S CONTACT DROP (the motion warping's measure, not the probe's): how far under the hips
/// joint the lowest of the seat vertices stands in a pose, model metres (`rows` are the pose's
/// palette rows with no world matrix). The warp puts the hips joint this far over the seat, so the
/// flesh (trousers on), not the joint, rests on it. `forward` (model space, +Z for a seat) keeps
/// to the flesh no further forward of the joint than `SEAT_FORWARD_M`. None without seat vertices.
pub(crate) fn seat_drop(model: &SkinnedModelData, seat: &[(usize, usize)], weights: &[(usize, f32)], rows: &[[f32; 4]], hips: Vec3, forward: Option<Vec3>) -> Option<f32> {
    let pts = skinned(model, seat, weights, rows, &Mat4::IDENTITY, 1);
    let low = pts
        .iter()
        .filter(|p| (glam::Vec2::new(p.x, p.z) - glam::Vec2::new(hips.x, hips.z)).length() < SEAT_ACROSS_M)
        .filter(|p| forward.is_none_or(|f| (**p - hips).dot(f) <= SEAT_FORWARD_M))
        .map(|p| p.y)
        .fold(f32::INFINITY, f32::min);
    low.is_finite().then_some(hips.y - low)
}

/// The lowest of `pts` within `SEAT_ACROSS_M` across of the drawn hips joint `hips` (and, given the
/// way the person faces, no further forward of it than `SEAT_FORWARD_M`), over the seat's contact
/// `seat` (metres). Around the hips, not the contact: with warping off (the red twin) the clip puts
/// the hips elsewhere on the seat, and their flesh there is what rests on it or not.
pub(crate) fn hips_clearance(pts: &[Vec3], hips: Vec3, seat: Vec3, up: Vec3, forward: Option<Vec3>) -> Option<f32> {
    let flat = |v: Vec3| v - up * v.dot(up);
    pts.iter()
        .filter(|p| flat(**p - hips).length() < SEAT_ACROSS_M)
        .filter(|p| forward.is_none_or(|f| flat(**p - hips).dot(f) <= SEAT_FORWARD_M))
        .map(|p| (*p - seat).dot(up))
        .reduce(f32::min)
}

/// The gap from the nearest of `pts` to a rung (a segment from `a` to `b`, `half` thick each way),
/// metres; negative inside it.
pub(crate) fn rung_gap(pts: &[Vec3], a: Vec3, b: Vec3, half: f32) -> Option<f32> {
    let ab = b - a;
    let len2 = ab.length_squared().max(1e-8);
    pts.iter()
        .map(|p| {
            let t = ((*p - a).dot(ab) / len2).clamp(0.0, 1.0);
            (*p - (a + ab * t)).length() - half
        })
        .reduce(f32::min)
}

/// The lowest of `pts` over a surface at height `y` along up (metres).
pub(crate) fn surface_clearance(pts: &[Vec3], y: f32) -> Option<f32> {
    pts.iter().map(|p| p.y - y).reduce(f32::min)
}

/// The nearest of `pts` to `at` (metres).
pub(crate) fn nearest(pts: &[Vec3], at: Vec3) -> Option<f32> {
    pts.iter().map(|p| (*p - at).length()).reduce(f32::min)
}

/// The readout as JSON.
pub(crate) fn report(probe: &AnchorProbe) -> serde_json::Value {
    serde_json::json!({
        "frames": probe.frames,
        "people": probe.rows.iter().map(|(who, row)| serde_json::json!({
            "who": who,
            "body": row.body,
            "anchor": row.anchor,
            "kind": row.kind,
            "phase_frames": { "enter": row.phases[0], "hold": row.phases[1], "exit": row.phases[2] },
            "measures": row.measures.iter().map(|(name, m)| (name.to_string(), serde_json::json!({
                "frames": m.frames,
                "lowest_m": (m.frames > 0).then_some(m.lowest),
                "highest_m": (m.frames > 0).then_some(m.highest),
                "mean_abs_m": if m.frames > 0 { m.sum_abs / m.frames as f32 } else { 0.0 },
            }))).collect::<serde_json::Map<String, serde_json::Value>>(),
        })).collect::<Vec<_>>(),
    })
}

/// Write the readout; the line for the log.
pub(crate) fn write(probe: &AnchorProbe) -> String {
    match serde_json::to_string_pretty(&report(probe)) {
        Ok(text) => {
            let _ = std::fs::create_dir_all("debug");
            if let Err(e) = std::fs::write(REPORT_PATH, text) {
                return format!("could not write {REPORT_PATH}: {e}");
            }
            format!("{} people over {} frames -> {REPORT_PATH}", probe.rows.len(), probe.frames)
        }
        Err(e) => format!("could not serialise the interaction probe: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE PROBE'S MEASURES READ WHAT THEY SAY: a point resting on a seat reads 0 and one 3 cm into
    /// it -0.03 (points further across than 10 cm are not the seat's, nor, for a seat, a thigh's
    /// underside forward of the hips); a point touching a rung's surface reads 0 and one 5 cm off it
    /// 0.05; a hand 1 cm over a bench reads 0.01.
    /// Seen red 2026-11-10 with the rung's thickness not taken off ("a touching point reads 0.040"),
    /// and with the seat's forward filter off ("the thigh forward of the hips is the legs'").
    #[test]
    fn the_probes_measures_read_what_they_say() {
        let up = Vec3::Y;
        let seat = Vec3::new(0.0, 0.47, 0.0);
        assert!((hips_clearance(&[Vec3::new(0.05, 0.47, 0.02), Vec3::new(0.0, 0.6, 0.0)], seat, seat, up, None).unwrap()).abs() < 1e-6);
        assert!((hips_clearance(&[Vec3::new(0.05, 0.44, 0.02), Vec3::new(0.5, 0.1, 0.0)], seat, seat, up, None).unwrap() + 0.03).abs() < 1e-6, "far points are not the seat's");
        // A thigh's underside forward of the hips is not the seat's, for a person facing +Z.
        assert!((hips_clearance(&[Vec3::new(0.0, 0.47, -0.05), Vec3::new(0.0, 0.44, 0.08)], seat, seat, up, Some(Vec3::Z)).unwrap()).abs() < 1e-6, "the thigh forward of the hips is the legs'");
        let (a, b) = (Vec3::new(-0.3, 1.0, 0.0), Vec3::new(0.3, 1.0, 0.0));
        let touching = rung_gap(&[Vec3::new(0.1, 1.04, 0.0)], a, b, 0.04).unwrap();
        assert!(touching.abs() < 1e-5, "a touching point reads {touching:.3}");
        assert!((rung_gap(&[Vec3::new(0.0, 1.0, 0.09)], a, b, 0.04).unwrap() - 0.05).abs() < 1e-5);
        assert!((surface_clearance(&[Vec3::new(0.0, 0.86, 0.0), Vec3::new(0.0, 0.95, 0.0)], 0.85).unwrap() - 0.01).abs() < 1e-5);
    }
}
