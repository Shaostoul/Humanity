//! The Media page's filters row (category, kind, person, My picks, the search box), moved here
//! from tiles.rs unchanged (Media round 5, Wave 0, 2026-10-12) so the round's list lane (1C) adds
//! Sort, the picked category's description and its shelf's heading in a file of their own
//! (docs/design/media-round-5-plan.md 3.9).

use super::{kinds, signed_in_here};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// The filters row: category, kind, person, My picks, and the search box.
pub(crate) fn draw_filters(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let before = state.media.filters.clone();
    let signed_in = signed_in_here(state);
    ui.horizontal_wrapped(|ui| {
        let m = &mut state.media;
        let cat_name = m.filters.category.and_then(|c| m.list.categories.iter().find(|x| x.id == c)).map(|c| c.name.clone()).unwrap_or_else(|| "All categories".into());
        egui::ComboBox::from_id_salt("media_category").selected_text(cat_name).show_ui(ui, |ui| {
            ui.selectable_value(&mut m.filters.category, None, "All categories");
            for c in &m.list.categories {
                ui.selectable_value(&mut m.filters.category, Some(c.id), &c.name);
            }
        });
        let kind_name = m.filters.kind.as_deref().and_then(kinds::kind).map(|k| k.label.clone()).unwrap_or_else(|| "All kinds".into());
        egui::ComboBox::from_id_salt("media_kind").selected_text(kind_name).show_ui(ui, |ui| {
            ui.selectable_value(&mut m.filters.kind, None, "All kinds");
            for k in kinds::kinds() {
                ui.selectable_value(&mut m.filters.kind, Some(k.id.clone()), &k.label);
            }
        });
        let person = m.filters.by.as_ref().and_then(|k| m.people.iter().find(|(key, _)| key == k)).map(|(_, n)| n.clone()).unwrap_or_else(|| "Everyone".into());
        egui::ComboBox::from_id_salt("media_person").selected_text(person).show_ui(ui, |ui| {
            ui.selectable_value(&mut m.filters.by, None, "Everyone");
            for (key, name) in &m.people {
                ui.selectable_value(&mut m.filters.by, Some(key.clone()), name);
            }
        });
        if signed_in {
            ui.checkbox(&mut m.filters.my_picks, "My picks").on_hover_text("Only the items you picked.");
        }
        let resp = ui.add(egui::TextEdit::singleline(&mut m.search).hint_text("Search titles").desired_width(180.0));
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if enter || widgets::Button::secondary("Search").show(ui, theme) {
            m.filters.q = m.search.trim().to_string();
        }
    });
    if state.media.filters != before {
        super::reload_list(state);
    }
}
