//! The Maps page's overlays as DATA (Maps increment 2,
//! docs/design/maps-zoom-to-anything.md 5.4): the layer switches
//! (`data/maps/layers.ron`), the named reference orbits
//! (`data/maps/reference_orbits.ron`) and the Lagrange pairs
//! (`data/maps/lagrange_pairs.ron`). Until this increment the last two were
//! code literals in `system_view.rs` whose own comments said "Could move to
//! data/ later", and the switches were two hard-coded buttons.
//!
//! Infinite-of-x: a new ring round any body, a new pair, a new layer is a row
//! in a file. The bar of switches over the canvas is drawn from the layer
//! list, showing only the layers that can be seen in the camera's band.
//!
//! Each file is read from the data folder first and from the copy built into
//! the exe when the folder has none or this version cannot read it
//! (`embedded_data::load_text_or_embedded`), once per run.

use std::sync::OnceLock;

use egui::{Align2, Pos2, Rect, RichText, Stroke, Vec2};
use glam::DVec3;
use serde::Deserialize;

use crate::cosmos::{body_world_position_3d_au, find_body, SolBody};
use crate::gui::theme::Theme;
use crate::gui::widgets::label_placer::LabelPlacer;
use crate::gui::GuiState;

use super::camera::{MapProjector, MapScene, NEAR_SPACE_MAX_M, WORLD_LAYER_MAX_M};

/// Where a layer shows, by the camera's distance from what it looks at
/// (section 4 of the design): the ship, near space, the solar system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Band {
    /// Up to 60 km: the ship from its own meshes.
    Ship,
    /// 60 km to 2 million km: Earth and the Moon, the ship an icon on its orbit.
    NearSpace,
    /// Beyond: the Sun's whole family.
    SolarSystem,
}

impl Band {
    /// The band a camera `distance_m` from its anchor is in.
    pub fn at(distance_m: f64) -> Band {
        if distance_m <= WORLD_LAYER_MAX_M {
            Band::Ship
        } else if distance_m <= NEAR_SPACE_MAX_M {
            Band::NearSpace
        } else {
            Band::SolarSystem
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Band::Ship => "Ship",
            Band::NearSpace => "NearSpace",
            Band::SolarSystem => "SolarSystem",
        }
    }
}

/// One overlay switch (`data/maps/layers.ron`).
#[derive(Debug, Clone, Deserialize)]
pub struct MapLayer {
    pub id: String,
    pub label: String,
    pub help: String,
    pub bands: Vec<Band>,
    #[serde(default)]
    pub default_on: bool,
}

#[derive(Debug, Deserialize)]
struct LayersFile {
    layers: Vec<MapLayer>,
}

/// One named ring round a body (`data/maps/reference_orbits.ron`).
#[derive(Debug, Clone, Deserialize)]
pub struct ReferenceOrbit {
    pub body: String,
    pub name: String,
    /// Above the body's surface.
    pub altitude_km: f64,
    pub about: String,
}

#[derive(Debug, Deserialize)]
struct ReferenceFile {
    orbits: Vec<ReferenceOrbit>,
}

/// One Lagrange pair (`data/maps/lagrange_pairs.ron`).
#[derive(Debug, Clone, Deserialize)]
pub struct LagrangePair {
    pub parent: String,
    pub child: String,
    pub label: String,
    /// (point, what is parked there).
    #[serde(default)]
    pub notable: Vec<(String, String)>,
}

/// The pairs and what each of the five points is.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LagrangeFile {
    pub pairs: Vec<LagrangePair>,
    /// (point, what it is), L1 to L5.
    #[serde(default)]
    pub points: Vec<(String, String)>,
}

fn load<T: for<'de> Deserialize<'de>>(rel: &str) -> Option<T> {
    match crate::embedded_data::load_text_or_embedded(&crate::data_dir(), rel, |t| ron::from_str::<T>(t).map_err(|e| e.to_string())) {
        Ok(v) => Some(v),
        Err(e) => {
            log::warn!("Maps: {e}; that overlay is left out");
            None
        }
    }
}

/// Every layer, in the file's order (the order of the buttons).
pub fn layers() -> &'static [MapLayer] {
    static L: OnceLock<Vec<MapLayer>> = OnceLock::new();
    L.get_or_init(|| load::<LayersFile>("maps/layers.ron").map(|f| f.layers).unwrap_or_default())
}

