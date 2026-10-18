//! An item's Edit window (Media round 5, lane 1D; docs/design/media-round-5-plan.md 3.9, with the
//! operator's answers in its section 10 and the orchestrator's decision 1 in section 9). It opens
//! over the item's page (item_view.rs), from Edit there or on a tile, for the item's uploader and
//! for admins:
//!
//! - the title, the description, the category (its description under it), the content labels;
//! - who can see it, under the same rules and words as Add to Media (a file already public where
//!   it was shared stays public; a members-only Media page makes everything members only);
//! - the licence, with the server's own line for it, and "Offer this for the game", shown only
//!   with a licence that allows it (CC0 and CC BY 4.0: the server's `game`). The editor states
//!   the facts plainly before Save: a Creative Commons licence cannot be taken back for copies
//!   already taken (`cc_warning`), and on a members-only item any Creative Commons licence lets
//!   every member who receives it republish it publicly (`cc_members_warning`). An admin editing
//!   someone else's item sees the licence and the tick but cannot change them: "Only Ann can
//!   change these." (a licence is the rights holder's grant);
//! - the hosting paragraph ("Your uploads stay yours."), as the server words it;
//! - the files: Remove on each added file and the cover (the uploader and admins), and, for the
//!   uploader, Add a file and Cover picture. A picked file is the same upload as Add to Media's,
//!   a row under "Uploads in progress", added to this item when it is up instead of published
//!   (uploading.rs `start_attach`).
//!
//! Save sends the edit (`POST /api/media/edit`, every field, signed over the BLAKE3 of the edit
//! exactly as sent: api_items.rs `edit_request`) and, apart from it, the visibility when that
//! changed (`POST /api/media/visibility`). The window closes when both are answered; a refusal is
//! shown in it in the relay's own words. Cancel throws the changes away. Every word about a
//! licence comes from the server's answer (data/media/licences.json on the relay), never from a
//! copy in this app.

use std::collections::BTreeSet;

use egui::RichText;

use super::api::{self, Item, ItemFile, Licences};
use super::api_items::{edit_request, visibility_request, AttachTo, EditFields, FileRole};
use super::item_view::{self, ItemPage};
use super::{signer_or_say, spawn, Job};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::widgets::file_browser::FilePickerState;
use crate::gui::widgets::tint_section::{self, Tint};
use crate::gui::GuiState;

/// A licence and the game tick, as a form shows them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LicenceChoice {
    pub licence: String,
    pub project_use: bool,
}

/// The Edit window's working copy of an item.
#[derive(Debug, Clone)]
pub struct EditForm {
    /// The item as it was when Edit opened: what Save compares with (the visibility is sent only
    /// when it changed), and whose files the window lists (they follow the item as files are added
    /// or taken off: item_view.rs `take_item`).
    pub was: Item,
    pub title: String,
    pub description: String,
    pub category: i64,
    pub labels: BTreeSet<String>,
    pub choice: LicenceChoice,
    pub members_only: bool,
    /// Why the last Save did not go, in the relay's own words.
    pub problem: String,
    /// The file picker for Add a file or Cover picture, while open.
    pub picker: Option<(FileRole, FilePickerState)>,
}

impl EditForm {
    /// The form for `item`, as it is now.
    pub fn of(item: &Item) -> Self {
        EditForm {
            was: item.clone(),
            title: item.title.clone(),
            description: item.description.clone(),
            category: if item.category_id > 0 { item.category_id } else { api::GENERAL_CATEGORY },
            labels: item.labels.iter().cloned().collect(),
            choice: LicenceChoice { licence: item.licence.clone(), project_use: item.project_use },
            members_only: item.members_only(),
            problem: String::new(),
            picker: None,
        }
    }

