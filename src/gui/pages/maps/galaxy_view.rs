//! The Maps page's Galaxy view: the real HYG catalog (`data/stars-map.bin`)
//! drawn top-down around Sol at light-year scale, with the curated nearby list
//! as the fallback when the catalog file is missing.
//! Moved out of `gui/pages/cosmos.rs` in Maps increment 0 (2026-10-20).

use egui::{Align2, Color32, Pos2, Rect, RichText, Rounding, Stroke, Vec2};
use std::sync::OnceLock;

use crate::gui::theme::Theme;
use crate::gui::widgets::label_placer::{self, LabelPlacer};
use crate::gui::GuiState;

use super::{allocate_canvas, body_color, catalog_text, spectral_color, MapFooter};

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

static NEARBY_STARS: OnceLock<Vec<NearbyStar>> = OnceLock::new();
static MAP_STARS: OnceLock<Vec<MapStar>> = OnceLock::new();

/// One star from `data/stars-map.bin` (HOSMAP01): true galactic cartesian
/// position in LIGHT-YEARS (+X galactic center, +Z north galactic pole),
/// records sorted brightest-first. Generated from the HYG catalog by
/// `scripts/build-stars-map-bin.js`; the format spec lives in its header.
pub(super) struct MapStar {
    pub(super) x: f32,
    pub(super) y: f32,
    pub(super) z: f32,
    pub(super) mag: f32,
    ci: f32,
    /// HYG proper name; only ~360 of ~110k stars carry one.
    pub(super) name: Option<Box<str>>,
}

/// Parse stars-map.bin. Returns None (not empty) on any structural problem
/// so the galaxy view can distinguish "file missing/corrupt" (fall back to
/// the curated nearby list) from "loaded fine".
pub(super) fn parse_stars_map(bytes: &[u8]) -> Option<Vec<MapStar>> {
    if bytes.len() < 16 || &bytes[0..8] != b"HOSMAP01" {
        return None;
    }
    let star_count = u32::from_le_bytes(bytes[8..12].try_into().ok()?) as usize;
    let named_count = u32::from_le_bytes(bytes[12..16].try_into().ok()?) as usize;
    let records_end = 16 + star_count * 20;
    if bytes.len() < records_end {
        return None;
    }
    let f = |off: usize| -> f32 { f32::from_le_bytes(bytes[off..off + 4].try_into().unwrap()) };
    let mut stars = Vec::with_capacity(star_count);
    for i in 0..star_count {
        let o = 16 + i * 20;
        stars.push(MapStar {
            x: f(o),
            y: f(o + 4),
            z: f(o + 8),
            mag: f(o + 12),
            ci: f(o + 16),
            name: None,
        });
    }
    // Names section: (u32 star_index, u8 len, len bytes) x named_count.
    let mut off = records_end;
    for _ in 0..named_count {
        if off + 5 > bytes.len() {
            return None;
        }
        let idx = u32::from_le_bytes(bytes[off..off + 4].try_into().ok()?) as usize;
        let len = bytes[off + 4] as usize;
        off += 5;
        if off + len > bytes.len() || idx >= stars.len() {
            return None;
        }
        stars[idx].name = std::str::from_utf8(&bytes[off..off + len])
            .ok()
            .map(|s| s.into());
        off += len;
    }
    Some(stars)
}

/// The full-catalog map stars, loaded once. Empty slice = file missing or
/// corrupt (the galaxy view then falls back to the curated nearby list).
fn map_stars() -> &'static [MapStar] {
    MAP_STARS.get_or_init(|| {
        let dir = crate::DATA_DIR
            .get()
            .cloned()
            .unwrap_or_else(|| std::path::PathBuf::from("data"));
        let loaded = std::fs::read(dir.join("stars-map.bin"))
            .ok()
            .and_then(|b| parse_stars_map(&b))
            .unwrap_or_default();
        if !loaded.is_empty() {
            log::info!(
                "Maps galaxy view: {} catalog stars loaded ({} named)",
                loaded.len(),
                loaded.iter().filter(|s| s.name.is_some()).count()
            );
        } else {
            log::warn!("Maps galaxy view: stars-map.bin missing/unreadable, using curated nearby list");
        }
        loaded
    })
}

fn nearby_stars() -> &'static [NearbyStar] {
    NEARBY_STARS.get_or_init(|| {
        let text = catalog_text("stars-nearby.json");
        let json = text.as_str();
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
        log::info!("Maps: loaded {} nearby stars", out.len());
        out
    })
}

// ─────────────────────── Galactic view (Sol-centered, ly) ───────────────────

