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

/// One Sol-system body for the system view + body browser sidebar +
/// details panel. Phase 3 (v0.203.2): expanded with the fields needed
/// for the right-side details panel (radius, mass, gravity, atmosphere
/// composition, orbital period, mean temperature, discovery info,
/// description).
#[derive(Debug, Clone)]
struct SolBody {
    id: String,
    name: String,
    body_type: String,
    /// Parent body id (e.g. "sun" for planets, "earth" for "moon").
    parent: Option<String>,
    /// Semi-major axis in AU (only set for direct sun-orbiters).
    semi_major_axis_au: f64,
    /// Semi-major axis in km (only set for moons orbiting their planet).
    semi_major_axis_km: f64,
    /// Body radius in km — for visual sizing.
    radius_km: f64,
    /// Mass in kg.
    mass_kg: f64,
    /// Surface gravity in m/s².
    surface_gravity_ms2: f64,
    /// Mean surface / cloud-top temperature in Kelvin.
    mean_temperature_k: f64,
    /// Orbital period in days.
    orbital_period_days: f64,
    /// Atmosphere composition summary (top 3 components, formatted).
    /// Empty string if no atmosphere.
    atmosphere_summary: String,
    /// Free-form description (1-2 sentences).
    description: String,
    /// Discovery year, if known. 0 = ancient / no record.
    discovery_year: i32,
    /// Discoverer name, if known.
    discoverer: String,
    /// IDs of bodies orbiting this one (e.g. moons of a planet).
    children: Vec<String>,
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
        let mut out: Vec<SolBody> = Vec::new();
        if let Some(arr) = parsed.get("bodies").and_then(|b| b.as_array()) {
            for body in arr {
                let id = body.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let name = body.get("name").and_then(|v| v.as_str()).unwrap_or(&id).to_string();
                let body_type = body.get("type").and_then(|v| v.as_str()).unwrap_or("").to_string();
                if body_type == "region" { continue; } // skip belts as positionable bodies
                let parent = body.get("parent").and_then(|v| v.as_str()).map(String::from);
                let orbit = body.get("orbit");
                let semi_major_axis_au = orbit.and_then(|o| o.get("semi_major_axis_au")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let semi_major_axis_km = orbit.and_then(|o| o.get("semi_major_axis_km")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let orbital_period_days = orbit.and_then(|o| o.get("orbital_period_days")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let physical = body.get("physical");
                let radius_km = physical.and_then(|p| p.get("radius_km")).and_then(|v| v.as_f64()).unwrap_or(1000.0);
                let mass_kg = physical.and_then(|p| p.get("mass_kg")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let surface_gravity_ms2 = physical.and_then(|p| p.get("surface_gravity_ms2")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                let mean_temperature_k = physical.and_then(|p| p.get("mean_temperature_k")).and_then(|v| v.as_f64()).unwrap_or(0.0);
                // Build a compact atmosphere summary from the composition map
                // ("78% N₂ · 21% O₂ · …"). Empty string if no atmosphere.
                let atmosphere_summary = body.get("atmosphere")
                    .and_then(|a| a.get("composition"))
                    .and_then(|c| c.as_object())
                    .map(|comp| {
                        let mut pairs: Vec<(String, f64)> = comp.iter()
                            .filter_map(|(k, v)| Some((k.clone(), v.as_f64()?)))
                            .collect();
                        // Highest concentration first.
                        pairs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                        pairs.iter().take(3)
                            .map(|(k, v)| format!("{:.1}% {}", v, k))
                            .collect::<Vec<_>>()
                            .join(" · ")
                    })
                    .unwrap_or_default();
                let description = body.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let (discovery_year, discoverer) = body.get("discovery")
                    .and_then(|d| {
                        let y = d.get("year").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                        let who = d.get("discoverer").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        Some((y, who))
                    })
                    .unwrap_or((0, String::new()));
                out.push(SolBody {
                    id, name, body_type, parent,
                    semi_major_axis_au, semi_major_axis_km, orbital_period_days,
                    radius_km, mass_kg, surface_gravity_ms2, mean_temperature_k,
                    atmosphere_summary, description, discovery_year, discoverer,
                    children: Vec::new(), // populated in second pass below
                });
            }
        }
        // Second pass: populate `children` lists by inverting the parent
        // relationship. This is what lets the body browser sidebar nest
        // moons under their planet without re-scanning every frame.
        let mut child_lists: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
        for b in &out {
            if let Some(p) = &b.parent {
                child_lists.entry(p.clone()).or_default().push(b.id.clone());
            }
        }
        for b in &mut out {
            if let Some(kids) = child_lists.get(&b.id) {
                b.children = kids.clone();
            }
        }
        log::info!("Cosmos: loaded {} Sol bodies (with parent-child links)", out.len());
        out
    })
}

/// Look up a body by id. O(N) scan, but the dataset is ~64 entries so
/// it's fine to call this from per-frame UI code.
fn find_body(id: &str) -> Option<&'static SolBody> {
    sol_bodies().iter().find(|b| b.id == id)
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
    // v0.203.2: System view gets left + right side panels (browser + details);
    // Galactic and Night Sky stay full-width since they're a single canvas
    // and don't have a discrete object hierarchy worth browsing.
    let in_system_view = state.cosmos_view == CosmosView::System;

    if in_system_view {
        // Left: collapsible body browser tree.
        egui::SidePanel::left("cosmos_body_browser")
            .resizable(true)
            .min_width(220.0)
            .max_width(360.0)
            .default_width(260.0)
            .frame(Frame::NONE.fill(theme.bg_panel()).inner_margin(theme.spacing_sm))
            .show(ctx, |ui| {
                draw_body_browser(ui, theme, state);
            });

        // Right: details for the selected body.
        egui::SidePanel::right("cosmos_body_details")
            .resizable(true)
            .min_width(260.0)
            .max_width(420.0)
            .default_width(300.0)
            .frame(Frame::NONE.fill(theme.bg_panel()).inner_margin(theme.spacing_md))
            .show(ctx, |ui| {
                draw_body_details(ui, theme, state);
            });
    }

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
                view_tab(ui, theme, state, CosmosView::System,    "System",          "Sol's Sun + planets + moons, AU scale (top-down 2D).");
                view_tab(ui, theme, state, CosmosView::Galactic,  "Galactic",        "Sol-centered map of nearby stars, light-year scale.");
                view_tab(ui, theme, state, CosmosView::NightSky,  "Night Sky",       "Earth-centered celestial sphere with constellation lines.");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(theme.spacing_md);
                    ui.label(
                        RichText::new("Scroll to zoom · click-drag to pan · 2D top-down (3D rotation in Phase 4).")
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

// ─────────────────────── Body browser (left sidebar) ────────────────────────

/// Region groups for the body browser. Each variant holds the body ids
/// that fall in that region; population happens lazily.
fn body_regions() -> Vec<(&'static str, Vec<&'static SolBody>)> {
    let bodies = sol_bodies();
    let mut star = Vec::new();
    let mut inner = Vec::new(); // terrestrial planets orbiting Sun
    let mut outer = Vec::new(); // gas/ice giants orbiting Sun
    let mut dwarfs = Vec::new(); // dwarf planets
    let mut asteroids = Vec::new(); // asteroids orbiting Sun directly

    for b in bodies {
        match (b.body_type.as_str(), b.parent.as_deref()) {
            ("star", _)                     => star.push(b),
            ("terrestrial", Some("sun"))    => inner.push(b),
            ("gas_giant",   Some("sun")) |
            ("ice_giant",   Some("sun"))    => outer.push(b),
            ("dwarf_planet", _)             => dwarfs.push(b),
            ("asteroid",    Some("sun"))    => asteroids.push(b),
            _ => {} // moons handled per-parent
        }
    }
    // Sort each list by AU distance for a natural inside-out order.
    let by_au = |a: &&SolBody, b: &&SolBody| a.semi_major_axis_au.partial_cmp(&b.semi_major_axis_au).unwrap_or(std::cmp::Ordering::Equal);
    inner.sort_by(by_au);
    outer.sort_by(by_au);
    dwarfs.sort_by(by_au);
    asteroids.sort_by(by_au);
    vec![
        ("Star",          star),
        ("Inner Planets", inner),
        ("Outer Planets", outer),
        ("Dwarf Planets", dwarfs),
        ("Asteroids",     asteroids),
    ]
}

fn draw_body_browser(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(
        RichText::new("Celestial Bodies")
            .size(theme.font_size_heading)
            .color(theme.text_primary())
            .strong(),
    );
    ui.label(
        RichText::new("Click a body to focus. Click a planet's ▸ to expand its moons.")
            .size(theme.font_size_small)
            .color(theme.text_muted()),
    );
    ui.add_space(theme.spacing_sm);

    ScrollArea::vertical().show(ui, |ui| {
        for (region_label, members) in body_regions() {
            if members.is_empty() { continue; }
            ui.label(
                RichText::new(region_label)
                    .size(theme.font_size_small)
                    .color(theme.accent())
                    .strong(),
            );
            for body in &members {
                draw_browser_row(ui, theme, state, body, /* depth */ 0);
                // For planets / dwarf planets, expand to show moons if requested.
                if !body.children.is_empty() && state.cosmos_expanded_planets.contains(&body.id) {
                    for moon_id in &body.children {
                        if let Some(moon) = find_body(moon_id) {
                            draw_browser_row(ui, theme, state, moon, /* depth */ 1);
                        }
                    }
                }
            }
            ui.add_space(theme.spacing_xs);
        }
    });
}

fn draw_browser_row(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, body: &SolBody, depth: usize) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.add_space(depth as f32 * 14.0);
        // Expand chevron only if the body has children (planets / dwarf planets with moons).
        if !body.children.is_empty() {
            let expanded = state.cosmos_expanded_planets.contains(&body.id);
            let chevron = if expanded { "▾" } else { "▸" };
            let resp = ui.add(egui::Label::new(
                RichText::new(chevron)
                    .size(theme.font_size_small)
                    .color(theme.text_muted())
                    .monospace(),
            ).sense(Sense::click()));
            if resp.clicked() {
                if expanded {
                    state.cosmos_expanded_planets.remove(&body.id);
                } else {
                    state.cosmos_expanded_planets.insert(body.id.clone());
                }
            }
        } else {
            ui.add_space(10.0);
        }
        // Color dot matching the body's display color.
        let (rect, _r) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, body_color(&body.name));
        // Name — clickable to select.
        let selected = state.cosmos_selected_body.as_deref() == Some(body.id.as_str());
        let label_color = if selected { theme.accent() } else { theme.text_primary() };
        let resp = ui.add(egui::Label::new(
            RichText::new(&body.name).size(theme.font_size_small).color(label_color),
        ).sense(Sense::click()));
        if resp.clicked() {
            state.cosmos_selected_body = Some(body.id.clone());
            // v0.205.0: sidebar clicks also focus the map. The user
            // explicitly said "show me X" by clicking it in the
            // browser — natural to follow up by centering on it.
            state.cosmos_focus_request = Some(body.id.clone());
        }
    });
}

