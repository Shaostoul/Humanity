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
//!   its words, Put it on Media and Cancel, which is the same `DELETE`: the relay removes a
//!   finished upload's stored file with its session (round 4), so no signature is needed.
//!
//! ADDED TO AN ITEM INSTEAD (Media round 5, lane 1D): a file or a cover picked in an item's Edit
//! window is the same upload, in the same list, carrying the item it goes to (`AttachTo`, kept in
//! its record too); once it is up it is added to that item (`POST /api/media/files/add`,
//! publishing.rs `attach`) instead of published, and its Cancel takes it off the item again.
//!
//! "Cancelled" is said only when the server confirmed it (round 4 of the design, D4-2): every
//! cancel is asked again while the server is busy writing (upload_cancel.rs), and one the server
//! did not confirm is said plainly, the upload staying listed with its Cancel until it is.
//!
//! THE RECORDS (`MediaState::unfinished`, kept in the app's config) are each upload the server has
//! named, so one a restart or a switch cut off can be carried on: on the same server, by the same
//! person, from the same file unchanged. A record goes when its upload is published, cancelled or
//! gone from the server. Several uploads can run at once; the relay allows three per person, and a
//! fourth waits, saying so, until one ends.

use std::sync::Arc;
use std::time::Duration;

use egui::RichText;
use serde_json::{json, Value};

use super::add::{self, Form, Of, Source};
use super::api::{self, Refusal};
use super::publishing::{self, Undo};
use super::resumable::{self, Control, Event, Phase, Plan, Run, Transport, Unfinished};
use super::upload_cancel::{self, Told};
use super::{signer_here, spawn_with, Job};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// What is said when an upload, or its publish, was cancelled, once the server confirmed it.
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
    /// Cancel was pressed: waiting for the worker, the publish or the server to say it stopped.
    pub cancelling: bool,
    /// The server's resumable routes it goes through, which its Cancel asks too once the file is
    /// up (a simulated server in the tests).
    pub transport: Arc<dyn Transport>,
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
    let Some(token) = super::token_here(state) else {
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
        licence: fields.licence,
        project_use: fields.project_use,
        attach: None,
    };
    push_upload(state, record, form.remove, token, t);
    Ok(())
}

/// Start the upload `record` describes, cleaned on this device first when `clean` (the form's
/// switch) or sent exactly as it is (`keep_metadata`): a new row in the list.
fn push_upload(state: &mut GuiState, record: Unfinished, clean: bool, token: String, t: Arc<dyn Transport>) {
    let plan = Plan {
        path: record.path.clone(),
        name: record.name.clone(),
        size: record.size,
        content_type: crate::gui::pages::chat::mime_for_filename(&record.name).to_string(),
        resume: None,
        file_changed: false,
        clean,
        keep_metadata: !clean,
    };
    let run = Run::start(t.clone(), plan, Arc::new(Control::new(Some(token))));
    state.media.uploads.push(Entry { local: super::next_id(), run, record, after: After::Uploading, cancelling: false, transport: t });
}

/// Start uploading the file at `path` (named `name`, `len` bytes) for an item's Edit window (Media
/// round 5, lane 1D): the same upload in pieces and the same row under "Uploads in progress" as
/// Add to Media's, added to the item `to` names as a file or its cover once it is up, instead of
/// published. Cleaned on this device first under the person's own switch ("Remove location and
/// camera details", always on under the protected setup), as a picked file is. Err with the
/// words when it cannot start.
pub(crate) fn start_attach(state: &mut GuiState, path: &std::path::Path, name: &str, len: u64, to: super::api_items::AttachTo) -> Result<(), String> {
    let t = transport(state);
    start_attach_with(state, path, name, len, to, t)
}

