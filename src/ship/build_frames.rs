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
//! WHAT LIES INSIDE ONE. A piece belongs to the frame whose box holds its position (the middle
//! of its footprint, at its base: [`box_holds`], over the floor plan and from a metre under the
//! floor up to the box's top, ship homes increment 7), and all of its turned footprint must lie on
//! that floor, give or take [`EDGE_SLACK_M`]. What the piece itself reaches up to is not bounded
//! by the box (a wall standing on a 6 m plot's upper floor at 3.0 m reaches 6.2 with its roof);
//! only a sanity range is ([`MIN_LOCAL_Y_M`], [`MAX_LOCAL_Y_M`]). The rule moved here from
//! engine/build_place.rs (`outside_own_plot`, increment 1a) so the relay and the game hold one
//! copy of it.
//!
//! Numbers are f32: the ship is about a kilometre long, where an f32 keeps a tenth of a
//! millimetre, ten times finer than the build rules' 1 mm tolerances. Today every plot and zone
//! corner sits on whole metres, so moving a grid-aligned pose between ship and frame metres is
//! exact.

use humanity_core::ecs::components::Transform;
use crate::ship::ship_structure::{ShipStructure, HOME_ZONE_ID};
use humanity_core::ecs::components::world_aabb;
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

/// THE BOX RULE (ship homes increment 7): does the box with min corner `origin` and `size` (a plot's
/// or a zone's, ship metres) hold the ship point `p`? Over its floor plan, x and z, edges included,
/// as before storeys, AND from [`MIN_LOCAL_Y_M`] under its floor up to its top, both included. A
/// figure over a plot but above its top is not on it: when plots stack (a cabin block or an
/// apartment over another home, the gate increment 7 opens), the point is on the plot whose box it
/// is in, never on every plot under it. Every point inside a 3 m box keeps the answer it had, and
/// so does every point inside a homestead plot, now 6 m tall. One rule for a build frame
/// (`BuildFrame::holds_point`) and the relay's plots (relay/handlers/ship_world.rs `plot_at`).
pub fn box_holds(origin: Vec3, size: Vec3, p: Vec3) -> bool {
    let hi = origin + size;
    p.x >= origin.x && p.x <= hi.x && p.z >= origin.z && p.z <= hi.z && p.y >= origin.y + MIN_LOCAL_Y_M && p.y <= hi.y
}

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

    /// Does this frame hold the ship point `p`: over its floor plan (x and z, edges included) and
    /// between a metre under its floor and its top ([`box_holds`], ship homes increment 7)?
    pub fn holds_point(&self, p: Vec3) -> bool {
        box_holds(self.origin, self.size, p)
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

    /// Does all of `local`'s turned footprint (`ecs::components::world_aabb`, x and z) lie on this
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

    /// The frame whose box holds the ship point `p` (a piece's position, which is the middle
    /// of its footprint at its base; [`box_holds`]), or None: a corridor, the gap between two
    /// plots, off the ship, over a frame's top. The
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

    fn data_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("data")
    }

    /// A FIGURE ABOVE A PLOT'S TOP IS NOT ON IT (ship homes increment 7): a frame holds a point
    /// over its floor plan only from a metre under its floor up to its top. Over p1, a 6 m
    /// homestead plot, a point at 6.5 m is on no frame, one at 5.9 m (a person jumping on its upper
    /// floor) and one at its floor are p1's; over c1, a 3 m cabin plot, 3.5 m is on none and 2.9 m
    /// is c1's; 1.5 m under a floor is on none. Every point inside the boxes keeps its answer.
    ///
    /// Seen red 2026-10-20 with the rule before storeys (across the floor only): "a point 6.5 m
    /// over p1 is on Some(\"plot:p1\")".
    #[test]
    fn a_figure_above_a_plots_top_is_not_on_it() {
        let frames = BuildFrames::of_ship(&ShipStructure::ship_for_relay(&data_dir()).unwrap());
        let at = |x: f32, y: f32, z: f32| frames.frame_at(Vec3::new(x, y, z)).map(|f| f.id.clone());
        assert_eq!(frames.get("plot:p1").unwrap().size.y, 6.0, "p1 is a two-storey plot");
        let v = at(30.0, 6.5, 40.0);
        assert_eq!(v, None, "a point 6.5 m over p1 is on {v:?}");
        assert_eq!(at(30.0, 5.9, 40.0).as_deref(), Some("plot:p1"), "jumping on p1's upper floor");
        assert_eq!(at(30.0, 6.0, 40.0).as_deref(), Some("plot:p1"), "its top is its own");
        assert_eq!(at(30.0, 0.0, 40.0).as_deref(), Some("plot:p1"), "its floor");
        assert_eq!(at(30.0, -1.5, 40.0), None, "under its floor");
        assert_eq!(at(82.0, 3.5, 89.0), None, "over a cabin's 3 m top");
        assert_eq!(at(82.0, 2.9, 89.0).as_deref(), Some("plot:c1"), "inside a cabin");
        assert!(box_holds(Vec3::ZERO, Vec3::new(55.0, 6.0, 89.0), Vec3::new(55.0, 6.0, 89.0)), "the box's far corner is inside it");
    }

}
