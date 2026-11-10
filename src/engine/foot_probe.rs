//! THE FOOT PROBE: how far the DRAWN feet are from the ground under them (character animation rung
//! 2a, 2026-11-09; docs/design/character-animation.md section 4 and the rung's proof).
//!
//! A rig can photograph a foot sinking into a stair tread, but a picture cannot say by how much, and
//! a check that reads the IK's own targets would only prove the setup code agrees with itself (the
//! "check that cannot fail", memory `feedback_checks_that_cannot_fail`). So this measures the mesh
//! the GPU draws: each near body's foot vertices (the shoe, `LoadedBody::foot_vertices`) are shaped
//! by the person's shape targets and skinned on the CPU with the very palette rows the frame hands
//! the GPU, put in the world by the same matrix, and each is measured against the ground under it
//! (the same ground the feet were put on, so aboard the footing and on a planet the drawn terrain).
//! The lowest foot vertex's height over the ground under it is that foot's CLEARANCE: about 0 for a
//! foot on the ground, negative for one through it, positive for one hovering.
//!
//! Asked with the showcase verb `{"foot_probe":"1.5"}` (seconds to watch; engine/ipc.rs); every
//! frame for that long it keeps each planted foot's clearance (a foot the clip says carries weight),
//! then writes `debug/foot_probe.json` for scripts/lib/foot-probe-check.js, the vantages'
//! `foot_probe` check. Dev tooling, permanent.

use glam::{Mat4, Vec3};
use humanity_animation::{FeetReport, Ground, LoadedBody};

use crate::engine::animation::{FigureKey, FootProbe, ProbeRow};

/// Where the readout lands, relative to the working directory like every other dev IPC file.
pub(crate) const REPORT_PATH: &str = "debug/foot_probe.json";

/// Every this-many-th foot vertex is measured (the sole is dense; a quarter of it still finds the
/// lowest point to well under a centimetre and keeps the probe's frames short).
const VERTEX_STRIDE: usize = 3;

/// A foot planted at least this much (the clip's contact weight) counts as carrying weight.
const PLANTED: f32 = 0.9;

/// The clearance of each foot: the lowest of its skinned vertices over the ground under it, metres
/// along `up`, or None when no ground was found under any of them.
pub(crate) fn clearances(body: &LoadedBody, weights: &[(usize, f32)], rows: &[[f32; 4]], world: &Mat4, up: Vec3, ground: &dyn Ground) -> [Option<Clearance>; 2] {
    let feet = skinned_feet(body, weights, rows, world);
    let mut out = [None, None];
    for (side, verts) in feet.iter().enumerate() {
        out[side] = verts
            .iter()
            .filter_map(|&p| ground.ground(p).map(|g| Clearance { metres: (p - g.point).dot(up), vertex: p, ground: g.point }))
            .min_by(|a, b| a.metres.total_cmp(&b.metres));
    }
    out
}

/// One foot's clearance this frame and where it was read: the lowest vertex (world) and the ground
/// point under it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Clearance {
    pub metres: f32,
    pub vertex: Vec3,
    pub ground: Vec3,
}

/// Each foot's vertices (every `VERTEX_STRIDE`-th) as the GPU draws them this frame, world metres:
/// shaped by the person's shape targets, skinned by the palette rows (the shaders' arithmetic,
/// `pose::apply_rows`), put in the world by the body's matrix.
pub(crate) fn skinned_feet(body: &LoadedBody, weights: &[(usize, f32)], rows: &[[f32; 4]], world: &Mat4) -> [Vec<Vec3>; 2] {
    let mut out: [Vec<Vec3>; 2] = Default::default();
    for (side, verts) in body.foot_vertices.iter().enumerate() {
        for &(pi, vi) in verts.iter().step_by(VERTEX_STRIDE) {
            let prim = &body.model.primitives[pi];
            let v = &prim.vertices[vi];
            let mut pos = Vec3::from(v.position);
            for &(t, w) in weights {
                if let Some(m) = prim.morphs.get(t) {
                    pos += Vec3::from(m.positions[vi]) * w;
                }
            }
            let mut skinned = Vec3::ZERO;
            for k in 0..4 {
                let w = v.weights[k] as f32 / 65535.0;
                if w <= 0.0 {
                    continue;
                }
                let j = v.joints[k] as usize;
                if j * 3 + 2 >= rows.len() {
                    continue;
                }
                skinned += humanity_animation::pose::apply_rows(&[rows[j * 3], rows[j * 3 + 1], rows[j * 3 + 2]], pos) * w;
            }
            out[side].push(world.transform_point3(skinned));
        }
    }
    out
}

