//! Putting something on the Media page, and asking for a category.
//!
//! ADD TO MEDIA: the file picker, then a short form (title, category, description, labels,
//! visibility, and since Media round 5 the licence, "All rights reserved" preselected, with the
//! server's own words beside it and its hosting paragraph, "Your uploads stay yours.": edit.rs
//! `draw_licence_choice` and `draw_hosting`, the same as the Edit window's). Sending it closes the
//! form: the file joins the uploads in progress on the page
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
use crate::gui::widgets::tint_section::{self, Tint};
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Of {
    /// Uploaded here just now (the upload under way is done with when it answers).
    Upload,
    /// A Media upload already on the server, waiting to be published.
    Waiting,
    /// A file already public in Shared Files or a chat.
    Shared,
    /// Uploaded here just now from an item's Edit window (Media round 5, lane 1D): added to that
    /// item as a file or its cover (`POST /api/media/files/add`) instead of published. The same
    /// upload, the same row under "Uploads in progress", the same Cancel.
    Attach { item: api::ItemRef, role: super::api_items::FileRole },
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
    /// The licence (Media round 5): empty until one is picked, which shows and sends the server's
    /// default ("All rights reserved"); and the "Offer this for the game" tick, offered only with a
    /// licence that allows it.
    pub licence: String,
    pub project_use: bool,
    /// More files chosen with the first (2026-10-16): uploaded and added to the item once it is
    /// published. Only for a picked file.
    pub more: Vec<super::resumable::MoreFile>,
    /// "Also post in chat" (operator, 2026-10-17): ticked, and the channel id (`post_to`) it is
    /// posted in once it is published. Only for a picked file.
    pub post_on: bool,
    pub post_to: String,
}

