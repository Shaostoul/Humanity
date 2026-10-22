//! The Maps page's Solar System view: the Sun's family in 3D through the
//! map's camera (`camera.rs`: an anchor, an f64 offset, 10 m to 1e14 m), over
//! the WORLD layer the engine renders for it (`you_layer.rs`,
//! `engine/map_view.rs`: the ship and Earth for real up close, the real star
//! sky beyond), you and your ship, the sky-events readout, and the info card
//! a body's name opens (Focus, Track).
//! Moved out of `gui/pages/cosmos.rs` in Maps increment 0 (2026-10-20). Maps
//! increment 1 (2026-10-21) moved it from AU around the Sun to metres around
//! Earth, the engine's own frame, so the ship and the planets are placed by
//! one set of numbers.
//!
//! Maps increment 2 (2026-10-21), "a solar system that looks real" (the
//! operator, 2026-10-20: "It just looks like a bunch of lines with a few
//! colored dots"): the symbols are built here from their own files, in this
//! order, back to front: the Sun's glare (`bodies.rs`), the belts' dust from
//! 20,000 real orbits (`belts.rs`), the orbit trails (`orbits.rs`), the
//! overlays from data (`overlays.rs`), every body as a phase-lit disc of its
//! real colour (`bodies.rs`), you and your ship (`you_layer.rs`), then the
//! names through the label placer (`labels.rs`). Diagram mode (`diagram.rs`)
//! re-places all of it. The whole build is timed as `cpu.maps_symbology`.

use egui::epaint::Mesh;
use egui::{Align2, Color32, Pos2, Rect, RichText, Sense, Shape, Vec2};

use crate::cosmos::{body_world_position_3d_au, eccentric_anomaly_at, find_body, orbit_axes_au, sol_bodies, SolBody, M_PER_AU};
use crate::gui::theme::Theme;
use crate::gui::widgets::label_placer::{LabelPlacer, LABEL_GAP};
use crate::gui::GuiState;

use super::bodies;
use super::camera::{MapProjector, MapScene, FOV_Y_RAD};
use super::labels::{self, LabelOffer};
use super::orbits;
use super::overlays::{self, Band};
use super::sky_events::{
    detect_sky_events, format_geo, scan_upcoming_sky_events, subpoint_lat_lon_deg, ConjunctionTightness,
};
use super::time_bar::{current_real_time_seconds_since_j2000, draw_time_controls, format_sim_time};
use super::you_layer::{self, WorldDrawn};
use super::{controls, format_km, format_with_commas, titlecase};

/// How far to stand from a body a Focus flies to: its disc about a sixth of
/// the view's height (and never inside 10 km).
fn focus_distance_m(scene: &MapScene, body: &SolBody) -> f64 {
    (scene.radius_m(body) / (FOV_Y_RAD * 0.5).tan() * 6.0).max(1.0e4)
}

/// One body as this frame draws it.
struct Drawn<'a> {
    body: &'a SolBody,
    screen: Pos2,
    depth: f64,
    world_m: glam::DVec3,
    /// Its true (or diagram) radius on screen, and the radius it is drawn at.
    true_r: f32,
    r: f32,
}

// ─────────────────────── System view ────────────────────────────────────────

