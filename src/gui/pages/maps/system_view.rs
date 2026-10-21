//! The Maps page's Solar System view: the Sun's family in 3D through the
//! map's camera (`camera.rs`: an anchor, an f64 offset, 10 m to 1e14 m), over
//! the WORLD layer the engine renders for it at ship scale (`you_layer.rs`,
//! `engine/map_view.rs`), with the reference-orbit (LEO, GEO...) and
//! Lagrange-point overlays, you and your ship, the sky-events readout, and the
//! info card a body's pill opens (Focus, Track).
//! Moved out of `gui/pages/cosmos.rs` in Maps increment 0 (2026-10-20). Maps
//! increment 1 (2026-10-21) moved it from AU around the Sun to metres around
//! Earth, the engine's own frame, so the ship and the planets are placed by
//! one set of numbers.

use egui::{Align2, Color32, Pos2, Rect, RichText, Rounding, Sense, Stroke, Vec2};

use crate::cosmos::{body_world_position_3d_au, find_body, sample_orbit_points, sol_bodies, SolBody};
use crate::gui::theme::Theme;
use crate::gui::GuiState;

use super::camera::{MapProjector, MapScene, FOV_Y_RAD};
use super::sky_events::{
    detect_sky_events, format_geo, scan_upcoming_sky_events, subpoint_lat_lon_deg, ConjunctionTightness,
};
use super::time_bar::{current_real_time_seconds_since_j2000, draw_time_controls, format_sim_time};
use super::{body_color, controls, format_km, format_with_commas, titlecase, you_layer};

/// How far to stand from a body a Focus flies to: its disc about a sixth of
/// the view's height (and never inside 10 km).
fn focus_distance_m(body: &SolBody) -> f64 {
    (body.radius_km * 1000.0 / (FOV_Y_RAD * 0.5).tan() * 6.0).max(1.0e4)
}

// ─────────────────────── System view ────────────────────────────────────────

