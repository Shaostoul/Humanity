//! The Font Size setting (Settings > Appearance): ONE scale for the whole desktop app (BUG-225).
//!
//! `settings.font_size` is a percent of the app's designed size, from 100 to 200 (WCAG 1.4.4,
//! "resize text", asks that text can reach 200% with nothing lost). It is egui's ZOOM FACTOR on
//! the main window's context, set by [`before_frame`] before every pass, so the words, and the
//! buttons, rows, gaps, icons and pictures around them, all grow together. Everything in the app
//! is measured in points; the zoom sets how many pixels a point is.
//!
//! WHY THE ZOOM FACTOR, NOT THE THEME'S TEXT SIZES (decided 2026-11-03):
//! - The theme's four text sizes reach only the text drawn with them. egui's own text styles
//!   (every plain `ui.label`), about 90 sizes written as numbers, and every height, gap and icon
//!   would stay where they are, so larger words would spill out of rows that did not grow, and
//!   the menus would not grow with the page: the first need the navigation design asks of this
//!   fix (docs/design/navigation.md, section 7).
//! - The theme is shared with the in-world screens and edited by Settings > Widgets > Fonts. A
//!   scaled theme would change every wall screen's picture, and could be saved back with the
//!   scale baked into the sizes.
//! - The zoom is how egui already scales for a high-density screen, so everything that turns
//!   points into pixels already reads `pixels_per_point`: the Maps page's world layer (rendered
//!   at the canvas's full pixel size), the model viewer, a file's picture, the dev IPC.
//!
//! WHAT IT REACHES:
//! - Every page, the top menu, the bottom strip, popups and the HUD: all drawn on the main
//!   context. A HUD marker is placed by projecting into `ctx.screen_rect()`, which is in points,
//!   so it stays on its target at any size.
//! - NOT the in-world screens. Each has a context of its own at one pixel per point
//!   (`screen_surface.rs`): a wall screen is a thing in the world that a person reads by walking
//!   up to it, its page is laid out for its own pixels, and in a shared world it should look the
//!   same to everyone standing in front of it. Its context's built-in keyboard zoom is off as
//!   well, so Ctrl and + typed into a focused screen do not grow it either.
//! - The dev IPC: its pointer verbs take window pixels and divide by the live
//!   `pixels_per_point`, and `find` multiplies back, so a coordinate read off a screenshot still
//!   lands. Rigs run at 100%: `scripts/rig-graphics.js` never copies `font_size` into a rig.
//! - The snapshots: `render_page_png` draws at the demo state's 100%, the size they always drew
//!   at; a snapshot asks for another size through `render_page_png_at`.
//!
//! THE KEYBOARD. Ctrl and + (or =) makes everything 10% larger, Ctrl and - 10% smaller, and Ctrl
//! and 0 puts it back to 100%, the way a web browser zooms. egui has these keys built in, but its
//! zoom is not saved and the setting would undo it on the next frame, so its own is turned off and
//! [`before_frame`] takes the keys out of the frame's input and changes the setting instead.
//!
//! DRAGGING THE SLIDER. The new size is applied when the slider is let go, not while it is
//! dragged: the slider's place under the pointer moves when the size changes, so a size applied
//! during the drag would feed back into the drag. While the button is held the value beside the
//! slider shows where it will land.

use egui::{Context, Event, Key, RawInput, RichText};

use crate::gui::theme::Theme;

/// The designed size: what every page was laid out at, and what a fresh install starts at.
pub const DEFAULT_PERCENT: f32 = 100.0;
/// The smallest size the setting offers.
pub const MIN_PERCENT: f32 = 100.0;
/// The largest size the setting offers (WCAG 1.4.4's 200%).
pub const MAX_PERCENT: f32 = 200.0;
/// How far one press of Ctrl and + or - moves the size.
pub const KEY_STEP_PERCENT: f32 = 10.0;

/// A size the app can use: a whole percent inside the range. Anything that is not a number
/// (a hand-edited config) is the designed size.
pub fn checked(percent: f32) -> f32 {
    if !percent.is_finite() {
        return DEFAULT_PERCENT;
    }
    percent.round().clamp(MIN_PERCENT, MAX_PERCENT)
}

/// egui's zoom factor for a size: 150% is 1.5.
pub fn zoom_for(percent: f32) -> f32 {
    checked(percent) / 100.0
}

