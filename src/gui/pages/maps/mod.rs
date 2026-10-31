//! The Maps page: one page, four views (Planet, Solar System, Galaxy, Night
//! Sky), drawn by `draw` from `GuiState::maps` (`gui/state_types/maps.rs`).
//!
//! It was `gui/pages/cosmos.rs` (v0.203.0, Phase 3 of the cosmos architecture)
//! until 2026-10-20, when Maps increment 0 (docs/design/maps-zoom-to-anything.md)
//! renamed it to the page's own name and carved its 3,880 lines into this
//! folder. `crate::cosmos` (`src/cosmos.rs`) keeps its name: it is the Sol
//! model the page and the game both read, not the page.
//!
//! Where things are:
//! - `mod.rs` (this file): the page entry `draw`, the view tabs, the view
//!   enum `MapView`, and what more than one view shares: the pan and zoom
//!   canvas, the footer, the body and star colours, the catalog reader, and
//!   the number formatting.
//! - `system_view.rs`: the 3D Solar System view, the reference-orbit and
//!   Lagrange overlays, and a body's info card with Focus and Track.
//! - `camera.rs` (increment 1): `MapCamera`, an anchor plus an f64 offset;
//!   `controls.rs` its mouse, keys and follow row; `you_layer.rs` the world
//!   layer and you and your ship; `stations.rs` the Stations rows.
//! - `body_list.rs`: the System view's two side panels, the body list on the
//!   left and the selected body's details on the right.
//! - `planet_view.rs`: the 2D OpenStreetMap region view.
//! - `galaxy_view.rs`: the top-down star map from `data/stars-map.bin`.
//! - `night_sky.rs`: the RA/Dec sky with constellation lines.
//! - `sky_events.rs`: conjunctions and eclipses now, the forward scan for the
//!   next ones, and where on Earth an eclipse is overhead.
//! - `time_bar.rs`: the page's clock (the date, play, speed, the scrubber).
//! - `tests.rs`: the page's unit tests.
//!
//! Operator 2026-05-10: "We at least want our galaxy and solar system
//! ... see the real stars and real constellations." This page reads
//! from the existing data files (constellations.json, stars-nearby.json,
//! stars-catalog.json, sol.json, stars-map.bin, data/maps/regions/).

use egui::{Align2, Color32, Frame, Pos2, Rect, RichText, Sense, Stroke, Vec2};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

mod belts;
mod bodies;
mod body_list;
pub mod camera;
mod controls;
mod diagram;
mod galaxy_view;
mod labels;
mod night_sky;
mod orbits;
pub mod overlays;
mod planet_view;
mod scene;
pub mod ship_view;
mod sky_events;
mod stations;
mod system_view;
mod time_bar;
/// The plan tiles' grid: a tile's key, the zoom, which tiles cover a view (Maps increment 3).
pub mod tile_grid;
pub mod world_layer;
pub mod you_layer;
#[cfg(test)]
mod tests;

pub use belts::BeltCache;
pub use sky_events::UpcomingSkyEvent;
pub use camera::MapCamera;
pub use stations::{parent_constants, StationRow};
pub(crate) use time_bar::current_real_time_seconds_since_j2000;
#[cfg(test)]
pub(crate) use time_bar::set_clock_for_snapshot;

