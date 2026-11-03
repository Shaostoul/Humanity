//! The top menu: TWO ROWS since navigation rung 2 (2026-11-03; docs/design/navigation.md,
//! sections 3, 4.1, 7 and 14).
//!
//! The PRIMARY ROW is the design's eight categories, each a page of its own (Humanity, Play, Chat,
//! the own-life category, Apps, Library, Dev, Settings), with Back and Forward at its left end and
//! the Aa switch, the notices bell and help at its right. The SECONDARY ROW under it lists the
//! category's other pages: the category of the page on screen, or, while the pointer rests on
//! another category's button (`timings.preview_rest_ms`, 200 ms), that category's, a PREVIEW in
//! the same row and at the same height, so nothing on the page moves. The pointer can travel down
//! into the row and click one; the preview stays while the pointer is on either row and goes
//! `timings.preview_leave_ms` (300 ms) after it leaves both, or at once on Escape ([`end_preview`],
//! from lib.rs). Settings > Appearance turns the preview off (`settings.nav_preview`).
//!
//! A row that does not fit across the window shows what fits in order and puts the rest under
//! MORE at its end (the "priority plus" pattern), never a third row.
//!
//! PLAY (the operator's answer of 2026-11-03, section 16.1): a tap opens Play's page (pages/play.rs)
//! and its row; a press and hold goes straight into the world, the button filling as it is held
//! (`widgets::hold_step`, the clock every hold in the app uses; `timings.play_hold_ms`, 800 ms).
//!
//! WHAT IS DATA: every button, its order, words, hint, icon, colour token and the page it opens
//! is data/gui/navigation.json (gui/navigation.rs reads it), the same file the website's menu is
//! drawn from. The sizes are theme tokens (`nav_primary_height` and its siblings); the colours are
//! one token per category (`nav_cat_<id>`). A button is added, moved or renamed in the file, never
//! here. This file draws what the file says and does what a click asks.
//!
//! BACK AND FORWARD are drawn here and wired in rung 4 (the app's history). Until then Back walks
//! the steps Escape walks (`GuiState::nav_back_stack`: a page opened from inside another), and
//! Forward is drawn dimmed, saying it is not built yet.
//!
//! COST (section 13's budget): two rows of at most nine buttons, laid out on frames the app draws
//! anyway; the menu is read once (no file read per frame); the hover state is a few numbers in
//! egui's memory. NOT a frame-cost stage: the menu is drawn only on frames a page covers, and the
//! frame costs throw those frames away (renderer/gpu_costs.rs `discard_frame`: no world was drawn),
//! so a `cpu.nav` stage would never be published (and a `cpu.span.*` cannot sit inside
//! `cpu.span.ui_run`). Its cost is measured headless instead: escape_menu_tests.rs
//! `the_two_rows_cost_a_small_part_of_a_frame` (printed with `--nocapture`).

use egui::{Color32, CornerRadius, FontId, Frame, Id, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use humanity_core::i18n::{t, t_dyn};

use crate::gui::navigation::{self, Menu, NavButton, NavCategory};
use crate::gui::theme::Theme;
use crate::gui::widgets::hint::Hint;
use crate::gui::{GuiPage, GuiState, NavDisplayMode};

/// Where the hover state is kept in egui's memory.
const HOVER_ID: &str = "nav_rows_hover";
/// The category the second row last showed for a page of its own (a page the rows leave out, such
/// as Relay Control from the chat's sidebar, keeps the row it was opened under).
const DOCKED_ID: &str = "nav_rows_docked";
/// The secondary row's rect, last frame (the preview stays while the pointer is on either row).
const SECONDARY_RECT_ID: &str = "nav_rows_secondary_rect";
/// The two rows' More lists (egui popups).
const PRIMARY_MORE: &str = "nav_primary_more";
const SECONDARY_MORE: &str = "nav_secondary_more";
/// The space between two buttons of a row, and inside a button around its words.
const GAP: f32 = 5.0;
const PAD: f32 = 12.0;
const ICON_GAP: f32 = 7.0;

/// The pointer's rest and the preview, between frames.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Hover {
    /// The category the pointer rests on, and since when (egui's clock, seconds).
    resting: Option<(usize, f64)>,
    /// The category the second row previews.
    preview: Option<usize>,
    /// When the pointer left both rows while a preview showed.
    left_at: Option<f64>,
}

