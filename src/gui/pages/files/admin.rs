//! The Files page's Admin tab (increment 2 of docs/design/files-showcase.md, 2026-10-11; on the
//! Files page since Media round 5, 2026-10-12, docs/design/media-round-5-plan.md 3.9): everything
//! an admin sets about the server's Files page, in one place. It was Server Settings > ADMIN >
//! Media until round 5; Server Settings now keeps one line, [`POINTER_LINE`], with "Open them",
//! which lands here (media.rs `open_admin_tab`). Only admins see the tab (media.rs `draw_body`).
//!
//! - Who sees it: Public or Members only, with the relay's own line under the switch saying what
//!   a member is (`media_members_note`: everyone signed in to this server).
//! - The Media space (its budget in MB) and each person's share of it, and "Public visitors get
//!   previews only". These four are server settings: they edit the page's one working copy and
//!   go with the same `server_settings_update` frame the other Save buttons send.
//!   Like the Server policy section, it says when the server's settings changed under unsaved
//!   edits, since Save sends the whole copy.
//! - The categories: add, rename, reorder, remove (General stays: it holds the items of a
//!   removed category), saved as one whole list (`POST /api/files/categories`). Unsaved edits
//!   stay until Save or Discard: a Refresh never throws them away.
//! - The category suggestions waiting, each with who suggested it, Accept and Decline.
//! - The meter, "12.4 GB of 50 GB used" with a bar and a line for what the server keeps, and the
//!   prune lists (largest, oldest, least viewed, waiting to be published), each row with Remove,
//!   and an item's row with Keep on the server (a kept item is in no prune list).
//! - The copyright reports waiting (2026-10-13, files/takedown.rs `draw_admin_section`): what each
//!   person wrote, a note for the public log, and Remove item or Keep.
//!
//! The requests and their answers are the Files page's (pages/files.rs `spawn`, `poll`), so an
//! answer lands whichever tab is open.

use egui::RichText;

use crate::gui::pages::files::{self as page, api, Job};
use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};

/// The general category's id (the relay's `GENERAL_CATEGORY_ID`): it cannot be removed.
const GENERAL: i64 = 1;

/// The prune lists' names, in the order of `FilesAdmin::prune_tab`.
const PRUNE_TABS: [&str; 4] = ["Largest", "Oldest", "Least viewed", "Waiting to be published"];

/// What Server Settings > ADMIN says where the section went, beside its "Open them" (the
/// website's /admin says the same words: web/pages/admin.html).
pub(crate) const POINTER_LINE: &str = "This server's Files page settings, categories and space are on the Files page, under Admin.";

/// The section. Asks the server for the categories, suggestions and usage the first time it is
/// drawn for a server, and reads their answers every frame. The tab it sits under names it, so it
/// has no heading of its own (the website's Admin tab has none either).
pub(crate) fn draw_admin(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    page::sync_server(state);
    page::poll(ui.ctx(), state);
    if !state.files.admin.requested && !page::server_here(state).is_empty() && page::signed_in_here(state) {
        page::refresh_admin(state);
    }
    widgets::body_hint(
        ui,
        theme,
        "The Files page shows the pictures, videos, models and files people chose to put there. Here you choose who sees it, how much space it has, its categories, and what to prune.",
    );
    ui.add_space(theme.spacing_sm);
    draw_settings(ui, theme, state);
    ui.add_space(theme.spacing_md);
    draw_categories(ui, theme, state);
    ui.add_space(theme.spacing_md);
    draw_suggestions(ui, theme, state);
    ui.add_space(theme.spacing_md);
    draw_meter(ui, theme, state);
    // The copyright reports waiting for a decision (2026-10-13, takedown.rs), on the same tab as the
    // website has them. The section checks for itself that this person is an admin here.
    ui.add_space(theme.spacing_md);
    super::takedown::draw_admin_section(ui, theme, state);
    if !state.files.admin.status.is_empty() {
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(&state.files.admin.status).size(theme.font_size_small).color(theme.text_secondary()));
    }
}

