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
const MIN_PILL_HEIGHT: f32 = 22.0;

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

/// PHASE 1 — Compute pill layout (with priority-sorted collision-dodge)
/// AND paint just the filled rounded-rect backgrounds. Call BEFORE
/// drawing the body circles so they render ON TOP of the backgrounds
/// and become the pills' left caps.
///
/// The pill is laid out such that its left semicircular cap is exactly
/// centered on the body. Height = `max(body_diameter + 2, MIN_PILL_HEIGHT)`.
/// Corner radius = height/2 (so left and right ends are fully rounded).
pub fn paint_pill_backgrounds(
    painter: &egui::Painter,
    theme: &Theme,
    pills: &[BodyPill<'_>],
) -> PillsLayout {
    // Sort: forced pills first (so they always claim their slot), then by
    // priority. Stable sort keeps draw-order deterministic frame-to-frame.
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
        // The name starts at body_center.x + body_radius + small gap, so
        // even when the pill cap is larger than the body, the name sits
        // just past the visual body. Padding adds a little air on the right.
        let name_start_x = c.body_screen.x + c.body_radius_px + 6.0;
        let h_pad_right = 10.0;
        let pill_right = name_start_x + text_galley.size().x + h_pad_right;

        let pill_left = c.body_screen.x - half_h;
        let pill_top = c.body_screen.y - half_h;
        let pill_rect = Rect::from_min_size(
            Pos2::new(pill_left, pill_top),
            Vec2::new(pill_right - pill_left, pill_height),
        );

        // Collision check — only forced pills override.
        if !c.forced && placed_rects.iter().any(|r| r.intersects(pill_rect)) {
            continue;
        }
        placed_rects.push(pill_rect);

        // Paint the background fill. The body circle (drawn by the
        // caller, on top of this) becomes the pill's visible left cap.
        // We use theme.bg_card() — opaque so the body reads cleanly.
        let bg = theme.bg_card();
        let radius = half_h;
        painter.rect_filled(pill_rect, radius, bg);

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

/// PHASE 2 — Paint pill borders + name text, and handle click interaction.
/// Call AFTER the body draw pass so the borders render on top of body
/// circles + conjunction/eclipse decorations.
///
/// Returns the id of the pill clicked this frame, if any. The caller
/// inspects this to drive expanded-state toggling.
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

        // Border styling: expanded = thicker accent; forced (hovered/
        // selected) = focus tone; otherwise = subtle border.
        // (The body circle covers the LEFT portion of the border, so what
        // the user sees is effectively a ring around the body + an outline
        // around the name extension. That's what we want.)
        let border_stroke = if pp.expanded {
            Stroke::new(1.8, theme.accent())
        } else if pp.forced || pill_hovered {
            Stroke::new(1.0, theme.border_focus())
        } else {
            Stroke::new(0.5, theme.border())
        };
        let radius = pp.rect.height() * 0.5;
        painter.rect_stroke(pp.rect, radius, border_stroke, egui::StrokeKind::Outside);

        // Name text — positioned just past the body's right edge so it's
        // immediately adjacent to whatever the user is looking at.
        let font = egui::FontId::proportional(11.0);
        let text_galley = painter.layout_no_wrap(
            pp.name.clone(),
            font,
            theme.text_primary(),
        );
        let text_pos = Pos2::new(
            pp.body_screen.x + pp.body_radius_px + 6.0,
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

/// PHASE 3 — Paint the card extension below an expanded pill.
///
/// The card grows DOWNWARD from the pill's bottom edge, sharing the
/// pill's left edge so the user sees one continuous outline (pill on
/// top, card below, connected). If extending down would clip the bottom
/// of the canvas, the card flips up and extends above the pill instead.
///
/// The card's heading row reuses the pill's name + body color as the
/// visual anchor — there is NO duplicate body dot inside the card.
pub fn paint_card_extension(
    ui: &mut Ui,
    painter: &egui::Painter,
    theme: &Theme,
    pp: &PlacedPill,
    card: &BodyCardData<'_>,
    canvas_rect: Rect,
) -> BodyCardResponse {
    let card_w_min = 220.0_f32;
    // Card width: at least the pill's width (so it never looks narrower
    // than its header), at least card_w_min for content legibility.
    let card_w = pp.rect.width().max(card_w_min);
    let card_h_est = 160.0_f32;

    // Try to extend down from pill.bottom. Flip up if it would clip.
    let mut extend_down = true;
    let mut card_top = pp.rect.bottom();
    if card_top + card_h_est > canvas_rect.bottom() - 8.0 {
        extend_down = false;
        card_top = pp.rect.top() - card_h_est;
    }
    // If flipping up still clips, fall back to extending down with bottom
    // clamped (the card will overflow the canvas a little, but staying
    // attached to the pill is more important than fitting perfectly).
    if !extend_down && card_top < canvas_rect.top() + 8.0 {
        extend_down = true;
        card_top = pp.rect.bottom();
    }

    // Card horizontal position: align left with pill. If the card is
    // wider than the pill and would extend past the canvas right edge,
    // shift the whole card left so it fits.
    let mut card_left = pp.rect.left();
    if card_left + card_w > canvas_rect.right() - 8.0 {
        card_left = canvas_rect.right() - 8.0 - card_w;
    }
    if card_left < canvas_rect.left() + 8.0 {
        card_left = canvas_rect.left() + 8.0;
    }

    let card_rect = Rect::from_min_size(
        Pos2::new(card_left, card_top),
        Vec2::new(card_w, card_h_est),
    );

    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(card_rect)
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
    );

    let mut response = BodyCardResponse {
        closed: false,
        action_clicked: None,
    };

    // Card frame: rounded only on the OPPOSITE end from the pill. When
    // extending down, the card's TOP corners are square (they merge with
    // the pill's bottom), the BOTTOM corners are rounded. When extending
    // up, vice versa.
    let corner_radius = if extend_down {
        egui::CornerRadius { nw: 0, ne: 0, sw: 8, se: 8 }
    } else {
        egui::CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }
    };

    egui::Frame::NONE
        .fill(theme.bg_card())
        .stroke(Stroke::new(1.8, theme.accent()))
        .corner_radius(corner_radius)
        .inner_margin(egui::Margin::same(10))
        .show(&mut child, |ui| {
            ui.set_min_width(card_w - 20.0);
            ui.set_max_width(card_w - 20.0);

            // Heading row — NO duplicate body dot. The pill above (with
            // the body as its left cap) already serves as the visual
            // anchor. Just show the name + Close button.
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&pp.name)
                        .size(theme.font_size_heading)
                        .color(theme.text_primary())
                        .strong(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("Close").clicked() {
                        response.closed = true;
                    }
                });
            });

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

    // Paint a seam-hiding overlay where the pill meets the card. Because
    // the pill has rounded bottom corners and the card has square top
    // corners (when extending down), there'd be a small "notch" at the
    // join where the pill's bottom-rounding cuts back inside. We hide it
    // by drawing a thin rectangle in the card's bg color across the seam.
    let seam_y = if extend_down {
        pp.rect.bottom()
    } else {
        pp.rect.top() - 1.0
    };
    let seam_left = card_rect.left().max(pp.rect.left()) + 1.0;
    let seam_right = card_rect.right().min(pp.rect.right()) - 1.0;
    if seam_right > seam_left {
        let seam_rect = Rect::from_min_size(
            Pos2::new(seam_left, seam_y - 1.0),
            Vec2::new(seam_right - seam_left, 3.0),
        );
        painter.rect_filled(seam_rect, 0.0, theme.bg_card());
    }

    // Use `_` to silence the unused-pp.color warning — color is exposed
    // on PlacedPill for callers that want to draw their own card variants
    // (e.g. loot-rarity-tinted cards) even though our default heading
    // doesn't render a dot anymore.
    let _ = pp.color;

    response
}
