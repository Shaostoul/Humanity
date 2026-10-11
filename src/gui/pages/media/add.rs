//! Putting something on the Media page, and asking for a category.
//!
//! ADD TO MEDIA: the file picker, then a short form (title, category, description, labels,
//! visibility). The file uploads with `?media=1` (chat/attach_send.rs, the same upload every file
//! takes, with the Media flag) and is then published with `POST /api/media`. Above the form is
//! what is left of the person's share of the Media space ("You have 412 MB of your 512 MB
//! left"), and a file over it is stopped before a byte is sent.
//!
//! PUT ON MEDIA: the same form for a file already on the server (a Shared Files row on the Files
//! page), without the picker. Such a file is already public where it was shared, so it goes on
//! Media as public only: the visibility choice is not offered, and a line says why (the relay
//! refuses members only for it, `storage::media::was_public`).
//!
//! SUGGEST A CATEGORY: a name and a note for the admins; their answer comes as a notice.

use std::collections::BTreeSet;
use std::path::PathBuf;

use egui::RichText;

use super::api::{self, PublishFields};
use super::{signer_here, spawn, spawn_with, Job};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiPage, GuiState};

/// Where the form's file is.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Source {
    /// A file on this computer, picked with Add to Media: uploaded, then published.
    Picked { path: PathBuf, name: String, len: u64 },
    /// A file already on the server (its stored name), published as it is: public only.
    Existing { filename: String, name: String, len: u64 },
}

/// The Add to Media or Put on Media form.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Form {
    pub source: Source,
    pub title: String,
    pub description: String,
    /// None: the server's first category (General).
    pub category: Option<i64>,
    /// The ticked label ids.
    pub labels: BTreeSet<String>,
    /// Members only (only for a file picked here, on a public page).
    pub members_only: bool,
    /// Why the last press did not go.
    pub problem: String,
    /// The upload or publish is under way.
    pub sending: bool,
}

impl Form {
    pub(crate) fn new(source: Source) -> Self {
        Form { source, title: String::new(), description: String::new(), category: None, labels: BTreeSet::new(), members_only: false, problem: String::new(), sending: false }
    }

    /// A file picked on this computer (not one already on the server).
    pub(crate) fn is_picked(&self) -> bool {
        matches!(self.source, Source::Picked { .. })
    }

    /// The file's name and size.
    pub(crate) fn file(&self) -> (&str, u64) {
        match &self.source {
            Source::Picked { name, len, .. } | Source::Existing { name, len, .. } => (name, *len),
        }
    }
}

/// The line for a file already public where it was shared.
pub(crate) const ALREADY_PUBLIC_LINE: &str = "This file is already public where it was shared.";
/// The line where a members-only server leaves no choice.
pub(crate) const MEMBERS_SERVER_LINE: &str = "This server's Media page is for members only, so this item is too.";
/// While the share's use is being added up.
pub(crate) const WORKING_OUT_LINE: &str = "Working out how much of your share is left...";

/// This person's share of the Media space in bytes, from the server's settings.
pub(crate) fn share_bytes(state: &GuiState) -> Option<i64> {
    state.server_settings.as_ref().map(|s| s.media_per_person_mb.max(1) * 1024 * 1024)
}

/// What is left of it, once the person's use is known.
pub(crate) fn share_left(state: &GuiState) -> Option<i64> {
    Some(share_bytes(state)? - state.media.my_use?)
}

/// Why `form`'s file cannot go now, if it cannot: over the person's upload limit (a file from
/// this computer only), or over what is left of their share.
pub(crate) fn form_problem(state: &GuiState, form: &Form) -> Option<String> {
    let (_, len) = form.file();
    let role_limit = if form.is_picked() { state.upload_limit_bytes } else { None };
    api::room_problem(len, role_limit, share_left(state))
}

/// Add to Media was pressed: the file picker opens, and the person's share starts being added up.
pub(crate) fn begin_add(state: &mut GuiState) {
    if signer_here(state).is_none() {
        state.media.status = "Unlock your identity and sign in to this server to add to its Media page.".to_string();
        return;
    }
    let limit = state.upload_limit_bytes.unwrap_or(0);
    state.media.picker = Some(
        widgets::file_browser::FilePickerState::new(crate::gui::pages::chat::ATTACH_EXTS, limit).with_pick_verb("Choose"),
    );
    super::add_up_my_use(state);
}

