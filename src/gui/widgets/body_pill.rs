//! Universal "world object label" widget — a single widget that morphs
//! through three visual states tied to the same anchor:
//!
//!   • **Circle** — the object's own rendered dot (no pill yet).
//!   • **Pill** — the dot grows a rightward extension to fit a name label.
//!     The object IS the pill's left cap; there is no separate "dot inside
//!     the pill". Only the pill's right end + name text are new visuals.
//!   • **Card** — the pill grows a downward extension into a rectangular
//!     info card. The pill stays as the card's "header"; there is no
//!     duplicate body dot inside the card heading.
//!
//! ## Why this is one widget, not three
//!
//! Operator insight 2026-05-12: *"It should be a single thing. It goes
//! from circle to pill to rectangular card all the while the planet icon
//! remains on the original first one and the name is always right next to
//! whatever is being looked at."*
//!
//! Before this refactor, the Cosmos page rendered three separate visual
//! elements for an expanded body: (1) the body in the canvas, (2) a pill
//! floating above-right with its OWN duplicate dot, (3) the expanded card
//! with a THIRD duplicate dot in its heading. That broke the user's
//! "I'm interacting with ONE thing" mental model. The widget now treats
//! all three states as a single continuous outline, with the body's own
//! rendered position as the anchor in every state.
//!
//! ## Why this is a universal widget
//!
//! Originally built for the Cosmos page (celestial bodies + 3D camera
//! projection). The same data shape works for **anything in world space
//! that needs a label**:
//!
//!   • Cosmos page (shipped) — celestial bodies, 3D camera projection
//!   • In-ship Map Room HUD (planned) — celestial bodies, FPS camera
//!   • AR-glasses sky overlay (Phase 4g) — bodies in real sky, AR pose
//!   • FPS loot pickups — items on the ground, FPS camera
//!   • FPS vehicle markers — vehicles in the distance, FPS camera
//!   • FPS NPC/player nameplates — entities in view, FPS camera
//!   • Anything else with a world-position and a name.
//!
//! All cases share the compute: project a world position to a 2D screen
//! position; render the dot-pill-card progression at that anchor; hit-test
//! for hover/click; expand on click. Only the source of `screen_pos` and
//! the contents of the card differ.
//!
//! ## Two-phase rendering
//!
//! For the "body is the pill's left cap" trick to work, the body circle
//! must be drawn ON TOP of the pill background. The widget therefore
//! exposes two phases that the caller invokes around its body draw:
//!
//!   1. `paint_pill_backgrounds(...)` — computes layout (with collision-
//!      dodge), paints just the filled rounded-rect backgrounds, returns
//!      `PillsLayout` with the placed rects.
//!   2. Caller draws the body circles + any decorations (conjunction
//!      rings, eclipse highlights, etc.) on top of the backgrounds.
//!   3. `paint_pill_overlays(...)` — paints the borders, name text, and
//!      handles click interaction. Returns `Option<String>` of any
//!      clicked pill id.
//!   4. `paint_card_extension(...)` — for any expanded pill, paints the
//!      card area below it. Returns `BodyCardResponse`.

use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use crate::gui::theme::Theme;

/// Minimum widget radius in px. Drives ALL THREE matching circles
/// (cap around body + pill top-left corner + panel top-left corner) so
/// they're always the same size at every zoom level (operator feedback
/// 2026-05-12 — "It should be like 3 overlapping circles of the same
/// size"). When the body's natural radius is larger than this, the
/// widget radius scales up with it.
const MIN_WIDGET_RADIUS: f32 = 9.0;