pub(super) fn draw_galactic_view(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let (rect, response, center, zoom) = allocate_canvas(ui, state);
    let paint = ui.painter_at(rect);
    paint.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(5, 5, 12));  // theme-exempt: deep-space backdrop

    let catalog = map_stars();
    // Base scale: 50 ly of radius fills the canvas at zoom 1; the zoom clamp
    // (0.005..200) then spans ~0.25 ly to ~10,000 ly of view radius, the whole
    // catalog. (The old view auto-fit to the ~50-star curated list instead.)
    let half = rect.width().min(rect.height()) as f64 / 2.0 - 30.0;
    let scale = (half / 50.0) * zoom as f64;
    let view_radius_ly = half / scale;

    // Every name on this map goes through one collision pass: Sol first, the
    // footer reserved, then stars brightest-first, then the ring labels, so
    // no two names print over each other.
    let mut labels = LabelPlacer::new(rect.shrink(2.0), label_placer::LABEL_GAP);

    // Distance rings on a 1-2-5 ladder: whichever rings land visibly inside
    // the current view, up to the whole catalog span. Their labels are
    // placed after the stars (`label_rings`), so a ring's "10 ly" never costs
    // a bright star its name.
    const RINGS_LY: [f64; 13] = [
        1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0,
    ];
    let mut rings: Vec<(f32, String)> = Vec::new();
    for &ring_ly in &RINGS_LY {
        let r = (ring_ly * scale) as f32;
        if r > 12.0 && r < rect.width().max(rect.height()) {
            paint.circle_stroke(center, r, Stroke::new(0.5, Color32::from_rgb(25, 25, 40)));  // theme-exempt: distance-ring — faint backdrop
            rings.push((r, format!("{} ly", ring_ly as i64)));
        }
    }

    // Sol at center — the universal anchor.
    let sol_r = 4.0_f32.max(2.0 * zoom.min(2.0));
    paint.circle_filled(center, sol_r, body_color("sun"));
    labels.reserve_exact(Rect::from_center_size(center, Vec2::splat(2.0 * sol_r)));
    let sol_label = paint.text(
        center + Vec2::new(8.0, 0.0),
        Align2::LEFT_CENTER,
        "Sol",
        egui::FontId::proportional(11.0),
        theme.text_primary(),
    );
    labels.reserve(sol_label);

    let hover_pos = ui.input(|i| i.pointer.hover_pos());

    if !catalog.is_empty() {
        // The real catalog (v0.1145): ~110k HYG stars, brightest-first. The
        // deep field earns its keep through TWO cheap gates: a magnitude
        // cutoff that rises as you zoom in (wide view = bright stars only,
        // close view = everything), and the brightest-first ordering, which
        // turns the cutoff into a plain `break`. Viewport culling skips the
        // off-screen rest. Worst case ~20k dots painted, fine for egui.
        let mag_cutoff = (7.5 - 5.0 * (view_radius_ly / 50.0).log10()).clamp(2.0, 21.0);
        let mut drawn = 0usize;
        let mut hovered: Option<(&MapStar, Pos2, f32)> = None;
        // Named stars big enough to label, brightest first (the catalog's
        // order), labelled after every dot is down.
        let mut named: Vec<(&str, Pos2, f32)> = Vec::new();
        let vis = rect.expand(8.0);
        for star in catalog {
            if star.mag > mag_cutoff as f32 {
                break; // sorted brightest-first: nothing dimmer follows
            }
            let px = center.x + (star.x as f64 * scale) as f32;
            let py = center.y - (star.y as f64 * scale) as f32; // screen y grows down
            let pos = Pos2::new(px, py);
            if !vis.contains(pos) {
                continue;
            }
            // Size by margin under the cutoff: the brightest stars in the
            // current view get ~4 px, ones at the cutoff fade in at 0.7 px.
            let headroom = (mag_cutoff as f32 - star.mag).max(0.0);
            let r = (0.7 + headroom * 0.55).min(4.5);
            let [cr, cg, cb] = crate::renderer::stars::ci_to_rgb(star.ci);
            let color = Color32::from_rgb(
                (cr * 255.0) as u8,
                (cg * 255.0) as u8,
                (cb * 255.0) as u8,
            ); // theme-exempt: star color from B-V index — physics, not theme
            paint.circle_filled(pos, r, color);
            drawn += 1;
            if let Some(n) = &star.name {
                // Named stars label once they are comfortably in view.
                if r > 2.0 {
                    named.push((n.as_ref(), pos, r));
                }
            }
            if let Some(hp) = hover_pos {
                let d = (hp - pos).length();
                if d < r + 4.0 && hovered.map_or(true, |(_, _, hd)| d < hd) {
                    hovered = Some((star, pos, d));
                }
            }
        }

        // Footer: what the view is actually showing right now. Laid out now
        // so star names keep off it; painted last, over everything.
        let footer = MapFooter::layout(
            &paint,
            theme,
            rect,
            format!(
                "Top-down galactic plane · HYG catalog, {} stars · showing {} to mag {:.1} within {:.0} ly · scroll to zoom",
                catalog.len(),
                drawn,
                mag_cutoff,
                view_radius_ly,
            ),
        );
        labels.reserve(footer.backing);
        label_stars(&paint, theme, &mut labels, &named);
        label_rings(&paint, &mut labels, center, &rings);

        if let Some((star, _, _)) = hovered {
            let dist = (star.x as f64 * star.x as f64
                + star.y as f64 * star.y as f64
                + star.z as f64 * star.z as f64)
                .sqrt();
            response.on_hover_ui_at_pointer(|ui| {
                ui.set_max_width(300.0);
                let title = star.name.as_deref().unwrap_or("Catalog star");
                ui.label(RichText::new(title).size(theme.font_size_body).color(theme.text_primary()).strong());
                ui.label(RichText::new(format!("{dist:.1} ly from Sol"))
                    .size(theme.font_size_small).color(theme.text_secondary()));
                ui.label(RichText::new(format!("Apparent magnitude {:.2}  ·  B-V {:+.2}", star.mag, star.ci))
                    .size(theme.font_size_small).color(theme.text_muted()));
                ui.label(RichText::new(format!("Galactic position: ({:.1}, {:.1}, {:.1}) ly", star.x, star.y, star.z))
                    .size(theme.font_size_small).color(theme.text_muted()).monospace());
            });
        }

        footer.paint(&paint, theme);
        return;
    }

    // Fallback (stars-map.bin missing/corrupt): the curated nearby list.
    let stars = nearby_stars();
    let mut hovered_star: Option<&NearbyStar> = None;
    let mut named: Vec<(&str, Pos2, f32)> = Vec::new();
    for star in stars {
        let px = center.x + (star.pos_ly.x * scale) as f32;
        let py = center.y - (star.pos_ly.y * scale) as f32;
        let pos = Pos2::new(px, py);
        let r = ((6.0 - star.apparent_magnitude.min(6.0)) as f32 * 0.8 + 1.5).clamp(1.5, 6.0);
        paint.circle_filled(pos, r, spectral_color(&star.spectral));
        if zoom > 1.5 {
            named.push((star.name.as_str(), pos, r));
        }
        if let Some(hp) = hover_pos {
            if (hp - pos).length() < r + 4.0 {
                hovered_star = Some(star);
            }
        }
    }
    let footer = MapFooter::layout(
        &paint,
        theme,
        rect,
        format!("Top-down galactic plane · curated list, {} stars (stars-map.bin not found)", stars.len()),
    );
    labels.reserve(footer.backing);
    // The curated list is nearest-first, not brightest-first: label the
    // brightest first so they keep their names when two collide.
    named.sort_by(|a, b| b.2.total_cmp(&a.2));
    label_stars(&paint, theme, &mut labels, &named);
    label_rings(&paint, &mut labels, center, &rings);

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

    footer.paint(&paint, theme);
}