/// One frame's step of the preview (a pure function, so its timings are tested on their own):
/// `hovered` is the category button under the pointer, `in_rows` whether the pointer is on either
/// row, `docked` the category of the page on screen.
fn step(prev: Hover, now: f64, hovered: Option<usize>, in_rows: bool, docked: Option<usize>, on: bool, rest: f64, leave: f64) -> Hover {
    if !on {
        return Hover::default();
    }
    let mut h = prev;
    h.resting = match (hovered, prev.resting) {
        (Some(i), Some((j, since))) if i == j => Some((i, since)),
        (Some(i), _) => Some((i, now)),
        (None, _) => None,
    };
    if let Some((i, since)) = h.resting {
        if now - since >= rest {
            // Resting on the docked category's own button brings its row back.
            h.preview = if Some(i) == docked { None } else { Some(i) };
        }
    }
    if in_rows {
        h.left_at = None;
    } else if h.preview.is_some() {
        let left = *h.left_at.get_or_insert(now);
        if now - left >= leave {
            h.preview = None;
            h.left_at = None;
        }
    }
    h
}

/// Draw the top menu: the two rows, then the notices bell (anchored beside the help button).
pub fn draw_nav_bar(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    draw_rows(ctx, theme, state);
    // The notices bell (notifications rung 2), anchored beside the help button: every page with
    // this header has it (gui/notice_panel.rs).
    crate::gui::notice_panel::draw(ctx, theme, state);
}

/// Escape, first: end the second row's preview, or close a More list, when one is showing (section
/// 10.4, rule 1: Escape closes the nearest thing open). True when it did, so the key does nothing
/// else (lib.rs).
pub fn end_preview(ctx: &egui::Context) -> bool {
    let id = Id::new(HOVER_ID);
    let h: Hover = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
    let more_open = ctx.memory(|m| m.is_popup_open(Id::new(PRIMARY_MORE)) || m.is_popup_open(Id::new(SECONDARY_MORE)));
    if h.preview.is_none() && !more_open {
        return false;
    }
    ctx.data_mut(|d| d.insert_temp(id, Hover::default()));
    if more_open {
        ctx.memory_mut(|m| m.close_popup());
    }
    true
}

/// The y offset that centres a square of `size` in the primary row, for the buttons anchored at
/// the window's top-right corner (the notices bell, help).
pub fn corner_y(theme: &Theme, size: f32) -> f32 {
    ((theme.nav_primary_height - size) * 0.5).max(2.0)
}

/// How far the two rows (and the line under them) reach down the window, in points: where the
/// pinned help panel hangs from.
pub fn rows_height(theme: &Theme) -> f32 {
    theme.nav_primary_height + theme.nav_secondary_height + theme.nav_separator_height
}

/// The room the window's top-right corner keeps for the help button and the notices bell, which
/// are anchored there and not laid out in the row.
fn corner_reserve() -> f32 {
    crate::gui::pages::keymap::HELP_TOGGLE_SIZE + crate::gui::notice_panel::BELL_SIZE + crate::gui::notice_panel::BELL_GAP + 18.0
}

/// A response named for assistive technology (its words, and whether it is the current one), so
/// a Tab stop says what it is (WCAG 4.1.2; the a11y Tab walk holds every stop to it).
fn named(resp: egui::Response, enabled: bool, selected: Option<bool>, words: &str) -> egui::Response {
    resp.widget_info(|| match selected {
        Some(s) => egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, s, words),
        None => egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, words),
    });
    resp
}

