//! The Media page's filters row (category, kind, person, Sort, My picks, the search box), the
//! picked category's description under it, the featured shelf's heading, and the admins'
//! three-way Feature choice. The row moved here from tiles.rs in Media round 5's Wave 0; lane 1C
//! (2026-10-12, docs/design/media-round-5-plan.md 3.9) added Sort, the description, the shelf's
//! heading and the Feature choice.
//!
//! THE WORDS ARE THE WEBSITE'S (web/pages/media-app.js `SORTS` and `FEATURE_CHOICES`, lane 1G),
//! so the two apps say the same thing: Sort is "Newest", "Most picked" or "Most downloaded"; the
//! shelf reads "Featured", or "Featured in Fire flow arts" when a category is picked; an item is
//! "Not featured", "Featured" or "Featured in its category".
//!
//! A change of any filter asks for the first page again (media.rs `reload_list`), with the sort
//! and the category in the request; a change of category asks for that category's shelf too
//! (media.rs `reload_featured`), since the shelf follows the picked category.

use super::api::{self, Featured, Item, Sort};
use super::{kinds, signed_in_here, Job};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// The orders the list can be in, in the words both apps use.
pub(crate) const SORTS: [(Sort, &str); 3] = [(Sort::New, "Newest"), (Sort::Picks, "Most picked"), (Sort::Downloads, "Most downloaded")];

/// The words for an order.
pub(crate) fn sort_words(sort: Sort) -> &'static str {
    SORTS.iter().find(|(s, _)| *s == sort).map(|(_, w)| *w).unwrap_or("Newest")
}

/// Where an item can be featured, in the words both apps use: on no shelf, on the main shelf (and
/// its category's), or on its category's shelf alone.
pub(crate) const FEATURE_CHOICES: [(Featured, &str); 3] =
    [(Featured::None, "Not featured"), (Featured::Main, "Featured"), (Featured::Category, "Featured in its category")];

/// The words for where an item is featured (the Feature choice, and the card's badge).
pub(crate) fn feature_words(featured: Featured) -> &'static str {
    FEATURE_CHOICES.iter().find(|(f, _)| *f == featured).map(|(_, w)| *w).unwrap_or("Not featured")
}

/// What the Feature choice says when hovered.
pub(crate) const FEATURE_TIP: &str =
    "Featured items lead the Media page. \"Featured in its category\" shows it only on its category's shelf, when someone picks that category.";

/// The filters row: category, kind, person, Sort, My picks, and the search box; under it, the
/// picked category's description.
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
        // The order (round 5). Newest is the server's own order and is not sent; the other two
        // are, and a new order starts the list afresh from its first page (no cursor).
        egui::ComboBox::from_id_salt("media_sort")
            .selected_text(sort_words(m.filters.sort))
            .show_ui(ui, |ui| {
                for (sort, words) in SORTS {
                    ui.selectable_value(&mut m.filters.sort, sort, words);
                }
            })
            .response
            .on_hover_text("The order of the list.");
        if signed_in {
            ui.checkbox(&mut m.filters.my_picks, "My picks").on_hover_text("Only the items you picked.");
        }
        let resp = ui.add(egui::TextEdit::singleline(&mut m.search).hint_text("Search titles").desired_width(180.0));
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if enter || widgets::Button::secondary("Search").show(ui, theme) {
            m.filters.q = m.search.trim().to_string();
        }
    });
    // The picked category's description, under the filters, in muted words (3.9): what an admin
    // wrote for it on the Admin tab. Nothing when no category is picked or it has none.
    let description = category_description(state);
    if !description.is_empty() {
        widgets::body_hint(ui, theme, &description);
    }
    if state.media.filters != before {
        let category_changed = state.media.filters.category != before.category;
        super::reload_list(state);
        // The shelf follows the picked category: its own shelf, or the main one again.
        if category_changed {
            super::reload_featured(state);
        }
    }
}

/// The picked category's description, trimmed ('' when no category is picked, the category is
/// not known, or it has none).
pub(crate) fn category_description(state: &GuiState) -> String {
    let Some(id) = state.media.filters.category else { return String::new() };
    state.media.list.categories.iter().find(|c| c.id == id).map(|c| c.description.trim().to_string()).unwrap_or_default()
}

/// The featured shelf's heading: "Featured" for the main shelf, "Featured in Fire flow arts" for
/// a category's (the one its items were asked for, so the heading never runs ahead of them).
pub(crate) fn shelf_heading(state: &GuiState) -> String {
    let name = state.media.shelf_of.and_then(|id| state.media.list.categories.iter().find(|c| c.id == id)).map(|c| c.name.trim()).filter(|n| !n.is_empty());
    match name {
        Some(name) => format!("Featured in {name}"),
        None => "Featured".to_string(),
    }
}

/// The admins' Feature choice for one item (round 5, 3.9 and 3.4): "Not featured", "Featured"
/// (the main shelf, and its category's) or "Featured in its category" (that shelf alone). A new
/// choice is sent at once (`POST /api/media/feature`, signed over `media_feature:<id>:<where>`),
/// and the answer updates the item and asks for the shelf again (media.rs `on_done`).
///
/// For lane 1D: the tile (tiles.rs) and the item's page (item_view.rs) call this in place of the
/// old Feature / Unfeature button. It draws nothing for anyone who is not an admin signed in to
/// this server. Returns true when a new choice was sent.
pub(crate) fn draw_feature_choice(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, item: &Item) -> bool {
    if !signed_in_here(state) || !super::is_admin(state) {
        return false;
    }
    let mut to = item.featured;
    // The id is the item's, under the parent's own id, so the same item on the shelf and in the
    // grid keeps two separate choices.
    egui::ComboBox::from_id_salt(("media_feature", item.id))
        .selected_text(egui::RichText::new(feature_words(to)).size(theme.font_size_small))
        .show_ui(ui, |ui| {
            for (place, words) in FEATURE_CHOICES {
                ui.selectable_value(&mut to, place, words);
            }
        })
        .response
        .on_hover_text(FEATURE_TIP);
    if to == item.featured {
        return false;
    }
    send_feature(state, item, to);
    true
}

/// Feature `item` at `to`: the signed request, or the reason it cannot go said on the page.
pub(crate) fn send_feature(state: &mut GuiState, item: &Item, to: Featured) {
    let r = match item.reference() {
        Ok(r) => r,
        Err(words) => {
            state.media.status = words;
            return;
        }
    };
    if let Some(signer) = super::signer_or_say(state) {
        super::spawn(state, Job::Feature { id: item.id, to }, api::feature_request(&signer, &r, to));
    }
}