/// A layer's default, or off for an id the file does not know.
pub fn layer_default(id: &str) -> bool {
    layers().iter().find(|l| l.id == id).map(|l| l.default_on).unwrap_or(false)
}

/// Every reference orbit.
pub fn reference_orbits() -> &'static [ReferenceOrbit] {
    static R: OnceLock<Vec<ReferenceOrbit>> = OnceLock::new();
    R.get_or_init(|| load::<ReferenceFile>("maps/reference_orbits.ron").map(|f| f.orbits).unwrap_or_default())
}

/// The Lagrange pairs.
pub fn lagrange() -> &'static LagrangeFile {
    static P: OnceLock<LagrangeFile> = OnceLock::new();
    P.get_or_init(|| load::<LagrangeFile>("maps/lagrange_pairs.ron").unwrap_or_default())
}

/// Whether layer `id` draws this frame: switched on, and the band is one of
/// its own.
pub fn showing(state: &GuiState, id: &str, band: Band) -> bool {
    state.maps.layer_on(id) && layers().iter().any(|l| l.id == id && l.bands.contains(&band))
}

/// The bar of switches in the canvas's top-right corner, one per layer that
/// can be seen in this band, then the Diagram switch. Returns the bar's
/// rectangle so the names keep off it.
pub(super) fn draw_layer_bar(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, rect: Rect, band: Band) -> Rect {
    let bar = Rect::from_min_max(Pos2::new(rect.left() + rect.width() * 0.35, rect.top() + 36.0), Pos2::new(rect.right() - 8.0, rect.top() + 60.0));
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(egui::Layout::right_to_left(egui::Align::Center)));
    let mut used = Rect::NOTHING;
    // Right to left: Diagram at the far right, then the layers last to first.
    let diagram_label = if state.maps.diagram { "Diagram: on" } else { "Diagram: off" };
    let r = toggle_button(&mut ui, theme, diagram_label, state.maps.diagram, "Diagram: spaces the orbits by the logarithm of their distance and enlarges every body, like a textbook orrery. Not to scale. Off is true scale.");
    used = used.union(r.rect);
    if r.clicked() {
        state.maps.diagram = !state.maps.diagram;
        // The diagram's places are not the true ones: start it from its own
        // overview, and come back to you when it is switched off.
        let me = state.maps.me.clone();
        let scene = MapScene::new(me.as_ref(), state.maps.sim_time_seconds, state.maps.live).with_diagram(state.maps.diagram);
        if state.maps.diagram {
            state.maps.camera.fly_to(super::camera::MapAnchor::Body("sun".into()), super::camera::DIAGRAM_OVERVIEW_M, 0.0, super::camera::OVERVIEW_PITCH_RAD, &scene);
        } else {
            state.maps.camera.back_to_me(&scene);
        }
    }
    for layer in layers().iter().rev().filter(|l| l.bands.contains(&band)) {
        let on = state.maps.layer_on(&layer.id);
        let r = toggle_button(&mut ui, theme, &layer.label, on, &layer.help);
        used = used.union(r.rect);
        if r.clicked() {
            state.maps.layers_on.insert(layer.id.clone(), !on);
        }
    }
    used
}

fn toggle_button(ui: &mut egui::Ui, theme: &Theme, label: &str, on: bool, help: &str) -> egui::Response {
    let colour = if on { theme.info() } else { theme.text_secondary() };
    ui.add(
        egui::Button::new(RichText::new(label).size(theme.font_size_small).color(colour))
            .fill(theme.bg_card())
            .stroke(Stroke::new(0.8, if on { theme.info() } else { theme.border() })),
    )
    .on_hover_text(help)
}

// ─────────────────────── Reference orbits ──────────────────────────────────

/// A reference ring of radius `r_m` round `centre`, `n` points in the
/// body's equatorial plane: the world's XZ plane (the game has no axial tilt).
pub(super) fn reference_ring_points(centre: DVec3, r_m: f64, n: usize) -> Vec<DVec3> {
    (0..n)
        .map(|i| {
            let theta = i as f64 * std::f64::consts::TAU / n as f64;
            centre + DVec3::new(r_m * theta.cos(), 0.0, r_m * theta.sin())
        })
        .collect()
}