fn tint(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

fn colour(theme: &Theme, cat: &NavCategory) -> Color32 {
    navigation::colour_of(&cat.colour).map_or(theme.text_muted(), |c| c(theme))
}

/// Open an entry's page: a menu click is LATERAL, so the back stack empties and Escape on the new
/// page goes to the world rather than through stale steps (v0.195). The world's entries go in
/// through Play's one way in (pages/play.rs); a Dev row item opens its part of the Dev page, and
/// the Dev button the developer's tools.
pub(crate) fn open(state: &mut GuiState, b: &NavButton) {
    if b.page == GuiPage::None {
        crate::gui::pages::play::enter_world(state, b.picker());
        return;
    }
    state.clear_nav_back();
    if b.page == GuiPage::Dev {
        state.active_dev_section = b.section.clone().unwrap_or_else(|| crate::gui::pages::dev_tab::FIRST_SECTION.to_string());
    }
    state.active_page = b.page;
}

/// The category whose row docks under the primary row: the page on screen's, else the last one
/// that had one.
fn docked(ctx: &egui::Context, menu: &Menu, state: &GuiState) -> (Option<usize>, Option<usize>) {
    let id = Id::new(DOCKED_ID);
    match menu.place_of(state.active_page, &state.active_dev_section) {
        Some((ci, item)) => {
            ctx.data_mut(|d| d.insert_temp(id, ci));
            (Some(ci), item)
        }
        None => (ctx.data(|d| d.get_temp::<usize>(id)).filter(|ci| *ci < menu.desktop.len()), None),
    }
}

/// A button's width in a row, from its words and whether its icon shows.
fn button_width(ui: &egui::Ui, words: &str, size: f32, icon: Option<f32>, show_words: bool) -> f32 {
    let w = if show_words { ui.painter().layout_no_wrap(words.to_string(), FontId::proportional(size), Color32::PLACEHOLDER).size().x } else { 0.0 };
    let i = icon.unwrap_or(0.0);
    let g = if icon.is_some() && show_words { ICON_GAP } else { 0.0 };
    PAD + i + g + w + PAD
}

/// How many of `widths` fit in `room` in order, with a More of `more_w` after them when not all do.
fn fitting(widths: &[f32], room: f32, more_w: f32) -> usize {
    let total: f32 = widths.iter().sum::<f32>() + GAP * widths.len().saturating_sub(1) as f32;
    if total <= room {
        return widths.len();
    }
    let mut used = more_w;
    let mut n = 0;
    for w in widths {
        if used + w + GAP > room {
            break;
        }
        used += w + GAP;
        n += 1;
    }
    n
}

#[derive(Clone, Copy, PartialEq)]
enum Look {
    Idle,
    Hovered,
    Previewed,
    Active,
}

fn draw_rows(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // The active button's channeling colour animates (the RGB language of the menu, v0.177).
    ctx.request_repaint();
    let menu = crate::gui::navigation::menu();
    let timings = menu.file.timings;
    let now = ctx.input(|i| i.time);
    let (docked_cat, docked_item) = docked(ctx, &menu, state);
    let hover_id = Id::new(HOVER_ID);
    let prev: Hover = ctx.data(|d| d.get_temp(hover_id)).unwrap_or_default();
    let shown_before = prev.preview.or(docked_cat);

    let primary = primary_row(ctx, theme, state, &menu, docked_cat, prev.preview);
    if primary.clicked {
        // A click goes somewhere: the row is the new page's, and any preview ends.
        ctx.data_mut(|d| d.insert_temp(hover_id, Hover::default()));
    } else {
        let pointer = ctx.input(|i| i.pointer.hover_pos());
        let secondary_last: Rect = ctx.data(|d| d.get_temp(Id::new(SECONDARY_RECT_ID))).unwrap_or(Rect::NOTHING);
        let in_more = ctx.memory(|m| m.is_popup_open(Id::new(SECONDARY_MORE)));
        let in_rows = in_more || pointer.is_some_and(|p| primary.rect.contains(p) || secondary_last.contains(p));
        let h = step(prev, now, primary.hovered, in_rows, docked_cat, state.settings.nav_preview, timings.preview_rest_ms as f64 / 1000.0, timings.preview_leave_ms as f64 / 1000.0);
        // The timers ask for a repaint when they end (the row would otherwise wait for input).
        if let Some((_, since)) = h.resting.filter(|_| h.preview.is_none()) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((since + timings.preview_rest_ms as f64 / 1000.0 - now).max(0.0)));
        }
        ctx.data_mut(|d| d.insert_temp(hover_id, h));
    }
    let h: Hover = ctx.data(|d| d.get_temp(hover_id)).unwrap_or_default();
    // What the second row shows: the preview, or the page on screen's own row. (A click this frame
    // already moved the page; its row comes next frame, the frame after the click.)
    let shown = if primary.clicked { shown_before } else { h.preview.or(docked_cat) };
    let previewing = h.preview.is_some() && !primary.clicked;
    if !menu.desktop.is_empty() {
        let rect = secondary_row(ctx, theme, state, &menu, shown, if previewing { None } else { docked_item }, previewing, primary.first_x);
        ctx.data_mut(|d| d.insert_temp(Id::new(SECONDARY_RECT_ID), rect));
    }
    // The RGB line under the two rows (v0.177's design language).
    let attack_pulse = state.attack_pulse_active;
    rgb_separator(ctx, theme, "nav_one_tier_sep", attack_pulse);
}

