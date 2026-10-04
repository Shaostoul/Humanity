//! Transit links (increment 4 of docs/design/ship-homes-and-logistics.md, "getting around at
//! ship scale"): the jumps aboard that are not walking. Today that is a teleporter and the
//! teleporter it is paired with (`PlacedStructure::pair`, by id); lifts and rail are later rungs
//! of the same idea ("Registered transit jumps (lifts, rail, rank teleporters) carry a link id
//! that the relay checks against the ship file", section 5.10).
//!
//! ONE function finds the link a person stands in (`ShipStructure::transit_link_at`), and both
//! sides use it: the game to make the jump (lib.rs, the teleporter pads) and to say so in its next
//! position update (`MoveDecl::Link`, src/ship/moves.rs), and the relay to check a link in a
//! shared zone against its own copy of the ship file (src/relay/handlers/move_check.rs). A link
//! in the player's own home is checked by where its ends are (both on the player's own plot),
//! because the relay has no copy of anyone's home.
//!
//! Before this increment a pair was the partner's INDEX in the list, which is how the shipped
//! homestead's west teleporter (index 3) came to jump people onto its ladder (index 5): the east
//! teleporter is index 6. Ids cannot drift that way (home_structure.rs `settle_structures`).

use crate::ship::moves::MoveDecl;
use crate::ship::ship_structure::ShipStructure;
use crate::ship::structure::{in_footprint, structure_type, StructureKind};
use glam::Vec3;

/// One way through a transit link: from the pad `from` to the pad `to`, both in zone `zone`
/// (ship metres; `*_at` is the pad's floor point, its placed position plus the zone's origin).
#[derive(Debug, Clone, PartialEq)]
pub struct TransitLink {
    pub zone: String,
    pub from: String,
    pub to: String,
    pub from_at: Vec3,
    pub to_at: Vec3,
    /// The entry pad's yaw, radians (its footprint turns with it).
    pub from_yaw: f32,
    /// How far from a pad's middle someone standing in it can be, metres across the floor: half
    /// the pad's footprint diagonal, with the 5 cm `in_footprint` allows.
    pub reach_m: f32,
}

impl TransitLink {
    /// What the game's next position update says about this jump (`"moved"`).
    pub fn declaration(&self) -> MoveDecl {
        MoveDecl::Link { zone: self.zone.clone(), from: self.from.clone(), to: self.to.clone(), from_at: self.from_at, to_at: self.to_at }
    }
}

impl ShipStructure {
    /// Every transit link of the ship, each way: every teleporter whose `pair` names another
    /// teleporter of the same zone body. Not the home while it is put away (a guest's home,
    /// increment 2): nobody stands there.
    pub fn transit_links(&self) -> Vec<TransitLink> {
        let mut out = Vec::new();
        for (zi, zone) in self.zones.iter().enumerate() {
            if self.is_away_home(zi) {
                continue;
            }
            let o = zone.origin_vec();
            let body = &zone.body;
            for (i, ps) in body.structures.iter().enumerate() {
                let Some(ty) = structure_type(&ps.type_id) else { continue };
                if ty.kind != StructureKind::Teleporter {
                    continue;
                }
                let Some(j) = body.pair_index(i) else { continue };
                let partner = &body.structures[j];
                if partner.type_id != ps.type_id || j == i {
                    continue;
                }
                let at = |p: (f32, f32, f32)| Vec3::new(p.0, p.1, p.2) + o;
                out.push(TransitLink {
                    zone: zone.id.clone(),
                    from: ps.id.clone(),
                    to: partner.id.clone(),
                    from_at: at(ps.pos),
                    to_at: at(partner.pos),
                    from_yaw: ps.rot_deg.to_radians(),
                    reach_m: (ty.size.0 * 0.5 + 0.05).hypot(ty.size.2 * 0.5 + 0.05),
                });
            }
        }
        out
    }

    /// The link whose entry pad the point stands in (across the floor), if any: where a person
    /// walking there is jumped to.
    pub fn transit_link_at(&self, p: Vec3) -> Option<TransitLink> {
        self.transit_links().into_iter().find(|l| {
            structure_type("teleporter").is_some_and(|ty| in_footprint(ty, (l.from_at.x, l.from_at.y, l.from_at.z), l.from_yaw, p.x, p.z))
        })
    }

