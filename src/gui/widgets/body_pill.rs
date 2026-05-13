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

/// Minimum pill height in px — enough to fit the name text + padding.
/// Tuned 2026-05-12 (operator feedback) — smaller cap so small bodies
/// don't look "lost" inside an oversized cap. The body-colored cap fill
/// (see `paint_pill_backgrounds`) makes the cap visually appear AS the
/// body's left dot, regardless of natural body radius, so this can be
/// kept tight.
const MIN_PILL_HEIGHT: f32 = 18.0;

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
        // Pill height: prefer the body's natural diameter so the left cap
        // matches the body's silhouette exactly; floor at MIN_PILL_HEIGHT
        // so small bodies (e.g. asteroids) still have a legible label.
        let pill_height = (c.body_radius_px * 2.0 + 2.0).max(MIN_PILL_HEIGHT);
        let half_h = pill_height * 0.5;

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

/// PHASE 3 — Paint the details PANEL behind an expanded pill.
///
/// **MUST be called BEFORE `paint_pill_overlays`** so the pill border
/// layers ON TOP of the panel border. The render order is:
///
///   1. `paint_pill_backgrounds` — pill fills (caller draws bodies on top).
///   2. caller draws bodies + decorations.
///   3. `paint_card_extension` — the details panel (BEHIND pill).
///   4. `paint_pill_overlays` — pill borders + name (ON TOP of panel border).
///
/// Layout (matches operator's 2026-05-12 sketch):
///
/// ```text
///   ┌─[pill]──┐ ←──── pill (strong/accent border) sits on top
///   │ ⊙  Name │
///   └─────────┘
///        ┌───────────────────────── panel ──────────┐
///        │ extends right past pill if content needs │
///        │ panel border is SOFT (gray); pill border │
///        │ is STRONG (accent). They overlap in the  │
///        │ pill area; pill border covers panel here │
///        │                                          │
///        └──────────────────────────────────────────┘
/// ```
///
/// The pill's top-left is the same as the panel's top-left — they share
/// the top-left corner. The pill's bottom edge stays visible inside the
/// panel, acting as a divider between title and content.
///
/// Width: `max(pill_width, panel_min_width)` so the panel can extend
/// further right than the pill when the name is short.
///
/// Auto-flip: if the panel would extend past the canvas bottom, it
/// flips up so the pill is at the panel's BOTTOM edge instead of its
/// top edge, with content rendered ABOVE the pill.
pub fn paint_card_extension(
    ui: &mut Ui,
    painter: &egui::Painter,
    theme: &Theme,
    pp: &PlacedPill,
    card: &BodyCardData<'_>,
    canvas_rect: Rect,
) -> BodyCardResponse {
    let panel_min_w = 240.0_f32;
    let pill_h = pp.rect.height();
    let content_h_est = 150.0_f32;
    let panel_h = pill_h + content_h_est;

    // Width: at least the pill's width, at least panel_min_w. If the
    // panel would extend past the canvas right edge, clamp the width
    // (don't shift the panel left, because that would detach it from
    // the pill which is anchored to the body).
    let mut panel_w = pp.rect.width().max(panel_min_w);
    let max_w_from_canvas = (canvas_rect.right() - 8.0 - pp.rect.left()).max(panel_min_w);
    panel_w = panel_w.min(max_w_from_canvas);

    // Vertical: prefer extending down (panel.top = pill.top); flip up if
    // it would clip the canvas bottom (panel.bottom = pill.bottom; content
    // renders above the pill).
    let mut extend_down = true;
    if pp.rect.top() + panel_h > canvas_rect.bottom() - 8.0 {
        // Try flipping up: panel ends at pill.bottom, extends upward.
        let panel_top_if_up = pp.rect.bottom() - panel_h;
        if panel_top_if_up >= canvas_rect.top() + 8.0 {
            extend_down = false;
        }
        // Else: stay extending down, accept the clip.
    }

    let panel_top = if extend_down {
        pp.rect.top()
    } else {
        pp.rect.bottom() - panel_h
    };
    let panel_rect = Rect::from_min_size(
        Pos2::new(pp.rect.left(), panel_top),
        Vec2::new(panel_w, panel_h),
    );

    // Paint the panel background + SOFT (gray) border.
    let panel_corner_radius = 8.0_f32;
    painter.rect_filled(panel_rect, panel_corner_radius, theme.bg_card());
    painter.rect_stroke(
        panel_rect,
        panel_corner_radius,
        Stroke::new(1.0, theme.border()),
        egui::StrokeKind::Outside,
    );

    // Content area: below the pill (when extending down) or above the
    // pill (when flipped up). Inset by 10 px on left/right; pill height
    // worth of vertical inset on the appropriate side.
    let content_top = if extend_down {
        pp.rect.bottom() + 6.0
    } else {
        panel_rect.top() + 10.0
    };
    let content_bottom = if extend_down {
        panel_rect.bottom() - 10.0
    } else {
        pp.rect.top() - 6.0
    };
    let content_rect = Rect::from_min_max(
        Pos2::new(panel_rect.left() + 10.0, content_top),
        Pos2::new(panel_rect.right() - 10.0, content_bottom),
    );

    // Close button overlay — top-right of the panel, ABOVE the content.
    // We place it in the top-right corner where the panel extends past
    // the pill (or in the top-right of the panel if the panel and pill
    // are the same width).
    let mut response = BodyCardResponse {
        closed: false,
        action_clicked: None,
    };
    let close_rect = Rect::from_min_size(
        Pos2::new(panel_rect.right() - 50.0, panel_rect.top() + 4.0),
        Vec2::new(44.0, pill_h - 6.0),
    );
    let mut close_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(close_rect)
            .layout(egui::Layout::centered_and_justified(egui::Direction::TopDown)),
    );
    if close_ui.small_button("Close").clicked() {
        response.closed = true;
    }

    // Render content inside the content area (below or above the pill).
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(content_rect)
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
    );
    egui::Frame::NONE
        .inner_margin(egui::Margin::ZERO)
        .show(&mut child, |ui| {
            ui.set_min_width(content_rect.width());
            ui.set_max_width(content_rect.width());

            if let Some(ref s) = card.subtitle {
                ui.label(
                    RichText::new(s)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
            }

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

    // Use `_` to silence unused-pp.color warning. (Caller can use this
    // when rendering loot-rarity-tinted card variants etc.)
    let _ = pp.color;

    response
}