/// The picker chose `path`. Its size is checked before anything is read: a file over the
/// person's upload limit, or over what is left of their share when that is known, is stopped
/// here with the numbers, and nothing is uploaded. Otherwise the form opens.
pub(crate) fn picked(state: &mut GuiState, path: &std::path::Path) {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
    let len = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(e) => {
            state.media.status = format!("{name} could not be read ({e}).");
            return;
        }
    };
    if let Some(problem) = api::room_problem(len, state.upload_limit_bytes, share_left(state)) {
        state.media.status = problem;
        return;
    }
    state.media.form = Some(Form::new(Source::Picked { path: path.to_path_buf(), name, len }));
}

/// Put a file already on the server (a Shared Files row of the Files page) on the Media page:
/// the form opens on the Media page, public only.
pub(crate) fn put_on_media(state: &mut GuiState, filename: &str, name: &str, len: u64) {
    super::sync_server(state);
    let source = Source::Existing { filename: api::bare_name(filename).to_string(), name: name.to_string(), len };
    state.media.form = Some(Form::new(source));
    super::add_up_my_use(state);
    state.push_nav_to(GuiPage::Media);
}

/// The picker, the form and the suggestion form, whichever are open.
pub(crate) fn draw_dialogs(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    if let Some(mut picker) = state.media.picker.take() {
        use crate::gui::widgets::file_browser::{file_picker_modal, FilePickerResult};
        match file_picker_modal(ctx, theme, &mut picker, "Add to Media: choose a file") {
            FilePickerResult::Open => state.media.picker = Some(picker),
            FilePickerResult::Cancelled => {}
            FilePickerResult::Picked(path) => picked(state, &path),
        }
    }
    draw_form(ctx, theme, state);
    draw_suggest(ctx, theme, state);
}

/// The Add to Media or Put on Media form, while open.
pub(crate) fn draw_form(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(mut form) = state.media.form.take() else { return };
    let problem = form_problem(state, &form);
    let members_server = super::members_only_server(state);
    let share = share_bytes(state);
    let (my_use, use_failed) = (state.media.my_use, state.media.my_use_failed);
    let categories = state.media.list.categories.clone();
    let labels = state.media.list.labels.clone();
    let title = if form.is_picked() { "Add to Media" } else { "Put on Media" };
    let mut open = true;
    let mut submit = false;
    let mut cancel = false;
    widgets::dialog_scrolling(ctx, theme, "media_publish_form", title, &mut open, |ui| {
        ui.set_max_width(520.0);
        let (name, len) = form.file();
        ui.label(RichText::new(format!("{name} ({})", api::size_words(len as i64))).size(theme.font_size_body).color(theme.text_primary()).strong());
        // The person's share, above the form.
        let share_text = match (share, my_use) {
            (Some(share), Some(used)) => api::share_line(used, share),
            (None, _) => "This server's Media share is not known yet.".to_string(),
            (_, None) if use_failed => "Your share could not be checked here; the server checks it.".to_string(),
            (_, None) => WORKING_OUT_LINE.to_string(),
        };
        ui.label(RichText::new(share_text).size(theme.font_size_small).color(theme.text_secondary()));
        if let Some(p) = problem.as_deref() {
            ui.label(RichText::new(p).size(theme.font_size_small).color(theme.danger()));
        }
        ui.add_space(theme.spacing_sm);
        widgets::form_row(ui, theme, "Title (optional)", |ui| {
            ui.add(egui::TextEdit::singleline(&mut form.title).desired_width(300.0).char_limit(120));
        });
        widgets::form_row(ui, theme, "Category", |ui| {
            let shown = form
                .category
                .and_then(|c| categories.iter().find(|x| x.id == c))
                .or_else(|| categories.first())
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "General".to_string());
            egui::ComboBox::from_id_salt("media_form_category").selected_text(shown).show_ui(ui, |ui| {
                for c in &categories {
                    ui.selectable_value(&mut form.category, Some(c.id), &c.name).on_hover_text(&c.description);
                }
            });
        });
        ui.label(RichText::new("Description (optional)").size(theme.font_size_small).color(theme.text_secondary()));
        ui.add(egui::TextEdit::multiline(&mut form.description).desired_rows(3).desired_width(480.0).char_limit(2000));
        if !labels.is_empty() {
            ui.add_space(theme.spacing_xs);
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
        ui.add_space(theme.spacing_xs);
        if !form.is_picked() {
            ui.label(RichText::new(ALREADY_PUBLIC_LINE).size(theme.font_size_small).color(theme.text_secondary()));
        } else if members_server {
            ui.label(RichText::new(MEMBERS_SERVER_LINE).size(theme.font_size_small).color(theme.text_secondary()));
        } else {
            ui.horizontal(|ui| {
                ui.radio_value(&mut form.members_only, false, "Public");
                ui.radio_value(&mut form.members_only, true, "Members only");
            });
        }
        if !form.problem.is_empty() {
            ui.label(RichText::new(&form.problem).size(theme.font_size_small).color(theme.danger()));
        }
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            let waiting = share.is_some() && my_use.is_none() && !use_failed;
            let ready = problem.is_none() && !waiting && !form.sending;
            ui.add_enabled_ui(ready, |ui| {
                submit = widgets::Button::primary(submit_words(&form)).show(ui, theme);
            });
            if form.sending {
                let doing = if form.is_picked() { "Uploading..." } else { "Publishing..." };
                ui.label(RichText::new(doing).size(theme.font_size_small).color(theme.text_muted()));
            }
            cancel = widgets::Button::secondary("Cancel").show(ui, theme);
        });
    });
    if submit && problem.is_none() && !form.sending {
        send_form(state, &mut form, members_server);
    }
    if open && !cancel {
        state.media.form = Some(form);
    }
}

