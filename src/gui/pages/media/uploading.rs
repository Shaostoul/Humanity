//! The Add to Media upload under way, as the page sees it: starting one (media/resumable.rs does
//! the sending), reading what its worker reports every frame (`poll`, from media.rs
//! `poll_upload`, whatever page is open), its progress bar with Pause, Resume, Retry and Cancel,
//! publishing it once it is up, and the offer to carry on an upload a restart cut off. Also the
//! person's own uploads waiting to be published (`/api/media/mine`'s `waiting`), with Put it on
//! Media and Remove.
//!
//! CANCEL STOPS IT ALL (the 2026-10-11 review found the worker published anyway): before the file
//! is up, the worker stops and tells the server, which drops what arrived; once it is up and its
//! publish is on its way, the publish's answer is followed at once by the upload's removal, so
//! nothing of it stays.
//!
//! The record of an upload that did not finish (`MediaState::unfinished`) is written to the app's
//! config when the server gives the upload its id, and forgotten when it is published, cancelled
//! or gone from the server.

use std::sync::Arc;

use egui::RichText;
use serde_json::Value;

use super::add::{self, Form, Of, Source};
use super::api::{self, Item, Refusal, RemoveTarget, Waiting};
use super::resumable::{self, Control, Event, Phase, Plan, Run, Transport, Unfinished};
use super::{signer_here, spawn, spawn_with, Job};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// What is said when an upload, or its publish, was cancelled.
pub(crate) const CANCELLED_LINE: &str = "Cancelled: nothing of it stays on the server.";

fn save_config(state: &GuiState) {
    crate::config::AppConfig::from_gui_state(state).save();
}

/// Forget the record of an unfinished upload, in the config too.
fn forget_unfinished(state: &mut GuiState) {
    if state.media.unfinished.take().is_some() {
        save_config(state);
    }
}

/// The server's resumable upload routes.
fn transport(state: &GuiState) -> Arc<dyn Transport> {
    Arc::new(resumable::Http::new(&super::base_of(state)))
}

/// Start uploading the form's picked file. Err with the words when it cannot start.
pub(crate) fn start(state: &mut GuiState, form: &Form) -> Result<(), String> {
    let t = transport(state);
    start_with(state, form, t)
}

/// [`start`] through `t` (the tests' simulated server).
pub(crate) fn start_with(state: &mut GuiState, form: &Form, t: Arc<dyn Transport>) -> Result<(), String> {
    let Source::Picked { path, name, len } = &form.source else {
        return Err("Only a file on this computer is uploaded.".to_string());
    };
    let Some(token) = crate::gui::pages::chat::upload_token_here(state) else {
        return Err(crate::gui::pages::chat::NOT_SIGNED_IN_YET.to_string());
    };
    // One upload at a time: an older one that did not finish is cancelled first.
    if state.media.unfinished.as_ref().is_some_and(|u| !u.id.is_empty()) {
        discard(state);
    }
    let fields = add::fields_of(form, "", &state.media.list.categories, super::members_only_server(state));
    state.media.unfinished = Some(Unfinished {
        server: state.media.server.clone(),
        key: state.media.key.clone(),
        id: String::new(), // until the server gives one: only then is it written to the config
        path: path.clone(),
        name: name.clone(),
        size: *len,
        modified_ms: resumable::modified_ms(path),
        title: form.title.clone(),
        description: form.description.clone(),
        category_id: fields.category_id,
        labels: fields.labels,
        members_only: form.members_only,
    });
    let plan = Plan { path: path.clone(), name: name.clone(), size: *len, content_type: crate::gui::pages::chat::mime_for_filename(name).to_string(), resume: None };
    state.media.upload = Some(Run::start(t, plan, Arc::new(Control::new(Some(token)))));
    Ok(())
}

/// Read what the upload's worker reported, and act on it. Every frame (media.rs `poll_upload`).
pub(crate) fn poll(ctx: &egui::Context, state: &mut GuiState) {
    let Some(run) = state.media.upload.as_mut() else { return };
    let events = run.poll();
    ctx.request_repaint_after(std::time::Duration::from_millis(250));
    for e in events {
        on_event(state, e);
    }
}