pub(super) fn draw_system_view(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // ── Initialize sim_time to current real-world time on first open
    //    so the user immediately sees today's planetary configuration
    //    (operator 2026-05-10: "if we could have all the planets line
    //    up in real life as they are today on May 12, 2026 then we
    //    could see what it'd look like on a great conjunction"). v0.208.0.
    //    While Live (the default since Maps increment 1) the feed keeps it
    //    on the engine's own sky clock instead; `controls::advance` runs the
    //    page's clock when it is detached. ──
    if !state.maps.sim_time_initialized {
        state.maps.sim_time_seconds = current_real_time_seconds_since_j2000();
        state.maps.sim_time_initialized = true;
    }

    // ── Time controls panel — top of the canvas area. Any scrub, skip or
    //    speed change takes the clock off Live (design 5.9); "Live" puts it
    //    back. ──
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
    paint.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(8, 8, 14));  // theme-exempt: deep-space backdrop
    controls::note_canvas(ui, rect, state);
    let me = state.maps.me.clone();
    let scene = MapScene::new(me.as_ref(), sim_time, state.maps.live);
    let proj = MapProjector::new(state.maps.camera.pose(&scene), rect);
    let world_drawn = you_layer::draw_world_backdrop(&paint, rect, &state.maps);

    let bodies = sol_bodies();

    // ── Focus requests fly the camera to the body and anchor it there, so
    //    it follows the body round its orbit. ──
    if let Some(focus_id) = state.maps.focus_request.take() {
        if focus_should_clear_tracking(state.maps.tracked_body.as_deref(), &focus_id) {
            state.maps.tracked_body = None;
        }
        if let Some(body) = find_body(&focus_id) {
            state.maps.camera.focus_body(&focus_id, focus_distance_m(body), &scene);
        }
        ui.ctx().request_repaint();
    }

    // ── Project all bodies, sort by depth for back-to-front draw ──
    struct ProjectedBody<'a> {
        body: &'a SolBody,
        screen: Pos2,
        depth: f64,
        world_m: glam::DVec3,
    }
    let mut projected: Vec<ProjectedBody> = Vec::with_capacity(bodies.len());
    for body in bodies {
        let Some(world) = scene.body_m(&body.id) else { continue };
        if let Some((screen, depth)) = proj.project(world) {
            projected.push(ProjectedBody { body, screen, depth, world_m: world });
        }
    }
    // Back-to-front: largest depth first (farther from camera).
    projected.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal));

    // ── Draw orbit ellipses (v0.207.0: real ellipses with eccentricity
    //    + inclination + argument of perihelion + longitude of ascending
    //    node). Uses sample_orbit_points for the math; same code path
    //    handles direct sun-orbiters AND moons because both are just
    //    "body in parent's frame" — the parent's world position is
    //    folded in by adding it after projection. An orbit far bigger than
    //    the view is left out (design 5.4's size fade, its first rung): at
    //    ship scale Earth's 96-chord orbit would cross the view as a line
    //    tens of thousands of km off Earth. ──
    let earth_au = scene.earth_helio_au();
    for body in bodies {
        let parent_au = if let Some(parent_id) = &body.parent {
            if parent_id == "sun" {
                glam::DVec3::ZERO
            } else if let Some(parent) = find_body(parent_id) {
                body_world_position_3d_au(parent, sim_time)
            } else {
                continue;
            }
        } else {
            continue; // Sun has no orbit
        };
        let a_m = if body.semi_major_axis_au > 0.0 { body.semi_major_axis_au * crate::cosmos::M_PER_AU } else { body.semi_major_axis_km * 1000.0 };
        if a_m > proj.pose.distance * 2.0e4 {
            continue;
        }
        let n = if body.body_type == "moon" { 48 } else { 96 };
        let points = sample_orbit_points(body, n);
        if points.is_empty() { continue; }
        let stroke = if body.body_type == "moon" {
            Stroke::new(0.4, Color32::from_rgb(35, 35, 55))  // theme-exempt: moon orbit — faintest
        } else {
            Stroke::new(0.6, Color32::from_rgb(45, 45, 70))  // theme-exempt: planet orbit — faint
        };
        let mut prev: Option<Pos2> = None;
        for p in &points {
            let world_pos = (parent_au + *p - earth_au) * crate::cosmos::M_PER_AU;
            if let Some((pp, _)) = proj.project(world_pos) {
                if let Some(prev_p) = prev {
                    paint.line_segment([prev_p, pp], stroke);
                }
                prev = Some(pp);
            } else {
                prev = None;
            }
        }
    }

    // Hover position is consumed by both Lagrange hover (just below) and
    // the body draw pass below — declare once here.
    let hover_pos = ui.input(|i| i.pointer.hover_pos());

    // ── Lagrange-point overlay (Phase 4d-soi, v0.211.0) ──
    // Render L1-L5 markers for each interesting parent-child pair when the
    // overlay is toggled on. Hovering a marker shows its name and any
    // notable things parked there (JWST, Trojans, etc.).
    let mut lagrange_markers: Vec<(String, Pos2, &'static str)> = Vec::new();
    if state.maps.show_lagrange {
        for pair in LAGRANGE_PAIRS {
            let Some(parent) = find_body(pair.parent_id) else { continue; };
            let Some(child) = find_body(pair.child_id) else { continue; };
            let Some(points) = compute_lagrange_points(parent, child, sim_time) else {
                continue;
            };
            let names = ["L1", "L2", "L3", "L4", "L5"];
            for (i, pt) in points.iter().enumerate() {
                if let Some((screen, _depth)) = proj.project(scene.au_to_m(*pt)) {
                    // Render the marker — small "×" via two crossed segments.
                    let s = 5.0_f32;
                    let color = theme.info();
                    paint.line_segment(
                        [screen + Vec2::new(-s, -s), screen + Vec2::new(s, s)],
                        Stroke::new(1.0, color),
                    );
                    paint.line_segment(
                        [screen + Vec2::new(-s, s), screen + Vec2::new(s, -s)],
                        Stroke::new(1.0, color),
                    );
                    // Tiny dot at the center for emphasis.
                    paint.circle_filled(screen, 1.5, color);
                    let label = format!("{} {}", pair.pair_label, names[i]);
                    lagrange_markers.push((label.clone(), screen, names[i]));
                    // Light text label below the × — only render if zoomed
                    // in enough that markers aren't tightly clustered.
                    // (Heuristic: skip labels when parent-child screen
                    // distance is < 40 px.)
                    let Some((parent_screen, _)) = proj.project(
                        scene.au_to_m(body_world_position_3d_au(parent, sim_time)))
                    else { continue; };
                    let Some((child_screen, _)) = proj.project(
                        scene.au_to_m(body_world_position_3d_au(child, sim_time)))
                    else { continue; };
                    let pair_screen_dist = (child_screen - parent_screen).length();
                    if pair_screen_dist > 50.0 {
                        paint.text(
                            screen + Vec2::new(0.0, 8.0),
                            Align2::CENTER_TOP,
                            names[i],
                            egui::FontId::proportional(9.0),
                            theme.info(),
                        );
                    }
                }
            }
        }

        // Lagrange marker hover tooltip — show name + notable parking
        // when the pointer is near a marker.
        if let Some(hp) = hover_pos {
            for pair in LAGRANGE_PAIRS {
                let Some(parent) = find_body(pair.parent_id) else { continue; };
                let Some(child) = find_body(pair.child_id) else { continue; };
                let Some(points) = compute_lagrange_points(parent, child, sim_time) else {
                    continue;
                };
                let names = ["L1", "L2", "L3", "L4", "L5"];
                for (i, pt) in points.iter().enumerate() {
                    if let Some((screen, _depth)) = proj.project(scene.au_to_m(*pt)) {
                        if (hp - screen).length() < 10.0 {
                            let lname = names[i];
                            let notable_text = pair.notable.iter()
                                .find(|(n, _)| *n == lname)
                                .map(|(_, what)| *what)
                                .unwrap_or("");
                            response.clone().on_hover_ui_at_pointer(|ui| {
                                ui.set_max_width(220.0);
                                ui.label(
                                    RichText::new(format!("{} {}", pair.pair_label, lname))
                                        .strong()
                                        .color(theme.text_primary()),
                                );
                                if !notable_text.is_empty() {
                                    ui.label(
                                        RichText::new(notable_text)
                                            .size(theme.font_size_small)
                                            .color(theme.text_secondary()),
                                    );
                                }
                                let blurb = match lname {
                                    "L1" => "Between the two bodies, unstable, needs station-keeping. Good for solar wind observatories looking 'between' the parent and child.",
                                    "L2" => "Past the child along the parent-child line, unstable but a low-station-keeping shadow zone. Telescope heaven.",
                                    "L3" => "On the opposite side of the parent from the child, too far + occluded to be very useful.",
                                    "L4" => "60° ahead of the child in its orbit, stable for μ < 0.0385. Trojan asteroids accumulate here.",
                                    "L5" => "60° behind the child in its orbit, stable for μ < 0.0385. Mirror of L4.",
                                    _ => "",
                                };
                                ui.label(
                                    RichText::new(blurb)
                                        .size(theme.font_size_small)
                                        .color(theme.text_muted())
                                        .italics(),
                                );
                            });
                        }
                    }
                }
            }
        }
    }
    let _ = lagrange_markers; // marker list saved for future click-to-track

    // ── Overlay toggles (top-right of canvas) ──
    // Lagrange + Reference orbits each get a small floating button so the
    // user can flip overlays on/off without leaving the system view.
    let toggles_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 260.0, rect.top() + 8.0),
        egui::vec2(252.0, 22.0),
    );
    let mut toggle_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(toggles_rect)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    // Lagrange toggle (rightmost).
    let lag_label = if state.maps.show_lagrange { "Lagrange: ON" } else { "Lagrange: OFF" };
    let lag_btn = egui::Button::new(
        RichText::new(lag_label)
            .size(theme.font_size_small)
            .color(if state.maps.show_lagrange { theme.info() } else { theme.text_secondary() }),
    )
    .fill(theme.bg_card())
    .stroke(Stroke::new(
        0.8,
        if state.maps.show_lagrange { theme.info() } else { theme.border() },
    ));
    if toggle_ui.add(lag_btn).on_hover_text("Toggle Lagrange-point overlay (L1-L5 for Sun-Earth, Earth-Moon, Sun-Mars, Sun-Jupiter, Sun-Saturn pairs)").clicked() {
        state.maps.show_lagrange = !state.maps.show_lagrange;
    }
    // Reference orbits toggle (next-to-left, only meaningful for Earth/Mars/Jupiter/Moon).
    let ref_label = if state.maps.show_reference_orbits { "Ref orbits: ON" } else { "Ref orbits: OFF" };
    let ref_btn = egui::Button::new(
        RichText::new(ref_label)
            .size(theme.font_size_small)
            .color(if state.maps.show_reference_orbits { theme.info() } else { theme.text_secondary() }),
    )
    .fill(theme.bg_card())
    .stroke(Stroke::new(
        0.8,
        if state.maps.show_reference_orbits { theme.info() } else { theme.border() },
    ));
    if toggle_ui.add(ref_btn).on_hover_text("Toggle reference-orbit rings (LEO/MEO/GEO/HEO around Earth; equivalent rings for Mars, Jupiter, Moon). Visible when zoomed in close enough.").clicked() {
        state.maps.show_reference_orbits = !state.maps.show_reference_orbits;
    }

    // ── UNIFIED MAP slice 1 (v0.688, operator direction 2026-07-04:
    //    "everything synced to one thing instead of separate systems"): the
    //    player's HOME marker rides Earth, the mining asteroids cluster
    //    around it, and an in-flight drone tracks between them. The asteroid
    //    frame is homestead-LOCAL km (a ~70 km bubble) -- sub-pixel at AU
    //    scale -- so they render as a labeled fan around the Home marker,
    //    with real distances in the labels keeping the scale honest. The
    //    Inventory page's mining mini-map now links here.
    // ── Sky events: detect conjunctions + eclipses for the current sim_time ──
    // (Phase 4d-tri, v0.210.0.) Cheap O(n²) over ~11 named bodies — runs every
    // frame; updates live as the user scrubs sim_time.
    let (conjunctions, eclipse) = detect_sky_events(sim_time);

    // ── Forward Sky-Events scan (Phase 4d-quad, v0.248.0) ──
    // The 400-day scan is ~3000× the cost of the instant detector, so it
    // does NOT run every frame. Re-scan only when sim_time has drifted
    // >12 h from the cached origin AND ≥300 ms wall-clock since the last
    // scan (so fast-forward / scrubbing can't trigger one per frame).
    {
        let drifted = !state.maps.upcoming_scan_origin.is_finite()
            || (sim_time - state.maps.upcoming_scan_origin).abs() > 12.0 * 3600.0;
        let wall_ok = state
            .maps.upcoming_last_scan
            .map(|t| t.elapsed().as_millis() >= 300)
            .unwrap_or(true);
        if drifted && wall_ok {
            state.maps.upcoming_events = scan_upcoming_sky_events(sim_time);
            state.maps.upcoming_scan_origin = sim_time;
            state.maps.upcoming_last_scan = Some(std::time::Instant::now());
        }
    }

    // Build a per-body conjunction-tightness map for visual highlighting.
    // Each body in any conjunction gets the TIGHTEST tightness it's involved in.
    let mut body_tightness: std::collections::HashMap<&str, ConjunctionTightness> =
        std::collections::HashMap::new();
    for ev in &conjunctions {
        // Skip events involving the Sun if there's also an eclipse — the
        // eclipse rendering handles Sun+Moon already.
        if eclipse.is_some() && (ev.body_a_id == "sun" || ev.body_b_id == "sun")
            && (ev.body_a_id == "moon" || ev.body_b_id == "moon") {
            continue;
        }
        for id in [ev.body_a_id.as_str(), ev.body_b_id.as_str()] {
            let entry = body_tightness.entry(id).or_insert(ev.tightness);
            if ev.tightness > *entry { *entry = ev.tightness; }
        }
    }

    // ── Draw bodies (back-to-front, with depth-scaled radii) ──
    //
    // Render order (v0.214.0 — operator feedback: "it should be a single
    // thing, not three separate widgets, so the planet icon remains on
    // the original first one"):
    //
    //   1. Compute body radii up front (used for pill-cap sizing).
    //   2. PHASE 1 — paint pill backgrounds (filled rounded-rects). The
    //      body's screen-position is the pill's left-cap center, so the
    //      body circle (drawn next) becomes the visible pill dot.
    //   3. Draw body circles + selected ring + conjunction rings +
    //      eclipse highlights — these all render ON TOP of pill backgrounds.
    //      The body IS the pill's left cap.
    //   4. PHASE 2 — paint pill borders + name text + click interaction.
    //   5. PHASE 3 — for any expanded pill, paint the card extension
    //      below it (flush with pill, sharing the left edge).
    //
    // (`hover_pos` declared earlier — shared with the Lagrange overlay pass.)
    let mut hovered_body: Option<&SolBody> = None;
    let mut clicked_body: Option<&SolBody> = None;

    // PASS A — compute body radii up front (no paint yet). We need radii
    // before we can lay out pills (the pill height matches the body's
    // diameter so the body becomes the pill's left cap).
    // Maps increment 1: the disc is TRUE size over a minimum (no more 40 px
    // ceiling, so Earth behind the ship is Earth-sized), while the pill
    // keeps a capped copy for its layout; over the world layer a disc bigger
    // than the icon floor is not painted at all, because the real globe is
    // there (design 5.3).
    let mut body_radii_px: Vec<f32> = Vec::with_capacity(projected.len());
    let mut disc_radii_px: Vec<f32> = Vec::with_capacity(projected.len());
    for pb in &projected {
        let true_r = proj.radius_px(pb.body.radius_km * 1000.0, pb.depth);
        let min_r = if pb.body.id == "sun" { 8.0 }
                    else if pb.body.body_type == "moon" { 1.5 }
                    else if pb.body.body_type == "asteroid" { 1.0 }
                    else { 2.5 };
        let max_r = if pb.body.id == "sun" { 80.0 } else { 40.0 };
        body_radii_px.push(true_r.max(min_r).min(max_r));
        let globe_shown = world_drawn && true_r > you_layer::ICON_FLOOR_PX;
        disc_radii_px.push(if globe_shown { 0.0 } else { true_r.max(min_r).min(rect.width().max(rect.height()) * 2.0) });
    }

    // PASS B — hit-test hover/click on body screen positions (still no
    // paint). The pill widget owns the click handler for any pilled body
    // (because the pill rect encompasses the body's left cap area), so
    // body hits here only matter for non-pilled bodies that the user
    // clicks on the canvas to select.
    for (i, pb) in projected.iter().enumerate() {
        let r_px = body_radii_px[i];
        if let Some(hp) = hover_pos {
            if (hp - pb.screen).length() < r_px + 4.0 {
                hovered_body = Some(pb.body);
                if response.clicked() {
                    clicked_body = Some(pb.body);
                }
            }
        }
    }
    let _suppress_unused: Option<glam::DVec3> = projected.first().map(|p| p.world_m);

    // PASS C — build pill input list. Decide visibility (priority tier
    // or forced via hover/select/expand) before any rendering happens.
    //
    // Priority key (lower = drawn first / always-visible):
    //   0 = star (Sun)
    //   1 = terrestrial / gas_giant / ice_giant (true planets)
    //   2 = dwarf_planet (Pluto, Ceres, etc.)
    //   3 = moon (shown only when hovered/selected/expanded)
    //   4 = asteroid (shown only when hovered/selected/expanded)
    use crate::gui::widgets::body_pill::{
        BodyPill, compute_pill_layout, paint_pill_backgrounds, paint_pill_overlays,
    };
    let mut pills: Vec<BodyPill> = Vec::with_capacity(projected.len());
    for (i, pb) in projected.iter().enumerate() {
        let r_px = body_radii_px[i];
        let priority: u8 = match pb.body.body_type.as_str() {
            "star" => 0,
            "terrestrial" | "gas_giant" | "ice_giant" => 1,
            "dwarf_planet" => 2,
            "moon" => 3,
            _ => 4,
        };
        let is_hovered = hovered_body.map(|b| b.id == pb.body.id).unwrap_or(false);
        let is_selected = state.maps.selected_body.as_deref() == Some(pb.body.id.as_str());
        let is_expanded = state.maps.expanded_body.as_deref() == Some(pb.body.id.as_str());
        let is_forced = is_hovered || is_selected || is_expanded;
        let should_show = priority <= 2 || is_forced;
        if !should_show { continue; }
        // Over the world layer's real globe a pill would paint its dark cap
        // onto the globe (a black disc in the middle of Earth in the first
        // rig capture, 2026-10-21): the globe is the body, so it gets a plain
        // label instead.
        if disc_radii_px[i] == 0.0 {
            paint.text(pb.screen, Align2::CENTER_CENTER, &pb.body.name, egui::FontId::proportional(13.0), theme.text_primary());
            continue;
        }
        pills.push(BodyPill {
            id: pb.body.id.as_str(),
            name: pb.body.name.as_str(),
            color: body_color(&pb.body.name),
            body_screen: pb.screen,
            body_radius_px: r_px,
            priority,
            forced: is_forced,
            expanded: is_expanded,
        });
    }

    // PHASE 0 — pure layout (no paint). Compute pill rects with collision-
    // dodge so the panel + pill bg + body draws can all reference them.
    let pill_layout = compute_pill_layout(&paint, theme, &pills);

    // PHASE 1 — paint the panel BEHIND the expanded pill (if any). Must
    // come before body draw + pill backgrounds so the body and pill cap
    // render ON TOP of the panel (operator feedback 2026-05-12 - "when
    // the panel is up the icon and circle around where the icon is is
    // missing"; that was a layering bug fixed here).
    if let Some(expanded_id) = state.maps.expanded_body.clone() {
        let placed = pill_layout.placed.iter().find(|pp| pp.id == expanded_id).cloned();
        let body = projected.iter().find(|pb| pb.body.id == expanded_id).map(|pb| pb.body);
        if let (Some(pp), Some(body)) = (placed, body) {
            draw_body_info_card_v2(ui, &paint, theme, body, &pp, rect, state);
        }
    }

    // PHASE 2 — paint pill backgrounds (rect fill + body-colored cap).
    // The body-colored cap fill makes the cap visually appear AS the
    // body, so small bodies don't look "lost" inside an oversized cap.
    paint_pill_backgrounds(&paint, theme, &pill_layout);

    // PASS D — draw body circles + selected ring + conjunction rings +
    // eclipse highlights. These overlay the pill backgrounds, completing
    // the "body IS the pill's left dot" visual trick. Order matters:
    // pill background first, body circle on top of the cap area, then
    // body decorations on top of body.
    for (i, pb) in projected.iter().enumerate() {
        let r_px = body_radii_px[i];

        if disc_radii_px[i] > 0.0 {
            paint.circle_filled(pb.screen, disc_radii_px[i], body_color(&pb.body.name));
        }
        // Note: removed the v0.214-era selected accent ring at r_px + 3
        // (operator feedback 2026-05-12 — "the orange circle around the
        // sprite was smaller than the pill top-left corner"). The pill
        // border's left semicircle (drawn in paint_pill_overlays) is the
        // single canonical "orange ring around the body" now, with
        // radius = widget_radius which also matches the pill + panel
        // corner radii. All three circles are the same size.

        // Conjunction highlight ring — bodies currently in a conjunction
        // pulse with a warm-tone ring. Tightness drives ring thickness
        // and brightness. v0.210.0.
        if let Some(t) = body_tightness.get(pb.body.id.as_str()) {
            let weight = t.weight();
            let outer_r = r_px + 6.0 + weight * 4.0;
            let inner_r = r_px + 3.0;
            paint.circle_stroke(
                pb.screen,
                outer_r,
                Stroke::new(1.0 + weight * 2.0, theme.warning()),
            );
            paint.circle_stroke(
                pb.screen,
                inner_r,
                Stroke::new(0.5, theme.warning()),
            );
        }
        // Eclipse highlight on Earth — Sun-Moon overlap. Render a strong
        // accent ring around Earth and a darker disc overlaid on the Sun.
        if let Some(ref ev) = eclipse {
            if pb.body.id == "earth" {
                paint.circle_stroke(
                    pb.screen,
                    r_px + 8.0,
                    Stroke::new(2.0, theme.danger()),
                );
                paint.text(
                    pb.screen + Vec2::new(0.0, r_px + 22.0),
                    Align2::CENTER_TOP,
                    "ECLIPSE",
                    egui::FontId::proportional(11.0),
                    theme.danger(),
                );
                let _ = ev;
            } else if pb.body.id == "sun" {
                let dark_r = (r_px as f64 * ev.coverage.sqrt()).clamp(0.0, r_px as f64) as f32;
                paint.circle_filled(pb.screen, dark_r, theme.bg_primary());
                paint.circle_stroke(pb.screen, r_px + 2.0, Stroke::new(1.0, theme.danger()));
            } else if pb.body.id == "moon" {
                paint.circle_stroke(
                    pb.screen,
                    r_px + 4.0,
                    Stroke::new(1.5, theme.danger()),
                );
            }
        }
    }

    // ── Reference-orbit rings (Phase 4d-ref, v0.212.0) ──
    // Render LEO / MEO / GEO / etc rings around any supported planet
    // when the toggle is on AND the body is large enough on screen to
    // make rings legible. Currently supports Earth, Mars, Jupiter, Moon.
    let mut ref_orbit_hover: Option<(&'static str, &'static str)> = None;
    if state.maps.show_reference_orbits {
        for (i, pb) in projected.iter().enumerate() {
            if reference_orbits_for(pb.body.id.as_str()).is_empty() { continue; }
            let r_px = body_radii_px[i];
            if let Some(hit) = draw_reference_orbits(
                &paint, theme, pb.body, pb.world_m, r_px,
                &proj, hover_pos,
            ) {
                ref_orbit_hover = Some(hit);
            }
        }
    }
    if let Some((name, blurb)) = ref_orbit_hover {
        response.clone().on_hover_ui_at_pointer(|ui| {
            ui.set_max_width(240.0);
            ui.label(RichText::new(name).strong().color(theme.text_primary()));
            ui.label(
                RichText::new(blurb)
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
            );
        });
    }

    // ── PHASE 3: paint pill OVERLAYS (borders + divider + name + click).
    // Pill borders render on top of body circles + their decorations AND
    // on top of the panel border (drawn in PHASE 1). Strong accent border
    // for any forced/hovered pill so the body always reads as "wrapped in
    // an orange ring" when the user is interacting with it.
    let clicked_pill = paint_pill_overlays(ui, &paint, theme, &pill_layout, "maps_pill");

    // ── You and your ship (Maps increment 1, `you_layer.rs`): its name and
    //    real height, its orbit ring, "You" aboard, the mining asteroids
    //    where the data puts them (their markers feed the click chain below:
    //    clicking one jumps to its mining modal), and, at ship scale with no
    //    world layer, why the ship is not drawn for real. ──
    let asteroid_markers = you_layer::draw_you(&paint, theme, &proj, &scene, state, world_drawn);
    if !world_drawn && you_layer::near_the_ship(&state.maps) {
        if let Some(why) = you_layer::world_layer_want(&state.maps).reason() {
            paint.text(rect.center_top() + Vec2::new(0.0, 44.0), Align2::CENTER_TOP, why, egui::FontId::proportional(12.0), theme.text_secondary());
        }
    }

    // Apply click selection. Asteroid fan markers claim clicks FIRST (tiny
    // 10 px zones; when zoomed in, Earth's body/pill otherwise eats every fan
    // click -- v0.692 review fix), then pills, then bodies.
    let clicked_asteroid = if response.clicked() {
        response
            .interact_pointer_pos()
            .and_then(|p| asteroid_markers.iter().find(|(_, mp)| mp.distance(p) < 10.0).cloned())
    } else {
        None
    };
    if let Some((ast_id, _)) = clicked_asteroid {
        state.pending_open_mining_modal = Some(ast_id);
        state.active_page = crate::gui::GuiPage::Inventory;
    } else if let Some(pill_id) = clicked_pill {
        // Toggle expanded state on pill click.
        if state.maps.expanded_body.as_deref() == Some(pill_id.as_str()) {
            state.maps.expanded_body = None;
        } else {
            state.maps.expanded_body = Some(pill_id.clone());
            state.maps.selected_body = Some(pill_id);
        }
    } else if let Some(b) = clicked_body {
        // Body click (no pill on this body) — select it. Next frame the
        // pill will appear because the body is now selected (forced=true).
        state.maps.selected_body = Some(b.id.clone());
    }

    // The camera's mouse and keys, now that everything is drawn from the
    // pose the world layer was rendered from (`controls`).
    controls::handle_input(ui, &response, rect, state);

    // ── Hover tooltip ──
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
                RichText::new("Click to open details · drag to rotate · scroll to zoom · shift+drag to pan")
                    .size(theme.font_size_small)
                    .color(theme.accent())
                    .italics(),
            );
        });
    }

    // ── HUD overlay: camera state for the operator ──
    // Laid out (not painted blind) so the sky-events list below can see how
    // much of the bottom edge these two lines take. Wrapped to the canvas
    // width: a narrow centre panel used to cut the help line at the edge.
    let hud_font = egui::FontId::proportional(10.0);
    let hud_wrap = (rect.width() - 16.0).max(40.0);
    let cam_galley = ui.fonts(|f| {
        f.layout(
            format!("Camera {} from {} · yaw {:+.0}° · pitch {:+.0}°",
                format_distance(proj.pose.distance), anchor_name(&state.maps), state.maps.camera.yaw.to_degrees(), state.maps.camera.pitch.to_degrees()),
            hud_font.clone(),
            theme.text_muted(),
            hud_wrap,
        )
    });
    let help_galley = ui.fonts(|f| {
        f.layout(
            "Drag to turn · scroll to zoom · shift+drag to pan (stops following) · Home: back to me · click a body to select".to_string(),
            hud_font.clone(),
            theme.text_muted(),
            hud_wrap,
        )
    });
    let help_top = rect.bottom() - 8.0 - help_galley.size().y;
    let cam_top = help_top - 2.0 - cam_galley.size().y;
    let hud_w = cam_galley.size().x.max(help_galley.size().x);
    paint.galley(Pos2::new(rect.left() + 8.0, cam_top), cam_galley, theme.text_muted());
    paint.galley(Pos2::new(rect.left() + 8.0, help_top), help_galley, theme.text_muted());

    // ── Sky-events HUD (Phase 4d-tri, v0.210.0) ──
    // Bottom-right corner readout. Shows up to 3 tightest conjunctions
    // currently in progress + any solar eclipse. Empty when sky is quiet.
    // Its base line is decided once the lines are known (below): the bottom
    // edge when it clears the camera lines, above them when it would not.
    let mut sky_lines: Vec<(String, Color32)> = Vec::new();
    if let Some(ref ev) = eclipse {
        let pct = (ev.coverage * 100.0).round();
        sky_lines.push((
            format!(
                "ECLIPSE: {} · {:.0}% coverage · Sun-Moon sep {:.3}°",
                ev.kind.label(), pct, ev.separation_deg
            ),
            theme.danger(),
        ));
        // Phase 4d-quad: shadow-axis ground point (≈ sub-lunar point —
        // where the eclipse is roughly overhead). Approximate; see
        // subpoint_lat_lon_deg docs for the caveats.
        if let (Some(earth), Some(moon)) = (find_body("earth"), find_body("moon")) {
            let earth_pos = body_world_position_3d_au(earth, sim_time);
            let moon_pos = body_world_position_3d_au(moon, sim_time);
            let (lat, lon) = subpoint_lat_lon_deg(moon_pos, earth_pos, sim_time);
            sky_lines.push((
                format!("  shadow axis over ~{} (approx)", format_geo(lat, lon)),
                theme.warning(),
            ));
        }
    }
    for ev in conjunctions.iter().take(3) {
        // Skip the Sun-Moon pair if there's an eclipse — already reported.
        if eclipse.is_some()
            && (ev.body_a_id == "sun" || ev.body_b_id == "sun")
            && (ev.body_a_id == "moon" || ev.body_b_id == "moon")
        {
            continue;
        }
        let color = match ev.tightness {
            ConjunctionTightness::Occultation => theme.danger(),
            ConjunctionTightness::Tight => theme.warning(),
            ConjunctionTightness::Conjunction => theme.warning(),
            ConjunctionTightness::Close => theme.text_secondary(),
        };
        sky_lines.push((
            format!(
                "{} + {}  {:.2}°  ({})",
                ev.body_a_name, ev.body_b_name, ev.angular_sep_deg, ev.tightness.label()
            ),
            color,
        ));
    }
    if sky_lines.is_empty() {
        sky_lines.push((
            "Sky is quiet now (no conjunctions within 5°)".to_string(),
            theme.text_muted(),
        ));
    }
    // ── Upcoming (Phase 4d-quad): next predicted events from the cached
    // forward scan. Always shown (independent of the instant readout).
    if !state.maps.upcoming_events.is_empty() {
        sky_lines.push(("Upcoming".to_string(), theme.text_muted()));
        for ue in state.maps.upcoming_events.iter().take(5) {
            let delta = (ue.when_sim_seconds - sim_time).max(0.0);
            let when = if delta < 86_400.0 {
                format!("in {:.0}h", (delta / 3600.0).max(1.0))
            } else {
                format!("in {:.0}d", delta / 86_400.0)
            };
            // Date only (strip the "  HH:MM UTC" tail).
            let date = format_sim_time(ue.when_sim_seconds);
            let date = date.split("  ").next().unwrap_or(&date);
            let color = match ue.severity {
                2 => theme.danger(),
                1 => theme.warning(),
                _ => theme.text_secondary(),
            };
            sky_lines.push((format!("{} ({}) {}", when, date, ue.label), color));
        }
    }
    // Legend (map-sync punch list): the orange double-ring around a
    // body (drawn earlier from `body_tightness`) was undiscoverable —
    // the operator asked "why is there an orange ring?". Show a one-line
    // key in the SAME warning color whenever a highlight is actually on
    // screen (i.e. there are real events, not the "quiet" placeholder).
    let any_highlight = !conjunctions.is_empty() || eclipse.is_some();
    if any_highlight {
        sky_lines.insert(
            0,
            (
                "orange ring = body in a current conjunction / close approach".to_string(),
                theme.warning(),
            ),
        );
    }
    // Where the list sits: on the bottom edge when it clears the camera lines
    // at the bottom-left, otherwise stacked above them. On a narrow centre
    // panel (both System side panels open) the two used to share the bottom
    // edge and the help line ran straight through the event list (2026-09-27
    // snapshot review).
    let sky_w = sky_lines
        .iter()
        .map(|(line, _)| {
            ui.fonts(|f| f.layout_no_wrap(line.clone(), hud_font.clone(), theme.text_muted()))
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    let sky_y_base = if hud_w + sky_w + 24.0 <= rect.width() {
        rect.bottom() - 8.0
    } else {
        cam_top - 8.0
    };
    // Section header.
    paint.text(
        Pos2::new(rect.right() - 8.0, sky_y_base - sky_lines.len() as f32 * 14.0 - 18.0),
        Align2::RIGHT_BOTTOM,
        "Sky events (from Earth's POV)",
        egui::FontId::proportional(10.0),
        theme.text_primary(),
    );
    for (i, (line, color)) in sky_lines.iter().enumerate() {
        let y = sky_y_base - (sky_lines.len() - 1 - i) as f32 * 14.0;
        paint.text(
            Pos2::new(rect.right() - 8.0, y),
            Align2::RIGHT_BOTTOM,
            line,
            egui::FontId::proportional(10.0),
            *color,
        );
    }

    // The follow row over the canvas, then the flight and the detached
    // clock move on for the next frame (`controls`).
    controls::draw_follow_row(ui, theme, state, rect);
    controls::advance(state);
}

/// A distance as the HUD says it: metres, km, or AU.
fn format_distance(m: f64) -> String {
    if m < 1.0e4 {
        format!("{m:.0} m")
    } else if m < 1.0e9 {
        format!("{} km", format_with_commas((m / 1000.0).round() as i64))
    } else {
        format!("{:.2} AU", m / crate::cosmos::M_PER_AU)
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

// ─────────────────────── Reference orbits (Phase 4d-ref, v0.212.0) ─────────

/// A named reference orbit around a body — LEO / GEO / etc. Stored as
/// altitude above the body's surface in km. Period is computed from the
/// body's mass via Kepler's third law (a³ = GMP²/4π²).
struct ReferenceOrbit {
    name: &'static str,
    altitude_km: f64,
    /// One-line role/description shown on hover.
    blurb: &'static str,
}

/// Hardcoded reference orbits per supported body. Only bodies for which
/// the "common reference orbit" naming applies — Earth has the canonical
/// LEO/MEO/GEO/HEO; Mars/Moon have analogues but they're not as
/// universally-named. Could move to data/ later.
fn reference_orbits_for(body_id: &str) -> &'static [ReferenceOrbit] {
    match body_id {
        "earth" => &[
            ReferenceOrbit { name: "LEO (low)",  altitude_km: 400.0,    blurb: "ISS altitude, 90 min period" },
            ReferenceOrbit { name: "LEO (high)", altitude_km: 2_000.0,  blurb: "Top of LEO band, 130 min period" },
            ReferenceOrbit { name: "MEO",        altitude_km: 20_200.0, blurb: "GPS / GNSS satellite belt, 12 h period" },
            ReferenceOrbit { name: "GEO",        altitude_km: 35_786.0, blurb: "Geostationary, 24 h period, fixed over equator" },
            ReferenceOrbit { name: "HEO",        altitude_km: 50_000.0, blurb: "High Earth orbit, >24 h period" },
            ReferenceOrbit { name: "Lunar TLI",  altitude_km: 384_400.0, blurb: "Trans-lunar injection apoapsis (Moon's orbit)" },
        ],
        "mars" => &[
            ReferenceOrbit { name: "LMO (low)", altitude_km: 200.0,    blurb: "Low Mars orbit (Mars Reconnaissance Orbiter, etc.)" },
            ReferenceOrbit { name: "areostationary", altitude_km: 17_032.0, blurb: "Mars-stationary, 24.6 h period (sol)" },
        ],
        "jupiter" => &[
            ReferenceOrbit { name: "Low Jovian orbit", altitude_km: 5_000.0, blurb: "Inside the radiation belts, punishing" },
            ReferenceOrbit { name: "Jovo-stationary",  altitude_km: 88_500.0, blurb: "Jupiter-stationary, 9.93 h period" },
        ],
        "moon" => &[
            ReferenceOrbit { name: "Low lunar orbit", altitude_km: 100.0, blurb: "100 km, Apollo command module altitude" },
            ReferenceOrbit { name: "NRHO apoapsis",   altitude_km: 70_000.0, blurb: "Near-rectilinear halo orbit, Gateway target" },
        ],
        _ => &[],
    }
}

/// Render reference-orbit rings around the focused body, in its equatorial
/// plane. The game has no axial tilt (`renderer/sky_frame.rs`), so every
/// body's equator is the world's XZ plane, the plane the home station's own
/// orbit lies in; until Maps increment 1 these rings were drawn in the XY
/// plane, standing on edge to the orbits they name.
fn draw_reference_orbits(
    paint: &egui::Painter,
    theme: &Theme,
    body: &SolBody,
    body_world_m: glam::DVec3,
    body_radius_px: f32,
    proj: &MapProjector,
    hover_pos: Option<Pos2>,
) -> Option<(&'static str, &'static str)> {
    let rings = reference_orbits_for(body.id.as_str());
    if rings.is_empty() { return None; }

    // Threshold — only render rings when the body's apparent radius is
    // > ~12 px (otherwise rings would overlap the body itself or be
    // sub-pixel). Below threshold render nothing.
    if body_radius_px < 12.0 { return None; }

    let mut hovered_info: Option<(&'static str, &'static str)> = None;
    let body_radius_m = body.radius_km * 1000.0;

    // Sample each ring at N points (circular, so uniform in angle) and
    // project each through the map's camera.
    for ring in rings {
        let r_m = body_radius_m + ring.altitude_km * 1000.0;
        let mut prev: Option<Pos2> = None;
        let mut first: Option<Pos2> = None;
        let mut closest_to_hover: Option<(Pos2, f32)> = None;
        for world in reference_ring_points(body_world_m, r_m, 96) {
            if let Some((p, _)) = proj.project(world) {
                if let Some(prev_p) = prev {
                    paint.line_segment([prev_p, p], Stroke::new(0.6, theme.info()));
                }
                if first.is_none() { first = Some(p); }
                prev = Some(p);
                if let Some(hp) = hover_pos {
                    let d = (hp - p).length();
                    if closest_to_hover.map_or(true, |(_, dd)| d < dd) {
                        closest_to_hover = Some((p, d));
                    }
                }
            } else {
                prev = None;
            }
        }
        // Close the ring (segment from last drawn → first drawn).
        if let (Some(prev_p), Some(first_p)) = (prev, first) {
            paint.line_segment([prev_p, first_p], Stroke::new(0.6, theme.info()));
        }
        // Ring label at theta = 0. Only if visible.
        if let Some((label_p, _)) = proj.project(body_world_m + glam::DVec3::new(r_m, 0.0, 0.0)) {
            paint.text(
                label_p + Vec2::new(6.0, 0.0),
                Align2::LEFT_CENTER,
                ring.name,
                egui::FontId::proportional(9.0),
                theme.info(),
            );
        }
        // Hover detection — if pointer is close to any sampled point on
        // this ring, surface the ring's blurb.
        if let Some((_, d)) = closest_to_hover {
            if d < 6.0 {
                hovered_info = Some((ring.name, ring.blurb));
            }
        }
    }
    hovered_info
}

/// A reference ring of radius `r_m` round `centre`, `n` points in the
/// body's equatorial plane: the world's XZ plane (the game has no axial tilt).
pub(super) fn reference_ring_points(centre: glam::DVec3, r_m: f64, n: usize) -> Vec<glam::DVec3> {
    (0..n)
        .map(|i| {
            let theta = i as f64 * std::f64::consts::TAU / n as f64;
            centre + glam::DVec3::new(r_m * theta.cos(), 0.0, r_m * theta.sin())
        })
        .collect()
}

// ─────────────────────── Lagrange points (Phase 4d-soi, v0.211.0) ──────────

/// A Lagrange-point pair worth rendering. The "parent" is the heavier body
/// (Sun, Earth, etc.), the "child" is the orbiting smaller body.
struct LagrangePair {
    parent_id: &'static str,
    child_id: &'static str,
    pair_label: &'static str, // short name shown on hover
    notable: &'static [(&'static str, &'static str)], // (lpoint, what's there)
}

/// Hardcoded list of interesting Lagrange pairs. Could move to data/ later
/// once we have a notion of "system bodies + L-point notes" in the data
/// format — but for v0.211 a static list of 5 pairs is plenty.
const LAGRANGE_PAIRS: &[LagrangePair] = &[
    LagrangePair {
        parent_id: "sun",
        child_id: "earth",
        pair_label: "Sun-Earth",
        notable: &[
            ("L1", "SOHO solar observatory"),
            ("L2", "JWST + Gaia + Euclid + Planck"),
            ("L4", "Trojan asteroid 2010 TK7"),
        ],
    },
    LagrangePair {
        parent_id: "earth",
        child_id: "moon",
        pair_label: "Earth-Moon",
        notable: &[
            ("L1", "Future lunar gateway candidate"),
            ("L2", "Lunar far-side relay (Queqiao)"),
            ("L4", "Future lunar gateway candidate"),
            ("L5", "Future lunar gateway candidate"),
        ],
    },
    LagrangePair {
        parent_id: "sun",
        child_id: "mars",
        pair_label: "Sun-Mars",
        notable: &[("L4", "Eureka Trojan asteroid")],
    },
    LagrangePair {
        parent_id: "sun",
        child_id: "jupiter",
        pair_label: "Sun-Jupiter",
        notable: &[
            ("L4", "Greek Trojan asteroids (~10,000 known)"),
            ("L5", "Trojan asteroids (Patroclus, Hektor)"),
        ],
    },
    LagrangePair {
        parent_id: "sun",
        child_id: "saturn",
        pair_label: "Sun-Saturn",
        notable: &[],
    },
];

/// Compute the five Lagrange points for a (parent, child) two-body system
/// at the given sim_time. Returns positions in heliocentric AU.
///
/// L1, L2, L3 use the cube-root-of-mass-ratio approximation
/// (good to ~1% for μ < 0.01, which covers all our pairs since even
/// Jupiter/Sun is ~0.001). L4 and L5 are the exact 60°-ahead/behind
/// equilateral triangles in the child's orbit plane.
pub(super) fn compute_lagrange_points(
    parent: &SolBody,
    child: &SolBody,
    sim_time_seconds: f64,
) -> Option<[glam::DVec3; 5]> {
    if parent.mass_kg <= 0.0 || child.mass_kg <= 0.0 { return None; }
    let parent_pos = body_world_position_3d_au(parent, sim_time_seconds);
    let child_pos = body_world_position_3d_au(child, sim_time_seconds);
    let separation = child_pos - parent_pos;
    let dist = separation.length();
    if dist <= 0.0 { return None; }
    let to_child = separation / dist;

    // μ = m_child / (m_parent + m_child). For Earth-Moon μ ≈ 0.0123;
    // for Sun-Earth μ ≈ 3e-6; for Sun-Jupiter μ ≈ 9.5e-4.
    let mu = child.mass_kg / (parent.mass_kg + child.mass_kg);
    let r_hill_factor = (mu / 3.0).cbrt();
    let l1_dist = dist * (1.0 - r_hill_factor);
    let l2_dist = dist * (1.0 + r_hill_factor);
    let l3_dist = dist * (1.0 + 5.0 * mu / 12.0);

    let l1 = parent_pos + to_child * l1_dist;
    let l2 = parent_pos + to_child * l2_dist;
    let l3 = parent_pos - to_child * l3_dist;

    // L4 / L5 = 60° rotation in the child's orbit plane, about its orbit
    // normal r x v, so a positive turn goes the way the child moves: L4
    // leads, L5 trails. The velocity is the child's own motion round the
    // parent over an hour. (Until Maps increment 1 this used "ecliptic up =
    // +Z", which lies IN the ecliptic of the engine's Y-up frame: L4 and L5
    // swapped every half orbit and went NaN on the Z axis.)
    let later = body_world_position_3d_au(child, sim_time_seconds + 3600.0)
        - body_world_position_3d_au(parent, sim_time_seconds + 3600.0);
    let orbit_normal = separation.cross(later - separation).normalize_or_zero();
    if orbit_normal == glam::DVec3::ZERO {
        return None;
    }

    // Rotate `separation` by +60° and -60° about orbit_normal.
    let angle_rad = 60.0_f64.to_radians();
    let rot = |v: glam::DVec3, axis: glam::DVec3, theta: f64| -> glam::DVec3 {
        // Rodrigues' rotation formula.
        let (s, c) = theta.sin_cos();
        v * c + axis.cross(v) * s + axis * (axis.dot(v)) * (1.0 - c)
    };
    let l4 = parent_pos + rot(separation, orbit_normal, angle_rad);
    let l5 = parent_pos + rot(separation, orbit_normal, -angle_rad);

    Some([l1, l2, l3, l4, l5])
}

// ─────────────────────── Body info card (Phase 4d-bis, v0.209.0) ───────────

/// Render the expanded info card for a SolBody. v0.214.0 takes a
/// `PlacedPill` (from the widget's phase-1 layout) so the card can attach
/// FLUSH to the pill's bottom edge — no duplicate body dot in the card
/// heading, no gap. The pill is the card's visual header; the card is
/// just the pill's downward extension.
///
/// Translates the SolBody into a generic `BodyCardData` and delegates to
/// `widgets::body_pill::paint_card_extension`. The widget owns the
/// visual layout (anchor flush with pill, flip-up if clipping bottom,
/// canvas clamp); this function owns the domain translation (formatting
/// "5.2 AU from Sun" etc.) plus the Maps-specific actions.
fn draw_body_info_card_v2(
    ui: &mut egui::Ui,
    paint: &egui::Painter,
    theme: &Theme,
    body: &SolBody,
    pp: &crate::gui::widgets::body_pill::PlacedPill,
    canvas_rect: Rect,
    state: &mut GuiState,
) {
    use crate::gui::widgets::body_pill::{BodyCardData, paint_card_extension};

    let dist_str = if body.semi_major_axis_au > 0.0 {
        format!("{:.2} AU from Sun", body.semi_major_axis_au)
    } else if body.semi_major_axis_km > 0.0 {
        format!(
            "{} km from {}",
            format_with_commas(body.semi_major_axis_km as i64),
            body.parent.as_deref().map(titlecase).unwrap_or_default(),
        )
    } else {
        String::new()
    };
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
    if !dist_str.is_empty() { subtitle_parts.push(dist_str); }
    if !period_str.is_empty() { subtitle_parts.push(period_str); }
    let subtitle = Some(subtitle_parts.join(" · "));

    let mut stats: Vec<(String, String)> = Vec::new();
    if body.radius_km > 0.0 {
        stats.push(("Radius".to_string(), format_km(body.radius_km)));
    }
    if body.mean_temperature_k > 0.0 {
        let temp_c = body.mean_temperature_k - 273.15;
        stats.push((
            "Mean temp".to_string(),
            format!("{:.0} °C ({:.0} K)", temp_c, body.mean_temperature_k),
        ));
    }
    if body.surface_gravity_ms2 > 0.0 {
        stats.push((
            "Surface gravity".to_string(),
            format!("{:.2} m/s²", body.surface_gravity_ms2),
        ));
    }

    let description = if body.description.is_empty() {
        None
    } else {
        Some(body.description.as_str())
    };

    let is_tracking = state.maps.tracked_body.as_deref() == Some(body.id.as_str());
    let actions = vec![
        ("Focus".to_string(), true),
        (if is_tracking { "Stop Tracking".to_string() } else { "Track".to_string() }, true),
    ];

    let data = BodyCardData {
        subtitle,
        stats,
        description,
        actions,
    };

    let resp = paint_card_extension(ui, paint, theme, pp, &data, canvas_rect);
    if resp.closed {
        state.maps.expanded_body = None;
    }
    if let Some(idx) = resp.action_clicked {
        match idx {
            0 => {
                state.maps.focus_request = Some(body.id.clone());
                state.maps.selected_body = Some(body.id.clone());
            }
            1 => {
                // Toggle tracking; starting tracking also snaps the camera
                // to the body immediately via a Focus request, rather than
                // waiting for the next frame's drift.
                state.maps.tracked_body = toggle_tracked_body(
                    state.maps.tracked_body.as_deref(),
                    &body.id,
                );
                if state.maps.tracked_body.is_some() {
                    state.maps.focus_request = Some(body.id.clone());
                }
            }
            _ => {}
        }
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