pub(super) fn draw_system_view(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // ── Initialize sim_time to current real-world time on first open
    //    so the user immediately sees today's planetary configuration
    //    (operator 2026-05-10). While Live (the default since Maps
    //    increment 1) the feed keeps it on the engine's own sky clock
    //    instead; `controls::advance` runs the page's clock when detached. ──
    if !state.maps.sim_time_initialized {
        state.maps.sim_time_seconds = current_real_time_seconds_since_j2000();
        state.maps.sim_time_initialized = true;
    }

    // ── Time controls panel. Any scrub, skip or speed change takes the
    //    clock off Live (design 5.9); "Live" puts it back. ──
    let clock_before = (state.maps.sim_time_seconds, state.maps.sim_speed, state.maps.live);
    draw_time_controls(ui, theme, state);
    if state.maps.live && clock_before.2 && (state.maps.sim_time_seconds, state.maps.sim_speed) != (clock_before.0, clock_before.1) {
        state.maps.live = false;
    }
    let sim_time = state.maps.sim_time_seconds;

    // ── The canvas. The camera input is applied at the END of the draw
    //    (`controls`), so everything below is drawn from the same pose the
    //    engine rendered the world layer from this frame. ──
    let available = ui.available_size();
    let (rect, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
    let paint = ui.painter_at(rect);
    let _cost = crate::renderer::frame_costs::stage("cpu.maps_symbology");
    let started = std::time::Instant::now();
    controls::note_canvas(ui, rect, state);
    let me = state.maps.me.clone();
    let scene = MapScene::new(me.as_ref(), sim_time, state.maps.live).with_diagram(state.maps.diagram);
    let proj = MapProjector::new(state.maps.camera.pose(&scene), rect);
    let band = Band::at(proj.pose.distance);
    let world = you_layer::draw_world_backdrop(&paint, rect, &state.maps);
    if world == WorldDrawn::Nothing {
        paint.rect_filled(rect, 0.0, theme.map_space());
    }
    let globes_drawn = world == WorldDrawn::Everything;
    let mut stats = crate::gui::MapSymbology { band: band.name().to_string(), ..Default::default() };

    // ── Focus requests fly the camera to the body and anchor it there, so
    //    it follows the body round its orbit. ──
    if let Some(focus_id) = state.maps.focus_request.take() {
        if focus_should_clear_tracking(state.maps.tracked_body.as_deref(), &focus_id) {
            state.maps.tracked_body = None;
        }
        if let Some(body) = find_body(&focus_id) {
            state.maps.camera.focus_body(&focus_id, focus_distance_m(&scene, body), &scene);
        }
        ui.ctx().request_repaint();
    }

    // ── Sky events: conjunctions and eclipses now, and the cached forward
    //    scan (re-run only when the clock has moved 12 h, at most every
    //    300 ms of wall time). ──
    let (conjunctions, eclipse) = detect_sky_events(sim_time);
    {
        let drifted = !state.maps.upcoming_scan_origin.is_finite() || (sim_time - state.maps.upcoming_scan_origin).abs() > 12.0 * 3600.0;
        let wall_ok = state.maps.upcoming_last_scan.map(|t| t.elapsed().as_millis() >= 300).unwrap_or(true);
        if drifted && wall_ok {
            state.maps.upcoming_events = scan_upcoming_sky_events(sim_time);
            state.maps.upcoming_scan_origin = sim_time;
            state.maps.upcoming_last_scan = Some(std::time::Instant::now());
        }
    }
    let mut body_tightness: std::collections::HashMap<&str, ConjunctionTightness> = std::collections::HashMap::new();
    for ev in &conjunctions {
        if eclipse.is_some() && (ev.body_a_id == "sun" || ev.body_b_id == "sun") && (ev.body_a_id == "moon" || ev.body_b_id == "moon") {
            continue;
        }
        for id in [ev.body_a_id.as_str(), ev.body_b_id.as_str()] {
            let entry = body_tightness.entry(id).or_insert(ev.tightness);
            if ev.tightness > *entry {
                *entry = ev.tightness;
            }
        }
    }

    // ── Names keep off the controls: the top strip (the follow row and the
    //    layer bar) and the two readouts at the bottom, laid out now and
    //    painted last. ──
    let mut placer = LabelPlacer::new(rect.shrink(2.0), LABEL_GAP);
    placer.reserve(Rect::from_min_max(rect.min, Pos2::new(rect.right(), rect.top() + 64.0)));
    let hud = Hud::layout(ui, theme, state, rect, &proj, (sim_time, &conjunctions, eclipse.as_ref()));
    for r in hud.rects() {
        placer.reserve(r);
    }

    // ── Every body, projected, back to front. ──
    let bodies_all = sol_bodies();
    let mut drawn: Vec<Drawn> = Vec::with_capacity(bodies_all.len());
    for body in bodies_all {
        let Some(world_m) = scene.body_m(&body.id) else { continue };
        let Some((screen, depth)) = proj.project(world_m) else { continue };
        let true_r = proj.radius_px(scene.radius_m(body), depth);
        let r = true_r.max(bodies::icon_floor_px(&body.id, &body.body_type)).min(rect.width().max(rect.height()) * 3.0);
        drawn.push(Drawn { body, screen, depth, world_m, true_r, r });
    }
    drawn.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal));
    let sun_m = scene.body_m("sun").unwrap_or_default();
    let hover_pos = ui.input(|i| i.pointer.hover_pos());

    // ── 1. The Sun's glare, behind everything (the world layer has the real
    //    Sun in it when it draws everything). ──
    if !globes_drawn {
        if let Some(sun) = drawn.iter().find(|d| d.body.id == "sun") {
            let light = bodies::icon_lit_linear(sun.body.map_color);
            let deg_per_px = (proj.pose.fov_y.to_degrees() / rect.height().max(1.0) as f64) as f32;
            let mut glare = Mesh::default();
            bodies::add_sun_glare(&mut glare, sun.screen, sun.r, deg_per_px, light);
            paint.add(Shape::mesh(glare));
        }
    }

    // ── 2. The belts' dust. ──
    if overlays::showing(state, "belts", band) {
        let set = super::belts::belt_set();
        state.maps.belts.update(set, sim_time, state.maps.diagram);
        let (dust, drawn) = super::belts::dust_mesh(set, &mut state.maps.belts, &proj, &scene, theme.map_belt(), theme.map_belt_outer());
        stats.belt_points = drawn;
        paint.add(Shape::Mesh(dust));
        stats.layers.push("belts".into());
    }

    // ── Hover and click on the bodies themselves; which bodies are FORCED
    //    (hovered, selected, the one the camera is on), whose orbit and name
    //    show whatever the rules below say; and whether the camera is close
    //    enough, in the solar-system band and within 1.5 AU, for the
    //    asteroids' and comets' own. ──
    let mut hovered_body: Option<&SolBody> = None;
    let mut clicked_body: Option<&SolBody> = None;
    for d in &drawn {
        if let Some(hp) = hover_pos {
            if (hp - d.screen).length() < d.r + 4.0 {
                hovered_body = Some(d.body);
                if response.clicked() {
                    clicked_body = Some(d.body);
                }
            }
        }
    }
    let anchor_id = match &state.maps.camera.anchor {
        super::camera::MapAnchor::Body(id) => Some(id.clone()),
        _ => None,
    };
    let (hovered_id, selected_id) = (hovered_body.map(|b| b.id.clone()), state.maps.selected_body.clone());
    let forced = move |id: &str| [&hovered_id, &selected_id, &anchor_id].iter().any(|f| f.as_deref() == Some(id));
    let near = band == Band::SolarSystem && proj.pose.distance < 1.5 * M_PER_AU;

    // ── 3. Orbit trails. An asteroid's or a comet's only when it is forced
    //    or the camera is near: twenty eccentric ellipses across the inner
    //    system were the "bunch of lines" itself (the first overview
    //    snapshot, 2026-10-21). ──
    // How far each moon stands from its planet on screen (whether its name
    // is offered, `labels::offered`), with or without the orbits drawn.
    let screen_of: std::collections::HashMap<&str, Pos2> = drawn.iter().map(|d| (d.body.id.as_str(), d.screen)).collect();
    let moon_spread: std::collections::HashMap<&str, f32> = drawn
        .iter()
        .filter(|d| d.body.body_type == "moon")
        .filter_map(|d| {
            let p = screen_of.get(d.body.parent.as_deref()?)?;
            Some((d.body.id.as_str(), (d.screen - *p).length()))
        })
        .collect();
    if overlays::showing(state, "orbits", band) {
        let mut lines = Mesh::default();
        for body in bodies_all {
            let Some(parent_id) = body.parent.as_deref() else { continue };
            if matches!(body.body_type.as_str(), "asteroid" | "comet" | "artificial") && !near && !forced(&body.id) {
                continue;
            }
            let Some(base) = scene.body_m(parent_id) else { continue };
            let Some((ax, bx, e)) = orbit_axes_au(body, sim_time) else { continue };
            let parent_depth = proj.project(base).map(|(_, d)| d).unwrap_or(proj.pose.distance);
            // The orbit's point at an eccentric anomaly, in the map frame, from
            // values worked out once per orbit (a sample is a few multiplies):
            // round the Sun through the scene's scale (the Diagram's log
            // spacing), round a planet spaced off its enlarged disc on the
            // Diagram (`MapScene::near_m`'s rule).
            let earth_au = scene.earth_display_au();
            let parent_km = find_body(parent_id).map(|p| p.radius_km).unwrap_or(1.0);
            let diagram = scene.diagram;
            let eval = |ea: f64| -> glam::DVec3 {
                let rel_au = ax * (ea.cos() - e) + bx * ea.sin();
                match (parent_id == "sun", diagram) {
                    (true, false) => (rel_au - earth_au) * M_PER_AU,
                    (true, true) => (super::diagram::helio_au(rel_au) - earth_au) * M_PER_AU,
                    (false, false) => base + rel_au * M_PER_AU,
                    (false, true) => base + super::diagram::near_offset(parent_km, rel_au * M_PER_AU),
                }
            };
            // How big the orbit is on screen: half its long axis, at the
            // nearer of its centre and its body (zoomed in beside Earth, its
            // orbit is a near-straight line through it, however small the
            // Sun's end of it looks).
            let a_disp = (eval(0.0) - eval(std::f64::consts::PI)).length() * 0.5;
            let body_depth = scene.body_m(&body.id).and_then(|p| proj.project(p)).map(|(_, d)| d).unwrap_or(parent_depth);
            let a_px = proj.radius_px(a_disp, parent_depth.min(body_depth).max(proj.pose.distance * 1e-6));
            let fade = orbits::size_fade(a_px, rect.height()) * orbits::scale_fade(a_disp, proj.pose.distance);
            if fade < 0.02 {
                continue;
            }
            let ea_body = eccentric_anomaly_at(body, sim_time);
            let samples = orbits::sample_adaptive(&eval, &proj, ea_body);
            let m_of = |ea: f64| ea - e * ea.sin();
            let m_body = m_of(ea_body);
            let (colour, width) = match body.body_type.as_str() {
                "terrestrial" | "gas_giant" | "ice_giant" => (theme.map_orbit_planet(), theme.map_orbit_width),
                "moon" => (theme.map_orbit_moon(), theme.map_orbit_width * 0.8),
                _ => (theme.map_orbit_minor(), theme.map_orbit_width * 0.65),
            };
            let width = width * fade.max(0.6);
            // Unbroken runs (a point behind the camera breaks the line).
            let mut run: Vec<(Pos2, f32)> = Vec::new();
            let flush = |run: &mut Vec<(Pos2, f32)>, lines: &mut Mesh| {
                orbits::add_polyline(lines, run, colour, width);
                run.clear();
            };
            let last = samples.len().saturating_sub(1);
            for (i, s) in samples.iter().enumerate() {
                match s.at {
                    Some(p) => {
                        // Ahead of the body round to it from behind: the trail
                        // is brightest on arrival (the last sample).
                        let ahead = (m_of(s.ea) - m_body) / std::f64::consts::TAU;
                        let behind = if i == 0 { 1.0 } else if i == last { 0.0 } else { 1.0 - ahead };
                        run.push((p, orbits::trail_alpha(behind) * fade));
                    }
                    None => flush(&mut run, &mut lines),
                }
            }
            flush(&mut run, &mut lines);
            stats.orbits += 1;
        }
        stats.orbit_vertices = lines.vertices.len() as u32;
        paint.add(Shape::mesh(lines));
        stats.layers.push("orbits".into());
    }

    // ── 4. Overlays from data: reference rings, Lagrange points. ──
    let mut ref_hover = None;
    let mut lag_hover = None;
    let mut overlay_labels = Vec::new();
    if overlays::showing(state, "reference_orbits", band) {
        ref_hover = overlays::draw_reference_orbits(&paint, theme, &proj, &scene, &mut overlay_labels, hover_pos);
        stats.layers.push("reference_orbits".into());
    }
    if overlays::showing(state, "lagrange", band) {
        lag_hover = overlays::draw_lagrange(&paint, theme, &proj, &scene, (&mut placer, &mut overlay_labels), hover_pos);
        stats.layers.push("lagrange".into());
    }

    // ── The open card: its body keeps a pill (the card hangs from it). ──
    use crate::gui::widgets::body_pill::{compute_pill_layout, paint_pill_backgrounds, paint_pill_overlays, BodyPill};
    let pills: Vec<BodyPill> = drawn
        .iter()
        .filter(|d| state.maps.expanded_body.as_deref() == Some(d.body.id.as_str()))
        .map(|d| BodyPill {
            id: d.body.id.as_str(),
            name: d.body.name.as_str(),
            color: bodies::map_color32(d.body.map_color),
            body_screen: d.screen,
            body_radius_px: d.r.clamp(3.0, 40.0),
            priority: 0,
            forced: true,
            expanded: true,
        })
        .collect();
    let pill_layout = compute_pill_layout(&paint, theme, &pills);
    if let Some(pp) = pill_layout.placed.first().cloned() {
        placer.reserve(pp.rect);
        if let Some(d) = drawn.iter().find(|d| d.body.id == pp.id) {
            draw_body_info_card_v2(ui, &paint, theme, d.body, &pp, rect, state);
        }
    }
    paint_pill_backgrounds(&paint, theme, &pill_layout);

    // ── 5. The bodies: phase-lit discs of their real colour, the Sun its
    //    own limb-darkened light; an icon of a minimum size when small, giving
    //    way to the real globe where the world layer draws one. ──
    // Not drawn unless forced: an asteroid or a comet outside the asteroids'
    // own range (the belts' dust stands for them), and a moon sitting on its
    // own planet (the 60 AU overview's centre was a heap of grey moon and
    // asteroid icons, 2026-10-21).
    let r_of: std::collections::HashMap<&str, f32> = drawn.iter().map(|d| (d.body.id.as_str(), d.r)).collect();
    let hidden = |d: &Drawn| {
        if forced(&d.body.id) {
            return false;
        }
        let minor = matches!(d.body.body_type.as_str(), "asteroid" | "comet" | "artificial");
        let on_parent = d.body.body_type == "moon"
            && moon_spread.get(d.body.id.as_str()).copied().unwrap_or(0.0)
                < d.body.parent.as_deref().and_then(|p| r_of.get(p)).copied().unwrap_or(0.0) + d.r + 1.0;
        (minor && !near) || on_parent
    };
    let mut discs = Mesh::default();
    for d in drawn.iter().filter(|d| !hidden(d)) {
        if d.body.id == "sun" {
            if !globes_drawn || d.true_r < bodies::HANDOFF_TO_PX {
                bodies::add_sun_disc(&mut discs, d.screen, d.r, bodies::icon_lit_linear(d.body.map_color));
            }
        } else {
            let alpha = bodies::icon_alpha(d.true_r, globes_drawn);
            if alpha <= 0.0 {
                continue;
            }
            let sun_view = bodies::sun_in_view(&proj.pose, d.world_m, sun_m);
            bodies::add_phase_disc(&mut discs, d.screen, d.r, bodies::icon_lit_linear(d.body.map_color), sun_view, alpha);
        }
        // A small disc keeps names off it; a big one (Earth filling the view)
        // is a backdrop names may sit over, their dark edge keeps them readable.
        if d.r < 30.0 {
            placer.reserve_exact(Rect::from_center_size(d.screen, Vec2::splat(2.0 * d.r)));
        }
        stats.bodies += 1;
    }
    paint.add(Shape::mesh(discs));
    paint_body_events(&paint, theme, &drawn, &body_tightness, eclipse.as_ref());
    let clicked_pill = paint_pill_overlays(ui, &paint, theme, &pill_layout, "maps_pill");

    // ── You and your ship (`you_layer.rs`): its orbit, the ship, its name
    //    first through the placer, the mining asteroids; at ship scale with
    //    no world layer, why the ship is not drawn for real. ──
    let asteroid_markers = you_layer::draw_you(&paint, theme, &proj, &scene, state, world, &mut placer);
    if world != WorldDrawn::Everything && you_layer::near_the_ship(&state.maps) {
        if let Some(why) = you_layer::world_layer_want(&state.maps).reason() {
            paint.text(rect.center_top() + Vec2::new(0.0, 70.0), Align2::CENTER_TOP, why, egui::FontId::proportional(12.0), theme.text_secondary());
        }
    }

    // ── 6. The names, through the placer, most important first. ──
    let mut label_rects: Vec<(String, Rect)> = Vec::new();
    if overlays::showing(state, "names", band) {
        let mut offers = Vec::new();
        for d in &drawn {
            let id = d.body.id.as_str();
            if state.maps.expanded_body.as_deref() == Some(id) {
                continue; // its pill names it
            }
            let forced = forced(id);
            let spread = moon_spread.get(id).copied().unwrap_or(0.0);
            if !labels::offered(&d.body.body_type, spread, near, forced) {
                continue;
            }
            offers.push(LabelOffer {
                id: id.to_string(),
                text: d.body.name.clone(),
                at: d.screen,
                r: d.r,
                priority: labels::priority(id, &d.body.body_type, d.true_r, forced),
                minor: !matches!(d.body.body_type.as_str(), "star" | "terrestrial" | "gas_giant" | "ice_giant" | "dwarf_planet"),
            });
        }
        let offered = offers.len();
        let (placed, dropped) = labels::place_and_paint(&paint, theme, &mut placer, offers);
        stats.labels_placed = placed.len() as u32;
        stats.labels_dropped = dropped;
        debug_assert_eq!(placed.len() + dropped as usize, offered);
        label_rects = placed;
        stats.layers.push("names".into());
    }
    // The overlays' own names last: where they still fit.
    overlays::place_overlay_labels(&paint, &mut placer, overlay_labels);

    // ── Clicks: an asteroid marker first (small targets), then the open
    //    card's pill, then a name, then a body. ──
    let click_at = if response.clicked() { response.interact_pointer_pos() } else { None };
    let clicked_asteroid = click_at.and_then(|p| asteroid_markers.iter().find(|(_, mp)| mp.distance(p) < 10.0).cloned());
    let clicked_label = click_at.and_then(|p| label_rects.iter().find(|(_, r)| r.contains(p)).map(|(id, _)| id.clone()));
    if let Some((ast_id, _)) = clicked_asteroid {
        state.pending_open_mining_modal = Some(ast_id);
        state.active_page = crate::gui::GuiPage::Inventory;
    } else if let Some(id) = clicked_pill.or(clicked_label) {
        if state.maps.expanded_body.as_deref() == Some(id.as_str()) {
            state.maps.expanded_body = None;
        } else {
            state.maps.expanded_body = Some(id.clone());
            state.maps.selected_body = Some(id);
        }
    } else if let Some(b) = clicked_body {
        state.maps.selected_body = Some(b.id.clone());
    }

    // The camera's mouse and keys, now that everything is drawn from the
    // pose the world layer was rendered from (`controls`).
    controls::handle_input(ui, &response, rect, state);

    // ── Hover texts: a reference ring, a Lagrange point, a body. ──
    if let Some((name, about)) = ref_hover {
        response.clone().on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(240.0);
            ui.label(RichText::new(name).strong().color(theme.text_primary()));
            ui.label(RichText::new(about).size(theme.font_size_small).color(theme.text_secondary()));
        });
    } else if let Some((title, notable, about)) = lag_hover {
        response.clone().on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(240.0);
            ui.label(RichText::new(title).strong().color(theme.text_primary()));
            if !notable.is_empty() {
                ui.label(RichText::new(notable).size(theme.font_size_small).color(theme.text_secondary()));
            }
            ui.label(RichText::new(about).size(theme.font_size_small).color(theme.text_muted()).italics());
        });
    } else if let (Some(body), Some(_)) = (hovered_body, hover_pos) {
        response.on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(280.0);
            ui.label(RichText::new(&body.name).size(theme.font_size_body).color(theme.text_primary()).strong());
            ui.label(RichText::new(format!("{} · {}", titlecase(&body.body_type), distance_line(body))).size(theme.font_size_small).color(theme.text_secondary()));
            if body.radius_km > 0.0 {
                ui.label(RichText::new(format!("Radius: {}", format_km(body.radius_km))).size(theme.font_size_small).color(theme.text_muted()));
            }
            ui.label(RichText::new("Click its name to open details · drag to rotate · scroll to zoom · shift+drag to pan").size(theme.font_size_small).color(theme.accent()).italics());
        });
    }

    // ── The readouts, the Diagram banner, the credit, the switches. ──
    hud.paint(&paint);
    if state.maps.diagram {
        labels::paint_with_halo(&paint, theme, rect.center_top() + Vec2::new(-150.0, 70.0), "Diagram: distances and sizes are not to scale".to_string(), 13.0, theme.warning());
    }
    if stats.belt_points > 0 {
        let credit = "Belts: real orbits from the Minor Planet Center (IAU MPCORB)";
        paint.text(Pos2::new(rect.right() - 8.0, rect.top() + 66.0), Align2::RIGHT_TOP, credit, egui::FontId::proportional(9.5), theme.text_muted());
    }
    overlays::draw_layer_bar(ui, theme, state, rect, band);
    controls::draw_follow_row(ui, theme, state, rect);
    stats.ms = started.elapsed().as_secs_f32() * 1000.0;
    state.maps.symbology = stats;
    controls::advance(state);
}

