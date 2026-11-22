//! THE PARTS ROW (docs/design/music-page.md section 28.7, version 3): the visualizer's part bars and
//! the Mixer, merged into one row just above the piano roll. The operator, 2026-11-21: "What if we have
//! the piano roll at the bottom like it is now, then just above that we have a row for that kind of
//! music section. Then instead of having two sections that kind of show the same thing we move those
//! volume bars in the mixer next to the actual kind of music section so we waste less vertical space on
//! kind of similar elements?"
//!
//! One column per part (pads, bass, drone, choir, bells, melody, drums, texture: `tune::ROLE_IDS`):
//! its level in decibels, its LIVE LEVEL BAR (the sound the part actually makes, `MusicStatus::roles`,
//! measured on the audio thread from each part's voices while the page is open: humanity_music::voice)
//! beside its FADER (-24 to +6 dB, named for its part, so a screen reader hears which part it moves),
//! and MUTE and SOLO under them (solo one or more parts and only those sound, the piece itself
//! unchanged: `Tune::solo`, which the score obeys as notes reach the voices). Reset the mixer sits in
//! the row; the beat pulse and the frequency spectrum (each octave band's level) at its right end. A
//! part the piece does not play is greyed, with the reason in its hint. The genre's drum machine counts
//! as the drums and its risers as the texture (`tune::part_of`), so a column acts on everything a
//! person hears as that part.
//!
//! A WIDE page draws the eight columns side by side with the pulse and spectrum after them. A PHONE
//! (or any page too narrow for eight columns and the spectrum) wraps the columns into TWO ROWS OF FOUR,
//! each a quarter of the width, with the pulse and spectrum as a strip of their own below: every part
//! stays in view (a sideways scroll would hide a muted part off the screen, and on a touch screen it
//! would fight the faders' upward drags).

use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use humanity_core::i18n::t;
use humanity_music::tune::{part_of, Tune, ROLES};

use super::{role_colour, role_words};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Button, ButtonSize, Hint};
use crate::gui::GuiState;

/// A column's width on a wide page, points.
pub const STRIP_W: f32 = 64.0;
/// The fader's travel on a wide page, points (a phone's is shorter: [`FADER_H_NARROW`]).
const FADER_H: f32 = 132.0;
const FADER_H_NARROW: f32 = 96.0;
/// The live level bar's thickness, points: wide enough to read as the part's own bar beside its fader.
const METER_W: f32 = 14.0;
/// The fader's range, dB.
pub const FADER_DB: (f32, f32) = (-24.0, 6.0);
/// The pulse and spectrum's least width beside the columns on a wide page, and their height on a phone.
const SPECTRUM_MIN_W: f32 = 300.0;
const SPECTRUM_H_NARROW: f32 = 110.0;

/// Which parts the piece shown plays (a written piece's staves, a composed piece's roles): from the
/// piano roll's transcription, all of them before it exists.
pub fn parts_playing(state: &GuiState) -> [bool; ROLES] {
    let sheet = state.music_page.sheet.as_ref().map(|s| &s.sheet);
    let mut plays = [sheet.is_none(); ROLES];
    if let Some(s) = sheet {
        for n in &s.notes {
            if let Some(k) = part_of(n.role) {
                plays[k] = true;
            }
        }
    }
    plays
}

/// Whether eight columns and the spectrum fit side by side in `width` points.
pub fn fits_in_one_row(width: f32, gap: f32) -> bool {
    width >= ROLES as f32 * (STRIP_W + gap) + SPECTRUM_MIN_W
}

/// One part's live level bar: filled from the bottom by the part's level on a decibel scale, in its
/// colour.
fn meter(ui: &mut egui::Ui, theme: &Theme, size: Vec2, level: f32, colour: Color32) {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    resp.hint("music_page.meter_hint");
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, theme.bg_tertiary());
    let f = humanity_music::score::meter_fraction(level);
    if f > 0.0 {
        painter.rect_filled(Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - rect.height() * f), rect.max), 2.0, colour);
    }
}