/// Per-pill input. Caller produces a `Vec<BodyPill>` per frame; the
/// widget runs collision-dodge + renders the survivors.
#[derive(Clone)]
pub struct BodyPill<'a> {
    pub id: &'a str,
    pub name: &'a str,
    /// Color of the underlying body — used only for the (small) card
    /// heading marker; NOT drawn as a separate dot in the pill itself
    /// since the body's own circle is the pill's left cap.
    pub color: Color32,
    /// Screen-space center of the body the pill belongs to. The pill's
    /// left semicircular cap is centered on this point.
    pub body_screen: Pos2,
    /// Apparent on-screen radius of the body in pixels. Used to set the
    /// pill's height when the body is larger than `MIN_PILL_HEIGHT/2`,
    /// so the pill's left cap exactly matches the body's silhouette.
    pub body_radius_px: f32,
    /// Priority for collision-dodge — lower = drawn first / never hidden.
    /// Typical mapping for celestial bodies: star=0, planets=1, dwarf
    /// planets=2, moons=3, asteroids=4. Generic-world callers can use 0
    /// for "always visible" and higher numbers for "hide on overlap".
    pub priority: u8,
    /// Forced visibility — bypass collision-dodge. Typically true for
    /// hover/select/expanded states.
    pub forced: bool,
    /// Whether the pill should be styled as "expanded" (thicker accent
    /// border) and have a card extension drawn below it.
    pub expanded: bool,
}

/// A placed pill — the result of running the collision-dodged layout.
#[derive(Clone)]
pub struct PlacedPill {
    pub id: String,
    pub name: String,
    pub color: Color32,
    pub rect: Rect,
    pub body_screen: Pos2,
    pub body_radius_px: f32,
    pub forced: bool,
    pub expanded: bool,
}

/// Layout result. The caller uses this to drive Phase 2 (overlays) and
/// Phase 3 (card extensions).
pub struct PillsLayout {
    pub placed: Vec<PlacedPill>,
}

