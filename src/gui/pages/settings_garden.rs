//! Settings > Gameplay > Garden updates (2026-10-31, docs/design/farm-clock.md rung 2,
//! `systems::farming::clock`).
//!
//! The operator, 2026-10-30: "What if we only update plant data once every hour or 6 or 24 or
//! whatever we configure? I imagine this would help MMO servers." The farm now advances each grow
//! area once an interval, the work spread over the interval's frames; this is where the interval
//! is chosen. Every second is the default; every frame is the farm as it ran before. The choices
//! are `clock::CHOICES` (game seconds and their words), so the list lives in one place.
//!
//! What the person is told, plainly (the hint): a longer choice is less work for the computer,
//! and nothing they see or do is behind for it, because what reads the garden brings it up to now
//! first (the plants on board and the Garden page within a second, a harvest, a watering and the
//! save exactly).

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::widgets::Hint;
use crate::gui::{widgets, GuiState, HintDisplay};
use crate::i18n::t;
use crate::systems::farming::clock;

/// Draw the Garden updates choice. Marks the settings dirty when it changes.
pub fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: HintDisplay) {
    ui.label(RichText::new(t("settings.garden_updates.title")).color(theme.text_secondary()).strong());
    ui.add_space(theme.spacing_xs);
    widgets::setting_hint(ui, theme, hint, t("settings.garden_updates.hint"));
    ui.horizontal_wrapped(|ui| {
        for (secs, key) in clock::CHOICES {
            let selected = (f64::from(state.settings.garden_update_s) - secs).abs() < 1e-3;
            let pressed = ui.radio(selected, RichText::new(t(key)).color(theme.text_primary())).hint("settings.garden_updates.choice_hint").clicked();
            if pressed && !selected {
                state.settings.garden_update_s = secs as f32;
                state.settings_dirty = true;
            }
        }
    });
    ui.add_space(theme.spacing_lg);
}

#[cfg(test)]
mod tests {
    /// Every choice the page offers has its words in data/i18n/en.json, and the default is one of
    /// them: a choice whose key English lacked would show its key on the page.
    #[test]
    fn every_choice_has_its_words_and_the_default_is_offered() {
        let en: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/i18n/en.json")).unwrap()).unwrap();
        for (_, key) in crate::systems::farming::clock::CHOICES {
            let mut v = &en;
            for part in key.split('.') {
                v = &v[part];
            }
            assert!(v.as_str().is_some_and(|s| !s.is_empty()), "{key} has no words in en.json");
        }
        assert!(crate::systems::farming::clock::CHOICES.iter().any(|(s, _)| *s == crate::systems::farming::clock::DEFAULT_INTERVAL_S));
    }
}