/// [`start_attach`] through `t` (the tests' simulated server).
pub(crate) fn start_attach_with(state: &mut GuiState, path: &std::path::Path, name: &str, len: u64, to: super::api_items::AttachTo, t: Arc<dyn Transport>) -> Result<(), String> {
    let Some(token) = super::token_here(state) else {
        return Err(crate::gui::pages::chat::NOT_SIGNED_IN_YET.to_string());
    };
    let record = Unfinished {
        server: state.media.server.clone(),
        key: state.media.key.clone(),
        id: String::new(),
        path: path.to_path_buf(),
        name: name.to_string(),
        size: len,
        modified_ms: resumable::modified_ms(path),
        attach: Some(to),
        ..Default::default()
    };
    let clean = crate::gui::pages::chat::attach_step::remove_at_start(state);
    push_upload(state, record, clean, token, t);
    Ok(())
}

/// Read what every upload's worker reported, and act on it. Every frame (media.rs `poll_upload`).
pub(crate) fn poll(ctx: &egui::Context, state: &mut GuiState) {
    sweep_cleaned(state);
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

/// Remove the cleaned copies no upload uses any more (an upload published, cancelled or forgotten),
/// once a minute: in use are the copies of the uploads running and of the unfinished records kept in
/// the config; a copy untouched for ten minutes that neither names is removed (2026-10-11).
pub(crate) fn sweep_cleaned(state: &mut GuiState) {
    let minute = std::time::Duration::from_secs(60);
    if state.media.last_sweep.is_some_and(|t| t.elapsed() < minute) {
        return;
    }
    state.media.last_sweep = Some(std::time::Instant::now());
    let in_use: Vec<std::path::PathBuf> = state.media.uploads.iter().map(|e| e.record.path.clone()).chain(state.media.unfinished.iter().map(|u| u.path.clone())).collect();
    crate::net::file_clean::sweep_cleaned(&crate::net::file_clean::cleaned_dir(), &in_use, 10 * minute);
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
                // Cancel came after the last piece went: the file is up but not on Media, and the
                // server removes it with its session (round 4), asked with the upload token, so it
                // goes whether or not the identity is unlocked. Round 3 sent a signed remove here,
                // which with no signature sent nothing and still said "Cancelled".
                entry.after = After::NotPublished { filename, words: String::new() };
                cancel_entry(state, local);
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
        Event::Cancelled { told } => {
            let Some(gone_entry) = drop_entry(state, local) else { return };
            match told {
                Told::Gone | Told::Published => {
                    forget_record(state, &gone_entry.record.id);
                    state.media.status = if told == Told::Gone { CANCELLED_LINE } else { upload_cancel::ALREADY_ON_MEDIA }.to_string();
                }
                // Not confirmed: its record stays, offered below with Cancel it (D4-2).
                Told::NotConfirmed(why) => state.media.status = upload_cancel::not_cancelled_words(&why),
            }
            super::refresh_mine(state);
        }
        // The worker cleaned the file into a copy, which is what goes up: the record follows the
        // copy, so an upload carried on after a restart reads the same bytes (2026-10-11).
        Event::Cleaned { path, size, modified_ms } => {
            let Some(entry) = entry(state, local) else { return };
            entry.record.path = path;
            entry.record.size = size;
            entry.record.modified_ms = modified_ms;
        }
        _ => {}
    }
}

/// Upload `local`'s file is up as `filename`: publish it with what its own form said, signed now;
/// or, made from an item's Edit window (round 5), add it to that item instead.
fn publish_entry(state: &mut GuiState, local: u64, filename: String) {
    let Some(record) = entry(state, local).map(|e| e.record.clone()) else { return };
    match signer_here(state) {
        Some(signer) => {
            match &record.attach {
                Some(to) => publishing::attach(state, local, &signer, to, &filename),
                None => {
                    let fields = fields_of_record(&record, &filename, super::members_only_server(state));
                    publishing::publish(state, Of::Upload, local, &signer, &fields);
                }
            }
            if let Some(e) = entry(state, local) {
                e.after = After::Publishing { filename };
            }
        }
        None => {
            let words = if record.attach.is_some() { UNLOCK_TO_ATTACH } else { UNLOCK_TO_PUBLISH };
            if let Some(e) = entry(state, local) {
                e.after = After::NotPublished { filename, words: words.to_string() };
            }
        }
    }
}