/// PHASE 0 — pure layout computation (no paint). Returns the placed
/// pill rects with priority-sorted collision-dodge applied. The caller
/// uses this layout to drive PHASE 1 (panel), PHASE 2 (pill backgrounds),
/// and PHASE 3 (pill overlays). Splitting compute from paint lets the
/// caller order the paint phases freely (panel must render UNDER pill
/// + body, so the order is panel → pill bg → body → pill border + text).
pub fn compute_pill_layout(
    painter: &egui::Painter,
    theme: &Theme,
    pills: &[BodyPill<'_>],
) -> PillsLayout {
    let mut sorted: Vec<&BodyPill<'_>> = pills.iter().collect();
    sorted.sort_by_key(|c| (!c.forced as u8, c.priority));

    let mut placed: Vec<PlacedPill> = Vec::with_capacity(sorted.len());
    let mut placed_rects: Vec<Rect> = Vec::with_capacity(sorted.len());

    for c in &sorted {
        // Widget radius — drives the cap, pill corner, AND panel corner.
        // Always at least MIN_WIDGET_RADIUS, otherwise scales with body
        // (body_radius + 1 so the cap is exactly 1 px outside the body
        // when the body is larger than the minimum). All three circles
        // (cap around body, pill top-left corner, panel top-left corner)
        // use this same radius — operator's "3 overlapping circles of
        // the same size" requirement.
        let widget_radius = (c.body_radius_px + 1.0).max(MIN_WIDGET_RADIUS);
        let pill_height = widget_radius * 2.0;
        let half_h = widget_radius;

        // Measure name text to determine pill width.
        let font = egui::FontId::proportional(11.0);
        let text_galley = painter.layout_no_wrap(
            c.name.to_string(),
            font,
            theme.text_primary(),
        );
        // Name starts just past the cap's right edge (NOT the body's
        // edge — the cap is what the user perceives as the body when the
        // pill is visible).
        let name_start_x = c.body_screen.x + half_h + 6.0;
        let h_pad_right = 10.0;
        let pill_right = name_start_x + text_galley.size().x + h_pad_right;

        let pill_left = c.body_screen.x - half_h;
        let pill_top = c.body_screen.y - half_h;
        let pill_rect = Rect::from_min_size(
            Pos2::new(pill_left, pill_top),
            Vec2::new(pill_right - pill_left, pill_height),
        );

        if !c.forced && placed_rects.iter().any(|r| r.intersects(pill_rect)) {
            continue;
        }
        placed_rects.push(pill_rect);

        placed.push(PlacedPill {
            id: c.id.to_string(),
            name: c.name.to_string(),
            color: c.color,
            rect: pill_rect,
            body_screen: c.body_screen,
            body_radius_px: c.body_radius_px,
            forced: c.forced,
            expanded: c.expanded,
        });
    }

    PillsLayout { placed }
}

/// PHASE 2 — Paint the pill BACKGROUNDS for the given layout. Includes
/// the body-colored cap fill so the cap visually appears AS the body
/// (no "tiny body floating in big empty cap" look — operator feedback
/// 2026-05-12). Render order is:
///
///   PHASE 1 (panel, if expanded — UNDER everything)
///   PHASE 2 (this — pill bg with body-colored cap)
///   caller draws body circles + decorations (selected ring, conjunction
///       rings, eclipse highlights — these layer ON TOP of the cap fill)
///   PHASE 3 (pill borders + name + interaction — ON TOP of bodies)
///
/// The body-colored cap is just a filled circle at body_screen with
/// radius = `cap_radius - 0.5`. The rest of the pill (where the name
/// goes) is filled with `bg_card` for legibility.
pub fn paint_pill_backgrounds(
    painter: &egui::Painter,
    theme: &Theme,
    layout: &PillsLayout,
) {
    for pp in &layout.placed {
        let pill_h = pp.rect.height();
        let half_h = pill_h * 0.5;
        let radius = half_h;

        // Step 1: fill the entire pill rect with the dark bg_card. This
        // gives the name area its background.
        painter.rect_filled(pp.rect, radius, theme.bg_card());

        // Step 2: fill the cap area (left semicircle) with the body's
        // color. This is the "cap visually = body" trick — when the
        // body is then drawn at its natural smaller radius on top, it's
        // invisible against this colored fill, so the cap reads as one
        // solid body-colored disk regardless of how small the body's
        // natural radius is.
        let cap_center = Pos2::new(pp.rect.left() + half_h, pp.rect.center().y);
        painter.circle_filled(cap_center, radius - 0.5, pp.color);
    }
}

/// PHASE 3 — Paint pill borders + vertical divider + name text, and
/// handle click interaction. Call AFTER the body draw pass so the
/// borders render on top of body circles + conjunction/eclipse rings.
///
/// Border colors: ALL pills with `forced` or `expanded` get the strong
/// accent border (matches operator's "the orange circle should be
/// encompassing the object" feedback 2026-05-12). Non-forced pills
/// (always-visible planets that the user isn't interacting with) get a
/// subtle muted border.
///
/// Returns the id of the pill clicked this frame, if any.
pub fn paint_pill_overlays(
    ui: &mut Ui,
    painter: &egui::Painter,
    theme: &Theme,
    layout: &PillsLayout,
    interact_id_salt: &str,
) -> Option<String> {
    let mut clicked_id: Option<String> = None;

    for pp in &layout.placed {
        let pill_id = ui.id().with((interact_id_salt, pp.id.as_str()));
        let pill_response = ui.interact(pp.rect, pill_id, Sense::click());
        let pill_hovered = pill_response.hovered();

        let pill_h = pp.rect.height();
        let half_h = pill_h * 0.5;

        // Border: STRONG accent for any forced/hovered/expanded state
        // (the user is "looking at" or interacting with this body).
        // Subtle for non-interacting always-shown planets.
        let border_stroke = if pp.expanded {
            Stroke::new(2.0, theme.accent())
        } else if pp.forced || pill_hovered {
            Stroke::new(1.4, theme.accent())
        } else {
            Stroke::new(0.5, theme.border())
        };
        painter.rect_stroke(pp.rect, half_h, border_stroke, egui::StrokeKind::Outside);

        // Vertical divider line between cap and name area (per operator's
        // sketch 2026-05-12 — "There's no line between the icon and title
        // text like on my paint drawing example"). The line sits at the
        // cap's right edge (which is where the body color ends and the
        // bg_card name area begins).
        let divider_x = pp.rect.left() + half_h;
        let divider_color = if pp.forced || pp.expanded || pill_hovered {
            theme.accent()
        } else {
            theme.border()
        };
        painter.line_segment(
            [
                Pos2::new(divider_x, pp.rect.top() + 2.0),
                Pos2::new(divider_x, pp.rect.bottom() - 2.0),
            ],
            Stroke::new(0.8, divider_color),
        );

        // Name text — positioned just past the cap's right edge.
        let font = egui::FontId::proportional(11.0);
        let text_galley = painter.layout_no_wrap(
            pp.name.clone(),
            font,
            theme.text_primary(),
        );
        let text_pos = Pos2::new(
            pp.rect.left() + half_h + 6.0,
            pp.rect.center().y - text_galley.size().y * 0.5,
        );
        painter.galley(text_pos, text_galley, theme.text_primary());

        if pill_response.clicked() {
            clicked_id = Some(pp.id.clone());
        }
    }

    clicked_id
}

// ─────────────────────── Info card (Phase 3) ────────────────────────────────

/// Data describing one expandable body info card. Pure presentation —
/// the widget knows nothing about SolBody, ECS components, AR metadata,
/// loot stacks, vehicles, etc. Callers translate their domain types into
/// this struct and the widget renders it consistently.
pub struct BodyCardData<'a> {
    /// Optional subtitle under the heading — typically "Type · Distance · Period"
    /// for celestial bodies, or "Stack of 12 · Common" for loot, etc.
    pub subtitle: Option<String>,
    /// Key/value rows for physical stats. Drawn in `text_muted`.
    pub stats: Vec<(String, String)>,
    /// Optional free-form description. Truncated to ~180 chars if longer.
    pub description: Option<&'a str>,
    /// Action buttons at the bottom — `(label, enabled)`. The widget
    /// returns the index of the clicked action via `BodyCardResponse`.
    pub actions: Vec<(String, bool)>,
}