/// A star or constellation table from the data folder, or the copy built into
/// the exe when the folder has none. (2026-10-03: these three read the built-in
/// copy only, so a data edit changed nothing on this page until a rebuild, and
/// the source stamp, BUG-133, could not see that the binary was behind.)
fn catalog_text(rel: &str) -> String {
    crate::embedded_data::read_data_or_embedded(&crate::data_dir(), rel).unwrap_or_default()
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

// The body colours that stood here (`body_color`, a hard-coded match on ten
// names, every other body grey) went in Maps increment 2: a body's colour is
// DATA now, `SolBody::map_color` from sol.json (`scripts/build-body-colors.js`),
// painted through `bodies::map_color32`.

// ─────────────────────── Page entry point ───────────────────────────────────

/// View mode — operator selectable via tab bar at the top of the page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MapView {
    /// 2D GPS-style map of a real Earth region: OpenStreetMap roads and
    /// building footprints from data/maps/regions/*.bin (maps ladder rung 3,
    /// v0.1146; generator scripts/fetch-osm-region.mjs).
    Planet,
    /// 3D System view of Sol — Sun at origin, planets in their orbits,
    /// moons in real positions relative to their planets. Rotatable
    /// camera (drag), zoomable (scroll), pannable target (shift+drag).
    /// v0.206.0: replaced the v0.203 2D top-down with proper 3D.
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
    let in_system_view = state.maps.view == MapView::System;

    if in_system_view {
        // Left: collapsible body browser tree.
        egui::SidePanel::left("maps_body_browser")
            .resizable(true)
            .min_width(220.0)
            .max_width(360.0)
            .default_width(260.0)
            .frame(Frame::NONE.fill(theme.bg_panel()).inner_margin(theme.spacing_sm))
            .show(ctx, |ui| {
                body_list::draw_body_browser(ui, theme, state);
            });

        // Right: details for the selected body.
        egui::SidePanel::right("maps_body_details")
            .resizable(true)
            .min_width(260.0)
            .max_width(420.0)
            .default_width(300.0)
            .frame(Frame::NONE.fill(theme.bg_panel()).inner_margin(theme.spacing_md))
            .show(ctx, |ui| {
                body_list::draw_body_details(ui, theme, state);
            });
    }

    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(Color32::from_rgb(8, 8, 14)).inner_margin(0.0))  // theme-exempt: deep-space backdrop — domain aesthetic, not theme
        .show(ctx, |ui| {
            // Header bar with view tabs + scale info.
            ui.horizontal(|ui| {
                ui.add_space(theme.spacing_md);
                // "Maps" is the page name (matches the nav button; the old
                // "Cosmos" heading was a third name for the same page). The
                // view ladder below follows the operator's map taxonomy:
                // planets (future GPS/OSM view) -> solar system -> galaxy ->
                // some day the cosmic web.
                ui.label(
                    RichText::new("Maps")
                        .size(theme.font_size_heading)
                        .color(theme.text_primary())
                        .strong(),
                );
                ui.add_space(theme.spacing_lg);
                view_tab(ui, theme, state, MapView::Planet,    "Planet",          "Real Earth regions from OpenStreetMap: roads and buildings, GPS-map style (2D).");
                view_tab(ui, theme, state, MapView::System,    "Solar System",    "Sol, Sun + planets + moons in 3D. Drag to rotate, scroll to zoom, shift+drag to pan.");
                view_tab(ui, theme, state, MapView::Galactic,  "Galaxy",          "Sol-centered map of real catalog stars, light-year scale (2D top-down).");
                view_tab(ui, theme, state, MapView::NightSky,  "Night Sky",       "Earth-centered celestial sphere with constellation lines (2D RA/Dec projection).");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(theme.spacing_md);
                    let hint = match state.maps.view {
                        MapView::Planet    => "2D map · scroll to zoom · click-drag to pan",
                        MapView::System    => "3D camera · drag to turn · scroll to zoom · Home: back to me",
                        MapView::Galactic  => "2D top-down · scroll to zoom · click-drag to pan",
                        MapView::NightSky  => "2D celestial sphere · scroll to zoom · click-drag to pan",
                    };
                    // Truncated to the room left beside the view tabs (full
                    // text on hover). Untruncated it ran leftward over the
                    // tabs whenever the side panels left the centre narrow,
                    // which with both System panels open is most of the time
                    // (2026-09-27 snapshot review). The same controls are
                    // spelled out at the bottom of the canvas anyway.
                    ui.add(
                        egui::Label::new(
                            RichText::new(hint)
                                .size(theme.font_size_small)
                                .color(theme.text_muted())
                                .italics(),
                        )
                        .truncate(),
                    );
                });
            });
            ui.separator();

            match state.maps.view {
                MapView::Planet    => planet_view::draw_planet_view(ui, theme, state),
                MapView::System    => system_view::draw_system_view(ui, theme, state),
                MapView::Galactic  => galaxy_view::draw_galactic_view(ui, theme, state),
                MapView::NightSky  => night_sky::draw_night_sky_view(ui, theme, state),
            }
        });
}

