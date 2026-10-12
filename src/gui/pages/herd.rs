//! Home > Animals (2026-10-12, first-hour audit S6): every farm animal the home keeps, how it
//! is (fed, watered, hungry, sick) and what it needs a day, in plain words. The rows come from
//! `systems::animal_care::herd_rows`, which lib.rs puts in `GuiState::herd` each frame; the
//! card is hidden when the home keeps no animals that need care.

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// What the card says before its rows.
const INTRO: &str = "Your animals eat from home storage (the Barn) and drink from the home's water \
                     tanks every day. Keep feed in storage and water in the tanks, and collect what \
                     they make with E when you stand by them.";

/// The Animals card, on the Home page under the Household card (homes.rs `draw`).
pub fn draw_card(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if state.herd.is_empty() {
        return;
    }
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(ui.available_width());
        if widgets::section_disclosure(ui, theme, ("homes_sec", "animals"), "Animals", None) {
            widgets::body_hint(ui, theme, INTRO);
            ui.add_space(theme.spacing_xs);
            for row in &state.herd {
                ui.label(RichText::new(&row.title).size(theme.font_size_body).color(theme.text_primary()).strong());
                let colour = if row.alert { theme.warning() } else { theme.text_secondary() };
                ui.label(RichText::new(&row.status).size(theme.font_size_small).color(colour));
                if !row.needs.is_empty() {
                    ui.label(RichText::new(&row.needs).size(theme.font_size_small).color(theme.text_muted()));
                }
                ui.add_space(theme.spacing_xs);
            }
        }
    });
    ui.add_space(theme.spacing_sm);
}