/// What the primary row reports to the frame: the category under the pointer, the row's rect,
/// where its first category starts (the second row lines up under it), and whether a click on it
/// went somewhere.
struct PrimaryOut {
    hovered: Option<usize>,
    rect: Rect,
    first_x: f32,
    clicked: bool,
}

fn primary_row(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, menu: &Menu, docked: Option<usize>, preview: Option<usize>) -> PrimaryOut {
    let mut out = PrimaryOut { hovered: None, rect: Rect::NOTHING, first_x: 0.0, clicked: false };
    egui::TopBottomPanel::top("nav_primary")
        .exact_height(theme.nav_primary_height)
        .frame(Frame::NONE.fill(theme.bg_card()).inner_margin(egui::Margin::symmetric(8, 0)))
        // The RGB line under both rows is the divider; the panel's own grey line would double it.
        .show_separator_line(false)
        .show(ctx, |ui| {
            let full = ui.max_rect();
            out.rect = ui.min_rect().union(full).expand2(egui::vec2(8.0, 0.0));
            let cy = full.center().y;
            let bh = theme.nav_primary_button_height.min(theme.nav_primary_height);
            let mode = state.nav_display_mode;
            let mut x = full.left();

            // Back and Forward (rung 2 draws them; rung 4 is the history they walk).
            for back in [true, false] {
                let r = Rect::from_min_size(egui::pos2(x, cy - bh / 2.0), Vec2::splat(bh));
                arrow_button(ui, theme, state, r, back);
                x = r.right() + 3.0;
            }
            x += 10.0;
            out.first_x = x;

            // The Aa switch, at the right end before the corner the bell and help keep.
            let aa_w = bh;
            let aa = Rect::from_min_size(egui::pos2(full.right() - corner_reserve() - aa_w, cy - bh / 2.0), Vec2::new(aa_w, bh));

            // The categories that fit, in order; the rest under More.
            let size = theme.font_size_body;
            let icon = mode.show_icon().then_some(theme.nav_icon_size);
            let widths: Vec<f32> = menu.desktop.iter().map(|c| button_width(ui, t_dyn(&c.button.label), size, icon, mode.show_label())).collect();
            let more_w = button_width(ui, t("nav.more"), size, None, true) + GAP;
            let fit = fitting(&widths, (aa.left() - 10.0 - x).max(0.0), more_w);
            let hold_s = menu.file.timings.play_hold_ms as f32 / 1000.0;
            for (ci, cat) in menu.desktop.iter().enumerate().take(fit) {
                let r = Rect::from_min_size(egui::pos2(x, cy - bh / 2.0), Vec2::new(widths[ci], bh));
                let look = if Some(ci) == docked {
                    Look::Active
                } else if Some(ci) == preview {
                    Look::Previewed
                } else {
                    Look::Idle
                };
                let (hovered, clicked) = category_button(ui, theme, state, r, cat, look, mode, hold_s, full.bottom());
                if hovered {
                    out.hovered = Some(ci);
                }
                out.clicked |= clicked;
                x = r.right() + GAP;
            }
            if fit < menu.desktop.len() {
                let r = Rect::from_min_size(egui::pos2(x, cy - bh / 2.0), Vec2::new(more_w - GAP, bh));
                let active = docked.is_some_and(|d| d >= fit);
                let rest: Vec<(bool, &NavButton)> = menu.desktop.iter().enumerate().skip(fit).map(|(ci, c)| (Some(ci) == docked, &c.button)).collect();
                // Marked in the page's category's colour when that category is in its list.
                let col = docked.and_then(|d| menu.desktop.get(d)).map_or(theme.accent(), |c| colour(theme, c));
                out.clicked |= more_button(ui, theme, state, r, Id::new(PRIMARY_MORE), active.then_some(col), &rest, size);
            }

            let aa_resp = named(ui.interact(aa, Id::new("nav_display_mode"), Sense::click()), true, None, t("nav.display_mode"));
            let fill = if aa_resp.hovered() { theme.bg_tertiary() } else { theme.bg_secondary() };
            ui.painter().rect_filled(aa, CornerRadius::same(6), fill);
            ui.painter().text(aa.center(), egui::Align2::CENTER_CENTER, mode.glyph(), FontId::proportional(theme.font_size_small), theme.text_muted());
            if aa_resp.hint("nav.display_mode_hint").clicked() {
                state.nav_display_mode = state.nav_display_mode.next();
                // Saved at once (v0.1066): the choice used to be lost on every restart.
                crate::config::AppConfig::from_gui_state(state).save();
            }
        });
    out
}