    /// What the edit sends: every field the form shows. Someone who is not the uploader (an admin)
    /// sends the licence and the tick as they were: only the uploader changes either.
    pub fn fields(&self, own: bool) -> EditFields {
        let choice = if own { self.choice.clone() } else { LicenceChoice { licence: self.was.licence.clone(), project_use: self.was.project_use } };
        EditFields {
            title: self.title.clone(),
            description: self.description.clone(),
            category_id: self.category,
            labels: self.labels.iter().cloned().collect(),
            licence: choice.licence,
            project_use: choice.project_use,
        }
    }
}

/// Open the Edit window for `item`, over its page (opened first when another item's, or none, is).
pub(crate) fn open_edit(state: &mut GuiState, item: &Item) {
    if state.media.item.as_ref().map(|p| p.id) != Some(item.id) {
        item_view::open(state, item);
    }
    if let Some(page) = state.media.item.as_mut() {
        page.edit = Some(EditForm::of(item));
    }
}

/// The licence picker shared by Add to Media and Edit: the licences this server offers, the
/// server's line for the one chosen and a link to its deed, "Offer this for the game" only with a
/// licence that allows it (and turned off with one that does not), and the plain facts before a
/// Creative Commons licence is saved: it cannot be taken back for copies already taken, and on a
/// `members` item every member who receives it may republish it publicly. `was` is the licence the
/// item has now (the fact is stated when leaving a Creative Commons licence too). With
/// `owner_only` (an admin on someone else's item) the choice is shown and cannot be changed, with
/// "Only {name} can change these."
pub(crate) fn draw_licence_choice(ui: &mut egui::Ui, theme: &Theme, licences: &Licences, choice: &mut LicenceChoice, members: bool, was: Option<&str>, owner_only: Option<&str>) {
    if choice.licence.trim().is_empty() {
        choice.licence = licences.default.clone();
    }
    let name_of = |id: &str| licences.get(id).map(|l| l.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| id.to_string());
    widgets::form_row(ui, theme, "Licence", |ui| match owner_only {
        Some(_) => {
            ui.label(RichText::new(name_of(&choice.licence)).size(theme.font_size_body).color(theme.text_primary()));
        }
        None => {
            egui::ComboBox::from_id_salt("media_licence_choice").selected_text(name_of(&choice.licence)).show_ui(ui, |ui| {
                for l in &licences.offered {
                    let hover = if l.official_name.is_empty() { l.line.clone() } else { format!("{}. {}", l.official_name, l.line) };
                    if ui.selectable_label(l.id == choice.licence, &l.name).on_hover_text(hover).clicked() {
                        choice.licence = l.id.clone();
                    }
                }
            });
        }
    });
    let chosen = licences.get(&choice.licence);
    if let Some(l) = chosen {
        if !l.line.trim().is_empty() {
            widgets::body_hint(ui, theme, &l.line);
        }
        if l.url.starts_with("https://") {
            ui.hyperlink_to(RichText::new(format!("Read {} in full", l.name)).size(theme.font_size_small), &l.url);
        }
    }
    // The game tick: only with a licence the server marks `game`; any other turns it off.
    let game = chosen.is_some_and(|l| l.game);
    if !game {
        choice.project_use = false;
    }
    if game && !licences.game_label.trim().is_empty() {
        ui.add_enabled_ui(owner_only.is_none(), |ui| {
            let tick = ui.checkbox(&mut choice.project_use, licences.game_label.as_str());
            if !licences.game_line.trim().is_empty() {
                tick.on_hover_text(&licences.game_line);
            }
        });
    }
    if let Some(name) = owner_only {
        widgets::body_hint(ui, theme, &format!("Only {name} can change these."));
        return;
    }
    // The facts, stated plainly before Save (the operator's answer of 2026-10-12).
    let cc = chosen.is_some_and(|l| l.cc);
    let leaving_cc = was.is_some_and(|w| w != choice.licence && licences.get(w).is_some_and(|l| l.cc));
    if (cc || leaving_cc) && !licences.cc_warning.trim().is_empty() {
        ui.label(RichText::new(&licences.cc_warning).size(theme.font_size_small).color(theme.warning()));
    }
    if cc && members && !licences.cc_members_warning.trim().is_empty() {
        ui.label(RichText::new(&licences.cc_members_warning).size(theme.font_size_small).color(theme.warning()));
    }
}