impl Form {
    pub(crate) fn new(source: Source) -> Self {
        Form {
            id: super::next_id(),
            source,
            title: String::new(),
            description: String::new(),
            category: None,
            labels: BTreeSet::new(),
            members_only: false,
            problem: String::new(),
            sending: false,
            remove: true,
            licence: String::new(),
            project_use: false,
            more: Vec::new(),
            post_on: false,
            post_to: String::new(),
        }
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
    let (name, len) = form.file();
    let mut files = vec![(name, len)];
    files.extend(form.more.iter().map(|m| (m.name.as_str(), m.size)));
    api::room_problem_together(&files, state.media.mine.as_ref())
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
        widgets::file_browser::FilePickerState::new(crate::gui::pages::chat::ATTACH_EXTS, limit).with_pick_verb("Choose").multiple(),
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

/// The picker chose `paths` (several at once, 2026-10-16): one item holds them all, the way a
/// 3D model's item holds its model file and its pictures. The first is the item's own file and
/// the form opens on it (`picked`); the rest are uploaded and added to the item once it is up.
/// All of them are checked against the person's share together first, so nothing starts that
/// cannot finish.
pub(crate) fn picked_many(state: &mut GuiState, paths: &[std::path::PathBuf]) {
    let Some((first, rest)) = paths.split_first() else { return };
    let mut more = Vec::new();
    for path in rest {
        match super::resumable::MoreFile::of(path) {
            Ok(m) => more.push(m),
            Err(words) => {
                state.media.status = words;
                return;
            }
        }
    }
    let first_len = std::fs::metadata(first).map(|m| m.len()).unwrap_or(0);
    let first_name = first.file_name().and_then(|n| n.to_str()).unwrap_or("file").to_string();
    let mut files = vec![(first_name.as_str(), first_len)];
    files.extend(more.iter().map(|m| (m.name.as_str(), m.size)));
    if let Some(problem) = api::room_problem_together(&files, state.media.mine.as_ref()) {
        state.media.status = problem;
        return;
    }
    picked(state, first);
    if let Some(form) = state.media.form.as_mut().filter(|f| f.is_picked()) {
        form.more = more;
    }
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
        match file_picker_modal(ctx, theme, &mut picker, "Add to Media: choose one or more files") {
            FilePickerResult::Open => state.media.picker = Some(picker),
            FilePickerResult::Cancelled => {}
            FilePickerResult::Picked(paths) => picked_many(state, &paths),
        }
    }
    draw_form(ctx, theme, state);
    draw_suggest(ctx, theme, state);
}

/// The line above the further files Add to Media holds, each then on its own row with its type
/// (the website's `MORE_LEAD`).
pub(crate) const MORE_LEAD: &str = "Also on this item, each added once the item is up:";

/// The Add to Media or Put on Media form, while open.
pub(crate) fn draw_form(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(mut form) = state.media.form.take() else { return };
    let problem = if form.sending { None } else { form_problem(state, &form) };
    let members_server = super::members_only_server(state);
    let mine = state.media.mine.clone();
    let mine_failed = state.media.mine_failed;
    let categories = state.media.list.categories.clone();
    let labels = state.media.list.labels.clone();
    let licences = state.media.list.licences.clone();
    let title = if form.is_picked() { "Add to Media" } else { "Put on Media" };
    // A picked file's look (what is hidden in it), its switch, and whether it may go (2026-10-11).
    let mut look = state.media.look.take().filter(|(id, _)| *id == form.id);
    if let (None, Source::Picked { path, name, .. }) = (&look, &form.source) {
        look = Some((form.id, crate::gui::pages::chat::attach_step::Looking::of_file(path.clone(), name.clone())));
    }
    let protected = crate::engine::protected::is_on(state);
    let post_choices = post_choices(state);
    let post_server = super::server_here(state);
    let mut may_go = !form.is_picked();
    let mut open = true;
    let mut submit = false;
    let mut cancel = false;
    widgets::dialog_scrolling(ctx, theme, "media_publish_form", title, &mut open, |ui| {
        ui.set_max_width(520.0);
        // Three parts in the chat sidebar's colours, darker (widgets/tint_section.rs, 2026-10-17):
        // the file first, what it is, then who can see it. The Edit window's parts, in this order.
        tint_section::show(ui, theme, Tint::Blue, super::edit::FILES, |ui| {
            let (name, len) = form.file();
            ui.horizontal_wrapped(|ui| {
                // Its type first, always (widgets/file_badge.rs, 2026-10-17).
                widgets::file_badge::show(ui, theme, &crate::net::file_risk::file_type(name, None));
                ui.label(RichText::new(format!("{name} ({})", api::size_words(len as i64))).size(theme.font_size_body).color(theme.text_primary()).strong());
            });
            if !form.more.is_empty() {
                ui.label(RichText::new(MORE_LEAD).size(theme.font_size_small).color(theme.text_secondary()));
                for m in &form.more {
                    ui.horizontal_wrapped(|ui| {
                        widgets::file_badge::show(ui, theme, &crate::net::file_risk::file_type(&m.name, None));
                        ui.label(RichText::new(format!("{} ({})", m.name, api::size_words(m.size as i64))).size(theme.font_size_small).color(theme.text_primary()));
                    });
                }
            }
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
        });
        ui.add_space(theme.spacing_sm);
        tint_section::show(ui, theme, Tint::Red, super::edit::ABOUT_IT, |ui| {
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
        });
        ui.add_space(theme.spacing_sm);
        tint_section::show(ui, theme, Tint::Green, super::edit::WHO_CAN_SEE_IT, |ui| {
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
            // The licence and the game tick (Media round 5), with the server's words beside them, and
            // what putting a file on this server lets its people do (the hosting paragraph): all from
            // the server's answer (data/media/licences.json on the relay), never a copy in this app.
            ui.add_space(theme.spacing_xs);
            let members = form.for_media() && (members_server || form.members_only);
            let shown = shown_licence(&form, &licences);
            let mut choice = super::edit::LicenceChoice { licence: shown.clone(), project_use: form.project_use };
            super::edit::draw_licence_choice(ui, theme, &licences, &mut choice, members, None, None);
            // Only a pick changes the form: the default shown stays "none picked" (sent as the
            // default), so drawing the form never changes it.
            if choice.licence != shown {
                form.licence = choice.licence;
            }
            form.project_use = choice.project_use;
            super::edit::draw_hosting(ui, theme, &licences);
            if form.is_picked() && !form.sending {
                draw_post_row(ui, theme, &mut form, &post_choices, &post_server);
            }
        });
        if !form.problem.is_empty() {
            ui.label(RichText::new(&form.problem).size(theme.font_size_small).color(theme.danger()));
        }
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            let waiting = mine.is_none() && !mine_failed && form.is_picked();
            let ready = problem.is_none() && !waiting && !form.sending && may_go;
            let pressed = ui.add_enabled_ui(ready, |ui| widgets::Button::primary(submit_words(&form)).show(ui, theme));
            submit = pressed.inner;
            // The Upload button wears the nav bar's active border (operator, 2026-10-17: "Upload
            // button with the RGB border effect"), following the same animation setting.
            if ready && form.is_picked() {
                crate::gui::widgets::tint_section::rgb_border(ui, theme, pressed.response.rect);
            }
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

/// The words over the window while files are dragged onto it with nothing more specific on screen,
/// and over the Media page (gui/drop_files.rs).
pub const DROP_TO_UPLOAD: &str = "Drop to upload to Media";

/// The words over a channel of the chat: the upload window opens with "Also post in chat" ticked.
pub(crate) fn drop_and_post_words(channel_name: &str) -> String {
    format!("Drop to upload to Media and post in {channel_name}")
}

/// Files dropped on the window (gui/drop_files.rs) open this form with them, exactly as choosing
/// them with Add to Media does (the first is the item, the rest added once it is up); with
/// `post_in`, "Also post in chat" is ticked for that channel (a drop on a channel of the chat).
pub(crate) fn upload_dropped(state: &mut GuiState, paths: &[std::path::PathBuf], post_in: Option<String>) {
    // The Media state follows the server on screen; matched first, so the form opened here is not
    // cleared by the match on the next frame.
    super::sync_server(state);
    // From another page the Media page's categories and licences may not be in yet.
    if !state.media.requested && !super::server_here(state).is_empty() {
        super::reload(state);
    }
    super::refresh_mine(state);
    picked_many(state, paths);
    if let (Some(form), Some(channel)) = (state.media.form.as_mut(), post_in) {
        form.post_on = true;
        form.post_to = channel;
    }
}

/// How a channel favourite is kept (widgets/file_picker_memory.rs `post_favorites`).
pub(crate) fn post_key(server: &str, channel: &str) -> String {
    format!("{server}\n{channel}")
}

/// The channels an upload can be posted in, as (id, words shown): the one open in the chat first
/// (when it is one of this server's channels), then the person's favourites, then the rest; a
/// read-only channel only for an admin.
pub(crate) fn post_choices(state: &GuiState) -> Vec<(String, String)> {
    let server = super::server_here(state);
    let admin = super::is_admin(state);
    let usable: Vec<&crate::gui::ChatChannel> = state.chat_channels.iter().filter(|c| admin || !c.read_only).collect();
    let fav = |id: &str| crate::gui::widgets::file_picker_memory::with(|m| m.is_post_favorite(&post_key(&server, id)));
    let current = &state.chat_active_channel;
    let mut out = Vec::new();
    if let Some(c) = usable.iter().find(|c| &c.id == current) {
        out.push((c.id.clone(), format!("#{} (open in the chat)", c.name)));
    }
    for c in usable.iter().filter(|c| &c.id != current && fav(&c.id)) {
        out.push((c.id.clone(), format!("\u{2b50} #{}", c.name)));
    }
    for c in usable.iter().filter(|c| &c.id != current && !fav(&c.id)) {
        out.push((c.id.clone(), format!("#{}", c.name)));
    }
    out
}

/// The words of "Also post in chat" and its tooltip.
pub const POST_IN_CHAT: &str = "Also post in chat";
pub const POST_IN_CHAT_TIP: &str = "When it is up, a card for it is posted in this channel: its title, then its picture, description and every file when opened.";

/// "Also post in chat": a tick on the left, then the channel and Add to favorites, greyed out while
/// the tick is off (operator, 2026-10-17).
fn draw_post_row(ui: &mut egui::Ui, theme: &Theme, form: &mut Form, choices: &[(String, String)], server: &str) {
    ui.add_space(theme.spacing_xs);
    if choices.is_empty() {
        form.post_on = false;
        ui.label(RichText::new(format!("{POST_IN_CHAT}: this server has no channel to post in.")).size(theme.font_size_small).color(theme.text_muted()));
        return;
    }
    // A channel no longer offered (gone, or read-only for this person) is never swapped for another
    // behind the person's back: the tick goes off, and the first channel is shown for them to pick.
    if !choices.iter().any(|(id, _)| *id == form.post_to) {
        if !form.post_to.is_empty() {
            form.post_on = false;
        }
        form.post_to = choices[0].0.clone();
    }
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut form.post_on, POST_IN_CHAT).on_hover_text(POST_IN_CHAT_TIP);
        ui.add_enabled_ui(form.post_on, |ui| {
            let shown = choices.iter().find(|(id, _)| *id == form.post_to).map(|(_, w)| w.clone()).unwrap_or_default();
            egui::ComboBox::from_id_salt("media_form_post_to").selected_text(shown).show_ui(ui, |ui| {
                for (id, words) in choices {
                    if ui.selectable_label(*id == form.post_to, words).clicked() {
                        form.post_to = id.clone();
                    }
                }
            });
            let key = post_key(server, &form.post_to);
            let fav = crate::gui::widgets::file_picker_memory::with(|m| m.is_post_favorite(&key));
            let words = if fav { "Remove from favorites" } else { "Add to favorites" };
            if widgets::Button::secondary(words).size(widgets::ButtonSize::Small).tooltip("Favorite channels are listed first, after the one open in the chat.").show(ui, theme) {
                crate::gui::widgets::file_picker_memory::with(|m| m.toggle_post_favorite(&key));
            }
        });
    });
}

/// What "Also post in chat" posts for a published item: its own address on this server's Media
/// page, which both apps show as the item's card (chat_card.rs: its title, how many files and how
/// much, opening out to its picture, description and every file, and its full window on a click;
/// operator, 2026-10-17: "I'd rather people know exactly what they're downloading instead of
/// guessing"). An app without cards shows a link to the item's page. A members-only item's page
/// opens for members only.
pub(crate) fn post_text(item: &api::Item, base: &str) -> String {
    super::api_items::item_link(base, item.id)
}

/// A just-published item's "Also post in chat": posted in `channel` through the chat's own send, as
/// if typed there, on the server it was uploaded to; said in the Media page's line either way.
pub(crate) fn post_published(state: &mut GuiState, answer: &serde_json::Value, channel: &str, server: &str) {
    let Some(item) = answer.get("item").and_then(api::Item::from_json) else { return };
    let name = state.chat_channels.iter().find(|c| c.id == channel).map(|c| format!("#{}", c.name)).unwrap_or_else(|| format!("#{channel}"));
    let here = crate::gui::pages::chat::norm_server_url(&state.server_url);
    // The chat's own send echoes a message locally even with no connection, so the connection is
    // checked first: "Posted" must mean the server has it.
    let connected = state.ws_client.as_ref().is_some_and(|c| c.is_connected());
    let words = if here != server {
        format!("Not posted in {name}: the chat is on another server now.")
    } else if !connected {
        format!("Not posted in {name}: the chat is not connected to its server.")
    } else if crate::gui::pages::chat::post_in(state, channel, &post_text(&item, &super::base_of(state))) {
        format!("Posted in {name}.")
    } else {
        format!("Not posted in {name}: the chat could not send it.")
    };
    state.media.status = format!("{} {words}", state.media.status).trim().to_string();
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
        // The licence and the game tick (Media round 5): what the form shows.
        licence: form.licence.clone(),
        project_use: form.project_use,
    }
}

/// The licence the form shows: the one picked, else the server's default. The game tick goes with
/// it only while that licence allows it (`edit::draw_licence_choice` keeps it so).
pub(crate) fn shown_licence(form: &Form, licences: &api::Licences) -> String {
    if form.licence.trim().is_empty() {
        licences.default.clone()
    } else {
        form.licence.clone()
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
