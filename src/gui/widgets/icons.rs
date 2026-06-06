//! Painter-drawn icons for egui.
//!
//! egui's default font has no emoji or icon glyphs. These functions draw
//! simple geometric icons using the painter API so they render on every
//! platform without custom fonts.

use egui::{Color32, Pos2, Rect, Stroke, Vec2};

/// Draw a gear/cog icon inside `rect`.
pub fn paint_cog(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.5;
    let inner_r = r * 0.55;
    let tooth_r = r * 0.95;
    let tooth_w = 0.38; // radians half-width of each tooth

    // Draw 6 teeth as short thick line segments radiating outward
    let teeth = 6;
    for i in 0..teeth {
        let angle = (i as f32) * std::f32::consts::TAU / (teeth as f32);
        let cos = angle.cos();
        let sin = angle.sin();
        let from = Pos2::new(c.x + cos * inner_r, c.y + sin * inner_r);
        let to = Pos2::new(c.x + cos * tooth_r, c.y + sin * tooth_r);
        painter.line_segment([from, to], Stroke::new(r * 0.35, color));

        // Small angled lines for tooth edges
        let a1 = angle + tooth_w;
        let a2 = angle - tooth_w;
        let tip1 = Pos2::new(c.x + a1.cos() * tooth_r, c.y + a1.sin() * tooth_r);
        let tip2 = Pos2::new(c.x + a2.cos() * tooth_r, c.y + a2.sin() * tooth_r);
        painter.line_segment([tip1, tip2], Stroke::new(1.0, color));
    }

    // Outer ring
    painter.circle_stroke(c, inner_r * 1.1, Stroke::new(r * 0.18, color));
    // Inner hole
    painter.circle_filled(c, inner_r * 0.4, color);
    painter.circle_filled(c, inner_r * 0.2, Color32::from_rgb(30, 30, 36));
}

/// Draw a microphone icon inside `rect`.
pub fn paint_mic(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.12).max(1.0), color);

    // Mic head (rounded capsule shape)
    let head_w = w * 0.28;
    let head_h = w * 0.42;
    let head_top = c.y - w * 0.32;
    let head_rect = Rect::from_min_size(
        Pos2::new(c.x - head_w, head_top),
        Vec2::new(head_w * 2.0, head_h),
    );
    painter.rect_stroke(head_rect, head_w, stroke, egui::StrokeKind::Outside);

    // Curved cradle below head
    let cradle_y = head_top + head_h;
    let cradle_r = w * 0.35;
    // Approximate half-circle with line segments
    let segments = 8;
    for i in 0..segments {
        let a1 = std::f32::consts::PI * (i as f32) / (segments as f32);
        let a2 = std::f32::consts::PI * ((i + 1) as f32) / (segments as f32);
        let p1 = Pos2::new(c.x - a1.cos() * cradle_r, cradle_y - w * 0.06 + a1.sin() * cradle_r);
        let p2 = Pos2::new(c.x - a2.cos() * cradle_r, cradle_y - w * 0.06 + a2.sin() * cradle_r);
        painter.line_segment([p1, p2], stroke);
    }

    // Stem
    let stem_top = cradle_y + cradle_r - w * 0.06;
    let stem_bot = c.y + w * 0.42;
    painter.line_segment(
        [Pos2::new(c.x, stem_top), Pos2::new(c.x, stem_bot)],
        stroke,
    );
    // Base
    painter.line_segment(
        [Pos2::new(c.x - w * 0.2, stem_bot), Pos2::new(c.x + w * 0.2, stem_bot)],
        stroke,
    );
}

/// Draw a right-pointing triangle (collapsed state).
pub fn paint_triangle_right(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height()) * 0.35;
    let points = vec![
        Pos2::new(c.x - s * 0.5, c.y - s),
        Pos2::new(c.x + s, c.y),
        Pos2::new(c.x - s * 0.5, c.y + s),
    ];
    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
}

/// Draw a down-pointing triangle (expanded state).
pub fn paint_triangle_down(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height()) * 0.35;
    let points = vec![
        Pos2::new(c.x - s, c.y - s * 0.5),
        Pos2::new(c.x + s, c.y - s * 0.5),
        Pos2::new(c.x, c.y + s),
    ];
    painter.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
}