/// The parts row. Changes go into `tu` (the tuning the page shows), which the page writes into the
/// person's choice.
pub(super) fn parts_row(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, tu: &mut Tune, wide: bool) {
    let hint = state.settings.hint_display;
    let plays = parts_playing(state);
    let status = state.music_dev.readout.score;
    let levels = status.map(|s| s.roles).unwrap_or([0.0; ROLES]);
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        let gap = theme.spacing_xs;
        if wide && fits_in_one_row(ui.available_width(), gap) {
            ui.horizontal_top(|ui| {
                for k in 0..ROLES {
                    column(ui, theme, tu, k, plays[k], levels[k], STRIP_W, FADER_H);
                    ui.add_space(gap);
                }
                ui.add_space(theme.spacing_sm);
                let w = ui.available_width().max(SPECTRUM_MIN_W - gap);
                ui.vertical(|ui| {
                    reset(ui, theme, tu);
                    pulse_and_spectrum(ui, theme, status.as_ref(), Vec2::new(w, FADER_H + 40.0));
                });
            });
        } else {
            // Two rows of four, each column a quarter of the width (the row's own spacing between
            // columns counted, so the fourth stays inside the card).
            let sp = ui.spacing().item_spacing.x;
            let col_w = ((ui.available_width() - 3.0 * sp) / 4.0 - 1.0).floor().max(STRIP_W);
            for row in 0..2 {
                ui.horizontal_top(|ui| {
                    for k in row * 4..row * 4 + 4 {
                        column(ui, theme, tu, k, plays[k], levels[k], col_w, FADER_H_NARROW);
                    }
                });
                ui.add_space(gap);
            }
            reset(ui, theme, tu);
            let w = ui.available_width();
            pulse_and_spectrum(ui, theme, status.as_ref(), Vec2::new(w, SPECTRUM_H_NARROW));
        }
        widgets::setting_hint(ui, theme, hint, t("music_page.mixer_hint"));
    });
}

/// Reset the mixer: every part back to its own level, nothing muted or soloed (shown only when
/// something is changed).
fn reset(ui: &mut egui::Ui, theme: &Theme, tu: &mut Tune) {
    if (tu.level_db != [0.0; ROLES] || tu.mute != 0 || tu.solo != 0) && Button::new(t("music_page.mixer_reset")).size(ButtonSize::Small).hint("music_page.mixer_reset_hint").show(ui, theme) {
        tu.level_db = [0.0; ROLES];
        tu.mute = 0;
        tu.solo = 0;
    }
}

/// Mute and Solo for part `k` (toggles; a soloed part's button is marked).
fn mute_solo(ui: &mut egui::Ui, theme: &Theme, tu: &mut Tune, k: usize) {
    let bit = 1u8 << k;
    let muted = tu.mute & bit != 0;
    if Button::tab(t("music_page.mute"), muted).size(ButtonSize::Small).hint("music_page.mute_hint").show(ui, theme) {
        tu.mute ^= bit;
    }
    let soloed = tu.solo & bit != 0;
    if Button::tab(t("music_page.solo"), soloed).size(ButtonSize::Small).hint("music_page.solo_hint").show(ui, theme) {
        tu.solo ^= bit;
    }
}

/// The part's level as words ("-3 dB").
fn db_words(db: f32) -> String {
    format!("{db:+.1}{}", t("music_page.unit_db"))
}

/// One part's column: its level, the live level bar beside a vertical fader named for its part (the
/// name under the fader, so a screen reader hears which part it moves), Mute and Solo.
#[allow(clippy::too_many_arguments)]
fn column(ui: &mut egui::Ui, theme: &Theme, tu: &mut Tune, k: usize, plays: bool, level: f32, w: f32, fader_h: f32) {
    let colour = if plays { role_colour(theme, k as u8) } else { theme.text_muted() };
    ui.allocate_ui_with_layout(Vec2::new(w, fader_h + 120.0), egui::Layout::top_down(egui::Align::Center), |ui| {
        ui.set_width(w);
        let r = ui.label(RichText::new(db_words(tu.level_db[k])).size(theme.font_size_small).color(theme.text_secondary()));
        if !plays {
            r.hint("music_page.part_silent_hint");
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().slider_width = fader_h;
            ui.spacing_mut().item_spacing.x = 3.0;
            meter(ui, theme, Vec2::new(METER_W, fader_h), level, colour);
            // A copy, written back only when the person moves it (a slider holds a value to its range
            // and steps when drawn, and that alone must never count as choosing).
            let mut db = tu.level_db[k];
            let name = RichText::new(role_words(k)).size(theme.font_size_small).strong().color(colour);
            if widgets::slider(ui, theme, egui::Slider::new(&mut db, FADER_DB.0..=FADER_DB.1).vertical().step_by(0.5).show_value(false).text(name)).hint("music_page.level_hint").changed() {
                tu.level_db[k] = db;
            }
        });
        mute_solo(ui, theme, tu, k);
    });
}

