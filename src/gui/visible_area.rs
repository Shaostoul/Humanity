//! The part of the window a person can see (2026-11-22, the phone; android/README.md, "What the
//! code does differently on a phone").
//!
//! WHY. On a phone the app draws its pages into the whole window, and Android lays other things over
//! it: the status bar and any camera cutout at the top, the navigation bar with its back, home and
//! recent-apps buttons at the bottom (at the side when the phone is turned), and the on-screen
//! keyboard while a text box is being typed in. The operator, 2026-11-22, restoring his identity on
//! his phone: "when I'm entering my 24 word recovery phrase to join on my phone I can't scroll low
//! enough when the keyboard opens to see the text box preventing me from spelling things
//! correctly." And on the Chat page: "The text box is low enough that it intersects with the
//! buttons to go back, home, and menu on my phone. Like I can't click the attach button cause of
//! the center button that takes me back to my phones homepage."
//!
//! WHAT. Before each frame the window's covered edges are measured ([`platform_covered`]) and egui
//! is given only the area inside them ([`before_frame`]): its screen rectangle shrinks, so every
//! page, panel, scroll area and window lays itself out where it can be seen, the chat's text box and
//! the Music page's player bar sit above the phone's buttons, and a scroll area ends above the
//! keyboard. A focused text box is scrolled into view when it gains focus or when the keyboard
//! opens, closes or changes height while it has focus ([`keep_in_view`]).
//!
//! HOW ANDROID SAYS WHAT IS COVERED. Two sources, and the app takes the smaller area:
//! - **The window's insets** (`android/java/.../VisibleArea.java`, read through JNI): Android's own
//!   account of how far the system bars, a display cutout and the keyboard reach in from each edge.
//!   android-activity 0.6.1's NativeActivity backend offers no insets of its own (its
//!   `InsetsChanged` event comes only from the GameActivity backend), so the app's Java listens on
//!   the window's root view.
//! - **The NativeActivity's content rectangle** (android-activity's `AndroidApp::content_rect`,
//!   through winit's `WindowExtAndroid::content_rect`): the area of the activity's own content view,
//!   which shrinks for the bars and the keyboard (the manifest's `adjustResize`) while Android fits
//!   the window's content inside them.
//! The content rectangle alone was not enough: the operator's phone drew the chat's text box under
//! the navigation bar, so his window reaches under the system bars, and a window laid out edge to
//! edge (Android 15 does that for every app targeting it, as this one does) keeps a content
//! rectangle the size of the whole window, the keyboard included. The insets are right in both
//! layouts. winit 0.30 drops the `ContentRectChanged` event with a warning, so the app reads both
//! itself, at each frame and at the drawing-on-change loop's tick (engine/redraw.rs), and draws a
//! frame when they change.
//!
//! A desktop has nothing covering its window: [`platform_covered`] is zero there and nothing here
//! changes how a desktop lays out or scrolls.

use egui::{Context, Id, Rect, RawInput};

/// How far in from each edge of the window something covers it, in the window's PIXELS (what
/// Android reports).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CoveredPx {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// How far in from each edge of egui's screen something covers it, in POINTS (what egui lays out
/// in).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Covered {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Covered {
    /// Nothing covered (a desktop, a phone before Android reported anything).
    pub fn is_zero(&self) -> bool {
        self.left <= 0.0 && self.top <= 0.0 && self.right <= 0.0 && self.bottom <= 0.0
    }
}

/// The covered edges of a `window` (its size in pixels) from the window's `insets` (left, top,
/// right, bottom, how far in each reaches) and the activity's `content` rectangle (left, top,
/// right, bottom, in the window's pixels), either of which may be missing: the area a person can
/// see is inside both. A report that would leave less than a quarter of the width or 15% of the
/// height (a keyboard turned sideways leaves more than that) is not believed: nothing is covered
/// then, so a bad report can never shrink the app to nothing.
pub fn covered_px(window: (u32, u32), insets: Option<[i32; 4]>, content: Option<[i32; 4]>) -> CoveredPx {
    let (w, h) = (window.0 as i32, window.1 as i32);
    if w <= 0 || h <= 0 {
        return CoveredPx::default();
    }
    let (mut l, mut t, mut r, mut b) = (0, 0, w, h);
    if let Some([il, it, ir, ib]) = insets {
        l = l.max(il);
        t = t.max(it);
        r = r.min(w - ir.max(0));
        b = b.min(h - ib.max(0));
    }
    // An empty rectangle (all zero) is the content rectangle before Android first set it.
    if let Some([cl, ct, cr, cb]) = content.filter(|[cl, ct, cr, cb]| cr > cl && cb > ct) {
        l = l.max(cl);
        t = t.max(ct);
        r = r.min(cr);
        b = b.min(cb);
    }
    if r - l < w / 4 || (b - t) * 100 < h * 15 {
        return CoveredPx::default();
    }
    CoveredPx { left: l, top: t, right: w - r, bottom: h - b }
}

