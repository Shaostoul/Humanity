//! BUILD FRAMES (increment 5 of docs/design/ship-homes-and-logistics.md, "building only on your
//! own plot", 2026-10-05): the places a piece built in the shared world is kept in, and the one
//! rule for what lies inside one.
//!
//! A piece the server keeps (src/systems/construction/shared.rs) is stored and sent in a FRAME,
//! with its pose measured from the frame's corner rather than in ship metres. Two kinds:
//! - `plot:<id>`, a household's plot: the box one home stands in (`Plot` in the ship file);
//! - `zone:<id>`, one of the ship's shared spaces: the Commons, First Street.
//!
//! `site:<id>` is kept for building on planets later; nothing here makes one, and a frame id that
//! starts with it is not a frame of the ship.
//!
//! The frame is also what decides WHO may build: a plot's holder (or someone they gave a household
//! permit) on a plot, someone the server gave the `can_edit_ship` rank in a shared space. The
//! relay reads that from the frame's kind and id alone.
//!
//! ONE LIST ON BOTH SIDES. The relay builds its frames from the ship file
//! (`ShipStructure::ship_for_relay`), the game from the ship it assembled around its own home.
//! [`BuildFrames::of_ship`] gives the same frames from either, because it leaves out the assembled
//! ship's `home` zone, as `door_points` does: that zone is a local alias for this player's own
//! home, which already stands inside its plot, so the home is part of its plot's frame. A guest's
//! home put away is left out the same way. Corridors are no frame: nothing is built in one.
//!
//! WHAT LIES INSIDE ONE. A piece belongs to the frame whose floor holds its centre, and all of
//! its turned footprint must lie on that floor, give or take [`EDGE_SLACK_M`]. Height is not
//! bounded by the frame's box (a roof on walls stands at 3.0 to 3.2 m over a 3 m plot); only a
//! sanity range is ([`MIN_LOCAL_Y_M`], [`MAX_LOCAL_Y_M`]). The rule moved here from
//! engine/build_place.rs (`outside_own_plot`, increment 1a) so the relay and the game hold one
//! copy of it.
//!
//! Numbers are f32: the ship is about a kilometre long, where an f32 keeps a tenth of a
//! millimetre, ten times finer than the build rules' 1 mm tolerances. Today every plot and zone
//! corner sits on whole metres, so moving a grid-aligned pose between ship and frame metres is
//! exact.

use crate::ecs::components::Transform;
use crate::ship::ship_structure::{ShipStructure, HOME_ZONE_ID};
use crate::systems::construction::placement::world_aabb;
use glam::Vec3;
use serde::Serialize;

/// The start of a plot's frame id: `plot:p3`.
pub const PLOT_PREFIX: &str = "plot:";
/// The start of a shared space's frame id: `zone:commons`.
pub const ZONE_PREFIX: &str = "zone:";
/// The start kept for a build site on a planet, `site:<id>` (later). Never made here.
pub const SITE_PREFIX: &str = "site:";

/// How far a piece may overhang its frame's edge and still count as inside, metres: half the
/// thickest wall piece (a 0.3 m stone wall), so a wall laid ON a plot's line, the way the home's
/// own shell sits on it, is allowed. The 1 m build grid puts a wall's centre on the line, which
/// leaves half its thickness over it. Anything more, a foundation's metre say, is refused. Moved
/// from engine/build_place.rs's `PLOT_EDGE_EPS_M` (increment 1a) so both sides use one number.
pub const EDGE_SLACK_M: f32 = 0.15;

/// What f32 rounding may add to a turned piece's box, metres. A quarter turn is not exact in
/// f32 (sin and cos of 90 degrees come out a few parts in a hundred million off), so a stone
/// wall laid exactly on the line could otherwise poke over [`EDGE_SLACK_M`] by its last bit and be
/// refused. A millimetre, the build rules' own tolerance; no piece size comes near it.
const ROUNDING_M: f32 = 1.0e-3;

