//! Universal "celestial body pill + info card" widget.
//!
//! ## Why this is a universal widget
//!
//! The pill-to-card pattern was first built for the Cosmos page (v0.209),
//! but the operator noted (2026-05-12) that the same UI pattern applies to:
//!
//! 1. **Cosmos page** — egui Painter on a god's-eye 3D camera.
//!    Bodies are SolBody, screen positions come from `project_to_screen`.
//! 2. **In-ship Map Room HUD** — when the player stands in a designated
//!    navigation room, holographic body labels float around them. Same
//!    pill widget; screen positions come from the player's camera view.
//! 3. **AR-glasses sky overlay (Phase 4g)** — pills overlay real-world
//!    celestial objects in the player's view. The "canvas" is a camera
//!    passthrough; screen positions come from the AR headset's view +
//!    projection matrices.
//!
//! In all three cases, the compute is identical: project a body's world
//! position to a 2D screen position, render a pill at that anchor, hit-
//! test the pill for hover/click, optionally expand into an info card.
//! Only the source of `screen_pos` differs.
//!
//! This module owns the visual layout (collision-dodge, rounded-rect bg
//! with theme tokens, info-card auto-position with canvas clamping) so
//! callers just produce the input data and consume the response.

use egui::{Align2, Color32, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};
use crate::gui::theme::Theme;

/// One pill candidate for placement. Caller fills this in per-frame; the
/// widget runs collision-dodge and renders the survivors.
#[derive(Clone)]
pub struct BodyPill<'a> {
    /// Stable identifier — used for egui interaction id + click-result.
    pub id: &'a str,
    /// Display name shown on the pill.
    pub name: &'a str,
    /// Dot color (the body's render color).
    pub color: Color32,
    /// Screen-space center of the body the pill belongs to (used as the
    /// pill's anchor reference + as the start of the connector line).
    pub body_screen: Pos2,
    /// Apparent on-screen radius of the body in pixels — the pill is
    /// offset by this much so it doesn't overlap the body itself.
    pub body_radius_px: f32,
    /// Priority for collision-dodge — lower = drawn first / never hidden.
    /// Typical mapping: star=0, planets=1, dwarf planets=2, moons=3,
    /// asteroids=4. But the widget makes no assumptions — caller chooses.
    pub priority: u8,
    /// Whether the pill should always be drawn even if it collides with
    /// higher-priority placed rects. Typically true for hover/select/
    /// expanded states.
    pub forced: bool,
    /// Whether the pill should be styled as "expanded" (accent border).
    pub expanded: bool,
}

/// A placed pill — the result of running the collision-dodged layout.
pub struct PlacedPill {
    pub id: String,
    pub rect: Rect,
}

/// Result of running the pill-pass on a frame.
pub struct PillsLayout {
    pub placed: Vec<PlacedPill>,
    /// id of the pill clicked this frame, if any.
    pub clicked_id: Option<String>,
}