/// What a press of one of the size keys does to the size, or `None` for any other event.
/// Ctrl, not Alt (Ctrl and Alt together is AltGr on many keyboards, which types characters);
/// Shift is allowed, because + is Shift and = on most keyboards.
fn key_step(event: &Event) -> Option<fn(f32) -> f32> {
    let Event::Key { key, pressed: true, modifiers, .. } = event else { return None };
    if !modifiers.command || modifiers.alt {
        return None;
    }
    match key {
        // To the next step up or down, so 137% goes to 140% or 130%, then on by tens.
        Key::Plus | Key::Equals => Some(|p| ((checked(p) / KEY_STEP_PERCENT).floor() + 1.0) * KEY_STEP_PERCENT),
        Key::Minus => Some(|p| ((checked(p) / KEY_STEP_PERCENT).ceil() - 1.0) * KEY_STEP_PERCENT),
        Key::Num0 => Some(|_| DEFAULT_PERCENT),
        _ => None,
    }
}

/// Make the context's scale the setting, before the pass measures anything. The frame loop
/// (lib.rs, just after `take_egui_input`) and the snapshot harness both call it before every
/// `ctx.run`, so a page is laid out at the person's size from its first widget on.
///
/// The size keys (Ctrl and +, -, 0) are taken out of `input` here and change `percent`; the
/// return value is true when they did, so the caller saves the setting. A size changed on the
/// Settings page is applied on the next frame, the way every setting is.
pub fn before_frame(ctx: &Context, input: &mut RawInput, percent: &mut f32) -> bool {
    // egui's own keyboard zoom would change the scale behind the setting's back.
    if ctx.options(|o| o.zoom_with_keyboard) {
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
    }
    let before = *percent;
    input.events.retain(|event| match key_step(event) {
        Some(step) => {
            *percent = checked(step(*percent));
            false
        }
        None => true,
    });
    // Takes effect at the start of this frame's pass (egui's `begin_pass`), and asks for a
    // repaint when it changes; a no-op when the scale is already right.
    ctx.set_zoom_factor(zoom_for(*percent));
    (*percent - before).abs() > f32::EPSILON
}

