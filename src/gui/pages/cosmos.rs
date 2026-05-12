//! Cosmos page (v0.203.0, Phase 3 of the cosmos architecture).
//!
//! Three view modes, one canvas:
//! - **System** — Sol's planets in their orbits (top-down 2D, AU-scale)
//! - **Galactic** — Sol-centered top-down map of nearby stars (light-year
//!   scale), labeled with proper names + spectral colors
//! - **Night Sky** — Earth-centered celestial sphere (RA/Dec
//!   equirectangular projection) with bright stars + constellation lines
//!   + the constellation myth/season/key-star info on hover
//!
//! Per `docs/design/cosmos-architecture.md` this is the user-facing
//! surface for the position model. Eventually it'll auto-pick the view
//! based on the player's `PositionInUniverse.container`; for now it
//! has a manual toggle since the position-driven render bridge ships
//! in a later phase.
//!
//! Operator 2026-05-10: "We at least want our galaxy and solar system
//! ... see the real stars and real constellations." This page reads
//! from the existing data files (constellations.json, stars-nearby.json,
//! stars-catalog.json, sol.json) — all of which already ship embedded.
//! The 119k full HYG catalog (data/stars.csv) is used by the 3D skybox
//! renderer; for 2D map purposes the embedded ~300 brightest + nearby
//! is plenty without the overhead.

use egui::{Align2, Color32, Frame, Pos2, Rect, RichText, Rounding, ScrollArea, Sense, Stroke, Vec2};
use std::sync::OnceLock;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

// ─────────────────────── Data layer (lazy-loaded caches) ────────────────────

/// One nearby star in 3D galactic coordinates (light-years from Sol).
#[derive(Debug, Clone)]
struct NearbyStar {
    name: String,
    /// 3D position in light-years from Sol.
    pos_ly: glam::DVec3,
    spectral: String,
    apparent_magnitude: f64,
    distance_ly: f64,
    /// Alternate / common name, may equal `name`.
    alt_name: String,
}

/// One bright catalog star in equatorial (RA / Dec) coordinates.
#[derive(Debug, Clone)]
struct BrightStar {
    name: String,
    /// Right Ascension in hours (0-24).
    ra_hours: f64,
    /// Declination in degrees (-90 to +90).
    dec_deg: f64,
    /// Apparent magnitude (lower = brighter).
    magnitude: f64,
    spectral: String,
}

/// One constellation with its line geometry.
#[derive(Debug, Clone)]
struct Constellation {
    name: String,
    abbr: String,
    /// Pairs of (star_name_a, star_name_b) — line endpoints.
    lines: Vec<(String, String)>,
    myth: String,
    season: String,
    key_stars: Vec<String>,
    objects: Vec<String>,
}

/// One Sol-system body for the system view. Snapshot positions until
/// real orbital math arrives in a later phase.
#[derive(Debug, Clone)]
struct SolBody {
    id: String,
    name: String,
    body_type: String,
    /// Parent body id (e.g. "sun" for planets, "earth" for moon).
    parent: Option<String>,
    /// Semi-major axis in AU (orbital distance).
    semi_major_axis_au: f64,
    /// Body radius in km — for visual sizing.
    radius_km: f64,
}

static NEARBY_STARS: OnceLock<Vec<NearbyStar>> = OnceLock::new();
static BRIGHT_STARS: OnceLock<Vec<BrightStar>> = OnceLock::new();
static CONSTELLATIONS: OnceLock<Vec<Constellation>> = OnceLock::new();
static SOL_BODIES: OnceLock<Vec<SolBody>> = OnceLock::new();

