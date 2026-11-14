//! NEIGHBOURS' GARDENS DRAWN (2026-11-14, the Row Street follow-up,
//! docs/design/home-room-program.md section 10).
//!
//! WHAT WAS WRONG. A neighbour is drawn without machines (crates/humanity-ship/src/neighbours.rs),
//! so its grow beds, and the plants in them, were not drawn at all: from Row Street's open green
//! your own yard stood planted between empty lawns.
//!
//! WHAT THIS DOES. Each neighbour DESIGN's planted yard is baked ONCE, at two levels, and drawn at
//! every neighbour plot of that design near the eye, as the operator's performance note asked: "an
//! open view shows every neighbour's garden, so gardens past the near range draw through the
//! instanced, levelled vegetation path, never one draw per plant". One merged mesh a design and a
//! level (all of its beds' plants, built by the garden's own worker, `plant_pass`, through the main
//! plant pass so the stage models and materials are shared), its beds one more, and each plot
//! draws those few meshes moved to its plot: a draw a mesh a plot, whatever the plant count. Within
//! `NEAR_M` the near level (every plant the bed draws for you); out to `FAR_M` the far level (a
//! quarter of the clumps, each widened to cover the same ground: `GroupJob::cap_share`); beyond,
//! nothing. COSMETIC ONLY: a neighbour's garden is the showcase planting of its design's beds (the
//! crop data/world/showcase.ron names for the bed's machine), nothing of it is simulated, saved or
//! sent, and no garden state that syncs is read or written.
//!
//! The beds are the design's machines file's (data/machines/<kind>.ron) soil beds, the grow
//! machines whose medium has plots on the ground (data/garden/grow_media.ron); towers, fields and
//! racks are the next rung (a homestead's fields would bake millions of vertices per design).
//!
//! THE BUDGET (stated before it was built): the bake is the worker's, off the frame. A row house's
//! three beds of potatoes are about 30,000 vertices near and 8,000 far; nineteen neighbours along
//! Row Street draw at most about 19 x 3 meshes, under 0.6 M vertices, and the far level carries
//! most of them. Memory: two levels a design, a few megabytes.

use std::collections::BTreeSet;

use glam::{Quat, Vec3};
use humanity_ship::ship_structure::ShipStructure;

use crate::engine::plant_layout::{plot_rects, PlotRect};
use crate::engine::plant_pass::{CropDraw, GroupJob, GroupLayout};
use crate::engine::state::EngineState;
use crate::renderer::RenderObject;

/// The near level is drawn while a neighbour's plot is within this of the eye (metres).
pub const NEAR_M: f32 = 30.0;
/// The far level out to here; past it no neighbour's garden is drawn (the same reach as the
/// neighbours' rooms lit, `neighbour_rooms::LIT_RANGE_M`).
pub const FAR_M: f32 = 72.0;
/// The share of a plot's clumps the far level draws, each widened to cover the same ground.
pub const FAR_SHARE: f32 = 0.25;
/// The plant pass's key prefix for a neighbour design's garden: never drawn where it was built
/// (at the origin, in the design's own metres) but at each plot (`push_draws`).
pub const GROUP_PREFIX: &str = "neighbour-garden:";

/// The growth a neighbour's beds are drawn at, bed by bed in turn: well grown, not all alike.
const GROWTH: [f32; 3] = [0.55, 0.75, 0.9];

/// One soil bed of a design, in the design's own metres: its machine type, its centre at its
/// base, its turn and its footprint.
#[derive(Debug, Clone, PartialEq)]
pub struct Bed {
    pub machine: String,
    pub pos: Vec3,
    pub yaw_deg: f32,
    pub size: (f32, f32, f32),
}

/// The plant pass key of design `kind`'s garden at a level.
pub fn key(kind: &str, far: bool) -> String {
    format!("{GROUP_PREFIX}{kind}:{}", if far { "far" } else { "near" })
}

