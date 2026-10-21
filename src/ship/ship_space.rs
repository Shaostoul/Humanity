//! Where a point is in the ship (increment 4 of docs/design/ship-homes-and-logistics.md,
//! "getting around at ship scale"): aboard or not, and whose air it breathes.
//!
//! ABOARD. Until this increment "aboard" was a 400 m sphere around the ship frame's origin (the
//! corner of plot p1, lib.rs, the station block): the first test ship fits inside it, but the
//! first drum (250 m radius by 2 km, docs/design/habitat-generation.md) runs out of it a fifth of
//! the way along, and a player walking there would have been let go of the ship's frame, left
//! behind in orbit. Now it is the ship's own box: every zone, plot, corridor and district, grown
//! by `ABOARD_MARGIN_M`.
//!
//! AIR. Until this increment the game had ONE sealed air space, 14,000 m3 whatever the home, and
//! a player was in it anywhere inside the box around every room of the ship, the Commons and
//! First Street included (home_spawn.rs, survival_env.rs `whereabouts`); a guest, whose home is
//! put away a kilometre off the ship, stretched that box over a kilometre of space. Now each home
//! has its own air, sized from its own design (`home_air_volume_m3`), a player is in it only
//! inside their own home, and the ship's shared spaces (its zones, corridors and the other plots)
//! breathe the ship's air. The game simulates only its own home's air; the ship's is the ship's
//! life support, kept at the standard the home starts at, until it gets a model of its own.

use crate::ship::home_structure::HomeStructure;
use crate::ship::ship_structure::{CorridorAxis, CorridorGeom, HomeDesign, ShipStructure, HOME_ZONE_ID};
use glam::Vec3;

/// How far outside the ship's box a person still counts as aboard, metres: the hull stands a few
/// metres off every zone (data/blueprints/hull_profile.ron `margin`), and someone out on the
/// plating, or flying a short way off it in Dev fly mode, still rides with the ship. 50 m is a
/// game rule, not a sourced figure.
pub const ABOARD_MARGIN_M: f32 = 50.0;

/// A box, (min, max) in ship metres.
pub type Aabb = (Vec3, Vec3);

/// How far over a walkable surface a person's eye can be, metres: the standing eye height
/// (surface_walk::EYE_HEIGHT_M, 1.7) and the rise of a jump at the homestead's 1 g
/// (renderer::camera::JUMP_SPEED_MPS, 5 m/s: 1.27 m), rounded up. A space's air reaches this far
/// over the highest thing in it (`air_top`); engine/survival_env.rs holds it to the camera's own
/// numbers and the gravity in data/game.csv.
pub const HEADROOM_M: f32 = 3.0;

/// A corridor's tube as a box: floor to lid, wall to wall, mouth to mouth.
pub fn tube_box(g: &CorridorGeom) -> Aabb {
    let hw = g.width * 0.5;
    match g.axis {
        CorridorAxis::X => (Vec3::new(g.start, g.floor_y, g.lat - hw), Vec3::new(g.end, g.floor_y + g.height, g.lat + hw)),
        CorridorAxis::Z => (Vec3::new(g.lat - hw, g.floor_y, g.start), Vec3::new(g.lat + hw, g.floor_y + g.height, g.end)),
    }
}

/// True when `p` is inside the box (its faces count as inside).
pub fn in_box(b: &Aabb, p: Vec3) -> bool {
    p.cmpge(b.0).all() && p.cmple(b.1).all()
}

/// How far from the ship frame's origin a person counted as aboard before increment 4, metres:
/// still the rule while no ship has assembled (the legacy layout, a world still loading).
pub const LEGACY_ABOARD_RADIUS_M: f64 = 400.0;

/// True when a point `rel` metres from the ship frame's origin (in the frame's own axes, f64 as
/// the station block in lib.rs measures it) is aboard: inside the ship's box once the ship has
/// assembled (`aboard_bounds`), else within the old 400 m. A point thousands of kilometres off
/// is far outside any box, so narrowing it to f32 for the test loses nothing that matters.
pub fn aboard_at(bounds: Option<Aabb>, rel: glam::DVec3) -> bool {
    match bounds {
        Some(b) => in_box(&b, rel.as_vec3()),
        None => rel.length() < LEGACY_ABOARD_RADIUS_M,
    }
}