// ─────────────────────── Body details (right sidebar) ───────────────────────

fn draw_body_details(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let body = match state.cosmos_selected_body.as_deref().and_then(find_body) {
        Some(b) => b,
        None => {
            ui.label(
                RichText::new("Select a body")
                    .size(theme.font_size_heading)
                    .color(theme.text_secondary()),
            );
            ui.label(
                RichText::new("Click any planet, moon, or dwarf planet in the left-side browser — its details appear here.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            return;
        }
    };

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(&body.name)
                .size(theme.font_size_title)
                .color(theme.text_primary())
                .strong(),
        );
        // v0.205.0: "Focus" button moves the map view to center on this
        // body. Especially useful when zooming with the cursor parked
        // elsewhere — one click re-centers without manual panning.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::Button::secondary("Focus")
                .tooltip("Center the map view on this body. Combine with scroll-wheel zoom to look closer.")
                .show(ui, theme)
            {
                state.cosmos_focus_request = Some(body.id.clone());
                ui.ctx().request_repaint();
            }
        });
    });
    ui.label(
        RichText::new(format!("{} · {}", titlecase(&body.body_type),
            body.parent.as_deref().map(|p| format!("orbits {}", titlecase(p))).unwrap_or_else(|| "—".to_string())))
            .size(theme.font_size_small)
            .color(theme.accent()),
    );
    ui.add_space(theme.spacing_sm);

    ScrollArea::vertical().show(ui, |ui| {
        // Physical properties.
        section_heading(ui, theme, "Physical");
        if body.radius_km > 0.0 {
            kv(ui, theme, "Radius", &format_km(body.radius_km));
        }
        if body.mass_kg > 0.0 {
            kv(ui, theme, "Mass", &format_mass(body.mass_kg));
        }
        if body.surface_gravity_ms2 > 0.0 {
            kv(ui, theme, "Surface gravity", &format!("{:.2} m/s² ({:.2} g)",
                body.surface_gravity_ms2, body.surface_gravity_ms2 / 9.81));
        }
        if body.mean_temperature_k > 0.0 {
            kv(ui, theme, "Mean temperature", &format_temperature(body.mean_temperature_k));
        }

        // Orbital properties.
        if body.semi_major_axis_au > 0.0 || body.semi_major_axis_km > 0.0 || body.orbital_period_days > 0.0 {
            ui.add_space(theme.spacing_sm);
            section_heading(ui, theme, "Orbit");
            if body.semi_major_axis_au > 0.0 {
                kv(ui, theme, "Semi-major axis", &format!("{:.3} AU", body.semi_major_axis_au));
            } else if body.semi_major_axis_km > 0.0 {
                kv(ui, theme, "Semi-major axis", &format_km(body.semi_major_axis_km));
            }
            if body.orbital_period_days > 0.0 {
                kv(ui, theme, "Orbital period", &format_period(body.orbital_period_days));
            }
        }

        // Atmosphere.
        if !body.atmosphere_summary.is_empty() {
            ui.add_space(theme.spacing_sm);
            section_heading(ui, theme, "Atmosphere");
            ui.label(
                RichText::new(&body.atmosphere_summary)
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
        } else {
            ui.add_space(theme.spacing_sm);
            section_heading(ui, theme, "Atmosphere");
            ui.label(
                RichText::new("None").size(theme.font_size_small).color(theme.text_muted()).italics(),
            );
        }

        // Discovery.
        if body.discovery_year > 0 || !body.discoverer.is_empty() {
            ui.add_space(theme.spacing_sm);
            section_heading(ui, theme, "Discovery");
            if body.discovery_year > 0 {
                kv(ui, theme, "Year", &body.discovery_year.to_string());
            }
            if !body.discoverer.is_empty() {
                kv(ui, theme, "Discoverer", &body.discoverer);
            }
        }

        // Description / flavor.
        if !body.description.is_empty() {
            ui.add_space(theme.spacing_md);
            ui.separator();
            ui.add_space(theme.spacing_xs);
            ui.label(
                RichText::new(&body.description)
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
        }

        // Children list (moons of a planet).
        if !body.children.is_empty() {
            ui.add_space(theme.spacing_md);
            section_heading(ui, theme, &format!("Moons ({})", body.children.len()));
            for moon_id in &body.children {
                if let Some(moon) = find_body(moon_id) {
                    let resp = ui.add(egui::Label::new(
                        RichText::new(format!("• {}", &moon.name))
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    ).sense(Sense::click()));
                    if resp.clicked() {
                        state.cosmos_selected_body = Some(moon.id.clone());
                        // v0.205.0: also focus on the moon (which falls
                        // back to its parent planet via the focus_request
                        // handler since moons live in the cosmetic ring).
                        state.cosmos_focus_request = Some(moon.id.clone());
                    }
                }
            }
        }
    });
}

fn section_heading(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.label(
        RichText::new(text)
            .size(theme.font_size_small)
            .color(theme.accent())
            .strong(),
    );
    ui.add_space(2.0);
}

fn kv(ui: &mut egui::Ui, theme: &Theme, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{}:", key)).size(theme.font_size_small).color(theme.text_muted()));
        ui.label(RichText::new(value).size(theme.font_size_small).color(theme.text_primary()));
    });
}

