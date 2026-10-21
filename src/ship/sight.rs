//! WHAT THE EYE CAN SEE ABOARD, IN THREE DIMENSIONS (ship homes increment 7 review).
//!
//! Two things in the game ask "can the player see that from here?": the walk-up target (the
//! machine E acts on, lib.rs) and the HUD's floating machine labels and crew nameplates
//! (gui/pages/hud.rs). Until the homestead had an upper floor both answered in plan only: a wall
//! anywhere on the line between the eye and the label, at any height, hid it, and nothing else
//! did. With storeys that is wrong both ways. A wall upstairs hid a label on the ground floor
//! under it, and a ground wall hid one upstairs over it; and nothing at all hid the bedroom's
//! machines from the dining room right under them, so their cards floated at the ceiling and E
//! reached up through the floor to sleep in the bed upstairs.
//!
//! So a sight line here is a 3-D segment, eye to target, and:
//! - a WALL (`WallSegment`) hides what is behind it only where the line crosses it between the
//!   wall's own foot and top (`y0`..`y1`): a wall upstairs is over the line of a person
//!   downstairs, a ground wall under the line of one upstairs;
//! - a SOLID floor slab hides what is on the other side of it (the floor of the storey above is
//!   the ceiling of the room below); a GLASS one does not, you see through a glass lid as through
//!   the glass roof (`FloorSlab::passes_light`);
//! - windows, guards and open doors are not blockers at all (the wall's sight spans,
//!   `wall_collision::sight_segments_with_shell_cuts`), and a shut door blocks between its sill
//!   and its head (`wall_collision::door_panel_segment`), which the caller adds.
//!
//! The walk-up target also keeps to the relay's reach (`moves::in_reach`): within 5 m and on the
//! player's storey by the home's own storeys, measured at the machine's foot (`walk_up_target`,
//! `storey_floor`).

use crate::ship::home_structure::{HomeStructure, SlabMaterial};
use crate::ship::ship_structure::ShipStructure;
use crate::ship::wall_collision::WallSegment;
use glam::Vec3;

/// A SOLID floor slab as a sight blocker: its box, from its underside to its top over its
/// footprint, in whatever frame the blockers are in (a body's, the ship's, or the HUD's world
/// frame with the station offset added).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlabBox {
    pub min: Vec3,
    pub max: Vec3,
}

impl SlabBox {
    /// This box moved by `by`.
    pub fn moved(self, by: Vec3) -> Self {
        SlabBox { min: self.min + by, max: self.max + by }
    }

    /// True when the straight line `eye` -> `target` passes through the INSIDE of the box. A
    /// line that only touches a face, an edge or a corner does not (the open box), so a label
    /// standing on a slab is not hidden by the slab under it.
    pub fn blocks(&self, eye: Vec3, target: Vec3) -> bool {
        // The slab method: on each axis, the part of the line (t from 0 at the eye to 1 at the
        // target) strictly between the box's two faces; the line is inside the box where all
        // three parts overlap, and it enters it only if that overlap is more than a point.
        let d = target - eye;
        let (mut t0, mut t1) = (0.0f32, 1.0f32);
        for k in 0..3 {
            let (o, dk, lo, hi) = (eye[k], d[k], self.min[k], self.max[k]);
            if dk.abs() < 1e-9 {
                // Running along this axis: inside the box on it everywhere, or nowhere.
                if o <= lo || o >= hi {
                    return false;
                }
                continue;
            }
            let (a, b) = ((lo - o) / dk, (hi - o) / dk);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
            if t0 >= t1 {
                return false;
            }
        }
        true
    }
}

/// Every sight blocker the HUD and the walk-up test against, in one frame: the walls (static
/// sight spans and the shut doors, each with the heights it stands between) and the solid slabs.
/// Rebuilt by lib.rs once a frame, reusing its two lists, so a frame allocates nothing.
#[derive(Clone, Debug, Default)]
pub struct SightBlockers {
    pub walls: Vec<WallSegment>,
    pub slabs: Vec<SlabBox>,
}

impl SightBlockers {
    /// Empty both lists, keeping their room.
    pub fn clear(&mut self) {
        self.walls.clear();
        self.slabs.clear();
    }

    /// True when there is nothing to block sight (no home, the build editor): every label shows.
    pub fn is_empty(&self) -> bool {
        self.walls.is_empty() && self.slabs.is_empty()
    }

