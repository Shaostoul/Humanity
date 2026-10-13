//! Putting something on the Media page, and asking for a category.
//!
//! ADD TO MEDIA: the file picker, then a short form (title, category, description, labels,
//! visibility). Sending it closes the form: the file joins the uploads in progress on the page
//! (media/uploading.rs, each with its bar, Pause, Resume and Cancel), goes up in pieces
//! (media/resumable.rs) and is then published with `POST /api/media`, signed then, with what this
//! form said (round 3: no form is held open for an upload, so opening another never touches it).
//! Above the form is what is left of the person's share of the
//! Media space, as the server counts it (`/api/media/mine`: "You have 412 MB of your 512 MB
//! left"), and a file over it, over the largest Media file their role may send, or over what is
//! left of the whole space is stopped before a byte is sent.
//!
//! PUT ON MEDIA: the same form for a file already on the server (a Shared Files row on the Files
//! page), without the picker. Such a file is already public where it was shared, so it goes on
//! Media as public only: the visibility choice is not offered, and a line says why (the relay
//! refuses members only for it, `storage::media::was_public`).
//!
//! A WAITING UPLOAD (one of the person's Media uploads whose publish failed or never happened,
//! listed on the page) opens the same form too, with the visibility choice: it was uploaded for
//! Media.
//!
//! THE CATEGORY SENT IS THE CATEGORY SHOWN: with none chosen, the form shows the general category
//! (the relay's `GENERAL_CATEGORY_ID`, whatever an admin renamed it) and sends its number.
//!
//! SUGGEST A CATEGORY: a name and a note for the admins; their answer comes as a notice.

use std::collections::BTreeSet;
use std::path::PathBuf;

use egui::RichText;

use super::api::{self, Category, PublishFields};
use super::{publishing, signer_here, spawn, uploading, Job};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiPage, GuiState};

/// Where the form's file is.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Source {
    /// A file on this computer, picked with Add to Media: uploaded in pieces, then published.
    Picked { path: PathBuf, name: String, len: u64 },
    /// A file already public on the server (its stored name), published as it is: public only.
    Existing { filename: String, name: String, len: u64 },
    /// One of the person's own Media uploads waiting to be published: published as it is.
    Waiting { filename: String, name: String, len: u64 },
}

/// What kind of file a publish puts on Media, so its answer is handled the right way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Of {
    /// Uploaded here just now (the upload under way is done with when it answers).
    Upload,
    /// A Media upload already on the server, waiting to be published.
    Waiting,
    /// A file already public in Shared Files or a chat.
    Shared,
}

/// The Add to Media or Put on Media form.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Form {
    /// Its own number (`media::next_id`): its publish's answer lands on this form only.
    pub id: u64,
    pub source: Source,
    pub title: String,
    pub description: String,
    /// None: the general category, which the form shows by name.
    pub category: Option<i64>,
    /// The ticked label ids.
    pub labels: BTreeSet<String>,
    /// Members only (only for a file uploaded for Media, on a public page).
    pub members_only: bool,
    /// Why the last press did not go.
    pub problem: String,
    /// The publish of a file already on the server is under way (an upload's never holds a form).
    pub sending: bool,
    /// "Remove location and camera details" for a picked file (the files section of
    /// docs/design/media-showcase.md, 2026-10-11): cleaned on this device before it goes, or kept
    /// by choice (`keep_metadata`). Starts as last chosen; always on under the protected setup.
    pub remove: bool,
}

impl Form {
    pub(crate) fn new(source: Source) -> Self {
        Form { id: super::next_id(), source, title: String::new(), description: String::new(), category: None, labels: BTreeSet::new(), members_only: false, problem: String::new(), sending: false, remove: true }
    }

    /// A file picked on this computer (not one already on the server).
    pub(crate) fn is_picked(&self) -> bool {
        matches!(self.source, Source::Picked { .. })
    }

    /// A file uploaded for Media (here, now or before), which may be members only.
    pub(crate) fn for_media(&self) -> bool {
        !matches!(self.source, Source::Existing { .. })
    }

    /// The file's name and size.
    pub(crate) fn file(&self) -> (&str, u64) {
        match &self.source {
            Source::Picked { name, len, .. } | Source::Existing { name, len, .. } | Source::Waiting { name, len, .. } => (name, *len),
        }
    }

    /// The stored name of a file already on the server (None for one still on this computer).
    pub(crate) fn stored(&self) -> Option<&str> {
        match &self.source {
            Source::Existing { filename, .. } | Source::Waiting { filename, .. } => Some(filename),
            Source::Picked { .. } => None,
        }
    }
}