/// Draw a lock (locked) icon inside `rect`.
pub fn paint_lock(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.1).max(1.0), color);

    // Lock body (rectangle)
    let body_w = w * 0.5;
    let body_h = w * 0.35;
    let body_top = c.y;
    let body_rect = Rect::from_min_size(
        Pos2::new(c.x - body_w * 0.5, body_top),
        Vec2::new(body_w, body_h),
    );
    painter.rect_filled(body_rect, 2.0, color);

    // Shackle (arc above body)
    let shackle_r = body_w * 0.35;
    let shackle_cy = body_top;
    let segments = 8;
    for i in 0..segments {
        let a1 = std::f32::consts::PI + std::f32::consts::PI * (i as f32) / (segments as f32);
        let a2 = std::f32::consts::PI + std::f32::consts::PI * ((i + 1) as f32) / (segments as f32);
        let p1 = Pos2::new(c.x + a1.cos() * shackle_r, shackle_cy + a1.sin() * shackle_r);
        let p2 = Pos2::new(c.x + a2.cos() * shackle_r, shackle_cy + a2.sin() * shackle_r);
        painter.line_segment([p1, p2], Stroke::new(stroke.width * 1.5, color));
    }
}

/// Draw an unlock icon inside `rect`.
pub fn paint_unlock(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.1).max(1.0), color);

    // Lock body
    let body_w = w * 0.5;
    let body_h = w * 0.35;
    let body_top = c.y;
    let body_rect = Rect::from_min_size(
        Pos2::new(c.x - body_w * 0.5, body_top),
        Vec2::new(body_w, body_h),
    );
    painter.rect_stroke(body_rect, 2.0, stroke, egui::StrokeKind::Outside);

    // Open shackle (arc shifted left, not closing on right side)
    let shackle_r = body_w * 0.35;
    let shackle_cy = body_top;
    let segments = 6;
    for i in 0..segments {
        let a1 = std::f32::consts::PI + std::f32::consts::PI * (i as f32) / (segments as f32 + 2.0);
        let a2 = std::f32::consts::PI + std::f32::consts::PI * ((i + 1) as f32) / (segments as f32 + 2.0);
        let offset_x = -w * 0.08;
        let p1 = Pos2::new(c.x + offset_x + a1.cos() * shackle_r, shackle_cy + a1.sin() * shackle_r);
        let p2 = Pos2::new(c.x + offset_x + a2.cos() * shackle_r, shackle_cy + a2.sin() * shackle_r);
        painter.line_segment([p1, p2], Stroke::new(stroke.width * 1.5, color));
    }
}

/// Draw a speaker/audio icon inside `rect` (sound waves emanating right).
pub fn paint_speaker(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.1).max(1.0), color);

    // Speaker cone (trapezoid)
    let left = c.x - w * 0.3;
    let mid = c.x - w * 0.05;
    let top_narrow = c.y - w * 0.12;
    let bot_narrow = c.y + w * 0.12;
    let top_wide = c.y - w * 0.3;
    let bot_wide = c.y + w * 0.3;

    let cone = vec![
        Pos2::new(left, top_narrow),
        Pos2::new(mid, top_wide),
        Pos2::new(mid, bot_wide),
        Pos2::new(left, bot_narrow),
    ];
    painter.add(egui::Shape::convex_polygon(cone, color, Stroke::NONE));

    // Sound waves (arcs to the right)
    let wave_cx = mid + w * 0.05;
    for wave in 1..=2 {
        let r = w * 0.15 * (wave as f32);
        let segments = 6;
        for i in 0..segments {
            let a1 = -std::f32::consts::FRAC_PI_4 + std::f32::consts::FRAC_PI_2 * (i as f32) / (segments as f32);
            let a2 = -std::f32::consts::FRAC_PI_4 + std::f32::consts::FRAC_PI_2 * ((i + 1) as f32) / (segments as f32);
            let p1 = Pos2::new(wave_cx + a1.cos() * r, c.y + a1.sin() * r);
            let p2 = Pos2::new(wave_cx + a2.cos() * r, c.y + a2.sin() * r);
            painter.line_segment([p1, p2], stroke);
        }
    }
}