/// One event the page acts on.
pub(crate) fn on_event(state: &mut GuiState, e: Event) {
    match e {
        Event::Started { id } => {
            if let Some(u) = state.media.unfinished.as_mut() {
                u.id = id;
            }
            save_config(state);
        }
        Event::Done { answer } => publish_uploaded(state, resumable::stored_name_of(&answer)),
        Event::Failed { words, gone } => {
            state.media.upload = None;
            if gone {
                forget_unfinished(state);
            }
            match state.media.form.as_mut().filter(|f| f.is_picked() && f.sending) {
                Some(f) => {
                    f.sending = false;
                    f.problem = words;
                }
                None => state.media.status = words,
            }
            super::refresh_mine(state);
        }
        Event::Cancelled => {
            state.media.upload = None;
            forget_unfinished(state);
            state.media.status = CANCELLED_LINE.to_string();
            super::refresh_mine(state);
        }
        _ => {}
    }
}

/// The file is up: publish it now, signed now; or, when Cancel was pressed meanwhile, take it off
/// the server again.
fn publish_uploaded(state: &mut GuiState, filename: String) {
    let cancelled = state.media.upload.as_ref().is_some_and(|r| r.ctl.is_cancelled());
    let form = state.media.form.clone().filter(|f| f.is_picked() && f.sending);
    let (Some(form), false) = (form, cancelled) else {
        remove_upload(state, &filename);
        state.media.upload = None;
        forget_unfinished(state);
        state.media.status = CANCELLED_LINE.to_string();
        return;
    };
    match signer_here(state) {
        Some(signer) => {
            let fields = add::fields_of(&form, &filename, &state.media.list.categories, super::members_only_server(state));
            spawn(state, Job::Publish { of: Of::Upload, filename }, api::publish_request(&signer, &fields));
        }
        None => {
            // No signature to publish with (the identity was locked meanwhile): it waits a day.
            state.media.upload = None;
            forget_unfinished(state);
            if let Some(f) = state.media.form.as_mut() {
                let (name, len) = (f.file().0.to_string(), f.file().1);
                f.source = Source::Waiting { filename, name, len };
                f.sending = false;
                f.problem = "Uploaded, but not on the Media page yet: unlock your identity, then press Put it on Media. It waits a day.".to_string();
            }
        }
    }
}

/// A publish's answer (media.rs `on_done`).
pub(crate) fn published(state: &mut GuiState, of: Of, filename: &str, result: Result<Value, Refusal>) {
    let cancelled = state.media.cancelled_publishes.remove(filename);
    if of == Of::Upload {
        state.media.upload = None;
        forget_unfinished(state);
    }
    match result {
        Ok(v) => {
            if cancelled {
                // Cancelled while it was on its way: off again at once.
                remove_upload(state, filename);
                state.media.status = CANCELLED_LINE.to_string();
            } else {
                if state.media.form.as_ref().is_some_and(|f| f.sending) {
                    state.media.form = None;
                }
                let item = v.get("item").and_then(Item::from_json);
                state.media.status = match item {
                    Some(i) => format!("\"{}\" is on the Media page now.", i.display_title()),
                    None => "It is on the Media page now.".to_string(),
                };
            }
            super::reload(state);
        }
        Err(e) => {
            if cancelled {
                // A Media upload nobody wants any more goes; a shared file stays where it was.
                if of != Of::Shared {
                    remove_upload(state, filename);
                }
            } else if let Some(f) = state.media.form.as_mut().filter(|f| f.sending) {
                f.sending = false;
                if of == Of::Upload {
                    let (name, len) = (f.file().0.to_string(), f.file().1);
                    f.source = Source::Waiting { filename: filename.to_string(), name, len };
                    f.problem = format!("Not put on the Media page: {} The file is uploaded and waits a day: press the button again.", e.words);
                } else {
                    f.problem = e.words;
                }
            } else {
                state.media.status = e.words;
            }
            super::refresh_mine(state);
        }
    }
}

/// Take a Media upload (or the item made of it) off the server by its file name.
fn remove_upload(state: &mut GuiState, filename: &str) {
    if let Some(signer) = signer_here(state) {
        let req = api::remove_request(&signer, &RemoveTarget::Upload(filename.to_string()));
        spawn(state, Job::Remove { id: None, from_admin: false }, req);
    }
}

