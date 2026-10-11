//! The uploads in progress, as a list of their own (round 3 of docs/design/media-showcase.md, the
//! 2026-10-11 review's finding 1). Each upload carries what its form said when it was sent, and is
//! published from that, by itself, once it is up: no form is held open for it, and opening another
//! form (Put on Media from the Files page, a waiting upload's Put it on Media) never touches it.
//! Round 2 tied the upload to the one open form, so Put on Media during an upload replaced that
//! form, and the finished upload, finding no form, was taken for a Cancel and deleted.
//!
//! Each row of the list has its bar, its words, Pause or Resume (or Retry), and Cancel:
//! - before the file is up, Cancel stops the worker, which tells the server (`DELETE`);
//! - once it is up and its publish is on its way, the publish is undone when it lands, even after
//!   a switch to another server or person (publishing.rs);
//! - a publish that did not go (refused, or no signature to send it with) stays in the list with
//!   its words, Put it on Media and Cancel.
//!
//! THE RECORDS (`MediaState::unfinished`, kept in the app's config) are each upload the server has
//! named, so one a restart or a switch cut off can be carried on: on the same server, by the same
//! person, from the same file unchanged. A record goes when its upload is published, cancelled or
//! gone from the server. Several uploads can run at once; the relay allows three per person, and a
//! fourth waits, saying so, until one ends.

use std::sync::Arc;

use egui::RichText;
use serde_json::Value;

use super::add::{self, Form, Of, Source};
use super::api::{self, Refusal};
use super::publishing;
use super::resumable::{self, Control, Event, Phase, Plan, Run, Transport, Unfinished};
use super::{signer_here, spawn_with, Job};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// What is said when an upload, or its publish, was cancelled.
pub(crate) const CANCELLED_LINE: &str = "Cancelled: nothing of it stays on the server.";
/// What an upload that is up says when there is no signature to publish it with.
pub(crate) const UNLOCK_TO_PUBLISH: &str = "Uploaded, but not on the Media page yet: unlock your identity, then press Put it on Media. It waits a day.";

/// What happens to an upload after its file is up.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum After {
    /// Still uploading: the run's phase says where it is.
    Uploading,
    /// Up, and its publish on its way.
    Publishing { filename: String },
    /// Up, but not on the Media page: why, and the stored name it is published by.
    NotPublished { filename: String, words: String },
}

/// One upload in the list.
pub(crate) struct Entry {
    /// Its number on this page (`media::next_id`): its publish's answer finds it by this.
    pub local: u64,
    pub run: Run,
    /// What it is, where it goes, and what its form said; once the server names it, also in the
    /// config's records.
    pub record: Unfinished,
    pub after: After,
    /// Cancel was pressed: waiting for the worker or the publish to say it stopped.
    pub cancelling: bool,
}

fn save_config(state: &GuiState) {
    crate::config::AppConfig::from_gui_state(state).save();
}

/// Keep `record` (one the server has named) in the config, replacing the one with its id.
fn keep_record(state: &mut GuiState, record: Unfinished) {
    state.media.unfinished.retain(|u| u.id != record.id);
    state.media.unfinished.push(record);
    save_config(state);
}

/// Forget the record with `id`, in the config too.
pub(crate) fn forget_record(state: &mut GuiState, id: &str) {
    let before = state.media.unfinished.len();
    state.media.unfinished.retain(|u| u.id != id || id.is_empty());
    if state.media.unfinished.len() != before {
        save_config(state);
    }
}

/// The server's resumable upload routes.
fn transport(state: &GuiState) -> Arc<dyn Transport> {
    Arc::new(resumable::Http::new(&super::base_of(state)))
}

/// The entry with `local`, if it is still in the list.
fn entry(state: &mut GuiState, local: u64) -> Option<&mut Entry> {
    state.media.uploads.iter_mut().find(|e| e.local == local)
}

fn drop_entry(state: &mut GuiState, local: u64) -> Option<Entry> {
    let i = state.media.uploads.iter().position(|e| e.local == local)?;
    Some(state.media.uploads.remove(i))
}

