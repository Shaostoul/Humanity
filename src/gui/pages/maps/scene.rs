//! The Maps page's SCENE: where everything is for one draw (Maps increment 1,
//! moved out of `camera.rs` in increment 2 when the Diagram joined it).

use glam::DVec3;

use crate::cosmos::{body_world_position_3d_au, find_body, M_PER_AU};
use crate::gui::state_types::MapSelf;

use super::camera::MapAnchor;
use super::diagram;

/// Where everything is, for one draw: the engine's feed (if any), the
/// page's clock, and whether the page shows the Diagram (Maps increment 2,
/// `diagram.rs`): every position below is a DISPLAY position, true scale or
/// the diagram's, so the camera, the orbits, the bodies and the belts agree.
#[derive(Debug, Clone, Copy)]
pub struct MapScene<'a> {
    pub me: Option<&'a MapSelf>,
    /// Seconds since J2000.0 the bodies are placed at.
    pub sim_time: f64,
    /// The clock is on the engine's own sky clock (the Sun then comes from
    /// the engine, exactly; off it, from the page's clock).
    pub live: bool,
    /// Diagram mode: log-spaced orbits and enlarged bodies, not to scale.
    pub diagram: bool,
    /// Earth's place round the Sun at `sim_time`, AU, worked out once: every
    /// orbit sample and belt point is measured from it (the map frame's origin).
    earth_au: DVec3,
}

impl<'a> MapScene<'a> {
    pub fn new(me: Option<&'a MapSelf>, sim_time: f64, live: bool) -> Self {
        let earth_au = find_body("earth").map(|e| body_world_position_3d_au(e, sim_time)).unwrap_or(DVec3::ZERO);
        Self { me, sim_time, live, diagram: false, earth_au }
    }

    /// The same scene with Diagram on or off.
    pub fn with_diagram(mut self, on: bool) -> Self {
        self.diagram = on;
        self
    }

    /// Earth's place around the Sun, AU (`cosmos`), at the scene's time.
    pub fn earth_helio_au(&self) -> DVec3 {
        self.earth_au
    }

    /// Earth's place on the map's own scale: as above, or its Diagram place.
    pub fn earth_display_au(&self) -> DVec3 {
        if self.diagram { diagram::helio_au(self.earth_au) } else { self.earth_au }
    }

    /// A heliocentric AU position (`cosmos`) in the map's frame: metres from
    /// Earth's centre (on the diagram, from Earth's diagram place, so Earth
    /// stays the origin in both modes).
    pub fn au_to_m(&self, p_au: DVec3) -> DVec3 {
        if self.diagram {
            (diagram::helio_au(p_au) - diagram::helio_au(self.earth_au)) * M_PER_AU
        } else {
            (p_au - self.earth_au) * M_PER_AU
        }
    }

    /// A body's centre in the map's frame. The Sun comes from the engine
    /// while live (`MapSelf::sun_pos_m`), exactly as the game placed it.
    /// On the diagram a moon stands off its planet by `diagram::near_offset`.
    pub fn body_m(&self, id: &str) -> Option<DVec3> {
        if id == "earth" {
            return Some(DVec3::ZERO);
        }
        if id == "sun" && self.live && !self.diagram {
            // Not before the world has placed it: until then the engine's
            // sun sits at the origin, which is Earth's centre.
            if let Some(me) = self.me.filter(|m| m.sun_pos_m.length() > 1.0e9) {
                return Some(me.sun_pos_m);
            }
        }
        let body = find_body(id)?;
        if self.diagram {
            if let Some(parent) = body.parent.as_deref().filter(|p| *p != "sun").and_then(find_body) {
                let rel = (body_world_position_3d_au(body, self.sim_time) - body_world_position_3d_au(parent, self.sim_time)) * M_PER_AU;
                return Some(self.body_m(&parent.id)? + diagram::near_offset(parent.radius_km, rel));
            }
        }
        Some(self.au_to_m(body_world_position_3d_au(body, self.sim_time)))
    }

    /// A point `offset_m` (true metres, world axes) from body `parent`'s
    /// centre, in the map's frame: the point itself at true scale, spaced out
    /// from the enlarged planet on the diagram. Moons' orbits, the ship, the
    /// reference rings and a planet's Lagrange points go through this.
    pub fn near_m(&self, parent: &str, offset_m: DVec3) -> Option<DVec3> {
        let base = self.body_m(parent)?;
        if !self.diagram {
            return Some(base + offset_m);
        }
        let r_km = find_body(parent).map(|b| b.radius_km).unwrap_or(6371.0);
        Some(base + diagram::near_offset(r_km, offset_m))
    }

    /// A true world-frame point near your ship (its hull, its orbit ring, you)
    /// in the map's frame: unchanged at true scale, placed round Earth's
    /// enlarged disc on the diagram.
    pub fn ship_place(&self, world_m: DVec3) -> DVec3 {
        let Some(me) = self.me.filter(|_| self.diagram) else { return world_m };
        // The ship's parent is Earth in practice, the map frame's origin; any
        // other parent is found at its true place first.
        let parent_true = if me.parent == "earth" {
            DVec3::ZERO
        } else {
            find_body(&me.parent).map(|b| (body_world_position_3d_au(b, self.sim_time) - self.earth_helio_au()) * M_PER_AU).unwrap_or(DVec3::ZERO)
        };
        self.near_m(&me.parent, world_m - parent_true).unwrap_or(world_m)
    }

    /// A body's radius as drawn, metres: its own, or its diagram size.
    pub fn radius_m(&self, body: &crate::cosmos::SolBody) -> f64 {
        if self.diagram {
            diagram::display_radius_m(body.radius_km)
        } else {
            body.radius_km * 1000.0
        }
    }

    /// Where "you" are for the camera: your ship's centre while aboard it,
    /// else your own position. None with no feed.
    pub fn me_m(&self) -> Option<DVec3> {
        self.me.map(|m| self.ship_place(if m.aboard { m.ship_centre_m() } else { m.you_world_m }))
    }

    /// The anchor's position, if it can be found.
    pub fn anchor_m(&self, anchor: &MapAnchor) -> Option<DVec3> {
        match anchor {
            MapAnchor::Me => self.me_m(),
            MapAnchor::Body(id) => self.body_m(id),
            MapAnchor::Point { body, offset_m } => self.body_m(body).map(|b| b + *offset_m),
        }
    }
}
