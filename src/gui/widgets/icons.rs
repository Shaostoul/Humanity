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
