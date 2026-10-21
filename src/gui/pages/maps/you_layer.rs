//! You and your ship on the Maps page (Maps increment 1,
//! docs/design/maps-zoom-to-anything.md): the WORLD layer the engine renders
//! from the map's camera (the ship from its own meshes, Earth behind it), and
//! the symbols over it: the ship's name and real altitude, its orbit ring,
//! "You" where you are aboard, the mining asteroids where the data puts them
//! and the drones flying between.
//!
//! Before this increment "You · Home" was a ring at EARTH'S CENTRE and the
//! asteroids a fan at fixed pixel offsets around it.
//!
//! Everything is placed from `MapSelf` (the engine's own values this frame,
//! `engine/map_feed.rs`) and projected with the page's `MapProjector`, the
//! same pose the world layer was rendered from (`engine/map_view.rs`
//! `render_camera`), so a symbol lands on the thing it names.

use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use glam::DVec3;

use crate::gui::state_types::{MapSelf, MapsState};
use crate::gui::theme::Theme;
use crate::gui::GuiState;

use super::camera::{MapProjector, MapScene, WORLD_LAYER_MAX_M};
use super::{format_with_commas, MapView};

/// Under this many pixels across, a body or the ship is an ICON of a minimum
/// size; over it the world layer's real thing takes over (design 5.3).
pub const ICON_FLOOR_PX: f32 = 6.0;

/// Whether the world layer is drawn, and if not, why not. One function for
/// the engine (which renders it) and the page (which draws it and says why
/// it is missing), so they never disagree.
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
    /// The camera is beyond the ship band (your ship is an icon there).
    TooFar,
}