    /// True when something stands between `eye` and `target`.
    ///
    /// COST: one plan-view crossing test per wall (four 2-D cross products, and the height only
    /// for a wall the line does cross) and one segment-box test per slab (three divisions). The
    /// HUD runs it once per label inside the dot reach (21 m) per frame, and the walk-up only for
    /// a machine already in reach and in the look cone. The homestead aboard the ship is a few
    /// hundred wall spans and about ten solid slabs, so a hundred labels in reach cost some tens
    /// of thousands of these tests a frame, a small fraction of a millisecond.
    pub fn blocked(&self, eye: Vec3, target: Vec3) -> bool {
        self.walls.iter().any(|w| wall_blocks(w, eye, target)) || self.slabs.iter().any(|s| s.blocks(eye, target))
    }
}

/// True when the line `eye` -> `target` crosses the wall at a height the wall stands at: it
/// crosses the wall's centre line in plan (a proper crossing, as `wall_collision::sight_blocked`
/// has always tested: grazing an end or running along the wall does not count, which errs toward
/// showing a label at a doorway's edge rather than blinking it), and at that point the line is
/// above the wall's foot and below its top.
pub fn wall_blocks(seg: &WallSegment, eye: Vec3, target: Vec3) -> bool {
    // Twice the signed area of the triangle (a, b, c) in plan: which side of a..b the point c is.
    let orient = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    let (e, t) = ((eye.x, eye.z), (target.x, target.z));
    // The wall's ends on opposite sides of the line, and the line's ends on opposite sides of
    // the wall (strictly): a proper crossing.
    if orient(e, t, seg.a) * orient(e, t, seg.b) >= 0.0 {
        return false;
    }
    let (de, dt) = (orient(seg.a, seg.b, e), orient(seg.a, seg.b, t));
    if de * dt >= 0.0 {
        return false;
    }
    // `de` and `dt` are the two ends' distances from the wall's line (both scaled alike), so the
    // line meets the wall `de / (de - dt)` of the way from the eye to the target.
    let y = eye.y + (target.y - eye.y) * (de / (de - dt));
    y > seg.y0 && y < seg.y1
}

/// The SOLID slabs of one body, as boxes in the body's own frame. A glass slab is left out: it is
/// a floor and a lid, but it is seen through.
pub fn solid_slabs(home: &HomeStructure) -> impl Iterator<Item = SlabBox> + '_ {
    home.floors.iter().filter(|s| s.material == SlabMaterial::Solid).map(|s| {
        let (x0, z0, x1, z1) = s.rect();
        let (y0, y1) = s.y_range();
        SlabBox { min: Vec3::new(x0, y0, z0), max: Vec3::new(x1, y1, z1) }
    })
}

/// The SOLID slabs of the whole ship in the ship's frame: each zone's, moved to that zone's
/// origin, as `wall_collision::ship_sight_segments` places each zone's walls. A home put away has
/// none (it is not drawn). The neighbours' homes are left out: they are drawn but no label of
/// this game is in them, and their own walls already stand between them and this home.
pub fn ship_solid_slabs(ship: &ShipStructure) -> impl Iterator<Item = SlabBox> + '_ {
    ship.zones.iter().enumerate().filter(move |(zi, _)| !ship.is_away_home(*zi)).flat_map(|(_, zone)| {
        let origin = Vec3::new(zone.origin.0, zone.origin.1, zone.origin.2);
        solid_slabs(&zone.body).map(move |b| b.moved(origin))
    })
}

/// How close to straight ahead a machine must be for the walk-up to pick it: the cosine of the
/// half-angle of the look cone, 0.9 (about 25 degrees), as it has been since v0.431.
pub const WALK_UP_CONE_COS: f32 = 0.9;

/// THE FLOOR OF THE STOREY A POINT IS ON, from the home's own storeys (its slab tops,
/// `HomeStructure::storeys`): the zone of `ship` that holds the point `p` (ship frame; in plan,
/// and from half a metre under its floor up to its top), and that zone's storey by
/// `storey_index`, as a ship-frame height. None outside every zone (a corridor, a home put away,
/// off the ship). Two points are on one storey when their floors are one height (`STOREY_EPS`),
/// so a home with 2.5 m storeys is read as it is built, not by a 3 m rule of thumb; and two zones
/// side by side on one deck share their ground storey.
///
/// COST: one plan test per zone, and the storeys of the one zone that holds the point, worked out
/// from its slabs (a small list). The walk-up asks it once for the player and once per machine
/// that is already in reach and in the look cone, so a handful of times a frame.
pub fn storey_floor(ship: &ShipStructure, p: Vec3) -> Option<f32> {
    use crate::ship::home_structure::storey_index;
    let zone = ship.zones.iter().enumerate().filter(|(zi, _)| !ship.is_away_home(*zi)).map(|(_, z)| z).find(|z| {
        let (b, (ox, oy, oz)) = (&z.body, z.origin);
        p.x >= ox
            && p.x <= ox + b.width
            && p.z >= oz
            && p.z <= oz + b.depth
            && p.y >= oy - crate::ship::moves::STOREY_BAND_BELOW_M
            && p.y <= oy + b.height
    })?;
    let storeys = zone.body.storeys();
    let k = storey_index(&storeys, p.y - zone.origin.1);
    storeys.get(k).map(|s| zone.origin.1 + s.floor)
}

