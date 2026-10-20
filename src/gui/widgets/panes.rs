//! Panes beside each other or one above another, each a card, with a handle between each two that
//! a drag moves (2026-10-19). Built for the Media page's three panes (operator, 2026-10-19: "left
//! side is the list of all of them, middle is the fit to area picture, 3D model, sound/music
//! visualizer, then right side is data/details/info. We could do that top, middle bottom too.").
//!
//! WHICH WAY THEY GO follows the area's shape ([`arrange_for`]): side by side when the area is at
//! least as wide as it is tall and every pane fits at its narrowest, else one above another. The
//! caller draws each pane the same way whichever it is (one closure, called with the pane's
//! number), so the two arrangements can never show different things.
//!
//! THE SHARES are each pane's part of the area along the way they go, summing to one, and are the
//! caller's to keep (the Media page keeps them in the app's settings, so the split survives a
//! restart). A drag on a handle moves the boundary between its two panes, never past either
//! one's narrowest ([`drag`]); a double-click puts those two back to the default shares. Each pane
//! gets a `Ui` of its own, clipped to its card, so a scroll area in one scrolls in that one alone.
//!
//! Every colour and size is the theme's: the card colour and border, the radius, the spacing
//! between panes (which is where the handle is), and the accent on a handle being dragged.
//!
//! Web mirror: none yet. The website's Media page is one column (web/pages/media.html), and the
//! web is not in this increment's scope.

use egui::{CursorIcon, Id, Rect, Sense, Stroke, Ui, Vec2};

use crate::gui::theme::Theme;

/// Which way the panes go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrange {
    /// Side by side, left to right.
    Across,
    /// One above another, top to bottom.
    Down,
}

/// What a drag on the handles did this frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Moved {
    /// The shares changed (a handle was dragged or double-clicked).
    pub changed: bool,
    /// A drag ended this frame, or a double-click reset two panes: the time to keep the shares.
    pub settled: bool,
}

/// The way `count` panes go in an area of `area`, none narrower than `min` with `gap` between
/// each two: side by side when the area is at least as wide as it is tall and they all fit,
/// else one above another.
pub fn arrange_for(area: Vec2, count: usize, min: f32, gap: f32) -> Arrange {
    let n = count.max(1) as f32;
    let fits = area.x >= min * n + gap * (n - 1.0);
    if area.x >= area.y && fits {
        Arrange::Across
    } else {
        Arrange::Down
    }
}

/// `shares` made usable: one per default, each above zero and finite, summing to one; else the
/// defaults. A config written by hand, or by an older build with another number of panes, can
/// hold anything.
pub fn checked(shares: &[f32], defaults: &[f32]) -> Vec<f32> {
    let usable = shares.len() == defaults.len() && shares.iter().all(|s| s.is_finite() && *s > 0.01);
    let from = if usable { shares } else { defaults };
    let sum: f32 = from.iter().sum();
    if sum <= 0.0 {
        return vec![1.0 / defaults.len().max(1) as f32; defaults.len()];
    }
    from.iter().map(|s| s / sum).collect()
}

/// Each pane's length along the way they go, for `total` points shared out by `shares`, none
/// shorter than `min` (when they cannot all have `min`, they share `total` evenly). The lengths
/// sum to `total`.
pub fn lengths(total: f32, shares: &[f32], min: f32) -> Vec<f32> {
    let n = shares.len().max(1);
    if total <= min * n as f32 {
        return vec![total.max(0.0) / n as f32; shares.len()];
    }
    let sum: f32 = shares.iter().sum::<f32>().max(f32::EPSILON);
    let mut out: Vec<f32> = shares.iter().map(|s| s / sum * total).collect();
    // A pane below its narrowest takes what it lacks from the others, each giving in proportion
    // to what it has over its own narrowest.
    let short: f32 = out.iter().map(|l| (min - l).max(0.0)).sum();
    if short > 0.0 {
        let spare: f32 = out.iter().map(|l| (l - min).max(0.0)).sum();
        for l in out.iter_mut() {
            if *l < min {
                *l = min;
            } else if spare > 0.0 {
                *l -= short * (*l - min) / spare;
            }
        }
    }
    out
}