/// Draw the reference rings round every body that has them, each one only
/// when it stands clear of the body's disc on screen and is not far bigger
/// than the canvas (until increment 2 a ring showed only once its PLANET was
/// 12 px across, which hid GEO round a 4 px Earth where it is plainly
/// readable). Each ring is named once, through the label placer. Returns the
/// ring the pointer is on: (name, about).
pub(super) fn draw_reference_orbits(
    paint: &egui::Painter,
    theme: &Theme,
    proj: &MapProjector,
    scene: &MapScene,
    labels: &mut Vec<OverlayLabel>,
    hover_pos: Option<Pos2>,
) -> Option<(String, String)> {
    let colour = theme.map_reference();
    let mut hovered = None;
    let too_big = proj.rect.width().max(proj.rect.height()) * 4.0;
    for ring in reference_orbits() {
        let Some(body) = find_body(&ring.body) else { continue };
        let Some(centre) = scene.body_m(&body.id) else { continue };
        let Some((_, depth)) = proj.project(centre) else { continue };
        let r_m = (body.radius_km + ring.altitude_km) * 1000.0;
        let body_px = proj.radius_px(scene.radius_m(body), depth).max(super::bodies::icon_floor_px(&body.id, &body.body_type));
        let ring_px = scene.near_m(&body.id, DVec3::new(r_m, 0.0, 0.0)).map(|p| proj.radius_px((p - centre).length(), depth)).unwrap_or(0.0);
        if ring_px < (body_px * 1.4).max(6.0) || ring_px > too_big {
            continue;
        }
        let pts: Vec<DVec3> = reference_ring_points(DVec3::ZERO, r_m, 128)
            .into_iter()
            .filter_map(|p| scene.near_m(&body.id, p))
            .collect();
        let mut prev: Option<Pos2> = None;
        let mut first: Option<Pos2> = None;
        let mut nearest = f32::MAX;
        for p in &pts {
            let s = proj.project(*p).map(|(s, _)| s);
            if let (Some(a), Some(b)) = (prev, s) {
                paint.line_segment([a, b], Stroke::new(0.7, colour));
            }
            if let (Some(hp), Some(b)) = (hover_pos, s) {
                nearest = nearest.min((hp - b).length());
            }
            if first.is_none() {
                first = s;
            }
            prev = s;
        }
        if let (Some(a), Some(b)) = (prev, first) {
            paint.line_segment([a, b], Stroke::new(0.7, colour));
        }
        if nearest < 6.0 {
            hovered = Some((ring.name.clone(), ring.about.clone()));
        }
        // The ring's name at its +X point, placed after the bodies' names.
        if let Some((at, _)) = scene.near_m(&body.id, DVec3::new(r_m, 0.0, 0.0)).and_then(|p| proj.project(p)) {
            labels.push(OverlayLabel { text: ring.name.clone(), at, colour, below_only: false });
        }
    }
    hovered
}

/// A name an overlay asks for (a ring's, an L point's), placed AFTER the
/// bodies' names so a marker never takes a planet's name's spot (the first
/// overview snapshot lost Earth's and Venus's names to the Sun-Earth L1 and
/// L2 labels, 2026-10-21).
#[derive(Debug, Clone)]
pub struct OverlayLabel {
    pub text: String,
    pub at: Pos2,
    pub colour: egui::Color32,
    /// Only under the marker (an L point's name), else any side.
    pub below_only: bool,
}

/// Place and paint the overlays' names where they still fit.
pub(super) fn place_overlay_labels(paint: &egui::Painter, placer: &mut LabelPlacer, labels: Vec<OverlayLabel>) {
    for l in labels {
        let galley = paint.layout_no_wrap(l.text, egui::FontId::proportional(9.5), l.colour);
        let size = galley.size();
        let spot = if l.below_only {
            placer.place([Align2::CENTER_TOP.anchor_size(l.at + Vec2::new(0.0, 6.0), size)])
        } else {
            placer.place(crate::gui::widgets::label_placer::point_candidates(l.at, 1.0, size, 4.0))
        };
        if let Some(spot) = spot {
            paint.galley(spot.min, galley, l.colour);
        }
    }
}

// ─────────────────────── Lagrange points ───────────────────────────────────