/// A home's own air, m3: what its rooms hold, storey by storey (ship homes increment 7,
/// docs/design/ship-homes-increment-7-plan.md section 3: "the home's air volume is the sum of its
/// rooms per storey").
///
/// - The ground storey is all air, its whole footprint from the floor to the floor above, less
///   the slab over each place (a slab's 0.25 m is concrete, not air).
/// - A storey above holds air where a room of it is, a slab under the place and a zone of that
///   storey over it (the rooms `detect_rooms` floods there), and where the air below rises into it
///   through no floor at all (the greenhouse's upper half, a stairwell, an elevator shaft). The
///   space over a lid nobody uses is no one's air (docs/design/homestead-upper-floor.md section 9
///   item 3(b): about 11,000 m3 of it over the homestead's lids).
///
/// With no slab it is the body's box, floor to roof, exactly as before storeys: the homestead's
/// 55 by 89 by 3 m, 14,685 m3, where one 14,000 m3 space used to stand for every home. Measured on
/// the same 0.5 m grid the rooms are flooded on.
pub fn home_air_volume_m3(body: &HomeStructure) -> f32 {
    if body.floors.is_empty() {
        return (body.width * body.depth * body.height).max(0.0);
    }
    const CELL: f32 = 0.5;
    let (w, d) = (body.width.max(0.0), body.depth.max(0.0));
    let (nx, nz) = ((w / CELL).ceil() as usize, (d / CELL).ceil() as usize);
    let storeys = body.storeys();
    // The thickest slab that is storey k's floor over (x, z), None where none is.
    let slab_at = |k: usize, x: f32, z: f32| -> Option<f32> {
        body.floors.iter().filter(|s| s.is_floor_of(&storeys[k]) && s.contains(x, z)).map(|s| s.thickness.max(0.0)).reduce(f32::max)
    };
    let zones: Vec<(usize, &crate::ship::home_structure::Zone)> =
        body.zones.iter().map(|z| (crate::ship::home_structure::storey_index(&storeys, z.origin.1), z)).collect();
    let mut volume = 0.0f64;
    // Which cells are air on the storey under the one being summed (under the ground: all).
    let mut below = vec![true; nx * nz];
    for (k, storey) in storeys.iter().enumerate() {
        let mut here = vec![false; nx * nz];
        for cz in 0..nz {
            for cx in 0..nx {
                let (x, z) = ((cx as f32 + 0.5) * CELL, (cz as f32 + 0.5) * CELL);
                let ci = cz * nx + cx;
                let air = if k == 0 {
                    true
                } else if slab_at(k, x, z).is_some() {
                    zones.iter().any(|(zk, zone)| *zk == k && zone.contains_xz(x, z))
                } else {
                    below[ci]
                };
                if !air {
                    continue;
                }
                here[ci] = true;
                // The cell's own area (the last row and column may be cut short by the box).
                let cw = ((cx as f32 + 1.0) * CELL).min(w) - cx as f32 * CELL;
                let cd = ((cz as f32 + 1.0) * CELL).min(d) - cz as f32 * CELL;
                let lid = if k + 1 < storeys.len() { slab_at(k + 1, x, z).unwrap_or(0.0) } else { 0.0 };
                volume += f64::from(cw * cd * (storey.height() - lid).max(0.0));
            }
        }
        below = here;
    }
    volume as f32
}

/// How high above its floor a space's AIR reaches (the box whose air a person breathes): its roof,
/// or a person standing on the top of the tallest thing built in it and jumping
/// (`HEADROOM_M`), whichever is higher. The tops are each piece's placed height plus its height
/// (src/ship/structure.rs: the top step of the stairs, the ramp's top, a deck's slab, a ladder's
/// top rung, the elevator's car at the top of its ride, which is one storey, its height).
///
/// The review of increment 4, P2: the box stopped at the roof (3 m for the homestead), so its own
/// elevator, ladder and stairs, which climb a storey, carried a person's eye to 4.7 m, out of
/// every air space: vacuum, hypoxia after 20 s. Before increment 4 the box around every room
/// reached the Commons' 8 m. The air's VOLUME is the space's rooms (`home_air_volume_m3`): this
/// is only where a person counts as in it.
///
/// Ship homes increment 7: a floor slab is a floor like the ground (a person stands on its top and
/// jumps: its top plus `HEADROOM_M`), and the rooms zoned on a storey above the ground reach their
/// own tops. A body with no slab has no storey above the ground, so this is what it always was.
pub fn air_top(body: &HomeStructure) -> f32 {
    let storeys = body.storeys();
    let pieces = body.structures.iter().filter_map(|s| crate::ship::structure::structure_type(&s.type_id).map(|t| s.pos.1 + t.size.1 + HEADROOM_M));
    let slabs = body.floors.iter().map(|s| s.top + HEADROOM_M);
    let upper_rooms = body
        .zones
        .iter()
        .filter(|z| crate::ship::home_structure::storey_index(&storeys, z.origin.1) > 0)
        .map(|z| z.origin.1 + z.size.1);
    pieces.chain(slabs).chain(upper_rooms).fold(body.height, f32::max)
}