/// Draw a plus icon inside `rect`.
pub fn paint_plus(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height()) * 0.35;
    let stroke = Stroke::new((s * 0.3).max(1.5), color);
    painter.line_segment([Pos2::new(c.x - s, c.y), Pos2::new(c.x + s, c.y)], stroke);
    painter.line_segment([Pos2::new(c.x, c.y - s), Pos2::new(c.x, c.y + s)], stroke);
}

/// Draw a right-arrow icon inside `rect` (for "join").
pub fn paint_arrow_right(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height()) * 0.35;
    let stroke = Stroke::new((s * 0.3).max(1.5), color);
    // Shaft
    painter.line_segment([Pos2::new(c.x - s, c.y), Pos2::new(c.x + s, c.y)], stroke);
    // Arrowhead
    painter.line_segment([Pos2::new(c.x + s * 0.3, c.y - s * 0.6), Pos2::new(c.x + s, c.y)], stroke);
    painter.line_segment([Pos2::new(c.x + s * 0.3, c.y + s * 0.6), Pos2::new(c.x + s, c.y)], stroke);
}

/// Draw a person/user icon inside `rect` (head circle + body arc).
pub fn paint_person(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());

    // Head (circle)
    let head_r = w * 0.2;
    let head_cy = c.y - w * 0.15;
    painter.circle_filled(Pos2::new(c.x, head_cy), head_r, color);

    // Body (half-ellipse / arc below head)
    let body_top = head_cy + head_r + w * 0.05;
    let body_w = w * 0.35;
    let body_h = w * 0.25;
    let segments = 8;
    for i in 0..segments {
        let a1 = std::f32::consts::PI * (i as f32) / (segments as f32);
        let a2 = std::f32::consts::PI * ((i + 1) as f32) / (segments as f32);
        let p1 = Pos2::new(c.x + a1.cos() * body_w, body_top + a1.sin() * body_h);
        let p2 = Pos2::new(c.x + a2.cos() * body_w, body_top + a2.sin() * body_h);
        painter.line_segment([p1, p2], Stroke::new((w * 0.12).max(1.5), color));
    }
    // Close the bottom with a flat line
    painter.line_segment(
        [Pos2::new(c.x - body_w, body_top), Pos2::new(c.x + body_w, body_top)],
        Stroke::new((w * 0.12).max(1.5), color),
    );
}

/// Allocate a small square for an icon and return (rect, response).
/// Use the rect to paint an icon, and the response for click/hover detection.
pub fn icon_button(ui: &mut egui::Ui, size: f32) -> (Rect, egui::Response) {
    ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click())
}

// ─────────────────────────── Nav item icons (v0.196.0) ───────────────────────
// Operator 2026-05-08: every main-menu nav button should have an icon
// alongside its label. egui's bundled font has spotty icon coverage
// (icon_glyph_lint enforces this), so we paint simple geometric shapes
// directly. The shapes are intentionally minimal — they need to be
// distinguishable at 12-14 px and theme-color-able. Crisp at any DPI
// because they're vector primitives going straight to the GPU.
//
// `paint_nav_icon` is the central router — match the page enum to a
// small painter call. Adding a new nav target? Add one match arm + one
// paint helper and you're done.

/// Speech bubble — Chat.
pub fn paint_chat(painter: &egui::Painter, rect: Rect, color: Color32) {
    let r = rect.width().min(rect.height()) * 0.4;
    let c = rect.center();
    let stroke = Stroke::new((r * 0.18).max(1.0), color);
    // Rounded rect body
    let body = Rect::from_center_size(Pos2::new(c.x, c.y - r * 0.1), Vec2::new(r * 1.7, r * 1.2));
    painter.rect_stroke(body, egui::Rounding::same(((r * 0.35) as u8).max(2)), stroke, egui::StrokeKind::Inside);
    // Tail (small triangle pointing down-left)
    let tail_top = Pos2::new(c.x - r * 0.4, body.bottom() - 0.5);
    let tail_bot = Pos2::new(c.x - r * 0.7, body.bottom() + r * 0.45);
    let tail_right = Pos2::new(c.x - r * 0.1, body.bottom() - 0.5);
    painter.line_segment([tail_top, tail_bot], stroke);
    painter.line_segment([tail_bot, tail_right], stroke);
}