/// The hosting paragraph ("Your uploads stay yours." and what putting a file on this server lets
/// its people do), beside Add to Media and on the edit screen, in the server's own words; nothing
/// when the server sent none.
pub(crate) fn draw_hosting(ui: &mut egui::Ui, theme: &Theme, licences: &Licences) {
    if licences.hosting_heading.trim().is_empty() && licences.hosting_text.trim().is_empty() {
        return;
    }
    ui.add_space(theme.spacing_xs);
    if !licences.hosting_heading.trim().is_empty() {
        ui.label(RichText::new(&licences.hosting_heading).size(theme.font_size_small).color(theme.text_primary()).strong());
    }
    if !licences.hosting_text.trim().is_empty() {
        widgets::body_hint(ui, theme, &licences.hosting_text);
    }
}

/// The picture types a cover can be (the relay: "A cover must be a picture: PNG, JPEG, GIF or WebP.").
pub const COVER_EXTS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

/// What the Edit window's buttons asked for, done once it is drawn.
#[derive(Default)]
struct Pressed {
    save: bool,
    cancel: bool,
    remove: Option<ItemFile>,
    add: Option<FileRole>,
}

/// The Edit window's id (its dialog, and the dimmed backdrop behind it, `-backdrop`).
pub(crate) const EDIT_DIALOG_ID: &str = "media_item_edit";