/// The lowest a piece may reach in a frame, metres from the frame's floor. A sanity range, not
/// the box's height: it stops a broken or hostile client parking pieces far below the deck.
pub const MIN_LOCAL_Y_M: f32 = -1.0;
/// The highest a piece may reach in a frame, metres from the frame's floor. Well over a roof on
/// walls on a foundation (3.4 m); a stop for absurd poses, not a building height limit.
pub const MAX_LOCAL_Y_M: f32 = 40.0;

/// Which kind of place a frame is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FrameKind {
    /// A household's plot: its holder builds there.
    Plot,
    /// One of the ship's shared spaces: built only with the server's `can_edit_ship` rank.
    Zone,
}

/// The kind and the place id of a frame id (`"plot:p3"` is a plot `p3`), or None for anything
/// that is not a frame of the ship: a planet site (`site:`), an empty id after the prefix, or a
/// string with no known prefix. The relay refuses such a frame as `bad_frame` before looking it
/// up.
pub fn parse_frame_id(id: &str) -> Option<(FrameKind, &str)> {
    let (kind, place) = if let Some(rest) = id.strip_prefix(PLOT_PREFIX) {
        (FrameKind::Plot, rest)
    } else if let Some(rest) = id.strip_prefix(ZONE_PREFIX) {
        (FrameKind::Zone, rest)
    } else {
        return None;
    };
    (!place.is_empty()).then_some((kind, place))
}

/// The frame id of plot `plot_id`: `plot:p3`.
pub fn plot_frame_id(plot_id: &str) -> String {
    format!("{PLOT_PREFIX}{plot_id}")
}

/// The frame id of the shared space `zone_id`: `zone:commons`.
pub fn zone_frame_id(zone_id: &str) -> String {
    format!("{ZONE_PREFIX}{zone_id}")
}

/// One place pieces are kept in: a plot or a shared space, its box in ship metres.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BuildFrame {
    /// `"plot:p3"` or `"zone:commons"`: a plot and a zone may share an id in the ship file, the
    /// prefix keeps them apart.
    pub id: String,
    pub kind: FrameKind,
    /// The box's min corner, ship metres (y is the deck).
    pub origin: Vec3,
    /// The box: width (x), height (y), depth (z), metres.
    pub size: Vec3,
}

impl BuildFrame {
    /// The place's own id, without the prefix: `p3` for `plot:p3`.
    pub fn place_id(&self) -> &str {
        parse_frame_id(&self.id).map_or("", |(_, place)| place)
    }

    /// The plot's id when this frame is a plot (`p3`), else None.
    pub fn plot_id(&self) -> Option<&str> {
        (self.kind == FrameKind::Plot).then(|| self.place_id())
    }

    /// `ship` (a pose in ship metres) as measured from this frame's corner. Turn and size are
    /// the same in both.
    pub fn to_local(&self, ship: &Transform) -> Transform {
        Transform { position: ship.position - self.origin, rotation: ship.rotation, scale: ship.scale }
    }

    /// `local` (a pose measured from this frame's corner) in ship metres.
    pub fn to_ship(&self, local: &Transform) -> Transform {
        Transform { position: local.position + self.origin, rotation: local.rotation, scale: local.scale }
    }

    /// Does this frame's floor hold the ship point `p`: is it over the box, on x and z, edges
    /// included? Height is not looked at (every frame is on the one deck today; stacked decks,
    /// increment 7, will add the storey).
    pub fn holds_point(&self, p: Vec3) -> bool {
        let (lo, hi) = (self.origin, self.origin + self.size);
        p.x >= lo.x && p.x <= hi.x && p.z >= lo.z && p.z <= hi.z
    }

