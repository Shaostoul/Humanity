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
//! STOREYS (ship homes increment 7, lane L). `rooms_changed` also hands the
//! renderer every SOLID floor slab (`occluder_slabs`), which keep lamp light
//! and sunlight on their own storey (renderer::room_probes, "FLOORS BETWEEN
//! STOREYS"). And a room's box is its storey's: its lid is the slab over it
//! (opaque under a solid one; under a glass one it passes what the glass and
//! the roof above it pass, as the glass roof always has), its floor above the
//! ground is the slab under it, and only the walls standing in its storey
//! face it.
//!
//! Kept out of lib.rs, which sits under the file-size ratchet: each hook there
//! is one line.

use glam::{DVec3, Vec3};

use crate::engine::state::EngineState;
use crate::renderer::room_probes::{
    merge_slabs, OccluderSlab, RoomBox, FACE_FLOOR, FACE_LID, FACE_NEG_X, FACE_NEG_Z, FACE_POS_X, FACE_POS_Z,
};
use crate::ship::fibonacci::RoomInfo;
use crate::ship::home_structure::{FloorSlab, HomeStructure, OPAQUE_CEILING_RGBA, ROOM_DIVIDER_MIN_M, STOREY_EPS};
use crate::ship::ship_structure::ShipStructure;

/// The glass material a glass floor slab is made of (`FloorSlab` carries only
/// Solid or Glass): the clear glass of the shell roof, `roof_is_glass`'s id.
pub const GLASS_SLAB_MATERIAL: u32 = 4;

/// How near a slab's top must be to a room's lid or floor height to be that
/// lid or floor (metres): the room detector's own storey tolerance.
const SLAB_LEVEL_EPS: f32 = 0.05;

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
    // The level first, so the rooms are laid out once, at the chosen detail.
    apply_detail(state);
    state.renderer.set_room_gi_rooms(boxes);
    // The floors between storeys: the same build, so a slab added or removed
    // in the editor changes what it blocks on the next frame.
    let slabs = occluder_slabs(state.gui_state.ship_structure.as_ref());
    state.renderer.set_room_gi_slabs(slabs);
}

/// Every SOLID floor slab of the ship as a box in the home (ship) frame, its
/// zone's origin added, adjacent ones that make one box merged
/// (`room_probes::merge_slabs`). A glass slab passes light and is left out
/// (`FloorSlab::passes_light`). Empty with no ship, and for every home and
/// zone without an upper floor.
pub fn occluder_slabs(ship: Option<&ShipStructure>) -> Vec<OccluderSlab> {
    let Some(ship) = ship else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for z in &ship.zones {
        let o = z.origin_vec();
        for s in z.body.floors.iter().filter(|s| !s.passes_light()) {
            let (x0, z0, x1, z1) = s.rect();
            let (y0, y1) = s.y_range();
            if x1 - x0 <= 0.0 || z1 - z0 <= 0.0 || y1 <= y0 {
                continue;
            }
            out.push(OccluderSlab { min: o + Vec3::new(x0, y0, z0), max: o + Vec3::new(x1, y1, z1) });
        }
    }
    merge_slabs(out)
}

/// Settings > Graphics > Bounced light detail (or the dev request
/// `{"bounced_light_detail":"low"}`) to the renderer: a no-op while it is
/// unchanged, a relayout of the probes and their atlas the frame it changes.
fn apply_detail(state: &mut EngineState) {
    let budget = crate::renderer::bounced_light_detail::detail_level(&state.gui_state.settings.bounced_light_detail).budget();
    state.renderer.set_room_gi_budget(budget);
}

