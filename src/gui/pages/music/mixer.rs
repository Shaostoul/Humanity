//! THE MIXER (docs/design/music-page.md section 8, rung 2): a compact strip for each part (pads,
//! bass, drone, choir, bells, melody, drums, texture: `tune::ROLE_IDS`), each with its colour and
//! name, a level fader (-24 to +6 dB), MUTE, SOLO (solo one or more parts and only those sound, the
//! piece itself unchanged: `Tune::solo`, which the score obeys as notes reach the voices) and a level
//! meter fed by the sound the part actually makes (`MusicStatus::roles`, measured on the audio thread
//! from each part's voices while the page is open: humanity_music::voice). A part the piece does not
//! play is greyed, with the reason in its hint.
//!
//! Wide (a desktop window), the strips stand side by side, each a column about 64 points wide with a
//! vertical fader; narrow (a phone), one row each. The genre's drum machine counts as the drums and its
//! risers as the texture (`tune::part_of`), so a strip acts on everything a person hears as that part.

use egui::{Color32, Pos2, Rect, RichText, Sense, Vec2};
use humanity_core::i18n::t;
use humanity_music::tune::{part_of, Tune, ROLES};

use super::{role_colour, role_words};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Button, ButtonSize, Hint};
use crate::gui::GuiState;

/// A strip's width on a wide page, points.
pub const STRIP_W: f32 = 64.0;
/// The fader's travel on a wide page, points.
const FADER_H: f32 = 132.0;
/// The meter's thickness, points.
const METER_W: f32 = 8.0;
/// The fader's range, dB.
pub const FADER_DB: (f32, f32) = (-24.0, 6.0);

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

/// One part's meter: a bar `size` big, filled from the bottom (or the left) by the part's level on a
/// decibel scale, in its colour.
fn meter(ui: &mut egui::Ui, theme: &Theme, size: Vec2, level: f32, colour: Color32, vertical: bool) {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    resp.hint("music_page.meter_hint");
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, theme.bg_tertiary());
    let f = humanity_music::score::meter_fraction(level);
    if f > 0.0 {
        let fill = if vertical { Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - rect.height() * f), rect.max) } else { Rect::from_min_max(rect.min, Pos2::new(rect.left() + rect.width() * f, rect.bottom())) };
        painter.rect_filled(fill, 2.0, colour);
    }
}

/// The mixer card: a strip per part. Changes go into `tu` (the tuning the page shows), which the page
/// writes into the person's choice.
pub(super) fn mixer(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, tu: &mut Tune, accent: Color32, wide: bool) {
    let hint = state.settings.hint_display;
    let plays = parts_playing(state);
    let levels = state.music_dev.readout.score.map(|s| s.roles).unwrap_or([0.0; ROLES]);
    widgets::subsection_header(ui, theme, accent, t("music_page.mixer"), t("music_page.mixer_note"));
    widgets::card(ui, theme, |ui| {
        let fits = ((ui.available_width() + theme.spacing_xs) / (STRIP_W + theme.spacing_xs)) as usize >= ROLES;
        if wide && fits {
            ui.horizontal_top(|ui| {
                for k in 0..ROLES {
                    ui.allocate_ui_with_layout(Vec2::new(STRIP_W, FADER_H + 120.0), egui::Layout::top_down(egui::Align::Center), |ui| {
                        ui.set_width(STRIP_W);
                        strip_column(ui, theme, tu, k, plays[k], levels[k]);
                    });
                    ui.add_space(theme.spacing_xs);
                }
            });
        } else {
            for k in 0..ROLES {
                ui.horizontal(|ui| strip_row(ui, theme, tu, k, plays[k], levels[k]));
            }
        }
        widgets::setting_hint(ui, theme, hint, t("music_page.mixer_hint"));
        if (tu.level_db != [0.0; ROLES] || tu.mute != 0 || tu.solo != 0) && ui.button(t("music_page.mixer_reset")).hint("music_page.mixer_reset_hint").clicked() {
            tu.level_db = [0.0; ROLES];
            tu.mute = 0;
            tu.solo = 0;
        }
    });
}

/// The part's name in its colour (greyed when the piece does not play it): a narrow page's row.
fn name(ui: &mut egui::Ui, theme: &Theme, k: usize, plays: bool) {
    let colour = if plays { role_colour(theme, k as u8) } else { theme.text_muted() };
    let r = ui.label(RichText::new(role_words(k)).size(theme.font_size_small).strong().color(colour));
    if !plays {
        r.hint("music_page.part_silent_hint");
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

/// A wide page's strip: the level, the meter beside a vertical fader named for its part (the name
/// under the fader, so a screen reader hears which part it moves), Mute and Solo.
fn strip_column(ui: &mut egui::Ui, theme: &Theme, tu: &mut Tune, k: usize, plays: bool, level: f32) {
    let colour = if plays { role_colour(theme, k as u8) } else { theme.text_muted() };
    ui.label(RichText::new(db_words(tu.level_db[k])).size(theme.font_size_small).color(theme.text_secondary()));
    ui.horizontal(|ui| {
        ui.spacing_mut().slider_width = FADER_H;
        ui.spacing_mut().item_spacing.x = 3.0;
        meter(ui, theme, Vec2::new(METER_W, FADER_H), level, colour, true);
        // A copy, written back only when the person moves it (a slider holds a value to its range and
        // steps when drawn, and that alone must never count as choosing).
        let mut db = tu.level_db[k];
        let name = RichText::new(role_words(k)).size(theme.font_size_small).strong().color(colour);
        if widgets::slider(ui, theme, egui::Slider::new(&mut db, FADER_DB.0..=FADER_DB.1).vertical().step_by(0.5).show_value(false).text(name)).hint("music_page.level_hint").changed() {
            tu.level_db[k] = db;
        }
    });
    mute_solo(ui, theme, tu, k);
}

/// A narrow page's strip: one row of the name, a fader, the meter, Mute and Solo.
fn strip_row(ui: &mut egui::Ui, theme: &Theme, tu: &mut Tune, k: usize, plays: bool, level: f32) {
    let colour = if plays { role_colour(theme, k as u8) } else { theme.text_muted() };
    ui.allocate_ui_with_layout(Vec2::new(84.0, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.set_width(84.0);
        name(ui, theme, k, plays);
    });
    let mut db = tu.level_db[k];
    ui.spacing_mut().slider_width = (ui.available_width() - 210.0).clamp(80.0, 260.0);
    let words = RichText::new(db_words(tu.level_db[k])).size(theme.font_size_small).color(theme.text_secondary());
    if widgets::slider(ui, theme, egui::Slider::new(&mut db, FADER_DB.0..=FADER_DB.1).step_by(0.5).show_value(false).text(words)).hint("music_page.level_hint").changed() {
        tu.level_db[k] = db;
    }
    meter(ui, theme, Vec2::new(40.0, METER_W), level, colour, false);
    mute_solo(ui, theme, tu, k);
}