/// "5.20 AU from Sun" or "384,400 km from Earth".
fn distance_line(body: &SolBody) -> String {
    if body.semi_major_axis_au > 0.0 {
        format!("{:.2} AU from Sun", body.semi_major_axis_au)
    } else if body.semi_major_axis_km > 0.0 {
        format!("{} km from {}", format_with_commas(body.semi_major_axis_km as i64), body.parent.as_deref().map(titlecase).unwrap_or_default())
    } else {
        String::new()
    }
}

/// The conjunction rings and the eclipse marks over the bodies.
fn paint_body_events(
    paint: &egui::Painter,
    theme: &Theme,
    drawn: &[Drawn],
    tightness: &std::collections::HashMap<&str, ConjunctionTightness>,
    eclipse: Option<&super::sky_events::EclipseEvent>,
) {
    use egui::Stroke;
    for d in drawn {
        let r = d.r;
        if let Some(t) = tightness.get(d.body.id.as_str()) {
            let w = t.weight();
            paint.circle_stroke(d.screen, r + 6.0 + w * 4.0, Stroke::new(1.0 + w * 2.0, theme.warning()));
            paint.circle_stroke(d.screen, r + 3.0, Stroke::new(0.5, theme.warning()));
        }
        let Some(ev) = eclipse else { continue };
        match d.body.id.as_str() {
            "earth" => {
                paint.circle_stroke(d.screen, r + 8.0, Stroke::new(2.0, theme.danger()));
                paint.text(d.screen + Vec2::new(0.0, r + 22.0), Align2::CENTER_TOP, "ECLIPSE", egui::FontId::proportional(11.0), theme.danger());
            }
            "sun" => {
                let dark_r = (r as f64 * ev.coverage.sqrt()).clamp(0.0, r as f64) as f32;
                paint.circle_filled(d.screen, dark_r, theme.bg_primary());
                paint.circle_stroke(d.screen, r + 2.0, Stroke::new(1.0, theme.danger()));
            }
            "moon" => {
                paint.circle_stroke(d.screen, r + 4.0, Stroke::new(1.5, theme.danger()));
            }
            _ => {}
        }
    }
}

