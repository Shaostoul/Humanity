//! Room GI glue (rung 1, 2026-09-27, docs/design/room-gi.md).
//!
//! The probe system itself lives in the renderer (`renderer::room_probes` for
//! the math and its CPU twin, `renderer::room_probes_gpu` for the atlas and
//! the update). This file is the two places the ENGINE feeds it:
//!
//! - `rooms_changed`, whenever the home is built or edited: the ship's
//!   detected rooms (`RoomInfo`, flood-filled boxes) become `RoomBox`es, each
//!   vertical face snapped out to the wall surface it stands against and given
//!   that wall's reflectance from data/blueprints/wall_materials.ron, the floor
//!   the shell material's, and the lid the roof's (a glass roof passes the
//!   light its blend passes, `1 - alpha`).
//! - `before_scene`, once a frame just before the scene pass: the station
//!   offset (render space), the eye, and the local up for the sky table when
//!   this frame has one.
//!
//! Kept out of lib.rs, which sits under the file-size ratchet: each hook there
//! is one line.

use glam::{DVec3, Vec3};

use crate::engine::state::EngineState;
use crate::renderer::room_probes::{RoomBox, FACE_FLOOR, FACE_LID, FACE_NEG_X, FACE_NEG_Z, FACE_POS_X, FACE_POS_Z};
use crate::ship::fibonacci::RoomInfo;
use crate::ship::home_structure::HomeStructure;
use crate::ship::ship_structure::ShipStructure;

/// Reflectance for a room no zone covers (a corridor tube, the legacy
/// fibonacci layout): mid grey.
pub const DEFAULT_REFL: f32 = 0.5;
/// No real surface returns more than this. It also keeps any bounce loop
/// strictly lossy, so a room with its lights off always goes dark.
pub const MAX_REFL: f32 = 0.95;
/// How far a room box face may move out to meet the wall surface behind it.
/// The flood fill works on 0.5 m cells and stops a whole cell short of a wall
/// at most, so a wall further than this belongs to something else.
pub const SNAP_REACH_M: f32 = 0.6;

/// The home was built or edited: hand the renderer the new rooms.
pub(crate) fn rooms_changed(state: &mut EngineState, room_info: &[RoomInfo]) {
    let boxes = room_boxes(state.gui_state.ship_structure.as_ref(), room_info);
    // One line per room, once per session (the first build), so the rig log
    // says what the probes traced without spamming every editor drag frame.
    static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        for (r, b) in room_info.iter().zip(&boxes) {
            log::info!(
                "[RoomGI] room {:<24} min ({:.2},{:.2},{:.2}) max ({:.2},{:.2},{:.2}) floor {:.2} walls {:.2}/{:.2}/{:.2}/{:.2} lid T {:.2}",
                r.id, b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z,
                luma(b.refl[FACE_FLOOR]), luma(b.refl[FACE_NEG_X]), luma(b.refl[FACE_POS_X]),
                luma(b.refl[FACE_NEG_Z]), luma(b.refl[FACE_POS_Z]), b.lid_transmittance
            );
        }
    }
    state.renderer.set_room_gi_rooms(boxes);
}

/// Once a frame, before the scene pass (lib.rs).
pub(crate) fn before_scene(state: &mut EngineState) {
    // The sky table's up is the celestial pass's (`aerial_up`, world frame);
    // the rooms live in the HULL frame, the same turn the sun direction takes.
    let sky_up = state.renderer.sky_view_uniform.is_some().then(|| {
        let up = state.renderer.aerial_up;
        let up = DVec3::new(up[0] as f64, up[1] as f64, up[2] as f64);
        crate::station::to_hull(state.station_ride, state.station_world_rot, up).as_vec3()
    });
    let eye = state.camera.effective_position();
    state.renderer.update_room_gi(state.station_off, eye, sky_up);
}

fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// What a surface of this material reflects diffusely: its colour, times its
/// opacity for glass (whatever the blend lets through is not reflected).
fn surface_refl(c: [f32; 4]) -> [f32; 3] {
    let a = c[3].clamp(0.0, 1.0);
    [(c[0] * a).clamp(0.0, MAX_REFL), (c[1] * a).clamp(0.0, MAX_REFL), (c[2] * a).clamp(0.0, MAX_REFL)]
}

/// One wall segment in the home frame's floor plan, with the half-thickness
/// its faces stand off the centre line and the reflectance of its exposed face.
struct Seg {
    a: (f32, f32),
    b: (f32, f32),
    half: f32,
    refl: [f32; 3],
}

