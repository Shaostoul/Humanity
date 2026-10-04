//! NEIGHBOURS (increment 2 of docs/design/ship-homes-and-logistics.md, "Meet in the Commons").
//!
//! WHY: since increment 1b every player's home stands on their own plot of the ship, and the game
//! draws only its own. Every other plot was empty floor-less void, so a person walking down First
//! Street saw nothing where their neighbour lives, and saw that neighbour walk out of a wall.
//!
//! WHAT: every plot but the one this game's home stands on (`ShipStructure::neighbour_plots`; all
//! of them while the home is put away, a guest's case) is drawn as its kind's DEFAULT design, the
//! shipped one (`HomeDesign::built_in`), never this player's own edited home: until homes are
//! shared, a neighbour's home looks like the default. With it, the plot's door corridor, and the
//! hole that corridor makes in its own shell and in the shared zone it runs to (the street's wall
//! where the neighbour's corridor arrives).
//!
//! RENDER ONLY, by design (the increment's own words): no machines (they are not part of a body),
//! no rooms (so no lights, no "you are in" volume, no sealed-atmosphere bound), and no collision
//! (src/ship/wall_collision.rs never sees a neighbour, and the hole in a shared zone is cut in its
//! mesh only, so its wall still stops anyone walking into a neighbour's corridor).
//!
//! It replaces the v0.638 clone tiling (`HomeStructure::tile_home_clones`, which stamped the
//! player's OWN home into every slot of a residential district and laid walkway connectors between
//! the slots; increment 1a switched it off because the clones overlapped the Commons).

use crate::ship::fibonacci::HomesteadMeshes;
use crate::ship::home_structure::ShellCut;
use crate::ship::ship_structure::{end_mouth_cut, push_corridor_tube, CorridorGeom, HomeDesign, Plot, ShipStructure};

/// One neighbour as this game draws it: its plot, the design drawn there (the default of the
/// plot's kind), and its door corridor (None when the corridor does not resolve: the home is then
/// drawn shut, as the ship file's validation would have refused it anyway).
pub struct Neighbour {
    pub plot: Plot,
    pub design: HomeDesign,
    pub tube: Option<CorridorGeom>,
}

/// Every neighbour of one ship, and the render-only holes their corridors make in its zones.
pub struct NeighbourView {
    pub neighbours: Vec<Neighbour>,
    /// (zone index, the hole) for each neighbour corridor's end on a shared zone.
    zone_cuts: Vec<(usize, ShellCut)>,
}

/// The neighbours of `ship`: each neighbour plot with the shipped design of its kind (one parse of
/// the built-in design per kind), its door corridor from that design's box, and the hole its
/// corridor makes in the shared zone it runs to. A plot whose kind has no built-in design is left
/// out (nothing to draw). Pure.
pub fn neighbour_view(ship: &ShipStructure) -> NeighbourView {
    let mut designs: Vec<(String, Option<HomeDesign>)> = Vec::new();
    let mut neighbours = Vec::new();
    let mut zone_cuts = Vec::new();
    for plot in ship.neighbour_plots() {
        let design = match designs.iter().find(|(k, _)| k == &plot.kind) {
            Some((_, d)) => d.clone(),
            None => {
                let d = HomeDesign::built_in(&plot.kind);
                designs.push((plot.kind.clone(), d.clone()));
                d
            }
        };
        let Some(design) = design else { continue };
        let tube = ship.plot_door_tube(plot, Some(&design)).ok();
        if let Some(g) = &tube {
            let z = &ship.zones[g.to_zone_idx];
            let cut = end_mouth_cut(z.origin_vec(), z.body.width, z.body.depth, z.body.height, g, g.end_to, g.end_from);
            zone_cuts.push((g.to_zone_idx, cut));
        }
        neighbours.push(Neighbour { plot: plot.clone(), design, tube });
    }
    NeighbourView { neighbours, zone_cuts }
}

impl NeighbourView {
    /// The render-only holes neighbour corridors make in zone `zi`'s shell.
    pub fn zone_cuts(&self, zi: usize) -> impl Iterator<Item = ShellCut> + '_ {
        self.zone_cuts.iter().filter(move |(z, _)| *z == zi).map(|(_, c)| *c)
    }

    /// The door a neighbour's home has, as a hole in its own shell where its corridor leaves.
    fn own_cut(n: &Neighbour) -> Option<ShellCut> {
        let g = n.tube.as_ref()?;
        let b = &n.design.body;
        let o = glam::Vec3::new(n.plot.origin.0, n.plot.origin.1, n.plot.origin.2);
        Some(end_mouth_cut(o, b.width, b.depth, b.height, g, g.end_from, g.end_to))
    }

    /// Draw every neighbour into `out`: its home's shell (`HomeStructure::bake_shell_groups`, its
    /// door cut through it) at its plot's origin, merged into opaque colour groups the way the
    /// district fillers are, and its corridor tube (`push_corridor_tube`, the one a ship corridor
    /// is drawn with). No room is registered for either.
    pub fn draw_into(&self, out: &mut HomesteadMeshes) {
        type Group = (Vec<crate::renderer::mesh::Vertex>, Vec<u32>, [f32; 3]);
        let mut groups: std::collections::HashMap<[i32; 3], Group> = std::collections::HashMap::new();
        for n in &self.neighbours {
            let cuts: Vec<ShellCut> = Self::own_cut(n).into_iter().collect();
            let (ox, oy, oz) = n.plot.origin;
            for (verts, idx, rgb) in n.design.body.bake_shell_groups(&cuts) {
                let key = [(rgb[0] * 64.0) as i32, (rgb[1] * 64.0) as i32, (rgb[2] * 64.0) as i32];
                let g = groups.entry(key).or_insert_with(|| (Vec::new(), Vec::new(), rgb));
                let base = g.0.len() as u32;
                g.0.extend(verts.into_iter().map(|mut v| {
                    v.position[0] += ox;
                    v.position[1] += oy;
                    v.position[2] += oz;
                    v
                }));
                g.1.extend(idx.into_iter().map(|i| i + base));
            }
            if let Some(g) = &n.tube {
                push_corridor_tube(out, g, n.design.body.shell_material);
            }
        }
        let mut list: Vec<_> = groups.into_iter().collect();
        list.sort_by_key(|(k, _)| *k);
        out.material_walls.extend(list.into_iter().map(|(_, (v, i, c))| (v, i, [c[0], c[1], c[2], 1.0])));
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
}