/// Wallet — rectangle with a small coin slot.
pub fn paint_wallet(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.1).max(1.0), color);
    let body = Rect::from_center_size(c, Vec2::new(w * 0.85, w * 0.6));
    painter.rect_stroke(body, egui::Rounding::same(2), stroke, egui::StrokeKind::Inside);
    // Coin slot circle on the right
    painter.circle_filled(Pos2::new(body.right() - w * 0.12, c.y), w * 0.06, color);
}

/// Heart — Donate.
pub fn paint_heart(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height()) * 0.4;
    // Two top circles for the lobes
    painter.circle_filled(Pos2::new(c.x - w * 0.45, c.y - w * 0.15), w * 0.45, color);
    painter.circle_filled(Pos2::new(c.x + w * 0.45, c.y - w * 0.15), w * 0.45, color);
    // Triangle for the bottom
    use egui::epaint::PathShape;
    let pts = vec![
        Pos2::new(c.x - w * 0.85, c.y),
        Pos2::new(c.x + w * 0.85, c.y),
        Pos2::new(c.x, c.y + w * 0.85),
    ];
    painter.add(PathShape::convex_polygon(pts, color, Stroke::NONE));
}

/// Key — Identity.
pub fn paint_key(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.1).max(1.0), color);
    let r = w * 0.18;
    // Bow (circle on the left)
    painter.circle_stroke(Pos2::new(c.x - w * 0.25, c.y), r, stroke);
    // Shaft
    painter.line_segment([Pos2::new(c.x - w * 0.25 + r, c.y), Pos2::new(c.x + w * 0.4, c.y)], stroke);
    // Teeth (two short downward strokes)
    painter.line_segment([Pos2::new(c.x + w * 0.2, c.y), Pos2::new(c.x + w * 0.2, c.y + w * 0.15)], stroke);
    painter.line_segment([Pos2::new(c.x + w * 0.35, c.y), Pos2::new(c.x + w * 0.35, c.y + w * 0.15)], stroke);
}

/// Scroll / document — Governance.
pub fn paint_scroll(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.08).max(1.0), color);
    let body = Rect::from_center_size(c, Vec2::new(w * 0.6, w * 0.8));
    painter.rect_stroke(body, egui::Rounding::same(1), stroke, egui::StrokeKind::Inside);
    // Three text lines inside
    for i in 0..3 {
        let y = body.top() + body.height() * (0.3 + 0.2 * i as f32);
        painter.line_segment([Pos2::new(body.left() + w * 0.08, y), Pos2::new(body.right() - w * 0.08, y)], stroke);
    }
}

/// Lifebuoy — Recovery.
pub fn paint_lifebuoy(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.4;
    let stroke = Stroke::new((r * 0.18).max(1.0), color);
    painter.circle_stroke(c, r, stroke);
    painter.circle_stroke(c, r * 0.45, stroke);
    // Four cross marks at compass points
    for i in 0..4 {
        let a = i as f32 * std::f32::consts::FRAC_PI_2;
        let from = Pos2::new(c.x + a.cos() * r * 0.45, c.y + a.sin() * r * 0.45);
        let to = Pos2::new(c.x + a.cos() * r, c.y + a.sin() * r);
        painter.line_segment([from, to], stroke);
    }
}

/// Checklist — Tasks.
pub fn paint_checklist(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.1).max(1.0), color);
    // Three rows of (small box + line)
    for i in 0..3 {
        let y = c.y + (i as f32 - 1.0) * w * 0.25;
        let box_left = c.x - w * 0.3;
        let box_size = w * 0.15;
        let bx = Rect::from_min_size(Pos2::new(box_left, y - box_size * 0.5), Vec2::splat(box_size));
        painter.rect_stroke(bx, egui::Rounding::same(1), stroke, egui::StrokeKind::Inside);
        // Line to the right of the box
        painter.line_segment(
            [Pos2::new(box_left + box_size + w * 0.05, y), Pos2::new(c.x + w * 0.35, y)],
            stroke,
        );
    }
}