/// Every room as a probe box. Rooms no zone covers get `DEFAULT_REFL` and an
/// opaque lid.
pub fn room_boxes(ship: Option<&ShipStructure>, rooms: &[RoomInfo]) -> Vec<RoomBox> {
    rooms.iter().map(|r| room_box(ship, r)).collect()
}

fn room_box(ship: Option<&ShipStructure>, r: &RoomInfo) -> RoomBox {
    let min = r.center - r.dimensions * 0.5;
    let max = r.center + r.dimensions * 0.5;
    let zone = ship.and_then(|s| {
        s.zones.iter().find(|z| {
            let o = z.origin_vec();
            let top = o + Vec3::new(z.body.width, z.body.height, z.body.depth);
            r.center.cmpge(o - Vec3::splat(0.01)).all() && r.center.cmple(top + Vec3::splat(0.01)).all()
        })
    });
    let Some(z) = zone else {
        return RoomBox { min, max, refl: [[DEFAULT_REFL; 3]; 6], lid_transmittance: 0.0 };
    };
    let body = &z.body;
    let o = z.origin_vec();
    let shell = surface_refl(HomeStructure::material_color(body.shell_material));
    let mut segs: Vec<Seg> = body
        .walls
        .iter()
        .map(|w| Seg {
            a: (w.a.0 + o.x, w.a.1 + o.z),
            b: (w.b.0 + o.x, w.b.1 + o.z),
            half: w.resolved_thickness() * 0.5,
            refl: surface_refl(HomeStructure::material_color(w.exposed_material())),
        })
        .collect();
    let (w, d) = (body.width.max(1.0), body.depth.max(1.0));
    let half = body.shell_resolved_thickness() * 0.5;
    for (a, b) in [((0.0, 0.0), (w, 0.0)), ((w, 0.0), (w, d)), ((w, d), (0.0, d)), ((0.0, d), (0.0, 0.0))] {
        segs.push(Seg { a: (a.0 + o.x, a.1 + o.z), b: (b.0 + o.x, b.1 + o.z), half, refl: shell });
    }
    let mut out = RoomBox { min, max, refl: [shell; 6], lid_transmittance: 0.0 };
    // The four walls, each from the UNSNAPPED box so the faces do not depend
    // on the order they are visited in.
    for face in [FACE_NEG_X, FACE_POS_X, FACE_NEG_Z, FACE_POS_Z] {
        let (refl, snap) = face_walls(&segs, face, min, max, shell);
        out.refl[face] = refl;
        if let Some(s) = snap {
            match face {
                FACE_NEG_X => out.min.x = out.min.x.min(s),
                FACE_POS_X => out.max.x = out.max.x.max(s),
                FACE_NEG_Z => out.min.z = out.min.z.min(s),
                _ => out.max.z = out.max.z.max(s),
            }
        }
    }
    out.refl[FACE_FLOOR] = shell;
    let roof = HomeStructure::material_color(body.roof_material);
    if body.roof_is_glass() {
        out.lid_transmittance = (1.0 - roof[3]).clamp(0.0, 1.0);
        out.refl[FACE_LID] = surface_refl(roof);
    } else {
        out.refl[FACE_LID] = surface_refl([roof[0], roof[1], roof[2], 1.0]);
    }
    out
}

