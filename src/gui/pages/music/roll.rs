//! THE PIANO ROLL, version 2 (docs/design/music-page.md section 7; the operator, 2026-11-20: "I love
//! the bars view at the bottom but, could we draw the whole song or variable? Like a slider or
//! something choosing visualized length"). A slider says how much of the piece it draws, from two
//! bars to the whole piece; an OVERVIEW above the bars draws the whole piece at once with the bars
//! shown marked as a window (click or drag it to move the window); FOLLOW THE MUSIC keeps the
//! playhead in view, or the window stays where the person put it. The bars carry their numbers, the
//! section letters and chord symbols sit above them, each part in its colour (a muted part faint).
//!
//! The sheet is the piece written down: a written piece's own timeline, or a composed piece written
//! down by the composer run dry, to its end when it has a set length (view.rs `ensure_sheet`).

use egui::epaint::StrokeKind;
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use humanity_core::i18n::t;
use humanity_music::sheet::{chord_symbol, Sheet};
use humanity_music::tune::ROLES;

use super::role_colour;
use crate::gui::theme::Theme;
use crate::gui::widgets::Hint;
use crate::gui::{widgets, GuiState};

/// The width of the roll's bar lines, one at the start of each bar drawn (the tests count them: what
/// is drawn, never the slider's value).
pub const BAR_LINE_W: f32 = 1.25;
/// The width of the overview's window outline (theme accent).
pub const WINDOW_LINE_W: f32 = 1.5;
/// The overview's height, points.
const OVERVIEW_H: f32 = 34.0;
/// Notes in view past which the roll merges them per two-pixel column, pitch and part (a whole
/// symphony in view is tens of thousands of notes and as many rectangles).
const MERGE_PAST: usize = 3000;
/// The narrowest bar (points) that gets its number; denser bars are numbered every few.
const NUMBER_PX: f32 = 22.0;

/// The lengths the slider offers for a piece of `total` bars: 2, 4, 8 ... below it, then the whole
/// piece (0).
pub fn length_steps(total: usize) -> Vec<usize> {
    let mut v = Vec::new();
    let mut n = 2;
    while n < total {
        v.push(n);
        n *= 2;
    }
    v.push(0);
    v
}

/// How many bars the roll draws of a piece of `total` bars when `chosen` are asked for (0: all).
pub fn bars_shown(chosen: usize, total: usize) -> usize {
    if chosen == 0 || chosen >= total {
        total
    } else {
        chosen
    }
}

/// The first bar drawn: following the music, the playhead in the first quarter of the bars shown
/// (one bar behind it at four bars, as version 1 drew); else where the person put the window.
pub fn first_bar(sheet: &Sheet, now: f64, n: usize, follow: bool, chosen_first: usize) -> usize {
    let total = sheet.bars.len();
    if n >= total {
        return 0;
    }
    let first = if follow { sheet.bar_at(now).unwrap_or(0).saturating_sub((n / 4).clamp(1, n - 1)) } else { chosen_first };
    first.min(total - n)
}

/// Where the sheet ends, seconds: the end of its last bar.
pub fn sheet_end(sheet: &Sheet) -> f64 {
    sheet.bars.len().checked_sub(1).and_then(|b| sheet.bar_span(b)).map(|s| s.1).unwrap_or(sheet.seconds).max(1e-6)
}