// ─────────────────────── The readouts at the bottom ─────────────────────────

/// The clock and this frame's sky events: (seconds since J2000, the
/// conjunctions now, an eclipse now).
type SkyNow<'a> = (f64, &'a [super::sky_events::ConjunctionEvent], Option<&'a super::sky_events::EclipseEvent>);

/// The camera lines at the bottom-left and the sky-events list at the
/// bottom-right, laid out first (so the names keep off them) and painted
/// last (so they sit on top).
struct Hud {
    cam: (Pos2, std::sync::Arc<egui::Galley>),
    help: (Pos2, std::sync::Arc<egui::Galley>),
    sky: Vec<(Pos2, std::sync::Arc<egui::Galley>)>,
}

impl Hud {
    /// `sky` is the clock and this frame's sky events.
    fn layout(ui: &egui::Ui, theme: &Theme, state: &GuiState, rect: Rect, proj: &MapProjector, sky: SkyNow) -> Self {
        let font = egui::FontId::proportional(10.0);
        let wrap = (rect.width() - 16.0).max(40.0);
        let cam_text = format!(
            "Camera {} from {} · yaw {:+.0}° · pitch {:+.0}°",
            format_distance(proj.pose.distance),
            anchor_name(&state.maps),
            state.maps.camera.yaw.to_degrees(),
            state.maps.camera.pitch.to_degrees()
        );
        let cam_g = ui.fonts(|f| f.layout(cam_text, font.clone(), theme.text_muted(), wrap));
        let help_g = ui.fonts(|f| {
            f.layout("Drag to turn · scroll to zoom · shift+drag to pan (stops following) · Home: back to me · click a name for details".to_string(), font.clone(), theme.text_muted(), wrap)
        });
        let help_top = rect.bottom() - 8.0 - help_g.size().y;
        let cam_top = help_top - 2.0 - cam_g.size().y;
        let hud_w = cam_g.size().x.max(help_g.size().x);
        let lines = sky_lines(theme, state, sky);
        let sky_galleys: Vec<_> = lines.into_iter().map(|(l, c)| ui.fonts(|f| f.layout_no_wrap(l, font.clone(), c))).collect();
        let sky_w = sky_galleys.iter().map(|g| g.size().x).fold(0.0_f32, f32::max);
        // On the bottom edge when it clears the camera lines, else above them
        // (2026-09-27 snapshot review: on a narrow panel they shared the edge).
        let base = if hud_w + sky_w + 24.0 <= rect.width() { rect.bottom() - 8.0 } else { cam_top - 8.0 };
        let n = sky_galleys.len();
        let sky = sky_galleys
            .into_iter()
            .enumerate()
            .map(|(i, g)| {
                let y = base - (n - 1 - i) as f32 * 14.0;
                (Pos2::new(rect.right() - 8.0 - g.size().x, y - g.size().y), g)
            })
            .collect();
        Self { cam: (Pos2::new(rect.left() + 8.0, cam_top), cam_g), help: (Pos2::new(rect.left() + 8.0, help_top), help_g), sky }
    }