/// The walls standing behind one vertical face of a room box: their
/// overlap-weighted reflectance (the zone shell's for any stretch no wall
/// covers) and the surface coordinate the face should move out to (the most
/// overlapping wall's room-side face), if any wall is within reach.
fn face_walls(segs: &[Seg], face: usize, min: Vec3, max: Vec3, shell: [f32; 3]) -> ([f32; 3], Option<f32>) {
    // along = the axis the face spans, across = the axis it faces along.
    let x_face = face == FACE_NEG_X || face == FACE_POS_X;
    let (c, lo, hi) = match face {
        FACE_NEG_X => (min.x, min.z, max.z),
        FACE_POS_X => (max.x, min.z, max.z),
        FACE_NEG_Z => (min.z, min.x, max.x),
        _ => (max.z, min.x, max.x),
    };
    let room_on_plus = face == FACE_NEG_X || face == FACE_NEG_Z;
    let span = (hi - lo).max(1e-3);
    let mut acc = [0.0f32; 3];
    let mut covered = 0.0f32;
    let mut best: Option<(f32, f32)> = None; // (overlap, surface)
    for s in segs {
        // (across coordinate of each end, along coordinate of each end)
        let (ca, cb, la, lb) = if x_face { (s.a.0, s.b.0, s.a.1, s.b.1) } else { (s.a.1, s.b.1, s.a.0, s.b.0) };
        let len = ((ca - cb).powi(2) + (la - lb).powi(2)).sqrt();
        if len < 0.05 || (ca - cb).abs() > 0.02 * len {
            continue; // not parallel to this face
        }
        let line = (ca + cb) * 0.5;
        let surface = if room_on_plus { line + s.half } else { line - s.half };
        let reach_ok = if room_on_plus {
            surface <= c + 0.05 && surface >= c - SNAP_REACH_M
        } else {
            surface >= c - 0.05 && surface <= c + SNAP_REACH_M
        };
        if !reach_ok {
            continue;
        }
        let overlap = (hi.min(la.max(lb)) - lo.max(la.min(lb))).max(0.0);
        if overlap < 0.01 {
            continue;
        }
        for k in 0..3 {
            acc[k] += s.refl[k] * overlap;
        }
        covered += overlap;
        if best.map_or(true, |(o, _)| overlap > o) {
            best = Some((overlap, surface));
        }
    }
    let bare = (span - covered).max(0.0);
    let total = covered + bare;
    let refl = [0, 1, 2].map(|k| ((acc[k] + shell[k] * bare) / total).clamp(0.0, MAX_REFL));
    (refl, best.map(|(_, s)| s))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped ship: every detected room becomes a box with sane
    /// reflectances, its faces never move more than the snap reach, and the
    /// glass-roofed home passes light through its lid.
    #[test]
    fn the_shipped_ship_yields_sane_room_boxes() {
        // Parsed directly (never `load`, which quarantines a file it rejects).
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/blueprints/ship_structure.ron");
        let text = std::fs::read_to_string(path).expect("the shipped ship structure");
        let ship: ShipStructure = ron::from_str(&text).expect("it parses");
        let meshes = ship.generate_meshes();
        let boxes = room_boxes(Some(&ship), &meshes.room_info);
        assert_eq!(boxes.len(), meshes.room_info.len());
        let mut glass = 0;
        for (r, b) in meshes.room_info.iter().zip(&boxes) {
            let rmin = r.center - r.dimensions * 0.5;
            let rmax = r.center + r.dimensions * 0.5;
            assert!(b.min.cmple(rmin + Vec3::splat(1e-4)).all() && b.max.cmpge(rmax - Vec3::splat(1e-4)).all(), "{}: a face moved inward", r.id);
            assert!((rmin - b.min).max_element() <= SNAP_REACH_M + 1e-4, "{}: a face moved too far", r.id);
            assert!((b.max - rmax).max_element() <= SNAP_REACH_M + 1e-4, "{}: a face moved too far", r.id);
            for f in b.refl {
                assert!(f.iter().all(|c| (0.0..=MAX_REFL).contains(c)), "{}: reflectance {f:?}", r.id);
            }
            if b.lid_transmittance > 0.0 {
                glass += 1;
                assert!(b.lid_transmittance < 1.0);
            }
        }
        assert!(glass > 0, "the home's glass roof reaches the probes");
    }

    /// A room beside an oak partition takes the oak's colour on that face and
    /// its face snaps out to the partition's surface; the far faces take the
    /// shell's.
    #[test]
    fn a_face_takes_the_wall_it_stands_against() {
        let segs = vec![Seg { a: (4.0, 0.0), b: (4.0, 6.0), half: 0.05, refl: [0.55, 0.40, 0.24] }];
        // Room on the -x side of the partition, its box stopping 0.3 m short.
        let (refl, snap) = face_walls(&segs, FACE_POS_X, Vec3::new(0.0, 0.0, 0.0), Vec3::new(3.65, 3.0, 6.0), [0.5; 3]);
        assert_eq!(refl, [0.55, 0.40, 0.24]);
        assert!((snap.unwrap() - 3.95).abs() < 1e-5);
        // Half covered: half oak, half shell.
        let (refl, _) = face_walls(&segs, FACE_POS_X, Vec3::new(0.0, 0.0, -6.0), Vec3::new(3.65, 3.0, 6.0), [0.5; 3]);
        assert!((refl[0] - 0.525).abs() < 1e-5);
        // A wall a metre away is not this face's wall.
        let (refl, snap) = face_walls(&segs, FACE_POS_X, Vec3::ZERO, Vec3::new(2.9, 3.0, 6.0), [0.5; 3]);
        assert_eq!((refl, snap), ([0.5; 3], None));
    }
}