/// Visibility, the budget, each person's share and previews: the server settings' working copy.
fn draw_settings(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let Some(cached) = state.server_settings.clone() else {
        widgets::body_hint(ui, theme, "Waiting for this server's settings. Nothing here can be changed until they arrive.");
        return;
    };
    let mut draft = crate::gui::connections::page_draft(state.server_settings_draft.as_ref(), Some(&cached)).unwrap_or_else(|| cached.clone());
    ui.horizontal(|ui| {
        ui.label(RichText::new("Who sees the Files page").size(theme.font_size_body).color(theme.text_primary()));
        ui.radio_value(&mut draft.media_visibility, "public".to_string(), "Public");
        ui.radio_value(&mut draft.media_visibility, "members".to_string(), "Members only");
    });
    // The relay's own words (it sends them with the settings), never this app's.
    let note = if cached.media_members_note.trim().is_empty() { crate::relay::storage::MEDIA_MEMBERS_NOTE } else { cached.media_members_note.as_str() };
    ui.label(RichText::new(note).size(theme.font_size_small).color(theme.text_muted()));
    ui.add_space(theme.spacing_xs);
    let (min, max) = crate::relay::storage::MEDIA_MB_RANGE;
    widgets::form_row_wide(ui, theme, "File space (MB)", |ui| {
        crate::gui::pages::server_settings::int_input(ui, &mut draft.media_budget_mb, min, max);
    });
    widgets::form_row_wide(ui, theme, "Each person's share (MB)", |ui| {
        crate::gui::pages::server_settings::int_input(ui, &mut draft.media_per_person_mb, min, max);
    });
    ui.checkbox(&mut draft.media_public_previews_only, "Public visitors get previews only")
        .on_hover_text("Members get the originals and downloads. The previews themselves come in a later version.");
    let dirty = draft != cached;
    let mut save = false;
    // As in Server Settings' Server policy: Save sends the whole copy (Server Settings' unsaved
    // edits too: one working copy), so say when the server's changed under it.
    crate::gui::pages::server_settings::draw_changed_underneath_note(ui, theme, state, dirty);
    ui.horizontal(|ui| {
        ui.add_enabled_ui(dirty, |ui| {
            save = widgets::Button::primary("Save Files page settings")
                .tooltip("Sends this server's settings, with any unsaved edits on the Server Settings page too.")
                .show(ui, theme);
        });
        if dirty {
            ui.label(RichText::new("Unsaved changes.").size(theme.font_size_small).color(theme.warning()));
        }
    });
    state.server_settings_draft = Some(draft.clone());
    if save {
        crate::gui::pages::server_settings::send_server_settings_update(state, &draft);
        state.discard_server_settings_draft();
        let now = ui.ctx().input(|i| i.time);
        state.toast("Files page settings sent to the relay", crate::gui::ToastKind::Success, now);
    }
}