    /// How far the ship point `p` is from this frame's floor, metres along the deck (x and z): 0
    /// over it. What the relay judges "in view" by (increment 4's 250 and 300 m,
    /// data/ship/shared_world.ron).
    pub fn floor_distance(&self, p: Vec3) -> f32 {
        let (lo, hi) = (self.origin, self.origin + self.size);
        let dx = (lo.x - p.x).max(p.x - hi.x).max(0.0);
        let dz = (lo.z - p.z).max(p.z - hi.z).max(0.0);
        (dx * dx + dz * dz).sqrt()
    }

    /// Does all of `local`'s turned footprint (`placement::world_aabb`, x and z) lie on this
    /// frame's floor, give or take [`EDGE_SLACK_M`]? `local` is measured from the frame's corner,
    /// so the floor is 0 to `size`. Only x and z are looked at; a NaN in either, or in the turn,
    /// reads as outside.
    pub fn footprint_inside(&self, local: &Transform) -> bool {
        let (a, b) = world_aabb(local);
        let s = EDGE_SLACK_M + ROUNDING_M;
        a.x >= -s && a.z >= -s && b.x <= self.size.x + s && b.z <= self.size.z + s
    }

    /// Does `local`'s box stay within the sanity range of heights ([`MIN_LOCAL_Y_M`] to
    /// [`MAX_LOCAL_Y_M`] over the frame's floor)? Only the heights are looked at (the footprint
    /// is `footprint_inside`'s); a NaN in one reads as outside.
    pub fn height_inside(&self, local: &Transform) -> bool {
        let (a, b) = world_aabb(local);
        a.y >= MIN_LOCAL_Y_M && b.y <= MAX_LOCAL_Y_M
    }
}

/// Every frame of one ship, in a fixed order: the shared spaces in the ship file's order, then
/// the plots in theirs (the order `door_points` lists its places in).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct BuildFrames {
    pub frames: Vec<BuildFrame>,
}

impl BuildFrames {
    /// The frames of `ship`: every zone but `home` (see the top of this file), then every plot.
    /// The same list from the relay's ship file and from any game's assembled ship. Pure.
    pub fn of_ship(ship: &ShipStructure) -> BuildFrames {
        let zones = ship.zones.iter().filter(|z| z.id != HOME_ZONE_ID).map(|z| BuildFrame {
            id: zone_frame_id(&z.id),
            kind: FrameKind::Zone,
            origin: z.origin_vec(),
            size: Vec3::new(z.body.width, z.body.height, z.body.depth),
        });
        let plots = ship.plots.iter().map(|p| BuildFrame {
            id: plot_frame_id(&p.id),
            kind: FrameKind::Plot,
            origin: Vec3::new(p.origin.0, p.origin.1, p.origin.2),
            size: Vec3::new(p.size.0, p.size.1, p.size.2),
        });
        BuildFrames { frames: zones.chain(plots).collect() }
    }

    /// A frame by id (`"plot:p3"`).
    pub fn get(&self, id: &str) -> Option<&BuildFrame> {
        self.frames.iter().find(|f| f.id == id)
    }