/// What an upload made for an item says when it is up but there is no signature to add it with.
pub(crate) const UNLOCK_TO_ATTACH: &str = "Uploaded, but not added to the item yet: unlock your identity, then press Add it to the item. It waits a day.";
/// The button that adds an upload that is up to its item (the "Put it on Media" of one made from
/// an item's Edit window).
pub(crate) const ADD_TO_ITEM: &str = "Add it to the item";

/// What an upload is published as: what its form said when it was sent.
pub(crate) fn fields_of_record(r: &Unfinished, filename: &str, members_server: bool) -> api::PublishFields {
    api::PublishFields {
        filename: filename.to_string(),
        title: r.title.clone(),
        description: r.description.clone(),
        category_id: r.category_id,
        labels: r.labels.clone(),
        visibility: if members_server || r.members_only { "members" } else { "public" }.to_string(),
        // The licence and the game tick (Media round 5): what the form said.
        licence: r.licence.clone(),
        project_use: r.project_use,
    }
}

/// Upload `local`'s publish is answered (publishing.rs `published`). After a Cancel, `result` is
/// what undoing it got when the publish's own worker undid it (`Undo::Worker`); when the page
/// did (`Undo::Page`), that undoing's own answer says how it went.
pub(crate) fn published(state: &mut GuiState, local: u64, filename: &str, result: Result<Value, Refusal>, undo: Undo) {
    match undo {
        Undo::Worker => {
            drop_entry(state, local);
            state.media.status = match result {
                Ok(_) => CANCELLED_LINE.to_string(),
                Err(e) => publishing::not_taken_off_words(&e.words),
            };
            super::refresh_mine(state);
            return;
        }
        Undo::Page => {
            drop_entry(state, local);
            super::refresh_mine(state);
            return;
        }
        Undo::None => {}
    }
    // Made from an item's Edit window (round 5): added to that item, not published.
    let attached = entry(state, local).is_some_and(|e| e.record.attach.is_some());
    match result {
        Ok(v) => {
            if let Some(gone) = drop_entry(state, local) {
                forget_record(state, &gone.record.id);
            }
            if attached {
                state.media.status = publishing::attached_words(&v);
                super::refresh_mine(state);
            } else {
                state.media.status = publishing::on_media_words(&v);
                super::reload(state);
            }
        }
        Err(e) => {
            let words = if attached {
                format!("Not added to the item: {} The file is uploaded and waits a day.", e.words)
            } else {
                format!("Not put on the Media page: {} The file is uploaded and waits a day.", e.words)
            };
            match entry(state, local) {
                Some(entry) => entry.after = After::NotPublished { filename: filename.to_string(), words },
                None => state.media.status = e.words,
            }
            super::refresh_mine(state);
        }
    }
}

/// A row's Cancel: the upload stops (and the server drops what arrived); a publish on its way is
/// undone when it lands; an upload that is up but not published is removed from the server now.
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
        After::NotPublished { .. } => cancel_entry(state, local),
    }
}