/// Back or Forward, an arrow in a square (dimmed, with a hint saying why, when it has nowhere to
/// go).
fn arrow_button(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, r: Rect, back: bool) {
    let can = back && !state.nav_back_stack.is_empty();
    let (name, hint) = match (back, can) {
        (true, true) => ("nav.back", "nav.back_from_hint"),
        (true, false) => ("nav.back", "nav.back_none_hint"),
        (false, _) => ("nav.forward", "nav.forward_not_yet_hint"),
    };
    let id = Id::new(if back { "nav_back" } else { "nav_forward" });
    // Disabled when there is nowhere to go: not a Tab stop and not clickable, its hint still under
    // the pointer (the hint system's disabled hover).
    let resp = ui.add_enabled_ui(can, |ui| named(ui.interact(r, id, Sense::click()), can, None, t(name))).inner;
    let fill = if can && resp.hovered() { theme.bg_tertiary() } else { theme.bg_secondary() };
    let ink = if can { theme.text_primary() } else { tint(theme.text_muted(), 110) };
    ui.painter().rect_filled(r, CornerRadius::same(6), fill);
    let ir = r.shrink(r.height() * 0.28);
    if back {
        crate::gui::widgets::icons::paint_arrow_left(ui.painter(), ir, ink);
    } else {
        crate::gui::widgets::icons::paint_arrow_right(ui.painter(), ir, ink);
    }
    if resp.hint(hint).clicked() && can {
        state.pop_nav_back();
    }
}

/// One category's button in the primary row. Returns (the pointer is on it, a click went
/// somewhere). Play's is pressed and held to go into the world; a tap opens its page.
#[allow(clippy::too_many_arguments)]
fn category_button(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, rect: Rect, cat: &NavCategory, look: Look, mode: NavDisplayMode, hold_s: f32, row_bottom: f32) -> (bool, bool) {
    let b = &cat.button;
    let is_play = b.page == GuiPage::Play;
    let id = Id::new(("nav_primary", cat.id.as_str()));
    let words = t_dyn(&b.label);
    // Play's button senses a drag too, so a press held still keeps it (the hold).
    let drag = if is_play { Sense::DRAG } else { Sense::empty() };
    let resp = named(ui.interact(rect, id, Sense::click() | drag), true, Some(look == Look::Active), words);
    let col = colour(theme, cat);
    let look = if look == Look::Idle && resp.hovered() { Look::Hovered } else { look };
    let time = ui.input(|i| i.time) as f32;
    let (fill, stroke) = match look {
        Look::Active => (tint(col, 46), Stroke::new(theme.nav_active_border_width, channeling_color(theme, time, state.attack_pulse_active, col))),
        Look::Previewed => (tint(col, 30), Stroke::new(theme.nav_hover_border_width, col)),
        Look::Hovered => (tint(col, 22), Stroke::new(theme.nav_hover_border_width, theme.nav_legacy_blue())),
        Look::Idle => (tint(col, 12), Stroke::new(1.0_f32, tint(col, 90))),
    };
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(6), fill);

    // Play: a press and hold goes into the world, the button filling from the left as it is held.
    // Once a hold has gone in, the rest of that press (and its release) does nothing more: the
    // latch is kept until the button is let go.
    let latch = id.with("went_in");
    let latched = is_play && ui.ctx().data(|d| d.get_temp::<bool>(latch)).unwrap_or(false);
    let mut went_in = false;
    if is_play {
        let holding = resp.is_pointer_button_down_on() && !latched;
        let (progress, fired) = crate::gui::widgets::hold_step(ui, id.with("hold"), holding, hold_s);
        if fired {
            ui.ctx().data_mut(|d| d.insert_temp(latch, true));
            crate::gui::pages::play::enter_world(state, false);
            went_in = true;
        } else if holding && progress > 0.0 {
            let fill_r = Rect::from_min_size(rect.min, Vec2::new(rect.width() * progress.min(1.0), rect.height()));
            ui.painter().rect_filled(fill_r, CornerRadius::same(6), tint(col, 120));
        }
        if latched && !resp.is_pointer_button_down_on() {
            ui.ctx().data_mut(|d| d.remove::<bool>(latch));
        }
    }
    let painter = ui.painter();
    painter.rect_stroke(rect, CornerRadius::same(6), stroke, StrokeKind::Inside);
    if look == Look::Active {
        // The category's colour runs down into its row.
        let bar = Rect::from_min_size(egui::pos2(rect.left() + 8.0, row_bottom - 3.0), Vec2::new((rect.width() - 16.0).max(0.0), 3.0));
        painter.rect_filled(bar, CornerRadius::ZERO, col);
    }
    let ink = if look == Look::Idle { theme.text_secondary() } else { theme.text_strong() };
    let mut x = rect.left() + PAD;
    if mode.show_icon() {
        let s = theme.nav_icon_size;
        let ir = Rect::from_min_size(egui::pos2(x, rect.center().y - s / 2.0), Vec2::splat(s));
        if let Some(paint) = crate::gui::widgets::icons::named(&b.icon) {
            paint(painter, ir, ink);
        }
        x += s + ICON_GAP;
    }
    if mode.show_label() {
        let g = painter.layout_no_wrap(words.to_string(), FontId::proportional(theme.font_size_body), ink);
        painter.galley(egui::pos2(x, rect.center().y - g.size().y / 2.0), g, ink);
    }
    // Unread messages anywhere: a dot on the Chat button, so chat activity shows from every page
    // (v0.719).
    if b.page == GuiPage::Chat && (state.chat_dms.iter().any(|d| d.unread) || state.chat_channels.iter().any(|c| c.unread)) {
        painter.circle_filled(egui::pos2(rect.right() - 5.0, rect.top() + 5.0), 3.0, theme.danger());
    }
    let hovered = resp.hovered();
    let mut clicked = went_in;
    let resp = resp.hint_dyn(&b.hint);
    if resp.clicked() && !went_in && !latched {
        open(state, b);
        clicked = true;
    }
    (hovered, clicked)
}