    fn rects(&self) -> Vec<Rect> {
        std::iter::once(&self.cam).chain(std::iter::once(&self.help)).chain(self.sky.iter()).map(|(p, g)| Rect::from_min_size(*p, g.size())).collect()
    }

    fn paint(self, paint: &egui::Painter) {
        for (p, g) in std::iter::once(self.cam).chain(std::iter::once(self.help)).chain(self.sky) {
            paint.galley(p, g, Color32::WHITE);
        }
    }
}

/// The sky-events readout's lines, top to bottom, with their colours: a
/// heading, the ring legend when a ring is on screen, any eclipse, the
/// tightest conjunctions now, and the next ones coming.
fn sky_lines(theme: &Theme, state: &GuiState, (sim_time, conjunctions, eclipse): SkyNow) -> Vec<(String, Color32)> {
    let mut lines: Vec<(String, Color32)> = vec![("Sky events (from Earth's POV)".to_string(), theme.text_primary())];
    if !conjunctions.is_empty() || eclipse.is_some() {
        lines.push(("orange ring = body in a current conjunction / close approach".to_string(), theme.warning()));
    }
    if let Some(ev) = eclipse {
        lines.push((format!("ECLIPSE: {} · {:.0}% coverage · Sun-Moon sep {:.3}°", ev.kind.label(), (ev.coverage * 100.0).round(), ev.separation_deg), theme.danger()));
        if let (Some(earth), Some(moon)) = (find_body("earth"), find_body("moon")) {
            let (lat, lon) = subpoint_lat_lon_deg(body_world_position_3d_au(moon, sim_time), body_world_position_3d_au(earth, sim_time), sim_time);
            lines.push((format!("  shadow axis over ~{} (approx)", format_geo(lat, lon)), theme.warning()));
        }
    }
    let mut any = false;
    for ev in conjunctions.iter().take(3) {
        if eclipse.is_some() && (ev.body_a_id == "sun" || ev.body_b_id == "sun") && (ev.body_a_id == "moon" || ev.body_b_id == "moon") {
            continue;
        }
        let color = match ev.tightness {
            ConjunctionTightness::Occultation => theme.danger(),
            ConjunctionTightness::Tight | ConjunctionTightness::Conjunction => theme.warning(),
            ConjunctionTightness::Close => theme.text_secondary(),
        };
        lines.push((format!("{} + {}  {:.2}°  ({})", ev.body_a_name, ev.body_b_name, ev.angular_sep_deg, ev.tightness.label()), color));
        any = true;
    }
    if !any && eclipse.is_none() {
        lines.push(("Sky is quiet now (no conjunctions within 5°)".to_string(), theme.text_muted()));
    }
    if !state.maps.upcoming_events.is_empty() {
        lines.push(("Upcoming".to_string(), theme.text_muted()));
        for ue in state.maps.upcoming_events.iter().take(5) {
            let delta = (ue.when_sim_seconds - sim_time).max(0.0);
            let when = if delta < 86_400.0 { format!("in {:.0}h", (delta / 3600.0).max(1.0)) } else { format!("in {:.0}d", delta / 86_400.0) };
            let date = format_sim_time(ue.when_sim_seconds);
            let date = date.split("  ").next().unwrap_or(&date).to_string();
            let color = match ue.severity {
                2 => theme.danger(),
                1 => theme.warning(),
                _ => theme.text_secondary(),
            };
            lines.push((format!("{} ({}) {}", when, date, ue.label), color));
        }
    }
    lines
}

