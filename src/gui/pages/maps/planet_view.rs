//! The Maps page's Planet view: a real Earth region from OpenStreetMap
//! (`data/maps/regions/*.bin`), roads and buildings drawn GPS-map style.
//! Moved out of `gui/pages/cosmos.rs` in Maps increment 0 (2026-10-20).

use egui::{Align2, Color32, Pos2, Rect, RichText, Rounding, Stroke, Vec2};
use std::sync::OnceLock;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::widgets::label_placer::{self, LabelPlacer};
use crate::gui::GuiState;
// The OSM region reader (HOSMREG1) lives in the terrain module, shared with
// the in-world 3D extruder. See src/terrain/osm_region.rs.
use crate::terrain::osm_region::{parse_region, triangulate_ring, OsmRegion, OsmRoad, WaterKind};

use super::{allocate_canvas, MapFooter};

// ─────────────────────── Planet view (OSM regions, meters) ──────────────────

// The HOSMREG1 parser and the region types used to live here, private to
// this page. They moved to `crate::terrain::osm_region` (v0.1148) when the
// in-world 3D extruder became a second reader of the same files: one parser,
// one projection contract, no drift. This view is unchanged otherwise; the
// field names are identical.

static MAP_REGIONS: OnceLock<Vec<OsmRegion>> = OnceLock::new();

/// All installed regions, loaded once (data dir scan). Empty = none
/// installed; the view shows how to fetch one.
fn map_regions() -> &'static [OsmRegion] {
    MAP_REGIONS.get_or_init(|| {
        let dir = crate::DATA_DIR
            .get()
            .cloned()
            .unwrap_or_else(|| std::path::PathBuf::from("data"))
            .join("maps/regions");
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let mut paths: Vec<_> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "bin"))
                .collect();
            paths.sort();
            for p in paths {
                if let Some(region) = std::fs::read(&p).ok().and_then(|b| parse_region(&b)) {
                    log::info!(
                        "Maps planet view: region \"{}\" loaded ({} roads, {} buildings)",
                        region.name,
                        region.roads.len(),
                        region.buildings.len()
                    );
                    out.push(region);
                }
            }
        }
        out
    })
}

/// The Planet view's ground: the map backdrop, island fill, and the halo
/// that lifts street names off the road strokes.
const MAP_GROUND: Color32 = Color32::from_rgb(14, 15, 13); // theme-exempt: map ground backdrop (near-black green), cartography

/// Per-class road style: (stroke width at zoom 1, color). Widths scale with
/// sqrt(zoom) so streets thicken as you close in without becoming ribbons.
fn road_style(class: u8, zoom: f32) -> (f32, Color32) {
    let w = zoom.sqrt().clamp(0.5, 6.0);
    match class {
        0 => (3.2 * w, Color32::from_rgb(235, 170, 80)),  // theme-exempt: cartographic palette (motorway amber), map convention not theme
        1 => (2.6 * w, Color32::from_rgb(220, 205, 160)), // theme-exempt: cartographic palette (primary)
        2 => (2.0 * w, Color32::from_rgb(190, 190, 190)), // theme-exempt: cartographic palette (secondary)
        3 => (1.5 * w, Color32::from_rgb(150, 150, 158)), // theme-exempt: cartographic palette (residential)
        4 => (1.0 * w, Color32::from_rgb(110, 110, 120)), // theme-exempt: cartographic palette (service)
        _ => (0.7 * w, Color32::from_rgb(95, 115, 95)),   // theme-exempt: cartographic palette (footpath green-grey)
    }
}

