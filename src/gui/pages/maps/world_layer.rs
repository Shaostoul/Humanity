//! The Maps page's WORLD LAYER, the page side (Maps increment 1; moved out of
//! `you_layer.rs` in increment 2, which taught it the star sky): whether the
//! engine renders the real world under the page's symbols this frame, the
//! whole world or the star sky alone, and if neither, why. One function for
//! the engine (`engine/map_view.rs`, which renders it) and the page (which
//! draws it and says why it is missing), so they never disagree.

use egui::{Color32, Pos2, Rect};

use crate::gui::state_types::MapsState;

use super::camera::{NEAR_SPACE_MAX_M, WORLD_LAYER_MAX_M};
use super::MapView;

/// Whether the world layer is drawn, and if not, why not. One function for
/// the engine (which renders it) and the page (which draws it and says why
/// it is missing), so they never disagree.
///
/// Maps increment 2: `Draw` reaches out to the edge of near space (2 million
/// km), where Earth and the Moon are real globes behind the ship's icon, and
/// wherever the whole world cannot be drawn but is loaded, the world layer
/// still draws the real STAR SKY (`draws_sky`): the solar-system band, the
/// Diagram, off Live, away from the ship.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldLayerWant {
    Draw,
    /// Not the Solar System view.
    OtherView,
    /// Nothing says where you are (a snapshot, or before the engine has run).
    NoFeed,
    /// The 3D world is not loaded yet.
    NotLoaded,
    /// You are not aboard your ship.
    NotAboard,
    /// The clock is off Live.
    NotLive,
    /// The camera is beyond near space (the solar-system band).
    TooFar,
    /// The Diagram is on: nothing is where the world has it.
    Diagram,
}

/// What the world layer drew under this frame's symbols.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorldDrawn {
    /// Nothing: the page paints its own space.
    Nothing,
    /// The real star sky (and Milky Way) from the map's direction.
    Sky,
    /// Everything: the ship, Earth and the Moon for real, the Sun, the sky.
    Everything,
}

impl WorldLayerWant {
    /// Whether the star sky is drawn when the whole world is not: the world
    /// is loaded, and only the ship's band or the clock or the Diagram keeps
    /// the rest out.
    pub fn draws_sky(self) -> bool {
        matches!(self, WorldLayerWant::NotAboard | WorldLayerWant::NotLive | WorldLayerWant::TooFar | WorldLayerWant::Diagram)
    }

    /// What the page says when the world layer is missing at ship scale.
    pub fn reason(self) -> Option<&'static str> {
        match self {
            WorldLayerWant::NotLoaded => Some("Enter your home to see your ship up close."),
            WorldLayerWant::NotAboard => Some("Your ship is drawn from its real meshes while you are aboard it."),
            WorldLayerWant::NotLive => {
                Some("Off the live clock the ship and Earth are drawn as symbols. Press Live to see them for real.")
            }
            WorldLayerWant::NoFeed => Some("The ship is shown as an outline here; in the game it is drawn for real."),
            _ => None,
        }
    }
}

/// Decide the world layer for this frame from the page's own state.
pub fn world_layer_want(maps: &MapsState) -> WorldLayerWant {
    if maps.view != MapView::System {
        return WorldLayerWant::OtherView;
    }
    let Some(me) = maps.me.as_ref() else { return WorldLayerWant::NoFeed };
    let scene = maps.scene();
    let near = (maps.camera.pose(&scene).eye - me.ship_centre_m()).length() <= NEAR_SPACE_MAX_M;
    if !me.world_loaded {
        WorldLayerWant::NotLoaded
    } else if maps.diagram {
        WorldLayerWant::Diagram
    } else if !me.aboard {
        WorldLayerWant::NotAboard
    } else if !maps.live {
        WorldLayerWant::NotLive
    } else if !near {
        WorldLayerWant::TooFar
    } else {
        WorldLayerWant::Draw
    }
}

/// Whether the camera is close enough to the ship that the world layer
/// would show it (the band the reasons above are worth saying in).
pub fn near_the_ship(maps: &MapsState) -> bool {
    let Some(me) = maps.me.as_ref() else { return false };
    !maps.diagram && (maps.camera.pose(&maps.scene()).eye - me.ship_centre_m()).length() <= WORLD_LAYER_MAX_M
}

/// Draw the world layer as the canvas backdrop when it is wanted and has
/// been rendered (the whole world, or the star sky). Returns what it drew.
pub fn draw_world_backdrop(paint: &egui::Painter, rect: Rect, maps: &MapsState) -> WorldDrawn {
    let want = world_layer_want(maps);
    let kind = if want == WorldLayerWant::Draw {
        WorldDrawn::Everything
    } else if want.draws_sky() {
        WorldDrawn::Sky
    } else {
        return WorldDrawn::Nothing;
    };
    // The last render must be of this kind: a render of the whole world is
    // not a sky, and the other way round.
    if maps.world.renders == 0 || maps.world.sky_only != (kind == WorldDrawn::Sky) {
        return WorldDrawn::Nothing;
    }
    let Some(tex) = maps.world.texture else { return WorldDrawn::Nothing };
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    paint.image(tex, rect, uv, Color32::WHITE);
    kind
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::pages::maps::camera::MapCamera;

    /// The world layer's bands (Maps increment 2): aboard and live, the whole
    /// world from the ship out to the edge of near space; beyond it, on the
    /// Diagram, off Live or away from the ship, the star sky alone; nothing
    /// with no feed or before the world is loaded.
    ///
    /// Seen red 2026-10-21 with `draws_sky` left false for the solar-system
    /// band: "11 AU out the world layer draws no star sky (TooFar)".
    #[test]
    fn the_whole_world_to_near_space_the_star_sky_beyond() {
        let me = crate::gui::pages::maps::camera::tests::feed_at(3600.0, glam::DVec3::ZERO);
        let mut maps = MapsState::default();
        maps.me = Some(me.clone());
        maps.sim_time_seconds = me.sky_seconds;
        maps.camera.open_on_me(Some(&me));
        assert_eq!(world_layer_want(&maps), WorldLayerWant::Draw, "at the ship");
        maps.camera.set_distance(1.0e9);
        assert_eq!(world_layer_want(&maps), WorldLayerWant::Draw, "a million km out is near space: the whole world");
        maps.camera = MapCamera::overview(11.0 * crate::cosmos::M_PER_AU);
        let far = world_layer_want(&maps);
        assert!(far == WorldLayerWant::TooFar && far.draws_sky(), "11 AU out the world layer draws no star sky ({far:?})");
        maps.camera.open_on_me(Some(&me));
        maps.diagram = true;
        assert!(world_layer_want(&maps).draws_sky(), "the Diagram keeps the star sky");
        maps.diagram = false;
        maps.live = false;
        assert!(world_layer_want(&maps).draws_sky(), "off Live keeps the star sky");
        maps.me.as_mut().unwrap().world_loaded = false;
        assert!(!world_layer_want(&maps).draws_sky(), "no world, no sky");
        maps.me = None;
        assert_eq!(world_layer_want(&maps), WorldLayerWant::NoFeed);
    }
}