/// The Edit window over the item's page, and its file picker, while open.
pub(crate) fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, page: &mut ItemPage) {
    let Some(mut form) = page.edit.take() else { return };
    let item = form.was.clone();
    let own = item_view::own(state, &item);
    let members_server = super::members_only_server(state);
    let categories = state.media.list.categories.clone();
    let labels = state.media.list.labels.clone();
    let licences = state.media.list.licences.clone();
    let saving = item_view::saving(state, item.id);
    let uploads: Vec<(String, String)> = state
        .media
        .uploads
        .iter()
        .filter(|e| e.record.attach.as_ref().is_some_and(|a| a.id == item.id))
        .map(|e| (super::uploading::row_heading(state, &e.record), e.run.words()))
        .collect();
    // Files dropped on the window while this is open are added to the item, as Add a file adds
    // them (gui/drop_files.rs); only its uploader adds files.
    if own {
        crate::gui::drop_files::register(ctx, crate::gui::drop_files::Level::Window, drop_words(&item), |state: &mut GuiState, paths| add_dropped(state, &paths));
    } else {
        crate::gui::drop_files::refuse(ctx, crate::gui::drop_files::Level::Window, format!("Files cannot be dropped here: only {} can add files to this item.", item.by_name()));
    }
    let mut open = true;
    let mut pressed = Pressed::default();
    widgets::dialog_scrolling(ctx, theme, EDIT_DIALOG_ID, &format!("Edit {}", item.display_title()), &mut open, |ui| {
        ui.set_max_width(560.0);
        // Three parts in the chat sidebar's colours, darker (widgets/tint_section.rs, 2026-10-17):
        // what it is, who can see it, its files.
        tint_section::show(ui, theme, Tint::Red, ABOUT_IT, |ui| {
            widgets::form_row(ui, theme, "Title", |ui| {
                ui.add(egui::TextEdit::singleline(&mut form.title).desired_width(320.0).char_limit(120));
            });
            widgets::form_row(ui, theme, "Category", |ui| {
                let shown = categories.iter().find(|c| c.id == form.category).map(|c| c.name.clone()).unwrap_or_else(|| item.category_name.clone());
                egui::ComboBox::from_id_salt("media_edit_category").selected_text(shown).show_ui(ui, |ui| {
                    for c in &categories {
                        if ui.selectable_label(c.id == form.category, &c.name).on_hover_text(&c.description).clicked() {
                            form.category = c.id;
                        }
                    }
                });
            });
            if let Some(about) = categories.iter().find(|c| c.id == form.category).map(|c| c.description.clone()).filter(|d| !d.trim().is_empty()) {
                widgets::body_hint(ui, theme, &about);
            }
            ui.label(RichText::new("Description").size(theme.font_size_small).color(theme.text_secondary()));
            ui.add(egui::TextEdit::multiline(&mut form.description).desired_rows(4).desired_width(520.0).char_limit(2000));
            if !labels.is_empty() {
                ui.label(RichText::new("Content labels: a labelled item stays hidden until someone clicks it.").size(theme.font_size_small).color(theme.text_secondary()));
                ui.horizontal_wrapped(|ui| {
                    for l in &labels {
                        let mut on = form.labels.contains(&l.id);
                        if ui.checkbox(&mut on, &l.label).on_hover_text(&l.desc).changed() {
                            if on {
                                form.labels.insert(l.id.clone());
                            } else {
                                form.labels.remove(&l.id);
                            }
                        }
                    }
                });
            }
        });

        // Who can see it: the same rules and words as Add to Media.
        ui.add_space(theme.spacing_sm);
        tint_section::show(ui, theme, Tint::Green, WHO_CAN_SEE_IT, |ui| {
            if item.keeps_file {
                ui.label(RichText::new(super::add::ALREADY_PUBLIC_LINE).size(theme.font_size_small).color(theme.text_secondary()));
            } else if members_server {
                ui.label(RichText::new(super::add::MEMBERS_SERVER_LINE).size(theme.font_size_small).color(theme.text_secondary()));
            } else {
                ui.horizontal(|ui| {
                    ui.radio_value(&mut form.members_only, false, "Public");
                    ui.radio_value(&mut form.members_only, true, "Members only");
                });
            }

            ui.add_space(theme.spacing_xs);
            let members = members_server || form.members_only;
            let owner_only = (!own).then(|| item.by_name().to_string());
            draw_licence_choice(ui, theme, &licences, &mut form.choice, members, Some(item.licence.as_str()), owner_only.as_deref());
            draw_hosting(ui, theme, &licences);
        });

        ui.add_space(theme.spacing_sm);
        tint_section::show(ui, theme, Tint::Blue, FILES, |ui| draw_files(ui, theme, &item, own, &uploads, &mut pressed));

        if !form.problem.is_empty() {
            ui.label(RichText::new(&form.problem).size(theme.font_size_small).color(theme.danger()));
        }
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!saving, |ui| {
                pressed.save = widgets::Button::primary(SAVE).show(ui, theme);
            });
            if saving {
                ui.label(RichText::new("Saving...").size(theme.font_size_small).color(theme.text_muted()));
            }
            pressed.cancel = widgets::Button::secondary("Cancel").tooltip("Close this without changing anything.").show(ui, theme);
        });
    });

    if let Some(f) = pressed.remove {
        page.confirm_file = Some(f);
    }
    if let Some(role) = pressed.add {
        let exts: &[&str] = if role == FileRole::Cover { COVER_EXTS } else { crate::gui::pages::chat::ATTACH_EXTS };
        let limit = state.media.mine.as_ref().map(|m| m.file_limit_bytes.max(0) as u64).unwrap_or(0);
        let picker = FilePickerState::new(exts, limit).with_pick_verb("Add");
        // Several files at once (2026-10-16); a cover is one picture.
        let picker = if role == FileRole::Cover { picker } else { picker.multiple() };
        form.picker = Some((role, picker));
        super::refresh_mine(state);
    }
    draw_picker(ctx, theme, state, &mut form);
    if pressed.save && !saving {
        save(state, &mut form, own);
    }
    if open && !pressed.cancel {
        page.edit = Some(form);
    }
}