/// The soil beds of design `kind`: its machines file's grow machines whose medium has plots on the
/// ground (a medium that is not stacked and whose id ends in "_bed": data/garden/grow_media.ron).
pub fn design_beds(data_dir: &std::path::Path, kind: &str, design: &humanity_ship::ship_structure::HomeDesign, media: &[humanity_systems::grow_machines::GrowMedium]) -> Vec<Bed> {
    let Some(home) = humanity_systems::grow_machines::load_home(&data_dir.join("machines").join(format!("{kind}.ron"))) else {
        return Vec::new();
    };
    let b = &design.body;
    let zones = [humanity_ship::machines::ZoneRect { id: "home".to_string(), origin: (0.0, 0.0, 0.0), size: (b.width, b.depth, b.height) }];
    let types: std::collections::HashMap<String, String> = home.all_instances().into_iter().map(|i| (i.id, i.machine)).collect();
    home.placements(&std::collections::HashMap::new(), Some(&zones[..]))
        .into_iter()
        .filter_map(|p| {
            let machine = types.get(&p.id)?.clone();
            let medium = media.iter().find(|m| m.matches(&machine))?;
            (medium.id.ends_with("_bed") && !medium.stacked).then(|| Bed {
                machine,
                pos: Vec3::new(p.pos.0, p.pos.1, p.pos.2),
                yaw_deg: p.rotation,
                size: p.size,
            })
        })
        .collect()
}

/// One design's whole garden as ONE plant pass job: every bed's plots as rects about the design's
/// origin (a bed turned a quarter swaps its plot's sides; the rest keep theirs), one crop a plot.
pub(crate) fn garden_job(kind: &str, beds: &[Bed], crop_of: &dyn Fn(&str) -> Option<(String, u32)>, plots_of: &dyn Fn(&str) -> u32, far: bool) -> Option<GroupJob> {
    use std::hash::{Hash, Hasher};
    let mut rects: Vec<PlotRect> = Vec::new();
    let mut crops: Vec<CropDraw> = Vec::new();
    for (bi, bed) in beds.iter().enumerate() {
        let Some((crop, per_m2)) = crop_of(&bed.machine) else { continue };
        let turn = Quat::from_rotation_y(bed.yaw_deg.to_radians());
        let quarter = ((bed.yaw_deg / 90.0).round() as i32).rem_euclid(2) == 1;
        for r in plot_rects(bed.size, plots_of(&bed.machine), false) {
            let c = bed.pos + turn * Vec3::new(r.center[0], 0.0, r.center[1]);
            let size = if quarter { [r.size[1], r.size[0]] } else { r.size };
            let holds = ((size[0] * size[1]) * per_m2 as f32).round().max(1.0) as u32;
            crops.push(CropDraw { def_id: crop.clone(), slot: rects.len() as u32, t: GROWTH[bi % GROWTH.len()], wilt: 0.0, dead: false, holds });
            rects.push(PlotRect { center: [c.x, c.z], size, floor: bed.pos.y + r.floor });
        }
    }
    if crops.is_empty() {
        return None;
    }
    let key = key(kind, far);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut h);
    for (c, r) in crops.iter().zip(&rects) {
        c.def_id.hash(&mut h);
        c.holds.hash(&mut h);
        c.t.to_bits().hash(&mut h);
        [r.center[0], r.center[1], r.size[0], r.size[1], r.floor].map(f32::to_bits).hash(&mut h);
    }
    Some(GroupJob {
        key,
        sig: h.finish().max(1),
        layout: GroupLayout::Plots { pos: Vec3::ZERO, yaw_deg: 0.0, rects },
        crops,
        cap_share: if far { FAR_SHARE } else { 1.0 },
    })
}

/// The kinds of design the ship's neighbours are drawn as (the plant pass's signature reads it, so
/// a ship loaded after the first plan still plans the neighbours' gardens).
pub fn neighbour_kinds(ship: Option<&ShipStructure>) -> BTreeSet<String> {
    let Some(ship) = ship else { return BTreeSet::new() };
    humanity_ship::neighbours::neighbour_view(ship).neighbours.iter().map(|n| n.design.kind.clone()).collect()
}