impl WorldLayerWant {
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
    let scene = MapScene::new(Some(me), maps.sim_time_seconds, maps.live);
    let near = (maps.camera.pose(&scene).eye - me.ship_centre_m()).length() <= WORLD_LAYER_MAX_M;
    if !me.world_loaded {
        WorldLayerWant::NotLoaded
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
    let scene = MapScene::new(Some(me), maps.sim_time_seconds, maps.live);
    (maps.camera.pose(&scene).eye - me.ship_centre_m()).length() <= WORLD_LAYER_MAX_M
}

/// Draw the world layer as the canvas backdrop when it is wanted and has
/// been rendered. Returns whether it was drawn.
pub(super) fn draw_world_backdrop(paint: &egui::Painter, rect: Rect, maps: &MapsState) -> bool {
    if world_layer_want(maps) != WorldLayerWant::Draw || maps.world.renders == 0 {
        return false;
    }
    let Some(tex) = maps.world.texture else { return false };
    let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    paint.image(tex, rect, uv, Color32::WHITE);
    true
}

/// The orbit through `pos` moving at `vel` around a body of gravitational
/// parameter `mu` at the origin, as `n` points with `pos` itself one of them
/// (so the ship always sits exactly on its line, the rule the in-world orbit
/// lines keep). Empty for an escape trajectory. Drawn from the engine's own
/// state vector: never a second propagation of the station's elements.
pub fn orbit_ring(pos: DVec3, vel: DVec3, mu: f64, n: usize) -> Vec<DVec3> {
    let r = pos.length();
    if r <= 0.0 || mu <= 0.0 || n < 3 {
        return Vec::new();
    }
    let h = pos.cross(vel);
    let energy = vel.length_squared() * 0.5 - mu / r;
    if energy >= 0.0 || h.length_squared() <= 0.0 {
        return Vec::new();
    }
    let a = -mu / (2.0 * energy);
    let e_vec = vel.cross(h) / mu - pos / r;
    let e = e_vec.length().min(0.99);
    let p_hat = if e > 1e-9 { e_vec / e_vec.length() } else { pos / r };
    let q_hat = h.normalize().cross(p_hat);
    let b = a * (1.0 - e * e).sqrt();
    let mut ring: Vec<DVec3> = (0..n)
        .map(|i| {
            let ea = i as f64 / n as f64 * std::f64::consts::TAU;
            p_hat * (a * (ea.cos() - e)) + q_hat * (b * ea.sin())
        })
        .collect();
    if let Some((i, _)) = ring.iter().enumerate().min_by(|(_, x), (_, y)| {
        x.distance_squared(pos).partial_cmp(&y.distance_squared(pos)).unwrap_or(std::cmp::Ordering::Equal)
    }) {
        ring[i] = pos;
    }
    ring
}

/// Stroke a closed polyline of world points, breaking it where a point is
/// behind the camera.
fn stroke_ring(paint: &egui::Painter, proj: &MapProjector, pts: &[DVec3], stroke: Stroke) {
    let mut prev: Option<Pos2> = None;
    for p in pts.iter().chain(pts.first()) {
        let s = proj.project(*p).map(|(s, _)| s);
        if let (Some(a), Some(b)) = (prev, s) {
            paint.line_segment([a, b], stroke);
        }
        prev = s;
    }
}

/// The ship's box as twelve projected edges (the outline drawn when the
/// world layer is not), or nothing when part of it is behind the camera.
fn box_edges(proj: &MapProjector, me: &MapSelf) -> Option<Vec<[Pos2; 2]>> {
    let (lo, hi) = me.ship_box_m?;
    let corner = |i: usize| {
        let pick = |bit: usize, a: f64, b: f64| if i & bit == 0 { a } else { b };
        me.ship_to_world(DVec3::new(pick(1, lo.x, hi.x), pick(2, lo.y, hi.y), pick(4, lo.z, hi.z)))
    };
    let pts: Vec<Pos2> = (0..8).map(|i| proj.project(corner(i)).map(|(s, _)| s)).collect::<Option<_>>()?;
    let mut edges = Vec::with_capacity(12);
    for i in 0..8usize {
        for bit in [1usize, 2, 4] {
            if i & bit == 0 {
                edges.push([pts[i], pts[i | bit]]);
            }
        }
    }
    Some(edges)
}

/// The ship's label: its name, then whose it is and its height.
pub fn ship_label(me: &MapSelf, parent_radius_m: f64) -> (String, String) {
    let alt_km = (me.ship_pos_m.length() - parent_radius_m) / 1000.0;
    let you = if me.aboard { "your home, you are aboard" } else { "your home" };
    (me.ship_name.clone(), format!("{you} · {} km up", format_with_commas(alt_km.round() as i64)))
}

/// Everything about you and your ship. Returns the asteroid markers (id,
/// canvas point) so a click on one can open its mining card.
pub(super) fn draw_you(
    paint: &egui::Painter,
    theme: &Theme,
    proj: &MapProjector,
    scene: &MapScene,
    state: &mut GuiState,
    world_drawn: bool,
) -> Vec<(String, Pos2)> {
    let mut markers = Vec::new();
    state.maps.ship_px = None;
    let Some(me) = scene.me else { return markers };
    let accent = theme.accent();

    // The ship's orbit, from its own state vector.
    let ring = orbit_ring(me.ship_pos_m, me.ship_vel_mps, me.parent_mu, 192);
    stroke_ring(paint, proj, &ring, Stroke::new(1.2, accent.gamma_multiply(0.7)));

    let centre = me.ship_centre_m();
    let Some((at, depth)) = proj.project(centre) else { return markers };
    let ppp = state.maps.pixels_per_point.max(0.1);
    state.maps.ship_px = Some([at.x * ppp, at.y * ppp]);
    let length_px = proj.radius_px(me.ship_length_m() * 0.5, depth) * 2.0;
    let big = length_px > 40.0;

    // The ship itself: the world layer draws it when it is there; otherwise
    // its outline when it is big on screen and an icon of a minimum size.
    if !world_drawn && big {
        if let Some(edges) = box_edges(proj, me) {
            for e in edges {
                paint.line_segment(e, Stroke::new(1.0, accent.gamma_multiply(0.8)));
            }
        }
    }
    if !world_drawn || length_px < ICON_FLOOR_PX {
        paint.circle_filled(at, 4.0, accent);
        paint.circle_stroke(at, 9.0, Stroke::new(1.6, accent));
    }
    let radius = crate::cosmos::find_body(&me.parent).map(|b| b.radius_km * 1000.0).unwrap_or(0.0);
    let (name, line2) = ship_label(me, radius);
    let lift = if big { (length_px * 0.5).min(proj.rect.height() * 0.4) + 6.0 } else { 14.0 };
    let label_at = at - Vec2::new(0.0, lift);
    paint.text(label_at - Vec2::new(0.0, 13.0), Align2::CENTER_BOTTOM, name, FontId::proportional(13.0), accent);
    paint.text(label_at, Align2::CENTER_BOTTOM, line2, FontId::proportional(10.5), theme.text_secondary());

    // You, where you are aboard, once the ship is big enough to tell.
    if big {
        if let Some((you, _)) = proj.project(me.you_world_m) {
            paint.circle_filled(you, 3.5, theme.info());
            paint.circle_stroke(you, 6.0, Stroke::new(1.2, theme.info()));
            paint.text(you + Vec2::new(8.0, 0.0), Align2::LEFT_CENTER, "You", FontId::proportional(11.0), theme.info());
        }
    }

    // The mining asteroids where the data puts them: km from the ship's
    // frame origin, in the ship's own axes (they keep station with it).
    let mut placed: std::collections::HashMap<String, Pos2> = std::collections::HashMap::new();
    for ast in &state.asteroids {
        let local = DVec3::new(ast.position[0] as f64, ast.position[1] as f64, ast.position[2] as f64) * 1000.0;
        let Some((p, _)) = proj.project(me.ship_to_world(local)) else { continue };
        if !proj.rect.expand(40.0).contains(p) {
            continue;
        }
        paint.circle_filled(p, 3.0, crate::gui::widgets::swatch_color(&ast.classification));
        paint.text(
            p + Vec2::new(6.0, 0.0),
            Align2::LEFT_CENTER,
            format!("{} · {:.0} km", ast.name, ast.distance),
            FontId::proportional(9.5),
            theme.text_secondary(),
        );
        placed.insert(ast.id.clone(), p);
        markers.push((ast.id.clone(), p));
    }
    // A drone in flight, along its leg between the ship and its rock.
    for drone in &state.drones {
        let Some(&rock) = placed.get(&drone.target) else { continue };
        let t = drone.phase_progress.clamp(0.0, 1.0);
        let dp = match drone.phase.as_str() {
            "Outbound" => at + (rock - at) * t,
            "Returning" => rock + (at - rock) * t,
            _ => rock,
        };
        paint.circle_filled(dp, 2.5, theme.info());
        paint.text(dp + Vec2::new(5.0, -4.0), Align2::LEFT_BOTTOM, "Drone", FontId::proportional(9.0), theme.info());
    }
    markers
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ship's orbit ring is drawn from its own state vector, passes
    /// exactly through the ship, and for the shipped synchronous home is a
    /// circle at the ship's own distance (42,241 km, the engine's).
    ///
    /// Seen red 2026-10-21 with the ship not spliced into the ring: "on an
    /// eccentric orbit the ring misses the ship by 111.4 km".
    #[test]
    fn the_orbit_ring_passes_through_the_ship() {
        for hour in [0.0, 7.0, 13.5] {
            let me = super::super::camera::tests::feed_at(hour * 3600.0, DVec3::ZERO);
            let ring = orbit_ring(me.ship_pos_m, me.ship_vel_mps, me.parent_mu, 192);
            assert_eq!(ring.len(), 192);
            let miss = ring.iter().map(|p| p.distance(me.ship_pos_m)).fold(f64::MAX, f64::min);
            assert!(miss < 1.0, "the ring misses the ship by {miss:.4} m");
            let r = me.ship_pos_m.length();
            for p in &ring {
                assert!((p.length() / r - 1.0).abs() < 1e-6, "a synchronous ring is a circle at the ship's distance");
            }
        }
        // An eccentric orbit, the ship well past periapsis: a ring sampled
        // evenly round the ellipse would pass hundreds of km from it.
        let (pos, vel) = (DVec3::new(1.0e7, 0.0, 0.0), DVec3::new(2000.0, 0.0, 6000.0));
        let ring = orbit_ring(pos, vel, crate::station::orbit::MU_EARTH, 192);
        let miss = ring.iter().map(|p| p.distance(pos)).fold(f64::MAX, f64::min);
        assert!(miss == 0.0, "on an eccentric orbit the ring misses the ship by {:.1} km", miss / 1000.0);
    }

    /// The ship's label says its real height, from the engine's own pose:
    /// 35,870 km over the game's 6,371 km Earth, never the old "400 km LEO".
    ///
    /// Seen red 2026-10-21 with the height not grouped: left "your home, you
    /// are aboard · 35870 km up", right "... · 35,870 km up".
    #[test]
    fn the_ship_label_says_its_real_height() {
        let me = super::super::camera::tests::feed_at(0.0, DVec3::ZERO);
        let (name, line2) = ship_label(&me, 6_371_000.0);
        assert_eq!(name, "UHS Mothership");
        assert_eq!(line2, "your home, you are aboard · 35,870 km up");
    }
}