/// The ship's air spaces as this game sees them (`ShipStructure::air_spaces`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AirSpaces {
    /// This player's own home: its box. None for a guest (the home is put away) or a ship with no
    /// home assembled.
    pub home: Option<Aabb>,
    /// The ship's shared spaces: every zone but the home, every corridor tube, and every plot but
    /// the home's own (a neighbour's home, which this game does not simulate: its air is part of
    /// the ship's as far as anyone here can tell).
    pub shared: Vec<Aabb>,
}

/// Whose air a point breathes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AirAt {
    /// This player's own home: its own sealed air.
    OwnHome,
    /// The ship's shared air.
    Ship,
    /// Neither: outside every pressurized space (on the hull, in space).
    Outside,
}

impl AirSpaces {
    /// Whose air `p` breathes: the home's own inside it (a home's box can overlap its own door
    /// corridor's mouth, and the home wins there), else the ship's inside any shared space.
    pub fn at(&self, p: Vec3) -> AirAt {
        if self.home.as_ref().is_some_and(|b| in_box(b, p)) {
            AirAt::OwnHome
        } else if self.shared.iter().any(|b| in_box(b, p)) {
            AirAt::Ship
        } else {
            AirAt::Outside
        }
    }
}

impl ShipStructure {
    /// The ship's box: every zone (not a home put away), plot, corridor tube and district, grown by
    /// `ABOARD_MARGIN_M` on every side. None for a ship with nothing in it.
    pub fn aboard_bounds(&self) -> Option<Aabb> {
        let mut boxes: Vec<Aabb> = Vec::new();
        for (zi, z) in self.zones.iter().enumerate() {
            if !self.is_away_home(zi) {
                let o = z.origin_vec();
                boxes.push((o, o + Vec3::new(z.body.width, z.body.height, z.body.depth)));
            }
        }
        boxes.extend(self.plots.iter().map(|p| p.aabb()));
        boxes.extend(self.districts.iter().map(|d| {
            let o = Vec3::new(d.origin.0, d.origin.1, d.origin.2);
            (o, o + Vec3::new(d.size.0, d.size.1, d.size.2))
        }));
        boxes.extend(self.corridors.iter().filter_map(|c| self.corridor_geometry(c).ok()).map(|g| tube_box(&g)));
        let first = *boxes.first()?;
        let (lo, hi) = boxes.iter().fold(first, |(lo, hi), (a, b)| (lo.min(*a), hi.max(*b)));
        Some((lo - Vec3::splat(ABOARD_MARGIN_M), hi + Vec3::splat(ABOARD_MARGIN_M)))
    }

    /// The air spaces (`AirSpaces`) of a home that breathes its own air: the home's own box, and the
    /// ship's shared ones.
    pub fn air_spaces(&self) -> AirSpaces {
        self.air_spaces_for(true)
    }