/// Every neighbour design's garden jobs, both levels (`plan_plant_groups`). The crop is the one
/// data/world/showcase.ron sows in the bed's machine; the plants a plot holds come from the crop's
/// own spacing (`farming::units::plants_in_plot`, per square metre here).
pub(crate) fn jobs(state: &EngineState) -> Vec<GroupJob> {
    if state.renderer.room_gi.neighbours_off {
        return Vec::new();
    }
    let kinds = neighbour_kinds(state.gui_state.ship_structure.as_ref());
    if kinds.is_empty() {
        return Vec::new();
    }
    let data_dir = humanity_core::storage::data_dir();
    let showcase = humanity_systems::grow_machines::load_showcase(&data_dir);
    let reg = state.data_store.get::<humanity_systems::farming::PlantRegistry>("plant_registry");
    let media = &state.gui_state.grow_media;
    let crop_of = |machine: &str| -> Option<(String, u32)> {
        let crop = showcase.as_ref()?.bed_crops.get(machine)?.clone();
        let def = reg?.get(&crop)?;
        let per_m2 = humanity_systems::farming::units::plants_in_plot(def, Some(1.0)).max(1);
        Some((crop, per_m2))
    };
    let plots_of = |machine: &str| media.iter().find(|m| m.matches(machine)).map_or(1, |m| m.plots.max(1));
    let mut out = Vec::new();
    for kind in kinds {
        let Some(design) = humanity_ship::ship_structure::HomeDesign::built_in_ref(&kind) else { continue };
        let beds = design_beds(&data_dir, &kind, design, media);
        for far in [false, true] {
            out.extend(garden_job(&kind, &beds, &crop_of, &plots_of, far));
        }
    }
    out
}

/// The draw key of design `kind`'s beds (one mesh of every soil bed of the design), drawn at both
/// levels: a neighbour draws no machines, so without them its plants would stand in the air.
pub fn bed_key(kind: &str) -> String {
    format!("{GROUP_PREFIX}{kind}:beds")
}

/// Every bed of `beds` as one box mesh in the design's own metres: each its machine's footprint
/// and height, turned by its yaw, built the way a wall box is (`fibonacci::wall_box`'s faces and
/// winding).
pub fn bed_boxes(beds: &[Bed]) -> (Vec<humanity_core::vertex::Vertex>, Vec<u32>) {
    use humanity_core::vertex::Vertex;
    let (mut vs, mut is) = (Vec::new(), Vec::new());
    for bed in beds {
        let turn = Quat::from_rotation_y(bed.yaw_deg.to_radians());
        let dir = turn * Vec3::X;
        let perp = Vec3::new(-dir.z, 0.0, dir.x);
        let (w, h, d) = bed.size;
        let (start, end) = (bed.pos - dir * (w * 0.5), bed.pos + dir * (w * 0.5));
        let (p0, p1, p2, p3) = (start - perp * (d * 0.5), start + perp * (d * 0.5), end + perp * (d * 0.5), end - perp * (d * 0.5));
        let (y0, y1) = (bed.pos.y, bed.pos.y + h);
        let v = |p: Vec3, y: f32, n: Vec3| Vertex { position: [p.x, y, p.z], normal: n.to_array(), uv: [0.0, 0.0] };
        let base = vs.len() as u32;
        vs.extend([
            v(p0, y0, Vec3::NEG_Y), v(p1, y0, Vec3::NEG_Y), v(p2, y0, Vec3::NEG_Y), v(p3, y0, Vec3::NEG_Y),
            v(p0, y1, Vec3::Y), v(p1, y1, Vec3::Y), v(p2, y1, Vec3::Y), v(p3, y1, Vec3::Y),
            v(p1, y0, perp), v(p2, y0, perp), v(p2, y1, perp), v(p1, y1, perp),
            v(p3, y0, -perp), v(p0, y0, -perp), v(p0, y1, -perp), v(p3, y1, -perp),
            v(p0, y0, -dir), v(p1, y0, -dir), v(p1, y1, -dir), v(p0, y1, -dir),
            v(p2, y0, dir), v(p3, y0, dir), v(p3, y1, dir), v(p2, y1, dir),
        ]);
        is.extend(
            [0u32, 1, 2, 0, 2, 3, 4, 6, 5, 4, 7, 6, 8, 9, 10, 8, 10, 11, 12, 13, 14, 12, 14, 15, 16, 17, 18, 16, 18, 19, 20, 21, 22, 20, 22, 23]
                .map(|k| base + k),
        );
    }
    (vs, is)
}