/// Compute the five Lagrange points for a (parent, child) two-body system
/// at the given sim_time. Returns positions in heliocentric AU.
///
/// L1, L2, L3 use the cube-root-of-mass-ratio approximation
/// (good to ~1% for μ < 0.01, which covers all our pairs since even
/// Jupiter/Sun is ~0.001). L4 and L5 are the exact 60°-ahead/behind
/// equilateral triangles in the child's orbit plane.
pub(super) fn compute_lagrange_points(parent: &SolBody, child: &SolBody, sim_time_seconds: f64) -> Option<[DVec3; 5]> {
    if parent.mass_kg <= 0.0 || child.mass_kg <= 0.0 {
        return None;
    }
    let parent_pos = body_world_position_3d_au(parent, sim_time_seconds);
    let child_pos = body_world_position_3d_au(child, sim_time_seconds);
    let separation = child_pos - parent_pos;
    let dist = separation.length();
    if dist <= 0.0 {
        return None;
    }
    let to_child = separation / dist;

    // μ = m_child / (m_parent + m_child). For Earth-Moon μ ≈ 0.0123;
    // for Sun-Earth μ ≈ 3e-6; for Sun-Jupiter μ ≈ 9.5e-4.
    let mu = child.mass_kg / (parent.mass_kg + child.mass_kg);
    let r_hill_factor = (mu / 3.0).cbrt();
    let l1 = parent_pos + to_child * (dist * (1.0 - r_hill_factor));
    let l2 = parent_pos + to_child * (dist * (1.0 + r_hill_factor));
    let l3 = parent_pos - to_child * (dist * (1.0 + 5.0 * mu / 12.0));

    // L4 / L5 = 60° rotation in the child's orbit plane, about its orbit
    // normal r x v, so a positive turn goes the way the child moves: L4
    // leads, L5 trails. The velocity is the child's own motion round the
    // parent over an hour. (Until Maps increment 1 this used "ecliptic up =
    // +Z", which lies IN the ecliptic of the engine's Y-up frame: L4 and L5
    // swapped every half orbit and went NaN on the Z axis.)
    let later = body_world_position_3d_au(child, sim_time_seconds + 3600.0) - body_world_position_3d_au(parent, sim_time_seconds + 3600.0);
    let orbit_normal = separation.cross(later - separation).normalize_or_zero();
    if orbit_normal == DVec3::ZERO {
        return None;
    }
    // Rodrigues' rotation formula, +60° and -60° about the orbit normal.
    let rot = |v: DVec3, theta: f64| -> DVec3 {
        let (s, c) = theta.sin_cos();
        v * c + orbit_normal.cross(v) * s + orbit_normal * orbit_normal.dot(v) * (1.0 - c)
    };
    let l4 = parent_pos + rot(separation, 60.0_f64.to_radians());
    let l5 = parent_pos + rot(separation, -60.0_f64.to_radians());
    Some([l1, l2, l3, l4, l5])
}

/// A Lagrange point (heliocentric AU) in the map's frame: through the
/// parent's own place, so on the diagram the Earth-Moon points stand round
/// Earth's enlarged disc as the Moon does.
fn lagrange_m(scene: &MapScene, parent: &SolBody, p_au: DVec3) -> Option<DVec3> {
    if parent.id == "sun" {
        return Some(scene.au_to_m(p_au));
    }
    let rel = (p_au - body_world_position_3d_au(parent, scene.sim_time)) * crate::cosmos::M_PER_AU;
    scene.near_m(&parent.id, rel)
}