/// Keep one frame of one person in the probe.
#[allow(clippy::too_many_arguments)]
pub(crate) fn record(probe: &mut FootProbe, key: FigureKey, body: &LoadedBody, tier: &'static str, mode: &'static str, clear: [Option<Clearance>; 2], feet: &FeetReport, contacts: [f32; 2], airborne: bool, up: Vec3) {
    let row = probe.rows.entry(key).or_insert_with(|| ProbeRow { body: body.choice.clone(), tier, mode, swing_low: [f32::INFINITY; 2], ground_lo: f32::INFINITY, ground_hi: f32::NEG_INFINITY, ..Default::default() });
    row.mode = mode;
    for i in 0..2 {
        let f = &feet.feet[i];
        // Where the foot found the ground, or with the feet off (the report then says nothing) the
        // ground under its lowest drawn vertex: the red arm's spread is measured too (it read 0 and
        // every red twin came out MISCONFIGURED instead of failing on its depth).
        if let Some(g) = f.ground.or(clear[i].map(|c| c.ground)) {
            let reference = *row.ground_ref.get_or_insert(g);
            let h = (g - reference).dot(up);
            row.ground_lo = row.ground_lo.min(h);
            row.ground_hi = row.ground_hi.max(h);
        }
        if f.on_tread {
            row.on_tread += 1;
        }
        let Some(cl) = clear[i] else { continue };
        let c = cl.metres;
        // The clip's own contact (with the feet off, the feet report says nothing).
        if !airborne && contacts[i] >= PLANTED {
            row.planted[i].push(c);
            if row.worst[i].is_none_or(|w| c < w.metres) {
                row.worst[i] = Some(cl);
            }
        } else {
            row.swing_low[i] = row.swing_low[i].min(c);
        }
    }
}

/// The readout as JSON.
pub(crate) fn report(probe: &FootProbe) -> serde_json::Value {
    let key_name = |k: &FigureKey| match k {
        FigureKey::Me => "me".to_string(),
        FigureKey::Showroom => "showroom".to_string(),
        FigureKey::Player(id) => format!("player:{id}"),
        FigureKey::Crew(id) => format!("crew:{id}"),
        FigureKey::Crowd(i) => format!("crowd:{i}"),
    };
    let mut people: Vec<(&FigureKey, &ProbeRow)> = probe.rows.iter().collect();
    people.sort_by_key(|(k, _)| key_name(k));
    let feet_of = |row: &ProbeRow, i: usize| {
        let v = &row.planted[i];
        let n = v.len();
        let worst_sink = v.iter().copied().fold(f32::INFINITY, f32::min);
        let worst_float = v.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let mean_abs = if n > 0 { v.iter().map(|c| c.abs()).sum::<f32>() / n as f32 } else { 0.0 };
        serde_json::json!({
            "planted_frames": n,
            "lowest_m": if n > 0 { Some(worst_sink) } else { None },
            "highest_m": if n > 0 { Some(worst_float) } else { None },
            "mean_abs_m": mean_abs,
            "swing_lowest_m": row.swing_low[i].is_finite().then_some(row.swing_low[i]),
            // Where the lowest planted reading was: the vertex (world) and the ground under it.
            "lowest_at": row.worst[i].map(|w| [w.vertex.x, w.vertex.y, w.vertex.z]),
            "lowest_ground": row.worst[i].map(|w| [w.ground.x, w.ground.y, w.ground.z]),
        })
    };
    serde_json::json!({
        "frames": probe.frames,
        "people": people.iter().map(|(k, row)| serde_json::json!({
            "who": key_name(k),
            "body": row.body,
            "tier": row.tier,
            "mode": row.mode,
            "ground_spread_m": if row.ground_hi >= row.ground_lo { row.ground_hi - row.ground_lo } else { 0.0 },
            "on_tread_frames": row.on_tread,
            "left": feet_of(row, 0),
            "right": feet_of(row, 1),
        })).collect::<Vec<_>>(),
    })
}

/// Write the readout; the line for the log.
pub(crate) fn write(probe: &FootProbe) -> String {
    let value = report(probe);
    match serde_json::to_string_pretty(&value) {
        Ok(text) => {
            let _ = std::fs::create_dir_all("debug");
            if let Err(e) = std::fs::write(REPORT_PATH, text) {
                return format!("could not write {REPORT_PATH}: {e}");
            }
            format!("{} people over {} frames -> {REPORT_PATH}", probe.rows.len(), probe.frames)
        }
        Err(e) => format!("could not serialise the foot probe: {e}"),
    }
}