/// Start uploading the form's picked file: a new row in the list, carrying what the form says.
/// Err with the words when it cannot start.
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
    let fields = add::fields_of(form, "", &state.media.list.categories, super::members_only_server(state));
    let record = Unfinished {
        server: state.media.server.clone(),
        key: state.media.key.clone(),
        id: String::new(), // until the server gives one: only then is it kept in the config
        path: path.clone(),
        name: name.clone(),
        size: *len,
        modified_ms: resumable::modified_ms(path),
        title: form.title.clone(),
        description: form.description.clone(),
        category_id: fields.category_id,
        labels: fields.labels,
        members_only: form.members_only,
    };
    let plan = Plan { path: path.clone(), name: name.clone(), size: *len, content_type: crate::gui::pages::chat::mime_for_filename(name).to_string(), resume: None };
    let run = Run::start(t, plan, Arc::new(Control::new(Some(token))));
    state.media.uploads.push(Entry { local: super::next_id(), run, record, after: After::Uploading, cancelling: false });
    Ok(())
}

/// Read what every upload's worker reported, and act on it. Every frame (media.rs `poll_upload`).
pub(crate) fn poll(ctx: &egui::Context, state: &mut GuiState) {
    if state.media.uploads.is_empty() {
        return;
    }
    let mut events = Vec::new();
    for e in state.media.uploads.iter_mut() {
        for ev in e.run.poll() {
            events.push((e.local, ev));
        }
    }
    ctx.request_repaint_after(std::time::Duration::from_millis(250));
    for (local, ev) in events {
        on_event(state, local, ev);
    }
}

/// One event from upload `local`'s worker.
pub(crate) fn on_event(state: &mut GuiState, local: u64, e: Event) {
    match e {
        Event::Started { id } => {
            let Some(entry) = entry(state, local) else { return };
            entry.record.id = id;
            let record = entry.record.clone();
            keep_record(state, record);
        }
        Event::Done { answer } => {
            let filename = resumable::stored_name_of(&answer);
            let Some(entry) = entry(state, local) else { return };
            if entry.cancelling || entry.run.ctl.is_cancelled() {
                // Cancel came after the last piece went: off again at once.
                let id = entry.record.id.clone();
                drop_entry(state, local);
                forget_record(state, &id);
                publishing::take_off_now(state, &filename);
                state.media.status = CANCELLED_LINE.to_string();
                return;
            }
            publish_entry(state, local, filename);
        }
        Event::Failed { words, gone } => {
            let Some(gone_entry) = drop_entry(state, local) else { return };
            if gone {
                forget_record(state, &gone_entry.record.id);
            }
            state.media.status = format!("{}: {words}", gone_entry.record.name);
            super::refresh_mine(state);
        }
        Event::Cancelled => {
            if let Some(gone_entry) = drop_entry(state, local) {
                forget_record(state, &gone_entry.record.id);
            }
            state.media.status = CANCELLED_LINE.to_string();
            super::refresh_mine(state);
        }
        _ => {}
    }
}

/// Upload `local`'s file is up as `filename`: publish it with what its own form said, signed now.
fn publish_entry(state: &mut GuiState, local: u64, filename: String) {
    let Some(record) = entry(state, local).map(|e| e.record.clone()) else { return };
    match signer_here(state) {
        Some(signer) => {
            let fields = fields_of_record(&record, &filename, super::members_only_server(state));
            publishing::publish(state, Of::Upload, local, &signer, &fields);
            if let Some(e) = entry(state, local) {
                e.after = After::Publishing { filename };
            }
        }
        None => {
            if let Some(e) = entry(state, local) {
                e.after = After::NotPublished { filename, words: UNLOCK_TO_PUBLISH.to_string() };
            }
        }
    }
}

/// What an upload is published as: what its form said when it was sent.
pub(crate) fn fields_of_record(r: &Unfinished, filename: &str, members_server: bool) -> api::PublishFields {
    api::PublishFields {
        filename: filename.to_string(),
        title: r.title.clone(),
        description: r.description.clone(),
        category_id: r.category_id,
        labels: r.labels.clone(),
        visibility: if members_server || r.members_only { "members" } else { "public" }.to_string(),
    }
}