fn format_km(km: f64) -> String {
    if km >= 1.0e6 { format!("{:.2} million km", km / 1.0e6) }
    else if km >= 1.0e3 { format!("{} km", format_with_commas(km as i64)) }
    else { format!("{:.1} km", km) }
}

fn format_mass(kg: f64) -> String {
    if kg >= 1.0e24 { format!("{:.3e} kg ({:.2} Earth masses)", kg, kg / 5.972e24) }
    else if kg >= 1.0e20 { format!("{:.3e} kg", kg) }
    else { format!("{:.3e} kg", kg) }
}

fn format_temperature(k: f64) -> String {
    let celsius = k - 273.15;
    let fahrenheit = celsius * 9.0 / 5.0 + 32.0;
    format!("{:.0} K  ({:.0} °C / {:.0} °F)", k, celsius, fahrenheit)
}

fn format_period(days: f64) -> String {
    if days.abs() >= 365.0 { format!("{:.2} years ({:.0} days)", days / 365.25, days) }
    else if days.abs() >= 1.0 { format!("{:.2} days", days) }
    else { format!("{:.2} hours", days * 24.0) }
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
///
/// v0.205.0 fixes (operator pushback 2026-05-10):
///   1. **Multiplicative zoom** instead of additive. At zoom=50, additive
///      `+= 0.005 * scroll` was a 0.01% step per scroll tick — basically
///      invisible. Multiplicative `*= 1.05^ticks` gives the same percent
///      change at every zoom level, so scroll feels consistent from 0.1×
///      to 50×.
///   2. **Zoom toward cursor.** The point under the cursor stays anchored
///      during zoom. Without this, every zoom-in re-centers on whatever
///      is at canvas center (usually the Sun). Standard map-UI convention
///      (Google Maps, Blender, Photoshop, etc.) is to anchor on cursor.
fn allocate_canvas(
    ui: &mut egui::Ui,
    state: &mut GuiState,
) -> (Rect, egui::Response, Pos2, f32) {
    let available = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());

    // Pan — click-drag (apply BEFORE zoom so cursor-anchored zoom uses
    // the latest pan).
    if response.dragged() {
        state.cosmos_pan += response.drag_delta();
    }

    // Zoom — multiplicative + cursor-anchored.
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll_delta != 0.0 && ui.rect_contains_pointer(rect) {
        let cursor = ui.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center());

        // 1.0015 per scroll-pixel = ~1.08× per typical 50-px scroll tick.
        // Picked to feel responsive without being jumpy.
        let zoom_before = state.cosmos_zoom;
        let zoom_after = (zoom_before * (1.0015_f32).powf(scroll_delta)).clamp(0.05, 200.0);

        // Cursor-anchored: shift pan so the world point under the cursor
        // ends up at the same screen position after the zoom change. Math:
        //   cursor_offset = cursor - rect.center()  (cursor relative to canvas center)
        //   ratio = zoom_after / zoom_before
        //   new_pan = cursor_offset * (1 - ratio) + old_pan * ratio
        // Derivation in the cosmos doc / commit message.
        let ratio = zoom_after / zoom_before;
        let cursor_offset = cursor - rect.center();
        state.cosmos_pan = cursor_offset * (1.0 - ratio) + state.cosmos_pan * ratio;
        state.cosmos_zoom = zoom_after;
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
    let sun_r = (8.0 * zoom).clamp(4.0, 30.0);
    paint.circle_filled(center, sun_r, body_color("sun"));
    paint.text(
        center + Vec2::new(0.0, -(sun_r + 8.0)),
        Align2::CENTER_BOTTOM,
        "Sun",
        egui::FontId::proportional(11.0),
        theme.text_primary(),
    );

    let hover_pos = ui.input(|i| i.pointer.hover_pos());
    let mut hovered_body: Option<&SolBody> = None;
    let mut clicked_body: Option<&SolBody> = None;

    // Track each planet's screen position so moons + ship icons + future
    // overlays can position relative to them without recomputing.
    let mut planet_positions: std::collections::HashMap<String, Pos2> = std::collections::HashMap::new();

    // First pass: planets + dwarf planets + asteroids that orbit Sun directly.
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
        // motion ships in a later phase alongside sim_time gossip.
        let angle = body.name.bytes().fold(0u64, |a, b| a.wrapping_add(b as u64)) as f32 * 0.137;
        let px = center.x + orbit_r * angle.cos();
        let py = center.y + orbit_r * angle.sin();
        let pos = Pos2::new(px, py);
        planet_positions.insert(body.id.clone(), pos);

        let r = if body.radius_km > 30000.0 { (5.0 * zoom).clamp(3.0, 14.0) }
                else if body.radius_km > 5000.0 { (3.5 * zoom).clamp(2.5, 9.0) }
                else { (2.5 * zoom).clamp(2.0, 6.0) };
        paint.circle_filled(pos, r, body_color(&body.name));

        // Highlight ring for the selected body.
        if state.cosmos_selected_body.as_deref() == Some(body.id.as_str()) {
            paint.circle_stroke(pos, r + 3.0, Stroke::new(1.5, theme.accent()));
        }

        if let Some(hp) = hover_pos {
            if (hp - pos).length() < r + 4.0 {
                hovered_body = Some(body);
                if response.clicked() {
                    clicked_body = Some(body);
                }
            }
        }
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

    // Second pass: moons. Rendered in a small ring AROUND their parent
    // planet's screen position. The ring radius is purely cosmetic
    // (planets-and-moons-to-scale would have moons sub-pixel close to
    // their planet); offsetting them by ~14-22 px makes them clickable
    // without needing to zoom into a single planet. Operator note
    // 2026-05-10: this addresses "what if a moon is directly underneath
    // the planet at the southern pole" in 2D top-down — moons are NEVER
    // rendered overlapping their parent here; they always get a visible
    // ring slot. Real 3D rotation will replace this when wgpu-in-egui
    // integration ships in Phase 4.
    for body in bodies.iter() {
        if body.body_type != "moon" { continue; }
        let parent_id = match &body.parent { Some(p) => p, None => continue };
        let parent_pos = match planet_positions.get(parent_id) { Some(p) => *p, None => continue };
        // Determine moon's slot in the ring around the parent.
        // Use index of this moon in its parent's children list for stable angles.
        let parent_body = match find_body(parent_id) { Some(p) => p, None => continue };
        let slot_idx = parent_body.children.iter().position(|m| m == &body.id).unwrap_or(0);
        let slot_count = parent_body.children.len().max(1) as f32;
        let angle = (slot_idx as f32 / slot_count) * std::f32::consts::TAU;
        let ring_r = 14.0_f32 + (slot_idx as f32 * 0.5).min(8.0);
        let mp = Pos2::new(
            parent_pos.x + ring_r * angle.cos(),
            parent_pos.y + ring_r * angle.sin(),
        );
        let mr = (1.5 * zoom).clamp(1.5, 4.0);
        paint.circle_filled(mp, mr, body_color(&body.name));
        if state.cosmos_selected_body.as_deref() == Some(body.id.as_str()) {
            paint.circle_stroke(mp, mr + 2.0, Stroke::new(1.0, theme.accent()));
        }
        if let Some(hp) = hover_pos {
            if (hp - mp).length() < mr + 3.0 {
                hovered_body = Some(body);
                if response.clicked() {
                    clicked_body = Some(body);
                }
            }
        }
    }

    // Click on Sun selects it too.
    if let Some(hp) = hover_pos {
        if (hp - center).length() < sun_r + 3.0 {
            if let Some(sun) = find_body("sun") {
                hovered_body = Some(sun);
                if response.clicked() {
                    clicked_body = Some(sun);
                }
            }
        }
    }

    // Apply click selection — drives the right-side details panel.
    if let Some(b) = clicked_body {
        state.cosmos_selected_body = Some(b.id.clone());
    }

    // v0.205.0: handle focus requests. When a body is requested as the
    // focus target (sidebar click or "Focus" button), shift cosmos_pan
    // so that body lands at the canvas center on this frame, then clear
    // the request. Lookup uses planet_positions which we populated above
    // with the body's screen position at current zoom + pan.
    if let Some(focus_id) = state.cosmos_focus_request.take() {
        let body_screen_pos = if focus_id == "sun" {
            Some(center)
        } else if let Some(p) = planet_positions.get(&focus_id) {
            Some(*p)
        } else {
            // Moon / asteroid not directly tracked — focus on its parent
            // (which is what the user probably expects since moons live
            // in the cosmetic ring around their planet).
            find_body(&focus_id)
                .and_then(|b| b.parent.as_ref())
                .and_then(|p| planet_positions.get(p))
                .copied()
        };
        if let Some(target) = body_screen_pos {
            // Translate pan so target moves from its current screen
            // position to the canvas center.
            let canvas_center = rect.center();
            let shift = canvas_center - target;
            state.cosmos_pan += shift;
        }
        // Note: pan changed mid-frame so the bodies just rendered are at
        // the OLD position. Egui repaints next frame so the visual lands
        // immediately. The one-frame stagger is imperceptible.
        ui.ctx().request_repaint();
    }

    // Hover tooltip.
    if let (Some(body), Some(_)) = (hovered_body, hover_pos) {
        response.on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(280.0);
            ui.label(RichText::new(&body.name).size(theme.font_size_body).color(theme.text_primary()).strong());
            let dist_str = if body.semi_major_axis_au > 0.0 {
                format!("{:.2} AU from Sun", body.semi_major_axis_au)
            } else if body.semi_major_axis_km > 0.0 {
                format!("{} km from {}", format_with_commas(body.semi_major_axis_km as i64),
                    body.parent.as_deref().map(titlecase).unwrap_or_default())
            } else {
                String::new()
            };
            ui.label(RichText::new(format!("{} · {}", titlecase(&body.body_type), dist_str))
                .size(theme.font_size_small).color(theme.text_secondary()));
            if body.radius_km > 0.0 {
                ui.label(RichText::new(format!("Radius: {}", format_km(body.radius_km)))
                    .size(theme.font_size_small).color(theme.text_muted()));
            }
            ui.label(
                RichText::new("Click to open details →")
                    .size(theme.font_size_small)
                    .color(theme.accent())
                    .italics(),
            );
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