    /// `air_spaces` for a home that breathes its own air (`own_air`, data/homes/starts.ron: a
    /// homestead) or one on the ship's ventilation (`own_air` false: a cabin, ship homes increment
    /// 6). A home on the ship's air is no sealed space of its own: its box is one of the ship's
    /// shared spaces, so its player breathes the ship's air in it, as a berth does.
    pub fn air_spaces_for(&self, own_air: bool) -> AirSpaces {
        let own_plot = self.home_plot().map(|p| p.id.clone());
        let mut spaces = AirSpaces::default();
        for (zi, z) in self.zones.iter().enumerate() {
            if self.is_away_home(zi) {
                continue;
            }
            let o = z.origin_vec();
            // Up to where a person standing on its tallest piece can be (`air_top`), every zone,
            // so a teleporter, stairs or a deck built in a shared zone one day reaches its air too.
            let b = (o, o + Vec3::new(z.body.width, air_top(&z.body), z.body.depth));
            if z.id == HOME_ZONE_ID && own_air {
                spaces.home = Some(b);
            } else {
                spaces.shared.push(b);
            }
        }
        spaces.shared.extend(self.corridors.iter().filter_map(|c| self.corridor_geometry(c).ok()).map(|g| tube_box(&g)));
        for p in self.plots.iter().filter(|p| own_plot.as_deref() != Some(p.id.as_str())) {
            spaces.shared.push(p.aabb());
            // A neighbour's door corridor is not in `corridors` (only the home's own is): its tube
            // runs from the default design's door to the shared zone (src/ship/neighbours.rs).
            if let Ok(g) = self.plot_door_tube(p, HomeDesign::built_in_ref(&p.kind)) {
                spaces.shared.push(tube_box(&g));
            }
        }
        spaces
    }

