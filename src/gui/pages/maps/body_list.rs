//! The Maps page's System view side panels: the body list on the left
//! (Stations, then the Sun's family by region, moons under their planets) and
//! the selected body's details on the right.
//! Moved out of `gui/pages/cosmos.rs` in Maps increment 0 (2026-10-20).

use egui::{RichText, ScrollArea, Sense, Vec2};

use crate::cosmos::{find_body, sol_bodies, SolBody};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

use super::{body_color, format_km, titlecase};

// ─────────────────────── Body browser (left sidebar) ────────────────────────

/// Region groups for the body browser. Each variant holds the body ids
/// that fall in that region; population happens lazily.
///
/// v0.207.0: Asteroids further subdivided by semi-major axis into the
/// real-astronomy regions so users can see WHICH region a body lives in
/// rather than all asteroids being lumped together (which made it
/// confusing why Ryugu showed up between Earth and Mars vs. Vesta out
/// past the main belt). The cutoffs follow standard convention:
///   - **Near-Earth Asteroids (NEA)**: PERIHELION q = a(1-e) < 1.3 AU --
///     the actual IAU/CNEOS definition. v0.686 fix: the earlier semi-major
///     axis shortcut (a < 1.3) misfiled Eros (a=1.46, q=1.13) and Itokawa
///     (a=1.32, q=0.95) -- two of the most famous NEAs, both visited by
///     spacecraft -- into the Main Belt (caught in the snapshot QA sweep).
///   - **Main Belt**: everything else with a < 4.0 AU (covers Hildas +
///     Trojans too; we'll split those once we have sample bodies there)
///   - **Trans-Neptunian / Outer Belt**: a ≥ 4.0 AU (Kuiper Belt + scattered disk)
/// Same buckets a planetary scientist would use.
fn body_regions() -> Vec<(&'static str, Vec<&'static SolBody>)> {
    let bodies = sol_bodies();
    let mut star = Vec::new();
    let mut inner = Vec::new();
    let mut outer = Vec::new();
    let mut dwarfs = Vec::new();
    let mut nea = Vec::new();         // Near-Earth asteroids (< 1.3 AU)
    let mut main_belt = Vec::new();   // Main belt (1.3 ≤ a < 4.0 AU)
    let mut trans_nep = Vec::new();   // Trans-Neptunian / outer (≥ 4.0 AU)

    for b in bodies {
        match (b.body_type.as_str(), b.parent.as_deref()) {
            ("star", _)                  => star.push(b),
            ("terrestrial", Some("sun")) => inner.push(b),
            ("gas_giant",   Some("sun")) |
            ("ice_giant",   Some("sun")) => outer.push(b),
            ("dwarf_planet", _)          => dwarfs.push(b),
            ("asteroid", Some("sun")) => {
                let a = b.semi_major_axis_au;
                // NEA = perihelion under 1.3 AU (the real definition), not
                // semi-major axis -- eccentric NEAs like Eros swing far out
                // but dive near Earth's orbit every revolution.
                let perihelion = a * (1.0 - b.eccentricity);
                if perihelion < 1.3 { nea.push(b); }
                else if a < 4.0 { main_belt.push(b); }
                else { trans_nep.push(b); }
            }
            _ => {} // moons handled per-parent in the sidebar tree
        }
    }
    let by_au = |a: &&SolBody, b: &&SolBody|
        a.semi_major_axis_au.partial_cmp(&b.semi_major_axis_au)
            .unwrap_or(std::cmp::Ordering::Equal);
    inner.sort_by(by_au);
    outer.sort_by(by_au);
    dwarfs.sort_by(by_au);
    nea.sort_by(by_au);
    main_belt.sort_by(by_au);
    trans_nep.sort_by(by_au);

    let mut regions: Vec<(&'static str, Vec<&'static SolBody>)> = vec![
        ("Star",          star),
        ("Inner Planets", inner),
        ("Outer Planets", outer),
        ("Dwarf Planets", dwarfs),
    ];
    // Only show non-empty asteroid sub-regions so users don't see "Main
    // Belt (0)" cluttering the sidebar.
    if !nea.is_empty()       { regions.push(("Near-Earth Asteroids", nea)); }
    if !main_belt.is_empty() { regions.push(("Main Belt", main_belt)); }
    if !trans_nep.is_empty() { regions.push(("Trans-Neptunian", trans_nep)); }
    regions
}

pub(super) fn draw_body_browser(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
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

    // ── Stations (v0.885, target-marker arc; from data since Maps increment
    // 1, `stations.rs`) ── tracking the home draws the in-world ring + label
    // (hud target_markers). Every data/stations/*.ron is a row.
    super::stations::draw_station_rows(ui, theme, state);
    ui.add_space(theme.spacing_xs);

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
                if !body.children.is_empty() && state.maps.expanded_planets.contains(&body.id) {
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
            let expanded = state.maps.expanded_planets.contains(&body.id);
            let chevron = if expanded { "▾" } else { "▸" };
            let resp = ui.add(egui::Label::new(
                RichText::new(chevron)
                    .size(theme.font_size_small)
                    .color(theme.text_muted())
                    .monospace(),
            ).sense(Sense::click()));
            if resp.clicked() {
                if expanded {
                    state.maps.expanded_planets.remove(&body.id);
                } else {
                    state.maps.expanded_planets.insert(body.id.clone());
                }
            }
        } else {
            ui.add_space(10.0);
        }
        // Color dot matching the body's display color.
        let (rect, _r) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 4.0, body_color(&body.name));
        // Name — clickable to select.
        let selected = state.maps.selected_body.as_deref() == Some(body.id.as_str());
        let label_color = if selected { theme.accent() } else { theme.text_primary() };
        let resp = ui.add(egui::Label::new(
            RichText::new(&body.name).size(theme.font_size_small).color(label_color),
        ).sense(Sense::click()));
        if resp.clicked() {
            state.maps.selected_body = Some(body.id.clone());
            // v0.205.0: sidebar clicks also focus the map. The user
            // explicitly said "show me X" by clicking it in the
            // browser — natural to follow up by centering on it.
            state.maps.focus_request = Some(body.id.clone());
        }
    });
}

// ─────────────────────── Body details (right sidebar) ───────────────────────

pub(super) fn draw_body_details(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let body = match state.maps.selected_body.as_deref().and_then(find_body) {
        Some(b) => b,
        None => {
            ui.label(
                RichText::new("Select a body")
                    .size(theme.font_size_heading)
                    .color(theme.text_secondary()),
            );
            ui.label(
                RichText::new("Click any planet, moon, or dwarf planet in the left-side browser, its details appear here.")
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
                state.maps.focus_request = Some(body.id.clone());
                ui.ctx().request_repaint();
            }
        });
    });
    ui.label(
        RichText::new(format!("{} · {}", titlecase(&body.body_type),
            body.parent.as_deref().map(|p| format!("orbits {}", titlecase(p))).unwrap_or_else(|| "-".to_string())))
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
                        state.maps.selected_body = Some(moon.id.clone());
                        // v0.205.0: also focus on the moon (which falls
                        // back to its parent planet via the focus_request
                        // handler since moons live in the cosmetic ring).
                        state.maps.focus_request = Some(moon.id.clone());
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
