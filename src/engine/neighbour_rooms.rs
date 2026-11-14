//! NEIGHBOURS' ROOMS LIT IN YOUR GAME (2026-11-14, the Row Street follow-up,
//! docs/design/home-room-program.md section 10).
//!
//! WHAT WAS WRONG. A neighbour's home is drawn without rooms (crates/humanity-ship/src/
//! neighbours.rs: render only), and indoors every surface takes its indirect light from the room
//! it faces (the room GI, renderer::room_probes): no room, no light but the 0.005 floor. Behind a
//! closed shell nobody saw it. Row Street's open green does (the operator: "I kinda prefer the open
//! green area"): from the green the neighbours' yards and house backs were near black beside your
//! own lit yard, by day and by night, so the green did not read as one green.
//!
//! WHAT THIS DOES. Every neighbour whose plot is within `LIT_RANGE_M` of the eye (and stays lit
//! until `LIT_SLACK_M` farther) hands the room GI its design's rooms, moved to its plot, and its
//! switched-on lamps (`Renderer::set_room_gi_neighbours`). They are AMBIENT-ONLY rooms: each takes
//! the room's ambient colour (`room_probes::room_ambient_with_unlit`: the sky through its glass
//! lid, its lamps' first arrival and their bounce, one colour a room, worked out on the GPU every
//! frame) and never holds probes, so the probe budget stays where the player is. Their lamps light
//! only their own rooms' ambient colour, never the fragment loop: no tile of the screen gains a
//! light, no party wall leaks one into your house, no shadow slot is spent.
//!
//! THE TECHNIQUE, and why it is a rung and not a stand-in. The ambient colour is the room GI's own
//! lowest level ("Ambient" in Settings), the integrating-sphere relation in progressive radiosity's
//! ambient-term form (Cohen, Chen, Wallace and Greenberg, SIGGRAPH 1988), and what ANY room shows
//! while its probes are not resident (probe streaming, docs/design/room-gi.md). Lit interiors seen
//! from outside in many games are faked behind windows (interior mapping, van Dongen 2008); here
//! the interiors are real geometry, so they need light, not a picture. The next rungs: a near
//! neighbour's lamps in the fragment loop once each is shadowed by the party wall (the shadow atlas
//! budget decides which), and its rooms holding probes (the probes then need the unlit lamps' first
//! arrival as the ambient colour has it).
//!
//! THE BUDGET (stated before it was built, docs/design/home-room-program.md section 10): at most
//! `MAX_NEIGHBOUR_ROOMS` rooms, whole plots nearest first. Each room costs one 64-thread workgroup
//! of `cs_ambient` a frame (about 1.5 us measured for the home's rooms, 2026-10-21: 0.073 ms for 48),
//! a 160-byte row of the room table and one texel of the ambient row; a far fragment's room pick
//! walks the nearer rooms first. So 256 rooms: under 0.4 ms of `cs_ambient` and 40 KB. The plot
//! choice is a distance test over the neighbours each frame (about 50 aboard).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use glam::Vec3;
use humanity_ship::ship_structure::{HomeDesign, ShipStructure};

use crate::engine::state::EngineState;
use crate::renderer::light::RoomLight;
use crate::renderer::room_probes::RoomBox;

/// A neighbour's plot is lit while its box is within this of the eye (metres): from the middle of
/// Row Street's green, every house of the terraces either side.
pub const LIT_RANGE_M: f32 = 72.0;
/// A lit plot stays lit until its box is this much farther, so walking at the boundary does not
/// lay the room table out again every step.
pub const LIT_SLACK_M: f32 = 8.0;
/// The most neighbours' rooms the room table takes, whole plots nearest first.
pub const MAX_NEIGHBOUR_ROOMS: usize = 256;

/// The neighbours lit this frame: their plots, their rooms and their lamps, in the home frame.
#[derive(Debug, Default, Clone)]
pub struct NeighbourLight {
    /// The plots lit, nearest the eye first.
    pub plots: Vec<String>,
    /// Their designs' rooms at their plots, as probe boxes, each plot's together.
    pub boxes: Vec<RoomBox>,
    /// How many of `boxes` each plot has, in the order of `plots` (the room table's plot rows,
    /// `room_probes::table_order`).
    pub runs: Vec<usize>,
    /// Their switched-on lamps at their plots (none of the open plot this player stands in, whose
    /// lamps the fragment loop already lights: `home_meshes::open_plot_lights`).
    pub lamps: Vec<RoomLight>,
}