/// Open box — Inventory.
pub fn paint_box(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.1).max(1.0), color);
    let body = Rect::from_center_size(c, Vec2::new(w * 0.7, w * 0.6));
    painter.rect_stroke(body, egui::Rounding::same(1), stroke, egui::StrokeKind::Inside);
    // Lid line across the top
    painter.line_segment(
        [Pos2::new(body.left(), body.top() + body.height() * 0.3), Pos2::new(body.right(), body.top() + body.height() * 0.3)],
        stroke,
    );
}

/// Crown — marks a P2P group the current user created/owns (vs joined).
/// Built from a filled band rect + three convex triangle spikes so the fill
/// is correct (a single concave crown polygon wouldn't tessellate cleanly).
pub fn paint_crown(painter: &egui::Painter, rect: Rect, color: Color32) {
    use egui::epaint::PathShape;
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let s = w * 0.42;
    let band_top = c.y + s * 0.10;
    let base_y = c.y + s * 0.62;
    // Jeweled band (bottom rectangle).
    painter.rect_filled(
        Rect::from_min_max(Pos2::new(c.x - s, band_top), Pos2::new(c.x + s, base_y)),
        egui::Rounding::same(0),
        color,
    );
    // Three spikes rising from the band (each a convex triangle).
    let mut spike = |a: Pos2, b: Pos2, apex: Pos2| {
        painter.add(PathShape::convex_polygon(vec![a, b, apex], color, Stroke::NONE));
    };
    spike(
        Pos2::new(c.x - s, band_top),
        Pos2::new(c.x - s * 0.34, band_top),
        Pos2::new(c.x - s * 0.66, c.y - s * 0.62),
    );
    spike(
        Pos2::new(c.x - s * 0.34, band_top),
        Pos2::new(c.x + s * 0.34, band_top),
        Pos2::new(c.x, c.y - s * 0.80),
    );
    spike(
        Pos2::new(c.x + s * 0.34, band_top),
        Pos2::new(c.x + s, band_top),
        Pos2::new(c.x + s * 0.66, c.y - s * 0.62),
    );
}

/// Map pin — Maps.
pub fn paint_pin(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let r = w * 0.25;
    // Drop shape: circle on top, triangle below
    painter.circle_filled(Pos2::new(c.x, c.y - w * 0.1), r, color);
    use egui::epaint::PathShape;
    let pts = vec![
        Pos2::new(c.x - r * 0.7, c.y),
        Pos2::new(c.x + r * 0.7, c.y),
        Pos2::new(c.x, c.y + w * 0.4),
    ];
    painter.add(PathShape::convex_polygon(pts, color, Stroke::NONE));
    // Inner hole
    painter.circle_filled(Pos2::new(c.x, c.y - w * 0.1), r * 0.4, Color32::from_rgb(20, 20, 26));
}

/// Shopping bag — Market.
pub fn paint_bag(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.1).max(1.0), color);
    let body = Rect::from_center_size(Pos2::new(c.x, c.y + w * 0.05), Vec2::new(w * 0.6, w * 0.55));
    painter.rect_stroke(body, egui::Rounding::same(1), stroke, egui::StrokeKind::Inside);
    // Handle (arc above the body)
    let handle_y = body.top() - w * 0.05;
    let handle_w = body.width() * 0.5;
    painter.line_segment([Pos2::new(c.x - handle_w * 0.5, handle_y), Pos2::new(c.x - handle_w * 0.5, body.top())], stroke);
    painter.line_segment([Pos2::new(c.x + handle_w * 0.5, handle_y), Pos2::new(c.x + handle_w * 0.5, body.top())], stroke);
    painter.line_segment([Pos2::new(c.x - handle_w * 0.5, handle_y), Pos2::new(c.x + handle_w * 0.5, handle_y)], stroke);
}