/// Save's label.
pub const SAVE: &str = "Save";

/// The three parts of the Edit window and of Add to Media (widgets/tint_section.rs), the website's
/// too (web/pages/media-item.js).
pub const ABOUT_IT: &str = "About it";
pub const WHO_CAN_SEE_IT: &str = "Who can see it";
pub const FILES: &str = "Files";

/// The files part of the Edit window: each added file and the cover with Remove (file 0 is the
/// item itself: never removed here), the uploads on their way to it, and, for the uploader, Add a
/// file and Cover picture.
fn draw_files(ui: &mut egui::Ui, theme: &Theme, item: &Item, own: bool, uploads: &[(String, String)], pressed: &mut Pressed) {
    let small = widgets::ButtonSize::Small;
    let files = item_view::files_of(item);
    // Each file's type first, always (widgets/file_badge.rs, 2026-10-17).
    let badge = |ui: &mut egui::Ui, name: &str| widgets::file_badge::show(ui, theme, &crate::net::file_risk::file_type(name, None));
    if let Some(first) = files.first() {
        let name = super::open::file_download_name(item, first);
        ui.horizontal_wrapped(|ui| {
            badge(ui, &name);
            widgets::body_hint(ui, theme, &format!("{name}: the item itself. To take it away, remove the item."));
        });
    }
    for f in files.iter().skip(1).chain(item.cover.iter()) {
        ui.horizontal_wrapped(|ui| {
            let name = super::open::file_download_name(item, f);
            badge(ui, &name);
            let line = if item.cover.as_ref() == Some(f) { format!("{name}, cover picture, {}", api::size_words(f.size_bytes)) } else { format!("{name}, {}", api::size_words(f.size_bytes)) };
            ui.label(RichText::new(line).size(theme.font_size_small).color(theme.text_primary()));
            if widgets::Button::danger("Remove").size(small).tooltip("Take it off this item. It is deleted from the server.").show(ui, theme) {
                pressed.remove = Some(f.clone());
            }
        });
    }
    for (heading, words) in uploads {
        ui.label(RichText::new(format!("{heading}: {words}")).size(theme.font_size_small).color(theme.text_secondary()));
    }
    if own {
        ui.horizontal_wrapped(|ui| {
            if widgets::Button::secondary(ADD_FILE).tooltip("Upload another file to go with this item.").show(ui, theme) {
                pressed.add = Some(FileRole::File);
            }
            let cover = if item.cover.is_some() { "Replace the cover picture" } else { COVER_PICTURE };
            if widgets::Button::secondary(cover).tooltip("A picture shown on its card and page: PNG, JPEG, GIF or WebP.").show(ui, theme) {
                pressed.add = Some(FileRole::Cover);
            }
        });
        widgets::body_hint(ui, theme, "Each goes up under Uploads in progress on the Media page and is added here when it is up.");
    }
}

/// The two buttons that add to an item.
pub const ADD_FILE: &str = "Add a file";
pub const COVER_PICTURE: &str = "Cover picture";

/// The picker for Add a file or Cover picture: the file chosen starts its upload, added to the
/// item when it is up. Its size is checked against the person's share and their role's largest
/// Media file first, as Add to Media's is.
fn draw_picker(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, form: &mut EditForm) {
    use crate::gui::widgets::file_browser::{file_picker_modal, FilePickerResult};
    let Some((role, mut picker)) = form.picker.take() else { return };
    let title = if role == FileRole::Cover { "Cover picture: choose a picture" } else { "Add files: choose one or more" };
    match file_picker_modal(ctx, theme, &mut picker, title) {
        FilePickerResult::Open => form.picker = Some((role, picker)),
        FilePickerResult::Cancelled => {}
        FilePickerResult::Picked(paths) => picked_all(state, form, role, &paths),
    }
}