/// Upload `local`'s publish is answered (publishing.rs `published`).
pub(crate) fn published(state: &mut GuiState, local: u64, filename: &str, result: Result<Value, Refusal>, cancelled: bool) {
    if cancelled {
        drop_entry(state, local);
        state.media.status = CANCELLED_LINE.to_string();
        super::refresh_mine(state);
        return;
    }
    match result {
        Ok(v) => {
            if let Some(gone) = drop_entry(state, local) {
                forget_record(state, &gone.record.id);
            }
            state.media.status = publishing::on_media_words(&v);
            super::reload(state);
        }
        Err(e) => {
            match entry(state, local) {
                Some(entry) => {
                    entry.after = After::NotPublished {
                        filename: filename.to_string(),
                        words: format!("Not put on the Media page: {} The file is uploaded and waits a day.", e.words),
                    }
                }
                None => state.media.status = e.words,
            }
            super::refresh_mine(state);
        }
    }
}

/// A row's Cancel: the upload stops (and the server drops what arrived); a publish on its way is
/// undone when it lands; an upload that is up but not published is taken off now.
pub(crate) fn cancel(state: &mut GuiState, local: u64) {
    let Some(e) = entry(state, local) else { return };
    let (after, id) = (e.after.clone(), e.record.id.clone());
    match after {
        After::Uploading => {
            e.run.ctl.cancel();
            e.cancelling = true;
        }
        After::Publishing { filename } => {
            e.cancelling = true;
            forget_record(state, &id);
            publishing::cancel(state, local, &filename);
        }
        After::NotPublished { filename, .. } => {
            drop_entry(state, local);
            forget_record(state, &id);
            publishing::take_off_now(state, &filename);
            state.media.status = CANCELLED_LINE.to_string();
        }
    }
}

/// Stop every upload where it is, without telling the server (which keeps each a day), for a
/// switch to another server or person: their records stay and are offered there again.
pub(crate) fn abandon_all(uploads: &[Entry]) {
    for e in uploads {
        e.run.ctl.abandon();
    }
}

/// Cancel the upload the record `id` names, on the server too (it would go after a day anyway).
pub(crate) fn discard(state: &mut GuiState, id: &str) {
    cancel_on_server(state, id);
    forget_record(state, id);
}

/// Tell the server to drop its upload `id` (a record's, or one of the person's unfinished
/// uploads the server listed).
pub(crate) fn cancel_on_server(state: &mut GuiState, id: &str) {
    let Some(token) = crate::gui::pages::chat::upload_token_here(state) else { return };
    if id.is_empty() {
        return;
    }
    let (t, id) = (transport(state), id.to_string());
    spawn_with(state, Job::Discard, move || match t.cancel(&token, &id) {
        Ok(r) if r.status < 300 => Ok(r.json()),
        Ok(r) => Err(Refusal::new(r.status, r.words())),
        Err(words) => Err(Refusal::new(0, words)),
    });
}

/// The records offered here: this server's and person's, the server named, none of them running.
pub(crate) fn offered(state: &GuiState) -> Vec<Unfinished> {
    state
        .media
        .unfinished
        .iter()
        .filter(|u| !u.id.is_empty() && u.server == state.media.server && u.key == state.media.key)
        .filter(|u| !state.media.uploads.iter().any(|e| e.record.id == u.id))
        .cloned()
        .collect()
}

/// Carry on the upload the record `id` names: only from the same file, unchanged. It joins the
/// list with what its form said, and only what the server does not have yet is sent.
pub(crate) fn resume(state: &mut GuiState, id: &str) {
    let t = transport(state);
    resume_with(state, id, t);
}

/// [`resume`] through `t`.
pub(crate) fn resume_with(state: &mut GuiState, id: &str, t: Arc<dyn Transport>) {
    let Some(u) = offered(state).into_iter().find(|u| u.id == id) else { return };
    if !u.file_unchanged() {
        discard(state, &u.id);
        state.media.status = resumable::FILE_CHANGED.to_string();
        return;
    }
    let Some(token) = crate::gui::pages::chat::upload_token_here(state) else {
        state.media.status = crate::gui::pages::chat::NOT_SIGNED_IN_YET.to_string();
        return;
    };
    let plan = Plan { path: u.path.clone(), name: u.name.clone(), size: u.size, content_type: crate::gui::pages::chat::mime_for_filename(&u.name).to_string(), resume: Some(u.id.clone()) };
    let run = Run::start(t, plan, Arc::new(Control::new(Some(token))));
    state.media.uploads.push(Entry { local: super::next_id(), run, record: u, after: After::Uploading, cancelling: false });
    super::refresh_mine(state);
}