/// Hammer — Crafting.
pub fn paint_hammer(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.12).max(1.0), color);
    // Head: horizontal rectangle on top
    let head = Rect::from_center_size(Pos2::new(c.x, c.y - w * 0.2), Vec2::new(w * 0.55, w * 0.2));
    painter.rect_filled(head, egui::Rounding::same(1), color);
    // Handle: diagonal line going down-right
    painter.line_segment(
        [Pos2::new(c.x + w * 0.05, c.y - w * 0.1), Pos2::new(c.x + w * 0.3, c.y + w * 0.35)],
        stroke,
    );
}

/// Building / city — Civilization.
pub fn paint_building(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.08).max(1.0), color);
    // Three vertical bars of varying height (skyline)
    let bar_w = w * 0.18;
    let positions = [-0.3, 0.0, 0.3];
    let heights = [0.5, 0.7, 0.55];
    for (i, &dx) in positions.iter().enumerate() {
        let h = w * heights[i];
        let bar = Rect::from_min_size(
            Pos2::new(c.x + dx * w - bar_w * 0.5, c.y + w * 0.35 - h),
            Vec2::new(bar_w, h),
        );
        painter.rect_stroke(bar, egui::Rounding::same(1), stroke, egui::StrokeKind::Inside);
    }
}

/// Palette — Studio.
pub fn paint_palette(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.08).max(1.0), color);
    // Oval body
    painter.circle_stroke(c, w * 0.4, stroke);
    // Three paint dots inside
    painter.circle_filled(Pos2::new(c.x - w * 0.18, c.y - w * 0.05), w * 0.06, color);
    painter.circle_filled(Pos2::new(c.x + w * 0.05, c.y - w * 0.18), w * 0.06, color);
    painter.circle_filled(Pos2::new(c.x + w * 0.18, c.y + w * 0.05), w * 0.06, color);
}

/// Compass — Onboarding.
pub fn paint_compass(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.4;
    let stroke = Stroke::new((r * 0.18).max(1.0), color);
    painter.circle_stroke(c, r, stroke);
    // Arrow pointing up-right (compass needle)
    use egui::epaint::PathShape;
    let pts = vec![
        Pos2::new(c.x - r * 0.3, c.y + r * 0.3),
        Pos2::new(c.x + r * 0.5, c.y - r * 0.5),
        Pos2::new(c.x + r * 0.15, c.y - r * 0.05),
    ];
    painter.add(PathShape::convex_polygon(pts, color, Stroke::NONE));
}

/// Robot head — Agents.
pub fn paint_robot(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.1).max(1.0), color);
    let body = Rect::from_center_size(c, Vec2::new(w * 0.6, w * 0.55));
    painter.rect_stroke(body, egui::Rounding::same(2), stroke, egui::StrokeKind::Inside);
    // Two eye dots
    painter.circle_filled(Pos2::new(c.x - w * 0.12, c.y - w * 0.05), w * 0.05, color);
    painter.circle_filled(Pos2::new(c.x + w * 0.12, c.y - w * 0.05), w * 0.05, color);
    // Antenna
    painter.line_segment([Pos2::new(c.x, body.top()), Pos2::new(c.x, body.top() - w * 0.15)], stroke);
    painter.circle_filled(Pos2::new(c.x, body.top() - w * 0.18), w * 0.04, color);
}

/// Bar chart — AI Usage.
pub fn paint_chart(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.1).max(1.0), color);
    // Three bars of increasing height
    let bar_w = w * 0.15;
    let xs = [-0.25, 0.0, 0.25];
    let hs = [0.3, 0.5, 0.7];
    for (i, &dx) in xs.iter().enumerate() {
        let h = w * hs[i];
        let bar = Rect::from_min_size(
            Pos2::new(c.x + dx * w - bar_w * 0.5, c.y + w * 0.35 - h),
            Vec2::new(bar_w, h),
        );
        painter.rect_filled(bar, egui::Rounding::same(1), color);
    }
    // Baseline
    painter.line_segment(
        [Pos2::new(c.x - w * 0.4, c.y + w * 0.36), Pos2::new(c.x + w * 0.4, c.y + w * 0.36)],
        stroke,
    );
}

