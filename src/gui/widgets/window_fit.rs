//! Every window fits the visible area (2026-11-23, the phone; android/README.md, "What the code
//! does differently on a phone").
//!
//! WHY. The operator, 2026-11-23, on his OnePlus 9 Pro (412 x 915 points) with v0.1571.0: "I can't
//! upload images because the windows aren't scaling to the phone." His screenshot: the Attach
//! window ("Attach files: choose one or more") running off both edges of the screen. The pages
//! already lay out only where nothing of Android's covers them (gui/visible_area.rs, 2026-11-22);
//! the windows over them did not: each had a width and height made for a desktop.
//!
//! WHAT. On a phone, or on any screen narrower or shorter than [`SMALL`] points ([`small`]), every
//! window, whatever size it asks for:
//! - is no WIDER than the visible area. egui holds a window's BODY inside the rectangle the window
//!   is constrained to, not the frame around it (egui 0.31, containers/window.rs, "Prevent window
//!   from becoming larger than the constrain rect": the body's largest width is the rectangle's
//!   width, the frame's margins and border come on top), so [`constrain_rect`] takes the window's own
//!   frame off each side, measured as it was last drawn, and every window is constrained to that
//!   (`layers::window`, the one place every window is made). Its body's text wraps to the width
//!   left, and a part that cannot wrap (a row of buttons wider than a phone) scrolls sideways inside
//!   it.
//! - is no TALLER than the visible area. Its body is a scroll area ([`Fitted::show_fitted`]), as tall
//!   as what is in it when that fits and at most the visible area less the window's title bar and
//!   frame, so the end of a long window (its Send or Cancel) is reached by scrolling inside it.
//!   egui is told the body may be any height (`min_height` and `max_height` unbounded, egui then
//!   gives it the most the rectangle allows), so a window grows back when the keyboard goes down
//!   (egui's own window scroll keeps the smaller height it once had, gui/visible_area.rs).
//! - stays CENTRED in the visible area when it is anchored at the centre: the rectangle it is
//!   constrained to is the visible area less the same width at each side.
//! The visible area is egui's screen rectangle (on a phone already the part above the keyboard and
//! inside Android's bars) less the unfinished-software warning along the bottom
//! (`bottom_strip::room_above`, where the app's dialogs stand).
//!
//! A window on a screen at least [`SMALL`] points wide and tall (a desktop) is drawn exactly as
//! before: nothing here runs there.
//!
//! EVERY WINDOW. Each is made by `layers::window` (`tests/layer_lint.rs` refuses egui's own) and
//! shown with [`Fitted::show_fitted`] (a lint in window_fit_tests.rs refuses a plain `.show`), so a
//! new window cannot skip the fit.
//!
//! COST: one scroll area per open window and two numbers kept per window in egui's memory for the
//! session; a pass is laid out again only on a frame where a window's size changed (egui's
//! `request_discard`), so nothing waits under a window's edge for a next frame that a phone,
//! drawing only on change, might not draw.

use egui::{Context, Id, InnerResponse, Rect, Ui, Vec2};

/// A screen narrower or shorter than this, in points, has its windows fitted (a phone is about
/// 412 wide; the smallest desktop window the app is used in is wider and taller).
pub const SMALL: f32 = 600.0;

/// A little room left between a window and the sides of the visible area, so a window reads as
/// one over the page rather than as the page.
const GUTTER: f32 = 4.0;

/// The most a window's frame is believed to take (both sides together; top and bottom with the
/// title bar), so an odd measurement can never squeeze a body to nothing.
const FRAME_MAX: Vec2 = Vec2::new(96.0, 240.0);

/// The least height a window's body is given, however little is visible.
const BODY_MIN: f32 = 48.0;

/// Whether windows are fitted this frame: on a phone (or its preview on a desktop), or on any
/// screen narrower or shorter than [`SMALL`].
pub fn small(ctx: &Context) -> bool {
    let screen = ctx.screen_rect();
    humanity_core::platform::device::phone() || screen.width() < SMALL || screen.height() < SMALL
}