/// The category list: rename and describe in place, Up and Down, Remove (not General), a new one,
/// and Save, which sends the whole list.
fn draw_categories(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Categories").size(theme.font_size_body).color(theme.text_primary()).strong());
    widgets::body_hint(ui, theme, "Every item has one. General stays: it holds the items of a category you remove. It can be renamed.");
    let a = &mut state.files.admin;
    let mut move_up = None;
    let mut move_down = None;
    let mut remove = None;
    let count = a.cats.len();
    for (i, c) in a.cats.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            let n = ui.add(egui::TextEdit::singleline(&mut c.name).desired_width(170.0).char_limit(60));
            let d = ui.add(egui::TextEdit::singleline(&mut c.description).desired_width(260.0).hint_text("Description (optional)").char_limit(300));
            if n.changed() || d.changed() {
                a.cats_dirty = true;
            }
            let small = widgets::ButtonSize::Small;
            if i > 0 && widgets::Button::ghost("Up").size(small).show(ui, theme) {
                move_up = Some(i);
            }
            if i + 1 < count && widgets::Button::ghost("Down").size(small).show(ui, theme) {
                move_down = Some(i);
            }
            if c.id == Some(GENERAL) {
                ui.label(RichText::new("stays").size(theme.font_size_small).color(theme.text_muted()));
            } else if widgets::Button::danger("Remove").size(small).show(ui, theme) {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = move_up {
        a.cats.swap(i, i - 1);
        a.cats_dirty = true;
    }
    if let Some(i) = move_down {
        a.cats.swap(i, i + 1);
        a.cats_dirty = true;
    }
    if let Some(i) = remove {
        a.cats.remove(i);
        a.cats_dirty = true;
    }
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut a.new_cat).desired_width(170.0).hint_text("New category").char_limit(60));
        let name = a.new_cat.trim().to_string();
        if widgets::Button::secondary("Add").show(ui, theme) && !name.is_empty() {
            a.cats.push(api::CategoryEdit { id: None, name, description: String::new() });
            a.new_cat.clear();
            a.cats_dirty = true;
        }
    });
    let dirty = a.cats_dirty;
    let mut save = false;
    let mut discard = false;
    ui.horizontal(|ui| {
        ui.add_enabled_ui(dirty, |ui| {
            save = widgets::Button::primary("Save categories").tooltip("Removing a category moves its items to General.").show(ui, theme);
            discard = widgets::Button::secondary("Discard changes").tooltip("Go back to the server's list.").show(ui, theme);
        });
        if dirty {
            ui.label(RichText::new("Unsaved changes.").size(theme.font_size_small).color(theme.warning()));
        }
    });
    if discard {
        // Back to the list as the server last sent it, and asked for again.
        state.files.admin.cats =
            state.files.list.categories.iter().map(|c| api::CategoryEdit { id: Some(c.id), name: c.name.clone(), description: c.description.clone() }).collect();
        state.files.admin.cats_dirty = false;
        page::refresh_admin(state);
    }
    if save {
        if let Some(signer) = page::signer_or_say(state) {
            let req = api::categories_request(&signer, &state.files.admin.cats);
            page::spawn(state, Job::SaveCategories, req);
        }
    }
}

/// The suggestions still waiting, with who suggested each, and Accept and Decline.
fn draw_suggestions(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Category suggestions").size(theme.font_size_body).color(theme.text_primary()).strong());
    let open: Vec<api::Suggestion> = state.files.admin.suggestions.iter().filter(|s| s.state == "open").cloned().collect();
    if open.is_empty() {
        widgets::body_hint(ui, theme, "None waiting.");
        return;
    }
    let mut decide: Option<(i64, bool)> = None;
    for s in &open {
        ui.horizontal_wrapped(|ui| {
            let who = if s.suggester_name.trim().is_empty() { "a member" } else { s.suggester_name.trim() };
            ui.label(RichText::new(format!("\"{}\", suggested by {who}", s.name)).size(theme.font_size_small).color(theme.text_primary()));
            if !s.note.trim().is_empty() {
                ui.label(RichText::new(s.note.trim()).size(theme.font_size_small).color(theme.text_muted()));
            }
            let small = widgets::ButtonSize::Small;
            if widgets::Button::primary("Accept").size(small).show(ui, theme) {
                decide = Some((s.id, true));
            }
            if widgets::Button::secondary("Decline").size(small).show(ui, theme) {
                decide = Some((s.id, false));
            }
        });
    }
    if let Some((id, accept)) = decide {
        if let Some(signer) = page::signer_or_say(state) {
            page::spawn(state, Job::Decide, api::decide_request(&signer, id, accept));
        }
    }
}