/// THE WALK-UP TARGET: which machine E acts on. `machines` gives each machine's label anchor
/// (the point the HUD draws its dot at, 0.4 m over its top) and the height it stands at (its
/// foot), both in the frame of `eye`; `feet` is the height of the player's feet in that frame;
/// `storey_floor` says which storey a point of that frame is on (the floor's height; None where
/// no home says, `storey_floor` above with the frame's offset). The answer is the index of the
/// nearest machine (by its label) that is
/// - within reach (`moves::INTERACT_REACH_M`, 5 m, to its label: the relay's reach,
///   `moves::in_reach`, is the outer limit);
/// - on the player's storey: the storey its FOOT is on is the one the player's feet are on, by
///   the home's own storeys. Its foot, not its label, which floats 0.4 m over the machine's top
///   (a 2.4 m tower's is 2.8 m over its floor) and so is on no storey in particular; and a unit
///   hung under a ceiling (the greenhouse's air handlers stand 2.7 m up) is on the storey under
///   that ceiling. Where no home says (a point in no zone), the relay's own band at the foot
///   (`moves::on_my_storey`'s, from the given feet);
/// - inside the look cone (`WALK_UP_CONE_COS` of `forward`);
/// - and in sight (`SightBlockers::blocked`: no wall across the line at its height, no solid
///   slab between).
///
/// The storey and the sight are tested last, and only for a machine that would beat the best so
/// far, so a frame pays for them a handful of times at most.
pub fn walk_up_target(
    eye: Vec3,
    feet: f32,
    forward: Vec3,
    machines: impl IntoIterator<Item = (Vec3, f32)>,
    blockers: &SightBlockers,
    storey_floor: impl Fn(Vec3) -> Option<f32>,
) -> Option<usize> {
    use crate::ship::home_structure::STOREY_EPS;
    use crate::ship::moves::{INTERACT_REACH_M, STOREY_BAND_ABOVE_M, STOREY_BAND_BELOW_M};
    let mine = storey_floor(Vec3::new(eye.x, feet, eye.z));
    let mut best: Option<(usize, f32)> = None;
    for (i, (label, foot)) in machines.into_iter().enumerate() {
        let to = label - eye;
        let dist = to.length();
        if !(0.05..=INTERACT_REACH_M).contains(&dist) {
            continue;
        }
        if (to / dist).dot(forward) < WALK_UP_CONE_COS {
            continue;
        }
        if best.is_some_and(|b| dist >= b.1) {
            continue;
        }
        let same_storey = match (mine, storey_floor(Vec3::new(label.x, foot, label.z))) {
            (Some(a), Some(b)) => (a - b).abs() < STOREY_EPS,
            _ => (-STOREY_BAND_BELOW_M..=STOREY_BAND_ABOVE_M).contains(&(foot - feet)),
        };
        if !same_storey || blockers.blocked(eye, label) {
            continue;
        }
        best = Some((i, dist));
    }
    best.map(|b| b.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship::wall_collision::ship_sight_segments;
    use std::collections::HashMap;
    use std::path::Path;

    /// One machine as the game places it: (id, room, label anchor, foot), ship frame.
    type Placed = (String, String, Vec3, f32);

    /// THE HOMESTEAD AS THE GAME WALKS IT: the shipped ship with the shipped homestead on its
    /// default plot, its sight blockers as lib.rs builds them (the ship's sight spans with every
    /// door open, and its solid slabs), and every machine of data/machines/home.ron where the
    /// game places it, its label 0.4 m over its top (engine/home_meshes.rs, world_load.rs).
    struct Aboard {
        blockers: SightBlockers,
        /// The home zone's origin: the homestead's own (body-local) coordinates plus this are
        /// ship coordinates.
        origin: Vec3,
        /// The homestead's storeys, in its own heights.
        storeys: Vec<crate::ship::home_structure::Storey>,
        machines: Vec<Placed>,
        ship: ShipStructure,
    }

    fn aboard() -> Aboard {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let ship = ShipStructure::load_and_assemble_shipped(&root.join("data"), None).expect("the shipped ship assembles");
        let hz = &ship.zones[ship.home_zone_index()];
        let origin = Vec3::new(hz.origin.0, hz.origin.1, hz.origin.2);
        let blockers = SightBlockers { walls: ship_sight_segments(&ship), slabs: ship_solid_slabs(&ship).collect() };
        let home = crate::machines::MachineHome::load(&root.join("data").join("machines").join("home.ron")).expect("home.ron parses");
        let machines = home
            .placements(&HashMap::new(), Some(&ship.zone_rects()))
            .into_iter()
            .map(|p| (p.id, p.room, Vec3::new(p.pos.0, p.top_y + 0.4, p.pos.2), p.pos.1))
            .collect();
        let storeys = hz.body.storeys();
        Aboard { blockers, origin, storeys, machines, ship }
    }

    impl Aboard {
        /// A point of the homestead, from its own coordinates.
        fn at(&self, x: f32, y: f32, z: f32) -> Vec3 {
            self.origin + Vec3::new(x, y, z)
        }

        fn machine(&self, id: &str) -> &Placed {
            self.machines.iter().find(|m| m.0 == id).unwrap_or_else(|| panic!("{id} is not in data/machines/home.ron"))
        }

        /// The ids of `room`'s machines whose labels are within `reach` of `eye` and SHOW (no
        /// blocker between).
        fn shown(&self, eye: Vec3, room: &str, reach: f32) -> Vec<&str> {
            self.machines
                .iter()
                .filter(|m| m.1 == room && m.2.distance(eye) <= reach && !self.blockers.blocked(eye, m.2))
                .map(|m| m.0.as_str())
                .collect()
        }

        /// How many of `room`'s machines are within `reach` of `eye`, shown or not.
        fn near(&self, eye: Vec3, room: &str, reach: f32) -> usize {
            self.machines.iter().filter(|m| m.1 == room && m.2.distance(eye) <= reach).count()
        }

        /// The machine E acts on, looking from `eye` straight at `id`'s label.
        fn e_at(&self, eye: Vec3, id: &str) -> Option<&str> {
            let fwd = (self.machine(id).2 - eye).normalize();
            let storey = |p: Vec3| storey_floor(&self.ship, p);
            walk_up_target(eye, eye.y - 1.7, fwd, self.machines.iter().map(|m| (m.2, m.3)), &self.blockers, storey).map(|i| self.machines[i].0.as_str())
        }
    }

    /// The HUD's card reach, metres (gui/mod.rs `machine_label_card_dist`'s default).
    const CARD_M: f32 = 8.0;

    /// A WALL HIDES ONLY ACROSS ITS OWN HEIGHT. A wall upstairs (3 to 6 m) hides a label on the
    /// upper floor from an eye on it and nothing on the ground floor under it; a ground wall (0
    /// to 3 m) the reverse; a wall at every height (a blocker whose heights are not known) hides
    /// both; grazing a wall's end hides nothing, as in plan before (`sight_blocked`).
    ///
    /// Seen red first (2026-10-21) with today's rule, the plan-view crossing at any height:
    /// "a wall upstairs hid a label downstairs".
    #[test]
    fn a_wall_hides_only_across_its_own_height() {
        let upstairs = WallSegment { a: (5.0, 0.0), b: (5.0, 10.0), half_thickness: 0.075, y0: 3.0, y1: 6.0 };
        let ground = WallSegment { y0: 0.0, y1: 3.0, ..upstairs };
        let every = WallSegment::at_every_height(upstairs.a, upstairs.b, 0.075);
        let one = |w: WallSegment| SightBlockers { walls: vec![w], slabs: Vec::new() };
        let (eye_down, label_down) = (Vec3::new(2.0, 1.7, 5.0), Vec3::new(8.0, 1.0, 5.0));
        let (eye_up, label_up) = (Vec3::new(2.0, 4.7, 5.0), Vec3::new(8.0, 4.0, 5.0));
        assert!(!one(upstairs).blocked(eye_down, label_down), "a wall upstairs hid a label downstairs");
        assert!(one(upstairs).blocked(eye_up, label_up), "a wall upstairs hides a label beyond it upstairs");
        assert!(one(ground).blocked(eye_down, label_down), "a ground wall hides a label beyond it downstairs");
        assert!(!one(ground).blocked(eye_up, label_up), "a ground wall hid a label upstairs");
        assert!(one(every).blocked(eye_down, label_down) && one(every).blocked(eye_up, label_up), "a wall at every height hides both");
        // Looking down a stairwell, the line drops through the ground wall's top or passes over it.
        assert!(one(ground).blocked(Vec3::new(2.0, 4.7, 5.0), Vec3::new(8.0, 0.5, 5.0)), "the line crosses the ground wall at 2.6 m");
        assert!(!one(ground).blocked(Vec3::new(4.0, 4.7, 5.0), Vec3::new(8.0, 0.5, 5.0)), "the line passes over the ground wall at 3.65 m");
        assert!(!one(every).blocked(Vec3::new(2.0, 1.7, 10.0), Vec3::new(8.0, 1.0, 10.0)), "grazing the wall's end");
        assert!(wall_blocks(&ground, eye_down, label_down) && !wall_blocks(&upstairs, eye_down, label_down), "the one-wall test agrees");
    }

    /// A SOLID SLAB HIDES THE STOREY BEYOND IT, A GLASS ONE DOES NOT. Over a 10 m square at 3 m
    /// (its underside at 2.75): a label upstairs is hidden from an eye downstairs and the reverse;
    /// a glass slab is no blocker (`solid_slabs` leaves it out); a label standing on the slab is
    /// seen from upstairs (the line touches the top face, never enters); a line down the
    /// stairwell beside the slab passes; and a line from the eye under it to a label under it
    /// passes under it.
    ///
    /// Seen red first (2026-10-21) with today's rule, no slab blocks: "a solid slab hid nothing
    /// upstairs from downstairs".
    #[test]
    fn a_solid_slab_hides_the_storey_beyond_it_and_glass_does_not() {
        // Any body will do: only its slabs are read.
        let mut home = crate::ship::ship_structure::HomeDesign::built_in("homestead").expect("the homestead is built in").body;
        home.floors = vec![crate::ship::home_structure::FloorSlab {
            a: (0.0, 0.0),
            b: (10.0, 10.0),
            top: 3.0,
            thickness: 0.25,
            material: SlabMaterial::Solid,
            label: None,
        }];
        let b = SightBlockers { walls: Vec::new(), slabs: solid_slabs(&home).collect() };
        assert_eq!(b.slabs, vec![SlabBox { min: Vec3::new(0.0, 2.75, 0.0), max: Vec3::new(10.0, 3.0, 10.0) }]);
        let (down, up) = (Vec3::new(5.0, 1.7, 5.0), Vec3::new(4.0, 4.0, 5.0));
        assert!(b.blocked(down, up), "a solid slab hid nothing upstairs from downstairs");
        assert!(b.blocked(Vec3::new(5.0, 4.7, 5.0), Vec3::new(4.0, 1.0, 5.0)), "nor downstairs from upstairs");
        assert!(!b.blocked(Vec3::new(5.0, 4.7, 5.0), Vec3::new(7.0, 3.0, 7.0)), "a label standing on the slab");
        assert!(!b.blocked(Vec3::new(9.0, 4.7, 5.0), Vec3::new(12.0, 1.0, 5.0)), "down the stairwell beside it");
        assert!(!b.blocked(down, Vec3::new(8.0, 1.0, 5.0)), "under it, both");
        home.floors[0].material = SlabMaterial::Glass;
        assert_eq!(solid_slabs(&home).count(), 0, "a glass slab is seen through");
        // The box moves with its zone.
        let moved = SlabBox { min: Vec3::ZERO, max: Vec3::ONE }.moved(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(moved, SlabBox { min: Vec3::new(1.0, 2.0, 3.0), max: Vec3::new(2.0, 3.0, 4.0) });
    }

    /// THE ROOMS OVER AND UNDER A SOLID FLOOR DO NOT SEE EACH OTHER'S CARDS (review of increment
    /// 7, case a). In the homestead's dining room, right under the bedroom (34..42, 30..38 on both
    /// storeys), none of the bedroom's machines within the card reach shows (bed_1, desk_2, the
    /// nightstands, shelf_6 used to float at the ceiling), while the dining room's own do; and
    /// from the dressing room over the second bedroom, none of the ground floor's under it shows,
    /// while the dressing room's own do.
    ///
    /// Seen red first (2026-10-21) with today's rule (plan only, no slab): "from the dining room,
    /// the bedroom's cards show through its ceiling: ["bed_1", "nightstand_1", "nightstand_2",
    /// "rug_2", "standing_mirror_1", "desk_2", "chair_6", "shelf_6", "chair_9", "side_table_4"]".
    #[test]
    fn the_rooms_over_and_under_a_solid_floor_do_not_see_each_others_cards() {
        let w = aboard();
        let dining = w.at(36.5, 1.7, 34.0);
        assert!(w.near(dining, "room-bedroom", CARD_M) >= 6, "the bedroom's machines are over the dining room");
        let over = w.shown(dining, "room-bedroom", CARD_M);
        assert!(over.is_empty(), "from the dining room, the bedroom's cards show through its ceiling: {over:?}");
        assert_eq!(w.shown(dining, "room-dining", CARD_M).len(), w.near(dining, "room-dining", CARD_M), "the dining room's own cards all show");
        let dressing = w.at(43.5, 4.7, 34.0);
        for room in ["room-bedroom-2", "room-dining"] {
            let under = w.shown(dressing, room, CARD_M);
            assert!(under.is_empty(), "from the dressing room, {room}'s cards show through its floor: {under:?}");
        }
        assert!(w.near(dressing, "room-bedroom-2", CARD_M) >= 4, "the second bedroom is under the dressing room");
        assert_eq!(w.shown(dressing, "room-dressing", CARD_M).len(), w.near(dressing, "room-dressing", CARD_M), "the dressing room's own cards all show");
    }

    /// A WALL UPSTAIRS HIDES NOTHING DOWNSTAIRS (case b). The upstairs wall between the
    /// dressing room and the bathrooms (x 45, 30..38, from 3.0 m) stands over the second
    /// bedroom, whose ground walls at x 45 were taken out: from beside bed_2 the wardrobe, the
    /// desk and the rug across the room show.
    ///
    /// Seen red first (2026-10-21) with today's rule: "from beside bed_2, wardrobe_4 is hidden".
    #[test]
    fn a_wall_upstairs_hides_nothing_downstairs() {
        let w = aboard();
        let eye = w.at(43.0, 1.7, 36.5);
        for id in ["wardrobe_4", "desk_5", "rug_12"] {
            assert!(!w.blockers.blocked(eye, w.machine(id).2), "from beside bed_2, {id} is hidden");
        }
    }

    /// A WALL DOWNSTAIRS HIDES NOTHING UPSTAIRS (case c). Entering the study, the ground floor's
    /// wall between the shower room and the boot room (z 34, 50..55, up to the slab) is under the
    /// floor: the chair and the bookshelves across the study show.
    ///
    /// Seen red first (2026-10-21) with today's rule: "entering the study, chair_8 is hidden".
    #[test]
    fn a_wall_downstairs_hides_nothing_upstairs() {
        let w = aboard();
        let eye = w.at(53.45, 4.7, 37.4);
        for id in ["chair_8", "bookshelf_5", "bookshelf_2"] {
            assert!(!w.blockers.blocked(eye, w.machine(id).2), "entering the study, {id} is hidden");
        }
    }

    /// E REACHES NOTHING THROUGH A FLOOR OR A WALL (case 1). From the dining room, looking up at
    /// the bed over its ceiling (its label 2.5 m away), E does not offer the bed; from the
    /// dressing room, looking down at bed_2 under its floor, nor that one; from the dining room,
    /// looking through the solid wall at x 42 at bed_2 2.5 m away, nor that one. On their own
    /// floors, both beds are E's.
    ///
    /// Seen red first (2026-10-21) with today's rule (5 m and the look cone): "from the dining
    /// room, E reaches the bed upstairs: Some("bed_1")".
    #[test]
    fn e_reaches_nothing_through_a_floor_or_a_wall() {
        let w = aboard();
        let got = w.e_at(w.at(36.5, 1.7, 34.0), "bed_1");
        assert_ne!(got, Some("bed_1"), "from the dining room, E reaches the bed upstairs: {got:?}");
        let got = w.e_at(w.at(43.5, 4.7, 36.0), "bed_2");
        assert_ne!(got, Some("bed_2"), "from the dressing room, E reaches the bed downstairs: {got:?}");
        let got = w.e_at(w.at(41.2, 1.7, 34.0), "bed_2");
        assert_ne!(got, Some("bed_2"), "from the dining room, E reaches the bed through the wall: {got:?}");
        assert_eq!(w.e_at(w.at(37.5, 4.7, 34.0), "bed_1"), Some("bed_1"), "upstairs, the bed upstairs");
        assert_eq!(w.e_at(w.at(45.5, 1.7, 34.0), "bed_2"), Some("bed_2"), "downstairs, the bed downstairs");
    }

    /// A home's storeys as `storey_floor` reads them, over the whole plane: the storeys
    /// `HomeStructure::storeys` derives from slabs whose tops are `tops` (any body will do: only
    /// its slabs and its 6 m height are read), and the floor of the one a point is on.
    fn storeys_of(tops: &[f32]) -> impl Fn(Vec3) -> Option<f32> {
        use crate::ship::home_structure::{storey_index, FloorSlab};
        let mut home = crate::ship::ship_structure::HomeDesign::built_in("homestead").expect("the homestead is built in").body;
        home.floors = tops
            .iter()
            .map(|&top| FloorSlab { a: (-50.0, -50.0), b: (50.0, 50.0), top, thickness: 0.25, material: SlabMaterial::Glass, label: None })
            .collect();
        let storeys = home.storeys();
        move |p: Vec3| Some(storeys[storey_index(&storeys, p.y)].floor)
    }

    /// E from `eye` (feet 1.7 m under it) looking straight at the first of `machines`.
    fn e_from(eye: Vec3, machines: &[(Vec3, f32)], storeys: impl Fn(Vec3) -> Option<f32>) -> Option<usize> {
        walk_up_target(eye, eye.y - 1.7, (machines[0].0 - eye).normalize(), machines.iter().copied(), &SightBlockers::default(), storeys)
    }

    /// THE STOREY OF A MACHINE IS WHERE IT STANDS, BY THE HOME'S OWN STOREYS. A 2.4 m tower's
    /// label is 2.8 m over its floor, past the 2.5 m a thing on one's own storey may rise: from
    /// beside it, E still reaches it; from the floor above, 0.2 m over its label, E does not. A
    /// unit hung under the ceiling (2.5 or 2.7 m up) is on the storey under that ceiling. In a
    /// home of 2.5 m storeys, a machine on the floor above (2.5 m up), seen through a glass floor
    /// or a stairwell, is not E's from the floor below, though a 3 m rule of thumb would put it
    /// on the same storey. Where no home says, the relay's own band at the foot.
    ///
    /// Seen red first (2026-10-21) four times: with today's rule (no storey at all) "from the
    /// floor above, E reaches the tower under it" (left: Some(0), right: None); with the storey
    /// tested at the LABEL (`moves::in_reach(eye, label, 5.0)`) "from beside it, E misses a 2.4 m
    /// tower" (left: None, right: Some(0)); with it tested at the foot by `moves::on_my_storey`
    /// (2.5 m over the feet at most) "from the floor under it, E misses a unit hung 2.7 m up"
    /// (left: None, right: Some(0)); and with the home's storeys ignored (the relay's band
    /// alone) "in a home of 2.5 m storeys, E reaches the floor above" (left: Some(0), right:
    /// None).
    #[test]
    fn the_storey_of_a_machine_is_where_it_stands() {
        let three = || storeys_of(&[3.0]);
        let tower = [(Vec3::new(2.0, 2.8, 0.0), 0.0)];
        assert_eq!(e_from(Vec3::new(0.0, 1.7, 0.0), &tower, three()), Some(0), "from beside it, E misses a 2.4 m tower");
        assert_eq!(e_from(Vec3::new(0.0, 4.7, 0.0), &tower, three()), None, "from the floor above, E reaches the tower under it");
        // A home of 2.5 m storeys: a desk on the floor above, its foot 2.5 m over the feet below.
        let above = [(Vec3::new(1.5, 3.4, 0.0), 2.5)];
        assert_eq!(e_from(Vec3::new(0.0, 1.7, 0.0), &above, storeys_of(&[2.5])), None, "in a home of 2.5 m storeys, E reaches the floor above");
        assert_eq!(e_from(Vec3::new(0.0, 4.2, 0.0), &above, storeys_of(&[2.5])), Some(0), "on that floor, E reaches it");
        // A machine hung under the ceiling: the greenhouse's air handlers stand 2.7 m up
        // (data/machines/home.ron), the plant room's 2.5 m, labels 3.3 m and 3.1 m up. From the
        // floor under them E reaches them; a unit standing on the floor above (3.0 m), it does not.
        for foot in [2.5, 2.7] {
            let unit = [(Vec3::new(1.5, foot + 0.64, 0.0), foot)];
            assert_eq!(e_from(Vec3::new(0.0, 1.7, 0.0), &unit, three()), Some(0), "from the floor under it, E misses a unit hung {foot} m up");
        }
        let upstairs = [(Vec3::new(1.5, 3.64, 0.0), 3.0)];
        assert_eq!(e_from(Vec3::new(0.0, 1.7, 0.0), &upstairs, three()), None, "a unit on the floor above");
        // Where no home says (a point in no zone), the relay's band at the foot: up to 2.5 m.
        let nowhere = || |_: Vec3| None::<f32>;
        assert_eq!(e_from(Vec3::new(0.0, 1.7, 0.0), &[(Vec3::new(1.5, 2.8, 0.0), 2.4)], nowhere()), Some(0), "no home: 2.4 m up");
        assert_eq!(e_from(Vec3::new(0.0, 1.7, 0.0), &upstairs, nowhere()), None, "no home: 3.0 m up");
        // Out of reach, or out of the cone, as ever.
        let none = SightBlockers::default();
        assert_eq!(walk_up_target(Vec3::new(-4.0, 1.7, 0.0), 0.0, Vec3::X, tower, &none, three()), None, "6 m away");
        assert_eq!(walk_up_target(Vec3::new(0.0, 1.7, 0.0), 0.0, Vec3::Z, tower, &none, three()), None, "looking away");
    }

    /// EVERY SHIPPED MACHINE IS E'S FROM ITS OWN FLOOR, AND SEEN FROM IT. For each machine of
    /// data/machines/home.ron, standing on the floor of the storey its foot is on (the
    /// homestead's own storeys), half a metre from it towards the middle of the home and looking
    /// at it, with nothing else about: E reaches it (`storey_floor` puts both on one storey), and
    /// no solid slab stands between that eye and its label (a label poking up into or through
    /// the floor over its own room would hide it from that room). A unit hung higher than its
    /// storey, or a label that pierces the slab over it, fails here and not on the operator's
    /// walk-through.
    ///
    /// Seen red first (2026-10-21) twice: in an earlier form, which read the storey by a fixed
    /// band over the feet, with that band at the relay's 2.5 m, "air_handler_g1 stands 2.70 m
    /// over its floor at 0.0, out of the walk-up's storey band"; and in this form, with the
    /// home's storeys ignored (the relay's band alone), "from its own floor at 0.0, E misses
    /// air_handler_g1 (its foot 2.70 m over it)".
    #[test]
    fn every_shipped_machine_is_es_from_its_own_floor_and_seen_from_it() {
        use crate::ship::home_structure::storey_index;
        let w = aboard();
        assert!(w.storeys.len() >= 2, "the homestead has an upper floor");
        let home = &w.ship.zones[w.ship.home_zone_index()].body;
        let middle = w.at(home.width * 0.5, 0.0, home.depth * 0.5);
        for (id, _room, label, foot) in &w.machines {
            let local = foot - w.origin.y;
            let floor = w.storeys[storey_index(&w.storeys, local)].floor;
            let toward = (Vec3::new(middle.x - label.x, 0.0, middle.z - label.z)).normalize_or_zero() * 0.5;
            let eye = Vec3::new(label.x + toward.x, w.origin.y + floor + 1.7, label.z + toward.z);
            let got = e_from(eye, &[(*label, *foot)], |p: Vec3| storey_floor(&w.ship, p));
            assert_eq!(got, Some(0), "from its own floor at {floor:.1}, E misses {id} (its foot {:.2} m over it)", local - floor);
            let slab = w.blockers.slabs.iter().any(|s| s.blocks(eye, *label));
            assert!(!slab, "{id}'s label at {:.2} m is behind a solid slab from its own floor at {floor:.1}", label.y - w.origin.y);
        }
    }

    /// THE SIGHT TEST IS WIRED IN: lib.rs builds the blockers with the solid slabs and the shut
    /// doors' heights and picks the walk-up target with `walk_up_target`, and the HUD tests every
    /// machine label and crew nameplate with `SightBlockers::blocked`, never the plan-only test.
    /// Without these the functions above are tested and never run.
    ///
    /// Seen red first (2026-10-21) before the wiring: "src/lib.rs never has
    /// crate::ship::sight::walk_up_target(".
    #[test]
    fn the_sight_test_is_wired_into_the_walk_up_and_the_hud() {
        let lib = include_str!("../lib.rs").replace("\r\n", "\n");
        for wired in ["crate::ship::sight::walk_up_target(", "crate::ship::sight::ship_solid_slabs(", "crate::ship::wall_collision::door_panel_segment(p.center + off"] {
            assert!(lib.contains(wired), "src/lib.rs never has {wired}");
        }
        let hud = include_str!("../gui/pages/hud.rs").replace("\r\n", "\n");
        assert_eq!(hud.matches("state.sight_blockers.blocked(").count(), 2, "the HUD tests the machine labels and the crew nameplates in 3-D");
        assert!(!hud.contains("wall_collision::sight_blocked("), "the HUD still tests a label in plan only");
    }
}