/// The form's send button.
pub(crate) fn submit_words(form: &Form) -> &'static str {
    if form.is_picked() {
        "Upload and add to Media"
    } else {
        "Put it on Media"
    }
}

/// The publish the form describes, for `filename` (its stored name or address).
pub(crate) fn fields_of(form: &Form, filename: &str, members_server: bool) -> PublishFields {
    // Only a file uploaded for Media can be members only; on a members-only server it is.
    let visibility = if form.is_picked() && (members_server || form.members_only) { "members" } else { "public" };
    PublishFields {
        filename: filename.to_string(),
        title: form.title.clone(),
        description: form.description.clone(),
        category_id: form.category,
        labels: form.labels.iter().cloned().collect(),
        visibility: visibility.to_string(),
    }
}

/// Send the form: upload (a picked file) and publish, on a worker thread.
fn send_form(state: &mut GuiState, form: &mut Form, members_server: bool) {
    let (name, _) = form.file();
    let name = name.to_string();
    // The recovery phrase guard (step F): nothing published may carry it.
    if crate::engine::warnings::guard_stops(state, &[&form.title, &form.description, &name]) {
        return;
    }
    let Some(signer) = signer_here(state) else {
        form.problem = "Sign in to this server first.".to_string();
        return;
    };
    form.problem.clear();
    form.sending = true;
    match &form.source {
        Source::Existing { filename, .. } => {
            let req = api::publish_request(&signer, &fields_of(form, filename, members_server));
            spawn(state, Job::Publish { uploaded: false }, req);
        }
        Source::Picked { path, .. } => {
            let Some(token) = crate::gui::pages::chat::upload_token_here(state) else {
                form.sending = false;
                form.problem = crate::gui::pages::chat::NOT_SIGNED_IN_YET.to_string();
                return;
            };
            let (path, base) = (path.clone(), super::base_of(state));
            let mime = crate::gui::pages::chat::mime_for_filename(&name).to_string();
            let template = fields_of(form, "", members_server);
            spawn_with(state, Job::Publish { uploaded: true }, move || {
                let bytes = std::fs::read(&path).map_err(|e| format!("{name} could not be read ({e})."))?;
                let url = crate::gui::pages::chat::upload_media_file_blocking(&base, &token, &name, &mime, bytes)?;
                // Signed only now, after the upload: a signature is good for five minutes.
                let fields = PublishFields { filename: url, ..template };
                api::send(&base, &api::publish_request(&signer, &fields))
            });
        }
    }
}

/// The Suggest a category form, while open.
fn draw_suggest(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some((mut name, mut note)) = state.media.suggest.take() else { return };
    let mut open = true;
    let mut send = false;
    let mut cancel = false;
    widgets::dialog(ctx, theme, "media_suggest", "Suggest a category", &mut open, |ui| {
        widgets::body_hint(ui, theme, "This server's admins choose its categories. Suggest one, and you will be told when they decide.");
        widgets::form_row(ui, theme, "Name", |ui| {
            ui.add(egui::TextEdit::singleline(&mut name).desired_width(260.0).char_limit(60));
        });
        ui.label(RichText::new("Note (optional)").size(theme.font_size_small).color(theme.text_secondary()));
        ui.add(egui::TextEdit::multiline(&mut note).desired_rows(2).desired_width(360.0).char_limit(500));
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!name.trim().is_empty(), |ui| {
                send = widgets::Button::primary("Send to the admins").show(ui, theme);
            });
            cancel = widgets::Button::secondary("Cancel").show(ui, theme);
        });
    });
    if send {
        if let Some(signer) = super::signer_or_say(state) {
            spawn(state, Job::Suggest, api::suggest_request(&signer, &name, &note));
        }
    }
    if open && !cancel && !send {
        state.media.suggest = Some((name, note));
    }
}
