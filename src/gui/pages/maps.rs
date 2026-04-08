//! Map Viewer — solar system orbit view with planet details.
//!
//! Left sidebar: scrollable list of celestial bodies grouped by type.
//! Center: orbit visualization with animated planets, zoom, pan.
//! Right panel: selected body details. Real/Sim toggle support.

use egui::{Color32, Frame, Pos2, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::cell::RefCell;

/// A celestial body parsed from solar system data.
#[derive(Debug, Clone)]
struct CelestialBody {
    id: String,
    name: String,
    body_type: String,
    parent: Option<String>,
    semi_major_axis_au: f64,
    radius_km: f64,
    gravity: f64,
    atmosphere_desc: String,
    orbital_period_days: f64,
    moons: Vec<String>,
    description: String,
    mass_kg: f64,
    mean_temperature_k: f64,
}

/// Page-local state for the map viewer.
struct MapPageState {
    bodies: Vec<CelestialBody>,
    selected_body: Option<usize>,
    zoom: f32,
    pan_offset: Vec2,
    dragging: bool,
    last_drag_pos: Option<Pos2>,
    animation_time: f64,
    initialized: bool,
}

impl Default for MapPageState {
    fn default() -> Self {
        Self {
            bodies: Vec::new(),
            selected_body: None,
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            dragging: false,
            last_drag_pos: None,
            animation_time: 0.0,
            initialized: false,
        }
    }
}

fn with_state<R>(f: impl FnOnce(&mut MapPageState) -> R) -> R {
    thread_local! {
        static STATE: RefCell<MapPageState> = RefCell::new(MapPageState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Parse bodies from embedded JSON data.
fn parse_bodies() -> Vec<CelestialBody> {
    let json_str = crate::embedded_data::SOLAR_SYSTEM_JSON;
    let parsed: serde_json::Value = serde_json::from_str(json_str).unwrap_or_default();
    let bodies_arr = parsed.get("bodies").and_then(|b| b.as_array());

    let mut result = Vec::new();
    if let Some(arr) = bodies_arr {
        for body in arr {
            let body_type = body.get("type").and_then(|t| t.as_str()).unwrap_or("unknown").to_string();
            // Skip "region" type entries (asteroid belt, kuiper belt, etc.)
            if body_type == "region" {
                continue;
            }

            let orbit = body.get("orbit");
            let physical = body.get("physical");
            let atm = body.get("atmosphere");

            let atm_desc = if let Some(atm) = atm {
                if let Some(comp) = atm.get("composition").and_then(|c| c.as_object()) {
                    if comp.is_empty() {
                        "None".to_string()
                    } else {
                        let parts: Vec<String> = comp.iter()
                            .take(3)
                            .map(|(k, v)| format!("{}: {:.1}%", k, v.as_f64().unwrap_or(0.0)))
                            .collect();
                        parts.join(", ")
                    }
                } else {
                    "None".to_string()
                }
            } else {
                "None".to_string()
            };

            let moons: Vec<String> = body.get("moons")
                .and_then(|m| m.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();

            result.push(CelestialBody {
                id: body.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                name: body.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
                body_type,
                parent: body.get("parent").and_then(|v| v.as_str()).map(|s| s.to_string()),
                semi_major_axis_au: orbit.and_then(|o| o.get("semi_major_axis_au")).and_then(|v| v.as_f64()).unwrap_or(0.0),
                radius_km: physical.and_then(|p| p.get("radius_km")).and_then(|v| v.as_f64()).unwrap_or(0.0),
                gravity: physical.and_then(|p| p.get("surface_gravity_ms2")).and_then(|v| v.as_f64()).unwrap_or(0.0),
                atmosphere_desc: atm_desc,
                orbital_period_days: orbit.and_then(|o| o.get("orbital_period_days")).and_then(|v| v.as_f64()).unwrap_or(0.0),
                moons,
                description: body.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                mass_kg: physical.and_then(|p| p.get("mass_kg")).and_then(|v| v.as_f64()).unwrap_or(0.0),
                mean_temperature_k: physical.and_then(|p| p.get("mean_temperature_k")).and_then(|v| v.as_f64()).unwrap_or(0.0),
            });
        }
    }
    result
}

/// Assign each body a distinct color.
fn body_color(name: &str, body_type: &str) -> Color32 {
    match name {
        "Sun" => Color32::from_rgb(255, 200, 50),
        "Mercury" => Color32::from_rgb(180, 170, 160),
        "Venus" => Color32::from_rgb(220, 190, 120),
        "Earth" => Color32::from_rgb(80, 160, 220),
        "Mars" => Color32::from_rgb(200, 100, 70),
        "Jupiter" => Color32::from_rgb(210, 180, 140),
        "Saturn" => Color32::from_rgb(220, 200, 150),
        "Uranus" => Color32::from_rgb(150, 210, 220),
        "Neptune" => Color32::from_rgb(80, 100, 200),
        "Pluto" => Color32::from_rgb(200, 180, 170),
        "Ceres" => Color32::from_rgb(150, 150, 140),
        "Eris" => Color32::from_rgb(200, 200, 210),
        _ => match body_type {
            "moon" => Color32::from_rgb(160, 160, 170),
            "dwarf_planet" => Color32::from_rgb(180, 160, 150),
            "asteroid" => Color32::from_rgb(140, 130, 120),
            _ => Color32::from_rgb(160, 160, 160),
        }
    }
}

fn type_display_name(body_type: &str) -> &str {
    match body_type {
        "star" => "Star",
        "terrestrial" => "Planet",
        "gas_giant" => "Planet",
        "ice_giant" => "Planet",
        "moon" => "Moon",
        "dwarf_planet" => "Dwarf Planet",
        "asteroid" => "Asteroid",
        _ => "Other",
    }
}

fn type_sort_order(body_type: &str) -> u8 {
    match body_type {
        "star" => 0,
        "terrestrial" | "gas_giant" | "ice_giant" => 1,
        "dwarf_planet" => 2,
        "moon" => 3,
        "asteroid" => 4,
        _ => 5,
    }
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Initialize bodies from embedded data on first draw
    with_state(|ps| {
        if !ps.initialized {
            ps.bodies = parse_bodies();
            // Default select Earth
            ps.selected_body = ps.bodies.iter().position(|b| b.name == "Earth");
            ps.initialized = true;
        }
        // Advance animation
        ps.animation_time += 0.016; // ~60fps
    });

    // Request continuous repaint for animation
    ctx.request_repaint();

    // Left sidebar: body list grouped by type
    egui::SidePanel::left("map_body_list")
        .min_width(160.0)
        .max_width(200.0)
        .frame(Frame::none().fill(theme.bg_sidebar()).inner_margin(8.0))
        .show(ctx, |ui| {
            ui.label(RichText::new("Celestial Bodies").size(theme.font_size_heading).color(theme.text_primary()));
            ui.add_space(theme.spacing_xs);
            ui.separator();
            ui.add_space(theme.spacing_xs);

            ScrollArea::vertical().show(ui, |ui| {
                with_state(|ps| {
                    // Group bodies by type category
                    let groups: &[(&str, &str)] = &[
                        ("star", "Stars"),
                        ("planet", "Planets"),
                        ("dwarf_planet", "Dwarf Planets"),
                        ("moon", "Moons"),
                        ("asteroid", "Asteroids"),
                    ];

                    for &(group_type, group_label) in groups {
                        let bodies_in_group: Vec<usize> = ps.bodies.iter().enumerate()
                            .filter(|(_, b)| {
                                if group_type == "planet" {
                                    matches!(b.body_type.as_str(), "terrestrial" | "gas_giant" | "ice_giant")
                                } else {
                                    b.body_type == group_type
                                }
                            })
                            .map(|(i, _)| i)
                            .collect();

                        if bodies_in_group.is_empty() {
                            continue;
                        }

                        ui.label(RichText::new(group_label).size(theme.font_size_small).color(theme.text_muted()));
                        ui.add_space(theme.row_gap);

                        for idx in bodies_in_group {
                            let body = &ps.bodies[idx];
                            let selected = ps.selected_body == Some(idx);
                            let text_color = if selected { theme.accent() } else { theme.text_secondary() };
                            let bc = body_color(&body.name, &body.body_type);

                            ui.horizontal(|ui| {
                                let (dot_rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                                ui.painter().circle_filled(dot_rect.center(), 4.0, bc);
                                if ui.selectable_label(selected, RichText::new(&body.name).color(text_color).size(theme.font_size_small)).clicked() {
                                    ps.selected_body = Some(idx);
                                }
                            });
                        }
                        ui.add_space(theme.spacing_xs);
                    }
                });
            });
        });

    // Right panel: selected body details
    with_state(|ps| {
        if let Some(idx) = ps.selected_body {
            if let Some(body) = ps.bodies.get(idx) {
                let body_clone = body.clone();
                egui::SidePanel::right("map_body_detail")
                    .min_width(220.0)
                    .max_width(300.0)
                    .frame(Frame::none().fill(theme.bg_sidebar()).inner_margin(10.0))
                    .show(ctx, |ui| {
                        ScrollArea::vertical().show(ui, |ui| {
                            ui.label(RichText::new(&body_clone.name).size(theme.font_size_title).color(theme.accent()));
                            ui.add_space(theme.spacing_xs);

                            // Type badge
                            let type_name = type_display_name(&body_clone.body_type);
                            egui::Frame::none()
                                .fill(Theme::c32(&theme.info))
                                .rounding(Rounding::same(3))
                                .inner_margin(Vec2::new(6.0, 2.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(type_name).size(theme.font_size_small).color(Color32::WHITE));
                                });

                            ui.add_space(theme.spacing_sm);

                            widgets::card(ui, theme, |ui| {
                                crate::gui::widgets::detail_row(ui, theme, "Radius", &format!("{:.1} km", body_clone.radius_km));
                                if body_clone.mass_kg > 0.0 {
                                    crate::gui::widgets::detail_row(ui, theme, "Mass", &format_mass(body_clone.mass_kg));
                                }
                                crate::gui::widgets::detail_row(ui, theme, "Gravity", &format!("{:.2} m/s2", body_clone.gravity));
                                crate::gui::widgets::detail_row(ui, theme, "Atmosphere", &body_clone.atmosphere_desc);
                                if body_clone.orbital_period_days > 0.0 {
                                    crate::gui::widgets::detail_row(ui, theme, "Orbital Period", &format!("{:.1} days", body_clone.orbital_period_days));
                                }
                                if body_clone.semi_major_axis_au > 0.0 {
                                    crate::gui::widgets::detail_row(ui, theme, "Orbit Radius", &format!("{:.3} AU", body_clone.semi_major_axis_au));
                                }
                                if body_clone.mean_temperature_k > 0.0 {
                                    crate::gui::widgets::detail_row(ui, theme, "Temperature", &format!("{:.0} K", body_clone.mean_temperature_k));
                                }
                                if !body_clone.moons.is_empty() {
                                    crate::gui::widgets::detail_row(ui, theme, "Moons", &body_clone.moons.len().to_string());
                                }
                            });

                            // Moons list
                            if !body_clone.moons.is_empty() {
                                ui.add_space(theme.spacing_xs);
                                ui.label(RichText::new("Moons:").size(theme.font_size_small).color(theme.text_secondary()));
                                ui.horizontal_wrapped(|ui| {
                                    for moon in &body_clone.moons {
                                        egui::Frame::none()
                                            .fill(theme.bg_secondary())
                                            .rounding(Rounding::same(3))
                                            .inner_margin(Vec2::new(4.0, 2.0))
                                            .show(ui, |ui| {
                                                ui.label(RichText::new(moon).size(theme.font_size_small).color(theme.text_muted()));
                                            });
                                    }
                                });
                            }

                            // Description
                            if !body_clone.description.is_empty() {
                                ui.add_space(theme.spacing_sm);
                                ui.label(RichText::new(&body_clone.description).color(theme.text_secondary()).size(theme.font_size_small));
                            }
                        });
                    });
            }
        }
    });

    // Center: orbit visualization
    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(8, 8, 14)).inner_margin(0.0))
        .show(ctx, |ui| {
            // Header bar
            ui.horizontal(|ui| {
                ui.add_space(theme.panel_margin);
                ui.label(RichText::new("Solar System Map").size(theme.font_size_heading).color(theme.text_primary()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mode_text = if state.context_real { "Real" } else { "Sim" };
                    if widgets::secondary_button(ui, theme, mode_text) {
                        state.context_real = !state.context_real;
                    }
                });
            });

            if state.context_real {
                // Real mode: earth focus
                ui.add_space(theme.spacing_lg);
                ui.vertical_centered(|ui| {
                    widgets::card(ui, theme, |ui| {
                        ui.label(RichText::new("Earth - Real World").size(theme.font_size_heading).color(theme.accent()));
                        ui.add_space(theme.spacing_xs);
                        ui.label(RichText::new("Interactive map integration (OpenStreetMap) is planned for the native app.").color(theme.text_secondary()));
                        ui.label(RichText::new("Use the web version for real-world mapping features.").color(theme.text_muted()));
                    });
                });
            } else {
                // Sim mode: orbit visualization
                let available = ui.available_size();
                let (rect, response) = ui.allocate_exact_size(available, egui::Sense::click_and_drag());
                let center = rect.center() + with_state(|ps| ps.pan_offset);
                let paint = ui.painter_at(rect);

                // Background
                paint.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(8, 8, 14));

                // Handle zoom with scroll wheel
                let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll_delta != 0.0 && ui.rect_contains_pointer(rect) {
                    with_state(|ps| {
                        ps.zoom = (ps.zoom + scroll_delta * 0.005).clamp(0.2, 10.0);
                    });
                }

                // Handle pan with click-drag
                if response.dragged() {
                    with_state(|ps| {
                        ps.pan_offset += response.drag_delta();
                    });
                }

                let (zoom, anim_time) = with_state(|ps| (ps.zoom, ps.animation_time));

                // Sun at center
                let sun_r = (8.0 * zoom).clamp(4.0, 30.0);
                paint.circle_filled(center, sun_r, Color32::from_rgb(255, 200, 50));

                // Draw orbits and planets for bodies that orbit the sun
                let mut clicked_body: Option<usize> = None;

                with_state(|ps| {
                    let max_au = 35.0;
                    let scale = ((rect.width().min(rect.height()) / 2.0 - 20.0) / max_au) * zoom;

                    // First pass: collect planet positions for moon rendering
                    let mut planet_positions: Vec<(String, f32, f32)> = Vec::new(); // (name, px, py)

                    for (i, body) in ps.bodies.iter().enumerate() {
                        // Only draw bodies that orbit the sun (planets, dwarf planets, asteroids)
                        if body.parent.as_deref() != Some("sun") {
                            continue;
                        }
                        if body.semi_major_axis_au <= 0.0 {
                            continue;
                        }

                        let orbit_r = body.semi_major_axis_au as f32 * scale;
                        if orbit_r < 3.0 || orbit_r > rect.width() {
                            continue;
                        }

                        // Orbit ring
                        paint.circle_stroke(center, orbit_r, Stroke::new(0.5, Color32::from_rgb(35, 35, 55)));

                        // Animated planet position
                        let angular_speed = if body.orbital_period_days > 0.0 {
                            std::f64::consts::TAU / (body.orbital_period_days * 0.01)
                        } else {
                            0.5
                        };
                        let angle = (anim_time * angular_speed) as f32;
                        let px = center.x + orbit_r * angle.cos();
                        let py = center.y + orbit_r * angle.sin();

                        // Store position for moon rendering
                        planet_positions.push((body.name.to_lowercase(), px, py));

                        // Planet size based on actual radius (log scale)
                        let planet_r = if body.radius_km > 30000.0 {
                            (5.0 * zoom).clamp(3.0, 12.0)
                        } else if body.radius_km > 5000.0 {
                            (3.5 * zoom).clamp(2.5, 8.0)
                        } else {
                            (2.5 * zoom).clamp(2.0, 6.0)
                        };

                        let bc = body_color(&body.name, &body.body_type);
                        let is_selected = ps.selected_body == Some(i);

                        if is_selected {
                            paint.circle_stroke(Pos2::new(px, py), planet_r + 3.0, Stroke::new(1.5, theme.accent()));
                        }
                        paint.circle_filled(Pos2::new(px, py), planet_r, bc);

                        // Label
                        if zoom > 0.8 || is_selected {
                            paint.text(
                                Pos2::new(px, py - planet_r - 6.0),
                                egui::Align2::CENTER_BOTTOM,
                                &body.name,
                                egui::FontId::proportional(10.0),
                                if is_selected { theme.accent() } else { theme.text_muted() },
                            );
                        }

                        // Click detection
                        if response.clicked() {
                            if let Some(click_pos) = response.interact_pointer_pos() {
                                let dist = ((click_pos.x - px).powi(2) + (click_pos.y - py).powi(2)).sqrt();
                                if dist < planet_r + 8.0 {
                                    clicked_body = Some(i);
                                }
                            }
                        }
                    }

                    // Second pass: draw moons orbiting their parent planets
                    for (i, body) in ps.bodies.iter().enumerate() {
                        let parent_name = match body.parent.as_deref() {
                            Some(p) if p != "sun" => p.to_lowercase(),
                            _ => continue,
                        };
                        // Find parent planet position
                        let (parent_px, parent_py) = match planet_positions.iter().find(|(n, _, _)| *n == parent_name) {
                            Some((_, px, py)) => (*px, *py),
                            None => continue,
                        };

                        // Moon orbit radius (scaled, with minimum visibility)
                        // Use semi_major_axis_au if available, otherwise derive from km
                        let moon_orbit_au = if body.semi_major_axis_au > 0.0 {
                            body.semi_major_axis_au
                        } else {
                            // Fallback: use km converted to AU
                            body.semi_major_axis_au.max(0.003)
                        };
                        // Exaggerate moon orbit radius so moons are visible (real scale is too tiny)
                        let moon_orbit_r = (moon_orbit_au as f32 * scale * 80.0).clamp(8.0, 40.0);

                        // Animated moon position around parent
                        let moon_speed = if body.orbital_period_days > 0.0 {
                            std::f64::consts::TAU / (body.orbital_period_days * 0.002)
                        } else {
                            2.0
                        };
                        let moon_angle = (anim_time * moon_speed) as f32;
                        let mx = parent_px + moon_orbit_r * moon_angle.cos();
                        let my = parent_py + moon_orbit_r * moon_angle.sin();

                        let moon_r = (1.5 * zoom).clamp(1.0, 4.0);
                        let mc = body_color(&body.name, &body.body_type);
                        let is_selected = ps.selected_body == Some(i);

                        // Small orbit ring around parent
                        if zoom > 2.0 {
                            paint.circle_stroke(
                                Pos2::new(parent_px, parent_py),
                                moon_orbit_r,
                                Stroke::new(0.3, Color32::from_rgb(50, 50, 70)),
                            );
                        }

                        if is_selected {
                            paint.circle_stroke(Pos2::new(mx, my), moon_r + 2.0, Stroke::new(1.0, theme.accent()));
                        }
                        paint.circle_filled(Pos2::new(mx, my), moon_r, mc);

                        // Moon label (only when zoomed in)
                        if zoom > 3.0 || is_selected {
                            paint.text(
                                Pos2::new(mx, my - moon_r - 4.0),
                                egui::Align2::CENTER_BOTTOM,
                                &body.name,
                                egui::FontId::proportional(8.0),
                                if is_selected { theme.accent() } else { theme.text_muted() },
                            );
                        }

                        // Click detection for moons
                        if response.clicked() {
                            if let Some(click_pos) = response.interact_pointer_pos() {
                                let dist = ((click_pos.x - mx).powi(2) + (click_pos.y - my).powi(2)).sqrt();
                                if dist < moon_r + 6.0 {
                                    clicked_body = Some(i);
                                }
                            }
                        }
                    }
                });

                if let Some(idx) = clicked_body {
                    with_state(|ps| ps.selected_body = Some(idx));
                }

                // Scale indicator at bottom
                let scale_y = rect.bottom() - 20.0;
                let scale_x = rect.left() + 20.0;
                let (zoom_val,) = with_state(|ps| (ps.zoom,));
                let au_per_100px = 35.0 / ((rect.width().min(rect.height()) / 2.0 - 20.0) * zoom_val) * 100.0;
                paint.line_segment(
                    [Pos2::new(scale_x, scale_y), Pos2::new(scale_x + 100.0, scale_y)],
                    Stroke::new(1.0, theme.text_muted()),
                );
                paint.text(
                    Pos2::new(scale_x + 50.0, scale_y - 4.0),
                    egui::Align2::CENTER_BOTTOM,
                    format!("{:.1} AU", au_per_100px),
                    egui::FontId::proportional(10.0),
                    theme.text_muted(),
                );

                // Zoom indicator
                paint.text(
                    Pos2::new(rect.right() - 10.0, scale_y),
                    egui::Align2::RIGHT_BOTTOM,
                    format!("Zoom: {:.1}x", zoom_val),
                    egui::FontId::proportional(10.0),
                    theme.text_muted(),
                );
            }
        });
}

// detail_row moved to crate::gui::widgets::detail_row

fn format_mass(kg: f64) -> String {
    if kg >= 1e27 {
        format!("{:.2e} kg", kg)
    } else if kg >= 1e24 {
        format!("{:.2} x 10^24 kg", kg / 1e24)
    } else if kg >= 1e21 {
        format!("{:.2} x 10^21 kg", kg / 1e21)
    } else {
        format!("{:.2e} kg", kg)
    }
}