/// A distance as the HUD says it: metres, km, or AU.
fn format_distance(m: f64) -> String {
    if m < 1.0e4 {
        format!("{m:.0} m")
    } else if m < 1.0e10 {
        format!("{} km", format_with_commas((m / 1000.0).round() as i64))
    } else {
        format!("{:.2} AU", m / M_PER_AU)
    }
}

/// What the camera is anchored on, as the HUD names it.
fn anchor_name(maps: &crate::gui::state_types::MapsState) -> String {
    use super::camera::MapAnchor;
    match &maps.camera.anchor {
        MapAnchor::Me => match maps.me.as_ref() {
            Some(me) if me.aboard => format!("{} (following you)", me.ship_name),
            Some(_) => "you (following)".to_string(),
            None => "the Sun".to_string(),
        },
        MapAnchor::Body(id) => find_body(id).map(|b| b.name.clone()).unwrap_or_else(|| id.clone()),
        MapAnchor::Point { body, .. } => format!("a point near {}", titlecase(body)),
    }
}

// ─────────────────────── Body info card (Phase 4d-bis, v0.209.0) ───────────

/// Render the expanded info card for a SolBody, hanging FLUSH from its pill
/// (`widgets::body_pill::paint_card_extension` owns the layout; this owns the
/// domain text and the Maps actions, Focus and Track).
fn draw_body_info_card_v2(
    ui: &mut egui::Ui,
    paint: &egui::Painter,
    theme: &Theme,
    body: &SolBody,
    pp: &crate::gui::widgets::body_pill::PlacedPill,
    canvas_rect: Rect,
    state: &mut GuiState,
) {
    use crate::gui::widgets::body_pill::{paint_card_extension, BodyCardData};

    let dist_str = distance_line(body);
    let period_str = if body.orbital_period_days > 365.25 {
        format!("{:.2} y period", body.orbital_period_days / 365.25)
    } else if body.orbital_period_days > 1.0 {
        format!("{:.1} d period", body.orbital_period_days)
    } else if body.orbital_period_days > 0.0 {
        format!("{:.1} h period", body.orbital_period_days * 24.0)
    } else {
        String::new()
    };
    let mut subtitle_parts: Vec<String> = vec![titlecase(&body.body_type)];
    if !dist_str.is_empty() {
        subtitle_parts.push(dist_str);
    }
    if !period_str.is_empty() {
        subtitle_parts.push(period_str);
    }
    let mut stats: Vec<(String, String)> = Vec::new();
    if body.radius_km > 0.0 {
        stats.push(("Radius".to_string(), format_km(body.radius_km)));
    }
    if body.mean_temperature_k > 0.0 {
        stats.push(("Mean temp".to_string(), format!("{:.0} °C ({:.0} K)", body.mean_temperature_k - 273.15, body.mean_temperature_k)));
    }
    if body.surface_gravity_ms2 > 0.0 {
        stats.push(("Surface gravity".to_string(), format!("{:.2} m/s²", body.surface_gravity_ms2)));
    }
    let is_tracking = state.maps.tracked_body.as_deref() == Some(body.id.as_str());
    let data = BodyCardData {
        subtitle: Some(subtitle_parts.join(" · ")),
        stats,
        description: (!body.description.is_empty()).then_some(body.description.as_str()),
        actions: vec![("Focus".to_string(), true), (if is_tracking { "Stop Tracking".to_string() } else { "Track".to_string() }, true)],
    };
    let resp = paint_card_extension(ui, paint, theme, pp, &data, canvas_rect);
    if resp.closed {
        state.maps.expanded_body = None;
    }
    match resp.action_clicked {
        Some(0) => {
            state.maps.focus_request = Some(body.id.clone());
            state.maps.selected_body = Some(body.id.clone());
        }
        Some(1) => {
            // Starting tracking also snaps the camera to the body with a Focus.
            state.maps.tracked_body = toggle_tracked_body(state.maps.tracked_body.as_deref(), &body.id);
            if state.maps.tracked_body.is_some() {
                state.maps.focus_request = Some(body.id.clone());
            }
        }
        _ => {}
    }
}

// ─────────────────────── Helpers ────────────────────────────────────────────

/// Decide the new tracked-body state after a Track/Stop-Tracking click:
/// toggles off if the clicked body is already the one being tracked,
/// otherwise starts tracking it.
pub(super) fn toggle_tracked_body(current: Option<&str>, clicked_body_id: &str) -> Option<String> {
    if current == Some(clicked_body_id) {
        None
    } else {
        Some(clicked_body_id.to_string())
    }
}

/// Whether consuming a Focus request should cancel continuous tracking.
/// A manual Focus elsewhere implies "stop auto-following" -- unless the
/// focus target IS the currently tracked body (e.g. Track's own initial
/// snap-to), in which case tracking should continue uninterrupted.
pub(super) fn focus_should_clear_tracking(tracked: Option<&str>, focus_id: &str) -> bool {
    tracked != Some(focus_id)
}