/// Draw L1 to L5 for every pair whose two bodies are far enough apart on
/// screen to tell the points apart, leaving out a point that would sit on
/// either body (Sun-Earth L1 and L2 are 1.5 million km from Earth, under a
/// pixel at the overview); their names are asked for in `labels`. Returns
/// the hover text of the point under the pointer: (title, what is there,
/// what the point is).
pub(super) fn draw_lagrange(
    paint: &egui::Painter,
    theme: &Theme,
    proj: &MapProjector,
    scene: &MapScene,
    (placer, labels): (&mut LabelPlacer, &mut Vec<OverlayLabel>),
    hover_pos: Option<Pos2>,
) -> Option<(String, String, String)> {
    /// A marker this close to a body, px, is left out: it would hide the body.
    const CLEAR_PX: f32 = 8.0;
    const NAMES: [&str; 5] = ["L1", "L2", "L3", "L4", "L5"];
    let colour = theme.map_reference();
    let data = lagrange();
    let mut hovered = None;
    for pair in &data.pairs {
        let (Some(parent), Some(child)) = (find_body(&pair.parent), find_body(&pair.child)) else { continue };
        let Some(points) = compute_lagrange_points(parent, child, scene.sim_time) else { continue };
        let screen = |id: &str| scene.body_m(id).and_then(|p| proj.project(p)).map(|(s, _)| s);
        let (Some(ps), Some(cs)) = (screen(&parent.id), screen(&child.id)) else { continue };
        let spread = (cs - ps).length();
        if spread < 30.0 {
            continue; // the five would sit on top of each other
        }
        for (i, pt) in points.iter().enumerate() {
            let Some((s, _)) = lagrange_m(scene, parent, *pt).and_then(|p| proj.project(p)) else { continue };
            if (s - ps).length() < CLEAR_PX || (s - cs).length() < CLEAR_PX {
                continue;
            }
            let k = 4.0;
            paint.line_segment([s + Vec2::new(-k, -k), s + Vec2::new(k, k)], Stroke::new(1.0, colour));
            paint.line_segment([s + Vec2::new(-k, k), s + Vec2::new(k, -k)], Stroke::new(1.0, colour));
            // The cross itself keeps names off it (its own name comes later).
            placer.reserve_exact(Rect::from_center_size(s, Vec2::splat(2.0 * k)));
            if spread > 50.0 {
                labels.push(OverlayLabel { text: NAMES[i].to_string(), at: s, colour, below_only: true });
            }
            if hover_pos.is_some_and(|hp| (hp - s).length() < 10.0) {
                let notable = pair.notable.iter().find(|(n, _)| n == NAMES[i]).map(|(_, w)| w.clone()).unwrap_or_default();
                let about = data.points.iter().find(|(n, _)| n == NAMES[i]).map(|(_, w)| w.clone()).unwrap_or_default();
                hovered = Some((format!("{} {}", pair.label, NAMES[i]), notable, about));
            }
        }
    }
    hovered
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three overlay files ship, parse, and carry what the page used to
    /// hard-code: five layers with the Lagrange points and the reference
    /// orbits among them, Earth's GEO ring at 35,786 km, the five pairs with
    /// JWST at Sun-Earth L2, and all five points explained.
    ///
    /// Seen red 2026-10-21 with the GEO row's altitude typed as 35768: "Earth's
    /// GEO ring is at 35768 km, not 35,786".
    #[test]
    fn the_shipped_map_overlays_parse() {
        let ids: Vec<&str> = layers().iter().map(|l| l.id.as_str()).collect();
        for want in ["orbits", "names", "belts", "reference_orbits", "lagrange"] {
            assert!(ids.contains(&want), "layers.ron has no {want:?} layer: {ids:?}");
        }
        assert!(layer_default("belts") && !layer_default("lagrange") && !layer_default("no_such_layer"));
        let geo = reference_orbits().iter().find(|r| r.body == "earth" && r.name == "GEO").expect("Earth's GEO ring");
        assert!((geo.altitude_km - 35_786.0).abs() < 0.5, "Earth's GEO ring is at {} km, not 35,786", geo.altitude_km);
        for r in reference_orbits() {
            assert!(find_body(&r.body).is_some(), "a reference orbit names a body sol.json does not have: {}", r.body);
        }
        let l = lagrange();
        assert_eq!(l.pairs.len(), 5);
        let se = l.pairs.iter().find(|p| p.parent == "sun" && p.child == "earth").expect("Sun-Earth");
        assert!(se.notable.iter().any(|(p, w)| p == "L2" && w.contains("JWST")));
        assert_eq!(l.points.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["L1", "L2", "L3", "L4", "L5"]);
    }

    /// The bands follow the design's table: the ship to 60 km, near space to
    /// 2 million km, the solar system beyond.
    ///
    /// Seen red 2026-10-21 with near space ending at STARS_FRAME_FULL_M (the
    /// camera frame blend's end, 2,000 km, not 2 million): "1e8 m (100,000
    /// km) is SolarSystem, not NearSpace".
    #[test]
    fn the_bands_follow_the_design_table() {
        assert_eq!(Band::at(10.0), Band::Ship);
        assert_eq!(Band::at(60_000.0), Band::Ship);
        assert_eq!(Band::at(1.0e8), Band::NearSpace, "1e8 m (100,000 km) is {:?}, not NearSpace", Band::at(1.0e8));
        assert_eq!(Band::at(1.9e9), Band::NearSpace);
        assert_eq!(Band::at(1.0e12), Band::SolarSystem);
    }
}