/// The meter and the prune lists.
fn draw_meter(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("File space").size(theme.font_size_body).color(theme.text_primary()).strong());
        if widgets::Button::ghost("Refresh").size(widgets::ButtonSize::Small).show(ui, theme) {
            page::refresh_admin(state);
        }
    });
    let Some(usage) = state.files.admin.usage.clone() else {
        widgets::body_hint(ui, theme, if page::signed_in_here(state) { "Asking the server..." } else { "Sign in to this server to see how its file space is used." });
        return;
    };
    let words = api::meter_words(usage.used_bytes, usage.budget_bytes);
    let fraction = if usage.budget_bytes > 0 { usage.used_bytes as f32 / usage.budget_bytes as f32 } else { 0.0 };
    ui.label(RichText::new(&words).size(theme.font_size_body).color(if fraction >= 0.9 { theme.warning() } else { theme.text_primary() }));
    widgets::progress_bar(ui, theme, fraction, None);
    if usage.kept_bytes > 0 {
        ui.label(RichText::new(api::kept_words(usage.kept_bytes)).size(theme.font_size_small).color(theme.text_secondary()));
    }
    ui.label(
        RichText::new(format!("{} items. Each person may use up to {}.", usage.items, api::size_words(usage.per_person_bytes)))
            .size(theme.font_size_small)
            .color(theme.text_muted()),
    );
    ui.add_space(theme.spacing_xs);
    ui.horizontal(|ui| {
        for (i, name) in PRUNE_TABS.iter().enumerate() {
            ui.selectable_value(&mut state.files.admin.prune_tab, i, *name);
        }
    });
    widgets::body_hint(ui, theme, "Items the server keeps are in none of these lists.");
    let mut asked: Option<api::RemoveTarget> = None;
    let mut keep: Option<api::Item> = None;
    let confirm = state.files.admin.confirm.clone();
    let mut confirmed = false;
    let mut cancelled = false;
    let mut row = |ui: &mut egui::Ui, target: api::RemoveTarget, item: Option<&api::Item>, text: String| {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(text).size(theme.font_size_small).color(theme.text_primary()));
            let small = widgets::ButtonSize::Small;
            if confirm.as_ref() == Some(&target) {
                ui.label(RichText::new("Remove it?").size(theme.font_size_small).color(theme.warning()));
                confirmed |= widgets::Button::danger("Yes, remove").size(small).show(ui, theme);
                cancelled |= widgets::Button::secondary("Cancel").size(small).show(ui, theme);
                return;
            }
            if widgets::Button::danger("Remove").size(small).show(ui, theme) {
                asked = Some(target);
            }
            if let Some(item) = item {
                if widgets::Button::secondary(page::KEEP_LABEL).size(small).tooltip(page::KEEP_LABEL_TIP).show(ui, theme) {
                    keep = Some(item.clone());
                }
            }
        });
    };
    let items = match state.files.admin.prune_tab {
        0 => &usage.largest,
        1 => &usage.oldest,
        2 => &usage.least_viewed,
        _ => &Vec::new(),
    };
    if state.files.admin.prune_tab == 3 {
        if usage.waiting.is_empty() {
            widgets::body_hint(ui, theme, "No uploads are waiting to be published.");
        }
        for w in &usage.waiting {
            let who = if w.uploader_name.trim().is_empty() { "a member" } else { w.uploader_name.trim() };
            row(ui, api::RemoveTarget::Upload(w.filename.clone()), None, format!("{} by {who}, {}, deleted after a day unless published", w.shown_name(), api::size_words(w.size_bytes)));
        }
    } else {
        if items.is_empty() {
            widgets::body_hint(ui, theme, "Nothing on the Files page yet.");
        }
        for item in items {
            let text = format!("{} by {}, {}, {} views", item.display_title(), item.by_name(), api::size_words(item.size_bytes), item.views);
            match item.reference() {
                Ok(r) => row(ui, api::RemoveTarget::Item(r), Some(item), text),
                // The server did not name its file: nothing can be asked of it from here.
                Err(_) => {
                    ui.label(RichText::new(text).size(theme.font_size_small).color(theme.text_muted()));
                }
            }
        }
    }
    if let Some(target) = asked {
        state.files.admin.confirm = Some(target);
    }
    if cancelled {
        state.files.admin.confirm = None;
    }
    if confirmed {
        if let (Some(target), Some(signer)) = (state.files.admin.confirm.take(), page::signer_or_say(state)) {
            let id = if let api::RemoveTarget::Item(r) = &target { Some(r.id) } else { None };
            page::spawn(state, Job::Remove { id, from_admin: true }, api::remove_request(&signer, &target));
        }
    }
    if let Some(item) = keep {
        if let (Ok(r), Some(signer)) = (item.reference(), page::signer_or_say(state)) {
            page::spawn(state, Job::Keep { id: item.id, on: true, from_admin: true }, api::keep_request(&signer, &r, true));
        }
    }
}

#[cfg(test)]
#[path = "admin_tests.rs"]
pub(crate) mod tests;