/// Wrench — Tools.
pub fn paint_wrench(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.13).max(1.0), color);
    // Diagonal handle
    painter.line_segment(
        [Pos2::new(c.x - w * 0.3, c.y + w * 0.3), Pos2::new(c.x + w * 0.2, c.y - w * 0.2)],
        stroke,
    );
    // Open jaw at the top
    painter.circle_stroke(Pos2::new(c.x + w * 0.25, c.y - w * 0.25), w * 0.13, stroke);
}

/// Bug — BugReport.
pub fn paint_bug(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.1).max(1.0), color);
    // Body oval
    painter.circle_filled(c, w * 0.25, color);
    // Two antennae
    painter.line_segment([Pos2::new(c.x - w * 0.12, c.y - w * 0.18), Pos2::new(c.x - w * 0.25, c.y - w * 0.35)], stroke);
    painter.line_segment([Pos2::new(c.x + w * 0.12, c.y - w * 0.18), Pos2::new(c.x + w * 0.25, c.y - w * 0.35)], stroke);
    // Three legs each side
    for i in 0..3 {
        let y = c.y - w * 0.1 + i as f32 * w * 0.12;
        painter.line_segment([Pos2::new(c.x - w * 0.2, y), Pos2::new(c.x - w * 0.4, y + w * 0.05)], stroke);
        painter.line_segment([Pos2::new(c.x + w * 0.2, y), Pos2::new(c.x + w * 0.4, y + w * 0.05)], stroke);
    }
}

/// Clipboard — Testing.
pub fn paint_clipboard(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.08).max(1.0), color);
    let body = Rect::from_center_size(c, Vec2::new(w * 0.55, w * 0.7));
    painter.rect_stroke(body, egui::Rounding::same(1), stroke, egui::StrokeKind::Inside);
    // Tab at the top
    let tab = Rect::from_center_size(Pos2::new(c.x, body.top() - w * 0.02), Vec2::new(w * 0.25, w * 0.1));
    painter.rect_filled(tab, egui::Rounding::same(1), color);
    // Lines inside
    for i in 0..3 {
        let y = body.top() + body.height() * (0.35 + 0.2 * i as f32);
        painter.line_segment([Pos2::new(body.left() + w * 0.08, y), Pos2::new(body.right() - w * 0.08, y)], stroke);
    }
}

/// Globe — Browser.
pub fn paint_globe(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.4;
    let stroke = Stroke::new((r * 0.16).max(1.0), color);
    painter.circle_stroke(c, r, stroke);
    // Vertical meridian
    painter.line_segment([Pos2::new(c.x, c.y - r), Pos2::new(c.x, c.y + r)], stroke);
    // Equator
    painter.line_segment([Pos2::new(c.x - r, c.y), Pos2::new(c.x + r, c.y)], stroke);
    // Two latitude curves (approximate as horizontal ellipse arcs)
    painter.line_segment([Pos2::new(c.x - r * 0.85, c.y - r * 0.45), Pos2::new(c.x + r * 0.85, c.y - r * 0.45)], stroke);
    painter.line_segment([Pos2::new(c.x - r * 0.85, c.y + r * 0.45), Pos2::new(c.x + r * 0.85, c.y + r * 0.45)], stroke);
}