fn nearby_stars() -> &'static [NearbyStar] {
    NEARBY_STARS.get_or_init(|| {
        let json = crate::embedded_data::STARS_NEARBY_JSON;
        // Schema per data/stars-nearby.json:
        //   [name, x_ly, y_ly, z_ly, spectral, apparent_mag, distance_ly, alt_name]
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
        let mut out = Vec::new();
        if let Some(arr) = parsed.as_array() {
            for row in arr {
                let r = match row.as_array() { Some(r) => r, None => continue };
                if r.len() < 7 { continue; }
                let name = r[0].as_str().unwrap_or("").to_string();
                let x = r[1].as_f64().unwrap_or(0.0);
                let y = r[2].as_f64().unwrap_or(0.0);
                let z = r[3].as_f64().unwrap_or(0.0);
                let spec = r[4].as_str().unwrap_or("").to_string();
                let am = r[5].as_f64().unwrap_or(99.0);
                let dist = r[6].as_f64().unwrap_or(0.0);
                let alt = r.get(7).and_then(|v| v.as_str()).unwrap_or(&name).to_string();
                out.push(NearbyStar {
                    name,
                    pos_ly: glam::DVec3::new(x, y, z),
                    spectral: spec,
                    apparent_magnitude: am,
                    distance_ly: dist,
                    alt_name: alt,
                });
            }
        }
        log::info!("Cosmos: loaded {} nearby stars", out.len());
        out
    })
}

fn bright_stars() -> &'static [BrightStar] {
    BRIGHT_STARS.get_or_init(|| {
        let json = crate::embedded_data::STARS_CATALOG_JSON;
        // Schema per data/stars-catalog.json:
        //   [name, ra_hours, dec_deg, magnitude, spectral]
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
        let mut out = Vec::new();
        if let Some(arr) = parsed.as_array() {
            for row in arr {
                let r = match row.as_array() { Some(r) => r, None => continue };
                if r.len() < 5 { continue; }
                out.push(BrightStar {
                    name: r[0].as_str().unwrap_or("").to_string(),
                    ra_hours: r[1].as_f64().unwrap_or(0.0),
                    dec_deg: r[2].as_f64().unwrap_or(0.0),
                    magnitude: r[3].as_f64().unwrap_or(99.0),
                    spectral: r[4].as_str().unwrap_or("").to_string(),
                });
            }
        }
        log::info!("Cosmos: loaded {} bright catalog stars", out.len());
        out
    })
}

fn constellations() -> &'static [Constellation] {
    CONSTELLATIONS.get_or_init(|| {
        let json = crate::embedded_data::CONSTELLATIONS_JSON;
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
        let mut out = Vec::new();
        if let Some(arr) = parsed.as_array() {
            for c in arr {
                let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let abbr = c.get("abbr").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let lines: Vec<(String, String)> = c.get("lines")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|pair| {
                        let p = pair.as_array()?;
                        Some((p.get(0)?.as_str()?.to_string(), p.get(1)?.as_str()?.to_string()))
                    }).collect())
                    .unwrap_or_default();
                let myth = c.get("myth").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let season = c.get("season").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let key_stars: Vec<String> = c.get("keyStars")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                let objects: Vec<String> = c.get("objects")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                out.push(Constellation { name, abbr, lines, myth, season, key_stars, objects });
            }
        }
        log::info!("Cosmos: loaded {} constellations", out.len());
        out
    })
}

fn sol_bodies() -> &'static [SolBody] {
    SOL_BODIES.get_or_init(|| {
        let json = crate::embedded_data::SOLAR_SYSTEM_JSON;
        let parsed: serde_json::Value = serde_json::from_str(json).unwrap_or(serde_json::Value::Null);
        let mut out = Vec::new();
        if let Some(arr) = parsed.get("bodies").and_then(|b| b.as_array()) {
            for body in arr {
                let id = body.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let name = body.get("name").and_then(|v| v.as_str()).unwrap_or(&id).to_string();
                let body_type = body.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if body_type == "region" { continue; } // skip belts as positionable bodies
                let parent = body.get("parent").and_then(|v| v.as_str()).map(String::from);
                let orbit = body.get("orbit");
                let semi_major_axis_au = orbit
                    .and_then(|o| o.get("semi_major_axis_au"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0);
                let radius_km = body.get("physical")
                    .and_then(|p| p.get("radius_km"))
                    .and_then(|v| v.as_f64())
                    .unwrap_or(1000.0);
                out.push(SolBody { id, name, body_type, parent, semi_major_axis_au, radius_km });
            }
        }
        log::info!("Cosmos: loaded {} Sol bodies", out.len());
        out
    })
}

// ─────────────────────── Spectral-class → color ─────────────────────────────