/// The room windows have: the visible area above the unfinished-software warning.
pub fn room(ctx: &Context) -> Rect {
    crate::gui::bottom_strip::room_above(ctx)
}

/// What a window's frame took the last time [`Fitted::show_fitted`] drew it (its whole size less
/// its body's: the margins and border at both sides, and top and bottom the title bar too), and
/// its whole size then.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Measured {
    frame: Vec2,
    size: Vec2,
}

fn measured_id(window: Id) -> Id {
    window.with("window_fit_measured")
}

/// The frame of the window `window`, as last measured; before it was ever drawn, egui's window
/// margin and border at both sides (and a control's height for its title bar).
fn frame_of(ctx: &Context, window: Id) -> Vec2 {
    if let Some(m) = ctx.data(|d| d.get_temp::<Measured>(measured_id(window))) {
        return m.frame;
    }
    let style = ctx.style();
    let stroke = 2.0 * style.visuals.window_stroke.width;
    let margin = style.spacing.window_margin.sum();
    Vec2::new(margin.x + stroke, margin.y + stroke + style.spacing.interact_size.y)
}

/// The rectangle the window `window` is constrained to inside `room` on a small screen: the room
/// less the window's frame and a [`GUTTER`] at each side (the module notes say why the frame comes
/// off). On a larger screen, `room` itself.
pub fn constrain_rect(ctx: &Context, window: Id, room: Rect) -> Rect {
    if !small(ctx) {
        return room;
    }
    let side = frame_of(ctx, window).x / 2.0 + GUTTER;
    let side = side.min((room.width() / 2.0 - BODY_MIN).max(0.0));
    room.shrink2(Vec2::new(side, 0.0))
}

/// A window shown inside the visible area (the module notes).
pub trait Fitted {
    /// `show`, with the window's body a scroll area that keeps the whole window inside the visible
    /// area on a small screen; on a larger screen exactly egui's `show`.
    fn show_fitted<R>(self, ctx: &Context, add: impl FnOnce(&mut Ui) -> R) -> Option<InnerResponse<Option<R>>>;
}

impl Fitted for egui::Window<'_> {
    fn show_fitted<R>(self, ctx: &Context, add: impl FnOnce(&mut Ui) -> R) -> Option<InnerResponse<Option<R>>> {
        if !small(ctx) {
            return self.show(ctx, add);
        }
        let room = room(ctx);
        let mut body = Rect::NOTHING;
        let mut window = None;
        let out = self.min_height(f32::INFINITY).max_height(f32::INFINITY).show(ctx, |ui| {
            let id = ui.layer_id().id;
            window = Some(id);
            let cap = (room.height() - frame_of(ui.ctx(), id).y).max(BODY_MIN).min(ui.available_height().max(BODY_MIN));
            // Both ways: what wraps is laid out to the width left and only scrolls up and down; a
            // part that cannot wrap (a row of buttons wider than a phone) scrolls sideways inside the
            // window rather than widening it past the screen (egui's scroll area follows its
            // contents' width in a direction it does not scroll).
            let inner = egui::ScrollArea::both()
                .id_salt("window_fit_body")
                .auto_shrink([true, true])
                .max_width(ui.available_width())
                .max_height(cap)
                .show(ui, add)
                .inner;
            body = ui.min_rect();
            inner
        });
        if let (Some(out), Some(id)) = (out.as_ref(), window) {
            if body.is_positive() {
                let size = out.response.rect.size();
                let now = Measured { frame: (size - body.size()).max(Vec2::ZERO).min(FRAME_MAX), size };
                let before = ctx.data_mut(|d| {
                    let before = d.get_temp::<Measured>(measured_id(id));
                    d.insert_temp(measured_id(id), now);
                    before
                });
                let changed = before.is_none_or(|b| (b.frame - now.frame).length() > 0.5 || (b.size - now.size).length() > 0.5);
                if changed {
                    ctx.request_discard("a window changed size to fit the visible area");
                }
            }
        }
        out
    }
}

#[cfg(test)]
#[path = "window_fit_tests.rs"]
mod tests;