/// The words over the window while files are dragged onto an item's Edit window.
pub(crate) fn drop_words(item: &Item) -> String {
    format!("Drop to add to {}", item.display_title())
}

/// Files dropped while the Edit window is open: added to the item as files, as Add a file adds them.
pub(crate) fn add_dropped(state: &mut GuiState, paths: &[std::path::PathBuf]) {
    let Some(mut page) = state.media.item.take() else { return };
    if let Some(mut form) = page.edit.take() {
        picked_all(state, &mut form, FileRole::File, paths);
        page.edit = Some(form);
    }
    state.media.item = Some(page);
}

/// Every file chosen in the picker, checked together against the person's share first so nothing
/// starts that cannot finish, then each started as `picked` starts one.
pub(crate) fn picked_all(state: &mut GuiState, form: &mut EditForm, role: FileRole, paths: &[std::path::PathBuf]) {
    if paths.len() > 1 {
        let mut files = Vec::new();
        for path in paths {
            match super::resumable::MoreFile::of(path) {
                Ok(m) => files.push(m),
                Err(words) => {
                    form.problem = words;
                    return;
                }
            }
        }
        let named: Vec<(&str, u64)> = files.iter().map(|m| (m.name.as_str(), m.size)).collect();
        if let Some(problem) = api::room_problem_together(&named, state.media.mine.as_ref()) {
            form.problem = problem;
            return;
        }
    }
    let mut problems = Vec::new();
    for path in paths {
        picked(state, form, role, path);
        if !form.problem.is_empty() {
            problems.push(std::mem::take(&mut form.problem));
        }
    }
    let started = paths.len() - problems.len();
    form.problem = problems.join(" ");
    if started > 1 {
        state.media.status = format!("Uploading {started} files: they are under Uploads in progress, and each is added to the item when it is up.");
    }
}

/// `path` was chosen to add to the item as `role`: checked, then uploaded (uploading.rs
/// `start_attach`), or why not said in the window.
pub(crate) fn picked(state: &mut GuiState, form: &mut EditForm, role: FileRole, path: &std::path::Path) {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
    let len = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(e) => {
            form.problem = format!("{name} could not be read ({e}).");
            return;
        }
    };
    if let Some(problem) = api::room_problem(len, state.media.mine.as_ref()) {
        form.problem = problem;
        return;
    }
    let Ok(item) = form.was.reference() else {
        form.problem = api::NO_FILE_NAME.to_string();
        return;
    };
    let to = AttachTo { id: item.id, filename: item.filename, role };
    match super::uploading::start_attach(state, path, &name, len, to) {
        Ok(()) => {
            form.problem.clear();
            state.media.status = format!("Uploading {name}: it is under Uploads in progress, and is added to the item when it is up.");
        }
        Err(words) => form.problem = words,
    }
}

/// Send the edit, and the visibility when it changed: both answer the item as it is now, and the
/// window closes once both are in (item_view.rs `on_answer`).
fn save(state: &mut GuiState, form: &mut EditForm, own: bool) {
    // The recovery phrase guard (step F): nothing published may carry it.
    if crate::engine::warnings::guard_stops(state, &[&form.title, &form.description]) {
        return;
    }
    let r = match form.was.reference() {
        Ok(r) => r,
        Err(words) => {
            form.problem = words;
            return;
        }
    };
    let Some(signer) = signer_or_say(state) else {
        form.problem = super::UNLOCK_LINE.to_string();
        return;
    };
    form.problem.clear();
    let id = form.was.id;
    spawn(state, Job::Edit { id }, edit_request(&signer, &r, &form.fields(own)));
    // Who can see it goes by its own signed route, only when it changed and may change.
    let may_change = !form.was.keeps_file && !super::members_only_server(state);
    if may_change && form.members_only != form.was.members_only() {
        spawn(state, Job::Edit { id }, visibility_request(&signer, &r, form.members_only));
    }
}