/// Name the stars in `named` (priority order: the first keeps its name when
/// two collide), each in the first clear spot around its dot: right, left,
/// above, below. The dots of every star in the list are reserved first, so a
/// name never covers another labelled star.
fn label_stars(paint: &egui::Painter, theme: &Theme, labels: &mut LabelPlacer, named: &[(&str, Pos2, f32)]) {
    for &(_, pos, r) in named {
        labels.reserve_exact(Rect::from_center_size(pos, Vec2::splat(2.0 * r)));
    }
    let font = egui::FontId::proportional(9.0);
    for &(name, pos, r) in named {
        let g = paint.layout_no_wrap(name.to_string(), font.clone(), theme.text_secondary());
        if let Some(spot) = labels.place(label_placer::point_candidates(pos, r, g.size(), 2.0)) {
            paint.galley(spot.min, g, theme.text_secondary());
        }
    }
}

/// Label the Galaxy view's distance rings (`(radius px, text)`), each at the
/// first clear spot where the ring crosses a diagonal: upper right (where it
/// always sat), then upper left, lower left, lower right. Placed after the
/// stars so the scale gives way to a star's name rather than the other way
/// round; a ring with no clear diagonal goes unlabelled, and the others still
/// give the scale.
fn label_rings(paint: &egui::Painter, labels: &mut LabelPlacer, center: Pos2, rings: &[(f32, String)]) {
    let ink = Color32::from_rgb(80, 80, 110); // theme-exempt: distance-ring label, faint backdrop
    for (r, text) in rings {
        let g = paint.layout_no_wrap(text.clone(), egui::FontId::proportional(9.0), ink);
        let d = r * 0.7;
        let spots = [Vec2::new(d, -d), Vec2::new(-d, -d), Vec2::new(-d, d), Vec2::new(d, d)]
            .map(|off| Align2::CENTER_CENTER.anchor_size(center + off, g.size()));
        if let Some(spot) = labels.place(spots) {
            paint.galley(spot.min, g, ink);
        }
    }
}