/// The beat pulse (the bar's first beat strongest, the bar and beat under it) and the spectrum in
/// octave bands, -60 to 0 dB, in a box `size` big.
fn pulse_and_spectrum(ui: &mut egui::Ui, theme: &Theme, status: Option<&humanity_music::MusicStatus>, size: Vec2) {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    resp.hint("music_page.visualizer_hint");
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 6.0, theme.bg_card());
    let font = FontId::proportional(theme.font_size_small);
    let label_h = theme.font_size_small + 6.0;
    let width = rect.width();
    // The beat pulse.
    let pulse_w = 130.0f32.min(width * 0.3);
    let centre = Pos2::new(rect.left() + pulse_w * 0.5, rect.center().y - label_h * 0.5);
    let (beat, bpb) = status.map(|s| (s.beat, s.beats_per_bar.max(1) as f32)).unwrap_or((0.0, 4.0));
    let phase = beat.fract();
    let down = beat < 1.0;
    let swell = (1.0 - phase).powi(2) * if down { 1.0 } else { 0.6 };
    let base = (pulse_w * 0.22).min((rect.height() - label_h) * 0.28).min(36.0);
    let colour = if down { theme.accent() } else { theme.text_secondary() };
    if status.is_some() {
        painter.circle_filled(centre, base + swell * base * 0.6, colour.gamma_multiply(0.25 + 0.5 * swell));
        painter.circle_stroke(centre, base, Stroke::new(2.0f32, colour));
    } else {
        painter.circle_stroke(centre, base, Stroke::new(1.0f32, theme.text_muted()));
    }
    let bar_n = status.map(|s| s.bar + 1).unwrap_or(0);
    let beat_n = beat.floor() as u32 + 1;
    painter.text(Pos2::new(centre.x, rect.bottom() - label_h * 0.5), Align2::CENTER_CENTER, t("music_page.beat").replace("{{bar}}", &bar_n.to_string()).replace("{{beat}}", &format!("{beat_n}/{bpb:.0}")), font.clone(), theme.text_secondary());
    // The spectrum.
    let spec = Rect::from_min_max(Pos2::new(rect.left() + pulse_w + 8.0, rect.top() + 8.0), Pos2::new(rect.right() - 8.0, rect.bottom() - label_h));
    let n = humanity_music::score::BANDS;
    let bw = spec.width() / n as f32;
    for k in 0..n {
        let amp = status.map(|s| s.bands[k]).unwrap_or(0.0);
        let db = 20.0 * amp.max(1e-6).log10();
        let v = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        let x0 = spec.left() + k as f32 * bw + bw * 0.12;
        let x1 = x0 + bw * 0.76;
        painter.rect_filled(Rect::from_min_max(Pos2::new(x0, spec.bottom() - spec.height() * v), Pos2::new(x1, spec.bottom())), 2.0, theme.accent().gamma_multiply(0.4 + 0.6 * v));
        // Every band's frequency where there is room, else every third.
        let every = if bw >= theme.font_size_small * 2.6 { 1 } else { 3 };
        if k % every == 0 || k == n - 1 {
            let hz = humanity_music::score::band_hz(k);
            let label = if hz >= 1000.0 { format!("{:.0}k", hz / 1000.0) } else { format!("{hz:.0}") };
            painter.text(Pos2::new((x0 + x1) * 0.5, rect.bottom() - label_h * 0.5), Align2::CENTER_CENTER, label, font.clone(), theme.text_muted());
        }
    }
}
