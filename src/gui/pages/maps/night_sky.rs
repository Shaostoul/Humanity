//! The Maps page's Night Sky view: the celestial sphere from Earth on an
//! RA/Dec grid, the bright-star catalog and the constellation figures (their
//! myths, seasons and key stars on hover).
//! Moved out of `gui/pages/cosmos.rs` in Maps increment 0 (2026-10-20).

use egui::{Align2, Color32, Pos2, RichText, Rounding, Stroke, Vec2};
use std::sync::OnceLock;

use crate::gui::theme::Theme;
use crate::gui::GuiState;

use super::{allocate_canvas, catalog_text, spectral_color};

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

static BRIGHT_STARS: OnceLock<Vec<BrightStar>> = OnceLock::new();
static CONSTELLATIONS: OnceLock<Vec<Constellation>> = OnceLock::new();

fn bright_stars() -> &'static [BrightStar] {
    BRIGHT_STARS.get_or_init(|| {
        let text = catalog_text("stars-catalog.json");
        let json = text.as_str();
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
        log::info!("Maps: loaded {} bright catalog stars", out.len());
        out
    })
}

fn constellations() -> &'static [Constellation] {
    CONSTELLATIONS.get_or_init(|| {
        let text = catalog_text("constellations.json");
        let json = text.as_str();
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
        log::info!("Maps: loaded {} constellations", out.len());
        out
    })
}

// ─────────────────────── Night Sky view (RA/Dec, Earth-centered) ────────────

pub(super) fn draw_night_sky_view(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
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
        inner.center() + from_center * zoom + state.maps.pan
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

    // Constellation lines first (drawn under the stars). Same theme token as
    // the 3D sky's figures (v0.786) so one Appearance color drives both.
    let const_color = theme.constellation_line();
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

/// Perpendicular distance from `p` to the line segment `a → b`.
fn dist_point_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    if len_sq < 1e-6 { return (p - a).length(); }
    let t = ((p - a).dot(ab) / len_sq).clamp(0.0, 1.0);
    let closest = a + ab * t;
    (p - closest).length()
}