/// The piano roll's card: the length slider and Follow the music, the overview, the bars.
pub(super) fn panel(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, _accent: Color32) {
    let now = state.music_dev.readout.score.map(|s| s.seconds).unwrap_or(0.0);
    // The parts muted in the choice as the page shows it (the person's, or what plays now).
    let mute = super::shown(&state.settings.music, &state.music_dev.readout).tune.mute;
    let endless = state.music_dev.readout.written_piece.is_none() && state.music_dev.readout.next_piece_s.is_none();
    // Taken out for the frame and put back (never copied: a symphony is tens of thousands of notes).
    let cache = state.music_page.sheet.take();
    let p = &mut state.music_page;
    widgets::card(ui, theme, |ui| {
        let Some(sheet) = cache.as_ref().map(|c| &c.sheet).filter(|s| !s.bars.is_empty()) else {
            ui.label(RichText::new(t("music_page.roll_empty")).color(theme.text_muted()));
            return;
        };
        let total = sheet.bars.len();
        let steps = length_steps(total);
        let shown_now = bars_shown(p.roll_bars, total);
        let mut k = steps.iter().position(|&s| bars_shown(s, total) >= shown_now).unwrap_or(steps.len() - 1);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t("music_page.roll_show")).color(theme.text_secondary()));
            let label = if steps[k] == 0 { t("music_page.roll_whole").replace("{{n}}", &total.to_string()) } else { t("music_page.roll_bars").replace("{{n}}", &steps[k].to_string()) };
            // The slider's own text names it for assistive technology (the a11y walk).
            if widgets::slider(ui, theme, egui::Slider::new(&mut k, 0..=steps.len() - 1).show_value(false).text(RichText::new(label).color(theme.text_primary()))).hint("music_page.roll_length_hint").changed() {
                p.roll_bars = steps[k];
            }
            ui.add_space(theme.spacing_sm);
            let first_now = first_bar(sheet, now, bars_shown(p.roll_bars, total), p.roll_follow, p.roll_first);
            if ui.checkbox(&mut p.roll_follow, t("music_page.roll_follow")).hint("music_page.roll_follow_hint").changed() && !p.roll_follow {
                // Let go where it is.
                p.roll_first = first_now;
            }
            if endless {
                let minutes = format!("{:.0}", (sheet_end(sheet) / 60.0).max(1.0));
                ui.label(RichText::new(t("music_page.roll_endless").replace("{{minutes}}", &minutes)).size(theme.font_size_small).color(theme.text_muted()));
            }
        });
        let n = bars_shown(p.roll_bars, total);
        let first = first_bar(sheet, now, n, p.roll_follow, p.roll_first);
        if let Some(bar) = overview(ui, theme, sheet, now, first, n, mute) {
            p.roll_follow = false;
            p.roll_first = bar.saturating_sub(n / 2).min(total - n);
        }
        let first = first_bar(sheet, now, n, p.roll_follow, p.roll_first);
        let room = ui.available_height();
        let h = if room > 1000.0 { 300.0 } else { (room - 4.0).max(140.0) };
        piano_roll(ui, theme, sheet, now, mute, first, n, h);
    });
    state.music_page.sheet = cache;
}

/// The overview: the whole piece at once (each part's notes merged per three-pixel column, the
/// section letters, the playhead) with the bars shown marked as a window. Returns the bar a click or
/// a drag on it points at.
fn overview(ui: &mut egui::Ui, theme: &Theme, sheet: &Sheet, now: f64, first: usize, n: usize, muted: u8) -> Option<usize> {
    let width = ui.available_width().max(200.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, OVERVIEW_H), Sense::click_and_drag());
    let resp = resp.hint("music_page.roll_overview_hint");
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, theme.bg_tertiary());
    let end = sheet_end(sheet);
    let x_of = |s: f64| rect.left() + (s / end) as f32 * rect.width();
    let (lo, hi) = sheet.notes.iter().filter(|x| (x.role as usize) < ROLES).fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x.midi), hi.max(x.midi)));
    let (lo, hi) = if lo > hi { (48.0, 72.0) } else { (lo - 1.0, hi + 1.0) };
    let inner = rect.shrink2(Vec2::new(0.0, 4.0));
    let y_of = |m: f32| inner.bottom() - (m - lo) / (hi - lo).max(1.0) * inner.height();
    let cols = ((rect.width() / 3.0).ceil() as usize).max(1);
    let mut spans = vec![[(f32::MAX, f32::MIN); ROLES]; cols];
    for x in sheet.notes.iter().filter(|x| (x.role as usize) < ROLES) {
        let c = (((x.at_s / end) * cols as f64) as usize).min(cols - 1);
        let s = &mut spans[c][x.role as usize];
        *s = (s.0.min(x.midi), s.1.max(x.midi));
    }
    for (c, col) in spans.iter().enumerate() {
        let x = rect.left() + (c as f32 + 0.5) * rect.width() / cols as f32;
        for (role, &(a, b)) in col.iter().enumerate() {
            if a > b {
                continue;
            }
            let faint = muted & (1 << role) != 0;
            let colour = role_colour(theme, role as u8).gamma_multiply(if faint { 0.2 } else { 0.85 });
            painter.line_segment([Pos2::new(x, y_of(a) + 0.75), Pos2::new(x, y_of(b) - 0.75)], Stroke::new(2.0f32, colour));
        }
    }
    let font = FontId::proportional(theme.font_size_small);
    for s in sheet.sections.iter().filter(|s| s.letter > 0) {
        painter.text(Pos2::new(x_of(s.at_s) + 2.0, rect.top() + 1.0), Align2::LEFT_TOP, ((b'A' + s.letter - 1) as char).to_string(), font.clone(), theme.text_secondary());
    }
    // The window: the bars the roll draws below.
    let (a, _) = sheet.bar_span(first).unwrap_or((0.0, 0.0));
    let (_, b) = sheet.bar_span((first + n).saturating_sub(1)).unwrap_or((end, end));
    let win = Rect::from_min_max(Pos2::new(x_of(a), rect.top() + 1.0), Pos2::new(x_of(b), rect.bottom() - 1.0));
    painter.rect_filled(win, 2.0, theme.accent().gamma_multiply(0.15));
    painter.rect_stroke(win, 2.0, Stroke::new(WINDOW_LINE_W, theme.accent()), StrokeKind::Inside);
    if now <= end {
        let x = x_of(now);
        painter.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())], Stroke::new(1.5f32, theme.text_primary()));
    }
    if resp.clicked() || resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let at = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64 * end;
            return sheet.bar_at(at).or(Some(sheet.bars.len().saturating_sub(1)));
        }
    }
    None
}