/// Settings > Appearance's Font Size row: its words, a slider from 100% to 200%, the value as a
/// percent, and the row's plain-language hint. Returns true when the setting changed, for the
/// caller to save. The size is kept back while the pointer is held (see the module notes).
pub fn setting_row(ui: &mut egui::Ui, theme: &Theme, percent: &mut f32) -> bool {
    use crate::gui::widgets::{self, with_hint};
    use humanity_core::i18n::t;
    let held_id = egui::Id::new("font_size_while_held");
    let mut shown: f32 = ui.data(|d| d.get_temp(held_id)).unwrap_or(*percent);
    with_hint(ui, "settings.font_size_desc", |ui| {
        widgets::settings_row(ui, theme, t("settings.font_size"), |ui| {
            // Room for the value so the slider never paints over it (as `labeled_slider` does).
            const VALUE_RESERVE: f32 = 56.0;
            widgets::custom_slider_capped(ui, theme, &mut shown, MIN_PERCENT..=MAX_PERCENT, VALUE_RESERVE);
            ui.label(RichText::new(format!("{:.0}%", shown)).color(theme.text_muted()).size(theme.font_size_small));
        });
    });
    let held = ui.input(|i| i.pointer.any_down());
    if held && (shown - *percent).abs() > f32::EPSILON {
        ui.data_mut(|d| d.insert_temp(held_id, shown));
        return false;
    }
    ui.data_mut(|d| d.remove::<f32>(held_id));
    let next = checked(shown);
    if (next - *percent).abs() > f32::EPSILON {
        *percent = next;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The operator's screen: a 65-inch TV at 2560 by 1440, Windows at 100% (one pixel per point
    /// before the setting). The window's size in points is what egui-winit hands egui: its pixels
    /// over the live scale.
    const WINDOW_PX: egui::Vec2 = egui::vec2(2560.0, 1440.0);

    fn input_for(ctx: &Context) -> RawInput {
        RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, WINDOW_PX / ctx.pixels_per_point())), ..Default::default() }
    }

    fn key(key: Key, ctrl: bool) -> Event {
        let modifiers = if ctrl { egui::Modifiers::COMMAND } else { egui::Modifiers::NONE };
        Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
    }

    /// One frame the way the app's loop runs it: the setting, then the pass. Returns the height
    /// in PIXELS of one line of body text, and the window's width in points.
    fn frame(ctx: &Context, percent: &mut f32, events: Vec<Event>) -> (f32, f32) {
        let mut input = input_for(ctx);
        input.events = events;
        before_frame(ctx, &mut input, percent);
        let mut line = 0.0;
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                line = ui.label("Font Size").rect.height();
            });
        });
        (line * ctx.pixels_per_point(), ctx.screen_rect().width())
    }

    /// THE GATE: the setting reaches the scale. At 150% and 200% the app's pixels per point are
    /// 1.5 and 2, a line of text is that many times taller on the screen, and the window holds
    /// that many times fewer points across (2560 pixels are 1280 points at 200%).
    ///
    /// Seen red 2026-11-03 with `before_frame`'s body taken out, which is the app before this fix
    /// (the setting was saved and loaded and read by nothing): "Font Size 150% drew at 1 pixels
    /// per point".
    #[test]
    fn the_setting_reaches_the_scale() {
        let mut base = DEFAULT_PERCENT;
        let ctx = Context::default();
        let (line_100, width_100) = (0..3).map(|_| frame(&ctx, &mut base, vec![])).last().unwrap();
        assert_eq!(width_100, 2560.0);
        for want in [150.0_f32, 200.0] {
            let ctx = Context::default();
            let mut percent = want;
            let (line, width) = (0..3).map(|_| frame(&ctx, &mut percent, vec![])).last().unwrap();
            let ppp = ctx.pixels_per_point();
            assert!((ppp - want / 100.0).abs() < 1e-4, "Font Size {want}% drew at {ppp} pixels per point");
            // Within a pixel: egui rounds a row to whole pixels at every scale.
            assert!((line - line_100 * want / 100.0).abs() <= 1.0, "a line of text is {line} px at {want}%, {line_100} px at 100%");
            assert!((width - 2560.0 * 100.0 / want).abs() < 0.5, "the window is {width} points across at {want}%");
        }
    }

    /// Ctrl and + / - / 0 change the SETTING (so the size is saved), in steps of ten, inside the
    /// range, and never reach the page. Ctrl and = counts as Ctrl and +; a plain - is typing.
    ///
    /// Seen red 2026-11-03 with `before_frame` doing nothing (the app before this fix): Ctrl and =
    /// gave "left: (false, 1), right: (true, 0)", the setting unchanged and the key left for the page.
    #[test]
    fn the_size_keys_step_the_setting_and_stay_out_of_the_page() {
        let ctx = Context::default();
        let step = |percent: &mut f32, k: Key, ctrl: bool| {
            let mut input = input_for(&ctx);
            input.events = vec![key(k, ctrl)];
            let changed = before_frame(&ctx, &mut input, percent);
            (changed, input.events.len())
        };
        let mut p = 100.0;
        assert_eq!(step(&mut p, Key::Equals, true), (true, 0));
        assert_eq!(p, 110.0, "Ctrl and = left the size at {p}");
        assert_eq!(step(&mut p, Key::Plus, true), (true, 0));
        assert_eq!(p, 120.0);
        let mut p = 137.0;
        step(&mut p, Key::Plus, true);
        assert_eq!(p, 140.0, "a step goes to the next ten");
        let mut p = 137.0;
        step(&mut p, Key::Minus, true);
        assert_eq!(p, 130.0);
        let mut p = 200.0;
        assert_eq!(step(&mut p, Key::Plus, true), (false, 0), "200% is the top: nothing to save");
        assert_eq!(p, 200.0);
        let mut p = 100.0;
        assert_eq!(step(&mut p, Key::Minus, true), (false, 0));
        assert_eq!(p, 100.0);
        let mut p = 170.0;
        assert_eq!(step(&mut p, Key::Num0, true), (true, 0));
        assert_eq!(p, 100.0);
        let mut p = 100.0;
        assert_eq!(step(&mut p, Key::Minus, false), (false, 1), "a plain minus is left for the page");
        assert!(!ctx.options(|o| o.zoom_with_keyboard), "egui's own keyboard zoom is off");
    }

    #[test]
    fn a_size_from_a_hand_edited_config_is_made_usable() {
        assert_eq!(checked(14.0), 100.0, "the old point sizes (10 to 24) are below the range");
        assert_eq!(checked(250.0), 200.0);
        assert_eq!(checked(149.6), 150.0);
        assert_eq!(checked(f32::NAN), DEFAULT_PERCENT);
        assert_eq!(zoom_for(175.0), 1.75);
    }

    /// The top menu at a size, in a window of `px` pixels: the height its panels take in points,
    /// the window's width in points, how many lines its words sit on (text tops more than half a
    /// row apart), and the right edge of its rightmost text.
    fn top_row_at(px: egui::Vec2, percent: f32) -> (f32, f32, usize, f32) {
        let theme = crate::gui::theme::load_theme();
        let ctx = Context::default();
        crate::gui::install_fonts(&ctx);
        theme.apply_to_egui(&ctx);
        let mut state = crate::gui::GuiState::default();
        state.settings.font_size = percent;
        state.active_page = crate::gui::GuiPage::Settings;
        let mut height = 0.0;
        let mut out = None;
        for _ in 0..3 {
            let mut input = RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, px / ctx.pixels_per_point())), ..Default::default() };
            before_frame(&ctx, &mut input, &mut state.settings.font_size);
            out = Some(ctx.run(input, |ctx| {
                crate::gui::pages::escape_menu::draw_nav_bar(ctx, &theme, &mut state);
                height = ctx.available_rect().top();
            }));
        }
        let mut tops: Vec<f32> = Vec::new();
        let mut right: f32 = 0.0;
        for clipped in &out.expect("ran").shapes {
            if let egui::Shape::Text(text) = &clipped.shape {
                let r = text.visual_bounding_rect();
                if r.top() < height && !text.galley.text().trim().is_empty() {
                    right = right.max(r.right());
                    if tops.iter().all(|t| (t - r.top()).abs() > 12.0) {
                        tops.push(r.top());
                    }
                }
            }
        }
        (height, ctx.screen_rect().width(), tops.len(), right)
    }

    /// THE TOP MENU AT EACH SIZE (navigation.md section 7, the second need). Printed with
    /// `--nocapture` for the record in BUG-225. Until navigation rung 2 the menu was one wrapping
    /// row, and this test held that day's counts as a ratchet (2026-11-03: one row up to 150% on
    /// the operator's screen and two at 200%; in 1280 by 720, two rows at 100% and three at 150%
    /// and 200%, 29% of the window's height). Since rung 2 (the same day) it is two rows that never
    /// wrap: what does not fit goes under More (pages/escape_menu.rs), so at every size the words
    /// sit on exactly two lines, the primary row's and the secondary row's, and nothing is cut off
    /// at the window's right edge. (Which buttons More takes at each size: escape_menu_tests.rs,
    /// `more_takes_what_does_not_fit_never_a_third_row`.) Seen red 2026-11-03 with More switched
    /// off (every button laid out whatever the room): "at 150% in 1280x720 px a word of the top row
    /// ends at 866.25, past the window's 853.34375 points".
    #[test]
    fn font_size_top_row_rows_at_each_size() {
        for (px, percent, most_rows) in [
            (WINDOW_PX, 100.0, 2),
            (WINDOW_PX, 150.0, 2),
            (WINDOW_PX, 200.0, 2),
            (egui::vec2(1280.0, 720.0), 100.0, 2),
            (egui::vec2(1280.0, 720.0), 150.0, 2),
            (egui::vec2(1280.0, 720.0), 200.0, 2),
        ] {
            let (height, width, rows, right) = top_row_at(px, percent);
            let share = height * percent / 100.0 / px.y;
            println!("top row at {percent}% in {}x{} px ({width:.0} points across): {height:.0} points tall, words on {rows} row(s), rightmost word ends at {right:.0}, {:.1}% of the window's height", px.x, px.y, share * 100.0);
            assert!(rows >= 1, "no words found in the top row at {percent}%");
            assert!(right <= width, "at {percent}% in {}x{} px a word of the top row ends at {right}, past the window's {width} points", px.x, px.y);
            assert_eq!(rows, most_rows, "at {percent}% in {}x{} px the top menu's words sit on {rows} lines (its two rows, never a third)", px.x, px.y);
        }
    }

    /// The frame loop calls `before_frame` between taking the window's input and running the
    /// pass: that call is the whole of the fix, and its absence was the whole of the bug.
    ///
    /// Seen red 2026-11-03 with the call in lib.rs replaced by `false`: "lib.rs does not set the
    /// Font Size before the main pass".
    #[test]
    fn the_frame_loop_sets_the_size_before_the_main_pass() {
        let lib = std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("src/lib.rs")).expect("read src/lib.rs");
        let take = lib.find("state.egui_state.take_egui_input(&state.window)").expect("the main window's input");
        let run = lib[take..].find("state.egui_ctx.run(raw_input").map(|i| take + i).expect("the main pass");
        let set = lib[take..run].find("crate::gui::font_size::before_frame(&state.egui_ctx, &mut raw_input, &mut state.gui_state.settings.font_size)");
        assert!(set.is_some(), "lib.rs does not set the Font Size before the main pass");
    }
}