fn format_km(km: f64) -> String {
    if km >= 1.0e6 { format!("{:.2} million km", km / 1.0e6) }
    else if km >= 1.0e3 { format!("{} km", format_with_commas(km as i64)) }
    else { format!("{:.1} km", km) }
}

fn view_tab(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &mut GuiState,
    mode: MapView,
    label: &str,
    tooltip: &str,
) {
    let active = state.maps.view == mode;
    if widgets::Button::secondary(label).active(active).tooltip(tooltip).show(ui, theme) {
        state.maps.view = mode;
        // Reset zoom + pan on view change so the user lands at a sensible default.
        state.maps.zoom = 1.0;
        state.maps.pan = Vec2::ZERO;
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
        state.maps.pan += response.drag_delta();
    }

    // Zoom — multiplicative + cursor-anchored.
    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll_delta != 0.0 && ui.rect_contains_pointer(rect) {
        let cursor = ui.input(|i| i.pointer.hover_pos()).unwrap_or(rect.center());

        // 1.0015 per scroll-pixel = ~1.08× per typical 50-px scroll tick.
        // Picked to feel responsive without being jumpy.
        let zoom_before = state.maps.zoom;
        // Lower bound 0.005 (was 0.05, v0.1145): the galaxy view's real
        // catalog spans thousands of light-years; at base scale 50 ly this
        // reaches a ~10,000 ly view radius.
        let zoom_after = (zoom_before * (1.0015_f32).powf(scroll_delta)).clamp(0.005, 200.0);

        // Cursor-anchored: shift pan so the world point under the cursor
        // ends up at the same screen position after the zoom change. Math:
        //   cursor_offset = cursor - rect.center()  (cursor relative to canvas center)
        //   ratio = zoom_after / zoom_before
        //   new_pan = cursor_offset * (1 - ratio) + old_pan * ratio
        // Derivation in the cosmos doc / commit message.
        let ratio = zoom_after / zoom_before;
        let cursor_offset = cursor - rect.center();
        state.maps.pan = cursor_offset * (1.0 - ratio) + state.maps.pan * ratio;
        state.maps.zoom = zoom_after;
    }

    let center = rect.center() + state.maps.pan;
    (rect, response, center, state.maps.zoom)
}

/// A map view's footer line (what the view is showing and, on the Planet
/// view, the OpenStreetMap attribution the licence requires) on a themed
/// backing, so it reads over whatever the map draws beneath it. Roads ran
/// straight through the bare attribution (2026-09-27 snapshot review).
/// `layout` first so the label pass can reserve `backing`; `paint` last so
/// the footer sits over everything.
struct MapFooter {
    galley: std::sync::Arc<egui::Galley>,
    text_rect: Rect,
    backing: Rect,
}

impl MapFooter {
    fn layout(paint: &egui::Painter, theme: &Theme, rect: Rect, text: String) -> Self {
        let galley = paint.layout_no_wrap(text, egui::FontId::proportional(10.0), theme.text_muted());
        let text_rect = Align2::LEFT_BOTTOM
            .anchor_size(Pos2::new(rect.left() + 8.0, rect.bottom() - 8.0), galley.size());
        let backing = text_rect.expand2(Vec2::new(theme.spacing_sm, theme.spacing_xs + 1.0));
        Self { galley, text_rect, backing }
    }

    fn paint(self, paint: &egui::Painter, theme: &Theme) {
        paint.rect(
            self.backing,
            theme.border_radius,
            theme.bg_panel().gamma_multiply(0.85),
            Stroke::new(theme.border_width, theme.border()),
            egui::StrokeKind::Inside,
        );
        paint.galley(self.text_rect.min, self.galley, theme.text_muted());
    }
}

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