/// The line for a file already public where it was shared.
pub(crate) const ALREADY_PUBLIC_LINE: &str = "This file is already public where it was shared.";
/// The line where a members-only server leaves no choice.
pub(crate) const MEMBERS_SERVER_LINE: &str = "This server's Media page is for members only, so this item is too.";
/// While the share's use is being asked for.
pub(crate) const WORKING_OUT_LINE: &str = "Working out how much of your share is left...";

/// The category the form shows and sends: the one chosen, if the server still lists it, else
/// the general category, named as the server names it.
pub(crate) fn shown_category(form: &Form, categories: &[Category]) -> (i64, String) {
    let id = form.category.filter(|c| categories.iter().any(|x| x.id == *c)).unwrap_or(api::GENERAL_CATEGORY);
    let name = categories.iter().find(|x| x.id == id).map(|x| x.name.clone()).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| "General".to_string());
    (id, name)
}

/// Why `form`'s file cannot go now, if it cannot: over the role's largest Media file, the
/// person's share, or the whole space (`api::room_problem`). A waiting upload already counts.
pub(crate) fn form_problem(state: &GuiState, form: &Form) -> Option<String> {
    if matches!(form.source, Source::Waiting { .. }) {
        return None;
    }
    let (_, len) = form.file();
    api::room_problem(len, state.media.mine.as_ref())
}

/// Add to Media was pressed: the file picker opens, and the person's own use is asked for.
pub(crate) fn begin_add(state: &mut GuiState) {
    if signer_here(state).is_none() {
        state.media.status = "Unlock your identity and sign in to this server to add to its Media page.".to_string();
        return;
    }
    // The role's largest Media file, when the server has said and there is one (0: no limit).
    let limit = state.media.mine.as_ref().map(|m| m.file_limit_bytes.max(0) as u64).unwrap_or(0);
    state.media.picker = Some(
        widgets::file_browser::FilePickerState::new(crate::gui::pages::chat::ATTACH_EXTS, limit).with_pick_verb("Choose"),
    );
    super::refresh_mine(state);
}

/// The picker chose `path`. Its size is checked before anything is read: a file over what the
/// server's numbers allow is stopped here with them, and nothing is uploaded. Otherwise the form
/// opens.
pub(crate) fn picked(state: &mut GuiState, path: &std::path::Path) {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
    let len = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(e) => {
            state.media.status = format!("{name} could not be read ({e}).");
            return;
        }
    };
    if let Some(problem) = api::room_problem(len, state.media.mine.as_ref()) {
        state.media.status = problem;
        return;
    }
    // What the file says is worked out on a worker while the form is filled in (2026-10-11).
    let looking = crate::gui::pages::chat::attach_step::Looking::of_file(path.to_path_buf(), name.clone());
    let form = Form { remove: crate::gui::pages::chat::attach_step::remove_at_start(state), ..Form::new(Source::Picked { path: path.to_path_buf(), name, len }) };
    state.media.look = Some((form.id, looking));
    state.media.form = Some(form);
}