/// Make (or, with the reuse switch off, make again) each neighbour design's bed mesh and its
/// material, once a frame before the plants are drawn (`home_meshes::rebuild_plant_meshes`).
pub(crate) fn prepare(state: &mut EngineState) {
    if state.renderer.room_gi.neighbours_off {
        return;
    }
    let kinds = neighbour_kinds(state.gui_state.ship_structure.as_ref());
    let reuse = humanity_core::reuse::enabled();
    let data_dir = humanity_core::storage::data_dir();
    for kind in kinds {
        let have = BEDS.with(|b| b.borrow().get(&kind).copied());
        if have.is_some() && reuse {
            continue;
        }
        let Some(design) = humanity_ship::ship_structure::HomeDesign::built_in_ref(&kind) else { continue };
        let beds = design_beds(&data_dir, &kind, design, &state.gui_state.grow_media);
        if beds.is_empty() {
            continue;
        }
        let (v, i) = bed_boxes(&beds);
        let mesh = crate::renderer::mesh::Mesh::from_vertices(&state.renderer.device, &v, &i);
        let slot = match have {
            Some((mi, ma)) => {
                state.renderer.replace_mesh(mi, mesh);
                (mi, ma)
            }
            None => {
                // The machine's own colour, as a bed you place is drawn (home_meshes, the machines).
                let c = design_bed_colour(&data_dir, &kind);
                let ma = state.renderer.add_material_typed([c[0], c[1], c[2], 1.0], 0.1, 0.7, 0.0);
                (state.renderer.add_mesh(mesh), ma)
            }
        };
        BEDS.with(|b| b.borrow_mut().insert(kind, slot));
    }
}