/// Response from `paint_card_extension`.
pub struct BodyCardResponse {
    /// True if the user clicked the Close button.
    pub closed: bool,
    /// Index into `BodyCardData::actions` if an action was clicked.
    pub action_clicked: Option<usize>,
}

/// PHASE 1 — Paint the details PANEL, positioned BELOW the pill as a
/// fully separate widget (per operator feedback 2026-05-12 — "The circle,
/// pill, and card should be treated as separate things so their
/// padding/margins don't interfere with each other").
///
/// **MUST be called BEFORE `paint_pill_backgrounds` and the body draw**
/// so the body + pill cap layer ON TOP of any panel area in their
/// vicinity. The full render order is:
///
///   PHASE 0: `compute_pill_layout` — pure compute, no paint.
///   PHASE 1: `paint_card_extension` — panel BELOW pill (this fn).
///   PHASE 2: `paint_pill_backgrounds` — pill fills + body-colored cap.
///   caller: draw body circles + decorations.
///   PHASE 3: `paint_pill_overlays` — pill border + divider + name.
///
/// Layout (panel as a SEPARATE widget below the pill):
///
/// ```text
///   ╭─[pill]───╮      ← pill is its own thing — body + cap + name.
///   │ ⊙  Name  │        Pill has its own border/padding/margin.
///   ╰──────────╯
///   ╭──────────────────────────╮  ← panel is a SEPARATE widget below.
///   │ Close button here →      │     Panel has its own border/padding.
///   │ Subtitle                 │     Cap is NOT inside this rect.
///   │ Stats                    │
///   │ Description              │
///   │ [Focus] [Track]          │
///   ╰──────────────────────────╯
/// ```
///
/// Width: `max(pill_width, panel_min_width)` so the panel can extend
/// further right than the pill when the name is short.
///
/// If the panel would extend past the canvas bottom, it flips UP and
/// renders above the pill instead. If past the right edge, the width
/// is clamped (the panel stays attached to the pill's left edge).
pub fn paint_card_extension(
    ui: &mut Ui,
    painter: &egui::Painter,
    theme: &Theme,
    pp: &PlacedPill,
    card: &BodyCardData<'_>,
    canvas_rect: Rect,
) -> BodyCardResponse {
    let panel_min_w = 240.0_f32;
    let panel_h = 160.0_f32;
    let panel_gap = 4.0_f32; // Small visual gap between pill and panel.

    // Width: at least the pill's width, at least panel_min_w. Clamped to
    // canvas right edge (don't shift left — the panel stays attached to
    // the pill's left edge so it visually aligns with the body+pill above).
    let mut panel_w = pp.rect.width().max(panel_min_w);
    let max_w_from_canvas = (canvas_rect.right() - 8.0 - pp.rect.left()).max(panel_min_w);
    panel_w = panel_w.min(max_w_from_canvas);

    // Vertical: prefer below pill; flip above if it would clip.
    let mut extend_down = true;
    if pp.rect.bottom() + panel_gap + panel_h > canvas_rect.bottom() - 8.0 {
        let panel_top_if_up = pp.rect.top() - panel_gap - panel_h;
        if panel_top_if_up >= canvas_rect.top() + 8.0 {
            extend_down = false;
        }
    }

    let panel_top = if extend_down {
        pp.rect.bottom() + panel_gap
    } else {
        pp.rect.top() - panel_gap - panel_h
    };
    let panel_rect = Rect::from_min_size(
        Pos2::new(pp.rect.left(), panel_top),
        Vec2::new(panel_w, panel_h),
    );

    // Paint the panel: filled rounded rect with all 4 corners rounded,
    // soft gray border. Fully self-contained — pill area is NOT inside
    // this rect, so the panel's padding/margin don't affect the cap.
    //
    // Corner radius matches the pill's corner radius (which equals the
    // cap radius) so all three "circles" — cap around body, pill top-
    // left, panel top-left — are the same size (operator feedback
    // 2026-05-12 — "3 overlapping circles of the same size").
    let panel_corner_radius = pp.rect.height() * 0.5;
    painter.rect_filled(panel_rect, panel_corner_radius, theme.bg_card());
    painter.rect_stroke(
        panel_rect,
        panel_corner_radius,
        Stroke::new(1.0, theme.border()),
        egui::StrokeKind::Outside,
    );

    let mut response = BodyCardResponse {
        closed: false,
        action_clicked: None,
    };

    // Render content inside the panel with standard 10 px inner margin.
    // Close button is in the top-right of the heading row (first row
    // of content). Subtitle / stats / description / actions follow.
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(panel_rect)
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
    );
    egui::Frame::NONE
        .inner_margin(egui::Margin::same(10))
        .show(&mut child, |ui| {
            ui.set_min_width(panel_rect.width() - 20.0);
            ui.set_max_width(panel_rect.width() - 20.0);

            // Top row — subtitle on left, Close button on right.
            ui.horizontal(|ui| {
                if let Some(ref s) = card.subtitle {
                    ui.label(
                        RichText::new(s)
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                }
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        if ui.small_button("Close").clicked() {
                            response.closed = true;
                        }
                    },
                );
            });

            for (k, v) in &card.stats {
                ui.label(
                    RichText::new(format!("{}: {}", k, v))
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }

            if let Some(desc) = card.description {
                ui.add_space(4.0);
                let trunc = if desc.chars().count() > 180 {
                    let head: String = desc.chars().take(180).collect();
                    format!("{}…", head)
                } else {
                    desc.to_string()
                };
                ui.label(
                    RichText::new(trunc)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
            }

            if !card.actions.is_empty() {
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for (i, (label, enabled)) in card.actions.iter().enumerate() {
                        let btn = ui.add_enabled(
                            *enabled,
                            egui::Button::new(label.as_str()),
                        );
                        if btn.clicked() {
                            response.action_clicked = Some(i);
                        }
                    }
                });
            }
        });

    let _ = pp.color;
    response
}