/// Ask the server to drop upload `local`'s session and the file stored from it (its file is up
/// but not on Media), through the upload's own routes; the row shows "Cancelling..." until the
/// server answers (`cancel_answered`), and gets its Cancel back when it cannot be asked.
fn cancel_entry(state: &mut GuiState, local: u64) {
    let Some(e) = entry(state, local) else { return };
    e.cancelling = true;
    let (id, t, pace) = (e.record.id.clone(), e.transport.clone(), e.run.ctl.pace);
    if !cancel_on_server_with(state, &id, t, pace) {
        let words = state.media.status.clone();
        if let Some(e) = entry(state, local) {
            e.cancelling = false;
            if let After::NotPublished { words: w, .. } = &mut e.after {
                *w = words;
            }
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

/// Cancel the upload the record `id` names, on the server too; the record goes once the server
/// confirmed (`cancel_answered`).
pub(crate) fn discard(state: &mut GuiState, id: &str) {
    cancel_on_server(state, id);
}

/// Ask the server to drop its upload `id` (a record's, or one of the person's unfinished uploads
/// the server listed) on a worker. See [`cancel_on_server_with`].
pub(crate) fn cancel_on_server(state: &mut GuiState, id: &str) -> bool {
    let t = transport(state);
    cancel_on_server_with(state, id, t, upload_cancel::CANCEL_PACE)
}

/// Ask the server at `t` to drop its upload `id` on a worker, again while it is busy writing
/// (`pace` the first wait between asks; upload_cancel.rs); the answer comes back as
/// `Job::Discard` (`cancel_answered`), and until then `id` is in `MediaState::cancelling`. False,
/// with the reason said on the page, when it cannot be asked: no id, or no upload token for this
/// server. A cancel already on its way is not asked twice.
pub(crate) fn cancel_on_server_with(state: &mut GuiState, id: &str, t: Arc<dyn Transport>, pace: Duration) -> bool {
    if state.media.cancelling.iter().any(|c| c == id) {
        return true;
    }
    let token = super::token_here(state);
    let (Some(token), false) = (token, id.is_empty()) else {
        state.media.status = upload_cancel::not_cancelled_words(upload_cancel::NOT_SIGNED_IN);
        return false;
    };
    state.media.cancelling.push(id.to_string());
    let id = id.to_string();
    spawn_with(state, Job::Discard { id: id.clone() }, move || match upload_cancel::tell(t.as_ref(), &token, &id, pace) {
        Told::Gone => Ok(json!({ "ok": true })),
        Told::Published => Ok(json!({ "ok": true, "published": true })),
        Told::NotConfirmed(why) => Err(Refusal::new(0, why)),
    });
    true
}

/// The server answered the cancel of its upload `id` (`Job::Discard`). Confirmed: it goes from
/// every list here (the uploads in progress, the records, the server's unfinished uploads) and
/// "Cancelled" is said, or that it is on Media and stays there. Not confirmed: it stays where it
/// was listed, with its Cancel, and the person is told why (round 4, D4-2: round 3 took the row
/// off at the press, even when the server answered 409 busy).
pub(crate) fn cancel_answered(state: &mut GuiState, id: &str, result: Result<Value, Refusal>) {
    state.media.cancelling.retain(|c| c != id);
    match result {
        Ok(v) => {
            if let Some(local) = state.media.uploads.iter().find(|e| e.record.id == id).map(|e| e.local) {
                drop_entry(state, local);
            }
            forget_record(state, id);
            if let Some(m) = state.media.mine.as_mut() {
                m.in_progress.retain(|p| p.id != id);
            }
            let published = v.get("published").and_then(Value::as_bool) == Some(true);
            state.media.status = if published { upload_cancel::ALREADY_ON_MEDIA } else { CANCELLED_LINE }.to_string();
            super::refresh_mine(state);
        }
        Err(e) => {
            let words = upload_cancel::not_cancelled_words(&e.words);
            for entry in state.media.uploads.iter_mut().filter(|e| e.record.id == id) {
                entry.cancelling = false;
                if let After::NotPublished { words: w, .. } = &mut entry.after {
                    *w = words.clone();
                }
            }
            state.media.status = words;
        }
    }
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

/// Carry on the upload the record `id` names. It joins the list with what its form said, and only
/// what the server does not have yet is sent, from the same file unchanged. A file changed, moved
/// or gone since is carried on too when the server already has every piece (a 507 kept them, or
/// the finish's answer was lost): it is finished from the server's copy and never needs the file.
/// With pieces still to come from it, the server is told to drop what arrived (round 4, D4-1:
/// round 3 cancelled the server's whole copy here whenever the local file had changed).
pub(crate) fn resume(state: &mut GuiState, id: &str) {
    let t = transport(state);
    resume_with(state, id, t);
}

/// [`resume`] through `t`.
pub(crate) fn resume_with(state: &mut GuiState, id: &str, t: Arc<dyn Transport>) {
    let Some(u) = offered(state).into_iter().find(|u| u.id == id) else { return };
    let Some(token) = super::token_here(state) else {
        state.media.status = crate::gui::pages::chat::NOT_SIGNED_IN_YET.to_string();
        return;
    };
    let content_type = crate::gui::pages::chat::mime_for_filename(&u.name).to_string();
    // Carried on: the server already has the start (and with it any keep_metadata), and the record
    // points at the cleaned copy when there is one, so nothing is cleaned again.
    let plan = Plan { path: u.path.clone(), name: u.name.clone(), size: u.size, content_type, resume: Some(u.id.clone()), file_changed: !u.file_unchanged(), clean: false, keep_metadata: false };
    let run = Run::start(t.clone(), plan, Arc::new(Control::new(Some(token))));
    state.media.uploads.push(Entry { local: super::next_id(), run, record: u, after: After::Uploading, cancelling: false, transport: t });
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
        let heading = row_heading(state, &e.record);
        widgets::card(ui, theme, |ui| {
            ui.label(RichText::new(heading).color(theme.text_primary()).strong());
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
        let cancelling = state.media.cancelling.contains(&u.id);
        widgets::card(ui, theme, |ui| {
            ui.label(RichText::new(format!("An upload did not finish: {} ({}).", u.name, api::size_words(u.size as i64))).color(theme.text_primary()).strong());
            widgets::body_hint(ui, theme, "The server keeps what arrived for a day after the last piece. Carry on sends only the rest, from the same file.");
            if cancelling {
                // Listed until the server confirmed the cancel (round 4, D4-2).
                ui.label(RichText::new("Cancelling...").size(theme.font_size_small).color(theme.text_muted()));
                return;
            }
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

/// What a row is headed: the file and its size, and for one made from an item's Edit window what
/// it goes to ("textures.zip (4 MB), to add to Dusk", "dusk.jpg (1 MB), the cover for Dusk"), the
/// item named by its title wherever this page shows it, else by its number.
pub(crate) fn row_heading(state: &GuiState, record: &Unfinished) -> String {
    let file = format!("{} ({})", record.name, api::size_words(record.size as i64));
    let Some(to) = &record.attach else { return file };
    let page = state.media.item.as_ref().and_then(|p| p.item.as_ref());
    let title = page
        .into_iter()
        .chain(state.media.list.items.iter())
        .chain(state.media.featured.iter())
        .find(|i| i.id == to.id)
        .map(|i| i.display_title())
        .unwrap_or_else(|| format!("item {}", to.id));
    match to.role {
        super::api_items::FileRole::File => format!("{file}, to add to {title}"),
        super::api_items::FileRole::Cover => format!("{file}, the cover for {title}"),
    }
}

/// A row's button `press` was pressed (its label).
pub(crate) fn press_on(state: &mut GuiState, local: u64, press: &str) {
    match press {
        "Cancel" => cancel(state, local),
        "Put it on Media" | ADD_TO_ITEM => {
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
            let words = if e.record.attach.is_some() { "Uploaded. Adding it to the item..." } else { "Uploaded. Putting it on the Media page..." };
            ui.label(RichText::new(words).size(theme.font_size_small).color(theme.text_secondary()));
            if widgets::Button::secondary("Cancel").size(small).tooltip("Take it off again: nothing of it stays on the server.").show(ui, theme) {
                pressed = Some("Cancel");
            }
        }
        After::NotPublished { words, .. } => {
            ui.label(RichText::new(words).size(theme.font_size_small).color(theme.warning()));
            ui.horizontal(|ui| {
                let again = if e.record.attach.is_some() { ADD_TO_ITEM } else { "Put it on Media" };
                if widgets::Button::primary(again).size(small).show(ui, theme) {
                    pressed = Some(again);
                }
                if widgets::Button::secondary("Cancel").size(small).tooltip("Delete the uploaded file from the server.").show(ui, theme) {
                    pressed = Some("Cancel");
                }
            });
        }
    }
    pressed
}