/// Router: paint the icon for a given GUI page. Returns true if an icon
/// was painted, false if the page has no icon assigned (caller can fall
/// back to a generic placeholder or skip the icon).
///
/// Add new nav items here — one match arm + one paint helper above and
/// the nav bar picks it up automatically.
pub fn paint_nav_icon(painter: &egui::Painter, rect: Rect, page: crate::gui::GuiPage, color: Color32) -> bool {
    use crate::gui::GuiPage as P;
    match page {
        P::Chat        => { paint_chat(painter, rect, color); true }
        P::Wallet      => { paint_wallet(painter, rect, color); true }
        P::Donate      => { paint_heart(painter, rect, color); true }
        P::Profile     => { paint_person(painter, rect, color); true }
        P::Identity    => { paint_key(painter, rect, color); true }
        P::Governance  => { paint_scroll(painter, rect, color); true }
        P::Recovery    => { paint_lifebuoy(painter, rect, color); true }
        P::Tasks       => { paint_checklist(painter, rect, color); true }
        P::Inventory   => { paint_box(painter, rect, color); true }
        P::Maps        => { paint_pin(painter, rect, color); true }
        P::Market      => { paint_bag(painter, rect, color); true }
        P::Crafting    => { paint_hammer(painter, rect, color); true }
        P::Civilization => { paint_building(painter, rect, color); true }
        P::Studio      => { paint_palette(painter, rect, color); true }
        P::Onboarding  => { paint_compass(painter, rect, color); true }
        // v0.197.0: Agents + AiUsage variants removed. paint_robot and
        // paint_chart are kept (other future pages might use them).
        P::Cosmos      => { paint_globe(painter, rect, color); true }
        P::Settings    => { paint_cog(painter, rect, color); true }
        P::Tools       => { paint_wrench(painter, rect, color); true }
        P::BugReport   => { paint_bug(painter, rect, color); true }
        P::Testing     => { paint_clipboard(painter, rect, color); true }
        P::Browser     => { paint_globe(painter, rect, color); true }
        // Folded carve tabs (v0.363). Humanity's icon IS the brand "H" — the H
        // is the icon, "Humanity" is the label, so the old separate brand button
        // is retired (operator: "only have a single H/Humanity page").
        P::Humanity => {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "H",
                egui::FontId::proportional(rect.height()),
                color,
            );
            true
        }
        P::Real     => { paint_person(painter, rect, color); true }
        P::Play     => { paint_hammer(painter, rect, color); true }
        P::Platform => { paint_wrench(painter, rect, color); true }
        P::Library  => { paint_scroll(painter, rect, color); true }
        _ => false,
    }
}

/// Eye — "read-only / view-only" channel marker (operator-requested
/// 2026-05-15: "Read only could be a simple eye design"). An almond
/// outline (two arcs approximated by a stroked ellipse) with a filled
/// pupil. Reads as "you can look but not write here."
pub fn paint_eye(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.09).max(1.0), color);
    // Almond shape — a stroked polyline forming a lens (two curved lids).
    use egui::epaint::PathShape;
    let hw = w * 0.42; // half-width of the eye
    let hh = w * 0.26; // lid bulge
    let n = 14;
    // Upper lid (left → right, bulging up) then lower lid (right → left,
    // bulging down): one closed almond outline.
    let mut pts: Vec<Pos2> = Vec::with_capacity(n * 2);
    for i in 0..=n {
        let t = i as f32 / n as f32; // 0..1
        let x = c.x - hw + 2.0 * hw * t;
        let y = c.y - hh * (std::f32::consts::PI * t).sin();
        pts.push(Pos2::new(x, y));
    }
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let x = c.x + hw - 2.0 * hw * t;
        let y = c.y + hh * (std::f32::consts::PI * t).sin();
        pts.push(Pos2::new(x, y));
    }
    painter.add(PathShape::closed_line(pts, stroke));
    // Pupil.
    painter.circle_filled(c, w * 0.13, color);
}

/// Federation — "this channel/server gossips to peer servers." A small
/// node-graph: a central hub with three satellite nodes connected by
/// lines. Reads as "networked / spreads across the mesh." Operator was
/// unsure on a federated icon (2026-05-15) — this is the proposed one;
/// node-graph is the clearest "federated/mesh" metaphor and stays
/// visually distinct from the eye.
pub fn paint_federation(painter: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let w = rect.width().min(rect.height());
    let stroke = Stroke::new((w * 0.07).max(1.0), color);
    let hub_r = w * 0.13;
    let node_r = w * 0.10;
    let orbit = w * 0.34;
    // Three satellites at 90°, 210°, 330° (one up, two lower) so it
    // doesn't read as a perfect triangle / play button.
    let sats = [
        Pos2::new(c.x, c.y - orbit),
        Pos2::new(c.x - orbit * 0.87, c.y + orbit * 0.5),
        Pos2::new(c.x + orbit * 0.87, c.y + orbit * 0.5),
    ];
    for s in &sats {
        painter.line_segment([c, *s], stroke);
    }
    painter.circle_filled(c, hub_r, color);
    for s in &sats {
        painter.circle_filled(*s, node_r, color);
    }
}