    /// The frame whose floor holds the ship point `p` (a piece's position, which is the middle
    /// of its footprint), or None: a corridor, the gap between two plots, off the ship. The
    /// ship file's validation refuses overlapping plots and zones, so at most one frame holds a
    /// point, unless two frames share an edge and `p` lies on it, when the first in the list
    /// does.
    pub fn frame_at(&self, p: Vec3) -> Option<&BuildFrame> {
        self.frames.iter().find(|f| f.holds_point(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::systems::construction::placement::placement_pose;
    use crate::systems::construction::BlueprintRegistry;
    use glam::Quat;

    fn data_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    fn assembled(plot: &str) -> ShipStructure {
        ShipStructure::load_and_assemble_shipped(&data_dir(), Some(plot)).expect("the shipped ship assembles")
    }

    fn shipped_blueprints() -> BlueprintRegistry {
        BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap()
    }

    /// THE SHIPPED SHIP'S FRAMES, the same from every side. The relay's ship file, the game's
    /// ship assembled at p1 and at p2, and a guest's ship (the home put away) all give the same
    /// fourteen frames: the Commons and First Street, then the twelve plots, with the ship
    /// file's corners and sizes, and never the assembled ship's `home` zone, which is the
    /// player's own home standing inside its plot. `frame_at` finds the plot, the street or the
    /// Commons a point stands over, and nothing in a corridor or between two plots;
    /// `parse_frame_id` knows a plot from a zone and refuses a planet site; moving a pose to a
    /// frame's metres and back gives the same bits; and the floor distances from p1's door are
    /// the ones the relay's view radius (250 m) will judge by: p4 at 256.5 m is out of it.
    /// Seen red 2026-10-05 with the `home` zone not filtered out of `of_ship`: "assembled at p1:
    /// the game's frames are the relay's", left: `BuildFrames { frames: [BuildFrame { id:
    /// "zone:home", kind: Zone, origin: Vec3(0.0, 0.0, 0.0), size: Vec3(55.0, 3.0, 89.0) },
    /// BuildFrame { id: "zone:commons", ...`. The home fills p1's box and comes first, so
    /// `frame_at` would have taken a piece in your own home for one in the ship's shared spaces.
    #[test]
    fn frames_of_the_shipped_ship() {
        let relay = ShipStructure::ship_for_relay(&data_dir()).expect("the relay's ship file loads");
        let frames = BuildFrames::of_ship(&relay);
        let ids: Vec<&str> = frames.frames.iter().map(|f| f.id.as_str()).collect();
        let mut want = vec!["zone:commons".to_string(), "zone:street-1".to_string()];
        want.extend((1..=12).map(|n| format!("plot:p{n}")));
        assert_eq!(ids, want, "the relay's frames");
        for (game, ship) in [("assembled at p1", assembled("p1")), ("assembled at p2", assembled("p2"))] {
            assert_eq!(BuildFrames::of_ship(&ship), frames, "{game}: the game's frames are the relay's");
        }
        let guest = assembled("p1").put_home_away().expect("the home can be put away");
        assert_eq!(BuildFrames::of_ship(&guest), frames, "a guest's frames are the relay's");
        assert!(frames.get("zone:home").is_none());

        // The ship file's numbers (data/blueprints/ship_structure.ron).
        let commons = frames.get("zone:commons").unwrap();
        assert_eq!((commons.kind, commons.origin, commons.size), (FrameKind::Zone, Vec3::new(65.0, 0.0, 20.0), Vec3::new(34.0, 8.0, 55.0)));
        let street = frames.get("zone:street-1").unwrap();
        assert_eq!((street.origin, street.size), (Vec3::new(65.0, 0.0, 85.0), Vec3::new(10.0, 4.0, 1100.0)));
        let p3 = frames.get("plot:p3").unwrap();
        assert_eq!((p3.kind, p3.origin, p3.size), (FrameKind::Plot, Vec3::new(0.0, 0.0, 198.0), Vec3::new(55.0, 3.0, 89.0)));
        assert_eq!((p3.place_id(), p3.plot_id(), commons.plot_id()), ("p3", Some("p3"), None));

        // Where a point stands.
        let at = |x: f32, z: f32| frames.frame_at(Vec3::new(x, 0.0, z)).map(|f| f.id.as_str());
        assert_eq!(at(30.0, 240.0), Some("plot:p3"));
        assert_eq!(at(55.0, 240.0), Some("plot:p3"), "a wall's centre on the plot's line is the plot's");
        assert_eq!(at(80.0, 40.0), Some("zone:commons"));
        assert_eq!(at(70.0, 500.0), Some("zone:street-1"));
        assert_eq!(at(60.0, 40.0), None, "p1's door corridor is no frame");
        assert_eq!(at(30.0, 94.0), None, "nor the gap between p1 and p2");
        assert_eq!(at(-5.0, 40.0), None, "nor off the ship");

        // Frame ids.
        assert_eq!(parse_frame_id("plot:p3"), Some((FrameKind::Plot, "p3")));
        assert_eq!(parse_frame_id("zone:commons"), Some((FrameKind::Zone, "commons")));
        for not_a_frame in ["site:moon-1", "p3", "plot:", "zone:", "", "home"] {
            assert_eq!(parse_frame_id(not_a_frame), None, "{not_a_frame:?}");
        }
        assert_eq!((plot_frame_id("p3"), zone_frame_id("commons")), ("plot:p3".to_string(), "zone:commons".to_string()));

        // Ship metres to frame metres and back, exactly, for the poses the placer makes.
        let reg = shipped_blueprints();
        let world = hecs::World::new();
        for (id, x, z, turns) in [("wood_foundation", 30.0, 240.0, 0u8), ("wood_wall", 55.0, 260.0, 1), ("roof", 12.0, 285.0, 3)] {
            let ship = placement_pose(reg.get(id).unwrap(), Vec3::new(x, 0.0, z), turns, &world, &reg, None);
            let local = p3.to_local(&ship);
            assert_eq!(local.position, Vec3::new(x, 0.0, z - 198.0), "{id}: measured from p3's corner");
            let back = p3.to_ship(&local);
            let bits = |t: &Transform| (t.position.to_array().map(f32::to_bits), t.rotation.to_array().map(f32::to_bits), t.scale.to_array().map(f32::to_bits));
            assert_eq!(bits(&back), bits(&ship), "{id}: there and back, to the bit");
        }
        let lifted = Transform { position: Vec3::new(30.0, 3.2, 240.0), rotation: Quat::IDENTITY, scale: Vec3::ONE };
        let back = p3.to_ship(&p3.to_local(&lifted)).position;
        assert_eq!(back.to_array().map(f32::to_bits), lifted.position.to_array().map(f32::to_bits), "a roof's height survives too");

        // How far each frame's floor is from p1's door (door_points: (53.5, 1.7, 40.5)).
        let door = Vec3::new(53.5, 1.7, 40.5);
        let dist = |id: &str| frames.get(id).unwrap().floor_distance(door);
        assert_eq!(dist("plot:p1"), 0.0);
        assert_eq!(dist("plot:p2"), 58.5);
        assert_eq!(dist("plot:p3"), 157.5);
        assert_eq!(dist("plot:p4"), 256.5, "out of a 250 m view");
        assert_eq!(dist("zone:commons"), 11.5);
        assert!((dist("zone:street-1") - (11.5f32 * 11.5 + 44.5 * 44.5).sqrt()).abs() < 1e-4);
    }

    /// A FOOTPRINT OVER THE PLOT LINE IS OUTSIDE (the plot cases of engine/build_place.rs's
    /// `a_piece_goes_aboard_only_inside_your_own_plot`, moved here with the rule). On p1
    /// (0..55 x 0..89), with real blueprints placed the way the ghost places them: a foundation
    /// in the yard and one flush in the far corner are inside; a 4 x 4 m foundation centred at
    /// (54, 40) is not, because it spans x 52..56 across the home's door (the critic's case
    /// against a centre-only test); a wall on the east line running north-south is inside and
    /// running east-west it reaches x 57; every wall laid ON a line (stone 0.3 m, wood 0.2,
    /// metal 0.15), on all four lines, is inside, the case the slack is for; a chest turned to
    /// run east-west on the east line pokes 30 cm over and is not; nor is the Commons or p2 for
    /// p1. On p2 (corner z 99) the same pieces measured from p2's corner follow its box. Heights:
    /// a roof on walls is inside, a piece below the deck by more than a metre or over 40 m is not.
    /// Seen red 2026-10-05 with the slack doubled to 0.30 m: "a chest pokes 30 cm over the line".
    #[test]
    fn a_footprint_over_the_plot_line_is_outside() {
        let frames = BuildFrames::of_ship(&ShipStructure::ship_for_relay(&data_dir()).unwrap());
        let (p1, p2) = (frames.get("plot:p1").unwrap(), frames.get("plot:p2").unwrap());
        let reg = shipped_blueprints();
        let world = hecs::World::new();
        let pose = |id: &str, x: f32, z: f32, turns: u8| {
            placement_pose(reg.get(id).unwrap_or_else(|| panic!("{id} in basic.ron")), Vec3::new(x, 0.0, z), turns, &world, &reg, None)
        };
        let inside = |f: &BuildFrame, id: &str, x: f32, z: f32, turns: u8| f.footprint_inside(&f.to_local(&pose(id, x, z, turns)));
        assert!(inside(p1, "wood_foundation", 30.0, 20.0, 0), "a foundation in the yard");
        assert!(inside(p1, "wood_foundation", 53.0, 87.0, 0), "flush in the far corner");
        assert!(!inside(p1, "wood_foundation", 54.0, 40.0, 0), "centred inside, but it covers the door and a metre of the corridor");
        assert!(inside(p1, "wood_wall", 55.0, 30.0, 1), "a wall on the east line, running north-south");
        assert!(!inside(p1, "wood_wall", 55.0, 40.0, 0), "running east-west from the line it reaches x 57");
        // Every wall laid on each of the four lines: half its thickness over, never more than the slack.
        for wall in ["stone_wall", "wood_wall", "metal_wall"] {
            for (x, z, turns) in [(55.0, 30.0, 1u8), (55.0, 30.0, 3), (0.0, 30.0, 1), (0.0, 30.0, 3), (20.0, 0.0, 0), (20.0, 0.0, 2), (20.0, 89.0, 0), (20.0, 89.0, 2)] {
                assert!(inside(p1, wall, x, z, turns), "a {wall} laid on p1's line at ({x}, {z}), {turns} turns");
            }
        }
        // A chest (1.0 x 0.6) turned so its 0.6 m side runs east-west, centred on the east line: 0.3 m over.
        assert!(!inside(p1, "storage_chest", 55.0, 30.0, 1), "a chest pokes 30 cm over the line");
        assert!(!inside(p1, "wood_foundation", 80.0, 40.0, 0), "the Commons is not p1");
        assert!(!inside(p1, "wood_foundation", 30.0, 140.0, 0), "p2 is not p1");
        assert!(inside(p2, "wood_foundation", 30.0, 140.0, 0), "p2's yard, measured from p2's corner");
        assert!(!inside(p2, "wood_foundation", 30.0, 20.0, 0), "and p1 is not p2");
        assert!(inside(p2, "wood_wall", 20.0, 99.0, 0), "a wall on p2's north line");
        assert!(!inside(p2, "wood_foundation", 20.0, 98.0, 0), "a foundation a metre over it");

        // Heights: no plot box bounds them, a sanity range does.
        let roof = p1.to_local(&Transform { position: Vec3::new(20.0, 3.2, 20.0), ..pose("roof", 20.0, 20.0, 0) });
        assert!(p1.height_inside(&roof), "a roof on walls on a foundation, over the 3 m plot box");
        let sunk = Transform { position: Vec3::new(20.0, -1.5, 20.0), ..roof.clone() };
        let high = Transform { position: Vec3::new(20.0, 39.9, 20.0), ..roof.clone() };
        assert!(!p1.height_inside(&sunk) && !p1.height_inside(&high), "below the deck, or reaching past 40 m");
        // A NaN reads as outside, never inside: in x for the footprint, in y for the height (each
        // looks only at its own axes; the relay's `validate_pose` refuses any NaN before either).
        let nan_x = Transform { position: Vec3::new(f32::NAN, 0.0, 20.0), ..roof.clone() };
        let nan_y = Transform { position: Vec3::new(20.0, f32::NAN, 20.0), ..roof };
        assert!(!p1.footprint_inside(&nan_x), "a NaN in x is outside the footprint");
        assert!(!p1.height_inside(&nan_y), "a NaN in y is outside the heights");
    }
}