/// A row's More: its button, and while open the list of the entries that did not fit. True when
/// an entry was chosen.
#[allow(clippy::too_many_arguments)]
/// `active` is the colour it is marked in when the page on screen is in its list.
fn more_button(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, r: Rect, popup: Id, active: Option<Color32>, rest: &[(bool, &NavButton)], size: f32) -> bool {
    let resp = named(ui.interact(r, popup.with("button"), Sense::click()), true, Some(active.is_some()), t("nav.more"));
    let fill = if let Some(col) = active { tint(col, 46) } else if resp.hovered() { theme.bg_tertiary() } else { theme.bg_secondary() };
    let painter = ui.painter();
    painter.rect_filled(r, CornerRadius::same(6), fill);
    let ink = if active.is_some() || resp.hovered() { theme.text_strong() } else { theme.text_secondary() };
    let g = painter.layout_no_wrap(t("nav.more").to_string(), FontId::proportional(size), ink);
    painter.galley(egui::pos2(r.left() + PAD, r.center().y - g.size().y / 2.0), g, ink);
    let resp = resp.hint("nav.more_hint");
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    let mut chosen: Option<NavButton> = None;
    egui::popup_below_widget(ui, popup, &resp, egui::PopupCloseBehavior::CloseOnClick, |ui| {
        ui.set_min_width(180.0);
        for (is_here, b) in rest {
            let words = RichText::new(t_dyn(&b.label)).size(size);
            if ui.selectable_label(*is_here, words).hint_dyn(&b.hint).clicked() {
                chosen = Some((*b).clone());
            }
        }
    });
    if let Some(b) = chosen {
        ui.memory_mut(|m| m.close_popup());
        open(state, &b);
        return true;
    }
    false
}