/// The form's Cancel while something is being sent: the upload stops (and the server drops what
/// arrived), or, when the file is up and its publish is on its way, that publish is undone as
/// soon as it lands.
pub(crate) fn cancel(state: &mut GuiState, form: &Form) {
    match (&form.source, state.media.upload.as_ref()) {
        (Source::Picked { .. }, Some(run)) => match &run.phase {
            Phase::Done { filename } => {
                state.media.cancelled_publishes.insert(filename.clone());
            }
            _ => run.ctl.cancel(),
        },
        _ => {
            if let Some(name) = form.stored() {
                state.media.cancelled_publishes.insert(name.to_string());
            }
        }
    }
    state.media.status = "Cancelling...".to_string();
}

/// Cancel an upload that did not finish, on the server too (it would go after a day anyway).
pub(crate) fn discard(state: &mut GuiState) {
    let Some(u) = state.media.unfinished.clone() else { return };
    if let (false, Some(token)) = (u.id.is_empty(), crate::gui::pages::chat::upload_token_here(state)) {
        let t = transport(state);
        spawn_with(state, Job::Discard, move || match t.cancel(&token, &u.id) {
            Ok(r) if r.status < 300 => Ok(r.json()),
            Ok(r) => Err(Refusal::new(r.status, r.words())),
            Err(words) => Err(Refusal::new(0, words)),
        });
    }
    forget_unfinished(state);
}

/// The unfinished upload offered for this server and person, when nothing is under way.
pub(crate) fn offered(state: &GuiState) -> Option<&Unfinished> {
    if state.media.upload.is_some() {
        return None;
    }
    state.media.unfinished.as_ref().filter(|u| !u.id.is_empty() && u.server == state.media.server && u.key == state.media.key)
}

/// Carry on the unfinished upload: only from the same file, unchanged. The form opens with what
/// it said, and only what the server does not have yet is sent.
pub(crate) fn resume(state: &mut GuiState) {
    let t = transport(state);
    resume_with(state, t);
}

/// [`resume`] through `t`.
pub(crate) fn resume_with(state: &mut GuiState, t: Arc<dyn Transport>) {
    let Some(u) = offered(state).cloned() else { return };
    if !u.file_unchanged() {
        discard(state);
        state.media.status = resumable::FILE_CHANGED.to_string();
        return;
    }
    let Some(token) = crate::gui::pages::chat::upload_token_here(state) else {
        state.media.status = crate::gui::pages::chat::NOT_SIGNED_IN_YET.to_string();
        return;
    };
    let mut form = Form::new(Source::Picked { path: u.path.clone(), name: u.name.clone(), len: u.size });
    form.title = u.title.clone();
    form.description = u.description.clone();
    form.category = Some(u.category_id);
    form.labels = u.labels.iter().cloned().collect();
    form.members_only = u.members_only;
    form.sending = true;
    state.media.form = Some(form);
    let plan = Plan { path: u.path.clone(), name: u.name.clone(), size: u.size, content_type: crate::gui::pages::chat::mime_for_filename(&u.name).to_string(), resume: Some(u.id) };
    state.media.upload = Some(Run::start(t, plan, Arc::new(Control::new(Some(token)))));
    super::refresh_mine(state);
}

/// The progress bar of the upload under way, its words, and Pause, Resume or Retry.
pub(crate) fn draw_progress(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let Some(run) = state.media.upload.as_ref() else { return };
    widgets::progress_bar(ui, theme, run.fraction(), None);
    let color = match run.phase {
        Phase::Waiting { .. } | Phase::NoRoom { .. } | Phase::Reconnecting { .. } => theme.warning(),
        Phase::Failed { .. } => theme.danger(),
        _ => theme.text_secondary(),
    };
    ui.label(RichText::new(run.words()).size(theme.font_size_small).color(color));
    ui.horizontal(|ui| match run.phase {
        Phase::Hashing | Phase::Sending | Phase::Reconnecting { .. } => {
            if widgets::Button::secondary("Pause").tooltip("Stop after the piece being sent; Resume carries on from there.").show(ui, theme) {
                run.ctl.pause(true);
            }
        }
        Phase::Paused | Phase::Waiting { .. } => {
            if widgets::Button::primary("Resume").show(ui, theme) {
                run.ctl.pause(false);
            }
        }
        Phase::NoRoom { .. } => {
            if widgets::Button::primary("Retry").tooltip("Try the finish again: the whole file is already on the server.").show(ui, theme) {
                run.ctl.retry();
            }
        }
        _ => {}
    });
}