thread_local! {
    /// Each design's bed mesh and material (`prepare`), by design kind.
    static BEDS: std::cell::RefCell<std::collections::HashMap<String, (usize, usize)>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// The colour of design `kind`'s first soil bed machine (its machines file), else a timber brown.
fn design_bed_colour(data_dir: &std::path::Path, kind: &str) -> [f32; 3] {
    humanity_systems::grow_machines::load_home(&data_dir.join("machines").join(format!("{kind}.ron")))
        .and_then(|h| h.catalog.iter().find(|(k, _)| k.ends_with("_bed")).map(|(_, d)| [d.color.0, d.color.1, d.color.2]))
        .unwrap_or([0.55, 0.42, 0.25])
}

/// Which level of a neighbour's garden is drawn with its plot `d` metres from the eye: Some(false)
/// near, Some(true) far, None not at all.
pub fn level_at(d: f32) -> Option<bool> {
    if d <= NEAR_M {
        Some(false)
    } else if d <= FAR_M {
        Some(true)
    } else {
        None
    }
}

/// This frame's neighbours' gardens into `out`: for each neighbour plot within `FAR_M` of the eye,
/// its design's garden at the level its distance picks (`level_at`), moved to its plot.
/// `slots(key)` is the plant pass's meshes of a key (mesh, material).
pub fn draws(ship: &ShipStructure, eye: Vec3, slots: &dyn Fn(&str) -> Option<Vec<(usize, usize)>>, out: &mut Vec<RenderObject>) {
    let view = humanity_ship::neighbours::neighbour_view(ship);
    for n in &view.neighbours {
        let o = Vec3::new(n.plot.origin.0, n.plot.origin.1, n.plot.origin.2);
        let s = Vec3::new(n.plot.size.0, n.plot.size.1, n.plot.size.2);
        let d = (o - eye).max(eye - (o + s)).max(Vec3::ZERO).length();
        let Some(far) = level_at(d) else { continue };
        let Some(mut meshes) = slots(&key(&n.design.kind, far)) else { continue };
        meshes.extend(slots(&bed_key(&n.design.kind)).unwrap_or_default());
        out.extend(meshes.into_iter().map(|(mesh, material)| RenderObject {
            fade: 0.0,
            position: o,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
            mesh,
            material,
        }));
    }
}

/// The neighbours' gardens to draw this frame (`home_meshes::push_home_plants`): their plants (the
/// plant pass's meshes of `GROUP_PREFIX` keys) at every neighbour plot near the eye.
pub(crate) fn push_draws(state: &EngineState, out: &mut Vec<RenderObject>) {
    let Some(ship) = state.gui_state.ship_structure.as_ref() else { return };
    if state.renderer.room_gi.neighbours_off {
        return;
    }
    let eye = state.camera.effective_position() - state.station_off;
    let pass = &state.garden_draw.plants;
    let slots = |k: &str| -> Option<Vec<(usize, usize)>> {
        match k.strip_suffix(":beds").and_then(|s| s.strip_prefix(GROUP_PREFIX)) {
            Some(kind) => BEDS.with(|b| b.borrow().get(kind).map(|&s| vec![s])),
            None => pass.slots_of(k),
        }
    };
    draws(ship, eye, &slots, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row_ship() -> ShipStructure {
        let data = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
        let ship = ShipStructure::load_ship_file(&data).expect("the shipped ship file");
        let design = humanity_ship::ship_structure::HomeDesign::built_in("row_house_two_bedroom").expect("the shipped row house");
        ship.assemble(design, "r1").expect("it assembles at r1")
    }

    /// A NEIGHBOUR'S BED DRAWS (2026-11-14, the Row Street follow-up). From the green behind r10,
    /// with each design's garden built, a neighbour plot within the near range draws its design's
    /// near garden at its plot (r3, r6), one farther out the far garden (r2), and none past
    /// `FAR_M`; the two-bedroom design's garden is its three soil beds' six plots of the showcase's
    /// potato, and the far level asks for a quarter of the clumps. Measured on what is DRAWN (the
    /// render objects and where they stand), not on what was asked.
    ///
    /// Seen red 2026-11-14 with `draws` pushing nothing: "r3's garden is not drawn from the green
    /// behind it".
    #[test]
    fn a_neighbour_s_bed_draws() {
        let ship = row_ship();
        let data = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
        let media = humanity_systems::grow_machines::load_grow_media(&data);
        let design = humanity_ship::ship_structure::HomeDesign::built_in_ref("row_house_two_bedroom").unwrap();
        let beds = design_beds(&data, "row_house_two_bedroom", design, &media);
        assert_eq!(beds.len(), 3, "the two-bedroom's yard holds three soil beds: {beds:?}");
        assert!(beds.iter().all(|b| b.pos.x > 10.0 && b.pos.x < 18.0), "every bed stands in the yard (x 10..18): {beds:?}");
        let crop_of = |_: &str| Some(("potato".to_string(), 4u32));
        let plots_of = |_: &str| 2u32;
        let near = garden_job("row_house_two_bedroom", &beds, &crop_of, &plots_of, false).expect("a near garden");
        let far = garden_job("row_house_two_bedroom", &beds, &crop_of, &plots_of, true).expect("a far garden");
        assert_eq!(near.crops.len(), 6, "six plots, one crop each");
        assert_eq!((near.cap_share, far.cap_share), (1.0, FAR_SHARE));
        assert_ne!(near.key, far.key);
        // Meshes 7 and 8 stand for the built near and far gardens.
        let slots = |k: &str| -> Option<Vec<(usize, usize)>> {
            if k == key("row_house_two_bedroom", false) {
                Some(vec![(7, 1)])
            } else if k == key("row_house_two_bedroom", true) {
                Some(vec![(8, 1)])
            } else {
                None
            }
        };
        // On the green behind r10: r3 and r6 (two-bedroom) within the near range, r2 past it.
        let eye = Vec3::new(117.0, 1.7, 345.0);
        let mut out = Vec::new();
        draws(&ship, eye, &slots, &mut out);
        let plot = |id: &str| ship.plots.iter().find(|p| p.id == id).cloned().unwrap();
        let dist = |id: &str| {
            let p = plot(id);
            let (o, s) = (Vec3::new(p.origin.0, p.origin.1, p.origin.2), Vec3::new(p.size.0, p.size.1, p.size.2));
            (o - eye).max(eye - (o + s)).max(Vec3::ZERO).length()
        };
        let at = |id: &str| {
            let p = plot(id);
            out.iter().filter(move |o| o.position == Vec3::new(p.origin.0, p.origin.1, p.origin.2)).map(|o| o.mesh).collect::<Vec<_>>()
        };
        assert_eq!(at("r3"), vec![7], "r3's garden is not drawn from the green behind it");
        assert_eq!(at("r6"), vec![7], "r6, {:.0} m away, draws the near garden", dist("r6"));
        assert!(dist("r2") > NEAR_M && dist("r2") <= FAR_M, "r2 stands in the far range ({} m)", dist("r2"));
        assert_eq!(at("r2"), vec![8], "r2, {:.0} m away, draws the far garden", dist("r2"));
        assert!(at("r1").is_empty(), "the player's own plot is drawn as a neighbour's garden");
        assert!(dist("r20") > FAR_M, "r20 stands past the far range");
        assert!(at("r20").is_empty(), "r20, {:.0} m away, draws a garden", dist("r20"));
    }
}