/// The secondary row: `shown`'s pages (the docked category's, or a previewed one, tinted in its
/// colour), the page on screen marked, More for what does not fit. Returns the row's rect.
#[allow(clippy::too_many_arguments)]
fn secondary_row(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, menu: &Menu, shown: Option<usize>, here: Option<usize>, previewing: bool, first_x: f32) -> Rect {
    let mut rect = Rect::NOTHING;
    let cat = shown.and_then(|ci| menu.desktop.get(ci));
    let col = cat.map_or(theme.text_muted(), |c| colour(theme, c));
    egui::TopBottomPanel::top("nav_secondary")
        .exact_height(theme.nav_secondary_height)
        .frame(Frame::NONE.fill(theme.bg_secondary()).inner_margin(egui::Margin::symmetric(8, 0)))
        .show_separator_line(false)
        .show(ctx, |ui| {
            let full = ui.max_rect();
            rect = full.expand2(egui::vec2(8.0, 0.0));
            ui.painter().rect_filled(rect, CornerRadius::ZERO, tint(col, if previewing { 26 } else { 12 }));
            let Some(cat) = cat else { return };
            let cy = full.center().y;
            let bh = theme.nav_secondary_button_height.min(theme.nav_secondary_height);
            let size = theme.nav_secondary_font_size;
            let widths: Vec<f32> = cat.items.iter().map(|b| button_width(ui, t_dyn(&b.label), size, None, true)).collect();
            let more_w = button_width(ui, t("nav.more"), size, None, true) + GAP;
            let x0 = first_x.max(full.left());
            let fit = fitting(&widths, (full.right() - x0).max(0.0), more_w);
            let mut x = x0;
            for (ii, b) in cat.items.iter().enumerate().take(fit) {
                let r = Rect::from_min_size(egui::pos2(x, cy - bh / 2.0), Vec2::new(widths[ii], bh));
                let is_here = here == Some(ii);
                let id = Id::new(("nav_secondary", b.id.as_str()));
                let words = t_dyn(&b.label);
                let resp = named(ui.interact(r, id, Sense::click()), true, Some(is_here), words);
                let painter = ui.painter();
                if is_here {
                    painter.rect_filled(r, CornerRadius::same(5), tint(col, 40));
                    painter.rect_filled(Rect::from_min_size(egui::pos2(r.left() + 6.0, r.bottom() - 3.0), Vec2::new(r.width() - 12.0, 3.0)), CornerRadius::same(1), col);
                } else if resp.hovered() {
                    painter.rect_filled(r, CornerRadius::same(5), tint(col, 24));
                }
                let ink = if is_here || resp.hovered() { theme.text_strong() } else { theme.text_secondary() };
                let g = painter.layout_no_wrap(words.to_string(), FontId::proportional(size), ink);
                painter.galley(egui::pos2(r.left() + PAD, cy - g.size().y / 2.0), g, ink);
                if resp.hint_dyn(&b.hint).clicked() {
                    open(state, b);
                }
                x = r.right() + GAP;
            }
            if fit < cat.items.len() {
                let r = Rect::from_min_size(egui::pos2(x, cy - bh / 2.0), Vec2::new(more_w - GAP, bh));
                let active = here.is_some_and(|i| i >= fit);
                let rest: Vec<(bool, &NavButton)> = cat.items.iter().enumerate().skip(fit).map(|(ii, b)| (here == Some(ii), b)).collect();
                more_button(ui, theme, state, r, Id::new(SECONDARY_MORE), active.then_some(col), &rest, size);
            }
        });
    rect
}

/// Pin the second row's preview on a category (the snapshots: a picture has no pointer to rest).
#[cfg(test)]
pub(crate) fn preview_for_snapshot(ctx: &egui::Context, category: &str) {
    let ci = crate::gui::navigation::menu().desktop.iter().position(|c| c.id == category);
    ctx.data_mut(|d| d.insert_temp(Id::new(HOVER_ID), Hover { resting: None, preview: ci, left_at: None }));
}