/// On the page: the offer to carry on an upload a restart cut off, and the person's own uploads
/// waiting to be put on Media.
pub(crate) fn draw_on_page(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if let Some(u) = offered(state).cloned() {
        let mut carry_on = false;
        let mut drop_it = false;
        widgets::card(ui, theme, |ui| {
            ui.label(RichText::new(format!("An upload did not finish: {} ({}).", u.name, api::size_words(u.size as i64))).color(theme.text_primary()).strong());
            widgets::body_hint(ui, theme, "The server keeps what arrived for a day after the last piece. Carry on sends only the rest, from the same file.");
            ui.horizontal(|ui| {
                carry_on = widgets::Button::primary("Carry on").show(ui, theme);
                drop_it = widgets::Button::secondary("Cancel it").tooltip("The server drops what arrived.").show(ui, theme);
            });
        });
        if carry_on {
            resume(state);
        } else if drop_it {
            discard(state);
        }
    }

    // What is being published now is not offered again below.
    let busy: Option<String> = state
        .media
        .upload
        .as_ref()
        .and_then(|r| if let Phase::Done { filename } = &r.phase { Some(filename.clone()) } else { None })
        .or_else(|| state.media.form.as_ref().and_then(|f| f.stored().map(str::to_string)));
    let waiting: Vec<Waiting> = state.media.mine.as_ref().map(|m| m.waiting.iter().filter(|w| Some(&w.filename) != busy.as_ref()).cloned().collect()).unwrap_or_default();
    if waiting.is_empty() || !super::signed_in_here(state) {
        return;
    }
    widgets::subsection_label(ui, theme, "Your uploads waiting to be put on Media");
    widgets::body_hint(ui, theme, "Each is deleted a day after it was uploaded, unless it is put on the Media page.");
    let sending = state.media.form.as_ref().is_some_and(|f| f.sending);
    for w in waiting {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(format!("{}, {}", w.shown_name(), api::size_words(w.size_bytes))).size(theme.font_size_small).color(theme.text_primary()));
            let small = widgets::ButtonSize::Small;
            if widgets::Button::secondary("Put it on Media").size(small).disabled(sending).show(ui, theme) {
                let source = Source::Waiting { filename: w.filename.clone(), name: w.shown_name().to_string(), len: w.size_bytes.max(0) as u64 };
                state.media.form = Some(Form::new(source));
            }
            if widgets::Button::danger("Remove").size(small).show(ui, theme) {
                state.media.confirm_waiting = Some(w.clone());
            }
        });
    }
}

/// The waiting upload's Remove confirmation: it is deleted from the server.
pub(crate) fn draw_confirm_waiting(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(w) = state.media.confirm_waiting.clone() else { return };
    let mut open = true;
    let mut confirmed = false;
    let mut cancelled = false;
    widgets::dialog(ctx, theme, "media_confirm_waiting", "Remove this upload", &mut open, |ui| {
        ui.label(RichText::new(w.shown_name()).size(theme.font_size_body).color(theme.text_primary()).strong());
        ui.label(RichText::new(api::REMOVE_DELETES).size(theme.font_size_body).color(theme.text_secondary()));
        ui.horizontal(|ui| {
            confirmed = widgets::Button::danger("Remove").show(ui, theme);
            cancelled = widgets::Button::secondary("Cancel").show(ui, theme);
        });
    });
    if confirmed {
        if let Some(signer) = super::signer_or_say(state) {
            let req = api::remove_request(&signer, &RemoveTarget::Upload(w.filename.clone()));
            spawn(state, Job::Remove { id: None, from_admin: false }, req);
        }
    }
    if confirmed || cancelled || !open {
        state.media.confirm_waiting = None;
    }
}