/// Map a spectral class letter to an approximate display color.
/// Hot blue (O/B) → white (A/F) → yellow (G) → orange (K) → red (M).
/// All RGB values are real astrophysical color approximations
/// (Mitchell Charity / NASA-published spectral type → RGB conversions),
/// not themable styling. Marked theme-exempt per-line.
fn spectral_color(class: &str) -> Color32 {
    let c = class.chars().next().unwrap_or('?');
    match c {
        'O' => Color32::from_rgb(155, 176, 255), // theme-exempt: spectral class O (hot blue) — physics, not theme
        'B' => Color32::from_rgb(170, 191, 255), // theme-exempt: spectral class B — physics, not theme
        'A' => Color32::from_rgb(202, 215, 255), // theme-exempt: spectral class A (white) — physics, not theme
        'F' => Color32::from_rgb(248, 247, 255), // theme-exempt: spectral class F — physics, not theme
        'G' => Color32::from_rgb(255, 244, 234), // theme-exempt: spectral class G (sun-like) — physics, not theme
        'K' => Color32::from_rgb(255, 210, 161), // theme-exempt: spectral class K (orange) — physics, not theme
        'M' => Color32::from_rgb(255, 180, 130), // theme-exempt: spectral class M (red dwarf) — physics, not theme
        _   => Color32::from_rgb(200, 200, 200), // theme-exempt: unknown spectral class fallback — physics, not theme
    }
}

/// Map a Sol body name to its display color. Approximations of real
/// imagery (Mars red, Jupiter banded, Earth blue, etc.). Domain data,
/// not theme tokens — the Sun stays yellow regardless of UI theme.
fn body_color(name: &str) -> Color32 {
    match name.to_lowercase().as_str() {
        "sun" => Color32::from_rgb(255, 220, 100),     // theme-exempt: real Sun color — astrophysics, not theme
        "mercury" => Color32::from_rgb(160, 130, 100), // theme-exempt: real Mercury color — astrophysics, not theme
        "venus" => Color32::from_rgb(225, 200, 140),   // theme-exempt: real Venus color — astrophysics, not theme
        "earth" => Color32::from_rgb(80, 140, 220),    // theme-exempt: real Earth color — astrophysics, not theme
        "mars" => Color32::from_rgb(200, 90, 60),      // theme-exempt: real Mars color — astrophysics, not theme
        "jupiter" => Color32::from_rgb(220, 180, 140), // theme-exempt: real Jupiter color — astrophysics, not theme
        "saturn" => Color32::from_rgb(220, 200, 150),  // theme-exempt: real Saturn color — astrophysics, not theme
        "uranus" => Color32::from_rgb(170, 220, 230),  // theme-exempt: real Uranus color — astrophysics, not theme
        "neptune" => Color32::from_rgb(80, 130, 220),  // theme-exempt: real Neptune color — astrophysics, not theme
        "pluto" => Color32::from_rgb(180, 160, 140),   // theme-exempt: real Pluto color — astrophysics, not theme
        _ => Color32::from_rgb(180, 180, 200),         // theme-exempt: unknown body color fallback — astrophysics, not theme
    }
}

// ─────────────────────── Page entry point ───────────────────────────────────

/// View mode — operator selectable via tab bar at the top of the page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CosmosView {
    /// Top-down 2D map of Sol — Sun + planets at fixed snapshot positions.
    System,
    /// Top-down 2D map of stars within ~50 ly of Sol (light-year scale).
    Galactic,
    /// Earth-centered celestial sphere (RA / Dec) with constellation lines.
    NightSky,
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(Color32::from_rgb(8, 8, 14)).inner_margin(0.0))  // theme-exempt: deep-space backdrop — domain aesthetic, not theme
        .show(ctx, |ui| {
            // Header bar with view tabs + scale info.
            ui.horizontal(|ui| {
                ui.add_space(theme.spacing_md);
                ui.label(
                    RichText::new("Cosmos")
                        .size(theme.font_size_heading)
                        .color(theme.text_primary())
                        .strong(),
                );
                ui.add_space(theme.spacing_lg);
                view_tab(ui, theme, state, CosmosView::System,    "System",          "Sol's Sun + planets, AU scale.");
                view_tab(ui, theme, state, CosmosView::Galactic,  "Galactic",        "Sol-centered map of nearby stars, light-year scale.");
                view_tab(ui, theme, state, CosmosView::NightSky,  "Night Sky",       "Earth-centered celestial sphere with constellation lines.");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(theme.spacing_md);
                    ui.label(
                        RichText::new("Tip: scroll to zoom, click-drag to pan, hover stars for details.")
                            .size(theme.font_size_small)
                            .color(theme.text_muted())
                            .italics(),
                    );
                });
            });
            ui.separator();

            match state.cosmos_view {
                CosmosView::System    => draw_system_view(ui, theme, state),
                CosmosView::Galactic  => draw_galactic_view(ui, theme, state),
                CosmosView::NightSky  => draw_night_sky_view(ui, theme, state),
            }
        });
}