    /// The link of zone `zone` from pad `from` to pad `to`, if this ship has it: what the relay
    /// checks a declared jump in a shared zone against.
    pub fn transit_link(&self, zone: &str, from: &str, to: &str) -> Option<TransitLink> {
        self.transit_links().into_iter().find(|l| l.zone == zone && l.from == from && l.to == to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship::home_structure::{HomeStructure, PlacedStructure};

    fn data() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    /// The shipped homestead, its structures read through the same loader every home goes
    /// through.
    fn shipped_home() -> HomeStructure {
        crate::ship::ship_structure::HomeDesign::built_in("homestead").expect("the shipped homestead").body
    }

    /// THE WEST TELEPORTER JUMPS TO THE EAST ONE, NEVER TO THE LADDER: in the shipped homestead,
    /// stepping into either teleporter lands on the other, and the ladder is no one's partner.
    /// Seen red 2026-10-04 on the shipped file with its pairs put back to list indexes and step 3
    /// of `settle_structures` (drop a pair of the wrong type) taken out: "one link each way:
    /// [TransitLink { zone: \"home\", from: \"teleporter-2\", to: \"teleporter-1\", ... }]" (the west
    /// teleporter's pair was the ladder, so it had no way through at all).
    #[test]
    fn the_west_teleporter_jumps_to_the_east_one() {
        let ship = ShipStructure::load_and_assemble_shipped(&data(), None).expect("the shipped ship assembles");
        let links: Vec<TransitLink> = ship.transit_links().into_iter().filter(|l| l.zone == "home").collect();
        assert_eq!(links.len(), 2, "one link each way: {links:?}");
        let home = &ship.zones[ship.home_zone_index()].body;
        for l in &links {
            let to = home.structures.iter().find(|s| s.id == l.to).expect("the partner is in the home");
            assert_eq!(to.type_id, "teleporter", "{} jumps to {}", l.from, l.to);
        }
        let west = links.iter().find(|l| l.from == "teleporter-1").expect("the west teleporter has a link");
        assert_eq!(west.to, "teleporter-2");
        // Standing in the west pad finds that link, and it lands on the east pad.
        let found = ship.transit_link_at(west.from_at + Vec3::new(0.3, 1.7, 0.1)).expect("the west pad is found");
        assert_eq!(found, *west);
        assert!((found.to_at - Vec3::new(31.0, 0.0, 80.0)).length() < 1e-4, "it lands on the east pad: {:?}", found.to_at);
        assert!(ship.transit_link_at(west.from_at + Vec3::new(3.0, 0.0, 0.0)).is_none(), "a step beside the pad is not in it");
    }

    /// A HOME SAVED WITH LIST-INDEX PAIRS STILL LOADS, and comes out as today's: the shipped
    /// homestead exactly as the previous code wrote it (tests/fixtures/homes/
    /// homestead_pairs_by_index.ron, copied unedited from data/homes/shipped/homestead.ron at
    /// 67a47bc65, v0.1456.0) reads with every piece given the id the editor would give it, the
    /// west teleporter's pair to the LADDER dropped, and the west teleporter paired with the east
    /// one, which named it: the same pieces and pairs as the shipped file now carries.
    /// Seen red 2026-10-04 with step 2 of `settle_structures` (an index becomes the id of the piece
    /// there) taken out: "the old home settles to the shipped pieces and pairs" (every pair of the
    /// old home came out None).
    #[test]
    fn a_home_saved_with_index_pairs_loads() {
        let old: HomeStructure = ron::from_str::<crate::ship::ship_structure::HomeDesign>(include_str!("../../tests/fixtures/homes/homestead_pairs_by_index.ron"))
            .expect("the home the previous code wrote still parses")
            .body;
        let now = shipped_home();
        let pairs = |h: &HomeStructure| -> Vec<(String, String, Option<String>)> { h.structures.iter().map(|s| (s.id.clone(), s.type_id.clone(), s.pair.clone())).collect() };
        assert_eq!(pairs(&old), pairs(&now), "the old home settles to the shipped pieces and pairs");
        let ladder = old.structures.iter().find(|s| s.type_id == "ladder").expect("the ladder");
        assert!(old.structures.iter().all(|s| s.pair.as_deref() != Some(ladder.id.as_str())), "nobody is paired with the ladder");
        // And it saves in the new form: ids, pairs by id.
        let text = ron::ser::to_string(&old).expect("it serializes");
        assert!(text.contains("pair:Some(\"teleporter-2\")") || text.contains("pair:Some(\"teleporter-1\")"), "pairs are written as ids: {}", &text[..text.len().min(200)]);
    }

    /// Ids are stable: removing a piece leaves every other pair as it was (with list indexes, a
    /// removal shifted every pair after it, and four removers each had to fix that up), and the
    /// id a new piece gets is never one in use. Seen red 2026-10-04 with `remove_structure` keeping
    /// the pairs that named the piece it took out: "the east pad is left unpaired".
    #[test]
    fn removing_a_piece_leaves_the_other_pairs_alone() {
        let mut h = shipped_home();
        let before = h.structures.iter().find(|s| s.id == "teleporter-1").and_then(|s| s.pair.clone());
        let stairs = h.structures.iter().position(|s| s.type_id == "stairs").expect("stairs");
        h.remove_structure(stairs);
        let after = h.structures.iter().find(|s| s.id == "teleporter-1").and_then(|s| s.pair.clone());
        assert_eq!(before, after, "the teleporter pair survived taking the stairs out");
        let west = h.structures.iter().position(|s| s.id == "teleporter-1").expect("west");
        h.remove_structure(west);
        assert!(h.structures.iter().all(|s| s.pair.as_deref() != Some("teleporter-1")), "the east pad is left unpaired");
        let id = h.next_structure_id("teleporter");
        assert!(h.structures.iter().all(|s| s.id != id), "{id} is new");
        h.structures.push(PlacedStructure { id: id.clone(), type_id: "teleporter".into(), pos: (1.0, 0.0, 1.0), rot_deg: 0.0, pair: None });
        assert_ne!(h.next_structure_id("teleporter"), id);
    }
}
