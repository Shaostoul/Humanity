//! NEIGHBOURS (increment 2 of docs/design/ship-homes-and-logistics.md, "Meet in the Commons").
//!
//! WHY: since increment 1b every player's home stands on their own plot of the ship, and the game
//! draws only its own. Every other plot was empty floor-less void, so a person walking down First
//! Street saw nothing where their neighbour lives, and saw that neighbour walk out of a wall.
//!
//! WHAT: every plot but the one this game's home stands on (`ShipStructure::neighbour_plots`; all
//! of them while the home is put away, a guest's case; never a plot the home's own plot sits in or
//! holds) is drawn as its kind's DEFAULT design, the shipped one (`HomeDesign::built_in_ref`),
//! never this player's own edited home: until homes are shared, a neighbour's home looks like the
//! default. With it, the plot's door corridor, and the hole that corridor makes in its own shell
//! and in the shared zone it runs to (the street's wall where the neighbour's corridor arrives). A
//! plot the default design does not fit is not drawn (the rule `assemble` uses for the player's
//! own home, `design_fits_plot`), with one line in the log.
//!
//! RENDER ONLY, by design (the increment's own words): no machines (they are not part of a body),
//! no rooms (so no lights, no "you are in" volume, no sealed-atmosphere bound), and no collision
//! (src/ship/wall_collision.rs's walking segments never see a neighbour, and the hole in a shared
//! zone is cut in its mesh only, so its wall still stops anyone walking into a neighbour's
//! corridor). What the eye meets does agree with what is drawn (increment 2 review, finding 8):
//! each neighbour corridor mouth has a sliding door pair that opens for the other players (the
//! neighbour walking home) and never for this one (`neighbour_mouths`, door_panels.rs), and the
//! HUD's sight check (`wall_collision::ship_sight_segments`) sees the neighbours' walls, their
//! corridors and those doors, with the hole in the street's wall open to it.
//!
//! BUILT ONCE (increment 2 review, finding 6). Every home rebuild runs `ShipStructure::
//! generate_meshes`, and an editor drag rebuilds every frame; none of what a neighbour draws
//! depends on the player's own home. So the view is kept (`neighbour_view`, one per thread) and
//! made again only when what it is made from changes (`NeighbourKey`: which plot is the home's,
//! the plots, and the shared zones' boxes); each shipped design is parsed once per process
//! (`HomeDesign::built_in_ref`); and one shell is baked per design and door (the same door on the
//! same design bakes the same shell), then moved to each plot that uses it. Drawing each neighbour
//! as an object of its own, so it can be culled and drawn in less detail far away, is later.
//!
//! It replaces the v0.638 clone tiling (`HomeStructure::tile_home_clones`, which stamped the
//! player's OWN home into every slot of a residential district and laid walkway connectors between
//! the slots; increment 1a switched it off because the clones overlapped the Commons).