fn view_tab(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &mut GuiState,
    mode: CosmosView,
    label: &str,
    tooltip: &str,
) {
    let active = state.cosmos_view == mode;
    if widgets::Button::secondary(label).active(active).tooltip(tooltip).show(ui, theme) {
        state.cosmos_view = mode;
        // Reset zoom + pan on view change so the user lands at a sensible default.
        state.cosmos_zoom = 1.0;
        state.cosmos_pan = Vec2::ZERO;
    }
}

// ─────────────────────── Pan + zoom helper ──────────────────────────────────

/// Common pan/zoom interaction — returns `(rect, response, center, zoom)`
/// for the canvas region.
fn allocate_canvas(
    ui: &mut egui::Ui,
    state: &mut GuiState,
) -> (Rect, egui::Response, Pos2, f32) {
    let available = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());

    // Zoom — mouse wheel.
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll_delta != 0.0 && ui.rect_contains_pointer(rect) {
        state.cosmos_zoom = (state.cosmos_zoom + scroll_delta * 0.005).clamp(0.1, 50.0);
    }
    // Pan — click-drag.
    if response.dragged() {
        state.cosmos_pan += response.drag_delta();
    }
    let center = rect.center() + state.cosmos_pan;
    (rect, response, center, state.cosmos_zoom)
}

// ─────────────────────── System view ────────────────────────────────────────

fn draw_system_view(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let (rect, response, center, zoom) = allocate_canvas(ui, state);
    let paint = ui.painter_at(rect);
    paint.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(8, 8, 14));  // theme-exempt: deep-space backdrop — domain aesthetic
    let bodies = sol_bodies();
    let max_au = 35.0_f64; // Pluto-ish range
    let scale = ((rect.width().min(rect.height()) as f64 / 2.0 - 20.0) / max_au) * zoom as f64;

    // Sun at center.
    paint.circle_filled(center, (8.0 * zoom).clamp(4.0, 30.0), body_color("sun"));
    paint.text(
        center + Vec2::new(0.0, -((8.0 * zoom).clamp(4.0, 30.0) + 8.0)),
        Align2::CENTER_BOTTOM,
        "Sun",
        egui::FontId::proportional(11.0),
        theme.text_primary(),
    );

    let hover_pos = ui.input(|i| i.pointer.hover_pos());
    let mut hovered_body: Option<&SolBody> = None;

    // Planets on circular orbits (snapshot positions — angle based on body
    // index for now; swap in real Kepler positions in a later phase).
    for body in bodies.iter() {
        if body.parent.as_deref() != Some("sun") || body.semi_major_axis_au <= 0.0 {
            continue;
        }
        let orbit_r = (body.semi_major_axis_au * scale) as f32;
        if orbit_r < 3.0 || orbit_r > rect.width() {
            continue;
        }
        // Faint orbit ring.
        paint.circle_stroke(center, orbit_r, Stroke::new(0.8, Color32::from_rgb(40, 40, 60)));  // theme-exempt: orbital ring — faint backdrop element
        // Snapshot angle (deterministic: hash of name → angle). Looks
        // static across frames, which is fine for a chart UI; live orbital
        // motion ships in Phase 5 alongside sim_time gossip.
        let angle = body.name.bytes().fold(0u64, |a, b| a.wrapping_add(b as u64)) as f32 * 0.137;
        let px = center.x + orbit_r * angle.cos();
        let py = center.y + orbit_r * angle.sin();
        let pos = Pos2::new(px, py);

        let r = if body.radius_km > 30000.0 { (5.0 * zoom).clamp(3.0, 14.0) }
                else if body.radius_km > 5000.0 { (3.5 * zoom).clamp(2.5, 9.0) }
                else { (2.5 * zoom).clamp(2.0, 6.0) };
        paint.circle_filled(pos, r, body_color(&body.name));

        // Hover detection.
        if let Some(hp) = hover_pos {
            if (hp - pos).length() < r + 4.0 {
                hovered_body = Some(body);
            }
        }

        // Label only at decent zoom to avoid clutter.
        if zoom > 0.6 {
            paint.text(
                pos + Vec2::new(0.0, -(r + 4.0)),
                Align2::CENTER_BOTTOM,
                &body.name,
                egui::FontId::proportional(10.0),
                theme.text_secondary(),
            );
        }
    }

    // Hover tooltip for the body the cursor is over.
    if let (Some(body), Some(_)) = (hovered_body, hover_pos) {
        response.on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(280.0);
            ui.label(RichText::new(&body.name).size(theme.font_size_body).color(theme.text_primary()).strong());
            ui.label(RichText::new(format!("{}  ·  {} AU from Sun", titlecase(&body.body_type), body.semi_major_axis_au))
                .size(theme.font_size_small).color(theme.text_secondary()));
            ui.label(RichText::new(format!("Radius: {} km", format_with_commas(body.radius_km as i64)))
                .size(theme.font_size_small).color(theme.text_muted()));
        });
    }
}