/// How far `eye` is from a plot's box (0 inside it).
fn plot_distance(eye: Vec3, p: &humanity_ship::ship_structure::Plot) -> f32 {
    let o = Vec3::new(p.origin.0, p.origin.1, p.origin.2);
    let s = Vec3::new(p.size.0, p.size.1, p.size.2);
    (o - eye).max(eye - (o + s)).max(Vec3::ZERO).length()
}

/// Which neighbours are lit with the eye at `eye` (home frame), as indices into the neighbours'
/// view, nearest first: every one whose plot is within `LIT_RANGE_M` (`LIT_SLACK_M` farther for
/// one in `held`, last frame's), while their rooms fit `MAX_NEIGHBOUR_ROOMS`.
fn lit_neighbours(view: &humanity_ship::neighbours::NeighbourView, eye: Vec3, held: &[String]) -> Vec<usize> {
    let mut near: Vec<(f32, usize)> = view
        .neighbours
        .iter()
        .enumerate()
        .filter_map(|(i, n)| {
            let d = plot_distance(eye, &n.plot);
            let reach = LIT_RANGE_M + if held.iter().any(|h| *h == n.plot.id) { LIT_SLACK_M } else { 0.0 };
            (d <= reach).then_some((d, i))
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut rooms = 0usize;
    let mut out = Vec::new();
    for (_, i) in near {
        let n = design_rooms(view.neighbours[i].design).len();
        if rooms + n > MAX_NEIGHBOUR_ROOMS {
            continue;
        }
        rooms += n;
        out.push(i);
    }
    out
}

/// The neighbours lit with the eye at `eye` (home frame; `held`, last frame's lit plots, keeps a
/// lit one lit `LIT_SLACK_M` farther): their plots, rooms and lamps.
pub fn neighbour_light(ship: &ShipStructure, eye: Vec3, held: &[String]) -> NeighbourLight {
    let view = humanity_ship::neighbours::neighbour_view(ship);
    // The open plot this player stands in has its lamps in the fragment loop already.
    let lit_open = ship.open.lit.as_deref().filter(|p| ship.open.plots.contains(*p));
    let mut out = NeighbourLight::default();
    for i in lit_neighbours(&view, eye, held) {
        let n = &view.neighbours[i];
        let o = Vec3::new(n.plot.origin.0, n.plot.origin.1, n.plot.origin.2);
        let rooms = design_rooms(n.design);
        out.boxes.extend(rooms.iter().map(|b| b.shifted(o)));
        out.runs.push(rooms.len());
        if lit_open != Some(n.plot.id.as_str()) {
            out.lamps.extend(crate::engine::home_meshes::placed_room_lights(&n.design.body.lights, o));
        }
        out.plots.push(n.plot.id.clone());
    }
    out
}

/// A design's rooms as probe boxes in its own metres (`room_gi::room_box_in`), a room inside
/// another's box ahead of it (`room_probes::pick_order`, once a design: the room table keeps each
/// plot's rooms in this order, `room_probes::table_order`). Kept per design
/// for the process: a shipped design is parsed once and never changes while the game runs
/// (`HomeDesign::built_in_ref`), so they cannot go stale; found again on every call while the
/// "turn off all reuse" switch is off (CLAUDE.md's no-caches rule, `humanity_core::reuse`).
fn design_rooms(design: &'static HomeDesign) -> Rc<Vec<RoomBox>> {
    thread_local! {
        static ROOMS: RefCell<HashMap<usize, Rc<Vec<RoomBox>>>> = RefCell::new(HashMap::new());
    }
    let key = design as *const HomeDesign as usize;
    let reuse = humanity_core::reuse::enabled();
    if reuse {
        if let Some(r) = ROOMS.with(|m| m.borrow().get(&key).cloned()) {
            return r;
        }
    }
    let rooms: Vec<RoomBox> = design
        .body
        .detect_rooms()
        .iter()
        .map(|r| crate::engine::room_gi::room_box_in(&design.body, Vec3::ZERO, r))
        .collect();
    let centre = rooms.iter().fold(Vec3::ZERO, |a, r| a + (r.min + r.max) * 0.5) / rooms.len().max(1) as f32;
    let rooms: Vec<RoomBox> =
        crate::renderer::room_probes::pick_order(&rooms, centre).into_iter().map(|i| rooms[i].clone()).collect();
    let rooms = Rc::new(rooms);
    if reuse {
        ROOMS.with(|m| m.borrow_mut().insert(key, rooms.clone()));
    }
    rooms
}

/// Once a frame, before room GI's own update (`room_gi::before_scene`): the neighbours near the
/// eye to the renderer. Their rooms are laid out again only when the lit set changes (the
/// renderer compares), and every frame with the reuse switch off.
pub(crate) fn before_scene(state: &mut EngineState) {
    thread_local! {
        static HELD: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    }
    let eye = state.camera.effective_position() - state.station_off;
    let held = HELD.with(|h| h.borrow().clone());
    let lit = match state.gui_state.ship_structure.as_ref() {
        Some(ship) if !state.renderer.room_gi.neighbours_off => {
            let view = humanity_ship::neighbours::neighbour_view(ship);
            let ids: Vec<String> = lit_neighbours(&view, eye, &held).into_iter().map(|i| view.neighbours[i].plot.id.clone()).collect();
            if ids == held && humanity_core::reuse::enabled() {
                return;
            }
            neighbour_light(ship, eye, &held)
        }
        _ => NeighbourLight::default(),
    };
    if lit.plots != held {
        log::info!(
            "[RoomGI] neighbours lit: {} plots, {} rooms, {} lamps ({})",
            lit.plots.len(),
            lit.boxes.len(),
            lit.lamps.len(),
            lit.plots.join(" ")
        );
    }
    HELD.with(|h| *h.borrow_mut() = lit.plots.clone());
    state.renderer.set_room_gi_neighbours(lit.boxes, lit.runs, lit.lamps);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::room_probes::{assign_lights, room_ambient_with_unlit, DirectSource};
    use humanity_ship::ship_structure::{HomeDesign, Plot};

    /// The shipped ship with the two-bedroom row house on r1, as a dev start makes it.
    fn row_ship() -> ShipStructure {
        let data = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data");
        let ship = ShipStructure::load_ship_file(&data).expect("the shipped ship file");
        let design = HomeDesign::built_in("row_house_two_bedroom").expect("the shipped row house");
        ship.assemble(design, "r1").expect("it assembles at r1")
    }

    fn in_plot(b: &RoomBox, p: &Plot) -> bool {
        let c = (b.min + b.max) * 0.5;
        let (o, s) = (Vec3::new(p.origin.0, p.origin.1, p.origin.2), Vec3::new(p.size.0, p.size.1, p.size.2));
        c.cmpge(o).all() && c.cmple(o + s).all()
    }

    /// A NEIGHBOUR'S ROOM IS LIT WITHIN RANGE AND DARK BEYOND IT. From the green behind r3 (the
    /// player's home on r1), r3's rooms are in the room table, and those with a lamp read lit at
    /// night (no sky, no sun: their lamps alone, through the ambient colour); r20, 114 m away along
    /// the green, is past the range and has no room, so it keeps the old floor; the player's own
    /// plot is never a neighbour; the budget holds.
    ///
    /// Seen red 2026-11-14 with the selection a stub: "no room of r3 is lit from the green behind it".
    #[test]
    fn a_neighbour_s_room_is_lit_within_range_and_dark_beyond() {
        let ship = row_ship();
        let plot = |id: &str| ship.plots.iter().find(|p| p.id == id).cloned().unwrap_or_else(|| panic!("no plot {id}"));
        let eye = Vec3::new(117.0, 1.7, 316.0);
        let nl = neighbour_light(&ship, eye, &[]);
        let r3 = plot("r3");
        let r3_rooms: Vec<usize> = (0..nl.boxes.len()).filter(|&i| in_plot(&nl.boxes[i], &r3)).collect();
        assert!(!r3_rooms.is_empty(), "no room of r3 is lit from the green behind it");
        let owned = assign_lights(&nl.boxes, &nl.lamps);
        let dark = DirectSource::Scene { lights: vec![], sun: None };
        let mut lit = 0;
        for &i in &r3_rooms {
            if owned[i].is_empty() {
                continue;
            }
            let lamps: Vec<RoomLight> = owned[i].iter().map(|&k| nl.lamps[k]).collect();
            let amb = room_ambient_with_unlit(&nl.boxes[i], &dark, &lamps, |_| Vec3::ZERO);
            assert!(amb.max_element() > 0.01, "r3's room {:?}..{:?} has lamps and reads {amb} at night", nl.boxes[i].min, nl.boxes[i].max);
            lit += 1;
        }
        assert!(lit >= 4, "{lit} of r3's rooms are lit by their lamps at night");
        let r20 = plot("r20");
        let far = (Vec3::new(r20.origin.0, eye.y, r20.origin.2) - eye).length();
        assert!(far > LIT_RANGE_M + LIT_SLACK_M, "r20 stands past the range ({far} m)");
        assert!(nl.boxes.iter().all(|b| !in_plot(b, &r20)), "r20, {far:.0} m away, is lit beyond the range");
        assert!(!nl.plots.iter().any(|p| p == "r1"), "the player's own plot is lit as a neighbour's");
        assert!(nl.boxes.len() <= MAX_NEIGHBOUR_ROOMS, "{} rooms, over the budget", nl.boxes.len());
    }

    /// A FRAGMENT OUTSIDE EVERY ROOM TESTS EACH PLOT ONCE (the frame cost, 2026-11-14). The first
    /// boot of the neighbours' light measured +1.2 to +2.4 ms of gpu.scene at Row Street's vantages
    /// with `cs_ambient` unchanged: every fragment outside every room (the green, the street, the
    /// yards, the houses' outside walls) walked every neighbour's room in the table, 175 to 201 of
    /// them. Now each lit plot is one row of the table that the pick tests before its rooms. Over a
    /// grid of floor points across the whole lit street: the pick picks what the old whole-table
    /// walk picks (the same room, or one exactly as near), and a point outside every room tests
    /// about one box a plot.
    ///
    /// Seen red 2026-11-14 with `table_order` the old whole-table order and no plot rows: "a floor
    /// point outside every room tests 201.0 boxes on average (15 plots, 201 rooms)".
    #[test]
    fn a_fragment_outside_every_room_tests_each_plot_once() {
        use crate::renderer::room_probes::{pick_order, pick_room, pick_room_counted, table_order};
        let ship = row_ship();
        let eye = Vec3::new(117.0, 1.7, 316.0);
        let nl = neighbour_light(&ship, eye, &[]);
        assert_eq!(nl.runs.iter().sum::<usize>(), nl.boxes.len(), "the runs cover the rooms");
        let (order, plots) = table_order(&nl.boxes, 0, &nl.runs, eye);
        let mut rows = order.clone();
        rows.sort_unstable();
        assert_eq!(rows, (0..nl.boxes.len()).collect::<Vec<_>>(), "every room has exactly one row");
        let table: Vec<RoomBox> = order.iter().map(|&i| nl.boxes[i].clone()).collect();
        let old_order = pick_order(&nl.boxes, eye);
        let old_table: Vec<RoomBox> = old_order.iter().map(|&i| nl.boxes[i].clone()).collect();
        let (lo, hi) = nl.boxes.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), r| (a.min(r.min), b.max(r.max)));
        let q_of = |p: Vec3| p + Vec3::Y * crate::renderer::room_probes::ROOM_PICK_OFFSET_M;
        let (mut outside, mut outside_tests, mut points) = (0usize, 0usize, 0usize);
        // A step no wall spacing shares, so the grid lands on wall lines no more than by chance.
        let step = 1.37;
        let mut z = lo.z - 6.0;
        while z <= hi.z + 6.0 {
            let mut x = lo.x - 6.0;
            while x <= hi.x + 6.0 {
                let p = Vec3::new(x, lo.y + 0.02, z);
                let (new, tested) = pick_room_counted(&table, &plots, p, Vec3::Y);
                let new = new.map(|t| order[t]);
                let old = pick_room(&old_table, p, Vec3::Y).map(|t| old_order[t]);
                if new != old {
                    let d = |i: Option<usize>| i.map(|i| nl.boxes[i].outside_distance(q_of(p)).max(0.0));
                    let (dn, dold) = (d(new), d(old));
                    assert!(
                        matches!((dn, dold), (Some(a), Some(b)) if (a - b).abs() < 1e-5),
                        "at {p}: the plot walk picks {new:?} ({dn:?} m), the old walk {old:?} ({dold:?} m)"
                    );
                }
                if old.is_none() {
                    outside += 1;
                    outside_tests += tested;
                }
                points += 1;
                x += step;
            }
            z += step;
        }
        assert!(outside > 100 && outside < points, "{outside} of {points} points outside every room: the grid misses the street");
        let mean = outside_tests as f32 / outside as f32;
        assert!(
            mean <= nl.runs.len() as f32 + 6.0,
            "a floor point outside every room tests {mean:.1} boxes on average ({} plots, {} rooms)",
            nl.runs.len(),
            nl.boxes.len()
        );
    }
}