use crate::renderer::mesh::Vertex;
use crate::ship::fibonacci::HomesteadMeshes;
use crate::ship::home_structure::ShellCut;
use crate::ship::ship_structure::{
    design_fits_plot, end_mouth_cut, push_corridor_tube, CorridorGeom, CorridorMouth, HomeDesign, Plot, ShipStructure,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// One neighbour as this game draws it: its plot, the design drawn there (the default of the
/// plot's kind), and its door corridor (None when the corridor does not resolve: the home is then
/// drawn shut, as the ship file's validation would have refused it anyway).
pub struct Neighbour {
    pub plot: Plot,
    pub design: &'static HomeDesign,
    pub tube: Option<CorridorGeom>,
}

/// Every neighbour of one ship, the render-only holes their corridors make in its zones, and
/// everything they draw, baked once (`neighbour_view`).
pub struct NeighbourView {
    pub neighbours: Vec<Neighbour>,
    /// (zone index, the hole) for each neighbour corridor's end on a shared zone.
    zone_cuts: Vec<(usize, ShellCut)>,
    /// The neighbours' shells and corridor tubes, ready to append (`draw_into`).
    meshes: HomesteadMeshes,
    /// How many shells were baked for it: one per (design, door), however many plots use it.
    pub(crate) shells_baked: usize,
}

/// What a ship's neighbours are made from, and so when the view must be made again: which plot
/// the home stands on (None while it is put away, or for a ship file on its own), every plot, and
/// each zone's id and box (a corridor runs to a zone's face and cuts its wall). The player's own
/// home's walls, lights and rooms are not in it: an edit of the home never re-bakes a neighbour.
#[derive(Clone, PartialEq)]
struct NeighbourKey {
    own: Option<String>,
    plots: Vec<Plot>,
    zones: Vec<(String, (f32, f32, f32), f32, f32, f32)>,
}

impl NeighbourKey {
    fn of(ship: &ShipStructure) -> NeighbourKey {
        NeighbourKey {
            own: ship.home_plot().map(|p| p.id.clone()),
            plots: ship.plots.clone(),
            zones: ship
                .zones
                .iter()
                .map(|z| (z.id.clone(), z.origin, z.body.width, z.body.depth, z.body.height))
                .collect(),
        }
    }
}

thread_local! {
    /// The last view made on this thread, and what it was made from. One entry: a game runs one
    /// ship, and every reader in one rebuild (the meshes, the hull, the sight lines, the doors)
    /// asks for the same one.
    static VIEW: RefCell<Option<(NeighbourKey, Rc<NeighbourView>)>> = const { RefCell::new(None) };
    /// How many views this thread has made (the test that an edit of the home makes none).
    static MADE: Cell<usize> = const { Cell::new(0) };
}

/// How many neighbour views this thread has made since it started (tests only read it).
#[cfg(test)]
pub(crate) fn views_made() -> usize {
    MADE.with(|m| m.get())
}

/// The neighbours of `ship` (see the module comment): kept from the last call on this thread
/// while what they are made from is unchanged (`NeighbourKey`), else made afresh
/// (`make_view`). Cheap to call from every reader of one rebuild.
pub fn neighbour_view(ship: &ShipStructure) -> Rc<NeighbourView> {
    let key = NeighbourKey::of(ship);
    VIEW.with(|slot| {
        if let Some((k, v)) = slot.borrow().as_ref() {
            if *k == key {
                return v.clone();
            }
        }
        let view = Rc::new(make_view(ship));
        MADE.with(|m| m.set(m.get() + 1));
        *slot.borrow_mut() = Some((key, view.clone()));
        view
    })
}

/// Make the view: each neighbour plot with the shipped design of its kind, its door corridor from
/// that design's box, the hole its corridor makes in the shared zone it runs to, and the meshes,
/// one shell baked per (design, door) and moved to each plot that uses it. A plot whose kind has
/// no built-in design, or whose design does not fit it, is left out. Pure but for the log line.
fn make_view(ship: &ShipStructure) -> NeighbourView {
    let mut neighbours = Vec::new();
    let mut zone_cuts = Vec::new();
    for plot in ship.neighbour_plots() {
        let Some(design) = HomeDesign::built_in_ref(&plot.kind) else { continue };
        if let Err(e) = design_fits_plot(design, plot) {
            log::warn!("Neighbours: plot {} is not drawn: {e}", plot.id);
            continue;
        }
        let tube = ship.plot_door_tube(plot, Some(design)).ok();
        if let Some(g) = &tube {
            let z = &ship.zones[g.to_zone_idx];
            let cut = end_mouth_cut(z.origin_vec(), z.body.width, z.body.depth, z.body.height, g, g.end_to, g.end_from);
            zone_cuts.push((g.to_zone_idx, cut));
        }
        neighbours.push(Neighbour { plot: plot.clone(), design, tube });
    }
    let (meshes, shells_baked) = bake(&neighbours);
    NeighbourView { neighbours, zone_cuts, meshes, shells_baked }
}

/// The door a neighbour's home has, as a hole in its own shell where its corridor leaves (in the
/// shell's own terms, an edge and a distance along it, so the same door on the same design is the
/// same cut on every plot).
fn own_cut(n: &Neighbour) -> Option<ShellCut> {
    let g = n.tube.as_ref()?;
    let b = &n.design.body;
    let o = glam::Vec3::new(n.plot.origin.0, n.plot.origin.1, n.plot.origin.2);
    Some(end_mouth_cut(o, b.width, b.depth, b.height, g, g.end_from, g.end_to))
}

/// One shell as `HomeStructure::bake_shell_groups` bakes it: (vertices, indices, rgb) groups.
type Shell = Vec<(Vec<Vertex>, Vec<u32>, [f32; 3])>;

/// Every neighbour's shell (`HomeStructure::bake_shell_groups`, its door cut through it) at its
/// plot's origin, merged into opaque colour groups the way the district fillers are, and its
/// corridor tube (`push_corridor_tube`, the one a ship corridor is drawn with). One bake per
/// (design, door): plots sharing both share the bake, moved. No room is registered for either.
/// Returns the meshes and how many shells it baked.
fn bake(neighbours: &[Neighbour]) -> (HomesteadMeshes, usize) {
    let mut out = empty_meshes();
    let mut shells: Vec<(&'static HomeDesign, Option<ShellCut>, Shell)> = Vec::new();
    let mut groups: std::collections::HashMap<[i32; 3], (Vec<Vertex>, Vec<u32>, [f32; 3])> = std::collections::HashMap::new();
    for n in neighbours {
        let cut = own_cut(n);
        let at = match shells.iter().position(|(d, c, _)| std::ptr::eq(*d, n.design) && *c == cut) {
            Some(at) => at,
            None => {
                let cuts: Vec<ShellCut> = cut.into_iter().collect();
                shells.push((n.design, cut, n.design.body.bake_shell_groups(&cuts)));
                shells.len() - 1
            }
        };
        let (ox, oy, oz) = n.plot.origin;
        for (verts, idx, rgb) in &shells[at].2 {
            let key = [(rgb[0] * 64.0) as i32, (rgb[1] * 64.0) as i32, (rgb[2] * 64.0) as i32];
            let g = groups.entry(key).or_insert_with(|| (Vec::new(), Vec::new(), *rgb));
            let base = g.0.len() as u32;
            g.0.extend(verts.iter().map(|v| {
                let mut v = *v;
                v.position[0] += ox;
                v.position[1] += oy;
                v.position[2] += oz;
                v
            }));
            g.1.extend(idx.iter().map(|i| i + base));
        }
        if let Some(g) = &n.tube {
            push_corridor_tube(&mut out, g, n.design.body.shell_material);
        }
    }
    let mut list: Vec<_> = groups.into_iter().collect();
    list.sort_by_key(|(k, _)| *k);
    out.material_walls.extend(list.into_iter().map(|(_, (v, i, c))| (v, i, [c[0], c[1], c[2], 1.0])));
    (out, shells.len())
}

/// A `HomesteadMeshes` with nothing in it.
fn empty_meshes() -> HomesteadMeshes {
    HomesteadMeshes {
        floors: Vec::new(),
        walls: (Vec::new(), Vec::new()),
        material_walls: Vec::new(),
        trim: (Vec::new(), Vec::new()),
        windows: (Vec::new(), Vec::new()),
        mirrors: (Vec::new(), Vec::new()),
        ceilings: (Vec::new(), Vec::new()),
        ceilings_opaque: (Vec::new(), Vec::new()),
        room_info: Vec::new(),
    }
}

/// Append index-offset copies of `from` onto `into`.
fn append(into: &mut (Vec<Vertex>, Vec<u32>), from: &(Vec<Vertex>, Vec<u32>)) {
    let base = into.0.len() as u32;
    into.0.extend_from_slice(&from.0);
    into.1.extend(from.1.iter().map(|i| i + base));
}

impl NeighbourView {
    /// The render-only holes neighbour corridors make in zone `zi`'s shell.
    pub fn zone_cuts(&self, zi: usize) -> impl Iterator<Item = ShellCut> + '_ {
        self.zone_cuts.iter().filter(move |(z, _)| *z == zi).map(|(_, c)| *c)
    }

    /// Append everything the neighbours draw to `out`: copies of the meshes baked when the view
    /// was made (`bake`, which fills the floors, the opaque walls and the two ceiling buffers).
    pub fn draw_into(&self, out: &mut HomesteadMeshes) {
        let m = &self.meshes;
        out.floors.extend(m.floors.iter().cloned());
        out.material_walls.extend(m.material_walls.iter().cloned());
        append(&mut out.ceilings, &m.ceilings);
        append(&mut out.ceilings_opaque, &m.ceilings_opaque);
    }

    /// Every neighbour corridor's door mouths, both ends (`ShipStructure::tube_mouths`): where the
    /// sliding door pairs that open only for the other players stand (door_panels.rs).
    pub fn neighbour_mouths(&self, ship: &ShipStructure) -> Vec<CorridorMouth> {
        self.neighbours.iter().filter_map(|n| n.tube.as_ref()).flat_map(|g| ship.tube_mouths(g)).collect()
    }

    /// The walls the neighbours make: each one's walls with its door open, at its plot (its windows
    /// open too when `sight`), and its corridor's two side walls. For the HUD's sight check
    /// (`wall_collision::ship_sight_segments`) and the rig's walls (`everyones_walls`); never a
    /// walking collider of this game.
    pub fn segments(&self, sight: bool) -> Vec<crate::ship::wall_collision::WallSegment> {
        use crate::ship::wall_collision::{corridor_side_walls, sight_segments_with_shell_cuts, wall_segments_with_shell_cuts, WallSegment};
        let mut out = Vec::new();
        for n in &self.neighbours {
            let cuts: Vec<ShellCut> = own_cut(n).into_iter().collect();
            let (ox, oz) = (n.plot.origin.0, n.plot.origin.2);
            let own = if sight { sight_segments_with_shell_cuts(&n.design.body, &cuts) } else { wall_segments_with_shell_cuts(&n.design.body, &cuts) };
            out.extend(own.into_iter().map(|s| WallSegment {
                a: (s.a.0 + ox, s.a.1 + oz),
                b: (s.b.0 + ox, s.b.1 + oz),
                half_thickness: s.half_thickness,
            }));
            if let Some(g) = &n.tube {
                out.extend(corridor_side_walls(g));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn data_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    /// The game's ship with its home on `plot`.
    fn on(plot: &str) -> ShipStructure {
        ShipStructure::load_and_assemble(&data_dir(), Some(plot)).expect("the shipped ship assembles")
    }

    /// Every vertex position of every opaque material-wall group, as a flat list.
    fn wall_points(m: &HomesteadMeshes) -> Vec<Vec3> {
        m.material_walls.iter().flat_map(|(v, _, _)| v.iter().map(|x| Vec3::from(x.position))).collect()
    }

    fn inside_xz(p: Vec3, lo: Vec3, hi: Vec3) -> bool {
        p.x >= lo.x - 0.2 && p.x <= hi.x + 0.2 && p.z >= lo.z - 0.2 && p.z <= hi.z + 0.2
    }

    /// THE NEIGHBOUR: with the home on p1, p2 is drawn as the default homestead (geometry fills
    /// p2's box) with its corridor to First Street, and nothing of it is a room, a light's room or
    /// a wall anyone walks into. On p2 the same holds for p1. Seen red 2026-10-04 with
    /// `draw_into` doing nothing (the 1b game, which drew no neighbour): "p2 is drawn: 0 vertices
    /// of the ship's geometry stand on it".
    #[test]
    fn the_other_plot_is_drawn_as_the_default_home_render_only() {
        use crate::ship::wall_collision::ship_wall_segments;
        for (own, other) in [("p1", "p2"), ("p2", "p1")] {
            let ship = on(own);
            let view = neighbour_view(&ship);
            assert_eq!(view.neighbours.len(), 1, "on {own}: one neighbour");
            assert_eq!(view.neighbours[0].plot.id, other);
            let (lo, hi) = ship.plots.iter().find(|p| p.id == other).unwrap().aabb();
            let m = ship.generate_meshes();
            let on_it = wall_points(&m).into_iter().filter(|p| inside_xz(*p, lo, hi)).count();
            assert!(on_it > 1000, "{other} is drawn: {on_it} vertices of the ship's geometry stand on it");
            // No room of the ship lies on the neighbour's plot: no light, no sealed volume, no HUD room.
            let rooms_on: Vec<&str> = m.room_info.iter().filter(|r| inside_xz(r.center, lo, hi)).map(|r| r.id.as_str()).collect();
            assert!(rooms_on.is_empty(), "on {own}: rooms on the neighbour's plot {other}: {rooms_on:?}");
            // Nothing on it collides.
            let walls_on = ship_wall_segments(&ship)
                .into_iter()
                .filter(|s| inside_xz(Vec3::new(s.a.0, 0.0, s.a.1), lo + Vec3::new(1.0, 0.0, 1.0), hi - Vec3::new(1.0, 0.0, 1.0)))
                .count();
            assert_eq!(walls_on, 0, "on {own}: {walls_on} collision segments stand inside the neighbour's plot {other}");
            // The neighbour's corridor: drawn, and the tube runs from its home's door to its zone.
            let g = view.neighbours[0].tube.as_ref().expect("the neighbour's corridor resolves");
            let p = &view.neighbours[0].plot;
            let door = Vec3::new(p.origin.0 + 55.0, 0.0, p.origin.2 + 40.0);
            assert!((g.end_from - door).length() < 1e-3, "{other}'s corridor leaves its home at its door {door:?}, got {:?}", g.end_from);
        }
    }

    /// The hole a neighbour's corridor makes in the shared zone it runs to is in the MESH only: on
    /// p1, p2's corridor arrives at First Street's west wall at z 139, so the street's mesh has an
    /// opening there and its collision does not (the wall still stops anyone walking into a
    /// neighbour's corridor). Seen red 2026-10-04 with the neighbour cuts left out of
    /// `generate_meshes`: "First Street has no hole where p2's corridor arrives: 0 cuts".
    #[test]
    fn a_neighbours_corridor_opens_the_street_wall_for_the_eye_only() {
        let ship = on("p1");
        let view = neighbour_view(&ship);
        let street = ship.zone_index("street-1").expect("street-1");
        let cuts: Vec<ShellCut> = view.zone_cuts(street).collect();
        assert_eq!(cuts.len(), 1, "First Street has no hole where p2's corridor arrives: {} cuts", cuts.len());
        // The west wall (edge 3, winding -z along x = 0) at z 139 - 85 local, the door 2 m wide.
        let c = cuts[0];
        assert_eq!(c.edge, 3);
        assert!((c.at - (110.0 - (54.0 + 1.0))).abs() < 1e-3, "edge-local at {}", c.at);
        // Collision keeps the street's west wall whole there: no zone cut of its own at z 139.
        assert!(
            ship.shell_cuts_for_zone(street).iter().all(|k| k.edge != 3),
            "the street's own (collision) cuts include the neighbour's door: {:?}",
            ship.shell_cuts_for_zone(street)
        );
    }

    /// A neighbour is drawn as the SHIPPED design, never this player's own edited home: a home
    /// file on disk with an extra wall changes the player's own home and no neighbour. (The
    /// built-in copy is what `HomeDesign::built_in` reads.)
    #[test]
    fn a_neighbour_is_the_shipped_design_not_my_own_edits() {
        let built_in = HomeDesign::built_in("homestead").expect("the homestead design is built in");
        let on_disk = HomeDesign::load(&data_dir(), "homestead").expect("the homestead design loads");
        assert_eq!(built_in.body.walls.len(), on_disk.body.walls.len(), "the shipped file is the built-in copy");
        let view = neighbour_view(&on("p1"));
        assert_eq!(view.neighbours[0].design.body.walls.len(), built_in.body.walls.len());
        assert!(HomeDesign::built_in("no_such_kind").is_none());
    }

    /// While the home is put away (a guest), EVERY plot is a neighbour's, and nothing of the home
    /// is drawn: no geometry and no room at the place it is kept, and every plot drawn. Seen red
    /// 2026-10-04 with `generate_meshes` drawing the home put away like any zone: "the home put away
    /// is drawn: 23 rooms at its place".
    #[test]
    fn a_home_put_away_is_drawn_nowhere_and_every_plot_is_a_neighbours() {
        let away = on("p1").put_home_away().expect("the home can be put away");
        assert!(away.home_is_away() && away.home_plot().is_none());
        let view = neighbour_view(&away);
        let ids: Vec<&str> = view.neighbours.iter().map(|n| n.plot.id.as_str()).collect();
        assert_eq!(ids, ["p1", "p2"], "every plot is a neighbour's while the home is away");
        let m = away.generate_meshes();
        let o = Vec3::from(crate::ship::ship_structure::HOME_AWAY_ORIGIN);
        let near = |p: Vec3| p.distance(o) < 200.0;
        let rooms = m.room_info.iter().filter(|r| near(r.center)).count();
        assert_eq!(rooms, 0, "the home put away is drawn: {rooms} rooms at its place");
        assert_eq!(wall_points(&m).into_iter().filter(|p| near(*p)).count(), 0, "geometry at the home's place");
        assert!(m.floors.iter().all(|(v, ..)| v.iter().all(|x| !near(Vec3::from(x.position)))), "a floor at the home's place");
    }

    /// FINDING 6 of the increment 2 review: an editor drag rebuilds the home every frame, and
    /// every rebuild re-baked a whole homestead shell per neighbour (twice: the meshes and the
    /// hull each made the view), though nothing a neighbour draws depends on the player's own
    /// home. Now one view per thread is kept and made again only when what it is made from
    /// changes. Here: a rebuild (meshes, hull, sight lines, doors) after an edit of the home makes
    /// no view; a move of the neighbour's plot makes one. Seen red 2026-10-04 with
    /// `neighbour_view` making a view on every call (the c98c5465b behaviour, with the doors and
    /// sight lines below also broken): "a rebuild after an edit of the home made 3 neighbour
    /// views".
    #[test]
    fn an_own_home_edit_does_not_rebake_the_neighbours() {
        let rebuild = |ship: &ShipStructure| {
            let _ = ship.generate_meshes();
            let _ = crate::ship::hull::hull_geom(ship, &crate::ship::hull::HullProfile::parse("()").unwrap());
            let _ = crate::ship::wall_collision::ship_sight_segments(ship);
            let _ = crate::ship::door_panels::ship_panel_placements(ship);
        };
        let mut ship = on("p1");
        rebuild(&ship);
        let before = views_made();
        let hz = ship.home_zone_index();
        ship.zones[hz].body.walls.push(ron::from_str("(a: (1.0, 1.0), b: (3.0, 1.0))").unwrap());
        rebuild(&ship);
        let made = views_made() - before;
        assert_eq!(made, 0, "a rebuild after an edit of the home made {made} neighbour views");
        let i = ship.plots.iter().position(|p| p.id == "p2").unwrap();
        ship.plots[i].origin.0 -= 1.0;
        rebuild(&ship);
        assert_eq!(views_made() - before, 1, "a move of the neighbour's plot is drawn: one view made");
    }

    /// Each shipped design is parsed once per process (finding 6): `built_in_ref` hands back the
    /// same parse every time. Seen red 2026-10-04 with `built_in_ref` parsing on every call
    /// (leaked, as the c98c5465b `built_in` parsed on every call): "two asks gave two parses".
    #[test]
    fn built_in_designs_parse_once() {
        let a = HomeDesign::built_in_ref("homestead").expect("the homestead is built in");
        let b = HomeDesign::built_in_ref("homestead").unwrap();
        assert!(std::ptr::eq(a, b), "two asks gave two parses");
        assert!(HomeDesign::built_in_ref("no_such_kind").is_none());
    }

    /// One shell is baked per design and door, however many plots use them (finding 6): with the
    /// home put away both plots are neighbours, the same design with the same door, so one bake
    /// is moved to both. Seen red 2026-10-04 with one bake per neighbour (the c98c5465b
    /// `draw_into`): "2 shells baked for 2 neighbours of one design and door".
    #[test]
    fn plots_of_one_design_and_door_share_one_bake() {
        let away = on("p1").put_home_away().unwrap();
        let view = neighbour_view(&away);
        assert_eq!(view.neighbours.len(), 2);
        assert_eq!(view.shells_baked, 1, "{} shells baked for 2 neighbours of one design and door", view.shells_baked);
        // And both are drawn: each plot carries its share of the geometry.
        let m = away.generate_meshes();
        for p in &away.plots {
            let (lo, hi) = p.aabb();
            let n = wall_points(&m).into_iter().filter(|q| inside_xz(*q, lo, hi)).count();
            assert!(n > 1000, "{} is drawn: {n} vertices", p.id);
        }
    }

    /// Times the rebuild an editor drag makes every frame, with the neighbours kept and made
    /// afresh each time (the c98c5465b cost, which also made the view a second time for the
    /// hull). Not a check: run with `--ignored --nocapture` to read the numbers.
    #[test]
    #[ignore]
    fn neighbour_rebuild_timing() {
        let ship = on("p1");
        let profile = crate::ship::hull::HullProfile::parse("()").unwrap();
        let time = |f: &dyn Fn()| {
            let t = std::time::Instant::now();
            for _ in 0..5 {
                f();
            }
            t.elapsed().as_secs_f64() * 1000.0 / 5.0
        };
        let kept = time(&|| {
            let _ = ship.generate_meshes();
            let _ = crate::ship::hull::hull_geom(&ship, &profile);
        });
        let one_view = time(&|| {
            let _ = make_view(&ship);
        });
        println!("rebuild with the neighbours kept: {kept:.1} ms; one neighbour view made: {one_view:.1} ms (the old rebuild made two: about {:.1} ms)", kept + 2.0 * one_view);
    }

    /// FINDING 7 of the increment 2 review, the other half: a plot the shipped design does not
    /// fit (the Dev Plots panel made it 1 m narrower) is not drawn, the rule `assemble` uses for
    /// the player's own home; it used to spill into the next plot or zone. Seen red 2026-10-04 on
    /// c98c5465b (no size check): "a neighbour that does not fit p2 is drawn: 1 neighbour(s)".
    #[test]
    fn a_neighbour_that_does_not_fit_its_plot_is_not_drawn() {
        let mut ship = on("p1");
        let i = ship.plots.iter().position(|p| p.id == "p2").unwrap();
        ship.plots[i].size.0 -= 1.0;
        let view = neighbour_view(&ship);
        assert_eq!(view.neighbours.len(), 0, "a neighbour that does not fit p2 is drawn: {} neighbour(s)", view.neighbours.len());
        let (lo, hi) = ship.plots[i].aabb();
        let m = ship.generate_meshes();
        let spill = wall_points(&m).into_iter().filter(|p| inside_xz(*p, lo - Vec3::new(1.0, 0.0, 0.0), hi + Vec3::new(1.0, 0.0, 0.0))).count();
        assert_eq!(spill, 0, "{spill} vertices drawn at p2");
    }

    /// FINDING 8 of the increment 2 review: on First Street the player saw an open hole into
    /// p2's corridor (their own corridor mouths have sliding doors), walking at it met an
    /// invisible wall, and a neighbour standing in their corridor had their nameplate hidden by
    /// that unseen wall. Now each neighbour corridor mouth (both ends) has a sliding door pair
    /// that opens for the other players and never for this one; the street's collision is still
    /// whole there; and the sight check has the hole open (a closed door blocks it, as the HUD's
    /// live door list adds) and sees the neighbour's walls. Seen red 2026-10-04 with no
    /// neighbour doors and no neighbour sight lines (the c98c5465b behaviour): "no door at
    /// p2's corridor mouth on First Street (x 65, z 139): []".
    #[test]
    fn a_neighbours_corridor_mouth_has_a_door_that_opens_for_others_only() {
        use crate::ship::door_panels::{door_actor_distance, ship_panel_placements};
        use crate::ship::wall_collision::{sight_blocked, ship_sight_segments, ship_wall_segments};
        let ship = on("p1");
        let doors = ship_panel_placements(&ship);
        let at = |x: f32| doors.iter().filter(|p| (p.center.x - x).abs() < 0.01 && (p.center.z - 139.0).abs() < 1.0).collect::<Vec<_>>();
        let street = at(65.0);
        assert_eq!(street.len(), 2, "no door at p2's corridor mouth on First Street (x 65, z 139): {street:?}");
        assert!(street.iter().all(|p| p.others_only && !p.is_window), "a neighbour's door opens for the others only");
        assert_eq!(at(55.0).len(), 2, "the door at p2's own end of its corridor");
        // This player's own corridor doors are ordinary: none of them is others-only.
        let own: Vec<_> = doors
            .iter()
            .filter(|p| (p.center.z - 40.0).abs() < 1.0 && ((p.center.x - 55.0).abs() < 0.01 || (p.center.x - 65.0).abs() < 0.01))
            .collect();
        assert!(own.len() == 4 && own.iter().all(|p| !p.others_only), "p1's own corridor doors: {own:?}");
        // Who opens it: the neighbour walking home, never this player (nor an animal).
        let d = street[0];
        let beside = d.center + Vec3::new(1.0, 1.7, 0.0);
        assert_eq!(door_actor_distance(d, Some(beside), &[], &[beside]), f32::MAX, "this player opens a neighbour's door");
        assert!(door_actor_distance(d, None, &[beside], &[]) < 1.5, "the neighbour walking home does not open it");
        let ordinary = doors.iter().find(|p| !p.others_only && !p.is_window).unwrap();
        assert!(door_actor_distance(ordinary, Some(ordinary.center), &[], &[]) < 0.01, "an ordinary door opens for this player");
        // Walking: the street's west wall is whole across the mouth. Sight: open there.
        let pack = |s: Vec<crate::ship::wall_collision::WallSegment>| s.into_iter().map(|w| [w.a.0, w.a.1, w.b.0, w.b.1]).collect::<Vec<_>>();
        let (walk, sight) = (pack(ship_wall_segments(&ship)), pack(ship_sight_segments(&ship)));
        let (street_eye, in_corridor) = ((70.0, 139.0), (60.0, 139.0));
        assert!(sight_blocked(street_eye, in_corridor, &walk), "the street's wall has a walking gap at p2's corridor");
        assert!(!sight_blocked(street_eye, in_corridor, &sight), "the hole into p2's corridor hides a nameplate in it");
        // The neighbour's own walls block sight as they are drawn: from its corridor, through its
        // east wall (not its door), into the home.
        assert!(sight_blocked(in_corridor, (40.0, 110.0), &sight), "a nameplate inside p2's home shows through its walls");
    }
}