// ─────────────────────── Galactic view (Sol-centered, ly) ───────────────────

fn draw_galactic_view(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let (rect, response, center, zoom) = allocate_canvas(ui, state);
    let paint = ui.painter_at(rect);
    paint.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(5, 5, 12));  // theme-exempt: deep-space backdrop
    let stars = nearby_stars();
    // Auto-fit: the most distant star in the dataset sets the base scale.
    let max_dist = stars.iter().map(|s| s.distance_ly).fold(15.0, f64::max).max(15.0);
    let scale = ((rect.width().min(rect.height()) as f64 / 2.0 - 30.0) / max_dist) * zoom as f64;

    // Faint grid rings at 5 / 10 / 25 / 50 ly for visual reference.
    for &ring_ly in &[5.0_f64, 10.0, 25.0, 50.0] {
        let r = (ring_ly * scale) as f32;
        if r > 5.0 && r < rect.width().max(rect.height()) {
            paint.circle_stroke(center, r, Stroke::new(0.5, Color32::from_rgb(25, 25, 40)));  // theme-exempt: distance-ring — faint backdrop
            paint.text(
                center + Vec2::new(r * 0.7, -r * 0.7),
                Align2::CENTER_CENTER,
                format!("{} ly", ring_ly as i64),
                egui::FontId::proportional(9.0),
                Color32::from_rgb(80, 80, 110),  // theme-exempt: distance-ring label — faint backdrop
            );
        }
    }

    // Sol at center — the universal anchor.
    paint.circle_filled(center, 4.0_f32.max(2.0 * zoom), body_color("sun"));
    paint.text(
        center + Vec2::new(8.0, 0.0),
        Align2::LEFT_CENTER,
        "Sol",
        egui::FontId::proportional(11.0),
        theme.text_primary(),
    );

    // Nearby stars projected to the X-Y galactic plane (Z dropped).
    let hover_pos = ui.input(|i| i.pointer.hover_pos());
    let mut hovered_star: Option<&NearbyStar> = None;
    for star in stars {
        let px = center.x + (star.pos_ly.x * scale) as f32;
        let py = center.y - (star.pos_ly.y * scale) as f32; // y inverted (screen y grows down)
        let pos = Pos2::new(px, py);
        // Brighter (lower mag) → larger dot.
        let r = ((6.0 - star.apparent_magnitude.min(6.0)) as f32 * 0.8 + 1.5).clamp(1.5, 6.0);
        paint.circle_filled(pos, r, spectral_color(&star.spectral));
        if zoom > 1.5 {
            paint.text(
                pos + Vec2::new(r + 2.0, 0.0),
                Align2::LEFT_CENTER,
                &star.name,
                egui::FontId::proportional(9.0),
                theme.text_secondary(),
            );
        }
        if let Some(hp) = hover_pos {
            if (hp - pos).length() < r + 4.0 {
                hovered_star = Some(star);
            }
        }
    }

    if let Some(star) = hovered_star {
        response.on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(300.0);
            ui.label(RichText::new(&star.name).size(theme.font_size_body).color(theme.text_primary()).strong());
            if star.alt_name != star.name {
                ui.label(RichText::new(format!("aka {}", &star.alt_name))
                    .size(theme.font_size_small).color(theme.text_secondary()).italics());
            }
            ui.label(RichText::new(format!("{:.2} ly from Sol  ·  spectral type {}", star.distance_ly, star.spectral))
                .size(theme.font_size_small).color(theme.text_secondary()));
            ui.label(RichText::new(format!("Apparent magnitude {:.2}", star.apparent_magnitude))
                .size(theme.font_size_small).color(theme.text_muted()));
            ui.label(RichText::new(format!("Galactic position: ({:.3}, {:.3}, {:.3}) ly", star.pos_ly.x, star.pos_ly.y, star.pos_ly.z))
                .size(theme.font_size_small).color(theme.text_muted()).monospace());
        });
    }

    // Footer hint.
    paint.text(
        Pos2::new(rect.left() + 8.0, rect.bottom() - 8.0),
        Align2::LEFT_BOTTOM,
        format!("Top-down galactic plane · {} stars within ~{:.0} ly · X/Y plane, Z dropped", stars.len(), max_dist),
        egui::FontId::proportional(10.0),
        theme.text_muted(),
    );
}