/// `covered` (pixels of a `window` that size) as points of egui's `screen` rectangle: the same
/// share of each side, so the Font Size's zoom and the phone's own scale need no care here.
pub fn in_points(covered: CoveredPx, window: (u32, u32), screen: Rect) -> Covered {
    let (w, h) = (window.0.max(1) as f32, window.1.max(1) as f32);
    Covered {
        left: covered.left.max(0) as f32 / w * screen.width(),
        top: covered.top.max(0) as f32 / h * screen.height(),
        right: covered.right.max(0) as f32 / w * screen.width(),
        bottom: covered.bottom.max(0) as f32 / h * screen.height(),
    }
}

/// The frame loop's call (engine/frame_ui_run.rs, after the Font Size and before the pass):
/// the window's `covered` pixels (from [`platform_covered`]) taken off the input's screen rectangle.
pub fn before_frame(ctx: &Context, input: &mut RawInput, covered: CoveredPx, window: (u32, u32)) {
    let edges = input.screen_rect.map_or(Covered::default(), |screen| in_points(covered, window, screen));
    apply(ctx, input, edges);
}

/// Take `edges` (points) off the input's screen rectangle, and remember them for this frame
/// ([`covered`], [`keep_in_view`]). The tests and the snapshots call this with a stand-in
/// keyboard or navigation bar; zero edges leave the input as it is.
pub fn apply(ctx: &Context, input: &mut RawInput, edges: Covered) {
    if let Some(screen) = input.screen_rect.as_mut() {
        if !edges.is_zero() {
            *screen = Rect::from_min_max(
                egui::pos2(screen.min.x + edges.left.max(0.0), screen.min.y + edges.top.max(0.0)),
                egui::pos2(screen.max.x - edges.right.max(0.0), screen.max.y - edges.bottom.max(0.0)),
            );
        }
    }
    ctx.data_mut(|d| d.insert_temp(edges_id(), edges));
}

/// What this frame's screen rectangle leaves out at each edge (zero on a desktop).
pub fn covered(ctx: &Context) -> Covered {
    ctx.data(|d| d.get_temp::<Covered>(edges_id())).unwrap_or_default()
}

fn edges_id() -> Id {
    Id::new("visible_area_covered")
}

/// Keep a focused text box where it can be seen: when it gains focus, or when the visible area
/// changes while it has focus (the keyboard opening, closing or changing height, the phone
/// turning), the scroll area it is in scrolls so the box and a line of context under it show. For
/// a box taller than most of the visible area (the recovery phrase, typed on a phone), the line
/// being typed and the line under it. Call it right after the box is drawn, with its `response`.
///
/// Only on a phone or while something covers the window: a desktop scrolls exactly as before.
/// (egui's own text box already scrolls to its cursor while the person types; what it never does is
/// scroll when the keyboard comes up over a box that was visible when it was tapped.)
pub fn keep_in_view(ui: &egui::Ui, response: &egui::Response) {
    let ctx = ui.ctx();
    if covered(ctx).is_zero() && !humanity_core::platform::device::phone() {
        return;
    }
    let seen_id = response.id.with("visible_area_seen");
    let screen = ctx.screen_rect();
    let seen = ctx.data(|d| d.get_temp::<Rect>(seen_id));
    if !response.has_focus() {
        if seen.is_some() {
            ctx.data_mut(|d| d.remove::<Rect>(seen_id));
        }
        return;
    }
    ctx.data_mut(|d| d.insert_temp(seen_id, screen));
    if !(response.gained_focus() || seen != Some(screen)) {
        return;
    }
    ui.scroll_to_rect(view_target(ui, response.rect, screen.height()), None);
}

/// The rectangle [`keep_in_view`] scrolls to: the box and a line under it, or, for a box too tall
/// for that, the line being typed (egui's keyboard rectangle for the focused box, the cursor) and a
/// line under it.
fn view_target(ui: &egui::Ui, box_rect: Rect, visible_h: f32) -> Rect {
    let line = ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y;
    let mut target = box_rect;
    if box_rect.height() + line > visible_h * 0.8 {
        // The focused text box tells egui where its cursor is, for the keyboard (in this frame's
        // output, set while the box was drawn).
        if let Some(ime) = ui.ctx().output(|o| o.ime) {
            if box_rect.expand(1.0).contains_rect(ime.cursor_rect) {
                target = ime.cursor_rect;
            }
        }
    }
    target.max.y += line;
    target
}