/// Move the boundary after pane `i` by `delta` points, in an area `total` points long: pane `i`
/// grows by it and pane `i + 1` shrinks by it, never either below `min`. The others keep theirs.
pub fn drag(shares: &mut [f32], i: usize, delta: f32, total: f32, min: f32) {
    if i + 1 >= shares.len() || total <= 0.0 {
        return;
    }
    let both = (shares[i] + shares[i + 1]) * total;
    if both < 2.0 * min {
        return;
    }
    let a = (shares[i] * total + delta).clamp(min, both - min);
    shares[i] = a / total;
    shares[i + 1] = (both - a) / total;
}

/// Put panes `i` and `i + 1` back to their default proportion, keeping their sum.
fn reset_pair(shares: &mut [f32], i: usize, defaults: &[f32]) {
    if i + 1 >= shares.len() || i + 1 >= defaults.len() {
        return;
    }
    let both = shares[i] + shares[i + 1];
    let want = defaults[i] + defaults[i + 1];
    if want <= 0.0 {
        return;
    }
    shares[i] = both * defaults[i] / want;
    shares[i + 1] = both * defaults[i + 1] / want;
}

/// Draw `shares.len()` panes in the space left in `ui`, the way `arrange` says, each a card. `add`
/// draws pane `i` into its own `Ui` and is told the card's rect (for anything painted around it).
/// The handles between them move the shares; what they did comes back. `id` keeps the handles'
/// and the panes' own ids steady from frame to frame.
pub fn show(
    ui: &mut Ui,
    theme: &Theme,
    id: Id,
    arrange: Arrange,
    shares: &mut [f32],
    defaults: &[f32],
    min: f32,
    mut add: impl FnMut(&mut Ui, usize, Rect),
) -> Moved {
    let mut moved = Moved::default();
    let n = shares.len();
    let area = ui.available_rect_before_wrap();
    if n == 0 || area.width() <= 0.0 || area.height() <= 0.0 {
        return moved;
    }
    let gap = theme.spacing_md;
    let along = |v: Vec2| if arrange == Arrange::Across { v.x } else { v.y };
    let total = (along(area.size()) - gap * (n as f32 - 1.0)).max(0.0);
    let sizes = lengths(total, shares, min);
    let mut start = if arrange == Arrange::Across { area.left() } else { area.top() };
    let radius = egui::CornerRadius::same(theme.border_radius as u8);
    for (i, size) in sizes.iter().enumerate() {
        let card = match arrange {
            Arrange::Across => Rect::from_min_size(egui::pos2(start, area.top()), egui::vec2(*size, area.height())),
            Arrange::Down => Rect::from_min_size(egui::pos2(area.left(), start), egui::vec2(area.width(), *size)),
        };
        ui.painter().rect(card, radius, theme.bg_card(), Stroke::new(1.0, theme.border()), egui::StrokeKind::Inside);
        let inner = card.shrink(theme.spacing_sm);
        let mut child = ui.new_child(egui::UiBuilder::new().id_salt(id.with(("pane", i))).max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
        child.set_clip_rect(inner.intersect(ui.clip_rect()));
        add(&mut child, i, card);
        start += size;
        if i + 1 < n {
            let handle = match arrange {
                Arrange::Across => Rect::from_min_size(egui::pos2(start, area.top()), egui::vec2(gap, area.height())),
                Arrange::Down => Rect::from_min_size(egui::pos2(area.left(), start), egui::vec2(area.width(), gap)),
            };
            let resp = ui.interact(handle, id.with(("handle", i)), Sense::click_and_drag());
            if resp.hovered() || resp.dragged() {
                ui.ctx().set_cursor_icon(if arrange == Arrange::Across { CursorIcon::ResizeHorizontal } else { CursorIcon::ResizeVertical });
            }
            if resp.dragged() {
                let d = resp.drag_delta();
                drag(shares, i, along(d), total, min);
                moved.changed = true;
            }
            if resp.drag_stopped() {
                moved.settled = true;
            }
            if resp.double_clicked() {
                reset_pair(shares, i, defaults);
                moved.changed = true;
                moved.settled = true;
            }
            // A short grip in the middle of the gap, the accent while it is held or pointed at.
            let active = resp.hovered() || resp.dragged();
            let color = if active { theme.accent() } else { theme.border() };
            let c = handle.center();
            let reach = theme.spacing_lg;
            let (a, b) = match arrange {
                Arrange::Across => (egui::pos2(c.x, c.y - reach), egui::pos2(c.x, c.y + reach)),
                Arrange::Down => (egui::pos2(c.x - reach, c.y), egui::pos2(c.x + reach, c.y)),
            };
            ui.painter().line_segment([a, b], Stroke::new(if active { 3.0 } else { 2.0 }, color));
            start += gap;
        }
    }
    ui.allocate_rect(area, Sense::hover());
    moved
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE WAY THE PANES GO FOLLOWS THE AREA'S SHAPE (operator, 2026-10-19: "We could do that top,
    /// middle bottom too."): his monitor's page (2560 by 1387) and the test screen's (1280 by 1000)
    /// put three panes side by side; a window taller than it is wide stacks them, and so does a
    /// wide one too narrow for three at their narrowest.
    ///
    /// Seen red 2026-10-19 with the shape test left out (`fits` alone): "a tall window stacks
    /// them" (left: Across, right: Down).
    #[test]
    fn the_way_the_panes_go_follows_the_areas_shape() {
        assert_eq!(arrange_for(egui::vec2(2560.0, 1300.0), 3, 280.0, 12.0), Arrange::Across, "his monitor");
        assert_eq!(arrange_for(egui::vec2(1280.0, 900.0), 3, 280.0, 12.0), Arrange::Across, "the test screen");
        assert_eq!(arrange_for(egui::vec2(900.0, 1400.0), 3, 280.0, 12.0), Arrange::Down, "a tall window stacks them");
        assert_eq!(arrange_for(egui::vec2(800.0, 600.0), 3, 280.0, 12.0), Arrange::Down, "too narrow for three side by side");
        assert_eq!(arrange_for(egui::vec2(1000.0, 1000.0), 3, 280.0, 12.0), Arrange::Across, "square, and they fit");
    }

    /// A DRAG MOVES ONE BOUNDARY AND NEVER SQUEEZES A PANE BELOW ITS NARROWEST; THE LENGTHS ALWAYS
    /// FILL THE AREA; A HAND-WRITTEN SPLIT IS CHECKED. Seen red 2026-10-19 with `drag` unclamped:
    /// "never below the narrowest: [0.9, -0.19999994, 0.3]" (the middle pane went below nothing).
    #[test]
    fn a_drag_moves_one_boundary_within_the_narrowest() {
        let mut s = vec![0.3, 0.4, 0.3];
        drag(&mut s, 0, 100.0, 1000.0, 200.0);
        assert!((s[0] - 0.4).abs() < 1e-5 && (s[1] - 0.3).abs() < 1e-5 && (s[2] - 0.3).abs() < 1e-5, "{s:?}");
        drag(&mut s, 0, 500.0, 1000.0, 200.0);
        assert!((s[1] * 1000.0 - 200.0).abs() < 1e-3, "never below the narrowest: {s:?}");
        let l = lengths(1000.0, &[0.05, 0.9, 0.05], 200.0);
        assert!(l.iter().all(|x| *x >= 199.9), "{l:?}");
        assert!((l.iter().sum::<f32>() - 1000.0).abs() < 1e-2, "they fill the area: {l:?}");
        assert_eq!(checked(&[f32::NAN, 1.0, 1.0], &[0.3, 0.4, 0.3]), vec![0.3, 0.4, 0.3], "a broken split is the default");
        let c = checked(&[2.0, 2.0, 4.0], &[0.3, 0.4, 0.3]);
        assert!((c[2] - 0.5).abs() < 1e-6, "a usable one is made to sum to one: {c:?}");
    }
}