/// The uploads in progress, each with its bar, its words and its buttons; then the records a
/// restart or a switch cut off, with Carry on.
pub(crate) fn draw_list(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if !state.media.uploads.is_empty() {
        widgets::subsection_label(ui, theme, "Uploads in progress");
    }
    let mut pressed: Vec<(u64, &'static str)> = Vec::new();
    for e in &state.media.uploads {
        widgets::card(ui, theme, |ui| {
            ui.label(RichText::new(format!("{} ({})", e.record.name, api::size_words(e.record.size as i64))).color(theme.text_primary()).strong());
            if let Some(press) = draw_row(ui, theme, e) {
                pressed.push((e.local, press));
            }
        });
    }
    for (local, press) in pressed {
        press_on(state, local, press);
    }

    for u in offered(state) {
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
            resume(state, &u.id);
        } else if drop_it {
            discard(state, &u.id);
        }
    }
}

/// A row's button `press` was pressed (its label).
pub(crate) fn press_on(state: &mut GuiState, local: u64, press: &str) {
    match press {
        "Cancel" => cancel(state, local),
        "Put it on Media" => {
            if let Some(After::NotPublished { filename, .. }) = entry(state, local).map(|e| e.after.clone()) {
                publish_entry(state, local, filename);
            }
        }
        _ => {
            if let Some(e) = entry(state, local) {
                match press {
                    "Pause" => e.run.ctl.pause(true),
                    "Resume" => e.run.ctl.pause(false),
                    _ => e.run.ctl.retry(),
                }
            }
        }
    }
}

/// One upload's bar, words and buttons; the button pressed, if one was.
fn draw_row(ui: &mut egui::Ui, theme: &Theme, e: &Entry) -> Option<&'static str> {
    let mut pressed = None;
    let small = widgets::ButtonSize::Small;
    if e.cancelling {
        ui.label(RichText::new("Cancelling...").size(theme.font_size_small).color(theme.text_muted()));
        return None;
    }
    match &e.after {
        After::Uploading => {
            widgets::progress_bar(ui, theme, e.run.fraction(), None);
            let color = match e.run.phase {
                Phase::Waiting { .. } | Phase::NoRoom { .. } | Phase::Reconnecting { .. } | Phase::Stuck { .. } => theme.warning(),
                Phase::Failed { .. } => theme.danger(),
                _ => theme.text_secondary(),
            };
            ui.label(RichText::new(e.run.words()).size(theme.font_size_small).color(color));
            ui.horizontal(|ui| {
                match e.run.phase {
                    Phase::Hashing | Phase::Sending | Phase::Reconnecting { .. } => {
                        if widgets::Button::secondary("Pause").size(small).tooltip("Stop sending; Resume carries on from where the server got.").show(ui, theme) {
                            pressed = Some("Pause");
                        }
                    }
                    Phase::Paused | Phase::Waiting { .. } => {
                        if widgets::Button::primary("Resume").size(small).show(ui, theme) {
                            pressed = Some("Resume");
                        }
                    }
                    Phase::NoRoom { .. } | Phase::Stuck { .. } => {
                        if widgets::Button::primary("Retry").size(small).tooltip("Try again: what the server has is kept.").show(ui, theme) {
                            pressed = Some("Retry");
                        }
                    }
                    _ => {}
                }
                if widgets::Button::secondary("Cancel").size(small).tooltip("Stop the upload: nothing of it stays on the server.").show(ui, theme) {
                    pressed = Some("Cancel");
                }
            });
        }
        After::Publishing { .. } => {
            ui.label(RichText::new("Uploaded. Putting it on the Media page...").size(theme.font_size_small).color(theme.text_secondary()));
            if widgets::Button::secondary("Cancel").size(small).tooltip("Take it off again: nothing of it stays on the server.").show(ui, theme) {
                pressed = Some("Cancel");
            }
        }
        After::NotPublished { words, .. } => {
            ui.label(RichText::new(words).size(theme.font_size_small).color(theme.warning()));
            ui.horizontal(|ui| {
                if widgets::Button::primary("Put it on Media").size(small).show(ui, theme) {
                    pressed = Some("Put it on Media");
                }
                if widgets::Button::secondary("Cancel").size(small).tooltip("Delete the uploaded file from the server.").show(ui, theme) {
                    pressed = Some("Cancel");
                }
            });
        }
    }
    pressed
}