/// The bars `first` .. `first + n` of the piece, `height` points tall: time across, pitch up, each
/// part in its colour (a muted part faint), each bar's line and number, chord symbols above each
/// chord and the section letter at each section's start, the playhead.
#[allow(clippy::too_many_arguments)]
pub fn piano_roll(ui: &mut egui::Ui, theme: &Theme, sheet: &Sheet, now: f64, muted: u8, first: usize, n: usize, height: f32) {
    let width = ui.available_width().max(320.0);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    resp.hint("music_page.roll_hint");
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, theme.bg_card());
    let bars: Vec<usize> = (first..first + n).filter(|&b| b < sheet.bars.len()).collect();
    let Some(&last) = bars.last() else { return };
    let (t0, _) = sheet.bar_span(first).unwrap_or((0.0, 1.0));
    let (_, t1) = sheet.bar_span(last).unwrap_or((t0, t0 + 1.0));
    // Two lines above the grid: the section letters, then the chord symbols.
    let head_h = theme.font_size_small * 2.8 + 12.0;
    let grid = Rect::from_min_max(Pos2::new(rect.left() + 4.0, rect.top() + head_h), Pos2::new(rect.right() - 4.0, rect.bottom() - 4.0));
    let x_of = |s: f64| grid.left() + ((s - t0) / (t1 - t0).max(1e-6)) as f32 * grid.width();
    let bar_px = grid.width() / bars.len() as f32;
    // The notes in view and their pitch range.
    let notes: Vec<_> = sheet.notes.iter().filter(|x| (x.role as usize) < ROLES && x.at_s < t1 && x.at_s + x.dur_s as f64 > t0).collect();
    let (lo, hi) = notes.iter().fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x.midi), hi.max(x.midi)));
    let (lo, hi) = if lo > hi { (48.0, 72.0) } else { ((lo - 2.0).floor(), (hi + 2.0).ceil()) };
    let row = grid.height() / (hi - lo + 1.0).max(1.0);
    let y_of = |m: f32| grid.bottom() - (m - lo + 1.0) * row;
    // Octave lines (each C), then each bar's line (and its beats, where there is room for them).
    let mut c = (lo / 12.0).ceil() * 12.0;
    while c <= hi {
        painter.line_segment([Pos2::new(grid.left(), y_of(c) + row), Pos2::new(grid.right(), y_of(c) + row)], Stroke::new(1.0f32, theme.border().gamma_multiply(0.5)));
        c += 12.0;
    }
    let font = FontId::proportional(theme.font_size_small);
    let bpb = sheet.beats_per_bar.max(1);
    let every = (NUMBER_PX / bar_px.max(0.1)).ceil().max(1.0) as usize;
    for &b in &bars {
        let (a, end) = sheet.bar_span(b).unwrap_or((0.0, 0.0));
        let x = x_of(a);
        painter.line_segment([Pos2::new(x, grid.top()), Pos2::new(x, grid.bottom())], Stroke::new(BAR_LINE_W, theme.border()));
        if bar_px / bpb as f32 >= 6.0 {
            for k in 1..bpb {
                let xb = x_of(a + (end - a) * k as f64 / bpb as f64);
                painter.line_segment([Pos2::new(xb, grid.top()), Pos2::new(xb, grid.bottom())], Stroke::new(1.0f32, theme.border().gamma_multiply(0.35)));
            }
        }
        // Bar numbers count from 1, as a musician says "bar 40".
        if b % every == 0 {
            painter.text(Pos2::new(x + 3.0, grid.top() + 2.0), Align2::LEFT_TOP, (b + 1).to_string(), font.clone(), theme.text_muted());
        }
    }
    // Section letters and chord symbols above the grid.
    for s in sheet.sections.iter().filter(|s| s.at_s >= t0 - 1e-6 && s.at_s < t1 && s.letter > 0) {
        painter.text(Pos2::new(x_of(s.at_s) + 3.0, rect.top() + 4.0), Align2::LEFT_TOP, ((b'A' + s.letter - 1) as char).to_string(), FontId::proportional(theme.font_size_small * 1.3), theme.accent());
    }
    // The section already playing when the view begins, at its left edge (fainter).
    if let Some(s) = sheet.section_at(t0 + 1e-6).filter(|s| s.letter > 0 && s.at_s < t0 - 1e-6) {
        painter.text(Pos2::new(grid.left() + 3.0, rect.top() + 4.0), Align2::LEFT_TOP, ((b'A' + s.letter - 1) as char).to_string(), FontId::proportional(theme.font_size_small * 1.3), theme.text_secondary());
    }
    // The chords, where a bar is wide enough to name them.
    if bar_px >= 28.0 {
        let flats = sheet.flats();
        if let Some(ch) = sheet.chord_at(t0 + 1e-6).filter(|c| c.at_s < t0 - 1e-6) {
            painter.text(Pos2::new(grid.left() + 3.0, rect.top() + head_h - 4.0), Align2::LEFT_BOTTOM, chord_symbol(ch.root, ch.pcs, flats), font.clone(), theme.text_secondary());
        }
        for ch in sheet.chords.iter().filter(|c| c.at_s >= t0 - 1e-6 && c.at_s < t1) {
            painter.text(Pos2::new(x_of(ch.at_s) + 3.0, rect.top() + head_h - 4.0), Align2::LEFT_BOTTOM, chord_symbol(ch.root, ch.pcs, flats), font.clone(), theme.text_primary());
        }
    }
    // The notes: one rectangle each, or merged per two-pixel column, pitch and part when there are
    // too many to draw one by one.
    let mut seen = std::collections::HashSet::new();
    let merge = notes.len() > MERGE_PAST;
    for x in &notes {
        let x0 = x_of(x.at_s).max(grid.left());
        let x1 = x_of(x.at_s + (x.dur_s as f64).max(0.05)).max(x0 + 2.0).min(grid.right());
        if merge && !seen.insert(((x0 / 2.0) as i32, x.midi as i32, x.role)) {
            continue;
        }
        let y = y_of(x.midi);
        let faint = muted & (1 << x.role) != 0;
        let colour = role_colour(theme, x.role).gamma_multiply(if faint { 0.15 } else { 0.45 + 0.55 * x.vel.clamp(0.0, 1.0) });
        painter.rect_filled(Rect::from_min_max(Pos2::new(x0, y + 1.0), Pos2::new(x1, y + row.max(2.0) - 1.0)), 2.0, colour);
    }
    // The playhead.
    if now >= t0 && now <= t1 {
        let x = x_of(now);
        painter.line_segment([Pos2::new(x, grid.top()), Pos2::new(x, grid.bottom())], Stroke::new(2.0f32, theme.accent()));
    }
}