/// Convert HSV to RGB Color32. theme-exempt: pure math helper.
pub(crate) fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color32 {
    let i = (h * 6.0).floor() as i32;
    let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Active-element "channeling" color, theme-aware (v0.177.0).
///
/// Reads `theme.nav_active_border_animation` (RGB_CYCLE / SOLID / PULSE / OFF)
/// and `theme.attack_indicator_style` (PULSE_RED / PULSE_YELLOW / FLASH_WHITE
/// / BORDER_ONLY / NONE) plus `animations_enabled` master switch + the speed
/// multipliers. `fallback` is the contextual color used in SOLID mode (e.g.
/// the active category's tinted color) and as the BORDER_ONLY attack color.
///
/// Shared by active nav buttons + the RGB separator so all attention-grabbing
/// elements animate in lockstep.
pub fn channeling_color(
    theme: &Theme,
    time: f32,
    attack_pulse: bool,
    fallback: Color32,
) -> Color32 {
    use crate::gui::theme::{anim, attack as atk};

    if !theme.animations_enabled {
        // Hold a static color when animations are disabled. For attack
        // mode pick the danger color; otherwise use the fallback.
        return if attack_pulse { theme.danger() } else { fallback };
    }

    if attack_pulse {
        let t_speed = (time * 4.0 * theme.attack_indicator_speed).max(0.0);
        match theme.attack_indicator_style {
            atk::NONE => {} // fall through to non-attack path
            atk::PULSE_RED => {
                let t = t_speed.sin() * 0.5 + 0.5;
                let v = 0.55 + 0.45 * t;
                // theme-exempt: programmatic pulse from theme tokens.
                return Color32::from_rgb((v * 255.0) as u8, 0, 0);
            }
            atk::PULSE_YELLOW => {
                let t = t_speed.sin() * 0.5 + 0.5;
                let v = 0.55 + 0.45 * t;
                // theme-exempt: programmatic pulse from theme tokens.
                return Color32::from_rgb((v * 255.0) as u8, (v * 220.0) as u8, 0);
            }
            atk::FLASH_WHITE => {
                // Square-ish blink at ~6 Hz.
                let t = ((time * 12.0 * theme.attack_indicator_speed).sin() > 0.0) as u8;
                return if t == 1 { Color32::WHITE } else { theme.danger() };
            }
            atk::BORDER_ONLY => return theme.danger(),
            _ => {}
        }
    }

    // Non-attack path: separator/active-border animation style.
    let speed = theme.nav_separator_animation_speed.max(0.0);
    match theme.nav_active_border_animation {
        anim::OFF => fallback,
        anim::SOLID => fallback,
        anim::RGB_CYCLE => {
            let hue = (time * 0.3 * speed) % 1.0;
            hsv_to_rgb(hue.rem_euclid(1.0), 0.9, 1.0)
        }
        anim::PULSE => {
            let t = (time * 2.0 * speed).sin() * 0.5 + 0.5;
            let v = 0.4 + 0.6 * t;
            // theme-exempt: brightness modulation of fallback color.
            Color32::from_rgb(
                (fallback.r() as f32 * v) as u8,
                (fallback.g() as f32 * v) as u8,
                (fallback.b() as f32 * v) as u8,
            )
        }
        _ => fallback,
    }
}

/// Thin horizontal separator panel under the two rows. Honors
/// `theme.nav_separator_animation` (OFF / SOLID / RGB_CYCLE / PULSE)
/// and `animations_enabled` master switch. Returns early without
/// allocating panel space when style == OFF.
fn rgb_separator(ctx: &egui::Context, theme: &Theme, panel_id: &'static str, attack_pulse: bool) {
    use crate::gui::theme::anim;
    if theme.nav_separator_animation == anim::OFF { return; }

    egui::TopBottomPanel::top(panel_id)
        .frame(Frame::NONE.fill(theme.bg_primary()).inner_margin(0.0))
        .exact_height(theme.nav_separator_height)
        .show_separator_line(false)
        .show(ctx, |ui| {
            let time = ui.ctx().input(|i| i.time) as f32;
            // Separator's "fallback" solid color is the accent — that's the
            // single color used when SOLID is selected or animations off.
            let fallback = theme.accent();
            let color = if theme.animations_enabled
                && theme.nav_separator_animation != anim::SOLID
            {
                separator_color(theme, time, attack_pulse, fallback)
            } else if attack_pulse && theme.animations_enabled {
                // Honor attack indicator even in SOLID mode for safety.
                channeling_color(theme, time, attack_pulse, fallback)
            } else {
                fallback
            };
            let rect = ui.max_rect();
            ui.painter().rect_filled(rect, CornerRadius::ZERO, color);
        });
}

/// Like `channeling_color` but reads `nav_separator_animation` instead of
/// `nav_active_border_animation`. Lets the separator pick a different style
/// from the active button border (e.g. solid separator + RGB borders).
fn separator_color(theme: &Theme, time: f32, attack_pulse: bool, fallback: Color32) -> Color32 {
    use crate::gui::theme::anim;
    if attack_pulse {
        return channeling_color(theme, time, attack_pulse, fallback);
    }
    let speed = theme.nav_separator_animation_speed.max(0.0);
    match theme.nav_separator_animation {
        anim::OFF => fallback,
        anim::SOLID => fallback,
        anim::RGB_CYCLE => {
            let hue = (time * 0.3 * speed) % 1.0;
            hsv_to_rgb(hue.rem_euclid(1.0), 0.9, 1.0)
        }
        anim::PULSE => {
            let t = (time * 2.0 * speed).sin() * 0.5 + 0.5;
            let v = 0.4 + 0.6 * t;
            // theme-exempt: brightness modulation of fallback color.
            Color32::from_rgb(
                (fallback.r() as f32 * v) as u8,
                (fallback.g() as f32 * v) as u8,
                (fallback.b() as f32 * v) as u8,
            )
        }
        _ => fallback,
    }
}

#[cfg(test)]
#[path = "escape_menu_tests.rs"]
mod tests;