// ─────────────────────── Night Sky view (RA/Dec, Earth-centered) ────────────

fn draw_night_sky_view(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let (rect, response, _center, zoom) = allocate_canvas(ui, state);
    let paint = ui.painter_at(rect);
    paint.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(5, 5, 12));  // theme-exempt: deep-space backdrop
    // Equirectangular projection of the celestial sphere:
    //   RA  in [0, 24) hours → x in [left, right]
    //   Dec in [-90, +90]    → y in [bottom, top]
    let inner = rect.shrink(20.0);
    let project = |ra: f64, dec: f64| -> Pos2 {
        let xnorm = (ra / 24.0).rem_euclid(1.0);
        let ynorm = ((90.0 - dec) / 180.0).clamp(0.0, 1.0);
        // Apply zoom + pan (zoom is centered on the visible center).
        let x = inner.left() + (xnorm as f32) * inner.width();
        let y = inner.top() + (ynorm as f32) * inner.height();
        let from_center = Pos2::new(x, y) - inner.center();
        inner.center() + from_center * zoom + state.cosmos_pan
    };

    // RA / Dec grid every 2 hours / 30 deg.
    for ra in (0..12).map(|i| i as f64 * 2.0) {
        let p1 = project(ra, -90.0);
        let p2 = project(ra, 90.0);
        paint.line_segment([p1, p2], Stroke::new(0.5, Color32::from_rgb(20, 20, 35)));  // theme-exempt: celestial grid line — faint backdrop
    }
    for dec in (-3..=3).map(|i| i as f64 * 30.0) {
        let p1 = project(0.0, dec);
        let p2 = project(24.0, dec);
        paint.line_segment([p1, p2], Stroke::new(0.5, Color32::from_rgb(20, 20, 35)));  // theme-exempt: celestial grid line — faint backdrop
        paint.text(
            project(0.0, dec) + Vec2::new(4.0, 0.0),
            Align2::LEFT_CENTER,
            format!("{:+}°", dec as i32),
            egui::FontId::proportional(8.0),
            Color32::from_rgb(70, 70, 95),  // theme-exempt: declination-axis label — faint backdrop
        );
    }

    // Index bright stars by name for constellation lookup.
    let stars = bright_stars();
    let star_by_name: std::collections::HashMap<&str, &BrightStar> =
        stars.iter().map(|s| (s.name.as_str(), s)).collect();

    // Constellation lines first (drawn under the stars).
    let const_color = Color32::from_rgb(60, 80, 130);  // theme-exempt: constellation line — faint backdrop element
    let cons = constellations();
    let hover_pos = ui.input(|i| i.pointer.hover_pos());
    let mut hovered_constellation: Option<&Constellation> = None;

    for c in cons {
        for (a, b) in &c.lines {
            if let (Some(sa), Some(sb)) = (star_by_name.get(a.as_str()), star_by_name.get(b.as_str())) {
                let pa = project(sa.ra_hours, sa.dec_deg);
                let pb = project(sb.ra_hours, sb.dec_deg);
                paint.line_segment([pa, pb], Stroke::new(1.0, const_color));
                // Label hover detection: did the cursor pass near this line?
                if let Some(hp) = hover_pos {
                    if dist_point_to_segment(hp, pa, pb) < 6.0 {
                        hovered_constellation = Some(c);
                    }
                }
            }
        }
        // Constellation name label at the centroid of its stars.
        let pts: Vec<Pos2> = c.lines.iter()
            .filter_map(|(a, b)| Some([star_by_name.get(a.as_str())?, star_by_name.get(b.as_str())?]))
            .flat_map(|pair| pair.iter().map(|s| project(s.ra_hours, s.dec_deg)).collect::<Vec<_>>())
            .collect();
        if !pts.is_empty() {
            let avg = pts.iter().fold(Vec2::ZERO, |acc, p| acc + p.to_vec2()) / pts.len() as f32;
            paint.text(
                avg.to_pos2(),
                Align2::CENTER_CENTER,
                &c.abbr,
                egui::FontId::proportional(10.0),
                Color32::from_rgb(110, 130, 180),  // theme-exempt: constellation abbreviation label
            );
        }
    }

    // Stars on top.
    let mut hovered_star: Option<&BrightStar> = None;
    for s in stars {
        let p = project(s.ra_hours, s.dec_deg);
        let r = ((4.5 - s.magnitude.min(4.5)) as f32 * 0.7 + 1.0).clamp(1.0, 4.5);
        paint.circle_filled(p, r, spectral_color(&s.spectral));
        if let Some(hp) = hover_pos {
            if (hp - p).length() < r + 4.0 {
                hovered_star = Some(s);
            }
        }
    }

    // Hover priority: a star tooltip beats a constellation tooltip if both fire.
    if let Some(s) = hovered_star {
        response.on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(280.0);
            ui.label(RichText::new(&s.name).size(theme.font_size_body).color(theme.text_primary()).strong());
            ui.label(RichText::new(format!("RA {:.3}h  ·  Dec {:+.2}°", s.ra_hours, s.dec_deg))
                .size(theme.font_size_small).color(theme.text_secondary()).monospace());
            ui.label(RichText::new(format!("Magnitude {:.2}  ·  Spectral {}", s.magnitude, s.spectral))
                .size(theme.font_size_small).color(theme.text_muted()));
        });
    } else if let Some(c) = hovered_constellation {
        response.on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(360.0);
            ui.label(RichText::new(&c.name).size(theme.font_size_heading).color(theme.text_primary()).strong());
            ui.label(RichText::new(format!("{}  ·  {}", &c.abbr, &c.season))
                .size(theme.font_size_small).color(theme.text_secondary()));
            if !c.myth.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new(&c.myth).size(theme.font_size_small).color(theme.text_secondary()));
            }
            if !c.key_stars.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new("Key stars:").size(theme.font_size_small).color(theme.text_primary()).strong());
                for ks in &c.key_stars {
                    ui.label(RichText::new(format!("  • {}", ks)).size(theme.font_size_small).color(theme.text_muted()));
                }
            }
            if !c.objects.is_empty() {
                ui.add_space(6.0);
                ui.label(RichText::new("Notable objects:").size(theme.font_size_small).color(theme.text_primary()).strong());
                for o in &c.objects {
                    ui.label(RichText::new(format!("  • {}", o)).size(theme.font_size_small).color(theme.text_muted()));
                }
            }
        });
    }

    // Footer hint.
    paint.text(
        Pos2::new(rect.left() + 8.0, rect.bottom() - 8.0),
        Align2::LEFT_BOTTOM,
        format!("Equirectangular celestial sphere · {} bright stars · {} constellations", stars.len(), cons.len()),
        egui::FontId::proportional(10.0),
        theme.text_muted(),
    );
}

// ─────────────────────── Helpers ────────────────────────────────────────────

fn titlecase(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().chain(chars).collect(),
    }
}

fn format_with_commas(n: i64) -> String {
    let s = n.abs().to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { out.push(','); }
        out.push(ch);
    }
    if n < 0 { out.push('-'); }
    out.chars().rev().collect()
}

/// Perpendicular distance from `p` to the line segment `a → b`.
fn dist_point_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    if len_sq < 1e-6 { return (p - a).length(); }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let closest = a + ab * t;
    (p - closest).length()
}