/// Lay out + draw a batch of pills with collision-dodge. The caller
/// inspects `clicked_id` to drive selection / expansion state.
///
/// `interact_id_salt` is a per-canvas string (e.g. "cosmos_pill") that
/// scopes egui interaction ids — so two surfaces (Cosmos page + Map Room)
/// can show pills for the same bodies without clobbering each other's
/// interaction state.
pub fn place_and_draw_pills(
    ui: &mut Ui,
    painter: &egui::Painter,
    theme: &Theme,
    pills: &[BodyPill<'_>],
    interact_id_salt: &str,
) -> PillsLayout {
    // Sort: forced pills first (so they always claim their slot), then by
    // priority. Stable sort keeps draw-order deterministic frame-to-frame.
    let mut sorted: Vec<&BodyPill<'_>> = pills.iter().collect();
    sorted.sort_by_key(|c| (!c.forced as u8, c.priority));

    let mut placed: Vec<PlacedPill> = Vec::with_capacity(sorted.len());
    let mut placed_rects: Vec<Rect> = Vec::with_capacity(sorted.len());
    let mut clicked_id: Option<String> = None;

    for c in &sorted {
        // Anchor pill above + right of the body, with a small gap. Keeps
        // the pill near its body but out of the body's own circle.
        let pill_anchor = c.body_screen
            + Vec2::new(c.body_radius_px + 4.0, -(c.body_radius_px + 16.0));

        // Pre-measure pill size for the collision check before painting.
        let font = egui::FontId::proportional(11.0);
        let text_galley = painter.layout_no_wrap(
            c.name.to_string(),
            font,
            theme.text_primary(),
        );
        let dot_r = 5.0_f32;
        let h_pad = 8.0_f32;
        let v_pad = 4.0_f32;
        let inner_gap = 6.0_f32;
        let pill_w = h_pad + dot_r * 2.0 + inner_gap + text_galley.size().x + h_pad;
        let pill_h = (dot_r * 2.0).max(text_galley.size().y) + v_pad * 2.0;
        let pill_rect = Rect::from_min_size(pill_anchor, egui::vec2(pill_w, pill_h));

        // Collision check — only forced pills override.
        if !c.forced && placed_rects.iter().any(|r| r.intersects(pill_rect)) {
            continue;
        }
        placed_rects.push(pill_rect);

        // Interact: clicking a pill returns its id. ui.interact gives us
        // a proper egui Response on an arbitrary screen-space rect.
        let pill_id = ui.id().with((interact_id_salt, c.id));
        let pill_response = ui.interact(pill_rect, pill_id, Sense::click());
        let pill_hovered = pill_response.hovered();

        // Draw the pill: rounded-rect background + thin border, dot on
        // left, name on right. Hover lightens the bg; expanded gets the
        // accent border. Theme tokens only — no hardcoded Color32::from_rgb.
        let bg = if pill_hovered { theme.bg_secondary() } else { theme.bg_card() };
        let border_stroke = if c.expanded {
            Stroke::new(1.5, theme.accent())
        } else if c.forced {
            Stroke::new(0.8, theme.border_focus())
        } else {
            Stroke::new(0.5, theme.border())
        };
        let radius = pill_h * 0.5;
        painter.rect_filled(pill_rect, radius, bg);
        painter.rect_stroke(pill_rect, radius, border_stroke, egui::StrokeKind::Outside);

        // Color dot (the body's render color).
        let dot_center = Pos2::new(
            pill_rect.left() + h_pad + dot_r,
            pill_rect.center().y,
        );
        painter.circle_filled(dot_center, dot_r, c.color);
        painter.circle_stroke(dot_center, dot_r, Stroke::new(0.5, theme.border()));

        // Name text — laid out from the dot's right edge.
        let text_pos = Pos2::new(
            dot_center.x + dot_r + inner_gap,
            pill_rect.center().y - text_galley.size().y * 0.5,
        );
        painter.galley(text_pos, text_galley, theme.text_primary());

        // Faint connector line from body edge to pill bottom-left if the
        // body is far enough below the pill that the relationship isn't
        // obvious.
        let connector_start = c.body_screen
            + Vec2::new(c.body_radius_px * 0.7, -c.body_radius_px * 0.7);
        let connector_end = Pos2::new(pill_rect.left() + 2.0, pill_rect.bottom() - 1.0);
        if (connector_end - connector_start).length() > 8.0 {
            painter.line_segment(
                [connector_start, connector_end],
                Stroke::new(0.5, theme.border()),
            );
        }

        if pill_response.clicked() {
            clicked_id = Some(c.id.to_string());
        }

        placed.push(PlacedPill { id: c.id.to_string(), rect: pill_rect });
    }

    PillsLayout { placed, clicked_id }
}

// ─────────────────────── Info card ──────────────────────────────────────────

/// Data describing one expandable body info card. The widget is purely
/// presentational — it knows nothing about SolBody, ECS components, or
/// AR-headset metadata. Callers translate their domain types into this
/// struct and the widget renders it consistently.
pub struct BodyCardData<'a> {
    /// Heading shown at the top of the card (typically body name).
    pub heading: &'a str,
    /// Dot color shown next to the heading.
    pub color: Color32,
    /// Subtitle line under the heading — typically "Type · Distance · Period".
    pub subtitle: Option<String>,
    /// Key/value rows for physical stats. Drawn in `text_muted`.
    pub stats: Vec<(String, String)>,
    /// Optional free-form description. Truncated to ~180 chars if longer.
    pub description: Option<&'a str>,
    /// Action buttons at the bottom — `(label, enabled)`. The widget
    /// returns the index of the clicked action via `BodyCardResponse`.
    pub actions: Vec<(String, bool)>,
}