/// Once a frame, before the scene pass (lib.rs).
pub(crate) fn before_scene(state: &mut EngineState) {
    apply_detail(state);
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
        s.zones.iter().position(|z| {
            let o = z.origin_vec();
            let top = o + Vec3::new(z.body.width, z.body.height, z.body.depth);
            r.center.cmpge(o - Vec3::splat(0.01)).all() && r.center.cmple(top + Vec3::splat(0.01)).all()
        })
    });
    let Some(z) = ship.zip(zone).map(|(s, zi)| &s.zones[zi]) else {
        return RoomBox { min, max, refl: [[DEFAULT_REFL; 3]; 6], lid_transmittance: 0.0 };
    };
    let body = &z.body;
    let o = z.origin_vec();
    let shell = surface_refl(HomeStructure::material_color(body.shell_material));
    // Only the walls standing in this room's storey face it: a wall overlaps
    // the room's height by at least what parts a storey into rooms
    // (`ROOM_DIVIDER_MIN_M`, or all of a lower room). So an upstairs room does
    // not take the ground floor's partitions under it, and a 1 m guard is not
    // a wall. With one storey every wall rises from the floor and all count.
    let need = ROOM_DIVIDER_MIN_M.min(max.y - min.y) - STOREY_EPS;
    let mut segs: Vec<Seg> = body
        .walls
        .iter()
        .filter(|w| {
            let (b, t) = w.y_range();
            (t + o.y).min(max.y) - (b + o.y).max(min.y) >= need
        })
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
    // What the roof passes: 1 - alpha for glass (what the renderer's blend
    // lets through a pane), nothing for an opaque roof.
    let roof_t = if body.roof_is_glass() { (1.0 - roof[3]).clamp(0.0, 1.0) } else { 0.0 };
    if body.roof_is_glass() {
        out.lid_transmittance = roof_t;
        out.refl[FACE_LID] = surface_refl(roof);
    } else {
        out.refl[FACE_LID] = surface_refl([roof[0], roof[1], roof[2], 1.0]);
    }
    // STOREYS: a slab is this room's lid when its top is the room's top (the
    // room detector caps a room at the floor of the storey above,
    // `detect_rooms`), and its floor when its top is the room's floor.
    let (fx0, fz0, fx1, fz1) = (min.x - o.x, min.z - o.z, max.x - o.x, max.z - o.z);
    let footprint = ((fx1 - fx0) * (fz1 - fz0)).max(1e-6);
    let overlap = |s: &FloorSlab| {
        let (x0, z0, x1, z1) = s.rect();
        (fx1.min(x1) - fx0.max(x0)).max(0.0) * (fz1.min(z1) - fz0.max(z0)).max(0.0)
    };
    let glass = HomeStructure::material_color(GLASS_SLAB_MATERIAL);
    let glass_t = (1.0 - glass[3]).clamp(0.0, 1.0);
    // What each face of a slab reflects, as `HomeStructure::generate_meshes`
    // draws it: a solid slab's TOP in the floor's material (the shell's) and
    // its UNDERSIDE in the opaque ceiling's concrete; a glass slab both ways
    // in the roof's glass, two faces that together pass what one roof pane
    // does (`glass_slab_face_alpha`), so its light is the roof glass's.
    let slab_top_refl = |s: &FloorSlab| if s.passes_light() { surface_refl(glass) } else { shell };
    let slab_under_refl = |s: &FloorSlab| if s.passes_light() { surface_refl(glass) } else { surface_refl(OPAQUE_CEILING_RGBA) };
    let lid_slabs: Vec<(&FloorSlab, f32)> = body
        .floors
        .iter()
        .filter(|s| (s.top + o.y - max.y).abs() < SLAB_LEVEL_EPS)
        .map(|s| (s, overlap(s)))
        .filter(|(_, a)| *a > 1e-4)
        .collect();
    if !lid_slabs.is_empty() {
        // The lid is the slab's UNDERSIDE, the ceiling you see, so no room box
        // ever holds a slab (`no_room_box_straddles_a_solid_slab`: the probe
        // update tests no floor). Its light: a solid slab passes none; a glass
        // one passes what the glass passes times what reaches it from above,
        // the floors of any higher storey over this room and then the roof.
        // Area-weighted over the room's footprint, the way a face takes the
        // walls behind it; a stretch no slab covers (a stairwell) passes
        // nothing straight down, which is what an opaque lid says.
        let lid_top = max.y - o.y;
        let above = |x0: f32, z0: f32, x1: f32, z1: f32| {
            let a = ((x1 - x0) * (z1 - z0)).max(1e-6);
            let mut t = roof_t;
            for s in body.floors.iter().filter(|s| s.top > lid_top + SLAB_LEVEL_EPS) {
                let (sx0, sz0, sx1, sz1) = s.rect();
                let cover = ((x1.min(sx1) - x0.max(sx0)).max(0.0) * (z1.min(sz1) - z0.max(sz0)).max(0.0) / a).min(1.0);
                t *= 1.0 - cover + cover * if s.passes_light() { glass_t } else { 0.0 };
            }
            t
        };
        let (mut t_sum, mut refl, mut covered) = (0.0f32, [0.0f32; 3], 0.0f32);
        for (s, a) in &lid_slabs {
            if s.passes_light() {
                let (x0, z0, x1, z1) = s.rect();
                t_sum += a * glass_t * above(fx0.max(x0), fz0.max(z0), fx1.min(x1), fz1.min(z1));
            }
            let r = slab_under_refl(s);
            for k in 0..3 {
                refl[k] += r[k] * a;
            }
            covered += a;
        }
        out.lid_transmittance = (t_sum / footprint).clamp(0.0, 1.0);
        out.refl[FACE_LID] = refl.map(|c| (c / covered.max(1e-6)).clamp(0.0, MAX_REFL));
        let underside = lid_slabs.iter().map(|(s, _)| s.underside() + o.y).fold(f32::INFINITY, f32::min);
        if underside < out.max.y && underside > out.min.y {
            out.max.y = underside;
        }
    }
    // An upstairs room stands on slabs: its floor reflects as they do.
    if min.y - o.y > SLAB_LEVEL_EPS {
        let (mut refl, mut covered) = ([0.0f32; 3], 0.0f32);
        for s in body.floors.iter().filter(|s| (s.top + o.y - min.y).abs() < SLAB_LEVEL_EPS) {
            let a = overlap(s);
            let r = slab_top_refl(s);
            for k in 0..3 {
                refl[k] += r[k] * a;
            }
            covered += a;
        }
        if covered > 1e-4 {
            out.refl[FACE_FLOOR] = refl.map(|c| (c / covered).clamp(0.0, MAX_REFL));
        }
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
        // Parsed directly (never `load`, which quarantines a file it rejects), then assembled
        // the way the game does it: the homestead design on the ship's default plot.
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let text = std::fs::read_to_string(data.join("blueprints/ship_structure.ron")).expect("the shipped ship file");
        let ship_file: ShipStructure = ron::from_str(&text).expect("it parses");
        // The SHIPPED design (data/homes/shipped/), never the checkout's data/homes/homestead.ron,
        // which is the developer's own home and an editor Save rewrites.
        let design = crate::ship::ship_structure::HomeDesign::built_in("homestead").expect("the shipped home design");
        let ship = ship_file.assemble(design, "p1").expect("it assembles at p1");
        let meshes = ship.generate_meshes();
        let boxes = room_boxes(Some(&ship), &meshes.room_info);
        assert_eq!(boxes.len(), meshes.room_info.len());
        // A lid may come DOWN to the underside of the slab over its room
        // (ship homes increment 7): by that slab's thickness at most.
        let drop = ship.zones.iter().flat_map(|z| z.body.floors.iter()).map(|s| s.thickness.max(0.0)).fold(0.0f32, f32::max);
        let mut glass = 0;
        for (r, b) in meshes.room_info.iter().zip(&boxes) {
            let rmin = r.center - r.dimensions * 0.5;
            let rmax = r.center + r.dimensions * 0.5;
            let rmax_lid = Vec3::new(rmax.x, rmax.y - drop, rmax.z);
            assert!(b.min.cmple(rmin + Vec3::splat(1e-4)).all() && b.max.cmpge(rmax_lid - Vec3::splat(1e-4)).all(), "{}: a face moved inward", r.id);
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

    /// The shipped ship, assembled the way the game does it (the ship file,
    /// the shipped homestead on the default plot p1, `generate_meshes`,
    /// `room_boxes`), with each room's id.
    fn shipped_rooms() -> (Vec<RoomInfo>, Vec<RoomBox>) {
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let text = std::fs::read_to_string(data.join("blueprints/ship_structure.ron")).expect("the shipped ship file");
        let ship_file: ShipStructure = ron::from_str(&text).expect("it parses");
        let design = crate::ship::ship_structure::HomeDesign::built_in("homestead").expect("the shipped home design");
        let ship = ship_file.assemble(design, "p1").expect("it assembles at p1");
        let meshes = ship.generate_meshes();
        let boxes = room_boxes(Some(&ship), &meshes.room_info);
        (meshes.room_info, boxes)
    }

    /// The shipped ship's probe lattice since probe streaming (2026-10-21):
    /// EVERY room at the level's spacing, the home and the Commons alike, and
    /// only a room that alone would hold more than a quarter of the budget
    /// (the commons hall, the street) coarsens, by itself. `--nocapture`
    /// prints every room: where the room-gi.md figures come from.
    #[test]
    fn the_shipped_ship_takes_the_level_spacing_in_every_room() {
        use crate::renderer::room_probes::{lattice_for, probe_counts, ProbeBudget, MAX_PROBES_PER_ROOM, PROBE_SPACING_M};
        let (rooms, boxes) = shipped_rooms();
        let lat = lattice_for(&boxes, ProbeBudget::default());
        let n = |c: [u32; 3]| c[0] as u64 * c[1] as u64 * c[2] as u64;
        let mut coarsened = 0;
        for (i, (r, b)) in rooms.iter().zip(&boxes).enumerate() {
            let s = b.size();
            let design = probe_counts(s, PROBE_SPACING_M);
            println!(
                "{:<28} {:>7.2} x {:>5.2} x {:>7.2} m  at 1 m {:>6}  laid out {:?} = {:>5} at {:.2} m",
                r.id, s.x, s.y, s.z, n(design), lat.counts[i], lat.probes_in(i), lat.spacing[i]
            );
            if n(design) <= MAX_PROBES_PER_ROOM as u64 {
                assert_eq!(lat.counts[i], design, "{} keeps 1 m", r.id);
            } else {
                coarsened += 1;
                assert!(lat.probes_in(i) <= MAX_PROBES_PER_ROOM, "{} within its own cap", r.id);
            }
        }
        println!("{} rooms, {} probes laid out, {coarsened} coarsened alone", boxes.len(), lat.total());
        assert!(coarsened < 5, "only the big halls and the street coarsen ({coarsened})");
    }

    /// PROBE STREAMING ON THE SHIPPED SHIP, every Bounced light detail level:
    /// standing in the home, then in the Commons, then home again, the rooms
    /// holding probes never pass the level's budget, the room the player
    /// stands in holds them within a fade of arriving, no room the budget
    /// wants is still out three fades after arriving, and every room has its
    /// ambient colour meanwhile, none dark where a lamp is (the twin's
    /// `room_ambient` with the shipped lights). `--nocapture` prints what
    /// each level holds where: the room-gi.md streaming table.
    #[test]
    fn the_shipped_ship_streams_its_probes_from_the_home_to_the_commons_and_back() {
        use crate::renderer::bounced_light_detail::detail_levels;
        use crate::renderer::room_probes::{
            assign_lights, lattice_for, room_ambient, DirectSource, Residency, LEAVE_FADE_FRAMES,
        };
        let (rooms, boxes) = shipped_rooms();
        let centre = |id: &str| {
            let i = rooms.iter().position(|r| r.id == id).unwrap_or_else(|| panic!("no room {id}: {:?}", rooms.iter().map(|r| &r.id).collect::<Vec<_>>()));
            Vec3::new(rooms[i].center.x, boxes[i].min.y + 1.7, rooms[i].center.z)
        };
        // The home's console room, and the largest room of the Commons.
        let commons_hall = rooms
            .iter()
            .zip(&boxes)
            .filter(|(r, _)| r.id.starts_with("commons:"))
            .max_by(|a, b| a.1.volume().total_cmp(&b.1.volume()))
            .map(|(r, _)| r.id.clone())
            .expect("the Commons has rooms");
        let home_room = rooms
            .iter()
            .zip(&boxes)
            .filter(|(r, _)| !r.id.contains(':') && !r.id.starts_with("corridor_"))
            .max_by(|a, b| a.1.volume().total_cmp(&b.1.volume()))
            .map(|(r, _)| r.id.clone())
            .expect("the home has rooms");
        let walk = [("home", centre(&home_room)), ("commons", centre(&commons_hall)), ("home again", centre(&home_room))];
        // The shipped lights: every room's own, as the renderer gives them.
        let ship = {
            let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
            let text = std::fs::read_to_string(data.join("blueprints/ship_structure.ron")).expect("the shipped ship file");
            let ship_file: ShipStructure = ron::from_str(&text).expect("it parses");
            let design = crate::ship::ship_structure::HomeDesign::built_in("homestead").expect("the shipped home design");
            ship_file.assemble(design, "p1").expect("it assembles at p1")
        };
        let lights = crate::engine::home_meshes::home_lights(Some(&ship), Vec::new(), true);
        let owned = assign_lights(&boxes, &lights);
        let mut dark = Vec::new();
        for (i, b) in boxes.iter().enumerate() {
            let direct = DirectSource::Scene { lights: owned[i].iter().map(|&k| lights[k]).collect(), sun: None };
            let amb = room_ambient(b, &direct, |_| Vec3::ZERO);
            if !owned[i].is_empty() && amb.max_element() <= 1.0e-4 {
                dark.push(rooms[i].id.clone());
            }
        }
        assert!(dark.is_empty(), "rooms with lamps but a dark ambient colour: {dark:?}");
        println!("{} rooms, {} lights; home room {home_room}, Commons hall {commons_hall}", boxes.len(), lights.len());
        for level in detail_levels().iter().filter(|l| l.max_probes > 0) {
            let lat = lattice_for(&boxes, level.budget());
            let probes: Vec<u32> = (0..boxes.len()).map(|i| lat.probes_in(i)).collect();
            let mut res = Residency::new(level.max_probes, boxes.len());
            let mut line = format!("{:<7} budget {:>6} of {:>6}:", level.name, level.max_probes, lat.total());
            for (what, eye) in walk {
                let here = (0..boxes.len())
                    .filter(|&i| boxes[i].contains(eye, 0.0))
                    .min_by(|&a, &b| boxes[a].volume().total_cmp(&boxes[b].volume()))
                    .expect("the eye stands in a room");
                let mut arrived = None;
                let mut waiting = Vec::new();
                for f in 0..LEAVE_FADE_FRAMES * 3 {
                    waiting = res.step(&boxes, &probes, eye).waiting;
                    assert!(res.resident_probes() <= level.max_probes, "{} {what}: {} probes held", level.id, res.resident_probes());
                    if arrived.is_none() && res.placement(here).weight >= 1.0 {
                        arrived = Some(f);
                    }
                }
                let arrived = arrived.unwrap_or(u32::MAX);
                assert!(arrived <= LEAVE_FADE_FRAMES + 2, "{} {what}: {} came in after {arrived} frames", level.id, rooms[here].id);
                let names: Vec<&str> = waiting.iter().map(|&i| rooms[i].id.as_str()).collect();
                assert!(waiting.is_empty(), "{} {what}: rooms the budget wants still out after {} frames: {names:?}", level.id, LEAVE_FADE_FRAMES * 3);
                line += &format!(
                    "  {what}: {} rooms, {} probes ({} in after {arrived} frames)",
                    res.resident_rooms(),
                    res.resident_probes(),
                    rooms[here].id
                );
            }
            println!("{line}");
        }
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

    // ── STOREYS (ship homes increment 7, lane L) ──

    use crate::renderer::light::RoomLight;
    use crate::renderer::room_probes::{layered_rows, light_anchor, light_irradiance, slabs_touched, sun_blocked_by_slabs};
    use crate::ship::ship_structure::ShipZone;

    /// The two-storey test ship: one zone, 10 x 10 x 6 m, a glass roof, a
    /// SOLID slab over the north half (z 0..5) at 3.0, a GLASS slab over the
    /// south-east quarter, the south-west quarter open (a stairwell). An oak
    /// ground wall along z 5 (up to the slab), an upstairs steel wall along
    /// x 5 standing on the slab, and an upstairs zone on each side of it (a
    /// room exists above the ground only where a zone says, so the glass
    /// quarter is a lid with no room on it, like the homestead's fields roof).
    fn two_storey_ship() -> ShipStructure {
        let body: HomeStructure = ron::from_str(
            "(width: 10.0, depth: 10.0, height: 6.0, \
             walls: [(a: (0.0, 5.0), b: (10.0, 5.0), height: 3.0, material: 3), \
                     (a: (5.0, 0.0), b: (5.0, 5.0), base: 3.0, height: 3.0)], \
             floors: [(a: (0.0, 0.0), b: (10.0, 5.0), top: 3.0), \
                      (a: (5.0, 5.0), b: (10.0, 10.0), top: 3.0, material: Glass)], \
             zones: [(id: \"up-west\", type_id: \"residential\", origin: (0.0, 3.0, 0.0), size: (5.0, 3.0, 5.0)), \
                     (id: \"up-east\", type_id: \"residential\", origin: (5.0, 3.0, 0.0), size: (5.0, 3.0, 5.0))])",
        )
        .expect("the test body parses");
        ShipStructure {
            zones: vec![ShipZone { id: "home".into(), label: String::new(), purpose: "residence".into(), origin: (0.0, 0.0, 0.0), body }],
            ..Default::default()
        }
    }

    /// The test ship's rooms, each with its probe box, found by where it is.
    fn two_storey_rooms() -> (ShipStructure, Vec<(RoomInfo, RoomBox)>) {
        let ship = two_storey_ship();
        let rooms = ship.generate_meshes().room_info;
        let boxes = room_boxes(Some(&ship), &rooms);
        (ship, rooms.into_iter().zip(boxes).collect())
    }

    fn room_at(rooms: &[(RoomInfo, RoomBox)], p: Vec3) -> &RoomBox {
        &rooms
            .iter()
            .find(|(r, _)| (r.center - p).abs().cmple(r.dimensions * 0.5).all())
            .unwrap_or_else(|| panic!("no room holds {p}: {:?}", rooms.iter().map(|(r, b)| (r.id.clone(), b.min, b.max)).collect::<Vec<_>>()))
            .1
    }

    /// ROOM BOXES PER STOREY. The ground room under the solid slab is lidded
    /// by it: opaque, its lid at the slab's underside (2.75), reflecting as
    /// that underside is drawn (the opaque ceiling's concrete). The ground room under the glass quarter and the open
    /// stairwell passes the light the glass and the roof above it pass, over
    /// the half of its footprint the glass covers. An upstairs room stands on
    /// the solid slab (its floor reflects as the slab does), keeps the roof as
    /// its lid, and does not take the oak ground wall under its +z edge for
    /// its face: only the walls of its own storey face it. Seen red first
    /// (2026-10-20) with the slab over a room left out of `room_box` (its lid
    /// filter never matching): "a solid floor is an opaque lid, left: 0.65,
    /// right: 0.0", the glass roof's transmittance through a solid floor.
    #[test]
    fn a_room_box_is_its_storeys() {
        let (ship, rooms) = two_storey_rooms();
        let body = &ship.zones[0].body;
        let shell = surface_refl(HomeStructure::material_color(body.shell_material));
        let roof = HomeStructure::material_color(body.roof_material);
        let roof_t = 1.0 - roof[3];
        let glass = HomeStructure::material_color(GLASS_SLAB_MATERIAL);
        let glass_t = 1.0 - glass[3];
        assert!(roof_t > 0.0 && glass_t > 0.0, "the test needs a glass roof and glass that passes light");

        let north = room_at(&rooms, Vec3::new(5.0, 1.0, 2.5));
        assert_eq!(north.lid_transmittance, 0.0, "a solid floor is an opaque lid");
        assert!((north.max.y - 2.75).abs() < 1e-5, "the lid is the slab's underside: {}", north.max.y);
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-5);
        assert!(
            close(north.refl[FACE_LID], surface_refl(OPAQUE_CEILING_RGBA)),
            "the lid reflects as a solid slab's underside is drawn, the opaque ceiling's concrete"
        );
        let oak = surface_refl(HomeStructure::material_color(3));
        // Overlap-weighted (`face_walls`), so equal to rounding.
        assert!(close(north.refl[FACE_POS_Z], oak), "the north ground room's +z face is the oak wall: {:?}", north.refl[FACE_POS_Z]);

        let south = room_at(&rooms, Vec3::new(5.0, 1.0, 7.5));
        let want = 0.5 * glass_t * roof_t;
        assert!((south.lid_transmittance - want).abs() < 1e-5, "half under glass: {} against {want}", south.lid_transmittance);
        assert!((south.max.y - 2.75).abs() < 1e-5);
        assert!(close(south.refl[FACE_LID], surface_refl(glass)), "the covered half is glass");

        for x in [2.5, 7.5] {
            let up = room_at(&rooms, Vec3::new(x, 4.5, 2.5));
            assert!((up.min.y - 3.0).abs() < 1e-5 && (up.max.y - 6.0).abs() < 1e-5, "upstairs is 3 to 6: {} {}", up.min.y, up.max.y);
            assert!(close(up.refl[FACE_FLOOR], shell), "the upstairs floor is the solid slab");
            assert!((up.lid_transmittance - roof_t).abs() < 1e-5, "the upstairs lid is the roof");
            assert!(close(up.refl[FACE_POS_Z], shell), "the oak wall under the upstairs room's edge is not its wall: {:?}", up.refl[FACE_POS_Z]);
        }
    }

    /// NO ROOM BOX HOLDS A SOLID SLAB, so the probe update, which traces each
    /// room's own box, needs no floor test (`room_probes::light_irradiance`):
    /// where a box's footprint overlaps a solid slab's, the box ends at or
    /// below the slab's underside or starts at or above its top. Checked on
    /// the test ship and on the homestead body carrying the upper floor's 19
    /// slabs (`homestead_with_the_upper_floor`). Seen red first (2026-10-20)
    /// with the slab over a room left out of `room_box` (so no lid and no
    /// snap): "room_1: box [0, 0, 0] to [10, 3, 4.95] straddles the slab
    /// [0, 2.75, 0] to [10, 3, 5]".
    #[test]
    fn no_room_box_straddles_a_solid_slab() {
        let (ship, rooms) = two_storey_rooms();
        let (home, home_rooms) = homestead_with_the_upper_floor();
        for (ship, rooms) in [(&ship, rooms), (&home, home_rooms)] {
            let slabs = occluder_slabs(Some(ship));
            assert!(!slabs.is_empty());
            for (r, b) in &rooms {
                for s in &slabs {
                    let overlap = b.min.x < s.max.x - 1e-3 && b.max.x > s.min.x + 1e-3 && b.min.z < s.max.z - 1e-3 && b.max.z > s.min.z + 1e-3;
                    let clear = b.max.y <= s.min.y + 1e-4 || b.min.y >= s.max.y - 1e-4;
                    assert!(!overlap || clear, "{}: box {} to {} straddles the slab {} to {}", r.id, b.min, b.max, s.min, s.max);
                }
            }
        }
    }

    /// A GLASS FLOOR PASSES LIGHT: it is a lid and a floor, but never an
    /// occluder. The solid slab is the one occluder; a lamp upstairs over the
    /// glass quarter lights the ground room under it exactly as with no floor,
    /// and the sun through the glass quarter is not blocked, while under the
    /// solid half both are. Seen red first (2026-10-20) with `occluder_slabs`
    /// keeping glass slabs: "only the solid slab occludes", the left side
    /// listing the glass quarter (5, 2.75, 5) to (10, 3, 10) as an occluder.
    #[test]
    fn a_glass_floor_passes_lamp_light_and_sunlight() {
        let ship = two_storey_ship();
        let slabs = occluder_slabs(Some(&ship));
        assert_eq!(slabs, vec![OccluderSlab { min: Vec3::new(0.0, 2.75, 0.0), max: Vec3::new(10.0, 3.0, 5.0) }], "only the solid slab occludes");
        let run = |l: &RoomLight| slabs_touched(l, &slabs).iter().map(|&i| slabs[i]).collect::<Vec<_>>();
        // A lamp over the glass, 0.4 m under the roof.
        let over_glass = RoomLight::point(Vec3::new(7.5, 5.6, 7.5), [1.0, 0.95, 0.85], 9.0, 7.0);
        let below_glass = Vec3::new(7.5, 0.0, 7.5);
        let open = light_irradiance(&over_glass, &[], below_glass, Vec3::Y);
        assert!(open.x > 0.0);
        assert_eq!(light_irradiance(&over_glass, &run(&over_glass), below_glass, Vec3::Y), open, "through the glass floor");
        // The same lamp over the solid half lights nothing under it.
        let over_solid = RoomLight::point(Vec3::new(7.5, 5.6, 2.5), [1.0, 0.95, 0.85], 9.0, 7.0);
        assert_eq!(light_irradiance(&over_solid, &run(&over_solid), Vec3::new(7.5, 0.0, 2.5), Vec3::Y), Vec3::ZERO);
        // The sun, high: blocked under the solid half, not under the glass.
        let rooms_box = (Vec3::ZERO, Vec3::new(10.0, 6.0, 10.0));
        let sun = Vec3::new(0.05, 1.0, 0.02);
        assert!(sun_blocked_by_slabs(&slabs, rooms_box, Vec3::new(7.5, 0.0, 2.5), sun));
        assert!(!sun_blocked_by_slabs(&slabs, rooms_box, below_glass, sun));
    }

    /// The homestead's body (the shipped design) with the upper floor's 19
    /// slabs at top 3.0 (docs/design/homestead-upper-floor.md section 1,
    /// slabs 1, 8 and 18 glass) and its height 6.0, alone in a ship, with its
    /// rooms and their boxes. No upstairs zones: the layout lane writes those;
    /// this is the slab layer the light work has to handle.
    fn homestead_with_the_upper_floor() -> (ShipStructure, Vec<(RoomInfo, RoomBox)>) {
        use crate::ship::home_structure::{FloorSlab, SlabMaterial};
        let mut design = crate::ship::ship_structure::HomeDesign::built_in("homestead").expect("the shipped home design");
        let layout: [(f32, f32, f32, f32, bool); 19] = [
            (0.0, 0.0, 20.0, 22.0, true),
            (20.0, 0.0, 55.0, 22.0, false),
            (0.0, 22.0, 30.1, 30.0, false),
            (30.1, 22.0, 34.0, 25.1, false),
            (31.9, 25.1, 34.0, 26.9, false),
            (30.1, 26.9, 32.5, 30.0, false),
            (32.5, 28.1, 34.0, 30.0, false),
            (34.0, 22.0, 55.0, 30.0, true),
            (0.0, 30.0, 12.0, 51.0, false),
            (12.0, 30.0, 20.0, 51.0, false),
            (20.0, 30.0, 34.0, 51.0, false),
            (34.0, 30.0, 55.0, 38.0, false),
            (34.0, 38.0, 36.5, 39.3, false),
            (40.5, 38.0, 55.0, 39.3, false),
            (34.0, 39.3, 55.0, 51.0, false),
            (0.0, 51.0, 10.0, 61.0, false),
            (0.0, 61.0, 10.0, 73.0, false),
            (0.0, 73.0, 40.0, 89.0, true),
            (40.0, 73.0, 55.0, 89.0, false),
        ];
        design.body.floors = layout
            .iter()
            .map(|&(x0, z0, x1, z1, glass)| FloorSlab {
                a: (x0, z0),
                b: (x1, z1),
                top: 3.0,
                thickness: 0.25,
                material: if glass { SlabMaterial::Glass } else { SlabMaterial::Solid },
                label: None,
            })
            .collect();
        design.body.height = 6.0;
        let ship = ShipStructure {
            zones: vec![ShipZone { id: "home".into(), label: String::new(), purpose: "residence".into(), origin: (0.0, 0.0, 0.0), body: design.body }],
            ..Default::default()
        };
        let rooms = ship.generate_meshes().room_info;
        let boxes = room_boxes(Some(&ship), &rooms);
        (ship, rooms.into_iter().zip(boxes).collect())
    }

    /// THE HOMESTEAD'S UPPER FLOOR, AS THE LIGHT SEES IT: its 16 solid slabs
    /// merge to 13 occluders, and with the home's own lights (as
    /// `home_lights` makes them) a light's run lists every one its range
    /// reaches. That is the measurement behind runs rather than the light-
    /// budget analysis' three slab slots per light: measured 2026-10-21, 49
    /// lights (a strip counts once per segment), 197 run rows layered (149
    /// slabs and 48 layer rows; 34 KB a frame with the sun's 14 rows), and 12
    /// lights whose range reaches more than
    /// three slabs, the workshop's 15 m panels up to 11 of the 13; three slots
    /// would let those light the upper floor through every unlisted slab.
    /// `--nocapture` prints each light's run and the rows a frame writes.
    /// Seen red first (2026-10-20) with the merge accepting a partial shared
    /// edge: "16 solid slabs merge to 13, left: 6, right: 13".
    #[test]
    fn the_homestead_upper_floor_fits_the_light_runs() {
        let (ship, rooms) = homestead_with_the_upper_floor();
        let slabs = occluder_slabs(Some(&ship));
        assert_eq!(slabs.len(), 13, "16 solid slabs merge to 13");
        let lights = crate::engine::home_meshes::home_lights(Some(&ship), Vec::new(), true);
        assert!(!lights.is_empty(), "the homestead carries its placed lights");
        let mut rows = 0usize;
        let mut most = 0usize;
        for l in &lights {
            let run = slabs_touched(l, &slabs);
            let touched: Vec<OccluderSlab> = run.iter().map(|&i| slabs[i]).collect();
            // What the frame writes: the run layered (`layered_rows`), a layer
            // row before each layer's slabs.
            rows += if touched.is_empty() { 0 } else { layered_rows(&touched, light_anchor(l)).len() };
            most = most.max(run.len());
            println!("light at ({:.1}, {:.1}, {:.1}) range {:>4.1}: {} slabs {:?}", l.pos.x, l.pos.y, l.pos.z, l.range, run.len(), run);
        }
        let sun_rows = layered_rows(&slabs, Vec3::ZERO).len();
        println!(
            "{} lights, {} occluder slabs ({} sun rows), {} run rows ({} bytes a frame), the longest run {} slabs; {} rooms",
            lights.len(),
            slabs.len(),
            sun_rows,
            rows,
            (sun_rows + rows) * 160,
            most,
            rooms.len()
        );
        assert!(most > 3, "some light reaches more than three slabs (the reason for runs)");
        assert!(rows + slabs.len() < crate::renderer::room_probes_gpu::MAX_SLAB_RUN_ROWS, "the home fits the run cap");
        // Every ground room under a solid slab is lidded at 2.75; the
        // greenhouse, under no slab, keeps its 6 m and the roof's light.
        let greenhouse = room_at(&rooms, Vec3::new(30.0, 1.0, 60.0));
        assert!((greenhouse.max.y - 6.0).abs() < 1e-4 && greenhouse.lid_transmittance > 0.0, "the greenhouse: {} T {}", greenhouse.max.y, greenhouse.lid_transmittance);
        let workshop = room_at(&rooms, Vec3::new(27.0, 1.0, 40.0));
        assert!((workshop.max.y - 2.75).abs() < 1e-4 && workshop.lid_transmittance == 0.0, "the workshop: {} T {}", workshop.max.y, workshop.lid_transmittance);
    }
}