    /// True when `p` (ship metres) is inside the ship's box (`aboard_bounds`). A ship with
    /// nothing in it holds no one.
    pub fn is_aboard(&self, p: Vec3) -> bool {
        self.aboard_bounds().is_some_and(|b| in_box(&b, p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    fn shipped(plot: &str) -> ShipStructure {
        ShipStructure::load_and_assemble_shipped(&data(), Some(plot)).expect("the shipped ship assembles")
    }

    /// ABOARD IS INSIDE THE SHIP'S BOUNDS, NOT A 400 M SPHERE: on the shipped ship every place a
    /// person can stand is aboard (the far end of First Street, the Factory District 190 m west),
    /// and so is a point on a 2 km drum 1 km along its length, which the sphere let go of; a point
    /// 60 m past the last district is not. Seen red 2026-10-04 with `is_aboard` measuring the
    /// old sphere (`p.length() < 400.0`): "past the box is not aboard" (310 m from the origin, 10 m
    /// past the ship's box, was still inside the sphere).
    #[test]
    fn aboard_is_inside_the_ships_bounds() {
        let ship = shipped("p1");
        for p in [Vec3::new(70.0, 1.7, 194.0), Vec3::new(-180.0, 1.7, 10.0), Vec3::new(26.0, 1.7, 40.0)] {
            assert!(ship.is_aboard(p), "{p} is aboard");
        }
        let (lo, hi) = ship.aboard_bounds().expect("the ship has a box");
        assert!(!ship.is_aboard(Vec3::new(hi.x + 10.0, 1.7, 10.0)), "past the box is not aboard");
        assert!(!ship.is_aboard(Vec3::new(10.0, lo.y - 10.0, 10.0)), "below the deck and the margin is not aboard");
        // A drum: one 2 km zone. 1 km along it is aboard; the old sphere had let go at 400 m.
        let mut drum = ship.clone();
        drum.zones[0].origin = (0.0, 0.0, -2000.0);
        drum.zones[0].body.depth = 2000.0;
        assert!(drum.is_aboard(Vec3::new(10.0, 1.7, -1000.0)), "1 km along the drum is aboard");
    }

    /// EACH HOME HAS ITS OWN AIR, sized from its own design, and only that home breathes it: on p1
    /// the home's box is p1's own home, the Commons and First Street breathe the ship's air, the
    /// corridor between them is the ship's, and the hull's outside is no one's. A guest (home put
    /// away) has no home air anywhere aboard. Seen red 2026-10-04 with `at` answering OwnHome for
    /// every room box, as the old `whereabouts` did with the box around every room: "the Commons
    /// breathes the ship's air, not the home's: OwnHome".
    #[test]
    fn each_home_breathes_its_own_air() {
        let ship = shipped("p1");
        let air = ship.air_spaces();
        let home = air.home.expect("p1's home has a box");
        assert_eq!(home.0, Vec3::ZERO, "the home stands on p1");
        assert_eq!(air.at(Vec3::new(26.0, 1.7, 40.0)), AirAt::OwnHome);
        assert_eq!(air.at(Vec3::new(80.0, 1.7, 60.0)), AirAt::Ship, "the Commons breathes the ship's air, not the home's: {:?}", air.at(Vec3::new(80.0, 1.7, 60.0)));
        assert_eq!(air.at(Vec3::new(70.0, 1.7, 150.0)), AirAt::Ship, "First Street");
        assert_eq!(air.at(Vec3::new(60.0, 1.7, 40.0)), AirAt::Ship, "the corridor from the home to the Commons");
        assert_eq!(air.at(Vec3::new(26.0, 1.7, 140.0)), AirAt::Ship, "the neighbour's plot, p2");
        assert_eq!(air.at(Vec3::new(26.0, 30.0, 40.0)), AirAt::Outside, "over the roof");
        // The home's air is its own design's volume: the homestead's 55 x 89 x 3 m.
        let body = &ship.zones[ship.home_zone_index()].body;
        assert!((home_air_volume_m3(body) - 55.0 * 89.0 * 3.0).abs() < 1.0);
        // On p2 the home's air moves with the home, and p1 is a neighbour's.
        let on_p2 = shipped("p2").air_spaces();
        assert_eq!(on_p2.at(Vec3::new(26.0, 1.7, 140.0)), AirAt::OwnHome);
        assert_eq!(on_p2.at(Vec3::new(26.0, 1.7, 40.0)), AirAt::Ship);
        // A guest: the home is put away, so nowhere aboard is home air, and the place it is kept
        // is not a space anyone breathes in.
        let guest = ship.put_home_away().expect("the home stood on a plot").air_spaces();
        assert_eq!(guest.home, None);
        assert_eq!(guest.at(Vec3::new(26.0, 1.7, 40.0)), AirAt::Ship, "p1 is a neighbour's now");
        let kept = Vec3::from(crate::ship::ship_structure::HOME_AWAY_ORIGIN) + Vec3::new(5.0, 1.7, 5.0);
        assert_eq!(guest.at(kept), AirAt::Outside);
    }

    /// EVERY PLOT IS ABOARD AND BREATHES ITS OWN AIR (the twelve plots along First Street,
    /// 2026-10-04). With the home on any plot: that plot's whole box and its door are aboard (the
    /// ship's box grows with its plots and its street, where the 400 m sphere of before increment
    /// 4 would have let go of p5 to p12), the home's air is that plot's and nowhere else's, and
    /// every other plot and First Street outside its door are the ship's air.
    ///
    /// Seen red 2026-10-04 on the two-plot ship file: "the twelve plots along First Street: 2";
    /// and on the twelve-plot file with `is_aboard` put back to the sphere (`p.length() <
    /// 400.0`): "p5's corner [55, 3, 485] is aboard".
    ///
    /// SINCE SHIP HOMES INCREMENT 6 (lane 1A) the ship file holds sixteen cabin plots too, and
    /// each plot is assembled with the shipped design of its OWN kind (a cabin on c1, never the
    /// homestead, which `assemble` refuses there), and "outside its door" is a metre into the zone
    /// its door corridor runs to: First Street, the Commons, or the cabins' hall. Seen red
    /// 2026-10-12 with every plot given the shipped homestead (the design of p1's kind, which
    /// this test's assembly put everywhere while every plot was a homestead): "c1 takes the
    /// shipped cabin: plot 'c1' takes a cabin home, not a homestead".
    /// (This is the own-air answer, `air_spaces`, for every plot: whether a home breathes its own
    /// air is its start's `own_air` in data/homes/starts.ron, and a cabin's is false,
    /// `a_cabin_breathes_the_ships_air` below.)
    #[test]
    fn every_plot_is_aboard_and_breathes_its_own_air() {
        let file = ShipStructure::load_ship_file(&data()).expect("the ship file loads");
        assert!(file.plots.len() >= 28, "the twelve homesteads along First Street and the sixteen cabins: {}", file.plots.len());
        for p in &file.plots {
            let design = HomeDesign::built_in(&p.kind).unwrap_or_else(|| panic!("the shipped {} design is built in", p.kind));
            let ship = file.clone().assemble(design, &p.id).unwrap_or_else(|e| panic!("{} takes the shipped {}: {e}", p.id, p.kind));
            let (lo, hi) = p.aabb();
            for c in [lo, hi, Vec3::new(lo.x, hi.y, hi.z), Vec3::new(hi.x, lo.y, lo.z)] {
                assert!(ship.is_aboard(c), "{}'s corner {c} is aboard", p.id);
            }
            let door = ship.home_spawn_world().unwrap_or_else(|| panic!("the shipped {} names its door", p.kind));
            assert!(ship.is_aboard(door), "{}'s door {door} is aboard", p.id);
            let air = ship.air_spaces();
            assert_eq!(air.home.map(|b| b.0), Some(lo), "{}: the home's air box stands on its plot", p.id);
            assert_eq!(air.at(door), AirAt::OwnHome, "{}'s door breathes its own home's air", p.id);
            let middle = (lo + hi) * 0.5 + Vec3::new(0.0, 1.7 - (hi.y - lo.y) * 0.5, 0.0);
            assert_eq!(air.at(middle), AirAt::OwnHome, "{}'s middle breathes its own home's air", p.id);
            // Out of the door, a metre into the zone its corridor runs to: the ship's air.
            let g = ship.corridor_geometry(&ship.corridors[0]).expect("the home's door corridor resolves");
            let outside = g.end_to + (g.end_to - g.end_from).normalize() * 1.0 + Vec3::new(0.0, 1.7, 0.0);
            assert_eq!(air.at(outside), AirAt::Ship, "{}: outside its door, {outside}, is the ship's air", p.id);
            for q in file.plots.iter().filter(|q| q.id != p.id) {
                let (qlo, qhi) = q.aabb();
                let mid = Vec3::new((qlo.x + qhi.x) * 0.5, 1.7, (qlo.z + qhi.z) * 0.5);
                assert_eq!(air.at(mid), AirAt::Ship, "with the home on {}, the neighbour's plot {} is the ship's air", p.id, q.id);
            }
        }
    }

    /// A body to test with: `width` x `depth` x `height`, nothing in it.
    fn bare(width: f32, depth: f32, height: f32) -> HomeStructure {
        ron::from_str(&format!("(width: {width}, depth: {depth}, height: {height})")).expect("a bare body parses")
    }

    /// A solid 0.25 m floor slab over a-b, its top at `top`.
    fn slab(a: (f32, f32), b: (f32, f32), top: f32) -> crate::ship::home_structure::FloorSlab {
        ron::from_str(&format!("(a: ({}, {}), b: ({}, {}), top: {top})", a.0, a.1, b.0, b.1)).expect("a slab parses")
    }

    /// A zone of a room on a storey: its min corner and size, a room type of none.
    fn zone(id: &str, origin: (f32, f32, f32), size: (f32, f32, f32)) -> crate::ship::home_structure::Zone {
        crate::ship::home_structure::Zone { id: id.into(), type_id: "room_hall".into(), origin, size, label: String::new(), room_type: None }
    }

    /// AIR REACHES OVER A SLAB AND THE ROOMS ON IT (ship homes increment 7, plan section 3:
    /// `air_top` counts slab tops and upper zones' tops). In a 3 m body with a mezzanine slab at
    /// 2.5 m, a person standing on the slab and jumping reaches 2.5 + `HEADROOM_M`, so the air does;
    /// a room zoned on that storey 4 m tall reaches 6.5 m. A body with no slab and no piece keeps its
    /// roof, and a zone on its ground storey (every zone written before storeys) changes nothing.
    ///
    /// Seen red 2026-10-20 with main's air_top put back (slabs and zones not counted): "the air over a 2.5 m slab stops
    /// at 3".
    #[test]
    fn air_reaches_over_a_slab_and_its_upper_rooms() {
        let mut body = bare(10.0, 10.0, 3.0);
        body.zones.push(zone("ground", (0.0, 0.0, 0.0), (10.0, 5.0, 10.0)));
        assert_eq!(air_top(&body), 3.0, "a one-storey body's air stops at its roof");
        body.floors.push(slab((0.0, 0.0), (4.0, 10.0), 2.5));
        assert!((air_top(&body) - (2.5 + HEADROOM_M)).abs() < 1e-5, "the air over a 2.5 m slab stops at {}", air_top(&body));
        body.zones.push(zone("mezzanine", (0.0, 2.5, 0.0), (4.0, 4.0, 10.0)));
        assert!((air_top(&body) - 6.5).abs() < 1e-5, "the air over the mezzanine's room stops at {}", air_top(&body));
    }

    /// A HOME'S AIR IS ITS ROOMS, STOREY BY STOREY (plan section 3: "the home's air volume is the
    /// sum of its rooms per storey"; docs/design/homestead-upper-floor.md section 9 item 3(b): the
    /// space over a lid nobody uses is no one's air). A 10 x 10 x 6 m body with a 0.25 m slab at 3.0
    /// over its north half and an upstairs room on half of that slab holds: the ground storey's
    /// 300 m3 less the slab's 12.5, the upstairs room's 25 m2 by 3 m, and the south half's upper
    /// 3 m, open to the room below as a stairwell or the greenhouse is: 512.5 m3. The lid's other
    /// half, with no room on it, holds none. A body with no slab is its box, as it always was, and
    /// so is a body whose only zones are on the ground.
    ///
    /// Seen red 2026-10-20 with main's volume put back (the box, whatever its storeys): "the two-storey body holds
    /// 600 m3, want 512.5".
    #[test]
    fn a_homes_air_is_its_rooms_storey_by_storey() {
        let mut body = bare(10.0, 10.0, 6.0);
        assert_eq!(home_air_volume_m3(&body), 600.0, "a body with no slab is its box");
        body.floors.push(slab((0.0, 0.0), (10.0, 5.0), 3.0));
        body.zones.push(zone("upstairs", (0.0, 3.0, 0.0), (10.0, 3.0, 2.5)));
        let v = home_air_volume_m3(&body);
        assert!((v - 512.5).abs() < 1e-3, "the two-storey body holds {v} m3, want 512.5");
        // Without the upstairs room, the lid's whole top is no one's air.
        body.zones.clear();
        let v = home_air_volume_m3(&body);
        assert!((v - 437.5).abs() < 1e-3, "with nobody upstairs the body holds {v} m3, want 437.5");
    }

    /// A CABIN BREATHES THE SHIP'S AIR (ship homes increment 6, plan 3.7): its start says
    /// `own_air: false` (data/homes/starts.ron), a berth on the ship's ventilation. So the cabin is
    /// no sealed space of its own: standing in it is the ship's air, as in the hall outside, and
    /// the game simulates no 20.5 m3 box (3.15 by 2.5 by 2.6 m, the shipped cabin's body) that its
    /// one person would use up. A homestead, whose start says `own_air: true`, keeps its own.
    ///
    /// Seen red 2026-10-12 with `air_spaces_for` ignoring `own_air` (the home zone always the
    /// home's own): "a cabin has 20.474998 m3 of its own air: Some((Vec3(80.5, 0.0, 88.0),
    /// Vec3(83.65, 2.6, 90.5)))".
    #[test]
    fn a_cabin_breathes_the_ships_air() {
        let starts = crate::ship::starts::Starts::load(&data());
        let own_air = |start: &str| starts.get(start).unwrap_or_else(|| panic!("the {start} start")).own_air;
        let file = ShipStructure::load_ship_file(&data()).expect("the ship file loads");
        let cabin = HomeDesign::built_in("cabin").expect("the cabin is built in");
        let ship = file.assemble(cabin, "c1").expect("the cabin goes on c1");
        let air = ship.air_spaces_for(own_air("cabin"));
        let body = &ship.zones[ship.home_zone_index()].body;
        assert!(air.home.is_none(), "a cabin has {} m3 of its own air: {:?}", home_air_volume_m3(body), air.home);
        let (lo, hi) = ship.home_plot().expect("the cabin stands on c1").aabb();
        let middle = Vec3::new((lo.x + hi.x) * 0.5, 1.7, (lo.z + hi.z) * 0.5);
        assert_eq!(air.at(middle), AirAt::Ship, "standing in the cabin is the ship's air");
        let door = ship.home_spawn_world().expect("the cabin names its door");
        assert_eq!(air.at(door), AirAt::Ship, "and so is its door");
        // The homestead keeps its own air.
        let homestead = shipped("p1").air_spaces_for(own_air("homestead"));
        assert_eq!(homestead.at(Vec3::new(26.0, 1.7, 40.0)), AirAt::OwnHome, "a homestead breathes its own air");
    }
}