/// The height a window's scrolling body gets this frame: last frame's content height, at most
/// `max_h` (the visible area less the window's own margins). With [`scrolling_body`] a window is as
/// tall as its contents, scrolls when the visible area is shorter (a phone's keyboard came up) and
/// grows back when it is taller again. (egui's own window scroll keeps the smaller height it once
/// had and never grows back, and a scroll area inside a window that sizes itself to its contents can
/// never grow past its first size: both seen 2026-11-22.) Give the window `fixed_size` of this.
pub fn body_height(ctx: &Context, id: Id, max_h: f32) -> f32 {
    let last = ctx.data(|d| d.get_temp::<f32>(id.with("content_h"))).unwrap_or(max_h);
    last.min(max_h).max(1.0)
}

/// A window's body: a vertical scroll area exactly `height` high (from [`body_height`] with the same
/// `max_h`), noting its contents' height for the next frame. When the contents changed height (24
/// words shown, an error line added) the pass is laid out again at once (egui's `request_discard`),
/// so nothing waits under the window's edge for a next frame that, on a phone drawing only on
/// change, might not come (seen 2026-11-22: the shown recovery words clipped for a frame).
pub fn scrolling_body<R>(ui: &mut egui::Ui, id: Id, height: f32, max_h: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let out = egui::ScrollArea::vertical()
        .id_salt(id.with("body"))
        .max_height(height)
        .min_scrolled_height(height)
        .auto_shrink([false, false])
        .show(ui, add);
    let content_h = out.content_size.y;
    ui.ctx().data_mut(|d| d.insert_temp(id.with("content_h"), content_h));
    if (content_h.min(max_h).max(1.0) - height).abs() > 0.5 {
        ui.ctx().request_discard("a window's body changed height");
    }
    out.inner
}

/// What covers the window now, in its pixels: on a phone the system bars, a display cutout and the
/// on-screen keyboard (the module notes); nothing on a desktop. Logged when it changes, for a
/// phone's log (`adb logcat -s HumanityOS`).
#[cfg(target_os = "android")]
pub fn platform_covered(window: &winit::window::Window) -> CoveredPx {
    use winit::platform::android::WindowExtAndroid;
    let size = window.inner_size();
    let insets = humanity_core::platform::android_jni::visible_area_insets().ok().flatten();
    let c = window.content_rect();
    let content = [c.left, c.top, c.right, c.bottom];
    let covered = covered_px((size.width, size.height), insets, Some(content));
    if let Ok(mut last) = LAST.lock() {
        if *last != Some(covered) {
            log::info!(
                "[VisibleArea] covered px left {} top {} right {} bottom {} (window {}x{}, insets {:?}, content rect {:?})",
                covered.left, covered.top, covered.right, covered.bottom, size.width, size.height, insets, content
            );
            *last = Some(covered);
        }
    }
    covered
}

/// What covers the window now: nothing, off a phone.
#[cfg(not(target_os = "android"))]
pub fn platform_covered(_window: &winit::window::Window) -> CoveredPx {
    CoveredPx::default()
}

/// Whether what covers the window changed since the last frame measured it (the keyboard came up
/// while the loop slept): the drawing-on-change loop asks at its tick and draws a frame when so
/// (engine/redraw.rs). Always false off a phone.
#[cfg(target_os = "android")]
pub fn platform_changed(window: &winit::window::Window) -> bool {
    use winit::platform::android::WindowExtAndroid;
    let size = window.inner_size();
    let insets = humanity_core::platform::android_jni::visible_area_insets().ok().flatten();
    let c = window.content_rect();
    let now = covered_px((size.width, size.height), insets, Some([c.left, c.top, c.right, c.bottom]));
    LAST.lock().map(|last| *last != Some(now)).unwrap_or(false)
}

/// Whether what covers the window changed: never, off a phone.
#[cfg(not(target_os = "android"))]
pub fn platform_changed(_window: &winit::window::Window) -> bool {
    false
}

/// What the last frame measured (Android).
#[cfg(target_os = "android")]
static LAST: std::sync::Mutex<Option<CoveredPx>> = std::sync::Mutex::new(None);

#[cfg(test)]
#[path = "visible_area_tests.rs"]
mod tests;