/// Response from `draw_body_card`.
pub struct BodyCardResponse {
    /// True if the user clicked the Close button (×).
    pub closed: bool,
    /// Index into `BodyCardData::actions` if an action was clicked.
    pub action_clicked: Option<usize>,
}

/// Render a body info card. Anchors near the body's screen position, with
/// auto-flip-left / auto-clamp-to-canvas so the card never goes off-screen.
/// Returns a response describing which interactions fired this frame.
///
/// The `canvas_rect` is the bounding rect of whatever surface the card is
/// drawing onto — for the Cosmos page that's the painter's canvas rect;
/// for the Map Room HUD it's the FPS viewport; for AR glasses it's the
/// passthrough frame.
pub fn draw_body_card(
    ui: &mut Ui,
    painter: &egui::Painter,
    theme: &Theme,
    data: &BodyCardData<'_>,
    body_screen: Pos2,
    body_radius_px: f32,
    canvas_rect: Rect,
) -> BodyCardResponse {
    let card_max_w = 260.0_f32;
    let card_min_w = 200.0_f32;
    let estimate_h = 160.0_f32;

    // Anchor preferred to the right of the body, slightly below the pill.
    let preferred_x = body_screen.x + body_radius_px + 8.0;
    let preferred_y = body_screen.y + body_radius_px * 0.5;
    let mut anchor = Pos2::new(preferred_x, preferred_y);
    if anchor.x + card_max_w > canvas_rect.right() - 8.0 {
        anchor.x = body_screen.x - body_radius_px - 8.0 - card_max_w;
    }
    if anchor.x < canvas_rect.left() + 8.0 {
        anchor.x = canvas_rect.left() + 8.0;
    }
    if anchor.y + estimate_h > canvas_rect.bottom() - 8.0 {
        anchor.y = canvas_rect.bottom() - 8.0 - estimate_h;
    }
    if anchor.y < canvas_rect.top() + 8.0 {
        anchor.y = canvas_rect.top() + 8.0;
    }
    let card_rect = Rect::from_min_size(anchor, egui::vec2(card_max_w, estimate_h));

    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(card_rect)
            .layout(egui::Layout::top_down(egui::Align::LEFT)),
    );

    let mut response = BodyCardResponse {
        closed: false,
        action_clicked: None,
    };

    egui::Frame::NONE
        .fill(theme.bg_panel())
        .stroke(Stroke::new(1.5, theme.accent()))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(&mut child, |ui| {
            ui.set_min_width(card_min_w);
            ui.set_max_width(card_max_w - 20.0);

            // Heading row: dot + name + close button.
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(14.0, 14.0),
                    Sense::hover(),
                );
                let p = ui.painter();
                p.circle_filled(rect.center(), 6.0, data.color);
                p.circle_stroke(rect.center(), 6.0, Stroke::new(0.5, theme.border()));
                ui.label(
                    RichText::new(data.heading)
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

            if let Some(ref s) = data.subtitle {
                ui.label(
                    RichText::new(s)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
            }

            for (k, v) in &data.stats {
                ui.label(
                    RichText::new(format!("{}: {}", k, v))
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            }

            if let Some(desc) = data.description {
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

            if !data.actions.is_empty() {
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    for (i, (label, enabled)) in data.actions.iter().enumerate() {
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

    // Subtle connector from body to card top-left corner.
    let card_corner = Pos2::new(card_rect.left() + 4.0, card_rect.top() + 6.0);
    painter.line_segment(
        [body_screen, card_corner],
        Stroke::new(0.6, theme.accent()),
    );

    let _ = Align2::CENTER_TOP; // silence unused import
    response
}