thread_local! {
    /// Selected region index (page-local view preference).
    static PLANET_REGION_IDX: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(super) fn draw_planet_view(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let regions = map_regions();
    if regions.is_empty() {
        ui.add_space(theme.spacing_lg);
        widgets::card(ui, theme, |ui| {
            ui.label(
                RichText::new("No map regions installed")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary()),
            );
            ui.add_space(theme.spacing_xs);
            ui.label(
                RichText::new(
                    "Fetch any place on Earth as roads + buildings from OpenStreetMap:",
                )
                .color(theme.text_secondary()),
            );
            ui.label(
                RichText::new(
                    "node scripts/fetch-osm-region.mjs --bbox <south,west,north,east> --name \"My Town\" --out data/maps/regions/my-town.bin",
                )
                .size(theme.font_size_small)
                .color(theme.text_muted())
                .monospace(),
            );
            ui.add_space(theme.spacing_xs);
            ui.label(
                RichText::new("The build order for this view: docs/design/maps-ladder.md, rung 3.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        });
        return;
    }

    // Region picker (only when there is a choice to make).
    let mut idx = PLANET_REGION_IDX.with(|c| c.get()).min(regions.len() - 1);
    if regions.len() > 1 {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Region:").color(theme.text_secondary()));
            egui::ComboBox::from_id_salt("planet_region")
                .selected_text(&regions[idx].name)
                .show_ui(ui, |ui| {
                    for (i, r) in regions.iter().enumerate() {
                        if ui.selectable_label(idx == i, &r.name).clicked() {
                            idx = i;
                            state.maps.zoom = 1.0;
                            state.maps.pan = Vec2::ZERO;
                        }
                    }
                });
        });
        PLANET_REGION_IDX.with(|c| c.set(idx));
    }
    let region = &regions[idx];

    let (rect, response, center, zoom) = allocate_canvas(ui, state);
    let paint = ui.painter_at(rect);
    paint.rect_filled(rect, Rounding::ZERO, MAP_GROUND);

    // Meters -> pixels: the region's larger half-extent fills the canvas at
    // zoom 1. Screen north is up (n grows up, screen y grows down).
    let half = rect.width().min(rect.height()) as f64 / 2.0 - 10.0;
    let scale = (half / region.half_east_m.max(region.half_north_m) as f64) * zoom as f64;
    let to_screen = |e: f32, n: f32| -> Pos2 {
        Pos2::new(
            center.x + (e as f64 * scale) as f32,
            center.y - (n as f64 * scale) as f32,
        )
    };
    // Viewport in meters, for culling against precomputed record bounds.
    let view_e = |x: f32| ((x - center.x) as f64 / scale) as f32;
    let view_n = |y: f32| ((center.y - y) as f64 / scale) as f32;
    let (min_e, max_e) = (view_e(rect.left()), view_e(rect.right()));
    let (max_n, min_n) = (view_n(rect.top()), view_n(rect.bottom()));
    let visible = |b: &(f32, f32, f32, f32)| -> bool {
        b.2 >= min_e && b.0 <= max_e && b.3 >= min_n && b.1 <= max_n
    };

    // Water first: the map's bottom layer. Filled through the shared ear
    // clipper (footprint-grade concave rings; egui alone only fills convex
    // shapes); islands repaint the backdrop over their parent water, so
    // file order (parents before their islands) is the painting order. Sea
    // reads a shade deeper than lakes, like any paper chart.
    let mut water_drawn = 0usize;
    for w in &region.water {
        if !visible(&w.bounds) {
            continue;
        }
        let color = match w.kind {
            WaterKind::Sea => Color32::from_rgb(21, 38, 51), // theme-exempt: cartography sea fill
            WaterKind::Inland => Color32::from_rgb(25, 45, 55), // theme-exempt: cartography lake fill
            WaterKind::Island => MAP_GROUND, // island = map backdrop
        };
        let Some(tris) = triangulate_ring(&w.ring) else {
            continue;
        };
        let mut mesh = egui::epaint::Mesh::default();
        for &(e, n) in &w.ring {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: to_screen(e, n),
                uv: egui::epaint::WHITE_UV,
                color,
            });
        }
        mesh.indices = tris;
        paint.add(egui::Shape::mesh(mesh));
        if w.kind != WaterKind::Island {
            water_drawn += 1;
        }
    }

    // Buildings next (under the roads). Outline-only: footprints are often
    // concave and egui only fills convex polygons; a clean stroke reads as
    // GPS-map. Height tints the line (unknown dim, taller lighter). Skip
    // buildings smaller than ~3 px so a zoomed-out view stays uncluttered.
    let min_px = 3.0 / scale as f32;
    let mut buildings_drawn = 0usize;
    for b in &region.buildings {
        if !visible(&b.bounds) || (b.bounds.2 - b.bounds.0).max(b.bounds.3 - b.bounds.1) < min_px {
            continue;
        }
        let pts: Vec<Pos2> = b.ring.iter().map(|&(e, n)| to_screen(e, n)).collect();
        let t = (b.height_m / 60.0).clamp(0.0, 1.0);
        let g = 70 + (t * 70.0) as u8;
        let color = Color32::from_rgb(g, g, g + 8); // theme-exempt: building outline, height-tinted grey — cartography
        paint.add(egui::Shape::closed_line(pts, Stroke::new(0.8, color)));
        buildings_drawn += 1;
    }

    // Roads, minor classes first so majors draw on top. At low zoom skip
    // service/footpaths entirely (class gate), the standard slippy-map ladder.
    let class_gate: u8 = if zoom < 0.7 {
        3
    } else if zoom < 1.6 {
        4
    } else {
        5
    };
    let mut roads_drawn = 0usize;
    for pass_class in (0..=class_gate).rev() {
        for road in region.roads.iter().filter(|r| r.class == pass_class) {
            if !visible(&road.bounds) {
                continue;
            }
            let pts: Vec<Pos2> = road.points.iter().map(|&(e, n)| to_screen(e, n)).collect();
            let (w, color) = road_style(road.class, zoom);
            paint.add(egui::Shape::line(pts, Stroke::new(w, color)));
            roads_drawn += 1;
        }
    }

    // Scale bar geometry and the footer are laid out BEFORE the names so the
    // label pass can keep off them; both are painted last, on top.
    let target_m = 100.0 / scale;
    let nice = [10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0]
        .into_iter()
        .min_by(|a: &f64, b: &f64| {
            (a / target_m - 1.0).abs().partial_cmp(&(b / target_m - 1.0).abs()).unwrap()
        })
        .unwrap_or(100.0);
    let bar_px = (nice * scale) as f32;
    let bar_y = rect.bottom() - 18.0;
    let bar_x = rect.right() - 16.0 - bar_px;
    let bar_label = if nice >= 1000.0 {
        format!("{:.0} km", nice / 1000.0)
    } else {
        format!("{nice:.0} m")
    };
    // Footer: region + stats + the ODbL attribution OpenStreetMap requires.
    // The notice text comes from credits::OSM_NOTICE, shared with the in-world
    // credit, so the two surfaces that draw OSM data cannot drift apart.
    let footer = MapFooter::layout(
        &paint,
        theme,
        rect,
        format!(
            "{} ({:.4}, {:.4}) · {} roads, {} buildings, {} water in view · {}",
            region.name,
            region.origin_lat,
            region.origin_lon,
            roads_drawn,
            buildings_drawn,
            water_drawn,
            crate::credits::OSM_NOTICE,
        ),
    );

    // Street and water names, through one collision pass so no two print
    // over each other (2026-09-27 snapshot review). Priority is road class
    // (motorway first), then the longer road; water names rank just after
    // primary roads. A road offers five spots along its length, middle
    // first, so its name can slide clear of a crossing street; a name with no
    // clear spot is dropped. The same name is not repeated within
    // `MIN_REPEAT_PX` (OSM splits one street into many ways, each named).
    // Capped, as before, so the map never becomes a word cloud.
    const MIN_REPEAT_PX: f32 = 220.0;
    const MAX_ROAD_LABELS: usize = 60;
    const MAX_WATER_LABELS: usize = 20;
    let road_ink = Color32::from_rgb(210, 210, 200); // theme-exempt: cartographic label, must read over both roads and ground
    let water_ink = Color32::from_rgb(150, 185, 205); // theme-exempt: cartographic water label
    struct MapName<'a> {
        rank: f32,
        length: f32,
        name: &'a str,
        spots: Vec<Pos2>,
        water: bool,
    }
    let mut names: Vec<MapName> = Vec::new();
    if zoom >= 1.6 {
        let label_classes: u8 = if zoom >= 4.0 { 5 } else { 2 };
        for road in &region.roads {
            let (Some(name), true) = (&road.name, road.class <= label_classes) else {
                continue;
            };
            if !visible(&road.bounds) {
                continue;
            }
            let pts: Vec<Pos2> = road.points.iter().map(|&(e, n)| to_screen(e, n)).collect();
            names.push(MapName {
                rank: road.class as f32,
                length: label_placer::polyline_length(&pts),
                name: name.as_ref(),
                spots: label_placer::along_polyline(&pts, &[0.5, 0.3, 0.7, 0.15, 0.85]),
                water: false,
            });
        }
    }
    // Named water (lakes, bays): centred on the polygon bounds, in the
    // blue-grey every chart uses for water names.
    if zoom >= 0.7 {
        for w in &region.water {
            let (Some(name), true) = (&w.name, w.kind != WaterKind::Island) else {
                continue;
            };
            if !visible(&w.bounds) {
                continue;
            }
            let (a, b) = (to_screen(w.bounds.0, w.bounds.1), to_screen(w.bounds.2, w.bounds.3));
            names.push(MapName {
                rank: 1.5,
                length: (b - a).length(),
                name: name.as_ref(),
                spots: vec![to_screen((w.bounds.0 + w.bounds.2) * 0.5, (w.bounds.1 + w.bounds.3) * 0.5)],
                water: true,
            });
        }
    }
    names.sort_by(|a, b| a.rank.total_cmp(&b.rank).then(b.length.total_cmp(&a.length)));
    let mut labels = LabelPlacer::new(rect.shrink(2.0), label_placer::LABEL_GAP);
    labels.reserve(footer.backing);
    labels.reserve(Rect::from_min_max(
        Pos2::new(bar_x, bar_y - 18.0),
        Pos2::new(bar_x + bar_px, bar_y + 2.0),
    ));
    let label_font = egui::FontId::proportional(9.5);
    let mut placed: Vec<(&str, Pos2)> = Vec::new();
    let (mut road_labels, mut water_labels) = (0usize, 0usize);
    for n in &names {
        let (count, cap, ink) = if n.water {
            (&mut water_labels, MAX_WATER_LABELS, water_ink)
        } else {
            (&mut road_labels, MAX_ROAD_LABELS, road_ink)
        };
        if *count >= cap {
            continue;
        }
        let g = paint.layout_no_wrap(n.name.to_string(), label_font.clone(), ink);
        let size = g.size();
        let spots = n.spots.iter().filter(|p| {
            !placed.iter().any(|(other, at)| *other == n.name && (*at - **p).length() < MIN_REPEAT_PX)
        });
        let Some(spot) = labels.place(spots.map(|p| Align2::CENTER_CENTER.anchor_size(*p, size))) else {
            continue;
        };
        // A thin halo in the ground colour lifts the name off the road
        // strokes under it, the way printed street maps do.
        for off in [Vec2::new(-1.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, -1.0), Vec2::new(0.0, 1.0)] {
            paint.galley_with_override_text_color(spot.min + off, g.clone(), MAP_GROUND);
        }
        paint.galley(spot.min, g, ink);
        placed.push((n.name, spot.center()));
        *count += 1;
    }

    // Hover: nearest named road within reach.
    if let Some(hp) = hover_named_road(region, ui, &to_screen) {
        response.clone().on_hover_ui_at_pointer(|ui| {
            ui.label(
                RichText::new(hp)
                    .size(theme.font_size_small)
                    .color(theme.text_primary()),
            );
        });
    }

    // Scale bar: a round-number length that spans 60-140 px at this zoom
    // (measured above, with the names).
    paint.line_segment(
        [Pos2::new(bar_x, bar_y), Pos2::new(bar_x + bar_px, bar_y)],
        Stroke::new(2.0, theme.text_secondary()),
    );
    paint.text(
        Pos2::new(bar_x + bar_px / 2.0, bar_y - 4.0),
        Align2::CENTER_BOTTOM,
        bar_label,
        egui::FontId::proportional(10.0),
        theme.text_secondary(),
    );

    footer.paint(&paint, theme);
}

/// The nearest named road within ~7 px of the pointer, by segment distance.
fn hover_named_road(
    region: &OsmRegion,
    ui: &egui::Ui,
    to_screen: &dyn Fn(f32, f32) -> Pos2,
) -> Option<String> {
    let hp = ui.input(|i| i.pointer.hover_pos())?;
    let mut best: Option<(f32, &OsmRoad)> = None;
    for road in &region.roads {
        let Some(_) = road.name else { continue };
        for seg in road.points.windows(2) {
            let a = to_screen(seg[0].0, seg[0].1);
            let b = to_screen(seg[1].0, seg[1].1);
            let ab = b - a;
            let t = ((hp - a).dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
            let d = (a + ab * t - hp).length();
            if d < 7.0 && best.map_or(true, |(bd, _)| d < bd) {
                best = Some((d, road));
            }
        }
    }
    best.and_then(|(_, r)| r.name.as_ref().map(|n| n.to_string()))
}