/// Put a file already on the server (a Shared Files row of the Files page) on the Media page:
/// the form opens on the Media page, public only. It replaces whatever form was open; an upload
/// in progress holds none (round 3), so none is touched, and a publish already on its way answers
/// to its own form's number, so it never lands on this one.
pub(crate) fn put_on_media(state: &mut GuiState, filename: &str, name: &str, len: u64) {
    super::sync_server(state);
    let source = Source::Existing { filename: api::bare_name(filename).to_string(), name: name.to_string(), len };
    state.media.form = Some(Form::new(source));
    super::refresh_mine(state);
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
    let problem = if form.sending { None } else { form_problem(state, &form) };
    let members_server = super::members_only_server(state);
    let mine = state.media.mine.clone();
    let mine_failed = state.media.mine_failed;
    let categories = state.media.list.categories.clone();
    let labels = state.media.list.labels.clone();
    let title = if form.is_picked() { "Add to Media" } else { "Put on Media" };
    // A picked file's look (what is hidden in it), its switch, and whether it may go (2026-10-11).
    let mut look = state.media.look.take().filter(|(id, _)| *id == form.id);
    if let (None, Source::Picked { path, name, .. }) = (&look, &form.source) {
        look = Some((form.id, crate::gui::pages::chat::attach_step::Looking::of_file(path.clone(), name.clone())));
    }
    let protected = crate::engine::protected::is_on(state);
    let mut may_go = !form.is_picked();
    let mut open = true;
    let mut submit = false;
    let mut cancel = false;
    widgets::dialog_scrolling(ctx, theme, "media_publish_form", title, &mut open, |ui| {
        ui.set_max_width(520.0);
        let (name, len) = form.file();
        ui.label(RichText::new(format!("{name} ({})", api::size_words(len as i64))).size(theme.font_size_body).color(theme.text_primary()).strong());
        if let (true, Some((_, looking))) = (form.is_picked() && !form.sending, look.as_mut()) {
            may_go = crate::gui::pages::chat::attach_step::draw_look(ui, theme, looking, &mut form.remove, protected, false).can_send;
        }
        // The person's share, above the form, as the server counts it.
        let share_text = match (&mine, mine_failed) {
            (Some(m), _) => api::share_line(m),
            (None, true) => "Your share could not be checked here; the server checks it.".to_string(),
            (None, false) => WORKING_OUT_LINE.to_string(),
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
            let (shown_id, shown) = shown_category(&form, &categories);
            egui::ComboBox::from_id_salt("media_form_category").selected_text(shown).show_ui(ui, |ui| {
                for c in &categories {
                    if ui.selectable_label(c.id == shown_id, &c.name).on_hover_text(&c.description).clicked() {
                        form.category = Some(c.id);
                    }
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
        if !form.for_media() {
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
            let waiting = mine.is_none() && !mine_failed && form.is_picked();
            let ready = problem.is_none() && !waiting && !form.sending && may_go;
            ui.add_enabled_ui(ready, |ui| {
                submit = widgets::Button::primary(submit_words(&form)).show(ui, theme);
            });
            if form.sending {
                ui.label(RichText::new("Publishing...").size(theme.font_size_small).color(theme.text_muted()));
            }
            let tip = if form.sending { "Stop the publish: it is taken off again as soon as it lands." } else { "Close this without putting anything on Media." };
            cancel = widgets::Button::secondary("Cancel").tooltip(tip).show(ui, theme);
        });
    });
    if submit && problem.is_none() && !form.sending && may_go && send_form(state, &mut form, members_server) {
        if form.is_picked() {
            crate::gui::pages::chat::attach_step::remember_switch(state, form.remove);
        }
        return; // an upload started: it is in the list on the page now, and the form is done
    }
    state.media.look = look;
    if cancel {
        // A Cancel while its publish is on its way undoes the publish when it lands.
        if form.sending {
            // Said first: when the answer is already in, the cancel itself says how it went.
            state.media.status = "Cancelling...".to_string();
            publishing::cancel(state, form.id, form.stored().unwrap_or(""));
        }
        return;
    }
    // The window's own close does nothing while something is being sent: only Cancel stops it.
    if open || form.sending {
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

/// The publish the form describes, for `filename` (its stored name or address): the category
/// shown, and members only only for a file uploaded for Media (always, on a members-only server).
pub(crate) fn fields_of(form: &Form, filename: &str, categories: &[Category], members_server: bool) -> PublishFields {
    let visibility = if form.for_media() && (members_server || form.members_only) { "members" } else { "public" };
    PublishFields {
        filename: filename.to_string(),
        title: form.title.clone(),
        description: form.description.clone(),
        category_id: shown_category(form, categories).0,
        labels: form.labels.iter().cloned().collect(),
        visibility: visibility.to_string(),
        // The licence and the game tick (Media round 5): the defaults until the form offers them.
        ..Default::default()
    }
}

/// Send the form: a picked file starts its upload, which joins the list on the page and is
/// published when it is up with what this form says (true: the form is done and closes); a file
/// already on the server is published now, the form waiting for the answer.
fn send_form(state: &mut GuiState, form: &mut Form, members_server: bool) -> bool {
    let (name, _) = form.file();
    let name = name.to_string();
    // The recovery phrase guard (step F): nothing published may carry it.
    if crate::engine::warnings::guard_stops(state, &[&form.title, &form.description, &name]) {
        return false;
    }
    let Some(signer) = signer_here(state) else {
        form.problem = "Sign in to this server first.".to_string();
        return false;
    };
    form.problem.clear();
    match form.stored().map(str::to_string) {
        Some(filename) => {
            form.sending = true;
            let of = if matches!(form.source, Source::Waiting { .. }) { Of::Waiting } else { Of::Shared };
            let fields = fields_of(form, &filename, &state.media.list.categories, members_server);
            publishing::publish(state, of, form.id, &signer, &fields);
            false
        }
        None => match uploading::start(state, form) {
            Ok(()) => {
                state.media.status = format!("Uploading {name}: it is in the list on this page, and goes on the Media page when it is up.");
                true
            }
            Err(words) => {
                form.problem = words;
                false
            }
        },
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
